//! `rules.tsv` (TECHNIK.md 8.6).

use std::fs;
use std::io;
use std::path::Path;

use super::{Left, Marker, Right, Rule, RulesError, Stage};
use crate::config::{Config, MarkerChar};
use crate::fsutil::{FsError, sha256_hex, write_atomic};
use crate::lexicon::Origin;

const COLUMNS: &str = "stage\tleft\tstem\tmarker\tsuffix\tright\ttarget\torigin";

/// SHA-256 of the settings that change the rules: markers, Binnen-I, fallback, own words.
pub fn config_hash(config: &Config) -> String {
    let mut s = String::new();
    let markers: Vec<&str> = MarkerChar::ALL
        .iter()
        .filter(|m| config.markers.contains(m))
        .map(|m| m.as_str())
        .collect();
    s.push_str(&format!("markers={}\n", markers.join(" ")));
    s.push_str(&format!("binnen_i={}\n", config.binnen_i));
    s.push_str(&format!("fallback={:?}\n", config.fallback));
    for (stem, w) in &config.words {
        s.push_str(&format!(
            "word={stem}\t{}\t{}\t{}\n",
            w.singular, w.plural, w.compound
        ));
    }
    sha256_hex(s.as_bytes())
}

fn header(config: &Config) -> String {
    format!(
        "# nostar rules v1 lexicon={} config={}",
        config.source.sha256,
        config_hash(config)
    )
}

pub fn write(path: &Path, rules: &[Rule], config: &Config) -> Result<(), FsError> {
    let mut s = format!("{}\n{COLUMNS}\n", header(config));
    for r in rules {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            r.stage as u8,
            left_str(r.left),
            r.stem,
            marker_str(r.marker),
            r.suffix,
            right_str(r.right),
            r.target,
            r.origin
        ));
    }
    write_atomic(path, s.as_bytes())
}

/// Reads the rules and checks that they were built from the current dictionary and settings.
pub fn read(path: &Path, config: &Config) -> Result<Vec<Rule>, RulesError> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(RulesError::Missing(path.to_path_buf()));
        }
        Err(e) => return Err(FsError::io("read", path)(e).into()),
    };
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("");
    let fields: Vec<&str> = first.split(' ').collect();
    let value = |name: &str| {
        fields
            .iter()
            .find_map(|f| f.strip_prefix(name)?.strip_prefix('='))
    };
    if !first.starts_with("# nostar rules v1 ") {
        return Err(RulesError::Parse {
            line: 1,
            message: format!("expected header, got {first:?}"),
        });
    }
    if value("lexicon") != Some(config.source.sha256.as_str()) {
        return Err(RulesError::Stale("dictionary"));
    }
    if value("config") != Some(config_hash(config).as_str()) {
        return Err(RulesError::Stale("config"));
    }
    let mut rules = Vec::new();
    for (i, line) in lines.enumerate() {
        let n = i + 2;
        if n == 2 && line == COLUMNS {
            continue;
        }
        rules.push(parse_line(line).map_err(|message| RulesError::Parse { line: n, message })?);
    }
    Ok(rules)
}

fn parse_line(line: &str) -> Result<Rule, String> {
    let c: Vec<&str> = line.split('\t').collect();
    if c.len() != 8 {
        return Err(format!("expected 8 columns, got {}", c.len()));
    }
    let bad = |col: &str, v: &str| format!("{col}: unknown value {v:?}");
    let stage = match c[0] {
        "1" => Stage::Fixed,
        "2" => Stage::Lexicon,
        "4" => Stage::Ending,
        "5" => Stage::Fallback,
        v => return Err(bad("stage", v)),
    };
    let left = match c[1] {
        "any" => Left::Any,
        "boundary" => Left::Boundary,
        "letter" => Left::Letter,
        v => return Err(bad("left", v)),
    };
    let marker = match c[3] {
        "gender" => Marker::Gender,
        "gender-text" => Marker::GenderText,
        "slash-hyphen" => Marker::SlashHyphen,
        "binnen-i" => Marker::BinnenI,
        v => return Err(bad("marker", v)),
    };
    let right = match c[5] {
        "letter" => Right::Letter,
        "wordend" => Right::WordEnd,
        "any" => Right::Any,
        v => return Err(bad("right", v)),
    };
    let origin = match c[7] {
        "config" => Origin::Config,
        "builtin" => Origin::Builtin,
        v => match v.strip_prefix("entgendert@") {
            Some(commit) => Origin::Entgendert {
                commit: commit.to_string(),
            },
            None => return Err(bad("origin", v)),
        },
    };
    Ok(Rule {
        stage,
        left,
        stem: c[2].to_string(),
        marker,
        suffix: c[4].to_string(),
        right,
        target: c[6].to_string(),
        origin,
    })
}

fn left_str(l: Left) -> &'static str {
    match l {
        Left::Any => "any",
        Left::Boundary => "boundary",
        Left::Letter => "letter",
    }
}

fn right_str(r: Right) -> &'static str {
    match r {
        Right::Letter => "letter",
        Right::WordEnd => "wordend",
        Right::Any => "any",
    }
}

fn marker_str(m: Marker) -> &'static str {
    match m {
        Marker::Gender => "gender",
        Marker::GenderText => "gender-text",
        Marker::SlashHyphen => "slash-hyphen",
        Marker::BinnenI => "binnen-i",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{build, fixed};

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nostar-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn round_trip_and_stale_header() {
        let dir = scratch("store");
        let path = dir.join("rules.tsv");
        let mut rules = fixed::forms();
        rules.extend(fixed::adjective_endings());
        build::sort(&mut rules);
        let mut config = Config::default();
        write(&path, &rules, &config).unwrap();
        assert_eq!(read(&path, &config).unwrap(), rules);

        config.binnen_i = false;
        assert!(matches!(
            read(&path, &config),
            Err(RulesError::Stale("config"))
        ));
        config.binnen_i = true;
        config.source.sha256 = "0".repeat(64);
        assert!(matches!(
            read(&path, &config),
            Err(RulesError::Stale("dictionary"))
        ));
        assert!(matches!(
            read(&dir.join("none.tsv"), &config),
            Err(RulesError::Missing(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn families_do_not_change_the_hash() {
        let a = Config::default();
        let mut b = Config::default();
        b.families.push("Noto Sans".into());
        assert_eq!(config_hash(&a), config_hash(&b));
        b.markers.pop();
        assert_ne!(config_hash(&a), config_hash(&b));
    }
}
