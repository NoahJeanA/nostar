//! `config.toml` (TECHNIK.md 5): read, check and fill in defaults.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

const DEFAULT_COMMIT: &str = "febca802f868c4ce968bbae70c099cdbc0454e18";
const DEFAULT_SHA256: &str = "6330c126964385e84fb5f01c47893a2a75a65b163f6be4bd19bc2448517f73b3";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Read(PathBuf, #[source] io::Error),
    #[error("{path}:{line}: {message}")]
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("{key}: {message}")]
    Invalid { key: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Standard,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    Cut,
    Marker,
    Off,
}

/// A gender marker as written in the text (fachl. Spezifikation 4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarkerChar {
    Star,
    Colon,
    Underscore,
    MiddleDot,
    Slash,
    SlashHyphen,
}

impl MarkerChar {
    pub const ALL: [MarkerChar; 6] = [
        Self::Star,
        Self::Colon,
        Self::Underscore,
        Self::MiddleDot,
        Self::Slash,
        Self::SlashHyphen,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Star => "*",
            Self::Colon => ":",
            Self::Underscore => "_",
            Self::MiddleDot => "·",
            Self::Slash => "/",
            Self::SlashHyphen => "/-",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub commit: String,
    pub sha256: String,
}

/// Own word from `[words.<stem>]`; `compound` defaults to `plural`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub singular: String,
    pub plural: String,
    pub compound: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub mode: Mode,
    pub families: Vec<String>,
    pub exclude: Vec<String>,
    pub markers: Vec<MarkerChar>,
    pub binnen_i: bool,
    pub fallback: Fallback,
    pub participles: bool,
    pub skip_monospace: bool,
    pub source: Source,
    /// Key: feminine stem as written before the marker.
    pub words: BTreeMap<String, Word>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct RawConfig {
    mode: Option<String>,
    families: Vec<String>,
    exclude: Vec<String>,
    markers: Option<Vec<String>>,
    binnen_i: Option<bool>,
    fallback: Option<String>,
    participles: bool,
    skip_monospace: bool,
    source: Option<RawSource>,
    words: BTreeMap<String, RawWord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    commit: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWord {
    singular: String,
    plural: String,
    compound: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: Mode::Standard,
            families: Vec::new(),
            exclude: Vec::new(),
            markers: MarkerChar::ALL.to_vec(),
            binnen_i: true,
            fallback: Fallback::Cut,
            participles: false,
            skip_monospace: false,
            source: Source {
                commit: DEFAULT_COMMIT.into(),
                sha256: DEFAULT_SHA256.into(),
            },
            words: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Reads `path`; a missing file means all defaults.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text, path),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Read(path.to_path_buf(), e)),
        }
    }

    /// `path` is only used in messages.
    pub fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| ConfigError::Parse {
            path: path.to_path_buf(),
            line: e.span().map_or(1, |s| {
                text[..s.start.min(text.len())].matches('\n').count() + 1
            }),
            message: parse_message(e.message()),
        })?;
        let d = Self::default();
        Ok(Self {
            mode: match raw.mode.as_deref() {
                None | Some("standard") => Mode::Standard,
                Some("all") => Mode::All,
                Some(other) => {
                    return Err(invalid(
                        "mode",
                        format!(r#"expected "standard" or "all", got "{other}""#),
                    ));
                }
            },
            families: check_entries("families", raw.families)?,
            exclude: check_exclude(raw.exclude)?,
            markers: match raw.markers {
                None => d.markers,
                Some(m) => check_markers(&m)?,
            },
            binnen_i: raw.binnen_i.unwrap_or(d.binnen_i),
            fallback: match raw.fallback.as_deref() {
                None | Some("cut") => Fallback::Cut,
                Some("marker") => Fallback::Marker,
                Some("off") => Fallback::Off,
                Some(other) => {
                    return Err(invalid(
                        "fallback",
                        format!(r#"expected "cut", "marker" or "off", got "{other}""#),
                    ));
                }
            },
            participles: raw.participles,
            skip_monospace: raw.skip_monospace,
            source: match raw.source {
                None => d.source,
                Some(s) => Source {
                    commit: hex("source.commit", &s.commit, 40)?,
                    sha256: hex("source.sha256", &s.sha256, 64)?,
                },
            },
            words: raw
                .words
                .into_iter()
                .map(|(stem, w)| check_word(stem, w))
                .collect::<Result<_, _>>()?,
        })
    }
}

fn invalid(key: &str, message: String) -> ConfigError {
    ConfigError::Invalid {
        key: key.into(),
        message,
    }
}

/// toml says ``unknown field `fallbak`, expected one of …``; TECHNIK.md 12 wants `unknown key "fallbak"`.
fn parse_message(m: &str) -> String {
    let quoted = |prefix: &str| {
        let rest = m.strip_prefix(prefix)?;
        let name = rest.strip_prefix('`')?.split('`').next()?;
        Some(name.to_string())
    };
    if let Some(k) = quoted("unknown field ") {
        format!(r#"unknown key "{k}""#)
    } else if let Some(k) = quoted("missing field ") {
        format!(r#"missing key "{k}""#)
    } else {
        m.trim_end_matches('.').replace('`', "\"")
    }
}

fn check_entries(key: &str, entries: Vec<String>) -> Result<Vec<String>, ConfigError> {
    match entries.iter().any(|e| e.trim().is_empty()) {
        true => Err(invalid(key, "empty entry".into())),
        false => Ok(entries),
    }
}

/// Entries are family names or absolute paths; anything with a slash counts as a path.
fn check_exclude(entries: Vec<String>) -> Result<Vec<String>, ConfigError> {
    let entries = check_entries("exclude", entries)?;
    match entries
        .iter()
        .find(|e| e.contains('/') && !e.starts_with('/'))
    {
        Some(e) => Err(invalid(
            "exclude",
            format!(r#"path must be absolute: "{e}""#),
        )),
        None => Ok(entries),
    }
}

fn check_markers(raw: &[String]) -> Result<Vec<MarkerChar>, ConfigError> {
    let mut out = Vec::new();
    for s in raw {
        let m = MarkerChar::ALL
            .into_iter()
            .find(|m| m.as_str() == s)
            .ok_or_else(|| invalid("markers", format!(r#"unsupported marker "{s}""#)))?;
        if out.contains(&m) {
            return Err(invalid("markers", format!(r#"duplicate marker "{s}""#)));
        }
        out.push(m);
    }
    Ok(out)
}

fn hex(key: &str, value: &str, len: usize) -> Result<String, ConfigError> {
    match value.len() == len && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        true => Ok(value.to_ascii_lowercase()),
        false => Err(invalid(
            key,
            format!(r#"expected {len} hex digits, got "{value}""#),
        )),
    }
}

/// Letters in the sense of Unicode category L, as in fachl. Spezifikation 3.
fn is_letter(c: char) -> bool {
    c.general_category_group() == GeneralCategoryGroup::Letter
}

fn check_word(stem: String, w: RawWord) -> Result<(String, Word), ConfigError> {
    let key = format!("words.{stem}");
    let letters = |field: &str, v: &str| match !v.is_empty() && v.chars().all(is_letter) {
        true => Ok(()),
        false => Err(invalid(
            &format!("{key}{field}"),
            format!(r#"only letters allowed, got "{v}""#),
        )),
    };
    letters("", &stem)?;
    if !stem.starts_with(char::is_uppercase) {
        return Err(invalid(
            &key,
            "stem must start with an uppercase letter".into(),
        ));
    }
    letters(".singular", &w.singular)?;
    letters(".plural", &w.plural)?;
    if let Some(c) = &w.compound {
        letters(".compound", c)?;
    }
    let compound = w.compound.unwrap_or_else(|| w.plural.clone());
    Ok((
        stem,
        Word {
            singular: w.singular,
            plural: w.plural,
            compound,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Config, ConfigError> {
        Config::parse(text, Path::new("/c/config.toml"))
    }

    fn err(text: &str) -> String {
        parse(text).unwrap_err().to_string()
    }

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let c = Config::load(Path::new("/nonexistent/nostar/config.toml")).unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn example_from_technik() {
        let c = parse(
            r#"
mode = "standard"
families = ["Adwaita Sans", "JetBrains Mono"]
exclude = []
markers = ["*", ":", "_", "·", "/", "/-"]
binnen_i = true
fallback = "cut"
participles = false
skip_monospace = false

[source]
commit = "febca802f868c4ce968bbae70c099cdbc0454e18"
sha256 = "6330c126964385e84fb5f01c47893a2a75a65b163f6be4bd19bc2448517f73b3"

[words.Freund]
singular = "Freund"
plural = "Freunde"
compound = "Freundes"
"#,
        )
        .unwrap();
        assert_eq!(c.families, ["Adwaita Sans", "JetBrains Mono"]);
        assert_eq!(c.markers, MarkerChar::ALL);
        assert_eq!(c.words["Freund"].compound, "Freundes");
    }

    #[test]
    fn compound_defaults_to_plural() {
        let c = parse("[words.Kund]\nsingular = \"Kunde\"\nplural = \"Kunden\"\n").unwrap();
        assert_eq!(c.words["Kund"].compound, "Kunden");
    }

    #[test]
    fn unknown_key_with_line() {
        assert_eq!(
            err("mode = \"all\"\n\n\nfallbak = \"cut\"\n"),
            r#"/c/config.toml:4: unknown key "fallbak""#
        );
        assert_eq!(
            err("[source]\ncommit = \"x\"\n"),
            r#"/c/config.toml:1: missing key "sha256""#
        );
    }

    #[test]
    fn wrong_type_is_a_parse_error() {
        assert!(err("binnen_i = \"yes\"\n").starts_with("/c/config.toml:1: "));
    }

    #[test]
    fn enum_values() {
        assert_eq!(
            err("fallback = \"abschneiden\""),
            r#"fallback: expected "cut", "marker" or "off", got "abschneiden""#
        );
        assert_eq!(
            err("mode = \"alle\""),
            r#"mode: expected "standard" or "all", got "alle""#
        );
        assert_eq!(
            parse("mode = \"all\"\nfallback = \"off\"")
                .unwrap()
                .fallback,
            Fallback::Off
        );
    }

    #[test]
    fn markers() {
        assert_eq!(
            err(r#"markers = ["*", "+"]"#),
            r#"markers: unsupported marker "+""#
        );
        assert_eq!(
            err(r#"markers = ["*", "*"]"#),
            r#"markers: duplicate marker "*""#
        );
        assert_eq!(parse("markers = []").unwrap().markers, []);
        assert_eq!(
            parse(r#"markers = ["/-", ":"]"#).unwrap().markers,
            [MarkerChar::SlashHyphen, MarkerChar::Colon]
        );
    }

    #[test]
    fn families_and_exclude() {
        assert_eq!(
            err(r#"families = ["Noto Sans", " "]"#),
            "families: empty entry"
        );
        assert_eq!(
            err(r#"exclude = ["fonts/x.ttf"]"#),
            r#"exclude: path must be absolute: "fonts/x.ttf""#
        );
        assert_eq!(
            parse(r#"exclude = ["Noto Sans", "/usr/share/fonts/x.ttf"]"#)
                .unwrap()
                .exclude
                .len(),
            2
        );
    }

    #[test]
    fn source_hex() {
        assert_eq!(
            err("[source]\ncommit = \"abc\"\nsha256 = \"x\"\n"),
            r#"source.commit: expected 40 hex digits, got "abc""#
        );
        let c = parse(&format!(
            "[source]\ncommit = \"{}\"\nsha256 = \"{}\"\n",
            "A".repeat(40),
            "0".repeat(64)
        ))
        .unwrap();
        assert_eq!(c.source.commit, "a".repeat(40));
    }

    #[test]
    fn words_checked() {
        assert_eq!(
            err("[words.freund]\nsingular = \"Freund\"\nplural = \"Freunde\"\n"),
            "words.freund: stem must start with an uppercase letter"
        );
        assert_eq!(
            err("[words.Freund]\nsingular = \"Freund\"\nplural = \"Freunde!\"\n"),
            r#"words.Freund.plural: only letters allowed, got "Freunde!""#
        );
        // TOML bare keys are ASCII only: stems with umlauts need quotes.
        assert!(parse("[words.\"Ärzt\"]\nsingular = \"Arzt\"\nplural = \"Ärzte\"\n").is_ok());
        assert!(parse("[words.Ärzt]\nsingular = \"Arzt\"\nplural = \"Ärzte\"\n").is_err());
    }
}
