# nostar

Zeigt gegenderte Wörter auf dem ganzen Linux-Desktop im generischen Maskulinum an, ohne Browser-Erweiterung.

![Vorher und nachher](img/before-after.png)

`nostar` ändert keinen Text. Es baut Regeln in Kopien deiner Schriftarten ein. Beim Zeichnen wird aus `Lehrer*innen` dann „Lehrer“, aus `Ärzt:innen` „Ärzte“. Kopieren, Suchen und Speichern sehen weiterhin den Originaltext.

> Status: Version 1, abgenommen am 2026-10-28 (GTK, Qt, Firefox, Chrome).

## So funktioniert es

![Ablauf](img/pipeline.svg)

1. Ein Wörterbuch mit ~4.500 Personenbezeichnungen ([Entgendert](https://github.com/janduettmann/Entgendert), aus Wiktionary) wird zu einer Regelliste.
2. Die Regeln landen als OpenType-Feature `calt` in Kopien deiner Fonts unter `~/.local/share/fonts/nostar/`.
3. Eine fontconfig-Regel sorgt dafür, dass Programme die Kopien statt der Originale verwenden.

Systemdateien werden nicht verändert, root-Rechte sind nicht nötig.

## Installation

Voraussetzungen: Rust ab 1.89, fontconfig (`fc-list`, `fc-match`, `fc-cache`), `curl`.

```sh
cargo install --path .   # installiert ~/.cargo/bin/nostar
```

## Benutzung

```sh
nostar data update      # Wörterbuch laden (feste Version, Prüfsumme)
nostar rules build      # Regelliste erzeugen und testen
nostar apply            # Fonts patchen und einrichten
```

Danach laufende Programme neu starten. Nach Updates der System-Schriften zeigt `nostar status` veraltete Kopien; `nostar apply` aktualisiert sie.

```sh
nostar show "jede*r Kolleg*in"   # Was würde angezeigt? -> jeder Kollege
nostar status                    # Was ist gepatcht, was ist veraltet?
nostar remove                    # Alles rückgängig machen
```

## Konfiguration

`~/.config/nostar/config.toml`. Alle Schlüssel sind optional.

```toml
families = ["JetBrains Mono"]                   # zusätzlich zu sans-serif/serif/monospace und den GNOME-Schriften
markers  = ["*", ":", "_", "·", "/", "/-"]
fallback = "cut"                                # unbekannte Wörter: "cut" | "marker" | "off"

[words.Freund]                                  # eigene Wörter
singular = "Freund"
plural   = "Freunde"
```

## Grenzen

- Wirkt nur, wo Programme Systemschriften nutzen. Webfonts, PDFs, Electron-Apps und Text in Bildern bleiben unverändert.
- Keine Grammatik: „mit den Lehrer\*innen“ wird zu „mit den Lehrer“.
- Wird ein Wort in Stücken gezeichnet (teilweise fett, Zeilenumbruch), greift die Regel nicht.
- Qt-Programme mit einem Font ohne Glyphe für den weichen Trennstrich (z.B. Noto Sans Mono): Wörter mit weichem Trennstrich werden nur teilweise umgewandelt.

## Dokumentation

- [SPEZIFIKATION.md](SPEZIFIKATION.md): was das Programm tut
- [TECHNIK.md](TECHNIK.md): wie es gebaut ist

## Lizenz

Wörterbuchdaten: CC BY-SA 4.0 (Entgendert, Wiktionary). Sie werden bei `data update` geladen und nicht mit `nostar` verteilt.
