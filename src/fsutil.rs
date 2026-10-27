//! File helpers: atomic writes, the lock file and SHA-256.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("{op} {path}")]
    Io {
        op: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("held by another nostar process: {0}")]
    LockHeld(PathBuf),
}

impl FsError {
    pub fn io(op: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Self {
        let path = path.to_path_buf();
        move |source| Self::Io { op, path, source }
    }
}

/// Writes `data` to `<path>.tmp` and renames it, so readers see the old or the new file, never a part.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<(), FsError> {
    create_parent(path)?;
    let tmp = tmp_path(path);
    let write = || -> io::Result<()> {
        let mut f = File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()
    };
    if let Err(e) = write() {
        let _ = fs::remove_file(&tmp);
        return Err(FsError::io("write", &tmp)(e));
    }
    fs::rename(&tmp, path).map_err(FsError::io("rename", &tmp))
}

/// `<path>.tmp` in the same directory, so that the rename stays on one file system.
pub fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

pub fn create_parent(path: &Path) -> Result<(), FsError> {
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => {
            fs::create_dir_all(dir).map_err(FsError::io("create", dir))
        }
        _ => Ok(()),
    }
}

/// Lowercase hex SHA-256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn sha256_file(path: &Path) -> Result<String, FsError> {
    let data = fs::read(path).map_err(FsError::io("read", path))?;
    Ok(sha256_hex(&data))
}

/// Exclusive lock for one nostar run; released when dropped.
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

impl Lock {
    pub fn acquire(path: &Path) -> Result<Self, FsError> {
        create_parent(path)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(FsError::io("open", path))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(FsError::LockHeld(path.to_path_buf())),
            Err(TryLockError::Error(e)) => Err(FsError::io("lock", path)(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nostar-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn write_atomic_creates_dirs_and_replaces() {
        let dir = scratch("atomic");
        let p = dir.join("a/b/rules.tsv");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert!(!tmp_path(&p).exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sha256_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn lock_is_exclusive_until_dropped() {
        let dir = scratch("lock");
        let p = dir.join("lock");
        let first = Lock::acquire(&p).unwrap();
        assert!(matches!(Lock::acquire(&p), Err(FsError::LockHeld(_))));
        drop(first);
        Lock::acquire(&p).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
