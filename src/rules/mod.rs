//! Rule list (TECHNIK.md 8): types, test cases and errors.

pub mod build;
pub mod edit;
pub mod engine;
pub mod fixed;
pub mod store;

use std::path::PathBuf;

use crate::config::{Config, Fallback, MarkerChar};
use crate::fsutil::FsError;
use crate::lexicon::Origin;

#[derive(Debug, thiserror::Error)]
pub enum RulesError {
    #[error(r#"rules.tsv missing, run "nostar rules build""#)]
    Missing(PathBuf),
    #[error(r#"rules.tsv built from other {0}, run "nostar rules build""#)]
    Stale(&'static str),
    #[error("rules.tsv:{line}: {message}")]
    Parse { line: usize, message: String },
    #[error("cases.tsv:{line}: {message}")]
    CasesFile { line: usize, message: String },
    #[error("{failed} of {total} cases differ{details}")]
    CasesDiffer {
        failed: usize,
        total: usize,
        details: String,
    },
    #[error(transparent)]
    Fs(#[from] FsError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Fixed = 1,
    Lexicon = 2,
    Ending = 4,
    Fallback = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Marker {
    /// One of the active markers `* : _ · /`.
    Gender,
    /// One of the active markers `* : ·`; `/` and `_` are common in paths and code
    /// (stage 1 adjective ending and stage 5).
    GenderText,
    /// A marker followed by `-`.
    SlashHyphen,
    /// No marker character; the suffix starts with `I`.
    BinnenI,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Left {
    Any,
    /// Word boundary before the stem; an exception otherwise (stage 1).
    Boundary,
    /// A letter must precede; no match otherwise.
    Letter,
}

/// Declared in the order of TECHNIK.md 8.4, rule 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Right {
    /// A letter must follow; no match otherwise.
    Letter,
    /// No letter may follow; an exception otherwise.
    WordEnd,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub stage: Stage,
    pub left: Left,
    /// Exact characters before the marker.
    pub stem: String,
    pub marker: Marker,
    pub suffix: String,
    pub right: Right,
    /// Replacement for stem + marker + suffix.
    pub target: String,
    pub origin: Origin,
}

/// One position of a rule source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atom {
    Char(char),
    Marker(Class),
}

/// Characters a marker position accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Gender,
    Text,
    /// First position of `SlashHyphen`: the gender class plus `/`.
    SlashFirst,
}

/// All characters that can be gender markers (fachl. Spezifikation 4.1).
pub const MARKER_CHARS: [char; 5] = ['*', ':', '_', '·', '/'];

impl Marker {
    fn atoms(self) -> Vec<Atom> {
        match self {
            Self::Gender => vec![Atom::Marker(Class::Gender)],
            Self::GenderText => vec![Atom::Marker(Class::Text)],
            Self::SlashHyphen => vec![Atom::Marker(Class::SlashFirst), Atom::Char('-')],
            Self::BinnenI => vec![],
        }
    }

    /// How the marker is written in messages and test texts.
    fn sample(self) -> &'static str {
        match self {
            Self::Gender | Self::GenderText => "*",
            Self::SlashHyphen => "/-",
            Self::BinnenI => "",
        }
    }
}

impl Rule {
    pub fn pattern(&self) -> Vec<Atom> {
        let mut p: Vec<Atom> = self.stem.chars().map(Atom::Char).collect();
        p.extend(self.marker.atoms());
        p.extend(self.suffix.chars().map(Atom::Char));
        p
    }

    pub fn stem_len(&self) -> usize {
        self.stem.chars().count()
    }

    /// Length of stem + marker + suffix in characters (TECHNIK.md 8.4).
    pub fn source_len(&self) -> usize {
        self.stem_len() + self.marker.atoms().len() + self.suffix.chars().count()
    }

    /// Source as it appears in text, with `*` for the marker class: `Ärzt*innen`.
    pub fn source(&self) -> String {
        format!("{}{}{}", self.stem, self.marker.sample(), self.suffix)
    }
}

/// Active markers from the configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markers {
    /// Active characters of `* : _ · /`.
    pub gender: Vec<char>,
    pub slash_hyphen: bool,
}

impl Markers {
    pub fn from_config(config: &Config) -> Self {
        let mut gender = Vec::new();
        let mut slash_hyphen = false;
        for m in &config.markers {
            match m {
                MarkerChar::SlashHyphen => slash_hyphen = true,
                other => gender.extend(other.as_str().chars()),
            }
        }
        Self {
            gender,
            slash_hyphen,
        }
    }

    pub fn accepts(&self, class: Class, c: char) -> bool {
        match class {
            Class::Gender => self.gender.contains(&c),
            Class::Text => c != '/' && c != '_' && self.gender.contains(&c),
            Class::SlashFirst => c == '/' || self.gender.contains(&c),
        }
    }
}

/// One line of `tests/cases.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub input: String,
    pub expected: String,
}

/// Cases that apply to a configuration (TECHNIK.md 8.7).
#[derive(Debug)]
pub struct Cases {
    pub cases: Vec<Case>,
    pub skipped: usize,
}

const CASES: &str = include_str!("../../tests/cases.tsv");

pub fn cases(config: &Config) -> Result<Cases, RulesError> {
    parse_cases(CASES, config)
}

fn parse_cases(text: &str, config: &Config) -> Result<Cases, RulesError> {
    let markers = Markers::from_config(config);
    let mut cases = Vec::new();
    let mut skipped = 0;
    for (i, line) in text.lines().enumerate() {
        let err = |message: String| RulesError::CasesFile {
            line: i + 1,
            message,
        };
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if !(2..=3).contains(&cols.len()) {
            return Err(err(format!("expected 2 or 3 columns, got {}", cols.len())));
        }
        let input = unescape(cols[0]).map_err(&err)?;
        let expected = unescape(cols[1]).map_err(&err)?;
        let needs = cols.get(2).copied().unwrap_or("");
        let mut applies = true;
        for need in needs.split(',').filter(|n| !n.is_empty()) {
            applies &= match need.trim() {
                "binnen_i" => config.binnen_i,
                "fallback=cut" => config.fallback == Fallback::Cut,
                "fallback=marker" => config.fallback == Fallback::Marker,
                "fallback=off" => config.fallback == Fallback::Off,
                other => return Err(err(format!(r#"unknown need "{other}""#))),
            };
        }
        if input != expected && !markers_active(&input, &markers) {
            applies = false;
        }
        let plain: String = input
            .chars()
            .filter(|c| *c != '\u{AD}')
            .collect::<String>()
            .to_lowercase();
        if config
            .words
            .keys()
            .any(|stem| plain.contains(&stem.to_lowercase()))
        {
            applies = false;
        }
        if applies {
            cases.push(Case { input, expected });
        } else {
            skipped += 1;
        }
    }
    Ok(Cases { cases, skipped })
}

fn markers_active(input: &str, markers: &Markers) -> bool {
    let chars: Vec<char> = input.chars().collect();
    chars.iter().enumerate().all(|(i, c)| {
        if !MARKER_CHARS.contains(c) {
            return true;
        }
        if chars.get(i + 1) == Some(&'-') {
            markers.slash_hyphen && (*c == '/' || markers.gender.contains(c))
        } else {
            markers.gender.contains(c)
        }
    })
}

/// `\u{AD}` stands for a character that cannot be seen in the file.
fn unescape(s: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("\\u{") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 3..];
        let end = tail
            .find('}')
            .ok_or_else(|| format!(r#"unterminated escape in "{s}""#))?;
        let c = u32::from_str_radix(&tail[..end], 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| format!(r#"invalid escape in "{s}""#))?;
        out.push(c);
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Case whose result differs from the expected display.
#[derive(Debug)]
pub struct Failure {
    pub input: String,
    pub expected: String,
    pub got: String,
}

pub fn check(set: &engine::RuleSet, cases: &[Case]) -> Vec<Failure> {
    cases
        .iter()
        .filter_map(|c| {
            let got = set.apply(&c.input).text;
            (got != c.expected).then(|| Failure {
                input: c.input.clone(),
                expected: c.expected.clone(),
                got,
            })
        })
        .collect()
}

/// Up to ten failures as `caused by`-style lines below the summary.
pub fn describe_failures(failures: &[Failure]) -> String {
    let mut s = String::new();
    for f in failures.iter().take(10) {
        s.push_str(&format!(
            "\n  {:?}: expected {:?}, got {:?}",
            f.input, f.expected, f.got
        ));
    }
    if failures.len() > 10 {
        s.push_str(&format!("\n  … {} more", failures.len() - 10));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_soft_hyphen() {
        assert_eq!(unescape(r"Leh\u{AD}rer").unwrap(), "Leh\u{AD}rer");
        assert!(unescape(r"x\u{zz}").is_err());
    }

    #[test]
    fn cases_skip_by_config() {
        let text = "Ärzt:innen\tÄrzte\nx:y\tx:y\nFoo*innen\tFoo\tfallback=cut\nÄrzt*in\tArzt\n";
        let mut config = Config::default();
        assert_eq!(parse_cases(text, &config).unwrap().cases.len(), 4);
        config.markers.retain(|m| *m != MarkerChar::Colon);
        config.fallback = Fallback::Off;
        let c = parse_cases(text, &config).unwrap();
        assert_eq!((c.cases.len(), c.skipped), (2, 2));
        assert!(matches!(
            parse_cases("a\tb\tnope\n", &config),
            Err(RulesError::CasesFile { line: 1, .. })
        ));
    }

    #[test]
    fn source_and_pattern() {
        let r = Rule {
            stage: Stage::Lexicon,
            left: Left::Any,
            stem: "Kolleg".into(),
            marker: Marker::SlashHyphen,
            suffix: "innen".into(),
            right: Right::Any,
            target: "Kollegen".into(),
            origin: Origin::Builtin,
        };
        assert_eq!(r.source(), "Kolleg/-innen");
        assert_eq!(r.source_len(), 13);
        assert_eq!(r.pattern()[6], Atom::Marker(Class::SlashFirst));
    }
}
