use std::path::PathBuf;
use std::sync::Arc;

use super::proxy::{Method, Resolver, Server};
use super::tunnel::Tunnel;
use super::EngineError;
use crate::catalog::{self, Region};

pub struct Engine {
    resolver: Arc<Resolver>,
}

pub struct Running {
    /// The unprivileged proxy used while the settings are being tested.
    server: Option<Server>,
    tunnel: Option<Tunnel>,
    port: u16,
    profile: String,
    only: Option<Vec<String>>,
    active: bool,
}

impl Engine {
    pub fn new(_resources: PathBuf) -> Self {
        Self { resolver: Arc::new(Resolver::new(None)) }
    }

    pub fn profiles(region: Region) -> Vec<&'static str> {
        catalog::proxy_profiles(region)
    }

    pub fn recover() {
        // The tunnel helper exits and restores the routes by itself when the app
        // that started it goes away, so there is nothing to undo here.
    }

    pub fn unload() {}

    pub async fn prepare(&self) -> Result<(), EngineError> {
        Ok(())
    }

    pub async fn start(&self, profile: &str, only: Option<&[String]>) -> Result<Running, EngineError> {
        let method =
            Method::parse(profile).ok_or_else(|| EngineError::Failed(format!("invalid profile: {profile}")))?;
        let only = only.map(|domains| domains.to_vec());
        let server = Server::start(method, self.resolver.clone(), only.clone().map(Arc::new), None)
            .await
            .map_err(|error| EngineError::Failed(error.to_string()))?;
        let port = server.port();
        Ok(Running {
            server: Some(server),
            tunnel: None,
            port,
            profile: profile.to_string(),
            only,
            active: false,
        })
    }
}

impl Running {
    pub fn alive(&mut self) -> bool {
        true
    }

    pub fn probe_proxy(&self) -> Option<u16> {
        Some(self.port)
    }

    /// Replace the temporary loopback proxy with the privileged tunnel. The
    /// helper runs the same proxy with its outbound sockets bound to the
    /// physical interface, so traffic can no longer loop back into the tunnel.
    pub async fn activate(&mut self) -> Result<(), EngineError> {
        drop(self.server.take());
        let profile = self.profile.clone();
        let only = self.only.clone().unwrap_or_default();
        let mut tunnel = tokio::task::spawn_blocking(move || Tunnel::start(&profile, &only))
            .await
            .map_err(|error| EngineError::Failed(error.to_string()))??;
        tunnel.up()?;
        self.port = tunnel.port();
        self.tunnel = Some(tunnel);
        self.active = true;
        Ok(())
    }

    pub async fn refresh(&mut self) -> Result<(), EngineError> {
        if let Some(tunnel) = &mut self.tunnel {
            if self.active {
                tunnel.up()?;
            }
        }
        Ok(())
    }

    pub async fn stop(mut self) {
        if let Some(tunnel) = self.tunnel.take() {
            tunnel.stop().await;
        }
        drop(self.server.take());
    }
}
