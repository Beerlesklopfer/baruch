# Baruch – free writing workshop for books and sermons

## Context

The author's books are revised today through MultiGPT chats (chat 59): attach a PDF,
get criticism, rework chapters one by one. Structure gets lost on the way, contexts overflow
(Eliezer HTTP 400, fixed in `958b796`/`e37814d`), and there are no revisions per section.

Decisions of the user (2026-11-05):
- **New repo „Baruch“** (private `Beerlesklopfer/baruch`, free later), a redesign, not integrated into
  MultiGPT; undocumented at first.
- ~~Rust, native desktop user interface (Iced)~~ → **Rust server with a browser user interface like David**
  (2026-11-05): the core is an RPC server that delivers the skeleton and the templates; widgets in TypeScript.
  Loco (Rails-like, axum + SeaORM + Tera, workers, JWT) checked and rejected for now: RPC is missing there
  too, the ballast does not fit a single-user setup; check again later for the Bible school (login, workers).
- **Import through MCP to MultiGPT** (2026-11-05): Baruch reads sources just like MultiGPT (the same
  preparation, important for research); no import parsers of its own. PDF, EPUB, MOBI etc. must be displayable in
  Baruch.
- **Literature management like Citavi** (2026-11-05): sources from RAG and the web as titles with quotations;
  citations in the text through Typst `@cite` (candidate: `hayagriva`, the bibliography format of Typst, with CSL styles).
- **SQLite stays the source** (2026-11-05, confirmed): works, revisions, chat, personas and the whole
  **author library** (titles, quotations, citations) live there; the LLM searches in it (a tool for the
  personas, full text through FTS5, meaning through LanceDB). LanceDB stays a pure search index.
  *Implemented (B1):* `<data>/baruch.sqlite` with the core schema (`baruch-store`) and the author library including
  FTS5 search (`baruch-library`); since Round 12 LanceDB as semantic search (index in the data folder).
- **Git revision system: decided and implemented** (2026-11-05, B1): SQLite stays the source, Git
  mirrors. Every acceptance and every own save becomes a commit in the Typst folder of the work
  `<data>/works/<slug>/` (`baruch-gitmirror`, pure Rust through gix): the author is the human or the
  persona, the committer the human, persona and model stand as trailers in the commit. No commit without a
  change; if the mirror fails, the change stays valid and the next commit catches up on the full
  state. The folder can be cloned, pushed (Forgejo/GitHub, only by the user) and typeset with
  `typst compile main.typ`. Only reading in an existing repo with history remains open.
- **Free (AGPL-3.0)** – deliberately as a counter-design to paid tools such as Logos.
- Screen: **chapter tree on the left, editor with revisions per section („‹ ›“) in the middle, assistants
  on the right.**
- Assistants write **only proposal revisions**; a revision becomes current only by a click.
  **Revision tree (Round 3, implemented):** per section exactly one current revision as a mark, switchable at any
  time; changing and deleting only by the human, logged in the Git mirror. Import only through the
  command line (`baruch-server import`), not through MCP (decision 2026-11-05).
  **Exception ideas (IDEA_CRUD, implemented in Round 7):** ideas change no book text; assistants and
  MCP may create, change and delete them, every action with origin as a commit in the mirror.
  **Exception posts (Round 8):** posts are no book text either; the human moves, deletes and
  converts all of them, assistants and MCP only their own (author = persona or `@mcp:<name>`), one commit each. Open
  proposals stand in the reading flow in the colour of their persona (Round 7). Titles of outline items are
  never empty, single-line and at most 200 characters long (Round 7).
  **Exception outline (wish 2026-11-06, implemented):** „Ein LLM hat den besseren Überblick durch seinen
  Context“ – assistants may **create** outline items (MCP `outline_add`, also a whole outline in
  one call and one commit; revision 1 and the commit carry the persona or the client). **Renaming,
  moving, changing the level, number on/off and deleting only after confirmation by the human** („UD nach
  Bestätigung“): assistants file a request (migration 0012), the human confirms or rejects
  (outline page, RPC `outline.request_decide`); stale requests are closed with a reason.
- **All personas are connected with each other**; the persona with the **largest context is
  automatically the lector**.
- Typesetting and **PDF preview with Typst** as in the existing build of the books.
- Import: **Typst/Markdown chapters, PDF, empty new**.
- **Bible texts and literature through MultiGPT RAG** (no bundled Bibles, no licence questions).
- A second use case of equal rank: **sermons**.

Name: Baruch, the scribe of Jeremiah, who wrote down the burnt scroll anew and extended
(Jer 36,32) – revisions are the core.

## Models (ideas, no code)

**Elisa** (`Beerlesklopfer/elisa`): personas as editable records with a model per persona
(`LLMRoleDescription`, `context_for()`); the rule "an LLM writes no binding content" with
origin per machine contribution (`generated_by_llm`, `llm_model`); an append-only revision log
(`app/history.py`); stable outline numbers (`app/outline.py`); a chat per section for
justifications; auditor Nathan (findings, never approval); lector JSON contract (`write-time-feedback`);
duty to cite (`app/grounding.py`); personas talk to each other through a relay
(`list_personas`, local ones ask external ones for a second opinion).

**MultiGPT:** MCP tools `ask` (model, costs, budget), `search_documents`, `upload_document`,
`read_document`, `list_models`; context length per model (`context_budget.context_length`);
version switcher (`chat/templates/chat/_message_actions.html`) as an interaction pattern.

## Architecture

- **Server with a browser user interface** (like David): `baruch-server` on axum 0.8, pages and fragments as
  minijinja templates, the core through JSON-RPC 2.0 (`POST /rpc`, among others `ui.render`, `revision.accept`);
  widgets in TypeScript (`data-widget`, compiled by `build.rs` with `tsc` and embedded); without
  JavaScript everything stays usable through links and forms (`POST /act`). Typst preview later as
  page images (`typst-render`) or SVG. The core as a library (`baruch-core`). Run locally on the laptop
  or on the school machine.
- **Data locally in SQLite** (sqlx, migrations), one file per data folder (`<data>/baruch.sqlite`),
  including the author library (literature, quotations, citations); released to the personas as a search tool
  (RPC `library.search`). The database itself secures the rules: exactly one `current` per
  section (partial index); a parent (`based_on`) only to an earlier revision of the same section,
  „aktuell“ switchable from `proposed`/`rejected`/`superseded`, origin immutable, deleting never at
  the current revision, numbers never reused (triggers, migration 0004; replaces "append only" from
  0001 and the status rule from 0003). Changing and deleting only by the human (server, not through MCP).
- **Git mirror per work** (`<data>/works/<slug>/`, a Git repo with `main.typ` and `abschnitte/NNN-slug.typ`):
  a snapshot of the current revisions after every change; paths and intermediate folders are checked
  (no `..`, no `.git`, no symlink), so that nothing is written or deleted outside the mirror.
- **Vector search embedded with LanceDB** (Rust, without a server): texts, revisions and comments
  for the search of the personas; MultiGPT RAG additionally for Bibles and literature.
- **Models:** any OpenAI-compatible address (Ollama, LM Studio, Eliezer, small 3B models
  locally) or ChatGPT/Claude with an own key; MultiGPT optional.
- **Typst embedded** (`typst` crate): live preview as PDF/page images next to the editor, export
  PDF; templates „Buch“ and „Predigt“ bundled, fonts bundled (offline).
- **Pluggable model providers** (`trait Backend`): MultiGPT (MCP, key per account; costs/budget there),
  OpenAI-compatible (Eliezer, Ollama, LM Studio), Anthropic/OpenAI directly with an own key. Every
  backend reports models and context length.
- **Pluggable knowledge source** (`trait Knowledge`): first MultiGPT RAG (`search_documents`, Bibles
  and literature as collections); without MultiGPT the Bible search stays off and is shown as such.
- Keys in the keyring of the system (`keyring` crate), never in plain text in the database.
- User interface German (English prepared, i18n), identifiers English, no inline JS.

## RAG reconciliation with MultiGPT

Baruch works offline with the embedded LanceDB and reconciles with MultiGPT as soon as it is
reachable (MCP, key per library).

- **What is reconciled:** texts, not vectors. The MCP interface delivers no vectors,
  and the embedding models can differ; each side embeds itself (default
  `nomic-embed-text`, 768 dimensions like MultiGPT – then search results are comparable).
- **Baruch → MultiGPT:** current revisions and comments/findings per work as documents into a
  collection of its own („Baruch: <Werk>“) through `upload_document`; one document per chapter, replaced on
  a change (`delete_document` + anew). So MultiGPT chats find the state too.
- **MultiGPT → Baruch:** fetch chosen collections (Bibles, literature) through `list_documents` /
  `read_document` and embed them locally; afterwards the Bible search works offline too.
- **Identifier and changes:** per document a stable id (work/chapter or MultiGPT document id)
  and a content hash; only what has changed since the last run is reconciled
  (`sync_state`: side, id, hash, time). If both sides are equal, the source wins:
  own works belong to Baruch, collections belong to MultiGPT – no merging.
- **Usage:** button „Abgleichen“ and display „zuletzt abgeglichen …“; errors (offline, budget)
  only as a hint, the work goes on locally.
- **MultiGPT:** needs nothing new for that except an API key with `docs.read`/`docs.write`; check
  whether `list_documents` delivers a modification time or hash, otherwise note it as a small extension.

## Use case Bible school

- **School:** one big machine with MultiGPT and Eliezer (large models, shared collections:
  Bibles, commentaries, scripts of the teachers, library).
- **Students:** Baruch on their own laptop with a local LanceDB; reconciles the collections of their
  course from MultiGPT and keeps working offline with small local models (3B). In the
  school network the personas run on the school machine (large context, the lector there).
- **Permissions through MultiGPT:** an account and API key per student; which collections he may read is regulated by
  MultiGPT (roles, shares). Baruch stores the key in the keyring.
- **Submission and supervision:** a student can submit a work (sermon draft, term paper) into a
  course collection; teachers see the state and findings, their comments come back as comments at the next
  reconciliation (never as a change to the text).
- **Costs:** large models of the school through its budget in MultiGPT; ChatGPT only if the student
  enters an own key.

## Data model (SQLite)

- `work` (kind = book|sermon, title, subtitle, audience, purpose, bible_version, markup =
  typst|markdown, template, knowledge_collections[])
- `node` (work, parent, position, number „6.2“, title, kind = part|chapter|section|sermon_part)
- `revision` (node, number, body, author_kind = human|assistant, persona, model, based_on,
  status = proposed|current|rejected|superseded, note, edited, created) – a tree per section (`based_on` =
  parent), exactly one `current` per section as a switchable mark; the current revision cannot
  be deleted (migration 0004); since Round 22 the human amends his current revision when saving in the
  editor (`amend_seq`, history `revision_amend` with text before/after, undo as a new step), as long as
  no rule requires a new revision (setting per work `work_revision_policy`, migration 0050)
- `thread` / `post` (node, persona|human, reply_to, body, sources[], created) – the shared
  section chat of all personas (their connection with each other)
- `persona` (handle „@exeget“, name, role, prompt, backend, model, context_length, active)
- `job` (node, persona, kind = review|propose|audit|ask_persona, status, error)

## Connected persona team

- **Lector automatically:** the active persona with the largest reported context; it gets the
  whole chapter or the whole sermon, moderates, collects the findings of the others and writes the
  proposal revision. If the model changes, the lector changes; show who it currently is.
- **Connection:** personas post into the section chat and call each other by `@handle`
  (job `ask_persona`); the lector sees all posts. Limit against endless loops: rounds per
  job and a cost limit.
- **Start team** (prompts editable): Exeget (text, context, main message, with RAG),
  Schrift (prove Bible references and wording, otherwise „nicht belegt“), Belege (sources, footnotes,
  „Angabe prüfen“), Nathan/Auditor (theologically vulnerable places, rules on violence/protection,
  emergency numbers; findings only), „Hörer“ (reads as the target group), for sermons additionally time keeper
  (length in minutes) and Anwendung (relation to everyday life).
- **Duty to cite of the Belege persona** (insight from MultiGPT chat 59): research by the models
  delivers foreign, often unverifiable sources. Therefore every factual statement needs evidence as
  `@schlüssel` from the literature (author library, Typst `@cite`) or the note „nicht belegt“.
  New sources first come into the literature as a title with a quotation, then into the text.
- **Own endpoint per persona, also as a server** (2026-11-05):
  - *Outgoing:* every persona has its own endpoint (`endpoint`: address, kind `openai`|`mcp`|`multigpt`,
    model; key per persona in the keyring). So personas run on different machines (laptop,
    school machine, Eliezer); the endpoint reports the context length, the lector follows from it.
  - *Incoming:* **one shared MCP server** in `baruch-server` (`POST /mcp`, with a token) for all
    personas: tools among others `list_personas`, `ask_persona {handle, …}`, search in the work and the
    author library (read only). With it other tools, other Baruch instances or the
    personas among each other (`@handle`, relay as in Elisa) call a persona – with its prompt and
    its endpoint. Results stay proposals or posts.
    **Chatting through MCP:** tool `chat {handle?, node?, message, thread?}` – a tool (e.g. Claude
    Code, MultiGPT, a script) chats with a persona or the whole team; with `node` the
    conversation lands in the section chat (author = name of the tool or token), otherwise in a thread of its own.
    The lector sees it too; `@handle` in the text calls further personas (round limit and cost limit hold).
- **The memory is the lector** (2026-11-05): it knows the whole work and the author library and
  reports "you already wrote in … that …" – repetitions, contradictions, cross-relations. It searches in SQLite
  (FTS5) and LanceDB; every post carries references to the place (`TextRef`: section, revision,
  wording), in the user interface as a link to jump to. No persona of its own for that.

## Screen

- **Left:** chapter tree or sermon outline, reordering by dragging, marks for open proposals
  and findings.
- **Middle:** the section in the editor (typeset view after the work template, switchable to Typst or Markdown), head „‹ 3/5 ›“ for
  revisions, proposals in the reading flow in the colour of the persona with a diff and „Übernehmen / Verwerfen“; switchable to the
  Typst preview (page) of the chapter.
- **Right:** personas (lector marked), „Prüfen“ / „Vorschlag“ for section or chapter,
  below them the shared section chat with sources.

## Stages

Overall view in time (finished rounds from Git, planned ones estimated): `docs/Fahrplan.md`; projects in
`docs/Backlog.md`.

1. **B1 skeleton:** repo, workspace (`baruch-core`, `baruch-server`), SQLite, works, chapter tree,
   revisions by hand with „‹ ›“, import Typst/Markdown by headings (`=`/`#`), export as a
   Typst folder, Typst preview and PDF.
2. **B2 assistants:** backends (MultiGPT MCP first, OpenAI-compatible), personas, automatic
   lector by context, section chat with `@` calls, proposal revisions with a diff. Implemented in Round 10
   with Eliezer (OpenAI-compatible) as the first backend; in the trial since 2026-11-06 the lector is fixed `@lektor`
   (switchable to „automatisch nach Kontext“).
3. **B3 PDF import:** through MultiGPT `upload_document`/`read_document` (or locally, when without
   MultiGPT), outline with a follow-up question. Implemented in Round 21 (integrated 2026-11-09): PDF through MultiGPT or locally
   (`pdftotext`), EPUB always locally (according to its tool description MultiGPT accepts no EPUB), preview with an editable
   outline, created only after the confirmation of the human (work, source of the library or both); plus Markdown,
   ZIP and folders with images. The MultiGPT assumptions are unchecked (manual check by the user).
4. **B4 sermon:** work kind and Typst template (text, introduction, parts, application, conclusion),
   time keeper, keyword notes as an export. Implemented in Round 16 (integrated 2026-11-08): template with a sermon text
   from SWORD, the lector knows the sermon, speaking time with a target time (tree, editor head, outline, RPC/MCP),
   keyword notes as PDF (A5, cards; page, RPC, CLI).
5. **B5 distribution:** server packages (.dmg, .msi, .deb/.AppImage), signing open, AGPL notices,
   updating.

## Today (state 2026-11-05)

- **B1 excerpt done:** `baruch-server` (axum, minijinja, TypeScript widgets) with a three-column
  user interface, light/dark theme, Alt+←/→, movable columns; data in SQLite instead of fixed in the
  code (sample work at the first start), accept/reject/save one transaction each, followed by a
  commit in the Git mirror. Literature and full-text search over work and library as a popup of the top bar
  (since Round 3; Alt+L, without JavaScript the pages `/literatur` and `/quelle`, `?lq=…`; hits as
  `<mark>`, hit links keep work and search, titles with hits expanded).
- **Hardening:** NUL and control characters in the search yield an empty search instead of a server error;
  status transitions by trigger; symlinks in the mirror are refused; the mirror catches up after an error
  with the next commit (tests).
- **Round B1.5 done:**
  - MCP tools through `POST /mcp` (`list_works`, `outline`, `read_node`, `search`, `propose_revision`,
    `add_post`, `list_personas`, until Round 3 also `import_typst`; without a token only from the own machine). `ask_persona` and
    `chat` stay stubs (B2).
  - Typst import of own books: CLI `baruch-server import/export` and (until Round 3) MCP `import_typst`, the work template
    up to the first heading in `work.template`, `=`/`==`/`===` as the outline, round trip byte for
    byte, duplicate protection through `source_sha256`.
  - Several works with a choice of work in the top bar, one Git mirror per work.
  - Open: `#let` definitions of the template do not hold in the section files of the mirror included through `#include`
    (decision with the user); `#heading(...)` before the first
    `=` heading stays part of the template; Markdown import.
- **Round 3 done (revtree · litpopup · small):**
  - Revision tree: a parent per revision, „aktuell“ switchable (`revision.set_current`), changing and deleting
    only by the human (`revision.update`/`revision.delete`), one commit each in the mirror with
    `Baruch-Action`, tree view and preview with `pick`. Numbers stay unique, with gaps.
  - Literature as a popup (top bar, Alt+L, without JavaScript `/literatur`), hits open the find spot,
    document view `/quelle`, highlights (quotation, key statement, contradiction, to check), quotation as a
    proposal revision.
  - Small things: port 8765, data folder `dirs::data_local_dir()`, `import_typst` no longer in MCP, template
    as `vorlage.typ` in the mirror, slug at word boundaries, sample work renamed (only new databases),
    parser limits fixed.
- **Round 4 done (works · outline · fixes):**
  - Managing works: unique titles (migration 0005, `title_key`), the choice of work opens immediately, cookie
    `baruch_work`, page `/werke`, creating (book/sermon), changing (metadata, work template with revisions),
    deleting with the typed-in title (mirror archived under `works/.geloescht/`), upload import and export as
    a download, CLI `import --title`. Only the human, not through MCP.
  - Outline manager: levels (part, chapter, section, subsection), moving, changing the level, creating,
    renaming, deleting (RPC `outline.*`, forms without JavaScript, drag and drop), numbering after the
    work template or configurable (migration 0006), resolved Typst escapes in titles, headings in the
    editor adjust the outline; one commit in the mirror per change.
  - Follow-up work: deleting needs `confirm`, the message names existing revisions, quoting without a section
    refused, highlights robust against whitespace, the full-text index follows deleted/moved posts (0105),
    tests of concurrency.
- **Round 5 done (tree · wysiwyg · r4):**
  - One outline: the tree on the left with drag and drop, context menu, renaming in place (F2,
    double click), creating, changing the level, deleting and numbering in the head; `/gliederung…` only without JavaScript now.
    Moving keeps the level. Switching and accepting let the outline follow the revision, also in the
    level (sub-items relevelled along, one commit); the revision itself stays unchanged (user decision
    2026-11-05). Roman chapters also from a numbering function of the template.
  - Typeset view editor, first step: crate `baruch-wysiwyg` (blocks, Typst islands, round trip byte-identical), RPC
    `wysiwyg.*`, widget with the switch „Satzbild | Quelltext“, style from the work template, typeset view diff of the
    proposals; without JavaScript only the source text. Description: `docs/WYSIWYG.md`.
  - Follow-up work: work ids never reused (migration 0007, `AUTOINCREMENT`), slugs of deleted works
    reserved, no new seeding after deleting all works, MCP and quoting only with the fixed key `key`,
    `work.import` through `/rpc` up to 16 MiB of text.
- **Round 6 done (numbering · satzbild · images):**
  - Outline schemes per work (migration 0008, twelve schemes from „aus der Werkvorlage“ to „Predigt“ and
    „eigenes Muster“ in a pattern language with `P|k` and `P1;P2;…`), page „Gliederungsschema“ with a preview,
    without JavaScript; outline, editor head, export and mirror follow, „aus der Werkvorlage“ byte-identical.
    `heading_numbering` reads only the last rule of the top level and recognises depth conditions.
  - Typeset view: `#szene`/`#arbeit` as blocks with editable paragraphs, labels at the heading, style only from
    rules of the top level, no empty area in the source text, a click opens the typeset view, inserting several
    paragraphs, images in islands with alternative text.
  - Images as text (migration 0009: Base64 or SVG, never BLOB) with revisions like sections, page „Bilder“,
    RPC `asset.*`, MCP read only, import of the images through the CLI and `export --out` byte-identical.
- **Round 8 done (inline · posts · typst):**
  - Proposals at their place in the reading flow (aligned with the shown revision, `baruch_wysiwyg::align`),
    the typeset view as the default already in the server HTML, lines with 3 : 1 contrast.
  - Sections addressed by the fixed key (`/?work=W&key=K`, MCP field `url`); deleting posts,
    moving them, turning them into ideas (the human all, MCP only its own; an exception as for ideas, posts are
    no book text); chat badges.
  - Typst preview with the work template: crate `baruch-render` (Typst 0.15.1 embedded, offline, SVG), page
    `/werke/{id}/satz`, RPC `preview.typst`, MCP `typst_check` (read only).
- **Round 10 done (B2: backend · personas · scripture):**
  - Model backend Eliezer: crate `baruch-llm` (streamed chat answers, one request at a time, limits,
    aliases), `models.rs` (RPC `model.*`, short test), comparison of the persona models when checking the connection.
  - Personas for real: review, proposal and „@persona fragen“ in the section chat, the answer word by word
    (`/assistenten/stream`, without JavaScript `/assistenten/act`), role prompts with hard rules (migration
    0015), the lector fixed @lektor (trial) or automatic, the lector knows the ideas, MCP `ask_persona`/`chat`.
  - Bible references: crate `baruch-scripture` (German versification of the Schlachter 1951 after SWORD „German“), RPC
    `scripture.check`, MCP `scripture_check`; the wording stays „ungeprüft“. MCP tokens for other machines
    (migration 0016, hash only).
- **Round 11 done (images and cover: mcp-assets · cover · images):**
  - Book as PDF (`typst-pdf`, the same page count as the typeset page, RPC `work.pdf`, `/werke/{id}/pdf`, CLI
    `export --pdf`), print check (missing images, raster images under 300 dpi).
  - Cover per work with front/back in typesetting, front matter with an imprint from the book data (migration 0017),
    cover PDF for the printer with spine and bleed, EAN-13 from the ISBN.
  - Images through MCP: `asset_add` immediately, replacing/cover/deleting only as a request (migration 0018) that the
    human confirms on the page „Bilder“. Image search, preview images, inserting an image, chapter image in the editor.
  - „Einstellungen → Assistenten“ with one tab per persona and text fields for role and role prompt;
    completion „@…“ in the section chat.
- **Round 12 done (using connections, RAG: lancedb · pgvector · multigpt):**
  - Semantic search switchable per work: LanceDB locally in the data folder (main RAG; sections in the current
    revision, ideas, highlights, author library; embeddings through Eliezer, one request at a time),
    pgvector directly (read only) and MultiGPT MCP; hits merged (Reciprocal Rank Fusion) without duplicates,
    a failed way never blocks, the wording search (FTS5) always stays.
  - Collections per work (migration 0031), status of the ways, „Index neu aufbauen“; switch wording/meaning in
    literature and ideas; MCP `rag_search`/`rag_status` (read only); @belege and the lector get hits
    as evidence `[Rn]` (migration 0032), hits are data, never instructions.
- **Round 13 done (source documents and web search: bau · quellen · websuche · abgleich):**
  - Server integration tests as one binary (`target/` 38 → 9 GB).
  - Full text of a source from MultiGPT in the document view (page-wise parts, find spot highlighted, adoption into
    the library), checking highlights without a quotation against the full text (migration 0106), MCP `source_read`
    (only documents from chosen collections); one shared MCP session with a pool.
  - @belege researches on request in the web through MultiGPT (`web_search`, `fetch_url`; preferred sources first);
    Baruch checks every quotation against the fetched page and sets the source line itself (migration 0034); not
    through MCP, not for a proposal.
  - Reconciliation Baruch ↔ MultiGPT (migration 0033): current revisions per chapter to „Baruch: <Werk>“ only after
    confirmation, chosen collections as a local copy in the LanceDB index (searchable offline), page „Abgleichen“.
    Deviation: comments/findings do not go out yet (user question in the backlog).
- **Round 14 done (parts and outline: teile-typst · teile-gliederung · nummerierung):**
  - Parts in the Typst text as the marker `#heading(level: 1, numbering: none)[T] <baruch-part>`, „ohne Nummer“ as
    `… <baruch-unnumbered>`; import, export (byte-identical), mirror, typeset view and typesetting (a part alone on a right-hand
    page, chapters count on across parts). Existing texts are never converted.
  - Creating parts in every work, also with a work template (tree, form, RPC `outline.add` with `level`, MCP
    `outline_add` with `level: 0` – assistants may create); renaming writes the marker line, switching
    reconciles; requests of the assistants with a preview (also without JavaScript); `idea.process` atomic as an outline item.
  - Numbering per level from one source (`Numbering`): functions of the work template evaluated per level, schemes
    with words (Juristisch, Predigt, …) come back as the same scheme on re-import; chapters without a number stand
    in export, mirror and typesetting as a marker (the DB stays), Typst counts like Baruch.
- **Round 15 done (lector and personas: lektor · anbieter · auftraege; 2026-11-08, review followed up):**
  - The lector according to the plan: the whole chapter (for sermons the work) in its wording, findings of the other personas, review,
    questions and proposal for a section or a whole chapter (a proposal per part, only `proposed`). If the context does not
    fit, Baruch never shortens silently: more distant parts summarised, too long sections reviewed in overlapping pieces,
    the note „Komprimiert: …“; every call is measured before sending, and what does not fit even when compressed
    is honestly refused. „Kritisch:“ is checked and asked for once more, removed in the proposal.
  - Model backends free per persona: Eliezer, any OpenAI-compatible services and Ollama (migration 0036), „lokal“
    or „extern“ (on creation by the address, a warning for a public address marked as local), book text to external
    connections only after approval per persona – also no embeddings through an external Eliezer; context length
    compared with the peer (Ollama: the effective length, never the maximum of the model).
  - Jobs for personas: chat stream with the own post immediately, notice page without JavaScript, an idea to a chosen
    persona, literature „an Persona geben“, threads without a section (migration 0038), MCP `chat` with a queue per
    client.
  - User wishes 2026-11-07: **Bibles in the literature** (SWORD Bibles in the literature popup and under
    `/literatur/bibel`, hit groups „Bibel“ and „Kommentar“, quotation as a proposal) and **SWORD as a
    research tool** (commentaries and lexicons per work, migration 0037; a fact block from Bible text, commentaries and
    lexicons for @belege and the lector, numbered evidence; MCP `commentary_lookup`/`lexicon_*`).
  - Protection: no page can be framed by foreign pages (`X-Frame-Options`, `frame-ancestors`).
- **Round 17 integrated (Bible references, rest of SWORD, rest of the lector: verszaehlung · stellen · pruefanzeige; 2026-11-08):**
  - All 18 versifications of SWORD and a checked mapping between them (psalm titles, chapter boundaries
    such as Joel 2/3, Mal 3/4 …; never guessed), RPC/MCP `scripture.map`; Bibles in another versification are read through the
    mapping (wording check, sermon text, lookup).
  - Reference parser: comma lists, times of day and counts are no references, „Psalm 139, … Vers 23“, quotations across
    paragraphs and nested, block quotations in scenes; 4th and 5th Moses in running text.
  - Check display: „Zitate prüfen“ in the editor (highlight in the typeset view, list; without JavaScript a page of its own), RPC
    `bible.marks`; enciphered (key only in the keyring) and BZIP2/XZ-packed SWORD modules, licence note
    everywhere, commentary widget.
  - Lector: quotation check only against the shown text, wrong reference claims in „Kritisch:“ findings marked,
    ideas framed, effective context length without a network query on page load.
- **Round 26 integrated (exchange format between instances, XML: ids · export · import; 2026-11-08, brought forward before
  R25 and R18):**
  - Every piece of a work has a globally unique identifier (UUID, migrations 0041/0042), deleted ones leave
    tombstones; every instance has an identifier and a name.
  - `.baruch` (ZIP with `werk.xml`, images and a manifest with SHA-256) or `.baruch.xml`, namespace
    `urn:baruch:werk:1` with XSD; all revisions, template, ideas, posts, threads, images, book data. Not in the
    format: keys, connections, persona models, open requests, foreign texts (`docs/Austauschformat.md`).
  - Reading in with a preview: new or reconcile (only add, never delete or overwrite, no revision read in
    becomes current, outline changes as requests); safe against DTD, bombs and path tricks. Paths: „Werk ▾“,
    RPC, command line; MCP only output.
- **Round 25 integrated (idea map: mind map of the ideas; karte-daten · karte-bild · karte-widget; 2026-11-08,
  after R26 and before R18):**
  - Page `/werke/{id}/karte` („Werkzeuge ▾ → Gedankenkarte“): the work in the middle, parts and chapters in a ring, sections
    gathered into the chapter (expandable), ideas at their reference, „Ohne Kapitel“ as a cloud; filters by status,
    keyword, origin and full text through GET; outline list and the list „Widersprüche“; outputs SVG, PDF, Markdown
    and FreeMind (`.mm`). Completely usable through forms without JavaScript.
  - Connections between ideas (leads on, belongs to, example of, contradicts; migration 0043) for the human,
    persona and MCP (`idea_link`, `idea_unlink`, `idea_links`, `idea_map`), every change a mirror commit, line
    „Verbindungen“ in `einfaelle.md`; positions on the map (display only, without a commit); nearness from the stored
    embeddings and equal keywords (only displayed, no embedding request when drawing).
  - Widget: zoom, panning, dragging an idea (onto a chapter = reference, onto an idea = connection, otherwise position), keyboard
    (arrows, Enter, V, N, Alt+E), sidebar.
  - Exchange format minor version 2: connections and positions in `<karte>` (XSD `baruch-werk-1-u2.xsd`).
  - Naming groups by an assistant: since Round 19b (a suggestion, the human adopts it as a keyword).
  - Not yet: a map across works (user question).
- **Round 18 integrated (editor and ribbon: menueband · konstrukte · satzbild-werk; 2026-11-08):**
  - Ribbon for editing: formats of the work template as buttons (`#szene`, `#arbeit` only if the template
    defines them; footnote, quotation, list, enumeration, term, break, link always), „Bild“ opens „Bild einfügen …“; in the
    typeset view the widget executes the command, in the source text the Typst text is inserted at the cursor position.
  - Typeset page without JavaScript (‹ › with `aria-label`, PDF, „Satz abbrechen“ as a form), Typst run cancellable
    (`preview.cancel`; evaluation and layout run to their end in Typst, what is saved is writing the pages), foreign
    pages trigger no typesetting and no PDF (`Sec-Fetch-Site`); font search at start; the top bar wraps in narrow
    windows; idea map: edges with labels, an arrow for directed kinds and a dash style of their own.
  - Typeset view: enumerations, term lists, nested lists, break `\`, `#link`, references `@…` as blocks instead of
    islands; islands editable in place; several images per block; Markdown headings also when changing
    a revision; lector: omitted parts in one line, cache of the summaries.
  - Typeset view settings per work („Einstellungen → Werk → Satzbild“: font, size, colours, sheet colour, text width;
    migration 0044, HTML only, never Typst/PDF), style already in the HTML; image at the cursor position (path with the revision in
    one transaction); literature hits highlighted in the typeset view; „Alle einklappen“ also in the typeset view.
- **Round 27 integrated (cover editor and imprint in the work template: umschlag-daten · umschlag-satz ·
  umschlag-widget; 2026-11-09, after R18 and before R19):**
  - Cover page as an editor: place title, subtitle, author, publisher, spine texts, motto, blurb, short biography,
    barcode, price and one free text per face freely as blocks (position, width, size, line spacing, colour,
    font, weight, alignment, shadow/gradient, running direction of the spine); guide lines, snapping, keyboard,
    undo; preview through GET and saving through forms also without JavaScript; quick choice top/middle/bottom and
    reset (migration 0045).
  - The same position in preview, cover PDF and book PDF (millimetres from the trim edge); without an own arrangement
    the typesetting stays byte-identical. Print check: bleed, safety margin, barcode, spine, contrast in the cover image,
    missing font (fallback with a hint).
  - Front matter from the work template (`schmutztitel`, `haupttitel`, `impressum`, `widmung` with the dictionary
    `baruch_buch`), new books get the imprint template, a button adopts it into existing works; addresses
    of publisher and printer; check of mandatory details (customary under the state press laws, no legal advice).
  - MCP reads the arrangement (`cover_layout`) and files at most a request (`cover_layout_set`); exchange format
    minor version 3 carries arrangement, addresses and typeset view settings.
  - Not yet: own font files (user question); cover in the EPUB since Round 20.
- **Round 28 integrated (findings: list of findings, overview over the work, jobs in the chat: befunde · befund-uebersicht ·
  chat-auftrag; 2026-11-09, after R27 and before R19):**
  - Finding as a record (migration 0046: `finding`, `finding_status`, `post_version`): answers of the personas are
    parsed after saving (kind praise/hint/critical, quotation, check note, `[B<id>]` references); the post stays
    unchanged. Only the human sets the status (open, adopted with a revision, discarded with a reason, postponed, done),
    one commit per change; assistants file proposals, the human decides. Similar repetitions
    are attached (text only, embeddings R19a). Mirror `befunde.md`, exchange format minor version 4.
  - Overview: page **„Werk ▾ → Befunde“** (matrix chapter × persona with symbol and number, filters, lists),
    badge „k offen“ in the outline, editor head, `/gliederung` and at the menu, accordion „Befunde“ with
    status forms, layer „Befunde“ in the idea map, „setzt Befund x um“ in the revision tree; MCP `findings`
    (read) and `finding_propose_status` (proposal).
  - Chat: „RFC“/`@alle` calls all active personas, the lector last with a collection and weighting (collective proposals
    with references to the origins); a request for text becomes a proposal; decisions of the author go framed into every
    call. The duty of „Kritisch“ only for @nathan any more, the lector begins with what carries. The chat scrolls in a limited
    height, copying/quoting, editing own posts with history (`post.edit`, MCP `post_edit`), whole history
    without JavaScript (`/beitraege`, `/beitrag`).
  - Not yet: similarity through embeddings and findings in the RAG index (R19a), a new section by a writing job.
- **Round 30 integrated (search/replace, cross-references, chat: solution proposals, variants, handling:
  suchen-verweise · chat-loesungen · chat-bedienung; 2026-11-09, before R29 and R19):**
  - Search and replace in the section (Ctrl+F/H, search bar in the editor) and in the work (Ctrl+Shift+F/H, sidebar
    or page `/suche` without JavaScript) with ranges, case, whole word and regular expression (time limit);
    replacing in the work creates one revision from the human per section, one commit, undo with one click;
    Bible quotations and quotations stay untouched (migration 0047, `replace_batch`).
  - Cross-references: a fixed jump label `<kN>` per outline item only in export, mirror and typesetting; reference
    `#link(<kN>)[…]` (optionally with „S. n“) through the ribbon „Verweis“; Ctrl+click jumps, Alt+← back; a list of
    possible and broken references (`/verweise`, print check); reading in and exchange rekey references.
  - Solutions: critical points carry `statt`/`besser` or `Schritt` (otherwise asked for once more, then „(ohne
    Lösungsvorschlag)“); „Als Vorschlag übernehmen“; variants A/B/C as sibling proposals with a comparison
    (`/varianten`); the review basis the same for all personas of a call and visible in the head; exchange minor version 5.
  - Chat: address „Lektor, …“ without @, relay on a direct address in an answer (one stage), live display
    („schreibt …“, new posts without reloading), ↻ repeat and ✕ delete, structured posts with finding blocks
    and checked jump labels.
  - Not yet: forward jump by key, reference page number as a work setting (R29); EPUB links since Round 20.
- **Round 29 integrated (typesetting in the settings: satz-daten · satz-typst · satz-oberflaeche; 2026-11-09):**
  - „Einstellungen → Werk → Satz“ (without JavaScript, with a preview): chapter start (continuous, new page, new right-hand
    page), part page, page format (A5, 12 × 19, 13.5 × 21.5, A4, custom), margins with binding offset, base font,
    size, line spacing, indentation or spacing, justification, hyphenation and language, page numbers (position, from the first
    chapter), running heads, footnotes, orphans/widows and „Verweis mit Seitenzahl“ as a work setting.
    Every change a mirror commit by the human; MCP only reads (`typeset_settings`).
  - Typst rules „// Baruch: Satz“ behind the work template (hook `snapshot::head`): the same in export, mirror,
    typesetting preview, book PDF and cover; byte-identical without settings. Hints where the work template sets the same thing
    itself; an offer to adopt a chapter start rule of the template as a setting (a new template revision).
  - Migration 0048 (`work_typeset`, tombstone kind), exchange minor version 6 (`<satz>`); `epub_style` as a
    data model (not wired yet).
  - Since R21b the import of an export reads the rule block back into the typesetting settings (only when exactly readable);
    EPUB uses `epub_style` since Round 20.
- **Round 19 finished (RAG maintenance, rest of the references: index · pgvector · treffer; 2026-11-09):**
  - Index: no new embedding attempt per search (searches only queue the cleanup), honest status with
    jobs left behind (`index.stale`, page „Verbindungen“), „Index neu aufbauen“ answers immediately
    (`deferred`), ideas of deleted works immediately in the shared area, start with an own data folder without a
    stored connection Eliezer without an embedding request; reconciliation with MultiGPT hardened.
  - Task prefixes of the embedding per way (LanceDB query/document, pgvector query); pgvector shortens every field in
    SQL, quotes schema names, warns on a password without TLS.
  - „schwach“ for hits below 0.45 in literature, ideas, evidence and `rag.search` (`weak`), collapsed without
    JavaScript; `idea.search` with `mode: "rag"` reports failures (-32001).
  - Idea map: „Gruppen benennen (Assistent)“ as a suggestion, adopting only by the human; MCP `idea_map`
    with `near_limit`/`near_threshold`. Ribbon „Bibelstelle“/„Literaturverweis“ when the template provides them.
  - References: wording against modules with hyphenation remnants („Wortlaut stimmt (Leerraum im Modul)“, a hint about the module),
    „Mal“ as Malachi before chapter and verse, quotation link, lemma probe of the lexicons, NFC in the comparison.
  - Not yet: older revisions/findings in the RAG index (user question 1), comments/findings to MultiGPT
    (user question 11).
- **Round 20 integrated (EPUB preview: epub · reader · epub-pruefung; 2026-11-09):**
  - EPUB 3 with an EPUB 2 fallback (new crate `baruch-epub`, pure): one XHTML file per part and chapter, text through the
    block model of baruch-wysiwyg (no typst-html), footnotes as a popup or endnotes, cross-references `#link(<kN>)`
    as internal links, images with alternative text, cover as `cover-image`, front matter, `nav.xhtml` + `toc.ncx`,
    metadata only as escaped text, typesetting settings as CSS (`epub_style`), deterministic bytes. What cannot be
    mapped is reported (`ISLAND_OMITTED`, page break rules of the template `TEMPLATE_PAGEBREAK_*` …).
    Paths: menu „Werk → Als EPUB herunterladen“, RPC `work.epub`, CLI `export --epub`, MCP `epub_export` (reading).
  - Preview per reader (menu „Ansicht → EPUB-Vorschau“): nine profiles (mandatory: Apple Books iPhone/iPad, Google Play
    Books, „Kindle (über EPUB)“, „Kindle-App (über EPUB)“), iframes side by side with reader CSS and an own CSP
    (`sandbox; frame-ancestors 'self'`), chapter boundaries, night mode; without JavaScript through a form GET. Profile values
    are assumptions without a device.
  - Check: own structure and accessibility check (xmllint, otherwise quick-xml), reader warnings, EPUBCheck from the
    PATH or a configured epubcheck.jar (never a download, only the human starts it; MCP `epub_check` never). Rest of the
    book data: ISBN prefix 978/979, `paper_sheet_mm` (thickness per sheet, without a migration), „Umschlag entfernt“ as a
    commit of its own, printer cover with the text rules of the work template. No migration.
  - Not yet: a real device/reader check and a real EPUBCheck run with Java (manual check), manufacturer readers on
    Android (user question), an own Kindle format (deliberately not, Send to Kindle accepts EPUB).
- **Round 31 integrated (index and glossary: glossar-daten · register-satz · kontextmenue; 2026-11-09):**
  - Glossary per work (migration 0049: terms with variants, short and detailed explanation, source, Bible references;
    own indexes; index proposals), RPC `glossary.*`/`index.*`, one mirror commit per change, mirror file
    `glossar.md`; assistants and MCP (`glossary_propose`, `index_propose`, `glossary.suggest`) only proposals, the
    human adopts. Exchange minor version 7 (glossary, own indexes, typesetting fields, tombstones).
  - Index mark in the text `#metadata((register: …, term: …))<baruch-idx>` (book text: setting/removing = a new revision of the
    human); indexes keyword, persons, Bible references (automatically from recognised references, biblical order) and
    up to 13 own ones. Typesetting settings: index pages (1–3 columns) with page numbers from Typst (ranges „12–14“,
    subentries, „siehe“/„siehe auch“), glossary page before/after the indexes, the first mention per chapter
    marked; projection only (export, mirror, PDF, EPUB with `glossary.xhtml`/`index-*.xhtml`), SQLite stays
    unchanged; without settings everything byte-identical.
  - Context menu in the typeset view and in the source text (right click, context menu key, Shift+F10; ARIA menu): „In Glossar
    aufnehmen“, „In Register aufnehmen“, „Im Glossar nachschlagen“, edit/remove the mark; ribbon group
    „Register und Glossar“. Without JavaScript: link „Markierung …“, pages „Glossar“ and „Register“ with all forms.
  - The import of an export with a book end takes book end and projection out again.
  - Not yet: editing a mark without JavaScript only as removing and setting anew; upper/lower case of
    equal keywords is not merged; Typst messages from the book end point to the last section;
    the context menu not checked in real browsers other than Firefox; on the import of an export without a glossary page the
    marking of the first mention (`#emph[…]`) stays as an ordinary emphasis in the text; index proposals do not travel
    along in the exchange format; no MCP tool for „Begriffe vorschlagen lassen“ (only the human triggers it).
- **Round 22 integrated (revisions and mirror: fassungen · befund-rueckfrage · spiegel-entwurf; 2026-11-10):**
  - Amending: „Speichern“ in the editor amends the own current revision (state k, per step text
    before/after and a mirror commit); a new revision arises with a reason (on request, persona revision, another
    base, heading/outline, large change, time limit, switched off). History with a word diff,
    undo as a new step, setting „Fassungen“ per work, „weitere Fassung(en)“ in the reading flow, side branches
    `refs/revisions/<key>/<n>` in the mirror. Migration 0050.
  - On a renewed review personas ask about own findings that are done („Befund n scheint erledigt?“, yes/no by
    the human); adopted findings remember the state of the revision. Archive of deleted works with restoring;
    mirror of the ideas without a work at start and after deleting.
  - Draft as a watermark (typesetting setting, „Als Entwurf herunterladen“ without changing the setting) in the
    typesetting preview, book PDF, keyword notes, on request the cover, EPUB title page/metadata; byte-identical without a draft.
    Exchange minor version 8 (amendments, draft, state at findings, fast-forwarding, a new part as a request with
    an identifier), `<kN>` show rule including the way back on import, mirror settable (`main.typ` with `#include`).
  - Not yet: the other writing paths (image, index mark, reference, replacing, outline) do not amend;
    option "session" instead of minutes; images and work kind do not come along from the archive; `persona.chat`/MCP
    `ask_persona` do not output the follow-up fields separately (since Round 23 they do: `done_proposals`, `done_notes`,
    `all_done`).
- **Round 23 finished (hardening: web · mcp-namen · geheimnisse; 2026-11-24):**
  - Editor and user interface: quotation tool (block with a following line for the reference, in the typeset view quotation and a paragraph of its own
    instead of a code island), „Fassung ändern“ with a history step and a state check, `guard::own_reads` against reading
    requests of foreign pages, fixed message codes, badges without JavaScript, editor lock while saving,
    behaviour tests of the widgets in the Node harness; leftovers of idea map, cover, EPUB, index, findings.
  - MCP: unique handles with umlaut transcription, `lokal-` for clients without a token, ownership only through the handle,
    a shared image quota without a token, upper limits and size limits, one relay path, feedback fields in
    `ask_persona`/`persona.chat`.
  - Secrets: redacting in one place (also Base64 and percent form), no redirects, filter before
    `fetch_url`, `confirm` when deleting model connections, limits in the exchange, a private EPUBCheck temp folder.
  - No migration, no new minor version, no new methods or tools. Review (14 findings),
    follow-up (old handle for tokens that only the new rule merged; shared quotas without a token)
    and PM close have taken place; leftovers in `docs/Backlog.md`, „Nach Runde 23 offen“.
- **Round 24 "Distribution (B5)" done** (2026-11-24; with review, follow-up and PM close – the last
  planned round): version 0.9.0.
  - Packages: .deb and AppImage built and started locally; .dmg and .msi only as CI jobs (unsigned, never
    run). Every package carries `LICENSE`, `THIRD-PARTY-LICENSES.txt` and the source code offer `QUELLTEXT.txt`;
    data stay in the user folder, no package creates or deletes anything there. Keyring per target system.
  - About page `/ueber` (version, licence, source code, third-party licences, data folder) in the menu „Werkzeuge“ and under
    „Einstellungen → Daten“; **no update check** – Baruch never asks the network for versions on its own.
  - CI: `ci.yml` (tests, clippy with `-D warnings`, `cargo deny`, `typst compile`, site build), `release.yml` (tag
    `v*`: packages, APT repository, release draft, GitHub Pages), `pages.yml`. No workflow has ever run;
    no tag, no push. All delivery goes through GitHub, nothing by hand.
  - Website `site/` (Hugo, own theme, without JavaScript) for `https://baruch.bernau.family/`; APT under `/apt/`.
  - Operation behind a reverse proxy: `--allow-host NAME`/`BARUCH_ALLOWED_HOSTS`, under such a name `/mcp`
    only with a token; access protection in the proxy is mandatory (`docs/Betrieb.md`). MultiGPT has no default address any more.
  - No migration, no new RPC methods or MCP tools. Open items and the steps of the user:
    `docs/Backlog.md`, „Nach Runde 24 offen“.
- **Round 37 "Help on the website" integrated** (2026-11-27, on `development`; version unchanged 0.9.0, in the
  CHANGELOG under „Unveröffentlicht“):
  - **The website is the source of the usage docs:** `site/content/de/hilfe/` (14 pages, everyday language, steps, keys
    for Windows/Linux and Mac) for users, `site/content/de/betrieb.md` for operators; the first setup (Bibles,
    assistants, MultiGPT, AI programs) is in `installation.md` and `mcp.md`. `docs/Bedienung.md` and
    `docs/Betrieb.md` are only entry points now: "Where is what", rule for developers, technical notes.
  - **Images:** list `site/data/hilfebilder.toml` (94 names), shortcode `hilfebild` (missing image = no output,
    unknown name or missing alt text = abort, dark variant through `<picture>`, a link to the full size),
    tool `site/tools/bildschirmfotos.py` (throwaway server, invented sample work, intermediary; `--pruefen`,
    `--fotos`, `--abgleich`). Link check `scripts/site-check.py` in `scripts/ci-site.sh`.
  - **User interface:** menu „Baruch ▾ → Hilfe“ and „Über Baruch“ link to `https://baruch.bernau.family/hilfe/`
    (an ordinary link, also for Lektoren). No migration, no new route, RPC method or MCP tool.
    Open items: `docs/Backlog.md`, „Runde 37 – integriert“.
- **Round 36 "Leftovers" integrated** (2026-11-27, on `development`; version unchanged 0.9.0, in the CHANGELOG under
  „Unveröffentlicht“):
  - Typst command line in the own binary: `baruch-server typst …` or the alias `baruch-typst` (`compile` to
    PDF, PNG, SVG, `fonts`, `--version`; offline, without packages; unsupported things honestly with exit 2). The packages
    deliver only `baruch-typst`, never `typst`; the CI loads no Typst any more. `pages.yml` also runs on
    publishing a release (not checked by execution).
  - sqlx 0.8.6 → 0.9.1 (libsqlite3-sys 0.40, RUSTSEC-2026-0459 fixed, the exception in `deny.toml` dropped);
    migrations and checksums unchanged, assembled SQL through `AssertSqlSafe`.
  - Leftovers from Round 35 (emergency path `passwort` lifts the lockout, work binding relaxed for three entries,
    display name of the Lektoren, theme on the login page), from Round 34 (the round trip of the mind map keeps the cloud,
    labels free, scroll bar), from 0.9.0 and „offen nach R24“ (among others the mirror commit names, when deleting
    an outline item, the revisions and posts deleted along with it).
  - No migration, no new route, RPC method or MCP tool. Open items: `docs/Backlog.md`, „Runde 36“.
- **Round 35 "Accounts and roles (Lektorat II)" integrated** (2026-11-27, on `development`; version unchanged
  0.9.0, in the CHANGELOG under „Unveröffentlicht“):
  - Without accounts Baruch stays open as before. With accounts, pages, forms and `/rpc` require a login
    (Argon2id, session as a cookie, lockout against brute force); `/mcp` stays with tokens. The first account (author)
    arises only at the own machine; emergency path `baruch-server passwort <name>`.
  - Roles per work: the author shares a work with Lektor accounts (contributors). Lektoren (`@lektorat:<name>`)
    see only shared works and write only proposals, posts and findings (also „Anmerkung zum Buch“),
    with origin in the mirror; only the author can accept, change and delete.
  - One place of the permission check (`roles`: `LEKTOR_ROUTES`, `LEKTOR_RPC`): for Lektoren everything is locked that
    does not stand there – also routes and methods of future rounds.
  - Migrations 0053 (accounts, sessions, failed attempts) and 0054 (shares); new RPC `account.*`, `member.*`,
    `revision.propose`, `finding.add`; no new MCP tool. Everything usable without JavaScript. Unchecked in a real
    browser behind a real proxy. Open items: `docs/Backlog.md`, „Nach Runde 35 offen“.
- **Round 34 "Mind map like XMind" integrated** (2026-11-25, on `development`; version unchanged 0.9.0, in the
  CHANGELOG under „Unveröffentlicht“, meant for 0.9.1):
  - The idea map is a mind map: the work in the middle, parts/chapters as branches right and left, ideas as
    sub-branches arbitrarily deep; creating (Tab/Enter), renaming (F2), re-hanging (dragging), collapsing, deleting with
    restoring, undo, connections as labelled lines. One layout, in Rust (SVG from the server,
    screen and output the same).
  - **Decision (user 2026-11-25): cloud and mind map are connected.** Ideas without a place lie in the cloud
    next to the tree; they move into the mind map (onto nodes, chapters or as a main topic onto the work) and back, including
    their child nodes. Nothing becomes a main branch unasked.
  - Ideas as a tree (migration 0052: title, parent node, order, main topic), RPC `idea.move`/`idea.tree`,
    MCP `idea_move`/`idea_tree`, exchange format minor version 9.
  - Outline view without JavaScript; output `.xmind`, `.mm`, SVG, PDF, Markdown; reading in `.xmind`/`.mm` with
    a preview. Unchecked in real XMind and Freeplane. Open items: `docs/Backlog.md`, „Nach Runde 34 offen“.
- **Round 33 "Ribbon and laptop mode" finished** (2026-11-25, on the branch `runde33` next to Round 32;
  version unchanged 0.9.0; merging with `development` and Round 32 including the counter-check are still pending):
  - A ribbon on every page replaces the menus „Werk“/„Ansicht“/„Werkzeuge“ and the row of buttons above the sheet: menu
    „Baruch ▾“ (program), tabs „Start“ · „Einfügen“ · „Prüfen“ · „Buch“ · „Ansicht“ with named groups; the buttons of the
    editor dock into the ribbon with JavaScript. Badges at the tabs. Everything reachable without JavaScript
    (tabs as links `?tab=`, groups as `<details>`, ribbon open/closed through `?band=`).
  - Laptop mode (automatic/on/off): below 1600 px a compact ribbon (groups as popups), below 1280 px outline and
    assistants as rails with a drawer; columns collapsible one by one; reset zoom. State as attributes on
    `<html>`, drawn by the server from cookies, no media queries.
  - Every page opens with its tab; the typeset page carries its controls in the tab „Buch“.
  - No migration, no new route, no new RPC method, no new MCP tool.
- **Round 32 "Lektorat I – Word and quitting" finished** (2026-11-25, on `development` after Round 33; version
  unchanged 0.9.0, in the CHANGELOG under „Unveröffentlicht“):
  - Word import (`/werke/import/word`, RPC `docx_import.*`, CLI `import DATEI.docx`): preview with outline,
    editors and hints; revision 1 = the state before all tracked changes, changes per editor as open
    proposals under `@word:<name>`, comments as posts – never a current revision.
  - Word export (`/werke/{id}/word`, RPC `work.docx_export`, CLI `export … --docx`): styles, real footnotes,
    embedded images, Typst islands with literal source text; on request the youngest open proposal per section
    as change tracking. The own file comes back on import with the same outline and the same text.
  - Quitting Baruch (`/beenden`, only at the own machine, not through RPC/MCP), clean shutdown also on
    SIGTERM, launchers with an app window, Pages in two jobs.
  - New crate `baruch-docx`; no migration, no new MCP tool. Nothing is checked in Microsoft Word.
- Checking: `cargo test --workspace`; preview `cargo run -p baruch-server`, in the browser
  http://127.0.0.1:8765/ (data in `~/.local/share/baruch` or `--data`).
- Not yet: manual check of the backends OpenAI-compatible and Ollama against real services (only mocks), comments/findings in the reconciliation to MultiGPT, Markdown import. Comments
  (section chat, findings) will later also be stored in the (graph) RAG, so that personas find earlier
  findings; today they lie in SQLite (`post` with references) and in the FTS5 index.

### Round 38 – English as the primary language, work language (2026-11-27)

Decision of the user (2026-11-27, before 0.9.1): the primary language is English for code, comments, commits,
developer docs, MCP and RPC; the user interface stays German until Round 39 (then German and English through
catalogs). Each work has a **work language** (`de`|`en`, migration 0055): personas answer in it and write their
markers in it, recognition accepts both languages, book output follows it from Round 39 on. It is independent of
the language of the user interface and of the hyphenation language of the typesetting settings. Exchange format
sub-version 10 carries it (`sprache` at `<angaben>`). Round 38 delivered this only in part; the open parts are in
`docs/Backlog.md`, "Open after Round 38".

## First steps after approval

1. The user creates `Beerlesklopfer/baruch` privately (or approves the creation); read access to the
   Typst book folder (today readable only for one user) or a copy to
   `~/Documents/Project/`, so that template and import are built on the real sources.
2. Locally `~/Documents/Project/baruch`: Cargo workspace, CLAUDE.md (rules: German UI, identifiers
   English, no tokens, version only from the user, suite/Firefox/agents never in parallel),
   `docs/Plan.md` with this plan, `LICENSE` AGPL-3.0.
3. Implement B1. MultiGPT stays unchanged; Baruch needs only an API key with `ask` and
   `docs.read`.

## Verification

- `cargo test`: revision invariants (one `current`, a proposal does not change `current`, accepting
  atomic), import/export round trip Typst and Markdown, lector choice by context, loop limit
  for `@` calls.
- Backend tests against a mocked MCP/OpenAI server, no real costs.
- Compare the Typst preview of the imported book with the PDF from the existing build.
- Real run: have a chapter from the user's book reviewed through MultiGPT (gemma on Eliezer).
- Desktop build on Linux; macOS/Windows through CI (platform runners), only in B5.

## Open (user)

- Read access to or path of the Typst book folder.
- Login relevant only for the server variant; desktop at first without accounts.
- ~~Target version~~ (0.9.0, Round 24). Time of publication and the steps before it (history,
  archive key, imprint: `docs/Backlog.md`, user question 17), signing for macOS/Windows (user question 10).
- ~~Git in the revision system~~ → decided and implemented (see above). Open only: reading in an existing repo
  (Typst book folder) with its history.
