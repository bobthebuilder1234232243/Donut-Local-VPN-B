#[cfg_attr(windows, allow(dead_code))]
pub mod proxy;

#[cfg(windows)]
mod driver;
#[cfg(windows)]
pub use driver::{Engine, Running};

#[cfg(not(windows))]
mod proxied;
#[cfg(not(windows))]
pub use proxied::{Engine, Running};

#[cfg(not(windows))]
pub mod tunnel;

use std::net::Ipv4Addr;

/// Settings that shape the tunnel for one session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TunnelConfig {
    pub tunnel: Ipv4Addr,
    pub prefix: u8,
    pub device: Ipv4Addr,
    pub dpi: bool,
}

/// Values the running engine reports to the session details panel.
#[derive(Clone, Debug)]
pub struct SessionStats {
    pub tunnel_ip: Option<String>,
    pub device_ip: Option<String>,
    pub dpi: bool,
    pub bytes_up: Option<u64>,
    pub bytes_down: Option<u64>,
}

/// Pin a socket to the physical interface so tunnel traffic cannot loop back
/// into the tunnel. Linux uses `SO_BINDTODEVICE`; macOS uses `IP_BOUND_IF` /
/// `IPV6_BOUND_IF`, which take an interface index.
#[cfg(unix)]
pub fn bind_interface(
    socket: &socket2::Socket,
    interface: &str,
    ipv6: bool,
) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::num::NonZeroU32;

    let name = CString::new(interface)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: `name` is a valid NUL-terminated C string for the duration of the call.
    let index = unsafe { libc::if_nametoindex(name.as_ptr()) };
    if index == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let index = NonZeroU32::new(index)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    if ipv6 {
        socket.bind_device_by_index_v6(Some(index))
    } else {
        socket.bind_device_by_index_v4(Some(index))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    MissingFiles,
    NeedsAdmin,
    Tunnel,
    NoProfile,
    Failed(String),
}

impl EngineError {
    pub fn code(&self) -> &'static str {
        match self {
            EngineError::MissingFiles => "missing_files",
            EngineError::NeedsAdmin => "needs_admin",
            EngineError::Tunnel => "tunnel",
            EngineError::NoProfile => "no_profile",
            EngineError::Failed(_) => "engine",
        }
    }

    pub fn fatal(&self) -> bool {
        !matches!(self, EngineError::Failed(_))
    }
}
