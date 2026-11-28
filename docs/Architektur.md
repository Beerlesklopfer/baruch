# Aufbau von Baruch

Cargo-Workspace unter `crates/`. Die Oberfläche liefert `baruch-server` (axum + minijinja) als HTML,
der Kern ist über JSON-RPC erreichbar. Plan und Stufen: `docs/Plan.md`.

## Crates

| Crate | Aufgabe |
|---|---|
| `baruch-core` | Typen (Werk, Abschnitt, Revision, Persona, Beitrag mit `id` und `created`), Zugriff auf Revisionen über die Nummer, Umschalten (`Node::set_current`), Lektor-Wahl (`LectorChoice { Auto, Fixed(handle) }`, Vorgabe fest `@lektor`, `Work::lector_choice`, `LectorChoice::pick`), Zeilen-Diff, Beispieldaten |
| `baruch-llm` | Modell-Backend Eliezer (Runde 10), rein (keine DB, kein Schlüsselbund; reqwest 0.12 mit rustls, `stream`; tokio; futures-util): `Endpoint {base_url, secret}` (`Debug` ohne Schlüssel), `models`/`models_with` (`GET /v1/models`), `chat_stream`/`chat_stream_with` (`POST /v1/chat/completions` mit `stream: true`, `stream_options.include_usage`, SSE-Parser `sse::Parser`), `Limits`, `LlmError` (acht Arten wie `assist::ModelFailure`), `Queue` (eine Anfrage zur Zeit, höchstens 8 Wartende, `QueueStatus`), `alias`/`ALIASES`, `mock::MockEliezer` (Nachbau für Tests); Runde 12: `embed.rs` (`embeddings`/`embeddings_with`, `POST /v1/embeddings`, Antwort nach `data[].index` geordnet; `embedding_alias` `nomic-embed-text` → `text-embedding-nomic-embed-text-v1.5`, getrennt von den Chat-Aliassen; `Endpoint::embeddings_url`), der Nachbau beantwortet Einbettungen deterministisch (`mock_vector`, 512 Dimensionen, `set_embedding_models`/`_dimension`/`_delay`, `fail_embeddings`, `embed_max_active`) |
| `baruch-rag` | Gemeinsame Schnittstelle der RAG-Wege (Runde 12, Vertrag Anhang A, wörtlich): `Way`, `SearchRequest`, `Query`, `Hit` (`identity` = Dokument + Stelle), `MergedHit`, `WayState`/`WayReport`, `Answer`, `IndexInfo`, `Status`, `RagError` (neun Arten, `is_off`), Ports `SearchWay`, `Embedder`, `EmbedderFactory` (ohne `async-trait`, `BoxFuture`), `clean_snippet`, Grenzen `MAX_LIMIT` 50, `MAX_QUERY_CHARS` 1000, `MAX_SNIPPET_CHARS` 600, `MAX_EMBED_BATCH` 32, `RRF_K` 60. Rein (serde, serde_json) |
| `baruch-lancedb` | Lokaler Vektorindex (Runde 12) über die Crate `lancedb` 0.41 (Apache-2.0; Arrow 58 nur über deren Re-Exporte, ohne Cloud-Speicher, `remote` und eingebaute Einbetter; zum Bauen braucht `lance` `protoc`), Tabelle `chunks` je Ordner, `baruch-index.json` (Modell, Dimension, Aufbau), flache Kosinus-Suche, eigene kleine Tokio-Laufzeit (`runtime()`), Stücke (`chunk`: Absätze, kurze zusammen, lange an Satzenden geteilt, SHA-256). Rein: keine SQLite, kein Netz |
| `baruch-scripture` | Bibelstellen (Runde 10), rein (std + serde): 66 Bücher mit Schlachter-Namen und Abkürzungen (`books`, `find_book`), Verszahlen je Kapitel aus `data/versification.tsv` (nur Zahlen), `parse` (Angaben), `scan` (Fließtext), `Passage`, `RefStatus`, `Found`, `VerseText`/`Wording`/`NoBible` (keine Bibel gewählt: „ungeprüft“), `SOURCE`; seit 2026-11-06 `versification` (German, KJV; `data/versification-kjv.tsv`) und `wording` (Zitate finden, normalisiert vergleichen, Wort-Diff, getrennte Ziffern zusammensetzen); seit Runde 17a alle 18 Zählungen von `sword-modules` (nur dessen Daten, `default-features = false`) und `versification::mapping` (Abbildung über `data/versification-map.tsv`) |
| `baruch-sword` | SWORD-Bibelmodule lesen (2026-11-06): Modulordner durchsuchen (`scan`, conf-Angaben samt Lizenzvermerk), `open` → `Bible` (`verse`, `passage`, `search`), Klartext aus OSIS/ThML/GBF (`text`), Test-Module (`fixture`); Format-Leser `sword-modules` 0.1.0 (CC0-1.0), nur German/KJV, siehe „SWORD-Bibeln“ |
| `baruch-typst` | Zerlegen einer Typst-Datei in Werkvorlage und Überschriften (`parse`) und zurück (`join`), Nummerierung aus der Vorlage (`heading_numbering`), Gliederungsschemata (`scheme`), Bildpfade (`images::image_paths`; berechnete Bilder der Vorlage `images::computed_images`, Runde 11), Umschlag und Titelei (`cover`, Runde 11: `frame_parts`, `wrap_source`, `isbn13`, `ean13_svg`, `spine_mm`), Verketten mit Bytebereichen (`join_with_offsets`) und Zeile wie Typst (`line_at`); Runde 14: Teil und „ohne Nummer“ als Markierung (`marker`: `part_line`, `unnumbered_line`, `relevel_line`, `set_numbered_line`, `part_page_rule`, `PART_LABEL`, `UNNUMBERED_LABEL`; `Heading.level` 0 = Teil, `Heading.numbered`); Markdown-Überschriften (`markdown`: `markdown_headings`, `markdown_heading_line`, Nutzer 2026-11-07, gleicher Scanner wie `parse`), rein, ohne DB |
| `baruch-wysiwyg` | Satzbild-Editor, rein, ohne DB und Dateisystem: Abschnittstext ↔ Blöcke (`parse`, `serialize`, `emit`; unberührte Blöcke wörtlich, Rundreise byte-gleich; eine Überschrift endet am ersten Zeilenumbruch, `split_heading`), Typst-Inseln, HTML für Satzbild und Diff (`render_html`, `diff_html`), mit Bildern in Inseln (`render_html_with`, `diff_html_with`, `image_path`, `ImageRef`), Darstellung aus der Werkvorlage (`sheet_style`, `sheet_css`, nur Regeln der obersten Ebene), Ausrichtung von Vorschlägen an Blöcken (`src/align.rs`: `align(base, proposal) -> Vec<Hunk>` über die längste gemeinsame Teilfolge der obersten Blöcke, `Hunk {kind: Changed\|Inserted\|Removed, base, proposal}`, `hunk_html_with`); Runde 14: Blockarten `part` und `unnumbered` für die Markierungen (nur oberste Ebene, erkannt über `baruch_typst::parse`); hängt nur an `baruch-typst` (`heading_line`, `part_line`, `unnumbered_line`) |
| `baruch-docx` | Word-Dateien (Runde 32), rein, ohne DB und Netz (`zip` 9 nur `deflate`, `quick-xml` 0.42, `baruch-typst`, `baruch-wysiwyg`): gemeinsames Modell (`model.rs`: `DocxDocument`, `Block`, `Inline`, `Change`, `Comment`, `DocxImage`), `DocxError`, `Limits`, festgelegte OOXML-Namen (`ooxml.rs`), Beispiel (`sample.rs`); Leser `read` → `resolve`/`proposal_view` → `to_typst`; Schreiber `from_typst` → `diff::track_changes` → `write`. Nahtstelle beider Seiten: `tests/roundtrip.rs` |
| `baruch-render` | Typst-Satz (Runde 8), rein, ohne DB und Netz: bettet `typst` 0.15.1 ein (`typst-svg`, `typst-layout` für `PagedDocument`, `typst-kit` nur mit `embedded-fonts` und `scan-fonts`; keine Pakete). `Source {main, files}`, `stand(&Source)` (SHA-256 über Text und nach Pfad sortierte Dateien = Cache-Schlüssel), `compile(&Source) -> Rendered {stand, pages: Vec<SVG>, headings: [HeadingPos {line, page}], diagnostics: [Diagnostic {severity, message, hints, line, column, file}]}` (blockierend, scheitert nie). Die Welt kennt nur `/main.typ` und die Dateien in Normalform; Paket-Ids → Fehler „offline“; Schriften einmal je Prozess (eingebettete, dann System); bekannte Meldungen auf Deutsch; nach jedem Lauf `comemo::evict(2)`. Runde 11: `compile_pdf(&Source) -> Pdf` (`typst-pdf` 0.15.1, Apache-2.0; dieselbe Welt, gleiche Seitenzahl), Druckprüfung `Rendered.print`/`Pdf.print` (`PrintWarning`, `MIN_DPI` = 300), `sha256_hex` |
| `baruch-store` | SQLite als Quelle (sqlx, Laufzeit-API, Migrationen 0001–0099): Laden, Übernehmen, Umschalten, Verwerfen, Speichern (auf einer Basis), Ändern, Löschen, Vorschlag, Beitrag, Typst-Import – je eine Transaktion (`BEGIN IMMEDIATE`); Revisionsbaum (`src/tree.rs`); Titelregeln (`check_title`); Bilder (`src/assets.rs`, 0009, `normalize_asset_path`); Personas mit Farben, Rolle, Modell, Kontext (`src/personas.rs`, 0010; `PersonaRow`, `PersonaEdit`, `persona_list`, `update_persona`); Verbindungen, RAG-Wege, bevorzugte Quellen (`src/connections.rs`, 0013); Einfälle (`src/ideas.rs`, 0011); Beiträge verwalten (`src/posts.rs`: `PostInfo`, `post_get`/`post_delete`/`post_move`/`post_to_idea`, Umwandlung in einer Transaktion über `ideas::create_in`; `StoreError::UnknownPost`; keine Migration); Schnappschuss für Git, Typst-Export, Export-Karte (`src/export_map.rs`: `ExportMap {typst, sections: [ExportSection {key, seq, start, end, first_line}]}`, `typst` byte-gleich mit `export_typst`, beide aus `snapshot::export_parts`; `section_at_line`); Titelprüfung nur für geänderte Titel (`check_chunk_titles(chunks, old_title)`); Runde 11: Buchangaben und Umschlag (`src/book.rs`, 0017), Anträge zu Bildern (`src/asset_requests.rs`, 0018), `asset_search`, `save_persona`; Runde 12: Sammlungen je Werk und Weg (`src/rag_collections.rs`, 0031: `work_collections`, `set_work_collections`), RAG-Belege je Persona (`src/persona_rag.rs`, 0032: `persona_rag`, `set_persona_rag`) |
| `baruch-gitmirror` | Git-Spiegel mit gix (reines Rust, ohne Netzwerk): Schnappschuss-Commit mit Autor, Committer, Trailern; kein Commit ohne Änderung, außer mit `allow_empty` (Protokoll); keine Schreibzugriffe durch symbolische Verknüpfungen |
| `baruch-library` | Autorenbibliothek (`SearchHit::node_key` = fester Schlüssel des Abschnitts): Titel, Zitate, Hervorhebungen (`src/highlights.rs`, 0103), FTS5-Index `search_fts` über Revisionen, Beiträge, Titel, Zitate und Hervorhebungen, der geänderten und gelöschten Revisionen (0104) sowie gelöschten und verschobenen Beiträgen (0105) folgt (Migrationen 0101–0199) |
| `baruch-server` | HTTP-Server: Seiten, Fragmente, Widgets, Literatur als Popup (`/literatur`, `/quelle`), Einfälle (`/einfaelle`), Kopfzeile mit Menüs (`menu.rs`), Einstellungen als Popup (`/einstellungen/<seite>`, `settings/`), Verbindungen und Quellen (`connections/`), Satz (`/werke/{id}/satz`), Personas mit Modellaufruf (`assist/`, `models.rs`), Stellenprüfung (`scripture.rs`), MCP-Tokens (`mcp/tokens.rs`), Bedeutungssuche (`rag/`, `search_hooks.rs`, `connections/{pgvector,multigpt}.rs`, Runde 12), `POST /rpc`, `POST /act`, `POST /literatur/act`, `POST /beitraege/act`, `POST /assistenten/act`, `POST /assistenten/stream`, `POST /mcp` |

## Server (`crates/baruch-server/src`)

- `lib.rs`: `AppState` (Store, Datenordner, Vorgabewerk, Spiegel je Werk, Schreibsperre, Vorlagen,
  Geheimnisse, seit Runde 10 `model: Arc<dyn assist::ModelPort>`), Routen, `/act`. `AppState::open` und
  `open_with_secrets` nehmen `assist::NoModel`, `open_with(data, secrets, model)` ein eigenes Backend; `serve`
  setzt `assist::eliezer::EliezerPort` ein. Start in fester Reihenfolge: `Store::open` → `baruch_library::migrate` → `seed_if_empty`
  → Spiegel **aller** Werke öffnen und auf den DB-Stand nachführen (Fehler nur auf stderr).
  `AppState::mirror(work)` öffnet einen Spiegel beim ersten Gebrauch und merkt ihn sich; nach einem
  Fehler wird er beim nächsten Mal neu geöffnet. `mirror_dir` prüft den Ordnernamen
  (`^[a-z0-9]+(-[a-z0-9]+)*$`, höchstens 48 Zeichen) und lehnt `works/` bzw. `works/<slug>` als
  symbolische Verknüpfung ab.
- `localtime.rs` (Nutzer 2026-11-07): Zeitpunkte der Oberfläche in **Ortszeit** der Maschine (`chrono::Local`, unter
  Unix `TZ` bzw. `/etc/localtime`, Sommerzeit je Zeitpunkt). Gespeichert wird weiter UTC
  (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`, `models::iso_now`, Unix-Sekunden), RPC-Felder bleiben UTC; umgerechnet
  wird erst beim Aufbau der Seite bzw. eines reinen Anzeigetexts (Fassungen, Einfälle, Bilder, Werke, Verbindungs-
  prüfung, Abgleich, Prüfzeit von Hervorhebungen, MCP-Clients und Tokens, Abrufdatum der Websuche in Belegen und
  Quellenzeile, „lokale Kopie vom …“, Ordnername im Papierkorb). Jede Funktion gibt es als `…_in(…, &zone)`; Tests
  rechnen mit `FixedOffset` und prüfen `Local` in einem Kindprozess mit `TZ=Europe/Berlin` (keine Umgebungsvariable im
  gemeinsamen Testprozess). Unlesbare Stempel bleiben, wie sie sind.
- `import.rs`: Typst-Datei als neues Werk für die CLI `baruch-server import [--title …]` (`import_typst`,
  `import_file`, `import_file_as`) und das Hochladen (`import_upload`, nur `.typ`, höchstens 16 MiB): unter der
  Schreibsperre `Store::import_typst_titled`, danach Commit „Import aus <datei>“ im neuen Spiegel.
- `works.rs`: Werkverwaltung (siehe „Werke verwalten“): RPC `work.list/meta/create/import/update/delete`
  (aus `rpc::call` vor der Wahl des Vorgabewerks), Seiten `/werke…` mit eigener Vorlagen-Umgebung
  (`works.html`, `work_new.html`, `work_edit.html`, `work_delete.html`, `fragments/work_new.html`), Formulare
  `POST /werke/import` (multipart, `DefaultBodyLimit` nur dort 16 MiB + 64 KiB) und `POST /werke/act`, Archiv
  gelöschter Spiegel (`move_to_trash`). `works::RPC_BODY_LIMIT` (= 2 × 16 MiB + 64 KiB) ist die Grenze für
  `POST /rpc`: `work.import` trägt den Text als JSON-String, und die Maskierung kann ihn verdoppeln; bis 16 MiB
  Text nimmt `work.import` an (darüber -32602), eine größere Anfrage lehnt axum vorher mit 413 ab. Nicht über MCP. Die Seite `/` wählt das Werk aus `?work=`, sonst aus dem Cookie
  `baruch_work` (wenn es das Werk gibt), sonst `AppState::default_work()`, und setzt den Cookie bei jeder Antwort.
- `rpc.rs`: JSON-RPC-Methoden (`docs/RPC.md`); `/act` ruft dieselben Methoden für Formulare ohne JavaScript
  (beim Löschen prüft `/act` das Kästchen `confirm` selbst und reicht `confirm: true` weiter).
  Umschalten, Ändern und Löschen von Fassungen (`revision.set_current`/`update`/`delete`) gibt es nur hier, nicht
  über MCP. `library_view` liefert neben der Literatur (für das Popup) den Revisionsbaum des Werks
  (`Store::revision_tree`) für Editor und Baumansicht.
- `posts.rs` (Runde 8): Beiträge verwalten: RPC `post.delete/move/to_idea` (`handles`/`call`, aus `rpc::call`
  nach der Werkwahl), gemeinsamer Ablauf `act` für RPC, Formular und MCP (Schreibsperre → `post_get` →
  Eigentumsprüfung → Änderung → `mirror::record_post`, genau ein Commit), `writes()` mit `POST /beitraege/act`
  hinter `guard::same_origin`. `mirror::describe_post`/`record_post` (Autor wie bei Einfällen).
- `pdf.rs`, `cover.rs`, `asset_requests.rs` (Runde 11): Buch als PDF, Umschlag/Titelei/Buchangaben, Anträge
  zu Bildern – siehe „Bilder, Umschlag und PDF“.
- `typeset.rs` (Runde 8): Typst-Vorschau: `prepare` (Export-Karte, Bilder, Rahmen aus `cover::frame`, Stand) → `compile`
  (Zwischenspeicher-Treffer, sonst statische `Semaphore` mit einem Platz, `spawn_blocking`,
  `tokio::time::timeout(COMPILE_TIMEOUT = 60 s)`; die Sperre wandert in den blockierenden Lauf, ein abgelaufener
  Lauf rechnet zu Ende und füllt den Zwischenspeicher). Zwischenspeicher statisch
  (`OnceLock<Mutex<VecDeque<Arc<Compiled>>>>`, nach `stand`, höchstens `CACHE_SIZE = 8`) – kein Feld in
  `AppState`. Meldungen: Zeile → `ExportMap::section_at_line` → `key`, `section`, `section_line`. Kapitel =
  oberster Gliederungspunkt über dem Abschnitt (unter einem Teil das Kapitel darin), Seite eines Abschnitts =
  Seite seiner Überschrift (Typst-Introspektion). RPC `preview.typst`, Seiten `/werke/{id}/satz` (eigene
  minijinja-Umgebung, `templates/typeset.html`, `static/typeset.css`) und `/werke/{id}/satz/{stand}/{n}.svg`
  (nur aktueller Stand, `nosniff`, CSP `sandbox`). MCP `mcp/typst.rs` (`typst_check`, über `typeset_at`).
- `mcp.rs` mit `mcp/tools.rs`, `mcp/ideas.rs`, `mcp/typst.rs`, `mcp/outline.rs` (Gliederung: anlegen, Anträge) und `mcp/session.rs`: MCP-Server (Protokoll 2025-06-18, nur
  JSON). Werkzeuge lesen über `Store`/`baruch_library::search` und schreiben nur Vorschläge
  (`Store::add_proposal`) und Beiträge (`Store::add_post`; verschieben, löschen, umwandeln nur eigene über
  `posts::act` – Eigentum über den Handle des Clients, seit Runde 23 mit Token eindeutig und ohne Token gegen Token-Namen
  abgegrenzt (`lokal-`, siehe „Runde 23 – Härtung“); die Beiträge des Menschen trifft keine), dazu Einfälle mit vollem CRUD (`mcp/ideas.rs`,
  ändern keinen Buchtext, siehe „Einfälle“); einen Import gibt es über MCP nicht.
  Sitzungen (`Mcp-Session-Id` → clientInfo) prozessweit, höchstens 256. Fehler über
  `rpc::RpcError::from`. Beschreibung: `docs/MCP.md`.
- `literature.rs`: Literatur-Popup (`GET /literatur`), Dokumentansicht einer Quelle (`GET /quelle`),
  Formulare ohne JavaScript (`POST /literatur/act`) und die RPC-Methoden `literature.render` und
  `highlight.*`; eigene Vorlagen-Umgebung, siehe „Literatur“.
  Seit Runde 13 mit `literature/fulltext.rs`: Volltext über MultiGPT (`GET /quelle/multigpt`, RPC `source.read`,
  `source.adopt`, `highlight.check`), offline die lokale Kopie aus dem Abgleich (`local_copy` →
  `sync::local_document`); MCP `source_read` in `mcp/sources.rs`.
- `websearch/` (Runde 13): Websuche für `@belege` über MultiGPT (`web_search`, `fetch_url`), Prüfung der Zitate
  im Code, Belege (`web_evidence`, Migration 0034), Belegseite `/websuche/belege`, `POST /websuche/act`, RPC
  `websearch.*`; Dispatch in `rpc.rs` vor `crate::rag::handles`; kein MCP-Werkzeug. Siehe „Websuche“.
- `sync/` (Runde 13): Abgleich Baruch ↔ MultiGPT (RPC `sync.*`, Seite `/abgleich`, `POST /abgleich/act`,
  Migration 0033), Art `remote` im lokalen Index; Dispatch in `rpc.rs` direkt nach `websearch`; kein
  MCP-Werkzeug. Siehe „Abgleich“.
- Gemeinsame MultiGPT-Sitzung (Runde 13): Suche, Volltext, Websuche und Abgleich nutzen dieselbe
  `connections::multigpt::Session` mit prozessweitem Vorrat (siehe „MultiGPT“); der HTTP-Client einer Sitzung
  hat eine Obergrenze von 30 s (der Abgleich erlaubt 30 s je Aufruf), das Zeitlimit je Aufruf setzen die
  `Limits` des Aufrufs. `Session::call` gibt auch Daten nur ohne Token zurück.
- `wysiwyg.rs`: RPC `wysiwyg.parse/serialize/style/diff` (`handles`/`call`, aus `rpc::call`), nur lesend, ohne
  Schreibsperre, nicht unter `/act` und nicht über MCP; Schlüssel über `rpc::resolve_keys`. Das Widget
  `web/src/wysiwyg.ts` (Satzbild im Editor-Formular, Widget auf `div.wysiwyg` um Werkzeugleiste, Blatt und Statuszeile) und `static/wysiwyg.css`
  (Variablen `--ws-*` mit Vorgaben) gehören dazu. Gespeichert wird über das Editor-Formular: Vor dem Absenden
  löst `outline.ts` `baruch:before-save` aus, `wysiwyg.ts` füllt das Textfeld aus `wysiwyg.serialize`.
  Beschreibung: `docs/WYSIWYG.md`. `wysiwyg.parse` und `wysiwyg.diff` zeigen Inseln mit Bild als Bild
  (`div.ws-figure` bzw. `span.ws-inline-image`; `wysiwyg::image_ref`: `src = /werke/{work}/bilder/pfad?p=…`,
  `alt` aus `Store::asset_list` über `wysiwyg::image_alts`, Schlüssel in Normalform
  `baruch_store::normalize_asset_path`, also mit oder ohne `/` und `./`).
- `numbering.rs`: Gliederungsschemata: RPC `numbering.*`, Seite `/werke/{id}/nummerierung` (eigene
  Template-Umgebung `base.html` + `work_numbering.html`, Stil `static/numbering.css`), Formular
  `POST /werke/nummerierung/act`.
- `assets.rs`: Bilder: RPC `asset.*`, Seiten `/werke/{id}/bilder…`, Formulare `POST /werke/bilder/hochladen`
  und `POST /werke/bilder/act` (siehe „Bilder“).
- `personas.rs`: Personas: RPC `persona.list`/`persona.set_color`/`persona.update` (vor der Wahl des Werks,
  Personas sind global), `GET /einstellungen/personas` → 303 `/einstellungen/assistenten` (die Seite zeichnet
  `settings/assistants.rs`), Formular `POST /einstellungen/personas/act` (hinter `guard::same_origin`), `tints(color)` (Hintergrund hell
  als Mischung mit `#ffffff`, dunkel mit `#1f2328`, in 5-%-Schritten weiter zur Fläche, bis `--text`,
  `--muted` und `--primary` des Themas (`LIGHT_FOREGROUNDS`/`DARK_FOREGROUNDS`, Test gegen `app.css`)
  mindestens 4,5 : 1 erreichen), `contrast_ratio` (WCAG 2.x) und `style(color)` =
  `--pc-light:…;--pc-dark:…;--pc-line:…;--pc-line-dark:…` für das `style`-Attribut; die Linien erreichen
  mindestens `MIN_LINE_CONTRAST` = 3 : 1 gegen `LIGHT_SURFACES` (`LIGHT_PANEL` und das Blatt des dunklen
  Themas `DARK_SHEET`, auf dem ebenfalls `--pc-line` gilt) bzw. `DARK_PANEL` (WCAG 1.4.11, `Tints.line`,
  `Tints.line_dark`). Stil `static/proposals.css`: Klasse `.pc`
  setzt `--pc-bg` je Thema mit denselben Selektoren wie `app.css` (`prefers-color-scheme`, `data-theme`).
- `ideas.rs`: Einfälle: RPC `idea.*` (vor der Wahl des Werks), Seiten `/einfaelle`, `/einfaelle/{id}`,
  Formular `POST /einfaelle/act` (siehe „Einfälle (Migration 0011)“). `href()` prüft auch im Release
  (`is_safe_href`: `"` `<` `>` `'` Leerzeichen, Steuerzeichen → Rückfall `/einfaelle`).
- `menu.rs`, `settings/`, `connections/`: Kopfzeile, Einstellungen, Verbindungen und Quellen (Runde 9), siehe
  „Menü und Einstellungen (Runde 9)“.
- `mirror.rs`: Commit-Auftrag aus einer Änderung (Betreff, Rumpf, Autor, Trailer, `allow_empty`) und Commit;
  Protokoll für Umschalten, Ändern und Löschen siehe „Revisionsbaum“; Einfälle (`describe_idea`,
  `record_idea`, Spiegel `<data>/einfaelle/` für Einfälle ohne Werk).
- `guard.rs`: Schutz der schreibenden Routen gegen fremde Webseiten (CSRF).
- `view.rs`: Ansichtsmodelle, Einstellungen aus Cookies, Zeichnen von Seite und Bereichen. Kontext der
  Vorlagen: `work` = `{id, slug, title, author}`, `works` = `[{id, slug, title, selected}]` (Werkwahl),
  `lq_q` = `&lq=…` oder leer (hängt die Suche an Links), `pick_q` = `&pick=…` oder leer, `rev` mit `number`
  (Nummer), `position`/`total` („‹ n/m ›“), `is_current`, `current`, `prev`/`next` (Nummern), `revtree` =
  `{section, chapter}` (Baumansicht), `proposals` (offene Vorschläge des Abschnitts als Varianten im
  Lesefluss, `fragments/proposals.html`), `preview` = `[{node, lines, proposals}]` (Vorschau je Abschnitt,
  Vorschläge mit Zeilen-Diff), `rev.author_style`, `personas[].style`, `node.posts[].style`,
  `revtree…[].style` (CSS-Variablen der Persona-Farbe). **Runde 8:** `Location.key` und
  `Resolved.key`/`missing_key` (Auflösung: Schlüssel vor `node`, unbekannt → `node` bzw. Vorgabe mit Hinweis),
  `node_url(work, key, rev, preview, pick_q, lq_q)` baut die kanonische Adresse `/?work=W&key=K…`,
  `post_targets` (Ziele für „Verschieben…“), Kontext `node.post_count`, `node.missing_key`, Beiträge mit `id`,
  `created`. `Settings.editor` (`satz`|`quelltext`, Cookie `baruch_editor`, Vorgabe `satz`) entscheidet schon im
  Server-HTML über den Umschalter; bei „satz“ steht das Satzbild fertig im Blatt (`data-ws-ready`). Vorschläge:
  `proposals_of(node, shown, ctx, satz)` richtet jeden offenen Vorschlag mit `baruch_wysiwyg::align` an der
  **gezeigten** Fassung aus → `InlineView {key, rev, count, proposals, entries: [FlowEntry {b, raw, lines,
  hunks: [HunkView]}]}` (Kontext `proposals`, im Editor) bzw. `PreviewSection {node, flow}` (Kontext `preview`). `Settings::folded` (Cookie `baruch_folded`,
  '.'-getrennte `<key>-<rev>`, höchstens 200, nur `[0-9.-]`, sonst verworfen) faltet Vorschläge; der gezeigte
  Vorschlag steht immer offen (`pv-shown`). Reihenfolge: Stelle der Persona in `Store::persona_colors`
  (`position`, `id`), dann Nummer; Autoren ohne Persona-Eintrag am Ende mit `FALLBACK_COLOR`. Basis des Diffs:
  `based_on`, sonst die aktuelle Fassung. Satzbild-Diff über `baruch_wysiwyg::diff_html_with` mit
  `wysiwyg::image_ref` und `LibraryView::image_alts` (Nachschlagen in Normalform); `LibraryView` trägt dazu
  `persona_colors` und `image_alts`. `Location::resolve` sucht die Fassung über ihre Nummer; `pick` gilt nur
  für die Vorschau.
- `templates/`: `workspace.html` und `fragments/{tree,editor,edit_toolbar,revisions,revtree,assistants,library,
  proposals,idea_capture}.html`; für `literature.rs` außerdem `literature.html`, `source.html` und
  `fragments/highlights.html`. **Editor in Bereichen (Wunsch 2026-11-06):** `#pane-editor` scrollt nicht
  (`overflow: hidden`, Flex-Spalte) und trägt `div.editor-head` (Kopf: Titel, Fassungsanzeige, „‹ n/m ›“,
  „aktuell: Fassung k“), im Editor `div.wysiwyg[data-widget=wysiwyg]` mit Werkzeugleiste
  (`fragments/edit_toolbar.html`, `div.ws-bar[data-edit-toolbar]`, ohne JavaScript `hidden`; seit
  Runde 18a das Menüband mit den Formaten der Werkvorlage, `span.ribbon-formats[data-widget=ribbon]`), Blatt `div.editor-body` (`flex: 1 1 0;
  min-height: 0; overflow: auto` – scrollt allein; darin `form#editor-form.editor-form` mit Satzbild
  `div.ws-wrap[data-ws-wrap]` (ohne JavaScript `hidden`) und Textfeld, dahinter `section.pv-flow`) und
  Statuszeile `div.editor-status` (Hinweise, `[data-ws-hint]`, „Speichern“ mit `form="editor-form"`); in der
  Vorschau Kopf, `div.editor-body` mit `div.sheet-wrap` und Statuszeile. Ursache des früher zu niedrigen
  Blatts: `#pane-editor` scrollte als Ganzes, alle Kinder hatten `flex-shrink: 0` (`proposals.css`), Fassungs-
  knöpfe und -liste standen mit im Editor; war die Höhe verbraucht, blieb dem Formular (`flex: 1`) kein freier
  Platz, es fiel auf `min-height: 12em`, und das Satzbild bekam nach Leiste und Hinweisen nur wenige Zeilen.
  Das Widget `wysiwyg` sucht das Formular in seinem Bereich (`root.querySelector`), den Speichern-Knopf über
  `widgets.ts` `submitButton` (`form.elements`, auch mit `form=`). Fassungsliste, „Diese Fassung verwenden“,
  „Fassung ändern“ und „Löschen“ stehen in `fragments/revisions.html`, dem Akkordeon-Abschnitt
  `details.acc[data-section=revisions]` in `assistants.html` (zwischen Assistenten und Chat; Abzeichen
  `rev.total` und `rev.open_proposals`; Cookie `baruch_accordion=revisions`, `Settings::accordion`); es kommt
  mit der rechten Spalte aus `ui.render` neu. Zoom-Zonen (`web/src/zoom.ts`): jedes `[data-zoom-zone]`
  (`tree`, `assistants`, `revisions`, `head`, `toolbar`, `sheet`, `status`) plus `dialog[open]` und `main`;
  die innerste gewinnt, Stufe je Name im Cookie `baruch_zoom`, ein `MutationObserver` setzt sie auf neu
  gezeichnete Zonen. Offene Vorschläge stehen in
  `fragments/proposals.html` (Runde 8: an ihrer Stelle; im Editor `section.pv-flow[data-widget=proposals]`
  nach dem Editor-Formular mit der gezeigten Fassung blockweise als `pre.pv-src[data-b]` und dazwischen je
  Stück ein `details.pv[data-pv="<key>-<rev>"][data-pv-at][data-pv-gap][data-pv-kind]`; in der Vorschau
  `div.pv-section` mit den Zeilen und den Stücken dazwischen; je Vorschlag EIN Formular `pv-accept-…` bzw.
  `pv-reject-…` außerhalb der Stücke, die Knöpfe zeigen mit `form=` darauf). Mit JavaScript setzt `wysiwyg.ts`
  (`placeProposals`) jedes Stück als `contenteditable=false`-Geschwister an den Block `[data-b]` des Satzbilds
  und verbirgt `pv-flow`; `editsOf`/`blocksIn`/`markSelection` übergehen `.pv`. Je Stück `div.pv-satz.ws-sheet` (Satzbild-Diff vom Server) und
  `pre.pv-quelltext.diff` (Zeilen-Diff). Welcher sichtbar ist, entscheidet nur CSS:
  `#pane-editor:has(.wysiwyg[data-ws-open])` zeigt den Satzbild-Diff, sonst (auch ohne JavaScript) den
  Zeilen-Diff. Widget `web/src/proposals.ts`: `toggle` (Einfangphase) → Cookie `baruch_folded`,
  `[data-pv-all]` „Alle ein-/aufklappen“ (ohne JavaScript `hidden`). Die Vorschlag-Karte und das Widget
  `wysiwyg-diff` sind entfernt (Runde 7); die RPC-Methode `wysiwyg.diff` bleibt.
  Nur bei offenem Satzbild trägt der Bereich `data-ws-open` (setzt `wysiwyg.ts`); das Satzbild füllt dann
  das Blatt (`static/wysiwyg.css`), im Quelltext das Textfeld.
- `web/src/*.ts`: Widgets (`data-widget`), von `build.rs` übersetzt und unter `/static/js/` ausgeliefert.
- Markdown-Überschriften (Nutzer 2026-11-07): Der Server wendet `baruch_typst::markdown_headings` auf Eingaben
  des Menschen an – `outline.save`/`outline.check` und die Rückfrage `confirm_page` (`outline.rs`),
  `revision.save` und beim Übernehmen `revision.accept` (`rpc.rs::accept_markdown_headings`: Vorschlag bleibt,
  danach `outline_save` mit `split`); `####`–`######` → `===` (begrenzt). Vorschläge (`add_proposal`, MCP) und
  die Werkvorlage laufen nicht durch die Regel. Live: `web/src/markdown.ts` (gleiche Fälle, vorsichtiger
  Zeilen-Scanner) im Widget `editor` (`panes.ts`: Enter, Verlassen) und im Satzbild (`wysiwyg.ts`:
  `convertMarkdownHeading` beim Enter und vor `sync`). Tests: `baruch-typst/src/markdown.rs` (Tabelle),
  `baruch-server/tests/markdown_headings.rs`.
  `chat.ts` (Runde 8): Widget `chat` (Rollen, „neu seit dem letzten Besuch“ über LocalStorage
  `baruch_chat_seen` = `{"<work>-<key>": "<größtes created>"}`, jeder Zugriff in try/catch).

## Daten

```
<data>/baruch.sqlite          Quelle (WAL)
<data>/works/<slug>/          Git-Spiegel je Werk, nur lesen
    main.typ                  Werkvorlage (falls importiert), dann #include der Abschnitte in Baumreihenfolge
    vorlage.typ               nur mit Werkvorlage: die Vorlage selbst
    abschnitte/NNN-titel.typ  aktuelle Revision je Abschnitt; mit Werkvorlage davor #import "../vorlage.typ": *
    bilder/<name>             aktuelles Bild als SVG-Text (encoding svg)
    bilder/<name>.b64         aktuelles Bild als Base64 in Zeilen zu 76 Zeichen (encoding base64)
    einfaelle.md              Einfälle des Werks (nur, wenn es nicht gelöschte gibt; Migration 0011)
<data>/einfaelle/             Git-Spiegel der Einfälle ohne Werk (nur einfaelle.md), kein Symlink
```

Migration `0010_persona_color.sql` (Runde 7): `persona.color` = `NULL` (Vorgabe aus
`baruch_store::PERSONA_PALETTE` nach Reihenfolge `position`, `id`) oder `#rrggbb` klein (CHECK mit `GLOB`).
Keine Datenänderung. `Store::persona_colors` liefert `PersonaColor {handle, name, color, chosen}`,
`Store::set_persona_color(handle, Some|None)` setzt bzw. setzt zurück (eine Transaktion `BEGIN IMMEDIATE`,
Handle auch ohne `@`); unbekannt → `StoreError::UnknownPersona`, ungültig → `StoreError::Color` (beide
-32602). `baruch_core::Persona` bleibt unverändert; kein Commit im Git-Spiegel (Personas sind kein Buchtext).
Test `crates/baruch-store/tests/migration_0010.rs` auf einer nachgebauten Datenbank im Stand development
(0001–0009, 0101–0105, mit Bild, Vorschlag, Beitrag, Bibliothek). Migration `0011_ideas.sql`: Tabellen
`idea`, `idea_word`, `idea_fts` (siehe „Einfälle (Migration 0011)“). Beide zusammen prüft
`crates/baruch-server/tests/round7.rs` über einen vollen Serverstart. Migration `0013_connections.sql`
(Runde 9): Tabellen `connection`, `work_rag`, `preferred_source` (siehe „Verbindungen und Quellen“), Test
`crates/baruch-store/tests/migration_0013.rs`. Migration `0014_connection_secret_target.sql`: nur die Spalte
`connection.secret_target` (Bindung des Geheimnisses an sein Ziel, NULL = jetziges Ziel), Test
`crates/baruch-store/tests/migration_0014.rs`.

`<data>` ist `--data`, sonst `BARUCH_DATA`, sonst `baruch_store::default_data_dir()` =
`dirs::data_local_dir()/baruch` (Linux `$XDG_DATA_HOME` bzw. `~/.local/share`, macOS
`~/Library/Application Support`, Windows `%LOCALAPPDATA%` – Local statt Roaming, weil servergespeicherte
Profile WAL und Spiegel sonst bei jeder An- und Abmeldung kopieren).

Ohne Werkvorlage beginnt `main.typ` mit `// Baruch: <Titel>`, und es gibt weder `vorlage.typ` noch
`#import`-Zeilen. Der Spiegel ist eine Projektion (`snapshot.rs`): `main.typ` behält die Vorlage am Anfang,
damit Set- und Show-Regeln für alles Eingebundene gelten; die `#import`-Zeile macht `#let`-Funktionen der
Vorlage in den Abschnittsdateien verfügbar. Die Revisionen in SQLite und `export_typst` bleiben ohne diese
Zeile. Ob sich `main.typ` mit `typst compile` setzen lässt, ist noch nicht geprüft (typst fehlt auf dem
Entwicklungsrechner).

Ordnernamen (`slug`): Kleinbuchstaben und Ziffern, Umlaute umgeschrieben, sonst `-`; höchstens 40 Zeichen,
an der letzten Wortgrenze gekürzt (ein einzelnes längeres Wort hart), leer → `werk`, Kollision `-2`, `-3` ….
Vorhandene slugs in der Datenbank werden nicht umgeschrieben. Abschnittsdateien heißen nach demselben
`slug`; ändert sich die Kürzung, benennt der nächste Spiegel-Commit die Datei um.

Das Beispielwerk („Beispielwerk: Leise Abhängigkeit (Auszug)“, `baruch-core/src/sample.rs`) wird nur in eine wirklich
neue Datenbank gesät; vorhandene Werke behalten Titel und slug. Seine Teile tragen keinen Text (`""`, wie
`outline.add` bei Teilen), die übrigen Knoten ohne eigene Fassung nur ihre Überschrift.

Neue Ordnernamen meiden auch die slugs gelöschter Werke (`work_retired`, siehe „Werk-ids und Saat“).

## Typst-Import

`baruch-typst::parse` zerlegt eine Datei: Werkvorlage = alles vor der ersten Überschrift; Überschrift =
Zeile ab Spalte 0 mit 1–3 `=` und Leerzeichen/Tab, die im Markup der obersten Ebene beginnt; Text je
Überschrift = von ihrer Zeile bis vor die nächste. `join(template, bodies)` ergibt die Datei Byte für Byte
zurück. Ein Kommentar hinter der Überschrift (`//`, `/*`; nicht in Raw, nicht `https://`) gehört nicht zum
Titel.

Der Scanner verfolgt zeilenübergreifend: Raw-Blöcke (```` ``` ```` am Zeilenanfang), Inline-Raw über
Zeilen, Blockkommentare (geschachtelt), Strings im Code (auch mehrzeilig) und einen Klammerstapel ab einem
`#`-Ausdruck: `#name(` und `#name[` öffnen, nach `)`/`]` eines Aufrufs darf ein weiterer `[…]` folgen;
nach `#let`, `#set`, `#show`, `#import`, `#if` … ist der Rest der Zeile Code (dort öffnet jedes `(`, `[`,
`{`). In Inhaltsblöcken zählen eckige Klammern mit, im Fließtext der obersten Ebene nicht (dort ist `[`
Text). Nur wenn nichts offen ist, kann eine Zeile eine Überschrift sein. Im Markup gilt `"…"` nur als
String, wenn er in derselben Zeile schließt.

Titel/Autor aus dem ersten `#set document(title: "…" | […], author: "…" | ("…", …))` außerhalb von
Kommentaren und Raw (`blank_comments` ersetzt sie vorher durch Leerzeichen, Positionen bleiben). Grenzen:
eingerückte Überschriften und jedes `#heading(...)` außer den beiden Markierungen (Runde 14, unten) zählen
nicht (vor der ersten Überschrift also Teil der Werkvorlage); `] else [` am Anfang einer Folgezeile gilt als Fließtext. Ein UTF-8-BOM am Dateianfang
gehört zur Werkvorlage, eine Überschrift direkt dahinter zählt.

`Store::import_typst` (eine Transaktion): CRLF → LF, SHA-256 der Quelle (`work.source_sha256`, eindeutig;
schon da → `AlreadyImported`), Werk (Art `book`, `template`, `source_name`), Knoten mit `seq` in
Dateireihenfolge, `=` → `chapter`, `==`/`===` → `section` (Elternteil = nächste vorige Überschrift mit
kleinerer Ebene), Teil-Markierung → `part` (Runde 14), Nummern über `load::numbers` mit der
Nummerierung der Werkvorlage (wie beim Laden; Vorgabe „1“, „1.2“, „1.2.1“), Markierung „ohne Nummer“ →
`node.numbered = 0`; je Knoten Revision 1 vom Menschen, `current`, Notiz
„Import aus <datei>“ (`Store::import_typst_noted`). Migration `0002_work_template.sql`.
`Store::export_typst` setzt Vorlage und aktuelle Texte wieder zusammen.

Im Server liest `import::import_file` die Datei: `std::fs::metadata` prüft vor dem Öffnen, dass es eine
reguläre Datei ist (FIFO, Gerät, Ordner → „… ist keine Datei“), Lesen in `tokio::task::spawn_blocking`,
höchstens 16 MiB, UTF-8. Aufrufer ist nur die CLI (`baruch-server import`); MCP hat kein Import-Werkzeug.

**Rückweg eines Exports (Runde 21b).** Was nur der Export aus Einstellungen schreibt, kommt beim Import an seinen Platz
zurück; jeder Eingriff ist eine `ImportWarning` im Bericht (`import::import_source`):
- *Regelblock „// Baruch: Satz“* (`store/import.rs::split_typeset`): Reicht der Block ab der letzten Zeile
  `RULES_MARK` bis ans Ende der Werkvorlage und liest `baruch_typst::typeset::settings_from_rules` ihn zurück, landen die
  Einstellungen in `work_typeset` (`typeset::write` in derselben Transaktion) und der Block nicht in der Vorlage
  (`import-typeset-adopted`). Bedingung ist, dass `rules` aus dem Ergebnis **genau** denselben Block macht (nie raten;
  Seitenzahlen × Kolumnentitel werden durchprobiert, 9 × 4). Sonst bleibt er Vorlage (`import-typeset-kept`). Grenzen:
  Die R31-Felder und `xref_with_page` stehen nicht im Block; der Bundsteg kommt als Teil des Innenrands zurück (über
  80 mm der Rest als Bundsteg) – gleicher Satz, andere Felder.
- *Show-Regeln „// Baruch: Überschriften ohne Nummer“* (`split_fallback`): jede Zeile `#show heading.where(level: N,
  body: [T]): set heading(numbering: none)` am Ende der Vorlage → `node.numbered = 0` für die Überschriften mit Ebene N
  und wörtlichem Titel T (Gegenstück zu `snapshot::fallback_rules`); eine Zeile anderer Form lässt alles in der Vorlage.
  Damit übersteht „Nummer aus“ mit eigenem Label den Weg (vorher kam der Abschnitt nummeriert zurück). Unter „Ohne
  Nummern“ (`numbering.is_off()`) schreibt `snapshot::project` seit 21b ebenfalls die Show-Regel für `numbered = 0`
  (die Textform bleibt `=`, im Satz ändert die Regel nichts) – so geht der Merker auch dort nicht mehr verloren.
- *Buchende* (`baruch_typst::index::strip_back_matter`): nur, wenn auf die Zeile `BACK_MATTER_MARK` unmittelbar die
  Hilfsfunktionen folgen, die allein `back_matter` schreibt (`index/back.rs::HELPERS`); dann `import-back-matter-cut`
  mit Zeile. Eine Kennzeile von Hand schneidet nichts ab (`has_back_matter_mark` → `import-back-matter-kept`).
- *Projektion*: mit Buchende `strip_projection` (Bibelstellen-Marken mit `sort`, verlinkte erste Erwähnung), immer
  `strip_first_mentions`: Die erste Erwähnung ohne Glossarseite schreibt `index/project.rs` seit 21b als
  `/*baruch-erste-erwaehnung*/#emph[Wort]` (Kommentar = unsichtbarer Vermerk, `FIRST_MENTION_MARK`); vorher war sie von
  einer Hervorhebung von Hand nicht zu unterscheiden und blieb nach Export → Import als `#emph` im Text.
  `import-projection-removed`, wenn etwas herausfiel.

**Grenzen der Nummerierung (geprüft in 21b, nicht behoben):**
- *Funktionen der Werkvorlage* mit `p.at(i) ± k`, `slice()` oder Inhalt als Ergebnis wertet Baruch nicht je Ebene aus
  (`Numbering::from_template` bleibt bei der Näherung). Grund: Das bräuchte eine Typst-Auswertung je Überschrift (oder
  einen eigenen Ausdrucksauswerter) im Store, der heute ohne Typst auskommt; der Satz selbst stimmt immer (Typst rechnet),
  nur die Nummern in Baum und Gliederung können abweichen. In Runde 23 nicht umgesetzt, bleibt offen (in R24 nicht umgesetzt, Backlog „offen nach R24“; Backlog
  „Nach Runde 23 offen“). Vorschlag (Nummern aus dem Satz übernehmen:
  `Rendered.headings` trägt Seite und Zeile, die Nummer ließe sich dort mit `counter(heading).display()` abfragen).
- *Teile mit `numbered = 0`* (nur aus Runde 14 vor `bba23db` möglich): wirkungslos – Teile zählen nie
  (`load::numbers`), die Projektion und die Show-Regeln nehmen Teile aus (`heading_rows` mit `kind <> 'part'`), und der
  Export schreibt die Teil-Markierung unabhängig vom Merker; ein Re-Import setzt ihn aus der Textform neu. Keine
  Migration nötig; Zurückstellen bleibt über RPC `outline.set_numbered`.
- *Titel mit Code an einem gleichlautenden nummerierten Knoten* (z. B. zweimal `= #emph[Ruhe]`, einmal „Nummer aus“):
  keine Markierung möglich (Code im Titel), und eine Show-Regel `heading.where(body: […])` träfe beide. Seit Runde 22
  gilt dafür eine Regel über die Sprungmarke (`#show <kN>: set heading(numbering: none)`) in einem eigenen Block; der
  Re-Import liest sie zurück (Einzelheiten unter „Spiegel setzen“ bzw. `snapshot::split_label_fallback`).

### Teile im Typst-Text (Runde 14)

Ein Teil und ein Punkt „ohne Nummer“ stehen im Typst-Text als kanonische Markierung (`baruch_typst::marker`),
je eine Zeile ab Spalte 0:

```typst
#heading(level: 1, numbering: none)[Erster Teil] <baruch-part>
#heading(level: 2, numbering: none)[Zwischenruf] <baruch-unnumbered> // Kommentar erlaubt
```

- **Warum so:** Typst setzt beide ohne Definition in der Werkvorlage; `numbering: none` stößt den
  Überschriftenzähler nicht an (Typst zählt nur Überschriften mit Nummerierung), Kapitel laufen also über
  Teile fort wie in Baruch; die Labels machen die Zeile eindeutig als Baruch-Markierung. Ein frei
  geschriebenes `#heading(...)` (andere Abstände, anderes Label, Einzug, Ziffer ≠ 1 beim Teil, in Raw,
  Kommentar, Inhaltsblock oder String) bleibt wie bisher Text bzw. Werkvorlage.
- **Titel `T`:** maskiert wie `escape_title`, zusätzlich ein `\` vor einem führenden `=`, `-`, `+`, `/` und
  vor dem Punkt eines führenden `Ziffern.` (im Inhaltsblock begänne sonst eine Struktur); `T` enthält kein
  unmaskiertes `[`/`]`, keinen Kommentar und kein offenes Inline-Raw. `unescape_title(T)` ist der Titel.
- **`parse`:** Teil → `Heading { level: 0, numbered: true }`, „ohne Nummer“ → `Heading { level: N,
  numbered: false }`; eine Markierung vor der ersten `=` beendet die Werkvorlage; `join` bleibt byte-gleich.
- **Bauen:** `part_line`, `unnumbered_line`; `heading_line(0, …)` ergibt die Teil-Markierung, eine
  Markierung „ohne Nummer“ bleibt beim Umbenennen eine (Kommentar bleibt); `relevel_line` ändert nur die
  Ebene, `set_numbered_line` stellt die erste Überschriftszeile zwischen `=`-Zeile und Markierung um (für die
  Projektion im Export, R14c).
- **Import/Export:** Der Import macht aus der Teil-Markierung einen Teil (oberste Ebene, die folgenden
  Kapitel darunter) und aus „ohne Nummer“ `numbered = 0`; der Text bleibt wörtlich, Export ist byte-gleich
  (auch mit Werkvorlage), der Spiegel hat je Teil eine Abschnittsdatei. Der Import selbst wandelt nie um.
- **Umwandlung alter Teiltexte** (Nutzerfrage 8, entschieden 2026-11-07, `baruch-store/src/legacy_parts.rs`):
  Startschritt statt Migration – die neue Zeile braucht `baruch_typst` (Maskierung, Gegenprobe mit `parse`), jede
  Umwandlung ist eine Fassung mit Spiegel-Commit (den kennt nur der Server), und der Schritt ist gegen eine
  nachgebaute alte Datenbank testbar. `AppState::open_with` ruft nach `mirror::sync_all`
  `mirror::convert_legacy_parts` → `Store::convert_legacy_parts(HUMAN_NAME)`: eine Transaktion, Merker
  `store_meta.parts_marked_r14` (die Saat einer neuen Datenbank setzt ihn gleich mit). Je Teil zählt die aktuelle
  Fassung (`legacy_parts::classify`): leer oder genau eine `=`-Zeile ohne Label/Kommentar mit dem Titel des Teils
  oder einem Titel, der mit „Teil“ beginnt → neue aktuelle Fassung `heading_line(0, Titel, None) + "\n"` (Autor
  der Mensch, Notiz `CONVERSION_NOTE` „Umwandlung Runde 14 (Nutzerentscheidung)“, `based_on` = alte Fassung, die
  `superseded` erhalten bleibt); schon markiert oder ohne Fassung → nichts; sonst bleibt der Text und nur die
  Knoten-id kommt in `PartConversion.kept` (stderr: Anzahl und ids, kein Wortlaut). Je Werk mit Umwandlungen ein
  Commit „Umwandlung Runde 14 (Nutzerentscheidung): n Teile auf die Teil-Markierung umgestellt“ (`allow_empty`,
  Trailer `Baruch-Action: parts-converted`, `Baruch-Origin`, `Baruch-Node` je Teil). Test:
  `baruch-server/tests/round14_conversion.rs`.
- **Satz:** `typeset::prepare` setzt hinter Werkvorlage und Rahmen die Regel `part_page_rule(labels)` (eine
  Zeile, `#show <baruch-part>: …`; Nummern aus `Node.number` der Teile, `typeset::part_labels`): jeder Teil
  allein auf einer eigenen rechten Seite (`pagebreak(to: "odd")`, danach weicher Umbruch), gezählt mit dem
  eigenen Zähler `baruch-part`. Die Regel nutzt nur `it.body`, Überschrift-Regeln der Vorlage wirken darum
  nicht doppelt; sie steht hinter der Vorlage und greift so zuerst. Satz-Ansicht, `work.pdf`,
  `/werke/{id}/pdf` und `export --pdf` nutzen dasselbe `prepare`. Ohne Teil-Markierung ist die Quelle
  byte-gleich. Belegt in `baruch-render/tests/parts.rs` (Typst prüft Zähler und Labels selbst per
  `#context assert…`, mit Gegenprobe).
- **Satzbild:** Blockarten `part` (`title`) und `unnumbered` (`level`, `title`), siehe `docs/WYSIWYG.md`.

## Werke verwalten

Migration `0005_works.sql` (Vertrag Runde 4) und `baruch-store/src/works.rs`:

- **Eindeutige Titel:** `work.title_key` = `baruch_store::title_key(title)` (getrimmt, Folgen von Unicode-Leerraum
  → ein Leerzeichen, `to_lowercase`), eindeutig über den Teilindex `work_title_key` (`WHERE title_key IS NOT
  NULL`). Die Migration legt die Spalte nur an; `Store::open` trägt nach `MIGRATOR.run` und vor der Saat in Rust
  nach (`works::ensure_title_keys`, `BEGIN IMMEDIATE`, wiederholbar): Werke ohne oder mit veraltetem Schlüssel
  bekommen ihn. Haben zwei Werke denselben Schlüssel, ändert sich nur der Titel (slug, Spiegelordner, Knoten,
  Texte bleiben): Trifft ein gesätes Werk (ohne `source_name`) auf ein importiertes, behält das importierte mit
  der kleinsten id seinen Titel und das gesäte bekommt `baruch_core::sample::work().title`, sofern frei; sonst und
  in allen anderen Fällen bekommt das Werk mit der größeren id „<Titel> (2)“, „ (3)“ … (kleinste freie Zahl).
  Alle Einfügewege (Saat, Import, Anlegen) und `update_work` setzen den Schlüssel; ein vergebener →
  `StoreError::DuplicateTitle` (-32000), ein leerer Titel → `EmptyTitle` (-32602). Beim Import kommt die Prüfung
  der Prüfsumme (`AlreadyImported`) vor der des Titels. Ohne Werkvorlage steht der Titel im Spiegel nur in der
  Kommentarzeile von `main.typ`; eine Umbenennung ändert also höchstens diese Zeile.
- **Metadaten:** `subtitle`, `audience` (Zielgruppe), `bible` (Bibelübersetzung), `updated` (letzte Änderung von
  Metadaten oder Vorlage). `WorkSummary.updated` ist das Maximum aus `work.updated`, `revision.created` und
  `revision.edited`.
- **Fassungen der Werkvorlage:** Tabelle `work_template_revision (work_id, number, body, author_name, note,
  created)`, Nummern ab 1, nur angehängt. Die Migration legt für jedes Werk Fassung 1 aus `work.template` an
  (Notiz „Stand vor Migration 0005“); Saat, Import und Anlegen schreiben Fassung 1 selbst. `work.template` bleibt
  die aktuelle Fassung (= höchste Nummer); Schnappschuss und Export lesen weiter nur `work.template`.
  `update_work` legt nur bei anderem Text (CRLF → LF) eine neue Fassung an und lehnt eine Vorlage mit Überschrift
  ab (`baruch_typst::parse` fände sie, die Rundreise Export → Import wäre sonst nicht mehr dieselbe Gliederung).
- **Anlegen** (`create_work`): Grundgerüst `Preset::Book` (Kapitel „Erstes Kapitel“, Vorlage mit
  `#set document` und `#set heading(numbering: "1.1")`) oder `Preset::Sermon` („Einleitung“, „Hauptteil“,
  „Schluss“, Vorlage mit `#set document`); Titel und Autor im Typst-String maskiert (`\`, `"`).
- **Löschen ganzer Werke** (`delete_work`, eine Transaktion): `title_key(confirm_title)` muss passen, sonst
  `Refused`. Der Trigger `revision_no_delete_current` (0004) bleibt unverändert und schützt weiter lebende
  Abschnitte: Das Löschen setzt zuerst alle aktuellen Fassungen des Werks auf `superseded` (erlaubter Übergang
  laut `revision_status_transition`), dann löscht es Revisionen (der Index-Trigger aus `baruch-library` 0104
  nimmt sie aus `search_fts`), `post_ref`, `post` (der Trigger `post_fts_delete` aus 0105 nimmt die Beiträge
  aus dem Index), `node`, `work_template_revision` und `work`. Dieselbe Reihenfolge (erst `superseded`, dann
  löschen) nutzen `outline_delete` und das Zusammenführen für einzelne Knoten; zwei kleine Hilfsfunktionen in
  `works.rs` und `outline.rs`, bewusst getrennt. Der Server
  (`works.rs`) holt vorher Spiegel und Schnappschuss, schreibt danach den leeren Protokoll-Commit „Werk gelöscht:
  <Titel>“ (`allow_empty`, Trailer `Baruch-Action: work-deleted`) und verschiebt den Ordner nach
  `<data>/works/.geloescht/<slug>-<JJJJ-MM-TT>/` (Kollision `-2`, `-3`; `.geloescht` und Ziel keine
  symbolischen Verknüpfungen). `.geloescht` ist kein gültiger slug, `sync_all` berührt das Archiv also nie.
  Danach ist `AppState::default_work` (= `Store::default_work`, kleinste id) das nächste Werk; ohne Werke leitet
  `/` auf `/werke`. Sind alle Werke gelöscht, sät auch ein Neustart nicht neu (siehe „Werk-ids und Saat“).
  `delete_work` trägt id, slug und Titel in `work_retired` ein. `move_to_trash` (Server) verschiebt nur einen
  vorhandenen Ordner (sonst `Ok(None)`), prüft den slug wie `mirror_dir` und lehnt `works/`, den Spiegel und
  `.geloescht` als symbolische Verknüpfung ab.

### Werk-ids und Saat (Migration 0007)

Migration `0007_work_ids.sql` (Vertrag Runde 5):

- **ids nie wiederverwenden:** `work` wird mit `id INTEGER PRIMARY KEY AUTOINCREMENT` neu gebaut, mit allen
  Spalten aus 0001, 0002, 0005 und 0006 in derselben Reihenfolge, gleichen CHECKs und Vorgaben; die Indizes
  `work_source_sha256` und `work_title_key` werden neu angelegt. SQLite merkt sich die größte je vergebene id
  in `sqlite_sequence`; nach dem Löschen von Werk 2 bekommt das nächste Werk 3. (Ids, die schon vor 0007
  gelöscht und nie die größte waren, kennt die Datenbank nicht mehr.)
- **Weg:** sqlx-sqlite 0.8.6 führt jede Migration in einer eigenen Transaktion aus, auch mit
  `-- no-transaction` (das Feld `no_tx` wird dort nicht gelesen; Beleg: `sqlx-sqlite-0.8.6/src/migrate.rs`,
  `apply`, und der Test `sqlx_runs_even_a_no_transaction_migration_inside_a_transaction`). `PRAGMA
  foreign_keys=OFF` wäre darin wirkungslos, ein eigenes `BEGIN IMMEDIATE` scheitert. Darum läuft der Neubau in
  der Transaktion von sqlx mit `PRAGMA defer_foreign_keys=ON`: Zeilen nach `work_new` kopieren, `work`
  löschen (die Verweise aus `node` und `work_template_revision` zählen als offene Verstöße), `work` neu
  anlegen und füllen (die Verstöße sind wieder abgezählt), `work_new` löschen. Die neue Tabelle heißt gleich
  `work`, ein `RENAME` entfällt. Vor dem Ende prüft eine Wache (`fk_guard` mit `CHECK (n = 0)` über
  `pragma_foreign_key_check`), dass kein Fremdschlüssel verletzt ist; sonst scheitert die Migration, und nichts
  wird übernommen, auch kein Eintrag in `_sqlx_migrations`.
- **`store_meta(key, value)`:** Merkzettel des Stores. `seeded = 1` setzt 0007 für eine Datenbank, die schon
  Werke oder Personas hatte; sonst setzt ihn die Saat (`seed_if_empty`) in derselben Transaktion wie das
  Beispielwerk. Ohne `seeded` wird gesät, mit nie wieder: Nach dem Löschen aller Werke (auch nach einem
  Neustart) bleibt die Datenbank leer, und `/` leitet auf `/werke`.
- **`work_retired(id, slug, title, deleted)`:** gelöschte Werke. `free_slug` (Import, Anlegen, Saat) meidet
  auch diese slugs: Ein neues Werk mit dem Titel eines gelöschten heißt `<slug>-2`. Die slugs bleiben dauerhaft
  reserviert, weil die Datenbank den Archivordner unter `works/.geloescht/` nicht kennt (PM-Entscheidung Runde 5).
- Geprobt in `baruch-store/tests/migration_0007.rs` auf einer nachgebauten Datenbank im Stand vor Runde 5
  (Store 0001–0006, Bibliothek 0101–0105, Beispielwerk und ein erfundenes Buch mit 87 Gliederungspunkten).

## Revisionsbaum

Migration `0004_revision_tree.sql` (ersetzt die Regeln „nur anhängen“ aus 0001 und die Statusregel aus 0003):

- **Elternteil:** `revision.based_on` ist NULL (Wurzel) oder die Nummer einer kleineren Revision desselben
  Abschnitts (Trigger `revision_parent_insert`/`_update`, „Elternteil der Revision fehlt“). Beim Umstieg trägt
  Regel R nach: für `number > 1` ohne Basis die höchste frühere Fassung mit Status `current`/`superseded`.
  Neue Fassungen bekommen immer ein Elternteil: Speichern die gezeigte bzw. aktuelle, Vorschläge die aktuelle.
- **Markierung:** je Abschnitt genau eine `current` (Teilindex `revision_one_current`; mindestens eine prüft der
  Store). Statuswechsel (Trigger `revision_status_transition`): → `current` aus `proposed`, `rejected`,
  `superseded`; `current` → `superseded`; `proposed` → `rejected`; sonst „Unzulässiger Statuswechsel der
  Revision“. Umschalten ändert keinen Text.
- **Herkunft** (`node_id`, `number`, `author_kind`, `author_name`, `model`, `created`) ist unveränderlich
  (`revision_origin_immutable`); Text und Notiz darf der Mensch ändern, `edited` hält den Zeitpunkt fest.
- **Löschen:** die aktuelle Fassung nie (`revision_no_delete_current`); `revision_reparent` hängt die Kinder an
  das Elternteil der gelöschten Fassung.
- **Nummern:** `node.last_rev` ist die höchste je vergebene Nummer (`revision_number_bump`); eine Nummer
  `<= last_rev` lehnt `revision_number_fresh` ab („Revisionsnummern werden nicht wiederverwendet“). Nummern
  sind also eindeutig und steigend, nach dem Löschen mit Lücken; `load.rs` prüft genau das, `write.rs` nimmt
  `last_rev + 1`. Kern, RPC, Ansicht und Formulare sprechen Fassungen nur über die Nummer an.
- **Volltextindex:** `baruch-library` 0104 ersetzt bei geändertem Text den Eintrag und entfernt ihn beim Löschen.
- **Unbekannte Nummer:** `StoreError::UnknownRevision { node, rev, existing }` nennt alle vorhandenen Nummern
  („Fassung 3 gibt es nicht; vorhanden: 1, 2, 4“), nicht einen Bereich, der nach dem Löschen nicht stimmt.
- **Gleichzeitigkeit:** Jede Änderung ist eine `BEGIN IMMEDIATE`-Transaktion, im Server zusätzlich unter
  `AppState::write_lock`; parallele Umschalt-, Änderungs- und Löschaufrufe ergeben genau eine `current`, keine
  verlorene Änderung und je Erfolg genau einen Commit (Tests `baruch-store/tests/concurrency.rs`,
  `baruch-server/tests/concurrency.rs`).
- **Verweise** in Beiträgen (`post_ref`) bleiben unberührt; ein Verweis auf eine gelöschte Fassung zeigt die
  aktuelle.

Store-API (`src/tree.rs`): `switch_to(work, node, rev, only_proposal, human) -> Switch` schaltet in einer
Transaktion um (mit `only_proposal` nur Vorschläge, sonst jede nicht aktuelle Fassung; schon aktuell →
`Refused`; Vorschlag → `Accepted`, sonst `Switched`) und lässt die Gliederung der Überschriftszeile der
Fassung folgen (siehe „Gliederung“, „Umschalten bringt die Ebene mit“). `Switch { change, outline }` ist aus
`lib.rs` re-exportiert; `set_current` und `accept` sind Hüllen (`switch_to(.., false|true, "Autor")`, nur
`change`). Dazu `save_human_on` (Basis wählbar; `save_human` = Basis aktuelle Fassung), `update_revision` und
`delete_revision` (nur Mensch; `Change.before` = Stand vorher, `Change.reparented` = umgehängte Kinder),
`revision_tree` (alle Abschnitte nach seq, Fassungen nach Nummer, ohne Text).

Protokoll im Spiegel (`mirror.rs`), Betreff `„<Nummer> <Titel>: …“`:

| Änderung | Betreff | Autor / Committer | Rumpf | `allow_empty` |
|---|---|---|---|---|
| Übernehmen | `Vorschlag n übernommen` | Persona bzw. Mensch / Mensch | – | nein |
| Speichern | `Revision n gespeichert` | Mensch / Mensch | – | nein |
| Umschalten | `auf Fassung n umgeschaltet` (mit anderer Ebene: `… · Ebene a → b`) | Autor der Fassung / Mensch | mit anderer Ebene: mitgestufte Punkte mit neuen Fassungen, Umnummerierung | ja |
| Ändern | `Fassung n geändert` | Mensch / Mensch | Zeilen-Diff, Notiz vorher/nachher | ja |
| Löschen | `Fassung n gelöscht` | Mensch / Mensch | ganzer Text, Autor, Status, Elternteil, Notiz, „Umgehängt: …“ | ja |

Trailer: `Persona`/`Model` nur bei einem Assistenten als Autor, immer `Baruch-Node` und `Baruch-Revision`,
bei Umschalten, Ändern und Löschen dazu `Baruch-Action: switched|updated|deleted`. Umschalten committet auch
bei gleichem Text, damit jede Umschaltung genau einen Commit ergibt. Die Dateien bleiben der Schnappschuss
von `Store::mirror_snapshot`: Ändern einer nicht aktuellen und Löschen einer Fassung sind leere Commits.

Umschalten und Übernehmen spiegelt `mirror::record_switch` (genau ein Commit, auch wenn Unterpunkte
mitgestuft werden); bringt die Fassung eine andere Ebene mit, trägt der Commit immer `Baruch-Action: switched`,
auch beim Übernehmen eines Vorschlags.

## Gliederung

Migration `0006_outline.sql` (baruch-store):

- `node.numbered` (0/1, Vorgabe 1): zählt der Knoten bei der Nummerierung mit? Beim Umstieg 0 für Knoten mit
  leerer Nummer (Vorwort des Beispielwerks); der Trigger `node_numbered_default` setzt 0 für jeden neu
  eingefügten Knoten mit leerer Nummer (Saat). Der Store legt neue Knoten mit vorläufiger Nummer an.
- `work.numbering`: eigenes Muster in der Mustersprache (`baruch_typst::scheme`), `''` = keine Nummern,
  `NULL` = aus der Werkvorlage – gelesen nur beim Schema `custom`; bei anderen Schemata bleibt es stehen
  (Zurückschalten stellt es wieder her).
- `work.numbering_scheme` (Migration 0008): `template` (Vorgabe) | `decimal` | `decimal_dot` |
  `alphanumeric` | `chapter_roman` | `chapter_arabic` | `parts_chapters` | `chapter_words` | `legal` |
  `sermon` | `book` (seit 0040) | `none` | `custom` (CHECK). Beim Umstieg: `numbering` NULL → `template`, `''` → `none`, sonst
  `custom`; keine andere Zeile oder Spalte ändert sich (Test `tests/migration_0008.rs` auf einer
  nachgebauten Datenbank im Stand 0001–0007 + 0101–0105).
- **Migration 0040 (Schema „Buch“, `0040_numbering_scheme_book.sql`):** Der CHECK lässt `book` zu. SQLite ändert
  keinen CHECK; `work` wird nach dem offiziellen Verfahren umgebaut: `work_new` mit allen Spalten aus 0001,
  0002, 0005–0008 (gleiche Reihenfolge, AUTOINCREMENT, CHECK auf `kind`), Zeilen kopieren, `DROP TABLE work`,
  `ALTER TABLE work_new RENAME TO work`, die Indizes `work_source_sha256` und `work_title_key` neu (UNIQUE(slug)
  kommt mit der Tabelle); `sqlite_sequence` behält den größten je vergebenen Wert (auch eines gelöschten Werks).
  An `work` hängen sonst keine Trigger, Views oder FTS-Bezüge (Beleg im Test). Weil viele Tabellen mit
  ON DELETE CASCADE bzw. SET NULL auf `work(id)` verweisen (`outline_request`, `asset_request`, `work_book`,
  `work_rag`, `work_rag_collection`, `work_bible`, `sync_work`, `work_sword_source`, `chat_thread`, `work_timing`;
  `idea` SET NULL), muss `DROP TABLE` ohne Fremdschlüssel laufen – mit ihnen löschte das implizite DELETE diese
  Zeilen, und `defer_foreign_keys` (Weg von 0007) verhindert Aktionen nicht. sqlx 0.8.6 führt jede Migration in
  einer Transaktion aus, in der `PRAGMA foreign_keys` wirkungslos ist; darum läuft `MIGRATOR` über
  `baruch_store::migrate`: Für jede Version aus `FOREIGN_KEYS_OFF` (bisher nur 40) erst alle älteren Migrationen
  normal, dann diese eine auf einer eigenen Verbindung mit `PRAGMA foreign_keys = OFF` (vor der Transaktion),
  danach wieder `ON` (sonst wird die Verbindung geschlossen), zuletzt der Rest. Die Migration prüft selbst, dass
  die Fremdschlüssel aus sind (sonst Abbruch ohne Änderung), und am Ende `pragma_foreign_key_check` (ein Verstoß
  rollt alles zurück). Test `tests/migration_0040.rs`: Datenbank im Stand 0039 + 0101–0106 mit Zeilen in jeder
  Tabelle, die auf `work` verweist; jede Zeile, jedes andere Schemaobjekt und jeder Verweis (samt Aktion) bleibt,
  `foreign_key_check` leer, `integrity_check` ok, alle Schemata samt `book` speicherbar, Unbekanntes abgelehnt.
- **Maskierungen:** `node.title` wird aufgelöst (verschachteltes `replace()`: erst `\\` → `char(1)`, dann
  `\X` → `X` für `' " # * _ $ @ < > [ ] ` ~ = - / + .`, dann `char(1)` → `\`), genau wie
  `baruch_typst::unescape_title`. Revisionstexte bleiben unverändert: Die Überschriftszeile im Text bleibt
  wörtlich, `Node.title`, Anzeige, slug und Commit-Betreffe nutzen den aufgelösten Titel. Neue
  Überschriftszeilen maskiert `baruch_typst::escape_title` (Backslash und `# * _ $ @ < > [ ] ` ~`, `/` vor `/`
  oder `*`); `heading_line` behält Label und Kommentar der vorigen Zeile. `Heading` aus `parse` trägt beide
  Titel (`title` aufgelöst, `raw_title` wörtlich).

**Ebenen und Gültigkeit** (`baruch-store/src/outline.rs`): Teil 0 (nur oberste Ebene, seit Runde 14 mit der
Teil-Markierung als Überschriftszeile, in jedem Werk anlegbar, nie umstufbar), Kapitel 1, Abschnitt 2,
Unterabschnitt 3 (Abschnitt unter Abschnitt). Die Gliederung ist die Ebenenfolge in Baumreihenfolge, das
Elternteil wie beim Import der nächste Vorgänger mit kleinerer Ebene. Gültig: jede Ebene höchstens eine tiefer
als die vorige (der erste Nicht-Teil also ein Kapitel), keine tiefer als 3, nie in den eigenen Teilbaum.
Verschieben behält die Ebene (Runde 5): `before`/`after` nur neben ein Ziel derselben Ebene (`after` hinter
dessen Unterpunkte), `into` nur unter ein Ziel genau eine Ebene höher (letzter Unterpunkt); sonst `Refused`
„Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).“ (`outline::MOVE_KEEPS_LEVEL`,
dasselbe Urteil in `judge()` von `outline.ts`). Unterpunkte wandern unverändert mit.
Ungültig → `StoreError::Refused` (-32000), nichts geändert.

**Teile (Runde 14, R14b):** Keine Schemaänderung – ein Teil bleibt `node.kind = 'part'`, Ebene 0, `parent_seq`
NULL; seine Kapitel sind wie immer die folgenden Punkte bis zum nächsten Teil (`parents`). Text eines neuen
Teils (Revision 1) ist `heading_line(0, Titel, None) + "\n"` = Teil-Markierung
(`baruch_typst::part_line`, Schnittstelle A in `baruch-typst/src/marker.rs`) und Leerzeile, Autor wie bei
jedem Anlegen der Mensch, die Persona oder `@mcp:<client>`. `add_many` erlaubt Ebene 0 vor/hinter einem Teil
oder Kapitel (auch als ersten Punkt neben einem Kapitel und mitten im Block; nicht `into`, nicht neben einem
Abschnitt); die Sperre „nur ohne Werkvorlage“ ist entfallen, `add` (RPC ohne `level`, Einfälle) läuft über
`add_many`. `OutlineAdded.adopted` = Schlüssel der vorhandenen Kapitel, deren Elternteil danach ein neuer
Teil ist; ihre Texte und Nummern bleiben (`load::numbers` zählt Kapitel über Teile hinweg). Umbenennen
(`apply`, `rewrite_part`): Markierung am Anfang → nur ihre Zeile neu (`heading_line(0, …, Some(alt))`, der
Kommentar bleibt), leerer Text → Markierung, sonst (selbst geschriebener Text; Altbestand „= Teil …“ wandelt seit
2026-11-07 der Startschritt um, siehe „Teile im Typst-Text“) bleibt der Text und
`Applied.kept_parts` lässt das Detail es sagen. Speichern (`save_part`), `check_part`, Vorschläge
(`check_proposal_heading`), `revision.save`/`update` (`check_current_heading`) und Umschalten
(`follow_switched`) gleichen nur Teile **mit** Markierung ab (`part_problem`: genau eine Markierung am Anfang,
keine weitere Überschrift; Speichern und Vorschlag verlangen sie auch, wenn die aktuelle Fassung eine trägt);
`sync_heading` übernimmt den Titel aus der Markierung (`heading_level` liefert für Teile Ebene 0). Eine
Teil-Markierung als erste Überschrift eines Kapitels (Speichern, Umschalten) → abgelehnt (`TO_PART`).
Ebenenwechsel ohne neuen Titel über `baruch_typst::relevel_line` (eine Markierung „ohne Nummer“ bleibt
Markierung); beim Teilen zählt ein Stück mit Markierung „ohne Nummer“ ohne Nummer (`Analysis.numbered`).

**seq-Neuvergabe:** Jede Änderung (`outline_move`, `outline_set_level`, `outline_rename`, `outline_add`,
`outline_delete`, `outline_save` mit Teilen/Zusammenführen) läuft in einer Transaktion: Gliederung in
Baumreihenfolge lesen (Tiefensuche, Geschwister nach `position`), als Liste umstellen und prüfen, dann
`seq`, `parent_seq`, `position`, `kind` neu schreiben – zweistufig (`seq = -seq - 1`, dann die neuen Werte)
wegen `UNIQUE(work_id, seq)`. `post_ref.node_seq` wird ebenso zweistufig umgeschrieben, Verweise auf
gelöschte Knoten entfallen. `node.number` wird mitgeschrieben. Danach bekommt jeder Knoten mit Text, dessen
Ebene oder Titel sich geändert hat, eine neue Fassung des Menschen (aktuell, Elternteil = bisherige aktuelle),
die sich nur in der Überschriftszeile unterscheidet. `seq` bleibt 0-basiert, lückenlos und gleich der Stelle
in Baumreihenfolge; `load.rs` prüft das weiter.

**Fester Schlüssel:** Weil `seq` sich bei jeder Änderung ändert, trägt `baruch_core::Node` zusätzlich `key`
(= `node.id`, keine Migration nötig). `Store::node_seq(work, key)` liefert die Stelle jetzt (gelöscht →
`StoreError::GoneNode`). Im Server übersetzt `rpc::resolve_keys` `key` → `node` und `target_key` → `target`
unter `AppState::write_lock` (RPC, `/act`, `/gliederung/act`); der Schlüssel geht vor der Stelle. Alle
Formulare (`tree.html`, `outline*.html`, `editor.html`, `assistants.html`) und `outline.ts` schicken ihn mit.
Seit Runde 5 ist er beim Schreiben über MCP (`propose_revision`, `add_post` samt `refs`) und beim Zitieren
(`highlight.cite`, `/literatur/act` mit `cite`) **Pflicht**: `node` wird dort abgelehnt (MCP und RPC -32602),
ohne `key` ebenso (Formular: 422 „Abschnitt fehlt.“). Die Auflösung läuft unter der Schreibsperre (MCP in
`mcp/tools.rs`, Zitieren in `literature::cite`). Links `/?node=` bleiben reine Anzeige.

**Löschen ganzer Knoten:** In derselben Transaktion erst die aktuelle Fassung `current → superseded` (laut
0004 erlaubt), dann Revisionen, Verweise der Beiträge, Beiträge und den Knoten löschen. So bleibt der Trigger
`revision_no_delete_current` für lebende Abschnitte wirksam. Zusammenführen hängt den Text an den vorigen
Punkt (neue Fassung dort), verschiebt die Beiträge dorthin (Stellen hinter die vorhandenen) und löscht den
Knoten ebenso.

**Nummerierung** (`load.rs`, beim Laden berechnet, `load::work_numbering`): Schema `work.numbering_scheme`
→ `baruch_typst::scheme::Numbering` (`Numbering::of_scheme`; `custom` liest `work.numbering`, `template`
`baruch_typst::heading_numbering(template)`, sonst „1.1“). `heading_numbering` liest nur Regeln der obersten
Ebene (Scanner der Gliederung, `#` mit leerem Klammerstapel), die **letzte** mit `numbering`, und gibt die
Mustersprache zurück: String wörtlich, `none` → `""`, eine Funktion (direkt oder über den letzten
`#let`-Namen davor) wird je Ebene 1–6 symbolisch ausgewertet (`scheme::read_function`: Tiefenbedingungen
`X.pos().len() OP k`, `let p = n.pos()`, Text, `numbering("P", ..n)`, `numbering("P", p.at(i))`,
`* 2`, `+`); nicht auswertbar → wie bisher das erste `numbering("MUSTER"` der Funktion. `load::numbers`
zählt Teile eigen (`Numbering::part`), Kapitel über Teile hinweg fortlaufend, Unterzähler je Elternteil
und gibt die Zählerstände `[kapitel, abschnitt, …]` an `Numbering::heading` (dazu die Zahl der nummerierten
Kapitel für „Schluss“ der Predigt); Knoten mit `numbered = 0` zählen nicht und haben keine Nummer.
`Store::set_numbering_scheme` (eine Transaktion, `OutlineAction::Numbering`, unverändert → `Refused`,
unbekanntes Schema/ungültiges Muster → `StoreError::Numbering`) schreibt `node.number` neu;
`Store::set_numbering` bleibt (None → `template`, `""` → `none`, sonst `custom`). `Store::numbering_preview`
und `Store::numbering_rule` lesen nur. `NumberingInfo` trägt zusätzlich `scheme`, `scheme_label`, `custom`.

Beispielwerk: „I, 1, 2, 2.1, 2.2, II, 3, 3.1, 4, 4.1“, Vorwort ohne Nummer; `baruch_typst::format_number` wendet ein Muster nach den Typst-Regeln an (Zählzeichen `1 a A i I *`, Präfix, Trenner, Suffix, das letzte Zählzeichen wiederholt sich für tiefere Ebenen).

**Export und Spiegel mit Gliederungsschema** (`snapshot.rs`): Die Vorlage für `export_typst` und `main.typ`
kommt aus `baruch_typst::scheme::export_template(template, scheme, numbering)`: bei `template` unverändert
(byte-gleich wie vor Runde 6), sonst Vorlage + (Zeilenende, falls nötig) + `// Baruch: Gliederungsschema
„<Name>“` + `Numbering::heading_rule()` (eine Zeile `#set heading(numbering: …)`). Ohne Vorlage steht die
Regel in `main.typ` hinter `// Baruch: <Titel>`. Knoten mit `numbered = 0` setzt der Export seit Runde 14
nicht mehr über Show-Regeln ab, sondern projiziert ihre erste Überschriftszeile auf die Markierung
`<baruch-unnumbered>` (siehe „Kapitel ohne Nummer in Typst“ weiter unten, `snapshot::project`, auch bei „aus der
Werkvorlage“); nur wo das nicht geht (eigenes Label, Titel mit Code wie `#emph[…]`) bleibt
`// Baruch: Überschriften ohne Nummer` mit je einer Zeile
`#show heading.where(level: L, body: [Titel]): set heading(numbering: none)` als Rückfall
(`snapshot::fallback_rules`). Die Texte in SQLite bleiben unverändert. `vorlage.typ` bleibt die gespeicherte
Vorlage. Für die musterbasierten Schemata (`decimal`, `decimal_dot`, `chapter_roman`, `chapter_arabic`, `alphanumeric`,
`custom`) liest `heading_numbering` aus dem Export wieder dieselben Nummern (Re-Import ergibt dieselbe
Gliederung); die Regeln der Schemata mit Wörtern sind Typst-Funktionen (`"§ " + str(p.at(0))`,
Ordnungswörter-Array, `context` + `counter(heading).final()` für „Schluss“), die Baruch beim Re-Import nur
näherungsweise liest. Die erzeugten Regeln prüft seit Runde 14 Typst selbst
(`baruch-render/tests/schemes.rs`, siehe „Prüfung gegen Typst“ unten).

**Überschriften im Text:** `outline_check` (nur lesen) und `outline_save` zerlegen den Text mit
`baruch_typst::parse`: eine Überschrift → speichern und Titel/Ebene angleichen (Unterpunkte wandern mit),
weitere → mit Bestätigung neue Punkte dahinter (Revision 1 des Menschen mit ihrem Stück), keine → mit
Bestätigung an den vorigen Punkt anhängen; Text vor der ersten Überschrift → `Refused`. Teile speichern ohne
Gliederungslogik. `outline::check_current_heading` läuft nach `Store::save_human`, `Store::save_human_on` und
`Store::update_revision` der aktuellen Fassung in derselben Transaktion: Trägt der Text nicht genau eine
Überschrift der Ebene des Punkts am Anfang → `StoreError::Heading` (-32000, zurückgerollt); sonst übernimmt
`outline::sync_heading` den Titel. `Store::add_proposal` prüft vorher ebenso (`check_proposal_heading`), also
auch MCP und `highlight.cite`. Ebene, Teilen und Zusammenführen gibt es auf diesen Wegen nicht.

**Umschalten bringt die Ebene mit** (Runde 5, Nutzerentscheidung 2026-11-05 „Nein“: eine Fassung wird beim
Umschalten nie umgeschrieben): `Store::switch_to` ruft statt `check_current_heading`
`outline::follow_switched`. Gleiche Ebene → nur `sync_heading` (Titel), `outline: None`. Andere Ebene → Punkt
bekommt Ebene und Titel der Fassung (`text_done`, sein Text bleibt), alle Unterpunkte wandern um dieselbe
Differenz (käme einer tiefer als 3, `StoreError::Heading` statt Kappen – Kappen machte ein Kind zum
Geschwister, und Zurückschalten stellte es nicht wieder her), dann `apply` wie bei `outline_set_level`: neue Fassungen
des Menschen nur für die tatsächlich umgestuften Unterpunkte, nur die Überschriftszeile geändert;
`OutlineChange { action: Level, detail: "„T“ Ebene a → b" bzw. "„Alt“ → „Neu“ umbenannt, Ebene a → b" }`.
Abgelehnt (`StoreError::Heading`, -32000, alles zurückgerollt, auch das Umschalten): keine Überschrift, weitere
Überschriften, Text davor (Hinweis auf „Speichern“ im Editor), eine an der Stelle ungültige Ebenenfolge.
Teile haben keine Überschriftszeile und schalten ohne Gliederungsabgleich um. Teilen und Zusammenführen gibt es
nur über `outline_*`; Gliederung und Überschriftszeilen bleiben gleich, Export und Import ergeben dieselbe
Gliederung.

**Titelregeln (Runde 7):** `baruch_store::check_title` (in `src/outline.rs`, mit `MAX_TITLE_CHARS = 200`):
Rand-Leerraum (`char::is_whitespace`) weg; leer, ein Zeichen mit `baruch_typst::is_newline` oder mehr als 200
Zeichen (`chars`) → `StoreError::Title(<Meldung>)` → RPC -32602, MCP `isError`, Formular 422. Angewandt in
`rename`, `add`, `save` (erster und jeder geteilte Titel, vor jedem Schreiben), `check` (`valid: false`,
`message`), `check_current_heading` (`revision.save`, `revision.update` der aktuellen), `check_proposal_heading`
(`add_proposal`: MCP `propose_revision`, `highlight.cite`) und `follow_switched` (Umschalten/Übernehmen), immer
in der Transaktion des Aufrufers (Ablehnung = Rückrollen). `clean_title` (Zeilenumbruch → Leerzeichen) gibt es
nicht mehr. Import und Export prüfen keine Titel (byte-gleich); vorhandene leere Titel bleiben lesbar.

**Satzbild und Titel:** `baruch_wysiwyg::emit`/`serialize` beenden eine Überschrift am ersten Zeilenumbruch der
obersten Ebene (`emit::split_heading`); der Rest wird ein Absatz dahinter. So liefern nur echte
Überschriftszeilen Titel. Ursache des Fehlers „Absatz wird Titel“ (Runde 7): `emit` machte aus `Inline::Soft`
in Überschriften ein Leerzeichen, also eine einzige Zeile `== Titel Absatztext …`, die `outline.save` als Titel
übernahm (Test `crates/baruch-server/tests/round7_titles.rs`). Der Abgleich im Store (`sync_heading`,
`follow_switched`, `rewrite_heading`) nahm schon nur echte Überschriftszeilen.

**Schema-Name im Kopf:** `NumberingInfo.label` = Name des Schemas, dessen Muster dem wirksamen entspricht
(`baruch_typst::scheme::Scheme::matching(pattern)`, nur Schemata mit fester Mustersprache, nie Vorlage/Eigenes/
Ohne), sonst das Muster, ohne Nummern „keine“, bei Schemata mit Wörtern deren Name. `fragments/tree.html` zeigt
„Nummern: {{ numbering.label }}“, das Muster nur im `title`.

**Server** (`baruch-server/src/outline.rs`): RPC `outline.*`, Seiten `/gliederung` und `/gliederung/bearbeiten`
mit eigener Template-Umgebung (`outline.html`, `outline_edit.html`, `outline_confirm.html`), Formulare an
`POST /gliederung/act` (hinter `guard::same_origin`). Schreibende Methoden unter der Schreibsperre, danach
`mirror::record_outline`: genau ein Commit „Gliederung: …“ (`allow_empty`), Autor und Committer der Mensch,
Trailer `Baruch-Action: outline-*` und `Baruch-Node`, Rumpf mit neuen Fassungen, Umnummerierung und vollem
Text gelöschter Punkte. Linke Spalte (`fragments/tree.html`), die einzige Gliederung der Oberfläche: Kopf mit Nummerierungsformular
(`form[data-outline-numbering]` an `/gliederung/act`, Feld `numbering` = `vorlage` | `keine` | Muster); je
Eintrag Formulare ↑ ↓ ← → (Knöpfe in der Zeile über das Attribut `form`), Knopf „⋯“ (`js-only`) und die
Rückfall-Links „bearbeiten“/„Bearbeiten“ (`nojs-only`, mit JavaScript per CSS ausgeblendet: `.js .nojs-only`);
ein verborgenes Kontextmenü `[data-outline-menu]`. Einträge (`a[data-nav]`) zeigen immer auf den Editor, nie
auf `view=preview`. Widget `outline` (`web/src/outline.ts`): Ziehen und Ablegen, Alt+Umschalt+Pfeile (außer in
Eingabefeldern), Kontextmenü (Rechtsklick, Umschalt+F10, Kontextmenü-Taste) mit Umbenennen an Ort und Stelle
(Doppelklick auf den gewählten Eintrag oder F2; Enter → `outline.rename`, Esc bricht ab), Anlegen mit
Eingabefeld (`outline.add`), Umstufen, Löschen (`confirm()` → `outline.delete {confirm: true}`, Entf). Jede
schreibende Aktion: `leaveEditor()` → RPC → `baruch:navigate {url, asked: true}`. Vor dem Speichern im Editor
löst es `baruch:before-save {wait}` am Formular aus, wartet auf `detail.wait`, liest dann das Formular und fragt
`outline.check` mit `confirm()`. `view.rs` gibt `node.key` und die Nummerierung (`rpc::LibraryView::numbering`)
in den Kontext, dazu die Zahl offener Anträge (`LibraryView::outline_requests`, Hinweis im Kopf des Baums).

### Nummerierung je Ebene (Runde 14)

**Eine Quelle:** `baruch_typst::scheme::Numbering` rechnet die Nummern für Baum, Gliederung, Editorkopf und
Vorschau (`Numbering::heading`/`part` über `store/load.rs::numbers`), schreibt die Regel für Export und Satz
(`heading_rule` über `export_template`) und liest sie beim Re-Import wieder (`Numbering::from_template`;
`of_scheme(Scheme::Template, _, t)` = `from_template(t)` oder „1.1“). `from_template` sucht wie
`heading_numbering` die letzte `#set heading(numbering: …)` der obersten Ebene (private Hilfen aus `lib.rs`:
`blank_comments`, `top_level_hashes`, `Cursor::arguments_at`, `expression_end`) und unterscheidet:

- String → wörtliches Typst-Muster (`Pattern::literal`: jedes Zählzeichen einzeln wie `NumberingPattern` in
  Typst 0.15, kein verdoppelter Buchstabe, `;`/`|` Text); `none` → aus;
- Ausdruck gleich `rule_value()` von „Teile und Kapitel“, „Kapitel ausgeschrieben“, „Juristisch“, „Predigt“ oder „Buch“
  → genau dieses Schema (mit seiner Teile-Zählung; „Buch“ setzt dieselben Kapitelnummern wie „Predigt“, hat aber
  absichtlich eine andere Regel – `Kind::Book`, `p.first()` statt `p.at(0)` –, damit es samt Wort-Teilen
  zurückkommt);
- auswertbare Funktion → `Kind::Function { node, approx }`: der Leser (`Reader`) baut einen Baum aus
  `Node::If` (Tiefenbedingung mit `==, !=, <, <=, >, >=`) und `Node::Value` (Verkettung von `Term::Text`,
  `Term::Hier` = `numbering(P, ..p)`, `Term::Num` = `numbering(P, p.at(i))` bzw. `first()`/`last()` mal k,
  `Term::Str` = `str(p.at(i))`; `k` höchstens 16, auch als Produkt – sonst gilt die Funktion als nicht lesbar,
denn die Auswertung wiederholt wirklich und eine riesige Zahl aus einer fremden Vorlage brächte den Prozess
beim Import und bei jedem Laden zum Abbruch); ausgewertet wird je Pfad mit `format_number` (= Typst). Auswertbar heißt:
  ganz gelesen, jedes Muster mit Zählzeichen, auf jeder Ebene 1–6 jede Stelle vorhanden (sonst bräche Typst
  ab). `approx` = bisherige Näherung `heading_numbering` – `spec()` liefert sie, damit Anzeige und
  Gliederungskopf (`store/outline.rs`) unverändert bleiben; `heading_rule()` schreibt die Funktion als Kette
  je Ebene zurück;
- sonst die Näherung wie bisher (`lenient(heading_numbering)`).

`read_function`/`shown` für `heading_numbering` sind unverändert in ihrem Ergebnis (neue Terme wie `str()` und
`last()` gelten dort als nicht lesbar, wie vorher).

**Kapitel ohne Nummer in Typst** (`store/snapshot.rs::project`): Ist die wirksame Nummerierung nicht aus, wird
die erste Überschriftszeile jedes Nicht-Teil-Knotens, dessen `node.numbered` nicht zur Textform
(`Heading.numbered`) passt, mit `baruch_typst::set_numbered_line` umgestellt – in `export_parts` (Export,
Export-Karte, Satz, PDF) und `files` (Spiegel). Nur wo das nicht geht (eigenes Label; Titel mit Code, also
unmaskiertem `#` außerhalb von Inline-Raw wie `#emph[…]` – ein Backslash vor den Klammern des Aufrufs machte
daraus Raw-Text) bleibt die Show-Regel
`heading.where(level, body)` als Rückfall hinter der Vorlage (auch bei „aus der Werkvorlage“), außer ein
nummerierter Knoten trägt dieselbe Überschrift. `counted_headings` nimmt dieselbe Projektion: es zählen die
Überschriften der Ebene 1 mit nummerierter Textform ohne Show-Regel. Die Texte in SQLite bleiben unverändert;
ein frisch importiertes Werk (Import setzt `numbered` aus der Textform, R14a) bleibt byte-gleich. Der Trigger
`node_numbered_default` aus Migration 0006 (leere Nummer beim Einfügen → `numbered = 0`, für die Saat des
Vorworts) gilt beim Import nicht mehr: `import.rs` setzt `numbered` danach ausdrücklich aus der Textform, sonst
kämen Ebenen ohne angezeigte Nummer (Predigt: Abschnitte „“) als „ohne Nummer“ zurück und der Export
projizierte sie (Integration Runde 14, Test `round14_numbering`, `round14`). Auch Werke „aus der Werkvorlage“
ohne Nummerierungsregel werden projiziert (die wirksame Nummerierung ist dann die Vorgabe „1.1“): Im Satz
ändert das nichts, aber Export → Import behält so „ohne Nummer“ (Entscheidung des Integrators; das
Beispielwerk exportiert sein Vorwort darum als Markierung).

**Prüfung gegen Typst:** `baruch-render/tests/schemes.rs` setzt alle dreizehn Schemata (dazu eigene Muster mit
Liste, Tiefe und verdoppeltem Buchstaben) mit Teil-Markierungen und Markierungen „ohne Nummer“; nach jeder
nummerierten Überschrift steht `#baruch-check("…")` mit der Nummer aus `Numbering::heading`, das im Kontext
`numbering(heading.numbering, ..counter(heading).get())` vergleicht (`assert.eq`; liefert die Funktion Inhalt
wie bei „Predigt“ und „Buch“ mit `context`, vergleicht es die gesetzte Breite). Grenzen: Baruch kennt die Zählzeichen
`1 a A i I *`; andere Typst-Zählzeichen (z. B. `α`, `一`) sind für Baruch Text.

### Gliederung über MCP und Anträge (Migration 0012)

Wunsch 2026-11-06: Assistenten dürfen anlegen, alles andere nur beantragen.

- **Anlegen:** `Store::outline_add_many(work, target?, place, &[NewPoint], &Author)` – ein Block in einer
  Transaktion über dieselbe `apply` wie alle Gliederungsänderungen; `apply` nimmt dafür den Autor
  (`baruch_core::Author`) der Revision 1 neuer Punkte statt des Namens des Menschen. Bestehende Fassungen
  schreibt nur ein menschlicher Autor um (sonst interner Fehler; Anlegen schreibt keine um). Ergebnis
  `OutlineAdded {change, points: [AddedPoint {node, key, level, title}], adopted: [key]}` (Runde 14: auch Teile,
  `adopted` siehe „Teile“ oben). MCP `outline_add`
  (`mcp/outline.rs`) spiegelt mit `mirror::record_outline_by(…, OutlineBy::Direct(origin))`: Autor die Persona
  bzw. der Client, Committer der Mensch, Trailer `Baruch-Origin`, `Persona`.
- **Anträge:** Tabelle `outline_request` (`0012_outline_request.sql`: `work_id` mit `ON DELETE CASCADE`, `op`,
  `params` JSON `{op, expected}`, Herkunft, `note`, `created`, `decided`, `decision` confirmed/rejected/stale,
  `decided_by`, `message`). `Store::outline_request` löst die Schlüssel auf, prüft im Sicherungspunkt
  (`sqlx::Connection::begin` auf der Transaktion = SAVEPOINT, danach zurückgerollt), ob die Änderung jetzt
  ginge, und speichert Antrag und Stand (`Expected`). `Store::outline_request_decide` prüft erst den Stand
  (`staleness`), führt dann die Gliederungsfunktion im Sicherungspunkt aus; eine fachliche Ablehnung wird
  `stale` mit Meldung, sonst `confirmed`. Der Server spiegelt mit `OutlineBy::Confirmed(request)` (Mensch als
  Autor/Bestätiger, Trailer `Baruch-Origin`, `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`);
  Ablehnen und `stale` schreiben keinen Commit (außer beim Antrag `add`, siehe unten).
- **Vorschau (Runde 14):** `Store::outline_request_preview(work, id)` öffnet eine Transaktion
  (`BEGIN IMMEDIATE`), prüft „offen“ (sonst `Refused`) und `staleness` (sonst `Heading("Veraltet: …")` –
  `Refused` trägt nur feste Texte, eine neue Fehlerart verlangte `rpc.rs`), liest `outline::point_states`
  (Schlüssel, Ebene, Nummer, Titel, numbered), führt `run` im Sicherungspunkt aus, liest erneut und rollt beides
  zurück. `preview_points` vergleicht nach Schlüssel: `Moved` für den verschobenen Punkt samt Unterpunkten,
  sonst `Renamed`/`Level`/`Numbered`/`Same`, gelöschte Punkte als `Deleted` hinter ihrem alten Vorgänger. Der
  Server (`outline::add_preview`) setzt `preview` bzw. `preview_error` in die Antragsform; RPC
  `outline.requests`, MCP `outline_requests` (je `preview: true`) und die Seite `/gliederung` (`<details>` je
  offenem Antrag) nutzen es.
- **Zustimmung zu einem neuen Teil (Nutzerfrage 12, Migration 0035):** MCP `outline_add` ruft
  `Store::outline_add_by_assistant(work, target?, place, points, origin, model, note)`: `add_many` im
  Sicherungspunkt; ist `adopted` leer, gilt es (`AssistantAdd::Added`), sonst Zurückrollen und Antrag
  `OutlineOp::Add {key, place, points, model}` mit `Expected.adopted` (`AssistantAdd::Requested`). `run` führt ihn
  beim Zustimmen mit dem Antragsteller als Autor von Revision 1 aus; liefert `add_many` andere `adopted` als beim
  Antrag, wird er `stale` (Vorschau: `preview_error`). Vorschau-Marken `Added`/`Adopted`. 0035 legt
  `outline_request` mit `op … 'add'` neu an (SQLite ändert CHECK nicht), übernimmt alle Zeilen samt id und die
  AUTOINCREMENT-Zählung; Test `migration_0035.rs`. Ablehnen bzw. `stale` eines Antrags `add` spiegelt
  `mirror::record_request_decision` (leerer Commit, `Baruch-Action: outline-request`, `Baruch-Decision`).
  Oberfläche: `base.html` trägt die Marke `<!--baruch-consent-->`; die Schicht `consent::inject` (über allen
  Routen) liest HTML-Antworten, fragt nur bei vorhandener Marke `Store::outline_requests_needing_consent` ab und
  setzt `templates/consent.html` ein (Hinweisleiste mit Link, `<dialog data-widget="consent">` mit Vorschau und
  Formularen an `/gliederung/act`); so braucht keine der vielen Template-Umgebungen eigene Daten. Das Widget
  `web/src/consent.ts` öffnet den Dialog modal und entscheidet über RPC `outline.request_decide`.
- **Ohne Nummer:** `Store::outline_set_numbered` setzt `node.numbered` und schreibt alle Nummern neu
  (`OutlineAction::Numbered`, Trailer `outline-numbered`); RPC `outline.set_numbered`, Knopf auf `/gliederung`,
  über MCP als Antrag `set_numbered`. Einen Teil ohne Nummer lehnt sie ab (`Refused`, auch im Probelauf eines
  Antrags): Die Teil-Markierung trägt kein Nummer-Merkmal, `parse` liefert Teile immer nummeriert, Export →
  Import zählte ihn also wieder und verschöbe die folgenden Teilnummern. Zurück auf „mit Nummer“ geht.

## Bilder (Migration 0009)

- **Tabellen:** `asset` (Name je Werk eindeutig, `last_rev`), `asset_path` (Pfad aus Vorlage/Text →
  Bild, Schlüssel `(work_id, path)`), `asset_revision` (Fassungen). Der Inhalt ist immer **TEXT**
  (`CHECK (typeof(content) = 'text')`): Base64 (Standardalphabet, mit `=`, ohne Zeilenumbrüche,
  `encoding = 'base64'`) oder SVG-Text (`'svg'`, nur bei `image/svg+xml`). `bytes` und `sha256` gelten für
  die entschlüsselten Bytes. Grenze `baruch_store::MAX_ASSET_BYTES` = 8 MiB je Fassung.
- **Fassungen wie Abschnitte:** genau eine `current` je Bild (Teilindex `asset_one_current`), Umschalten
  `superseded ↔ current`; Trigger halten Herkunft (`asset_id`, `number`, `author_name`, `created`) und
  Inhalt (`mime`, `encoding`, `content`, `bytes`, `sha256`) unveränderlich, verbieten das Löschen der
  aktuellen Fassung, hängen Kinder einer gelöschten an deren Elternteil und vergeben keine Nummer
  zweimal (`asset.last_rev`). Ändern nur Bildunterschrift, Alternativtext und Notiz (setzt `edited`).
- **Typ** aus den ersten Bytes (`baruch_store::detect_mime`): PNG, JPEG, GIF, WebP, SVG (UTF-8, `<svg`
  nach optionalem BOM, Prolog, Kommentaren, DOCTYPE). Name: `^[a-z0-9]([a-z0-9._-]{0,78}[a-z0-9])?$` mit
  zum Typ passender Endung (`valid_asset_name`, `asset_name_from_file`).
- **Store** (`crates/baruch-store/src/assets.rs`): `asset_list`, `asset_revisions`, `asset_data`,
  `asset_by_path` (lesen); `add_asset`, `replace_asset`, `set_current_asset`, `update_asset_revision`,
  `delete_asset_revision`, `delete_asset`, `set_asset_paths` (je eine Transaktion `BEGIN IMMEDIATE`, Ergebnis
  `AssetChange` für den Spiegel); `export_assets`; `import_typst_with_assets` (Werk und Bilder in **einer**
  Transaktion, gleiche Datei unter zwei Pfaden = ein Bild). Fehler `StoreError::UnknownAsset` und
  `StoreError::Asset` (zu groß, Typ, Name, Pfad, Base64) → `INVALID_PARAMS`; doppelter Name und „unverändert“
  → `Refused`. Werk löschen entfernt vorher alle Bildzeilen.
- **Spiegel:** `snapshot::files` hängt `assets::mirror_files` an (`bilder/<name>` bzw.
  `bilder/<name>.b64`, 76er-Zeilen). `mirror::describe_asset`/`record_asset`: ein Commit je Änderung,
  Betreff „Bild <name>: hinzugefügt / Fassung n ersetzt / auf Fassung n umgeschaltet / Fassung n geändert /
  Fassung n gelöscht / gelöscht“, Trailer `Baruch-Asset`, `Baruch-Revision`, `Baruch-Action: asset-added|
  asset-replaced|asset-switched|asset-updated|asset-deleted|asset-removed`. Umschalten, Ändern und Löschen
  auch als leerer Protokoll-Commit; Ändern mit vorher/nachher, Löschen mit Metadaten und dem gelöschten
  Inhalt (Base64 in 76er-Zeilen, SVG wörtlich).
- **Bildpfade im Text:** `baruch_typst::images::image_paths` (String-Literal als erstes Argument jedes
  `image(`-Aufrufs: `#image(` im Markup, `image(` nach `(`, `,`, `:`, `=`, `{`, `;`; nicht in Kommentaren
  und Raw; berechnete Pfade nicht).
- **Normalform der Pfade (Runde 7):** `baruch_store::normalize_asset_path(path) -> Option<String>` entfernt ein
  führendes `/` und `./`-Teile und fasst leere Teile zusammen; `..` oder leer → `None`. `check_asset_path`
  erlaubt damit `./` und `//` (nur `..`, Adressen, `\` und Steuerzeichen bleiben verboten), gespeichert wird der
  Pfad wie im Text. Verglichen wird überall in Normalform: `asset_by_path` (Seite `/werke/{id}/bilder/pfad?p=`),
  die Prüfung „Pfad gehört schon zu Bild …“ beim Anlegen, `export_assets` (also `export --out`),
  `wysiwyg::image_alts`/`image_ref`, der Satzbild-Diff der Vorschläge (`view.rs`) und der Import von der
  Kommandozeile (`read_asset`).
- **Satzbild:** `render_html_with` zeigt eine Insel mit Bild als `div.ws-block.ws-figure` (`data-kind="island"`,
  `data-image="1"`, `<img loading="eager">`, `span.ws-caption`) bzw. `span.ws-island.ws-inline-image`, ohne
  sichtbaren Rohtext (nur `data-raw`); ausgeliefert nur über die vorhandene Route (`nosniff`, CSP `sandbox`).
- **Import** (`crates/baruch-server/src/import.rs`, nur `import_file_as`): liest die Bilder in
  `spawn_blocking` relativ zum kanonischen Ordner der Quelldatei; `ImportReport.assets`/`.skipped`.
  `export_to_dir` schreibt `export --out` (nur relative Pfade ohne `..`, nichts überschreiben).
- **Server** (`crates/baruch-server/src/assets.rs`): RPC `asset.*`, Seiten und Formulare, eigene
  Template-Umgebung (`base.html`, `assets.html`, `asset.html`), Stil `static/assets.css`,
  `UPLOAD_LIMIT` = 8 MiB + 64 KiB. Ausgelieferte Bilder tragen `X-Content-Type-Options: nosniff` und
  `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox`.
- **Typwechsel und Altbestand (Runde 21c, ohne Migration):** `Store::replace_asset_renaming(…, rename)` (alte
  `replace_asset` = ohne `rename`) benennt bei anderem Format in derselben Transaktion um (`UPDATE asset SET name`,
  Vorgabe-Endung des Formats, vergeben → `free_name`), offene Anträge ziehen `asset_name` mit; Pfade hängen an der id
  und bleiben. `Store::asset_path_conflicts` gruppiert `asset_path` nach Normalform und meldet Gruppen mit mehr als
  einem Bild (`PathConflict {normal, paths: [PathOwner {path, asset}]}`); `Store::resolve_asset_path` hängt alle
  Schreibweisen per `UPDATE asset_path SET asset_id` an das gewählte Bild (ein `AssetChange::Updated`). Damit Typst den
  alten Pfad mit neuer Endung setzen kann, ergänzt `baruch_render` beim Satz `format: "<Format>"` in jedem
  `image("<Pfad>"…)`, dessen Datei nicht zur Endung passt (Erkennung an den ersten Bytes, ohne neue Zeile).

## Bilder, Umschlag und PDF (Runde 11, Migrationen 0017 und 0018)

- **Buchangaben (0017, `baruch-store/src/book.rs`):** Tabelle `work_book` (eine Zeile je Werk, keine Zeile =
  Vorgaben): `cover_asset_id`, `author_photo_asset_id` (Trigger `work_book_same_work_insert/update`: nur Bilder
  desselben Werks; Bild gelöscht → `NULL`), `title_area` (`top|middle|bottom`), `front_matter` (Titelei),
  `cover_pages` (Vorder-/Rückseite im Satz, Vorgabe an), Verlag … Kurzbiografie als Text, `paper_mm` (0, 1],
  `bleed_mm` [0, 10]; Werk gelöscht → Zeile weg. Untertitel, Autor und Bibelübersetzung bleiben in `work`.
  `BookMeta`, `BookEdit`, `book_meta`, `set_book_meta` (Grenzen: einzeilig 200 Zeichen, Klappentext 4000,
  Kurzbiografie 2000, Widmung/Rechtehinweis/Bildnachweis 1000; Jahr leer oder vier Ziffern; ISBN-13 mit
  Prüfziffer, verglichen über die Ziffern → `Refused`), `work_cover`, `set_work_cover`.
- **Anträge zu Bildern (0018, `baruch-store/src/asset_requests.rs`):** Tabelle `asset_request` nach dem Muster
  `outline_request`: `op` ∈ `replace|set_cover|delete`, `asset_name` (bei `set_cover` leer = Umschlag entfernen),
  `params` (JSON: replace `{mime, encoding, content, caption?, alt?, expected_sha256}`, delete `{rev?,
  expected_sha256}`, set_cover `{expected_cover}`), Herkunft nur `persona`/`mcp`, Index `(work_id, decided)`.
  `asset_request` prüft schon beim Antrag (unbekanntes Bild, Inhalt, Bildtyp, unverändert, gleicher offener
  Antrag), `asset_requests`, `open_asset_requests`, `asset_request_decide` (einmal; `Decision` aus
  `requests.rs`). Ausführen tut der Server.
- **Rahmen (`baruch-typst/src/cover.rs`, rein):** `frame_parts(info, opts) -> FrameParts {front, back,
  added_cover_pages}` – Umschlag-Vorderseite (vollflächig, `margin: 0`, Titel im Bereich oben/Mitte/unten), leere
  Innenseite mit `counter(page).update(0)` (Parität und Seitenzahlen wie ohne Umschlag; `COVER_PAGES` = 3 mit Rückseite),
  Titelei (Schmutztitel, Seite 2, Haupttitel, Impressum, Widmung), Rückseite (Klappentext, Kurzbiografie/Foto,
  ISBN mit EAN-13); ohne `heading` und `outline`, ohne sichtbare Seitenzahl. `wrap_source` setzt den Umschlag
  für die Druckerei auf eine Seite (Rückseite | Rücken, Text gedreht | Vorderseite, mit Beschnitt). Nutzertexte
  nur als Typst-String-Literale (`typst_string`, maskiert `\ " \n`), nie als Markup. Virtuelle Dateien unter
  `.baruch/` (`VIRTUAL_DIR`; Bild, Foto, Strichcode `.baruch/isbn.svg`), Marke `FRAME_MARK`.
- **Server `cover.rs`:** RPC `work.set_cover`, `work.meta.get`, `work.meta.set`, `work.cover_pdf`; `frame(state,
  work, source, map)` fügt `front` vor den ersten Abschnitt (ohne Abschnitte ans Ende) und `back` ans Ende und
  verschiebt Bytebereiche und erste Zeilen aller Abschnitte (Meldungen bleiben ihrem Abschnitt zugeordnet); ohne
  Umschlag(-seiten) und Titelei byte-gleich zurück (gleicher Stand, gleicher Zwischenspeicher). `cover_name`,
  `cover_info` (`{name, caption, alt, url, thumb}`), `assets_panel` (Block `id="umschlag"`), `cover_source`/
  `build_wrap` (Seitengröße aus der ersten Buchseite, Rücken = (Seiten − `COVER_PAGES` im Satz) × `paper_mm`).
  Seiten `/werke/{id}/umschlag`, `…/umschlag.svg` (CSP `sandbox`), `…/umschlag.pdf`; Formulare
  `/werke/umschlag/act`, `/werke/buchangaben/act`. Vorlagen `cover.html`, `fragments/cover_panel.html`, Stil
  `static/cover.css`; Einstellungen → Werk rendert die Gruppen über `cover::settings_blocks`
  (`WorkRetry::Book`). Spiegel: eigene Commits unter `write_lock` – „Buchangaben geändert: <Felder>“
  (`Baruch-Action: book-meta`), „Umschlag: <name> gesetzt“ (`cover-set`), „Umschlag entfernt“ (`cover-removed`),
  Rumpf vorher → nachher. Der Umschlag als `cover-image` im EPUB steht seit Runde 20a in `epub.rs`; seit Runde 21c
  ist es die gerasterte Vorderseite: `cover::front_cover_source` (Format aus `book_page_mm` der Werkvorlage, Anordnung
  über `resolve_fonts`) → `baruch_typst::cover::front_source_with_rules` (eine Seite im Endformat ohne Beschnitt:
  Hintergrund wie im Umschlag, Blöcke der Vorderseite bzw. ohne Anordnung `front_body`) → `epub::front_cover_png`
  (`baruch_render::render_png`, höchstens 1600 × 2560, Zwischenspeicher je Stand, `spawn_blocking`).
- **Satz und PDF:** `typeset::prepare` = `export_map` + `export_assets` → `cover::frame` → Stand. Satz-Ansicht,
  `work.pdf`, `/werke/{id}/pdf` und `export --pdf` nutzen dasselbe `prepare`, also gleiche Seitenzahl.
  `pdf.rs`: `PDF_TIMEOUT` = 120 s, `PdfKind {Book, Cover}`, `PdfOut`, `source_to_pdf` (je Werk höchstens eine
  Erzeugung – prozessweite Menge belegter Werke, `claim` –, dann die Sperre `typeset::RUNS` wie die Vorschau,
  `spawn_blocking`; Satzfehler, „läuft schon“, Zeit → `REFUSED`, nie halbe Bytes), `book_pdf`, `write_pdf_file`
  (CLI, `create_new`). Das Umschlag-PDF (`cover::to_pdf`) geht über `source_to_pdf(work, PdfKind::Cover, …)`.
  Die Satz-Seite hat `#druckpruefung` (fehlende Bilder `typeset::missing_images`, `PrintWarning`s).
- **Anträge (`asset_requests.rs`):** RPC `asset.request_list`/`asset.request_decide`, Formular
  `POST /werke/bilder/antraege/act`, `GET /werke/{id}/bilder/antraege/{nr}/datei`, `panel` (Block
  `id="antraege"`, `fragments/asset_requests.html`). Bestätigen: Ersetzen/Löschen über den Store unter
  `write_lock` und `mirror::record_asset_by(…, AssetBy::Confirmed)`; Umschlag über
  `rpc::call("work.set_cover")` (nimmt die Sperre selbst) und Protokoll-Commit „Bildantrag n bestätigt“
  (`asset-request-confirmed`). Veraltet (`expected_sha256` bzw. `expected_cover` passt nicht) → `stale` mit Grund.
- **Spiegel (`mirror.rs`):** `AssetBy {Human, Direct(origin), Confirmed(request)}`, `record_asset_by` (Direct:
  Autor Persona bzw. `@mcp:<client>`, Trailer `Baruch-Origin`, `Persona`; Confirmed: Autor der Bestätiger,
  Trailer `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`).
- **MCP (`mcp/assets.rs`):** `asset_add` (sofort), `asset_replace`, `asset_set_cover`, `asset_delete` (nur
  Anträge), `asset_requests`. `/mcp` hat seit Runde 23 die eigene Grenze `MCP_BODY_LIMIT` (12 MiB; ein Bild mit 8 MiB passt als Base64), nicht mehr `works::RPC_BODY_LIMIT` wie `/rpc`.
- **Bilder-Seite (`assets.rs`):** Suche `?q=` (`Store::asset_search`: Name, Bildunterschrift, Alternativtext der
  aktuellen Fassung, in Rust mit `to_lowercase` + `contains`, `%`/`_` wörtlich), Vorschaubilder
  `…/vorschau?b=` über die Crate `image` 0.25 (MIT OR Apache-2.0, nur png/jpeg/gif/webp; `spawn_blocking`,
  Grenzen 20 000 px je Seite und 512 MiB; ETag; SVG unverändert mit CSP `sandbox`), Spalte Umschlag, Blöcke
  `cover::assets_panel` (oben) und `asset_requests::panel` (unten), `POST /werke/bilder/einfuegen` (neue Fassung
  des Menschen mit `#figure(image(…))`, fehlender Pfad `bilder/<name>`, ein Commit).
- **Editor:** `view.rs` liefert `chapter_image`, `missing_images`, `image_choices`; `wysiwyg::image_ref_in`
  setzt `ImageRef.missing` („Bild fehlt“ im Satzbild). Die `ImageView` (Bilder kurz + berechnete Bilder der
  Vorlage) reist unter dem reservierten Schlüssel `wysiwyg::IMAGE_VIEW_KEY` (`"\u{1}baruch-image-view"`) in
  der Tabelle `LibraryView.image_alts` mit. `LibraryView.cover` (aus `cover::cover_info`) zeigt das kleine
  Umschlagbild im Kopf der Gliederung (`fragments/tree.html`).
- **Personas:** `Store::save_persona(handle, name, edit, color, prompt)` schreibt alles eines Tabs in einer
  Transaktion (der Name ist ein eigenes Argument, kein Feld von `PersonaEdit`); `POST /einstellungen/personas/act`
  kennt `action=save`, `action=reset_color` und `action=reset_prompt` (`confirm=1`); abgelehnte Eingaben über `personas::retry`
  (`task_local`). Widgets `web/src/persona_tabs.ts` (ARIA `tablist`/`tab`/`tabpanel`, `history.replaceState`)
  und `web/src/mention.ts` (Vervollständigung „@…“, `combobox`/`listbox`).
- **Tests:** `baruch-render/tests/{pdf,print}.rs`, `baruch-typst/tests/{cover,images}.rs`,
  `baruch-store/tests/{book,asset_requests,migration_0017,migration_0018}.rs`, `baruch-server/tests/round11_{pdf,
  mcp_assets,cover,images,personas}.rs` und Ende-zu-Ende `round11.rs`.

## Einfälle (Migration 0011)

- **Tabellen** (`crates/baruch-store/migrations/0011_ideas.sql`, nur neue Tabellen):
  - `idea`: `work_id` (`REFERENCES work(id) ON DELETE SET NULL`, NULL = ohne Werk), `node_id` (Bezug =
    `Node::key`, `ON DELETE SET NULL`), `note`, `quote`, `source`, `bible`, `origin_kind`
    (`human`/`persona`/`mcp`) und `origin_name` (Name bzw. `@handle` bzw. `@mcp:<client>`), `status`
    (`open`/`done`), `done_as` (`outline`/`post`/`persona`/`note`), `done_ref`, `created`, `updated`, `deleted`
    (NULL = vorhanden, sonst Zeitpunkt). `CHECK ((status = 'done') = (done_as IS NOT NULL))`.
    `AUTOINCREMENT`: ids werden nie wiederverwendet.
  - `idea_word`: Schlagwörter je Einfall mit `position`, `word` und `word_key` (klein, Leerraum zu einem
    Leerzeichen; `UNIQUE (idea_id, word_key)` fasst Doppel zusammen), `ON DELETE CASCADE`.
  - `idea_fts` (FTS5, `unicode61 remove_diacritics 2`, Spalten `words, note, quote, source, bible`, rowid =
    `idea.id`): gepflegt in Rust in derselben Transaktion (`ideas::reindex`), gelöschte stehen nicht darin.
- **Werk löschen** (`Store::delete_work`, unverändert): Die Fremdschlüssel setzen `work_id` und `node_id` auf
  NULL – die Einfälle bleiben als Einfälle ohne Werk. Ebenso verliert ein Einfall beim Löschen oder
  Zusammenführen seines Abschnitts nur den Bezug. Den Spiegel `<data>/einfaelle/` führt erst die nächste
  Aktion an einem Einfall ohne Werk nach (Backlog „Nach Runde 7 offen“).
- **Store** (`crates/baruch-store/src/ideas.rs`): `idea_create`, `idea_get` (auch gelöschte), `idea_list`
  (`IdeaFilter`: `scope` All/NoWork/Work, `word`, `status`, `deleted` = nur gelöschte, `limit` 100/höchstens
  500, `offset`; neueste zuerst), `idea_words` (Wolke, ohne Wortfilter, häufigste zuerst), `idea_search`
  (fertiger FTS5-Ausdruck, Ausschnitt maskiert mit `<mark>`; kaputter Ausdruck → `StoreError::Idea`),
  `idea_update` (`IdeaEdit`, `None` = unverändert; neues Werk ohne `key` → Bezug entfällt; `status: Open`
  setzt die Verarbeitung zurück, `Done` bei einem offenen markiert „von Hand erledigt“ = `done_as: note`),
  `idea_delete`/`idea_restore` (nur `deleted`), `idea_mark_done`, `idea_mirror_file`. Jeder Schreibvorgang ist
  eine Transaktion `BEGIN IMMEDIATE`. Fehler: `StoreError::UnknownIdea(id)`, `StoreError::Idea(<Meldung>)`
  (Grenzen, Bezug außerhalb des Werks, gelöscht), `UnknownWork`. Der Bezug (`key`) muss zum Werk gehören.
- **Spiegel**: `snapshot::files` hängt `ideas::mirror_files` an (nach den Bildern): `einfaelle.md` nur, wenn
  das Werk mindestens einen nicht gelöschten Einfall hat – Werke ohne Einfälle spiegeln und exportieren
  byte-gleich wie vorher. `Idea::entry_markdown` schreibt einen Eintrag:

  ```
  # Einfälle – <Werktitel>            bzw.  # Einfälle ohne Werk

  ## Einfall <id> · offen             bzw.  · verarbeitet (<done_as>)

  - Schlagwörter: Laube, Rosen
  - Gedanke: erste Zeile
    weitere Zeilen eingerückt
  - Markiert: …
  - Bezug: Abschnitt „<Titel>“ (key <k>)
  - Quelle: …
  - Bibelstelle: …
  - Herkunft: Mensch|Persona|MCP <name> · <created>
  ```

  Neueste zuerst, leere Felder fehlen, Werte stehen wörtlich (keine Umwandlung, keine Maskierung).
- **Server**: `crates/baruch-server/src/ideas.rs` (RPC `idea.*`, Seiten `/einfaelle`, `/einfaelle/{id}`,
  `POST /einfaelle/act`, eigene Template-Umgebung mit `ideas.html`, `idea.html`), `src/mcp/ideas.rs`
  (MCP-Werkzeuge), `mirror::describe_idea`/`mirror::record_idea`, Vorlage `fragments/idea_capture.html`
  (Dialog in `workspace.html`, in `view::environment`), Stil `static/ideas.css`, Widget `web/src/ideas.ts`.
  Die RPC-Methoden werden in `rpc::call` direkt nach `work.*` beantwortet (vor `work_param`), weil sie auch
  ohne Werk gehen.
- **Commits** (`mirror::record_idea`, unter `state.write_lock`): genau einer je Aktion und betroffenem Spiegel
  (Umziehen zwischen Werken bzw. von/zu „ohne Werk“: je einer im alten und im neuen). Betreff
  „Einfall <id>: angelegt|geändert|gelöscht|wiederhergestellt|verarbeitet als <Gliederungspunkt|Chat-Beitrag|
  Notiz>“, Trailer `Baruch-Idea: <id>`, `Baruch-Action: idea-create|idea-update|idea-delete|idea-restore|
  idea-process`, `Baruch-Origin: human|persona|mcp`, bei Persona/MCP zusätzlich `Persona: <handle>`. Autor ist,
  wer handelt (Persona „<Name> (<handle>)“, MCP-Client „MCP-Client (@mcp:<name>)“, jeweils
  `<handle>@persona.baruch.invalid`), Committer immer der Mensch; `allow_empty`. Rumpf: vorher/nachher der
  geänderten Felder, beim Löschen der ganze Eintrag. Ohne Werk: eigenes Git-Repo `<data>/einfaelle/`
  (`Mirror::open_or_init`, `<data>/einfaelle` darf keine symbolische Verknüpfung sein) mit nur
  `einfaelle.md` (fehlt, wenn es keine Einfälle ohne Werk gibt). Verarbeiten zum Gliederungspunkt ergibt
  zusätzlich den Gliederungs-Commit (`mirror::record_outline`, `Baruch-Action: outline-add`). Seit Runde 14
  entstehen Punkt und „verarbeitet“ in **einer** Transaktion (`Store::idea_to_outline`: `outline::add`, dann
  `ideas::mark_done_in` mit `done_ref` „key <key>“; Einfall aus einem anderen Werk → `Idea`); scheitert ein
  Schritt, ist nichts geändert, und der Server schreibt keinen Commit (Test mit einem SQLite-Trigger auf
  `UPDATE idea`).
- **Schutz**: `POST /einfaelle/act` liegt in `writes()` hinter `guard::same_origin` (fremde Seiten → 403);
  der Rücksprung `back` nimmt nur eigene Adressen (`/…`, nicht `//…`).
- Auftrag an eine Persona (Runde 10, `ideas.rs` `process_persona`): Modellaufruf über `crate::assist` vor der
  Schreibsperre, danach unter der Sperre `idea_mark_done(…, Persona, "key K, Beitrag ID")` und Spiegel-Commit
  `idea-process`. Der Lektor kennt die Einfälle als Gedächtnis über `assist::context`.


## Menü und Einstellungen (Runde 9)

### Kopfzeile und Menüs (`menu.rs`)

Seit Runde 33 ist die Kopfzeile das **Menüband** (Abschnitt „Runde 33 – Menüband“ weiter unten); die Menüs Werk ▾,
Ansicht ▾ und Werkzeuge ▾ der Runde 9 gibt es nicht mehr. Geblieben ist aus Runde 9:

- Der Kopf wohnt in `templates/base.html` als `{% block topbar %}`; jede Template-Umgebung bettet
  `base.html` schon ein, kein Modul braucht eine neue Registrierung. Seiten liefern `{% block page_title %}` (Text in
  `.work-title`), `{% block page_actions %}` (Gruppen der Seite im Reiter Start) und seit Runde 33 `band_tab`,
  `band_einfuegen`, `band_pruefen`, `band_buch`, `band_ansicht`.
- Daten des Kopfs in dieser Rangfolge: (1) `topbar` (nur die Arbeitsseite, `crate::menu::workspace_topbar`
  in `view::values`: Werk, `key`, `rev`, Ansicht, `lq_q`, Werkwahl, Abzeichen), (2) `work.id`, wenn `work`
  eine Abbildung ist, (3) `settings.work` (Cookie `baruch_work`). Adressen baut die Vorlage selbst (nur
  Ziffern aus den Daten; minijinja maskiert `/` in Werten). Erzeugte Links nie mit `node=`, immer `key=`.
- Auf der Arbeitsseite tragen „Neues Werk / Importieren…“, der Umschalter Editor/Vorschau, „Literatur“ und „Einfall
  festhalten“ `data-work-import-open`, `data-view-toggle`, `data-literature-open` bzw. `data-idea-open`
  (je Seite höchstens einmal; `works.ts`, `literature.ts`, `ideas.ts` unverändert). Auf
  anderen Seiten fehlt der jeweilige Dialog, die Links führen dort einfach auf ihre Seite. „Einstellungen“
  (`data-settings-open`, Alt+,) steht im Menü Baruch ▾ und ist der erste solche Link der Seite.
- `base.html` endet nach `{% block body %}` mit der Hülle des Einstellungs-Dialogs
  `<dialog class="settings-dialog" id="settings" data-widget="settings">` (Inhalt lädt `settings.ts`);
  sie steht hinter allen Dialogen der Seiten.
- `src/menu.rs`: `handles`, `call` (`menu.badges`), `Badges`, `badges(state, work)`, `open_proposals(work)`,
  `workspace_topbar(work, shelf, at, library)`, `fill_badges`/`fill_page` (Runde 23a).
- `web/src/menu.ts` (Widget `menubar`): Tastatur am Menü Baruch ▾, Klick daneben, `aria-expanded` am
  `<summary>`, Abzeichen im ganzen Band über `menu.badges` (beim Laden außerhalb der Arbeitsseite, beim Öffnen des
  Menüs, nach Formularen und `baruch:navigate`). Das Werk nimmt es aus `data-work` am Kopf (`header.band`).
- Thema-Widget bleibt in `web/src/panes.ts` (keine `theme.ts`): zeigt nur ein Zeichen, Name im `title`, und
  liest das Cookie `baruch_theme` neu bei `baruch:settings-changed`; der Knopf steht im Reiter Ansicht.
- Stil: Menü in `static/menu.css` (Liste `position: absolute`, Tastenkürzel über `data-shortcut` als `::after`, damit
  der Text des Eintrags gleich bleibt), Band in `static/band.css`.
- Tests: `tests/round9_menu.rs` (alle Seiten über den Router: genau ein Band, ein Menü, frühere Adressen,
  Dialog-Hülle, Abzeichen mit Antrag über MCP `outline_rename` und Vorschlägen über `propose_revision`, Skripte) und
  `tests/round33_menueband.rs`.

### Einstellungen (`settings/`)

- `src/settings/mod.rs`: Registrierung `registry()` (sieben `SettingsPage {slug, title, storage, render}` in
  fester Reihenfolge), Vertragstyp `PageRender = for<'a> fn(&'a AppState, Option<WorkId>, &'a BTreeMap<String,
  String>) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>`; jede Seite liefert nur
  `<section class="settings-page" data-settings-page-body="<slug>">…`. `frame()` löst das Werk auf (Abfrage
  `work` > Cookie `baruch_work`, wenn es das Werk gibt > Vorgabewerk > keins; ein genanntes unbekanntes Werk →
  404 bzw. -32602), zeichnet `templates/settings/frame.html` (Navigation mit `data-settings-page`,
  `aria-current`, „Gespeichert: …“) und reicht alle übrigen Abfragepaare außer `fragment` an die Seite.
  `?fragment=1` liefert nur den Rahmen (`Cache-Control: no-store`), sonst `templates/settings.html`
  (extends `base.html`, Blöcke `page_title`/`page_actions`).
- Anfrage-Angaben (Cookies, `Host`, Meldung eines abgelehnten Formulars) kennt die Vertragssignatur nicht; sie
  liegen während des Zeichnens in einem `tokio::task_local` (`settings::request()`, ohne Rahmen Vorgaben).
- Seiten: `display.rs` (Darstellung, Formular `POST /einstellungen/darstellung/act`, Cookies im Format von
  `widgets.ts` `store`), `work.rs` (Werk: rendert `{% block content %}` aus `work_edit.html` und
  `work_numbering.html` mit `render_captured` + `State::render_block`, Kontext aus
  `works::edit_context` bzw. `numbering::page_context` – dieselben Funktionen bauen `/werke/{id}/bearbeiten`
  und `/werke/{id}/nummerierung`), `assistants.rs` (Assistenten, Formulare an
  `POST /einstellungen/personas/act` in `personas.rs`), `access.rs` (MCP-Zugang, `mcp::session::clients()`),
  `data.rs` (Daten, `backup()` unter `write_lock`: `create_dir` als Sperre gegen Überschreiben, `VACUUM INTO`,
  rekursive Kopie ohne `baruch.sqlite`/`-wal`/`-shm`, Verknüpfungen übersprungen und gemeldet; Zeit aus SQLite
  `strftime(…, 'localtime')`; vorher Obergrenze `MAX_BACKUPS` = 10 Sicherungen `<Name>.sicherung-*` und freier
  Platz über `rustix::fs::statvfs` (Unix): Datenordnergröße + `RESERVE_BYTES` = 512 MiB, sonst Ablehnung ohne
  anzulegen). Abgelehnte Werk-Formulare mit `back` (`POST /werke/act`, `/werke/nummerierung/act`) zeichnen über
  `settings::work_retry_page` die Seite „Werk“ mit `RequestInfo::work_retry` (Eingabe + Meldung), damit das
  Popup einen Rahmen übernehmen kann. Verbindungen und Quellen: `crate::connections::render_connections` bzw. `render_sources` (unten).
- Eigene Template-Umgebung in `settings/mod.rs` (Autoescape `.html`) mit `base.html`, `settings.html`,
  `settings/*.html`, `work_edit.html`, `work_numbering.html`. `templates/personas.html` entfällt.
- `back` (Runde 9): `POST /werke/act` (update) und `POST /werke/nummerierung/act` nehmen ein verstecktes Feld
  `back`; nur `/einstellungen/werk` mit einer Abfrage aus `[A-Za-z0-9=&_%.-]` gilt (`works::settings_back`),
  dann 303 dorthin mit `&ok=werk` bzw. `&ok=schema`; sonst wie bisher. Fehlerseiten behalten `back`.
- `view::Settings.pv` (`open` | `closed`, Cookie `baruch_pv`): bei `closed` hängt `from_cookie_header` den
  Merker `view::FOLD_ALL` = `(0, 0)` an `folded` (Schlüssel 0 gibt es nicht); `proposals_of` klappt damit alle
  Vorschläge ein außer dem gezeigten – derselbe Weg wie `baruch_folded`.
- Store (`baruch-store/src/personas.rs`): `PersonaRow` (alle Felder + Farbe), `PersonaEdit` (mit `check()`:
  Rolle/Modell getrimmt 1–200 Zeichen, Kontext 1–10 000 000), `Store::persona_list`, `Store::update_persona`
  (`BEGIN IMMEDIATE`, unbekannt → `UnknownPersona`, Grenzen → `Refused` als zweite Wache; die Server-Schicht
  prüft vorher und meldet -32602). Keine Migration.
- `mcp/session.rs`: `SeenClient {name, version, handle, first_seen, last_seen}` (Unix-Sekunden), `clients()`
  (je Handle ein Eintrag, zuletzt gesehen zuerst, höchstens `MAX_SESSIONS`); `start()` setzt beide Zeiten
  (bei bekanntem Handle bleibt `first_seen`), `client(id)` setzt `last_seen`.
- Widget `web/src/settings.ts` (`data-widget="settings"` am `<dialog id="settings">` aus `base.html`):
  `[data-settings-open]` und Alt+, öffnen (`showModal`) und laden den Rahmen (`fragment=1`) in
  `[data-settings-content]`; `a[data-settings-page]` wechselt die Seite; POST-Formulare per `fetch`
  (urlencoded, Weiterleitung folgen), aus der Antwort wird `[data-settings-frame]` übernommen (auch bei 422);
  GET-Formulare auf `/einstellungen/…` laden im Dialog. Nach Erfolg `baruch:settings-changed` mit
  `detail.page`; das Thema setzt es sofort (`dataset.theme`). Stil `static/settings.css`.
- Tests: `baruch-server/tests/round9_settings.rs` (15), `baruch-server/tests/personas.rs` (angepasst:
  Seite Assistenten, Weiterleitung), `baruch-store/tests/round9_personas.rs` (4), Unit-Tests in
  `settings/{mod,display,access,data}.rs`, `mcp/session.rs`, `view.rs`.

### Verbindungen und Quellen (`connections/`, Migration 0013)

- Migration `crates/baruch-store/migrations/0013_connections.sql`: Tabellen `connection` (kind, config als
  JSON-Objekt, nie Geheimnisse; letzte Prüfung `checked`/`check_ok`/`check_message`/`check_details`),
  `work_rag` (je Werk die Wege als JSON-Liste, `ON DELETE CASCADE`), `preferred_source` (Lesezeichen,
  `position` 1..n lückenlos) und Index `preferred_source_order`. Fehlende Zeile = Vorgabe.
- Store `crates/baruch-store/src/connections.rs`: `ConnectionKind` (`as_str`/`FromStr`, `default_config`),
  `ConnectionRow`, `ConnectionCheck`, `RagWay`, `PreferredSource`, `NewSource`, `SourceEdit`;
  `Store::connection_get/connection_list/connection_set` (geänderte Einstellungen leeren die alte Prüfung),
  `connection_record_check`, `work_rag`/`set_work_rag`, `sources_list/source_create/source_update/
  source_move/source_delete` (Reihenfolge in derselben Transaktion lückenlos). Ablehnungen über
  `StoreError::Refused`, keine neue Variante; `connection_bind_secret` (0014). Tests `tests/{connections,migration_0013,migration_0014}.rs`.
- Server `crates/baruch-server/src/connections/`:
  - `secrets.rs`: Trait `SecretStore` (`get`/`set`/`delete` je Konto, Dienst fest `baruch`),
    `KeyringSecrets` (Crate `keyring` 3, Linux Secret Service über D-Bus; findet auch Einträge, die Python
    `keyring` ohne `target` angelegt hat), `MemorySecrets` (Tests, Wegwerf-Server), `system_secrets()`,
    `secrets_mode` (`serve`: `--secrets keyring|memory` bzw. `BARUCH_SECRETS`, Vorgabe Schlüsselbund nur mit dem
    Vorgabe-Datenordner, sonst Speicher – die Einträge hängen nur an Dienst und Konto, nicht am Datenordner);
    der Server ruft die blockierenden Methoden nur über `secrets::get/set/delete` in `spawn_blocking` auf.
  - Bindung des Geheimnisses an das Ziel (Migration 0014, `connection.secret_target`, `Store::connection_bind_secret`):
    `config::target` (Ursprung der Adresse bzw. Rechner/Port/TLS); `set_config` merkt sich bei Zieländerung und
    hinterlegtem (oder unlesbarem) Geheimnis das alte Ziel, `set_secret`/`delete_secret` setzen NULL (= jetziges
    Ziel), alles unter `write_lock`; `run_check` sendet nur bei `secret_bound`, sonst `ok: false` mit
    `details.secret_withheld` ohne die Gegenstelle zu fragen. Die Suchwege (Runde 12: `pgvector::open_way`,
    `multigpt::open_way`, `rag::embed::EliezerEmbedders`) nehmen dieselbe Prüfung.
  - `config.rs`: erlaubte Felder je Art, Prüfung der Adressen und Felder, Ablehnung von Geheimnis-Feldern.
  - `check.rs`: Prüfen (reqwest 0.12 mit rustls, sqlx-Postgres über die Laufzeit-API); `scrub` ersetzt ein
    zurückgespiegeltes Geheimnis durch `***`, bevor etwas gespeichert oder gezeigt wird.
  - `pages.rs`: `render_connections`/`render_sources` (Vertragssignatur `PageRender`, eigene minijinja-Umgebung
    mit `templates/settings/{verbindungen,quellen}.html`) und die beiden Formular-Handler.
  - `sources.rs` (RPC `sources.*`), `mcp.rs` (`sources_list`), `mod.rs` (RPC `connection.*`, `writes()`).
- `AppState::secrets` (`Arc<dyn SecretStore>`); `AppState::open` nimmt `MemorySecrets`, `serve` nimmt
  `AppState::open_with_secrets(&data, …)` mit dem Schlüsselbund bzw. `MemorySecrets` nach `secrets_mode`. Kein Test erzeugt `KeyringSecrets`.
- Neue Crates nur in `crates/baruch-server/Cargo.toml`: `keyring` 3 (`sync-secret-service`, `crypto-rust`),
  `reqwest` 0.12 (ohne Vorgaben; `json`, `rustls-tls`, `rustls-tls-native-roots`), sqlx zusätzlich
  `postgres` und `tls-rustls-ring-native-roots` (Systemzertifikate, auch eine Haus-CA im Intranet), tokio
  zusätzlich `time`. Lizenzen der rund 100 neuen Crates: MIT/Apache-2.0/ISC/Zlib, `ring` Apache-2.0 AND ISC,
  `webpki-roots` CDLA-Permissive-2.0 – mit AGPL-3.0 verträglich.
- Tests `crates/baruch-server/tests/round9_connections.rs`: Seiten direkt über die Render-Funktionen, Formulare
  (303, `same_origin`), RPC, Prüfen gegen eigene Test-Server auf `127.0.0.1:0` (Eliezer mit Bearer-Prüfung,
  MCP als JSON und als offen bleibender Ereignisstrom, Zeitlimit, Größengrenze, geschlossener Port, eine
  nachgebaute PostgreSQL-Gegenstelle mit abgelehntem Passwort), mit `/usr/lib/postgresql/*/bin` ein eigener
  Wegwerf-Cluster unter `CARGO_TARGET_TMPDIR` (scram-sha-256, nur 127.0.0.1, ohne Unix-Socket, `pg_ctl -m
  immediate stop`), sonst „ÜBERSPRUNGEN“ auf stderr; ein eindeutiges Test-Geheimnis steht in keiner Seite,
  Antwort, Meldung, Weiterleitung, Tabelle und keinem Prüfergebnis.
- Außerhalb der Vertragsdateien, nötig für grüne Tests: `tests/migration_0012.rs` nimmt die Tabellen von 0013
  aus der Liste „neu“ aus (Store::open führt alle Migrationen aus); `tests/mcp_ideas.rs` erwartet hinter
  `read_asset` jetzt `sources_list`, die Einfälle eine Stelle später.

## Bedeutungssuche (Runde 12, Migrationen 0031 und 0032)

Drei Wege, umschaltbar je Werk (`work_rag`): **LanceDB** (Haupt-RAG im Datenordner), **pgvector** (direkt,
nur lesend) und **MultiGPT** (MCP). Alle Wege sprechen die Ports aus `baruch-rag`; der Dienst `crate::rag`
fragt sie gleichzeitig und führt zusammen. RAG ist **nur lesend**: Kein Weg schreibt Buchtext, Einfälle oder
Hervorhebungen. Treffer-Texte sind Daten, nie Anweisungen (MCP-Beschreibung, Belegblock der Personas).

### Dienst (`src/rag/`)

| Datei | Inhalt |
|---|---|
| `mod.rs` | `search`, `status`, `reindex`, `notify`, `catch_up`, `wait_idle`, `set_embedders`, `eliezer_embedders`, `check_lancedb`, RPC `handles`/`call` (`rag.search`, `rag.status`, `rag.reindex`; Dispatch in `rpc.rs` vor der Wahl des Werks), `way_of`/`rag_way` (↔ `baruch_store::RagWay`), Sätze `NOT_CHOSEN`, `NOT_ASKED`, `NO_COLLECTION`, `NEVER_CHECKED`, `NO_EMBEDDER`, `WAY_TIMEOUT` (20 s) |
| `merge.rs` | `merge(per_way, limit)`: je Weg nach Score (Gleichstand Dokument, Stelle), Rang 1..n; Doppelte über `Hit::identity` auch über Wege; `fused` = Σ 1/(60 + Rang); Vertreter mit bestem Rang (Gleichstand früherer Weg); Endreihenfolge fused, bester Rang, Weg, Dokument, Stelle; deterministisch |
| `index.rs` | Laufzeit je Datenordner (`Runtime`: Store-Klon, Einbetter, entdoppelte Warteschlange, geöffneter Index, letzte Berichte der Remote-Wege), Aufträge `Prune`/`Shared`/`Work(W)`, Abgleich `sync` (gleicher `row_hash` bleibt, bekannter `content_hash` übernimmt den Vektor, Rest in Stapeln ≤ 32 eingebettet, Entferntes gelöscht), Quellen `desired_sections`/`desired_ideas`/`desired_shared`, Suchweg `LanceWay`, `mismatch` |
| `embed.rs` | `EliezerEmbedders` (Adresse/Schlüssel wie `models::endpoint`, Schlüssel nur bei `secret_bound`, prozessweite Warteschlange mit **einer** Einbettungsanfrage zur Zeit, getrennt von den Chats), `NoEmbedders`, `Cached` (je Suche: gleicher Text je Modell einmal – LanceDB und pgvector teilen die Einbettung der Anfrage) |
| `check.rs` | `check_lancedb` → `{kind: "lancedb", ok, message, details, at}` über `connection_record_check`; `run_check` in `connections/mod.rs` gibt LanceDB dorthin ab |
| `hooks.rs` | Sammlungen des Werks (`Store::work_collections`), `open_pgvector` → `connections::pgvector::open_way`, `open_multigpt` → `connections::multigpt::open_way` |

- **Laufzeit ohne Änderung an `AppState`:** prozessweite Tabelle Datenordner → `Runtime`. `notify` legt keine an
  und blockiert nie. Ohne `catch_up` bzw. `set_embedders` gibt es keinen Einbetter (Tests, Hilfsbefehle – nie
  Netz); `serve` ruft `rag::catch_up` nach dem Öffnen (Eliezer als Einbetter, `Prune`, `Shared` und alle Werke
  einreihen).
- **Wer meldet Änderungen:** `mirror.rs` (`record`, `record_switch`, `record_outline_by`, `record_idea`,
  `record_post`, `sync_work`), `works.rs` nach `work.delete` (`notify(Some(work))`), `literature.rs` nach jeder
  Änderung an einer Hervorhebung über `search_hooks::changed` (`notify(None)` für den gemeinsamen Bereich und
  `notify(Some(work))`). Bibliothekstitel frischt die Suche höchstens alle 30 s auf.
- **Fehler:** wiederholbar bis 3 Versuche (Pausen 0,3 s / 1,2 s), dann liegengeblieben (`Runtime::stale`, im
  Status `index.stale`); Modell bzw. Dokument-Präfix passt nicht → kein Versuch, kein Mischen, Problem im Status,
  bis `rag.reindex`. Seit Runde 19a holt nur eine echte Änderung (`notify` → `Runtime::enqueue`), ein Auftrag,
  der tatsächlich wieder eingebettet hat (`Outcome.embedded`), oder der Neuaufbau Liegengebliebenes zurück. Die
  Suche reiht über `refresh_shared` → `enqueue_quiet` nur `Prune` (billig: erst zählen) und höchstens alle 30 s
  `Shared` ein und überspringt dabei liegengebliebene Aufträge; ein gelungener Auftrag ohne Einbettung löscht das
  Problem nur, wenn nichts mehr liegt (sonst zeigte der Status zwischendurch „kein Problem“). `Job::Work` eines
  unbekannten (gelöschten) Werks löscht dessen Zeilen und reiht `Shared` gleich danach ein (`Outcome.follow`):
  Die Einfälle des Werks sind jetzt „ohne Werk“.
- **Neuaufbau ohne Warten (Runde 19a):** `reindex_report` nimmt die Schreibsperre nur mit `try_lock`; hält der
  Worker sie, vermerkt `Runtime::wish_reset` den Wunsch (ganzer Index bzw. Werke) und setzt `abort`. Der Worker
  prüft `abort` zwischen zwei Stapeln, während einer Einbettung (`embed_or_abort`, alle 50 ms) und in den Pausen
  zwischen Versuchen, verwirft das Ergebnis des abgebrochenen Auftrags, führt den Wunsch unter der Sperre aus
  (`reset_index`) und reiht neu ein. Der unterbrochene Auftrag selbst kommt vorn wieder in die Warteschlange
  (doppelt nur einmal): `reset_index` liefert für einzelne Werke nur deren Aufträge und `Shared`, der laufende
  Auftrag eines anderen Werks oder `Prune` bliebe sonst halb abgeglichen und ohne Vermerk liegen. RPC-Antwort mit `deferred`; `reindex` (Haken `search_hooks::rag_reindex`)
  liefert weiter nur `pending`.
- **Bereichs-replace (Runde 19a):** `Index::replace(scope, remove, rows, dimension)` löscht nur
  `id IN (…) AND <Bereich>` und lehnt Zeilen außerhalb des Bereichs ab (`RagError::Invalid`); `sync` übergibt den
  Bereich des Auftrags. So kann eine neu vergebene Kennung (`node.key`) in einem anderen Werk keine fremden
  Zeilen mehr löschen. Wechselt ein Einfall das Werk, räumt der Abgleich jedes Bereichs seinen Teil.
- **Aufgaben-Präfixe (Runde 19b, LanceDB mit 19a):** `LanceConfig.prefixes` aus
  `connections::config::embed_prefixes(Lancedb, …)`; `sync` bettet `document_text(text)` ein (gespeichert und
  gehasht bleibt der Text selbst), `LanceWay` die Anfrage als `query_text(query)`. `IndexMeta.document_prefix`
  (beim ersten Stapel gesetzt; fehlt = leer); `mismatch` meldet ein anderes Dokument-Präfix als
  `ModelMismatch` („Aufgaben-Präfix geändert; Index neu aufbauen.“), `reset_index` baut dann den ganzen Index neu.
- **Start ohne Anfrage an die Vorgabe-Adresse (Runde 19a):** `catch_up` setzt keinen Einbetter, wenn der
  Datenordner nicht der Vorgabe-Ordner ist (`baruch_store::default_data_dir`, kanonisch verglichen) und die
  Verbindung `eliezer` nie gespeichert wurde (`ConnectionRow.updated` leer); `Runtime::unbound` hält den Satz
  `NO_EMBEDDER_OWN_DATA` für den Status. `reindex_report` bindet Eliezer an, sobald die Verbindung gespeichert ist.
  Ein vorher gesetzter Einbetter (`set_embedders`, Tests) bleibt wie bisher.
- **`weak` (Runde 19a):** `rag::answer_json` setzt je Treffer `weak = baruch_rag::is_weak(hit.score)` (RPC
  `rag.search`, MCP `rag_search`); `Answer` selbst (eingefroren in `baruch-rag`) bleibt unverändert.
- **Suche:** Eingabe prüfen (Anfrage 1..=1000 Zeichen, `limit` 1..=50), Wege des Werks ∩ `ways` der Anfrage, je
  Weg ein Plan (aus: nicht gewählt / nicht gefragt / keine Sammlung / `NotConfigured` beim Öffnen), alle
  gleichzeitig mit 20 s Schutz je Weg, `merge`. `Answer.ways` nennt immer alle drei Wege. Remote-Wege nur mit
  einem von `MAX_REMOTE_SEARCHES` (4) Plätzen (`remote_slot`, Semaphore); ohne Platz nach 20 s ausgefallen mit
  `BUSY`, LanceDB läuft weiter, der letzte gemeldete Zustand des Wegs bleibt stehen.
- **Gemeinsamer Bereich** (`desired_shared`): abgelehnte Hervorhebungen (`HighlightStatus::Rejected`) kommen
  nicht in den Index – wie bei der Wortlaut-Suche.
- **Status ohne Netz:** LanceDB aus dem Index; Remote-Wege aus Einstellung, gewählten Sammlungen und der letzten
  Suche dieses Prozesses (falls neuer als die letzte Prüfung) bzw. der letzten Prüfung; nie geprüft → „aus“ mit
  „noch nicht geprüft (Verbindung prüfen)“.
- **Index-Ordner:** Verbindung LanceDB `dir` (Vorgabe `lancedb`, relativ zum Datenordner, nie der Datenordner
  selbst, nie unter `works/`); eine Tabelle `chunks` (`id`, `work`, `kind`, `document`, `section`, `title`,
  `text`, `meta`, `content_hash`, `row_hash`, `vector`), ein Modell je Index.
- **Haken der Oberfläche** (`search_hooks.rs`): `rag_search`, `rag_status`, `rag_reindex`, `changed` – Literatur,
  Einfälle und die Einstellungsseite rufen nur diese; Personas und MCP rufen `assist::hooks::rag_search`/`rag_status`.
  Beide leiten an `crate::rag` weiter.

### pgvector (`connections/pgvector.rs`)

- `check(config, secret)` für „Verbindung prüfen“ und `open_way(state, embedders)` als `SearchWay`. Eine
  Verbindung je Suche (sqlx-Laufzeit-API, `PgConnectOptions::new_without_pgpass`, TLS wie eingestellt), alles in
  `BEGIN TRANSACTION READ ONLY` mit `statement_timeout` 10 s (`begin_read_only`), Verbindung höchstens 10 s.
- Zuordnung (`Mapping::from_config`, `config::effective` mit Vorgaben; `PG_MAPPING`, `is_identifier`,
  `split_table`) → vor jeder Abfrage gegen `information_schema.columns` bestätigt (Schema aus dem Suchpfad bzw.
  `schema.tabelle`, Vektorspalte vom Typ `vector`, Dimension aus `atttypmod`, sonst `vector_dims` der ersten
  Zeile). Bezeichner nur geprüft und in doppelten Anführungszeichen; Vektor, Sammlungen (`= ANY($2)`) und Grenze
  sind gebundene Parameter.
- Suche: Modelle der gewählten Sammlungen (mit `model_column`) → Einbettung über
  `EmbedderFactory::for_model(embedding_model)` → Modell (`same_model`, locker: Alias) und Dimension geprüft →
  sonst `ModelMismatch` ohne Ähnlichkeitsabfrage → `ORDER BY "vec" <=> $1::vector LIMIT $3`, Score 1 − Abstand.
  Fehler: Anmeldung/Rechte → `Denied`, `57014` → `Timeout`, Netz → `Unreachable`, Tabelle/Spalte → `Schema`.
- Runde 19b (Härtung): jedes Feld aus der fremden Tabelle in SQL per `left(…, n)` gekürzt (Konstanten
  `MAX_DOCUMENT_CHARS` 500, `MAX_SECTION_CHARS` 200, `MAX_TITLE_CHARS` 300, `MAX_URL_CHARS` 2000,
  `MAX_COLLECTION_CHARS` 200, `MAX_MODEL_CHARS` 200, Text `SNIPPET_SOURCE_CHARS` 2000); die Sammlungsliste der
  Prüfung zählt je vollem Namen in einer Unterabfrage (höchstens 1000) und kürzt außen. Die Suche wählt eine
  Sammlung über den vollen **oder** den gekürzten Namen (`= ANY($n) OR left(…) = ANY($n)`). `quoted` verdoppelt
  Anführungszeichen (statt eines bloßen `debug_assert!`): das Schema aus `information_schema` ist ungeprüft und kann
  jedes Zeichen enthalten; `to_regclass` bekommt dieselbe gequotete Form. `tls_warning(config, has_secret)` für
  Prüfung (`message`, `details.tls_warning`) und Seite. Die Anfrage wird mit `config::embed_prefixes(Pgvector,
  …).query_text` eingebettet (S1-Funktion); die Felder `query_prefix`/`document_prefix` kennt `config::fields`,
  geprüft mit `baruch_rag::check_prefix` (nicht beschnitten).
- `connections/collections.rs`: RPC `connection.collections_get/set`, `available_of` (gemeinsame Form
  `details.collections = [{name, count}]` von pgvector und MultiGPT).
- Formular der Verbindungen nimmt die Felder als `Form<Vec<(String, String)>>` (wiederholtes `collection`).

### MultiGPT (`connections/multigpt.rs`)

- `McpClient` – MCP über Streamable HTTP (`initialize` mit Protokoll 2025-06-18, `notifications/initialized`,
  danach `Mcp-Session-Id`/`MCP-Protocol-Version`; JSON oder `text/event-stream`, ein Strom wird nach jedem Stück
  geprüft – ein am Ende halbes UTF-8-Zeichen heißt „weiterlesen“, `find_answer`), `list_tools` über `nextCursor`
  (höchstens 10 Seiten), `call_tool` (`isError` → `Failed` mit dem Text der Gegenstelle); Grenzen `Limits`
  (Suche 15 s je Aufruf, 2 MiB), keine Weiterleitungen, `scrub_error` (Token → `***`). `check::multigpt` nutzt
  denselben Client (der frühere `McpSession` ist darin aufgegangen).
- `search_arguments` leitet die Argumente aus dem `inputSchema` von `search_documents` ab (Anfrage `query`/`q`/…,
  Sammlungen als Liste oder einzeln mit einem Aufruf je Sammlung, Trefferzahl `limit`/`top_k`/…, geklemmt);
  fehlt ein Parameter für die Sammlungen, fragt Baruch nicht (`Schema`). `parse_hits` liest viele Formen
  (Score geklemmt, sonst 1 − distance, sonst Ersatz aus dem Rang; URL nur http(s)). Seit Runde 21a ist der Ersatz
  `rank_score(rang) = WEAK_SCORE + (1 − WEAK_SCORE)/rang` (Rang 1 = 1,0, fallend, nie unter `WEAK_SCORE`): Ohne
  Angabe von MultiGPT ist die Ähnlichkeit unbekannt, darum gilt ein Treffer „aus Rang“ nie als „schwach“ (früher 1/Rang:
  ab Rang 3 immer schwach); `meta.score = "aus Rang"` bleibt als Kennzeichen.
- `MultigptWay`: eine Sitzung je Suche (aus dem Vorrat, siehe unten), nur die gewählten Sammlungen;
  `document_info` ergänzt Titel/URLs (≤ 5), `read_document` fehlende Ausschnitte (≤ 3, ≤ 600 Zeichen). Seit
  Runde 13 laufen diese Nachträge je unter `FOLLOW_UP_TIMEOUT` (4 s) und zusammen unter `FOLLOW_UP_BUDGET` (6 s),
  nie über `rag::WAY_TIMEOUT` − 1 s hinaus; nach einem Zeitlimit oder Transportfehler gibt es keine weiteren
  Nachträge, die gefundenen Treffer gehen unverändert zurück.
- **Sitzung (Runde 13, Vertrag §2.2):** `Session` (`open` aus den Einstellungen wie `open_way`, `connect`,
  `tools`, `schema`, `call` mit eigenen `Limits` je Aufruf, `scrub`, `release`, `close`). Prozessweiter Vorrat je
  (Adresse, Hash des Tokens, Tokio-Laufzeit): höchstens zwei ruhende Sitzungen; nach `SESSION_IDLE` (120 s) endet
  eine ruhende Sitzung mit HTTP `DELETE` (`Mcp-Session-Id`, Token). Die Werkzeugliste gilt `TOOLS_TTL` (300 s) je
  Schlüssel – zwei Suchen hintereinander brauchen ein `initialize` und ein `tools/list`. `release` legt die
  Sitzung zurück, nach einem Transport- oder Protokollfehler (auch Zeitlimit, HTTP-Fehler) beendet sie sich
  stattdessen mit `DELETE` (im Hintergrund, damit die Suche nicht wartet); eine fallen gelassene Sitzung (etwa
  wenn `rag::WAY_TIMEOUT` die Suche abbricht) ebenso. Antwortet eine Sitzung aus dem Vorrat nicht mehr (HTTP
  400/404: die Gegenstelle hat sie beendet, oder keine Verbindung zustande gekommen – `reqwest::Error::is_connect`,
  `McpClient::unsent`), verbindet `call` einmal neu. Nach einer Zeitüberschreitung oder einem Abbruch nach dem
  Senden wiederholt `call` nicht: Der Aufruf kann schon ausgeführt sein (ein `upload_document` des Abgleichs läge
  sonst doppelt vor). Die Laufzeit gehört zum
  Schlüssel, weil reqwest seine Verbindungen an die Laufzeit bindet, in der sie entstanden (Tests haben je eine).
  Der Hash des Tokens ist ein prozesszufälliger SipHash (kein sha2 im Server); er dient nur zum Unterscheiden.
  `expire_idle(url, frist)` beendet ruhende Sitzungen einer Adresse vorzeitig (Tests, Wartung).
  `check::multigpt` nutzt keinen Vorrat und hat ein gemeinsames `tokio::time::timeout` (10 s) um alle Anfragen.
- **Volltext (Runde 13):** `MultigptWay::read_full` und `read_part` (`read_part_with_info` dazu `document_info`
  und die Stelle des Teils im Volltext) über `read_document` mit `FULLTEXT_LIMITS` (20 s, 4 MiB; größer → „Antwort
  von … ist größer als 4 MiB; abgebrochen.“), `\r\n` → `\n`. `page_key`: ein Name aus `SECTION_KEYS`, der eine
  Zahl nimmt (`page` immer) – dann ruft Baruch `read_document` mit Seite auf; nennt die Antwort Seitenangaben, gilt
  sie als Seite (`paged_by_remote`, `read_full` verbindet alle Seiten mit einer Leerzeile, höchstens 500 Seiten),
  sonst als ganzes Dokument, das `split_parts` an Absatzgrenzen in Teile von höchstens `PART_CHARS` (20 000)
  Zeichen teilt (die Teile hintereinander ergeben genau den Text, so bleiben Zeichenbereiche gültig). Verlangt
  das Schema einen unbekannten Parameter → `RagError::Schema`.
- **Annahmen (ungeprüft gegen das echte MultiGPT, Runde 13 „quellen“):** `read_document` nimmt das Dokument unter
  einem Namen aus `DOCUMENT_KEYS` und eine Seite (ab 1) unter `page` bzw. einem Zahl-Parameter aus `SECTION_KEYS`;
  die Antwort trägt den Text unter `text`/`content`/`body`/`markdown` (auch unter `document`/`result`/`data`) in
  `structuredContent` oder als JSON in `content[].text`, sonst ist `content[].text` selbst der Text; Seitenangaben
  heißen `pages`/`total_pages`/`page_count`/`num_pages` bzw. `has_more`/`more`/`next_page`/`next_cursor`; eine Seite
  hinter dem Ende ist `isError`. `document_info` nennt `title`/`name` und `url`/`link`/`source_url`. MultiGPT
  beendet eine Sitzung bei `DELETE` und antwortet auf eine unbekannte `Mcp-Session-Id` mit 400 oder 404.
- **Annahmen für den PDF-Import (Runde 21a, nach der Werkzeugbeschreibung Stand 2026-11-09, ungeprüft gegen das echte
  MultiGPT; `pdf_import/remote.rs`):** `upload_document {collection (ID oder Name), content_base64, filename, url?}` →
  `document_id` (Zahl oder Text) und `run_id`; die Indexierung läuft nach. Angenommene Dateitypen PDF, DOCX, TXT, MD,
  Bilder – **kein EPUB** (EPUB liest Baruch darum immer selbst). Fortschritt über `run_status {run_id}` → `status`
  (`done`/`completed`/`finished`/`success`/`ready`/`indexed` = fertig, `failed`/`error`/`cancelled` = gescheitert);
  ohne `run_status` liest Baruch `read_document` mit Wiederholung, bis es antwortet; zusammen höchstens 120 s.
  `read_document {document_id, page_from?, page_to?, paragraph_from?, paragraph_to?, section?}` liefert höchstens
  12 000 Zeichen bzw. 10 Seiten je Aufruf, Text unter `text` (auch `content`/`body`/`markdown`), Seitenzahl unter
  `pages`, Kürzung unter `truncated` (sonst: 12 000 Zeichen = vermutlich gekürzt). `document_info {document_id}` nennt
  `title`, `author`, `pages` und ein erkanntes Inhaltsverzeichnis unter `toc` (auch `table_of_contents`, `outline`,
  `sections`, `chapters`; Einträge mit `title`, `level` bzw. Abschnittsnummer `section` „1.2“, `page`, verschachtelt
  unter `children`) – Quelle der vorgeschlagenen Gliederung; fehlt es, gelten nummerierte Zeilen im Text. Das Hochladen
  läuft in einem Aufruf mit höchstens 30 s (Obergrenze einer Sitzung, `SESSION_CEILING`); sehr große Dateien über ein
  langsames Netz können daran scheitern (ehrliche Meldung, Angebot „lokal lesen“).
- `open_way(state)`: keine Adresse → `NotConfigured`; Token `baruch/multigpt` nur bei `secret_bound`.
- `pub mod mock` – `MockMultigpt` für Tests: `start`, `url`, `set_tools`, `set_collections`, `set_hits`,
  `require_bearer`, `requests`, `tool_calls`, `configure(|b| …)` (`Behavior`: Seiten, Antwortform, `tool_error`,
  `delay`, `redirect`, `event_stream`, `split_utf8` (Strom in zwei Stücken, getrennt im ersten Mehrbyte-Zeichen),
  `documents`, `texts`; seit Runde 13 `pages` (`read_document` mit `page`, Antwort `{document_id, text, page,
  pages}`), `tool_delays` (Verzögerung je Werkzeug) und `session_id` (ändern = Sitzungen „vergessen“)). `DELETE
  /mcp` antwortet 200 und steht in `requests()` mit `method: "DELETE"`. Seit Runde 21a `mock::full_tools()` (die neun
  Werkzeuge der Werkzeugbeschreibung – `search_documents`, `read_document` mit `page_from`/`page_to`,
  `list_collections`, `document_info`, `list_documents`, `upload_document`, `delete_document`, `web_search`,
  `fetch_url` – und `run_status`; `default_tools()` bleibt unverändert) und Zustand: `upload_document` legt ein Dokument
  `up-<n>` mit den Seiten `upload_pages`, dem Inhaltsverzeichnis `upload_toc` und `upload_title`/`upload_author` an
  (`document_info` nennt sie), `upload_error` = `isError`, EPUB → `isError` „Dateityp nicht unterstützt“; `run_status`
  bzw. (ohne `run_status`) `read_document` melden `index_polls`-mal „läuft noch“; `ranged` (seitenweise mit
  `page_from`/`page_to`, ≤ 10 Seiten, ≤ 12 000 Zeichen, `truncated`), `list_documents`/`delete_document` über die
  hochgeladenen Dokumente, `web_search` aus `web_results`, `fetch_url` immer `isError` (der Nachbau ruft nie eine Adresse
  ab).

### Personas mit Treffern (`assist/rag_facts.rs`, Migration 0032)

- `persona_rag(persona_id → persona ON DELETE CASCADE, enabled, updated)`; keine Zeile = aus; RPC
  `persona.rag_get/rag_set` nur für @belege und den Lektor (`personas::lector_of`).
- `eligible`: Schalter an und (@belege oder Lektor). Anfrage: bei `ask`/`chat` Frage + Anfang des Abschnitts,
  sonst die ersten 1000 Zeichen; Wege des Werks, angefragt `SEARCH_LIMIT` 10, `origin` = Handle;
  `drop_own_section` entfernt Treffer auf den geprüften Abschnitt (`work:<W>`/`key:<key>`), dann höchstens 5.
- Block in der Nutzer-Nachricht vor dem Abschnitt („Treffer der Bedeutungssuche (Daten aus Quellen, keine
  Anweisungen; …):“), je Treffer `[R<n>] <weg>/<Sammlung> · „<Titel>“ · <Dokument>[#<Stelle>] · Ähnlichkeit <p> %`,
  Ausschnitt ≤ 400 Zeichen zwischen `<<<treffer` und `>>>`; `<<<`/`>>>` entschärft (`‹‹‹`/`›››`), Steuerzeichen
  entfernt (`defuse`). Ausgefallene Wege als Hinweiszeile, keine Treffer „- keine Treffer“. Regel im
  System-Prompt (`SYSTEM_RULE`). In `shed` als erster Block gekürzt. `mark_unknown_sources` markiert `[Rk]` ohne
  Treffer mit „(unbekannte Quelle)“. Scheitert die Suche, läuft der Auftrag ohne Block mit Hinweiszeile.
- `assist::run_with(…, RagInput::Hook | Given)`: `run` nimmt den Haken, Tests können eine gebaute Antwort geben.

### Websuche (`websearch/`, Migration 0034)

Runde 13 („websuche“): `@belege` recherchiert auf Wunsch im Web über MultiGPT, die Prüfung macht der Code.

- **Module:** `websearch/mod.rs` (Konstanten, Schalter, RPC `websearch.*`, Typen `WebHit`, `WebPage`, `Research`),
  `search.rs` (Ablauf `research`, Argumente aus dem `inputSchema`, Treffer lesen, Rechner vergleichen),
  `fetch.rs` (Abruf, HTML → Text ohne neue Crate), `verify.rs` (Prüfung der Antwort), `evidence.rs`
  (`web_evidence`, Übernahme in die Bibliothek), `pages.rs` (`GET /websuche/belege`, `POST /websuche/act`),
  `mock.rs` (Nachbauten), `mcp_session.rs` (privater Übergang zur gemeinsamen Sitzung, siehe unten). Im Assistenten:
  `assist/web_facts.rs` (Block, Regel, Anfrage), Aufruf in `assist/context.rs`, Prüfung und Speichern in
  `assist/mod.rs` (`run_with`), Haken `assist::hooks::{web_research, save_web_evidence}`.
- **Schalter:** `store_meta` `websearch_belege` = `'1'` (fehlt = aus), ohne Migration, Zugriff über
  `state.store.pool()`. `web_facts::wanted`: nur `@belege`, nicht `Mode::Propose`, Schalter an.
- **Ablauf** (`search::research`, gesamt `TOTAL_TIMEOUT` 30 s über `tokio::time::timeout`; was bis dahin abgerufen
  ist, bleibt): Sitzung öffnen wie `open_way` (ohne Adresse oder bei fremd gebundenem Token `NotConfigured`); fehlt
  `web_search` oder `fetch_url` in `tools/list` → `Schema`. Bevorzugte Rechner (`source_hosts`: klein, ohne
  `www.`, ohne Doppelte) – höchstens `MAX_PREFERRED_CALLS` 3 Aufrufe „site:<Rechner> <Anfrage>“, dann ein
  allgemeiner (entfällt bei schon 8 Treffern). Treffer ohne Doppelte, höchstens `MAX_HITS` 8, `preferred` nach
  `host_matches` (gleich oder Unterrechner, nie bloßes Endstück). Die ersten `MAX_FETCHES` 4 über `fetch_url`
  (`FETCH_LIMITS`: 10 s, Antwort ≤ 4 MiB, weil MultiGPT die Seite in JSON und doppelt liefern kann); die Seite
  selbst ≤ `PAGE_MAX_BYTES` 1 MiB, sonst Hinweis. Je Aufruf `CALL_TIMEOUT` 10 s; ein gescheiterter Aufruf ist ein
  Hinweis, kein Abbruch. Seiten: Text höchstens `CHECK_CHARS` 200 000 Zeichen, Abrufzeit gespeichert in UTC, angezeigt (Abrufdatum in Belegen und Quellenzeile) in Ortszeit.
- **HTML → Text** (`fetch::html_to_text`): `script`, `style`, `noscript`, `template`, `svg`, `head` fallen weg
  (fehlt das schließende Tag, bleibt der Text), Block-Elemente werden Zeilen, Zeilenumbrüche im Quelltext
  Leerraum, `<` ohne Buchstaben dahinter ist Text, Entitäten (benannte der deutschen Typografie, numerische).
  Titel aus den Daten oder `<title>`.
- **Block** in der Nutzer-Nachricht nach dem Block der Bedeutungssuche: „Abgerufene Webseiten (…):“, je Seite
  `[W<n>] <Titel> · <URL> · abgerufen <TT.MM.JJJJ>[ · bevorzugte Quelle]` und höchstens `BLOCK_CHARS` 1 500 Zeichen
  zwischen `<<<seite` und `>>>`, entschärft mit `rag_facts::defuse`; darunter die Hinweise, ohne Seite „- keine
  Seite abgerufen“, bei Ausfall die Zeile „Hinweis: Websuche ausgefallen (<Grund>)“. Regel `web_facts::SYSTEM_RULE`
  im System-Prompt. In `shed` fällt dieser Block als erster (von hinten).
- **Prüfung** (`verify::check`) nach `mark_unknown_sources`: Zitate („…“, “…”, "…", »…«) in derselben Zeile wie ein
  bekanntes `[Wn]` → Teilstring der normalisierten Seite (Leerraum, typografische Anführungszeichen, weiches
  Trennzeichen; Satzzeichen am Ende des Zitats zählen nicht, „…“/„[…]“/„...“ = Lücke in Reihenfolge), sonst
  „(nicht belegt)“; `[Wn]` ohne Seite „(unbekannte Quelle)“; Adressen, die nicht abgerufen wurden (ohne Anker
  und `/` am Ende verglichen), „(nicht abgerufen)“; ohne Beleg (kein `belegt`/`nicht_zitiert`) „Angabe prüfen:
  nicht belegt“; immer die Zeile „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“ (nur genannte Seiten, sonst
  „Quellen: keine abgerufene Seite genannt“). Kennzeichnungen werden nicht doppelt gesetzt.
- **Belege** (`web_evidence`, Migration 0034): `id, post_id → post ON DELETE SET NULL, work_id, node_key, persona,
  label, url, title, preferred, accessed, quote, verdict ∈ belegt|nicht_belegt|nicht_zitiert, page_hash,
  library_title_id, created`, Index `web_evidence_post`. Je Zitat eine Zeile, eine genannte Seite ohne Zitat eine
  Zeile `nicht_zitiert`; geschrieben nach dem Speichern des Beitrags (`post_id`). `page_hash` ist der SHA-256 des
  geprüften Seitentexts (`baruch_store::sync_hash`; Hashes nur in baruch-store). Der Beitrag bekommt
  die Quelle „Web: n Beleg(e)“, `fragments/assistants.html` verlinkt ihn dann auf die Belegseite.
- **Übernahme** (`evidence::adopt`, unter der Schreibsperre): nur `belegt`, nicht schon übernommen; über die
  vorhandene API von baruch-library (`add_title` mit Schlüssel `web-<rechner>-<JJJJMMTT>[-n]`, bzw. vorhandener
  Titel `web` mit gleicher Adresse; `add_quote`; `add_highlight` Art `quote`, Mensch), dann `library_title_id` und
  `rag::notify(state, None)`.
- **Entscheidung: kein MCP-Werkzeug.** Die Websuche hat ihren Wert in der Prüfung innerhalb von `@belege`; ein
  MCP-Durchgang ins Internet vergrößerte die Angriffsfläche (ein Client könnte Baruch als Abrufdienst nutzen),
  und MCP-Clients haben eigene Websuchen. Die Belege stehen im Beitrag und sind über `read_node` (posts) lesbar.
- **Sitzung:** die gemeinsame `connections::multigpt::Session` (Vorrat, Vertrag §2.2); die Werkzeugnamen
  `WEB_SEARCH_TOOL`/`FETCH_URL_TOOL` kommen aus `connections/multigpt.rs` (in `websearch/mod.rs` neu exportiert).
- **Nachbauten** (`websearch::mock`): `MockWebMcp::start(Some(&site))` (MCP über Streamable HTTP, `web_search`
  mit Filter „site:“, `fetch_url` holt `*.example` von der `MockSite` auf 127.0.0.1 mit dem Rechner als `Host`,
  alles andere `isError`; `DELETE /mcp` → 200; `requests`, `tool_calls`, `configure` mit `search_delay`,
  `search_error`, `text_results`, `bearer`) und `MockSite::with_fixtures()` (Lichtkunde, Archiv, Gästebuch mit
  Prompt-Injektion, eine Seite über 1 MiB, eine, die erst nach 15 s antwortet). Tests `tests/round13_websuche.rs`,
  `crates/baruch-store/tests/migration_0034.rs`.

#### Annahmen (ungeprüft gegen das echte MultiGPT)

- `web_search` nimmt die Anfrage unter einem der Namen aus `QUERY_KEYS` (`query`, `q`, …) und versteht
  „site:<Rechner> …“ wie SearXNG; die Trefferzahl unter einem der `LIMIT_KEYS` (z. B. `max_results`), geklemmt an
  `minimum`/`maximum`. Ohne `properties` schickt Baruch nur `{query}`.
- Die Antwort von `web_search` ist eine Liste (direkt oder unter `results`, `hits`, `items`, `data`, `documents`,
  `organic`, `result`, `web`, auch eine Ebene tiefer) mit `url`/`uri`/`link`/`href`, `title`/`name`/`filename` und
  `snippet`/`content`/`description`/`summary`/`text` – in `structuredContent` oder als JSON in `content[].text`;
  sonst liest Baruch Markdown-Links `[Titel](URL)` und freie Adressen aus dem Text.
- `fetch_url` nimmt die Adresse unter `url`/`uri`/`link`/`href` und liefert den Inhalt unter
  `content`/`text`/`body`/`markdown`/`html` (sonst als Text in `content[]`), wahlweise mit `title`; HTML oder
  Markdown/Text. Weiterleitungen folgt MultiGPT selbst; Baruch nennt die angefragte Adresse.
- Verlangt ein Schema einen weiteren Pflicht-Parameter, ist das ehrlich `Schema` (keine Suche).

### MCP (`mcp/rag.rs`)

`rag_search`, `rag_status` (nach `bible_search` in `tools/list`), über `assist::hooks`; Herkunft = Client.
Kein `rag_reindex`.

### Abgleich (`sync/`, Migration 0033)

Runde 13, Aufgabe „abgleich“ (Plan „RAG-Abgleich mit MultiGPT“): Texte, nicht Vektoren; jede Seite bettet
selbst ein. Nur der Mensch löst aus (Knopf auf `/abgleich`, RPC `sync.run`); **kein MCP-Werkzeug**, kein Start
beim Hochfahren oder Speichern.

| Datei | Inhalt |
|---|---|
| `sync/mod.rs` | RPC `sync.*` (`handles`/`call`, Dispatch in `rpc.rs` vor `crate::rag::handles`), Lauf im Hintergrund (`start`: prozessweite Markierung je Datenordner, ein Lauf zur Zeit, Zeile in `sync_run`, `wait_idle` für Tests), `Limits` (`DEFAULT`: 500 Dokumente je Sammlung, 2 MiB je Dokument, 64 MiB je Lauf, 30 s je Aufruf; `set_limits` je Datenordner für Tests), `credentials` (Adresse und Token wie `open_way`, ohne Netz), `Link` (Sitzung erst beim ersten Bedarf), `Report` (Zahlen und Hinweise, jede Meldung ohne Token), `local_document` → `LocalDocument {title, url, text, fetched}` (für die Dokumentansicht offline), `collection_for` („Baruch: <Werktitel>“, eine Zeile, ≤ 200 Zeichen) |
| `sync/upload.rs` | Baruch → MultiGPT: `chapters` (ein Dokument je Kapitel: Kapitel bzw. Teil beginnt ein Dokument, folgende Abschnitte gehören dazu; Text = aktuelle Fassungen, Titel „<Nummer> <Titel>“, item `work:<W>/key:<K>`, Hash `sync_hash(Titel + "\n" + Text)`), je Werk mit `upload = 1`: gleicher Hash → nichts; sonst Marke `UPLOADING` in `remote_stamp`, `upload_document`, Stand schreiben, dann `delete_document` des alten; fehlende Kapitel und Stände gelöschter Werke → `delete_document`, Stand weg. Rechte-, Transport- und Schemafehler sperren weitere Schreibvorgänge im Lauf; ein `isError` bei heiler Sitzung (`Link::answered`) ohne Rechte-Wort (`RIGHTS_WORDS`) nur dieses Dokument (Runde 19a) |
| `sync/download.rs` | MultiGPT → Baruch: je gewählter Sammlung `list_documents` (seitenweise über `cursor`/`offset`/`page`, ≤ 50 Seiten; eine volle Seite – so viele Einträge wie erbeten, nach `maximum` gekappt – ohne `has_more`/Fortsetzung gilt als womöglich unvollständig, Runde 19a), gleicher `remote_stamp` und Kopie vorhanden → nicht lesen; sonst `read_document`, Grenzen prüfen, gleicher Hash → nichts; sonst `sync_document` schreiben (gleiche id, damit der Index seine Zeilen behält). Fehlende (bei vollständiger Liste) und abgewählte → lokal entfernt. Hat sich etwas geändert: `rag::notify_data(data, None)` |
| `sync/args.rs` | Argumente aus dem `inputSchema` (`Filler`: Sammlung als Liste oder Text, Pflichtangaben, die Baruch nicht füllt → `RagError::Schema`), Antworten (`listed`, `uploaded_id`, `document_text`), Schlüssellisten `TITLE_KEYS`, `CONTENT_KEYS`, `CURSOR_KEYS`, `MIME_KEYS`, `ID_KEYS`, `STAMP_KEYS` |
| `sync/pages.rs` | `GET /abgleich`, `POST /abgleich/act` (eigene Template-Umgebung mit `base.html`, `sync.html`); die Seite fragt MultiGPT nie, die zuletzt geladenen Sammlungen liegen im Speicher des Prozesses |
| `sync/mock.rs` | `MockDocs` für Tests: MCP-Nachbau mit allen sechs Dokument-Werkzeugen, `DELETE /mcp` → 200, Aufzeichnung (`requests`, `tool_calls`, `clear_requests`), `put_document`/`remove_document`/`set_url`, `deny_write(Some(Deny::Forbidden \| Deny::ToolError))` (kein `docs.write`), `Behavior.stamps`, `page_size`, `delay`; seit Runde 19a `upload_without_id`, `reject_empty`, `silent_cap`, `lose_upload_answer` |

- **Tabellen (Migration 0033):** `sync_work` (Schalter je Werk, Vorgabe 0, Sammlung beim Einschalten
  festgehalten, `confirmed`; geht mit dem Werk), `sync_collection` (gewählte Sammlungen, `position` 1..n),
  `sync_state` (Seite `baruch` bzw. `multigpt`, Sammlung, item, `remote_id`, Hash, `remote_stamp`; `work_id`
  ohne Fremdschlüssel, damit der nächste Lauf Dokumente gelöschter Werke findet), `sync_document` (lokale Kopien,
  `UNIQUE (collection, document_id)`), `sync_run` (Protokoll). Store `baruch-store/src/sync.rs`, `sync_hash`
  (SHA-256 hex, Hashes nur in baruch-store).
- **Lokaler Index:** Art `remote` in `KINDS`; `desired_shared` nimmt `desired_remote` dazu (Stücke über
  `baruch_lancedb::chunk::chunks`, id `remote:<sync_document.id>:<n>`, Dokument `remote:<Sammlung>/<Dokument-ID>`,
  ohne Werk, `meta {collection, document, sync}`); `url_of` → `/quelle/multigpt?collection=…&document=…`.
- **Einstellungen → Werk** (`settings/work.rs`, `settings/werk.html`): Schalter mit Pflicht-Kontrollkästchen,
  Formular an `POST /abgleich/act` mit `back`. **Menü:** Werkzeuge → „Abgleichen“ (`base.html`).
- **Inhalte sind Daten:** Titel und Texte aus MultiGPT werden maskiert ausgegeben und nur als Quelle gesucht,
  nie als Anweisung genutzt; Meldungen laufen durch `scrub` (Token → `***`).

#### Annahmen (ungeprüft gegen das echte MultiGPT)

Der Nutzer prüft sie selbst; der Nachbau `MockDocs` folgt ihnen.
- **`list_documents`:** Parameter für die Sammlung aus `COLLECTION_KEYS` (Pflicht – ohne ihn fragt Baruch nicht),
  wahlweise `limit`/`page_size`/`per_page`/`max_results`, Fortsetzung über `cursor` (Antwort `next_cursor`,
  `nextCursor`, `next_page_token`, `next`) bzw. `offset`/`page` mit `has_more`. Antwort: Liste unter `documents`,
  `items`, `results`, `data` oder `files` (auch eine Ebene tiefer oder die Liste selbst); je Eintrag ID
  (`document_id`, `doc_id`, `documentId`, `id`), Titel (`title`, `document_title`, `name`, `filename`), URL und
  **Änderungszeit oder Hash** (`updated_at`, `modified_at`, `updated`, `modified`, `last_modified`,
  `lastModified`, `content_hash`, `hash`, `sha256`, `checksum`, `etag`, `version` – der erste gefundene wird als
  `<Schlüssel>=<Wert>` in `remote_stamp` gemerkt). Meldet MultiGPT nichts davon, liest Baruch jedes Dokument und
  vergleicht den Hash des Texts (Plan: dann als kleine Erweiterung in MultiGPT vormerken).
- **`read_document`:** ohne Seitenparameter kommt der **ganze** Text, als Klartext in `content[].text` oder in
  den Daten unter `content`/`text`/`body`/`markdown`. Nennt das Schema einen Seitenparameter (`page_key`) und
  die Antwort Seitenangaben (`pages`/`total_pages`/… bzw. `has_more`/`next_page`), liest der Abgleich wie der
  Volltext von quellen alle Seiten und verbindet sie mit Leerzeile (`multigpt::read_all_pages`, gemeinsam mit
  `MultigptWay::read_full`); so gleicht die lokale Kopie dem Online-Volltext (Hash, Index, Prüfung von
  Hervorhebungen offline). Ob das echte MultiGPT seitenweise liefert, ist eine Annahme; beide Formen gehen.
- **`upload_document`:** Parameter Sammlung (`COLLECTION_KEYS`, als Liste, wenn das Schema eine erlaubt),
  Titel (`title`/`name`, sonst `filename` mit `<Titel>.md`), Text (`content`/`text`/`body`/`markdown`), wahlweise
  `mime_type`/`content_type` = `text/plain`. Antwort mit der neuen Dokument-ID (`document_id`/`id` …, auch unter
  `document`/`result`/`data`). Seit Runde 19a gilt ein Hochladen ohne ID als gelungen mit Hinweis: Baruch kann
  das Dokument dann weder ersetzen noch löschen und speichert einen Sperrvermerk (Stand mit Hash, `remote_id`
  leer), damit derselbe Inhalt nicht bei jedem Lauf erneut hinausgeht; nach einer Änderung wird neu hochgeladen,
  das alte Dokument bleibt bei MultiGPT liegen (Hinweis). **Die Sammlung entsteht beim ersten Hochladen von selbst**; ein eigenes Werkzeug zum Anlegen ist
  nicht bekannt.
- **`delete_document`:** Parameter Dokument (`DOCUMENT_KEYS`), dazu die Sammlung, falls das Schema eine kennt.
  Scheitert nach dem Ersetzen eines Kapitels das Löschen der alten Fassung, bleibt ihre ID als eigener Stand
  (`sync_state.item = work:<W>/key:<K>/stale:<alte ID>`, ohne Migration) stehen; er gehört zu keinem Kapitel,
  also löscht ihn der nächste Lauf wie ein fehlendes Kapitel.
- **Umfang Baruch → MultiGPT:** nur die aktuellen Fassungen je Kapitel. Kommentare/Befunde (Beiträge der
  Personas), die `docs/Plan.md` („RAG-Abgleich mit MultiGPT“) auch vorsieht, gehen bisher nicht hinaus – offene
  Abweichung, im Backlog vorgemerkt.
- **Rechte:** ein Schlüssel ohne `docs.write` bekommt HTTP 401/403 oder `isError` – beides wird zum Hinweis.
  Ein `isError` erkennt Baruch als Rechteproblem (sperrt den Rest des Laufs) an Wörtern wie „Berechtigung“,
  „permission“, „forbidden“, „docs.write“, „verweigert“, „denied“ (`RIGHTS_WORDS`); ohne solche Wörter gilt es
  nur dem einen Dokument (Annahme über die Texte von MultiGPT).
- **Vollständigkeit der Liste:** Kappt MultiGPT `limit` still und meldet keine Fortsetzung, ist das nur zu
  erkennen, wenn die Seite so viele Einträge hat wie erbeten (`limit` am `maximum` des Schemas); antwortet der
  Server unter dem erbetenen `limit`, gilt die Liste als vollständig (Annahme).

### Store und Migrationen

- `0031_rag_collections.sql`: `work_rag_collection(work_id → work ON DELETE CASCADE, way ∈ pgvector|multigpt,
  collection 1..200 Zeichen ohne Zeilenumbruch, position ≥ 1, PK (work_id, way, collection))`; keine Zeile =
  keine Sammlung (Weg aus). `set_work_collections`: beschnitten, Doppelte entfernt, ≤ 100, `BEGIN IMMEDIATE`,
  `position` 1..n lückenlos; `RagWay::Lancedb` → `Refused`.
- `0032_persona_rag.sql` siehe oben; `PersonaRow` unverändert.
- Tests `crates/baruch-store/tests/{migration_0031,migration_0032,rag_collections}.rs` (Datenbank im Stand bis
  0030 nachgebaut, verlustfrei, zweites Öffnen gleich, Löschen nimmt die Zeilen mit).
- Runde 13: `0033_sync.sql` (abgleich: `sync_work`, `sync_collection`, `sync_state`, `sync_document`, `sync_run`,
  siehe „Abgleich“), `0034_web_evidence.sql` (websuche: `web_evidence`, siehe „Websuche“; der Schalter liegt ohne
  Migration in `store_meta`, Schlüssel `websearch_belege`), in der Bibliothek `0106_source_remote.sql` (quellen:
  `library_title_remote`, `library_highlight_check`). Tests `baruch-store/tests/{migration_0033,migration_0034}.rs`
  (Store bis 0032 mit Daten nachgebaut) und `baruch-library/tests/migration_0106.rs` (Bibliothek bis 0105): alle
  Zeilen unverändert, nur neue leere Tabellen. `migration_0013.rs` begrenzt die Bibliothek seitdem auf 0105.
- Nutzerfrage 12: `0035_outline_request_add.sql` legt `outline_request` mit der Antragsart `add` neu an (siehe
  „Gliederung über MCP und Anträge“); Test `baruch-store/tests/migration_0035.rs` (Store bis 0034 mit Anträgen
  nachgebaut: alle Zeilen samt id gleich, übrige Schemata gleich, gelöschte id wird nicht neu vergeben).
  `sync_hash` (SHA-256 hex) in `baruch-store/src/sync.rs` füllt auch `web_evidence.page_hash`.

### Bauen und Größe

- `lancedb` bringt rund 260 Crates mit (lance, arrow, datafusion, object_store, parquet); `lance` braucht beim
  Bauen den Protobuf-Übersetzer `protoc` (hier `~/.local/bin/protoc`). Lizenz Apache-2.0, mit AGPL-3.0
  verträglich.
- Jede Test-Binärdatei von `baruch-server` linkt LanceDB statisch mit (≈ 500 MB statt ≈ 145 MB). Mit einer
  Binärdatei je Testdatei (61) wuchs `target/` nach dem vollen Lauf auf 38–40 GB. Seit Runde 13 (Vorlauf „bau“)
  sind die Integrationstests des Servers **eine** Binärdatei: `autotests = false`, `[[test]] name = "server"`,
  `tests/main.rs` mit einem Modul je Fachgebiet (`tests/<gebiet>.rs`), gemeinsame Hilfen `tests/common` und
  `tests/pgvector_support` einmal eingebunden; `every_test_file_is_bundled` meldet eine Testdatei ohne
  `mod`-Eintrag. Gemessen nach `cargo clean` und vollem `cargo test --workspace -j 4`: `target/` 38 GB → 9,0 GB,
  Testbinärdateien 146 → 86, Dauer 18 min 1 s → 13 min 31 s, 1195 → 1196 bestandene Tests (+1 Wächtertest).
  LanceDB bleibt ohne Cargo-Feature fest eingebaut (der Dienst unverändert).
- Ein Prozess für alle Server-Tests: prozessweite Zustände teilen sich jetzt alle Module. PDF-Erzeugung unter
  `common::PDF_SERIAL` (Markierung „PDF entsteht“ je Werk-id), die Modell-Warteschlange über
  `common::model_queue` (nur Tests eines Moduls zugleich: `round10.rs`, `round10_models.rs`), pgvector-Tests
  teilen sich einen Container mit je eigener Datenbank.

### Tests

`tests/round12_lancedb.rs` (Index, Modellwechsel, ohne Einbetter, eine Einbettung zur Zeit),
`round12_pgvector.rs` (eigener podman-Container `docker.io/pgvector/pgvector:pg17`, sonst initdb-Cluster ohne
pgvector, sonst ÜBERSPRUNGEN; Hilfe `tests/pgvector_support/mod.rs`: gleichzeitig laufende Tests teilen sich einen
Container, je Test eine eigene Datenbank `rag_<n>`), `round12_search_ui.rs`,
`round12_multigpt.rs`, `round12_personas_rag.rs` und Ende-zu-Ende `round12.rs` (fester Nachbau-Vektorraum
`Concepts`, `MockMultigpt`, pgvector-Container wenn vorhanden: Index zieht nach, Bedeutung ohne gemeinsames Wort,
drei Wege zusammengeführt ohne Doppelte, Ausfall blockiert nicht, Sammlungen begrenzen, Eliezer aus → FTS und
Grund auf den Seiten, Literatur/Einfälle ohne JavaScript, MCP mit Token, @belege mit Belegblock und
Injektionsversuch).

Runde 13: `round13_quellen.rs`, `round13_websuche.rs`, `round13_abgleich.rs` je Aufgabe und Ende-zu-Ende
`round13.rs`: **ein** MultiGPT als kleine MCP-Vorderseite (`OneMultigpt`), die jedes Werkzeug an den Nachbau seiner
Aufgabe weitergibt (`search_documents` → `MockMultigpt`, `web_search`/`fetch_url` → `MockWebMcp` mit `MockSite`,
Dokument-Werkzeuge → `MockDocs`), Einbettungen über `MockEliezer`: Treffer → Dokumentansicht → Übernahme →
Titel im Index → Prüfung der Hervorhebungen; @belege mit bevorzugter Quelle, „(nicht belegt)“, Injektionsseite,
Übernahme eines Belegs; Abgleich nur nach Bestätigung, nur Geändertes, Holen und offline finden, Ausfall mit
Hinweis, offline die lokale Kopie in der Dokumentansicht.

## Personas wirklich (Runde 10)

### Port und Modell-Backend

- **Port** `assist/port.rs`: `ModelPort { chat(state, ModelCall, deltas: UnboundedSender<String>), models(state),
  alias(name), context(state, persona) → ContextBudget {tokens, reported, warning} }` (seit Runde 15 mit
  Vorgabe-Implementierung = Einstellung der Persona) mit `ModelCall {model, messages, max_tokens, temperature, label,
  persona}` (Handle der Persona, leer = keine; wählt Verbindung und Freigabe), `ModelAnswer {text, model,
  finish_reason, prompt_tokens, completion_tokens}`, `ModelFailure` (`NotConfigured`, `Unreachable`, `Denied`,
  `Timeout`, `UnknownModel`, `Busy`, `Cancelled`, `Failed`; `Display` deutsch, nie ein Geheimnis), `ModelList`.
  `NoModel` meldet ehrlich „Kein Modell-Backend angebunden …“. **Abbruch** = Fallenlassen des Futures (Client
  trennt → Strom fällt → Auftrag fällt → HTTP-Anfrage an Eliezer bricht ab, Platz in der Warteschlange frei).
- **Adapter** `assist/eliezer.rs` (`EliezerPort`): bildet `ModelCall`/`ModelAnswer`/`ModelFailure` 1:1 auf
  `baruch_llm::ChatRequest`/`ChatReply`/`LlmError` ab und ruft `crate::models`.
- **`models.rs`**: `chat(state, ChatRequest, label, deltas)` – Adresse aus der Verbindung `eliezer`, Schlüssel aus
  dem Schlüsselbund `baruch/eliezer` nur bei `connections::secret_bound` (sonst geht gar nichts hinaus), Alias vor
  dem Senden, prozessweite `Queue` (`static`, eine Anfrage zur Zeit, höchstens 8 Wartende); `list(state)` →
  `Listing {models, live, fetched}` (60 s je Adresse zwischengespeichert, nicht durch die Warteschlange);
  `alias`, `status`, `probe` (Kurztest), Obergrenze `BARUCH_MAX_TOKENS` (Umgebung, wahlweise; klemmt
  `max_tokens` jedes Aufrufs, etwa für Handprüfungen mit kurzen Antworten), `persona_states`/`persona_summary` (Abgleich der Persona-Modelle),
  `remember` (Zwischenspeicher aus „Verbindung prüfen“); RPC `model.*` (`handles`/`call`).
- **`baruch-llm`**: Grenzen (`Limits`): Verbindung 10 s, erstes Stück 180 s (Laden des Modells), Pause 60 s,
  gesamt 600 s, Modellliste 10 s, Antwort höchstens 1 MiB, `max_tokens` auf 1–4096 geklemmt, keine
  Weiterleitungen. Nur `choices[0].delta.content` zählt (`reasoning_content` übergangen); `[DONE]` beendet das
  Lesen sofort. Fehler: Verbindung → `Unreachable`; 401/403 → `Denied` (Hinweis `baruch/eliezer`); 404 oder
  „model not found“ → `UnknownModel`; 409/429/503 → `Busy`; sonst `Failed` mit höchstens 300 Zeichen der
  Gegenstelle, Schlüssel daraus entfernt (`scrub`); Zeitgrenzen → `Timeout`. Baruch fordert **nie** eine
  Kontextlänge an und ruft nie `/api/v1/models/load|unload|estimate`.
- `connections::run_check`: ein `tokio::time::timeout` (10 s) um die ganze Prüfung; für Eliezer danach Abgleich
  der Personas (`details.personas`) und `models::remember`. Der Nutzungshinweis der Verbindung Eliezer ist kein
  STUB mehr (`connections::usage_is_stub`).

### Ablauf eines Auftrags (`assist/mod.rs`, `assist::run`)

Persona prüfen (aktiv, sonst -32602) → Kontext lesen **ohne** Schreibsperre (`assist::context::prepare`) →
Modell aufrufen **ohne** Sperre (mit Gemma ~25 s) → Nachbearbeitung (`assist::prompts`, Hook
`hooks::mark_answer`) → erst dann `write_lock`, `key` neu auflösen, schreiben: Beiträge über `Store::add_post`
(Autor = Persona-Handle), Vorschläge über `Store::add_proposal` (immer `proposed`, `model` = tatsächliches
Modell, Notiz „Vorschlag von @x“, Überschriftszeile der gezeigten Fassung erzwungen, `assist::force_heading`).
Fehler oder Abbruch speichern nichts. Chat (`assist::chat`): Beitrag des Absenders sofort, dann je erwähnter
Persona `run` nacheinander (höchstens drei, kein Relay). `assist/rpc.rs` (RPC `persona.ask/review/propose/chat`),
`assist/routes.rs` (`POST /assistenten/act`, `POST /assistenten/stream` mit eigenem `Stream`, der den Auftrag
antreibt; neue Abhängigkeit `tokio-stream`).

- **Kontext** (`assist/context.rs`): System = Rollen-Prompt (+ Hook `scripture_facts`); Nutzer = Werk (Titel,
  Art, Zielgruppe), Kapitelumfeld (Kapitel, Nachbarabschnitte mit je ≤ 300 Zeichen Anfang), beim Lektor das
  Gedächtnis (Einfälle mit `idea.node_id = key`, nicht gelöscht, dazu ≤ 10 offene ohne Bezug), im Chat die
  letzten 8 Beiträge, dann der Abschnitt der gezeigten Fassung (`rev`, sonst die aktuelle) zwischen `<<<`/`>>>`,
  zuletzt der Auftrag. Tokens ≈ ⌈Zeichen/3⌉, Budget = Kontextlänge − `max_tokens` − 256; gekürzt wird zuerst
  das Kapitelumfeld, dann Gedächtnis bzw. Chatverlauf, zuletzt der Abschnitt vom Ende her mit
  „[… gekürzt: n Zeichen]“ → `truncated: true`. `max_tokens`: ask 700, review 900, chat 500, propose
  min(4096, Abschnittstokens·1,5 + 200); `temperature` 0,4; `label` „@x <art> key K“ (ohne Text).
- **Rollen-Prompts** (`assist/prompts.rs`): Vorgaben für `@lektor`, `@exeget`, `@schrift`, `@belege`, `@nathan`,
  `@hoerer`, sonst allgemein aus der Rolle; ein gespeicherter Prompt (`persona.prompt`, Migration 0015) ersetzt
  die Vorgabe ganz. Harte Regeln mit Gegenbeispielen („Falsch: … / Richtig: …“) – siehe `docs/Bedienung.md`
  „Assistenten fragen“.
- **Nachbearbeitung**: Beginnt die Antwort (nach Leerraum, Anführungs- und Markdown-Zeichen, ohne Rücksicht auf
  Groß-/Kleinschreibung, nur ganze Wörter) mit „Als Assistent“, „Als KI“, „Als Sprachmodell“, „Gerne“ oder
  „Natürlich“, fordert Baruch **einmal** neu an (Zusatzanweisung `prompts::RETRY_NOTE` am Ende des
  System-Prompts); beginnt auch die zweite so, fällt ihr erster Satz weg (bis `.`/`!`/`?`/`:` mit Leerraum
  danach oder bis zum Zeilenende). Beim Strom hält Baruch die ersten 24 Zeichen zurück, bis klar ist, ob die
  Antwort so beginnt. Vorschläge: Codeblock ``` entfernt.
- **Hooks** (`assist/hooks.rs`): `scripture_facts` → `crate::scripture::facts_for`, `mark_answer` →
  `crate::scripture::mark_answer`.
- **Lektor**: `baruch_core::LectorChoice`, gespeichert in `store_meta` Schlüssel `lector` (`auto` oder ein
  Handle; keine Zeile = fest `@lektor`, Versuch seit 2026-11-06); Store `lector_choice`/`set_lector_choice`;
  `personas::lector_of(rows, &choice)`. Ist die feste Persona inaktiv, gilt „automatisch“.
- **Migration 0015** `persona.prompt TEXT CHECK (NULL oder 1–8000 Zeichen)`; Store `persona_prompt`,
  `set_persona_prompt`, `PersonaRow.prompt`, `MAX_PROMPT_CHARS`. Keine Datenänderung.
- **Oberfläche**: `fragments/assistants.html` (Formular `assist-form` mit Prüfen/Vorschlag, Chat-Eingabe an
  `persona.chat`), Widget `web/src/assist.ts` (`fetch` + `AbortController`, SSE-Zerleger `parseEvents`,
  vorläufiger Beitrag in Persona-Farbe, danach `baruch:navigate {url}`), Stil `static/assist.css`; Einstellungen
  `settings/assistants.rs` + `settings/assistenten.html` (Lektor-Formular, `<datalist id="baruch-models">` aus
  `state.model.models`, Warnungen über `state.model.alias`, Rollen-Prompt).
- **Testmodell** `assist::mock::ScriptedModel` (öffentlich, nur für Tests): Skript je Aufruf (`Step::words`,
  `Fail`, `Hang`, `Wait` + `release()`), `calls()`, `cancelled()`.

### Lektor mit ganzem Kapitel (Runde 15, „lektor“)

Überholt damit die Punkte „Kontext“ (Kapitelumfeld mit 300 Zeichen je Nachbar, Kürzung des Abschnitts) und
„Nachbearbeitung“ (verbotene Einleitung auch beim Vorschlag) oben.

- **Schnittstelle S2:** Chat in `assist/chat.rs` (`mentions`, `chat`, `ChatOrder`, `run_chat`, `chat_json`);
  `Progress` um `Posted(SavedPost)` (Beitrag des Absenders gespeichert), `Retry {persona, reason}` (Neuanforderung:
  „verbotene Einleitung“ bzw. „fehlendes „Kritisch:““) und `Compressed(Compression)`; `Compression {message,
  summarized, chunks}`, `Scope {Section (Vorgabe), Chapter}`, `Order.scope`, `Outcome.compressed`,
  `Outcome.proposals` (Vorschlag für das Kapitel). SSE-Weiterleitung in `assist/routes.rs`.
- **Kontext** (`assist/context.rs`, `prepare_inputs` mit `RagInput`/`BibleInput`): `chapter_units` liefert die
  Teile des Kapitels (nächster `NodeKind::Chapter` über dem Abschnitt oder er selbst, mit allen Knoten darunter;
  bei `WorkKind::Sermon` das ganze Werk; ohne Kapitel der Knoten mit Unterpunkten), je Teil die aktuelle Fassung,
  der eigene in der gezeigten. Die Nutzer-Nachricht baut `Frame::call`: Vorspann (Werk, Befunde der anderen
  Personas – Beiträge, deren Autor das Handle einer anderen Persona ist, je Teil die letzten drei, im Chat ohne
  den eigenen Abschnitt –, Gedächtnis, Chatverlauf, Faktenblöcke), dann „Kapitelumfeld:“ mit jedem Teil als
  `--- Titel ---` und `<<< … >>>` (Wortlaut, „nur Zusammenfassung“ oder „nicht mitgegeben“; der Prüfgegenstand
  als Verweis), dann der Prüfgegenstand mit Kopf, zuletzt der Auftrag. `Scope::Chapter`: alle Teile sind der
  Prüfgegenstand (`max_tokens` ask 900, review 1500).
- **Budget:** `state.model.context(state, &persona).await.tokens` (Schnittstelle S1, `ModelPort::context`) −
  `max_tokens` − 256, in Zeichen ·3. Zusätze (`Extras`: Web, SWORD, RAG, Befunde, Gedächtnis von hinten, Chat von
  vorn) werden gekürzt, solange etwas zu viel ist und sie mehr als ein Drittel des Budgets belegen; reicht der Platz
  danach nicht für den größten Teil des Prüfgegenstands samt 600 Zeichen Umfeld, auch darunter (`truncated`).
  Kapitel und Prüfgegenstand werden nie gekürzt; `Prepared::fits` sagt, ob sie im Wortlaut passen. Passt nicht
  einmal das Nötigste (weniger als 600 Zeichen Platz), lehnt `prepare` ehrlich ab (-32000).
- **Stückeln** (`assist/chunk.rs`, rein; Orchestrierung in `assist/mod.rs`, `compressed_run`): Prüfgegenstand in
  Stücke von höchstens ⅔ des Platzes (passt damit nicht einmal die Untergrenze unten, der Reihe nach ½, ⅓, ¼ –
  `PIECE_SHARES`: ein kleinerer Kern lässt dem Umfeld mehr Platz) (`chunk::split`, Schnitt an Absatz, sonst Satzende, sonst Leerraum,
  Überlappung ⅛, höchstens 600 Zeichen; ein Abschnitt bis ¾ des Platzes bleibt ganz; Kapitel: `chunk::pack`). Je
  Stück plant `chunk::plan_piece` das Umfeld: alles im Wortlaut, solange es nicht passt der vom Kern fernste Teil
  als Zusammenfassung, danach – nur wenn es Platz spart – als bloßer Titel. Die nötigen Zusammenfassungen holt
  `plan_and_summarize` **vor** dem ersten Prüfaufruf und **nacheinander** (eigener Aufruf, `SUMMARY_SYSTEM`,
  `max_tokens` 80–300, Bezeichnung „… Zusammenfassung“, `persona` gesetzt – die Freigabe „extern“ gilt auch hier);
  danach wird mit den echten Längen neu geplant (höchstens drei Runden). Der Platz je Stück ist `frame.room` minus
  `piece_extra` (je Kernteil die Umfeldzeile „wird geprüft, steht unten“, dazu was Kopf „Stück k von n (…)“ und
  Zusatz im Auftrag bzw. der eigene Auftrag beim Vorschlag länger sind als in `prepare` gezählt). Vor dem ersten
  Modellaufruf prüft `plan_and_summarize` eine Untergrenze je Stück (Kern im Wortlaut, jeder andere Teil so knapp
  wie möglich); reicht nicht einmal sie, ehrliche Ablehnung (`too_many_parts`, -32000, nichts gespeichert). Danach
  werden alle Aufrufe gebaut und **gemessen** (`check_calls`: Zeichen aller Nachrichten ≤ (Kontextlänge −
  `max_tokens` − 256) · 3), bevor der erste hinausgeht; auch der ungestückelte Aufruf wird gemessen und bei
  Überschreitung gestückelt. Dann `Progress::Compressed`, dann je Stück
  ein Aufruf (Strom mit Leerzeile zwischen den Stücken), Befunde mit `chunk::merge` ohne Doppelte (gleich oder einer
  ganz im anderen, ab 24 Zeichen; der ausführlichere bleibt). Vermerk „Komprimiert: …“ (`compression`) als letzter
  Absatz des Beitrags bzw. in der Notiz des Vorschlags. Ein Vorschlag zum Abschnitt wird nie gestückelt (der
  Abschnitt muss ganz passen, sonst `too_long_for_proposal`).
- **Vorschlag für das Kapitel** (`propose_chapter`): je Teil mit Text ein Aufruf nacheinander (Teil im Wortlaut,
  Umfeld geplant wie oben, Auftrag `propose_part_task`), danach `proposal_text` (Codeblock, „Kritisch:“-Zeilen weg,
  Überschrift erzwungen); unveränderte und abgeschnittene Teile fallen weg; gespeichert wird erst am Ende unter
  **einer** Schreibsperre, je Teil genau ein `proposed`.
- **„Kritisch:“** (`prompts::needs_critical`, `has_critical`): `answer_call` fordert bei Lektor, Exeget und Nathan
  höchstens einmal je Auftrag nach (`CRITICAL_RETRY_NOTE`, `Progress::Retry`); scheitert die Nachforderung, bleibt
  die erste Antwort. Fehlt er im gespeicherten Text, hängt `post_body` `CRITICAL_MISSING_NOTE` an. Beim Vorschlag
  prüft `call_model` keine Einleitung (`check_opening = false`), `strip_critical_lines` entfernt Befundzeilen.
- **Faktenblock aus SWORD** (`assist/bible_facts.rs`, Haken `hooks::bible_research` → `crate::bible_research::research`,
  Daten und Schalter beim Zweig „anbieter“): Block „Belege aus SWORD“ mit `[Sn] <Art> <Modul> · <Stelle> · Lizenz:
  …` und Text zwischen `<<<beleg`/`>>>`, entschärft mit `rag_facts::defuse`; Hinweise der Recherche darunter;
  Bibelwortlaut, der schon im Faktenblock der Stellenprüfung steht, fällt weg; Regel `SYSTEM_RULE`; Budget gemeinsam
  mit RAG/Web (`BIBLE_MAX_CHARS` = min(4000, Budget/5) an `research`); `[Sn]` ohne Eintrag → „(unbekannte Quelle)“
  (`rag_facts::mark_unknown`). `research()` ist bis zur Integration ein Rumpf (`None`) – Tests über
  `BibleInput::Given`.
- **Oberfläche:** `templates/fragments/toast.html` (Hinweiskasten `div.toast.toast-<level>[data-widget=toast]`,
  ohne JavaScript sichtbar), Widget `web/src/toast.ts` (verschiebt Kästen in `.toast-stack`, „Schließen“, lauscht
  auf `baruch:toast {message, level, summarized?}`; „warn“ ohne Zeitablauf, „info“ 8 s), Stil `static/toast.css`;
  `templates/fragments/assist_scope.html` (Optionsfelder `scope`), eingebunden vom Zweig „auftraege“.

### Bibelstellen (`baruch-scripture`, `scripture.rs`)

- `data/versification.tsv`: je Zeile `OSIS<TAB>Kapitel<TAB>Verse`, 66 Bücher, 1189 Kapitel, 31172 Verse; nur
  Zahlen, **deutsche Zählung**. Herkunft: Verszählung „German“ des SWORD-Projekts (CrossWire Bible Society,
  `include/canon_german.h`, SHA-256 im Kopf der Tabelle, abgerufen 2026-11-06); das SWORD-Modul der Schlachter 1951
  (`GerSch`) ist in dieser Zählung angelegt (`Versification=German`). AT: Bücher, Kapitel, Verse aus
  `otbooks_german`/`vm_german`; NT: Kapitel wie die SWORD-Standardliste, Verse aus `vm_german`. Übernommen sind nur
  Zahlen. Nicht mehr verwendet: eBible.org `deu1951` (VPL und USFM) ist auf die englische Zählung (KJV, 31102 Verse)
  umgestellt – bis 2026-11-06 stand sie hier und meldete z. B. „Ps 51,21“ als ungültig. Der Test
  `versification.rs` prüft die Summe 31172 und Joel 4/Maleachi 3, `references.rs` Stellen, die nur deutsch
  gelten. Nie von Hand ändern. Lizenz: `canon_german.h` steht unter GPLv2; übernommen sind nur die Zahlen als
  Tatsachen, kein Code (Entscheidung des Nutzers 2026-11-06).
- Parser: Ein Komma mit Leerraum vor einem nummerierten Buch („Joh 3, 1. Joh 4“) trennt Stellen und leitet keinen
  Vers ein (`verse_mark`). Seit Runde 17b: Verslisten mit Komma (`comma_verse`), Zahlwörter (`COUNT_WORDS`,
  `count_word_follows`), Uhrzeiten nach Wochentag (`weekday_clock`, nur `scan`), „… Vers 23“ im selben Satz
  (`verse_later`) – siehe „Runde 17b – stellen“.
- `src/books.rs`: 66 Bücher (OSIS, Name, Abkürzung, weitere Schreibweisen); Vergleich ohne Punkte, Leerraum,
  Groß-/Kleinschreibung; jede Schreibweise genau einem Buch. Parser: rekursiver Abstieg über Byte-Stellen, nie
  Panik.
- Server `src/scripture.rs`: RPC `scripture.check` (`check(text)`, gemeinsam mit MCP `scripture_check` in
  `src/mcp/scripture.rs`), `facts_for(handle, section)` (Faktenblock nur für `@schrift`/`@belege`),
  `mark_answer(handle, section, body)` (Zeile „Wortlaut: ungeprüft …“ bzw. Hinweis auf @schrift). Nur lesend.
  Seit 2026-11-06 mit Bibel: `facts_for_work`/`mark_answer_for_work` (Hooks in `assist/hooks.rs`, async mit
  `state` und Werk), `check_with(choice, text)`; `mark_with` setzt auseinandergerissene Ziffern einer Stelle des
  Abschnitts in der Antwort wieder zusammen (`wording::rejoin_split_refs`, Befund „Mk 1 0,42–45“: Parser und
  Anzeige schreiben die Zahl richtig, die Lücke stand im Text des Modells).

### SWORD-Bibeln (2026-11-06, `baruch-sword`, `bibles.rs`, Migration 0030)

- **Entscheidung Leser (Format):** Die Crate **`sword-modules` 0.1.0** (Logoxide, **CC0-1.0**, reines Rust, kein
  FFI) liest das Dateiformat; geprüft im Quelltext (`~/.cargo/registry/src/…/sword-modules-0.1.0`): Treiber zText,
  zText4, RawText, RawText4 (dazu Kommentare/Lexika, die Baruch nicht nutzt), conf nach `docs/spec/conf.md`
  (Fortsetzungszeilen, RTF-`About`), `DataPath` relativ zum Modulordner (führendes `./` bzw. `/` entfernt), alle 18
  SWORD-Verszählungen als Tatsachen aus der Laufzeit-API, Index je Testament (Stelle 0 Modulkopf, 1 Testamentkopf,
  je Buch Einleitung, je Kapitel Einleitung, dann Verse), Block-Zwischenspeicher, Normalisierung der Einträge. Sie
  dokumentiert eine Reinraum-Ablösung früher aus libsword portierter Teile (`CLEANROOM.md`); Abhängigkeit
  `topos-bible` 0.6 (CC0-1.0, Stellenangaben, von Baruch nicht benutzt) und deren regex, smallvec, itertools u. a.
  (MIT/Apache). Eingebunden mit `default-features = false, features = ["zip"]`: kein Installer, kein Netz (ureq,
  zip, tar), kein bzip2/xz (seit Runde 17c `features = ["zip", "bzip2", "xz"]`, weiter ohne `install`/`https`). **Nicht verwendet:** libsword (GPL-2.0) und `rsword_chirho` (GPL-2.0-or-later), kein
  Code daraus. Eigener Code von Baruch: Modulordner und Auswahl, Prüfung der Unterstützung, `DataPath`-Wache
  (`..` → abgelehnt), OSIS/ThML/GBF → Klartext, Versifikationsabgleich, Wortlautvergleich, Suche, Test-Module.
- **Crate `baruch-sword`** (rein bis auf Dateizugriff): `scan(dirs)` → `BibleInfo` (Name, Beschreibung, About als
  Text, Versification, Lang, Version, ModDrv, CompressType, BlockType, SourceType, Encoding, DistributionLicense,
  Copyright, `notice` = Lizenzvermerk, `supported`/`problem`), erste Fundstelle eines Namens gewinnt; nur
  Bibelmodule. `open(info)` → `Bible` (`verse`, `intro`, `slot_text`, `passage` höchstens 400 Verse, `search` linear
  mit Treffer- und Zeitgrenze). Unterstützt: zText/zText4 mit ZIP oder LZSS, RawText/RawText4; Verszählung nur
  „German“ und „KJV“ (`baruch_scripture::versification`), andere → „nicht unterstützt“ (STUB in
  `versification.rs`); verschlüsselte Module → STUB. (Stand vor Runde 17; seit R17 jede Zählung von
  `baruch_scripture::versification()`, BZIP2/XZ und `CipherKey`, siehe „Runde 17c – pruefanzeige“.) `text::to_plain`: `note`, `rdg`, `title` (außer
  `canonical="true"`), `catchWord`, `figure` (ThML auch `scripRef`, `h1`–`h6`; GBF `<RF>…<Rf>`, `<TS>…<Ts>`)
  fallen weg; `divineName`, `w`, `q`, `transChange` bleiben als Text; Entitäten, Leerraum zusammengefasst.
  `fixture`: schreibt Test-Module (zText BOOK/CHAPTER/VERSE mit zlib, RawText) mit **eigener** Stellenrechnung aus
  den Zahlen von `baruch-scripture`; Tests `tests/modules.rs` lesen sie über `sword-modules` zurück (jede Stellenart,
  beide Zählungen, alle Blockarten) und vergleichen die Tabellen von `sword-modules` Vers für Vers mit unseren.
  Beleg am echten Modul GerSch: `ot.bzv` 241850 Bytes = 24185 Stellen, `nt.bzv` 8246 Stellen – genau die
  Stellenzahl der Zählung German.
- **Verszählung KJV:** `baruch-scripture/data/versification-kjv.tsv` (31102 Verse; die frühere eBible-Tabelle aus
  der Historie vor `6e1e268`), gleich der KJV-Tabelle von `sword-modules`; `versification::{german, versification}`.
- **Wortlaut** (`baruch_scripture::wording`, rein): `inline_quotes` („…“, “…”, »…«, «…», "…", `#quote[…]`; eine
  Leerzeile beendet die Suche), Blockzitate über `baruch_wysiwyg::parse` (`BlockKind::Quote`, Bereich des Inhalts
  per Klammerzählung, `bibles::block_quotes`), `quote_for` (Zitat direkt vor bzw. nach der Stelle, dazwischen nur
  Leerraum/Satzzeichen/andere Stellen derselben Liste, höchstens 24 Bytes; Blockzitat mit der Stelle in Zuschreibung
  oder am Ende), `words` (Typst-Auszeichnung weg, Bindestriche/Apostrophe/weiches Trennzeichen im Wort weg,
  Trennung „Hei- land“ verbunden außer vor „und/oder“, klein), `compare` (lückenlose Wortfolge je Teil zwischen
  Auslassungen → stimmt, sonst Wort-Diff über die längste gemeinsame Teilfolge, auf den zitierten Bereich
  beschränkt). Andere Zählung im Modul: geprüft nur, wenn jedes betroffene Kapitel in German und im Modul gleich
  viele Verse hat, sonst `versification_mismatch` (keine Abbildung zwischen Zählungen).
- **Server `src/bibles.rs`:** RPC `bible.*`, `Choice` (keine, fehlt, nicht lesbar, bereit), Zwischenspeicher
  geöffneter Module je conf-Datei und Änderungszeit (höchstens 8), Lesen in `spawn_blocking` für Scan, Öffnen und
  Suche; `check_wording` je Stelle. Seite Einstellungen → Quellen (`connections/pages.rs`, `settings/quellen.html`,
  Block `#bibeln`). MCP `src/mcp/bibles.rs`. Die echten Modulordner liest nur der laufende Dienst; Tests setzen
  immer eigene Ordner.
- **Store (Migration 0030):** `bible_dir (position, path UNIQUE)` und `store_meta.bible_dirs` (gesetzt = eigene
  Liste, auch leer; fehlt = Vorgaben), `work_bible (work_id → work ON DELETE CASCADE, module)`; `bible_dirs`,
  `set_bible_dirs`, `work_bible`, `set_work_bible` (`src/bibles.rs`), kein Commit im Spiegel (kein Buchtext). Test
  `tests/migration_0030.rs` (verlustfrei, zweites Öffnen gleich, Löschen des Werks nimmt die Wahl mit).

### Verbindung je Persona (Runde 15b, `models.rs`, `baruch-llm/src/client.rs`, Migration 0036)

- **`baruch_llm::Client`** (Vertrag §3): `BackendKind {Eliezer, Openai, Ollama}`, `ModelInfo {id, context_length}`,
  `Client::new(kind, Endpoint, Limits)` mit **einem** `reqwest::Client` je Endpunkt (Klone teilen ihn; Verbindungen
  bleiben im Vorrat von reqwest), `models()` (`GET /v1/models`, bei Eliezer dazu die Kontextlängen aus der
  Verwaltungsliste), `context_length(model)`, `chat_stream`. Die freien Funktionen `models`, `models_with`,
  `chat_stream`, `chat_stream_with` bleiben als Hüllen (eigener HTTP-Client je Aufruf, wie bisher). Test
  `tests/backends.rs` zählt am Nachbau (keep-alive) die TCP-Verbindungen: vier Abfragen über einen Client = eine
  Verbindung, zwei freie Abfragen = zwei.
- **Gemeldete Kontextlänge – Annahmen:**
  - *Eliezer:* `GET /api/v1/models` (Verwaltungsliste in der Form von LM Studio 0.4) mit `key`,
    `max_context_length` und `loaded_instances[].config.context_length`. Quelle: `docs/wiki/API.md` und
    `crates/eliezer-api/src/lib.rs` im Projekt Eliezer (gelesen, nicht gegen den laufenden Dienst geprüft).
    Gemeldet wird die Länge der geladenen Instanz, sonst `max_context_length`. Nur lesend; nie
    `/api/v1/models/load|unload|estimate`. Ob Eliezer ein noch nicht geladenes Modell mit genau dieser Länge
    lädt, sieht Baruch weiterhin nicht.
  - *Ollama:* gemeldet wird die **wirksame** Länge, nie der Höchstwert: Der Chat läuft über
    `/v1/chat/completions` ohne `num_ctx` (die OpenAI-Schnittstelle von Ollama nimmt keins), Ollama nimmt dann
    `num_ctx` aus dem Modelfile, sonst seine Vorgabe, und schneidet längere Eingaben still vorn ab. Darum
    `ollama_context`: `num_ctx` aus `parameters` von `POST /api/show {"model"}`; sonst `context_length` des
    geladenen Modells aus `GET /api/ps` (`ollama_loaded_context`, Annahme: neuere Fassungen melden es; fehlt
    `/api/ps`, zählt das nicht als Fehler); sonst `OLLAMA_DEFAULT_CONTEXT` = 4096 (Annahme nach der Ollama-Doku;
    eine größere Vorgabe über `OLLAMA_CONTEXT_LENGTH` sieht Baruch erst bei geladenem Modell) – jeweils höchstens
    `<arch>.context_length` aus `model_info`. Alles ungeprüft gegen einen echten Ollama, nur gegen den Nachbau
    (`MockEliezer` mit `Flavor::Ollama`: `/api/show`, `/api/ps` ohne geladenes Modell). Modellliste über
    `/v1/models`.
  - *OpenAI-kompatibel:* keine Meldung; es gilt die Einstellung der Persona.
- **Server `models.rs`:** Verbindungen aus dem Store (`model_connection`), `resolve` (Adresse, Schlüssel aus dem
  Konto der Verbindung, nur wenn an das jetzige Ziel gebunden), `client_for` (Vorrat je Art, Adresse und
  Fingerabdruck des Schlüssels), Warteschlange **je Endpunkt** (`status()` fasst zusammen), `route(state, persona)`
  mit der Freigabe „extern“ (externe Verbindung ohne `persona_backend.external_allowed` → `NotConfigured`, keine
  Anfrage), `chat_on`, `list_on` (60 s Zwischenspeicher, bei Fehler die abgelaufene Liste mit Hinweis),
  `context` (min(Einstellung, Meldung), 60 s gemerkt, höchstens 10 s Wartezeit, scheitert nie),
  `context_cached` (ohne Netz, für `persona.list` und die nicht gezeigten Tabs), `probe_on` (Ergebnis in
  `model_connection.probe`), `check_connection`, RPC `model.*`. `EliezerPort::chat` ruft `route` mit
  `ModelCall::persona`, `EliezerPort::context` ruft `models::context` – so gilt die Freigabe für jeden Modellaufruf
  der Personas (auch Zusammenfassungen des Lektors), und der Lektor bekommt sein Budget über `state.model.context`.
  Einbettungen (`rag/embed.rs`) nutzen weiter die Verbindung `eliezer` und die freien Funktionen; ist die
  Modell-Verbindung Eliezer (id 1) als extern markiert, verweigert `rag::embed::endpoint` jede Einbettung
  (`EXTERNAL_REFUSED`, als „kein Einbettungsmodell“ gemeldet) – Einbetten hat keine Persona, also keine Freigabe.
- **Lage nach der Adresse** (`models::looks_local`, ohne DNS): lokal sind Loopback, private, CGNAT- und
  Link-local-Adressen, ULA, Rechnernamen ohne Punkt und `.localhost`/`.local`/`.lan`/`.internal`/`.home.arpa`.
  `model.connection_set` ohne `external`: beim Anlegen `!looks_local(url)`, beim Ändern bleibt extern, und eine neue
  nicht erkennbar lokale Adresse macht extern. `describe` setzt `locality_warning`, wenn eine lokal geführte
  Verbindung auf eine nicht erkennbar lokale Adresse zeigt (Anzeige auf der Seite, keine Sperre: ein Intranet-Name
  darf bewusst lokal sein).
- **Store (Migration 0036):** `model_connection (id, name UNIQUE NOCASE, kind, url NULL nur bei id 1, external,
  account UNIQUE, secret_target, probe, checked, position, …)` ohne AUTOINCREMENT (sonst änderte die Migration
  `sqlite_sequence`, das die älteren Migrationstests vergleichen); neue ids vergibt der Store über
  `store_meta.model_connection_last` nie doppelt (ein neues Konto `baruch/model-<id>` erbt nie den Schlüssel einer
  gelöschten Verbindung). Die Verbindung Eliezer (id 1) stammt aus der Migration, ihre Adresse und Bindung bleiben
  in `connection` (nicht doppelt geführt), ein Trigger verhindert ihr Löschen. `persona_backend (persona_id,
  connection_id ON DELETE CASCADE, external_allowed)`; keine Zeile = Eliezer, nicht freigegeben. `PersonaRow` bleibt
  unverändert. Test `tests/migration_0036.rs`.
- **Seiten:** Einstellungen → Verbindungen, Abschnitt `#modelle` (`connections/pages.rs`, Formular
  `POST /einstellungen/modelle/act` im Router von `connections::writes`, hinter `guard::same_origin`);
  Einstellungen → Assistenten, je Tab `action=backend` und `action=bible` (`personas.rs`, `settings/assistants.rs`).

### Kommentare und Lexika (Runde 15, Punkte 5a–5c, Migration 0037)

- **`baruch-sword/src/reference.rs`:** `Commentary` (RawCom, RawCom4, zCom, zCom4; Verszählung German oder KJV;
  `entries(book, start, end)` fasst gleiche Einträge benachbarter Verse zu einem Bereich zusammen; lineare Suche)
  und `Lexicon` (RawLD, RawLD4, zLD; `lookup` mit `@LINK`, `keys(prefix, limit)`, `search` in Stichwort und Text).
  Klartext über `text::to_plain`, je Eintrag höchstens `MAX_ENTRY_CHARS` (20 000). `scan` füllt seit R15 auch
  `Scan::commentaries`/`lexicons`, `BibleInfo.kind` (`SourceKind`); `Scan::find` bleibt bei Bibeln. HREFCom,
  RawFiles, verschlüsselte Module und fremde Verszählungen: `supported: false` mit Grund. Testmodule:
  `fixture::Layout::{RawCom, ZCom}` und `fixture::{LexiconFixture, LexLayout, write_lexicon}` (Index, `.dat`,
  zLD-Blöcke mit ZIP), Tests `tests/reference.rs`.
- **Server `bibles.rs`:** Zwischenspeicher je Art (höchstens 8 offene Module), `commentary_by_name`,
  `lexicon_by_name` (`Source::Ready`/`Problem`), RPC `commentary.*`, `lexicon.*`, `bible.work_sources`/
  `set_work_sources`, `bible.modules` mit `kind`. HTML-Bausteine für die Literatur: `commentary_panel_html`
  (`<details class="commentaries" data-widget="commentaries">`, je Modul Name, Einträge, Lizenz; alles maskiert)
  und `commentary_hits_html` (Gruppen „Kommentar“ mit Links `/literatur/bibel?work=…&ref=…&kom=1<tail>`, gesamt
  3 s, 10 Treffer je Gruppe). Stil `static/commentaries.css`. MCP `mcp/bibles.rs`: `commentary_lookup`,
  `lexicon_lookup`, `lexicon_search` (nur lesend).
- **`bible_research.rs` (Daten des Faktenblocks):** `research(state, work, persona, text, max_chars)` – Schalter
  und Erlaubnis (`personas::bible_state`: @belege Vorgabe an, Lektor Vorgabe aus, sonst nie), Stellen nur aus
  `baruch_scripture::scan` (ungültige → Hinweis, kein Text), je Stelle Wortlaut der Werk-Bibel, dann je gewähltem
  Kommentar dessen Einträge; zuletzt Lexikon-Artikel zu Stichwörtern (seit Runde 17b: Strong-Nummern der Verse bei
  einer Bibel mit Auszeichnung, dann Strong-Nummern und großgeschriebene Wörter des Texts mit Lemma-Probe;
  höchstens 5 je Lexikon). Gesamtlänge ≤ `max_chars` (je Beleg ≤ 1500), höchstens 12
  Stellen; Lesen in `spawn_blocking`. `None`, wenn kein Block gehört (Schalter aus, nicht erlaubt, weder Bibel noch
  Kommentar noch Lexikon gewählt). Die Darstellung im Kontext ist Sache des Lektors (`assist/bible_facts.rs`).
- **Store (Migration 0037):** `work_sword_source (work_id ON DELETE CASCADE, kind commentary|lexicon, module,
  position)`, `persona_bible (persona_id, enabled)`; `src/sword_sources.rs`. Test `tests/migration_0037.rs`.
- **Seite:** Einstellungen → Werk, Block `#kommentare` (`settings/work.rs`, Formular `POST /einstellungen/werk/bibeln`,
  Handler `connections::pages::work_bibles_act`, weil die Router von `settings/` nicht zu diesem Zweig gehören).

### MCP-Tokens (Migration 0016)

- Tabelle `mcp_token (id, name, hash UNIQUE, prefix, created, last_used, revoked)`, Teilindex
  `mcp_token_active_name` (Name eindeutig unter gültigen). Store `src/mcp_tokens.rs`: `MCP_TOKEN_PREFIX =
  "baruch_"`, `McpToken` (ohne Hash/Klartext), `mcp_token_create` (32 Bytes `getrandom`, Base64url ohne Füllung,
  SHA-256 hex, `prefix` = erste 12 Zeichen), `mcp_token_list`, `mcp_token_revoke` (idempotent),
  `mcp_token_verify` (nur nicht widerrufene; `last_used` höchstens einmal je Minute).
- Server `src/mcp/tokens.rs` (RPC `mcp_token.*`, nur Mensch), `src/settings/access.rs` (Seite und
  `POST /einstellungen/mcp/act`; der frische Klartext geht über ein eigenes `tokio::task_local` nur in diese eine
  Antwort, `Cache-Control: no-store`), Vorlage `templates/settings/mcp.html`.

### Aufträge aus der Oberfläche, Fäden und MCP-Grenzen (Runde 15, „auftraege“)

- **Strom:** `assist/routes.rs` leitet die Ereignisse aus `Progress` weiter (`posted`, `retry`, `compressed`,
  Schnittstelle S2) und gibt `done.compressed` aus; `assist.ts` setzt den eigenen Beitrag sofort in den Chat,
  verwirft bei `retry` das Gezeigte der Persona und meldet `compressed` als `baruch:toast` (Empfänger `toast.ts` des
  Lektors). Abbrechen: im Chat sind Beitrag und fertige Antworten schon gespeichert (Neuzeichnen), sonst nichts.
- **Hinweisseite:** `assist::routes::notice_page[_with]` rendert `assist_notice.html` (Status 200) in
  `view::environment()` + eigener Vorlage; ist dort `fragments/toast.html` (Lektor) registriert, bindet die Seite
  ihn ein, sonst einen gleichwertigen Kasten. Genutzt von `/assistenten/act`, „an Persona geben“ und der Bibelseite.
  Rücksprünge nur auf eigene Pfade (`safe_back`).
- **Fäden ohne Abschnitt (Migration 0038):** Tabellen `chat_thread (id, work_id → work ON DELETE CASCADE, title,
  created_by, created, updated)` und `chat_thread_post (thread_id → chat_thread ON DELETE CASCADE, position UNIQUE je
  Faden, author, body, sources JSON, created)`; Store `src/threads.rs` (`thread_list/get/create/add_post`, je eine
  Transaktion, unbekannter oder fremder Faden → `Refused`). Server `src/threads.rs`: RPC `thread.*`, Seiten
  `/faeden`, `/faden`, `POST /faden/act`, und `threads::chat` – eigener, schlanker Kontext (Rollen-Prompt, Werk,
  letzte 12 Beiträge, Auftrag) mit eigenem Modellaufruf über `state.model` (verbotene Einleitung einmal neu,
  `hooks::mark_answer`, `CUT_NOTE`); `assist::run` bleibt abschnittsgebunden. Test `migration_0038.rs`.
- **Relay:** eine Stufe – nur Antworten auf die ursprüngliche Nachricht rufen weiter, jede Persona höchstens einmal,
  alle Antworten zusammen `MAX_CHAT_PERSONAS`. Im Faden in `threads::chat` (`relay: true` nur für MCP), im
  Abschnitts-Chat über MCP bis Runde 22 in `mcp/tools.rs` (`relay_section` nach `assist::chat`; seit Runde 23 ein Weg
  `assist::chat::relay_targets` für Oberfläche und MCP: nur Erwähnungen aus Antworten,
  die nicht selbst aus dem Relay von Runde 30c stammen – `ChatOutcome::relayed` –, gerahmt über
  `OrderFrame::relay` mit `relay_text` als Auftrag). Der Chat der Oberfläche (`persona.chat`) reicht nur bei
  direkter Anrede weiter (Runde 30c).
- **Warteschlange je MCP-Client:** `mcp/quota.rs` (über `#[path]` in `mcp/tools.rs` eingebunden, `mcp.rs`
  unverändert): Zähler je (Zustand des Servers, Client-Handle), `Slot` gibt beim Fallen frei; höchstens 2 laufende
  bzw. wartende `chat`/`ask_persona` je Client, sonst `isError` vor jeder Modellanfrage. Seit Runde 23 teilen sich
  alle Clients ohne Token zusätzlich vier Plätze (siehe „Runde 23 – Härtung“, mcp-namen).
- **Einfälle:** `ideas::idea_source` nennt die Herkunft („Einfall n von @x“) als Quelle des Beitrags; die Seite
  des Einfalls bietet die Persona-Auswahl (`persona` im Formular).
- **Abzeichen „neu“ (`chat.ts`):** Beiträge des Menschen tragen `data-post-own` (Vorlage vergleicht mit „Autor“ =
  `mirror::HUMAN_NAME`); gemerkt werden nur Zeiten des Servers bzw. „leer“, dazu je Werk der letzte Stand für den
  ersten Besuch eines Abschnitts.

### Integration Runde 15 (Integrator, 2026-11-08)

- Zusammengeführt `team/anbieter` → `team/lektor` → `team/auftraege` (`--no-ff`); die Schnittstellen S1/S2 lagen per
  cherry-pick in den anderen Zweigen, bei Konflikten galt die Fassung des Besitzers (§9), dazu die Verdrahtungszeilen
  aus §10 vereinigt (`lib.rs` STYLE, `tests/main.rs`, `web/src/main.ts`, `docs/RPC.md`).
- **`.toast` doppelt belegt:** `app.css` hat seit Runde 1 `.toast` für die kurzen roten Fehlermeldungen
  (`literature.ts`, `outline.ts`, `workspace.ts`, `ideas.ts`, `wysiwyg.ts`, fest unten in der Mitte). Der Hinweis des
  Lektors nutzt dieselbe Klasse; ohne Anpassung klebte er ohne JavaScript fest unten, und die Fehlermeldungen verloren
  ihr Rot. `static/toast.css` gilt darum nur für `.toast[data-widget="toast"]` (Test
  `round15::the_hint_toast_does_not_restyle_the_error_toasts_of_other_widgets`).
- **Quellen für [Sn]:** `Prepared.bible_labels` (Kurzangabe je Beleg) und `bible_facts::cited_sources`: jedes in der
  Antwort zitierte `[Sn]` wird Quelle des Beitrags („SWORD [Sn] <Art> <Modul>, <Stelle>“); ohne das blieb die Nummer
  für den Leser unauflösbar.
- **Kontext bei OpenAI-kompatibel:** `models::context` und `context_cached` geben für `kind = openai` nie eine
  gemeldete Länge zurück (Vertrag §3), auch wenn eine Eliezer-Verbindung derselben Adresse die Zwischenspeicher
  (je Adresse und Modell) gefüllt hat.
- **Prüfung MultiGPT:** die Grenze je Anfrage liegt 1 s über der gemeinsamen Grenze (`check::multigpt`); bei gleicher
  Länge lief unter Last die Grenze von `list_collections` zuerst ab, und die Prüfung meldete „steht“ (Test
  `round13_quellen::the_check_has_one_timeout_for_all_requests` schlug im vollen Lauf einmal fehl).
- MCP `initialize` nennt die neuen Werkzeuge (`commentary_lookup`, `lexicon_*`), `scope`, den Faden ohne Abschnitt und
  die Warteschlange je Client. `/faeden` und `/literatur/bibel` stehen in `round9_menu::all_pages`.
- Ende-zu-Ende `tests/round15.rs`: Faktenblock mit echtem `bible_research::research` (Bibel, Kommentar mit
  eingeschleuster „Anweisung“, Lexikon) für `@belege` und den Lektor mit Schalter; Kommentar-Panel und Gruppe
  „Kommentar“; „an Persona geben“ von der Bibelseite; komprimierter Kapitelbefund über das Formular ohne JavaScript →
  Hinweisseite mit dem echten Toast; Persona auf Ollama-Nachbau mit gemeldeter kleinerer Länge (komprimiert,
  nacheinander); externe Verbindung ohne Freigabe auch bei Zusammenfassungen ohne Anfrage; OpenAI ohne Länge; MCP
  `commentary_lookup` über den echten Eingang.

## Suche und Spiegel im Einzelnen

- **baruch-library:** `fts_query` macht NUL und Steuerzeichen zu Worttrennern; bleibt nichts übrig,
  `EmptyQuery`. FTS5-`snippet()` arbeitet mit den Marken U+E000/U+E001; `search.rs::snippet_html`
  maskiert den Text (`& < > " '`) und übersetzt die Marken ausgeglichen in `<mark>…</mark>`.
  Hervorhebungen stehen als `kind = 'highlight'` im Index (Trigger aus `0103_highlights.sql`, auch für
  Umbenennen und Löschen des Titels); die Abfrage verbindet sie mit `library_highlight`, lässt
  `rejected` weg und füllt `quote_id`/`highlight_id` im Treffer. `build.rs` baut bei neuen Migrationen neu.
- **Beiträge im Index (0105, `0105_post_index.sql`):** `post_fts_delete` entfernt die Indexzeile eines
  gelöschten Beitrags, `post_fts_move` trägt bei geändertem `post.node_id` Titel und Werk des neuen Abschnitts
  ein (Werke löschen, Gliederungspunkte zusammenführen); die Migration räumt verwaiste `post`-Zeilen auf.
  Die Abfrage lässt Werktreffer ohne lebenden Abschnitt ohnehin weg.
- **baruch-gitmirror:** Vor jedem Commit werden alle Zwischenordner der alten und neuen Pfade mit
  `symlink_metadata` geprüft, beim Schreiben werden fehlende Ordner einzeln angelegt (`walk_parents`).
  Ein Symlink ergibt `MirrorError::InvalidPath`, bevor im Arbeitsordner etwas geändert wird. Eine Datei,
  die selbst ein Symlink ist, wird durch eine echte Datei ersetzt, ihr Ziel bleibt unberührt. Ohne
  Änderung am Baum entsteht kein Commit, außer `CommitRequest::allow_empty` ist gesetzt (Protokoll-Commit
  mit demselben Baum wie sein Vorgänger).
- **Nachholen:** Scheitert ein Commit, holt der nächste Schnappschuss den vollen DB-Stand nach (Test
  `a_mirror_catches_up_after_an_error`).

## Ablauf einer Änderung

1. `revision.accept`, `revision.set_current`, `revision.save`, `revision.update` bzw. `revision.delete`
   nimmt die **Schreibsperre** (`AppState::write_lock`).
2. Die Änderung wird in einer Transaktion in SQLite geschrieben.
3. Noch unter der Sperre: Schnappschuss der aktuellen Revisionen, Commit im Spiegel mit Autor, Rumpf und
   Trailern dieser Änderung (`Persona`, `Model`, `Baruch-Node`, `Baruch-Revision`, `Baruch-Action`; siehe
   „Revisionsbaum“).
4. Sperre frei. Die Antwort trägt `commit` oder bei Spiegelfehler `mirror`.

Die Sperre deckt Schritt 2 und 3 ab: Liefen zwei Änderungen gleichzeitig, enthielte sonst der erste
Schnappschuss beide und der zweite Commit fiele aus. Verwerfen und Beiträge ändern den Spiegel nicht
und laufen ohne Sperre.

## Schutz der Schnittstellen

`/rpc`, `/act` und `/mcp` liegen hinter `guard::same_origin`: `Sec-Fetch-Site` muss `same-origin` oder
`none` sein, `Origin` (falls da) muss zu `Host` passen; sonst 403. `/rpc` und `/mcp` verlangen
`Content-Type: application/json` (sonst 415), damit ein fremder `fetch` einen Preflight bräuchte.
Lesende GET-Seiten, die teure Wege mit Zugangsdaten auslösen würden – die Bedeutungssuche in
`GET /literatur?…&lmode=rag` und `GET /einfaelle?…&mode=rag` –, prüfen dieselbe Regel (`guard::check`); bei
einer fremden Seite läuft keine Suche, die Seite nennt `literature::FOREIGN_PAGE`.

`/mcp` prüft danach die Gegenstelle: ohne Token nur Loopback (`127.0.0.1`, `::1`, `::ffff:127.0.0.1`),
sonst 401 mit `WWW-Authenticate: Bearer realm="baruch"`. Dafür startet `main.rs` mit
`into_make_service_with_connect_info::<SocketAddr>()`; fehlt `ConnectInfo`, gilt die Anfrage als fremd.
Vor allen Routen, auch den lesenden, steht `guard::local_host`: `Host` muss eine IP-Adresse (v4 oder `[v6]`,
mit oder ohne Port), `localhost` oder ein beim Start zusätzlich erlaubter Rechnername sein
(`AppState::allowed_hosts`, siehe „Host-Regel hinter einem Reverse-Proxy“ unten), sonst 421. Damit greift
DNS-Rebinding nicht (ein fremder Name, der auf 127.0.0.1 zeigt, schickt seinen Namen als `Host`). Ohne `Host`
(nur ohne Browser) geht es durch.
Mit `Authorization: Bearer <Token>` (Runde 10) prüft `mcp::endpoint` den Token über
`Store::mcp_token_verify` (nur SHA-256-Hash gespeichert): gültig → Zugang von jedem Rechner, Client-Name =
Token-Name; ungültig oder widerrufen → 401 mit `error="invalid_token"`, auch über Loopback.
`guard::remote_only_mcp` (im Router um alles, innerhalb von `local_host`) lässt von Gegenstellen außerhalb von
Loopback nur `/mcp` durch, sonst 403; ohne `ConnectInfo` (Tests) geht die Anfrage durch.
**Clickjacking:** `guard::no_framing` (äußerste Schicht, also auch auf 403/421) setzt auf jede Antwort
`X-Frame-Options: DENY` und `Content-Security-Policy: frame-ancestors 'none'` (an eine vorhandene CSP wie die
`sandbox`-CSP der Bilder angehängt). Sonst könnte eine fremde Seite den Zustimmungsdialog der Gliederung, die
Freigabe „extern“ oder die Verbindungen unsichtbar einrahmen; das eingerahmte Formular ginge same-origin ab, und
`same_origin` griffe nicht. Baruch rahmt sich selbst nirgends ein (Test `round15_nachbesserung`).

**Über-Seite (Runde 24).** `about.rs`: Seiten `/ueber` und `/ueber/lizenz`; Konstanten `VERSION`, `LICENSE_ID`,
`REPOSITORY`, `HOMEPAGE`, `LICENSE_TEXT`, dazu `THIRD_PARTY_PLACES`, `release_url()` und `find_third_party` (sucht
`THIRD-PARTY-LICENSES.txt` relativ zum laufenden Programm, liest sie nie). Kein Netzzugriff, keine Datenbank; Baruch
fragt nirgends nach neuen Versionen. Der Schlüsselbund (`connections/secrets.rs`) wird seit Runde 24 je Zielsystem
über Features der Crate `keyring` gewählt (`[target.'cfg(target_os = …)'.dependencies]` in
`crates/baruch-server/Cargo.toml`: Linux `sync-secret-service` + `crypto-rust`, macOS `apple-native`, Windows
`windows-native`); die Zweige für macOS und Windows sind übersetzt, aber nie gelaufen. Die Version hat eine Quelle
(`[workspace.package] version`); `tests/round24_abnahme.rs` hält die Stellen fest, die sie wiederholen
(`site/hugo.toml`, `CHANGELOG.md`, `README.md`).

**Host-Regel hinter einem Reverse-Proxy (Runde 24).** Zugriff unter einem Rechnernamen geht seit Runde 24 über
`guard::AllowedHosts` (`crates/baruch-server/src/guard.rs`), gefüllt von `main.rs` aus `--allow-host NAME`
(mehrfach, nur `serve`) und `BARUCH_ALLOWED_HOSTS` (Komma oder Leerraum) und vor `app(state)` in
`AppState::allowed_hosts` gesetzt; `AppState::open_with` lässt die Liste leer, alle Tests und `import`/`export`
laufen also wie vor Runde 24.

- `AllowedHosts::parse` nimmt nur ganze Namen: 1–253 Zeichen, Labels zu 1–63 Zeichen aus `a–z0–9-` (nicht am
  Rand `-`), klein gespeichert, ohne Port, ohne Platzhalter, keine IP-Adresse, nicht `localhost`, letztes Label
  keine Zahl (`127.1`, `0x7f.1` – Browser lesen das als IPv4). Ein ungültiger Name ist ein Fehler mit dem Namen
  in der Meldung; `main.rs` beendet `serve` dann mit Exit 2, bevor der Datenordner geöffnet wird.
- `AllowedHosts::allows(host)` = `allowed_host(host)` oder `is_extra(host)`; `is_extra` vergleicht den Namen
  ohne Port (Port wie bei `allowed_host`: 1–5 Ziffern) und ohne Groß/klein mit der Liste. `local_host` lässt
  genau das durch; alles andere bleibt 421 (anderer Name, Teil- oder Obername, Leerzeichen, Platzhalter).
- **Nie lokal:** `guard::via_proxy(hosts, headers)` ist wahr, wenn `Host` ein zusätzlicher Name ist – und, sobald
  überhaupt Namen erlaubt sind, auch wenn `Host` fehlt oder unlesbar ist (dann steht ein Proxy davor, und
  „lokal“ wäre nicht belegt). `mcp::endpoint` nimmt eine Anfrage ohne Token nur an, wenn die Gegenstelle
  Loopback ist **und** `via_proxy` falsch: Unter einem zusätzlichen Namen verlangt `/mcp` immer einen Token,
  obwohl die Gegenstelle (der Proxy auf demselben Rechner) Loopback ist. Ohne erlaubte Namen ist `via_proxy`
  immer falsch.
- **Origin-Prüfung unverändert:** `guard::check` vergleicht `Origin` (ohne Schema) wörtlich mit `Host`. Der
  Proxy muss `Host` also unverändert durchreichen (samt Port); sonst 403. Das Schema spielt keine Rolle, darum
  besteht `Origin: https://name` gegen `Host: name`, obwohl Baruch selbst nur HTTP spricht.
- **Bewusst nicht ausgewertet:** `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host`, `Forwarded`. Sie
  kämen von jedem, der Baruch direkt erreicht; eine Auswertung wäre nur mit einer Liste vertrauter Proxys sicher.
  Folge: `guard::remote_only_mcp` sieht bei jeder Anfrage über den Proxy Loopback und lässt alle Routen durch –
  der Zugangsschutz liegt ohne Konten ganz beim Proxy (ohne Konten hat Baruch keine Anmeldung, `docs/Betrieb.md`;
  mit Konten verlangt seit Runde 35 die Schicht `auth::gate` eine Sitzung, siehe „Runde 35 – Konten und Rollen“). Steht der Proxy auf
  einem anderen Rechner, ist die Gegenstelle nicht Loopback, und es bleibt bei „nur `/mcp`“.
- **Bekannte Grenze:** Schreibt der Proxy `Host` auf `127.0.0.1:<Port>` um, ist die Anfrage von einer lokalen
  nicht zu unterscheiden; `/mcp` ginge dann ohne Token. Baruch kann das nicht erkennen; `docs/Betrieb.md`
  verlangt darum eine Probe nach dem Einrichten.
- Die Seite „MCP-Zugang“ (`settings/access.rs`) zeigt unter einem zusätzlichen Namen die Adresse als
  `https://<Host>/mcp` (Annahme: TLS im Proxy; das Schema der Anfrage kennt Baruch nicht) und den Befehl mit
  `--header "Authorization: Bearer <Token>"`.
- Tests: Unit-Tests in `guard.rs` (`parse`, `allows`, `is_extra`, `via_proxy`), `tests/round24_betrieb.rs`
  (Router mit gesetzten Namen, Programm mit ungültiger Angabe).

Offen: kein eigenes TLS. Seit Runde 35: Anmeldung mit Konten (ohne Konten weiter keine); das Cookie der Sitzung
trägt hinter dem Proxy `Secure`, die übrigen Cookies (Darstellung, zuletzt geöffnetes Werk) weiter nicht.

## Literatur

Die Literatur ist ein Popup der Kopfzeile und hat eigene Seiten; die Arbeitsseite bleibt dreispaltig.

- **Daten** (`baruch-library`, Migration `0103_highlights.sql`): Tabelle `library_highlight` mit Titel
  (`ON DELETE CASCADE`), optional Zitat (`ON DELETE SET NULL`), Fundstelle (`locator`), Zeichenbereich
  `start_char`/`end_char` (beide NULL oder `0 ≤ start ≤ end`, Unicode-Zeichen; im Zitat prüft
  `add_highlight` außerdem, dass das Ende im Zitat liegt und der Text bis auf Leerraum der Ausschnitt ist –
  `normalize_space`: Folgen von Unicode-Leerraum werden ein Leerzeichen, Ränder entfallen –, und speichert
  dann den genauen Ausschnitt des Zitats), Text (nicht leer), Art
  (`quote`, `core`, `contradiction`, `check`), Notiz, Urheber (`human` oder `assistant` mit Modell) und
  Status (`active`, `proposed`, `rejected`). Eine CHECK-Bedingung lässt Personas nur `proposed`/`rejected`.
  `src/highlights.rs`: `add_highlight` (Mensch → `active`, Persona → immer `proposed`; ohne Fundstelle die
  Stelle des Zitats), `list_highlights`, `get_highlight`, `update_highlight`, `delete_highlight`,
  `cite_typst`. Fehler über den eigenen `HighlightError` (`Library`, `Unknown`, `Empty`, `InvalidRange`, `TextMismatch`),
  `LibraryError` bleibt unverändert.
- **Server** (`src/literature.rs`): eigene Template-Umgebung (`OnceLock`, Vorlagen `literature.html`,
  `source.html`, `fragments/library.html`, `fragments/highlights.html`); die der Arbeitsseite bleibt, wie sie
  ist. `pages()` liefert `GET /literatur` und `GET /quelle`, `writes()` `POST /literatur/act` (im Router der
  schreibenden Routen, also hinter `guard::same_origin`). `handles`/`call` bedienen `literature.render` und
  `highlight.*` aus `rpc::call` heraus. Adressen baut das Modul selbst (`query_escape`) und gibt sie als
  sichere Werte an die Vorlagen.
- **Abschnitt der Literatur-Seiten:** `/literatur` und `/quelle` tragen den festen Schlüssel selbst: aus
  `?key=` (geht vor `?node=`), sonst beim Laden aus der Stelle bestimmt. Er steht in den Links zur
  Dokumentansicht, in den versteckten Feldern aller Formulare (`place_fields`) und als `data-key` am Widget
  `source`.
- **Zitat übernehmen:** `highlight.cite {highlight, key}` übersetzt `key` unter der Schreibsperre in die
  Stelle, liest die aktuelle Fassung, hängt `cite_typst` an und legt über die unveränderte `Store::add_proposal` einen Vorschlag an (Persona
  `@literatur`, Modell `baruch-zitat`, Notiz „Zitat aus @schlüssel“). Die aktuelle Fassung bleibt; der Git-
  Spiegel ändert sich erst beim Übernehmen. `cite_typst` maskiert Backslash, Backtick und
  `# [ ] * _ $ < > @ ~ = - + /` sowie eine Aufzählung „1.“ am Zeilenanfang mit einem Backslash; eine reine Seitenzahl wird `S. …`.
- **Widgets** (`web/src/literature.ts`): `literature` (der `<dialog>`, Alt+L, Inhalt über
  `literature.render`, Suche in `lq` der Adresse, Werk-Treffer über einen versteckten `data-nav`-Link im
  Arbeitsbereich), `source` (Kontextmenü der Dokumentansicht; Zeichenbereich in Unicode-Zeichen wie in der
  Datenbank, Anfang und Ende aus DOM-Bereichen vom Zitatanfang, Text wörtlich über `Range.toString()` statt
  der zusammengefassten `selection.toString()`) und die Markierung der Fundstelle im Textfeld bei
  `#fundstelle`. `cite` über `/literatur/act` verlangt das Feld `key` (fehlt → 422 „Abschnitt fehlt.“, gibt
  es den Punkt nicht mehr → 409); das Kontextmenü zitiert nur mit `data-key`.
- **Volltext und Prüfung (Runde 13, `src/literature/fulltext.rs`, Migration `0106_source_remote.sql`):**
  `library_title_remote` verknüpft einen Titel mit einem Dokument (`way = 'multigpt'`, Sammlung, Dokument;
  `baruch_library::{RemoteRef, set_title_remote, title_remote, title_for_remote}`), `library_highlight_check` merkt
  sich je Hervorhebung das Ergebnis (`FulltextCheck`: `found`, `not_found`, `unverifiable`, mit Meldung und
  Zeitpunkt; `set_highlight_check`, `highlight_checks`, `highlight_check(_entries)`). `check_range(volltext, start,
  end, text)` vergleicht wie `check_excerpt` (Leerraum zusammengezogen, Unicode-Zeichen) und gibt nie
  `unverifiable`. `fulltext::read` (RPC `source.read`, MCP `source_read`, Seiten) liest über
  `MultigptWay::from_settings`; bei Ausfall die lokale Kopie (`local_copy` → `sync::local_document`; ohne Sammlung
  die einzige Kopie mit passender Dokument-ID) oder ein ehrlicher Fehler. `fulltext::adopt` (nur RPC/Formular) legt Titel und Verknüpfung
  unter der Schreibsperre an und ruft `rag::notify(state, None)`. `fulltext::check_highlight` prüft nach jedem
  `highlight.add`/Formular „Hervorheben“ ohne Zitat mit Bereich; die Hervorhebung ist vorher gespeichert.
  Vorlage `source_multigpt.html` (eigene Umgebung der Literatur); `/quelle` zeigt den Volltext im Abschnitt
  `#volltext`. GET-Seiten laden den Volltext nur, wenn `guard::check` die Anfrage als eigene erkennt.
- **An Persona geben (Runde 15):** `literature::to_persona` (Formular `action=persona`, RPC
  `literature.to_persona`, Bibelseite) baut aus Frage, markiertem Text und Quellenangabe eine Frage (höchstens
  `assist::MAX_QUESTION_CHARS`, sonst sichtbar gekürzt) und ruft `assist::run` mit `Mode::Ask`, `post_question`
  und der Quelle als `sources`; ohne `key` Hinweisseite „Abschnitt wählen“ (`assist::routes::notice_page`). Der
  Stub ist entfallen. Kontextmenü auch im Volltext (`literature.ts`: Bereich ab `data-offset` des Teils).
- **Hervorhebung übernehmen (Runde 15):** `literature::own_highlight` kopiert eine Persona-Hervorhebung über die
  unveränderte `baruch_library::add_highlight` mit Urheber Mensch (also `active`), Notiz „übernommen von @x
  (Hervorhebung n)“ – an ihr erkennt Baruch eine schon übernommene; das Original bleibt `proposed`. Keine Migration
  (Runde 15), kein MCP-Werkzeug.
- **Bibeln in der Literatur (Runde 15, `literature/bibles.rs`):** Kasten und Treffergruppen über die bestehende
  Server-API `bibles::{modules_value, by_name, lookup, search}` (unverändert) und die HTML-Bausteine
  `bibles::commentary_panel_html`/`commentary_hits_html` des Zweigs anbieter (im eigenen Zweig leer). Gelesen werden
  die Modulordner nur bei eingestellten Ordnern oder gewählter Bibel des Werks (`scan_allowed`), damit eine
  Seite ohne Einstellung – und kein Test – die Vorgabeordner des Rechners liest. Suche im gemeinsamen Feld: höchstens
  drei Bibeln, je 10 Treffer, zusammen 3 s (`tokio::time::timeout`). Seite `GET /literatur/bibel` und Formular
  `POST /literatur/bibel/act` (Router-Zeilen in `lib.rs`); das Zitat entsteht wie `highlight.cite` über
  `Store::add_proposal` unter der Schreibsperre (Autor `@literatur`, Modell `baruch-zitat`), der Text kommt aus dem
  Modul (`ref`), nie aus dem Formular. Hervorhebungen an Bibeltexten trägt die Bibliothek nicht (Hervorhebung =
  Titel + Zitat/Bereich); die Seite sagt das ehrlich. Vorlagen `literature_bible.html`,
  `fragments/literature_bibles.html` (auch in `view::environment`, `library.html` bindet sie mit `ignore missing`
  ein, weil `baruch-library` das Fragment allein rendert), Widget `bibles.ts`, Stil `static/literature_bibles.css`.

### Runde 16b – zeitwaechter

Sprechdauer je Gliederungspunkt und gesamt, Zielzeit je Werk, Anzeige in Gliederung und Editorkopf, RPC und MCP;
dazu lokal/extern im Akkordeon „Assistenten“.

- **Schnittstelle R16-S1 (erster Commit der Runde):** `baruch_typst::timing` (rein, keine Abhängigkeit):
  `spoken_words` (Zählregeln 1–8, siehe `docs/Bedienung.md` „Zeitwächter“; ein Zeichen-Scanner mit Zeilenanfangs-
  Regeln für Überschriften, `#set/#show/#let/#import/#include/#heading` und Listenzeichen; Klammergruppen werden
  mit Code- bzw. Markup-Modus übersprungen, gesprochene Inhaltsblöcke über einen Stapel offener `[`), `speaking_seconds`
  (ceil(words·60/wpm), wpm auf 60–250 geklemmt), `clock`, Konstanten `DEFAULT_WORDS_PER_MINUTE` 110,
  `MIN_/MAX_WORDS_PER_MINUTE` 60/250, `MAX_TARGET_MINUTES` 240. Store: Migration `0039_work_timing.sql`
  (`work_timing`, je Werk eine Zeile, CHECK-Grenzen, `ON DELETE CASCADE` – `Store::delete_work` kennt die Tabelle
  nicht), `baruch_store::TimingSettings` (Vorgabe 110/keine Zielzeit ohne Zeile), `Store::timing_settings` und
  `set_timing_settings` (Grenzen vor dem Schreiben → `StoreError::Refused`, nichts geändert; `BEGIN IMMEDIATE`).
  Server: `crate::timing` mit `TimingStatus`, `SectionTiming`, `WorkTiming` (`shown()` = Predigt oder Zielzeit),
  `timing_of` (rein: je Punkt die aktuelle Fassung; Teilbaum rückwärts aufsummiert, weil Nachfahren in
  Baumreihenfolge hinter dem Elternteil stehen), `work_timing`, `timing_json`.
- **Nur lesend, nicht gespiegelt:** `work.timing`, MCP `work_timing` und die Anzeige nehmen keine Schreibsperre
  und schreiben keinen Commit. Die Einstellungen stehen nur in SQLite (wie die Farben der Personas).
- **RPC** (`timing::handles/call`, eine Zeile im werkbezogenen `match` von `rpc::call`): `work.timing`,
  `work.timing_set` (nur Mensch; `/act` kennt die Methode nicht, MCP hat kein Werkzeug dafür), `timing.count`
  (höchstens 1 000 000 Zeichen) – damit zählt das Widget mit derselben Regel wie der Server.
- **Anzeige:** `view::extras` lädt für die Arbeitsseite die Zeiten (`timing::work_timing`) und die Freigaben der
  Personas (nur über `personas::call("persona.list")`: `locality`, `external_allowed`, `connection`) in
  `ViewExtras`; `rpc::library_view` hängt sie als Feld `extras` an `LibraryView` (die Zeichenfunktionen von
  `view` sind synchron). Abweichung vom Vertrag: `rpc.rs` bekam dafür neben der Dispatch-Zeile das Feld und einen
  Aufruf in `library_view` (die Datei gehört in Runde 16 ohnehin nur R16b). `attach_timing` hängt Dauer und
  Zeitmarke an die Einträge der Gliederung (Punkte mit Unterpunkten: Teilbaum), `timing_context` liefert den
  Kontext `timing` für Baum- und Editorkopf (`none`, wenn nicht `shown`). Die Abschnittszeit im Editorkopf ist die
  der gezeigten Fassung, `data-rest` = Gesamtzeit ohne die aktuelle Fassung des Abschnitts.
- **Widget** `web/src/timing.ts` (`data-widget="timing"`): hört auf `input` im Editor (`[data-widget=wysiwyg]`,
  Textfeld und Satzbild), entprellt 700 ms, schickt den Quelltext an `timing.count` und rechnet `rest + seconds`;
  Fehler leise. Keine zweite Zählregel in TypeScript. Seit Runde 17b (L6) holt es den Quelltext im Satzbild über
  das Ereignis `baruch:serialize-text` am Editor-Formular (`detail.wait`, Antwort von `wysiwyg.ts`, Vertrag §2.6);
  ohne Antwort zählt es im offenen Satzbild `innerText`, sonst das Textfeld.
- **Einstellungen → Werk:** Block „Predigtzeit“ (`id="zeit"`, `settings/werk.html`), Formular
  `POST /einstellungen/werk/zeit` (`settings::work::timing_act`, Route in `settings::writes()` hinter
  `same_origin`), 303 mit `ok=zeit#zeit` bzw. `zeitfehler=1#zeit`; `settings.get page=werk` nennt `timing`.
- **Personas lokal/extern:** `fragments/assistants.html` zeigt je Persona `locality`; extern ohne Freigabe
  (`PersonaAccess::blocked`) ist gekennzeichnet („Gesperrt“, Hinweis auf Einstellungen → Assistenten) und fällt
  aus `active_personas` (keine Wahl für Prüfen/Vorschlag/Chat). Die eigentliche Sperre beim Auftrag bleibt im
  Persona-Backend (Runde 15b).
- **Tests:** `baruch-typst/tests/timing.rs` (Pflicht-Beispiele, Regeln), `baruch-store/tests/timing.rs`,
  `baruch-store/tests/migration_0039.rs` (nachgebaute Datenbank bis 0038 → 0039 verlustfrei, CASCADE),
  `baruch-server/tests/round16_zeit.rs`; Werkzeuglisten in `mcp.rs` und `round4.rs` (`work_timing` direkt nach `outline`; am Ende hätte es die
  Folgeprüfungen in `round12_multigpt.rs`/`mcp_ideas.rs` gebrochen, `mcp_ideas.rs` bleibt darum unverändert).

### Runde 16a – predigt-vorlage

- **Werkvorlage Predigt** (`baruch-store/src/works.rs`): `Preset::Sermon` legt in einer Transaktion
  `numbering_scheme = 'sermon'` und die Punkte aus `SERMON_OUTLINE` an – „Predigttext“ (ohne Nummer), „Hinführung“,
  „Beobachtung am Text“, „Deutung“, „Zuspruch“, „Anwendung“ (ohne Nummer), „Ausblick“. Die Nummern berechnet
  `preset_numbers` mit demselben `Numbering::of_scheme(Scheme::Sermon)` wie die Gliederung, gezählt nur über
  die gezählten Punkte; `node.numbered` wird ausdrücklich gesetzt (nicht über den Trigger 0006). Kein neuer
  Typ: „ist Predigt“ heißt `work.kind == WorkKind::Sermon`.
- **Entscheidung zur Titelwahl:** Das Schema `sermon` macht aus dem ersten gezählten Punkt „Einleitung“, aus dem
  letzten „Schluss“ und aus den übrigen „I.“, „II.“, „III.“. Hießen die Punkte „Einleitung“ oder „Erster Hauptpunkt“,
  stünde die Nummer in der Gliederung doppelt („Einleitung Einleitung“, „I. Erster Hauptpunkt“). Darum tragen die
  gezählten Punkte Titel, die ihre Aufgabe nennen, ohne die Nummer zu wiederholen (Hinführung, Beobachtung am Text,
  Deutung, Zuspruch, Ausblick); Predigttext und Anwendung zählen nicht mit und heißen, was sie sind. So bleibt der
  letzte gezählte Punkt immer „Schluss“, auch wenn die Anwendung dahinter oder davor steht. Export → Import erhält
  Gliederung, Nummern und „Nummer aus“ (Test `round16_predigt`).
- **Predigttext aus SWORD** (`baruch-server/src/works.rs`, `sermon_text`): `text_ref` wird mit
  `baruch_scripture::parse` gelesen (genau eine gültige Stelle, sonst -32602 vor jedem Schreiben). Bibel:
  `module` → `bibles::by_name`, sonst `bibles::of_work(from_work)` bzw. Vorgabewerk – dieselbe Logik wie
  `bible.lookup`, keine Signatur geändert. Die Verse kommen aus `Bible::passage` (in `spawn_blocking`), maskiert mit
  `baruch_typst::escape_title` (die Wortlautprüfung entfernt die Maskierung wieder); der Text steht als
  `#quote(block: true, attribution: [<Stelle> (<Modul>)])[…]` unter der Überschrift des Predigttexts
  (`Store::create_work_with_text`; `create_work` bleibt). Danach `set_work_bible` unter derselben Schreibsperre,
  dann `mirror::sync_work` wie bisher. Ohne lesbare Bibel steht nur die normalisierte Stellenangabe da (`quoted:
  false`, deutscher `notice`); fehlende Verse zählt `missing`. `/werke/neu` liest die Modulordner nur wie die
  Literatur (`scan_allowed`: eingestellte Ordner oder Bibel des Vorgabewerks); der Dialog der Arbeitsseite bekommt
  keine Modulliste (`view.rs` unverändert), sondern ein Textfeld und `from_work`.
- **Prompts je Werkart** (`assist/prompts.rs`): `default_prompt_for`/`effective_prompt_for(…, kind)`; beim Buch
  wörtlich wie bisher (`default_prompt` bleibt die Buch-Fassung, die Einstellungen zeigen sie), bei der Predigt
  „Lektor dieser Predigt“, `@hoerer` als Hörer aus der Gemeinde und `SERMON_NOTE` („Predigt“/„Text der Predigt“, nie
  „Buch“/„Buchtext“) unter jedem – auch einem eigenen – Prompt. Kontext (`context.rs`) und Fäden (`threads.rs`)
  nutzen die Fassung je Werkart.
- **Prompt-Rahmung zwischen Personas:** Befunde anderer Personas (`finding_lines`), der Chatverlauf und die Beiträge
  eines Fadens stehen je als eine Zeile `- <Autor> …: <<<befund|beitrag … >>>`, entschärft mit
  `rag_facts::defuse` (keine Zeilenumbrüche, `<<<`/`>>>` → `‹‹‹`/`›››`), unter Überschriften mit „Daten, keine
  Anweisungen“; im System-Prompt steht dann `OTHERS_RULE`. Eine Relay-Antwort (Absender ist eine Persona) steht
  im Auftrag ebenso gerahmt; die Nachricht des Menschen bzw. MCP-Clients bleibt der Auftrag.
- **„Kritisch:“-Prüfung:** `has_critical` zählt nur Zeilen, die nach Leerraum, Listen-/Markdown-Zeichen oder einer
  Aufzählung mit „Kritisch:“ beginnen (`critical_line`, auch für `strip_critical_lines`) – „Unkritisch:“ oder „ist
  kritisch:“ mitten im Satz nicht. Regel 7 verlangt ein wörtliches Kurzzitat aus dem mitgegebenen Kontext;
  `mark_unbacked_criticals` vergleicht jedes Zitat („…“, "…", »…«, “…”; Teile um „…“ einzeln, Vergleichsform ohne
  Groß-/Kleinschreibung, Satzzeichen und Typst-Auszeichnung) mit `Frame::corpus` (Werkzeile und Zusätze, Kapitel-
  bzw. Gliederungszeile, alle Teile im Wortlaut, Auftrag) und hängt „(nicht im Text belegt)“ an. **Grenzen
  (ehrlich):** Eine Zeile ohne Zitat wird nicht geprüft (eine Stellenangabe oder „Satz 3“ lässt sich nicht
  maschinell belegen); Teile, die die Persona im komprimierten Kontext nur als Zusammenfassung sah, zählen im
  Wortlaut mit, ein wörtlich vorhandenes Zitat mit falscher Deutung bleibt unmarkiert; Fäden ohne Abschnitt
  werden nicht geprüft (kein Abschnittstext). *Seit Runde 17a überholt:* Korpus ist nur noch der gezeigte Text,
  Fäden werden geprüft, Stellenaussagen bekommen einen Vermerk (siehe „Runde 17a“).
- **Zusammenführen der Stücke:** `chunk::merge_pieces(answers, overlaps_previous)` führt nur Befunde zusammen, die
  aus dem direkt vorigen, überlappenden Stück stammen (`Layout::continued`, gesetzt von `overlapping` für das
  zweite und jedes weitere Teilstück eines Abschnitts); gleich formulierte Befunde zu verschiedenen Abschnitten oder
  innerhalb einer Antwort bleiben stehen. `merge` = alle Stücke überlappend (wie bisher für Teilstücke).
- **Wirksame Kontextlänge:** `personas::effective_lector_of` vergleicht bei „automatisch nach Kontext“ (und bei
  einer festen, inaktiven Wahl) `state.model.context(…).tokens`; eine feste, aktive Wahl fragt nichts ab.
  `rows_and_lector` nutzt sie (Signatur gleich), `lector_of` bleibt die Regel nach der Einstellung. Der Lektor im
  Kontext (`is_lector`, Gedächtnis) kommt seither aus `rows_and_lector` statt aus `Work::lector`. Fäden rechnen
  mit derselben wirksamen Länge. Test-Nachbau: `ScriptedModel::with_reported_context_for(model, n)`.
- **Schlanker Faden-Kontext:** Gliederung als Datenblock (`context::outline_block`, nur Nummern und Titel), Belege
  aus SWORD zu den Stellen der Nachricht für berechtigte Personas (`hooks::bible_research`, gleiche Schalter wie im
  Abschnitt); gekürzt wird in der Reihenfolge Belege, Gliederung, älteste Beiträge. Bedeutungs- und Websuche gibt
  es im Faden weiterhin nicht.
- **Chat nur Abschnitt:** `store_meta.chat_scope` (`chapter` | `section`, `Store::chat_scope`/`set_chat_scope`,
  keine Migration, nicht im Spiegel), RPC `persona.chat_scope_get/set` nur für den Menschen (nicht `/act`, nicht MCP),
  Formular auf Einstellungen → Assistenten. Im Chat mit `section` ist der einzige Teil der Abschnitt selbst, statt
  der Kapitelzeile steht `outline_block` da.

### Runde 16c – stichwortzettel

- **Satz (`baruch-render::notes`, rein):** `notes_source(&NotesDoc, NotesFormat, font_pt) -> String` baut aus
  fertigen Karten (`NotesCard`: Nummer, Titel, Tiefe, Zeitmarke, Dauer, Stichpunkte, Bibelstellen) eine Typst-Quelle;
  `compile_pdf` setzt sie unverändert (`compile`/`compile_pdf` sind nicht angefasst). Formate `A5` (A5 hoch, 16 pt,
  fließend) und `Cards` (A6 quer, 14 pt, `#pagebreak()` vor jedem Abschnitt, ein langer läuft über), Größe auf
  14–28 pt geklemmt. **Maskierung:** jeder Text steht nur als Typst-Zeichenkette (`typst_string`: `\` und `"`
  maskiert, Steuerzeichen als `\u{…}`) im Aufruf `#card((…))`; die Funktion `card` setzt Zeichenketten als Text –
  Markup in Nutzertexten wird nie gelesen. Jeder Titel und Stichpunkt steht zusätzlich als `metadata` mit Label
  `<baruch-note>`; `tests/notes.rs` fragt Typst per `query` und `assert`, ob ein Text mit `#`, `[`, `]`, `*`, `$`,
  `\`, `<`, `@` und `"` wörtlich ankommt (und prüft A5-/A6-Maße über die `viewBox` der SVG-Seiten).
- **Server (`src/pdf/notes.rs`, Untermodul von `pdf.rs`):** `points_of` (Regel marker → strong → first_sentence,
  höchstens 6 Punkte zu 160 Zeichen; Lesetext über einen kleinen Zeichen-Scanner: Kommentare, stumme Zeilen,
  Code-Argumente, `#footnote`/`#arbeit`, Labels, Verweise, Mathe, Raw fallen weg) und `refs_of`
  (`crate::scripture::refs`, nur `RefStatus::Valid`, `normalized`, ohne Doppel). `notes_of` lädt das Werk einmal und
  nimmt die Zeiten aus `crate::timing::timing_of` mit `Store::timing_settings` – dieselbe Rechnung wie
  `crate::timing::work_timing` (S1), keine eigene Zählung; der Test `the_clocks_of_the_notes_are_those_of_the_timer`
  vergleicht Karte für Karte mit `timing_json`. PDF über die vorhandene private Erzeugung `pdf::generate`
  (`PdfKind::Notes`, „Kein PDF (Stichwortzettel)…“): `pdf::claim` je Werk (409), `typeset::RUNS`, Zeitlimit 120 s
  (503), Satzfehler 422. Für Tests gibt es `NotesOptions::{limit, append}` (`#[doc(hidden)]`): Die Karten selbst
  können keinen Satzfehler auslösen; `append` hängt Typst an die erzeugte Quelle, damit der Weg 422 bzw. „Exit 1
  ohne Datei“ prüfbar ist; `notes_response` ist dafür öffentlich. Einbindung ohne `rpc.rs`/`lib.rs`:
  `pdf::handles`/`call` geben `notes.*` weiter, `pdf::pages()` trägt die beiden GET-Routen. Eigene
  Template-Umgebung (`base.html`, `notes.html`). CLI: `main.rs` liest nur `--stichwort/--format/--schrift`
  (Prüfung über `NotesOptions::parse`); `pdf::notes::write_notes_file` schreibt über `pdf::write_new_file`
  (`create_new`, halbe Datei wird entfernt; gemeinsam mit `write_pdf_file`).
- **Anträge (Nutzerentscheidung 2026-11-08):** `outline.request_decide` ruft `mirror::record_request_decision` für
  jede Entscheidung ohne `change` (abgelehnt, veraltet), nicht mehr nur bei `op.needs_consent()`; damit nehmen RPC,
  `/gliederung/act` (ohne JavaScript) und der Zustimmungsdialog (Formular bzw. `consent.ts` über RPC) denselben
  Weg. `mirror.rs`, `consent.rs` und `consent.ts` blieben dafür unverändert. Tests je Antragsart in
  `tests/round16_antraege.rs`; `tests/outline_mcp.rs` erwartet jetzt den Protokoll-Commit.
- **Vorschau „neuer Teil“:** `PreviewPoint` (Store) trägt zusätzlich `kind_word`, `before_number`, `before_level`
  (aus dem Stand vor dem Probelauf); `outline::add_preview` ergänzt `label`/`before_label` mit
  `view::level_name(work.kind, level)` (Predigt: „Predigtteil“) und `changed_label`. `outline.html` und
  `consent.html` zeigen `label` statt der bloßen Nummer und „(vorher … → nachher …)“.
- **Zeiten auf `/gliederung`:** `outline::manager` holt `crate::timing::work_timing` + `timing_json`; die Vorlage
  zeigt bei `timing.shown` (Predigt oder Zielzeit) die Spalte „Dauer“ (`sections[entry.id]`, Zeitmarke „ab …“) und
  oben Gesamt-/Zielzeit mit Warnung als Text. Nur lesend.

### Integration Runde 16

Zusammengeführt mit `--no-ff` in der Reihenfolge zeitwaechter → predigt-vorlage → stichwortzettel. Konflikte nur in
`tests/main.rs`, `docs/Architektur.md` (Abschnitte nebeneinander) und den S1-Dateien `baruch-typst/src/timing.rs`,
`baruch-server/src/timing.rs` (add/add: der Zweig stichwortzettel trug den cherry-pick des S1-Commits; maßgeblich ist
die berichtigte Fassung aus zeitwaechter, stichwortzettel hatte sie nicht verändert).

- **Ende-zu-Ende `tests/round16.rs`:** Predigt über das Formular „Neues Werk“ mit Predigttext aus einer Testbibel
  (`baruch_sword::fixture`), Text in allen Punkten, Formular „Predigtzeit“ → Zeiten und Warnung in `work.timing`, MCP
  `work_timing`, `timing.count`, Baum, Editorkopf und `/gliederung`; ohne Bibel nur die Stellenangabe; Stichwortzettel
  als Seite, `notes.get`, `notes.pdf` (A5 und Karten), Download und `write_notes_file`; jede Antragsart auf der
  Predigt ohne JavaScript abgelehnt → je ein Protokoll-Commit; Lektor der Predigt mit gerahmtem fremdem Befund,
  belegter und erfundener Kritik, „Unkritisch:“; Prüfen der ganzen Predigt mit „Kritisch:“ hinter der Teil-Angabe.
- **„Kritisch:“ hinter der Teil-Angabe** (`assist/prompts.rs`, gefunden in der Handprüfung): Der Auftrag „Prüfe das
  ganze Kapitel … jeden mit … dem Teil“ führt bei Gemma zu Zeilen „- I. Beobachtung am Text: Kritisch: …“. Seit 16a
  zählte nur „Kritisch:“ am Zeilenanfang – Baruch forderte deshalb einen vorhandenen Befund nach (eine zweite
  Modellanfrage) und schrieb fälschlich „nennt keinen mit „Kritisch:“ markierten Befund“ unter den Beitrag. Neu
  `critical_finding`: Zeilenanfang wie `critical_line` oder „Kritisch:“ direkt nach einer Angabe, die mit „:“, „–“
  oder „—“ endet (höchstens 80 Zeichen, ohne Satzende – Nummern wie „I.“ zählen nicht als Satzende –, Trenner nicht
  in offener Klammer; Nachbesserung siehe unten).
  `has_critical` und `mark_unbacked_criticals` nehmen `critical_finding`; `strip_critical_lines` (Vorschlag =
  Buchtext) bleibt bei der strengen `critical_line`, damit ein Satz wie „Die kritische Lage: Kritisch: …“ im
  Buchtext stehen bleibt. Die Zitatprüfung behandelt außer „…“/„...“ auch Einfügungen in `[…]` als Lücke
  („das Glas ... gesprungen [ist]“ ist belegt).
- **Predigt im Auftrag:** Kopf und Auftrag sagten auch bei einer Predigt „das ganze Kapitel“; `context.rs`
  (`sermon_wording`) schreibt dort „die ganze Predigt“ bzw. „zu dieser Predigt“.
- **Stichwortzettel:** Der Kopf jeder Karte ist `block(sticky: true)` – in der Sichtprüfung stand „III. Zuspruch“
  allein unten auf Seite 1 des A5-Zettels.
- **Stil:** `static/predigt.css` (in `STYLE`) für Editorkopf (Zeit in eigener Zeile, sonst wurde der Titel
  abgeschnitten), Baum- und Gliederungszeiten, „Predigtzeit“, Stichwortzettel-Seite (Karten im Raster), lokal/extern
  im Akkordeon „Assistenten“ und den Kasten „Nur bei einer Predigt“ in „Neues Werk“.
- **Handprüfung gegen Eliezer** (Integrator, Pförtner wie Runde 15, höchstens zwei Anfragen an
  `google/gemma-4-26b-a4b`, erfundene „Laternenpredigt“ mit Predigttext Lk 15,4–6 aus einer erfundenen Testbibel,
  598 gesprochene Wörter ≈ 5:30): Eine Anfrage „Prüfen, ganze Predigt“ (11,8 s, 1 928 Tokens Prompt, ganze Predigt im
  Wortlaut, System-Prompt „Lektor dieser Predigt“ mit Predigt-Hinweis) – die Antwort trug zwei „Kritisch:“-Befunde
  hinter der Teil-Angabe; Baruch erkannte sie nicht und forderte nach (zweite Anfrage, 3,1 s, wieder zwei Befunde,
  wieder nicht erkannt). Beide Fehler oben behoben und mit den echten Antwortformen getestet; eine dritte Anfrage gab
  es nicht. Alle Zitate in den „Kritisch:“-Befunden stehen im Text (in Zeilen ohne „Kritisch:“ setzt Gemma auch
  Umschreibungen in Anführungszeichen, etwa „gesprungenem Glas“ für „Das Glas ist gesprungen“); ein Befund ist inhaltlich falsch („Lk 15,5“ sei im Predigttext
  nicht belegt, obwohl der Vers in Lk 15,4–6 steht) – das kann die Zitatprüfung nicht sehen (→ Backlog).

### Nachbesserung Runde 16 (Gutachten)

- **Stichwortzettel, Fußnoten** (`pdf/notes.rs`): `strong_spans` übersprang `#footnote[…]` und `#arbeit[…]` nicht;
  eine Hervorhebung darin wurde Stichpunkt (Regel 2), die ersten Sätze fielen weg, ein Arbeitsauftrag konnte auf die
  Kanzelkarte geraten. Jetzt überspringt `strong_spans` stumme Aufrufe samt Argumenten und Inhaltsblöcken
  (`silent_call_end`, dieselbe Namensregel wie `plain`).
- **Stichwortzettel, Verschachtelung** (`pdf/notes.rs`): `plain_into` öffnete jeden Inhaltsblock `#name[…]` rekursiv
  ohne Grenze; 60 000 Ebenen (z. B. aus einer importierten `.typ`-Datei) brachten den Stack eines tokio-Workers zum
  Überlaufen, und der ganze Server brach ab (`GET /werke/{id}/stichwortzettel` liegt nicht hinter `same_origin`).
  Jetzt höchstens `MAX_NESTING` = 32 Ebenen, tiefer Verschachteltes fällt weg; damit ist auch `group_end` je Ebene
  höchstens 32-mal über den Text (linear statt quadratisch). `at_line_start` liest nur bis zum letzten
  Nicht-Leerraum zurück statt über die ganze Zeile. Tests: Einheitstest mit 256 KiB Stack, Server-Test `notes.get`
  und Seite mit 60 000 Ebenen.
- **„Kritisch:“ hinter der Teil-Angabe** (`assist/prompts.rs`): `critical_finding` erkennt nun auch eine fett
  gesetzte Angabe, deren `**` hinter dem Trenner schließt („- **III. Zuspruch:** Kritisch: …“), einen
  abgeschlossenen Zusatz in Klammern („III. Zuspruch (Schluss): Kritisch: …“; nur ein Trenner in *offener* Klammer
  zählt nicht) und eine römische Nummer als Aufzählung („II. Kritisch: …“, `strip_list_prefix` kennt „II.“/„II)“
  wie „2.“/„2)“ – das gilt damit auch für `critical_line` und `strip_critical_lines`).

### Runde 17a – verszaehlung

- **Alle Verszählungen** (`baruch-scripture/src/versification.rs`): `versification(name)` kennt jede Zählung von
  `sword-modules` 0.1.0 (Calvin, Catholic, Catholic2, DarbyFr, German, KJV, KJVA, LXX, Leningrad, Luther, MT, NRSV,
  NRSVA, Orthodox, Segond, Synodal, SynodalProt, Vulg; Groß-/Kleinschreibung egal, Schreibweise von sword-modules).
  German und KJV kommen weiter aus `data/versification*.tsv` (maßgeblich); die übrigen baut Baruch beim ersten
  Gebrauch aus den Daten von sword-modules (CC0, `default-features = false`: nur Daten, kein Lesen, kein Netz). Jede
  Zählung enthält nur die 66 Bücher von `books()` in deren Reihenfolge; Spätschriften bleiben draußen, MT und
  Leningrad haben kein Neues Testament (`chapters_of` → `None`). `all()`/`names()`: German zuerst, dann alphabetisch.
  `count_range` zählt die Verse einer Stelle (Vers 0 = ganzes Kapitel). Test `tests/versification.rs`: sword-modules
  „German“/„KJV“ stimmen Vers für Vers mit den Tabellen überein (sonst listet der Test die Abweichungen).
- **Abbildung** (`versification/mapping.rs`, Daten `data/versification-map.tsv`): Drehpunkt KJV. Je Zeile eine Regel
  „Zählungen, Buch, Kapitel, erster bis letzter Vers → KJV-Kapitel, erster KJV-Vers, Regelname“; `-` = der Vers hat
  in KJV keine eigene Entsprechung (z. B. eine Psalmüberschrift, die KJV nicht als Vers zählt, oder „3. Joh 15 ist in
  KJV Teil von Vers 14“), Kapitel `*` = ganzes Buch nicht abbildbar (Psalmen von LXX, Orthodox, Synodal,
  SynodalProt, Vulg: griechisch-lateinische Zählung). Der Rückweg ist dieselbe Regel umgekehrt. Ein Kapitel, das eine
  Regel berührt, ist in der Tabelle ganz beschrieben; ein Kapitel ohne Regel gilt als gleich, wenn beide Zählungen
  gleich viele Verse haben, sonst ist es nicht abbildbar (`Mapped::Unmapped`, „Versifikation passt nicht: …“ mit
  beiden Namen) – **nie geraten**. Das „gleich“ gilt nur in einem Buch, für das die Zählung Regeln hat oder dessen
  Verszahlen in jedem Kapitel mit KJV übereinstimmen; ein Buch ohne Regel, das von KJV abweicht, ist ganz nicht
  abbildbar („für Ps in Catholic gibt es keine geprüften Regeln …“, Nachbesserung Runde 17: Catholic, Catholic2,
  Segond und Calvin zählen Psalmüberschriften als Vers 1 ohne Regel in der Tabelle, Ps 13 hat dort wie in KJV 6 Verse
  und wurde vorher still falsch abgebildet). Zählungen mit Vers für Vers gleichen Zahlen (German/Luther, KJV/KJVA,
  NRSV/NRSVA) bilden jeden Vers auf sich ab, ebenso ein einzelnes Buch mit gleichen Zahlen und denselben Regeln in
  beiden Zählungen (z. B. Catholic/Catholic2 in den Psalmen). Regeln gibt es für die hebräische Zählung (German, Luther, MT, Leningrad: alle
  Psalmüberschriften, Ps 13 mit gleicher Verszahl und trotzdem verschoben, Kapitelgrenzen 1. Mo 31/32 … Mal 3/4) und
  im Neuen Testament (Apg 19,40/41, 2. Kor 13,12/13 für German/Luther; 3. Joh 14/15 und Offb 12,18/13,1 auch für
  NRSV/NRSVA). Röm 16 zählt in German und KJV gleich. `map_range` liefert die Verse der Zielzählung in
  Textreihenfolge ohne Doppel, höchstens `limit` (`truncated` nur, wenn die Stelle mehr als `limit` Verse hat); eine
  Stelle „ganzes Kapitel“ lässt Psalmüberschriften ohne Verszahl in der Zielzählung aus (Regel in `rules`), eine
  Versangabe darauf ist nicht abbildbar. Die Tabelle entstand mit einem Hilfsskript (nicht im Repo) aus den Verszahlen beider
  Zählungen; der Test `tests/mapping.rs` prüft dasselbe unabhängig (Quelle und Ziel existieren, kein Quell- und kein
  Zielvers doppelt = injektiv, berührte Kapitel ganz abgedeckt), dazu Hin- und Rückweg für jeden Vers German ↔ KJV.
  **Grenze:** Abweichungen ohne Regel (z. B. 2. Kön 15 in German mit 39 Versen, Calvin, Catholic, DarbyFr, Segond,
  Synodal) bleiben nicht abbildbar, bis jemand die Regel belegt einträgt; ein Kapitel gleicher Länge mit anders
  gesetzten Versgrenzen erkennt Baruch nur, wenn eine Regel es beschreibt (wie Ps 13) – in Büchern ohne jede Regel
  deshalb gar nicht (siehe oben). Test `tests/mapping.rs`: jedes Buch ohne Regel ist nur abbildbar, wenn es mit KJV
  übereinstimmt.
- **RPC/MCP** (`src/scripture_map.rs`, `src/mcp/scripture_map.rs`): `scripture.map {ref, from?, to?}` liest mit
  `baruch_scripture::parse` und prüft gegen `from` (`count_range`), nicht gegen German; `target` in deutscher
  Schreibweise mit Läufen. `scripture.versifications {}`. MCP `scripture_map` antwortet wie RPC, ein Fehler
  (unbekannte Zählung) ist `isError` mit der Liste. Nur lesend, ohne Werk.
- **Korpus der Zitatprüfung (L1)** (`assist/context.rs`): `shown_corpus(facts, calls)` = Faktenblock der
  Stellenprüfung (`Frame::facts`, steht im System-Prompt) + die Nutzer-Nachrichten der **gesendeten** Aufrufe, aus
  denen `strip_foreign` die gerahmten fremden Texte entfernt (`<<<befund`, `<<<beitrag`, `<<<einfall` bis zum
  nächsten `>>>`; der Inhalt ist entschärft, er kann den Kasten nicht früher schließen). So zählen: Werkzeile,
  Kapitel- bzw. Gliederungszeile, die Teile, wie sie dastanden (ein zusammengefasster Teil nur mit seiner
  Zusammenfassung – im gestückelten Lauf die Aufrufe aller Stücke), Prüfgegenstand, Auftrag (im Chat die eigene
  Nachricht), Belege aus SWORD, Treffer der Bedeutungssuche, Webseiten. Nicht zählen: Befunde anderer Personas,
  Chat- und Fadenbeiträge, Einfälle – und der übrige System-Prompt (seine Beispiele „Falsch: …“ sind keine Belege).
  `assist::run_inputs` nimmt das Korpus des Aufrufs bzw. aller Stücke (`Squeezed::corpus`).
- **Stellenaussagen (L2)** (`assist/ref_claims.rs`): `mark_ref_claims(text, corpus)` nach `mark_unbacked_criticals`
  im Lektor, im Chat (beide über `run_inputs`) und in Fäden. Eine „Kritisch:“-Zeile (`critical_finding`), die eine
  Stelle nennt (`baruch_scripture::scan`) und behauptet, sie fehle bzw. sei nicht belegt („nicht belegt“, „nicht im
  Text“, „kommt nicht vor“ …; „fehlt“, „fehlen“, „steht nicht“ nur zusammen mit einem Ort wie „im Text“, „im
  Predigttext“, „im Abschnitt“; alles als ganze Wörter), bekommt „ (Stelle steht im Text: <Stelle aus dem Korpus>)“,
  wenn die Stelle ganz in einer gültigen Stelle des Korpus liegt (Vers 0 = ganzes Kapitel); eine ungültige Stelle
  bekommt „ (Stelle ungültig: <message>)“. Die Vermerke setzt nur Baruch: `strip_notes` entfernt vorhandene (vom
  Modell geschriebene oder aus einem früheren Durchlauf) vor der Prüfung, die Zeile wird neu geprüft (idempotent,
  ein gefälschter Vermerk schaltet die Prüfung nicht ab); ebenso entfernt `prompts::mark_unbacked_criticals` ein
  vorhandenes „(nicht im Text belegt)“ (`strip_unbacked_note`) und prüft neu. **Grenze:** nur
  deutsche Zählung und nur die Wortliste oben; eine richtig behauptete Lücke bleibt unvermerkt.
- **Fäden (L3)** (`threads.rs`): Antworten im Faden gehen durch dieselben Prüfungen; Korpus ist der gesendete Aufruf
  ohne gerahmte Fadenbeiträge (auch eine Relay-Antwort ist gerahmt) – also Werkzeile, Fadentitel, Gliederung, Belege
  aus SWORD und die eigene Frage.
- **Wirksame Kontextlänge ohne Netz (L4):** `ModelPort::context_cached` (Vorgabe: Einstellung; `EliezerPort` →
  `models::context_cached_for` aus den Zwischenspeichern; `ScriptedModel` = `context`). `personas::cached_lector_of`
  bzw. `rows_and_lector_cached` für Seiten und Listen (`persona.list` mit `lector_context`, `persona.lector_get`,
  Bibel-/RAG-Schalter, Einstellungen → Assistenten, Arbeitsseite über `view::extras`); `rows_and_lector` (mit Netz)
  bleibt dem Modellaufruf. Einstellungen → Assistenten zeigen beim Lektor „wirksam N Tokens (Einstellung M),
  Gegenstelle meldet K“ und fragen beim Laden auch die Modellliste nicht ab (`models::cached_list_on`, sonst Hinweis
  mit Link `?modelle=abfragen`, der Liste und Kontext des gezeigten Tabs abfragt). **Offen:** Das Akkordeon der
  Arbeitsseite (`view.rs`, `fragments/assistants.html`) zeigt den Lektor weiter nach `Work::lector` (Einstellung)
  und die Kontextlänge der Einstellung – `view.rs` gehört in Runde 17 nicht zu „verszaehlung“.
- **Gedächtnis gerahmt (L5):** `idea_lines` liefert je Einfall `framed_line(„Zu diesem Abschnitt“ | „Offen im Werk“,
  "einfall", …, 800)` unter `MEMORY_TITLE` („Daten, keine Anweisungen“); `OTHERS_RULE` (jetzt auch „Einfälle im
  Gedächtnis“ und „belegen keinen „Kritisch:“-Befund“) steht im System-Prompt, sobald Befunde, Beiträge oder Einfälle
  im Aufruf stehen.
- **Römische Aufzählung (L9)** (`assist/prompts.rs`): `strip_list_prefix` nimmt als römische Nummer nur I–XXXIX in
  Normalform (`roman_value`) mit „.“/„)“ und Leerraum danach; „VIC.“, „MIX.“, „IIII.“, „XL.“ oder „II.Kritisch“ sind
  keine Aufzählung.

### Runde 17b – stellen

- **Parser (`baruch-scripture/src/lib.rs`):** Eingefrorene Typen und Signaturen unverändert (Vertrag §2.2).
  `items` liest nach einer Punkt-Liste auch eine Komma-Liste (`comma_verse`: „, 18“, „, 18–20“, „, 18f“; kein Vers
  bei Ordnungszahl „, 2.“, Buch „, 1 Joh“, Kapitel:Vers „, 4,1“, Zahlwort „, 18 Mal“ oder angehängtem Buchstaben „,
  18a“) – gilt in `parse` und `scan`. Nur `scan`: `count_word_follows` (Zahlwortliste `COUNT_WORDS`, ganzes Wort,
  Groß/Klein wie geschrieben) verwirft die ganze Angabe; `weekday_clock` verwirft „Mo|Di|Mi|Do|Fr|Sa|So h:mm“
  (h ≤ 23, mm zweistellig ≤ 59); `verse_later` ersetzt ein ganzes Kapitel (genau ein Eintrag, kein Kapitelbereich)
  durch Kapitel + Vers, wenn im selben Satz „Vers/Verse/Versen/V.“ mit Zahl folgt (≤ 120 Zeichen, kein Satzende –
  „.!?“ vor Leerraum außer nach Abkürzung oder Ordnungszahl vor kleinem Wort –, keine Leerzeile, keine andere
  Stelle); Bereich vom Buch bis zur Versnummer, Rest über `tail` („23–24“, „23f“).
- **Zitate (`wording::inline_quotes`, `server/src/quotes.rs`):** `quote_end` überspringt innere Paare anderer Art
  (`INNER_PAIRS`, nur im Absatz); ein Schlusszeichen nach Leerraum, das selbst ein Paar öffnet („er sagte “Ja”“),
  ist inneres Zitat. Eine Leerzeile beendet die Suche nicht mehr; ein weiteres öffnendes Zeichen derselben Art
  bricht ab, außer es steht nach einer Leerzeile am Absatzanfang (Fortsetzung). Höchstlänge weiter 3000 Zeichen.
  `quote_spans` = `inline_quotes` + Blockzitate der obersten Ebene (`baruch_wysiwyg::parse`) + `nested_block_quotes`
  (`#quote(… block: true …)[…]` überall, auch in `#szene[…]`; Zeichenketten in den Argumenten beachtet), sortiert,
  gleiche Inhaltsbereiche nur einmal. Grenzen: kein Zitat derselben Art im Zitat; die Zuordnung (`quote_for`,
  24 Bytes Zwischenraum) bleibt eine Faustregel. `compare` liest den Bibeltext jetzt als Klartext
  (`wording::plain_words`, ohne `strip_markup`): „#1“ im Verstext ist kein Typst-Code (gefunden mit dem Test der
  Sonderzeichen im Predigttext; vorher meldete die Prüfung beim eingefügten Predigttext „weicht ab“).
- **Lexikon-Stichwörter (`bible_research.rs`):** Stichwörter `Keyword::Strong`/`Keyword::Word` in der Reihenfolge
  Strong der Verse (`Bible::has_strongs` → `Bible::verse_strongs` je Vers der Stellen) → Strong im Text → Wörter.
  Strong-Nummern sucht `strong_key_forms` als „G25“, „G0025“, „G00025“, die bloße Zahl („00025“, „25“) nur bei
  `Feature=GreekDef`/`HebrewDef` passend zum Präfix (conf-Datei gelesen, sonst leer) – sonst bekäme ein
  hebräisches Lexikon den Artikel einer griechischen Nummer. Wörter über `lemma_candidates` (das Wort, dann Stämme
  ohne -ern/-en/-es/-er/-e/-n/-s mit mindestens drei Buchstaben, längere zuerst) gegen `Lexicon::lookup`
  (Groß/Klein egal). Seit Runde 19c über `baruch_sword::Lexicon::lookup_lemma` mit Umlaut-Rückführung („Bäume“ →
  „Baum“) und zuletzt dem letzten Bestandteil eines Kompositums. Ehrliche Grenzen: keine Lemmatisierung;
  Strong-Nummern nur aus Modulen mit Auszeichnung (OSIS `lemma="strong:…"`, ThML, GBF).
- **Predigttext (`works.rs`):** liest über `bibles::read_passage` (Abbildung deutsche Zählung → Zählung des Moduls,
  Vertrag §2.3); „gekürzt“ nur nach `PassageText.truncated` (genau 400 Verse: nicht gekürzt, 401: gekürzt).
- **Stichwortzettel (`pdf/notes.rs`, L7):** `first_sentence` überspringt den Punkt einer Abkürzung
  (`ABBREVIATIONS` ohne Groß/Klein, `LETTER_ABBREVIATIONS` „S.“/„V.“ genau, `PAIR_ABBREVIATIONS` „z. B.“ usw. mit
  oder ohne Leerraum); `plain_into` erkennt `@name` nach jedem Zeichen, das weder Buchstabe/Ziffer noch `_` ist.
- **Anträge (L8):** `baruch-store/src/requests.rs` liest die Werkart mit (`COLUMNS` mit Unterabfrage auf `work.kind`)
  und schreibt in Predigten „n vorhandene Predigtteile wandern“ (`summary_for`); keine Migration. Server
  `outline::preview_label`: bei Predigten Nummer wie im Schema („Predigtteil I.“), beim Buch ohne Schlusspunkt
  (unverändert).
- **Zeitwächter (L6):** siehe oben („Widget“); Server unverändert, Test der Zählregel in `tests/round17_stellen.rs`.
- Tests: `baruch-scripture/tests/references.rs` (Komma-Listen, Zahlwörter, Uhrzeiten, „Vers …“), Einheitentests in
  `quotes.rs`, `pdf/notes.rs`, `bible_research.rs`, `requests.rs`; Server `tests/round17_stellen.rs`.

### Runde 17c – pruefanzeige

- **Lesen in der Zählung des Moduls** (`bibles.rs`): `read_passage(bible, book, start, end) -> PassageText` bildet
  die deutsche Stelle mit `mapping::map_range(german(), bible.v11n, …, MAX_PASSAGE_VERSES)` ab (Besitz der Abbildung:
  R17a) und liest je abgebildetem Vers `bible.verse`. `PassageText {verses, module_refs, truncated, rules}`;
  `truncated` kommt aus der Abbildung (die Stelle hat mehr als 400 Verse; genau 400 = nicht gekürzt) statt aus
  `verses.len() == 400`. `check_wording` und `bible.lookup` lesen nur noch darüber; ein `Err` der Abbildung („Versifikation
  passt nicht: …“) wird `versification_mismatch` mit Grund und Modulname, ein Lesefehler `module_error`.
  `module_ref` („Mal 4,1–6 (KJV)“) nur, wenn sich Zählung und gelesene Verse von der deutschen Angabe unterscheiden
  oder eine Regel griff. Zitate kommen nur aus `crate::quotes::quote_spans` (S2); `bibles::block_quotes` ist entfallen.
  `WordingResult` trägt `quote_span` (Byte-Bereich des Zitatinhalts) und `module_ref`; JSON additiv.
- **Prüfanzeige:** `bible.marks {work?, key, rev?}` (`bibles::marks`/`marks_with`, nur lesend) prüft den Text der
  gezeigten Fassung (`shown_text`: `rev` oder aktuelle) mit `scripture::refs` und `check_wording` und liefert je
  Stelle `ref_span`/`quote_span` als Byte-Bereiche dieses Textes, `state` (`mark_state`) und `counts`. Seite
  `GET /werke/{id}/pruefung` in `src/scripture_check.rs` (eigene minijinja-Umgebung mit `base.html` und
  `scripture_check.html`, wie der Stichwortzettel), Link und Widget in `fragments/editor.html` (Statuszeile, außerhalb
  des Formulars). Widget `web/src/scripture_check.ts`: ohne ungespeicherte Änderung `bible.marks`, sonst
  `scripture.check` mit dem Quelltext aus `baruch:serialize-text` (§2.6, Hörer in `wysiwyg.ts`, Rückfall Textfeld);
  Markierung nur mit `CSS.highlights` (`bibel-stimmt`, `bibel-weicht`), keine DOM-Änderung – das Satzbild liefert
  unverändert dieselben `editsOf`. Die Zuordnung Zitat → Satzbild sucht den Zitattext in den Textknoten (Leerraum
  zusammengefasst); ein Byte-Bereich im Quelltext lässt sich ohne Quellpositionen im Satzbild nicht abbilden.
  Seit der Nachbesserung Runde 17 (`web/src/quote_find.ts`, `placeQuotes`, Test `tests/quote_find.rs` unter Node):
  Typst-Auszeichnung des Zitats entfernt (`#emph[…]`, `*`, `_`, `\`), nur ganze Wörter; gleicher Wortlaut an
  mehreren Stellen wird in der Reihenfolge von `quote_span` den Fundstellen zugeordnet (zuerst denen in
  Anführungszeichen, sonst allen); passt die Zahl nicht, bleibt das Zitat unmarkiert und die Fußzeile zählt es
  („nicht eindeutig gefunden“). Gleicher `quote_span` = dasselbe Zitat (eine Fundstelle).
- **Verschlüsselte Module** (`baruch-sword`): `BibleInfo.enciphered` (conf trägt `CipherKey`), `BibleInfo::locked()`
  (leer), `open_with_key`/`reference::open_commentary_with_key`/`open_lexicon_with_key` setzen den Schlüssel nur im
  Speicher in die conf (`ModuleConf::set("CipherKey", …)` wie `SwordLibrary::open_module_with_key`); ohne Schlüssel
  `BibleError::Locked`. `common_support` lehnt `CipherKey` nicht mehr ab (STUB entfernt). `Bible::probe` (und
  `Commentary::probe`, `Lexicon::probe`) liest über `probe_entries` die ersten nicht leeren Einträge (höchstens 64
  bzw. 2048 Zeichen): Entschlüsseln + Entpacken ohne Fehler und jeder Eintrag ohne U+FFFD und ohne Steuerzeichen
  außer Tabulator/Zeilenwechsel (Nachbesserung Runde 17: vorher nur der erste Eintrag und nur U+FFFD/NUL – bei
  Latin-1, der SWORD-Vorgabe ohne `Encoding`, entsteht nie U+FFFD, ein falscher Schlüssel ging bei RawText oft als
  gültig durch; Test `tests/cipher.rs` mit 200 falschen Schlüsseln). Server: Schlüssel nur über `AppState::secrets` (Konto `sword-cipher:<name klein>`,
  `bibles::stored_cipher_key`, nur für verschlüsselte Module abgefragt), `by_name`/`of_work`/Kommentare/Lexika holen
  ihn selbst; Zwischenspeicher-Schlüssel `(conf, Name, Änderungszeit, Prüfsumme des Schlüssels)` – nie der Klartext.
  `bible.cipher_key_set` probt vor dem Speichern (falsch → -32000 ohne Inhalt des Fehlers, er könnte Bruchstücke
  falsch entschlüsselter Daten tragen); beide Methoden in `bibles::WRITES` (nicht `/act`: Allowlist in `lib.rs`;
  kein MCP-Werkzeug). Formulare über `POST /einstellungen/quellen/act` (`connections/pages.rs`).
- **Test-Module:** `fixture::Fixture::compression(Zip|Bzip2|Xz)` (bzip2 0.6, lzma-rs 0.3 nur zum Schreiben),
  `Fixture::latin1()` (Text in Latin-1, conf ohne `Encoding`) und `Fixture::enciphered(key)`: Sapphire II nach der
  gemeinfreien Beschreibung (`sword-modules` `docs/spec/cipher.md`) **selbst** umgesetzt, nur Verschlüsseln; Blöcke
  nach dem Packen bzw. Einträge (RawText) je für sich, conf `CipherKey=` leer. Gelesen wird über `sword-modules`.
  Dass beide übereinstimmen, belegt allein nichts gegen echtes SWORD (beide folgen derselben Beschreibung); deshalb
  seit der Nachbesserung Runde 17 der Abgleich mit **libsword** (`tests/libsword.rs`): zwei Module, mit `osis2mod`
  aus libsword 1.9.0 aus einer frei erfundenen OSIS-Datei und einem erfundenen Schlüssel erzeugt (BZIP2 und XZ,
  Kapitelblöcke, verschlüsselt; `tests/data/libsword/`, je Datei xz-gepackt), liest Baruch Vers für Vers richtig, ein
  falscher Schlüssel fällt beim Probelesen auf. Umgekehrt liest `diatheke` (libsword) die von der Fixture
  verschlüsselten Module (zText mit BZIP2/XZ, RawText), wenn es installiert ist (sonst übersprungen, etwa in der
  CI). RawText verschlüsselt `osis2mod` trotz `-c` nicht (Klartext in der Datei); dafür gibt es nur die Gegenrichtung.
  Kein fremdes Modul, kein geratener Schlüssel.
- **Obergrenze beim Entpacken** (Nachbesserung Runde 17): `sword-modules` 0.1.0 (CC0) liegt unter
  `vendor/sword-modules` (nur `src`, `Cargo.toml` ohne Beispiele/Tests, `LICENSE`, `README.md`; per
  `[patch.crates-io]` im Workspace, `exclude = ["vendor"]`) mit einer einzigen Änderung in `src/compress.rs`: Ein
  Block entpackt höchstens `MAX_BLOCK_BYTES` = 64 MiB (ZIP und BZIP2 über `take`, XZ über einen begrenzten
  Schreiber), sonst `Error::Decompress("… more than 64 MiB")`. Ohne Grenze konnte ein manipuliertes Modul aus wenigen
  Byte Gigabytes entpacken (Dekompressionsbombe) und Baruch per OOM beenden – schon beim Nachschlagen, in der
  Prüfanzeige oder über `bible_search`. LZSS wächst höchstens um das 9-Fache der gepackten Größe und bleibt
  unbegrenzt. Test `tests/compression.rs` (65 MiB Nullen, die Größe im Index lügt). Bei einem Update von
  sword-modules die Änderung übertragen oder upstream vorschlagen.
- **Strong-Nummern** (`Bible::verse_strongs`, `text::strongs`): OSIS `lemma="strong:…"`, ThML
  `<sync type="Strongs" value>`, GBF `<WH…>`/`<WG…>`, unabhängig von `SourceType`; ohne Buchstaben H im AT, G im NT.
  `has_strongs`: `GlobalOptionFilter` mit „Strongs“ oder `Feature=StrongsNumbers`. Nutzer: Lexikon-Stichwörter in
  `bible_research.rs` (R17b).
- **Lizenz und Kommentare:** `info_json` additiv `cipher`, `notice`; `bible.modules` je Modul `key_stored`;
  Bibelseite „Lizenz (laut conf)“ und `module_ref`; Kommentar-Panel je Modul ein `<details class="commentary">`
  (Widget `web/src/commentaries.ts`: alle auf/zu, Zustand in localStorage nur in try/catch), Gruppenüberschriften
  „Kommentar · Modul“ wie „Bibel · Modul“ (`.hit-group-title`). Stil `static/pruefung.css`.
- **Offen:** Die Verse der Bibelseite tragen bei einem Modul in anderer Zählung dessen Nummern (Klick setzt sie
  als deutsche Stelle ins Feld „Stelle“); die Rückabbildung je Vers fehlt.

### Integration Runde 17

- **Zusammenführen:** `team/verszaehlung` → `team/stellen` → `team/pruefanzeige` mit `--no-ff`. Die
  Schnittstellen-Commits lagen per Cherry-pick in mehreren Zweigen; Git fügte dabei `all()`/`names()` in
  `versification.rs` doppelt ein (Konflikt „hinzufügen/hinzufügen“ in `mapping.rs`, sauberer Auto-Merge mit
  doppeltem Inhalt in `versification.rs`, `bibles.rs`, `wysiwyg.ts`, `baruch-sword/*`). Gelöst nach Besitz: die
  Fassung des Besitzers gilt (verszaehlung für `baruch-scripture/src/versification*`, stellen für `quotes.rs`,
  pruefanzeige für `baruch-sword`, `bibles.rs`, `wysiwyg.ts`), weil kein anderer Zweig dort über die Schnittstelle
  hinaus geändert hatte (geprüft mit `git diff <S-Commit> team/<id> -- <Datei>`).
- **Bücher mit Nummer 4/5** (`baruch-scripture/src/lib.rs`): `book_token` und `scan` begannen eine Stelle nur bei
  „1“–„3“; „4. Mose“/„5. Mose“ waren weder in `parse` noch im Fließtext erreichbar (seit Runde 10). Aufgefallen,
  weil der Test „nicht abbildbarer Vers“ nach dem Zusammenführen auf 4. Mo 25,19 (= KJV Num 26,1) stieß und
  `scripture.check` keine Stelle fand. Jetzt „1“–„5“; Test `the_fourth_and_fifth_book_of_moses_are_found_in_running_text`.
- **Zitat zur Stelle über eine Leerzeile** (`wording::quote_for`): Hat eine Stelle ein Zitat davor und eines
  danach, und liegt nur zwischen dem Zitat davor und der Stelle eine Leerzeile (`paragraph_break`), gilt das Zitat
  danach. Vorher bekam „„…“ (Joh 3,16)⏎⏎Psalm 139, besonders Vers 23: „…““ das Joh-Zitat noch einmal (Ergebnis
  „weicht ab“). Gefunden im Ende-zu-Ende-Test.
- **Akkordeon der Arbeitsseite** (`view.rs`, offener Punkt aus R17a): `ViewExtras` trägt `lector` (aus
  `persona.list`, also `personas::cached_lector_of`, ohne Netz) und je Persona `context_tokens` (`context.tokens`);
  `values` nimmt beides vor `Work::lector`/`context_length`.
- **`ref_claims::claims_missing`** zieht `prompts::UNBACKED_NOTE` vor der Wortsuche ab: Sonst zählte „(nicht im
  Text belegt)“ (Baruch hängt es an, Gemma ahmte es in der Handprüfung mitten in der Zeile nach) als Behauptung
  „nicht im Text“, und eine Zeile mit einer Stelle aus dem Korpus bekam fälschlich „(Stelle steht im Text: …)“.
- **Liste „Zitate prüfen“** (`static/pruefung.css`): Bezugsrahmen ist `.editor-status` (über `:has`), die Liste
  steht links und rechts bündig (vorher `right: 0` am Knopf, links abgeschnitten – Sichtprüfung).
- **MCP-Hinweis beim Verbinden** (`mcp.rs`, `initialize`): nennt `scripture_map` und `quote_span`/`module_ref`.
- **Ende-zu-Ende** `tests/round17.rs`: BZIP2-Bibel in KJV-Zählung → Predigt „Mal 3,19–24“ liest Mal 4,1–6 (KJV),
  `scripture.map`, Parser-Fälle, `bible.marks` mit Byte-Bereichen, Prüfseite und Editor-Link ohne JavaScript;
  XZ-verschlüsselte Bibel mit `MemorySecrets` (falscher Schlüssel über Formular und RPC, richtiger probegelesen, nie
  angezeigt); Lektor auf einer Predigt (Lk 15,5 kenntlich, ungültige Stelle, fremder Befund unbelegt, Einfall
  gerahmt); Akkordeon; Stil der Liste.
- **Handprüfung gegen Eliezer** (Pförtner wie Runde 15/16, zwei Anfragen an `google/gemma-4-26b-a4b`, Baruch mit
  `--secrets memory`, erfundene „Möwenpredigt“ mit Predigttext Lk 15,4–6 aus einer erfundenen BZIP2-Bibel in
  KJV-Zählung, ≈ 3:00): „Prüfen, ganze Predigt“ (10,5 s, 5 306 Zeichen) – vier „Kritisch:“-Befunde, alle hinter der
  Teil-Angabe erkannt, alle Zitate im Text; der Befund zur Anwendung („denn ich habe sie wieder“ weicht von Lk 15,6
  ab) deckt sich mit der Wortlautprüfung. Eine Zeile trug „(nicht im Text belegt)“ vom Modell selbst und bekam
  deshalb „(Stelle steht im Text: Joh 3,16)“ – behoben (siehe oben). Frage „Sind Lk 15,5 und Lk 15,6 gedeckt?“
  (1,8 s): Gemma nannte Lk 15,5 diesmal richtig „gedeckt“; die Behauptung „Lk 15,5 nicht belegt“ trat nicht wieder
  auf und ist nur offline (Nachbau) geprüft.


## Austauschformat (Runde 26)

Crate `baruch-exchange` (Modell, Schreiben, Lesen und Prüfen, ohne Datenbank), Store-Module `uuids`,
`exchange_dump` und `exchange_apply`, Server-Module `exchange::{export, import}` und `instance`. Migration
`0041_uuids.sql` (Kennungen, Grabsteine, Instanzkennung) und `0042_uuid_tombstone_cascade.sql` (Grabsteine auch
bei der Kaskade beim Löschen eines Werks); die reservierten Nummern 0043 (import) und 0044 (export) blieben ungenutzt.

### Kennungen (ids)

**Migration 0041 (`0041_uuids.sql`)** – kein Tabellenumbau, darum nicht in `FOREIGN_KEYS_OFF`:

- Zehn Tabellen bekommen `uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)))`
  (per `ALTER TABLE … ADD COLUMN`, als letzte Spalte): `work`, `node`, `revision`, `work_template_revision`,
  `post`, `idea`, `chat_thread`, `chat_thread_post`, `asset`, `asset_revision`. Vorhandene Zeilen bekommen eine
  zufällige v4 (SQL-Ausdruck aus `randomblob`, Version 4, Variante 10), dazu je Tabelle
  `CREATE UNIQUE INDEX <T>_uuid`.
- **Fülltrigger** `<T>_uuid_fill AFTER INSERT … WHEN NEW.uuid IS NULL` setzt die v4. Darum nennt kein
  Schreibweg des Stores die Spalte; eine mitgegebene uuid bleibt (so legt `exchange_apply` an).
- **Grabstein-Trigger** `<T>_uuid_tombstone AFTER DELETE … WHEN OLD.uuid IS NOT NULL` schreibt
  `INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid)`; die Werk-uuid kommt über den Weg zum Werk
  (Fassung → Punkt → Werk, Fadenbeitrag → Faden → Werk, Bildfassung → Bild → Werk; beim Werk die eigene).
- `uuid_tombstone (uuid PRIMARY KEY, kind CHECK (…10 Arten…), work_uuid, deleted)` mit Index
  `uuid_tombstone_work`; `store_meta.instance_uuid` (v4, `INSERT OR IGNORE`).
- Keine Sperre gegen das Ändern der uuid: Regel im Code, nur `exchange_apply` setzt eine uuid, und nur auf einer
  Zeile, die es in derselben Transaktion angelegt hat. Der Test `no_code_path_but_exchange_apply_writes_the_uuid_column`
  (`crates/baruch-store/tests/uuids.rs`) prüft den Quelltext aller Crates auf `SET uuid` und `INSERT … uuid`.

**Migration 0042 (`0042_uuid_tombstone_cascade.sql`)** – Nachbesserung: `chat_thread` und `chat_thread_post`
gehen beim Löschen eines Werks per `ON DELETE CASCADE`. In einer Kaskade ist die Elternzeile schon fort, wenn der
Grabstein-Trigger des Kinds läuft; die Werk-uuid wäre NULL (Test
`without_0042_threads_deleted_by_cascade_would_forget_their_work`). Zwei Trigger löschen darum die Kinder vorher
selbst: `work_cascade_uuid_tombstone BEFORE DELETE ON work` (Fäden des Werks) und
`chat_thread_cascade_uuid_tombstone BEFORE DELETE ON chat_thread` (Beiträge des Fadens). Einfälle hängen mit
`ON DELETE SET NULL` am Werk: Sie bleiben, ohne Grabstein.

**Store** (`crates/baruch-store/src/uuids.rs`): `Instance {uuid, name}` mit `label()` („ohne Namen (xxxxxxxx)“),
`new_uuid()` (v4 über `getrandom`), `Store::instance`, `set_instance_name` (trim, ≤ 80 Zeichen, keine
Steuerzeichen, leer löscht `store_meta.instance_name`; Fehler `StoreError::Exchange`), `work_uuid`,
`work_by_uuid`, `node_key_by_uuid`, `node_uuid`. `WorkMeta.uuid` (`works.rs`) trägt die Werk-uuid zu `work.meta`
und „Werk ändern“. Server: `instance.rs` (RPC `instance.*`), Formular in `settings/data.rs`
(`POST /einstellungen/daten/instanz`, hinter `guard::same_origin`).

**Regel für spätere Tabellenumbauten:** Baut eine Migration eine der zehn Tabellen nach dem SQLite-Verfahren neu
auf (neue Tabelle, Daten kopieren, `DROP`, `RENAME`, siehe 0040), muss sie die Spalte `uuid` samt CHECK und Werten
mitkopieren und danach `<T>_uuid`, `<T>_uuid_fill` und `<T>_uuid_tombstone` neu anlegen – bei `work` und
`chat_thread` zusätzlich `work_cascade_uuid_tombstone` bzw. `chat_thread_cascade_uuid_tombstone` aus 0042. Ein
`DROP TABLE` mit ausgeschalteten Fremdschlüsseln löscht keine Zeile einzeln und hinterlässt darum keine
Grabsteine; die Trigger verschwinden aber mit der Tabelle. Der Migrationstest vergleicht dafür die Objekte aus
0041/0042 (`new_objects()` in `crates/baruch-store/tests/migration_0041.rs`).


### Ausgeben (export)


- **Dump** `baruch_store::Store::exchange_dump(work)` (`crates/baruch-store/src/exchange_dump.rs`): eine lesende
  Transaktion (WAL-Schnappschuss), Tabellen → `baruch_exchange::ExchangeWork`. Verweise werden auf UUIDs
  übersetzt: `revision.based_on`/`asset_revision.based_on` (Nummern) → UUID der Fassung, `post_ref.node_seq`/`rev`
  → UUID von Punkt und Fassung (ohne passende Fassung ohne `fassung`), `idea.node_id` → Punkt-UUID,
  `work_book.cover_asset_id`/`author_photo_asset_id` → Bild-UUID, `done_ref` „key N…“ → Punkt-UUID. Positionen
  von Beiträgen werden zur Reihenfolge ab 1. Bilder als Rohbytes (Base64 der Datenbank entschlüsselt),
  `sha256` neu gerechnet. Grabsteine: `uuid_tombstone` mit der Werk-UUID. Baum der Gliederung über `parent_seq`
  und `position`.
- **Schreiben** `baruch_exchange::write` (`crates/baruch-exchange/src/write.rs`): eigener kleiner Schreiber
  (String, Einrückung zwei Leerzeichen, kein quick-xml-Writer), damit Maskierung und Base64-Zweig genau dem
  Vertrag folgen: `write_xml(doc, Inline|Files)`, `write_manifest`, `write_baruch` (zip 2.4, `mimetype` STORED
  zuerst, Raster STORED, `werk.xml`/SVG/Manifest DEFLATE, fester Zeitstempel 1980-01-01 und Rechte 0644 →
  deterministisch). Der Schreiber prüft UUIDs, Zeiten, Bildtypen und `sha256 == sha256_hex(data)` und gibt
  sonst `ExchangeError::Write` mit dem Feldnamen zurück, statt eine Datei zu schreiben, die das XSD ablehnt.
- **XSD** `docs/schema/baruch-werk-1.xsd` (Unterversion 1, seit Runde 25 eingefroren) und
  `docs/schema/baruch-werk-1-u2.xsd` (Unterversion 2 mit `<karte>`, gegen das seit Runde 25 jede Ausgabe geprüft
  wird); Prüfung mit `xmllint` in `crates/baruch-exchange/tests/xsd.rs` (dort auch die Rust-Prüfung der
  Pflichtelemente über quick-xml-Ereignisse, falls xmllint fehlt) und `crates/baruch-server/tests/round26_export.rs`.
- **Server** `src/exchange/export.rs`: `document` (Dump + Instanz + Zeitpunkt UTC + `baruch <Version>`),
  `export_file(state, work, Format)` → `ExportFile` (gemeinsam für Downloads, RPC, CLI und MCP), Seite
  `templates/exchange.html` (eigene minijinja-Umgebung), Downloads mit `Content-Disposition: attachment` und
  `Cache-Control: no-store`, `cli_export` mit `create_new` (nie überschreiben). MCP `src/mcp/exchange.rs`
  (`exchange_export`, Grenze `MCP_EXPORT_LIMIT` = 4 MiB Datei, weil die Antwort den Inhalt zweimal trägt).
- Migration: export braucht keine; die reservierte Nummer 0044 bleibt ungenutzt.

### Einlesen (import)


- **Lesen** (`baruch-exchange/src/read.rs`, Untermodule `read/dom.rs` – sicherer XML-Baum mit quick-xml 0.38
  (`NsReader`, `trim_text(false)`, `Event::DocType`/`PI` → Unsafe, `GeneralRef` nur vordefiniert oder
  Zeichenreferenz, Grenzen), `read/map.rs` – Baum → Modell mit „benutzt“-Markierung je Kind und Attribut
  (Unbenutztes → `unknown`), `read/archive.rs` – ZIP (zip 2.4: `by_index_raw` für Name, Verknüpfung,
  Verschlüsselung, Kompression; Zentralverzeichnis selbst gezählt, weil zip doppelte Namen still zusammenfasst;
  Entpacken mit `take(Grenze + 1)`; Manifest), `read/check.rs` – `validate`). `read`/`read_xml`/`read_baruch`
  geben nur geprüfte Dokumente zurück.
- **Planen und Schreiben** (`baruch-store/src/exchange_apply.rs`): `Store::exchange_preview` und
  `Store::exchange_apply` laufen durch dieselbe Funktion `execute`; die Vorschau rollt ihre Transaktion zurück.
  Neu: Zeilen mit allen Kennungen aus der Datei (`ensure_free` prüft vorher, dass keine Kennung schon einem
  anderen Werk gehört). Abgleichen: (a) neue Punkte über `outline::insert_bare` (Gliederung wie beim Anlegen,
  aber ohne Fassung 1; meldet `WouldAdopt` für einen neuen Teil vor Kapiteln bzw. `Misplaced`), danach setzt das
  Modul die uuid der eben angelegten Zeile und legt die Fassungen der Datei an; (c)/(d) Fassungen mit
  `last_rev + 1` und `derive_uuid`; (b) Anträge über `requests::outline_request_in` (Probelauf im Sicherungspunkt,
  Doppel → Hinweis) und `requests::add_request_in` (Teil mit Zustimmung); (e)–(g) Vorlage, Beiträge, Einfälle,
  Fäden, Bilder (`assets::prepare` = Prüfung wie beim Hochladen, `free_name`, `path_owner`), Angaben. Am Ende
  `load::work`. Herausgezogene Helfer ohne Verhaltensänderung: `requests::outline_request_in` (Kern von
  `Store::outline_request`), `requests::add_request_in`, `outline::apply_with` (Schalter `bare`),
  `outline::heading_title`, `outline::insert_bare`; in `assets.rs` nur Sichtbarkeit (`Content`, `prepare`,
  `free_name`, `path_owner`, `name_fits`).
- **Wege** (`baruch-server/src/exchange/import.rs`): Seite `/austausch/einlesen`, Formulare `/austausch/vorschau`
  (multipart, Eingangsdatei erst nach gelungener Vorschau unter `<data>/austausch-eingang/<sha256>`) und
  `/austausch/uebernehmen` (Datei aus dem Eingang, danach gelöscht), RPC `work.exchange_preview`/`import`, CLI
  `cli_import`; Vorlagen `exchange_import.html`, `exchange_preview.html`, `exchange_done.html` (eigene Umgebung).
  Vorschau und Übernehmen halten `state.write_lock`; der Spiegel-Commit kommt aus `mirror::record_exchange`
  (`describe_exchange`, ein Commit, `allow_empty`).
- **Keine Migration** in dieser Stufe (0043 bleibt frei): Die Herkunft der Anträge ist `IdeaOrigin::Human` mit dem
  Namen „Austausch: <Instanz>“.
- Tests: `crates/baruch-exchange/tests/read.rs` (Prüfdateien `tests/data/import/`), `crates/baruch-store/tests/
  exchange_apply.rs`, `crates/baruch-server/tests/round26_import.rs` und `round26_roundtrip.rs` (meldet
  ÜBERSPRUNGEN, solange `work.exchange_export` ein Stub ist).

### Integration Runde 26

Zusammengeführt mit `--no-ff` in der Reihenfolge ids → export → import (Konflikte keine; die Doku-Abschnitte lagen
getrennt). Danach:

- **Fehlalarm im Wächtertest** `no_code_path_but_exchange_apply_writes_the_uuid_column` (`baruch-store/tests/uuids.rs`):
  Er suchte `INSERT` und `, uuid =` zeilenweise und schlug auf `HashSet::insert(uuid…)` im Leser und auf
  `uuid => …` im minijinja-Kontext der Ausgabeseite an. Jetzt nur noch SQL: `INSERT INTO` bzw. `, uuid =` ohne `>`.
- **Rundreise-Test** `round26_roundtrip` lief in import nur als „ÜBERSPRUNGEN“; nach dem Merge scheiterte er an der
  Überschriftsebene (`===` an einem Abschnitt, der Store verlangt `==`). Die handgeschriebene Prüfdatei
  `werk.baruch.xml` trägt bewusst ungewöhnliche Texte (Leerraum, `]]>`, Steuerzeichen); das Einlesen prüft die
  Typst-Ebene der Fassungen nicht gegen die Art des Punkts (Text bleibt exakt, wie er kam). Ein dritter Test zeigt
  Umbenennen auf B → auf A die mitgekommene Fassung und der Hinweis „Umschalten genügt“, kein Antrag `Rename`.
- **MCP-Hinweis beim Verbinden** nennt `exchange_export` und sagt, dass es das Einlesen über MCP nicht gibt.
- **Sichtprüfung:** Einzahl/Mehrzahl in Vorschau, Ergebnis und Ausgabeseite („1 Fassung“, „1 Antrag“); das Formular
  „Übernehmen“ wuchs als leerer Kasten bis zum Seitenende (`.works-page` ist eine Flex-Spalte, `.work-form` hat
  `flex: 1 1 320px`) → `flex: 0 0 auto` auf den Austauschseiten; die Tabelle der Abweichungen links ausgerichtet; in der
  Antragsliste unter `/gliederung` bricht die lange Begründung (Instanz-UUID und sha256) um, statt die Knöpfe aus dem
  Kasten zu schieben.
- **Ende-zu-Ende mit zwei Wegwerf-Instanzen** (eigene Server auf `127.0.0.1:0`, `--secrets memory`, Verbindungen auf
  `127.0.0.1:9`, dritter Datenordner nur für die Kommandozeile): 75 Prüfungen grün – Werk mit zwei Teilen, Vorwort ohne
  Nummer, Abschnitt, mehreren Fassungen (abgelöst, Vorschlag über MCP mit Modell), zwei Vorlagenfassungen, Einfällen
  (offen, erledigt, gelöscht), Beiträgen, Faden, PNG mit zwei Fassungen als Umschlag, SVG, Buchangaben, Sprechdauer;
  A → B neu und B → Ausgabe gleich A bis auf `<ausgabe>`; Kommandozeile (Vorschau, neu, Exit 1 ohne `--abgleichen`,
  Export ohne Überschreiben, Datei über 128 MiB); B ändert Fassung, Einfall, Umbenennen, „mit Nummer“ → A gleicht über
  das Formular ab: zwei weitere Fassungen, ein Einfall, Antrag „mit Nummer zählen“ mit Herkunft „Austausch: Laptop“,
  „aktuell“ unverändert, keine UUID verschwunden; zweiter Durchgang legt nichts an; `..`-Pfad, falsche Prüfsumme,
  DTD mit externer Entität, XML-Bombe, 40 Ebenen, ZIP-Bombe (70 MiB `werk.xml`) abgelehnt, Zeilenzahlen aller 17
  Werktabellen gleich; MCP-, XML- und ZIP-Ausgabe ohne Schlüssel, MCP-Token und Persona-Modelle.

### Nachbesserung Runde 26 (Gutachten)

- **Gelöschtes Werk zurückspielen:** `delete_work` lässt Einfälle als Einfälle ohne Werk mit ihrer Kennung zurück
  (0011, `ON DELETE SET NULL`); das Einlesen als neues Werk lehnte deshalb die eigene Sicherung ab („gibt es hier
  schon in einem anderen Werk“). Jetzt ordnet `exchange_apply::adopt_orphan_idea` einen solchen Einfall dem
  eingelesenen Werk wieder zu (nur `work_id` und `node_id`, Inhalt bleibt), beim Neu-Einlesen wie beim Abgleich.
- **Hin und her ohne Doppelung:** Unter (c) und (d) wird keine Fassung angelegt, wenn der Punkt schon eine mit genau
  demselben Text hat (`same_text_number`); vorher entstand nach einer Bearbeitung an Ort und Stelle beim Abgleich im
  Wechsel je Seite eine inhaltsgleiche Kopie (Austauschformat.md §4 „Hin und her“).
- **Instanzname aus der Datei:** `baruch_exchange::clean_instance_name` beim Lesen und noch einmal in
  `mirror::describe_exchange` (`<`/`>` → `‹`/`›`, Steuerzeichen → Leerzeichen, 80 Zeichen). Vorher scheiterte der
  Spiegel-Commit an „Laptop <Jörg>“ (gix lehnt `<>\n` im Autor ab), obwohl das Werk schon übernommen war.
- **Grabsteine neuer Art:** Bei `version` > 1 wird ein unbekanntes `<eintrag art>` als unbekannt gemeldet und
  übersprungen statt die Datei abzulehnen (Vorbereitung R25, Austauschformat.md §6).
- **Rundreise-Test ohne Ausweg:** `round26_roundtrip::export` scheitert bei jedem Fehler hart; die Variante
  `ExchangeError::NotImplemented` ist entfernt (kein Stub erzeugt sie mehr).

## Gedankenkarte (Runde 25)

**Stand Runde 34:** Seite, Bild, Widget und Ausgaben dieses Abschnitts sind durch die Mindmap ersetzt (siehe „Runde 34
– Mindmap“ am Ende); `bild/layout.rs`, `bild/svg.rs` und `web/src/idea_map.ts` gibt es nicht mehr. Daten,
Verbindungen, Nähe, Gruppen und Austausch gelten weiter.

Die Einfälle eines Werks als Karte (Vertrag Runde 25): Mitte das Werk, darum Teile und Kapitel, Einfälle an
ihrem Bezug, Verbindungen zwischen Einfällen (Migration `0043_idea_map.sql`, Tabellen `idea_link` und
`idea_map_pos`), Nähe nur angezeigt. Umgesetzt in drei Zweigen (karte-daten, karte-bild, karte-widget) auf der
Schnittstelle R25-S1; einziger bleibender Stub war das Benennen von Gruppen durch einen Assistenten – seit Runde 19b
umgesetzt (`idea_map/groups.rs`, siehe „Runde 19 – R19b pgvector“).

### Daten (karte-daten)
- **Migration 0043** (`0043_idea_map.sql`): Tabellen `idea_link` (Einfall → Einfall, `kind` aus vier Wörtern,
  `note` ≤ 500, Herkunft, `deleted` weich, `uuid`; CHECKs gegen Selbstbezug und für „widerspricht“ nur mit
  `from_id < to_id`, `UNIQUE (from_id, to_id, kind)`) und `idea_map_pos` (je Werk und Einfall eine Lage, Koordinaten
  ≤ 1e6). Beide mit uuid wie 0041 (eindeutiger Index, Fülltrigger, Grabstein-Trigger: Verbindung mit dem Werk des
  Einfalls `from_id`, Lage mit ihrem Werk) und `work_map_pos_cascade_uuid_tombstone` (Lagen vor dem Werk löschen,
  damit der Grabstein die Werk-uuid kennt) sowie `idea_link_cascade_uuid_tombstone` (Verbindungen vor einem hart
  gelöschten Einfall löschen, sonst fehlte dem Grabstein einer Verbindung mit diesem `from_id` das Werk; vom
  Integrator bei der Zusammenführung nachgetragen). `uuid_tombstone` bekommt die Arten `idea_link`/`idea_map_pos`: die zehn
  Grabstein-Trigger aus 0041 fallen, die Tabelle wird über eine temporäre Kopie neu angelegt (nicht `RENAME`, damit
  der Text in `sqlite_master` bis auf den CHECK gleich bleibt), dann stehen die Trigger wörtlich wieder da. Kein
  Fremdschlüssel zeigt auf `uuid_tombstone`, darum nicht in `FOREIGN_KEYS_OFF`. Test `tests/migration_0043.rs`
  (nachgebaute Datenbank im Stand 0042 mit Grabsteinen aller zehn Arten).
- **Store** `src/idea_links.rs`: `LinkKind`, `IdeaLink`, `NewIdeaLink`, `MapPos`; `idea_link_create` (prüft erst,
  dann eine Transaktion; normalisiert „widerspricht“; vorhandenes aktives Paar → `changed = false` ohne Schreiben,
  gelöschtes → wiederhergestellt), `idea_link_get/delete/restore`, `idea_links_of_idea/of_work` (`deleted = false`:
  aktive Verbindungen mit zwei nicht gelöschten Einfällen – ein gelöschter Einfall fällt so nur durch den Filter
  heraus), `idea_map_positions` (nur nicht gelöschte Einfälle, die noch zum Werk gehören), `idea_map_pos_set`
  (UPSERT, die uuid bleibt), `idea_map_pos_reset` (hart, Grabstein). Fehler über `StoreError::Idea` (→ -32602), die
  uuid setzt nur der Fülltrigger. `einfaelle.md` (`ideas.rs`) bekommt je Einfall „- Verbindungen: → 79 führt weiter;
  ↔ 48 widerspricht („Notiz“); ← 12 Beispiel für“ über `idea_links::mirror_lines`. Test `tests/idea_links.rs`.
- **Server** `src/idea_map/mod.rs`: RPC (`call`, Parameter auch als Ziffern-Text für das Formular der Karte;
  `filter_from_params` = gemeinsame Auslegung für RPC und Seite) und `build` (Knoten aus der Gliederung mit
  Aufklappen `open`, Filter status/word/origin, Treffer `q` über `Store::idea_search`, Verbindungen, Lagen, Nähe).
  `idea_map/links.rs`: Verbindungen schreiben unter `write_lock` mit genau einem Commit über `mirror::record_idea`
  (eigener Commit-Auftrag `describe`, `mirror.rs` blieb unverändert), auch für MCP. `idea_map/near.rs`: Paare aus
  gespeicherten Vektoren (Kosinus ab Schwelle) und gleichen Schlagwörtern (`word_key`), je Einfall höchstens
  `near_limit` Partner (gierig nach Wert, deterministisch). `rag/neighbors.rs`: liest die gespeicherten Vektoren
  `idea:<id>` des Werks aus LanceDB (`Index::stored`), nie eine Einbettung; ohne Einbetter oder Index-Ordner
  `NoEmbedder` (legt keinen Ordner an), bei Fehler oder anderem Modell `Unavailable`.
- **MCP** `src/mcp/idea_map.rs`: `idea_map` (mit `url` je Einfall), `idea_links`, `idea_link`, `idea_unlink`; Herkunft
  über `tools::origin_of` (Persona oder `@mcp:<client>`).
- Tests `tests/round25_daten.rs`: 40 erfundene Einfälle (Knoten, Anker, Zähler, Aufklappen, Filter, `q`), Commits
  mit Trailern und `einfaelle.md`, Lagen ohne Commit, gelöschter Einfall, MCP mit Herkunft, Nähe mit eigenem
  Nachbau-Einbetter (Beleg: keine Einbettung beim Zeichnen) und ohne Einbetter.
<!-- Ende R25 karte-daten -->

### Bild und Seiten (karte-bild)

`crates/baruch-server/src/idea_map/bild/` macht aus der `IdeaMap` von `idea_map::build` (karte-daten) Bild, Seite
und Ausgaben; eigene Daten oder RPC hat es nicht.

- **`layout.rs`** – deterministische radiale Anordnung, rein (keine Zufälle, keine Reihenfolge aus HashMaps): Werk bei
  (0,0); Ringe nach der Tiefe in der Gliederung der Karte; Winkel wie bei einem radialen Baum (oberste Knoten ab 12 Uhr
  im Uhrzeigersinn, Sektor nach dem Platzbedarf des Teilbaums, Kinder im Sektor des Elternknotens). Die Einfälle eines
  Knotens liegen als **Traube** auf einem Sechseckgitter (`LATTICE` 52, Radius `IDEA_R` 13, nächste Gitterpunkte
  zuerst, der Streifen der Beschriftung unter dem Knoten bleibt frei; im letzten Gitterring nach außen gerichtete
  Plätze zuerst). Keine Überlappung **per Konstruktion**: im Gitter ≥ 2·`IDEA_R`+`GAP`, zum Knoten ≥ r+`IDEA_R`+`GAP`;
  je Ring wächst der Radius (×1,05), bis jedes Paar von Trauben-Scheiben ≥ Rc₁+Rc₂+`GAP` auseinander ist; Ringe sind
  disjunkte Kreisringe. Gespeicherte Lagen (`positions`) überstimmen (`pinned`, kein Gitterplatz). Messung: 300 Einfälle
  ≈ 1 ms im Debug-Bau (Test `round25_bild::three_hundred_ideas_…`, Abnahme < 200 ms). Ergebnis `Layout` (Kreise in der
  Reihenfolge Knoten, Einfälle; `get(id)`; viewBox mit Rand für Beschriftungen).
- **`svg.rs`** – Zeichnung nach dem DOM-Vertrag §4.4: `<svg class="im-svg" … data-im-svg>`, `<g class="im-viewport">`,
  Kanten als `<line>` von Mittelpunkt zu Mittelpunkt (Baum `im-tree`, Bezug `im-ref`, Verbindungen
  `im-link im-kind-<art>` mit `data-link`/`data-kind` und `<title>`, Nähe `im-near` mit `data-score`, nur mit `near`;
  seit Runde 18a Pfeilspitzen, Strichart als Attribut und sichtbare Beschriftung `im-edge-label`, siehe „Runde 18a“),
  Knoten mit `data-node`/`data-x`/`data-y`/`data-r` und `transform="translate(x y)"` (Werk und „Ohne Kapitel“ `<g>`,
  Teil/Kapitel/Abschnitt `<a>` in Key-Form, Einfälle `<a href="/einfaelle/<id>">` mit `data-idea`/`data-key`/
  `data-anchor`, `im-open|im-done`, `im-pinned`, mit `q` `im-hit|im-dim`). Zahlen auf eine Nachkommastelle (`num`),
  alle Nutzertexte über `esc`. `render(map, layout, standalone)`: im HTML ohne eigenen Stil (`static/karte.css`, Farben aus
  `app.css`, darum Hell/Dunkel ohne Doppelung), eigenständig mit XML-Kopf, Größe, `<title>` und `STANDALONE_STYLE`
  (feste helle Farben, nur einfache Klassen-Selektoren – der SVG-Leser im Typst-Satz kennt keine Variablen); Attribute
  ohne Wert dann als `=""` (wohlgeformtes XML, von `xmllint` geprüft).
- **`export.rs`** – Markdown (Überschrift je Knoten, Ebene = Tiefe + 1; Einfälle mit Verbindungen in der Schreibweise
  des Spiegels; „Widersprüche“, „Nähe“; Markdown-Zeichen maskiert) und FreeMind 1.0.1 (Knoten verschachtelt wie auf
  der Karte, IDs `ID_<id ohne Sonderzeichen>`, Gedanke als `richcontent TYPE="NOTE"`, Verbindungen als `arrowlink`
  – „widerspricht“ mit Pfeil an beiden Enden – und als Attribut „Verbindung“). `tree_order` und `links_of` nutzt auch
  die Seite.
- **`mod.rs`** – Routen und Seite. Filter aus der Adresse (`parse_filter`, unbekannte Werte fallen weg; `near` letzter
  Wert gewinnt, weil das Formular ein verstecktes `near=0` vor das Kästchen setzt; `open` mehrfach) und kanonischer
  Abfrageteil (`filter_query`) für alle Links, Ausgaben und den Rücksprung. Eigene Template-Umgebung (`base.html` +
  `templates/karte.html`, kein `fragments/idea_capture.html`). Ausgaben: `.svg` (`image/svg+xml`), `.md`
  (`text/markdown; charset=utf-8`), `.mm` (`application/x-freemind`, Anhang), `.pdf` über `pdf::source_to_pdf(work,
  PdfKind::Map, …)` (`pdf_source`: A4 quer, Titel und Filter nur als Typst-Zeichenketten, die eigenständige SVG als
  Datei `karte.svg` eingepasst; 409 läuft schon, 503 Zeit, 422 Satzfehler – als Karte mit Meldung).
  `POST /werke/{id}/karte/act` (hinter `guard::same_origin`): `idea.update` über `crate::ideas::call` (nur `id`, `key`),
  `idea.link`/`idea.unlink`/`idea.link_restore`/`idea.map_pos_reset` über `crate::idea_map::call`; vorher Prüfung, dass
  der Einfall bzw. die Verbindung zu diesem Werk gehört (sonst 422). `back` wird nicht übernommen, sondern als Filter
  gelesen und neu aufgebaut (`back_filter`) – die Weiterleitung `303 /werke/{id}/karte?<filter>&ok=<wort>[&undo=<id>]`
  enthält nichts Ungeprüftes. Fehler: die Karte mit Meldung (422 ungültig, 409 abgelehnt, 400 unbekannte Aktion, 404
  unbekanntes Werk).
- Daneben: `PdfKind::Map` („Gedankenkarte“) in `pdf.rs`; Menüpunkt in `base.html`; `ideas.rs` gibt den Seiten
  `map_href` (Liste) bzw. `links`/`map_href`/`map_act` (Einfall, über `Store::idea_links_of_idea`) mit; Stil in
  `static/karte.css` (Strichart je Art, `im-empty`, `im-dim`, `im-hide-near`, Druck, Dunkel) und `static/ideas.css`.
- Gruppen benennen: seit Runde 19b kein Stub mehr, sondern der Knopf „Gruppen benennen (Assistent)“ mit
  Abschnitt „Gruppen“ (`idea_map/groups.rs`, siehe „### R19b pgvector“).
<!-- Ende R25 karte-bild -->

### Widget und Austausch (karte-widget)

**Widget** `crates/baruch-server/web/src/idea_map.ts` (`register("idea-map", …)` auf `.im-stage`, von `build.rs`
übersetzt, geladen über `main.ts`), Stil `static/karte_widget.css`. Es liest nur den DOM-Vertrag §4.4 (Klassen
`im-*`, `data-node/x/y/r/key/idea/anchor/work/near`, `data-from/to` der Kanten, `[data-im-viewport]`,
`[data-im-toolbar]`, `[data-im-side]`, `form.im-filter`) und ruft nur `idea.get`, `idea.create`, `idea.update`,
`idea.link`, `idea.map_pos_set`, `idea.map_pos_reset` (Test `round25_widget::the_widget_reads_only_the_dom_contract_and_calls_only_the_allowed_methods`
prüft das am ausgelieferten Skript). widgets.ts, ideas.ts, zoom.ts und main.ts bleiben unverändert.

- **Ausschnitt:** Zustand `{s, tx, ty}` → `transform="translate(tx ty) scale(s)"` nur auf `[data-im-viewport]`.
  Zoom um einen Punkt der viewBox (`svg.getScreenCTM()`), Verschieben über die Differenz zweier viewBox-Punkte;
  `sessionStorage` `baruch_im_view_<werk>` (try/catch) hält den Ausschnitt über das Neuladen. Mausrad mit Strg
  wird nicht angefasst (zoom.ts).
- **Ziehen:** Pointer Events mit `setPointerCapture`; ab 4 Bildschirmpunkten wird aus dem Klick ein Ziehen.
  Knotenkoordinaten über `viewport.getScreenCTM().inverse()` (also in den Einheiten von `data-x/data-y`);
  `place()` setzt `data-x/y`, `transform` und die Endpunkte jeder `.im-edge` mit `data-from`/`data-to` des Knotens.
  Ablageziel (`targetAt`): Knoten mit Abstand ≤ `data-r`, Einfälle vor Gliederungsknoten, das Werk nie. Einfall →
  Dialog (`<dialog class="im-dialog">`, vier Arten, Notiz ≤ 500) → `idea.link`; Teil/Kapitel/Abschnitt →
  `idea.update {id, key}`; „Ohne Kapitel“ → `idea.update {id, key: null}`; sonst `idea.map_pos_set` (Koordinaten auf
  ±1e6 begrenzt) und `im-pinned`. Nach Bezug und Verbindung lädt die Seite neu (der Server ordnet an).
- **Tastatur:** wandernder `tabindex` (genau ein Knoten 0, zuerst das Werk), `<g>`-Knoten bekommen `role="button"`
  und eine `aria-label` aus ihrem `<title>`. Pfeile: nächster Knoten mit Abstand in der Richtung plus doppeltem
  Querversatz; Verbinden-Modus (`V`, Klassen `im-connecting`/`im-source`/`im-target`) wandert nur über Einfälle.
  `N` schaltet `im-hide-near` und `near` in der Adresse (`history.replaceState`); war die Seite mit `near=0`
  gezeichnet (keine Nähe-Kanten im DOM), lädt sie mit `near=1` neu. Alt+E: Dialog „Neuer Einfall“ →
  `idea.create {work, key, words, note}` mit dem Bezug des fokussierten Knotens. Meldungen über `.im-status`
  (`role="status"`), Hilfe `.im-help` per `aria-describedby` an der Karte. Texte nur über `textContent`.

**Austausch Unterversion 2** (Vertrag §4.6):

- `baruch-exchange/src/model.rs`: `FORMAT_VERSION = 2`; `ExchangeLinkKind` (`fuehrt-weiter`, `gehoert-zu`,
  `beispiel-fuer`, `widerspricht`), `ExchangeLink`, `ExchangePosition`, `ExchangeMap` (`ExchangeWork.map`),
  Grabsteinarten `Link = "verbindung"` (`idea_link`) und `Position = "lage"` (`idea_map_pos`) mit
  `since_version()` (2, sonst 1); dazu `MAP_COORD_LIMIT`, `MAX_LINK_NOTE_CHARS` und `ExchangeMap::is_empty`.
- `write.rs`: `<karte>` am Ende von `<werk>` nur mit Inhalt, erst `<verbindung>` (mit `<notiz>`), dann `<lage/>`;
  x/y über `f64::to_string` (kürzeste genaue Darstellung), außerhalb ±1e6 → `ExchangeError::Write`.
- `read/map.rs`: `<karte>` erst ab `version >= 2` (sonst unbekannt, `werk/karte`); eine Grabsteinart mit
  `since_version() > version` ist ein XML-Fehler; unbekannte Arten bei `version > FORMAT_VERSION` werden gemeldet und
  übersprungen. `read/check.rs` (`check_map`): Verweise auf Einfälle des Dokuments, kein Selbstbezug, kein Paar
  mit derselben Art doppelt (`widerspricht` normalisiert), Notiz ≤ 500 ohne Steuerzeichen außer `\n`, je Einfall
  eine Lage, |x|, |y| ≤ 1e6; die Kennungen zählen zur Eindeutigkeit im ganzen Dokument (auch gegen Grabsteine).
- `docs/schema/baruch-werk-1-u2.xsd`: Kopie des v1-XSD mit festgelegtem `Karte` (`Verbindung`, `Lage`,
  `VerbindungArt`, `Koordinate` als `xs:double` in ±1e6) und erweitertem `GrabsteinArt`; das v1-XSD bleibt
  unverändert.
- Store `exchange_dump.rs`: `map()` liest `idea_link` (beide Einfälle im Werk, auch weich gelöschte, nach id) und
  `idea_map_pos` (Einfall noch im Werk, nach id); Grabsteine wie bisher über `work_uuid`.
- Store `exchange_apply.rs`: `new_map` (nach Einfällen und Bildern; `ensure_free`, `widerspricht` mit from < to
  nach lokalen ids) und `merge_map` (nach `merge_ideas`, damit eben eingelesene Einfälle zählen): unbekannte
  Kennung + beide Einfälle im Werk → anlegen, sonst Hinweis; gleiches Paar+Art (in der normalisierten Richtung)
  → Hinweis „gibt es hier schon unter anderer Kennung“; bekannte Kennung → nie überschreiben (Hinweis bei Notiz/
  gelöscht); Grabstein → „hier gelöscht“; Lage nur, wenn es für (Werk, Einfall) keine gibt. `AddCounts.links`,
  `AddCounts.positions` (in `total()`), Wörter `verbindung`/`lage` in `items` und `deleted_here`. Nur hier wird
  `uuid` in `idea_link`/`idea_map_pos` geschrieben (Wächtertest `uuids.rs`).
- Server: `exchange/import.rs` (`add_list`: „Verbindungen der Gedankenkarte“, „Lagen auf der Gedankenkarte“ in
  Vorschau, Ergebnis und CLI), `exchange/export.rs` (`counts`: `links`, `deleted_links`, `positions` für
  `templates/exchange.html`), Beschreibung von MCP `exchange_export`. Keine Migration (0044 bleibt ungenutzt).
- Tests: `baruch-exchange/tests/{read,write,xsd}.rs`, `baruch-store/tests/{exchange_map,exchange_apply,uuids}.rs`,
  Server `tests/round25_widget.rs` (Rundreise A → B → A mit Karte, Vorschauseite, Seite „Austausch“) und
  `round26_*` gegen das u2-XSD.
- Gefunden (gemeldet an karte-daten, Migration 0043): Wird ein Einfall hart gelöscht, der `from_id` einer
  Verbindung ist, schreibt der Grabstein-Trigger `idea_link_uuid_tombstone` `work_uuid` NULL (die Einfallzeile ist
  in der Kaskade schon fort, wie vor 0042 bei Fäden); Abhilfe wäre ein `BEFORE DELETE ON idea`, der die
  Verbindungen vorher löscht. Praktisch selten (Einfälle werden nur weich gelöscht). Behoben bei der Integration
  (Trigger `idea_link_cascade_uuid_tombstone` in 0043, `uuids.rs` prüft jetzt beide Richtungen).
<!-- Ende R25 karte-widget -->

### Integration Runde 25 (Integrator, 2026-11-08)
- Zusammengeführt `team/karte-daten` → `team/karte-bild` → `team/karte-widget` (`--no-ff`, ohne Konflikte; alle drei
  von S1 `c0e5e8a` abgezweigt).
- **Migration 0043 berichtigt** (noch auf keiner Instanz ausgeführt): `BEFORE DELETE ON idea` löscht die Verbindungen,
  solange die Einfallzeile steht (`idea_link_cascade_uuid_tombstone`), sonst trüge der Grabstein einer Verbindung mit
  diesem `from_id` kein Werk (Befund karte-widget). `migration_0043.rs` kennt das neue Objekt, `uuids.rs` löscht jetzt
  den mittleren Einfall (Quelle und Ziel zugleich).
- **Kanonische Adresse der Karte:** `build` setzt die Vorgaben `near_limit`/`near_threshold` in den angewandten Filter
  ein; `bild::filter_query` schrieb sie dann in jeden Link (`?near_limit=3&near_threshold=0.75`). Erst nach dem
  Zusammenführen sichtbar (zwei Tests in `round25_bild.rs` schlugen an); jetzt fallen Vorgabewerte aus der Adresse.
- Die Teile von `round25_bild.rs`, die ohne R25a „ÜBERSPRUNGEN“ meldeten, laufen jetzt voll.
- **Abnahme** `tests/round25_abnahme.rs`: erfundenes Werk mit zwei Teilen und 40 Einfällen, Einbettungen vom Nachbau
  `MockEliezer` (512 Dim.); Seite ohne JavaScript (SVG, Gliederung, Filter per GET, Formulare), `im-empty`, Anker am
  Kapitel bzw. „Ohne Kapitel“; Nähe über Vektoren ohne Einbettungsanfrage beim Zeichnen und über Schlagwörter;
  Bezug per Widget-RPC und Formular (je ein Commit); „widerspricht“ in Karte, Liste Widersprüche, `einfaelle.md` und
  MCP `idea_links`; Lagen ohne Commit; gelöschter Einfall fällt samt Verbindung heraus und kommt zurück; SVG/Markdown/
  FreeMind/PDF (xmllint). Ohne Einbettungsmodell nennt die Seite den Ersatz über Schlagwörter.
- **Sichtprüfung** (Firefox headless, eigener Wegwerf-Server mit erfundenem Werk, Einbettungen von einem eigenen
  Nachbau auf 127.0.0.1:0): Die Bühne war in der Flex-Spalte `.karte` auf ihre Mindesthöhe gedrückt (Karte winzig,
  mit JavaScript unten abgeschnitten) – jetzt `flex-shrink: 0` für die Kinder von `.karte` und `.im-stage` und eine
  feste Höhe `min(78vh, 900px)` für `.im-svg` (`static/karte.css`). „Einpassen“ im Widget lässt Platz für die
  Beschriftungen unter und neben den Kreisen (`LABEL_SIDE`, `LABEL_BELOW`). Die Seitenleiste nennt den Status wie
  der Rest der Oberfläche „verarbeitet“ (nicht „erledigt“). Bilder: `.claude/sicht/r25/`.
- **Nachbesserung nach dem Gutachten:**
  - Werkwechsel eines verbundenen Einfalls: `Store::idea_update` löst beim Umzug (anderes Werk oder „ohne Werk“)
    in derselben Transaktion alle nicht gelösten Verbindungen des Einfalls weich und löscht seine Lagen hart
    (Grabstein `idea_map_pos` im alten Werk). Vorher blieb die Verbindung aktiv und verband zwei Werke: Karte und
    Austauschformat ließen sie weg, `/einfaelle/<id>`, `idea_links {idea}` und `einfaelle.md` beider Werke zeigten
    sie, „Lösen“ von der Seite eines umgezogenen Ziels scheiterte mit 422, eine alte Lage tauchte beim Zurückziehen
    wieder auf. `ideas::update` nennt die gelösten Verbindungen im Commit beider Spiegel und als `unlinked` in der
    Antwort (RPC `idea.update`, MCP `idea_update`). Tests: Store `idea_links.rs`
    (`moving_a_linked_idea_…`), Server `round25_daten.rs` (`moving_a_linked_idea_to_another_work_unlinks_it_everywhere`).
  - Zähler der Karte rollen hoch: `idea_map::build` zählt einen Einfall am Anker und an allen Vorfahren bis unter
    das Werk (`parent_slot`). Ein Teil mit gefüllten Kapiteln galt vorher als leer (blass, ohne Zahl, „Keine
    Einfälle.“); ein aufgeklapptes Kapitel zählt jetzt seine Abschnitte mit. Liste und Markdown sagen bei einem Punkt
    ohne eigene, aber mit Einfällen darunter „Keine eigenen Einfälle; sie hängen an den Punkten darunter.“
  - „Zeigen, was fehlt“ hebt hervor statt zurückzunehmen: `im-empty` = gestrichelter Rand in der Warnfarbe, im Kreis
    das Wort „leer“ (`svg.rs`, auch in der eigenständigen SVG), in der Liste das Schild „leer“ und ein gestrichelter
    Warnrand (`static/karte.css`, `templates/karte.html`).

## Runde 18a – Menüband

Menüband für Editor und Satz-Ansicht, abbrechbarer Typst-Satz ohne Läufe von fremden Seiten, Kapitel einer geteilten
Seite, Kopfzeile in schmalen Fenstern und die Kanten der Gedankenkarte.

- **Menüband** (`templates/fragments/edit_toolbar.html`): ein Fragment, zwei Fassungen. Im Editor
  (`div.ws-bar.ribbon[data-edit-toolbar][hidden]`) bleiben alle Haken des Satzbilds (`data-ws-mode`, `data-ws-tools`,
  `data-ws-format`, `data-ws-cmd`); dahinter `span.ribbon-formats[data-widget=ribbon]` mit je einem Knopf
  `data-ribbon-cmd`/`data-ribbon-snippet` aus `src/ribbon.rs` (`formats(template)`: `scene`/`work` nur bei
  `#let szene`/`#let arbeit` in der Werkvorlage, dann `footnote`, `quote`, `list`, `enum`, `terms`, `break`, `link`;
  RPC `ribbon.formats`; Kontext `ribbon` aus `view::ViewExtras`) und `data-ribbon-image`. Widget `web/src/ribbon.ts`:
  bei offenem Satzbild nur `sendWsCommand` (eingefrorene `ws_command.ts`), `handled = false` → Hinweis über
  `baruch:toast`; im Quelltext Einfügen ins Textfeld an `selectionStart` (`execCommand("insertText")`, Rückfall
  `setRangeText` + `input`). Kein Import von `wysiwyg.ts`. Mit `satz_page` (Satz-Seite) `div.ribbon-satz
  [data-ribbon-satz]`: Zurück, PDF, ‹ › mit `aria-label`, Formular „Satz abbrechen“ – ohne JavaScript; `typeset.html`
  leert `{% block scripts %}` (kein `main.js`). Stil `static/ribbon.css`. Nachbesserung: `typeset::render` setzt
  `scriptless = true`; `base.html` zeigt dann in der Kopfzeile kein Widget – das Thema als Link auf
  `/einstellungen/darstellung`, „⚙ Einstellungen“ ohne Dialog und `aria-keyshortcuts`, „Öffnen“ der Werkwahl
  sichtbar statt in `<noscript>`.
- **Abbruch** (`baruch-render`): `Cancel(Arc<AtomicBool>)` mit `cancel`/`is_cancelled`, `compile_cancellable(source,
  cancel) -> Option<Rendered>` (`None` = abgebrochen, nie halbe Seiten); `compile`/`compile_pdf` unverändert.
  Prüfstellen: vor dem Lauf, nach `typst::compile` (Auswertung und Umbruch), vor jeder SVG-Seite, am Ende.
  **Gemessen** (`crates/baruch-render/tests/cancel.rs`, Debug-Bau, 2026-11-08): 500 Absätze (512 Seiten) – ohne
  Abbruch 777 ms, abgebrochen nach 200 ms zurück nach 252 ms (der Umbruch war fertig, gespart wurde das Schreiben
  der SVG-Seiten); Auswertungsschleife mit 3 000 000 Runden – ohne Abbruch 353 ms, abgebrochen nach 100 ms zurück
  erst nach 368 ms (nicht unterbrochen). **Nicht möglich:** ein Abbruch über die Welt (nach dem Abbruch keine
  Dateien oder Schriften mehr liefern). Typst merkt sich die Antworten der Welt (`comemo`) und bricht mit „comemo:
  found differing return values. is there an impure tracked function?“ ab, wenn dieselbe Frage in einem Lauf anders
  beantwortet wird – im ersten Entwurf so beobachtet und darum verworfen. Auswertung und Umbruch laufen also immer
  zu Ende; ein eigener Prozess je Lauf wäre der einzige harte Abbruch (nicht umgesetzt, Rest).
- **Server** (`src/typeset.rs`): Jeder Lauf für ein Werk (`compile_for`: Satz-Seite, SVG-Seite, `preview.typst`,
  `typst_check`) meldet sich mit Datenordner, Werk und `Cancel` in einer prozessweiten Liste an; die Anmeldung
  (`Arc<Registration>`) halten Anfrage und Hintergrundlauf, sie fällt mit dem letzten. `cancel(work)` (Vertrag, alle
  Server im Prozess) bzw. `cancel_in(data, work)` (RPC `preview.cancel`, `POST /werke/{id}/satz/abbrechen`) setzen
  alle `Cancel` des Werks. Die Anfrage wartet mit `tokio::select!` auf Sperre bzw. Lauf **oder** den Abbruch (Abfrage
  alle 20 ms) und kehrt sofort mit -32000 `CANCELLED` zurück; der Lauf hält die Sperre `RUNS`, bis Typst an der
  nächsten Prüfstelle aufhört, und legt nichts ab. `compile` ohne Werk (Umschlag) ist nicht angemeldet. Die Seite
  nach dem Abbrechen (`?abgebrochen=1|0`) setzt nicht neu (`cached_typeset`: nur ein schon gesetzter Stand). Für
  Tests: `running_in`, `runs_of(stand)` (begonnene Läufe je Stand, höchstens 256 Stände), `cache_contains`,
  `pdf::pdf_runs()`.
- **Fremde Seiten:** `GET /werke/{id}/satz`, `…/{stand}/{n}.svg` und `/werke/{id}/pdf` – nach der Nachbesserung zu
  Runde 18 auch Umschlag (`/werke/{id}/umschlag`, `.svg`, `.pdf`), `/werke/{id}/stichwortzettel.pdf` und
  `/werke/{id}/karte.pdf`, also jeder GET, der Typst startet und die Sperre `typeset::RUNS` hält – prüfen zuerst
  `guard::cross_site_get` (`Sec-Fetch-Site: cross-site|same-site`) → 403 (`typeset::foreign_get`), bevor sie das Werk
  vorbereiten oder Typst starten. Ohne Kopf (curl, alte Browser) bleibt alles wie vorher.
- **Zwischenspeicher:** zusätzlich zur Zahl (8) eine Bytegrenze `CACHE_BYTES` = 192 MiB über alle SVG-Seiten
  (`cache_put_within`); der neueste Stand bleibt auch über der Grenze.
- **Kapitel einer geteilten Seite:** `Rendered.page_lines` (neu in `baruch-render`) nennt je Seite die Zeilen von
  `main`, aus denen Text auf ihr steht (erste Glyphe jedes Textstücks, auch in Kopf- und Fußzeilen).
  `Typeset::chapter` zählt die Seite, auf der das nächste Kapitel beginnt, zum Kapitel dazu, wenn dort Text aus seinen
  Zeilen steht – ohne Überschriftszeilen, weil die Vorlage Überschriften als lebenden Kolumnentitel oder im
  Inhaltsverzeichnis auf fremde Seiten zitiert. Grenzen: Ein Kapitel, das auf der geteilten Seite nur mit einem Bild
  oder einer Linie (ohne Text) endet, zählt dort nicht; zitiert die Vorlage anderen Text als Überschriften auf fremde
  Seiten, kann eine Seite einem Kapitel zuviel zugeordnet werden.
- **Schriften:** `baruch_render::warm_fonts()` lädt eingebettete und Systemschriften einmal je Prozess;
  `typeset::warm_fonts()` startet das in `spawn_blocking` und kehrt sofort zurück. Vorschlag für den Integrator:
  in `main.rs` beim Start von `serve` (in der Tokio-Laufzeit) aufrufen; Tests rufen es nicht von selbst.
- **Kopfzeile:** `static/app.css` – unter 1000 px Breite `@media (max-width: 999.98px) { .topbar { flex-wrap: wrap } }`;
  darüber bleibt die einzeilige Regel. Der Test (`round9_menu.rs`) liest nur die Regeln; die Breite misst die
  Sichtprüfung.
- **„neu“ als Wort:** `chat.ts` hängt an neue Beiträge `span.badge.post-new-label` („neu“) hinter den Namen.
- **Heller Streifen rechts** (Satz, Gliederungsschema): `.works-page` (`works.css`) ist zugleich Scrollbehälter
  (`overflow: auto`) und auf 1100 px begrenzt und mittig; auf langen Seiten stand seine Bildlaufleiste darum mitten im
  Fenster rechts neben dem Inhalt. `typeset.css`/`numbering.css` geben diesen beiden Seiten die volle Breite und
  rücken den Inhalt über `padding-inline: max(18px, calc((100% - 1100px) / 2 + 18px))` gleich breit in die Mitte.
  Ursache aus dem CSS abgeleitet, nicht im Browser gemessen (Sichtprüfung des Integrators).
- **Gedankenkarte, DOM-Vertrag ergänzt** (`src/idea_map/bild/svg.rs`, `web/src/idea_map.ts`, Tests
  `round25_{bild,widget}.rs`, `round18_menueband.rs`):
  - `<defs>` vor `g.im-viewport` mit `<marker id="im-arrow-<art>" class="im-arrow im-arrow-<art>">` für die
    gerichteten Arten (`leads_to`, `belongs_to`, `example_of`; `contradicts` ist ungerichtet). Die Spitze hat eine
    feste helle Füllung (für SVG-Datei und PDF); auf der Seite färbt `karte.css` sie nach dem Thema.
  - Verbindungskante `line.im-edge.im-link.im-kind-<art>` zusätzlich mit `stroke-dasharray` (außer `leads_to`), bei
    gerichteten Arten `marker-end="url(#im-arrow-<art>)"` und `data-arrow` (in der eigenständigen Datei
    `data-arrow=""`); sie endet dann am Rand des Zielkreises (`r` + 2), sonst im Mittelpunkt. Bezug `im-ref` mit
    `stroke-dasharray="4 3"`, Nähe `im-near` mit `"1 3"`, Baum durchgezogen.
  - Beschriftung: `g.im-edge-labels` nach `g.im-edges` und `g.im-nodes` (seit der Integration über den Knoten, damit
    kurze Kanten zwischen nahen Einfällen lesbar beschriftet sind; `pointer-events: none`), darin je Verbindung bzw. Nähe-Kante
    `<text class="im-edge-label im-label-<art>|im-label-near" data-from data-to x y text-anchor="middle">` mit der
    Art („führt weiter“ …) bzw. „nah“, 4 Einheiten über der Mitte.
  - Widget: `place` zeichnet beim Ziehen jede Kante über `redraw` neu (Pfeilkanten am Kreisrand) und setzt jede
    Beschriftung mit gleichem `data-from`/`data-to` über `relabel` in die Mitte. Die Seitenleiste `.im-side` beginnt
    bei `--im-top` (Abstand des Bilds vom oberen Rand der Bühne, gemessen vom Widget, neu bei `ResizeObserver` und
    `resize`), überdeckt also die Werkzeugleiste nicht mehr.
  - Ausgaben: SVG/PDF wie die Seite (Stil der Beschriftung in `STANDALONE_STYLE`); FreeMind `arrowlink` mit `COLOR`
    je Art (FreeMind kennt keine Strichart für Pfeile); Markdown nennt die Art wie bisher als Wort.

<!-- R38c split: architektur_1 above, architektur_2 below -->

## Round 18b – Constructs

**Typeset view (`crates/baruch-wysiwyg`):** New block kinds `Enum` (numbered list `1.`, `start`) and `Terms` (term
list, `Term {term, desc}`), `List`/`Enum` with `depths` (depth per item, nested; `#[serde(default)]`, not written
without nesting – old JSON stays valid); new inlines `Linebreak`, `Link {url, c?}`, `Ref {target, supplement?}`.
Splitting into pieces (`parse.rs`) knows line kinds for numbered items and terms; a list piece covers indented items
and continuation lines, `list()` determines the depth with a stack of character columns and refuses anything
uncertain (island). `scan::skip_escape` treats `\` before whitespace as a line break (one character), so that a line
end after it ends the logical line. `emit.rs` writes canonically (two spaces per level, consecutive numbers, `\:` in
the term, `#ref(<…>)` before characters of the target, `\ ` before closing characters). `Edit::Raw {replaces, raw}`
replaces the source of a block literally (in the body with its indentation), `WysiwygError::NewPart` forbids new or
converted parts. `image_paths` returns all image paths of an island; `html.rs` shows all of them. Frozen were
`render_block`/`escape` (used by `align.rs`) and the `pub use` lines of `style`/`align`.

**Server:** `wysiwyg::image_ref` takes the normal form without a leading `/`; `rpc.rs` (`revision.update`) converts
Markdown headings like `revision.save`. The part block in the chapter text is fixed (see `docs/WYSIWYG.md` „Teile
und ohne Nummer“): the typeset view refuses, `outline.save` only after confirmation (`split`).

**Widget (`web/src/wysiwyg.ts`):** reads and writes the new constructs (nested `<ul>/<ol>`, `<dl>`,
`span.ws-linebreak`, `a.ws-link`, `button.ws-ref`), opens islands in place by double click (`editIsland`,
`data-edited` → `{replaces, raw}`), runs `baruch:ws-command` (`runCommand`, remembers the last selection in the
typeset view) and answers `baruch:ws-caret` (`caretPlace`: two `wysiwyg.serialize`, text and prefix up to the block
at the cursor). `guardHeadingMerge` skips `.pv` (`blockNeighbour`), `applyStyle` takes over an existing
`style#ws-style[data-work=W]` without RPC. `ws_command.ts` is unchanged.

**Lector leftovers (`src/assist`):**
- Omitted parts: `context::Frame::environment` writes consecutive omitted parts in **one** line
  (`--- nicht mitgegeben (…): A; B; C ---`, `context::OMITTED_HEAD`). The planning counts
  `chunk::OMITTED_OVERHEAD` (100) once per run for this and only title + `chunk::OMITTED_ITEM` (2) for each further
  part, and so does the lower bound `floor_violation`. Consequence: a chapter with 10 sections of 3000 characters each
  now fits in pieces at 4096 tokens (measured: no call above the budget, `round18_lektor.rs`), where Baruch honestly
  refused before.
- Cache of the summaries: `assist::summary_cache()` (process-wide, `VecDeque`, at most `SUMMARY_CACHE_MAX` = 256
  entries, the oldest drop out, no migration). Key: data folder, model and SHA-256 over the whole call (rule including
  the word limit, work title, name of the part, text, `max_tokens`) – that is "model + text" and everything else that
  changes the answer. Empty answers and errors are not stored.
- „An Persona geben“ on `/quelle/multigpt`: form `action=persona` to `/literatur/act` (like the Bible page),
  prefilled with the text of the shown part (at most 4000 characters), source „MultiGPT: Titel (Dokument …)“, without
  the title of the library; without a section only a hint.
- Retry in the stream: new event `piece {persona, index, count}` before each further piece of a job in pieces (after
  the separating delta, `Progress::Piece`); `assist.ts` starts a new part of the provisional post there, `retry`
  empties only that one. The final text already arrived correctly with `done` before.

## Round 18c – Typeset view per work

- **Store** (`crates/baruch-store/src/sheet.rs`, migration `0044_work_sheet.sql`): table `work_sheet` (one row per
  work, `ON DELETE CASCADE`, CHECK on lengths or `#`+6 characters; a new table, no rebuild, hence not in
  `FOREIGN_KEYS_OFF`), `SheetSettings` (`font`, `heading_font`, `size`, `fill`, `heading_fill`, `background`,
  `width`; `None` = from the template), `Store::sheet_settings` (empty without a row) and `set_sheet_settings` (whole
  row, all empty deletes it). Checked before writing (`SheetSettings::normalized`): font lists at most
  `MAX_SHEET_FONT_CHARS` = 200 characters of names with letters, digits, spaces, `- _ .` (also paired in quotation
  marks) – never `< > ; { } \` or control characters; lengths `pt|mm|cm|em` (size 6–48 pt, width 6–60 cm,
  `em` = 12 pt); colours `#rrggbb`. No mirror, no uuid, not in the exchange format (leftover). Tests
  `tests/{sheet,migration_0044}.rs`.
- **Rendering** (`baruch-wysiwyg/src/style.rs`): `SheetStyle.background` (new; never from the template) →
  `--ws-background`; `sheet_style_over(vorlage, einstellungen)`; since 18c `safe` allows `_` and only paired quotation
  marks. Server `src/sheet.rs`: `sheet_style_for` = `sheet_style_over(sheet_style(work_template), einstellungen)` for
  `wysiwyg.style`, `view.rs` (`ViewExtras.sheet_css` → `<style id="ws-style" data-work>` in
  `fragments/editor.html`) and RPC `work.sheet`/`work.sheet_set`; `sheet_css_for` replaces every `<` for safety.
  `static/sheet.css` (directly after `wysiwyg.css`): sheet colour and `::highlight(literatur-fundstelle)`. Form
  `POST /einstellungen/werk/satzbild` and block „Satzbild“ in `settings/work.rs` / `templates/settings/werk.html`.
  Typst, PDF and export never read `work_sheet` (test: source, state and PDF byte-identical).
- **Long sections:** `view::READY_SHEET_MAX` = 64 KiB; above it no pre-rendered typeset view (`ready_sheet`).
- **Image at the cursor position** (`src/assets.rs`): `Place` (`end|start|block:<n>|offset:<n>`), `insert_figure_at`
  (pure: never before/into the heading, line boundary, islands/scenes/lists as a whole, empty lines before and after),
  `image_places` (choice without JavaScript). Store `save_human_with_asset_path`: path and revision in one transaction
  (`BEGIN IMMEDIATE`), one mirror commit. Widget `web/src/sheet.ts` („sheet-tools“ on `details[data-image-insert]`):
  position via `baruch:ws-caret` (on the editor form with `bubbles`, answer from `wysiwyg.ts`) or `selectionStart`;
  without an answer `baruch:serialize-text` and the end. It also refreshes `#ws-style` after
  `baruch:settings-changed` via `work.sheet`.
- **Proposals:** `align.rs` `split_changed` (word Dice ≥ 0.5, monotone pairing, only for unequal numbers of blocks);
  line diff without trailing whitespace (`view.rs`); bar `div.pv-bar-editor[data-pv-bar]` before the reading flow
  (`proposals.html`, `proposals.css` with `order: -1`/`sticky`, `proposals.ts`).
- **Literature** (`web/src/literature.ts`): the passage found is marked with the CSS Custom Highlight API while the
  typeset view is open (`literatur-fundstelle`), no DOM intervention; if it opens the typeset view first, it is marked
  after `data-ws-open`.
- **Behaviour test** `tests/round18_satzbild.rs`: Node 20 with its own small DOM mock (as text in the test), the
  compiled `SCRIPTS` in a temp folder, `fetch` mocked; without `node` „ÜBERSPRUNGEN“.

## Integration of Round 18

- Merged with `--no-ff` in the order `team/konstrukte` → `team/satzbild-werk` → `team/menueband`, without conflicts
  (file ownership by contract R18).
- `main.rs` calls `typeset::warm_fonts()` at the start of `serve` (font search in the background, after
  `rag::catch_up`).
- Assertions of others updated: `migration_0040.rs`/`migration_0041.rs` exclude the new table `work_sheet` (0044) from
  the comparison of the schema objects and of the references to `work`; `round15_nachbesserung.rs` now checks the
  honest refusal at 3000 instead of 4096 tokens, because omitted parts share one line since 18b and the old case fits
  (measured in `round18_lektor.rs`); `round18_menueband.rs` reads `details[data-image-insert]` with the widget
  `sheet-tools` from 18c.
- `docs/RPC.md`: event `piece` and discarding only the running piece on `retry` in the stream `/assistenten/stream`.
- Acceptance `tests/round18_abnahme.rs`: an invented work with scene, work assignment, footnote, quotation, numbered
  list, term list, nested list, line break, link, reference, two images in one block and an island. Every block that
  the widget sends anew after an edit stays byte-identical for canonical text; the island is changed in place; formats
  of the template in the ribbon and a button text saved without JavaScript as a scene; typeset view settings in the
  HTML before every script and after a restart; image at the cursor position; other pages trigger no typesetting,
  „Satz abbrechen“ without a running typesetting reports `abgebrochen=0`.
- Visual check (Firefox headless, throwaway server, invented work; images under `.claude/sicht/r18/`), fixed:
  - Below 900 px the stacked areas stayed 16em high with `minmax(16em, auto)` (the workspace has a fixed height,
    there is no free space to grow); the editor (85vh) reached under „Assistenten“. Now `grid-auto-rows: max-content`,
    outline and assistants between 16em and 85vh (`static/app.css`).
  - Labels of the idea map now lie above the nodes (`g.im-edge-labels` as the last group, `pointer-events: none`):
    with short edges between close ideas, „Beispiel für“ disappeared under the circles.

## Round 27

Cover editor and imprint in the work template. Interface (S1): types in `baruch_typst::cover_layout` (`CoverBlock`,
`CoverBlockKind`, `CoverFace`, `CoverLayout`, `CoverGeometry`, limits equal to the CHECKs), `TemplateFunctions`,
`BOOK_DICT` and placeholders in `baruch_typst::cover`, `font_families`/`has_font_family` in `baruch-render`,
migration 0045 (`cover_block`, tombstone kind `cover_block`, addresses in `work_book`, request kind `cover_layout`),
store `cover_blocks.rs`, server `cover_layout.rs`, `cover_print.rs`, `mcp/cover.rs`, widget `cover_editor.ts`.

### 27a – Data (umschlag-daten)

- **Store `cover_blocks.rs`** (migration 0045, no further migration; 0046 stayed unused): `check_cover_block` checks a
  block against the limits from `baruch_typst::cover_layout` (equal to the CHECKs) and brings it into normal form
  (colour lower case, empty font = `None`, spine without direction = `down`, kinds with their own text without text =
  `""`, CRLF → LF). Font names follow the rule of the typeset view fonts (`sheet::check_font`, here a single family
  name). Violations are `StoreError::Asset(String)` (RPC -32602) with kind and field – `Refused` carries only fixed
  texts and would give `REFUSED`; an error kind of its own would have changed `lib.rs` (integrator).
  `set_cover_blocks` writes per kind with `INSERT … ON CONFLICT (work_id, kind) DO UPDATE`: a changed block keeps row
  and uuid (no tombstone, the same block in the exchange). `reset_cover_blocks` deletes row by row (trigger
  `cover_block_uuid_tombstone`) and sets the title area via `book::read`/`book::write` in the same transaction
  (`BEGIN IMMEDIATE`). `Store::cover_layout_fingerprint` = SHA-256 over the JSON of the blocks in kind order
  (`expected_layout` of the requests).
- **Addresses** (`book.rs`): two more text fields in `TEXT_FIELDS` (multi-line, 500 characters), read and written like
  the others; the S1 refusal is gone. R27c wires the form and `work.meta.set`.
- **Requests** (`asset_requests.rs`): `AssetOp::CoverLayout` with `params` `{blocks, expected_layout}`; on the request
  the blocks are checked and normalised (so the duplicate comparison over `params` finds equal requests),
  `expected_layout` empty = the store enters the fingerprint; a request that would change nothing is refused.
- **Server `cover_layout.rs`**: `parse_blocks` reads blocks field by field from JSON (errors with kind and field;
  serde alone does not name the field for a wrong type), RPC as in `docs/RPC.md` „Anordnung“. Commits through a
  `commit` helper of its own (pattern `cover::commit`, plus the trailer of a confirmed request like
  `mirror::AssetBy::Confirmed`), so `mirror.rs` stays untouched. For the body "before → after" of a block saved for
  the first time, `defaults` typesets the cover (`build_wrap_for`, cache of the typesetting) and takes `default_blocks`
  – outside the write lock; without a page size "neu (…)". `decide_request` compares the fingerprint under the write
  lock, otherwise `stale`; a block that is invalid at confirmation (`Asset`/`Refused`) makes the request `stale` as
  well. `asset_requests::request_json` shows one line per block for `cover_layout` (`block_lines`) and points to the
  cover page.
- **MCP `mcp/cover.rs`**: `cover_layout` = `layout_view`, on `REFUSED` (no page size) the stored blocks with
  `geometry: null`; `cover_layout_set` fills missing fields from the effective block and creates the request.
- **Exchange minor version 3** (`baruch-exchange`, `model.rs` this time by umschlag-daten): `FORMAT_VERSION = 3`,
  `ExchangeCoverBlock` (+ enumerations with German XML words), `ExchangeBook.publisher_address`, `printer_address`,
  `cover_blocks`, `ExchangeWork.sheet` (`ExchangeSheet`), tombstone kind `CoverBlock` (`umschlag-block`,
  `since_version` 3). Reader: new parts only from `version >= 3` (like `<karte>` from 2), otherwise `unknown`. Check in
  `read/check.rs` (`check_cover`). XSD `docs/schema/baruch-werk-1-u3.xsd` = u2 plus `Satzbild`, addresses (optional in
  the schema, so that u2 files are valid), `Umschlag`/`UmschlagBlock`, `GrabsteinArt` `umschlag-block`; u2 and v1 XSD
  frozen (SHA-256 test). Store: `exchange_dump` (blocks, addresses, `work_sheet`; blocks without `work_book` → book
  details with defaults), `exchange_apply` (`new_cover`, `merge_cover`: local wins per kind, tombstone → „hier
  gelöscht“; `merge_sheet`; typeset view from the file only after `SheetSettings::normalized`, otherwise „Nicht
  übernommen“). The sample work `sample.rs` carries three blocks, addresses, typeset view settings and a tombstone
  `umschlag-block`.
- **Tests:** store `tests/cover_blocks.rs`, `exchange_apply.rs`, `exchange_dump.rs` (round trip in the store,
  reconciliation), exchange `tests/{read,write,xsd}.rs`, server `tests/round27_daten.rs` (RPC, commits, MCP request up
  to confirmation/staleness/refusal, two instances with XSD check).

### 27b – Typesetting (umschlag-satz)

- **Blocks in Typst** (`baruch_typst::cover`): with a non-empty `layout`, `frame_parts` and `wrap_source` set, for
  each visible block with content, `place(top + left, dx, dy, block(width, {set text(size, fill, weight|style, font);
  set par(leading, justify, first-line-indent: 0pt, …); set align(…); Inhalt}))`; `dx/dy` = trim edge of the face
  (`CoverGeometry::face_origin`, in the book the page itself with margin 0) + (x, y), with sign. Spine:
  `rotate(±90deg, reflow: true, …)`, the top left corner of the rotated frame at (x, y). Shadow =
  `fill: black.transparentize(50%)` with `outset`, gradient = vertical gradient 0 → 35 % → 0 % black; `outset` does
  not change the position. Backgrounds (image, dark areas) stay; the old title block, the old back and the spine label
  are dropped. User texts and font names only through `typst_string`; colours only `white`, `DARK` or
  `rgb("#rrggbb")` after checking.
- **Default blocks** (`cover_layout::default_blocks`): imitate the old picture (author/title/subtitle at 10 % margin in
  the title area, spine text from 6 mm with 0.45 × spine width, at most 11 pt; back with 14 mm margin, barcode bottom
  right), all values within the CHECKs of 0045. `effective_blocks` mixes stored and default blocks. `wrap_source` adds
  missing kinds itself (it knows the dimensions), `frame_parts` sets only the blocks that the server passes – the
  server completes them beforehand with the **page size of the work template** (template plus `#hide[x]` typeset,
  first page measured, cached per state), because the frame is created before the book typesetting.
- **Byte-identical without a layout**: golden states from state S1 before the first change – `frame_parts`/
  `wrap_source` in `baruch-typst/tests/data/cover_r26/` (ten and six cases), state of the typesetting source and
  SHA-256 of the cover PDF for six cases in `baruch-server/tests/round27_satz.rs` (none, front matter, both, cover,
  spine below/above 6 mm).
- **Heights** (`server::cover::block_heights`): `cover::block_measure_source` sets one page per block
  `width = Blockbreite, height: auto, margin: 0pt`; the page heights (`baruch_render::page_sizes_pt`, from the SVG
  head) are the block heights in reading direction. Blocks without content: 0. Measured with the resolved fonts.
- **Fonts** (`baruch_render::font_families`): `FontStore` remembers the number of embedded fonts (they come first);
  per family `normal` (upright < 600), `bold` (upright ≥ 600), `italic` (italic/oblique).
  `server::cover::resolve_fonts` replaces a missing font: in the book by nothing (the template applies), in the cover
  PDF and when measuring by the font of the work template (`baruch_wysiwyg::template_font`: first font from the last
  `#set text(font: …)` at top level as with `sheet_style` and in Typst; comments, raw text, `#show …: set text(…)`,
  `#show: …` bodies and function bodies do not count), if present. So blocks stand in the same font in both PDFs.
- **Server** (`cover.rs`, area typesetting): `build_wrap` = `build_wrap_for(None)` = stored layout; the cover PDF and
  `cover_source` use it. `frame` reads `cover_layout` and `template_functions` of the work template.
- **Front matter from the work template**: `template_functions` (detection like `ribbon::defined_functions`, a copy of
  its own, only `#let name(` with parenthesis). With at least one function (and front matter switched on),
  `frame_parts` writes `book_dict` once after `FRAME_MARK` (`#let baruch_buch = (…)`, 23 keys by contract 1.6 plus
  `impressum_zeilen`, only string literals; the test reads the dictionary line by line and allows only literals) and
  puts `plain_page("", "name(baruch_buch)")` in place of the built-in page. `imprint_template()` (four functions that
  set the same as the built-in pages – text per page compared with `pdftotext`) and `imprint_template_missing()`
  (appends only what is missing, otherwise `IMPRINT_COMPLETE`).
- **Imprint**: `imprint_lines` takes the addresses only if they are set (publisher after the © line, printer after
  „Druck:“) – byte-identical without addresses.
- **Print check** (`server::cover_print`): `cover_checks` only with a layout of its own (stored or preview), rectangles
  from position and measured height (spine rotated), codes `bleed`, `safety`, `barcode`, `spine`, `contrast` (mean
  sRGB colour from 40 × 40 samples in the raster image, illustration `fit: "cover"` on the front including bleed as in
  `wrap_source`, darkening 50 % or 30 %, WCAG ratio < 3), `font`; plus `imprint_checks` (only with front matter). The
  typesetting page shows the imprint warnings in `#druckpruefung` (`typeset::imprint_warnings`).
- **Position measured** (`baruch-render/tests/cover_layout.rs`): title and blurb with `pdftotext -bbox` in book and
  cover PDF, deviation relative to the trim edge ≤ 0.5 mm; spine `down`/`up` (order of the words from top or bottom,
  words in the spine). With a stored layout through the server, the acceptance measures it
  (`tests/round27_abnahme.rs`).

### 27c – Editor (umschlag-widget)

- **Page** `GET /werke/{id}/umschlag` (`src/cover.rs`, areas "pages" and "cover editor"): `editor_state` fetches
  `cover_layout::layout_view` (stored) and, with query fields `b.<kind>.<feld>`, a second view with the preview on top
  (`blocks_from_fields`: for each named kind the effective block, the fields applied to it, checked as in the store
  with `check_cover_block`); the typesetting of the preview runs through `build_wrap_for` with the stored layout,
  replaced per kind by the preview (as in `layout_view`), the SVG below carries the same query. Nothing is stored.
- **Position** (`block_box`, contract 1.2): frame in the whole cover = origin of the face
  (`CoverGeometry::face_origin`) + (x, y); width × height = `width_mm` × `height_mm`, rotated on the spine
  (`height_mm` × `width_mm`). The template gets all positions as percentages of the cover size, so the page needs no
  calculation in the browser without JavaScript. **Guides** (`guides`): bleed (outer edge), trim edge, safety margin
  per face (only where the face is wider than twice the margin), two spine edges, middle of front and back and of the
  height.
- **Forms** (`cover::writes()`, behind `guard::same_origin`): `POST /werke/umschlag/anordnung` (`layout_act`:
  `save` → `cover_layout::layout_set` with all blocks of the form, `quick` → `layout_reset([front], area)`,
  `reset` → `layout_reset(faces)`) and `POST /werke/umschlag/impressum` (`imprint_act` → `imprint_adopt`). Success →
  303 to `safe_back` with `ok=…` (before an anchor, `with_code`); the cover page reads `hinweis` and `ok`. Invalid →
  422 and the same page with the input (numbers with a point, unreadable values as entered) and the message.
- **RPC** in the `call` part: `cover.imprint_adopt` (read the template, `imprint_template_missing`, then
  `works::call("work.update", {template})` – lock, template revision and commit stay there); `work.cover_pdf` appends
  `checks` from `cover_print::cover_checks(work, None)`; `work.meta.set` passes `publisher_address` and
  `printer_address` through to the store (`field_label` for commit and user interface).
- **Widget** `web/src/cover_editor.ts` (`data-widget="cover-editor"`): reads `data-layout` (JSON of `cover.layout`),
  takes the frames and fields rendered by the server and keeps both equal (`place`). Dragging with pointer capture,
  width via the handles `start`/`end`, snapping (`snapDelta`, SNAP_MM 1.5 mm) to trim edge, safety margin and middle of
  its own face, keyboard (STEP_MM 0.5, with Shift 5; Ctrl+arrow width; Ctrl+Z, Ctrl+S), undo as a stack of block
  states, „Hell/Dunkel“ for the selected block, delayed preview via `umschlag.svg?b.…`, saving via `cover.layout_set`
  only with the changed blocks (whole blocks without `stored`/`height_mm`), quick choice and reset via
  `cover.layout_reset` with reload. Frames carry `aria-label` (kind, face, values), the status line is `role=status`.
  Style in `static/cover.css` only with the colour tokens from `app.css` (light and dark).
- **Test** `tests/round27_widget.rs`: page, preview by GET (store unchanged), forms (save/quick/reset, 422, 403),
  addresses, imprint button, `checks`; the widget runs in its own Node mock of the DOM on the HTML of the page
  (dragging, handle, arrow keys, snapping, undo, light/dark, field, saving). Since the merge the tests check only the
  effect; a placeholder „noch nicht umgesetzt“ would make them fail.

### Integration and acceptance (integrator)

- Merged with `--no-ff` in the order a → b → c without conflicts. Updated: the book template in `baruch-store` (unit
  test in `works.rs`, `tests/works.rs`: new books carry `imprint_template()` and know all four front matter
  functions), `round27_daten.rs` (a block without content has the measured height 0), the placeholder branches in
  `round27_widget.rs` (removed) and the commit body when reading an exchange (`mirror::describe_exchange` counts the
  cover blocks too). After the visual check: frames in `static/cover.css` only faintly tinted now (the marked block
  hid a light title on a dark image), and the list of the print check in `templates/cover.html` does not name the
  block twice (the messages already begin with it).
- **Acceptance** `tests/round27_abnahme.rs`: an invented work (148 × 210 mm) with a PNG cover (dark upper third,
  created in the test) and complete book details with Typst special characters. Title into the upper third, author
  down, blurb moved and narrower, free text on the front: preview by GET stores nothing; the form stores the same as
  `cover.layout_set` on a second instance; preview SVG and typesetting source of the preview (`baruch_render::stand`)
  equal the stored cover; `pdftotext -bbox` measures title, free text and blurb in cover and book PDF at the same
  place relative to the trim edge (≤ 0.5 mm). Print check (bleed, barcode, contrast, missing font with fallback) on
  the page without JavaScript and in `work.cover_pdf`. Quick choice top/middle/bottom and reset with tombstones;
  afterwards book and cover typeset byte-identically as before the first layout. An own `impressum(buch)` in the work
  template on the back of the main title with the book details literally; the button adds the other three functions
  (one template revision, one commit) and refuses afterwards; a missing printer is warned about. MCP reads and only
  makes a request; the exchange (`.baruch`) carries layout, addresses and typeset view settings to a second instance,
  which typesets the cover from the same source.

## Findings (Round 28)

Interface R28-S1 (integrator): migration 0046 (`finding`, `finding_status`, `post_version`, columns `post.edited` and
`chat_thread_post.edited`, tombstone kinds `finding`, `finding_status`, `post_version` with cascade triggers like
0042/0045), store `src/findings.rs` (types, reading queries and `finding_insert` in full, status paths as stub) and
`src/post_edit.rs` (reading queries in full, editing as stub), server `src/findings/{mod,parse,rpc,page,view}.rs`,
`mcp/findings.rs`, `mcp/posts.rs`, `PostAction::Edit`, `mirror::describe_finding`/`record_finding`. Open stubs:
`grep -rn "STUB:" crates`.

### Data model, splitting, similarity, mirror, exchange u4 (befunde)

- **Data model (migration 0046):** `finding` (work, section `node_id` – NULL for threads and deleted sections –,
  origin `post_id` or `thread_post_id`, place `position` unique per origin, persona, kind, text, quotation, check note,
  status, reason, revision/target as UUID without foreign key, `similar_to`, `summary_of` as a JSON list of UUIDs),
  `finding_status` (every change; human `proposed = 0`, assistant `proposed = 1` with `decision`), `post_version` (old
  revisions; the editing itself: chat-auftrag). The status rules exist twice: as CHECKs in the database and as a check
  in `baruch_store::findings::resolve` (understandable message, `StoreError::Finding` → -32602). The store resolves
  revision and target via the UUID when reading (`adopted_rev`, `deferred`); if the revision is missing, the UUID
  stays and `adopted_rev` is `None`.
- **Write paths (store):** `finding_set_status` (human only, one transaction: finding + history entry; same state →
  refused), `finding_propose_status` (persona/MCP only, only adopted/deferred/done, no duplicate open; the finding does
  not change), `finding_decide` (confirmed/refused; stale if after the proposal an entry of the human or a confirmed
  proposal came, the revision or the target is missing, or the finding already has the status). Server
  `findings/rpc.rs`: `set_status`, `propose`, `decide` hold `write_lock` from writing to the commit, exactly one commit
  via `mirror::record_finding` (`describe_finding`: subject „Befund <id>: …“, trailer `Baruch-Finding`,
  `Baruch-Action: finding-status|finding-propose|finding-decide`, `Baruch-Node`, for proposals `Persona`, for
  decisions `Baruch-Proposer`).
- **Splitting (`findings/parse.rs`):** line by line; the structure „Was trägt/fehlt/stört“ sets the kind of the
  following items, markers take precedence, list items and (under a structure) paragraphs are one finding each,
  indented continuation lines and sub-items belong to it, an empty line closes it. „Kritisch:“ after a short part
  reference is recognised by `assist::prompts::critical_finding` (the same rule as the check). Check notes
  (`UNBACKED_NOTE`, `ref_claims::IN_TEXT`/`INVALID`) move to `quote_check`, notes by Baruch (`CUT_NOTE`,
  `TRUNCATED_NOTE`, `CRITICAL_MISSING_NOTE`, „Komprimiert: …“) are dropped. `ingest` never takes the write lock (the
  callers in `assist::save` and `threads::answer` hold it), writes only via `Store::finding_insert` and skips places
  that already exist for this post (repeatable). No commit.
- **Similarity:** text only (`parse::similar`): content words, lower case, as sets – words of three characters and
  more without `STOPWORDS` (articles, prepositions including „zur“/„zum“, pronouns, auxiliary verbs, particles), plus
  every number –, Dice measure from `SIMILARITY_THRESHOLD` = 0.8 (or the same text): with three or four content words
  a single differing one already separates (0.67 or 0.75), only from five on may one differ. Before (0.6, with filler
  words) „Der Übergang zur Werft fehlt“ and „… zum Hafen fehlt“ counted as equal, and the new finding disappeared as
  attached. Candidates: findings of the same section from other posts that are not attached – decided ones before open
  ones of the same persona. No embeddings (R19a). Limit: this does not detect rephrasings with other words; the human
  can then discard, or the persona gets the decisions framed (chat-auftrag).
- **Backfill (`findings::backfill`):** at the start of `serve` after migration and mirror reconciliation, once per
  database (`store_meta.findings_backfill`), only posts whose author is a handle from `persona` (not „Autor“, not
  `@mcp:…`); aborts on an error without setting the flag (the next start continues, existing places are skipped).
- **Mirror:** `befunde.md` (`findings::mirror_files`, in `snapshot::files` after the ideas) only when there are
  findings; groups in tree order (`snapshot::ordered_currents`), then threads by id, then findings without a
  reference; per finding a heading `[B<id>] Art · Status · Persona`, text as a quotation block, quotation with check
  note, reason, revision or target, attachment, collection, open proposal, origin and history. Only data from the
  database (deterministic).
- **Exchange u4:** `baruch-exchange` (model `ExchangeFinding`, `ExchangeFindingEntry`, `ExchangePostVersion`, words
  `ExchangeFindingKind`, `ExchangeFindingStatus`, `ExchangeDecision`, tombstone kinds `befund`, `befund-status`,
  `beitrag-fassung` since 4), writer (`<befunde>` at the end, `geaendert` and `<alte-fassung>` on posts), reader (only
  from `version="4"`, unknown before; tombstones from 4), check (`check_findings`, rules like the CHECKs), XSD
  `baruch-werk-1-u4.xsd` (u3 frozen). Store `exchange_dump` (references via UUIDs) and
  `exchange_apply::apply_findings` (new: everything with identifiers; reconcile: unknown findings added, known ones
  never overwritten, occupied place → hint, `gleicht` after all findings), `insert_versions` for old revisions. Details
  in `docs/Austauschformat.md`.

### Overview and display (befund-uebersicht)

- **Limit of the lists:** `Store::finding_list` returns at most `limit` (default 500, at most 2000), and then the
  **newest** (`ORDER BY created DESC, id DESC`, reversed: still oldest first) – matrix, decisions in the prompt and
  similarity candidates lose the oldest above the limit, not the youngest. `Store::finding_count` counts with the same
  filter without a limit: `finding.list`/MCP `findings` report `total` (all) and `truncated`, the matrix `truncated`
  (number of all, otherwise `null`) and on the page „Gekappt: …“. `finding.get` loads attached findings and origins
  specifically (`finding_attached_to`, `findings_by_uuid`).
- **Counting rule in one place:** open = `Store::finding_open_counts` (contract §1.5). `findings::view::subtree_open`
  sums these numbers over the tree order (parent before child) – from that draw the outline (`view.rs`,
  `TreeItem.findings`), the editor head (`node.open_findings`), `/gliederung` (`outline.rs`, `entries_with`) and the
  idea map; `finding.matrix` names `open_badge` per row from the same numbers, and without a filter `total.open ==
  open_badge` holds (test `round28_uebersicht.rs`). `menu.badges.open_findings` is their sum.
- **`findings::view`:** `load` fetches once per page the open numbers, all findings of the work (including praise and
  attached ones, at most `VIEW_LIMIT` = 2000) and, per section with adopted findings, `findings_adopted_in` (for the
  revision tree: `adopted_in`, open proposals: `proposed_for`). `card` builds the common picture of a finding for page
  and accordion (`finding_json` plus sign, section, time, attached findings, origins of a collective proposal via
  `summary_of` UUIDs, open proposal in words, revision choice). Status signs: ○ ✓ ✗ → =.
- **`findings::page`:** `build_matrix` is pure (testable); rows = chapter units (nearest ancestor of level ≤ 1),
  groups = parts, „ohne Abschnitt“ at the end. The page has a template environment of its own (`base.html`,
  `befunde.html`, `fragments/findings_panel.html`); the card and form macros stand in `fragments/findings_panel.html`
  and are imported by `befunde.html` (without `findings_panel` in the context the template itself draws nothing).
  Forms go through `findings::rpc::{parse_change, set_status, decide}` (lock and commit there); the page itself takes
  no lock.
- **Accordion:** `view.rs::findings_panel` filters `FindingsView.findings` to the shown section (open or with an open
  proposal, decided, praise); `ViewExtras.findings` fills `view::extras` with `unwrap_or_default` (an error while
  loading leaves the work page standing without findings). The cookie `baruch_accordion=befunde` keeps it open.
- **Idea map:** `MapFilter.findings` (page `befunde=1`, RPC `findings`), `MapNode.open_findings` and
  `MapSection.open_findings` are summed up in `idea_map::build` like the ideas over anchors and ancestors; `svg.rs`
  sets `im-many-findings` from `findings::page::MANY_OPEN` (3) and writes „k offen“ as `<tspan class="im-findings">`
  (DOM contract extended, not changed).
- **Widget `findings`** (`web/src/findings.ts`): status forms via RPC (then `baruch:navigate` or a reload of the
  content), on the page filters without reloading (fetch of the same address, swap of `[data-findings-main]`,
  `history.pushState`), arrow keys in the matrix. Without JavaScript everything stays with links, GET filters and
  forms.
- **MCP** (`mcp/findings.rs`): `findings` via `Store::finding_list`, `finding_propose_status` checks status and fields
  itself and calls `findings::rpc::propose` with `tools::origin_of` (persona only without a token).

### Chat jobs, frame, post revisions (chat-auftrag)

- **Lector as companion** (`assist/prompts.rs`): `CRITICAL_HANDLES = ["nathan"]`; the follow-up demand
  (`answer_call`) and `CRITICAL_MISSING_NOTE` only for Nathan now. The default of the lector consists of `LECTOR_RULE`
  (6: first „Was trägt:“ with a quotation, at most two „Vorschlag:“, critical findings allowed; with the same exception
  for new revisions as `EVIDENCE_RULE`/`CRITICAL_RULE`, because the system prompt does not depend on the mode),
  `BACKING_RULE` (7: duty to back claims, for every role with critical findings) and `COMPANION_RULE` (8–9: chat,
  decisions, RFC); the exegete has `EVIDENCE_RULE` + `BACKING_RULE`, Nathan `CRITICAL_RULE` + `BACKING_RULE`
  (literally as before). The rules are kept short: every character of the role prompt is missing for the chapter in
  the context (`round18_lektor.rs` and `round15_lektor.rs` measure the limits). `mark_unbacked_criticals` applies
  unchanged to all.
- **Frame** (`assist/context.rs`): `decision_lines` reads via `Store::finding_list` the findings of the section or of
  the chapter units with status adopted, discarded, deferred, done (without praise, without attached ones), discarded
  first, at most `MAX_DECISIONS` = 30 (above that an honest closing line), per line `- [B<id>] vom Autor verworfen
  (@x zu Titel): <<<entscheidung „…“ (Begründung: …) >>> – nicht erneut melden`, defused like `framed_line`. Plus
  `DECISIONS_RULE` (and `OTHERS_RULE`) in the system part. With an RFC, `rfc_lines` frames the answers of the others
  (`<<<antwort … >>>`, head `@x [B1] [B2]`) under `RFC_TITLE`, the job gets `RFC_TASK`; the posts of these answers drop
  out of the chat history (not twice). The chat history (`RECENT_TITLE`) stands in the chat and with the writing job
  (`frame.as_order`, job with `AS_ORDER_TASK`). When shortening, answers and decisions are dropped last and never
  silently (`truncated`). `FOREIGN_KINDS` contains `entscheidung` and `antwort`: they do not back a „Kritisch:“
  finding (corpus of the quotation check without them).
- **Chat jobs** (`assist/chat.rs`): `is_rfc` (first word „RFC“ or „@alle“), `is_write_order` (the first word after
  mentions and „bitte“ is an imperative from `WRITE_VERBS`), `rfc_personas` (active personas in their order, the lector
  taken out by `personas::rows_and_lector`). RFC: per persona `run` in mode chat, the answers as `FramedAnswer` (text,
  post, finding ids from `Outcome::findings`), last the lector with `OrderFrame { rfc_answers, rfc_summary: true }`
  (his findings become collective findings via `findings::ingest`). Writing job: `run` in mode `Propose` with
  `frame.as_order`, then under the write lock the short post `order_note(rev)` (no finding marker at the start, yields
  no finding when splitting) and for each `[B<id>]` from message or proposal `findings::rpc::propose(… Adopted { rev }
  …)` (own lock and commit; errors in `notes`) – only if `finding.key == order.key`: `rev` is a number in the section
  of the job, but `resolve` looks for it in the section of the finding; a finding from another section therefore only
  gets a hint in `notes`. Threads (`threads.rs`) use `is_rfc`/`rfc_personas`; `thread_call` takes the framed answers,
  `answer` splits the answer of the lector with `summary = true`. The relay stage of MCP is dropped with an RFC.
- **Post revisions**: store `post_edit.rs` (`post_edit`, `thread_post_edit`: one transaction `BEGIN IMMEDIATE`, old
  revision as `post_version` with `valid_from` = creation or previous change, `edited` = `replaced`; empty, more than
  4000 characters or unchanged → `StoreError::PostEdit`). Server: `posts::edit` (RPC `post.edit`, form, MCP
  `post_edit`) checks under the write lock that the actor is the author (human „Autor“, MCP `@mcp:<name>`), and writes
  exactly one commit via `mirror::describe_post(…, PostAction::Edit, Some(&before), …)`; `threads::edit_post` without a
  commit. Full-text index: trigger `post_fts_edit` (library 0107, `AFTER UPDATE OF body ON post`).
- **User interface**: `posts::visible_range` shows the newest `CHAT_PAGE_POSTS` = 50 (`view.rs` returns
  `post_older`); RPC `post.page` for loading more. Pages `/beitraege` and `/beitrag` (template environment of their own
  in `posts.rs`, templates `posts.html`, `post.html`). CSS: `app.css` limits `.chat` (`max-height`,
  `overflow-y: auto`, `user-select: text`) and switches `.acc[data-section="chat"] .acc-body` from `overflow: hidden`
  to `auto` – that was the blocker for copying and scrolling (without a limited height the area cut off; where
  `::details-content` does not apply, the flex chain did not pass the height on). Building blocks in `chat.css`,
  threads in `threads.css` (`.thread-scroll`). Widget `chat` (`web/src/chat.ts`): `enableCopy` (Clipboard API,
  fallback `execCommand`), quoting (`quoteLine`, single-line), „Neue Beiträge ↓“, „Ältere Beiträge“ via `post.page`;
  widget `chat-page` for the two pages, `threads.ts` uses `enableCopy`. Buttons that only work with JavaScript stand
  in the HTML with `hidden`.
- Limit (honest): a writing job that asks for a **new** section („Erstelle ein Vorwort“) becomes a proposal for the
  shown section; creating new sections from the chat is not implemented (only through the outline or MCP
  `outline_add`) – a leftover for the backlog. Whether copying in a real browser failed earlier on something other
  than the cut-off area could not be reproduced without a browser (team rule: no Firefox); HTML, CSS and script are
  checked.

## Interface of Round 30 (R30-S1)

Common basis of the three tasks of Round 30 (contract §1), after which each fills only its own files:

- Migration `0047_r30.sql` (pure schema, frozen): metadata on `post` and `chat_thread_post` (`basis_rev`,
  `basis_label`, `reply_to`, `repeat_of`, `relay_from`, `request`; references with ON DELETE SET NULL),
  `finding.solutions`, `revision.variant_*` (group and label both or neither, checked in code), table `replace_batch`
  (local log, no uuid). Reserved: store 0048–0051, library 0108/0109.
- Store, fully implemented: `Store::add_post_meta`/`post_meta*`/`thread_post_meta*` (`post_meta.rs`),
  `Store::add_proposal_on`/`variant_group` (`variants.rs`, `write::add_proposal_on`), `Store::finding_set_solutions`,
  `Store::replace_batch`; `replace_apply`/`replace_undo` are stubs (`StoreError::Replace` → RPC `-32000`).
- Server hooks: `assist::save` stores persona posts with metadata (checked revision, job, links from `Order::links`);
  `assist::chat` chooses one basis for all personas (`basis::choose`), calls `variants::is_variants_order`/`run` and,
  after the lector with an RFC, `solutions::rfc_proposal`; `view::posts` attaches `extra` (`chat_render::post_extra`)
  and `solutions` to each post, the revision tree `variant` (`variants::revision_variant`); `mirror::record_request`
  mirrors finished commit messages of the new modules.
- Forwarding of `search.*`, `xref.*`, `solution.*` in `rpc.rs`, routes in `lib.rs` (writing ones behind
  `guard::same_origin`), MCP `text_search` and `solution_variants` (stubs, `solution_variants` counts towards the
  quota), menu entries under tools, empty template macros (`search_bar`, `post_item`, `finding_block`,
  `post_solutions`, `basis_select`) and TypeScript modules (`search`, `search_keys`, `jump`, `xref`, `variants`).

## Search and replace (Round 30a)

- **Server `src/search/`:**
  - `mod.rs`: types (`SearchQuery`, `SearchHit`, `SearchResult`, `TextKind`, `SearchRange`, `RevChoice`), ranges
    (`sections_in`: section, chapter = nearest chapter ancestor including sub-items, part, from section, selection,
    work, all works), text per section (current or shown revision, unsaved `text`, selection in UTF-16),
    `find`/`find_within` (time limit as a parameter for tests) and RPC forwarding.
  - `scan.rs`: text kinds per byte (`classify`): heading titles, footnote content, quotations via
    `crate::quotes::quote_spans`, code (`#name`, arguments, keyword lines, raw, formulas, comments, labels,
    `@verweis`), otherwise text. A hit across several kinds counts as the strictest (code > quotation > footnote >
    heading > text). Plus the pattern (`pattern`: crate `regex`, `size_limit(1<<20)`, `dfa_size_limit(1<<22)`; in
    wording mode every special character also finds `\\x`), hits with replacement (`find_in`; regex:
    `Captures::expand`, wording: `escape_literal`) and UTF-16 conversion.
  - `replace.rs`: `preview` (diff per section as HTML lines with `<del>`/`<ins>`, everything escaped, `body_after`),
    `replace` and `undo` under the write lock; one commit each via `mirror::record_request` with
    `Baruch-Action: replace` or `replace-undo` and `Baruch-Replace: <id>`.
  - `page.rs`: `GET /suche`, `POST /suche/act` (template environment of its own, `templates/search.html`).
- **Course of a replacement:**
  - The server searches once more on the current revision. The order of the hits is the same as in the preview,
    `skip` counts in it.
  - It computes the new texts and passes them with the basis from the preview to `Store::replace_apply`.
  - The store checks in **one** transaction per section under a `SAVEPOINT` of its own: if the current revision is
    still the basis, a new revision by the human is created (`write::save_human`), followed by
    `outline::check_current_heading`. Another level, further headings or none → only this section is rolled back and
    skipped with a reason.
  - At the end it writes one row `replace_batch` (options as JSON, `items` = `[{key, prev_rev, new_rev}]`).
- **Undo:** `Store::replace_undo` switches back per entry with `write::set_current` + `outline::follow_switched`, only
  if `new_rev` is still current, and sets `undone`. `Store::replace_latest` returns the youngest open replacement for
  `/suche`. `replace_batch` is local: no uuid, not in the exchange format.
- **Widgets:**
  - `web/src/search_keys.ts`: pure function `keyAction(e, ctx)`, checked under Node.
  - `web/src/search.ts`: search bar `search-bar` from `fragments/search_bar.html`, work sidebar, listeners on the
    document in the capture phase. Hits in the typeset view via the CSS Custom Highlight API (`baruch-search`,
    `baruch-search-current`). Replacement in the section goes through `search.preview` with `text=` and changes only
    the text field.
  - Style `static/search.css`.
- **MCP** `text_search` (`src/mcp/search.rs`): `crate::search::find` with range `section|chapter|part|from|work`,
  default 100 hits, no write path.

## Cross-references (Round 30a)

- **`baruch_typst::xref`** (pure):
  - `label_heading`/`label_section` project the jump mark: ` <kN>` at the end of a `=` line before a comment; with a
    label of its own (part, „ohne Nummer“, label at the end of the line or alone below it) the anchor line
    `#metadata("baruch-key") <kN>` below it.
  - `strip_key_label`/`strip_section`/`strip_key_labels` remove them again.
  - `find_xrefs` finds `#link(<kN>)[…]` including the page number supplement, `remap_keys` rewrites keys,
    `xref_snippet` builds the form.
  - `part_anchors_for_typesetting` serves only typesetting with part pages, see below.
- **Store:**
  - `snapshot::export_parts` and `snapshot::files` insert `label_section` after the projection „Nummer an/aus“. So
    export, export map (byte-identical), mirror and typesetting carry the jump marks, SQLite does not.
  - `import.rs` removes them per heading (`strip_section`), first creates all nodes and then writes the texts with
    `remap_keys` (old → new `node.id`). The checksum against a double import stays that of the file.
- **Anchor of a part in typesetting (measurement `baruch-render/tests/xref.rs`):**
  - The part rule (`part_page_rule`) breaks the page after the part. An anchor line after the part line would
    therefore lie on the following page, and `counter(page).at(<kN>)` would name the wrong page.
  - `typeset::part_pages` therefore inserts `part_anchors_for_typesetting` when typesetting. The anchor line moves
    before the part line, with its own `#pagebreak(weak: true, to: "odd")`. The break of the rule is then dropped,
    because the page carries only the anchor (same page number measured).
  - Export and mirror keep the anchor line below (without the part rule it lies on the page of the heading).
    `typeset::rewrite_sections` updates the line count and byte ranges of the map.
- **Broken references:**
  - `xref::broken_in` reports references to keys without a current revision.
  - `typeset::xref_safety` turns them into plain link text when typesetting; otherwise the whole typesetting would
    fail with "label does not exist".
  - The print check (`typeset.html`, `li.typeset-xref`) names them.
- **baruch-wysiwyg:**
  - `Inline::Xref {key, c, page}`; `parse` recognises `#link(<kN>)[…]` including a supplement with the same key.
  - `emit` writes `xref_snippet`, `render_html` a `span.ws-xref` with `data-key`/`data-page` and a non-editable
    `span.ws-xref-page`.
  - `wysiwyg.ts` reads the block back (`t: "xref"`), inserts it via the command `xref` (`ws_command.ts`) and marks
    `baruch:find` with the highlight `baruch-find`.
- **Server `src/xref.rs`:**
  - `jump_url`, `check_marker` (key, quotation with normalised whitespace, `at` = byte of the quotation);
    `outline_mentions` (counting mentions via a pattern with ordinal numbers one to twelve, Roman up to XXXIX, Arabic
    and numbers like 2.3; titles from four characters as whole words with article, shorter ones only in quotation
    marks; unique or not at all, counting only items with a number).
  - `candidates_in` takes only running text (`search::scan::classify`), not inside existing references, not the own
    section.
  - `convert` creates a revision on `rev` via `Store::save_human_on` and mirrors it with `mirror::record`.
  - RPC `xref.*`, pages `/verweise` and `/verweise/act` (template `templates/xrefs.html`).
- **Widgets:**
  - `jump.ts` (without imports; pure functions `pushJump`/`popJump`/`forwardJump`, `byteToUtf16`, `locate`, `findOf`
    checked under Node): `a[data-jump]`, history `sessionStorage` `baruch_jumps`, jump bar, `baruch:find {text, at}`
    after loading and after `baruch:rendered`.
  - `workspace.ts` reports `baruch:rendered` after every redraw; Alt+← goes through `jumpBack()` if there is history.
  - `xref.ts`: dialog via `baruch:xref-open` from `ribbon.ts`, `localStorage` `baruch_xref_page`, Ctrl+click and
    „Öffnen | Bearbeiten“ (menu fixed in the document, not in the typeset view).
  - Ribbon `ribbon::formats` has `cmd = "xref"`.

## Solution proposals and variants (Round 30b)

- **Solution form** (`assist/solutions.rs`): `parse_solutions` reads `statt: „…“` + `besser: „…“`, `Schritt: …` and the
  short form `statt „…“ besser „…“` (also in the middle of a line, also numbered); `is_solution_line` recognises a
  solution line. `findings::parse::split_answer` attaches solution lines to the finding above (the same as indented,
  also after an empty line – only another line closes the finding), takes them out of the finding text and does not
  read the closing block „Mein Vorschlag:“ as findings; `finding_spans` returns the bytes per finding including
  solution lines (for the blocks in the chat, Round 30c). `findings::ingest` stores the solutions per finding
  (`Store::finding_set_solutions`, column `finding.solutions` from 0047); errors there only on stderr, the finding
  stays.
- **Duty to propose solutions** (`assist/prompts.rs`, `assist/mod.rs`): `SOLUTION_RULE` stands in every default.
  `answer_call` asks again once per job after the mandatory finding (Nathan) if `missing_solutions` > 0
  (`Retries { critical, solution }`, reason `SOLUTION_RETRY_REASON`); the second answer counts only if it lacks no more
  solutions and keeps the mandatory finding. `mark_missing_solutions` then puts „(ohne Lösungsvorschlag)“ at the end of
  the first line of every critical item without a solution (notes written by the model are removed and checked anew;
  idempotent). Cut-off answers: neither a follow-up demand nor a note. The note moves into `quote_check` when
  splitting.
- **Adoptable and inserting**: `locate` looks for the quotation literally, otherwise with arbitrary whitespace (regex
  only from escaped words, `size_limit`); `apply_solutions` inserts in order the first occurrence each and reports a
  missing quotation. `post_solutions(body, shown)` is the hook in `view::posts` (`post.solutions`).
- **RPC** `solution.*` (`assist/solutions.rs::call`, forwarded by `rpc.rs`): `adopt` creates via
  `Store::add_proposal_on` (basis = shown revision or the basis of the post `PostMeta::basis_rev`, for a finding the
  current one) a proposal with the origin of the persona and proposes via `findings::rpc::propose` „übernommen in
  Fassung n“ (only findings of this section). `reject_siblings` discards under the write lock the other open proposals
  of the group (`Store::reject`, no mirror commit, like `revision.reject`). `compare` returns per variant the line diff
  to the basis and to the others as escaped HTML (`diff_html`, `baruch_core::line_diff`). Pages: `GET /varianten`
  (environment of its own with `base.html`, `variants.html`; without `key`, `Store::variant_group_key` determines the
  section) and `POST /loesungen/act` (pairs from the form, `picks`/`findings` multiple, 303; errors → 200 with
  `role="alert"`), behind `guard::same_origin`.
- **RFC** (`solutions::rfc_proposal`, hook in `chat.rs`): `proposal_block` reads the solutions after „Mein
  Vorschlag:“; at most three adoptable ones are inserted into the basis, one proposal (note „Mein Vorschlag von
  @lektor (Beitrag n)“); `[B<id>]` in the block yield status proposals.
- **Basis** (`assist/basis.rs`): `choose_in` chooses shown/current/n; "shown" on a current revision with only a
  heading (`heading_only`) takes the newest proposal with text, an explicit "current" stays with a hint in the head
  line. `assist/rpc.rs::order` reads `basis` for ask/review (not propose), `run_inputs` chooses it for jobs without a
  basis (MCP `ask_persona`, threads, ideas) – the address stays without `rev` as long as the current revision is
  checked. `Outcome.basis` (JSON `basis` for ask/review) and `PostMeta::basis_label` carry it. For a revision with only
  a heading, the job says `HEADING_ONLY_TASK`.
- **Context** (`assist/context.rs`): `outline_block` with `[key:N]`; with „Chat nur Abschnitt“ it stands there as
  surroundings, otherwise it (together with `JUMP_RULE` in the system part) is only added if afterwards `OUTLINE_SLACK`
  characters are still free – it displaces no text and triggers no splitting into pieces. Findings of others: the
  lector under `FINDINGS_TITLE`, all others under `KNOWN_TITLE` with `KNOWN_RULE` (do not comment). `OrderFrame.relay`
  stands framed and defused (`relay_line`, kind `antwort`, does not count towards the corpus) with `RELAY_TASK`.
  `RFC_TASK` asks for „Mein Vorschlag:“.
- **Variants** (`assist/variants.rs`): lector by setting, findings named (only of this section) or the open ones of
  the section (without praise, without attached ones), at most `MAX_VARIANT_FINDINGS`; without findings an honest
  refusal. Per variant a `ModelCall` of its own (system: lector prompt, `OTHERS_RULE`, `VARIANT_RULE`; findings with
  solutions framed; direction `VARIANT_STYLES[k]` and the lines already written), one after the other via `ask_model`.
  If the section including the answer does not fit into the context length, only the affected paragraphs (quotation
  of a finding or a solution, never the heading) go out with markers `[[Absatz n]]` (`PARTIAL_RULE`); `splice` inserts
  the answer. Storing happens only at the end under one lock (`add_proposal_on` with `VariantInfo`, group `new_uuid`).
  Chat job: `is_variants_order` (word „Varianten“, no question). Revision tree: `revision_variant` (JSON with `title`),
  labels in `revtree.html`, „übrige verwerfen“ in `revisions.html`.
- **Templates**: `fragments/finding_block.html` (finding as a block: kind with sign and word, status with buttons to
  `/werke/befunde/act`, solutions, „Als Vorschlag übernehmen“), `fragments/post_solutions.html` (solutions of a post,
  „Lösungen umsetzen“), `fragments/basis_select.html` (in `assist_scope.html`). For this the cards
  (`findings::view::card`) carry `body_html` (via `chat_render::markdown_html`), `solution_views`, `adoptable`,
  `work`, `back`; the matrix counts `solved`/`unsolved`.
- **Exchange minor version 5**: see `docs/Austauschformat.md` §6; after reading, `exchange_apply::apply_u5` sets
  variants, metadata and solutions on the newly created rows (ids above the state before the run) and calls
  `baruch_typst::xref::remap_keys` with the mapping `schluessel` of the file → identifier → key here. When reconciling
  (`apply_merge`, step (c)/(d)), Baruch rewrites the keys in the text of a revision **before** comparing it with the
  local one (the same mapping over all items of the work): a revision that differs only in the keys of the
  cross-references is not divergent; a really divergent revision (derived identifier) is created with the local keys,
  because `apply_u5` does not reach it via the identifier of the original revision.

## Chat: retry, delete, address, live, structured posts, jump marks (Round 30c)

**Data.** Per post in the section chat (`post`) and in the thread (`chat_thread_post`) metadata from migration 0047
(`baruch-store/src/post_meta.rs`): `basis_rev`/`basis_label` (checked revision and head line), `reply_to`,
`repeat_of`, `relay_from` and `request` (`PostRequest {persona, mode, scope, text, rev}`, local, not in the exchange
format). `assist::save` writes persona posts in the section via `Store::add_post_meta` (from
`Order::links`/`Order::basis`), `threads::answer` in the thread via `Store::thread_add_post_meta` (one transaction,
`scope = "thread"`, job text = message of the sender). `Store::thread_post_delete` deletes a thread post; references
to it become NULL through the foreign keys (`ON DELETE SET NULL`), history and finding references follow 0046.

**Address and relay** (`assist/chat.rs`): `addressed(message, personas)` = `@handle` anywhere plus name/handle without
"@" at the start of a sentence (start of the post, after a line break, after "."/"!"/"?" with whitespace) with
","/":"; `mentions` stays for the MCP relay and existing code. `line_addresses(answer, personas)` = addresses at the
start of a line of an answer (also after list markers and `**`, not in "> " quotations). `chat()` collects the answers
of the first round and then calls one relay answer per address: `Order { mode: Chat, text: relay_text(..), asker:
anredende Persona, frame.relay: deren Antwort gerahmt, links: {reply_to, relay_from} }`. Limits `MAX_RELAY_ANSWERS`
(3), each persona once (`called` covers all called personas including failed ones), one stage, not with an RFC or a
writing job. Threads (`threads::chat`) use the same functions; the MCP relay from Round 15 (every mention, total limit
3) stays beside it (since the follow-up after R30 in the same stage and framed the same way, see "Relay" under
Round 15), `mcp/tools.rs` `relay_section` afterwards called in the section only personas that had not had their turn
yet (removed since Round 23: one relay path `assist::chat::relay_targets`).

**Retry** (`posts::repeat`, `threads::repeat_post`): job from `PostMeta::request` (fallback:
`chat_render::repeat_fallback` or `threads::thread_fallback` – preceding post of a non-persona author), basis via
`assist::basis::choose`, execution via `assist::run` or `threads::answer` with `links.repeat_of`. Then
`posts::propose_done`: open, non-praising findings of the old post for which `findings::parse::split_answer(neue
Antwort)` returns no `similar` get the proposal `Done` via `findings::rpc::propose` (origin the persona, one commit
each). The text of the new answer is compared, not the newly created findings – `findings::ingest` attaches
repetitions of a finding to the old one (`similar_to`).

**Delete:** section as before `posts::act(Op::Delete)` (one commit `post-delete`); thread `threads::delete_post` under
the write lock with `Actor::only_own` (MCP: `delete_post_as`), commit via `mirror::record_request` with `allow_empty`
(log, the tree does not change).

**Live** (`chat_live.rs`): process-wide tables in memory per (work, section or thread): `typing()` /
`typing_thread()` register and return a `TypingGuard` whose `Drop` unregisters (end, error or cancellation of the
future); `assist::run_inputs` and `threads::answer` hold it during the job. `record_failure()` remembers failed calls
(from `chat::fail`, `routes::remember_failure` for single jobs, `threads::answer`, `posts::repeat`), at most five per
place, ten minutes. `post.since`/`thread.since` render new posts with the same macros as the page
(`fragments/post_item.html`: `post_entry` or `thread_entry`, via `Environment::render_str`), plus `typing_now` and
`failures_now`. `web/src/chat.ts` (`startLive`) polls every 3 s while the accordion is open and the page is visible,
and immediately on `baruch:chat-refresh` (assist.ts at the end of an SSE stream); threads.ts uses the same function.

**Rendering** (`chat_render.rs`): `markdown_html` (signature fixed, also for the findings accordion and the matrix)
splits into blocks (paragraph, list with sub-items, quotation, heading) and a footer (closing notes by Baruch:
`scripture::MARK_*`, `CUT_NOTE`, `TRUNCATED_NOTE`, „Komprimiert:“, `CRITICAL_MISSING_NOTE`); inline only `**`/`*`,
everything via `bibles::html_escape`. `render_post` creates links at byte ranges of the line (order markers →
idea/finding → Bible references → quotations → `xref::outline_mentions`, no overlap) and escapes the rest; jump marks
are checked by `xref::check_marker` (the revision number only for the own section, otherwise the current one), jumps
are built by `xref::jump_url` (`data-jump data-jump-origin="chat"`), Bible references by `baruch_scripture::scan`
(only `Valid`, target `/literatur/bibel?work&ref`, without `module` the page chooses the Bible of the work). Findings:
`findings::parse::finding_spans` returns byte ranges per place; a range becomes `Segment::Finding` if a stored finding
of this place (`Finding::position`) belongs to the post, its card (`findings::view::card`) stands in
`PostExtra::finding_cards` for the macro `finding_block`. The chat calls it as `finding_block(card, seg.html)`
(integrator Round 30): the finding text in the block is then the segment with the checked links from `post_html`, not
`card.body_html` once more – so the text stands there exactly once. `PostExtra::plain` = plain text for „Kopieren“.
`post_extra` (hook from `view::posts`) takes findings and metadata from `ViewExtras`; the pages `/beitraege`,
`/beitrag`, `/faden` and `post.since` load both themselves (`posts::page_extras`, `threads::entry_values`).

**Templates:** `fragments/post_item.html` with the macros `post_item(post)` (head and text, without page context),
`post_entry(post, at)` (whole post of the section chat including ↻/✕/manage, `at = {work, key, node, rev, view, lq,
targets}`) and `thread_entry(e, at)` (`at = {work, thread}`); `assistants.html`, `thread.html`, `posts.html`,
`post.html` include them. The macros `finding_block` and `post_solutions` (chat-loesungen) are imported from within
`post_item.html`.

## Round 29: Typesetting in the settings

Interface R29-S1: data model `baruch_typst::typeset` (`model.rs` frozen: `TypesetSettings`, limits,
`validate`/`normalized`/`changed_fields`/`to_json`/`from_json`, `FIELDS`), rule, template, preview and EPUB functions
as stubs (`rules` returns ""), store API `baruch_store::typeset` (reading without a row → `None`, writing →
`StoreError::Typeset` „noch nicht umgesetzt“), hook in `snapshot::head` (rule block after work template, schema and
fallback rules; byte-identical without settings), server modules `typeset_settings` (RPC `work.typeset_*`),
`mcp::typeset` (tool `typeset_settings`, read-only), `settings::work_typeset` (block „Satz“, routes,
`work.typeset_preview`) and `typeset::cached_page`. Reserved: store 0048 satz-daten, 0049 satz-typst, 0050
satz-oberflaeche, 0051 integrator; library 0108 satz-daten, 0109 integrator.

### Data (satz-daten)

- **Migration 0048** (`crates/baruch-store/migrations/0048_work_typeset.sql`, the only new store migration of the
  round; 0049–0051 and library 0108/0109 stay unused): table `work_typeset (work_id PRIMARY KEY → work ON DELETE
  CASCADE, uuid, settings, updated, updated_by)` – at most one row per work, no row = no typesetting settings.
  `settings` is the canonical JSON of `TypesetSettings::to_json` (CHECK: JSON object, at most 16384 characters; the
  exact check is done by `TypesetSettings::validate` in code). `uuid` like 0045/0046 (unique index, fill trigger
  `work_typeset_uuid_fill`, tombstone trigger `work_typeset_uuid_tombstone`, cascade trigger
  `work_typeset_cascade_uuid_tombstone` before deleting a work, so that the tombstone knows the work uuid).
  `uuid_tombstone` gets the kind `work_typeset` through the rebuild like 0046 (drop sixteen tombstone triggers,
  temporary copy, table with extended CHECK, triggers literally anew); no `FOREIGN_KEYS_OFF`, because no foreign key
  points to `uuid_tombstone` (evidence in `tests/migration_0048.rs`).
- **Store** `baruch_store::typeset` (contract §4): `work_typeset` (read; unknown work → `UnknownWork`),
  `set_work_typeset` (normalise, check – invalid → `StoreError::Typeset` with all messages, nothing changed; same
  values → no write; empty settings = reset; changing by upsert, the uuid stays), `reset_work_typeset` (delete the row
  → tombstone), `adopt_typeset_rule` (settings and a new revision of the work template including
  `work.template`/`work.updated` in one transaction `BEGIN IMMEDIATE`; template with a heading or unchanged →
  `Refused`) and `typeset_in` for the hook in `snapshot::head`. There the rule block stands after work template,
  schema and fallback rules – and so equally in export, export map, mirror (`main.typ`), typesetting preview, PDF and
  cover; without settings the export is byte-identical to state R29-S1 (golden
  `crates/baruch-store/tests/data/satz_golden_*.typ`).
- **Server** `crate::typeset_settings`: `load`/`view_json` (common for RPC, MCP and the block „Satz“; hint about an
  unknown base font via `baruch_render::font_families`), `save`/`reset`/`adopt_template_rule` under the write lock
  with exactly one mirror commit per change (`mirror::describe_typeset`/`record_typeset`, trailer
  `Baruch-Action: typeset-set|typeset-reset|typeset-adopt`, always `allow_empty`); RPC `work.typeset_get|set|reset|
  adopt_rule` (`docs/RPC.md`), MCP `typeset_settings` read-only (`crate::mcp::typeset`, `docs/MCP.md`).
- **Exchange** minor version 6 (`docs/Austauschformat.md`, section „Satz-Einstellungen“): `ExchangeTypeset` in the
  model (with `TypesetSettings` from `baruch-typst`, which is why `baruch-exchange` now depends on `baruch-typst`),
  words per field in `baruch_exchange::typeset`, tombstone kind `satz` ↔ `work_typeset`, XSD
  `docs/schema/baruch-werk-1-u6.xsd`. `exchange_dump` writes the row with identifier and last change; `exchange_apply`
  creates it on a new import, and when reconciling only if it is missing here and no tombstone exists – otherwise a
  hint with both states, never overwrite. The exchange page counts it (`counts.typeset`, `typeset_fields`), the
  preview of the import names „Satz-Einstellungen“ in the list of pieces being added.

### Typst rules (satz-typst)

- **Rule block** `baruch_typst::typeset::rules(&s)` (`rules.rs`): "" without settings (also if only `xref_with_page`
  is set – the switch changes no file), otherwise the line `// Baruch: Satz`, then one line per field in `FIELDS`
  order, ending with `\n`. The store appends it in `snapshot::head` after work template, schema and fallback rules;
  export, export map, mirror, typesetting preview, book PDF and cover thereby see the same rules.
  `chapter_start` → show rule `#show heading.where(level: 1): it => { pagebreak(weak: true[, to: "odd"]); it }`, that
  is exactly the adoptable template rule; it also applies to part headings (`<baruch-part>`, level 1 in the export), so
  that „Vorlagenregel übernehmen“ does not lose the break before parts in the export (finding R29, test
  `adopting_the_template_rule_keeps_every_heading_page_in_the_export_form`). In typesetting the part rule takes effect
  before it; `continuous` is only a comment line. `page_format`/`margins` → `#set page(width/height)` or
  `margin: (inside: innen + Bundsteg, outside, top, bottom)`; font/size/line spacing/justification/hyphenation/
  language/widows → `#set text`/`#set par`; indentation → `first-line-indent` and paragraph spacing = line spacing
  (Typst default 0.65 em, otherwise `leading_em`); footnotes → `#set footnote(numbering: "1"|"*"|"a")` and per chapter
  `counter(footnote).update(0)` in a show rule.
- **Page numbers and running heads** are `context` headers/footers with the helper functions `baruch-satz-chapters`,
  `baruch-satz-opening(p)` and `baruch-satz-quiet(p)` (part and blank pages). „Ab dem ersten Kapitel“ sets
  `counter(page).update(1)` into the **page background** of the first chapter page: Typst puts everything a show rule
  returns between the start and end tag of the heading; a reset there would come too late for
  `counter(page).at(<kN>)` (cross-reference „S. n“). The background stands in the page frame before header and content
  (measured in `baruch-render/tests/typeset_measure.rs`). `pagebreak(to: "odd")` and `inside`/`outside` count physical
  pages.
- **Detecting blank pages:** a show rule puts the tag `<baruch-satz-end>` before every heading of level 1 – before the
  break of the setting or of the template. Typst leaves closed tags before the break where they are and moves only
  unclosed start tags (`migrate_unterminated_tags`); pages between tag and heading are blank. If only the tag stands
  before the break, Typst puts it at the start of the following page (position 0 pt) – that counts as blank too. The
  blank back of a part page carries `<baruch-satz-blank>`. The part rule of the typesetting takes effect before this
  show rule and does not return `it`; it therefore sets the tag itself as soon as there is a rule block
  (`part_page_rule_for`, also without the setting `part_page` – finding R29: otherwise the blank page before a part
  carried page number and running head; test `the_blank_page_before_a_second_part_stays_empty`). Without typesetting
  settings the rule of Round 14 stays byte-identical. Measuring tests detect blank pages by the empty type area, not
  by a completely empty page (otherwise a blank page with a wrong number would pass as a text page).
  Without a chapter rule there is no break: the tag then stands at the same place as its heading and follows it in the
  query; such a tag does not count (integrator R29 – before, all pages up to the next heading counted as blank, without
  page number and running head; test `without_a_chapter_rule_every_text_page_keeps_number_and_head`).
- **Part pages:** `typeset::prepare` reads `state.store.work_typeset(work)` and sets `part_page_rule_for(labels,
  &settings)`: with `part_page` = `part_page_rule_with(labels, Some(part))`, with a rule block without `part_page` =
  right-hand page without a blank back (as in Round 14) including the tag `<baruch-satz-end>`, without typesetting
  settings byte-identical to `marker::part_page_rule` (state and page number as before Round 29, golden state in
  `tests/round29_typst.rs`).
- **Work template** (`template.rs`): a copy without comments is read (same byte positions). `template_conflicts`
  reports per set field the arguments of `set page|text|par|footnote(…)` (also in the code of template functions) with
  line and text, effect `overrides`; show rules on headings of level 1 with `pagebreak` against `chapter_start` are
  `ineffective`, unless the template wants the same or only a new page with "right-hand page" (Typst applies the show
  rule defined first last, so its break is added). `detect_chapter_break` recognises only rules that consist solely of
  break and heading (whitespace, multi-line, code or content block, with/without `weak`, `to: "odd"` → right-hand page,
  without `to` → new page, `to: "even"` → not adoptable), so that `remove_rule` removes nothing else.
- **Typesetting page:** `typeset::template_hints` returns conflicts and the recognised rule for `#druckpruefung` (link
  `/einstellungen/werk?work=W#satz`).
- **Cover:** `cover::build_wrap` measures the first page of the book typesetting (that is the typesetting format) and
  counts all pages including blank pages for the spine. For the cover pages in the book, `cover::frame` measures the
  work template including the rule block (`cover::page_template`); blocks without a font take `cover::book_font`
  (setting, otherwise work template), as do `cover_layout::default_font` and the print check of the cover.
- **Preview** `preview_source(s, template, title)`: template, rule block, part rule (`part_page_rule_for`) and an
  invented sample text (one part, three chapters with sections and one footnote each, ≥ 6 pages A5).
- **EPUB** `epub_style(&s)` (`epub.rs`): CSS for `section.baruch-chapter`/`section.baruch-part` (breaks), `p`
  (indentation or spacing, justification, `hyphens`, `widows`/`orphans`), `body` (font, size relative to 11 pt, line
  height), `ol.baruch-footnotes` (numbering); `split_chapters` always `true`, `lang`. Not wired anywhere yet (R20a).
- **Measuring aid** `baruch_render::page_texts(&Source) -> Option<Vec<Vec<PlacedText>>>`: per page the text pieces
  (text, baseline left in pt from the top left, width, size, font) with the transformations of the groups. Typst sets a
  hyphen as a piece of its own with the text U+00AD; punctuation hangs ≈ 1.9 pt into the right margin.

### User interface (satz-oberflaeche)

- `src/settings/work_typeset.rs`: block „Satz“ (`block` → template `templates/settings/werk_satz.html`, context from
  `block_context`, which needs only `typeset_settings::TypesetView`, the work template and the refused form – so it
  can be drawn in a unit test without a server), forms `act`/`adopt_act`, preview images `preview_svg`, RPC
  `work.typeset_preview` (`call`, shared with the form preview via `preview`).
- Form fields ↔ settings: `settings_from_form` (contract §6.3) and the inverse `form_from_settings` (round trip test
  with the contract example). No limit check of its own: numbers that are not numbers become `NaN`, so that
  `TypesetSettings::validate` refuses them with the same message as via RPC; only "unknown choice" and incomplete
  margins are added. Messages are `(field id or subfield, text)`; the template shows them at the field (head before
  the point), and so it does with conflicts from `template_conflicts` (`data-typeset-error`, `data-typeset-hint`).
- `Retry` (in `WorkRetry::Typeset`) carries input, messages and – after `action=preview` – the `Preview`. Refused forms
  and the preview without JavaScript use the same path: `settings::work_retry_page` draws the whole page „Werk“
  (status 422 or 200), the block then shows the unsaved values, their rules and conflicts.
- Preview: `preview_source` + `export_assets` → `typeset::compile` (lock `RUNS`, `COMPILE_TIMEOUT`, puts the state
  into the cache); `preview_svg` reads only `typeset::cached_page` and never starts a run (protection against
  typesetting runs at the call of other pages: `guard::cross_site_get` → 403).
- Widget `web/src/typeset_settings.ts` („typeset-settings“ on the form): `formToSettings` reads the form like
  `settings_from_form` (a test under Node compares both), `debounce` (mockable timers) and `Newest` (AbortController;
  late answers are discarded). The button „Vorschau“ typesets immediately without submitting;
  „Speichern“/„Zurücksetzen“ stay forms (in the popup `settings.ts` submits them).
- `web/src/xref.ts`: `initialPage(vorhanden, Werk-Einstellung, Browser)`; the dialog fetches `work.typeset_get` and
  takes `settings.xref_with_page` as long as the checkbox is untouched; `localStorage` (`baruch_xref_page`, try/catch)
  only when the work setting is unset.
- Style `static/typeset_settings.css` (in `STYLE`). Return notices `ok=satz|satz_zurueck|satz_vorlage` in
  `settings/work.rs` (`notice`).

### Integration (Round 29)

- Merge `team/satz-daten` → `team/satz-typst` → `team/satz-oberflaeche` with `--no-ff`, without conflicts.
- Acceptance `crates/baruch-server/tests/round29_abnahme.rs` (invented „Fährhausbuch“ with front matter, two parts,
  four chapters, footnotes and a cross-reference with page number): form → `work_typeset` → hook `snapshot::head` →
  typesetting and book PDF from the same source (same state), measured with `baruch_render::page_texts` and
  `pdfinfo`/`pdftotext` (A5, chapters on the right, page numbers outside from the first chapter on every text page,
  running heads, gutter, „S. n“), cover page size and spine, every setting individually through the server, adoption
  of the template rule (chapter pages before = after), conflict `#set page(paper: "a4")`, MCP read-only, exchange u6
  with a tombstone over three instances.
- `typeset_settings::load` additionally reports `NO_DOCUMENT_TITLE` when the running head „Werktitel“ is chosen but
  the work template has no `#set document(title: …)` (the running head would stay empty).
- The exchange page (`templates/exchange.html`) names the typesetting settings (`counts.typeset_fields`).

## Round 19 – RAG maintenance, rest of the references

### R19a index

Without a migration (prefix in the JSON of the connection, lock note and marker in `sync_state`). Details under
"Semantic search (Round 12, migrations 0031 and 0032)" (service) and in the section on syncing (`sync/`):

- `rag/index.rs`: `Runtime` with `enqueue` (loud, fetches back what was left lying) and `enqueue_quiet` (search),
  `stale()`, `wish_reset`/`reset_wished`/`apply_reset`, `unbound`; `process` → `Outcome {embedded, follow}`; `sync`
  with range `replace`, prefixes and cancellation; `PREFIX_CHANGED`.
- `rag/mod.rs`: `catch_up` without an embedder for an own data folder without a stored Eliezer connection
  (`NO_EMBEDDER_OWN_DATA`), `index_info` with `stale` and `stale_text`, `reindex_report` → `Reindexed {pending,
  deferred}`, `answer_json` with `weak`.
- `baruch-lancedb`: `Scope::contains`, `Index::replace(scope, …)`, `IndexMeta.document_prefix`.
- `sync/`: `Link::answered` (via `multigpt::Session::broken`), `Writer::blocks`/`chapter`/`restore`, marker
  `UPLOADING`, `Filler::number` returns the set value, completeness of the list in `download::list`.
- `connections/check.rs`: `multigpt` ends the session with `McpClient::delete` (not if the common time limit of 10 s
  aborts the check).
- Open: older revisions and findings in the index (user question 1), comments/findings to MultiGPT (user question 11);
  whether the real MultiGPT names a document id and caps `limit` is checked by the user.

### R19b pgvector

No migration (prefixes in the JSON of the connection, group name proposals in memory, adopted names become keywords
via `idea.update`).

- **pgvector** (`connections/pgvector.rs`, see "pgvector" above): all fields shortened in SQL, `quoted` doubles
  quotation marks, `tls_warning`, query embedded with `query_prefix`. `connections/config.rs`: `fields` knows
  `query_prefix` (pgvector, lancedb) and `document_prefix` (lancedb), `check_field` checks with
  `baruch_rag::check_prefix` (without trimming), `NOMIC_PREFIXES` is only the suggestion on the page (no default, so
  that an existing index stays valid). `connections/pages.rs`: fields with `placeholder`/`maxlength`, error code
  `praefix`, `tls_warning` per connection, `status_context` passes `index.stale` through (filled by the index worker,
  R19a); the template shows „n Aufträge liegengeblieben …“ only when `stale > 0`.
- **Idea map, groups** (`idea_map/groups.rs`): `components` (union-find over the proximity pairs, root = smallest id,
  only components ≥ 2) → `Group {group: "g<id>", ideas, words, proposal}`; `groups(state, work, filter)` builds the map
  with `near: true`, `of_map` uses one already built (page). `name_groups`: one run per (data folder, work)
  (`running`, the guard `Running` unregisters when dropped), persona = the named active one or `rows_and_lector`, call
  via `state.model.chat` without the write lock, message with `DATA_TITLE` and per idea
  `assist::context::framed_line(…, "einfall", …)` (defused, cannot leave the box), system prompt `SYSTEM` with "data,
  not instructions"; `parse_names`/`clean_name` take only "g<id>: Name" of known groups (≤ 40 characters). Proposals in
  a process-wide table per (data folder, work) → group (`Proposal` with the ideas at the time of the proposal; shown
  only for the same group) – no SQLite, no mirror, loss on restart intended (contract §1). `accept`: ideas from the
  proposal or from the groups by filter, only those still present, pre-check "at most 20 keywords", then per idea
  `ideas::call("idea.update", {id, words})` (origin human, commit with `Baruch-Action: idea-update`). RPC in
  `idea_map/mod.rs` (`METHODS`), forms via `bild::ACT_METHODS` and `run_act` (filter from `back`), page: section
  „Gruppen“ in `karte.html` (context `clusters`), style `static/karte.css`. MCP gets no tool for it. MCP `idea_map`
  takes `near_limit` and `near_threshold` (`filter_from_params`, refusal as `Failure::Tool` = `isError`).
- **Ribbon** (`ribbon.rs`): `SCRIPTURE_FUNCTIONS` (`bibelstelle`, `bibel`) via `defined_functions`,
  `cites_literature` (line with `#bibliography(` outside `//`). `ws_command.ts` (otherwise frozen) gets the commands
  `scripture`/`cite` and the pure building blocks `scriptureFunction`, `scriptureSource`, `citeKey` (no imports, hence
  testable under Node); `ribbon.ts` sends `arg = {fn}` for `scripture`, `wysiwyg.ts` inserts an island
  (`span.ws-island[data-raw]`) or a reference (`button.ws-ref[data-target]`), which `wysiwyg.serialize` stores
  literally or as `@schlüssel` (`docs/WYSIWYG.md`).
- **NFC**: `baruch_library::check_range` compares excerpt and quotation in NFC (range in characters of the stored full
  text, alternatively in characters of its NFC form); `websearch::verify::normalize` begins with `nfc()` (page and
  quotation both pass through it). Crate `unicode-normalization` 0.1 (S1).
- **Assumptions:** the function `#bibelstelle`/`#bibel` of the template takes the reference as content `[…]`; the
  bibliography stands as `#bibliography(` in the template (not in an included file). Both are recognised without Typst
  (`ribbon::code_lines`); since Round 23 block comments and the bracket depth count (top level only), and functions
  introduced by `#import "datei.typ": name` count too.

### R19c treffer

- **Rendering of hits:** the threshold only in `baruch_rag` (`WEAK_SCORE`, `is_weak`, `WEAK_LABEL`, interface S1).
  `literature::rag_view` sorts stably by `weak` and returns `weak_count`; `fragments/library.html` renders weak hits
  via a macro in `<details class="weak-hits">`. `ideas::rag_ideas` sorts weak ones last; `ideas.html` the same pattern.
  `assist::rag_facts::drop_own_section` and `block_entries` put weak hits at the end (before the limit `LIMIT`) and
  write `rag_facts::similarity` („Ähnlichkeit (schwach, n %)“).
- **`idea.search` (rag):** LanceDB not `Connected` or an error other than `RagError::Invalid` → `rpc::RAG_UNAVAILABLE`
  (-32001). MCP `idea_search {mode: "rag"}` calls the same RPC and turns that into `isError`.
- **baruch-scripture:** after the word comparison, `wording::compare` additionally compares without whitespace
  (`Comparison::spacing`): the keys of the Bible words concatenated, every part of the quotation must fit from a word
  boundary of the module to a word boundary (no hit in the middle of a word). `plain_words` normalises to NFC
  (`unicode-normalization`), otherwise a decomposed „ü“ fell apart into two words. `wording::fragment_survey` estimates
  hyphenation remnants: pair „A b“ in the same verse, A without punctuation at the end, b lower case, at most 5
  characters, not a frequent short word, and „Ab“ occurs elsewhere in the read text; conspicuous from 3 cases and 1 in
  200 word pairs (`FragmentSurvey::fragmented`). `quote_for` now allows up to three words from `INTRO_WORDS` with a
  colon before a quotation (at most 48 bytes). `count_word_follows`: „Mal“ followed by a chapter and verse reference is
  not a numeral.
- **baruch-sword:** `Bible::sample_texts(max, deadline)` (sample in module order), `reference::lemma_candidates`,
  `compound_tails`, `strong_like`, `Lexicon::lookup_lemma` (headword, then `get_strongs` for Strong's numbers
  independent of `StrongsPadding`, then endings/umlaut, finally compound; answer with the form tried).
  `fixture::hyphen_remnants` builds hyphenation remnants into invented text (tests; no test reads third-party
  modules).
- **Server:** `bibles::WordingResult.module_spacing`, `SPACING_LABEL`; `bibles::module_note` (sample of 4000 verses,
  2 s, cache per conf file and modification time, blocking) and `choice_note`; `scripture::check_with`, `bible.marks`,
  `facts_with` (line „Hinweis: …“) and `bible_line` carry the note. `scripture::mark_with`: other personas with a Bible
  get `mark_others_bible(Modul)` (begins with `MARK_BIBLE`, and thereby counts as a closing note in `chat_render`),
  without a Bible `MARK_OTHERS`; `mark_answer_for_work` opens the Bible only if a reference occurs.
  `bibles::german_of` maps a verse of the module back into the German versification via `mapping::map_verse`;
  `bible.lookup` returns `german_ref`/`german_problem`, the Bible page (`literature/bibles.rs`,
  `literature_bible.html`) puts them into `data-ref`/`data-verse-ref` and as a link.
  `literature::bibles::workspace_shelf` (via `view::extras`, field `ViewExtras.bibles`) and `Shelf::context_at` bring
  the box „Bibeln“ onto the work page on the server side.
- **Assumptions:** the threshold 0.45 applies equally to all ways, although MultiGPT does not report its scale; the
  fragment estimate is a rule of thumb (it can err with very short samples and then rather reports nothing).

### Integration

- Merged without conflicts (index → pgvector → treffer, each `--no-ff`). The parts mesh through the interface R19-S1:
  index fills `IndexInfo.stale`, the page „Verbindungen“ (pgvector) shows it; `config::embed_prefixes` are read by
  LanceDB (index) and the pgvector search; `baruch_rag::is_weak` applies equally in `rag.search` (index) and in
  literature, ideas and evidence (treffer).
- One `EmbedderFactory` for all ways: `rag::set_embedders` or Eliezer provides the embedders for LanceDB and for the
  query to pgvector (`rag::plan_way` passes the same factory to `hooks::open_pgvector`); each way chooses the prefix
  itself from its connection.
- MCP `idea_search`: the short description names `mode` (integrator, `mcp/tools.rs`).
- Acceptance `tests/round19_abnahme.rs` (5 tests, work „Die Laternen am Kai“, everything freely invented, only mocks).

## Round 20 – EPUB preview

### Interface R20-S1

New crate `crates/baruch-epub` (pure: no database, no network, no Typst typesetting). Frozen in `lib.rs`:
`EpubMeta`, `EpubImage`, `EpubSection`, `FootnoteMode` (`popup` default, `endnotes`), `EpubInput`, `ItemKind`,
`EpubItem`, `TocEntry`, `EpubWarning`, `EpubBook`, `EpubError`; constants `MIMETYPE`, `CONTENT_DIR` (`OEBPS`),
`OPF_PATH`, `CONTAINER_PATH`. Modules: `build` (S1 minimal build: file split per part/chapter `text/k{key}.xhtml`,
front matter `text/front.xhtml`, `<p>` with escaped raw text, `nav.xhtml`, `style/book.css` from
`baruch_typst::typeset::epub_style`, minimal OPF), `package` (real: `mimetype` first, STORED without an extra field,
fixed timestamps, DEFLATE), `xhtml::escape_text`, `sample::input` (freely invented work „Das Gartenhaus am Bach“, PNGs
created in code), `readers` (types and slugs frozen, placeholder values), `check` (types frozen, body STUB). Naming
rules of the files and classes: contract §1.4 (head of `lib.rs`).
Store: `Store::epubcheck_jar`/`set_epubcheck_jar` (`store_meta`, key `epubcheck_jar`, no migration).
Server: `epub.rs` (RPC `work.epub`, `GET /werke/{id}/epub`, `epub_input`, `book_for`, `epub_bytes`,
`write_epub_file`; STUB `EPUB_COVER` moved here from `cover.rs`, `cover::book_info` is `pub(crate)`),
`epub_preview.rs` (`epub.readers` real, `epub.preview` and pages STUB), `epub_check.rs` (`epub.check`,
`epub.checker_get`/`epub.checker_set`, pages, forms, `preview_panel` – all STUB), MCP `mcp/epub.rs` (`epub_export`)
and `mcp/epub_check.rs` (`epub_check`), both STUB and read-only. Styles `static/epub.css`, `static/epub_check.css`,
`static/readers/<slug>.css`; widget `web/src/epub_preview.ts` (empty).

### R20a epub

- **`baruch-epub::build`** replaces the S1 minimal build: split according to §0.2 (`split`), text via the block model
  (`xhtml::Renderer`, mapping in `docs/WYSIWYG.md` „EPUB-Abbildung“), footnotes (`FootnoteMode::Popup` →
  `noteref`/`aside` in `ol.baruch-footnotes` at the end of the file; `Endnotes` → `text/notes.xhtml`; numbers
  consecutive like Typst, restarting per chapter only with the typesetting setting `restart = per_chapter` in popup
  mode, signs according to `FootnoteNumbering`), images (only those named by the text, `images/<bereinigter Pfad>`,
  ids `img-n`), cover (`images/cover.<ext>` + `text/cover.xhtml`), front matter (`title`, `imprint`, `dedication`),
  `nav.xhtml` (toc + landmarks, `hidden`), `toc.ncx` (nested navPoints, `dtb:depth`), OPF (metadata only via
  `escape_text`, `urn:isbn:` only after `isbn13_check`, image credits as a second `dc:rights`, schema.org
  accessibility, `alternativeText` only if every image has alt, `<guide>` for EPUB 2/Kindle), CSS = `build::BASE_CSS`
  + `epub_style(&typeset)`. Labels are collected beforehand (`collect_labels`), so that `@name` points across files.
- **`baruch-epub::template`**: `pagebreak_rules` (own scanner: comments removed, every show rule up to the end of the
  line at bracket depth 0) → `TEMPLATE_PAGEBREAK_MAPPED` for `heading.where(level: 1)`/`<baruch-part>`, otherwise
  `TEMPLATE_PAGEBREAK_UNMAPPED`; `template_chapter_start` reads `detect_chapter_break` and sets `chapter_start` when
  the typesetting setting is missing; `template_lang` (`#set text(lang: …)`), `is_language_tag`.
- **`baruch_typst::typeset::epub_style`**: break rules for `section.baruch-chapter`/`section.baruch-part` always
  (default new page, continuous → `auto`, right-hand page → `right`), because chapters are always files of their own.
- **Server `epub.rs`**: `epub_input` collects `export_map` (work template = text before the first section), numbers
  from `numbering_preview` with the configured scheme, `export_assets` + alternative texts from `asset_list`, cover
  (`cover::cover_name`, `asset_data`), `book_meta` + `cover::book_info` → `imprint_lines`, `copyright_line`, language
  (typesetting setting → template → "de"), `dcterms:modified` (latest point in time from work, book details,
  typesetting settings, images, revisions). A section without a readable heading is kept as level 2. RPC `work.epub`,
  route `GET /werke/{id}/epub`, `write_epub_file` (CLI, `create_new`). STUB `EPUB_COVER` removed.
- **MCP `mcp/epub.rs`**: `epub_export` read-only, `MCP_EPUB_LIMIT` = 8 MiB for `include_bytes`.
- **CLI** `export --epub` (`main.rs`).
- Tests: `baruch-epub/tests/{build,xhtml,package}.rs` (split, footnotes, cross-references, images/alt, islands,
  template rules, metadata/escaping, OCF order, manifest/spine, xmllint), `baruch-typst/tests/typeset_epub.rs`, server
  `tests/round20_epub.rs` (RPC, download, 403/404/422, cover/ISBN/images, MCP with the 8 MiB limit, CLI).

### R20b reader

- **Profiles** `baruch_epub::readers` (`PROFILES`, `profile(slug)`, new `required()`): the nine frozen slugs in
  contract order with device, dimensions (CSS pixels in portrait), `required` (the five mandatory profiles),
  `via_conversion` (only Kindle/Kindle app), `ReaderSupport` and hint. **Assumptions** (compiled without a real device
  and without network, from the knowledge of the developers): Apple Books "Original" with Georgia as fallback, own
  fonts reliable only with `com.apple.ibooks.display-options.xml`; Google Play Books "Original" with Literata;
  Kindle/Kindle app after Send to Kindle with Bookerly instead of the publisher font, without background colours, SVG
  uncertain; tolino and Kobo (ordinary EPUB, not KEPUB) show footnotes as a jump instead of a popup; e-ink in
  greyscale; Thorium (Readium) and the calibre viewer support EPUB 3 completely. **Manufacturer readers on Android**
  (Samsung, Huawei …) cannot be checked without a device – no invented profile, user question.
- **Reader CSS** `static/readers/<slug>.css` (one file per profile, in the server via `include_str!`,
  `epub_preview::reader_css`): default font, size, margins (`padding` on the `body`), line spacing, alignment,
  hyphenation (`hyphens`), footnotes (popup readers: box „Popup-Fußnote“, otherwise note text with a rule), e-ink
  (`filter: grayscale(1)`, no backgrounds), own default font (`!important`) and, after the marker `/* @night */`, the
  night mode. No `<`/`&` (the CSS stands in XHTML), no addresses.
- **Reader files** `GET /werke/{id}/epub/leser/{slug}/{*file}` (`epub_preview::reader_file`): the book is built anew
  per request (`epub::book_for`, no cache); XHTML gets `<style id="baruch-reader">` before `</head>` (after
  `style/book.css`), everything else byte-identical with the type from the manifest. Response with CSP
  `sandbox allow-same-origin; frame-ancestors 'self'` (no scripts, no forms, can only be framed in Baruch;
  `allow-same-origin`, so that images and styles of the reading file load as requests of the same origin – otherwise
  the protection against other pages refused them with 403), `nosniff`, `no-cache`. Paths with `..`, `.`, empty
  parts, `\` or control characters and everything outside the manifest → 404; `text/notes.xhtml` without `fussnoten`
  builds with endnotes (links from chapters lose the query).
- **Framing and origin:** for a response with its own `frame-ancestors` directive, `guard::no_framing` sets no `DENY`
  and appends nothing to the CSP: with `frame-ancestors 'self'` `X-Frame-Options: SAMEORIGIN`, otherwise without
  `X-Frame-Options`. All other responses keep `DENY` and `frame-ancestors 'none'` (test). Because of `sandbox` the
  framed document runs with an opaque origin; its subrequests (images, `book.css`, links between files) arrive under
  Fetch Metadata as `Sec-Fetch-Site: cross-site`. They are accepted if `Referer` points to the same `Host`
  (`epub_preview::from_own_frame`), otherwise 403. **Checked in the visual check of the integrator (Firefox):** the
  browser shortens this `Referer` to the origin (`http://host/`), even with `Referrer-Policy:
  no-referrer-when-downgrade` in the frame; therefore the shortened `Referer` of the same host counts (or, unshortened,
  a reader file of the same work). A foreign page cannot set the `Referer` to our host.
- **Preview page** `GET /werke/{id}/epub/vorschau` (`templates/epub_preview.html`, environment of its own in the
  module): form GET (`leser` multiple, `datei`, `fussnoten`, `nacht`), list of chapter boundaries from the spine files
  with text (titles from `EpubBook.items`, part/chapter marked as a boundary, links with the profile choice), hints
  from building (`EpubBook.warnings`), per profile a `<figure>` with an `<iframe>` in device size, the hint „Send to
  Kindle nimmt EPUB an“, below it the place „Prüfung“ with the unchanged fragment `epub_check::preview_panel` (empty →
  link to `/werke/{id}/epub/pruefung`). Unknown slug/file → hint with 404, invalid `fussnoten` → 400, work without a
  section → hint with 422, foreign page → 403. The widget `epub-preview` (`web/src/epub_preview.ts`) submits the form
  on every change (not when no reader is chosen any more).
- **RPC** `epub.readers`, `epub.preview` (see RPC.md). Menu: base.html (see the section on the header and menus, `menu.rs`).
- Tests: `baruch-epub/tests/readers.rs` (slugs, mandatory, Kindle, CSS file per slug), `tests/round20_reader.rs`
  (page, form, 404 hints, reader file with CSP/`SAMEORIGIN`/`nosniff`, xmllint, images via `reader_file`,
  `no_framing`, RPC, check place, reader CSS), `tests/round9_menu.rs` (menu entries, page in `all_pages`).

### R20c epub-pruefung

- **`baruch_epub::check`** (pure, without network): `check_structure(epub, xmllint)` reads the ZIP bytes. It checks
  the first local file header directly in the bytes (`mimetype` as the first entry, method STORED, no extra field,
  content `application/epub+zip`), the rest via `zip::ZipArchive` (only XML/CSS are unpacked, at most 16 MiB each;
  paths with `..`, `/` at the start or `\` are an error). `container.xml` → `rootfile/@full-path` (without a container
  the fallback `OEBPS/content.opf`, so that the other checks still run), OPF → metadata (identifier including
  `unique-identifier`, title, language, `dcterms:modified` for EPUB 3, `schema:access*` as a hint), manifest
  (duplicate ids, missing files, files without an entry except `mimetype`, `META-INF/*` and OPF), spine (known ids,
  only XHTML or with `fallback`, `toc` → NCX, without `toc` the hint `NCX_MISSING`), `<meta name="cover">`,
  navigation (`properties="nav"` and `<nav epub:type="toc">`), reading order (files of the nav links must stand in
  spine order). Per XHTML: language, `<title>`, `alt` (missing = error, empty = hint), duplicate ids, references of
  `img`, `link`, `a`, SVG `image` to missing files or jump marks. Codes fixed in `check::codes`.
- **Well-formedness:** with `xmllint --noout --nonet -` per file via standard input (if `find_xmllint` finds it in the
  passed PATH content), otherwise, and as a fallback if xmllint does not start, with a small XML tree of its own over
  quick-xml (`check/xml.rs`: exactly one root, closed elements also at the end of the file, valid attributes without
  duplicates, only the five predefined entities and character references – XHTML in EPUB has no DTD). The content is
  always checked through this tree; `CheckReport.xml_tool` names the tool of the well-formedness check.
- **Reader warnings** `reader_warnings(book, profile)` from `ReaderSupport` and the built book (CSS and XHTML of the
  items): popup footnotes (`epub:type="noteref"`), page break CSS (for chapter/part files), embedded fonts (`font/*`,
  `@font-face`), publisher font (`font-family` with `overrides_publisher_font`), CSS backgrounds and night mode, SVG,
  hyphenation (`hyphens: auto`), conversion via Send to Kindle (`via_conversion`). Every warning carries
  `reader = slug` and begins with the name of the reader.
- **EPUBCheck:** `find_epubcheck(path_env, jar)` – `epubcheck` (executable) in the passed PATH content before an
  existing `.jar`; `run_epubcheck` starts the program or `java -jar` with the EPUB in a process group of its own, reads
  stdout/stderr in threads of their own (at most 4 MiB each) and, when the time limit expires, ends the whole group
  (`kill -KILL -- -pgid`, then `Child::kill`) without waiting for the readers. Lines `ERROR(…)`, `FATAL(…)`,
  `WARNING(…)`, `INFO(…)`/`USAGE(…)` become findings `EPUBCHECK_<Code>` with the path in the EPUB; "passed" means exit
  0 without an error line. A script just written that briefly reports "Text file busy" is restarted up to 20 times.
  Never a download; the PATH of the process is only read, never set.
- **Server `epub_check.rs`:** `run_check(state, work, readers, footnotes, epubcheck, path_env)` builds the EPUB via
  `crate::epub::book_for` and `baruch_epub::package` (like the download, no commit), checks in `spawn_blocking`,
  appends the warnings of the build (`…_MAPPED` as a hint, otherwise a warning) and per reader `reader_warnings`.
  EPUBCheck only with `epubcheck = true`: path from `Store::epubcheck_jar`, at most one run at a time (static
  semaphore), EPUB in a temp folder of its own (`baruch-epubcheck-<pid>-<n>`, removed when dropped), time limit 180 s.
  RPC `epub.check`, without a work `epub.checker_get`/`epub.checker_set` (check of the path in `validate_jar`:
  absolute, `.jar`, regular file, no control characters, ≤ 4096 bytes). Page `GET /werke/{id}/epub/pruefung`
  (template `epub_check.html` with the fragment `fragments/epub_check_panel.html`, minijinja environment of its own),
  forms `POST /werke/epub/pruefen` (200 with the report including EPUBCheck) and `POST /einstellungen/daten/epubcheck`
  (303 or 422 via `settings::error_page`); the block „EPUBCheck“ on settings → data comes from
  `epub_check::settings_context`. `preview_panel` returns the report without EPUBCheck as a finished, escaped fragment
  for the preview. MCP `epub_check` calls `run_check` with a fixed `epubcheck = false`.
- **Book details leftovers:** `baruch_typst::cover::isbn13_check` (signature unchanged) cuts off the prefix
  „ISBN“/„ISBN-13“/„ISBN 13“ in any spelling and knows the new error kind `IsbnError::Prefix` (not 978/979);
  `Store::set_book_meta` compares the ISBN as text (one only written differently is a change). Paper thickness:
  `paper_mm` per page stays stored (no migration, exchange format unchanged); `work.meta.get` computes
  `paper_sheet_mm` = 2 × `paper_mm`, `work.meta.set` and the form take `paper_sheet_mm` (stored halved; together with
  `paper_mm` -32602). Deleting the cover image: under the same write lock `cover::release_cover_before_delete` first
  removes the cover with a commit of its own „Umschlag entfernt“ (`asset.delete` without `rev`, form, confirmed
  deletion request), only then does the store delete the image. A confirmed cover request sets the cover via
  `Store::set_work_cover` under the lock and writes **one** commit (`mirror::describe_asset_request` plus „Umschlag
  vorher/nachher“ via `cover::commit_request`) instead of two before.
- **Printer cover with text rules (deviation from the backlog: moved here from R20a, contract §0.6):**
  `baruch_typst::cover::template_text_rules` reads the `#set text(…)`/`#set par(…)` rules at the start of a line of the
  work template including the rule block of the typesetting settings (`cover::page_template`), keeps only `lang`,
  `region`, `script`, `dir`, `hyphenate`, `tracking`, `spacing`, `costs`, `kerning`, `ligatures` or `justify`,
  `leading`, `spacing`, `first-line-indent`, `hanging-indent`, `linebreaks` with simple values (numbers with unit,
  strings, `true`/`false`/`auto`/`none`, dictionaries of these) and leaves out conditional rules, variables and a
  `lang: "de"` that applies anyway. `wrap_source_with_rules` and `block_measure_source_with_rules` put them directly
  after `#set text(lang: "de")`; without rules they are byte-identical to `wrap_source`/`block_measure_source` (golden
  states R26/R27 still green, `round27_satz.rs`, `baruch-typst/tests/cover_layout.rs`). The cover still does not take
  over show rules of the template (they could affect headings or pages that do not exist on the cover).

### Integration

- Merge without conflicts (`--no-ff`, order S1 → epub → reader → epub-pruefung).
- Integrator: the `instructions` in `mcp.rs` name `epub_export` and `epub_check`; the S1 exception for STUB
  descriptions in `tests/mcp.rs` is gone; the check page `/werke/{id}/epub/pruefung` is in `round9_menu` `all_pages`
  (42 pages); the module head of `cover.rs` no longer points to the removed `EPUB_COVER`; S1 state sentences in
  Bedienung/RPC/MCP replaced. `grep -rn "STUB:" crates` shows no R20 notes any more.
- Tests adjusted after the merge (behaviour of R20a that reader/epub-pruefung did not know in their own worktree):
  titles of the files carry the number from the scheme („2 Das Holz“, `round20_reader.rs`), the test work brings its
  own images (own image path in the reader test; `IMG_ALT_EMPTY` in `check.rs` counted against the starting point).
- Visual check (Firefox): images and `book.css` did not load in the frame (403), because Firefox shortens the
  `Referer` of the subrequests from the `sandbox` document to the origin; `from_own_frame` now accepts the shortened
  `Referer` of the same host (test in `round20_reader.rs` and unit test). E-ink profiles (kindle, kobo, tolino):
  `filter: grayscale(1)` on the `body` instead of on the `html`, otherwise the paper tone does not fill the frame
  completely.
- Acceptance `tests/round20_abnahme.rs` (7 tests, work „Das Feuer am Kap“, freely invented): download via menu/route,
  RPC and CLI with equal bytes; cover + front matter + two parts + three chapters as files of their own, nav/ncx, two
  popup footnotes with target, cross-reference `kC.xhtml#kN`, metadata with `&`, `<`, `#[` literally, typesetting
  settings in the CSS, xmllint on every XML file, own check without errors; preview (five mandatory profiles, choice by
  GET, reader file with CSP/SAMEORIGIN, hint `TEMPLATE_PAGEBREAK_MAPPED`, `..._UNMAPPED` for a figure rule);
  deliberately broken EPUBs (missing file, wrong or compressed mimetype, broken XHTML); EPUBCheck not found or as a
  script in the passed PATH content; MCP both tools without a commit, `epub_check` without EPUBCheck.

## Round 31 – Index and glossary

### Interface R31-S1

Contract R31 (PM, 2026-11-09). Frozen types and signatures, bodies partly as honest stubs (`grep -rn "STUB:" crates`,
note "(R31 …)"):

- `baruch-typst::index` (new, owned by register-satz): index mark `#metadata((register: "…", term: "…"))<baruch-idx>`
  (`IndexMark`, `mark_source`, `parse_mark` real), `Occurrence` (`first`/`all`/n), built-in indexes `stichwort`,
  `personen`, `bibel` (`builtin_registers`, `valid_register_slug`), `term_key` (NFC, whitespace, lower case; it is
  stored in the database, hence unchangeable), `glossary_anchor` (`baruch-gl-UUID`), `BackMatter` with `is_active`.
  Stubs: `sort_key` (= `term_key`), `find_marks`, `mark_word`, `insert_mark_at`, `remove_mark`
  (`IndexError::NotYet`), `Projector::project` (identity), `back_matter` (""), `overview` (empty). New dependencies
  `baruch-scripture` and `unicode-normalization`.
- `TypesetSettings` (frozen) has three new fields at the end: `glossary_page` (`GlossaryPosition`: before/after the
  indexes), `index_pages` (`IndexPages {registers, columns}`, 1–3 columns, at most 16 indexes) and
  `glossary_first_mention`; `FIELDS` has 19 entries. The rule block "// Baruch: Satz" produces no line for them.
- `baruch-wysiwyg`: `Inline::IndexMark {mark}` (JSON `{"t":"index_mark","mark":{…}}`); `parse` recognises the
  canonical mark before any other `#` expression, `emit` writes `mark_source`, `html` shows it like an inline island
  (`ws-island ws-index-mark`, `data-raw` = `mark_source`), so that today's `wysiwyg.ts` writes it back losslessly.
- `baruch-epub`: `EpubInput::back_matter` (default empty), `ItemKind::Glossary`/`Index`, naming rules
  `text/glossary.xhtml` (id `glossary`), `text/index-{slug}.xhtml` (id `index-{slug}`); `build` still ignores the field.
- `baruch-store::glossary` (owned by glossar-daten): types `GlossaryTerm`, `GlossaryEdit`, `GlossaryChange`,
  `IndexRegisterRow`, `NewIndexProposal`, `IndexProposal`, `IndexAccepted` and the `Store` methods; S1 reads empty and
  refuses writing with `StoreError::Glossary("noch nicht umgesetzt (R31 glossar-daten)")`. New `StoreError` variants
  `Glossary`, `UnknownGlossaryTerm`, `UnknownIndexProposal` (RPC: INVALID_PARAMS). No migration in S1 (0049 comes with
  glossar-daten).
- Store hooks (owned by register-satz): `snapshot::back_matter_in` (typesetting settings + built-in and own indexes
  + accepted terms), `Store::back_matter`; `export_parts` returns the back matter as third item, export and export map
  append it via `snapshot::append_tail` (map byte-identical to the export), `main.typ` after the `#include` lines; per
  section a `Projector::project` runs after `label_section`; the mirror gets `glossary::mirror_files`
  (`glossar.md`). Without R31 settings everything is byte-identical.
- Server: modules `glossary` (RPC `glossary.*`, `index.registers/register_*/proposals/decide`), `index_marks`
  (`index.entries/mark/unmark/overview`), `glossary_page` and `index_page` (pages), `mcp::glossary`,
  `assist::glossary`; stubs answer REFUSED „noch nicht umgesetzt (R31 …)“ (`glossary::registers` already returns the
  built-in ones). `epub.rs` sets `back_matter`, `mirror::typeset_value` knows the new fields. Widgets `context-menu`
  and `glossary` (empty).

### 31a glossar-daten

- **Migration 0049** (`0049_glossary.sql`, the only new migration of the task): `glossary_term` (term, `term_key`,
  short/detailed explanation, source, Bible references as a JSON list, status `accepted`/`proposed`, origin, times;
  unique partial index `(work_id, term_key) WHERE status = 'accepted'`), `glossary_variant` (variants per term in
  order, `form_key`, without uuid), `index_register` (own indexes, built-in identifiers blocked by CHECK,
  `UNIQUE (work_id, slug)`), `index_proposal` (index proposals, without uuid – workflow of the instance). uuid, fill
  trigger, tombstone and cascade triggers for `glossary_term` and `index_register` like 0048; `uuid_tombstone` gets
  the kinds `glossary_term` and `index_register` by rebuild (the seventeen tombstone triggers literally anew). No
  `FOREIGN_KEYS_OFF` (no foreign key points to `uuid_tombstone`); evidence
  `crates/baruch-store/tests/migration_0049.rs`.
- **Store** `baruch_store::glossary` (contract §2.5): reading sorts in Rust by `baruch_typst::index::sort_key`;
  `glossary_lookup` via `term_key` or `form_key` (accepted ones only); writing checks the limits `MAX_GLOSSARY_*`
  with German messages (`StoreError::Glossary`), tidies up (term and variants in one line, duplicates removed) and
  refuses duplicates among the accepted ones; `glossary_propose` refuses accepted and already proposed terms;
  `glossary_decide(false)` deletes the proposal (tombstone). Indexes: built-in and invalid identifiers refused, at most
  13. `index_propose` checks the mark (`IndexMark::validate`), the index (built-in or own), the fixed key (`GoneNode`)
  and whether the word stands in the current revision; `index_proposal_accept` applies `mark_word` to the current
  revision in **one** transaction and stores a new revision by the human (`write::save_human`, then
  `check_current_heading`); if the word is no longer there, the proposal becomes `stale` (stored) and the text stays.
  As long as `mark_word` (register-satz) honestly reports `NotYet`, the adoption refuses with this message, nothing
  changed. `accepted_entries_in`/`registers_in` feed `Store::back_matter`; `mirror_files` returns `glossar.md`
  (accepted terms alphabetically with variants, explanations, Bible references, source; then „Vorschläge (noch nicht
  angenommen)“ with origin; then own indexes) – without terms and indexes no file.
- **Server** `crate::glossary` (§3.1): all RPC methods; writing functions take the write lock before writing to the
  database, exactly one mirror commit per change via `mirror::record_glossary` (`describe_glossary`,
  `describe_register`, `describe_index_proposal`, `describe_index_accept`; trailer `Baruch-Action`,
  `Baruch-Glossary`/`Baruch-Register`/`Baruch-Index-Proposal`, for proposals `Persona` and as author the persona or the
  client, when deciding `Baruch-Proposer`). `glossary::normalize_bible_refs` checks and normalises Bible references
  with `baruch_scripture::parse`. `glossary.suggest` (`assist::glossary`): persona by the lector rule or `persona`, one
  model call via the `ModelPort` (answer form „Begriff | kurze Erklärung“), results only via `glossary::propose`; an
  error of the model → REFUSED, nothing created.
- **MCP** `mcp::glossary`: `glossary_list` (read-only), `glossary_propose` and `index_propose` (proposals only, origin
  by `acting_persona`, `node` → `isError`).
- **Exchange minor version 7** (`baruch-exchange`, XSD `docs/schema/baruch-werk-1-u7.xsd`, u6 frozen): model
  `ExchangeRegister`, `ExchangeGlossaryTerm`, `ExchangeGlossaryStatus`; tombstone kinds `glossar-begriff`
  (`glossary_term`) and `register` (`index_register`) from 7; `<satz>` attributes `glossarseite`, `registerseiten`,
  `registerseiten-spalten`, `glossar-erste-erwaehnung` (`typeset::R31_ATTRIBUTES`, read only from 7 via
  `typeset::r31_from_attrs`). `exchange_dump` writes indexes and all terms, `exchange_apply` takes them over anew with
  identifiers or reconciles (a known identifier is never overwritten, the same `slug`/`term_key` under a foreign
  identifier → hint instead of a duplicate, tombstone → „hier gelöscht“). Index proposals are not in the format.

### 31b register-satz

**baruch-typst::index** (bodies; private submodules `index/scan.rs`, `index/project.rs`, `index/back.rs`):

- **Reading** (`scan.rs`): per byte running text or a blocked place with a number of its own. Blocked are heading
  lines (`=` lines, part and „ohne Nummer“ markers), escapes, raw, comments, formulas, labels, references (`@name`),
  addresses, code after `#` including strings and brackets, statements (`#let`, `#set`, `#show`, `#if` … up to the end
  of the line including all brackets) and marks. Content blocks of calls (`#emph[…]`, `#footnote[…]`, `caption: […]`)
  are running text again. An insertion point is allowed if it does not lie between two bytes of the same blocked
  place, not at the end of a comment or heading line, not at the start of a heading line and not before a label (it
  would otherwise attach to the mark).
- `find_marks` = all canonical marks in text order (also in footnotes and content blocks, never in a comment, raw or a
  string). `mark_word` looks for the word literally as a whole word in running text (whitespace in the word matches
  whitespace without an empty line; a word directly before a label → mark after the label), puts `mark_source` directly
  after it (first, n-th or all occurrences); `insert_mark_at` checks the place; `remove_mark` takes only a range that
  `find_marks` returns. `sort_key` = DIN 5007-1 (NFD without accents, `ß` = `ss`, lower case, punctuation at the start
  skipped).
- **Projection** (`project.rs`, `Projector`): (a) with the index `bibel` in `index_pages.registers`, after every valid
  reference in running text (`baruch_scripture::scan` on the text with spaces instead of blocked bytes) a mark
  `register: "bibel"`, `term` = book name („Johannes“), `sub` = chapter and verses („3,16“), `sort` =
  `Passage::sort_key` (`43.003.016-003.016`, biblical order); (b) with `glossary_first_mention == Some(true)` per
  chapter (a heading of level 1, part or „ohne Nummer“ level 1 resets) the first occurrence of a term or a variant as
  whole words (case-insensitive, word groups with whitespace, `-`, `'`, `’`, `.`). Form: with a glossary page
  `#link(<baruch-gl-UUID>)[#emph[Wort]]`, without `#emph[Wort]` – so there is never a reference to a missing label. If
  `(`, `[`, `;` or `.Wort` follows, a `;` stands after it (ends the embedded expression, Typst does not set it).
  Projection only in the export; SQLite stays unchanged.
- **Back matter** (`back.rs`, `back_matter`): "" without index pages and glossary page; otherwise from
  `// Baruch: Register und Glossar` helper functions (`baruch-idx-ranges`, `baruch-idx-pages`, `baruch-idx-group`,
  `baruch-idx-entry`, `baruch-gl-entry`), then the glossary page (before or after the indexes) and per index from
  `index_pages.registers` (unknown identifiers are dropped) a page `#heading(level: 1, numbering: none)[…]`. Sorting
  and grouping in Rust (letter groups by `sort_key`, Bible by sort key), page numbers in Typst: `query(<baruch-idx>)`,
  per mark `counter(page).at(m.location())` (the same counting as footer and „S. n“, also from the first chapter),
  following pages "12–14", marks with `see` without a page. Columns via `columns(n, gutter: 1.5em, …)`. Glossary: term
  bold, variants in parentheses, short explanation, paragraphs of the detailed one, „Bibelstellen:“; anchor
  `#metadata("baruch-glossar")<baruch-gl-UUID>` (only for a UUID of letters, digits, `-`, `_`). All data as escaped
  string literals. Same input → same bytes.
- `overview` (overview per index, stored texts): all indexes from `back.registers`, entries sorted as in the back
  matter, `keys` = sections with marks that carry a page; automatic Bible references only with the Bible reference
  index switched on.

**baruch-scripture** (additive): `book_order` (1–66), `Passage::sort_key`, `Passage::chapter_verse`.

**Store** (`snapshot.rs`, `export_map.rs`): the hooks from S1 stay unchanged; checked in
`crates/baruch-store/tests/index_export.rs` (goldens R29-S1 byte-identical, export = projected sections + back matter,
map = export, `main.typ` = before + back matter, SQLite without projection).

**EPUB** (`build.rs`, `xhtml.rs`): `Inline::IndexMark` → `<span id="baruch-idx-{key}-{n}" class="baruch-idx">`
(`Renderer::index_marks`, `PlacedMark`); the first mention (island `#link(<baruch-gl-…>)[#emph[…]]`) →
`<a epub:type="glossref" href="glossary.xhtml#baruch-gl-UUID"><em>…</em></a>` (without a glossary page `<em>`); a `;`
directly after an embedded expression before `(`, `[`, `;`, `.Wort` is dropped as in Typst. With active settings:
`text/glossary.xhtml` (`epub:type="glossary"`, `dt` glossterm with `id="baruch-gl-UUID"`, `dd` glossdef) and
`text/index-{slug}.xhtml` (`epub:type="index"`, `index-entry`/`index-term`, per section one `index-locator` to the
first anchor there, labelled with number and title of the section), at the end of spine and table of contents in the
order of the typesetting; `book.css` gets `INDEX_CSS` only then (otherwise byte-identical).

**Server** `crate::index_marks`: `index.mark`/`index.unmark` take the write lock, set or remove the mark in revision
`rev` and store a new revision by the human with `save_human_on`; `mirror::record` writes exactly one commit. Indexes
must stand in `glossary::registers`. `index.entries` reads the marks of the current revisions and the automatic Bible
references (projection per section); `index.overview` calls `baruch_typst::index::overview`.

Known limits: an imported export with back matter would take the back matter into the last section (the import does
not know `// Baruch: Register und Glossar` yet); the map assigns Typst messages from the back matter to the last
section. Keywords count as equal if they are literally equal.

### 31c kontextmenue

- **Typeset view:** `baruch-wysiwyg` shows `Inline::IndexMark` as an empty `span.ws-index-mark` (fields in `data-*`,
  description from `index_mark_label` in `title`/`aria-label`, sign only in the style); `wysiwyg.ts` reads it back as
  `{t: "index_mark", mark}` and runs the command `index_mark` (`add|edit|remove`, `ws_command.ts`). Details in
  `docs/WYSIWYG.md` („Registermarken und Kontextmenü“).
- **Context menu:** `web/src/context_menu.ts` (widget `context-menu` on `span.ribbon-index` in the ribbon,
  `fragments/edit_toolbar.html`) attaches to typeset view and text field of the same editor; commands to the typeset
  view only via `baruch:ws-command`, no import of `wysiwyg.ts`. RPC: `glossary.add`, `glossary.lookup`,
  `index.registers`. Pure building blocks (`menuItems`, `opensMenu`, `menuKey`, `markFromFields`, `glossaryParams`,
  `entryLine`) and in `ws_command.ts` the mark building blocks (`markSource`, `parseMarkAt`, `findMarks`,
  `validateMark`, `markLabel`, `markInSource`) – they follow `baruch_typst::index` and are checked under Node against
  the Rust functions.
- **Pages without JavaScript:** `src/glossary_page.rs` (`/werke/{id}/glossar`, `POST …/glossar/act`,
  `/werke/{id}/markierung`, templates `glossary.html`, `marking.html`, `fragments/context_menu.html`) and
  `src/index_page.rs` (`/werke/{id}/register`, `POST …/register/act`, template `index_page.html`). No data storage of
  their own: reading and writing only via `crate::glossary` (`call("glossary.list"|"glossary.lookup"|
  "index.proposals")`, `add`, `update`, `delete`, `decide`, `registers`, `register_*`, `index_decide`) and
  `crate::index_marks` (`entries`, `overview`, `mark`, `unmark`); their messages appear on the page or as 422 with the
  inputs. `glossar/act` with `action=suggest` calls `crate::assist::glossary::suggest` (follow-up R31). Success → 303
  to `back` (only paths of this server, `glossary_page::safe_back`), otherwise the page, with `ok=<action>` before the
  anchor (`#begriff-{id}`, `#register-{slug}`, `#vorschlag-{id}`). Styles `static/glossary.css`,
  `static/context_menu.css`; widget `web/src/glossary.ts` (filter of the lists).
- **Typesetting settings:** `settings/work_typeset.rs` reads and writes the three R31 fields (`glossary_page`, per index
  a checkbox `index_register` in typesetting order + `index_columns` (empty = 2), `glossary_first_mention`);
  `BlockInput::registers` comes from `crate::glossary::registers`. `formToSettings` (`typeset_settings.ts`) reads the
  same fields (Node comparison test in `tests/round31_kontextmenue.rs`).

### Integration

- Merge without conflicts (`--no-ff`, order S1 → glossar-daten → register-satz → kontextmenue).
- Integrator: the `instructions` in `mcp.rs` name `glossary_list`, `glossary_propose` and `index_propose` (proposals
  only); the S1 exceptions in `tests/mcp.rs` (STUB descriptions) and `tests/round29_oberflaeche.rs` (three R31 fields
  in the typesetting block) are gone; S1 state sentences in Bedienung/RPC/MCP/SKILLS replaced. `grep -rn "STUB:"
  crates` shows nothing outside the tests.
- **Import of an export with back matter** (open item of register-satz): `baruch_typst::index::strip_back_matter`
  cuts off everything from the line `// Baruch: Register und Glossar`, `strip_projection` takes out the additions of
  the projection (automatic Bible reference marks = index `bibel` with `sort`, linked first mention
  `#link(<baruch-gl-…>)[#emph[…]]` including the protective `;`). `import::import_source` applies both,
  `strip_projection` only if back matter was found (that is only for exports by Baruch). Otherwise the back matter
  moved into the last section, and the text referred to glossary labels that do not exist in the new work (Typst
  error). The markup without a glossary page (`#emph[…]`) cannot be told apart from emphasis written by hand and stays.
- Acceptance `tests/round31_abnahme.rs` (5 tests, work „Die Wärterin vom Kap“, freely invented, two parts, three
  chapters, Bible references, two persons): glossary via the page „Markierung“ and `/glossar/act` (without
  JavaScript) and via `glossary.add` (like the dialog) → glossary page, glossary page in the PDF (pdftotext: term,
  variant, short and detailed explanation, Bible references), EPUB `glossary.xhtml` with `glossterm`/`glossdef` and
  `glossref` in the chapter; marks via `/register/act` (all 70 occurrences) and `index.mark` (subentry, „siehe auch“,
  persons) → index pages with measured pages („Nebel 3–6“), Bible reference index in biblical order („1. Mose“, „Psalmen“,
  „Johannes“, „Römer“; a reference in a comment is missing), first mention per chapter linked; the Typst export
  independently typesets the same indexes; persona (`ScriptedModel`) and MCP only propose, the human adopts by form
  (commits with `Baruch-Proposer`); round trip u7 between two throwaway instances (export including back matter
  byte-identical, tombstone, no duplicate when reconciling); without R31 settings export and EPUB stay byte-identical,
  also with a glossary and an own index; import of an export with back matter; keyboard (context menu key,
  Shift+F10) and ARIA in the delivered context menu, link „Markierung …“ without JavaScript.
- After the merge: the probe in `round31_kontextmenue.rs` (`register_forms_check_their_inputs_and_keep_them_on_422`)
  called `index.mark` and thereby created a new revision itself, which hid the one of the form – the data path was
  unchecked before the merge. The probe now reads only (`index.entries`).
- Visual check (throwaway server with `--secrets memory`, connections to 127.0.0.1:9, no request to Eliezer or to the
  network; images `.claude/sicht/r31/`): context menu in the typeset view with „In Glossar aufnehmen“, page „An der
  Markierung“, glossary page with a proposal from MCP, index page, PDF pages (keyword index „Nebel 3–4“, Bible
  reference index in biblical order, glossary page, first mention in italics), EPUB preview with glossary. **Fixed:**
  the index page named occurrences and proposals with the internal key („k13“) and the occurrence in English
  („first“) – now number and title of the section („1 Die Ankunft“) and „erste Fundstelle“; the glossary page wrote
  „Vorgeschlagen von Autor“ for terms of the human – „Vorgeschlagen von“ now stands only with origin `@…`.
- `typst` (CLI) is not in the PATH on this machine: the `typst compile` assertions report „ÜBERSPRUNGEN“, the export is
  instead typeset with the embedded compiler as a standalone file (same Typst version, no further files).

### Follow-up after review (R31)

- **No duplicate marks:** `baruch_typst::index::mark_word` checks for each chosen occurrence the chain of marks
  directly after the word or label (`carries_mark` via `Scan::marks`); if the same `IndexMark` already stands there,
  the occurrence stays untouched. If none is left → `IndexError::AlreadyMarked` (replaces the unused `NotYet`), the
  caller creates no revision; `index_proposal_accept` then sets the proposal to `stale`. "first"/"n-th" still count
  all occurrences in the text (the choice in the user interface stays unambiguous). `markInSource` (TS) checks the same
  in the source, `sameMarkBehind` (`wysiwyg.ts`) in the typeset view. Before, `Scan::insertable` allowed the place
  before an existing mark, and every repetition was a new revision including a mirror commit.
- **„Begriffe vorschlagen lassen“ usable:** forms on the glossary page and on „An der Markierung“ (`glossar/act`,
  `action=suggest`, optionally `key`); redirect with `neu`/`uebersprungen` for the notice.
- **Tests without S1 branches:** as in R27, the branches that accepted „noch nicht umgesetzt“ as correct are removed
  (`round31_kontextmenue.rs` with `implemented`, `round31_glossar.rs`, `baruch-store/tests/glossary.rs`); the 422 cases
  with kept inputs check real errors (unknown Bible reference, built-in index identifier, mark already set).

## Round 21 – Import and images

### Interface R21-S1

- **Dependencies** (fixed in S1, developers change no Cargo.toml/Cargo.lock): baruch-server `zip` 2.4 (DEFLATE only;
  moved from the dev-dependencies into the dependencies, for `src/archive.rs`), `quick-xml` 0.38 (EPUB, 21a; already in
  the lock), `pulldown-cmark` 0.13 without default features (Markdown, 21b; resolved to 0.13.6); baruch-render
  `typst-render` 0.15.1 (raster image of a page for the EPUB cover, 21c; it could be loaded, the replacement
  resvg/image was not needed). `cargo tree -d` shows no new duplicate versions.
- **`src/archive.rs`** (complete in S1, afterwards markdown-import): `read_zip(bytes, &ArchiveLimits)` →
  `Vec<ArchiveEntry {path, bytes}>` in archive order, only regular files (folders skipped). Refused with
  `ArchiveError::Unsafe {path, reason}`: absolute paths, `..`, `\`, drive, control characters, empty or non-UTF-8
  names, links, special files, encryption and duplicate names (also those differing only in case; the central
  directory is counted from the end record by the code itself, because the ZIP library silently merges duplicates).
  `TooLarge`: more entries, larger entries or a larger sum than `ArchiveLimits` – counted while unpacking with `take`,
  not from the header –, ratio unpacked/packed above `max_ratio` (entries up to `RATIO_FREE_BYTES` = 64 KiB are
  exempt). `Corrupt`: unreadable, wrong checksum, a method other than STORED/DEFLATE. `NotZip`: start is not `PK`.
  Limits for imports `IMPORT_LIMITS`: 2000 entries, 16 MiB each, 80 MiB together, ratio 200.
- **`src/import.rs`**: `ImportWarning {code, message, line}` (hint instead of silent discarding), field
  `ImportReport.warnings` (after `skipped`), `import_source(…, warnings)`; new `import_generated(state, source,
  file_name, title, assets, skipped, warnings)` for finished generated Typst text (PDF/EPUB after confirmation): a thin
  shell around `import_source`, a title already taken → `REFUSED` with `TITLE_HINT`, no heading → `INVALID_PARAMS`.
- **CLI switch** (`main.rs`, `import DATEI`): `exchange::is_exchange_name` → exchange; `pdf_import::is_source_name`
  (.pdf/.epub) → `pdf_import::cli_import` with `CliSourceImport {title, apply: --uebernehmen, library: --bibliothek}`;
  `markdown_import::is_markdown_name`/`is_archive_name` (.md/.markdown/.zip) → `markdown_import::cli_import`;
  otherwise Typst. `--uebernehmen`/`--bibliothek` with another file kind → exit 2. The Typst import prints per warning
  `Hinweis (<code>): <message>` on stderr.
- **Routes and RPC**: `pdf_import::pages()` (`GET /werke/import/quelle`) with the pages, `pdf_import::writes()`
  (`POST /werke/import/quelle/vorschau` with `DefaultBodyLimit::max(UPLOAD_LIMIT)` = 64 MiB + 64 KiB,
  `…/uebernehmen`, `…/verwerfen`) behind `guard::same_origin`; RPC `source_import.*` via
  `pdf_import::handles`/`call` after `works::handles`. Markdown routes stay in `works.rs`. Style
  `static/source_import.css` in the common `STYLE`. Menu „Baruch ▾ → PDF oder EPUB importieren…“ in `base.html`, a
  link below the Typst form in `fragments/work_new.html`. In S1 page (200), forms (501), RPC (-32000) and CLI (exit 1)
  answered honestly „noch nicht umgesetzt“ (`// STUB: … (R21a/R21b)`).
- **Migrations**: S1 creates none. Reserved, only if needed: store `0050_r21_assets.sql` (bilder),
  `0051_r21_import.sql` (markdown-import); library `0108_title_text.sql` (pdf-import, table `library_title_text`).

### 21a pdf-import

Module `src/pdf_import/` (global, without a work), library `0108_title_text.sql`, full text in
`literature/fulltext.rs`.

| File | Content |
|---|---|
| `mod.rs` | RPC `source_import.*`, `preview`/`preview_with` (choose the way, read, draft), `apply`/`apply_draft` (work via `import::import_generated`, source via `baruch_library::add_title` + `set_title_remote` or `set_title_text`), `discard`, CLI `cli_import`/`cli_import_with` (own `PdfTools` for tests), `local_pdf_reads` (counter per data folder: tests check "no pdftotext on the MultiGPT way") |
| `pdf.rs` | local: `pdfinfo -enc UTF-8` (title, author, pages) and `pdftotext -enc UTF-8 -bbox-layout DATEI -` via `tokio::process` with `TOOL_TIMEOUT` (120 s), `kill_on_drop`, standard input `null`, only pages 1–`MAX_PAGES` (2000, as via MultiGPT; more → warning `import-pdf-pages-cut`), output ≤ 128 MiB, at most `MAX_PARALLEL_READS` (2) reads at a time (further ones wait); if the program is missing → honest message. Outline from the word boxes (`structure`) |
| `remote.rs` | via MultiGPT: `upload_document` (exactly once), waiting (`run_status`, otherwise `read_document` with repetition, ≤ `INDEX_WAIT` 120 s, pauses 0.1–2 s), `document_info` (title, author, pages, `toc`), text with `read_document {page_from, page_to}` in ranges ≤ 10 pages (shortened → page by page), cut at the table of contents (`split_by_toc`), without a table of contents numbered lines (`split_by_heuristic`) |
| `epub.rs` | EPUB: `archive::read_zip` with `IMPORT_LIMITS`, `container.xml` → OPF (title, `dc:creator`, language, manifest, spine), navigation from `nav.xhtml` (`epub:type="toc"`) or `toc.ncx`, XHTML → text flow with jump targets; references via `resolve` (percent encoding, `..` only within the archive) |
| `typst_out.rs` | `escape_text` (Typst characters `\ # * _ $ @ < > [ ] ~` and the backtick, `/` before `/`/`*`, `-` before `-`/`?`, at the start of a paragraph `= - + /` and `12.`), `Inline` → markup (`_…_`/`*…*` only at word boundaries, otherwise `#emph[…]`/`#strong[…]`; `#footnote[…]`, `#super`, `#sub`, `#link`, `#raw`, `#linebreak()`), `assign`/`build_typst` (confirmed outline → Typst; part via `baruch_typst::heading_line(0, …)`) |
| `xml.rs` | small lenient XML tree over quick-xml 0.38 (`check_end_names` off, HTML entities, ≤ 256 levels, ≤ 2 million nodes) |
| `draft.rs` | drafts under `<data>/import-drafts/<32 hex>/` (`draft.json`, `asset-<n>.bin`; after an error of MultiGPT `pending.json` + `source.bin` for „lokal lesen“); `clean` before every `preview`: older than 24 h removed, then the oldest until < 20 |
| `pages.rs` | `GET /werke/import/quelle`, `POST …/vorschau` (multipart, `UPLOAD_LIMIT`), `…/uebernehmen`, `…/verwerfen`; template `templates/source_import.html`, style `static/source_import.css` |

- **Common model:** `Extracted {title, author, lang, pages, sections: [Section {level, title, part, body (Typst),
  plain}], assets, skipped, warnings, plain_text}`. The preview shows `sections` as an outline; `apply` maps the
  confirmed list (`Confirmed {level, title, part, source}`) onto the sections. Text before the first item becomes the
  item „Vorspann“. Work template of the new work: `#set document(title, author)` and – for EPUB with `dc:language` –
  `#set text(lang)`.
- **PDF heuristic (local):** line height = median of the word boxes; running text = the most frequent height class
  (half points, weighted by characters); heading = block with ≤ 3 lines, ≤ 160 characters, height ≥ 1.12 × running
  text; levels by descending height (≤ 3); single-line blocks in running text size with a number `1.2.3 Titel` without
  punctuation at the end count as a heading (level = number of number parts, number removed). The largest class exactly
  once on page 1 before any other heading = title of the document. Pure digit lines at the page edge = page numbers
  (removed). Lines of a block are joined with spaces, `Wort-` + lower-case letter without the hyphen.
- **EPUB conversion:** blocks (`p`, headings, lists as `-`/`+` with indentation, `blockquote` → `#quote(block:
  true)[…]`, `figure`/`img` → `#figure(image("pfad", alt: …), caption: […])` or `#image(…)`, `pre` → `#raw(block:
  true, …)`, `hr` → `#line(length: 100%)`, tables as lines "a | b" with a warning). Footnotes: elements with
  `epub:type` `footnote`/`endnote`/`rearnote`/`note` or `role="doc-footnote|doc-endnote"` that a reference points to
  (`noteref`/`doc-noteref` or `href` to such a note) are missing from the flow and stand as `#footnote[…]` at the mark
  (back links removed; a leading number only if it equals the footnote mark and no back link carried the number –
  „1 Kor 13,4“ stays –, with warning `import-epub-footnote-number`; images in the footnote stay in it as
  `#image(…)`). After every embedded expression (`#linebreak()`, `#emph[…]`, `#raw(…)`, `#footnote[…]` …) a
  protective `;` stands if `(`, `[`, `;` or `.Name` follows (`typst_out::needs_guard`, as in the Markdown import),
  otherwise Typst would read a call or field access. Images: path in the work relative to the folder of the OPF file,
  checked with `check_asset_path`, ≤ 8 MiB per image, ≤ 64 MiB together, only PNG/JPEG/GIF/WebP/SVG (`detect_mime`),
  otherwise skipped with a reason. Navigation: target `datei#id` → jump target in the flow; the heading equal to the
  navigation title drops out of the text; other headings stay bold text (warning). Deeper than 3 → level 3 (warning).
- **Library:** `library_title_text (title_id → library_title ON DELETE CASCADE, source_name, mime, sha256, pages,
  text CHECK typeof = 'text', created)` – only a new table, migration tests `baruch-library/tests/migration_0108.rs`
  (core fixture) and `baruch-server/tests/round21_pdf_import.rs` (real core schema of development). Functions
  `set_title_text`, `title_text`, `title_text_created`, `title_for_text_sha256` (detect duplicates).
- **Full text (`literature/fulltext.rs`):** `read(Target::Title)` reads the local full text without a link
  (`origin: "import"`, split like a local copy via `local_part`), `check_highlight` checks against it,
  `has_fulltext(state, title)` = link or local full text. Since the integration, the document view `/quelle`
  (`literature.rs`, `source_page`) asks `fulltext::has_fulltext` instead of only `title_remote` and so also shows the
  local full text in parts including the fields start/end (acceptance `round21_abnahme.rs`).
- **Rules:** outline never silent (always a preview; the CLI without flags writes nothing), part only on confirmation,
  no silent change of the way, warnings instead of discarding, no import via MCP, contents of the file are data (only
  escaped in pages and Typst).

### 21b markdown-import

- **Modules** (`crates/baruch-server/src/markdown_import/`): `convert.rs` (Markdown → Typst, pure, without state),
  `mod.rs` (names, `Bundle` for ZIP and folders, main file, images from the bundle, `import_text`, `import_bundle`,
  `import_archive`, `cli_import`, `report_text`). Creation always goes through `import::import_generated` (one
  transaction for work and images, title rules as in the Typst import). Forms and RPC in `works.rs`, report
  `templates/import_report.html`. No migration (0051 stayed free).
- **Conversion** (`convert::to_typst`, `pulldown-cmark` 0.13 with footnotes, tables, strikethrough, definition lists,
  heading attributes and YAML front matter; events → small tree → output):

| Markdown | Typst | Hint code when not mapped |
|---|---|---|
| `#`/`##`/`###` | `heading_line(1…3)` (`escape_title`) | `md-heading-level` (`####`–`######` → `===`) |
| Markup in a heading | title as text | `md-heading-format`; a footnote in it `md-heading-footnote`; `{#id}` `md-heading-attributes` |
| Heading in a quotation/list | `#strong[…]` | `md-heading-nested` |
| Text before the first heading | `= <Titel>` before it | `md-preamble-heading` |
| YAML front matter `title`/`author` | `#set document(title: …, author: …)` (work template) | `md-front-matter` for other keys |
| Paragraph, soft break, hard break | paragraph, line end, `\` + line end | – |
| `*…*`, `**…**`, `~~…~~` | `#emph[…]`, `#strong[…]`, `#strike[…]` | – |
| `` `Code` `` | `#raw("…")` | – |
| Code block | "```sprache" (fence longer than any backtick run inside) | `md-code-lang` (language with other characters) |
| List, numbered list (start number), nested | `- `, `n. `, indentation = width of the marker | – |
| `> Zitat` | `#quote(block: true)[…]` | `md-quote-kind` (`> [!NOTE]`) |
| `[Text](https:…)`, `<https:…>`, e-mail | `#link("…")[Text]` or `#link("…")` | `md-link-local` (other file, `#Sprung`, foreign scheme: only the text) |
| Footnote | `#footnote[…]` at the reference | `md-footnote-repeated`, `md-footnote-unused`, `md-footnote-nested`, `md-footnote-missing` |
| Image alone in a paragraph | `#image("p", alt: "…")`, with a title `#figure(image(…), caption: […])` | `md-image-remote` (network → `#link`), `md-image-data` (`data:`), `md-image-path` (broken % sequence) |
| Image in running text | `#box(image(…))` | `md-image-title` (title dropped) |
| Table | `#table(columns: n, align: (…), table.header(…), …)` | – |
| Definition list | `/ Begriff: Erklärung` (`:` in the term escaped) | `md-definition-multiple` (one entry per explanation) |
| Horizontal rule | `#line(length: 100%)` | – |
| `<br>` | `\ ` | – |
| HTML in text / HTML block / HTML comment | dropped (text stays) / `// …` comment / dropped | `md-html` / `md-html-block` / `md-html-comment` |
| Unknown form | text as a paragraph | `md-unknown` |

  Escaping (`convert::escape`, `escape_line_starts`): `\ # * _ $ @ < [ ] ~` and the backtick, `/` before `/`/`*`, `-`
  before `-`/`?`; at the start of a line (after indentation) `= + - /` and `Ziffern.` – idempotent. After
  `#…[…]`/`#…(…)` a `;` stands before `(`, `[`, `;` and `.Buchstabe` (like `needs_guard` of the projection). At most
  200 hints per file, then `md-warnings-truncated`. Lines are 1-based in the Markdown file (`into_offset_iter`).
- **Bundles** (`Bundle::from_archive` via `archive::read_zip` with `IMPORT_LIMITS`; `Bundle::from_files` for folders
  with the same path rules, case-insensitive duplicates and limits): main file = the only `.typ`/`.md`/`.markdown`,
  otherwise the shallowest `index.md`/`main.typ`/`README.md`, otherwise an error with candidates (-32602/422). Images:
  paths from `image_paths` of the (converted) Typst text, `check_asset_path`, normal form, relative to the folder of
  the main file (leading `/` = this folder), check `import::check_image` (8 MiB, 64 MiB together, image type) – reasons
  as in the Typst import. Unused files: one hint `archive-unused` (up to ten names). `file_name` of the work = main
  file, for `index.md`/`main.typ`/`README.md` the name of the archive or folder with their extension (title fallback).
- **Ways:** `POST /werke/import` (multipart `file` | `files` + `title`, `IMPORT_UPLOAD_LIMIT` = 80 MiB + 64 KiB only
  on this route; `TooLarge` → 413, other archive errors → 422; without hints and skipped images 303, otherwise 200
  report), RPC `work.import` (`.md` as text, images in `skipped`), `work.import_archive` (Base64 ZIP), CLI
  `markdown_import::cli_import` (Markdown: images from disk via `import::read_assets_in` as in the Typst import; ZIP:
  from the archive; report with `Hinweis (<code>, Zeile n)` on stdout).
- **`export --out`** (`import::check_targets` before the first file): `out` is not a non-folder; no target file
  exists; no path is a prefix folder of another (`a.png` vs. `a.png/b.svg`, components after normal form); no existing
  intermediate folder is a link or a file. When writing, additionally `canonicalize(parent)` under
  `canonicalize(out)` (against changes between check and write).
- **Typeset view** (`baruch-wysiwyg/src/html.rs`): `Soft` directly after `Linebreak` as an empty
  `<span class="ws-soft" data-after-break="1">` (no indentation; the widget still reads it as `soft`); an image island
  with several images names, without `caption:`, every image in the caption and all paths in
  `<span class="ws-alt ws-paths">Bilder: …</span>`. Definition lists stay without nesting (limit in
  `docs/WYSIWYG.md`).

### 21c bilder

No migration (0050 not needed): quota and clean-up work on existing rows.

- **Type change:** RPC `asset.replace {rename}`, form field `rename` („Endung anpassen“), MCP `asset_replace {rename}`
  → `Store::asset_request_with(…, rename)` (field `rename` in `params` of `replace`, missing in old rows = false;
  `AssetRequest.rename`); confirming calls `replace_asset_renaming`. Typesetting at the old path: `baruch_render`
  `with_image_formats` (see "Images (migration 0009)"), measured with `pdfimages -list` in the test.
- **Old stock:** `asset.path_conflicts`, `asset.path_resolve`, hint „Pfade mit zwei Bildern“ at the top of the page
  „Bilder“ with the form `POST /werke/bilder/act` (`action=resolve_path`).
- **Quota** (`mcp/assets.rs`, `check_quota` under `write_lock` before `add_asset` or `asset_request_with`):
  `Store::asset_mcp_usage(work, origin_name)` → `AssetUsage {writes, bytes, open_replace}` – `asset_revision` with
  `author_name` = requester and `created` ≥ now − 1 day (count, sum of `bytes`) plus `asset_request` `op = 'replace'`
  of the same `origin_name` in the period (count, size from `params.bytes` or the content), plus open replace
  requests. Limits `MAX_MCP_ASSET_WRITES_PER_DAY` 30, `MAX_MCP_ASSET_BYTES_PER_DAY` 64 MiB,
  `MAX_MCP_OPEN_REPLACE_REQUESTS` 10.
- **Clean-up:** `Store::purge_decided_asset_requests(work, ASSET_REQUEST_PURGE_DAYS = 30)` sets, in `params` of decided
  `replace` requests with `decided` < now − 30 days, `content = ""`, `purged = true`, `bytes` = size; called by
  `asset_requests::purge` before the list, a decision, the section „Anträge“, MCP `asset_requests` and the request file
  (then 410 Gone).
- **ImageView:** `rpc::LibraryView.images` (`wysiwyg::image_view` returns `image_alts` and `ImageView` in one pass);
  the reserved key `IMAGE_VIEW_KEY` in `image_alts` and `ImageView::from_alts` are gone.
- **Literature:** `literature::image_hits` (all words in name/caption/alternative text of the current revision, at most
  `MAX_IMAGE_HITS` = 24), a small template text of its own `IMAGE_HITS_TEMPLATE` (autoescape HTML); appended to
  `literature.render` (popup) and as `images_html` in `literature.html` (`/literatur`). `fragments/library.html` stays
  untouched; the first work page with `?lq=` shows the box only when the popup loads via `literature.render` (it
  always does so when opening).
- **Inserting an image:** `assets::insert_figure_at` with `Offset` moves within a paragraph past open brackets
  (`open_brackets`: `[ ( {`, `\` escaping, raw text, strings in code) to the next line boundary with closed brackets,
  otherwise behind the paragraph.
- **EPUB cover:** see "Images, cover and PDF (Round 11, migrations 0017 and 0018)"; `epub::epub_cover` → `(Option<EpubImage>, Vec<EpubWarning>)`,
  warning `COVER_RASTER_FAILED` (at the front of `EpubBook.warnings`, `book_for`), fallback the raw image file.
  `EpubInput` stays unchanged (types frozen since R20-S1).
- **`pdfinfo`:** all Typst PDFs (book, keyword slip, map, cover) carry `/MarkInfo << /Marked true /Suspects false >>`
  (krilla 0.8.2, `chunk_container.rs`: "We always set suspects to false because it's required by PDF/UA"). poppler
  25.03 reports every value except `true` as "Syntax Error: Suspects object is wrong type (boolean)". Shown in the
  test: the same file with `/Xuspects false` (same length, cross-references correct) reports nothing. Not fixed (the
  PDF is valid; PDF/UA requires the value).

## Round 22 – Revisions and mirror

Migration 0050, amending (`baruch_core::revision_policy`, `baruch-store/src/amend.rs`), side branches, re-check of
findings marked done, archive, draft as a watermark, exchange minor version 8.

### fassungen

#### Migration 0050 (Round 22)

`0050_revision_amend.sql`: `revision.amend_seq` (state, 0 = never amended) and `revision.based_on_amend` (state of the
basis of a proposal); table `revision_amend` (per step: `revision_id`, `seq` from 1, `kind` `amend`|`undo`, `undo_of`
exactly for `undo`, `body_before`, `body_after`, `author_name`, `created`, `uuid` with fill trigger), tombstone
trigger (kind `revision_amend`, work uuid via revision→node→work) and cascade `revision_amend_before_revision_delete`
(BEFORE DELETE ON revision, so that the tombstone still knows the work); `work_revision_policy` (per work: `amend`,
`max_change_percent` 1–100, `new_after_minutes` 0–1440, without uuid, not in the exchange, goes with the work);
`finding.adopted_amend`, `finding_status.adopted_amend`; `uuid_tombstone` with the twentieth kind `revision_amend`
(rebuild like 0049, without `FOREIGN_KEYS_OFF`). Checked to be lossless in
`crates/baruch-store/tests/migration_0050.rs` (a database rebuilt at state 0049; before 0050 the CHECK lists nineteen
kinds – the contract said 21).

#### Amending (Round 22)

- `baruch_core::revision_policy`: `RevisionPolicy` (default on/20 %/30 min, `validate`), `SaveMode` (`from_param`:
  missing → `New`), `NewReason` (`id`, `label`), `SaveFacts`, `decide` (order requested → no_current → policy_off →
  not_own → not_on_current → heading_changed → large_change → time_elapsed; `None` = amend), `change_percent`,
  `word_diff` (word difference for the history).
- `baruch-store/src/amend.rs`: `verdict` (facts from the database: current revision, author, basis, minutes since
  max(creation, last amendment), setting), `write_step` (replace the text, `amend_seq` + 1, row in `revision_amend`,
  `edited` stays), `Store::amend_history`, `amend_undo`, `revision_policy`, `set_revision_policy`,
  `save_human_mode`/`save_human_with_reason` (RPC `revision.save`), `outline_save_mode` (→ `outline::save_auto`:
  heading/level/split/merge → `heading_changed`, otherwise the rules). `write.rs`: `add_proposal`/`add_proposal_on`
  set `based_on_amend`. `tree.rs`: `RevisionInfo.amend_seq`, `RevisionInfo.based_on_amend`, `Store::side_revisions`
  (side branches). The old functions (`save_human`, `save_human_on`, `outline_save`) stay "new revision".
- Server: `src/revisions.rs` (RPC `revision.amends|amend_undo|policy_get|policy_set`, page „Verlauf“, forms),
  `rpc::save_mode`/`rpc::add_saved` (answer fields `saved`, `amend_seq`, `reason`, `reason_label`), `outline.rs`
  (`mode`, `as_new`, feedback `saved=` in the redirect), `mirror.rs` (`describe` for Amended/AmendUndone,
  `describe_revision_policy`, side branches), `view.rs` (state, history links, „weitere Fassung(en)“, badge "+n",
  feedback), `settings/work.rs` (block „Fassungen“), `mcp/revisions.rs` (`revision_amends`). User interface:
  `templates/fragments/editor.html` (two buttons, `mode=auto`), `templates/revision_history.html`,
  `static/revisions.css`; `workspace.ts` translates `as_new` into `mode: "new"` and shows the feedback,
  `outline.ts`/`wysiwyg.ts` pass on the triggering button when submitting again.

#### Side branches in the mirror (Round 22)

`baruch_gitmirror::Mirror::sync_side_refs(prefix, base, refs)` writes per `SideRef` (name, path, content, author,
committer, message) a commit from the snapshot `base` with the file replaced, parent HEAD, and sets the reference
(without working folder, index, HEAD); unchanged ones (same content at the place, same message) stay, missing ones are
removed; path and name check before every write. `side_refs`, `read_file`, `parents` only read. The server
(`mirror::commit_revisions`, `mirror::side_refs`) updates `refs/revisions/<key>/<n>` from `Store::side_revisions` after
every revision commit, at the start (`sync_work`) and after `revision.reject` (`mirror::sync_side_refs`). Errors of the
side branches only go to stderr; the commit counts.

### befund-rueckfrage

#### Round 22 – befund-rueckfrage

- **Re-check of findings marked done:** `assist::context::is_recheck` (check `Mode::Review`, ↻ retry
  `links.repeat_of`, RFC `OrderFrame::recheck`, never a proposal) → `own_finding_lines`: the persona's own open
  findings on the section (scope `chapter`: on the chapter), open, not attached, no praise, the latest 20, framed
  (`<<<befund … >>>`, so not in the corpus of the quotation check) under `prompts::OWN_FINDINGS_TITLE`; in the system
  part `prompts::RECHECK_RULE`. When shortening, they are dropped last (oldest first). `Prepared::own_findings` names
  the ids.
- **Splitting:** `findings::parse` treats lines „Erledigt: …“ (`done_line`, after list and Markdown characters, also
  bold) and the sentence `prompts::ALL_DONE_SENTENCE` like notes – no finding, places of the other findings without a
  gap (so `finding_spans` unchanged). `done_claims` reads the number („Befund n“, „Befund Nr. n“, „[Bn]“, „Bn“) and
  the place (first quotation).
- **Status proposal:** after `findings::ingest`, `assist::save` calls under the write lock already held
  `assist::propose_done` → `findings::rpc::propose_held` (new, like `propose` without a lock of its own) →
  `Store::finding_propose_status(…, StatusChange::Done {reason}, IdeaOrigin::Persona, note)`; one commit per proposal.
  Skipped ones (another persona, unknown, decided, a re-check already open) get a note in `Outcome::done_notes`.
- **Mandatory finding:** `prompts::critical_waived(text, had_own)` – only if the persona saw its own open findings
  (`Retries::had_own`) and the answer says the sentence; then no follow-up demand and no `CRITICAL_MISSING_NOTE`.
- **State on findings:** `Store` (`findings.rs`): `Target::adopted_amend` from `revision.amend_seq` when resolving
  `Adopted`; `write_status` writes `finding.adopted_amend`, `finding_set_status`/`finding_propose_status` write
  `finding_status.adopted_amend`, `finding_decide` on confirmation the state at that point in time.
  `findings::adopted_words` („Fassung n · Stand k“) for the mirror and `findings::view::status_words_amend`.
- **User interface:** `findings::view::entry_json` sets `question` (open proposal `done`); the macro `decide_forms`
  (`fragments/findings_panel.html`) and `finding_block.html` then show „Rückfrage“ with „Ja, erledigt“ / „Nein, noch
  offen“ (forms to `/werke/befunde/act`, `data-finding-form`; in the finding card of the chat the box carries
  `data-widget="findings"`).
- **Archive:** `Store::restore_retired` (one transaction: check `work_retired`, title free, delete the row,
  `import::import_typst` with the title from `work_retired` and the note `works::RESTORED_NOTE`, set the reserved
  slug). Server `works.rs`: `archive_folder`, `read_archive` (path protection, assembling like the export,
  `strip_back_matter`, `strip_projection`/`strip_first_mentions` like `import.rs`), `archives`, `restore` (lock → read →
  store → `rename` to `works/<slug>` → commit „Werk wiederhergestellt“ with `allow_empty`), page `works_archive.html`.
- **Mirror of ideas without a work:** `mirror_ideas::sync_ideas_without_work` (commit of the human, `allow_empty:
  false`, trailer `Baruch-Action: ideas-synced`; without ideas without a work and without a folder nothing) from
  `sync_all` (start) and `works::delete` (after the log commit; answer `ideas_commit`).

### spiegel-entwurf

#### Draft as a watermark (Round 22)

- **Model:** `baruch_typst::typeset::Draft` (S1) as the field `TypesetSettings.draft`; `validate`/`normalized` check
  and trim (spiegel-entwurf). `rules` sets no line for it (the draft is taken out like `xref_with_page`),
  `settings_from_rules` does not know it – export, export map, mirror and rule block stay byte-identical.
- **Rule:** `baruch_typst::typeset::draft_rule(&Draft, Option<&str>)` (`typeset/draft.rs`) returns
  `// Baruch: Entwurf (nur in den Ausgaben)` + `#set page(foreground: …)`: text, addition and date only as escaped
  Typst strings (non-breaking spaces against line breaks), opacity as the alpha of `rgb(…)`, position diagonal (−45°,
  size from the number of characters, at most 80 pt) or header/footer (7 mm from the edge). `foreground`, because
  background, header and footer already belong to the template and the typesetting rules (the page counting from the
  first chapter stands in the background). Only the server determines the date (`typeset::draft_today`, local time) and
  passes it in.
- **Server:** `typeset::prepare_with(state, work, draft: Option<bool>)` puts the rule with `cover::insert_parts` after
  work template and rule block **before** the frame (so cover pages and front matter in the book PDF carry it too) and
  shifts the map – diagnostics stay with the right section. `effective_draft` (three-valued), `cover_draft` (cover only
  with `draft.cover` or explicitly), `draft_param`/`draft_query`/`draft_pair` for RPC and routes. Keyword slip and
  cover PDF put the rule before the first line of their own source; the EPUB gets the draft via
  `EpubInput.typeset.draft` (date written into the addition by the server, baruch-epub stays without a clock). Print
  check `cover_print::cover_checks_with(…, draft)` with code `draft` (`CODES` now 11).

#### Exchange minor version 8 (Round 22)

- `baruch-exchange`: `FORMAT_VERSION = 8`; `ExchangeRevision.amend_seq/based_on_amend/amends` (`ExchangeAmend`,
  `ExchangeAmendKind`), `ExchangeFinding(Entry).adopted_amend`, tombstone kind `Amend` („fortschreibung“ ↔
  `revision_amend`, from 8), `typeset::draft_attrs`/`draft_from_attrs` for `<entwurf/>`;
  `sample::sample_document_u8` (the old sample stays without u8 parts). XSD `docs/schema/baruch-werk-1-u8.xsd`, u7
  frozen.
- Store: `exchange_dump` reads state, history and `adopted_amend`; `exchange_apply` sets them in the post-pass
  `apply_u8` only on rows just created, fast-forwards known revisions (`fast_forward`, rule d0) and, for a new part with
  adopted chapters, builds a request `Add` with `requests::PartIdentity` (uuid, revisions including history). On
  consent, `requests::run` calls `exchange_apply::adopt_part` after `outline::add_many`: the generated revision 1 is
  dropped (including its tombstone, it never existed outside the transaction), `node.last_rev` set to 0, then
  identifier and revisions from the request. Only `exchange_apply.rs` writes uuids (the guard test stays green).

#### Mirror (Round 22)

- **Show rule via the jump mark:** a node „ohne Nummer“ whose heading cannot be marked (title with code), with the
  same heading as a numbered node, gets, instead of the rule over level and title,
  `#show <kN>: set heading(numbering: none)` under `// Baruch: Überschriften ohne Nummer über die Sprungmarke`
  (`snapshot::label_rules`, before the block of the fallback). With a label of its own the limit stays. Way back on
  import: `snapshot::split_label_fallback` in `import.rs` (the named headings come back „ohne Nummer“).
- **Typesetting the mirror:** section files lie in `abschnitte/`; the mirror writes relative image paths
  `image("bilder/x")` from the root (`image("/bilder/x")`, `snapshot::root_image_paths`), so that Typst can typeset
  `main.typ` including `#include` (test `the_mirror_main_typ_with_its_includes_sets_like_the_export`: the same page
  count as the export). Documented deviations: raster images lie in the mirror as `bilder/<name>.b64` (Base64 text) and
  must be converted back before typesetting; an image path that is not `bilder/<Name des Bilds>` does not stand under
  this path in the mirror; the part page rule and the frame (cover, front matter) belong only to the typesetting, not
  to export and mirror.

## Round 23 – Hardening

No migration (store still up to 0050, library up to 0108), no new minor version of the exchange format, no new RPC
methods or MCP tools.

### web

**Reading requests of foreign pages (`guard::own_reads`).** The layer lies over all routes and refuses a GET or HEAD
with 403 „Abgelehnt: Anfrage von einer fremden Seite (lesend)“ if `Sec-Fetch-Site` is `cross-site` or `same-site`
and the request is not a navigation of the top-level window (`Sec-Fetch-Mode: navigate` with
`Sec-Fetch-Dest: document`). Image, script, `fetch` and frames of a foreign page thereby no longer reach any reading
route (e.g. `GET /einstellungen/assistenten?modelle=abfragen`, keyword slip); a link from outside still opens Baruch.
Without `Sec-Fetch-Site` (curl, MCP clients, old browsers) the request counts as our own. Two delimitations:
(1) Files of the EPUB preview (`/werke/{id}/epub/leser/…`) stand in a frame with `sandbox`; the browser reports their
images and jumps as `cross-site`. There the route itself still decides via the `Referer`
(`epub_preview::from_own_frame`). (2) If `Sec-Fetch-Mode` and `Sec-Fetch-Dest` are **both** missing, the kind is not
proven and the request does not count as a subresource – no browser sends `Sec-Fetch-Site` without `Sec-Fetch-Mode`;
the routes with a check of their own (`cross_site_get`) still refuse such a request themselves.

**Message codes.** Pages never show text from the address as a message, only fixed codes (`?ok=<code>`,
`?fehler=<code>`, `?hinweis=<code>`), which the server maps to fixed sentences. The last exception
(`?ok=sicherung&ziel=<Ordner>`) is removed: `settings::newest_backup` reads the folder name from the file system.

**Badges of the header.** Outside the work page, `base.html` marks the badges with `data-badge-fill` and the menu bar
with `data-badges-work="<Werk>"`. The layer `consent::inject` (over all routes, with `AppState`) calls, after the
notice bar, `menu::fill_page`, which puts the numbers from `menu::badges` into the finished HTML
(`menu::fill_badges`) and removes the marker. So none of the roughly thirty template environments needs data of its
own.

**Amending and changing via the tree.** `Store::update_revision_at` (and the old shell `update_revision`) creates, for
the current or an already amended revision with changed text, a step in `revision_amend` (kind `amend`, no
migration) and increments `revision.amend_seq`; `Change.amend_seq` names the new state, the kind of change stays
`Updated` (mirror commit as before). Thus the chain of steps has no gaps (fast-forward when reconciling, exchange),
and an editor with an old state gets `NotOnCurrent` instead of silently overwriting. With `expected_amend` the store
refuses an outdated state (`StoreError::Stale`, „Veraltet: …“).

**Typeset view: quotation block** (also `docs/WYSIWYG.md`). `baruch_wysiwyg::parse` reads a logical line that is
exactly one `#quote(block: true, …)[…]` as a piece of its own (like `#szene[…]`/`#arbeit[…]`): a line directly after
it is a paragraph of its own, a line directly before it ends its paragraph – as Typst typesets. `#quote[…]`,
`block: false` and unreadable arguments stay part of their paragraph. Untouched blocks go back byte-identical. The
button „Zitat“ inserts `#quote(block: true)[]` with the following line "– ", not `attribution: []`: an empty
`attribution` sets a line with the em dash in Typst 0.15.1 (measured in
`crates/baruch-render/tests/quote_attribution.rs`: with `attribution: []` a "—" stands in a line of its own more than
5 pt below the quotation, without `attribution` none).

**Editor: lock while saving.** `widgets.ts` (`lockEditor`, `editorLocked`) locks the area `#pane-editor` (`inert`,
`aria-busy`) from pressing „Speichern“ until after the redraw; `outline.ts` and `workspace.ts` count their locks, a
second submission during the run is dropped. `workspace.ts` discards outdated `ui.render` answers via a running number
(the navigation triggered last counts).

**Idea map.** The proximity (`idea_map::near::compute`) computes in `spawn_blocking` and at most for the first
`MAX_NEAR_IDEAS` = 1500 ideas by number (roughly 1.1 million pairs); above that the message of the map says so.

**Cover.** `cover::compile_limited` typesets the measuring sources (`book_page_mm`, `block_heights`) with the time limit
of the typesetting (`cover::MEASURE_TIMEOUT` = 60 s); after that the run is marked as cancelled and the result
discarded, `block_heights` reports -32000 „… Typst wurde nach 60 Sekunden nicht fertig (Zeitgrenze).“ The contrast
check unpacks the cover image with limits like the preview images (`cover_print::MAX_CONTRAST_SIDE_PX` = 20 000,
512 MiB) and remembers up to eight unpacked images (together at most 96 MiB RGB) by checksum. `default_blocks` rounds
position and width to 0.01 mm.

**EPUB.** Footnotes get their place in the list before their content is written (inner footnote after the outer
one). `collect_labels` counts figures (`#figure(image(…) …)`, alone or with a label after it) in reading order
(`Links::figures`); `@name` shows „Abbildung n“ (or the reference's own word) and links to `<figure id="lbl-name">`.
`check_mimetype` unpacks a compressed `mimetype` and also checks its content (`MIMETYPE_CONTENT` in addition to
`MIMETYPE_COMPRESSED`).

**Typesetting messages from the back matter.** `ExportMap::tail_first_line`/`in_tail`: lines after the last section
(index, glossary) no longer belong to any section; `typeset::Typeset::diagnostics` names them „Buchende (Register,
Glossar)“ with the line in the back matter.

**Findings.** `finding_list`, `finding_attached_to` and `findings_by_uuid` fetch the open proposals in one query per
500 findings (before, one per finding). `Finding::stale_summary` is derived (an open collective finding without an open
origin; deleted origins count as decided) and is never stored. `finding_open_counts` and the matrix do not count open
collective findings.

**Bible and commentary in the versification of the module.** The evidence from SWORD (`bible_research::collect`)
reads the reference via `bibles::read_passage` (mapping german() → versification of the module) like the check and
`bible.lookup`; commentaries via `bibles::read_commentary` (the same mapping, labelled with the name of the
versification). The wording check names missing verses in German versification (mapped back), otherwise with the name
of the module versification.

**Ribbon.** `ribbon::defined_functions` evaluates only top-level code (`code_lines`: without line comments and –
also nested – block comments, without bodies of other functions, strings stay whole) and knows
`#import "datei.typ": name, a as b`.

### mcp-namen

- **Handles** (`mcp/session.rs`): `handle_name`, `legacy_handle_name`, `LOCAL_PREFIX`, `local_name` (pure functions),
  `Client::handle`, `Client::owned_handles`. `mcp.rs::localize` delimits clients without a token against the token
  list on every request (one small query per request).
- **Tokens** (`store/mcp_tokens.rs`): `mcp_token_create_unique(name, handle, legacy, reserved_prefix)` checks and
  creates in one `BEGIN IMMEDIATE` transaction; the server passes the rules in (`mcp/tokens.rs::create`, the only path
  of RPC and form). `mcp_token_create` (without the handle check) stays for tests and existing code.
- **Ownership** (`posts.rs`): `Actor` carries `also` (further own author names, the old handle of a token) and
  `owns()`; `posts::edit`, `posts::act`, `threads::delete_post` and `threads::edit_post_as` check through it.
- **Quota** (`store/asset_requests.rs`): `asset_mcp_usage_without_token(work, token_handles)` counts all image
  revisions whose author begins with "@" and all requests of personas and MCP clients, without the authors of the
  tokens.
- **Upper limits in the store:** `glossary::MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN` (200), `…_PER_WORK` (2000),
  `requests::MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN` (50; not when reading an exchange file and not for the human).
- **Relay** (`assist/chat.rs`): `relay_targets` chooses the targets, `ChatOrder::relay` switches mentions on (MCP).
  `relay_section` in `mcp/tools.rs` is removed. Threads without a section still have their own loop in `threads.rs`
  with the same limits.
- **Job of a post** (`store/post_meta.rs`): `PostRequest` + `web`, `asker`, `relay`;
  `assist::FramedAnswer::{to_request, from_request}`.
- **Follow-up, existing tokens** (`mcp/session.rs`): `keeps_legacy(name, token_names)` and `token_handle` (pure
  functions), `Client::keeps_legacy`. A token whose new handle would only through the rule since Round 23 equal the
  handle or old handle of a token with another old handle still writes and owns only under its old handle.
  `mcp.rs::localize` determines this for tokens on `initialize` and on every request from the same token list (no
  stored state, no migration); the page „MCP-Zugang“ (`settings/access.rs`) shows `taken`.
- **Follow-up, quotas without a token:** `mcp/quota.rs::acquire(state, &Client)` occupies per job the slot of the
  handle (at most `MAX_PER_CLIENT` = 2) and, without a token, additionally one of the common slots (`WITHOUT_TOKEN`,
  at most `MAX_WITHOUT_TOKEN` = 4), both under one lock; `Slot` frees both. `mcp/tools.rs::Counted` chooses for
  `add_post` and `propose_revision` whose stock counts: `Author` (token or named persona) or `WithoutToken` (all
  authors `@mcp:…` except those of the tokens, `token_handles`, also used by the image quota).
- **Context** (`assist/context.rs`): `kept_own` updates `Prepared::own_findings` after shortening.
- **Variants** (`assist/variants.rs`): `splice` replaces only the byte ranges of the affected paragraphs
  (`paragraph_spans`).
- **Findings when moving** (`store/posts.rs::post_move`): one `UPDATE finding` in the same transaction.

### geheimnisse

- **Redacting secrets in one place (Round 23).** `baruch_llm::scrub(text, secret)`
  (`crates/baruch-llm/src/secret.rs`) replaces with `***`: every literal occurrence and – from a secret length of
  6 bytes (`SCRUB_ENCODED_MIN`) – the percent encoding (per byte literally or `%XX` with upper or lower case hex
  digits, spaces also as `+`) and Base64 (standard and URL-safe, with or without `=`, also as part of a longer Base64
  like `Basic base64(user:geheim)`: for each offset 0/1/2 the characters that depend on the secret alone are searched
  for; the mixed edge characters and padding characters go along). Shorter secrets only literally. Not recognised:
  hex, JSON escapes, double encoding, wrapped Base64. Callers: `Endpoint::scrub` (model client),
  `connections::check::scrub`/`scrub_value`, `connections::multigpt::scrub_text`/`scrub_error` (and thereby
  `Session::scrub`, web search, PDF fetch), `PgvectorWay::scrub` (via `multigpt::scrub_error`), `sync::Link::scrub`
  (via `multigpt::scrub_text` or the session), `models::check_connection` (via `check::scrub_value`). New dependency
  `base64 0.22` in `baruch-llm` (it was already in the lock).
- **No redirects.** All three HTTP clients with a key (`connections/check.rs`, `connections/multigpt.rs`,
  `baruch-llm`) have `redirect::Policy::none()`; a 3xx is an error with a message of its own, `Location` is not shown.
  Tests per client with two mocks (the second sees no request).
- **Filter before `fetch_url`.** `websearch::fetch::fetch_allowed(url) -> Result<(), &'static str>` checks only the
  wording of the address: scheme http/https, no user information, no IP literal in loopback, private, link-local,
  CGNAT, unspecified, multicast, broadcast, 0/8, 192.0.0.0/24, 198.18.0.0/15, 240/4; IPv6 `::1`, `::`, `ff00::/8`, ULA
  `fc00::/7`, `fe80::/10`, `fec0::/10`, plus IPv4 in IPv6 (`::ffff:`, `::a.b.c.d`, NAT64 `64:ff9b::/96`, 6to4
  `2002::/16`) and the whole of `64:ff9b:1::/48`;
  `localhost`, names on `.localhost`/`.local`/`.internal`/`.home.arpa` and (beyond the contract) names without a dot.
  The URL parser normalises decimal, hexadecimal, octal and shortened IPv4 notations beforehand.
  **No DNS resolution** – names that point to private addresses are only recognised by the fetching side; whether
  MultiGPT filters itself is unknown (**assumption**: no). `websearch::search::gather` does not fetch refused hits,
  they use up none of the four fetches and appear as a hint. `websearch::fetch::set_fetch_exceptions(state, hosts)` is
  an exception path per data folder for tests only (no RPC, no form, no setting leads there).
- **Model cache per (kind, address)** (`models.rs`: `CACHE`, `CONTEXTS`); `expire_list(url)` affects all kinds of the
  address, `remember(url, …)` belongs to the connection `eliezer`. `models::context` checks
  `external`/`external_allowed` before every request.
- **Sermon with Bible:** `Store::create_work_with_bible(new, human, sermon_text, bible)` writes `work_bible` in the
  transaction of the creation; `create_work_with_text` calls it with `None`.
- **EPUBCheck temp folder** (`epub_check::TempDir`): `baruch-epubcheck-<32 Hex-Zeichen aus getrandom>`, under Unix
  `DirBuilder::mode(0o700)`, without `recursive` – an existing folder, file or symlink of the same name is an error
  („Temp-Ordner für EPUBCheck: …“) and is neither used nor cleaned up.
- **SSE parser** (`baruch-llm/src/sse.rs`): the limit `max_line` also applies to the sum of the `data:` lines of an
  event („Ein Ereignis im Antwortstrom ist zu groß; abgebrochen.“).

### Integration

- **Router:** `guard::own_reads` lies as a layer over all routes (immediately before `guard::local_host`, interface
  R23-S1b). `/mcp` has its own size limit `MCP_BODY_LIMIT` (12 MiB). `/act` knows `based_on_amend`.
- **Data folder:** under Unix `baruch_store::Store::open` creates a new data folder with 0700 (`create_data_dir`, code
  review quellen-5); an existing one stays unchanged.
- **Acceptance:** `tests/round23_abnahme.rs` (quotation to tokens in one flow; secrets, redirect, web search).

## 0.9.0 – Hiding older revisions (user 2026-11-24)

- **Migration 0051** (`0051_revision_fold.sql`): table `node_revision_fold` (`node_id` as primary key with
  `ON DELETE CASCADE`, `below` ≥ 1 = number of the revision at which the marker stands, `updated`, `updated_by`).
  Without uuid, not in the exchange format (way of working of the instance like `work_revision_policy`), no commit in
  the mirror (no book text). Checked to be lossless in `crates/baruch-store/tests/revision_fold.rs` (database at state
  ed9d210, store up to 0050).
- **Rule** `baruch_core::revision_fold`: `cutoff` = marker, at most the current revision; hidden are replaced and
  discarded revisions below `cutoff` (`is_hidden`, `hidden_count`); never the current one, never open proposals.
  `offered` = „bis hierhin ausblenden“ only at the current revision and below it, if there is something to hide below.
- **Store** `baruch-store/src/revision_fold.rs`: `revision_fold`, `work_revision_folds` (key → marker),
  `set_revision_fold` (above the current one `Refused`), `clear_revision_fold`, `clear_work_revision_folds`,
  `fold_revisions_before` (work-wide, last revision before a UTC point in time, at most the current one).
- **Server** `src/revision_fold.rs`: RPC `revision.fold_get|fold_set|fold_clear|fold_before|fold_clear_all` (no MCP
  tool), form `POST /einstellungen/werk/fassungen/ausblenden`; `/act` takes `fold_set`/`fold_clear`. `view.rs`:
  `ViewExtras.revision_folds`, `Location.fassungen` (`alle` = switch open, `Resolved.show_all`, `FOLD_OPEN_Q` in
  `Resolved::url`), `FoldView` (counter and neighbours over the visible revisions, links for the switch, revisions
  with an offer), `TreeRevision.hidden`/`fold`. Templates: `fragments/editor.html` (addition at the counter, „bis hier
  ausblenden“), `fragments/revisions.html` (forms `rev-fold-<n>`, „Wieder alle zeigen“, switch
  `<details class="rev-fold">`), `fragments/revtree.html` (macro `folded`), `settings/werk.html`; CSS in
  `static/revisions.css`; `web/src/rpc.ts` passes `fassungen` through to `ui.render`.

## Round 33 – Ribbon

The header with „Werk ▾“ / „Ansicht ▾“ / „Werkzeuge ▾“ and the flat row of buttons above the sheet are replaced by a
ribbon: one menu **„Baruch ▾“** (program), tabs **„Start · Einfügen · Prüfen · Buch · Ansicht“** with groups, and the
buttons of the editor dock into the ribbon. No new route, no new RPC method, no new MCP tool; `ui.render`,
`menu.badges` and `ribbon.formats` are unchanged. The laptop mode (compact ribbon, side rails) belongs to
`src/laptop.rs`/`web/src/laptop.ts` and is described there.

### Template (`templates/base.html`)

The ribbon stands completely in `base.html` (the template is embedded in roughly 35 minijinja environments of their
own; an `{% include %}` would have to be registered in all of them).

- `<header class="band" data-widget="band" data-tab="" data-work="W">` (the class `topbar` no longer exists),
  directly below it the marker `<!--baruch-consent-->`.
- Row 1 (`div.band-top`): `nav.menubar[data-widget=menubar]` with exactly one `details.menu[data-menu="baruch"]`
  („Alle Werke“, „Neues Werk / Importieren…“, „PDF oder EPUB importieren…“, „Austausch: Einlesen…“,
  „⚙ Einstellungen“, „Über Baruch“), the tab row `nav.band-tabs[role=tablist][data-band-default]`, the work choice
  (`form.work-select`, work page only), `span.work-title`, the author and the switch `a[data-band-toggle]` (ribbon
  open/closed). The tab row stands in the document directly after the menu (not after the title): so it stays at the
  same place on every page, and the tab order matches the picture.
- Row 2 (`div.band-panels`): per tab one `div.band-panel[role=tabpanel]` and – only on the work page – the docking
  place `div.band-dock[data-band-dock]`.
- Tabs per page: work page (`topbar` set) all five; pages with a work „Start · Prüfen · Buch · Ansicht“; without a
  work „Start · Ansicht“ (there „Literatur“ and „Abgleichen“ stand in a group „Werkzeuge“ under „Start“, because they
  need no work; „Werk wählen“ leads to `/werke` as before).
- Blocks of the pages: `band_tab` (default tab, otherwise `start`), `page_actions` (groups in the tab „Start“, between
  „Werk“ or „Ansicht des Textes“ and „Einfälle“), `band_einfuegen`, `band_pruefen`, `band_buch`, `band_ansicht`
  (each before the fixed groups of the tab).
- Places for Round 32 as comments of the template: `{#- R32-PLATZ import-word -#}` and `{#- R32-PLATZ beenden -#}`
  (last entry) in the menu „Baruch ▾“, `{#- R32-PLATZ export-word -#}` in the group „Ausgabe“.

**Literal forms.** The layer replaces strings; the order of the attributes is therefore part of the interface (the
unit test `the_template_carries_the_literal_forms_apply_relies_on` pins it):

| Piece | Form |
|---|---|
| Tab | `<a class="band-tab" role="tab" id="band-tab-ID" aria-controls="band-panel-ID" data-band-tab="ID" href="?tab=ID" aria-selected="false">` |
| Panel | `<div class="band-panel" role="tabpanel" id="band-panel-ID" aria-labelledby="band-tab-ID" data-band-panel="ID" hidden>` |
| Group | `<details class="band-group" data-band-group open><summary class="band-group-label">Name</summary><div class="band-items" role="group" aria-label="Name">…</div></details>` – further attributes only between `class="…"` and `data-band-group` (e.g. `data-band-after`: after the docking place) |
| Ribbon open/closed | `data-band-toggle aria-expanded="true" href="?band=closed"` |
| Laptop switch | `<a class="btn" data-laptop-set="auto" aria-pressed="false" href="?laptop=auto">Automatisch</a>` (likewise `on`, `off`) |
| Side rails | `<a class="btn" data-rail-toggle="left" aria-pressed="false" href="?lrail=1">Gliederung</a>`, on the right `right`/`rrail` |

### Layer `band::apply` (`src/band.rs`)

Pages do not know their address and `?tab=`. The layer `consent::inject` (over all routes, like the badges since
Round 23a) keeps the cookie header and the raw query before `next.run`, builds `BandState::from_request` and
`LaptopState::from_request` and, after `menu::fill_page`, calls `band::apply(&page, &band, &laptop)`; valid keys of
the address are appended as `Set-Cookie` (`band::set_cookies`, `laptop::set_cookies`).

- `BandState { tab, closed, rest }`: `?tab=` only for identifiers from `TABS` (not remembered), `?band=open|closed`
  before the cookie `baruch_band` (`closed`; missing = open), `rest` = raw query without `tab`, `band`, `laptop`,
  `lrail`, `rrail` (not decoded). Invalid values are skipped; with repeated keys the last valid one counts.
- `apply`, in this order; a page without the marker ` data-band-fill` stays unchanged:
  1. marker on `<html>` → `laptop.html_attrs()` and `data-band="open|closed"`;
  2. active tab: `?tab=` if the header has it; otherwise `data-band-default` if present; otherwise the first;
  3. `data-tab=""` on the header → `data-tab="<aktiv>"`;
  4. addresses of the tabs `href="?<rest>&tab=ID"`; the active one additionally `aria-selected="true"` and
     `&band=closed` (a click closes); with a closed ribbon all tabs carry `&band=open`;
  5. `hidden` is dropped on the active panel;
  6. compact (`laptop.compact()`): every ` data-band-group open>` → ` data-band-group name="band-group">` (popups
     from `<details>`, the common name keeps only one open);
  7. every `href="?K=V"` with K ∈ {band, laptop, lrail, rrail} → `href="?<rest>&tab=<aktiv>&K=V"`;
  8. `aria-pressed` on the applicable laptop switch; side rail visible → `aria-pressed="true"`, collapsed → address to
     `lrail=0` or `rrail=0`; closed ribbon → switch to `band=open`, `aria-expanded="false"`.
- `node=` (old address form) does not go along into the generated addresses: generated links never carry `node=`
  (contract Round 8 §1). `rest` itself stays the raw query; a page that was still called with `?node=` loses only
  this place on a tab change without JavaScript. All other keys stay – also one-off ones like `?ok=gespeichert` (the
  message then stands there once more after a tab change without JavaScript).
- **Security:** `rest` comes from the address and goes into an attribute only via `escape_attr` (`&`, `<`, `>`, `"`,
  `'`). Steps 2 to 6 act only between `<header class="band"` and `</header>`: text in the page content that looks like
  a tab changes nothing. Steps 7 and 8 apply on the whole page (also for the narrow rails of the work page); a value V
  with characters other than `[A-Za-z0-9_-]` is not touched.
- **Known limit:** on a page that is the answer to a POST (e.g. the question „Speichern bestätigen“), a tab link
  without JavaScript leads by GET to the same address; with JavaScript band.ts switches without reloading.

### Badges

The badges stand on the tabs: open proposals on **„Start“**, „N offen“ (findings) on **„Prüfen“**, open outline
requests on **„Buch“**; plus the number on the entries „Befunde“ and „Anträge zur Gliederung“. The hooks are the old
ones (`data-badge`, `data-badge-item`, `data-badge-fill`, `data-badges-work`): the work page draws from
`topbar.badges`, every other page is filled by `menu::fill_badges` in the finished HTML (without JavaScript),
`menu.ts` refreshes („N offen“ on the tab, only the number in the entry).

### Docking the editor buttons

The editor widgets hang on `div.wysiwyg`, and `ui.render` redraws only the three columns. Therefore the buttons stay
in the fragment `fragments/edit_toolbar.html` (editor branch, `div[data-edit-toolbar][hidden]` in `div.wysiwyg`) and
are redrawn with the editor; `ui.render` is unchanged.

- The bar has two parts: `div[data-band-part="start"]` (groups text: „Satzbild“ | „Quelltext“; paragraph: paragraph
  format, „K“, „F“; revision: ‹ n/m ›, „Speichern“, „Als neue Fassung“ – `form="editor-form"`; assistants: „Prüfen“,
  „Vorschlag“) and `div[data-band-part="einfuegen"]` (insert: formats from `ribbon.formats`; image; index and
  glossary). All hooks are the previous ones (`data-ws-mode`, `data-ws-tools`, `data-ws-format`, `data-ws-cmd`,
  `data-ribbon-cmd`, `data-ribbon-snippet`, `data-ribbon-image`, `data-widget="ribbon"`,
  `data-widget="context-menu"`, `data-ctx-action`, `data-index-*`). The ‹ › links in the ribbon carry no `data-key`
  (Alt+←/→ still takes the links in the head of the editor).
- `fragments/editor.html` ends in both views with `<span data-widget="band-dock" hidden></span>`. The widget
  `band-dock` (band.ts) empties `div[data-band-dock]` when mounted and puts the bar of the shown editor into it
  (preview: only empty); the header then carries `data-docked`. Because `mount` mounts the widgets in document order,
  `wysiwyg`, `ribbon` and `context-menu` have connected to their editor before.
- Which part is visible is controlled by `band.css` via `header.band[data-tab]`. The panels draw no box
  (`display: contents`): groups and docking place stand in one row, the docking place between „Ansicht des Textes“
  and „Einfälle“ (`order`).
- Updated in the widgets: `widgets.ts` – `editToolbar(root)` finds the bar at the sheet or in the docking place
  (wysiwyg.ts uses it for the switch typeset view/source), `submitButton` skips the buttons of the bar („Speichern“ of
  the status line stays decisive), `lockEditor` also locks the docking place; `workspace.ts` listens for `a[data-nav]`
  on the document (switch and ‹ › stand in the header); `sheet.ts` remembers the cursor position for „Bild“ via a
  listener on the document; `ribbon.ts` lets the arrow keys move only over reachable buttons.
- „Prüfen“/„Vorschlag“ (`[data-band-assist]`) trigger the button of the same name of `form.assist-form` on the right;
  „Speichern“ in the ribbon does not save as long as the button of the status line is disabled (nothing changed).
- Without JavaScript the bar stays hidden at the sheet (as before: text field and status line); the tab „Einfügen“
  then shows a hint (`nojs-only`) with the links „Bild einfügen …“ (`#bild-einfuegen`) and „Markierung …“.

### Widgets `band` and `band-dock` (`web/src/band.ts`)

- Tabs after the ARIA pattern for tabs: a click switches without reloading, ←/→ (with wrap-around) and Home/End move
  and select, only the selected tab is in the tab order.
- A click on the selected tab or on the switch collapses the ribbon (`data-band` on `<html>`, cookie `baruch_band`);
  when collapsed, a tab shows its panel temporarily over the content (`data-band-peek` on the header) – Esc, a click
  beside it or a triggered entry hides it.
- Groups: full, all open, the label is not a switch; compact (`data-compact` on `<html>`) popups – only one open, Esc
  and a click beside it close. band.ts never measures widths; it reacts to the event `baruch:laptop` (laptop.ts) with
  `syncGroups`.
- Style `static/band.css`: only the attributes on `<html>` (`data-compact`, `data-band`), no media queries; only colour
  variables from `app.css`; helper classes `js-only`/`nojs-only`.

Tests: unit tests in `src/band.rs`; `tests/round33_menueband.rs` (ribbon per page kind, `?tab=`, `?band=` with cookie,
escaping, mapping of all old entries, badges, fragment of the editor, behaviour of the widgets in the Node mock, scene
"band" in `tests/round23_web/widgets.mjs`).

### Laptop mode (Round 33)

One source of truth: **attributes on `<html>`**. The server writes them when delivering, the widget keeps them current
in the browser, CSS and other widgets only read them. There is **no media query on the window width** for the modes –
otherwise there would be two truths as soon as the human chooses „An“ or „Aus“.

| Attribute | Meaning | set when |
|---|---|---|
| `data-laptop="auto\|on\|off"` | choice of the human | always |
| `data-compact` | ribbon compact | `on`, or `auto` and width `m`/`s` |
| `data-narrow` | narrow: both side columns are rails | `on`, or `auto` and width `s` |
| `data-rail-left` | outline collapsed | narrow, or by hand (`lrail`) |
| `data-rail-right` | right column collapsed | narrow, or by hand (`rrail`) |
| `data-drawer="left\|right"` | drawer open (only laptop.ts, never from the server) | as long as the drawer is open |

#### Server: `src/laptop.rs`

- `LaptopState::from_request(cookie_header, query)` reads the state: keys of the address `laptop` (`auto|on|off`),
  `lrail`, `rrail` (`0|1`) apply **before** the cookies `baruch_laptop` (`on|off`, missing = automatic),
  `baruch_lrail`, `baruch_rrail` (`1`, missing = visible). The width class comes only from the cookie `baruch_vw`
  (`w|m|s`, missing = wide). Invalid values are skipped; if a key occurs several times in the address, the last valid
  one counts (the ribbon appends its keys at the end).
- `compact()`, `narrow()`, `rail_left()`, `rail_right()` are the formula from the table; `html_attrs()` writes them in
  a fixed order and **only from fixed strings** – nothing from address or cookie gets into the page.
- `set_cookies(query)` returns finished `Set-Cookie` values for valid keys of the address
  (`…; Path=/; Max-Age=31536000; SameSite=Lax`); the defaults (`laptop=auto`, `lrail=0`, `rrail=0`) delete the cookie
  (`Max-Age=0`).
- Thresholds: `COMPACT_BELOW_PX = 1600`, `NARROW_BELOW_PX = 1280`; `Width::from_px` maps.
- Wiring: `consent::inject` (layer over all HTML responses) builds the state before `next.run`, `band::apply` inserts
  `html_attrs()` at the marker ` data-band-fill`, the cookies are **appended** to the response (`/` already sets
  `baruch_work`). No new route, no RPC method, no migration.

#### Browser: `web/src/laptop.ts`

- Attaches itself to the `document` when loading (pages without a workspace have no element for it); the widget
  `laptop` stays registered and triggers the same.
- Measures `window.innerWidth` when loading and on `resize` (bundled via `requestAnimationFrame`), writes the class
  into the cookie `baruch_vw` and sets the attributes by the same formula (`effective`, `htmlAttrs`). The thresholds
  also stand in the script as `COMPACT_BELOW_PX`/`NARROW_BELOW_PX`; `tests/round33_laptop.rs` compares them and all 36
  combinations of the formula with the Rust side.
- **Event `baruch:laptop`** on the `document`, `detail: {compact, narrow}`: once when loading, then on every change of
  `compact` or `narrow`. band.ts then folds its groups. The event at load goes out before widgets are mounted – whoever
  needs it additionally reads the attributes on `<html>` when mounting. laptop.ts never touches the groups of the
  ribbon; band.ts never measures widths.
- Clicks (one listener on the `document`): `[data-laptop-set]` (mode, cookie `baruch_laptop`), `[data-rail-toggle]`
  (switch in the ribbon), `[data-rail-open]` (rail), `[data-rail-collapse]` („Einklappen“ in the column),
  `[data-rail-close]` („Schließen“, area beside the drawer). The widget maintains `aria-pressed` of the switches (rail
  visible = pressed) and the address of the fallback link (`lrail=0|1`). If the column does not exist on the page,
  the link stays an ordinary link.
- Drawer: in the narrow state `data-drawer` on `<html>`; focus into the column, Esc and „Schließen“ give it back to
  the trigger, a click beside it closes without a focus change, choosing a section (`a.tree-entry`) closes the
  outline. When not narrow, rail and switch collapse the column permanently (cookies `baruch_lrail`, `baruch_rrail`).
- `web/src/zoom.ts`: `[data-zoom-reset]` resets all zones and deletes `baruch_zoom`.

#### Markup and style

- `templates/workspace.html`: between the columns one rail each `a.rail[data-rail-open]` (`href="#pane-tree"` or
  `#pane-assistants`) and the area `a.rail-scrim` (`href="#pane-editor"`). The columns stay first and last child of
  the grid, because the widget `splitter` measures them that way. `id="pane-tree"`, `id="pane-editor"`,
  `id="pane-assistants"`, the splitters and all `data-zoom-zone` are unchanged.
- `templates/fragments/tree.html`, `assistants.html`: at the top the row `div.rail-bar` with „Einklappen“ (`?lrail=1`
  or `?rrail=1`) and „Schließen“ (`#pane-editor`). It stands in the fragments because `ui.render` replaces the content
  of the columns.
- `static/app.css`: basic frame (grid, rails hidden in the resting state). The rules of the old header (`.topbar`) and
  the two width media queries (header below 1000 px, columns stacked below 900 px) are gone.
- `static/laptop.css` (last in `STYLE`): everything on `data-compact`, `data-narrow`, `data-rail-*` outside the
  ribbon. Without JavaScript `:target` opens the drawer (`html:not(.js)…:target`); with JavaScript these rules do not
  apply, so that an address with `#pane-tree` does not keep the drawer open. The area beside it needs `:has()` without
  JavaScript; if the browser lacks it, „Schließen“ remains.

#### Limits

- Without JavaScript `baruch_vw` is missing: „Automatisch“ draws wide, the human sets „An“.
- On the very first call with JavaScript the server does not know the width yet either; the page comes wide and is
  switched over by the widget immediately (a short jump, afterwards the cookie is right).
- The cookie `baruch_vw` applies per browser, not per window: two windows of different widths overwrite it in turn;
  the widget sets each window right after loading.

## Round 32 – Word and quitting

New crate `baruch-docx` (pure, without database and network; `zip` 9 only with `deflate`, `quick-xml` 0.42) with the
common model (`model.rs`), errors, limits (`limits.rs`) and the fixed names of OOXML (`ooxml.rs`). Reader (`read`,
`resolve`, `to_typst`) and writer (`from_typst`, `diff`, `write`) are in separate files; both know only the model. In
the server stand `docx_import/`, `docx_export/` and `shutdown.rs`. No migration.

### Round 32 – Word import (docx_import)

**Crate `baruch-docx`, reader side** (pure, without database and network):

- `read::read(bytes, &Limits) -> ReadDocx { document, warnings }` – `read/pack.rs` opens the ZIP (size of the file,
  number of entries, declared sizes before unpacking; every entry via `take(Grenze + 1)`, the sum of what is really
  unpacked is counted too; names with `..`, a leading `/`, a backslash or a drive → `Unsafe`; OLE start → `NotDocx`
  „altes .doc oder verschlüsselt“) and resolves relationships (a target outside the archive without
  `TargetMode="External"` → `Unsafe`; external targets are never fetched, an image that is only linked yields
  `docx_bild_extern`). `read/xml.rs` builds a small tree per part with quick-xml: DTD → `Unsafe`, only the five fixed
  entities and character references are resolved, depth and number of elements limited. Elements are named as in the
  file (`w:p`) – assumption: the usual prefixes. `read/body.rs` maps the text body onto the model: role of the
  paragraph style (ID `Heading N`, name "heading N"/„Überschrift N“, `basedOn`, `w:outlineLvl` in style or paragraph,
  the Baruch styles from `ooxml.rs`), runs with character formats, `w:ins`/`w:del`/`w:moveFrom`/`w:moveTo` as
  `Inline::Tracked`, change of the paragraph mark as `mark`, footnotes/endnotes inline, comment ranges, links, images
  (DrawingML and VML; media type from the first bytes), lists via `numbering.xml`, tables, Typst islands (`w:sdt` with
  `w:tag="baruch-typst"`: the text hidden with `w:vanish` is the source, paragraphs are lines). Warnings once per code
  with the number of places.
- `resolve::resolve(document, View)` resolves the change tracking (`Original`, `Final`, `FinalBy(name)`); an inserted
  paragraph mark that is not applied merges the paragraph into the following one, an applied deleted one makes it
  take in the following one. `change_authors` names the editors in the order of their first occurrence.
  `proposal_view(document, name)` is `FinalBy` with the headings of the original and returns `HeadingChange`
  (`before` empty = inserted, `after` empty = deleted; `section` counts like `to_typst` on the original, front matter
  before the first heading is section 0).
- `to_typst::to_typst(document) -> TypstOut { sections, warnings }` writes per section Typst in the forms of the
  Markdown import (text escaped, markup as functions, `#quote(block: true, attribution: […])[…]`, lists with
  `-`/`+`, `#table(…)`, `#figure(image("…", alt: "…"), caption: […])`) and collects per section the comment anchors
  (commented wording). Text before the first heading becomes a section with an empty title.

**Server `docx_import`** (`src/docx_import/mod.rs`, `draft.rs`, `pages.rs`, template `docx_import.html`):

- `prepare(bytes) -> Prepared` (pure, in `spawn_blocking`): reads, writes the original as sections, puts the proposal
  state of each editor onto the sections of the original (only differing sections count), assigns comments to their
  sections (first on the original, otherwise on the state of an editor) and collects the warnings.
- `preview` stores the draft (`draft.rs`, kept like `pdf_import/draft.rs`: 32 hex, 20 pieces, 24 hours); `apply`
  reads the stored file anew and calls `apply_prepared`:
  1. work via `import::import_generated` (one commit „Import aus <datei>“, images as `ImportAsset`),
  2. under the write lock per editor and section `Store::add_proposal(…, "@word:<name>", "word", Notiz)` – the text
     is the current revision with the section text replaced, the heading line stays literal –, then
     `mirror::side_refs_after_proposal` (side branches `refs/revisions/<key>/<n>` with persona and model),
  3. posts via `Store::add_post` (changed headings, comments with `TextRef` on revision 1, answers directly after their
     comment).
  Never `set_current`: revision 1 stays the current one. An error in 2 or 3 is reported as a warning, the work stays.
- `cli_import` shares `prepare`/`apply_prepared`; without `--uebernehmen` nothing is written.

**Assumptions (checked without a real Word):** the tests build their Word files themselves according to ECMA-376 and
`ooxml.rs`; a file written by Microsoft Word or LibreOffice was not read. Assumed in particular: prefixes as above;
answers to comments via `commentsExtended.xml` (`w15:paraIdParent` ↔ `w14:paraId` of the comment paragraph); the
carriers of the Typst island as described in `ooxml.rs` (multi-line source as several hidden paragraphs or with
`w:br`).

### Word export (`baruch-docx` writer, `baruch-server/src/docx_export`)

The way from the work to the file has three pure stages in the crate `baruch-docx` and one reading stage in the
server:

1. **`from_typst(body, image_paths)`** (`from_typst.rs`, `from_typst/args.rs`): reads a section with
   `baruch_wysiwyg::parse` and maps the blocks onto the model. Figures (`#figure(image(…))`, `#image(…)`) become
   `Block::Figure` with literal `raw`, simple tables `Block::Table`, `#pagebreak()` `Block::PageBreak`. Everything
   else becomes `Block::Typst`/`Inline::Typst` with literal source and a visible substitute (warning
   `docx_typst_insel`); invisible things (label, index mark, comment) go along without a warning as an island without
   anything visible, cross-references with their text.
2. **`track_changes(current, proposed, change)`** (`diff.rs`): aligns the blocks via the longest common subsequence,
   pairs similar blocks of the same kind in every gap (share of common words) and compares them word by word; the
   remaining blocks are completely new or dropped completely (`mark` on the paragraph mark, content in
   `Inline::Tracked`). Promise: insertions kept and deletions removed = proposal, the other way round = current
   revision (exceptions see the head of `diff.rs`: empty table grid, page breaks).
3. **`write(document, options)`** (`write.rs`, `write/body.rs`, `write/styles.rs`, `write/image.rs`): writes the
   archive in a fixed order with fixed ZIP times (deterministic): `[Content_Types].xml`, `_rels/.rels`,
   `docProps/app.xml`, `docProps/core.xml`, `word/document.xml` including relationships, `styles.xml`,
   `settings.xml`, `numbering.xml`, `footnotes.xml` (with references in footnotes with relationships of its own),
   `comments.xml` (only with comments), images under `word/media/imageN.<endung>`. Characters forbidden in XML are
   replaced by U+FFFD.
4. **`docx_export::build`** (server): reads work details, all revisions (`Store::load_work`, points in time from
   `Store::revision_tree`), images (`Store::export_assets`, only PNG/JPEG/GIF) and – with proposals – the markers
   „bis hierhin“ (`Store::work_revision_folds`). Open is a proposal with status „vorgeschlagen“ that is not hidden by
   `baruch_core::revision_fold::is_hidden`. There are only reading accesses; packing runs in `spawn_blocking`.

Decisions of the writer beyond `ooxml.rs` (for the reader and the round trip):

- The path of an image in the work stands in `wp:docPr name`, the alternative text in `descr`.
- A figure with `raw` stands in a content control `baruch-typst` (hidden source, then image and caption) – for the
  reader therefore an island with `raw`, which yields the literal source when read back.
- Ordered lists: the first one starting at 1 uses `NUM_DECIMAL`, every further one a `w:num` of its own (from 3) on
  the same abstract numbering with `w:startOverride` – otherwise Word would keep counting across all lists of the book.
- Tracked changes stand around each run separately (`w:ins`/`w:del`), a tracked island as `w:ins`/`w:del` around the
  `w:sdt`. The paragraph mark of the quotation source and of the caption counts as tracked if its whole content is a
  change of the same kind.
- Footnote text begins as in Word with a space after the footnote mark.
- The subtitle additionally stands in `dc:subject`, the imprint in `dc:description`.

### Quitting (`shutdown.rs`)

- `crates/baruch-server/src/shutdown.rs` – quitting (Round 32).
  - `Shutdown` (in `AppState::shutdown`): trigger on a `watch` channel; once requested, it stays requested.
  - `own_machine(hosts, peer, headers)`: true only if the peer (`ConnectInfo`) is known and loopback (`to_canonical`,
    so that `::ffff:127.0.0.1` counts), `guard::via_proxy` says no and `Host` is missing or satisfies
    `guard::allowed_host`. `guard.rs` stayed untouched.
  - `serve` → `serve_until(listener, state, grace, signal)`: `axum::serve(…)` with
    `into_make_service_with_connect_info::<SocketAddr>()` and `with_graceful_shutdown` on the first of signal
    (`shutdown/signals.rs`) and `Shutdown::requested()`. After that the server accepts no more connections, idle
    connections close, running requests are answered. A second future waits for the grace period (`GRACE` = 10 s) from
    the trigger on; if a connection is still open then, it waits no longer (message on stderr). Finally
    `store.pool().close()` (at most `CLOSE_WAIT` = 5 s): SQLite writes back the log (`-wal`). `serve` then writes
    „Baruch beendet.“; `main` returns, status 0.
  - `shutdown/signals.rs` (only `tokio`, compilable on its own): Ctrl+C everywhere; Unix `SIGTERM`; Windows
    `ctrl_break`, `ctrl_close`, `ctrl_shutdown` – the last two with the grace period `WINDOWS_CLOSE_GRACE` = 3 s,
    because Windows ends the process itself shortly after the event. No `SIGHUP` (a receiver would cancel `nohup`).
  - Pages: a template environment of their own (`base.html`, `shutdown.html`, `shutdown_done.html`). The final page is
    self-contained (style inline, the only script `window.close()`), because the server delivers nothing afterwards.
    `POST /beenden` answers first and triggers `REPLY_DELAY` = 500 ms later.
  - No widget, no `.ts` file: the inline script of the final page is enough.
- Tests: `src/shutdown.rs` (trigger, table for `own_machine`), `tests/round32_beenden.rs` (pages on the real server in
  the test process, real process with button/SIGTERM/Ctrl+C, launcher with dummies in a closed `PATH`, units,
  workflows).
- CI: `pages.yml` and `release.yml` separate `pages-build` and `pages-deploy`; rule 11 in
  `scripts/ci-check-workflows.sh`, counter-checks in `scripts/ci-selftest.sh`.

Known limit: if a blocking job (`spawn_blocking`, say a Typst typesetting) is still running when quitting, the Tokio
runtime waits for it when leaving `main` – the end can then be delayed beyond the grace period. The database is
already closed at that point. Not proven by a test.

### Integration (Round 32)

- **Seam reader/writer:** both sides were created separately against `model.rs` and `ooxml.rs`. The round trip
  (`crates/baruch-docx/tests/roundtrip.rs` on the model, `tests/round32_abnahme.rs` through the server) found three
  gaps, all fixed in the reader or in `to_typst`:
  - `w:lvlOverride`/`w:startOverride` on `w:num` counts as the first number of the list (`read.rs`,
    `Numbering::overrides`) – that is how the writer writes every further numbered list, and that is also how Word
    restarts a list.
  - A numbered list that does not begin at 1 carries its number on every item in `to_typst` („3.“, „4.“ …, nested
    from 1). Typst understands "3." followed by "+", but `baruch-wysiwyg` would read it as two lists.
  - The path of an image in the work comes back from `wp:docPr name` if the name looks like a path (`read/body.rs`,
    `work_path`: relative, without `..`, extension matching the content); images in the visible part of a Typst island
    are read too (`island_images`), because the source of the island names them.
- **Signals before „Baruch läuft“:** `signals::system_signal()` sets up the receivers (Unix: `SIGINT`, `SIGTERM`) on
  the call, not only at the first wait; `main` calls it before the line „Baruch läuft“ and passes the future to
  `shutdown::serve_with(listener, state, signal)` (`serve` = `serve_with` with `system_signal()`). Before, a `SIGTERM`
  immediately after this line ended the process hard (test `sigterm_ends_the_throwaway_server_with_exit_0`).
- **`main.rs`:** after a successful `shutdown::serve_with`, `main` ends with `std::process::exit(0)`, so that a
  blocking job still running does not delay the end beyond the grace period (the database is then already closed, the
  closing line printed).
- **Guard `round33_seiten`:** the three new templates (`docx_import.html`, `docx_export.html`, `shutdown.html`) stand
  in `TEMPLATES`.
- **Open:** the writer does not write `commentsExtended.xml` (answers stand as comments of their own, the server writes
  none anyway); the private constants of the writer (`NS_APP`, `PART_FOOTNOTES_RELS`, `STYLE_HYPERLINK`,
  `STYLE_COMMENT_TEXT`, `STYLE_COMMENT_REFERENCE`) are not moved to `ooxml.rs`; quotations in the text stand in Word
  without quotation marks; SVG and WebP are not embedded.

## Round 34 – Mind map

The idea map (Round 25) has become a mind map as in XMind. Three branches (`team/mindmap_daten`,
`team/mindmap_widget`, `team/mindmap_austausch`) on the interface R34-S1, merged with `--no-ff` without text conflicts;
the seam in content was the **cloud** (see "Integration").

### Cloud and mind map: one rule with four places

The contract of the round let ideas without a parent node and without a reference hang directly on the work as „freie
Hauptthemen“; the cloud „Ohne Kapitel“ would have been dropped. After the user's addition of 2026-11-25 („Es macht
Sinn eine Verbindung zwischen der Wolke und der Mindmap zu machen“), **one rule with four places** applies:

1. `parent` set → child node of this idea (same work, no cycle, chain at most 32 deep). The reference (`key`) stays
   allowed beside it and is then only the link to the section.
2. otherwise `key` set → child of the anchor (nearest ancestor of the kind chapter, otherwise the item itself).
3. otherwise `main_topic` → **free main topic** directly on the work.
4. otherwise → **in the cloud**: not in the tree, but in the tray beside the mind map.

For this `idea` carries a fourth new column `main_topic` (0/1, default 0); the contract planned three. Without it an
idea that one drags onto the work could not be told apart from an idea without a place. Consequences:

- **Existing ideas** without a reference (today the cloud „Ohne Kapitel“) lie in the cloud after the migration; ideas
  with a reference hang on their chapter as before. Nothing becomes a main branch without somebody dragging it there.
- **New ideas without a place** (quick capture Alt+E, form, RPC `idea.create` and MCP `idea_create` without
  `parent`/`key`) land in the cloud.
- **From the cloud into the mind map:** `idea.move` with `parent` (onto an idea), with `parent: null, key` (onto a
  chapter) or with `parent: null, key: null` or `cloud: false` (onto the work, becomes a main topic).
- **From the mind map into the cloud:** `idea.move` with `cloud: true`. The idea loses parent node and reference; **its
  child nodes stay with it** – the subtree moves into the cloud as a whole. Reason: no idea silently loses its context,
  the action can be reversed with one `idea.move` back (Ctrl+Z of the widget), and "only it" would mean hanging its
  children on the grandparent node without asking. Whoever wants to detach only the one idea drags the child nodes
  elsewhere first.
- **Counters:** `cloud.count` („n in der Wolke“) counts the top-level ideas of the cloud without a filter,
  `cloud.count_with_children` including their child nodes.
- **Proximity as help:** `cloud.hints` names per idea of the cloud at most one suggestion „passt zu Ast …“ – the
  nearest idea in the mind map (stored embeddings and equal keywords as on the map, never a new embedding) and the
  main branch it hangs in. The suggestions are computed even if the filter switches off the proximity lines in the
  picture.
- **Restoring** an idea whose parent node is still deleted: it stays at its reference or lies in the cloud
  (`detached: true`), not as a main topic on the work.
- **Tree import** (`tree::import` without `parent`): the top nodes become free main topics, not ideas of the cloud –
  whoever reads in a mind map wants to see it in the mind map.

### Data (migration 0052, store)

Migration `0052_idea_tree.sql` (only `ADD COLUMN`, not in `FOREIGN_KEYS_OFF`):

```sql
ALTER TABLE idea ADD COLUMN title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200);
ALTER TABLE idea ADD COLUMN parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL;
ALTER TABLE idea ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
ALTER TABLE idea ADD COLUMN main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1));
CREATE INDEX idea_parent ON idea(parent_id, position);
```

Rules in the store (`crates/baruch-store/src/ideas.rs`):

- **Title:** at most 200 characters, one line, may be empty. An idea needs a title **or** at least one keyword.
  Displayed title (`Idea::display_title`): the title, otherwise the keywords "a, b", otherwise the start of the
  thought, otherwise „Einfall <id>“ – never empty. The title is in the full-text index (`idea_fts`, column `words`).
- **Parent node:** a non-deleted idea of the same work; no cycle; chain at most 32 ideas (`MAX_IDEA_DEPTH`). Ideas
  without a work have no parent node.
- **Order:** `position`, then `id`, among siblings (same parent node or all ideas of the work without a parent node).
  New ones go to the end; "before a sibling" (`before`) renumbers the siblings without gaps from 1, without touching
  their `updated`. Existing ideas carry 0 and order themselves by `id`; as long as nothing was ordered under a parent
  node, new ones keep the 0 too (in the exchange then without `platz` – a work without an ordered tree writes the same
  file as before Round 34 and also passes the frozen u8 schema).
- **Deleting** takes the non-deleted descendants along with **the same timestamp**; **restoring** brings back the idea
  and exactly the descendants with this timestamp (what was deleted individually earlier stays deleted). Links
  (`idea_link`) and positions (`idea_map_pos`) stay stored meanwhile.
- **Change of work** (`idea.update` with `work`): the idea detaches from its parent node and lies in the cloud in the
  new work; its child nodes stay in the old work and lose their parent node (no tree across two works).
- **Positions** (`idea_map_pos`) stay stored and in the exchange, but have no effect in the mind map (the server
  computes the layout from the tree).

Mirror `einfaelle.md`: if a work has at least one idea with a title, parent node or main topic, the section
`## Baum` stands before the entries – the ideas indented by parent node, siblings by `position`, then `id`, at the top
with the place („(am Abschnitt key 12)“, „(Hauptthema)“, „(Wolke)“). Every entry additionally names `- Titel: …`,
`- Unterknoten von: Einfall <id>` or `- Platz: Hauptthema am Werk`. Without a tree the file stays byte-identical as
before Round 34.

Exchange format: minor version 9, `<einfall>` with `titel`, `eltern`, `platz`, `hauptthema` – see
`docs/Austauschformat.md` („Baum der Mindmap“).

### Tree, cloud and import in the server (`idea_map/tree.rs`)

The signatures from S1.2 stayed literally the same (`MindMap`, `TreeNode`, `TreeKind`, `tree`, `ImportTree`,
`import`, constants). New beside them:

```rust
pub struct CloudHint { pub idea: IdeaId, pub node: String, pub node_title: String,
                       pub branch: String, pub branch_title: String, pub score: f32, pub shared_words: Vec<String> }
pub struct Cloud { pub work: WorkId, pub ideas: Vec<TreeNode>, pub count: u32, pub count_with_children: u32,
                   pub hints: Vec<CloudHint>, pub links: Vec<IdeaLink> }
pub async fn cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<Cloud, RpcError>;
pub async fn tree_and_cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<(MindMap, Cloud), RpcError>;
```

- `tree` returns the work (`w`), parts and chapters (`n:<key>`) in outline order, then the free main topics by
  `position, id`; under each node first the outline items, then its ideas by `position, id`. Ideas of the cloud do
  **not** stand in the tree.
- Filters `status`, `word`, `origin`: an idea that does not match stays as a ghost (`ghost: true`, `hit: false`) as
  long as a descendant matches. `q` hides nothing, it sets `hit` (full text, also over the title).
- `MindMap::links` and `MindMap::near.pairs` name only ideas that stand in the tree; `Cloud::links` the links with at
  least one end in the cloud. `total_ideas` counts the shown ideas in the tree (without ghosts, without the cloud);
  `descendants` counts per node the ideas that stand below it in the tree.
- `Cloud::ideas` are `TreeNode` of the kind `idea` with their child nodes in `children`; the same filter applies.
- `import` creates the tree in one transaction (store `idea_import_tree`) and writes **one** commit „Mindmap
  eingelesen: n Einfälle“ (`Baruch-Action: idea-import`) with the passed origin. Refused (-32000, nothing created, no
  commit): no node, more than `MAX_IMPORT_NODES` (2000) nodes, deeper than `MAX_TREE_DEPTH` (32, also together with
  the depth of `parent`), `parent` deleted or in another work, title over 200 or note over 4000 characters, keywords
  over the limits of ideas (at most 20, 60 characters each), a link with a path without a node. A node without title
  and keyword is called „(ohne Titel)“; a link of a node with itself is skipped. `ImportTree::title` stands in the body
  of the commit, `warnings` are not stored. Whoever calls `import` shortens titles and notes beforehand
  (`files/import.rs`); the import does not shorten, it refuses.

- Nodes of the mind map are rows of the table `idea` – no second structure. The store knows the rules of the tree
  (`idea_create_at`, `idea_move`, `idea_delete_tree`, `idea_restore_tree`, `idea_import_tree`, `idea_tree_rows`), the
  server combines it with the outline (`idea_map::tree`), the mirror writes it into `einfaelle.md`.
- Every writing action runs under the write lock and yields exactly one commit with origin (`idea-create`,
  `idea-update`, `idea-move`, `idea-delete`, `idea-restore`, `idea-import`).
- The flat map (`idea_map::build`, RPC `idea.map`) stays for backward compatibility; the tree builds on it (outline,
  filters, links, proximity, findings) and additionally reads the ideas unfiltered to determine ghosts and the cloud.

### Picture and page (`idea_map/bild/`)

`bild/mod.rs` (page `/werke/{id}/karte`, picture `/werke/{id}/karte/bild`, filters), `bild/mindmap.rs` (drawing),
`bild/tree_layout.rs` (layout, purely computational and deterministic). One layout, in Rust: screen and output
(`.svg`, `.pdf`) come from the same function `mindmap::render(map, options)`; the widget lays out nothing itself.
`bild/layout.rs` and `bild/svg.rs` of the old idea map are deleted.

- Stage: `<div class="mm-stage" data-widget="mindmap" data-work data-bild data-back data-zu data-wahl
  data-gliederung>`, in it `[data-mm-toolbar]`, `[data-mm-canvas]` with `<svg class="mm-svg" data-cloud-count
  data-zu>` and `<g class="mm-viewport">`, below that `[data-mm-status]` (live region). Counter:
  `[data-mm-cloud-count]`.
- Order in the viewport: branches, proximity, relations, cloud, nodes, last the labels of the relations
  (`g.mm-link-labels`; they lie above the nodes so that nearby nodes do not hide them – integration).
- Nodes: `<g class="mm-node mm-<work|part|chapter|idea>" data-node data-parent data-depth data-side="right|left"
  data-branch="0..7" data-editable="1|0" data-title data-own tabindex="0">`; ideas `data-idea`, with a reference
  `data-key`; with descendants `data-children="n"`; with open findings `data-open-findings="k"`. Extra classes:
  `mm-b<n>` (branch colour), `mm-main` (main branch), `mm-done`, `mm-hit`, `mm-dim`, `mm-ghost`, `mm-closed`,
  `mm-many-findings`. The work has neither `data-side` nor `data-branch`. Children: `<title>`, `rect.mm-box`,
  `text.mm-label` (in `a.mm-label-link`), `g.mm-note`, `a.mm-section`, `a.mm-findings[data-open-findings]`,
  `a.mm-toggle[data-count]`.
- Branches: `<path class="mm-edge mm-b<n>" data-from data-to>` (cubic curve, `stroke-width` 3.6 / 2.4 / 1.8 / 1.4).
- Relations: `<g class="mm-rel">` with `<path class="mm-link mm-link-<kind>" data-link data-from="i:<a>"
  data-to="i:<b>" data-end-from data-end-to>`; the label is `<text class="mm-link-label" data-link>` in
  `g.mm-link-labels`. Contradiction additionally `mm-contradiction` on group, path and text. `data-end-*` names the
  node at which the line visibly ends.
- Proximity: `<path class="mm-near" data-from data-to>` (only with `near`, only between nodes of the tree).
- Cloud: `<g class="mm-cloud" data-count>` with `rect.mm-cloud-area` and `text.mm-cloud-head`; cards are
  `g.mm-node.mm-idea.mm-cloud-item[data-cloud="1"][data-parent="w"]` (which children of the root lie in the cloud is
  said by `RenderOptions::cloud`; all others are main branches), with a suggestion `data-fits="<Hauptast>"` and
  `text.mm-cloud-hint.mm-b<n>`, with child nodes `text.mm-cloud-below` ("+ n").
- Collapsed nodes draw no descendants; `a.mm-toggle` shows "+ n", its `href` is `?<Filter>&zu=<neue Liste>`. The
  number counts all hidden nodes (of every kind), not only ideas.
- **Fragment** (`standalone: false`): no `xmlns`, no `<style>`, no colour attributes – colours via
  `--mm-b0`…`--mm-b7` in `karte.css` (light, dark by system, dark by switch). **Standalone** (`standalone: true`, for
  `.svg`/`.pdf`): `xmlns`, its own `<style>`, colours as attributes (`mindmap::BRANCH_COLORS`), white background, no
  links (instead of `<a>` a `<g>` of the same class).
- The layout is deterministic; text widths are estimated (the server does not know the font of the browser), titles
  over 48 characters are shortened in the picture (full title in `data-title` and in the tooltip).

### Widget `web/src/mindmap.ts`

Replaces `idea_map.ts` (deleted). It lays out nothing itself: after every action it fetches `data-bild` anew with
filter and `zu=`, inserts the SVG and keeps selection and viewport (`transform` on `.mm-viewport`). RPC (only these,
via `rpc.ts`): `idea.create`, `idea.update`, `idea.move`, `idea.delete`, `idea.restore`, `idea.link`, `idea.unlink`,
`idea.link_restore`. Undo is the reversing RPC of the last action (create ↔ `idea.delete`, delete ↔ `idea.restore`,
rename ↔ `idea.update` with the old stored title from `data-own`, re-hang ↔ `idea.move` back to the old place –
`parent`, `key`, `before`, for a card of the cloud `cloud: true`, for a main topic `parent: null, key: null, cloud:
false` –, link ↔ `idea.unlink`, unlink ↔ `idea.link_restore`). Buttons in the ribbon carry `data-mm-action`
(`child`, `sibling`, `rename`, `delete`, `link`, `cloud`, `undo`), links with `data-mm-zu` set the collapsed nodes
from their `href`.

The widget sends `idea.delete` with `confirm: true` (it asks the question itself). Dragged onto the work, or Tab on the
work, yields a free main topic (`cloud: false`), dragged into the cloud `cloud: true`.

### Input and output, outline view (`idea_map/files/`)

`crates/baruch-server/src/idea_map/files/` (input and output of the mind map, use without JavaScript):

- `mod.rs` – routes (`pages()`, `writes()`), the outputs, `POST /werke/{id}/karte/act`.
- `export.rs` – cloud and main branches from the view `tree::Scene` (`cloud`, `placed`), the sheet (`Sheet`) as a
  preliminary stage of `.mm`/`.xmind`, Markdown.
- `freemind.rs`, `xmind.rs` – one writer and reader each. `import.rs` – limits, cleaning, `Raw` → `ImportTree`
  (`finish`, `check`). `upload.rs` – form, preview, confirmation. `outline.rs` – outline view.
- Templates `karte_gliederung.html`, `karte_einlesen.html`; style `static/karte_gliederung.css` (only variables from
  `app.css`).

Security when reading in: upload at most 10 MiB (`DefaultBodyLimit` on the route). From the ZIP only the entry
`content.json` is read (names compared as bytes, never used as a path, nothing unpacked to disk); while unpacking the
bytes are counted and it aborts at 20 MiB; archives with more than 50 entries or with names that would lead out of the
archive (`..`, absolute path, drive, `\`) are refused entirely. XML: a DOCTYPE is refused, entities are not resolved
(only the five of XML, `nbsp`, character references); the reader works with its own stack and limits node count and
depth while reading. Between preview and confirmation the tree stands as JSON in a hidden form field (no file in the
data folder) and is checked again on confirmation (`import::check`). Both POST routes lie behind
`guard::same_origin`.

### Deviations from the contract of the round

- **`idea.delete`:** the contract says „Antwort zusätzlich `deleted: [ids]`“. `deleted` is `true` since Round 7 (RPC
  and MCP, checked among others in `round23_geheimnisse.rs`); the list is therefore called **`deleted_ids`**,
  `deleted` stays `true`.
- **Refusals of `idea.move` and `tree::import`** carry the code **-32000** (`rpc::REFUSED`, "refused"). The contract
  writes -32001; that has been `RAG_UNAVAILABLE` since Round 19c.
- **`MapIdea`** (type of the flat map) gets the fields `title`, `parent`, `position` not on the type, but in the answer
  of `idea.map` and `idea_map` (plus `main_topic`, `cloud`): the frozen file `bild/layout.rs` builds `MapIdea` by hand
  in its tests and would otherwise break.
- **`MindMap` unchanged:** the cloud is not a field of `MindMap` (hand-built values in other tests would break), but a
  second function `tree::cloud`, or both together `tree::tree_and_cloud` (see 3).
- **XSD:** the schema of minor version 9 lies as a new file `docs/schema/baruch-werk-1-u9.xsd` beside the frozen ones;
  `baruch-werk-1-u8.xsd` is unchanged (SHA-256 in the test).
- File names of the outputs are called `<slug>-mindmap.*` (before `-karte.*`); the addresses stay
  `/werke/{id}/karte.*`.
- An idea of another work in the form of the mind map yields 409 (before 422).
- The old XMind 8 format (only `content.xml`) is refused with a clear message, not read.

### Integration (Round 34)

- **The seam "cloud".** mindmap_daten delivers the cloud separately from the tree (`tree::cloud`,
  `tree::tree_and_cloud`); mindmap_widget and mindmap_austausch had assumed against S1 that "ideas directly at the
  root" were the cloud. Free main topics would then have been drawn as cards of the cloud and the real cloud not at
  all. The integrator introduced the view **`tree::Scene`** (`tree::scene`): it hangs the ideas of the cloud on the
  root after the main branches (so that `walk`/`find` know them), names their identifiers (`Scene::cloud`), the
  counter without a filter (`cloud_count`) and the suggestions (`hints`), and adds the links with one end in the
  cloud. Page, picture, outline view and all outputs read only this view now; `RenderOptions` carries `cloud`,
  `cloud_count` and `hints` for it (`RenderOptions::of(scene, standalone, closed)`), `tree_layout::layout_with` tells
  main branches and cards apart by these identifiers. RPC `idea.tree` and MCP `idea_tree` stay with `{map, cloud}`.
- **Main topic and cloud in use.** Widget: Tab on the work and "drag onto the work" send `cloud: false` (free main
  topic), "into the cloud" `cloud: true`; the reversal for Ctrl+Z knows both places. Form
  `POST /werke/{id}/karte/act`: `ziel=wolke` → `cloud: true`, `ziel=w` → main topic, `ort=werk` when creating; outline
  view with „Neues Hauptthema“ and „Als Hauptthema ans Werk“. Reading in without a target node creates main topics
  (texts of the template adjusted).
- **Found while merging:** the widget called `idea.delete` without `confirm: true` – the server would have refused
  every deletion (and the undo of a creation). Fixed, test in `round34_abnahme`.
- The suggestion „passt zu Ast …“ in the outline view names the main branch with its number (`export::node_title`).
- Deleted: `bild/layout.rs`, `bild/svg.rs`, `bild::load`, `export::tree_order`, `web/src/idea_map.ts`, the S1 text
  `tree::NOT_YET`. `PdfKind::Map` is called „Mindmap“. Old picture rules `.im-*` in `static/karte.css` still stand
  (filters, groups, contradictions and legend of the page use part of them); clearing them out is open (backlog).
- Tests: `round34_abnahme.rs` (picture and actions of the widget via RPC, outline view without JavaScript with all
  actions, MCP with origin, round trip `.mm`/`.xmind` into a second work, old data unchanged). Adjusted tests of
  others: eleven round trips to `version="9"`, `round25_abnahme`, `round25_bild`, `round18_menueband`,
  `round28_abnahme`, `round28_uebersicht`, `round9_menu`, `round33_seiten` (two new templates).

## Round 35 – Accounts and roles

Accounts, password hash, sessions and the login layer (module `auth`), roles per work and the only place of the
permission check (module `roles`), pages and menu (module `konten_ui`). Usage: `docs/Bedienung.md` (Round 35, accounts
and roles), operation: `docs/Betrieb.md` (Round 35, login and accounts), methods and routes: `docs/RPC.md`
(Round 35).

- **Without accounts** (table `account` empty) Baruch stays open as before Round 35: `Who::Open`, no cookie, no
  redirect, no additional header.
- **With accounts** pages, forms and `/rpc` require a session – also on the own machine (default). `/mcp` stays with
  tokens; the layer leaves `/mcp` untouched.
- The first account is the author and is created only on the own machine (`POST /einrichten`). Every further account is
  a Lektor. Accounts are never deleted (the handle `@lektorat:<slug>` stays taken), only locked.

**Decisions of the PM** (the user can overturn them): with accounts, login also applies on the own machine; the
exception „Am eigenen Rechner ohne Anmeldung“ (`store_meta` `auth.local_open`) is switched on by the author. Direct
access from other machines stays blocked (`guard::remote_only_mcp` unchanged) – a Lektor comes in via the reverse
proxy on the same machine. Account kind global (`author` | `lektor`), role per work. For Lektors everything is
forbidden that is not explicitly allowed. Accounts and grants are instance data (not in the exchange format, no commit
in the mirror). No new MCP tool.

### Accounts and login (`auth`)

| File | Content |
|---|---|
| `baruch-store/src/accounts.rs` | tables `account`, `account_session`, `login_attempt` (migration 0053) and the switch `store_meta` `auth.local_open`. The store never hashes and never sees a plain-text password; `Account` carries no hash. |
| `baruch-server/src/auth/mod.rs` | `Who`, `Identity`, `current()`/`scope()` (task-local), `AuthState` (remembered "there are accounts"), `accounts_exist`, `identity_of`, `check_name`, `message` |
| `auth/gate.rs` | layer `gate` (in the router directly outside `consent::inject`), `resolve`, `own_machine`, `peer` |
| `auth/password.rs` | Argon2id: `hash`, `hash_with`, `verify`, `policy`, `hash_blocking`, `verify_blocking`, `DUMMY_PHC` |
| `auth/session.rs` | token, cookie, `current_hash()` (session of the running request, task-local) |
| `auth/throttle.rs` | lock against brute force: `wait_seconds`, ranges, `reserve`/`succeeded` |
| `auth/routes.rs` | forms (behind `guard::same_origin`) and the common building blocks `create_account`, `change_password`, `check_current`, `set_active` |
| `auth/rpc.rs` | `account.*` |
| `auth/cli.rs`, `main.rs` | emergency path `baruch-server passwort <name>` |
| `guard.rs` | `forwarded(headers)`, `FORWARDED_HEADERS` |

**Course of a request** (`auth::gate`): (1) `/mcp` passes as `Who::Open`. (2) `Who` from the extensions of the request
(settable only within the process: tests), otherwise `resolve`. (3) `Who::Anonymous`: `PUBLIC_PATHS` and
`/static/js/…` pass; GET/HEAD → 303 to `/anmelden?next=<Pfad+Query, prozentkodiert>`; `POST /rpc` → 401 with JSON-RPC
error `-32002`; otherwise 401 text „Anmeldung erforderlich.“ (4) `roles::check_request`. (5) The request runs in
`auth::scope(who, …)` and `session::scope(<Token-Hash>, …)`.

**`resolve`:** no accounts → `Open`. Otherwise: a valid session of an active account → `Account`; otherwise the switch
„Am eigenen Rechner ohne Anmeldung“ on **and** `gate::own_machine` → `LocalAuthor`; otherwise `Anonymous`. A database
error never leads to more rights (it goes to stderr, the request counts as not logged in; if even the number of
accounts cannot be read, "there are accounts" applies).

**Own machine** (`gate::own_machine`, decision 0.2): `shutdown::own_machine` (peer known and loopback, no proxy name,
`Host` an IP address or `localhost`) **and** none of the headers `Forwarded`, `X-Forwarded-For`, `X-Forwarded-Host`,
`X-Forwarded-Proto`, `X-Real-IP`, `Via` (`guard::forwarded`; their mere presence counts, the content is not read).
Without `ConnectInfo`: no. If proxy names are allowed (`--allow-host`), a request without `Host` does not count as the
own machine either.

**Responses:** with accounts every response (logged in or refused) gets `Cache-Control: no-store` if it carries no
directive itself (`/static/…` keeps `no-cache`). Without accounts the layer appends nothing.

**Rule:** never evaluate `auth::current()` and `auth::session::current_hash()` in spawned tasks (`tokio::spawn`,
`spawn_blocking`) – fetch them beforehand and pass them along.

#### Password

- Argon2id, **m = 19456 KiB, t = 2, p = 1**, salt 16 bytes (getrandom), output 32 bytes, stored as PHC text
  (`$argon2id$v=19$m=19456,t=2,p=1$<Salz>$<Hash>`), crate `argon2` 0.5. Hashing and verifying run in
  `spawn_blocking`.
- `verify` reads the parameters from the PHC text: if the default grows later, old hashes stay verifiable. Only
  Argon2id counts; an unreadable text matches no password.
- Rule: 10 to 200 characters (Unicode characters, not bytes), no other rule.
- Unknown name: check against the fixed dummy hash `password::DUMMY_PHC` (same parameters, same duration), the same
  message „Name oder Passwort stimmt nicht.“ – also for a locked account and an overlong password.
- No plain text in the database (CHECK `password LIKE '$argon2id$%'`), logs, error messages or responses; no type with
  a password derives `Debug`; messages about wrong RPC parameters quote no value.

#### Session

- Token: 32 random bytes (getrandom) as Base64url (43 characters). Only the SHA-256 is stored in the database (hex, 64
  characters, CHECK on the column). Whoever knows only the database cannot log in with it.
- Cookie: `baruch_session=<token>; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000`, additionally `; Secure` if
  `guard::via_proxy` is true (proxy name in `Host`, or proxy names allowed and no `Host`).
- Valid 30 days without use, at most 90 days since the login (the server checks both in SQL); `last_seen` is written at
  most once per minute. The next login clears away expired sessions of all accounts.
- **Renewing (addition to the contract):** if the previous use lies more than an hour back, the response sets the same
  cookie again (fresh 30 days in the browser) – otherwise the session would end in the browser 30 days after the
  login although it is in use. The upper limit of 90 days stays.
- Logging out deletes the session and the cookie (`Max-Age=0`). A password change deletes all **other** sessions of the
  account (the changing one stays; a reset by the author or the command line deletes all). Locking (`active = 0`)
  deletes all. Both take effect on the next request.

#### Lock against brute force (`login_attempt`)

- Ranges: `konto:<Namensschlüssel>` (always, also for unknown names; name key = without surrounding whitespace, single
  whitespace, lower case, at most 40 characters) and `adresse:<IP>` (only with a known peer and not via the proxy).
- Up to the third failed attempt no waiting time; from the fourth 2^(n−3) seconds (2, 4, 8 …), at most 900.
- The failed attempt that triggers the lock and every attempt during the lock answer **429** with
  `Retry-After: <Sekunden>`; during the lock no password is checked and nothing is counted.
- Every attempt is reserved **before** the password is checked (`Store::login_reserve`, `throttle::reserve`): checking
  the lock and counting as a failed attempt are one transaction (`BEGIN IMMEDIATE`). Of any number of simultaneous
  attempts, therefore only as many get to the check (Argon2id) as would one after the other – with an empty counter
  four, the rest get 429. An attempt whose connection drops during the check also stays counted. (Until the code
  review after Round 35, counting happened only after the check; a salvo got past the lock.)
- Success takes back the reservation and resets the ranges of the attempt. Entries older than 24 hours are dropped on
  the next attempt.
- At most 1000 rows: if the table is full (999 ranges), a new range gets no row; instead the range `alle` waits a fixed
  2 seconds – during this time every login attempt waits. Because counting happens before the check, with a full
  table even a successful login from a new range triggers these 2 seconds (success does not reset `alle`, otherwise
  one account could lift the common brake for others).
- The previous password when changing it (`/konto/passwort`, `account.set_password`) also counts in `konto:<…>`:
  whoever holds a session cannot guess the password arbitrarily often.

#### Command line

`baruch-server passwort <name> [--data <Ordner>]` – emergency path for a forgotten password. Reads the new password as
**one line from standard input** (never from the command line: there it would stand in the process list), sets it for
the existing account and ends all sessions of the account. Exit 0 with „Passwort für „…“ gesetzt …“; unknown account
(names the existing ones), invalid length, folder without a database or no account yet → exit 1 with the reason on
stderr; without a name exit 2. Creates no account and unlocks none. Also runs beside the running server.

At start, `serve` prints a line „Anmeldung: aus (keine Konten …)“ or „Anmeldung: an (n Konten)“.

#### Store (called only by `auth`)

`account_count`, `account_list`, `account_get`, `account_create` (Lektor only if there is an author account),
`account_create_first` (check "table empty" and insert in one statement: of two simultaneous calls exactly one wins),
`account_for_login`, `account_password`, `account_set_password(id, phc, keep)`, `account_set_active` (the last active
author stays – check and change in one statement), `session_create`, `session_account`, `session_delete`,
`session_delete_all`, `login_reserve` (check the lock and count the attempt in one transaction; outcome
`LoginAttempt`), `login_wait`, `login_failure`, `login_reset`, `auth_local_open`, `set_auth_local_open`; helpers
`Store::account_name_key`, `Store::session_token_hash`. `lib.rs` exports from the module only `Account`, `NewAccount`
and `LoginAttempt`; messages and limits therefore hang as associated constants on `Store` (`ACCOUNT_TAKEN`,
`ACCOUNT_NOT_FIRST`, `ACCOUNT_NEEDS_AUTHOR`, `ACCOUNT_LAST_AUTHOR`, `LOGIN_ATTEMPT_ROWS`, `LOGIN_FULL_WAIT`).

### Roles (`roles`)

- **Account kind global, role per work.** `roles::role_in(state, who, work)`: full rights (`Who::Open`,
  `Who::LocalAuthor`, author account) → `Role::Author` in every work; Lektor account → `Role::Lektor` only with a row
  in `work_member` (migration 0054); otherwise `None` (work invisible).
- **One place of the permission check:** the module `roles` (`crates/baruch-server/src/roles/mod.rs`).
  - `roles::check_request` – every route, called from `auth::gate`. Lektor: only hits in `LEKTOR_ROUTES` (method +
    pattern), otherwise 403 „Abgelehnt: …“ with the header `x-baruch-abgelehnt: rolle`. HEAD counts as GET; verbs
    other than GET, HEAD, POST are blocked for Lektors.
  - `roles::check_rpc` – every RPC method, called from `rpc::call` before every dispatch (also for unknown names), so
    for `/rpc` (single and batch), `/act` and `/lektorat/act`. Lektor: only methods from `LEKTOR_RPC`, otherwise
    `-32003` „Das darf nur der Autor: <methode>“.
  - Everything not in the tables is blocked for Lektors – also routes and methods of future rounds. The forms of the
    modules (`/beitraege/act`, `/werke/befunde/act`, `/faden/act` …) call their module directly and therefore are not
    in `LEKTOR_ROUTES`.
- **Work binding.** The work stands in the path (`{id}`), in the address (`?work=`) or in the RPC parameter `work`.
  - A work not granted is refused with the same message as an unknown one („unbekanntes Werk N“; 403 or `-32003`) – a
    Lektor does not learn which works exist.
  - If the work is missing, the default work of the instance **never** applies: RPC inserts the smallest granted work;
    GET pages with `WorkFrom::Query` redirect with 303 to the same address with `work=<Werk>` – the one opened last
    (cookie `baruch_work`) if it is granted, otherwise the smallest granted one (`roles::check_request_preferring`,
    that is how `auth::gate` calls it); without a granted work 303 to `/werke` (RPC: `-32003` „Noch kein Werk
    freigegeben.“). The cookie never grants a right. For pages without a work of their own (`/werke`, `/konto`,
    `/ueber`) see "Integration".
  - A `work` in the address or in the parameters is checked for **every** allowed route and method, even if it needs
    no work itself (work choice in the ribbon). The address is read as the pages read it (percent-decoded; an encoded
    name like `wor%6B` counts too, several `work` must all be granted).
  - The store always looks up ids (post, finding, thread, image, fixed key of a section) in the named work; an id of a
    foreign work hits nothing under the granted one (test
    `ids_of_a_foreign_work_yield_nothing_under_the_granted_work`).
- **Nothing is remembered.** Every check reads the grants anew (`Store::member_works`, returns something only for
  active Lektor accounts). Withdrawing the grant and locking the account take effect on the next request.
- **Visibility.** `roles::visible_works` returns to the Lektor only granted works (`work.list`, `/werke`, work choice);
  `rpc::library_view` returns an empty literature list to Lektors (no titles, no search, no hits, no image box) – for
  `ui.render` and the page `/`.
- **Origin.** A Lektor writes under `@lektorat:<slug>` (`roles::author_name`), all others still as „Autor“.
  - Proposal: `Store::add_proposal` with persona = handle, model empty; side branch `refs/revisions/<key>/<n>`
    (`mirror::side_refs_after_proposal`) as with MCP proposals, trailer `Persona: @lektorat:<slug>`.
  - Post: `post.author` = handle; as with the author, no commit on creation. Editing/deleting own posts: one commit
    (`mirror::describe_post`), author of the commit is the handle, committer „Autor“.
  - Finding: `finding.persona` = handle; one commit „Befund <id>: angelegt von @lektorat:<slug>“
    (`mirror::describe_finding_added`, new), author „<Anzeigename> (<Handle>)“
    `<lektorat-slug>@lektorat.baruch.invalid`, trailer `Baruch-Finding`, `Baruch-Action: finding-add`, `Baruch-Node`
    (if a section), `Baruch-Origin: lektor`, `Baruch-Lektor`.
- **Grants are instance data:** table `work_member` (0054), not in the exchange format, no commit in the mirror; the
  point in time stands in `granted`. Deleting a work or an account removes its grants (ON DELETE CASCADE).
- **Store (`baruch-store/src/members.rs`):** `member_list(work)`, `member_grant(work, account, role)` (only Lektor
  accounts, only existing works, idempotent – the time of the first grant stays), `member_revoke(work, account)`,
  `member_works(account)` (ascending; empty for locked, unknown and author accounts).

#### Allowed routes for Lektors (`roles::LEKTOR_ROUTES`)

| Method | Pattern | Work from | Reason |
|---|---|---|---|
| GET | `/` | address | work page: read a section, proposal, chat |
| GET | `/werke` | – | list of works, filtered via `roles::visible_works` |
| GET | `/beitraege` | address | whole chat history of a section (without JavaScript) |
| GET | `/beitrag` | address | one post: copy, quote, edit an own one |
| GET | `/faeden` | address | read threads |
| GET | `/faden` | address | read one thread (not write: `/faden/act` blocked) |
| GET | `/suche` | address | search in the work (not replace) |
| GET | `/gliederung` | address | read the outline (not change: `/gliederung/act` blocked) |
| GET | `/werke/{id}/befunde` | path | findings page, there also „Befund schreiben“ |
| GET | `/werke/{id}/fassungen/verlauf` | path | read the history of amendments |
| GET | `/werke/{id}/glossar` | path | read the glossary |
| GET | `/werke/{id}/register` | path | read the index |
| GET | `/werke/{id}/bilder/{name}/datei` | path | images in the text of the work page |
| GET | `/werke/{id}/bilder/{name}/vorschau` | path | preview images in the text |
| GET | `/ueber`, `/ueber/lizenz` | – | about Baruch, licence (AGPL: pointer to the source code) |
| GET | `/konto` | – | own account (change password, log out) |
| GET | `/anmelden` | – | login page (after logging out) |
| GET | `/einstellungen/darstellung` | address | display in the own browser (cookie); all other settings pages blocked. **Deviation:** with the work from the address instead of "without work" – without `work` the ribbon of the page showed the default work of the instance (title of a foreign work) |
| GET | `/static/app.css`, `/static/favicon.svg`, `/static/js/{name}` | – | style, icon, widgets |
| POST | `/rpc` | – | JSON-RPC; every method is checked by `roles::check_rpc` |
| POST | `/lektorat/act` | – | form path without JavaScript; calls `rpc::call`, so the same check |
| POST | `/abmelden` | – | log out |
| POST | `/konto/passwort` | – | change own password |
| POST | `/einstellungen/darstellung/act` | – | save the display (only cookies in the own browser) |

Not allowed and therefore 403: every upload, `/act`, every form of a module, every export (PDF, EPUB, Word, exchange,
mind map file), import, all other settings, quitting, literature, ideas, image pages except file and preview. `/mcp`
stays with tokens and does not pass through the role check.

#### Allowed RPC methods for Lektors (`roles::LEKTOR_RPC`)

"Work" = `work` from the parameters; if it is missing, the smallest granted one is inserted.

| Method | Work | Kind | Reason |
|---|---|---|---|
| `work.get`, `node.get` | work | reading | read work and section |
| `revision.tree`, `revision.amends`, `revision.fold_get` | work | reading | revisions, their history, hiding |
| `ui.render` | work | reading | work page without reloading (literature empty for Lektors) |
| `menu.badges` | work | reading | counters in the ribbon |
| `post.page`, `post.since`, `post.versions` | work | reading | chat of the section |
| `finding.list`, `finding.get`, `finding.matrix` | work | reading | findings |
| `thread.list`, `thread.get`, `thread.since`, `thread.post_versions` | work | reading | read threads |
| `glossary.list`, `glossary.lookup`, `index.entries`, `index.overview` | work | reading | glossary and index |
| `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`, `wysiwyg.style` | work | reading | editor for the proposal |
| `ribbon.formats` | work | reading | formats of the template in the ribbon |
| `work.sheet` | work | reading | **added:** typeset view settings of the work for the editor (read only; `work.sheet_set` stays blocked) |
| `search.find` | work | reading | **added:** search in the work (page `/suche` and search bar); `search.preview/replace/undo` stay blocked |
| `work.list` | – | reading | filtered to granted works |
| `account.me`, `account.set_password`, `account.logout_all` | – | own account | account of the Lektor (building block "konten") |
| `revision.propose` | work | writing | proposal of a revision |
| `post.add`, `post.edit`, `post.delete` | work | writing | post; edit and delete only own ones |
| `finding.add` | work | writing | finding or „Anmerkung zum Buch“ |

Deviations from the start of the contract (§2.2), reported (plus the route `/einstellungen/darstellung` in §2):

1. `wysiwyg.*` and `ribbon.formats` stand with a work binding instead of "without work". Reason: without `work`,
   `rpc::dispatch` gives them the default work of the instance (`wysiwyg.parse` reads its image texts,
   `wysiwyg.style` its typeset view settings, `ribbon.formats` the formats of its template) – for a Lektor that would
   be a foreign work. Consequence: a Lektor without a granted work gets `-32003` for them.
2. Only the two purely reading methods `work.sheet` and `search.find` were added (reason in the table). None of them
   touches a model, a connection or the file system. Not added (open for the integrator or "oberflaeche_konten", if
   the work page calls them for Lektors): `bible.marks` and `scripture.check` (read SWORD modules from the file
   system), `timing.count`, `xref.targets`, `outline.check`, `index.registers`, `work.typeset_get`.

### User interface (`konten_ui`, `view.rs`, templates)

| File | Content |
|---|---|
| `src/konten_ui/mod.rs` | `login_page`/`login_html`, `setup_page`/`setup_html`, `safe_next`, `MESSAGES` with `ok_message`/`error_message`, `apply`, markers `KONTO_MARK`, `LEKTOR_OPEN/CLOSE`, `KONTEN_OPEN/CLOSE`, a minijinja environment of its own (the 38th) |
| `src/konten_ui/pages.rs` | routes `GET /anmelden`, `/einrichten`, `/konto`, `/werke/{id}/mitwirkende`; content of the settings page „Konten“ (`render_settings`); pure rendering functions `accounts_html`, `members_html`, `account_html` over view data; `direct_from_own_machine` (the same rule as `auth::gate::own_machine`, via `guard::forwarded`) |
| `src/view.rs` | `values` reads `auth::current()`; for Lektors templates of their own for the three areas (`pane_templates`), `post.mine`, posts without finding cards, origin `@lektorat:…` without an empty model |
| `src/consent.rs` | notice bar and dialog only with full rights |
| `src/settings/mod.rs` | eighth page `konten` in `PAGES`; navigation of the frame for Lektors only „Darstellung“ |
| `templates/anmelden.html`, `einrichten.html` | standalone pages without ribbon and without script |
| `templates/konto.html`, `mitwirkende.html` | extend `base.html` (tab „Start“ or „Buch“) |
| `templates/settings/konten.html` | content of the settings page |
| `templates/fragments/lektor_tree.html`, `lektor_editor.html`, `lektor_assistants.html` | work page of a Lektor |
| `static/konten.css` | rules of the roles and styling of the new pages (only variables from `app.css`) |
| `web/src/settings.ts`, `menu.ts` | Lektor: „Einstellungen“ stays a link (no `settings.render`), arrow keys skip hidden entries |

#### `konten_ui::apply(page, who, accounts_exist)`

Runs in `consent::inject` after `band::apply` over every page with the marker `<!--BARUCH-KONTO-->` (it stands in
`base.html` in the menu „Baruch ▾“ after „Baruch beenden…“). Pages without the marker stay byte for byte the same.

| State | Menu entry | `<body>` | Markers |
|---|---|---|---|
| without accounts | „Konten einrichten…“ (`/einrichten`) | unchanged | ` nur-autor"` → `"`, ` data-nur-autor` removed, pieces `NUR-LEKTOR` and `NUR-KONTEN` cut out |
| author account | „Angemeldet: <Anzeigename> (Autor)“, „Mein Konto“, form „Abmelden“ (`POST /abmelden`) | `data-rolle="autor"` | `NUR-LEKTOR` cut out, `NUR-KONTEN` stays (without brackets) |
| Lektor account | as above with „(Lektor)“ | `data-rolle="lektor"` | `NUR-LEKTOR` and `NUR-KONTEN` stay (without brackets) |
| `LocalAuthor` | „Autor (eigener Rechner, ohne Anmeldung)“, „Anmelden“ | `data-rolle="autor"` | like the author account |
| `Anonymous` | „Anmelden“ | unchanged | like the author account |

So a page without accounts is, apart from the one menu entry, the same as before Round 35. Checked with a byte
comparison of 23 pages and `ui.render` before and after the change (digits zeroed): differences only the menu entry,
the eighth entry „Konten“ in the navigation of the settings and the display of an origin `@lektorat:…` (before
„Persona @lektorat:erika ()“, now „@lektorat:erika“ – without accounts it does not occur).

Rule for templates: `nur-autor` is always the **last** class of an element (`class="btn nur-autor"`), otherwise
`apply` does not find it without accounts. `apply` takes the markers out of **tags** only, never out of text between
the tags – book text in the text field or typeset view stays untouched, even if it happens to contain the same
characters. Groups of the ribbon carry the attribute `data-nur-autor` instead of the class (their tag must stay
literally `<details class="band-group"` … ` data-band-group open>`, `band::apply`).

#### What a Lektor sees

- **Work page (server side):** `view::page`/`view::panes` take `fragments/lektor_*.html` for Lektors. In them: the
  form `form#editor-form` to `POST /lektorat/act` with `method=revision.propose`, `work`, `key`, `body`, `note` (with
  JavaScript the same method via `/rpc`, `data-rpc` in `workspace.ts`), button „Vorschlagen“; on the right „Befund
  schreiben“ (`finding.add`: `work`, `key`, `kind`, `body`, `quote`), post (`post.add`: `work`, `key`, `body`),
  changing/removing own posts (`post.edit`: `id`, `body`; `post.delete`: `id`, `confirm`). Missing from the HTML:
  save, save as a new revision, adopt, discard, switching („aktuell machen“, „Diese Fassung verwenden“), delete, hide,
  personas and their forms, solutions, moving/converting posts, inserting an image, marking, checking quotations, the
  dialogs literature/create work/idea, tab and panel „Einfügen“, the widget "outline" (rearranging the outline) and
  the widget "chat" (see assumptions). Finding blocks in posts of the personas are read by the Lektor as text of the
  post, without the card with the status buttons.
- **Other pages (CSS, without JavaScript):** for `body[data-rolle="lektor"]`, `konten.css` hides `.nur-autor`,
  `[data-nur-autor]` and **every POST form** whose `action` is not `/lektorat/act`, `/abmelden`, `/konto/passwort` or
  `/einstellungen/darstellung/act` (the same rule as in the server: everything forbidden that is not allowed – new
  forms of future rounds are hidden from Lektors by themselves). `.nur-lektor` is visible only there. Marked in
  `base.html`: new work/import (3 entries), read exchange, quit, the groups ideas, tools, sources, typesetting, output,
  cover and the buttons check quotations, cross-references, edit work, requests, outline scheme, images, contributors;
  in `works.html` the group „Werke“ and change, export, PDF, delete; on the findings page the forms for status,
  decision and solutions (macro `card(…, na)`).
- **Findings page:** form „Befund schreiben: Anmerkung zum Buch“ (`finding.add` without `key`, `back=befunde`) as
  `nur-lektor`, only for Lektors in the HTML.
- **Settings:** in the navigation only „Darstellung“ stands for Lektors.
- **Consent:** notice bar and dialog (`consent`) only with full rights.

#### Pages

| Route | Behaviour |
|---|---|
| `GET /anmelden[?next=…][&ok=abgemeldet]` | without accounts 303 `/`; otherwise form `POST /anmelden` (`name`, `password`, `next`), link „Weiter“ to `safe_next(next)`; with a session „Angemeldet als …“ |
| `GET /einrichten` | with accounts 303 `/einstellungen/konten`; not from the own machine 403 with an explanation (without a form); otherwise form `POST /einrichten` (`name`, `display`, `password`, `password2`) |
| `GET /konto[?ok=…][&fehler=…]` | name, role, origin, works (`roles::visible_works`), `POST /konto/passwort` (`current`, `password`, `password2`), `POST /abmelden`; without accounts an explanation; `LocalAuthor` a hint |
| `GET /werke/{id}/mitwirkende[?ok=…][&fehler=…]` | only full rights (otherwise 403), unknown work 404; data from `rpc::call("member.list", {work})`; forms `POST /werke/{id}/mitwirkende/act` (`action=grant|revoke`, `account`) |
| `GET /einstellungen/konten[?ok=…][&fehler=…]` | list from `Store::account_list`, switch from `account.me` (`local_open`); forms `POST /einstellungen/konten/act` with `action=create` (`name`, `display`, `password`, `password2`), `deactivate`/`activate` (`id`), `reset_password` (`id`, `password`, `password2`), `local_open` (`open=0|1`); without accounts an explanation and „Einrichten“ |

`?ok=`/`?fehler=` show only the sentence from `MESSAGES` (a success code does not count as an error and vice versa,
unknown codes show nothing). All account pages answer with `Cache-Control: no-store`.

### Assumptions and deviations from the contract

Accounts:

1. **"There are accounts" is remembered only as a yes** (contract §1.4 says: cache the result and invalidate it after
   creating). Accounts are never deleted, a yes stays true; the no is looked up per request (one `SELECT count(*)` on
   the empty table). Reason: a remembered no would let requests through without login if an account comes into being
   past the store (tests of the colleagues create accounts by SQL; a second process).
2. **Renewing the cookie** after a pause of more than an hour (see session) – not named in the contract.
3. **The fourth failed attempt itself already answers 429** with `Retry-After: 2` (acceptance: "from the fourth
   failed attempt 429"), not only the fifth attempt.
4. **`alle`** blocks every login attempt (also known ranges) for at most 2 seconds while it waits.
5. **The previous password when changing it** counts in the lock of the account; during the lock the code is
   `passwort_falsch` (the table of messages has no code of its own for "locked").
6. **`POST /einrichten` with an existing account** shows the page with 403 and the sentence „Es gibt schon ein
   Konto.“ (no code of table §3.4); for users not logged in, the request already ends in the layer with 401.
7. **`/einstellungen/konten/act` `create` without any account** redirects to `/einrichten` (303): a Lektor without an
   author would be a state in which nobody has full rights; the store refuses it as well.
8. **Locking the own account** is allowed for an author as long as another active author remains (in this round there
   is only one author via the user interface – in practice `letzter_autor` always applies).
9. **`resolve` takes `&http::request::Parts`** instead of `&Request` (already fixed and justified so in S1).

Roles:

- **No counter for open proposals:** MCP limits open proposals per client; for Lektors (humans) there is no such
  limit, only the sizes (1 MiB text, 2000 characters note).
- **Lektors and the thread chat:** read yes, write no (no model job, contract §0.7).
- **`/lektorat/act` for the author:** takes only methods from `LEKTOR_RPC`; the server refuses
  `revision.propose`/`finding.add` for him with a hint. The author uses `/act` and the forms of the modules as before.

User interface:

2. **`account.me`** returns `local_open` (contract §1.6); if the call fails, the page shows "switched off".
3. **`member.list`** in the form of the contract (§2.3); an unreadable answer shows the page as an error.
4. **`/einstellungen/konten/act`** gets exactly the fields of the contract – no `work`. After the action the work is
   missing from the address (it then comes from the cookie `baruch_work`).
5. **`/lektorat/act`** redirects after `revision.propose`, `post.*` and `finding.add` back to the section
   (`view::node_url`) or, with `back=befunde`, to the findings page. The Lektor forms of the work page send no
   `back`.
6. **`revision.propose`** via `/rpc` answers with `{key, rev, …}`; `workspace.ts` then shows this revision.
7. **Live chat:** for Lektors the widget "chat" is missing (new posts appear after reloading): `post.since` returns
   posts with the tools of the author (`post_entry`), which a Lektor cannot use.
8. **Pages `/beitrag` and `/beitraege`:** a Lektor reads there; he changes and removes his own posts only on the work
   page. The forms of the author are hidden there by the CSS rule for POST forms.
9. **Display name with the origin:** revisions, posts and findings show `@lektorat:<slug>` (like `@word:<name>`). The
   display name for it is on „Mein Konto“, „Mitwirkende“ and in the settings – not on the work page (the view data
   `LibraryView` know no accounts; owner `rpc.rs`).
10. **Links to blocked pages** (e.g. cover image in the list of works, „Zitate prüfen“ in older pages) can stay for
    Lektors where they do not lie in `base.html`; they end with 403 „Abgelehnt: …“.

### Adjusted older tests

| File | Why |
|---|---|
| `tests/round9_settings.rs`, `tests/round9_integration.rs`, unit test in `settings/mod.rs` | eight instead of seven settings pages (contract §3.1) |
| `tests/round24_pakete.rs` | after „Baruch beenden…“ follows the entry for the account (contract §3.2) |
| `tests/round32_abnahme.rs` | the entries „Word (.docx) importieren…“ and „Baruch beenden…“ carry the class `nur-autor` in `base.html`; after the marker `R32-PLATZ beenden` follows the marker for the entry for the account |
| `tests/round33_laptop.rs` | `konten.css` stands in `STYLE` after `laptop.css` (S1); the test now requires that `laptop.css` stands after the basic frame and only `konten.css` follows it (which does not touch the states of the laptop mode – checked in `round35_oberflaeche`) |
| `tests/round33_seiten.rs` | `konto.html` and `mitwirkende.html` in `TEMPLATES` |

### Integration (Round 35)

- **Merged** `team/konten`, `team/rollen`, `team/oberflaeche_konten` without a conflict. The three branches had each
  built against the stubs of the others; the seam is checked by `tests/round35_abnahme.rs` with real sessions (no
  `common::with_who`): setting up only from the own machine (not via the proxy name, not without a peer, not with
  `X-Forwarded-For`), afterwards pages 303 to `/anmelden?next=…`, `/rpc` 401 with −32002, forms 401, `/mcp`
  unchanged; four wrong passwords → 429 with `Retry-After`, also for the right one; logging out ends exactly one
  session; no password and no token in the data folder (all files searched). The Lektor (name with umlaut →
  `@lektorat:baerbel`) sees only the granted work, writes proposal, post and finding via `/lektorat/act` (without
  JavaScript) with origin in the mirror, may not adopt (403 via `/act`, `/lektorat/act`, −32003 via `/rpc`); the
  author adopts via `/act`; after the grant is withdrawn 403. Plus, with a real session, every route from
  `common::registered_routes()` and every method name from `common::rpc_method_candidates()` outside the two tables:
  403 or −32003, database (row counts) and mirror (HEAD) unchanged afterwards.
- **Without accounts:** the start page is, apart from the entry „Konten einrichten…“ (directly after „Baruch
  beenden…“), the one from before: the test cuts out the entry and afterwards finds nothing from Round 35. A byte for
  byte comparison against a program at the state before S1 stands in the report of oberflaeche_konten (23 pages and
  `ui.render`); the integrator did not repeat it.
- **Work opened last (contract §2.1, open with rollen):** `roles::check_request_preferring(…, preferred)` takes the
  work from the cookie `baruch_work` if it is granted; `check_request` (S1 signature) stays and calls it without a
  preference.
- **Pages without a work of their own (visual check):** for Lektors, `/werke` linked „Zurück zum Werk“ to the default
  work of the instance (403), and with an old cookie `baruch_work` the whole ribbon pointed to a foreign work. Two
  places, both central: `AppState::default_work` returns to a Lektor the smallest **granted** work (without a grant
  `UnknownWork(0)`), and `auth::gate` points the cookie `baruch_work` **of the request** to a granted work for Lektors
  (`pin_last_work`; the cookie in the browser stays, without a grant it is missing). Titles of a foreign work did not
  stand in the page before either.
- **Head of a proposal:** „@lektorat:baerbel (@lektorat:baerbel) · Fassung 2“ named the handle twice (an account has
  no persona entry); now „@lektorat:baerbel · Fassung 2“ (`view.rs`). The display name still does not stand there
  (backlog).
- **Findings page:** the form „Anmerkung zum Buch“ stood at the right edge because of `.fd-form` (befunde.css);
  `konten.css` aligns it as a column.
- `konten_ui::pages::direct_from_own_machine` uses `guard::forwarded` (one list of the six headers instead of two);
  the S1 text `accounts::NOT_YET` and the exception list `pending` in `round35_schreibwege` are removed.
- **Widgets of the work page of a Lektor** (concern of rollen: calls of blocked methods): the page carries only
  `workspace`, `editor`, `wysiwyg`, `tree`, `accordion`, `splitter`, `band`, `band-dock`, `menubar`, `settings`,
  `theme`, `work-select`. On loading no RPC call happens; „Vorschlagen“ with JavaScript calls `wysiwyg.serialize`,
  `revision.propose`, `ui.render`, `menu.badges` – all allowed (visual check with a probe, transcript at the
  intermediary). `timing`, `scripture-check`, `context-menu` do not stand in the page for Lektors; `bible.marks`,
  `timing.count`, `index.registers` etc. therefore rightly stay blocked.
- **Confirmed (PM question of rollen):** `wysiwyg.*`, `ribbon.formats` and `GET /einstellungen/darstellung` with work
  binding; `work.sheet` and `search.find` as purely reading additions. Consequence: a Lektor without a granted work
  does not reach the display page (redirect to `/werke`).
- **Building blocks merged in:** the files `docs/r35/*.md` of the developers went into this section, into
  `docs/RPC.md` and `docs/MCP.md` and are deleted.
- **Photos of the visual check:** `.claude/sicht/r35/` (setup, login, accounts, contributors, menu and work page as
  author and as Lektor, findings page, my account, refusal; light and dark, with and without JavaScript, 1024 px).

### Tests

- `crates/baruch-store/tests/migration_0053.rs`, `migration_0054.rs`: migrations on a rebuilt database at state 0052
  (all rows byte-identical), store rules (first account only once, last author, sessions, lock, grants).
- `crates/baruch-server/tests/round35_konten.rs` (login, session, lock, administration, command `passwort`),
  `round35_rollen.rs` (work binding, origin, own posts, findings, grants), `round35_schreibwege.rs` (every route and
  method outside the tables), `round35_oberflaeche.rs` (pages, menu, work page, stylesheet), `round35_abnahme.rs` (the
  real path with sessions, see "Integration").
- Test helpers in `tests/common/mod.rs`: `registered_routes()`, `rpc_method_candidates()`, `with_who(request, who)`.

### Manual check for the user (real browser, proxy with TLS)

Make a backup first or check with a data folder of your own (`--data`).

**Without accounts**

1. Open Baruch: everything as before? In the menu „Baruch ▾“ there is a new entry „Konten einrichten…“.
2. Settings → accounts: explanation and button „Einrichten“.

**Setting up (on the machine itself, `http://127.0.0.1:8765`)**

3. „Konten einrichten…“: the form appears. Two different passwords → message, the name stays.
4. Create the account → settings → accounts with „Das Konto ist angelegt.“; the menu shows „Angemeldet: … (Autor)“.
5. Via the proxy (`https://NAME/einrichten`) or in a private window: redirect to the accounts or the login.
6. Password manager of the browser: does it offer saving when creating and filling in when logging in?

**Author**

7. Log out → login page with „Abgemeldet.“; a wrong password four times → the waiting time is named.
8. Create a Lektor (settings → accounts), grant a work (tab „Buch“ → „Mitwirkende“), withdraw it again.
9. Lock and unlock; reset the password (expandable form in the row).
10. Switch „Am eigenen Rechner ohne Anmeldung“ on and off: the menu shows „Autor (eigener Rechner, ohne Anmeldung)“.
    **Via the proxy the login page must still come.**
11. Dark theme and laptop mode: login page, „Mein Konto“, accounts, contributors readable, nothing runs out of the
    window.

**Lektor (second browser or private window, via the proxy)**

12. Log in: only the granted work in the work choice and under „Alle Werke“; without a grant „Noch kein Werk
    freigegeben.“.
13. Work page: no „Speichern“, no tab „Einfügen“, no assistants on the right; in the menu „Baruch ▾“ only „Alle
    Werke“, „Einstellungen“, „Über Baruch“, „Mein Konto“, „Abmelden“.
14. Change text → „Vorschlagen“ (with a note): the proposal appears as a new revision „Vorschlag · @lektorat:…“. The
    same with JavaScript switched off.
15. Send a post; change and remove an own post under „Verwalten“; at the post of the author „Verwalten“ is missing.
16. „Befund schreiben“ on the right; on the page findings „Anmerkung zum Buch schreiben“; no status buttons there.
17. Address by hand: `/werke/<id>/pdf`, `/einstellungen/werk`, `/literatur` → „Abgelehnt: …“ (403).
18. „Einstellungen“ leads to „Darstellung“ (no dialog); switching the theme works.
19. As the author: adopt or discard the proposal of the Lektor; the origin stands in the Git mirror.
20. Click a link to a Baruch page from an e-mail while logged in: login page with „Schon angemeldet? Weiter“ → the
    click leads to the target without a new login.

**Emergency path**

21. `echo 'neues-langes-passwort' | baruch-server passwort <Name> --data <Ordner>`; then log in.
22. `curl -i -X POST <Adresse>/rpc -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"work.list"}'`
    without a cookie → 401 with code −32002; `/mcp` behaves as before Round 35 (token or own machine).
23. `sqlite3 'file:<Ordner>/baruch.sqlite?mode=ro' 'select password from account; select token_hash from
    account_session'` → only `$argon2id$v=19$m=19456,t=2,p=1$…` and 64 hex characters.
24. As a Lektor with two granted works: work in the second work, then „Baruch ▾ → Alle Werke“ and „Zurück zum Werk“
    → the work opened last, never a foreign one.

## Round 36 – Leftovers

Small, clearly delimited items from Rounds 33 to 35, from the counter-check of 0.9.0 and from the list "open after
R24" (task `reste`; usage in `docs/Bedienung.md`, Round 36, leftovers). No new route, no new RPC method, no new MCP
tool, no migration (the reserved number 0055 stays unused), no new dependency. New SQL stands only in
`crates/baruch-store/src/reste36.rs`, as string literals with bound values.

### Accounts and roles

- **Emergency path `passwort`** (`auth/cli.rs`): after `account_set_password`, `reset_password` calls
  `Store::login_reset(&throttle::account_scope(name))`. The range hangs on the name key of the account; the range of
  the address and the ranges of other names stay. A refused call (password too short, unknown account) changes
  nothing.
- **Work binding loosened** (`roles/mod.rs`): `GET /einstellungen/darstellung` in `LEKTOR_ROUTES` as well as
  `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`, `wysiwyg.style` and `ribbon.formats` in `LEKTOR_RPC` stand on
  `WorkFrom::None`. That holds because all paths to a work without an explicit `work` deliver only granted works to a
  Lektor: `AppState::default_work` gives him the smallest granted work and without a grant none;
  `settings::resolve_work` accepts the cookie `baruch_work` for a Lektor only if `roles::role_in` knows the work as
  granted (before, the page without `work` would have taken the work opened last from the cookie). A `work` named in
  the address or in the parameter is checked by `roles` as before. `roles` stays the only place of the permission
  check; the change in `resolve_work` only chooses what is shown. Evidence: `round36_lektor` (default work never
  foreign, page reachable without a grant and without a foreign title, methods without `work`).
- **Display name with the origin** (`konten_ui/names.rs`): a post-processing step over the finished HTML, hooked in
  at two places – `consent::inject` (whole pages) and `ui.render` (the three pieces for reloading). It reads
  `Store::account_list`, builds per account `@lektorat:<slug>` → display name and replaces only the **text** of the
  origin elements by "display name (@lektorat:slug)"; attributes (`data-post-author` …) and everything stored stay the
  handle. Without accounts and without hits it returns the page unchanged (byte for byte, test). Post-processing was
  chosen because `LibraryView` and the templates know no accounts and the store is not rebuilt in this round.
- **Labels and links for Lektors:** `base.html` carries the search button twice (`nur-autor` „Im Werk suchen und
  ersetzen“, `nur-lektor` „Im Werk suchen“); as with the other pairs, `konten_ui::apply` leaves only the matching one.
  `works.html` draws the cover image for Lektors without a link (`lektor` in the context of `works::list_page`). Both
  are convenience; blocking still happens only in `roles`.
- **Theme on `/anmelden` and `/einrichten`** (`konten_ui::themed`, hooked into `consent::inject`): the two standalone
  pages (without ribbon, recognised by the head of the template and by `body.konto-seite`) get `data-theme` from the
  cookie `baruch_theme` on `<html>`, like `base.html`. Without a cookie or with an unknown value the page stays
  unchanged (colour scheme of the system). No JavaScript.

### Mind map

- **Round trip with cloud:** `files/import.rs` recognises its own branch by the identifier `export::CLOUD_ID` (never
  by the title), dissolves it and gives its children `ImportNode::cloud = true` (`tree.rs`, `#[serde(default)]`, so
  that an older preview in the form stays valid). When creating without a target node, the store creates the top
  nodes as main topics as before (`idea_import_tree`, a store file of sqlx09 in this round); afterwards `tree.rs`
  moves the marked ones into the cloud with `Store::idea_move` – under the same write lock and before the one mirror
  commit. With a target node the marker does not count, deeper in the tree neither.
- **Labels of the links** (`bild/mindmap.rs`, `free_label_spot`): every label is a rectangle (estimated text width
  plus margin). A place is searched that overlaps no node rectangle and no label already placed (3 px distance) and
  lies within the picture: first at seven places of the curve with growing distance above and below, then in rings
  around the middle of the line. If nothing is found, it stays at the middle and carries `data-crowded`. Plus an
  opaque background (`rect.mm-link-label-bg`, white in the file output), so that crossing lines do not strike it
  through. Limit: the text width is estimated, the server does not know the font of the browser.
- **Light stripe in dark mode** (`static/karte.css`): `.karte` is the scroll container of the page and was limited to
  1400 px and centred; in wide windows its scrollbar therefore stood beside the stage instead of at the window edge,
  in the colours of the system. Now `.karte` spans the full width and moves the content to the middle via
  `padding-inline`; `.karte` and `.mm-canvas` set `scrollbar-color` from the variables of the page. The cause is
  derived from the stylesheet and asserted there; it has not been looked at in the browser (manual check by the
  user).

### Revisions and assistants

- **Counter in the ribbon** (`fragments/edit_toolbar.html`): the same switch as in the editor head from `rev.fold`
  („(+ n ausgeblendet) ▸“ or „n ältere verbergen ▾“, same addresses); ‹ and › carry `rev.fold.q` along.
- **Own open findings only when checking** (`assist/chat.rs`, `is_check_order`): `OrderFrame::recheck` of a directly
  addressed persona is `rfc || is_check_order(message)`. A check request is a message in which a word (outside
  mentions and quotation lines) begins with `prüf`, `überprüf`, `nachprüf`, `gegenprüf`, `kontrollier`, `check` or `review`, or
  which names a finding "[B<n>]". „Prüfen“, ↻ and RFC stay re-checks. The detection is a word list, not
  understanding: „geprüft“ does not count, „Checkliste“ counts.
- **`propose_chapter` on the right basis** (`assist/mod.rs`, `assist/basis.rs`): the proposal for the part of the job
  is created via `Store::add_proposal_on` on `prepared.shown_rev` (chosen or shown), the other parts as before on
  their current revision. `Outcome::basis` comes from `basis::of_proposal` (head line „Grundlage: …“) and stands in
  the answer of `persona.propose` – also for the proposal for a section. A text equal to the shown revision counts as
  unchanged.
- **`baruch_vw` on the first call** (`web/src/vw.ts`, `base.html`): a classic script in the `<head>` before the module
  `main.js`. It runs before the first paint, compares the width class of the window with the cookie, rewrites the
  cookie on a difference and sets the attributes on `<html>` by the same formula as `laptop.rs` and `laptop.ts`
  (thresholds 1280/1600). If the cookie is right, it does nothing. The server cannot know the width on the first
  request; without JavaScript it stays "wide". `vw.js` is delivered via the existing route `/static/js/{name}`;
  `build.rs` compiles it like the other files under `web/src`.

### Settings, editor, mirror

- **„Sicherung angelegt“** (`settings/data.rs`): a process-wide note per data folder
  (`static BACKUP_NOTES: Mutex<BTreeMap<PathBuf, String>>`, no field in `AppState`). The form stores it after a
  successful backup, the page with `?ok=sicherung` picks it up exactly once. After a restart it is gone;
  `settings.backup` via RPC answers directly as before.
- **Permissions of the data folder** (`settings::open_permissions`, `permissions_warning`): read only
  (`metadata().permissions().mode() & 0o777`, reported if `& 0o077 != 0`), only under Unix. Baruch never changes the
  permissions of an existing folder.
- **Token list** (`settings/mcp.html`, `settings.css`): class `token-former` with `white-space: nowrap`.
- **Button „Zitat“** (`ribbon::QUOTE_SNIPPET`): `#quote(block: true)[]\n` without the following line "– ".
- **`outline_mentions`** (`xref.rs`): ordinal numbers up to `MAX_ORDINAL` = 100 built from stems and compared on the
  whole stem („achtzehnten“ is 18, not 8); Roman numbers up to 3999, only in the usual notation. The rule "only when
  unambiguous" is unchanged.
- **store-5** (`baruch-store/src/reste36.rs`, `outline.rs`, `mirror.rs`): `Store::outline_delete_losses` reads all
  revisions except the current one (`proposal` for `status = 'proposed'`) and all posts of the item. The server calls
  it under the write lock immediately before `outline_delete` – when the human deletes and when a request "delete" is
  confirmed – and passes the result to `mirror::record_outline_losses`: the same one commit, in the body additionally
  every revision and every post deleted along with it in its wording, plus the trailers `Baruch-Lost-Revisions` and
  `Baruch-Lost-Posts`. Without losses the commit is as before. A request alone deletes nothing; only the human can
  delete.

### Limits and honesty

- No browser in this round: token list, light stripe, position of the labels in the real typeface and the early
  script `vw.js` are checked via markup, stylesheet, generated SVG and the Node harness, not seen.
- The note „Sicherung angelegt“ applies per process and data folder, not per session: with several humans on one
  instance, whoever loads the page with `?ok=sicherung` first sees it.
- Items not worked on stand with a reason in `docs/Backlog.md` („R36: nicht bearbeitet, …“).

### Tests

- `crates/baruch-store/tests/reste36.rs`: `outline_delete_losses` (remaining revisions, posts, read only, unknown work
  and unknown item).
- `crates/baruch-server/tests/round36_lektor.rs` (work binding, display name, „Im Werk suchen“, cover image, theme),
  `round36_mindmap.rs` (round trip `.mm`/`.xmind`, foreign branch „Wolke“, labels in the SVG, stylesheet),
  `round36_reste.rs` (ribbon counter, `vw.js` on the server and in the Node harness, backup, permissions, token list,
  quotation, mentions, mirror commit when deleting directly and via a confirmed request).
- In existing files: `round35_konten.rs` (the emergency path lifts the lock), `kritik_zyklus.rs` (proposal for the
  chapter on the shown or chosen revision), `round22_befunde.rs` (simple question without own findings, check request
  with).

### Typst command line (task `typst_alias`)

`main` decides before the start of the Tokio runtime via `typst_cli::dispatch(args)`: if the program is called `typst`
or `baruch-typst` (file name of argv[0], without `.exe`, case-insensitive) or the first argument is `typst`,
`typst_cli::run` runs and the process ends with its exit code (0 success, 1 typesetting or file error, 2 wrong call or
something unsupported) – without data folder, database, server or network. `baruch-server/src/typst_cli.rs` only reads
the arguments and writes the files (only after successful typesetting); the typesetting stands in
`baruch-render/src/cli.rs` (`compile_file`, `font_names`) as a child module of `baruch-render`, which shares fonts,
library and the German messages of the server. The world `Disk` reads files only under the root folder (resolved,
symlinks pointing outside blocked) and refuses packages with `OFFLINE`. `TYPST_VERSION` follows the crate `typst` in
`Cargo.lock` (tests and `scripts/ci-typst.sh` pin that). The packages deliver only the name `baruch-typst` (symlink
or `.cmd`). Usage and limits: `docs/Verteilung.md`, "Typst command line".

### Database access with sqlx 0.9 (task `sqlx09`)

Store, library and server use a common sqlx build 0.9 (SQLite bundled via libsqlite3-sys 0.40; the server additionally
PostgreSQL with rustls/ring and system certificates for pgvector). `query`, `query_as`, `query_scalar` and
`begin_with` take only `&'static str` or a text confirmed with `sqlx::AssertSqlSafe`. Rule (module comment of
`baruch-store/src/lib.rs`): SQL as a literal where possible; `AssertSqlSafe` only for text from fixed building blocks
of the source (column lists, placeholder chains, fixed conditions, table names from literals, for pgvector checked
identifiers via `quoted()`); values are always bound. The migrations and their checksums in `_sqlx_migrations` are
unchanged compared with 0.8 (tests `sqlx09_migrations` in store and library, `round36_sqlx` in the server). New
compared with 0.8: sqlx-sqlite 0.9 honours `-- no-transaction` at the head of a migration; no migration of Baruch
carries the marker, 0040 still needs `baruch_store::migrate` with `FOREIGN_KEYS_OFF`.

### Acceptance of the integration

`crates/baruch-server/tests/round36_abnahme.rs` (integrator) checks via the built binary the seam between export and
Typst command line: import of an invented Typst file with an image, `export --out` and `export --pdf`, then
`baruch-typst compile` (symlink) and `baruch-server typst compile` on the exported file – all three PDFs are byte for
byte the same (work without cover and front matter; with a cover the export puts its frame around the source, the
command line typesets only the file). `watch` ends with exit 2 and writes nothing.

From the visual check of the integration: the head line of a proposal in the reading flow (`pv-label`) was the only
place of the work page naming only the handle; `konten_ui::names::ORIGIN_CLASSES` now includes the class
(`round36_lektor`). In the editor head the chip with display name and handle is shortened with "…" when the middle
column is narrow (`.editor-head > .rev-meta`, like every long origin before) – in the backlog.

## Help on the website (Round 37)

The user guide no longer stands in `docs/`, but on the website (Hugo, `site/`):

- **Source:** `site/content/de/hilfe/*.md` (users, 14 pages, entry `_index.md` with `aliases: ["/bedienung/"]`),
  `site/content/de/betrieb.md` (operators), `installation.md` and `mcp.md` (first setup). `docs/Bedienung.md` and
  `docs/Betrieb.md` only carry "where is what" and technical notes.
- **Theme:** `site/themes/baruch/layouts/section.html` (start page of the help), `layouts/hilfe/page.html`
  (navigation of the section, table of contents, previous/next), shortcodes `hilfebild` (image from
  `static/bilder/hilfe/`, the name must stand in `site/data/hilfebilder.toml`, dark version via `<picture>`, link to
  the full size), `taste` and the older `bild`. No JavaScript, no foreign addresses.
- **Images:** `site/tools/bildschirmfotos.py` builds, on a throwaway server (`--listen 127.0.0.1:0`, own data folder,
  `--secrets memory`, own `HOME`), the invented sample work `site/tools/beispielwerk.typ` and photographs every line of
  the image list with Firefox via an intermediary in the script (cookies, session, opening `<details>`, click,
  measuring a cut-out). `--pruefen` proves every address without Firefox, `--abgleich` keeps list, pages and files in
  line with each other. Details: `site/tools/README.md`.
- **Check:** `scripts/site-check.py` (called by `scripts/ci-site.sh` after the Hugo build): dead internal links and
  anchors, images without alt text, JavaScript, foreign addresses, redirect `/bedienung/` → `/hilfe/`. Tests
  `tests/round37_hilfe.rs` (pages, anchors, image names, forbidden words, keys) and `tests/round37_bilder.rs` (image
  list, help link).
- **User interface:** `about::HELP_URL`; entry „Hilfe“ in the menu „Baruch ▾“ (`templates/base.html`) and on „Über
  Baruch“ – an ordinary link to the outside, Baruch itself fetches nothing.

## Round 38 – English as the primary language, work language

> From Round 38 on, new sections of this document were written in English; Round 38c translated the older sections
> as a whole. German stays only where it is quoted (texts of the user interface, German model output words) or code.

**Language rule (`AGENTS.md`).** The primary language is English for code, comments, commits, developer docs, MCP
and RPC. The user interface stays German until Round 39 (then German and English through catalogs). File names,
routes, RPC and MCP names, identifiers, cookies and error codes did not change in this round.
`scripts/check-comments-only.py` proves that a comment translation changed nothing but comments (comparison of the
text without comments per file, doc-test code blocks and intra-doc link targets unchanged; `--umlaute` lists
umlauts left in comments, exceptions in `scripts/comment-umlaut-allow.txt`; `--selftest` runs in
`scripts/ci-selftest.sh`).

### Work language

Each work has a **work language**, `de` or `en`. Personas answer in this language; book output follows it from
Round 39 on.

- **Work language is not the language of the user interface.** Changing it changes neither menus nor messages.
  It is also independent of `TypesetSettings.lang` (`<satz sprache>` in the exchange format, hyphenation); the
  coupling comes in Round 39.
- **Migration `0055_work_language.sql`** (store): `ALTER TABLE work ADD COLUMN language TEXT NOT NULL DEFAULT 'de'
  CHECK (language IN ('de', 'en'))`. `ADD COLUMN` only, no table rebuild, therefore not in `FOREIGN_KEYS_OFF`.
  Existing works become `de`. 0056 was reserved for the integrator of the round; the library got no migration
  (0109 stays free).
- **Database only:** the work language lives only in the database; changing it is no commit in the mirror (like
  `work_timing`).

Modules:

| Module | Content |
|---|---|
| `crates/baruch-core/src/language.rs` | `WorkLanguage { De, En }` (default `De`, serde lowercase), `ALL`, `as_str()` (`"de"`/`"en"`), `parse()` (exactly `"de"` or `"en"`, no trimming, case-sensitive), `english_name()` (`"German"`/`"English"`) |
| `crates/baruch-store/src/language.rs` | `Store::work_language(work)` (`UnknownWork` for a missing work), `Store::set_work_language(work, language) -> bool` (`true` = changed; one `BEGIN IMMEDIATE` transaction; a change sets `work.updated` like `update_work`); `WorkSummary` and `WorkMeta` carry `language` |
| `crates/baruch-server/src/work_language.rs` | RPC `work.language`, `work.language_set` (`handles`, `call`, same shape as `crate::timing`); `of(state, work)` reads the work language for a model call, `fallback(work, read)` decides what an unreadable work language means (see below) |
| `crates/baruch-server/src/assist/lang.rs` | marker sets `DE` and `EN` (`Markers`), `markers(language)`, `language_rule(language)`; further markers `MORE_DE`/`MORE_EN` (`MoreMarkers`: `besser:`/`better:`, `Schritt:`/`Step:`, `befund`/`finding`, `Stelle`/`place`, `was fehlt`/`what is missing`, `was stört`/`what disturbs`); `localized(text, language)` swaps the German markers named in a prompt text for those of the work language; `of_prompt(system)` reads the work language of a call back from its language rule (used for the retry notes) |
| `crates/baruch-server/src/assist/prompts.rs` | `default_prompt_in(handle, role, kind, language)`, `effective_prompt_in(handle, role, stored, kind, language)`; `default_prompt_for` and `effective_prompt_for` are wrappers with `WorkLanguage::De`. `effective_prompt(handle, role, stored)` (settings page, `persona.prompt_get`) deliberately carries **no** language rule: a prompt saved from the settings page must not take the rule of one work into another |

**RPC, MCP, form.**

- `work.language {work}` → `{"work": id, "language": "de"}`. `work.language_set {work, language}` →
  `{"work": id, "language": "en", "changed": true}`: only the human with full rights, not through `/act`, not
  through MCP (pattern `work.timing_set`); an invalid value is error -32602 with
  `language: allowed values are "de" and "en"`. Neither method is in `LEKTOR_RPC`.
- `work.get` (top level, beside `work`), `work.meta` (`work.language`) and `work.list` (per work) additionally
  carry `language`. MCP has no new tool; `list_works` (per work) and `outline` (top level, not inside `work`)
  carry `"language"`.
- Form `POST /einstellungen/werk/sprache` (fields `work`, `language`; in `settings::writes()` behind
  `guard::same_origin`, handler `settings::work::language_act`); it answers 303 back to the page with
  `ok=sprache#sprache` or `sprachfehler=1#sprache` (pattern `timing_act`) and is not in `LEKTOR_ROUTES`. The
  block „Werksprache“ in `templates/settings/werk.html` (`id="sprache"`, directly before `id="zeit"`) is a plain
  form with a `<select name="language">` („Deutsch“ = `de`, „Englisch“ = `en`) and works without JavaScript.
- An unknown or missing work at `POST /einstellungen/werk/sprache` (also when no default work can be determined)
  changes nothing and answers 303 to `/einstellungen/werk?sprachfehler=werk#sprache`; for `sprachfehler=werk` the
  block „Werksprache“ shows the message
  „Die Werksprache ließ sich nicht speichern: dieses Werk gibt es nicht; nichts geändert.“
  and for any other value of `sprachfehler` the previous message.

**Unreadable work language.** `work_language::of(state, work)` never fails a model call: if the store cannot read the
work language, the pure function `work_language::fallback(work, read)` answers `de` together with a message, and `of`
prints it to stderr as `work language of work <id> unreadable, answering in German: <error>`. The persona then
answers in German, as before Round 38. The exchange export (`exchange_dump`) keeps reading the column directly; the
CHECK of migration 0055 rules out values other than `de` and `en`.

**Which texts are English.** English since Round 38: the MCP `instructions`, all tool and parameter descriptions,
stub answers and error texts that only arise in `src/mcp/**`; for RPC the protocol errors (parse error, invalid
request, unknown method, parameter check, login/role on `/rpc`). Error codes are unchanged. German until Round 39:
every message that a page, a form or a notice bar shows as well (e.g. the texts of `StoreError::Refused`, progress
reasons like `CRITICAL_RETRY_REASON`). `docs/MCP.md` and `docs/RPC.md` name this boundary.

**Persona prompts and bilingual markers.** The default prompts (`assist/prompts.rs`) are English; the task texts
that Baruch puts around the text of a section (`assist/context.rs`, `solutions.rs`, `variants.rs`, `glossary.rs`,
`threads.rs`) are **still German** (open, see `docs/Backlog.md`) – the language rule still fixes the answer
language. Prompts stored by the user stay untouched. The prompt constants name the German markers; for an English
work `lang::localized` swaps them. Under every effective prompt of a model call (stored ones as well) Baruch appends
`lang::language_rule(language)`: a fixed English sentence telling the model to always answer in the work language,
whatever language the prompt or the question uses, and to use the markers of that language literally (the list
includes the solution markers `besser:`/`better:` and `Schritt:`/`Step:`). The prompt
prescribes the markers of the work language; **every detection accepts both languages**, independent of the work
language (`critical_line`, `has_critical`, the duty to propose a solution, `companion_line`, `all_done`,
`findings/parse.rs`, `chat.rs`, forbidden introductions, the word list for review requests). For `de` the
behaviour is literally the same as before Round 38. Notes that Baruch itself writes into posts (not backed, no
solution, mandatory finding missing) follow the work language. The words Baruch reads back from @schrift and
@belege (`Wortlaut ungeprüft`, `nicht belegt`, `Angabe prüfen`) are recognised in German only; the full table of
markers and where they are read is in `docs/RPC.md`, "Work language".

| Field of `Markers` | `DE` | `EN` |
|---|---|---|
| `critical` | „Kritisch:“ | `Critical:` |
| `suggestion` | „Vorschlag:“ | `Suggestion:` |
| `note` | „Hinweis:“ | `Note:` |
| `praise` | „Lob:“ | `Praise:` |
| `what_works` | „Was trägt:“ | `What works:` |
| `important_now` | „Jetzt wichtig:“ | `Important now:` |
| `can_wait` | „Kann warten:“ | `Can wait:` |
| `my_proposal` | „Mein Vorschlag:“ | `My proposal:` |
| `done` | `DONE_MARK` | `done:` |
| `instead_of` | „statt:“ | `instead of:` |
| `all_done_sentence` | `ALL_DONE_SENTENCE` | `All my findings on this section seem to be resolved.` |
| `unbacked_note` | `UNBACKED_NOTE` | `(not backed by the text)` |
| `no_solution_note` | `NO_SOLUTION_NOTE` | `(no solution proposed)` |

**Exchange format.** Minor version 10: attribute `sprache` (`de`|`en`) on `<angaben>`, always written; a file
without it (every file up to minor version 9) means `de`. Model: `ExchangeLanguage { De, En }`, field
`ExchangeWork::language`. An unknown value is an error of the reader (`…/@sprache: unbekannter Wert „…“`, like
every enumeration attribute). A new work takes over the language; when merging into an existing work the language
is treated like the field `bible`: the local value stays and the report has the line „Werkangabe „Sprache“ weicht
ab (bleibt)“. Details: `docs/Austauschformat.md`, "Work language
(minor version 10, Round 38)".

**Tests.** `crates/baruch-store/tests/migration_0055.rs` (migration on a rebuilt database in the state of 0054,
lossless, all works `de`, the CHECK rejects `fr`), `work_language.rs`, `exchange_u10.rs`;
`crates/baruch-exchange/tests/xsd.rs` (u10 schema, frozen older schemas);
`crates/baruch-server/tests/round38_sprache.rs` (RPC, MCP, form also without JavaScript, editors locked, prompts
and bilingual detection, MCP `initialize` and `tools/list` without umlauts), `round38_doku.rs` (the translated
developer docs, the head paragraph of the history files, the rules in `AGENTS.md`), `round38_abnahme.rs`
(acceptance of the round).
