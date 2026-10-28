# nostar

[English](README.md)

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

- Debian, Ubuntu: `.deb` von der [Release-Seite](https://github.com/NoahJeanA/nostar/releases)
- Fedora, openSUSE: `.rpm` von der Release-Seite
- Arch Linux: AUR-Paket `nostar` oder `nostar-bin`
- Mit Rust (ab 1.89): `cargo install nostar`

Zur Laufzeit nötig: fontconfig (`fc-list`, `fc-match`, `fc-cache`) und `curl`; `gsettings` ist optional (GNOME-Schriften).

Vor dem Deinstallieren des Pakets `nostar remove` ausführen: Das Paket entfernt nur das Programm, nicht die Kopien in deinem Home-Verzeichnis.

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

## Schriftarten

`nostar` verändert Kopien deiner installierten Schriftarten und behält ihre Namen bei. Die Kopien sind nur für deinen eigenen Gebrauch bestimmt; gib sie nicht weiter. Manche Lizenzen (z.B. die OFL mit „Reserved Font Name“ bei Liberation) erlauben veränderte Versionen nur unter anderem Namen. Schriften, die du nicht verändern darfst, schließt du mit `exclude` aus.

## Lizenz

`nostar`: GPL-3.0-or-later, siehe [LICENSE](LICENSE).

Wörterbuchdaten: CC BY-SA 4.0 (Entgendert, Wiktionary). Sie werden bei `data update` geladen und nicht mit `nostar` verteilt.

Testdaten mit eigenen Lizenzen: Schriften in `tests/fixtures/` (OFL, siehe `OFL.txt`), Wörterbuch-Ausschnitt `tests/fixtures/dictionary.js` (CC BY-SA 4.0), Referenztexte in `tests/bench/` (CC BY-SA 3.0 und PSF, siehe `tests/bench/README.md`).
