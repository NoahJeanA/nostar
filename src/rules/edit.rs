//! Edit script, anchor and pass of a rule (TECHNIK.md 9.1).

use super::{Atom, Rule};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Keep,
    Deumlaut,
    /// Replace by a non-empty sequence.
    Replace(String),
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pass {
    /// Anchor in the stem; runs first.
    Stem,
    /// Anchor at the marker or later.
    Marker,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// One operation per source position.
    pub ops: Vec<Op>,
    /// First position whose operation is not `Keep`.
    pub anchor: usize,
    pub pass: Pass,
}

pub fn deumlaut(c: char) -> Option<char> {
    match c {
        'ä' => Some('a'),
        'ö' => Some('o'),
        'ü' => Some('u'),
        'Ä' => Some('A'),
        'Ö' => Some('O'),
        'Ü' => Some('U'),
        _ => None,
    }
}

/// `None` if the target equals the source, so the rule changes nothing.
pub fn edit(rule: &Rule) -> Option<Edit> {
    let source = rule.pattern();
    let target: Vec<char> = rule.target.chars().collect();
    let mut ops = Vec::with_capacity(source.len());
    for (atom, t) in source.iter().zip(&target) {
        match atom {
            Atom::Char(c) if c == t => ops.push(Op::Keep),
            Atom::Char(c) if deumlaut(*c) == Some(*t) => ops.push(Op::Deumlaut),
            _ => break,
        }
    }
    let common = ops.len();
    let rest: String = target[common..].iter().collect();
    if common < source.len() {
        ops.push(if rest.is_empty() {
            Op::Delete
        } else {
            Op::Replace(rest)
        });
        ops.resize(source.len(), Op::Delete);
    } else if !rest.is_empty() {
        // The source is a prefix of the target: the last position carries the rest.
        let last = ops.pop()?;
        let Atom::Char(c) = source[common - 1] else {
            return None;
        };
        let c = if last == Op::Deumlaut {
            deumlaut(c).unwrap_or(c)
        } else {
            c
        };
        ops.push(Op::Replace(format!("{c}{rest}")));
    }
    let anchor = ops.iter().position(|o| *o != Op::Keep)?;
    let pass = if anchor < rule.stem_len() {
        Pass::Stem
    } else {
        Pass::Marker
    };
    Some(Edit { ops, anchor, pass })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexicon::Origin;
    use crate::rules::{Left, Marker, Right, Stage};

    fn rule(stem: &str, marker: Marker, suffix: &str, target: &str) -> Rule {
        Rule {
            stage: Stage::Lexicon,
            left: Left::Any,
            stem: stem.into(),
            marker,
            suffix: suffix.into(),
            right: Right::Any,
            target: target.into(),
            origin: Origin::Builtin,
        }
    }

    fn ops_from_anchor(r: &Rule) -> (usize, Vec<Op>, Pass) {
        let e = edit(r).unwrap();
        (e.anchor, e.ops[e.anchor..].to_vec(), e.pass)
    }

    use Op::*;

    #[test]
    fn examples_from_technik_9_1() {
        let d = |n| vec![Delete; n];
        assert_eq!(
            ops_from_anchor(&rule("Lehrer", Marker::Gender, "innen", "Lehrer")),
            (6, d(6), Pass::Marker)
        );
        let mut kollegen = vec![Replace("en".into())];
        kollegen.extend(d(5));
        assert_eq!(
            ops_from_anchor(&rule("Kolleg", Marker::Gender, "innen", "Kollegen")),
            (6, kollegen, Pass::Marker)
        );
        let mut aerzte = vec![Replace("e".into())];
        aerzte.extend(d(5));
        assert_eq!(
            ops_from_anchor(&rule("Ärzt", Marker::Gender, "innen", "Ärzte")),
            (4, aerzte, Pass::Marker)
        );
        assert_eq!(
            ops_from_anchor(&rule("Ärzt", Marker::Gender, "in", "Arzt")),
            (
                0,
                vec![Deumlaut, Keep, Keep, Keep, Delete, Delete, Delete],
                Pass::Stem
            )
        );
        let mut binnen = vec![Replace("en".into())];
        binnen.extend(d(4));
        assert_eq!(
            ops_from_anchor(&rule("Kolleg", Marker::BinnenI, "Innen", "Kollegen")),
            (6, binnen, Pass::Marker)
        );
    }

    #[test]
    fn slash_hyphen_and_prefix_target() {
        assert_eq!(
            ops_from_anchor(&rule("e", Marker::GenderText, "r", "er")),
            (1, vec![Replace("r".into()), Delete], Pass::Marker)
        );
        assert_eq!(
            ops_from_anchor(&rule("Kund", Marker::SlashHyphen, "in", "Kunde")).1,
            vec![Replace("e".into()), Delete, Delete, Delete]
        );
        // stage 5 "cut": everything from the marker on disappears
        assert_eq!(
            ops_from_anchor(&rule("", Marker::GenderText, "in", "")),
            (0, vec![Delete; 3], Pass::Marker)
        );
    }
}
