//! Lexicon -> ordered, minimized rule list (TECHNIK.md 8.2-8.4).

use std::cmp::Reverse;
use std::collections::HashSet;

use super::edit::{Pass, edit};
use super::engine::RuleSet;
use super::fixed::{self, lower_first};
use super::{Atom, Left, Marker, Markers, Right, Rule, Stage};
use crate::config::{Config, Fallback};
use crate::lexicon::{Lexicon, Origin, Person};

/// Builds the rule list for `lexicon` (with own words already merged) and `config`.
pub fn build(lexicon: &Lexicon, config: &Config) -> RuleSet {
    let markers = Markers::from_config(config);
    let mut rules = generate(lexicon, config, &markers);
    drop_stem_anchors(&mut rules);
    sort(&mut rules);
    dedupe(&mut rules);
    log_stages(&rules);
    ending_hits(lexicon);
    let before = rules.len();
    let set = minimize(RuleSet::new(rules, markers));
    log::info!("minimized {before} -> {} rules", set.rules().len());
    set
}

fn generate(lexicon: &Lexicon, config: &Config, markers: &Markers) -> Vec<Rule> {
    let mut rules = fixed::forms();
    rules.extend(fixed::adjective_endings());
    let dictionary = lexicon
        .persons
        .iter()
        .find_map(|p| match &p.origin {
            o @ Origin::Entgendert { .. } => Some(o.clone()),
            _ => None,
        })
        .unwrap_or(Origin::Builtin);
    rules.extend(fixed::adjectival(&lexicon.adjectival, &dictionary));
    let mut gender = Vec::new();
    if !markers.gender.is_empty() {
        gender.push(Marker::Gender);
    }
    if markers.slash_hyphen {
        gender.push(Marker::SlashHyphen);
    }
    for p in &lexicon.persons {
        person_rules(p, Stage::Lexicon, &gender, config.binnen_i, &mut rules);
    }
    for e in &lexicon.endings {
        let p = Person {
            stem: e.ending.clone(),
            singular: e.singular.clone(),
            plural: e.plural.clone(),
            compound: e.plural.clone(),
            origin: dictionary.clone(),
        };
        person_rules(&p, Stage::Ending, &gender, false, &mut rules);
    }
    rules.extend(fallback(config.fallback));
    rules
}

/// Variants of TECHNIK.md 8.2: casing x suffix casing x marker, plus Binnen-I.
fn person_rules(p: &Person, stage: Stage, markers: &[Marker], binnen_i: bool, out: &mut Vec<Rule>) {
    let casings = [
        (
            p.stem.clone(),
            [("innen", "in"), ("Innen", "In")].as_slice(),
            ToString::to_string as fn(&str) -> String,
        ),
        (
            lower_first(&p.stem),
            [("innen", "in"), ("Innen", "In")].as_slice(),
            lower_first,
        ),
        (
            p.stem.to_uppercase(),
            [("INNEN", "IN")].as_slice(),
            str::to_uppercase,
        ),
    ];
    let mut seen = HashSet::new();
    for (stem, suffixes, case) in casings {
        if !seen.insert(stem.clone()) {
            continue;
        }
        for &marker in markers {
            for (plural, singular) in suffixes {
                let forms = Forms {
                    singular: case(&p.singular),
                    plural: case(&p.plural),
                    compound: case(&p.compound),
                };
                push_variants(p, stage, &stem, marker, (plural, singular), &forms, out);
            }
        }
    }
    if binnen_i && stage == Stage::Lexicon {
        // Original spelling and, for compounds, a lowercase start: "MitarbeiterInnen".
        for case in [ToString::to_string as fn(&str) -> String, lower_first] {
            let forms = Forms {
                singular: case(&p.singular),
                plural: case(&p.plural),
                compound: case(&p.compound),
            };
            let stem = case(&p.stem);
            push_variants(
                p,
                stage,
                &stem,
                Marker::BinnenI,
                ("Innen", "In"),
                &forms,
                out,
            );
        }
    }
}

struct Forms {
    singular: String,
    plural: String,
    compound: String,
}

fn push_variants(
    p: &Person,
    stage: Stage,
    stem: &str,
    marker: Marker,
    (plural, singular): (&str, &str),
    forms: &Forms,
    out: &mut Vec<Rule>,
) {
    let rule = |suffix: &str, right, target: &str| Rule {
        stage,
        left: Left::Any,
        stem: stem.to_string(),
        marker,
        suffix: suffix.to_string(),
        right,
        target: target.to_string(),
        origin: p.origin.clone(),
    };
    if forms.compound == forms.plural {
        out.push(rule(plural, Right::Any, &forms.plural));
    } else {
        out.push(rule(plural, Right::Letter, &forms.compound));
        out.push(rule(plural, Right::WordEnd, &forms.plural));
    }
    out.push(rule(singular, Right::WordEnd, &forms.singular));
}

/// Stage 5, only with `GenderText` so that paths, URLs and code stay (TECHNIK.md 15, point 5).
fn fallback(mode: Fallback) -> Vec<Rule> {
    let rule = |suffix: &str, right| Rule {
        stage: Stage::Fallback,
        left: Left::Letter,
        stem: String::new(),
        marker: Marker::GenderText,
        suffix: suffix.to_string(),
        right,
        target: String::new(),
        origin: Origin::Builtin,
    };
    match mode {
        Fallback::Cut => ["innen", "Innen", "INNEN"]
            .map(|s| rule(s, Right::Any))
            .into_iter()
            .chain(["in", "In", "IN"].map(|s| rule(s, Right::WordEnd)))
            .collect(),
        Fallback::Marker => vec![rule("", Right::Letter)],
        Fallback::Off => vec![],
    }
}

/// Rules anchored in the stem at a letter other than an umlaut are dropped (TECHNIK.md 8.2).
fn drop_stem_anchors(rules: &mut Vec<Rule>) {
    let mut warned = HashSet::new();
    rules.retain(|r| {
        let Some(e) = edit(r) else {
            log::debug!("dropped {} -> {}: no change", r.source(), r.target);
            return false;
        };
        let anchor = match r.pattern()[e.anchor] {
            Atom::Char(c) => c,
            Atom::Marker(_) => return true,
        };
        if e.pass == Pass::Marker || super::edit::deumlaut(anchor).is_some() {
            return true;
        }
        let message = format!(
            "dropped {} -> {}: anchor '{anchor}' is not an umlaut",
            r.source(),
            r.target
        );
        if warned.insert(r.stem.to_lowercase()) {
            log::warn!("{message}");
        } else {
            log::debug!("{message}");
        }
        false
    });
}

/// TECHNIK.md 8.4.
pub fn sort(rules: &mut [Rule]) {
    rules.sort_by(|a, b| {
        (a.stage, Reverse(a.source_len()), a.right)
            .cmp(&(b.stage, Reverse(b.source_len()), b.right))
            .then_with(|| {
                (&a.stem, a.marker, &a.suffix, a.left).cmp(&(&b.stem, b.marker, &b.suffix, b.left))
            })
            .then_with(|| a.target.cmp(&b.target))
            .then_with(|| a.origin.to_string().cmp(&b.origin.to_string()))
    });
}

/// Of rules with the same source and conditions only the first can ever match.
/// Different targets are conflicts (fachl. Spezifikation 6.2).
fn dedupe(rules: &mut Vec<Rule>) {
    let mut conflicts = 0;
    let mut kept: Vec<Rule> = Vec::with_capacity(rules.len());
    for r in rules.drain(..) {
        if let Some(prev) = kept.last()
            && (
                prev.stage,
                prev.left,
                &prev.stem,
                prev.marker,
                &prev.suffix,
                prev.right,
            ) == (r.stage, r.left, &r.stem, r.marker, &r.suffix, r.right)
        {
            if prev.target != r.target {
                conflicts += 1;
                log::debug!(
                    "conflict {}: {} ({}) wins over {} ({})",
                    r.source(),
                    prev.target,
                    prev.origin,
                    r.target,
                    r.origin
                );
            }
            continue;
        }
        kept.push(r);
    }
    if conflicts > 0 {
        log::info!("{conflicts} conflicts, first rule kept");
    }
    *rules = kept;
}

fn log_stages(rules: &[Rule]) {
    let count = |s| rules.iter().filter(|r| r.stage == s).count();
    log::info!(
        "generated {} rules: stage 1: {}, stage 2: {}, stage 4: {}, stage 5: {}",
        rules.len(),
        count(Stage::Fixed),
        count(Stage::Lexicon),
        count(Stage::Ending),
        count(Stage::Fallback)
    );
}

/// Hit rate of each ending rule on the dictionary stems it would cover (TECHNIK.md 15, point 3).
fn ending_hits(lexicon: &Lexicon) {
    for e in &lexicon.endings {
        let mut total = 0;
        let mut hits = 0;
        for p in &lexicon.persons {
            if !matches!(p.origin, Origin::Entgendert { .. }) {
                continue;
            }
            let Some(base) = p.stem.strip_suffix(e.ending.as_str()) else {
                continue;
            };
            // Only stems for which this is the longest matching ending.
            let longer = lexicon
                .endings
                .iter()
                .any(|o| o.ending.len() > e.ending.len() && p.stem.ends_with(o.ending.as_str()));
            if longer {
                continue;
            }
            total += 1;
            if p.singular == format!("{base}{}", e.singular)
                && p.plural == format!("{base}{}", e.plural)
            {
                hits += 1;
            }
        }
        if total == 0 {
            log::info!("ending -{}: no dictionary stems", e.ending);
            continue;
        }
        // The dictionary is an extract, so this is lower than Entgendert's rate; info only.
        let rate = 100.0 * f64::from(hits) / f64::from(total);
        log::info!(
            "ending -{}: {rate:.1} % of {total} dictionary stems",
            e.ending
        );
    }
}

/// TECHNIK.md 8.3: a stage 2 rule goes if the engine gives the same result for its own
/// source without it. Back to front, so every check sees the final state of later rules.
fn minimize(mut set: RuleSet) -> RuleSet {
    let mut keep = vec![true; set.rules().len()];
    for i in (0..set.rules().len()).rev() {
        let r = &set.rules()[i];
        if r.stage != Stage::Lexicon {
            continue;
        }
        let texts = test_texts(r, set.markers());
        let before: Vec<String> = texts.iter().map(|t| set.render(t)).collect();
        set.set_alive(i, false);
        if texts.iter().zip(&before).any(|(t, b)| set.render(t) != *b) {
            set.set_alive(i, true);
        } else {
            keep[i] = false;
        }
    }
    let markers = set.markers().clone();
    let rules = set
        .rules()
        .iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(r, _)| r.clone())
        .collect();
    RuleSet::new(rules, markers)
}

/// The source with `*` and with `/` as marker; with a compound part where a letter may follow.
fn test_texts(r: &Rule, markers: &Markers) -> Vec<String> {
    let samples: Vec<String> = match r.marker {
        Marker::BinnenI => vec![String::new()],
        Marker::SlashHyphen => ["*-", "/-"].map(String::from).to_vec(),
        Marker::Gender | Marker::GenderText => {
            let mut v: Vec<String> = markers
                .gender
                .iter()
                .find(|c| **c != '/')
                .map(|c| c.to_string())
                .into_iter()
                .collect();
            if markers.gender.contains(&'/') {
                v.push("/".into());
            }
            v
        }
    };
    let mut texts = Vec::new();
    for m in samples {
        let text = format!("{}{m}{}", r.stem, r.suffix);
        match r.right {
            Right::WordEnd => texts.push(text),
            Right::Any => {
                texts.push(format!("{text}zimmer"));
                texts.push(text);
            }
            Right::Letter => texts.push(format!("{text}zimmer")),
        }
    }
    texts
}
