//! Routing and interface setup for the TUN tunnel.
//!
//! Everything here runs inside the privileged helper, so the commands are
//! executed as root. The default route is replaced by two /1 routes pointing at
//! the TUN interface; the original default route stays in place and is used for
//! the DNS over HTTPS endpoints the proxy needs when the DPI bypass is on. Those
//! endpoints are pinned with /32 host routes so they never loop back into the
//! tunnel.

use std::net::IpAddr;
use std::path::Path;
use std::process::{Command, Stdio};

const DNS_ENDPOINTS: &[&str] = &["1.1.1.1", "1.0.0.1", "8.8.8.8", "8.8.4.4"];
const METRIC: &str = "5";

#[derive(Clone, Debug, Default)]
pub struct Physical {
    pub interface: Option<String>,
    pub gateway: Option<String>,
}

fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

fn tool(candidates: &[&str], fallback: &str) -> String {
    candidates
        .iter()
        .find(|path| Path::new(path).exists())
        .map(|path| (*path).to_string())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(target_os = "linux")]
fn ip_tool() -> String {
    tool(&["/usr/sbin/ip", "/sbin/ip", "/usr/bin/ip"], "ip")
}

#[cfg(target_os = "linux")]
fn route_tool() -> String {
    ip_tool()
}

#[cfg(target_os = "macos")]
fn route_tool() -> String {
    tool(&["/sbin/route"], "route")
}

/// Find the interface and gateway the traffic would use without the tunnel.
pub fn physical() -> Physical {
    #[cfg(target_os = "linux")]
    {
        let text = output(&ip_tool(), &["route", "show", "default"]);
        let words: Vec<&str> = text.split_whitespace().collect();
        Physical {
            gateway: words
                .iter()
                .position(|word| *word == "via")
                .and_then(|index| words.get(index + 1))
                .map(|value| value.to_string()),
            interface: words
                .iter()
                .position(|word| *word == "dev")
                .and_then(|index| words.get(index + 1))
                .map(|value| value.to_string()),
        }
    }

    #[cfg(target_os = "macos")]
    {
        let text = output(&route_tool(), &["-n", "get", "default"]);
        let mut physical = Physical::default();
        for line in text.lines() {
            let line = line.trim();
            if let Some(value) = line.strip_prefix("interface:") {
                physical.interface = Some(value.trim().to_string());
            }
            if let Some(value) = line.strip_prefix("gateway:") {
                physical.gateway = Some(value.trim().to_string());
            }
        }
        physical
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Physical::default()
    }
}

/// Bring the interface up and send all traffic through it. The DoH endpoint
/// routes are only needed when the DPI bypass resolver is active.
pub fn enable(tun: &str, dpi: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let ip = ip_tool();
        let _ = run(&ip, &["link", "set", "dev", tun, "up"]);
        // IPv6 is added by hand because the TUN crate only configures IPv4.
        let _ = run(&ip, &["-6", "addr", "replace", "fd00:66::1/64", "dev", tun]);
        let _ = run(&ip, &["route", "replace", "0.0.0.0/1", "dev", tun, "metric", METRIC]);
        let _ = run(&ip, &["route", "replace", "128.0.0.0/1", "dev", tun, "metric", METRIC]);
        let _ = run(&ip, &["-6", "route", "replace", "::/1", "dev", tun, "metric", METRIC]);
        let _ = run(&ip, &["-6", "route", "replace", "8000::/1", "dev", tun, "metric", METRIC]);

        if dpi {
            let physical = physical();
            if let (Some(gateway), Some(interface)) = (physical.gateway, physical.interface) {
                for endpoint in DNS_ENDPOINTS {
                    let route = format!("{endpoint}/32");
                    let _ = run(
                        &ip,
                        &["route", "replace", &route, "via", &gateway, "dev", &interface, "metric", METRIC],
                    );
                }
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let ifconfig = tool(&["/sbin/ifconfig"], "ifconfig");
        let _ = run(&ifconfig, &[tun, "inet6", "fd00:66::1", "prefixlen", "64"]);

        let route = route_tool();
        for network in ["0.0.0.0/1", "128.0.0.0/1"] {
            let _ = run(&route, &["-n", "delete", "-net", network]);
            if !run(&route, &["-n", "add", "-net", network, "-interface", tun]) {
                return Err(format!("could not route {network} through {tun}"));
            }
        }
        for network in ["::/1", "8000::/1"] {
            let _ = run(&route, &["-n", "delete", "-inet6", "-net", network]);
            if !run(&route, &["-n", "add", "-inet6", "-net", network, "-interface", tun]) {
                return Err(format!("could not route {network} through {tun}"));
            }
        }

        if dpi {
            if let Some(gateway) = physical().gateway {
                for endpoint in DNS_ENDPOINTS {
                    if endpoint.parse::<IpAddr>().is_err() {
                        continue;
                    }
                    let _ = run(&route, &["-n", "delete", "-host", endpoint]);
                    let _ = run(&route, &["-n", "add", "-host", endpoint, &gateway]);
                }
            }
        }
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (tun, dpi);
        Err("unsupported platform".into())
    }
}

/// Remove the routes added by [`enable`]. Also used when the helper shuts down
/// unexpectedly, so every call ignores routes that are already gone.
pub fn disable(tun: &str) {
    #[cfg(target_os = "linux")]
    {
        let ip = ip_tool();
        for network in ["0.0.0.0/1", "128.0.0.0/1"] {
            let _ = run(&ip, &["route", "del", network, "dev", tun]);
        }
        for network in ["::/1", "8000::/1"] {
            let _ = run(&ip, &["-6", "route", "del", network, "dev", tun]);
        }
        for endpoint in DNS_ENDPOINTS {
            let route = format!("{endpoint}/32");
            let _ = run(&ip, &["route", "del", &route]);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let route = route_tool();
        for network in ["0.0.0.0/1", "128.0.0.0/1"] {
            let _ = run(&route, &["-n", "delete", "-net", network]);
        }
        for network in ["::/1", "8000::/1"] {
            let _ = run(&route, &["-n", "delete", "-inet6", "-net", network]);
        }
        for endpoint in DNS_ENDPOINTS {
            let _ = run(&route, &["-n", "delete", "-host", endpoint]);
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = tun;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_endpoints_are_addresses() {
        for endpoint in DNS_ENDPOINTS {
            assert!(endpoint.parse::<IpAddr>().is_ok(), "{endpoint}");
        }
    }
}
