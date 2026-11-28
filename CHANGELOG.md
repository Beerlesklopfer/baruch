# Änderungen

> From Round 38 on, new entries are written in English; older entries stay German.

Alle nennenswerten Änderungen an Baruch, die neueste Version zuerst. Die Versionsnummer legt der Autor fest
(`[workspace.package] version` in `Cargo.toml`). Einzelheiten je Runde – Commits, Dauer, Befunde – stehen in
[docs/Fahrplan.md](docs/Fahrplan.md), offene Punkte in [docs/Backlog.md](docs/Backlog.md).

## Unveröffentlicht

### Developer documentation on the website, help gaps, rest of the interface literals (Round 39c)

- **Developer documentation on the website:** `/en/docs/` with nine pages (usage, architecture, RPC, MCP, exchange
  format, WYSIWYG, operation, distribution, plan), generated from `docs/*.md` by `site/tools/docs_pages.py` (run by
  `scripts/ci-site.sh`, output in the ignored folder `site/generated/`). Links between the documents become site
  links, links to other repository files become links to GitHub. Menu entry "Documentation" (English) and
  "Dokumentation (englisch)" (German).
- **Help gaps closed (German and English):** what is logged and the Git mirror (`#protokoll`, `#spiegel`), dark
  theme (`#dunkles-thema`), rebuilding the index (`#index-neu-aufbauen`), reading in a source file
  (`#quelle-einlesen`), word counts (`#statistik`).
- **New work language:** the form for a new work and the RPC method `work.create` (optional `language`, `de` by
  default) set the work language; the preset titles (first chapter, the parts of a sermon) follow it
  (`book.preset.*`). German presets are unchanged.
- **Interface in English, rest:** the RAG index, the MultiGPT and pgvector connections, import, archive, Markdown
  import, the mind map, the store texts reachable from forms and RPC, Bible module notes, scripture check refusals,
  print and typesetting labels, numbering refusals and the cross reference tooltip come from the catalogs; the
  draft date of an English work is written as `2026-11-28` (`typeset::draft_today_in`). Catalog wording aligned
  (Lector, "Exchange: Write out/Read in", "revisions", B and I buttons). Website shortcode words from the theme
  catalogs. German texts stay byte-equal.

### Book outputs in the work language, English website and help (Round 39b)

- **Book outputs follow the work language:** EPUB, PDF (book, cover, front matter, notes sheet, draft watermark),
  Word and the Typst export of an English work carry English labels and generated words (contents, cover, title
  page, glossary, index, figure, footnote back links, "p." in cross references, "DRAFT", numbering words such as
  "Part I", "Chapter One", "para.", "no.", imprint lines). One book catalog in `baruch-core`
  (`crates/baruch-core/locales/{en,de}/book.ftl`, `baruch_core::book_text`). The book language tag (EPUB
  `dc:language`, DOCX `w:lang`, Typst `#set text(lang: "en")`) follows the typesetting setting, else the work
  language. German works produce byte-equal output (golden fixtures in the crate tests).
- **Read-back words:** Baruch writes the markers it reads back (scripture check, claims, web check, chat footer) in
  the work language and recognises both languages in every work.
- **Website in English:** every page of baruch.bernau.family exists in English under `/en/` (help, installation,
  operation, MCP, licence, legal notice), with a plain language switch and `hreflang` links; the English help
  shows English screenshots (`site/static/bilder/en/`). The interface menu links the help of the interface
  language.
- **Screenshot tool:** `site/tools/bildschirmfotos.py --lang de|en` takes the pictures in either language (English
  sample work "A Year on Gull Holm"); a missing light picture stops the site build in both languages.
- **Interface:** numbering schemes, typesetting fields, cover blocks, finding states, EPUB reader profiles and model
  connection kinds are translated on English pages; language form on the sign-in and setup pages; the menu form
  returns to the same place; "Revisions" instead of "Versions" in the English settings.
- **Still German (planned later):** mirror commit texts, idea-map exports, register titles stored as user data,
  exchange-format names, new works' preset texts, and the server modules `rag/**`, `multigpt.rs`, `pgvector.rs`,
  `import.rs`, `archive.rs`, `markdown_import`, `pdf_import`; WYSIWYG serialisation writes "S." in new cross
  references of an English work.

### User interface in English and German (Round 39a)

- **Two languages:** the user interface speaks English and German. Without a choice Baruch follows the browser
  (`Accept-Language`), otherwise English. The choice is made in the menu „Baruch ▾“ (English / Deutsch) or under
  Settings → Display (also „like the browser“); both are plain forms without JavaScript (`POST
  /einstellungen/sprache`). It is kept in the cookie `baruch_lang` and as a setting of the instance or, with
  accounts, of the account (Lektors too). MCP answers in English; the work language (book outputs) stays a
  separate setting per work.
- **Catalogs:** `crates/baruch-server/locales/{en,de}/*.ftl` (Fluent), 38 area files per language: templates
  (98 pages and fragments), the 47 widgets (catalog in the page head, `web/src/i18n.ts`), the messages of the
  server (notices, form errors, error pages, `/rpc` refusals) and the store messages (`store_text.rs`; `/rpc` in
  the request language, `/mcp` in English). German keeps its wording character for character.
- **Formatting:** dates (`ui_minutes`, `ui_day`) and numbers (`i18n::number`, `fmtNumber`) follow the interface
  language.
- **Still German on English pages:** labels from other crates (numbering schemes, typesetting fields, cover
  labels, finding states, EPUB reader profiles), the semantic index (`rag/**`), some import paths and connection
  messages (`multigpt.rs`, `pgvector.rs`, `import.rs`, `archive.rs`, `markdown_import`, `pdf_import`), store texts
  without a pattern; book outputs follow the work language only in Round 39b.
- **Checks:** `round39a_i18n` (catalogs, placeables, markup, unknown and orphaned keys, literal finder over
  templates and widgets against `scripts/i18n-literal-allow.txt`, resolution and switch), one test per task, and
  `round39a_pages` (every GET page in en and de: no raw key, no German letters on English pages outside a listed
  set of known exceptions). Every page is about 25–30 KB larger because of the widget catalog.

### English as the primary language (Round 38)

- **Rules for agents:** `AGENTS.md` is English, every rule unchanged in substance. Only the language rule is new:
  the primary language is English for code, comments, commits, developer docs, MCP and RPC; the user interface
  stays German until Round 39 (then German and English through catalogs); each work has its own work language
  (`de`|`en`). A new line says where the help lives (`site/content/de/hilfe/`, `site/content/de/betrieb.md`).
- **Developer docs:** `README.md` (with a note that website and user interface are German) and `docs/Betrieb.md`
  are English. File names, routes, RPC and MCP names and commands did not change; texts of the German user
  interface are quoted in German. The other developer documents followed in Round 38c (below).
- **History files:** `docs/Backlog.md`, `docs/Fahrplan.md`, `HANDOFF.md` and this file say in a head paragraph
  that new entries are written in English from Round 38 on; older entries stay German.
- **Release notes:** the text that `.github/workflows/release.yml` writes for a release is English, names the
  installation page and now also says when the macOS and Windows packages are signed.
- **Work language in the developer docs:** `docs/Architektur.md` (migration 0055, modules, the rule "work language
  is not the language of the user interface", bilingual markers), `docs/Austauschformat.md` (minor version 10,
  attribute `sprache` on `<angaben>`) and `docs/Bedienung.md` (block „Werksprache“, route
  `/einstellungen/werk/sprache`) describe it in new English sections.
- **Tests:** `round38_doku` (8 tests: translated files carry German only as quotation or code, every file of the
  task is either translated or listed as still German, head paragraph of the four history files, every rule of
  `AGENTS.md` by keyword, README note and anchors, English release notes, work language in the developer docs).

- **Work language and English interfaces:** Every work now has a work language, German or English (Einstellungen
  → Werk → Werksprache; RPC `work.language`, `work.language_set`; field `language` in `work.get`, `work.meta`,
  `work.list` and in MCP `list_works` and `outline`). Assistants answer in the work language and write their markers
  in it (`Kritisch:` or `Critical:` …); Baruch recognises the markers of both languages and writes its own notes in
  the work language. The language of the user interface does not change. The exchange format counts up to
  sub-version 10 and carries the work language (`sprache` at `<angaben>`, schema
  `docs/schema/baruch-werk-1-u10.xsd`); older files read as German works. The MCP server now speaks English:
  instructions, tool and parameter descriptions and its own error texts; RPC protocol errors are English as well.
  Error codes are unchanged; messages that the user interface also shows stay German for now. The default prompts
  of the personas are English; prompts stored by the user are untouched, and a language rule below every prompt
  fixes the answer language. Migration `0055_work_language.sql` (existing works become German).
- **Comments:** Comments in `crates/baruch-server` are being moved to English. Done in this round: `build.rs` and
  the modules `about` to `ideas` under `src/` (all of `assist`, `auth`, `connections`, `docx_export`,
  `docx_import`, `exchange`, `findings`, `idea_map`, plus `archive`, `asset_requests`, `assets`, `band`,
  `bible_research`, `bibles`, `chat_live`, `chat_render`, `consent`, `cover*`, `epub*`, `glossary*`, `guard`). Only
  comments changed – no string, identifier or behaviour (`scripts/check-comments-only.py` proves it); quoted German
  UI texts and markers stay verbatim.
- **Comments, rest (Round 38b):** All remaining comments are English: the server modules `import` to `xref`, the
  server tests, every other crate (`baruch-store`, `baruch-typst`, `baruch-core`, `baruch-gitmirror`,
  `baruch-lancedb`, `baruch-rag`, `baruch-library`, `baruch-llm`, `baruch-docx`, `baruch-epub`, `baruch-exchange`,
  `baruch-render`, `baruch-scripture`, `baruch-sword`, `baruch-wysiwyg`), the TypeScript widgets, the developer
  comments of the templates (`{# #}`), the scripts under `scripts/` and `site/tools/bildschirmfotos.py` – 801 files.
  Only comments changed; the one exception is the checksum in `crates/baruch-rag/tests/port.rs`, which pins
  `crates/baruch-rag/src/lib.rs` byte for byte and follows its translated comments. Quoted German texts of the user
  interface, commit subjects, user quotations and sample inputs stay German inside the English sentences; every
  such comment line with an umlaut has its own entry in `scripts/comment-umlaut-allow.txt` (the broad entry for
  `crates/baruch-server` is gone), so `check-comments-only.py --umlaute crates scripts site/tools` is empty. The
  check tool no longer takes two bracket characters in code spans for an intra-doc link.
- **Developer docs and model texts, rest (Round 38c):** `SKILLS.md`, `docs/Plan.md`, `docs/Verteilung.md`,
  `docs/Bedienung.md`, `docs/MCP.md`, `docs/Austauschformat.md`, `docs/WYSIWYG.md`, `docs/RPC.md` and
  `docs/Architektur.md` are English as a whole; texts of the German user interface stay quoted in German, every
  fact, table and code block is kept. New tool `scripts/check-doc-language.py` (also in `scripts/ci-selftest.sh`)
  finds German prose in a document that counts as English. The task texts Baruch sends to models around a section
  (review and revision tasks, memory, findings, chat history, fact blocks from SWORD, the semantic search and the web,
  variants, glossary, names of idea groups, the connection probe) are English; German works keep their German
  markers literally, and the language rule still fixes the answer language. Fixes: an unknown work in the form
  „Werksprache“ now gets a message (`sprachfehler=werk`) instead of a silent redirect, and an unreadable work
  language is logged before Baruch falls back to German. Still German until Round 39: every text of the user
  interface (also when it travels through RPC or MCP), the words read back from @schrift/@belege, strings, test
  names and assertion messages; the history files stay German below their head paragraph.
- **Acceptance:** `round38_abnahme` (4 tests: the work language from the form through the exchange file to a second
  instance and from an older file, English interfaces with a German user interface, language rule in the prompt
  with unchanged German behaviour, tools and records of the round).

### Hilfe auf der Webseite (Runde 37)

- **Hilfe für Anwender:** Die Webseite hat unter `/hilfe/` 14 Seiten in Alltagssprache mit Schritten zum Nachmachen
  („So geht es“): Erste Schritte · Vorhandene Texte einlesen · Menüband und Laptop-Modus · Gliederung · Schreiben,
  Bilder und Fassungen · Assistenten, Chat und Befunde · Einfälle und Mindmap · Bibel, Literatur und Quellen ·
  Suchen, Verweise, Register und Glossar · Buchsatz · Austausch, Sicherung und Sicherheit · Einstellungen · Konten
  und Lektoren · Tasten und Maus. Jede Überschrift hat einen festen Anker, jeder Abschnitt ein Bild mit alt-Text
  (alle 94 Namen der Bildliste sind verwendet). Der Einstieg nennt den
  schnellsten Weg: Text hinein, Vorschläge annehmen, PDF heraus.
- **Tasten für Windows/Linux und Mac:** `tasten.md` sammelt alle Tastenkürzel und Mausgesten in Tabellen mit beiden
  Spalten; die Mac-Tasten sind aus dem Code belegt (`ctrlKey || metaKey`, `event.code`) und als „am Mac nicht
  ausprobiert“ gekennzeichnet.
- **`docs/Bedienung.md` ist nur noch Einstieg:** Verweis auf die Hilfe, die Tabelle „Wo steht was“ (alle 44 früheren
  `##`-Überschriften → Hilfeseite mit Anker), die Regel für Entwickler („Bedienung nachziehen heißt: die Hilfeseite
  im selben Commit ändern“) und „Technische Anmerkungen“ je Hilfeseite (Routen, Cookies, Parameter, Commits,
  RPC-/MCP-Namen, Grenzen, Annahmen). Von rund 4200 auf rund 1000 Zeilen.
- **Tests:** `round37_hilfe` (11 Tests: Seiten vorhanden und ausgeschrieben, Kopf nach Vertrag, Bildnamen und
  alt-Texte, jeder gelistete Name verwendet, Anker und Bild je Abschnitt, feste Anker, Verweise, verbotene Wörter
  und Routen, Tasten mit Mac-Entsprechung, `docs/Bedienung.md` mit lückenloser Tabelle). `round24_abnahme`,
  `round32_abnahme`, `round33_abnahme` und `round33_seiten` lesen die sichtbaren Texte in der Hilfe, das Technische
  weiter in `docs/Bedienung.md`; die Verbotslisten gelten für alle Dateien unter `site/content/hilfe/`.
- **Betrieb auf der Webseite:** `/betrieb/` ist jetzt die vollständige Beschreibung für Betreiber – Datenordner,
  Sicherung und Wiederherstellen, Dauerbetrieb und Aktualisieren, Erreichbarkeit in drei Stufen, Reverse-Proxy
  mit nginx und Caddy samt Pflichtprobe, Anmeldung und Konten, `/mcp` mit Token, Verbindungen nach außen, TLS zu
  pgvector. Fachwörter sind beim ersten Vorkommen erklärt, Geprüftes und Angenommenes bleibt getrennt
  (Abschnitt „Was nicht erprobt ist“). `docs/Betrieb.md` ist nur noch Verweis, „Wo steht was“, Stand der
  Prüfung, Offenes und technische Anmerkungen.
- **Ersteinrichtung Schritt für Schritt:** Installation → „Nach der Installation“ (erster Start, Bibeln
  einrichten: woher Module kommen, wohin der Ordner gehört, wo man ihn einträgt, wie man die Bibel des Werks
  wählt; Aktualisieren). MCP und Assistenten → „Assistenten einrichten“ (Adresse, Schlüssel im Schlüsselbund,
  Verbindung prüfen, Modell je Assistent, ausprobieren), „MultiGPT einrichten“, „KI-Programme mit Baruch
  verbinden“ (eigener Rechner, eigenes Netz, Internet).
- **Startseite, Funktionen, Installation, MCP in Alltagssprache;** alle Verweise auf die alte Seite Bedienung und
  auf `docs/Bedienung.md` bzw. `docs/Betrieb.md` bei GitHub führen jetzt in die Hilfe bzw. auf `/betrieb/`. Jede
  Überschrift dieser Seiten hat einen festen Anker; die Zeile „Direkt zu“ der Installation zeigte vorher für
  macOS auf einen Anker, den es nicht gab.
- **Hilfe im Theme:** Startseite der Hilfe als Liste der Seiten mit Vorspann; jede Hilfeseite mit der Navigation
  des Abschnitts (aktuelle Seite markiert), einem Inhaltsverzeichnis aus den Überschriften und „vorige/nächste
  Seite“ – ohne JavaScript. „Hilfe“ im Hauptmenü ist auch auf den Unterseiten markiert. Stylesheet für die
  Bilder der Hilfe (nie breiter als die Spalte, Rahmen, Unterschrift), Tastenangaben, Tabellen und Navigation auf
  Telefonbreite (waagerecht rollbar), Druck. Eine Suche gibt es nicht: Sie ginge nur mit JavaScript oder über eine
  fremde Adresse.
- **Neuer Shortcode `taste`:** `{{< taste "Strg+S" "⌘S" >}}` ergibt „Strg+S (Mac: ⌘S)“.
- **Linkprüfung `scripts/site-check.py`** (nur Python-Standardbibliothek): tote interne Links und Anker, Bilder
  ohne alt-Text, JavaScript, fremde Adressen für Stil, Schrift oder Bild, Links auf `docs/Bedienung.md` und
  `docs/Betrieb.md` bei GitHub, GitHub-Links im Text der Hilfe, Weiterleitung `/bedienung/` → `/hilfe/`;
  `--selbsttest` prüft das Skript an einem erfundenen Mini-Baum (18 Zusicherungen). `scripts/ci-site.sh` ruft es
  nach dem Hugo-Bau auf; `scripts/ci-selftest.sh` hat dazu vier neue Zusicherungen.
- Menü **Baruch ▾ → Hilfe** öffnet die Hilfe auf der Webseite in einem neuen Fenster (gewöhnlicher Link, auch ohne
  JavaScript und für Lektoren; der Rechner braucht dafür das Internet – Baruch selbst ruft nichts ab). Die Seite
  „Über Baruch“ nennt die Adresse der Hilfe unter der Webseite.
- Werkzeug `site/tools/bildschirmfotos.py` neu gefasst: Die Bilder der Hilfe kommen aus der Liste
  `site/data/hilfebilder.toml` (94 Namen, je Bild Adresse, Fenster, Ausschnitt, Schritte, erwartete Texte, Stufe);
  die sieben Bilder der Seite „Funktionen“ entstehen weiter mit. Neu: `--abgleich` (verwendete Namen gegen die
  Liste, fehlende Dateien, Größe gegen 20 MB), `--nur`, `--sword`, `--breite`, `--ohne-palette`; `--pruefen` baut
  das Beispielwerk ganz auf und belegt jede Aufnahme (200, Inhalt, erwartete Texte, Wirkung der Schritte).
- Ein Vermittler im Skript (vor dem Wegwerf-Server) ergänzt Cookies und Sitzung, öffnet `<details>`, fügt Klick- und
  Mess-Skript ein und liefert Antworten von Formularen aus; je Aufnahme ein eigenes Firefox-Profil, dunkle Fassungen
  für Einträge mit `dunkel = true`, Nachbearbeitung mit ImageMagick (Ausschnitt, Breite, 256 Farben).
- Beispielwerk „Ein Jahr auf Mövenoog“ erweitert (Fußnote, Blockzitat, Liste, Bibelstelle); alles Weitere legt das
  Skript über RPC, MCP und Formulare des eigenen Servers an (zweite Fassung, gezeichnete Bilder, Register, Glossar,
  Buchangaben, Einfälle, Beiträge, Vorschlag, Gliederungsantrag, Faden, Quelle, Archiv, Konten, Befunde).
- **Bilder aufgenommen:** 94 Bilder der Hilfe, drei davon zusätzlich dunkel, und die sieben Bilder der Seite
  „Funktionen“ neu – alle vom erfundenen Beispielwerk „Ein Jahr auf Mövenoog“ auf einem Wegwerf-Server, zusammen
  4,2 MB. Jedes Bild der Hilfe ist ein Link auf die Datei in voller Größe. Die Bilder zur Bibel zeigen eine
  erzeugte „Beispielbibel“ mit einem einzigen Vers.
- **Behoben:** Das Suchfeld der Literatur (rechte Spalte und Seite „Literatur“) war ohne Suchwort mit „None“
  vorbelegt; es ist jetzt leer.
- `docs/Bedienung.md` und `docs/Betrieb.md` bleiben als Einstieg für Entwickler bestehen; die alte Seite
  `/bedienung/` der Webseite leitet auf `/hilfe/` weiter.

### Reste vor 0.9.1 (Runde 36)

- **Typst-Kommandozeile eingebaut:** `baruch-typst` (Symlink, unter Windows `baruch-typst.cmd`) bzw.
  `baruch-server typst …` setzt eine Typst-Datei zu PDF, PNG oder SVG – `compile EINGABE [AUSGABE]` mit `--root`,
  `--font-path`, `--format`, `--ppi`, dazu `fonts` und `--version` (`typst 0.15.1 (Baruch <Version>)`). Dieselbe
  Engine und dieselben Schriften wie im Server, ohne Netz und ohne Pakete; die mit `export --out` geschriebene
  Typst-Datei ergibt Byte für Byte die PDF von `export --pdf` (Werk ohne Umschlag und Titelei). `watch`, `query`
  und anderes Nicht-Unterstütztes wird ehrlich abgelehnt (Exit 2). Anders als der Export überschreibt `compile`
  eine vorhandene Ausgabedatei. Die Pakete installieren nie eine Datei namens `typst`.
- **Sicherheit:** sqlx 0.8.6 → 0.9.1; das gebündelte SQLite kommt jetzt aus libsqlite3-sys 0.40
  (RUSTSEC-2026-0459 behoben), die Ausnahme in `deny.toml` ist entfallen. Vorhandene Datenbanken öffnen unverändert
  (gleiche Prüfsummen der Migrationen, keine neue Migration). Zusammengesetztes SQL ist ausdrücklich als geprüft
  gekennzeichnet (`AssertSqlSafe`), Werte bleiben gebunden. Verlangt Rust ≥ 1.94 (gebaut wird mit 1.99.0).
- **CI:** Der Satz eines Exports wird mit dem eigenen Programm geprüft; die CI lädt kein Typst mehr.
- **Webseite:** `pages.yml` läuft zusätzlich, wenn ein Release veröffentlicht wird (auch bei einem Tag auf dem
  Commit des letzten main-Laufs); nicht ausgeführt geprüft.

#### Geändert

- `baruch-server passwort NAME` hebt die Sperre nach Fehlversuchen für das Konto auf; die Anmeldung mit dem neuen
  Passwort gelingt sofort.
- Lektoren: Einstellungen → Darstellung auch ohne freigegebenes Werk; Anzeigename des Kontos neben
  `@lektorat:<name>` auf der Arbeitsseite; Knopf „Im Werk suchen“; Umschlagbild in der Werkliste ohne Link auf die
  gesperrte Seite.
- Anmelde- und Einrichtungsseite folgen dem gewählten Thema (hell/dunkel), ohne JavaScript.
- Mindmap: Eine von Baruch ausgegebene `.mm`/`.xmind` kommt beim Einlesen mit ihrer Wolke zurück; Beschriftungen
  der Verbindungen liegen nicht mehr auf Knoten; die Bildlaufleiste steht am Fensterrand (heller Streifen im
  dunklen Thema).
- Der Zähler der Fassungen im Menüband zeigt „(+ n ausgeblendet)“.
- Eine direkt angesprochene Persona bekommt ihre eigenen offenen Befunde nur noch bei einem Prüfauftrag mit.
- Der Vorschlag für das ganze Kapitel beruht auf der gezeigten bzw. gewählten Fassung und nennt sie (`basis` in der
  Antwort von `persona.propose`).
- Nach einer Größenänderung des Fensters erscheint die Seite gleich im passenden Zustand (frühes Skript `vw.js`).
- Der Knopf „Zitat“ fügt keine Zeile „– “ mehr ein.
- Nennungen im Text: Ordnungszahlen bis „hundertste“, römische Zahlen über XXXIX hinaus.
- Löschen eines Gliederungspunkts: Der Git-Spiegel nennt auch die mitgelöschten Fassungen und Beiträge im Wortlaut.

#### Behoben

- „Sicherung angelegt“ erschien bei jedem Aufruf von `/einstellungen/daten?ok=sicherung`; jetzt nur einmal nach
  einer wirklich angelegten Sicherung.
- Token-Liste: „(früher `@mcp:…`)“ brach mitten im Namen um.

#### Neu

- Einstellungen → Daten meldet zu offene Rechte eines vorhandenen Datenordners (geändert werden sie nie).

### Strg+S speichert normal

- **Strg+S (⌘+S) schreibt fort statt eine neue Fassung anzulegen:** Die Vorgabe der Einstellung „Fassungen“ hat
  keine Größen- und keine Zeitgrenze mehr (bisher 20 % und 30 Minuten; nach jeder Pause entstand so eine neue Fassung).
  Jeder Stand bleibt im Verlauf der Fortschreibungen; die Grenzen lassen sich je Werk wieder einschalten.
- **Strg+S gilt auf der ganzen Arbeitsseite**, auch mit dem Fokus in Gliederung, Menüband oder Chat; der Dialog
  „Seite speichern“ des Browsers erscheint dort nicht mehr.
- **Nach dem Speichern bleibt man an der Stelle:** Cursor, Auswahl und Scrollstand bleiben, der Fokus ist wieder im
  Text, im Satzbild wie im Quelltext.

### Konten und Rollen (Runde 35)

- **Ohne Konten bleibt alles wie bisher:** keine Anmeldung, keine Umleitung. Neu ist nur der Eintrag „Konten
  einrichten…“ im Menü „Baruch ▾“.
- **Konten:** Das erste Konto gehört dem Autor und lässt sich nur am eigenen Rechner anlegen (`/einrichten`).
  Danach verlangen Seiten, Formulare und `/rpc` eine Anmeldung – auch am eigenen Rechner; das lässt sich unter
  Einstellungen → Konten ausschalten („Am eigenen Rechner ohne Anmeldung“). `/mcp` bleibt bei seinen Tokens.
- **Lektoren:** Der Autor legt Lektoren an (Einstellungen → Konten) und gibt ihnen einzelne Werke frei (Reiter Buch
  → Mitwirkende). Ein Lektor sieht nur freigegebene Werke und schreibt dort Vorschläge für Fassungen („Vorschlagen“
  statt „Speichern“), Beiträge im Abschnitts-Chat und Befunde bzw. Anmerkungen zum Buch – alles unter seiner
  Herkunft `@lektorat:<name>`, mit Commit im Git-Spiegel. Übernehmen, Ändern, Löschen, Personas, Export, Import und
  Einstellungen bleiben dem Autor.
- **Oberfläche:** Anmeldeseite, „Mein Konto“ (Passwort ändern, Abmelden), Einstellungsseite „Konten“ (Anlegen,
  Sperren, Passwort zurücksetzen) und die Seite „Mitwirkende“ je Werk – alles ohne JavaScript bedienbar. Im Menü
  „Baruch ▾“ steht, wer angemeldet ist.
- **Sicherheit:** Passwörter nur als Argon2id-Prüfwert, Sitzungs-Cookie `HttpOnly` und `SameSite=Strict`, Bremse
  gegen Durchprobieren. Für Lektoren ist jede Seite und jede RPC-Methode gesperrt, die nicht ausdrücklich erlaubt
  ist.
- **Bremse gegen Durchprobieren, nachgebessert:** Ein Anmeldeversuch zählt jetzt schon vor der Prüfung des Passworts
  (und wird bei Erfolg zurückgenommen). Viele gleichzeitige Versuche kommen damit nicht mehr an der Sperre vorbei.
- **Notweg:** `baruch-server passwort <Name>` setzt am Rechner ein neues Passwort.
- Noch nicht in einem echten Browser und nicht hinter einem echten Proxy mit TLS geprüft; PDF-Lesen für Lektoren
  ist offen.

### Mindmap wie XMind (Runde 34)

- **Mindmap statt Gedankenkarte:** Die Seite `/werke/<id>/karte` (Start → Einfälle → Mindmap) zeigt das Werk in der
  Mitte, Teile und Kapitel als Äste rechts und links und die Einfälle als Unteräste, beliebig tief. Jeder Hauptast
  hat seine Farbe; Äste lassen sich einklappen („−“/„+ n“). Alte Lesezeichen gelten weiter.
- **Bedienung wie gewohnt:** Tab legt einen Unterknoten an, Enter ein Geschwister, F2 oder Doppelklick benennt um,
  Entf löscht samt Unterknoten (wiederherstellbar), Ziehen hängt um oder ordnet, Pfeiltasten wandern, Leertaste
  klappt, Strg+Z nimmt die letzte Aktion zurück, Alt+L verbindet zwei Einfälle mit einer beschrifteten Linie.
- **Einfälle als Baum:** Ein Einfall hat jetzt einen Titel, kann unter einem anderen Einfall hängen und hat eine
  Reihenfolge unter seinen Geschwistern. Titel oder Schlagwort genügt.
- **Wolke und Mindmap verbunden:** Einfälle ohne Platz liegen in der Wolke unter dem Baum („n in der Wolke“). Von
  dort zieht man sie auf einen Knoten, auf ein Kapitel oder aufs Werk (freies Hauptthema) – und zurück, samt
  Unterknoten. Wo die Nähe einen passenden Einfall kennt, steht am Kärtchen „passt zu Ast …“. Bestehende Einfälle
  ohne Bezug liegen nach der Umstellung in der Wolke; Verbindungen und gespeicherte Lagen bleiben erhalten (Lagen
  haben in der Mindmap keine Wirkung mehr).
- **Ohne JavaScript:** Die Gliederungsansicht (`/werke/<id>/karte/gliederung`) zeigt dieselbe Mindmap als
  verschachtelte Liste mit Formularen für alle Aktionen.
- **Austausch mit XMind, FreeMind und Freeplane:** Ausgabe als `.xmind`, `.mm`, SVG, PDF und Markdown; Einlesen von
  `.xmind` und `.mm` mit Vorschau (Titel, Notizen, Schlagwörter, Reihenfolge, Beziehungen). In echtem XMind und
  Freeplane ist nichts geprüft – der Aufbau der XMind-Datei folgt der Formatbeschreibung aus dem Gedächtnis.
- **RPC und MCP:** `idea.move`/`idea_move` und `idea.tree`/`idea_tree` neu; `idea.create`/`idea_create` mit `title`,
  `parent`, `before`, `cloud`; Löschen nimmt die Unterknoten mit. Jede Aktion ist ein Commit mit Herkunft.
- **Austauschformat Unterversion 9** (`titel`, `eltern`, `platz`, `hauptthema` am Einfall); ältere Dateien bleiben
  lesbar. Migration 0052 (nur neue Spalten).

### Word und Beenden (Runde 32)

- **Word-Import:** „Baruch ▾ → Word (.docx) importieren…“ liest eine Word-Datei, zeigt Gliederung, Bearbeiter und
  Hinweise und legt nach Bestätigung ein Werk an. Fassung 1 ist der Stand vor allen verfolgten Änderungen; die
  Änderungen jedes Bearbeiters werden offene Vorschläge unter `@word:<name>`, Kommentare werden Beiträge an der
  kommentierten Stelle – nie eine aktuelle Fassung. Auch über RPC (`docx_import.preview/get/apply/discard`) und die
  Kommandozeile (`baruch-server import DATEI.docx [--uebernehmen]`); nicht über MCP.
- **Word-Export für den Verlag:** Buch → Export → „Word …“ (`/werke/<id>/word`), RPC `work.docx_export` und
  `baruch-server export ID|SLUG --docx DATEI [--vorschlaege]` schreiben das Werk als .docx: Titelei aus den
  Werkangaben, Formatvorlagen „Überschrift 1–3“, „Baruch Teil“ und „Baruch ohne Nummer“, echte Fußnoten, eingebettete
  Bilder mit Alternativtext, Listen, Blockzitate und einfache Tabellen. Was Word nicht abbilden kann, steht als
  Typst-Insel mit wörtlichem Quelltext in der Datei und wird als Warnung genannt. Auf Wunsch liegt je Abschnitt der
  jüngste offene Vorschlag als Änderungsverfolgung über der aktuellen Fassung, mit der Persona als Bearbeiter. Der
  Export ändert nichts am Werk. Das Öffnen in Word prüft nur der Nutzer; mit LibreOffice ist nur eine Umwandlung
  ohne Fenster geprüft.
- **Baruch beenden:** Menü Baruch ▾ → „Baruch beenden…“ mit Rückfrage (`/beenden`, auch ohne JavaScript). Nur am
  eigenen Rechner, nie unter einem Proxy-Namen, nicht über RPC oder MCP. Danach „Baruch ist beendet – dieses
  Fenster kann geschlossen werden“; das Fenster schließt sich, wenn der Browser es zulässt.
- **Sauberes Herunterfahren** auch auf `SIGTERM` (`systemctl stop`, `kill`) und unter Windows auf Strg+Pause,
  das Schließen des Fensters und das Herunterfahren (Windows ungeprüft): laufende Antworten gehen zu Ende
  (höchstens 10 s), die Datenbank wird geschlossen, letzte Zeile „Baruch beendet.“, Status 0. Bisher nur Strg+C.
- **Starter:** öffnen, wenn vorhanden, ein eigenes Fenster eines Chromium-artigen Browsers (`--app`), unter
  Windows Edge; `BARUCH_BROWSER=standard` nimmt den Standardbrowser. Der Starter wartet nach einem Signal auf
  das saubere Ende des Servers.
- **CI:** Webseite wird in zwei Jobs gebaut (`pages-build`) und ausgeliefert (`pages-deploy`).
- **Rundreise:** Eine von Baruch geschriebene Word-Datei liest der Word-Import mit derselben Gliederung und
  demselben Text wieder ein; eine Datei mit Änderungsverfolgung ergibt wieder aktuelle Fassung und offenen
  Vorschlag. Listen, die nicht bei 1 beginnen, behalten beim Import ihre Nummern („3.“, „4.“ …).

## 0.9.0

Erste Version mit fertigen Paketen. Sie fasst die gesamte bisherige Arbeit zusammen (Runden 1 bis 24, begonnen
am 2026-11-05); davor gab es nur den Stand 0.0.1 aus dem Quelltext.

### Verteilung (Runde 24)

- **Pakete:** `.deb` für Debian und Ubuntu (amd64), AppImage (x86_64), `.dmg` für macOS (Apple Silicon und
  Intel) und `.msi` für Windows (x64). Die Pakete für macOS und Windows sind unsigniert und entstehen nur in der
  CI. Jedes Paket enthält die Lizenz, die Lizenzen aller eingebundenen Bibliotheken
  (`THIRD-PARTY-LICENSES.txt`) und den Hinweis auf den Quelltext.
- **APT-Repository** unter `https://baruch.bernau.family/apt/` (Suite `stable`, Komponente `main`); neue
  Versionen kommen dort mit `sudo apt upgrade`.
- **Seite „Über Baruch“** (Baruch ▾ → Über Baruch, auch von Einstellungen → Daten aus): Version, Lizenz mit
  Lizenztext, Quellcode, Webseite, Ort der Drittlizenzen je Paket, Datenordner.
- **Keine Aktualisierungsprüfung:** Baruch fragt nie von sich aus im Netz nach neuen Versionen. Die Über-Seite
  verlinkt die Release-Seite; aktualisiert wird über APT oder von Hand.
- **Daten bleiben beim Benutzer:** Kein Paket legt etwas im Datenordner an, und das Entfernen löscht dort
  nichts. Die systemd-Unit liegt dem `.deb` nur als Beispiel bei; ein Systemdienst wird nicht eingerichtet.
- **Schlüsselbund je System:** unter Linux der Secret Service, unter macOS der Schlüsselbund, unter Windows die
  Anmeldeinformationsverwaltung (die beiden letzten gebaut, aber noch nicht auf einem echten Gerät erprobt).
- **Kommandozeile:** `baruch-server --version`; für den Betrieb hinter einem Reverse-Proxy `--allow-host NAME`
  bzw. `BARUCH_ALLOWED_HOSTS`. Unter einem zusätzlichen Namen gilt eine Anfrage nie als lokal (MCP nur mit
  Token).
- **MultiGPT ohne Vorgabe-Adresse:** Eine neue Instanz hat keine voreingestellte MultiGPT-Adresse mehr; eine
  gespeicherte Verbindung bleibt unverändert.
- **CI und Webseite:** Tests, `clippy` und Lizenzprüfung (`cargo deny`) in GitHub Actions; ein Release-Ablauf
  baut die Pakete, das APT-Repository und die Webseite `https://baruch.bernau.family/`. Signiert wird nur in
  einer geschützten Umgebung und nur, wenn der veröffentlichte Fingerabdruck zum Archivschlüssel passt.
- **Beispielwerk:** Eine neue Instanz bekommt das frei erfundene „Beispielwerk: Leise Abhängigkeit (Auszug)“
  von „Erika Mustermann“. Vorhandene Werke ändern sich nicht.

- **Abhängigkeiten (Sicherheitsmeldungen):** quick-xml 0.38 → 0.42, zip 2.4 → 9.8, url 2.5.8 → 2.5.9; verbleibende
  RustSec-Meldungen ohne verfügbaren Fix (quick-xml über citationberg, libsqlite3-sys über sqlx 0.8, chrono)
  sind in `deny.toml` begründet ausgenommen.

### Werke, Gliederung und Fassungen

- Vor Runde 1 (Umbau): lokaler Server (axum) mit Seiten, die auch ohne JavaScript bedienbar sind, und
  TypeScript-Widgets statt einer Oberfläche in Iced.
- Runde 1: SQLite als Quelle aller Daten, Git-Spiegel je Werk, Autorenbibliothek mit Volltextsuche (FTS5).
- Runde 2: MCP-Server mit echten Werkzeugen, Import und Export von Typst, mehrere Werke nebeneinander.
- Runde 3: Fassungen als Baum – genau eine ist aktuell, jederzeit umschaltbar; Literatur als Popup,
  Hervorhebungen.
- Runde 4: Werke anlegen, ändern und löschen, Import per Hochladen, eindeutige Titel, Gliederungsmanager.
- Runde 5: eine Gliederung als Baum, ein Klick öffnet den Editor im Satzbild.
- Runde 6: Gliederungsschemata (Dezimal, römisch, Predigt, Buch, eigenes Muster …), Bilder.
- Runde 8 (Nachgang): Gliederung über MCP – Anlegen sofort, Umbenennen, Verschieben, Umstufen und Löschen nur
  als Antrag, den der Mensch bestätigt.
- Runde 14 (mit Nachgang): Teile als eigene Ebene, „ohne Nummer“, Markdown-Überschriften beim Tippen als
  Gliederung; Zeitangaben in Ortszeit.
- Runde 22: Fortschreiben der eigenen Fassung bei kleinen Änderungen (statt jedes Mal einer neuen Fassung) mit
  Verlauf und Zurücknehmen; Archiv gelöschter Werke; Entwurf als Wasserzeichen.
- Ältere Fassungen ausblenden: an jeder Fassung „bis hierhin ausblenden“ (Akkordeon „Fassungen“, Editor-Kopf); ersetzte
  und verworfene Fassungen darunter stehen hinter dem Schalter „n ältere Fassungen zeigen ▸ / verbergen ▾“, der Zähler
  zeigt „2/3 (+ 24 ausgeblendet)“ und ‹ › überspringt sie. Die aktuelle Fassung und offene Vorschläge bleiben immer
  sichtbar, nichts wird gelöscht; „Wieder alle zeigen“ hebt die Marke auf. Werkweit unter Einstellungen → Werk →
  Fassungen „Ältere Fassungen ausblenden bis …“ (Datum). Nur der Mensch, nicht über MCP; Migration 0051.

### Schreiben

- Runde 5 bis 8: Satzbild-Editor (Absätze, Überschriften, Szenen, Bilder, Fußnoten) mit verlustfreier
  Rundreise nach Typst; Vorschläge der Assistenten an ihrer Stelle im Lesefluss, in der Farbe der Persona.
- Runde 7: Einfälle mit Schlagwörtern, frei oder an einem Abschnitt, auch über MCP.
- Runde 9: eine Kopfzeile mit den Menüs Werk, Ansicht und Werkzeuge; Einstellungen als Popup und als Seiten
  ohne JavaScript; Verbindungen mit Schlüsselbund.
- Runde 18: Menüband mit den Formaten der Werkvorlage, Listen, Begriffslisten, Links, „Satz abbrechen“.
- Runde 25: Gedankenkarte der Einfälle mit Verbindungen, Nähe und Ausgabe als SVG, PDF, Markdown und FreeMind.
- Runde 30: Suchen und Ersetzen im Abschnitt und im ganzen Werk (auch mit regulären Ausdrücken, mit
  Zurücknehmen), Querverweise mit festen Sprungmarken.
- Runde 31: Stichwort-, Personen- und Bibelstellenregister, Glossar, Kontextmenü im Text.

### Assistenten

- Runde 10: Personas rufen wirklich ein Modell auf (OpenAI-kompatibel, gestreamt, mit Abbrechen); Prüfen,
  Vorschlagen und Fragen im Abschnitts-Chat; MCP `ask_persona` und `chat`.
- Runde 15: der Lektor liest ganze Kapitel (bei Überlänge überlappend gestückelt, nie still gekürzt), eigene
  Verbindung je Persona (auch Ollama; externe Anbieter nur nach Freigabe), Fäden ohne Abschnitt.
- Runde 28: Befunde als eigene Datensätze mit Status (übernommen, verworfen, verschoben, erledigt), Übersicht
  übers Werk, Aufträge im Chat (`@alle`, RFC), Beiträge kopieren, zitieren und wiederholen.
- Runde 30: Lösungen statt nur Kritik („statt … / besser …“), Varianten nebeneinander vergleichen.
- Runde 22: Personas fragen nach, ob ein Befund erledigt ist.
- Fehler behoben (Kritik-Zyklus): Ein Kritiker kann den Vorschlag des Lektors gegenprüfen und der Lektor darauf
  weiterschreiben – die Prüfgrundlage bietet die offenen Vorschläge an, „@persona …“ im Chat prüft die eigenen offenen
  Befunde nach, und ein Vorschlag beruht auf der gezeigten bzw. gewählten Fassung statt stets auf der aktuellen.

### Bibel und Predigt

- SWORD (neben Runde 11): lokale SWORD-Bibeln lesen, Bibel je Werk, Wortlaut von Zitaten prüfen.
- Runde 16: Predigtvorlage, Sprechdauer und Zielzeit, Stichwortzettel als A5-Blatt oder Karteikarten.
- Runde 15: SWORD-Kommentare und -Lexika zum Nachschlagen.
- Runde 17: alle 18 Verszählungen des SWORD-Projekts mit Umrechnung, Stellen in Listen und Sätzen erkennen
  („Psalm 139 … Vers 23“), komprimierte und verschlüsselte Module, Prüfanzeige der Zitate im Editor.
- Runde 19: Wortlautprüfung auch bei Trennresten im Modul, Bibelseite in anderer Zählung.

### Literatur und Suche

- Runde 12: Bedeutungssuche (RAG) mit lokalem Vektorindex (LanceDB), wahlweise pgvector und MultiGPT als
  weitere Wege.
- Runde 13: Dokumente aus MultiGPT lesen und übernehmen, Websuche mit Belegen, Abgleich nur auf Knopfdruck.
- Runde 19: Pflege des Index ohne lange Sperre, ehrlicher Status, schwache Treffer gekennzeichnet.

### Satz, Buch und E-Book

- Runde 8: Typst-Satz mit der Werkvorlage als Vorschau.
- Runde 11: Umschlag (Vorderseite, Rücken aus der Seitenzahl, Rückseite mit EAN-13), Titelei, Buch als PDF.
- Runde 20: EPUB 3 mit Vorschau in neun Reader-Profilen, Strukturprüfung und EPUBCheck (wenn installiert).
- Runde 27: Umschlag-Editor mit frei verschiebbaren Blöcken, Hilfslinien und Druckprüfung; Impressum in der
  Werkvorlage.
- Runde 29: Buchsatz in den Einstellungen (Kapitelanfang, Seitenformat, Ränder, Schrift, Kolumnentitel,
  Schusterjungen und Hurenkinder) mit Vorschau.

### Import, Export und Austausch

- Runde 21: Import von Markdown (auch als ZIP mit Bildern), PDF und EPUB – immer mit Vorschau der Gliederung,
  angelegt erst nach Bestätigung.
- Runde 26: Austauschformat `.baruch` (XML im ZIP) zwischen Instanzen; Einlesen als neues Werk oder als
  Abgleich, bei dem nichts gelöscht oder überschrieben wird.

### Sicherheit und Härtung

- Der Server antwortet nur unter `127.0.0.1`, `[::1]` und `localhost` (seit Runde 24 zusätzlich unter
  ausdrücklich erlaubten Namen); schreibende Aufrufe nimmt er nur von der eigenen Seite an.
- Runde 9: Schlüssel und Token nur im Schlüsselbund des Systems, nie in Dateien.
- Runde 23: Härtung nach Gutachten – Zitat-Werkzeug, Autorennamen und Kontingente der MCP-Clients, Schutz der
  Geheimnisse, strengere Grenzen beim Abruf fremder Seiten.
- Eingelesene Dateien werden mit Grenzen für Größe, Tiefe und entpackte Bytes geprüft (Runden 17, 21, 26).

### Menüband und Laptop-Modus (Runde 33)

- **Menüband:** Die Menüs „Werk“, „Ansicht“, „Werkzeuge“ und die Knopfreihe über dem Blatt sind ein Menüband mit dem
  Menü **Baruch ▾** und den Reitern **Start, Einfügen, Prüfen, Buch, Ansicht**. Jede Funktion bleibt ohne JavaScript
  erreichbar (Reiter als Links `?tab=`, Gruppen als aufklappbare Bereiche).
- **Band ein- und ausklappen:** Ein Klick auf den gewählten Reiter oder auf ▴ klappt das Band zu (gemerkt im Cookie
  `baruch_band`); im zugeklappten Band zeigt ein Reiter seine Gruppen vorübergehend.
- **Abzeichen an den Reitern:** Start zeigt die offenen Vorschläge, Prüfen „N offen“ (Befunde), Buch die offenen
  Anträge zur Gliederung.
- **Seiten im Menüband:** Jede Seite öffnet mit ihrem Reiter (Satz, Umschlag, EPUB, Austausch, Glossar, Register,
  Bilder, Gliederungsschema, Werk bearbeiten: Buch; Befunde, Zitate prüfen, Suche, Querverweise, Literatur, Abgleich:
  Prüfen; Einstellungen: Ansicht; sonst Start). Die Links der Seite stehen dort als benannte Gruppen.
- **Satz-Seite:** „Zurück zum Werk“, „PDF herunterladen“, ‹ Seite n von m › und „Satz abbrechen“ stehen im Reiter
  Buch statt in einem eigenen Band unter der Kopfzeile.
- **Laptop-Modus:** Ansicht → Laptop-Modus „Automatisch / An / Aus“. Automatisch wird das Band unter 1600 px
  Fensterbreite kompakt (Gruppen als Knöpfe mit Popup), unter 1280 px klappen Gliederung und Assistenten zu schmalen
  Leisten ein; ein Klick auf die Leiste holt die Spalte als Schublade über den Text (Esc, Klick daneben oder
  „Schließen“ legt sie weg).
- **Seitenspalten einzeln einklappen** („Einklappen“ in der Spalte oder Ansicht → Seitenleisten); die Wahl wird
  gemerkt. Ansicht → **Zoom zurücksetzen** setzt alle Bereiche auf 100 %.
- **Ohne JavaScript:** Schalter als Links, Schublade über ein Sprungziel. Die Fensterbreite kennt der Server dann
  nicht; den Laptop-Modus stellt man von Hand „An“.
- **Umbenannt:** „Satz“ heißt „Satz-Vorschau“; „Als PDF/Entwurf/EPUB herunterladen“ heißen in der Gruppe Ausgabe
  „PDF“, „Entwurf“, „EPUB“; „Editor“ außerhalb der Arbeitsseite heißt „Zum Text“.
- **Geändert:** Unter 900 px Fensterbreite stehen die Spalten nicht mehr untereinander, sondern werden Leisten.
  Alle Seiten unter dem Menüband haben denselben Seitenrand.
- **Webseite:** Das Bildschirmfoto-Skript kennt eine Fenstergröße je Bild und zeigt die Arbeitsseite zusätzlich auf
  Laptop-Breite (1366×768).

### Bekannte Grenzen

- Die Pakete für macOS und Windows sind unsigniert und wurden noch auf keinem echten Gerät erprobt.
- Kein Paket für arm64 unter Linux; das `.deb` braucht glibc 2.39 oder neuer (Debian 13, Ubuntu 24.04).
- Baruch hat keine Anmeldung: Der Betrieb hinter einem Reverse-Proxy braucht dort einen Zugangsschutz.
- Bibeltexte, Kommentare und Literatur bringt Baruch nicht mit.
