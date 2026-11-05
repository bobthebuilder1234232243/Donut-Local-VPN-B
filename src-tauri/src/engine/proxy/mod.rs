mod desync;
mod dns;
pub mod tls;

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;

pub use desync::Method;
pub use dns::Resolver;

pub const PREFERRED_PORT: u16 = 18080;
const HEAD_LIMIT: usize = 16 * 1024;
const HELLO_LIMIT: usize = 64 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(7);

pub struct Server {
    port: u16,
    task: JoinHandle<()>,
}

impl Server {
    pub async fn start(
        method: Method,
        resolver: Arc<Resolver>,
        only: Option<Arc<Vec<String>>>,
        interface: Option<String>,
    ) -> io::Result<Self> {
        let listener = match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], PREFERRED_PORT))).await {
            Ok(listener) => listener,
            Err(_) => TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?,
        };
        let port = listener.local_addr()?.port();
        let method = Arc::new(method);
        let task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((client, _)) => {
                        let method = method.clone();
                        let resolver = resolver.clone();
                        let only = only.clone();
                        let interface = interface.clone();
                        tokio::spawn(async move {
                            let _ = serve(
                                client,
                                &method,
                                &resolver,
                                only.as_deref().map(Vec::as_slice),
                                interface.as_deref(),
                            )
                            .await;
                        });
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
                }
            }
        });
        Ok(Self { port, task })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

enum Request {
    Connect { host: String, port: u16 },
    Forward { host: String, port: u16, head: Vec<u8> },
}

async fn serve(
    mut client: TcpStream,
    method: &Method,
    resolver: &Resolver,
    only: Option<&[String]>,
    interface: Option<&str>,
) -> io::Result<()> {
    client.set_nodelay(true)?;
    let (head, body) = timeout(IO_TIMEOUT, read_head(&mut client))
        .await
        .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))??;
    let request = parse_request(&head).ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;

    match request {
        Request::Connect { host, port } => {
            let mut upstream = match connect(resolver, &host, port, interface).await {
                Ok(stream) => stream,
                Err(e) => {
                    let _ = client.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n").await;
                    return Err(e);
                }
            };
            client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await?;
            let hello = read_client_hello(&mut client, body).await?;
            let bypass = only.is_none_or(|domains| crate::catalog::covers(domains, &host));
            if bypass && !hello.is_empty() {
                method.send(&mut upstream, &hello).await?;
            } else {
                upstream.write_all(&hello).await?;
            }
            tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
        }
        Request::Forward { host, port, head } => {
            let mut upstream = connect(resolver, &host, port, interface).await?;
            upstream.write_all(&head).await?;
            upstream.write_all(&body).await?;
            tokio::io::copy_bidirectional(&mut client, &mut upstream).await?;
        }
    }
    Ok(())
}

async fn read_head(stream: &mut TcpStream) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(end + 4);
            return Ok((buf, rest));
        }
        if buf.len() > HEAD_LIMIT {
            return Err(io::ErrorKind::InvalidData.into());
        }
    }
}

async fn read_client_hello(stream: &mut TcpStream, mut buf: Vec<u8>) -> io::Result<Vec<u8>> {
    let mut chunk = [0u8; 4096];
    loop {
        if !buf.is_empty() {
            if !tls::is_handshake(&buf) {
                return Ok(buf);
            }
            if tls::record_len(&buf).is_some_and(|len| buf.len() >= len) {
                return Ok(buf);
            }
        }
        if buf.len() > HELLO_LIMIT {
            return Ok(buf);
        }
        match timeout(IO_TIMEOUT, stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Err(_) => return Ok(buf),
            Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
            Ok(Err(e)) => return Err(e),
        }
    }
}

fn parse_request(head: &[u8]) -> Option<Request> {
    let text = std::str::from_utf8(head).ok()?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (verb, target, version) = (first.next()?, first.next()?, first.next()?);

    if verb.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = split_host_port(target, 443)?;
        return Some(Request::Connect { host, port });
    }

    let rest = target.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = split_host_port(authority, 80)?;

    let mut out = format!("{verb} {path} {version}\r\n");
    for line in lines.filter(|l| !l.is_empty()) {
        let name = line.split(':').next().unwrap_or("").trim().to_ascii_lowercase();
        if matches!(name.as_str(), "proxy-connection" | "connection" | "keep-alive" | "proxy-authorization") {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str("Connection: close\r\n\r\n");
    Some(Request::Forward { host, port, head: out.into_bytes() })
}

fn split_host_port(value: &str, default: u16) -> Option<(String, u16)> {
    if let Some(rest) = value.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = match tail.strip_prefix(':') {
            Some(p) => p.parse().ok()?,
            None => default,
        };
        return Some((host.to_string(), port));
    }
    match value.rsplit_once(':') {
        Some((host, port)) => Some((host.to_string(), port.parse().ok()?)),
        None if !value.is_empty() => Some((value.to_string(), default)),
        None => None,
    }
}

async fn connect(resolver: &Resolver, host: &str, port: u16, interface: Option<&str>) -> io::Result<TcpStream> {
    let mut last = io::Error::from(io::ErrorKind::NotFound);
    for ip in resolver.lookup(host).await? {
        match timeout(CONNECT_TIMEOUT, dial(ip, port, interface)).await {
            Ok(Ok(stream)) => {
                stream.set_nodelay(true)?;
                return Ok(stream);
            }
            Ok(Err(e)) => last = e,
            Err(_) => last = io::ErrorKind::TimedOut.into(),
        }
    }
    Err(last)
}

/// Connect to a destination, optionally pinning the socket to a physical
/// interface. The tunnel setup needs this: without it the proxy's own
/// connections would be routed back into the TUN and loop forever.
async fn dial(ip: std::net::IpAddr, port: u16, interface: Option<&str>) -> io::Result<TcpStream> {
    #[cfg(unix)]
    if let Some(interface) = interface {
        use socket2::{Domain, Protocol, SockAddr, Socket, Type};

        let domain = if ip.is_ipv4() { Domain::IPV4 } else { Domain::IPV6 };
        let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
        crate::engine::bind_interface(&socket, interface, ip.is_ipv6())?;
        socket.set_nonblocking(true)?;
        match socket.connect(&SockAddr::from(SocketAddr::new(ip, port))) {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error),
        }
        let stream = TcpStream::from_std(std::net::TcpStream::from(socket))?;
        stream.writable().await?;
        if let Some(error) = stream.take_error()? {
            return Err(error);
        }
        return Ok(stream);
    }

    let _ = interface;
    TcpStream::connect((ip, port)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect() {
        match parse_request(b"CONNECT discord.com:443 HTTP/1.1\r\nHost: discord.com:443\r\n\r\n") {
            Some(Request::Connect { host, port }) => {
                assert_eq!(host, "discord.com");
                assert_eq!(port, 443);
            }
            _ => panic!("expected connect"),
        }
    }

    #[test]
    fn rewrites_plain_http() {
        let head = b"GET http://example.com:8080/a?b=1 HTTP/1.1\r\nHost: example.com:8080\r\nProxy-Connection: keep-alive\r\n\r\n";
        match parse_request(head) {
            Some(Request::Forward { host, port, head }) => {
                assert_eq!(host, "example.com");
                assert_eq!(port, 8080);
                let text = String::from_utf8(head).unwrap();
                assert!(text.starts_with("GET /a?b=1 HTTP/1.1\r\n"));
                assert!(text.contains("Host: example.com:8080\r\n"));
                assert!(!text.to_ascii_lowercase().contains("proxy-connection"));
                assert!(text.ends_with("Connection: close\r\n\r\n"));
            }
            _ => panic!("expected forward"),
        }
    }

    #[test]
    fn splits_ipv6_authority() {
        assert_eq!(split_host_port("[::1]:8443", 443), Some(("::1".into(), 8443)));
        assert_eq!(split_host_port("[::1]", 443), Some(("::1".into(), 443)));
        assert_eq!(split_host_port("host", 80), Some(("host".into(), 80)));
        assert_eq!(split_host_port("host:x", 80), None);
    }

    #[tokio::test]
    async fn tunnels_tls_through_desync() {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_port = upstream.local_addr().unwrap().port();
        let hello = tls::client_hello("discord.com");
        let expected = hello.clone();

        let server_side = tokio::spawn(async move {
            let (mut socket, _) = upstream.accept().await.unwrap();
            let mut received = Vec::new();
            let mut chunk = [0u8; 1024];
            while received.len() < expected.len() + 5 {
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                received.extend_from_slice(&chunk[..n]);
            }
            socket.write_all(b"pong").await.unwrap();
            received
        });

        let method = Method::parse("tlsrec=sni+1 split=sni+3").unwrap();
        let server = Server::start(method, Arc::new(Resolver::new(None)), None, None).await.unwrap();
        let mut client = TcpStream::connect(("127.0.0.1", server.port())).await.unwrap();
        client
            .write_all(format!("CONNECT 127.0.0.1:{upstream_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut reply = [0u8; 39];
        client.read_exact(&mut reply).await.unwrap();
        assert!(reply.starts_with(b"HTTP/1.1 200"));

        client.write_all(&hello).await.unwrap();
        let mut pong = [0u8; 4];
        client.read_exact(&mut pong).await.unwrap();
        assert_eq!(&pong, b"pong");

        let received = server_side.await.unwrap();
        assert_eq!(received.len(), hello.len() + 5);
        assert_eq!(tls::record_len(&received), tls::sni_offset(&hello).map(|s| s + 1));
    }

    #[tokio::test]
    async fn direct_mode_passes_the_hello_unchanged() {
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_port = upstream.local_addr().unwrap().port();
        let hello = tls::client_hello("discord.com");
        let expected = hello.clone();

        let server_side = tokio::spawn(async move {
            let (mut socket, _) = upstream.accept().await.unwrap();
            let mut received = vec![0u8; expected.len()];
            socket.read_exact(&mut received).await.unwrap();
            assert_eq!(received, expected);
            let mut extra = [0u8; 1];
            assert!(
                timeout(Duration::from_millis(200), socket.read(&mut extra)).await.is_err(),
                "direct mode wrote more than the original ClientHello"
            );
        });

        let server = Server::start(Method::direct(), Arc::new(Resolver::new(None)), None, None)
            .await
            .unwrap();
        let mut client = TcpStream::connect(("127.0.0.1", server.port())).await.unwrap();
        client
            .write_all(format!("CONNECT 127.0.0.1:{upstream_port} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut reply = [0u8; 39];
        client.read_exact(&mut reply).await.unwrap();
        assert!(reply.starts_with(b"HTTP/1.1 200"));

        client.write_all(&hello).await.unwrap();
        server_side.await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn serve_for_manual_testing() {
        let spec = std::env::var("FREEDPI_METHOD").unwrap_or_else(|_| "split=1".into());
        let server = Server::start(Method::parse(&spec).unwrap(), Arc::new(Resolver::new(None)), None, None)
            .await
            .unwrap();
        eprintln!("listening on {} with {spec}", server.port());
        tokio::time::sleep(Duration::from_secs(90)).await;
    }

    #[tokio::test]
    #[ignore]
    async fn reaches_discord_through_proxy() {
        let resolver = Arc::new(Resolver::new(None));
        let urls = vec!["https://discord.com/api/v9/gateway".to_string()];
        let mut working = Vec::new();
        for profile in crate::catalog::proxy_profiles(crate::catalog::Region::Auto) {
            let server = Server::start(Method::parse(profile).unwrap(), resolver.clone(), None, None)
                .await
                .unwrap();
            let ok = crate::probe::reachable(&urls, Some(server.port())).await;
            eprintln!("{profile:<28} {}", if ok { "ok" } else { "blocked" });
            if ok {
                working.push(profile);
            }
        }
        assert!(!working.is_empty());
    }
}
