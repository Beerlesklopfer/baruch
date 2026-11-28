# Structure of Baruch

Cargo workspace under `crates/`. The user interface is delivered by `baruch-server` (axum + minijinja) as HTML,
the core is reachable through JSON-RPC. Plan and stages: `docs/Plan.md`.

## Crates

| Crate | Task |
|---|---|
| `baruch-core` | Types (work, section, revision, persona, post with `id` and `created`), access to revisions by number, switching (`Node::set_current`), lector choice (`LectorChoice { Auto, Fixed(handle) }`, default fixed `@lektor`, `Work::lector_choice`, `LectorChoice::pick`), line diff, sample data |
| `baruch-llm` | Model backend Eliezer (Round 10), pure (no DB, no keyring; reqwest 0.12 with rustls, `stream`; tokio; futures-util): `Endpoint {base_url, secret}` (`Debug` without the key), `models`/`models_with` (`GET /v1/models`), `chat_stream`/`chat_stream_with` (`POST /v1/chat/completions` with `stream: true`, `stream_options.include_usage`, SSE parser `sse::Parser`), `Limits`, `LlmError` (eight kinds like `assist::ModelFailure`), `Queue` (one request at a time, at most 8 waiting, `QueueStatus`), `alias`/`ALIASES`, `mock::MockEliezer` (mock for tests); Round 12: `embed.rs` (`embeddings`/`embeddings_with`, `POST /v1/embeddings`, answer ordered by `data[].index`; `embedding_alias` `nomic-embed-text` → `text-embedding-nomic-embed-text-v1.5`, separate from the chat aliases; `Endpoint::embeddings_url`), the mock answers embeddings deterministically (`mock_vector`, 512 dimensions, `set_embedding_models`/`_dimension`/`_delay`, `fail_embeddings`, `embed_max_active`) |
| `baruch-rag` | Common interface of the RAG ways (Round 12, contract appendix A, literally): `Way`, `SearchRequest`, `Query`, `Hit` (`identity` = document + place), `MergedHit`, `WayState`/`WayReport`, `Answer`, `IndexInfo`, `Status`, `RagError` (nine kinds, `is_off`), ports `SearchWay`, `Embedder`, `EmbedderFactory` (without `async-trait`, `BoxFuture`), `clean_snippet`, limits `MAX_LIMIT` 50, `MAX_QUERY_CHARS` 1000, `MAX_SNIPPET_CHARS` 600, `MAX_EMBED_BATCH` 32, `RRF_K` 60. Pure (serde, serde_json) |
| `baruch-lancedb` | Local vector index (Round 12) through the crate `lancedb` 0.41 (Apache-2.0; Arrow 58 only through its re-exports, without cloud storage, `remote` and built-in embedders; building `lance` needs `protoc`), table `chunks` per folder, `baruch-index.json` (model, dimension, build), flat cosine search, its own small Tokio runtime (`runtime()`), chunks (`chunk`: paragraphs, short ones together, long ones split at sentence ends, SHA-256). Pure: no SQLite, no network |
| `baruch-scripture` | Bible references (Round 10), pure (std + serde): 66 books with Schlachter names and abbreviations (`books`, `find_book`), verse counts per chapter from `data/versification.tsv` (numbers only), `parse` (references), `scan` (running text), `Passage`, `RefStatus`, `Found`, `VerseText`/`Wording`/`NoBible` (no Bible chosen: „ungeprüft“), `SOURCE`; since 2026-11-06 `versification` (German, KJV; `data/versification-kjv.tsv`) and `wording` (find quotations, compare normalised, word diff, join separated digits); since Round 17a all 18 versifications of `sword-modules` (only its data, `default-features = false`) and `versification::mapping` (mapping through `data/versification-map.tsv`) |
| `baruch-sword` | Reading SWORD Bible modules (2026-11-06): searching module folders (`scan`, conf entries including the licence note), `open` → `Bible` (`verse`, `passage`, `search`), plain text from OSIS/ThML/GBF (`text`), test modules (`fixture`); format reader `sword-modules` 0.1.0 (CC0-1.0), only German/KJV, see "SWORD Bibles" |
| `baruch-typst` | Splitting a Typst file into work template and headings (`parse`) and back (`join`), numbering from the template (`heading_numbering`), outline schemes (`scheme`), image paths (`images::image_paths`; computed images of the template `images::computed_images`, Round 11), cover and front matter (`cover`, Round 11: `frame_parts`, `wrap_source`, `isbn13`, `ean13_svg`, `spine_mm`), concatenation with byte ranges (`join_with_offsets`) and line as in Typst (`line_at`); Round 14: part and „ohne Nummer“ as a marker (`marker`: `part_line`, `unnumbered_line`, `relevel_line`, `set_numbered_line`, `part_page_rule`, `PART_LABEL`, `UNNUMBERED_LABEL`; `Heading.level` 0 = part, `Heading.numbered`); Markdown headings (`markdown`: `markdown_headings`, `markdown_heading_line`, user 2026-11-07, same scanner as `parse`), pure, without DB |
| `baruch-wysiwyg` | Typeset view editor, pure, without DB and file system: section text ↔ blocks (`parse`, `serialize`, `emit`; untouched blocks literally, round trip byte-identical; a heading ends at the first line break, `split_heading`), Typst islands, HTML for typeset view and diff (`render_html`, `diff_html`), with images in islands (`render_html_with`, `diff_html_with`, `image_path`, `ImageRef`), presentation from the work template (`sheet_style`, `sheet_css`, only top-level rules), alignment of proposals to blocks (`src/align.rs`: `align(base, proposal) -> Vec<Hunk>` over the longest common subsequence of the top-level blocks, `Hunk {kind: Changed\|Inserted\|Removed, base, proposal}`, `hunk_html_with`); Round 14: block kinds `part` and `unnumbered` for the markers (top level only, recognised through `baruch_typst::parse`); depends only on `baruch-typst` (`heading_line`, `part_line`, `unnumbered_line`) |
| `baruch-docx` | Word files (Round 32), pure, without DB and network (`zip` 9 only `deflate`, `quick-xml` 0.42, `baruch-typst`, `baruch-wysiwyg`): common model (`model.rs`: `DocxDocument`, `Block`, `Inline`, `Change`, `Comment`, `DocxImage`), `DocxError`, `Limits`, fixed OOXML names (`ooxml.rs`), sample (`sample.rs`); reader `read` → `resolve`/`proposal_view` → `to_typst`; writer `from_typst` → `diff::track_changes` → `write`. Seam of both sides: `tests/roundtrip.rs` |
| `baruch-render` | Typst typesetting (Round 8), pure, without DB and network: embeds `typst` 0.15.1 (`typst-svg`, `typst-layout` for `PagedDocument`, `typst-kit` only with `embedded-fonts` and `scan-fonts`; no packages). `Source {main, files}`, `stand(&Source)` (SHA-256 over text and files sorted by path = cache key), `compile(&Source) -> Rendered {stand, pages: Vec<SVG>, headings: [HeadingPos {line, page}], diagnostics: [Diagnostic {severity, message, hints, line, column, file}]}` (blocking, never fails). The world knows only `/main.typ` and the files in normal form; package ids → error „offline“; fonts once per process (embedded, then system); known messages in German; after each run `comemo::evict(2)`. Round 11: `compile_pdf(&Source) -> Pdf` (`typst-pdf` 0.15.1, Apache-2.0; the same world, the same page count), print check `Rendered.print`/`Pdf.print` (`PrintWarning`, `MIN_DPI` = 300), `sha256_hex` |
| `baruch-store` | SQLite as the source (sqlx, runtime API, migrations 0001–0099): loading, accepting, switching, discarding, saving (on a base), changing, deleting, proposal, post, Typst import – one transaction each (`BEGIN IMMEDIATE`); revision tree (`src/tree.rs`); title rules (`check_title`); images (`src/assets.rs`, 0009, `normalize_asset_path`); personas with colours, role, model, context (`src/personas.rs`, 0010; `PersonaRow`, `PersonaEdit`, `persona_list`, `update_persona`); connections, RAG ways, preferred sources (`src/connections.rs`, 0013); ideas (`src/ideas.rs`, 0011); managing posts (`src/posts.rs`: `PostInfo`, `post_get`/`post_delete`/`post_move`/`post_to_idea`, conversion in one transaction through `ideas::create_in`; `StoreError::UnknownPost`; no migration); snapshot for Git, Typst export, export map (`src/export_map.rs`: `ExportMap {typst, sections: [ExportSection {key, seq, start, end, first_line}]}`, `typst` byte-identical with `export_typst`, both from `snapshot::export_parts`; `section_at_line`); title check only for changed titles (`check_chunk_titles(chunks, old_title)`); Round 11: book data and cover (`src/book.rs`, 0017), requests for images (`src/asset_requests.rs`, 0018), `asset_search`, `save_persona`; Round 12: collections per work and way (`src/rag_collections.rs`, 0031: `work_collections`, `set_work_collections`), RAG evidence per persona (`src/persona_rag.rs`, 0032: `persona_rag`, `set_persona_rag`) |
| `baruch-gitmirror` | Git mirror with gix (pure Rust, without network): snapshot commit with author, committer, trailers; no commit without a change, except with `allow_empty` (log); no writes through symbolic links |
| `baruch-library` | Author library (`SearchHit::node_key` = fixed key of the section): titles, quotations, highlights (`src/highlights.rs`, 0103), FTS5 index `search_fts` over revisions, posts, titles, quotations and highlights, which follows changed and deleted revisions (0104) as well as deleted and moved posts (0105) (migrations 0101–0199) |
| `baruch-server` | HTTP server: pages, fragments, widgets, literature as a popup (`/literatur`, `/quelle`), ideas (`/einfaelle`), header with menus (`menu.rs`), settings as a popup (`/einstellungen/<seite>`, `settings/`), connections and sources (`connections/`), typesetting (`/werke/{id}/satz`), personas with model call (`assist/`, `models.rs`), reference check (`scripture.rs`), MCP tokens (`mcp/tokens.rs`), semantic search (`rag/`, `search_hooks.rs`, `connections/{pgvector,multigpt}.rs`, Round 12), `POST /rpc`, `POST /act`, `POST /literatur/act`, `POST /beitraege/act`, `POST /assistenten/act`, `POST /assistenten/stream`, `POST /mcp` |

## Server (`crates/baruch-server/src`)

- `lib.rs`: `AppState` (store, data folder, default work, mirror per work, write lock, templates,
  secrets, since Round 10 `model: Arc<dyn assist::ModelPort>`), routes, `/act`. `AppState::open` and
  `open_with_secrets` take `assist::NoModel`, `open_with(data, secrets, model)` an own backend; `serve`
  installs `assist::eliezer::EliezerPort`. Start in fixed order: `Store::open` → `baruch_library::migrate` → `seed_if_empty`
  → open the mirrors of **all** works and bring them up to the DB state (errors only on stderr).
  `AppState::mirror(work)` opens a mirror on first use and remembers it; after an
  error it is opened again the next time. `mirror_dir` checks the folder name
  (`^[a-z0-9]+(-[a-z0-9]+)*$`, at most 48 characters) and refuses `works/` or `works/<slug>` as a
  symbolic link.
- `localtime.rs` (user 2026-11-07): times of the user interface in the machine's **local time** (`chrono::Local`, under
  Unix `TZ` or `/etc/localtime`, daylight saving time per instant). Storage stays UTC
  (`strftime('%Y-%m-%dT%H:%M:%fZ','now')`, `models::iso_now`, Unix seconds), RPC fields stay UTC; conversion
  happens only when the page or a pure display text is built (revisions, ideas, images, works, connection
  check, sync, check time of highlights, MCP clients and tokens, retrieval date of the web search in evidence and
  source line, „lokale Kopie vom …“, folder name in the trash). Every function exists as `…_in(…, &zone)`; tests
  compute with `FixedOffset` and check `Local` in a child process with `TZ=Europe/Berlin` (no environment variable in
  the shared test process). Unreadable stamps stay as they are.
- `import.rs`: Typst file as a new work for the CLI `baruch-server import [--title …]` (`import_typst`,
  `import_file`, `import_file_as`) and the upload (`import_upload`, only `.typ`, at most 16 MiB): under the
  write lock `Store::import_typst_titled`, afterwards the commit „Import aus <datei>“ in the new mirror.
- `works.rs`: work management (see "Managing works"): RPC `work.list/meta/create/import/update/delete`
  (from `rpc::call` before the default work is chosen), pages `/werke…` with their own template environment
  (`works.html`, `work_new.html`, `work_edit.html`, `work_delete.html`, `fragments/work_new.html`), forms
  `POST /werke/import` (multipart, `DefaultBodyLimit` only there 16 MiB + 64 KiB) and `POST /werke/act`, archive
  of deleted mirrors (`move_to_trash`). `works::RPC_BODY_LIMIT` (= 2 × 16 MiB + 64 KiB) is the limit for
  `POST /rpc`: `work.import` carries the text as a JSON string, and escaping can double it; `work.import` accepts up
  to 16 MiB of text (above that -32602), axum refuses a larger request beforehand with 413. Not through MCP. The page `/` chooses the work from `?work=`, otherwise from the cookie
  `baruch_work` (if the work exists), otherwise `AppState::default_work()`, and sets the cookie with every answer.
- `rpc.rs`: JSON-RPC methods (`docs/RPC.md`); `/act` calls the same methods for forms without JavaScript
  (when deleting, `/act` checks the box `confirm` itself and passes on `confirm: true`).
  Switching, changing and deleting revisions (`revision.set_current`/`update`/`delete`) exist only here, not
  through MCP. `library_view` delivers, next to the literature (for the popup), the revision tree of the work
  (`Store::revision_tree`) for the editor and the tree view.
- `posts.rs` (Round 8): managing posts: RPC `post.delete/move/to_idea` (`handles`/`call`, from `rpc::call`
  after the work is chosen), common procedure `act` for RPC, form and MCP (write lock → `post_get` →
  ownership check → change → `mirror::record_post`, exactly one commit), `writes()` with `POST /beitraege/act`
  behind `guard::same_origin`. `mirror::describe_post`/`record_post` (author as with ideas).
- `pdf.rs`, `cover.rs`, `asset_requests.rs` (Round 11): book as PDF, cover/front matter/book data, requests
  for images – see "Images, cover and PDF".
- `typeset.rs` (Round 8): Typst preview: `prepare` (export map, images, frame from `cover::frame`, stand) → `compile`
  (cache hit, otherwise a static `Semaphore` with one slot, `spawn_blocking`,
  `tokio::time::timeout(COMPILE_TIMEOUT = 60 s)`; the permit moves into the blocking run, an expired
  run computes to the end and fills the cache). Cache static
  (`OnceLock<Mutex<VecDeque<Arc<Compiled>>>>`, by `stand`, at most `CACHE_SIZE = 8`) – no field in
  `AppState`. Messages: line → `ExportMap::section_at_line` → `key`, `section`, `section_line`. Chapter =
  top-level outline item above the section (under a part the chapter inside it), page of a section =
  page of its heading (Typst introspection). RPC `preview.typst`, pages `/werke/{id}/satz` (own
  minijinja environment, `templates/typeset.html`, `static/typeset.css`) and `/werke/{id}/satz/{stand}/{n}.svg`
  (only the current stand, `nosniff`, CSP `sandbox`). MCP `mcp/typst.rs` (`typst_check`, through `typeset_at`).
- `mcp.rs` with `mcp/tools.rs`, `mcp/ideas.rs`, `mcp/typst.rs`, `mcp/outline.rs` (outline: creating, requests) and `mcp/session.rs`: MCP server (protocol 2025-06-18, JSON
  only). Tools read through `Store`/`baruch_library::search` and write only proposals
  (`Store::add_proposal`) and posts (`Store::add_post`; moving, deleting, converting only their own through
  `posts::act` – ownership through the handle of the client, since Round 23 unique with a token and, without a token, kept
  apart from token names (`lokal-`, see "Round 23 – Hardening"); none of them hits the human's posts), plus ideas with full CRUD (`mcp/ideas.rs`,
  they change no book text, see "Ideas"); there is no import through MCP.
  Sessions (`Mcp-Session-Id` → clientInfo) process-wide, at most 256. Errors through
  `rpc::RpcError::from`. Description: `docs/MCP.md`.
- `literature.rs`: literature popup (`GET /literatur`), document view of a source (`GET /quelle`),
  forms without JavaScript (`POST /literatur/act`) and the RPC methods `literature.render` and
  `highlight.*`; own template environment, see "Literature".
  Since Round 13 with `literature/fulltext.rs`: full text through MultiGPT (`GET /quelle/multigpt`, RPC `source.read`,
  `source.adopt`, `highlight.check`), offline the local copy from the sync (`local_copy` →
  `sync::local_document`); MCP `source_read` in `mcp/sources.rs`.
- `websearch/` (Round 13): web search for `@belege` through MultiGPT (`web_search`, `fetch_url`), check of the quotations
  in code, evidence (`web_evidence`, migration 0034), evidence page `/websuche/belege`, `POST /websuche/act`, RPC
  `websearch.*`; dispatch in `rpc.rs` before `crate::rag::handles`; no MCP tool. See "Web search".
- `sync/` (Round 13): sync Baruch ↔ MultiGPT (RPC `sync.*`, page `/abgleich`, `POST /abgleich/act`,
  migration 0033), kind `remote` in the local index; dispatch in `rpc.rs` directly after `websearch`; no
  MCP tool. See "Sync".
- Shared MultiGPT session (Round 13): search, full text, web search and sync use the same
  `connections::multigpt::Session` with a process-wide pool (see "MultiGPT"); the HTTP client of a session
  has an upper limit of 30 s (the sync allows 30 s per call), the time limit per call is set by the
  `Limits` of the call. `Session::call` also returns data only without the token.
- `wysiwyg.rs`: RPC `wysiwyg.parse/serialize/style/diff` (`handles`/`call`, from `rpc::call`), read-only, without
  write lock, not under `/act` and not through MCP; keys through `rpc::resolve_keys`. The widget
  `web/src/wysiwyg.ts` (typeset view in the editor form, widget on `div.wysiwyg` around toolbar, sheet and status line) and `static/wysiwyg.css`
  (variables `--ws-*` with defaults) belong to it. Saving goes through the editor form: before submitting,
  `outline.ts` fires `baruch:before-save`, `wysiwyg.ts` fills the text field from `wysiwyg.serialize`.
  Description: `docs/WYSIWYG.md`. `wysiwyg.parse` and `wysiwyg.diff` show islands with an image as an image
  (`div.ws-figure` or `span.ws-inline-image`; `wysiwyg::image_ref`: `src = /werke/{work}/bilder/pfad?p=…`,
  `alt` from `Store::asset_list` through `wysiwyg::image_alts`, keys in normal form
  `baruch_store::normalize_asset_path`, i.e. with or without `/` and `./`).
- `numbering.rs`: outline schemes: RPC `numbering.*`, page `/werke/{id}/nummerierung` (own
  template environment `base.html` + `work_numbering.html`, style `static/numbering.css`), form
  `POST /werke/nummerierung/act`.
- `assets.rs`: images: RPC `asset.*`, pages `/werke/{id}/bilder…`, forms `POST /werke/bilder/hochladen`
  and `POST /werke/bilder/act` (see "Images").
- `personas.rs`: personas: RPC `persona.list`/`persona.set_color`/`persona.update` (before the work is chosen,
  personas are global), `GET /einstellungen/personas` → 303 `/einstellungen/assistenten` (the page is drawn by
  `settings/assistants.rs`), form `POST /einstellungen/personas/act` (behind `guard::same_origin`), `tints(color)` (background light
  as a mix with `#ffffff`, dark with `#1f2328`, in 5 % steps further towards the surface until `--text`,
  `--muted` and `--primary` of the theme (`LIGHT_FOREGROUNDS`/`DARK_FOREGROUNDS`, test against `app.css`)
  reach at least 4.5 : 1), `contrast_ratio` (WCAG 2.x) and `style(color)` =
  `--pc-light:…;--pc-dark:…;--pc-line:…;--pc-line-dark:…` for the `style` attribute; the lines reach
  at least `MIN_LINE_CONTRAST` = 3 : 1 against `LIGHT_SURFACES` (`LIGHT_PANEL` and the sheet of the dark
  theme `DARK_SHEET`, on which `--pc-line` applies too) or `DARK_PANEL` (WCAG 1.4.11, `Tints.line`,
  `Tints.line_dark`). Style `static/proposals.css`: class `.pc`
  sets `--pc-bg` per theme with the same selectors as `app.css` (`prefers-color-scheme`, `data-theme`).
- `ideas.rs`: ideas: RPC `idea.*` (before the work is chosen), pages `/einfaelle`, `/einfaelle/{id}`,
  form `POST /einfaelle/act` (see "Ideas (migration 0011)"). `href()` also checks in release builds
  (`is_safe_href`: `"` `<` `>` `'` space, control characters → fallback `/einfaelle`).
- `menu.rs`, `settings/`, `connections/`: header, settings, connections and sources (Round 9), see
  "Menu and settings (Round 9)".
- `mirror.rs`: commit order from a change (subject, body, author, trailers, `allow_empty`) and commit;
  log for switching, changing and deleting see "Revision tree"; ideas (`describe_idea`,
  `record_idea`, mirror `<data>/einfaelle/` for ideas without a work).
- `guard.rs`: protection of the writing routes against foreign web pages (CSRF).
- `view.rs`: view models, settings from cookies, drawing of page and panes. Context of the
  templates: `work` = `{id, slug, title, author}`, `works` = `[{id, slug, title, selected}]` (work choice),
  `lq_q` = `&lq=…` or empty (appends the search to links), `pick_q` = `&pick=…` or empty, `rev` with `number`
  (number), `position`/`total` („‹ n/m ›“), `is_current`, `current`, `prev`/`next` (numbers), `revtree` =
  `{section, chapter}` (tree view), `proposals` (open proposals of the section as variants in the
  reading flow, `fragments/proposals.html`), `preview` = `[{node, lines, proposals}]` (preview per section,
  proposals with line diff), `rev.author_style`, `personas[].style`, `node.posts[].style`,
  `revtree…[].style` (CSS variables of the persona colour). **Round 8:** `Location.key` and
  `Resolved.key`/`missing_key` (resolution: key before `node`, unknown → `node` or default with a hint),
  `node_url(work, key, rev, preview, pick_q, lq_q)` builds the canonical address `/?work=W&key=K…`,
  `post_targets` (targets for „Verschieben…“), context `node.post_count`, `node.missing_key`, posts with `id`,
  `created`. `Settings.editor` (`satz`|`quelltext`, cookie `baruch_editor`, default `satz`) already decides in the
  server HTML about the toggle; with „satz“ the typeset view stands ready in the sheet (`data-ws-ready`). Proposals:
  `proposals_of(node, shown, ctx, satz)` aligns every open proposal with `baruch_wysiwyg::align` to the
  **shown** revision → `InlineView {key, rev, count, proposals, entries: [FlowEntry {b, raw, lines,
  hunks: [HunkView]}]}` (context `proposals`, in the editor) or `PreviewSection {node, flow}` (context `preview`). `Settings::folded` (cookie `baruch_folded`,
  '.'-separated `<key>-<rev>`, at most 200, only `[0-9.-]`, otherwise discarded) folds proposals; the shown
  proposal is always open (`pv-shown`). Order: position of the persona in `Store::persona_colors`
  (`position`, `id`), then number; authors without a persona entry at the end with `FALLBACK_COLOR`. Base of the diff:
  `based_on`, otherwise the current revision. Typeset view diff through `baruch_wysiwyg::diff_html_with` with
  `wysiwyg::image_ref` and `LibraryView::image_alts` (lookup in normal form); for this `LibraryView` carries
  `persona_colors` and `image_alts`. `Location::resolve` looks up the revision by its number; `pick` applies only
  to the preview.
- `templates/`: `workspace.html` and `fragments/{tree,editor,edit_toolbar,revisions,revtree,assistants,library,
  proposals,idea_capture}.html`; for `literature.rs` also `literature.html`, `source.html` and
  `fragments/highlights.html`. **Editor in panes (request 2026-11-06):** `#pane-editor` does not scroll
  (`overflow: hidden`, flex column) and carries `div.editor-head` (head: title, revision display, „‹ n/m ›“,
  „aktuell: Fassung k“), in the editor `div.wysiwyg[data-widget=wysiwyg]` with toolbar
  (`fragments/edit_toolbar.html`, `div.ws-bar[data-edit-toolbar]`, `hidden` without JavaScript; since
  Round 18a the ribbon with the formats of the work template, `span.ribbon-formats[data-widget=ribbon]`), sheet `div.editor-body` (`flex: 1 1 0;
  min-height: 0; overflow: auto` – scrolls on its own; inside it `form#editor-form.editor-form` with typeset view
  `div.ws-wrap[data-ws-wrap]` (`hidden` without JavaScript) and text field, behind it `section.pv-flow`) and
  status line `div.editor-status` (hints, `[data-ws-hint]`, „Speichern“ with `form="editor-form"`); in the
  preview head, `div.editor-body` with `div.sheet-wrap` and status line. Cause of the formerly too low
  sheet: `#pane-editor` scrolled as a whole, all children had `flex-shrink: 0` (`proposals.css`), revision
  buttons and list stood in the editor too; once the height was used up, the form (`flex: 1`) had no free
  space left, it fell back to `min-height: 12em`, and the typeset view got only a few lines after bar and hints.
  The widget `wysiwyg` looks for the form in its pane (`root.querySelector`), for the save button through
  `widgets.ts` `submitButton` (`form.elements`, also with `form=`). Revision list, „Diese Fassung verwenden“,
  „Fassung ändern“ and „Löschen“ stand in `fragments/revisions.html`, the accordion section
  `details.acc[data-section=revisions]` in `assistants.html` (between assistants and chat; badges
  `rev.total` and `rev.open_proposals`; cookie `baruch_accordion=revisions`, `Settings::accordion`); it comes
  anew with the right column from `ui.render`. Zoom zones (`web/src/zoom.ts`): every `[data-zoom-zone]`
  (`tree`, `assistants`, `revisions`, `head`, `toolbar`, `sheet`, `status`) plus `dialog[open]` and `main`;
  the innermost wins, level per name in the cookie `baruch_zoom`, a `MutationObserver` applies it to newly
  drawn zones. Open proposals stand in
  `fragments/proposals.html` (Round 8: in their place; in the editor `section.pv-flow[data-widget=proposals]`
  after the editor form with the shown revision block by block as `pre.pv-src[data-b]` and in between, per
  hunk, a `details.pv[data-pv="<key>-<rev>"][data-pv-at][data-pv-gap][data-pv-kind]`; in the preview
  `div.pv-section` with the lines and the hunks in between; per proposal ONE form `pv-accept-…` or
  `pv-reject-…` outside the hunks, the buttons point to it with `form=`). With JavaScript `wysiwyg.ts`
  (`placeProposals`) places every hunk as a `contenteditable=false` sibling at the block `[data-b]` of the typeset view
  and hides `pv-flow`; `editsOf`/`blocksIn`/`markSelection` skip `.pv`. Per hunk `div.pv-satz.ws-sheet` (typeset view diff from the server) and
  `pre.pv-quelltext.diff` (line diff). Which one is visible is decided by CSS alone:
  `#pane-editor:has(.wysiwyg[data-ws-open])` shows the typeset view diff, otherwise (also without JavaScript) the
  line diff. Widget `web/src/proposals.ts`: `toggle` (capture phase) → cookie `baruch_folded`,
  `[data-pv-all]` „Alle ein-/aufklappen“ (`hidden` without JavaScript). The proposal card and the widget
  `wysiwyg-diff` are removed (Round 7); the RPC method `wysiwyg.diff` stays.
  Only with an open typeset view does the pane carry `data-ws-open` (set by `wysiwyg.ts`); the typeset view then fills
  the sheet (`static/wysiwyg.css`), in source mode the text field does.
- `web/src/*.ts`: widgets (`data-widget`), compiled by `build.rs` and delivered under `/static/js/`.
- Markdown headings (user 2026-11-07): the server applies `baruch_typst::markdown_headings` to the input
  of the human – `outline.save`/`outline.check` and the confirmation `confirm_page` (`outline.rs`),
  `revision.save` and when accepting `revision.accept` (`rpc.rs::accept_markdown_headings`: the proposal stays,
  afterwards `outline_save` with `split`); `####`–`######` → `===` (capped). Proposals (`add_proposal`, MCP) and
  the work template do not run through the rule. Live: `web/src/markdown.ts` (same cases, cautious
  line scanner) in the widget `editor` (`panes.ts`: Enter, leaving) and in the typeset view (`wysiwyg.ts`:
  `convertMarkdownHeading` on Enter and before `sync`). Tests: `baruch-typst/src/markdown.rs` (table),
  `baruch-server/tests/markdown_headings.rs`.
  `chat.ts` (Round 8): widget `chat` (roles, „neu seit dem letzten Besuch“ through LocalStorage
  `baruch_chat_seen` = `{"<work>-<key>": "<largest created>"}`, every access in try/catch).

## Data

```
<data>/baruch.sqlite          source (WAL)
<data>/works/<slug>/          Git mirror per work, read only
    main.typ                  work template (if imported), then #include of the sections in tree order
    vorlage.typ               only with a work template: the template itself
    abschnitte/NNN-titel.typ  current revision per section; with a work template preceded by #import "../vorlage.typ": *
    bilder/<name>             current image as SVG text (encoding svg)
    bilder/<name>.b64         current image as Base64 in lines of 76 characters (encoding base64)
    einfaelle.md              ideas of the work (only if there are undeleted ones; migration 0011)
<data>/einfaelle/             Git mirror of the ideas without a work (only einfaelle.md), no symlink
```

Migration `0010_persona_color.sql` (Round 7): `persona.color` = `NULL` (default from
`baruch_store::PERSONA_PALETTE` in the order `position`, `id`) or `#rrggbb` lower case (CHECK with `GLOB`).
No data change. `Store::persona_colors` delivers `PersonaColor {handle, name, color, chosen}`,
`Store::set_persona_color(handle, Some|None)` sets or resets (one transaction `BEGIN IMMEDIATE`,
handle also without `@`); unknown → `StoreError::UnknownPersona`, invalid → `StoreError::Color` (both
-32602). `baruch_core::Persona` stays unchanged; no commit in the Git mirror (personas are not book text).
Test `crates/baruch-store/tests/migration_0010.rs` on a rebuilt database in the state of development
(0001–0009, 0101–0105, with image, proposal, post, library). Migration `0011_ideas.sql`: tables
`idea`, `idea_word`, `idea_fts` (see "Ideas (migration 0011)"). Both together are checked by
`crates/baruch-server/tests/round7.rs` through a full server start. Migration `0013_connections.sql`
(Round 9): tables `connection`, `work_rag`, `preferred_source` (see "Connections and sources"), test
`crates/baruch-store/tests/migration_0013.rs`. Migration `0014_connection_secret_target.sql`: only the column
`connection.secret_target` (binding of the secret to its target, NULL = current target), test
`crates/baruch-store/tests/migration_0014.rs`.

`<data>` is `--data`, otherwise `BARUCH_DATA`, otherwise `baruch_store::default_data_dir()` =
`dirs::data_local_dir()/baruch` (Linux `$XDG_DATA_HOME` or `~/.local/share`, macOS
`~/Library/Application Support`, Windows `%LOCALAPPDATA%` – Local instead of Roaming, because server-stored
profiles would otherwise copy WAL and mirror at every logon and logoff).

Without a work template `main.typ` begins with `// Baruch: <Titel>`, and there is neither `vorlage.typ` nor
`#import` lines. The mirror is a projection (`snapshot.rs`): `main.typ` keeps the template at the beginning,
so that set and show rules apply to everything included; the `#import` line makes `#let` functions of the
template available in the section files. The revisions in SQLite and `export_typst` stay without this
line. Whether `main.typ` can be typeset with `typst compile` has not been checked yet (typst is missing on the
development machine).

Folder names (`slug`): lower-case letters and digits, umlauts transcribed, otherwise `-`; at most 40 characters,
shortened at the last word boundary (a single longer word hard), empty → `werk`, collision `-2`, `-3` ….
Existing slugs in the database are not rewritten. Section files are named after the same
`slug`; if the shortening changes, the next mirror commit renames the file.

The sample work („Beispielwerk: Leise Abhängigkeit (Auszug)“, `baruch-core/src/sample.rs`) is seeded only into a really
new database; existing works keep title and slug. Its parts carry no text (`""`, like
`outline.add` for parts), the other nodes without a revision of their own only their heading.

New folder names also avoid the slugs of deleted works (`work_retired`, see "Work ids and seed").

## Typst import

`baruch-typst::parse` splits a file: work template = everything before the first heading; heading =
line from column 0 with 1–3 `=` and space/tab that begins in top-level markup; text per
heading = from its line up to before the next one. `join(template, bodies)` gives back the file byte for
byte. A comment behind the heading (`//`, `/*`; not in raw, not `https://`) does not belong to the
title.

The scanner keeps track across lines: raw blocks (```` ``` ```` at the line start), inline raw across
lines, block comments (nested), strings in code (also multi-line) and a bracket stack from a
`#` expression: `#name(` and `#name[` open, after `)`/`]` of a call another `[…]` may follow;
after `#let`, `#set`, `#show`, `#import`, `#if` … the rest of the line is code (there every `(`, `[`,
`{` opens). In content blocks square brackets count, in top-level running text they do not (there `[` is
text). Only when nothing is open can a line be a heading. In markup `"…"` only counts as a
string if it closes on the same line.

Title/author from the first `#set document(title: "…" | […], author: "…" | ("…", …))` outside of
comments and raw (`blank_comments` replaces them with spaces beforehand, positions stay). Limits:
indented headings and every `#heading(...)` except the two markers (Round 14, below) do not count
(before the first heading they are thus part of the work template); `] else [` at the start of a following line counts as running text. A UTF-8 BOM at the start of the file
belongs to the work template, a heading directly behind it counts.

`Store::import_typst` (one transaction): CRLF → LF, SHA-256 of the source (`work.source_sha256`, unique;
already there → `AlreadyImported`), work (kind `book`, `template`, `source_name`), nodes with `seq` in
file order, `=` → `chapter`, `==`/`===` → `section` (parent = nearest preceding heading with a
smaller level), part marker → `part` (Round 14), numbers through `load::numbers` with the
numbering of the work template (as when loading; default „1“, „1.2“, „1.2.1“), marker „ohne Nummer“ →
`node.numbered = 0`; per node revision 1 by the human, `current`, note
„Import aus <datei>“ (`Store::import_typst_noted`). Migration `0002_work_template.sql`.
`Store::export_typst` puts template and current texts together again.

In the server `import::import_file` reads the file: `std::fs::metadata` checks before opening that it is a
regular file (FIFO, device, folder → „… ist keine Datei“), reading in `tokio::task::spawn_blocking`,
at most 16 MiB, UTF-8. The only caller is the CLI (`baruch-server import`); MCP has no import tool.

**Way back of an export (Round 21b).** What only the export writes from settings comes back to its place on import;
every intervention is an `ImportWarning` in the report (`import::import_source`):
- *Rule block „// Baruch: Satz“* (`store/import.rs::split_typeset`): if the block from the last line
  `RULES_MARK` reaches to the end of the work template and `baruch_typst::typeset::settings_from_rules` reads it back, the
  settings land in `work_typeset` (`typeset::write` in the same transaction) and the block does not land in the template
  (`import-typeset-adopted`). The condition is that `rules` makes **exactly** the same block from the result (never guess;
  page numbers × running heads are tried out, 9 × 4). Otherwise it stays template (`import-typeset-kept`). Limits:
  the R31 fields and `xref_with_page` are not in the block; the gutter comes back as part of the inner margin (above
  80 mm the rest as gutter) – same typesetting, different fields.
- *Show rules „// Baruch: Überschriften ohne Nummer“* (`split_fallback`): every line `#show heading.where(level: N,
  body: [T]): set heading(numbering: none)` at the end of the template → `node.numbered = 0` for the headings with level N
  and literal title T (counterpart of `snapshot::fallback_rules`); a line of a different form leaves everything in the template.
  So „Nummer aus“ with an own label survives the way (before, the section came back numbered). Under „Ohne
  Nummern“ (`numbering.is_off()`) `snapshot::project` since 21b also writes the show rule for `numbered = 0`
  (the text form stays `=`, in typesetting the rule changes nothing) – so the flag is no longer lost there either.
- *Back matter* (`baruch_typst::index::strip_back_matter`): only if the line `BACK_MATTER_MARK` is immediately followed by the
  helper functions that only `back_matter` writes (`index/back.rs::HELPERS`); then `import-back-matter-cut`
  with line. A marker line written by hand cuts off nothing (`has_back_matter_mark` → `import-back-matter-kept`).
- *Projection*: with back matter `strip_projection` (Bible reference marks with `sort`, linked first mention), always
  `strip_first_mentions`: since 21b `index/project.rs` writes the first mention without a glossary page as
  `/*baruch-erste-erwaehnung*/#emph[Wort]` (comment = invisible note, `FIRST_MENTION_MARK`); before, it could not be told apart
  from a highlight by hand and stayed in the text as `#emph` after export → import.
  `import-projection-removed` if something fell out.

**Limits of the numbering (checked in 21b, not fixed):**
- *Functions of the work template* with `p.at(i) ± k`, `slice()` or content as the result are not evaluated per level by Baruch
  (`Numbering::from_template` stays with the approximation). Reason: that would need a Typst evaluation per heading (or
  an own expression evaluator) in the store, which today does without Typst; the typesetting itself is always right (Typst computes),
  only the numbers in tree and outline can differ. Not done in Round 23, stays open (not done in R24, backlog „offen nach R24“; backlog
  „Nach Runde 23 offen“). Proposal (take the numbers over from the typesetting:
  `Rendered.headings` carries page and line, the number could be queried there with `counter(heading).display()`).
- *Parts with `numbered = 0`* (only possible from Round 14 before `bba23db`): without effect – parts never count
  (`load::numbers`), the projection and the show rules leave out parts (`heading_rows` with `kind <> 'part'`), and the
  export writes the part marker independently of the flag; a re-import sets it anew from the text form. No
  migration needed; resetting stays possible through RPC `outline.set_numbered`.
- *Title with code at an identically named numbered node* (e.g. twice `= #emph[Ruhe]`, once „Nummer aus“):
  no marker possible (code in the title), and a show rule `heading.where(body: […])` would hit both. Since Round 22
  a rule over the label (`#show <kN>: set heading(numbering: none)`) in its own block applies to this; the
  re-import reads it back (details in "Round 22 – Revisions and mirror", the item on setting the mirror, and `snapshot::split_label_fallback`).

### Parts in the Typst text (Round 14)

A part and an item „ohne Nummer“ stand in the Typst text as a canonical marker (`baruch_typst::marker`),
one line each from column 0:

```typst
#heading(level: 1, numbering: none)[Erster Teil] <baruch-part>
#heading(level: 2, numbering: none)[Zwischenruf] <baruch-unnumbered> // Kommentar erlaubt
```

- **Why so:** Typst sets both without a definition in the work template; `numbering: none` does not advance the
  heading counter (Typst counts only headings with numbering), so chapters run on across
  parts as in Baruch; the labels make the line unambiguous as a Baruch marker. A freely
  written `#heading(...)` (other spacing, other label, indentation, digit ≠ 1 for the part, in raw,
  comment, content block or string) stays text or work template as before.
- **Title `T`:** escaped like `escape_title`, additionally a `\` before a leading `=`, `-`, `+`, `/` and
  before the dot of a leading `digits.` (otherwise a structure would begin in the content block); `T` contains no
  unescaped `[`/`]`, no comment and no open inline raw. `unescape_title(T)` is the title.
- **`parse`:** part → `Heading { level: 0, numbered: true }`, „ohne Nummer“ → `Heading { level: N,
  numbered: false }`; a marker before the first `=` ends the work template; `join` stays byte-identical.
- **Building:** `part_line`, `unnumbered_line`; `heading_line(0, …)` gives the part marker, a
  marker „ohne Nummer“ stays one when renaming (the comment stays); `relevel_line` changes only the
  level, `set_numbered_line` switches the first heading line between `=` line and marker (for the
  projection in the export, R14c).
- **Import/export:** the import makes a part out of the part marker (top level, the following
  chapters below it) and `numbered = 0` out of „ohne Nummer“; the text stays literal, the export is byte-identical
  (also with a work template), the mirror has one section file per part. The import itself never converts.
- **Conversion of old part texts** (user question 8, decided 2026-11-07, `baruch-store/src/legacy_parts.rs`):
  a start step instead of a migration – the new line needs `baruch_typst` (escaping, cross-check with `parse`), every
  conversion is a revision with a mirror commit (which only the server knows), and the step is testable against a
  rebuilt old database. `AppState::open_with` calls, after `mirror::sync_all`,
  `mirror::convert_legacy_parts` → `Store::convert_legacy_parts(HUMAN_NAME)`: one transaction, flag
  `store_meta.parts_marked_r14` (the seed of a new database sets it right away). Per part the current
  revision counts (`legacy_parts::classify`): empty or exactly one `=` line without label/comment with the title of the part
  or a title that begins with „Teil“ → new current revision `heading_line(0, Titel, None) + "\n"` (author
  the human, note `CONVERSION_NOTE` „Umwandlung Runde 14 (Nutzerentscheidung)“, `based_on` = old revision, which
  is kept as `superseded`); already marked or without a revision → nothing; otherwise the text stays and only the
  node id goes into `PartConversion.kept` (stderr: count and ids, no wording). Per work with conversions one
  commit „Umwandlung Runde 14 (Nutzerentscheidung): n Teile auf die Teil-Markierung umgestellt“ (`allow_empty`,
  trailer `Baruch-Action: parts-converted`, `Baruch-Origin`, `Baruch-Node` per part). Test:
  `baruch-server/tests/round14_conversion.rs`.
- **Typesetting:** `typeset::prepare` puts the rule `part_page_rule(labels)` behind work template and frame (one
  line, `#show <baruch-part>: …`; numbers from `Node.number` of the parts, `typeset::part_labels`): every part
  alone on its own right-hand page (`pagebreak(to: "odd")`, then a weak break), counted with the
  own counter `baruch-part`. The rule uses only `it.body`, so heading rules of the template do not act
  twice; it stands behind the template and thus takes effect first. Typeset page, `work.pdf`,
  `/werke/{id}/pdf` and `export --pdf` use the same `prepare`. Without a part marker the source is
  byte-identical. Shown in `baruch-render/tests/parts.rs` (Typst checks counters and labels itself via
  `#context assert…`, with a cross-check).
- **Typeset view:** block kinds `part` (`title`) and `unnumbered` (`level`, `title`), see `docs/WYSIWYG.md`.

## Managing works

Migration `0005_works.sql` (contract Round 4) and `baruch-store/src/works.rs`:

- **Unique titles:** `work.title_key` = `baruch_store::title_key(title)` (trimmed, runs of Unicode white space
  → one space, `to_lowercase`), unique through the partial index `work_title_key` (`WHERE title_key IS NOT
  NULL`). The migration only creates the column; `Store::open` fills it in Rust after `MIGRATOR.run` and before the seed
  (`works::ensure_title_keys`, `BEGIN IMMEDIATE`, repeatable): works without a key or with an outdated one
  get it. If two works have the same key, only the title changes (slug, mirror folder, nodes,
  texts stay): if a seeded work (without `source_name`) meets an imported one, the imported one with
  the smallest id keeps its title and the seeded one gets `baruch_core::sample::work().title`, if free; otherwise and
  in all other cases the work with the larger id gets „<Titel> (2)“, „ (3)“ … (smallest free number).
  All insert paths (seed, import, creating) and `update_work` set the key; a taken one →
  `StoreError::DuplicateTitle` (-32000), an empty title → `EmptyTitle` (-32602). On import the check
  of the checksum (`AlreadyImported`) comes before the one of the title. Without a work template the title stands in the mirror only in the
  comment line of `main.typ`; a renaming thus changes at most this line.
- **Metadata:** `subtitle`, `audience` (target group), `bible` (Bible translation), `updated` (last change of
  metadata or template). `WorkSummary.updated` is the maximum of `work.updated`, `revision.created` and
  `revision.edited`.
- **Revisions of the work template:** table `work_template_revision (work_id, number, body, author_name, note,
  created)`, numbers from 1, append only. The migration creates revision 1 from `work.template` for every work
  (note „Stand vor Migration 0005“); seed, import and creating write revision 1 themselves. `work.template` stays
  the current revision (= highest number); snapshot and export still read only `work.template`.
  `update_work` creates a new revision only for a different text (CRLF → LF) and refuses a template with a heading
  (`baruch_typst::parse` would find it, the round trip export → import would otherwise no longer be the same outline).
- **Creating** (`create_work`): skeleton `Preset::Book` (chapter „Erstes Kapitel“, template with
  `#set document` and `#set heading(numbering: "1.1")`) or `Preset::Sermon` („Einleitung“, „Hauptteil“,
  „Schluss“, template with `#set document`); title and author escaped in the Typst string (`\`, `"`).
- **Deleting whole works** (`delete_work`, one transaction): `title_key(confirm_title)` must match, otherwise
  `Refused`. The trigger `revision_no_delete_current` (0004) stays unchanged and keeps protecting living
  sections: deleting first sets all current revisions of the work to `superseded` (an allowed transition
  according to `revision_status_transition`), then it deletes revisions (the index trigger from `baruch-library` 0104
  takes them out of `search_fts`), `post_ref`, `post` (the trigger `post_fts_delete` from 0105 takes the posts
  out of the index), `node`, `work_template_revision` and `work`. The same order (first `superseded`, then
  delete) is used by `outline_delete` and the merging of single nodes; two small helper functions in
  `works.rs` and `outline.rs`, deliberately separate. The server
  (`works.rs`) fetches mirror and snapshot beforehand, afterwards writes the empty log commit „Werk gelöscht:
  <Titel>“ (`allow_empty`, trailer `Baruch-Action: work-deleted`) and moves the folder to
  `<data>/works/.geloescht/<slug>-<JJJJ-MM-TT>/` (collision `-2`, `-3`; `.geloescht` and target no
  symbolic links). `.geloescht` is not a valid slug, so `sync_all` never touches the archive.
  Afterwards `AppState::default_work` (= `Store::default_work`, smallest id) is the next work; without works
  `/` redirects to `/werke`. If all works are deleted, a restart does not seed again either (see "Work ids and seed").
  `delete_work` enters id, slug and title in `work_retired`. `move_to_trash` (server) moves only an
  existing folder (otherwise `Ok(None)`), checks the slug like `mirror_dir` and refuses `works/`, the mirror and
  `.geloescht` as a symbolic link.

### Work ids and seed (migration 0007)

Migration `0007_work_ids.sql` (contract Round 5):

- **Never reuse ids:** `work` is rebuilt with `id INTEGER PRIMARY KEY AUTOINCREMENT`, with all
  columns from 0001, 0002, 0005 and 0006 in the same order, the same CHECKs and defaults; the indexes
  `work_source_sha256` and `work_title_key` are created anew. SQLite remembers the largest id ever given
  in `sqlite_sequence`; after deleting work 2 the next work gets 3. (Ids that were already deleted before 0007
  and were never the largest are no longer known to the database.)
- **Way:** sqlx-sqlite 0.8.6 runs every migration in its own transaction, also with
  `-- no-transaction` (the field `no_tx` is not read there; evidence: `sqlx-sqlite-0.8.6/src/migrate.rs`,
  `apply`, and the test `sqlx_runs_even_a_no_transaction_migration_inside_a_transaction`). `PRAGMA
  foreign_keys=OFF` would have no effect inside it, an own `BEGIN IMMEDIATE` fails. That is why the rebuild runs in
  the transaction of sqlx with `PRAGMA defer_foreign_keys=ON`: copy rows to `work_new`, delete `work`
  (the references from `node` and `work_template_revision` count as open violations), create `work` anew
  and fill it (the violations are counted off again), delete `work_new`. The new table is named `work`
  right away, a `RENAME` is not needed. Before the end a guard (`fk_guard` with `CHECK (n = 0)` over
  `pragma_foreign_key_check`) checks that no foreign key is violated; otherwise the migration fails, and nothing
  is taken over, not even an entry in `_sqlx_migrations`.
- **`store_meta(key, value)`:** notepad of the store. 0007 sets `seeded = 1` for a database that already had
  works or personas; otherwise the seed (`seed_if_empty`) sets it in the same transaction as the
  sample work. Without `seeded` it seeds, with it never again: after deleting all works (also after a
  restart) the database stays empty, and `/` redirects to `/werke`.
- **`work_retired(id, slug, title, deleted)`:** deleted works. `free_slug` (import, creating, seed) also avoids
  these slugs: a new work with the title of a deleted one is named `<slug>-2`. The slugs stay reserved permanently,
  because the database does not know the archive folder under `works/.geloescht/` (PM decision Round 5).
- Rehearsed in `baruch-store/tests/migration_0007.rs` on a rebuilt database in the state before Round 5
  (store 0001–0006, library 0101–0105, sample work and an invented book with 87 outline items).

## Revision tree

Migration `0004_revision_tree.sql` (replaces the rules „nur anhängen“ from 0001 and the status rule from 0003):

- **Parent:** `revision.based_on` is NULL (root) or the number of a smaller revision of the same
  section (triggers `revision_parent_insert`/`_update`, „Elternteil der Revision fehlt“). On the switch-over
  rule R fills in: for `number > 1` without a base the highest earlier revision with status `current`/`superseded`.
  New revisions always get a parent: saving the shown or current one, proposals the current one.
- **Marker:** exactly one `current` per section (partial index `revision_one_current`; at least one is checked by the
  store). Status changes (trigger `revision_status_transition`): → `current` from `proposed`, `rejected`,
  `superseded`; `current` → `superseded`; `proposed` → `rejected`; otherwise „Unzulässiger Statuswechsel der
  Revision“. Switching changes no text.
- **Origin** (`node_id`, `number`, `author_kind`, `author_name`, `model`, `created`) is immutable
  (`revision_origin_immutable`); text and note may be changed by the human, `edited` records the time.
- **Deleting:** never the current revision (`revision_no_delete_current`); `revision_reparent` hangs the children on
  the parent of the deleted revision.
- **Numbers:** `node.last_rev` is the highest number ever given (`revision_number_bump`); a number
  `<= last_rev` is refused by `revision_number_fresh` („Revisionsnummern werden nicht wiederverwendet“). Numbers
  are thus unique and ascending, with gaps after deleting; `load.rs` checks exactly that, `write.rs` takes
  `last_rev + 1`. Core, RPC, view and forms address revisions only by number.
- **Full-text index:** `baruch-library` 0104 replaces the entry when the text changes and removes it on deletion.
- **Unknown number:** `StoreError::UnknownRevision { node, rev, existing }` names all existing numbers
  („Fassung 3 gibt es nicht; vorhanden: 1, 2, 4“), not a range that is wrong after deleting.
- **Concurrency:** every change is a `BEGIN IMMEDIATE` transaction, in the server additionally under
  `AppState::write_lock`; parallel switch, change and delete calls result in exactly one `current`, no
  lost change and exactly one commit per success (tests `baruch-store/tests/concurrency.rs`,
  `baruch-server/tests/concurrency.rs`).
- **References** in posts (`post_ref`) stay untouched; a reference to a deleted revision shows the
  current one.

Store API (`src/tree.rs`): `switch_to(work, node, rev, only_proposal, human) -> Switch` switches in one
transaction (with `only_proposal` only proposals, otherwise every non-current revision; already current →
`Refused`; proposal → `Accepted`, otherwise `Switched`) and lets the outline follow the heading line of the
revision (see "Outline", "Switching brings the level along"). `Switch { change, outline }` is re-exported from
`lib.rs`; `set_current` and `accept` are wrappers (`switch_to(.., false|true, "Autor")`, only
`change`). In addition `save_human_on` (base selectable; `save_human` = base current revision), `update_revision` and
`delete_revision` (human only; `Change.before` = state before, `Change.reparented` = re-hung children),
`revision_tree` (all sections by seq, revisions by number, without text).

Log in the mirror (`mirror.rs`), subject `„<Nummer> <Titel>: …“`:

| Change | Subject | Author / committer | Body | `allow_empty` |
|---|---|---|---|---|
| Accepting | `Vorschlag n übernommen` | persona or human / human | – | no |
| Saving | `Revision n gespeichert` | human / human | – | no |
| Switching | `auf Fassung n umgeschaltet` (with another level: `… · Ebene a → b`) | author of the revision / human | with another level: items re-levelled along with their new revisions, renumbering | yes |
| Changing | `Fassung n geändert` | human / human | line diff, note before/after | yes |
| Deleting | `Fassung n gelöscht` | human / human | whole text, author, status, parent, note, „Umgehängt: …“ | yes |

Trailers: `Persona`/`Model` only with an assistant as author, always `Baruch-Node` and `Baruch-Revision`,
with switching, changing and deleting in addition `Baruch-Action: switched|updated|deleted`. Switching commits even
with the same text, so that every switch results in exactly one commit. The files stay the snapshot
of `Store::mirror_snapshot`: changing a non-current revision and deleting a revision are empty commits.

Switching and accepting are mirrored by `mirror::record_switch` (exactly one commit, even when sub-items
are re-levelled along); if the revision brings another level along, the commit always carries `Baruch-Action: switched`,
also when accepting a proposal.

## Outline

Migration `0006_outline.sql` (baruch-store):

- `node.numbered` (0/1, default 1): does the node count in the numbering? On the switch-over 0 for nodes with
  an empty number (foreword of the sample work); the trigger `node_numbered_default` sets 0 for every newly
  inserted node with an empty number (seed). The store creates new nodes with a provisional number.
- `work.numbering`: own pattern in the pattern language (`baruch_typst::scheme`), `''` = no numbers,
  `NULL` = from the work template – read only with the scheme `custom`; with other schemes it stays in place
  (switching back restores it).
- `work.numbering_scheme` (migration 0008): `template` (default) | `decimal` | `decimal_dot` |
  `alphanumeric` | `chapter_roman` | `chapter_arabic` | `parts_chapters` | `chapter_words` | `legal` |
  `sermon` | `book` (since 0040) | `none` | `custom` (CHECK). On the switch-over: `numbering` NULL → `template`, `''` → `none`, otherwise
  `custom`; no other row or column changes (test `tests/migration_0008.rs` on a
  rebuilt database in the state 0001–0007 + 0101–0105).
- **Migration 0040 (scheme „Buch“, `0040_numbering_scheme_book.sql`):** the CHECK admits `book`. SQLite changes
  no CHECK; `work` is rebuilt by the official procedure: `work_new` with all columns from 0001,
  0002, 0005–0008 (same order, AUTOINCREMENT, CHECK on `kind`), copy rows, `DROP TABLE work`,
  `ALTER TABLE work_new RENAME TO work`, the indexes `work_source_sha256` and `work_title_key` anew (UNIQUE(slug)
  comes with the table); `sqlite_sequence` keeps the largest value ever given (also of a deleted work).
  Otherwise no triggers, views or FTS references hang on `work` (evidence in the test). Because many tables
  refer to `work(id)` with ON DELETE CASCADE or SET NULL (`outline_request`, `asset_request`, `work_book`,
  `work_rag`, `work_rag_collection`, `work_bible`, `sync_work`, `work_sword_source`, `chat_thread`, `work_timing`;
  `idea` SET NULL), `DROP TABLE` must run without foreign keys – with them the implicit DELETE would delete these
  rows, and `defer_foreign_keys` (the way of 0007) does not prevent actions. sqlx 0.8.6 runs every migration in
  a transaction in which `PRAGMA foreign_keys` has no effect; that is why `MIGRATOR` runs through
  `baruch_store::migrate`: for every version from `FOREIGN_KEYS_OFF` (so far only 40) first all older migrations
  normally, then this one on its own connection with `PRAGMA foreign_keys = OFF` (before the transaction),
  afterwards `ON` again (otherwise the connection is closed), finally the rest. The migration checks itself that
  the foreign keys are off (otherwise abort without change), and at the end `pragma_foreign_key_check` (a violation
  rolls everything back). Test `tests/migration_0040.rs`: database in the state 0039 + 0101–0106 with rows in every
  table that refers to `work`; every row, every other schema object and every reference (including its action) stays,
  `foreign_key_check` empty, `integrity_check` ok, all schemes including `book` storable, unknown ones refused.
- **Escapes:** `node.title` is unescaped (nested `replace()`: first `\\` → `char(1)`, then
  `\X` → `X` for `' " # * _ $ @ < > [ ] ` ~ = - / + .`, then `char(1)` → `\`), exactly like
  `baruch_typst::unescape_title`. Revision texts stay unchanged: the heading line in the text stays
  literal, `Node.title`, display, slug and commit subjects use the unescaped title. New
  heading lines are escaped by `baruch_typst::escape_title` (backslash and `# * _ $ @ < > [ ] ` ~`, `/` before `/`
  or `*`); `heading_line` keeps label and comment of the previous line. `Heading` from `parse` carries both
  titles (`title` unescaped, `raw_title` literal).

**Levels and validity** (`baruch-store/src/outline.rs`): part 0 (top level only, since Round 14 with the
part marker as its heading line, can be created in every work, can never be re-levelled), chapter 1, section 2,
subsection 3 (section under section). The outline is the sequence of levels in tree order, the
parent, as on import, the nearest predecessor with a smaller level. Valid: every level at most one deeper
than the previous one (so the first non-part is a chapter), none deeper than 3, never into its own subtree.
Moving keeps the level (Round 5): `before`/`after` only next to a target of the same level (`after` behind
its sub-items), `into` only under a target exactly one level higher (last sub-item); otherwise `Refused`
„Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).“ (`outline::MOVE_KEEPS_LEVEL`,
the same verdict in `judge()` of `outline.ts`). Sub-items move along unchanged.
Invalid → `StoreError::Refused` (-32000), nothing changed.

**Parts (Round 14, R14b):** no schema change – a part stays `node.kind = 'part'`, level 0, `parent_seq`
NULL; its chapters are, as always, the following items up to the next part (`parents`). The text of a new
part (revision 1) is `heading_line(0, Titel, None) + "\n"` = part marker
(`baruch_typst::part_line`, interface A in `baruch-typst/src/marker.rs`) and an empty line, author as with
every creation the human, the persona or `@mcp:<client>`. `add_many` allows level 0 before/behind a part
or chapter (also as the first item next to a chapter and in the middle of the block; not `into`, not next to a
section); the restriction „nur ohne Werkvorlage“ is gone, `add` (RPC without `level`, ideas) runs through
`add_many`. `OutlineAdded.adopted` = keys of the existing chapters whose parent is afterwards a new
part; their texts and numbers stay (`load::numbers` counts chapters across parts). Renaming
(`apply`, `rewrite_part`): marker at the beginning → only its line anew (`heading_line(0, …, Some(alt))`, the
comment stays), empty text → marker, otherwise (self-written text; old stock „= Teil …“ has been converted by the
start step since 2026-11-07, see "Parts in the Typst text") the text stays and
`Applied.kept_parts` lets the detail say so. Saving (`save_part`), `check_part`, proposals
(`check_proposal_heading`), `revision.save`/`update` (`check_current_heading`) and switching
(`follow_switched`) only reconcile parts **with** a marker (`part_problem`: exactly one marker at the beginning,
no further heading; saving and proposal also demand it when the current revision carries one);
`sync_heading` takes the title from the marker (`heading_level` delivers level 0 for parts). A
part marker as the first heading of a chapter (saving, switching) → refused (`TO_PART`).
Level changes without a new title through `baruch_typst::relevel_line` (a marker „ohne Nummer“ stays a
marker); when splitting, a piece with the marker „ohne Nummer“ counts as without a number (`Analysis.numbered`).

**Reassigning seq:** every change (`outline_move`, `outline_set_level`, `outline_rename`, `outline_add`,
`outline_delete`, `outline_save` with splitting/merging) runs in one transaction: read the outline in
tree order (depth-first, siblings by `position`), rearrange and check it as a list, then
write `seq`, `parent_seq`, `position`, `kind` anew – in two stages (`seq = -seq - 1`, then the new values)
because of `UNIQUE(work_id, seq)`. `post_ref.node_seq` is rewritten in two stages as well, references to
deleted nodes are dropped. `node.number` is written along. Afterwards every node with text whose
level or title changed gets a new revision of the human (current, parent = previous current one),
which differs only in the heading line. `seq` stays 0-based, without gaps and equal to the position
in tree order; `load.rs` keeps checking that.

**Fixed key:** because `seq` changes with every change, `baruch_core::Node` additionally carries `key`
(= `node.id`, no migration needed). `Store::node_seq(work, key)` delivers the current position (deleted →
`StoreError::GoneNode`). In the server `rpc::resolve_keys` translates `key` → `node` and `target_key` → `target`
under `AppState::write_lock` (RPC, `/act`, `/gliederung/act`); the key takes precedence over the position. All
forms (`tree.html`, `outline*.html`, `editor.html`, `assistants.html`) and `outline.ts` send it along.
Since Round 5 it is **mandatory** when writing through MCP (`propose_revision`, `add_post` including `refs`) and when citing
(`highlight.cite`, `/literatur/act` with `cite`): `node` is refused there (MCP and RPC -32602),
likewise without `key` (form: 422 „Abschnitt fehlt.“). The resolution runs under the write lock (MCP in
`mcp/tools.rs`, citing in `literature::cite`). Links `/?node=` stay pure display.

**Deleting whole nodes:** in the same transaction first the current revision `current → superseded` (allowed
according to 0004), then delete revisions, references of the posts, posts and the node. So the trigger
`revision_no_delete_current` stays effective for living sections. Merging appends the text to the previous
item (new revision there), moves the posts there (positions behind the existing ones) and deletes the
node the same way.

**Numbering** (`load.rs`, computed when loading, `load::work_numbering`): scheme `work.numbering_scheme`
→ `baruch_typst::scheme::Numbering` (`Numbering::of_scheme`; `custom` reads `work.numbering`, `template`
`baruch_typst::heading_numbering(template)`, otherwise „1.1“). `heading_numbering` reads only top-level
rules (scanner of the outline, `#` with an empty bracket stack), the **last** one with `numbering`, and returns the
pattern language: string literally, `none` → `""`, a function (directly or through the last
`#let` name before it) is evaluated symbolically per level 1–6 (`scheme::read_function`: depth conditions
`X.pos().len() OP k`, `let p = n.pos()`, text, `numbering("P", ..n)`, `numbering("P", p.at(i))`,
`* 2`, `+`); not evaluable → as before the first `numbering("MUSTER"` of the function. `load::numbers`
counts parts separately (`Numbering::part`), chapters continuously across parts, sub-counters per parent,
and passes the counter states `[kapitel, abschnitt, …]` to `Numbering::heading` (plus the number of numbered
chapters for „Schluss“ of the sermon); nodes with `numbered = 0` do not count and have no number.
`Store::set_numbering_scheme` (one transaction, `OutlineAction::Numbering`, unchanged → `Refused`,
unknown scheme/invalid pattern → `StoreError::Numbering`) rewrites `node.number`;
`Store::set_numbering` stays (None → `template`, `""` → `none`, otherwise `custom`). `Store::numbering_preview`
and `Store::numbering_rule` only read. `NumberingInfo` additionally carries `scheme`, `scheme_label`, `custom`.

Sample work: „I, 1, 2, 2.1, 2.2, II, 3, 3.1, 4, 4.1“, foreword without a number; `baruch_typst::format_number` applies a pattern according to the Typst rules (counting symbols `1 a A i I *`, prefix, separator, suffix, the last counting symbol repeats for deeper levels).

**Export and mirror with an outline scheme** (`snapshot.rs`): the template for `export_typst` and `main.typ`
comes from `baruch_typst::scheme::export_template(template, scheme, numbering)`: with `template` unchanged
(byte-identical as before Round 6), otherwise template + (line end, if needed) + `// Baruch: Gliederungsschema
„<Name>“` + `Numbering::heading_rule()` (one line `#set heading(numbering: …)`). Without a template the
rule stands in `main.typ` behind `// Baruch: <Titel>`. Since Round 14 the export no longer sets off nodes with `numbered = 0`
through show rules, but projects their first heading line onto the marker
`<baruch-unnumbered>` (see "Chapters without a number in Typst" further below, `snapshot::project`, also with „aus der
Werkvorlage“); only where that is not possible (own label, title with code like `#emph[…]`) does
`// Baruch: Überschriften ohne Nummer` with one line each
`#show heading.where(level: L, body: [Titel]): set heading(numbering: none)` stay as a fallback
(`snapshot::fallback_rules`). The texts in SQLite stay unchanged. `vorlage.typ` stays the stored
template. For the pattern-based schemes (`decimal`, `decimal_dot`, `chapter_roman`, `chapter_arabic`, `alphanumeric`,
`custom`) `heading_numbering` reads the same numbers back from the export (re-import gives the same
outline); the rules of the schemes with words are Typst functions (`"§ " + str(p.at(0))`,
ordinal-word array, `context` + `counter(heading).final()` for „Schluss“), which Baruch reads only
approximately on re-import. Since Round 14 the generated rules are checked by Typst itself
(`baruch-render/tests/schemes.rs`, see "Check against Typst" below).

**Headings in the text:** `outline_check` (read only) and `outline_save` split the text with
`baruch_typst::parse`: one heading → save and align title/level (sub-items move along),
more → with confirmation new items behind it (revision 1 of the human with its piece), none → with
confirmation append to the previous item; text before the first heading → `Refused`. Parts are saved without
outline logic. `outline::check_current_heading` runs after `Store::save_human`, `Store::save_human_on` and
`Store::update_revision` of the current revision in the same transaction: if the text does not carry exactly one
heading of the item's level at the beginning → `StoreError::Heading` (-32000, rolled back); otherwise
`outline::sync_heading` takes over the title. `Store::add_proposal` checks the same beforehand (`check_proposal_heading`), so
also MCP and `highlight.cite`. Level, splitting and merging do not exist on these paths.

**Switching brings the level along** (Round 5, user decision 2026-11-05 „Nein“: a revision is never rewritten
when switching): `Store::switch_to` calls `outline::follow_switched` instead of `check_current_heading`.
Same level → only `sync_heading` (title), `outline: None`. Different level → the item
gets level and title of the revision (`text_done`, its text stays), all sub-items move by the same
difference (if one would come deeper than 3, `StoreError::Heading` instead of capping – capping would make a child a
sibling, and switching back would not restore it), then `apply` as with `outline_set_level`: new revisions
of the human only for the sub-items actually re-levelled, only the heading line changed;
`OutlineChange { action: Level, detail: "„T“ Ebene a → b" or "„Alt“ → „Neu“ umbenannt, Ebene a → b" }`.
Refused (`StoreError::Heading`, -32000, everything rolled back, also the switching): no heading, further
headings, text before it (hint to „Speichern“ in the editor), a sequence of levels invalid at that position.
Parts have no heading line and switch without outline reconciliation. Splitting and merging exist
only through `outline_*`; outline and heading lines stay the same, export and import give the same
outline.

**Title rules (Round 7):** `baruch_store::check_title` (in `src/outline.rs`, with `MAX_TITLE_CHARS = 200`):
edge white space (`char::is_whitespace`) removed; empty, a character with `baruch_typst::is_newline` or more than 200
characters (`chars`) → `StoreError::Title(<Meldung>)` → RPC -32602, MCP `isError`, form 422. Applied in
`rename`, `add`, `save` (first and every split title, before each write), `check` (`valid: false`,
`message`), `check_current_heading` (`revision.save`, `revision.update` of the current one), `check_proposal_heading`
(`add_proposal`: MCP `propose_revision`, `highlight.cite`) and `follow_switched` (switching/accepting), always
in the transaction of the caller (refusal = rollback). `clean_title` (line break → space) no longer
exists. Import and export check no titles (byte-identical); existing empty titles stay readable.

**Typeset view and titles:** `baruch_wysiwyg::emit`/`serialize` end a heading at the first top-level line break
(`emit::split_heading`); the rest becomes a paragraph behind it. So only real
heading lines deliver titles. Cause of the bug „Absatz wird Titel“ (Round 7): `emit` turned `Inline::Soft`
in headings into a space, i.e. a single line `== Titel Absatztext …`, which `outline.save` took over as the title
(test `crates/baruch-server/tests/round7_titles.rs`). The reconciliation in the store (`sync_heading`,
`follow_switched`, `rewrite_heading`) already took only real heading lines.

**Scheme name in the head:** `NumberingInfo.label` = name of the scheme whose pattern matches the effective one
(`baruch_typst::scheme::Scheme::matching(pattern)`, only schemes with a fixed pattern language, never template/own/
none), otherwise the pattern, without numbers „keine“, with schemes with words their name. `fragments/tree.html` shows
„Nummern: {{ numbering.label }}“, the pattern only in the `title`.

**Server** (`baruch-server/src/outline.rs`): RPC `outline.*`, pages `/gliederung` and `/gliederung/bearbeiten`
with their own template environment (`outline.html`, `outline_edit.html`, `outline_confirm.html`), forms to
`POST /gliederung/act` (behind `guard::same_origin`). Writing methods under the write lock, afterwards
`mirror::record_outline`: exactly one commit „Gliederung: …“ (`allow_empty`), author and committer the human,
trailers `Baruch-Action: outline-*` and `Baruch-Node`, body with new revisions, renumbering and the full
text of deleted items. Left column (`fragments/tree.html`), the only outline of the user interface: head with numbering form
(`form[data-outline-numbering]` to `/gliederung/act`, field `numbering` = `vorlage` | `keine` | pattern); per
entry forms ↑ ↓ ← → (buttons in the row through the attribute `form`), button „⋯“ (`js-only`) and the
fallback links „bearbeiten“/„Bearbeiten“ (`nojs-only`, hidden by CSS with JavaScript: `.js .nojs-only`);
a hidden context menu `[data-outline-menu]`. Entries (`a[data-nav]`) always point to the editor, never
to `view=preview`. Widget `outline` (`web/src/outline.ts`): drag and drop, Alt+Shift+arrows (except in
input fields), context menu (right click, Shift+F10, context menu key) with renaming in place
(double click on the chosen entry or F2; Enter → `outline.rename`, Esc cancels), creating with
an input field (`outline.add`), re-levelling, deleting (`confirm()` → `outline.delete {confirm: true}`, Del). Every
writing action: `leaveEditor()` → RPC → `baruch:navigate {url, asked: true}`. Before saving in the editor
it fires `baruch:before-save {wait}` at the form, waits for `detail.wait`, then reads the form and asks
`outline.check` with `confirm()`. `view.rs` puts `node.key` and the numbering (`rpc::LibraryView::numbering`)
into the context, plus the number of open requests (`LibraryView::outline_requests`, hint in the head of the tree).

### Numbering per level (Round 14)

**One source:** `baruch_typst::scheme::Numbering` computes the numbers for tree, outline, editor head and
preview (`Numbering::heading`/`part` through `store/load.rs::numbers`), writes the rule for export and typesetting
(`heading_rule` through `export_template`) and reads it again on re-import (`Numbering::from_template`;
`of_scheme(Scheme::Template, _, t)` = `from_template(t)` or „1.1“). Like
`heading_numbering`, `from_template` looks for the last top-level `#set heading(numbering: …)` (private helpers from `lib.rs`:
`blank_comments`, `top_level_hashes`, `Cursor::arguments_at`, `expression_end`) and distinguishes:

- string → literal Typst pattern (`Pattern::literal`: every counting symbol separately like `NumberingPattern` in
  Typst 0.15, no doubled letter, `;`/`|` text); `none` → off;
- expression equal to `rule_value()` of „Teile und Kapitel“, „Kapitel ausgeschrieben“, „Juristisch“, „Predigt“ or „Buch“
  → exactly this scheme (with its part counting; „Buch“ sets the same chapter numbers as „Predigt“, but
  deliberately has a different rule – `Kind::Book`, `p.first()` instead of `p.at(0)` – so that it comes back
  together with its word parts);
- evaluable function → `Kind::Function { node, approx }`: the reader (`Reader`) builds a tree from
  `Node::If` (depth condition with `==, !=, <, <=, >, >=`) and `Node::Value` (concatenation of `Term::Text`,
  `Term::Hier` = `numbering(P, ..p)`, `Term::Num` = `numbering(P, p.at(i))` or `first()`/`last()` times k,
  `Term::Str` = `str(p.at(i))`; `k` at most 16, also as a product – otherwise the function counts as unreadable,
because the evaluation really repeats, and a huge number from a foreign template would bring the process down
on import and on every load); evaluation runs per path with `format_number` (= Typst). Evaluable means:
  read completely, every pattern with counting symbols, on every level 1–6 every position present (otherwise Typst would
  abort). `approx` = the previous approximation `heading_numbering` – `spec()` delivers it, so that display and
  outline head (`store/outline.rs`) stay unchanged; `heading_rule()` writes the function back as a chain
  per level;
- otherwise the approximation as before (`lenient(heading_numbering)`).

`read_function`/`shown` for `heading_numbering` are unchanged in their result (new terms like `str()` and
`last()` count as unreadable there, as before).

**Chapters without a number in Typst** (`store/snapshot.rs::project`): if the effective numbering is not off,
the first heading line of every non-part node whose `node.numbered` does not match the text form
(`Heading.numbered`) is switched with `baruch_typst::set_numbered_line` – in `export_parts` (export,
export map, typesetting, PDF) and `files` (mirror). Only where that is not possible (own label; title with code, i.e.
an unescaped `#` outside inline raw like `#emph[…]` – a backslash before the brackets of the call would turn it
into raw text) does the show rule
`heading.where(level, body)` stay as a fallback behind the template (also with „aus der Werkvorlage“), unless a
numbered node carries the same heading. `counted_headings` takes the same projection: the
headings of level 1 with a numbered text form without a show rule count. The texts in SQLite stay unchanged;
a freshly imported work (import sets `numbered` from the text form, R14a) stays byte-identical. The trigger
`node_numbered_default` from migration 0006 (empty number on insert → `numbered = 0`, for seeding the
foreword) no longer applies on import: `import.rs` afterwards sets `numbered` explicitly from the text form, otherwise
levels without a displayed number (sermon: sections „“) would come back as „ohne Nummer“ and the export would
project them (integration Round 14, test `round14_numbering`, `round14`). Works „aus der Werkvorlage“
without a numbering rule are projected too (the effective numbering is then the default „1.1“): in typesetting
this changes nothing, but export → import thus keeps „ohne Nummer“ (decision of the integrator; that is why the
sample work exports its foreword as a marker).

**Check against Typst:** `baruch-render/tests/schemes.rs` typesets all thirteen schemes (plus own patterns with
list, depth and doubled letter) with part markers and markers „ohne Nummer“; after every
numbered heading stands `#baruch-check("…")` with the number from `Numbering::heading`, which compares in context
`numbering(heading.numbering, ..counter(heading).get())` (`assert.eq`; if the function delivers content
as with „Predigt“ and „Buch“ with `context`, it compares the typeset width). Limits: Baruch knows the counting symbols
`1 a A i I *`; other Typst counting symbols (e.g. `α`, `一`) are text for Baruch.

### Outline through MCP and requests (migration 0012)

Request 2026-11-06: assistants may create, everything else only request.

- **Creating:** `Store::outline_add_many(work, target?, place, &[NewPoint], &Author)` – one block in one
  transaction through the same `apply` as all outline changes; for this `apply` takes the author
  (`baruch_core::Author`) of revision 1 of new items instead of the human's name. Existing revisions
  are rewritten only by a human author (otherwise an internal error; creating rewrites none). Result
  `OutlineAdded {change, points: [AddedPoint {node, key, level, title}], adopted: [key]}` (Round 14: also parts,
  `adopted` see "Parts" above). MCP `outline_add`
  (`mcp/outline.rs`) mirrors with `mirror::record_outline_by(…, OutlineBy::Direct(origin))`: author the persona
  or the client, committer the human, trailers `Baruch-Origin`, `Persona`.
- **Requests:** table `outline_request` (`0012_outline_request.sql`: `work_id` with `ON DELETE CASCADE`, `op`,
  `params` JSON `{op, expected}`, origin, `note`, `created`, `decided`, `decision` confirmed/rejected/stale,
  `decided_by`, `message`). `Store::outline_request` resolves the keys, checks in a savepoint
  (`sqlx::Connection::begin` on the transaction = SAVEPOINT, rolled back afterwards) whether the change would work
  now, and stores request and state (`Expected`). `Store::outline_request_decide` first checks the state
  (`staleness`), then runs the outline function in a savepoint; a refusal on the merits becomes
  `stale` with a message, otherwise `confirmed`. The server mirrors with `OutlineBy::Confirmed(request)` (human as
  author/confirmer, trailers `Baruch-Origin`, `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`);
  rejecting and `stale` write no commit (except for the request `add`, see below).
- **Preview (Round 14):** `Store::outline_request_preview(work, id)` opens a transaction
  (`BEGIN IMMEDIATE`), checks „offen“ (otherwise `Refused`) and `staleness` (otherwise `Heading("Veraltet: …")` –
  `Refused` carries only fixed texts, a new error kind would have needed `rpc.rs`), reads `outline::point_states`
  (key, level, number, title, numbered), runs `run` in a savepoint, reads again and rolls both
  back. `preview_points` compares by key: `Moved` for the moved item including its sub-items,
  otherwise `Renamed`/`Level`/`Numbered`/`Same`, deleted items as `Deleted` behind their old predecessor. The
  server (`outline::add_preview`) puts `preview` or `preview_error` into the request form; RPC
  `outline.requests`, MCP `outline_requests` (each `preview: true`) and the page `/gliederung` (`<details>` per
  open request) use it.
- **Consent to a new part (user question 12, migration 0035):** MCP `outline_add` calls
  `Store::outline_add_by_assistant(work, target?, place, points, origin, model, note)`: `add_many` in a
  savepoint; if `adopted` is empty, it stands (`AssistantAdd::Added`), otherwise roll back and request
  `OutlineOp::Add {key, place, points, model}` with `Expected.adopted` (`AssistantAdd::Requested`). `run` executes it
  on consent with the requester as the author of revision 1; if `add_many` delivers other `adopted` than at the time of the
  request, it becomes `stale` (preview: `preview_error`). Preview marks `Added`/`Adopted`. 0035 creates
  `outline_request` anew with `op … 'add'` (SQLite does not change a CHECK), takes over all rows including id and the
  AUTOINCREMENT count; test `migration_0035.rs`. Rejecting or `stale` of a request `add` is mirrored by
  `mirror::record_request_decision` (empty commit, `Baruch-Action: outline-request`, `Baruch-Decision`).
  User interface: `base.html` carries the mark `<!--baruch-consent-->`; the layer `consent::inject` (over all
  routes) reads HTML answers, queries `Store::outline_requests_needing_consent` only when the mark is present and
  inserts `templates/consent.html` (notice bar with link, `<dialog data-widget="consent">` with preview and
  forms to `/gliederung/act`); so none of the many template environments needs its own data. The widget
  `web/src/consent.ts` opens the dialog modally and decides through RPC `outline.request_decide`.
- **Without a number:** `Store::outline_set_numbered` sets `node.numbered` and rewrites all numbers
  (`OutlineAction::Numbered`, trailer `outline-numbered`); RPC `outline.set_numbered`, button on `/gliederung`,
  through MCP as the request `set_numbered`. It refuses a part without a number (`Refused`, also in the trial run of a
  request): the part marker carries no number flag, `parse` always delivers parts numbered, so export →
  import would count it again and shift the following part numbers. Going back to „mit Nummer“ works.

## Images (migration 0009)

- **Tables:** `asset` (name unique per work, `last_rev`), `asset_path` (path from template/text →
  image, key `(work_id, path)`), `asset_revision` (revisions). The content is always **TEXT**
  (`CHECK (typeof(content) = 'text')`): Base64 (standard alphabet, with `=`, without line breaks,
  `encoding = 'base64'`) or SVG text (`'svg'`, only with `image/svg+xml`). `bytes` and `sha256` apply to
  the decoded bytes. Limit `baruch_store::MAX_ASSET_BYTES` = 8 MiB per revision.
- **Revisions like sections:** exactly one `current` per image (partial index `asset_one_current`), switching
  `superseded ↔ current`; triggers keep origin (`asset_id`, `number`, `author_name`, `created`) and
  content (`mime`, `encoding`, `content`, `bytes`, `sha256`) immutable, forbid deleting the
  current revision, hang the children of a deleted one on its parent and never give a number
  twice (`asset.last_rev`). Changing only caption, alternative text and note (sets `edited`).
- **Type** from the first bytes (`baruch_store::detect_mime`): PNG, JPEG, GIF, WebP, SVG (UTF-8, `<svg`
  after an optional BOM, prolog, comments, DOCTYPE). Name: `^[a-z0-9]([a-z0-9._-]{0,78}[a-z0-9])?$` with
  an extension matching the type (`valid_asset_name`, `asset_name_from_file`).
- **Store** (`crates/baruch-store/src/assets.rs`): `asset_list`, `asset_revisions`, `asset_data`,
  `asset_by_path` (reading); `add_asset`, `replace_asset`, `set_current_asset`, `update_asset_revision`,
  `delete_asset_revision`, `delete_asset`, `set_asset_paths` (one transaction `BEGIN IMMEDIATE` each, result
  `AssetChange` for the mirror); `export_assets`; `import_typst_with_assets` (work and images in **one**
  transaction, the same file under two paths = one image). Errors `StoreError::UnknownAsset` and
  `StoreError::Asset` (too large, type, name, path, Base64) → `INVALID_PARAMS`; duplicate name and „unverändert“
  → `Refused`. Deleting a work removes all image rows beforehand.
- **Mirror:** `snapshot::files` appends `assets::mirror_files` (`bilder/<name>` or
  `bilder/<name>.b64`, lines of 76). `mirror::describe_asset`/`record_asset`: one commit per change,
  subject „Bild <name>: hinzugefügt / Fassung n ersetzt / auf Fassung n umgeschaltet / Fassung n geändert /
  Fassung n gelöscht / gelöscht“, trailers `Baruch-Asset`, `Baruch-Revision`, `Baruch-Action: asset-added|
  asset-replaced|asset-switched|asset-updated|asset-deleted|asset-removed`. Switching, changing and deleting
  also as an empty log commit; changing with before/after, deleting with metadata and the deleted
  content (Base64 in lines of 76, SVG literally).
- **Image paths in the text:** `baruch_typst::images::image_paths` (string literal as the first argument of every
  `image(` call: `#image(` in markup, `image(` after `(`, `,`, `:`, `=`, `{`, `;`; not in comments
  and raw; computed paths not).
- **Normal form of the paths (Round 7):** `baruch_store::normalize_asset_path(path) -> Option<String>` removes a
  leading `/` and `./` parts and merges empty parts; `..` or empty → `None`. `check_asset_path`
  thus allows `./` and `//` (only `..`, addresses, `\` and control characters stay forbidden), the path is stored
  as in the text. Comparison happens everywhere in normal form: `asset_by_path` (page `/werke/{id}/bilder/pfad?p=`),
  the check „Pfad gehört schon zu Bild …“ when creating, `export_assets` (thus `export --out`),
  `wysiwyg::image_alts`/`image_ref`, the typeset view diff of the proposals (`view.rs`) and the import from the
  command line (`read_asset`).
- **Typeset view:** `render_html_with` shows an island with an image as `div.ws-block.ws-figure` (`data-kind="island"`,
  `data-image="1"`, `<img loading="eager">`, `span.ws-caption`) or `span.ws-island.ws-inline-image`, without
  visible raw text (only `data-raw`); delivered only through the existing route (`nosniff`, CSP `sandbox`).
- **Import** (`crates/baruch-server/src/import.rs`, only `import_file_as`): reads the images in
  `spawn_blocking` relative to the canonical folder of the source file; `ImportReport.assets`/`.skipped`.
  `export_to_dir` writes `export --out` (only relative paths without `..`, overwrite nothing).
- **Server** (`crates/baruch-server/src/assets.rs`): RPC `asset.*`, pages and forms, own
  template environment (`base.html`, `assets.html`, `asset.html`), style `static/assets.css`,
  `UPLOAD_LIMIT` = 8 MiB + 64 KiB. Delivered images carry `X-Content-Type-Options: nosniff` and
  `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox`.
- **Type change and old stock (Round 21c, without migration):** `Store::replace_asset_renaming(…, rename)` (the old
  `replace_asset` = without `rename`) renames in the same transaction when the format differs (`UPDATE asset SET name`,
  default extension of the format, taken → `free_name`), open requests carry `asset_name` along; paths hang on the id
  and stay. `Store::asset_path_conflicts` groups `asset_path` by normal form and reports groups with more than
  one image (`PathConflict {normal, paths: [PathOwner {path, asset}]}`); `Store::resolve_asset_path` hangs all
  spellings onto the chosen image via `UPDATE asset_path SET asset_id` (one `AssetChange::Updated`). So that Typst can typeset the
  old path with a new extension, `baruch_render` adds `format: "<Format>"` when typesetting in every
  `image("<Pfad>"…)` whose file does not match the extension (detection by the first bytes, without a new line).

## Images, cover and PDF (Round 11, migrations 0017 and 0018)

- **Book data (0017, `baruch-store/src/book.rs`):** table `work_book` (one row per work, no row =
  defaults): `cover_asset_id`, `author_photo_asset_id` (triggers `work_book_same_work_insert/update`: only images
  of the same work; image deleted → `NULL`), `title_area` (`top|middle|bottom`), `front_matter` (front matter),
  `cover_pages` (front/back side in typesetting, default on), publisher … short biography as text, `paper_mm` (0, 1],
  `bleed_mm` [0, 10]; work deleted → row gone. Subtitle, author and Bible translation stay in `work`.
  `BookMeta`, `BookEdit`, `book_meta`, `set_book_meta` (limits: single-line 200 characters, blurb 4000,
  short biography 2000, dedication/rights notice/image credits 1000; year empty or four digits; ISBN-13 with
  check digit, compared by the digits → `Refused`), `work_cover`, `set_work_cover`.
- **Requests for images (0018, `baruch-store/src/asset_requests.rs`):** table `asset_request` after the pattern of
  `outline_request`: `op` ∈ `replace|set_cover|delete`, `asset_name` (with `set_cover` empty = remove cover),
  `params` (JSON: replace `{mime, encoding, content, caption?, alt?, expected_sha256}`, delete `{rev?,
  expected_sha256}`, set_cover `{expected_cover}`), origin only `persona`/`mcp`, index `(work_id, decided)`.
  `asset_request` already checks on the request (unknown image, content, image type, unchanged, same open
  request), `asset_requests`, `open_asset_requests`, `asset_request_decide` (once; `Decision` from
  `requests.rs`). Executing is done by the server.
- **Frame (`baruch-typst/src/cover.rs`, pure):** `frame_parts(info, opts) -> FrameParts {front, back,
  added_cover_pages}` – cover front (full bleed, `margin: 0`, title in the area top/middle/bottom), empty
  inside page with `counter(page).update(0)` (parity and page numbers as without a cover; `COVER_PAGES` = 3 with back side),
  front matter (half title, page 2, main title, imprint, dedication), back side (blurb, short biography/photo,
  ISBN with EAN-13); without `heading` and `outline`, without a visible page number. `wrap_source` sets the cover
  for the printer on one page (back | spine, text rotated | front, with bleed). User texts
  only as Typst string literals (`typst_string`, escapes `\ " \n`), never as markup. Virtual files under
  `.baruch/` (`VIRTUAL_DIR`; image, photo, barcode `.baruch/isbn.svg`), mark `FRAME_MARK`.
- **Server `cover.rs`:** RPC `work.set_cover`, `work.meta.get`, `work.meta.set`, `work.cover_pdf`; `frame(state,
  work, source, map)` inserts `front` before the first section (without sections at the end) and `back` at the end and
  shifts byte ranges and first lines of all sections (messages stay assigned to their section); without
  cover (pages) and front matter it returns byte-identically (same stand, same cache). `cover_name`,
  `cover_info` (`{name, caption, alt, url, thumb}`), `assets_panel` (block `id="umschlag"`), `cover_source`/
  `build_wrap` (page size from the first book page, spine = (pages − `COVER_PAGES` in typesetting) × `paper_mm`).
  Pages `/werke/{id}/umschlag`, `…/umschlag.svg` (CSP `sandbox`), `…/umschlag.pdf`; forms
  `/werke/umschlag/act`, `/werke/buchangaben/act`. Templates `cover.html`, `fragments/cover_panel.html`, style
  `static/cover.css`; Settings → Work renders the groups through `cover::settings_blocks`
  (`WorkRetry::Book`). Mirror: own commits under `write_lock` – „Buchangaben geändert: <Felder>“
  (`Baruch-Action: book-meta`), „Umschlag: <name> gesetzt“ (`cover-set`), „Umschlag entfernt“ (`cover-removed`),
  body before → after. The cover as `cover-image` in the EPUB has been in `epub.rs` since Round 20a; since Round 21c
  it is the rasterised front: `cover::front_cover_source` (format from `book_page_mm` of the work template, layout
  through `resolve_fonts`) → `baruch_typst::cover::front_source_with_rules` (one page in the final format without bleed:
  background as in the cover, blocks of the front or, without a layout, `front_body`) → `epub::front_cover_png`
  (`baruch_render::render_png`, at most 1600 × 2560, cache per stand, `spawn_blocking`).
- **Typesetting and PDF:** `typeset::prepare` = `export_map` + `export_assets` → `cover::frame` → stand. Typeset page,
  `work.pdf`, `/werke/{id}/pdf` and `export --pdf` use the same `prepare`, thus the same page count.
  `pdf.rs`: `PDF_TIMEOUT` = 120 s, `PdfKind {Book, Cover}`, `PdfOut`, `source_to_pdf` (at most one
  generation per work – a process-wide set of busy works, `claim` –, then the lock `typeset::RUNS` like the preview,
  `spawn_blocking`; typesetting error, „läuft schon“, time → `REFUSED`, never half bytes), `book_pdf`, `write_pdf_file`
  (CLI, `create_new`). The cover PDF (`cover::to_pdf`) goes through `source_to_pdf(work, PdfKind::Cover, …)`.
  The typeset page has `#druckpruefung` (missing images `typeset::missing_images`, `PrintWarning`s).
- **Requests (`asset_requests.rs`):** RPC `asset.request_list`/`asset.request_decide`, form
  `POST /werke/bilder/antraege/act`, `GET /werke/{id}/bilder/antraege/{nr}/datei`, `panel` (block
  `id="antraege"`, `fragments/asset_requests.html`). Confirming: replacing/deleting through the store under
  `write_lock` and `mirror::record_asset_by(…, AssetBy::Confirmed)`; cover through
  `rpc::call("work.set_cover")` (takes the lock itself) and the log commit „Bildantrag n bestätigt“
  (`asset-request-confirmed`). Outdated (`expected_sha256` or `expected_cover` does not match) → `stale` with a reason.
- **Mirror (`mirror.rs`):** `AssetBy {Human, Direct(origin), Confirmed(request)}`, `record_asset_by` (Direct:
  author persona or `@mcp:<client>`, trailers `Baruch-Origin`, `Persona`; Confirmed: author the confirmer,
  trailers `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`).
- **MCP (`mcp/assets.rs`):** `asset_add` (immediately), `asset_replace`, `asset_set_cover`, `asset_delete` (only
  requests), `asset_requests`. Since Round 23 `/mcp` has its own limit `MCP_BODY_LIMIT` (12 MiB; an image with 8 MiB fits as Base64), no longer `works::RPC_BODY_LIMIT` like `/rpc`.
- **Images page (`assets.rs`):** search `?q=` (`Store::asset_search`: name, caption, alternative text of the
  current revision, in Rust with `to_lowercase` + `contains`, `%`/`_` literally), thumbnails
  `…/vorschau?b=` through the crate `image` 0.25 (MIT OR Apache-2.0, only png/jpeg/gif/webp; `spawn_blocking`,
  limits 20 000 px per side and 512 MiB; ETag; SVG unchanged with CSP `sandbox`), column cover, blocks
  `cover::assets_panel` (top) and `asset_requests::panel` (bottom), `POST /werke/bilder/einfuegen` (new revision
  of the human with `#figure(image(…))`, missing path `bilder/<name>`, one commit).
- **Editor:** `view.rs` delivers `chapter_image`, `missing_images`, `image_choices`; `wysiwyg::image_ref_in`
  sets `ImageRef.missing` („Bild fehlt“ in the typeset view). The `ImageView` (images in short + computed images of the
  template) travels under the reserved key `wysiwyg::IMAGE_VIEW_KEY` (`"\u{1}baruch-image-view"`) in
  the table `LibraryView.image_alts`. `LibraryView.cover` (from `cover::cover_info`) shows the small
  cover image in the head of the outline (`fragments/tree.html`).
- **Personas:** `Store::save_persona(handle, name, edit, color, prompt)` writes everything of one tab in one
  transaction (the name is an argument of its own, not a field of `PersonaEdit`); `POST /einstellungen/personas/act`
  knows `action=save`, `action=reset_color` and `action=reset_prompt` (`confirm=1`); refused input through `personas::retry`
  (`task_local`). Widgets `web/src/persona_tabs.ts` (ARIA `tablist`/`tab`/`tabpanel`, `history.replaceState`)
  and `web/src/mention.ts` (completion „@…“, `combobox`/`listbox`).
- **Tests:** `baruch-render/tests/{pdf,print}.rs`, `baruch-typst/tests/{cover,images}.rs`,
  `baruch-store/tests/{book,asset_requests,migration_0017,migration_0018}.rs`, `baruch-server/tests/round11_{pdf,
  mcp_assets,cover,images,personas}.rs` and end-to-end `round11.rs`.

## Ideas (migration 0011)

- **Tables** (`crates/baruch-store/migrations/0011_ideas.sql`, only new tables):
  - `idea`: `work_id` (`REFERENCES work(id) ON DELETE SET NULL`, NULL = without a work), `node_id` (reference =
    `Node::key`, `ON DELETE SET NULL`), `note`, `quote`, `source`, `bible`, `origin_kind`
    (`human`/`persona`/`mcp`) and `origin_name` (name or `@handle` or `@mcp:<client>`), `status`
    (`open`/`done`), `done_as` (`outline`/`post`/`persona`/`note`), `done_ref`, `created`, `updated`, `deleted`
    (NULL = present, otherwise the time). `CHECK ((status = 'done') = (done_as IS NOT NULL))`.
    `AUTOINCREMENT`: ids are never reused.
  - `idea_word`: keywords per idea with `position`, `word` and `word_key` (lower case, white space reduced to one
    space; `UNIQUE (idea_id, word_key)` merges duplicates), `ON DELETE CASCADE`.
  - `idea_fts` (FTS5, `unicode61 remove_diacritics 2`, columns `words, note, quote, source, bible`, rowid =
    `idea.id`): maintained in Rust in the same transaction (`ideas::reindex`), deleted ones are not in it.
- **Deleting a work** (`Store::delete_work`, unchanged): the foreign keys set `work_id` and `node_id` to
  NULL – the ideas stay as ideas without a work. Likewise an idea only loses its reference when its section is deleted or
  merged. The mirror `<data>/einfaelle/` is brought up to date only by the next
  action on an idea without a work (backlog „Nach Runde 7 offen“).
- **Store** (`crates/baruch-store/src/ideas.rs`): `idea_create`, `idea_get` (also deleted ones), `idea_list`
  (`IdeaFilter`: `scope` All/NoWork/Work, `word`, `status`, `deleted` = only deleted ones, `limit` 100/at most
  500, `offset`; newest first), `idea_words` (cloud, without word filter, most frequent first), `idea_search`
  (finished FTS5 expression, snippet escaped with `<mark>`; broken expression → `StoreError::Idea`),
  `idea_update` (`IdeaEdit`, `None` = unchanged; new work without `key` → the reference is dropped; `status: Open`
  resets the processing, `Done` on an open one marks „von Hand erledigt“ = `done_as: note`),
  `idea_delete`/`idea_restore` (only `deleted`), `idea_mark_done`, `idea_mirror_file`. Every write is
  a transaction `BEGIN IMMEDIATE`. Errors: `StoreError::UnknownIdea(id)`, `StoreError::Idea(<Meldung>)`
  (limits, reference outside the work, deleted), `UnknownWork`. The reference (`key`) must belong to the work.
- **Mirror**: `snapshot::files` appends `ideas::mirror_files` (after the images): `einfaelle.md` only if
  the work has at least one undeleted idea – works without ideas mirror and export
  byte-identically as before. `Idea::entry_markdown` writes an entry:

  ```
  # Einfälle – <Werktitel>            or  # Einfälle ohne Werk

  ## Einfall <id> · offen             or  · verarbeitet (<done_as>)

  - Schlagwörter: Laube, Rosen
  - Gedanke: erste Zeile
    weitere Zeilen eingerückt
  - Markiert: …
  - Bezug: Abschnitt „<Titel>“ (key <k>)
  - Quelle: …
  - Bibelstelle: …
  - Herkunft: Mensch|Persona|MCP <name> · <created>
  ```

  Newest first, empty fields are missing, values stand literally (no conversion, no escaping).
- **Server**: `crates/baruch-server/src/ideas.rs` (RPC `idea.*`, pages `/einfaelle`, `/einfaelle/{id}`,
  `POST /einfaelle/act`, own template environment with `ideas.html`, `idea.html`), `src/mcp/ideas.rs`
  (MCP tools), `mirror::describe_idea`/`mirror::record_idea`, template `fragments/idea_capture.html`
  (dialog in `workspace.html`, in `view::environment`), style `static/ideas.css`, widget `web/src/ideas.ts`.
  The RPC methods are answered in `rpc::call` directly after `work.*` (before `work_param`), because they also
  work without a work.
- **Commits** (`mirror::record_idea`, under `state.write_lock`): exactly one per action and affected mirror
  (moving between works or from/to „ohne Werk“: one each in the old and in the new). Subject
  „Einfall <id>: angelegt|geändert|gelöscht|wiederhergestellt|verarbeitet als <Gliederungspunkt|Chat-Beitrag|
  Notiz>“, trailers `Baruch-Idea: <id>`, `Baruch-Action: idea-create|idea-update|idea-delete|idea-restore|
  idea-process`, `Baruch-Origin: human|persona|mcp`, with persona/MCP additionally `Persona: <handle>`. The author is
  whoever acts (persona „<Name> (<handle>)“, MCP client „MCP-Client (@mcp:<name>)“, each
  `<handle>@persona.baruch.invalid`), the committer always the human; `allow_empty`. Body: before/after of the
  changed fields, on deletion the whole entry. Without a work: own Git repository `<data>/einfaelle/`
  (`Mirror::open_or_init`, `<data>/einfaelle` must not be a symbolic link) with only
  `einfaelle.md` (missing if there are no ideas without a work). Processing into an outline item additionally
  results in the outline commit (`mirror::record_outline`, `Baruch-Action: outline-add`). Since Round 14
  item and „verarbeitet“ come about in **one** transaction (`Store::idea_to_outline`: `outline::add`, then
  `ideas::mark_done_in` with `done_ref` „key <key>“; idea from another work → `Idea`); if a
  step fails, nothing is changed, and the server writes no commit (test with an SQLite trigger on
  `UPDATE idea`).
- **Protection**: `POST /einfaelle/act` lies in `writes()` behind `guard::same_origin` (foreign pages → 403);
  the return address `back` takes only own addresses (`/…`, not `//…`).
- Task for a persona (Round 10, `ideas.rs` `process_persona`): model call through `crate::assist` before the
  write lock, afterwards under the lock `idea_mark_done(…, Persona, "key K, Beitrag ID")` and mirror commit
  `idea-process`. The lector knows the ideas as memory through `assist::context`.


## Menu and settings (Round 9)

### Header and menus (`menu.rs`)

Since Round 33 the header is the **ribbon** (section "Round 33 – Ribbon" further below); the menus „Werk ▾“,
„Ansicht ▾“ and „Werkzeuge ▾“ of Round 9 no longer exist. What remains from Round 9:

- The head lives in `templates/base.html` as `{% block topbar %}`; every template environment already embeds
  `base.html`, no module needs a new registration. Pages deliver `{% block page_title %}` (text in
  `.work-title`), `{% block page_actions %}` (groups of the page in the tab Start) and since Round 33 `band_tab`,
  `band_einfuegen`, `band_pruefen`, `band_buch`, `band_ansicht`.
- Data of the head in this order of precedence: (1) `topbar` (only the work page, `crate::menu::workspace_topbar`
  in `view::values`: work, `key`, `rev`, view, `lq_q`, work choice, badges), (2) `work.id`, if `work`
  is a mapping, (3) `settings.work` (cookie `baruch_work`). Addresses are built by the template itself (only
  digits from the data; minijinja escapes `/` in values). Generated links never with `node=`, always `key=`.
- On the work page „Neues Werk / Importieren…“, the toggle editor/preview, „Literatur“ and „Einfall
  festhalten“ carry `data-work-import-open`, `data-view-toggle`, `data-literature-open` and `data-idea-open`
  (at most once per page; `works.ts`, `literature.ts`, `ideas.ts` unchanged). On
  other pages the respective dialog is missing, the links there simply lead to their page. „Einstellungen“
  (`data-settings-open`, Alt+,) stands in the menu „Baruch ▾“ and is the first such link of the page.
- `base.html` ends after `{% block body %}` with the shell of the settings dialog
  `<dialog class="settings-dialog" id="settings" data-widget="settings">` (content loaded by `settings.ts`);
  it stands behind all dialogs of the pages.
- `src/menu.rs`: `handles`, `call` (`menu.badges`), `Badges`, `badges(state, work)`, `open_proposals(work)`,
  `workspace_topbar(work, shelf, at, library)`, `fill_badges`/`fill_page` (Round 23a).
- `web/src/menu.ts` (widget `menubar`): keyboard at the menu „Baruch ▾“, click beside it, `aria-expanded` at the
  `<summary>`, badges in the whole ribbon through `menu.badges` (on load outside the work page, on opening the
  menu, after forms and `baruch:navigate`). It takes the work from `data-work` at the head (`header.band`).
- The theme widget stays in `web/src/panes.ts` (no `theme.ts`): shows only one character, name in the `title`, and
  reads the cookie `baruch_theme` anew on `baruch:settings-changed`; the button stands in the tab View.
- Style: menu in `static/menu.css` (list `position: absolute`, keyboard shortcuts through `data-shortcut` as `::after`, so that
  the text of the entry stays the same), ribbon in `static/band.css`.
- Tests: `tests/round9_menu.rs` (all pages through the router: exactly one ribbon, one menu, earlier addresses,
  dialog shell, badges with a request through MCP `outline_rename` and proposals through `propose_revision`, scripts) and
  `tests/round33_menueband.rs`.

### Settings (`settings/`)

- `src/settings/mod.rs`: registration `registry()` (seven `SettingsPage {slug, title, storage, render}` in
  fixed order), contract type `PageRender = for<'a> fn(&'a AppState, Option<WorkId>, &'a BTreeMap<String,
  String>) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>>`; every page delivers only
  `<section class="settings-page" data-settings-page-body="<slug>">…`. `frame()` resolves the work (query
  `work` > cookie `baruch_work`, if the work exists > default work > none; a named unknown work →
  404 or -32602), draws `templates/settings/frame.html` (navigation with `data-settings-page`,
  `aria-current`, „Gespeichert: …“) and passes all other query pairs except `fragment` to the page.
  `?fragment=1` delivers only the frame (`Cache-Control: no-store`), otherwise `templates/settings.html`
  (extends `base.html`, blocks `page_title`/`page_actions`).
- Request data (cookies, `Host`, message of a refused form) are unknown to the contract signature; they
  lie in a `tokio::task_local` during drawing (`settings::request()`, defaults without a frame).
- Pages: `display.rs` (display, form `POST /einstellungen/darstellung/act`, cookies in the format of
  `widgets.ts` `store`), `work.rs` (work: renders `{% block content %}` from `work_edit.html` and
  `work_numbering.html` with `render_captured` + `State::render_block`, context from
  `works::edit_context` or `numbering::page_context` – the same functions build `/werke/{id}/bearbeiten`
  and `/werke/{id}/nummerierung`), `assistants.rs` (assistants, forms to
  `POST /einstellungen/personas/act` in `personas.rs`), `access.rs` (MCP access, `mcp::session::clients()`),
  `data.rs` (data, `backup()` under `write_lock`: `create_dir` as a lock against overwriting, `VACUUM INTO`,
  recursive copy without `baruch.sqlite`/`-wal`/`-shm`, links skipped and reported; time from SQLite
  `strftime(…, 'localtime')`; beforehand an upper limit `MAX_BACKUPS` = 10 backups `<Name>.sicherung-*` and free
  space through `rustix::fs::statvfs` (Unix): data folder size + `RESERVE_BYTES` = 512 MiB, otherwise refusal without
  creating anything). Refused work forms with `back` (`POST /werke/act`, `/werke/nummerierung/act`) draw through
  `settings::work_retry_page` the page „Werk“ with `RequestInfo::work_retry` (input + message), so that the
  popup can take over a frame. Connections and sources: `crate::connections::render_connections` or `render_sources` (below).
- Own template environment in `settings/mod.rs` (autoescape `.html`) with `base.html`, `settings.html`,
  `settings/*.html`, `work_edit.html`, `work_numbering.html`. `templates/personas.html` is gone.
- `back` (Round 9): `POST /werke/act` (update) and `POST /werke/nummerierung/act` take a hidden field
  `back`; only `/einstellungen/werk` with a query from `[A-Za-z0-9=&_%.-]` counts (`works::settings_back`),
  then 303 there with `&ok=werk` or `&ok=schema`; otherwise as before. Error pages keep `back`.
- `view::Settings.pv` (`open` | `closed`, cookie `baruch_pv`): with `closed` `from_cookie_header` appends the
  flag `view::FOLD_ALL` = `(0, 0)` to `folded` (key 0 does not exist); `proposals_of` thereby folds all
  proposals except the shown one – the same way as `baruch_folded`.
- Store (`baruch-store/src/personas.rs`): `PersonaRow` (all fields + colour), `PersonaEdit` (with `check()`:
  role/model trimmed 1–200 characters, context 1–10 000 000), `Store::persona_list`, `Store::update_persona`
  (`BEGIN IMMEDIATE`, unknown → `UnknownPersona`, limits → `Refused` as a second guard; the server layer
  checks beforehand and reports -32602). No migration.
- `mcp/session.rs`: `SeenClient {name, version, handle, first_seen, last_seen}` (Unix seconds), `clients()`
  (one entry per handle, last seen first, at most `MAX_SESSIONS`); `start()` sets both times
  (with a known handle `first_seen` stays), `client(id)` sets `last_seen`.
- Widget `web/src/settings.ts` (`data-widget="settings"` at the `<dialog id="settings">` from `base.html`):
  `[data-settings-open]` and Alt+, open (`showModal`) and load the frame (`fragment=1`) into
  `[data-settings-content]`; `a[data-settings-page]` changes the page; POST forms via `fetch`
  (urlencoded, follow redirects), `[data-settings-frame]` is taken over from the answer (also with 422);
  GET forms on `/einstellungen/…` load in the dialog. After success `baruch:settings-changed` with
  `detail.page`; it sets the theme immediately (`dataset.theme`). Style `static/settings.css`.
- Tests: `baruch-server/tests/round9_settings.rs` (15), `baruch-server/tests/personas.rs` (adapted:
  page assistants, redirect), `baruch-store/tests/round9_personas.rs` (4), unit tests in
  `settings/{mod,display,access,data}.rs`, `mcp/session.rs`, `view.rs`.

### Connections and sources (`connections/`, migration 0013)

- Migration `crates/baruch-store/migrations/0013_connections.sql`: tables `connection` (kind, config as a
  JSON object, never secrets; last check `checked`/`check_ok`/`check_message`/`check_details`),
  `work_rag` (per work the ways as a JSON list, `ON DELETE CASCADE`), `preferred_source` (bookmarks,
  `position` 1..n without gaps) and index `preferred_source_order`. Missing row = default.
- Store `crates/baruch-store/src/connections.rs`: `ConnectionKind` (`as_str`/`FromStr`, `default_config`),
  `ConnectionRow`, `ConnectionCheck`, `RagWay`, `PreferredSource`, `NewSource`, `SourceEdit`;
  `Store::connection_get/connection_list/connection_set` (changed settings clear the old check),
  `connection_record_check`, `work_rag`/`set_work_rag`, `sources_list/source_create/source_update/
  source_move/source_delete` (order without gaps in the same transaction). Refusals through
  `StoreError::Refused`, no new variant; `connection_bind_secret` (0014). Tests `tests/{connections,migration_0013,migration_0014}.rs`.
- Server `crates/baruch-server/src/connections/`:
  - `secrets.rs`: trait `SecretStore` (`get`/`set`/`delete` per account, service fixed `baruch`),
    `KeyringSecrets` (crate `keyring` 3, Linux Secret Service over D-Bus; also finds entries that Python
    `keyring` created without `target`), `MemorySecrets` (tests, throwaway server), `system_secrets()`,
    `secrets_mode` (`serve`: `--secrets keyring|memory` or `BARUCH_SECRETS`, default keyring only with the
    default data folder, otherwise memory – the entries hang only on service and account, not on the data folder);
    the server calls the blocking methods only through `secrets::get/set/delete` in `spawn_blocking`.
  - Binding of the secret to the target (migration 0014, `connection.secret_target`, `Store::connection_bind_secret`):
    `config::target` (origin of the address or host/port/TLS); `set_config` remembers the old target when the target changes and
    a secret is stored (or unreadable), `set_secret`/`delete_secret` set NULL (= current
    target), all under `write_lock`; `run_check` sends only with `secret_bound`, otherwise `ok: false` with
    `details.secret_withheld` without asking the other side. The search ways (Round 12: `pgvector::open_way`,
    `multigpt::open_way`, `rag::embed::EliezerEmbedders`) take the same check.
  - `config.rs`: allowed fields per kind, check of the addresses and fields, refusal of secret fields.
  - `check.rs`: checking (reqwest 0.12 with rustls, sqlx Postgres through the runtime API); `scrub` replaces a
    mirrored-back secret with `***` before anything is stored or shown.
  - `pages.rs`: `render_connections`/`render_sources` (contract signature `PageRender`, own minijinja environment
    with `templates/settings/{verbindungen,quellen}.html`) and the two form handlers.
  - `sources.rs` (RPC `sources.*`), `mcp.rs` (`sources_list`), `mod.rs` (RPC `connection.*`, `writes()`).
- `AppState::secrets` (`Arc<dyn SecretStore>`); `AppState::open` takes `MemorySecrets`, `serve` takes
  `AppState::open_with_secrets(&data, …)` with the keyring or `MemorySecrets` according to `secrets_mode`. No test creates `KeyringSecrets`.
- New crates only in `crates/baruch-server/Cargo.toml`: `keyring` 3 (`sync-secret-service`, `crypto-rust`),
  `reqwest` 0.12 (without defaults; `json`, `rustls-tls`, `rustls-tls-native-roots`), sqlx additionally
  `postgres` and `tls-rustls-ring-native-roots` (system certificates, also an in-house CA in the intranet), tokio
  additionally `time`. Licences of the roughly 100 new crates: MIT/Apache-2.0/ISC/Zlib, `ring` Apache-2.0 AND ISC,
  `webpki-roots` CDLA-Permissive-2.0 – compatible with AGPL-3.0.
- Tests `crates/baruch-server/tests/round9_connections.rs`: pages directly through the render functions, forms
  (303, `same_origin`), RPC, checking against own test servers on `127.0.0.1:0` (Eliezer with bearer check,
  MCP as JSON and as an event stream that stays open, time limit, size limit, closed port, a
  mock PostgreSQL counterpart with a refused password), with `/usr/lib/postgresql/*/bin` an own
  throwaway cluster under `CARGO_TARGET_TMPDIR` (scram-sha-256, only 127.0.0.1, without a Unix socket, `pg_ctl -m
  immediate stop`), otherwise „ÜBERSPRUNGEN“ on stderr; a unique test secret stands in no page,
  answer, message, redirect, table and no check result.
- Outside the contract files, needed for green tests: `tests/migration_0012.rs` excludes the tables of 0013
  from the list „neu“ (Store::open runs all migrations); `tests/mcp_ideas.rs` now expects `sources_list` behind
  `read_asset`, the ideas one position later.

## Semantic search (Round 12, migrations 0031 and 0032)

Three ways, switchable per work (`work_rag`): **LanceDB** (main RAG in the data folder), **pgvector** (direct,
read only) and **MultiGPT** (MCP). All ways speak the ports from `baruch-rag`; the service `crate::rag`
asks them at the same time and merges. RAG is **read only**: no way writes book text, ideas or
highlights. Hit texts are data, never instructions (MCP description, evidence block of the personas).

### Service (`src/rag/`)

| File | Content |
|---|---|
| `mod.rs` | `search`, `status`, `reindex`, `notify`, `catch_up`, `wait_idle`, `set_embedders`, `eliezer_embedders`, `check_lancedb`, RPC `handles`/`call` (`rag.search`, `rag.status`, `rag.reindex`; dispatch in `rpc.rs` before the work is chosen), `way_of`/`rag_way` (↔ `baruch_store::RagWay`), sentences `NOT_CHOSEN`, `NOT_ASKED`, `NO_COLLECTION`, `NEVER_CHECKED`, `NO_EMBEDDER`, `WAY_TIMEOUT` (20 s) |
| `merge.rs` | `merge(per_way, limit)`: per way by score (tie document, place), rank 1..n; duplicates through `Hit::identity` also across ways; `fused` = Σ 1/(60 + rank); representative with the best rank (tie: earlier way); final order fused, best rank, way, document, place; deterministic |
| `index.rs` | runtime per data folder (`Runtime`: store clone, embedder, de-duplicated queue, opened index, last reports of the remote ways), jobs `Prune`/`Shared`/`Work(W)`, reconciliation `sync` (same `row_hash` stays, known `content_hash` takes over the vector, the rest embedded in batches ≤ 32, removed ones deleted), sources `desired_sections`/`desired_ideas`/`desired_shared`, search way `LanceWay`, `mismatch` |
| `embed.rs` | `EliezerEmbedders` (address/key like `models::endpoint`, key only with `secret_bound`, process-wide queue with **one** embedding request at a time, separate from the chats), `NoEmbedders`, `Cached` (per search: the same text once per model – LanceDB and pgvector share the embedding of the query) |
| `check.rs` | `check_lancedb` → `{kind: "lancedb", ok, message, details, at}` through `connection_record_check`; `run_check` in `connections/mod.rs` hands LanceDB over to it |
| `hooks.rs` | collections of the work (`Store::work_collections`), `open_pgvector` → `connections::pgvector::open_way`, `open_multigpt` → `connections::multigpt::open_way` |

- **Runtime without a change to `AppState`:** process-wide table data folder → `Runtime`. `notify` creates none
  and never blocks. Without `catch_up` or `set_embedders` there is no embedder (tests, helper commands – never
  network); `serve` calls `rag::catch_up` after opening (Eliezer as embedder, enqueue `Prune`, `Shared` and all
  works).
- **Who reports changes:** `mirror.rs` (`record`, `record_switch`, `record_outline_by`, `record_idea`,
  `record_post`, `sync_work`), `works.rs` after `work.delete` (`notify(Some(work))`), `literature.rs` after every
  change to a highlight through `search_hooks::changed` (`notify(None)` for the shared area and
  `notify(Some(work))`). The search refreshes library titles at most every 30 s.
- **Errors:** repeatable up to 3 attempts (pauses 0.3 s / 1.2 s), then stalled (`Runtime::stale`, in the
  status `index.stale`); model or document prefix does not match → no attempt, no mixing, problem in the status,
  until `rag.reindex`. Since Round 19a only a real change (`notify` → `Runtime::enqueue`), a job
  that actually embedded again (`Outcome.embedded`), or the rebuild brings stalled work back. The
  search enqueues through `refresh_shared` → `enqueue_quiet` only `Prune` (cheap: count first) and at most every 30 s
  `Shared`, skipping stalled jobs; a successful job without embedding clears the
  problem only if nothing is left (otherwise the status would show „kein Problem“ in between). `Job::Work` of an
  unknown (deleted) work deletes its rows and enqueues `Shared` right afterwards (`Outcome.follow`):
  the ideas of the work are now „ohne Werk“.
- **Rebuild without waiting (Round 19a):** `reindex_report` takes the write lock only with `try_lock`; if the
  worker holds it, `Runtime::wish_reset` records the wish (whole index or works) and sets `abort`. The worker
  checks `abort` between two batches, during an embedding (`embed_or_abort`, every 50 ms) and in the pauses
  between attempts, discards the result of the aborted job, carries out the wish under the lock
  (`reset_index`) and re-enqueues. The interrupted job itself goes back to the front of the queue
  (duplicates only once): `reset_index` delivers for single works only their jobs and `Shared`; otherwise the running
  job of another work or `Prune` would stay half reconciled and without a note. RPC answer with `deferred`; `reindex` (hook `search_hooks::rag_reindex`)
  still delivers only `pending`.
- **Scoped replace (Round 19a):** `Index::replace(scope, remove, rows, dimension)` deletes only
  `id IN (…) AND <Bereich>` and refuses rows outside the scope (`RagError::Invalid`); `sync` passes the
  scope of the job. So a newly given identifier (`node.key`) in another work can no longer delete foreign
  rows. If an idea changes its work, the reconciliation of each scope clears its part.
- **Task prefixes (Round 19b, LanceDB with 19a):** `LanceConfig.prefixes` from
  `connections::config::embed_prefixes(Lancedb, …)`; `sync` embeds `document_text(text)` (the text itself stays what is stored and
  hashed), `LanceWay` the query as `query_text(query)`. `IndexMeta.document_prefix`
  (set with the first batch; missing = empty); `mismatch` reports a different document prefix as
  `ModelMismatch` („Aufgaben-Präfix geändert; Index neu aufbauen.“), `reset_index` then rebuilds the whole index.
- **Start without a request to the default address (Round 19a):** `catch_up` sets no embedder if the
  data folder is not the default folder (`baruch_store::default_data_dir`, compared canonically) and the
  connection `eliezer` was never saved (`ConnectionRow.updated` empty); `Runtime::unbound` keeps the sentence
  `NO_EMBEDDER_OWN_DATA` for the status. `reindex_report` binds Eliezer as soon as the connection is saved.
  An embedder set beforehand (`set_embedders`, tests) stays as before.
- **`weak` (Round 19a):** `rag::answer_json` sets `weak = baruch_rag::is_weak(hit.score)` per hit (RPC
  `rag.search`, MCP `rag_search`); `Answer` itself (frozen in `baruch-rag`) stays unchanged.
- **Search:** check the input (query 1..=1000 characters, `limit` 1..=50), ways of the work ∩ `ways` of the query, per
  way a plan (off: not chosen / not asked / no collection / `NotConfigured` on opening), all
  at the same time with a 20 s guard per way, `merge`. `Answer.ways` always names all three ways. Remote ways only with
  one of `MAX_REMOTE_SEARCHES` (4) slots (`remote_slot`, semaphore); without a slot failed after 20 s with
  `BUSY`, LanceDB keeps running, the last reported state of the way stays in place.
- **Shared area** (`desired_shared`): rejected highlights (`HighlightStatus::Rejected`) do not go
  into the index – as with the wording search.
- **Status without network:** LanceDB from the index; remote ways from the setting, the chosen collections and the last
  search of this process (if newer than the last check) or the last check; never checked → „aus“ with
  „noch nicht geprüft (Verbindung prüfen)“.
- **Index folder:** connection LanceDB `dir` (default `lancedb`, relative to the data folder, never the data folder
  itself, never under `works/`); one table `chunks` (`id`, `work`, `kind`, `document`, `section`, `title`,
  `text`, `meta`, `content_hash`, `row_hash`, `vector`), one model per index.
- **Hooks of the user interface** (`search_hooks.rs`): `rag_search`, `rag_status`, `rag_reindex`, `changed` – literature,
  ideas and the settings page call only these; personas and MCP call `assist::hooks::rag_search`/`rag_status`.
  Both forward to `crate::rag`.

### pgvector (`connections/pgvector.rs`)

- `check(config, secret)` for „Verbindung prüfen“ and `open_way(state, embedders)` as a `SearchWay`. One
  connection per search (sqlx runtime API, `PgConnectOptions::new_without_pgpass`, TLS as configured), everything in
  `BEGIN TRANSACTION READ ONLY` with `statement_timeout` 10 s (`begin_read_only`), connection at most 10 s.
- Mapping (`Mapping::from_config`, `config::effective` with defaults; `PG_MAPPING`, `is_identifier`,
  `split_table`) → confirmed against `information_schema.columns` before every query (schema from the search path or
  `schema.tabelle`, vector column of type `vector`, dimension from `atttypmod`, otherwise `vector_dims` of the first
  row). Identifiers only checked and in double quotes; vector, collections (`= ANY($2)`) and limit
  are bound parameters.
- Search: models of the chosen collections (with `model_column`) → embedding through
  `EmbedderFactory::for_model(embedding_model)` → model (`same_model`, lenient: alias) and dimension checked →
  otherwise `ModelMismatch` without a similarity query → `ORDER BY "vec" <=> $1::vector LIMIT $3`, score 1 − distance.
  Errors: login/rights → `Denied`, `57014` → `Timeout`, network → `Unreachable`, table/column → `Schema`.
- Round 19b (hardening): every field from the foreign table shortened in SQL via `left(…, n)` (constants
  `MAX_DOCUMENT_CHARS` 500, `MAX_SECTION_CHARS` 200, `MAX_TITLE_CHARS` 300, `MAX_URL_CHARS` 2000,
  `MAX_COLLECTION_CHARS` 200, `MAX_MODEL_CHARS` 200, text `SNIPPET_SOURCE_CHARS` 2000); the collection list of the
  check counts per full name in a subquery (at most 1000) and shortens outside. The search chooses a
  collection by the full **or** the shortened name (`= ANY($n) OR left(…) = ANY($n)`). `quoted` doubles
  quotation marks (instead of a mere `debug_assert!`): the schema from `information_schema` is unchecked and can
  contain any character; `to_regclass` gets the same quoted form. `tls_warning(config, has_secret)` for
  the check (`message`, `details.tls_warning`) and the page. The query is embedded with `config::embed_prefixes(Pgvector,
  …).query_text` (S1 function); the fields `query_prefix`/`document_prefix` are known to `config::fields`,
  checked with `baruch_rag::check_prefix` (not trimmed).
- `connections/collections.rs`: RPC `connection.collections_get/set`, `available_of` (common form
  `details.collections = [{name, count}]` of pgvector and MultiGPT).
- The connections form takes the fields as `Form<Vec<(String, String)>>` (repeated `collection`).

### MultiGPT (`connections/multigpt.rs`)

- `McpClient` – MCP over Streamable HTTP (`initialize` with protocol 2025-06-18, `notifications/initialized`,
  then `Mcp-Session-Id`/`MCP-Protocol-Version`; JSON or `text/event-stream`, a stream is checked after every
  piece – a half UTF-8 character at the end means „weiterlesen“, `find_answer`), `list_tools` over `nextCursor`
  (at most 10 pages), `call_tool` (`isError` → `Failed` with the text of the other side); limits `Limits`
  (search 15 s per call, 2 MiB), no redirects, `scrub_error` (token → `***`). `check::multigpt` uses
  the same client (the former `McpSession` has merged into it).
- `search_arguments` derives the arguments from the `inputSchema` of `search_documents` (query `query`/`q`/…,
  collections as a list or singly with one call per collection, hit count `limit`/`top_k`/…, clamped);
  if a parameter for the collections is missing, Baruch does not ask (`Schema`). `parse_hits` reads many forms
  (score clamped, otherwise 1 − distance, otherwise a substitute from the rank; URL only http(s)). Since Round 21a the substitute is
  `rank_score(rang) = WEAK_SCORE + (1 − WEAK_SCORE)/rang` (rank 1 = 1.0, falling, never below `WEAK_SCORE`): without
  a figure from MultiGPT the similarity is unknown, which is why a hit „aus Rang“ never counts as „schwach“ (formerly 1/rank:
  always weak from rank 3); `meta.score = "aus Rang"` stays as a marker.
- `MultigptWay`: one session per search (from the pool, see below), only the chosen collections;
  `document_info` adds titles/URLs (≤ 5), `read_document` missing snippets (≤ 3, ≤ 600 characters). Since
  Round 13 these follow-ups each run under `FOLLOW_UP_TIMEOUT` (4 s) and together under `FOLLOW_UP_BUDGET` (6 s),
  never beyond `rag::WAY_TIMEOUT` − 1 s; after a time limit or transport error there are no further
  follow-ups, the hits found go back unchanged.
- **Session (Round 13, contract §2.2):** `Session` (`open` from the settings like `open_way`, `connect`,
  `tools`, `schema`, `call` with own `Limits` per call, `scrub`, `release`, `close`). Process-wide pool per
  (address, hash of the token, Tokio runtime): at most two idle sessions; after `SESSION_IDLE` (120 s) an
  idle session ends with HTTP `DELETE` (`Mcp-Session-Id`, token). The tool list is valid `TOOLS_TTL` (300 s) per
  key – two searches in a row need one `initialize` and one `tools/list`. `release` puts the
  session back; after a transport or protocol error (also time limit, HTTP error) it ends itself
  instead with `DELETE` (in the background, so that the search does not wait); a dropped session (for instance
  when `rag::WAY_TIMEOUT` aborts the search) likewise. If a session from the pool no longer answers (HTTP
  400/404: the other side has ended it, or no connection came about – `reqwest::Error::is_connect`,
  `McpClient::unsent`), `call` reconnects once. After a timeout or an abort after
  sending, `call` does not repeat: the call may already have been carried out (an `upload_document` of the sync would
  otherwise exist twice). The runtime belongs to the
  key because reqwest binds its connections to the runtime in which they came about (tests have one each).
  The hash of the token is a process-random SipHash (no sha2 in the server); it serves only to tell apart.
  `expire_idle(url, frist)` ends idle sessions of an address early (tests, maintenance).
  `check::multigpt` uses no pool and has one common `tokio::time::timeout` (10 s) around all requests.
- **Full text (Round 13):** `MultigptWay::read_full` and `read_part` (`read_part_with_info` in addition `document_info`
  and the position of the part in the full text) through `read_document` with `FULLTEXT_LIMITS` (20 s, 4 MiB; larger → „Antwort
  von … ist größer als 4 MiB; abgebrochen.“), `\r\n` → `\n`. `page_key`: a name from `SECTION_KEYS` that takes a
  number (`page` always) – then Baruch calls `read_document` with a page; if the answer names page data, it counts
  as a page (`paged_by_remote`, `read_full` joins all pages with an empty line, at most 500 pages),
  otherwise as a whole document, which `split_parts` divides at paragraph boundaries into parts of at most `PART_CHARS` (20 000)
  characters (the parts one after another give exactly the text, so character ranges stay valid). If
  the schema demands an unknown parameter → `RagError::Schema`.
- **Assumptions (unchecked against the real MultiGPT, Round 13 „quellen“):** `read_document` takes the document under
  a name from `DOCUMENT_KEYS` and a page (from 1) under `page` or a number parameter from `SECTION_KEYS`;
  the answer carries the text under `text`/`content`/`body`/`markdown` (also under `document`/`result`/`data`) in
  `structuredContent` or as JSON in `content[].text`, otherwise `content[].text` itself is the text; page data
  are called `pages`/`total_pages`/`page_count`/`num_pages` or `has_more`/`more`/`next_page`/`next_cursor`; a page
  behind the end is `isError`. `document_info` names `title`/`name` and `url`/`link`/`source_url`. MultiGPT
  ends a session on `DELETE` and answers an unknown `Mcp-Session-Id` with 400 or 404.
- **Assumptions for the PDF import (Round 21a, after the tool description as of 2026-11-09, unchecked against the real
  MultiGPT; `pdf_import/remote.rs`):** `upload_document {collection (ID oder Name), content_base64, filename, url?}` →
  `document_id` (number or text) and `run_id`; the indexing follows later. Assumed file types PDF, DOCX, TXT, MD,
  images – **no EPUB** (that is why Baruch always reads EPUB itself). Progress through `run_status {run_id}` → `status`
  (`done`/`completed`/`finished`/`success`/`ready`/`indexed` = finished, `failed`/`error`/`cancelled` = failed);
  without `run_status` Baruch reads `read_document` with repetition until it answers; together at most 120 s.
  `read_document {document_id, page_from?, page_to?, paragraph_from?, paragraph_to?, section?}` delivers at most
  12 000 characters or 10 pages per call, text under `text` (also `content`/`body`/`markdown`), page count under
  `pages`, truncation under `truncated` (otherwise: 12 000 characters = presumably truncated). `document_info {document_id}` names
  `title`, `author`, `pages` and a recognised table of contents under `toc` (also `table_of_contents`, `outline`,
  `sections`, `chapters`; entries with `title`, `level` or section number `section` „1.2“, `page`, nested
  under `children`) – source of the proposed outline; if it is missing, numbered lines in the text count. The upload
  runs in one call with at most 30 s (upper limit of a session, `SESSION_CEILING`); very large files over a
  slow network can fail on that (honest message, offer „lokal lesen“).
- `open_way(state)`: no address → `NotConfigured`; token `baruch/multigpt` only with `secret_bound`.
- `pub mod mock` – `MockMultigpt` for tests: `start`, `url`, `set_tools`, `set_collections`, `set_hits`,
  `require_bearer`, `requests`, `tool_calls`, `configure(|b| …)` (`Behavior`: pages, answer form, `tool_error`,
  `delay`, `redirect`, `event_stream`, `split_utf8` (stream in two pieces, split inside the first multi-byte character),
  `documents`, `texts`; since Round 13 `pages` (`read_document` with `page`, answer `{document_id, text, page,
  pages}`), `tool_delays` (delay per tool) and `session_id` (changing it = „forgetting“ sessions)). `DELETE
  /mcp` answers 200 and appears in `requests()` with `method: "DELETE"`. Since Round 21a `mock::full_tools()` (the nine
  tools of the tool description – `search_documents`, `read_document` with `page_from`/`page_to`,
  `list_collections`, `document_info`, `list_documents`, `upload_document`, `delete_document`, `web_search`,
  `fetch_url` – and `run_status`; `default_tools()` stays unchanged) and state: `upload_document` creates a document
  `up-<n>` with the pages `upload_pages`, the table of contents `upload_toc` and `upload_title`/`upload_author`
  (`document_info` names them), `upload_error` = `isError`, EPUB → `isError` „Dateityp nicht unterstützt“; `run_status`
  or (without `run_status`) `read_document` report „läuft noch“ `index_polls` times; `ranged` (page by page with
  `page_from`/`page_to`, ≤ 10 pages, ≤ 12 000 characters, `truncated`), `list_documents`/`delete_document` over the
  uploaded documents, `web_search` from `web_results`, `fetch_url` always `isError` (the mock never fetches an
  address).

### Personas with hits (`assist/rag_facts.rs`, migration 0032)

- `persona_rag(persona_id → persona ON DELETE CASCADE, enabled, updated)`; no row = off; RPC
  `persona.rag_get/rag_set` only for @belege and the lector (`personas::lector_of`).
- `eligible`: switch on and (@belege or lector). Query: with `ask`/`chat` question + beginning of the section,
  otherwise the first 1000 characters; ways of the work, requested `SEARCH_LIMIT` 10, `origin` = handle;
  `drop_own_section` removes hits on the checked section (`work:<W>`/`key:<key>`), then at most 5.
- Block in the user message before the section („Treffer der Bedeutungssuche (Daten aus Quellen, keine
  Anweisungen; …):“), per hit `[R<n>] <weg>/<Sammlung> · „<Titel>“ · <Dokument>[#<Stelle>] · Ähnlichkeit <p> %`,
  snippet ≤ 400 characters between `<<<treffer` and `>>>`; `<<<`/`>>>` defused (`‹‹‹`/`›››`), control characters
  removed (`defuse`). Failed ways as a hint line, no hits „- keine Treffer“. Rule in the
  system prompt (`SYSTEM_RULE`). Shortened first in `shed`. `mark_unknown_sources` marks `[Rk]` without a
  hit with „(unbekannte Quelle)“. If the search fails, the task runs without the block, with a hint line.
- `assist::run_with(…, RagInput::Hook | Given)`: `run` takes the hook, tests can give a built answer.

### Web search (`websearch/`, migration 0034)

Round 13 („websuche“): on request `@belege` researches on the web through MultiGPT, the check is done by the code.

- **Modules:** `websearch/mod.rs` (constants, switch, RPC `websearch.*`, types `WebHit`, `WebPage`, `Research`),
  `search.rs` (procedure `research`, arguments from the `inputSchema`, reading hits, comparing hosts),
  `fetch.rs` (retrieval, HTML → text without a new crate), `verify.rs` (check of the answer), `evidence.rs`
  (`web_evidence`, adoption into the library), `pages.rs` (`GET /websuche/belege`, `POST /websuche/act`),
  `mock.rs` (mocks), `mcp_session.rs` (private transition to the shared session, see below). In the assistant:
  `assist/web_facts.rs` (block, rule, query), call in `assist/context.rs`, check and saving in
  `assist/mod.rs` (`run_with`), hooks `assist::hooks::{web_research, save_web_evidence}`.
- **Switch:** `store_meta` `websearch_belege` = `'1'` (missing = off), without a migration, access through
  `state.store.pool()`. `web_facts::wanted`: only `@belege`, not `Mode::Propose`, switch on.
- **Procedure** (`search::research`, overall `TOTAL_TIMEOUT` 30 s through `tokio::time::timeout`; what has been fetched by then
  stays): open a session like `open_way` (without an address or with a token bound elsewhere `NotConfigured`); if
  `web_search` or `fetch_url` is missing in `tools/list` → `Schema`. Preferred hosts (`source_hosts`: lower case, without
  `www.`, without duplicates) – at most `MAX_PREFERRED_CALLS` 3 calls „site:<Rechner> <Anfrage>“, then one
  general one (dropped when there are already 8 hits). Hits without duplicates, at most `MAX_HITS` 8, `preferred` by
  `host_matches` (equal or a subdomain, never a mere suffix). The first `MAX_FETCHES` 4 through `fetch_url`
  (`FETCH_LIMITS`: 10 s, answer ≤ 4 MiB, because MultiGPT can deliver the page in JSON and twice); the page
  itself ≤ `PAGE_MAX_BYTES` 1 MiB, otherwise a hint. Per call `CALL_TIMEOUT` 10 s; a failed call is a
  hint, not an abort. Pages: text at most `CHECK_CHARS` 200 000 characters, retrieval time stored in UTC, displayed (retrieval date in evidence and source line) in local time.
- **HTML → text** (`fetch::html_to_text`): `script`, `style`, `noscript`, `template`, `svg`, `head` are dropped
  (if the closing tag is missing, the text stays), block elements become lines, line breaks in the source
  white space, `<` without a letter behind it is text, entities (named ones of German typography, numeric).
  Title from the data or `<title>`.
- **Block** in the user message after the block of the semantic search: „Abgerufene Webseiten (…):“, per page
  `[W<n>] <Titel> · <URL> · abgerufen <TT.MM.JJJJ>[ · bevorzugte Quelle]` and at most `BLOCK_CHARS` 1 500 characters
  between `<<<seite` and `>>>`, defused with `rag_facts::defuse`; below it the hints, without a page „- keine
  Seite abgerufen“, on failure the line „Hinweis: Websuche ausgefallen (<Grund>)“. Rule `web_facts::SYSTEM_RULE`
  in the system prompt. In `shed` this block is dropped first (from the back).
- **Check** (`verify::check`) after `mark_unknown_sources`: quotations („…“, “…”, "…", »…«) on the same line as a
  known `[Wn]` → substring of the normalised page (white space, typographic quotation marks, soft
  hyphen; punctuation at the end of the quotation does not count, „…“/„[…]“/„...“ = gap in order), otherwise
  „(nicht belegt)“; `[Wn]` without a page „(unbekannte Quelle)“; addresses that were not fetched (compared without anchor
  and trailing `/`), „(nicht abgerufen)“; without evidence (no `belegt`/`nicht_zitiert`) „Angabe prüfen:
  nicht belegt“; always the line „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“ (only named pages, otherwise
  „Quellen: keine abgerufene Seite genannt“). Markings are not set twice.
- **Evidence** (`web_evidence`, migration 0034): `id, post_id → post ON DELETE SET NULL, work_id, node_key, persona,
  label, url, title, preferred, accessed, quote, verdict ∈ belegt|nicht_belegt|nicht_zitiert, page_hash,
  library_title_id, created`, index `web_evidence_post`. One row per quotation, a named page without a quotation one
  row `nicht_zitiert`; written after the post is saved (`post_id`). `page_hash` is the SHA-256 of the
  checked page text (`baruch_store::sync_hash`; hashes only in baruch-store). The post gets
  the source „Web: n Beleg(e)“, `fragments/assistants.html` then links it to the evidence page.
- **Adoption** (`evidence::adopt`, under the write lock): only `belegt`, not already adopted; through the
  existing API of baruch-library (`add_title` with key `web-<rechner>-<JJJJMMTT>[-n]`, or an existing
  title `web` with the same address; `add_quote`; `add_highlight` kind `quote`, human), then `library_title_id` and
  `rag::notify(state, None)`.
- **Decision: no MCP tool.** The web search has its value in the check within `@belege`; an
  MCP passage into the internet would enlarge the attack surface (a client could use Baruch as a retrieval service),
  and MCP clients have their own web searches. The evidence stands in the post and is readable through `read_node` (posts).
- **Session:** the shared `connections::multigpt::Session` (pool, contract §2.2); the tool names
  `WEB_SEARCH_TOOL`/`FETCH_URL_TOOL` come from `connections/multigpt.rs` (re-exported in `websearch/mod.rs`).
- **Mocks** (`websearch::mock`): `MockWebMcp::start(Some(&site))` (MCP over Streamable HTTP, `web_search`
  with the filter „site:“, `fetch_url` fetches `*.example` from the `MockSite` on 127.0.0.1 with the host as `Host`,
  everything else `isError`; `DELETE /mcp` → 200; `requests`, `tool_calls`, `configure` with `search_delay`,
  `search_error`, `text_results`, `bearer`) and `MockSite::with_fixtures()` (light lore, archive, guest book with
  prompt injection, a page over 1 MiB, one that answers only after 15 s). Tests `tests/round13_websuche.rs`,
  `crates/baruch-store/tests/migration_0034.rs`.

#### Assumptions (unchecked against the real MultiGPT)

- `web_search` takes the query under one of the names from `QUERY_KEYS` (`query`, `q`, …) and understands
  „site:<Rechner> …“ like SearXNG; the hit count under one of the `LIMIT_KEYS` (e.g. `max_results`), clamped to
  `minimum`/`maximum`. Without `properties` Baruch sends only `{query}`.
- The answer of `web_search` is a list (directly or under `results`, `hits`, `items`, `data`, `documents`,
  `organic`, `result`, `web`, also one level deeper) with `url`/`uri`/`link`/`href`, `title`/`name`/`filename` and
  `snippet`/`content`/`description`/`summary`/`text` – in `structuredContent` or as JSON in `content[].text`;
  otherwise Baruch reads Markdown links `[Titel](URL)` and free addresses from the text.
- `fetch_url` takes the address under `url`/`uri`/`link`/`href` and delivers the content under
  `content`/`text`/`body`/`markdown`/`html` (otherwise as text in `content[]`), optionally with `title`; HTML or
  Markdown/text. MultiGPT follows redirects itself; Baruch names the requested address.
- If a schema demands a further mandatory parameter, that is honestly `Schema` (no search).

### MCP (`mcp/rag.rs`)

`rag_search`, `rag_status` (after `bible_search` in `tools/list`), through `assist::hooks`; origin = client.
No `rag_reindex`.

### Sync (`sync/`, migration 0033)

Round 13, task „abgleich“ (plan „RAG-Abgleich mit MultiGPT“): texts, not vectors; every side embeds
itself. Only the human triggers (button on `/abgleich`, RPC `sync.run`); **no MCP tool**, no start
on start-up or saving.

| File | Content |
|---|---|
| `sync/mod.rs` | RPC `sync.*` (`handles`/`call`, dispatch in `rpc.rs` before `crate::rag::handles`), run in the background (`start`: process-wide mark per data folder, one run at a time, row in `sync_run`, `wait_idle` for tests), `Limits` (`DEFAULT`: 500 documents per collection, 2 MiB per document, 64 MiB per run, 30 s per call; `set_limits` per data folder for tests), `credentials` (address and token like `open_way`, without network), `Link` (session only on first need), `Report` (numbers and hints, every message without the token), `local_document` → `LocalDocument {title, url, text, fetched}` (for the document view offline), `collection_for` („Baruch: <Werktitel>“, one line, ≤ 200 characters) |
| `sync/upload.rs` | Baruch → MultiGPT: `chapters` (one document per chapter: a chapter or part begins a document, following sections belong to it; text = current revisions, title „<Nummer> <Titel>“, item `work:<W>/key:<K>`, hash `sync_hash(Titel + "\n" + Text)`), per work with `upload = 1`: same hash → nothing; otherwise mark `UPLOADING` in `remote_stamp`, `upload_document`, write the state, then `delete_document` of the old one; missing chapters and states of deleted works → `delete_document`, state gone. Rights, transport and schema errors block further writes in the run; an `isError` with an intact session (`Link::answered`) without a rights word (`RIGHTS_WORDS`) only this document (Round 19a) |
| `sync/download.rs` | MultiGPT → Baruch: per chosen collection `list_documents` (page by page through `cursor`/`offset`/`page`, ≤ 50 pages; a full page – as many entries as requested, capped at `maximum` – without `has_more`/continuation counts as possibly incomplete, Round 19a), same `remote_stamp` and copy present → do not read; otherwise `read_document`, check limits, same hash → nothing; otherwise write `sync_document` (same id, so that the index keeps its rows). Missing ones (with a complete list) and deselected ones → removed locally. If something changed: `rag::notify_data(data, None)` |
| `sync/args.rs` | arguments from the `inputSchema` (`Filler`: collection as a list or text, mandatory data that Baruch does not fill → `RagError::Schema`), answers (`listed`, `uploaded_id`, `document_text`), key lists `TITLE_KEYS`, `CONTENT_KEYS`, `CURSOR_KEYS`, `MIME_KEYS`, `ID_KEYS`, `STAMP_KEYS` |
| `sync/pages.rs` | `GET /abgleich`, `POST /abgleich/act` (own template environment with `base.html`, `sync.html`); the page never asks MultiGPT, the collections loaded last lie in the memory of the process |
| `sync/mock.rs` | `MockDocs` for tests: MCP mock with all six document tools, `DELETE /mcp` → 200, recording (`requests`, `tool_calls`, `clear_requests`), `put_document`/`remove_document`/`set_url`, `deny_write(Some(Deny::Forbidden \| Deny::ToolError))` (no `docs.write`), `Behavior.stamps`, `page_size`, `delay`; since Round 19a `upload_without_id`, `reject_empty`, `silent_cap`, `lose_upload_answer` |

- **Tables (migration 0033):** `sync_work` (switch per work, default 0, collection recorded on switching
  on, `confirmed`; goes with the work), `sync_collection` (chosen collections, `position` 1..n),
  `sync_state` (side `baruch` or `multigpt`, collection, item, `remote_id`, hash, `remote_stamp`; `work_id`
  without a foreign key, so that the next run finds documents of deleted works), `sync_document` (local copies,
  `UNIQUE (collection, document_id)`), `sync_run` (log). Store `baruch-store/src/sync.rs`, `sync_hash`
  (SHA-256 hex, hashes only in baruch-store).
- **Local index:** kind `remote` in `KINDS`; `desired_shared` adds `desired_remote` (chunks through
  `baruch_lancedb::chunk::chunks`, id `remote:<sync_document.id>:<n>`, document `remote:<Sammlung>/<Dokument-ID>`,
  without a work, `meta {collection, document, sync}`); `url_of` → `/quelle/multigpt?collection=…&document=…`.
- **Settings → Work** (`settings/work.rs`, `settings/werk.html`): switch with a mandatory check box,
  form to `POST /abgleich/act` with `back`. **Menu:** tools → „Abgleichen“ (`base.html`).
- **Contents are data:** titles and texts from MultiGPT are output escaped and only searched as a source,
  never used as an instruction; messages run through `scrub` (token → `***`).

#### Assumptions (unchecked against the real MultiGPT)

The user checks them himself; the mock `MockDocs` follows them.
- **`list_documents`:** parameter for the collection from `COLLECTION_KEYS` (mandatory – without it Baruch does not ask),
  optionally `limit`/`page_size`/`per_page`/`max_results`, continuation through `cursor` (answer `next_cursor`,
  `nextCursor`, `next_page_token`, `next`) or `offset`/`page` with `has_more`. Answer: list under `documents`,
  `items`, `results`, `data` or `files` (also one level deeper or the list itself); per entry ID
  (`document_id`, `doc_id`, `documentId`, `id`), title (`title`, `document_title`, `name`, `filename`), URL and
  **modification time or hash** (`updated_at`, `modified_at`, `updated`, `modified`, `last_modified`,
  `lastModified`, `content_hash`, `hash`, `sha256`, `checksum`, `etag`, `version` – the first one found is remembered as
  `<Schlüssel>=<Wert>` in `remote_stamp`). If MultiGPT reports none of these, Baruch reads every document and
  compares the hash of the text (plan: then note it down as a small extension in MultiGPT).
- **`read_document`:** without a page parameter the **whole** text comes, as plain text in `content[].text` or in
  the data under `content`/`text`/`body`/`markdown`. If the schema names a page parameter (`page_key`) and
  the answer page data (`pages`/`total_pages`/… or `has_more`/`next_page`), the sync reads, like the
  full text of quellen, all pages and joins them with an empty line (`multigpt::read_all_pages`, shared with
  `MultigptWay::read_full`); so the local copy equals the online full text (hash, index, check of
  highlights offline). Whether the real MultiGPT delivers page by page is an assumption; both forms work.
- **`upload_document`:** parameters collection (`COLLECTION_KEYS`, as a list if the schema allows one),
  title (`title`/`name`, otherwise `filename` with `<Titel>.md`), text (`content`/`text`/`body`/`markdown`), optionally
  `mime_type`/`content_type` = `text/plain`. Answer with the new document ID (`document_id`/`id` …, also under
  `document`/`result`/`data`). Since Round 19a an upload without an ID counts as successful with a hint: Baruch can then
  neither replace nor delete the document and stores a blocking note (state with hash, `remote_id`
  empty), so that the same content does not go out again on every run; after a change it is uploaded anew,
  the old document stays at MultiGPT (hint). **The collection comes about by itself with the first upload**; an own tool for creating it is
  not known.
- **`delete_document`:** parameter document (`DOCUMENT_KEYS`), plus the collection if the schema knows one.
  If deleting the old revision fails after replacing a chapter, its ID stays as a state of its own
  (`sync_state.item = work:<W>/key:<K>/stale:<alte ID>`, without a migration); it belongs to no chapter,
  so the next run deletes it like a missing chapter.
- **Scope Baruch → MultiGPT:** only the current revisions per chapter. Comments/findings (posts of the
  personas), which `docs/Plan.md` („RAG-Abgleich mit MultiGPT“) also provides for, do not go out so far – an open
  deviation, noted in the backlog.
- **Rights:** a key without `docs.write` gets HTTP 401/403 or `isError` – both become a hint.
  Baruch recognises an `isError` as a rights problem (blocks the rest of the run) by words like „Berechtigung“,
  „permission“, „forbidden“, „docs.write“, „verweigert“, „denied“ (`RIGHTS_WORDS`); without such words it applies
  only to the one document (assumption about the texts of MultiGPT).
- **Completeness of the list:** if MultiGPT caps `limit` silently and reports no continuation, that can only be
  recognised if the page has as many entries as requested (`limit` at the `maximum` of the schema); if the
  server answers below the requested `limit`, the list counts as complete (assumption).

### Store and migrations

- `0031_rag_collections.sql`: `work_rag_collection(work_id → work ON DELETE CASCADE, way ∈ pgvector|multigpt,
  collection 1..200 characters without a line break, position ≥ 1, PK (work_id, way, collection))`; no row =
  no collection (way off). `set_work_collections`: trimmed, duplicates removed, ≤ 100, `BEGIN IMMEDIATE`,
  `position` 1..n without gaps; `RagWay::Lancedb` → `Refused`.
- `0032_persona_rag.sql` see above; `PersonaRow` unchanged.
- Tests `crates/baruch-store/tests/{migration_0031,migration_0032,rag_collections}.rs` (database in the state up to
  0030 rebuilt, lossless, a second opening equal, deleting takes the rows along).
- Round 13: `0033_sync.sql` (abgleich: `sync_work`, `sync_collection`, `sync_state`, `sync_document`, `sync_run`,
  see "Sync"), `0034_web_evidence.sql` (websuche: `web_evidence`, see "Web search"; the switch lies without a
  migration in `store_meta`, key `websearch_belege`), in the library `0106_source_remote.sql` (quellen:
  `library_title_remote`, `library_highlight_check`). Tests `baruch-store/tests/{migration_0033,migration_0034}.rs`
  (store up to 0032 rebuilt with data) and `baruch-library/tests/migration_0106.rs` (library up to 0105): all
  rows unchanged, only new empty tables. Since then `migration_0013.rs` limits the library to 0105.
- User question 12: `0035_outline_request_add.sql` creates `outline_request` anew with the request kind `add` (see
  "Outline through MCP and requests"); test `baruch-store/tests/migration_0035.rs` (store up to 0034 rebuilt with requests:
  all rows including id equal, other schemas equal, a deleted id is not given again).
  `sync_hash` (SHA-256 hex) in `baruch-store/src/sync.rs` also fills `web_evidence.page_hash`.

### Building and size

- `lancedb` brings roughly 260 crates along (lance, arrow, datafusion, object_store, parquet); `lance` needs the
  protobuf compiler `protoc` when building (here `~/.local/bin/protoc`). Licence Apache-2.0, compatible with
  AGPL-3.0.
- Every test binary of `baruch-server` links LanceDB statically (≈ 500 MB instead of ≈ 145 MB). With one
  binary per test file (61) `target/` grew to 38–40 GB after the full run. Since Round 13 (preliminary task „bau“)
  the integration tests of the server are **one** binary: `autotests = false`, `[[test]] name = "server"`,
  `tests/main.rs` with one module per subject (`tests/<gebiet>.rs`), common helpers `tests/common` and
  `tests/pgvector_support` included once; `every_test_file_is_bundled` reports a test file without a
  `mod` entry. Measured after `cargo clean` and a full `cargo test --workspace -j 4`: `target/` 38 GB → 9.0 GB,
  test binaries 146 → 86, duration 18 min 1 s → 13 min 31 s, 1195 → 1196 passed tests (+1 guard test).
  LanceDB stays built in permanently without a Cargo feature (the service unchanged).
- One process for all server tests: process-wide states are now shared by all modules. PDF generation under
  `common::PDF_SERIAL` (mark „PDF entsteht“ per work id), the model queue through
  `common::model_queue` (only tests of one module at a time: `round10.rs`, `round10_models.rs`), pgvector tests
  share one container with a database of their own each.

### Tests

`tests/round12_lancedb.rs` (index, model change, without embedder, one embedding at a time),
`round12_pgvector.rs` (own podman container `docker.io/pgvector/pgvector:pg17`, otherwise an initdb cluster without
pgvector, otherwise „ÜBERSPRUNGEN“; helper `tests/pgvector_support/mod.rs`: tests running at the same time share one
container, per test an own database `rag_<n>`), `round12_search_ui.rs`,
`round12_multigpt.rs`, `round12_personas_rag.rs` and end-to-end `round12.rs` (fixed mock vector space
`Concepts`, `MockMultigpt`, pgvector container if present: the index follows, meaning without a common word,
three ways merged without duplicates, a failure does not block, collections limit, Eliezer off → FTS and
reason on the pages, literature/ideas without JavaScript, MCP with token, @belege with evidence block and
injection attempt).

Round 13: `round13_quellen.rs`, `round13_websuche.rs`, `round13_abgleich.rs` per task and end-to-end
`round13.rs`: **one** MultiGPT as a small MCP front (`OneMultigpt`), which passes every tool to the mock of its
task (`search_documents` → `MockMultigpt`, `web_search`/`fetch_url` → `MockWebMcp` with `MockSite`,
document tools → `MockDocs`), embeddings through `MockEliezer`: hit → document view → adoption →
title in the index → check of the highlights; @belege with a preferred source, „(nicht belegt)“, injection page,
adoption of an evidence item; sync only after confirmation, only what changed, fetching and finding offline, failure with
a hint, offline the local copy in the document view.

## Personas for real (Round 10)

### Port and model backend

- **Port** `assist/port.rs`: `ModelPort { chat(state, ModelCall, deltas: UnboundedSender<String>), models(state),
  alias(name), context(state, persona) → ContextBudget {tokens, reported, warning} }` (since Round 15 with a
  default implementation = setting of the persona) with `ModelCall {model, messages, max_tokens, temperature, label,
  persona}` (handle of the persona, empty = none; chooses connection and release), `ModelAnswer {text, model,
  finish_reason, prompt_tokens, completion_tokens}`, `ModelFailure` (`NotConfigured`, `Unreachable`, `Denied`,
  `Timeout`, `UnknownModel`, `Busy`, `Cancelled`, `Failed`; `Display` in German, never a secret), `ModelList`.
  `NoModel` honestly reports „Kein Modell-Backend angebunden …“. **Abort** = dropping the future (client
  disconnects → stream drops → task drops → HTTP request to Eliezer aborts, slot in the queue free).
- **Adapter** `assist/eliezer.rs` (`EliezerPort`): maps `ModelCall`/`ModelAnswer`/`ModelFailure` 1:1 onto
  `baruch_llm::ChatRequest`/`ChatReply`/`LlmError` and calls `crate::models`.
- **`models.rs`**: `chat(state, ChatRequest, label, deltas)` – address from the connection `eliezer`, key from
  the keyring `baruch/eliezer` only with `connections::secret_bound` (otherwise nothing at all goes out), alias before
  sending, process-wide `Queue` (`static`, one request at a time, at most 8 waiting); `list(state)` →
  `Listing {models, live, fetched}` (cached 60 s per address, not through the queue);
  `alias`, `status`, `probe` (short test), upper limit `BARUCH_MAX_TOKENS` (environment, optional; clamps
  `max_tokens` of every call, for instance for manual checks with short answers), `persona_states`/`persona_summary` (reconciliation of the persona models),
  `remember` (cache from „Verbindung prüfen“); RPC `model.*` (`handles`/`call`).
- **`baruch-llm`**: limits (`Limits`): connection 10 s, first piece 180 s (loading the model), pause 60 s,
  total 600 s, model list 10 s, answer at most 1 MiB, `max_tokens` clamped to 1–4096, no
  redirects. Only `choices[0].delta.content` counts (`reasoning_content` skipped); `[DONE]` ends
  reading immediately. Errors: connection → `Unreachable`; 401/403 → `Denied` (hint `baruch/eliezer`); 404 or
  „model not found“ → `UnknownModel`; 409/429/503 → `Busy`; otherwise `Failed` with at most 300 characters of the
  other side, key removed from it (`scrub`); time limits → `Timeout`. Baruch **never** requests a
  context length and never calls `/api/v1/models/load|unload|estimate`.
- `connections::run_check`: one `tokio::time::timeout` (10 s) around the whole check; for Eliezer afterwards the reconciliation
  of the personas (`details.personas`) and `models::remember`. The usage hint of the connection Eliezer is no longer a
  STUB (`connections::usage_is_stub`).

### Course of a task (`assist/mod.rs`, `assist::run`)

Check the persona (active, otherwise -32602) → read the context **without** the write lock (`assist::context::prepare`) →
call the model **without** the lock (with Gemma ~25 s) → post-processing (`assist::prompts`, hook
`hooks::mark_answer`) → only then `write_lock`, resolve `key` anew, write: posts through `Store::add_post`
(author = persona handle), proposals through `Store::add_proposal` (always `proposed`, `model` = actual
model, note „Vorschlag von @x“, heading line of the shown revision enforced, `assist::force_heading`).
Errors or abort store nothing. Chat (`assist::chat`): post of the sender immediately, then per mentioned
persona `run` one after another (at most three, no relay). `assist/rpc.rs` (RPC `persona.ask/review/propose/chat`),
`assist/routes.rs` (`POST /assistenten/act`, `POST /assistenten/stream` with its own `Stream` that drives the
task; new dependency `tokio-stream`).

- **Context** (`assist/context.rs`): system = role prompt (+ hook `scripture_facts`); user = work (title,
  kind, target group), chapter surroundings (chapter, neighbouring sections with ≤ 300 characters of beginning each), with the lector the
  memory (ideas with `idea.node_id = key`, not deleted, plus ≤ 10 open ones without a reference), in the chat the
  last 8 posts, then the section of the shown revision (`rev`, otherwise the current one) between `<<<`/`>>>`,
  finally the task. Tokens ≈ ⌈characters/3⌉, budget = context length − `max_tokens` − 256; shortened first is
  the chapter surroundings, then memory or chat history, finally the section from the end with
  „[… gekürzt: n Zeichen]“ → `truncated: true`. `max_tokens`: ask 700, review 900, chat 500, propose
  min(4096, section tokens·1.5 + 200); `temperature` 0.4; `label` „@x <art> key K“ (without text).
- **Role prompts** (`assist/prompts.rs`): defaults for `@lektor`, `@exeget`, `@schrift`, `@belege`, `@nathan`,
  `@hoerer`, otherwise general from the role; a stored prompt (`persona.prompt`, migration 0015) replaces
  the default entirely. Hard rules with counter-examples („Falsch: … / Richtig: …“) – see `docs/Bedienung.md`
  „Assistenten fragen“.
- **Post-processing**: if the answer begins (after white space, quotation and Markdown characters, regardless of
  case, only whole words) with „Als Assistent“, „Als KI“, „Als Sprachmodell“, „Gerne“ or
  „Natürlich“, Baruch requests anew **once** (additional instruction `prompts::RETRY_NOTE` at the end of the
  system prompt); if the second one begins like that too, its first sentence is dropped (up to `.`/`!`/`?`/`:` with white space
  behind it or up to the end of the line). While streaming Baruch holds back the first 24 characters until it is clear whether the
  answer begins like that. Proposals: code block ``` removed.
- **Hooks** (`assist/hooks.rs`): `scripture_facts` → `crate::scripture::facts_for`, `mark_answer` →
  `crate::scripture::mark_answer`.
- **Lector**: `baruch_core::LectorChoice`, stored in `store_meta` key `lector` (`auto` or a
  handle; no row = fixed `@lektor`, trial since 2026-11-06); store `lector_choice`/`set_lector_choice`;
  `personas::lector_of(rows, &choice)`. If the fixed persona is inactive, „automatisch“ applies.
- **Migration 0015** `persona.prompt TEXT CHECK (NULL oder 1–8000 Zeichen)`; store `persona_prompt`,
  `set_persona_prompt`, `PersonaRow.prompt`, `MAX_PROMPT_CHARS`. No data change.
- **User interface**: `fragments/assistants.html` (form `assist-form` with check/proposal, chat input to
  `persona.chat`), widget `web/src/assist.ts` (`fetch` + `AbortController`, SSE parser `parseEvents`,
  provisional post in the persona colour, afterwards `baruch:navigate {url}`), style `static/assist.css`; settings
  `settings/assistants.rs` + `settings/assistenten.html` (lector form, `<datalist id="baruch-models">` from
  `state.model.models`, warnings through `state.model.alias`, role prompt).
- **Test model** `assist::mock::ScriptedModel` (public, only for tests): script per call (`Step::words`,
  `Fail`, `Hang`, `Wait` + `release()`), `calls()`, `cancelled()`.

### Lector with the whole chapter (Round 15, „lektor“)

This supersedes the items "Context" (chapter surroundings with 300 characters per neighbour, shortening of the section) and
"Post-processing" (forbidden opening also for the proposal) above.

- **Interface S2:** chat in `assist/chat.rs` (`mentions`, `chat`, `ChatOrder`, `run_chat`, `chat_json`);
  `Progress` extended by `Posted(SavedPost)` (post of the sender saved), `Retry {persona, reason}` (re-request:
  „verbotene Einleitung“ or „fehlendes „Kritisch:““) and `Compressed(Compression)`; `Compression {message,
  summarized, chunks}`, `Scope {Section (Vorgabe), Chapter}`, `Order.scope`, `Outcome.compressed`,
  `Outcome.proposals` (proposal for the chapter). SSE forwarding in `assist/routes.rs`.
- **Context** (`assist/context.rs`, `prepare_inputs` with `RagInput`/`BibleInput`): `chapter_units` delivers the
  parts of the chapter (nearest `NodeKind::Chapter` above the section or itself, with all nodes below it;
  with `WorkKind::Sermon` the whole work; without a chapter the node with its sub-items), per part the current revision,
  its own in the shown one. The user message is built by `Frame::call`: preamble (work, findings of the other
  personas – posts whose author is the handle of another persona, per part the last three, in the chat without
  its own section –, memory, chat history, fact blocks), then „Kapitelumfeld:“ with every part as
  `--- Titel ---` and `<<< … >>>` (wording, „nur Zusammenfassung“ or „nicht mitgegeben“; the subject of the check
  as a reference), then the subject of the check with a head, finally the task. `Scope::Chapter`: all parts are the
  subject of the check (`max_tokens` ask 900, review 1500).
- **Budget:** `state.model.context(state, &persona).await.tokens` (interface S1, `ModelPort::context`) −
  `max_tokens` − 256, in characters ·3. Extras (`Extras`: web, SWORD, RAG, findings, memory from the back, chat from
  the front) are shortened as long as something is too much and they occupy more than a third of the budget; if the space
  afterwards does not suffice for the largest part of the subject of the check plus 600 characters of surroundings, also below that (`truncated`).
  Chapter and subject of the check are never shortened; `Prepared::fits` says whether they fit in their wording. If not
  even the minimum fits (less than 600 characters of space), `prepare` honestly refuses (-32000).
- **Chunking** (`assist/chunk.rs`, pure; orchestration in `assist/mod.rs`, `compressed_run`): subject of the check in
  pieces of at most ⅔ of the space (if even the lower limit below does not fit with that, in turn ½, ⅓, ¼ –
  `PIECE_SHARES`: a smaller core leaves more space to the surroundings) (`chunk::split`, cut at a paragraph, otherwise sentence end, otherwise white space,
  overlap ⅛, at most 600 characters; a section up to ¾ of the space stays whole; chapter: `chunk::pack`). Per
  piece `chunk::plan_piece` plans the surroundings: everything in its wording; as long as it does not fit, the part farthest from the core
  as a summary, afterwards – only if it saves space – as a mere title. The summaries needed are fetched by
  `plan_and_summarize` **before** the first check call and **one after another** (own call, `SUMMARY_SYSTEM`,
  `max_tokens` 80–300, label „… Zusammenfassung“, `persona` set – the release „extern“ applies here too);
  afterwards it plans anew with the real lengths (at most three rounds). The space per piece is `frame.room` minus
  `piece_extra` (per core part the surroundings line „wird geprüft, steht unten“, plus what the head „Stück k von n (…)“ and
  the addition in the task, or the own task for the proposal, are longer than counted in `prepare`). Before the first
  model call `plan_and_summarize` checks a lower limit per piece (core in its wording, every other part as short
  as possible); if not even that suffices, an honest refusal (`too_many_parts`, -32000, nothing stored). Afterwards
  all calls are built and **measured** (`check_calls`: characters of all messages ≤ (context length −
  `max_tokens` − 256) · 3) before the first one goes out; the unchunked call is measured too and chunked when
  it exceeds. Then `Progress::Compressed`, then per piece
  one call (stream with an empty line between the pieces), findings with `chunk::merge` without duplicates (equal or one
  wholly inside the other, from 24 characters; the more detailed one stays). Note „Komprimiert: …“ (`compression`) as the last
  paragraph of the post or in the note of the proposal. A proposal for the section is never chunked (the
  section must fit whole, otherwise `too_long_for_proposal`).
- **Proposal for the chapter** (`propose_chapter`): per part with text one call after another (part in its wording,
  surroundings planned as above, task `propose_part_task`), afterwards `proposal_text` (code block, „Kritisch:“ lines removed,
  heading enforced); unchanged and cut-off parts are dropped; storing happens only at the end under
  **one** write lock, exactly one `proposed` per part.
- **„Kritisch:“** (`prompts::needs_critical`, `has_critical`): `answer_call` re-requests with lector, exegete and Nathan
  at most once per task (`CRITICAL_RETRY_NOTE`, `Progress::Retry`); if the re-request fails, the first answer
  stays. If it is missing in the stored text, `post_body` appends `CRITICAL_MISSING_NOTE`. For the proposal
  `call_model` checks no opening (`check_opening = false`), `strip_critical_lines` removes finding lines.
- **Fact block from SWORD** (`assist/bible_facts.rs`, hook `hooks::bible_research` → `crate::bible_research::research`,
  data and switch with the branch „anbieter“): block „Belege aus SWORD“ with `[Sn] <Art> <Modul> · <Stelle> · Lizenz:
  …` and text between `<<<beleg`/`>>>`, defused with `rag_facts::defuse`; hints of the research below it;
  Bible wording that already stands in the fact block of the reference check is dropped; rule `SYSTEM_RULE`; budget shared
  with RAG/web (`BIBLE_MAX_CHARS` = min(4000, budget/5) to `research`); `[Sn]` without an entry → „(unbekannte Quelle)“
  (`rag_facts::mark_unknown`). Until the integration `research()` is a stub body (`None`) – tests through
  `BibleInput::Given`.
- **User interface:** `templates/fragments/toast.html` (notice box `div.toast.toast-<level>[data-widget=toast]`,
  visible without JavaScript), widget `web/src/toast.ts` (moves boxes into `.toast-stack`, „Schließen“, listens
  to `baruch:toast {message, level, summarized?}`; „warn“ without expiry, „info“ 8 s), style `static/toast.css`;
  `templates/fragments/assist_scope.html` (radio buttons `scope`), included by the branch „auftraege“.

### Bible references (`baruch-scripture`, `scripture.rs`)

- `data/versification.tsv`: per line `OSIS<TAB>Kapitel<TAB>Verse`, 66 books, 1189 chapters, 31172 verses; only
  numbers, **German versification**. Origin: versification „German“ of the SWORD project (CrossWire Bible Society,
  `include/canon_german.h`, SHA-256 in the head of the table, retrieved 2026-11-06); the SWORD module of the Schlachter 1951
  (`GerSch`) is laid out in this versification (`Versification=German`). OT: books, chapters, verses from
  `otbooks_german`/`vm_german`; NT: chapters as in the SWORD standard list, verses from `vm_german`. Only
  numbers are taken over. No longer used: eBible.org `deu1951` (VPL and USFM) is converted to the English versification (KJV, 31102 verses)
  – it stood here until 2026-11-06 and reported, for example, „Ps 51,21“ as invalid. The test
  `versification.rs` checks the sum 31172 and Joel 4/Malachi 3, `references.rs` references that are valid only in German
  versification. Never change by hand. Licence: `canon_german.h` is under GPLv2; only the numbers are taken over as
  facts, no code (decision of the user 2026-11-06).
- Parser: a comma with white space before a numbered book („Joh 3, 1. Joh 4“) separates references and does not introduce a
  verse (`verse_mark`). Since Round 17b: verse lists with commas (`comma_verse`), number words (`COUNT_WORDS`,
  `count_word_follows`), clock times after a weekday (`weekday_clock`, only `scan`), „… Vers 23“ in the same sentence
  (`verse_later`) – see "Round 17b – stellen".
- `src/books.rs`: 66 books (OSIS, name, abbreviation, further spellings); comparison without dots, white space,
  case; every spelling belongs to exactly one book. Parser: recursive descent over byte positions, never
  a panic.
- Server `src/scripture.rs`: RPC `scripture.check` (`check(text)`, shared with MCP `scripture_check` in
  `src/mcp/scripture.rs`), `facts_for(handle, section)` (fact block only for `@schrift`/`@belege`),
  `mark_answer(handle, section, body)` (line „Wortlaut: ungeprüft …“ or a hint to @schrift). Read only.
  Since 2026-11-06 with a Bible: `facts_for_work`/`mark_answer_for_work` (hooks in `assist/hooks.rs`, async with
  `state` and work), `check_with(choice, text)`; `mark_with` joins torn-apart digits of a reference of the
  section in the answer together again (`wording::rejoin_split_refs`, finding „Mk 1 0,42–45“: parser and
  display write the number correctly, the gap stood in the text of the model).

### SWORD Bibles (2026-11-06, `baruch-sword`, `bibles.rs`, migration 0030)

- **Decision on the reader (format):** the crate **`sword-modules` 0.1.0** (Logoxide, **CC0-1.0**, pure Rust, no
  FFI) reads the file format; checked in the source (`~/.cargo/registry/src/…/sword-modules-0.1.0`): drivers zText,
  zText4, RawText, RawText4 (plus commentaries/lexicons, which Baruch does not use), conf according to `docs/spec/conf.md`
  (continuation lines, RTF `About`), `DataPath` relative to the module folder (leading `./` or `/` removed), all 18
  SWORD versifications as facts from the runtime API, index per testament (position 0 module head, 1 testament head,
  per book introduction, per chapter introduction, then verses), block cache, normalisation of the entries. It
  documents a clean-room replacement of parts formerly ported from libsword (`CLEANROOM.md`); dependency
  `topos-bible` 0.6 (CC0-1.0, references, not used by Baruch) and its regex, smallvec, itertools and others
  (MIT/Apache). Included with `default-features = false, features = ["zip"]`: no installer, no network (ureq,
  zip, tar), no bzip2/xz (since Round 17c `features = ["zip", "bzip2", "xz"]`, still without `install`/`https`). **Not used:** libsword (GPL-2.0) and `rsword_chirho` (GPL-2.0-or-later), no
  code from them. Baruch's own code: module folders and choice, check of support, `DataPath` guard
  (`..` → refused), OSIS/ThML/GBF → plain text, versification reconciliation, wording comparison, search, test modules.
- **Crate `baruch-sword`** (pure except for file access): `scan(dirs)` → `BibleInfo` (name, description, About as
  text, Versification, Lang, Version, ModDrv, CompressType, BlockType, SourceType, Encoding, DistributionLicense,
  Copyright, `notice` = licence note, `supported`/`problem`), the first occurrence of a name wins; only
  Bible modules. `open(info)` → `Bible` (`verse`, `intro`, `slot_text`, `passage` at most 400 verses, `search` linear
  with hit and time limit). Supported: zText/zText4 with ZIP or LZSS, RawText/RawText4; versification only
  „German“ and „KJV“ (`baruch_scripture::versification`), others → „nicht unterstützt“ (STUB in
  `versification.rs`); encrypted modules → STUB. (State before Round 17; since R17 every versification of
  `baruch_scripture::versification()`, BZIP2/XZ and `CipherKey`, see "Round 17c – pruefanzeige".) `text::to_plain`: `note`, `rdg`, `title` (except
  `canonical="true"`), `catchWord`, `figure` (ThML also `scripRef`, `h1`–`h6`; GBF `<RF>…<Rf>`, `<TS>…<Ts>`)
  are dropped; `divineName`, `w`, `q`, `transChange` stay as text; entities, white space collapsed.
  `fixture`: writes test modules (zText BOOK/CHAPTER/VERSE with zlib, RawText) with its **own** position arithmetic from
  the numbers of `baruch-scripture`; tests `tests/modules.rs` read them back through `sword-modules` (every kind of position,
  both versifications, all block kinds) and compare the tables of `sword-modules` verse by verse with ours.
  Evidence on the real module GerSch: `ot.bzv` 241850 bytes = 24185 positions, `nt.bzv` 8246 positions – exactly the
  number of positions of the versification German.
- **Versification KJV:** `baruch-scripture/data/versification-kjv.tsv` (31102 verses; the former eBible table from
  the history before `6e1e268`), equal to the KJV table of `sword-modules`; `versification::{german, versification}`.
- **Wording** (`baruch_scripture::wording`, pure): `inline_quotes` („…“, “…”, »…«, «…», "…", `#quote[…]`; an
  empty line ends the search), block quotations through `baruch_wysiwyg::parse` (`BlockKind::Quote`, range of the content
  by bracket counting, `bibles::block_quotes`), `quote_for` (quotation directly before or after the reference, in between only
  white space/punctuation/other references of the same list, at most 24 bytes; block quotation with the reference in the attribution
  or at the end), `words` (Typst markup removed, hyphens/apostrophes/soft hyphen inside the word removed,
  hyphenation „Hei- land“ joined except before „und/oder“, lower case), `compare` (gapless word sequence per part between
  omissions → matches, otherwise a word diff over the longest common subsequence, limited to the quoted
  range). Another versification in the module: checked only if every affected chapter has the same number of verses in German and in the module,
  otherwise `versification_mismatch` (no mapping between versifications).
- **Server `src/bibles.rs`:** RPC `bible.*`, `Choice` (none, missing, unreadable, ready), cache of
  opened modules per conf file and modification time (at most 8), reading in `spawn_blocking` for scan, opening and
  search; `check_wording` per reference. Page Settings → Sources (`connections/pages.rs`, `settings/quellen.html`,
  block `#bibeln`). MCP `src/mcp/bibles.rs`. Only the running service reads the real module folders; tests always set
  their own folders.
- **Store (migration 0030):** `bible_dir (position, path UNIQUE)` and `store_meta.bible_dirs` (set = own
  list, also empty; missing = defaults), `work_bible (work_id → work ON DELETE CASCADE, module)`; `bible_dirs`,
  `set_bible_dirs`, `work_bible`, `set_work_bible` (`src/bibles.rs`), no commit in the mirror (no book text). Test
  `tests/migration_0030.rs` (lossless, a second opening equal, deleting the work takes the choice along).

### Connection per persona (Round 15b, `models.rs`, `baruch-llm/src/client.rs`, migration 0036)

- **`baruch_llm::Client`** (contract §3): `BackendKind {Eliezer, Openai, Ollama}`, `ModelInfo {id, context_length}`,
  `Client::new(kind, Endpoint, Limits)` with **one** `reqwest::Client` per endpoint (clones share it; connections
  stay in the pool of reqwest), `models()` (`GET /v1/models`, with Eliezer in addition the context lengths from the
  management list), `context_length(model)`, `chat_stream`. The free functions `models`, `models_with`,
  `chat_stream`, `chat_stream_with` stay as wrappers (own HTTP client per call, as before). Test
  `tests/backends.rs` counts the TCP connections at the mock (keep-alive): four queries through one client = one
  connection, two free queries = two.
- **Reported context length – assumptions:**
  - *Eliezer:* `GET /api/v1/models` (management list in the form of LM Studio 0.4) with `key`,
    `max_context_length` and `loaded_instances[].config.context_length`. Source: `docs/wiki/API.md` and
    `crates/eliezer-api/src/lib.rs` in the project Eliezer (read, not checked against the running service).
    Reported is the length of the loaded instance, otherwise `max_context_length`. Read only; never
    `/api/v1/models/load|unload|estimate`. Whether Eliezer loads a model not yet loaded with exactly this length
    is still not visible to Baruch.
  - *Ollama:* reported is the **effective** length, never the maximum: the chat runs through
    `/v1/chat/completions` without `num_ctx` (the OpenAI interface of Ollama takes none), Ollama then takes
    `num_ctx` from the Modelfile, otherwise its default, and silently cuts longer input off at the front. Hence
    `ollama_context`: `num_ctx` from `parameters` of `POST /api/show {"model"}`; otherwise `context_length` of the
    loaded model from `GET /api/ps` (`ollama_loaded_context`, assumption: newer versions report it; if
    `/api/ps` is missing, that does not count as an error); otherwise `OLLAMA_DEFAULT_CONTEXT` = 4096 (assumption after the Ollama docs;
    a larger default through `OLLAMA_CONTEXT_LENGTH` is seen by Baruch only with a loaded model) – each at most
    `<arch>.context_length` from `model_info`. All unchecked against a real Ollama, only against the mock
    (`MockEliezer` with `Flavor::Ollama`: `/api/show`, `/api/ps` without a loaded model). Model list through
    `/v1/models`.
  - *OpenAI-compatible:* no report; the setting of the persona applies.
- **Server `models.rs`:** connections from the store (`model_connection`), `resolve` (address, key from the
  account of the connection, only if bound to the current target), `client_for` (pool per kind, address and
  fingerprint of the key), queue **per endpoint** (`status()` summarises), `route(state, persona)`
  with the release „extern“ (external connection without `persona_backend.external_allowed` → `NotConfigured`, no
  request), `chat_on`, `list_on` (60 s cache, on error the expired list with a hint),
  `context` (min(setting, report), remembered 60 s, at most 10 s waiting time, never fails),
  `context_cached` (without network, for `persona.list` and the tabs not shown), `probe_on` (result in
  `model_connection.probe`), `check_connection`, RPC `model.*`. `EliezerPort::chat` calls `route` with
  `ModelCall::persona`, `EliezerPort::context` calls `models::context` – so the release applies to every model call
  of the personas (also summaries of the lector), and the lector gets its budget through `state.model.context`.
  Embeddings (`rag/embed.rs`) keep using the connection `eliezer` and the free functions; if the
  model connection Eliezer (id 1) is marked as external, `rag::embed::endpoint` refuses every embedding
  (`EXTERNAL_REFUSED`, reported as „kein Einbettungsmodell“) – embedding has no persona, thus no release.
- **Location by address** (`models::looks_local`, without DNS): local are loopback, private, CGNAT and
  link-local addresses, ULA, host names without a dot and `.localhost`/`.local`/`.lan`/`.internal`/`.home.arpa`.
  `model.connection_set` without `external`: when creating `!looks_local(url)`, when changing external stays, and a new
  address not recognisably local makes it external. `describe` sets `locality_warning` when a connection kept as local
  points to an address not recognisably local (display on the page, no block: an intranet name
  may deliberately be local).
- **Store (migration 0036):** `model_connection (id, name UNIQUE NOCASE, kind, url NULL nur bei id 1, external,
  account UNIQUE, secret_target, probe, checked, position, …)` without AUTOINCREMENT (otherwise the migration would change
  `sqlite_sequence`, which the older migration tests compare); new ids are given by the store through
  `store_meta.model_connection_last` never twice (a new account `baruch/model-<id>` never inherits the key of a
  deleted connection). The connection Eliezer (id 1) stems from the migration, its address and binding stay
  in `connection` (not kept twice), a trigger prevents its deletion. `persona_backend (persona_id,
  connection_id ON DELETE CASCADE, external_allowed)`; no row = Eliezer, not released. `PersonaRow` stays
  unchanged. Test `tests/migration_0036.rs`.
- **Pages:** Settings → Connections, section `#modelle` (`connections/pages.rs`, form
  `POST /einstellungen/modelle/act` in the router of `connections::writes`, behind `guard::same_origin`);
  Settings → Assistants, per tab `action=backend` and `action=bible` (`personas.rs`, `settings/assistants.rs`).

### Commentaries and lexicons (Round 15, items 5a–5c, migration 0037)

- **`baruch-sword/src/reference.rs`:** `Commentary` (RawCom, RawCom4, zCom, zCom4; versification German or KJV;
  `entries(book, start, end)` merges equal entries of neighbouring verses into one range; linear search)
  and `Lexicon` (RawLD, RawLD4, zLD; `lookup` with `@LINK`, `keys(prefix, limit)`, `search` in keyword and text).
  Plain text through `text::to_plain`, per entry at most `MAX_ENTRY_CHARS` (20 000). Since R15 `scan` also fills
  `Scan::commentaries`/`lexicons`, `BibleInfo.kind` (`SourceKind`); `Scan::find` stays with Bibles. HREFCom,
  RawFiles, encrypted modules and foreign versifications: `supported: false` with a reason. Test modules:
  `fixture::Layout::{RawCom, ZCom}` and `fixture::{LexiconFixture, LexLayout, write_lexicon}` (index, `.dat`,
  zLD blocks with ZIP), tests `tests/reference.rs`.
- **Server `bibles.rs`:** cache per kind (at most 8 open modules), `commentary_by_name`,
  `lexicon_by_name` (`Source::Ready`/`Problem`), RPC `commentary.*`, `lexicon.*`, `bible.work_sources`/
  `set_work_sources`, `bible.modules` with `kind`. HTML building blocks for the literature: `commentary_panel_html`
  (`<details class="commentaries" data-widget="commentaries">`, per module name, entries, licence; everything escaped)
  and `commentary_hits_html` (groups „Kommentar“ with links `/literatur/bibel?work=…&ref=…&kom=1<tail>`, total
  3 s, 10 hits per group). Style `static/commentaries.css`. MCP `mcp/bibles.rs`: `commentary_lookup`,
  `lexicon_lookup`, `lexicon_search` (read only).
- **`bible_research.rs` (data of the fact block):** `research(state, work, persona, text, max_chars)` – switch
  and permission (`personas::bible_state`: @belege default on, lector default off, otherwise never), references only from
  `baruch_scripture::scan` (invalid ones → hint, no text), per reference the wording of the work's Bible, then per chosen
  commentary its entries; finally lexicon articles on keywords (since Round 17b: Strong numbers of the verses with
  a Bible with markup, then Strong numbers and capitalised words of the text with a lemma probe;
  at most 5 per lexicon). Total length ≤ `max_chars` (per evidence item ≤ 1500), at most 12
  references; reading in `spawn_blocking`. `None` if no block belongs there (switch off, not allowed, neither Bible nor
  commentary nor lexicon chosen). The presentation in the context is the business of the lector (`assist/bible_facts.rs`).
- **Store (migration 0037):** `work_sword_source (work_id ON DELETE CASCADE, kind commentary|lexicon, module,
  position)`, `persona_bible (persona_id, enabled)`; `src/sword_sources.rs`. Test `tests/migration_0037.rs`.
- **Page:** Settings → Work, block `#kommentare` (`settings/work.rs`, form `POST /einstellungen/werk/bibeln`,
  handler `connections::pages::work_bibles_act`, because the routers of `settings/` do not belong to this branch).

### MCP tokens (migration 0016)

- Table `mcp_token (id, name, hash UNIQUE, prefix, created, last_used, revoked)`, partial index
  `mcp_token_active_name` (name unique among valid ones). Store `src/mcp_tokens.rs`: `MCP_TOKEN_PREFIX =
  "baruch_"`, `McpToken` (without hash/plain text), `mcp_token_create` (32 bytes `getrandom`, Base64url without padding,
  SHA-256 hex, `prefix` = first 12 characters), `mcp_token_list`, `mcp_token_revoke` (idempotent),
  `mcp_token_verify` (only non-revoked ones; `last_used` at most once per minute).
- Server `src/mcp/tokens.rs` (RPC `mcp_token.*`, human only), `src/settings/access.rs` (page and
  `POST /einstellungen/mcp/act`; the fresh plain text goes through its own `tokio::task_local` only into this one
  answer, `Cache-Control: no-store`), template `templates/settings/mcp.html`.

### Tasks from the user interface, threads and MCP limits (Round 15, „auftraege“)

- **Stream:** `assist/routes.rs` forwards the events from `Progress` (`posted`, `retry`, `compressed`,
  interface S2) and outputs `done.compressed`; `assist.ts` puts the own post into the chat immediately,
  discards what the persona has shown so far on `retry` and reports `compressed` as `baruch:toast` (receiver `toast.ts` of the
  lector). Cancelling: in the chat, post and finished answers are already stored (redraw), otherwise nothing.
- **Notice page:** `assist::routes::notice_page[_with]` renders `assist_notice.html` (status 200) in
  `view::environment()` + own template; if `fragments/toast.html` (lector) is registered there, the page includes
  it, otherwise an equivalent box. Used by `/assistenten/act`, „an Persona geben“ and the Bible page.
  Return jumps only to own paths (`safe_back`).
- **Threads without a section (migration 0038):** tables `chat_thread (id, work_id → work ON DELETE CASCADE, title,
  created_by, created, updated)` and `chat_thread_post (thread_id → chat_thread ON DELETE CASCADE, position UNIQUE je
  Faden, author, body, sources JSON, created)`; store `src/threads.rs` (`thread_list/get/create/add_post`, one
  transaction each, unknown or foreign thread → `Refused`). Server `src/threads.rs`: RPC `thread.*`, pages
  `/faeden`, `/faden`, `POST /faden/act`, and `threads::chat` – its own lean context (role prompt, work,
  last 12 posts, task) with its own model call through `state.model` (forbidden opening once anew,
  `hooks::mark_answer`, `CUT_NOTE`); `assist::run` stays bound to sections. Test `migration_0038.rs`.
- **Relay:** one stage – only answers to the original message call on, every persona at most once,
  all answers together `MAX_CHAT_PERSONAS`. In the thread in `threads::chat` (`relay: true` only for MCP), in the
  section chat through MCP until Round 22 in `mcp/tools.rs` (`relay_section` after `assist::chat`; since Round 23 one way
  `assist::chat::relay_targets` for user interface and MCP: only mentions from answers
  that do not themselves stem from the relay of Round 30c – `ChatOutcome::relayed` –, framed through
  `OrderFrame::relay` with `relay_text` as the task). The chat of the user interface (`persona.chat`) passes on only with
  direct address (Round 30c).
- **Queue per MCP client:** `mcp/quota.rs` (included through `#[path]` in `mcp/tools.rs`, `mcp.rs`
  unchanged): counter per (state of the server, client handle), `Slot` releases when dropped; at most 2 running
  or waiting `chat`/`ask_persona` per client, otherwise `isError` before any model request. Since Round 23
  all clients without a token additionally share four slots (see "Round 23 – Hardening", mcp-namen).
- **Ideas:** `ideas::idea_source` names the origin („Einfall n von @x“) as the source of the post; the page
  of the idea offers the persona choice (`persona` in the form).
- **Badge „neu“ (`chat.ts`):** posts of the human carry `data-post-own` (the template compares with „Autor“ =
  `mirror::HUMAN_NAME`); only times of the server or „leer“ are remembered, plus per work the last state for the
  first visit of a section.

### Integration Round 15 (integrator, 2026-11-08)

- Merged `team/anbieter` → `team/lektor` → `team/auftraege` (`--no-ff`); the interfaces S1/S2 lay in the other branches
  via cherry-pick, in conflicts the revision of the owner applied (§9), plus the wiring lines
  from §10 united (`lib.rs` STYLE, `tests/main.rs`, `web/src/main.ts`, `docs/RPC.md`).
- **`.toast` used twice:** since Round 1 `app.css` has `.toast` for the short red error messages
  (`literature.ts`, `outline.ts`, `workspace.ts`, `ideas.ts`, `wysiwyg.ts`, fixed at the bottom centre). The hint of the
  lector uses the same class; without adjustment it stuck at the bottom without JavaScript, and the error messages lost
  their red. That is why `static/toast.css` applies only to `.toast[data-widget="toast"]` (test
  `round15::the_hint_toast_does_not_restyle_the_error_toasts_of_other_widgets`).
- **Sources for [Sn]:** `Prepared.bible_labels` (short reference per evidence item) and `bible_facts::cited_sources`: every `[Sn]` quoted in the
  answer becomes a source of the post („SWORD [Sn] <Art> <Modul>, <Stelle>“); without that the number stayed
  unresolvable for the reader.
- **Context with OpenAI-compatible:** `models::context` and `context_cached` never return a
  reported length for `kind = openai` (contract §3), even if an Eliezer connection with the same address has filled the caches
  (per address and model).
- **Check MultiGPT:** the limit per request lies 1 s above the common limit (`check::multigpt`); with equal
  length the limit of `list_collections` expired first under load, and the check reported „steht“ (test
  `round13_quellen::the_check_has_one_timeout_for_all_requests` failed once in the full run).
- MCP `initialize` names the new tools (`commentary_lookup`, `lexicon_*`), `scope`, the thread without a section and
  the queue per client. `/faeden` and `/literatur/bibel` stand in `round9_menu::all_pages`.
- End-to-end `tests/round15.rs`: fact block with the real `bible_research::research` (Bible, commentary with
  an injected „Anweisung“, lexicon) for `@belege` and the lector with the switch; commentary panel and group
  „Kommentar“; „an Persona geben“ from the Bible page; compressed chapter finding through the form without JavaScript →
  notice page with the real toast; persona on an Ollama mock with a reported smaller length (compressed,
  one after another); external connection without release also for summaries, without a request; OpenAI without length; MCP
  `commentary_lookup` through the real entry.

## Search and mirror in detail

- **baruch-library:** `fts_query` turns NUL and control characters into word separators; if nothing is left,
  `EmptyQuery`. FTS5 `snippet()` works with the marks U+E000/U+E001; `search.rs::snippet_html`
  escapes the text (`& < > " '`) and translates the marks, balanced, into `<mark>…</mark>`.
  Highlights stand as `kind = 'highlight'` in the index (triggers from `0103_highlights.sql`, also for
  renaming and deleting the title); the query joins them with `library_highlight`, leaves out
  `rejected` and fills `quote_id`/`highlight_id` in the hit. `build.rs` rebuilds on new migrations.
- **Posts in the index (0105, `0105_post_index.sql`):** `post_fts_delete` removes the index row of a
  deleted post, `post_fts_move` enters title and work of the new section when `post.node_id` changes
  (deleting works, merging outline items); the migration clears orphaned `post` rows.
  The query leaves out work hits without a living section anyway.
- **baruch-gitmirror:** before every commit all intermediate folders of the old and new paths are checked with
  `symlink_metadata`, when writing missing folders are created one by one (`walk_parents`).
  A symlink results in `MirrorError::InvalidPath` before anything is changed in the working folder. A file
  that is itself a symlink is replaced by a real file, its target stays untouched. Without a
  change to the tree no commit comes about, unless `CommitRequest::allow_empty` is set (log commit
  with the same tree as its predecessor).
- **Catching up:** if a commit fails, the next snapshot catches up the full DB state (test
  `a_mirror_catches_up_after_an_error`).

## Course of a change

1. `revision.accept`, `revision.set_current`, `revision.save`, `revision.update` or `revision.delete`
   takes the **write lock** (`AppState::write_lock`).
2. The change is written to SQLite in one transaction.
3. Still under the lock: snapshot of the current revisions, commit in the mirror with author, body and
   trailers of this change (`Persona`, `Model`, `Baruch-Node`, `Baruch-Revision`, `Baruch-Action`; see
   "Revision tree").
4. Lock free. The answer carries `commit` or, on a mirror error, `mirror`.

The lock covers steps 2 and 3: if two changes ran at the same time, the first
snapshot would otherwise contain both and the second commit would drop out. Discarding and posts do not change the mirror
and run without the lock.

## Protection of the interfaces

`/rpc`, `/act` and `/mcp` lie behind `guard::same_origin`: `Sec-Fetch-Site` must be `same-origin` or
`none`, `Origin` (if present) must match `Host`; otherwise 403. `/rpc` and `/mcp` demand
`Content-Type: application/json` (otherwise 415), so that a foreign `fetch` would need a preflight.
Reading GET pages that would trigger expensive ways with credentials – the semantic search in
`GET /literatur?…&lmode=rag` and `GET /einfaelle?…&mode=rag` – check the same rule (`guard::check`); for
a foreign page no search runs, the page names `literature::FOREIGN_PAGE`.

`/mcp` then checks the other side: without a token only loopback (`127.0.0.1`, `::1`, `::ffff:127.0.0.1`),
otherwise 401 with `WWW-Authenticate: Bearer realm="baruch"`. For this `main.rs` starts with
`into_make_service_with_connect_info::<SocketAddr>()`; if `ConnectInfo` is missing, the request counts as foreign.
In front of all routes, also the reading ones, stands `guard::local_host`: `Host` must be an IP address (v4 or `[v6]`,
with or without port), `localhost` or a host name additionally allowed at start
(`AppState::allowed_hosts`, see "Host rule behind a reverse proxy" below), otherwise 421. So
DNS rebinding does not work (a foreign name that points to 127.0.0.1 sends its name as `Host`). Without `Host`
(only without a browser) it passes.
With `Authorization: Bearer <Token>` (Round 10) `mcp::endpoint` checks the token through
`Store::mcp_token_verify` (only the SHA-256 hash stored): valid → access from any machine, client name =
token name; invalid or revoked → 401 with `error="invalid_token"`, also over loopback.
`guard::remote_only_mcp` (in the router around everything, inside `local_host`) lets only `/mcp` through from other sides outside
loopback, otherwise 403; without `ConnectInfo` (tests) the request passes.
**Clickjacking:** `guard::no_framing` (outermost layer, thus also on 403/421) sets on every answer
`X-Frame-Options: DENY` and `Content-Security-Policy: frame-ancestors 'none'` (appended to an existing CSP like the
`sandbox` CSP of the images). Otherwise a foreign page could invisibly frame the consent dialog of the outline, the
release „extern“ or the connections; the framed form would go off same-origin, and
`same_origin` would not take effect. Baruch frames itself nowhere (test `round15_nachbesserung`).

**About page (Round 24).** `about.rs`: pages `/ueber` and `/ueber/lizenz`; constants `VERSION`, `LICENSE_ID`,
`REPOSITORY`, `HOMEPAGE`, `LICENSE_TEXT`, plus `THIRD_PARTY_PLACES`, `release_url()` and `find_third_party` (looks for
`THIRD-PARTY-LICENSES.txt` relative to the running program, never reads it). No network access, no database; Baruch
nowhere asks for new versions. Since Round 24 the keyring (`connections/secrets.rs`) is chosen per target system
through features of the crate `keyring` (`[target.'cfg(target_os = …)'.dependencies]` in
`crates/baruch-server/Cargo.toml`: Linux `sync-secret-service` + `crypto-rust`, macOS `apple-native`, Windows
`windows-native`); the branches for macOS and Windows are compiled but have never run. The version has one source
(`[workspace.package] version`); `tests/round24_abnahme.rs` pins the places that repeat it
(`site/hugo.toml`, `CHANGELOG.md`, `README.md`).

**Host rule behind a reverse proxy (Round 24).** Since Round 24 access under a host name goes through
`guard::AllowedHosts` (`crates/baruch-server/src/guard.rs`), filled by `main.rs` from `--allow-host NAME`
(repeatable, only `serve`) and `BARUCH_ALLOWED_HOSTS` (comma or white space) and set in
`AppState::allowed_hosts` before `app(state)`; `AppState::open_with` leaves the list empty, so all tests and `import`/`export`
run as before Round 24.

- `AllowedHosts::parse` takes only whole names: 1–253 characters, labels of 1–63 characters from `a–z0–9-` (no `-` at the
  edge), stored in lower case, without port, without wildcard, no IP address, not `localhost`, last label
  not a number (`127.1`, `0x7f.1` – browsers read that as IPv4). An invalid name is an error with the name
  in the message; `main.rs` then ends `serve` with exit 2 before the data folder is opened.
- `AllowedHosts::allows(host)` = `allowed_host(host)` or `is_extra(host)`; `is_extra` compares the name
  without port (port as with `allowed_host`: 1–5 digits) and case-insensitively with the list. `local_host` lets
  exactly that through; everything else stays 421 (another name, sub- or super-name, spaces, wildcard).
- **Never local:** `guard::via_proxy(hosts, headers)` is true if `Host` is an additional name – and, as soon as
  any names are allowed at all, also if `Host` is missing or unreadable (then a proxy stands in front, and
  „lokal“ would not be shown). `mcp::endpoint` accepts a request without a token only if the other side is
  loopback **and** `via_proxy` is false: under an additional name `/mcp` always demands a token,
  although the other side (the proxy on the same machine) is loopback. Without allowed names `via_proxy` is
  always false.
- **Origin check unchanged:** `guard::check` compares `Origin` (without scheme) literally with `Host`. The
  proxy must thus pass `Host` through unchanged (including the port); otherwise 403. The scheme plays no role, that is why
  `Origin: https://name` passes against `Host: name`, although Baruch itself speaks only HTTP.
- **Deliberately not evaluated:** `X-Forwarded-For`, `X-Forwarded-Proto`, `X-Forwarded-Host`, `Forwarded`. They
  would come from anyone who reaches Baruch directly; evaluating them would only be safe with a list of trusted proxies.
  Consequence: `guard::remote_only_mcp` sees loopback for every request through the proxy and lets all routes through –
  without accounts the access protection lies entirely with the proxy (without accounts Baruch has no login, `docs/Betrieb.md`;
  with accounts the layer `auth::gate` demands a session since Round 35, see "Round 35 – Accounts and roles"). If the proxy stands on
  another machine, the other side is not loopback, and it stays at „nur `/mcp`“.
- **Known limit:** if the proxy rewrites `Host` to `127.0.0.1:<Port>`, the request cannot be told apart from a local
  one; `/mcp` would then go without a token. Baruch cannot recognise that; that is why `docs/Betrieb.md`
  demands a probe after setting up.
- The page „MCP-Zugang“ (`settings/access.rs`) shows the address under an additional name as
  `https://<Host>/mcp` (assumption: TLS in the proxy; Baruch does not know the scheme of the request) and the command with
  `--header "Authorization: Bearer <Token>"`.
- Tests: unit tests in `guard.rs` (`parse`, `allows`, `is_extra`, `via_proxy`), `tests/round24_betrieb.rs`
  (router with names set, program with an invalid value).

Open: no own TLS. Since Round 35: login with accounts (without accounts still none); the session cookie
carries `Secure` behind the proxy, the other cookies (display, work opened last) still do not.

## Literature

The literature is a popup of the header and has its own pages; the work page stays three-column.

- **Data** (`baruch-library`, migration `0103_highlights.sql`): table `library_highlight` with title
  (`ON DELETE CASCADE`), optional quotation (`ON DELETE SET NULL`), location (`locator`), character range
  `start_char`/`end_char` (both NULL or `0 ≤ start ≤ end`, Unicode characters; in the quotation
  `add_highlight` also checks that the end lies inside the quotation and that the text, apart from white space, is the excerpt –
  `normalize_space`: runs of Unicode white space become one space, edges are dropped –, and then stores
  the exact excerpt of the quotation), text (not empty), kind
  (`quote`, `core`, `contradiction`, `check`), note, originator (`human` or `assistant` with model) and
  status (`active`, `proposed`, `rejected`). A CHECK condition allows personas only `proposed`/`rejected`.
  `src/highlights.rs`: `add_highlight` (human → `active`, persona → always `proposed`; without a location the
  location of the quotation), `list_highlights`, `get_highlight`, `update_highlight`, `delete_highlight`,
  `cite_typst`. Errors through its own `HighlightError` (`Library`, `Unknown`, `Empty`, `InvalidRange`, `TextMismatch`),
  `LibraryError` stays unchanged.
- **Server** (`src/literature.rs`): own template environment (`OnceLock`, templates `literature.html`,
  `source.html`, `fragments/library.html`, `fragments/highlights.html`); the one of the work page stays as it
  is. `pages()` delivers `GET /literatur` and `GET /quelle`, `writes()` `POST /literatur/act` (in the router of the
  writing routes, thus behind `guard::same_origin`). `handles`/`call` serve `literature.render` and
  `highlight.*` from within `rpc::call`. The module builds addresses itself (`query_escape`) and passes them as
  safe values to the templates.
- **Section of the literature pages:** `/literatur` and `/quelle` carry the fixed key themselves: from
  `?key=` (takes precedence over `?node=`), otherwise determined from the position when loading. It stands in the links to the
  document view, in the hidden fields of all forms (`place_fields`) and as `data-key` at the widget
  `source`.
- **Taking over a quotation:** `highlight.cite {highlight, key}` translates `key` under the write lock into the
  position, reads the current revision, appends `cite_typst` and creates a proposal through the unchanged `Store::add_proposal` (persona
  `@literatur`, model `baruch-zitat`, note „Zitat aus @schlüssel“). The current revision stays; the Git
  mirror changes only on accepting. `cite_typst` escapes backslash, backtick and
  `# [ ] * _ $ < > @ ~ = - + /` as well as an enumeration „1.“ at the start of a line with a backslash; a bare page number becomes `S. …`.
- **Widgets** (`web/src/literature.ts`): `literature` (the `<dialog>`, Alt+L, content through
  `literature.render`, search in `lq` of the address, work hits through a hidden `data-nav` link in the
  work area), `source` (context menu of the document view; character range in Unicode characters as in the
  database, start and end from DOM ranges from the start of the quotation, text literally through `Range.toString()` instead of
  the collapsed `selection.toString()`) and the marking of the location in the text field with
  `#fundstelle`. `cite` through `/literatur/act` demands the field `key` (missing → 422 „Abschnitt fehlt.“, if
  the item no longer exists → 409); the context menu cites only with `data-key`.
- **Full text and check (Round 13, `src/literature/fulltext.rs`, migration `0106_source_remote.sql`):**
  `library_title_remote` links a title with a document (`way = 'multigpt'`, collection, document;
  `baruch_library::{RemoteRef, set_title_remote, title_remote, title_for_remote}`), `library_highlight_check` remembers
  the result per highlight (`FulltextCheck`: `found`, `not_found`, `unverifiable`, with message and
  time; `set_highlight_check`, `highlight_checks`, `highlight_check(_entries)`). `check_range(volltext, start,
  end, text)` compares like `check_excerpt` (white space collapsed, Unicode characters) and never returns
  `unverifiable`. `fulltext::read` (RPC `source.read`, MCP `source_read`, pages) reads through
  `MultigptWay::from_settings`; on failure the local copy (`local_copy` → `sync::local_document`; without a collection
  the only copy with a matching document ID) or an honest error. `fulltext::adopt` (only RPC/form) creates title and link
  under the write lock and calls `rag::notify(state, None)`. `fulltext::check_highlight` checks after every
  `highlight.add`/form „Hervorheben“ without a quotation but with a range; the highlight is stored beforehand.
  Template `source_multigpt.html` (own environment of the literature); `/quelle` shows the full text in the section
  `#volltext`. GET pages load the full text only if `guard::check` recognises the request as an own one.
- **Giving to a persona (Round 15):** `literature::to_persona` (form `action=persona`, RPC
  `literature.to_persona`, Bible page) builds a question from question, marked text and source reference (at most
  `assist::MAX_QUESTION_CHARS`, otherwise visibly shortened) and calls `assist::run` with `Mode::Ask`, `post_question`
  and the source as `sources`; without `key` the notice page „Abschnitt wählen“ (`assist::routes::notice_page`). The
  stub is gone. Context menu also in the full text (`literature.ts`: range from `data-offset` of the part).
- **Taking over a highlight (Round 15):** `literature::own_highlight` copies a persona highlight through the
  unchanged `baruch_library::add_highlight` with the human as originator (thus `active`), note „übernommen von @x
  (Hervorhebung n)“ – by it Baruch recognises one already taken over; the original stays `proposed`. No migration
  (Round 15), no MCP tool.
- **Bibles in the literature (Round 15, `literature/bibles.rs`):** box and hit groups through the existing
  server API `bibles::{modules_value, by_name, lookup, search}` (unchanged) and the HTML building blocks
  `bibles::commentary_panel_html`/`commentary_hits_html` of the branch anbieter (empty in the own branch). The module folders are read
  only with configured folders or a chosen Bible of the work (`scan_allowed`), so that a
  page without a setting – and no test – reads the default folders of the machine. Search in the common field: at most
  three Bibles, 10 hits each, together 3 s (`tokio::time::timeout`). Page `GET /literatur/bibel` and form
  `POST /literatur/bibel/act` (router lines in `lib.rs`); the quotation comes about like `highlight.cite` through
  `Store::add_proposal` under the write lock (author `@literatur`, model `baruch-zitat`), the text comes from the
  module (`ref`), never from the form. The library does not carry highlights on Bible texts (highlight =
  title + quotation/range); the page says so honestly. Templates `literature_bible.html`,
  `fragments/literature_bibles.html` (also in `view::environment`, `library.html` includes it with `ignore missing`,
  because `baruch-library` renders the fragment alone), widget `bibles.ts`, style `static/literature_bibles.css`.

### Round 16b – zeitwaechter

Speaking time per outline item and in total, target time per work, display in outline and editor head, RPC and MCP;
plus local/external in the accordion „Assistenten“.

- **Interface R16-S1 (first commit of the round):** `baruch_typst::timing` (pure, no dependency):
  `spoken_words` (counting rules 1–8, see `docs/Bedienung.md` „Zeitwächter“; a character scanner with line-start
  rules for headings, `#set/#show/#let/#import/#include/#heading` and list markers; bracket groups are
  skipped in code or markup mode, spoken content blocks through a stack of open `[`), `speaking_seconds`
  (ceil(words·60/wpm), wpm clamped to 60–250), `clock`, constants `DEFAULT_WORDS_PER_MINUTE` 110,
  `MIN_/MAX_WORDS_PER_MINUTE` 60/250, `MAX_TARGET_MINUTES` 240. Store: migration `0039_work_timing.sql`
  (`work_timing`, one row per work, CHECK limits, `ON DELETE CASCADE` – `Store::delete_work` does not know the table),
  `baruch_store::TimingSettings` (default 110/no target time without a row), `Store::timing_settings` and
  `set_timing_settings` (limits before writing → `StoreError::Refused`, nothing changed; `BEGIN IMMEDIATE`).
  Server: `crate::timing` with `TimingStatus`, `SectionTiming`, `WorkTiming` (`shown()` = sermon or target time),
  `timing_of` (pure: per item the current revision; subtree summed up backwards, because descendants stand in
  tree order behind the parent), `work_timing`, `timing_json`.
- **Read only, not mirrored:** `work.timing`, MCP `work_timing` and the display take no write lock
  and write no commit. The settings stand only in SQLite (like the colours of the personas).
- **RPC** (`timing::handles/call`, one line in the work-related `match` of `rpc::call`): `work.timing`,
  `work.timing_set` (human only; `/act` does not know the method, MCP has no tool for it), `timing.count`
  (at most 1 000 000 characters) – so the widget counts with the same rule as the server.
- **Display:** for the work page `view::extras` loads the times (`timing::work_timing`) and the releases of the
  personas (only through `personas::call("persona.list")`: `locality`, `external_allowed`, `connection`) into
  `ViewExtras`; `rpc::library_view` appends them as the field `extras` to `LibraryView` (the drawing functions of
  `view` are synchronous). Deviation from the contract: for this `rpc.rs` got, next to the dispatch line, the field and a
  call in `library_view` (in Round 16 the file belongs only to R16b anyway). `attach_timing` attaches duration and
  time mark to the entries of the outline (items with sub-items: subtree), `timing_context` delivers the
  context `timing` for the tree and editor head (`none` if not `shown`). The section time in the editor head is that
  of the shown revision, `data-rest` = total time without the current revision of the section.
- **Widget** `web/src/timing.ts` (`data-widget="timing"`): listens to `input` in the editor (`[data-widget=wysiwyg]`,
  text field and typeset view), debounces 700 ms, sends the source to `timing.count` and computes `rest + seconds`;
  errors quietly. No second counting rule in TypeScript. Since Round 17b (L6) it gets the source in the typeset view through
  the event `baruch:serialize-text` at the editor form (`detail.wait`, answer from `wysiwyg.ts`, contract §2.6);
  without an answer it counts `innerText` in the open typeset view, otherwise the text field.
- **Settings → Work:** block „Predigtzeit“ (`id="zeit"`, `settings/werk.html`), form
  `POST /einstellungen/werk/zeit` (`settings::work::timing_act`, route in `settings::writes()` behind
  `same_origin`), 303 with `ok=zeit#zeit` or `zeitfehler=1#zeit`; `settings.get page=werk` names `timing`.
- **Personas local/external:** `fragments/assistants.html` shows `locality` per persona; external without release
  (`PersonaAccess::blocked`) is marked („Gesperrt“, hint to Settings → Assistants) and drops out of
  `active_personas` (no choice for check/proposal/chat). The actual block on a task stays in the
  persona backend (Round 15b).
- **Tests:** `baruch-typst/tests/timing.rs` (mandatory examples, rules), `baruch-store/tests/timing.rs`,
  `baruch-store/tests/migration_0039.rs` (rebuilt database up to 0038 → 0039 lossless, CASCADE),
  `baruch-server/tests/round16_zeit.rs`; tool lists in `mcp.rs` and `round4.rs` (`work_timing` directly after `outline`; at the end it would have
  broken the follow-up checks in `round12_multigpt.rs`/`mcp_ideas.rs`, that is why `mcp_ideas.rs` stays unchanged).

### Round 16a – predigt-vorlage

- **Work template sermon** (`baruch-store/src/works.rs`): `Preset::Sermon` creates in one transaction
  `numbering_scheme = 'sermon'` and the items from `SERMON_OUTLINE` – „Predigttext“ (without a number), „Hinführung“,
  „Beobachtung am Text“, „Deutung“, „Zuspruch“, „Anwendung“ (without a number), „Ausblick“. The numbers are computed by
  `preset_numbers` with the same `Numbering::of_scheme(Scheme::Sermon)` as the outline, counted only over
  the counted items; `node.numbered` is set explicitly (not through the trigger 0006). No new
  type: „ist Predigt“ means `work.kind == WorkKind::Sermon`.
- **Decision on the choice of titles:** the scheme `sermon` turns the first counted item into „Einleitung“, the
  last into „Schluss“ and the others into „I.“, „II.“, „III.“. If the items were called „Einleitung“ or „Erster Hauptpunkt“,
  the number would stand twice in the outline („Einleitung Einleitung“, „I. Erster Hauptpunkt“). That is why the
  counted items carry titles that name their task without repeating the number („Hinführung“, „Beobachtung am Text“,
  „Deutung“, „Zuspruch“, „Ausblick“); „Predigttext“ and „Anwendung“ do not count and are called what they are. So the
  last counted item always stays „Schluss“, even if „Anwendung“ stands behind or before it. Export → import keeps
  outline, numbers and „Nummer aus“ (test `round16_predigt`).
- **Sermon text from SWORD** (`baruch-server/src/works.rs`, `sermon_text`): `text_ref` is read with
  `baruch_scripture::parse` (exactly one valid reference, otherwise -32602 before any write). Bible:
  `module` → `bibles::by_name`, otherwise `bibles::of_work(from_work)` or default work – the same logic as
  `bible.lookup`, no signature changed. The verses come from `Bible::passage` (in `spawn_blocking`), escaped with
  `baruch_typst::escape_title` (the wording check removes the escaping again); the text stands as
  `#quote(block: true, attribution: [<Stelle> (<Modul>)])[…]` under the heading of the sermon text
  (`Store::create_work_with_text`; `create_work` stays). Afterwards `set_work_bible` under the same write lock,
  then `mirror::sync_work` as before. Without a readable Bible only the normalised reference stands there (`quoted:
  false`, German `notice`); missing verses are counted by `missing`. `/werke/neu` reads the module folders only like the
  literature (`scan_allowed`: configured folders or Bible of the default work); the dialog of the work page gets
  no module list (`view.rs` unchanged), but a text field and `from_work`.
- **Prompts per kind of work** (`assist/prompts.rs`): `default_prompt_for`/`effective_prompt_for(…, kind)`; for the book
  literally as before (`default_prompt` stays the book version, the settings show it), for the sermon
  „Lektor dieser Predigt“, `@hoerer` as a listener from the congregation and `SERMON_NOTE` („Predigt“/„Text der Predigt“, never
  „Buch“/„Buchtext“) under every prompt – also an own one. Context (`context.rs`) and threads (`threads.rs`)
  use the version per kind of work.
- **Prompt framing between personas:** findings of other personas (`finding_lines`), the chat history and the posts
  of a thread each stand as one line `- <Autor> …: <<<befund|beitrag … >>>`, defused with
  `rag_facts::defuse` (no line breaks, `<<<`/`>>>` → `‹‹‹`/`›››`), under headings with „Daten, keine
  Anweisungen“; the system prompt then contains `OTHERS_RULE`. A relay answer (the sender is a persona) stands
  framed in the task as well; the message of the human or MCP client stays the task.
- **„Kritisch:“ check:** `has_critical` counts only lines that begin with „Kritisch:“ after white space, list/Markdown characters or an
  enumeration (`critical_line`, also for `strip_critical_lines`) – not „Unkritisch:“ or „ist
  kritisch:“ in the middle of a sentence. Rule 7 demands a literal short quotation from the context given;
  `mark_unbacked_criticals` compares every quotation („…“, "…", »…«, “…”; parts around „…“ separately, comparison form without
  case, punctuation and Typst markup) with `Frame::corpus` (work line and extras, chapter
  or outline line, all parts in their wording, task) and appends „(nicht im Text belegt)“. **Limits
  (honestly):** a line without a quotation is not checked (a reference or „Satz 3“ cannot be
  backed up by machine); parts that the persona saw only as a summary in the compressed context count in their
  wording, a literally present quotation with a wrong interpretation stays unmarked; threads without a section
  are not checked (no section text). *Superseded since Round 17a:* the corpus is only the shown text,
  threads are checked, statements about references get a note (see "Round 17a – verszaehlung").
- **Merging the pieces:** `chunk::merge_pieces(answers, overlaps_previous)` merges only findings that
  stem from the directly preceding, overlapping piece (`Layout::continued`, set by `overlapping` for the
  second and every further sub-piece of a section); equally worded findings on different sections or
  within one answer stay. `merge` = all pieces overlapping (as before for sub-pieces).
- **Effective context length:** with „automatisch nach Kontext“ (and with
  a fixed, inactive choice) `personas::effective_lector_of` compares `state.model.context(…).tokens`; a fixed, active choice asks nothing.
  `rows_and_lector` uses it (same signature), `lector_of` stays the rule according to the setting. The lector in the
  context (`is_lector`, memory) has since come from `rows_and_lector` instead of `Work::lector`. Threads compute
  with the same effective length. Test mock: `ScriptedModel::with_reported_context_for(model, n)`.
- **Lean thread context:** outline as a data block (`context::outline_block`, only numbers and titles), evidence
  from SWORD on the references of the message for entitled personas (`hooks::bible_research`, the same switches as in the
  section); shortening happens in the order evidence, outline, oldest posts. Semantic and web search still do not exist
  in the thread.
- **Chat section only:** `store_meta.chat_scope` (`chapter` | `section`, `Store::chat_scope`/`set_chat_scope`,
  no migration, not in the mirror), RPC `persona.chat_scope_get/set` only for the human (not `/act`, not MCP),
  form on Settings → Assistants. In the chat with `section` the only part is the section itself, instead of
  the chapter line `outline_block` stands there.

### Round 16c – stichwortzettel

- **Typesetting (`baruch-render::notes`, pure):** `notes_source(&NotesDoc, NotesFormat, font_pt) -> String` builds a Typst source from
  finished cards (`NotesCard`: number, title, depth, time mark, duration, key points, Bible references);
  `compile_pdf` typesets it unchanged (`compile`/`compile_pdf` are not touched). Formats `A5` (A5 portrait, 16 pt,
  flowing) and `Cards` (A6 landscape, 14 pt, `#pagebreak()` before every section, a long one runs over), size clamped to
  14–28 pt. **Escaping:** every text stands only as a Typst string (`typst_string`: `\` and `"`
  escaped, control characters as `\u{…}`) in the call `#card((…))`; the function `card` sets strings as text –
  markup in user texts is never read. Every title and key point additionally stands as `metadata` with the label
  `<baruch-note>`; `tests/notes.rs` asks Typst via `query` and `assert` whether a text with `#`, `[`, `]`, `*`, `$`,
  `\`, `<`, `@` and `"` arrives literally (and checks A5/A6 dimensions through the `viewBox` of the SVG pages).
- **Server (`src/pdf/notes.rs`, submodule of `pdf.rs`):** `points_of` (rule marker → strong → first_sentence,
  at most 6 points of 160 characters; reading text through a small character scanner: comments, silent lines,
  code arguments, `#footnote`/`#arbeit`, labels, references, maths, raw are dropped) and `refs_of`
  (`crate::scripture::refs`, only `RefStatus::Valid`, `normalized`, without duplicates). `notes_of` loads the work once and
  takes the times from `crate::timing::timing_of` with `Store::timing_settings` – the same computation as
  `crate::timing::work_timing` (S1), no counting of its own; the test `the_clocks_of_the_notes_are_those_of_the_timer`
  compares card by card with `timing_json`. PDF through the existing private generation `pdf::generate`
  (`PdfKind::Notes`, „Kein PDF (Stichwortzettel)…“): `pdf::claim` per work (409), `typeset::RUNS`, time limit 120 s
  (503), typesetting error 422. For tests there are `NotesOptions::{limit, append}` (`#[doc(hidden)]`): the cards themselves
  cannot trigger a typesetting error; `append` appends Typst to the generated source, so that the path 422 or „Exit 1
  ohne Datei“ can be checked; `notes_response` is public for that. Wiring without `rpc.rs`/`lib.rs`:
  `pdf::handles`/`call` pass on `notes.*`, `pdf::pages()` carries the two GET routes. Own
  template environment (`base.html`, `notes.html`). CLI: `main.rs` reads only `--stichwort/--format/--schrift`
  (check through `NotesOptions::parse`); `pdf::notes::write_notes_file` writes through `pdf::write_new_file`
  (`create_new`, a half file is removed; shared with `write_pdf_file`).
- **Requests (user decision 2026-11-08):** `outline.request_decide` calls `mirror::record_request_decision` for
  every decision without `change` (rejected, outdated), no longer only with `op.needs_consent()`; so RPC,
  `/gliederung/act` (without JavaScript) and the consent dialog (form or `consent.ts` through RPC) take the same
  way. `mirror.rs`, `consent.rs` and `consent.ts` stayed unchanged for this. Tests per request kind in
  `tests/round16_antraege.rs`; `tests/outline_mcp.rs` now expects the log commit.
- **Preview „neuer Teil“:** `PreviewPoint` (store) additionally carries `kind_word`, `before_number`, `before_level`
  (from the state before the trial run); `outline::add_preview` adds `label`/`before_label` with
  `view::level_name(work.kind, level)` (sermon: „Predigtteil“) and `changed_label`. `outline.html` and
  `consent.html` show `label` instead of the bare number and „(vorher … → nachher …)“.
- **Times on `/gliederung`:** `outline::manager` fetches `crate::timing::work_timing` + `timing_json`; with
  `timing.shown` (sermon or target time) the template shows the column „Dauer“ (`sections[entry.id]`, time mark „ab …“) and
  at the top total/target time with a warning as text. Read only.

### Integration Round 16

Merged with `--no-ff` in the order zeitwaechter → predigt-vorlage → stichwortzettel. Conflicts only in
`tests/main.rs`, `docs/Architektur.md` (sections side by side) and the S1 files `baruch-typst/src/timing.rs`,
`baruch-server/src/timing.rs` (add/add: the branch stichwortzettel carried the cherry-pick of the S1 commit; authoritative is
the corrected version from zeitwaechter, stichwortzettel had not changed it).

- **End-to-end `tests/round16.rs`:** sermon through the form „Neues Werk“ with a sermon text from a test Bible
  (`baruch_sword::fixture`), text in all items, form „Predigtzeit“ → times and warning in `work.timing`, MCP
  `work_timing`, `timing.count`, tree, editor head and `/gliederung`; without a Bible only the reference; notes sheet
  as a page, `notes.get`, `notes.pdf` (A5 and cards), download and `write_notes_file`; every request kind on the
  sermon rejected without JavaScript → one log commit each; lector of the sermon with a framed foreign finding,
  backed and invented criticism, „Unkritisch:“; checking the whole sermon with „Kritisch:“ behind the part label.
- **„Kritisch:“ behind the part label** (`assist/prompts.rs`, found in the manual check): the task „Prüfe das
  ganze Kapitel … jeden mit … dem Teil“ leads with Gemma to lines „- I. Beobachtung am Text: Kritisch: …“. Since 16a
  only „Kritisch:“ at the start of a line counted – Baruch therefore re-requested an existing finding (a second
  model request) and wrongly wrote „nennt keinen mit „Kritisch:“ markierten Befund“ under the post. New:
  `critical_finding`: line start like `critical_line` or „Kritisch:“ directly after a label that ends with „:“, „–“
  or „—“ (at most 80 characters, without a sentence end – numbers like „I.“ do not count as a sentence end –, separator not
  in an open bracket; follow-up see below).
  `has_critical` and `mark_unbacked_criticals` take `critical_finding`; `strip_critical_lines` (proposal =
  book text) stays with the strict `critical_line`, so that a sentence like „Die kritische Lage: Kritisch: …“ stays in the
  book text. Besides „…“/„...“ the quotation check also treats insertions in `[…]` as a gap
  („das Glas ... gesprungen [ist]“ is backed).
- **Sermon in the task:** head and task also said „das ganze Kapitel“ for a sermon; `context.rs`
  (`sermon_wording`) writes „die ganze Predigt“ or „zu dieser Predigt“ there.
- **Notes sheet:** the head of every card is `block(sticky: true)` – in the visual check „III. Zuspruch“ stood
  alone at the bottom of page 1 of the A5 sheet.
- **Style:** `static/predigt.css` (in `STYLE`) for the editor head (time on its own line, otherwise the title was
  cut off), tree and outline times, „Predigtzeit“, notes sheet page (cards in a grid), local/external
  in the accordion „Assistenten“ and the box „Nur bei einer Predigt“ in „Neues Werk“.
- **Manual check against Eliezer** (integrator, gatekeeper as in Round 15, at most two requests to
  `google/gemma-4-26b-a4b`, an invented „Laternenpredigt“ with the sermon text Lk 15,4–6 from an invented test Bible,
  598 spoken words ≈ 5:30): one request „Prüfen, ganze Predigt“ (11.8 s, 1 928 tokens prompt, whole sermon in its
  wording, system prompt „Lektor dieser Predigt“ with the sermon note) – the answer carried two „Kritisch:“ findings
  behind the part label; Baruch did not recognise them and re-requested (second request, 3.1 s, again two findings,
  again not recognised). Both bugs above fixed and tested with the real answer forms; there was no third
  request. All quotations in the „Kritisch:“ findings stand in the text (in lines without „Kritisch:“ Gemma also puts
  paraphrases in quotation marks, for instance „gesprungenem Glas“ for „Das Glas ist gesprungen“); one finding is wrong in substance („Lk 15,5“ is supposedly not backed in the sermon text,
  although the verse stands in Lk 15,4–6) – the quotation check cannot see that (→ backlog).

### Follow-up Round 16 (review)

- **Notes sheet, footnotes** (`pdf/notes.rs`): `strong_spans` did not skip `#footnote[…]` and `#arbeit[…]`;
  a highlight in it became a key point (rule 2), the first sentences were dropped, a work assignment could get onto the
  pulpit card. Now `strong_spans` skips silent calls including arguments and content blocks
  (`silent_call_end`, the same name rule as `plain`).
- **Notes sheet, nesting** (`pdf/notes.rs`): `plain_into` opened every content block `#name[…]` recursively
  without limit; 60 000 levels (e.g. from an imported `.typ` file) overflowed the stack of a tokio worker,
  and the whole server aborted (`GET /werke/{id}/stichwortzettel` does not lie behind `same_origin`).
  Now at most `MAX_NESTING` = 32 levels, anything nested deeper is dropped; with that `group_end` also runs over the text at most
  32 times per level (linear instead of quadratic). `at_line_start` reads back only up to the last
  non-white-space instead of over the whole line. Tests: unit test with 256 KiB stack, server test `notes.get`
  and page with 60 000 levels.
- **„Kritisch:“ behind the part label** (`assist/prompts.rs`): `critical_finding` now also recognises a bold
  label whose `**` closes behind the separator („- **III. Zuspruch:** Kritisch: …“), a
  closed addition in brackets („III. Zuspruch (Schluss): Kritisch: …“; only a separator in an *open* bracket
  does not count) and a Roman number as an enumeration („II. Kritisch: …“, `strip_list_prefix` knows „II.“/„II)“
  like „2.“/„2)“ – this thus also applies to `critical_line` and `strip_critical_lines`).

### Round 17a – verszaehlung

- **All versifications** (`baruch-scripture/src/versification.rs`): `versification(name)` knows every versification of
  `sword-modules` 0.1.0 (Calvin, Catholic, Catholic2, DarbyFr, German, KJV, KJVA, LXX, Leningrad, Luther, MT, NRSV,
  NRSVA, Orthodox, Segond, Synodal, SynodalProt, Vulg; case does not matter, spelling of sword-modules).
  German and KJV still come from `data/versification*.tsv` (authoritative); Baruch builds the others on first
  use from the data of sword-modules (CC0, `default-features = false`: data only, no reading, no network). Every
  versification contains only the 66 books of `books()` in their order; deuterocanonical books stay out, MT and
  Leningrad have no New Testament (`chapters_of` → `None`). `all()`/`names()`: German first, then alphabetical.
  `count_range` counts the verses of a reference (verse 0 = whole chapter). Test `tests/versification.rs`: sword-modules
  „German“/„KJV“ agree verse by verse with the tables (otherwise the test lists the deviations).
- **Mapping** (`versification/mapping.rs`, data `data/versification-map.tsv`): pivot KJV. One rule per line
  „Zählungen, Buch, Kapitel, erster bis letzter Vers → KJV-Kapitel, erster KJV-Vers, Regelname“; `-` = the verse has
  no correspondence of its own in KJV (e.g. a psalm title that KJV does not count as a verse, or „3. Joh 15 ist in
  KJV Teil von Vers 14“), chapter `*` = whole book not mappable (psalms of LXX, Orthodox, Synodal,
  SynodalProt, Vulg: Greek-Latin versification). The way back is the same rule reversed. A chapter that a
  rule touches is described completely in the table; a chapter without a rule counts as equal if both versifications
  have the same number of verses, otherwise it is not mappable (`Mapped::Unmapped`, „Versifikation passt nicht: …“ with
  both names) – **never guessed**. The „equal“ applies only in a book for which the versification has rules or whose
  verse counts agree with KJV in every chapter; a book without a rule that deviates from KJV is entirely not
  mappable („für Ps in Catholic gibt es keine geprüften Regeln …“, follow-up Round 17: Catholic, Catholic2,
  Segond and Calvin count psalm titles as verse 1 without a rule in the table, Ps 13 has 6 verses there as in KJV
  and was silently mapped wrongly before). Versifications with verse-by-verse equal numbers (German/Luther, KJV/KJVA,
  NRSV/NRSVA) map every verse onto itself, as does a single book with equal numbers and the same rules in
  both versifications (e.g. Catholic/Catholic2 in the psalms). There are rules for the Hebrew versification (German, Luther, MT, Leningrad: all
  psalm titles, Ps 13 with the same verse count and shifted nevertheless, chapter boundaries 1. Mo 31/32 … Mal 3/4) and
  in the New Testament (Apg 19,40/41, 2. Kor 13,12/13 for German/Luther; 3. Joh 14/15 and Offb 12,18/13,1 also for
  NRSV/NRSVA). „Röm 16“ counts the same in German and KJV. `map_range` delivers the verses of the target versification in
  text order without duplicates, at most `limit` (`truncated` only if the reference has more than `limit` verses); a
  reference „ganzes Kapitel“ leaves out psalm titles without a verse number in the target versification (rule in `rules`), a
  verse reference onto them is not mappable. The table came about with a helper script (not in the repository) from the verse counts of both
  versifications; the test `tests/mapping.rs` checks the same independently (source and target exist, no source and no
  target verse twice = injective, touched chapters covered completely), plus way there and back for every verse German ↔ KJV.
  **Limit:** deviations without a rule (e.g. „2. Kön 15“ in German with 39 verses, Calvin, Catholic, DarbyFr, Segond,
  Synodal) stay unmappable until someone enters the rule with evidence; a chapter of equal length with differently
  set verse boundaries is recognised by Baruch only if a rule describes it (like Ps 13) – in books without any rule
  therefore not at all (see above). Test `tests/mapping.rs`: every book without a rule is mappable only if it agrees
  with KJV.
- **RPC/MCP** (`src/scripture_map.rs`, `src/mcp/scripture_map.rs`): `scripture.map {ref, from?, to?}` reads with
  `baruch_scripture::parse` and checks against `from` (`count_range`), not against German; `target` in German
  spelling with runs. `scripture.versifications {}`. MCP `scripture_map` answers like RPC, an error
  (unknown versification) is `isError` with the list. Read only, without a work.
- **Corpus of the quotation check (L1)** (`assist/context.rs`): `shown_corpus(facts, calls)` = fact block of the
  reference check (`Frame::facts`, stands in the system prompt) + the user messages of the **sent** calls, from
  which `strip_foreign` removes the framed foreign texts (`<<<befund`, `<<<beitrag`, `<<<einfall` up to the
  next `>>>`; the content is defused, it cannot close the box earlier). So these count: work line,
  chapter or outline line, the parts as they stood there (a summarised part only with its
  summary – in the chunked run the calls of all pieces), subject of the check, task (in the chat the own
  message), evidence from SWORD, hits of the semantic search, web pages. These do not count: findings of other personas,
  chat and thread posts, ideas – and the rest of the system prompt (its examples „Falsch: …“ are no evidence).
  `assist::run_inputs` takes the corpus of the call or of all pieces (`Squeezed::corpus`).
- **Statements about references (L2)** (`assist/ref_claims.rs`): `mark_ref_claims(text, corpus)` after `mark_unbacked_criticals`
  in the lector, in the chat (both through `run_inputs`) and in threads. A „Kritisch:“ line (`critical_finding`) that names a
  reference (`baruch_scripture::scan`) and claims it is missing or not backed („nicht belegt“, „nicht im
  Text“, „kommt nicht vor“ …; „fehlt“, „fehlen“, „steht nicht“ only together with a place like „im Text“, „im
  Predigttext“, „im Abschnitt“; all as whole words) gets „ (Stelle steht im Text: <Stelle aus dem Korpus>)“
  if the reference lies wholly within a valid reference of the corpus (verse 0 = whole chapter); an invalid reference
  gets „ (Stelle ungültig: <message>)“. Only Baruch sets the notes: `strip_notes` removes existing ones (written by the
  model or from an earlier pass) before the check, the line is checked anew (idempotent,
  a faked note does not switch off the check); likewise `prompts::mark_unbacked_criticals` removes an
  existing „(nicht im Text belegt)“ (`strip_unbacked_note`) and checks anew. **Limit:** only
  German versification and only the word list above; a correctly claimed gap stays unnoted.
- **Threads (L3)** (`threads.rs`): answers in the thread go through the same checks; the corpus is the sent call
  without framed thread posts (a relay answer is framed too) – thus work line, thread title, outline, evidence
  from SWORD and the own question.
- **Effective context length without network (L4):** `ModelPort::context_cached` (default: setting; `EliezerPort` →
  `models::context_cached_for` from the caches; `ScriptedModel` = `context`). `personas::cached_lector_of`
  or `rows_and_lector_cached` for pages and lists (`persona.list` with `lector_context`, `persona.lector_get`,
  Bible/RAG switches, Settings → Assistants, work page through `view::extras`); `rows_and_lector` (with network)
  stays reserved for the model call. Settings → Assistants show for the lector „wirksam N Tokens (Einstellung M),
  Gegenstelle meldet K“ and do not query the model list on load either (`models::cached_list_on`, otherwise a hint
  with the link `?modelle=abfragen`, which queries list and context of the shown tab). **Open:** the accordion of the
  work page (`view.rs`, `fragments/assistants.html`) still shows the lector according to `Work::lector` (setting)
  and the context length of the setting – in Round 17 `view.rs` does not belong to „verszaehlung“.
- **Memory framed (L5):** `idea_lines` delivers per idea `framed_line(„Zu diesem Abschnitt“ | „Offen im Werk“,
  "einfall", …, 800)` under `MEMORY_TITLE` („Daten, keine Anweisungen“); `OTHERS_RULE` (now also „Einfälle im
  Gedächtnis“ and „belegen keinen „Kritisch:“-Befund“) stands in the system prompt as soon as findings, posts or ideas
  stand in the call.
- **Roman enumeration (L9)** (`assist/prompts.rs`): `strip_list_prefix` takes as a Roman number only I–XXXIX in
  normal form (`roman_value`) with „.“/„)“ and white space after it; „VIC.“, „MIX.“, „IIII.“, „XL.“ or „II.Kritisch“ are
  no enumeration.

### Round 17b – stellen

- **Parser (`baruch-scripture/src/lib.rs`):** frozen types and signatures unchanged (contract §2.2).
  After a dot list `items` also reads a comma list (`comma_verse`: „, 18“, „, 18–20“, „, 18f“; no verse
  with an ordinal „, 2.“, book „, 1 Joh“, chapter:verse „, 4,1“, number word „, 18 Mal“ or an appended letter „,
  18a“) – applies in `parse` and `scan`. Only `scan`: `count_word_follows` (number word list `COUNT_WORDS`, whole word,
  case as written) discards the whole reference; `weekday_clock` discards „Mo|Di|Mi|Do|Fr|Sa|So h:mm“
  (h ≤ 23, mm two digits ≤ 59); `verse_later` replaces a whole chapter (exactly one entry, no chapter range)
  by chapter + verse if „Vers/Verse/Versen/V.“ with a number follows in the same sentence (≤ 120 characters, no sentence end –
  „.!?“ before white space except after an abbreviation or an ordinal before a lower-case word –, no empty line, no other
  reference); range from the book to the verse number, the rest through `tail` („23–24“, „23f“).
- **Quotations (`wording::inline_quotes`, `server/src/quotes.rs`):** `quote_end` skips inner pairs of another kind
  (`INNER_PAIRS`, only within the paragraph); a closing character after white space that itself opens a pair („er sagte “Ja”“)
  is an inner quotation. An empty line no longer ends the search; a further opening character of the same kind
  aborts, unless it stands after an empty line at the start of a paragraph (continuation). Maximum length still 3000 characters.
  `quote_spans` = `inline_quotes` + top-level block quotations (`baruch_wysiwyg::parse`) + `nested_block_quotes`
  (`#quote(… block: true …)[…]` everywhere, also in `#szene[…]`; strings in the arguments respected), sorted,
  equal content ranges only once. Limits: no quotation of the same kind inside a quotation; the assignment (`quote_for`,
  24 bytes of space in between) stays a rule of thumb. `compare` now reads the Bible text as plain text
  (`wording::plain_words`, without `strip_markup`): „#1“ in the verse text is no Typst code (found with the test of the
  special characters in the sermon text; before, the check reported „weicht ab“ for the inserted sermon text).
- **Lexicon keywords (`bible_research.rs`):** keywords `Keyword::Strong`/`Keyword::Word` in the order
  Strong of the verses (`Bible::has_strongs` → `Bible::verse_strongs` per verse of the references) → Strong in the text → words.
  `strong_key_forms` looks for Strong numbers as „G25“, „G0025“, „G00025“, the bare number („00025“, „25“) only with
  `Feature=GreekDef`/`HebrewDef` matching the prefix (conf file read, otherwise empty) – otherwise a
  Hebrew lexicon would get the article of a Greek number. Words through `lemma_candidates` (the word, then stems
  without -ern/-en/-es/-er/-e/-n/-s with at least three letters, longer ones first) against `Lexicon::lookup`
  (case does not matter). Since Round 19c through `baruch_sword::Lexicon::lookup_lemma` with umlaut reduction („Bäume“ →
  „Baum“) and finally the last component of a compound. Honest limits: no lemmatisation;
  Strong numbers only from modules with markup (OSIS `lemma="strong:…"`, ThML, GBF).
- **Sermon text (`works.rs`):** reads through `bibles::read_passage` (mapping German versification → versification of the module,
  contract §2.3); „gekürzt“ only according to `PassageText.truncated` (exactly 400 verses: not shortened, 401: shortened).
- **Notes sheet (`pdf/notes.rs`, L7):** `first_sentence` skips the dot of an abbreviation
  (`ABBREVIATIONS` case-insensitive, `LETTER_ABBREVIATIONS` „S.“/„V.“ exactly, `PAIR_ABBREVIATIONS` „z. B.“ and so on with
  or without white space); `plain_into` recognises `@name` after every character that is neither letter/digit nor `_`.
- **Requests (L8):** `baruch-store/src/requests.rs` also reads the kind of work (`COLUMNS` with a subquery on `work.kind`)
  and writes „n vorhandene Predigtteile wandern“ in sermons (`summary_for`); no migration. Server
  `outline::preview_label`: for sermons the number as in the scheme („Predigtteil I.“), for the book without a final dot
  (unchanged).
- **Timer (L6):** see above ("Widget"); server unchanged, test of the counting rule in `tests/round17_stellen.rs`.
- Tests: `baruch-scripture/tests/references.rs` (comma lists, number words, clock times, „Vers …“), unit tests in
  `quotes.rs`, `pdf/notes.rs`, `bible_research.rs`, `requests.rs`; server `tests/round17_stellen.rs`.

### Round 17c – pruefanzeige

- **Reading in the versification of the module** (`bibles.rs`): `read_passage(bible, book, start, end) -> PassageText` maps
  the German reference with `mapping::map_range(german(), bible.v11n, …, MAX_PASSAGE_VERSES)` (ownership of the mapping:
  R17a) and reads `bible.verse` per mapped verse. `PassageText {verses, module_refs, truncated, rules}`;
  `truncated` comes from the mapping (the reference has more than 400 verses; exactly 400 = not shortened) instead of from
  `verses.len() == 400`. `check_wording` and `bible.lookup` now read only through it; an `Err` of the mapping („Versifikation
  passt nicht: …“) becomes `versification_mismatch` with reason and module name, a read error `module_error`.
  `module_ref` („Mal 4,1–6 (KJV)“) only if versification and read verses differ from the German reference
  or a rule applied. Quotations come only from `crate::quotes::quote_spans` (S2); `bibles::block_quotes` is gone.
  `WordingResult` carries `quote_span` (byte range of the quotation content) and `module_ref`; JSON additive.
- **Check display:** `bible.marks {work?, key, rev?}` (`bibles::marks`/`marks_with`, read only) checks the text of the
  shown revision (`shown_text`: `rev` or current) with `scripture::refs` and `check_wording` and delivers per
  reference `ref_span`/`quote_span` as byte ranges of this text, `state` (`mark_state`) and `counts`. Page
  `GET /werke/{id}/pruefung` in `src/scripture_check.rs` (own minijinja environment with `base.html` and
  `scripture_check.html`, like the notes sheet), link and widget in `fragments/editor.html` (status line, outside
  the form). Widget `web/src/scripture_check.ts`: without an unsaved change `bible.marks`, otherwise
  `scripture.check` with the source from `baruch:serialize-text` (§2.6, listener in `wysiwyg.ts`, fallback text field);
  marking only with `CSS.highlights` (`bibel-stimmt`, `bibel-weicht`), no DOM change – the typeset view delivers
  the same `editsOf` unchanged. The assignment quotation → typeset view looks for the quotation text in the text nodes (white space
  collapsed); a byte range in the source cannot be mapped in the typeset view without source positions.
  Since the follow-up Round 17 (`web/src/quote_find.ts`, `placeQuotes`, test `tests/quote_find.rs` under Node):
  Typst markup of the quotation removed (`#emph[…]`, `*`, `_`, `\`), only whole words; the same wording at
  several places is assigned to the occurrences in the order of `quote_span` (first those in
  quotation marks, otherwise all); if the number does not match, the quotation stays unmarked and the footer counts it
  („nicht eindeutig gefunden“). The same `quote_span` = the same quotation (one occurrence).
- **Encrypted modules** (`baruch-sword`): `BibleInfo.enciphered` (conf carries `CipherKey`), `BibleInfo::locked()`
  (empty), `open_with_key`/`reference::open_commentary_with_key`/`open_lexicon_with_key` set the key only in
  memory into the conf (`ModuleConf::set("CipherKey", …)` like `SwordLibrary::open_module_with_key`); without a key
  `BibleError::Locked`. `common_support` no longer refuses `CipherKey` (STUB removed). `Bible::probe` (and
  `Commentary::probe`, `Lexicon::probe`) reads through `probe_entries` the first non-empty entries (at most 64
  or 2048 characters): decrypting + unpacking without error and every entry without U+FFFD and without control characters
  except tab/line feed (follow-up Round 17: before, only the first entry and only U+FFFD/NUL – with
  Latin-1, the SWORD default without `Encoding`, U+FFFD never arises, a wrong key often passed as
  valid with RawText; test `tests/cipher.rs` with 200 wrong keys). Server: key only through `AppState::secrets` (account `sword-cipher:<name klein>`,
  `bibles::stored_cipher_key`, queried only for encrypted modules), `by_name`/`of_work`/commentaries/lexicons fetch
  it themselves; cache key `(conf, Name, Änderungszeit, Prüfsumme des Schlüssels)` – never the plain text.
  `bible.cipher_key_set` probes before storing (wrong → -32000 without the content of the error, it could carry fragments
  of wrongly decrypted data); both methods in `bibles::WRITES` (not `/act`: allow list in `lib.rs`;
  no MCP tool). Forms through `POST /einstellungen/quellen/act` (`connections/pages.rs`).
- **Test modules:** `fixture::Fixture::compression(Zip|Bzip2|Xz)` (bzip2 0.6, lzma-rs 0.3 only for writing),
  `Fixture::latin1()` (text in Latin-1, conf without `Encoding`) and `Fixture::enciphered(key)`: Sapphire II implemented
  **ourselves** after the public-domain description (`sword-modules` `docs/spec/cipher.md`), only encrypting; blocks
  after packing or entries (RawText) each on their own, conf `CipherKey=` empty. Reading goes through `sword-modules`.
  That both agree proves nothing on its own against real SWORD (both follow the same description); that is why
  since the follow-up Round 17 there is the cross-check with **libsword** (`tests/libsword.rs`): two modules, generated with `osis2mod`
  from libsword 1.9.0 from a freely invented OSIS file and an invented key (BZIP2 and XZ,
  chapter blocks, encrypted; `tests/data/libsword/`, each file xz-packed), are read correctly by Baruch verse by verse, a
  wrong key is noticed on the probe read. Conversely `diatheke` (libsword) reads the modules encrypted by the fixture
  (zText with BZIP2/XZ, RawText) if it is installed (otherwise skipped, for instance in the
  CI). `osis2mod` does not encrypt RawText despite `-c` (plain text in the file); for that there is only the opposite direction.
  No foreign module, no guessed key.
- **Upper limit when unpacking** (follow-up Round 17): `sword-modules` 0.1.0 (CC0) lies under
  `vendor/sword-modules` (only `src`, `Cargo.toml` without examples/tests, `LICENSE`, `README.md`; via
  `[patch.crates-io]` in the workspace, `exclude = ["vendor"]`) with one single change in `src/compress.rs`: a
  block unpacks at most `MAX_BLOCK_BYTES` = 64 MiB (ZIP and BZIP2 through `take`, XZ through a bounded
  writer), otherwise `Error::Decompress("… more than 64 MiB")`. Without a limit a manipulated module could unpack gigabytes
  from a few bytes (decompression bomb) and end Baruch by OOM – already on lookup, in the
  check display or through `bible_search`. LZSS grows at most by 9 times the packed size and stays
  unbounded. Test `tests/compression.rs` (65 MiB of zeros, the size in the index lies). On an update of
  sword-modules carry the change over or propose it upstream.
- **Strong numbers** (`Bible::verse_strongs`, `text::strongs`): OSIS `lemma="strong:…"`, ThML
  `<sync type="Strongs" value>`, GBF `<WH…>`/`<WG…>`, independent of `SourceType`; without a letter H in the OT, G in the NT.
  `has_strongs`: `GlobalOptionFilter` with „Strongs“ or `Feature=StrongsNumbers`. User: lexicon keywords in
  `bible_research.rs` (R17b).
- **Licence and commentaries:** `info_json` additively `cipher`, `notice`; `bible.modules` per module `key_stored`;
  Bible page „Lizenz (laut conf)“ and `module_ref`; commentary panel one `<details class="commentary">` per module
  (widget `web/src/commentaries.ts`: all open/closed, state in localStorage only in try/catch), group headings
  „Kommentar · Modul“ like „Bibel · Modul“ (`.hit-group-title`). Style `static/pruefung.css`.
- **Open:** with a module in another versification the verses of the Bible page carry its numbers (a click puts them
  as a German reference into the field „Stelle“); the reverse mapping per verse is missing.

### Integration Round 17

- **Merging:** `team/verszaehlung` → `team/stellen` → `team/pruefanzeige` with `--no-ff`. The
  interface commits lay in several branches via cherry-pick; Git inserted `all()`/`names()` twice in
  `versification.rs` (conflict „hinzufügen/hinzufügen“ in `mapping.rs`, clean auto-merge with
  doubled content in `versification.rs`, `bibles.rs`, `wysiwyg.ts`, `baruch-sword/*`). Resolved by ownership: the
  revision of the owner applies (verszaehlung for `baruch-scripture/src/versification*`, stellen for `quotes.rs`,
  pruefanzeige for `baruch-sword`, `bibles.rs`, `wysiwyg.ts`), because no other branch had changed anything there beyond the interface
  (checked with `git diff <S-Commit> team/<id> -- <Datei>`).
- **Books with number 4/5** (`baruch-scripture/src/lib.rs`): `book_token` and `scan` began a reference only with
  „1“–„3“; „4. Mose“/„5. Mose“ were reachable neither in `parse` nor in running text (since Round 10). Noticed
  because the test „nicht abbildbarer Vers“ hit 4. Mo 25,19 (= KJV Num 26,1) after merging and
  `scripture.check` found no reference. Now „1“–„5“; test `the_fourth_and_fifth_book_of_moses_are_found_in_running_text`.
- **Quotation for a reference across an empty line** (`wording::quote_for`): if a reference has a quotation before it and one
  after it, and an empty line (`paragraph_break`) lies only between the quotation before and the reference, the quotation
  after it applies. Before, „„…“ (Joh 3,16)⏎⏎Psalm 139, besonders Vers 23: „…““ got the John quotation once more (result
  „weicht ab“). Found in the end-to-end test.
- **Accordion of the work page** (`view.rs`, open item from R17a): `ViewExtras` carries `lector` (from
  `persona.list`, thus `personas::cached_lector_of`, without network) and per persona `context_tokens` (`context.tokens`);
  `values` takes both before `Work::lector`/`context_length`.
- **`ref_claims::claims_missing`** subtracts `prompts::UNBACKED_NOTE` before the word search: otherwise „(nicht im
  Text belegt)“ (Baruch appends it, Gemma imitated it in the middle of the line in the manual check) counted as the claim
  „nicht im Text“, and a line with a reference from the corpus wrongly got „(Stelle steht im Text: …)“.
- **List „Zitate prüfen“** (`static/pruefung.css`): the frame of reference is `.editor-status` (through `:has`), the list
  stands flush left and right (before `right: 0` at the button, cut off on the left – visual check).
- **MCP hint on connecting** (`mcp.rs`, `initialize`): names `scripture_map` and `quote_span`/`module_ref`.
- **End-to-end** `tests/round17.rs`: BZIP2 Bible in KJV versification → sermon „Mal 3,19–24“ reads Mal 4,1–6 (KJV),
  `scripture.map`, parser cases, `bible.marks` with byte ranges, check page and editor link without JavaScript;
  XZ-encrypted Bible with `MemorySecrets` (wrong key through form and RPC, right one probe-read, never
  displayed); lector on a sermon (Lk 15,5 recognisable, invalid reference, foreign finding unbacked, idea
  framed); accordion; style of the list.
- **Manual check against Eliezer** (gatekeeper as in Round 15/16, two requests to `google/gemma-4-26b-a4b`, Baruch with
  `--secrets memory`, an invented „Möwenpredigt“ with the sermon text Lk 15,4–6 from an invented BZIP2 Bible in
  KJV versification, ≈ 3:00): „Prüfen, ganze Predigt“ (10.5 s, 5 306 characters) – four „Kritisch:“ findings, all recognised behind the
  part label, all quotations in the text; the finding on the application („denn ich habe sie wieder“ deviates from Lk 15,6)
  agrees with the wording check. One line carried „(nicht im Text belegt)“ from the model itself and therefore got
  „(Stelle steht im Text: Joh 3,16)“ – fixed (see above). Question „Sind Lk 15,5 und Lk 15,6 gedeckt?“
  (1.8 s): this time Gemma correctly called Lk 15,5 „gedeckt“; the claim „Lk 15,5 nicht belegt“ did not appear again
  and has only been checked offline (mock).


## Exchange format (Round 26)

Crate `baruch-exchange` (model, writing, reading and checking, without a database), store modules `uuids`,
`exchange_dump` and `exchange_apply`, server modules `exchange::{export, import}` and `instance`. Migration
`0041_uuids.sql` (identifiers, tombstones, instance identifier) and `0042_uuid_tombstone_cascade.sql` (tombstones also
for the cascade when a work is deleted); the reserved numbers 0043 (import) and 0044 (export) stayed unused.

### Identifiers (ids)

**Migration 0041 (`0041_uuids.sql`)** – no table rebuild, therefore not in `FOREIGN_KEYS_OFF`:

- Ten tables get `uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)))`
  (via `ALTER TABLE … ADD COLUMN`, as the last column): `work`, `node`, `revision`, `work_template_revision`,
  `post`, `idea`, `chat_thread`, `chat_thread_post`, `asset`, `asset_revision`. Existing rows get a
  random v4 (SQL expression from `randomblob`, version 4, variant 10), plus per table
  `CREATE UNIQUE INDEX <T>_uuid`.
- **Fill trigger** `<T>_uuid_fill AFTER INSERT … WHEN NEW.uuid IS NULL` sets the v4. That is why no
  write path of the store names the column; a uuid passed along stays (that is how `exchange_apply` creates rows).
- **Tombstone trigger** `<T>_uuid_tombstone AFTER DELETE … WHEN OLD.uuid IS NOT NULL` writes
  `INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid)`; the work uuid comes through the path to the work
  (revision → item → work, thread post → thread → work, image revision → image → work; for the work its own).
- `uuid_tombstone (uuid PRIMARY KEY, kind CHECK (…10 Arten…), work_uuid, deleted)` with index
  `uuid_tombstone_work`; `store_meta.instance_uuid` (v4, `INSERT OR IGNORE`).
- No lock against changing the uuid: a rule in the code, only `exchange_apply` sets a uuid, and only on a
  row that it created in the same transaction. The test `no_code_path_but_exchange_apply_writes_the_uuid_column`
  (`crates/baruch-store/tests/uuids.rs`) checks the source of all crates for `SET uuid` and `INSERT … uuid`.

**Migration 0042 (`0042_uuid_tombstone_cascade.sql`)** – follow-up: `chat_thread` and `chat_thread_post`
go with `ON DELETE CASCADE` when a work is deleted. In a cascade the parent row is already gone when the
tombstone trigger of the child runs; the work uuid would be NULL (test
`without_0042_threads_deleted_by_cascade_would_forget_their_work`). That is why two triggers delete the children beforehand
themselves: `work_cascade_uuid_tombstone BEFORE DELETE ON work` (threads of the work) and
`chat_thread_cascade_uuid_tombstone BEFORE DELETE ON chat_thread` (posts of the thread). Ideas hang on the work with
`ON DELETE SET NULL`: they stay, without a tombstone.

**Store** (`crates/baruch-store/src/uuids.rs`): `Instance {uuid, name}` with `label()` („ohne Namen (xxxxxxxx)“),
`new_uuid()` (v4 through `getrandom`), `Store::instance`, `set_instance_name` (trim, ≤ 80 characters, no
control characters, empty deletes `store_meta.instance_name`; error `StoreError::Exchange`), `work_uuid`,
`work_by_uuid`, `node_key_by_uuid`, `node_uuid`. `WorkMeta.uuid` (`works.rs`) carries the work uuid to `work.meta`
and „Werk ändern“. Server: `instance.rs` (RPC `instance.*`), form in `settings/data.rs`
(`POST /einstellungen/daten/instanz`, behind `guard::same_origin`).

**Rule for later table rebuilds:** if a migration rebuilds one of the ten tables by the SQLite procedure
(new table, copy data, `DROP`, `RENAME`, see 0040), it must copy the column `uuid` including CHECK and values
along and afterwards create `<T>_uuid`, `<T>_uuid_fill` and `<T>_uuid_tombstone` anew – for `work` and
`chat_thread` additionally `work_cascade_uuid_tombstone` or `chat_thread_cascade_uuid_tombstone` from 0042. A
`DROP TABLE` with foreign keys switched off deletes no row individually and therefore leaves no
tombstones; but the triggers disappear with the table. For this the migration test compares the objects from
0041/0042 (`new_objects()` in `crates/baruch-store/tests/migration_0041.rs`).


### Output (export)


- **Dump** `baruch_store::Store::exchange_dump(work)` (`crates/baruch-store/src/exchange_dump.rs`): one reading
  transaction (WAL snapshot), tables → `baruch_exchange::ExchangeWork`. References are translated to
  UUIDs: `revision.based_on`/`asset_revision.based_on` (numbers) → UUID of the revision, `post_ref.node_seq`/`rev`
  → UUID of item and revision (without a matching revision without `fassung`), `idea.node_id` → item UUID,
  `work_book.cover_asset_id`/`author_photo_asset_id` → image UUID, `done_ref` „key N…“ → item UUID. Positions
  of posts become an order from 1. Images as raw bytes (Base64 of the database decoded),
  `sha256` recomputed. Tombstones: `uuid_tombstone` with the work UUID. Tree of the outline through `parent_seq`
  and `position`.
- **Writing** `baruch_exchange::write` (`crates/baruch-exchange/src/write.rs`): own small writer
  (string, indentation two spaces, no quick-xml writer), so that escaping and the Base64 branch follow the
  contract exactly: `write_xml(doc, Inline|Files)`, `write_manifest`, `write_baruch` (zip 2.4, `mimetype` STORED
  first, raster STORED, `werk.xml`/SVG/manifest DEFLATE, fixed time stamp 1980-01-01 and permissions 0644 →
  deterministic). The writer checks UUIDs, times, image types and `sha256 == sha256_hex(data)` and otherwise returns
  `ExchangeError::Write` with the field name instead of writing a file that the XSD refuses.
- **XSD** `docs/schema/baruch-werk-1.xsd` (minor version 1, frozen since Round 25) and
  `docs/schema/baruch-werk-1-u2.xsd` (minor version 2 with `<karte>`, against which every output has been checked since Round 25);
  check with `xmllint` in `crates/baruch-exchange/tests/xsd.rs` (there also the Rust check of the
  mandatory elements through quick-xml events, in case xmllint is missing) and `crates/baruch-server/tests/round26_export.rs`.
- **Server** `src/exchange/export.rs`: `document` (dump + instance + time UTC + `baruch <Version>`),
  `export_file(state, work, Format)` → `ExportFile` (shared for downloads, RPC, CLI and MCP), page
  `templates/exchange.html` (own minijinja environment), downloads with `Content-Disposition: attachment` and
  `Cache-Control: no-store`, `cli_export` with `create_new` (never overwrite). MCP `src/mcp/exchange.rs`
  (`exchange_export`, limit `MCP_EXPORT_LIMIT` = 4 MiB file, because the answer carries the content twice).
- Migration: export needs none; the reserved number 0044 stays unused.

### Reading in (import)


- **Reading** (`baruch-exchange/src/read.rs`, submodules `read/dom.rs` – safe XML tree with quick-xml 0.38
  (`NsReader`, `trim_text(false)`, `Event::DocType`/`PI` → Unsafe, `GeneralRef` only predefined or
  character reference, limits), `read/map.rs` – tree → model with a „benutzt“ mark per child and attribute
  (unused → `unknown`), `read/archive.rs` – ZIP (zip 2.4: `by_index_raw` for name, link,
  encryption, compression; central directory counted ourselves, because zip silently merges duplicate names;
  unpacking with `take(Grenze + 1)`; manifest), `read/check.rs` – `validate`). `read`/`read_xml`/`read_baruch`
  return only checked documents.
- **Planning and writing** (`baruch-store/src/exchange_apply.rs`): `Store::exchange_preview` and
  `Store::exchange_apply` run through the same function `execute`; the preview rolls its transaction back.
  New: rows with all identifiers from the file (`ensure_free` checks beforehand that no identifier already belongs to
  another work). Reconciling: (a) new items through `outline::insert_bare` (outline as when creating,
  but without revision 1; reports `WouldAdopt` for a new part before chapters or `Misplaced`), afterwards the
  module sets the uuid of the row just created and creates the revisions of the file; (c)/(d) revisions with
  `last_rev + 1` and `derive_uuid`; (b) requests through `requests::outline_request_in` (trial run in a savepoint,
  duplicate → hint) and `requests::add_request_in` (part with consent); (e)–(g) template, posts, ideas,
  threads, images (`assets::prepare` = check as on upload, `free_name`, `path_owner`), data. At the end
  `load::work`. Helpers pulled out without a change in behaviour: `requests::outline_request_in` (core of
  `Store::outline_request`), `requests::add_request_in`, `outline::apply_with` (switch `bare`),
  `outline::heading_title`, `outline::insert_bare`; in `assets.rs` only visibility (`Content`, `prepare`,
  `free_name`, `path_owner`, `name_fits`).
- **Ways** (`baruch-server/src/exchange/import.rs`): page `/austausch/einlesen`, forms `/austausch/vorschau`
  (multipart, incoming file only after a successful preview under `<data>/austausch-eingang/<sha256>`) and
  `/austausch/uebernehmen` (file from the inbox, deleted afterwards), RPC `work.exchange_preview`/`import`, CLI
  `cli_import`; templates `exchange_import.html`, `exchange_preview.html`, `exchange_done.html` (own environment).
  Preview and accepting hold `state.write_lock`; the mirror commit comes from `mirror::record_exchange`
  (`describe_exchange`, one commit, `allow_empty`).
- **No migration** in this stage (0043 stays free): the origin of the requests is `IdeaOrigin::Human` with the
  name „Austausch: <Instanz>“.
- Tests: `crates/baruch-exchange/tests/read.rs` (test files `tests/data/import/`), `crates/baruch-store/tests/
  exchange_apply.rs`, `crates/baruch-server/tests/round26_import.rs` and `round26_roundtrip.rs` (reports
  „ÜBERSPRUNGEN“ as long as `work.exchange_export` is a stub).

### Integration Round 26

Merged with `--no-ff` in the order ids → export → import (no conflicts; the doc sections lay
apart). Afterwards:

- **False alarm in the guard test** `no_code_path_but_exchange_apply_writes_the_uuid_column` (`baruch-store/tests/uuids.rs`):
  it searched `INSERT` and `, uuid =` line by line and fired on `HashSet::insert(uuid…)` in the reader and on
  `uuid => …` in the minijinja context of the output page. Now only SQL: `INSERT INTO` or `, uuid =` without `>`.
- **Round-trip test** `round26_roundtrip` ran in import only as „ÜBERSPRUNGEN“; after the merge it failed on the
  heading level (`===` at a section, the store demands `==`). The hand-written test file
  `werk.baruch.xml` deliberately carries unusual texts (white space, `]]>`, control characters); reading in does not check the
  Typst level of the revisions against the kind of the item (the text stays exactly as it came). A third test shows
  renaming on B → on A the revision that came along and the hint „Umschalten genügt“, no request `Rename`.
- **MCP hint on connecting** names `exchange_export` and says that there is no reading in through MCP.
- **Visual check:** singular/plural in preview, result and output page („1 Fassung“, „1 Antrag“); the form
  „Übernehmen“ grew as an empty box down to the end of the page (`.works-page` is a flex column, `.work-form` has
  `flex: 1 1 320px`) → `flex: 0 0 auto` on the exchange pages; the table of deviations aligned left; in the
  request list under `/gliederung` the long reason (instance UUID and sha256) wraps instead of pushing the buttons out of the
  box.
- **End-to-end with two throwaway instances** (own servers on `127.0.0.1:0`, `--secrets memory`, connections to
  `127.0.0.1:9`, a third data folder only for the command line): 75 checks green – work with two parts, foreword without
  a number, section, several revisions (superseded, proposal through MCP with model), two template revisions, ideas
  (open, done, deleted), posts, thread, PNG with two revisions as cover, SVG, book data, speaking time;
  A → B new and B → output equal to A except for `<ausgabe>`; command line (preview, new, exit 1 without `--abgleichen`,
  export without overwriting, file over 128 MiB); B changes revision, idea, renaming, „mit Nummer“ → A reconciles through
  the form: two more revisions, one idea, request „mit Nummer zählen“ with origin „Austausch: Laptop“,
  „aktuell“ unchanged, no UUID vanished; a second pass creates nothing; `..` path, wrong checksum,
  DTD with an external entity, XML bomb, 40 levels, ZIP bomb (70 MiB `werk.xml`) refused, row counts of all 17
  work tables equal; MCP, XML and ZIP output without keys, MCP tokens and persona models.

### Follow-up Round 26 (review)

- **Playing back a deleted work:** `delete_work` leaves ideas behind as ideas without a work with their identifier
  (0011, `ON DELETE SET NULL`); reading in as a new work therefore refused its own backup („gibt es hier
  schon in einem anderen Werk“). Now `exchange_apply::adopt_orphan_idea` assigns such an idea to the
  work read in again (only `work_id` and `node_id`, the content stays), both when reading in anew and when reconciling.
- **Back and forth without duplication:** under (c) and (d) no revision is created if the item already has one with exactly
  the same text (`same_text_number`); before, after an edit in place, reconciling alternately on each side produced
  a copy with identical content (Austauschformat.md §4 „Hin und her“).
- **Instance name from the file:** `baruch_exchange::clean_instance_name` when reading and once more in
  `mirror::describe_exchange` (`<`/`>` → `‹`/`›`, control characters → spaces, 80 characters). Before, the
  mirror commit failed on „Laptop <Jörg>“ (gix refuses `<>\n` in the author), although the work had already been taken over.
- **Tombstones of a new kind:** with `version` > 1 an unknown `<eintrag art>` is reported as unknown and
  skipped instead of refusing the file (preparation R25, Austauschformat.md §6).
- **Round-trip test without an escape:** `round26_roundtrip::export` fails hard on every error; the variant
  `ExchangeError::NotImplemented` is removed (no stub produces it any more).

## Idea map (Round 25)

**State of Round 34:** page, image, widget and outputs of this section are replaced by the mind map (see
"Round 34 – Mind map" at the end); `bild/layout.rs`, `bild/svg.rs` and `web/src/idea_map.ts` no longer exist. Data,
links, nearness, groups and exchange still apply.

The ideas of a work as a map (contract Round 25): in the centre the work, around it parts and chapters, ideas at
their reference, links between ideas (migration `0043_idea_map.sql`, tables `idea_link` and
`idea_map_pos`), nearness only displayed. Implemented in three branches (karte-daten, karte-bild, karte-widget) on the
interface R25-S1; the only remaining stub was the naming of groups by an assistant – implemented since Round 19b
(`idea_map/groups.rs`, see "Round 19 – RAG maintenance, rest of the references", R19b pgvector).

### Data (karte-daten)
- **Migration 0043** (`0043_idea_map.sql`): tables `idea_link` (idea → idea, `kind` from four words,
  `note` ≤ 500, origin, `deleted` soft, `uuid`; CHECKs against self-reference and for „widerspricht“ only with
  `from_id < to_id`, `UNIQUE (from_id, to_id, kind)`) and `idea_map_pos` (one position per work and idea, coordinates
  ≤ 1e6). Both with uuid as in 0041 (unique index, fill trigger, tombstone trigger: link with the work of the
  idea `from_id`, position with its work) and `work_map_pos_cascade_uuid_tombstone` (delete positions before the work,
  so that the tombstone knows the work uuid) as well as `idea_link_cascade_uuid_tombstone` (delete links before a hard-
  deleted idea, otherwise the tombstone of a link with this `from_id` would lack the work; added by the
  integrator during the merge). `uuid_tombstone` gets the kinds `idea_link`/`idea_map_pos`: the ten
  tombstone triggers from 0041 are dropped, the table is created anew through a temporary copy (not `RENAME`, so that
  the text in `sqlite_master` stays the same except for the CHECK), then the triggers stand there again literally. No
  foreign key points to `uuid_tombstone`, therefore not in `FOREIGN_KEYS_OFF`. Test `tests/migration_0043.rs`
  (rebuilt database in the state 0042 with tombstones of all ten kinds).
- **Store** `src/idea_links.rs`: `LinkKind`, `IdeaLink`, `NewIdeaLink`, `MapPos`; `idea_link_create` (checks first,
  then one transaction; normalises „widerspricht“; existing active pair → `changed = false` without writing,
  deleted one → restored), `idea_link_get/delete/restore`, `idea_links_of_idea/of_work` (`deleted = false`:
  active links with two undeleted ideas – a deleted idea thus drops out only through the filter),
  `idea_map_positions` (only undeleted ideas that still belong to the work), `idea_map_pos_set`
  (UPSERT, the uuid stays), `idea_map_pos_reset` (hard, tombstone). Errors through `StoreError::Idea` (→ -32602), the
  uuid is set only by the fill trigger. `einfaelle.md` (`ideas.rs`) gets per idea `- Verbindungen: → 79 führt weiter;
  ↔ 48 widerspricht („Notiz“); ← 12 Beispiel für` through `idea_links::mirror_lines`. Test `tests/idea_links.rs`.
- **Server** `src/idea_map/mod.rs`: RPC (`call`, parameters also as digit text for the form of the map;
  `filter_from_params` = common interpretation for RPC and page) and `build` (nodes from the outline with
  expanding `open`, filter status/word/origin, hits `q` through `Store::idea_search`, links, positions, nearness).
  `idea_map/links.rs`: writing links under `write_lock` with exactly one commit through `mirror::record_idea`
  (own commit order `describe`, `mirror.rs` stayed unchanged), also for MCP. `idea_map/near.rs`: pairs from
  stored vectors (cosine from a threshold) and equal keywords (`word_key`), per idea at most
  `near_limit` partners (greedy by value, deterministic). `rag/neighbors.rs`: reads the stored vectors
  `idea:<id>` of the work from LanceDB (`Index::stored`), never an embedding; without an embedder or index folder
  `NoEmbedder` (creates no folder), on error or another model `Unavailable`.
- **MCP** `src/mcp/idea_map.rs`: `idea_map` (with `url` per idea), `idea_links`, `idea_link`, `idea_unlink`; origin
  through `tools::origin_of` (persona or `@mcp:<client>`).
- Tests `tests/round25_daten.rs`: 40 invented ideas (nodes, anchors, counters, expanding, filters, `q`), commits
  with trailers and `einfaelle.md`, positions without a commit, deleted idea, MCP with origin, nearness with an own
  mock embedder (evidence: no embedding when drawing) and without an embedder.
<!-- End R25 karte-daten -->

### Image and pages (karte-bild)

`crates/baruch-server/src/idea_map/bild/` turns the `IdeaMap` of `idea_map::build` (karte-daten) into image, page
and outputs; it has no data or RPC of its own.

- **`layout.rs`** – deterministic radial layout, pure (no randomness, no order from HashMaps): work at
  (0,0); rings by the depth in the outline of the map; angles as in a radial tree (top-level nodes from 12 o'clock
  clockwise, sector by the space needed by the subtree, children in the sector of the parent node). The ideas of a
  node lie as a **cluster** on a hexagonal grid (`LATTICE` 52, radius `IDEA_R` 13, nearest grid points
  first, the strip of the label below the node stays free; in the last grid ring places facing outwards
  first). No overlap **by construction**: in the grid ≥ 2·`IDEA_R`+`GAP`, to the node ≥ r+`IDEA_R`+`GAP`;
  per ring the radius grows (×1.05) until every pair of cluster discs is ≥ Rc₁+Rc₂+`GAP` apart; rings are
  disjoint annuli. Stored positions (`positions`) override (`pinned`, no grid place). Measurement: 300 ideas
  ≈ 1 ms in the debug build (test `round25_bild::three_hundred_ideas_…`, acceptance < 200 ms). Result `Layout` (circles in the
  order nodes, ideas; `get(id)`; viewBox with a margin for labels).
- **`svg.rs`** – drawing according to the DOM contract §4.4: `<svg class="im-svg" … data-im-svg>`, `<g class="im-viewport">`,
  edges as `<line>` from centre to centre (tree `im-tree`, reference `im-ref`, links
  `im-link im-kind-<art>` with `data-link`/`data-kind` and `<title>`, nearness `im-near` with `data-score`, only with `near`;
  since Round 18a arrowheads, dash style as an attribute and a visible label `im-edge-label`, see "Round 18a – Ribbon"),
  nodes with `data-node`/`data-x`/`data-y`/`data-r` and `transform="translate(x y)"` (work and „Ohne Kapitel“ `<g>`,
  part/chapter/section `<a>` in key form, ideas `<a href="/einfaelle/<id>">` with `data-idea`/`data-key`/
  `data-anchor`, `im-open|im-done`, `im-pinned`, with `q` `im-hit|im-dim`). Numbers to one decimal place (`num`),
  all user texts through `esc`. `render(map, layout, standalone)`: in the HTML without its own style (`static/karte.css`, colours from
  `app.css`, hence light/dark without duplication), standalone with XML head, size, `<title>` and `STANDALONE_STYLE`
  (fixed light colours, only simple class selectors – the SVG reader in Typst typesetting knows no variables); attributes
  without a value then as `=""` (well-formed XML, checked by `xmllint`).
- **`export.rs`** – Markdown (heading per node, level = depth + 1; ideas with links in the notation
  of the mirror; „Widersprüche“, „Nähe“; Markdown characters escaped) and FreeMind 1.0.1 (nodes nested as on
  the map, IDs `ID_<id ohne Sonderzeichen>`, thought as `richcontent TYPE="NOTE"`, links as `arrowlink`
  – „widerspricht“ with an arrow at both ends – and as the attribute „Verbindung“). `tree_order` and `links_of` are also used by
  the page.
- **`mod.rs`** – routes and page. Filter from the address (`parse_filter`, unknown values are dropped; for `near` the last
  value wins, because the form puts a hidden `near=0` before the check box; `open` repeatable) and canonical
  query part (`filter_query`) for all links, outputs and the return jump. Own template environment (`base.html` +
  `templates/karte.html`, no `fragments/idea_capture.html`). Outputs: `.svg` (`image/svg+xml`), `.md`
  (`text/markdown; charset=utf-8`), `.mm` (`application/x-freemind`, attachment), `.pdf` through `pdf::source_to_pdf(work,
  PdfKind::Map, …)` (`pdf_source`: A4 landscape, title and filter only as Typst strings, the standalone SVG fitted in as the
  file `karte.svg`; 409 already running, 503 time, 422 typesetting error – as a map with a message).
  `POST /werke/{id}/karte/act` (behind `guard::same_origin`): `idea.update` through `crate::ideas::call` (only `id`, `key`),
  `idea.link`/`idea.unlink`/`idea.link_restore`/`idea.map_pos_reset` through `crate::idea_map::call`; beforehand a check that
  the idea or the link belongs to this work (otherwise 422). `back` is not taken over, but read as a filter
  and rebuilt (`back_filter`) – the redirect `303 /werke/{id}/karte?<filter>&ok=<wort>[&undo=<id>]`
  contains nothing unchecked. Errors: the map with a message (422 invalid, 409 refused, 400 unknown action, 404
  unknown work).
- Besides: `PdfKind::Map` („Gedankenkarte“) in `pdf.rs`; menu entry in `base.html`; `ideas.rs` passes the pages
  `map_href` (list) or `links`/`map_href`/`map_act` (idea, through `Store::idea_links_of_idea`); style in
  `static/karte.css` (dash style per kind, `im-empty`, `im-dim`, `im-hide-near`, print, dark) and `static/ideas.css`.
- Naming groups: since Round 19b no longer a stub, but the button „Gruppen benennen (Assistent)“ with the
  section „Gruppen“ (`idea_map/groups.rs`, see "Round 19 – RAG maintenance, rest of the references", R19b pgvector).
<!-- End R25 karte-bild -->

### Widget and exchange (karte-widget)

**Widget** `crates/baruch-server/web/src/idea_map.ts` (`register("idea-map", …)` on `.im-stage`, compiled by `build.rs`,
loaded through `main.ts`), style `static/karte_widget.css`. It reads only the DOM contract §4.4 (classes
`im-*`, `data-node/x/y/r/key/idea/anchor/work/near`, `data-from/to` of the edges, `[data-im-viewport]`,
`[data-im-toolbar]`, `[data-im-side]`, `form.im-filter`) and calls only `idea.get`, `idea.create`, `idea.update`,
`idea.link`, `idea.map_pos_set`, `idea.map_pos_reset` (test `round25_widget::the_widget_reads_only_the_dom_contract_and_calls_only_the_allowed_methods`
checks that on the delivered script). widgets.ts, ideas.ts, zoom.ts and main.ts stay unchanged.

- **View section:** state `{s, tx, ty}` → `transform="translate(tx ty) scale(s)"` only on `[data-im-viewport]`.
  Zoom around a point of the viewBox (`svg.getScreenCTM()`), panning through the difference of two viewBox points;
  `sessionStorage` `baruch_im_view_<werk>` (try/catch) keeps the view section across reloading. The mouse wheel with Ctrl
  is not touched (zoom.ts).
- **Dragging:** pointer events with `setPointerCapture`; from 4 screen pixels on a click becomes a drag.
  Node coordinates through `viewport.getScreenCTM().inverse()` (thus in the units of `data-x/data-y`);
  `place()` sets `data-x/y`, `transform` and the end points of every `.im-edge` with `data-from`/`data-to` of the node.
  Drop target (`targetAt`): node with distance ≤ `data-r`, ideas before outline nodes, never the work. Idea →
  dialog (`<dialog class="im-dialog">`, four kinds, note ≤ 500) → `idea.link`; part/chapter/section →
  `idea.update {id, key}`; „Ohne Kapitel“ → `idea.update {id, key: null}`; otherwise `idea.map_pos_set` (coordinates limited to
  ±1e6) and `im-pinned`. After a reference and a link the page reloads (the server lays out).
- **Keyboard:** roving `tabindex` (exactly one node 0, the work first), `<g>` nodes get `role="button"`
  and an `aria-label` from their `<title>`. Arrows: nearest node by distance in the direction plus double the
  lateral offset; connect mode (`V`, classes `im-connecting`/`im-source`/`im-target`) moves only over ideas.
  `N` toggles `im-hide-near` and `near` in the address (`history.replaceState`); if the page was drawn with `near=0`
  (no nearness edges in the DOM), it reloads with `near=1`. Alt+E: dialog „Neuer Einfall“ →
  `idea.create {work, key, words, note}` with the reference of the focused node. Messages through `.im-status`
  (`role="status"`), help `.im-help` via `aria-describedby` at the map. Texts only through `textContent`.

**Exchange minor version 2** (contract §4.6):

- `baruch-exchange/src/model.rs`: `FORMAT_VERSION = 2`; `ExchangeLinkKind` (`fuehrt-weiter`, `gehoert-zu`,
  `beispiel-fuer`, `widerspricht`), `ExchangeLink`, `ExchangePosition`, `ExchangeMap` (`ExchangeWork.map`),
  tombstone kinds `Link = "verbindung"` (`idea_link`) and `Position = "lage"` (`idea_map_pos`) with
  `since_version()` (2, otherwise 1); plus `MAP_COORD_LIMIT`, `MAX_LINK_NOTE_CHARS` and `ExchangeMap::is_empty`.
- `write.rs`: `<karte>` at the end of `<werk>` only with content, first `<verbindung>` (with `<notiz>`), then `<lage/>`;
  x/y through `f64::to_string` (shortest exact representation), outside ±1e6 → `ExchangeError::Write`.
- `read/map.rs`: `<karte>` only from `version >= 2` (otherwise unknown, `werk/karte`); a tombstone kind with
  `since_version() > version` is an XML error; unknown kinds with `version > FORMAT_VERSION` are reported and
  skipped. `read/check.rs` (`check_map`): references to ideas of the document, no self-reference, no pair
  with the same kind twice (`widerspricht` normalised), note ≤ 500 without control characters except `\n`, per idea
  one position, |x|, |y| ≤ 1e6; the identifiers count towards uniqueness in the whole document (also against tombstones).
- `docs/schema/baruch-werk-1-u2.xsd`: copy of the v1 XSD with a defined `Karte` (`Verbindung`, `Lage`,
  `VerbindungArt`, `Koordinate` as `xs:double` in ±1e6) and an extended `GrabsteinArt`; the v1 XSD stays
  unchanged.
- Store `exchange_dump.rs`: `map()` reads `idea_link` (both ideas in the work, also soft-deleted ones, by id) and
  `idea_map_pos` (idea still in the work, by id); tombstones as before through `work_uuid`.
- Store `exchange_apply.rs`: `new_map` (after ideas and images; `ensure_free`, `widerspricht` with from < to
  by local ids) and `merge_map` (after `merge_ideas`, so that ideas just read in count): unknown
  identifier + both ideas in the work → create, otherwise a hint; the same pair+kind (in the normalised direction)
  → hint „gibt es hier schon unter anderer Kennung“; known identifier → never overwrite (hint for note/
  deleted); tombstone → „hier gelöscht“; position only if there is none for (work, idea). `AddCounts.links`,
  `AddCounts.positions` (in `total()`), words `verbindung`/`lage` in `items` and `deleted_here`. Only here is
  `uuid` written into `idea_link`/`idea_map_pos` (guard test `uuids.rs`).
- Server: `exchange/import.rs` (`add_list`: „Verbindungen der Gedankenkarte“, „Lagen auf der Gedankenkarte“ in
  preview, result and CLI), `exchange/export.rs` (`counts`: `links`, `deleted_links`, `positions` for
  `templates/exchange.html`), description of MCP `exchange_export`. No migration (0044 stays unused).
- Tests: `baruch-exchange/tests/{read,write,xsd}.rs`, `baruch-store/tests/{exchange_map,exchange_apply,uuids}.rs`,
  server `tests/round25_widget.rs` (round trip A → B → A with map, preview page, page „Austausch“) and
  `round26_*` against the u2 XSD.
- Found (reported to karte-daten, migration 0043): if an idea that is the `from_id` of a
  link is deleted hard, the tombstone trigger `idea_link_uuid_tombstone` writes `work_uuid` NULL (the idea row is
  already gone in the cascade, as with threads before 0042); a remedy would be a `BEFORE DELETE ON idea` that deletes the
  links beforehand. Rare in practice (ideas are only deleted softly). Fixed in the integration
  (trigger `idea_link_cascade_uuid_tombstone` in 0043, `uuids.rs` now checks both directions).
<!-- End R25 karte-widget -->

### Integration Round 25 (integrator, 2026-11-08)
- Merged `team/karte-daten` → `team/karte-bild` → `team/karte-widget` (`--no-ff`, without conflicts; all three
  branched from S1 `c0e5e8a`).
- **Migration 0043 corrected** (not yet run on any instance): `BEFORE DELETE ON idea` deletes the links
  while the idea row still stands (`idea_link_cascade_uuid_tombstone`), otherwise the tombstone of a link with
  this `from_id` would carry no work (finding karte-widget). `migration_0043.rs` knows the new object, `uuids.rs` now deletes
  the middle idea (source and target at once).
- **Canonical address of the map:** `build` puts the defaults `near_limit`/`near_threshold` into the applied filter;
  `bild::filter_query` then wrote them into every link (`?near_limit=3&near_threshold=0.75`). Visible only after
  merging (two tests in `round25_bild.rs` fired); now default values are dropped from the address.
- The parts of `round25_bild.rs` that reported „ÜBERSPRUNGEN“ without R25a now run fully.
- **Acceptance** `tests/round25_abnahme.rs`: invented work with two parts and 40 ideas, embeddings from the mock
  `MockEliezer` (512 dim.); page without JavaScript (SVG, outline, filter via GET, forms), `im-empty`, anchor at the
  chapter or „Ohne Kapitel“; nearness through vectors without an embedding request when drawing and through keywords;
  reference via widget RPC and form (one commit each); „widerspricht“ in the map, list of contradictions, `einfaelle.md` and
  MCP `idea_links`; positions without a commit; a deleted idea drops out together with its link and comes back; SVG/Markdown/
  FreeMind/PDF (xmllint). Without an embedding model the page names the substitute through keywords.
- **Visual check** (Firefox headless, own throwaway server with an invented work, embeddings from an own
  mock on 127.0.0.1:0): the stage was squeezed to its minimum height in the flex column `.karte` (map tiny,
  cut off at the bottom with JavaScript) – now `flex-shrink: 0` for the children of `.karte` and `.im-stage` and a
  fixed height `min(78vh, 900px)` for `.im-svg` (`static/karte.css`). „Einpassen“ in the widget leaves room for the
  labels below and beside the circles (`LABEL_SIDE`, `LABEL_BELOW`). The side bar names the status like
  the rest of the user interface „verarbeitet“ (not „erledigt“). Images: `.claude/sicht/r25/`.
- **Follow-up after the review:**
  - Change of work of a linked idea: when moving (another work or „ohne Werk“) `Store::idea_update` dissolves
    all undissolved links of the idea softly in the same transaction and deletes its positions hard
    (tombstone `idea_map_pos` in the old work). Before, the link stayed active and connected two works: map and
    exchange format left it out, `/einfaelle/<id>`, `idea_links {idea}` and `einfaelle.md` of both works showed
    it, „Lösen“ from the page of a moved target failed with 422, an old position reappeared when moving back.
    `ideas::update` names the dissolved links in the commit of both mirrors and as `unlinked` in the
    answer (RPC `idea.update`, MCP `idea_update`). Tests: store `idea_links.rs`
    (`moving_a_linked_idea_…`), server `round25_daten.rs` (`moving_a_linked_idea_to_another_work_unlinks_it_everywhere`).
  - Counters of the map roll up: `idea_map::build` counts an idea at the anchor and at all ancestors up to below
    the work (`parent_slot`). A part with filled chapters counted as empty before (pale, without a number, „Keine
    Einfälle.“); an expanded chapter now counts its sections too. List and Markdown say for an item
    without own ideas but with ideas below it „Keine eigenen Einfälle; sie hängen an den Punkten darunter.“
  - „Zeigen, was fehlt“ highlights instead of toning down: `im-empty` = dashed border in the warning colour, in the circle
    the word „leer“ (`svg.rs`, also in the standalone SVG), in the list the tag „leer“ and a dashed
    warning border (`static/karte.css`, `templates/karte.html`).

## Round 18a – Ribbon

Ribbon for editor and typeset page, cancellable Typst typesetting without runs from foreign pages, chapter of a shared
page, header in narrow windows and the edges of the idea map.

- **Ribbon** (`templates/fragments/edit_toolbar.html`): one fragment, two versions. In the editor
  (`div.ws-bar.ribbon[data-edit-toolbar][hidden]`) all hooks of the typeset view stay (`data-ws-mode`, `data-ws-tools`,
  `data-ws-format`, `data-ws-cmd`); behind them `span.ribbon-formats[data-widget=ribbon]` with one button each
  `data-ribbon-cmd`/`data-ribbon-snippet` from `src/ribbon.rs` (`formats(template)`: `scene`/`work` only with
  `#let szene`/`#let arbeit` in the work template, then `footnote`, `quote`, `list`, `enum`, `terms`, `break`, `link`;
  RPC `ribbon.formats`; context `ribbon` from `view::ViewExtras`) and `data-ribbon-image`. Widget `web/src/ribbon.ts`:
  with an open typeset view only `sendWsCommand` (frozen `ws_command.ts`), `handled = false` → hint through
  `baruch:toast`; in source mode inserting into the text field at `selectionStart` (`execCommand("insertText")`, fallback
  `setRangeText` + `input`). No import of `wysiwyg.ts`. With `satz_page` (typeset page) `div.ribbon-satz
  [data-ribbon-satz]`: back, PDF, ‹ › with `aria-label`, form „Satz abbrechen“ – without JavaScript; `typeset.html`
  empties `{% block scripts %}` (no `main.js`). Style `static/ribbon.css`. Follow-up: `typeset::render` sets
  `scriptless = true`; `base.html` then shows no widget in the header – the theme as a link to
  `/einstellungen/darstellung`, „⚙ Einstellungen“ without dialog and `aria-keyshortcuts`, „Öffnen“ of the work choice
  visible instead of in `<noscript>`.
- **Cancellation** (`baruch-render`): `Cancel(Arc<AtomicBool>)` with `cancel`/`is_cancelled`, `compile_cancellable(source,
  cancel) -> Option<Rendered>` (`None` = cancelled, never half pages); `compile`/`compile_pdf` unchanged.
  Check points: before the run, after `typst::compile` (evaluation and layout), before every SVG page, at the end.
  **Measured** (`crates/baruch-render/tests/cancel.rs`, debug build, 2026-11-08): 500 paragraphs (512 pages) – without
  cancellation 777 ms, cancelled after 200 ms back after 252 ms (the layout was finished, what was saved was writing
  the SVG pages); evaluation loop with 3 000 000 rounds – without cancellation 353 ms, cancelled after 100 ms back
  only after 368 ms (not interrupted). **Not possible:** a cancellation through the world (delivering no more
  files or fonts after the cancellation). Typst memoises the answers of the world (`comemo`) and aborts with „comemo:
  found differing return values. is there an impure tracked function?“ if the same question is answered differently
  within one run – observed like that in the first draft and therefore discarded. So evaluation and layout always run
  to the end; an own process per run would be the only hard cancellation (not done, remainder).
- **Server** (`src/typeset.rs`): every run for a work (`compile_for`: typeset page, SVG page, `preview.typst`,
  `typst_check`) registers with data folder, work and `Cancel` in a process-wide list; the registration
  (`Arc<Registration>`) is held by request and background run, it drops with the last one. `cancel(work)` (contract, all
  servers in the process) or `cancel_in(data, work)` (RPC `preview.cancel`, `POST /werke/{id}/satz/abbrechen`) set
  all `Cancel` of the work. The request waits with `tokio::select!` for the lock or run **or** the cancellation (polled
  every 20 ms) and returns immediately with -32000 `CANCELLED`; the run holds the lock `RUNS` until Typst stops at the
  next check point, and stores nothing. `compile` without a work (cover) is not registered. The page
  after cancelling (`?abgebrochen=1|0`) does not typeset anew (`cached_typeset`: only an already typeset stand). For
  tests: `running_in`, `runs_of(stand)` (runs begun per stand, at most 256 stands), `cache_contains`,
  `pdf::pdf_runs()`.
- **Foreign pages:** `GET /werke/{id}/satz`, `…/{stand}/{n}.svg` and `/werke/{id}/pdf` – after the follow-up to
  Round 18 also the cover (`/werke/{id}/umschlag`, `.svg`, `.pdf`), `/werke/{id}/stichwortzettel.pdf` and
  `/werke/{id}/karte.pdf`, i.e. every GET that starts Typst and holds the lock `typeset::RUNS` – first check
  `guard::cross_site_get` (`Sec-Fetch-Site: cross-site|same-site`) → 403 (`typeset::foreign_get`), before they prepare the work
  or start Typst. Without the header (curl, old browsers) everything stays as before.
- **Cache:** in addition to the count (8) a byte limit `CACHE_BYTES` = 192 MiB over all SVG pages
  (`cache_put_within`); the newest stand stays even above the limit.
- **Chapter of a shared page:** `Rendered.page_lines` (new in `baruch-render`) names per page the lines of
  `main` from which text stands on it (first glyph of every text piece, also in headers and footers).
  `Typeset::chapter` adds the page on which the next chapter begins to the chapter if text from its
  lines stands there – without heading lines, because the template quotes headings as a running head or in the
  table of contents on foreign pages. Limits: a chapter that ends on the shared page only with an image
  or a line (without text) does not count there; if the template quotes other text than headings on foreign
  pages, a page can be assigned to one chapter too many.
- **Fonts:** `baruch_render::warm_fonts()` loads embedded and system fonts once per process;
  `typeset::warm_fonts()` starts that in `spawn_blocking` and returns immediately. Proposal for the integrator:
  call it in `main.rs` at the start of `serve` (in the Tokio runtime); tests do not call it by themselves.
- **Header:** `static/app.css` – below 1000 px width `@media (max-width: 999.98px) { .topbar { flex-wrap: wrap } }`;
  above that the single-line rule stays. The test (`round9_menu.rs`) only reads the rules; the width is measured by the
  visual check.
- **„neu“ as a word:** `chat.ts` appends `span.badge.post-new-label` („neu“) behind the name of new posts.
- **Light strip on the right** (typesetting, outline scheme): `.works-page` (`works.css`) is at the same time a scroll container
  (`overflow: auto`) and limited to 1100 px and centred; on long pages its scroll bar therefore stood in the middle of the
  window to the right of the content. `typeset.css`/`numbering.css` give these two pages the full width and
  move the content, equally wide, into the middle through `padding-inline: max(18px, calc((100% - 1100px) / 2 + 18px))`.
  Cause derived from the CSS, not measured in the browser (visual check of the integrator).
- **Idea map, DOM contract extended** (`src/idea_map/bild/svg.rs`, `web/src/idea_map.ts`, tests
  `round25_{bild,widget}.rs`, `round18_menueband.rs`):
  - `<defs>` before `g.im-viewport` with `<marker id="im-arrow-<art>" class="im-arrow im-arrow-<art>">` for the
    directed kinds (`leads_to`, `belongs_to`, `example_of`; `contradicts` is undirected). The tip has a
    fixed light fill (for SVG file and PDF); on the page `karte.css` colours it according to the theme.
  - Link edge `line.im-edge.im-link.im-kind-<art>` additionally with `stroke-dasharray` (except `leads_to`), with
    directed kinds `marker-end="url(#im-arrow-<art>)"` and `data-arrow` (in the standalone file
    `data-arrow=""`); it then ends at the edge of the target circle (`r` + 2), otherwise at the centre. Reference `im-ref` with
    `stroke-dasharray="4 3"`, nearness `im-near` with `"1 3"`, tree solid.
  - Label: `g.im-edge-labels` after `g.im-edges` and `g.im-nodes` (since the integration above the nodes, so that
    short edges between close ideas are labelled legibly; `pointer-events: none`), inside it per link or nearness edge
    `<text class="im-edge-label im-label-<art>|im-label-near" data-from data-to x y text-anchor="middle">` with the
    kind („führt weiter“ …) or „nah“, 4 units above the middle.
  - Widget: while dragging `place` redraws every edge through `redraw` (arrow edges at the circle edge) and puts every
    label with the same `data-from`/`data-to` into the middle through `relabel`. The side bar `.im-side` begins
    at `--im-top` (distance of the image from the upper edge of the stage, measured by the widget, anew on `ResizeObserver` and
    `resize`), so it no longer covers the toolbar.
  - Outputs: SVG/PDF like the page (style of the label in `STANDALONE_STYLE`); FreeMind `arrowlink` with `COLOR`
    per kind (FreeMind knows no dash style for arrows); Markdown names the kind as a word as before.

<!-- R38c split: architektur_1 above, architektur_2 below -->

## Runde 18b – Konstrukte

**Satzbild (`crates/baruch-wysiwyg`):** Neue Blockarten `Enum` (Aufzählung `1.`, `start`) und `Terms` (Begriffsliste,
`Term {term, desc}`), `List`/`Enum` mit `depths` (Tiefe je Punkt, geschachtelt; `#[serde(default)]`, ohne Schachtelung
nicht geschrieben – alte JSON bleibt gültig); neue Inlines `Linebreak`, `Link {url, c?}`, `Ref {target,
supplement?}`. Das Zerlegen in Stücke (`parse.rs`) kennt Zeilenarten für Aufzählung und Begriff; ein Listen-Stück
umfasst eingerückte Punkte und Folgezeilen, `list()` bestimmt die Tiefe über einen Stapel der Zeichenspalten und
lehnt Unsicheres ab (Insel). `scan::skip_escape` behandelt `\` vor Leerraum als Umbruch (ein Zeichen), damit ein
Zeilenende dahinter die logische Zeile beendet. `emit.rs` schreibt kanonisch (zwei Leerzeichen je Ebene, fortlaufende
Nummern, `\:` im Begriff, `#ref(<…>)` vor Zeichen des Ziels, `\ ` vor schließenden Zeichen). `Edit::Raw {replaces,
raw}` ersetzt den Quelltext eines Blocks wörtlich (im Rumpf mit dessen Einzug), `WysiwygError::NewPart` verbietet
neue oder umgewandelte Teile. `image_paths` liefert alle Bildpfade einer Insel; `html.rs` zeigt alle. Eingefroren
blieben `render_block`/`escape` (genutzt von `align.rs`) und die `pub use`-Zeilen von `style`/`align`.

**Server:** `wysiwyg::image_ref` nimmt die Normalform ohne führendes `/`; `rpc.rs` (`revision.update`) wandelt
Markdown-Überschriften um wie `revision.save`. Der Teil-Block im Kapiteltext ist festgelegt (siehe
`docs/WYSIWYG.md` „Teile und ohne Nummer“): Satzbild lehnt ab, `outline.save` nur nach Bestätigung (`split`).

**Widget (`web/src/wysiwyg.ts`):** liest und schreibt die neuen Konstrukte (verschachtelte `<ul>/<ol>`, `<dl>`,
`span.ws-linebreak`, `a.ws-link`, `button.ws-ref`), öffnet Inseln per Doppelklick an Ort und Stelle (`editIsland`,
`data-edited` → `{replaces, raw}`), führt `baruch:ws-command` aus (`runCommand`, merkt die letzte Auswahl im Satzbild)
und beantwortet `baruch:ws-caret` (`caretPlace`: zwei `wysiwyg.serialize`, Text und Präfix bis zum Block am Cursor).
`guardHeadingMerge` überspringt `.pv` (`blockNeighbour`), `applyStyle` übernimmt ein vorhandenes
`style#ws-style[data-work=W]` ohne RPC. `ws_command.ts` ist unverändert.

**Lektor-Reste (`src/assist`):**
- Ausgelassene Teile: `context::Frame::environment` schreibt aufeinanderfolgende ausgelassene Teile in **eine** Zeile
  (`--- nicht mitgegeben (…): A; B; C ---`, `context::OMITTED_HEAD`). Die Planung zählt dafür je Folge einmal
  `chunk::OMITTED_OVERHEAD` (100) und je weiterem Teil nur Titel + `chunk::OMITTED_ITEM` (2), ebenso die Untergrenze
  `floor_violation`. Folge: Ein Kapitel mit 10 Abschnitten zu je 3000 Zeichen passt bei 4096 Tokens jetzt gestückelt
  (gemessen: kein Aufruf über dem Budget, `round18_lektor.rs`), wo Baruch vorher ehrlich ablehnte.
- Zwischenspeicher der Zusammenfassungen: `assist::summary_cache()` (prozessweit, `VecDeque`, höchstens
  `SUMMARY_CACHE_MAX` = 256 Einträge, älteste fallen heraus, keine Migration). Schlüssel: Datenordner, Modell und
  SHA-256 über den ganzen Aufruf (Regel samt Wortgrenze, Werktitel, Bezeichnung des Teils, Text, `max_tokens`) – also
  „Modell + Text“ und alles, was die Antwort sonst ändert. Leere Antworten und Fehler werden nicht gespeichert.
- „An Persona geben“ auf `/quelle/multigpt`: Formular `action=persona` an `/literatur/act` (wie die Bibelseite),
  vorbelegt mit dem Text des gezeigten Teils (höchstens 4000 Zeichen), Quelle „MultiGPT: Titel (Dokument …)“, ohne
  Titel der Bibliothek; ohne Abschnitt nur ein Hinweis.
- Retry im Strom: neues Ereignis `piece {persona, index, count}` vor jedem weiteren Stück eines gestückelten Auftrags
  (nach dem trennenden Delta, `Progress::Piece`); `assist.ts` beginnt dort einen neuen Teil des vorläufigen Beitrags,
  `retry` leert nur diesen. Der Endtext kam schon vorher mit `done` richtig.

## Runde 18c – Satzbild je Werk

- **Store** (`crates/baruch-store/src/sheet.rs`, Migration `0044_work_sheet.sql`): Tabelle `work_sheet` (eine Zeile je
  Werk, `ON DELETE CASCADE`, CHECK auf Längen bzw. `#`+6 Zeichen; neue Tabelle, kein Umbau, darum nicht in
  `FOREIGN_KEYS_OFF`), `SheetSettings` (`font`, `heading_font`, `size`, `fill`, `heading_fill`, `background`,
  `width`; `None` = aus der Vorlage), `Store::sheet_settings` (ohne Zeile leer) und `set_sheet_settings` (ganze Zeile,
  alles leer löscht sie). Geprüft wird vor dem Schreiben (`SheetSettings::normalized`): Schriftlisten höchstens
  `MAX_SHEET_FONT_CHARS` = 200 Zeichen aus Namen mit Buchstaben, Ziffern, Leerzeichen, `- _ .` (auch paarig in
  Anführungszeichen) – nie `< > ; { } \` oder Steuerzeichen; Längen `pt|mm|cm|em` (Größe 6–48 pt, Breite 6–60 cm,
  `em` = 12 pt); Farben `#rrggbb`. Kein Spiegel, keine uuid, nicht im Austauschformat (Rest). Tests
  `tests/{sheet,migration_0044}.rs`.
- **Darstellung** (`baruch-wysiwyg/src/style.rs`): `SheetStyle.background` (neu; nie aus der Vorlage) →
  `--ws-background`; `sheet_style_over(vorlage, einstellungen)`; `safe` lässt seit 18c `_` zu und nur paarige
  Anführungszeichen. Server `src/sheet.rs`: `sheet_style_for` = `sheet_style_over(sheet_style(work_template),
  einstellungen)` für `wysiwyg.style`, `view.rs` (`ViewExtras.sheet_css` → `<style id="ws-style" data-work>` in
  `fragments/editor.html`) und RPC `work.sheet`/`work.sheet_set`; `sheet_css_for` ersetzt zur Sicherheit jedes `<`.
  `static/sheet.css` (direkt nach `wysiwyg.css`): Blattfarbe und `::highlight(literatur-fundstelle)`. Formular
  `POST /einstellungen/werk/satzbild` und Block „Satzbild“ in `settings/work.rs` / `templates/settings/werk.html`.
  Typst, PDF und Export lesen `work_sheet` nie (Test: Quelle, Stand und PDF byte-gleich).
- **Lange Abschnitte:** `view::READY_SHEET_MAX` = 64 KiB; darüber kein vorab gesetztes Satzbild (`ready_sheet`).
- **Bild an der Cursorstelle** (`src/assets.rs`): `Place` (`end|start|block:<n>|offset:<n>`), `insert_figure_at`
  (rein: nie vor/in die Überschrift, Zeilengrenze, Inseln/Szenen/Listen als Ganzes, Leerzeilen davor und dahinter),
  `image_places` (Auswahl ohne JavaScript). Store `save_human_with_asset_path`: Pfad und Fassung in einer
  Transaktion (`BEGIN IMMEDIATE`), ein Spiegel-Commit. Widget `web/src/sheet.ts` („sheet-tools“ am
  `details[data-image-insert]`): Stelle über `baruch:ws-caret` (am Editor-Formular mit `bubbles`, Antwort von
  `wysiwyg.ts`) bzw. `selectionStart`; ohne Antwort `baruch:serialize-text` und Ende. Es frischt außerdem `#ws-style`
  nach `baruch:settings-changed` über `work.sheet` auf.
- **Vorschläge:** `align.rs` `split_changed` (Wort-Dice ≥ 0,5, monotone Paarung, nur bei ungleich vielen Blöcken);
  Zeilen-Diff ohne Leerraum am Ende (`view.rs`); Leiste `div.pv-bar-editor[data-pv-bar]` vor dem Lesefluss
  (`proposals.html`, `proposals.css` mit `order: -1`/`sticky`, `proposals.ts`).
- **Literatur** (`web/src/literature.ts`): Fundstelle bei offenem Satzbild über die CSS Custom Highlight API
  (`literatur-fundstelle`), kein DOM-Eingriff; öffnet das Satzbild erst, wird nach `data-ws-open` markiert.
- **Verhaltenstest** `tests/round18_satzbild.rs`: Node 20 mit eigenem kleinen DOM-Nachbau (im Test als Text), die
  übersetzten `SCRIPTS` in einem Temp-Ordner, `fetch` nachgebaut; ohne `node` „ÜBERSPRUNGEN“.

## Integration Runde 18

- Zusammengeführt mit `--no-ff` in der Reihenfolge `team/konstrukte` → `team/satzbild-werk` → `team/menueband`, ohne
  Konflikte (Dateibesitz nach Vertrag R18).
- `main.rs` ruft beim Start von `serve` `typeset::warm_fonts()` (Schriftsuche im Hintergrund, nach `rag::catch_up`).
- Fremde Zusicherungen nachgezogen: `migration_0040.rs`/`migration_0041.rs` nehmen die neue Tabelle `work_sheet`
  (0044) aus dem Vergleich der Schemaobjekte und der Verweise auf `work` aus; `round15_nachbesserung.rs` prüft die
  ehrliche Ablehnung jetzt bei 3000 statt 4096 Tokens, weil ausgelassene Teile seit 18b eine Zeile teilen und der
  alte Fall passt (gemessen in `round18_lektor.rs`); `round18_menueband.rs` liest `details[data-image-insert]` mit
  dem Widget `sheet-tools` aus 18c.
- `docs/RPC.md`: Ereignis `piece` und das Verwerfen nur des laufenden Stücks bei `retry` im Strom `/assistenten/stream`.
- Abnahme `tests/round18_abnahme.rs`: ein erfundenes Werk mit Szene, Arbeitsauftrag, Fußnote, Zitat, Aufzählung,
  Begriffsliste, geschachtelter Liste, Umbruch, Link, Verweis, zwei Bildern in einem Block und einer Insel. Jeder Block,
  den das Widget nach einer Bearbeitung neu schickt, bleibt bei kanonischem Text byte-gleich; die Insel wird an Ort
  und Stelle geändert; Formate der Vorlage im Menüband und ein ohne JavaScript gespeicherter Knopftext als Szene;
  Satzbild-Einstellungen im HTML vor jedem Skript und nach einem Neustart; Bild an der Cursorstelle; fremde Seiten
  lösen keinen Satz aus, „Satz abbrechen“ ohne laufenden Satz meldet `abgebrochen=0`.
- Sichtprüfung (Firefox headless, Wegwerf-Server, erfundenes Werk; Bilder unter `.claude/sicht/r18/`), behoben:
  - Unter 900 px blieben die gestapelten Bereiche bei `minmax(16em, auto)` 16em hoch (die Arbeitsfläche hat eine feste
    Höhe, es gibt keinen freien Platz zum Wachsen); der Editor (85vh) ragte unter „Assistenten“. Jetzt
    `grid-auto-rows: max-content`, Gliederung und Assistenten zwischen 16em und 85vh (`static/app.css`).
  - Beschriftungen der Gedankenkarte liegen jetzt über den Knoten (`g.im-edge-labels` als letzte Gruppe,
    `pointer-events: none`): bei kurzen Kanten zwischen nahen Einfällen verschwand „Beispiel für“ unter den Kreisen.

## Runde 27

Umschlag-Editor und Impressum in der Werkvorlage. Schnittstelle (S1): Typen in `baruch_typst::cover_layout`
(`CoverBlock`, `CoverBlockKind`, `CoverFace`, `CoverLayout`, `CoverGeometry`, Grenzen gleich den CHECKs),
`TemplateFunctions`, `BOOK_DICT` und Platzhalter in `baruch_typst::cover`, `font_families`/`has_font_family` in
`baruch-render`, Migration 0045 (`cover_block`, Grabsteinart `cover_block`, Anschriften in `work_book`, Antragsart
`cover_layout`), Store `cover_blocks.rs`, Server `cover_layout.rs`, `cover_print.rs`, `mcp/cover.rs`, Widget
`cover_editor.ts`.

### 27a – Daten (umschlag-daten)

- **Store `cover_blocks.rs`** (Migration 0045, keine weitere Migration; 0046 blieb unbenutzt):
  `check_cover_block` prüft einen Block nach den Grenzen aus `baruch_typst::cover_layout` (gleich den CHECKs) und
  bringt ihn in Normalform (Farbe klein, leere Schrift = `None`, Rücken ohne Laufrichtung = `down`, Arten mit eigenem
  Text ohne Text = `""`, CRLF → LF). Schriftnamen nach der Regel der Satzbild-Schriften (`sheet::check_font`, hier
  ein einzelner Familienname). Verstöße sind `StoreError::Asset(String)` (RPC -32602) mit Art und Feld – `Refused`
  trägt nur feste Texte und gäbe `REFUSED`; eine eigene Fehlerart hätte `lib.rs` geändert (Integrator).
  `set_cover_blocks` schreibt je Art mit `INSERT … ON CONFLICT (work_id, kind) DO UPDATE`: ein geänderter Block
  behält Zeile und uuid (kein Grabstein, im Austausch derselbe Block). `reset_cover_blocks` löscht je Zeile (Trigger
  `cover_block_uuid_tombstone`) und setzt den Titelbereich über `book::read`/`book::write` in derselben Transaktion
  (`BEGIN IMMEDIATE`). `Store::cover_layout_fingerprint` = SHA-256 über das JSON der Blöcke in Art-Reihenfolge
  (`expected_layout` der Anträge).
- **Anschriften** (`book.rs`): zwei weitere Textfelder in `TEXT_FIELDS` (mehrzeilig, 500 Zeichen), gelesen und
  geschrieben wie die übrigen; die S1-Ablehnung ist weg. Das Formular und `work.meta.set` verdrahtet R27c.
- **Anträge** (`asset_requests.rs`): `AssetOp::CoverLayout` mit `params` `{blocks, expected_layout}`; beim Antrag
  werden die Blöcke geprüft und normalisiert (so findet der Doppel-Vergleich über `params` gleiche Anträge),
  `expected_layout` leer = der Store trägt den Fingerabdruck ein; ein Antrag, der nichts ändern würde, wird
  abgelehnt.
- **Server `cover_layout.rs`**: `parse_blocks` liest Blöcke feldweise aus JSON (Fehler mit Art und Feld; serde allein
  nennt bei einem falschen Typ das Feld nicht), RPC wie `docs/RPC.md` „Anordnung“. Commits über eine eigene
  `commit`-Hilfe (Muster `cover::commit`, dazu Trailer eines bestätigten Antrags wie `mirror::AssetBy::Confirmed`), so
  bleibt `mirror.rs` unberührt. Für den Rumpf „vorher → nachher“ eines erstmals gespeicherten Blocks setzt
  `defaults` den Umschlag (`build_wrap_for`, Zwischenspeicher des Satzes) und nimmt `default_blocks` – außerhalb
  der Schreibsperre; ohne Seitenmaß „neu (…)“. `decide_request` vergleicht den Fingerabdruck unter der Schreibsperre,
  sonst `stale`; ein beim Bestätigen ungültiger Block (`Asset`/`Refused`) macht den Antrag ebenfalls `stale`.
  `asset_requests::request_json` zeigt bei `cover_layout` je Block eine Zeile (`block_lines`) und verweist auf die
  Umschlagseite.
- **MCP `mcp/cover.rs`**: `cover_layout` = `layout_view`, bei `REFUSED` (kein Seitenmaß) die gespeicherten Blöcke mit
  `geometry: null`; `cover_layout_set` füllt fehlende Felder aus dem wirksamen Block und legt den Antrag an.
- **Austausch Unterversion 3** (`baruch-exchange`, `model.rs` diesmal von umschlag-daten): `FORMAT_VERSION = 3`,
  `ExchangeCoverBlock` (+ Aufzählungen mit deutschen XML-Wörtern), `ExchangeBook.publisher_address`,
  `printer_address`, `cover_blocks`, `ExchangeWork.sheet` (`ExchangeSheet`), Grabsteinart `CoverBlock`
  (`umschlag-block`, `since_version` 3). Leser: neue Teile erst ab `version >= 3` (wie `<karte>` ab 2), sonst
  `unknown`. Prüfung in `read/check.rs` (`check_cover`). XSD `docs/schema/baruch-werk-1-u3.xsd` = u2 plus `Satzbild`,
  Anschriften (im Schema optional, damit u2-Dateien gelten), `Umschlag`/`UmschlagBlock`, `GrabsteinArt`
  `umschlag-block`; u2- und v1-XSD eingefroren (SHA-256-Test). Store: `exchange_dump` (Blöcke, Anschriften,
  `work_sheet`; Blöcke ohne `work_book` → Buchangaben mit Vorgaben), `exchange_apply` (`new_cover`, `merge_cover`:
  lokal gewinnt je Art, Grabstein → „hier gelöscht“; `merge_sheet`; Satzbild aus der Datei nur nach
  `SheetSettings::normalized`, sonst „Nicht übernommen“). Das Beispielwerk `sample.rs` trägt drei Blöcke, Anschriften,
  Satzbild und einen Grabstein `umschlag-block`.
- **Tests:** Store `tests/cover_blocks.rs`, `exchange_apply.rs`, `exchange_dump.rs` (Rundreise im Store, Abgleich),
  Austausch `tests/{read,write,xsd}.rs`, Server `tests/round27_daten.rs` (RPC, Commits, MCP-Antrag bis zum
  Bestätigen/Veralten/Ablehnen, zwei Instanzen mit XSD-Prüfung).

### 27b – Satz (umschlag-satz)

- **Blöcke in Typst** (`baruch_typst::cover`): `frame_parts` und `wrap_source` setzen mit einer nicht leeren
  `layout` je sichtbarem Block mit Inhalt `place(top + left, dx, dy, block(width, {set text(size, fill, weight|style,
  font); set par(leading, justify, first-line-indent: 0pt, …); set align(…); Inhalt}))`; `dx/dy` = Schnittkante der
  Fläche (`CoverGeometry::face_origin`, im Buch die Seite selbst mit Rand 0) + (x, y), mit Vorzeichen. Rücken:
  `rotate(±90deg, reflow: true, …)`, die linke obere Ecke des gedrehten Rahmens bei (x, y). Schatten =
  `fill: black.transparentize(50%)` mit `outset`, Verlauf = senkrechter Verlauf 0 → 35 % → 0 % Schwarz; `outset`
  ändert die Lage nicht. Hintergründe (Bild, dunkle Flächen) bleiben, der alte Titelblock, die alte Rückseite und das
  Rückenetikett entfallen. Nutzertexte und Schriftnamen nur über `typst_string`; Farben nur `white`, `DARK` oder
  `rgb("#rrggbb")` nach Prüfung.
- **Vorgabeblöcke** (`cover_layout::default_blocks`): ahmen das alte Bild nach (Autor/Titel/Untertitel bei 10 %
  Rand im Titelbereich, Rückentext ab 6 mm mit 0,45 × Rückenbreite, höchstens 11 pt; Rückseite mit 14 mm Rand,
  Strichcode unten rechts), alle Werte innerhalb der CHECKs von 0045. `effective_blocks` mischt gespeicherte und
  Vorgabeblöcke. `wrap_source` ergänzt fehlende Arten selbst (es kennt die Maße), `frame_parts` setzt nur die Blöcke,
  die der Server übergibt – der Server ergänzt sie vorher mit der **Seitengröße der Werkvorlage** (Vorlage plus
  `#hide[x]` gesetzt, erste Seite gemessen, je Stand zwischengespeichert), denn der Rahmen entsteht vor dem Buchsatz.
- **Byte-gleich ohne Anordnung**: Goldstände aus dem Stand S1 vor der ersten Änderung – `frame_parts`/`wrap_source`
  in `baruch-typst/tests/data/cover_r26/` (zehn bzw. sechs Fälle), Stand der Satzquelle und SHA-256 des Umschlag-PDF
  für sechs Fälle in `baruch-server/tests/round27_satz.rs` (ohne, Titelei, beides, Umschlag, Rücken unter/über 6 mm).
- **Höhen** (`server::cover::block_heights`): `cover::block_measure_source` setzt je Block eine Seite
  `width = Blockbreite, height: auto, margin: 0pt`; die Seitenhöhen (`baruch_render::page_sizes_pt`, aus dem SVG-Kopf)
  sind die Blockhöhen in Leserichtung. Blöcke ohne Inhalt: 0. Gemessen mit den aufgelösten Schriften.
- **Schriften** (`baruch_render::font_families`): `FontStore` merkt sich die Zahl der eingebetteten Schriften (sie
  stehen vorn); je Familie `normal` (aufrecht < 600), `bold` (aufrecht ≥ 600), `italic` (kursiv/schräg).
  `server::cover::resolve_fonts` ersetzt eine fehlende Schrift: im Buch durch nichts (die Vorlage gilt), im
  Umschlag-PDF und beim Messen durch die Schrift der Werkvorlage (`baruch_wysiwyg::template_font`: erste
  Schrift aus dem letzten `#set text(font: …)` auf oberster Ebene wie bei `sheet_style` und in Typst; Kommentare,
  Raw-Text, `#show …: set text(…)`, `#show: …`-Rümpfe und Funktionsrümpfe zählen nicht), sofern vorhanden. So stehen Blöcke in beiden PDFs mit derselben Schrift.
- **Server** (`cover.rs`, Bereich Satz): `build_wrap` = `build_wrap_for(None)` = gespeicherte Anordnung; Umschlag-PDF
  und `cover_source` nutzen sie. `frame` liest `cover_layout` und `template_functions` der Werkvorlage.
- **Titelei aus der Werkvorlage**: `template_functions` (Erkennung wie `ribbon::defined_functions`, eigene Kopie, nur
  `#let name(` mit Klammer). Mit mindestens einer Funktion (und eingeschalteter Titelei) schreibt `frame_parts` nach
  `FRAME_MARK` einmal `book_dict` (`#let baruch_buch = (…)`, 23 Schlüssel nach Vertrag 1.6 plus `impressum_zeilen`,
  nur String-Literale; Test liest das Wörterbuch zeilenweise und lässt nur Literale zu) und setzt
  `plain_page("", "name(baruch_buch)")` an die Stelle der eingebauten Seite. `imprint_template()` (vier Funktionen,
  die dasselbe setzen wie die eingebauten Seiten – Text je Seite mit `pdftotext` verglichen) und
  `imprint_template_missing()` (hängt nur Fehlendes an, sonst `IMPRINT_COMPLETE`).
- **Impressum**: `imprint_lines` nimmt die Anschriften nur auf, wenn sie gesetzt sind (Verlag nach der ©-Zeile,
  Druckerei hinter „Druck:“) – ohne Anschriften byte-gleich.
- **Druckprüfung** (`server::cover_print`): `cover_checks` nur mit eigener Anordnung (gespeichert bzw. Vorschau),
  Rechtecke aus Lage und gemessener Höhe (Rücken gedreht), Codes `bleed`, `safety`, `barcode`, `spine`, `contrast`
  (mittlere sRGB-Farbe aus 40 × 40 Stichproben im Rasterbild, Abbildung `fit: "cover"` auf Vorderseite samt
  Beschnitt wie in `wrap_source`, Abdunklung 50 % bzw. 30 %, WCAG-Verhältnis < 3), `font`; dazu `imprint_checks`
  (nur mit Titelei). Die Satz-Seite zeigt die Impressum-Warnungen in `#druckpruefung` (`typeset::imprint_warnings`).
- **Lage gemessen** (`baruch-render/tests/cover_layout.rs`): Titel und Klappentext mit `pdftotext -bbox` in Buch- und
  Umschlag-PDF, Abweichung relativ zur Schnittkante ≤ 0,5 mm; Rücken `down`/`up` (Reihenfolge der Wörter von oben
  bzw. unten, Wörter im Rücken). Mit gespeicherter Anordnung über den Server misst es die Abnahme
  (`tests/round27_abnahme.rs`).

### 27c – Editor (umschlag-widget)

- **Seite** `GET /werke/{id}/umschlag` (`src/cover.rs`, Bereiche „Seiten“ und „Umschlag-Editor“): `editor_state` holt
  `cover_layout::layout_view` (gespeichert) und mit Abfragefeldern `b.<kind>.<feld>` eine zweite Ansicht mit der
  Vorschau darüber (`blocks_from_fields`: je genannte Art der wirksame Block, darauf die Felder, geprüft wie im Store
  mit `check_cover_block`); der Satz der Vorschau läuft über `build_wrap_for` mit der gespeicherten Anordnung, je Art
  durch die Vorschau ersetzt (wie in `layout_view`), das SVG darunter trägt dieselbe Abfrage. Nichts wird gespeichert.
- **Lage** (`block_box`, Vertrag 1.2): Rahmen im Gesamtumschlag = Ursprung der Fläche (`CoverGeometry::face_origin`)
  + (x, y); Breite × Höhe = `width_mm` × `height_mm`, auf dem Rücken gedreht (`height_mm` × `width_mm`). Die Vorlage
  bekommt alle Lagen als Prozent der Umschlaggröße, darum braucht die Seite ohne JavaScript keine Rechnung im Browser.
  **Hilfslinien** (`guides`): Beschnitt (Außenkante), Schnittkante, Sicherheitsabstand je Fläche (nur, wo die Fläche
  breiter als zweimal der Abstand ist), zwei Rückenkanten, Mitte der Vorder- und Rückseite und der Höhe.
- **Formulare** (`cover::writes()`, hinter `guard::same_origin`): `POST /werke/umschlag/anordnung` (`layout_act`:
  `save` → `cover_layout::layout_set` mit allen Blöcken des Formulars, `quick` → `layout_reset([front], area)`,
  `reset` → `layout_reset(faces)`) und `POST /werke/umschlag/impressum` (`imprint_act` → `imprint_adopt`). Erfolg →
  303 auf `safe_back` mit `ok=…` (vor einem Anker, `with_code`); die Umschlagseite liest `hinweis` und `ok`. Ungültig
  → 422 und dieselbe Seite mit der Eingabe (Zahlen mit Punkt, Unlesbares wie eingegeben) und der Meldung.
- **RPC** im `call`-Teil: `cover.imprint_adopt` (Vorlage lesen, `imprint_template_missing`, dann
  `works::call("work.update", {template})` – Sperre, Vorlagenfassung und Commit bleiben dort); `work.cover_pdf` hängt
  `checks` aus `cover_print::cover_checks(work, None)` an; `work.meta.set` reicht `publisher_address` und
  `printer_address` an den Store durch (`field_label` für Commit und Oberfläche).
- **Widget** `web/src/cover_editor.ts` (`data-widget="cover-editor"`): liest `data-layout` (JSON von `cover.layout`),
  nimmt die vom Server gerenderten Rahmen und Felder und hält beide gleich (`place`). Ziehen mit Zeigerfang, Breite
  über die Griffe `start`/`end`, Einrasten (`snapDelta`, SNAP_MM 1,5 mm) an Schnittkante, Sicherheitsabstand und Mitte
  der eigenen Fläche, Tastatur (STEP_MM 0,5, mit Umschalt 5; Strg+Pfeil Breite; Strg+Z, Strg+S), Rückgängig als
  Stapel der Blockstände, „Hell/Dunkel“ für den gewählten Block, verzögerte Vorschau über `umschlag.svg?b.…`, Speichern
  über `cover.layout_set` nur mit den geänderten Blöcken (ganze Blöcke ohne `stored`/`height_mm`), Schnellwahl und
  Zurücksetzen über `cover.layout_reset` mit Neuladen. Rahmen tragen `aria-label` (Art, Fläche, Werte), die
  Statuszeile ist `role=status`. Stil in `static/cover.css` nur mit den Farb-Tokens aus `app.css` (hell und dunkel).
- **Test** `tests/round27_widget.rs`: Seite, Vorschau per GET (Store unverändert), Formulare (save/quick/reset, 422,
  403), Anschriften, Impressum-Knopf, `checks`; das Widget läuft im eigenen Node-Nachbau des DOM auf dem HTML der Seite
  (Ziehen, Griff, Pfeiltasten, Einrasten, Rückgängig, Hell/Dunkel, Feld, Speichern). Seit dem Zusammenführen prüfen
  die Tests nur noch die Wirkung; ein Platzhalter „noch nicht umgesetzt“ ließe sie scheitern.

### Integration und Abnahme (Integrator)

- Zusammengeführt mit `--no-ff` in der Reihenfolge a → b → c ohne Konflikte. Nachgezogen: die Buchvorlage in
  `baruch-store` (Unit-Test in `works.rs`, `tests/works.rs`: neue Bücher tragen `imprint_template()` und kennen alle
  vier Titelei-Funktionen), `round27_daten.rs` (ein Block ohne Inhalt hat die gemessene Höhe 0), die
  Platzhalter-Zweige in `round27_widget.rs` (entfernt) und der Commit-Rumpf beim Einlesen eines Austauschs
  (`mirror::describe_exchange` zählt die Umschlag-Blöcke mit). Nach der Sichtprüfung: Rahmen in `static/cover.css`
  nur noch schwach getönt (der markierte Block verdeckte hellen Titel auf dunklem Bild), und die Liste der
  Druckprüfung in `templates/cover.html` nennt den Block nicht doppelt (die Meldungen beginnen schon mit ihm).
- **Abnahme** `tests/round27_abnahme.rs`: ein erfundenes Werk (148 × 210 mm) mit PNG-Umschlag (dunkles oberes Drittel,
  im Test erzeugt) und vollständigen Buchangaben mit Typst-Sonderzeichen. Titel ins obere Drittel, Autor nach unten,
  Klappentext verschoben und schmaler, Freitext auf der Vorderseite: Vorschau per GET speichert nichts; das Formular
  speichert dasselbe wie `cover.layout_set` auf einer zweiten Instanz; Vorschau-SVG und Satzquelle der Vorschau
  (`baruch_render::stand`) gleichen dem gespeicherten Umschlag; `pdftotext -bbox` misst Titel, Freitext und Klappentext
  in Umschlag- und Buch-PDF an derselben Stelle relativ zur Schnittkante (≤ 0,5 mm). Druckprüfung (Beschnitt,
  Strichcode, Kontrast, fehlende Schrift mit Rückfall) auf der Seite ohne JavaScript und in `work.cover_pdf`.
  Schnellwahl oben/Mitte/unten und Zurücksetzen mit Grabsteinen; danach setzen Buch und Umschlag byte-gleich wie vor
  der ersten Anordnung. Eigene `impressum(buch)` in der Werkvorlage auf der Rückseite des Haupttitels mit den
  Buchangaben wörtlich, der Knopf ergänzt die übrigen drei Funktionen (eine Vorlagenfassung, ein Commit) und lehnt
  danach ab; fehlende Druckerei wird gewarnt. MCP liest und stellt nur einen Antrag; der Austausch (`.baruch`) trägt
  Anordnung, Anschriften und Satzbild auf eine zweite Instanz, die den Umschlag aus derselben Quelle setzt.

## Befunde (Runde 28)

Schnittstelle R28-S1 (Integrator): Migration 0046 (`finding`, `finding_status`, `post_version`, Spalten `post.edited`
und `chat_thread_post.edited`, Grabstein-Arten `finding`, `finding_status`, `post_version` mit Kaskaden-Triggern wie
0042/0045), Store `src/findings.rs` (Typen, lesende Abfragen und `finding_insert` voll, Statuswege als Stub) und
`src/post_edit.rs` (lesende Abfragen voll, Bearbeiten als Stub), Server `src/findings/{mod,parse,rpc,page,view}.rs`,
`mcp/findings.rs`, `mcp/posts.rs`, `PostAction::Edit`, `mirror::describe_finding`/`record_finding`. Offene Stubs:
`grep -rn "STUB:" crates`.

### Datenmodell, Zerlegen, Ähnlichkeit, Spiegel, Austausch u4 (befunde)

- **Datenmodell (Migration 0046):** `finding` (Werk, Abschnitt `node_id` – bei Fäden und gelöschten Abschnitten NULL
  –, Herkunft `post_id` oder `thread_post_id`, Stelle `position` eindeutig je Herkunft, Persona, Art, Text, Zitat,
  Prüfvermerk, Status, Begründung, Fassung/Ziel als UUID ohne Fremdschlüssel, `similar_to`, `summary_of` als
  JSON-Liste von UUIDs), `finding_status` (jede Änderung; Mensch `proposed = 0`, Assistent `proposed = 1` mit
  `decision`), `post_version` (alte Fassungen, Bearbeiten selbst: chat-auftrag). Die Statusregeln stehen doppelt: als
  CHECKs in der Datenbank und als Prüfung in `baruch_store::findings::resolve` (verständliche Meldung,
  `StoreError::Finding` → -32602). Fassung und Ziel löst der Store beim Lesen über die UUID auf (`adopted_rev`,
  `deferred`); fehlt die Fassung, bleibt die UUID, `adopted_rev` ist `None`.
- **Schreibwege (Store):** `finding_set_status` (nur Mensch, eine Transaktion: Befund + Verlaufseintrag; gleicher
  Stand → abgelehnt), `finding_propose_status` (nur Persona/MCP, nur adopted/deferred/done, kein Doppel offen;
  der Befund ändert sich nicht), `finding_decide` (bestätigt/abgelehnt; veraltet, wenn nach dem Vorschlag ein
  Eintrag des Menschen oder ein bestätigter Vorschlag kam, die Fassung bzw. das Ziel fehlt oder der Befund den
  Status schon hat). Server `findings/rpc.rs`: `set_status`, `propose`, `decide` halten `write_lock` vom Schreiben
  bis zum Commit, genau ein Commit über `mirror::record_finding` (`describe_finding`: Betreff „Befund <id>: …“,
  Trailer `Baruch-Finding`, `Baruch-Action: finding-status|finding-propose|finding-decide`, `Baruch-Node`, bei
  Vorschlägen `Persona`, bei Entscheidungen `Baruch-Proposer`).
- **Zerlegen (`findings/parse.rs`):** zeilenweise; Gliederung „Was trägt/fehlt/stört“ setzt die Art der folgenden
  Punkte, Marken gehen vor, Aufzählungen und (unter einer Gliederung) Absätze sind je ein Befund, eingerückte
  Folgezeilen und Unterpunkte gehören dazu, eine Leerzeile schließt. „Kritisch:“ hinter einer kurzen Teil-Angabe
  erkennt `assist::prompts::critical_finding` (dieselbe Regel wie die Prüfung). Prüfvermerke (`UNBACKED_NOTE`,
  `ref_claims::IN_TEXT`/`INVALID`) wandern in `quote_check`, Vermerke von Baruch (`CUT_NOTE`, `TRUNCATED_NOTE`,
  `CRITICAL_MISSING_NOTE`, „Komprimiert: …“) fallen weg. `ingest` nimmt nie die Schreibsperre (Aufrufer in
  `assist::save` und `threads::answer` halten sie), schreibt nur über `Store::finding_insert` und übergeht Stellen,
  die es zu diesem Beitrag schon gibt (wiederholbar). Kein Commit.
- **Ähnlichkeit:** nur Text (`parse::similar`): Inhaltswörter, klein, als Mengen – Wörter ab drei Zeichen ohne
  `STOPWORDS` (Artikel, Präpositionen samt „zur“/„zum“, Pronomen, Hilfsverben, Partikeln), dazu jede Zahl –, Dice-Maß
  ab `SIMILARITY_THRESHOLD` = 0,8 (oder gleicher Text): Bei drei oder vier Inhaltswörtern trennt schon ein
  abweichendes (0,67 bzw. 0,75), erst ab fünf darf eines abweichen. Vorher (0,6, mit Füllwörtern) galten „Der
  Übergang zur Werft fehlt“ und „… zum Hafen fehlt“ als gleich, der neue Befund verschwand als angehängt. Kandidaten: nicht angehängte Befunde desselben Abschnitts aus
  anderen Beiträgen – entschiedene vor offenen derselben Persona. Keine Einbettungen (R19a). Grenze: Umformulierungen
  mit anderen Wörtern erkennt das nicht; der Mensch kann dann verwerfen bzw. die Persona bekommt die Entscheidungen
  gerahmt (chat-auftrag).
- **Nachtrag (`findings::backfill`):** beim Start von `serve` nach Migration und Spiegelabgleich, einmal je Datenbank
  (`store_meta.findings_backfill`), nur Beiträge, deren Autor ein Handle aus `persona` ist (nicht „Autor“, nicht
  `@mcp:…`); bricht bei einem Fehler ab, ohne den Merker zu setzen (der nächste Start macht weiter, vorhandene Stellen
  werden übergangen).
- **Spiegel:** `befunde.md` (`findings::mirror_files`, in `snapshot::files` nach den Einfällen) nur bei Befunden;
  Gruppen in Baumreihenfolge (`snapshot::ordered_currents`), dann Fäden nach id, dann Befunde ohne Bezug; je Befund
  Überschrift `[B<id>] Art · Status · Persona`, Text als Zitatblock, Zitat mit Prüfvermerk, Begründung, Fassung bzw.
  Ziel, Anhang, Sammlung, offener Vorschlag, Herkunft und Verlauf. Nur Daten aus der Datenbank (deterministisch).
- **Austausch u4:** `baruch-exchange` (Modell `ExchangeFinding`, `ExchangeFindingEntry`, `ExchangePostVersion`,
  Wörter `ExchangeFindingKind`, `ExchangeFindingStatus`, `ExchangeDecision`, Grabsteinarten `befund`,
  `befund-status`, `beitrag-fassung` seit 4), Schreiber (`<befunde>` am Ende, `geaendert` und `<alte-fassung>` an
  Beiträgen), Leser (erst ab `version="4"`, davor unbekannt; Grabsteine ab 4), Prüfung (`check_findings`, Regeln wie
  die CHECKs), XSD `baruch-werk-1-u4.xsd` (u3 eingefroren). Store `exchange_dump` (Verweise über UUIDs) und
  `exchange_apply::apply_findings` (neu: alles mit Kennungen; abgleichen: unbekannte Befunde dazu, bekannte nie
  überschreiben, belegte Stelle → Hinweis, `gleicht` nach allen Befunden), `insert_versions` für alte Fassungen.
  Einzelheiten in `docs/Austauschformat.md`.

### Übersicht und Anzeige (befund-uebersicht)

- **Grenze der Listen:** `Store::finding_list` liefert höchstens `limit` (Vorgabe 500, höchstens 2000) und dann die
  **neuesten** (`ORDER BY created DESC, id DESC`, umgedreht: weiter älteste zuerst) – Matrix, Entscheidungen im Prompt
  und Ähnlichkeitskandidaten verlieren über der Grenze die ältesten, nicht die jüngsten. `Store::finding_count` zählt
  mit demselben Filter ohne Grenze: `finding.list`/MCP `findings` melden `total` (alle) und `truncated`, die Matrix
  `truncated` (Zahl aller, sonst `null`) und auf der Seite „Gekappt: …“. `finding.get` lädt Angehängte und Ursprünge
  gezielt (`finding_attached_to`, `findings_by_uuid`).
- **Zählregel an einer Stelle:** offen = `Store::finding_open_counts` (Vertrag §1.5). `findings::view::subtree_open`
  summiert diese Zahlen über die Baumreihenfolge (Elternteil vor Kind) – daraus zeichnen Gliederung (`view.rs`,
  `TreeItem.findings`), Editorkopf (`node.open_findings`), `/gliederung` (`outline.rs`, `entries_with`) und die
  Gedankenkarte; `finding.matrix` nennt je Zeile `open_badge` aus denselben Zahlen und ohne Filter gilt `total.open ==
  open_badge` (Test `round28_uebersicht.rs`). `menu.badges.open_findings` ist ihre Summe.
- **`findings::view`:** `load` holt einmal je Seite die offenen Zahlen, alle Befunde des Werks (samt Lob und
  angehängten, höchstens `VIEW_LIMIT` = 2000) und je Abschnitt mit übernommenen Befunden `findings_adopted_in` (für
  den Fassungsbaum: `adopted_in`, offene Vorschläge: `proposed_for`). `card` baut das gemeinsame Bild eines Befunds
  für Seite und Akkordeon (`finding_json` plus Zeichen, Abschnitt, Zeit, angehängte, Ursprünge eines Sammelvorschlags
  über `summary_of`-UUIDs, offener Vorschlag in Worten, Fassungsauswahl). Status-Zeichen: ○ ✓ ✗ → =.
- **`findings::page`:** `build_matrix` ist rein (testbar); Zeilen = Kapitel-Einheiten (nächster Vorfahre der Ebene
  ≤ 1), Gruppen = Teile, „ohne Abschnitt“ am Ende. Die Seite hat eine eigene Vorlagen-Umgebung (`base.html`,
  `befunde.html`, `fragments/findings_panel.html`); die Karten- und Formular-Makros stehen in
  `fragments/findings_panel.html` und werden von `befunde.html` importiert (ohne `findings_panel` im Kontext zeichnet
  die Vorlage selbst nichts). Formulare gehen über `findings::rpc::{parse_change, set_status, decide}` (Sperre und
  Commit dort); die Seite nimmt selbst keine Sperre.
- **Akkordeon:** `view.rs::findings_panel` filtert `FindingsView.findings` auf den gezeigten Abschnitt (offen bzw. mit
  offenem Vorschlag, entschieden, Lob); `ViewExtras.findings` füllt `view::extras` mit `unwrap_or_default` (ein Fehler
  beim Laden lässt die Arbeitsseite ohne Befunde stehen). Cookie `baruch_accordion=befunde` hält es offen.
- **Gedankenkarte:** `MapFilter.findings` (Seite `befunde=1`, RPC `findings`), `MapNode.open_findings` und
  `MapSection.open_findings` werden in `idea_map::build` wie die Einfälle über Anker und Vorfahren aufsummiert; `svg.rs`
  setzt `im-many-findings` ab `findings::page::MANY_OPEN` (3) und schreibt „k offen“ als `<tspan class="im-findings">`
  (DOM-Vertrag ergänzt, nicht geändert).
- **Widget `findings`** (`web/src/findings.ts`): Statusformulare über RPC (danach `baruch:navigate` bzw. Neuladen des
  Inhalts), auf der Seite Filter ohne Neuladen (fetch derselben Adresse, Tausch von `[data-findings-main]`,
  `history.pushState`), Pfeiltasten in der Matrix. Ohne JavaScript bleibt alles bei Links, GET-Filtern und Formularen.
- **MCP** (`mcp/findings.rs`): `findings` über `Store::finding_list`, `finding_propose_status` prüft Status und Felder
  selbst und ruft `findings::rpc::propose` mit `tools::origin_of` (Persona nur ohne Token).

### Chat-Aufträge, Rahmen, Beitragsfassungen (chat-auftrag)

- **Lektor als Begleiter** (`assist/prompts.rs`): `CRITICAL_HANDLES = ["nathan"]`; Nachforderung (`answer_call`) und
  `CRITICAL_MISSING_NOTE` nur noch für Nathan. Die Vorgabe des Lektors besteht aus `LECTOR_RULE` (6: zuerst „Was
  trägt:“ mit Zitat, höchstens zwei „Vorschlag:“, kritische Befunde erlaubt; mit derselben Ausnahme für neue
  Fassungen wie `EVIDENCE_RULE`/`CRITICAL_RULE`, denn der System-Prompt hängt nicht vom Modus ab), `BACKING_RULE` (7: Belegpflicht, für jede
  Rolle mit kritischen Befunden) und `COMPANION_RULE` (8–9: Chat, Entscheidungen, RFC); der Exeget hat
  `EVIDENCE_RULE` + `BACKING_RULE`, Nathan `CRITICAL_RULE` + `BACKING_RULE` (wörtlich wie bisher). Die Regeln sind
  knapp gehalten: Jedes Zeichen des Rollen-Prompts fehlt dem Kapitel im Kontext (`round18_lektor.rs` und
  `round15_lektor.rs` messen die Grenzen). `mark_unbacked_criticals` gilt unverändert für alle.
- **Rahmen** (`assist/context.rs`): `decision_lines` liest über `Store::finding_list` die Befunde des Abschnitts bzw.
  der Kapitel-Einheiten mit Status übernommen, verworfen, verschoben, erledigt (ohne Lob, ohne angehängte), verworfene
  zuerst, höchstens `MAX_DECISIONS` = 30 (darüber eine ehrliche Schlusszeile), je Zeile `- [B<id>] vom Autor verworfen
  (@x zu Titel): <<<entscheidung „…“ (Begründung: …) >>> – nicht erneut melden`, entschärft wie `framed_line`.
  Dazu im System `DECISIONS_RULE` (und `OTHERS_RULE`). Beim RFC rahmt `rfc_lines` die Antworten der anderen
  (`<<<antwort … >>>`, Kopf `@x [B1] [B2]`) unter `RFC_TITLE`, der Auftrag bekommt `RFC_TASK`; die Beiträge dieser
  Antworten fallen aus dem Chatverlauf (nicht doppelt). Der Chatverlauf (`RECENT_TITLE`) steht im Chat und beim
  Schreibauftrag (`frame.as_order`, Auftrag mit `AS_ORDER_TASK`). Beim Kürzen fallen Antworten und Entscheidungen
  zuletzt und nie still (`truncated`). `FOREIGN_KINDS` enthält `entscheidung` und `antwort`: Sie belegen keinen
  „Kritisch:“-Befund (Korpus der Zitatprüfung ohne sie).
- **Chat-Aufträge** (`assist/chat.rs`): `is_rfc` (erstes Wort „RFC“ oder „@alle“), `is_write_order` (erstes Wort nach
  Erwähnungen und „bitte“ ist ein Imperativ aus `WRITE_VERBS`), `rfc_personas` (aktive Personas in ihrer Reihenfolge,
  der Lektor nach `personas::rows_and_lector` herausgenommen). RFC: je Persona `run` im Modus Chat, die Antworten als
  `FramedAnswer` (Text, Beitrag, Befund-ids aus `Outcome::findings`), zuletzt der Lektor mit
  `OrderFrame { rfc_answers, rfc_summary: true }` (seine Befunde werden über `findings::ingest` Sammelbefunde).
  Schreibauftrag: `run` im Modus `Propose` mit `frame.as_order`, danach unter der Schreibsperre der kurze Beitrag
  `order_note(rev)` (keine Befund-Marke am Anfang, ergibt beim Zerlegen keinen Befund) und für jedes `[B<id>]` aus
  Nachricht oder Vorschlag `findings::rpc::propose(… Adopted { rev } …)` (eigene Sperre und Commit; Fehler in
  `notes`) – nur wenn `finding.key == order.key`: `rev` ist eine Nummer im Abschnitt des Auftrags, `resolve` sucht sie
  aber im Abschnitt des Befunds; ein Befund aus einem anderen Abschnitt bekommt darum nur einen Hinweis in `notes`. Fäden (`threads.rs`) nutzen `is_rfc`/`rfc_personas`; `thread_call` nimmt die gerahmten Antworten,
  `answer` zerlegt die Antwort des Lektors mit `summary = true`. Die Relay-Stufe von MCP entfällt beim RFC.
- **Beitragsfassungen**: Store `post_edit.rs` (`post_edit`, `thread_post_edit`: eine Transaktion `BEGIN IMMEDIATE`,
  alte Fassung als `post_version` mit `valid_from` = Anlage bzw. vorige Änderung, `edited` = `replaced`; leer,
  mehr als 4000 Zeichen oder unverändert → `StoreError::PostEdit`). Server: `posts::edit` (RPC `post.edit`, Formular, MCP
  `post_edit`) prüft unter der Schreibsperre, dass der Handelnde der Autor ist (Mensch „Autor“, MCP `@mcp:<name>`),
  und schreibt genau einen Commit über `mirror::describe_post(…, PostAction::Edit, Some(&before), …)`;
  `threads::edit_post` ohne Commit. Volltextindex: Trigger `post_fts_edit` (Bibliothek 0107, `AFTER UPDATE OF body ON
  post`).
- **Oberfläche**: `posts::visible_range` zeigt die neuesten `CHAT_PAGE_POSTS` = 50 (`view.rs` liefert `post_older`);
  RPC `post.page` für das Nachladen. Seiten `/beitraege` und `/beitrag` (eigene Template-Umgebung in `posts.rs`,
  Vorlagen `posts.html`, `post.html`). CSS: `app.css` begrenzt `.chat` (`max-height`, `overflow-y: auto`,
  `user-select: text`) und stellt `.acc[data-section="chat"] .acc-body` von `overflow: hidden` auf `auto` – das war der
  Blocker beim Kopieren und Scrollen (ohne begrenzte Höhe schnitt der Bereich ab; wo `::details-content` nicht
  greift, reichte die Flex-Kette die Höhe nicht weiter). Bausteine in `chat.css`, Fäden in `threads.css`
  (`.thread-scroll`). Widget `chat` (`web/src/chat.ts`): `enableCopy` (Clipboard-API, Rückfall `execCommand`),
  Zitieren (`quoteLine`, einzeilig), „Neue Beiträge ↓“, „Ältere Beiträge“ über `post.page`; Widget `chat-page` für die
  beiden Seiten, `threads.ts` nutzt `enableCopy`. Knöpfe, die nur mit JavaScript wirken, stehen mit `hidden` im HTML.
- Grenze (ehrlich): Ein Schreibauftrag, der einen **neuen** Abschnitt verlangt („Erstelle ein Vorwort“), wird ein
  Vorschlag für den gezeigten Abschnitt; das Anlegen neuer Abschnitte aus dem Chat heraus ist nicht umgesetzt (nur
  über die Gliederung bzw. MCP `outline_add`) – Rest für den Backlog. Ob Kopieren im echten Browser vorher an etwas
  anderem als dem abgeschnittenen Bereich scheiterte, ließ sich ohne Browser (Teamregel: kein Firefox) nicht
  nachstellen; geprüft sind HTML, CSS und Skript.

## Schnittstelle Runde 30 (R30-S1)

Gemeinsame Grundlage der drei Aufgaben von Runde 30 (Vertrag §1), danach füllt jeder nur seine Dateien:

- Migration `0047_r30.sql` (reines Schema, eingefroren): Metadaten an `post` und `chat_thread_post` (`basis_rev`,
  `basis_label`, `reply_to`, `repeat_of`, `relay_from`, `request`; Verweise mit ON DELETE SET NULL),
  `finding.solutions`, `revision.variant_*` (Gruppe und Etikett beide oder keins, geprüft im Code), Tabelle
  `replace_batch` (lokales Protokoll, keine uuid). Reserviert: Store 0048–0051, Bibliothek 0108/0109.
- Store, voll umgesetzt: `Store::add_post_meta`/`post_meta*`/`thread_post_meta*` (`post_meta.rs`),
  `Store::add_proposal_on`/`variant_group` (`variants.rs`, `write::add_proposal_on`), `Store::finding_set_solutions`,
  `Store::replace_batch`; `replace_apply`/`replace_undo` sind Stubs (`StoreError::Replace` → RPC `-32000`).
- Server-Haken: `assist::save` speichert Persona-Beiträge mit Metadaten (geprüfte Fassung, Auftrag, Verknüpfungen aus
  `Order::links`); `assist::chat` wählt eine Prüfgrundlage für alle Personas (`basis::choose`), ruft
  `variants::is_variants_order`/`run` und nach dem Lektor beim RFC `solutions::rfc_proposal`; `view::posts` hängt je
  Beitrag `extra` (`chat_render::post_extra`) und `solutions` an, der Fassungsbaum `variant`
  (`variants::revision_variant`); `mirror::record_request` spiegelt fertige Commit-Nachrichten der neuen Module.
- Weiterleitung `search.*`, `xref.*`, `solution.*` in `rpc.rs`, Routen in `lib.rs` (schreibende hinter
  `guard::same_origin`), MCP `text_search` und `solution_variants` (Stubs, `solution_variants` zählt zur Quote),
  Menüeinträge unter Werkzeuge, leere Vorlagen-Makros (`search_bar`, `post_item`, `finding_block`, `post_solutions`,
  `basis_select`) und TypeScript-Module (`search`, `search_keys`, `jump`, `xref`, `variants`).

## Suchen und Ersetzen (Runde 30a)

- **Server `src/search/`:**
  - `mod.rs`: Typen (`SearchQuery`, `SearchHit`, `SearchResult`, `TextKind`, `SearchRange`, `RevChoice`), Bereiche
    (`sections_in`: Abschnitt, Kapitel = nächster Kapitel-Vorfahr samt Unterpunkten, Teil, ab Abschnitt, Auswahl,
    Werk, alle Werke), Text je Abschnitt (aktuelle bzw. gezeigte Fassung, ungespeicherter `text`, Auswahl in
    UTF-16), `find`/`find_within` (Zeitgrenze als Parameter für Tests) und RPC-Weiterleitung.
  - `scan.rs`: Textarten je Byte (`classify`): Überschriftstitel, Fußnoteninhalt, Zitate über
    `crate::quotes::quote_spans`, Code (`#name`, Argumente, Schlüsselwortzeilen, Raw, Formeln, Kommentare, Labels,
    `@verweis`), sonst Text. Ein Treffer über mehrere Arten zählt als die strengste (Code > Zitat > Fußnote >
    Überschrift > Text). Dazu das Muster (`pattern`: Crate `regex`, `size_limit(1<<20)`, `dfa_size_limit(1<<22)`;
    im Wortlaut-Modus findet jedes Sonderzeichen auch `\\x`), Treffer mit Ersatz (`find_in`; Regex:
    `Captures::expand`, Wortlaut: `escape_literal`) und UTF-16-Umrechnung.
  - `replace.rs`: `preview` (Diff je Abschnitt als HTML-Zeilen mit `<del>`/`<ins>`, alles maskiert, `body_after`),
    `replace` und `undo` unter der Schreibsperre; je ein Commit über `mirror::record_request` mit
    `Baruch-Action: replace` bzw. `replace-undo` und `Baruch-Replace: <id>`.
  - `page.rs`: `GET /suche`, `POST /suche/act` (eigene Vorlagen-Umgebung `templates/search.html`).
- **Ablauf beim Ersetzen:**
  - Der Server sucht noch einmal auf der aktuellen Fassung. Die Reihenfolge der Treffer ist dieselbe wie in der
    Vorschau, `skip` zählt in ihr.
  - Er rechnet die neuen Texte und übergibt sie mit der Basis aus der Vorschau an `Store::replace_apply`.
  - Der Store prüft in **einer** Transaktion je Abschnitt unter einem eigenen `SAVEPOINT`: Ist die aktuelle Fassung
    noch die Basis, entsteht eine neue Fassung vom Menschen (`write::save_human`), danach folgt
    `outline::check_current_heading`. Andere Ebene, weitere oder keine Überschrift → nur dieser Abschnitt wird
    zurückgerollt und mit Grund übersprungen.
  - Am Ende schreibt er eine Zeile `replace_batch` (Optionen als JSON, `items` = `[{key, prev_rev, new_rev}]`).
- **Rückgängig:** `Store::replace_undo` schaltet je Eintrag mit `write::set_current` + `outline::follow_switched`
  zurück, nur wenn `new_rev` noch aktuell ist, und setzt `undone`. `Store::replace_latest` liefert das jüngste offene
  Ersetzen für `/suche`. `replace_batch` ist lokal: keine uuid, nicht im Austauschformat.
- **Widgets:**
  - `web/src/search_keys.ts`: reine Funktion `keyAction(e, ctx)`, unter Node geprüft.
  - `web/src/search.ts`: Suchleiste `search-bar` aus `fragments/search_bar.html`, Werk-Seitenleiste, Listener am
    Dokument in der Capture-Phase. Treffer im Satzbild über die CSS Custom Highlight API (`baruch-search`,
    `baruch-search-current`). Der Ersatz im Abschnitt geht über `search.preview` mit `text=` und ändert nur das
    Textfeld.
  - Stil `static/search.css`.
- **MCP** `text_search` (`src/mcp/search.rs`): `crate::search::find` mit Bereich `section|chapter|part|from|work`,
  Vorgabe 100 Treffer, kein Schreibweg.

## Querverweise (Runde 30a)

- **`baruch_typst::xref`** (rein):
  - `label_heading`/`label_section` projizieren die Sprungmarke: ` <kN>` am Ende einer `=`-Zeile vor einem
    Kommentar, bei eigenem Label (Teil, „ohne Nummer“, Label am Zeilenende oder allein darunter) die Ankerzeile
    `#metadata("baruch-key") <kN>` darunter.
  - `strip_key_label`/`strip_section`/`strip_key_labels` entfernen sie wieder.
  - `find_xrefs` findet `#link(<kN>)[…]` samt Seitenzahl-Zusatz, `remap_keys` setzt Schlüssel um, `xref_snippet`
    baut die Form.
  - `part_anchors_for_typesetting` dient nur dem Satz mit Teil-Seiten, siehe unten.
- **Store:**
  - `snapshot::export_parts` und `snapshot::files` setzen nach der Projektion „Nummer an/aus“ `label_section` ein.
    Damit tragen Export, Export-Karte (byte-gleich), Spiegel und Satz die Sprungmarken, SQLite nicht.
  - `import.rs` entfernt sie je Überschrift (`strip_section`), legt erst alle Knoten an und schreibt dann die Texte
    mit `remap_keys` (alt → neue `node.id`). Die Prüfsumme gegen doppelten Import bleibt die der Datei.
- **Anker eines Teils im Satz (Messung `baruch-render/tests/xref.rs`):**
  - Die Teil-Regel (`part_page_rule`) bricht nach dem Teil um. Eine Ankerzeile hinter der Teil-Zeile läge darum auf
    der Folgeseite, und `counter(page).at(<kN>)` nennte die falsche Seite.
  - `typeset::part_pages` setzt deshalb im Satz `part_anchors_for_typesetting` ein. Die Ankerzeile wandert vor die
    Teil-Zeile, mit eigenem `#pagebreak(weak: true, to: "odd")`. Der Umbruch der Regel entfällt danach, weil die Seite
    nur den Anker trägt (gleiche Seitenzahl gemessen).
  - Export und Spiegel bleiben bei der Ankerzeile darunter (ohne Teil-Regel liegt sie auf der Seite der
    Überschrift). Zeilenzahl und Bytebereiche der Karte zieht `typeset::rewrite_sections` nach.
- **Kaputte Verweise:**
  - `xref::broken_in` meldet Verweise auf Schlüssel ohne aktuelle Fassung.
  - `typeset::xref_safety` macht sie im Satz zum bloßen Linktext; sonst scheiterte der ganze Satz an „label does not
    exist“.
  - Die Druckprüfung (`typeset.html`, `li.typeset-xref`) nennt sie.
- **baruch-wysiwyg:**
  - `Inline::Xref {key, c, page}`; `parse` erkennt `#link(<kN>)[…]` samt Zusatz mit demselben Schlüssel.
  - `emit` schreibt `xref_snippet`, `render_html` einen `span.ws-xref` mit `data-key`/`data-page` und einen nicht
    bearbeitbaren `span.ws-xref-page`.
  - `wysiwyg.ts` liest den Block zurück (`t: "xref"`), fügt ihn über den Befehl `xref` (`ws_command.ts`) ein und
    markiert `baruch:find` mit dem Highlight `baruch-find`.
- **Server `src/xref.rs`:**
  - `jump_url`, `check_marker` (Schlüssel, Zitat mit vereinheitlichtem Leerraum, `at` = Byte des Zitats);
    `outline_mentions` (Zählnennungen über ein Muster mit Ordnungszahlen eins bis zwölf, römisch bis XXXIX,
    arabisch und Nummern wie 2.3; Titel ab vier Zeichen als ganze Wörter mit Artikel, kürzere nur in
    Anführungszeichen; eindeutig oder gar nicht, gezählt nur Punkte mit Nummer).
  - `candidates_in` nimmt nur Fließtext (`search::scan::classify`), nicht in vorhandenen Verweisen, nicht den eigenen
    Abschnitt.
  - `convert` legt über `Store::save_human_on` eine Fassung auf `rev` an und spiegelt sie mit `mirror::record`.
  - RPC `xref.*`, Seiten `/verweise` und `/verweise/act` (Vorlage `templates/xrefs.html`).
- **Widgets:**
  - `jump.ts` (ohne Importe; reine Funktionen `pushJump`/`popJump`/`forwardJump`, `byteToUtf16`, `locate`, `findOf`
    unter Node geprüft): `a[data-jump]`, Verlauf `sessionStorage` `baruch_jumps`, Sprungleiste,
    `baruch:find {text, at}` nach dem Laden und nach `baruch:rendered`.
  - `workspace.ts` meldet `baruch:rendered` nach jedem Neuzeichnen; Alt+← geht über `jumpBack()`, wenn Verlauf da ist.
  - `xref.ts`: Dialog über `baruch:xref-open` aus `ribbon.ts`, `localStorage` `baruch_xref_page`, Strg+Klick und
    „Öffnen | Bearbeiten“ (Menü fest im Dokument, nicht im Satzbild).
  - Menüband `ribbon::formats` hat `cmd = "xref"`.

## Lösungsvorschläge und Varianten (Runde 30b)

- **Lösungsform** (`assist/solutions.rs`): `parse_solutions` liest `statt: „…“` + `besser: „…“`, `Schritt: …` und die
  Kurzform `statt „…“ besser „…“` (auch mitten in einer Zeile, auch nummeriert); `is_solution_line` erkennt eine
  Lösungszeile. `findings::parse::split_answer` hängt Lösungszeilen an den Befund darüber (gleich wie eingerückt, auch nach einer Leerzeile – erst eine andere Zeile schließt den Befund),
  nimmt sie aus dem Befundtext und liest den Schlussblock „Mein Vorschlag:“ nicht als Befunde; `finding_spans` liefert
  die Bytes je Befund samt Lösungszeilen (für die Blöcke im Chat, Runde 30c). `findings::ingest` speichert die
  Lösungen je Befund (`Store::finding_set_solutions`, Spalte `finding.solutions` aus 0047); Fehler dort nur auf
  stderr, der Befund bleibt.
- **Lösungspflicht** (`assist/prompts.rs`, `assist/mod.rs`): `SOLUTION_RULE` steht in jeder Vorgabe.
  `answer_call` fordert nach dem Pflichtbefund (Nathan) einmal je Auftrag nach, wenn `missing_solutions` > 0
  (`Retries { critical, solution }`, Grund `SOLUTION_RETRY_REASON`); die zweite Antwort gilt nur, wenn ihr nicht mehr
  Lösungen fehlen und sie den Pflichtbefund behält. `mark_missing_solutions` setzt danach „(ohne Lösungsvorschlag)“
  ans Ende der ersten Zeile jedes kritischen Punkts ohne Lösung (vom Modell geschriebene Vermerke werden entfernt und
  neu geprüft; idempotent). Abgeschnittene Antworten: weder Nachforderung noch Vermerk. Der Vermerk wandert beim
  Zerlegen in `quote_check`.
- **Übernehmbar und Einsetzen**: `locate` sucht das Zitat wörtlich, sonst mit beliebigem Leerraum (Regex nur aus
  maskierten Wörtern, `size_limit`); `apply_solutions` setzt der Reihe nach je das erste Vorkommen ein und meldet ein
  fehlendes Zitat. `post_solutions(body, shown)` ist der Haken in `view::posts` (`post.solutions`).
- **RPC** `solution.*` (`assist/solutions.rs::call`, weitergeleitet von `rpc.rs`): `adopt` legt über
  `Store::add_proposal_on` (Basis = gezeigte bzw. Prüfgrundlage des Beitrags `PostMeta::basis_rev`, beim Befund die
  aktuelle) einen Vorschlag mit Herkunft der Persona an und schlägt über `findings::rpc::propose` „übernommen in
  Fassung n“ vor (nur Befunde dieses Abschnitts). `reject_siblings` verwirft unter der Schreibsperre die übrigen
  offenen Vorschläge der Gruppe (`Store::reject`, kein Spiegel-Commit, wie `revision.reject`). `compare` liefert je
  Variante den Zeilen-Diff zur Basis und zu den übrigen als maskiertes HTML (`diff_html`, `baruch_core::line_diff`).
  Seiten: `GET /varianten` (eigene Umgebung mit `base.html`, `variants.html`; ohne `key` bestimmt
  `Store::variant_group_key` den Abschnitt) und `POST /loesungen/act` (Paare aus dem Formular, `picks`/`findings`
  mehrfach, 303; Fehler → 200 mit `role="alert"`), hinter `guard::same_origin`.
- **RFC** (`solutions::rfc_proposal`, Haken in `chat.rs`): `proposal_block` liest die Lösungen nach „Mein Vorschlag:“;
  höchstens drei übernehmbare werden in die Prüfgrundlage eingesetzt, ein Vorschlag (Notiz „Mein Vorschlag von
  @lektor (Beitrag n)“); `[B<id>]` im Block ergeben Statusvorschläge.
- **Prüfgrundlage** (`assist/basis.rs`): `choose_in` wählt gezeigt/aktuell/n; „gezeigt“ auf einer aktuellen Fassung
  nur mit Überschrift (`heading_only`) nimmt den neuesten Vorschlag mit Text, ausdrücklich „aktuell“ bleibt mit
  Hinweis in der Kopfzeile. `assist/rpc.rs::order` liest `basis` für ask/review (propose nicht), `run_inputs` wählt
  sie für Aufträge ohne Grundlage (MCP `ask_persona`, Fäden, Einfälle) – die Adresse bleibt ohne `rev`, solange die
  aktuelle Fassung geprüft wird. `Outcome.basis` (JSON `basis` bei ask/review) und `PostMeta::basis_label` tragen sie.
  Der Auftrag sagt bei einer Fassung nur mit Überschrift `HEADING_ONLY_TASK`.
- **Kontext** (`assist/context.rs`): `outline_block` mit `[key:N]`; bei „Chat nur Abschnitt“ steht er als Umfeld da,
  sonst kommt er (samt `JUMP_RULE` im System) nur dazu, wenn danach noch `OUTLINE_SLACK` Zeichen frei sind – er
  verdrängt keinen Text und löst keine Stückelung aus. Fremde Befunde: der Lektor unter `FINDINGS_TITLE`, alle anderen
  unter `KNOWN_TITLE` mit `KNOWN_RULE` (nicht kommentieren). `OrderFrame.relay` steht gerahmt und entschärft
  (`relay_line`, Art `antwort`, zählt nicht zum Korpus) mit `RELAY_TASK`. `RFC_TASK` verlangt „Mein Vorschlag:“.
- **Varianten** (`assist/variants.rs`): Lektor nach Einstellung, Befunde genannt (nur dieses Abschnitts) oder die
  offenen des Abschnitts (ohne Lob, ohne angehängte), höchstens `MAX_VARIANT_FINDINGS`; ohne Befunde ehrliche
  Ablehnung. Je Variante ein eigener `ModelCall` (System: Lektor-Prompt, `OTHERS_RULE`, `VARIANT_RULE`; Befunde mit
  Lösungen gerahmt; Richtung `VARIANT_STYLES[k]` und die schon geschriebenen Zeilen), nacheinander über
  `ask_model`. Passt der Abschnitt samt Antwort nicht in die Kontextlänge, gehen nur die betroffenen Absätze (Zitat
  eines Befunds bzw. einer Lösung, nie die Überschrift) mit Marken `[[Absatz n]]` hinaus (`PARTIAL_RULE`); `splice`
  setzt die Antwort ein. Gespeichert wird erst am Ende unter einer Sperre (`add_proposal_on` mit `VariantInfo`,
  Gruppe `new_uuid`). Chat-Auftrag: `is_variants_order` (Wort „Varianten“, keine Frage). Fassungsbaum:
  `revision_variant` (JSON mit `title`), Etiketten in `revtree.html`, „übrige verwerfen“ in `revisions.html`.
- **Vorlagen**: `fragments/finding_block.html` (Befund als Block: Art mit Zeichen und Wort, Status mit Knöpfen an
  `/werke/befunde/act`, Lösungen, „Als Vorschlag übernehmen“), `fragments/post_solutions.html` (Lösungen eines
  Beitrags, „Lösungen umsetzen“), `fragments/basis_select.html` (in `assist_scope.html`). Die Karten
  (`findings::view::card`) tragen dafür `body_html` (über `chat_render::markdown_html`), `solution_views`,
  `adoptable`, `work`, `back`; die Matrix zählt `solved`/`unsolved`.
- **Austausch Unterversion 5**: siehe `docs/Austauschformat.md` §6; `exchange_apply::apply_u5` setzt nach dem Einlesen
  auf den neu angelegten Zeilen (ids über dem Stand vor dem Lauf) Varianten, Metadaten und Lösungen und ruft
  `baruch_typst::xref::remap_keys` mit der Zuordnung `schluessel` der Datei → Kennung → Schlüssel hier. Beim Abgleich
  (`apply_merge`, Schritt (c)/(d)) schlüsselt Baruch den Text einer Fassung **vor** dem Vergleich mit der hiesigen
  um (dieselbe Zuordnung über alle Punkte des Werks): Eine Fassung, die sich nur in den Schlüsseln der Querverweise
  unterscheidet, ist nicht abweichend; eine wirklich abweichende Fassung (abgeleitete Kennung) wird mit den hiesigen
  Schlüsseln angelegt, denn `apply_u5` erreicht sie über die Kennung der Originalfassung nicht.

## Chat: Wiederholen, Löschen, Anrede, Live, gegliederte Beiträge, Sprungmarken (Runde 30c)

**Daten.** Je Beitrag im Abschnitts-Chat (`post`) und im Faden (`chat_thread_post`) Metadaten aus Migration 0047
(`baruch-store/src/post_meta.rs`): `basis_rev`/`basis_label` (geprüfte Fassung und Kopfzeile), `reply_to`,
`repeat_of`, `relay_from` und `request` (`PostRequest {persona, mode, scope, text, rev}`, lokal, nicht im
Austauschformat). Persona-Beiträge im Abschnitt schreibt `assist::save` über `Store::add_post_meta` (aus
`Order::links`/`Order::basis`), im Faden `threads::answer` über `Store::thread_add_post_meta` (eine Transaktion,
`scope = "thread"`, Auftragstext = Nachricht des Absenders). `Store::thread_post_delete` löscht einen Fadenbeitrag;
Verweise darauf werden durch die Fremdschlüssel NULL (`ON DELETE SET NULL`), Verlauf und Befund-Verweise folgen 0046.

**Anrede und Relay** (`assist/chat.rs`): `addressed(message, personas)` = `@handle` überall plus Name/Handle ohne „@“
am Satzanfang (Beitragsanfang, nach Zeilenumbruch, nach „.“/„!“/„?“ mit Leerraum) mit „,“/„:“; `mentions` bleibt für
MCP-Relay und Bestand. `line_addresses(answer, personas)` = Anreden am Zeilenanfang einer Antwort (auch hinter
Aufzählungszeichen und `**`, nicht in „> “-Zitaten). `chat()` sammelt die Antworten der ersten Runde und ruft danach je
Anrede eine Relay-Antwort: `Order { mode: Chat, text: relay_text(..), asker: anredende Persona, frame.relay: deren Antwort
gerahmt, links: {reply_to, relay_from} }`. Grenzen `MAX_RELAY_ANSWERS` (3), jede Persona einmal (`called` umfasst alle
Gerufenen samt gescheiterten), eine Stufe, nicht bei RFC/Schreibauftrag. Fäden (`threads::chat`) nutzen dieselben
Funktionen; das MCP-Relay aus Runde 15 (jede Erwähnung, Gesamtgrenze 3) bleibt daneben bestehen (seit der
Nachbesserung nach R30 in derselben Stufe und ebenso gerahmt, siehe „Relay“ unter Runde 15), `mcp/tools.rs`
`relay_section` rief im Abschnitt danach nur noch Personas, die nicht schon dran waren (seit Runde 23 entfernt: ein
Relay-Weg `assist::chat::relay_targets`).

**Wiederholen** (`posts::repeat`, `threads::repeat_post`): Auftrag aus `PostMeta::request` (Rückfall:
`chat_render::repeat_fallback` bzw. `threads::thread_fallback` – vorangehender Beitrag eines Nicht-Persona-Autors),
Prüfgrundlage über `assist::basis::choose`, Ausführung über `assist::run` bzw. `threads::answer` mit
`links.repeat_of`. Danach `posts::propose_done`: offene, nicht-lobende Befunde des alten Beitrags, zu denen
`findings::parse::split_answer(neue Antwort)` keinen `similar` liefert, bekommen über `findings::rpc::propose` den
Vorschlag `Done` (Herkunft die Persona, je ein Commit). Verglichen wird der Text der neuen Antwort, nicht die neu
angelegten Befunde – `findings::ingest` hängt Wiederholungen eines Befunds an den alten an (`similar_to`).

**Löschen:** Abschnitt wie bisher `posts::act(Op::Delete)` (ein Commit `post-delete`); Faden `threads::delete_post`
unter der Schreibsperre mit `Actor::only_own` (MCP: `delete_post_as`), Commit über `mirror::record_request` mit
`allow_empty` (Protokoll, der Baum ändert sich nicht).

**Live** (`chat_live.rs`): prozessweite Tabellen im Arbeitsspeicher je (Werk, Abschnitt bzw. Faden): `typing()` /
`typing_thread()` tragen ein und geben eine `TypingGuard`, deren `Drop` austrägt (Ende, Fehler oder Abbruch des Futures);
`assist::run_inputs` und `threads::answer` halten sie während des Auftrags. `record_failure()` merkt gescheiterte Aufrufe
(aus `chat::fail`, `routes::remember_failure` für Einzelaufträge, `threads::answer`, `posts::repeat`), höchstens fünf je
Ort, zehn Minuten. `post.since`/`thread.since` rendern neue Beiträge mit denselben Makros wie die Seite
(`fragments/post_item.html`: `post_entry` bzw. `thread_entry`, über `Environment::render_str`), dazu `typing_now` und
`failures_now`. `web/src/chat.ts` (`startLive`) fragt alle 3 s bei offenem Akkordeon und sichtbarer Seite ab und sofort
auf `baruch:chat-refresh` (assist.ts am Ende eines SSE-Stroms); threads.ts nutzt dieselbe Funktion.

**Darstellung** (`chat_render.rs`): `markdown_html` (Signatur fest, auch für Befund-Akkordeon und Matrix) zerlegt in
Blöcke (Absatz, Liste mit Unterpunkten, Zitat, Überschrift) und eine Fußzeile (Schlusshinweise von Baruch:
`scripture::MARK_*`, `CUT_NOTE`, `TRUNCATED_NOTE`, „Komprimiert:“, `CRITICAL_MISSING_NOTE`); Inline nur `**`/`*`, alles
über `bibles::html_escape`. `render_post` legt Links an Byte-Bereichen der Zeile an (Reihenfolge Marken → Einfall/Befund
→ Bibelstellen → Zitate → `xref::outline_mentions`, keine Überschneidung) und maskiert den Rest; Sprungmarken prüft
`xref::check_marker` (die Fassungsnummer nur für den eigenen Abschnitt, sonst die aktuelle), Sprünge baut
`xref::jump_url` (`data-jump data-jump-origin="chat"`), Bibelstellen `baruch_scripture::scan` (nur `Valid`, Ziel
`/literatur/bibel?work&ref`, die Seite wählt ohne `module` die Bibel des Werks). Befunde: `findings::parse::finding_spans`
liefert Byte-Bereiche je Stelle; ein Bereich wird `Segment::Finding`, wenn zum Beitrag ein gespeicherter Befund dieser
Stelle (`Finding::position`) gehört, seine Karte (`findings::view::card`) steht in `PostExtra::finding_cards` für das
Makro `finding_block`. Der Chat ruft es als `finding_block(card, seg.html)` auf (Integrator Runde 30): Der Befundtext
im Block ist dann das Segment mit den geprüften Links aus `post_html`, nicht noch einmal `card.body_html` – so steht
der Text genau einmal da. `PostExtra::plain` = Klartext für „Kopieren“. `post_extra` (Haken aus `view::posts`) nimmt
Befunde und Metadaten aus `ViewExtras`; die Seiten `/beitraege`, `/beitrag`, `/faden` und `post.since` laden beides
selbst (`posts::page_extras`, `threads::entry_values`).

**Vorlagen:** `fragments/post_item.html` mit den Makros `post_item(post)` (Kopf und Text, ohne Seitenkontext),
`post_entry(post, at)` (ganzer Beitrag des Abschnitts-Chats samt ↻/✕/Verwalten, `at = {work, key, node, rev, view, lq,
targets}`) und `thread_entry(e, at)` (`at = {work, thread}`); `assistants.html`, `thread.html`, `posts.html`,
`post.html` binden sie ein. Die Makros `finding_block` und `post_solutions` (chat-loesungen) werden aus
`post_item.html` heraus importiert.

## Runde 29: Buchsatz in den Einstellungen

Schnittstelle R29-S1: Datenmodell `baruch_typst::typeset` (`model.rs` eingefroren: `TypesetSettings`, Grenzen,
`validate`/`normalized`/`changed_fields`/`to_json`/`from_json`, `FIELDS`), Regel-, Vorlagen-, Vorschau- und
EPUB-Funktionen als Stubs (`rules` liefert ""), Store-API `baruch_store::typeset` (Lesen ohne Zeile → `None`, Schreiben
→ `StoreError::Typeset` „noch nicht umgesetzt“), Haken in `snapshot::head` (Regelblock hinter Werkvorlage, Schema- und
Rückfall-Regeln; ohne Einstellungen byte-gleich), Server-Module `typeset_settings` (RPC `work.typeset_*`), `mcp::typeset`
(Werkzeug `typeset_settings`, nur lesend), `settings::work_typeset` (Block „Satz“, Routen, `work.typeset_preview`) und
`typeset::cached_page`. Reserviert: Store 0048 satz-daten, 0049 satz-typst, 0050 satz-oberflaeche, 0051 Integrator;
Bibliothek 0108 satz-daten, 0109 Integrator.

### Daten (satz-daten)

- **Migration 0048** (`crates/baruch-store/migrations/0048_work_typeset.sql`, einzige neue Store-Migration der Runde;
  0049–0051 und Bibliothek 0108/0109 bleiben unbenutzt): Tabelle `work_typeset (work_id PRIMARY KEY → work ON DELETE
  CASCADE, uuid, settings, updated, updated_by)` – höchstens eine Zeile je Werk, keine Zeile = keine
  Satz-Einstellungen. `settings` ist das kanonische JSON von `TypesetSettings::to_json` (CHECK: JSON-Objekt, höchstens
  16384 Zeichen; die genaue Prüfung macht `TypesetSettings::validate` im Code). `uuid` wie 0045/0046 (eindeutiger
  Index, Fülltrigger `work_typeset_uuid_fill`, Grabstein-Trigger `work_typeset_uuid_tombstone`, Kaskaden-Trigger
  `work_typeset_cascade_uuid_tombstone` vor dem Löschen eines Werks, damit der Grabstein die Werk-uuid kennt).
  `uuid_tombstone` bekommt die Art `work_typeset` über den Umbau wie 0046 (sechzehn Grabstein-Trigger droppen,
  temporäre Kopie, Tabelle mit erweitertem CHECK, Trigger wörtlich neu); kein `FOREIGN_KEYS_OFF`, weil kein
  Fremdschlüssel auf `uuid_tombstone` zeigt (Beleg in `tests/migration_0048.rs`).
- **Store** `baruch_store::typeset` (Vertrag §4): `work_typeset` (lesen; unbekanntes Werk → `UnknownWork`),
  `set_work_typeset` (normalisieren, prüfen – ungültig → `StoreError::Typeset` mit allen Meldungen, nichts geändert;
  gleiche Werte → kein Schreiben; leere Einstellungen = zurücksetzen; Ändern per Upsert, die uuid bleibt),
  `reset_work_typeset` (Zeile löschen → Grabstein), `adopt_typeset_rule` (Einstellungen und neue Fassung der
  Werkvorlage samt `work.template`/`work.updated` in einer Transaktion `BEGIN IMMEDIATE`; Vorlage mit Überschrift
  oder unverändert → `Refused`) und `typeset_in` für den Haken in `snapshot::head`. Dort steht der Regelblock hinter
  Werkvorlage, Schema- und Rückfall-Regeln – damit gleich in Export, Export-Karte, Spiegel (`main.typ`), Satz-Vorschau,
  PDF und Umschlag; ohne Einstellungen ist der Export byte-gleich zum Stand R29-S1 (Golden
  `crates/baruch-store/tests/data/satz_golden_*.typ`).
- **Server** `crate::typeset_settings`: `load`/`view_json` (gemeinsam für RPC, MCP und den Block „Satz“; Hinweis auf
  eine unbekannte Grundschrift über `baruch_render::font_families`), `save`/`reset`/`adopt_template_rule` unter der
  Schreibsperre mit genau einem Spiegel-Commit je Änderung (`mirror::describe_typeset`/`record_typeset`, Trailer
  `Baruch-Action: typeset-set|typeset-reset|typeset-adopt`, immer `allow_empty`); RPC `work.typeset_get|set|reset|
  adopt_rule` (`docs/RPC.md`), MCP `typeset_settings` nur lesend (`crate::mcp::typeset`, `docs/MCP.md`).
- **Austausch** Unterversion 6 (`docs/Austauschformat.md`, Abschnitt „Satz-Einstellungen“): `ExchangeTypeset` im
  Modell (mit `TypesetSettings` aus `baruch-typst`, darum hängt `baruch-exchange` jetzt an `baruch-typst`), Wörter je
  Feld in `baruch_exchange::typeset`, Grabsteinart `satz` ↔ `work_typeset`, XSD `docs/schema/baruch-werk-1-u6.xsd`.
  `exchange_dump` schreibt die Zeile mit Kennung und letzter Änderung; `exchange_apply` legt sie beim neuen Einlesen
  an und beim Abgleichen nur, wenn sie hier fehlt und kein Grabstein besteht – sonst Hinweis mit beiden Ständen, nie
  überschreiben. Die Austauschseite zählt sie (`counts.typeset`, `typeset_fields`), die Vorschau des Einlesens nennt
  „Satz-Einstellungen“ in der Liste der hinzukommenden Stücke.

### Typst-Regeln (satz-typst)

- **Regelblock** `baruch_typst::typeset::rules(&s)` (`rules.rs`): "" ohne Einstellungen (auch wenn nur
  `xref_with_page` gesetzt ist – der Schalter ändert keine Datei), sonst Zeile `// Baruch: Satz`, dann je Feld eine Zeile
  in `FIELDS`-Reihenfolge, Ende mit `\n`. Der Store hängt ihn in `snapshot::head` hinter Werkvorlage, Schema- und
  Rückfall-Regeln; Export, Export-Karte, Spiegel, Satz-Vorschau, Buch-PDF und Umschlag sehen damit dieselben Regeln.
  `chapter_start` → Show-Regel `#show heading.where(level: 1): it => { pagebreak(weak: true[, to: "odd"]); it }`, also
  genau die übernehmbare Vorlagenregel; sie gilt auch für Teil-Überschriften (`<baruch-part>`, im Export Ebene 1), damit
  „Vorlagenregel übernehmen“ im Export den Umbruch vor Teilen nicht verliert (Befund R29, Test
  `adopting_the_template_rule_keeps_every_heading_page_in_the_export_form`). Im Satz greift vor ihr die Teil-Regel;
  `continuous` ist nur eine Kommentarzeile. `page_format`/`margins` → `#set page(width/height)` bzw. `margin: (inside: innen + Bundsteg, outside,
  top, bottom)`; Schrift/Größe/Zeilenabstand/Blocksatz/Silbentrennung/Sprache/Schusterjungen → `#set text`/`#set par`;
  Einzug → `first-line-indent` und Absatzabstand = Zeilenabstand (Typst-Vorgabe 0,65 em, sonst `leading_em`); Fußnoten
  → `#set footnote(numbering: "1"|"*"|"a")` und je Kapitel `counter(footnote).update(0)` in einer Show-Regel.
- **Seitenzahlen und Kolumnentitel** sind `context`-Kopf-/Fußzeilen mit den Hilfsfunktionen `baruch-satz-chapters`,
  `baruch-satz-opening(p)` und `baruch-satz-quiet(p)` (Teil- und Leerseiten). „Ab dem ersten Kapitel“ setzt
  `counter(page).update(1)` in den **Seitenhintergrund** der ersten Kapitelseite: Typst legt alles, was eine Show-Regel
  liefert, zwischen Start- und Endmarke der Überschrift; eine Rücksetzung dort käme für `counter(page).at(<kN>)`
  (Querverweis „S. n“) zu spät. Der Hintergrund steht im Seitenrahmen vor Kopfzeile und Inhalt (gemessen in
  `baruch-render/tests/typeset_measure.rs`). `pagebreak(to: "odd")` und `inside`/`outside` zählen physische Seiten.
- **Leerseiten erkennen:** Eine Show-Regel setzt vor jede Überschrift der Ebene 1 die Marke `<baruch-satz-end>` – vor den
  Umbruch der Einstellung oder der Vorlage. Typst lässt abgeschlossene Marken vor dem Umbruch stehen und verschiebt nur
  unabgeschlossene Startmarken (`migrate_unterminated_tags`); Seiten zwischen Marke und Überschrift sind leer. Steht vor
  dem Umbruch nur die Marke, legt Typst sie an den Anfang der Folgeseite (Lage 0 pt) – auch das zählt als leer. Die
  leere Rückseite einer Teil-Seite trägt `<baruch-satz-blank>`. Die Teil-Regel des Satzes greift vor dieser Show-Regel und
  gibt `it` nicht zurück; sie setzt die Marke darum selbst, sobald es einen Regelblock gibt (`part_page_rule_for`, auch
  ohne Einstellung `part_page` – Befund R29: sonst trug die Leerseite vor einem Teil Seitenzahl und Kolumnentitel; Test
  `the_blank_page_before_a_second_part_stays_empty`). Ohne Satz-Einstellungen bleibt die Regel von Runde 14 byte-gleich.
  Messtests erkennen Leerseiten am leeren Satzspiegel, nicht an einer ganz leeren Seite (sonst liefe eine Leerseite mit
  falscher Zahl als Textseite durch).
  Ohne Kapitelregel gibt es keinen Umbruch: Die Marke steht dann an derselben Stelle wie ihre Überschrift und folgt
  ihr in der Abfrage; eine solche Marke zählt nicht (Integrator R29 – vorher galten alle Seiten bis zur nächsten
  Überschrift als leer, ohne Seitenzahl und Kolumnentitel; Test
  `without_a_chapter_rule_every_text_page_keeps_number_and_head`).
- **Teil-Seiten:** `typeset::prepare` liest `state.store.work_typeset(work)` und setzt `part_page_rule_for(labels,
  &settings)`: mit `part_page` = `part_page_rule_with(labels, Some(part))`, mit Regelblock ohne `part_page` = rechte Seite
  ohne leere Rückseite (wie Runde 14) samt Marke `<baruch-satz-end>`, ohne Satz-Einstellungen byte-gleich zu
  `marker::part_page_rule` (Stand und Seitenzahl wie vor Runde 29, Goldstand in `tests/round29_typst.rs`).
- **Werkvorlage** (`template.rs`): gelesen wird eine Kopie ohne Kommentare (gleiche Bytestellen). `template_conflicts`
  meldet je gesetztem Feld die Argumente von `set page|text|par|footnote(…)` (auch im Code von Vorlagenfunktionen) mit
  Zeile und Text, Wirkung `overrides`; Show-Regeln auf Überschriften der Ebene 1 mit `pagebreak` gegen `chapter_start`
  sind `ineffective`, außer die Vorlage will dasselbe oder nur eine neue Seite bei „rechte Seite“ (Typst wendet die
  zuerst definierte Show-Regel zuletzt an, ihr Umbruch kommt also hinzu). `detect_chapter_break` erkennt nur Regeln, die
  ausschließlich aus Umbruch und Überschrift bestehen (Leerraum, mehrzeilig, Code- oder Inhaltsblock, mit/ohne `weak`,
  `to: "odd"` → rechte Seite, ohne `to` → neue Seite, `to: "even"` → nicht übernehmbar), damit `remove_rule` nichts
  anderes entfernt.
- **Satz-Seite:** `typeset::template_hints` liefert Konflikte und erkannte Regel für `#druckpruefung` (Link
  `/einstellungen/werk?work=W#satz`).
- **Umschlag:** `cover::build_wrap` misst die erste Seite des Buchsatzes (also das Satz-Format) und zählt alle Seiten
  samt Leerseiten für den Rücken. Für die Umschlagseiten im Buch misst `cover::frame` die Werkvorlage samt Regelblock
  (`cover::page_template`); Blöcke ohne Schrift nehmen `cover::book_font` (Einstellung, sonst Werkvorlage), ebenso
  `cover_layout::default_font` und die Druckprüfung des Umschlags.
- **Vorschau** `preview_source(s, template, title)`: Vorlage, Regelblock, Teil-Regel (`part_page_rule_for`) und ein
  erfundener Mustertext (ein Teil, drei Kapitel mit Abschnitten und je einer Fußnote, ≥ 6 Seiten A5).
- **EPUB** `epub_style(&s)` (`epub.rs`): CSS für `section.baruch-chapter`/`section.baruch-part` (Umbrüche), `p` (Einzug
  bzw. Abstand, Blocksatz, `hyphens`, `widows`/`orphans`), `body` (Schrift, Größe relativ zu 11 pt, Zeilenhöhe),
  `ol.baruch-footnotes` (Zählung); `split_chapters` immer `true`, `lang`. Noch nirgends verdrahtet (R20a).
- **Messhilfe** `baruch_render::page_texts(&Source) -> Option<Vec<Vec<PlacedText>>>`: je Seite die Textstücke (Text,
  Grundlinie links in pt ab oben links, Breite, Größe, Schrift) mit den Transformationen der Gruppen. Typst setzt einen
  Trennstrich als eigenes Stück mit dem Text U+00AD; Satzzeichen hängen ≈ 1,9 pt in den rechten Rand.

### Oberfläche (satz-oberflaeche)

- `src/settings/work_typeset.rs`: Block „Satz“ (`block` → Vorlage `templates/settings/werk_satz.html`, Kontext aus
  `block_context`, das nur `typeset_settings::TypesetView`, die Werkvorlage und das abgelehnte Formular braucht – so im
  Unit-Test ohne Server zeichenbar), Formulare `act`/`adopt_act`, Vorschaubilder `preview_svg`, RPC
  `work.typeset_preview` (`call`, gemeinsam mit der Formular-Vorschau über `preview`).
- Formularfelder ↔ Einstellungen: `settings_from_form` (Vertrag §6.3) und die Umkehrung `form_from_settings`
  (Rundreise-Test mit dem Vertragsbeispiel). Keine eigene Grenzprüfung: Zahlen, die keine sind, werden `NaN`, damit
  `TypesetSettings::validate` sie mit derselben Meldung ablehnt wie über RPC; dazu kommen nur „unbekannte Wahl“ und
  unvollständige Ränder. Meldungen sind `(Feld-id bzw. Unterfeld, Text)`; die Vorlage zeigt sie beim Feld (Kopf vor dem
  Punkt), Konflikte aus `template_conflicts` ebenso (`data-typeset-error`, `data-typeset-hint`).
- `Retry` (in `WorkRetry::Typeset`) trägt Eingabe, Meldungen und – nach `action=preview` – die `Preview`. Abgelehnte
  Formulare und die Vorschau ohne JavaScript nutzen denselben Weg: `settings::work_retry_page` zeichnet die ganze Seite
  „Werk“ (Status 422 bzw. 200), der Block zeigt dann die ungespeicherten Werte, ihre Regeln und Konflikte.
- Vorschau: `preview_source` + `export_assets` → `typeset::compile` (Sperre `RUNS`, `COMPILE_TIMEOUT`, legt den Stand in
  den Zwischenspeicher); `preview_svg` liest nur `typeset::cached_page` und startet nie einen Lauf (Schutz gegen
  Satzläufe auf Zuruf fremder Seiten: `guard::cross_site_get` → 403).
- Widget `web/src/typeset_settings.ts` („typeset-settings“ am Formular): `formToSettings` liest das Formular wie
  `settings_from_form` (Test unter Node vergleicht beide), `debounce` (nachbaubare Zeitgeber) und `Newest`
  (AbortController; verspätete Antworten werden verworfen). Der Knopf „Vorschau“ setzt sofort, ohne abzuschicken;
  „Speichern“/„Zurücksetzen“ bleiben Formulare (im Popup schickt sie `settings.ts`).
- `web/src/xref.ts`: `initialPage(vorhanden, Werk-Einstellung, Browser)`; der Dialog holt `work.typeset_get` und nimmt
  `settings.xref_with_page`, solange das Häkchen unberührt ist; `localStorage` (`baruch_xref_page`, try/catch) nur bei
  ungesetzter Werk-Einstellung.
- Stil `static/typeset_settings.css` (in `STYLE`). Rücksprung-Hinweise `ok=satz|satz_zurueck|satz_vorlage` in
  `settings/work.rs` (`notice`).

### Integration (Runde 29)

- Zusammenführung `team/satz-daten` → `team/satz-typst` → `team/satz-oberflaeche` mit `--no-ff`, ohne Konflikte.
- Abnahme `crates/baruch-server/tests/round29_abnahme.rs` (erfundenes „Fährhausbuch“ mit Titelei, zwei Teilen, vier
  Kapiteln, Fußnoten und Querverweis mit Seitenzahl): Formular → `work_typeset` → Haken `snapshot::head` → Satz und
  Buch-PDF aus derselben Quelle (gleicher Stand), gemessen mit `baruch_render::page_texts` und `pdfinfo`/`pdftotext`
  (A5, Kapitel rechts, Seitenzahlen außen ab dem ersten Kapitel auf jeder Textseite, Kolumnentitel, Bundsteg,
  „S. n“), Umschlag-Seitenmaß und Rücken, jede Einstellung einzeln über den Server, Übernahme der Vorlagenregel
  (Kapitelseiten vorher = nachher), Konflikt `#set page(paper: "a4")`, MCP nur lesend, Austausch u6 mit Grabstein über
  drei Instanzen.
- `typeset_settings::load` meldet zusätzlich `NO_DOCUMENT_TITLE`, wenn der Kolumnentitel „Werktitel“ gewählt ist, die
  Werkvorlage aber kein `#set document(title: …)` hat (die Kolumne bliebe leer).
- Austauschseite (`templates/exchange.html`) nennt die Satz-Einstellungen (`counts.typeset_fields`).

## Runde 19 – RAG-Pflege, Reste der Stellen

### R19a index

Ohne Migration (Präfix im JSON der Verbindung, Sperrvermerk und Marke in `sync_state`). Einzelheiten unter
„Bedeutungssuche“ (Dienst) und „Abgleich“:

- `rag/index.rs`: `Runtime` mit `enqueue` (laut, holt Liegengebliebenes zurück) und `enqueue_quiet` (Suche),
  `stale()`, `wish_reset`/`reset_wished`/`apply_reset`, `unbound`; `process` → `Outcome {embedded, follow}`;
  `sync` mit Bereichs-`replace`, Präfixen und Abbruch; `PREFIX_CHANGED`.
- `rag/mod.rs`: `catch_up` ohne Einbetter bei eigenem Datenordner ohne gespeicherte Verbindung Eliezer
  (`NO_EMBEDDER_OWN_DATA`), `index_info` mit `stale` und `stale_text`, `reindex_report` → `Reindexed {pending,
  deferred}`, `answer_json` mit `weak`.
- `baruch-lancedb`: `Scope::contains`, `Index::replace(scope, …)`, `IndexMeta.document_prefix`.
- `sync/`: `Link::answered` (über `multigpt::Session::broken`), `Writer::blocks`/`chapter`/`restore`, Marke
  `UPLOADING`, `Filler::number` liefert den gesetzten Wert, Vollständigkeit der Liste in `download::list`.
- `connections/check.rs`: `multigpt` beendet die Sitzung mit `McpClient::delete` (nicht, wenn das gemeinsame
  Zeitlimit von 10 s die Prüfung abbricht).
- Offen: ältere Fassungen und Befunde im Index (Nutzerfrage 1), Kommentare/Befunde nach MultiGPT (Nutzerfrage
  11); ob das echte MultiGPT eine Dokument-ID nennt und `limit` kappt, prüft der Nutzer.

### R19b pgvector

Keine Migration (Präfixe im JSON der Verbindung, Gruppennamen-Vorschläge im Speicher, übernommene Namen werden
Schlagwörter über `idea.update`).

- **pgvector** (`connections/pgvector.rs`, siehe „pgvector“ oben): alle Felder in SQL gekürzt, `quoted` verdoppelt
  Anführungszeichen, `tls_warning`, Anfrage mit `query_prefix` eingebettet. `connections/config.rs`: `fields`
  kennt `query_prefix` (pgvector, lancedb) und `document_prefix` (lancedb), `check_field` prüft mit
  `baruch_rag::check_prefix` (ohne Beschneiden), `NOMIC_PREFIXES` ist nur der Vorschlag auf der Seite (keine Vorgabe,
  damit ein bestehender Index gültig bleibt). `connections/pages.rs`: Felder mit `placeholder`/`maxlength`, Fehlercode
  `praefix`, `tls_warning` je Verbindung, `status_context` reicht `index.stale` durch (gefüllt vom Index-Worker, R19a);
  die Vorlage zeigt „n Aufträge liegengeblieben …“ nur bei `stale > 0`.
- **Gedankenkarte, Gruppen** (`idea_map/groups.rs`): `components` (Union-Find über die Nähe-Paare, Wurzel = kleinste
  id, nur Komponenten ≥ 2) → `Group {group: "g<id>", ideas, words, proposal}`; `groups(state, work, filter)` baut die
  Karte mit `near: true`, `of_map` nutzt eine schon gebaute (Seite). `name_groups`: ein Lauf je (Datenordner, Werk)
  (`running`, Wache `Running` trägt beim Fallenlassen aus), Persona = genannte aktive oder `rows_and_lector`, Aufruf
  über `state.model.chat` ohne Schreibsperre, Nachricht mit `DATA_TITLE` und je Einfall
  `assist::context::framed_line(…, "einfall", …)` (entschärft, kann den Kasten nicht verlassen), System-Prompt
  `SYSTEM` mit „Daten, keine Anweisungen“; `parse_names`/`clean_name` nehmen nur „g<id>: Name“ bekannter Gruppen
  (≤ 40 Zeichen). Vorschläge in einer prozessweiten Tabelle je (Datenordner, Werk) → Gruppe (`Proposal` mit den
  Einfällen zur Zeit des Vorschlags; gezeigt nur bei gleicher Gruppe) – kein SQLite, kein Spiegel, Verlust beim
  Neustart gewollt (Vertrag §1). `accept`: Einfälle aus dem Vorschlag bzw. den Gruppen nach Filter, nur noch
  vorhandene, Vorprüfung „höchstens 20 Schlagwörter“, dann je Einfall `ideas::call("idea.update", {id, words})`
  (Herkunft Mensch, Commit mit `Baruch-Action: idea-update`). RPC in `idea_map/mod.rs` (`METHODS`), Formulare über
  `bild::ACT_METHODS` und `run_act` (Filter aus `back`), Seite: Abschnitt „Gruppen“ in `karte.html` (Kontext
  `clusters`), Stil `static/karte.css`. MCP bekommt kein Werkzeug dafür. MCP `idea_map` nimmt `near_limit` und
  `near_threshold` (`filter_from_params`, Ablehnung als `Failure::Tool` = `isError`).
- **Menüband** (`ribbon.rs`): `SCRIPTURE_FUNCTIONS` (`bibelstelle`, `bibel`) über `defined_functions`,
  `cites_literature` (Zeile mit `#bibliography(` außerhalb `//`). `ws_command.ts` (sonst eingefroren) bekommt die
  Befehle `scripture`/`cite` und die reinen Bausteine `scriptureFunction`, `scriptureSource`, `citeKey` (keine
  Importe, darum unter Node testbar); `ribbon.ts` schickt bei `scripture` `arg = {fn}`, `wysiwyg.ts` setzt eine Insel
  (`span.ws-island[data-raw]`) bzw. einen Verweis (`button.ws-ref[data-target]`), die `wysiwyg.serialize` wörtlich bzw.
  als `@schlüssel` speichert (`docs/WYSIWYG.md`).
- **NFC**: `baruch_library::check_range` vergleicht Ausschnitt und Zitat in NFC (Bereich in Zeichen des gespeicherten
  Volltexts, ersatzweise in Zeichen seiner NFC-Form); `websearch::verify::normalize` beginnt mit `nfc()` (Seite und
  Zitat gehen beide hindurch). Crate `unicode-normalization` 0.1 (S1).
- **Annahmen:** Die Funktion `#bibelstelle`/`#bibel` der Vorlage nimmt die Stelle als Inhalt `[…]`; das Literaturverzeichnis
  steht als `#bibliography(` in der Vorlage (nicht in einer eingebundenen Datei). Beides ist ohne Typst erkannt
  (`ribbon::code_lines`); seit Runde 23 zählen dabei Blockkommentare und die Klammertiefe (nur oberste Ebene), und
  per `#import "datei.typ": name` eingeführte Funktionen zählen mit.

### R19c treffer

- **Trefferdarstellung:** Schwelle allein in `baruch_rag` (`WEAK_SCORE`, `is_weak`, `WEAK_LABEL`, Schnittstelle S1).
  `literature::rag_view` ordnet stabil nach `weak` und liefert `weak_count`; `fragments/library.html` rendert schwache
  Treffer über ein Makro in `<details class="weak-hits">`. `ideas::rag_ideas` sortiert schwache zuletzt;
  `ideas.html` dasselbe Muster. `assist::rag_facts::drop_own_section` und `block_entries` stellen schwache Treffer
  ans Ende (vor der Grenze `LIMIT`) und schreiben `rag_facts::similarity` („Ähnlichkeit (schwach, n %)“).
- **`idea.search` (rag):** LanceDB nicht `Connected` oder Fehler außer `RagError::Invalid` → `rpc::RAG_UNAVAILABLE`
  (-32001). MCP `idea_search {mode: "rag"}` ruft dieselbe RPC und macht daraus `isError`.
- **baruch-scripture:** `wording::compare` vergleicht nach dem Wortvergleich zusätzlich ohne Leerraum
  (`Comparison::spacing`): die Schlüssel der Bibelwörter aneinandergehängt, jeder Teil des Zitats muss von einer
  Wortgrenze des Moduls bis zu einer Wortgrenze passen (kein Treffer mitten im Wort). `plain_words` normalisiert
  nach NFC (`unicode-normalization`), sonst zerfiel ein zerlegtes „ü“ in zwei Wörter. `wording::fragment_survey`
  schätzt Trennreste: Paar „A b“ im selben Vers, A ohne Satzzeichen am Ende, b klein, höchstens 5 Zeichen, kein
  häufiges kurzes Wort, und „Ab“ kommt sonst im gelesenen Text vor; auffällig ab 3 Fällen und 1 von 200 Wortpaaren
  (`FragmentSurvey::fragmented`). `quote_for` lässt vor einem Zitat danach bis zu drei Wörter aus `INTRO_WORDS` mit
  Doppelpunkt zu (höchstens 48 Bytes). `count_word_follows`: „Mal“ mit folgender Kapitel- und Versangabe ist kein
  Zahlwort.
- **baruch-sword:** `Bible::sample_texts(max, deadline)` (Stichprobe in Modulreihenfolge), `reference::lemma_candidates`,
  `compound_tails`, `strong_like`, `Lexicon::lookup_lemma` (Stichwort, dann `get_strongs` für Strong-Nummern
  unabhängig von `StrongsPadding`, dann Endungen/Umlaut, zuletzt Kompositum; Antwort mit probierter Form).
  `fixture::hyphen_remnants` baut in erfundenen Text Trennreste ein (Tests; fremde Module liest kein Test).
- **Server:** `bibles::WordingResult.module_spacing`, `SPACING_LABEL`; `bibles::module_note` (Stichprobe 4000 Verse,
  2 s, Zwischenspeicher je conf-Datei und Änderungszeit, blockierend) und `choice_note`; `scripture::check_with`,
  `bible.marks`, `facts_with` (Zeile „Hinweis: …“) und `bible_line` tragen den Hinweis. `scripture::mark_with`: andere
  Personas mit Bibel bekommen `mark_others_bible(Modul)` (beginnt mit `MARK_BIBLE`, gilt damit in `chat_render` als
  Schlusshinweis), ohne Bibel `MARK_OTHERS`; `mark_answer_for_work` öffnet die Bibel nur, wenn eine Stelle vorkommt.
  `bibles::german_of` bildet einen Vers des Moduls über `mapping::map_verse` ins Deutsche zurück; `bible.lookup`
  liefert `german_ref`/`german_problem`, die Bibelseite (`literature/bibles.rs`, `literature_bible.html`) setzt sie in
  `data-ref`/`data-verse-ref` und als Link. `literature::bibles::workspace_shelf` (über `view::extras`, Feld
  `ViewExtras.bibles`) und `Shelf::context_at` bringen den Kasten „Bibeln“ serverseitig auf die Arbeitsseite.
- **Annahmen:** Die Schwelle 0,45 gilt für alle Wege gleich, obwohl MultiGPT seine Skala nicht meldet; die
  Fragment-Schätzung ist eine Faustregel (kann bei sehr kurzen Stichproben irren und meldet dann eher nichts).

### Integration

- Zusammengeführt ohne Konflikte (index → pgvector → treffer, je `--no-ff`). Die Teile greifen über die
  Schnittstelle R19-S1 ineinander: `IndexInfo.stale` füllt index, die Seite „Verbindungen“ (pgvector) zeigt es;
  `config::embed_prefixes` lesen LanceDB (index) und die pgvector-Suche; `baruch_rag::is_weak` gilt in `rag.search`
  (index) und in Literatur, Einfällen und Belegen (treffer) gleich.
- Ein `EmbedderFactory` für alle Wege: `rag::set_embedders` bzw. Eliezer liefert die Einbetter für LanceDB und für
  die Anfrage an pgvector (`rag::plan_way` reicht denselben Factory an `hooks::open_pgvector`); das Präfix wählt
  jeder Weg selbst aus seiner Verbindung.
- MCP `idea_search`: Kurzbeschreibung nennt `mode` (Integrator, `mcp/tools.rs`).
- Abnahme `tests/round19_abnahme.rs` (5 Tests, Werk „Die Laternen am Kai“, alles frei erfunden, nur Nachbauten).

## Runde 20 – EPUB-Vorschau

### Schnittstelle R20-S1

Neue Crate `crates/baruch-epub` (rein: keine Datenbank, kein Netz, kein Typst-Satz). Eingefroren in `lib.rs`:
`EpubMeta`, `EpubImage`, `EpubSection`, `FootnoteMode` (`popup` Vorgabe, `endnotes`), `EpubInput`, `ItemKind`,
`EpubItem`, `TocEntry`, `EpubWarning`, `EpubBook`, `EpubError`; Konstanten `MIMETYPE`, `CONTENT_DIR` (`OEBPS`),
`OPF_PATH`, `CONTAINER_PATH`. Module: `build` (S1-Minimalbau: Dateiaufteilung je Teil/Kapitel `text/k{key}.xhtml`,
Vorspann `text/front.xhtml`, `<p>` mit maskiertem Rohtext, `nav.xhtml`, `style/book.css` aus
`baruch_typst::typeset::epub_style`, minimales OPF), `package` (echt: `mimetype` zuerst STORED ohne Extra-Feld, feste
Zeitstempel, DEFLATE), `xhtml::escape_text`, `sample::input` (frei erfundenes Werk „Das Gartenhaus am Bach“, PNGs im
Code erzeugt), `readers` (Typen und Slugs eingefroren, Platzhalterwerte), `check` (Typen eingefroren, Körper STUB).
Namensregeln der Dateien und Klassen: Vertrag §1.4 (Kopf von `lib.rs`).
Store: `Store::epubcheck_jar`/`set_epubcheck_jar` (`store_meta`, Schlüssel `epubcheck_jar`, keine Migration).
Server: `epub.rs` (RPC `work.epub`, `GET /werke/{id}/epub`, `epub_input`, `book_for`, `epub_bytes`,
`write_epub_file`; STUB `EPUB_COVER` von `cover.rs` hierher verschoben, `cover::book_info` ist `pub(crate)`),
`epub_preview.rs` (`epub.readers` echt, `epub.preview` und Seiten STUB), `epub_check.rs` (`epub.check`,
`epub.checker_get`/`epub.checker_set`, Seiten, Formulare, `preview_panel` – alles STUB), MCP `mcp/epub.rs`
(`epub_export`) und `mcp/epub_check.rs` (`epub_check`), beide STUB und nur lesend. Stile `static/epub.css`,
`static/epub_check.css`, `static/readers/<slug>.css`; Widget `web/src/epub_preview.ts` (leer).

### R20a epub

- **`baruch-epub::build`** ersetzt den S1-Minimalbau: Aufteilung nach §0.2 (`split`), Text über das Blockmodell
  (`xhtml::Renderer`, Abbildung in `docs/WYSIWYG.md` „EPUB-Abbildung“), Fußnoten (`FootnoteMode::Popup` →
  `noteref`/`aside` in `ol.baruch-footnotes` am Dateiende; `Endnotes` → `text/notes.xhtml`; Nummern fortlaufend wie
  Typst, je Kapitel neu nur mit Satz-Einstellung `restart = per_chapter` im Popup-Modus, Zeichen nach
  `FootnoteNumbering`), Bilder (nur die vom Text genannten, `images/<bereinigter Pfad>`, ids `img-n`), Umschlag
  (`images/cover.<ext>` + `text/cover.xhtml`), Titelei (`title`, `imprint`, `dedication`), `nav.xhtml` (toc +
  landmarks, `hidden`), `toc.ncx` (verschachtelte navPoints, `dtb:depth`), OPF (Metadaten nur über `escape_text`,
  `urn:isbn:` nur nach `isbn13_check`, Bildnachweis als zweites `dc:rights`, schema.org-Barrierefreiheit,
  `alternativeText` nur wenn jedes Bild alt hat, `<guide>` für EPUB-2/Kindle), CSS = `build::BASE_CSS` +
  `epub_style(&typeset)`. Labels werden vorab gesammelt (`collect_labels`), damit `@name` dateiübergreifend zeigt.
- **`baruch-epub::template`**: `pagebreak_rules` (eigener Scanner: Kommentare entfernt, jede Show-Regel bis zum
  Zeilenende auf Klammertiefe 0) → `TEMPLATE_PAGEBREAK_MAPPED` für `heading.where(level: 1)`/`<baruch-part>`, sonst
  `TEMPLATE_PAGEBREAK_UNMAPPED`; `template_chapter_start` liest `detect_chapter_break` und setzt `chapter_start`, wenn
  die Satz-Einstellung fehlt; `template_lang` (`#set text(lang: …)`), `is_language_tag`.
- **`baruch_typst::typeset::epub_style`**: Umbruchregeln für `section.baruch-chapter`/`section.baruch-part` immer
  (Vorgabe neue Seite, fortlaufend → `auto`, rechte Seite → `right`), weil Kapitel immer eigene Dateien sind.
- **Server `epub.rs`**: `epub_input` sammelt `export_map` (Werkvorlage = Text vor dem ersten Abschnitt), Nummern aus
  `numbering_preview` mit dem eingestellten Schema, `export_assets` + Alternativtexte aus `asset_list`, Umschlag
  (`cover::cover_name`, `asset_data`), `book_meta` + `cover::book_info` → `imprint_lines`, `copyright_line`,
  Sprache (Satz-Einstellung → Vorlage → „de“), `dcterms:modified` (jüngster Zeitpunkt aus Werk, Buchangaben,
  Satz-Einstellungen, Bildern, Fassungen). Ein Abschnitt ohne lesbare Überschrift bleibt als Ebene 2 erhalten. RPC
  `work.epub`, Route `GET /werke/{id}/epub`, `write_epub_file` (CLI, `create_new`). STUB `EPUB_COVER` entfernt.
- **MCP `mcp/epub.rs`**: `epub_export` lesend, `MCP_EPUB_LIMIT` = 8 MiB für `include_bytes`.
- **CLI** `export --epub` (`main.rs`).
- Tests: `baruch-epub/tests/{build,xhtml,package}.rs` (Aufteilung, Fußnoten, Querverweise, Bilder/alt, Inseln,
  Vorlagenregeln, Metadaten/Maskierung, OCF-Reihenfolge, Manifest/Spine, xmllint), `baruch-typst/tests/typeset_epub.rs`,
  Server `tests/round20_epub.rs` (RPC, Download, 403/404/422, Umschlag/ISBN/Bilder, MCP mit 8-MiB-Grenze, CLI).

### R20b reader

- **Profile** `baruch_epub::readers` (`PROFILES`, `profile(slug)`, neu `required()`): die neun eingefrorenen Slugs in
  Vertragsreihenfolge mit Gerät, Maßen (CSS-Pixel im Hochformat), `required` (die fünf Pflicht-Profile),
  `via_conversion` (nur Kindle/Kindle-App), `ReaderSupport` und Hinweis. **Annahmen** (ohne echtes Gerät und ohne
  Netz zusammengestellt, beim Wissensstand der Entwickler): Apple Books „Original“ mit Georgia als Rückfall, eigene
  Schriften sicher nur mit `com.apple.ibooks.display-options.xml`; Google Play Books „Original“ mit Literata;
  Kindle/Kindle-App nach Send to Kindle mit Bookerly statt der Verlagsschrift, ohne Hintergrundfarben, SVG unsicher;
  tolino und Kobo (gewöhnliches EPUB, nicht KEPUB) zeigen Fußnoten als Sprung statt Popup; E-Ink in Graustufen;
  Thorium (Readium) und der calibre-Betrachter können EPUB 3 vollständig. **Hersteller-Reader auf Android**
  (Samsung, Huawei …) sind ohne Gerät nicht prüfbar – kein erfundenes Profil, Nutzerfrage.
- **Reader-CSS** `static/readers/<slug>.css` (je Profil eine Datei, im Server per `include_str!`,
  `epub_preview::reader_css`): Standardschrift, -größe, Ränder (`padding` am `body`), Zeilenabstand, Ausrichtung,
  Silbentrennung (`hyphens`), Fußnoten (Popup-Reader: Kasten „Popup-Fußnote“, sonst Notentext mit Linie), E-Ink
  (`filter: grayscale(1)`, keine Hintergründe), eigene Vorgabeschrift (`!important`) und nach der Marke
  `/* @night */` der Nachtmodus. Kein `<`/`&` (das CSS steht in XHTML), keine Adressen.
- **Reader-Dateien** `GET /werke/{id}/epub/leser/{slug}/{*file}` (`epub_preview::reader_file`): das Buch entsteht je
  Anfrage neu (`epub::book_for`, kein Zwischenspeicher); XHTML bekommt `<style id="baruch-reader">` vor `</head>`
  (hinter `style/book.css`), alles andere byte-gleich mit dem Typ aus dem Manifest. Antwort mit CSP
  `sandbox allow-same-origin; frame-ancestors 'self'` (keine Skripte, keine Formulare, nur in Baruch einrahmbar; `allow-same-origin`, damit Bilder und Stile der Lesedatei als Anfragen derselben Herkunft laden – sonst wies der Schutz gegen fremde Seiten sie mit 403 ab), `nosniff`,
  `no-cache`. Pfade mit `..`, `.`, leeren Teilen, `\` oder Steuerzeichen und alles außerhalb des Manifests → 404;
  `text/notes.xhtml` ohne `fussnoten` baut mit Endnoten (Links aus Kapiteln verlieren die Abfrage).
- **Rahmen und Herkunft:** `guard::no_framing` setzt bei einer Antwort mit eigener `frame-ancestors`-Angabe kein
  `DENY` und hängt nichts an die CSP: bei `frame-ancestors 'self'` `X-Frame-Options: SAMEORIGIN`, sonst ohne
  `X-Frame-Options`. Alle anderen Antworten behalten `DENY` und `frame-ancestors 'none'` (Test). Wegen `sandbox`
  läuft das eingerahmte Dokument mit undurchsichtiger Herkunft; seine Unteranfragen (Bilder, `book.css`, Links
  zwischen Dateien) kommen nach Fetch Metadata als `Sec-Fetch-Site: cross-site`. Sie werden angenommen, wenn
  `Referer` auf denselben `Host` zeigt (`epub_preview::from_own_frame`), sonst 403. **Geprüft in der Sichtprüfung
  des Integrators (Firefox):** Der Browser kürzt diesen `Referer` auf die Herkunft (`http://host/`), auch mit
  `Referrer-Policy: no-referrer-when-downgrade` im Rahmen; darum gilt der gekürzte `Referer` desselben Hosts (oder,
  ungekürzt, eine Reader-Datei desselben Werks). Eine fremde Seite kann den `Referer` nicht auf unseren Host setzen.
- **Vorschau-Seite** `GET /werke/{id}/epub/vorschau` (`templates/epub_preview.html`, eigene Umgebung im Modul):
  Formular-GET (`leser` mehrfach, `datei`, `fussnoten`, `nacht`), Kapitelgrenzen-Liste aus den Spine-Dateien mit
  Text (Titel aus `EpubBook.items`, Teil/Kapitel als Grenze markiert, Links mit der Profilwahl), Hinweise beim
  Erzeugen (`EpubBook.warnings`), je Profil eine `<figure>` mit `<iframe>` in Gerätegröße, Hinweis „Send to Kindle
  nimmt EPUB an“, darunter der Platz „Prüfung“ mit dem unveränderten Fragment `epub_check::preview_panel` (leer →
  Link auf `/werke/{id}/epub/pruefung`). Unbekannter Slug/Datei → Hinweis mit 404, ungültige `fussnoten` → 400,
  Werk ohne Abschnitt → Hinweis mit 422, fremde Seite → 403. Widget `epub-preview` (`web/src/epub_preview.ts`)
  schickt das Formular bei jeder Änderung ab (nicht, wenn kein Reader mehr gewählt ist).
- **RPC** `epub.readers`, `epub.preview` (siehe RPC.md). Menü: base.html (siehe „Kopfzeile“).
- Tests: `baruch-epub/tests/readers.rs` (Slugs, Pflicht, Kindle, CSS-Datei je Slug), `tests/round20_reader.rs`
  (Seite, Formular, 404-Hinweise, Reader-Datei mit CSP/`SAMEORIGIN`/`nosniff`, xmllint, Bilder per `reader_file`,
  `no_framing`, RPC, Prüf-Platz, Reader-CSS), `tests/round9_menu.rs` (Menüeinträge, Seite in `all_pages`).

### R20c epub-pruefung

- **`baruch_epub::check`** (rein, ohne Netz): `check_structure(epub, xmllint)` liest die ZIP-Bytes. Den ersten lokalen
  Dateikopf prüft es direkt in den Bytes (`mimetype` als erster Eintrag, Methode STORED, kein Extra-Feld, Inhalt
  `application/epub+zip`), den Rest über `zip::ZipArchive` (nur XML/CSS werden entpackt, je höchstens 16 MiB; Pfade mit
  `..`, `/` am Anfang oder `\` sind ein Fehler). `container.xml` → `rootfile/@full-path` (ohne container der Rückfall
  `OEBPS/content.opf`, damit die übrigen Prüfungen trotzdem laufen), OPF → Metadaten (Kennung samt
  `unique-identifier`, Titel, Sprache, `dcterms:modified` bei EPUB 3, `schema:access*` als Hinweis), Manifest
  (doppelte ids, fehlende Dateien, Dateien ohne Eintrag außer `mimetype`, `META-INF/*` und OPF), Spine (bekannte ids,
  nur XHTML oder mit `fallback`, `toc` → NCX, ohne `toc` Hinweis `NCX_MISSING`), `<meta name="cover">`, Navigation
  (`properties="nav"` und `<nav epub:type="toc">`), Lesereihenfolge (Dateien der nav-Links müssen in Spine-Reihenfolge
  stehen). Je XHTML: Sprache, `<title>`, `alt` (fehlend = Fehler, leer = Hinweis), doppelte ids, Verweise von `img`,
  `link`, `a`, SVG-`image` auf fehlende Dateien bzw. Sprungmarken. Codes fest in `check::codes`.
- **Wohlgeformtheit:** mit `xmllint --noout --nonet -` je Datei über die Standardeingabe (wenn `find_xmllint` ihn im
  übergebenen PATH-Inhalt findet), sonst und als Rückfall, wenn xmllint nicht startet, mit einem eigenen kleinen
  XML-Baum über quick-xml (`check/xml.rs`: genau eine Wurzel, geschlossene Elemente auch am Dateiende, gültige
  Attribute ohne Doppelte, nur die fünf vordefinierten Entitäten und Zeichenreferenzen – XHTML im EPUB hat keine DTD).
  Der Inhalt wird immer über diesen Baum geprüft; `CheckReport.xml_tool` nennt das Werkzeug der Wohlgeformtheit.
- **Reader-Warnungen** `reader_warnings(book, profile)` aus `ReaderSupport` und dem gebauten Buch (CSS und XHTML der
  Items): Popup-Fußnoten (`epub:type="noteref"`), Seitenumbruch-CSS (bei Kapitel-/Teil-Dateien), eingebettete Schriften
  (`font/*`, `@font-face`), Verlagsschrift (`font-family` bei `overrides_publisher_font`), CSS-Hintergründe und
  Nachtmodus, SVG, Silbentrennung (`hyphens: auto`), Umwandlung über Send to Kindle (`via_conversion`). Jede Warnung
  trägt `reader = slug` und beginnt mit dem Namen des Readers.
- **EPUBCheck:** `find_epubcheck(path_env, jar)` – `epubcheck` (ausführbar) im übergebenen PATH-Inhalt vor einem
  vorhandenen `.jar`; `run_epubcheck` startet das Programm bzw. `java -jar` mit dem EPUB in einer eigenen
  Prozessgruppe, liest stdout/stderr in eigenen Fäden (je höchstens 4 MiB) und beendet bei Ablauf der Zeitgrenze die
  ganze Gruppe (`kill -KILL -- -pgid`, dann `Child::kill`), ohne auf die Leser zu warten. Zeilen `ERROR(…)`,
  `FATAL(…)`, `WARNING(…)`, `INFO(…)`/`USAGE(…)` werden Befunde `EPUBCHECK_<Code>` mit dem Pfad im EPUB; „bestanden“
  heißt Exit 0 ohne Fehlerzeile. Ein eben geschriebenes Skript, das kurz „Text file busy“ meldet, wird bis zu 20-mal
  erneut gestartet. Nie ein Download; PATH des Prozesses wird nur gelesen, nie gesetzt.
- **Server `epub_check.rs`:** `run_check(state, work, readers, footnotes, epubcheck, path_env)` baut das EPUB über
  `crate::epub::book_for` und `baruch_epub::package` (wie der Download, kein Commit), prüft in `spawn_blocking`, hängt
  die Warnungen des Baus an (`…_MAPPED` als Hinweis, sonst Warnung) und je Reader `reader_warnings`. EPUBCheck nur mit
  `epubcheck = true`: Pfad aus `Store::epubcheck_jar`, höchstens ein Lauf zur Zeit (statische Semaphore), EPUB in einem
  eigenen Temp-Ordner (`baruch-epubcheck-<pid>-<n>`, beim Fallenlassen entfernt), Zeitgrenze 180 s. RPC `epub.check`,
  werklos `epub.checker_get`/`epub.checker_set` (Prüfung des Pfads in `validate_jar`: absolut, `.jar`, reguläre Datei,
  keine Steuerzeichen, ≤ 4096 Bytes). Seite `GET /werke/{id}/epub/pruefung` (Vorlage `epub_check.html` mit dem
  Fragment `fragments/epub_check_panel.html`, eigene minijinja-Umgebung), Formulare `POST /werke/epub/pruefen` (200 mit
  Bericht samt EPUBCheck) und `POST /einstellungen/daten/epubcheck` (303 bzw. 422 über `settings::error_page`); der
  Block „EPUBCheck“ auf Einstellungen → Daten kommt aus `epub_check::settings_context`. `preview_panel` liefert den
  Bericht ohne EPUBCheck als fertiges, maskiertes Fragment für die Vorschau. MCP `epub_check` ruft `run_check` fest mit
  `epubcheck = false`.
- **Buchangaben-Reste:** `baruch_typst::cover::isbn13_check` (Signatur unverändert) schneidet den Vorsatz
  „ISBN“/„ISBN-13“/„ISBN 13“ in jeder Schreibung ab und kennt die neue Fehlerart `IsbnError::Prefix` (nicht 978/979);
  `Store::set_book_meta` vergleicht die ISBN als Text (eine nur anders geschriebene ist eine Änderung). Papierdicke:
  gespeichert bleibt `paper_mm` je Seite (keine Migration, Austauschformat unverändert); `work.meta.get` rechnet
  `paper_sheet_mm` = 2 × `paper_mm`, `work.meta.set` und das Formular nehmen `paper_sheet_mm` (halbiert gespeichert; mit
  `paper_mm` zusammen -32602). Umschlagbild löschen: `cover::release_cover_before_delete` entfernt unter derselben
  Schreibsperre zuerst den Umschlag mit eigenem Commit „Umschlag entfernt“ (`asset.delete` ohne `rev`, Formular,
  bestätigter Löschantrag), erst dann löscht der Store das Bild. Ein bestätigter Umschlag-Antrag setzt den Umschlag
  über `Store::set_work_cover` unter der Sperre und schreibt **einen** Commit (`mirror::describe_asset_request` plus
  „Umschlag vorher/nachher“ über `cover::commit_request`) statt zuvor zweier.
- **Druckerei-Umschlag mit Textregeln (Abweichung vom Backlog: aus R20a hierher, Vertrag §0.6):**
  `baruch_typst::cover::template_text_rules` liest die `#set text(…)`/`#set par(…)`-Regeln am Zeilenanfang der
  Werkvorlage samt Regelblock der Satz-Einstellungen (`cover::page_template`), behält nur `lang`, `region`, `script`,
  `dir`, `hyphenate`, `tracking`, `spacing`, `costs`, `kerning`, `ligatures` bzw. `justify`, `leading`, `spacing`,
  `first-line-indent`, `hanging-indent`, `linebreaks` mit einfachen Werten (Zahlen mit Einheit, Zeichenketten,
  `true`/`false`/`auto`/`none`, Wörterbücher daraus) und lässt bedingte Regeln, Variablen und ein `lang: "de"`, das
  ohnehin gilt, weg. `wrap_source_with_rules` und `block_measure_source_with_rules` setzen sie direkt hinter
  `#set text(lang: "de")`; ohne Regeln sind sie byte-gleich zu `wrap_source`/`block_measure_source` (Goldstände
  R26/R27 unverändert grün, `round27_satz.rs`, `baruch-typst/tests/cover_layout.rs`). Show-Regeln der Vorlage nimmt der
  Umschlag weiterhin nicht mit (sie könnten Überschriften oder Seiten betreffen, die es auf dem Umschlag nicht gibt).

### Integration

- Zusammenführen ohne Konflikte (`--no-ff`, Reihenfolge S1 → epub → reader → epub-pruefung).
- Integrator: `mcp.rs`-`instructions` nennen `epub_export` und `epub_check`; die S1-Ausnahme für STUB-Beschreibungen in
  `tests/mcp.rs` ist entfallen; die Prüfseite `/werke/{id}/epub/pruefung` steht in `round9_menu` `all_pages` (42
  Seiten); Modulkopf von `cover.rs` verweist nicht mehr auf den entfernten `EPUB_COVER`; S1-Stand-Sätze in
  Bedienung/RPC/MCP ersetzt. `grep -rn "STUB:" crates` zeigt keine R20-Vermerke mehr.
- Nach dem Zusammenführen angepasste Tests (Verhalten von R20a, das reader/epub-pruefung im eigenen Worktree nicht
  kannten): Titel der Dateien tragen die Nummer aus dem Schema („2 Das Holz“, `round20_reader.rs`), das Testwerk
  bringt eigene Bilder mit (eigener Bildpfad im Reader-Test; `IMG_ALT_EMPTY` in `check.rs` gegen die Ausgangslage
  gezählt).
- Sichtprüfung (Firefox): Bilder und `book.css` luden im Rahmen nicht (403), weil Firefox den `Referer` der
  Unteranfragen aus dem `sandbox`-Dokument auf die Herkunft kürzt; `from_own_frame` nimmt jetzt den gekürzten
  `Referer` desselben Hosts an (Test in `round20_reader.rs` und Unit-Test). E-Ink-Profile (kindle, kobo, tolino):
  `filter: grayscale(1)` am `body` statt am `html`, sonst füllt der Papierton den Rahmen nicht ganz.
- Abnahme `tests/round20_abnahme.rs` (7 Tests, Werk „Das Feuer am Kap“, frei erfunden): Download über Menü/Route,
  RPC und CLI mit gleichen Bytes; Umschlag + Titelei + zwei Teile + drei Kapitel als eigene Dateien, nav/ncx, zwei
  Popup-Fußnoten mit Ziel, Querverweis `kC.xhtml#kN`, Metadaten mit `&`, `<`, `#[` wörtlich, Satz-Einstellungen im
  CSS, xmllint auf jede XML-Datei, eigene Prüfung ohne Fehler; Vorschau (fünf Pflicht-Profile, Auswahl per GET,
  Reader-Datei mit CSP/SAMEORIGIN, Hinweis `TEMPLATE_PAGEBREAK_MAPPED`, `..._UNMAPPED` bei einer Figur-Regel);
  absichtlich kaputte EPUBs (fehlende Datei, falscher bzw. komprimierter mimetype, kaputtes XHTML); EPUBCheck nicht
  gefunden bzw. als Skript im übergebenen PATH-Inhalt; MCP beide Werkzeuge ohne Commit, `epub_check` ohne EPUBCheck.

## Runde 31 – Register und Glossar

### Schnittstelle R31-S1

Vertrag R31 (PM, 2026-11-09). Eingefrorene Typen und Signaturen, Körper zum Teil als ehrliche Stubs
(`grep -rn "STUB:" crates`, Vermerk „(R31 …)“):

- `baruch-typst::index` (neu, Besitz register-satz): Registermarke `#metadata((register: "…", term: "…"))<baruch-idx>`
  (`IndexMark`, `mark_source`, `parse_mark` echt), `Occurrence` (`first`/`all`/n), eingebaute Register `stichwort`,
  `personen`, `bibel` (`builtin_registers`, `valid_register_slug`), `term_key` (NFC, Leerraum, klein; steht in der
  Datenbank, darum unveränderlich), `glossary_anchor` (`baruch-gl-UUID`), `BackMatter` mit `is_active`. Stubs:
  `sort_key` (= `term_key`), `find_marks`, `mark_word`, `insert_mark_at`, `remove_mark` (`IndexError::NotYet`),
  `Projector::project` (Identität), `back_matter` (""), `overview` (leer). Neue Abhängigkeiten `baruch-scripture`
  und `unicode-normalization`.
- `TypesetSettings` (eingefroren) hat drei neue Felder hinten: `glossary_page` (`GlossaryPosition`: vor/nach den
  Registern), `index_pages` (`IndexPages {registers, columns}`, 1–3 Spalten, höchstens 16 Register) und
  `glossary_first_mention`; `FIELDS` hat 19 Einträge. Der Regelblock „// Baruch: Satz“ erzeugt für sie keine Zeile.
- `baruch-wysiwyg`: `Inline::IndexMark {mark}` (JSON `{"t":"index_mark","mark":{…}}`); `parse` erkennt die
  kanonische Marke vor jedem anderen `#`-Ausdruck, `emit` schreibt `mark_source`, `html` zeigt sie wie eine Inline-Insel
  (`ws-island ws-index-mark`, `data-raw` = `mark_source`), damit das heutige `wysiwyg.ts` sie verlustfrei zurückschreibt.
- `baruch-epub`: `EpubInput::back_matter` (Vorgabe leer), `ItemKind::Glossary`/`Index`, Namensregeln
  `text/glossary.xhtml` (id `glossary`), `text/index-{slug}.xhtml` (id `index-{slug}`); `build` ignoriert das Feld noch.
- `baruch-store::glossary` (Besitz glossar-daten): Typen `GlossaryTerm`, `GlossaryEdit`, `GlossaryChange`,
  `IndexRegisterRow`, `NewIndexProposal`, `IndexProposal`, `IndexAccepted` und die `Store`-Methoden; S1 liest leer und
  lehnt Schreiben mit `StoreError::Glossary("noch nicht umgesetzt (R31 glossar-daten)")` ab. Neue `StoreError`-Varianten
  `Glossary`, `UnknownGlossaryTerm`, `UnknownIndexProposal` (RPC: INVALID_PARAMS). Keine Migration in S1 (0049 bringt
  glossar-daten).
- Store-Haken (Besitz register-satz): `snapshot::back_matter_in` (Satz-Einstellungen + eingebaute und eigene Register
  + angenommene Begriffe), `Store::back_matter`; `export_parts` liefert als drittes das Buchende, Export und
  Export-Karte hängen es über `snapshot::append_tail` an (Karte byte-gleich zum Export), `main.typ` hinter die
  `#include`-Zeilen; je Abschnitt läuft nach `label_section` ein `Projector::project`; der Spiegel bekommt
  `glossary::mirror_files` (`glossar.md`). Ohne R31-Einstellungen alles byte-gleich.
- Server: Module `glossary` (RPC `glossary.*`, `index.registers/register_*/proposals/decide`), `index_marks`
  (`index.entries/mark/unmark/overview`), `glossary_page` und `index_page` (Seiten), `mcp::glossary`, `assist::glossary`;
  Stubs antworten REFUSED „noch nicht umgesetzt (R31 …)“ (`glossary::registers` liefert schon die eingebauten).
  `epub.rs` setzt `back_matter`, `mirror::typeset_value` kennt die neuen Felder. Widgets `context-menu` und `glossary`
  (leer).

### 31a glossar-daten

- **Migration 0049** (`0049_glossary.sql`, einzige neue Migration der Aufgabe): `glossary_term` (Begriff, `term_key`,
  kurze/ausführliche Erklärung, Quelle, Bibelstellen als JSON-Liste, Status `accepted`/`proposed`, Herkunft, Zeiten;
  eindeutiger Teilindex `(work_id, term_key) WHERE status = 'accepted'`), `glossary_variant` (Varianten je Begriff in
  Reihenfolge, `form_key`, ohne uuid), `index_register` (eigene Register, eingebaute Kennungen per CHECK gesperrt,
  `UNIQUE (work_id, slug)`), `index_proposal` (Registervorschläge, ohne uuid – Arbeitsablauf der Instanz). uuid,
  Fülltrigger, Grabstein- und Kaskaden-Trigger für `glossary_term` und `index_register` wie 0048; `uuid_tombstone`
  bekommt die Arten `glossary_term` und `index_register` per Umbau (die siebzehn Grabstein-Trigger wörtlich neu). Kein
  `FOREIGN_KEYS_OFF` (kein Fremdschlüssel zeigt auf `uuid_tombstone`); Beleg `crates/baruch-store/tests/migration_0049.rs`.
- **Store** `baruch_store::glossary` (Vertrag §2.5): Lesen sortiert in Rust nach `baruch_typst::index::sort_key`;
  `glossary_lookup` über `term_key` bzw. `form_key` (nur angenommene); Schreiben prüft Grenzen `MAX_GLOSSARY_*` mit
  deutschen Meldungen (`StoreError::Glossary`), räumt auf (Begriff und Varianten in einer Zeile, Doppel weg) und lehnt
  Doppel unter den angenommenen ab; `glossary_propose` lehnt angenommene und schon vorgeschlagene Begriffe ab;
  `glossary_decide(false)` löscht den Vorschlag (Grabstein). Register: eingebaute und ungültige Kennungen abgelehnt,
  höchstens 13. `index_propose` prüft Marke (`IndexMark::validate`), Register (eingebaut oder eigen), festen Schlüssel
  (`GoneNode`) und ob das Wort in der aktuellen Fassung steht; `index_proposal_accept` wendet in **einer** Transaktion
  `mark_word` auf die aktuelle Fassung an und speichert eine neue Fassung des Menschen (`write::save_human`, danach
  `check_current_heading`); steht das Wort nicht mehr da, wird der Vorschlag `stale` (gespeichert) und der Text bleibt.
  Solange `mark_word` (register-satz) ehrlich `NotYet` meldet, lehnt die Übernahme mit dieser Meldung ab, nichts
  geändert. `accepted_entries_in`/`registers_in` speisen `Store::back_matter`; `mirror_files` liefert `glossar.md`
  (angenommene Begriffe alphabetisch mit Varianten, Erklärungen, Bibelstellen, Quelle; danach „Vorschläge (noch nicht
  angenommen)“ mit Herkunft; danach eigene Register) – ohne Begriffe und Register keine Datei.
- **Server** `crate::glossary` (§3.1): alle RPC-Methoden; schreibende Funktionen nehmen die Schreibsperre vor dem
  DB-Schreiben, genau ein Spiegel-Commit je Änderung über `mirror::record_glossary` (`describe_glossary`,
  `describe_register`, `describe_index_proposal`, `describe_index_accept`; Trailer `Baruch-Action`,
  `Baruch-Glossary`/`Baruch-Register`/`Baruch-Index-Proposal`, bei Vorschlägen `Persona` und Autor die Persona bzw. der
  Client, beim Entscheiden `Baruch-Proposer`). Bibelstellen prüft und normalisiert `glossary::normalize_bible_refs` mit
  `baruch_scripture::parse`. `glossary.suggest` (`assist::glossary`): Persona nach Lektor-Regel bzw. `persona`, ein
  Modellaufruf über den `ModelPort` (Antwortform „Begriff | kurze Erklärung“), Ergebnisse nur über
  `glossary::propose`; ein Fehler des Modells → REFUSED, nichts angelegt.
- **MCP** `mcp::glossary`: `glossary_list` (lesend), `glossary_propose` und `index_propose` (nur Vorschläge, Herkunft
  nach `acting_persona`, `node` → `isError`).
- **Austausch Unterversion 7** (`baruch-exchange`, XSD `docs/schema/baruch-werk-1-u7.xsd`, u6 eingefroren): Modell
  `ExchangeRegister`, `ExchangeGlossaryTerm`, `ExchangeGlossaryStatus`; Grabsteinarten `glossar-begriff`
  (`glossary_term`) und `register` (`index_register`) ab 7; `<satz>`-Attribute `glossarseite`, `registerseiten`,
  `registerseiten-spalten`, `glossar-erste-erwaehnung` (`typeset::R31_ATTRIBUTES`, gelesen erst ab 7 über
  `typeset::r31_from_attrs`). `exchange_dump` gibt Register und alle Begriffe aus, `exchange_apply` übernimmt sie neu
  mit Kennungen bzw. gleicht ab (bekannte Kennung nie überschrieben, gleicher `slug`/`term_key` unter fremder Kennung
  → Hinweis statt Doppel, Grabstein → „hier gelöscht“). Registervorschläge sind nicht im Format.

### 31b register-satz

**baruch-typst::index** (Körper; private Untermodule `index/scan.rs`, `index/project.rs`, `index/back.rs`):

- **Lesen** (`scan.rs`): je Byte Fließtext oder eine gesperrte Stelle mit eigener Nummer. Gesperrt sind
  Überschriftszeilen (`=`-Zeilen, Teil- und „ohne Nummer“-Markierungen), Maskierungen, Raw, Kommentare, Formeln, Labels,
  Verweise (`@name`), Adressen, Code hinter `#` samt Strings und Klammern, Anweisungen (`#let`, `#set`, `#show`, `#if` …
  bis zum Zeilenende samt aller Klammern) und Marken. Inhaltsblöcke von Aufrufen (`#emph[…]`, `#footnote[…]`,
  `caption: […]`) sind wieder Fließtext. Eine Einfügestelle ist erlaubt, wenn sie nicht zwischen zwei Bytes derselben
  gesperrten Stelle liegt, nicht am Ende einer Kommentar- oder Überschriftszeile, nicht am Anfang einer Überschriftszeile
  und nicht vor einem Label (es hinge sonst an der Marke).
- `find_marks` = alle kanonischen Marken in Textreihenfolge (auch in Fußnoten und Inhaltsblöcken, nie in Kommentar, Raw
  oder String). `mark_word` sucht das Wort wörtlich als ganzes Wort im Fließtext (Leerraum im Wort passt auf Leerraum
  ohne Leerzeile; Wort direkt vor einem Label → Marke hinter das Label), setzt `mark_source` direkt dahinter (erste,
  n-te oder alle Fundstellen); `insert_mark_at` prüft die Stelle; `remove_mark` nimmt nur einen Bereich, den
  `find_marks` liefert. `sort_key` = DIN 5007-1 (NFD ohne Akzente, ß = ss, klein, Satzzeichen am Anfang übergangen).
- **Projektion** (`project.rs`, `Projector`): (a) mit Register `bibel` in `index_pages.registers` hinter jede gültige
  Stelle im Fließtext (`baruch_scripture::scan` auf dem Text mit Leerzeichen statt gesperrter Bytes) eine Marke
  `register: "bibel"`, `term` = Buchname („Johannes“), `sub` = Kapitel und Verse („3,16“), `sort` =
  `Passage::sort_key` (`43.003.016-003.016`, biblische Reihenfolge); (b) mit `glossary_first_mention == Some(true)` je
  Kapitel (Überschrift Ebene 1, Teil oder „ohne Nummer“ Ebene 1 setzt zurück) die erste Fundstelle eines Begriffs oder
  einer Variante als ganze Wörter (Groß-/Kleinschreibung egal, Wortgruppen mit Leerraum, `-`, `'`, `’`, `.`). Form:
  mit Glossarseite `#link(<baruch-gl-UUID>)[#emph[Wort]]`, ohne `#emph[Wort]` – so gibt es nie einen Verweis auf ein
  fehlendes Label. Folgt `(`, `[`, `;` oder `.Wort`, steht ein `;` dahinter (beendet den eingebetteten Ausdruck, Typst
  setzt ihn nicht). Projektion nur im Export; SQLite bleibt unverändert.
- **Buchende** (`back.rs`, `back_matter`): "" ohne Registerseiten und Glossarseite; sonst ab `// Baruch: Register und
  Glossar` Hilfsfunktionen (`baruch-idx-ranges`, `baruch-idx-pages`, `baruch-idx-group`, `baruch-idx-entry`,
  `baruch-gl-entry`), dann Glossarseite (vor oder nach den Registern) und je Register aus `index_pages.registers`
  (unbekannte Kennungen fallen weg) eine Seite `#heading(level: 1, numbering: none)[…]`. Sortierung und Gruppierung in
  Rust (Buchstabengruppen nach `sort_key`, Bibel nach Sortierschlüssel), Seitenzahlen in Typst: `query(<baruch-idx>)`,
  je Marke `counter(page).at(m.location())` (dieselbe Zählung wie Fußzeile und „S. n“, auch ab dem ersten Kapitel),
  Folgeseiten „12–14“, Marken mit `see` ohne Seite. Spalten über `columns(n, gutter: 1.5em, …)`. Glossar: Begriff fett,
  Varianten in Klammern, kurze Erklärung, Absätze der ausführlichen, „Bibelstellen:“; Anker
  `#metadata("baruch-glossar")<baruch-gl-UUID>` (nur bei UUID aus Buchstaben, Ziffern, `-`, `_`). Alle Daten als
  maskierte String-Literale. Gleiche Eingabe → gleiche Bytes.
- `overview` (Übersicht je Register, gespeicherte Texte): alle Register aus `back.registers`, Einträge wie im Buchende
  sortiert, `keys` = Abschnitte mit Marken, die eine Seite tragen; automatische Bibelstellen nur mit eingeschaltetem
  Bibelstellenregister.

**baruch-scripture** (additiv): `book_order` (1–66), `Passage::sort_key`, `Passage::chapter_verse`.

**Store** (`snapshot.rs`, `export_map.rs`): die Haken aus S1 bleiben unverändert; geprüft in
`crates/baruch-store/tests/index_export.rs` (Goldens R29-S1 byte-gleich, Export = projizierte Abschnitte + Buchende,
Karte = Export, `main.typ` = vorher + Buchende, SQLite ohne Projektion).

**EPUB** (`build.rs`, `xhtml.rs`): `Inline::IndexMark` → `<span id="baruch-idx-{key}-{n}" class="baruch-idx">`
(`Renderer::index_marks`, `PlacedMark`); die erste Erwähnung (Insel `#link(<baruch-gl-…>)[#emph[…]]`) →
`<a epub:type="glossref" href="glossary.xhtml#baruch-gl-UUID"><em>…</em></a>` (ohne Glossarseite `<em>`); ein `;`
direkt hinter einem eingebetteten Ausdruck vor `(`, `[`, `;`, `.Wort` entfällt wie in Typst. Mit aktiven Einstellungen:
`text/glossary.xhtml` (`epub:type="glossary"`, `dt` glossterm mit `id="baruch-gl-UUID"`, `dd` glossdef) und
`text/index-{slug}.xhtml` (`epub:type="index"`, `index-entry`/`index-term`, je Abschnitt ein `index-locator` auf den
ersten Anker dort, beschriftet mit Nummer und Titel des Abschnitts), am Ende von Spine und Inhaltsverzeichnis in der
Reihenfolge des Satzes; `book.css` bekommt `INDEX_CSS` nur dann (sonst byte-gleich).

**Server** `crate::index_marks`: `index.mark`/`index.unmark` nehmen die Schreibsperre, setzen bzw. entfernen die Marke
in der Fassung `rev` und speichern mit `save_human_on` eine neue Fassung des Menschen; `mirror::record` schreibt genau
einen Commit. Register müssen in `glossary::registers` stehen. `index.entries` liest die Marken der aktuellen Fassungen
und die automatischen Bibelstellen (Projektion je Abschnitt); `index.overview` ruft `baruch_typst::index::overview`.

Bekannte Grenzen: Ein importierter Export mit Buchende nähme das Buchende in den letzten Abschnitt auf (der Import kennt
`// Baruch: Register und Glossar` noch nicht); Meldungen von Typst aus dem Buchende ordnet die Karte dem letzten
Abschnitt zu. Stichwörter gelten als gleich, wenn sie wörtlich gleich sind.

### 31c kontextmenue

- **Satzbild:** `baruch-wysiwyg` zeigt `Inline::IndexMark` als leeres `span.ws-index-mark` (Felder in `data-*`,
  Beschreibung aus `index_mark_label` in `title`/`aria-label`, Zeichen nur im Stil); `wysiwyg.ts` liest es als
  `{t: "index_mark", mark}` zurück und führt den Befehl `index_mark` (`add|edit|remove`, `ws_command.ts`) aus. Einzelheiten
  in `docs/WYSIWYG.md` („Registermarken und Kontextmenü“).
- **Kontextmenü:** `web/src/context_menu.ts` (Widget `context-menu` auf `span.ribbon-index` im Menüband,
  `fragments/edit_toolbar.html`) hängt sich an Satzbild und Textfeld desselben Editors; Befehle ans Satzbild nur über
  `baruch:ws-command`, kein Import von `wysiwyg.ts`. RPC: `glossary.add`, `glossary.lookup`, `index.registers`. Reine
  Bausteine (`menuItems`, `opensMenu`, `menuKey`, `markFromFields`, `glossaryParams`, `entryLine`) und in `ws_command.ts`
  die Marken-Bausteine (`markSource`, `parseMarkAt`, `findMarks`, `validateMark`, `markLabel`, `markInSource`) – sie
  folgen `baruch_typst::index` und werden unter Node gegen die Rust-Funktionen geprüft.
- **Seiten ohne JavaScript:** `src/glossary_page.rs` (`/werke/{id}/glossar`, `POST …/glossar/act`, `/werke/{id}/markierung`,
  Vorlagen `glossary.html`, `marking.html`, `fragments/context_menu.html`) und `src/index_page.rs` (`/werke/{id}/register`,
  `POST …/register/act`, Vorlage `index_page.html`). Keine eigene Datenhaltung: gelesen und geschrieben wird nur über
  `crate::glossary` (`call("glossary.list"|"glossary.lookup"|"index.proposals")`, `add`, `update`, `delete`, `decide`,
  `registers`, `register_*`, `index_decide`) und `crate::index_marks` (`entries`, `overview`, `mark`, `unmark`); deren
  Meldungen erscheinen auf der Seite bzw. als 422 mit den Eingaben. `glossar/act` mit `action=suggest` ruft
  `crate::assist::glossary::suggest` (Nachbesserung R31). Erfolg → 303 auf `back`
  (nur Pfade dieses Servers, `glossary_page::safe_back`), sonst die Seite, mit `ok=<action>` vor dem Anker
  (`#begriff-{id}`, `#register-{slug}`, `#vorschlag-{id}`). Stile `static/glossary.css`, `static/context_menu.css`;
  Widget `web/src/glossary.ts` (Filter der Listen).
- **Satz-Einstellungen:** `settings/work_typeset.rs` liest und schreibt die drei R31-Felder (`glossary_page`, je Register
  ein Häkchen `index_register` in Satzreihenfolge + `index_columns` (leer = 2), `glossary_first_mention`);
  `BlockInput::registers` kommt aus `crate::glossary::registers`. `formToSettings` (`typeset_settings.ts`) liest dieselben
  Felder (Node-Vergleichstest in `tests/round31_kontextmenue.rs`).

### Integration

- Zusammenführen ohne Konflikte (`--no-ff`, Reihenfolge S1 → glossar-daten → register-satz → kontextmenue).
- Integrator: `mcp.rs`-`instructions` nennen `glossary_list`, `glossary_propose` und `index_propose` (nur Vorschläge);
  die S1-Ausnahmen in `tests/mcp.rs` (STUB-Beschreibungen) und `tests/round29_oberflaeche.rs` (drei R31-Felder im
  Satz-Block) sind entfallen; S1-Stand-Sätze in Bedienung/RPC/MCP/SKILLS ersetzt. `grep -rn "STUB:" crates` zeigt
  außerhalb der Tests nichts.
- **Import eines Exports mit Buchende** (offener Punkt von register-satz): `baruch_typst::index::strip_back_matter`
  schneidet alles ab der Zeile `// Baruch: Register und Glossar` ab, `strip_projection` nimmt die Zusätze der Projektion
  heraus (automatische Bibelstellen-Marken = Register `bibel` mit `sort`, verlinkte erste Erwähnung
  `#link(<baruch-gl-…>)[#emph[…]]` samt Schutz-`;`). `import::import_source` wendet beides an, `strip_projection` nur,
  wenn ein Buchende gefunden wurde (also nur bei Exporten von Baruch). Sonst wanderte das Buchende in den letzten
  Abschnitt, und der Text verwiese auf Glossar-Labels, die es im neuen Werk nicht gibt (Typst-Fehler). Die Auszeichnung
  ohne Glossarseite (`#emph[…]`) ist von Hand geschriebener Hervorhebung nicht zu unterscheiden und bleibt stehen.
- Abnahme `tests/round31_abnahme.rs` (5 Tests, Werk „Die Wärterin vom Kap“, frei erfunden, zwei Teile, drei Kapitel,
  Bibelstellen, zwei Personen): Glossar über die Seite „Markierung“ und `/glossar/act` (ohne JavaScript) und über
  `glossary.add` (wie der Dialog) → Glossar-Seite, Glossarseite im PDF (pdftotext: Begriff, Variante, kurze und
  ausführliche Erklärung, Bibelstellen), EPUB `glossary.xhtml` mit `glossterm`/`glossdef` und `glossref` im Kapitel;
  Marken über `/register/act` (alle 70 Fundstellen) und `index.mark` (Unterstichwort, „siehe auch“, Personen) →
  Registerseiten mit gemessenen Seiten („Nebel 3–6“), Bibelstellenregister in biblischer Reihenfolge (1. Mose, Psalmen,
  Johannes, Römer; Stelle im Kommentar fehlt), erste Erwähnung je Kapitel verlinkt; der Typst-Export setzt eigenständig
  dieselben Register; Persona (`ScriptedModel`) und MCP schlagen nur vor, der Mensch übernimmt per Formular (Commits mit
  `Baruch-Proposer`); Rundreise u7 zwischen zwei Wegwerf-Instanzen (Export samt Buchende byte-gleich, Grabstein, kein
  Doppel beim Abgleich); ohne R31-Einstellungen bleiben Export und EPUB byte-gleich, auch mit Glossar und eigenem
  Register; Import eines Exports mit Buchende; Tastatur (Kontextmenü-Taste, Umschalt+F10) und ARIA im ausgelieferten
  Kontextmenü, Link „Markierung …“ ohne JavaScript.
- Nach dem Zusammenführen: Die Sonde in `round31_kontextmenue.rs` (`register_forms_check_their_inputs_and_keep_them_on_422`)
  rief `index.mark` auf und legte damit selbst eine neue Fassung an, die die des Formulars verdeckte – der Datenweg war vor
  dem Zusammenführen ungeprüft. Sonde jetzt lesend (`index.entries`).
- Sichtprüfung (Wegwerf-Server mit `--secrets memory`, Verbindungen auf 127.0.0.1:9, keine Anfrage an Eliezer oder ins
  Netz; Bilder `.claude/sicht/r31/`): Kontextmenü im Satzbild mit „In Glossar aufnehmen“, Seite „An der Markierung“,
  Glossar-Seite mit Vorschlag von MCP, Registerseite, PDF-Seiten (Stichwortregister „Nebel 3–4“, Bibelstellenregister in
  biblischer Reihenfolge, Glossarseite, erste Erwähnung kursiv), EPUB-Vorschau mit Glossar. **Behoben:** Die
  Registerseite nannte Fundstellen und Vorschläge mit dem inneren Schlüssel („k13“) und die Fundstelle englisch
  („first“) – jetzt Nummer und Titel des Abschnitts („1 Die Ankunft“) und „erste Fundstelle“; die Glossar-Seite schrieb
  bei Begriffen des Menschen „Vorgeschlagen von Autor“ – „Vorgeschlagen von“ steht nur noch bei Herkunft `@…`.
- `typst` (CLI) ist auf diesem Rechner nicht im PATH: die `typst compile`-Zusicherungen melden „ÜBERSPRUNGEN“, der Export
  wird stattdessen mit dem eingebetteten Compiler als eigenständige Datei gesetzt (gleiche Typst-Version, keine
  weiteren Dateien).

### Nachbesserung nach Gutachten (R31)

- **Keine doppelten Marken:** `baruch_typst::index::mark_word` prüft je gewählter Fundstelle die Kette der Marken direkt
  hinter dem Wort bzw. Label (`carries_mark` über `Scan::marks`); steht dieselbe `IndexMark` schon dort, bleibt die
  Fundstelle unberührt. Bleibt keine übrig → `IndexError::AlreadyMarked` (ersetzt das ungenutzte `NotYet`), der
  Aufrufer legt keine Fassung an; `index_proposal_accept` setzt den Vorschlag dann auf `stale`. „erste“/„n-te“ zählen
  weiter alle Fundstellen im Text (Auswahl der Oberfläche bleibt eindeutig). `markInSource` (TS) prüft dasselbe im
  Quelltext, `sameMarkBehind` (`wysiwyg.ts`) im Satzbild. Vorher ließ `Scan::insertable` die Stelle vor einer
  vorhandenen Marke zu, und jede Wiederholung war eine neue Fassung samt Spiegel-Commit.
- **„Begriffe vorschlagen lassen“ bedienbar:** Formulare auf der Glossarseite und auf „An der Markierung“
  (`glossar/act`, `action=suggest`, wahlweise `key`); Weiterleitung mit `neu`/`uebersprungen` für den Hinweis.
- **Tests ohne S1-Zweige:** Wie in R27 sind die Zweige, die „noch nicht umgesetzt“ als richtig gelten ließen, entfernt
  (`round31_kontextmenue.rs` mit `implemented`, `round31_glossar.rs`, `baruch-store/tests/glossary.rs`); die
  422-Fälle mit erhaltenen Eingaben prüfen echte Fehler (unbekannte Bibelstelle, eingebaute Register-Kennung, Marke
  schon gesetzt).

## Runde 21 – Import und Bilder

### Schnittstelle R21-S1

- **Abhängigkeiten** (in S1 festgelegt, Entwickler ändern keine Cargo.toml/Cargo.lock): baruch-server `zip` 2.4 (nur
  DEFLATE; aus den dev-dependencies in die dependencies, für `src/archive.rs`), `quick-xml` 0.38 (EPUB, 21a; schon im
  Lock), `pulldown-cmark` 0.13 ohne Standard-Features (Markdown, 21b; aufgelöst zu 0.13.6); baruch-render
  `typst-render` 0.15.1 (Rasterbild einer Seite für das EPUB-Cover, 21c; ließ sich laden, der Ersatz resvg/image war
  nicht nötig). `cargo tree -d` zeigt keine neuen Doppel-Versionen.
- **`src/archive.rs`** (in S1 vollständig, danach markdown-import): `read_zip(bytes, &ArchiveLimits)` →
  `Vec<ArchiveEntry {path, bytes}>` in Archivreihenfolge, nur reguläre Dateien (Ordner übersprungen). Abgelehnt mit
  `ArchiveError::Unsafe {path, reason}`: absolute Pfade, `..`, `\`, Laufwerk, Steuerzeichen, leere oder Nicht-UTF-8-Namen,
  Verknüpfungen, Sonderdateien, Verschlüsselung und doppelte Namen (auch nur in Groß-/Kleinschreibung verschieden; das
  Zentralverzeichnis wird ab dem Endsatz selbst gezählt, weil die ZIP-Bibliothek Doppel still zusammenfasst).
  `TooLarge`: mehr Einträge, größere Einträge oder Summe als `ArchiveLimits` – gezählt beim Entpacken mit `take`, nicht
  aus dem Kopf –, Verhältnis entpackt/gepackt über `max_ratio` (Einträge bis `RATIO_FREE_BYTES` = 64 KiB sind davon
  frei). `Corrupt`: unlesbar, falsche Prüfsumme, anderes Verfahren als STORED/DEFLATE. `NotZip`: Anfang nicht `PK`.
  Grenzen für Importe `IMPORT_LIMITS`: 2000 Einträge, je 16 MiB, zusammen 80 MiB, Verhältnis 200.
- **`src/import.rs`**: `ImportWarning {code, message, line}` (Hinweis statt stillem Verwerfen), Feld
  `ImportReport.warnings` (hinter `skipped`), `import_source(…, warnings)`; neu `import_generated(state, source,
  file_name, title, assets, skipped, warnings)` für fertig erzeugten Typst-Text (PDF/EPUB nach Bestätigung): dünne Hülle
  um `import_source`, vergebener Titel → `REFUSED` mit `TITLE_HINT`, keine Überschrift → `INVALID_PARAMS`.
- **CLI-Weiche** (`main.rs`, `import DATEI`): `exchange::is_exchange_name` → Austausch; `pdf_import::is_source_name`
  (.pdf/.epub) → `pdf_import::cli_import` mit `CliSourceImport {title, apply: --uebernehmen, library: --bibliothek}`;
  `markdown_import::is_markdown_name`/`is_archive_name` (.md/.markdown/.zip) → `markdown_import::cli_import`; sonst Typst.
  `--uebernehmen`/`--bibliothek` mit anderer Dateiart → Exit 2. Der Typst-Import druckt je Warnung
  `Hinweis (<code>): <message>` auf stderr.
- **Routen und RPC**: `pdf_import::pages()` (`GET /werke/import/quelle`) bei den Seiten, `pdf_import::writes()`
  (`POST /werke/import/quelle/vorschau` mit `DefaultBodyLimit::max(UPLOAD_LIMIT)` = 64 MiB + 64 KiB, `…/uebernehmen`,
  `…/verwerfen`) hinter `guard::same_origin`; RPC `source_import.*` über `pdf_import::handles`/`call` hinter
  `works::handles`. Markdown-Routen bleiben in `works.rs`. Stil `static/source_import.css` im gemeinsamen `STYLE`.
  Menü „Baruch ▾ → PDF oder EPUB importieren…“ in `base.html`, Verweis unter dem Typst-Formular in
  `fragments/work_new.html`. In S1 antworten Seite (200), Formulare (501), RPC (-32000) und CLI (Exit 1) ehrlich
  „noch nicht umgesetzt“ (`// STUB: … (R21a/R21b)`).
- **Migrationen**: S1 legt keine an. Reserviert, nur falls nötig: Store `0050_r21_assets.sql` (bilder),
  `0051_r21_import.sql` (markdown-import); Bibliothek `0108_title_text.sql` (pdf-import, Tabelle `library_title_text`).

### 21a pdf-import

Modul `src/pdf_import/` (global, ohne Werk), Bibliothek `0108_title_text.sql`, Volltext in `literature/fulltext.rs`.

| Datei | Inhalt |
|---|---|
| `mod.rs` | RPC `source_import.*`, `preview`/`preview_with` (Weg wählen, lesen, Entwurf), `apply`/`apply_draft` (Werk über `import::import_generated`, Quelle über `baruch_library::add_title` + `set_title_remote` bzw. `set_title_text`), `discard`, CLI `cli_import`/`cli_import_with` (eigene `PdfTools` für Tests), `local_pdf_reads` (Zähler je Datenordner: Tests prüfen „kein pdftotext auf dem MultiGPT-Weg“) |
| `pdf.rs` | lokal: `pdfinfo -enc UTF-8` (Titel, Autor, Seiten) und `pdftotext -enc UTF-8 -bbox-layout DATEI -` über `tokio::process` mit `TOOL_TIMEOUT` (120 s), `kill_on_drop`, Standardeingabe `null`, nur Seiten 1–`MAX_PAGES` (2000, wie über MultiGPT; mehr → Warnung `import-pdf-pages-cut`), Ausgabe ≤ 128 MiB, höchstens `MAX_PARALLEL_READS` (2) Lesevorgänge zugleich (weitere warten); fehlt das Programm → ehrliche Meldung. Gliederung aus den Wortrahmen (`structure`) |
| `remote.rs` | über MultiGPT: `upload_document` (genau einmal), Warten (`run_status`, sonst `read_document` mit Wiederholung, ≤ `INDEX_WAIT` 120 s, Pausen 0,1–2 s), `document_info` (Titel, Autor, Seiten, `toc`), Text mit `read_document {page_from, page_to}` in Bereichen ≤ 10 Seiten (gekürzt → Seite für Seite), Schnitt am Inhaltsverzeichnis (`split_by_toc`), ohne Verzeichnis nummerierte Zeilen (`split_by_heuristic`) |
| `epub.rs` | EPUB: `archive::read_zip` mit `IMPORT_LIMITS`, `container.xml` → OPF (Titel, `dc:creator`, Sprache, Manifest, Spine), Navigation aus `nav.xhtml` (`epub:type="toc"`) bzw. `toc.ncx`, XHTML → Textfluss mit Sprungzielen; Verweise über `resolve` (Prozent-Kodierung, `..` nur innerhalb des Archivs) |
| `typst_out.rs` | `escape_text` (Typst-Zeichen `\ # * _ ` $ @ < > [ ] ~`, `/` vor `/`/`*`, `-` vor `-`/`?`, am Absatzanfang `= - + /` und `12.`), `Inline` → Markup (`_…_`/`*…*` nur an Wortgrenzen, sonst `#emph[…]`/`#strong[…]`; `#footnote[…]`, `#super`, `#sub`, `#link`, `#raw`, `#linebreak()`), `assign`/`build_typst` (bestätigte Gliederung → Typst; Teil über `baruch_typst::heading_line(0, …)`) |
| `xml.rs` | kleiner nachsichtiger XML-Baum über quick-xml 0.38 (`check_end_names` aus, HTML-Entitäten, ≤ 256 Ebenen, ≤ 2 Mio. Knoten) |
| `draft.rs` | Entwürfe unter `<data>/import-drafts/<32 hex>/` (`draft.json`, `asset-<n>.bin`; nach Fehler von MultiGPT `pending.json` + `source.bin` für „lokal lesen“); `clean` vor jedem `preview`: älter als 24 h weg, dann die ältesten bis < 20 |
| `pages.rs` | `GET /werke/import/quelle`, `POST …/vorschau` (multipart, `UPLOAD_LIMIT`), `…/uebernehmen`, `…/verwerfen`; Vorlage `templates/source_import.html`, Stil `static/source_import.css` |

- **Gemeinsames Modell:** `Extracted {title, author, lang, pages, sections: [Section {level, title, part, body (Typst),
  plain}], assets, skipped, warnings, plain_text}`. Die Vorschau zeigt `sections` als Gliederung; `apply` bildet die
  bestätigte Liste (`Confirmed {level, title, part, source}`) auf die Abschnitte ab. Text vor dem ersten Punkt wird der
  Punkt „Vorspann“. Werkvorlage des neuen Werks: `#set document(title, author)` und – bei EPUB mit `dc:language` –
  `#set text(lang)`.
- **PDF-Heuristik (lokal):** Zeilenhöhe = Median der Wortrahmen; Fließtext = häufigste Höhenklasse (halbe Punkte,
  nach Zeichen gewichtet); Überschrift = Block mit ≤ 3 Zeilen, ≤ 160 Zeichen, Höhe ≥ 1,12 × Fließtext; Ebenen nach
  absteigender Höhe (≤ 3); einzeilige Blöcke in Fließtextgröße mit Nummer `1.2.3 Titel` ohne Satzzeichen am Ende gelten
  als Überschrift (Ebene = Zahl der Nummernteile, Nummer entfernt). Größte Klasse genau einmal auf Seite 1 vor jeder
  anderen Überschrift = Titel des Dokuments. Reine Ziffernzeilen am Seitenrand = Seitenzahlen (entfernt). Zeilen eines
  Blocks werden mit Leerzeichen verbunden, `Wort-` + Kleinbuchstabe ohne Trennstrich.
- **EPUB-Umwandlung:** Blöcke (`p`, Überschriften, Listen als `-`/`+` mit Einzug, `blockquote` → `#quote(block:
  true)[…]`, `figure`/`img` → `#figure(image("pfad", alt: …), caption: […])` bzw. `#image(…)`, `pre` → `#raw(block:
  true, …)`, `hr` → `#line(length: 100%)`, Tabellen als Zeilen „a | b“ mit Warnung). Fußnoten: Elemente mit
  `epub:type` `footnote`/`endnote`/`rearnote`/`note` bzw. `role="doc-footnote|doc-endnote"`, auf die ein Verweis zeigt
  (`noteref`/`doc-noteref` oder `href` auf eine solche Note), fehlen im Fluss und stehen als `#footnote[…]` am Zeichen
  (Rücksprünge weg; eine führende Zahl nur, wenn sie gleich dem Fußnotenzeichen ist und kein Rücksprung-Link die
  Nummer trug – „1 Kor 13,4“ bleibt –, mit Warnung `import-epub-footnote-number`; Bilder in der Fußnote bleiben als
  `#image(…)` in ihr). Hinter jedem eingebetteten Ausdruck (`#linebreak()`, `#emph[…]`, `#raw(…)`, `#footnote[…]` …)
  steht ein Schutz-`;`, wenn danach `(`, `[`, `;` oder `.Name` folgt (`typst_out::needs_guard`, wie beim
  Markdown-Import), sonst läse Typst einen Aufruf bzw. Feldzugriff. Bilder: Pfad im Werk relativ zum Ordner der OPF-Datei, geprüft mit
  `check_asset_path`, ≤ 8 MiB je Bild, ≤ 64 MiB zusammen, nur PNG/JPEG/GIF/WebP/SVG (`detect_mime`), sonst übersprungen
  mit Grund. Navigation: Ziel `datei#id` → Sprungziel im Fluss; die Überschrift gleich dem Navigationstitel fällt aus dem
  Text; andere Überschriften bleiben fetter Text (Warnung). Tiefer als 3 → Ebene 3 (Warnung).
- **Bibliothek:** `library_title_text (title_id → library_title ON DELETE CASCADE, source_name, mime, sha256, pages,
  text CHECK typeof = 'text', created)` – nur neue Tabelle, Migrationstests `baruch-library/tests/migration_0108.rs`
  (Kern-Fixture) und `baruch-server/tests/round21_pdf_import.rs` (echtes Kernschema von development). Funktionen
  `set_title_text`, `title_text`, `title_text_created`, `title_for_text_sha256` (Doppel erkennen).
- **Volltext (`literature/fulltext.rs`):** `read(Target::Title)` liest ohne Verknüpfung den lokalen Volltext
  (`origin: "import"`, geteilt wie eine lokale Kopie über `local_part`), `check_highlight` prüft gegen ihn,
  `has_fulltext(state, title)` = Verknüpfung oder lokaler Volltext. Die Dokumentansicht `/quelle` (`literature.rs`,
  `source_page`) fragt seit der Integration `fulltext::has_fulltext` statt nur `title_remote` und zeigt so auch den
  lokalen Volltext in Teilen samt den Feldern Anfang/Ende (Abnahme `round21_abnahme.rs`).
- **Regeln:** Gliederung nie still (immer Vorschau; CLI ohne Flags schreibt nichts), Teil nur auf Bestätigung, kein
  stiller Wechsel des Wegs, Warnungen statt Verwerfen, kein Import über MCP, Inhalte der Datei sind Daten (nur maskiert
  in Seiten und Typst).

### 21b markdown-import

- **Module** (`crates/baruch-server/src/markdown_import/`): `convert.rs` (Markdown → Typst, rein, ohne Zustand),
  `mod.rs` (Namen, `Bundle` für ZIP und Ordner, Haupt-Datei, Bilder aus dem Bündel, `import_text`, `import_bundle`,
  `import_archive`, `cli_import`, `report_text`). Angelegt wird immer über `import::import_generated` (eine Transaktion
  für Werk und Bilder, Titelregeln wie beim Typst-Import). Formulare und RPC in `works.rs`, Bericht
  `templates/import_report.html`. Keine Migration (0051 blieb frei).
- **Umwandlung** (`convert::to_typst`, `pulldown-cmark` 0.13 mit Fußnoten, Tabellen, Durchgestrichenem,
  Begriffslisten, Überschriften-Attributen und YAML-Kopf; Ereignisse → kleiner Baum → Ausgabe):

| Markdown | Typst | Hinweis-Code, wenn nicht abgebildet |
|---|---|---|
| `#`/`##`/`###` | `heading_line(1…3)` (`escape_title`) | `md-heading-level` (`####`–`######` → `===`) |
| Auszeichnung in einer Überschrift | Titel als Text | `md-heading-format`; Fußnote darin `md-heading-footnote`; `{#id}` `md-heading-attributes` |
| Überschrift in Zitat/Liste | `#strong[…]` | `md-heading-nested` |
| Text vor der ersten Überschrift | `= <Titel>` davor | `md-preamble-heading` |
| YAML-Kopf `title`/`author` | `#set document(title: …, author: …)` (Werkvorlage) | `md-front-matter` für andere Schlüssel |
| Absatz, weicher Umbruch, harter Umbruch | Absatz, Zeilenende, `\` + Zeilenende | – |
| `*…*`, `**…**`, `~~…~~` | `#emph[…]`, `#strong[…]`, `#strike[…]` | – |
| `` `Code` `` | `#raw("…")` | – |
| Codeblock | ```` ```sprache ```` (Zaun länger als jede Backtick-Folge darin) | `md-code-lang` (Sprache mit anderen Zeichen) |
| Liste, Aufzählung (Startnummer), verschachtelt | `- `, `n. `, Einzug = Breite des Zeichens | – |
| `> Zitat` | `#quote(block: true)[…]` | `md-quote-kind` (`> [!NOTE]`) |
| `[Text](https:…)`, `<https:…>`, E-Mail | `#link("…")[Text]` bzw. `#link("…")` | `md-link-local` (andere Datei, `#Sprung`, fremdes Schema: nur der Text) |
| Fußnote | `#footnote[…]` am Verweis | `md-footnote-repeated`, `md-footnote-unused`, `md-footnote-nested`, `md-footnote-missing` |
| Bild allein im Absatz | `#image("p", alt: "…")`, mit Titel `#figure(image(…), caption: […])` | `md-image-remote` (Netz → `#link`), `md-image-data` (`data:`), `md-image-path` (kaputte %-Folge) |
| Bild im Fließtext | `#box(image(…))` | `md-image-title` (Titel entfällt) |
| Tabelle | `#table(columns: n, align: (…), table.header(…), …)` | – |
| Begriffsliste | `/ Begriff: Erklärung` (`:` im Begriff maskiert) | `md-definition-multiple` (je Erklärung ein Eintrag) |
| Trennlinie | `#line(length: 100%)` | – |
| `<br>` | `\ ` | – |
| HTML im Text / HTML-Block / HTML-Kommentar | entfällt (Text bleibt) / `// …`-Kommentar / entfällt | `md-html` / `md-html-block` / `md-html-comment` |
| unbekannte Form | Text als Absatz | `md-unknown` |

  Maskierung (`convert::escape`, `escape_line_starts`): `\ # * _ $ @ < [ ] ~` und der Backtick, `/` vor `/`/`*`, `-` vor `-`/`?`; am
  Zeilenanfang (nach Einzug) `= + - /` und `Ziffern.` – idempotent. Hinter `#…[…]`/`#…(…)` steht vor `(`, `[`, `;` und
  `.Buchstabe` ein `;` (wie `needs_guard` der Projektion). Höchstens 200 Hinweise je Datei, dann
  `md-warnings-truncated`. Zeilen sind 1-basiert in der Markdown-Datei (`into_offset_iter`).
- **Bündel** (`Bundle::from_archive` über `archive::read_zip` mit `IMPORT_LIMITS`; `Bundle::from_files` für Ordner mit
  denselben Pfadregeln, Doppeln ohne Groß-/Kleinschreibung und Grenzen): Haupt-Datei = einzige `.typ`/`.md`/`.markdown`,
  sonst die flachste `index.md`/`main.typ`/`README.md`, sonst Fehler mit Kandidaten (-32602/422). Bilder: Pfade aus
  `image_paths` des (umgewandelten) Typst-Texts, `check_asset_path`, Normalform, relativ zum Ordner der Haupt-Datei
  (führendes `/` = dieser Ordner), Prüfung `import::check_image` (8 MiB, zusammen 64 MiB, Bildtyp) – Gründe wie beim
  Typst-Import. Unbenutzte Dateien: ein Hinweis `archive-unused` (bis zehn Namen). `file_name` des Werks = Haupt-Datei,
  bei `index.md`/`main.typ`/`README.md` der Name des Archivs bzw. Ordners mit deren Endung (Titel-Rückfall).
- **Wege:** `POST /werke/import` (multipart `file` | `files` + `title`, `IMPORT_UPLOAD_LIMIT` = 80 MiB + 64 KiB nur an
  dieser Route; `TooLarge` → 413, andere Archivfehler → 422; ohne Hinweise und übersprungene Bilder 303, sonst 200
  Bericht), RPC `work.import` (`.md` als Text, Bilder in `skipped`), `work.import_archive` (Base64-ZIP), CLI
  `markdown_import::cli_import` (Markdown: Bilder von der Platte über `import::read_assets_in` wie beim Typst-Import;
  ZIP: aus dem Archiv; Bericht mit `Hinweis (<code>, Zeile n)` auf stdout).
- **`export --out`** (`import::check_targets` vor der ersten Datei): `out` ist kein Nicht-Ordner; keine Zieldatei
  vorhanden; kein Pfad Präfix-Ordner eines anderen (`a.png` vs. `a.png/b.svg`, Komponenten nach Normalform); kein
  vorhandener Zwischenordner eine Verknüpfung oder Datei. Beim Schreiben zusätzlich `canonicalize(parent)` unter
  `canonicalize(out)` (gegen Änderungen zwischen Prüfung und Schreiben).
- **Satzbild** (`baruch-wysiwyg/src/html.rs`): `Soft` direkt hinter `Linebreak` als leeres
  `<span class="ws-soft" data-after-break="1">` (keine Einrückung; das Widget liest es weiter als `soft`); eine Bildinsel
  mit mehreren Bildern nennt ohne `caption:` jedes Bild in der Unterschrift und alle Pfade in
  `<span class="ws-alt ws-paths">Bilder: …</span>`. Begriffslisten bleiben ohne Schachtelung (Grenze in
  `docs/WYSIWYG.md`).

### 21c bilder

Keine Migration (0050 nicht gebraucht): Kontingent und Aufräumen arbeiten auf vorhandenen Zeilen.

- **Typwechsel:** RPC `asset.replace {rename}`, Formularfeld `rename` („Endung anpassen“), MCP `asset_replace {rename}`
  → `Store::asset_request_with(…, rename)` (Feld `rename` in `params` von `replace`, fehlt in alten Zeilen = falsch;
  `AssetRequest.rename`); Bestätigen ruft `replace_asset_renaming`. Satz am alten Pfad: `baruch_render`
  `with_image_formats` (siehe „Bilder (Migration 0009)“), gemessen mit `pdfimages -list` im Test.
- **Altbestand:** `asset.path_conflicts`, `asset.path_resolve`, Hinweis „Pfade mit zwei Bildern“ oben auf der Seite
  „Bilder“ mit Formular `POST /werke/bilder/act` (`action=resolve_path`).
- **Kontingent** (`mcp/assets.rs`, `check_quota` unter `write_lock` vor `add_asset` bzw. `asset_request_with`):
  `Store::asset_mcp_usage(work, origin_name)` → `AssetUsage {writes, bytes, open_replace}` – `asset_revision` mit
  `author_name` = Antragsteller und `created` ≥ jetzt − 1 Tag (Anzahl, Summe `bytes`) plus `asset_request` `op =
  'replace'` desselben `origin_name` im Zeitraum (Anzahl, Größe aus `params.bytes` bzw. dem Inhalt), dazu offene
  Ersetzen-Anträge. Grenzen `MAX_MCP_ASSET_WRITES_PER_DAY` 30, `MAX_MCP_ASSET_BYTES_PER_DAY` 64 MiB,
  `MAX_MCP_OPEN_REPLACE_REQUESTS` 10.
- **Aufräumen:** `Store::purge_decided_asset_requests(work, ASSET_REQUEST_PURGE_DAYS = 30)` setzt in `params` von
  entschiedenen `replace`-Anträgen mit `decided` < jetzt − 30 Tage `content = ""`, `purged = true`, `bytes` = Größe;
  aufgerufen von `asset_requests::purge` vor Liste, Entscheidung, Abschnitt „Anträge“, MCP `asset_requests` und der
  Antragsdatei (dann 410 Gone).
- **ImageView:** `rpc::LibraryView.images` (`wysiwyg::image_view` liefert `image_alts` und `ImageView` in einem Gang);
  der reservierte Schlüssel `IMAGE_VIEW_KEY` in `image_alts` und `ImageView::from_alts` sind entfallen.
- **Literatur:** `literature::image_hits` (alle Wörter in Name/Bildunterschrift/Alternativtext der aktuellen Fassung,
  höchstens `MAX_IMAGE_HITS` = 24), eigener kleiner Vorlagentext `IMAGE_HITS_TEMPLATE` (Autoescape HTML); angehängt an
  `literature.render` (Popup) und als `images_html` in `literature.html` (`/literatur`). `fragments/library.html` bleibt
  unberührt; die erste Arbeitsseite mit `?lq=` zeigt den Kasten erst, wenn das Popup über `literature.render` lädt
  (das tut es beim Öffnen immer).
- **Bild einfügen:** `assets::insert_figure_at` mit `Offset` rückt in einem Absatz über offene Klammern
  (`open_brackets`: `[ ( {`, `\`-Maskierung, Rohtext, Zeichenketten in Code) hinweg an die nächste Zeilengrenze mit
  geschlossenen Klammern, sonst hinter den Absatz.
- **EPUB-Cover:** siehe „Bilder, Umschlag und PDF“; `epub::epub_cover` → `(Option<EpubImage>, Vec<EpubWarning>)`,
  Warnung `COVER_RASTER_FAILED` (vorn in `EpubBook.warnings`, `book_for`), Rückfall die rohe Bilddatei. `EpubInput`
  bleibt unverändert (Typen seit R20-S1 eingefroren).
- **`pdfinfo`:** Alle Typst-PDFs (Buch, Stichwortzettel, Karte, Umschlag) tragen `/MarkInfo << /Marked true
  /Suspects false >>` (krilla 0.8.2, `chunk_container.rs`: „We always set suspects to false because it's required by
  PDF/UA“). poppler 25.03 meldet jeden Wert außer `true` als „Syntax Error: Suspects object is wrong type
  (boolean)“. Belegt im Test: dieselbe Datei mit `/Xuspects false` (gleiche Länge, Querverweise stimmen) meldet
  nichts. Nicht behoben (das PDF ist gültig; PDF/UA verlangt den Wert).

## Runde 22 – Revisionen und Spiegel

Migration 0050, Fortschreiben (`baruch_core::revision_policy`, `baruch-store/src/amend.rs`), Seitenzweige,
Rückfrage erledigter Befunde, Archiv, Entwurf als Wasserzeichen, Austausch Unterversion 8.

### fassungen

#### Migration 0050 (Runde 22)

`0050_revision_amend.sql`: `revision.amend_seq` (Stand, 0 = nie fortgeschrieben) und `revision.based_on_amend`
(Stand der Basis eines Vorschlags); Tabelle `revision_amend` (je Schritt: `revision_id`, `seq` ab 1, `kind`
`amend`|`undo`, `undo_of` genau bei `undo`, `body_before`, `body_after`, `author_name`, `created`, `uuid` mit
Fülltrigger), Grabstein-Trigger (Art `revision_amend`, Werk-uuid über revision→node→work) und Kaskade
`revision_amend_before_revision_delete` (BEFORE DELETE ON revision, damit der Grabstein das Werk noch kennt);
`work_revision_policy` (je Werk: `amend`, `max_change_percent` 1–100, `new_after_minutes` 0–1440, ohne uuid, nicht
im Austausch, geht mit dem Werk); `finding.adopted_amend`, `finding_status.adopted_amend`; `uuid_tombstone` mit der
zwanzigsten Art `revision_amend` (Umbau wie 0049, ohne `FOREIGN_KEYS_OFF`). Verlustfrei geprüft in
`crates/baruch-store/tests/migration_0050.rs` (nachgebaute Datenbank im Stand 0049; im CHECK stehen vor 0050
neunzehn Arten – der Vertrag nannte 21).

#### Fortschreiben (Runde 22)

- `baruch_core::revision_policy`: `RevisionPolicy` (Vorgabe an/20 %/30 min, `validate`), `SaveMode` (`from_param`:
  fehlt → `New`), `NewReason` (`id`, `label`), `SaveFacts`, `decide` (Reihenfolge requested → no_current →
  policy_off → not_own → not_on_current → heading_changed → large_change → time_elapsed; `None` = fortschreiben),
  `change_percent`, `word_diff` (Wort-Unterschied für den Verlauf).
- `baruch-store/src/amend.rs`: `verdict` (Tatsachen aus der DB: aktuelle Fassung, Autor, Basis, Minuten seit
  max(Anlage, letzte Fortschreibung), Einstellung), `write_step` (Text ersetzen, `amend_seq` + 1, Zeile in
  `revision_amend`, `edited` bleibt), `Store::amend_history`, `amend_undo`, `revision_policy`, `set_revision_policy`,
  `save_human_mode`/`save_human_with_reason` (RPC `revision.save`), `outline_save_mode` (→ `outline::save_auto`:
  Überschrift/Ebene/Teilen/Zusammenführen → `heading_changed`, sonst Regeln). `write.rs`: `add_proposal`/
  `add_proposal_on` setzen `based_on_amend`. `tree.rs`: `RevisionInfo.amend_seq`, `RevisionInfo.based_on_amend`,
  `Store::side_revisions` (Seitenzweige). Die alten Funktionen (`save_human`, `save_human_on`, `outline_save`) bleiben
  „neue Fassung“.
- Server: `src/revisions.rs` (RPC `revision.amends|amend_undo|policy_get|policy_set`, Seite „Verlauf“, Formulare),
  `rpc::save_mode`/`rpc::add_saved` (Antwortfelder `saved`, `amend_seq`, `reason`, `reason_label`), `outline.rs`
  (`mode`, `as_new`, Rückmeldung `saved=` in der Weiterleitung), `mirror.rs` (`describe` für Amended/AmendUndone,
  `describe_revision_policy`, Seitenzweige), `view.rs` (Stand, Verlauf-Links, „weitere Fassung(en)“, Abzeichen „+n“,
  Rückmeldung), `settings/work.rs` (Block „Fassungen“), `mcp/revisions.rs` (`revision_amends`). Oberfläche:
  `templates/fragments/editor.html` (zwei Knöpfe, `mode=auto`), `templates/revision_history.html`,
  `static/revisions.css`; `workspace.ts` übersetzt `as_new` in `mode: "new"` und zeigt die Rückmeldung,
  `outline.ts`/`wysiwyg.ts` reichen den auslösenden Knopf beim erneuten Absenden weiter.

#### Seitenzweige im Spiegel (Runde 22)

`baruch_gitmirror::Mirror::sync_side_refs(prefix, base, refs)` schreibt je `SideRef` (Name, Pfad, Inhalt, Autor,
Committer, Nachricht) einen Commit aus dem Schnappschuss `base` mit ersetzter Datei, Elternteil HEAD, und setzt die
Referenz (ohne Arbeitsordner, Index, HEAD); unveränderte (gleicher Inhalt an der Stelle, gleiche Nachricht) bleiben,
fehlende werden entfernt; Pfad- und Namensprüfung vor jedem Schreiben. `side_refs`, `read_file`, `parents` lesen nur.
Der Server (`mirror::commit_revisions`, `mirror::side_refs`) führt `refs/revisions/<key>/<n>` aus
`Store::side_revisions` nach jedem Revisions-Commit, beim Start (`sync_work`) und nach `revision.reject`
(`mirror::sync_side_refs`) nach. Fehler der Seitenzweige stehen nur auf stderr; der Commit gilt.

### befund-rueckfrage

#### Runde 22 – befund-rueckfrage

- **Rückfrage erledigter Befunde:** `assist::context::is_recheck` (Prüfen `Mode::Review`, ↻ Wiederholen
  `links.repeat_of`, RFC `OrderFrame::recheck`, nie Vorschlag) → `own_finding_lines`: eigene offene Befunde der
  Persona zum Abschnitt (Umfang `chapter`: zum Kapitel), offen, nicht angehängt, kein Lob, die jüngsten 20, gerahmt
  (`<<<befund … >>>`, also nicht im Korpus der Zitatprüfung) unter `prompts::OWN_FINDINGS_TITLE`; im System
  `prompts::RECHECK_RULE`. Beim Kürzen fallen sie zuletzt (älteste zuerst). `Prepared::own_findings` nennt die ids.
- **Zerlegung:** `findings::parse` behandelt Zeilen „Erledigt: …“ (`done_line`, nach Aufzählungs- und
  Markdown-Zeichen, auch fett) und den Satz `prompts::ALL_DONE_SENTENCE` wie Vermerke – kein Befund, Stellen der
  übrigen Befunde ohne Lücke (also `finding_spans` unverändert). `done_claims` liest Nummer („Befund n“, „Befund Nr.
  n“, „[Bn]“, „Bn“) und Stelle (erstes Zitat).
- **Statusvorschlag:** `assist::save` ruft nach `findings::ingest` unter der schon gehaltenen Schreibsperre
  `assist::propose_done` → `findings::rpc::propose_held` (neu, wie `propose` ohne eigene Sperre) →
  `Store::finding_propose_status(…, StatusChange::Done {reason}, IdeaOrigin::Persona, note)`; je Vorschlag ein Commit.
  Übergangen (fremde Persona, unbekannt, entschieden, schon offene Rückfrage) wird mit Vermerk in
  `Outcome::done_notes`.
- **Pflichtbefund:** `prompts::critical_waived(text, had_own)` – nur wenn die Persona eigene offene Befunde sah
  (`Retries::had_own`) und die Antwort den Satz sagt; dann keine Nachforderung und kein `CRITICAL_MISSING_NOTE`.
- **Stand an Befunden:** `Store` (`findings.rs`): `Target::adopted_amend` aus `revision.amend_seq` beim Auflösen von
  `Adopted`; `write_status` schreibt `finding.adopted_amend`, `finding_set_status`/`finding_propose_status` schreiben
  `finding_status.adopted_amend`, `finding_decide` beim Bestätigen den Stand zu diesem Zeitpunkt.
  `findings::adopted_words` („Fassung n · Stand k“) für Spiegel und `findings::view::status_words_amend`.
- **Oberfläche:** `findings::view::entry_json` setzt `question` (offener Vorschlag `done`); Makro `decide_forms`
  (`fragments/findings_panel.html`) und `finding_block.html` zeigen dann „Rückfrage“ mit „Ja, erledigt“ / „Nein, noch
  offen“ (Formulare an `/werke/befunde/act`, `data-finding-form`; in der Befundkarte des Chats trägt der Kasten
  `data-widget="findings"`).
- **Archiv:** `Store::restore_retired` (eine Transaktion: `work_retired` prüfen, Titel frei, Zeile löschen,
  `import::import_typst` mit Titel aus `work_retired` und Notiz `works::RESTORED_NOTE`, reservierten slug setzen).
  Server `works.rs`: `archive_folder`, `read_archive` (Pfadschutz, Zusammensetzen wie Export, `strip_back_matter`,
  `strip_projection`/`strip_first_mentions` wie `import.rs`), `archives`, `restore` (Sperre → lesen → Store →
  `rename` nach `works/<slug>` → Commit „Werk wiederhergestellt“ mit `allow_empty`), Seite `works_archive.html`.
- **Spiegel der Einfälle ohne Werk:** `mirror_ideas::sync_ideas_without_work` (Commit des Menschen, `allow_empty:
  false`, Trailer `Baruch-Action: ideas-synced`; ohne Einfälle ohne Werk und ohne Ordner nichts) aus `sync_all`
  (Start) und `works::delete` (nach dem Protokoll-Commit; Antwort `ideas_commit`).

### spiegel-entwurf

#### Entwurf als Wasserzeichen (Runde 22)

- **Modell:** `baruch_typst::typeset::Draft` (S1) als Feld `TypesetSettings.draft`; `validate`/`normalized` prüfen und
  trimmen (spiegel-entwurf). `rules` setzt dafür keine Zeile (der Entwurf wird wie `xref_with_page` herausgenommen),
  `settings_from_rules` kennt ihn nicht – Export, Export-Karte, Spiegel und Regelblock bleiben byte-gleich.
- **Regel:** `baruch_typst::typeset::draft_rule(&Draft, Option<&str>)` (`typeset/draft.rs`) liefert
  `// Baruch: Entwurf (nur in den Ausgaben)` + `#set page(foreground: …)`: Text, Zusatz und Datum nur als maskierte
  Typst-Zeichenketten (geschützte Leerzeichen gegen Umbruch), Deckkraft als Alpha von `rgb(…)`, Lage diagonal (−45°,
  Größe aus der Zeichenzahl, höchstens 80 pt) oder Kopf-/Fußzeile (7 mm vom Rand). `foreground`, weil Hintergrund,
  Kopf- und Fußzeile schon Vorlage und Satz-Regeln gehören (die Seitenzählung ab dem ersten Kapitel steht im
  Hintergrund). Das Datum ermittelt nur der Server (`typeset::draft_today`, Ortszeit) und reicht es herein.
- **Server:** `typeset::prepare_with(state, work, draft: Option<bool>)` setzt die Regel mit `cover::insert_parts` hinter
  Werkvorlage und Regelblock **vor** den Rahmen (Umschlagseiten und Titelei im Buch-PDF tragen sie also auch) und
  verschiebt die Karte – Diagnosen bleiben beim richtigen Abschnitt. `effective_draft` (dreiwertig), `cover_draft`
  (Umschlag nur mit `draft.cover` oder ausdrücklich), `draft_param`/`draft_query`/`draft_pair` für RPC und Routen.
  Stichwortzettel und Umschlag-PDF stellen die Regel vor die erste Zeile ihrer eigenen Quelle; das EPUB bekommt den
  Entwurf über `EpubInput.typeset.draft` (Datum vom Server in den Zusatz geschrieben, baruch-epub bleibt ohne Uhr).
  Druckprüfung `cover_print::cover_checks_with(…, draft)` mit Code `draft` (`CODES` jetzt 11).

#### Austausch Unterversion 8 (Runde 22)

- `baruch-exchange`: `FORMAT_VERSION = 8`; `ExchangeRevision.amend_seq/based_on_amend/amends` (`ExchangeAmend`,
  `ExchangeAmendKind`), `ExchangeFinding(Entry).adopted_amend`, Grabsteinart `Amend` („fortschreibung“ ↔
  `revision_amend`, ab 8), `typeset::draft_attrs`/`draft_from_attrs` für `<entwurf/>`; `sample::sample_document_u8`
  (das alte Beispiel bleibt ohne u8-Teile). XSD `docs/schema/baruch-werk-1-u8.xsd`, u7 eingefroren.
- Store: `exchange_dump` liest Stand, Verlauf und `adopted_amend`; `exchange_apply` setzt sie im Nachlauf `apply_u8` nur
  auf eben angelegten Zeilen, spult bekannte Fassungen vor (`fast_forward`, Regel d0) und baut für einen neuen Teil mit
  übernommenen Kapiteln einen Antrag `Add` mit `requests::PartIdentity` (uuid, Fassungen samt Verlauf). Beim Zustimmen
  ruft `requests::run` nach `outline::add_many` `exchange_apply::adopt_part`: die erzeugte Fassung 1 fällt weg (samt
  ihres Grabsteins, sie gab es nie außerhalb der Transaktion), `node.last_rev` auf 0, dann Kennung und Fassungen aus dem
  Antrag. Nur `exchange_apply.rs` schreibt uuids (Wächtertest bleibt grün).

#### Spiegel (Runde 22)

- **Show-Regel über die Sprungmarke:** Ein Knoten „ohne Nummer“, dessen Überschrift sich nicht markieren lässt (Titel
  mit Code), mit derselben Überschrift wie ein nummerierter Knoten bekommt statt der Regel über Ebene und Titel
  `#show <kN>: set heading(numbering: none)` unter `// Baruch: Überschriften ohne Nummer über die Sprungmarke`
  (`snapshot::label_rules`, vor dem Block des Rückfalls). Mit eigenem Label bleibt die Grenze. Rückweg beim Import:
  `snapshot::split_label_fallback` in `import.rs` (die genannten Überschriften kommen „ohne Nummer“ zurück).
- **Spiegel setzen:** Abschnittsdateien liegen in `abschnitte/`; relative Bildpfade `image("bilder/x")` schreibt der
  Spiegel ab der Wurzel (`image("/bilder/x")`, `snapshot::root_image_paths`), damit Typst `main.typ` samt `#include`
  setzen kann (Test `the_mirror_main_typ_with_its_includes_sets_like_the_export`: gleiche Seitenzahl wie der Export).
  Dokumentierte Abweichungen: Rasterbilder liegen im Spiegel als `bilder/<name>.b64` (Base64-Text) und müssen vor dem
  Setzen zurückgewandelt werden; ein Bildpfad, der nicht `bilder/<Name des Bilds>` ist, steht im Spiegel nicht unter
  diesem Pfad; die Teil-Seiten-Regel und der Rahmen (Umschlag, Titelei) gehören nur zum Satz, nicht zu Export und Spiegel.

## Runde 23 – Härtung

Keine Migration (Store weiter bis 0050, Bibliothek bis 0108), keine neue Unterversion des Austauschformats, keine
neuen RPC-Methoden oder MCP-Werkzeuge.

### web

**Lesende Anfragen fremder Seiten (`guard::own_reads`).** Die Schicht liegt über allen Routen und lehnt ein GET oder
HEAD mit 403 „Abgelehnt: Anfrage von einer fremden Seite (lesend)“ ab, wenn `Sec-Fetch-Site` `cross-site` oder
`same-site` ist und die Anfrage keine Navigation des obersten Fensters ist (`Sec-Fetch-Mode: navigate` mit
`Sec-Fetch-Dest: document`). Bild, Skript, `fetch` und Rahmen einer fremden Seite erreichen damit keine lesende Route
mehr (z. B. `GET /einstellungen/assistenten?modelle=abfragen`, Stichwortzettel); ein Link von außen öffnet Baruch
weiter. Ohne `Sec-Fetch-Site` (curl, MCP-Clients, alte Browser) gilt die Anfrage als eigene. Zwei Abgrenzungen:
(1) Dateien der EPUB-Vorschau (`/werke/{id}/epub/leser/…`) stehen in einem Rahmen mit `sandbox`; der Browser meldet
deren Bilder und Sprünge als `cross-site`. Dort entscheidet weiter die Route selbst über den `Referer`
(`epub_preview::from_own_frame`). (2) Fehlen `Sec-Fetch-Mode` und `Sec-Fetch-Dest` **beide**, ist die Art nicht belegt
und die Anfrage gilt nicht als Unterressource – kein Browser schickt `Sec-Fetch-Site` ohne `Sec-Fetch-Mode`; die Routen
mit eigener Prüfung (`cross_site_get`) lehnen eine solche Anfrage weiter selbst ab.

**Meldecodes.** Seiten zeigen nie Text aus der Adresse als Meldung, nur feste Codes (`?ok=<code>`, `?fehler=<code>`,
`?hinweis=<code>`), die der Server auf feste Sätze abbildet. Die letzte Ausnahme (`?ok=sicherung&ziel=<Ordner>`) ist
entfernt: Den Ordnernamen liest `settings::newest_backup` aus dem Dateisystem.

**Abzeichen der Kopfzeile.** `base.html` kennzeichnet außerhalb der Arbeitsseite die Abzeichen mit `data-badge-fill` und
die Menüleiste mit `data-badges-work="<Werk>"`. Die Schicht `consent::inject` (über allen Routen, mit `AppState`) ruft
nach der Hinweisleiste `menu::fill_page`, das die Zahlen aus `menu::badges` in das fertige HTML einsetzt
(`menu::fill_badges`) und das Kennzeichen entfernt. So braucht keine der rund dreißig Vorlagen-Umgebungen eigene Daten.

**Fortschreiben und Ändern über den Baum.** `Store::update_revision_at` (und die alte Hülle `update_revision`) legt
für die aktuelle bzw. eine schon fortgeschriebene Fassung bei geändertem Text einen Schritt in `revision_amend` an
(Art `amend`, keine Migration) und erhöht `revision.amend_seq`; `Change.amend_seq` nennt den neuen Stand, die Art der
Änderung bleibt `Updated` (Spiegel-Commit wie bisher). Damit ist die Kette der Schritte lückenlos (Vorspulen beim
Abgleich, Austausch), und ein Editor mit altem Stand bekommt `NotOnCurrent` statt still zu überschreiben. Mit
`expected_amend` lehnt der Store einen veralteten Stand ab (`StoreError::Stale`, „Veraltet: …“).

**Satzbild: Zitatblock** (auch `docs/WYSIWYG.md`). `baruch_wysiwyg::parse` liest eine logische Zeile, die genau ein
`#quote(block: true, …)[…]` ist, als eigenes Stück (wie `#szene[…]`/`#arbeit[…]`): Eine Zeile direkt dahinter ist ein
eigener Absatz, eine Zeile direkt davor beendet ihren Absatz – wie Typst setzt. `#quote[…]`, `block: false` und
unlesbare Argumente bleiben Teil ihres Absatzes. Unberührte Blöcke gehen byte-gleich zurück. Der Knopf „Zitat“ fügt
`#quote(block: true)[]` mit der Folgezeile „– “ ein, nicht `attribution: []`: Eine leere `attribution` setzt in Typst
0.15.1 eine Zeile mit dem Geviertstrich (gemessen in `crates/baruch-render/tests/quote_attribution.rs`: mit
`attribution: []` steht ein „—“ in einer eigenen Zeile mehr als 5 pt unter dem Zitat, ohne `attribution` keiner).

**Editor: Sperre während des Speicherns.** `widgets.ts` (`lockEditor`, `editorLocked`) sperrt den Bereich
`#pane-editor` (`inert`, `aria-busy`) vom Druck auf „Speichern“ bis nach dem Neuzeichnen; `outline.ts` und
`workspace.ts` zählen ihre Sperren, ein zweites Absenden während des Laufs entfällt. `workspace.ts` verwirft überholte
`ui.render`-Antworten über eine laufende Nummer (es gilt die zuletzt ausgelöste Navigation).

**Gedankenkarte.** Die Nähe (`idea_map::near::compute`) rechnet in `spawn_blocking` und höchstens für die ersten
`MAX_NEAR_IDEAS` = 1500 Einfälle nach Nummer (rund 1,1 Millionen Paare); darüber sagt es die Meldung der Karte.

**Umschlag.** `cover::compile_limited` setzt die Mess-Sätze (`book_page_mm`, `block_heights`) mit der Zeitgrenze des
Satzes (`cover::MEASURE_TIMEOUT` = 60 s); danach ist der Lauf als abgebrochen markiert und das Ergebnis verworfen,
`block_heights` meldet -32000 „… Typst wurde nach 60 Sekunden nicht fertig (Zeitgrenze).“ Die Kontrastprüfung entpackt
das Umschlagbild mit Grenzen wie die Vorschaubilder (`cover_print::MAX_CONTRAST_SIDE_PX` = 20 000, 512 MiB) und merkt
sich bis zu acht entpackte Bilder (zusammen höchstens 96 MiB RGB) nach Prüfsumme. `default_blocks` rundet Lage und Breite
auf 0,01 mm.

**EPUB.** Fußnoten bekommen ihren Platz in der Liste, bevor ihr Inhalt geschrieben ist (innere Fußnote hinter der
äußeren). `collect_labels` zählt Abbildungen (`#figure(image(…) …)`, allein oder mit Label dahinter) in Lesereihenfolge
(`Links::figures`); `@name` zeigt „Abbildung n“ (bzw. das eigene Wort des Verweises) und verlinkt auf
`<figure id="lbl-name">`. `check_mimetype` entpackt einen komprimierten `mimetype` und prüft auch dessen Inhalt
(`MIMETYPE_CONTENT` zusätzlich zu `MIMETYPE_COMPRESSED`).

**Satzmeldungen aus dem Buchende.** `ExportMap::tail_first_line`/`in_tail`: Zeilen hinter dem letzten Abschnitt
(Register, Glossar) gehören keinem Abschnitt mehr; `typeset::Typeset::diagnostics` nennt sie
„Buchende (Register, Glossar)“ mit der Zeile im Buchende.

**Befunde.** `finding_list`, `finding_attached_to` und `findings_by_uuid` holen die offenen Vorschläge in einer Abfrage
je 500 Befunde (vorher je Befund eine). `Finding::stale_summary` ist abgeleitet (offener Sammelbefund ohne offenen
Ursprung; gelöschte Ursprünge zählen als entschieden) und wird nie gespeichert. `finding_open_counts` und die Matrix
zählen offene Sammelbefunde nicht.

**Bibel und Kommentar in der Zählung des Moduls.** Die Belege aus SWORD (`bible_research::collect`) lesen die Stelle
über `bibles::read_passage` (Abbildung german() → Zählung des Moduls) wie Prüfung und `bible.lookup`; Kommentare über
`bibles::read_commentary` (dieselbe Abbildung, Beschriftung mit dem Namen der Zählung). Fehlende Verse nennt die
Wortlautprüfung in deutscher Zählung (zurück abgebildet), sonst mit dem Namen der Modulzählung.

**Menüband.** `ribbon::defined_functions` wertet nur Code der obersten Ebene aus (`code_lines`: ohne Zeilen- und – auch
geschachtelte – Blockkommentare, ohne Rümpfe anderer Funktionen, Zeichenketten bleiben ganz) und kennt
`#import "datei.typ": name, a as b`.

### mcp-namen

- **Handles** (`mcp/session.rs`): `handle_name`, `legacy_handle_name`, `LOCAL_PREFIX`, `local_name` (reine Funktionen), `Client::handle`, `Client::owned_handles`. `mcp.rs::localize` grenzt Clients ohne Token bei jeder Anfrage gegen die Tokenliste ab (eine kleine Abfrage je Anfrage).
- **Tokens** (`store/mcp_tokens.rs`): `mcp_token_create_unique(name, handle, legacy, reserved_prefix)` prüft in einer `BEGIN IMMEDIATE`-Transaktion und legt an; die Regeln reicht der Server herein (`mcp/tokens.rs::create`, einziger Weg von RPC und Formular). `mcp_token_create` (ohne Handle-Prüfung) bleibt für Tests und Bestand.
- **Eigentum** (`posts.rs`): `Actor` trägt `also` (weitere eigene Autorennamen, der Alt-Handle eines Tokens) und `owns()`; `posts::edit`, `posts::act`, `threads::delete_post` und `threads::edit_post_as` prüfen darüber.
- **Kontingent** (`store/asset_requests.rs`): `asset_mcp_usage_without_token(work, token_handles)` zählt alle Bildfassungen, deren Autor mit „@“ beginnt, und alle Anträge von Personas und MCP-Clients, ohne die Autoren der Tokens.
- **Obergrenzen im Store:** `glossary::MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN` (200), `…_PER_WORK` (2000), `requests::MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN` (50; nicht beim Einlesen einer Austauschdatei und nicht für den Menschen).
- **Relay** (`assist/chat.rs`): `relay_targets` wählt die Ziele, `ChatOrder::relay` schaltet Erwähnungen zu (MCP). `relay_section` in `mcp/tools.rs` ist entfernt. Fäden ohne Abschnitt haben weiter ihre eigene Schleife in `threads.rs` mit denselben Grenzen.
- **Auftrag eines Beitrags** (`store/post_meta.rs`): `PostRequest` + `web`, `asker`, `relay`; `assist::FramedAnswer::{to_request, from_request}`.
- **Nachbesserung, Bestand** (`mcp/session.rs`): `keeps_legacy(name, token_names)` und `token_handle` (reine Funktionen), `Client::keeps_legacy`. Ein Token, dessen neuer Handle erst durch die Regel seit Runde 23 dem Handle oder Alt-Handle eines Tokens mit anderem Alt-Handle gliche, schreibt und besitzt weiter nur unter seinem Alt-Handle. `mcp.rs::localize` bestimmt das für Tokens bei `initialize` und bei jeder Anfrage aus derselben Tokenliste (kein gespeicherter Zustand, keine Migration); die Seite „MCP-Zugang“ (`settings/access.rs`) zeigt `taken`.
- **Nachbesserung, Kontingente ohne Token:** `mcp/quota.rs::acquire(state, &Client)` belegt je Auftrag den Platz des Handles (höchstens `MAX_PER_CLIENT` = 2) und ohne Token zusätzlich einen der gemeinsamen Plätze (`WITHOUT_TOKEN`, höchstens `MAX_WITHOUT_TOKEN` = 4), beide unter einer Sperre; `Slot` gibt beide frei. `mcp/tools.rs::Counted` wählt für `add_post` und `propose_revision`, wessen Bestand zählt: `Author` (Token oder genannte Persona) oder `WithoutToken` (alle Autoren `@mcp:…` außer denen der Tokens, `token_handles`, auch vom Bildkontingent genutzt).
- **Kontext** (`assist/context.rs`): `kept_own` führt `Prepared::own_findings` nach dem Kürzen nach.
- **Varianten** (`assist/variants.rs`): `splice` ersetzt nur die Bytebereiche der betroffenen Absätze (`paragraph_spans`).
- **Befunde beim Verschieben** (`store/posts.rs::post_move`): ein `UPDATE finding` in derselben Transaktion.

### geheimnisse

- **Geheimnisse schwärzen an einer Stelle (Runde 23).** `baruch_llm::scrub(text, secret)`
  (`crates/baruch-llm/src/secret.rs`) ersetzt durch `***`: jedes wörtliche Vorkommen und – ab 6 Byte Länge des
  Geheimnisses (`SCRUB_ENCODED_MIN`) – die Prozent-Kodierung (je Byte wörtlich oder `%XX` mit großen oder kleinen
  Hex-Ziffern, Leerzeichen auch als `+`) und Base64 (Standard und URL-sicher, mit oder ohne `=`, auch als Teil eines
  längeren Base64 wie `Basic base64(user:geheim)`: gesucht werden je Versatz 0/1/2 die Zeichen, die allein vom
  Geheimnis abhängen; die gemischten Randzeichen und Füllzeichen gehen mit). Kürzere Geheimnisse nur wörtlich. Nicht
  erkannt: Hex, JSON-Escapes, doppelte Kodierung, umbrochenes Base64. Aufrufer: `Endpoint::scrub` (Modell-Client),
  `connections::check::scrub`/`scrub_value`, `connections::multigpt::scrub_text`/`scrub_error` (damit `Session::scrub`,
  Websuche, PDF-Abruf), `PgvectorWay::scrub` (über `multigpt::scrub_error`), `sync::Link::scrub` (über
  `multigpt::scrub_text` bzw. die Sitzung), `models::check_connection` (über `check::scrub_value`). Neue Abhängigkeit
  `base64 0.22` in `baruch-llm` (stand schon im Lock).
- **Keine Weiterleitungen.** Alle drei HTTP-Clients mit Schlüssel (`connections/check.rs`, `connections/multigpt.rs`,
  `baruch-llm`) haben `redirect::Policy::none()`; ein 3xx ist ein Fehler mit eigener Meldung, `Location` wird nicht
  gezeigt. Tests je Client mit zwei Nachbauten (der zweite sieht keine Anfrage).
- **Filter vor `fetch_url`.** `websearch::fetch::fetch_allowed(url) -> Result<(), &'static str>` prüft nur den Wortlaut
  der Adresse: Schema http/https, keine Benutzerangaben, kein IP-Literal in Loopback, privat, Link-Local, CGNAT,
  unspezifiziert, Multicast, Broadcast, 0/8, 192.0.0.0/24, 198.18.0.0/15, 240/4; IPv6 `::1`, `::`, `ff00::/8`, ULA `fc00::/7`,
  `fe80::/10`, `fec0::/10`, dazu IPv4 in IPv6 (`::ffff:`, `::a.b.c.d`, NAT64 `64:ff9b::/96`, 6to4 `2002::/16`) und
  das ganze `64:ff9b:1::/48`;
  `localhost`, Namen auf `.localhost`/`.local`/`.internal`/`.home.arpa` und (über den Vertrag hinaus) Namen ohne
  Punkt. Dezimale, hexadezimale, oktale und verkürzte IPv4-Schreibweisen normalisiert der URL-Parser vorher.
  **Keine DNS-Auflösung** – Namen, die auf private Adressen zeigen, erkennt nur die abrufende Seite; ob MultiGPT
  selbst filtert, ist unbekannt (**Annahme**: nein). `websearch::search::gather` ruft abgelehnte Treffer nicht ab, sie
  verbrauchen keinen der vier Abrufe und erscheinen als Hinweis. `websearch::fetch::set_fetch_exceptions(state, hosts)`
  ist ein Ausnahmeweg je Datenordner nur für Tests (kein RPC, kein Formular, keine Einstellung führt dorthin).
- **Modell-Zwischenspeicher je (Art, Adresse)** (`models.rs`: `CACHE`, `CONTEXTS`); `expire_list(url)` betrifft alle
  Arten der Adresse, `remember(url, …)` gehört zur Verbindung `eliezer`. `models::context` prüft
  `external`/`external_allowed` vor jeder Anfrage.
- **Predigt mit Bibel:** `Store::create_work_with_bible(new, human, sermon_text, bible)` schreibt `work_bible` in der
  Transaktion des Anlegens; `create_work_with_text` ruft sie mit `None`.
- **EPUBCheck-Temp-Ordner** (`epub_check::TempDir`): `baruch-epubcheck-<32 Hex-Zeichen aus getrandom>`, unter Unix
  `DirBuilder::mode(0o700)`, ohne `recursive` – ein vorhandener Ordner, eine Datei oder ein Symlink gleichen Namens
  ist ein Fehler („Temp-Ordner für EPUBCheck: …“) und wird weder benutzt noch aufgeräumt.
- **SSE-Parser** (`baruch-llm/src/sse.rs`): Die Grenze `max_line` gilt auch für die Summe der `data:`-Zeilen eines
  Ereignisses („Ein Ereignis im Antwortstrom ist zu groß; abgebrochen.“).

### Integration

- **Router:** `guard::own_reads` liegt als Schicht über allen Routen (unmittelbar vor `guard::local_host`, Schnittstelle
  R23-S1b). `/mcp` hat die eigene Größengrenze `MCP_BODY_LIMIT` (12 MiB). `/act` kennt `based_on_amend`.
- **Datenordner:** `baruch_store::Store::open` legt einen neuen Datenordner unter Unix mit 0700 an (`create_data_dir`,
  Codereview quellen-5); ein vorhandener bleibt unverändert.
- **Abnahme:** `tests/round23_abnahme.rs` (Zitat bis Tokens in einem Ablauf; Geheimnisse, Weiterleitung, Websuche).

## 0.9.0 – Ältere Fassungen ausblenden (Nutzer 2026-11-24)

- **Migration 0051** (`0051_revision_fold.sql`): Tabelle `node_revision_fold` (`node_id` als Primärschlüssel mit
  `ON DELETE CASCADE`, `below` ≥ 1 = Nummer der Fassung, an der die Marke steht, `updated`, `updated_by`). Ohne uuid,
  nicht im Austauschformat (Arbeitsweise der Instanz wie `work_revision_policy`), kein Commit im Spiegel (kein
  Buchtext). Verlustfrei geprüft in `crates/baruch-store/tests/revision_fold.rs` (Datenbank im Stand ed9d210, Store
  bis 0050).
- **Regel** `baruch_core::revision_fold`: `cutoff` = Marke, höchstens die aktuelle Fassung; ausgeblendet sind ersetzte
  und verworfene Fassungen unter `cutoff` (`is_hidden`, `hidden_count`); nie die aktuelle, nie offene Vorschläge.
  `offered` = „bis hierhin ausblenden“ nur an der aktuellen Fassung und darunter, wenn darunter etwas auszublenden ist.
- **Store** `baruch-store/src/revision_fold.rs`: `revision_fold`, `work_revision_folds` (Schlüssel → Marke),
  `set_revision_fold` (über der aktuellen `Refused`), `clear_revision_fold`, `clear_work_revision_folds`,
  `fold_revisions_before` (werkweit, letzte Fassung vor einem UTC-Zeitpunkt, höchstens die aktuelle).
- **Server** `src/revision_fold.rs`: RPC `revision.fold_get|fold_set|fold_clear|fold_before|fold_clear_all` (kein
  MCP-Werkzeug), Formular `POST /einstellungen/werk/fassungen/ausblenden`; `/act` nimmt `fold_set`/`fold_clear`.
  `view.rs`: `ViewExtras.revision_folds`, `Location.fassungen` (`alle` = Schalter offen, `Resolved.show_all`,
  `FOLD_OPEN_Q` in `Resolved::url`), `FoldView` (Zähler und Nachbarn über die sichtbaren Fassungen, Links für den
  Schalter, Fassungen mit Angebot), `TreeRevision.hidden`/`fold`. Vorlagen: `fragments/editor.html` (Zusatz am Zähler,
  „bis hier ausblenden“), `fragments/revisions.html` (Formulare `rev-fold-<n>`, „Wieder alle zeigen“, Schalter
  `<details class="rev-fold">`), `fragments/revtree.html` (Makro `folded`), `settings/werk.html`; CSS in
  `static/revisions.css`; `web/src/rpc.ts` reicht `fassungen` an `ui.render` durch.

## Runde 33 – Menüband

Die Kopfzeile mit Werk ▾ / Ansicht ▾ / Werkzeuge ▾ und die flache Knopfreihe über dem Blatt sind durch ein Menüband
ersetzt: ein Menü **Baruch ▾** (Programm), Reiter **Start · Einfügen · Prüfen · Buch · Ansicht** mit Gruppen, und die
Knöpfe des Editors docken in das Band an. Keine neue Route, keine neue RPC-Methode, kein neues MCP-Werkzeug;
`ui.render`, `menu.badges` und `ribbon.formats` sind unverändert. Der Laptop-Modus (kompaktes Band, Seitenleisten)
gehört zu `src/laptop.rs`/`web/src/laptop.ts` und ist dort beschrieben.

### Vorlage (`templates/base.html`)

Das Band steht vollständig in `base.html` (die Vorlage ist in rund 35 eigenen minijinja-Umgebungen eingebettet; ein
`{% include %}` müsste in allen registriert werden).

- `<header class="band" data-widget="band" data-tab="" data-work="W">` (die Klasse `topbar` gibt es nicht mehr),
  direkt darunter die Marke `<!--baruch-consent-->`.
- Zeile 1 (`div.band-top`): `nav.menubar[data-widget=menubar]` mit genau einem `details.menu[data-menu="baruch"]`
  (Alle Werke, Neues Werk / Importieren…, PDF oder EPUB importieren…, Austausch: Einlesen…, ⚙ Einstellungen, Über
  Baruch), die Reiterzeile `nav.band-tabs[role=tablist][data-band-default]`, die Werkwahl (`form.work-select`, nur
  Arbeitsseite), `span.work-title`, der Autor und der Schalter `a[data-band-toggle]` (Band auf/zu). Die Reiterzeile
  steht im Dokument gleich hinter dem Menü (nicht hinter dem Titel): So bleibt sie auf jeder Seite an derselben
  Stelle, und die Tab-Folge entspricht dem Bild.
- Zeile 2 (`div.band-panels`): je Reiter ein `div.band-panel[role=tabpanel]` und – nur auf der Arbeitsseite – der
  Andockplatz `div.band-dock[data-band-dock]`.
- Reiter je Seite: Arbeitsseite (`topbar` gesetzt) alle fünf; Seiten mit Werk Start · Prüfen · Buch · Ansicht; ohne
  Werk Start · Ansicht (dort stehen „Literatur“ und „Abgleichen“ in einer Gruppe „Werkzeuge“ unter Start, weil sie
  kein Werk brauchen; „Werk wählen“ führt wie bisher auf `/werke`).
- Blöcke der Seiten: `band_tab` (Vorgabe-Reiter, sonst `start`), `page_actions` (Gruppen im Reiter Start, zwischen
  „Werk“ bzw. „Ansicht des Textes“ und „Einfälle“), `band_einfuegen`, `band_pruefen`, `band_buch`, `band_ansicht`
  (jeweils vor den festen Gruppen des Reiters).
- Plätze für Runde 32 als Kommentare der Vorlage: `{#- R32-PLATZ import-word -#}` und `{#- R32-PLATZ beenden -#}`
  (letzter Eintrag) im Menü Baruch ▾, `{#- R32-PLATZ export-word -#}` in der Gruppe Ausgabe.

**Wörtliche Formen.** Die Schicht ersetzt Zeichenketten; die Reihenfolge der Attribute ist darum Teil der
Schnittstelle (der Unit-Test `the_template_carries_the_literal_forms_apply_relies_on` hält sie fest):

| Stück | Form |
|---|---|
| Reiter | `<a class="band-tab" role="tab" id="band-tab-ID" aria-controls="band-panel-ID" data-band-tab="ID" href="?tab=ID" aria-selected="false">` |
| Feld | `<div class="band-panel" role="tabpanel" id="band-panel-ID" aria-labelledby="band-tab-ID" data-band-panel="ID" hidden>` |
| Gruppe | `<details class="band-group" data-band-group open><summary class="band-group-label">Name</summary><div class="band-items" role="group" aria-label="Name">…</div></details>` – weitere Attribute nur zwischen `class="…"` und `data-band-group` (z. B. `data-band-after`: hinter dem Andockplatz) |
| Band auf/zu | `data-band-toggle aria-expanded="true" href="?band=closed"` |
| Laptop-Schalter | `<a class="btn" data-laptop-set="auto" aria-pressed="false" href="?laptop=auto">Automatisch</a>` (ebenso `on`, `off`) |
| Seitenleisten | `<a class="btn" data-rail-toggle="left" aria-pressed="false" href="?lrail=1">Gliederung</a>`, rechts `right`/`rrail` |

### Schicht `band::apply` (`src/band.rs`)

Seiten kennen ihre Adresse und `?tab=` nicht. Die Schicht `consent::inject` (über allen Routen, wie die Abzeichen
seit Runde 23a) hält vor `next.run` Cookie-Kopf und rohe Abfrage fest, bildet `BandState::from_request` und
`LaptopState::from_request` und ruft nach `menu::fill_page` `band::apply(&page, &band, &laptop)`; gültige Schlüssel
der Adresse hängt sie als `Set-Cookie` an (`band::set_cookies`, `laptop::set_cookies`).

- `BandState { tab, closed, rest }`: `?tab=` nur für Kennungen aus `TABS` (nicht gemerkt), `?band=open|closed` vor
  dem Cookie `baruch_band` (`closed`; fehlt = offen), `rest` = rohe Abfrage ohne `tab`, `band`, `laptop`, `lrail`,
  `rrail` (nicht dekodiert). Ungültige Werte werden übergangen; bei mehrfachen Schlüsseln gilt der letzte gültige.
- `apply`, in dieser Reihenfolge; eine Seite ohne die Marke ` data-band-fill` bleibt unverändert:
  1. Marke an `<html>` → `laptop.html_attrs()` und `data-band="open|closed"`;
  2. aktiver Reiter: `?tab=`, falls der Kopf ihn hat; sonst `data-band-default`, falls vorhanden; sonst der erste;
  3. `data-tab=""` am Kopf → `data-tab="<aktiv>"`;
  4. Adressen der Reiter `href="?<rest>&tab=ID"`; der aktive zusätzlich `aria-selected="true"` und `&band=closed`
     (Klick klappt zu); bei zugeklapptem Band tragen alle Reiter `&band=open`;
  5. `hidden` fällt am aktiven Feld weg;
  6. kompakt (`laptop.compact()`): jedes ` data-band-group open>` → ` data-band-group name="band-group">` (Popups
     aus `<details>`, der gemeinsame Name hält nur eines offen);
  7. jedes `href="?K=V"` mit K ∈ {band, laptop, lrail, rrail} → `href="?<rest>&tab=<aktiv>&K=V"`;
  8. `aria-pressed` am geltenden Laptop-Schalter; Seitenleiste sichtbar → `aria-pressed="true"`, eingeklappt →
     Adresse auf `lrail=0` bzw. `rrail=0`; zugeklapptes Band → Schalter auf `band=open`, `aria-expanded="false"`.
- `node=` (alte Adressform) geht nicht in die erzeugten Adressen mit: Erzeugte Links tragen nie `node=` (Vertrag
  Runde 8 §1). `rest` selbst bleibt die rohe Abfrage; eine Seite, die noch mit `?node=` aufgerufen wurde, verliert
  beim Reiterwechsel ohne JavaScript nur diese Stelle. Alle anderen Schlüssel bleiben – auch einmalige wie
  `?ok=gespeichert` (die Meldung steht dann nach einem Reiterwechsel ohne JavaScript noch einmal da).
- **Sicherheit:** `rest` kommt aus der Adresse und geht nur über `escape_attr` (`&`, `<`, `>`, `"`, `'`) in ein
  Attribut. Die Schritte 2 bis 6 wirken nur zwischen `<header class="band"` und `</header>`: Text im Seiteninhalt, der
  wie ein Reiter aussieht, verstellt nichts. Die Schritte 7 und 8 gelten auf der ganzen Seite (auch für die schmalen
  Leisten der Arbeitsseite); ein Wert V mit anderen Zeichen als `[A-Za-z0-9_-]` wird nicht angefasst.
- **Bekannte Grenze:** Auf einer Seite, die Antwort eines POST ist (z. B. die Rückfrage „Speichern bestätigen“),
  führt ein Reiter-Link ohne JavaScript per GET auf dieselbe Adresse; mit JavaScript wechselt band.ts ohne Neuladen.

### Abzeichen

Die Abzeichen stehen an den Reitern: offene Vorschläge an **Start**, „N offen“ (Befunde) an **Prüfen**, offene
Anträge zur Gliederung an **Buch**; dazu die Zahl an den Einträgen „Befunde“ und „Anträge zur Gliederung“. Die Haken
sind die alten (`data-badge`, `data-badge-item`, `data-badge-fill`, `data-badges-work`): Die Arbeitsseite zeichnet aus
`topbar.badges`, jede andere Seite füllt `menu::fill_badges` im fertigen HTML (ohne JavaScript), `menu.ts` frischt auf
(„N offen“ am Reiter, nur die Zahl im Eintrag).

### Andocken der Editor-Knöpfe

Die Editor-Widgets hängen an `div.wysiwyg`, und `ui.render` zeichnet nur die drei Spalten neu. Darum bleiben die
Knöpfe im Fragment `fragments/edit_toolbar.html` (Editor-Zweig, `div[data-edit-toolbar][hidden]` in `div.wysiwyg`) und
werden mit dem Editor neu gezeichnet; `ui.render` ist unverändert.

- Die Leiste hat zwei Teile: `div[data-band-part="start"]` (Gruppen Text: Satzbild | Quelltext; Absatz:
  Absatzformat, K, F; Fassung: ‹ n/m ›, Speichern, Als neue Fassung – `form="editor-form"`; Assistenten: Prüfen,
  Vorschlag) und `div[data-band-part="einfuegen"]` (Einfügen: Formate aus `ribbon.formats`; Bild; Register und
  Glossar). Alle Haken sind die bisherigen (`data-ws-mode`, `data-ws-tools`, `data-ws-format`, `data-ws-cmd`,
  `data-ribbon-cmd`, `data-ribbon-snippet`, `data-ribbon-image`, `data-widget="ribbon"`,
  `data-widget="context-menu"`, `data-ctx-action`, `data-index-*`). Die ‹ ›-Links im Band tragen kein `data-key`
  (Alt+←/→ nimmt weiter die Links im Kopf des Editors).
- `fragments/editor.html` endet in beiden Ansichten mit `<span data-widget="band-dock" hidden></span>`. Das Widget
  `band-dock` (band.ts) leert beim Einhängen `div[data-band-dock]` und hängt die Leiste des gezeigten Editors hinein
  (Vorschau: nur leeren); der Kopf trägt dann `data-docked`. Weil `mount` die Widgets in Dokumentreihenfolge
  einhängt, haben sich `wysiwyg`, `ribbon` und `context-menu` vorher mit ihrem Editor verbunden.
- Welcher Teil sichtbar ist, regelt `band.css` über `header.band[data-tab]`. Die Felder zeichnen keinen Kasten
  (`display: contents`): Gruppen und Andockplatz stehen in einer Zeile, der Andockplatz zwischen „Ansicht des
  Textes“ und „Einfälle“ (`order`).
- Nachgezogen in den Widgets: `widgets.ts` – `editToolbar(root)` findet die Leiste am Blatt oder im Andockplatz
  (wysiwyg.ts nutzt es für den Umschalter Satzbild/Quelltext), `submitButton` überspringt die Knöpfe der Leiste
  (maßgeblich bleibt „Speichern“ der Statuszeile), `lockEditor` sperrt auch den Andockplatz; `workspace.ts` lauscht
  für `a[data-nav]` am Dokument (Umschalter und ‹ › stehen im Kopf); `sheet.ts` merkt die Cursorstelle für „Bild“
  über einen Lauscher am Dokument; `ribbon.ts` lässt die Pfeiltasten nur über erreichbare Knöpfe wandern.
- „Prüfen“/„Vorschlag“ (`[data-band-assist]`) lösen den gleichnamigen Knopf von `form.assist-form` rechts aus;
  „Speichern“ im Band speichert nicht, solange der Knopf der Statuszeile gesperrt ist (nichts geändert).
- Ohne JavaScript bleibt die Leiste verborgen am Blatt (wie bisher: Textfeld und Statuszeile); der Reiter Einfügen
  zeigt dann einen Hinweis (`nojs-only`) mit den Links „Bild einfügen …“ (`#bild-einfuegen`) und „Markierung …“.

### Widgets `band` und `band-dock` (`web/src/band.ts`)

- Reiter nach dem ARIA-Muster für Tabs: Klick wechselt ohne Neuladen, ←/→ (mit Umlauf) und Pos1/Ende wandern und
  wählen, nur der gewählte Reiter steht in der Tab-Folge.
- Klick auf den gewählten Reiter oder auf den Schalter klappt das Band (`data-band` an `<html>`, Cookie
  `baruch_band`); zugeklappt zeigt ein Reiter sein Feld vorübergehend über dem Inhalt (`data-band-peek` am Kopf) –
  Esc, ein Klick daneben oder ein ausgelöster Eintrag blendet es aus.
- Gruppen: voll alle offen, die Beschriftung ist kein Schalter; kompakt (`data-compact` an `<html>`) Popups – nur
  eines offen, Esc und Klick daneben schließen. band.ts misst nie Breiten; es reagiert auf das Ereignis
  `baruch:laptop` (laptop.ts) mit `syncGroups`.
- Stil `static/band.css`: nur die Attribute an `<html>` (`data-compact`, `data-band`), keine Media-Queries; nur
  Farbvariablen aus `app.css`; Hilfsklassen `js-only`/`nojs-only`.

Tests: Unit-Tests in `src/band.rs`; `tests/round33_menueband.rs` (Band je Seitenart, `?tab=`, `?band=` mit Cookie,
Maskierung, Zuordnung aller alten Einträge, Abzeichen, Fragment des Editors, Verhalten der Widgets im Node-Nachbau,
Szene „band“ in `tests/round23_web/widgets.mjs`).

### Laptop-Modus (Runde 33)

Eine Quelle der Wahrheit: **Attribute an `<html>`**. Der Server schreibt sie beim Ausliefern, das Widget hält sie
im Browser aktuell, CSS und andere Widgets lesen nur sie. Es gibt für die Modi **keine Media-Query auf die
Fensterbreite** – sonst gäbe es zwei Wahrheiten, sobald der Mensch „An“ oder „Aus“ wählt.

| Attribut | Bedeutung | gesetzt, wenn |
|---|---|---|
| `data-laptop="auto\|on\|off"` | Wahl des Menschen | immer |
| `data-compact` | Band kompakt | `on`, oder `auto` und Breite `m`/`s` |
| `data-narrow` | schmal: beide Seitenspalten sind Leisten | `on`, oder `auto` und Breite `s` |
| `data-rail-left` | Gliederung eingeklappt | schmal, oder von Hand (`lrail`) |
| `data-rail-right` | rechte Spalte eingeklappt | schmal, oder von Hand (`rrail`) |
| `data-drawer="left\|right"` | Schublade offen (nur laptop.ts, nie vom Server) | solange die Schublade offen ist |

#### Server: `src/laptop.rs`

- `LaptopState::from_request(cookie_header, query)` liest den Zustand: Schlüssel der Adresse `laptop`
  (`auto|on|off`), `lrail`, `rrail` (`0|1`) gelten **vor** den Cookies `baruch_laptop` (`on|off`, fehlt =
  automatisch), `baruch_lrail`, `baruch_rrail` (`1`, fehlt = sichtbar). Die Breitenklasse kommt nur aus dem Cookie
  `baruch_vw` (`w|m|s`, fehlt = breit). Ungültige Werte werden übergangen; steht ein Schlüssel mehrfach in der
  Adresse, gilt der letzte gültige (das Band hängt seine Schlüssel hinten an).
- `compact()`, `narrow()`, `rail_left()`, `rail_right()` sind die Formel aus der Tabelle; `html_attrs()` schreibt
  sie in fester Reihenfolge und **nur aus festen Zeichenketten** – nichts aus Adresse oder Cookie gelangt in die
  Seite.
- `set_cookies(query)` liefert fertige `Set-Cookie`-Werte für gültige Schlüssel der Adresse
  (`…; Path=/; Max-Age=31536000; SameSite=Lax`); die Vorgaben (`laptop=auto`, `lrail=0`, `rrail=0`) löschen das
  Cookie (`Max-Age=0`).
- Schwellen: `COMPACT_BELOW_PX = 1600`, `NARROW_BELOW_PX = 1280`; `Width::from_px` ordnet zu.
- Verdrahtung: `consent::inject` (Schicht über allen HTML-Antworten) bildet den Zustand vor `next.run`,
  `band::apply` setzt `html_attrs()` an der Marke ` data-band-fill` ein, die Cookies werden an die Antwort
  **angehängt** (`/` setzt schon `baruch_work`). Keine neue Route, keine RPC-Methode, keine Migration.

#### Browser: `web/src/laptop.ts`

- Hängt sich beim Laden selbst am `document` ein (Seiten ohne Arbeitsfläche haben kein Element dafür); das Widget
  `laptop` bleibt registriert und löst dasselbe aus.
- Misst `window.innerWidth` beim Laden und bei `resize` (gebündelt über `requestAnimationFrame`), schreibt die
  Klasse ins Cookie `baruch_vw` und setzt die Attribute nach derselben Formel (`effective`, `htmlAttrs`). Die
  Schwellen stehen als `COMPACT_BELOW_PX`/`NARROW_BELOW_PX` auch im Skript; `tests/round33_laptop.rs` vergleicht
  sie und alle 36 Kombinationen der Formel mit der Rust-Seite.
- **Ereignis `baruch:laptop`** am `document`, `detail: {compact, narrow}`: einmal beim Laden, danach bei jedem
  Wechsel von `compact` oder `narrow`. band.ts klappt danach seine Gruppen. Das Ereignis beim Laden geht hinaus,
  bevor Widgets eingehängt sind – wer es braucht, liest beim Einhängen zusätzlich die Attribute an `<html>`.
  laptop.ts fasst die Gruppen des Bands nie an; band.ts misst nie Breiten.
- Klicks (ein Hörer am `document`): `[data-laptop-set]` (Modus, Cookie `baruch_laptop`), `[data-rail-toggle]`
  (Schalter im Band), `[data-rail-open]` (Leiste), `[data-rail-collapse]` („Einklappen“ in der Spalte),
  `[data-rail-close]` („Schließen“, Fläche neben der Schublade). Das Widget pflegt `aria-pressed` der Schalter
  (Leiste sichtbar = gedrückt) und die Adresse des Rückfall-Links (`lrail=0|1`). Gibt es die Spalte auf der Seite
  nicht, bleibt der Link ein gewöhnlicher Link.
- Schublade: im schmalen Zustand `data-drawer` an `<html>`; Fokus in die Spalte, Esc und „Schließen“ geben ihn
  dem Auslöser zurück, ein Klick daneben schließt ohne Fokuswechsel, die Wahl eines Abschnitts (`a.tree-entry`)
  schließt die Gliederung. Nicht schmal klappen Leiste und Schalter die Spalte dauerhaft (Cookies `baruch_lrail`,
  `baruch_rrail`).
- `web/src/zoom.ts`: `[data-zoom-reset]` setzt alle Zonen zurück und löscht `baruch_zoom`.

#### Markup und Stil

- `templates/workspace.html`: zwischen den Spalten je eine Leiste `a.rail[data-rail-open]` (`href="#pane-tree"`
  bzw. `#pane-assistants`) und die Fläche `a.rail-scrim` (`href="#pane-editor"`). Die Spalten bleiben erstes und
  letztes Kind des Rasters, weil das Widget `splitter` sie so misst. `id="pane-tree"`, `id="pane-editor"`,
  `id="pane-assistants"`, die Splitter und alle `data-zoom-zone` sind unverändert.
- `templates/fragments/tree.html`, `assistants.html`: oben die Zeile `div.rail-bar` mit „Einklappen“
  (`?lrail=1` bzw. `?rrail=1`) und „Schließen“ (`#pane-editor`). Sie steht in den Fragmenten, weil `ui.render`
  den Inhalt der Spalten ersetzt.
- `static/app.css`: Grundgerüst (Raster, Leisten im Ruhezustand verborgen). Die Regeln der alten Kopfzeile
  (`.topbar`) und die beiden Breiten-Media-Queries (Kopfzeile unter 1000 px, Spalten untereinander unter 900 px)
  sind entfallen.
- `static/laptop.css` (in `STYLE` zuletzt): alles an `data-compact`, `data-narrow`, `data-rail-*` außerhalb des
  Bands. Ohne JavaScript öffnet `:target` die Schublade (`html:not(.js)…:target`); mit JavaScript gelten diese
  Regeln nicht, damit eine Adresse mit `#pane-tree` die Schublade nicht offen hält. Die Fläche daneben braucht
  ohne JavaScript `:has()`; fehlt das im Browser, bleibt „Schließen“.

#### Grenzen

- Ohne JavaScript fehlt `baruch_vw`: „Automatisch“ zeichnet breit, der Mensch stellt „An“.
- Beim allerersten Aufruf mit JavaScript kennt der Server die Breite ebenfalls noch nicht; die Seite kommt breit
  und wird vom Widget sofort umgestellt (ein kurzer Sprung, danach stimmt das Cookie).
- Das Cookie `baruch_vw` gilt je Browser, nicht je Fenster: Zwei verschieden breite Fenster überschreiben es
  gegenseitig; das Widget stellt jedes Fenster nach dem Laden richtig.

## Runde 32 – Word und Beenden

Neues Crate `baruch-docx` (rein, ohne Datenbank und Netz; `zip` 9 nur mit `deflate`, `quick-xml` 0.42) mit dem
gemeinsamen Modell (`model.rs`), Fehlern, Grenzen (`limits.rs`) und den festgelegten Namen von OOXML (`ooxml.rs`).
Leser (`read`, `resolve`, `to_typst`) und Schreiber (`from_typst`, `diff`, `write`) sind dateigetrennt; beide
kennen nur das Modell. Im Server stehen `docx_import/`, `docx_export/` und `shutdown.rs`. Keine Migration.

### Runde 32 – Word-Import (docx_import)

**Crate `baruch-docx`, Leserseite** (rein, ohne Datenbank und Netz):

- `read::read(bytes, &Limits) -> ReadDocx { document, warnings }` – `read/pack.rs` öffnet das ZIP (Größe der Datei,
  Zahl der Einträge, erklärte Größen vor dem Entpacken; jeder Eintrag über `take(Grenze + 1)`, die Summe des wirklich
  Entpackten zählt mit; Namen mit `..`, führendem `/`, Rückstrich oder Laufwerk → `Unsafe`; OLE-Anfang → `NotDocx`
  „altes .doc oder verschlüsselt“) und löst Beziehungen auf (Ziel außerhalb des Archivs ohne `TargetMode="External"`
  → `Unsafe`; externe Ziele werden nie abgerufen, ein nur verknüpftes Bild ergibt `docx_bild_extern`).
  `read/xml.rs` baut je Teil einen kleinen Baum mit quick-xml: DTD → `Unsafe`, nur die fünf festen Entitäten und
  Zeichenverweise werden aufgelöst, Tiefe und Elementzahl begrenzt. Elemente heißen wie in der Datei (`w:p`) –
  Annahme: die üblichen Präfixe. `read/body.rs` bildet den Textkörper auf das Modell ab: Rolle der Absatzvorlage
  (ID `Heading N`, Name „heading N“/„Überschrift N“, `basedOn`, `w:outlineLvl` in Vorlage oder Absatz, die
  Baruch-Vorlagen aus `ooxml.rs`), Läufe mit Zeichenformaten, `w:ins`/`w:del`/`w:moveFrom`/`w:moveTo` als
  `Inline::Tracked`, Änderung der Absatzmarke als `mark`, Fußnoten/Endnoten inline, Kommentarbereiche, Links,
  Bilder (DrawingML und VML; Medientyp aus den ersten Bytes), Listen über `numbering.xml`, Tabellen, Typst-Inseln
  (`w:sdt` mit `w:tag="baruch-typst"`: der mit `w:vanish` verborgene Text ist der Quelltext, Absätze sind Zeilen).
  Warnungen je Code einmal mit der Zahl der Stellen.
- `resolve::resolve(document, View)` löst die Änderungsverfolgung auf (`Original`, `Final`, `FinalBy(name)`); eine
  nicht angewandte eingefügte Absatzmarke lässt den Absatz im folgenden aufgehen, eine angewandte gelöschte lässt ihn
  den folgenden aufnehmen. `change_authors` nennt die Bearbeiter in der Reihenfolge des ersten Auftretens.
  `proposal_view(document, name)` ist `FinalBy` mit den Überschriften des Originals und liefert `HeadingChange`
  (`before` leer = eingefügt, `after` leer = gelöscht; `section` zählt wie `to_typst` am Original, ein Vorspann vor
  der ersten Überschrift ist Abschnitt 0).
- `to_typst::to_typst(document) -> TypstOut { sections, warnings }` schreibt je Abschnitt Typst in den Formen des
  Markdown-Imports (Text maskiert, Auszeichnung als Funktion, `#quote(block: true, attribution: […])[…]`, Listen mit
  `-`/`+`, `#table(…)`, `#figure(image("…", alt: "…"), caption: […])`) und sammelt je Abschnitt die Kommentar-Anker
  (kommentierter Wortlaut). Text vor der ersten Überschrift wird ein Abschnitt mit leerem Titel.

**Server `docx_import`** (`src/docx_import/mod.rs`, `draft.rs`, `pages.rs`, Vorlage `docx_import.html`):

- `prepare(bytes) -> Prepared` (rein, in `spawn_blocking`): liest, schreibt das Original als Abschnitte, legt je
  Bearbeiter den Vorschlagsstand auf die Abschnitte des Originals (nur abweichende Abschnitte zählen), ordnet
  Kommentare ihren Abschnitten zu (zuerst am Original, sonst am Stand eines Bearbeiters) und sammelt die Warnungen.
- `preview` legt den Entwurf ab (`draft.rs`, gehalten wie `pdf_import/draft.rs`: 32 Hex, 20 Stück, 24 Stunden);
  `apply` liest die abgelegte Datei neu und ruft `apply_prepared`:
  1. Werk über `import::import_generated` (ein Commit „Import aus <datei>“, Bilder als `ImportAsset`),
  2. unter der Schreibsperre je Bearbeiter und Abschnitt `Store::add_proposal(…, "@word:<name>", "word", Notiz)` –
     der Text ist die aktuelle Fassung mit ersetztem Abschnittstext, die Überschriftszeile bleibt wörtlich –, danach
     `mirror::side_refs_after_proposal` (Seitenzweige `refs/revisions/<key>/<n>` mit Persona und Modell),
  3. Beiträge über `Store::add_post` (geänderte Überschriften, Kommentare mit `TextRef` auf Fassung 1, Antworten
     direkt hinter ihrem Kommentar).
  Nie `set_current`: Fassung 1 bleibt die aktuelle. Ein Fehler in 2 oder 3 wird als Warnung gemeldet, das Werk
  bleibt.
- `cli_import` teilt `prepare`/`apply_prepared`; ohne `--uebernehmen` wird nichts geschrieben.

**Annahmen (ohne echtes Word geprüft):** Die Tests bauen ihre Word-Dateien selbst nach ECMA-376 und `ooxml.rs`; eine
von Microsoft Word oder LibreOffice geschriebene Datei wurde nicht gelesen. Angenommen ist insbesondere: Präfixe wie
oben; Antworten auf Kommentare über `commentsExtended.xml` (`w15:paraIdParent` ↔ `w14:paraId` des Kommentarabsatzes);
die Träger der Typst-Insel wie in `ooxml.rs` beschrieben (mehrzeiliger Quelltext als mehrere verborgene Absätze oder
mit `w:br`).

### Word-Export (`baruch-docx` Schreiber, `baruch-server/src/docx_export`)

Der Weg vom Werk zur Datei hat drei reine Stufen im Crate `baruch-docx` und eine lesende im Server:

1. **`from_typst(body, image_paths)`** (`from_typst.rs`, `from_typst/args.rs`): liest einen Abschnitt mit
   `baruch_wysiwyg::parse` und bildet die Blöcke auf das Modell ab. Abbildungen (`#figure(image(…))`, `#image(…)`)
   werden `Block::Figure` mit wörtlichem `raw`, einfache Tabellen `Block::Table`, `#pagebreak()` `Block::PageBreak`.
   Alles andere wird `Block::Typst`/`Inline::Typst` mit wörtlichem Quelltext und sichtbarem Ersatz (Warnung
   `docx_typst_insel`); Unsichtbares (Label, Registermarke, Kommentar) geht ohne Warnung als Insel ohne Sichtbares
   mit, Querverweise mit ihrem Text.
2. **`track_changes(current, proposed, change)`** (`diff.rs`): richtet die Blöcke über die längste gemeinsame
   Teilfolge aus, ordnet in jeder Lücke ähnliche Blöcke gleicher Art einander zu (Anteil gemeinsamer Wörter) und
   vergleicht sie wortweise; übrige Blöcke sind ganz neu oder entfallen ganz (`mark` an der Absatzmarke, Inhalt in
   `Inline::Tracked`). Zusage: Einfügungen behalten und Löschungen entfernt = Vorschlag, umgekehrt = aktuelle
   Fassung (Ausnahmen siehe Kopf von `diff.rs`: leeres Tabellengitter, Seitenumbrüche).
3. **`write(document, options)`** (`write.rs`, `write/body.rs`, `write/styles.rs`, `write/image.rs`): schreibt das
   Archiv in fester Reihenfolge mit festen ZIP-Zeiten (deterministisch): `[Content_Types].xml`, `_rels/.rels`,
   `docProps/app.xml`, `docProps/core.xml`, `word/document.xml` samt Beziehungen, `styles.xml`, `settings.xml`,
   `numbering.xml`, `footnotes.xml` (bei Verweisen in Fußnoten mit eigenen Beziehungen), `comments.xml` (nur mit
   Kommentaren), Bilder unter `word/media/imageN.<endung>`. XML-verbotene Zeichen werden durch U+FFFD ersetzt.
4. **`docx_export::build`** (Server): liest Werkangaben, alle Fassungen (`Store::load_work`, Zeitpunkte aus
   `Store::revision_tree`), Bilder (`Store::export_assets`, nur PNG/JPEG/GIF) und – mit Vorschlägen – die Marken
   „bis hierhin“ (`Store::work_revision_folds`). Offen ist ein Vorschlag mit Status „vorgeschlagen“, der nicht nach
   `baruch_core::revision_fold::is_hidden` ausgeblendet ist. Es gibt nur lesende Zugriffe; das Packen läuft in
   `spawn_blocking`.

Festlegungen des Schreibers über `ooxml.rs` hinaus (für den Leser und die Rundreise):

- Der Pfad eines Bildes im Werk steht in `wp:docPr name`, der Alternativtext in `descr`.
- Eine Abbildung mit `raw` steht in einem Inhaltssteuerelement `baruch-typst` (verborgener Quelltext, dann Bild und
  Unterschrift) – für den Leser also eine Insel mit `raw`, was beim Zurücklesen den wörtlichen Quelltext ergibt.
- Geordnete Listen: die erste mit Beginn 1 benutzt `NUM_DECIMAL`, jede weitere eine eigene `w:num` (ab 3) auf
  derselben abstrakten Nummerierung mit `w:startOverride` – sonst zählte Word über alle Listen des Buchs weiter.
- Verfolgte Änderungen stehen um jeden Lauf einzeln (`w:ins`/`w:del`), eine verfolgte Insel als
  `w:ins`/`w:del` um das `w:sdt`. Die Absatzmarke der Zitatquelle und der Bildunterschrift gilt als verfolgt, wenn
  ihr ganzer Inhalt eine Änderung derselben Art ist.
- Fußnotentext beginnt wie in Word mit einem Leerzeichen hinter dem Fußnotenzeichen.
- Untertitel steht zusätzlich in `dc:subject`, das Impressum in `dc:description`.

### Beenden (`shutdown.rs`)

- `crates/baruch-server/src/shutdown.rs` – Beenden (Runde 32).
  - `Shutdown` (in `AppState::shutdown`): Auslöser auf einem `watch`-Kanal; einmal verlangt, bleibt verlangt.
  - `own_machine(hosts, peer, headers)`: wahr nur, wenn die Gegenstelle (`ConnectInfo`) bekannt und Loopback ist
    (`to_canonical`, damit `::ffff:127.0.0.1` zählt), `guard::via_proxy` verneint und `Host` fehlt oder
    `guard::allowed_host` erfüllt. `guard.rs` blieb unberührt.
  - `serve` → `serve_until(listener, state, grace, signal)`: `axum::serve(…)` mit
    `into_make_service_with_connect_info::<SocketAddr>()` und `with_graceful_shutdown` auf das erste von Signal
    (`shutdown/signals.rs`) und `Shutdown::requested()`. Danach nimmt der Server keine Verbindung mehr an,
    ruhende Verbindungen schließen, laufende Anfragen werden beantwortet. Eine zweite Zukunft wartet ab dem
    Auslöser die Frist (`GRACE` = 10 s) ab; ist dann noch eine Verbindung offen, wird nicht länger gewartet
    (Meldung auf stderr). Zuletzt `store.pool().close()` (höchstens `CLOSE_WAIT` = 5 s): SQLite schreibt das
    Protokoll (`-wal`) zurück. `serve` schreibt dann „Baruch beendet.“; `main` kehrt zurück, Status 0.
  - `shutdown/signals.rs` (nur `tokio`, einzeln übersetzbar): Strg+C überall; Unix `SIGTERM`; Windows
    `ctrl_break`, `ctrl_close`, `ctrl_shutdown` – die letzten beiden mit Frist `WINDOWS_CLOSE_GRACE` = 3 s, weil
    Windows den Prozess kurz nach dem Ereignis selbst beendet. Kein `SIGHUP` (ein Empfänger höbe `nohup` auf).
  - Seiten: eigene Template-Umgebung (`base.html`, `shutdown.html`, `shutdown_done.html`). Die Abschlussseite
    ist in sich geschlossen (Stil inline, einziges Skript `window.close()`), weil der Server danach nichts mehr
    liefert. `POST /beenden` antwortet zuerst und löst `REPLY_DELAY` = 500 ms später aus.
  - Kein Widget, keine `.ts`-Datei: Das Inline-Skript der Abschlussseite genügt.
- Tests: `src/shutdown.rs` (Auslöser, Tabelle zu `own_machine`), `tests/round32_beenden.rs` (Seiten am echten
  Server im Testprozess, echter Prozess mit Knopf/SIGTERM/Strg+C, Starter mit Attrappen in abgeschlossenem
  `PATH`, Units, Workflows).
- CI: `pages.yml` und `release.yml` trennen `pages-build` und `pages-deploy`; Regel 11 in
  `scripts/ci-check-workflows.sh`, Gegenproben in `scripts/ci-selftest.sh`.

Bekannte Grenze: Läuft beim Beenden noch ein blockierender Auftrag (`spawn_blocking`, etwa ein Typst-Satz),
wartet die Tokio-Laufzeit beim Verlassen von `main` auf ihn – das Ende kann sich dann über die Frist hinaus
verzögern. Die Datenbank ist zu diesem Zeitpunkt bereits geschlossen. Nicht mit einem Test belegt.

### Integration (Runde 32)

- **Nahtstelle Leser/Schreiber:** Beide Seiten entstanden getrennt gegen `model.rs` und `ooxml.rs`. Die Rundreise
  (`crates/baruch-docx/tests/roundtrip.rs` am Modell, `tests/round32_abnahme.rs` über den Server) fand drei Lücken,
  alle im Leser bzw. in `to_typst` behoben:
  - `w:lvlOverride`/`w:startOverride` an `w:num` gilt als erste Nummer der Liste (`read.rs`, `Numbering::overrides`) –
    so schreibt der Schreiber jede weitere Aufzählung, und so beginnt auch Word eine Liste neu.
  - Eine Aufzählung, die nicht bei 1 beginnt, trägt in `to_typst` an jedem Punkt ihre Nummer („3.“, „4.“ …,
    geschachtelt ab 1). „3.“ gefolgt von „+“ versteht Typst, `baruch-wysiwyg` läse es aber als zwei Listen.
  - Der Pfad eines Bildes im Werk kommt aus `wp:docPr name` zurück, wenn der Name wie ein Pfad aussieht
    (`read/body.rs`, `work_path`: relativ, ohne `..`, Endung passend zum Inhalt); Bilder im sichtbaren Teil einer
    Typst-Insel werden mitgelesen (`island_images`), weil der Quelltext der Insel sie nennt.
- **Signale vor „Baruch läuft“:** `signals::system_signal()` richtet die Empfänger (Unix: `SIGINT`, `SIGTERM`) beim
  Aufruf ein, nicht erst beim ersten Warten; `main` ruft es vor der Zeile „Baruch läuft“ und reicht die Zukunft an
  `shutdown::serve_with(listener, state, signal)` (`serve` = `serve_with` mit `system_signal()`). Vorher beendete ein
  `SIGTERM` unmittelbar nach dieser Zeile den Prozess hart (Test `sigterm_ends_the_throwaway_server_with_exit_0`).
- **`main.rs`:** Nach erfolgreichem `shutdown::serve_with` endet `main` mit `std::process::exit(0)`, damit ein noch
  laufender blockierender Auftrag das Ende nicht über die Frist hinaus verzögert (die Datenbank ist dann schon
  geschlossen, die Schlusszeile ausgegeben).
- **Wächter `round33_seiten`:** Die drei neuen Vorlagen (`docx_import.html`, `docx_export.html`, `shutdown.html`)
  stehen in `TEMPLATES`.
- **Offen:** `commentsExtended.xml` schreibt der Schreiber nicht (Antworten stehen als eigene Kommentare, der Server
  schreibt ohnehin keine); die privat gehaltenen Konstanten des Schreibers (`NS_APP`, `PART_FOOTNOTES_RELS`,
  `STYLE_HYPERLINK`, `STYLE_COMMENT_TEXT`, `STYLE_COMMENT_REFERENCE`) sind nicht nach `ooxml.rs` gezogen; Zitate im
  Text stehen in Word ohne Anführungszeichen; SVG und WebP werden nicht eingebettet.

## Runde 34 – Mindmap

Die Gedankenkarte (Runde 25) ist eine Mindmap wie in XMind geworden. Drei Zweige (`team/mindmap_daten`,
`team/mindmap_widget`, `team/mindmap_austausch`) auf der Schnittstelle R34-S1, zusammengeführt mit `--no-ff` ohne
Textkonflikte; die inhaltliche Naht war die **Wolke** (siehe „Integration“).

### Wolke und Mindmap: eine Regel mit vier Plätzen

Der Vertrag der Runde ließ Einfälle ohne Elternknoten und ohne Bezug als „freie Hauptthemen“ direkt am Werk hängen;
die Wolke „Ohne Kapitel“ wäre entfallen. Nach dem Nachtrag des Nutzers vom 2026-11-25 („Es macht Sinn eine
Verbindung zwischen der Wolke und der Mindmap zu machen“) gilt **eine Regel mit vier Plätzen**:

1. `parent` gesetzt → Unterknoten dieses Einfalls (gleiches Werk, kein Kreis, Kette höchstens 32 tief). Der Bezug
   (`key`) bleibt daneben erlaubt und ist dann nur der Link zum Abschnitt.
2. sonst `key` gesetzt → Kind des Ankers (nächster Vorfahre der Art Kapitel, sonst der Punkt selbst).
3. sonst `main_topic` → **freies Hauptthema** direkt am Werk.
4. sonst → **in der Wolke**: nicht im Baum, sondern in der Ablage neben der Mindmap.

Dafür trägt `idea` eine vierte neue Spalte `main_topic` (0/1, Vorgabe 0); der Vertrag sah drei vor. Ohne sie ließe
sich ein Einfall, den man aufs Werk zieht, nicht von einem Einfall ohne Platz unterscheiden. Folgen:

- **Bestehende Einfälle** ohne Bezug (heute Wolke „Ohne Kapitel“) liegen nach der Migration in der Wolke; Einfälle
  mit Bezug hängen wie bisher an ihrem Kapitel. Nichts wird zum Hauptast, ohne dass jemand es dorthin zieht.
- **Neue Einfälle ohne Platz** (Schnellerfassung Alt+E, Formular, RPC `idea.create` und MCP `idea_create` ohne
  `parent`/`key`) landen in der Wolke.
- **Aus der Wolke in die Mindmap:** `idea.move` mit `parent` (auf einen Einfall), mit `parent: null, key` (auf ein
  Kapitel) oder mit `parent: null, key: null` bzw. `cloud: false` (aufs Werk, wird Hauptthema).
- **Aus der Mindmap in die Wolke:** `idea.move` mit `cloud: true`. Der Einfall verliert Elternknoten und Bezug;
  **seine Unterknoten bleiben an ihm** – der Teilbaum wandert als Ganzes in die Wolke. Begründung: kein Einfall
  verliert dabei stillschweigend seinen Zusammenhang, die Aktion ist mit einem `idea.move` zurück umkehrbar
  (Strg+Z des Widgets), und „nur er“ hieße, seine Kinder ungefragt an den Großelternknoten zu hängen. Wer nur den
  einen Einfall lösen will, zieht vorher die Unterknoten woandershin.
- **Zähler:** `cloud.count` („n in der Wolke“) zählt die Einfälle der obersten Ebene der Wolke ohne Filter,
  `cloud.count_with_children` samt ihren Unterknoten.
- **Nähe als Hilfe:** `cloud.hints` nennt je Einfall der Wolke höchstens einen Vorschlag „passt zu Ast …“ – den
  nächsten Einfall in der Mindmap (gespeicherte Einbettungen und gleiche Schlagwörter wie auf der Karte, nie eine
  neue Einbettung) und den Hauptast, in dem er hängt. Die Vorschläge werden auch berechnet, wenn der Filter die
  Nähe-Linien im Bild ausschaltet.
- **Wiederherstellen** eines Einfalls, dessen Elternknoten noch gelöscht ist: Er bleibt an seinem Bezug bzw. liegt
  in der Wolke (`detached: true`), nicht als Hauptthema am Werk.
- **Baum-Import** (`tree::import` ohne `parent`): Die obersten Knoten werden freie Hauptthemen, nicht Einfälle der
  Wolke – wer eine Mindmap einliest, will sie in der Mindmap sehen.

### Daten (Migration 0052, Store)

Migration `0052_idea_tree.sql` (nur `ADD COLUMN`, nicht in `FOREIGN_KEYS_OFF`):

```sql
ALTER TABLE idea ADD COLUMN title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200);
ALTER TABLE idea ADD COLUMN parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL;
ALTER TABLE idea ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
ALTER TABLE idea ADD COLUMN main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1));
CREATE INDEX idea_parent ON idea(parent_id, position);
```

Regeln im Store (`crates/baruch-store/src/ideas.rs`):

- **Titel:** höchstens 200 Zeichen, eine Zeile, darf leer sein. Ein Einfall braucht Titel **oder** mindestens ein
  Schlagwort. Angezeigter Titel (`Idea::display_title`): der Titel, sonst die Schlagwörter „a, b“, sonst der Anfang
  des Gedankens, sonst „Einfall <id>“ – nie leer. Der Titel steht im Volltextindex (`idea_fts`, Spalte `words`).
- **Elternknoten:** nicht gelöschter Einfall desselben Werks; kein Kreis; Kette höchstens 32 Einfälle
  (`MAX_IDEA_DEPTH`). Einfälle ohne Werk haben keinen Elternknoten.
- **Reihenfolge:** `position`, dann `id`, unter Geschwistern (gleicher Elternknoten bzw. alle Einfälle des Werks
  ohne Elternknoten). Neue kommen ans Ende; „vor ein Geschwister“ (`before`) nummeriert die Geschwister lückenlos
  ab 1 neu, ohne deren `updated` anzufassen. Bestehende Einfälle tragen 0 und ordnen sich nach `id`; solange unter
  einem Elternknoten nichts geordnet wurde, behalten auch neue die 0 (im Austausch dann ohne `platz` – ein Werk ohne
  geordneten Baum gibt dieselbe Datei aus wie vor Runde 34 und besteht auch das eingefrorene u8-Schema).
- **Löschen** nimmt die nicht gelöschten Nachfahren mit **demselben Zeitstempel** mit; **Wiederherstellen** bringt
  den Einfall und genau die Nachfahren mit diesem Zeitstempel zurück (früher einzeln Gelöschtes bleibt gelöscht).
  Verbindungen (`idea_link`) und Lagen (`idea_map_pos`) bleiben dabei gespeichert.
- **Werkwechsel** (`idea.update` mit `work`): Der Einfall löst sich vom Elternknoten und liegt im neuen Werk in der
  Wolke; seine Unterknoten bleiben im alten Werk und verlieren den Elternknoten (kein Baum über zwei Werke).
- **Lagen** (`idea_map_pos`) bleiben gespeichert und im Austausch, sind in der Mindmap aber ohne Wirkung (die
  Anordnung rechnet der Server aus dem Baum).

Spiegel `einfaelle.md`: Hat ein Werk mindestens einen Einfall mit Titel, Elternknoten oder Hauptthema, steht vor den
Einträgen der Abschnitt `## Baum` – die Einfälle eingerückt nach Elternknoten, Geschwister nach `position`, dann
`id`, oben mit dem Platz („(am Abschnitt key 12)“, „(Hauptthema)“, „(Wolke)“). Jeder Eintrag nennt zusätzlich
`- Titel: …`, `- Unterknoten von: Einfall <id>` bzw. `- Platz: Hauptthema am Werk`. Ohne Baum bleibt die Datei
byte-gleich wie vor Runde 34.

Austauschformat: Unterversion 9, `<einfall>` mit `titel`, `eltern`, `platz`, `hauptthema` – siehe
`docs/Austauschformat.md` („Baum der Mindmap“).

### Baum, Wolke und Import im Server (`idea_map/tree.rs`)

Die Signaturen aus S1.2 sind wörtlich geblieben (`MindMap`, `TreeNode`, `TreeKind`, `tree`, `ImportTree`, `import`,
Konstanten). Neu daneben:

```rust
pub struct CloudHint { pub idea: IdeaId, pub node: String, pub node_title: String,
                       pub branch: String, pub branch_title: String, pub score: f32, pub shared_words: Vec<String> }
pub struct Cloud { pub work: WorkId, pub ideas: Vec<TreeNode>, pub count: u32, pub count_with_children: u32,
                   pub hints: Vec<CloudHint>, pub links: Vec<IdeaLink> }
pub async fn cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<Cloud, RpcError>;
pub async fn tree_and_cloud(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<(MindMap, Cloud), RpcError>;
```

- `tree` liefert Werk (`w`), Teile und Kapitel (`n:<key>`) in Gliederungsreihenfolge, danach die freien Hauptthemen
  nach `position, id`; unter jedem Knoten erst die Gliederungspunkte, dann seine Einfälle nach `position, id`.
  Einfälle der Wolke stehen **nicht** im Baum.
- Filter `status`, `word`, `origin`: Ein Einfall, der nicht passt, bleibt als Geist (`ghost: true`, `hit: false`)
  stehen, solange ein Nachfahre passt. `q` blendet nichts aus, es setzt `hit` (Volltext, auch über den Titel).
- `MindMap::links` und `MindMap::near.pairs` nennen nur Einfälle, die im Baum stehen; `Cloud::links` die
  Verbindungen mit mindestens einem Ende in der Wolke. `total_ideas` zählt die gezeigten Einfälle im Baum (ohne
  Geister, ohne Wolke); `descendants` zählt je Knoten die Einfälle, die unter ihm im Baum stehen.
- `Cloud::ideas` sind `TreeNode` der Art `idea` mit ihren Unterknoten in `children`; derselbe Filter gilt.
- `import` legt den Baum in einer Transaktion an (Store `idea_import_tree`) und schreibt **einen** Commit „Mindmap
  eingelesen: n Einfälle“ (`Baruch-Action: idea-import`) mit der übergebenen Herkunft. Abgelehnt (-32000, nichts
  angelegt, kein Commit): kein Knoten, mehr als `MAX_IMPORT_NODES` (2000) Knoten, tiefer als `MAX_TREE_DEPTH` (32,
  auch zusammen mit der Tiefe von `parent`), `parent` gelöscht oder in einem anderen Werk, Titel über 200 bzw. Notiz
  über 4000 Zeichen, Schlagwörter über den Grenzen der Einfälle (höchstens 20, je 60 Zeichen), eine Verbindung mit
  einem Pfad ohne Knoten. Ein Knoten ohne Titel und Schlagwort heißt „(ohne Titel)“; eine Verbindung eines Knotens
  mit sich selbst wird übergangen. `ImportTree::title` steht im Rumpf des Commits, `warnings` werden nicht
  gespeichert. Wer `import` ruft, kürzt Titel und Notizen vorher
  (`files/import.rs`); der Import kürzt nicht, er lehnt ab.

- Knoten der Mindmap sind Zeilen der Tabelle `idea` – keine zweite Struktur. Der Store kennt die Regeln des Baums
  (`idea_create_at`, `idea_move`, `idea_delete_tree`, `idea_restore_tree`, `idea_import_tree`, `idea_tree_rows`),
  der Server setzt ihn mit der Gliederung zusammen (`idea_map::tree`), der Spiegel schreibt ihn in `einfaelle.md`.
- Jede schreibende Aktion läuft unter der Schreibsperre und ergibt genau einen Commit mit Herkunft (`idea-create`,
  `idea-update`, `idea-move`, `idea-delete`, `idea-restore`, `idea-import`).
- Die flache Karte (`idea_map::build`, RPC `idea.map`) bleibt als Abwärtsverträglichkeit; der Baum baut auf ihr auf
  (Gliederung, Filter, Verbindungen, Nähe, Befunde) und liest die Einfälle zusätzlich ungefiltert, um Geister und
  die Wolke zu bestimmen.

### Bild und Seite (`idea_map/bild/`)

`bild/mod.rs` (Seite `/werke/{id}/karte`, Bild `/werke/{id}/karte/bild`, Filter), `bild/mindmap.rs` (Zeichnung),
`bild/tree_layout.rs` (Anordnung, rein rechnerisch und deterministisch). Eine Anordnung, in Rust: Bildschirm und
Ausgabe (`.svg`, `.pdf`) entstehen aus derselben Funktion `mindmap::render(map, options)`; das Widget ordnet nichts
selbst an. `bild/layout.rs` und `bild/svg.rs` der alten Gedankenkarte sind gelöscht.

- Bühne: `<div class="mm-stage" data-widget="mindmap" data-work data-bild data-back data-zu data-wahl
  data-gliederung>`, darin `[data-mm-toolbar]`, `[data-mm-canvas]` mit `<svg class="mm-svg" data-cloud-count
  data-zu>` und `<g class="mm-viewport">`, darunter `[data-mm-status]` (Live-Region). Zähler:
  `[data-mm-cloud-count]`.
- Reihenfolge im Viewport: Äste, Nähe, Beziehungen, Wolke, Knoten, zuletzt die Beschriftungen der Beziehungen
  (`g.mm-link-labels`; sie liegen über den Knoten, damit nahe Knoten sie nicht verdecken – Integration).
- Knoten: `<g class="mm-node mm-<work|part|chapter|idea>" data-node data-parent data-depth data-side="right|left"
  data-branch="0..7" data-editable="1|0" data-title data-own tabindex="0">`; Einfälle `data-idea`, mit Bezug
  `data-key`; mit Nachfahren `data-children="n"`; mit offenen Befunden `data-open-findings="k"`. Zusatzklassen: `mm-b<n>` (Astfarbe), `mm-main` (Hauptast),
  `mm-done`, `mm-hit`, `mm-dim`, `mm-ghost`, `mm-closed`, `mm-many-findings`. Das Werk hat weder `data-side` noch
  `data-branch`. Kinder: `<title>`, `rect.mm-box`, `text.mm-label` (in `a.mm-label-link`), `g.mm-note`,
  `a.mm-section`, `a.mm-findings[data-open-findings]`, `a.mm-toggle[data-count]`.
- Äste: `<path class="mm-edge mm-b<n>" data-from data-to>` (kubische Kurve, `stroke-width` 3.6 / 2.4 / 1.8 / 1.4).
- Beziehungen: `<g class="mm-rel">` mit `<path class="mm-link mm-link-<kind>" data-link data-from="i:<a>"
  data-to="i:<b>" data-end-from data-end-to>`; die Beschriftung ist `<text class="mm-link-label" data-link>` in
  `g.mm-link-labels`. Widerspruch zusätzlich `mm-contradiction` an Gruppe, Pfad und Text. `data-end-*` nennt den
  Knoten, an dem die Linie sichtbar endet.
- Nähe: `<path class="mm-near" data-from data-to>` (nur mit `near`, nur zwischen Knoten des Baums).
- Wolke: `<g class="mm-cloud" data-count>` mit `rect.mm-cloud-area` und `text.mm-cloud-head`; Kärtchen sind
  `g.mm-node.mm-idea.mm-cloud-item[data-cloud="1"][data-parent="w"]` (welche Kinder der Wurzel in der Wolke
  liegen, sagt `RenderOptions::cloud`; alle anderen sind Hauptäste), mit Vorschlag `data-fits="<Hauptast>"` und
  `text.mm-cloud-hint.mm-b<n>`, mit Unterknoten `text.mm-cloud-below` („+ n“).
- Eingeklappte Knoten zeichnen keine Nachfahren; `a.mm-toggle` zeigt „+ n“, sein `href` ist `?<Filter>&zu=<neue
  Liste>`. Die Zahl zählt alle verborgenen Knoten (jede Art), nicht nur Einfälle.
- **Fragment** (`standalone: false`): kein `xmlns`, kein `<style>`, keine Farbattribute – Farben über
  `--mm-b0`…`--mm-b7` in `karte.css` (hell, dunkel nach System, dunkel nach Schalter). **Eigenständig**
  (`standalone: true`, für `.svg`/`.pdf`): `xmlns`, eigener `<style>`, Farben als Attribute
  (`mindmap::BRANCH_COLORS`), weißer Grund, keine Links (statt `<a>` ein `<g>` derselben Klasse).
- Die Anordnung ist deterministisch; Textbreiten sind geschätzt (der Server kennt die Schrift des Browsers nicht),
  Titel über 48 Zeichen werden im Bild gekürzt (voller Titel in `data-title` und im Tooltip).

### Widget `web/src/mindmap.ts`

Ersetzt `idea_map.ts` (gelöscht). Es ordnet nichts selbst an: Nach jeder Aktion holt es `data-bild` mit Filter und
`zu=` neu, setzt das SVG ein und behält Auswahl und Ausschnitt (`transform` auf `.mm-viewport`). RPC (nur diese,
über `rpc.ts`): `idea.create`, `idea.update`, `idea.move`, `idea.delete`, `idea.restore`, `idea.link`,
`idea.unlink`, `idea.link_restore`. Rückgängig ist die Umkehr-RPC der letzten Aktion (anlegen ↔ `idea.delete`,
löschen ↔ `idea.restore`, umbenennen ↔ `idea.update` mit dem alten gespeicherten Titel aus `data-own`, umhängen ↔
`idea.move` zurück an den alten Platz – `parent`, `key`, `before`, bei einem Kärtchen der Wolke `cloud: true`, bei
einem Hauptthema `parent: null, key: null, cloud: false` –, verbinden ↔ `idea.unlink`, lösen ↔ `idea.link_restore`).
Knöpfe im Menüband tragen `data-mm-action` (`child`, `sibling`, `rename`, `delete`, `link`, `cloud`, `undo`), Links
mit `data-mm-zu` setzen die eingeklappten Knoten aus ihrem `href`.

`idea.delete` schickt das Widget mit `confirm: true` (die Rückfrage stellt es selbst). Aufs Werk gezogen bzw. Tab
am Werk ergibt ein freies Hauptthema (`cloud: false`), in die Wolke gezogen `cloud: true`.

### Ein- und Ausgabe, Gliederungsansicht (`idea_map/files/`)

`crates/baruch-server/src/idea_map/files/` (Ein- und Ausgabe der Mindmap, Bedienung ohne JavaScript):

- `mod.rs` – Routen (`pages()`, `writes()`), die Ausgaben, `POST /werke/{id}/karte/act`.
- `export.rs` – Wolke und Hauptäste aus der Sicht `tree::Scene` (`cloud`, `placed`), das Blatt
  (`Sheet`) als Vorstufe von `.mm`/`.xmind`, Markdown.
- `freemind.rs`, `xmind.rs` – je Schreiber und Leser. `import.rs` – Grenzen, Reinigung, `Raw` → `ImportTree`
  (`finish`, `check`). `upload.rs` – Formular, Vorschau, Bestätigung. `outline.rs` – Gliederungsansicht.
- Vorlagen `karte_gliederung.html`, `karte_einlesen.html`; Stil `static/karte_gliederung.css` (nur Variablen aus
  `app.css`).

Sicherheit beim Einlesen: Upload höchstens 10 MiB (`DefaultBodyLimit` an der Route). Aus dem ZIP wird allein der
Eintrag `content.json` gelesen (Namen als Bytes verglichen, nie als Pfad benutzt, nichts auf Platte entpackt); beim
Entpacken wird mitgezählt und bei 20 MiB abgebrochen; Archive mit mehr als 50 Einträgen oder mit Namen, die aus dem
Archiv herausführten (`..`, absoluter Pfad, Laufwerk, `\`), werden ganz abgelehnt. XML: eine DOCTYPE wird
abgelehnt, Entitäten werden nicht aufgelöst (nur die fünf von XML, `nbsp`, Zeichenreferenzen); der Leser arbeitet
mit eigenem Stapel und begrenzt Knotenzahl und Tiefe beim Lesen. Zwischen Vorschau und Bestätigung steht der Baum
als JSON in einem verborgenen Formularfeld (keine Datei im Datenordner) und wird bei der Bestätigung erneut
geprüft (`import::check`). Beide POST-Routen liegen hinter `guard::same_origin`.

### Abweichungen vom Vertrag der Runde

- **`idea.delete`:** Der Vertrag nennt „Antwort zusätzlich `deleted: [ids]`“. `deleted` ist seit Runde 7 `true`
  (RPC und MCP, geprüft u. a. in `round23_geheimnisse.rs`); die Liste heißt darum **`deleted_ids`**, `deleted`
  bleibt `true`.
- **Ablehnungen von `idea.move` und `tree::import`** tragen den Code **-32000** (`rpc::REFUSED`, „abgelehnt“). Der
  Vertrag schreibt -32001; das ist seit Runde 19c `RAG_UNAVAILABLE`.
- **`MapIdea`** (Typ der flachen Karte) bekommt die Felder `title`, `parent`, `position` nicht am Typ, sondern in
  der Antwort von `idea.map` und `idea_map` (dazu `main_topic`, `cloud`): Die eingefrorene Datei `bild/layout.rs`
  baut `MapIdea` in ihren Tests von Hand und bräche sonst.
- **`MindMap` unverändert:** Die Wolke ist kein Feld von `MindMap` (Handbau in fremden Tests bräche), sondern eine
  zweite Funktion `tree::cloud` bzw. beide zusammen `tree::tree_and_cloud` (siehe 3).
- **XSD:** Das Schema der Unterversion 9 liegt als neue Datei `docs/schema/baruch-werk-1-u9.xsd` neben den
  eingefrorenen; `baruch-werk-1-u8.xsd` ist unverändert (SHA-256 im Test).
- Dateinamen der Ausgaben heißen `<slug>-mindmap.*` (vorher `-karte.*`); die Adressen bleiben `/werke/{id}/karte.*`.
- Ein Einfall eines anderen Werks im Formular der Mindmap ergibt 409 (vorher 422).
- Das alte XMind-8-Format (nur `content.xml`) wird mit klarer Meldung abgelehnt, nicht gelesen.

### Integration (Runde 34)

- **Die Naht „Wolke“.** mindmap_daten liefert die Wolke getrennt vom Baum (`tree::cloud`, `tree::tree_and_cloud`);
  mindmap_widget und mindmap_austausch hatten gegen S1 angenommen, „Einfälle direkt an der Wurzel“ seien die Wolke.
  Damit wären freie Hauptthemen als Kärtchen der Wolke gezeichnet worden und die echte Wolke gar nicht. Der
  Integrator hat die Sicht **`tree::Scene`** eingeführt (`tree::scene`): Sie hängt die Einfälle der Wolke hinter den
  Hauptästen an die Wurzel (damit `walk`/`find` sie kennen), nennt ihre Kennungen (`Scene::cloud`), den Zähler ohne
  Filter (`cloud_count`) und die Vorschläge (`hints`), und fügt die Verbindungen mit einem Ende in der Wolke hinzu.
  Seite, Bild, Gliederungsansicht und alle Ausgaben lesen nur noch diese Sicht; `RenderOptions` trägt dafür `cloud`,
  `cloud_count` und `hints` (`RenderOptions::of(scene, standalone, closed)`), `tree_layout::layout_with` unterscheidet
  Hauptäste und Kärtchen nach diesen Kennungen. RPC `idea.tree` und MCP `idea_tree` bleiben bei `{map, cloud}`.
- **Hauptthema und Wolke in der Bedienung.** Widget: Tab am Werk und „aufs Werk ziehen“ schicken `cloud: false`
  (freies Hauptthema), „in die Wolke“ `cloud: true`; die Umkehr für Strg+Z kennt beide Plätze. Formular
  `POST /werke/{id}/karte/act`: `ziel=wolke` → `cloud: true`, `ziel=w` → Hauptthema, `ort=werk` beim Anlegen;
  Gliederungsansicht mit „Neues Hauptthema“ und „Als Hauptthema ans Werk“. Einlesen ohne Zielknoten legt Hauptthemen
  an (Texte der Vorlage angepasst).
- **Gefunden beim Zusammenführen:** Das Widget rief `idea.delete` ohne `confirm: true` – der Server hätte jedes
  Löschen (und das Rückgängig eines Anlegens) abgelehnt. Behoben, Test in `round34_abnahme`.
- Vorschlag „passt zu Ast …“ in der Gliederungsansicht nennt den Hauptast mit Nummer (`export::node_title`).
- Gelöscht: `bild/layout.rs`, `bild/svg.rs`, `bild::load`, `export::tree_order`, `web/src/idea_map.ts`, der S1-Text
  `tree::NOT_YET`. `PdfKind::Map` heißt „Mindmap“. Alte Bildregeln `.im-*` in `static/karte.css` stehen noch (Filter,
  Gruppen, Widersprüche und Legende der Seite benutzen einen Teil davon); ausmisten ist offen (Backlog).
- Tests: `round34_abnahme.rs` (Bild und Aktionen des Widgets über RPC, Gliederungsansicht ohne JavaScript mit allen
  Aktionen, MCP mit Herkunft, Rundreise `.mm`/`.xmind` in ein zweites Werk, alte Daten unverändert). Angepasste
  fremde Tests: elf Rundreisen auf `version="9"`, `round25_abnahme`, `round25_bild`, `round18_menueband`,
  `round28_abnahme`, `round28_uebersicht`, `round9_menu`, `round33_seiten` (zwei neue Vorlagen).

## Runde 35 – Konten und Rollen

Konten, Passwort-Hash, Sitzungen und die Anmelde-Schicht (Modul `auth`), Rollen je Werk und die einzige Stelle der
Rechteprüfung (Modul `roles`), Seiten und Menü (Modul `konten_ui`). Bedienung: `docs/Bedienung.md` („Konten und
Rollen“), Betrieb: `docs/Betrieb.md` („Anmeldung und Konten“), Methoden und Routen: `docs/RPC.md` („Runde 35“).

- **Ohne Konten** (Tabelle `account` leer) bleibt Baruch offen wie vor Runde 35: `Who::Open`, kein Cookie, keine
  Umleitung, keine zusätzliche Kopfzeile.
- **Mit Konten** verlangen Seiten, Formulare und `/rpc` eine Sitzung – auch am eigenen Rechner (Vorgabe). `/mcp`
  bleibt bei Tokens; die Schicht lässt `/mcp` unberührt.
- Das erste Konto ist der Autor und entsteht nur am eigenen Rechner (`POST /einrichten`). Jedes weitere Konto ist
  ein Lektor. Konten werden nie gelöscht (das Handle `@lektorat:<slug>` bleibt vergeben), nur gesperrt.

**Entscheidungen des PM** (der Nutzer kann sie umstoßen): Mit Konten gilt die Anmeldung auch am eigenen Rechner;
die Ausnahme „Am eigenen Rechner ohne Anmeldung“ (`store_meta` `auth.local_open`) schaltet der Autor ein. Direkter
Zugriff von anderen Rechnern bleibt gesperrt (`guard::remote_only_mcp` unverändert) – ein Lektor kommt über den
Reverse-Proxy auf demselben Rechner. Kontoart global (`author` | `lektor`), Rolle je Werk. Für Lektoren ist alles
verboten, was nicht ausdrücklich erlaubt ist. Konten und Freigaben sind Instanzdaten (nicht im Austauschformat, kein
Commit im Spiegel). Kein neues MCP-Werkzeug.

### Konten und Anmeldung (`auth`)

| Datei | Inhalt |
|---|---|
| `baruch-store/src/accounts.rs` | Tabellen `account`, `account_session`, `login_attempt` (Migration 0053) und der Schalter `store_meta` `auth.local_open`. Der Store hasht nie und sieht nie ein Klartext-Passwort; `Account` trägt keinen Hash. |
| `baruch-server/src/auth/mod.rs` | `Who`, `Identity`, `current()`/`scope()` (task-lokal), `AuthState` (gemerktes „es gibt Konten“), `accounts_exist`, `identity_of`, `check_name`, `message` |
| `auth/gate.rs` | Schicht `gate` (im Router direkt außerhalb von `consent::inject`), `resolve`, `own_machine`, `peer` |
| `auth/password.rs` | Argon2id: `hash`, `hash_with`, `verify`, `policy`, `hash_blocking`, `verify_blocking`, `DUMMY_PHC` |
| `auth/session.rs` | Token, Cookie, `current_hash()` (Sitzung der laufenden Anfrage, task-lokal) |
| `auth/throttle.rs` | Sperre gegen Durchprobieren: `wait_seconds`, Bereiche, `reserve`/`succeeded` |
| `auth/routes.rs` | Formulare (hinter `guard::same_origin`) und die gemeinsamen Bausteine `create_account`, `change_password`, `check_current`, `set_active` |
| `auth/rpc.rs` | `account.*` |
| `auth/cli.rs`, `main.rs` | Notweg `baruch-server passwort <name>` |
| `guard.rs` | `forwarded(headers)`, `FORWARDED_HEADERS` |

**Ablauf einer Anfrage** (`auth::gate`): (1) `/mcp` läuft als `Who::Open` durch. (2) `Who` aus den Extensions der
Anfrage (nur im Prozess setzbar: Tests), sonst `resolve`. (3) `Who::Anonymous`: `PUBLIC_PATHS` und `/static/js/…`
laufen durch; GET/HEAD → 303 auf `/anmelden?next=<Pfad+Query, prozentkodiert>`; `POST /rpc` → 401 mit JSON-RPC-Fehler
`-32002`; sonst 401 Text „Anmeldung erforderlich.“ (4) `roles::check_request`. (5) Die Anfrage läuft in
`auth::scope(who, …)` und `session::scope(<Token-Hash>, …)`.

**`resolve`:** keine Konten → `Open`. Sonst: gültige Sitzung eines aktiven Kontos → `Account`; sonst Schalter
„Am eigenen Rechner ohne Anmeldung“ an **und** `gate::own_machine` → `LocalAuthor`; sonst `Anonymous`. Ein
Datenbankfehler führt nie zu mehr Rechten (er steht auf stderr, die Anfrage gilt als nicht angemeldet; ist schon
die Zahl der Konten nicht lesbar, gilt „es gibt Konten“).

**Eigener Rechner** (`gate::own_machine`, Entscheidung 0.2): `shutdown::own_machine` (Gegenstelle bekannt und
Loopback, kein Proxy-Name, `Host` eine IP-Adresse oder `localhost`) **und** keine der Kopfzeilen `Forwarded`,
`X-Forwarded-For`, `X-Forwarded-Host`, `X-Forwarded-Proto`, `X-Real-IP`, `Via` (`guard::forwarded`; schon das
Vorhandensein zählt, der Inhalt wird nicht gelesen). Ohne `ConnectInfo`: nein. Sind Proxy-Namen erlaubt
(`--allow-host`), gilt auch eine Anfrage ohne `Host` nicht als eigener Rechner.

**Antworten:** Mit Konten bekommt jede Antwort (angemeldet oder abgewiesen) `Cache-Control: no-store`, wenn sie
selbst keine Angabe trägt (`/static/…` behält `no-cache`). Ohne Konten hängt die Schicht nichts an.

**Regel:** `auth::current()` und `auth::session::current_hash()` nie in abgespaltenen Aufgaben (`tokio::spawn`,
`spawn_blocking`) auswerten – vorher holen und mitgeben.

#### Passwort

- Argon2id, **m = 19456 KiB, t = 2, p = 1**, Salz 16 Byte (getrandom), Ausgabe 32 Byte, gespeichert als PHC-Text
  (`$argon2id$v=19$m=19456,t=2,p=1$<Salz>$<Hash>`), Crate `argon2` 0.5. Hashen und Prüfen laufen in
  `spawn_blocking`.
- `verify` liest die Parameter aus dem PHC-Text: Wächst die Vorgabe später, bleiben alte Hashes prüfbar. Nur
  Argon2id gilt; ein unlesbarer Text passt zu keinem Passwort.
- Regel: 10 bis 200 Zeichen (Unicode-Zeichen, nicht Bytes), sonst keine.
- Unbekannter Name: Prüfung gegen den festen Schein-Hash `password::DUMMY_PHC` (gleiche Parameter, gleiche Dauer),
  gleiche Meldung „Name oder Passwort stimmt nicht.“ – auch für ein gesperrtes Konto und ein überlanges Passwort.
- Kein Klartext in Datenbank (CHECK `password LIKE '$argon2id$%'`), Logs, Fehlermeldungen oder Antworten; kein Typ
  mit Passwort leitet `Debug` ab; Meldungen zu falschen RPC-Parametern zitieren keinen Wert.

#### Sitzung

- Token: 32 Zufallsbytes (getrandom) als Base64url (43 Zeichen). In der Datenbank steht nur der SHA-256 (hex,
  64 Zeichen, CHECK der Spalte). Wer nur die Datenbank kennt, kann sich damit nicht anmelden.
- Cookie: `baruch_session=<token>; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000`, zusätzlich `; Secure`, wenn
  `guard::via_proxy` wahr ist (Proxy-Name im `Host`, oder Proxy-Namen erlaubt und kein `Host`).
- Gültig 30 Tage ohne Benutzung, höchstens 90 Tage seit der Anmeldung (beides prüft der Server in SQL);
  `last_seen` wird höchstens einmal je Minute geschrieben. Abgelaufene Sitzungen aller Konten räumt die nächste
  Anmeldung weg.
- **Erneuern (Ergänzung zum Vertrag):** Liegt die vorige Benutzung länger als eine Stunde zurück, setzt die Antwort
  dasselbe Cookie erneut (frische 30 Tage im Browser) – sonst endete die Sitzung im Browser 30 Tage nach der
  Anmeldung, obwohl sie benutzt wird. Die Obergrenze von 90 Tagen bleibt.
- Abmelden löscht die Sitzung und das Cookie (`Max-Age=0`). Passwortwechsel löscht alle **anderen** Sitzungen des
  Kontos (die ändernde bleibt; Zurücksetzen durch den Autor oder die Befehlszeile löscht alle). Sperren
  (`active = 0`) löscht alle. Beides wirkt bei der nächsten Anfrage.

#### Sperre gegen Durchprobieren (`login_attempt`)

- Bereiche: `konto:<Namensschlüssel>` (immer, auch für unbekannte Namen; Namensschlüssel = ohne Rand, Leerraum
  einfach, klein, höchstens 40 Zeichen) und `adresse:<IP>` (nur mit bekannter Gegenstelle und nicht über den Proxy).
- Bis zum dritten Fehlversuch keine Wartezeit; ab dem vierten 2^(n−3) Sekunden (2, 4, 8 …), höchstens 900.
- Der Fehlversuch, der die Sperre auslöst, und jeder Versuch während der Sperre antworten **429** mit
  `Retry-After: <Sekunden>`; während der Sperre wird kein Passwort geprüft und nichts gezählt.
- Jeder Versuch wird **vor** der Prüfung des Passworts reserviert (`Store::login_reserve`, `throttle::reserve`):
  Prüfung der Sperre und Zählen als Fehlversuch sind eine Transaktion (`BEGIN IMMEDIATE`). Von beliebig vielen
  gleichzeitigen Versuchen kommen darum nur so viele bis zur Prüfung (Argon2id) wie nacheinander – bei leerem
  Zähler vier, der Rest bekommt 429. Auch ein Versuch, dessen Verbindung während der Prüfung abbricht, bleibt
  gezählt. (Bis zum Codereview nach Runde 35 wurde erst nach der Prüfung gezählt; eine Salve ging an der Sperre
  vorbei.)
- Erfolg nimmt die Reservierung zurück und setzt die Bereiche des Versuchs zurück. Einträge, die älter als 24 Stunden
  sind, fallen beim nächsten Versuch weg.
- Höchstens 1000 Zeilen: Ist die Tabelle voll (999 Bereiche), bekommt ein neuer Bereich keine Zeile; stattdessen
  wartet der Bereich `alle` fest 2 Sekunden – in dieser Zeit wartet jeder Anmeldeversuch. Weil vor der Prüfung
  gezählt wird, löst bei voller Tabelle auch eine gelungene Anmeldung aus einem neuen Bereich diese 2 Sekunden aus
  (der Erfolg setzt `alle` nicht zurück, sonst könnte ein Konto die gemeinsame Bremse für andere aufheben).
- Auch das bisherige Passwort beim Ändern (`/konto/passwort`, `account.set_password`) zählt in `konto:<…>`: Wer
  eine Sitzung in der Hand hat, kann das Passwort nicht beliebig oft raten.

#### Befehlszeile

`baruch-server passwort <name> [--data <Ordner>]` – Notweg bei vergessenem Passwort. Liest das neue Passwort als
**eine Zeile von der Standardeingabe** (nie von der Befehlszeile: dort stünde es in der Prozessliste), setzt es für
das vorhandene Konto und beendet alle Sitzungen des Kontos. Exit 0 mit „Passwort für „…“ gesetzt …“; unbekanntes
Konto (nennt die vorhandenen), ungültige Länge, Ordner ohne Datenbank oder noch kein Konto → Exit 1 mit Grund auf
stderr; ohne Namen Exit 2. Legt kein Konto an und entsperrt keines. Läuft auch neben dem laufenden Server.

`serve` gibt beim Start eine Zeile „Anmeldung: aus (keine Konten …)“ bzw. „Anmeldung: an (n Konten)“ aus.

#### Store (nur von `auth` gerufen)

`account_count`, `account_list`, `account_get`, `account_create` (Lektor nur, wenn es ein Autor-Konto gibt),
`account_create_first` (Prüfung „Tabelle leer“ und Einfügen in einer Anweisung: von zwei gleichzeitigen Aufrufen
gewinnt genau einer), `account_for_login`, `account_password`, `account_set_password(id, phc, keep)`,
`account_set_active` (letzter aktiver Autor bleibt – Prüfung und Änderung in einer Anweisung), `session_create`,
`session_account`, `session_delete`, `session_delete_all`, `login_reserve` (Sperre prüfen und Versuch zählen in
einer Transaktion; Ausgang `LoginAttempt`), `login_wait`, `login_failure`, `login_reset`,
`auth_local_open`, `set_auth_local_open`; Hilfen `Store::account_name_key`, `Store::session_token_hash`.
`lib.rs` reicht aus dem Modul nur `Account`, `NewAccount` und `LoginAttempt` heraus; Meldungen und Grenzen hängen darum als
zugehörige Konstanten an `Store` (`ACCOUNT_TAKEN`, `ACCOUNT_NOT_FIRST`, `ACCOUNT_NEEDS_AUTHOR`,
`ACCOUNT_LAST_AUTHOR`, `LOGIN_ATTEMPT_ROWS`, `LOGIN_FULL_WAIT`).

### Rollen (`roles`)

- **Kontoart global, Rolle je Werk.** `roles::role_in(state, who, work)`: volle Rechte (`Who::Open`,
  `Who::LocalAuthor`, Autor-Konto) → `Role::Author` in jedem Werk; Lektor-Konto → `Role::Lektor` nur mit Zeile in
  `work_member` (Migration 0054); sonst `None` (Werk unsichtbar).
- **Eine Stelle der Rechteprüfung:** das Modul `roles` (`crates/baruch-server/src/roles/mod.rs`).
  - `roles::check_request` – jede Route, aufgerufen aus `auth::gate`. Lektor: nur Treffer in `LEKTOR_ROUTES`
    (Methode + Muster), sonst 403 „Abgelehnt: …“ mit Kopfzeile `x-baruch-abgelehnt: rolle`. HEAD zählt als GET;
    andere Verben als GET, HEAD, POST sind für Lektoren gesperrt.
  - `roles::check_rpc` – jede RPC-Methode, aufgerufen aus `rpc::call` vor jedem Dispatch (auch für unbekannte
    Namen), also für `/rpc` (einzeln und Batch), `/act` und `/lektorat/act`. Lektor: nur Methoden aus `LEKTOR_RPC`,
    sonst `-32003` „Das darf nur der Autor: <methode>“.
  - Alles, was nicht in den Tabellen steht, ist für Lektoren gesperrt – auch Routen und Methoden künftiger Runden.
    Die Formulare der Module (`/beitraege/act`, `/werke/befunde/act`, `/faden/act` …) rufen ihr Modul direkt und
    stehen darum nicht in `LEKTOR_ROUTES`.
- **Werk-Bindung.** Das Werk steht im Pfad (`{id}`), in der Adresse (`?work=`) oder im RPC-Parameter `work`.
  - Ein nicht freigegebenes Werk wird mit derselben Meldung abgelehnt wie ein unbekanntes („unbekanntes Werk N“;
    403 bzw. `-32003`) – ein Lektor erfährt nicht, welche Werke es gibt.
  - Fehlt das Werk, gilt **nie** das Vorgabewerk der Instanz: RPC setzt das kleinste freigegebene Werk ein; GET-Seiten
    mit `WorkFrom::Query` leiten mit 303 auf dieselbe Adresse mit `work=<Werk>` um – das zuletzt geöffnete (Cookie
    `baruch_work`), wenn es freigegeben ist, sonst das kleinste freigegebene (`roles::check_request_preferring`, so
    ruft `auth::gate` auf); ohne freigegebenes Werk 303 auf `/werke` (RPC: `-32003` „Noch kein Werk freigegeben.“).
    Das Cookie gibt nie ein Recht. Für Seiten ohne eigenes Werk (`/werke`, `/konto`, `/ueber`) siehe „Integration“.
  - Ein `work` in der Adresse bzw. in den Parametern wird bei **jeder** erlaubten Route und Methode geprüft, auch wenn
    sie selbst kein Werk braucht (Werkwahl im Menüband). Die Adresse wird gelesen wie die Seiten sie lesen
    (prozentdekodiert; ein kodierter Name wie `wor%6B` zählt mit, mehrere `work` müssen alle freigegeben sein).
  - Ids (Beitrag, Befund, Faden, Bild, fester Schlüssel eines Abschnitts) sucht der Store immer im genannten Werk;
    eine id eines fremden Werks trifft unter dem freigegebenen nichts (Test
    `ids_of_a_foreign_work_yield_nothing_under_the_granted_work`).
- **Nichts wird gemerkt.** Jede Prüfung liest die Freigaben neu (`Store::member_works`, liefert nur für aktive
  Lektor-Konten etwas). Entzug der Freigabe und Sperre des Kontos wirken bei der nächsten Anfrage.
- **Sichtbarkeit.** `roles::visible_works` liefert dem Lektor nur freigegebene Werke (`work.list`, `/werke`,
  Werkwahl); `rpc::library_view` liefert Lektoren eine leere Literaturliste (keine Titel, keine Suche, keine Treffer,
  kein Bilderkasten) – für `ui.render` und die Seite `/`.
- **Herkunft.** Ein Lektor schreibt unter `@lektorat:<slug>` (`roles::author_name`), alle anderen weiter als „Autor“.
  - Vorschlag: `Store::add_proposal` mit Persona = Handle, Modell leer; Seitenzweig `refs/revisions/<key>/<n>`
    (`mirror::side_refs_after_proposal`) wie bei MCP-Vorschlägen, Trailer `Persona: @lektorat:<slug>`.
  - Beitrag: `post.author` = Handle; wie beim Autor kein Commit beim Anlegen. Bearbeiten/Löschen eigener Beiträge:
    ein Commit (`mirror::describe_post`), Autor des Commits ist der Handle, Committer „Autor“.
  - Befund: `finding.persona` = Handle; ein Commit „Befund <id>: angelegt von @lektorat:<slug>“
    (`mirror::describe_finding_added`, neu), Autor „<Anzeigename> (<Handle>)“
    `<lektorat-slug>@lektorat.baruch.invalid`, Trailer `Baruch-Finding`, `Baruch-Action: finding-add`,
    `Baruch-Node` (falls Abschnitt), `Baruch-Origin: lektor`, `Baruch-Lektor`.
- **Freigaben sind Instanzdaten:** Tabelle `work_member` (0054), nicht im Austauschformat, kein Commit im Spiegel;
  der Zeitpunkt steht in `granted`. Löschen eines Werks oder Kontos entfernt seine Freigaben (ON DELETE CASCADE).
- **Store (`baruch-store/src/members.rs`):** `member_list(work)`, `member_grant(work, account, role)` (nur
  Lektor-Konten, nur vorhandene Werke, idempotent – der Zeitpunkt der ersten Freigabe bleibt),
  `member_revoke(work, account)`, `member_works(account)` (aufsteigend; leer für gesperrte, unbekannte und
  Autor-Konten).

#### Erlaubte Routen für Lektoren (`roles::LEKTOR_ROUTES`)

| Methode | Muster | Werk aus | Begründung |
|---|---|---|---|
| GET | `/` | Adresse | Arbeitsseite: Abschnitt lesen, Vorschlag, Chat |
| GET | `/werke` | – | Werkliste, gefiltert über `roles::visible_works` |
| GET | `/beitraege` | Adresse | ganze Chat-Historie eines Abschnitts (ohne JavaScript) |
| GET | `/beitrag` | Adresse | ein Beitrag: kopieren, zitieren, eigenen bearbeiten |
| GET | `/faeden` | Adresse | Fäden lesen |
| GET | `/faden` | Adresse | einen Faden lesen (schreiben nicht: `/faden/act` gesperrt) |
| GET | `/suche` | Adresse | im Werk suchen (ersetzen nicht) |
| GET | `/gliederung` | Adresse | Gliederung lesen (ändern nicht: `/gliederung/act` gesperrt) |
| GET | `/werke/{id}/befunde` | Pfad | Befundseite, dort auch „Befund schreiben“ |
| GET | `/werke/{id}/fassungen/verlauf` | Pfad | Verlauf der Fortschreibungen lesen |
| GET | `/werke/{id}/glossar` | Pfad | Glossar lesen |
| GET | `/werke/{id}/register` | Pfad | Register lesen |
| GET | `/werke/{id}/bilder/{name}/datei` | Pfad | Bilder im Text der Arbeitsseite |
| GET | `/werke/{id}/bilder/{name}/vorschau` | Pfad | Vorschaubilder im Text |
| GET | `/ueber`, `/ueber/lizenz` | – | Über Baruch, Lizenz (AGPL: Hinweis auf den Quelltext) |
| GET | `/konto` | – | eigenes Konto (Passwort ändern, Abmelden) |
| GET | `/anmelden` | – | Anmeldeseite (nach dem Abmelden) |
| GET | `/einstellungen/darstellung` | Adresse | Darstellung im eigenen Browser (Cookie); alle anderen Einstellungsseiten gesperrt. **Abweichung:** mit Werk aus der Adresse statt „ohne Werk“ – ohne `work` zeigte das Menüband der Seite das Vorgabewerk der Instanz (Titel eines fremden Werks) |
| GET | `/static/app.css`, `/static/favicon.svg`, `/static/js/{name}` | – | Stil, Symbol, Widgets |
| POST | `/rpc` | – | JSON-RPC; jede Methode prüft `roles::check_rpc` |
| POST | `/lektorat/act` | – | Formularweg ohne JavaScript; ruft `rpc::call`, also dieselbe Prüfung |
| POST | `/abmelden` | – | Abmelden |
| POST | `/konto/passwort` | – | eigenes Passwort ändern |
| POST | `/einstellungen/darstellung/act` | – | Darstellung speichern (nur Cookies im eigenen Browser) |

Nicht erlaubt und darum 403: jeder Upload, `/act`, jedes Formular eines Moduls, jeder Export (PDF, EPUB, Word,
Austausch, Mindmap-Datei), Import, alle übrigen Einstellungen, Beenden, Literatur, Einfälle, Bilderseiten außer Datei
und Vorschau. `/mcp` bleibt bei Tokens und läuft nicht durch die Rollenprüfung.

#### Erlaubte RPC-Methoden für Lektoren (`roles::LEKTOR_RPC`)

„Werk“ = `work` aus den Parametern; fehlt es, wird das kleinste freigegebene eingesetzt.

| Methode | Werk | Art | Begründung |
|---|---|---|---|
| `work.get`, `node.get` | Werk | lesend | Werk und Abschnitt lesen |
| `revision.tree`, `revision.amends`, `revision.fold_get` | Werk | lesend | Fassungen, ihr Verlauf, Ausblendung |
| `ui.render` | Werk | lesend | Arbeitsseite ohne Neuladen (Literatur für Lektoren leer) |
| `menu.badges` | Werk | lesend | Zähler im Menüband |
| `post.page`, `post.since`, `post.versions` | Werk | lesend | Chat des Abschnitts |
| `finding.list`, `finding.get`, `finding.matrix` | Werk | lesend | Befunde |
| `thread.list`, `thread.get`, `thread.since`, `thread.post_versions` | Werk | lesend | Fäden lesen |
| `glossary.list`, `glossary.lookup`, `index.entries`, `index.overview` | Werk | lesend | Glossar und Register |
| `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`, `wysiwyg.style` | Werk | lesend | Editor für den Vorschlag |
| `ribbon.formats` | Werk | lesend | Formate der Vorlage im Menüband |
| `work.sheet` | Werk | lesend | **ergänzt:** Satzbild des Werks für den Editor (nur lesen; `work.sheet_set` bleibt gesperrt) |
| `search.find` | Werk | lesend | **ergänzt:** Suche im Werk (Seite `/suche` und Suchleiste); `search.preview/replace/undo` bleiben gesperrt |
| `work.list` | – | lesend | gefiltert auf freigegebene Werke |
| `account.me`, `account.set_password`, `account.logout_all` | – | eigenes Konto | Konto des Lektors (Baustein „konten“) |
| `revision.propose` | Werk | schreibend | Vorschlag einer Fassung |
| `post.add`, `post.edit`, `post.delete` | Werk | schreibend | Beitrag; bearbeiten und löschen nur eigene |
| `finding.add` | Werk | schreibend | Befund bzw. „Anmerkung zum Buch“ |

Abweichungen vom Start des Vertrags (§2.2), gemeldet (dazu die Route `/einstellungen/darstellung` in §2):

1. `wysiwyg.*` und `ribbon.formats` stehen mit Werk-Bindung statt „ohne Werk“. Grund: `rpc::dispatch` gibt ihnen ohne
   `work` das Vorgabewerk der Instanz (`wysiwyg.parse` liest dessen Bildtexte, `wysiwyg.style` dessen Satzbild,
   `ribbon.formats` die Formate dessen Vorlage) – das wäre für einen Lektor ein fremdes Werk. Folge: Ein Lektor ohne
   freigegebenes Werk bekommt für sie `-32003`.
2. Ergänzt wurden nur die beiden rein lesenden Methoden `work.sheet` und `search.find` (Begründung in der Tabelle).
   Keine davon fasst ein Modell, eine Verbindung oder das Dateisystem an. Nicht ergänzt (offen für den Integrator bzw.
   „oberflaeche_konten“, falls die Arbeitsseite sie für Lektoren aufruft): `bible.marks` und `scripture.check` (lesen
   SWORD-Module aus dem Dateisystem), `timing.count`, `xref.targets`, `outline.check`, `index.registers`,
   `work.typeset_get`.

### Oberfläche (`konten_ui`, `view.rs`, Vorlagen)

| Datei | Inhalt |
|---|---|
| `src/konten_ui/mod.rs` | `login_page`/`login_html`, `setup_page`/`setup_html`, `safe_next`, `MESSAGES` mit `ok_message`/`error_message`, `apply`, Marken `KONTO_MARK`, `LEKTOR_OPEN/CLOSE`, `KONTEN_OPEN/CLOSE`, eigene minijinja-Umgebung (die 38.) |
| `src/konten_ui/pages.rs` | Routen `GET /anmelden`, `/einrichten`, `/konto`, `/werke/{id}/mitwirkende`; Inhalt der Einstellungsseite „Konten“ (`render_settings`); reine Darstellungsfunktionen `accounts_html`, `members_html`, `account_html` über Ansichtsdaten; `direct_from_own_machine` (dieselbe Regel wie `auth::gate::own_machine`, über `guard::forwarded`) |
| `src/view.rs` | `values` liest `auth::current()`; für Lektoren eigene Vorlagen der drei Bereiche (`pane_templates`), `post.mine`, Beiträge ohne Befund-Karten, Herkunft `@lektorat:…` ohne leeres Modell |
| `src/consent.rs` | Hinweisleiste und Dialog nur bei vollen Rechten |
| `src/settings/mod.rs` | achte Seite `konten` in `PAGES`; Navigation des Rahmens für Lektoren nur „Darstellung“ |
| `templates/anmelden.html`, `einrichten.html` | eigenständige Seiten ohne Menüband und ohne Skript |
| `templates/konto.html`, `mitwirkende.html` | erweitern `base.html` (Reiter Start bzw. Buch) |
| `templates/settings/konten.html` | Inhalt der Einstellungsseite |
| `templates/fragments/lektor_tree.html`, `lektor_editor.html`, `lektor_assistants.html` | Arbeitsseite eines Lektors |
| `static/konten.css` | Regeln der Rollen und Gestaltung der neuen Seiten (nur Variablen aus `app.css`) |
| `web/src/settings.ts`, `menu.ts` | Lektor: „Einstellungen“ bleibt ein Link (kein `settings.render`), Pfeiltasten überspringen ausgeblendete Einträge |

#### `konten_ui::apply(page, who, accounts_exist)`

Läuft in `consent::inject` nach `band::apply` über jede Seite mit der Marke `<!--BARUCH-KONTO-->` (sie steht in
`base.html` im Menü „Baruch ▾“ nach „Baruch beenden…“). Seiten ohne Marke bleiben Byte für Byte gleich.

| Zustand | Menüeintrag | `<body>` | Kennzeichen |
|---|---|---|---|
| ohne Konten | „Konten einrichten…“ (`/einrichten`) | unverändert | ` nur-autor"` → `"`, ` data-nur-autor` entfernt, Stücke `NUR-LEKTOR` und `NUR-KONTEN` herausgeschnitten |
| Autor-Konto | „Angemeldet: <Anzeigename> (Autor)“, „Mein Konto“, Formular „Abmelden“ (`POST /abmelden`) | `data-rolle="autor"` | `NUR-LEKTOR` herausgeschnitten, `NUR-KONTEN` bleibt (ohne Klammern) |
| Lektor-Konto | wie oben mit „(Lektor)“ | `data-rolle="lektor"` | `NUR-LEKTOR` und `NUR-KONTEN` bleiben (ohne Klammern) |
| `LocalAuthor` | „Autor (eigener Rechner, ohne Anmeldung)“, „Anmelden“ | `data-rolle="autor"` | wie Autor-Konto |
| `Anonymous` | „Anmelden“ | unverändert | wie Autor-Konto |

Damit ist eine Seite ohne Konten bis auf den einen Menüeintrag dieselbe wie vor Runde 35. Geprüft mit einem
Byte-Vergleich von 23 Seiten und `ui.render` vor und nach der Änderung (Ziffern genullt): Unterschiede nur der
Menüeintrag, der achte Eintrag „Konten“ in der Navigation der Einstellungen und die Anzeige einer Herkunft
`@lektorat:…` (vorher „Persona @lektorat:erika ()“, jetzt „@lektorat:erika“ – ohne Konten kommt sie nicht vor).

Regel für Vorlagen: `nur-autor` ist immer die **letzte** Klasse eines Elements (`class="btn nur-autor"`), sonst
findet `apply` sie ohne Konten nicht. `apply` nimmt die Kennzeichen nur aus **Tags** heraus, nie aus Text zwischen
den Tags – Buchtext im Textfeld oder Satzbild bleibt unberührt, auch wenn er zufällig dieselben Zeichen enthält. Gruppen des Menübands tragen statt der Klasse das Attribut `data-nur-autor`
(ihr Tag muss wörtlich `<details class="band-group"` … ` data-band-group open>` bleiben, `band::apply`).

#### Was ein Lektor sieht

- **Arbeitsseite (serverseitig):** `view::page`/`view::panes` nehmen für Lektoren `fragments/lektor_*.html`. Darin:
  Formular `form#editor-form` an `POST /lektorat/act` mit `method=revision.propose`, `work`, `key`, `body`, `note`
  (mit JavaScript dieselbe Methode über `/rpc`, `data-rpc` in `workspace.ts`), Knopf „Vorschlagen“; rechts
  „Befund schreiben“ (`finding.add`: `work`, `key`, `kind`, `body`, `quote`), Beitrag (`post.add`: `work`, `key`,
  `body`), eigene Beiträge ändern/entfernen (`post.edit`: `id`, `body`; `post.delete`: `id`, `confirm`). Es fehlen
  im HTML: Speichern, Als neue Fassung, Übernehmen, Verwerfen, Umschalten („aktuell machen“, „Diese Fassung
  verwenden“), Löschen, Ausblenden, Personas und ihre Formulare, Lösungen, Verschieben/Umwandeln von Beiträgen,
  Bild einfügen, Markierung, Zitate prüfen, die Dialoge Literatur/Werk anlegen/Einfall, Reiter und Feld „Einfügen“,
  das Widget „outline“ (Umstellen der Gliederung) und das Widget „chat“ (siehe Annahmen). Befund-Blöcke in
  Beiträgen der Personas liest der Lektor als Text des Beitrags, ohne die Karte mit den Status-Knöpfen.
- **Übrige Seiten (CSS, ohne JavaScript):** `konten.css` blendet für `body[data-rolle="lektor"]` aus: `.nur-autor`,
  `[data-nur-autor]` und **jedes POST-Formular**, dessen `action` nicht `/lektorat/act`, `/abmelden`,
  `/konto/passwort` oder `/einstellungen/darstellung/act` ist (dieselbe Regel wie im Server: alles verboten, was nicht
  erlaubt ist – neue Formulare künftiger Runden sind für Lektoren von selbst verborgen). `.nur-lektor` ist nur dort
  sichtbar. Gekennzeichnet sind in `base.html`: Neues Werk/Import (3 Einträge), Austausch einlesen, Beenden, die
  Gruppen Einfälle, Werkzeuge, Quellen, Satz, Ausgabe, Umschlag und die Knöpfe Zitate prüfen, Querverweise, Werk
  bearbeiten, Anträge, Gliederungsschema, Bilder, Mitwirkende; in `works.html` die Gruppe „Werke“ und Ändern, Export,
  PDF, Löschen; auf der Befundseite die Formulare zu Status, Entscheidung und Lösungen (Makro `card(…, na)`).
- **Befundseite:** Formular „Befund schreiben: Anmerkung zum Buch“ (`finding.add` ohne `key`, `back=befunde`) als
  `nur-lektor`, nur für Lektoren im HTML.
- **Einstellungen:** In der Navigation steht für Lektoren nur „Darstellung“.
- **Zustimmung:** Hinweisleiste und Dialog (`consent`) nur bei vollen Rechten.

#### Seiten

| Route | Verhalten |
|---|---|
| `GET /anmelden[?next=…][&ok=abgemeldet]` | ohne Konten 303 `/`; sonst Formular `POST /anmelden` (`name`, `password`, `next`), Link „Weiter“ auf `safe_next(next)`; mit Sitzung „Angemeldet als …“ |
| `GET /einrichten` | mit Konten 303 `/einstellungen/konten`; nicht vom eigenen Rechner 403 mit Erklärung (ohne Formular); sonst Formular `POST /einrichten` (`name`, `display`, `password`, `password2`) |
| `GET /konto[?ok=…][&fehler=…]` | Name, Rolle, Herkunft, Werke (`roles::visible_works`), `POST /konto/passwort` (`current`, `password`, `password2`), `POST /abmelden`; ohne Konten Erklärung; `LocalAuthor` Hinweis |
| `GET /werke/{id}/mitwirkende[?ok=…][&fehler=…]` | nur volle Rechte (sonst 403), unbekanntes Werk 404; Daten aus `rpc::call("member.list", {work})`; Formulare `POST /werke/{id}/mitwirkende/act` (`action=grant|revoke`, `account`) |
| `GET /einstellungen/konten[?ok=…][&fehler=…]` | Liste aus `Store::account_list`, Schalter aus `account.me` (`local_open`); Formulare `POST /einstellungen/konten/act` mit `action=create` (`name`, `display`, `password`, `password2`), `deactivate`/`activate` (`id`), `reset_password` (`id`, `password`, `password2`), `local_open` (`open=0|1`); ohne Konten Erklärung und „Einrichten“ |

`?ok=`/`?fehler=` zeigen nur den Satz aus `MESSAGES` (ein Erfolgscode gilt nicht als Fehler und umgekehrt, Unbekanntes
zeigt nichts). Alle Konto-Seiten antworten mit `Cache-Control: no-store`.

### Annahmen und Abweichungen vom Vertrag

Konten:

1. **„Es gibt Konten“ wird nur als Ja gemerkt** (Vertrag §1.4 sagt: Ergebnis zwischenspeichern und nach dem Anlegen
   ungültig machen). Konten werden nie gelöscht, ein Ja bleibt wahr; das Nein wird je Anfrage nachgesehen (ein
   `SELECT count(*)` auf der leeren Tabelle). Grund: Ein gemerktes Nein ließe Anfragen ohne Anmeldung durch, wenn
   ein Konto an der Ablage vorbei entsteht (Tests der Kollegen legen Konten per SQL an; zweiter Prozess).
2. **Cookie erneuern** nach mehr als einer Stunde Pause (siehe Sitzung) – im Vertrag nicht genannt.
3. **Der vierte Fehlversuch antwortet selbst schon 429** mit `Retry-After: 2` (Abnahme: „ab dem vierten Fehlversuch
   429“), nicht erst der fünfte Versuch.
4. **`alle`** sperrt, solange es wartet, jeden Anmeldeversuch (auch bekannte Bereiche) für höchstens 2 Sekunden.
5. **Bisheriges Passwort beim Ändern** zählt in der Sperre des Kontos mit; während der Sperre lautet der Code
   `passwort_falsch` (die Tabelle der Meldungen hat keinen eigenen Code für „gesperrt“).
6. **`POST /einrichten` mit vorhandenem Konto** zeigt die Seite mit 403 und dem Satz „Es gibt schon ein Konto.“
   (kein Code der Tabelle §3.4); für Unangemeldete endet die Anfrage schon in der Schicht mit 401.
7. **`/einstellungen/konten/act` `create` ohne jedes Konto** leitet auf `/einrichten` (303): Ein Lektor ohne Autor
   wäre ein Zustand, in dem niemand volle Rechte hat; der Store lehnt ihn ebenfalls ab.
8. **Sperren des eigenen Kontos** ist einem Autor erlaubt, solange ein anderer aktiver Autor bleibt (in dieser Runde
   gibt es über die Oberfläche nur einen Autor – praktisch greift immer `letzter_autor`).
9. **`resolve` nimmt `&http::request::Parts`** statt `&Request` (schon in S1 so festgelegt und begründet).

Rollen:

- **Kein Zähler für offene Vorschläge:** MCP begrenzt offene Vorschläge je Client; für Lektoren (Menschen) gibt es
  keine solche Grenze, nur die Größen (1 MiB Text, 2000 Zeichen Notiz).
- **Lektoren und der Faden-Chat:** lesen ja, schreiben nein (kein Modellauftrag, Vertrag §0.7).
- **`/lektorat/act` für den Autor:** nimmt nur Methoden aus `LEKTOR_RPC`; `revision.propose`/`finding.add` lehnt der
  Server für ihn mit Hinweis ab. Der Autor nutzt wie bisher `/act` und die Formulare der Module.

Oberfläche:

2. **`account.me`** liefert `local_open` (Vertrag §1.6); scheitert der Aufruf, zeigt die Seite „ausgeschaltet“.
3. **`member.list`** in der Form des Vertrags (§2.3); eine unlesbare Antwort zeigt die Seite als Fehler.
4. **`/einstellungen/konten/act`** bekommt genau die Felder des Vertrags – kein `work`. Nach der Aktion fehlt das
   Werk in der Adresse (es kommt dann aus dem Cookie `baruch_work`).
5. **`/lektorat/act`** leitet nach `revision.propose`, `post.*` und `finding.add` zurück auf den Abschnitt
   (`view::node_url`) bzw. mit `back=befunde` auf die Befundseite. Die Lektor-Formulare der Arbeitsseite schicken
   kein `back`.
6. **`revision.propose`** über `/rpc` antwortet mit `{key, rev, …}`; `workspace.ts` zeigt danach diese Fassung.
7. **Live-Chat:** Für Lektoren fehlt das Widget „chat“ (neue Beiträge erscheinen nach dem Neuladen): `post.since`
   liefert Beiträge mit den Werkzeugen des Autors (`post_entry`), die ein Lektor nicht nutzen kann.
8. **Seiten `/beitrag` und `/beitraege`:** Ein Lektor liest dort; seine eigenen Beiträge ändert und entfernt er nur auf
   der Arbeitsseite. Die Formulare des Autors sind dort über die CSS-Regel für POST-Formulare verborgen.
9. **Anzeigename bei der Herkunft:** Fassungen, Beiträge und Befunde zeigen `@lektorat:<slug>` (wie `@word:<name>`).
   Den Anzeigenamen dazu gibt es auf „Mein Konto“, „Mitwirkende“ und in den Einstellungen – auf der Arbeitsseite
   nicht (die Ansichtsdaten `LibraryView` kennen keine Konten; Besitzer `rpc.rs`).
10. **Links auf gesperrte Seiten** (z. B. Umschlagbild in der Werkliste, „Zitate prüfen“ in älteren Seiten) können für
    Lektoren stehen bleiben, wo sie nicht in `base.html` liegen; sie enden mit 403 „Abgelehnt: …“.

### Angepasste ältere Tests

| Datei | Warum |
|---|---|
| `tests/round9_settings.rs`, `tests/round9_integration.rs`, Unit-Test in `settings/mod.rs` | acht statt sieben Einstellungsseiten (Vertrag §3.1) |
| `tests/round24_pakete.rs` | hinter „Baruch beenden…“ folgt der Eintrag zum Konto (Vertrag §3.2) |
| `tests/round32_abnahme.rs` | die Einträge „Word (.docx) importieren…“ und „Baruch beenden…“ tragen in `base.html` die Klasse `nur-autor`; hinter der Marke `R32-PLATZ beenden` folgt die Marke für den Eintrag zum Konto |
| `tests/round33_laptop.rs` | `konten.css` steht in `STYLE` hinter `laptop.css` (S1); der Test verlangt jetzt, dass `laptop.css` hinter dem Grundgerüst steht und danach nur `konten.css` folgt (das die Zustände des Laptop-Modus nicht anfasst – geprüft in `round35_oberflaeche`) |
| `tests/round33_seiten.rs` | `konto.html` und `mitwirkende.html` in `TEMPLATES` |

### Integration (Runde 35)

- **Zusammengeführt** `team/konten`, `team/rollen`, `team/oberflaeche_konten` ohne Konflikt. Die drei Zweige hatten
  je gegen die Stubs der anderen gebaut; die Naht prüft `tests/round35_abnahme.rs` mit echten Sitzungen (kein
  `common::with_who`): Einrichten nur vom eigenen Rechner (nicht über den Proxy-Namen, nicht ohne Gegenstelle, nicht
  mit `X-Forwarded-For`), danach Seiten 303 auf `/anmelden?next=…`, `/rpc` 401 mit −32002, Formulare 401, `/mcp`
  unverändert; viermal falsches Passwort → 429 mit `Retry-After`, auch für das richtige; Abmelden beendet genau eine
  Sitzung; kein Passwort und kein Token im Datenordner (alle Dateien durchsucht). Die Lektorin (Name mit Umlaut →
  `@lektorat:baerbel`) sieht nur das freigegebene Werk, schreibt Vorschlag, Beitrag und Befund über
  `/lektorat/act` (ohne JavaScript) mit Herkunft im Spiegel, darf nicht übernehmen (403 über `/act`, `/lektorat/act`,
  −32003 über `/rpc`); der Autor übernimmt über `/act`; nach dem Entzug der Freigabe 403. Dazu mit echter Sitzung jede
  Route aus `common::registered_routes()` und jeder Methodenname aus `common::rpc_method_candidates()` außerhalb der
  beiden Tabellen: 403 bzw. −32003, Datenbank (Zeilenzahlen) und Spiegel (HEAD) danach unverändert.
- **Ohne Konten:** Die Startseite ist bis auf den Eintrag „Konten einrichten…“ (direkt hinter „Baruch beenden…“) die
  von vorher: Der Test schneidet den Eintrag heraus und findet danach nichts aus Runde 35 mehr. Ein Vergleich Byte für
  Byte gegen ein Programm vom Stand vor S1 steht im Bericht von oberflaeche_konten (23 Seiten und `ui.render`); der
  Integrator hat ihn nicht wiederholt.
- **Zuletzt geöffnetes Werk (Vertrag §2.1, bei rollen offen):** `roles::check_request_preferring(…, preferred)` nimmt
  das Werk aus dem Cookie `baruch_work`, wenn es freigegeben ist; `check_request` (S1-Signatur) bleibt und ruft es
  ohne Wunsch.
- **Seiten ohne eigenes Werk (Sichtprüfung):** `/werke` verlinkte für Lektoren „Zurück zum Werk“ auf das Vorgabewerk
  der Instanz (403), und mit einem alten Cookie `baruch_work` zeigte das ganze Menüband auf ein fremdes Werk. Zwei
  Stellen, beide zentral: `AppState::default_work` liefert einem Lektor das kleinste **freigegebene** Werk (ohne
  Freigabe `UnknownWork(0)`), und `auth::gate` richtet für Lektoren das Cookie `baruch_work` **der Anfrage** auf ein
  freigegebenes Werk (`pin_last_work`; das Cookie im Browser bleibt, ohne Freigabe fehlt es). Titel eines fremden
  Werks standen auch vorher nicht in der Seite.
- **Kopf eines Vorschlags:** „@lektorat:baerbel (@lektorat:baerbel) · Fassung 2“ nannte den Handle doppelt (ein Konto
  hat keinen Persona-Eintrag); jetzt „@lektorat:baerbel · Fassung 2“ (`view.rs`). Der Anzeigename steht dort weiter
  nicht (Backlog).
- **Befundseite:** Das Formular „Anmerkung zum Buch“ stand wegen `.fd-form` (befunde.css) am rechten Rand;
  `konten.css` richtet es als Spalte aus.
- `konten_ui::pages::direct_from_own_machine` nutzt `guard::forwarded` (eine Liste der sechs Kopfzeilen statt zwei);
  der S1-Text `accounts::NOT_YET` und die Ausnahmeliste `pending` in `round35_schreibwege` sind entfernt.
- **Widgets der Arbeitsseite eines Lektors** (Sorge von rollen: Aufrufe gesperrter Methoden): Die Seite trägt nur
  `workspace`, `editor`, `wysiwyg`, `tree`, `accordion`, `splitter`, `band`, `band-dock`, `menubar`, `settings`,
  `theme`, `work-select`. Beim Laden kommt kein RPC-Aufruf; „Vorschlagen“ mit JavaScript ruft `wysiwyg.serialize`,
  `revision.propose`, `ui.render`, `menu.badges` – alle erlaubt (Sichtprüfung mit Probe, Mitschrift am Vermittler).
  `timing`, `scripture-check`, `context-menu` stehen für Lektoren nicht in der Seite; `bible.marks`, `timing.count`,
  `index.registers` usw. bleiben darum zu Recht gesperrt.
- **Bestätigt (PM-Frage von rollen):** `wysiwyg.*`, `ribbon.formats` und `GET /einstellungen/darstellung` mit
  Werk-Bindung; `work.sheet` und `search.find` als rein lesende Ergänzungen. Folge: Ein Lektor ohne freigegebenes Werk
  erreicht die Darstellungsseite nicht (Umleitung auf `/werke`).
- **Bausteine eingesetzt:** Die Dateien `docs/r35/*.md` der Entwickler sind in diesen Abschnitt, in `docs/RPC.md`
  und `docs/MCP.md` aufgegangen und gelöscht.
- **Fotos der Sichtprüfung:** `.claude/sicht/r35/` (Einrichten, Anmelden, Konten, Mitwirkende, Menü und Arbeitsseite
  als Autor und als Lektor, Befundseite, Mein Konto, Ablehnung; hell und dunkel, mit und ohne JavaScript, 1024 px).

### Tests

- `crates/baruch-store/tests/migration_0053.rs`, `migration_0054.rs`: Migrationen auf nachgebauter Datenbank im Stand
  0052 (alle Zeilen byte-gleich), Store-Regeln (erstes Konto nur einmal, letzter Autor, Sitzungen, Sperre, Freigaben).
- `crates/baruch-server/tests/round35_konten.rs` (Anmeldung, Sitzung, Sperre, Verwaltung, Befehl `passwort`),
  `round35_rollen.rs` (Werk-Bindung, Herkunft, eigene Beiträge, Befunde, Freigaben), `round35_schreibwege.rs` (jede
  Route und Methode außerhalb der Tabellen), `round35_oberflaeche.rs` (Seiten, Menü, Arbeitsseite, Stylesheet),
  `round35_abnahme.rs` (der echte Weg mit Sitzungen, siehe „Integration“).
- Testhilfen in `tests/common/mod.rs`: `registered_routes()`, `rpc_method_candidates()`, `with_who(request, who)`.

### Handprüfung für den Nutzer (echter Browser, Proxy mit TLS)

Vorher eine Sicherung anlegen oder mit einem eigenen Datenordner prüfen (`--data`).

**Ohne Konten**

1. Baruch öffnen: alles wie bisher? Im Menü „Baruch ▾“ steht neu „Konten einrichten…“.
2. Einstellungen → Konten: Erklärung und Knopf „Einrichten“.

**Einrichten (am Rechner selbst, `http://127.0.0.1:8765`)**

3. „Konten einrichten…“: Formular erscheint. Zwei verschiedene Passwörter → Meldung, Name bleibt stehen.
4. Konto anlegen → Einstellungen → Konten mit „Das Konto ist angelegt.“; Menü zeigt „Angemeldet: … (Autor)“.
5. Über den Proxy (`https://NAME/einrichten`) bzw. in einem privaten Fenster: Umleitung auf die Konten bzw. Anmeldung.
6. Passwortmanager des Browsers: bietet er beim Anlegen das Speichern und beim Anmelden das Ausfüllen an?

**Autor**

7. Abmelden → Anmeldeseite mit „Abgemeldet.“; falsches Passwort viermal → Wartezeit wird genannt.
8. Lektor anlegen (Einstellungen → Konten), Werk freigeben (Reiter Buch → Mitwirkende), wieder entziehen.
9. Sperren und Entsperren; Passwort zurücksetzen (aufklappbares Formular in der Zeile).
10. „Am eigenen Rechner ohne Anmeldung“ ein- und ausschalten: Menü zeigt „Autor (eigener Rechner, ohne Anmeldung)“.
    **Über den Proxy muss trotzdem die Anmeldeseite kommen.**
11. Dunkles Thema und Laptop-Modus: Anmeldeseite, Mein Konto, Konten, Mitwirkende lesbar, nichts läuft aus dem Fenster.

**Lektor (zweiter Browser oder privates Fenster, über den Proxy)**

12. Anmelden: nur das freigegebene Werk in der Werkwahl und unter „Alle Werke“; ohne Freigabe „Noch kein Werk
    freigegeben.“.
13. Arbeitsseite: kein „Speichern“, kein Reiter „Einfügen“, keine Assistenten rechts; im Menü „Baruch ▾“ nur Alle
    Werke, Einstellungen, Über Baruch, Mein Konto, Abmelden.
14. Text ändern → „Vorschlagen“ (mit Notiz): Der Vorschlag erscheint als neue Fassung „Vorschlag · @lektorat:…“.
    Dasselbe mit abgeschaltetem JavaScript.
15. Beitrag senden; eigenen Beitrag unter „Verwalten“ ändern und entfernen; beim Beitrag des Autors fehlt „Verwalten“.
16. „Befund schreiben“ rechts; auf der Seite Befunde „Anmerkung zum Buch schreiben“; dort keine Status-Knöpfe.
17. Adresse von Hand: `/werke/<id>/pdf`, `/einstellungen/werk`, `/literatur` → „Abgelehnt: …“ (403).
18. „Einstellungen“ führt auf Darstellung (kein Dialog); Thema umschalten wirkt.
19. Als Autor: Vorschlag des Lektors übernehmen bzw. verwerfen; im Git-Spiegel steht die Herkunft.
20. Link auf eine Baruch-Seite aus einer E-Mail anklicken, während man angemeldet ist: Anmeldeseite mit „Schon
    angemeldet? Weiter“ → der Klick führt ohne neue Anmeldung ans Ziel.

**Notweg**

21. `echo 'neues-langes-passwort' | baruch-server passwort <Name> --data <Ordner>`; danach anmelden.
22. `curl -i -X POST <Adresse>/rpc -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"work.list"}'`
    ohne Cookie → 401 mit Code −32002; `/mcp` verhält sich wie vor Runde 35 (Token bzw. eigener Rechner).
23. `sqlite3 'file:<Ordner>/baruch.sqlite?mode=ro' 'select password from account; select token_hash from
    account_session'` → nur `$argon2id$v=19$m=19456,t=2,p=1$…` und 64 Hex-Zeichen.
24. Als Lektor mit zwei freigegebenen Werken: im zweiten Werk arbeiten, dann „Baruch ▾ → Alle Werke“ und „Zurück zum
    Werk“ → das zuletzt geöffnete Werk, nie ein fremdes.

## Runde 36 – Reste

Kleine, klar abgegrenzte Punkte aus den Runden 33 bis 35, aus der Gegenprobe 0.9.0 und aus der Liste „offen nach
R24“ (Aufgabe `reste`; Bedienung in `docs/Bedienung.md`, „Runde 36 – Reste“). Keine neue Route, keine neue
RPC-Methode, kein neues MCP-Werkzeug, keine Migration (die reservierte Nummer 0055 bleibt unbenutzt), keine neue
Abhängigkeit. Neues SQL steht nur in `crates/baruch-store/src/reste36.rs`, als Zeichenketten-Literale mit gebundenen
Werten.

### Konten und Rollen

- **Notweg `passwort`** (`auth/cli.rs`): Nach `account_set_password` ruft `reset_password`
  `Store::login_reset(&throttle::account_scope(name))` auf. Der Bereich hängt am Namensschlüssel des Kontos; der
  Bereich der Adresse und die Bereiche anderer Namen bleiben. Ein abgelehnter Aufruf (zu kurzes Passwort,
  unbekanntes Konto) ändert nichts.
- **Werk-Bindung gelockert** (`roles/mod.rs`): `GET /einstellungen/darstellung` in `LEKTOR_ROUTES` sowie
  `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`, `wysiwyg.style` und `ribbon.formats` in `LEKTOR_RPC` stehen
  auf `WorkFrom::None`. Das trägt, weil alle Wege zu einem Werk ohne ausdrückliches `work` für einen Lektor nur
  Freigegebenes liefern: `AppState::default_work` gibt ihm das kleinste freigegebene Werk und ohne Freigabe keines;
  `settings::resolve_work` lässt das Cookie `baruch_work` für einen Lektor nur gelten, wenn `roles::role_in` das Werk
  als freigegeben kennt (vorher hätte die Seite ohne `work` das zuletzt geöffnete Werk aus dem Cookie genommen). Ein
  in der Adresse oder im Parameter genanntes `work` prüft `roles` wie bisher. `roles` bleibt die einzige Stelle der
  Rechteprüfung; die Änderung in `resolve_work` wählt nur aus, was gezeigt wird. Beleg: `round36_lektor`
  (Vorgabewerk nie fremd, Seite ohne Freigabe erreichbar und ohne fremden Titel, Methoden ohne `work`).
- **Anzeigename zur Herkunft** (`konten_ui/names.rs`): Ein Nachbearbeitungsschritt über das fertige HTML, an zwei
  Stellen eingehängt – `consent::inject` (ganze Seiten) und `ui.render` (die drei Stücke für das Nachladen). Er
  liest `Store::account_list`, bildet je Konto `@lektorat:<slug>` → Anzeigename und ersetzt nur den **Text** der
  Herkunfts-Elemente durch „Anzeigename (@lektorat:slug)“; Attribute (`data-post-author` …) und alles Gespeicherte
  bleiben der Handle. Ohne Konten und ohne Treffer gibt er die Seite unverändert zurück (Byte für Byte, Test).
  Gewählt wurde die Nachbearbeitung, weil `LibraryView` und die Vorlagen keine Konten kennen und der Store in dieser
  Runde nicht umgebaut wird.
- **Beschriftung und Links für Lektoren:** `base.html` trägt den Knopf der Suche zweimal (`nur-autor` „Im Werk
  suchen und ersetzen“, `nur-lektor` „Im Werk suchen“); `konten_ui::apply` lässt wie bei den übrigen Paaren nur den
  passenden stehen. `works.html` zeichnet das Umschlagbild für Lektoren ohne Link (`lektor` im Kontext von
  `works::list_page`). Beides ist Bequemlichkeit; gesperrt wird weiter allein in `roles`.
- **Thema auf `/anmelden` und `/einrichten`** (`konten_ui::themed`, eingehängt in `consent::inject`): Die beiden
  eigenständigen Seiten (ohne Menüband, erkannt am Kopf der Vorlage und an `body.konto-seite`) bekommen
  `data-theme` aus dem Cookie `baruch_theme` an `<html>`, wie `base.html`. Ohne Cookie oder mit einem unbekannten
  Wert bleibt die Seite unverändert (Farbschema des Systems). Kein JavaScript.

### Mindmap

- **Rundreise mit Wolke:** `files/import.rs` erkennt den eigenen Ast an der Kennung `export::CLOUD_ID` (nie am
  Titel), löst ihn auf und gibt seinen Kindern `ImportNode::cloud = true` (`tree.rs`, `#[serde(default)]`, damit
  eine ältere Vorschau im Formular weiter gilt). Beim Anlegen ohne Zielknoten legt der Store die obersten Knoten
  wie bisher als Hauptthemen an (`idea_import_tree`, eine Store-Datei von sqlx09 in dieser Runde); danach verschiebt
  `tree.rs` die gekennzeichneten mit `Store::idea_move` in die Wolke – unter derselben Schreibsperre und vor dem
  einen Spiegel-Commit. Mit Zielknoten zählt das Kennzeichen nicht, tiefer im Baum auch nicht.
- **Beschriftungen der Verbindungen** (`bild/mindmap.rs`, `free_label_spot`): Jede Beschriftung ist ein Rechteck
  (geschätzte Textbreite plus Rand). Gesucht wird ein Platz, der kein Knotenrechteck und keine schon gesetzte
  Beschriftung überschneidet (3 px Abstand) und im Bild liegt: zuerst an sieben Stellen der Kurve mit wachsendem
  Abstand darüber und darunter, dann in Ringen um die Mitte der Linie. Findet sich nichts, bleibt sie an der Mitte
  und trägt `data-crowded`. Dazu ein deckender Grund (`rect.mm-link-label-bg`, in der Datei-Ausgabe weiß), damit
  kreuzende Linien sie nicht durchstreichen. Grenze: Die Textbreite ist geschätzt, der Server kennt die Schrift des
  Browsers nicht.
- **Heller Streifen im dunklen Modus** (`static/karte.css`): `.karte` ist der Scrollbehälter der Seite und war auf
  1400 px begrenzt und zentriert; ihre Bildlaufleiste stand in breiten Fenstern darum neben der Bühne statt am
  Fensterrand, in den Farben des Systems. Jetzt reicht `.karte` über die volle Breite und rückt den Inhalt über
  `padding-inline` in die Mitte; `.karte` und `.mm-canvas` setzen `scrollbar-color` aus den Variablen der Seite.
  Die Ursache ist aus dem Stylesheet abgeleitet und dort zugesichert; im Browser angesehen ist es nicht
  (Handprüfung beim Nutzer).

### Fassungen und Assistenten

- **Zähler im Band** (`fragments/edit_toolbar.html`): derselbe Schalter wie im Editor-Kopf aus `rev.fold`
  („(+ n ausgeblendet) ▸“ bzw. „n ältere verbergen ▾“, gleiche Adressen); ‹ und › tragen `rev.fold.q` weiter.
- **Eigene offene Befunde nur beim Prüfen** (`assist/chat.rs`, `is_check_order`): `OrderFrame::recheck` einer
  direkt angesprochenen Persona ist `rfc || is_check_order(message)`. Ein Prüfauftrag ist eine Nachricht, in der
  ein Wort (außerhalb von Erwähnungen und Zitatzeilen) mit prüf, überprüf, nachprüf, gegenprüf, kontrollier, check
  oder review beginnt, oder die einen Befund „[B<n>]“ nennt. „Prüfen“, ↻ und RFC bleiben erneutes Prüfen. Die
  Erkennung ist eine Wortliste, kein Verstehen: „geprüft“ zählt nicht, „Checkliste“ zählt.
- **`propose_chapter` auf der richtigen Grundlage** (`assist/mod.rs`, `assist/basis.rs`): Der Vorschlag für den
  Teil des Auftrags entsteht über `Store::add_proposal_on` auf `prepared.shown_rev` (gewählt bzw. gezeigt), die
  übrigen Teile wie bisher auf ihrer aktuellen Fassung. `Outcome::basis` kommt aus `basis::of_proposal` (Kopfzeile
  „Grundlage: …“) und steht in der Antwort von `persona.propose` – auch beim Vorschlag für einen Abschnitt. Ein
  Text, der der gezeigten Fassung gleicht, zählt als unverändert.
- **`baruch_vw` beim ersten Aufruf** (`web/src/vw.ts`, `base.html`): ein klassisches Skript im `<head>` vor dem
  Modul `main.js`. Es läuft vor dem ersten Zeichnen, vergleicht die Breitenklasse des Fensters mit dem Cookie,
  schreibt bei Abweichung das Cookie neu und setzt die Attribute an `<html>` nach derselben Formel wie
  `laptop.rs` und `laptop.ts` (Schwellen 1280/1600). Stimmt das Cookie, tut es nichts. Der Server kann die Breite
  bei der ersten Anfrage nicht kennen; ohne JavaScript bleibt es bei „breit“. Ausgeliefert wird `vw.js` über die
  vorhandene Route `/static/js/{name}`; `build.rs` übersetzt es wie die übrigen Dateien unter `web/src`.

### Einstellungen, Editor, Spiegel

- **„Sicherung angelegt“** (`settings/data.rs`): ein prozessweiter Vermerk je Datenordner
  (`static BACKUP_NOTES: Mutex<BTreeMap<PathBuf, String>>`, kein Feld in `AppState`). Das Formular legt ihn nach
  einer gelungenen Sicherung ab, die Seite mit `?ok=sicherung` holt ihn genau einmal ab. Nach einem Neustart ist er
  weg; `settings.backup` über RPC antwortet wie bisher direkt.
- **Rechte des Datenordners** (`settings::open_permissions`, `permissions_warning`): nur lesend
  (`metadata().permissions().mode() & 0o777`, gemeldet wenn `& 0o077 != 0`), nur unter Unix. Baruch ändert die
  Rechte eines vorhandenen Ordners nie.
- **Token-Liste** (`settings/mcp.html`, `settings.css`): Klasse `token-former` mit `white-space: nowrap`.
- **Knopf „Zitat“** (`ribbon::QUOTE_SNIPPET`): `#quote(block: true)[]\n` ohne Folgezeile „– “.
- **`outline_mentions`** (`xref.rs`): Ordnungszahlen bis `MAX_ORDINAL` = 100 aus Stämmen gebildet und am ganzen
  Stamm verglichen („achtzehnten“ ist 18, nicht 8); römische Zahlen bis 3999, nur in üblicher Schreibweise. Die
  Regel „nur bei Eindeutigkeit“ ist unverändert.
- **store-5** (`baruch-store/src/reste36.rs`, `outline.rs`, `mirror.rs`): `Store::outline_delete_losses` liest alle
  Fassungen außer der aktuellen (`proposal` für `status = 'proposed'`) und alle Beiträge des Punkts. Der Server
  ruft es unter der Schreibsperre unmittelbar vor `outline_delete` auf – beim Löschen durch den Menschen und beim
  Bestätigen eines Antrags „Löschen“ – und gibt das Ergebnis an `mirror::record_outline_losses`: derselbe eine
  Commit, im Rumpf zusätzlich jede mitgelöschte Fassung und jeder Beitrag im Wortlaut, dazu die Trailer
  `Baruch-Lost-Revisions` und `Baruch-Lost-Posts`. Ohne Verluste ist der Commit wie bisher. Ein Antrag allein
  löscht nichts; löschen kann nur der Mensch.

### Grenzen und Ehrliches

- Kein Browser in dieser Runde: Token-Liste, heller Streifen, Lage der Beschriftungen im echten Schriftbild und
  das frühe Skript `vw.js` sind über Markup, Stylesheet, erzeugtes SVG und das Node-Harness geprüft, nicht gesehen.
- Der Vermerk „Sicherung angelegt“ gilt je Prozess und Datenordner, nicht je Sitzung: Bei mehreren Menschen an
  einer Instanz sieht ihn, wer die Seite zuerst mit `?ok=sicherung` lädt.
- Nicht bearbeitete Punkte stehen mit Grund in `docs/Backlog.md` („R36: nicht bearbeitet, …“).

### Tests

- `crates/baruch-store/tests/reste36.rs`: `outline_delete_losses` (übrige Fassungen, Beiträge, nur lesend,
  unbekanntes Werk und unbekannter Punkt).
- `crates/baruch-server/tests/round36_lektor.rs` (Werk-Bindung, Anzeigename, „Im Werk suchen“, Umschlagbild,
  Thema), `round36_mindmap.rs` (Rundreise `.mm`/`.xmind`, fremder Ast „Wolke“, Beschriftungen im SVG, Stylesheet),
  `round36_reste.rs` (Band-Zähler, `vw.js` am Server und im Node-Harness, Sicherung, Rechte, Token-Liste, Zitat,
  Nennungen, Spiegel-Commit beim Löschen direkt und über einen bestätigten Antrag).
- In vorhandenen Dateien: `round35_konten.rs` (Notweg hebt die Sperre auf), `kritik_zyklus.rs` (Vorschlag fürs
  Kapitel auf der gezeigten bzw. gewählten Fassung), `round22_befunde.rs` (schlichte Frage ohne eigene Befunde,
  Prüfauftrag mit).

### Typst-Kommandozeile (Aufgabe `typst_alias`)

`main` entscheidet vor dem Start der Tokio-Laufzeit über `typst_cli::dispatch(args)`: Heißt das Programm `typst`
oder `baruch-typst` (Dateiname von argv[0], ohne `.exe`, Groß/Klein egal) oder ist das erste Argument `typst`, läuft
`typst_cli::run` und der Prozess endet mit dessen Exit-Code (0 Erfolg, 1 Satz- oder Dateifehler, 2 falscher Aufruf
oder Nicht-Unterstütztes) – ohne Datenordner, Datenbank, Server oder Netz. `baruch-server/src/typst_cli.rs` liest
nur die Argumente und schreibt die Dateien (erst nach gelungenem Satz); der Satz steht in
`baruch-render/src/cli.rs` (`compile_file`, `font_names`) als Kindmodul von `baruch-render`, das Schriften,
Bibliothek und die deutschen Meldungen des Servers mitbenutzt. Die Welt `Disk` liest Dateien nur unter dem
Wurzelordner (aufgelöst, Symlinks nach außen gesperrt) und lehnt Pakete mit `OFFLINE` ab. `TYPST_VERSION` folgt dem
Crate `typst` in `Cargo.lock` (Tests und `scripts/ci-typst.sh` halten das fest). Die Pakete liefern nur den Namen
`baruch-typst` (Symlink bzw. `.cmd`). Bedienung und Grenzen: `docs/Verteilung.md`, „Typst-Kommandozeile“.

### Datenbankzugriff mit sqlx 0.9 (Aufgabe `sqlx09`)

Store, Bibliothek und Server nutzen eine gemeinsame sqlx-Ausprägung 0.9 (SQLite gebündelt über libsqlite3-sys 0.40;
der Server zusätzlich PostgreSQL mit rustls/ring und Systemzertifikaten für pgvector). `query`, `query_as`,
`query_scalar` und `begin_with` nehmen nur noch `&'static str` oder einen mit `sqlx::AssertSqlSafe` bestätigten
Text. Regel (Modulkommentar von `baruch-store/src/lib.rs`): SQL wo möglich als Literal; `AssertSqlSafe` nur für Text
aus festen Bausteinen des Quelltexts (Spaltenlisten, Platzhalter-Ketten, feste Bedingungen, Tabellennamen aus
Literalen, bei pgvector geprüfte Bezeichner über `quoted()`); Werte werden immer gebunden. Die Migrationen und ihre
Prüfsummen in `_sqlx_migrations` sind gegenüber 0.8 unverändert (Tests `sqlx09_migrations` in Store und Bibliothek,
`round36_sqlx` im Server). Neu gegenüber 0.8: sqlx-sqlite 0.9 beachtet `-- no-transaction` am Kopf einer Migration;
keine Migration von Baruch trägt die Markierung, 0040 braucht weiter `baruch_store::migrate` mit `FOREIGN_KEYS_OFF`.

### Abnahme der Integration

`crates/baruch-server/tests/round36_abnahme.rs` (Integrator) prüft über die gebaute Binärdatei die Naht zwischen
Export und Typst-Kommandozeile: Import einer erfundenen Typst-Datei mit Bild, `export --out` und `export --pdf`,
dann `baruch-typst compile` (Symlink) und `baruch-server typst compile` auf der exportierten Datei – alle drei PDF
sind Byte für Byte gleich (Werk ohne Umschlag und Titelei; mit Umschlag legt der Export seinen Rahmen um die
Quelle, die Kommandozeile setzt nur die Datei). `watch` endet mit Exit 2 und schreibt nichts.

Aus der Sichtprüfung der Integration: Die Kopfzeile eines Vorschlags im Lesefluss (`pv-label`) nannte als einzige
Stelle der Arbeitsseite nur den Handle; `konten_ui::names::ORIGIN_CLASSES` nimmt die Klasse jetzt mit
(`round36_lektor`). Im Editor-Kopf wird der Chip mit Anzeigename und Handle bei schmaler Mittelspalte mit „…“
gekürzt (`.editor-head > .rev-meta`, wie bisher jede lange Herkunft) – im Backlog.

## Hilfe auf der Webseite (Runde 37)

Die Bedienung steht nicht mehr in `docs/`, sondern auf der Webseite (Hugo, `site/`):

- **Quelle:** `site/content/de/hilfe/*.md` (Anwender, 14 Seiten, Einstieg `_index.md` mit `aliases: ["/bedienung/"]`),
  `site/content/de/betrieb.md` (Betreiber), `installation.md` und `mcp.md` (Ersteinrichtung). `docs/Bedienung.md` und
  `docs/Betrieb.md` tragen nur noch „Wo steht was“ und technische Anmerkungen.
- **Theme:** `site/themes/baruch/layouts/section.html` (Startseite der Hilfe), `layouts/hilfe/page.html` (Navigation
  des Abschnitts, Inhaltsverzeichnis, vor/zurück), Shortcodes `hilfebild` (Bild aus `static/bilder/hilfe/`, Name muss
  in `site/data/hilfebilder.toml` stehen, dunkle Fassung über `<picture>`, Link auf die volle Größe), `taste` und der
  ältere `bild`. Kein JavaScript, keine fremden Adressen.
- **Bilder:** `site/tools/bildschirmfotos.py` baut auf einem Wegwerf-Server (`--listen 127.0.0.1:0`, eigener
  Datenordner, `--secrets memory`, eigenes `HOME`) das erfundene Beispielwerk `site/tools/beispielwerk.typ` aus und
  fotografiert jede Zeile der Bildliste mit Firefox über einen Vermittler im Skript (Cookies, Sitzung, `<details>`
  öffnen, Klick, Messung eines Ausschnitts). `--pruefen` belegt jede Adresse ohne Firefox, `--abgleich` hält Liste,
  Seiten und Dateien gegeneinander. Einzelheiten: `site/tools/README.md`.
- **Prüfung:** `scripts/site-check.py` (von `scripts/ci-site.sh` nach dem Hugo-Bau aufgerufen): tote interne Links
  und Anker, Bilder ohne alt-Text, JavaScript, fremde Adressen, Weiterleitung `/bedienung/` → `/hilfe/`. Tests
  `tests/round37_hilfe.rs` (Seiten, Anker, Bildnamen, verbotene Wörter, Tasten) und `tests/round37_bilder.rs`
  (Bildliste, Hilfe-Link).
- **Oberfläche:** `about::HELP_URL`; Eintrag „Hilfe“ im Menü Baruch ▾ (`templates/base.html`) und auf „Über Baruch“ –
  ein gewöhnlicher Link nach außen, Baruch selbst ruft nichts ab.

## Round 38 – English as the primary language, work language

> From Round 38 on, new sections of this document are written in English; the sections above stay German until
> they are translated as a whole.

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
| `crates/baruch-server/src/work_language.rs` | RPC `work.language`, `work.language_set` (`handles`, `call`, same shape as `crate::timing`); `of(state, work)` reads the work language for a model call |
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
