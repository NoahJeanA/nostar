//! `manifest.toml`: the patched fonts and the directories `apply` created (TECHNIK.md 10.4).

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::SystemError;
use crate::fsutil::{FsError, write_atomic};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// SHA-256 of the `rules.tsv` the fonts were patched with.
    #[serde(default)]
    pub rules: String,
    #[serde(default)]
    pub created_dirs: Vec<PathBuf>,
    #[serde(default, rename = "font")]
    pub fonts: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub original: PathBuf,
    pub original_sha256: String,
    pub copy: PathBuf,
}

impl Manifest {
    /// A missing file is an empty manifest.
    pub fn load(path: &Path) -> Result<Self, SystemError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(FsError::io("read", path)(e).into()),
        };
        toml::from_str(&text).map_err(|e| {
            let line = e.span().map_or(1, |s| {
                text[..s.start.min(text.len())].matches('\n').count() + 1
            });
            SystemError::Manifest(path.to_path_buf(), line, e.message().to_string())
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), SystemError> {
        let text = toml::to_string(self)
            .map_err(|e| SystemError::Manifest(path.to_path_buf(), 0, e.to_string()))?;
        write_atomic(path, text.as_bytes())?;
        log::info!("wrote {}", path.display());
        Ok(())
    }

    pub fn entry(&self, original: &Path) -> Option<&Entry> {
        self.fonts.iter().find(|e| e.original == original)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_missing() {
        let dir = std::env::temp_dir().join(format!("nostar-test-{}-manifest", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("manifest.toml");
        assert_eq!(Manifest::load(&path).unwrap(), Manifest::default());
        let m = Manifest {
            rules: "ab".repeat(32),
            created_dirs: vec!["/home/u/.local/share/fonts".into()],
            fonts: vec![Entry {
                original: "/usr/share/fonts/a.ttf".into(),
                original_sha256: "cd".repeat(32),
                copy: "/home/u/.local/share/fonts/nostar/usr/share/fonts/a.ttf".into(),
            }],
        };
        m.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[[font]]"), "{text}");
        assert_eq!(Manifest::load(&path).unwrap(), m);
        std::fs::write(&path, "rules = 1\n").unwrap();
        assert!(matches!(
            Manifest::load(&path),
            Err(SystemError::Manifest(_, 1, _))
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
