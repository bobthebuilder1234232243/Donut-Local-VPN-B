use std::io;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use super::tls;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pos {
    Fixed(usize),
    Sni(isize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Method {
    pub record: Option<Pos>,
    pub split: Option<Pos>,
    pub delay: Duration,
    pub disorder: bool,
}

const DEFAULT_TTL: u32 = 64;

impl Pos {
    fn parse(value: &str) -> Option<Self> {
        match value.strip_prefix("sni") {
            Some("") => Some(Pos::Sni(0)),
            Some(delta) => delta.parse().ok().map(Pos::Sni),
            None => value.parse().ok().map(Pos::Fixed),
        }
    }

    fn resolve(self, sni: Option<usize>) -> Option<usize> {
        match self {
            Pos::Fixed(at) => Some(at),
            Pos::Sni(delta) => sni?.checked_add_signed(delta),
        }
    }
}

impl Method {
    /// No evasion at all: bytes are forwarded exactly as they arrive. Used when
    /// the DPI bypass is turned off in the settings.
    pub fn direct() -> Self {
        Self::default()
    }

    /// Whether this method changes the first packet at all.
    pub fn evades(&self) -> bool {
        self.record.is_some() || self.split.is_some() || self.disorder || !self.delay.is_zero()
    }

    pub fn parse(spec: &str) -> Option<Self> {
        let mut method = Method::default();
        for token in spec.split_whitespace() {
            let (key, value) = token.split_once('=')?;
            match key {
                "tlsrec" => method.record = Some(Pos::parse(value)?),
                "split" => method.split = Some(Pos::parse(value)?),
                "delay" => method.delay = Duration::from_millis(value.parse().ok()?),
                "disorder" => method.disorder = value == "1",
                _ => return None,
            }
        }
        Some(method)
    }

    pub fn apply(&self, data: &[u8]) -> (Vec<u8>, Option<usize>) {
        let sni = tls::sni_offset(data);
        let mut out = data.to_vec();
        let mut record_at = None;

        if let Some(at) = self.record.and_then(|p| p.resolve(sni)) {
            if let Some(split) = tls::split_record(data, at) {
                out = split;
                record_at = Some(at);
            }
        }

        let split_at = self
            .split
            .and_then(|p| p.resolve(sni))
            .map(|at| match record_at {
                Some(r) if at >= r => at + 5,
                _ => at,
            })
            .filter(|&at| at > 0 && at < out.len());

        (out, split_at)
    }

    pub async fn send(&self, stream: &mut TcpStream, data: &[u8]) -> io::Result<()> {
        let (payload, split) = self.apply(data);
        let Some(at) = split else {
            return stream.write_all(&payload).await;
        };
        let ttl = stream.ttl().unwrap_or(DEFAULT_TTL);
        if self.disorder {
            stream.set_ttl(1)?;
        }
        stream.write_all(&payload[..at]).await?;
        stream.flush().await?;
        if self.disorder {
            tokio::time::sleep(Duration::from_millis(5)).await;
            stream.set_ttl(ttl)?;
        }
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        stream.write_all(&payload[at..]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    #[test]
    fn parses_all_bundled_profiles() {
        for profile in catalog::proxy_profiles(catalog::Region::Auto) {
            assert!(Method::parse(profile).is_some(), "{profile}");
        }
        assert!(Method::parse("nope=1").is_none());
        assert!(Method::parse("split").is_none());
    }

    #[test]
    fn parses_positions() {
        let m = Method::parse("tlsrec=sni+1 split=sni-2 delay=50").unwrap();
        assert_eq!(m.record, Some(Pos::Sni(1)));
        assert_eq!(m.split, Some(Pos::Sni(-2)));
        assert_eq!(m.delay, Duration::from_millis(50));
        assert_eq!(Method::parse("split=1").unwrap().split, Some(Pos::Fixed(1)));
    }

    #[test]
    fn direct_mode_never_touches_the_hello() {
        let hello = tls::client_hello("discord.com");
        let method = Method::direct();
        assert!(!method.evades());
        let (out, at) = method.apply(&hello);
        assert_eq!(out, hello);
        assert_eq!(at, None);
    }

    #[test]
    fn split_inside_server_name() {
        let hello = tls::client_hello("discord.com");
        let sni = tls::sni_offset(&hello).unwrap();
        let (out, at) = Method::parse("split=sni+1").unwrap().apply(&hello);
        assert_eq!(out, hello);
        assert_eq!(at, Some(sni + 1));
    }

    #[test]
    fn split_after_record_split_is_shifted() {
        let hello = tls::client_hello("discord.com");
        let sni = tls::sni_offset(&hello).unwrap();
        let (out, at) = Method::parse("tlsrec=sni+1 split=sni+3").unwrap().apply(&hello);
        assert_eq!(out.len(), hello.len() + 5);
        assert_eq!(at, Some(sni + 3 + 5));
        assert_eq!(&out[sni + 5 + 1..sni + 5 + 11], b"iscord.com");
    }

    #[test]
    fn non_tls_is_passed_through() {
        let data = b"\x00\x01\x02";
        let (out, at) = Method::parse("tlsrec=sni+1 split=sni+1").unwrap().apply(data);
        assert_eq!(out, data);
        assert_eq!(at, None);
    }
}
