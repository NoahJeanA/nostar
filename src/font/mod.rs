//! Patch of one font: read -> compile -> write -> verify (TECHNIK.md 9).
//! Works on bytes; reading and writing files is the caller's job.

pub mod compile;
pub mod glyphs;
pub mod gsub;
pub mod verify;

use read_fonts::FontRef;

use crate::config::Config;
use crate::plan::Plan;
use crate::rules::{self, Markers};

/// More growth than this is an error (fachl. Spezifikation 8.3).
const MAX_GROWTH: usize = 5 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("read: {0}")]
    Read(String),
    /// The font does not meet fachl. Spezifikation 7.2.
    #[error("{0}")]
    NotSuitable(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("compile: {0}")]
    Compile(String),
    #[error("write: {0}")]
    Write(String),
    #[error("size +{0} KiB exceeds 5 MiB")]
    TooLarge(usize),
    #[error("verify: {0}")]
    Verify(String),
}

/// A patched font and figures for the log.
pub struct Patched {
    pub data: Vec<u8>,
    pub lookups: usize,
    pub rules: usize,
    /// GSUB rules left out for missing glyphs.
    pub dropped: usize,
}

/// Patches `original` with the plan and checks the result against the test cases
/// that apply to the markers the font has.
pub fn patch(original: &[u8], plan: &Plan, config: &Config) -> Result<Patched, FontError> {
    let font = FontRef::new(original).map_err(|e| FontError::Read(e.to_string()))?;
    let g = glyphs::Glyphs::new(&font)?;
    let active = Markers::from_config(config);
    glyphs::check(&g, &active).map_err(FontError::NotSuitable)?;
    let markers = g.markers(&active);
    let compiled = compile::compile(plan, &g, &markers)?;
    let lookups = compiled.lookups.len();
    log::debug!(
        "lookups={lookups} rules={} dropped={}",
        compiled.rules,
        compiled.dropped
    );
    let data = gsub::insert(&font, compiled.lookups)?;
    let growth = data.len().saturating_sub(original.len());
    if growth > MAX_GROWTH {
        return Err(FontError::TooLarge(growth / 1024));
    }
    let mut cases = rules::cases_with(config, &markers)
        .map_err(|e| FontError::Verify(e.to_string()))?
        .cases;
    // A case with a character the font has no glyph for (e.g. U+00AD) says nothing about it.
    cases.retain(|c| c.input.chars().all(|ch| g.has(ch)));
    verify::verify(original, &data, &cases, &g.reverse())?;
    Ok(Patched {
        data,
        lookups,
        rules: compiled.rules,
        dropped: compiled.dropped,
    })
}
