//! All file system locations of nostar (TECHNIK.md 6). No other module builds paths.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    #[error("HOME not set or not absolute, needed for {0}")]
    NoHome(&'static str),
}

/// Base directories resolved once from the environment.
#[derive(Debug, Clone)]
pub struct Paths {
    home: Option<PathBuf>,
    config: PathBuf,
    data: PathBuf,
    state: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Self, PathsError> {
        Self::from_vars(|name| std::env::var_os(name))
    }

    fn from_vars(var: impl Fn(&str) -> Option<OsString>) -> Result<Self, PathsError> {
        let absolute = |name: &str| var(name).map(PathBuf::from).filter(|p| p.is_absolute());
        let home = absolute("HOME");
        // XDG base directory spec: relative values are invalid and ignored.
        let base = |name: &'static str, default: &str| {
            absolute(name)
                .or_else(|| home.as_ref().map(|h| h.join(default)))
                .ok_or(PathsError::NoHome(name))
        };
        Ok(Self {
            config: base("XDG_CONFIG_HOME", ".config")?,
            data: base("XDG_DATA_HOME", ".local/share")?,
            state: base("XDG_STATE_HOME", ".local/state")?,
            home,
        })
    }

    /// Paths below `home` without reading the environment.
    #[cfg(test)]
    pub fn from_vars_for_tests(home: &Path) -> Self {
        let home = home.as_os_str().to_os_string();
        Self::from_vars(|name| (name == "HOME").then(|| home.clone()))
            .unwrap_or_else(|_| unreachable!("absolute test home"))
    }

    pub fn config_file(&self) -> PathBuf {
        self.config.join("nostar/config.toml")
    }

    /// Downloaded dictionary, named after the first 12 characters of its commit.
    pub fn dictionary(&self, commit: &str) -> PathBuf {
        let short: String = commit.chars().take(12).collect();
        self.data
            .join(format!("nostar/source/entgendert-{short}.js"))
    }

    pub fn rules(&self) -> PathBuf {
        self.data.join("nostar/rules.tsv")
    }

    pub fn fonts_dir(&self) -> PathBuf {
        self.data.join("fonts/nostar")
    }

    /// Patched copy of `original`, mirroring its absolute path below [`Self::fonts_dir`].
    pub fn patched_font(&self, original: &Path) -> PathBuf {
        let relative = original.strip_prefix("/").unwrap_or(original);
        self.fonts_dir().join(relative)
    }

    /// Original of a patched copy: the inverse of [`Self::patched_font`].
    pub fn original_of(&self, copy: &Path) -> Option<PathBuf> {
        copy.strip_prefix(self.fonts_dir())
            .ok()
            .map(|rel| Path::new("/").join(rel))
    }

    /// `~/.local/share/fonts`, the user font directory fontconfig reads.
    pub fn fonts_base(&self) -> PathBuf {
        self.data.join("fonts")
    }

    pub fn source_dir(&self) -> PathBuf {
        self.data.join("nostar/source")
    }

    /// The nostar directories below the three XDG bases.
    pub fn own_dirs(&self) -> [PathBuf; 3] {
        [
            self.config.join("nostar"),
            self.data.join("nostar"),
            self.state.join("nostar"),
        ]
    }

    /// `~/.config/fontconfig` and its `conf.d`.
    pub fn fontconfig_dirs(&self) -> [PathBuf; 2] {
        [
            self.config.join("fontconfig"),
            self.config.join("fontconfig/conf.d"),
        ]
    }

    pub fn fontconfig_rule(&self) -> PathBuf {
        self.config.join("fontconfig/conf.d/99-nostar.conf")
    }

    pub fn manifest(&self) -> PathBuf {
        self.state.join("nostar/manifest.toml")
    }

    pub fn log(&self) -> PathBuf {
        self.state.join("nostar/nostar.log")
    }

    pub fn log_backup(&self) -> PathBuf {
        self.state.join("nostar/nostar.log.1")
    }

    pub fn lock(&self) -> PathBuf {
        self.state.join("nostar/lock")
    }

    /// Shortens the home directory to `~` in user-facing text.
    pub fn tilde(&self, text: &str) -> String {
        match self.home.as_ref().and_then(|h| h.to_str()) {
            Some(h) if h != "/" => text.replace(h, "~"),
            _ => text.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn paths(vars: &[(&str, &str)]) -> Result<Paths, PathsError> {
        let map: HashMap<String, OsString> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        Paths::from_vars(|name| map.get(name).cloned())
    }

    #[test]
    fn defaults_below_home() {
        let p = paths(&[("HOME", "/home/u")]).unwrap();
        assert_eq!(
            p.config_file(),
            Path::new("/home/u/.config/nostar/config.toml")
        );
        assert_eq!(
            p.rules(),
            Path::new("/home/u/.local/share/nostar/rules.tsv")
        );
        assert_eq!(p.lock(), Path::new("/home/u/.local/state/nostar/lock"));
        assert_eq!(
            p.fontconfig_rule(),
            Path::new("/home/u/.config/fontconfig/conf.d/99-nostar.conf")
        );
    }

    #[test]
    fn xdg_overrides_and_relative_values_ignored() {
        let p = paths(&[
            ("HOME", "/home/u"),
            ("XDG_CONFIG_HOME", "/tmp/c"),
            ("XDG_DATA_HOME", "rel"),
            ("XDG_STATE_HOME", "/tmp/s"),
        ])
        .unwrap();
        assert_eq!(p.config_file(), Path::new("/tmp/c/nostar/config.toml"));
        assert_eq!(
            p.rules(),
            Path::new("/home/u/.local/share/nostar/rules.tsv")
        );
        assert_eq!(p.manifest(), Path::new("/tmp/s/nostar/manifest.toml"));
    }

    #[test]
    fn no_home_needed_when_all_xdg_set() {
        let p = paths(&[
            ("XDG_CONFIG_HOME", "/c"),
            ("XDG_DATA_HOME", "/d"),
            ("XDG_STATE_HOME", "/s"),
        ])
        .unwrap();
        assert_eq!(p.log(), Path::new("/s/nostar/nostar.log"));
        assert!(matches!(
            paths(&[("XDG_CONFIG_HOME", "/c")]),
            Err(PathsError::NoHome("XDG_DATA_HOME"))
        ));
    }

    #[test]
    fn dictionary_and_patched_font() {
        let p = paths(&[("HOME", "/home/u")]).unwrap();
        assert_eq!(
            p.dictionary("febca802f868c4ce968bbae70c099cdbc0454e18"),
            Path::new("/home/u/.local/share/nostar/source/entgendert-febca802f868.js")
        );
        assert_eq!(
            p.patched_font(Path::new("/usr/share/fonts/noto/NotoSans-Regular.ttf")),
            Path::new(
                "/home/u/.local/share/fonts/nostar/usr/share/fonts/noto/NotoSans-Regular.ttf"
            )
        );
    }

    #[test]
    fn original_of_copy() {
        let p = paths(&[("HOME", "/home/u")]).unwrap();
        let orig = Path::new("/usr/share/fonts/noto/NotoSans-Regular.ttf");
        assert_eq!(p.original_of(&p.patched_font(orig)).as_deref(), Some(orig));
        assert_eq!(p.original_of(orig), None);
    }

    #[test]
    fn tilde() {
        let p = paths(&[("HOME", "/home/u")]).unwrap();
        assert_eq!(p.tilde("/home/u/.config/x: bad"), "~/.config/x: bad");
    }
}
