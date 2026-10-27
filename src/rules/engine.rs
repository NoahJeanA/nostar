//! Reference engine (TECHNIK.md 8.5): applies the rule list to text the way HarfBuzz
//! applies the compiled GSUB, so that fonts can be checked against it.

use std::collections::HashSet;
use std::ops::Range;

use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

use super::edit::{Op, Pass, deumlaut, edit};
use super::{Atom, Left, MARKER_CHARS, Markers, Right, Rule};
use crate::config::is_letter;

/// Stands for any marker character in the lookup keys; the class is checked afterwards.
const MARKER_KEY: char = '\u{E000}';

/// One replacement in the original text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Character indices in the original text, including skipped characters inside.
    pub range: Range<usize>,
    pub target: String,
    /// Index into [`RuleSet::rules`].
    pub rule: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub text: String,
    pub changes: Vec<Change>,
}

/// Ordered rules prepared for matching.
pub struct RuleSet {
    rules: Vec<Rule>,
    /// `None` for rules that change nothing.
    edits: Vec<Option<Compiled>>,
    markers: Markers,
    alive: Vec<bool>,
    stem: PassIndex,
    marker: PassIndex,
}

/// Rules of one pass, grouped by anchor position and sorted by their normalized source
/// for prefix search.
#[derive(Default)]
struct PassIndex {
    by_anchor: Vec<Vec<(Vec<char>, usize)>>,
    anchors: HashSet<char>,
    max_back: usize,
    max_len: usize,
    /// Marker pass: letters after the anchor match without regard to case, as the
    /// folded glyph classes in the font do (TECHNIK.md 9.4).
    fold: bool,
}

struct Compiled {
    pattern: Vec<Atom>,
    ops: Vec<Op>,
    anchor: usize,
}

#[derive(Clone, Copy)]
struct Item {
    ch: char,
    orig: usize,
}

struct Candidate {
    rule: usize,
    /// Start in the context window.
    start: usize,
    len: usize,
}

fn key(c: char) -> char {
    if MARKER_CHARS.contains(&c) {
        MARKER_KEY
    } else {
        c
    }
}

/// Case folding of the font's input classes: a letter and its single-character case mappings.
pub fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) => l,
        _ => c,
    }
}

/// Skipped while matching and kept in place, as HarfBuzz does with marks (`IgnoreMarks`).
fn skipped(c: char) -> bool {
    c.general_category() == GeneralCategory::NonspacingMark
}

const SOFT_HYPHEN: char = '\u{AD}';

/// Letter at word boundaries. HarfBuzz uses a soft hyphen where it matches and skips it
/// where it does not; in the LETTER class it counts as a letter, so forms with one stay
/// unchanged instead of half converted (TECHNIK.md 8.5).
pub fn boundary_letter(c: char) -> bool {
    is_letter(c) || c == SOFT_HYPHEN
}

impl RuleSet {
    /// `rules` must be in list order (TECHNIK.md 8.4). Rules that change nothing never match.
    pub fn new(rules: Vec<Rule>, markers: Markers) -> Self {
        let mut stem = PassIndex::default();
        let mut marker = PassIndex {
            fold: true,
            ..PassIndex::default()
        };
        let mut edits = Vec::with_capacity(rules.len());
        for (i, r) in rules.iter().enumerate() {
            let Some(e) = edit(r) else {
                edits.push(None);
                continue;
            };
            let pattern = r.pattern();
            let index = match e.pass {
                Pass::Stem => &mut stem,
                Pass::Marker => &mut marker,
            };
            let keys: Vec<char> = pattern
                .iter()
                .enumerate()
                .map(|(p, a)| match a {
                    Atom::Char(c) if index.fold && p > e.anchor => fold(*c),
                    Atom::Char(c) => *c,
                    Atom::Marker(_) => MARKER_KEY,
                })
                .collect();
            index.anchors.insert(keys[e.anchor]);
            index.max_back = index.max_back.max(e.anchor);
            index.max_len = index.max_len.max(keys.len());
            if index.by_anchor.len() <= e.anchor {
                index.by_anchor.resize_with(e.anchor + 1, Vec::new);
            }
            index.by_anchor[e.anchor].push((keys, i));
            edits.push(Some(Compiled {
                pattern,
                ops: e.ops,
                anchor: e.anchor,
            }));
        }
        for group in stem.by_anchor.iter_mut().chain(marker.by_anchor.iter_mut()) {
            group.sort();
        }
        Self {
            alive: vec![true; rules.len()],
            rules,
            edits,
            markers,
            stem,
            marker,
        }
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn markers(&self) -> &Markers {
        &self.markers
    }

    /// Minimization switches single rules off and on again.
    pub(crate) fn set_alive(&mut self, rule: usize, alive: bool) {
        self.alive[rule] = alive;
    }

    pub fn apply(&self, text: &str) -> Output {
        let items: Vec<Item> = text
            .chars()
            .enumerate()
            .map(|(orig, ch)| Item { ch, orig })
            .collect();
        let mut changes = Vec::new();
        let items = self.pass(&self.stem, items, &mut changes);
        let items = self.pass(&self.marker, items, &mut changes);
        changes.sort_by_key(|c| c.range.start);
        Output {
            text: items.iter().map(|i| i.ch).collect(),
            changes,
        }
    }

    /// Left to right; matches against already replaced text on the left, as GSUB does.
    fn pass(&self, index: &PassIndex, input: Vec<Item>, changes: &mut Vec<Change>) -> Vec<Item> {
        let mut out: Vec<Item> = Vec::with_capacity(input.len());
        let mut i = 0;
        while i < input.len() {
            let item = input[i];
            if skipped(item.ch) || item.ch == SOFT_HYPHEN || !index.anchors.contains(&key(item.ch))
            {
                out.push(item);
                i += 1;
                continue;
            }
            // Context without marks: output before i, input from i on. Soft hyphens stay in
            // it for the boundary checks; `vis` holds the positions that rules match.
            let mut back: Vec<Item> = Vec::new();
            let mut seen = 0;
            for it in out.iter().rev().filter(|it| !skipped(it.ch)) {
                if seen > index.max_back + 2 {
                    break;
                }
                seen += usize::from(it.ch != SOFT_HYPHEN);
                back.push(*it);
            }
            back.reverse();
            let mut ahead: Vec<usize> = Vec::new();
            let mut seen = 0;
            for j in (i..input.len()).filter(|&j| !skipped(input[j].ch)) {
                if seen > index.max_len + 2 {
                    break;
                }
                seen += usize::from(input[j].ch != SOFT_HYPHEN);
                ahead.push(j);
            }
            let window: Vec<Item> = back
                .iter()
                .copied()
                .chain(ahead.iter().map(|&j| input[j]))
                .collect();
            let vis: Vec<usize> = (0..window.len())
                .filter(|&w| window[w].ch != SOFT_HYPHEN)
                .collect();
            let anchor = vis.iter().position(|&w| w == back.len()).unwrap_or(0);
            let Some((c, blocked)) = self.select(index, &window, &vis, anchor) else {
                out.push(item);
                i += 1;
                continue;
            };
            let last = ahead[vis[c.start + c.len - 1] - back.len()];
            let ops = match &self.edits[c.rule] {
                Some(compiled) if !blocked => &compiled.ops,
                _ => {
                    out.extend_from_slice(&input[i..=last]);
                    i = last + 1;
                    continue;
                }
            };
            let mut p = anchor - c.start;
            for it in &input[i..=last] {
                if skipped(it.ch) || it.ch == SOFT_HYPHEN {
                    out.push(*it);
                    continue;
                }
                match &ops[p] {
                    Op::Keep => out.push(*it),
                    Op::Deumlaut => out.push(Item {
                        ch: deumlaut(it.ch).unwrap_or(it.ch),
                        orig: it.orig,
                    }),
                    Op::Replace(s) => out.extend(s.chars().map(|ch| Item { ch, orig: it.orig })),
                    Op::Delete => {}
                }
                p += 1;
            }
            changes.push(Change {
                range: window[vis[c.start]].orig..input[last].orig + 1,
                target: self.rules[c.rule].target.clone(),
                rule: c.rule,
            });
            i = last + 1;
        }
        out
    }

    /// First rule in list order whose source matches with its anchor at `vis[anchor]`;
    /// `true` if it is an exception. Candidate positions count in `vis`.
    fn select(
        &self,
        index: &PassIndex,
        window: &[Item],
        vis: &[usize],
        anchor: usize,
    ) -> Option<(Candidate, bool)> {
        let mut found: Vec<Candidate> = Vec::new();
        for (k, keys) in index.by_anchor.iter().enumerate().take(anchor + 1) {
            let start = anchor - k;
            let (mut lo, mut hi) = (0, keys.len());
            for d in 0..index.max_len.min(vis.len() - start) {
                let ch = window[vis[start + d]].ch;
                let c = key(if index.fold && d > k { fold(ch) } else { ch });
                let range = &keys[lo..hi];
                let a = range.partition_point(|(s, _)| s.len() <= d || s[d] < c);
                let b = range.partition_point(|(s, _)| s.len() <= d || s[d] <= c);
                (lo, hi) = (lo + a, lo + b);
                if lo == hi {
                    break;
                }
                let len = d + 1;
                for (s, rule) in &keys[lo..hi] {
                    if s.len() != len {
                        break;
                    }
                    let Some(Compiled { anchor: a, .. }) = &self.edits[*rule] else {
                        continue;
                    };
                    if *a == k && len > k && self.alive[*rule] {
                        found.push(Candidate {
                            rule: *rule,
                            start,
                            len,
                        });
                    }
                }
            }
        }
        found.sort_by_key(|c| c.rule);
        found.into_iter().find_map(|c| {
            let state = self.state(&c, window, vis)?;
            Some((c, state))
        })
    }

    /// `None`: no match; `Some(true)`: exception; `Some(false)`: replacement.
    ///
    /// Soft hyphens follow HarfBuzz: used where they match (they are in the LETTER class),
    /// skipped where they do not (rule characters, markers, the letter the gate needs).
    fn state(&self, c: &Candidate, window: &[Item], vis: &[usize]) -> Option<bool> {
        let rule = &self.rules[c.rule];
        let pattern = &self.edits[c.rule].as_ref()?.pattern;
        let classes_ok = pattern.iter().enumerate().all(|(p, atom)| match atom {
            Atom::Marker(class) => self.markers.accepts(*class, window[vis[c.start + p]].ch),
            Atom::Char(_) => true,
        });
        if !classes_ok {
            return None;
        }
        let at = |w: Option<usize>| w.and_then(|w| window.get(w)).map(|it| it.ch);
        let letter = |w: Option<usize>| at(w).is_some_and(boundary_letter);
        let marker = |w: Option<usize>| at(w).is_some_and(|ch| self.markers.gender.contains(&ch));
        let first = vis[c.start];
        let last = vis[c.start + c.len - 1];
        // nearest neighbours, and the nearest ones that are no soft hyphen
        let before = first.checked_sub(1);
        let after = Some(last + 1);
        let before_vis = c.start.checked_sub(1).map(|v| vis[v]);
        let after_vis = vis.get(c.start + c.len).copied();
        if rule.left == Left::Letter && !at(before_vis).is_some_and(is_letter) {
            return None;
        }
        if rule.right == Right::Letter && !letter(after) {
            return None;
        }
        let boundary = rule.left == Left::Boundary;
        let blocked_left = boundary
            && (letter(before)
                || (marker(before_vis) && letter(before_vis.and_then(|w| w.checked_sub(1)))));
        let blocked_right = rule.right == Right::WordEnd
            && (letter(after)
                || (boundary && marker(after_vis) && letter(after_vis.map(|w| w + 1))));
        Some(blocked_left || blocked_right)
    }

    /// Text after applying the rules; shorthand for tests and minimization.
    pub fn render(&self, text: &str) -> String {
        self.apply(text).text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexicon::Origin;
    use crate::rules::{Marker, Stage};

    fn markers() -> Markers {
        Markers {
            gender: MARKER_CHARS.to_vec(),
            slash_hyphen: true,
        }
    }

    fn rule(
        stage: Stage,
        left: Left,
        src: (&str, Marker, &str),
        right: Right,
        target: &str,
    ) -> Rule {
        Rule {
            stage,
            left,
            stem: src.0.into(),
            marker: src.1,
            suffix: src.2.into(),
            right,
            target: target.into(),
            origin: Origin::Builtin,
        }
    }

    fn lexicon(stem: &str, suffix: &str, right: Right, target: &str) -> Rule {
        rule(
            Stage::Lexicon,
            Left::Any,
            (stem, Marker::Gender, suffix),
            right,
            target,
        )
    }

    #[test]
    fn two_passes_and_compound() {
        let set = RuleSet::new(
            vec![
                lexicon("Ärzt", "innen", Right::Any, "Ärzte"),
                lexicon("Ärzt", "in", Right::WordEnd, "Arzt"),
            ],
            markers(),
        );
        assert_eq!(set.render("Ärzt*innen"), "Ärzte");
        assert_eq!(set.render("die Ärzt:in kommt"), "die Arzt kommt");
        assert_eq!(set.render("Ärzt_innenkammer"), "Ärztekammer");
        // word end exception: "Ärzt*inx" stays
        assert_eq!(set.render("Ärzt*inx"), "Ärzt*inx");
    }

    #[test]
    fn exception_blocks_later_rules_at_same_anchor() {
        // TECHNIK.md 8.3: without the plural rule, "Brit*innen" hangs on the exception of "Brit*in".
        let ending = rule(
            Stage::Ending,
            Left::Any,
            ("it", Marker::Gender, "innen"),
            Right::Any,
            "iten",
        );
        let singular = lexicon("Brit", "in", Right::WordEnd, "Brite");
        let set = RuleSet::new(vec![singular.clone(), ending.clone()], markers());
        assert_eq!(set.render("Brit*innen"), "Brit*innen");
        let plural = lexicon("Brit", "innen", Right::Any, "Briten");
        let set = RuleSet::new(vec![plural, singular, ending], markers());
        assert_eq!(set.render("Brit*innen"), "Briten");
    }

    #[test]
    fn letter_right_does_not_block() {
        let set = RuleSet::new(
            vec![
                lexicon("Freund", "innen", Right::Letter, "Freundes"),
                lexicon("Freund", "innen", Right::WordEnd, "Freunde"),
            ],
            markers(),
        );
        assert_eq!(set.render("Freund*innenkreis"), "Freundeskreis");
        assert_eq!(set.render("Freund*innen."), "Freunde.");
    }

    #[test]
    fn boundary_includes_marker_and_letter() {
        let der = rule(
            Stage::Fixed,
            Left::Boundary,
            ("der", Marker::Gender, "die"),
            Right::WordEnd,
            "der",
        );
        let set = RuleSet::new(vec![der], markers());
        assert_eq!(set.render("der*die Arzt"), "der Arzt");
        assert_eq!(set.render("der/die/das"), "der/die/das");
        assert_eq!(set.render("das/der/die"), "das/der/die");
        assert_eq!(set.render("oder*die"), "oder*die");
    }

    #[test]
    fn fallback_needs_letter_and_skips_slash() {
        let cut = rule(
            Stage::Fallback,
            Left::Letter,
            ("", Marker::GenderText, "innen"),
            Right::Any,
            "",
        );
        let set = RuleSet::new(vec![cut], markers());
        assert_eq!(set.render("Xyz*innen"), "Xyz");
        assert_eq!(set.render("Xyz/innen"), "Xyz/innen");
        assert_eq!(set.render("*innen"), "*innen");
    }

    #[test]
    fn marks_skipped_soft_hyphen_maybe_skipped() {
        let set = RuleSet::new(
            vec![
                lexicon("Lehrer", "innen", Right::Any, "Lehrer"),
                lexicon("Ärzt", "in", Right::WordEnd, "Arzt"),
            ],
            markers(),
        );
        // skipped inside the source, kept in place
        assert_eq!(set.render("Leh\u{AD}rer*in\u{AD}nen"), "Leh\u{AD}rer\u{AD}");
        // a letter at the word end: the exception of "Ärzt*in" holds
        assert_eq!(set.render("Ärzt*in\u{AD}"), "Ärzt*in\u{AD}");
        assert_eq!(set.render("Ärzt*in\u{AD} "), "Ärzt*in\u{AD} ");
        assert_eq!(set.render("Lehre\u{301}r*innen"), "Lehre\u{301}r");
        let out = set.apply("Die Ärzt*in.");
        assert_eq!(out.text, "Die Arzt.");
        assert_eq!(out.changes[0].range, 4..11);
    }

    #[test]
    fn suffix_case_is_folded_in_marker_pass() {
        let set = RuleSet::new(
            vec![
                lexicon("Kolleg", "innen", Right::Any, "Kollegen"),
                lexicon("Ärzt", "in", Right::WordEnd, "Arzt"),
            ],
            markers(),
        );
        assert_eq!(set.render("Kolleg*INNEN Kolleg*Innen"), "Kollegen Kollegen");
        // stem pass compares exactly
        assert_eq!(set.render("Ärzt*IN"), "Ärzt*IN");
    }

    #[test]
    fn inactive_marker_does_not_match() {
        let mut m = markers();
        m.gender.retain(|c| *c != ':');
        let set = RuleSet::new(vec![lexicon("Ärzt", "innen", Right::Any, "Ärzte")], m);
        assert_eq!(set.render("Ärzt:innen Ärzt*innen"), "Ärzt:innen Ärzte");
    }
}
