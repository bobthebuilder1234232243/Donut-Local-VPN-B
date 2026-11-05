use std::time::Duration;

#[cfg(windows)]
#[link(name = "dnsapi")]
extern "system" {
    fn DnsFlushResolverCache() -> i32;
}

fn flush_dns() {
    #[cfg(windows)]
    unsafe {
        DnsFlushResolverCache();
    }
}

pub async fn reachable(urls: &[String], proxy_port: Option<u16>) -> bool {
    flush_dns();

    let builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .connect_timeout(Duration::from_secs(7))
        .redirect(reqwest::redirect::Policy::none())
        .pool_max_idle_per_host(0)
        .user_agent(concat!("DonutLocalVPN/", env!("CARGO_PKG_VERSION")));

    let builder = match proxy_port {
        Some(port) => match reqwest::Proxy::all(format!("http://127.0.0.1:{port}")) {
            Ok(proxy) => builder.proxy(proxy),
            Err(_) => return false,
        },
        None => builder.no_proxy(),
    };

    let Ok(client) = builder.build() else {
        return false;
    };

    for _ in 0..ROUNDS {
        if !round(&client, urls).await {
            return false;
        }
    }
    true
}

const ROUNDS: usize = 2;

async fn round(client: &reqwest::Client, urls: &[String]) -> bool {
    let checks: Vec<_> = urls
        .iter()
        .map(|url| {
            let client = client.clone();
            let url = url.clone();
            tokio::spawn(async move { client.get(url).send().await.is_ok() })
        })
        .collect();

    let mut ok = true;
    for check in checks {
        ok &= matches!(check.await, Ok(true));
    }
    ok
}
