//! `nostar apply [--dry-run]` (fachl. Spezifikation 10.1, TECHNIK.md 10.2).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use crate::Error;
use crate::config::Config;
use crate::font::{self, select};
use crate::fsutil::{FsError, Lock, sha256_file, write_atomic};
use crate::paths::Paths;
use crate::plan::{self, Plan};
use crate::system::manifest::{Entry, Manifest};
use crate::system::{SystemError, fontconfig};

/// A font to patch and the checksum of its original.
struct Job {
    original: PathBuf,
    sha256: String,
    copy: PathBuf,
}

pub fn apply(paths: &Paths, config: &Config, dry_run: bool) -> Result<ExitCode, Error> {
    // No room for the rule: stop before any change; the file is the user's (TECHNIK.md 10.4).
    for dir in paths.fontconfig_dirs() {
        if dir.exists() && !dir.is_dir() {
            return Err(SystemError::NotADirectory(dir).into());
        }
    }
    let _lock = Lock::acquire(&paths.lock())?;
    let set = super::rules::load(paths, config)?;
    let rules_sha = sha256_file(&paths.rules())?;
    let listed = super::listed(config)?;
    let selection = select::select(&listed, config, paths);
    let mut manifest = Manifest::load(&paths.manifest())?;

    let mut unchanged = Vec::new();
    let mut jobs = Vec::new();
    for original in &selection.fonts {
        let sha256 = sha256_file(original)?;
        let copy = paths.patched_font(original);
        let same = manifest.rules == rules_sha
            && copy.exists()
            && manifest
                .entry(original)
                .is_some_and(|e| e.original_sha256 == sha256 && e.copy == copy);
        if same {
            unchanged.push(original.clone());
        } else {
            jobs.push(Job {
                original: original.clone(),
                sha256,
                copy,
            });
        }
    }
    let selected: HashSet<&PathBuf> = selection.fonts.iter().collect();
    let stale: Vec<Entry> = manifest
        .fonts
        .iter()
        .filter(|e| !selected.contains(&e.original))
        .cloned()
        .collect();

    if dry_run {
        print_dry_run(paths, &jobs, unchanged.len(), &selection.skipped, &stale);
        return Ok(ExitCode::SUCCESS);
    }

    for dir in [paths.fonts_base(), paths.fonts_dir()]
        .into_iter()
        .chain(paths.fontconfig_dirs())
    {
        if !dir.exists() && !manifest.created_dirs.contains(&dir) {
            manifest.created_dirs.push(dir);
        }
    }
    let plan = plan::plan(set.rules());
    let results = patch_all(&jobs, &plan, config);

    let mut changed = false;
    let mut fonts: Vec<Entry> = manifest
        .fonts
        .iter()
        .filter(|e| unchanged.contains(&e.original))
        .cloned()
        .collect();
    let mut failed = Vec::new();
    for (job, result) in jobs.iter().zip(results) {
        match result {
            Ok(()) => {
                changed = true;
                fonts.push(Entry {
                    original: job.original.clone(),
                    original_sha256: job.sha256.clone(),
                    copy: job.copy.clone(),
                });
            }
            Err(reason) => {
                // An old copy would hide nothing but double the font (fachl. Spezifikation 9.2).
                changed |= fontconfig::remove_file(&job.copy)?;
                failed.push((job.original.clone(), reason));
            }
        }
    }
    let mut removed = 0;
    for e in &stale {
        if fontconfig::remove_file(&e.copy)? {
            removed += 1;
        }
    }
    let keep: HashSet<PathBuf> = fonts.iter().map(|e| e.copy.clone()).collect();
    removed += remove_orphans(&paths.fonts_dir(), &keep)?;
    changed |= removed > 0;

    fonts.sort_by(|a, b| a.original.cmp(&b.original));
    let new_manifest = Manifest {
        rules: rules_sha,
        created_dirs: manifest.created_dirs.clone(),
        fonts,
    };
    if new_manifest != Manifest::load(&paths.manifest())? {
        new_manifest.save(&paths.manifest())?;
    }
    let originals: Vec<&Path> = new_manifest
        .fonts
        .iter()
        .map(|e| e.original.as_path())
        .collect();
    changed |= fontconfig::write_rule(&paths.fontconfig_rule(), &originals)?;
    if changed {
        fontconfig::cache_force(&paths.fonts_base())?;
    }
    let copies: Vec<&Path> = new_manifest
        .fonts
        .iter()
        .map(|e| e.copy.as_path())
        .collect();
    if !copies.is_empty() && !fontconfig::unlisted(&copies)?.is_empty() {
        // a stale cache from an earlier run
        fontconfig::cache_force(&paths.fonts_base())?;
        let missing = fontconfig::unlisted(&copies)?;
        if let Some(first) = missing.first() {
            return Err(
                SystemError::Unlisted(missing.len(), copies.len(), first.to_path_buf()).into(),
            );
        }
    }

    let patched = jobs.len() - failed.len();
    println!("{:<10}{:>4}", "patched", patched);
    println!("{:<10}{:>4}", "unchanged", unchanged.len());
    println!(
        "{:<10}{:>4}{}",
        "skipped",
        selection.skipped.len(),
        reasons(&selection.skipped)
    );
    let fail_line = |(p, r): &(PathBuf, String)| paths.tilde(&format!("{}: {r}", p.display()));
    match failed.as_slice() {
        [] => println!("{:<10}{:>4}", "failed", 0),
        [one] => println!("{:<10}{:>4}  {}", "failed", 1, fail_line(one)),
        many => {
            println!("{:<10}{:>4}", "failed", many.len());
            for f in many {
                println!("  {}", fail_line(f));
            }
        }
    }
    if removed > 0 {
        println!("{:<10}{:>4}", "removed", removed);
    }
    if changed {
        println!("{}", super::RESTART);
    }
    Ok(if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// `  (no latin letters: 598, collection: 9)`, most frequent first.
fn reasons(skipped: &[(PathBuf, select::Skip)]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    let mut counts: BTreeMap<select::Skip, usize> = BTreeMap::new();
    for (_, s) in skipped {
        *counts.entry(*s).or_default() += 1;
    }
    let mut v: Vec<(select::Skip, usize)> = counts.into_iter().collect();
    v.sort_by_key(|(s, n)| (std::cmp::Reverse(*n), *s));
    let parts: Vec<String> = v
        .iter()
        .map(|(s, n)| format!("{}: {n}", s.label()))
        .collect();
    format!("  ({})", parts.join(", "))
}

/// Patches in parallel (TECHNIK.md 10.1); writes each copy atomically.
fn patch_all(jobs: &[Job], plan: &Plan, config: &Config) -> Vec<Result<(), String>> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<(), String>>>> = Mutex::new(vec![None; jobs.len()]);
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    std::thread::scope(|s| {
        for _ in 0..threads.min(jobs.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    // A malformed font must not end the run (fachl. Spezifikation 12).
                    let r = std::panic::catch_unwind(|| patch_one(job, plan, config))
                        .unwrap_or_else(|_| Err("internal error while patching".into()));
                    if let Ok(mut all) = results.lock() {
                        all[i] = Some(r);
                    }
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.unwrap_or_else(|| Err("not patched".into())))
        .collect()
}

fn patch_one(job: &Job, plan: &Plan, config: &Config) -> Result<(), String> {
    let start = Instant::now();
    let original = std::fs::read(&job.original)
        .map_err(|e| FsError::io("read", &job.original)(e).to_string())?;
    let patched = match font::patch(&original, plan, config) {
        Ok(p) => p,
        Err(e) => {
            log::warn!(target: "font", "failed {}: {e}", job.original.display());
            return Err(e.to_string());
        }
    };
    write_atomic(&job.copy, &patched.data).map_err(|e| e.to_string())?;
    log::info!(
        target: "font",
        "patched {} rules={} lookups={} size=+{}KiB time={}ms",
        job.original.display(),
        patched.rules,
        patched.lookups,
        patched.data.len().saturating_sub(original.len()) / 1024,
        start.elapsed().as_millis()
    );
    Ok(())
}

/// Files below the nostar font directory that no manifest entry owns (an aborted run);
/// empty directories there go too.
fn remove_orphans(dir: &Path, keep: &HashSet<PathBuf>) -> Result<usize, Error> {
    let mut removed = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            removed += remove_orphans(&path, keep)?;
            if std::fs::remove_dir(&path).is_ok() {
                log::info!("deleted {}", path.display());
            }
        } else if !keep.contains(&path) && fontconfig::remove_file(&path)? {
            removed += 1;
        }
    }
    Ok(removed)
}

fn print_dry_run(
    paths: &Paths,
    jobs: &[Job],
    unchanged: usize,
    skipped: &[(PathBuf, select::Skip)],
    stale: &[Entry],
) {
    println!("{:<10}{:>4}", "patch", jobs.len());
    for j in jobs {
        println!("  {}", paths.tilde(&j.original.display().to_string()));
    }
    println!("{:<10}{:>4}", "unchanged", unchanged);
    println!("{:<10}{:>4}{}", "skipped", skipped.len(), reasons(skipped));
    println!("{:<10}{:>4}", "remove", stale.len());
    for e in stale {
        println!("  {}", paths.tilde(&e.copy.display().to_string()));
    }
}
