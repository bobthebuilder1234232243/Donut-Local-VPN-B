use std::net::{IpAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use crate::controller::Controller;
use crate::log;

const TICK: Duration = Duration::from_secs(5);
const HEALTH: Duration = Duration::from_secs(45);
const SLEEP_GAP: Duration = Duration::from_secs(20);
const SETTLE: Duration = Duration::from_secs(4);

pub fn local_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

pub fn online() -> bool {
    local_address().is_some()
}

pub fn watch(controller: Arc<Controller>) {
    tauri::async_runtime::spawn(async move {
        let mut address = local_address();
        let mut last_wall = SystemTime::now();
        let mut last_tick = Instant::now();
        let mut last_health = Instant::now();
        loop {
            tokio::time::sleep(TICK).await;
            let wall = SystemTime::now();
            let wall_gap = wall.duration_since(last_wall).unwrap_or_default();
            let woke = wall_gap > last_tick.elapsed() + SLEEP_GAP;
            last_wall = wall;
            last_tick = Instant::now();

            let current = local_address();
            let changed = current != address;
            address = current;

            if woke {
                log::write("system woke from sleep");
            }
            if changed {
                match current {
                    Some(_) => log::write("network changed"),
                    None => log::warn("network lost"),
                }
            }
            if (woke || changed) && current.is_some() {
                tokio::time::sleep(SETTLE).await;
                controller.network_changed().await;
                last_health = Instant::now();
            } else if last_health.elapsed() >= HEALTH {
                controller.health_check().await;
                last_health = Instant::now();
            }
        }
    });
}
