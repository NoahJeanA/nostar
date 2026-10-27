//! Entgendert dictionary: download with curl, check SHA-256, parse (TECHNIK.md 7).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{self, is_word};
use crate::fsutil::{FsError, create_parent, sha256_hex, tmp_path};
use crate::lexicon::{EndingRule, Lexicon, Origin, Person};
use crate::paths::Paths;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("run curl for {url}")]
    Curl {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("curl exit status {status}: {url}")]
    CurlStatus {
        status: String,
        url: String,
        #[source]
        stderr: Option<CurlOutput>,
    },
    #[error(r#"dictionary missing: {0}, run "nostar data update""#)]
    Missing(PathBuf),
    #[error("sha256 mismatch for {file}: expected {expected}…, got {got}…")]
    Mismatch {
        file: String,
        expected: String,
        got: String,
    },
    #[error(r#"{file}: field "{field}" not found"#)]
    FieldMissing { file: String, field: &'static str },
    #[error(r#"{file}: field "{field}": invalid entry "{entry}""#)]
    Entry {
        file: String,
        field: &'static str,
        entry: String,
    },
    #[error(transparent)]
    Fs(#[from] FsError),
}

/// What curl wrote to stderr, shown as `caused by:`.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CurlOutput(String);

pub fn url(commit: &str) -> String {
    format!(
        "https://raw.githubusercontent.com/janduettmann/Entgendert/{commit}/extension/dictionary.js"
    )
}

/// `nostar data update`: downloads the configured version and moves it into place.
pub fn update(paths: &Paths, source: &config::Source) -> Result<PathBuf, SourceError> {
    let target = paths.dictionary(&source.commit);
    download(&url(&source.commit), &target, &source.sha256)?;
    Ok(target)
}

/// Downloads to `<target>.tmp`, then renames if the checksum matches; removes the tmp file otherwise.
fn download(url: &str, target: &Path, sha256: &str) -> Result<(), SourceError> {
    create_parent(target)?;
    let tmp = tmp_path(target);
    log::debug!("download {url}");
    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location"])
        .args(["--max-time", "60", "--output"])
        .arg(&tmp)
        .arg(url)
        .output()
        .map_err(|source| SourceError::Curl {
            url: url.to_string(),
            source,
        })?;
    if !output.status.success() {
        let _ = fs::remove_file(&tmp);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(SourceError::CurlStatus {
            status: output
                .status
                .code()
                .map_or_else(|| "none (signal)".to_string(), |c| c.to_string()),
            url: url.to_string(),
            stderr: (!stderr.is_empty()).then_some(CurlOutput(stderr)),
        });
    }
    let checked = fs::read(&tmp)
        .map_err(FsError::io("read", &tmp))
        .map_err(SourceError::from)
        .and_then(|data| check(target, &data, sha256).map(|()| data.len()));
    let size = match checked {
        Ok(size) => size,
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    };
    fs::rename(&tmp, target).map_err(FsError::io("rename", &tmp))?;
    log::info!("downloaded {}, sha256 ok", file_name(target));
    log::debug!("{} size={size}", target.display());
    Ok(())
}

/// Reads the downloaded dictionary of the configured version, checks it again and parses it.
pub fn load(paths: &Paths, source: &config::Source) -> Result<Lexicon, SourceError> {
    let path = paths.dictionary(&source.commit);
    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SourceError::Missing(path));
        }
        Err(e) => return Err(FsError::io("read", &path)(e).into()),
    };
    check(&path, &data, &source.sha256)?;
    let text = String::from_utf8(data)
        .map_err(|e| FsError::io("read", &path)(std::io::Error::other(e)))?;
    let lexicon = parse(&text, &file_name(&path), &source.commit)?;
    log::debug!(
        "{}: persons={} endings={} adjectival={}",
        file_name(&path),
        lexicon.persons.len(),
        lexicon.endings.len(),
        lexicon.adjectival.len()
    );
    Ok(lexicon)
}

fn check(path: &Path, data: &[u8], expected: &str) -> Result<(), SourceError> {
    let got = sha256_hex(data);
    if got == expected {
        return Ok(());
    }
    Err(SourceError::Mismatch {
        file: file_name(path),
        expected: expected.chars().take(8).collect(),
        got: got.chars().take(8).collect(),
    })
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Parses the four fields of `dictionary.js` (TECHNIK.md 7.2). `file` is only used in messages.
pub fn parse(text: &str, file: &str, commit: &str) -> Result<Lexicon, SourceError> {
    let origin = Origin::Entgendert {
        commit: commit.to_string(),
    };
    let entries = |field: &'static str| -> Result<Vec<&str>, SourceError> {
        let value = find_field(text, field).ok_or_else(|| SourceError::FieldMissing {
            file: file.to_string(),
            field,
        })?;
        Ok(value.split(';').filter(|e| !e.is_empty()).collect())
    };
    let invalid = |field: &'static str, entry: &str| SourceError::Entry {
        file: file.to_string(),
        field,
        entry: entry.to_string(),
    };

    let mut persons = Vec::new();
    for e in entries("irregular")? {
        let p = irregular(e, &origin).ok_or_else(|| invalid("irregular", e))?;
        persons.push(p);
    }
    for e in entries("regular")? {
        if !is_word(e) {
            return Err(invalid("regular", e));
        }
        persons.push(Person {
            stem: e.to_string(),
            singular: e.to_string(),
            plural: e.to_string(),
            compound: e.to_string(),
            origin: origin.clone(),
        });
    }
    let mut adjectival = Vec::new();
    for e in entries("adjectival")? {
        if !is_word(e) {
            return Err(invalid("adjectival", e));
        }
        adjectival.push(e.to_string());
    }
    let mut endings = Vec::new();
    for e in entries("rules")? {
        match ending(e) {
            Some(Some(rule)) => endings.push(rule),
            Some(None) => {}
            None => return Err(invalid("rules", e)),
        }
    }
    Ok(Lexicon {
        persons,
        endings,
        adjectival,
    })
}

/// Value of `<name>: "…"`; `name` must not be the tail of a longer key (`regular` in `irregular`).
fn find_field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}: \"");
    let mut from = 0;
    while let Some(i) = text[from..].find(&key) {
        let start = from + i;
        let prev = text[..start].chars().next_back();
        if !prev.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            let value = &text[start + key.len()..];
            return value.find('"').map(|end| &value[..end]);
        }
        from = start + key.len();
    }
    None
}

/// `stem,singular,plural[,dative[,a]]`; the dative is checked but not used (TECHNIK.md 7.3).
fn irregular(entry: &str, origin: &Origin) -> Option<Person> {
    let parts: Vec<&str> = entry.split(',').collect();
    if !(3..=5).contains(&parts.len()) || parts.get(4).is_some_and(|flag| *flag != "a") {
        return None;
    }
    let stem = parts[0];
    let singular = decode(stem, parts[1]);
    let plural = decode(&singular, parts[2]);
    let dative = decode(&plural, parts.get(3).copied().unwrap_or(""));
    [stem, &singular, &plural, &dative]
        .iter()
        .all(|w| is_word(w))
        .then(|| Person {
            stem: stem.to_string(),
            compound: plural.clone(),
            singular,
            plural,
            origin: origin.clone(),
        })
}

/// `ending=s,p,d` with the forms appended to the ending, or `ending=!` (no rule, `Some(None)`).
fn ending(entry: &str) -> Option<Option<EndingRule>> {
    let (ending, forms) = entry.split_once('=')?;
    if !is_word(ending) {
        return None;
    }
    if forms == "!" {
        return Some(None);
    }
    let parts: Vec<&str> = forms.split(',').collect();
    if parts.len() != 3 || !parts.iter().all(|p| p.is_empty() || is_word(p)) {
        return None;
    }
    Some(Some(EndingRule {
        ending: ending.to_string(),
        singular: format!("{ending}{}", parts[0]),
        plural: format!("{ending}{}", parts[1]),
    }))
}

/// Empty -> `base`; `+x` -> `base` + x; otherwise the code itself.
fn decode(base: &str, code: &str) -> String {
    match code.strip_prefix('+') {
        Some(rest) => format!("{base}{rest}"),
        None if code.is_empty() => base.to_string(),
        None => code.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/dictionary.js");
    const COMMIT: &str = "febca802f868c4ce968bbae70c099cdbc0454e18";

    fn person<'a>(lex: &'a Lexicon, stem: &str) -> &'a Person {
        lex.persons.iter().find(|p| p.stem == stem).unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nostar-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn decode_codes() {
        assert_eq!(decode("Ärzt", ""), "Ärzt");
        assert_eq!(decode("Kolleg", "+e"), "Kollege");
        assert_eq!(decode("Arzt", "Ärzte"), "Ärzte");
    }

    #[test]
    fn parses_fixture() {
        let lex = parse(FIXTURE, "dictionary.js", COMMIT).unwrap();
        let arzt = person(&lex, "Ärzt");
        assert_eq!(
            (
                arzt.singular.as_str(),
                arzt.plural.as_str(),
                arzt.compound.as_str()
            ),
            ("Arzt", "Ärzte", "Ärzte")
        );
        assert_eq!(
            arzt.origin,
            Origin::Entgendert {
                commit: COMMIT.into()
            }
        );
        // three parts: dative defaults to the plural
        assert_eq!(person(&lex, "Abiturient").plural, "Abiturienten");
        // five parts with the adjectival flag
        let beamt = person(&lex, "Beamt");
        assert_eq!(
            (beamt.singular.as_str(), beamt.plural.as_str()),
            ("Beamter", "Beamte")
        );
        let lehrer = person(&lex, "Lehrer");
        assert_eq!(
            (lehrer.singular.as_str(), lehrer.plural.as_str()),
            ("Lehrer", "Lehrer")
        );
        assert!(lex.adjectival.contains(&"Vorsitzende".to_string()));
        let eg = lex.endings.iter().find(|r| r.ending == "eg").unwrap();
        assert_eq!((eg.singular.as_str(), eg.plural.as_str()), ("ege", "egen"));
        // "ier=!" gives no rule
        assert!(lex.endings.iter().all(|r| r.ending != "ier"));
    }

    #[test]
    fn regular_is_not_found_inside_irregular() {
        let text = r#"irregular: "Ärzt,Arzt,Ärzte", regular: "Lehrer", adjectival: "", rules: """#;
        let lex = parse(text, "d.js", COMMIT).unwrap();
        let stems: Vec<_> = lex.persons.iter().map(|p| p.stem.as_str()).collect();
        assert_eq!(stems, ["Ärzt", "Lehrer"]);
    }

    #[test]
    fn errors_name_field_and_entry() {
        let err = |text: &str| {
            parse(text, "dictionary.js", COMMIT)
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            err(r#"regular: "", adjectival: "", rules: """#),
            r#"dictionary.js: field "irregular" not found"#
        );
        assert_eq!(
            err(r#"irregular: "Ärzt,Arzt", regular: "", adjectival: "", rules: """#),
            r#"dictionary.js: field "irregular": invalid entry "Ärzt,Arzt""#
        );
        assert_eq!(
            err(r#"irregular: "", regular: "Lehrer1", adjectival: "", rules: """#),
            r#"dictionary.js: field "regular": invalid entry "Lehrer1""#
        );
        assert_eq!(
            err(r#"irregular: "", regular: "", adjectival: "", rules: "eg=e,en""#),
            r#"dictionary.js: field "rules": invalid entry "eg=e,en""#
        );
    }

    #[test]
    fn download_checks_sha256() {
        let dir = scratch("download");
        let origin = dir.join("dictionary.js");
        fs::write(&origin, FIXTURE).unwrap();
        let url = format!("file://{}", origin.display());
        let target = dir.join("source/entgendert-febca802f868.js");

        let err = download(&url, &target, &"0".repeat(64)).unwrap_err();
        assert!(matches!(err, SourceError::Mismatch { .. }), "{err}");
        assert!(!target.exists());
        assert!(!tmp_path(&target).exists());

        download(&url, &target, &sha256_hex(FIXTURE.as_bytes())).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), FIXTURE);
        assert!(!tmp_path(&target).exists());

        let missing = format!("file://{}", dir.join("nothing.js").display());
        let err = download(&missing, &dir.join("x.js"), &"0".repeat(64)).unwrap_err();
        assert!(
            matches!(&err, SourceError::CurlStatus { status, stderr: Some(_), .. } if status == "37"),
            "{err:?}"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
