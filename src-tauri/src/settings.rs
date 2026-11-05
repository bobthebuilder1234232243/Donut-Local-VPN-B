use std::fs;
use std::io;
use std::net::Ipv4Addr;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, Region};

pub const DEFAULT_TUNNEL_IP: &str = "10.66.0.1/24";
pub const DEFAULT_DEVICE_IP: &str = "10.66.0.2";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub region: Region,
    pub targets: Vec<String>,
    pub custom: Vec<String>,
    pub selective: bool,
    pub autostart: bool,
    pub language: String,
    pub profile: Option<String>,
    pub engine_active: bool,
    pub notifications: bool,
    pub after_vpn: bool,
    pub auto_connect: bool,
    pub dpi: bool,
    pub tunnel_ip: String,
    pub device_ip: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub region: Region,
    pub targets: Vec<String>,
    pub custom: Vec<String>,
    #[serde(default)]
    pub selective: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            region: Region::Auto,
            targets: vec!["discord".into()],
            custom: Vec::new(),
            selective: false,
            autostart: false,
            language: String::new(),
            profile: None,
            engine_active: false,
            notifications: true,
            after_vpn: true,
            auto_connect: false,
            dpi: true,
            tunnel_ip: DEFAULT_TUNNEL_IP.into(),
            device_ip: DEFAULT_DEVICE_IP.into(),
        }
    }
}

/// Accept `10.66.0.1`, `10.66.0.1/24` and their IPv6-free equivalents. A bare
/// address uses /24, which is the default for the tunnel.
pub fn parse_ipv4_cidr(input: &str) -> Option<(Ipv4Addr, u8)> {
    let input = input.trim();
    let (address, prefix) = match input.split_once('/') {
        Some((address, prefix)) => (address.parse::<Ipv4Addr>().ok()?, prefix.parse::<u8>().ok()?),
        None => (input.parse::<Ipv4Addr>().ok()?, 24),
    };
    if prefix == 0
        || prefix > 32
        || address.is_unspecified()
        || address.is_multicast()
        || address == Ipv4Addr::BROADCAST
    {
        return None;
    }
    Some((address, prefix))
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(tmp, path)
    }

    pub fn apply(&mut self, prefs: Preferences) {
        if self.region != prefs.region {
            self.profile = None;
        }
        self.region = prefs.region;
        self.selective = prefs.selective;
        self.targets = prefs
            .targets
            .into_iter()
            .filter(|id| catalog::TARGETS.iter().any(|t| t.id == id))
            .collect();
        let mut custom: Vec<String> = Vec::new();
        for domain in prefs.custom.iter().filter_map(|d| catalog::normalize_domain(d)) {
            if !custom.contains(&domain) {
                custom.push(domain);
            }
        }
        self.custom = custom;
    }

    pub fn set_language(&mut self, lang: &str) {
        let lang = lang.to_ascii_lowercase();
        self.language = if lang.starts_with("tr") {
            "tr"
        } else if lang.starts_with("ru") {
            "ru"
        } else if lang.starts_with("es") {
            "es"
        } else if lang.starts_with("it") {
            "it"
        } else if lang.starts_with("pl") {
            "pl"
        } else if lang.starts_with("ko") {
            "ko"
        } else if lang.starts_with("fr") {
            "fr"
        } else if lang.starts_with("zh-tw")
            || lang.starts_with("zh-hant")
            || lang.starts_with("zh-hk")
            || lang.starts_with("zh-mo")
        {
            "zh-TW"
        } else {
            "en"
        }
        .into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_are_sanitized() {
        let mut settings = Settings::default();
        settings.profile = Some("-9".into());
        settings.apply(Preferences {
            region: Region::Turkey,
            targets: vec!["discord".into(), "unknown".into()],
            custom: vec!["Example.com".into(), "example.com/path".into(), "not a domain".into()],
            selective: true,
        });
        assert!(settings.selective);
        assert_eq!(settings.targets, vec!["discord"]);
        assert_eq!(settings.custom, vec!["example.com"]);
        assert_eq!(settings.profile, None);
    }

    #[test]
    fn language_is_normalized() {
        let mut settings = Settings::default();
        for (input, expected) in [
            ("tr-TR", "tr"),
            ("ru-RU", "ru"),
            ("es-ES", "es"),
            ("it-IT", "it"),
            ("pl-PL", "pl"),
            ("ko-KR", "ko"),
            ("zh-TW", "zh-TW"),
            ("zh-Hant-HK", "zh-TW"),
            ("fr-FR", "fr"),
            ("de-DE", "en"),
            ("zh-CN", "en"),
        ] {
            settings.set_language(input);
            assert_eq!(settings.language, expected, "{input}");
        }
    }

    #[test]
    fn tunnel_addresses_are_validated() {
        assert_eq!(parse_ipv4_cidr("10.66.0.1"), Some(("10.66.0.1".parse().unwrap(), 24)));
        assert_eq!(parse_ipv4_cidr(" 10.66.0.1/30 "), Some(("10.66.0.1".parse().unwrap(), 30)));
        for invalid in ["", "10.66.0", "10.66.0.256", "0.0.0.0", "10.66.0.1/0", "10.66.0.1/33", "::1", "10.66.0.1/x"] {
            assert_eq!(parse_ipv4_cidr(invalid), None, "{invalid}");
        }
    }
}
