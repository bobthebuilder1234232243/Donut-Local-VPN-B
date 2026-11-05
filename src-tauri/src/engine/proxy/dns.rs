use std::collections::HashMap;
use std::io;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;

const ENDPOINTS: &[&str] = &["https://1.1.1.1/dns-query", "https://8.8.8.8/resolve"];
const TTL: Duration = Duration::from_secs(300);
const A_RECORD: u16 = 1;

#[derive(Deserialize)]
struct Reply {
    #[serde(rename = "Answer", default)]
    answer: Vec<Answer>,
}

#[derive(Deserialize)]
struct Answer {
    #[serde(rename = "type")]
    kind: u16,
    data: String,
}

pub struct Resolver {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, (Vec<IpAddr>, Instant)>>,
}

impl Resolver {
    /// `interface` pins DNS over HTTPS requests to the physical interface so
    /// they do not travel through the tunnel they are helping to set up.
    pub fn new(interface: Option<String>) -> Self {
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5));
        #[cfg(unix)]
        if let Some(interface) = interface {
            builder = builder.interface(interface.as_str());
        }
        #[cfg(not(unix))]
        let _ = interface;
        let client = builder.build().unwrap_or_default();
        Self { client, cache: Mutex::new(HashMap::new()) }
    }

    pub async fn lookup(&self, host: &str) -> io::Result<Vec<IpAddr>> {
        if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
            return Ok(vec![ip]);
        }
        let host = host.to_ascii_lowercase();
        if let Some(ips) = self.cached(&host) {
            return Ok(ips);
        }
        if is_hostname(&host) {
            for endpoint in ENDPOINTS {
                if let Some(ips) = self.query(endpoint, &host).await {
                    if let Ok(mut cache) = self.cache.lock() {
                        cache.insert(host.clone(), (ips.clone(), Instant::now()));
                    }
                    return Ok(ips);
                }
            }
        }
        let ips: Vec<IpAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.map(|a| a.ip()).collect();
        if ips.is_empty() {
            Err(io::ErrorKind::NotFound.into())
        } else {
            Ok(ips)
        }
    }

    fn cached(&self, host: &str) -> Option<Vec<IpAddr>> {
        let cache = self.cache.lock().ok()?;
        cache
            .get(host)
            .filter(|(_, at)| at.elapsed() < TTL)
            .map(|(ips, _)| ips.clone())
    }

    async fn query(&self, endpoint: &str, host: &str) -> Option<Vec<IpAddr>> {
        let reply: Reply = self
            .client
            .get(format!("{endpoint}?name={host}&type=A"))
            .header("accept", "application/dns-json")
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let ips: Vec<IpAddr> = reply
            .answer
            .into_iter()
            .filter(|a| a.kind == A_RECORD)
            .filter_map(|a| a.data.parse().ok())
            .collect();
        (!ips.is_empty()).then_some(ips)
    }
}

fn is_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_check_blocks_injection() {
        assert!(is_hostname("discord.com"));
        assert!(!is_hostname("a.com&type=AAAA"));
        assert!(!is_hostname(""));
    }

    #[tokio::test]
    async fn literal_addresses_skip_lookup() {
        let resolver = Resolver::new(None);
        assert_eq!(resolver.lookup("127.0.0.1").await.unwrap(), vec!["127.0.0.1".parse::<IpAddr>().unwrap()]);
        assert_eq!(resolver.lookup("[::1]").await.unwrap(), vec!["::1".parse::<IpAddr>().unwrap()]);
    }
}
