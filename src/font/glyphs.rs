//! Characters -> glyphs, the letter set and the glyph checks of fachl. Spezifikation 7.2.

use std::collections::{BTreeSet, HashMap};

use read_fonts::tables::cmap::PlatformId;
use read_fonts::{FontRef, TableProvider};
use write_fonts::types::GlyphId16;

use super::FontError;
use crate::config::is_letter;
use crate::rules::Markers;

/// Letters every patched font needs (fachl. Spezifikation 7.2).
const REQUIRED: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZäöüÄÖÜß";

/// Fewer letters than this: symbol, icon or other-script font.
const MIN_LETTERS: usize = 20;

pub struct Glyphs {
    cmap: HashMap<char, GlyphId16>,
    /// Glyphs of all letters in the sense of fachl. Spezifikation 3.
    pub letters: BTreeSet<GlyphId16>,
}

impl Glyphs {
    pub fn new(font: &FontRef) -> Result<Self, FontError> {
        let cmap_table = font
            .cmap()
            .map_err(|e| FontError::Read(format!("cmap: {e}")))?;
        let mut cmap = HashMap::new();
        let mut letters = BTreeSet::new();
        for record in cmap_table.encoding_records() {
            // Only Unicode subtables; a Mac Roman table maps byte codes, not characters.
            let unicode = matches!(
                (record.platform_id(), record.encoding_id()),
                (PlatformId::Unicode, _) | (PlatformId::Windows, 1 | 10)
            );
            if !unicode {
                continue;
            }
            let Ok(sub) = record.subtable(cmap_table.offset_data()) else {
                continue;
            };
            for (cp, gid) in sub.iter() {
                let (Some(c), Ok(g)) = (char::from_u32(cp), u16::try_from(gid.to_u32())) else {
                    continue;
                };
                let g = GlyphId16::new(g);
                cmap.entry(c).or_insert(g);
                if is_letter(c) {
                    letters.insert(g);
                }
            }
        }
        Ok(Self { cmap, letters })
    }

    pub fn get(&self, c: char) -> Option<GlyphId16> {
        self.cmap.get(&c).copied()
    }

    pub fn has(&self, c: char) -> bool {
        self.cmap.contains_key(&c)
    }

    /// Number of distinct letters with a glyph.
    pub fn letter_count(&self) -> usize {
        self.cmap.keys().filter(|c| is_letter(**c)).count()
    }

    /// Active markers the font has glyphs for; rules for missing ones are left out (7.2).
    pub fn markers(&self, active: &Markers) -> Markers {
        Markers {
            gender: active
                .gender
                .iter()
                .copied()
                .filter(|c| self.has(*c))
                .collect(),
            slash_hyphen: active.slash_hyphen && self.has('/') && self.has('-'),
        }
    }

    /// A gender marker, `-` or `/` whose glyph is also the glyph of a letter, of U+00AD or of
    /// another of these characters; the GSUB classes need them apart (fachl. Spezifikation 7.2).
    fn shared(&self, markers: &Markers) -> Option<(char, char)> {
        let mut own = markers.gender.clone();
        own.push('-');
        if markers.slash_hyphen {
            own.push('/');
        }
        own.sort();
        own.dedup();
        let mut by_glyph = HashMap::new();
        for c in own {
            if let Some(g) = self.get(c)
                && let Some(other) = by_glyph.insert(g, c)
            {
                return Some((other, c));
            }
        }
        let mut others: Vec<(char, GlyphId16)> = self
            .cmap
            .iter()
            .filter(|(c, _)| is_letter(**c) || **c == '\u{AD}')
            .map(|(c, g)| (*c, *g))
            .collect();
        others.sort();
        others
            .into_iter()
            .find_map(|(c, g)| by_glyph.get(&g).map(|own| (*own, c)))
    }

    /// One character for a glyph, to describe shaping results in messages.
    pub fn reverse(&self) -> HashMap<GlyphId16, char> {
        let mut rev = HashMap::new();
        let mut chars: Vec<(&char, &GlyphId16)> = self.cmap.iter().collect();
        chars.sort();
        for (c, g) in chars {
            rev.entry(*g).or_insert(*c);
        }
        rev
    }
}

/// Why a font is not patched (fachl. Spezifikation 7.2, glyph part).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unsuitable {
    #[error("{0} letters")]
    FewLetters(usize),
    #[error("no glyph for U+{:04X}", u32::from(*.0))]
    MissingLetter(char),
    #[error("no gender marker")]
    NoMarker,
    #[error("U+{:04X} and U+{:04X} share a glyph", u32::from(*.0), u32::from(*.1))]
    SharedGlyph(char, char),
}

pub fn check(glyphs: &Glyphs, active: &Markers) -> Result<(), Unsuitable> {
    let count = glyphs.letter_count();
    if count < MIN_LETTERS {
        return Err(Unsuitable::FewLetters(count));
    }
    if let Some(c) = REQUIRED.chars().find(|c| !glyphs.has(*c)) {
        return Err(Unsuitable::MissingLetter(c));
    }
    let markers = glyphs.markers(active);
    if markers.gender.is_empty() && !markers.slash_hyphen {
        return Err(Unsuitable::NoMarker);
    }
    if let Some((a, b)) = glyphs.shared(&markers) {
        return Err(Unsuitable::SharedGlyph(a, b));
    }
    Ok(())
}
