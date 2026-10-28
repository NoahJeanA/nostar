# Spezifikation: `nostar`

Status: Version 1, abgenommen · Stand: 2026-10-28

---

## 1. Ziel

`nostar` sorgt dafür, dass gegenderte Wortformen auf dem eigenen Linux-System im **generischen Maskulinum** angezeigt werden:

| Text | Anzeige |
|---|---|
| Lehrer\*innen | Lehrer |
| Kolleg\*innen | Kollegen |
| Ärzt:innen | Ärzte |
| jede\*r | jeder |

Das geschieht auf Ebene der **Schriftarten (Fonts)**. Es wird **nur die Darstellung** verändert. Der Text selbst (in Dateien, Zwischenablage, Suche, Netzwerk) bleibt unverändert.

## 2. Nicht-Ziele

- Keine Änderung von Textinhalten, Dateien oder Netzwerkverkehr
- Keine Browser-Erweiterung, kein Proxy, keine Kernel-Komponente
- Keine Änderung an Dateien außerhalb des Home-Verzeichnisses; keine root-Rechte
- Keine vollständige Grammatikprüfung
- Keine Abdeckung von Text in Bildern, Videos oder PDFs mit eingebetteten Schriften

## 3. Begriffe

| Begriff | Bedeutung |
|---|---|
| **Buchstabe** | Jedes Unicode-Zeichen der Kategorie „Letter“ (Lu, Ll, Lt, Lm, Lo), z.B. a–z, ä, ö, ü, ß, é |
| **Wort** | Eine ununterbrochene Folge aus Buchstaben und Genderzeichen. Vor und nach dem Wort steht ein Nicht-Buchstabe oder der Anfang bzw. das Ende des Textes. |
| **Genderzeichen** | Ein Zeichen, das eine Genderform markiert, siehe 4.1 |
| **Genderform** | Ein Wort, das ein Genderzeichen enthält, z.B. `Lehrer*innen` |
| **Zielform** | Die Form, die stattdessen angezeigt wird, z.B. `Lehrer` |
| **Stamm** | Der Teil vor dem Genderzeichen, z.B. `Lehrer`, `Ärzt` |
| **Endung** | Der Teil nach dem Genderzeichen, z.B. `innen`, `in`, `r`, `e` |
| **Regel** | Zuordnung einer Genderform (oder eines Musters) zu einer Zielform |
| **Regelliste** | Font-unabhängige, geprüfte Sammlung aller Regeln (Abschnitt 6) |
| **Original-Font** | Eine installierte Font-Datei, z.B. unter `/usr/share/fonts/` |
| **Gepatchter Font** | Kopie eines Original-Fonts mit eingebauter Regelliste |
| **Manifest** | Vom Programm geführte Liste aller gepatchten Fonts und ihrer Herkunft |

## 4. Erkennung und Umwandlung

### 4.1 Genderzeichen

| Zeichen | Beispiel | Voreinstellung |
|---|---|---|
| `*` | Lehrer\*innen | an |
| `:` | Lehrer:innen | an |
| `_` | Lehrer_innen | an |
| `·` (U+00B7) | Lehrer·innen | an |
| `/` und `/-` | Lehrer/innen, Lehrer/-innen | an |
| Binnen-I | LehrerInnen | an, **nur für Stämme aus dem Wörterbuch** (Stufe 2), damit Wörter wie „LinkedIn“ oder „PlugIn“ unberührt bleiben |

Die aktiven Zeichen sind konfigurierbar (Abschnitt 11).

### 4.2 Stufen

Für jede Genderform werden die Stufen **der Reihe nach** geprüft. Die **erste passende** Stufe bestimmt die Zielform.

| Stufe | Inhalt | Beispiele |
|---|---|---|
| **1. Feste Formen** | Artikel, Pronomen, Determinative, Adjektivendungen, substantivierte Adjektive | `der*die` → der · `ein*e` → ein · `eine*n` → einen · `jede*r` → jeder · `Vorsitzende*r` → Vorsitzender · `Angestellte*n` → Angestellten |
| **2. Wörterbuch** | Bekannte Personenbezeichnungen, auch unregelmäßige | `Ärzt*innen` → Ärzte · `Ärzt*in` → Arzt · `Bäuer*innen` → Bauern · `Kolleg*in` → Kollege · `Beamt*innen` → Beamte |
| **3. Zusammensetzungen** | Unbekannter Stamm, dessen **Ende** ein Wörterbuch-Stamm ist | `Stammkund*innen` → Stammkunden · `Oberärzt*innen` → Oberärzte |
| **4. Endungsregeln** | Unbekannter Stamm mit typischer Endung | `Polizist*innen` → Polizisten (`-ist`) · `Ingenieur*innen` → Ingenieure (`-eur`) · `Arbeiter*innen` → Arbeiter (`-er`) |
| **5. Rückfall** | Nichts passt | gemäß Einstellung `fallback` (Abschnitt 11) |

Endungsregeln (Stufe 4) werden aus dem Wörterbuch **abgeleitet** und nur übernommen, wenn sie auf den Wörterbucheinträgen eine Trefferquote von mindestens 98 % erreichen. Mehrdeutige Endungen (z.B. `-ier`) erhalten keine Regel.

### 4.3 Genderzeichen in der Wortmitte

Steht nach der Endung noch ein Wortteil (Zusammensetzung), wird nur die Genderform umgewandelt, der Rest bleibt:

| Text | Anzeige |
|---|---|
| Lehrer\*innenzimmer | Lehrerzimmer |
| Ärzt\*innenkammer | Ärztekammer |
| Kund\*innenservice | Kundenservice |
| Lehrer\*innen-Konferenz | Lehrer-Konferenz |

Die Zielform für Zusammensetzungen ist die Plural-Zielform. Eine abweichende Fugenform kann in den eigenen Ergänzungen angegeben werden (Abschnitt 11; das Entgendert-Wörterbuch führt keine Fugenformen).

### 4.4 Groß- und Kleinschreibung

Jede Regel gilt auch für:
- Wortanfang groß oder klein (`Lehrer*innen`, `lehrer*innen` z.B. in Überschriften-Stilen)
- großgeschriebene Endung (`Lehrer*Innen`)
- durchgehende Großschreibung (`LEHRER*INNEN` → LEHRER)

Die Zielform übernimmt die Schreibweise des Originals.

### 4.5 Verbindliche Beispiele

Diese Fälle sind Teil der Testsammlung (13.1) und müssen exakt so angezeigt werden. Die Spalte „Stufe“ nennt, woher die Zielform fachlich stammt. `nostar show` kann Stufe 4 melden, wo hier 2 steht (z.B. „Lehrer\*innen“, „Kolleg\*in“): Liefert die Endungsregel dasselbe Ergebnis, wird die Wörterbuchregel beim Bauen weggelassen (TECHNIK.md 8.3).

| Text | Anzeige | Stufe |
|---|---|---|
| Lehrer\*innen | Lehrer | 2 |
| Lehrer\*in | Lehrer | 2 |
| Kolleg\*innen | Kollegen | 2 |
| Kolleg\*in | Kollege | 2 |
| Ärzt:innen | Ärzte | 2 |
| Ärzt\*in | Arzt | 2 |
| Bäuer\*innen | Bauern | 2 |
| Kund_innen | Kunden | 2 |
| KollegInnen | Kollegen | 2 (Binnen-I) |
| Stammkund\*innen | Stammkunden | 3 |
| Polizist\*innen | Polizisten | 4 |
| der\*die | der | 1 |
| jede\*r | jeder | 1 |
| ein\*e | ein | 1 |
| Vorsitzende\*r | Vorsitzender | 1 |
| Lehrer\*innenzimmer | Lehrerzimmer | 2 |
| LEHRER\*INNEN | LEHRER | 2 |
| 5 \* 3 | 5 \* 3 | keine Genderform |
| 5\*3 | 5\*3 | keine Genderform |
| x\*y | x\*y | keine Genderform |
| \*wichtig\* | \*wichtig\* | keine Genderform |
| Fußnote\* | Fußnote\* | keine Genderform |
| 12:30 | 12:30 | keine Genderform |
| https://example.org | unverändert | keine Genderform |
| LinkedIn | LinkedIn | Binnen-I nicht im Wörterbuch |
| PlugIn | PlugIn | Binnen-I nicht im Wörterbuch |

### 4.6 Bewusst nicht behandelt (Version 1)

| Fall | Beispiel | Anzeige in Version 1 | Grund |
|---|---|---|---|
| Fall (Kasus) aus dem Satz | mit den Lehrer\*innen | mit den Lehrer | Braucht das Wort davor; siehe 4.7 |
| Doppelnennung | Lehrerinnen und Lehrer | unverändert | Umfasst mehrere Wörter, siehe 4.7 |
| Neutrale Formen | Studierende | unverändert | Ohne Genderzeichen müsste die Regel an einem häufigen Buchstaben ansetzen; das verlangsamt jeden Text (TECHNIK.md 9.2) |
| Mehrteilige neutrale Pronomen | der/die/das, er/sie/es | unverändert | Keine eindeutige Zielform |
| Weibliche Form vorne | die\*der, sie\*er | unverändert | Die Regel müsste an einem häufigen Buchstaben (`i`, `s`) ansetzen; das verlangsamt jeden Text um etwa 8 % (Spike, TECHNIK.md 9.2) |

### 4.7 Erweiterung: Wortübergreifende Regeln (nicht Version 1)

Regeln, die das **vorherige Wort** einbeziehen (z.B. „den Lehrer\*innen“ → „den Lehrern“), sind technisch möglich. Sie beziehen aber das Leerzeichen in die Regeln ein. Firefox und Chrome stellen Text normalerweise Wort für Wort dar und schalten das für solche Fonts ab, was die Darstellung verlangsamen kann. Diese Erweiterung wird erst nach Messung der Auswirkungen erwogen. Kandidaten: „den“, „mit den“, „von den“, „zu den“, „bei den“, „aus den“.

### 4.8 Bekannte Lücken

| Lücke | Folge |
|---|---|
| Wort kommt in mehreren Stücken an (teilweise fett oder verlinkt, Zeilenumbruch im Wort, ein Zeichen fehlt im Font) | Original bleibt sichtbar |
| Weiche Trennstriche (U+00AD) im Wort, häufig auf Nachrichtenseiten | Die Regel greift, der unsichtbare Trennstrich bleibt an seiner Stelle (HarfBuzz überspringt ihn beim Vergleich). Steht er direkt am Wortende („Lehrer\*in\u00AD“), bleibt die Form unverändert. |
| Weicher Trennstrich im Wort, Qt-Programm, Font ohne Glyphe für U+00AD (z.B. Noto Sans Mono) | Qt nimmt den Trennstrich aus einem Ersatzfont und teilt das Wort; die Regeln sehen nur Bruchstücke, es entstehen Teilformen wie „Lehrernen“. GTK und Browser sind nicht betroffen. Behebbar nur durch eine zusätzliche Glyphe (8.1), daher nicht in Version 1 |
| Programme mit eigenen Fonts (Webfonts, Electron-Apps, PDFs) oder ohne HarfBuzz | Original bleibt sichtbar |
| Cursor und Markieren | In umgewandelten Wörtern springt der Cursor durch ausgeblendete Zeichen |

## 5. Datenquellen

| Quelle | Inhalt | Lizenz | Verwendung |
|---|---|---|---|
| **Entgendert** (janduettmann/Entgendert) | Wörterbuch mit ~4.500 Personenbezeichnungen inkl. Plural und Dativ Plural; 42 Endungsregeln | Code MIT, Wörterbuch CC BY-SA 4.0 | **Primäre Quelle** für Stufen 1, 2, 4 |
| **german-nouns** (gambolputty/german-nouns) | ~100.000 Substantive mit Beugungsformen aus Wiktionary | CC BY-SA 4.0 | Ergänzung, Fugenformen (4.3), Neuerzeugung des Wörterbuchs |
| **RGGL** (motsiw/rggl) | Handgepflegte Sonderfälle | GPL-3.0 | Nur als Ideenquelle für Testfälle, keine Übernahme von Code oder Daten |
| **Eigene Ergänzungen** | Korrekturen und Zusätze des Nutzers | – | Hat Vorrang vor allen anderen Quellen |

Anforderungen:
- Jede Quelle wird in einer **festen Version** verwendet und per Prüfsumme gesichert.
- Das Programm lädt **nichts selbstständig** aus dem Netz. Das Aktualisieren der Daten ist ein eigener, ausdrücklicher Schritt.
- Bei Widersprüchen gilt: eigene Ergänzungen > Entgendert > german-nouns.

## 6. Regelliste

Die Regelliste ist das **font-unabhängige** Zwischenergebnis zwischen Daten und Fonts. Sie wird einmal erzeugt, geprüft und dann in jeden Font eingebaut.

### 6.1 Inhalt einer Regel

| Feld | Bedeutung | Beispiel |
|---|---|---|
| Stufe | 1–5 | 2 |
| Stamm | Feste Zeichenfolge oder Endungsmuster | `Ärzt` |
| Genderzeichen | Eines oder mehrere aus 4.1 | `*`, `:`, `_` … |
| Endung | Feste Zeichenfolge | `innen` |
| Position | `Wortende` oder `vor Wortteil` (4.3) | `Wortende` |
| Zielform | Was angezeigt wird | `Ärzte` |
| Quelle | Herkunft der Regel | `Entgendert 0.3.0` |

### 6.2 Erzeugung

1. Daten aller Quellen einlesen und zusammenführen (Vorrang gemäß Abschnitt 5)
2. Für jeden Eintrag alle Varianten erzeugen: Genderzeichen × Groß/Klein (4.4) × Position (4.3)
3. Endungsregeln ableiten und gegen das Wörterbuch prüfen (4.2)
4. Konflikte melden: zwei Regeln mit gleichem Muster und unterschiedlicher Zielform
5. Regeln nach Stufe und innerhalb einer Stufe nach **Länge des Musters absteigend** ordnen, damit das genaueste Muster zuerst greift

### 6.3 Prüfung

Bevor eine Regelliste in Fonts eingebaut wird, muss sie die gesamte Testsammlung (13.1) bestehen.

## 7. Auswahl der Fonts

### 7.1 Kandidaten

Ermittlung über fontconfig. Zwei Modi:

- **„standard“** (Voreinstellung): die Fonts hinter `sans-serif`, `serif`, `monospace`, die Oberflächen-, Dokument- und Monospace-Schrift aus den GNOME-Einstellungen (falls vorhanden), die in der Konfiguration genannten Familien (z.B. Terminalschrift) sowie die Ersatzschriften für Arial, Times New Roman und Courier New. Jeweils **alle Schnitte**.
- **„alle“**: jeder installierte Font, der 7.2 erfüllt

### 7.2 Ausschlusskriterien

Ein Font wird **nicht** gepatcht, wenn:
- er weniger als 20 Buchstaben im Sinne von Abschnitt 3 abdeckt (Symbol-, Icon-, Emoji- und Fonts für andere Schriftsysteme),
- ihm Buchstaben fehlen, die für die deutsche Regelliste nötig sind (a–z, A–Z, ä, ö, ü, Ä, Ö, Ü, ß),
- er kein einziges der aktiven Genderzeichen enthält,
- ein Genderzeichen oder der Bindestrich dieselbe Glyphe wie ein Buchstabe oder der weiche Trennstrich hat (z.B. Icon-Fonts); die Regeln könnten sie nicht unterscheiden,
- er eine Font-Sammlung (`.ttc`/`.otc`) oder ein Bitmap-Format (`.pcf`, `.bdf`) ist,
- er in der Konfiguration ausgeschlossen ist,
- er bereits ein gepatchter Font von `nostar` ist.

Fehlt einem Font nur ein einzelnes Genderzeichen, werden die Regeln für dieses Zeichen weggelassen, alle anderen eingebaut.

## 8. Veränderung eines Fonts

Wie die Regeln in den Font eingebaut werden, regelt [TECHNIK.md](TECHNIK.md), Abschnitt 9.

### 8.1 Grundsätze

- Der Original-Font wird **niemals verändert**. Es wird immer eine Kopie geschrieben.
- Alle bestehenden Glyphen, Namen und Funktionen des Fonts (Ligaturen, Kerning, Sprachfunktionen) bleiben erhalten.
- Der **Familienname bleibt gleich**, damit Programme den Font unter seinem bekannten Namen finden.
- Es werden **keine Glyphen hinzugefügt oder verändert**. Die Zielformen bestehen ausschließlich aus vorhandenen Buchstaben.
- Unterstützt werden TrueType-Fonts (`.ttf`), OpenType-Fonts mit CFF-Konturen (`.otf`) und variable Fonts.

### 8.2 Wirkung

- Die Regeln sind in jedem Programm aktiv, das die Standardfunktionen des Fonts nutzt. Der Nutzer muss nichts einschalten.
- Sie gelten für alle Sprach- und Schrifteinstellungen eines Textes.
- Eigene Ersetzungen des Fonts, z.B. Ligaturen wie `fi` oder `fl`, dürfen die Erkennung **nicht** verhindern („Influencer\*innen“ wird erkannt).

### 8.3 Größe und Geschwindigkeit

- Erwarteter Zuwachs: etwa 1 MB pro Font. Mehr als 5 MB pro Font gilt als Fehler.
- Text darf **nicht spürbar langsamer** dargestellt werden. Gemessen wird der Mehraufwand gegenüber dem Original-Font je MB Text, mit HarfBuzz auf festen Referenztexten (TECHNIK.md 9.7, 14.1):

| Text | Höchstens |
|---|---|
| Prosa ohne Genderformen | +25 ms je MB |
| Code, URLs, Pfade (viele `*`, `:`, `/`) ohne Genderformen | +40 ms je MB |
| Text mit Genderformen | +100 ms je MB |

Zum Vergleich: Eine Bildschirmseite Text hat etwa 5 KB, der Mehraufwand liegt dort also bei höchstens 0,5 ms. Ein Verhältnis zur Originalzeit wäre ungeeignet: Der Mehraufwand ist in allen Fonts etwa gleich, sehr schnelle Fonts sähen im Verhältnis aber deutlich schlechter aus.

### 8.4 Prüfung vor der Verwendung

Jeder gepatchte Font wird vor seiner Verwendung automatisch gegen die Testsammlung (13.1) geprüft. Zusätzlich wird geprüft, dass Text ohne Genderformen genauso dargestellt wird wie im Original. Schlägt eine Prüfung fehl, wird die Kopie verworfen und das Original bleibt aktiv.

## 9. Einbindung ins System

### 9.1 Ablageorte

| Was | Pfad |
|---|---|
| Gepatchte Fonts | `~/.local/share/fonts/nostar/` + **gespiegelter Originalpfad** |
| fontconfig-Regel | `~/.config/fontconfig/conf.d/99-nostar.conf` |
| Konfiguration | `~/.config/nostar/config` |
| Eigene Ergänzungen | `~/.config/nostar/ergaenzungen` |
| Datenquellen (Kopien) | `~/.local/share/nostar/daten/` |
| Regelliste | `~/.local/share/nostar/regeln` |
| Manifest, Protokoll | `~/.local/state/nostar/` |

### 9.2 fontconfig-Regel

`99-nostar.conf` blendet **jeden erfolgreich gepatchten Original-Font** über seinen exakten Dateipfad aus (`<selectfont><rejectfont>`). Originale, deren Patch fehlgeschlagen ist, werden **nicht** ausgeblendet.

### 9.3 Cache

Nach jeder Änderung wird `fc-cache` ausgeführt. Laufende Programme übernehmen Änderungen erst nach einem Neustart. Darauf weist das Programm hin.

## 10. Befehle

| Befehl | Wirkung |
|---|---|
| `nostar daten aktualisieren` | Lädt die Datenquellen in einer **ausdrücklich angegebenen** Version, prüft Prüfsummen. Einziger Befehl mit Netzwerkzugriff. |
| `nostar regeln bauen` | Erzeugt die Regelliste aus den Daten (6.2) und prüft sie (6.3) |
| `nostar regeln testen` | Führt die Testsammlung gegen die Regelliste aus und zeigt Abweichungen |
| `nostar zeige "TEXT"` | Zeigt, wie ein Text umgewandelt würde, mit Stufe und Quelle der greifenden Regel |
| `nostar apply` | Fonts patchen und einrichten |
| `nostar status` | Zustand anzeigen |
| `nostar remove` | Alles rückgängig machen |

### 10.1 `nostar apply`

1. Regelliste laden; fehlt sie oder ist sie ungeprüft: Abbruch
2. Kandidaten bestimmen (Abschnitt 7)
3. Für jeden Kandidaten: überspringen, wenn er im Manifest steht und sich weder Font noch Regelliste geändert haben (Prüfsummen); sonst patchen (Abschnitt 8)
4. Kopien entfernen, deren Original nicht mehr existiert oder nicht mehr ausgewählt ist
5. fontconfig-Regel neu schreiben, Cache aktualisieren
6. Zusammenfassung: gepatcht / unverändert / übersprungen (mit Grund) / fehlgeschlagen (mit Grund)

`apply` ist **idempotent**. Option `--dry-run` zeigt nur an, was passieren würde.

### 10.2 `nostar status`

Zeigt, ohne etwas zu ändern:
- ob `nostar` aktiv ist
- Version der Datenquellen und der Regelliste, Anzahl der Regeln je Stufe
- Anzahl gepatchter Fonts
- **veraltete** Kopien (Original oder Regelliste seit dem Patchen geändert)
- **neue** Kandidaten, die noch nicht gepatcht sind
- welcher Font für `sans-serif`, `serif` und `monospace` gewählt wird und ob es eine gepatchte Kopie ist

### 10.3 `nostar remove`

Entfernt gepatchte Fonts, fontconfig-Regel, Regelliste, Manifest und Protokoll. Konfiguration, eigene Ergänzungen und Datenquellen bleiben nur mit `--keep-config` erhalten. Danach wird der Cache aktualisiert. Gelöscht werden **ausschließlich** Pfade aus dem Manifest oder unterhalb der Orte aus 9.1.

### 10.4 Rückgabewerte

| Code | Bedeutung |
|---|---|
| 0 | Erfolg |
| 1 | Teilweise Fehler (einzelne Fonts fehlgeschlagen, Rest erfolgreich) |
| 2 | Abbruch (z.B. fontconfig nicht verfügbar, Konfiguration ungültig, Regelliste fehlt oder fällt durch Tests) |

## 11. Konfiguration

| Schlüssel | Bedeutung | Voreinstellung |
|---|---|---|
| `mode` | `standard` oder `alle` (7.1) | `standard` |
| `families` | Zusätzliche Familien (Oberflächen-, Terminalschrift) | leer |
| `exclude` | Familien oder Pfade, die nie gepatcht werden | leer |
| `chars` | Aktive Genderzeichen (4.1) | `* : _ · / /-` |
| `binnen_i` | Binnen-I für Wörterbuch-Stämme | an |
| `fallback` | Stufe 5: `abschneiden` (Stamm ohne Genderzeichen und Endung), `zeichen` (nur Genderzeichen ausblenden), `aus` (unverändert lassen) | `abschneiden` |
| `skip_monospace` | Monospace-Fonts nicht patchen | aus |

## 12. Robustheit

- Dateien werden **atomar** geschrieben (temporäre Datei, dann umbenennen).
- Die fontconfig-Regel wird erst geschrieben, **nachdem** alle Fonts fertig und geprüft sind.
- Ein Fehler bei einem Font bricht nicht den ganzen Lauf ab.
- Nur ein Lauf gleichzeitig (Sperrdatei).
- Netzwerkzugriff ausschließlich in `nostar daten aktualisieren`.

## 13. Tests und Abnahme

Wie und auf welchen Ebenen getestet wird, regelt [TECHNIK.md](TECHNIK.md), Abschnitt 14.

### 13.1 Testsammlung

Paare aus Eingabe und erwarteter Anzeige. Sie enthält mindestens:
- alle Beispiele aus 4.5
- Fälle, die **nicht** umgewandelt werden dürfen: Code, Uhrzeiten, URLs, Markdown, Fußnoten, „LinkedIn“-artige Wörter
- echte Sätze aus Nachrichtenseiten und Behördentexten
- Fälle mit weichen Trennstrichen (4.8)

### 13.2 Definition of Done

`nostar` ist fertig, wenn:

1. Alle Fälle der Testsammlung in einem GTK-Programm, in einem Qt-Programm, in Firefox und in Chrome (mit Systemschriften) exakt wie angegeben angezeigt werden; ausgenommen die Lücke „Qt und Font ohne U+00AD“ aus 4.8.
2. Kopieren von „Lehrer\*innen“ weiterhin `Lehrer*innen` liefert.
3. Text ohne Genderformen in Latein, Griechisch und Kyrillisch genauso aussieht wie mit dem Original-Font, inklusive Ligaturen.
4. Die Grenzen aus 8.3 eingehalten werden.
5. Ein zweites `apply` nichts ändert.
6. Nach `remove` die installierten Fonts, die Schriftauswahl und das Home-Verzeichnis im Zustand vor dem ersten `apply` sind.
7. Ein beschädigter Original-Font als „fehlgeschlagen“ gemeldet wird, das Original aktiv bleibt und alle anderen Fonts gepatcht werden.

## 14. Entscheidungen

Punkte 1–8 entschieden am 2026-10-26 (Punkt 5 geändert am 2026-10-27), Punkte 9–11 am 2026-10-27, Punkte 12–13 am 2026-10-28. Technische Abweichungen von dieser Spezifikation stehen in [TECHNIK.md](TECHNIK.md), Abschnitt 15, und sind freigegeben.

| # | Frage | Entscheidung |
|---|---|---|
| 1 | Rückfall bei unbekannten Wörtern (`fallback`) | `abschneiden` |
| 2 | Aktive Genderzeichen | `* : _ · / /-`, Binnen-I nur für Wörterbuch-Stämme |
| 3 | Fall-Erkennung („den Lehrern“) | Nicht in Version 1 (4.7) |
| 4 | Datenquelle | Nur Entgendert-Wörterbuch (TECHNIK.md 15, Punkt 2) |
| 5 | Neutrale Formen („Studierende“) | Nicht in Version 1 (4.6); Einstellung `participles` entfällt; entschieden 2026-10-27 |
| 6 | Monospace-Fonts ausnehmen | Nein; mit Wörterbuch ist das Risiko für Code gering |
| 7 | Automatisch nach Paket-Updates neu patchen | Nein; `status` weist auf veraltete Kopien hin |
| 8 | Programmiersprache | Rust (fontations: `read-fonts`, `write-fonts`; Prüfung mit `harfrust`) |
| 9 | Feste Formen mit weiblicher Form vorne (`die*der`, `sie*er`) | Nicht in Version 1 (4.6); entschieden 2026-10-27 |
| 10 | Maß für die Geschwindigkeit | Mehraufwand in ms je MB statt Verhältnis (8.3); entschieden 2026-10-27 |
| 11 | Binnen-I | Bleibt an, trotz etwa 2–10 % Mehraufwand bei Prosa (Großbuchstabe `I` in „Ihre“, „Ihnen“); entschieden 2026-10-27 |
| 12 | Qt, weicher Trennstrich und Font ohne Glyphe für U+00AD | Bekannte Lücke (4.8), keine zusätzliche Glyphe; entschieden 2026-10-28 |
| 13 | Oberflächenschriften im Modus „standard“ | Automatisch aus den GNOME-Einstellungen (7.1); entschieden 2026-10-28 |

## 15. Quellen

- Entgendert: https://github.com/janduettmann/Entgendert
- RGGL – Remove German Gender Language: https://github.com/motsiw/rggl
- german-nouns: https://github.com/gambolputty/german-nouns
