//! Stage 1: fixed forms and adjective endings (TECHNIK.md 8.2).

use super::{Left, Marker, Right, Rule, Stage};
use crate::lexicon::Origin;

/// Feminine form last; forms with the feminine form first stay unchanged (fachl. Spezifikation 4.6).
const FORMS: &[(&str, &str, &str)] = &[
    // articles
    ("der", "die", "der"),
    ("den", "die", "den"),
    ("dem", "der", "dem"),
    ("des", "der", "des"),
    ("ein", "e", "ein"),
    ("einem", "r", "einem"),
    ("eines", "r", "eines"),
    // pronouns
    ("er", "sie", "er"),
    ("ihn", "sie", "ihn"),
    ("ihm", "ihr", "ihm"),
    ("sein", "ihr", "sein"),
    ("seine", "ihre", "seine"),
    ("seinen", "ihren", "seinen"),
    ("seinem", "ihrem", "seinem"),
    ("seiner", "ihrer", "seiner"),
    ("seines", "ihres", "seines"),
    // determiners
    ("kein", "e", "kein"),
    ("mein", "e", "mein"),
    ("dein", "e", "dein"),
    ("sein", "e", "sein"),
    ("unser", "e", "unser"),
    ("eine", "n", "einen"),
    ("keine", "n", "keinen"),
    ("jede", "n", "jeden"),
    ("diese", "n", "diesen"),
    ("jene", "n", "jenen"),
    ("welche", "n", "welchen"),
    ("meine", "n", "meinen"),
    ("deine", "n", "deinen"),
    ("seine", "n", "seinen"),
    ("unsere", "n", "unseren"),
];

/// Fixed forms, as listed, with an uppercase start and in capitals.
pub fn forms() -> Vec<Rule> {
    let mut rules = Vec::new();
    for (stem, suffix, target) in FORMS {
        let variants = [
            (stem.to_string(), suffix.to_string(), target.to_string()),
            (upper_first(stem), suffix.to_string(), upper_first(target)),
            (
                stem.to_uppercase(),
                suffix.to_uppercase(),
                target.to_uppercase(),
            ),
        ];
        for (stem, suffix, target) in variants {
            rules.push(boundary_rule(stem, suffix, target, Origin::Builtin));
        }
    }
    rules
}

/// `<word>*n` for nominalised adjectives, as written and in capitals, without left
/// boundary; `<word>*r` is covered by the adjective ending `e*r`. Boundary exceptions
/// for all of them would not fit into the GSUB (TECHNIK.md 9.3).
pub fn adjectival(words: &[String], origin: &Origin) -> Vec<Rule> {
    let mut rules = Vec::new();
    for w in words {
        for (stem, suffix) in [(w.clone(), "n"), (w.to_uppercase(), "N")] {
            rules.push(Rule {
                left: Left::Any,
                ..boundary_rule(
                    stem.clone(),
                    suffix.into(),
                    format!("{stem}{suffix}"),
                    origin.clone(),
                )
            });
        }
    }
    rules
}

/// `e*r` -> `er`, only with `GenderText`. `e*n` would turn code like `size*n` into
/// `sizen`; the determiners with `*n` are fixed forms instead.
pub fn adjective_endings() -> Vec<Rule> {
    [("e", "r"), ("E", "R")]
        .into_iter()
        .map(|(stem, suffix)| Rule {
            stage: Stage::Fixed,
            left: Left::Any,
            stem: stem.into(),
            marker: Marker::GenderText,
            suffix: suffix.into(),
            right: Right::WordEnd,
            target: format!("{stem}{suffix}"),
            origin: Origin::Builtin,
        })
        .collect()
}

fn boundary_rule(stem: String, suffix: String, target: String, origin: Origin) -> Rule {
    Rule {
        stage: Stage::Fixed,
        left: Left::Boundary,
        stem,
        marker: Marker::Gender,
        suffix,
        right: Right::WordEnd,
        target,
        origin,
    }
}

pub fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

pub fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_lowercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}
