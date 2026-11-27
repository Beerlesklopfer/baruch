# Webseite

Quelle der Webseite `https://baruch.bernau.family/`, gebaut mit [Hugo](https://gohugo.io). Diese Datei beschreibt
nur das **lokale Bauen und die Vorschau**. Ausgeliefert wird die Seite ausschließlich von der CI (GitHub Pages,
`.github/workflows/`, beschrieben in `docs/Verteilung.md`) – niemand lädt von Hand etwas hoch.

## Bauen

Hugo ab 0.146 genügt, die einfache Fassung (nicht „extended“): Das Theme nutzt kein SCSS und keine
Bildverarbeitung.

```sh
hugo --minify --panicOnWarning -s site      # aus dem Hauptordner; Ausgabe in site/public/
```

`--panicOnWarning` macht jede Warnung zum Fehler; so prüft auch die CI. `site/public/`, `site/resources/` und
`.hugo_build.lock` stehen in `.gitignore` und werden nie eingecheckt.

Nach dem Bau prüft `scripts/site-check.py` die Ausgabe (nur Python-Standardbibliothek, kein Netz):

```sh
python3 scripts/site-check.py               # über site/public; Exit 1 mit der Liste der Fundstellen
python3 scripts/site-check.py --selbsttest  # prüft das Skript an einem erfundenen Mini-Baum
```

Es meldet tote interne Links und Anker (`/apt/` ausgenommen), Bilder ohne alt-Text, JavaScript, fremde Adressen
für Stil, Schrift oder Bild, Links auf `docs/Bedienung.md` oder `docs/Betrieb.md` bei GitHub, Links zu github.com
im Text der Hilfe und eine fehlende Weiterleitung von `/bedienung/` auf `/hilfe/`. `scripts/ci-site.sh` ruft es
nach dem Hugo-Bau auf; die CI scheitert also an einem toten Link.

## Vorschau

```sh
hugo server -s site --bind 127.0.0.1 --port 1313     # http://127.0.0.1:1313/
```

Die Vorschau lädt die Seite bei Änderungen neu und fügt dafür ein eigenes Skript ein (nur in der Vorschau; die
gebaute Seite enthält kein JavaScript).

## Aufbau

| Pfad | Inhalt |
|---|---|
| `hugo.toml` | Adresse, Menüs und die Werte `version`, `repo`, `aptFingerprint` |
| `content/` | die sieben festen Seiten: Start (`_index.md`), Funktionen, Installation, Betrieb, MCP und Assistenten, Lizenz, Impressum |
| `content/hilfe/` | der Abschnitt **Hilfe**: Startseite (`_index.md`, übernimmt mit `aliases` die alte Adresse `/bedienung/`) und je Thema eine Seite, geordnet nach `weight` |
| `data/hilfebilder.toml` | die Liste aller Bilder der Hilfe (Name, Seite, was das Bild zeigt); nur Namen von dort gelten im Shortcode `hilfebild` |
| `themes/baruch/` | das Theme, vollständig im Repository: Vorlagen (`layouts/`), ein Stylesheet, das Symbol |
| `static/CNAME` | der Rechnername für GitHub Pages |
| `static/bilder/` | Bildschirmfotos für Startseite und Funktionen (1400×860, dazu `arbeitsplatz-laptop.png` in 1366×768); die Seite baut auch ohne sie |
| `static/bilder/hilfe/` | Bilder der Hilfe: `<name>.png`, daneben wahlweise `<name>-dunkel.png` für das dunkle System |
| `tools/` | erfundenes Werk und Skript für die Bildschirmfotos (`tools/README.md`) |

## Quelle für Bedienung und Betrieb

Seit Runde 37 ist **die Webseite die Quelle**: `content/hilfe/**` ist die einzige Beschreibung für Anwender,
`content/betrieb.md` die einzige für Betreiber. `docs/Bedienung.md` und `docs/Betrieb.md` im Repository sind nur
noch der Einstieg: Verweis auf die Webseite, „Wo steht was“, Stand der Prüfung, Offenes und technische
Anmerkungen (Adressen, Cookie-Namen, Grenzen der Prüfung) – **keine Bedienschritte in beiden Dateien**.

- „Bedienung nachziehen“ heißt: die Hilfeseite im selben Commit ändern; „Betrieb nachziehen“: `content/betrieb.md`.
- Die **Ersteinrichtung** steht nur einmal: Bibeln in `installation.md` (`#bibeln-einrichten`), Assistenten,
  MultiGPT und KI-Programme in `mcp.md` (`#assistenten-einrichten`, `#multigpt-einrichten`, `#mcp-clients`). Die
  Hilfe beschreibt die Seiten und Felder und verweist dorthin.
- Die Seiten verweisen untereinander nur mit gewöhnlichen Links ab `/` auf Seiten oder auf feste Anker
  (`[Text](/hilfe/konten/#lektoren)`), nicht mit `relref`. Feste Anker bleiben stehen, auch wenn eine Überschrift
  sich ändert.
- Der Shortcode `doku` (Link ins Repository) bleibt für Dateien, die wirklich nur dort stehen: `LICENSE`,
  `README.md`, `docs/Verteilung.md`, `docs/RPC.md`, `docs/MCP.md`, `docs/Architektur.md`.

## Schreiben

- **Alltagssprache.** Fachwörter ersetzen oder beim ersten Vorkommen in einem Halbsatz erklären (Fassung statt
  Revision, Assistent statt Persona, Bedeutungssuche statt RAG; MCP, Reverse-Proxy, Token, SWORD, Typst,
  Git-Spiegel, Cookie). Keine Rundennummern, keine Dateinamen des Quelltexts, keine Routen – außer Adressen, die
  man wirklich eintippt (`http://127.0.0.1:8765/`).
- **Tasten für beide Welten:** `{{< taste "Strg+S" "⌘S" >}}` ergibt „Strg+S (Mac: ⌘S)“. Was am Mac gilt, aus dem
  Code belegen; sonst ehrlich „am Mac ungeprüft“.
- **Kein rohes HTML** in Markdown (`unsafe = false`), auch keine HTML-Kommentare – sie brechen den Bau ab.
- **Jede Überschrift trägt einen festen Anker** in Kleinbuchstaben und ASCII: `## Sicherung {#sicherung}`.
- **Geprüftes und Angenommenes trennen:** Was niemand erprobt hat (macOS, Windows, ein echter Proxy), bleibt als
  „nicht geprüft“ gekennzeichnet.
- **Keine echten Namen, Werke oder Daten.** Das Beispielwerk der Bilder ist erfunden.
- Kopf einer Seite: `title`, `description` (ein Satz; erscheint als Vorspann und in der Liste der Hilfe), in der
  Hilfe `weight`; `inhalt: true` setzt auf einer festen Seite ein Inhaltsverzeichnis unter den Vorspann (die
  Hilfeseiten haben es immer).

## Vorlagen

| Vorlage | für |
|---|---|
| `layouts/baseof.html`, `_partials/kopf.html`, `_partials/fuss.html` | Grundgerüst, Kopfzeile mit Hauptmenü, Fußzeile. Im Hauptmenü ist ein Eintrag auch auf den Unterseiten seines Abschnitts markiert (`aria-current="true"`, auf der Seite selbst `"page"`) |
| `layouts/home.html` | Startseite |
| `layouts/page.html` | feste Seiten |
| `layouts/section.html` | Startseite der Hilfe: Liste der Seiten nach `weight` mit `description` |
| `layouts/hilfe/page.html` | Seiten der Hilfe: Navigation des Abschnitts (aktuelle Seite markiert), Inhaltsverzeichnis aus den Überschriften der Ebenen 2 und 3 (`markup.tableOfContents` in `hugo.toml`), „vorige/nächste Seite“ |

Eine **Suche** gibt es bewusst nicht: Sie ginge nur mit JavaScript oder über eine fremde Adresse. Das Stylesheet
(`static/css/baruch.css`) folgt mit `prefers-color-scheme` dem hellen oder dunklen System, lässt Tabellen und die
Navigation der Hilfe auf Telefonbreite waagerecht rollen und blendet im Druck die Navigation aus.

## Regeln

- **Keine fremden Adressen:** keine Skripte, Schriften, Stile oder Bilder von anderen Rechnern, keine Zähler,
  keine Cookies. Schriften sind Systemschriften. Verlinkt werden nur die eigene Seite, das Repository und, im
  Fließtext, was der Text nennt.
- **Ohne JavaScript:** Die Seite enthält keines.
- **Deutsch**, Bezeichner in den Vorlagen ebenfalls deutsch (es ist Inhalt, kein Programm).
- **Version:** `params.version` in `hugo.toml` bestimmt alle Dateinamen und Links der Seite Installation
  (Shortcodes `download`, `datei`, `release`). Einzige Quelle der Version ist `Cargo.toml`; wer sie dort ändert,
  zieht `hugo.toml` nach. Versionsnummern legt nur der Nutzer fest.
- **Fingerabdruck:** `params.aptFingerprint` ist leer, bis der Nutzer den Fingerabdruck des Archivschlüssels
  einträgt. Leer zeigt die Seite Installation einen sichtbaren Platzhalter-Hinweis. Nicht aus dem Netz holen,
  nicht raten.
- **Impressum:** Die Angaben in `content/impressum.md` trägt nur der Betreiber ein. Eine Seite mit
  `platzhalter: true` im Kopf wird sichtbar als Platzhalter markiert.
- **Warnungen beim Bau:** `scripts/ci-site.sh` warnt (ohne abzubrechen), solange der Fingerabdruck leer ist oder
  eine Seite `platzhalter: true` trägt. Der Fingerabdruck steht außerdem in `README.md` und
  `docs/Verteilung.md`; `scripts/ci-fingerprint.sh` prüft, dass alle drei Stellen übereinstimmen, und ein
  signiertes Release scheitert, wenn er fehlt oder nicht zum Schlüssel passt.
- **Sprache in `hugo.toml`:** `locale = "de"`. Der ältere Schlüssel `languageCode` ist seit Hugo 0.158 veraltet
  und bricht mit `--panicOnWarning` den Bau ab.

## Shortcodes

| Aufruf | Ergebnis |
|---|---|
| `{{< bild datei="satz.png" alt="…" titel="…" >}}` | Bild aus `static/bilder/` (Startseite, Funktionen); fehlt die Datei, nichts |
| `{{< hilfebild name="gliederung-seite" alt="…" titel="…" breite="640" dunkel="nein" >}}` | Bild der Hilfe aus `static/bilder/hilfe/<name>.png`, bei dunklem System `<name>-dunkel.png`, falls vorhanden. `name` muss in `data/hilfebilder.toml` stehen, `alt` ist Pflicht (sonst bricht der Bau ab); `titel` ist die Unterschrift, `breite` die größte Anzeigebreite in Pixeln. Fehlt die Datei, nichts. Der Shortcode ist eingefroren: Änderungen nur durch den Integrator |
| `{{< taste "Strg+S" "⌘S" >}}` | Tastenangabe „Strg+S (Mac: ⌘S)“; ohne zweiten Wert nur die erste Taste |
| `{{< download "Baruch-VERSION-x64.msi" >}}` | Link auf die Datei des Release (`VERSION` wird ersetzt) |
| `{{< datei "Baruch-VERSION-x64.msi" >}}` | nur der Dateiname, für Befehle |
| `{{< release >}}` | Link auf die Release-Seite der Version |
| `{{< doku "docs/MCP.md" "MCP" >}}` | Link auf eine Datei im Repository (Zweig `main`); nicht für Bedienung und Betrieb, siehe oben |
| `{{< fingerabdruck >}}` | Fingerabdruck des Archivschlüssels oder der Platzhalter-Hinweis |
