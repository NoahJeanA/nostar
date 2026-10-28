//! Inserts the new lookups into the font's GSUB (TECHNIK.md 9.5).

use std::collections::HashMap;

use read_fonts::{FontRef, TableProvider};
use write_fonts::FontBuilder;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::gsub::{
    ExtensionSubstFormat1, ExtensionSubtable, Gsub, SubstitutionLookup, SubstitutionLookupList,
};
use write_fonts::tables::layout::{
    ChainedSequenceContext, Feature, FeatureList, FeatureRecord, LangSys, Lookup, Script,
    ScriptList, ScriptRecord, SequenceContext, SequenceLookupRecord,
};
use write_fonts::types::Tag;

use super::FontError;

const CALT: Tag = Tag::new(b"calt");
const DFLT: Tag = Tag::new(b"DFLT");
const LATN: Tag = Tag::new(b"latn");

/// Font with `new` lookups in front of the existing ones and the two gates (the first two
/// lookups) in `calt`; all other tables unchanged.
pub fn insert(font: &FontRef, new: Vec<SubstitutionLookup>) -> Result<Vec<u8>, FontError> {
    let n = u16::try_from(new.len())
        .map_err(|_| FontError::Compile(format!("{} lookups", new.len())))?;
    let mut gsub: Gsub = match font.gsub() {
        Ok(t) => t.to_owned_table(),
        Err(_) => Gsub::new(
            ScriptList::default(),
            FeatureList::default(),
            SubstitutionLookupList::default(),
        ),
    };
    if gsub.script_list2.is_some() || gsub.feature_list2.is_some() || gsub.lookup_list2.is_some() {
        return Err(FontError::Unsupported(
            "GSUB 1.2 with 32-bit offsets".into(),
        ));
    }
    let lookup_list = gsub
        .lookup_list
        .as_mut()
        .ok_or_else(|| FontError::Unsupported("GSUB without lookup list".into()))?;
    let mut old: Vec<SubstitutionLookup> = lookup_list
        .lookups
        .drain(..)
        .map(|l| (*l).clone())
        .collect();
    for l in &mut old {
        shift_lookup(l, n)?;
    }
    let mut all = new;
    all.extend(old);
    // Every lookup as extension: keeps the 16-bit offsets small even with thousands of
    // lookups in front of a large existing GSUB.
    *lookup_list = SubstitutionLookupList::new(all.into_iter().map(to_extension).collect());

    let needs_calt = lang_sys_without_calt(&gsub);
    let (remap, new_calt) = features(&mut gsub, n, needs_calt)?;
    feature_variations(&mut gsub, n, &remap)?;
    scripts(&mut gsub, &remap, new_calt)?;

    let mut b = FontBuilder::new();
    b.add_table(&gsub)
        .map_err(|e| FontError::Write(format!("GSUB: {e}")))?;
    b.copy_missing_tables(font.clone());
    Ok(b.build())
}

/// True if a language system has no `calt` or there is no `DFLT` or `latn` script, so a
/// `calt` with only the gates is needed (TECHNIK.md 9.5).
fn lang_sys_without_calt(gsub: &Gsub) -> bool {
    let calt = calt_indices(gsub);
    let Some(sl) = gsub.script_list.as_ref() else {
        return true;
    };
    if !sl
        .script_records
        .iter()
        .any(|r| r.script_tag == DFLT || r.script_tag == LATN)
    {
        return true;
    }
    let lacks = |ls: &LangSys| !has_calt(ls, |i| calt.contains(&i));
    sl.script_records.iter().any(|sr| {
        sr.script.default_lang_sys.as_ref().is_some_and(lacks)
            || sr
                .script
                .lang_sys_records
                .iter()
                .any(|lr| lacks(&lr.lang_sys))
    })
}

fn calt_indices(gsub: &Gsub) -> Vec<u16> {
    gsub.feature_list
        .as_ref()
        .map(|fl| {
            fl.feature_records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.feature_tag == CALT)
                .map(|(i, _)| i as u16)
                .collect()
        })
        .unwrap_or_default()
}

fn has_calt(ls: &LangSys, is_calt: impl Fn(u16) -> bool) -> bool {
    ls.feature_indices.iter().any(|&i| is_calt(i))
        || (ls.required_feature_index != 0xFFFF && is_calt(ls.required_feature_index))
}

/// Shifts lookup indices, puts the gates in front of every `calt` and adds a `calt` with
/// only the gates if `needs_calt`. Returns the old -> new feature index map and the index
/// of the new `calt`.
fn features(
    gsub: &mut Gsub,
    n: u16,
    needs_calt: bool,
) -> Result<(HashMap<u16, u16>, Option<u16>), FontError> {
    let fl = gsub
        .feature_list
        .as_mut()
        .ok_or_else(|| FontError::Unsupported("GSUB without feature list".into()))?;
    let mut records: Vec<(Tag, Feature, Option<u16>)> = Vec::new();
    for (i, r) in fl.feature_records.iter().enumerate() {
        let mut f = (*r.feature).clone();
        shift_indices(&mut f.lookup_list_indices, n)?;
        if r.feature_tag == CALT {
            f.lookup_list_indices.splice(0..0, [0, 1]);
        }
        records.push((r.feature_tag, f, Some(i as u16)));
    }
    if needs_calt {
        records.push((CALT, Feature::new(None, vec![0, 1]), None));
    }
    // The feature list stays sorted by tag.
    records.sort_by_key(|r| r.0);
    let mut remap = HashMap::new();
    let mut new_calt = None;
    for (new_i, r) in records.iter().enumerate() {
        match r.2 {
            Some(old) => {
                remap.insert(old, new_i as u16);
            }
            None => new_calt = Some(new_i as u16),
        }
    }
    *fl = FeatureList::new(
        records
            .into_iter()
            .map(|(t, f, _)| FeatureRecord::new(t, f))
            .collect(),
    );
    Ok((remap, new_calt))
}

/// Feature variations: new feature indices; alternate features get shifted lookups and,
/// for `calt`, the gates in front (TECHNIK.md 9.5).
fn feature_variations(gsub: &mut Gsub, n: u16, remap: &HashMap<u16, u16>) -> Result<(), FontError> {
    let tags: HashMap<u16, Tag> = gsub
        .feature_list
        .as_ref()
        .map(|fl| {
            fl.feature_records
                .iter()
                .enumerate()
                .map(|(i, r)| (i as u16, r.feature_tag))
                .collect()
        })
        .unwrap_or_default();
    let Some(fv) = gsub.feature_variations.as_mut() else {
        return Ok(());
    };
    if fv
        .lookup_variation_records
        .as_ref()
        .is_some_and(|v| !v.is_empty())
    {
        return Err(FontError::Unsupported("GSUB lookup variations".into()));
    }
    for rec in &mut fv.feature_variation_records {
        let Some(sub) = rec.feature_table_substitution.as_mut() else {
            continue;
        };
        for s in &mut sub.substitutions {
            s.feature_index = *remap.get(&s.feature_index).ok_or_else(|| {
                FontError::Read(format!("feature variation: feature {}", s.feature_index))
            })?;
            let f: &mut Feature = &mut s.alternate_feature;
            shift_indices(&mut f.lookup_list_indices, n)?;
            if tags.get(&s.feature_index) == Some(&CALT) {
                f.lookup_list_indices.splice(0..0, [0, 1]);
            }
        }
    }
    Ok(())
}

/// New feature indices in every language system; the new `calt` goes into every language
/// system without `calt`, with `DFLT` added if there is neither `DFLT` nor `latn`.
fn scripts(
    gsub: &mut Gsub,
    remap: &HashMap<u16, u16>,
    new_calt: Option<u16>,
) -> Result<(), FontError> {
    // the font's own `calt` features, by new index
    let mut calt = calt_indices(gsub);
    calt.retain(|&i| Some(i) != new_calt);
    let sl = gsub
        .script_list
        .as_mut()
        .ok_or_else(|| FontError::Unsupported("GSUB without script list".into()))?;
    if new_calt.is_some()
        && !sl
            .script_records
            .iter()
            .any(|r| r.script_tag == DFLT || r.script_tag == LATN)
    {
        // Only a script of its own: a new `latn` would hide the features under `DFLT`.
        sl.script_records.push(ScriptRecord::new(
            DFLT,
            Script::new(Some(LangSys::new(vec![])), vec![]),
        ));
        sl.script_records.sort_by_key(|r| r.script_tag);
    }
    let fix = |ls: &mut LangSys| -> Result<(), FontError> {
        let map = |i: u16| {
            remap
                .get(&i)
                .copied()
                .ok_or_else(|| FontError::Read(format!("language system: feature {i}")))
        };
        for i in ls.feature_indices.iter_mut() {
            *i = map(*i)?;
        }
        if ls.required_feature_index != 0xFFFF {
            ls.required_feature_index = map(ls.required_feature_index)?;
        }
        if let Some(c) = new_calt
            && !has_calt(ls, |i| calt.contains(&i))
        {
            ls.feature_indices.push(c);
            ls.feature_indices.sort();
        }
        Ok(())
    };
    for sr in sl.script_records.iter_mut() {
        if let Some(d) = sr.script.default_lang_sys.as_mut() {
            fix(d)?;
        }
        for lr in sr.script.lang_sys_records.iter_mut() {
            fix(&mut lr.lang_sys)?;
        }
    }
    Ok(())
}

fn shift_indices(indices: &mut [u16], n: u16) -> Result<(), FontError> {
    for i in indices {
        *i = i
            .checked_add(n)
            .ok_or_else(|| FontError::Write("more than 65535 lookups".into()))?;
    }
    Ok(())
}

fn shift_records(recs: &mut [SequenceLookupRecord], n: u16) -> Result<(), FontError> {
    for r in recs {
        r.lookup_list_index = r
            .lookup_list_index
            .checked_add(n)
            .ok_or_else(|| FontError::Write("more than 65535 lookups".into()))?;
    }
    Ok(())
}

fn shift_seq_ctx(ctx: &mut SequenceContext, n: u16) -> Result<(), FontError> {
    match ctx {
        SequenceContext::Format1(f) => {
            for s in f.seq_rule_sets.iter_mut().filter_map(|s| s.as_mut()) {
                for r in s.seq_rules.iter_mut() {
                    shift_records(&mut r.seq_lookup_records, n)?;
                }
            }
        }
        SequenceContext::Format2(f) => {
            for s in f.class_seq_rule_sets.iter_mut().filter_map(|s| s.as_mut()) {
                for r in s.class_seq_rules.iter_mut() {
                    shift_records(&mut r.seq_lookup_records, n)?;
                }
            }
        }
        SequenceContext::Format3(f) => shift_records(&mut f.seq_lookup_records, n)?,
        _ => {
            return Err(FontError::Unsupported(
                "sequence context format 4 or later".into(),
            ));
        }
    }
    Ok(())
}

fn shift_chain_ctx(ctx: &mut ChainedSequenceContext, n: u16) -> Result<(), FontError> {
    match ctx {
        ChainedSequenceContext::Format1(f) => {
            for s in f
                .chained_seq_rule_sets
                .iter_mut()
                .filter_map(|s| s.as_mut())
            {
                for r in s.chained_seq_rules.iter_mut() {
                    shift_records(&mut r.seq_lookup_records, n)?;
                }
            }
        }
        ChainedSequenceContext::Format2(f) => {
            for s in f
                .chained_class_seq_rule_sets
                .iter_mut()
                .filter_map(|s| s.as_mut())
            {
                for r in s.chained_class_seq_rules.iter_mut() {
                    shift_records(&mut r.seq_lookup_records, n)?;
                }
            }
        }
        ChainedSequenceContext::Format3(f) => shift_records(&mut f.seq_lookup_records, n)?,
        _ => {
            return Err(FontError::Unsupported(
                "chained sequence context format 4 or later".into(),
            ));
        }
    }
    Ok(())
}

/// Shifts `SubstLookupRecord`s of contextual lookups (types 5 and 6, also inside type 7).
fn shift_lookup(l: &mut SubstitutionLookup, n: u16) -> Result<(), FontError> {
    match l {
        SubstitutionLookup::Contextual(l) => {
            for s in l.subtables.iter_mut() {
                shift_seq_ctx(s, n)?;
            }
        }
        SubstitutionLookup::ChainContextual(l) => {
            for s in l.subtables.iter_mut() {
                shift_chain_ctx(s, n)?;
            }
        }
        SubstitutionLookup::Extension(l) => {
            for s in l.subtables.iter_mut() {
                match &mut **s {
                    ExtensionSubtable::Contextual(e) => shift_seq_ctx(&mut e.extension, n)?,
                    ExtensionSubtable::ChainContextual(e) => shift_chain_ctx(&mut e.extension, n)?,
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn to_extension(l: SubstitutionLookup) -> SubstitutionLookup {
    fn wrap<T: Clone + Default>(
        l: Lookup<T>,
        ty: u16,
        f: fn(ExtensionSubstFormat1<T>) -> ExtensionSubtable,
    ) -> SubstitutionLookup {
        let subs = l
            .subtables
            .iter()
            .map(|s| f(ExtensionSubstFormat1::new(ty, (**s).clone())))
            .collect();
        let mut n = Lookup::new(l.lookup_flag, subs);
        n.mark_filtering_set = l.mark_filtering_set;
        SubstitutionLookup::Extension(n)
    }
    match l {
        SubstitutionLookup::Single(l) => wrap(l, 1, ExtensionSubtable::Single),
        SubstitutionLookup::Multiple(l) => wrap(l, 2, ExtensionSubtable::Multiple),
        SubstitutionLookup::Alternate(l) => wrap(l, 3, ExtensionSubtable::Alternate),
        SubstitutionLookup::Ligature(l) => wrap(l, 4, ExtensionSubtable::Ligature),
        SubstitutionLookup::Contextual(l) => wrap(l, 5, ExtensionSubtable::Contextual),
        SubstitutionLookup::ChainContextual(l) => wrap(l, 6, ExtensionSubtable::ChainContextual),
        SubstitutionLookup::Reverse(l) => wrap(l, 8, ExtensionSubtable::Reverse),
        e @ SubstitutionLookup::Extension(_) => e,
    }
}
