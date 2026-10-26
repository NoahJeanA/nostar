# Technische Spezifikation: `nostar`

Status: Entwurf 1 · Stand: 2026-10-26 · Grundlage: [SPEZIFIKATION.md](SPEZIFIKATION.md) (Entwurf 3)

Dieses Dokument legt fest, **wie** `nostar` gebaut wird. **Was** das Programm tut, steht in der fachlichen Spezifikation. Abweichungen davon sind in Abschnitt 15 gesammelt und brauchen eine Freigabe.

---

## 1. Leitlinien

| Leitlinie | Bedeutung für den Code |
|---|---|
| **Nur Spezifiziertes** | Keine Funktion, Option oder Abhängigkeit ohne Grundlage in einer der beiden Spezifikationen |
| **Klare Zuständigkeit** | Jedes Modul hat genau eine Aufgabe (Abschnitt 3). Module reden über Datentypen miteinander, nicht über Dateien. |
| **Klein** | Wenige Abhängigkeiten (Abschnitt 4), keine asynchrone Laufzeit, keine Plugins, keine Hintergrunddienste |
| **Nachvollziehbar** | Jede Änderung am System wird protokolliert (Abschnitt 11) |
| **Englisch im Code** | Bezeichner, Kommentare, Log- und Fehlermeldungen auf Englisch; Meldungen kurz und konkret (Abschnitt 12) |

## 2. Gesamtablauf

![Ablauf](img/pipeline.svg)

```
config.toml ─┐
             ▼
 source ──► lexicon ──► rules ──► plan ──► font ──► system
 (Download,  (Wörter-    (Regel-   (Edit-    (GSUB pro   (fontconfig,
  Prüfsumme)  buch)       liste)    skripte,  Font, prüfen) Manifest)
                                    Pässe)
```

1. **source:** lädt das Entgendert-Wörterbuch in fester Version und prüft die Prüfsumme.
2. **lexicon:** liest das Wörterbuch ein und ergänzt es um die eigenen Wörter aus `config.toml`.
3. **rules:** erzeugt die Regelliste, minimiert und ordnet sie und prüft sie gegen die Testsammlung.
4. **plan:** übersetzt die Regeln in zeichenbasierte Edit-Skripte und teilt sie auf Pässe auf. Das ist noch unabhängig von einem Font.
5. **font:** baut den Plan pro Font in die GSUB-Tabelle ein, schreibt die Kopie und prüft sie.
6. **system:** schreibt die fontconfig-Regel und das Manifest und ruft `fc-cache` auf.

Schritte 1–3 laufen einmal je Daten- oder Regeländerung, Schritte 4–6 bei `apply`.

## 3. Module und Zuständigkeiten

Ein einziges Crate (`nostar`, Binary). Keine Workspace-Aufteilung.

```
src/
├── main.rs            CLI-Definition, Befehlsaufruf, Exit-Codes
├── config.rs          config.toml lesen und prüfen
├── paths.rs           alle Pfade (XDG)
├── log.rs             Logger (stderr + Datei)
├── fsutil.rs          atomares Schreiben, Sperrdatei, SHA-256
├── source.rs          Wörterbuch laden, prüfen, parsen
├── lexicon.rs         Wörterbuch-Modell + eigene Wörter
├── rules/
│   ├── mod.rs         Typen: Rule, Pattern, Marker, Stage, Boundary
│   ├── fixed.rs       Stufe 1: feste Formen, Partizipien
│   ├── build.rs       lexicon → Regelliste (Varianten, Minimierung, Ordnung)
│   ├── engine.rs      Referenz-Umsetzung auf Text
│   └── store.rs       rules.tsv lesen/schreiben
├── plan.rs            Regeln → Edit-Skripte → Pässe (zeichenbasiert)
├── font/
│   ├── mod.rs         Patch eines Fonts: lesen → kompilieren → schreiben → prüfen
│   ├── select.rs      Kandidaten über fontconfig
│   ├── glyphs.rs      Zeichen → Glyphen, Buchstabenmenge, Ausschlussprüfung
│   ├── compile.rs     Plan → GSUB-Lookups (glyphenbasiert)
│   ├── gsub.rs        Lookups in bestehende GSUB einfügen, Indizes verschieben
│   └── verify.rs      Formung mit harfrust, Vergleich Original/Kopie
├── system/
│   ├── fontconfig.rs  99-nostar.conf schreiben, fc-cache
│   └── manifest.rs    manifest.toml lesen/schreiben
└── cmd/
    ├── data.rs        nostar data update
    ├── rules.rs       nostar rules build | test
    ├── show.rs        nostar show
    ├── apply.rs       nostar apply
    ├── status.rs      nostar status
    └── remove.rs      nostar remove
tests/
├── cases.tsv          Testsammlung (SPEZIFIKATION 13.1)
├── fixtures/          kleiner Latin-Font (OFL) für Font-Tests
├── engine.rs          Regelliste gegen cases.tsv
└── font.rs            Patch + Prüfung am Fixture-Font
```

### 3.1 Abhängigkeiten zwischen Modulen

Abhängigkeiten zeigen nur nach unten. `cmd/*` ist die einzige Schicht, die mehrere Fachmodule zusammenführt.

```
cmd ──► source, lexicon, rules, plan, font, system
font ──► plan, rules (Typen)
plan ──► rules (Typen)
rules ──► lexicon
alle ──► config, paths, log, fsutil
```

Verboten: `rules` kennt keine Fonts, `font` kennt kein Dateisystem außer Ein- und Ausgabepfad, `system` kennt keine Regeln.

## 4. Abhängigkeiten (Crates)

| Crate | Zweck | Begründung |
|---|---|---|
| `clap` (Feature `derive`) | Kommandozeile | Hilfe-Texte und Fehlermeldungen ohne eigenen Code |
| `serde` (Feature `derive`), `toml` | `config.toml`, `manifest.toml` | Vorgabe TOML |
| `read-fonts`, `write-fonts` | Fonts lesen, GSUB bauen, Font schreiben | Einzige gepflegte Rust-Bibliothek, die bestehende Tabellen verändern und neu schreiben kann |
| `harfrust` | Formung bei der Prüfung (9.6) | Rust-Portierung von HarfBuzz, keine C-Abhängigkeit |
| `sha2` | Prüfsummen | |
| `thiserror` | Fehlertypen | |
| `log` | Log-Schnittstelle | Eigener Logger, siehe Abschnitt 11 |
| `unicode-properties` | Unicode-Kategorie „Letter“ | `char::is_alphabetic` umfasst mehr als Kategorie L |

**Nicht** verwendet: `anyhow`, `tokio`, HTTP-Clients, `regex`, `chrono`, `rayon`, Logging-Frameworks.

Externe Programme (per `std::process::Command`): `fc-list`, `fc-match`, `fc-cache`, `curl` (nur `data update`).

Versionen werden über `Cargo.lock` fixiert. Mindestversion Rust: 1.89 (für `File::try_lock`).

## 5. Konfiguration

Datei: `$XDG_CONFIG_HOME/nostar/config.toml`. Fehlt sie, gelten die Voreinstellungen. Unbekannte Schlüssel sind ein Fehler (`#[serde(deny_unknown_fields)]`).

```toml
# Fonts: "standard" or "all"
mode = "standard"
families = ["Adwaita Sans", "JetBrains Mono"]
exclude = []                       # family names or absolute paths

# Recognition
markers = ["*", ":", "_", "·", "/", "/-"]
binnen_i = true
fallback = "cut"                   # "cut" | "marker" | "off"
participles = false
skip_monospace = false

[source]
commit = "febca802f868c4ce968bbae70c099cdbc0454e18"
sha256 = "6330c126964385e84fb5f01c47893a2a75a65b163f6be4bd19bc2448517f73b3"

# Own words, take precedence over the dictionary.
# Key: feminine stem as written before the marker.
[words.Freund]
singular = "Freund"
plural = "Freunde"
compound = "Freundes"              # optional, default: plural
```

| Schlüssel | Typ | Prüfung |
|---|---|---|
| `mode` | Enum `standard` \| `all` | |
| `families`, `exclude` | Liste von Strings | nicht leer; Pfade müssen absolut sein |
| `markers` | Liste aus `* : _ · / /-` | keine Duplikate, keine anderen Werte |
| `binnen_i`, `participles`, `skip_monospace` | bool | |
| `fallback` | Enum `cut` \| `marker` \| `off` | |
| `source.commit` | 40 Hex-Zeichen | |
| `source.sha256` | 64 Hex-Zeichen | |
| `words.<stamm>` | `singular`, `plural` Pflicht, `compound` optional | nur Buchstaben; Stamm beginnt mit Großbuchstaben |

## 6. Pfade

Alle Pfade entstehen ausschließlich in `paths.rs` und respektieren `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`.

| Zweck | Pfad |
|---|---|
| Konfiguration | `~/.config/nostar/config.toml` |
| Wörterbuch | `~/.local/share/nostar/source/entgendert-<commit12>.js` |
| Regelliste | `~/.local/share/nostar/rules.tsv` |
| Gepatchte Fonts | `~/.local/share/fonts/nostar/<Originalpfad ohne führenden />` |
| fontconfig-Regel | `~/.config/fontconfig/conf.d/99-nostar.conf` |
| Manifest | `~/.local/state/nostar/manifest.toml` |
| Log | `~/.local/state/nostar/nostar.log` |
| Sperrdatei | `~/.local/state/nostar/lock` |

## 7. Datenquelle (`source.rs`, `lexicon.rs`)

### 7.1 Laden

`nostar data update` ruft auf:

```
curl --fail --silent --show-error --location --max-time 60 --output <tmp> \
  https://raw.githubusercontent.com/janduettmann/Entgendert/<commit>/extension/dictionary.js
```

Danach SHA-256 prüfen. Stimmt sie, wird die Datei atomar an ihren Platz verschoben. Sonst wird sie gelöscht und ein Fehler gemeldet.

### 7.2 Format

`dictionary.js` enthält vier String-Felder: `irregular`, `regular`, `adjectival`, `rules`. `source.rs` liest genau diese vier Felder aus (Suche nach `<feld>: "` bis zum nächsten `"`). Ein weitergehender JavaScript-Parser ist nicht nötig, weil die Datei über die Prüfsumme fixiert ist.

| Feld | Aufbau | Bedeutung |
|---|---|---|
| `irregular` | `stamm,a,b,c[,a]` getrennt durch `;` | Singular = decode(stamm, a), Plural = decode(Singular, b), Dativ = decode(Plural, c); Flag `a` = adjektivisch |
| `regular` | `stamm` getrennt durch `;` | Singular = Plural = stamm |
| `adjectival` | Wörter getrennt durch `;` | substantivierte Adjektive („Vorsitzende“) |
| `rules` | `endung=s,p,d` oder `endung=!` | Endungsregel; `!` = keine Regel |

decode(basis, code): leer → basis; beginnt mit `+` → basis + Rest; sonst → code.

### 7.3 Modell

```rust
pub struct Person {
    pub stem: String,          // "Ärzt"
    pub singular: String,      // "Arzt"
    pub plural: String,        // "Ärzte"
    pub compound: String,      // "Ärzte" (default: plural)
    pub origin: Origin,
}
pub struct EndingRule { pub ending: String, pub singular: String, pub plural: String }
pub enum Origin { Entgendert { commit: String }, Config, Builtin }
pub struct Lexicon { pub persons: Vec<Person>, pub endings: Vec<EndingRule>, pub adjectival: Vec<String> }
```

Einträge aus `[words]` ersetzen gleichnamige Stämme aus dem Wörterbuch. Der Dativ wird gelesen, aber in Version 1 nicht verwendet (fachl. Spezifikation 4.6).

## 8. Regeln (`rules/`)

### 8.1 Typen

```rust
pub enum Stage { Fixed = 1, Lexicon = 2, Ending = 4, Fallback = 5 }
pub enum Marker { Star, Colon, Underscore, MiddleDot, Slash, SlashHyphen, BinnenI }
pub enum Right { WordEnd, Any }       // Any: compound part may follow
pub struct Rule {
    pub stage: Stage,
    pub left_boundary: bool,         // only stage 1: no letter before the stem
    pub stem: String,                // exact characters before the marker
    pub marker: Marker,
    pub suffix: String,              // "innen", "in", "r", ...
    pub right: Right,
    pub target: String,              // replacement for stem + marker + suffix
    pub origin: Origin,
}
```

Stufe 2 und 3 der fachlichen Spezifikation fallen zusammen. Eine Stamm-Regel prüft nur das **Ende** des Wortes. „Stammkund\*innen“ trifft dadurch die Regel für „kund“ (Variante mit kleinem Anfangsbuchstaben), Abschnitt 15, Punkt 4.

### 8.2 Erzeugung (`build.rs`)

Für jede `Person`:

| Variante | Stamm | Endung | Ziel | `right` |
|---|---|---|---|---|
| Plural | `Ärzt` | `innen` | `Ärzte` | `Any` |
| Singular | `Ärzt` | `in` | `Arzt` | `WordEnd` |

Jede Variante wird vervielfacht über:
- **Schreibweise des Stamms:** Original (`Ärzt`), kleiner Anfang (`ärzt`, für Zusammensetzungen), Großbuchstaben (`ÄRZT`). Bei Großbuchstaben werden Endung und Ziel ebenfalls großgeschrieben.
- **Endung mit großem I:** `innen` und `Innen`, `in` und `In`
- **Genderzeichen:** jedes aktive aus `markers`
- **Binnen-I** (wenn `binnen_i`): nur Originalschreibweise, Marker `BinnenI`, Endung `Innen`/`In`

Bei `compound ≠ plural` entstehen zwei Plural-Regeln: `WordEnd` → Plural und `Any` mit Buchstaben danach → `compound`.

Für Endungsregeln entstehen dieselben Varianten mit dem Endungsmuster als Stamm. Endungen mit `!` erzeugen keine Regel.

**Stufe 5 (`fallback`)** erzeugt Regeln nur für `* : _ ·`. `/` und `/-` sind ausgenommen, damit Pfade und URLs (`docs/index`, `x/in`) unberührt bleiben (Abschnitt 15, Punkt 5).
- `cut`: Genderzeichen + `in`/`innen` (alle Schreibweisen) ausblenden, Buchstabe davor erforderlich
- `marker`: nur das Genderzeichen ausblenden, Buchstabe davor und danach erforderlich
- `off`: keine Regel

### 8.3 Minimierung

Eine Stamm-Regel entfällt, wenn die Referenz-Umsetzung **ohne** sie für alle ihre Varianten dasselbe Ergebnis liefert, z.B. weil die Endungsregel `er` „Lehrer\*innen“ ohnehin zu „Lehrer“ macht. Das verkleinert die Regelliste erheblich (der Großteil von `regular` sind Stämme auf `-er`). Die Minimierung läuft einmal in `rules build`. Ihr Ergebnis wird protokolliert (`rules: minimized 18412 -> 5230 rules`).

### 8.4 Reihenfolge

Die Regelliste ist total geordnet:
1. nach Stufe aufsteigend
2. innerhalb der Stufe nach Länge von `stem + marker + suffix` absteigend
3. bei Gleichstand `right = WordEnd` vor `Any`
4. dann lexikografisch (für eine stabile, vergleichbare Datei)

Die Referenz-Umsetzung und der GSUB-Compiler werten dieselbe Reihenfolge aus.

### 8.5 Referenz-Umsetzung (`engine.rs`)

`fn apply(rules: &RuleSet, text: &str) -> Vec<Change>` mit `Change { range, target, rule }`.

Text wird von links nach rechts gelesen. An jeder Position wird die erste passende Regel in Listenreihenfolge angewendet. Danach geht es hinter dem ersetzten Bereich weiter. Das bildet die Arbeitsweise von HarfBuzz nach (Abschnitt 9.4). Kombinierende Zeichen (Kategorie Mn) werden beim Vergleich übersprungen.

### 8.6 Speicherformat `rules.tsv`

UTF-8, eine Regel pro Zeile, Tab-getrennt, erste Zeile `# nostar rules v1 lexicon=<sha256> config=<sha256>`.

```
stage  left  stem    marker  suffix  right    target  origin
2      0     Ärzt    *       innen   any      Ärzte   entgendert@febca80
2      0     Ärzt    *       in      wordend  Arzt    entgendert@febca80
```

### 8.7 Testsammlung `tests/cases.tsv`

Zwei Spalten: Eingabe und erwartete Anzeige. Die Datei wird mit `include_str!` ins Programm übernommen und von `rules build`, `rules test` und der Font-Prüfung benutzt.

## 9. GSUB-Kompilierung (`plan.rs`, `font/compile.rs`, `font/gsub.rs`)

Das ist der technisch anspruchsvollste Teil. Ziel: Die geordnete Regelliste wird so in GSUB übersetzt, dass HarfBuzz genau dasselbe Ergebnis liefert wie die Referenz-Umsetzung, **ohne** normalen Text spürbar zu verlangsamen.

### 9.1 Edit-Skript je Regel (`plan.rs`)

Quelle Q = `stem + marker + suffix`, Ziel Z = `target`. Für jede Quellposition wird genau eine Operation bestimmt:

| Operation | Bedeutung | Beispiel |
|---|---|---|
| `Keep` | Glyphe bleibt | `r`, `z`, `t` in „Ärzt“ |
| `Deumlaut` | ä→a, ö→o, ü→u, Ä→A, Ö→O, Ü→U | `Ä` in „Ärzt\*in“ → „Arzt“ |
| `Replace(s)` | Glyphe wird durch Zeichenfolge s ersetzt (s darf leer sein) | `*` → `en` in „Kolleg\*innen“ |
| `Delete` | Glyphe entfällt | `i`, `n`, `n`, `e`, `n` |

Algorithmus:
1. Gemeinsamer Anfang P von Q und Z, wobei ein Umlaut in Q und sein Grundbuchstabe in Z als gleich gelten (Operation `Deumlaut`).
2. Erste Position nach P: `Replace(Rest von Z)`. Alle folgenden Positionen: `Delete`.
3. **Anker** = erste Position mit einer Operation ≠ `Keep`.

Beispiele:

| Regel | Anker | Operationen ab Anker |
|---|---|---|
| Lehrer\*innen → Lehrer | `*` | Replace(""), Delete×5 |
| Kolleg\*innen → Kollegen | `*` | Replace("en"), Delete×5 |
| Ärzt\*innen → Ärzte | `*` | Replace("e"), Delete×5 |
| Ärzt\*in → Arzt | `Ä` | Deumlaut, Keep×3, Replace(""), Delete×2 |
| KollegInnen → Kollegen | `I` | Replace("en"), Delete×4 |

### 9.2 Pässe

Eine GSUB-Regel kann nur Glyphen **ab ihrem Anker** verändern. Davor liegende Glyphen sind nur Kontext. Daraus folgen zwei Pässe:

| Pass | Anker | Regeln | Wird geprüft an |
|---|---|---|---|
| **Stamm-Pass** | im Stamm (Umlaut, Kürzung) | wenige hundert | jedem Glyph, das Anker sein kann (v.a. Umlaute) |
| **Marker-Pass** | Genderzeichen bzw. `I` beim Binnen-I | alle übrigen | jedem Genderzeichen bzw. `I` |

Der Stamm-Pass läuft vor dem Marker-Pass. Eine Regel liegt in genau einem Pass. Da der Stamm-Pass die Genderform vollständig umwandelt, findet der Marker-Pass dort nichts mehr.

### 9.3 Aufbau der Lookups

![GSUB-Aufbau](img/gsub.svg)

Marker-Pass:

```
Gate-Lookup  (Typ 6, Format 2, in Feature calt)
  ├── Regel: Rückblick = letzter Stammbuchstabe X, Eingabe = Anker, Vorschau = LETTER
  │          → wende Dispatch-Lookup D_X an Position 0 an
  ├── ... eine Regel je X
  └── Regel: Rückblick = LETTER, Eingabe = Anker → D_fallback
Dispatch-Lookup D_X  (Typ 6, Format 2, als Extension)
  └── alle Regeln mit letztem Stammbuchstaben X in Listenreihenfolge,
      danach die Rückfall-Regeln (Stufe 5)
      → je Position: Deumlaut-, Replace_k- oder Delete-Lookup
Dispatch-Lookup D_fallback
  └── nur die Rückfall-Regeln
Hilfs-Lookups (nur über Dispatch erreichbar)
  ├── Delete     (Typ 2): Glyphe → leere Folge
  ├── Deumlaut   (Typ 1): ä→a, ö→o, ü→u, Ä→A, Ö→O, Ü→U
  └── Replace_k  (Typ 2): Glyphe → Folge; so gepackt, dass jede Glyphe je Lookup höchstens eine Folge hat
```

Die Rückfall-Regeln stehen am Ende jedes D_X, weil HarfBuzz nach der ersten passenden Gate-Regel nicht weitersucht, auch wenn D_X dann nichts tut.

Der Stamm-Pass ist gleich aufgebaut, verteilt aber nach der Anker-Glyphe (z.B. `Ä`) statt nach dem letzten Stammbuchstaben und hat keine Rückfall-Regeln.

Begründung für Gate und Dispatch: HarfBuzz prüft an einem Anker alle Regeln, die mit diesem Anker beginnen, nacheinander. Ohne Gate wären das an jedem `*`, `:` oder `/` mehrere zehntausend Regeln. Das Gate verteilt nach letztem Stammbuchstaben und verwirft Anker ohne Buchstaben davor und danach (z.B. `5 * 3`, `https://`) mit wenigen Vergleichen.

### 9.4 Regel-Details

- **Klassen (Format 2):**
  - Rückblick: jeder Buchstabe eine eigene Klasse (groß/klein getrennt)
  - Eingabe: die Genderzeichen `* : _ ·` bilden **eine** Klasse; `/`, `-` und jeder Buchstabe eigene Klassen
  - Vorschau: alle Buchstaben eine Klasse `LETTER`
- **Wortende (`WordEnd`):** GSUB kann „Ende des Textes“ nicht abfragen. Deshalb steht vor jeder `WordEnd`-Regel eine Ausnahme-Regel mit gleicher Eingabe und Vorschau `LETTER`, aber ohne Aktion. HarfBuzz beendet die Suche bei der ersten passenden Regel, auch wenn sie nichts tut.
- **Linke Grenze (Stufe 1):** ebenso über eine vorangestellte Ausnahme mit Rückblick `LETTER`
- **Lookup-Flag:** `IgnoreMarks` für Gate und Dispatch
- **Größe:** Dispatch-Lookups werden als Extension (Typ 7) gespeichert. Ein Subtable wird geteilt, bevor er 64 KiB erreicht. Die Teile behalten die Reihenfolge. HarfBuzz probiert Subtables der Reihe nach, die Semantik bleibt also gleich.

### 9.5 Einfügen in den Font (`gsub.rs`)

1. Bestehende GSUB lesen (oder leer anlegen)
2. Neue Lookups **vor** alle bestehenden setzen, Reihenfolge: Gate Stamm-Pass, Gate Marker-Pass, Dispatch-, Hilfs-Lookups
3. Alle bestehenden Lookup-Verweise um die Anzahl neuer Lookups verschieben: Feature-Liste, `SubstLookupRecord` in Typ 5/6 (auch innerhalb von Typ 7), Feature-Variationen
4. Feature `calt` mit den zwei Gate-Lookups anlegen und in allen Script-/Sprach-Einträgen aktivieren (`DFLT` und `latn` anlegen, falls nötig). Die Feature-Liste bleibt nach Tag sortiert. Alle Feature-Indizes werden neu zugeordnet: in Script-/Sprach-Einträgen, beim Pflicht-Feature (`ReqFeatureIndex`) und in Feature-Variationen.
5. Alle anderen Tabellen unverändert übernehmen, Font mit `write-fonts` schreiben

Bekannte Einschränkung: Das Feature `rvrn` (Varianten-Ersetzung variabler Fonts) läuft in HarfBuzz vor `calt`. Ersetzt es Buchstaben, sieht unsere Regel die ersetzte Glyphe. Gemeldet wird das von der Prüfung (9.6).

### 9.6 Prüfung (`verify.rs`)

Setzt fachl. Spezifikation 8.4 um. Für jeden gepatchten Font mit `harfrust`:

| Prüfung | Bestanden, wenn |
|---|---|
| **Umwandlung** | Für jeden Fall aus `cases.tsv`: Formung der Eingabe mit der **Kopie** = Formung der erwarteten Anzeige mit dem **Original** (gleiche Glyphen-IDs und Vorschübe) |
| **Unverändert** | Ein fester Korpus ohne Genderformen (Latein, Griechisch, Kyrillisch, Code, URLs, Uhrzeiten) ergibt in Original und Kopie identische Glyphen und Vorschübe |

Schlägt eine Prüfung fehl, wird die Kopie gelöscht und der Font als `failed` mit dem ersten abweichenden Fall gemeldet.

## 10. Befehle

| Befehl | Modul | Ablauf |
|---|---|---|
| `nostar data update` | `cmd/data.rs` | 7.1 |
| `nostar rules build` | `cmd/rules.rs` | lexicon → Regeln → Minimierung → Ordnung → `cases.tsv` prüfen → `rules.tsv` atomar schreiben |
| `nostar rules test` | `cmd/rules.rs` | `rules.tsv` gegen `cases.tsv` |
| `nostar show <text>` | `cmd/show.rs` | Referenz-Umsetzung, Ausgabe wie unten |
| `nostar apply [--dry-run]` | `cmd/apply.rs` | wie fachl. Spezifikation 10.1 |
| `nostar status` | `cmd/status.rs` | wie fachl. Spezifikation 10.2 |
| `nostar remove [--keep-config]` | `cmd/remove.rs` | wie fachl. Spezifikation 10.3 |

Globale Option: `-v` (zusätzlich `info` auf stderr), `-vv` (`debug`).

Beispielausgabe `show`:

```
$ nostar show "Die Ärzt*innen und jede*r Kolleg*in"
Die Ärzte und jeder Kollege
  Ärzt*innen  -> Ärzte    stage 2  entgendert@febca80
  jede*r      -> jeder    stage 1  builtin
  Kolleg*in   -> Kollege  stage 2  entgendert@febca80
```

Beispielausgabe `apply`:

```
$ nostar apply
patched    142
unchanged    0
skipped    611  (no latin letters: 598, collection: 9, excluded: 4)
failed       1  /usr/share/fonts/TTF/Foo.ttf: verify: "Ärzt*in" shaped as "Ärzt*in", expected "Arzt"
restart running applications to see the change
```

### 10.1 Parallelität

`apply` patcht Fonts parallel mit `std::thread::scope`. Die Anzahl der Threads entspricht `std::thread::available_parallelism`. Der Plan (9.1–9.2) wird einmal erstellt und von allen Threads gelesen.

### 10.2 Ablauf `apply` mit Sicherungen

1. Sperre holen (`File::try_lock` auf `lock`)
2. `rules.tsv` laden, Kopfzeile gegen aktuelles Wörterbuch und aktuelle Konfiguration prüfen
3. Kandidaten bestimmen, Manifest laden, Arbeitsliste bilden
4. Je Font: in `<ziel>.tmp` schreiben → prüfen → umbenennen
5. Veraltete Kopien löschen
6. Manifest atomar schreiben
7. `99-nostar.conf` atomar schreiben
8. `fc-cache` aufrufen

Ein Abbruch zwischen 4 und 7 lässt das System funktionsfähig: Neue Kopien ohne Regel in `99-nostar.conf` sind zusätzliche Fonts gleichen Namens. Der nächste `apply` räumt auf.

## 11. Logging (`log.rs`)

- Schnittstelle: Crate `log`; eigener Logger in rund 60 Zeilen
- **Datei** `nostar.log`: alle Stufen ab `info`, mit `-vv` ab `debug`. Bei mehr als 1 MiB wird sie beim Start zu `nostar.log.1` umbenannt (eine Sicherung).
- **stderr:** `warn` und `error` immer; `info` mit `-v`; `debug` mit `-vv`
- **stdout:** nur die Ergebnisausgabe des Befehls, kein Log

Format einer Zeile:

```
2026-10-26T13:40:01Z INFO  font: patched /usr/share/fonts/noto/NotoSans-Regular.ttf rules=5230 size=+812KiB time=41ms
```

Zeitstempel in UTC aus `SystemTime` (eigene Umrechnung, keine Abhängigkeit). Das `target` ist der Modulname.

Was protokolliert wird:

| Modul | `info` | `debug` |
|---|---|---|
| `source` | Download, Prüfsumme ok | URL, Größe |
| `rules` | Anzahl Regeln je Stufe, Minimierung, Testergebnis | Konflikte im Detail |
| `font` | patched/skipped/failed je Font mit Grund | Lookup-Anzahl, Subtable-Größen |
| `system` | jede geschriebene oder gelöschte Datei, `fc-cache` | Inhalt von `99-nostar.conf` |
| `cmd` | Start und Ende eines Befehls mit Dauer | |

## 12. Fehler

- Ein Fehlertyp je Modul mit `thiserror`. `main.rs` hat einen Typ `Error`, der diese über `#[from]` zusammenfasst und auf Exit-Codes abbildet.
- Meldungen: Englisch, kleingeschrieben, ohne Punkt am Ende, mit dem konkreten Objekt (Pfad, Wert, Fall). Keine Füllwörter wie „failed to“, „an error occurred“, „unable to“, „please“.
- Ausgabe auf stderr: `error: <modul>: <meldung>`, darunter Ursachen mit `  caused by: …`.
- Kein `unwrap`/`expect` außerhalb von Tests.

Beispiele:

```
error: config: ~/.config/nostar/config.toml:4: unknown key "fallbak"
error: config: fallback: expected "cut", "marker" or "off", got "abschneiden"
error: config: markers: unsupported marker "+"
error: source: sha256 mismatch for entgendert-febca802f868.js: expected 6330c126…, got 9b1c40aa…
error: source: curl exit status 22: https://raw.githubusercontent.com/…/dictionary.js
error: source: dictionary.js: field "irregular" not found
error: rules: rules.tsv missing, run "nostar rules build"
error: rules: rules.tsv built from other config, run "nostar rules build"
error: rules: 3 of 214 cases differ
  "Kolleg*innen": expected "Kollegen", got "Kolleg"
error: font: NotoSans-Regular.ttf: no glyph for U+00E4
error: font: NotoSans-Regular.ttf: GSUB: subtable of lookup 12 exceeds 64 KiB after split
error: verify: NotoSans-Regular.ttf: "Ärzt*in" shaped as "Ärzt*in", expected "Arzt"
error: lock: held by another nostar process: ~/.local/state/nostar/lock
error: fontconfig: fc-cache exit status 1: <stderr von fc-cache>
```

### 12.1 Exit-Codes

| Code | Fall |
|---|---|
| 0 | Erfolg |
| 1 | `apply`: mindestens ein Font `failed`, Rest erfolgreich; `rules test`: Abweichungen |
| 2 | Abbruch: Konfiguration, Datenquelle, Regelliste, Sperre, fontconfig |

## 13. Code-Regeln

- `cargo fmt`, `cargo clippy -- -D warnings` (Standard-Lints, kein `pedantic`)
- Kein `unsafe`
- Öffentliche Funktionen nur, wo ein anderes Modul sie braucht
- Kommentare erklären das **Warum**, nicht das Was. Doc-Kommentare an öffentlichen Typen und Funktionen.
- Keine globalen Zustände außer dem Logger
- Funktionen über ~50 Zeilen sind ein Hinweis zum Aufteilen, keine harte Grenze
- Keine Feature-Flags, keine bedingte Kompilierung

## 14. Tests

Setzt fachl. Spezifikation 13 um.

### 14.1 Testebenen

| Ebene | Ort | Inhalt | Wann |
|---|---|---|---|
| Unit | jeweils im Modul | decode (7.2), Edit-Skripte (9.1), Klassenbildung, Index-Verschiebung (9.5), Konfigprüfung | `cargo test` |
| Regeln | `tests/engine.rs` | Referenz-Umsetzung (8.5) mit festem Wörterbuch-Ausschnitt gegen `cases.tsv` | `cargo test`, `nostar rules build`, `nostar rules test` |
| Font | `tests/font.rs` | Fixture-Font patchen, Prüfung 9.6 muss bestehen; Font ohne GSUB; variabler Font | `cargo test` |
| Font im Betrieb | `verify.rs` | Prüfung 9.6 an jedem gepatchten Font | `nostar apply` |
| Geschwindigkeit | `tests/font.rs`, `#[ignore]` | Formung von 1 MB Text mit und ohne Genderzeichen, Original vs. Kopie | `cargo test -- --ignored` |
| System | manuell | Darstellung in GTK, Qt, Firefox, Chrome | vor Freigabe |

Der Fixture-Font ist eine Latin-Teilmenge von Noto Sans (OFL) mit Ligaturen, weniger als 100 KB.

Die Testsammlung `tests/cases.tsv` (8.7) setzt fachl. Spezifikation 13.1 um.

### 14.2 Nachweis der Definition of Done

| DoD (fachl. 13.2) | Nachweis |
|---|---|
| 1. Testsammlung wird überall korrekt angezeigt | Regeln + Font automatisch; GTK, Qt, Firefox, Chrome manuell mit `cases.tsv` als Textdatei bzw. HTML-Seite |
| 2. Kopieren liefert Originaltext | manuell |
| 3. Text ohne Genderformen unverändert | Prüfung 9.6 „Unverändert“ |
| 4. Größe und Geschwindigkeit | Font-Test (Größe), Geschwindigkeitstest |
| 5. Zweites `apply` ändert nichts | manuell: `apply` zweimal, zweite Ausgabe `patched 0` |
| 6. `remove` stellt Ausgangszustand her | manuell: `fc-list`, `fc-match` und `find ~/.local ~/.config` vor `apply` und nach `remove` vergleichen |
| 7. Beschädigter Font | manuell: beschädigte Kopie eines Fonts unter `families` eintragen |

## 15. Abweichungen von der fachlichen Spezifikation

| # | Fachliche Spezifikation | Hier | Grund |
|---|---|---|---|
| 1 | Befehle deutsch (`daten aktualisieren`, `regeln bauen`, `zeige`) | englisch (`data update`, `rules build`, `show`) | Vorgabe: Code auf Englisch |
| 2 | german-nouns als zweite Quelle | nur Entgendert | german-nouns enthält keine Fugenformen und keine männlich/weiblich-Paare; Entgendert ist bereits daraus erzeugt |
| 3 | Endungsregeln selbst ableiten (≥ 98 %) | aus Entgendert übernehmen | Dort mit demselben Verfahren abgeleitet (98,8 %); eigene Ableitung wäre doppelter Code |
| 4 | Stufen 2 und 3 getrennt | zusammengelegt (8.1) | Gleiches Ergebnis, eine Regel statt zwei |
| 5 | Rückfall für alle Genderzeichen | nicht für `/` und `/-` | Schutz von Pfaden und URLs |
| 6 | Ergänzungen in eigener Datei `~/.config/nostar/ergaenzungen` | Abschnitt `[words]` in `config.toml` | Eine Datei weniger, Vorgabe TOML |
| 7 | Konfiguration `~/.config/nostar/config` | `config.toml` | TOML |

## 16. Quellen

- Entgendert, Wörterbuch und Format: https://github.com/janduettmann/Entgendert
- HarfBuzz, GSUB-Größenlimits und Extension-Lookups: https://flutter.googlesource.com/third_party/harfbuzz/+/HEAD/docs/repacker.md
- OpenType GSUB: https://learn.microsoft.com/en-us/typography/opentype/spec/gsub
