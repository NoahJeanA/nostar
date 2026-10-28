//! fontconfig rule, manifest and fontconfig commands (TECHNIK.md 10.3, 10.4).

pub mod fontconfig;
pub mod manifest;

use std::path::PathBuf;

use crate::fsutil::FsError;

#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("run {0}")]
    Spawn(&'static str, #[source] std::io::Error),
    #[error("{program} exit status {status}: {stderr}")]
    Command {
        program: &'static str,
        status: String,
        stderr: String,
    },
    #[error("{0} is a file, not a directory")]
    NotADirectory(PathBuf),
    #[error("{0}:{1}: {2}")]
    Manifest(PathBuf, usize, String),
    #[error(transparent)]
    Fs(#[from] FsError),
}

impl SystemError {
    /// `fontconfig` for fontconfig commands and directories (TECHNIK.md 12), `system` otherwise.
    pub fn module(&self) -> &'static str {
        match self {
            Self::Spawn(..) | Self::Command { .. } | Self::NotADirectory(_) => "fontconfig",
            _ => "system",
        }
    }
}
