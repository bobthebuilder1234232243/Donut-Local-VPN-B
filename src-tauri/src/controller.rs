use std::future::Future;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{watch, Mutex as AsyncMutex};

use crate::autostart;
use crate::catalog;
use crate::engine::{Engine, EngineError, Running};
use crate::log;
use crate::network;
use crate::notify;
use crate::i18n;
use crate::probe;
use crate::settings::{Preferences, Settings};
use crate::tray;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Off,
    Connecting,
    On,
    Stopping,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: Phase,
    pub step: usize,
    pub total: usize,
    pub profile: Option<String>,
    pub error: Option<String>,
    pub since: Option<u64>,
    pub port: Option<u16>,
}

impl Status {
    fn new(phase: Phase) -> Self {
        Self { phase, step: 0, total: 0, profile: None, error: None, since: None, port: None }
    }

    fn connecting(step: usize, total: usize) -> Self {
        Self { step, total, ..Self::new(Phase::Connecting) }
    }

    fn on(profile: String, step: usize, total: usize, port: Option<u16>) -> Self {
        let since = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        Self { profile: Some(profile), step, total, since: Some(since), port, ..Self::new(Phase::On) }
    }

    fn failed(error: &EngineError) -> Self {
        Self { error: Some(error.code().into()), ..Self::new(Phase::Error) }
    }
}

enum Stop {
    Cancelled,
    Failed(EngineError),
}

pub struct Controller {
    app: AppHandle,
    path: PathBuf,
    engine: Engine,
    settings: Mutex<Settings>,
    status: Mutex<Status>,
    session: AsyncMutex<Option<Running>>,
    cancel: Mutex<Option<watch::Sender<bool>>>,
    failures: Mutex<u32>,
    demoted: Mutex<Option<String>>,
    recovering: Mutex<bool>,
    wanted: Mutex<bool>,
}

const MAX_FAILURES: u32 = 2;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn cancellable<F: Future>(cancel: &mut watch::Receiver<bool>, future: F) -> Option<F::Output> {
    tokio::select! {
        output = future => Some(output),
        _ = cancel.wait_for(|stop| *stop) => None,
    }
}

impl Controller {
    pub fn new(app: &AppHandle) -> tauri::Result<Self> {
        let path = app.path().app_config_dir()?.join("settings.json");
        let mut settings = Settings::load(&path);
        if settings.engine_active {
            Engine::recover();
            settings.engine_active = false;
        }
        settings.autostart = autostart::enabled();
        let _ = settings.save(&path);

        Ok(Self {
            app: app.clone(),
            path,
            engine: Engine::new(app.path().resource_dir()?),
            settings: Mutex::new(settings),
            status: Mutex::new(Status::new(Phase::Off)),
            session: AsyncMutex::new(None),
            cancel: Mutex::new(None),
            failures: Mutex::new(0),
            demoted: Mutex::new(None),
            recovering: Mutex::new(false),
            wanted: Mutex::new(false),
        })
    }

    pub fn status(&self) -> Status {
        lock(&self.status).clone()
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    fn update<F: FnOnce(&mut Settings)>(&self, change: F) -> Settings {
        let mut settings = lock(&self.settings);
        change(&mut settings);
        let _ = settings.save(&self.path);
        settings.clone()
    }

    fn publish(&self) {
        let status = self.status();
        let settings = self.settings();
        let _ = self.app.emit("status", &status);
        let _ = self.app.emit("settings", &settings);
        tray::update(&self.app, &status, &settings);
    }

    fn set_status(&self, status: Status) {
        if status.phase != Phase::Connecting {
            log::write(format!(
                "status {:?}{}{}",
                status.phase,
                status.profile.as_deref().map(|p| format!(" profile={p}")).unwrap_or_default(),
                status.error.as_deref().map(|e| format!(" error={e}")).unwrap_or_default()
            ));
        }
        *lock(&self.status) = status;
        self.publish();
    }

    pub fn detect_language(&self, language: &str) {
        if self.settings().language.is_empty() {
            self.set_language(language);
        }
    }

    pub fn set_language(&self, language: &str) -> Settings {
        let settings = self.update(|s| s.set_language(language));
        self.publish();
        settings
    }

    pub fn save_preferences(&self, preferences: Preferences) -> Settings {
        let settings = self.update(|s| s.apply(preferences));
        self.publish();
        settings
    }

    pub async fn set_autostart(&self, enabled: bool) -> Result<Settings, String> {
        let result = tokio::task::spawn_blocking(move || autostart::set(enabled))
            .await
            .map_err(|e| e.to_string())?;
        let actual = tokio::task::spawn_blocking(autostart::enabled).await.unwrap_or(false);
        let settings = self.update(|s| s.autostart = actual);
        self.publish();
        result.map(|_| settings)
    }

    fn notify(&self, body: &str) {
        if self.settings().notifications {
            notify::show(&self.app, body);
        }
    }

    pub fn set_after_vpn(&self, enabled: bool) -> Settings {
        let settings = self.update(|s| s.after_vpn = enabled);
        self.publish();
        settings
    }

    pub fn set_notifications(&self, enabled: bool) -> Settings {
        let settings = self.update(|s| s.notifications = enabled);
        self.publish();
        settings
    }

    pub async fn disconnect_by_user(&self) {
        *lock(&self.wanted) = false;
        let was = self.status().phase;
        self.disconnect().await;
        if was == Phase::On {
            self.notify(i18n::labels(&self.settings().language).disconnected);
        }
    }

    pub async fn toggle(&self) {
        match self.status().phase {
            Phase::Off | Phase::Error => self.connect().await,
            Phase::Connecting | Phase::On => self.disconnect_by_user().await,
            Phase::Stopping => {}
        }
    }

    pub async fn connect(&self) {
        let (sender, mut cancel) = watch::channel(false);
        {
            let mut status = lock(&self.status);
            if matches!(status.phase, Phase::Connecting | Phase::On | Phase::Stopping) {
                return;
            }
            *status = Status::connecting(0, 0);
        }
        *lock(&self.cancel) = Some(sender);
        *lock(&self.wanted) = true;
        self.publish();

        let mut session = self.session.lock().await;
        let result = self.search(&mut cancel).await;
        *lock(&self.cancel) = None;

        let recovering = std::mem::take(&mut *lock(&self.recovering));
        let labels = i18n::labels(&self.settings().language);
        match (&result, recovering) {
            (Ok(_), true) => self.notify(labels.restored),
            (Ok(_), false) => self.notify(labels.connected),
            (Err(Stop::Failed(_)), true) => self.notify(labels.failed),
            (Err(Stop::Failed(_)), false) => self.notify(labels.not_connected),
            (Err(Stop::Cancelled), _) => {}
        }

        match result {
            Ok((running, profile)) => {
                let port = running.probe_proxy();
                *session = Some(running);
                self.update(|s| s.profile = Some(profile.clone()));
                let number = self.profile_number(&profile);
                self.set_status(Status::on(profile, number.0, number.1, port));
            }
            Err(Stop::Cancelled) => self.set_status(Status::new(Phase::Off)),
            Err(Stop::Failed(error)) => self.set_status(Status::failed(&error)),
        }
    }

    pub async fn disconnect(&self) {
        if let Some(sender) = lock(&self.cancel).as_ref() {
            let _ = sender.send(true);
        }
        let mut session = self.session.lock().await;
        if let Some(running) = session.take() {
            self.set_status(Status::new(Phase::Stopping));
            running.stop().await;
        }
        self.update(|s| s.engine_active = false);
        self.set_status(Status::new(Phase::Off));
    }

    pub async fn health_check(&self) {
        self.check(MAX_FAILURES).await;
    }

    pub async fn network_changed(&self) {
        let phase = self.status().phase;
        if phase == Phase::Error && *lock(&self.wanted) && self.settings().after_vpn {
            log::write("network changed after a failure, reconnecting");
            *lock(&self.recovering) = true;
            self.connect().await;
            return;
        }
        if phase != Phase::On {
            return;
        }
        if let Some(running) = self.session.lock().await.as_mut() {
            if let Err(error) = running.refresh().await {
                log::write(format!("refresh failed: {}", error.code()));
            }
        }
        self.check(1).await;
    }

    pub fn diagnostics(&self) -> String {
        let settings = self.settings();
        let status = self.status();
        let mut out = vec![
            format!("FreeDPI {}", env!("CARGO_PKG_VERSION")),
            format!("os: {} {}", std::env::consts::OS, std::env::consts::ARCH),
            format!("region: {:?}", settings.region),
            format!("targets: {}", settings.targets.join(", ")),
            format!("custom sites: {}", settings.custom.len()),
            format!("only test sites: {}", settings.selective),
            format!("autostart: {}", settings.autostart),
            format!("status: {:?} profile={:?} error={:?}", status.phase, status.profile, status.error),
            format!("online: {}", network::online()),
            String::new(),
        ];
        out.extend(log::recent());
        out.join("\n")
    }

    async fn check(&self, limit: u32) {
        if self.status().phase != Phase::On {
            *lock(&self.failures) = 0;
            return;
        }
        if !network::online() {
            return;
        }
        let settings = self.settings();
        let urls = catalog::target_urls(&settings.targets, &settings.custom);
        let ok = probe::reachable(&urls, self.proxy_port().await).await;
        if self.status().phase != Phase::On {
            return;
        }
        {
            let mut failures = lock(&self.failures);
            if ok {
                *failures = 0;
                return;
            }
            *failures += 1;
            log::write(format!("health check failed ({}/{limit})", *failures));
            if *failures < limit {
                return;
            }
            *failures = 0;
        }
        if !network::online() {
            return;
        }
        log::write("searching for a new setting");
        *lock(&self.recovering) = true;
        self.notify(i18n::labels(&self.settings().language).lost);
        if limit > 1 {
            *lock(&self.demoted) = self.status().profile;
            self.update(|s| s.profile = None);
        }
        self.disconnect().await;
        self.connect().await;
    }

    fn profile_number(&self, profile: &str) -> (usize, usize) {
        let all = Engine::profiles(self.settings().region);
        let index = all.iter().position(|p| *p == profile).map_or(0, |i| i + 1);
        (index, all.len())
    }

    pub async fn proxy_port(&self) -> Option<u16> {
        self.session.lock().await.as_ref().and_then(|running| running.probe_proxy())
    }

    async fn search(&self, cancel: &mut watch::Receiver<bool>) -> Result<(Running, String), Stop> {
        let settings = self.settings();
        let urls = catalog::target_urls(&settings.targets, &settings.custom);
        let only = settings
            .selective
            .then(|| catalog::target_domains(&settings.targets, &settings.custom));
        let mut profiles = catalog::ordered(Engine::profiles(settings.region), settings.profile.as_deref());
        if let Some(demoted) = lock(&self.demoted).take() {
            if let Some(index) = profiles.iter().position(|p| *p == demoted) {
                let item = profiles.remove(index);
                profiles.push(item);
            }
        }
        log::write(format!(
            "search region={:?} targets={} selective={}",
            settings.region,
            settings.targets.join(","),
            settings.selective
        ));
        self.engine.prepare().await.map_err(Stop::Failed)?;

        let total = profiles.len();
        for (index, profile) in profiles.into_iter().enumerate() {
            if *cancel.borrow() {
                return Err(Stop::Cancelled);
            }
            self.set_status(Status::connecting(index + 1, total));

            let mut running = match self.engine.start(&profile, only.as_deref()).await {
                Ok(running) => running,
                Err(error) if error.fatal() => return Err(Stop::Failed(error)),
                Err(error) => {
                    log::write(format!("setting {} failed to start: {error:?}", index + 1));
                    continue;
                }
            };

            let reachable = if running.alive() {
                match cancellable(cancel, probe::reachable(&urls, running.probe_proxy())).await {
                    Some(ok) => ok,
                    None => {
                        running.stop().await;
                        return Err(Stop::Cancelled);
                    }
                }
            } else {
                false
            };

            if !reachable || *cancel.borrow() {
                if !reachable {
                    log::write(format!("setting {} did not work", index + 1));
                }
                running.stop().await;
                if *cancel.borrow() {
                    return Err(Stop::Cancelled);
                }
                continue;
            }

            self.update(|s| s.engine_active = true);
            if let Err(error) = running.activate().await {
                running.stop().await;
                self.update(|s| s.engine_active = false);
                return Err(Stop::Failed(error));
            }
            return Ok((running, profile));
        }
        Err(Stop::Failed(EngineError::NoProfile))
    }
}
