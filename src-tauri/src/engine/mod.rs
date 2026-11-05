#[cfg_attr(windows, allow(dead_code))]
pub mod proxy;

#[cfg(windows)]
mod driver;
#[cfg(windows)]
pub use driver::{Engine, Running};

#[cfg(not(windows))]
mod proxied;
#[cfg(not(windows))]
pub use proxied::{Engine, Running};

#[cfg(not(windows))]
pub mod tunnel;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    MissingFiles,
    NeedsAdmin,
    Tunnel,
    NoProfile,
    Failed(String),
}

impl EngineError {
    pub fn code(&self) -> &'static str {
        match self {
            EngineError::MissingFiles => "missing_files",
            EngineError::NeedsAdmin => "needs_admin",
            EngineError::Tunnel => "tunnel",
            EngineError::NoProfile => "no_profile",
            EngineError::Failed(_) => "engine",
        }
    }

    pub fn fatal(&self) -> bool {
        !matches!(self, EngineError::Failed(_))
    }
}
