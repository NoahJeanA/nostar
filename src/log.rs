//! Logger (TECHNIK.md 11): every line to the log file, warnings and errors (more with -v) to stderr.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{LevelFilter, Log, Metadata, Record};

use crate::fsutil::{FsError, create_parent};

/// The log file is moved to the backup when it grows past this size.
const ROTATE_BYTES: u64 = 1024 * 1024;

struct Logger {
    stderr: LevelFilter,
    file_level: LevelFilter,
    file: Option<Mutex<File>>,
}

/// Installs the logger. Without a usable log file it still logs to stderr and returns the reason.
pub fn init(verbosity: u8, path: &Path, backup: &Path) -> Result<(), FsError> {
    let (stderr, file_level) = match verbosity {
        0 => (LevelFilter::Warn, LevelFilter::Info),
        1 => (LevelFilter::Info, LevelFilter::Info),
        _ => (LevelFilter::Debug, LevelFilter::Debug),
    };
    let opened = open(path, backup);
    let (file, result) = match opened {
        Ok(f) => (Some(Mutex::new(f)), Ok(())),
        Err(e) => (None, Err(e)),
    };
    let logger = Logger {
        stderr,
        file_level,
        file,
    };
    // Only main calls init, once; a second logger would be a bug and is ignored.
    if log::set_logger(Box::leak(Box::new(logger))).is_ok() {
        log::set_max_level(stderr.max(file_level));
    }
    result
}

fn open(path: &Path, backup: &Path) -> Result<File, FsError> {
    create_parent(path)?;
    if fs::metadata(path).is_ok_and(|m| m.len() > ROTATE_BYTES) {
        fs::rename(path, backup).map_err(FsError::io("rename", path))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(FsError::io("open", path))
}

impl Log for Logger {
    fn enabled(&self, m: &Metadata) -> bool {
        m.level() <= self.stderr || (self.file.is_some() && m.level() <= self.file_level)
    }

    fn log(&self, r: &Record) {
        let target = module(r.target());
        if r.level() <= self.stderr {
            eprintln!(
                "{}: {target}: {}",
                r.level().as_str().to_lowercase(),
                r.args()
            );
        }
        if let Some(file) = &self.file
            && r.level() <= self.file_level
        {
            let line = format!(
                "{} {:<5} {target}: {}\n",
                timestamp(SystemTime::now()),
                r.level().as_str(),
                r.args()
            );
            if let Ok(mut f) = file.lock() {
                // A failing log write must not abort the command.
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {
        if let Some(Ok(mut f)) = self.file.as_ref().map(|f| f.lock()) {
            let _ = f.flush();
        }
    }
}

/// `nostar::rules::build` -> `rules`; explicit targets such as `lock` stay as they are.
fn module(target: &str) -> &str {
    let rest = target.strip_prefix("nostar::").unwrap_or(target);
    rest.split("::").next().unwrap_or(rest)
}

/// UTC timestamp `2026-10-26T13:40:01Z` without a date crate.
fn timestamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to (year, month, day); Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn timestamps() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        // 2026-10-26T13:40:01Z
        assert_eq!(
            timestamp(UNIX_EPOCH + Duration::from_secs(1_793_022_001)),
            "2026-10-26T13:40:01Z"
        );
        // leap day
        assert_eq!(
            timestamp(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
    }

    #[test]
    fn module_names() {
        assert_eq!(module("nostar::rules::build"), "rules");
        assert_eq!(module("nostar::config"), "config");
        assert_eq!(module("lock"), "lock");
    }
}
