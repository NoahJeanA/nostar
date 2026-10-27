//! Plan -> GSUB lookups for one font (TECHNIK.md 9.3, 9.4).
//!
//! Lookup order: stem gate, marker gate, Delete, Deumlaut, Replace_k, then the plan's
//! nodes in plan order.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use write_fonts::tables::gsub::{MultipleSubst, Sequence, SingleSubst, SubstitutionLookup};
use write_fonts::tables::layout::{
    ChainedClassSequenceRule, ChainedClassSequenceRuleSet, ChainedSequenceContext, ClassDef,
    CoverageTable, Lookup, LookupFlag, SequenceLookupRecord,
};
use write_fonts::types::GlyphId16;

use super::FontError;
use super::glyphs::Glyphs;
use crate::plan::{Anchor, Dir, Edge, GRule, Kind, MARKER_KEY, Node, Plan, Root};
use crate::rules::edit::{Op, deumlaut};
use crate::rules::engine::fold;
use crate::rules::{Atom, Class, Markers};

const DELETE: u16 = 2;
const DEUMLAUT: u16 = 3;
const REPLACE0: u16 = 4;

/// Subtables are split before they reach this estimated size (TECHNIK.md 9.4).
const SUBTABLE_LIMIT: usize = 40_000;

pub struct Compiled {
    pub lookups: Vec<SubstitutionLookup>,
    /// GSUB rules written.
    pub rules: usize,
    /// GSUB rules left out because the font lacks a glyph.
    pub dropped: usize,
}

type Glyphset = BTreeSet<GlyphId16>;

/// Glyph classes of a format 2 table.
#[derive(Default)]
struct Classes {
    of: HashMap<Key, u16>,
    glyphs: BTreeMap<GlyphId16, u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Char(char),
    Gender,
    /// `* : ·`, the gender markers of `GenderText`.
    TextMarker,
    /// `/` and `_` when active.
    CodeMarker,
    Slash,
    Letter,
    Hyphen,
}

impl Classes {
    /// Class of `key` with `glyphs`; a glyph already in another class is a conflict.
    fn add(&mut self, key: Key, glyphs: &Glyphset) -> Result<u16, FontError> {
        let next = self.of.len() as u16 + 1;
        let class = *self.of.entry(key).or_insert(next);
        for g in glyphs {
            match self.glyphs.insert(*g, class) {
                Some(old) if old != class => {
                    return Err(FontError::Compile(format!(
                        "glyph {} in two classes ({key:?})",
                        g.to_u16()
                    )));
                }
                _ => {}
            }
        }
        Ok(class)
    }

    fn def(&self) -> ClassDef {
        self.glyphs.iter().map(|(g, c)| (*g, *c)).collect()
    }
}

struct Ctx<'a> {
    g: &'a Glyphs,
    gender: Glyphset,
    text: Glyphset,
    /// `/` when only `/-` is active: its own class (TECHNIK.md 9.4).
    slash: Glyphset,
    letters: Glyphset,
    /// Backtrack classes: one per character, case sensitive.
    bt: Classes,
    /// Lookahead classes of the stem tree: one per character, markers as one class.
    fwd: Classes,
    /// Input classes per table; `Text` shares the table of `Marker`.
    input: HashMap<Kind, Classes>,
    /// Leaf lookahead: LETTER = 1, gender marker = 2.
    la: Classes,
    delete: Glyphset,
    deumlaut: BTreeMap<GlyphId16, GlyphId16>,
    replace: Vec<BTreeMap<GlyphId16, Vec<GlyphId16>>>,
}

/// A GSUB rule with its records, ready to be placed.
struct Prepared<'a> {
    rule: &'a GRule,
    records: Vec<SequenceLookupRecord>,
}

pub fn compile(plan: &Plan, g: &Glyphs, markers: &Markers) -> Result<Compiled, FontError> {
    let set =
        |cs: &mut dyn Iterator<Item = char>| -> Glyphset { cs.filter_map(|c| g.get(c)).collect() };
    let gender = set(&mut markers.gender.iter().copied());
    let text = set(&mut markers
        .gender
        .iter()
        .copied()
        .filter(|c| !matches!(c, '/' | '_')));
    let slash = if markers.slash_hyphen && !markers.gender.contains(&'/') {
        set(&mut std::iter::once('/'))
    } else {
        Glyphset::new()
    };
    // The soft hyphen counts as a letter at word boundaries (TECHNIK.md 8.5).
    let mut letters = g.letters.clone();
    letters.extend(g.get('\u{AD}'));
    let mut ctx = Ctx {
        g,
        gender,
        text,
        slash,
        letters,
        bt: Classes::default(),
        fwd: Classes::default(),
        input: HashMap::new(),
        la: Classes::default(),
        delete: Glyphset::new(),
        deumlaut: BTreeMap::new(),
        replace: Vec::new(),
    };
    let letters = ctx.letters.clone();
    ctx.la.add(Key::Letter, &letters)?;
    let gender = ctx.gender.clone();
    ctx.la.add(Key::Gender, &gender)?;

    // Records first: they fix the number of Replace_k lookups before the nodes.
    let mut leaves: Vec<Option<Vec<Prepared>>> = Vec::with_capacity(plan.nodes.len());
    let mut dropped = 0;
    let mut written = 0;
    for node in &plan.nodes {
        let Node::Leaf(rules) = node else {
            leaves.push(None);
            continue;
        };
        let mut prepared = Vec::new();
        for r in rules {
            match prepare(&mut ctx, r)? {
                Some(p) => prepared.push(p),
                None => dropped += 1,
            }
        }
        written += prepared.len();
        leaves.push(Some(prepared));
    }
    let node_base = REPLACE0 + ctx.replace.len() as u16;

    let mut lookups = vec![
        stem_gate(&mut ctx, plan, node_base)?,
        marker_gate(&ctx, plan, node_base)?,
    ];
    lookups.extend(helpers(&ctx));
    for (node, leaf) in plan.nodes.iter().zip(&leaves) {
        let lookup = match (node, leaf) {
            (Node::Leaf(_), Some(prepared)) => leaf_lookup(&mut ctx, prepared)?,
            (
                Node::Branch {
                    dir,
                    anchor,
                    path,
                    children,
                    other,
                },
                _,
            ) => branch(&mut ctx, *dir, *anchor, path, children, *other, node_base)?,
            _ => chain(Vec::new()),
        };
        lookups.push(lookup);
    }
    Ok(Compiled {
        lookups,
        rules: written,
        dropped,
    })
}

impl Ctx<'_> {
    fn one(&self, c: char) -> Option<GlyphId16> {
        self.g.get(c)
    }

    /// Glyphs an input position accepts. Marker-pass letters after the anchor are folded.
    fn atom(&self, kind: Kind, pos: usize, atom: &Atom) -> Glyphset {
        match atom {
            Atom::Char(c) if kind != Kind::Stem && pos > 0 => variants(*c)
                .into_iter()
                .filter_map(|v| self.one(v))
                .collect(),
            Atom::Char(c) => self.one(*c).into_iter().collect(),
            Atom::Marker(Class::Gender) => self.gender.clone(),
            Atom::Marker(Class::Text) => self.text.clone(),
            Atom::Marker(Class::SlashFirst) => self.gender.union(&self.slash).copied().collect(),
        }
    }

    fn anchor(&self, a: Anchor) -> Glyphset {
        match a {
            Anchor::Root(Root::Marker) => self.gender.union(&self.slash).copied().collect(),
            Anchor::Root(Root::BinnenI) => self.one('I').into_iter().collect(),
            Anchor::Char(c) => self.one(c).into_iter().collect(),
        }
    }

    fn bt_class(&mut self, c: char) -> Result<Option<u16>, FontError> {
        match self.one(c) {
            Some(g) => self.bt.add(Key::Char(c), &[g].into()).map(Some),
            None => Ok(None),
        }
    }

    fn fwd_class(&mut self, c: char) -> Result<Option<u16>, FontError> {
        if c == MARKER_KEY {
            let all: Glyphset = self.gender.union(&self.slash).copied().collect();
            return self.fwd.add(Key::Gender, &all).map(Some);
        }
        match self.one(c) {
            Some(g) => self.fwd.add(Key::Char(c), &[g].into()).map(Some),
            None => Ok(None),
        }
    }

    /// Input class alternatives of a position. Marker glyphs are split into `* : ·`,
    /// `/ _` and a lone `/`, so that marker and text rules share one class table and
    /// one subtable per leaf.
    fn input_classes(
        &mut self,
        kind: Kind,
        pos: usize,
        atom: &Atom,
    ) -> Result<Vec<u16>, FontError> {
        let code: Glyphset = self.gender.difference(&self.text).copied().collect();
        let mut keys: Vec<(Key, Glyphset)> = Vec::new();
        match atom {
            Atom::Char(c) => {
                let key = if kind != Kind::Stem && pos > 0 {
                    Key::Char(fold(*c))
                } else {
                    Key::Char(*c)
                };
                keys.push((key, self.atom(kind, pos, atom)));
            }
            Atom::Marker(class) => {
                keys.push((Key::TextMarker, self.text.clone()));
                if *class != Class::Text {
                    keys.push((Key::CodeMarker, code));
                }
                if *class == Class::SlashFirst {
                    keys.push((Key::Slash, self.slash.clone()));
                }
            }
        }
        let classes = self.input.entry(table(kind)).or_default();
        keys.into_iter()
            .filter(|(_, glyphs)| !glyphs.is_empty())
            .map(|(k, glyphs)| classes.add(k, &glyphs))
            .collect()
    }

    /// Replace_k packing: each glyph has at most one sequence per lookup.
    fn replace_slot(&mut self, glyphs: &Glyphset, seq: &[GlyphId16]) -> u16 {
        let fits = |m: &BTreeMap<GlyphId16, Vec<GlyphId16>>| {
            glyphs.iter().all(|g| m.get(g).is_none_or(|s| s == seq))
        };
        let k = match self.replace.iter().position(fits) {
            Some(k) => k,
            None => {
                self.replace.push(BTreeMap::new());
                self.replace.len() - 1
            }
        };
        for g in glyphs {
            self.replace[k].insert(*g, seq.to_vec());
        }
        k as u16
    }
}

/// A letter and its single-character case mappings.
fn variants(c: char) -> Vec<char> {
    let mut v = vec![c];
    for mapped in [
        c.to_lowercase().collect::<Vec<_>>(),
        c.to_uppercase().collect(),
    ] {
        if let [one] = mapped[..]
            && !v.contains(&one)
        {
            v.push(one);
        }
    }
    v
}

/// Records from the last input position to the first, each at its own match position;
/// `None` if a glyph is missing. HarfBuzz keeps the positions before a changed glyph, so
/// a skipped soft hyphen inside the match is never hit (TECHNIK.md 9.4).
fn prepare<'a>(ctx: &mut Ctx, r: &'a GRule) -> Result<Option<Prepared<'a>>, FontError> {
    if r.backtrack.iter().any(|c| ctx.one(*c).is_none())
        || r.input
            .iter()
            .enumerate()
            .any(|(i, a)| ctx.atom(r.kind, i, a).is_empty())
    {
        return Ok(None);
    }
    let mut records = Vec::new();
    for (pos, op) in r.action.iter().rev() {
        let index =
            u16::try_from(*pos).map_err(|_| FontError::Compile(format!("sequence index {pos}")))?;
        let glyphs = ctx.atom(r.kind, *pos, &r.input[*pos]);
        let lookup = match op {
            Op::Keep => continue,
            Op::Delete => {
                ctx.delete.extend(&glyphs);
                DELETE
            }
            Op::Deumlaut => {
                let Atom::Char(c) = r.input[*pos] else {
                    return Ok(None);
                };
                let (Some(from), Some(to)) = (ctx.one(c), deumlaut(c).and_then(|d| ctx.one(d)))
                else {
                    return Ok(None);
                };
                ctx.deumlaut.insert(from, to);
                DEUMLAUT
            }
            Op::Replace(s) => {
                let Some(seq) = s.chars().map(|c| ctx.one(c)).collect::<Option<Vec<_>>>() else {
                    return Ok(None);
                };
                REPLACE0 + ctx.replace_slot(&glyphs, &seq)
            }
        };
        records.push(SequenceLookupRecord::new(index, lookup));
    }
    Ok(Some(Prepared { rule: r, records }))
}

/// Class table of a kind.
fn table(kind: Kind) -> Kind {
    match kind {
        Kind::Text => Kind::Marker,
        k => k,
    }
}

fn flag() -> LookupFlag {
    LookupFlag::IGNORE_MARKS
}

fn chain(subtables: Vec<ChainedSequenceContext>) -> SubstitutionLookup {
    SubstitutionLookup::ChainContextual(Lookup::new(
        flag(),
        subtables.into_iter().map(Into::into).collect(),
    ))
}

fn helpers(ctx: &Ctx) -> Vec<SubstitutionLookup> {
    let delete_cov: CoverageTable = ctx.delete.iter().copied().collect();
    let delete_seqs = ctx.delete.iter().map(|_| Sequence::new(vec![])).collect();
    let mut out = vec![SubstitutionLookup::Multiple(Lookup::new(
        LookupFlag::empty(),
        vec![MultipleSubst::format_1(delete_cov, delete_seqs)],
    ))];
    let cov: CoverageTable = ctx.deumlaut.keys().copied().collect();
    out.push(SubstitutionLookup::Single(Lookup::new(
        LookupFlag::empty(),
        vec![SingleSubst::format_2(
            cov,
            ctx.deumlaut.values().copied().collect(),
        )],
    )));
    for m in &ctx.replace {
        let cov: CoverageTable = m.keys().copied().collect();
        let seqs = m.values().map(|s| Sequence::new(s.clone())).collect();
        out.push(SubstitutionLookup::Multiple(Lookup::new(
            LookupFlag::empty(),
            vec![MultipleSubst::format_1(cov, seqs)],
        )));
    }
    out
}

/// Stem pass root: umlaut followed by the full prefix up to the marker (TECHNIK.md 9.3).
fn stem_gate(ctx: &mut Ctx, plan: &Plan, base: u16) -> Result<SubstitutionLookup, FontError> {
    let mut input = Classes::default();
    let mut sets: Vec<Option<ChainedClassSequenceRuleSet>> = vec![None];
    for (anchor, children) in &plan.stem_gate {
        let Some(glyph) = ctx.one(*anchor) else {
            continue;
        };
        let class = input.add(Key::Char(*anchor), &[glyph].into())?;
        let mut rules = Vec::new();
        'child: for (prefix, node) in children {
            let mut la = Vec::new();
            for c in prefix {
                match ctx.fwd_class(*c)? {
                    Some(cls) => la.push(cls),
                    None => continue 'child,
                }
            }
            let rec = vec![SequenceLookupRecord::new(0, base + *node as u16)];
            rules.push(ChainedClassSequenceRule::new(vec![], vec![], la, rec));
        }
        if sets.len() <= class as usize {
            sets.resize(class as usize + 1, None);
        }
        sets[class as usize] = Some(ChainedClassSequenceRuleSet::new(rules));
    }
    let cov: CoverageTable = input.glyphs.keys().copied().collect();
    Ok(chain(vec![ChainedSequenceContext::format_2(
        cov,
        ClassDef::default(),
        input.def(),
        ctx.fwd.def(),
        sets,
    )]))
}

/// Marker pass root: anchor with a letter before and a letter or '-' after it,
/// dispatched by the last stem letter.
fn marker_gate(ctx: &Ctx, plan: &Plan, base: u16) -> Result<SubstitutionLookup, FontError> {
    let mut keys: BTreeSet<char> = BTreeSet::new();
    for (_, children, _) in &plan.marker_gate {
        keys.extend(children.iter().map(|c| c.0));
    }
    let mut bt = Classes::default();
    let mut key_class = HashMap::new();
    for c in &keys {
        if let Some(g) = ctx.one(*c) {
            key_class.insert(*c, bt.add(Key::Char(*c), &[g].into())?);
        }
    }
    // Without the soft hyphen: HarfBuzz skips it here and sees the stem letter before it.
    let shy = ctx.one('\u{AD}');
    let other_letters: Glyphset = ctx
        .letters
        .iter()
        .copied()
        .filter(|g| !bt.glyphs.contains_key(g) && Some(*g) != shy)
        .collect();
    let other = bt.add(Key::Letter, &other_letters)?;
    let mut la = Classes::default();
    la.add(Key::Letter, &ctx.letters)?;
    let hyphen: Glyphset = ctx.one('-').into_iter().collect();
    la.add(Key::Hyphen, &hyphen)?;
    let mut input = Classes::default();
    let mut sets: Vec<Option<ChainedClassSequenceRuleSet>> = vec![None];
    for (root, children, other_node) in &plan.marker_gate {
        let glyphs = ctx.anchor(Anchor::Root(*root));
        if glyphs.is_empty() {
            continue;
        }
        let class = input.add(
            Key::Char(if *root == Root::BinnenI {
                'I'
            } else {
                MARKER_KEY
            }),
            &glyphs,
        )?;
        let own: HashMap<char, usize> = children.iter().copied().collect();
        let mut rules = Vec::new();
        let mut push = |bt_class: u16, node: usize| {
            for la_class in [1, 2] {
                let rec = vec![SequenceLookupRecord::new(0, base + node as u16)];
                rules.push(ChainedClassSequenceRule::new(
                    vec![bt_class],
                    vec![],
                    vec![la_class],
                    rec,
                ));
            }
        };
        // Keys of another root fall back to this root's `other`.
        for (c, cls) in &key_class {
            if let Some(node) = own.get(c).copied().or(*other_node) {
                push(*cls, node);
            }
        }
        if let Some(node) = other_node {
            push(other, *node);
        }
        if sets.len() <= class as usize {
            sets.resize(class as usize + 1, None);
        }
        sets[class as usize] = Some(ChainedClassSequenceRuleSet::new(rules));
    }
    let cov: CoverageTable = input.glyphs.keys().copied().collect();
    Ok(chain(vec![ChainedSequenceContext::format_2(
        cov,
        bt.def(),
        input.def(),
        la.def(),
        sets,
    )]))
}

fn branch(
    ctx: &mut Ctx,
    dir: Dir,
    anchor: Anchor,
    path: &[char],
    children: &[(char, usize)],
    other: Option<usize>,
    base: u16,
) -> Result<SubstitutionLookup, FontError> {
    let anchor_glyphs = ctx.anchor(anchor);
    let mut input = Classes::default();
    input.add(Key::Gender, &anchor_glyphs)?;
    let mut rules = Vec::new();
    let targets = children
        .iter()
        .map(|(c, n)| (Some(*c), *n))
        .chain(other.map(|n| (None, n)));
    'target: for (c, node) in targets {
        let mut seq = Vec::new();
        for ch in path.iter().chain(c.iter()) {
            let cls = match dir {
                Dir::Back => ctx.bt_class(*ch)?,
                Dir::Fwd => ctx.fwd_class(*ch)?,
            };
            match cls {
                Some(cls) => seq.push(cls),
                None => continue 'target,
            }
        }
        let rec = vec![SequenceLookupRecord::new(0, base + node as u16)];
        rules.push(match dir {
            Dir::Back => ChainedClassSequenceRule::new(seq, vec![], vec![], rec),
            Dir::Fwd => ChainedClassSequenceRule::new(vec![], vec![], seq, rec),
        });
    }
    let cov: CoverageTable = anchor_glyphs.iter().copied().collect();
    let (bt, la) = match dir {
        Dir::Back => (ctx.bt.def(), ClassDef::default()),
        Dir::Fwd => (ClassDef::default(), ctx.fwd.def()),
    };
    let sets = vec![None, Some(ChainedClassSequenceRuleSet::new(rules))];
    Ok(chain(vec![ChainedSequenceContext::format_2(
        cov,
        bt,
        input.def(),
        la,
        sets,
    )]))
}

/// Runs of one kind become format 2 subtables; rules with context before the stem
/// (left boundary, letter before) need LETTER in the backtrack and become format 3.
fn leaf_lookup(ctx: &mut Ctx, rules: &[Prepared]) -> Result<SubstitutionLookup, FontError> {
    let mut subtables = Vec::new();
    let mut run: Vec<&Prepared> = Vec::new();
    let mut run_size = 0;
    for p in rules {
        let r = p.rule;
        if r.before != Edge::None {
            flush(ctx, &mut run, &mut subtables)?;
            run_size = 0;
            subtables.push(format3(ctx, p));
            continue;
        }
        let size = 12 + 2 * (r.backtrack.len() + r.input.len() + 2) + 4 * p.records.len();
        if run_size + size > SUBTABLE_LIMIT
            || run
                .first()
                .is_some_and(|f| table(f.rule.kind) != table(r.kind))
        {
            flush(ctx, &mut run, &mut subtables)?;
            run_size = 0;
        }
        run.push(p);
        run_size += size;
    }
    flush(ctx, &mut run, &mut subtables)?;
    Ok(chain(subtables))
}

fn after_classes(e: Edge) -> Vec<u16> {
    match e {
        Edge::None => vec![],
        Edge::Letter => vec![1],
        Edge::MarkerLetter => vec![2, 1],
    }
}

fn flush(
    ctx: &mut Ctx,
    run: &mut Vec<&Prepared>,
    subtables: &mut Vec<ChainedSequenceContext>,
) -> Result<(), FontError> {
    let Some(first) = run.first() else {
        return Ok(());
    };
    let mut sets: BTreeMap<u16, Vec<ChainedClassSequenceRule>> = BTreeMap::new();
    for p in run.iter() {
        let r = p.rule;
        let mut bt = Vec::new();
        for c in &r.backtrack {
            match ctx.bt_class(*c)? {
                Some(cls) => bt.push(cls),
                None => return Err(FontError::Compile(format!("no backtrack class for {c:?}"))),
            }
        }
        // Expand `SlashFirst` alternatives into separate rules.
        let mut seqs: Vec<Vec<u16>> = vec![vec![]];
        for (i, a) in r.input.iter().enumerate() {
            let alts = ctx.input_classes(r.kind, i, a)?;
            seqs = seqs
                .iter()
                .flat_map(|s| {
                    alts.iter().map(move |c| {
                        let mut n = s.clone();
                        n.push(*c);
                        n
                    })
                })
                .collect();
        }
        for seq in seqs {
            let Some((first, rest)) = seq.split_first() else {
                continue;
            };
            sets.entry(*first)
                .or_default()
                .push(ChainedClassSequenceRule::new(
                    bt.clone(),
                    rest.to_vec(),
                    after_classes(r.after),
                    p.records.clone(),
                ));
        }
    }
    let classes = ctx.input.entry(table(first.rule.kind)).or_default();
    let max = sets.keys().max().copied().unwrap_or(0) as usize;
    let mut rule_sets: Vec<Option<ChainedClassSequenceRuleSet>> = vec![None; max + 1];
    let firsts: BTreeSet<u16> = sets.keys().copied().collect();
    for (k, v) in sets {
        rule_sets[k as usize] = Some(ChainedClassSequenceRuleSet::new(v));
    }
    let cov: CoverageTable = classes
        .glyphs
        .iter()
        .filter(|(_, c)| firsts.contains(c))
        .map(|(g, _)| *g)
        .collect();
    let input_def = classes.def();
    subtables.push(ChainedSequenceContext::format_2(
        cov,
        ctx.bt.def(),
        input_def,
        ctx.la.def(),
        rule_sets,
    ));
    run.clear();
    Ok(())
}

fn format3(ctx: &Ctx, p: &Prepared) -> ChainedSequenceContext {
    let r = p.rule;
    let cov = |glyphs: Glyphset| -> CoverageTable { glyphs.into_iter().collect() };
    let mut bt: Vec<CoverageTable> = r
        .backtrack
        .iter()
        .map(|c| cov(ctx.one(*c).into_iter().collect()))
        .collect();
    match r.before {
        Edge::None => {}
        Edge::Letter => bt.push(cov(ctx.letters.clone())),
        Edge::MarkerLetter => {
            bt.push(cov(ctx.gender.clone()));
            bt.push(cov(ctx.letters.clone()));
        }
    }
    let input = r
        .input
        .iter()
        .enumerate()
        .map(|(i, a)| cov(ctx.atom(r.kind, i, a)))
        .collect();
    let la = match r.after {
        Edge::None => vec![],
        Edge::Letter => vec![cov(ctx.letters.clone())],
        Edge::MarkerLetter => vec![cov(ctx.gender.clone()), cov(ctx.letters.clone())],
    };
    ChainedSequenceContext::format_3(bt, input, la, p.records.clone())
}
