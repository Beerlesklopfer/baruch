# Using Baruch

**Usage is described in the help on the website:** <https://baruch.bernau.family/hilfe/> – in the repository the
pages under [`site/content/de/hilfe/`](../site/content/de/hilfe/). It is the only description for users, in
everyday language, step by step and with images. For operators the same holds for
[`site/content/de/betrieb.md`](../site/content/de/betrieb.md) (entry point: [`docs/Betrieb.md`](Betrieb.md)).

This file now only contains:

1. the table **"Where is what"** – every former heading of this file with its new place,
2. the **rule for developers**,
3. **technical notes** per help page: what no user needs (addresses and routes, cookie names,
   parameters, commits in the Git mirror, RPC and MCP names, limits of the verification, assumptions about unchecked things).

Usage steps no longer stand here – never in both files.

## Rule for developers

"Update the usage docs" (AGENTS.md: docs grow with the code) means since Round 37: **change the responsible help page under
`site/content/de/hilfe/` in the same commit.** Technical matters of the same change go into the "Technical
notes" below. For the help pages:

- Everyday language: „Fassung“ instead of „Revision“, „Assistent“ (role) instead of „Persona“; explain technical terms (MCP, Token, SWORD, Typst,
  reverse proxy …) at their first occurrence in a half sentence. No round numbers, no file names of the
  source, no routes (only addresses that one really types in).
- Keys always for both worlds („Strg+S (Mac: ⌘S)“), the Mac key proven from the code; `tasten.md` collects all of them.
- Every heading carries a fixed anchor `{#…}`; every `##` heading has an image
  (`{{< hilfebild name="…" alt="…" titel="…" >}}`, names only from `site/data/hilfebilder.toml`) or stands in the head
  of the page under `ohne_bild`. No raw HTML, also no HTML comments (the build aborts).
- Links only as ordinary links from `/`; to `/installation/`, `/mcp/`, `/betrieb/` only to the page or
  its fixed anchors. No `doku` shortcode and no link to GitHub in the help.
- Examples freely invented; the sample work of the help is called „Ein Jahr auf Mövenoog“ by Mara Tiedemann.
- Checking: `hugo --minify --panicOnWarning -s site` and `cargo test -p baruch-server --test server round37_hilfe`.

Comments in the Rust code that still name `docs/Bedienung.md „…“` mean the section from the table below.

## Where is what

Every `##` heading of the former revision of this file (state `00537dd`, `git show 00537dd:docs/Bedienung.md`):

| Former heading | Help page and anchor |
|---|---|
| „Starten“ | [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#starten`; data folder: `/betrieb/#datenordner` |
| „Typst- oder Markdown-Datei importieren“ | [`einlesen.md`](../site/content/de/hilfe/einlesen.md) `#typst-markdown`, `#markdown`, `#teile`, `#export-import`, `#kommandozeile` |
| „PDF oder EPUB importieren (Runde 21a)“ | [`einlesen.md`](../site/content/de/hilfe/einlesen.md) `#pdf-epub` |
| „Werke“ | [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#werke`, `#neues-werk`, `#predigt`, `#werk-bearbeiten`, `#loeschen-archiv` |
| „Bildschirm“ | [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#bildschirm` |
| „Menüband“ | [`menueband.md`](../site/content/de/hilfe/menueband.md) `#menueband`, `#menue-baruch`, `#reiter`, `#band-einklappen`, `#kompaktes-band`, `#laptop-modus`, `#ohne-javascript`; buttons of the editor: [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#einfuegen`; „Über Baruch“: [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#ueber` |
| „Zeitangaben“ | [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#zeitangaben` |
| „Adresse“ | [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#adresse` |
| „Gliederung“ | [`gliederung.md`](../site/content/de/hilfe/gliederung.md) `#gliederung`, `#ueberschriften`, `#teile`, `#ohne-nummer`, `#seite`, `#antraege`, `#zustimmung-teil`, `#schema`, `#muster` |
| „Zeitwächter“ | [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#zeitwaechter` |
| „Satzbild“ | [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#satzbild`, `#ansichten`, `#einfuegen`, `#bild-einfuegen` |
| „Bilder“ | [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#bilder`, `#bild-einfuegen` |
| „Revisionen“ | [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#fassungen`, `#ausblenden` |
| „Vorschläge im Lesefluss“ | [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#vorschlaege` |
| „Abschnitts-Chat“ | [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#chat` |
| „Assistenten fragen“ | [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#pruefen`, `#websuche` |
| „Befunde (Runde 28)“ | [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#befunde` |
| „Chat bedienen und Aufträge (Runde 28, chat-auftrag)“ | [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#chat`, `#faeden` |
| „Suchen und Ersetzen (Runde 30a)“ | [`suchen.md`](../site/content/de/hilfe/suchen.md) `#suchen`, `#ersetzen` |
| „Querverweise (Runde 30a)“ | [`suchen.md`](../site/content/de/hilfe/suchen.md) `#verweise` |
| „Lösungsvorschläge und Varianten (Runde 30b)“ | [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#pruefen`, `#loesungen`, `#rueckfrage` |
| „Chat: Wiederholen, Löschen, Anrede, Live, gegliederte Beiträge, Sprungmarken (Runde 30c)“ | [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#chat` |
| „Bibelstellen“ | [`bibel-quellen.md`](../site/content/de/hilfe/bibel-quellen.md) `#zitate-pruefen`, `#nachschlagen` |
| „Satz (Typst-Vorschau)“ | [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#satz`, `#satz-einstellungen` |
| „Buch als PDF, Umschlag und Titelei“ | [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#pdf`, `#umschlag`, `#epub` |
| „Stichwortzettel“ | [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#stichwortzettel` |
| „Literatur“ | [`bibel-quellen.md`](../site/content/de/hilfe/bibel-quellen.md) `#literatur`, `#bedeutungssuche`, `#quellen`, `#nachschlagen` |
| „Einfälle“ | [`einfaelle.md`](../site/content/de/hilfe/einfaelle.md) `#festhalten`, `#liste`, `#einfall` |
| „Mindmap“ | [`einfaelle.md`](../site/content/de/hilfe/einfaelle.md) `#mindmap`, `#wolke`, `#mindmap-tasten`, `#mindmap-gliederung`, `#mindmap-austausch` |
| „Fäden“ | [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#faeden` |
| „Abgleich mit MultiGPT“ | [`bibel-quellen.md`](../site/content/de/hilfe/bibel-quellen.md) `#abgleich` |
| „Vergrößern mit Strg+Mausrad“ | [`menueband.md`](../site/content/de/hilfe/menueband.md) `#vergroessern` |
| „Einstellungen“ | [`einstellungen.md`](../site/content/de/hilfe/einstellungen.md) `#oeffnen`, `#darstellung`, `#werk`, `#assistenten`, `#verbindungen`, `#quellen`, `#mcp-zugang`, `#daten`; typesetting: [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#satz-einstellungen`; Bibles (SWORD), commentaries and lexicons: [`bibel-quellen.md`](../site/content/de/hilfe/bibel-quellen.md) `#bibeln-waehlen` |
| „Sicherheit“ | [`austausch-sicherung.md`](../site/content/de/hilfe/austausch-sicherung.md) `#sicherheit`; operation behind a proxy: `/betrieb/#reverse-proxy` |
| „Konten und Rollen (Runde 35)“ | [`konten.md`](../site/content/de/hilfe/konten.md) `#einrichten`, `#anmelden`, `#lektoren`, `#lektor-rechte`, `#passwort` |
| „Austausch zwischen Instanzen“ | [`austausch-sicherung.md`](../site/content/de/hilfe/austausch-sicherung.md) `#ausgeben`, `#einlesen`, `#instanz` |
| „Umschlag-Editor (Runde 27)“ | [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#umschlag` |
| „Runde 19 – RAG-Pflege, Reste der Stellen“ | [`bibel-quellen.md`](../site/content/de/hilfe/bibel-quellen.md) `#bedeutungssuche`, `#zitate-pruefen`, `#nachschlagen`; [`einstellungen.md`](../site/content/de/hilfe/einstellungen.md) `#verbindungen` |
| „EPUB (Runde 20)“ | [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#epub` |
| „Runde 31 – Register und Glossar“ | [`suchen.md`](../site/content/de/hilfe/suchen.md) `#glossar`, `#register` |
| „Runde 22 – Revisionen und Spiegel“ | [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#fassungen`, `#verlauf`; follow-up question: [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#rueckfrage`; archive: [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#loeschen-archiv`; draft: [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#pdf`; exchange: [`austausch-sicherung.md`](../site/content/de/hilfe/austausch-sicherung.md) `#einlesen` |
| „Runde 23 – Härtung“ | distributed by content: [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#einfuegen`, `#fassungen`; [`gliederung.md`](../site/content/de/hilfe/gliederung.md) `#antraege`; [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#chat`, `#befunde`, `#websuche`; [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#umschlag`, `#epub`; [`einstellungen.md`](../site/content/de/hilfe/einstellungen.md) `#verbindungen`, `#mcp-zugang`; [`austausch-sicherung.md`](../site/content/de/hilfe/austausch-sicherung.md) `#sicherung`, `#datenordner` |
| „Runde 32 – Word und Beenden“ | [`einlesen.md`](../site/content/de/hilfe/einlesen.md) `#word`; [`buchsatz.md`](../site/content/de/hilfe/buchsatz.md) `#word-export`; [`erste-schritte.md`](../site/content/de/hilfe/erste-schritte.md) `#beenden` |
| „Runde 36 – Reste“ | distributed by content: [`konten.md`](../site/content/de/hilfe/konten.md) `#lektor-rechte`, `#passwort`, `#anmelden`; [`einfaelle.md`](../site/content/de/hilfe/einfaelle.md) `#mindmap-austausch`; [`schreiben.md`](../site/content/de/hilfe/schreiben.md) `#ausblenden`, `#einfuegen`; [`assistenten.md`](../site/content/de/hilfe/assistenten.md) `#rueckfrage`, `#pruefen`; [`menueband.md`](../site/content/de/hilfe/menueband.md) `#laptop-modus`; [`austausch-sicherung.md`](../site/content/de/hilfe/austausch-sicherung.md) `#sicherung`, `#datenordner`; [`gliederung.md`](../site/content/de/hilfe/gliederung.md) `#gliederung` |

All keyboard shortcuts and mouse gestures are additionally collected in
[`tasten.md`](../site/content/de/hilfe/tasten.md); the entry point is [`_index.md`](../site/content/de/hilfe/_index.md).

## Technical notes

Per help page, what is deliberately missing there. Interfaces in detail: `docs/RPC.md`, `docs/MCP.md`; structure:
`docs/Architektur.md`; operation: `docs/Betrieb.md`; distribution and launchers: `docs/Verteilung.md`.

### „Erste Schritte“ (`erste-schritte.md`)

- **Invocation:**

  ```
  baruch-server [serve] [--listen 127.0.0.1:8765] [--data ORDNER] [--secrets keyring|memory] [--allow-host NAME]…
  baruch-server --version
  baruch-server import DATEI.typ [--title "TITEL"] [--data ORDNER]
  baruch-server export ID|SLUG [--data ORDNER]
  ```

  The command (`serve`, `import`, `export`) stands before the options. `--version` prints `baruch-server <Version>`.
  `--listen` also as `BARUCH_LISTEN` (default `127.0.0.1:8765`, the same port as with the user service from
  `packaging/systemd/`), `--data` also as `BARUCH_DATA`. `--allow-host NAME` (repeatable, also
  `BARUCH_ALLOWED_HOSTS`, comma-separated) additionally allows a host name for operation behind a reverse proxy
  (`docs/Betrieb.md`).
- **Data folder:** `ORDNER/baruch.sqlite`, Git mirror of every work in `ORDNER/works/<slug>/`. Default without `--data`:
  Linux `~/.local/share/baruch` (or `$XDG_DATA_HOME/baruch`), macOS `~/Library/Application Support/baruch`,
  Windows `C:\Users\<name>\AppData\Local\baruch` – deliberately **Local**, not Roaming: server-stored profiles of a
  domain would otherwise copy the SQLite database (including the WAL) and the Git mirror back and forth at every login and logout.
  If Baruch creates the data folder itself, it gets the permissions 0700 under Linux and macOS; Baruch does not change
  an existing folder (whoever puts `--data` outside their own home checks the permissions themselves – database and
  mirror contain the whole manuscript).
- **Secrets** of the connections (key, token, password) lie in the keyring of the system (service `baruch`) only with the
  default data folder. With any other `--data` they stay in the memory of the process and
  are gone at the end – so a throwaway or test server can neither read nor overwrite or delete the entries `baruch/…` of the
  actual service. `--secrets keyring` (`BARUCH_SECRETS=keyring`) explicitly takes the
  keyring, `--secrets memory` the memory; the start output names what holds („Geheimnisse: …“).
- **Sample work:** a really new database gets „Beispielwerk: Leise Abhängigkeit (Auszug)“; an
  existing data folder is not changed. The help and its images show the invented work
  „Ein Jahr auf Mövenoog“ instead (image list `site/data/hilfebilder.toml`).
- **Choice of work:** address `/?work=<id>`; cookie `baruch_work` (one year). `/` without `?work=` opens the remembered
  work, otherwise the oldest, otherwise the list of works. Links and forms carry the work along.
- **Pages:** list of works `/werke`; create/import `/werke/neu` (with JavaScript the dialog „Importieren…“);
  change `/werke/<id>/bearbeiten`; delete `/werke/<id>/loeschen`; archive `/werke/archiv`; export
  `/werke/<id>/export` (downloads `<slug>.typ`, like `baruch-server export`); „Über Baruch“ `/ueber`, licence text
  `/ueber/lizenz` (plain text).
- **New work:** the work template of a new book contains `#set document(…)` and
  `#set heading(numbering: "1.1")`; commit „Werk angelegt: <Titel>“. A sermon text from SWORD stands as
  `#quote(block: true, attribution: [Joh 3,16 (Modul)])[…]`; missing verses are reported by `sermon_text.missing`; an
  invalid reference answers with status 422 without JavaScript. On `/werke/neu` with an empty Bible the default work holds,
  in the dialog the shown work.
- **Unique titles:** message `Es gibt schon ein Werk „…“ (slug).` If two works had the same
  title before the update, one of them gets a new one at the first start: the seeded sample work its sample title again, otherwise the
  younger one „<Titel> (2)“. Folder names, mirrors and texts stay.
- **Changing:** commit „Werk geändert: <Felder>“, when a file changes (e.g. `vorlage.typ`). `#set`, `#show`, `#let` …
  belong in the work template.
- **Deleting:** last commit „Werk gelöscht: <Titel>“, the mirror moves to
  `ORDNER/works/.geloescht/<slug>-<JJJJ-MM-TT>/`. The folder name (`slug`) stays permanently reserved (new work:
  `<slug>-2`); work numbers are never assigned again. The ideas stand immediately in the mirror of the ideas without a
  work (`<data>/einfaelle/einfaelle.md`); at start Baruch brings this mirror up to date (no commit without a change).
- **Restoring from the archive:** the folder moves back to `works/<slug>`, commit „Werk wiederhergestellt“.
  It does not exist through MCP.
- **Address:** `/?work=<id>&key=<schlüssel>&rev=<nummer>[&view=preview][&pick=…][&lq=<suche>]`. `key` is the fixed
  key of the section (the MCP field `url` names the same address). Older addresses with `node=<stelle>` keep
  working; if the key is missing, Baruch shows the section of the position `node` or the default section and
  „Abschnitt mit Schlüssel K gibt es nicht (mehr).“ `rev` is the number of the revision (with gaps after deleting),
  not its position. `pick=<abschnitt>:<nummer>[,…]` tentatively puts other revisions into the preview;
  invalid entries are skipped. Without `work` the default work (the oldest), an unknown work 404.
  With `?lq=` the box „Bilder des Werks“ stands below the hits, also without JavaScript.
- **Times:** local time since the user decision of 2026-11-07 (before that UTC); stored and delivered through RPC/MCP
  is still UTC.
- **„Über Baruch“:** licence `AGPL-3.0-or-later`; source at the tag `v<Version>`; website
  `https://baruch.bernau.family/`; `THIRD-PARTY-LICENSES.txt` per package (if the file lies next to the program, its
  path stands there). A program built from the source does not have the file; the page names the command
  (`scripts/third-party-licenses.py`). In the AppImage the path lies in the mounted image and only holds as long as
  Baruch runs. Only through the APT repository (Debian, Ubuntu) do new versions come with `sudo apt upgrade`.
- **Quitting:**

  | Route | Purpose |
  |---|---|
  | `GET /beenden` | confirmation with a form; not from the own machine 403 with an explanation |
  | `POST /beenden` | behind `guard::same_origin`; only from the own machine (also not under an `--allow-host` name), otherwise 403; response 200 with the final page, 500 ms later Baruch shuts down |

  With JavaScript the final page calls `window.close()`; browsers allow that only for windows that a script
  or the launcher opened as an app window. The launchers (`baruch`, AppImage, macOS program,
  Windows start menu) therefore open Baruch with a Chromium-like browser as an `--app` window;
  `BARUCH_BROWSER=standard` always takes the default browser, `BARUCH_NO_BROWSER=1` opens none (state of verification
  per platform: `docs/Verteilung.md`, "Quitting Baruch"). As a service (`systemd --user`, `Restart=on-failure`)
  Baruch stays off after quitting. `systemctl --user stop`, `kill` (SIGTERM) and under Windows Ctrl+Break or
  closing the console window shut down just as cleanly; last line „Baruch beendet.“, status 0. Baruch cannot be quit through RPC
  and MCP.

### „Vorhandene Texte einlesen“ (`einlesen.md`)

- **Typst import, rules:** `=` chapter, `==` section, `===` subsection; the text including the heading line becomes
  revision 1. Title and author from `#set document(title: …, author: …)`; commented-out `#set document(…)` (`//`,
  `/* … */`) do not count. Title taken: exit 1 with `Es gibt schon ein Werk „…“ (slug). Mit --title "…" einen
  anderen Titel wählen.` Folder name (`slug`) from the title, at most 40 characters, cut at the last word boundary
  („Mövenoog – ein ganzes Jahr auf der kleinen Insel“ → `moevenoog-ein-ganzes-jahr-auf-der`), on equality `-2`,
  `-3` …; existing works keep their folder name.
- **Mirror after the import:** commit „Import aus <datei>“ with `vorlage.typ`, `main.typ` (work template + `#include`
  of all sections) and `abschnitte/NNN-titel.typ`, each with the first line `#import "../vorlage.typ": *` (only in the
  mirror, not in the revision; so own functions such as `#szene` hold there too). Works without a template have
  neither `vorlage.typ` nor the line. Output: work, folder name, number of sections, mirror folder, commit.
- **Refused:** the same file a second time (also renamed or with CRLF; stderr, exit 1); a FIFO, a
  device or a folder („… ist keine Datei“, without waiting); not UTF-8; over 16 MiB. A BOM stays in the
  work template, the first heading still becomes a chapter.
- **No outline** are heading lines in raw blocks, in multi-line inline raw, in block comments, in
  multi-line strings in code (`#let x = "…"`) and in multi-line content blocks from code (`#block[…]`,
  `#figure(caption: […])`, `#let x = […]`, `#show …: it => […]`). A comment behind a heading
  (`== Titel // Notiz`) does not belong to the title. Known limit: indented headings, `====` and deeper and
  `#heading(...)` are not recognised – except the markers `<baruch-part>` and `<baruch-unnumbered>` (exactly in
  the documented form; `// …` may stand behind them; other spacing, another label or an indentation make
  them text or work template). Another `#heading(...)` before the first heading stays work template.
  Existing works are not rewritten for the markers.
- **Export on the command line:** `baruch-server export <id|slug>` writes the work template + current texts to
  stdout (without the `#import` lines of the mirror); right after the import byte for byte the input.
  `--out ORDNER` writes the Typst file (under its import name, otherwise `<slug>.typ`) and every image to its
  path (without `./` and a leading `/`), byte-identical. Before the first file the export checks: an existing target file,
  an intermediate folder as a link or file, two image paths at the same time file and folder (`a.png` and `a.png/b.svg`)
  → exit 1 with a reason, nothing written.
- **Images on import (CLI):** what the template and text name with `image("…")` is read (also in
  `#figure(image("…"))`), relative to the folder of the file; `./bilder/a.png`, `bilder//a.png` and `/bilder/a.png` are
  the same image `bilder/a.png` (the path in the text stays as it is written). Skipped: outside the
  folder (through `..` or a symlink), paths with `..`, missing, over 8 MiB, not an image – one line each „Bild
  übersprungen: <pfad> (<grund>)“ on stderr; output „Bilder: n“; together at most 64 MiB per file.
  Uploading a single file (`/werke/import`) and `work.import` bring only the text.
- **Export → import, hint codes** (CLI: `Hinweis (<code>): …` on stderr): `import-typeset-adopted` (the block
  `// Baruch: Satz` becomes a typesetting setting), `import-typeset-kept` (changed by hand, stays work template; the
  binding offset comes back as part of the inner margin), show rules `// Baruch: Überschriften ohne Nummer` → flag
  „Nummer aus“ (also for a heading with its own label and under „Ohne Nummern“), `import-back-matter-cut` (from
  `// Baruch: Register und Glossar`, only when the generated block stands behind it, with line),
  `import-back-matter-kept`, `import-projection-removed` (without a glossary page the first mention carries the note
  `/*baruch-erste-erwaehnung*/#emph[Wort]`).
- **Markdown → Typst:** `#`/`##`/`###` → `=`/`==`/`===`, `####`–`######` limited to `===`; YAML head → `#set
  document(…)`; `#emph[…]`, `#strong[…]`, `#strike[…]`; `#quote(block: true)[…]`; `#raw("…")`; `#link("…")[Text]`;
  `#footnote[…]`; `#image("…", alt: "…")` or `#figure(…, caption: […])`; `#table(…)`; `/ Begriff: Erklärung`;
  `<br>` becomes a break, HTML blocks stand as a comment. Literal stay `#`, `*`, `_`, `$`, `@`, `<`, `[`, `]`,
  `` ` ``, `\`, `~` and `=`/`-`/`+`/`/` at the line start. Files that the main file does not name: `archive-unused`.
  Refused archives: links, `..`, absolute paths, duplicate names, zip bombs. CLI output:
  `Hinweis (<code>, Zeile n): …`.
- **EPUB on the command line:** `baruch-server export <id|slug> --epub DATEI [--data ORDNER]`; an existing file
  exit 1; `--epub` together with `--out`, `--pdf`, `--stichwort` or `--baruch` exit 2.
- **PDF/EPUB:** page `/werke/import/quelle`; locally `pdftotext`/`pdfinfo` (package `poppler-utils`); EPUB footnotes
  become `#footnote[…]`, the number at the start is dropped only as the number of the footnote mark (hint „Fußnotennummer
  entfernt“; „1 Kor 13,4“ stays). According to its tool description MultiGPT accepts no EPUB (an assumption from the
  description, not checked against a real service). Drafts in `import-drafts/`. Command line with
  `--data DIR`; without `--uebernehmen` and `--bibliothek` it writes nothing, a presumed part becomes a
  chapter there; both switches with `.typ`/`.md`/`.zip` → exit 2.
- **Word:** page `/werke/import/word`; proposals under `@word:<name>` (model „word“); name: lower case, ä→ae, ö→oe,
  ü→ue, ß→ss, everything else a hyphen, at most 40 characters, without a name `@word:unbekannt`. Headings from the
  styles „Überschrift 1–3“/„Heading 1–3“ (also derived styles and the outline level of the paragraph);
  tables as a Typst table (an island in the typeset view); images PNG, JPEG, GIF, WebP, SVG. From a file written by Baruch
  `@lektor` becomes `@word:lektor`; Typst islands come back literally (test
  `round32_abnahme`). Hint codes:

  | Code | Meaning |
  |---|---|
  | `docx_ueberschrift_tief` | heading 4–6 stands bold in the section |
  | `docx_text_vor_ueberschrift` | text before the first heading in a first chapter of its own |
  | `docx_ueberschrift_eingefuegt`, `docx_ueberschrift_geloescht` | heading inserted or deleted by the editor |
  | `docx_vorspann_eingefuegt` | inserted text before the first heading – not taken over |
  | `docx_formataenderung` | tracked format changes: the final state holds |
  | `docx_verschoben` | moved text as deletion and insertion |
  | `docx_textfeld`, `docx_objekt` | text box, drawing, embedded object – not taken over |
  | `docx_formel` | formula only as plain text |
  | `docx_tabelle_vereinfacht` | merged cells, rows of unequal length, inserted/deleted table rows |
  | `docx_bild_format`, `docx_bild_extern`, `docx_bild_uebersprungen` | format not supported (EMF/WMF), only linked, too large |
  | `docx_endnote`, `docx_fussnote_absaetze` | endnotes as footnotes; several paragraphs of a footnote as lines |
  | `docx_verborgener_text`, `docx_symbol`, `docx_unbekannt` | hidden text, symbol fonts, unknown element |
  | `docx_kommentar_ohne_stelle` | comment at the first section |
  | `docx_gliederung_abweichend`, `docx_vorschlag_abgelehnt`, `docx_beitrag_abgelehnt` | a proposal or post could not be created (the work exists anyway) |

  Command line: `baruch-server import DATEI.docx [--uebernehmen] [--title "TITEL"] [--data ORDNER]`; an unreadable
  file exit 1.
- There is no import through MCP (no Typst, PDF, EPUB, Word).

### „Menüband und Laptop-Modus“ (`menueband.md`)

- The ribbon replaces the former top bar with the menus „Werk“, „Ansicht“ and „Werkzeuge“ and the former
  toolbar above the sheet; no entry has been dropped. Renamed: „Satz“ → „Satz-Vorschau“, „Als
  PDF/Entwurf/EPUB herunterladen“ → „PDF“, „Entwurf“, „EPUB“; „Editor“ outside the work page → „Zum Text“.
- **Addresses:** tab `?tab=start|einfuegen|pruefen|buch|ansicht`, ribbon `?band=…` (cookie `baruch_band`),
  laptop mode `?laptop=on|off|auto` (cookie `baruch_laptop`, one year), rails `?lrail=1`, `?rrail=1` (cookies
  `baruch_lrail`, `baruch_rrail`); drawers without JavaScript through the jump targets `#pane-tree` and
  `#pane-assistants`. Without JavaScript menu and groups are `<details>`.
- **Targets:** new work `/werke/neu`; settings without JavaScript `/einstellungen/darstellung`; search `/suche`;
  cross-references `/verweise`; literature `/literatur` (on the work page the popup); reconciliation `/abgleich`;
  „Literatur“ as the back group on `/quelle`. The typeset page carries in the tab „Buch“ the groups „Zurück“,
  „Herunterladen“, „Seiten“ („‹ Seite n von N ›“) and „Satzlauf“ („Satz abbrechen“).
- **Badges** stand in the delivered HTML of every page; the script only refreshes them.
- **Zoom:** steps in the cookie `baruch_zoom`; Ctrl+mouse wheel evaluates only `ctrlKey` (so on the Mac Ctrl, not ⌘),
  Ctrl+0 also `metaKey`.
- **Help entry:** an ordinary link to `about::HELP_URL` (`https://baruch.bernau.family/hilfe/`), new
  window, without `nur-autor` (Lektoren see it too); no route, no entry in `roles`.
- **K** and **F** are called „Hervorheben“ and „Fett“ for screen readers. In the selection list „Absatzformat“ the
  arrow keys belong to the list.
- Formerly Baruch put the three columns below each other under 900 px; that has been dropped.

### „Gliederung“ (`gliederung.md`)

- **Notation in the text:** part = `#heading(level: 1, numbering: none)[Titel] <baruch-part>`, chapter `= `,
  section `== `, subsection `=== `; an item without a number can stand as
  `#heading(level: N, numbering: none)[Titel] <baruch-unnumbered>`. If level or title change, the
  section gets a new revision from the human that differs only in the heading line (label `<…>`
  and comment `// …` stay). Every outline change is exactly one commit („Gliederung: …“).
- **Assistants:** create through MCP `outline_add` (commit with their name; part with `level: 0`); renaming,
  moving, changing the level, number on/off, deleting only as a request. In a sermon the request says „n vorhandene
  Predigtteile wandern unter den neuen Teil“. If an assistant puts a part before a chapter in the middle of a part
  (without taking over existing chapters this counts as creating), the following chapters up to the next part stand
  under the new part.
- **Markdown headings** (since 2026-11-07): 1–6 hashes and a space at the line start. Not converted:
  `\# …`, a hash in quotation marks (`'# …'`, `"# …"`), Typst code (`#heading(…)`, `#set`, `#let`), indented
  lines, lines in raw blocks, comments and content blocks, the work template. Proposals of the assistants stay
  as they are; on acceptance the same rule holds (the proposal stays in the revision tree, followed by the
  converted revision). „Fassung ändern“ converts like „Speichern“.
- **Title rules, messages:** „Ein Gliederungspunkt braucht einen Titel.“, „Der Titel darf keinen Zeilenumbruch
  enthalten.“, „Der Titel ist zu lang (höchstens 200 Zeichen, hier N).“ Only a changed title is checked
  (also with `revision.save`, switching, accepting, every heading that would split the section); an item
  `== ` without a title stays. Import and export stay byte-identical.
- **Deleting:** the commit names the text of the current revision and what else gets lost – every other revision
  (open proposals counted separately) and every post in its wording.
- **Without JavaScript:** outline manager `/gliederung`, editing `/gliederung/bearbeiten`; with JavaScript
  the fallback links are hidden. The confirmation for splitting/merging is a page of its own without JavaScript
  with „Zurück“.
- **Parts, details:** renaming rewrites the marker line (a comment behind it stays; a part with
  empty text gets the marker). A part without a marker (self-written text) keeps its text;
  only the title changes, the message says so. Another title in the marker renames the part, also
  when switching to an older revision; a revision without a marker switches without outline reconciliation.
- **Older parts (user decision 2026-11-07):** at the first start Baruch once converts all parts whose
  current revision is empty or consists only of the old line „= Teil …“ (exactly one `=` line without label and
  comment, title that of the part or beginning with „Teil“) to the marker: a new current revision of the
  human with the note „Umwandlung Runde 14 (Nutzerentscheidung)“, one commit per work „Umwandlung Runde 14
  (Nutzerentscheidung): n Teile …“; the old revision stays switchable. Parts with other text stay; stderr
  names the count and the node ids. Flag `parts_marked_r14` in `store_meta`; a new database sets it right away
  (its sample work keeps its empty parts).
- **Without a number:** commit `Gliederung: „…“ ohne Nummer` (`Baruch-Action: outline-numbered`), RPC
  `outline.set_numbered`. If one changes the level through the outline, the marker stays (only `level:` changes).
  In export, mirror and typesetting an item with „Nummer aus“ stands as
  `#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>` – in all schemes with numbers; the
  stored text stays (`= Vorwort`). A heading with its own label (`= Vorwort <vorwort>`) instead gets
  `#show heading.where(…): set heading(numbering: none)` behind the template. With „Ohne Nummern“ everything stays
  as it is stored.
- **Requests, commits:** confirming = one commit with the human as author and committer, trailers `Baruch-Origin`,
  `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`, subject „… (Antrag N)“. Rejected or stale
  (user decision 2026-11-08, every kind of request): an empty commit „Gliederung: Antrag N abgelehnt“ or
  „… veraltet“, trailers `Baruch-Decision: rejected|stale`, `Baruch-Requested-By`, `Baruch-Origin`,
  `Baruch-Request`, `Baruch-Decided-By`, in the body requester, summary, justification, reason. The same whether
  through form, approval dialog or RPC. Approval of a part: revision 1 of the part carries the assistant.
  The notice bar links to `/gliederung#antraege`; preview `/gliederung/antraege/ID/vorschau`. Sermons name
  „Predigtteil II.“ in the preview.
- **Switching with another level:** one commit („… auf Fassung n umgeschaltet · Ebene 1 → 2“, in the body the
  items relevelled along); every sub-item relevelled along gets a new revision from the human. Refused: a revision
  without a heading, with further headings, with text before the heading, with a level that is not possible at that
  place (e.g. the first item as a section). „Fassung ändern“ and `revision.save` of the current revision
  require exactly one heading of the level of the item; a non-current revision can be changed freely.
  Assistants cannot create proposals with another level.
- **Escapes** in headings (`\'`, `\#`, `\*` …) are shown resolved by the outline („Anna's Laube“);
  file names in the mirror follow the resolved title and the position. The export (`work.export`) is valid Typst in a new order
  after rearrangements, byte-identical to the import file without a change.
- **Outline scheme:** page `/werke/<id>/nummerierung`; preview through GET; commit `Gliederung:
  Gliederungsschema „Juristisch“`; an invalid pattern 422, an unchanged scheme 409. In the export the template is followed by
  a line `// Baruch: Gliederungsschema „…“` and a `#set heading(numbering: …)` rule (with „Aus der
  Werkvorlage“ nothing); checked for all thirteen schemes. The stored template (`vorlage.typ`) stays. In the
  head of the outline: „I|1“ → „Nur Kapitel römisch“, „1.1“ → „Dezimal (DIN 1421)“, otherwise the pattern, the raw
  pattern in the tooltip; values `vorlage`, `keine`.
- **Pattern language:** a pattern with exactly one counter symbol shows only the own number of the level, one with several
  counts hierarchically; levels beyond the list have no number; `;` and `|` are reserved.
- **Numbering from the work template:** the last `#set heading(numbering: …)` rule of the top level counts.
  A text pattern holds literally as in Typst (`"1;"` yields „2;“, `aa` are two counter symbols). A function is
  computed per level if it consists only of conditions on the depth (`n.pos().len()` with comparisons, also through
  `let p = n.pos()`), `if`/`else`, text, `numbering("…", ..n)`, `numbering("…", p.at(i))` (`p.first()`,
  `p.last()`), `str(p.at(i))`, `* 2` and `+` – directly or through a `#let` function. Example:
  `(..n) => if n.pos().len() == 1 { "Kapitel " + str(n.pos().at(0)) } else { numbering("1.1", ..n) }` yields
  „Kapitel 3“ · „3.2“ · „3.2.1“. The rule that Baruch exports for „Teile und Kapitel“, „Kapitel ausgeschrieben“,
  „Juristisch“, „Predigt“, „Buch“ becomes this scheme again on reading in (afterwards „Aus
  der Werkvorlage“ counts as the scheme). What cannot be computed: the first `numbering("…")` of the function for all levels.
- **Mentions in the text** („im dreizehnten Kapitel“, „Kapitel XLII“) are recognised by Baruch up to the hundredth
  ordinal number and for Roman numerals in the usual notation, only when exactly one item can be meant.

### „Schreiben, Bilder und Fassungen“ (`schreiben.md`)

- **Typeset view:** the source stays Typst; before saving the typeset view is translated into Typst, afterwards the
  rules of the source text hold. The server delivers the typeset view ready-made (no detour through the source text); sections over
  64 KiB come without a ready-made typeset view. Remembered view: cookie `baruch_editor`. If one arrives through a hit of the
  literature search (`#fundstelle`), the editor starts in the source text; with the typeset view open, the find spot is highlighted
  yellow there (does not change the text, disappears when typing). Description of blocks, islands and RPC:
  `docs/WYSIWYG.md`.
- **Heading in the typeset view:** Backspace at the start of a paragraph directly behind a heading and Delete at the
  end of a heading do not pull the paragraph into the heading (an empty paragraph is dropped). The typeset view
  checks the title rules with the same message as the server. The typeset view does not offer parts (the server refuses
  a new part from the typeset view); for parts and items without a number no paragraph format can be chosen,
  the marker is saved again (a comment behind it stays).
- **Blocks:** `#szene[…]`, `#arbeit[…]` as a box; arguments (`#szene(ort: […])`) and a label `<name>` on the
  next line stand small next to it and are preserved. `#image("…")` and `#figure(image("…"), caption: […])`
  appear as an image as soon as the image is created under this path (also with `./`); a block with several
  `image(…)` shows all of them. `1. …` numbered list, `/ Begriff: Erklärung` term list, `\` forced break
  (↵), `#link("…")[Text]`, `@ziel` as a button (a click shows the label; change only in the source text). Islands:
  `#let`, `#set`, `#show`, own functions, formulas, raw text, comments, lists with mixed markers;
  leaving the island field after a change also takes it over. Bible reference as an island `#bibelstelle[…]` (only
  if the template defines `#let bibelstelle(…)` or `#let bibel(…)`, with the name of the template),
  literature reference `@schlüssel` (only with `#bibliography(…)`); scene only with `#let szene`, work assignment only with
  `#let arbeit`; term `/ Begriff: …`, break `\` at the line end. In the source text blocks begin at a
  line start.
- **Quotation button:** until Round 35 it inserted a line „– “ which, unfilled, stood as a single dash in the book;
  since Round 36 it is dropped. A reference directly below a quotation block is shown by the
  typeset view as quotation and paragraph (no code island).
- **Saving:** since 2026-11-27 normal saving without a size and time limit (before: a new revision after
  30 minutes of pause or from 20 % change). The dialog „Seite speichern“ of the browser does not appear on the
  work page. A switched-on time limit counts from the last write of the revision: the first small
  correction to an older section that was never amended yields a new revision („Zeitgrenze
  überschritten“). The editor sends the loaded state along; with a different state a new revision arises („nicht
  auf der aktuellen Fassung bearbeitet“). Every amendment is a commit. Feedback with JavaScript as a
  hint at the bottom, without it in the editor head. All other writing paths (inserting an image, index mark, reference, replacing,
  outline) create new revisions. If saving without JavaScript is refused (e.g. invalid heading level),
  Baruch shows the message and the text in a text field („Erneut speichern“). The confirmation „Teilen/Zusammenführen und
  speichern“ keeps the save mode; the head afterwards names the revision and the reason („Überschrift geändert“).
- **„Einstellungen → Werk → Fassungen“** (anchor `#fassungen`): amending on/off, percent 1–100 (default 20),
  minutes 0–1440 (0 = no time limit, default 30); only in the database (not in the exchange format), every
  change a log commit.
- **History:** `/werke/<id>/fassungen/verlauf`; every undo is a commit. „Fassung ändern“ on the current
  or an amended revision is a step in the history; if it has meanwhile been amended in another tab,
  Baruch refuses („Veraltet: …“, the form to `/act` passes `based_on_amend` through).
- **Further revisions:** link `?rev=`, „aktuell machen“ without JavaScript as a form to `/act`.
- **Revisions:** a new revision has the shown revision as its parent. The server itself checks deleting (without the checkbox
  „Löschen bitte bestätigen.“); children hang on the parent afterwards; unknown number: „Fassung 3 gibt es nicht;
  vorhanden: 1, 2, 4“. „In der Vorschau probieren“ = `pick` in the address. Every acceptance, switch,
  save, change and deletion is exactly one commit (`docs/Architektur.md`); changing and deleting write
  before/after or the deleted text into the commit message. If the mirror fails, the change stays
  stored; the next commit catches up.
- **Hiding:** address `&fassungen=alle`; the mark holds per section, stands only in the database (not in the
  exchange format), no commit; the action exists only at the current revision and below it; after switching to
  an older revision a higher mark holds only up to below the new current one. Work-wide: mark on the last
  revision before the day (at most the current one), replaces existing marks. Not possible through MCP. The counter in the
  ribbon shows the same as the one in the editor head.
- **Proposals in the reading flow:** lines at least 3 : 1 contrast (WCAG 1.4.11); on the sheet, which stays light even in the dark
  theme (#f4f1ea), the light line holds. Collapsing: cookie `baruch_folded`; without JavaScript the
  browser collapses by itself (not across reloads). Authors without a persona entry: `@mcp:<name>`. A rephrased
  paragraph next to a new one appears as two pieces („geändert“, „eingefügt“).
- **Images:** page `/werke/<id>/bilder`, search `?q=…` (case-insensitive, `%` and `_` literal), preview
  `…/vorschau?b=320` (loads when scrolled into view), one image `/werke/<id>/bilder/<name>[?fassung=n]` with SHA-256, bytes
  `/werke/<id>/bilder/<name>/datei[?fassung=n]`, by path `/werke/<id>/bilder/pfad?p=<Pfad>` (every spelling,
  never `..`), requests `#antraege`, proposed content `/werke/<id>/bilder/antraege/<nr>/datei` (removed 30 days after
  the decision, then HTTP 410). Empty name = from the file name (lower case, invalid characters `-`, on
  collision `-2` before the extension); „SVG als Base64 speichern“ („(Base64)“ in the list). Mirror: `bilder/<name>`
  or `bilder/<name>.b64` (76 characters per line), commit „Bild <name>: …“. Adjusting the extension: `haus.svg` →
  `haus.png` (taken: `haus-2.png`), paths stay, in the EPUB the file is called e.g. `haus.svg.png`. Path collision:
  commit „Pfad-Kollision … aufgelöst“. Limits for assistants per work in 24 hours: 30 creations/replacement requests,
  together 64 MiB, at most 10 open replacement requests.
- **Inserting an image:** `#figure(image("<Pfad>", alt: "…"), caption: [...])` is inserted as the human (one commit;
  the text of the caption is escaped). Without a path the image gets `bilder/<name>` together with the revision.
  Cursor position in the source text: the next line boundary, never into a multi-line construct (`#footnote[…]`,
  `#link(…)[…]`). Chapter image: recognised from a literal list such as `#let kapitelbilder = ("bilder/a.png", …)` and
  `image(kapitelbilder.at(counter(heading).get().first() - 1))`, when chapters are the first level; otherwise
  „Kapitelbild fehlt: <Pfad>“ or „Bild aus der Vorlage“.
- **Time keeper:** seconds = words × 60 ÷ words per minute, rounded up; total time = sum of the items. Only in
  the database, no commit. RPC `work.timing`, `work.timing_set`, `timing.count`; MCP `work_timing` (read only).
  Counting rules (`baruch_typst::timing::spoken_words`): (1) CRLF → LF; (2) comments `/* … */` and `//` at the
  line start or after whitespace do not count; (3) heading lines and part markers do not; (4) raw text
  and maths `$…$` do not; (5) `#name`/`#name.feld` including arguments are dropped, following content blocks `[…]` are
  spoken – except `#footnote` and `#arbeit`; lines with `#set`, `#show`, `#let`, `#import`, `#include` are dropped
  entirely; (6) labels `<…>` and references `@name` are dropped; (7) `*`, `_` are dropped, `~` becomes a space,
  list markers at the line start are dropped, `\x` becomes `x`; (8) a word is a sequence without whitespace with
  at least one letter or digit. Example: `#quote(block: true)[Im Anfang war das Wort.] <zitat>`
  = 5 words = 3 seconds at 110.

### „Assistenten, Chat und Befunde“ (`assistenten.md`)

- **Terms:** in code, RPC and MCP the assistants are called "persona", the short name "handle". Baruch calls the model
  of the persona through the model backend Eliezer („Einstellungen → Verbindungen“).
- **Jobs:** forms without JavaScript to `/assistenten/act`; errors show a notice page (instead of an
  error page) with „Zurück zum Abschnitt“, in the chat per persona who did not answer; with „komprimiert“ the
  hint and „Weiter zum Abschnitt“. With JavaScript the floating notice (toast) at the bottom right, which stays.
  With Gemma a call takes about 25 s. If Baruch requests anew, what was shown so far disappears („Antwort
  wird neu angefordert (…)“).
- **Length of the answers:** at most 700 (question), 900 (review), 500 (chat) tokens, for a proposal by the length of the
  section (at most 4096); `BARUCH_MAX_TOKENS` sets a smaller upper limit. Cut off: „… (Antwort
  abgeschnitten: Längengrenze erreicht)“.
- **Queue:** Eliezer works on one job at a time (otherwise it loads a model twice; the graphics card
  of the development machine has 22 GB), eight wait. The context length of the persona must match the one with which
  Eliezer loads the model (Gemma: 32768); Baruch requests none itself.
- **Lector:** for the trial since 2026-11-06 fixed `@lektor` (all personas on one model); the badge says
  in its title „fest gewählt“ or „nach größtem Kontext“. Memory: ideas about the section and up to ten open ones
  of the work without a reference. Findings of the others: the last three per section.
- **Prompt rules:** forbidden openings „Als Assistent“, „Als KI“, „Als Sprachmodell“, „Gerne“, „Natürlich“
  (if the second answer also begins that way, its first sentence is dropped); Bible verses in their wording only from `@schrift`;
  the listener reads in the first-person perspective of the target group. In a proposal Baruch removes lines that begin with „Kritisch:“,
  „Was trägt:“, „Vorschlag:“, „Jetzt wichtig:“, „Kann warten:“, a code block around the text, and checks
  no forbidden openings there. The note „Komprimiert“ stands in the note of a proposal, never in the book text;
  note for a chapter „Vorschlag von @lektor (ganzes Kapitel)“, the head names „beruht auf Fassung n“.
- **Lector default:** demands „Was trägt:“ only for a review; in the chat the lector first responds to the message
  (rule 8 of the prompt); an own role prompt stays untouched (pay attention to the tone during the manual check).
  „Chat nur Abschnitt“ holds for all works and stands only in the database; the persona then gets the section,
  title and outline (only numbers and titles). Review and questions in the area „Assistenten“ still see the chapter.
- **Work page without JavaScript:** in the outline the arrows and „Bearbeiten“ stand in a row of their own
  below the entry; in a narrow column the badge „k offen“ slips below the title. The list „Zitate prüfen“
  hangs on the whole status line.
- **„Kritisch:“ detection** (only Nathan as auditor is still mandatory): a line that begins with „Kritisch:“ after list markers,
  or „Kritisch:“ directly behind the reference of the part („- III. Zuspruch: Kritisch: …“, bold, with the addition
  „(Schluss)“ or only „II. Kritisch: …“); „Unkritisch:“ or „… ist kritisch: …“ do not count. If it is missing after
  one follow-up request, a hint from Baruch stands below the post. Solution in a fixed form:
  `- statt: „Zitat“`, `- besser: „…“` or `- Schritt: …`; progress „fehlender Lösungsvorschlag“; for a
  cut-off answer no follow-up request.
- **Review job recognised** by a word that begins with „prüf“, „überprüf“, „nachprüf“, „gegenprüf“, „kontrollier“,
  „check“ or „review“, or by a finding number („[B12]“). Only own, open findings; foreign,
  unknown or decided numbers Baruch skips; no double follow-up question. If the persona sees everything
  implemented, it says „Alle meine Befunde zu diesem Abschnitt scheinen erledigt.“ (then no mandatory finding).
  „Ja, erledigt“ sets „erledigt ohne Änderung“ (reason „laut @persona umgesetzt …“); both one commit.
  Adopted findings show „übernommen in Fassung 3 · Stand 2“.
- **Critique cycle:** also through MCP `ask_persona` with `rev`. If the current revision shows only the heading and
  a proposal with text exists, all review the proposal (hint „aktuelle Fassung enthält nur die
  Überschrift – Vorschlag n prüfen?“ on an explicit choice). Only the lector comments on foreign findings; the
  others see them as „schon gemeldet“.
- **Chat:** new mark in the LocalStorage `baruch_chat_seen` (own posts never new; first visit: foreign posts
  since the last visit of any section of the work; empty chat: flag "was empty"; only server times are
  compared). Whole history `/beitraege` (anchor `#chat-neueste`), post `/beitrag`, threads `/faeden?work=…`,
  thread `/faden?…`. Completion without JavaScript through `datalist`; at the sentence start after two letters the
  form of address („Le“ → „Lektor, “). Mentions: at most three, each only once, case-insensitive; inactive or
  unknown ones are skipped; RFC lifts the limit. Live: a poll every three seconds.
- **Commits:** deleting a post (carries the whole text; in a thread „Fadenbeitrag n: gelöscht“), editing
  („Beitrag n: bearbeitet“ with before/after), turning into an idea. Threads never stand in the mirror: editing
  a thread post yields no commit (the old revision only in the database and the exchange format). After
  moving, the open findings stand at the target section, adopted ones stay at the old one.
- **MCP:** only own posts (`post_edit`, moving, deleting, turning into an idea; `docs/MCP.md`). `chat` without a
  section creates a thread; one relay stage, at most three answers per message, every persona once; per
  client at most two jobs at the same time. All clients without a token share per work 300 posts in 24 hours,
  100 open proposals and four concurrent model jobs (per name still two). Relay answers are repeated on ↻
  with the same form of address. Older answers without a stored job: ↻ repeats the last
  message of the human before it as a chat.
- **Jump labels in the prompt:** `[key:105] 2.3 Titel`; personas name places as `[[key:105]]` or
  `[[key:105|„Zitat“]]`; unchecked ones stay text with „Sprungmarke ungeprüft: …“. HTML from an answer is never
  executed. The closing note („Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft“) stands as a footer.
- **Findings:** page `/werke/<id>/befunde`; file `befunde.md` in the mirror (by part, chapter, section, then
  threads); every status change, every proposal, every decision one commit. „Angehängt“: content words from three
  letters on without filler words, plus every number, Dice measure from 0.8. The matrix counts at most the newest 2000 („Gekappt:
  …“); the list „Offen, älter als n Tage“ capped at 200. Collective proposals do not count in „k offen“; if all
  origins are decided: „veraltet (alle Ursprünge entschieden)“. Jumping there through `find` in the address. At the
  first start with this function Baruch once parses all existing persona answers. The exchange file
  (`.baruch`, minor version 4) carries findings including their history (`docs/Austauschformat.md`). Solutions without JavaScript to
  `/loesungen/act`; variants `/varianten` (siblings in the revision tree; long sections as a partial proposal).
  Mind map: `?befunde=1`.
- **Web search:** MultiGPT tools `web_search` (SearXNG behind it) and `fetch_url`; the first three hosts of the
  source list each with „site:<Rechner> <Anfrage>“; [W1]…[W4], at most 1 500 characters per page; pages over 1 MiB
  or without an answer after 10 s skipped; the whole web search at most 30 s. Quotation check: whole words; with
  an ellipsis „…“ at least three words per piece, at most 400 characters skipped; „(unbekannte Quelle)“,
  „(nicht abgerufen)“; last line „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“. Not fetched are
  `localhost`, 127.x, 10.x, 192.168.x, 172.16–31.x, 169.254.x, 100.64–127.x, `fc00::/7`, `fe80::/10`, names ending in
  `.local`, `.internal`, `.home.arpa`, `.localhost`, names without a dot, addresses with user information, other
  schemes; Baruch asks no DNS (a public name pointing to a private address is not recognised). Evidence page
  `/websuche/belege`; an adopted piece of evidence = a title of the kind „web“ with the note „Abgerufen am … (Websuche @belege)“.
  No MCP tool; through `ask_persona`/`chat` @belege does not search the web. A proposal gets no web search.
  Assumption: names and parameters of the MultiGPT tools as in `connections/multigpt.rs`; only the user checks against a real
  MultiGPT service and a real search engine.

### „Bibel, Literatur und Quellen“ (`bibel-quellen.md`)

- **Versification:** the versification „German“ of the SWORD project (Schlachter 1951, `docs/Architektur.md`): „Ps 51,21“,
  „Neh 3,38“, „1. Kön 5,32“, „Dan 3,33“, „3. Joh 15“. Baruch knows all 18 versifications of the SWORD project
  (Luther, NRSV, Vulg, MT …). Mapping through RPC `scripture.map` or MCP `scripture_map`, default KJV → German
  („Joel 2,28–32“ → „Joel 3,1–5“, „Mal 4,1–6“ → „Mal 3,19–24“, „Ps 51,1“ → „Ps 51,3“), only through a checked
  rule table. Not mapped: the psalms of the Greek-Latin versifications (LXX, Vulgata, Synodal); for
  Catholic, Catholic2, Segond and Calvin there are no checked rules (books counted differently stay
  „Versifikation passt nicht“).
- **Modules:** Baruch reads SWORD modules itself (drivers zText, RawText; compression ZIP, LZSS, BZIP2, XZ;
  commentaries RawCom, RawCom4, zCom, zCom4; lexicons RawLD, RawLD4, zLD), without libsword. Not readable: HREFCom,
  RawFiles, unknown versification. A block that would unpack to more than 64 MiB is refused.
  Module folder = folder with `mods.d/` and `modules/`; default `$SWORD_PATH`, then `~/.sword`, then
  `/usr/share/sword`; a module name counts only once (the earlier folder wins). `DataPath` relative to the module folder,
  `..` refused. Free modules: CrossWire (`https://www.crosswire.org/sword/modules/`), installer `installmgr`
  (package `sword-utils`), BibleTime, Xiphos; or unpack a module ZIP
  (`https://www.crosswire.org/ftpmirror/pub/sword/packages/rawzip/`); own modules with `osis2mod`.
  Licence note from `DistributionLicense`, `Copyright` of the conf. Enciphered = `CipherKey=` in the conf;
  key in the keyring under `sword-cipher:<modul>`, never in the conf, the database or a page. Stored:
  module folders for all works, the Bible per work (migration 0030), commentaries/lexicons per work (migration 0037, form
  to `POST /einstellungen/werk/bibeln`). RPC `bible.set_dirs`, `bible.set_work`, `bible.cipher_key_set`,
  `bible.cipher_key_delete`, `bible.modules`, `bible.set_work_sources`, `bible.work_sources`, `bible.check`,
  `bible.lookup`, `bible.search`, `bible.marks`, `scripture.check`, `commentary.lookup/search`,
  `lexicon.lookup/keys/search`; MCP read only `bible_modules`, `bible_lookup`, `bible_search`, `scripture_check`,
  `commentary_lookup`, `lexicon_lookup`, `lexicon_search`.
- **Wording check:** plain text without footnotes, cross-references and headings is read (canonical
  psalm titles stay). Quotation forms: „…“, “…”, »…«, "…", `#quote[…]`, block quotation
  `#quote(block: true, attribution: [Joh 3,16])[…]` (reference in the attribution, at the end or directly behind it;
  also in `#szene[…]`). Between reference and quotation only whitespace, brackets, punctuation, further references of the same
  list or short introductory words with a colon. Inner quotation marks and several paragraphs (repeated
  quotation mark or up to the closing mark) count; limits: a quotation of the same kind inside a quotation, a forgotten
  closing mark before a paragraph with the same mark. Normalised: punctuation, case, hyphens,
  hyphenations, apostrophes, whitespace, typographic characters, Typst markup (`_…_`, `*…*`, escapes,
  footnotes in the quotation), decomposed umlauts. If the reference stands at the start of a paragraph and the quotation behind it, the latter belongs to the
  reference. Hyphenation remnants in the module („die ses“): an additional comparison without whitespace → „Wortlaut stimmt (Leerraum im
  Modul)“, tolerates only additional word boundaries of the module; a hint for conspicuously many word fragments.
- **Reference detection, details:** books with one chapter („Jud 3“ = Jud 1,3); „4. Mo 25,19“, „5. Mose 6,4“,
  „4Mo 1,1“; „Joh 3, 1. Joh 4“ are two references; only with a verse count „Am“, „Mal“, „Mi“, „Ob“, „Hab“, „Ri“, „Tit“,
  „Da“, „Hi“ („Ob“ only with chapter and verse or a range). No reference before a numeral word („Johannes 3 Mal“,
  „Psalm 23 Jahre“, „3x“, „Stück“, „Tage“, „Uhr“) – only without an explicit verse: „Joh 3,16 Mal 3“ stays
  Joh 3,16, „Joh 3,16 Mal 3,20“ are two references; a time of day after a weekday (Mo–So, hour 0–23, minute 00–59; in the
  reference field „Mi 7:18“ stays Micah). "Verse in the same sentence": at most 120 characters, no sentence end, no
  empty line, no other reference in between (also „Verse 23–24“, „V. 23“); another chapter without a book name
  ends the search. English versifications: „Mal 4,1“, „2. Kor 13,14“ invalid. Sermon text: up to 400 verses
  complete, from 401 on „Nur die ersten 400 Verse eingefügt.“; special characters in the Bible text are escaped.
- **Fact block for personas:** `@schrift` and `@belege` get „Stellenprüfung (Baruch, Verszählung Schlachter
  1951 – kein Wortlaut): …“ or, with a Bible, module, versification, licence note and result per reference. Answers
  carry „Wortlaut: ungeprüft – keine lokale Bibel in Baruch (geprüft nur Buch/Kapitel/Vers).“ or „Wortlaut
  geprüft von Baruch (nicht vom Modell), Modul …, Versifikation …, Lizenz laut conf: …“. If the model separates digits
  („Mk 1 0,42–45“), Baruch joins them (finding 2026-11-06: the separated digits came from the text of the
  model). Notes „(nicht im Text belegt)“, „(Stelle steht im Text: …)“, „(Stelle ungültig: …)“ are set only by
  Baruch; if a model writes them itself, Baruch removes them and checks anew. „fehlt“ counts only with a location. Evidence is
  only the shown text, the job and Baruch's evidence – not foreign findings, earlier posts or ideas
  (these stand framed as "data, not instructions").
- **Checking quotations:** page `/werke/<id>/pruefung?key=…&rev=…` (also on Ctrl/Shift click); highlighting through the
  CSS Custom Highlight API (older browsers: only the list), only whole words, ambiguous wording stays
  unhighlighted („n Zitat(e) im Satzbild nicht eindeutig gefunden (nur in der Liste)“).
- **Lexicon keywords:** first Strong numbers of the verses, then Strong numbers in the text („G25“, „G26“ finds
  „G0026“), then capitalised words with a lemma probe (without -ern, -en, -es, -er, -e, -n, -s; umlaut traced
  back; last component of a compound); at most 5 articles per lexicon; commentary entries at most
  4000 characters.
- **Literature:** page `/literatur?work=…&node=…&rev=…&lq=…`, mode `lmode=rag` (ideas: `mode=rag`),
  document view `/quelle?title=…` (`&quote=…#zitat-…`, `&highlight=…#hervorhebung-…`), full text
  `/quelle/multigpt?collection=…&document=…`, Bible `/literatur/bibel`, hits in the work
  `/?work&node&rev&lq#fundstelle`. The pages remember the section through `key`. Quoting creates
  `#quote(block: true, attribution: [@schlüssel[Stelle]])[…]` or `[Ps 139,23–24 (Modul)]`, originator
  `@literatur`. Bible hits in the search field: at most 10 per Bible, at most three Bibles, 3 s. Full text in parts of
  at most 20 000 characters; a response over 4 MiB is refused; fields „Anfang/Ende im Volltext“ (characters from 0);
  citation keys of adopted MultiGPT documents `mg-…`. A search consisting only of control characters: „Keine Treffer.“
- **Semantic search:** ways LanceDB, pgvector, MultiGPT; hits of several ways once („gefunden von …“,
  Reciprocal Rank Fusion); links only for a safe address (in the work `/…`, otherwise http/https). Opened from a foreign
  page (`Sec-Fetch-Site`/`Origin`), Baruch does not search by meaning and loads no full text.
  At most four searches ask the remote ways at the same time (after that, after 20 s „Zu viele gleichzeitige Suchen …“).
  `weak` in `rag.search` and MCP `rag_search`. Further details under „Einstellungen“.
- **Reconciliation:** page `/abgleich`. Document title „<Nummer> <Titel>“; a part and sections before the first
  chapter each a document of its own; the collection is fixed when switching on. If deleting the
  old document fails, the next reconciliation deletes it. The reconciliation does not upload posts (unlike in the plan;
  noted in the backlog). Fetched documents: kind `remote`; 30 s per request; incomplete lists delete no
  local copy; permissions `docs.write`/`docs.read`; without a document id a hint instead of uploading again; lost
  responses are reported by the next reconciliation; „Verbindung prüfen“ ends its session at MultiGPT. Assumption: tools
  `search_documents`, `read_document`, `list_collections`, `document_info`, `list_documents`, `upload_document`,
  `delete_document` with the parameters from `connections/multigpt.rs`; checked against a mock.

### „Einfälle und Mindmap“ (`einfaelle.md`)

- **Pages:** list `/einfaelle` (`?work=<id>|none|all&word=&status=open|done|all&q=&deleted=1`, meaning
  `&mode=rag`), one idea `/einfaelle/{id}`, mind map `/werke/{id}/karte` (old bookmarks of the idea map
  keep working; `?befunde=1`, collapsed `zu=<Kennungen>`), outline view `/werke/{id}/karte/gliederung`,
  reading in `/werke/{id}/karte/einlesen`.
- **Idea:** duplicate keywords (also in different upper/lower case) collapse; no line break
  in a keyword. Source from the literature: „Literatur: <Titel>“. The server checks deleting. Processing: title from
  the keywords („Laube, Rosen“, at most 200 characters, title rules of the outline); the item and „verarbeitet“
  arise in one step; a post with the author „Autor“ or „Notiz“, the selection as `> …`, source „Einfall <id>“
  or „Einfall 3 von @belege“ / „… von @mcp:claude-code“ (the author of the post stays whoever processes).
- **Mirror:** every action (create, change, delete, restore, process, every action and reversal on
  the mind map) is a commit with origin: `works/<slug>/einfaelle.md` or, without a work, `<data>/einfaelle/`
  (`einfaelle.md`); the delete commit carries the whole entry.
- **Assistants:** full access to ideas (decision IDEA_CRUD; MCP `idea_create`, `idea_update`,
  `idea_delete`, `idea_move`, `idea_link` …). `idea_create` without `parent` and `key` puts it into the cloud. Not yet
  implemented: the lector suggests fitting places on its own.
- **Mind map:** the first half of the main branches stands on the right, the second on the left. Stored positions of the old
  idea map stay in the database, but have no effect. If one end of a relation is collapsed, the
  line ends at the visible ancestor; labels look for a free place on an opaque background (in a
  full map a little beside the line). „widerspricht“ without an arrow, dash-dot. Nearness is never stored
  (stored embeddings or equal keywords, best pair). „Gruppen benennen (Assistent)“: suggestions
  with accept/discard, only until the restart. Collapsed state in `localStorage` per work; the viewport until
  the tab is closed. Shift+Tab and Tab without a chosen node stay with the browser. Ctrl/Shift+click on the
  title opens the idea or section as a link. At chapter, part and work Enter acts like Tab (at the work: a free
  main topic). Dragging an idea onto the upper/lower edge of an idea orders it before/after. Dark theme:
  the scroll bar at the window edge in the colours of the page. Ribbon of the page: groups as
  `details.band-group[data-band-group]` – „Knoten“, „Ansicht“, „Ausgabe“, „Einlesen“, „Gliederungsansicht“, „Zurück“.
- **Keys on the Mac (from `mindmap.ts`):** Alt+E and Alt+L also evaluate `event.code` (KeyE, KeyL) and therefore work
  with ⌥; Ctrl+Z and the mouse wheel evaluate `ctrlKey || metaKey`.
- **State of verification:** the tests check the calls that the widget issues, and the picture that the server then
  delivers; in the visual check it ran in Firefox with synthetic events. Real mouse and keyboard, zoom and
  arrow keys are a manual check (`HANDOFF.md`).
- **Output:** `.xmind` = ZIP with `content.json`, `metadata.json`, `manifest.json`; `.mm` FreeMind/Freeplane;
  `.svg`; `.pdf` (A4 landscape, title and filter); `.md`. In `.svg`/`.pdf` collapsed stays collapsed, in `.mm`
  and `.xmind` never. Transfer: title (chapter with number, „1 Helling“); note (`notes.plain.content` or
  `richcontent TYPE="NOTE"`, one paragraph per line); keywords (`labels` or the attribute „Schlagwort“);
  relations (`relationships` with `end1Id`, `end2Id`, `title` or `arrowlink` with `MIDDLE_LABEL`; label =
  kind and note; `.mm` colours per kind); not: status, origin, reference (only in `.md`), quotation, source, Bible reference,
  positions, images, styles, markers.
- **Reading in:** HTML notes become plain text; relations come as „führt weiter“ with the label as the
  note; the main topic of the file does not itself become an idea; the own branch „Wolke“ is recognised by its identifier
  (`baruch-wolke`), not by its title; „unter Knoten …“ knows no cloud. One commit, origin human.
  Limits: `.xmind` archive at most 50 entries and 20 MiB unpacked; at most 20 keywords per node; `.mm`
  only UTF-8 or ASCII; XMind 8 (only `content.xml`) refused. Not read: further sheets (hint),
  summaries, callouts, images, markers, styles, other attributes, `richcontent TYPE="DETAILS"`.
- **Assumptions about XMind and Freeplane (not checked, manual check by the user):** the structure of the `.xmind` file
  follows the open description of the format from memory; only the round trip with the own
  reader is checked.

  | Assumption | Risk if it does not hold |
  |---|---|
  | XMind (2020 and newer) opens a ZIP with `content.json`, `metadata.json`, `manifest.json` without `content.xml` and without a preview image | file does not open |
  | `content.json` is an array of sheets with `id`, `class: "sheet"`, `title`, `rootTopic`, `relationships` | sheet is not recognised |
  | Topics: `id`, `class: "topic"`, `title`, `children.attached`, `notes.plain.content`, `labels` | notes or labels are missing |
  | `structureClass: "org.xmind.ui.map.unbalanced"` distributes the branches right and left | a different arrangement |
  | Topic ids may be arbitrary texts (`i:12`, `n:7`, `baruch-wolke`) | relations or topics are missing |
  | `relationships`: `id`, `end1Id`, `end2Id`, `title` suffice | relation lines are missing |
  | `manifest.json` with `file-entries` for both files | warning when opening |
  | On reading in: free topics under `children.detached`, HTML notes under `notes.realHTML.content` | contents are missing after reading in |
  | Freeplane reads `MIDDLE_LABEL` and shows attributes; FreeMind 1.0.1 skips `MIDDLE_LABEL` | the label is missing in the display |
  | Freeplane writes labels as `MIDDLE_LABEL` (otherwise `SOURCE_LABEL`/`TARGET_LABEL`), notes as `richcontent TYPE="NOTE"` | label or note is missing after reading in |

### „Suchen, Verweise, Register und Glossar“ (`suchen.md`)

- **Search:** page `/suche`
  (`/suche?work=W&q=…&range=…&key=…&sel=1,2,3&case=1&word=1&re=1&kinds=text,heading`); selection from the editor
  `range=selection&start=…&end=…` (UTF-16, range „Markierung im Editor (Zeichen a–b)“); shown revision
  `shown_rev=n` or `rev=shown` (only for section and selection; preview and replacing always apply to the current
  revision). Hit links `/?work=W&key=K&find=…&at=…`. Without parts the range „Teil“ is the whole work.
- **Rules:** the search runs in the Typst source text. In wording mode a Typst special character also finds its
  escaped form („C#“ finds „C\#“), the replacement is escaped. Regular expression: syntax of the Rust crate `regex`,
  groups `$1` or `${name}`, replacement literal in the source text. Hits: default 500 shown, at most 2000 (more
  are counted); after „abgebrochen“ replacing only works with a smaller range. The preview shows only whole
  sections with together at most 2000 places; over 8 MiB of inserted text: „Zu viele Ersetzungen auf einmal“.
  The preview does not start on the request of a foreign page (403). A section whose current revision has
  changed since the preview is skipped with a reason.
- **Commits:** `Ersetzt „x“ → „y“ in n Abschnitten` (one commit for all), „Ersetzen rückgängig: …“. MCP only searches
  (`text_search`); proposals there through `propose_revision`.
- **Keys (from `search_keys.ts`):** Ctrl also counts as ⌘ (`ctrlKey || metaKey`); letters through `code`, therefore
  Alt+C/W/R/L work on the Mac with ⌥. macOS usually catches ⌘H itself (hide window) – not checked on the
  Mac; Ctrl+H reaches the browser. After „Alle“ „⌶“ switches off again. With the typeset view open the editor switches
  to the source text after replacing. On `/suche` Alt+C/W/R toggle the checkboxes when the focus is in the search or
  replacement field.
- **Jump labels:** every outline item has `<k{Schlüssel}>` (such as `<k103>`), only in export, mirror,
  typeset page and PDF: at the line end of the heading (`= Einleitung <k103>`); parts, items without a number and
  headings with their own label keep their label and get `#metadata("baruch-key") <k103>` below it. At the
  first start with this function every mirror changes once (one commit per work). What is inserted is
  `#link(<k103>)[die Einleitung]` or, with a page number,
  `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]`. Without JavaScript one writes the
  form oneself; `xref.targets` or `/verweise` (`?key=K`) name the labels. Ctrl+click also works on
  `<kN>`/`#link(<kN>)` in the source text. Jump history per browser tab (`sessionStorage`); „Vorwärts“ only as a button.
  When an export is read in again, the labels disappear from the headings, the references point to the
  new keys. In the EPUB internal links to the heading in the chapter file. Counted mentions („das dritte
  Kapitel“) only for chapters with a number. If the work setting „Verweis mit Seitenzahl“ is unset,
  the browser remembers the last choice; when set, a change in the dialog holds only for this reference.
- **Glossary and index:** pages `/werke/{id}/glossar`, `/werke/{id}/register` (`?key=K`: section with „Marke
  setzen“), `/werke/{id}/markierung?key=K&rev=R`; without a work `/werke`. Index mark in the text:
  `#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>`; never in headings, code, comments, raw
  or another mark; message „… trägt an der gewählten Fundstelle schon diese Registermarke; nichts
  geändert“; „alle“ only sets missing ones. Own indexes: identifier of lower-case letters, digits, hyphen
  (e.g. `orte`), title up to 80 characters, at most 13. Index proposals become „veraltet“ when the word is missing
  or already carries the mark; they stay on their instance. Discarded glossary proposals leave a
  tombstone (they do not come back on exchange). RPC `glossary.suggest`; MCP `glossary_list`,
  `glossary_propose`, `index_propose`. Mirror: file `glossar.md`, every change a commit. The exchange file
  (minor version 7) carries the glossary including proposals, own indexes and the typesetting settings for them; at most
  5000 glossary terms, index titles at most 80 characters (otherwise the file is refused entirely).
- **In typesetting:** index and glossary pages arise only when writing out (Typst export, mirror,
  typesetting preview, PDF, EPUB) from the line `// Baruch: Register und Glossar` on; the stored text stays. The
  export can be typeset with `typst compile` without a package and without a network (checked with the embedded compiler).
  In the EPUB `Glossar` and a file of its own per index at the end. Bible reference index: not from comments, code
  or headings; order of the biblical books. On reading in again the index marks of the
  human stay; the emphasis of the first mention without a glossary page stays as an ordinary emphasis.
  Checked end to end in `tests/round31_abnahme.rs`.

### „Buchsatz: PDF, E-Book und Umschlag“ (`buchsatz.md`)

- **Addresses:** typesetting `/werke/<id>/satz?key=<schlüssel>` (section `#druckpruefung`), PDF `/werke/<id>/pdf`
  (`<slug>.pdf`), cover `/werke/<id>/umschlag`, cover PDF `/werke/<id>/umschlag.pdf`
  (`<slug>-umschlag.pdf`), EPUB `/werke/{id}/epub` (`?fussnoten=endnoten`), preview `/werke/{id}/epub/vorschau`,
  check `/werke/<id>/epub/pruefung`, EPUBCheck `POST /werke/epub/pruefen`, keyword notes
  `/werke/<id>/stichwortzettel` and `…/stichwortzettel.pdf?format=…&schrift=…` (`<slug>-stichwortzettel.pdf`),
  Word `/werke/<id>/word` (`<ordnername>.docx`). Draft: every output with `?entwurf=1` or `?entwurf=0`
  (`/pdf`, `/stichwortzettel.pdf`, `/epub`, `/umschlag.pdf`, `/satz`).
- **Typeset page:** loads no JavaScript (the theme symbol leads to „Einstellungen → Darstellung“, „Einstellungen“ opens
  the page, no Alt+, and no arrow keys). A page that two chapters share stands in the list of both;
  a living running head does not count as text of the chapter. Messages with line and column in the export.
  Missing font: `Schrift „x“ fehlt; ersetzt durch eingebettete Schrift`. Packages (`#import "@preview/…"`):
  „Typst-Pakete werden nicht geladen (offline)“. `image("/x")`, `image("./x")`, `image("x")` find the same image.
  After 60 s „Typst brauchte länger als 60 s“ (503); cache at most 8 states and 192 MiB of pages.
  Cancelling: „Der Satz wurde abgebrochen. Es wurde nicht neu gesetzt.“ / „Es lief kein Satz, der abzubrechen
  war.“; waiting requests 503. Typst cannot be stopped in the middle of computing: it stops after the layout;
  a long loop of the template runs to its end. From a foreign website the typeset page, page images and
  PDF start no typesetting (403). Print check, example: `Bild „umschlag.png“: 142 dpi bei 14,8 × 21,6 cm – für 300 dpi
  braucht es 1748 × 2552 Pixel`; SVG never appears; it does not count among the errors of Typst.
- **Typesetting settings:** table `work_typeset`, every change a commit; rule block `// Baruch: Satz` behind
  the work template in `main.typ`, export, typesetting preview, book PDF and cover – only when at least one
  setting is set. Only the human changes them (MCP `typeset_settings` reads). Anchor `#satz`. Limits and
  details: binding offset empty = 0; paragraph spacing empty = 1 or 0.5 em; language e.g. `de`, `en`, `de-CH`; counting „ab
  dem ersten Kapitel“: the first chapter page carries the 1, a cross-reference „S. n“ names the printed number; the
  work title for the running head comes from `#set document(title: …)`; the break at the chapter start also holds in the
  export before a part; with indented paragraphs the first paragraph after a heading stays without indentation.
  Adoptable rule: `#show heading.where(level: 1): it => { pagebreak(weak: true, to: "odd"); it }` (also
  varied, multi-line or `it => [#pagebreak() #it]`); adopting = a new revision of the work template without the
  line, one commit. Preview images lie only in the cache. „Erzeugte Typst-Regeln“ shows the block.
  The page size of the cover follows the first book page; the spine computes with the page count including blank pages.
- **Typeset view settings** (anchor `#satzbild`, SQLite only, not in the mirror and not in the exchange format):
  font as a list as in CSS (at most 200 characters, without `; < > { }`), size 6–48 pt, text width 6–60 cm with
  `pt`, `mm`, `cm`, `em`, colours `#rrggbb`; „Alles aus der Vorlage“ resets all of them.
- **Draft:** text at most 40, addition 120 characters; date = day of the output in local time; opacity 5–50 %
  (default 12 %); on the cover PDF only with the checkbox (then a warning of the print check); EPUB: a note on the
  title page and „(Entwurf)“ in the title; export, mirror and generated rules stay without a watermark.
- **PDF:** error 422 (page with messages), 409 („Für dieses Werk entsteht gerade ein PDF …“), 503 after 120 s.
  Command line `baruch-server export <id|slug> --pdf DATEI [--data ORDNER]`: an existing file exit 1 („… gibt es
  schon; nichts überschrieben“), typesetting error exit 1, `--pdf` with `--out` exit 2; warnings of the print check on
  stderr. `pdfinfo` (poppler 25.03) reports for all PDFs „Syntax Error: Suspects object is wrong type
  (boolean)“ – harmless: Typst writes `/MarkInfo << /Marked true /Suspects false >>` (PDF/UA); measured in the test
  `round21_bilder`.
- **Cover:** blocks per face in mm from the trim edge at the top left (negative = in the bleed; y between −50 and
  1000); colour `light`, `dark` or `#rrggbb`; free text/motto at most 1000 characters; snapping up to 1.5 mm. Without
  JavaScript the address carries the values as `b.<Art>.<Feld>=…` (e.g. `?b.front_title.y_mm=40`). Quick choice and
  reset are one commit each. As soon as a block is stored, Baruch typesets the cover from the blocks
  (preview, cover PDF, front/back in the book PDF). Font without a choice: the first one from the last
  `#set text(font: …)` at the top level. Typst measures the height of the blocks with the text and paragraph rules of the
  work template (`#set text(…)`/`#set par(…)` at the line start; not font, size, colour). Uploading own font files
  does not work yet. Name tags of the spine blocks only at the chosen block. Contrast: WCAG below 3 : 1,
  measured in the raster image including shadow/gradient; SVG not measured; images over 20 000 pixels of edge length „nicht
  gemessen“. Spine computation: „Rücken 1,5 mm = 30 Buchseiten (15 Blatt) × 0,1 mm je Blatt (die 3 Umschlagseiten
  im Satz zählen nicht)“. Deleting the cover image first removes the cover (commit „Umschlag entfernt“); a
  confirmed cover request is a commit „Bildantrag n bestätigt: …“. RPC `cover.layout`; MCP `cover_layout`
  reads, `cover_layout_set` only files a request.
- **Book data:** year with four digits; ISBN-13 with or without hyphens, prefix 978/979, stored as
  entered; single-line fields 200 characters, blurb 4000, short biography 2000, dedication, rights notice,
  picture credits 1000, addresses 500; invalid 422; commit „Buchangaben geändert: …“.
- **Front matter from the work template:** functions `#let schmutztitel(buch) = …`, `haupttitel`, `impressum`, `widmung`;
  `buch` is a dictionary of strings: `titel`, `untertitel`, `autor`, `verlag`, `verlag_anschrift`,
  `ort`, `jahr`, `auflage`, `isbn`, `preis`, `reihe`, `druck`, `druck_anschrift`, `bibel`, `bibel_hinweis`,
  `bildnachweis`, `widmung`, `klappentext`, `vita`, `copyright`, `dnb_titel`, `dnb_text`, `gesetzt`,
  `impressum_zeilen`. Order: half title, page 2 (series or empty), main title, imprint (note of the
  German National Library, © line, edition, ISBN, Bible translation, picture credits, „Gesetzt mit Baruch“,
  printer), dedication including a blank page. New books get the imprint template appended. Addresses: publisher after
  the © line, printer behind „Druck:“, lines joined with a comma. Back: barcode EAN-13 or
  placeholder.
- **EPUB:** EPUB 3 with `toc.ncx` for EPUB 2 readers; without Typst typesetting, no commit; `break-before` in the style.
  Cover image: the typeset front as PNG up to 1600 × 2560 pixels; if that fails, the image file with the warning
  `COVER_RASTER_FAILED`. Language: typesetting setting, otherwise `#set text(lang: …)`, otherwise German. Hint codes:
  `ISLAND_OMITTED`, `IMAGE_NO_ALT`, `IMAGE_MISSING`, `ARGS_OMITTED`, `TEMPLATE_PAGEBREAK_MAPPED`,
  `TEMPLATE_PAGEBREAK_UNMAPPED` (RPC `work.epub`, MCP `epub_export`, CLI stderr). Same state, same bytes;
  without a section 422; foreign page 403. A reference `@name` to a figure shows „Abbildung n“; a footnote
  in a footnote stands behind its outer one. Profiles of the preview: Apple Books (iPhone, iPad), Google Play
  Books, „Kindle (über EPUB)“, „Kindle-App (über EPUB)“, selectable tolino, Kobo, Thorium Reader, calibre – assumptions
  without a device; popup footnotes appear as a box; unknown readers/files 404 with a hint; arrow keys in
  a selection list do not reload (Enter confirms). Check: `mimetype` as the first uncompressed entry,
  `META-INF/container.xml`, package document, manifest, spine, nav, NCX; well-formedness with `xmllint --noout
  --nonet`, otherwise quick-xml. EPUBCheck: `epubcheck` in the PATH, otherwise the configured `epubcheck.jar` with
  `java -jar`; at most 3 minutes, one at a time; MCP `epub_check` gets only the own check.
- **Keyword notes:** mark `// Stichwort: …` (case-insensitive), otherwise `*…*` and `#strong[…]`, otherwise the first
  sentence per paragraph. No sentence end after a digit („1. Kor“) and after abbreviations („z. B.“, „d. h.“, „u. a.“, „bzw.“,
  „vgl.“, „usw.“, „ca.“, „Nr.“, „S.“, „Kap.“, „V.“, „etc.“, „ggf.“, „evtl.“ …). Not counted: headings,
  comments, `#set`/`#show`/`#let`/`#import`/`#include`, raw, maths, `#footnote[…]`, `#arbeit[…]`; deeper than
  32 levels is dropped; `@name` is dropped (an `@` in the middle of a word stays). Bible references uniform, each once.
  A5 portrait default 16 pt, cards 14 pt. Errors 409/422/503 as for the PDF. Command line
  `baruch-server export <id|slug> --stichwort DATEI [--format a5|karten] [--schrift PT] [--data ORDNER]`
  („Stichwortzettel geschrieben: DATEI (n Seiten, m Bytes)“; wrong combinations exit 2).
- **Word export:** styles „Überschrift 1–3“ (deeper „Überschrift 4–6“), „Baruch Teil“, „Baruch ohne
  Nummer 1–3“, „Zitat“, „Baruch Zitatquelle“, character style „Baruch Zitat“; tables from
  `#table(columns: …, […], …)` (also `table.header`); images PNG, JPEG, GIF embedded; Typst islands with
  hidden source text. Warnings (RPC, CLI): `docx_typst_insel`, `docx_bild_fehlt`,
  `docx_tabelle_vereinfacht`, `docx_weitere_vorschlaege`; the page in the browser delivers without a report.
  Change tracking: word by word; a new/struck figure as a paragraph; for tables the empty grid stays;
  page breaks not tracked; a level change of a heading as struck and new. Hidden revisions
  do not count. Command line
  `baruch-server export ID|SLUG --docx DATEI [--vorschlaege] [--data ORDNER]`. It does not exist through MCP.
- **Typst command line:** `baruch-typst compile buch.typ buch.pdf` or `baruch-server typst compile …`
  (`docs/Verteilung.md`, "Typst command line").

### „Austausch, Sicherung und Sicherheit“ (`austausch-sicherung.md`)

- **Exchange:** format in `docs/Austauschformat.md`. Output `/werke/{id}/austausch` (without a work to the list of works):
  `.baruch` = ZIP container with the work as XML, the images as files of their own and a manifest with checksums;
  `.baruch.xml` = one XML file with the images as Base64. Scope also: tombstones of deleted parts, the mind map with
  connections (also deleted ones) and remembered positions (minor version 2), findings including their history (minor version 4),
  glossary and indexes (minor version 7), history of the amendments. Not in it: keys and tokens,
  connections (models, MultiGPT, pgvector), personas including models, open requests, the computed nearness, Bible
  and literature texts, accounts, sessions, shares, the settings „Fassungen“ and „Satzbild“, marks for
  hiding. Command line `baruch-server export ID|SLUG --baruch DATEI [--data ORDNER]` (if `DATEI` ends in
  `.xml`, XML only; an existing file exit 1; with `--out`, `--pdf`, `--stichwort` exit 2; the output names the work,
  identifier, size, SHA-256). RPC `work.exchange_export`, MCP `exchange_export` (read only, at most 4 MiB).
- **Reading in:** `/austausch/einlesen`; deviation links `/?work=W&key=K&rev=R`; commit „Austausch aus
  <Instanz>“. A connection of the mind map that already exists (same ideas, same kind) does not come twice;
  a position only for ideas without one of their own. If an item over there was renamed through the outline, the revision
  travels along with the new heading: hint „Umschalten genügt“ instead of a request. Unknown things from a newer
  file are not taken over. Fast-forwarding only for the own current revision and only when the steps of the file
  yield the new text without gaps; revisions of personas or MCP clients, proposals and superseded revisions
  are never overwritten („abweichende Fassung“). A new part from the file arises after approval „mit
  Kennung und n Fassungen“ exactly as over there (text, origin, status, history). Findings that already exist here
  keep status, reason and history („… 2 Einträge im Verlauf der Datei nicht übernommen“). Cover blocks
  arrive in normal form (font name without whitespace at the edges, colour lower case, line ends `\n`); a block that the
  check of the editor refuses is taken over and stands among the hints. If the heading of a revision does not fit
  the kind of its item (`===` at a chapter), a hint stands there. Command line
  `baruch-server import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title TITEL] [--data ORDNER]`
  (work already present without `--abgleichen`: exit 1; `--abgleichen` for an unknown work: exit 1; with `.typ`: exit 2).
  Reading in does not exist through MCP.
- **Instance:** identifier = UUID, assigned at the first start; name without line breaks or tabs, whitespace at the edges
  is dropped; without a name „ohne Namen (3f2b8c1e)“ (first 8 characters of the identifier). Identifier of the work (UUID) under
  „Werk bearbeiten“.
- **Backup:** folder `<Name>.sicherung-<JJJJ-MM-TT-HHMMSS>` (local time), database through `VACUUM INTO`, plus
  mirrors, ideas and everything else; symbolic links are skipped; a second backup in
  the same second fails; limit 10 backups; after the backup 512 MiB must stay free. Address
  after creating `?ok=sicherung`; „Sicherung angelegt“ stands there only once. Not through MCP. Restoring:
  `docs/Betrieb.md`.
- **Security:** writing calls (`/rpc`, `/act`, `/mcp`) only from the own page; another `Origin` or
  `Sec-Fetch-Site` → 403 (scripts without a browser do not send these headers). Likewise 403 for typesetting, typeset pages
  as images, cover (page, image, PDF), book PDF, keyword notes PDF and mind map as PDF, when a foreign page
  triggers them. All pages answer only under an IP address or `localhost` (`http://127.0.0.1:8765/`,
  `http://[::1]:8765/`, `http://localhost:8765/`); a foreign name (DNS rebinding) → 421. Under a host name
  only with `--allow-host NAME` / `BARUCH_ALLOWED_HOSTS` (exactly the name, any port in `Host`; no
  wildcard, no port, no IP – invalid: exit 2). The proxy must pass `Host` through unchanged (otherwise 403
  „Origin passt nicht zu Host“); Baruch does not evaluate `X-Forwarded-*`. Under an allowed name a
  request never counts as "from the own machine" (`/mcp` there always with a token, 401 without). Without accounts the proxy must protect
  the access. Examples (nginx, Caddy), probe and TLS to pgvector (`PGSSLROOTCERT`): `docs/Betrieb.md`.
  `/mcp` without a token only from 127.0.0.1 or ::1 (otherwise 401); with a token also from other machines, everything else there
  403. Through MCP only proposals, posts and ideas; sections only through `key` (a call with `node` is
  refused, `docs/MCP.md`). The import reads only ordinary files (FIFO or device are refused before opening).
- **Data folder:** message about permissions with a number (e.g. 0755); `chmod 700 <Ordner>` lets only the owner in.

### „Einstellungen“ (`einstellungen.md`)

- **Addresses:** `/einstellungen/<seite>` (`darstellung`, `werk`, `assistenten`, `verbindungen`, `quellen`, `mcp`,
  `daten`, with accounts `konten`); the old address `/einstellungen/personas` redirects to `assistenten`. The popup is
  up to 1480 px wide; a refused form does not count as saved (theme, typeset view and badges of the
  work page are then not fetched anew). Badges and popup need JavaScript.
- **Cookies (`baruch_*`):** `baruch_work`, `baruch_editor`, `baruch_pv` (proposals initially open/closed),
  `baruch_folded` (at most 200 entries `<key>-<rev>`), `baruch_band`, `baruch_laptop`, `baruch_lrail`,
  `baruch_rrail`, `baruch_zoom`, plus column widths, the open area on the right, collapsed chapters and the theme.
- **Work language (Round 38):** block „Werksprache“ on the page
  `/einstellungen/werk` (`id="sprache"`, directly before the time keeper `id="zeit"`): a `<select name="language">`
  with „Deutsch“ (`de`) and „Englisch“ (`en`) and the button „Speichern“; a plain form, usable without JavaScript.
  Route `POST /einstellungen/werk/sprache` (fields `work`, `language`), answer 303 back to the page with
  `ok=sprache#sprache`, on an invalid value `sprachfehler=1#sprache`; for an unknown or missing work `sprachfehler=werk#sprache`
  (303 to `/einstellungen/werk?sprachfehler=werk#sprache`) with the message „Die Werksprache ließ sich nicht speichern: dieses
  Werk gibt es nicht; nichts geändert.“ (nothing is changed); locked for Lektoren (not in `LEKTOR_ROUTES`).
  RPC `work.language` and `work.language_set` (only the human, not through `/act`, not through MCP); `work.get`,
  `work.meta`, `work.list` and MCP `list_works`/`outline` carry `language`. Stored only in the database
  (`work.language`, migration 0055), no commit in the mirror. The assistants answer in the work language; the
  language of the user interface does not change, and neither does the hyphenation language of the typesetting
  settings. Help page: `site/content/de/hilfe/einstellungen.md`.
- **Work:** stored in SQLite and in the Git mirror. Reconciliation switch: checkbox „Ja, die aktuellen Fassungen
  dieses Werks dürfen nach MultiGPT“; the switching itself sends nothing.
- **Assistants** (SQLite, all works): tabs without JavaScript as `/einstellungen/assistenten?persona=<handle ohne
  @>` (anchor `#persona-<handle>`; an unknown `?persona=` shows the first tab with a hint). Limits: name 1–80,
  role and model 1–200 characters, context 1–10 000 000 tokens, prompt 1–8000 characters, colour `#rrggbb` or `#rgb`
  (a filled-in text field takes precedence); invalid 422. Palette: eight colours in the order of the personas;
  backgrounds are brought to at least 4.5 : 1 contrast (WCAG AA); colours do not stand in the mirror and
  cannot be changed through MCP. On a tie of the context the earlier persona holds. Warnings „Modell …
  ist unbekannt bei Eliezer“, „Alter Name: Alias → …“, a hint about the graphics memory for different models,
  „Modellliste nicht verfügbar: …“. The context length serves only for shortening; Baruch never requests one. Forms:
  `action=rag` (up to five hits `[R1]`…`[R5]`, the reviewed section is never among them), `POST /websuche/act`,
  `action=backend`, `action=bible` (default: @belege on, lector off). Effective context: Eliezer and Ollama
  report the length, OpenAI-compatible services do not; with Ollama `num_ctx` from the Modelfile counts, otherwise the length
  of the loaded model, otherwise 4096 tokens. When opening, Baruch asks no peer (only the shown tab on
  request, at most 10 s, remembered for 60 s); example „wirksam 4096 Tokens (Einstellung 32768), Gegenstelle meldet
  4096“.
- **Connections** (SQLite; secrets in the keyring, service `baruch`):

  | Connection | Fields | Secret | Default |
  |---|---|---|---|
  | Eliezer (model backend) | address | key `baruch/eliezer` | `http://127.0.0.1:1234` |
  | MultiGPT (MCP) | address | token `baruch/multigpt` | empty (= not set up); placeholder `https://multigpt.example/mcp/` |
  | pgvector (PostgreSQL) | host, port, database, user, TLS (`disable`/`prefer`/`require`/`verify-full`); table (`tabelle` or `schema.tabelle`), columns text, vector, collection, document (mandatory), position, title, address, model; embedding model; query prefix | password `baruch/pgvector` | host empty, port 5432, TLS `prefer`; `chunks`, `content`, `embedding`, `collection`, `document`, `nomic-embed-text` |
  | LanceDB (local) | folder (relative to the data folder or absolute), embedding model, prefix of the query and of the documents | – | `lancedb`, `nomic-embed-text` |

  MultiGPT formerly had an intranet address as its default; since then it is empty. The placeholder is never
  stored and never requested; a stored address stays (no migration); whoever only used the default
  enters the address once. Without an address: way „aus“, full text „Keine Adresse für MultiGPT eingetragen
  (Einstellungen → Verbindungen)“. Task prefixes (for nomic-embed-text `search_query: ` and
  `search_document: `, the space belongs to them; at most 64 characters); a changed document prefix requires
  „Index neu aufbauen“. TLS hint for a password with `prefer`/`disable`, milder with `require`; only `verify-full`
  without a hint. Column names: letters, digits, `_`, at most 63 characters, confirmed against
  `information_schema`. Secret: not empty, one line, at most 4096 characters; bound to scheme, host
  and port (pgvector: host, port, TLS) – otherwise „Nicht geprüft: …“ or „Schlüssel gilt für ein anderes Ziel;
  nicht gesendet“. Addresses only `http://`/`https://`, without user information and without a token in the query.
  **„Verbindung prüfen“** (10 s together, response at most 1 MiB, no redirect – a redirect is
  an error, secrets never go to its target): Eliezer `GET <Adresse>/v1/models` with `Authorization: Bearer`,
  afterwards a comparison of the models of all personas (list valid for 60 s); MultiGPT MCP `initialize` (protocol
  `2025-06-18`, client `baruch`), `notifications/initialized`, `tools/list` with `Mcp-Session-Id` over at most
  10 pages (`nextCursor`), `list_collections`, JSON or `text/event-stream`; pgvector login (never from
  `PGPASSWORD`/`~/.pgpass`), version of the extension `vector`, table, collections, dimension, models (values
  shortened: collection names 200 characters, at most 1000 collections); LanceDB folder, tables, entries, model,
  dimension and probe embedding „Baruch prüft die Einbettung.“. HTTP 401: „Zugang verweigert … Schlüssel
  baruch/… prüfen“. Short test: at most 16 tokens, preselected `google/gemma-4-26b-a4b`. Model connections
  (section `#modelle`): kinds Eliezer, OpenAI-compatible (`/v1/models`, `/v1/chat/completions`), Ollama;
  key `baruch/model-<id>`; local „nach der Adresse“ are loopback (`127.…`, `localhost`, `::1`), private
  addresses (`10.…`, `172.16–31.…`, `192.168.…`, `fd…`), names without a dot and under `.local`, `.lan`, `.internal`,
  `.home.arpa`; one request at a time per connection; if Eliezer itself is external, Baruch embeds nothing.
  Forms to `POST /einstellungen/verbindungen/act` or `POST /einstellungen/modelle/act`, response 303 with
  `?ok=<code>` / `?fehler=<code>[&kind=…]` / `?modellfehler=…`; inputs never stand in the address.
- **Semantic search (RAG):** section `#rag`; default without a choice: LanceDB; a request (RPC, MCP, persona) can
  only leave ways out. Collections at most 100, each 1–200 characters. The local index (default
  `<Datenordner>/lancedb`) embeds only changed paragraphs anew (through Eliezer, its own queue, at most 32
  texts per request), catches up on all works at start, remembers model and dimension („Der Index ist mit dem
  Einbettungsmodell … aufgebaut, eingestellt ist …“). If embedding fails three times, jobs stay behind
  (`n Aufträge liegengeblieben – erneuter Versuch nach einer Änderung oder „Index neu aufbauen“`); a search
  does not try again. With an own `--data` without a stored Eliezer there are no
  embeddings at start. „Index neu aufbauen“ answers immediately and aborts a hanging job; only the human. Per
  search all ways at the same time (20 s each). pgvector: cosine distance, read only. MultiGPT: `search_documents`,
  `read_document`.
- **Sources:** title 1–200 characters, address http/https; links with `rel="noopener noreferrer"`; MCP
  `sources_list`; forms to `POST /einstellungen/quellen/act`. A bookmark list cannot be read in.
- **MCP access:** only the SHA-256 hash is stored; address `http://<Rechner>:<Port>/mcp`; token `baruch_…`, name
  1–64 characters; author `@mcp:<tokenname>` (from `clientInfo` only the version counts; the table names „früher
  `@mcp:…`“ when the name rule formerly yielded another author; refused are names that would yield the same author
  as an existing or revoked token or that begin with „lokal-“). Command:
  `claude mcp add -s local --transport http baruch http://<IP>:8765/mcp --header "Authorization: Bearer <Token>"`.
  Precondition: `--listen 192.168.…:8765`; addressing through the IP address (otherwise 421). From other machines
  only `/mcp` is reachable (otherwise 403 „Von anderen Rechnern nur /mcp mit Token.“); a wrong token is refused
  locally as well (401). Forms to `POST /einstellungen/mcp/act` (creating 200, revoking 303 `?ok=widerrufen`).
  Behind a reverse proxy the page shows `https://<Name>/mcp` (`https` is an assumption); basic auth of the proxy
  must exempt `/mcp` (`docs/Betrieb.md`).
- **Data:** EPUBCheck path absolute, extension `.jar`, an existing file; `epubcheck` in the PATH takes precedence.

### „Konten und Lektoren“ (`konten.md`)

- **Addresses:** `/einrichten` (only at the own machine, otherwise 403), `/anmelden`, `/konto`,
  `/werke/<id>/mitwirkende`, „Einstellungen → Konten“. With accounts, pages, forms and `/rpc` require a
  login; `/mcp` stays with tokens.
- **Permissions:** the only place of the check is `roles` (`LEKTOR_ROUTES`, `LEKTOR_RPC`); everything not allowed
  answers 403 „Abgelehnt: …“. Origin `@lektorat:<name>`; every proposal, post and finding is a commit.
- **Stored:** password as an Argon2id check value, session as a hash; not in the exchange format and not in the
  mirror. The browser does not send the session cookie on a jump from outside („Schon angemeldet?
  Weiter“). Login and setup follow the theme cookie without JavaScript.
- **Command line:** `baruch-server passwort NAME [--data ORDNER]` reads the password as one line from the
  input, ends all sessions and lifts the lockout after failed attempts.
- **"At the own machine without a login":** if a reverse proxy on the same machine rewrites `Host`,
  every visitor would look like the own machine (`docs/Betrieb.md`).
- For Lektoren the cover image in the list of works is not a link; they reach „Einstellungen → Darstellung“ even
  without a shared work. Open: whether Lektoren may read the book as a PDF (currently not).

### „Tasten und Maus“ (`tasten.md`)

- The Mac column is derived from `crates/baruch-server/web/src`, not checked on a Mac: `ctrlKey || metaKey` in
  `panes.ts` (Ctrl+S), `wysiwyg.ts` (Ctrl+I, Ctrl+B, Ctrl+Enter), `search_keys.ts`, `threads.ts`, `xref.ts`,
  `cover_editor.ts`, `mindmap.ts`, `zoom.ts` (only Ctrl+0). Only `ctrlKey`: the mouse wheel in `zoom.ts`. Alt shortcuts
  with `event.code`: Alt+E (`ideas.ts`, `mindmap.ts`), Alt+L in the mind map, Alt+C/W/R/L of the search. Only through
  `event.key`: Alt+L for the literature (`literature.ts`) and Alt+, for the settings (`settings.ts`,
  `aria-keyshortcuts="Alt+Comma"` in `base.html`) – on the Mac ⌥ produces another character there, so the shortcuts
  probably do not take effect (a candidate for a correction).
