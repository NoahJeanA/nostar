//! `nostar status` (fachl. Spezifikation 10.2, TECHNIK.md 10.5). Changes nothing.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::Error;
use crate::config::Config;
use crate::font::select;
use crate::fsutil::sha256_file;
use crate::paths::Paths;
use crate::rules::{RulesError, Stage, store};
use crate::system::fontconfig;
use crate::system::manifest::Manifest;

pub fn status(paths: &Paths, config: &Config) -> Result<(), Error> {
    let manifest = Manifest::load(&paths.manifest())?;
    let rule_file = paths.fontconfig_rule();
    let tilde = |p: &std::path::Path| paths.tilde(&p.display().to_string());

    let active = rule_file.exists() && !manifest.fonts.is_empty();
    if active {
        out!(
            "{:<12}yes, {} fonts, {}",
            "active",
            manifest.fonts.len(),
            tilde(&rule_file)
        );
    } else {
        out!("{:<12}no", "active");
    }

    let short: String = config.source.commit.chars().take(7).collect();
    let dictionary = if paths.dictionary(&config.source.commit).exists() {
        String::new()
    } else {
        r#", missing, run "nostar data update""#.to_string()
    };
    out!("{:<12}entgendert@{short}{dictionary}", "dictionary");

    let rules_sha = sha256_file(&paths.rules()).ok();
    match store::read(&paths.rules(), config) {
        Ok(rules) => {
            let count = |s| rules.iter().filter(|r| r.stage == s).count();
            out!(
                "{:<12}{} (stage 1: {}, 2: {}, 4: {}, 5: {}), current",
                "rules",
                rules.len(),
                count(Stage::Fixed),
                count(Stage::Lexicon),
                count(Stage::Ending),
                count(Stage::Fallback)
            );
        }
        Err(e @ (RulesError::Missing(_) | RulesError::Stale(_))) => {
            out!("{:<12}{e}", "rules");
        }
        Err(e) => return Err(e.into()),
    }

    out!("{:<12}{}", "patched", manifest.fonts.len());
    let mut outdated: Vec<(PathBuf, &str)> = Vec::new();
    for e in &manifest.fonts {
        let reason = if !e.original.exists() {
            Some("original missing")
        } else if sha256_file(&e.original).ok().as_deref() != Some(e.original_sha256.as_str()) {
            Some("original changed")
        } else if rules_sha.as_deref() != Some(manifest.rules.as_str()) {
            Some("rules changed")
        } else if !e.copy.exists() {
            Some("copy missing")
        } else {
            None
        };
        if let Some(r) = reason {
            outdated.push((e.original.clone(), r));
        }
    }
    out!("{:<12}{}", "outdated", outdated.len());
    for (p, r) in &outdated {
        out!("  {}: {r}", tilde(p));
    }

    let listed = super::listed(config)?;
    let selection = select::select(&listed, config, paths);
    let known: HashSet<&PathBuf> = manifest.fonts.iter().map(|e| &e.original).collect();
    let new: Vec<&PathBuf> = selection
        .fonts
        .iter()
        .filter(|p| !known.contains(p))
        .collect();
    out!("{:<12}{}", "new", new.len());
    for p in new {
        out!("  {}", tilde(p));
    }

    for generic in ["sans-serif", "serif", "monospace"] {
        match fontconfig::best(generic)? {
            Some(f) => {
                let copy = if paths.original_of(&f.file).is_some() {
                    "  (patched copy)"
                } else {
                    ""
                };
                out!("{generic:<12}{}  {}{copy}", f.family, tilde(&f.file));
            }
            None => out!("{generic:<12}none"),
        }
    }
    Ok(())
}
