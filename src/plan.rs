//! Rules -> passes, gate and tree keys, character based (TECHNIK.md 9.2-9.4).
//! The plan is made once and shared by all fonts; `font::compile` maps it to glyphs.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::rules::edit::{Op, Pass, deumlaut, edit};
use crate::rules::engine::fold;
use crate::rules::{Atom, Class, Left, Right, Rule};

/// Most GSUB rules in one leaf before the tree splits further.
pub const LEAF_MAX: usize = 40;

/// Tree key for any marker class.
pub const MARKER_KEY: char = '\u{E000}';

/// Context beyond the rule's own characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Edge {
    None,
    Letter,
    /// A marker, then a letter (seen from the rule outwards).
    MarkerLetter,
}

/// Which input class table a GSUB rule uses (TECHNIK.md 9.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// Stem pass: every character its own class.
    Stem,
    /// Marker pass with a gender marker as anchor: suffix letters folded.
    Marker,
    /// Marker pass with `I` as anchor: `I` exact, the rest folded.
    Binnen,
    /// Marker pass with the `GenderText` class as anchor.
    Text,
}

/// One GSUB chain rule at character level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GRule {
    pub kind: Kind,
    /// Stem before the anchor, nearest character first.
    pub backtrack: Vec<char>,
    /// From the anchor to the end of the source.
    pub input: Vec<Atom>,
    pub before: Edge,
    pub after: Edge,
    /// Operations at input positions; empty for an exception.
    pub action: Vec<(usize, Op)>,
}

impl GRule {
    pub fn is_exception(&self) -> bool {
        self.action.is_empty()
    }
}

/// Anchor of a marker-pass root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Root {
    /// Any marker class.
    Marker,
    BinnenI,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    /// Marker pass: keys are the stem read right to left.
    Back,
    /// Stem pass: keys are the characters after the anchor.
    Fwd,
}

#[derive(Debug, Clone)]
pub enum Node {
    Branch {
        dir: Dir,
        /// Characters of the anchor glyph class.
        anchor: Anchor,
        path: Vec<char>,
        children: Vec<(char, usize)>,
        other: Option<usize>,
    },
    Leaf(Vec<GRule>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anchor {
    Root(Root),
    Char(char),
}

/// Umlaut and its gate entries: key characters after it up to the marker, node.
pub type StemRoot = (char, Vec<(Vec<char>, usize)>);
/// Marker-pass root: last stem letter -> node, node for other letters.
pub type MarkerRoot = (Root, Vec<(char, usize)>, Option<usize>);

#[derive(Debug, Clone, Default)]
pub struct Plan {
    /// Umlaut -> (key characters after it up to the marker, node), longest first.
    pub stem_gate: Vec<StemRoot>,
    /// Root -> (last stem letter, node), node for other letters.
    pub marker_gate: Vec<MarkerRoot>,
    pub nodes: Vec<Node>,
    /// Rules that cannot be compiled (logged).
    pub skipped: usize,
}

/// Builds the plan for rules in list order.
pub fn plan(rules: &[Rule]) -> Plan {
    let mut plan = Plan::default();
    let mut seen = HashSet::new();
    let mut stem: BTreeMap<char, Vec<GRule>> = BTreeMap::new();
    let mut marker: BTreeMap<Root, Vec<GRule>> = BTreeMap::new();
    for r in rules {
        let Some(grules) = grules(r) else {
            log::warn!("not compilable: {} -> {}", r.source(), r.target);
            plan.skipped += 1;
            continue;
        };
        for g in grules {
            if !seen.insert(dedupe_key(&g)) {
                continue;
            }
            match (g.kind, &g.input[0]) {
                (Kind::Stem, Atom::Char(c)) => stem.entry(*c).or_default().push(g),
                (Kind::Binnen, _) => marker.entry(Root::BinnenI).or_default().push(g),
                _ => marker.entry(Root::Marker).or_default().push(g),
            }
        }
    }
    for (root, rs) in marker {
        let (children, other) = split(&mut plan, rs, Dir::Back, Anchor::Root(root), 0, &[]);
        plan.marker_gate.push((root, children, other));
    }
    for (anchor, rs) in stem {
        // The gate checks everything up to and including the marker, so ordinary words
        // with an umlaut fail in the gate without a nested lookup (TECHNIK.md 9.3).
        let mut groups: Vec<(Vec<char>, Vec<GRule>)> = Vec::new();
        for r in rs {
            let k = key(&r, Dir::Fwd);
            let end = k
                .iter()
                .position(|c| *c == MARKER_KEY || *c == 'I')
                .map_or(k.len(), |i| i + 1);
            let prefix = k[..end].to_vec();
            match groups.iter_mut().find(|g| g.0 == prefix) {
                Some(g) => g.1.push(r),
                None => groups.push((prefix, vec![r])),
            }
        }
        // Longer prefixes first: a shorter one could match the same text and end the search.
        groups.sort_by_key(|g| std::cmp::Reverse(g.0.len()));
        let mut children = Vec::new();
        for (prefix, g) in groups {
            let node = tree(
                &mut plan,
                g,
                Dir::Fwd,
                Anchor::Char(anchor),
                prefix.len(),
                &prefix,
            );
            children.push((prefix, node));
        }
        plan.stem_gate.push((anchor, children));
    }
    plan
}

/// The GSUB rules of one rule in order: exceptions first, then the rule itself.
/// `None` if the font gates could not reach it the way the reference engine does.
fn grules(r: &Rule) -> Option<Vec<GRule>> {
    let e = edit(r)?;
    let pattern = r.pattern();
    let backtrack: Vec<char> = pattern[..e.anchor]
        .iter()
        .rev()
        .map(|a| match a {
            Atom::Char(c) => Some(*c),
            Atom::Marker(_) => None,
        })
        .collect::<Option<_>>()?;
    let input = pattern[e.anchor..].to_vec();
    let kind = match (e.pass, &input[0]) {
        (Pass::Stem, Atom::Char(c)) if deumlaut(*c).is_some() => Kind::Stem,
        (Pass::Marker, Atom::Marker(Class::Text)) => Kind::Text,
        (Pass::Marker, Atom::Marker(_)) => Kind::Marker,
        (Pass::Marker, Atom::Char('I')) => Kind::Binnen,
        _ => return None,
    };
    if kind != Kind::Stem {
        // The marker gate needs a letter before the anchor and a letter or '-' after it.
        let letter_before = !backtrack.is_empty() || r.left == Left::Letter;
        let after_ok = match input.get(1) {
            Some(Atom::Char(c)) => crate::config::is_letter(*c) || *c == '-',
            Some(Atom::Marker(_)) => false,
            None => r.right == Right::Letter,
        };
        if !letter_before || !after_ok {
            return None;
        }
    }
    let action: Vec<(usize, Op)> = e.ops[e.anchor..]
        .iter()
        .enumerate()
        .filter(|(_, op)| **op != Op::Keep)
        .map(|(i, op)| (i, op.clone()))
        .collect();
    // With an empty stem the marker gate already requires the letter before the anchor;
    // leaving the condition out keeps the rule in format 2 (TECHNIK.md 9.4).
    let letter_before = r.left == Left::Letter && !(kind != Kind::Stem && backtrack.is_empty());
    let base = GRule {
        kind,
        backtrack,
        input,
        before: if letter_before {
            Edge::Letter
        } else {
            Edge::None
        },
        after: if r.right == Right::Letter {
            Edge::Letter
        } else {
            Edge::None
        },
        action: Vec::new(),
    };
    let mut out = Vec::new();
    let boundary = r.left == Left::Boundary;
    if boundary {
        for before in [Edge::Letter, Edge::MarkerLetter] {
            out.push(GRule {
                before,
                ..base.clone()
            });
        }
    }
    if r.right == Right::WordEnd {
        out.push(GRule {
            after: Edge::Letter,
            ..base.clone()
        });
        if boundary {
            out.push(GRule {
                after: Edge::MarkerLetter,
                ..base.clone()
            });
        }
    }
    out.push(GRule { action, ..base });
    Some(out)
}

/// Identical conditions: a later rule could never fire. Marker-pass input is case folded.
fn dedupe_key(g: &GRule) -> String {
    let input: Vec<Atom> = g
        .input
        .iter()
        .enumerate()
        .map(|(i, a)| match a {
            Atom::Char(c) if g.kind != Kind::Stem && i > 0 => Atom::Char(fold(*c)),
            other => *other,
        })
        .collect();
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
        g.kind, g.backtrack, input, g.before, g.after, g.action
    )
}

/// Tree key: the stem right to left, or the characters after the anchor.
pub fn key(g: &GRule, dir: Dir) -> Vec<char> {
    match dir {
        Dir::Back => g.backtrack.clone(),
        Dir::Fwd => g.input[1..]
            .iter()
            .map(|a| match a {
                Atom::Char(c) => *c,
                Atom::Marker(_) => MARKER_KEY,
            })
            .collect(),
    }
}

fn tree(
    plan: &mut Plan,
    rules: Vec<GRule>,
    dir: Dir,
    anchor: Anchor,
    depth: usize,
    path: &[char],
) -> usize {
    let splittable = rules.iter().any(|r| key(r, dir).len() > depth);
    if rules.len() <= LEAF_MAX || !splittable {
        plan.nodes.push(Node::Leaf(rules));
        return plan.nodes.len() - 1;
    }
    let id = plan.nodes.len();
    plan.nodes.push(Node::Leaf(Vec::new()));
    let (children, other) = split(plan, rules, dir, anchor, depth, path);
    plan.nodes[id] = Node::Branch {
        dir,
        anchor,
        path: path.to_vec(),
        children,
        other,
    };
    id
}

/// Distributes rules by their key character at `depth`. Rules with a shorter key go into
/// every child, keeping their order, and into `other`. Small children are merged into
/// `other`, smallest first; every rule there checks its full context itself.
fn split(
    plan: &mut Plan,
    rules: Vec<GRule>,
    dir: Dir,
    anchor: Anchor,
    depth: usize,
    path: &[char],
) -> (Vec<(char, usize)>, Option<usize>) {
    let mut counts: BTreeMap<char, usize> = BTreeMap::new();
    for r in &rules {
        if let Some(c) = key(r, dir).get(depth) {
            *counts.entry(*c).or_default() += 1;
        }
    }
    let mut merged = BTreeSet::new();
    let mut other_size = rules.iter().filter(|r| key(r, dir).len() <= depth).count();
    let mut by_size: Vec<(usize, char)> = counts.iter().map(|(c, n)| (*n, *c)).collect();
    by_size.sort();
    for (n, c) in by_size {
        if other_size + n > LEAF_MAX {
            break;
        }
        other_size += n;
        merged.insert(c);
    }
    let mut children = Vec::new();
    for c in counts.keys().filter(|c| !merged.contains(c)) {
        let sub: Vec<GRule> = rules
            .iter()
            .filter(|r| {
                let k = key(r, dir);
                k.len() <= depth || k[depth] == *c
            })
            .cloned()
            .collect();
        let mut p = path.to_vec();
        p.push(*c);
        children.push((*c, tree(plan, sub, dir, anchor, depth + 1, &p)));
    }
    let short: Vec<GRule> = rules
        .into_iter()
        .filter(|r| {
            let k = key(r, dir);
            k.len() <= depth || merged.contains(&k[depth])
        })
        .collect();
    let other = (!short.is_empty()).then(|| {
        plan.nodes.push(Node::Leaf(short));
        plan.nodes.len() - 1
    });
    (children, other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexicon::Origin;
    use crate::rules::{Marker, Stage};

    fn rule(
        stem: &str,
        marker: Marker,
        suffix: &str,
        left: Left,
        right: Right,
        target: &str,
    ) -> Rule {
        Rule {
            stage: Stage::Lexicon,
            left,
            stem: stem.into(),
            marker,
            suffix: suffix.into(),
            right,
            target: target.into(),
            origin: Origin::Builtin,
        }
    }

    #[test]
    fn exceptions_come_first() {
        let r = rule(
            "der",
            Marker::Gender,
            "die",
            Left::Boundary,
            Right::WordEnd,
            "der",
        );
        let g = grules(&r).unwrap();
        let shape: Vec<(Edge, Edge, bool)> = g
            .iter()
            .map(|g| (g.before, g.after, g.is_exception()))
            .collect();
        assert_eq!(
            shape,
            [
                (Edge::Letter, Edge::None, true),
                (Edge::MarkerLetter, Edge::None, true),
                (Edge::None, Edge::Letter, true),
                (Edge::None, Edge::MarkerLetter, true),
                (Edge::None, Edge::None, false),
            ]
        );
        assert_eq!(g[0].backtrack, ['r', 'e', 'd']);
        assert_eq!(g[0].kind, Kind::Marker);
    }

    #[test]
    fn kinds_and_passes() {
        let stem = rule(
            "Ärzt",
            Marker::Gender,
            "in",
            Left::Any,
            Right::WordEnd,
            "Arzt",
        );
        assert_eq!(grules(&stem).unwrap()[0].kind, Kind::Stem);
        let binnen = rule(
            "Kolleg",
            Marker::BinnenI,
            "Innen",
            Left::Any,
            Right::Any,
            "Kollegen",
        );
        assert_eq!(grules(&binnen).unwrap()[0].kind, Kind::Binnen);
        let fallback = rule(
            "",
            Marker::GenderText,
            "innen",
            Left::Letter,
            Right::Any,
            "",
        );
        let g = grules(&fallback).unwrap();
        // the gate already requires the letter before the marker
        assert_eq!((g[0].kind, g[0].before), (Kind::Text, Edge::None));
        // a stem anchor that is no umlaut cannot be compiled
        let herzog = rule(
            "Herzog",
            Marker::Gender,
            "innen",
            Left::Any,
            Right::Any,
            "Herzöge",
        );
        assert!(grules(&herzog).is_none());
    }

    #[test]
    fn big_groups_split_into_tree() {
        let stems: Vec<String> = (0..60)
            .map(|i| format!("St{}", char::from(b'a' + (i % 26) as u8)).repeat(1 + i / 26))
            .collect();
        let rules: Vec<Rule> = stems
            .iter()
            .map(|s| rule(s, Marker::Gender, "innen", Left::Any, Right::Any, s))
            .collect();
        let p = plan(&rules);
        assert_eq!(p.marker_gate.len(), 1);
        let leaves: Vec<&Vec<GRule>> = p
            .nodes
            .iter()
            .filter_map(|n| match n {
                Node::Leaf(l) => Some(l),
                Node::Branch { .. } => None,
            })
            .collect();
        assert!(leaves.iter().all(|l| l.len() <= LEAF_MAX));
        assert_eq!(leaves.iter().map(|l| l.len()).sum::<usize>(), 60);
    }
}
