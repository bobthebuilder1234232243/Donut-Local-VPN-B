//! Userspace TCP/IP stack that turns the TUN interface into a transparent
//! tunnel towards the local proxy engine.
//!
//! Every TCP connection that arrives on the TUN interface is terminated here
//! with `smoltcp` and relayed to the loopback proxy with a `CONNECT` request.
//! The proxy keeps doing the real work (TLS ClientHello splitting, DNS over
//! HTTPS and profile selection); this module only moves bytes.
//!
//! DNS is special-cased: a query sent to any resolver through the TUN is
//! answered from the same DNS over HTTPS resolver the proxy uses, so name
//! lookups cannot leak to the provider even when the DNS setting is unchanged.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{
    HardwareAddress, IpAddress, IpCidr, IpListenEndpoint, Ipv4Address, Ipv6Address,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, watch};

use super::dns;
use crate::engine::proxy::Resolver;

pub const MTU: usize = 1500;
pub const TUNNEL_ADDRESS_V6: IpAddr = IpAddr::V6(Ipv6Addr::new(0xfd00, 0x66, 0, 0, 0, 0, 0, 1));
const STREAM_BUFFER: usize = 128 * 1024;
const PENDING_LIMIT: usize = 256 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const TICK: Duration = Duration::from_millis(2);

/// Where DNS queries that arrive on the tunnel are answered from.
#[derive(Clone)]
pub enum DnsMode {
    /// Answer locally from the DNS over HTTPS resolver.
    Doh(Arc<Resolver>),
    /// Forward the query unchanged to the system resolver through the
    /// physical interface, used when the DPI bypass is off.
    Forward(Option<String>),
}

/// Traffic counters shared by the stack and the privileged helper.
#[derive(Default)]
pub struct Counters {
    up: AtomicU64,
    down: AtomicU64,
}

impl Counters {
    pub fn up(&self) -> u64 {
        self.up.load(Ordering::Relaxed)
    }

    pub fn down(&self) -> u64 {
        self.down.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> (u64, u64) {
        (self.up(), self.down())
    }

    fn add_up(&self, bytes: usize) {
        self.up.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    fn add_down(&self, bytes: usize) {
        self.down.fetch_add(bytes as u64, Ordering::Relaxed);
    }
}

/// Create the TUN interface used for the tunnel.
///
/// Linux uses a plain `donut0` interface with a /24 address. macOS uses an
/// auto-numbered `utunN` point-to-point interface, so the peer address is set
/// to the address the kernel sources packets from.
pub fn create(
    tunnel: Ipv4Addr,
    prefix: u8,
    device: Ipv4Addr,
) -> io::Result<(tun::AsyncDevice, String)> {
    let [a, b, c, d] = tunnel.octets();
    let [e, f, g, h] = device.octets();
    let [i, j, k, l] = prefix_mask(prefix);
    let mut config = tun::Configuration::default();
    config
        .address((a, b, c, d))
        .destination((e, f, g, h))
        .netmask((i, j, k, l))
        .mtu(MTU as u16)
        .up();

    #[cfg(target_os = "linux")]
    config.tun_name("donut0").platform_config(|platform| {
        platform.ensure_root_privileges(true);
    });

    #[cfg(target_os = "macos")]
    config.tun_name("utun");

    let device = tun::create_as_async(&config).map_err(io::Error::other)?;
    let name = tun::AbstractDevice::tun_name(&*device)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok((device, name))
}

fn prefix_mask(prefix: u8) -> [u8; 4] {
    let bits = if prefix >= 32 { u32::MAX } else { u32::MAX << (32 - prefix) };
    bits.to_be_bytes()
}

/// Run the stack until `stop` is signalled or the TUN device goes away.
pub async fn run(
    device: tun::AsyncDevice,
    proxy_port: u16,
    tunnel: Ipv4Addr,
    prefix: u8,
    dns: DnsMode,
    mut stop: watch::Receiver<bool>,
    counters: Arc<Counters>,
) {
    let (mut writer, mut reader) = match device.split() {
        Ok(parts) => parts,
        Err(_) => return,
    };

    let (inbound, mut packets) = mpsc::channel::<Vec<u8>>(2048);
    let reader_task = tokio::spawn(async move {
        let mut buffer = vec![0u8; 65_536];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if inbound.send(buffer[..n].to_vec()).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let (outbound, mut replies) = mpsc::channel::<Vec<u8>>(2048);
    let writer_task = tokio::spawn(async move {
        while let Some(packet) = replies.recv().await {
            if writer.write_all(&packet).await.is_err() {
                break;
            }
        }
    });

    let mut net = Net::new(proxy_port, tunnel, prefix, dns, outbound, counters);
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = stop.changed() => break,
            packet = packets.recv() => {
                match packet {
                    Some(packet) => {
                        net.ingress(&packet);
                        // Drain what already arrived so a burst is handled in one pass.
                        for _ in 0..255 {
                            match packets.try_recv() {
                                Ok(packet) => net.ingress(&packet),
                                Err(_) => break,
                            }
                        }
                    }
                    None => break,
                }
            }
            _ = tick.tick() => {}
        }
        net.service();
    }

    // Closing both halves of the TUN device is part of a clean teardown: on
    // Linux the kernel removes the interface when the last descriptor closes.
    // The reader is blocked in `read`, so it has to be cancelled, not awaited.
    drop(net);
    drop(packets);
    reader_task.abort();
    writer_task.abort();
    let _ = reader_task.await;
    let _ = writer_task.await;
}

struct Listener {
    port: u16,
    handle: SocketHandle,
}

struct Flow {
    handle: SocketHandle,
    destination: SocketAddr,
    connecting: Option<oneshot::Receiver<io::Result<TcpStream>>>,
    upstream: Option<TcpStream>,
    to_upstream: Vec<u8>,
    to_client: Vec<u8>,
    client_closed: bool,
    server_closed: bool,
    failed: bool,
}

impl Flow {
    fn finished(&self) -> bool {
        self.failed
            || (self.client_closed && self.server_closed && self.to_client.is_empty())
    }
}

struct Net {
    interface: Interface,
    device: TunDevice,
    sockets: SocketSet<'static>,
    listeners: Vec<Listener>,
    flows: Vec<Flow>,
    /// Client 4-tuple -> original destination. The destination is not part of
    /// the listening socket, so it is remembered from the SYN packet.
    destinations: HashMap<(IpAddr, u16), IpAddr>,
    syns: HashMap<u16, usize>,
    proxy_port: u16,
    dns: DnsMode,
    counters: Arc<Counters>,
    replies: mpsc::Sender<Vec<u8>>,
}

impl Net {
    fn new(
        proxy_port: u16,
        tunnel: Ipv4Addr,
        prefix: u8,
        dns: DnsMode,
        replies: mpsc::Sender<Vec<u8>>,
        counters: Arc<Counters>,
    ) -> Self {
        let mut device = TunDevice::new(MTU);
        let config = Config::new(HardwareAddress::Ip);
        let mut interface = Interface::new(config, &mut device, Instant::now());
        let [a, b, c, d] = tunnel.octets();
        interface.update_ip_addrs(|addresses| {
            let _ = addresses.push(IpCidr::new(IpAddress::v4(a, b, c, d), prefix));
            let _ = addresses.push(IpCidr::new(IpAddress::from(TUNNEL_ADDRESS_V6), 64));
        });
        // With AnyIP and a route whose gateway is one of our own addresses the
        // interface accepts packets for any destination. Replies still carry
        // the real destination address because the medium is raw IP.
        let _ = interface
            .routes_mut()
            .add_default_ipv4_route(Ipv4Address::from(tunnel));
        if let IpAddr::V6(address) = TUNNEL_ADDRESS_V6 {
            let _ = interface.routes_mut().add_default_ipv6_route(Ipv6Address::from(address));
        }
        interface.set_any_ip(true);

        Self {
            interface,
            device,
            sockets: SocketSet::new(Vec::new()),
            listeners: Vec::new(),
            flows: Vec::new(),
            destinations: HashMap::new(),
            syns: HashMap::new(),
            proxy_port,
            dns,
            counters,
            replies,
        }
    }

    /// Inspect a packet read from the TUN device.
    fn ingress(&mut self, packet: &[u8]) {
        let Some(meta) = inspect(packet) else {
            return;
        };
        match meta.protocol {
            PROTOCOL_TCP => {
                if meta.syn && !meta.ack {
                    if self.destinations.len() > 4096 {
                        self.destinations.clear();
                    }
                    self.destinations.insert((meta.src, meta.src_port), meta.dst);
                    *self.syns.entry(meta.dst_port).or_default() += 1;
                    self.ensure_listeners(meta.dst_port);
                }
                self.device.push(packet.to_vec());
            }
            PROTOCOL_UDP if meta.dst_port == 53 => self.resolve_dns(&meta, packet),
            _ => {}
        }
    }

    fn resolve_dns(&self, meta: &Meta, packet: &[u8]) {
        let Some(query) = udp_payload(packet) else {
            return;
        };
        let query = query.to_vec();
        let replies = self.replies.clone();
        let mode = self.dns.clone();
        let meta = *meta;
        tokio::spawn(async move {
            let answer = match mode {
                DnsMode::Doh(resolver) => dns::respond(&resolver, &query).await,
                DnsMode::Forward(interface) => {
                    dns::forward(interface, SocketAddr::new(meta.dst, 53), &query).await
                }
            };
            let Some(answer) = answer else {
                return;
            };
            if let Some(packet) = dns::datagram(meta.dst, meta.src, 53, meta.src_port, &answer) {
                let _ = replies.send(packet).await;
            }
        });
    }

    fn ensure_listeners(&mut self, port: u16) {
        let wanted = self.syns.get(&port).copied().unwrap_or(0);
        let idle = self
            .listeners
            .iter()
            .filter(|listener| {
                listener.port == port
                    && self.sockets.get::<tcp::Socket>(listener.handle).state() == tcp::State::Listen
            })
            .count();
        for _ in idle..wanted {
            self.add_listener(port);
        }
    }

    fn add_listener(&mut self, port: u16) {
        let mut socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0; STREAM_BUFFER]),
            tcp::SocketBuffer::new(vec![0; STREAM_BUFFER]),
        );
        if socket
            .listen(IpListenEndpoint { addr: None, port })
            .is_err()
        {
            return;
        }
        let handle = self.sockets.add(socket);
        self.listeners.push(Listener { port, handle });
    }

    fn service(&mut self) {
        self.interface
            .poll(Instant::now(), &mut self.device, &mut self.sockets);
        self.accept();
        for flow in &mut self.flows {
            service_flow(flow, &mut self.sockets, &self.counters);
        }
        self.reap();
        self.flush();
        self.syns.clear();
    }

    fn accept(&mut self) {
        let mut index = 0;
        while index < self.listeners.len() {
            let listener = &self.listeners[index];
            let port = listener.port;
            let handle = listener.handle;
            let mut replace = false;
            let mut abort = false;

            {
                let socket = self.sockets.get::<tcp::Socket>(handle);
                match socket.state() {
                    tcp::State::Established => {
                        if let Some(remote) = socket.remote_endpoint() {
                            let client = (std_to_ip(remote.addr), remote.port);
                            if let Some(destination) = self.destinations.remove(&client) {
                                let destination = SocketAddr::new(destination, port);
                                let (tx, rx) = oneshot::channel();
                                let proxy_port = self.proxy_port;
                                tokio::spawn(async move {
                                    let _ = tx.send(open_upstream(proxy_port, destination).await);
                                });
                                self.flows.push(Flow {
                                    handle,
                                    destination,
                                    connecting: Some(rx),
                                    upstream: None,
                                    to_upstream: Vec::new(),
                                    to_client: Vec::new(),
                                    client_closed: false,
                                    server_closed: false,
                                    failed: false,
                                });
                                replace = true;
                            } else {
                                abort = true;
                            }
                        }
                    }
                    tcp::State::Closed | tcp::State::TimeWait => replace = true,
                    _ => {}
                }
            }

            if abort {
                self.sockets.get_mut::<tcp::Socket>(handle).abort();
                replace = true;
            }

            if replace {
                self.listeners.swap_remove(index);
                self.add_listener(port);
            } else {
                index += 1;
            }
        }
    }

    fn reap(&mut self) {
        let mut index = 0;
        while index < self.flows.len() {
            if self.flows[index].finished() {
                if let Some(flow) = self.flows.get_mut(index) {
                    if let Some(upstream) = &flow.upstream {
                        close(upstream, libc::SHUT_RDWR);
                    }
                    if self.sockets.get::<tcp::Socket>(flow.handle).state() != tcp::State::Closed {
                        self.sockets.get_mut::<tcp::Socket>(flow.handle).abort();
                    }
                }
                let handle = self.flows[index].handle;
                if self.sockets.iter().any(|(candidate, _)| candidate == handle) {
                    self.sockets.remove(handle);
                }
                self.flows.swap_remove(index);
            } else {
                index += 1;
            }
        }
    }

    fn flush(&mut self) {
        while let Some(packet) = self.device.pop() {
            if self.replies.try_send(packet).is_err() {
                // A full queue means the kernel is not keeping up; TCP will retransmit.
                break;
            }
        }
    }
}

fn service_flow(flow: &mut Flow, sockets: &mut SocketSet<'static>, counters: &Counters) {
    if flow.upstream.is_none() {
        if let Some(receiver) = &mut flow.connecting {
            match receiver.try_recv() {
                Ok(Ok(stream)) => {
                    flow.upstream = Some(stream);
                    flow.connecting = None;
                }
                Ok(Err(_)) => flow.failed = true,
                Err(oneshot::error::TryRecvError::Empty) => {}
                Err(oneshot::error::TryRecvError::Closed) => flow.failed = true,
            }
        }
    }

    {
        let socket = sockets.get_mut::<tcp::Socket>(flow.handle);

        while socket.may_recv() && flow.to_upstream.len() < PENDING_LIMIT {
            let mut buffer = [0u8; 16 * 1024];
            match socket.recv_slice(&mut buffer) {
                Ok(0) => break,
                Ok(n) => flow.to_upstream.extend_from_slice(&buffer[..n]),
                Err(_) => break,
            }
        }

        if !socket.may_recv() && flow.to_upstream.is_empty() {
            flow.client_closed = true;
        }
    }

    if let Some(upstream) = &mut flow.upstream {
        while !flow.to_upstream.is_empty() {
            match upstream.try_write(&flow.to_upstream) {
                Ok(0) => break,
                Ok(n) => {
                    flow.to_upstream.drain(..n);
                    counters.add_up(n);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    flow.failed = true;
                    return;
                }
            }
        }

        let mut buffer = [0u8; 16 * 1024];
        loop {
            match upstream.try_read(&mut buffer) {
                Ok(0) => {
                    flow.server_closed = true;
                    break;
                }
                Ok(n) => {
                    flow.to_client.extend_from_slice(&buffer[..n]);
                    counters.add_down(n);
                    if flow.to_client.len() >= PENDING_LIMIT {
                        break;
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    flow.server_closed = true;
                    break;
                }
            }
        }
    }

    {
        let socket = sockets.get_mut::<tcp::Socket>(flow.handle);
        while !flow.to_client.is_empty() {
            match socket.send_slice(&flow.to_client) {
                Ok(0) => break,
                Ok(n) => {
                    flow.to_client.drain(..n);
                }
                Err(_) => {
                    flow.failed = true;
                    return;
                }
            }
        }
    }

    if flow.client_closed && flow.to_upstream.is_empty() {
        if let Some(upstream) = &flow.upstream {
            close(upstream, libc::SHUT_WR);
        }
    }

    if flow.server_closed && flow.to_client.is_empty() {
        let socket = sockets.get_mut::<tcp::Socket>(flow.handle);
        if socket.may_send() {
            socket.close();
        }
    }

    if flow.upstream.is_some() && flow.connecting.is_none() {
        if let Some(stream) = &flow.upstream {
            if stream.peer_addr().is_ok() && flow.client_closed && flow.server_closed {
                flow.failed = true;
            }
        }
    }
}

/// Open a connection to the loopback proxy and ask it for the destination.
async fn open_upstream(proxy_port: u16, destination: SocketAddr) -> io::Result<TcpStream> {
    let mut stream = tokio::time::timeout(
        CONNECT_TIMEOUT,
        TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], proxy_port))),
    )
    .await
    .map_err(|_| io::ErrorKind::TimedOut)??;
    stream.set_nodelay(true)?;

    let authority = match destination {
        SocketAddr::V4(_) => format!("{}:{}", destination.ip(), destination.port()),
        SocketAddr::V6(address) => format!("[{}]:{}", address.ip(), address.port()),
    };
    let request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await?;

    let mut head = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    while head.len() < 8192 {
        let read = tokio::time::timeout(CONNECT_TIMEOUT, stream.read(&mut byte)).await;
        match read {
            Ok(Ok(1)) => head.push(byte[0]),
            Ok(_) => break,
            Err(_) => return Err(io::ErrorKind::TimedOut.into()),
        }
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let status = String::from_utf8_lossy(&head);
    if !status.starts_with("HTTP/1.1 200") && !status.starts_with("HTTP/1.0 200") {
        return Err(io::Error::other("proxy refused the connection"));
    }
    Ok(stream)
}

const PROTOCOL_TCP: u8 = 6;
const PROTOCOL_UDP: u8 = 17;

#[derive(Clone, Copy, Debug)]
struct Meta {
    src: IpAddr,
    dst: IpAddr,
    protocol: u8,
    src_port: u16,
    dst_port: u16,
    syn: bool,
    ack: bool,
}

/// Read the parts of an IPv4 or IPv6 packet needed by the tunnel.
fn inspect(packet: &[u8]) -> Option<Meta> {
    match packet.first()? >> 4 {
        4 => {
            let header = (*packet.get(0)? & 0x0f) as usize * 4;
            if header < 20 || packet.len() < header + 20 {
                return None;
            }
            let protocol = *packet.get(9)?;
            if protocol != PROTOCOL_TCP && protocol != PROTOCOL_UDP {
                return None;
            }
            let src = IpAddr::V4(Ipv4Addr::new(
                *packet.get(12)?,
                *packet.get(13)?,
                *packet.get(14)?,
                *packet.get(15)?,
            ));
            let dst = IpAddr::V4(Ipv4Addr::new(
                *packet.get(16)?,
                *packet.get(17)?,
                *packet.get(18)?,
                *packet.get(19)?,
            ));
            let transport = packet.get(header..)?;
            meta(src, dst, protocol, transport)
        }
        6 => {
            if packet.len() < 40 + 20 {
                return None;
            }
            let protocol = *packet.get(6)?;
            if protocol != PROTOCOL_TCP && protocol != PROTOCOL_UDP {
                return None;
            }
            let mut src = [0u8; 16];
            src.copy_from_slice(packet.get(8..24)?);
            let mut dst = [0u8; 16];
            dst.copy_from_slice(packet.get(24..40)?);
            meta(
                IpAddr::V6(Ipv6Addr::from(src)),
                IpAddr::V6(Ipv6Addr::from(dst)),
                protocol,
                packet.get(40..)?,
            )
        }
        _ => None,
    }
}

fn meta(src: IpAddr, dst: IpAddr, protocol: u8, transport: &[u8]) -> Option<Meta> {
    let src_port = u16::from_be_bytes([*transport.first()?, *transport.get(1)?]);
    let dst_port = u16::from_be_bytes([*transport.get(2)?, *transport.get(3)?]);
    let (syn, ack) = if protocol == PROTOCOL_TCP {
        let flags = *transport.get(13)?;
        (flags & 0x02 != 0, flags & 0x10 != 0)
    } else {
        (false, false)
    };
    Some(Meta { src, dst, protocol, src_port, dst_port, syn, ack })
}

fn udp_payload(packet: &[u8]) -> Option<&[u8]> {
    let (offset, length) = match packet.first()? >> 4 {
        4 => {
            let header = (*packet.first()? & 0x0f) as usize * 4;
            let total = u16::from_be_bytes([*packet.get(2)?, *packet.get(3)?]) as usize;
            (header, total.saturating_sub(header + 8))
        }
        6 => {
            let length = u16::from_be_bytes([*packet.get(4)?, *packet.get(5)?]) as usize;
            (40, length.saturating_sub(8))
        }
        _ => return None,
    };
    let payload = packet.get(offset + 8..offset + 8 + length)?;
    Some(payload)
}

fn std_to_ip(address: IpAddress) -> IpAddr {
    match address {
        IpAddress::Ipv4(address) => IpAddr::V4(Ipv4Addr::from(address.octets())),
        IpAddress::Ipv6(address) => IpAddr::V6(Ipv6Addr::from(address.octets())),
    }
}

/// Half-close or reset the loopback connection without an async round trip.
fn close(stream: &TcpStream, how: i32) {
    // SAFETY: `stream` owns a live descriptor for the duration of the call.
    unsafe {
        libc::shutdown(stream.as_raw_fd(), how);
    }
}

/// `smoltcp` device backed by two packet queues.
struct TunDevice {
    inbound: VecDeque<Vec<u8>>,
    outbound: VecDeque<Vec<u8>>,
    mtu: usize,
}

impl TunDevice {
    fn new(mtu: usize) -> Self {
        Self {
            inbound: VecDeque::new(),
            outbound: VecDeque::new(),
            mtu,
        }
    }

    fn push(&mut self, packet: Vec<u8>) {
        if self.inbound.len() < 1024 {
            self.inbound.push_back(packet);
        }
    }

    fn pop(&mut self) -> Option<Vec<u8>> {
        self.outbound.pop_front()
    }
}

struct RxBuffer(Vec<u8>);

struct TxBuffer<'a> {
    queue: &'a mut VecDeque<Vec<u8>>,
    mtu: usize,
}

impl RxToken for RxBuffer {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.0)
    }
}

impl TxToken for TxBuffer<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0u8; len.min(self.mtu) ];
        let result = f(&mut buffer);
        self.queue.push_back(buffer);
        result
    }
}

impl Device for TunDevice {
    type RxToken<'a> = RxBuffer;
    type TxToken<'a> = TxBuffer<'a>;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let packet = self.inbound.pop_front()?;
        Some((
            RxBuffer(packet),
            TxBuffer { queue: &mut self.outbound, mtu: self.mtu },
        ))
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxBuffer { queue: &mut self.outbound, mtu: self.mtu })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ip;
        capabilities.max_transmission_unit = self.mtu;
        capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipv4_tcp(dst: [u8; 4], port: u16, flags: u8) -> Vec<u8> {
        let mut packet = vec![0u8; 40];
        packet[0] = 0x45;
        packet[2..4].copy_from_slice(&(40u16).to_be_bytes());
        packet[8] = 64;
        packet[9] = PROTOCOL_TCP;
        packet[12..16].copy_from_slice(&[10, 66, 0, 2]);
        packet[16..20].copy_from_slice(&dst);
        packet[20..22].copy_from_slice(&40000u16.to_be_bytes());
        packet[22..24].copy_from_slice(&port.to_be_bytes());
        packet[20 + 12] = 0x50;
        packet[20 + 13] = flags;
        packet
    }

    #[test]
    fn reads_tcp_destination() {
        let meta = inspect(&ipv4_tcp([1, 2, 3, 4], 443, 0x02)).unwrap();
        assert_eq!(meta.dst, "1.2.3.4".parse::<IpAddr>().unwrap());
        assert_eq!(meta.dst_port, 443);
        assert!(meta.syn && !meta.ack);
    }

    #[test]
    fn ignores_other_protocols() {
        let mut packet = ipv4_tcp([1, 2, 3, 4], 443, 0x02);
        packet[9] = 1;
        assert!(inspect(&packet).is_none());
        assert!(inspect(&[0x45]).is_none());
    }

    #[test]
    fn parses_ipv6_tcp() {
        let mut packet = vec![0u8; 60];
        packet[0] = 0x60;
        packet[4..6].copy_from_slice(&(20u16).to_be_bytes());
        packet[6] = PROTOCOL_TCP;
        packet[7] = 64;
        packet[8..24].copy_from_slice(&TUNNEL_ADDRESS_V6.to_string().parse::<Ipv6Addr>().unwrap().octets());
        packet[24..40].copy_from_slice(&"2606:4700::1111".parse::<Ipv6Addr>().unwrap().octets());
        packet[40..42].copy_from_slice(&50000u16.to_be_bytes());
        packet[42..44].copy_from_slice(&443u16.to_be_bytes());
        packet[52] = 0x50;
        packet[53] = 0x02;
        let meta = inspect(&packet).unwrap();
        assert_eq!(meta.dst, "2606:4700::1111".parse::<IpAddr>().unwrap());
        assert!(meta.syn);
    }
}
