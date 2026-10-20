use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, Region};

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
        }
    }
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
        settings.set_language("tr-TR");
        assert_eq!(settings.language, "tr");
        settings.set_language("ru-RU");
        assert_eq!(settings.language, "ru");
        settings.set_language("de-DE");
        assert_eq!(settings.language, "en");
    }
}
