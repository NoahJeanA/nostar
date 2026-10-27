//! `nostar show <text>`

use crate::Error;
use crate::cmd::rules::load;
use crate::config::{Config, is_letter};
use crate::paths::Paths;
use crate::rules::MARKER_CHARS;

pub fn show(paths: &Paths, config: &Config, text: &str) -> Result<(), Error> {
    let set = load(paths, config)?;
    let out = set.apply(text);
    println!("{}", out.text);
    let chars: Vec<char> = text.chars().collect();
    // One line per changed word, with the rule of its first change.
    let mut lines: Vec<(String, String, String)> = Vec::new();
    let mut last_end = 0;
    for c in &out.changes {
        let (start, end) = word(&chars, c.range.start, c.range.end);
        if start < last_end {
            continue;
        }
        last_end = end;
        let source: String = chars[start..end].iter().collect();
        let rule = &set.rules()[c.rule];
        let info = format!("stage {}  {}", rule.stage as u8, rule.origin);
        lines.push((source.clone(), set.render(&source), info));
    }
    let ws = lines.iter().map(|l| l.0.chars().count()).max().unwrap_or(0);
    let wt = lines.iter().map(|l| l.1.chars().count()).max().unwrap_or(0);
    for (source, target, info) in &lines {
        println!("  {source:<ws$}  -> {target:<wt$}  {info}");
    }
    Ok(())
}

/// Widens a change to the whole word: letters, markers, soft hyphens and `-` after a marker.
fn word(chars: &[char], mut start: usize, mut end: usize) -> (usize, usize) {
    let part = |i: usize| {
        let c = chars[i];
        is_letter(c)
            || MARKER_CHARS.contains(&c)
            || c == '\u{AD}'
            || (c == '-' && i > 0 && MARKER_CHARS.contains(&chars[i - 1]))
    };
    while start > 0 && part(start - 1) {
        start -= 1;
    }
    while end < chars.len() && part(end) {
        end += 1;
    }
    (start, end)
}
