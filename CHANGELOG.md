# Changelog

## 0.1.0 – 2026-10-28

First version.

- `nostar data update`: Entgendert dictionary in a fixed version with SHA-256 check
- `nostar rules build | test`: rule list with fixed forms, dictionary, ending rules and fallback, checked against 149 test cases
- `nostar show`: how a text would be displayed, with stage and source of each rule
- `nostar apply [--dry-run]`: patches font copies (GSUB `calt`), verifies each copy with HarfBuzz, hides the originals via fontconfig; mode `standard` (generic families, GNOME fonts, `families`) or `all`
- `nostar status`, `nostar remove [--keep-config]`
- Man pages and shell completions for packages (`--generate-man`, `--generate`)
