//! Commands (TECHNIK.md 10): the only layer that combines several modules.

/// One line of result output on stdout, like `println!`.
macro_rules! out {
    ($($arg:tt)*) => {
        $crate::cmd::write_line(format_args!($($arg)*))
    };
}

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
use crate::system::{desktop, fontconfig};

/// Families whose fonts are candidates in mode `standard` (fachl. Spezifikation 7.1), besides
/// the GNOME fonts.
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
            // The GNOME fonts like the generic names: whatever fontconfig shows for them.
            let mut patterns: Vec<String> = STANDARD.iter().map(|p| p.to_string()).collect();
            patterns.extend(
                desktop::families()
                    .iter()
                    .map(|f| fontconfig::family_pattern(f)),
            );
            for pattern in &patterns {
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

/// Writes one line to stdout. A closed pipe (`nostar status | head`) ends the program with
/// exit code 0: every command prints only after its work is done, and the lock is released
/// with the process.
pub fn write_line(args: std::fmt::Arguments) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_fmt(args)
        .and_then(|()| stdout.write_all(b"\n"))
    {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        log::warn!("stdout: {e}");
    }
}

pub const RESTART: &str = "restart running applications to see the change";
