//! Transparent local tunnel for macOS and Linux.
//!
//! Instead of pointing the system proxy at the loopback engine, the app asks a
//! short-lived privileged helper to create a TUN interface and route all
//! traffic through it. The helper owns the TUN device and the userspace stack,
//! and forwards every TCP connection to the same proxy engine (including the
//! TLS ClientHello splitting and DNS over HTTPS) that the Windows build uses.
//!
//! The helper is started with the normal platform password prompt, reports
//! readiness and the proxy port over a loopback control connection, and tears
//! everything down when that connection closes, so a crash of the app cannot
//! leave routes behind.

mod dns;
mod helper;
pub mod routes;
pub mod stack;

pub use helper::run as run_helper;

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use super::{EngineError, TunnelConfig};

const START_TIMEOUT: Duration = Duration::from_secs(120);
const READ_TIMEOUT: Duration = Duration::from_secs(25);
const STATS_TIMEOUT: Duration = Duration::from_secs(3);

pub struct Tunnel {
    child: Child,
    control: TcpStream,
    reader: BufReader<TcpStream>,
    port: u16,
    name: String,
}

/// Live values reported by the helper for the session details panel.
#[derive(Clone, Debug)]
pub struct Stats {
    pub tunnel: String,
    pub device: String,
    pub dpi: bool,
    pub up: u64,
    pub down: u64,
}

impl Tunnel {
    /// Start the privileged helper and wait until it is ready to be activated.
    pub fn start(
        profile: &str,
        only: &[String],
        config: TunnelConfig,
    ) -> Result<Self, EngineError> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|error| EngineError::Failed(error.to_string()))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| EngineError::Failed(error.to_string()))?;
        let control_port = listener
            .local_addr()
            .map_err(|error| EngineError::Failed(error.to_string()))?
            .port();
        let token = token();
        let arguments = arguments(control_port, &token, profile, only, config);
        let mut child = launch(&arguments)?;

        let started = Instant::now();
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    let _ = child.kill();
                    return Err(EngineError::Failed(error.to_string()));
                }
            }
            if matches!(child.try_wait(), Ok(Some(_))) {
                return Err(EngineError::NeedsAdmin);
            }
            if started.elapsed() > START_TIMEOUT {
                let _ = child.kill();
                return Err(EngineError::Tunnel);
            }
            std::thread::sleep(Duration::from_millis(60));
        };

        socket
            .set_nonblocking(false)
            .and_then(|_| socket.set_read_timeout(Some(READ_TIMEOUT)))
            .map_err(|error| EngineError::Failed(error.to_string()))?;
        let mut reader = BufReader::new(
            socket
                .try_clone()
                .map_err(|error| EngineError::Failed(error.to_string()))?,
        );

        let mut line = String::new();
        read_line(&mut reader, &mut line)?;
        if line.trim() != format!("hello {token}") {
            let _ = child.kill();
            return Err(EngineError::Tunnel);
        }
        socket
            .write_all(b"ok\n")
            .map_err(|error| EngineError::Failed(error.to_string()))?;

        line.clear();
        read_line(&mut reader, &mut line)?;
        let mut parts = line.split_whitespace();
        let ready = parts.next() == Some("ready");
        let port = parts.next().and_then(|value| value.parse::<u16>().ok());
        let name = parts.next().unwrap_or("tun").to_string();
        let (true, Some(port)) = (ready, port) else {
            let _ = child.kill();
            return Err(EngineError::Tunnel);
        };

        Ok(Self { child, control: socket, reader, port, name })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Ask the helper to (re)install the routes and bring the tunnel up.
    pub fn up(&mut self) -> Result<(), EngineError> {
        self.control
            .write_all(b"up\n")
            .map_err(|error| EngineError::Failed(error.to_string()))?;
        let mut line = String::new();
        read_line(&mut self.reader, &mut line)?;
        if line.trim() == "ok" {
            Ok(())
        } else {
            Err(EngineError::Tunnel)
        }
    }

    /// Read the tunnel addresses, DPI state and byte counters from the helper.
    /// The session lock serialises this with `up`, so the short reply cannot be
    /// confused with another command's answer.
    pub fn stats(&mut self) -> Option<Stats> {
        self.control.set_read_timeout(Some(STATS_TIMEOUT)).ok()?;
        let result = self.stats_inner();
        let _ = self.control.set_read_timeout(Some(READ_TIMEOUT));
        result
    }

    fn stats_inner(&mut self) -> Option<Stats> {
        self.control.write_all(b"stats\n").ok()?;
        let mut line = String::new();
        if self.reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let mut parts = line.split_whitespace();
        if parts.next() != Some("stats") {
            return None;
        }
        Some(Stats {
            tunnel: parts.next()?.to_string(),
            device: parts.next()?.to_string(),
            dpi: parts.next()? == "1",
            up: parts.next()?.parse().ok()?,
            down: parts.next()?.parse().ok()?,
        })
    }

    pub async fn stop(self) {
        let _ = tokio::task::spawn_blocking(move || drop(self)).await;
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        let _ = self.control.write_all(b"quit\n");
        let _ = self.control.shutdown(Shutdown::Both);
        let until = Instant::now() + Duration::from_secs(4);
        while Instant::now() < until {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_line(reader: &mut BufReader<TcpStream>, line: &mut String) -> Result<(), EngineError> {
    reader
        .read_line(line)
        .map_err(|error| EngineError::Failed(error.to_string()))?;
    if line.is_empty() {
        return Err(EngineError::Tunnel);
    }
    Ok(())
}

fn arguments(
    control: u16,
    token: &str,
    profile: &str,
    only: &[String],
    config: TunnelConfig,
) -> Vec<String> {
    vec![
        "--tunnel-helper".into(),
        "--control".into(),
        control.to_string(),
        "--token".into(),
        token.into(),
        "--profile".into(),
        profile.into(),
        "--only".into(),
        serde_json::to_string(only).unwrap_or_else(|_| "[]".into()),
        "--tunnel".into(),
        format!("{}/{}", config.tunnel, config.prefix),
        "--device".into(),
        config.device.to_string(),
        "--dpi".into(),
        if config.dpi { "1" } else { "0" }.into(),
    ]
}

fn token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{nanos:x}{:x}", std::process::id())
}

fn launch(arguments: &[String]) -> Result<Child, EngineError> {
    let exe = std::env::current_exe().map_err(|error| EngineError::Failed(error.to_string()))?;
    if root() {
        return Command::new(&exe)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| EngineError::Failed(error.to_string()));
    }

    #[cfg(target_os = "macos")]
    {
        let command = std::iter::once(exe.to_string_lossy().into_owned())
            .chain(arguments.iter().cloned())
            .map(|argument| shell_quote(&argument))
            .collect::<Vec<_>>()
            .join(" ");
        let script = format!(
            "do shell script \"{} >/dev/null 2>&1\" with administrator privileges",
            command.replace('\\', "\\\\").replace('"', "\\\"")
        );
        Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| EngineError::Failed(error.to_string()))
    }

    #[cfg(target_os = "linux")]
    {
        let mut pkexec = Command::new("pkexec");
        pkexec
            .arg(&exe)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match pkexec.spawn() {
            Ok(child) => Ok(child),
            Err(_) => Command::new("sudo")
                .arg("-n")
                .arg(&exe)
                .args(arguments)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| EngineError::Failed(error.to_string())),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = exe;
        Err(EngineError::NeedsAdmin)
    }
}

#[cfg(unix)]
fn root() -> bool {
    // SAFETY: geteuid has no preconditions and never fails.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn root() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
