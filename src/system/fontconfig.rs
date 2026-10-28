//! fontconfig: `fc-match`, `fc-list`, `99-nostar.conf` and `fc-cache` (TECHNIK.md 10.3, 10.4).

use std::path::{Path, PathBuf};
use std::process::Command;

use super::SystemError;
use crate::fsutil::{FsError, write_atomic};

/// One font file as fontconfig sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FcFont {
    pub file: PathBuf,
    pub family: String,
    /// `spacing=100`
    pub mono: bool,
}

const FORMAT: &str = "%{file}\t%{family[0]}\t%{spacing}\n";

fn run(program: &'static str, args: &[&str]) -> Result<String, SystemError> {
    log::debug!("{program} {}", args.join(" "));
    let out = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| SystemError::Spawn(program, e))?;
    if !out.status.success() {
        return Err(SystemError::Command {
            program,
            status: out
                .status
                .code()
                .map_or_else(|| "none (signal)".to_string(), |c| c.to_string()),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn parse(text: &str) -> Vec<FcFont> {
    text.lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let file = cols.next().filter(|f| !f.is_empty())?;
            Some(FcFont {
                file: PathBuf::from(file),
                family: cols.next().unwrap_or("").to_string(),
                mono: cols.next() == Some("100"),
            })
        })
        .collect()
}

/// All font files matching `pattern` (`:` for all).
pub fn list(pattern: &str) -> Result<Vec<FcFont>, SystemError> {
    Ok(parse(&run("fc-list", &["--format", FORMAT, pattern])?))
}

/// The font fontconfig chooses for `pattern`, e.g. `sans-serif`.
pub fn best(pattern: &str) -> Result<Option<FcFont>, SystemError> {
    Ok(parse(&run("fc-match", &["--format", FORMAT, pattern])?)
        .into_iter()
        .next())
}

/// Family name as a fontconfig pattern value: `\`, `-`, `:` and `,` escaped.
pub fn family_pattern(family: &str) -> String {
    let mut s = String::from(":family=");
    for c in family.chars() {
        if matches!(c, '\\' | '-' | ':' | ',') {
            s.push('\\');
        }
        s.push(c);
    }
    s
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `99-nostar.conf`: every patched original hidden by its exact path.
pub fn rule(originals: &[&Path]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\"?>\n\
         <!DOCTYPE fontconfig SYSTEM \"urn:fontconfig:fonts.dtd\">\n\
         <!-- written by nostar, do not edit; nostar remove deletes it -->\n\
         <fontconfig>\n  <selectfont>\n    <rejectfont>\n",
    );
    for o in originals {
        s.push_str(&format!(
            "      <pattern><patelt name=\"file\"><string>{}</string></patelt></pattern>\n",
            xml_escape(&o.to_string_lossy())
        ));
    }
    s.push_str("    </rejectfont>\n  </selectfont>\n</fontconfig>\n");
    s
}

/// Writes the rule if its content changes; `true` if written.
pub fn write_rule(path: &Path, originals: &[&Path]) -> Result<bool, SystemError> {
    let text = rule(originals);
    if std::fs::read_to_string(path).is_ok_and(|old| old == text) {
        return Ok(false);
    }
    write_atomic(path, text.as_bytes())?;
    log::info!("wrote {}", path.display());
    log::debug!("{text}");
    Ok(true)
}

pub fn remove_file(path: &Path) -> Result<bool, SystemError> {
    match std::fs::remove_file(path) {
        Ok(()) => {
            log::info!("deleted {}", path.display());
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(FsError::io("delete", path)(e).into()),
    }
}

pub fn cache() -> Result<(), SystemError> {
    run("fc-cache", &[])?;
    log::info!("fc-cache done");
    Ok(())
}

/// Rebuilds the cache of `dir` even if fontconfig thinks it is current: a program that
/// scans while `apply` writes leaves a cache with the same mtime second (TECHNIK.md 10.4).
pub fn cache_force(dir: &Path) -> Result<(), SystemError> {
    let dir = dir.to_string_lossy();
    run("fc-cache", &["-f", &dir])?;
    log::info!("fc-cache -f {dir} done");
    Ok(())
}

/// Copies fontconfig does not list.
pub fn unlisted<'a>(copies: &[&'a Path]) -> Result<Vec<&'a Path>, SystemError> {
    let listed: std::collections::HashSet<PathBuf> =
        list(":")?.into_iter().map(|f| f.file).collect();
    Ok(copies
        .iter()
        .copied()
        .filter(|c| !listed.contains(*c))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lines() {
        let fonts = parse("/a/B.ttf\tNoto Sans\t\n/a/M.ttf\tMono\t100\n\n");
        assert_eq!(fonts.len(), 2);
        assert!(!fonts[0].mono && fonts[1].mono);
        assert_eq!(fonts[0].family, "Noto Sans");
    }

    #[test]
    fn escapes() {
        assert_eq!(family_pattern("Foo-Bar: X"), ":family=Foo\\-Bar\\: X");
        let r = rule(&[Path::new("/x/A&B.ttf")]);
        assert!(r.contains("<string>/x/A&amp;B.ttf</string>"), "{r}");
    }
}
