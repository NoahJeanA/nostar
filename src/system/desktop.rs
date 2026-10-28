//! Desktop fonts from GNOME's `gsettings` (fachl. Spezifikation 7.1, TECHNIK.md 10.3).

use std::process::Command;

const SCHEMA: &str = "org.gnome.desktop.interface";
const KEYS: [&str; 3] = ["font-name", "document-font-name", "monospace-font-name"];

/// Pango style and size words after the family (`Cantarell Bold Italic 11`).
const STYLE_WORDS: &[&str] = &[
    "normal",
    "roman",
    "oblique",
    "italic",
    "small-caps",
    "all-small-caps",
    "petite-caps",
    "all-petite-caps",
    "unicase",
    "title-caps",
    "thin",
    "ultra-light",
    "extra-light",
    "light",
    "semi-light",
    "demi-light",
    "book",
    "regular",
    "medium",
    "semi-bold",
    "demi-bold",
    "bold",
    "ultra-bold",
    "extra-bold",
    "heavy",
    "black",
    "ultra-heavy",
    "extra-heavy",
    "ultra-condensed",
    "extra-condensed",
    "condensed",
    "semi-condensed",
    "semi-expanded",
    "expanded",
    "extra-expanded",
    "ultra-expanded",
    "not-rotated",
    "south",
    "upside-down",
    "north",
    "rotated-left",
    "east",
    "rotated-right",
    "west",
];

/// Families of the GNOME interface, document and monospace font. Empty without
/// `gsettings` or the schema; that is not an error.
pub fn families() -> Vec<String> {
    let mut out = Vec::new();
    for key in KEYS {
        let value = match Command::new("gsettings")
            .args(["get", SCHEMA, key])
            .output()
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            Ok(o) => {
                log::debug!(
                    "gsettings get {SCHEMA} {key}: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                );
                continue;
            }
            Err(e) => {
                log::debug!("gsettings: {e}");
                return out;
            }
        };
        for family in parse(&value) {
            log::debug!("{key}: {family}");
            if !out.contains(&family) {
                out.push(family);
            }
        }
    }
    out
}

/// `'Adwaita Mono 11'` -> `["Adwaita Mono"]`; a Pango font description with quotes,
/// a family list, style words and a size.
fn parse(value: &str) -> Vec<String> {
    let s = value.trim().trim_matches('\'');
    let mut words: Vec<&str> = s.split_whitespace().collect();
    if words
        .last()
        .is_some_and(|w| w.trim_end_matches("px").parse::<f64>().is_ok())
    {
        words.pop();
    }
    while words.len() > 1
        && words
            .last()
            .is_some_and(|w| STYLE_WORDS.contains(&w.to_lowercase().as_str()))
    {
        words.pop();
    }
    words
        .join(" ")
        .split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_descriptions() {
        assert_eq!(parse("'Adwaita Mono 11'\n"), ["Adwaita Mono"]);
        assert_eq!(parse("'Cantarell Bold Italic 11'"), ["Cantarell"]);
        assert_eq!(parse("'Rubik 10.5'"), ["Rubik"]);
        assert_eq!(
            parse("'Noto Sans,Noto Color Emoji 11'"),
            ["Noto Sans", "Noto Color Emoji"]
        );
        assert_eq!(parse("'Monospace'"), ["Monospace"]);
        assert!(parse("''").is_empty());
    }
}
