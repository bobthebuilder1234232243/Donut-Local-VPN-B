//! Real TUN integration test.
//!
//! Unlike the unit tests, this one creates an actual TUN interface, installs
//! the split default routes, sends real traffic through the userspace stack and
//! checks that it leaves through the local proxy without looping back into the
//! tunnel. It needs root (CAP_NET_ADMIN) and /dev/net/tun, so it only runs in
//! the privileged container from `.github/workflows/test-tunnel.yml`.
//!
//! Everything outside the tunnel is Quad9: TCP 9.9.9.9:443 for the TLS relay,
//! UDP 9.9.9.9:53 for DNS. Both are outside the networks the tunnel routes, so
//! reaching them proves the physical path was used. Set `DONUT_TEST_DPI=0` to
//! run the same checks with the DPI bypass off.

use std::net::IpAddr;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::watch;

use tunnel_harness::engine::proxy::{Method, Resolver, Server};
use tunnel_harness::engine::tunnel::routes;
use tunnel_harness::engine::tunnel::stack::{self, Counters, DnsMode};

const TUNNEL: [u8; 4] = [10, 66, 0, 1];
const DEVICE: [u8; 4] = [10, 66, 0, 2];
const TCP_TARGET: &str = "9.9.9.9:443";
const DNS_TARGET: &str = "9.9.9.9:53";

fn output(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("could not run {program}: {error}"));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn link_exists(name: &str) -> bool {
    Command::new("ip")
        .args(["link", "show", name])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn query(name: &str) -> Vec<u8> {
    let mut buffer = vec![0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        buffer.push(label.len() as u8);
        buffer.extend_from_slice(label.as_bytes());
    }
    buffer.push(0);
    buffer.extend_from_slice(&1u16.to_be_bytes());
    buffer.extend_from_slice(&1u16.to_be_bytes());
    buffer
}

/// The same ClientHello shape the proxy unit tests use, built here so the test
/// does not depend on a `#[cfg(test)]` helper of the library.
fn client_hello(host: &str) -> Vec<u8> {
    let name = host.as_bytes();
    let mut sni = Vec::new();
    sni.extend_from_slice(&((name.len() + 3) as u16).to_be_bytes());
    sni.push(0);
    sni.extend_from_slice(&(name.len() as u16).to_be_bytes());
    sni.extend_from_slice(name);

    let mut extensions = Vec::new();
    extensions.extend_from_slice(&[0x00, 0x0b, 0x00, 0x02, 0x01, 0x00]);
    extensions.extend_from_slice(&[0x00, 0x00]);
    extensions.extend_from_slice(&(sni.len() as u16).to_be_bytes());
    extensions.extend_from_slice(&sni);

    let mut body = vec![0x03, 0x03];
    body.extend_from_slice(&[7u8; 32]);
    body.push(0);
    body.extend_from_slice(&[0x00, 0x02, 0x13, 0x01]);
    body.extend_from_slice(&[0x01, 0x00]);
    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);

    let mut handshake = vec![0x01];
    handshake.extend_from_slice(&(body.len() as u32).to_be_bytes()[1..]);
    handshake.extend_from_slice(&body);

    let mut record = vec![0x16, 0x03, 0x01];
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tunnel_carries_real_traffic_and_tears_down_cleanly() {
    assert_eq!(
        // SAFETY: geteuid takes no arguments and has no side effects.
        unsafe { libc::geteuid() },
        0,
        "the TUN integration test must run as root (CAP_NET_ADMIN)"
    );
    assert!(
        std::path::Path::new("/dev/net/tun").exists(),
        "/dev/net/tun is missing; the container needs --device=/dev/net/tun"
    );

    let dpi = std::env::var("DONUT_TEST_DPI")
        .map(|value| value != "0")
        .unwrap_or(true);
    let physical = routes::physical();
    assert!(
        physical.interface.is_some(),
        "no default route, cannot tell the physical interface from the tunnel"
    );
    let interface = physical.interface.clone();

    let resolver = if dpi {
        Arc::new(Resolver::new(interface.clone()))
    } else {
        Arc::new(Resolver::system())
    };
    let method = if dpi {
        Method::parse("split=1").expect("split=1 is a valid method")
    } else {
        Method::direct()
    };
    assert_eq!(method.evades(), dpi);

    let server = Server::start(method, resolver.clone(), None, interface.clone())
        .await
        .expect("proxy server");
    let counters = Arc::new(Counters::default());

    let (device, name) = stack::create(
        TUNNEL.into(),
        24,
        DEVICE.into(),
    )
    .expect("create TUN interface");
    println!("tunnel interface: {name} on {:?}, dpi={dpi}", interface);

    let (stop, stopped) = watch::channel(false);
    let dns = if dpi {
        DnsMode::Doh(resolver.clone())
    } else {
        DnsMode::Forward(interface.clone())
    };
    let stack_task = tokio::spawn(stack::run(
        device,
        server.port(),
        TUNNEL.into(),
        24,
        dns,
        stopped,
        counters.clone(),
    ));

    routes::enable(&name).expect("enable routes");

    let links = output("ip", &["-4", "addr", "show", "dev", &name]);
    assert!(
        links.contains("10.66.0.1/24"),
        "the TUN interface has no address:\n{links}"
    );

    let routes_text = output("ip", &["route", "show"]);
    for network in ["0.0.0.0/1", "128.0.0.0/1"] {
        assert!(
            routes_text
                .lines()
                .any(|line| line.contains(network) && line.contains(&name)),
            "route {network} does not point at {name}:\n{routes_text}"
        );
    }

    // DNS through the tunnel. With the bypass on this is answered by the
    // resolver over DoH; with it off the query is forwarded to Quad9 through
    // the physical interface. Either way the answer must be a real A record.
    let socket = UdpSocket::bind("0.0.0.0:0").await.expect("bind DNS socket");
    socket
        .send_to(&query("example.com"), DNS_TARGET)
        .await
        .expect("send DNS query");
    let mut answer = [0u8; 512];
    let (read, _) = tokio::time::timeout(Duration::from_secs(20), socket.recv_from(&mut answer))
        .await
        .expect("DNS query timed out; DNS did not survive the tunnel")
        .expect("DNS reply");
    assert!(read >= 12, "short DNS reply: {read} bytes");
    assert_eq!(&answer[0..2], &[0x12, 0x34], "DNS id was not echoed");
    assert_eq!(answer[3] & 0x0f, 0, "DNS server returned an error");
    let answers = u16::from_be_bytes([answer[6], answer[7]]);
    assert!(answers > 0, "DNS reply has no answer records");

    // TCP through the tunnel: a real TLS handshake to Quad9. If the relay
    // looped back into the tunnel this would hang until the timeout.
    let mut tcp = tokio::time::timeout(Duration::from_secs(20), TcpStream::connect(TCP_TARGET))
        .await
        .expect("TCP connect timed out; the relay is probably looping")
        .expect("connect through the tunnel");
    tcp.set_nodelay(true).expect("nodelay");
    tcp.write_all(&client_hello("cloudflare.com"))
        .await
        .expect("write ClientHello");
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(20), tcp.read(&mut byte))
        .await
        .expect("TLS response timed out")
        .expect("read TLS response");
    assert!(read > 0, "the server closed the TLS connection");

    let deadline = Instant::now() + Duration::from_secs(10);
    let (up, down) = loop {
        let (up, down) = counters.snapshot();
        if up > 0 && down > 0 {
            break (up, down);
        }
        assert!(
            Instant::now() < deadline,
            "traffic counters did not move: up={up} down={down}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    println!("counters: up={up} down={down}");

    // Teardown. Dropping the client and stopping the stack must remove the
    // routes and close the TUN device, restoring normal connectivity.
    drop(tcp);
    let _ = stop.send(true);
    tokio::time::timeout(Duration::from_secs(5), stack_task)
        .await
        .expect("the stack did not stop")
        .expect("stack task panicked");
    routes::disable(&name);

    let routes_text = output("ip", &["route", "show"]);
    for network in ["0.0.0.0/1", "128.0.0.0/1"] {
        assert!(
            !routes_text
                .lines()
                .any(|line| line.contains(network) && line.contains(&name)),
            "route {network} was left behind:\n{routes_text}"
        );
    }

    let mut gone = false;
    for _ in 0..50 {
        if !link_exists(&name) {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(gone, "TUN interface {name} was left behind");

    let restored = tokio::time::timeout(Duration::from_secs(20), TcpStream::connect("1.1.1.1:443"))
        .await
        .expect("the network did not recover after teardown");
    assert!(restored.is_ok(), "the network did not recover after teardown");

    let _: IpAddr = "1.1.1.1".parse().unwrap();
}
