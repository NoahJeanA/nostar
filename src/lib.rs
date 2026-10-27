//! nostar: displays German gender forms in the generic masculine by patching font copies.
//!
//! The library holds all modules so that integration tests in `tests/` can reach them;
//! `main.rs` only parses the command line and maps errors to exit codes.

pub mod config;
pub mod fsutil;
pub mod log;
pub mod paths;

use config::ConfigError;
use fsutil::FsError;
use paths::PathsError;

/// All errors that abort a command (exit code 2, TECHNIK.md 12.1).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Paths(#[from] PathsError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Fs(#[from] FsError),
    /// Command defined but not built yet (milestones 3 to 6).
    #[error("{0}: not implemented")]
    NotImplemented(&'static str),
}

impl Error {
    /// Module name shown as `error: <module>: <message>`.
    pub fn module(&self) -> &'static str {
        match self {
            Self::Paths(_) => "paths",
            Self::Config(_) => "config",
            Self::Fs(FsError::LockHeld(_)) => "lock",
            Self::Fs(_) => "fs",
            Self::NotImplemented(_) => "cmd",
        }
    }
}
