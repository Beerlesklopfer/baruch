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
| `content/` | die acht Seiten: Start (`_index.md`), Funktionen, Installation, Bedienung, Betrieb, MCP und Assistenten, Lizenz, Impressum |
| `themes/baruch/` | das Theme, vollständig im Repository: Vorlagen (`layouts/`), ein Stylesheet, das Symbol |
| `static/CNAME` | der Rechnername für GitHub Pages |
| `static/bilder/` | Bildschirmfotos (1400×860, dazu `arbeitsplatz-laptop.png` in 1366×768); die Seite baut auch ohne sie |
| `tools/` | erfundenes Werk und Skript für die Bildschirmfotos (`tools/README.md`) |

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
- **Impressum:** `content/impressum.md` ist ein sichtbar markierter Platzhalter (`platzhalter: true`). Die
  Angaben trägt der Betreiber ein; danach `platzhalter` entfernen.
- **Warnungen beim Bau:** `scripts/ci-site.sh` warnt (ohne abzubrechen), solange der Fingerabdruck leer ist oder
  eine Seite `platzhalter: true` trägt. Der Fingerabdruck steht außerdem in `README.md` und
  `docs/Verteilung.md`; `scripts/ci-fingerprint.sh` prüft, dass alle drei Stellen übereinstimmen, und ein
  signiertes Release scheitert, wenn er fehlt oder nicht zum Schlüssel passt.
- **Sprache in `hugo.toml`:** `locale = "de"`. Der ältere Schlüssel `languageCode` ist seit Hugo 0.158 veraltet
  und bricht mit `--panicOnWarning` den Bau ab.

## Shortcodes

| Aufruf | Ergebnis |
|---|---|
| `{{< bild datei="satz.png" alt="…" titel="…" >}}` | Bild aus `static/bilder/`; fehlt die Datei, nichts |
| `{{< download "Baruch-VERSION-x64.msi" >}}` | Link auf die Datei des Release (`VERSION` wird ersetzt) |
| `{{< datei "Baruch-VERSION-x64.msi" >}}` | nur der Dateiname, für Befehle |
| `{{< release >}}` | Link auf die Release-Seite der Version |
| `{{< doku "docs/Betrieb.md" "Betrieb" >}}` | Link auf eine Datei im Repository (Zweig `main`) |
| `{{< fingerabdruck >}}` | Fingerabdruck des Archivschlüssels oder der Platzhalter-Hinweis |
