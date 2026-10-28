//! Candidates from fontconfig's font list, checked against fachl. Spezifikation 7.2
//! (TECHNIK.md 10.3).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use read_fonts::FontRef;

use super::glyphs::{self, Glyphs, Unsuitable};
use crate::config::Config;
use crate::paths::Paths;
use crate::rules::Markers;

/// One file from `fc-list` or `fc-match`.
#[derive(Debug, Clone)]
pub struct Listed {
    pub file: PathBuf,
    pub family: String,
    pub mono: bool,
}

/// Reason in the `apply` summary, in the order of the checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Skip {
    Collection,
    Format,
    Excluded,
    Monospace,
    Unreadable,
    NoLatinLetters,
    NoGenderMarker,
    SharedGlyphs,
}

impl Skip {
    pub fn label(self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Format => "format",
            Self::Excluded => "excluded",
            Self::Monospace => "monospace",
            Self::Unreadable => "unreadable",
            Self::NoLatinLetters => "no latin letters",
            Self::NoGenderMarker => "no gender marker",
            Self::SharedGlyphs => "shared glyphs",
        }
    }
}

#[derive(Debug, Default)]
pub struct Selection {
    /// Originals to patch, sorted by path.
    pub fonts: Vec<PathBuf>,
    pub skipped: Vec<(PathBuf, Skip)>,
}

/// Copies under the nostar font directory stand for their original (fontconfig hides the
/// originals after the first `apply`); copies whose original is gone are left out.
pub fn select(listed: &[Listed], config: &Config, paths: &Paths) -> Selection {
    let mut originals: BTreeMap<PathBuf, &Listed> = BTreeMap::new();
    for l in listed {
        let path = match paths.original_of(&l.file) {
            Some(original) if original.exists() => original,
            Some(_) => continue,
            None => l.file.clone(),
        };
        originals.entry(path).or_insert(l);
    }
    let markers = Markers::from_config(config);
    let mut selection = Selection::default();
    for (path, l) in originals {
        match check(&path, l, config, &markers) {
            Ok(()) => selection.fonts.push(path),
            Err(skip) => {
                log::debug!("skipped {}: {}", path.display(), skip.label());
                selection.skipped.push((path, skip));
            }
        }
    }
    selection
}

fn check(path: &Path, l: &Listed, config: &Config, markers: &Markers) -> Result<(), Skip> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "ttc" | "otc" => return Err(Skip::Collection),
        "ttf" | "otf" => {}
        _ => return Err(Skip::Format),
    }
    let excluded = config
        .exclude
        .iter()
        .any(|e| *e == l.family || Path::new(e) == path);
    if excluded {
        return Err(Skip::Excluded);
    }
    if config.skip_monospace && l.mono {
        return Err(Skip::Monospace);
    }
    let data = std::fs::read(path).map_err(|_| Skip::Unreadable)?;
    let font = FontRef::new(&data).map_err(|_| Skip::Unreadable)?;
    let g = Glyphs::new(&font).map_err(|_| Skip::Unreadable)?;
    glyphs::check(&g, markers).map_err(|e| match e {
        Unsuitable::FewLetters(_) | Unsuitable::MissingLetter(_) => Skip::NoLatinLetters,
        Unsuitable::NoMarker => Skip::NoGenderMarker,
        Unsuitable::SharedGlyph(..) => Skip::SharedGlyphs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_map_to_originals_and_reasons() {
        let dir = std::env::temp_dir().join(format!("nostar-test-{}-select", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let noto = dir.join("NotoSans.ttf");
        std::fs::write(
            &noto,
            include_bytes!("../../tests/fixtures/NotoSans-Latin.ttf"),
        )
        .unwrap();
        let broken = dir.join("Broken.ttf");
        std::fs::write(&broken, b"not a font").unwrap();
        let home = dir.join("home");
        let paths = Paths::from_vars_for_tests(&home);
        let listed = |file: PathBuf, family: &str| Listed {
            file,
            family: family.into(),
            mono: false,
        };
        let config = Config {
            exclude: vec!["Excluded Family".into()],
            ..Config::default()
        };
        let sel = select(
            &[
                listed(paths.patched_font(&noto), "Noto Sans"),
                listed(noto.clone(), "Noto Sans"),
                listed(paths.patched_font(&dir.join("gone.ttf")), "Gone"),
                listed(dir.join("x.ttc"), "C"),
                listed(dir.join("x.pcf"), "P"),
                listed(dir.join("e.ttf"), "Excluded Family"),
                listed(broken.clone(), "B"),
            ],
            &config,
            &paths,
        );
        assert_eq!(sel.fonts, [noto]);
        let mut reasons: Vec<Skip> = sel.skipped.iter().map(|s| s.1).collect();
        reasons.sort();
        assert_eq!(
            reasons,
            [
                Skip::Collection,
                Skip::Format,
                Skip::Excluded,
                Skip::Unreadable
            ]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
