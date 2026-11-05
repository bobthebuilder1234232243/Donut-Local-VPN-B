//! Privileged helper process.
//!
//! Creating a TUN interface and changing the default route needs root on both
//! macOS and Linux. The user interface starts this helper through the platform
//! password prompt (`osascript` on macOS, `pkexec` on Linux); the helper owns
//! the TUN device, the userspace stack and the loopback proxy, and exits as
//! soon as the unprivileged parent closes the control connection.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::watch;

use super::{routes, stack};
use crate::engine::proxy::{Method, Resolver, Server};

struct Args {
    control: u16,
    token: String,
    profile: String,
    only: Vec<String>,
}

impl Args {
    fn parse(raw: &[String]) -> Option<Self> {
        if !raw.iter().any(|argument| argument == "--tunnel-helper") {
            return None;
        }
        let value = |key: &str| {
            raw.iter()
                .position(|argument| argument == key)
                .and_then(|index| raw.get(index + 1))
                .cloned()
        };
        Some(Self {
            control: value("--control")?.parse().ok()?,
            token: value("--token")?,
            profile: value("--profile")?,
            only: value("--only")
                .and_then(|only| serde_json::from_str(&only).ok())
                .unwrap_or_default(),
        })
    }
}

/// Entry point used by `main` when the app is started with `--tunnel-helper`.
pub fn run() -> i32 {
    let Some(args) = Args::parse(&std::env::args().collect::<Vec<_>>()) else {
        eprintln!("donut-local-vpn: invalid tunnel helper arguments");
        return 2;
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("donut-local-vpn: {error}");
            return 1;
        }
    };
    match runtime.block_on(serve(args)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("donut-local-vpn: {error}");
            1
        }
    }
}

async fn serve(args: Args) -> Result<(), String> {
    let physical = routes::physical();
    let resolver = Arc::new(Resolver::new(physical.interface.clone()));
    let method = Method::parse(&args.profile).ok_or("invalid profile")?;
    let only = (!args.only.is_empty()).then(|| Arc::new(args.only.clone()));
    let server = Server::start(method, resolver.clone(), only, physical.interface.clone())
        .await
        .map_err(|error| error.to_string())?;
    let port = server.port();

    let (device, name) = stack::create().map_err(|error| error.to_string())?;
    let (stop, stopped) = watch::channel(false);
    let stack = tokio::spawn(stack::run(device, port, resolver, stopped));

    let control = TcpStream::connect(("127.0.0.1", args.control))
        .await
        .map_err(|error| format!("could not reach the app: {error}"))?;
    let (reader, mut writer) = control.into_split();
    let mut lines = BufReader::new(reader).lines();

    writer
        .write_all(format!("hello {}\n", args.token).as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    if lines.next_line().await.map_err(|error| error.to_string())?.as_deref().map(str::trim)
        != Some("ok")
    {
        return Err("the app did not accept the helper".into());
    }
    writer
        .write_all(format!("ready {port} {name}\n").as_bytes())
        .await
        .map_err(|error| error.to_string())?;

    let outcome = loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break Ok(()),
            Err(error) => break Err(error.to_string()),
        };
        match line.trim() {
            "up" => {
                let tun = name.clone();
                match tokio::task::spawn_blocking(move || routes::enable(&tun)).await {
                    Ok(Ok(())) => {
                        let _ = writer.write_all(b"ok\n").await;
                    }
                    Ok(Err(error)) => {
                        let _ = writer.write_all(format!("error {error}\n").as_bytes()).await;
                    }
                    Err(error) => {
                        let _ = writer.write_all(format!("error {error}\n").as_bytes()).await;
                    }
                }
            }
            "quit" | "down" => break Ok(()),
            "" => {}
            other => {
                let _ = writer
                    .write_all(format!("error unknown command {other}\n").as_bytes())
                    .await;
            }
        }
    };

    let tun = name.clone();
    let _ = tokio::task::spawn_blocking(move || routes::disable(&tun)).await;
    let _ = stop.send(true);
    let _ = tokio::time::timeout(Duration::from_secs(2), stack).await;
    drop(server);
    outcome
}
