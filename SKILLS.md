# Baruch – skills for agents

Rules are in `AGENTS.md`, the plan in `docs/Plan.md`, the handover state in `HANDOFF.md`.

## Building and checking
- Test the core: `nice -n 10 ionice -c3 cargo test -p baruch-core`
- Fetch TypeScript once: `cd crates/baruch-server/web && npm ci`
- Build and test the server: `nice -n 10 ionice -c3 cargo test -p baruch-server -j 4`; only one subject area of the
  integration tests: `… --test server <gebiet>::` (e.g. `round12::`)
- Test everything: `nice -n 10 ionice -c3 cargo test --workspace -j 4` (in the main folder or in your own worktree)
- Start: `cargo run -p baruch-server -- --data DIR` → http://127.0.0.1:8765/ (default of `--listen`)
  - `--secrets keyring|memory` or `BARUCH_SECRETS`: the default is the keyring only with the default data folder;
    with your own `--data` (throwaway server) secrets stay in memory and the user's keyring stays untouched.
    Agents never use `--secrets keyring` with a throwaway folder (only exception: the read-only manual check
    against Eliezer approved by the user, see below).
  - `--listen ADRESSE:PORT` or `BARUCH_LISTEN`; agents only `--listen 127.0.0.1:0` (8765 belongs to the user's
    service, `packaging/systemd/`) and end their own process by PID. Never touch the service and
    `~/.local/share/baruch` (no `systemctl`, `pkill`, `killall`).
  - `--data DIR` or `BARUCH_DATA`, default `baruch_store::default_data_dir()` = `dirs::data_local_dir()` +
    `baruch`: Linux `~/.local/share/baruch` (`$XDG_DATA_HOME/baruch`), macOS
    `~/Library/Application Support/baruch`, Windows `%LOCALAPPDATA%\baruch` (Local, not Roaming:
    server-stored profiles would copy the SQLite WAL and the mirror at every login/logout).
    Always use a throwaway folder for trying things out, not the real data.
- Data folder:
  - `baruch.sqlite` (WAL): the source. On the first start the core schema and the sample work
    „Beispielwerk: Leise Abhängigkeit (Auszug)“ (`baruch-core/src/sample.rs`, only seeded into an empty database),
    then the library (literature, FTS5) with sample titles.
  - `works/<slug>/`: Git mirror per work (`main.typ` = work template + `#include`, `abschnitte/NNN-titel.typ`;
    with a work template additionally `vorlage.typ`, and every section file begins with
    `#import "../vorlage.typ": *`), one commit per acceptance/save/import with the trailers `Persona`, `Model`,
    `Baruch-Node`, `Baruch-Revision`. Read only; the database decides the state, at start all works are brought
    up to date. slug: at most 40 characters, cut at the last word boundary, collision `-2`, `-3`.
- Import a Typst book (only through the command line, not through MCP):
  `cargo run -p baruch-server -- import buch.typ --data DIR` (new work, output
  work/slug/sections/mirror/commit; a second import of the same file → exit 1; FIFO or folder →
  „… ist keine Datei“); back: `cargo run -p baruch-server -- export <id|slug> --data DIR` (Typst on
  stdout, byte-identical after import, without the `#import` lines of the mirror). The import also reads the images
  from `image("…")` relative to the folder of the file (not outside it), back with `export <id|slug> --out ORDNER`
  (text and images byte-identical, nothing overwritten).
  `docs/buch.typ` is the book of a third party: never commit it, never copy it into tests/docs; tests use
  their own files under `crates/<crate>/tests/data/`.
- MCP in Claude Code: `claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp`
  (holds for a manual start and the user service). Without a token only loopback; from other machines with
  `--header "Authorization: Bearer <Token>"` („Einstellungen → MCP-Zugang“); tools in `docs/MCP.md`.
- Model backend (Round 10): `serve` connects Eliezer through `assist::eliezer::EliezerPort`. A manual check against
  the real Eliezer only when the user approves it for the round: your own server
  `serve --listen 127.0.0.1:0 --data <Wegwerf-Ordner> --secrets keyring`, read only (no
  `connection.set_secret`/`delete_secret`, do not change the address), only the approved model, never
  `/api/v1/models/load|unload|estimate`, one request at a time, short outputs (`BARUCH_MAX_TOKENS=300` at
  start clamps `max_tokens` of every model call); only Baruch reads the key. Set the persona models in the
  throwaway folder to the approved model with `persona.update`.
- Screenshot: `firefox --headless --screenshot bild.png --window-size=1400,860 http://127.0.0.1:PORT/`
- Throwaway server for the visual check without a request to Eliezer: first create the data folder with
  `baruch-server import DATEI.typ --data DIR`, then set the connections `eliezer` and `multigpt` in
  `DIR/baruch.sqlite` to `http://127.0.0.1:9` (`INSERT INTO connection (kind, config) … ON CONFLICT(kind) DO UPDATE`),
  only then `serve --listen 127.0.0.1:0 --data DIR --secrets memory` – otherwise `rag::catch_up` embeds at start
  through the default `127.0.0.1:1234`. Collapsed `<details>` for a photo: fetch the page with `curl`, set `open`,
  insert `<base href>` and photograph the file.
  (Port of your own throwaway server; `--no-remote --profile <eigener Ordner>`). Cookies such as `baruch_folded` for
  a photo: after the first photo insert a row into `<profil>/cookies.sqlite` (`moz_cookies`) following the pattern of
  `baruch_work`.
- **Manual check against Eliezer with a gatekeeper** (integrator Round 15, instead of `--secrets keyring`): a small
  program on `127.0.0.1:0` (Python, `http.server`) forwards only `POST /v1/chat/completions` (at most the approved
  number, one at a time, only the approved model) and the reading lists `GET /v1/models`, `GET /api/v1/models` to
  `127.0.0.1:1234`; everything else (embeddings at start, load/unload) gets 403. Only the gatekeeper reads the key
  (`keyring.get_password("baruch", "eliezer")`) and puts it into the Authorization header; never print it,
  never log it. Baruch runs with `--secrets memory`, the connection `eliezer` points to the gatekeeper,
  `multigpt` to `127.0.0.1:9`. The log of the gatekeeper (duration, length, messages) shows whether the context
  was complete.
- Photos after a click (integrator Round 17, e.g. „Zitate prüfen“ with a highlight in the typeset view, expanded
  comments): a small front server of the same origin that passes everything on and inserts into HTML with
  `?klick=SELEKTOR` a script (click after `DOMContentLoaded`) and an image that only arrives after 3 s – so the
  `load` event that `firefox --screenshot` waits for comes only after the result of the click.
- Shared scratchpad: all agents of a round share it. Helper files and logs only with your own task id in the name
  (`verszaehlung_…`, `integrator17_…`), otherwise another agent overwrites them.
- Pages with JavaScript that only arise as a POST response (e.g. the notice page after `/assistenten/act`): save the
  response with `curl` and deliver it through a small front server of the same origin (pass everything else to the
  throwaway server), otherwise the modules do not load from `file://`.
- Never in parallel with Firefox, other agents or test runs of other projects.

## User interface
- Templates: `crates/baruch-server/templates` (`base.html`, `workspace.html`,
  `fragments/{tree,editor,edit_toolbar,revisions,revtree,assistants,library,proposals,idea_capture}.html`), embedded in `view.rs`; for the
  literature popup and its pages also `literature.html`, `source.html`, `fragments/highlights.html`
  (own environment in `literature.rs`); for managing works `works.html`, `work_new.html`,
  `work_edit.html`, `work_delete.html` (own environment in `works.rs`, `fragments/work_new.html` also in
  `view.rs` for the import dialog of the top bar); for the outline `outline.html`, `outline_edit.html`,
  `outline_confirm.html` (own environment in `outline.rs`); for the outline scheme `work_numbering.html`
  (own environment in `numbering.rs`); for the images `assets.html`, `asset.html` (own environment in
  `assets.rs`); for the settings `settings.html` and `settings/*.html` (own environment in `settings/mod.rs`,
  plus `settings/{verbindungen,quellen}.html` in `connections/pages.rs`); for the ideas
  `ideas.html`, `idea.html` (own environment in `ideas.rs`, `fragments/idea_capture.html` also in `view.rs`
  for the dialog of the top bar). CSS per area in
  `static/{app,library,works,outline,wysiwyg,numbering,assets,proposals,ideas,typeset,menu,settings,connections}.css`,
  joined in `lib.rs` (`STYLE`). minijinja 3 takes its own types through `Value::from(Serde(x))`.
- Top bar (Round 9): only in `base.html` (`{% block topbar %}`, opening tag exactly `<header class="topbar">`);
  pages deliver `{% block page_title %}` and `{% block page_actions %}`, never a top bar of their own. Menus
  „Werk“/„Ansicht“/„Werkzeuge“ as `<details class="menu">` (usable without JavaScript), widget `menubar`
  (`web/src/menu.ts`, badges through RPC `menu.badges`), data of the work page from
  `crate::menu::workspace_topbar`. At most one `data-literature-open`, `data-idea-open`,
  `data-work-import-open`, `data-view-toggle` per page. Test `tests/round9_menu.rs` (add new pages there to
  `all_pages`). The theme widget stays in `panes.ts` and listens to `baruch:settings-changed`.
- Settings: `src/settings/` (registration, frame, pages), templates `templates/settings.html` and
  `templates/settings/*.html` (own environment in `settings/mod.rs`), widget `web/src/settings.ts`, style
  `static/settings.css`. New page: an entry in `PAGES` with a `PageRender` function that only delivers
  `<section class="settings-page" data-settings-page-body="<slug>">…`; cookies/host through
  `settings::request()`. Never copy form blocks of other pages, reuse them as `{% block content %}` with the
  context building pulled out (pattern `work_edit.html`/`work_numbering.html`).
- Tests with a backup put the data folder into a subfolder of a tempdir (`round9_settings.rs`,
  `open_in_subdir`), otherwise the backup lands in the temp folder itself.
- Connections and sources: `src/connections/` (RPC `connection.*`, `sources.*`, forms
  `POST /einstellungen/{verbindungen,quellen}/act`, MCP `sources_list`). Secrets only through
  `AppState::secrets` (`SecretStore`); tests always take `MemorySecrets` (`AppState::open`), never the real
  keyring; checks only against your own test servers on `127.0.0.1:0`. Secrets are bound to their target
  (migration 0014): after `connection.set` with a new address in a test, first `connection.set_secret`, otherwise
  `connection.check` holds the secret back (`tests/round9_connections.rs`, `confirm_secret`). No secret in HTML, RPC, messages,
  addresses or check results (test with a unique test secret in `tests/round9_connections.rs`).
- Works: work choice `<select data-widget="work-select">` in the top bar (`web/src/works.ts`), cookie
  `baruch_work`; `/werke…` without JavaScript. Outline: the tree in `fragments/tree.html` (numbering in the head,
  forms ↑ ↓ ← → and links `nojs-only` as the fallback without JavaScript), widget `outline` (`web/src/outline.ts`:
  drag and drop, context menu, rename by double click/F2, create, delete with `confirm: true`); after
  a change without reloading `baruch:navigate` (listened to by `workspace.ts`). Every writing tree action first calls
  `leaveEditor()` (`widgets.ts`), the only question about unsaved text.
- Typeset view: area `div.wysiwyg[data-widget=wysiwyg]` in the editor form (`fragments/editor.html`, without
  JavaScript `hidden`), widget `web/src/wysiwyg.ts`, style `static/wysiwyg.css`, RPC `wysiwyg.*` in
  `src/wysiwyg.rs` (`docs/WYSIWYG.md`). What is submitted is always the text field: `outline.ts` fires
  `baruch:before-save {wait}` before `new FormData`, `wysiwyg.ts` fills the text field. No import between
  `outline.ts` and `wysiwyg.ts`. Cookie `baruch_editor` = `satz` | `quelltext`. A heading is always one line in the
  typeset view (`splitHeading`, `headingEdits`); the client checks the title rules with the same
  message, the server stays decisive (`check_title`).
- Proposals in the reading flow: `fragments/proposals.html`, widget `proposals` (`web/src/proposals.ts`), cookie
  `baruch_folded`. Since Round 8 at their place: `baruch_wysiwyg::align` against the **shown** revision, one
  `details.pv[data-pv-at]` per piece; `wysiwyg.ts` puts the pieces into the typeset view and skips `.pv` when
  saving. Persona colours only as checked `#rrggbb` through `crate::personas::style` into the
  `style` attribute (`--pc-light`, `--pc-dark`, `--pc-line`, `--pc-line-dark`), never unchecked from user input.
- Addresses: canonical `/?work=W&key=K[&rev][&view=preview][&pick][&lq]` (`view::node_url`); new links and
  redirects never with `node=` (test `round8_posts::no_generated_link_or_redirect_carries_node_any_more`).
- Managing posts: `src/posts.rs` (`post.*`, `POST /beitraege/act`), widget `chat` (`web/src/chat.ts`,
  LocalStorage `baruch_chat_seen` only in try/catch). MCP may only touch its own posts (persona or `@mcp:<name>`).
- Typst typesetting: crate `baruch-render` (offline, SVG), `src/typeset.rs` (`preview.typst`, `/werke/{id}/satz`,
  static cache, one semaphore, 60 s), `src/mcp/typst.rs` (`typst_check`). Tests that use the
  static cache append a comment line of their own to the work (their own state).
- Book as PDF and cover (Round 11): `typeset::prepare` puts the frame from `cover::frame` around the source
  (cover front/back, front matter; without both byte-identical) – typeset page, `work.pdf`, `/werke/{id}/pdf` and
  `export --pdf` use the same `prepare` (same page count). PDF only through `crate::pdf::source_to_pdf`
  (`baruch_render::compile_pdf`, one generation per work, lock `typeset::RUNS`, 120 s). The mark „PDF
  entsteht“ is process-wide per work id: tests that create PDFs or set `pdf::claim` first hold
  `common::PDF_SERIAL` (all server tests run in one process; `round11_pdf.rs`, `round11.rs`,
  `round11_cover.rs`). PDF responses are bytes: in tests
  not through `Server::send` (UTF-8), but `oneshot` + `to_bytes`. Cover/book data: store `src/book.rs`
  (migration 0017), `baruch-typst/src/cover.rs` (pure; user texts only as string literals), server `src/cover.rs`.
  Print check `Rendered.print` (raster images under 300 dpi), not in `diagnostics`.
- Ideas: dialog `<dialog data-widget="idea-capture">` in the top bar (`web/src/ideas.ts`: Alt+E,
  selection as `quote`, focus back), without JavaScript the pages `/einfaelle…` with forms to
  `POST /einfaelle/act`.
- Literature: popup `<dialog data-widget="literature">` in the top bar of `workspace.html` (content
  `fragments/library.html`), without JavaScript the pages `/literatur` and `/quelle` from `src/literature.rs`
  with their own template environment (`literature.html`, `source.html`, `fragments/highlights.html`) and
  forms to `POST /literatur/act`. RPC `literature.render` and `highlight.*` (`docs/RPC.md`).
  minijinja masks `/` in values as `&#x2f;`; `literature::href` passes self-built addresses on safely.
- Personas (Round 10): `src/assist/` (port `port.rs`, adapter `eliezer.rs`, context `context.rs`, role prompts
  `prompts.rs`, hooks `hooks.rs` → `crate::scripture`, RPC `rpc.rs`, routes `routes.rs` with
  `POST /assistenten/act` and `/assistenten/stream`), model backend `src/models.rs` (RPC `model.*`, process-wide
  queue, cache of the model list), widget `web/src/assist.ts`, style `static/assist.css`.
  Model calls only through `crate::models::chat` or `state.model` (queue, binding of the key, alias),
  never `baruch_llm::chat_stream` directly. Call the model **without** `write_lock`, lock only for writing and
  resolve `key` again. The chat input is a form to `/assistenten/act` with `method=persona.chat`, field
  `message` (tests search for `value="persona.chat"`).
- Round 15: chat in `assist/chat.rs`, chunking `assist/chunk.rs`, SWORD fact block `assist/bible_facts.rs` (data
  from `src/bible_research.rs`), progress `assist::Progress` (`Posted`, `Retry`, `Compressed`) → SSE in
  `routes.rs`; budget only through `state.model.context(state, &persona)` (`ModelPort::context`, `models::context`).
  Toast `fragments/toast.html` + `web/src/toast.ts` (event `baruch:toast`) – CSS only for
  `.toast[data-widget="toast"]`, because the bare class `.toast` is the red error message from `app.css`.
  Notice page without JavaScript `assist_notice.html` (`routes::notice_page`). Connections per persona: store
  `model_connections.rs` (migration 0036), `baruch_llm::Client` (`client.rs`), approval „extern“ in
  `EliezerPort::chat`. SWORD commentaries/lexicons `baruch-sword/src/reference.rs`, choice per work
  `sword_sources.rs` (0037), building blocks `bibles::commentary_panel_html`/`commentary_hits_html`. Bibles in the
  literature `literature/bibles.rs` (`/literatur/bibel`), threads without a section `src/threads.rs` (0038), MCP queue
  per client `mcp/quota.rs`.
- Round 16 („Predigt“): the work kind sermon only through `work.kind == WorkKind::Sermon`; template in `store/works.rs`
  (`Preset::Sermon`, `create_work_with_text`, sermon text from SWORD in `server/works.rs`). Time keeper: counting rule
  only `baruch_typst::timing::spoken_words` (the editor widget `web/src/timing.ts` counts through RPC `timing.count`,
  never in TypeScript), settings `Store::timing_settings` (0039, SQLite only), server `src/timing.rs`
  (`work_timing`/`timing_json`; `WorkTiming::shown` = sermon or target time). Keyword notes: typesetting
  `baruch-render/src/notes.rs` (user texts only as Typst strings), server `src/pdf/notes.rs` (hangs on
  `pdf::handles`/`pages`, PDFs under `common::PDF_SERIAL`), page `templates/notes.html`. Style of all R16 building
  blocks in `static/predigt.css`. „Kritisch:“: `prompts::critical_finding` (line start or after the part reference)
  for review and quotation check, `critical_line` (line start only) for removing it from proposals.
- Manual check with a SWORD test Bible: a throwaway server needs real module files. For that, temporarily write an
  example `crates/baruch-sword/examples/<name>.rs` (`baruch_sword::fixture`, freely invented text), run it with
  `cargo run -p baruch-sword --example <name> -- <Ordner im Scratchpad>`, delete the file again (never
  commit it) and set the folder through RPC `bible.set_dirs`. Outline requests in the throwaway server through MCP
  (`initialize` → `Mcp-Session-Id`, then `tools/call outline_rename`/`outline_add`).
- Round 11 user interface: cover page `templates/cover.html` and block `fragments/cover_panel.html` (own
  environment in `cover.rs`, style `static/cover.css`; also in „Einstellungen → Werk“ and at the top of the page
  „Bilder“), requests about images `fragments/asset_requests.html` (own environment in `asset_requests.rs`, at the
  bottom of „Bilder“). „Einstellungen → Assistenten“: one tab per persona (`?persona=<handle>`, widget `persona-tabs`
  in `web/src/persona_tabs.ts`), one form `action=save`; chat completion „@…“ `web/src/mention.ts`.
  Addresses built by the server in templates (`back`, `url`) with `|safe`, otherwise minijinja writes `/` as `&#x2f;`
  (test `tests/assets.rs`).
- Bible references: crate `baruch-scripture`, server `src/scripture.rs` (RPC `scripture.check`, `facts_for`,
  `mark_answer`), MCP `src/mcp/scripture.rs`. Round 17: versifications `versification.rs` (all 18 of
  `sword-modules`, German/KJV from `data/*.tsv`) and mapping `versification/mapping.rs` with the rule table
  `data/versification-map.tsv` (pivot KJV, never guess), RPC/MCP `scripture.map`/`scripture_map`
  (`src/scripture_map.rs`, `src/mcp/scripture_map.rs`); quotation spans only through `crate::quotes::quote_spans`
  (inline + block quotations, also in `#szene`), assignment `wording::quote_for`; reading in the versification of the
  module only through `bibles::read_passage` (never `bible.verse` with a German reference); check display RPC
  `bible.marks`, page `src/scripture_check.rs`, widget `web/src/scripture_check.ts` (CSS Custom Highlight API, no DOM
  change); keys of enciphered modules only in `AppState::secrets` (account `sword-cipher:<modul>`), tests with
  `MemorySecrets` and `Fixture::enciphered(<erfundener Schlüssel>)`, BZIP2/XZ through `Fixture::compression`.
  Reference claims in findings: `assist/ref_claims.rs` (after `mark_unbacked_criticals`). MCP tokens: store `src/mcp_tokens.rs` (migration 0016, hash only),
  server `src/mcp/tokens.rs` (RPC `mcp_token.*`), page `settings/access.rs`, `guard::remote_only_mcp`.
  Pages that show a secret once: 200 directly (no redirect), `Cache-Control: no-store`, value through
  a `task_local` of its own only into this response.
- RPC methods are at the top of `src/rpc.rs`; `ui.render` delivers the HTML of the three areas,
  `library.search` is the search tool of the lector (work and library). All methods take `work?`
  (missing → default work). MCP in `src/mcp.rs`, tools in `src/mcp/tools.rs`, sessions in
  `src/mcp/session.rs`, ideas in `src/mcp/ideas.rs` (`docs/MCP.md`); errors always through `rpc::RpcError::from`. Writing routes behind `guard::same_origin`; test requests with a foreign `Origin`
  get 403, `/rpc` needs `Content-Type: application/json`.
- Docs: usage = help page under `site/content/de/hilfe/` (since Round 37; `docs/Bedienung.md` is the entry point and
  carries only technical matters), `docs/Architektur.md`, `docs/RPC.md`, `docs/MCP.md` – update them in the same commit.
  Forms (`form[data-rpc]`, field `method`) go to `POST /act` without JavaScript, through RPC with JavaScript.
- Widgets: `web/src/widgets.ts` (`register`/`mount`), `workspace.ts` (navigation, forms, Alt+←/→ without
  Shift, `confirm: true` when deleting, event `baruch:navigate` with `detail.url` → redraw),
  `panes.ts` (column borders, theme, editor, outline, chat), `library.ts` (filter titles), `menu.ts`
  (menu bar), `settings.ts` (settings popup, Alt+,),
  `literature.ts` (popup with Alt+L, find spot in the editor, context menu of the document view).
- Model: David (`~/Dokumente/Projekte/Psalm/server`).

## Crates
- `baruch-core`: types, sample data, lector choice, diff. `Post` has `id` (0 = new) and `created`.
- `baruch-render`: Typst typesetting (Round 8), pure: `Source`, `stand`, `compile` → SVG pages, heading pages,
  diagnostics. The only crate with `typst*` dependencies (0.15.1, `typst-kit` only `embedded-fonts`/`scan-fonts`);
  tests `tests/{compile,stand,schemes,lines,pdf,print}.rs`; Round 11: `compile_pdf` (typst-pdf 0.15.1), `PrintWarning`.
- `baruch-typst`: pure, without DB: `parse` (work template + headings `=`/`==`/`===`, `title` resolved, `raw_title`
  literal) and `join` (byte-identical back); `escape_title`/`unescape_title`, `is_newline`, `heading_line`,
  `heading_numbering` (last rule of the top level, pattern language with `P|k` and `P1;P2;…`), `scheme`
  (outline schemes: `Scheme`, `Numbering`, `export_template`), `images::image_paths` (image paths from
  `image("…")`), `format_number` (tests `tests/{roundtrip,numbering,scheme,images}.rs`, data `tests/data/`). Do not
  change public functions without updating `baruch-wysiwyg` as well. Round 14: `marker` (part `#heading(level: 1, numbering: none)[T]
  <baruch-part>`, „ohne Nummer“ `… <baruch-unnumbered>`; `part_line`, `unnumbered_line`, `relevel_line`,
  `set_numbered_line`, `part_page_rule`; `Heading.level` 0 = part, `Heading.numbered`), tests `tests/parts.rs`
  (data `tests/data/teile.typ`); `Numbering::from_template` (re-import, functions per level, word schemes).
  Never write markers as a literal, always through these functions. Text of a new part =
  `heading_line(0, Titel, None) + "\n"`; export/mirror/typesetting project `node.numbered` through
  `set_numbered_line` (`store/snapshot.rs`), the DB texts stay. The trigger `node_numbered_default` (0006)
  sets `numbered = 0` for an empty number – whoever inserts nodes sets `numbered` explicitly afterwards (import)
  or inserts with the provisional number `'0'` (`outline.rs`).
- `baruch-wysiwyg`: pure, without DB and file system: `parse` (never fails; the concatenation of the `raw` = text),
  `serialize` (Keep literal, edited blocks canonical, islands deletable only as a whole), `emit`,
  `render_html`, `diff_html` (with images: `render_html_with`/`diff_html_with` and `image_path`; islands with
  an image as `ws-figure`), `emit`/`serialize` end a heading at the first line break
  (`split_heading`, Round 7),
  `sheet_style`/`sheet_css` (only rules of the top level). Tests
  `tests/{roundtrip,emit,serialize,html,style,blocks,align,parts,konstrukte}.rs`, data `tests/data/satzbild.typ` (freely invented, all
  constructs and islands) and `tests/data/szenen.typ` (scenes, work assignments, labels, image island); the property test runs with
  a fixed seed and can be extended through `WS_SEED`/`WS_CASES`.
- `baruch-store`: SQLite (sqlx, runtime API only, migrations 0001–0099); `accept`/`reject`/`save_human`/
  `add_proposal`/`add_post`/`import_typst` and the revision tree (`src/tree.rs`: `set_current`, `save_human_on`,
  `update_revision`, `delete_revision`, `revision_tree`; `switch_to` behind `set_current`/`accept`), works (`src/works.rs`: `create_work`, `update_work`,
  `delete_work`, unique titles through `title_key`) and outline (`src/outline.rs`: `outline_*`,
  `set_numbering`, `set_numbering_scheme`, `numbering_preview`, `numbering_rule`, `sync_heading`,
  `check_title`/`MAX_TITLE_CHARS` = title rules for every path, `StoreError::Title` → -32602) and images
  (`src/assets.rs`, migration 0009: `asset_*`, `add_asset`, `replace_asset`, `import_typst_with_assets`,
  `export_assets`, `normalize_asset_path` = normal form for every path comparison; tests
  `tests/{assets,migration_0009}.rs`), persona colours (`src/personas.rs`, migration 0010:
  `persona_colors`, `set_persona_color`; Round 9 `persona_list`, `update_persona` with `PersonaEdit::check`;
  tests `tests/{personas,migration_0010,round9_personas}.rs`), connections and sources (`src/connections.rs`,
  migration 0013: `connection_*`, `work_rag`/`set_work_rag`, `source_*` with a gapless `position`; tests
  `tests/{connections,migration_0013,migration_0014}.rs`; 0014: binding `secret_target`), role prompt and
  lector choice (migration 0015, `persona_prompt`/`set_persona_prompt`, `lector_choice`/`set_lector_choice` in
  `store_meta` `lector`; tests `tests/{round10_personas,migration_0015}.rs`), MCP tokens (`src/mcp_tokens.rs`,
  migration 0016; tests `tests/{mcp_tokens,migration_0016}.rs`) and ideas
  (`src/ideas.rs`, migration 0011: `idea_*`, `Idea::entry_markdown`, `einfaelle.md` through `snapshot::files` only
  when ideas exist; tests `tests/{ideas,migration_0011}.rs`), posts (`src/posts.rs`, Round 8:
  `post_get/delete/move/to_idea`, `PostInfo::entry_markdown`; tests `tests/posts.rs`), export map
  (`src/export_map.rs`: `export_map(w).typst == export_typst(w)` byte-identical; test `tests/export_map.rs`) each one transaction (`BEGIN IMMEDIATE`),
  `mirror_snapshot` for Git, `export_typst`. New migration: `build.rs` has `rerun-if-changed=migrations`;
  never change existing ones (checksum). sqlx-sqlite runs every migration in its own transaction
  (0.8.6 ignored `-- no-transaction`, **0.9 honours it** – never set the mark, test
  `migration_0007`): no `BEGIN`/`COMMIT` and no `PRAGMA foreign_keys` in a
  migration, instead `PRAGMA defer_foreign_keys=ON` and a guard through `pragma_foreign_key_check` (0007).
- `baruch-library`: titles, quotations, highlights (`src/highlights.rs`, 0103), full-text search `search_fts`
  (migrations 0101–0199; 0104 keeps the index current for changed and deleted revisions, 0105 for
  deleted and moved posts); `build.rs` has
  `rerun-if-changed=migrations`. Both migrators
  share `_sqlx_migrations` with `ignore_missing`; order `Store::open` → `migrate` → `seed_if_empty`
  (`AppState::open` in the server).
- `baruch-llm` (Round 10): client for Eliezer (`models`, `chat_stream`, `Limits`, `LlmError`, `Queue`, `alias`),
  pure; mock `mock::MockEliezer` (public, for tests of the server too); tests `tests/{client,queue}.rs`,
  unit tests in `src/{lib,sse}.rs`.
- `baruch-scripture` (Round 10): pure (std + serde), `parse`/`scan`/`find_book`/`books`, `RefStatus`, `NoBible`
  (STUB wording); data `data/versification.tsv` (numbers only, source/licence in the head, never change by hand –
  regenerate from the source and update the head); tests `tests/{references,versification}.rs`. Since 2026-11-06
  `versification` (German, KJV from `data/versification-kjv.tsv`) and `wording` (find and compare a quotation),
  test `tests/wording.rs`.
- `baruch-sword` (2026-11-06): read SWORD Bibles through `sword-modules` (CC0, only feature `zip`; never libsword or
  GPL crates such as `rsword_chirho`). Always write test Bibles yourself (`baruch_sword::fixture`, freely invented
  text, temp folder); server tests set their module folders through `bible.set_dirs` (`tests/sword_bibles.rs`), never
  the real folders of the machine. Never put foreign Bible text into the repo, tests or docs.
- `baruch-rag` (Round 12): shared interface of the RAG ways (contract, literal; `tests/port.rs` checks the
  SHA-256 of both files – a change updates the test). `baruch-lancedb`: local index through `lancedb`
  0.41, pure (no SQLite, no network), its own Tokio runtime, tests `tests/index.rs`. `baruch-llm` additionally
  `embed.rs` (`POST /v1/embeddings`, alias `nomic-embed-text`), test `tests/embeddings.rs`.
- `baruch-gitmirror`: Git mirror through gix (pure Rust), synchronous, in the server in `spawn_blocking`;
  `CommitRequest::allow_empty` forces a commit with an unchanged tree (log).
- `baruch-server`: library (`src/lib.rs`, `rpc.rs`, `view.rs`, `mirror.rs`, `import.rs`, `mcp.rs`, `literature.rs`,
  `works.rs`, `outline.rs`, `wysiwyg.rs`, `numbering.rs`, `assets.rs`, `personas.rs`, `ideas.rs`, `posts.rs`,
  `typeset.rs`, `menu.rs`, `settings/`, `connections/`, `assist/`, `models.rs`, `scripture.rs`, `mcp/ideas.rs`,
  `mcp/typst.rs`, `mcp/scripture.rs`, `mcp/tokens.rs`, Round 11 `pdf.rs`, `cover.rs`, `asset_requests.rs`, `mcp/assets.rs`,
  Round 12 `rag/`, `search_hooks.rs`, `connections/{pgvector,multigpt,collections}.rs`, `mcp/rag.rs`,
  `assist/rag_facts.rs`) and a thin `main.rs`
  (`serve`/`import [--title]`/`export [--out | --pdf DATEI]`). Default work at run time
  `state.default_work()` (smallest id now; `state.work` is only the state at start and is not read),
  mirror per work (`state.mirror(work)`, opened at first use). Tests (modules of the one
  binary `tests/main.rs`, see "Tests") in
  `tests/{rpc,pages,library_search,literature,git_mirror,import,works,work_crud,outline,node_keys,headings,tree,wysiwyg,satzbild,numbering,assets,mcp_assets,origin,mcp,revisions,round3,round4,round5_fixes,round5,round6,round7_titles,proposals,personas,ideas,mcp_ideas,round7,concurrency,outline_mcp,round9_menu,round9_settings,round9_connections,round10,round10_models,round10_personas,round10_scripture,round10_tokens,round11,round11_pdf,round11_mcp_assets,round11_cover,round11_images,round11_personas,round12…,round13…,round14,round14_parts,round14_outline,round14_numbering}.rs`
  with a temp data folder (`tests/common`:
  `open()`, `import_garden()`, `rpc()`, `git()`). MCP tests set the peer with
  `request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127,0,0,1], 40000))))`.

## Revisions
- Revision tree (migration 0004): every revision has a parent `based_on` (root = NULL), own saves as well
  (`save_human_on` takes the shown revision, `save_human` the current one).
- Exactly one `current` per section (partial index `revision_one_current`); it is a **mark**, switchable
  to any other revision (`Store::set_current`, RPC `revision.set_current`). Accepting = switching to a
  proposal; rejecting stays a status for proposals. Status changes only → current (from proposed, rejected,
  superseded), current → superseded, proposed → rejected (trigger `revision_status_transition`).
- Changing (`update_revision`: text, note, sets `edited`) and deleting (`delete_revision`) only by the
  human, never through MCP. Deleting needs a confirmation that the server checks (`/act`: checkbox
  `confirm`, RPC: `confirm: true`). Origin (section, number, author, model, time) is immutable by trigger, the
  current revision cannot be deleted, children of a deleted revision hang on its parent.
- Numbers are unique and rising, with gaps after deleting, never reused (`node.last_rev`).
  Access everywhere through the **number** (`Node::revision`, `position_of`, `current_number`), never through position ± 1.
- Sections: `Node::id` is the position (seq) that every outline change assigns anew; `Node::key` (= `node.id`
  in SQLite) stays. Writing forms, RPC and MCP send `key` along (`rpc::resolve_keys` under
  `write_lock`, the key takes precedence). New writing paths the same way.
- Current revision through `revision.save` or changing the current one, and every proposal: exactly one heading of the
  level of the item, otherwise `StoreError::Heading`. Switching and accepting (`Store::switch_to`): the outline
  follows the revision, also in the level (sub-items relevelled along, one commit); the revision is never rewritten.
- Work ids are never reused (`AUTOINCREMENT`, migration 0007), slugs of deleted works are permanently reserved in
  `work_retired`, seeding happens only without `store_meta.seeded`.
- MCP writes (`propose_revision`, `add_post`, `refs`, `outline_*`) and `highlight.cite` quotes only with `key`; `node`
  → -32602.
- Outline through MCP (wish 2026-11-06, `src/mcp/outline.rs`, store `outline_add_many`, `src/requests.rs`,
  migration 0012): `outline_add` creates immediately (author persona/client, one commit); renaming, moving,
  changing the level, number on/off, deleting only as a request that the human confirms. Tool name checks exempt
  `outline_*`.
- Assistants and MCP callers write only proposals (`propose_revision`, `Store::add_proposal`) and posts;
  they may move, delete and turn their own posts into ideas (Round 8, one commit each
  `Baruch-Action: post-*`), never foreign ones.
- Every acceptance, switch, save, change and deletion is a commit in the mirror (trailer
  `Baruch-Action: switched|updated|deleted`); switching, changing and deleting also as an empty log commit
  (`allow_empty`), changing with before/after, deleting with the deleted text.
- Images have revisions like sections (exactly one `current`, switchable, content immutable by trigger –
  replacing is a new revision; changing only caption/alternative text/note; deleting only by the human with
  `confirm`; mirror commit `Baruch-Action: asset-*`). Content always TEXT (Base64 or SVG), never BLOB. Through MCP
  read (`list_assets`, `read_asset`) and create immediately (`asset_add`, origin in the mirror); replacing, setting/removing
  the cover and deleting only as a request (`asset_replace`, `asset_set_cover`, `asset_delete`; store
  `src/asset_requests.rs`, migration 0018, server `src/asset_requests.rs`) that the human confirms (Round 11).
- Ideas change no book text: full CRUD for assistants and MCP too (IDEA_CRUD), every action one
  commit with `Baruch-Action: idea-*` and `Baruch-Origin`; ideas without a work in the Git folder
  `<data>/einfaelle/`. Persona colours are global and are not in the mirror.
- Titles of outline items: not empty, without a line break, at most 200 characters (`check_title`) on
  every path (outline, save, switch, proposals); only a changed title is checked (Round 8:
  `check_chunk_titles(chunks, old_title)`); import/export do not check (byte-identical).
- An error in the Git mirror rolls nothing back; the RPC response then carries `mirror` instead of `commit`.

## Exchange format between instances (Round 26)
- Format, elements, identifiers, reading in and limits: `docs/Austauschformat.md`; XSD `docs/schema/baruch-werk-1-u3.xsd`
  (namespace `urn:baruch:werk:1`, minor version `version="3"` since Round 27 with `<umschlag>` (blocks, tombstone kind
  `umschlag-block`), `<verlag-anschrift>`, `<druckerei-anschrift>` and `<satzbild>`; minor version 2 since Round 25 with
  `<karte>`). `docs/schema/baruch-werk-1.xsd` (minor version 1) and `baruch-werk-1-u2.xsd` stay frozen – never
  change them (a test pins their SHA-256); a new minor version gets an XSD of its own.
- Crate `baruch-exchange` (pure, without DB): `model.rs` (types `Exchange*`, frozen, integrator only), `write.rs`
  (own writer, deterministic ZIP), `read.rs` with `read/{dom,map,archive,check}.rs` (safe reader:
  no DTD, limits from `limits.rs`, path and checksum check, `validate`), `sample.rs` (sample work,
  freely invented). Tests `tests/{write,xsd,read}.rs`, data `tests/data/import/*.baruch.xml`.
- Store: `uuids.rs` (instance, `new_uuid`, queries by UUID), `exchange_dump.rs` (work → `ExchangeWork`),
  `exchange_apply.rs` (preview and accepting through the same code, the preview rolls back; `derive_uuid`).
  Migrations 0041 (columns `uuid`, fill triggers, tombstones `uuid_tombstone`, `store_meta.instance_uuid`) and 0042
  (tombstones on the cascade); 0043 and 0045 rebuild `uuid_tombstone` for new tombstone kinds (all triggers literally
  anew, model `0045_cover_block.sql`). **Only `exchange_apply` sets a uuid**, and only on rows just created
  (guard test `uuids.rs::no_code_path_but_exchange_apply_writes_the_uuid_column`). Whoever later rebuilds one of the ten
  tables takes the column, the index and both triggers along. Tests `tests/{migration_0041,uuids,exchange_dump,
  exchange_apply}.rs`.
- Server: `src/exchange/{mod,export,import}.rs`, `src/instance.rs`, `src/mcp/exchange.rs`; routes
  `GET /werke/{id}/austausch[.baruch|.xml]`, `GET /austausch/einlesen`, `POST /austausch/vorschau` (multipart, field
  `datei`, inbox `<data>/austausch-eingang/<sha256>`, cleared away after 24 h), `POST /austausch/uebernehmen`
  (urlencoded `sha256`, `mode`, `title`), `POST /einstellungen/daten/instanz`; RPC `work.exchange_*`,
  `instance.*`; MCP only `exchange_export`; CLI `export ID --baruch DATEI`, `import DATEI.baruch[.xml]
  [--abgleichen] [--vorschau] [--title]`. Tests `tests/round26_{ids,export,import,roundtrip}.rs`.
- XSD check in tests: `xmllint --noout --nonet --schema docs/schema/baruch-werk-1-u3.xsd DATEI`; if `xmllint` is missing,
  it is honestly skipped with a message (a Rust check of the required elements runs anyway).
- End to end by hand (integrator R26): two throwaway servers A/B following the detour above, build a work through RPC
  (proposal through MCP `propose_revision` with `model`), output through `GET`, reading in through RPC and form, command line
  on a third folder without a running server; compare row counts through `sqlite3` read access
  (`file:…?mode=ro`) before and after manipulated files. Comparing two outputs: everything except
  `<ausgabe …/>` must be byte-identical.

## Cover editor (Round 27)
- Types frozen in `baruch_typst::cover_layout` (`CoverBlock`, 15 `CoverBlockKind`, `CoverGeometry` with
  `face_origin`/`face_size`, limits = CHECKs of migration 0045); coordinates in mm from the trim edge at the top left of
  the own face, in the book PDF the cover front is a page W × H with its origin at the trim edge.
  `default_blocks` imitates the old picture per title area, `effective_blocks` = stored or default per kind.
- Without a row in `cover_block` and without front matter functions of the template, `frame_parts`/`wrap_source` stay
  byte-identical (golden states `baruch-typst/tests/data/cover_r26/`, SHA-256 in `round27_satz.rs`); whoever changes the
  typesetting without a layout must rewrite the golden states deliberately (ignored writer in `baruch-typst/tests/cover_layout.rs`).
- Store `cover_blocks.rs` (`set_cover_blocks` upsert per kind without tombstone, `reset_cover_blocks` with tombstone and
  title area in one transaction, `cover_layout_fingerprint` for `expected_layout`); server `cover_layout.rs` (RPC
  `cover.layout|layout_set|layout_reset|fonts`, `layout_view` = stored + preview → sizes, heights, print check),
  `cover_print.rs` (codes `CODES`, contrast in the raster image, SVG covers not measured, `imprint_checks`),
  `cover.rs` in two areas (typesetting: `book_info` … `cover_pdf`; pages/forms/RPC: the rest), `mcp/cover.rs`
  (read only or request through `asset_request` op `cover_layout`). Widget `web/src/cover_editor.ts`, style
  `static/cover.css`.
- Forms without JavaScript: preview `GET /werke/{id}/umschlag[.svg]?b.<kind>.<feld>=…` (stores nothing),
  `POST /werke/umschlag/anordnung` (`action=save|quick|reset`), `POST /werke/umschlag/impressum`.
- Front matter from the work template: `#let schmutztitel|haupttitel|impressum|widmung(buch)` deliver content; Baruch puts
  each on a page of its own and then writes `#let baruch_buch = (…)` once (string literals only). New books
  get `imprint_template()`; store tests of the book template compare with `imprint_template()`.
- Measure the position: `pdftotext -bbox` (word, `xMin`/`yMin` in pt) in the cover and book PDF, subtract the origin
  of the face from `CoverGeometry::face_origin` (`baruch-render/tests/cover_layout.rs`, `tests/round27_abnahme.rs`); if
  `pdftotext` is missing, honestly „ÜBERSPRUNGEN“. Fonts: `baruch_render::font_families()` (blocking, `spawn_blocking`).
- Tests: `tests/round27_{daten,satz,widget,abnahme}.rs`, store `tests/{cover_blocks,migration_0045}.rs`,
  `baruch-typst/tests/cover_layout.rs`, `baruch-render/tests/cover_layout.rs`; PNG with a dark upper third from
  `round27_satz::dark_top_png`.

## Findings and chat (Round 28)
- Data: migration 0046 (`finding`, `finding_status`, `post_version`, columns `post.edited`/`chat_thread_post.edited`,
  tombstone kinds `finding`/`finding_status`/`post_version`, cascade triggers like 0042/0045). Migration tests that compare the
  schema after `Store::open` (0016–0018, 0040, 0041) take `, edited TEXT` out only for `post` and
  `chat_thread_post` (`revision` already has a column `edited`) and add the 0046 objects to the list of new ones.
- Store `src/findings.rs` (types, `finding_insert`/`finding_get`/`finding_list`/`finding_history`/
  `finding_open_counts`/`findings_adopted_in`) and `src/post_edit.rs` (`post_versions`, `thread_post_versions`,
  `post_edited_map`); tests with real findings create them through `Store::finding_insert`.
- Server `src/findings/{mod,parse,rpc,page,view}.rs`: parsing only through `findings::ingest` (never takes the
  write lock, the caller may hold it; no commit), called in `assist::save` (not for propose) and
  `threads.rs::answer`; errors there only on stderr, the post stays. MCP `mcp/findings.rs`, `mcp/posts.rs`;
  mirror `mirror::describe_finding`/`record_finding`, editing a post `PostAction::Edit`.
- Tests: `tests/round28_{befunde,uebersicht,chat,abnahme}.rs`, store `tests/{findings,post_edit,migration_0046}.rs`.
- RFC in a test (integrator R28): do not guess the order of the personas, fetch it before the script through
  `assist::chat::rfc_personas(&state)` and then push the answers into the `ScriptedModel` (`push`); every
  answer with exactly one marker yields exactly one finding, the ids then follow the call order (look up `max(id)`
  beforehand), and the script of the lector can quote `[B<id>]`. Model `tests/round28_abnahme.rs`.
- Status paths of findings without JavaScript: `POST /werke/befunde/act` (`method=finding.set_status|finding.decide`,
  `back` as a relative address) → 303; error → 200 with `role="alert"` and the draft. New pages belong in
  `tests/round9_menu.rs` (`all_pages`); `menu.badges` delivers `open_findings` since R28.

## Round 30 (search/replace, cross-references, chat solutions)
- Interface R30-S1: migration 0047 (`docs/Architektur.md`, "Interface of Round 30 (R30-S1)"), migration test
  `crates/baruch-store/tests/migration_0047.rs`; migration tests that compare the schema after `Store::open` take out
  the appended 0047 columns and `replace_batch`/`replace_batch_work`/`revision_variant_group`. New
  `Order` fields in tests: `links: Default::default(), basis: None`; `ChatOrder` gets `basis: Default::default()`.

### Round 30a
- Jump labels `<kN>` stand in export, export map, mirror and typesetting, never in SQLite.
  - Tests that compare an export, a mirror file or `export --out` with the source compare
    `baruch_typst::xref::strip_key_labels(&…)`. After a re-import the works have new keys, so strip both
    sides.
  - New typesetting golden states (`round27_satz.rs`) are measured by the test run itself (output „gemessen:“).
- Typesetting source: parts have the anchor line before the part line in typesetting (`part_anchors_for_typesetting`),
  and broken references are only text there. Whoever compares ranges of `prepare` with `export_map` applies the same
  function (`round14_parts.rs`).
- Search: `baruch_server::search::find_within(&state, work, &query, Duration::ZERO)` checks the time limit without
  process-wide state. A catastrophic expression stays linear with the crate `regex`.
- Store replace: a section whose heading does not fit rolls back only its `SAVEPOINT`
  (`round30_replace.rs`). `replace_batch` never goes into the exchange format.
- Node probes: `search_keys.js` and `jump.js` have no imports or only pure ones. For `xref.js` the test writes all
  `SCRIPTS` together with `package.json` (`"type": "module"`) into a folder (`round30_verweise.rs`). If `node` is
  missing, the tests report „ÜBERSPRUNGEN“.

### Round 30b
- Duty of a solution in tests: every default demands a solution under „Kritisch: …“; if it is missing, `assist::run`
  asks for it **once** (its own retry reason `prompts::SOLUTION_RETRY_REASON`) – that uses up another step of the
  `ScriptedModel` or an answer of the `MockEliezer` (empty → „OK“). Scripted answers with „Kritisch:“ therefore
  get a solution line (`\n- Schritt: prüfen.` or `- statt: „…“`/`- besser: „…“`), otherwise all following steps
  shift; a test that only checks markers may expect the note `prompts::NO_SOLUTION_NOTE` (then the
  follow-up request fails at the end of the script). Cut-off answers (`Step::cut`) ask for nothing.
- Prompts now contain the outline with `[key:N]` (when there is room) and `JUMP_RULE`; assertions on
  outline lines check `"] 1.1 Titel"` instead of `"- 1.1 Titel"`. Only the lector sees foreign findings under
  `FINDINGS_TITLE`, all others under `KNOWN_TITLE`.
- Solutions, variants and the RFC collective proposal in tests: `tests/round30_loesungen.rs`, `tests/round30_varianten.rs`
  (variant answers in the form `Zeile: …\nBefunde: [B1]\nText:\n= …`; partial proposal with a small context length
  through `persona.update {context_length}` and `[[Absatz n]]` answers). Store: `tests/round30_variants.rs`
  (exchange u5 with `baruch_exchange::sample`, reconciliation never changes known rows). Exchange:
  `baruch-exchange/tests/{read,xsd,write}.rs` (u5 round trip, u4 files, frozen u4 XSD by SHA-256).
- `remap_keys` (R30a) has been implemented since the integration; the store test for remapping the cross-references in
  `exchange_apply` now really checks the rekeying (the fallback message „übersprungen“ only applies when
  `remap_keys` changes nothing).

### Round 30c
- Chat handling: store `post_meta.rs` (`thread_add_post_meta`), `threads.rs` (`thread_post_delete`); server
  `assist/chat.rs` (`addressed`, `line_addresses`, `relay_text`, `basis_param`, `MAX_RELAY_ANSWERS`), `posts.rs`
  (`post.repeat`, `post.since`, `propose_done`), `threads.rs` (`thread.post_delete`, `thread.repeat`, `thread.since`,
  `delete_post_as`), `chat_render.rs` (Markdown subset, links, segments), `chat_live.rs` (writing …/errors),
  template `fragments/post_item.html` (`post_item`, `post_entry`, `thread_entry`).
- Tests: `tests/round30_bedienung.rs`, `tests/round30_live.rs`, store `tests/round30_post_meta.rs`. Relay in a test: put the
  answer of the first persona with lines „@handle: …“ into the script, then one step per relay answer; check the order
  of the calls through `ScriptedModel::calls()` (`persona`). „schreibt …“ in a test with `Step::Wait` and
  `model.started().await`, then query `post.since`, afterwards `model.release()`. The live tables are process-wide per
  (work id, key): integration tests check only their own entries (`find`, remember the time `since`), absence only in
  unit tests with their own work id (`chat_live::tests`).
- Pure functions of the widgets (chat.ts `typingLine`/`freshFailures`/`deleteQuestion`, mention.ts
  `addressAt`/`filterAddresses`/`applyAddress`) under Node like `tests/quote_find.rs`, but with all `SCRIPTS` as `.mjs`
  in one folder (imports rewritten to `.mjs`), because chat.js imports `rpc.js` and `widgets.js`.
- Template macros do not see the page context: everything through arguments (`at = dict(...)` in the template). Single
  posts for `post.since` are rendered by `Environment::render_str` with `{% from … import post_entry %}`.

### Round 30 (integration)
- Acceptance `tests/round30_abnahme.rs`: a coast book (3 parts, 12 chapters, 48 sections, 6 personas) for all three
  sub-rounds. Interplay that only works after the merge: finding blocks in the chat need the real
  `finding_spans` – a list item of a persona answer is then a block of its own (`post-finding` with `finding_block`),
  no longer part of a shared `<ul>`; jump addresses from `check_marker` carry `&at=<Byte>` for a literal quotation.
- Scripted answers with „Kritisch:“ in chat tests need a solution line (see Round 30b), otherwise the
  follow-up request shifts all following steps – updated that way in `round30_bedienung.rs`.
- `finding_block(card, body)`: the chat hands its segment in as `body`, so that the finding text does not appear
  twice (test: text in the block exactly once).

## Typesetting settings (Round 29)
- Interface R29-S1: `baruch_typst::typeset::model` is frozen (changes only through integrator/PM). The only
  place of the typesetting rules is the hook in `baruch_store::snapshot::head`; a work without settings exports
  byte-identically. Test files `crates/baruch-server/tests/round29_*.rs` are already listed in `tests/main.rs`.

### satz-daten
- Store `src/typeset.rs` (table `work_typeset`, migration 0048), tests `crates/baruch-store/tests/{typeset,
  migration_0048}.rs`. Migration tests that compare the schema after `Store::open` (0040, 0041) take out
  `work_typeset` together with its index and triggers (0016–0018 compare only their own tables).
- Golden of the export without settings: `crates/baruch-store/tests/data/satz_golden_{export,main}.typ` from the state
  R29-S1 (source `satz_quelle.typ`, scheme „decimal“). Rewrite only deliberately with the ignored test
  `write_goldens_from_the_current_state` – and only when the export without settings changes on purpose.
- Tests "with settings" compute the expected export as the old export + `baruch_typst::typeset::rules(&s)` at the
  place where the first section begins (`export_map().sections[0].start`); so they stay valid whatever
  satz-typst writes into the rules (no fallback branches).
- Exchange: `baruch_exchange::typeset` (words, `TYPESET_ATTRIBUTES`, `full_sample` with all sixteen fields), the
  sample work carries typesetting settings (`sample_uuid(400)`) and a tombstone `satz` (`sample_uuid(401)`). Tests
  that build older minor versions from the sample take both out through `without_u6`.
- Server: `crate::typeset_settings` (RPC, `load`, `ADOPT_NOTE`, `NO_RULE`), `mirror::describe_typeset`; test
  `tests/round29_daten.rs` (RPC, commits with trailers and body, MCP, exchange over three instances).

### satz-typst
- Rules: `baruch_typst::typeset::rules` (`rules.rs`), template `template.rs` (`template_conflicts`,
  `detect_chapter_break`, `remove_rule`), preview `preview.rs`, EPUB `epub.rs`. Tests
  `baruch-typst/tests/typeset_{rules,template,epub}.rs`; the effect of every setting is measured by
  `baruch-render/tests/typeset_measure.rs` with `baruch_render::page_texts` (source as in the server: template, `rules`,
  `part_page_rule_with`, text).
- Measuring: lines = text pieces with the same baseline; footnote marks are pieces of their own of one character; a
  hyphen is a piece „\u{ad}“; right edges only from lines that end in a letter (punctuation hangs
  into the margin); blank pages have no text pieces and no `page_lines`.
- New rules that hang on the heading stand behind the chapter rule (Typst applies the show rule defined last
  first): so the mark `<baruch-satz-end>` lies before the break. Nothing that should hold for `counter(page).at(…)`
  belongs in the show rule of a heading (it would lie behind its start mark) – use the page background for that.
- Server: `typeset::prepare` only passes on `part_page` (`state.store.work_typeset`), everything else already is in
  `export_map`. The golden state "work without settings" in `tests/round29_typst.rs` is measured with `typeset.rs`/`cover.rs` from S1
  (output „gemessen:“).

### satz-oberflaeche
- Block „Satz“: `src/settings/work_typeset.rs` + `templates/settings/werk_satz.html` (registered in `settings::templates()`).
  Macros of the template get messages and hints as arguments (`notes(id, errors[id], hints[id])`),
  they do not see the page context. Always update new form fields in `settings_from_form`, `form_from_settings`,
  `formToSettings` (TS) and in the round trip test at the same time.
- Tests `tests/round29_oberflaeche.rs`: excerpt of the block with `satz_block` (up to `id="abgleich"`); preview tests
  fetch the images right after setting (process-wide cache with eight states) and use a work title of their own or
  values of their own per test (their own state). The success paths save/reset/accept are checked by the acceptance
  (`round29_abnahme.rs`), not by this file.
- Pure functions of the widgets (`formToSettings`, `debounce`, `Newest`, `xref.ts` `initialPage`) under Node as in
  `round30_live.rs` (all `SCRIPTS` as `.mjs`); compare `formToSettings` with `settings_from_form` through
  `serde_json::from_value::<TypesetSettings>` (JS writes `20` instead of `20.0`).

### Round 29 (integration)
- Acceptance `tests/round29_abnahme.rs`: measure the effect of the settings in the server with `typeset::typeset` (chapter pages
  through `page_of`, page sizes through `page_sizes_pt`) and `baruch_render::page_texts(&typeset.prepared.source)` in
  `spawn_blocking` – that is exactly the source of the PDF (`PdfOut.stand == prepared.stand`); the PDF itself with `pdfinfo`
  (page sizes) and `pdftotext` („S. n“). Always check page numbers and running heads on **all** text pages, not only
  on one: part and blank pages have at most three text pieces (`page.len() <= 3`).
- Running head/page number without a chapter rule: the mark `<baruch-satz-end>` then follows its heading in the query
  (same position) and must not count as "blank page up to the next heading" (finding of the integration, test
  `without_a_chapter_rule_every_text_page_keeps_number_and_head` in `baruch-render`).
- Measuring footnote marks: single text pieces of one character (the size of the superscript is not in `size_pt`).

## Idea map (Round 25)
- **Mind map since Round 34** (see "Round 34 – Mind map" at the end): `bild/{layout,svg}.rs` and `web/src/idea_map.ts`
  no longer exist; data, connections, nearness and groups still hold.
- Data: migration 0043 (`idea_link`, `idea_map_pos`, tombstone kinds `idea_link`/`idea_map_pos`), store
  `idea_links.rs`; server `src/idea_map/{mod,links,near}.rs` (RPC `idea.map` … `idea.map_pos_reset`, `build` delivers
  the whole map), `src/idea_map/bild/{mod,layout,svg,export}.rs` (page, form `/werke/{id}/karte/act`,
  outputs), `src/mcp/idea_map.rs`, `rag/neighbors.rs` (reads only stored vectors), widget
  `web/src/idea_map.ts`. Connections: every change one mirror commit; positions without a commit; nearness never stored.
- The DOM contract of the map (classes `im-*`, `data-node`, `data-x/y/r`) is read by widget and tests; whoever changes it changes
  `svg.rs`, `idea_map.ts` and the tests `round25_{bild,widget}.rs` together.
- `build` returns the applied filter with the defaults filled in; links of the page are built by `bild::filter_query`
  without default values. Tests: `tests/round25_{daten,bild,widget,abnahme}.rs`, store `tests/{idea_links,
  migration_0043,exchange_map}.rs`.

## Editor and ribbon (Round 18)
- Ribbon: `templates/fragments/edit_toolbar.html` (editor: `data-edit-toolbar hidden`; typeset page: without `hidden`, usable
  without JavaScript), formats from `src/ribbon.rs` (`formats(template)`, RPC `ribbon.formats`), widget
  `web/src/ribbon.ts`, style `static/ribbon.css`. Commands to the typeset view only through `web/src/ws_command.ts`
  (`baruch:ws-command`, `sendWsCommand`; cursor position `baruch:ws-caret`) – frozen; `wysiwyg.ts` executes them,
  nobody else changes `wysiwyg.ts` for that. Buttons carry `data-ribbon-cmd`, „Bild“ `data-ribbon-image` (opens
  `details[data-image-insert]` in `fragments/editor.html`).
- Cancelling typesetting: `baruch_render::Cancel` + `compile_cancellable` (`compile`/`compile_pdf` unchanged); server
  `typeset::cancel(work)`/`cancel_in`, RPC `preview.cancel`, form `POST /werke/{id}/satz/abbrechen`. Counters for
  tests: `typeset::runs_of(stand)`, `pdf::pdf_runs()`. GET of typesetting, page SVG and PDF with
  `guard::cross_site_get` → 403 without a run. `typeset::warm_fonts()` runs at the start of `serve` (`main.rs`). Typst
  cannot be interrupted hard (measurement in `baruch-render/tests/cancel.rs`); a cancellation through the world
  (delivering no more files) breaks comemo ("found differing return values") – do not try again.
- Typeset view settings per work: store `src/sheet.rs` (migration 0044 `work_sheet`, no mirror, no uuid; since
  Round 27 in the exchange format as `<satzbild>`, minor version 3), `baruch_wysiwyg::sheet_style_over`, server `src/sheet.rs` (`sheet_style_for`/`sheet_css_for`, RPC
  `work.sheet`/`work.sheet_set`), form `POST /einstellungen/werk/satzbild` (`settings/work.rs`), style in the HTML as
  `<style id="ws-style" data-work="W">` (the widget takes it over without RPC). Typeset view settings never change Typst,
  export and PDF.
- Migration tests 0040/0041 compare schema objects after `Store::open`: exempt a new table (like `work_sheet`) there in
  `without_0041`/`new_objects` and, for references to `work`, in `references_after`.
- Behaviour test without a browser (`tests/round18_satzbild.rs`, `HARNESS`): the compiled widgets run in Node 20 on
  a small DOM mock with a mocked `fetch`; if `node` is missing, the test reports „ÜBERSPRUNGEN“. If a
  widget needs new DOM parts when mounting, extend the mock, not the assertions.
- Image at the cursor position: `POST /werke/bilder/einfuegen` with `place` = `end|start|block:<n>|offset:<n>` and optionally
  `body`; path and revision in one transaction (store `assets.rs`).
- Acceptance: `tests/round18_abnahme.rs` (an invented work with all constructs; every block that the widget sends
  anew stays byte-identical for canonical text).

## Exchange between machines
- Through GitHub: commits on branches, feedback as issue comments.
- Push only after a „ja“ (yes) of the user; no tokens in commits, issues or documents.

## Tests (model: David, `~/Dokumente/Projekte/Psalm/server/tests`)
- **Every crate has its own tests**, and every crate is a library + a thin program: logic in `src/lib.rs`,
  `main.rs` only start and arguments – so server, store and mirror can be checked from `tests/`.
- **Integration tests per subject area** under `crates/<crate>/tests/<gebiet>.rs` (e.g. `revisions.rs`,
  `rpc.rs`, `pages.rs`, `library_search.rs`, `git_mirror.rs`, `cookies.rs`), shared helpers in
  `tests/common/mod.rs` (`mod common;`), e.g. `open() -> (TempDir, Store)` with a real SQLite file in the
  temp folder, sample work, helpers for RPC calls. Test data (Typst chapters, repos) in `tests/data/`.
- **`baruch-server`: one test binary** (since Round 13): every test binary links the server together with LanceDB
  (≈ 500 MB); hence `autotests = false` and `[[test]] name = "server"` with `tests/main.rs`, every file
  `tests/<gebiet>.rs` is a module there. **New test file:** create it and add `mod <gebiet>;` to `tests/main.rs`
  (otherwise `every_test_file_is_bundled` fails), helpers with `use crate::common;` (or
  `use crate::pgvector_support;`) instead of `mod common;`. No new standalone test binaries in the server.
  All modules run in **one** process: process-wide states (the mark „PDF entsteht“, the queue of the
  models, MCP sessions, the typesetting semaphore, index run times) are shared by all server tests. Hence: PDFs only under
  `common::PDF_SERIAL`; tests that fill the model queue or check its state begin with
  `let _turn = common::model_queue(module_path!()).await;` (only tests of one module at a time; among others
  `round10.rs`, `round10_models.rs`, `round15_anbieter.rs`, `round15.rs`); never set environment variables or change the working directory;
  assertions on process-wide lists (e.g. seen MCP clients) with `find`, never on the whole list.
  **Wait for the index after every change:** every change to ideas or text queues the index anew
  (`rag::notify`); even when only metadata change (e.g. the reference with `idea.move`), the
  background task replaces the row in LanceDB – delete, then write anew, not in one step. Whoever afterwards checks
  something that reads stored vectors (nearness, „passt zu Ast“, semantic search) first calls
  `rag::wait_idle(&server.state).await`; otherwise the vector occasionally is missing under load (Round 38,
  `round25_abnahme.rs`: 5 of 10 runs red with four test threads).
- **Real building blocks instead of dummies:** a real SQLite file, a real Git repo in the temp folder, the router through
  `tower::ServiceExt::oneshot`; only foreign services are mocked (MultiGPT, models).
- **Models in tests** (Round 10): never the real Eliezer, never a network except 127.0.0.1:0. Personas alone:
  `AppState::open_with(dir, Arc::new(MemorySecrets::default()), Arc::new(model.clone()))` with
  `assist::mock::ScriptedModel` (`tests/round10_personas.rs`). End to end with HTTP: `Arc::new(EliezerPort)` and
  `baruch_llm::mock::MockEliezer::start()`, address through `connection.set {kind: "eliezer", config: {url:
  mock.url()}}` (with a key, `connection.set_secret` afterwards, binding); answers with `mock.push(Reply::text(&[…]))`,
  check with `chat_bodies()`, `forbidden_paths()`, `context_fields()`, `max_active()`, `cancelled()`
  (`tests/round10.rs`). `AppState::open` has `NoModel`. Check every new test file beforehand:
  `grep -n 1234 crates/*/tests/<datei>.rs`. The queue in `models` is process-wide: tests of the same file
  share it (no assertion on `waiting == 0`; in the bundle see `common::model_queue` above). The default of the lector is fixed `@lektor`; tests in which
  the lector should follow the context first set `persona.set_lector {mode: "auto"}`.
  Since Round 15: connection per persona through `model.connection_set {name, kind: "eliezer"|"openai"|"ollama", url,
  external}` and `persona.backend_set {persona, connection, external_allowed?}`; the mock plays the kind with
  `MockEliezer::set_flavor(Flavor::Ollama | Flavor::Openai)` and reports context lengths with `set_context_length`.
  Context of the lector in a test: `ScriptedModel::with_reported_context(n)` or `persona.update {context_length}`;
  chunking/compressing with an own `ModelPort` that answers calls with „Zusammenfassung“ in `label` briefly
  (`tests/round15_lektor.rs` `Responder`, `tests/round15.rs`). Fact block without SWORD modules through
  `BibleInput::Given`, with modules through `baruch_sword::fixture` (`write`, `write_lexicon`) and `bible.set_dirs`,
  `bible.set_work`, `bible.set_work_sources`. The lector never calls the model in parallel (`max_active() == 1`).
- **Semantic search in tests** (Round 12): never Eliezer, never MultiGPT, never the user's pgvector. Without
  `rag::set_embedders`/`rag::catch_up` there is no embedder (the search reports `NO_EMBEDDER`, never a network). Embedder:
  `rag::set_embedders(&state, rag::eliezer_embedders(&state))` after `connection.set` to `MockEliezer` (512
  dimensions, `set_embedding_dimension`, concurrency with `embed_max_active()`), or an own
  `EmbedderFactory` with a fixed vector space (`tests/round12.rs` `Concepts`); afterwards `rag::catch_up` and
  `rag::wait_idle`. MultiGPT: `connections::multigpt::mock::MockMultigpt`. pgvector: `use crate::pgvector_support;` →
  `start("<aufgabe>")` (own podman container with the local image `docker.io/pgvector/pgvector:pg17`/`:pg16`,
  port only on 127.0.0.1, name `baruch-r12-<aufgabe>-<pid>-<n>`; tests running at the same time share one
  container, each gets its own database `rag_<n>` (`Pg::database`, `config()`); with the last `Pg`
  `Drop` removes only this container; otherwise an initdb cluster without pgvector, otherwise `ÜBERSPRUNGEN`). Never touch
  foreign containers or images, no `podman rmi`.
- **MultiGPT mocks** (Round 13; never the real MultiGPT, never SearXNG, never the internet):
  - `connections::multigpt::mock::MockMultigpt` (search, `document_info`, `read_document`): since Round 13 also
    `DELETE /mcp` (in `requests()` with `"method": "DELETE"`), page-wise `read_document` (`Behavior.pages`),
    delay per tool (`Behavior.tool_delays`), own `session_id`.
  - `websearch::mock::{MockWebMcp, MockSite}`: `web_search` (filter „site:“), `fetch_url` fetches only `*.example` from
    the local `MockSite` (`with_fixtures`: light lore, harbour chronicle, guest book with prompt injection, page over
    1 MiB, slow page); `set_results`, `tool_calls`, `configure(|b| b.search_delay = …)`.
  - `sync::mock::MockDocs`: all six document tools, `put_document`/`remove_document`/`set_url`,
    `documents(collection)`, `deny_write(Some(Deny::Forbidden | Deny::ToolError))`, `clear_requests`.
  - Every mock answers `DELETE /mcp` with 200. **One** MultiGPT for everything: the front `OneMultigpt` in
    `tests/round13.rs` passes every tool on to its mock.
  - **Session pool:** `connections::multigpt::Session` stores idle sessions process-wide per (address,
    token hash, Tokio runtime), at most two, tool list `TOOLS_TTL`. Every test has its own
    mock (own address), so tests never share a session. Assertions on the number of
    `initialize`/`tools/list` only hold within one test; a second run uses the session from the pool
    (no new `initialize`). `multigpt::expire_idle(url, Dauer)` ends idle sessions immediately (with DELETE).
    After use always `release` or `close`; whoever drops a session triggers a DELETE in the background.
  - Reconciliation in tests: `sync::wait_idle(&state)`; shrink limits with `sync::set_limits` (per data folder).
- **Building with LanceDB** (Round 12): `lance` needs `protoc` in the `PATH` (here `~/.local/bin/protoc`). The first build
  of the dependencies ≈ 15 min; the one server test binary ≈ 530 MB, `target/` after `cargo clean` and a full
  run ≈ 9 GB (before the bundling in Round 13: 61 server binaries of ≈ 500 MB each, ≈ 38–40 GB).
- MCP tests with a token: `Authorization: Bearer <secret>` and `ConnectInfo(SocketAddr::from(([192, 0, 2, 7],
  50000)))` (TEST-NET-1) as a foreign peer; token through `mcp_token.create`. Never a token in outputs
  or messages of tests (assertions check only for absence).
- **Version lists** of migrations are never pinned completely (`vec![101, 102, 103, 104]` breaks with the
  next migration), check them with `starts_with` or against `MIGRATOR.iter()`.
- **Migration tests** compare after a migrator up to their own version (`version <= n`) and open
  afterwards with `Store::open`; so they do not break on later migrations (`migration_0007.rs` to
  `migration_0016.rs`, `migrate_upto`; `migration_0009.rs` since Round 7, because 0010 changes the table `persona`).
- **Reproduce errors from the work:** first rebuild the flow through RPC as the user interface sends it
  (e.g. `wysiwyg.parse` → `wysiwyg.serialize` with the instructions of the widget → `outline.check` →
  `outline.save`), let the test fail **before** the fix and name the output in the report
  (`tests/round7_titles.rs`).
- Checks that search MCP tool names for `update`/`delete`/`create` (`outline.rs`, `revisions.rs`,
  `work_crud.rs`) exempt `idea_*`, `post_*`, `outline_*` and `asset_*` (`asset_delete` and others are requests,
  `outline.rs` checks that their description begins with „Antrag“) – ideas and posts are the intended exception. Add new
  MCP tools to the tool list tests `mcp.rs`, `round4.rs` and `mcp_ideas.rs`.
- **Create images in the test** (a small PNG with a CRC/zlib block in the test, SVG as text), no binary files in the repo.
- **Test names as a sentence** that describes the behaviour (`accepting_a_proposal_keeps_exactly_one_current`,
  `the_literature_list_shows_titles_without_javascript`), and every assertion with a reason in the text
  (`assert!(html.contains(..), "Vorschlag-Karte fehlt: {html}")`).
- **Check rendered pages** like David: concrete excerpts in the HTML (escaping, German, no leftovers of
  placeholders), also without JavaScript (forms to `/act`).
- **Unit tests** for small logic directly in the module (`#[cfg(test)] mod tests`), integration tests for flows.
- Run: `nice -n 10 ionice -c3 cargo test --workspace -j 4` in your own folder (worktrees: own `target/`, see AGENTS.md).
- **Team rounds with worktrees:** a shared `CARGO_TARGET_DIR` mixes the workspace crates of the
  worktrees (same artifact hashes), a run can then test foreign sources. Either a target folder of its own per
  worktree or, as in B1.5, `--config profile.dev.package.<crate>.codegen-units=<n>` per worktree;
  repeat the run in the main folder before merging.

## Round 19

### index

- Tests of the index worker with an own `EmbedderFactory` (pattern `Recording` in `tests/round19_index.rs`: records
  every call, vectors by keywords, modes „gelingt“, „scheitert“, „hängt“). Separate the queries of the search
  from index embeddings (`index_calls`): the search always embeds its query, only the index must embed nothing
  more after the withdrawal.
- After a search first wait for `rag::wait_idle` before you need the write lock (`rag.reindex` with
  `deferred: false`): every search silently queues `Prune` (and every 30 s `Shared`).
- A `rag::catch_up` without an embedder set beforehand binds **no** embedder in tests (own data folder)
  any more, unless the connection `eliezer` is stored; tests set the mock beforehand as before
  (`rag::set_embedders`) or store `eliezer` on `MockEliezer`.
- Tests set prefixes of the connection LanceDB through `connection.set` (since the merge with pgvector) or
  `Store::connection_set` (full set with `dir` and `embedding_model`), **before** the import (otherwise the first
  job embeds without a prefix).
- `MockDocs` (reconciliation) knows `upload_without_id`, `reject_empty` (content only from headings), `silent_cap`
  and `lose_upload_answer` (HTTP 502 after creating). The session pool remembers the tool list:
  change schemas (such as the `maximum` of `limit`) before the first run.
- To prove "fails before the fix", a helper script in the scratchpad turns the core lines back; after
  copying back (`shutil.copy2` keeps the modification time) touch the files with `touch`, otherwise cargo does not
  rebuild.

### pgvector

- Tests `tests/round19_pgvector.rs`; container tests through `pgvector_support::start("r19pgvector")` (own database per
  test, `pg.database`). A schema for the search path holds per database: `ALTER DATABASE <pg.database> SET search_path
  …` (never `ALTER ROLE` – all tests of the container share the role). Measure lengths in characters
  (`chars().count()`), the limits stand as `pgvector::MAX_*_CHARS`.
- Task prefixes in a test: `connection.set` now accepts `query_prefix`/`document_prefix`; what was embedded
  is shown by `FixedFactory::last` → `FixedEmbedder::calls` (texts per call).
- Groups of the map: without an embedder the nearness pairs arise only from equal keywords – this lets you build
  groups without LanceDB (`idea.create` with matching `words`). Naming needs a `ScriptedModel`
  (`AppState::open_with`) and `common::model_queue(module_path!())`; the answer in the script in the form
  „g<id>: Name“. Suggestions are process-wide per (data folder, work) – every test has its temp folder, so no
  overlap.
- Building blocks of the typeset view (`ws_command.ts` imports nothing): under Node write `ws_command.js` from
  `baruch_server::SCRIPTS` as `.mjs` and import it; if `node` is missing, „ÜBERSPRUNGEN“.
- In TypeScript do not write `\u2028`/`\u2029` escapes in regex literals through the Edit tool (it inserts the
  real characters, which end a regex literal); use `\p{Zl}`/`\p{Zp}` with the flag `u` instead.

### treffer
- Weak hits only through `baruch_rag::is_weak` (never your own thresholds); pages show them in `<details
  class="weak-hits" data-weak-hits>`, tests search for `data-weak-label`. Fixed vectors: an own `EmbedderFactory` with
  concept axes (model `tests/round19_treffer.rs`, `rag::set_embedders`, then `rag::catch_up`/`wait_idle`).
- Modules with hyphenation remnants only invented: `baruch_sword::fixture::hyphen_remnants(text, every)`; for the estimate
  to trigger, the whole words must occur in other verses (model `psalm()` in `round19_treffer.rs`).
- `bibles::module_note` remembers the result per conf file and modification time; tests with their own temp folder are
  independent of it.
- Lemma probe: `Lexicon::lookup_lemma`; a test lexicon with `StrongsPadding=false` shows that „G26“ does not hit
  without the probe.

### Integration
- Acceptance `tests/round19_abnahme.rs`: one mock embedder for all ways (`Space`: concept axes, recording,
  modes succeeds/fails/hangs) through `rag::set_embedders` – pgvector gets the same `EmbedderFactory` as LanceDB,
  so `texts().last()` after `rag.search` shows the query together with the prefix of the way asked. pgvector through
  `rag.search` instead of `open_way`: `connection.rag_set {ways: ["pgvector"]}`, `connection.check` and
  `connection.collections_set`, a table with `vector(4)` matching the dimension of the mock.
- The literature page shows at most `limit` hits, also from the library seeded at start (sample titles);
  assertions on a particular weak section only for one that is surely in the list.
- SWORD fixture: books with OSIS names (`John`, not `Joh`; `Ps` and `Mal` are the same), otherwise „Schlüssel … gibt
  es nicht“.
- Evidence block end to end: `store.set_persona_rag("@belege", true)`, `ScriptedModel` with one step,
  `persona.review` through RPC; the block stands in the last user message (`model.calls()`).

## EPUB (Round 20)

Common: test work `baruch_epub::sample::input()` (freely invented, PNGs generated in code), build and test only with
your own `target/`; EPUBCheck in tests only as a self-written script through `path_env`, never change the PATH of the
process; xmllint with `--noout --nonet`.

### epub

- EPUB tests in the crate: `cargo test -p baruch-epub` (without server, fast). Variations of the test work directly on
  `sample::input()` (append a section, extend the template, switch `footnotes`); keys in `sample::KEYS`.
- Large, incompressible images (e.g. for the 8 MiB limit of `epub_export`): `sample::png_with(w, h, |x, y|
  …)` with noise; three images of ≈ 3.3 MiB each (1050 × 1050) stay each under `MAX_ASSET_BYTES`.
- Well-formedness: write every `.xhtml`/`.ncx`/`.opf`/`.xml` from the ZIP into a temp folder and
  call `/usr/bin/xmllint --noout --nonet` (`tests/package.rs`), otherwise quick-xml and „ÜBERSPRUNGEN“.
- A work without a section in a server test: delete revisions and nodes directly in SQLite (pattern `outline_mcp.rs`).
- Check equal bytes: compare RPC `work.epub {inline: true}` with the download (same state).

### reader

- Reader profiles in `baruch_epub::readers` (slugs frozen), CSS per profile in
  `crates/baruch-server/static/readers/<slug>.css` – a new profile needs both and a branch in
  `epub_preview::reader_css`; `baruch-epub/tests/readers.rs` checks that there is exactly one file for every slug.
- Reader CSS without `<` and `&` (it stands as `<style>` in XHTML) and without addresses; night mode behind `/* @night */`.
- Reader files in a test: `GET /werke/{w}/epub/leser/{slug}/{href}`; `href` from `work.epub` (`files`). The S1 build
  does not output images – check `epub_preview::reader_file` directly on an `EpubBook` with an own `EpubItem`.
- Reproduce sub-requests from the frame in a test: `sec-fetch-site: cross-site` + `host` + `referer` to a
  reader file of the same work.
- Visual check (integrator/user only, never in parallel with tests): throwaway server following "Building and checking", photograph
  the page `/werke/{w}/epub/vorschau`; images and `book.css` must load in the frame. Firefox shortens the
  `Referer` from `sandbox` documents to the origin (integrator R20). Take a **fresh** Firefox profile for every photo
  (otherwise the cache shows old responses) and give the image path **absolutely**
  (`--screenshot` with a relative path writes nothing). The headers of the sub-requests can be seen through a small
  logging Python intermediary on `127.0.0.1:0` in front of the throwaway server.

### epub-pruefung

- Testing the own check: `crates/baruch-epub/tests/check.rs` builds the test work (`build` + `package`) and rewrites
  the ZIP itself for every broken variant (`zip::ZipWriter`, entries in order, `mimetype` optionally DEFLATE);
  OPF/XHTML are changed as text (`replace`). Every variant checks exactly its code (`codes::…`). Add new codes to
  `baruch_epub::check::codes` and to `docs/RPC.md` (epub.check).
- A valid test work = no finding with `Severity::Error`; hints (`NCX_MISSING`, `A11Y_METADATA_MISSING`,
  `IMG_ALT_EMPTY`) are allowed. If R20a changes the test work or the build, `the_valid_sample_has_no_error` must stay
  green – otherwise the build or the check is wrong.
- xmllint: tests take it only through `find_xmllint(std::env::var_os("PATH"))`; if it is missing, „ÜBERSPRUNGEN“ for the
  xmllint assertions; the fallback to quick-xml has a test of its own (a path that does not start).
- Never a real EPUBCheck in tests: a script `epubcheck` (`#!/bin/sh`, executable) in a temp folder, passed as
  `path_env` (`find_epubcheck(Some(dir), jar)`, in the server `epub_check::run_check(…, Some(dir))`); a marker file in the
  script shows whether it ran. A hanging script (`sleep 30`) with a short time limit checks the cancellation including
  grandchildren. Never set the PATH of the process. If a real `epubcheck` is in the PATH of the machine, the tests skip to
  „nicht gefunden“ (`ÜBERSPRUNGEN`).
- Jar setting: test files `epubcheck.jar` with any content suffice (Java is never started); MCP must have
  no way to it (test through `tools/list`).
- Leftovers of the book data: count commits with `git rev-list --count HEAD` in the mirror (`round20_pruefung.rs`), order
  with `git log -2 --format=%s` („Umschlag entfernt“ before „Bild …: gelöscht“). Cover sources and spine only under
  `common::PDF_SERIAL` (Typst).
- Golden states R26/R27 (`baruch-typst/tests/cover_layout.rs`, `round27_satz.rs`) stay unchanged: text rules of the
  work template are only added when the template sets more than `#set text(lang: "de")`.

### Integration

- End-to-end acceptance following the pattern of `tests/round20_abnahme.rs`: import a work, images/cover/book data/
  typesetting settings through RPC, a cross-reference through `add_proposal` + `revision.accept` (keys only known after
  the import). Rebuild broken EPUBs from the real bytes with `zip::ZipWriter`; for "broken XHTML" choose a file with
  `</p>` (part pages have no paragraph).
- CLI against a data folder only once the `AppState` in the test is released again (block), then compare bytes.

## Round 31 – Index and glossary

### Interface R31-S1

- Mark in the text: `#metadata((register: "…", term: "…"))<baruch-idx>` (`baruch_typst::index::mark_source`,
  `parse_mark`); test helper for the round trip in the typeset view: `crates/baruch-wysiwyg/tests/index_mark.rs`.
- `term_key` is stored in the database: never change it. `sort_key` is never stored.
- Export, map and mirror stay byte-identical without R31 settings (`BackMatter::is_active`); whoever appends the end of
  the book uses `snapshot::append_tail` (map = export).
- Server test files: `round31_glossar.rs` (a), `round31_register.rs` (b), `round31_kontextmenue.rs` (c); the acceptance
  `round31_abnahme.rs` is created by the integrator.
- The three MCP tools were STUB in S1; implemented since R31a, the exception in `tests/mcp.rs` is gone.

### 31a glossar-daten

- Store `src/glossary.rs`, migration 0049, tests `crates/baruch-store/tests/{glossary,migration_0049}.rs`; migration tests
  0040/0041/0048 take the 0049 objects out (`R31_OBJECTS`). Test works freely invented.
- Accepting an index proposal hangs on `baruch_typst::index::mark_word` (register-satz). Tests check with a
  probe (`mark_word(…)` → `IndexError::NotYet`?) the honest S1 behaviour or afterwards the real acceptance
  (`glossary.rs::accepting_an_index_proposal_is_a_new_human_revision_or_stale_without_change`,
  `round31_glossar.rs::mcp_only_proposes_and_the_human_decides`).
- Exchange minor version 7: sample work (`baruch_exchange::sample`) with two indexes (`sample_uuid(1110/1111)`), three
  terms (`1100`–`1102`) and tombstones `glossar-begriff`/`register` (`1103`/`1112`). Tests that build older
  minor versions from the sample take the u7 parts out through `without_u7` (contained in `without_u6`).
  `docs/schema/baruch-werk-1-u6.xsd` is frozen (SHA-256 in the test).
- Server test `tests/round31_glossar.rs` (RPC with commits and trailers, MCP, `glossary.suggest` with `ScriptedModel`
  under `common::model_queue`, exchange over two instances with xmllint against u7). Older server tests expect
  `version="7"` or the u7 XSD.

### 31b register-satz

- Marks and projection: `baruch_typst::index` with private submodules `index/{scan,project,back}.rs`; tests
  `crates/baruch-typst/tests/index_{marks,back_matter}.rs` (text), measured in typesetting in
  `crates/baruch-render/tests/index_measure.rs` (`page_texts`: page number = digits in the footer of the page on which
  the marked word stands; index lines = text pieces of one baseline, separated by columns). Typeset without hyphenation
  (`#set text(hyphenate: false)`), otherwise keywords fall apart.
- Source in the measuring test as in the store: template + `rules` + per section `Projector::project` + `back_matter`. In
  several columns Typst flows into the second column only when the first is full – create many keywords for the column test.
- Store: `crates/baruch-store/tests/index_export.rs` computes the expected export from the old map (sections through
  `Projector`, behind them `back_matter`); glossary terms come empty from the store until glossar-daten, the glossary page
  then says „Keine Begriffe.“.
- EPUB: `crates/baruch-epub/tests/index.rs` projects `sample::input()` itself (in the server the sections already come
  projected from `export_map`); check links of the indexes to `baruch-idx-{key}-{n}` against the files.
- Server: `tests/round31_register.rs`; PDF only under `common::PDF_SERIAL`, text with `pdftotext` per page (`\x0c`).
  `typst` is not in the PATH on the development machine: the test reports „ÜBERSPRUNGEN“ and additionally typesets the
  export with the embedded compiler (`baruch_render::page_texts`) as a standalone file.

### 31c kontextmenue

- Mark in the typeset view: `span.ws-index-mark` without a text node (characters only in the style); whoever changes the HTML updates
  `render_index_mark` (baruch-wysiwyg), `fillMark`/`markOf` (wysiwyg.ts), `markLabel` (ws_command.ts) and the tests
  `baruch-wysiwyg/tests/index_mark.rs` and `tests/round31_kontextmenue.rs` together.
- TypeScript counterparts of `baruch_typst::index` (`markSource`, `parseMarkAt`, `validateMark`) and `index_mark_label`
  (`markLabel`) are checked by `round31_kontextmenue.rs` under Node against the Rust functions; every change there updates the TS.
- Page tests against tasks that may still be missing: first ask through RPC whether the method reports „noch nicht umgesetzt“
  (`not_yet`), then check exactly this state (message with kept inputs or the path to the display) – so the tests
  stay valid before and after merging 31a/31b.
- New form fields of the typesetting block: `index_register` (multiple) is one field per checkbox; `block_context` needs
  the Vec of the form pairs for that (the `BTreeMap` `v` keeps only one value per name).
- Return addresses built by the server (`back`) in templates with `|safe` and `&` → `&amp;` beforehand (minijinja writes
  `/` as `&#x2f;` otherwise); `glossary_page::back_to` follows only paths of this server.
- `round29_oberflaeche.rs` still exempts the three R31 fields from the label check (note to the integrator: the
  exception can now go; the check is also in `round31_kontextmenue.rs`).

### Integration

- Acceptance `tests/round31_abnahme.rs`: a work with 70 equal paragraphs in the middle chapter, so that a keyword runs over
  several pages (range „3–6“ measured against pdftotext); `fill()` creates glossary and marks exactly as the
  user interface sends them (forms without JavaScript or RPC like the dialogs). Page number = index of the pdftotext page + 1
  with `PageNumberStart::FirstPage`.
- „siehe auch“ of a mark with a subentry stands in the line of the subentry behind the page number
  („Leuchtturm 2; siehe auch Nebel“).
- An export carries identifier labels (`<kN>`, `#metadata("baruch-key")`) behind headings and parts that are missing in the
  import file: check byte identity without R31 settings against the export before, not against the
  import file.
- Import of an export with a book end: `strip_back_matter`/`strip_projection` (unit test in
  `baruch-typst/tests/index_back_matter.rs`, end to end in the acceptance).

## Round 21 – Import and images

### Interface R21-S1

- Dependencies are fixed since S1 (`zip`, `quick-xml` 0.38, `pulldown-cmark` 0.13 without default features in the server,
  `typst-render` 0.15.1 in baruch-render; typst-render could be loaded, the replacement resvg/image was not needed).
  Developers change no Cargo.toml/Cargo.lock; needs go to the integrator.
- Read ZIP only through `baruch_server::archive::read_zip` (limits `IMPORT_LIMITS`). Build test archives in the test with
  `zip::ZipWriter`; the writer does not allow equal names – in the unit test the second name is rewritten in the bytes
  (header and central directory, same length), special files through the external attributes (offset 38 in the
  central directory, mode in the upper 16 bits), because `unix_permissions` takes only the permission bits.
- Hints of an import as `import::ImportWarning {code, message, line}` in `ImportReport.warnings`; codes begin
  with `md-`, `archive-` or `import-`. PDF/EPUB create the work only through `import::import_generated`.
- Server test files: `round21_pdf_import.rs` (a), `round21_markdown_import.rs` (b), `round21_bilder.rs` (c), in S1 only
  with a head; the acceptance `round21_abnahme.rs` is created by the integrator. No S1 branches in tests (lesson R31).

### 21a pdf-import

- Create test documents only in the test: PDF through `baruch_render::compile_pdf` from invented Typst under
  `common::PDF_SERIAL` (Typst default: level 1 sets larger than level 2 – the local heuristic reads the levels from that),
  EPUB by hand with `zip::ZipWriter` (mimetype STORED, `container.xml`, OPF, `nav.xhtml`, XHTML), PNG through
  `baruch_epub::sample::png`. Symlink in the archive with `ZipWriter::add_symlink`, too large an entry as STORED noise
  over 16 MiB, a bomb as 8 MiB of spaces DEFLATE.
- The local PDF way needs Poppler (`pdftotext`, `pdfinfo`); tests check that with `poppler()` and otherwise report
  „ÜBERSPRUNGEN“. "pdftotext missing" without environment variables: `pdf_import::cli_import_with`/`preview_with` with
  `PdfTools { pdftotext: <nicht vorhandener Pfad>, … }`.
- Check "no silent switch": `pdf_import::local_pdf_reads(&state)` counts pdftotext runs per data folder; after an
  error of MultiGPT it must stay 0.
- MultiGPT mock with `mock::full_tools()`, document through `configure(|b| { b.upload_pages = …; b.upload_toc = …;
  b.index_polls = 1; })`; check calls through `tool_calls("upload_document" | "read_document" | "run_status")`
  (`page_from`/`page_to` in the arguments). Never change `default_tools()`.
- Drafts lie under `<data>/import-drafts/<32 hex>/`; test age/count by having the test create folders with
  `draft.json {"created": <Sekunden>}` (the cleanup reads only `created`) and then calling `source_import.preview`.
- Forms in the test as multipart by hand (boundary, `Content-Disposition`); 413 with a file over
  `pdf_import::UPLOAD_LIMIT`.
- `/quelle?title=…` shows the local full text only once `literature.rs` uses `fulltext::has_fulltext` (integrator);
  until then the tests check the full text through RPC `source.read {title}` and `highlight.add`.

### 21b markdown-import

- Conversion only in `src/markdown_import/convert.rs` (`to_typst`, pure); unit tests there for every form. A new form =
  a row in the table of `docs/Architektur.md` („21b markdown-import“) and an `md-…` code, never discard silently.
- Test texts in Rust strings: a line continuation `\` at the line end swallows the indentation of the next line –
  write indented Markdown lines (nested lists) with `\n  - …` in the **same** source line.
- Build archives in the test with `zip::ZipWriter` (`tests/round21_markdown_import.rs`, `zip_of`); large images in the archive from
  pseudo-randomness, otherwise `read_zip` takes them for a zip bomb (ratio > 200). Folder upload = several parts `files`
  with a relative path in the `filename` (that is how browsers send it with `webkitdirectory`).
- Paths in the report: minijinja writes `/` as `&#x2f;`, `'` as `&#x27;` – assertions on HTML accordingly.
- Lines of the hints are 1-based in the **Markdown** file (not in the generated Typst).
- Round trips export → import in the test: titles come from `#set document` – pass `title` on re-import, otherwise
  „Titel vergeben“; comparisons with `baruch_typst::xref::strip_key_labels` on both sides.
- `numbering_scheme.rs`/`round14_numbering.rs` (foreign) check under „Ohne Nummern“ only that the `=` line stays; since
  21b the show rule of the fallback for `numbered = 0` additionally stands there in the template of the export.
- A foreign test that hangs on the old form (message to the integrator): `tests/work_crud.rs:424` expects the old
  file field `<input type="file" name="file" accept=".typ" required>`; since 21b `accept=".typ,.md,.markdown,.zip"` without
  `required` (the folder is the alternative) plus `<input type="file" name="files" multiple webkitdirectory>`.

### 21c bilder

- Server tests in `tests/round21_bilder.rs`; store in `crates/baruch-store/tests/assets_r21c.rs`. Test images with
  `baruch_epub::sample::png(w, h, [r, g, b])`, SVG as text; large contents (8 MiB) = PNG signature + filler bytes (the type
  is recognised only by the first bytes).
- The legacy `/x.png`/`x.png` can no longer be created through the store: rebuild it in the test with `sqlx::query("INSERT INTO
  asset_path …")` on `server.state.store.pool()`. The age of decisions likewise with `UPDATE
  asset_request SET decided = strftime(…, 'now', '-31 days')` (no protective trigger); `asset_revision.created` on the
  other hand is immutable by trigger.
- Quota in the test: one MCP session of its own per client (`clientInfo.name` → `@mcp:<name>`), counting is done through
  `Store::asset_mcp_usage`. 30 small `asset_add` are fast; for 64 MiB eight `asset_replace` of 8 MiB each (database
  only, no Git).
- Measuring PDF typesetting: `pdfimages -list` (columns page, num, type, width, height); if it is missing → „ÜBERSPRUNGEN“. `pdfinfo`
  reports the Suspects line for every Typst PDF (cause in Architektur „21c bilder“); tests compare against a
  copy with `/Xuspects false`.
- Measuring the EPUB cover: `image::load_from_memory` on `OEBPS/images/cover.png`, comparison with
  `baruch_render::render_png` on a source with only the cover image (same page size → same pixel size);
  block frame in mm × (width / page width). Everything with Typst under `common::PDF_SERIAL`.
- Whoever changes the EPUB cover: server tests that count the file list always see a `cover` at the front since R21c.

### Integration

- Merging pdf-import → markdown-import → bilder with `--no-ff` ran without conflicts; the integrator only updated
  foreign tests that were hit by changes according to the contract: `migration_0048/0049.rs` check the highest
  library migration only as "≥ 107" (0108 was added – whoever adds a migration checks this way, not for
  equality), `round12_multigpt.rs` compares with `multigpt::rank_score(2)` instead of 1/rank, `work_crud.rs` with the
  new file field (`accept=".typ,.md,.markdown,.zip"` without `required`, plus `name="files" multiple webkitdirectory`).
- `/quelle?title=…` asks `literature::fulltext::has_fulltext` (MultiGPT link **or** local full text from
  0108), no longer only `title_remote`. Check: accept an EPUB with `target: "library"`, then the text stands on the
  page and not `data-fulltext="none"`.
- Acceptance `tests/round21_abnahme.rs` (8 tests, „Der Apfelgarten am Deich“, freely invented): read lines of the outline from
  the preview page through `name="src" value="…"`/`name="line" value="…"` and repeat them as a form (order
  `line`, `src` per item). Prove the EPUB cover without coordinates: rasterise the same work with `cover.layout_set` and one
  invisible block each and compare pixels (title, subtitle, author provable individually).
- Re-import of an `export --out` folder: pack the folder as a ZIP and read it in on a **fresh** data folder
  (`common::open()`) through `work.import_archive`, otherwise the taken title refuses; comparison
  `strip_key_labels(export_typst)` and `format!("{:?}", export_assets)`.
- `markdown_import::cli_import` prints hints in the report on **stdout** (`Hinweis (<code>, Zeile n): …`), the
  Typst import (`main.rs::import`) on stderr.
- `cargo fmt --all` after the merge: the branches were not formatted throughout.

## Round 22 – Revisions and mirror

### fassungen

#### Revisions

- **Amending (Round 22):** only `outline.save`/`revision.save` with `mode: "auto"` amend (store
  `outline_save_mode`, `save_human_mode`/`save_human_with_reason`); if `mode` is missing, everything stays "new revision" – old
  tests and clients need nothing. Every step is exactly one mirror commit (`Baruch-Action: amended|amend-undone`,
  `Baruch-Amend`), undo only as a new step (`revision.amend_undo`), never delete history rows.
  New writing paths keep creating new revisions until they explicitly get `SaveMode::Auto`.
- Time limit in a test without sleeping: `revision.created` is immutable by trigger – in the test briefly take out the trigger
  `revision_origin_immutable`, reset `created`, recreate the trigger literally
  (`crates/baruch-store/tests/revision_amend.rs`, `age_revision`); decisive is max(creation, last amendment).
- `change_percent` counts whitespace as equal parts (a completely new sentence ≈ 50 %); rewrite test texts for "large change"
  clearly, for "small change" insert one word.
- Reading side branches `refs/revisions/<key>/<n>`: `state.mirror(work).await?.side_refs("refs/revisions/")`,
  `read_file(name, pfad)`, `parents(name)` or `git log <ref>` in the mirror folder. A new proposal is not a commit;
  its reference is created by `mirror::side_refs_after_proposal` right after creation (all proposal paths in the server).
- Checking addresses in delivered pages: minijinja masks `/` as `&#x2f;` and `&` as `&amp;`; convert back before searching
  (`round22_fassungen.rs`, `readable`).

### befund-rueckfrage

- Round 22 (befund-rueckfrage): tests `tests/round22_befunde.rs` (MockEliezer under `common::model_queue`: context of the
  renewed review through `chat_bodies()`, follow-up question, mandatory finding, form „Ja/Nein“, state through SQL
  `UPDATE revision SET amend_seq`), `tests/round22_archiv.rs` (create → delete → `/werke/archiv` →
  restore, symlink in the archive, `mirror::sync_all` for ideas without a work), store `tests/works_restore.rs` and
  the state tests in `tests/findings.rs`. Persona answers with „Erledigt: Befund n“ in a test only for findings
  of the same persona; `mirror_ideas` is private – check it in a test through `mirror::sync_all` or `work.delete`.
- `/werke/archiv` and the history of a revision are in `tests/round9_menu.rs` (`all_pages`, now 47 pages).

### spiegel-entwurf

#### Round 22 – spiegel-entwurf

- Draft: rule `baruch_typst::typeset::draft_rule` (tests `typeset/draft.rs`, `baruch-render/tests/draft.rs` with
  `page_texts`); server `typeset::prepare_with`/`effective_draft`/`cover_draft`. Tests `tests/round22_entwurf.rs` under
  `common::PDF_SERIAL`. **pdftotext and watermark:** the header/footer is set letter-spaced (pdftotext separates the
  letters with spaces) – remove all whitespace before searching; pdftotext splits diagonal text between the
  lines of the book text, so check the typesetting source (`prepare_with`) with `baruch_render::page_texts` there.
- Without a draft the typesetting source is exactly `export_map().typst` (the test compares directly); PDF comparisons through the `sha256` of the
  RPC response (`inline: true`).
- Exchange u8: sample with u8 parts `baruch_exchange::sample::sample_document_u8()`; `sample_document()` stays u7
  (older tests build files of the minor versions 1–7 from it). XSD check `docs/schema/baruch-werk-1-u8.xsd`; older
  server tests keep checking their outputs against u7 (valid without u8 parts) and expect `version="8"` since R22.
- Store tests `crates/baruch-store/tests/exchange_u8.rs` (fast-forwarding, approving/rejecting a request with an identifier); server
  `tests/round22_abgleich.rs` (two instances, approval with a mirror commit, `<kN>` rule, setting the mirror with
  `Store::mirror_snapshot` + `baruch_render::compile`).

### Integration

- Acceptance `tests/round22_abnahme.rs`: amending three times, undo through the form, „Als neue Fassung speichern“,
  large change, time limit (aged revision, setting through the form), `policy_off`; persona revision → `not_own`,
  side branch right after an MCP proposal; follow-up question after ↻ repeat with MockEliezer, „Ja, erledigt“ through the form;
  watermark with date measured per page (pdftotext, pages separated by `\x0c`, remove whitespace), cover,
  EPUB, byte-identical without; exchange between two throwaway instances with fast-forwarding and a partial request; archive;
  `<kN>` rule through export → import.
- `node.get` carries `amend_seq` per revision since the integrator (before only `revision.tree`).

## Round 23 – Hardening

### web

- **Behaviour tests of the widgets without a browser.** The DOM mock is in `tests/round18_satzbild.rs` (`DOM`) and is
  usable for every test file through `run_harness(scene, &scenario)`. The scenes of this round are in
  `tests/round23_web/widgets.mjs` (additions: `FormData`, `<dialog>`, `DOMParser`, history, `requestSubmit(submitter)`,
  fields with `form="<id>"`; mocked network for RPC and pages). A scene is chosen by `scenario.scene`; pure functions
  of a module are called by the scene `pure` (`module`, `calls`). For trying quickly without `cargo`: compile widgets with
  `web/node_modules/.bin/tsc -p web/tsconfig.json --outDir <ordner>` and call `node harness.mjs scenario.json
  file://<ordner>`.
- **Check the mock before you believe it.** The selector "descendant" (`a b`) never matched in the mock until Round 22
  (the ancestor fell away during parsing); `:checked` was missing. Both are fixed. If a `querySelector` in the mock
  matches nothing, first check the selector in the mock.
- **Typst remembers function calls.** A recursive `fib(n)` is finished immediately in the embedded Typst (comemo); for
  a test that needs computing time, use a loop (`for i in range(1500000) { s += i }`).
- **Feature unification.** `cargo test -p <crate>` rebuilds the dependencies with other features (long wait);
  for single server tests use `cargo test --workspace --test server -- <filter>` (the test target is called `server`).
- **Process-wide counters in tests.** Keep counters for "how often" (e.g. `cover_print::contrast_decodes`) per key
  (checksum) and use unique data in the test; all server tests run side by side in one process.
- **Protection of the working tree.** The protection of the worktree refuses shell calls with a here document in which a word such as `commit` occurs;
  create helper scripts as a file in the scratchpad and call them.

### mcp-namen

- MCP tests with names: a client without a token that is named like a token writes as `@mcp:lokal-<handle>`. Tests that mix a token and a client of the same name without a token must expect that.
- A token with the name of a revoked one can no longer be created through RPC and form; tests take a new name or `Store::mcp_token_create` (legacy without the handle check).
- Image quota in a test: all sessions without a token share one quota per work. Whoever wants to check "another client keeps writing" needs a token for that (`mcp_token.create`, foreign peer with `Authorization: Bearer`).
- Testing upper limits without creating hundreds of commits: create one real entry and multiply it with `WITH RECURSIVE … INSERT … SELECT` (pattern in `tests/round23_mcp_namen.rs`).
- Real peer for `/mcp`: `axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())` on `127.0.0.1:0` and `reqwest` with `no_proxy()` (pattern `listen`/`Wire` in the same file).
- `ChatOrder` has the mandatory field `relay` (user interface `false`, MCP `true`); write `PostRequest` in tests with `..Default::default()`.

### geheimnisse

- `baruch_llm::scrub` is the one place for redacting; new paths call it, never `text.replace(secret, "***")`.
- Mock for redirects: `Recorder` in `crates/baruch-llm/tests/redirect.rs` or `round23_geheimnisse.rs` (raw
  TCP server on 127.0.0.1:0 that remembers the requests; two of them: the first answers 30x pointing to the second).
- Web search against 127.0.0.1 in a test: `websearch::fetch::set_fetch_exceptions(&state, &["127.0.0.1"])`, afterwards
  empty it again. The mocks use `*.example` and do not need that.
- `baruch_exchange::sample::sample_document()` carries chapters with `==` under parts; since Round 23 every
  reading in of this sample reports hints for that in `deviations` (assertions on `deviations` with `find`/`any`, never on
  "empty").
- The reader is strict: a file with `version="1"` must not carry parts of later minor versions (a
  lowered `version` on the current output is refused).

### Integration

- Acceptance `tests/round23_abnahme.rs` uses the helpers of the three tasks (`round23_mcp_namen::{mill, local, remote,
  create_token, …}`, `round23_geheimnisse::{with_models, redirecting}`; `pub(crate)` for that). New acceptances may
  share such helpers instead of copying them.
- Pages outside the work page show the default work without `?work=` (badges in the menu): tests on badges
  append `?work=<id>`.
- `own_reads` in a test: send `Sec-Fetch-Site: cross-site` **with** `Sec-Fetch-Mode` and `Sec-Fetch-Dest`; only
  `Sec-Fetch-Site` alone does not count as a subresource (older tests imitate the link from outside that way).
- A test that expects "another client without a token keeps writing" is wrong since Round 23 (shared
  quota); `round21_bilder.rs` now expects the refusal.
  Since the follow-up this also holds for `add_post` (300/24 h), open proposals (100) and the model jobs
  (all clients without a token together four, `quota::MAX_WITHOUT_TOKEN`; per name still two): a test that needs more than
  four hanging model jobs through loopback without a token takes tokens (`store.mcp_token_create`, request with
  `Authorization: Bearer …`). A test imitates the legacy from before Round 23 with `store.mcp_token_create` (without
  the handle check), see `round23_mcp_namen::two_old_tokens_that_only_the_new_rule_would_merge_stay_apart`.
- Screenshots with theme and JavaScript (integrator R23): with JavaScript the theme widget reads `document.cookie`;
  a front server that only sets the `Cookie` header of the request therefore yields the light theme with JavaScript. The
  front server additionally sets the cookies as `Set-Cookie` in every response, and every variant (theme, with/without
  JavaScript) gets a Firefox profile of its own (cookies hold per machine, not per port); before the actual photo
  load any page once. Without JavaScript: `user_pref("javascript.enabled", false);` in `user.js` of the profile.

## Round 24 – Distribution

### pakete

- **Version in one place.** The source is `[workspace.package] version` in `Cargo.toml`; scripts read it through
  `scripts/version.sh`, the program through `about::VERSION`. It is repeated only in `site/hugo.toml`
  (`params.version`), `README.md` and `CHANGELOG.md`; `tests/round24_abnahme.rs` and `scripts/ci-site.sh` report a
  deviation. Only the user changes the number.
- **Check a package without installing it:** `scripts/build-deb.sh` (release build, about 12 minutes the first time),
  `dpkg-deb -I/-c`, `dpkg-deb -x paket.deb <Probeordner>`, then start the server from it with `HOME` and `XDG_*` on empty
  folders (create the data folder with `import` beforehand, connections to `127.0.0.1:9`, see "Building and checking").
  Never start the launcher `baruch` without arguments: it takes port 8765 and the default connections.
- **AppImage locally:** `packaging/appimage/build-appimage.sh` downloads appimagetool and the runtime with a checksum to
  `target/appimage/tools/` (no system installation); `--appdir-only` works without a network. Probe folders and images
  belong to `/home` (e.g. `~/.cache/baruch-runde/`), not into the RAM file system `/tmp`: the binary has 340 MB.
- **Cross check for Windows/macOS** fails locally at the first C dependency (`zstd-sys`, no cross compiler).
  What works without C: `cargo check --target <Ziel> -p keyring` and `-p baruch-epub`.
- **Do not switch environment variables in the test process** (all server tests share one process): check branches that
  hang on a variable (`APPIMAGE` on the about page) through a pure function with a parameter, or by hand.

### ci

- **Before the commit:** `nice -n 10 ionice -c3 cargo clippy --workspace --all-targets -j 4 -- -D warnings` is green
  (the CI treats warnings as errors; an `#[allow(clippy::…)]` needs a reason), plus
  `cargo fmt --all -- --check`, `cargo deny check licenses sources bans` and `scripts/ci-selftest.sh` (without a network:
  helper scripts and rules of the workflows).
- **Pinned checksums:** `crates/baruch-rag/tests/port.rs` pins the SHA-256 of `baruch-rag/src/lib.rs` and
  `Cargo.toml`. Whoever changes one of the two files (even only an `#[allow]`) updates the constant in the same
  commit (`sha256sum`).
- **Changing workflows:** actions only with a 40-digit SHA (`git ls-remote <Repo> 'refs/tags/<Tag>*'`), values and
  secrets only through `env`, never `${{ … }}` in `run:`; `scripts/ci-check-workflows.sh` checks the rules. Tools
  with version and checksum are in `scripts/ci-tool.sh`. `actionlint`/`shellcheck` are missing on the machine.
- **No workflow runs without the user:** no tag, no push, no "Run workflow".
- **Archive key and fingerprint (follow-up):** `secrets.APT_GPG_*` only in a job with
  `environment: release` (rule 10 in `scripts/ci-check-workflows.sh`). The fingerprint stands in `site/hugo.toml`,
  `README.md` and `docs/Verteilung.md`; `scripts/ci-fingerprint.sh` checks the three places (exit 3 = still open) and
  with `--key` against the key. Never enter, guess or fetch a fingerprint from the network – not even
  "for testing"; tests take invented values in a temp folder (`--root`).
- **Assertions on placeholders:** a test must not nail a value that the user enters later to "empty"
  (`round24_abnahme` did that with `aptFingerprint` and would have failed upon entering it): check "empty or
  valid".
- **Nothing private or foreign in the tree:** machine name, login name, names and book titles of third parties belong in no
  file; `round24_abnahme::the_current_tree_names_no_private_machine_and_no_third_party` searches for them (search words
  assembled in the test). Sample title in tests and docs: the invented „Mövenoog“.

### webseite

- **Hugo:** `hugo --minify --panicOnWarning -s site` (0.168.0, not "extended" – no SCSS). `languageCode` is deprecated since
  Hugo 0.158 and aborts with `--panicOnWarning`; `locale = "de"` is set instead. The output `site/public/` is
  ignored. The site works without JavaScript and without foreign addresses for style, font or image.
- **Screenshots of the website:** `site/tools/bildschirmfotos.py --pruefen` (without Firefox) or `--fotos` (integrator
  only, never in parallel with test runs); invented work `site/tools/beispielwerk.typ`.
- **Reproduce a proxy in a test:** a request to the router with the `Host` header that the proxy passes through, and
  `ConnectInfo` loopback (`round24_betrieb::{open_allowing, mcp_ping}`); set `AppState::allowed_hosts` after opening.
  Names invented (`*.example.org`).
- **Look up foreign assumptions:** "`PGSSLROOTCERT` makes sqlx verify even with `require`" stood in the backlog and in the
  contract and was wrong (sqlx 0.8.6 verifies only with `verify-ca`/`verify-full`, source under
  `~/.cargo/registry/src`). Read the source before documenting.

### Integration

- **Tests that hang on a default:** the empty MultiGPT default made `round12_lancedb` fail (it expected
  an opened way "with the default address"). If a task changes a default, first
  `grep -rn "<Funktion>" crates/*/tests` and name the places in the report.
- **`cargo fmt` after the merge:** new files of the branches were not formatted throughout; the integrator
  formats once (`cargo fmt --all`) before clippy and the test run.
- **Photographing the website:** deliver `site/public` through `python3 -m http.server --bind 127.0.0.1 <Port>` (the
  links of the site are absolute from `/`). Dark theme: an own Firefox profile, in `user.js`
  `user_pref("layout.css.prefers-color-scheme.content-override", 0);` **and** `user_pref("ui.systemUsesDarkTheme", 1);`
  (the first line alone stayed light headless); narrow: `--window-size=420,2600`. Images with `loading=lazy` are missing
  in the photo (it is taken at the `load` event): for the photo take a copy of `site/public` and remove
  `loading=lazy decoding=async` there. The same settings also act on Baruch itself (without a theme cookie
  the user interface follows the system).
- **Checking internal links of the website:** through the built HTML files (every `href`/`src` without a foreign host
  must lie as a file under `site/public`, anchors as `id`); `/apt/` only arises at the release.

## Round 33 – Ribbon and laptop mode

### Band (`templates/base.html`, `src/band.rs`, `web/src/band.ts`)

- **The ribbon stands completely in `base.html`.** The template is embedded in about 35 minijinja environments of
  their own (`include_str!`); an `{% include %}` in it would have to be registered in all of them. So new parts of the
  ribbon go directly into `base.html`, pages only deliver blocks (`band_tab`, `page_actions`, `band_einfuegen`,
  `band_pruefen`, `band_buch`, `band_ansicht`).
- **Literal forms:** the layer `consent::inject` → `band::apply` replaces strings in the finished HTML (tab,
  field, `… data-band-group open>`, laptop switch, side rails). Never change the order of the attributes; further
  attributes of a group only between `class="band-group"` and `data-band-group`.
- **New page:** set `{% block band_tab %}` and put the actions as groups in the literal form into the block of its
  own tab. `round33_seiten::every_page_names_its_default_tab_and_keeps_its_actions_in_groups` has the list.
- **Modes only through attributes on `<html>`** (`data-laptop`, `data-compact`, `data-narrow`, `data-rail-*`, `data-band`):
  no media query on the window width, otherwise there are two truths as soon as the human chooses „An“/„Aus“.
  `band.css` owns the ribbon in both states, `laptop.css` everything outside it.
- **Work in the ribbon:** order of precedence `topbar` → `work.id` (only if `work` is a map) → cookie `baruch_work`. Pages
  that pass `work` as a string (e.g. `/einfaelle`) show „Prüfen“/„Buch“ only with the cookie – tests fetch such
  pages like the browser with `baruch_work=W`.
- **Test helpers:** `band(html)`, `panel(head, id)`, `tabs(head)` (round33_menueband.rs, round9_menu.rs); the head is called
  `header.band`, no longer `header.topbar`.

### Integration

- **Tests in foreign files:** each of the three tasks left red tests in files of another task (old
  label „Satz“, removed `.topbar` rules, the former ribbon of the typeset page). That is to be expected with strict
  file ownership: the developers deliver the text proposal (file, place, old → new), the integrator enters it. A
  test over "all groups of the page" must know the groups from `base.html` after the merge (group
  „Darstellung“ with the theme button).
- **Order:** visual check before the final full run (but never at the same time as a run): it found
  three things that changed CSS (cropped group label, wrapping of row 1 at 1024 px, „Einklappen ›“ at the
  frame). After the full run that would have meant a second run.
- **Firefox headless without a click:** `--screenshot` cannot click anything and cannot set cookies. Help: a small
  intermediary (Python, `http.server` on 127.0.0.1:0) in front of the throwaway server that adds cookies per request
  (`Cookie` to the server **and** `Set-Cookie` to the browser – the theme widget reads `document.cookie` and otherwise
  resets), sets `<details … open>` (without JavaScript) or inserts a `<script>` that clicks a `summary` after `load`
  (with JavaScript; band.ts collapses groups opened on the server side again in the compact ribbon).
  Without JavaScript: a profile with `user_pref("javascript.enabled", false);`. Drawer without JavaScript: address with
  `#pane-tree`.
- **podman slow:** `timeout 20 podman info` answered, the pgvector tests still needed over ten minutes
  (start and `podman rm`). Do not abort – the run ends; only leave them out when `podman info` hangs.
- **Parallel rounds:** Round 33 ran beside Round 32 on an integration branch of its own (`runde33`, main folder
  `.claude/worktrees/r33`). The name deviates from the rule `team/<id>`; the places for the later merge
  are marked in `base.html` with `{#- R32-PLATZ … -#}` (import-word, beenden, export-word).

## Round 32 – Word and quitting

### Word (`crates/baruch-docx`, `docx_import`, `docx_export`)

- **Two sides, one model:** reader and writer arose separately against `model.rs` and `ooxml.rs`. What really
  connects both is shown only by the round trip: `crates/baruch-docx/tests/roundtrip.rs` (model → file → model) and
  `round32_abnahme` (work → file → work). Whoever changes something in `write/**` or `read/**` runs both.
- **Test files:** `crates/baruch-docx/tests/data/import/mod.rs` builds a freely invented .docx as a ZIP from
  XML strings (`sample()`, `with_body(…)`, helpers `p`, `r`, `ins`, `del`, `list_item`, `drawing`, `cell`); the
  server tests include it once (`round32_docx_import.rs`: `#[path = "…/data/import/mod.rs"] pub(crate) mod docx;`,
  other files with `use crate::round32_docx_import::docx;` – clippy refuses `#[path]` twice as `duplicate_mod`).
  No real Word.
- **Lists in Typst:** `baruch-wysiwyg` reads „3.“ followed by „+“ as two lists (Typst itself as one). Whoever generates
  Typst that the editor and the Word export should see as one list writes out every number for a start ≠ 1
  („3.“, „4.“ …).
- **Image path:** the writer stores the path in the work in `wp:docPr name`; the reader only takes it when it looks like a
  path (`read/body.rs`, `work_path`). Word names drawings „Grafik 1“ – then `bilder/<Dateiname>` applies.
- **Viewing a file without Word:** `soffice -env:UserInstallation=file://<eigener Ordner> --headless --convert-to pdf
  --outdir <Ordner> datei.docx`, then `pdftoppm -r 70 -png`. That is no Firefox and no window; it shows whether
  LibreOffice opens the file and how changes, footnotes, lists and tables arrive. It does not replace Word.

### Beenden (`shutdown.rs`)

- **Set up signals early:** `tokio::signal` sets up a receiver only when `signal(…)` is called, an
  `async fn` even only at the first await. `system_signal()` therefore sets up at the call, and `main` calls it before
  the line „Baruch läuft“. A test that sends `SIGTERM` right after this line
  (`sigterm_ends_the_throwaway_server_with_exit_0`) found the gap; with a write request before it, it stayed
  hidden.
- **Real process in a test:** `Process` in `round32_beenden.rs` and `round32_abnahme.rs` – create the data folder in the test process,
  connections `eliezer`/`multigpt` to `http://127.0.0.1:9`, `pool().close()`, then
  `CARGO_BIN_EXE_baruch-server serve --listen 127.0.0.1:0 --secrets memory --data <Temp>`; read the address from „Baruch läuft: “;
  end only through your own PID (`kill -TERM <pid>`), `Drop` cleans up.
- **`Host` in a test:** `reqwest` sends a `Host` header of its own (`.header("host", NAME)`) – that is how you check an
  allowed proxy name on the real process.
- **Guard `round33_seiten`:** every template that extends `base.html` needs a row in `TEMPLATES`. Whoever builds a
  page delivers the row in the report (the file belongs to the integrator).

### Integration

- **Visual check:** fetch a page that only arises as a response to `POST` and loads nothing afterwards (final page) with
  `curl -X POST -H "Origin: <Adresse>"` and photograph the file. Pages with stylesheets cannot be photographed
  from `file://` (the protection against a foreign origin refuses the stylesheets) – use the intermediary
  from Round 33 for that (here: menu „Baruch“ with `open`).
- **`.work-form` has a base height of 320 px** (`works.css`): a page with only a small form sets
  `flex: 0 0 auto` (like `assets.css`, `docx_export.css`), otherwise an empty box stays below the button.

## Round 34 – Mind map

- **Where things are:** data `crates/baruch-store/src/ideas.rs` (migration 0052: `title`, `parent_id`, `position`,
  `main_topic`), tree/cloud/import `src/idea_map/tree.rs`, picture `src/idea_map/bild/{mod,mindmap,tree_layout}.rs`,
  input/output and outline view `src/idea_map/files/`, widget `web/src/mindmap.ts`, templates
  `karte.html`, `karte_gliederung.html`, `karte_einlesen.html`.
- **Four places of an idea:** parent node → child node; otherwise reference → at the chapter; otherwise `main_topic` →
  main topic at the work; otherwise the cloud. `idea.create` without a place puts it into the cloud; "onto the work" needs `cloud: false`.
- **Page, picture and outputs read `tree::Scene`** (`tree::scene`), never `tree::tree` alone: otherwise the cloud is missing.
  `mindmap::render` gets the cloud through `RenderOptions::of(scene, …)`. Whoever builds a tree by hand
  (`round34_widget.rs`, `round34_austausch.rs`) names the identifiers of the cloud themselves.
- **Only the seam checks assumptions in parallel branches:** two branches had made the same assumption about the cloud
  against the interface, the third built it differently; all three runs were green on their own. When
  merging, therefore first search the reports for "assumption" and write an end-to-end test across the seam
  (`round34_abnahme.rs`) before foreign tests are adjusted.
- **Widget without a browser test:** `mindmap.ts` runs in no test. What is checked is which RPC it calls with which fields
  (e.g. `idea.delete` with `confirm: true`), and the picture afterwards. Whoever changes the widget compares the parameters
  with `ideas.rs` and makes a visual check with a **probe**: the front server (see "Photos after a click")
  inserts a script that sends events – `click` on `.mm-box`, `keydown` **to the chosen node**
  (`.mm-node.mm-selected`; on `document` the mind map listens only for Strg+Z, Alt+E, Alt+L), dragging as `pointerdown`
  on the node and `pointermove`/`pointerup` on `window` (bring the target into the window with `scrollIntoView` first,
  otherwise `elementFromPoint` finds nothing) – and reports the result back through `fetch`; read the state afterwards through RPC.
- **Photos of the mind map:** `/werke/{id}/karte` (with `zu=<Kennung>` for a collapsed branch), outline view
  `/werke/{id}/karte/gliederung?wahl=i:<id>`; the preview of reading in is a POST response (save it with `curl -F file=@…`,
  insert `<base href>`, photograph the file).
- **Exchange format:** minor version 9; a work without an ordered tree still passes the frozen u8 schema.

## Round 35 – Accounts and roles

- **Where things are:** accounts/sessions/lockout `crates/baruch-store/src/accounts.rs` (0053), shares `members.rs`
  (0054); login `src/auth/` (`gate` = layer, `password`, `session`, `throttle`, `routes`, `rpc`, `cli`);
  permission check `src/roles/` (`mod.rs` with `LEKTOR_ROUTES`, `LEKTOR_RPC`, `check_request`, `check_rpc`; `rpc.rs`,
  `lektor.rs`, `routes.rs`); pages and menu entry `src/konten_ui/`; templates `anmelden.html`, `einrichten.html`,
  `konto.html`, `mitwirkende.html`, `settings/konten.html`, `fragments/lektor_*.html`; style `static/konten.css`.
- **New route or RPC method:** for Lektoren it is locked until it stands in `roles::LEKTOR_ROUTES` or
  `LEKTOR_RPC` – that is intended. Add it only when a Lektor needs it: read-only or one of the
  allowed writing paths, never anything that touches a model, a connection or the file system, always with
  a work binding (`WorkFrom`) and a reason in `docs/Architektur.md`. `round35_schreibwege` and `round35_abnahme`
  count routes and methods from the source (`common::registered_routes`, `common::rpc_method_candidates`) – whoever
  registers a route so that the helper does not find it (no `.route("…", get(/post(`) bypasses the guard.
- **Forms of the modules do not go through `rpc::call`** (`/beitraege/act`, `/werke/befunde/act` …) and are therefore
  closed to Lektoren. A form for Lektoren posts to `POST /lektorat/act` (field `method` from `LEKTOR_RPC`).
- **Who is asking?** `auth::current()` (task-local; outside a request `Who::Open`). Never evaluate it in `tokio::spawn` or
  `spawn_blocking` – fetch it beforehand and pass it along. Origin when writing: `roles::author_name(&who)`.
  `AppState::default_work()` is for Lektoren the smallest shared work; `store.default_work()` stays the
  smallest id (use it only for system purposes).
- **Templates:** `nur-autor` is always the **last** class (`class="btn nur-autor"`), groups of the ribbon carry
  `data-nur-autor`; pieces only for Lektoren stand between `<!--NUR-LEKTOR-->` … `<!--/NUR-LEKTOR-->`, pieces only
  with accounts between `<!--NUR-KONTEN-->` … `<!--/NUR-KONTEN-->`. `konten_ui::apply` clears everything away without accounts –
  the page must then be as before except for „Konten einrichten…“ (`round35_oberflaeche`, `round35_abnahme`).
  Hiding by CSS is only convenience; security lies in `roles` alone.
- **Tests:** identity without login through `common::with_who(request, who)` (whole router) or `auth::scope(who,
  …)` (one call); accounts for foreign keys through SQL with the hash `$argon2id$test`. The real path (setup →
  cookie) only where it is checked (`round35_konten`, `round35_abnahme`): `ConnectInfo` with loopback **and**
  `Host: 127.0.0.1:…` = own machine; `Host` = proxy name (`state.allowed_hosts = AllowedHosts::parse(…)`) = through
  the proxy. Without `ConnectInfo` a request never counts as the own machine. Never print a password or token in an
  assertion message. Let a lockout expire in a test: `UPDATE login_attempt SET blocked_until = …`.
- **Assumptions at the seam:** the three branches built against stubs of the others (a Lektor through the whole router was
  checkable in no branch). Errors of the seam were found only by the visual check with real accounts: „Zurück zum Werk“ on pages
  without a work of their own pointed to the default work. So with roles look at every page also **without** `work` and with an
  **old cookie** `baruch_work`.
- **Visual check with accounts:** throwaway server as above; create accounts with `curl` directly at the server (`/einrichten`
  from 127.0.0.1, cookie into a file, delete it afterwards). The intermediary adds the cookie
  `baruch_session` per request, passes `Host` and `Origin` on with the address of the server (otherwise 403 from
  `guard::same_origin`) and logs `/rpc` with method and error code – so you see whether a widget for Lektoren
  calls a locked method. Dark for the pages without a ribbon (login, setup): a profile with
  `user_pref("ui.systemUsesDarkTheme", 1);`. Menu „Baruch ▾“ open: `<details class="menu" data-menu="baruch" open>`.

## Round 36 – Leftovers (Typst command line, sqlx 0.9)

- **Typst command line:** arguments `crates/baruch-server/src/typst_cli.rs` (`dispatch` runs in `main` before the
  Tokio runtime, `run` writes only into `out`/`err` and can therefore be checked without a process), typesetting
  `crates/baruch-render/src/cli.rs`. Trying it: `target/debug/baruch-server typst compile x.typ x.pdf` or a
  symlink `baruch-typst` to the binary. Exit 0/1/2 fixed (success / typesetting or file error / wrong call or
  unsupported). No package creates a file `typst` (`round36_typst`). CI: `scripts/ci-typst.sh
  <baruch-server>`.
- **sqlx 0.9:** `query`, `query_as`, `query_scalar`, `begin_with` take `&'static str`. Assembled text only
  through `sqlx::AssertSqlSafe(format!(…))` and only from fixed building blocks of the source (rule in the module comment of
  `baruch-store/src/lib.rs`); always bind values. The same holds in tests (`sqlx::query(AssertSqlSafe(sql))` for
  a `String`). `Migrator { … }` as a struct expression needs `..Migrator::DEFAULT`. Guards:
  `round36_sqlx` (exactly one sqlx version in the lock, no exception RUSTSEC-2026-0459 in `deny.toml`),
  `sqlx09_migrations` in store and library (checksums as under 0.8). Before the commit now
  `cargo deny check` with all four checks (advisories needs a network).
- **Merging a rebuild across the workspace** (sqlx09 last): first bring `development` into the branch
  and run `cargo check --workspace --all-targets` there (the compiler reports every new place), then in the main folder
  `git merge --no-ff --no-commit`, full run, only then the commit – so a half rebuild never stands on
  `development`.
- **Checking "the same PDF":** typst-pdf without a timestamp is deterministic; `export --pdf` and
  `baruch-typst compile` on the file from `export --out` are equal byte for byte as long as the work has no
  cover and no front matter (`round36_abnahme`).
- **Lektoren:** display names come into the text of the origin elements through `konten_ui::names`; without accounts
  the output stays equal byte for byte. `settings::resolve_work` lets the cookie `baruch_work` count for Lektoren only for
  a shared work.
- **One-time note in the process** (`settings/data.rs`, „Sicherung angelegt“): holds per process and data folder,
  not per session; tests with their own data folder do not disturb each other.

## Round 37 – Help on the website

### hilfe_text (`site/content/de/hilfe/`, `docs/Bedienung.md`)

- **Usage = help page.** Whoever changes the user interface changes in the same commit the page under
  `site/content/de/hilfe/` and – for routes, cookies, parameters, commits – the "Technical notes" in
  `docs/Bedienung.md`. The rules stand there under "Rule for developers"; `round37_hilfe` enforces them.
- **Writing a help page:** every heading with `{#anker}`; every `##` with `{{< hilfebild … >}}` or an entry in
  `ohne_bild`; no raw HTML and no HTML comment (Hugo aborts with `--panicOnWarning`); angle brackets only
  in code (`` `<name>` ``); links only from `/` and only to existing anchors; keys as „Strg+S (Mac: ⌘S)“ in
  the same or the following line. Forbidden are among others „Runde “, „Revision“, „Akkordeon“, „Cookie“, „Commit“,
  „RPC“, „RAG“, „docs/“, `` `/ `` (code that begins with a slash) and the routes of the user interface.
- **Tests that read docs:** search visible texts of the user interface in the help page, technical matters in
  `docs/Bedienung.md`. Headings of the help end with the anchor – `"\n## Menüband {#menueband}\n"`, not
  `"\n## Menüband\n"`.
- **Proving Mac keys:** `grep -rn 'metaKey\|ctrlKey\|altKey' crates/baruch-server/web/src`. `ctrlKey || metaKey`
  → ⌘; Alt shortcuts work on the Mac only when the code evaluates `event.code` (⌥ otherwise produces another character).
- **Worktree protection:** the protection refuses commands with `git` in pipes or with a here document ("too complex");
  put longer checks as a script into the scratchpad and call them one by one.

### webseite (`site/content`, theme, `scripts/site-check.py`)

- **Fixed anchors instead of generated ones:** Hugo forms anchors from the heading (`#macos-nicht-signiert` never existed,
  because the heading was called „macOS“). Every heading gets `{#anker}`; `scripts/site-check.py` finds dead
  anchors after the build.
- **Checking anchors promised in parallel:** if a branch links to anchors that another one only delivers, build a copy
  of `site/` in the scratchpad with invented sections including these anchors and check there – this proves
  that on your own state only what was promised is missing.
- **Hugo, section templates:** `layouts/hilfe/page.html` holds only for pages under `content/de/hilfe/`;
  `layouts/section.html` for the start page of the section. Show the table of contents only when it has entries:
  condition `in .TableOfContents "<li"` (so no box appears on the placeholder pages without a heading).
- **Marking the menu entry on subpages:** `IsMenuCurrent` only matches the page itself; for the subpages
  of a section `$seite.IsDescendant .Page` at the menu entry (only if `.Page.IsSection`).
- **The sandbox refuses here documents with certain content** ("too complex to verify"): create files with the
  write tool, put longer replacements as a Python script into the scratchpad and run it.
- **`ci-selftest.sh` builds probe sites without the help section:** checks that presuppose the real site
  need a switch (`site-check.py --ohne-hilfe`, set by `ci-site.sh` when `content/de/hilfe` is missing) –
  and a counter-check that the check really applies with the section.

### bilder (`site/tools/bildschirmfotos.py`, `site/data/hilfebilder.toml`)

- **Findings without a model exist only through a Lektor account:** `finding.add` and `revision.propose` are reserved for
  Lektoren (-32000 for everybody else). So for photos: create accounts last, log in as a Lektor, write
  findings; the images with findings then show the logged-in author (session through the intermediary).
- **Posts and proposals of an assistant role without a model:** MCP at your own server from 127.0.0.1 without a token,
  `add_post` or `propose_revision` with `persona: "lektor"` → author `@lektor`. `thread.post` and MCP `chat` call
  no model as long as the message mentions no assistant (`answers` and `errors` stay empty).
- **Throwaway server and Bible modules:** without a stored list Baruch reads `$SWORD_PATH`, `~/.sword` and
  `/usr/share/sword` – that is, the modules of the user. For photos set `bible.set_dirs` to an
  empty folder (with `mods.d`) right after the start.
- **Previews of reading in are partly pages, partly responses:** `POST /werke/import/word/vorschau` answers with
  303 to the page of the draft, `POST /austausch/vorschau` (file field `datei`, not `file`) and
  `POST /werke/import/quelle/vorschau` with 200. The report after `POST /werke/import` appears only when there is something
  to report (such as a missing image in a Markdown file), otherwise 303 to the work.
- **Intermediary for photos as part of the tool** (`site/tools/bildschirmfotos.py`, class `Relay`): recipe per
  shot through `_bild=<Aufnahme>` in the address; session only towards the server; `Set-Cookie` for the other cookies
  also to the browser; rewrite Host/Origin/Referer (counter-check: the same request directly → 403). Everything except the
  effect of click and measurement can be checked without Firefox.
- **Dark checkable in the HTML:** with the cookie `baruch_theme=dark`, `<html>` carries `data-theme="dark"`.
- **Worktree protection and shell:** the protection refuses compound commands with a here document as "too complex";
  put helper scripts into the scratchpad with the write tool (name with the task id) and call them one by one.
- **Dummy instead of Firefox:** whoever wants to rehearse `--fotos` without Firefox first checks with `command -v firefox` that
  the dummy stands first in the path **and is executable** – `shutil.which` skips non-executable files and
  then finds the real browser.
- **Templates mask slashes:** minijinja writes inserted addresses as `https:&#x2f;&#x2f;…`; tests
  that search for a link with an inserted address first replace `&#x2f;` by `/`.
- **`| head` on a Python tool:** catch `BrokenPipeError`, otherwise `--liste | head` ends with an
  error message.

### Integration

- **Merging:** after the interface S1 there was no shared file; three `--no-ff` without a conflict. If a
  branch name already existed (`team/bilder` from Round 21c, `team/webseite` from Round 24), check beforehand which
  branch belongs to the round (`git worktree list`, `git merge-base`).
- **Photos show paths:** „Einstellungen → Daten“, „→ Quellen“ and „Über Baruch“ name the data folder, the module folder and
  the default under `$HOME`. For images that are published: `--temp /var/tmp/baruch-hilfe` (disk, neutral),
  copy the Bible modules there; otherwise the tool aborts `--fotos` and gives the throwaway server a `HOME` of its own.
  After the photos look at every image (contact sheets with `montage`) and search the PNG files with `grep -a` for your
  own name.
- **`target/release/baruch-server` can be old:** the tool took it first and photographed an old
  user interface (here: 404 for the Word export). Since the PM close of Round 37, without `--server` it takes the file
  built last and names it; whether it matches the source it does not check – build first after changes.
- **`firefox --headless --screenshot`:** takes the top of the page, an anchor in the address changes nothing (take a tall
  window plus a measured crop); images with `loading="lazy"` are missing in the photo although they
  arrive in the browser – for the visual check of the website photograph a copy of `site/public` without `loading=lazy`.
  Dark: `ui.systemUsesDarkTheme = 1` in `user.js` of the throwaway profile.
- **Proofread alt texts after the photos:** they arise before the images. Deviations were empty states (reconciliation,
  cross-references, MCP access without a token), the time keeper (book instead of sermon) and everything that a crop does not
  show.
- **minijinja:** `{{ wert | default('') }}` only replaces missing values; an `Option::None` is output as „None“.
  For form fields `default('', true)`.
- **Quotation and reference:** the wording check assigns a quotation only when between quotation and reference there is only whitespace,
  brackets and punctuation – `„…“ (#bibelstelle[Psalm 119,105])` is not assigned, `„…“ (Psalm 119,105)`
  is.
- **Test target:** `cargo test -p baruch-server --test server …` (not `--test main`; `Cargo.toml`: `name = "server"`,
  `path = "tests/main.rs"`).


## Round 38 – English, comment translations, work language

Since Round 38c the whole file is English.

- **Comment-only changes:** `python3 scripts/check-comments-only.py BASE HEAD [PATH…]` compares every changed file
  without its comments (Rust, TypeScript, templates, Python, shell) and exits 0 only if nothing but comments
  changed; doc-test code blocks and intra-doc link targets must stay equal. `--umlaute PATH…` lists German letters
  left in comments; exceptions go into `scripts/comment-umlaut-allow.txt` (glob, substring, reason, separated by
  TAB; the integrator maintains it). `--selftest` runs in `scripts/ci-selftest.sh`.
- **Merging a comment branch:** first merge `development` into the branch. In a conflict take code and strings from
  `development` and the English comment from the branch, then run the check between `development` and the branch
  (exit 0) before merging with `--no-ff`. A doc line that starts with an intra-doc link and a colon
  (`` [`NAME`]: text ``) is not a link definition; the check knows that since the integration of Round 38.
- **Lessons of Round 38b (rest of the comments):** one developer manages about 1,500 to 1,900 umlaut lines
  (3,000 to 3,800 comment lines) including one full run; measure with the scanners of the check tool and split by
  path, never by line range. Two bracket characters in code spans with prose between them (`` `[` and `]` ``) are
  not an intra-doc link; the check knows that since the integration of Round 38b (a target neither starts nor ends
  with a blank). Before translating a file, grep the tests for its name: `crates/baruch-rag/src/lib.rs` is pinned
  by a SHA-256 in `crates/baruch-rag/tests/port.rs` (`LIB_SHA256`), so a comment change there needs the new hash
  in the same commit and is the integrator's job. Run `--umlaute --allow /dev/null` on the own paths: a broad allow
  entry hides untranslated lines. Allow entries are narrow – path of one file, the quoted text as substring; a
  single word as substring (`Prüfen`, `Löschen`) would also hide an untranslated line with that word. A stop-word
  scan over the comments (German words without umlauts outside „…“ and backticks) finds what the umlaut check
  cannot. German `//` comments inside JavaScript that a test carries in a raw string are string content: they stay
  until the string itself is changed.
- **List continuation in doc comments:** keep the list structure (`- `, `1. `) of a doc comment when translating;
  clippy `doc_lazy_continuation` fails otherwise.
- **Work language in tests:** set it with `store.set_work_language(work, WorkLanguage::En)`; the model is
  `assist::mock::ScriptedModel`. Assert the prompt through `lang::language_rule(language)` and the markers through
  `lang::markers(language)`, never through copied wording. Tests with German model answers must keep their answer
  texts: for `de` every behaviour is as before Round 38.
- **Exchange schema:** tests that validate current output use `docs/schema/baruch-werk-1-u10.xsd`; the older
  schemas are frozen (`crates/baruch-exchange/tests/xsd.rs`).

## Round 39a – User interface in two languages

- **Catalogs and keys:** how to localise a text is in `docs/Bedienung.md`, "Localising a text (Round 39a)"; the
  scaffold in `docs/Architektur.md`, "Round 39a". Never `|safe` on `t()`; markup only through `_html` keys.
- **Tests and the language:** the harness `common::router` pins requests without a language to German, so older
  tests keep their German assertions. A template rendered outside a request (unit test, direct call of a page
  function) is English: wrap it in `i18n::sync_scope(Lang::De, …)` or `i18n::scope(Lang::De, …)`. A real server
  process in a test gets `Accept-Language: de` explicitly. MCP tests assert the English text.
- **Widget catalog in every page:** `<script id="baruch-i18n">` carries every `widget.*` key with its text in the
  page language (about 25–30 KB). Checks for raw keys, for absent words or for page size cut that element out
  first (`widget.ideas.source` contains the key `ideas.source`). Node harnesses of widgets inject the German
  catalog (`i18n::widget_catalog_json(Lang::De)`) when they assert German text.
- **Template pitfalls:** a loop variable named `t` hides `t()` inside the loop; a plain key's text is escaped, so
  `/` appears as `&#x2f;` in raw HTML (keys whose text needs a literal slash for an older assertion were made
  `_html` keys without markup).
- **Merging catalog branches:** conflicts in `scripts/i18n-literal-allow.txt` are resolved as a three-way set merge
  against the S1 commit (a base line stays only if no branch removed it, added reason lines are kept);
  `round39a_i18n` then proves it (every finding listed, no stale entry).
- **What is still German:** `tests/round39a_pages.rs` walks every GET page in en and de; its `KNOWN_GERMAN` names
  every German piece left on English pages with its source. Fixing one means removing its entry.

## Round 39b – Book outputs in the work language, English website

- **Book words:** every word Baruch writes INTO a book output comes from `baruch_core::book_text::book_t(work_language,
  …)` (`crates/baruch-core/locales/{en,de}/book.ftl`), never from the interface catalogs. Old functions stay as the
  German case and get an `_in` twin with a `WorkLanguage` (`of_scheme_in`, `draft_rule_in`, `frame_parts_in`,
  `typeset::draft_rule_for_in`, `verify::check_in`, …); a server call site that builds book output from a work must
  use the `_in` form with `crate::work_language::of(state, work)`. Recognition (reading back) accepts both languages.
- **German stays byte-equal:** golden fixtures under `crates/*/tests/data/round39b/` were written once with the S1
  code; never regenerate them (`BARUCH_BLESS_R39B` was only for that first commit). A failing golden test means German
  output changed.
- **Hugo, two languages:** 0.168 rejects `languageName` (deprecated, fatal with `--panicOnWarning`); use `label`. Use
  `hugo.Sites.Default` instead of `site.Sites`. English pages have the German path under `content/en/`; picture
  paths go through the partial `bildpfad.html`, and a missing light picture is a build error in both languages, so the
  English pictures (`bildschirmfotos.py --fotos --lang en`) must exist before `ci-site.sh` passes.
- **Exceptions of the page walk:** a `KNOWN_GERMAN` entry may hide several sources with the same words (Round 39b:
  "über EPUB" stood both for the reader choice and for the reader warnings of `baruch-epub` `check.rs`). Remove an
  entry only after the page walk passes without it.
