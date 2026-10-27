//! nostar: displays German gender forms in the generic masculine by patching font copies.
//!
//! The library holds all modules so that integration tests in `tests/` can reach them;
//! `main.rs` only parses the command line and maps errors to exit codes.

pub mod cmd;
pub mod config;
pub mod fsutil;
pub mod lexicon;
pub mod log;
pub mod paths;
pub mod rules;
pub mod source;

use config::ConfigError;
use fsutil::FsError;
use paths::PathsError;
use rules::RulesError;
use source::SourceError;

/// All errors that abort a command (exit code 2, TECHNIK.md 12.1).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Paths(#[from] PathsError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Fs(#[from] FsError),
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Rules(#[from] RulesError),
    /// Command defined but not built yet (milestones 5 and 6).
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
            Self::Source(_) => "source",
            Self::Rules(_) => "rules",
            Self::NotImplemented(_) => "cmd",
        }
    }
}
