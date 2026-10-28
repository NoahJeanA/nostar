//! `nostar remove [--keep-config]` (fachl. Spezifikation 10.3, TECHNIK.md 10.6).

use std::path::{Path, PathBuf};

use crate::Error;
use crate::fsutil::{FsError, Lock};
use crate::paths::Paths;
use crate::system::fontconfig;
use crate::system::manifest::Manifest;

pub fn remove(paths: &Paths, keep_config: bool) -> Result<(), Error> {
    let lock = Lock::acquire(&paths.lock())?;
    let manifest = Manifest::load(&paths.manifest())?;
    let mut removed = remove_tree(&paths.fonts_dir())?;
    let mut files = vec![paths.fontconfig_rule(), paths.rules(), paths.manifest()];
    if !keep_config {
        files.push(paths.config_file());
        removed += remove_tree(&paths.source_dir())?;
    }
    for f in &files {
        removed += usize::from(fontconfig::remove_file(f)?);
    }
    fontconfig::cache()?;
    // Last, so that the log keeps everything up to here.
    drop(lock);
    for f in [paths.log(), paths.log_backup(), paths.lock()] {
        removed += usize::from(fontconfig::remove_file(&f)?);
    }
    // Directories nostar created, deepest first; only when empty.
    let mut dirs: Vec<PathBuf> = manifest.created_dirs.clone();
    dirs.extend(paths.own_dirs());
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in dirs {
        if std::fs::remove_dir(&d).is_ok() {
            log::info!("deleted {}", d.display());
        }
    }
    println!("removed {removed} files");
    println!("{}", super::RESTART);
    Ok(())
}

/// Deletes `dir` with everything below it; returns the number of files.
fn remove_tree(dir: &Path) -> Result<usize, Error> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut n = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            n += remove_tree(&path)?;
        } else {
            n += usize::from(fontconfig::remove_file(&path)?);
        }
    }
    std::fs::remove_dir(dir).map_err(FsError::io("delete", dir))?;
    log::info!("deleted {}", dir.display());
    Ok(n)
}
