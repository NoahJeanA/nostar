//! Check of a patched font with harfrust (TECHNIK.md 9.6).

use std::collections::HashMap;

use harfrust::{Buffer, ShapeOptions, ShaperFont};
use write_fonts::types::GlyphId16;

use super::FontError;
use crate::rules::Case;

/// Text without gender forms that must shape identically in original and copy:
/// German with umlauts and feminine forms without marker, other scripts, code, URLs, times.
pub const UNCHANGED: &[&str] = &[
    "Die Ärztin und der Arzt trafen die Bäuerinnen und Bauern in Köln.",
    "Lehrerinnen und Lehrer, Kolleginnen und Kollegen, Bürgerinnen und Bürger",
    "Fünf Brüder übten öffentlich: Größe, Übermaß, Öl, Äpfel, süß und früh.",
    "Influenzaimpfung, Kaffeefilter, flache Schiffe, offizielle Ausflüge",
    "Ihre Anfrage wird Ihnen am Montag beantwortet. Im Internet ist alles anders.",
    "LinkedIn, PlugIn, YouTube, InDesign, iPhone, WhatsApp, BahnCard",
    "Studierende, Mitarbeitende und Lehrende der Universität",
    "The quick brown fox jumps over the lazy dog; official office affairs.",
    "Ο γρήγορος καφέ αλεπού πηδά πάνω από τον τεμπέλη σκύλο.",
    "Съешь же ещё этих мягких французских булок да выпей чаю.",
    "fn main() { let x = a * b; let y = size*n; println!(\"{}\", x / 2); }",
    "if (ptr->in && is_in(x)) { fade_in(); opt_in = check_in || built_in; }",
    "SELECT * FROM users WHERE name LIKE 'in%'; **kwargs, *args, a[e:n]",
    "https://example.org/a/b?id=5 and C:\\Users\\in /usr/share/fonts",
    "Termin 10:00–12:30, Ratio 16:9, 5 * 3 = 15, 5*3, x*y, *wichtig*, Fußnote*",
    "der/die/das, er/sie/es, die*der, sie*er, display:inline, Note: in",
];

pub struct Shaper {
    font: harfrust::Font,
}

impl Shaper {
    pub fn new(data: &[u8]) -> Result<Self, FontError> {
        let font = harfrust::Font::new(data.to_vec(), 0)
            .ok_or_else(|| FontError::Verify("load font for shaping".into()))?;
        Ok(Self { font })
    }

    /// Glyph ids and advances.
    pub fn shape(&self, text: &str) -> Result<Vec<(u32, i32)>, FontError> {
        let sf = ShaperFont::new(&self.font);
        let mut buf = Buffer::new();
        buf.push_str(text);
        buf.guess_segment_properties();
        harfrust::shape(&sf, &mut buf, ShapeOptions::new())
            .map_err(|e| FontError::Verify(format!("shape {text:?}: {e}")))?;
        Ok(buf
            .glyph_infos()
            .iter()
            .zip(buf.glyph_positions())
            .map(|(i, p)| (i.glyph_id, p.x_advance))
            .collect())
    }
}

/// Every case: input with the copy = expected display with the original; the unchanged
/// corpus: identical in both. The first difference is the error.
pub fn verify(
    original: &[u8],
    patched: &[u8],
    cases: &[Case],
    reverse: &HashMap<GlyphId16, char>,
) -> Result<(), FontError> {
    let orig = Shaper::new(original)?;
    let copy = Shaper::new(patched)?;
    let text = |glyphs: &[(u32, i32)]| -> String {
        glyphs
            .iter()
            .filter(|(_, advance)| *advance != 0)
            .map(|(g, _)| {
                u16::try_from(*g)
                    .ok()
                    .and_then(|g| reverse.get(&GlyphId16::new(g)).copied())
                    .unwrap_or('?')
            })
            .collect()
    };
    for c in cases {
        let got = copy.shape(&c.input)?;
        if got != orig.shape(&c.expected)? {
            return Err(FontError::Verify(format!(
                "{:?} shaped as {:?}, expected {:?}",
                c.input,
                text(&got),
                c.expected
            )));
        }
    }
    for line in UNCHANGED {
        let got = copy.shape(line)?;
        if got != orig.shape(line)? {
            return Err(FontError::Verify(format!(
                "unchanged text {line:?} shaped as {:?}",
                text(&got)
            )));
        }
    }
    Ok(())
}
