//! Commands (TECHNIK.md 10): the only layer that combines several modules.

pub mod apply;
pub mod data;
pub mod remove;
pub mod rules;
pub mod show;
pub mod status;

use std::collections::BTreeSet;

use crate::Error;
use crate::config::{Config, Mode};
use crate::font::select::Listed;
use crate::system::fontconfig;

/// Families whose fonts are candidates in mode `standard` (fachl. Spezifikation 7.1).
const STANDARD: [&str; 6] = [
    "sans-serif",
    "serif",
    "monospace",
    "Arial",
    "Times New Roman",
    "Courier New",
];

/// Font files fontconfig lists for the configured mode (TECHNIK.md 10.3).
pub fn listed(config: &Config) -> Result<Vec<Listed>, Error> {
    let fonts = match config.mode {
        Mode::All => fontconfig::list(":")?,
        Mode::Standard => {
            let mut families = BTreeSet::new();
            for pattern in STANDARD {
                if let Some(f) = fontconfig::best(pattern)? {
                    families.insert(f.family);
                }
            }
            families.extend(config.families.iter().cloned());
            let mut fonts = Vec::new();
            for family in families {
                fonts.extend(fontconfig::list(&fontconfig::family_pattern(&family))?);
            }
            fonts
        }
    };
    Ok(fonts
        .into_iter()
        .map(|f| Listed {
            file: f.file,
            family: f.family,
            mono: f.mono,
        })
        .collect())
}

pub const RESTART: &str = "restart running applications to see the change";
