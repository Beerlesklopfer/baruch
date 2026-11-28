# JSON-RPC of Baruch

`POST /rpc`, JSON-RPC 2.0, single or as a batch. Forms without JavaScript send the same methods to
`POST /act` (allowed: `revision.accept`, `revision.reject`, `revision.save`, `revision.set_current`,
`revision.update`, `revision.delete`, `post.add`; fields as below (`node`, `key`, `rev`, `body`, `note`,
`based_on`), plus `work`, `view`, `lq` and, when deleting, `confirm`) and are then redirected to the page
(`/?work=W&key=K&rev=R[&view=preview][&lq=…]`, `rev` = number from the answer). Forms for managing posts go
to `POST /beitraege/act` (see "Managing posts"). Source:
`crates/baruch-server/src/rpc.rs`. `rev` is everywhere the **number** of a revision, not its position: after
a deletion the sequence of numbers has gaps; a number is never assigned again.

**Accounts and roles (Round 35):** Without accounts everything here holds as described. If there are accounts, `/rpc`
requires a session (cookie `baruch_session`; otherwise HTTP 401 with error -32002), and for Lektor accounts only the methods from
`roles::LEKTOR_RPC` are allowed (otherwise -32003) – see "Round 35 – Accounts and roles" at the end.

**Sections: position and fixed key.** `node` is the position of the section in tree order (`seq`,
from 0); every change of the outline assigns it anew. `work.get` additionally names `key` for each node, the fixed
key (database id), which stays the same when other items are moved, re-levelled, created, deleted, split or
merged. **Every method with `node` also takes `key`** (a number or digits as text), the
outline methods with `target` also take `target_key`. If the key is present, **it applies instead of the position**: a
stale tab thus writes into the section it showed, not into the one that now stands at the old position,
and never deletes someone else's item. If the item no longer exists → -32000 „Diesen Gliederungspunkt
gibt es nicht mehr (gelöscht oder zusammengeführt); bitte neu laden.“ All forms and the editor send
the key along. Translating and writing run under the same write lock.

**Citing only by key (Round 5):** `highlight.cite` and `/literatur/act` with `action=cite` take
the section only by `key`. `highlight.cite` with `node` → -32602 „node (Stelle) wird nicht angenommen; key
mitgeben“, without `key` → -32602; the form without `key` → 422 „Abschnitt fehlt.“ (even if `node` is
present), with a key that no longer exists → 409. Likewise the MCP tools
`propose_revision` and `add_post` (including `refs`) require `key` (`docs/MCP.md`). Links `/?node=` remain display only.

**Addresses (Round 8):** The canonical address of a section is `/?work=W&key=K` (+ optionally `&rev=R`,
`&view=preview`, `&pick=…`, `&lq=…`, in this order). It stays valid when the outline is rearranged.
`node=` is accepted on reading only as a fallback; generated links and redirects (`/act`,
`/gliederung/act`, `/literatur/act`, `/einfaelle/act`, `/beitraege/act`) no longer carry it. Resolution: if
the key exists, its section applies (before `node`); otherwise `node`, otherwise the default section, and the
editor head shows „Abschnitt mit Schlüssel K gibt es nicht (mehr).“. Likewise `/gliederung`,
`/gliederung/bearbeiten`, `/literatur` and `/quelle` take the parameter `key`. `pick` stays `<position>:<number>`.
The outline methods additionally answer with `key` (fixed key of the item `node`, otherwise `null`).

**Headings and outline:** If the current revision is set through `revision.save` or `revision.update` (of the
current revision), its text must begin with exactly one heading of the item's level (parts:
only a revision with a part mark must begin with exactly this one mark, Round 14). Otherwise -32000 with a reason (other level, further headings, no heading, text before it), and
nothing changes. A new title of the same level renames the item.
**Switching and accepting** (`revision.set_current`, `revision.accept`) never change the text of the revision;
instead **the outline follows its heading line** (user decision 2026-11-05): with the same level only
the title, with another level level and title, and all sub-items move by the same difference (if one would end up
deeper than 3, it is refused), each with a new revision by the human that differs only in the heading line; the
answer then carries `outline`. Refused (-32000, nothing changed) are only revisions without a heading, with
further headings, with text before the heading (hint at „Speichern“ in the editor) and with a level that
is invalid at that position (e.g. the first item as a section) or a part mark on a chapter;
parts: a revision with a part mark aligns the title (Round 14), one without switches without aligning.
So export and re-import always yield the same outline; splitting and merging only go through
`outline.*`.

**Delete confirmation:** `/act` with `revision.delete` requires the ticked checkbox `confirm` (value `1` or
`on`); if it is missing, the server answers 422 „Löschen bitte bestätigen.“ and deletes nothing (`required` in the HTML
alone is not enough). Over `/rpc`, `revision.delete` requires `confirm: true`. Errors from `/act` come as
422 (invalid, also an unknown revision) with the text of the message.

Requests to `/rpc` need `Content-Type: application/json` (otherwise 415). Requests from foreign web pages
(`Origin` does not match `Host`, or `Sec-Fetch-Site` is neither `same-origin` nor `none`) are refused by
`/rpc` and `/act` with 403. If `Host` is neither an IP address nor `localhost` nor a host name allowed at start with
`--allow-host NAME` or `BARUCH_ALLOWED_HOSTS` (Round 24, operation behind a
reverse proxy, `docs/Betrieb.md`), all routes (including the page) answer 421 (protection against DNS rebinding,
`guard::local_host`). Under an allowed name, `/rpc` is reachable like the user interface without its own login –
the proxy provides access protection; the comparison of `Origin` and `Host` only passes if the proxy passes `Host`
through unchanged (if it rewrites it, `/rpc` answers 403). `/mcp` always requires a token under such a
name (`docs/MCP.md`, "Access"). The MCP server under `POST /mcp` is described in `docs/MCP.md`.

**Work:** Every method takes the optional parameter `work` (work id, number). If it is missing, the
default work applies (the one with the smallest id **now**, so after deleting a work the next one;
`work.update` and `work.delete` require `work` explicitly); an unknown work or a `work` that is not a number
yields -32602. Data lives in `<data>/baruch.sqlite`; each change is one transaction.

## Methods

| Method | Parameters | Answer |
|---|---|---|
| `work.list` | – | `{works: [WorkSummary]}` – all works by id: `{id, slug, title, author, nodes, kind, subtitle, sections, proposals, created, updated}`; `nodes` = nodes, `sections` = nodes without parts, `proposals` = open proposals, `updated` = last change (metadata, template, revisions); works without any works too (see "Works") |
| `work.get` | `{work?}` | `{work, lector, cover}` – work with outline, revisions, posts, personas; handle of the lector; `cover` = `CoverInfo \| null` (Round 11, see "Book as PDF and cover") |
| `work.export` | `{work?}` | `{typst}` – work template and current texts as one Typst file (see "Import and export") |
| `node.get` | `{node}` | section with revisions and chat |
| `revision.accept` | `{node, rev}` | `{node, rev, outline?, commit?, mirror?, markdown?}` – the proposal becomes the only current revision (switching; only proposals, otherwise -32000); the outline follows its heading line, `outline` = outline answer (without `commit`) for another level. If the proposal contains Markdown headings (`# Titel`, user 2026-11-07, `docs/WYSIWYG.md`), a revision of the human with the converted headings follows (like `outline.save` with `split`); `node`/`rev`/`outline` then apply to it, `markdown` = note („umgewandelt“ or why not) |
| `revision.reject` | `{node, rev}` | `{node, rev}` – proposal discarded, `rev` is the current revision; no commit |
| `revision.set_current` | `{node, rev}` | `{node, rev, outline?, commit?, mirror?}` – the mark „aktuell“ onto any non-current revision (proposal, discarded, replaced); the text of the revision never changes, the outline follows its heading line (`outline` for another level, see above); already current → -32000 „Diese Fassung ist schon aktuell.“ |
| `revision.save` | `{node, body, based_on?, based_on_amend?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, commit?, mirror?}` – as before; Round 22: `mode` `"auto"` amends the author's own current revision if no rule requires a new revision (see "Amending"), `"new"` always creates a new revision; **if `mode` is missing, `"new"` applies** (all old clients unchanged); unknown value → -32602. `saved`: `"amended"` (with `amend_seq` = new state), `"new"` (in mode `"auto"` with `reason` and `reason_label`) or `"unchanged"` |
| `revision.update` | `{node, rev, body?, note?, based_on_amend?}` | `{node, rev, changed, amend_seq?, commit?, mirror?}` – Round 23: step in the history and state check, see "Round 23 – Hardening"; changes text and/or note of the revision `rev` (human only), sets `edited`; `changed: false` without a change; empty text → -32000 „Eine Fassung braucht Text.“; Round 18b: Markdown headings (`# Titel` …) in the text become Typst headings as with `revision.save` (`baruch_typst::markdown`); a further heading is then refused by the method like a typed `=` line |
| `revision.delete` | `{node, rev, confirm: true}` | `{node, rev, deleted, reparented, commit?, mirror?}` – revision `deleted` deleted (human only), `rev` = current revision, `reparented` = children that now hang on the parent; without `confirm: true` → -32602 „Löschen bitte bestätigen (confirm).“; current revision → -32000 |
| `revision.tree` | `{node?, chapter?}` | with `node`: `{revisions: [RevisionInfo]}`; with `chapter: true` or without `node`: `{sections: [{node, number, title, current, revisions}]}` for the chapter of the section or the whole work. `RevisionInfo` = `{number, based_on, status, author, note, created, edited, current}` (without text) |
| `post.add` | `{node \| key, body}` | `{node, key, id}` – post in the section chat (trimmed); `id` = post |
| `post.delete` | `{id, confirm: true}` | `{id, node, key, deleted: {author, body}, commit?, mirror?}` – see "Managing posts" |
| `post.move` | `{id, target_key \| target}` | `{id, node, key, from_node, from_key, commit?, mirror?}`; Round 23: the findings of the post move along (except accepted ones) |
| `post.to_idea` | `{id, words}` | `{id, idea, node, key, commit?, mirror?}` |
| `preview.typst` | `{key?, page?}` | `{stand, pages, page, chapter, diagnostics, print, cached, millis, svg?}` – see "Typst preview" |
| `library.list` | – | `{titles: [TitleEntry]}` – titles of the author's library with quotations |
| `library.search` | `{query, kinds?, work?, all_revisions?, limit?}` | `{hits: [SearchHit]}` – full-text search, tool of the lector |
| `ui.render` | `{work?, key?, node?, rev?, view?, pick?, lq?, fassungen?}` | `{tree, editor, assistants, url, title}` – HTML of the three areas; `key` goes before `node` (unknown → fallback with a note); `url` = `/?work=W&key=K&rev=R[&view=preview][&pick=…][&lq=…][&fassungen=alle]` (0.9.0: `fassungen: "alle"` opens the switch „ältere Fassungen“, see "Hiding older revisions"). `pick` = `"<node>:<rev>[,…]"` tentatively inserts revisions into the preview (invalid entries skipped, the mark stays) |

All methods except `work.list` additionally take `work?` (written out in the table above only for `work.*` and
`ui.render`). Assistants only create proposals; `revision.save`, `revision.set_current`,
`revision.update` and `revision.delete` are reserved to the human and do not exist over MCP.
`wysiwyg.*` (see "Typeset view") only reads and is not under `/act`. `numbering.*` (see "Numbering schemes") and `asset.*`
(see "Images") are neither under `/act` nor over MCP; they have their own forms. Likewise `persona.*`
(see "Personas") and `idea.*` (see "Ideas"); both work without a work. Likewise (Round 9) `menu.*`,
`settings.*`, `connection.*` and `sources.*` (see "Menu", "Settings", "Connections and sources") and (Round 10)
`model.*`, `scripture.check`, `scripture.map`, `scripture.versifications` (Round 17a) and `mcp_token.*` (see "Personas"), plus (2026-11-06) `bible.*` (see "Bibles"). `ui.render` delivers in `editor`
the open proposals of the section (`details.pv`, see `docs/Bedienung.md`, proposals in the reading flow) and
reads the cookie `baruch_folded` of the request for that; since Round 8 they stand at their place in the reading flow
(`section.pv-flow`, per piece `details.pv[data-pv-at][data-pv-kind]`, forms `pv-accept-…`/`pv-reject-…`),
and the editor HTML follows the cookie `baruch_editor` (switch; with „satz“ a finished typeset view in the HTML).

### Managing posts (Round 8)

`post.delete`, `post.move` and `post.to_idea` run under the write lock and yield exactly one commit in the
mirror of the work (`allow_empty`, posts are not in the mirror). Over RPC it is always the human who acts; they may
manage all posts (MCP only its own, `docs/MCP.md`).

- `post.delete {id, confirm: true}`: without `confirm: true` → -32602 „Löschen bitte bestätigen (confirm).“;
  unknown id → -32602 „Beitrag N gibt es in Werk W nicht.“
- `post.move {id, target_key | target}`: to the end of the section chat of the target (`target_key` goes before
  `target`), only within the work; same target → -32000; no target → -32602.
- `post.to_idea {id, words}`: idea with work, reference = section, thought = text, source = sources joined with „; “
  (at most 200 characters), origin from the author (`@mcp:` → MCP, persona handle → persona, otherwise human);
  afterwards the post is deleted – one transaction. `words` as with `idea.create` (list or text). If
  the idea fails (e.g. no keyword), the post stays.
- Commit: subject „Beitrag <id>: gelöscht“ | „…: verschoben nach <Nummer Titel>“ | „…: in Einfall <n>
  umgewandelt“; trailers `Baruch-Post`, `Baruch-Action` (`post-delete` | `post-move` | `post-to-idea`),
  `Baruch-Node` (fixed key), with the conversion `Baruch-Idea`. Deleting and converting carry the whole
  post (author, time, section, sources, references, text) in the body; the conversion is **one** commit whose
  snapshot already contains `einfaelle.md` with the new idea.
- Form without JavaScript: `POST /beitraege/act` (fields `method`, `work`, `key` = shown section, `id`,
  `rev?`, `view?`, `lq?`, `confirm`, `target_key`, `words`) → 303 to `/?work=W&key=K[&rev][&view=preview][&lq]`,
  error 422 with text, foreign origin 403.

#### post.edit, post.versions, post.page, post.to_findings (Round 28, chat-auftrag)

Answered by `src/posts.rs` (store `post_edit.rs`, migration 0046; full-text index library 0107). The acting party is
the human.

- `post.edit {work?, id, body}` → `{id, work, node, key, body, edited, versions: [PostVersion], url, commit?|mirror?}`.
  Only posts of the human (author „Autor“); a post of a persona or an MCP client → -32000 „Nur eigene
  Beiträge lassen sich bearbeiten …“ (nothing changed, no commit). Text trimmed, at most 4000 characters; empty
  or unchanged → -32602. The old revision stays as `post_version`, `edited` = time of the change, exactly one
  commit „Beitrag <id>: bearbeitet“ with body `Vorher:\n…\n\nNachher:\n…` (trailers `Baruch-Action: post-edit`,
  `Baruch-Post`, `Baruch-Node`). The full-text search (`library.search`) then finds the new text (trigger
  `post_fts_edit`).
- `post.versions {work?, id}` → `{id, work, versions}`; `PostVersion` = `{id, uuid, body, valid_from, replaced,
  edited_by}`, oldest first, empty for posts never edited.
- `post.page {work?, key, before?, limit?}` → `{work, key, posts, older, history_url}`: the up to `limit` (1–200,
  default `CHAT_PAGE_POSTS` = 50) posts before the post `before` (without `before`: the newest), oldest first;
  per post `{id, author, body, created, time, edited, edited_time, own, style, sources, url}`; `older` = how many
  still stand before them. A `before` from another section → -32602.
- `post.to_findings {work?, id}` → `{id, work, key, findings: [finding_json], attached: [finding_json]}`: splits
  the post through `findings::ingest` with `manual = true` (without markers one „Hinweis“ with the whole text); the
  post stays byte-identical, no commit (creating findings is not one).
- The form `POST /beitraege/act` additionally takes `method=post.edit` (field `body`) and `method=post.to_findings`;
  with `back=post` it then goes to the post page `/beitrag?work&id` instead of the section.
- Pages without JavaScript: `GET /beitraege?work&key` (whole chat history, anchors `#post-<id>` and `#chat-neueste`,
  chat form) and `GET /beitrag?work&id` (text in a read-only, selectable field, „Zitieren“ as a
  chat form with „> …“, editing one's own posts, history, „In Befunde umwandeln“).

### Typst preview (Round 8)

`preview.typst {work?, key?, page?}` – read only, no commit, answered by `src/typeset.rs`. The work is
typeset with embedded Typst 0.15.1 (work template, numbering scheme, images of the work; without network, without
packages). At most one run at a time, a cache for 8 states and together at most 192 MiB SVG (Round
18a; the newest state always stays).

- `key`: fixed key of a section; unknown → -32602. `page`: page from 1; then the answer carries
  `svg` (the page as SVG text); outside `1..=pages` → -32602.
- Answer: `{stand /*SHA-256 hex over text and images*/, pages /*number*/, page /*page of the section key, without
  key 1, with page that one*/, chapter: {first, last}, diagnostics: [{severity: "error"|"warning", message, hints,
  line, column, file?, key, section /*„2.1 Titel“ | „Werkvorlage“ | null*/, section_line}], print: [PrintWarning],
  cached, millis, svg?}`. `PrintWarning` (Round 11) = `{path, page, width_px, height_px, width_mm, height_mm,
  dpi, needed_width_px, needed_height_px, message}` for every raster image under 300 dpi at its typeset size
  (not in `diagnostics`). Typesetting includes the frame (cover, front matter) when it is switched on.
- Part pages (Round 14): if part marks (`… <baruch-part>`) stand in the work, a line `#show <baruch-part>: …` is added
  after work template and frame (`baruch_typst::part_page_rule` with the numbers of the parts):
  each part alone on its own right-hand page, the Typst heading counter stays untouched. Lines in
  `diagnostics` (`section_line`) still refer to the section. Without a part mark the
  typeset source is byte-identical to `work.export` (or with frame as before).
- On an error: `pages = 0`, `diagnostics` with `severity: "error"`. Time limit 60 s → -32000 „Typst brauchte
  länger als 60 s“. Cancelled through `preview.cancel` or the form of the typesetting page (Round 18a) → -32000
  „Satz abgebrochen.“ (immediately, also while waiting for the lock).
- `chapter` (Round 18a): a page shared by two chapters counts for both – `last` of a chapter is the
  page on which the next one begins if text from its lines still stands there (heading lines do not count,
  because the template may quote them as running heads), otherwise the page before.
- Pages without RPC: `GET /werke/{id}/satz?key=&page=` (HTML) and `GET /werke/{id}/satz/{stand}/{n}.svg`
  (`image/svg+xml`, `nosniff`, `Content-Security-Policy: sandbox; default-src 'none'; img-src data:;
  style-src 'unsafe-inline'`; foreign, old or invalid state, page 0 or beyond the end → 404).
  Round 18a: both (and `GET /werke/{id}/pdf`; since the follow-up to Round 18 also all other GETs that start
  Typst: `/werke/{id}/umschlag`, `/werke/{id}/umschlag.svg`, `/werke/{id}/umschlag.pdf`,
  `/werke/{id}/stichwortzettel.pdf` and `/werke/{id}/karte.pdf`) answer with `Sec-Fetch-Site: cross-site` or `same-site` with 403
  „Abgelehnt: Der Satz startet nicht auf Anfrage einer fremden Seite.“, without starting Typst; without the header (curl,
  older browsers) or with `same-origin`/`none` as before. The typesetting page loads no `main.js`; its ribbon has
  the form `POST /werke/{id}/satz/abbrechen` (field `key?`, behind the origin guard: foreign origin → 403) → 303
  to `/werke/{id}/satz?abgebrochen=1[&key=K]` (if none was running: `abgebrochen=0`). With `abgebrochen` the page does
  not typeset anew but only shows an already typeset state or the message with „Neu setzen“.

### preview.cancel

`preview.cancel {work?}` → `{cancelled: bool}` (Round 18a, `src/typeset.rs`). Cancels all Typst runs of the work
started by `preview.typst`, the typesetting page, its page images or MCP `typst_check` – including those still
waiting for the lock or computing on in the background after the time limit. `cancelled` = at least one was running.
The waiting requests return immediately with -32000 „Satz abgebrochen.“; a cancelled run puts nothing into the
cache. Typst itself stops only after the layout (before writing the SVG pages) and holds the
lock until then; a long loop in the template runs to its end (see `docs/Architektur.md`, "Round 18a – Ribbon"). PDF generation
(`work.pdf`, `/werke/{id}/pdf`) is not cancelled. Unknown work → -32602. No commit.

### ribbon.formats

`ribbon.formats {work?}` → `{formats: [RibbonFormat]}` (Round 18a, `src/ribbon.rs`), read only. The buttons of the
ribbon: first the formats of the work template – `scene` only if it defines `#let szene(…)` or `#let szene =`,
`work` only with `#let arbeit` –, then always `footnote`, `quote`, `list`, `enum`, `terms`, `break`, `link`.
`RibbonFormat = {cmd /*command to the typeset view, baruch:ws-command*/, label /*„Szene“*/, title /*tooltip*/, snippet
/*Typst text for the source, e.g. "#szene[\n\n]\n", "#footnote[]", "- \n", "\\\n"*/}`. The same list stands
in the ribbon of the editor (`data-ribbon-cmd`, `data-ribbon-snippet`). Unknown work → -32602.
Since Round 30a `xref` („Verweis“) is added; since Round 19b, after `scene`/`work` from the template: `scripture`
(„Bibelstelle“) only if the template defines `#let bibelstelle(…)` or `#let bibel(…)` (snippet `#bibelstelle[]`
or `#bibel[]` with the name found; assumption: the function takes the reference as content), and `cite`
(„Literaturverweis“, snippet `@`) only if the template contains `#bibliography(` (not in a comment).
Recognition works as with `szene`, without Typst (`ribbon::defined_functions` over `code_lines`), but since Round 23 no longer
merely line by line: line and (also nested) block comments do not count, a `#let` in the body of another
function (bracket depth > 0) does not count, and names introduced with `#import "datei.typ": name` count (`*` does not;
renaming `a as b` → `b`).

### Book as PDF and cover (Round 11)

Read only or for the human only; not over `/act`. All take `work?`. Errors: -32602 input,
-32000 domain (REFUSED).

| Method | Parameters | Result |
|---|---|---|
| `work.pdf` | `work, inline?: bool = false` | `{file_name: "<slug>.pdf", pages, bytes, sha256, url: "/werke/{id}/pdf", print: [PrintWarning], stand, millis, pdf_base64?}` – the book like the typesetting view (same page count, also with the part pages from Round 14); `pdf_base64` only with `inline: true`. Typesetting error → -32000 „Kein PDF (Buch): Der Satz scheitert mit n Fehler(n). Erster: …“; already running → -32000 „Für dieses Werk entsteht gerade ein PDF …“; time → -32000 „Typst brauchte länger als 120 s“. No commit |
| `work.set_cover` | `work, name: string\|null` | `{work, cover: CoverInfo\|null, commit?, mirror?}` (commit „Umschlag: <name> gesetzt“ or „Umschlag entfernt“ only on a change) |
| `work.meta.get` | `work` | `{work, meta: BookMeta + title/subtitle/author/bible + paper_sheet_mm (Round 20c: thickness per sheet = 2 × paper_mm), cover: CoverInfo\|null}` |
| `work.meta.set` | `work, subtitle?, author?, bible?, publisher?, place?, year?, edition?, isbn?, price?, series?, printer?, bible_notice?, image_credit?, dedication?, blurb?, bio?, publisher_address?, printer_address?, author_photo?: name\|null, title_area?: "top"\|"middle"\|"bottom", front_matter?, cover_pages?, paper_mm?, paper_sheet_mm? (Round 20c, per sheet, above 0 up to 2 mm; not together with paper_mm → -32602), bleed_mm?` | `{work, changed: [...], commit?, mirror?}` (commit „Buchangaben geändert: <Felder>“; `subtitle`/`author`/`bible` as `work.update`) |
| `work.cover_pdf` | `work, inline?: bool` | `{file_name: "<slug>-umschlag.pdf", pages, bytes, sha256, url: "/werke/{id}/umschlag.pdf", spine_mm, checks: [{code, block, message}], pdf_base64?}` – cover for the print shop (one page); `checks` = print check of the cover (Round 27) |
| `notes.get` | `work` | Round 16, read only: `{work, title, kind: "book"\|"sermon", format_choices: ["a5", "karten"], cards: [{key, number, title, depth, start_clock, clock, points: [String], refs: [String], rule: "marker"\|"strong"\|"first_sentence"}], total_clock, target_clock\|null}` – note sheet: per outline item time mark and duration from the time watch (like `work.timing`), bullet points by the rule in `docs/Bedienung.md` (note sheet) (at most 6 of at most 160 characters each), valid Bible references normalised without duplicates |
| `notes.pdf` | `work, format?: "a5"\|"karten" = "a5", font_pt?: 14–28 (default 16 or 14), inline?: bool` | `{file_name: "<slug>-stichwortzettel.pdf", format, font_pt, pages, bytes, sha256, url: "/werke/{id}/stichwortzettel.pdf?format=…&schrift=…", print, millis, pdf_base64?}` – like `work.pdf` (one generation per work, same errors with „Kein PDF (Stichwortzettel): …“); wrong format or size → -32602. No commit |

`CoverInfo = {name, caption, alt, url: "/werke/{id}/bilder/{name}/datei", thumb: "/werke/{id}/bilder/{name}/vorschau?b=320"}`.

Pages of the note sheet (Round 16, GET only): `/werke/{id}/stichwortzettel?format=a5|karten&schrift=14–28`
(choice of format, preview of the cards, link to the PDF; wrong input → 422 with page) and
`/werke/{id}/stichwortzettel.pdf?format=&schrift=` (`application/pdf` as attachment `<slug>-stichwortzettel.pdf`;
409 already running, 422 typesetting error or wrong input, 503 time expired – as a page).
`BookMeta = {publisher, place, year, edition, isbn, price, series, printer, bible_notice, image_credit,
dedication, blurb, bio, author_photo, cover, title_area, front_matter, cover_pages, paper_mm, bleed_mm, updated}`.
Limits: single-line fields 200 characters, `blurb` 4000, `bio` 2000, `dedication`/`bible_notice`/`image_credit`
1000; `year` empty or four digits; `isbn` empty or ISBN-13 with check digit; `paper_mm` (0, 1], `bleed_mm`
[0, 10] (number or text with a decimal comma); unknown image, invalid `title_area` → -32602.

Pages and forms (without JavaScript): `GET /werke/{id}/pdf` (200 `application/pdf`, `Content-Disposition:
attachment; filename="<slug>.pdf"`; 404 unknown work, 409 already running, 422 HTML with messages and links,
503 time), `GET /werke/{id}/umschlag` (page), `…/umschlag.svg` (preview, CSP `sandbox`), `…/umschlag.pdf`
(attachment `<slug>-umschlag.pdf`, error codes like `/pdf`), `POST /werke/umschlag/act` (`work, action=set|remove|area,
name, title_area, back`), `POST /werke/buchangaben/act` (`work`, all fields, checkboxes `front_matter`,
`cover_pages`, `back`). `back` only relative and safe (default `/einstellungen/werk?work=W`); target in the
settings → `&ok=umschlag|buchangaben`, otherwise `?hinweis=umschlag_gesetzt|umschlag_entfernt|buchangaben`;
invalid input → 422 with the input kept.

Command line: `baruch-server export <id|slug> --pdf DATEI [--data ORDNER]` (never overwrites; typesetting error →
exit 1 without a file; with `--out` → exit 2).

### Works
Methods from `crates/baruch-server/src/works.rs`, for the human only (user interface, RPC), **not over MCP**.
They run before the choice of the default work and therefore also work when there is no work. Titles are unique:
two works may not have the same title, where upper/lower case and white space do not count
(„ Das Buch “ = „das  buch“).

| Method | Parameters | Answer |
|---|---|---|
| `work.meta` | `{work?}` | `{work: WorkMeta, template_revisions: [{number, author, note, created}]}`; `WorkMeta` = `{id, slug, kind, title, subtitle, author, audience, bible, template, template_rev, source_name, created, updated}` (`audience` = target audience, `bible` = Bible translation, `template_rev` = number of the current revision of the work template) |
| `work.create` | `{title, author?, kind?, preset?, subtitle?, audience?, bible?, text_ref?, module?, from_work?}` | `{work, slug, title, nodes, sermon_text?, commit?, mirror?}` – new work; `kind` and `preset` from `book`, `sermon` (if one is missing, it follows the other, otherwise book). Book: work template `#set document(title, author)` + `#set heading(numbering: "1.1")`, chapter „Erstes Kapitel“; sermon (Round 16a): `#set document`, numbering scheme `sermon`, items „Predigttext“ (without number), „Hinführung“ (introduction), „Beobachtung am Text“, „Deutung“, „Zuspruch“ (I.–III.), „Anwendung“ (without number), „Ausblick“ (conclusion). Per outline item revision 1 by the human (`= <Titel>`), template as revision 1. Round 16a: `text_ref` (only for a sermon, at most 500 characters, exactly one valid Bible reference, otherwise -32602 and nothing created) sets the sermon text: with a readable SWORD Bible (`module`, empty: the Bible of `from_work`, otherwise of the default work) as `#quote(block: true, attribution: [<Stelle> (<Modul>)])[<Wortlaut>]`, without a Bible only the reference; the chosen Bible becomes the SWORD Bible of the new work. `sermon_text: {ref, normalized\|null, module\|null, quoted, verses, missing, notice\|null}` (`verses` = inserted verses, `missing` = those missing in the module). Without `text_ref`, `{kind: "sermon"}` stays valid. Form `POST /werke/act` (`action=create`, plus `text_ref`, `module`, `from_work`) → 303 `/?work=N`, error → form page with message and input (422 or 409) |
| `work.import` | `{file_name, source, title?}` | `ImportReport` = `{work, slug, title, nodes, commit?, mirror?}` – Typst text as a new work like `baruch-server import`; only file names ending in `.typ`; `source` at most 16 MiB (UTF-8 bytes), above that -32602 „<datei> ist größer als 16 MiB“; `title` (empty = from `#set document`, otherwise the file name). The whole request to `POST /rpc` may be at most `works::RPC_BODY_LIMIT` = 2 × 16 MiB + 64 KiB (JSON escaping can double the text); the server refuses a larger request before reading it with **413** |
| `work.update` | `{work, title?, subtitle?, author?, kind?, audience?, bible?, template?, note?}` | `{work, changed, template_rev?, commit?, mirror?}` – `changed` = fields with another value (`title`, `subtitle`, `author`, `kind`, `audience`, `bible`, `template`); a different work template (CRLF → LF) becomes the new revision `template_rev` with note `note`, the old ones stay readable; a heading in the template → -32000. Slug and mirror folder stay when renaming |
| `work.delete` | `{work, confirm_title}` | `{deleted, title, slug, archive, next, commit?, mirror?}` – deletes the work with outline, revisions, posts and template revisions; `confirm_title` must be the title (upper/lower case and white space do not matter), otherwise -32000 „Der eingegebene Titel passt nicht; nichts gelöscht.“; `archive` = `works/.geloescht/<slug>-<JJJJ-MM-TT>` (mirror folder afterwards), `next` = new default work or `null` |

`work.update` and `work.delete` require `work` explicitly (missing → -32602). Errors: title taken →
-32000 „Es gibt schon ein Werk „…“ (slug).“, empty title → -32602 „Ein Werk braucht einen Titel.“, unknown
work → -32602. Mirror: creating → commit „Werk angelegt: <Titel>“; changing → „Werk geändert: <Felder>“ (e.g.
„Werk geändert: Titel, Werkvorlage Fassung 3“), only if a file changes (title without work template, template);
deleting → empty log commit „Werk gelöscht: <Titel>“ with trailer `Baruch-Action: work-deleted`, after which
the folder moves to `<data>/works/.geloescht/<slug>-<JJJJ-MM-TT>/` (on the same day `-2`, `-3` …). An
error in the mirror does not stop the deletion in the database; the answer then carries `mirror`.
id, slug and title of the deleted work afterwards stand in `work_retired`: the database never assigns the id
again (`AUTOINCREMENT`, migration 0007), the slug stays blocked for new works (a new work with the same
title is called `<slug>-2`). If all works are deleted, the next start seeds no sample work (`store_meta.seeded`).

Without JavaScript: `POST /werke/act` (behind the same protection as `/act`, foreign `Origin` → 403) with `action` =
`create`, `update`, `delete` and the fields as above (`work`, `title`, `author`, `kind`, `preset`, `subtitle`,
`audience`, `bible`, `template`, `note`, `confirm_title`); then 303 to `/?work=<id>` (create, update) or
`/werke?geloescht=<Titel>` (delete). Errors as an HTML page with the form: 422 (invalid), 409 (refused).
`POST /werke/import` (multipart, fields `file` and `title`, request at most 16 MiB + 64 KiB): success → 303
`/?work=<id>`, not `.typ` or not UTF-8 → 422, larger than 16 MiB → 413, already imported or title taken
→ 409 with a note. Pages: `GET /werke`, `/werke/neu`, `/werke/{id}/bearbeiten[?vorlage=n]`,
`/werke/{id}/loeschen`, `/werke/{id}/export` (`Content-Disposition: attachment; filename="<slug>.typ"`, text
of `work.export`).

### Outline
Methods from `crates/baruch-server/src/outline.rs`; all take `work?`, human only (not over MCP; there
are `outline_add` and requests, see "Outline requests" below and `docs/MCP.md`).
Numbers may come as text, booleans as `true`, `"1"` or `"on"` (forms). Levels: part 0
(since Round 14 with the part mark `#heading(level: 1, numbering: none)[…] <baruch-part>` as the first line),
chapter 1 (`= `), section 2 (`== `), subsection 3 (`=== `; each also as the mark „ohne Nummer“); the
outline is the sequence of levels in tree order (parent = nearest predecessor with a smaller level).
Invalid (chapter under a section, level 4, into its own subtree, part under an item or next to
a section, changing the level to or from level 0, no effect) → -32000, nothing changed. The restriction „Teile nur
in Werken ohne Werkvorlage“ no longer exists since Round 14.

| Method | Parameters | Answer |
|---|---|---|
| `outline.move` | `{node, target, place}` | outline answer – `place` = `before`/`after` (before the target or after its sub-items) or `into` (last sub-item); **the level stays**: `before`/`after` only with the same level as the target, `into` only with the target's level = level − 1, otherwise -32000 „Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).“; sub-items move along |
| `outline.set_level` | `{node, level}` | outline answer – level 1–3, sub-items by the same difference; never parts, nothing becomes a part (0 → -32000); a mark „ohne Nummer“ stays a mark (only `level:` changes) |
| `outline.rename` | `{node, title}` | outline answer – new title, new heading line (escaped; the mark „ohne Nummer“ stays a mark). Part (Round 14): with a part mark only its line is renewed (comment stays), an empty part text gets one; a part text without a mark (legacy „= Teil …“) stays unchanged, only the title changes (`detail` says „Text ohne Teil-Markierung unverändert“) |
| `outline.add` | `{target, place, title, level?}` | outline answer + `adopted: [key]` – new item, revision 1 by the human = heading line + empty line. `level` (Round 14) = absolute level 0–3; if missing, the level of the place. `level: 0` creates a **part** (text = part mark + empty line): only with `place` `before`/`after` at a part or chapter, also with a work template; `into` with 0 or a section as target → -32000. `adopted` = fixed keys of the existing chapters that now stand under the new part (order, texts and chapter numbers unchanged; otherwise `[]`) |
| `outline.delete` | `{node, confirm: true}` | outline answer – without `confirm` -32602 „Löschen bitte bestätigen (confirm).“; with sub-items or as the last chapter -32000; `node` = the item now shown (the previous one) |
| `outline.check` | `{node, body}` | `{title, level, split, merge, valid, message}` – what saving would change (writes nothing; Markdown headings count as with `outline.save`): `split` = titles of new headings, `merge` = no heading |
| `outline.save` | `{node, body, based_on?, based_on_amend?, split?, merge?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, outline?, commit?, mirror?}` – as before; `mode` as with `revision.save`. Splitting, merging, a new heading or level always yield a new revision (`reason: "heading_changed"`) |
| `outline.set_numbered` | `{node, numbered}` | outline answer (`action: "numbered"`) – `numbered: false` counts the item without a number (preface, introduction), `true` with one again; the other numbers move up, the text stays; already so → -32000; part with `numbered: false` → -32000 (parts always count: the part mark knows no „ohne Nummer“, export → import would count it again); `numbered` missing → -32602 |
| `outline.requests` | `{open? = true, preview? = false}` | `{work, requests: [OutlineRequest]}` – requests of the assistants (read only), with `open: false` also decided ones; with `preview: true` (Round 14) per request `preview: [PreviewPoint]` or `preview_error` (see "Outline requests") |
| `outline.request_decide` | `{id, decision: "confirm" \| "reject"}` | `{request, node?, key?, outline?, commit?, mirror?}` – see "Outline requests"; since Round 16 (user decision 2026-11-08) every decision without an outline change (refused or stale, every kind of request) also carries `commit` (empty log commit) or `mirror` |
| `outline.set_numbering` | `{numbering}` | outline answer – `null` = scheme „aus der Werkvorlage“, `""` = no numbers (`none`), otherwise a pattern of the pattern language (scheme `custom`; `1.1`, `I|1`, `I.;A.;1.;a);aa)`); invalid → -32602 „Nummerierung: …“, unchanged → -32000. The form stands in the head of the tree (with and without JavaScript) |

Titles (`outline.rename`, `outline.add`, heading lines in `outline.save`, `revision.save`,
`revision.update` of the current revision, `revision.set_current`/`revision.accept`, proposals over MCP and
`highlight.cite`): not empty, without a line break, at most 200 characters; otherwise -32602 „Ein Gliederungspunkt
braucht einen Titel.“ / „Der Titel darf keinen Zeilenumbruch enthalten.“ / „Der Titel ist zu lang (höchstens
200 Zeichen, hier N).“, nothing changed. `outline.check` reports the same as `valid: false` with `message`.
(Until Round 6 an empty title was -32000.) Existing empty titles from an import stay until one renames.

Outline answer: `{node, action, title, detail, revisions: [{node, rev}], renumbered: [[alt, neu], …],
commit?, mirror?}` – `node` is the seq afterwards, `action` one of `moved`, `level`, `renamed`, `added`,
`deleted`, `split`, `merged`, `numbering`, `numbered`, `revisions` the new revisions (heading lines only),
`renumbered` the changed seq. Every change assigns seq anew in tree order and rewrites references in
posts (references to deleted items are dropped); the key `key` stays. `revision.save` never changes
the outline (no splitting, no merging, no change of level: it refuses such texts, see
"Headings and outline" above); the editor saves through `outline.save`. As after every change of the
current revision (`revision.save`, `revision.accept`, `revision.set_current`, `revision.update` of the current
revision), the outline item takes the title from the heading line of the same level (`sync_heading`);
`revision.accept` and `revision.set_current` also take over another level (see above).

Without JavaScript: `POST /gliederung/act` (behind the same protection as `/act`, foreign `Origin` → 403) with
`method` = `outline.*` (except `outline.check`) and the fields `work`, `node`, `key`, `target`, `target_key`, `place`, `level`,
`title`, `body`, `based_on`, `based_on_amend`, `split`, `merge`, `confirm`, `numbered` (`1`/`0`), `id`, `decision`, `numbering` (`vorlage` = from the template, `keine` =
no numbers, otherwise a pattern), `view`, `lq`; then 303 to `/?work&node[&rev][&view=preview][&lq]` or with
`view=gliederung` to `/gliederung?work&node[&lq]`. Errors 422 (invalid) or 409 (refused). If
`outline.save` would split or merge, it answers 200 with a confirmation page (text in a hidden
field, buttons „Teilen und speichern“ or „Zusammenführen und speichern“ with `split=1`/`merge=1`, „Zurück“).
Pages: `GET /gliederung?work&node&lq` (outline manager), `GET /gliederung/bearbeiten?work&node&lq` – the
fallback without JavaScript; with JavaScript the user interface does not link them (tree with a context menu, see
`docs/Bedienung.md`).

#### Outline requests

Migration 0012 (`outline_request`), wish of 2026-11-06 „UD nach Bestätigung“. Assistants create requests over MCP
(`outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`, `outline_delete`);
the human decides with `outline.request_decide`. Since user question 12 (decision 2026-11-07, migration 0035)
MCP `outline_add` also becomes a request (kind `add`) if a new part would take over existing chapters; such
requests need the **active approval** of the human (dialog on every page, see `docs/Bedienung.md`).

`OutlineRequest` = `{id, work, op: {op: "rename"|"move"|"set_level"|"set_numbered"|"delete"|"add", key, title? |
target_key, place? | level? | numbered? | place, points: [{title, level?}], model}, expected: {title, level,
target_title?, adopted?}, origin: {kind: "persona"|"mcp",
handle}, origin_label, note, created, decided, decision: null|"confirmed"|"rejected"|"stale", decision_label,
decided_by, message, summary}`. `expected` is the state at the time of the request, `summary` the German sentence (`„„Sommer“ in
„Hochsommer“ umbenennen“`).

- `decision: "reject"` → `decision: "rejected"`, the outline stays; for **every** kind of request (user decision
  2026-11-08, Round 16; before only `add`) an empty log commit „Gliederung: Antrag N abgelehnt“ with trailers
  `Baruch-Action: outline-request`, `Baruch-Decision: rejected`, `Baruch-Origin`, `Baruch-Request`,
  `Baruch-Requested-By`, `Baruch-Decided-By`; the answer carries `commit` (or `mirror` on a mirror error).
  Likewise „… veraltet“ with `Baruch-Decision: stale` if a request is stale at confirmation.
- `decision: "confirm"` → if the request is stale (item or target gone, title or level different from
  `expected`) or the outline function now refuses, it is closed with `decision: "stale"` and `message`
  (outline unchanged, empty log commit „… veraltet“, see above). A request `add` is also
  stale if the new part would now take over other chapters than `expected.adopted`. Otherwise it runs like `outline.rename`/`move`/`set_level`/
  `set_numbered`/`delete` under the name of the human (`add`: the new items like `outline.add`, revision 1 by the
  requester): `decision: "confirmed"`, `outline` = outline answer,
  exactly one commit „Gliederung: … (Antrag N)“ with trailers `Baruch-Origin`, `Baruch-Request`,
  `Baruch-Requested-By`, `Baruch-Confirmed-By`.
- Already decided → -32000 „Über diesen Antrag ist schon entschieden.“; unknown id → -32602 „Antrag N gibt
  es in Werk W nicht.“; `decision` missing or different → -32602.

**Preview (Round 14):** `outline.requests` with `preview: true` computes for each open request the outline after
confirmation (store `outline_request_preview`: executed in a savepoint and rolled back, the database
stays): `preview` = `[PreviewPoint]` in tree order, `PreviewPoint` = `{key, level, number, title, mark,
kind_word, before_number, before_level, label, before_label, changed_label}` (Round 16: `kind_word` „Teil“ |
„Kapitel“ | „Abschnitt“ | „Unterabschnitt“; `label` kind and number according to the scheme, e.g. „Teil I“, „Kapitel 2“, for
sermons „Predigtteil II“, without number „Kapitel (ohne Nummer)“; `before_*` the state before confirmation, `null` for
new items; `changed_label` = kind or number change or the chapter moves under a new part – then
page and dialog show „(vorher … → nachher …)“),
`mark` = `same` | `renamed` | `moved` (the moved item with its sub-items) | `level` (also sub-items moving
along) | `numbered` | `deleted` (deleted items at their old position, with old level and number) |
`added` (new item of a request `add`) | `adopted` (chapter that moves under the new part).
Titles, levels, numbers and order (without `deleted`) are those of the outline after `outline.request_decide`
with `confirm`. Instead of `preview` there is `preview_error` (text) if the request is stale („Veraltet: …“ with
reason), the outline function now refuses („Nicht mehr ausführbar: …“) or it is already decided. The page
`/gliederung#antraege` shows the preview of each open request without JavaScript (`<details>` „Vorschau“).

Without JavaScript: form to `POST /gliederung/act` with `method=outline.request_decide`, `work`, `id`, `decision`
(`confirm`/`reject`); then 303 to `/gliederung?work=…#antraege`. `outline.requests` is not under `/act`.

**Approval on every page (user question 12):** As long as a request `add` is open, the layer
`crate::consent` puts into every HTML page with the marker `<!--baruch-consent-->` (from `base.html`) a notice bar
with the link `/gliederung?work=W#antrag-N` and a `<dialog id="consent" data-widget="consent">` with preview and
the forms „Zustimmen“/„Ablehnen“ (the same fields as above, protected like all forms by
`guard::same_origin`). The widget `web/src/consent.ts` opens the dialog modally and sends the decision through
`outline.request_decide`; without a click nothing happens. Other answers (JSON, PDF, fragments) stay
unchanged. There are no new RPC methods for this.

#### Numbering schemes

Methods from `crates/baruch-server/src/numbering.rs`; all take `work?`, human only (not over MCP).
`scheme` is an id: `template`, `decimal`, `decimal_dot`, `alphanumeric`, `chapter_roman`,
`chapter_arabic`, `parts_chapters`, `chapter_words`, `legal`, `sermon`, `book`, `none`, `custom`; `custom` is the
own pattern (only with `custom`; if it is missing, the stored one applies). Unknown scheme, missing or
invalid pattern → -32602.

| Method | Parameters | Answer |
|---|---|---|
| `numbering.schemes` | `{}` | `{schemes: [{id, label, example, preview: [NumberPreview], more, error?}], current: NumberingInfo}` – `preview` = the first 12 items of the work's own outline, `more` = further ones; `error` instead of a preview (e.g. own pattern missing) |
| `numbering.preview` | `{scheme, custom?}` | `{numbers: [NumberPreview], rule}` – all items; `rule` = the line `#set heading(numbering: …)` that the export puts after the template, `null` for `template`. Writes nothing |
| `numbering.set` | `{scheme, custom?}` | outline answer (`action: "numbering"`, `detail` e.g. „Gliederungsschema „Juristisch““, „Nummerierung „I|1““, „keine Nummern“, „Nummerierung aus der Vorlage („1.1“)“), `commit?`/`mirror?`; unchanged → -32000 |

`NumberPreview` = `{node, key, level, number, title}` (position, fixed key, level 0–3, number according to the
scheme, title), in tree order. `NumberingInfo` = `{numbering, template, effective, scheme,
scheme_label, custom}`: `numbering` = pattern language (`null` = from the template or a scheme with words, `""` =
none), `template` = pattern of the work template, `effective` = display text (pattern, `""` = none, for schemes
with words their example), `custom` = stored own pattern (even if another scheme currently
applies).

Round 14 (names and answer shapes unchanged): with `template` the numbers in `preview`/`numbers`
and in the tree come from `Numbering::from_template` – a string in the template is a literal Typst pattern (`;`, `|`
are text), Baruch's own rule of a scheme with words (from an earlier export) counts again like
that scheme, a function is evaluated exactly per level. `NumberingInfo.effective`/`template` still show for
such a function the approximation (first `numbering` pattern); for Baruch's rule of a scheme with
words `numbering` is `null` and `effective` the example of „Aus der Werkvorlage“. Chapters with „Nummer
aus“ stand in the export (`work.export`), in the export map, in the mirror and in typesetting as
`#heading(level: N, numbering: none)[…] <baruch-unnumbered>`; the stored texts stay unchanged.

Without JavaScript: page `GET /werke/{id}/nummerierung[?schema=ID&muster=SPEC]` (also `scheme`/`custom`, as
the button „Vorschau“ sends it by GET), form `POST /werke/nummerierung/act` (behind the same protection
as `/act`, foreign `Origin` → 403) with `work`, `scheme`, `custom`; then 303 to
`/werke/{id}/nummerierung?ok=1` (notice „Das Gliederungsschema ist übernommen.“), errors 422 (invalid)
or 409 (unchanged) with the page and the message. Mirror: exactly one commit „Gliederung: <detail>“,
trailer `Baruch-Action: outline-numbering`, even without a file change.

Git mirror for the outline: every outline change yields exactly one commit (even without a file change, e.g.
for the numbering), author and committer the human, subject `„Gliederung: <detail>“` (e.g. `„Gliederung:
„Sommer“ vor „Frühling“ verschoben“`, `„…: „Aussaat“ Ebene 2 → 1“`, `„…: „Alt“ → „Neu“ umbenannt“`, `„…: „Tiefe“
gelöscht“`), body with the new revisions, the renumbering and the full text of deleted items,
trailers `Baruch-Action: outline-move|outline-level|outline-rename|outline-add|outline-delete|outline-split|outline-merge|outline-numbering`
and `Baruch-Node`. Section files are named `{seq:03}-<slug(aufgelöster Titel)>.typ`; after a rearrangement
they are renamed.

### Import and export
A work is created from a Typst file through `baruch-server import <datei> [--title "…"]`
(`docs/Bedienung.md`), by uploading on `/werke/neu` or in the dialog „Importieren…“ (`POST /werke/import`)
or RPC `work.import`; there is no MCP tool for it (an import creates current revisions in the name of the
human). The title must be free (otherwise -32000, in the CLI with the hint at `--title`); the same file
a second time stays „schon importiert“, even with another title. `work.export` delivers the work again as one file: work template +
current texts in outline order, without the `#import "../vorlage.typ": *` lines that only stand in the
Git mirror before every section file. Directly after the import this is byte for byte the input (after
CRLF → LF). If a stored text ends without a line end, the export adds one so that the next
heading stands at the start of a line. With a numbering scheme other than „aus der Werkvorlage“ the
template is followed by the rule of the scheme (see "Numbering schemes"). Images come along through the CLI
(`import` reads `image("…")` paths relative to the folder of the file, `export --out ORDNER` writes text and
images, `docs/Bedienung.md`) and since Round 21b through a ZIP or a folder (`work.import_archive`, upload);
`work.import` and uploading a single file bring only the text.

Since Round 21b, `work.import {file_name, source, title?}` takes Markdown besides `.typ` (`.md`, `.markdown`, text
only, at most 16 MiB): the text is converted to Typst (table in `docs/Architektur.md`, "21b
markdown-import"), every image it names stands in `skipped` with the reason „nicht mitgeschickt …“, and what could not
be mapped in `warnings` (codes `md-…`, with the line of the Markdown file). A `.zip` through `work.import` →
-32602 with the hint at `work.import_archive`; any other ending → -32602.

Way back of an export (Round 21b, all import ways): rule block „// Baruch: Satz“ → typesetting settings of the new work
(`warnings` `import-typeset-adopted`, otherwise `import-typeset-kept`), show rules „// Baruch: Überschriften ohne Nummer“
→ `numbered = 0`, generated back matter cut off (`import-back-matter-cut` with line; an identifying line written by hand stays,
`import-back-matter-kept`), additions of the projection removed (`import-projection-removed`). Details in
`docs/Bedienung.md` ("Export → import, hint codes").

Parts and „ohne Nummer“ (Round 14): the lines `#heading(level: 1, numbering: none)[T] <baruch-part>` and
`#heading(level: N, numbering: none)[T] <baruch-unnumbered>` (canonical, `docs/Architektur.md`, parts in the
Typst text) become on import a part (top level, the following chapters under it) or an item with
`numbered = 0`; the text stays literal, `work.export` returns the file byte-identical (also with a
work template). Every other `#heading(...)` line stays text or work template as before.

Errors on import (codes as with RPC, in the CLI as a message on stderr and exit 1): file already imported
(same SHA-256 after CRLF → LF) → -32000 with „Diese Datei ist schon importiert: Werk „…“ (slug).“; title
taken → -32000 „Es gibt schon ein Werk „…“ (slug).“ (CLI: „… Mit --title "…" einen anderen Titel wählen.“); no
heading, not UTF-8, larger than 16 MiB, not a regular file (FIFO, device, folder: „… ist keine
Datei“) → -32602.

### source_import.preview, source_import.get, source_import.apply, source_import.discard (21a)

Global (without a work; switch in `rpc::call` after `works::handles`, module `pdf_import`). No MCP counterpart: an import
creates current revisions under the name of the human (decision 2026-11-05), for PDF/EPUB neither.

- **`source_import.preview {file_name, content, way?, collection?, title?}`** – `content` = file as Base64 (standard,
  with `=`); `file_name` ends in `.pdf` or `.epub`; `way` = `"auto"` (default) | `"multigpt"` | `"local"`. Reads the
  file and creates only a **draft** (no work, no title). Answer:
  `{draft, file_name, kind: "pdf"|"epub", way: "multigpt"|"local", way_reason, title, author, outline: [{index, level
  1..3, title, chars, part, sample}], warnings: [{code, message, line}], skipped: [{path, reason}], assets: [pfad],
  pages?, document?: {collection, id}}`. `title` = title under which it would be taken over (own `title`, otherwise from the
  file, otherwise the file name); `part` = „möglicher Teil“ (a hint only; a part is created only with `part: true` in
  `apply`); `sample` = text sample (at most 240 characters); `index` = number of the section for `apply`.
  - Way: EPUB always local (reason in `way_reason`). PDF with `"auto"`: MultiGPT if a connection is set up and
    `collection` is given, otherwise local (reason). `"multigpt"` without a connection or without `collection` → -32602.
  - MultiGPT fails (upload, indexing, reading) → -32000 with the reason, „Weg multigpt“ and the offer „lokal
    lesen“ (the same file with `way: "local"`); **no** automatic switch.
  - Local without Poppler → -32000 „pdftotext … ist nicht installiert …“.
  - Invalid (name, Base64, empty, larger than 64 MiB or above `RPC_BODY_LIMIT`, EPUB unsafe or too large –
    path escape, link, limits of `archive::IMPORT_LIMITS` –, no OPF) → -32602; nothing is created.
  - Warning codes (selection): `import-pdf-plain`, `import-pdf-page-numbers`, `import-pdf-numbering`,
    `import-pdf-pages-cut` (local more than 2000 pages: only the first read), `import-epub-footnote-number`
    (footnote number at the start of a footnote removed),
    `import-multigpt-plain`, `import-multigpt-truncated`, `import-heading-not-found`, `import-outline-heuristic`,
    `import-epub-table`, `import-epub-unsupported`, `import-epub-heading-as-text`, `import-epub-footnote-missing`,
    `import-epub-nav-target-missing`, `import-epub-deep-nav`, `import-epub-no-nav`, `import-front-matter`,
    `import-no-outline`; on taking over `import-outline-merged`.
- **`source_import.get {draft}`** → like `preview`. Unknown, discarded, taken over or older than 24 h → -32602.
- **`source_import.apply {draft, target?, title?, author?, outline?}`** – `target` = `"work"` (default) | `"library"` |
  `"both"`. `outline` = the outline confirmed by the human `[{level 1..3, title, part?, source?}]`:
  - with `source` (= `index` from the preview): every item with `source` gets the text of that section; `source`
    ascends (order as in the file, moving → -32602); sections without an item are **struck out**, their text goes
    to the previous item (at the start to the next), warning `import-outline-merged`; items without `source` are new,
    empty items (say, a part);
  - without any `source`: as many items as proposed, assignment by position;
  - `part: true` (level 1 only) creates a part (AGENTS.md, addition 2026-11-07: only on confirmation by the human);
  - without `outline`: the proposal, parts as chapters.

  Work through `import::import_generated` (note and commit „Import aus <datei>“, images of the EPUB as images of the work,
  warnings in the report). Source through `baruch_library::add_title` (kind `book`, key `q-…`, title/author, note with
  file and way) and – read locally – full text in `library_title_text` (migration 0108) or – through MultiGPT – the
  link `library_title_remote` (collection, document). Answer `{report?: ImportReport, library_title?: id}`; afterwards
  the draft is gone. Errors: title taken → -32000 with `TITLE_HINT` (draft stays); file already a source in the
  library (same SHA-256 or same document) → -32000, then no work is created either; outline invalid (empty,
  level, title missing, part not at the top, `source` wrong) → -32602; twice at the same time → -32000.
- **`source_import.discard {draft}`** → `{discarded: bool}` (`false` if it did not exist); not a valid name
  (32 lower-case hex characters) → -32602.

The local full text of a source created this way is readable through `source.read {title}` (`origin: "import"`, note with
file and date), and `highlight.add`/`highlight.check` check highlights without a quotation against it.

### work.import_archive (21b)

`work.import_archive {file_name, content, title?}` → `ImportReport` (like `work.import`, with `assets`, `skipped`,
`warnings`). `file_name` must end in `.zip` (otherwise -32602), `content` is the archive as Base64 (not valid
Base64 → -32602); the whole request lies within `RPC_BODY_LIMIT` (otherwise 413 before the call, so an archive up to
about 24 MiB – larger ones through the form `POST /werke/import`, up to 80 MiB).

The archive is read by `archive::read_zip` with `IMPORT_LIMITS` (2000 entries, 16 MiB each, together 80 MiB, ratio 200):
links, special files, `..`, absolute paths, `\`, duplicate names, zip bombs and broken archives → -32602 with a
German reason, nothing is created. It needs exactly one main file (`.typ`, `.md`, `.markdown`; with several
the shallowest `index.md`, `main.typ` or `README.md`), otherwise -32602 with the candidates. Images (`image("…")` or
`![…](…)`) are relative to the main file; missing, too large, unsafe ones and non-images stand with a reason in
`skipped`, files without a reference as the hint `archive-unused`. Title taken → -32000 with a hint at another
title (`title`), the same main file a second time → -32000 „schon importiert“. No MCP counterpart.

```json
{"jsonrpc": "2.0", "id": 1, "method": "work.import_archive",
 "params": {"file_name": "heft.zip", "content": "UEsDBBQAAAAIA…", "title": "Das Heft"}}
```

Since R21-S1 the report of an import (`ImportReport`, answer of `work.import`) carries the field `warnings`: a list of
`{code, message, line}` (`line` may be `null`) for everything that was not taken over literally; with the Typst import
empty so far.

### Git mirror
After `revision.accept`, `revision.set_current`, `revision.save`, `revision.update` and `revision.delete` (each with a
change) the Typst folder `<data>/works/<slug>/` of the affected work is mirrored as a commit (every work
has its own mirror). `commit` is then the commit id. If the mirror fails, the change holds nevertheless; the
answer carries `mirror` with the error text, the next commit catches up the state. The DB write and the
commit run under a common lock: even simultaneous changes each get their own
commit that changes exactly their file. `revision.reject` and `post.add` do not commit.

Commit: subject `„<Nummer> <Titel>: Vorschlag <rev> übernommen“`, `„…: Revision <rev> gespeichert“`, `„…: auf
Fassung <rev> umgeschaltet“`, `„…: Fassung <rev> geändert“` or `„…: Fassung <rev> gelöscht“`. If the
switched or accepted revision brings another level along, it stays **one** commit: subject with the addition
`„ · Ebene a → b“`, body with the items re-levelled along and their new revisions („Mitgestuft …: 002 Tiefe:
Fassung 3“) and the renumbering, trailer `Baruch-Action: switched` (also when accepting).
For proposals (and when switching to a revision of a persona) the author is the persona
(`Name (@handle) <handle@persona.baruch.invalid>`; characters outside
`[A-Za-z0-9._-]` in the handle become `-` in the address, `@mcp:claude-code` → `mcp-claude-code@…`), committer the
human (`Autor <autor@baruch.invalid>`), trailers `Persona`, `Model`. Changing and deleting: author and committer
the human, body with a line diff and the note before/after or the full deleted text with author,
status, parent, note and „Umgehängt: …“; the commit is made even without a change to the files (empty
log commit). Always: `Baruch-Node`, `Baruch-Revision`; on switching, changing and deleting
`Baruch-Action: switched|updated|deleted`.

A part with a part mark (Round 14) has, like every section with a current revision, its own file
`abschnitte/NNN-<titel>.typ` (with a work template after the `#import` line); `main.typ` includes them in
tree order.

### Literature and highlights
Methods from `crates/baruch-server/src/literature.rs`; all take `work?`. Highlights over RPC always come
from the human (status `active`).

| Method | Parameters | Answer |
|---|---|---|
| `literature.render` | `{work?, node?, key?, rev?, view?, lq?, lmode?}` (the key goes before the position; `lmode` = `fts` \| `rag`, unknown = `fts`, Round 12) | `{html, lq, lmode, url}` – content of the literature popup (search form, hits, titles with quotations and highlights); `url` = `/literatur?…` |
| `highlight.list` | `{title?}` | `{highlights: [Highlight]}` – all or those of one title; since Round 13 every highlight carries `check` (check against the full text, see below) or `null` |
| `highlight.add` | `{title, quote?, locator?, start?, end?, text, kind, note?}` | `{highlight, check}` – `kind` from `quote`, `core`, `contradiction`, `check`; `start`/`end` both or neither, from 0, in Unicode characters; with `quote` the range ends within the quotation and `text` is the excerpt `start..end`, where white space does not count (runs of Unicode white space such as line break, tab, non-breaking space count as one space, edges are dropped); the exact excerpt of the quotation is then stored; text different in content → -32602; without `locator` the locator of the quotation. **Round 13:** without `quote`, with `start`/`end` Baruch checks the range against the full text of the source (only for a title linked with MultiGPT): `check.result` = `found` („im Volltext gefunden“, the text stands exactly in `start..end`, white space does not count), `not_found` („im Volltext nicht gefunden“) or `unverifiable` („nicht prüfbar“: no link, MultiGPT unreachable – `message` names the reason). The highlight is stored in every case; without a range or with a quotation `check` is `null` |
| `highlight.check` | `{highlight}` | `{highlight, check: {result, label, message, checked}}` – checks again against the full text (Round 13); only highlights without a quotation with a range, otherwise -32602 |
| `highlight.update` | `{highlight, kind?, note?, status?}` | `{highlight}` – `status` from `active`, `proposed`, `rejected`; highlights of personas do not become `active` (-32000) |
| `highlight.delete` | `{highlight}` | `{deleted}` |
| `highlight.cite` | `{highlight, key}` | `{node, key, rev}` – proposal revision: current text + `#quote(block: true, attribution: [@key[Stelle]])[Text]` (escaped), author `@literatur`, model `baruch-zitat`, note „Zitat aus @key“; the current revision stays. `key` is the fixed key of the section (required, resolved under the write lock); `node` → -32602 |
| `highlight.own` | `{highlight}` | Round 15: `{highlight, original}` – takes over a highlight of a persona as **one's own**: a new highlight of the human (`active`) with text, range, quotation, locator and kind of the original, note „übernommen von @x (Hervorhebung n)“; the original stays a proposal. Own highlight or already taken over → -32000. Not over MCP |
| `literature.to_persona` | `{work?, key, persona?, text, source?, title?, highlight?, question?}` | Round 15 („an Persona geben“): like `persona.ask` – the question (default „Was sagt dieser Text zu meinem Abschnitt?“) with the selected text and the source becomes a post of the human, the answer of the persona (default the lector) a post with the source `source` (plus „Hervorhebung n“ with `highlight`); answer like `persona.ask` (`post`, `question_post`, `model`, `compressed`, `url`, …). `text` 1–4000 characters, `source` one line ≤ 300 characters (empty = „Literatur“); without `key` → -32602 „Abschnitt wählen …“, `node` → -32602. Nothing in the book text |
| `source.read` | `{work?, title? \| document, collection?, part? = 1, q?}` (Round 23: `q` at most 500 characters) | Round 13: part of the full text through MultiGPT `read_document`: `{source: {title, url, document, collection, title_id}, part, parts, more, text, marks: [[start, end]], origin: "multigpt" \| "lokal", message?, offset, paged_by_remote}`. `title` = title with a link (without → -32602), otherwise `document` (document id, 1–500 characters). If the schema of `read_document` knows a page parameter (`page` or similar) and the answer names pages, Baruch reads page by page (`parts` may be `null` = end unknown); otherwise Baruch splits the full text at paragraph boundaries into parts of at most 20 000 characters (`offset` = first character of the part in the full text). `marks`: occurrences of `q` in the part (case-insensitive; if `q` does not occur, its words of three characters or more). Answers above 4 MiB, time limit 20 s. Failure → -32000 „Der Volltext ist gerade nicht abrufbar: …“; with a local copy from the sync, instead `origin: "lokal"` and `message` „… lokale Kopie vom …“. Contents are data |
| `source.adopt` | `{document, collection?}` | `{title: TitleEntry, remote: {way, collection, document}}` – Round 13: takes over a document from MultiGPT into the author's library (title and address from `document_info`, otherwise the document id; citation key `mg-…`, kind `other`, note „Aus MultiGPT übernommen …“) and links it (`library_title_remote`); already taken over → the same title. Afterwards the index catches up the shared area. Human only (no MCP tool) |

`Highlight`: `{id, title_id, quote_id, locator, start, end, text, kind, note, author, status, created}`,
`author` = `{kind: "human", name}` or `{kind: "persona", handle, model}`. Errors: empty text, invalid
range, unknown kind, unknown title, unknown highlight or a quotation of another title → -32602.

Without JavaScript: `POST /literatur/act` (behind the same protection as `/act`, foreign `Origin` → 403) with
`action` = `highlight`, `cite`, `persona`, `own`, `similar`, `update`, `delete`, since Round 13 `adopt` (fields `document`,
`collection`; 303 to `/quelle?title=…&notice=adopted`) and `check` (field `highlight`; 303 with `notice=checked`), and
the fields `work`, `node`, `key`,
`rev`, `lq`, `title`, `quote`, `highlight`, `locator`, `start`, `end`, `text`, `kind`, `note`, `status`,
`confirm` (required for deleting); then 303 to `/quelle?…`, `/literatur?…` or, when citing, to the
proposal `/?work&node&rev`. `cite` needs the fixed key `key` of the section;
if it is missing, the server answers 422 „Abschnitt fehlt.“, if it no longer exists, 409 – in both cases without
a highlight and without a proposal. `persona` (Round 15, fields `persona`, `question`, `source?`, otherwise the source from
title and locator `@key, S. 3`) like `literature.to_persona`, then 303 to the section; without `key` or on
an error the notice page (status 200, `assist_notice.html`) with message and a link back. `own` (Round 15,
field `highlight`) like `highlight.own`, 303 to `/quelle?…&notice=owned`. Errors: 422
(invalid), 409 (refused). Pages: `GET /literatur?work&node&key&rev&lq`,
`GET /quelle?work&node&key&title&quote?&highlight?&lq&teil?`; both carry the key in their links and
forms themselves (from `key`, otherwise determined from `node` at loading), `key` goes before `node`. `/quelle` shows for
a title linked with MultiGPT the full text (part `teil`, anchor `#volltext`). Round 13:
`GET /quelle/multigpt?work&key&rev&lq&lmode&collection&document&section&teil&q` – full text of a document from
MultiGPT, part by part with links „vorheriger/nächster Teil“, occurrence `q` as `<mark>`, button „In die Bibliothek
übernehmen“; without JavaScript. From a foreign page (`Sec-Fetch-Site`/`Origin`) neither of the two
pages loads the full text. Hits of MultiGPT in the semantic search (`literature::rag_view`) carry `fulltext_link`
(`/quelle/multigpt?collection=…&document=…[&section=…]&q=…`).

**Bibles in the literature (Round 15):** no RPC methods of their own; `literature.render` and `/literatur` carry the
box „Bibeln“ (Bible modules from `bible.modules`, entries without `kind` count as a Bible, the Bible of the work marked
and first) and in the wording mode with a search the hit groups „Bibel“ (per module, at most 10 hits, at most
3 modules, time limit 3 s, through `bibles::search`) and „Kommentar“ (`bibles::commentary_hits_html`). The
module folders are read only if they are configured or the work has a Bible. Pages without JavaScript:
`GET /literatur/bibel?work&key&rev&lq&lmode&module&ref&book&chapter&q&kom` (books, paging through chapters, opening the reference
`ref`, wording `q`, next to the reference the commentary panel `bibles::commentary_panel_html`, `kom=1`
unfolds it; without a Bible a notice with a link to `/einstellungen/quellen`) and `POST /literatur/bibel/act` (behind
`same_origin`) with `action=cite` (Bible quotation as a proposal revision: current text + `#quote(block: true,
attribution: [Ps 139,23–24 (Modul)])[…]`, author `@literatur`, model `baruch-zitat`, note „Bibelzitat …“; 303 to the
proposal) or `action=persona` (verses to `persona` with `question`, source „Bibel MODUL, Stelle“, 303 to the
section) and the fields `work`, `key` (required, otherwise notice page „Abschnitt wählen“), `module`, `ref`. The
Bible text always comes from the module, never from the form. There is no `action=highlight` (notice page: highlighting
only for titles of the library).

### Typeset view

Read only, only through `POST /rpc` (not `/act`, not MCP), without the write lock; answered by
`src/wysiwyg.rs`. Blocks, instructions and HTML: `docs/WYSIWYG.md`.

| Method | Parameters | Answer |
|---|---|---|
| `wysiwyg.parse` | `{work?, body}` | `{blocks, html}` – blocks `{kind, …, raw}` (concatenation of the `raw` = `body`, byte-identical) and HTML of the typeset view (`div.ws-block[data-b][data-kind]`, islands `contenteditable="false"`); heading with `label` (label on the line after it), scene/work with `args` (arguments literal) and `body`; Round 14: part `{kind: "part", title}` (`div.ws-block.ws-part[data-kind=part][data-level=0]`) and „ohne Nummer“ `{kind: "unnumbered", level, title}` (`div.ws-block.ws-unnumbered[data-kind=unnumbered][data-level]`), title as plain text; islands with an image path appear as an image: `div.ws-block.ws-figure[data-kind=island][data-image=1]` with `<img class="ws-image" src="/werke/{work}/bilder/pfad?p=…" alt="…" loading="eager">` (alternative text of the current revision from "Images", otherwise empty) and `span.ws-caption`, inline `span.ws-island.ws-inline-image`; raw text only in `data-raw`; paths in normal form (`./` and since Round 18b also a leading `/` are dropped: `p=bilder%2Fa.png`); Round 18b: all images of an island (`image_paths`), new kinds `enum` (`{start, items, depths?}`, `<ol start>`) and `terms` (`{items: [{term, desc}]}`, `<dl>`), lists with `depths` (nested), inlines `linebreak`, `link` (`{url, c?}`, `a.ws-link`) and `ref` (`{target, supplement?}`, `button.ws-ref`, not editable); shapes in `docs/WYSIWYG.md` |
| `wysiwyg.serialize` | `{work?, body, edits}` | `{body}` – new text from the original `body` and the instructions `edits` (`{keep: i}` literal, `{replaces: i\|null, block}` newly generated, `{replaces: i, body: [instructions]}` only in the body of a scene/work, Round 18b `{replaces: i, raw: "<Typst>"}` source of block `i` literal – island in place, empty = deleted); unknown or duplicate index, new island (also `raw` without `replaces`) or an island edited as a block, body instruction for a block without a body, new or converted part (Round 18b: „Einen Teil legt man in der Gliederung an, nicht im Satzbild.“), wrong shape → -32602 with a German message. A heading ends at the first line break (`{"t": "soft"}` or a line end in the text); the rest becomes its own paragraph after it. A changed part is rebuilt with `part_line`, „ohne Nummer“ with `unnumbered_line` (if it replaces such a mark, its comment `// …` stays) |
| `wysiwyg.style` | `{work?}` | `{css, style}` – appearance from the work template: `style` = `{font, size, fill, emph_fill, strong_fill, heading_font, heading_fill, indent, leading, justify, width, scene, work}` (missing = `null`), `css` = variables under `.ws-sheet`; only rules of the top level (not in show rules, `#show: …`, function bodies), several → the last |
| `wysiwyg.diff` | `{work?, key\|node, rev, base?}` | `{html, base}` – revision `rev` against `base` in the typeset view (`div.ws-same`, `div.ws-del`, `div.ws-ins`); if `base` is missing, its parent (`based_on`) applies, otherwise the current revision; unknown revision → -32602; images in islands as with `wysiwyg.parse` (since Round 18b all images of an island, address without a leading `/`). The editor no longer uses the method since Round 7 (the typeset-view diff of the proposals comes finished in the HTML); it stays for other callers |

### Images

All methods take `work?`. Writing ones run under the write lock and carry `commit` or
`mirror`; they do not exist over `/act` nor over MCP.

| Method | Parameters | Result |
|---|---|---|
| `asset.list` | – | `{assets: [AssetSummary]}` (`name, mime, encoding, bytes, current, revisions, caption, alt, paths, updated`) |
| `asset.get` | `name, rev?, content? = true` | `{asset, revision: AssetRevisionInfo, encoding, content?}` |
| `asset.revisions` | `name` | `{revisions: [AssetRevisionInfo]}` (`number, based_on, current, mime, encoding, bytes, sha256, caption, alt, author, note, created, edited`) |
| `asset.add` | `file_name, content, encoding: "base64"\|"svg", name?, path?, caption?, alt?, note?` | `{name, rev}` |
| `asset.replace` | `name, file_name, content, encoding, note?, rename?: bool = false` | `{name, rev, renamed_from?}` (new revision, current at once). Round 21c: a format other than the ending of the name only with `rename: true` (otherwise -32602 with the hint „Endung anpassen“); then the image is named with the default ending of the new format (`haus.svg` → `haus.png`, taken → `haus-2.png`), `name` is the new name, `renamed_from` the old one; all paths stay with the image, open requests take the name along; one commit |
| `asset.set_current` | `name, rev` | `{name, rev}` |
| `asset.update` | `name, rev, caption?, alt?, note?, paths?` | `{name, rev, changed}` (fields and paths together one commit) |
| `asset.delete` | `name, rev?, confirm: true` | `{name, rev?, deleted}` (without `rev` the whole image) |
| `asset.path_conflicts` | – | `{conflicts: [{normal, paths: [{path, asset}]}]}` (Round 21c) – legacy: paths that are equal in normal form (`/x.png`, `x.png`, `./x.png`) but belong to different images; by normal form, spellings by path |
| `asset.path_resolve` | `normal, asset` | `{name, rev, commit?}` (Round 21c) – human only: all spellings of `normal` then belong to the image `asset`, the other images lose only this path; one commit „Bild <asset>: …“ with „Pfad-Kollision … aufgelöst“. No collision → -32000; `asset` has none of the spellings → -32602 |

| `asset.search` | `query` | `{assets: [AssetSummary]}` (Round 11) – name, caption, alternative text of the current revision, case-insensitive, `%`/`_` literal, empty = all; `query` missing → -32602 |
| `asset.request_list` | `open?: bool = true` | `{work, requests: [AssetRequest]}` (Round 11) – requests of the assistants |
| `asset.request_decide` | `id, decision: "confirm"\|"reject"` | `{request, commit?, mirror?, cover?}` (Round 11) – human only |

Errors: without `confirm: true` → -32602; unknown image, unknown revision, too large (8 MiB), not a
supported image, broken Base64, invalid name or path → -32602; duplicate name, replaced
unchanged, already current, deleting the current revision → -32000.

**Requests for images (Round 11, migration 0018):** `AssetRequest` = `{id, work, op: "replace"|"set_cover"|
"delete", name, …, origin {kind, handle}, origin_label, note, created, decided, decision: "confirmed"|"rejected"|
"stale", decision_label, decided_by, message, summary, url, content_url?}`; with `replace` `mime, encoding, bytes,
caption, alt, expected_sha256` (without content; that stands under `content_url` =
`/werke/{id}/bilder/antraege/{nr}/datei`), with `delete` `rev, expected_sha256`, with `set_cover`
`expected_cover`. `asset.request_decide` executes on confirmation (replacing/deleting with commit „… (Antrag n)“,
trailers `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Confirmed-By`; cover through `work.set_cover` plus
log commit „Bildantrag n bestätigt“, `cover` = its answer). If `expected_sha256` or
`expected_cover` no longer matches → `decision: "stale"` with `message`, no error. Already decided → -32000.
Form `POST /werke/bilder/antraege/act` (`work, id, decision=confirm|reject`) → 303
`/werke/{w}/bilder?hinweis=antrag_bestaetigt|antrag_abgelehnt|antrag_veraltet#antraege`.

**Round 21c:** `AssetRequest` additionally has `rename` (request to replace with a change of type; `summary` then ends in „– mit
neuer Endung“, confirming renames like `asset.replace` with `rename`) and `purged`. Before every `asset.request_list`,
`asset.request_decide`, the section „Anträge“ of the page „Bilder“ and the request file, Baruch removes the content
of decided replace requests whose decision lies more than 30 days back (`baruch_store::ASSET_REQUEST_PURGE_DAYS`)
(`purged: true`, `bytes` stays the size before, decision and origin stay).
`GET /werke/{id}/bilder/antraege/{nr}/datei` delivers the content with MIME, `X-Content-Type-Options: nosniff` and
`Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox`; after the clean-up 410 Gone with
a message (text). Forms: replacing (`POST /werke/bilder/hochladen`) with field `rename=1` („Endung anpassen“; success
with a new name → 303 `/werke/{w}/bilder/<neu>?hinweis=endung_angepasst`); `POST /werke/bilder/act` with
`action=resolve_path`, `normal`, `name` (the image that keeps the path) → 303 `/werke/{w}/bilder?hinweis=pfad_zugeordnet`,
errors → page „Bilder“ with a message (422/409).

Pages (Round 11): `GET /werke/{id}/bilder?q=<Suche>`; `GET /werke/{id}/bilder/{name}/vorschau?b=<px>[&fassung=n]`
(raster scaled down, longest side ≤ b, JPEG stays JPEG, otherwise PNG; `b` 64–1024, default 320, otherwise 400;
`ETag`, `If-None-Match` → 304, `nosniff`, CSP `sandbox`; SVG unchanged like `/datei`); `POST
/werke/bilder/einfuegen` (`work, key, rev` = base, `name, caption?, place` (since Round 18c also `start`, `block:<n>`,
`offset:<n>` and `body`, see "work.sheet_set") → new revision of the human with
`#figure(image("<Pfad>"), caption: [...])`, missing path `bilder/<name>`, one commit; 303 `/?work=W&key=K`,
422 input, 409 refused). `wysiwyg.parse`/`wysiwyg.diff` show images without an entry as „Bild fehlt: <Pfad>“
(`ws-missing`).

`/werke/{id}/bilder/pfad?p=` finds an image under every spelling of its path (`/bilder/a.png`,
`bilder/a.png`, `./bilder/a.png`), never with `..`.

### Personas

Personas apply to all works; the management methods take no `work`. Only over `/rpc` (not `/act`,
not MCP); the tasks (`ask`/`review`/`propose`/`chat`) also go through `POST /assistenten/act` and
`/assistenten/stream` (see "Tasks for personas").

| Method | Parameters | Result |
|---|---|---|
| `persona.list` | – | `{personas: [{handle, name, role, model, context_length, active, lector, color, chosen, prompt_custom, tints: {light, dark, line, line_dark}, connection: {id, name, kind}, locality: "lokal"\|"extern", external_allowed, context: {tokens, reported, warning}}], palette: ["#rrggbb", …], lector, lector_mode, chat_scope}`; complete even without any work. Round 15b: `connection` = model connection of the persona (default Eliezer), `context` without a network query from the caches (without a report `tokens` = setting). Round 16a: additionally `chat_scope` (`"chapter"`\|`"section"`, see `persona.chat_scope_get`); with „automatisch nach Kontext“ `lector` compares the effective context length (`ModelPort::context`), all other fields unchanged. Since Round 17a also `lector_context` (effective context length of the lector `{tokens, reported, warning}` or `null`); lector and `context` without a network query (cache, otherwise the setting) – only a model call asks the remote side |
| `persona.chat_scope_get` | `{}` | `{scope: "chapter"\|"section"}` (Round 16a) – what the section chat passes along: `chapter` (default) the whole chapter or the whole sermon, `section` only the section, plus title and outline of the work. Stored in `store_meta` (`chat_scope`), not in the mirror |
| `persona.chat_scope_set` | `{scope}` | ditto; another value → -32602. Human only: not over `POST /act`, not over MCP. Form `POST /einstellungen/personas/act` with `method=persona.chat_scope_set`, `scope=chapter\|section`, `work?` → 303 `/einstellungen/assistenten?ok=chat[&work=W]#chat` |
| `persona.backend_get` | `{persona}` | `{persona, connection: {id, name, kind, kind_title, external}, locality, external_allowed, explicit, usable, warning}` (Round 15b) – `explicit: false` = no row in `persona_backend` (default: Eliezer, not allowed); `usable: false` = external connection without permission (then `warning` as a sentence) |
| `persona.backend_set` | `{persona, connection: id, external_allowed?: bool = false}` | like `persona.backend_get`; unknown persona or connection → -32602. Human only. Form `POST /einstellungen/personas/act` with `action=backend`, `handle`, `connection`, `external_allowed=1` (missing = off) → 303 `…?persona=<h>&ok=verbindung#persona-<h>` |
| `persona.bible_get` | `{persona}` | `{persona, bible_context, allowed, default}` (Round 15) – Bible evidence from SWORD in the context (`crate::bible_research`); `allowed` = @belege or the lector according to the choice; without a row `default` applies (@belege on, lector off) |
| `persona.bible_set` | `{persona, on: bool}` | `{persona, bible_context}`; only for @belege and the lector, otherwise -32602 „Bibel-Belege aus SWORD gibt es nur für @belege und den Lektor.“ Table `persona_bible` (migration 0037). Form `action=bible`, `handle`, `on=1` → 303 `…&ok=bibel` |
| `persona.set_color` | `handle` (with or without `@`), `color`: `"#rrggbb"`/`"#rgb"` or `null` (default) | `{handle, color, chosen}` |
| `persona.update` | `handle`, `name?` (Round 11: trimmed 1–80 characters, one line), `role?`, `model?`, `context_length?`, `active?` | `PersonaRow {handle, name, role, model, context_length, active, color, chosen, prompt}`; role/model trimmed 1–200 characters, context 1–10 000 000, `active` a boolean, unknown persona → -32602 |
| `persona.prompt_get` | `{handle}` | `{handle, prompt, default, custom}` – effective role prompt, default from the code, own yes/no |
| `persona.set_prompt` | `{handle, prompt: Text \| null}` | ditto; `null` = default; trimmed, empty or above 8000 characters → -32000 (migration 0015, column `persona.prompt`) |
| `persona.lector_get` | `{}` | `{mode: "fixed"\|"auto", handle, lector}` |
| `persona.set_lector` | `{mode, handle?}` | ditto; unknown persona, wrong mode or `fixed` without `handle` → -32602; stored in `store_meta` (`lector`), without a row fixed `@lektor` applies |
| `persona.rag_get` | `{persona}` | `{persona, rag_context, allowed}` (Round 12) – `allowed` = @belege or the lector according to the stored choice (fixed or automatic); unknown persona → -32602 |
| `persona.rag_set` | `{persona, on: bool}` | `{persona, rag_context}`; only for @belege and the lector, otherwise -32602 „Bedeutungs-Treffer gibt es nur für @belege und den Lektor.“; `on` not a boolean → -32602. Table `persona_rag` (migration 0032), no commit in the mirror. Form `POST /einstellungen/personas/act` with `action=rag`, `handle`, `on=1` (missing = off), `work?` → 303 `/einstellungen/assistenten?persona=<h>&ok=rag[&work=W]#persona-<h>`, refused → 422 |

- `color` is the effective colour (`chosen: false` = default from `palette` in the order of the personas).
- `tints.light`/`tints.dark`: backgrounds on which text, muted text and link text of the light
  (`#1d2125`, `#5f6b76`, `#3b5bdb`) or dark (`#e3e6ea`, `#9aa5b1`, `#7d97ff`) theme reach at least
  4.5 : 1; `tints.line` = line in the light theme with at least 3 : 1 against `#ffffff`,
  `tints.line_dark` = line in the dark theme with at least 3 : 1 against `#1f2328` (WCAG 1.4.11; the colour
  itself, darkened or lightened if necessary).
- `role`, `model`, `context_length`, `active`, `lector` come since Round 9 from the table `persona` (no
  longer from the default work), complete even without works; `lector` follows the choice (`LectorChoice`): fixed one
  persona (default `@lektor`, trial since 2026-11-06); if it is inactive, or with „automatisch“, the active
  persona with the largest context (on a tie the earlier one; since Round 16a the effective length, since Round 17a on
  pages and in lists without a network query: what the cache knows, otherwise the setting – only a model call
  asks the remote side). `work.get` carries `work.lector_choice`
  (`{mode, handle}`). Since Round 10 the model goes to Eliezer; the context length only serves for shortening in Baruch.

#### Tasks for personas (Round 10)

Addressing always by `key` (`node` → -32602), `work` missing → default work; text (question, hint, instruction,
message) at most 4000 characters; inactive or unknown persona → -32602; error of the model → -32000 with an
honest message, nothing is stored then. The model is called **without** the write lock; writing
happens afterwards under `write_lock` (post through `Store::add_post`, proposal through `Store::add_proposal`).

| Method | Parameters | Result |
|---|---|---|
| `persona.ask` | `{work?, key, persona, question, rev?, scope?}` | `{post: {id, author, body}, question_post: {id, author, body}, model, truncated, cut, compressed, url, key, work, persona}`; the question first becomes a post of the human |
| `persona.review` | `{work?, key, persona?, rev?, question?, scope?}` | like `ask` without a question post (`question_post: null`); `persona` missing → the lector |
| `persona.propose` | `{work?, key, persona?, instruction?, rev?, scope?}` | `{rev, status: "proposed", author, model, truncated, compressed, proposals, url, key, work, basis}` (`basis: {rev, label, proposal}` since Round 36: the revision on which the proposal is based, header line „Grundlage: …“); heading line of the shown revision enforced; equal to the current one → -32000 |
| `persona.chat` | `{work?, key, message, rev?}` | `{posts: [{id, author, body}], errors: [{persona, message}], url, truncated, cut, compressed}` (Round 23: plus `done_proposals?`, `done_notes?`, `all_done?`; quotation lines „> …“ call nobody): post of the human, then for each mentioned active persona (`@handle`, at most three, without duplicates, one after another) its answer; `compressed` = the first compression of an answer or `null` (Round 15) |

`model` is the model that actually answered; `truncated: true` if the context was shortened (the post
then ends with „(Kontext gekürzt)“). `cut: true` if the model stopped at the length limit (`finish_reason`
„length“): the post then ends with „… (Antwort abgeschnitten: Längengrenze erreicht)“; a **proposal** cut off
this way is not created (-32000, nothing stored). `max_tokens`: ask 700, review 900, chat 500, propose
min(4096, section tokens·1.5 + 200); `temperature` 0.4.

**Scope, whole chapter and compression (Round 15, „lektor“):**
- Context of every task: the whole chapter or, for a sermon, the whole sermon in its wording ("Chapter surroundings",
  the section of the task stands under it in the shown revision), the findings of the other personas on parts
  of the chapter (the last three per part) and – as before – memory, chat history and fact blocks.
- `scope` (optional, with `persona.ask|review|propose`): `"section"` (default; missing or empty) checks the section,
  `"chapter"` the whole chapter (`max_tokens` ask 900, review 1500). With a proposal with `"chapter"` the
  persona writes a new revision for each part with text, one after another; every changed part gets exactly one `proposed`
  (note „Vorschlag von @x (ganzes Kapitel)“), unchanged ones and ones cut off at the length limit are dropped.
  `proposals: [{key, rev, label}]` names all of them, `rev`/`url` point to the first; without a single one → -32000.
  Other values → -32602. `persona.chat` knows no `scope` (always the section).
- Budget: effective context length according to the model port (setting of the persona, aligned with the remote side) −
  `max_tokens` − 256, characters ≈ tokens·3. If not everything fits, additions are shortened first (fact blocks, findings,
  memory, chat; only down to a third of the budget, `truncated: true`, post with „(Kontext gekürzt)“).
  Chapter and object of review are never shortened: if they do not fit, the object of review is split into overlapping pieces
  (each reviewed in its wording), the model first summarises the remaining parts one after another (label of the
  call „… Zusammenfassung“), what does not fit even so stands only with its title. The findings of the pieces are merged
  without duplicates.
- `compressed`: `null` or `{message, summarized, chunks}` – `message` begins with „Komprimiert:“, `summarized`
  names the parts the persona saw only as a summary („1.1 Ausflug“, „Zeichen 1–4000 von Abschnitt …“;
  entirely omitted ones with „(ausgelassen)“), `chunks` the number of review pieces (1 = not split). The post then ends
  with the paragraph `message`; with a proposal it stands in the note (never in the book text). A proposal needs the
  section entirely in its wording; if it does not fit → -32000 („zu lang …; Nichts angelegt“).
- „Kritisch:“: if it is missing with `@lektor`, `@exeget` or `@nathan` (question, review, chat), it is requested again once per task
  (stream: event `retry {persona, reason: "fehlendes „Kritisch:“"}`); if it is still missing, the
  post ends with `„(Hinweis von Baruch: Diese Antwort nennt keinen mit „Kritisch:“ markierten Befund, auch nicht nach
  einer Nachforderung.)“`. In a proposal, lines beginning with „Kritisch:“ are dropped; forbidden openings
  are not checked with a proposal (the first sentence is book text).
- Evidence from SWORD: if the persona gets evidence (decision in `bible_research`, setting per persona),
  the block "Evidence from SWORD" with `[S1]…[Sn]` (kind, module, reference or keyword, licence note) stands before the chapter;
  every `[Sn]` of the answer without an entry gets „(unbekannte Quelle)“.

Forms and stream (behind `guard::same_origin`):
- `POST /assistenten/act` (form, without JavaScript): `method` ∈ `persona.ask|review|propose|chat`, `work`,
  `key`, `persona`, `question`/`message`/`instruction`, `rev`, `view`, `lq`, since Round 15 `scope`
  (`section`|`chapter`, field from `fragments/assist_scope.html`, goes unchanged to the RPC) → after the finished
  answer 303 to the key address (`/?work=W&key=K[&rev=R]…`, with a proposal with its `rev`). **Round 15:** errors
  show, instead of 422, the notice page `assist_notice.html` (status 200, `base.html`, message and a link back to the
  section); if a persona fails in the chat, the page names one line per persona („Dein Beitrag und die fertigen
  Antworten sind gespeichert …“). If the result carries `compressed`, the form does not redirect silently but
  shows the same page with the toast component (`fragments/toast.html`) and the link „Weiter zum Abschnitt“.
- `POST /assistenten/stream` (`Content-Type: application/json`, body `{method, …}` as RPC) →
  `text/event-stream` (`Cache-Control: no-store`) with `start {persona, style}`, `delta {text}`, since Round 15
  `posted {post: {id, author, body}}` (chat: the sender's post is stored, comes before the first `delta`),
  `retry {persona, reason}` (the answer is requested again, e.g. „verbotene Einleitung“; discard what was shown of this persona
  – since Round 18b, with split tasks only that of the running piece), `compressed {message,
  summarized, chunks}` (context split), since Round 18b `piece {persona, index, count}` (the next piece of a
  split task begins), then
  `done {kind: "post"|"proposal", post?|rev?, posts?, url, truncated, compressed: null|{…}}` or `error {message}`.
  If the client disconnects, the model call is aborted: with a single task nothing is stored, in the chat the
  sender's post and answers already finished stay.
- `POST /einstellungen/personas/act` new: `method=persona.set_prompt` (`handle`, `prompt`, `reset=1` = default;
  → `?ok=prompt|vorgabe#persona-<handle>`) and `method=persona.set_lector` (`mode`, `handle`; → `?ok=lektor#lektor`).

#### persona.chat: RFC, @alle, as a task (Round 28, chat-auftrag)

`persona.chat {work?, key, message, rev?, as_order?}` (`as_order`: `true` or from forms `"1"`/`"on"`, default
`false`) → the previous fields and `{rfc, proposals: [{persona, rev, url}], findings: [id], notes: [Text]}`
(`src/assist/chat.rs`):

- **RFC:** „RFC“ as the first word (case does not matter, also „RFC:“) or the mention „@alle“ → `rfc: true`;
  every active persona answers exactly once (the limit of three answers does not apply, the queue of the models
  does), the lector by setting last. It gets the answers of the others framed as data (`<<<antwort …
  >>>`, each with their findings `[B<id>]`) and the task of weighting them into „Jetzt wichtig:“ and „Kann warten:“,
  without taking over others' findings unchecked; its findings become collective findings (`summary_of`). Inactive personas
  do not answer.
- **Writing task:** if the message begins, after mentions and „bitte“, with a writing verb in the imperative
  („Schreibe“, „Formuliere“, „Erstelle“, „Überarbeite“, „Kürze“, „Ergänze“ …) or `as_order` is set,
  every mentioned persona (with `as_order` without a mention the lector) writes a **proposal** for the section (`proposals`,
  never accepted), and a short post of the persona stands in the chat („Fassung n dieses Abschnitts liegt als Vorschlag
  bereit …“; Baruch writes the text, no second model call). Deleting and new sections stay with the human
  (new sections only through the outline or MCP `outline_add`). If message or proposal name findings with
  „[B<id>]“, Baruch makes a status proposal „übernommen in Fassung n“ for each (`findings::rpc::propose`,
  origin the persona); if that fails, the reason stands in `notes`.
- Otherwise as before: one answer per mentioned active persona (at most three).
- Form `POST /assistenten/act` with the checkbox `as_order`; after a chat message the redirect goes to
  `…#chat-neueste` (newest post).

#### Model backend (Round 10)

Human only: over `/rpc`, not over `POST /act` and not over MCP (`crate::models`, before the choice of work).

| Method | Parameters | Result |
|---|---|---|
| `model.list` | `{persona? \| connection?}` | `{models: [Name…], live, fetched: ISO, message, contexts: [{id, context_length}], aliases: {alt: neu}, personas: [{handle, model, active, state: "bekannt"\|"alias"\|"unbekannt", target?}], connection}`; Round 15b: list of the connection of the persona or of the connection `connection` (id), without either Eliezer; `personas` only the personas of this connection; cached for 60 s per address (`live: false`). If the remote side does not answer and there is an (expired) list: that one with `live: false` and `message` („… antwortet nicht (…); Liste aus dem abgelaufenen Zwischenspeicher (Stand …)“); otherwise → -32000 with a German sentence |
| `model.status` | `{}` | `{running: label \| null, waiting: number}` – queues per endpoint (one request at a time per address, at most eight waiting), summarised; label e.g. `"@lektor ask key 17"`, never text |
| `model.probe` | `{model, persona? \| connection?}` | `{ok: true, model, requested, reply (at most 200 characters), ms, finish_reason, completion_tokens, at, connection}` or `{ok: false, model, requested, message, ms, at, connection}`; fixed task "Answer only with: OK", `max_tokens` 16, through the queue of the connection (without one, Eliezer); no book text, therefore also over an external connection without permission. The result stands in the database (`model_connection.probe`, survives a restart); missing, multi-line or longer than 200 characters `model` → -32602 |
| `model.context` | `{persona}` | `ContextBudget {tokens, reported, warning}` + `{persona, setting}` (Round 15b): `tokens` = min(setting, reported length), without a report the setting; Ollama reports the effective length (`num_ctx` of the Modelfile, otherwise that of the loaded model from `/api/ps`, otherwise 4096 – never the maximum of the model); `warning` e.g. „Einstellung 65536, Eliezer meldet 32768; wirksam sind 32768 Tokens.“; waits at most as long as the model list (10 s), result cached for 60 s; never fails (only an unknown persona → -32602) |
| `model.connections` | `{}` | `{connections: [{id, name, kind, kind_title, url, external, locality, locality_warning?, account, secret: "hinterlegt"\|"fehlt"\|null, secret_for?, secret_withheld?, probe, check, position, eliezer, personas}], kinds: [{kind, title}]}` – never a key |
| `model.connection_set` | `{id?, name, kind: "eliezer"\|"openai"\|"ollama", url, external?: bool}` | like an entry of `model.connections`; `external` missing → on creation by the address (not recognisably local → `true`, see `models::looks_local`), on change `true` stays, and a new not recognisably local `url` sets `true`; `locality_warning` (sentence) if a connection kept as local points to a not recognisably local address; without `id` new (account `baruch/model-<id>`, ids never reused), with `id` change (missing fields stay). Name 1–80 characters, unique; address like `connection.set` (http/https, without user and key). For Eliezer (id 1) `url` goes to the connection `eliezer` (with its binding), the kind stays `eliezer`. If the address points to another target (origin), a stored key stays bound to the old one and is not sent. Human only |
| `model.connection_delete` | `{id, confirm: true}` (Round 23: without `confirm: true` -32602) | `{id, deleted: true, personas: [Handle]}`; its key is removed, its personas use Eliezer again (not allowed); Eliezer itself → -32000. Human only |
| `model.connection_set_secret` | `{id, secret}` | `{id, secret: "hinterlegt"}`; key only in the keyring, applies afterwards to the current address (Eliezer: account `eliezer`). Empty, above 4096 characters or multi-line → -32602. Human only |
| `model.connection_delete_secret` | `{id, confirm: true}` (Round 23: without `confirm: true` -32602) | `{id, secret: "fehlt"}`. Human only |
| `model.connection_check` | `{id}` | `{ok, message, details: {models, contexts}, at, connection}`, stored in `model_connection.checked`; a failure is `ok: false`; key for another target → `ok: false` „… gilt für ein anderes Ziel; nicht gesendet“, the remote side is not asked. Eliezer: like `connection.check {kind: eliezer}` |

Old Ollama names are resolved before sending (`llama3.2:3b` → `llama-3.2-3b-instruct-q4-k-m`, `qwen3:4b` →
`qwen3-4b-q4-k-m`; only for connections of the kind Eliezer). Baruch never requests a context length and never calls
`/api/v1/models/load|unload|estimate`.

**Connection per persona (Round 15b):** a task of a persona (`persona.*`, `/assistenten/*`, MCP `ask_persona`/
`chat`) goes over its connection. If the connection is external and the persona is not allowed, Baruch refuses with
-32000 `„Die Verbindung „…“ ist extern; für @… ist sie nicht freigegeben (Einstellungen → Assistenten). Es wurde
nichts gesendet.“` – checked in `EliezerPort::chat`, so for every model call of the personas. Forms without
JavaScript: `POST /einstellungen/modelle/act` with `action=save` (create without `id`: `name`, `kind`, `url`,
`external=1|0|auto` – `auto`, the default on creation, by the address; the checkbox on change: unticked
local), `secret` (`id`, `secret`), `delete_secret` and `delete` (`id`, `confirm=1`), `check` and `probe`
(`id`, `model`) → 303 `/einstellungen/verbindungen?ok=modell_…#modell-<id>` or `?modellfehler=<code>#modelle`.

#### Bible references (Round 10)

| Method | Parameters | Result |
|---|---|---|
| `scripture.check` | `{text, work?, module?}` | `{refs: [{input, span: [a, b], normalized, book, book_id, status, message, wording, wording_code, quote, bible_text, partial, diff, diff_line}], truncated, source, bible}`; `status` ∈ `valid\|unknown_book\|no_such_chapter\|no_such_verse\|malformed`, `message` German with the maximum; if the whole single-line text is a reference, it is read strictly, otherwise searched for in running text; at most 200 000 characters (otherwise -32602), at most 200 references (`truncated`); without `text` → -32602. **Wording:** with the Bible of the work (`work`, missing → default work) or the module `module`, per reference `wording_code` ∈ `match` („Wortlaut stimmt“, `partial` for a partial quotation), `differs` („Wortlaut weicht ab“, `diff` = `[{op: same\|bible\|quote, text}]`, `diff_line` with `[−…]`/`[+…]`), `no_quote` („kein Zitat gefunden“), `missing_verse`, `versification_mismatch`, `invalid_ref`, `module_error`; `bible_text` = text of the Bible (at most 4000 characters), `quote` = quotation at the reference. Without a Bible `wording` = `"ungeprüft"`, `wording_code` = `unchecked`, `bible` = `null`. Read only |

**Converting versifications (Round 17a).** Read only, without a work, also over `POST /act`:

| Method | Parameters | Result |
|---|---|---|
| `scripture.map` | `{ref, from? = "KJV", to? = "German"}` | `{from, to, refs: [{input, normalized_from, status, target, verses: [{book, chapter, verse}], rules, message}], versifications: [Namen]}`; `ref` 1–500 characters, read with `baruch_scripture::parse` (several with „;“, book names German or OSIS) and checked **against `from`** (not against the German versification): „Joel 3,21“ is valid in KJV, not in German. `status` ∈ `same` (same verses), `moved` (other verses by a rule, `rules` names it), `unmapped` (no rule covers the difference, `message` begins with „Versifikation passt nicht: “ – never guessed), `invalid` (book unknown or the reference does not exist in `from`; `message` names the reason). `target` = verses in `to` in German notation („Joel 3,1–5“, runs with „; “), otherwise `null`; `verses` individually (book as OSIS abbreviation), at most 1000 per reference (then „– gekürzt“ in `message`). A reference „ganzes Kapitel“ leaves out psalm titles that the target versification does not count as a verse (rule in `rules`). Versifications: German, Calvin, Catholic, Catholic2, DarbyFr, KJV, KJVA, Leningrad, Luther, LXX, MT, NRSV, NRSVA, Orthodox, Segond, Synodal, SynodalProt, Vulg (case does not matter); unknown versification → -32602 with this list; `ref` missing or too long → -32602. Rule table `crates/baruch-scripture/data/versification-map.tsv` (origin in its head), see `docs/Architektur.md`, "Round 17a" |
| `scripture.versifications` | `{}` | `{versifications: [{name, books, chapters, verses}]}` – German first, then alphabetical; only the 66 books of the reference check (MT and Leningrad: 39, without the New Testament) |

#### Bibles (SWORD, 2026-11-06)

Source `src/bibles.rs`, reader crate `baruch-sword`. `work?` as everywhere (missing → default work); `module?` is a
module name from `bible.modules` (missing → the Bible of the work). Every answer names in `bible` module, versification
and licence note: `{module, description, versification, license (DistributionLicense; Copyright or „keine
Lizenzangabe in der conf“), distribution_license, copyright, lang, version, driver, dir, supported, problem, cipher,
notice}`. Since Round 17c: `cipher` ∈ `none` (not encrypted) | `locked` (encrypted, key missing) |
`unlocked` (key in the keyring or already in the conf), `notice` = `license` (licence note under the
name that commentaries and lexicons carry as well). The key itself never stands in an answer.

**Versification of the module (R17c):** references are always counted in German (Schlachter 1951). If Baruch reads a module in
another versification (KJV, Luther, Vulgata, Synodal …, all versifications of `sword-modules`),
`bibles::read_passage` maps the reference through `baruch_scripture::versification::mapping::map_range` onto the versification of the
module and reads there (at most 400 verses; `truncated` = the reference has more). What cannot be mapped
stays `versification_mismatch` with a reason („Versifikation passt nicht: …“ with both versifications); never guessed.
`scripture.check`, `bible.check` and MCP `scripture_check` carry per reference **additively** `quote_span: [s, e] | null`
(byte range of the quotation content without quotation marks in the checked text) and `module_ref: string | null` (reference read
in the versification of the module, e.g. „Mal 4,1–6 (KJV)“, only if it differs from the German one); all
previous fields stay. Quotations come only from `crate::quotes::quote_spans` (quotation marks, nested,
`#quote[…]`, block quotations).

| Method | Parameters | Result |
|---|---|---|
| `bible.modules` | `{work?}` | `{dirs, default_dirs, modules: [bible …], problems, work, work_bible, work_sources: {commentaries, lexicons}}`; `default_dirs`: no own list stored (then `$SWORD_PATH`, `~/.sword`, `/usr/share/sword`); `problems`: missing folders, unreadable confs, duplicate names. Since Round 15 per module `kind`: `bible` (zText/zText4/RawText/RawText4), `commentary` (RawCom/RawCom4/zCom/zCom4) or `lexicon` (RawLD/RawLD4/zLD); if `kind` is missing, `bible` applies. Unreadable ones (HREFCom, RawFiles, unknown compression, unknown versification `„Verszählung „X“ kennt Baruch nicht“`) stand with `supported: false` and a reason. Since Round 17c per module `cipher` and `key_stored` (key stored in the keyring yes/no); encrypted modules are `supported: true` and without a key `cipher: locked`; compression ZIP, LZSS, BZIP2 and XZ |
| `bible.set_dirs` | `{dirs: [Pfad] \| null}` | like `bible.modules`; each entry an absolute path or `~/…`, trimmed, empty ones dropped, duplicates once, at most 20 of 1000 characters each, otherwise -32602; `null` = back to the defaults. Human only (not over MCP) |
| `bible.set_work` | `{work?, module: Name \| null}` | like `bible.modules`; unknown or unreadable module → -32602; `null`/empty = no Bible. Human only (not over MCP) |
| `bible.lookup` | `{ref, work?, module?}` | `{bible, refs: [{input, normalized, book, book_id, status, message, verses: [{chapter, verse, text \| null}], missing, truncated, module_ref, rules}]}`; `ref` read strictly like `scripture.check` (at most 500 characters, 20 references), since R17c read through `read_passage`: `verses` in the versification of the module (at most 400; `truncated` from `PassageText`, exactly 400 verses = not shortened), `module_ref` as above, `rules` = applied rules of the mapping, not mappable → `status: versification_mismatch` with a reason; `text: null` = verse missing in the module; plain text without footnotes, cross-references, headings (canonical ones stay). Without a Bible → -32000 „Keine Bibel gewählt …“; encrypted without a key → -32000 with „… ist verschlüsselt; der Schlüssel fehlt …“ |
| `bible.search` | `{query, work?, module?, limit? = 50}` | `{bible, query, hits: [{ref, book_id, chapter, verse, text}], truncated, searched_verses, ms}`; linear search for the word sequence (normalised like the wording comparison), `limit` 1–200, time limit 5 s (`truncated`); `query` 1–200 characters, otherwise -32602 |
| `bible.check` | `{work?, key}` | like `scripture.check` for the current revision of the section `key`, plus `work`, `key`, `title`. Read only |
| `bible.marks` | `{work?, key, rev?}` | check display (R17c, read only, no MCP tool – MCP takes `scripture_check`): `{work, key, rev, title, bible, notice, marks: [{ref, ref_span: [s, e], quote_span: [s, e] \| null, code, state, label, quote, bible_text, module_ref, diff_line}], counts: {stimmt, weicht_ab, andere}}`. The shown revision `rev` is checked (number of the revision, default the current one); spans are byte ranges in **its** text. `state` ∈ `stimmt` (`match`), `weicht_ab` (`differs`), `ohne_zitat` (`no_quote`), `ungeprueft` (`unchecked`), `fehler` (other codes); `notice` = licence note of the conf. Unknown revision → -32602 |
| `bible.cipher_key_set` | `{module, key}` | `{module, cipher: "unlocked"}` (R17c). Key of an **encrypted** module (Bible, commentary or lexicon) from the publisher: 1–255 characters in one line (otherwise -32602), unknown or unencrypted module → -32602. Baruch first reads the first existing entry as a probe (decrypt, unpack, valid UTF-8); if it does not fit: -32000 „Schlüssel passt nicht zum Modul; nichts gespeichert.“ Stored only in the keyring (service `baruch`, account `sword-cipher:<modulname klein>`), never in the database, conf, answers, messages or logs. Human only (not `/act`, not MCP) |
| `bible.cipher_key_delete` | `{module, confirm: true}` | `{module, cipher}` (`locked`, or `unlocked` if the conf itself carries a key); without `confirm: true` → -32602. Human only (not `/act`, not MCP) |
| `bible.work_sources` | `{work}` | `{work, bible, commentaries: [Name], lexicons: [Name]}` (Round 15) – Bible, commentaries and lexicons of the work |
| `bible.set_work_sources` | `{work, commentaries: [Name], lexicons: [Name]}` | like `bible.work_sources`; only found, readable modules of the right kind (spelling as in the conf), duplicates and empty ones dropped, at most 20 per kind; otherwise -32602. Table `work_sword_source` (migration 0037). Human only (not `/act`, not MCP). Form `POST /einstellungen/werk/bibeln` with `method=bible.set_work_sources`, `work`, repeated fields `commentary` and `lexicon` → 303 `/einstellungen/werk?work=W&ok=kommentare#kommentare` or `&kommentarfehler=1` |
| `commentary.lookup` | `{ref, work?, module?}` (Round 23: reference in German versification, mapped into the versification of the commentary) | `{ref, work, commentaries: [{module, description, versification, notice, entries: [{ref, text, truncated}], problem?}]}` (Round 15) – for each chosen commentary (or `module`) the entries for the reference in the **versification of the commentary**; an entry for several verses appears once with a range („Ps 139,23–24“). Plain text without markup, at most 4000 characters per entry (`truncated`); `problem`: module missing/unreadable, reference invalid, no entry. `ref` 1–500 characters |
| `commentary.search` | `{query, work?, module?, limit? = 50}` | `{query, work, commentaries: [{module, …, hits: [{ref, text, truncated}], truncated, problem?}]}`; linear search (like `bible.search`), time limit 5 s in total |
| `lexicon.lookup` | `{key, work?, module?}` | `{module, key, found, text, notice, truncated}` – first chosen (or named) readable lexicon; case according to the conf, `@LINK` followed, Strong numbers like `G25`; `found: false` = no such keyword. Without a lexicon → -32000 |
| `lexicon.keys` | `{work?, module?, prefix?, limit? = 100}` | `{module, prefix, keys, truncated, total, notice}` – keywords as in the module, sorted, with prefix, at most 500 |
| `lexicon.search` | `{query, work?, module?, limit? = 50}` | `{module, query, hits: [{key, excerpt, in_key}], truncated, notice}` – keyword hits first, then hits in the article text; time limit 5 s |

Forms without JavaScript on Settings → Sources: `POST /einstellungen/quellen/act` with `method=bible.set_dirs`
(`dirs` one folder per line, `reset=1` = default; codes `ok=bibel_ordner|bibel_vorgabe`, `fehler=bibel_ordner`)
or `method=bible.set_work` (`work`, `module`, empty = none; `ok=bibel_gewaehlt|bibel_keine`, `fehler=bibel_modul`),
since R17c `method=bible.cipher_key_set` (`module`, `key` as a password field; `ok=bibel_schluessel`,
`fehler=bibel_schluessel_falsch|bibel_schluessel|schluesselbund`) and `method=bible.cipher_key_delete` (`module`,
required checkbox `confirm=1`; `ok=bibel_schluessel_entfernt`, `fehler=bestaetigen`), 303 back to
`/einstellungen/quellen?…#bibeln`. Page without JavaScript `GET /werke/{id}/pruefung?key=K[&rev=N]`: the same data
as `bible.marks` as a list (`src/scripture_check.rs`).

#### MCP tokens (Round 10)

Human only: over `/rpc` and the form of the page „MCP-Zugang“, not over `POST /act` and not over MCP.

| Method | Parameters | Result |
|---|---|---|
| `mcp_token.list` | `{}` | `{tokens: [McpToken]}`; `McpToken = {id, name, prefix, created, last_used, revoked}` (never hash or plain text) |
| `mcp_token.create` | `{name}` | `{token: McpToken, secret}` – Round 23: also -32000 if the handle (`@mcp:<handle>`) is taken, is that of a revoked token or is reserved with `lokal-` (see "Round 23 – Hardening"); `secret` is the plain text (`baruch_…`), **only in this answer**; name empty, above 64 characters, with control characters or duplicate among valid ones → -32000 |
| `mcp_token.revoke` | `{id, confirm: true}` | `{token}`; without `confirm: true` → -32602; unknown → -32000; idempotent |

Form `POST /einstellungen/mcp/act`: creating (`name`) answers 200 directly with the page and the plain text
exactly once (`Cache-Control: no-store`, never in an address, redirect, cookie or message); revoking (`id`,
`confirm=1`, the server checks the checkbox) → 303 `?ok=widerrufen`.
- Errors: unknown persona (`„Persona „@x“ gibt es nicht.“`), invalid colour („Farbe als #rrggbb
  angeben.“), missing `handle` or `color` → -32602. No commit in the mirror.

### Web search for @belege (Round 13)

Only over `POST /rpc` (human), not over `POST /act`, **no MCP tool** (reason in Architektur.md).
Before the choice of work in `rpc::call` (`crate::websearch`).

| Method | Parameters | Result |
|---|---|---|
| `websearch.get` | `{}` | `{on, configured, preferred}` – `on` = switch (`store_meta` `websearch_belege` = `'1'`, missing = off), `configured` = an address is entered for MultiGPT, `preferred` = number of preferred sources |
| `websearch.set` | `{on: bool}` | `{on}`; not a boolean → -32602 |
| `websearch.evidence` | `{post}` | `{post, evidence: [{id, label, url, title, preferred, accessed, quote, verdict, library_title, post, work, key, persona, created}]}` – evidence of a post in the order of creation; `label` „W1“ …, `accessed` UTC time, `verdict` ∈ `belegt` \| `nicht_belegt` \| `nicht_zitiert`, `library_title` = adopted title or `null`; without evidence `[]` |
| `websearch.adopt` | `{evidence}` | `{title: TitleEntry, quote: Quote, highlight: Highlight, evidence}` – only with `verdict = 'belegt'`; otherwise, with unknown or already adopted evidence -32602 (nothing created). Creates a title `kind: "web"` with `url`, `publisher` = host and note „Abgerufen am TT.MM.JJJJ (Websuche @belege)“ (if there is already a title `web` with the same address, that one is taken), the quotation as `library_quote` and a highlight `kind: quote` by the human; sets `library_title_id`, then `rag::notify(state, None)` |

`settings.get {page: "assistenten"}` additionally contains `websearch: {on}`.

Course in `persona.ask`/`persona.review`/`persona.chat` (only `@belege`, only with the switch on, not with
`persona.propose`): first `web_search` with „site:<Rechner> <Anfrage>“ for the first three preferred sources, then
a general call; at most 8 hits, 4 fetches through `fetch_url`, 10 s each, 30 s in total, page ≤ 1 MiB. The
code checks the answer (`post.body`): „(nicht belegt)“, „(unbekannte Quelle)“, „(nicht abgerufen)“, „Angabe
prüfen: nicht belegt“, line „Quellen: [Wn] Titel – URL (abgerufen am TT.MM.JJJJ)“, on failure „Hinweis: Websuche
ausgefallen (<Grund>)“. With evidence the post carries the source „Web: n Beleg(e)“ (`post.sources`).

Form `POST /websuche/act` (behind `guard::same_origin`, foreign `Origin` → 403), always 303:
- `action=switch` (`on=1`, missing = off; `work`) → `/einstellungen/assistenten?persona=belege&ok=websuche[&work=W]#persona-belege`.
- `action=adopt` (`evidence`, optionally `work`, `key`, `post`) → `/websuche/belege?work=W&key=K&post=P&ok=uebernommen#beleg-<id>`
  or `&fehler=nicht_belegt|schon|unbekannt|intern`; unknown `action` → `&fehler=aktion`.

Page: `GET /websuche/belege?work&key&post` – evidence of a post with the button „In die Bibliothek“ (only with
`belegt`), without JavaScript.

### Menu

- `menu.badges {work?}` → `{work, outline_requests, open_proposals}`: numbers of the badges of the header –
  open (undecided) outline requests and open proposals across all sections of the work;
  the work page draws the same numbers on the server. If `work` is missing, the default work. Parameters that are not
  an object, `work` not a number or an unknown work → `-32602`. Read only; answered before the choice
  of the default work in `rpc::call` (`crate::menu`).
- Round 28 (befund-uebersicht): `menu.badges` additionally delivers `open_findings` – open findings in the whole work
  by the counting rule (contract R28 §1.5: `status = 'open'`, not attached, no praise, with a section; sum of
  `Store::finding_open_counts`). Badge „k offen“ on the tab **„Prüfen“** and on the entry „Befunde“.

### Settings

Without a required work, before the choice of work in `rpc::call`; only over `/rpc` (not `/act`, not MCP).

| Method | Parameters | Result |
|---|---|---|
| `settings.pages` | – | `{pages: [{slug, title, storage, url}]}` (seven, fixed order) |
| `settings.render` | `page`, `work?`, `query?: {…}` (text, number, boolean) | `{page, title, storage, html}` – `html` like `GET /einstellungen/<page>?fragment=1`; without `Host` „MCP-Zugang“ shows only the path `/mcp` |
| `settings.get` | `page`, `work?` | `darstellung`: `{theme: system\|light\|dark, editor, pv, accordion, widths: {left, right}}` from the cookies of the request; `werk`: `{work, title, subtitle, author, kind, audience, bible, template_rev, numbering: {scheme, custom}, timing: {words_per_minute, target_seconds}}` (`timing` since Round 16b, `target_seconds` `null` = no target time); `assistenten`: `{personas: [PersonaRow], lector, lector_mode, lector_handle}`; `mcp`: `{path: "/mcp", loopback_without_token: true, clients: [SeenClient], tokens: [McpToken]}` (`loopback_without_token` is fixed `true` and describes the direct access from the own machine; under a host name allowed with `--allow-host`, `/mcp` needs a token nevertheless, `docs/MCP.md`, "Access"); `daten`: `{dir, size_bytes, mirrors: [{work, slug, dir, head}], version, instance: {uuid, name}}` (`instance` since Round 26); `verbindungen`/`quellen` → -32602 („über connection.list bzw. sources.list“) |
| `settings.set` | `page`, `work?`, `values: {…}` | `werk`: like `work.update` (`{work, changed, …}`); `assistenten`: `values {handle, role?, model?, context_length?, active?, color?}` → `{persona: PersonaRow, lector}` (check everything first, then write); `darstellung` → -32602 („Cookies setzt das Formular POST /einstellungen/darstellung/act“); otherwise -32602 |
| `settings.backup` | `confirm: true` | `{dir, size_bytes, skipped}`; without `confirm` -32602, target exists, already 10 backups `<Name>.sicherung-*` next to the data folder (Baruch deletes none itself) or too little space (after the backup 512 MiB must stay free, Unix `statvfs`) and others → -32603 with a German message |

`persona.update` see "Personas".

Forms (behind `guard::same_origin`, foreign `Origin` → 403):
- `POST /einstellungen/darstellung/act`: `theme=system|light|dark`, `editor=satz|quelltext`, `pv=open|closed`,
  `accordion=personas|revisions|chat`, `reset_widths=1` (each optional), `work` → Set-Cookie
  `baruch_<n>=<v>; Path=/; Max-Age=31536000; SameSite=Lax` (deleting with `Max-Age=0`; `system` deletes
  `baruch_theme`, `open` deletes `baruch_pv`, `reset_widths` deletes `baruch_left`/`baruch_right`), 303
  `/einstellungen/darstellung?ok=1[&work=W]`; invalid → 422 with the page, no cookie.
- `POST /einstellungen/personas/act`: `method=persona.set_color` (as before) or `method=persona.update`
  (`handle, role, model, context_length, active=1|leer`) → 303
  `/einstellungen/assistenten?ok=farbe|persona[&work=W]#persona-<handle ohne @>`; error → 422 with the page.
  Round 11 (one tab per persona, `GET /einstellungen/assistenten?persona=<handle ohne @>[&fragment=1]`):
  `action=save` (`handle, name, color, color_text, role, model, context_length, active, prompt`; all in one
  transaction, an unchanged default prompt stays the default) or `action=reset_prompt` (`handle`, `confirm=1`;
  without `confirm` → confirmation question) or `action=reset_color` (`handle`; colour back to the palette)
  → 303 `/einstellungen/assistenten?persona=<h>&ok=persona|vorgabe|farbe` or
  `&rueckfrage=vorgabe` (`[&work=W]#persona-<h>`); refused → 422 with the tab and the input.
- `POST /einstellungen/werk/zeit` (Round 16b, block „Predigtzeit“): `method=work.timing_set`, `work`,
  `words_per_minute` (60–250), `target_minutes` (1–240, empty = no target time) → 303
  `/einstellungen/werk?work=W&ok=zeit#zeit`; invalid → 303 `…&zeitfehler=1#zeit`, nothing changed.
- `POST /einstellungen/daten/sicherung`: `confirm=1` required (otherwise 422) → 303
  `/einstellungen/daten?ok=sicherung[&work=W]` (since Round 23 without `ziel`: a fixed message code; the page reads the folder name of the
  newest backup from the file system).
- `POST /werke/act` (update) and `POST /werke/nummerierung/act`: new field `back` (see structure). With a
  valid `back` a refused form (422 or 409) answers with the whole settings page „Werk“
  (`settings.html` with exactly one `[data-settings-frame]`), message and input in the respective form; so
  the popup takes over the frame, without JavaScript the page stands there.

Pages: `GET /einstellungen` → 303 `/einstellungen/darstellung[?work=W]`; `GET /einstellungen/<seite>[?work=W]
[&fragment=1]` (unknown → 404); `GET /einstellungen/personas` → 303 `/einstellungen/assistenten[?work=W]`.

### Connections and sources

Human only: over `POST /rpc` and the own forms, not over `POST /act` and not over MCP. Errors:
-32602 invalid (message without the input secret), -32603 internal or „Schlüsselbund: …“.

| Method | Parameters | Answer |
|---|---|---|
| `connection.list` | – | `{connections: [{kind, title, config, updated, secret, secret_error?, secret_for?, check}]}`; `secret` = `"hinterlegt"` \| `"fehlt"` \| `null` (LanceDB or keyring not readable, then `secret_error`); `secret_for` = target for which the stored secret applies, only if the settings have since pointed to another one (see "Binding"); `check` = `{at, ok, message, details}` \| `null` |
| `connection.get` | `{kind}` | one connection as above |
| `connection.set` | `{kind, config}` | one connection; given fields replace the stored ones. If the target changes, a stored secret stays bound to the old one (`secret_for`). -32602 for an unknown field, a secret field (`password`, `token`, `key`, `secret` …), an address with user info, scheme ≠ http/https, a key in the query, port ∉ 1–65535, unknown TLS mode, host with spaces |
| `connection.set_secret` | `{kind, secret}` | `{kind, secret: "hinterlegt"}`, applies afterwards to the current target; empty, multi-line, above 4096 characters or `lancedb` → -32602 |
| `connection.delete_secret` | `{kind, confirm: true}` | `{kind, secret: "fehlt"}`; without `confirm: true` → -32602 |
| `connection.check` | `{kind}` (`eliezer` \| `multigpt` \| `pgvector` \| `lancedb`) | `{kind, ok, message, details, at}`, stored in `connection`; a failure is `ok: false`, not an RPC error; if the secret is bound to another target: `ok: false`, „Nicht geprüft: …“, `details: {secret_withheld: true, secret_target}`, the remote side is not asked; with `eliezer` additionally `details.personas` (like `model.list`) and the comparison in `message`; the whole check at most 10 s. `details` (Round 12): `multigpt` `{server: {name, version}, tools, pages, collections: [{name, count}]}` (`tools/list` over `nextCursor`, at most 10 pages; `list_collections` if offered); `pgvector` `{server_version, vector, table, columns: {rolle: spalte}, collections: [{name, count}], chunks, dimension, models}` (`ok` = connected ∧ `vector` ∧ table/columns fit ∧ dimension known; without `vector` `{server_version, vector: null}`); `lancedb` `{dir, exists, writable, tables, entries, model, dimension, configured_model, embedder: "ok" \| reason, probe}` (`ok` = folder usable ∧ probe embedding ∧ model/dimension fit the index) |
| `connection.rag_get` | `{work?}` | `{work, ways, default}` (without a row `["lancedb"]`, `default: true`) |
| `connection.rag_set` | `{work?, ways}` | `{work, ways, default: false}`; ways ⊆ `lancedb`, `pgvector`, `multigpt` (empty = full text only), fixed order |
| `connection.collections_get` | `{work?, way}` | `{work, way, collections: [..], available: [{name, count}], available_at}` (Round 12); `available` from `details.collections` of the last check (empty names dropped), `available_at` its time or `null`; `way` only `pgvector` \| `multigpt`, otherwise -32602 |
| `connection.collections_set` | `{work?, way, collections: [..]}` | `{work, way, collections}` – trimmed, without duplicates (the first wins), order as given; not text, empty, multi-line, above 200 characters, above 100 entries, `lancedb` or unknown work → -32602, nothing changed. Table `work_rag_collection` (migration 0031) |
| `sources.list` | – | `{sources: [{id, title, url, note, position, created, updated}]}` |
| `sources.create` | `{title, url, note?}` | the new source (at the end) |
| `sources.update` | `{id, title?, url?, note?}` | the changed source |
| `sources.move` | `{id, position}` | `{sources}` (position outside 1..n → to the edge) |
| `sources.delete` | `{id, confirm: true}` | `{deleted}` |

Since Round 13 the web search of `@belege` uses the list of sources (see "Web search for @belege"): for the first
three hosts one call `web_search` each with „site:<Rechner>“, marking „bevorzugte Quelle“ by the host
of the address. The page Sources shows this as `data-usage` (no longer a STUB).

**Binding of the secret to the target** (migration 0014, column `connection.secret_target`): a secret
applies only to the target for which it was stored – for Eliezer and MultiGPT the origin of the address
(scheme, host, port; another path on the same server is the same target), for pgvector host, port and
TLS mode. `connection.set` with another target remembers the previous target if a secret is stored; afterwards
`connection.check` does not send the secret to the new one (neither as bearer nor as password). The confirmation
is entering it anew: only `connection.set_secret` (or „Speichern“ with a secret entered) binds it to
the current target. Back at the old target it applies again; `connection.delete_secret` releases the binding. So
`connection.set` with a foreign address plus `connection.check` does not drain a stored key.

`connection.set` for `pgvector` additionally knows since Round 12 `table` (`tabelle` or `schema.tabelle`),
`text_column`, `vector_column`, `collection_column`, `document_column` (required), `section_column`, `title_column`,
`url_column`, `model_column` (empty = does not exist) and `embedding_model`; identifiers
`^[A-Za-z_][A-Za-z0-9_]{0,62}$`, otherwise -32602.

Since Round 19b the **task prefixes** of the embedding: `lancedb` `query_prefix` and `document_prefix`, `pgvector`
only `query_prefix` (its data is already embedded; `document_prefix` there → -32602 „unbekanntes Feld“). Checked
with `baruch_rag::check_prefix`: at most 64 characters, no control characters (no line break either), otherwise -32602
(form code `praefix`); **not trimmed** – the space at the end belongs to it (nomic-embed-text:
`"search_query: "` / `"search_document: "`). If the field is missing, „kein Präfix“ applies (behaviour as before, no
rebuild). The pgvector search embeds the query as `query_prefix + query`; LanceDB uses both (R19a).
`connection.check` for `pgvector` (Round 19b): all fields already shortened in SQL (collections of the list to 200
characters, counted per full name, at most 1000; models to 200); with a stored password and TLS `prefer`,
`disable` or `require` (encrypted, but without checking the certificate) `message` appends a note,
`details.tls_warning` carries it; only `verify-full` stays without a note. A schema from `information_schema`
is quoted with doubled quotation marks; configured names stay restricted to the identifier rule.

### Semantic search (RAG, Round 12)

Over `POST /rpc` (origin „Autor“), not over `/act`; over MCP only `rag_search`/`rag_status` (see MCP.md).
Read only: no way writes book text, ideas or highlights.

| Method | Parameters | Result |
|---|---|---|
| `rag.search` | `{work?, query, ways?, collections?, limit?}` | answer JSON `{work, query, hits: [{way, collection, document, section, title, snippet, score, weak, url, meta, rank, fused, found_by: [{way, rank, score}]}], ways: [{way, state, label, message, hits, millis}]}`. `query` trimmed 1–1000 characters, `limit` 1–50 (default 10), `ways` ⊆ {`lancedb`, `pgvector`, `multigpt`} (intersected with the ways of the work), `collections` a list (LanceDB: kinds `section`, `idea`, `highlight`, `library`, since Round 13 `remote`; remote: intersection with the chosen collections); otherwise and for an unknown work -32602 |
| `rag.status` | `{work?}` | `{work, ways, index: {dir, entries, model, dimension, configured_model, built, pending, stale, problem}, fts: true}` – without network; `stale` = stuck jobs (Round 19a) |
| `rag.reindex` | `{work, confirm: true}` | `{work, queued: true, pending, deferred}`; without `work` or `confirm: true` -32602, without an embedder -32000 (with reason). If the model or the document prefix does not fit (or `baruch-index.json` is damaged), the whole index is discarded and rebuilt, otherwise the rows of the work. Answers immediately: if the index worker currently holds the write lock, a reset wish is noted (`deferred: true`), the worker aborts the running job, resets afterwards and queues the interrupted job again (also that of another work or the clean-up; duplicates only once). Human only: no MCP tool, not over `/act` |

- **`ways` of the answer** always names all three ways in the order lancedb, pgvector, multigpt; `state` ∈
  `connected` | `failed` | `off`, `label` „verbunden“ | „ausgefallen“ | „aus“. Off: „Für dieses Werk nicht gewählt
  (Einstellungen → Verbindungen).“, „In dieser Anfrage nicht gefragt.“, „Keine Sammlung gewählt (Einstellungen →
  Verbindungen).“ or the reason on opening (no host, no address, secret for another target).
  Failed: error, time limit (20 s per way), model does not fit, no embedding model. One way never blocks
  the others.
- **Score** 0..=1 per way (LanceDB and pgvector 1 − cosine distance; MultiGPT its score, otherwise 1 − distance,
  otherwise since Round 21a a substitute from the rank: `WEAK_SCORE + (1 − WEAK_SCORE)/rank`, `meta.score = "aus Rang"`, never
  „schwach“), not comparable across ways. **`weak`** (Round 19a) = `baruch_rag::is_weak(score)`: score below
  0.45 (`WEAK_SCORE`) or not a number; display text „schwach“. The hits stay in their order, nothing
  is hidden. **Merging:** duplicates = same document and same
  section (trimmed, lower case), also across ways; `fused` = Σ 1/(60 + rank) (Reciprocal Rank Fusion), representative =
  hit with the best rank; deterministic.
- **LanceDB hits:** sections `collection: "section"`, `document: "work:W"`, `section: "key:K"`,
  `url: "/?work=W&key=K"`, `meta: {work, key, rev, chunk}`; ideas `idea:<id>`, `url: "/einfaelle/<id>"`,
  `meta: {idea, work, key, status}`; highlights `title:<T>`/`highlight:<H>`; library `title:<T>` or
  `quote:<Q>` with `url: "/quelle?…"`; fetched MultiGPT documents (Round 13, kind `remote`) see "Sync with
  MultiGPT".
- The query is embedded once per embedding model (LanceDB and pgvector share it as long as their
  query prefixes are equal).
- **Stuck jobs** (Round 19a): if embedding fails after three attempts, the job stays
  stuck (`index.stale`); `problem` stays set until a real change (saving, idea, sync) or
  `rag.reindex` brings the jobs back. `rag.search` triggers no new attempt (only silently the clean-up of
  deleted works and at most every 30 s the shared area, without stuck ones). The state of
  LanceDB in `ways` then names `„n Aufträge liegengeblieben – erneuter Versuch nach einer Änderung oder „Index
  neu aufbauen““`.
- **Task prefixes** (Round 19b, implemented in LanceDB with 19a): the connection LanceDB knows `query_prefix`
  and `document_prefix` (default empty = as before). Documents are embedded with the document prefix, the
  query with the query prefix; `baruch-index.json` records `document_prefix`. A different configured
  document prefix is like a different model: LanceDB „ausgefallen“ with „Aufgaben-Präfix geändert; Index neu
  aufbauen.“, until `rag.reindex` rebuilds the whole index.
- **Start without an embedder:** with its own data folder (`--data`) and without a stored connection Eliezer
  the service binds no embedder at start (no request to the default address); `problem` says why.
  `rag.reindex` binds Eliezer as soon as the connection is stored.
- Ideas of a deleted work (`work.delete`) can be found in the shared area immediately (Round 19a).
- Form `POST /einstellungen/verbindungen/act` with `method=rag.reindex`, `work`, `confirm=1` →
  `?ok=reindex` or `?fehler=bestaetigen|reindex`.

Forms (field `method`, always 303):
- `POST /einstellungen/verbindungen/act`: `connection.set` (fields `kind`, `url` or `host`, `port`,
  `database`, `user`, `tls` or `dir`, `embedding_model`, optionally `secret`), `connection.set_secret`,
  `connection.delete_secret` (`confirm=1`), `connection.check`, `connection.rag_set` (`work`, `way_lancedb=1`,
  `way_pgvector=1`, `way_multigpt=1`), `connection.collections_set` (`work`, `way`, repeated `collection`,
  `collection_new`; → `?ok=sammlungen` or `?fehler=sammlung|weg`), `rag.reindex` (`work`, `confirm=1`; →
  `?ok=reindex` or `?fehler=bestaetigen|reindex`). pgvector additionally `table`, `text_column`, … (see above). Success → `/einstellungen/verbindungen?ok=<code>[&work=W]#<kind|rag>`
  with `ok` ∈ `gespeichert`, `gespeichert_geheimnis`, `geheimnis`, `geheimnis_entfernt`, `geprueft`, `rag`.
  „Speichern“ with a secret first checks both (secret: not empty, one line, at most 4096 characters),
  then stores the settings and after that the secret; an invalid secret therefore changes nothing.
  If only the keyring refuses, the settings are already stored: code `gespeichert_ohne_geheimnis`.
  New (Round 10): `method=model.probe&kind=eliezer&model=…` (short test) → 303 `?ok=kurztest#eliezer` or
  `?fehler=kurztest_modell&kind=eliezer` (without a model, nothing sent); for other connections `?fehler=methode`.
- `POST /einstellungen/quellen/act`: `sources.create` (`title`, `url`, `note`), `sources.update` (`id`, …),
  `sources.move` (`id`, `direction=up|down` or `position`), `sources.delete` (`id`, `confirm=1`). Success →
  `/einstellungen/quellen?ok=<angelegt|geaendert|verschoben|geloescht>[&work=W]#quelle-<id>` (deleted:
  `#quellen`).
- Errors → `?fehler=<code>[&kind=<kind>][&work=W]`, codes: `kind`, `url`, `url_schema`, `url_benutzer`,
  `url_geheimnis`, `geheimnis_feld`, `host`, `port`, `tls`, `feld`, `ordner`, `modell`, `geheimnis_leer`,
  `geheimnis_ungueltig`, `kein_geheimnis`, `bestaetigen`, `schluesselbund`, `gespeichert_ohne_geheimnis`, `werk`, `weg`, `stub_lancedb`,
  `titel`, `adresse`, `notiz`, `quelle`, `position`, `methode`, `intern`; unknown codes show a
  general message, never the code itself.
- Pages: `GET /einstellungen/verbindungen` and `/einstellungen/quellen` are shown by the frame of the settings
  (task settings) with `crate::connections::render_connections` or `render_sources`.

### Sync with MultiGPT (Round 13)

Over `POST /rpc`, not over `/act` and **not over MCP** (no tool, in particular none that uploads):
only the human triggers the sync. Source `crates/baruch-server/src/sync/`, store `baruch-store/src/sync.rs`
(migration 0033). Only `sync.run` and `sync.collections` talk to MultiGPT; all other methods and the
page `/abgleich` only read or write SQLite.

| Method | Parameters | Result |
|---|---|---|
| `sync.status` | `{work?}` | `{last: {id, started, finished, ok, message, uploaded, replaced, removed, downloaded, unchanged, failed} \| null, running, works: [{work, title, upload, collection, confirmed, documents, synced}], collections: [{collection, documents, synced}], configured}` – `work` limits `works` to the work (unknown → -32602); `documents` = uploaded chapters or local copies, `synced` = last sync; `message`: first line the result, then one note per line; `configured` = address for MultiGPT entered |
| `sync.run` | `{}` | `{started: true, running: true, run}` – starts a run in the background (row in `sync_run`); if one is already running: `{started: false, running: true}` (no second run) |
| `sync.set_upload` | `{work, on, confirm?}` | `{work, upload, collection}` – switch „nach MultiGPT hochladen“ (default off). `on: true` without `confirm: true` → -32602 (confirmation before the first upload), nothing changed; on switching on, the collection „Baruch: <Werktitel>“ is recorded (one already recorded stays); `on: false` keeps it. Without `work`, unknown work or `on` not a boolean → -32602 |
| `sync.set_collections` | `{collections: [String]}` | `{collections}` – replaces the collections chosen for fetching (trimmed, duplicates removed, each 1–200 characters in one line, at most 100; otherwise -32602, nothing changed) |
| `sync.collections` | `{}` | `{collections: [{name, count, selected}]}` – online through `list_collections` (for the selection); MultiGPT not set up, not reachable or without the tool → -32000 with an honest sentence |

- **One run:** first Baruch → MultiGPT (only works with `upload`; one document per chapter, text = current
  revisions of chapter and subsections, title „<Nummer> <Titel>“; changed hash (SHA-256 over title
  and text) → `upload_document`, then `delete_document` of the old one; missing chapter or deleted work →
  `delete_document`), then MultiGPT → Baruch (chosen collections through `list_documents` and `read_document`,
  only what changed by `remote_stamp` or hash; limits 500 documents per collection, 2 MiB per document, 64 MiB per
  run, 30 s per call). Afterwards the local index catches up the copies (kind `remote`, see "Semantic search").
- **Numbers:** `uploaded` newly uploaded, `replaced` replaced (newly uploaded and the old one deleted), `removed`
  removed (documents of missing chapters or deleted works at MultiGPT, local copies of deleted or
  deselected documents), `downloaded` fetched, `unchanged` unchanged, `failed` failed.
- **Errors are notes:** not set up, not reachable, refused (HTTP 401/403 or `isError`,
  e.g. without `docs.write`), schema unknown, limits → `ok: false` and a sentence in `message`; locally
  everything stays unchanged. A permission, transport or schema error ends writing to MultiGPT for this
  run; an `isError` of the remote side for **one** document (e.g. an empty chapter; text without a hint at
  permissions) counts as `failed` and blocks only that one (Round 19a). No token in messages.
- **Hardening (Round 19a):** if `upload_document` names no document id, a note stands in `message`, and the
  state is stored with the hash and an empty id (block mark): the same content does not go out again.
  Before every upload the state carries the mark „Hochladen begonnen“; if the answer is lost (time limit,
  broken connection), the next run reports that the chapter may be present twice at MultiGPT. When
  fetching, a full page without `has_more`/continuation counts as possibly incomplete (the `limit` stands at the
  `maximum` of the schema or is silently capped): then no local copy is deleted (note). The collection
  of a work stays, when renaming, the one recorded at switching on.
- **LanceDB hits of the kind `remote`:** `collection: "remote"`, `document: "remote:<Sammlung>/<Dokument-ID>"`,
  `url: "/quelle/multigpt?collection=…&document=…"` (URL-encoded), `meta: {collection, document, sync}`
  (`sync` = id of the local copy).

Form `POST /abgleich/act` (behind `guard::same_origin`, always 303), field `action`:
- `run` → `/abgleich[?work=W]&ok=gestartet` or `ok=laeuft`;
- `upload` (`work`, `on=1` for switching on, missing = off, `confirm=1` required when switching on, optionally
  `back` = `/abgleich…` or `/einstellungen/werk…`) → `back` or `/abgleich?work=W` with `ok=hochladen_an|
  hochladen_aus`; without the checkbox `/abgleich?work=W&fehler=bestaetigen`, unknown work `fehler=werk`;
- `collections` (repeated `collection`, plus `collection_new`) → `ok=sammlungen` or `fehler=sammlung`;
- `load_collections` → `ok=geladen` or `fehler=laden` (reason on the page).
Page `GET /abgleich?work=W` (see Bedienung.md, sync with MultiGPT), without network.

### Ideas

Ideas (Round 7) also work without a work: `work` missing or `null` = without a work (for lists: all).
Over RPC the origin is always the human („Autor“). Every writing method runs under the write lock and
yields exactly one commit per affected mirror; the answer carries `commit` or `mirror` (error text).
Not under `/act`; the pages have their own form `POST /einfaelle/act`.

| Method | Parameters | Answer |
|---|---|---|
| `idea.create` | `{work?, key?, words: [..] \| "a, b; c", note?, quote?, source?, bible?}` | `{idea, commit?, mirror?}` |
| `idea.get` | `{id}` | `{idea}` (deleted ones too) |
| `idea.list` | `{work?: id \| "none" \| "all", word?, status?: "open" \| "done" \| "all", deleted?: bool, limit?, offset?}` | `{ideas, words: [{word, count}]}`; `deleted: true` = only deleted ones; `words` = cloud over work/status (without the word filter) |
| `idea.search` | `{query, work?, status?, limit?, mode?}` | `{ideas, hits: [{id, snippet}]}`; FTS5 through `baruch_library::fts_query`, `snippet` escaped HTML with `<mark>`; empty search → -32602. `mode` (Round 12) `fts` (default) \| `rag`, otherwise -32602; `rag`: semantic search (way LanceDB, kind `idea`), a work is needed (otherwise -32602 „Bedeutungssuche nur innerhalb eines Werks“) → `{ideas, hits: [{id, snippet, score}], ways}` in the order of rank, deleted ones and ones not matching `status` drop out; since Round 19c per hit `weak` (similarity below `baruch_rag::WEAK_SCORE`), weak hits last; if the semantic search fails (LanceDB not connected, e.g. without an embedding model): -32001 (`RAG_UNAVAILABLE`) „Bedeutungssuche nicht verfügbar: LanceDB (lokal): <Grund>.“ (until Round 19: -32000 or an empty list) |
| `idea.update` | `{id, words?, note?, quote?, source?, bible?, work?, key?, status?: "open" \| "done"}` | `{idea, changed, commit?, commits?, unlinked?}`; `work`/`key`: `null` = without; a new work without `key` → the reference is dropped; when moving, `commits` = both commits, `unlinked` = ids of the connections softly released in the process (connections only within one work; positions of the idea on maps are removed) |
| `idea.delete` | `{id, confirm: true}` | `{id, deleted: true, idea, commit?}`; without `confirm: true` → -32602; restorable |
| `idea.restore` | `{id}` | `{idea, commit?}` |
| `idea.process` | `{id, as: "outline" \| "post" \| "note" \| "persona", key?, place?: "before" \| "after" \| "into", title?, persona?}` | `{idea, node?, key?, outline?, post?, commit?}`; `outline`: `Store::idea_to_outline` (since Round 14 **one** transaction: create an item like `outline.add` after (`place`) the section `key` or the reference and mark the idea done with `done_ref` „key <key>“; if a step fails, nothing is changed and there is no commit), title = `title` or the keywords (200 characters), `outline` = outline answer with its own `commit`; `post`/`note`: post (author „Autor“ or „Notiz“, source „Einfall <id>“, since Round 15 for an idea of a persona or an MCP client „Einfall <id> von @x“ or „… von @mcp:…“); afterwards `status: done`. `persona` (Round 10, `persona?`, default the lector; since Round 15 also selectable in the form): the idea as a question about the referenced section to the persona (with origin „Einfall von @x“) → post of the persona, idea processed (`done_as: persona`, `done_ref` = post); answer `{idea, node, key, post_id, persona, model, truncated, url, commit?\|mirror?}`; model call before the write lock; if it fails → -32000, the idea stays open |

`idea` = `{id, work, key, node_title, words, note, quote, source, bible, origin: {kind: "human", name} |
{kind: "persona" | "mcp", handle}, status, done_as, done_ref, created, updated, deleted}`.

Errors: unknown idea („Einfall 7 gibt es nicht.“), a violated limit (1–20 keywords, each ≤ 60
characters, without a line break; thought/mark ≤ 4000, source/Bible reference ≤ 200), reference outside the
work („Der Abschnitt gehört nicht zu diesem Werk.“), a deleted idea when changing/processing, processing
without a work or section → -32602; unknown work → -32602.

Since Round 25, `einfaelle.md` in the mirror carries for every idea with connections, after the other fields, the line
`„- Verbindungen: → 79 führt weiter; ↔ 48 widerspricht („Notiz“); ← 12 Beispiel für“` (outgoing →, incoming ←,
undirected ↔; by connection id; only active ones to ideas that are not deleted). A deleted idea takes its
connections and position out of map, lists and mirror and brings them back with `idea.restore` (see "Idea map").

### Idea map (Round 25)

Since Round 34 the page is called „Mindmap“; tree, cloud, `idea.move` and `idea.tree` stand under "Round 34 – Mind map"
at the end. The methods of this section stay (flat view, connections, positions – positions have no effect in the
mind map).

Map of the ideas of a work, connections between ideas and stored positions (`crate::idea_map`,
store `idea_links.rs`, migration 0043). Over RPC the origin is always the human („Autor“); writing methods run
under the write lock. Not under `/act`; the map has its own form `POST /werke/{id}/karte/act` that
calls the same methods (numbers may therefore also come as digit text; empty texts count as missing).
`work` is required for `idea.map`, `idea.map_pos_set` and `idea.map_pos_reset` (no default work).

| Method | Parameters | Answer |
|---|---|---|
| `idea.map` | `{work, status?: "open" \| "done" \| "all", word?, origin?: "human" \| "persona" \| "mcp", q?, near?: bool = true, near_limit?: 1..5, near_threshold?: 0..1, open?: [key]}` | `IdeaMap` = `{work, title, nodes, ideas, links, near, positions, filter}` |
| `idea.links` | `{work}` or `{idea}`, each with `deleted?: bool = false` (exactly one of `work`/`idea`, otherwise -32602) | `{links: [IdeaLink]}` by id |
| `idea.link` | `{from, to, kind: "leads_to" \| "belongs_to" \| "example_of" \| "contradicts", note?}` | `{link, changed, commit?\|mirror?}` |
| `idea.unlink` | `{id}` (soft, no `confirm` needed: restorable) | `{link, commit?\|mirror?}` |
| `idea.link_restore` | `{id}` | `{link, commit?\|mirror?}` |
| `idea.map_pos_set` | `{work, idea, x, y}` | `{pos: {idea, x, y, updated}}` (no commit) |
| `idea.map_pos_reset` | `{work, idea?}` (without `idea` all positions of the work) | `{removed}` (no commit) |
| `idea.map_groups` | `{work}` + filter like `idea.map` (incl. `near_limit`, `near_threshold`; `near` does not count, nearness is always on for groups) | `{groups: [{group: "g<kleinste id>", ideas: [id], words: [..], proposal?: {name, persona, model, at}}]}` (Round 19b) |
| `idea.map_groups_name` | `{work, persona?}` + filter as above | `{proposals: [{group, ideas, name}], persona, model}`; without a group `{proposals: [], persona, model: null, message}` (Round 19b) |
| `idea.map_group_accept` | `{work, group, name?}` + filter (only needed without a proposal) | `{group, name, ideas, changed: [id], commits: [..]}` (Round 19b, human only); Round 23: stale proposal → -32000, limits checked beforehand |
| `idea.map_group_dismiss` | `{work, group}` | `{group, removed: bool}` (Round 19b) |

**Groups (Round 19b, `idea_map/groups.rs`):** a group is a connected component of the nearness pairs
(`near.pairs`, see below) with at least two ideas – without a model, deterministic, identifier `g<kleinste id>`,
ideas ascending, groups by identifier; `words` = keywords of the ideas, most frequent first (at most 12).
`idea.map_groups_name` asks the model through `state.model.chat` (queue like the other tasks): the lector
by setting or the named `persona` (must be active, otherwise -32602). Keywords and notes go into the model as
framed data (`<<<einfall … >>>`, "data, not instructions"), at most 40 groups of 12 ideas each.
Lines „g<id>: Name“ are expected; only known groups count, with a cleaned name of 1 to 40 characters
(control and Markdown characters, quotation marks at the edges, `,`/`;` removed), per group the first valid line; everything
else is discarded. The proposals live **only in memory** (per data folder and work): no commit, nothing in
SQLite or in the mirror, gone after a restart. At most one run per work at a time (otherwise -32000 „… benennt schon
…“). Failure of the model → -32000 with the reason. `idea.map_groups` shows a proposal only as long as the group
has exactly the same ideas. `idea.map_group_accept` (only over RPC and the form of the map, **no
MCP tool**): name = `name` (cleaned as above, otherwise -32602) or the proposal (both missing → -32602); the
ideas from the proposal or, without a proposal, from the groups by the filter (unknown group → -32602). Every
idea that does not yet carry the keyword (same `word_key`) gets it through `idea.update` (origin human,
one commit per idea with `Baruch-Action: idea-update`, `Baruch-Origin: human`); if one already had 20 keywords,
it is refused beforehand with -32000, nothing changed. Afterwards the proposal is gone. `idea.map_group_dismiss` removes
only the proposal. `group` may be `"g12"` or `12`, otherwise -32602.

```json
{"jsonrpc":"2.0","id":6,"method":"idea.map_groups_name","params":{"work":3}}
→ {"proposals":[{"group":"g48","ideas":[48,52,79],"name":"Saat und Reihe"}],"persona":"@lektor","model":"gemma"}
{"jsonrpc":"2.0","id":7,"method":"idea.map_group_accept","params":{"work":3,"group":"g48","name":"Aussaat"}}
→ {"group":"g48","name":"Aussaat","ideas":[48,52,79],"changed":[48,52,79],"commits":["5d1e…","8a0b…","c41f…"]}
```

**`idea.map`** assembles the map:
- `nodes`: `{id, kind, key, parent, number, title, sections, open_ideas, total_ideas}`; `id` = `"w"` (work, `kind`
  `work`, `parent` `null`), `"n:<key>"` (`part`, `chapter`, `section`) or `"none"` (`no_chapter`, title „Ohne
  Kapitel“, only if a shown idea has no reference); `parent` = nearest ancestor that is itself a node,
  otherwise `"w"`. Sections under a chapter stand folded in its `sections` (`{key, number, title,
  ideas}`); if the key of the chapter is in `open`, they are nodes of their own (`section`). Sections without a chapter
  above them are always nodes of their own. Counter = shown ideas hanging at the node or at a node below it
  (a part with its chapters, a chapter with its sections, folded as well as unfolded), for the work all
  shown ones.
- `ideas`: `{id, node: "i:<id>", anchor, key, label, words, note, status, origin, hit}` by id; `anchor` = id of the
  node at which the idea hangs (folded the chapter, otherwise `"none"`), `key` = exact reference, `label` =
  keywords „a, b“ (at most 60 characters). Only non-deleted ideas of the work; ideas without a work do not
  appear. `status`, `word` (compared by the `word_key`: case and white space do not matter) and `origin` remove
  non-matching ideas with their edges; `q` (full text like `idea.search`) marks hits with `hit: true`, the others
  stay with `hit: false` (without `q` all `true`).
- `links`: active connections between shown ideas; `positions`: stored positions of shown ideas
  (`{idea, x, y, updated}`, SVG user units, work at 0,0).
- `near`: `{state: "ok" | "off" | "no_embedder" | "unavailable", message, pairs: [{a, b, score, via: "vector" |
  "words" | "both", shared_words}]}` (`a < b`). Display only, never stored, **no embedding request**: vectors
  are those already stored in the LanceDB index (`idea:<id>`, `rag/neighbors.rs`); a cosine from `near_threshold`
  (default 0.75) counts as `vector`, an equal keyword as `words` (also without embedding), both as `both`.
  `score`: cosine or share of common keywords (Jaccard) or the larger one. Per idea at most `near_limit`
  (default 3) partners (pairs by `score` descending). Without an embedder or without vectors in the index `no_embedder`, on
  an error or another model of the index `unavailable`, each with an honest `message` and the keyword nearness;
  `near: false` → `off` without pairs.
- `filter`: the applied filter (defaults for `near_limit`/`near_threshold` filled in, `open` only with chapters
  of the work, `q` without a search word is dropped).

Example:
```json
{"jsonrpc":"2.0","id":1,"method":"idea.map","params":{"work":3,"status":"open","open":[12]}}
→ {"work":3,"title":"Das Laubenbuch",
   "nodes":[{"id":"w","kind":"work","key":null,"parent":null,"number":"","title":"Das Laubenbuch","sections":[],"open_ideas":5,"total_ideas":5},
            {"id":"n:12","kind":"chapter","key":12,"parent":"n:11","number":"1","title":"Säen","sections":[],"open_ideas":2,"total_ideas":2}, …],
   "ideas":[{"id":48,"node":"i:48","anchor":"n:12","key":12,"label":"Saat, Reihe","words":["Saat","Reihe"],"note":"…","status":"open","origin":{"kind":"human","name":"Autor"},"hit":true}, …],
   "links":[{"id":7,"uuid":"…","from":48,"to":79,"kind":"contradicts","note":"beißt sich","origin":{"kind":"human","name":"Autor"},"created":"…","updated":"…","deleted":null}],
   "near":{"state":"no_embedder","message":"Keine Einbettung angebunden (Einstellungen → Verbindungen); Nähe nur über gleiche Schlagwörter.","pairs":[{"a":48,"b":52,"score":0.5,"via":"words","shared_words":["Saat"]}]},
   "positions":[{"idea":48,"x":120.5,"y":-40.0,"updated":"…"}],
   "filter":{"status":"open","word":null,"origin":null,"q":null,"near":true,"near_limit":3,"near_threshold":0.75,"open":[12]}}
```

**Connections** (`IdeaLink` = `{id, uuid, from, to, kind, note, origin: {kind, name|handle}, created, updated,
deleted}`): only idea ↔ idea of the same work (also both without a work), neither deleted, no self-reference, note
at most 500 characters without control characters except line break; otherwise -32602 with a German sentence, nothing written.
`contradicts` („widerspricht“) is undirected and is stored with `from < to`. `idea.link` on an already
existing pair with the same kind: active → unchanged (`changed: false`, no commit); softly deleted →
restored (`changed: true`, note replaced if a new one comes along). `idea.link_restore` likewise requires
two connectable ideas. A deleted idea takes its connections and position out of map, lists and mirror
(only filtered) and brings them back with `idea.restore`.

Mirror: every change to a connection is exactly one commit in the mirror of the work (without a work in
`<data>/einfaelle/`), subject „Verbindung <id>: Einfall <a> <Art> Einfall <b> (angelegt|gelöscht|wiederhergestellt)“,
trailers `Baruch-Action: idea-link|idea-unlink|idea-link-restore`, `Baruch-Idea-Link: <id>`, `Baruch-Origin:
human|persona|mcp` (with persona/MCP plus `Persona`); `einfaelle.md` carries for every idea the line „- Verbindungen: …“
(see "Ideas"). Positions yield no commit.

```json
{"jsonrpc":"2.0","id":2,"method":"idea.link","params":{"from":79,"to":48,"kind":"contradicts","note":"beißt sich"}}
→ {"link":{"id":7,"from":48,"to":79,"kind":"contradicts",…,"deleted":null},"changed":true,"commit":"3f2c…"}
{"jsonrpc":"2.0","id":3,"method":"idea.unlink","params":{"id":7}}
→ {"link":{"id":7,…,"deleted":"2026-11-08T20:01:02.345Z"},"commit":"9a1b…"}
{"jsonrpc":"2.0","id":4,"method":"idea.map_pos_set","params":{"work":3,"idea":48,"x":120.5,"y":-40}}
→ {"pos":{"idea":48,"x":120.5,"y":-40.0,"updated":"…"}}
{"jsonrpc":"2.0","id":5,"method":"idea.map_pos_reset","params":{"work":3}}
→ {"removed":1}
```

Positions: at most one per (work, idea); the idea must belong to the work and not be deleted, `x`/`y`
finite with an absolute value of at most 1 000 000 (otherwise -32602); setting a second time replaces the position. Resetting deletes hard
(tombstone `idea_map_pos` for the exchange). Errors: unknown work as with the other methods, unknown
idea or unknown connection („Verbindung 7 gibt es nicht.“), wrong parameters → -32602.

### Threads without a section (Round 15)

Conversations per work that hang at no section (migration 0038, `src/threads.rs`): MCP `chat` without `key` creates
one or continues it, the human reads and answers. No book text, no section chat, not in the mirror.
All take `work?` (missing → default work).

| Method | Parameters | Answer |
|---|---|---|
| `thread.list` | `{work?}` | `{work, threads: [{id, work, title, created_by, created, updated, posts, last_author}]}`, most recently written first |
| `thread.get` | `{work?, thread}` | `{work, thread: {…as above}, posts: [{id, thread, position, author, body, sources, created}], url}`; unknown thread (also one of another work) → -32000 „Diesen Faden gibt es in diesem Werk nicht.“ |
| `thread.post` | `{work?, thread?, message, persona?}` | post of the human („Autor“; without `thread` a new thread, title from the message, ≤ 80 characters), then the mentioned active personas (`@handle`) and `persona` answer (at most three, without relay): `{work, thread, title, posts: [{id, author, body, position}], answers: [{persona, model, post_id, text, cut, truncated, relay, asked_by}], errors, url, cut, truncated}`; empty message → -32602 |

Context of an answer in the thread: role prompt, work (title, kind, target audience), the last 12 posts of the thread (each
at most 600 characters; if the context length is not enough, the oldest drop out), the task; `max_tokens` 500. Since Round
17a Baruch also checks „Kritisch:“ quotations here against the shown text (work line, outline, evidence from SWORD, the
own question – not the thread posts) and notes wrong statements about references („(Stelle steht im Text: …)“,
„(Stelle ungültig: …)“). Pages
without JavaScript: `GET /faeden?work` (overview, „Neuer Faden“), `GET /faden?work&thread` (posts, answer form),
`POST /faden/act` (behind `same_origin`; fields `work`, `thread?`, `message`, `persona?`; 303 to the thread, errors
as a notice on the same page with the draft in the field).

#### thread.post_edit, thread.post_versions (Round 28, chat-auftrag)

- `thread.post_edit {work?, thread, id, body}` → `{work, thread, id, body, edited, versions, url}`: only own
  thread posts of the human (others' and persona posts → -32000); the old revision stays as `post_version`.
  Thread posts never stand in the mirror (rule since Round 15), therefore **no commit**.
- `thread.post_versions {work?, thread, id}` → `{work, thread, id, versions}` (oldest first).
- `thread.post` recognises RFC or „@alle“ like `persona.chat` (all active personas, the lector last with a collection,
  no relay stage) and additionally delivers `{rfc, findings}`. A writing task in the thread stays an answer (there
  is no section for a proposal).
- Form without JavaScript `POST /faden/bearbeiten` (`work`, `thread`, `id`, `body`; behind `same_origin`) → 303 to
  `/faden?work&thread#beitrag-<id>`, errors as a notice on the page (200). `GET /faden?…&zitat=<id>` prefills the
  answer with the quoted post („> …“); `POST /faden/act` redirects to `…#chat-neueste`.

### Time watch (Round 16b)

Speaking time per outline item and in total (`src/timing.rs`, counting `baruch_typst::timing`, rules in
`docs/Bedienung.md`, time watch). Words per minute and target time stand per work only in SQLite (`work_timing`,
migration 0039; without a row 110 words per minute, no target time), **not in the mirror** (like the colours of the
personas). All take `work?` (missing → default work, unknown → -32602).

| Method | Parameters | Answer |
|---|---|---|
| `work.timing` | `{work?}` | `{work, kind: "book" \| "sermon", words_per_minute, target_seconds, words, seconds, status: "none" \| "ok" \| "over", over_seconds, shown, clock, target_clock, over_clock, sections: [{key, seq, depth, number, title, words, seconds, start_seconds, subtree_seconds, clock, start_clock, subtree_clock}]}` – read only, no write lock, no commit. Per item in `seq` order the **current** revision; `words`/`seconds` only the item's own text, `start_seconds` = sum of the previous items (time mark), `subtree_seconds` = own text including descendants, `depth` = number of ancestors; `seconds` in total = sum of the items; `status` `over` exactly when `seconds > target_seconds` (`over_seconds` = difference), `none` without a target time; `shown` = sermon or target time set (then outline, tree and editor head show the times); `*_clock` = „m:ss“ or „h:mm:ss“ |
| `work.timing_set` | `{work, words_per_minute?, target_minutes?: number \| null}` | `{work, words_per_minute, target_seconds}`; missing fields stay, `target_minutes: null` (or empty text) deletes the target time. `work` required; words per minute 60–250, target time whole minutes 1–240, otherwise -32602, nothing changed. **Human only:** not over `/act`, not over MCP |
| `timing.count` | `{work?, text}` | `{words, seconds, clock, words_per_minute}` – counts `text` with the same rule as `work.timing` and computes with the words per minute of the work; `text` at most 1 000 000 characters (otherwise or without `text` -32602). The editor widget counts live with it (no second counting rule in TypeScript) |

MCP: `work_timing {work}` delivers the same as `work.timing` (`docs/MCP.md`).

### work.sheet

Round 18c (`src/sheet.rs`, store `work_sheet`, migration 0044): typeset view settings of the work. Read only.

| Method | Parameters | Result |
|---------|-----------|----------|
| `work.sheet` | `{work?}` | `{settings: {font, heading_font, size, fill, heading_fill, background, width}, css}` – each field text or `null` (= from the work template); `css` = `.ws-sheet { --ws-…: …; }` from the work template, overridden by the set fields (`sheet_style_over`), the same as `wysiwyg.style` and the `<style id="ws-style" data-work="W">` in the editor. Unknown work → error |

### work.sheet_set

| Method | Parameters | Result |
|---------|-----------|----------|
| `work.sheet_set` | `{work?, font?, heading_font?, size?, fill?, heading_fill?, background?, width?, reset?: bool}` | `{settings, css}` like `work.sheet`. A missing field stays, `null` or empty text takes it back (again from the template), `reset: true` first takes all back; taking everything back deletes the row. Check in the store: font/heading font at most 200 characters, families separated by commas, each name of letters, digits, spaces, `- _ .` (also in `"…"`/`'…'`); `size` 6–48 pt, `width` 6–60 cm, each as a number with the unit `pt`, `mm`, `cm` or `em` (`em` = 12 pt); colours `#rrggbb` (stored lower case). Invalid → -32000 with a German message („Satzbild: …“), nothing changed; a field that is not text → -32602. No mirror commit, not in the exchange format; Typst, PDF and export stay byte-identical. **Human only:** not over MCP |

Form without JavaScript: `POST /einstellungen/werk/satzbild` (behind `guard::same_origin`, foreign `Origin` → 403):
`method=work.sheet_set`, `work` and all seven fields (empty = from the template) or `reset=1` → 303
`/einstellungen/werk?work=W&ok=satzbild#satzbild`; invalid → 303 `…&satzbildfehler=1#satzbild`, nothing changed.

Image at the cursor position (Round 18c, form `POST /werke/bilder/einfuegen`, behind `guard::same_origin`): to
`work, key, rev` (base), `name`, `caption?` come `place` = `end` (default) | `start` (under the heading) |
`block:<n>` (before the top-level block `n` of `wysiwyg.parse`; `n` ≥ number of blocks = end) | `offset:<n>` (UTF-16 position,
at the next line boundary ≥ n; in the middle of a block that is not a paragraph, after it; empty lines after a block
skipped) and optionally `body` (complete unsaved text, CRLF becomes LF; then the positions within it apply,
and its heading line must be that of the revision `rev`, otherwise 422 „erst speichern“). Never before or into the
heading of the section. The figure stands as a block of its own (an empty line before and after it). Path assignment
(`bilder/<name>` if the image has none) and revision in **one** transaction
(`Store::save_human_with_asset_path`), one mirror commit; if saving fails, no path stays. Unknown
position → 422.

### `library.search`
- `query`: words, all must occur; case and accents do not matter („langmutig“ finds „langmütig“).
  FTS operators in the text have no effect. NUL and control characters separate words like a space; if
  `query` consists only of them (or of words without letters and digits), the answer is -32602 „Leere Suche.“.
- `kinds`: list of `revision`, `post`, `title`, `quote`, `highlight`; if missing, all. Refused
  highlights never occur.
- `work`: work hits only from this work (default: default work); library hits always.
- `all_revisions`: `true` also replaced, discarded and proposed revisions; default only current ones.
- `limit`: default 20, at most 100.
- Hits: `kind`, `work`, `node`, `node_key` (fixed key of the section with `revision`/`post`, otherwise
  `null`), `rev`, `title_id`, `quote_id` (with `quote` the quotation, with `highlight` the
  quotation in which it lies), `highlight_id` (with `highlight`), `key` (citation key), `label` (with
  highlights with their locator), `snippet`, `rank` (bm25, smaller is better). `snippet` is **HTML**: text escaped (`&` `<` `>` `"` `'`), hits in `<mark>…</mark>`;
  Typst brackets stay literal. Callers show it as HTML or remove the tags.

## Errors

| Code | Meaning |
|---|---|
| -32700 | JSON not readable |
| -32600 | not a JSON-RPC 2.0 request |
| -32601 | unknown method |
| -32602 | invalid parameters: unknown work, unknown section, unknown (also deleted) revision or base („Fassung 3 gibt es nicht; vorhanden: 1, 2, 4“, without revisions „vorhanden: keine“), deleting without `confirm: true` („Löschen bitte bestätigen (confirm).“), empty post, empty search, unknown title, import without a heading, unknown numbering scheme or invalid pattern („Nummerierung: …“), unknown image, too large or unsupported image, broken Base64, title of an outline item empty, with a line break or above 200 characters, unknown persona or invalid colour, unknown or refused idea |
| -32603 | internal error (database, template) |
| -32000 | refused on domain grounds, e.g. „Nur Vorschläge lassen sich übernehmen.“, „Diese Fassung ist schon aktuell.“, „Eine Fassung braucht Text.“, „Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.“, „Diese Datei ist schon importiert …“ |
| -32001 | semantic search failed (idea.search with mode rag, Round 19c) |

The database additionally secures the rules of the revision tree itself (migration 0004, see
`docs/Architektur.md`): status changes only → current (from proposed, rejected, superseded), current → superseded
and proposed → rejected („Unzulässiger Statuswechsel der Revision“), origin immutable, current revision
not deletable, parent present and smaller, numbers never reused. The store checks beforehand and
reports domain cases as -32000; a violation that ever occurred outside the store would come as -32603.

HTTP: 421 with a foreign `Host`, 403 for requests from foreign web pages, 415 without `Content-Type: application/json`. The page `/?work=<id>`
with an unknown work answers 404 „unbekanntes Werk“.

## Exchange (Round 26)

Exchange between Baruch instances (`crates/baruch-server/src/exchange/`, `src/instance.rs`; format in
`docs/Austauschformat.md`). The methods are dispatched before the default work is resolved; `work.exchange_export` takes the
default work without `work`, reading in needs no work (it creates one or finds it by the work UUID). Reading in exists only over RPC,
form and command line, not over MCP.

### work.exchange_export


Whole work as an exchange file (`src/exchange/export.rs`), read only.

- Parameters: `work?` (work id; missing → default work), `format?`: `"baruch"` (default, ZIP container) | `"xml"`
  (XML only, images as Base64).
- Answer: `{work, uuid, format, file_name, media_type, bytes, sha256, data?, xml?}` – `uuid` of the work,
  `file_name` `<slug>.baruch` or `<slug>.baruch.xml`, `media_type` `application/vnd.baruch.werk+zip` or
  `application/vnd.baruch.werk+xml`, `bytes` and `sha256` (lower-case hex) of the delivered file; with
  `baruch` the file as Base64 in `data`, with `xml` as text in `xml`.
- Errors: unknown work, `work` not a number, unknown `format` → -32602; a character forbidden in XML in
  an attribute (e.g. author name) → -32603 with the name of the field.
- Format sub-version 2 (Round 25): with `<karte>` (connections between ideas, also softly deleted ones, and
  remembered positions, each with a UUID) and the tombstone kinds `verbindung`/`lage`.
- No secret, no connection to services, no persona models, no requests in the file
  (`docs/Austauschformat.md` §1). The answer is not limited; for very large works download (`GET /werke/{id}/austausch.baruch`)
  and command line are handier.

```json
{"jsonrpc":"2.0","id":1,"method":"work.exchange_export","params":{"work":2,"format":"xml"}}
→ {"result":{"work":2,"uuid":"…","format":"xml","file_name":"die-muehle.baruch.xml",
   "media_type":"application/vnd.baruch.werk+xml","bytes":18234,"sha256":"…","xml":"<?xml …"}}
```

### work.exchange_preview


Preview of reading in; writes nothing (planned in a rolled-back transaction).

- Parameters: `data` (Base64 of the file, `.baruch` or `.baruch.xml`), `file_name?`, `title?` (for the preview
  of a new work under its own title).
- Answer: `{mode: "new"|"merge", work_uuid, title, known_work: id|null, instance: {uuid, name}, exported,
  version, sha256, add: {nodes, revisions, template_revisions, ideas, posts, threads, thread_posts, assets,
  asset_revisions, links, positions}, items: [{kind, uuid, label}] (at most 500), truncated, deviations: [{kind, uuid, key?, rev?,
  label, here, there, action: "fassung"|"antrag"|"hinweis"}], requests: [{op, key, label}], deleted_here: [{kind,
  uuid, label}], unknown: [String], refusals: [String]}`. `kind` are the XML words (`punkt`, `fassung`,
  `vorlage`, `einfall`, `beitrag`, `faden`, `faden-beitrag`, `bild`, `bildfassung`, since Round 25 `verbindung`,
  `lage`; with notes also `werk`, `buch`, `sprechdauer`, `bibelmodul`, `quellen`, `version`). `links`/`positions`
  count connections and positions of the idea map (sync: no duplicate for the same pair and the same kind, a
  position only without a local one; `docs/Austauschformat.md` §4 (h)). `key`/`rev` lead to `/?work=W&key=K&rev=R`.
- Errors: unsafe, broken or invalid file, wrong namespace, limit → -32602 with a reason (§5 in
  `docs/Austauschformat.md`). A taken title is not an error but stands in `refusals`.
- Size: limited by `RPC_BODY_LIMIT` (32 MiB + 64 KiB, Base64); larger files through the form or the
  command line.

```json
{"jsonrpc":"2.0","id":1,"method":"work.exchange_preview","params":{"data":"PD94bWwg…","file_name":"muehle.baruch.xml"}}
```


### work.exchange_import


Taking over (one transaction under the write lock, plans anew, one commit in the mirror).

- Parameters: `data` (Base64), `sha256` (checksum of the file shown before), `mode: "new"|"merge"`, `title?`
  (only „neu“).
- Answer: `{work, slug, title, mode, added: {…like add}, requests: [id], deviations: [...], deleted_here,
  refusals, unknown, commit?, mirror?}` (`mirror` instead of `commit` if the mirror failed; without either
  if a merge changed nothing).
- Errors: `sha256` does not match → -32602; mode does not match the file (new with a known work, merge with an
  unknown one) → -32000; taken title → -32000 with a hint at an own title; file as with the preview
  → -32602. On every error nothing is written.
- Reading in does not exist over MCP (only the human reads in).

```json
{"jsonrpc":"2.0","id":2,"method":"work.exchange_import","params":{"data":"PD94bWwg…","sha256":"0b2e…","mode":"merge"}}
```


### instance.get and instance.set_name


Identifier and name of this Baruch instance (`crates/baruch-server/src/instance.rs`, store `uuids.rs`). The name goes
along in the exchange format as the origin (`<ausgabe instanzname>`, origin of the requests „Austausch: <Name>“).

| Method | Parameters | Result |
| --- | --- | --- |
| `instance.get` | – | `{uuid, name}` – `uuid` from `store_meta.instance_uuid` (migration 0041), `name` empty as long as none is set |
| `instance.set_name` | `name` (text; missing or `null` = empty) | `{uuid, name}` after saving |

- `name` is trimmed; at most 80 characters (characters, not bytes), no control characters (line break, tab,
  DEL, C1). Otherwise -32602 with a German message („Der Name der Instanz darf höchstens 80 Zeichen haben …“ or
  „… keine Steuerzeichen …“), nothing stored. `<` and `>` are allowed; a name from a file read in
  is cleaned (`<` → `‹`, `>` → `›`, control characters → spaces, at most 80 characters; `docs/Austauschformat.md`
  §4), because Git refuses them in the author of the exchange commit. Not text (e.g. a number) → -32602.
- Empty (also only white space) resets the name; the identifier always stays the same.
- There is no MCP tool for it (a setting of the human). Form without JavaScript:
  `POST /einstellungen/daten/instanz` (see `docs/Bedienung.md`). `settings.get {page: "daten"}` additionally delivers
  `instance: {uuid, name}`.
- The work UUID stands in `work.meta` → `work.uuid` (read only; `work.update` knows no field for it).

```json
→ {"jsonrpc": "2.0", "id": 1, "method": "instance.set_name", "params": {"name": "  Arbeitszimmer "}}
← {"jsonrpc": "2.0", "id": 1, "result": {"uuid": "3f2b8c1e-9a47-4d2e-b5c3-0e1f2a3b4c5d", "name": "Arbeitszimmer"}}
```

## Cover layout and imprint (Round 27)

Contract Round 27, section 4. Blocks (`CoverBlock`): `{kind, x_mm, y_mm, width_mm, align, size_pt, leading_em,
color, font, style, backdrop, visible, direction, text}`; coordinates in mm from the trim edge of the respective face.

### Layout (umschlag-daten)

- `cover.layout {work}` → `{work, stored, title_area, blocks: [CoverBlock + stored + height_mm], geometry:
  {page_width_mm, page_height_mm, spine_mm, bleed_mm, safety_mm}, warnings: [{code, block, message}], default_font}`.
  `blocks` always has all 15 kinds in a fixed order (`front_title` … `back_free`): the stored block
  (`stored: true`) or the default block (`stored: false`). `stored` at the top = the work has at least one row (own
  layout). Typesetting error (no page size) → `REFUSED`.
- `cover.layout_set {work, blocks}` → `{work, layout, commit?, mirror?}`. Every named block replaces its kind
  entirely, unnamed kinds stay. A block needs all fields except `font`, `direction`, `text` (missing = `null`);
  the fields `stored` and `height_mm` from `cover.layout` may come along, so a block that was read can be sent back
  changed. Limits (equal to the CHECKs of migration 0045): `x_mm`, `y_mm` −50 to 1000, `width_mm` 1 to
  1000, `size_pt` 4 to 200, `leading_em` 0 to 3, `align` `left|center|right|justify`, `color` `light|dark|#rrggbb`
  (stored lower case), `style` `normal|bold|italic`, `backdrop` `none|shadow|gradient`, `font` at most 200 characters
  of letters, digits, spaces, `- _ .` (no `; { } < > \`, no control characters; empty = font of the
  work template), `direction` `down|up` only on the spine (if missing there: `down`), `text` (at most 1000 characters)
  only with `front_free`, `spine_free`, `back_free`, `back_motto`; the same kind twice in one list is not possible.
  Invalid → `-32602` with a German message naming kind and field (e.g. `„Umschlag-Block „Titel (Vorderseite)“
  (front_title), Feld x_mm: erlaubt -50 bis 1000 mm, nicht 2000.“`), nothing changed. If something changes: exactly one
  commit „Umschlag: Anordnung geändert“ (`Baruch-Action: cover-layout`, empty allowed), body per block
  „Titel (Vorderseite): x 12,0 → 15,0 mm, Schnitt normal → fett“; a block stored for the first time is compared with its
  default block („… (vorher Vorgabe)“). Unchanged → no commit. If the view cannot be built afterwards
  (typesetting error), `layout: null` and `layout_error` stand in the answer; it is stored nevertheless.
- `cover.layout_reset {work, faces?: ["front"|"spine"|"back"], area?: "top"|"middle"|"bottom"}` → like
  `cover.layout_set`. Deletes the rows of the faces (if `faces` is missing: all; per row a tombstone `cover_block`) and
  with `area` sets the title area, both in one transaction with one commit: „Umschlag: Schnellwahl
  oben|Mitte|unten“ (with `faces: ["front"]` with `area`) or „Umschlag: Anordnung zurückgesetzt“
  (`Baruch-Action: cover-layout-reset`), body with the deleted blocks and „Titelbereich: oben → unten“. If
  nothing changes, no commit.
- `cover.fonts {work?}` → `{families: [{name, styles, embedded}], default}` (fonts of typesetting, `default` =
  font of the work template or „Libertinus Serif“).
- Requests (MCP `cover_layout_set`, `docs/MCP.md`): `asset.request_list` shows a request with `op: "cover_layout"`,
  `blocks` (in normal form) and `expected_layout` (SHA-256 over the JSON of the stored blocks at the time of the request).
  `asset.request_decide {decision: "confirm"}` executes it like `cover.layout_set`, with the commit „Umschlag: Anordnung
  geändert (Antrag N)“, requester and confirmer in the body and in the trailers (`Baruch-Request`,
  `Baruch-Requested-By`, `Baruch-Confirmed-By`). If the human has changed the layout in the meantime (fingerprint
  no longer matches), the request becomes `stale` with a reason; refusing changes nothing.

```json
→ {"jsonrpc": "2.0", "id": 1, "method": "cover.layout_set", "params": {"work": 2, "blocks": [
    {"kind": "front_title", "x_mm": 15, "y_mm": 40, "width_mm": 118, "align": "center", "size_pt": 28,
     "leading_em": 0.65, "color": "light", "font": null, "style": "bold", "backdrop": "shadow", "visible": true}]}}
← {"jsonrpc": "2.0", "id": 1, "result": {"work": 2, "layout": {"work": 2, "stored": true, …}, "commit": "3f0c…"}}
→ {"jsonrpc": "2.0", "id": 2, "method": "cover.layout_reset", "params": {"work": 2, "faces": ["front"], "area": "bottom"}}
```

### Imprint template and addresses (umschlag-widget)

- `cover.imprint_adopt {work}` → `{work, template_rev, commit?, mirror?}`: appends the front matter functions of the
  imprint template that the work template lacks (`baruch_typst::cover::imprint_template_missing`) to the template – as a
  new template revision by the human through `work.update` (commit as there). If the template already has all four
  (`schmutztitel`, `haupttitel`, `impressum`, `widmung`): `REFUSED` („Impressum-Vorlage nicht übernommen: …“), nothing
  changed. Form without JavaScript: `POST /werke/umschlag/impressum` (`work`, `back`) → 303 to `back` with
  `ok=impressum`, refused 422 with a message.
- `work.meta.get` additionally delivers `publisher_address` and `printer_address` in `meta`; `work.meta.set` takes both
  (text, multi-line, at most 500 characters, limits in the store). In the commit body they are called „Anschrift des Verlags bzw.
  Herausgebers“ and „Anschrift der Druckerei“ (one line each, line breaks as spaces). Form: fields
  `publisher_address`, `printer_address` in `POST /werke/buchangaben/act`.
- `work.cover_pdf` additionally answers with `checks: [{code, block, message}]` (`cover_print::cover_checks` of the
  stored layout, codes as in `cover.layout`).
- Editor without JavaScript (routes in `cover::pages()`/`writes()`): `GET /werke/{id}/umschlag` and
  `GET /werke/{id}/umschlag.svg` take query fields `b.<kind>.<feld>` (`feld` = `x_mm`, `y_mm`, `width_mm`, `align`,
  `size_pt`, `leading_em`, `color`, `font`, `style`, `backdrop`, `visible` (1/0), `direction`, `text`; numbers with a point
  or comma) as a preview without saving; invalid → 422 with a message naming block and field.
  `POST /werke/umschlag/anordnung` with `work`, `back`, `action=save` (all `b.*` fields → `cover.layout_set`),
  `action=quick` with `area=top|middle|bottom` (→ `cover.layout_reset {faces: ["front"], area}`) or `action=reset`
  with repeated `faces` (none = all) → 303 to `back` with `ok=anordnung`; invalid → 422 with the input. GET with
  typesetting from foreign pages (`Sec-Fetch-Site: cross-site|same-site`) → 403 without a Typst run; the POST forms stand
  behind `guard::same_origin`.

## Findings (Round 28)

Answers of the personas are split into findings (kind praise/hint/critical; status open, adopted, rejected,
deferred, done without a change). Only the human sets a status; assistants only make proposals. Data in
migration 0046 (`finding`, `finding_status`, `post_version`). Errors: unknown finding, limits and status rules →
-32602.

### finding.list, finding.get, finding.set_status, finding.decide (befunde)

Answered by `src/findings/rpc.rs`. All take `work` (number or digits; missing → default work). Numbers may
also come as digit text (forms). A finding as JSON (`Finding`): `{id, uuid, work, key, node, post,
thread, thread_post, position, persona, kind, body, quote, quote_check, status, reason, adopted_rev,
adopted_revision, deferred: {kind: "node", key} | {kind: "idea", id} | null, deferred_uuid, similar_to, summary_of:
[uuid], pending: FindingStatusEntry | null, created, updated, kind_label, status_label, url}`; `kind` = `praise`,
`hint`, `critical` („Lob“, „Hinweis“, „kritisch“), `status` = `open`, `adopted`, `rejected`, `deferred`, `done`
(„offen“, „übernommen“, „verworfen“, „verschoben“, „erledigt ohne Änderung“); `url` = `/werke/<work>/befunde?befund=<id>`.
A history entry (`FindingStatusEntry`): `{id, uuid, finding, status, reason, adopted_rev, adopted_revision,
deferred, deferred_uuid, actor: {kind: "human"|"persona"|"mcp", …}, proposed, note, decision:
"confirmed"|"rejected"|"stale"|null, decided, decided_by, created}`.

- `finding.list {work, key?, keys?, subtree?, post?, status?, kind?, persona?, since?, until?, include_praise? =
  false, include_attached? = false, limit?}` → `{findings: [Finding], total, truncated}` – oldest first; if
  more than `limit` match, the newest `limit`. `keys`, `status` and
  `kind` as a list or text „a,b“; `subtree: true` adds all sub-items to `key`/`keys`; `since`/`until`
  compare `created` (ISO, `until` exclusive); `limit` 1 to 2000 (default 500). Without `include_praise` no
  praise (unless `kind` names it), without `include_attached` no attached findings (`similar_to`). `total` = number of all
  matching findings (without the limit), `truncated` = `total` is larger than the delivered list. Read only.
- `finding.get {work, id}` → `{finding, history: [FindingStatusEntry] (oldest first), attached: [Finding] (attached to
  this one), summary_of: [Finding] (those a collective proposal summarises)}`. Unknown finding or
  finding of another work → -32602.
- `finding.set_status {work, id, status, reason?, rev?, target_key?, target_idea?}` (human only, also over
  `POST /act`; there is no way over MCP) → `{finding, before, commit?|mirror?}`. `adopted` needs `rev` (a
  revision that exists in the finding's section), `rejected` a reason `reason` (trimmed not empty, at most
  2000 characters), `deferred` exactly one of `target_key` (another section of the same work) and `target_idea`
  (idea of the same work), `done` and `open` take `reason` optionally. If something is missing, too long, if
  the revision or target does not exist or the finding already has exactly this state → -32602, and nothing changes. Otherwise
  the store writes the finding and a history entry (`actor_kind = human`, `proposed = 0`) in one transaction,
  and the mirror gets **exactly one commit**: subject „Befund <id>: <Status>“ (e.g. „verworfen“, „übernommen in
  Fassung 3“, „verschoben nach 1.2 Titel“), body with kind, persona, place, text and reason, trailers
  `Baruch-Finding`, `Baruch-Action: finding-status`, `Baruch-Node` (fixed key, if the finding has a
  section); the author is the human.
- `finding.decide {work, entry, confirm: bool}` (human only) → `{finding, entry, commit?|mirror?}`. Confirmed
  (`confirm: true`), the finding gets the proposed status (`decision = confirmed`); refused, it stays
  (`rejected`). A **stale** proposal – the finding has since received a status from the human or from a
  confirmed proposal, the proposed revision or the target is missing, or the finding already has the status – becomes
  `stale` and changes nothing. Unknown or already decided proposal → -32602. Exactly one commit (trailers
  `Baruch-Action: finding-decide`, `Baruch-Proposer: <Herkunft des Vorschlags>`; the author is the human).

Proposals of assistants do not go through `/rpc` but through `findings::rpc::propose` (MCP
`finding_propose_status`, befund-uebersicht): only `adopted`, `deferred`, `done` – the store refuses `rejected` and `open`
(„Nur der Mensch kann einen Befund verwerfen oder wieder öffnen …“, -32602); an equal open proposal
(same status, same revision or same target) is not created twice (-32602). Exactly one commit
(`Baruch-Action: finding-propose`, trailer `Persona: <handle>`, author the persona or the MCP client). Creating
findings when splitting is not a commit; `befunde.md` stands in the next snapshot.

Exchange: since sub-version 4, `work.exchange_export`/`work.exchange_import` carry findings with their history and old
post revisions; the preview counts them in `add.findings`, `add.finding_entries`, `add.post_versions`
(`docs/Austauschformat.md`).

### finding.matrix and page /werke/<id>/befunde (befund-uebersicht)

`finding.matrix {work?, status?, kind?, persona?, since?, until?, include_praise? = false, older_than_days?}` – read
only (`src/findings/page.rs`). If `work` is missing, the default work. `status` or `kind` as a list or text with commas
(database words `open|adopted|rejected|deferred|done` or `praise|hint|critical`); `since`/`until` as a time like
`2026-11-01` (`created >= since`, `created < until`); `older_than_days` 0–3650 (only findings that are older). Wrong
values → -32602. Answer:

```json
{"work": 2, "title": "Wanderbuch", "personas": ["@lektor", "@nathan"],
 "groups": [{"kind": "part", "key": 12, "number": "I", "title": "Aufbruch",
             "rows": [{"key": 13, "number": "1", "title": "Morgen", "level": 1, "part": false, "keys": [13, 14, 15],
                       "cells": [{"open": 2, "adopted": 0, "rejected": 1, "deferred": 0, "done": 0, "praise": 0, "total": 3}, …],
                       "total": {…}, "open_badge": 3}, …],
             "totals": [{…}, …], "total": {…}, "open_badge": 3}, …],
 "totals": [{…}], "total": {…}, "open_by_key": {"13": 2, "14": 1}, "open_findings": 7,
 "history": [{"key": 13, "label": "1 Morgen", "total": {…},
              "revisions": [{"key": 13, "label": "1 Morgen", "rev": 1, "findings": [4]}]}],
 "filter": {"status": [], "kind": [], "persona": null, "since": null, "until": null, "include_praise": false,
            "older_than_days": null},
 "truncated": null}
```

- **Rows** are chapters with their sub-items (`keys`); sections without a chapter stand on their own, a part with its own
  findings (at the part or at sections directly under it) gets the row „… (Teil selbst)“ (`part: true`). Every
  chapter has a row, even without findings.
- **Groups**: `kind = "part"` per part, `"none"` for chapters without a part before them, `"unplaced"` for findings without a section
  (threads, deleted sections) at the end; per group sums per persona (`totals`) and in total (`total`).
- **Columns** (`personas`): personas with findings in the filter, in the order of the personas, then alphabetical.
- **Counting**: attached findings (`similar_to`) never; praise only with `include_praise` and then only in `praise` (never as
  open). `open_badge` = sum of `open_by_key` over `keys` (counting rule §1.5, without a filter): without a filter, per row
  `total.open == open_badge` holds – the same number as the badges in outline, editor head and `/gliederung`.
- **Capped** (`truncated`): if more than 2000 findings match the filter, the cells count only the newest 2000 and
  `truncated` names the number of all (otherwise `null`); the badges (`open_by_key`, `open_findings`) always count all.
- **History** (`history`): rows with decided findings; `revisions` per section and revision the adopted
  findings (`Store::findings_adopted_in`, only findings in the filter).

Page and forms (usable without JavaScript):

- `GET /werke/<id>/befunde` with the same filters as GET parameters (`status` and `kind` repeatable,
  `include_praise=1`); unknown work → 404, invalid values are dropped. In addition `key` (+ `persona`): findings of the cell
  (chapter with sub-items; attached ones under their original), `befund=<id>`: one finding with history, original and
  sources of a collective proposal; without either the list „offen, älter als n Tagen“ (`older_than_days`, default 14),
  the collective proposals and the history per chapter. `ok=status|entscheidung` shows the feedback.
- `POST /werke/befunde/act` (behind `guard::same_origin`): `method=finding.set_status` (`work`, `id`, `status`,
  `reason?`, `rev?`, `target_key?`, `target_idea?`; empty fields are dropped) through `findings::rpc::parse_change` and
  `findings::rpc::set_status`; `method=finding.decide` (`work`, `entry`, `confirm=1|0`, `id` only for the error page)
  through `findings::rpc::decide`. Then 303 to `back` (own addresses only) or `/werke/<id>/befunde?befund=<id>&ok=…`;
  errors 200 with a notice (`role="alert"`) and the draft in the form; unknown action 400, unknown work 404,
  foreign page 403.
- Idea map: `idea.map` additionally takes `findings?: bool` (layer „Befunde“, default `false`); then every
  node carries `open_findings` (at the node including everything below it) and every summarised section its own. Page
  `/werke/<id>/karte?befunde=1`.

## Search and replace (Round 30a)

All with `work?`; the fields of the search lie flat: `q` (1–500 characters), `case`, `word`, `re` (default `false`),
`range` = `selection|section|chapter|part|from|picked|work|all` (default `section`), `key` (for selection, section,
chapter, part, from), `sel: [key]` (picked), `start`/`end` (UTF-16, selection only), `rev` = `current|shown` (shown only
with section/selection, with `shown_rev`), `kinds: [text|heading|footnote|quote|code]` (default when searching
`text, heading, footnote, quote`, when replacing `text, heading, footnote`; Baruch never replaces code), `text`
(unsaved editor text of `key`), `limit` (default 500, at most 2000).

- `search.find {…}` → `{hits: [{work, key, number, title, rev, kind, start, end, u16_start, u16_end, line, before,
  matched, after, url}], total, truncated, timed_out, range_label}`. `start`/`end` are bytes, `u16_*` UTF-16 units
  in the text of the revision; `url` is the jump address (`/?work=W&key=K[&rev=R]&find=…&at=Byte`). Time limit 2 s
  (`timed_out: true`, the list is then incomplete). Invalid or too large regular expression → -32602.
- `search.preview {…, replacement}` → `{sections: [{key, number, title, base_rev, hits, replacements, diff_html,
  body_after}], total, truncated, timed_out, range_label}`. `replacement` at most 2000 characters, regex with `$1`/
  `${name}`, escaped for Typst in the wording mode. `range=all` → -32602. The inserted replacement texts of all hits
  together may be at most 8 MiB (every hit counts at least one byte), otherwise -32000 „Zu viele
  Ersetzungen auf einmal …“ (applies likewise to `search.replace`, then nothing is replaced). `sections` contains only
  whole sections, together at most 500 sections and 2000 places (otherwise `truncated: true`).
- `search.replace {…, replacement, sections?: [{key, base_rev, skip: [Stelle]}], confirm: true}` → `{batch: {id, work,
  created, query, replacement, items: [{key, prev_rev, new_rev}], undone}, changed: [{key, rev, prev}], skipped:
  [{key, reason}], commit?, mirror?}`. Human only (not over MCP, not over `/act`). `skip` counts in the
  hit list of the preview; without `sections` all sections with hits on their current revision. Skipped
  with a reason: base no longer current, level or headings would change, no hit any more. At most 500
  sections; without `confirm` → -32602; after the time runs out → -32000, nothing replaced. Exactly one commit `„Ersetzt „x“ → „y“ in
  n Abschnitten“`.
- `search.undo {batch, confirm: true}` → `{batch, restored: [{key, rev}], skipped: [{key, reason}], commit?}`; one
  commit `„Ersetzen rückgängig: „x“ → „y“ (n Abschnitte)“`; already undone → -32000.

## Cross-references (Round 30a)

All with `work?`.

- `xref.targets {q?, limit?}` → `{targets: [{key, label, number, title, level, snippet}]}`. Filters by title
  (contains), number (begins with) or label `kN`; `level` 0 part … 3 subsection; `snippet` =
  `#link(<kN>)[Titel]`; `limit` default 50, at most 500.
- `xref.resolve {label}` → `{key, url, number, title}` (`label` as `kN` or `<kN>`; unknown → -32602).
- `xref.candidates {key?}` → `{candidates: [{key, rev, start, end, text, target_key, replacement}]}`: possible
  cross-references (mentions in the running text of the current revision, see `xref::outline_mentions`), proposal only.
- `xref.convert {key, rev, picks: [Stelle], confirm: true}` → `{node, key, rev, converted, commit?}`: the chosen
  candidates of the revision `rev` (positions in the list of `xref.candidates` for this section) are inserted,
  a new revision by the human on `rev`, one commit. Without `confirm` → -32602.
- `xref.broken {}` → `{broken: [{key, rev, label, start, text, target}]}`: references to keys without a current
  revision.

## Solution proposals and variants (Round 30b)

All methods with `work?` (missing → default work) and `key` (fixed key of the section).

- `solution.list {key, post?, rev?}` → `{solutions: [{index, quote, replacement, step, adoptable, finding}], rev}`:
  with `post` the solutions of the post (`index` = position for `picks`), otherwise the solutions of all findings of the section
  (`index` per finding, `finding` set). `adoptable`: quotation and new wording, the quotation stands in revision `rev`
  (without: the basis of review of the post or the current one; white space may differ).
- `solution.adopt {key, post | finding, rev?, picks: [usize]}` → `{rev, base, persona, note, url,
  proposed_findings: [id], notes: [..]}`: inserts the chosen solutions one after another into the base (`rev`, otherwise
  the basis of review of the post, for a finding the current revision) and creates **one proposal** (origin the
  persona of the post, note „Lösung aus Beitrag n“ or „… (Befund [B m])“); the findings with these solutions
  get the status proposal „übernommen in Fassung n“. A solution that cannot be adopted, an unknown position, a post from
  another section → `-32602`; text unchanged → `-32000`. `picks` also as a number or „1,2“.
- `solution.variants {key, rev?, findings?: [id], count?: 2|3, post?}` → `{group, base, persona, variants: [{rev,
  label, summary, findings, url}], notes}`: the lector (by setting) writes the variants as proposals on
  the base `rev` (default: current revision) from the named or open findings of the section; `count` other
  than 2 or 3 → `-32602`; without findings → `-32000`.
- `solution.reject_siblings {key, rev, confirm: true}` → `{rejected: [rev], group, kept}`: discards the other open
  proposals of the variant group of `rev` (human only; without `confirm` or no variant → `-32602`).
- `solution.compare {key, group}` → `{base, group, key, work, title, variants: [{rev, label, summary, findings,
  status, diff_html, against: [{rev, label, diff_html}], url}]}`; `diff_html` is escaped HTML (`<ins>`/`<del>`).
- Basis of review `basis` = `shown` (default) | `current` | number or digits with `persona.ask` and `persona.review`
  (answer `basis: {rev, label, proposal}`); `persona.propose` is based on the shown revision `rev` or the
  explicitly chosen one (`basis` = `current` | number), also on an open proposal (bug 0.9.0, critique cycle;
  before always on the current one). Since Round 36 this also holds for `scope: "chapter"`: the proposal for the part
  of the task (`key`) is based on the shown or chosen revision, the other parts on their current one; the
  answer names it in `basis` („Grundlage: Fassung 2 (Vorschlag von @lektor)“), and a proposal equal to the shown
  revision counts as unchanged. In the chat a directly addressed persona sees its own open findings
  of the section (re-checking as with the RFC) – since Round 36 only with a review task
  (`assist::chat::is_check_order`: a word begins with `prüf`, `überprüf`, `nachprüf`, `gegenprüf`, `kontrollier`, `check` or
  `review`, or the message names „[B<n>]“), not with a plain question. For the chat
  `persona.chat` reads the parameter with Round 30c (chat-bedienung). Unknown word → `-32602`.
- Form `POST /loesungen/act` (behind `same_origin`): `method` = `solution.adopt` | `solution.variants` |
  `solution.reject_siblings`, plus `work`, `key`, `rev`, `post`, `finding`, `picks` (repeatable), `findings` (repeatable),
  `count`, `confirm`, `back` (own address); success → 303 (proposal or `/varianten` or `back`), error → 200
  with a notice. Page `GET /varianten?work=W&key=K&group=G` (without `key`: section from the group).

## Chat: repeating, deleting, addressing, live, structured posts, labels (Round 30c)

All methods take `work?` (otherwise the default work); the acting party is the human.

- `post.repeat {id, rev?, basis?}` → `{work, key, post: {id, author, body}, url, repeat_of, basis: {rev, label, proposal},
  proposed_done: [finding id], notes: [Text]}`. Repeats the request of a persona post: persona, mode (`ask`,
  `review`, `chat`), scope and text from the stored task (`post.request`, migration 0047), against the shown
  revision `rev` or according to `basis` (`shown` default | `current` | `<n>`, `assist::basis::choose`). The new post carries
  `repeat_of` (the old post stays), `reply_to` like the old one. Open findings (without praise) of the old answer that do not
  come back in the new one (`findings::parse::similar` against the findings of the new answer) are proposed by the persona as
  „erledigt“ (`finding_propose_status`, one commit `finding-propose` each; the human decides). Fallback for
  posts without a stored task (before Round 30): the preceding message of the human or of an MCP client in the
  same section as chat; if there is none or the post is not from a persona (or is the short post of a
  writing task) → `-32000`. If the model fails → `-32000`, nothing stored, error in the live display.
- `post.since {key, after, rev?}` → `{work, key, posts: [{id, author, html}], truncated, typing: [{persona, since}],
  failures: [{persona, message, at}]}`. Posts of the section after the post `after` (if it no longer stands there: all
  with a larger id), at most 50, as finished HTML (macro `post_entry` from `fragments/post_item.html`, as in the
  section chat against the revision `rev`); `typing` = personas with a running task in this section
  (`chat_live::typing_now`), `failures` = failed model calls of the last ten minutes (at most five, oldest
  first; `at` ISO time, the browser shows each once).
- `thread.post_delete {thread, id, confirm: true}` → `{work, thread, id, deleted: {author, body}, url, commit?|mirror?}`.
  Without `confirm: true` → `-32602`. The human deletes any thread post; exactly one commit „Fadenbeitrag n: gelöscht“
  through `mirror::record_request` (trailers `Baruch-Thread`, `Baruch-Thread-Post`, `Baruch-Action: thread-post-delete`,
  body with the whole text; the tree of the mirror stays, threads do not stand in the mirror). References of other posts
  (`reply_to`, `repeat_of`) become NULL.
- `thread.repeat {thread, id}` → `{work, thread, post: {id, author, body}, url, repeat_of, proposed_done, notes}`: like
  `post.repeat` in the thread (task = message of the sender, `request.scope = "thread"`; fallback the preceding
  message of the human or of an MCP client); without a new relay.
- `thread.since {thread, after}` → `{work, thread, posts: [{id, author, html}], truncated, typing, failures}` (macro
  `thread_entry`).

Changed behaviour of existing methods:
- `persona.chat` and `thread.post`: addressing by name at the start of a sentence („Lektor, …“, `assist::chat::addressed`) calls like
  `@handle`; a persona answer with `@handle:` or an address at the start of a line calls the addressed persona once
  (relay: one stage, at most three relay answers, each persona once, not with RFC/writing task). `persona.chat`
  takes `basis` (`shown` | `current` | `<n>`); unknown value → `-32602`.
- `post.page` stays plain (text without structure); `post.since` delivers the typeset HTML.

Forms (without JavaScript, behind `guard::same_origin`):
- `POST /beitraege/act` takes `method=post.repeat` (fields `id`, `rev`, `basis`) → 303 to the section with
  `#post-<neu>`; error 422 with text.
- `POST /faden/act` takes `method=thread.post_delete` (checkbox `confirm=1`, otherwise a notice on the page, status 200)
  or `method=thread.repeat` (field `id`) → 303 to the thread; without `method` a message as before.
- `POST /assistenten/act` passes `basis` on to the RPC for all four methods.

## Typesetting settings (Round 29)

Typesetting settings of a work (chapter start, page format, margins, font, page numbers …) as Typst rules
„// Baruch: Satz“ after the work template; data model `baruch_typst::typeset::TypesetSettings` (JSON: only set
fields, unknown fields are refused). Changed only by the human (user interface); MCP only reads.

### work.typeset_get, work.typeset_set, work.typeset_reset, work.typeset_adopt_rule (satz-daten)

Module `crate::typeset_settings` (Round 29a). Only from the user interface (= human, `HUMAN_NAME`); `work` missing → the
default work, unknown work → `-32602`. The writing methods take the write lock and produce **exactly one**
mirror commit per change; none without a change.

JSON of the settings (example with all sixteen fields; every missing field = „aus der Werkvorlage“):

```json
{"chapter_start":"right_page","part_page":{"side":"right","blank_back":true},"page_format":{"kind":"a5"},
 "margins":{"inside_mm":20.0,"outside_mm":15.0,"top_mm":18.0,"bottom_mm":22.0,"binding_mm":5.0},
 "font":"Libertinus Serif","font_size_pt":10.5,"leading_em":0.65,"paragraph":{"kind":"indent","em":1.0},
 "justify":true,"hyphenate":true,"lang":"de","page_numbers":{"position":"bottom_outside","start":"first_chapter"},
 "running_head":"chapter","footnotes":{"numbering":"arabic","restart":"per_chapter"},"widows_orphans":true,
 "xref_with_page":true}
```

Values: `chapter_start` `continuous|new_page|right_page`; `part_page.side` `right|any`; `page_format.kind`
`a5|120x190|135x215|a4|custom` (with `custom` with `width_mm`, `height_mm`); `paragraph.kind` `indent|spacing`;
`page_numbers.position` `bottom_center|bottom_outside|top_outside|hidden`, `.start` `first_page|first_chapter`;
`running_head` `off|work_title|chapter`; `footnotes.numbering` `arabic|symbols|letters`, `.restart`
`continuous|per_chapter`. Limits and rounding as `TypesetSettings::validate`/`normalized` (mm and pt to 0.1, em to
0.01; font and language trimmed).

- `work.typeset_get {work?}` → `{work, set, uuid|null, updated|null, updated_by|null, settings, rules, conflicts
  [{field, label, line, text, effect, message}], template_rule {line, text, chapter_start}|null, warnings [String],
  fields [{id, label}]}`. `rules` is the block „// Baruch: Satz“ from `baruch_typst::typeset::rules` ("" without
  settings); `warnings` names e.g. a base font that Typst does not know (`„Schrift „X“ fehlt; Typst setzt mit
  der eingebetteten Schrift“`) or the running head „Werktitel“ although the work template has no `#set document(title: …)`
  (the running head would stay empty); `fields` are the sixteen field ids with a label in a fixed order.
- `work.typeset_set {work?, settings}` → `{work, settings, changed [ids], commit?, mirror?}`. Replaces the settings
  entirely (normalised). Invalid → `-32602` with **all** messages („Satz-Einstellungen: Rand innen: 5 bis 80 mm;
  Schriftgröße: 6 bis 24 pt; …“), nothing changed; unknown fields or missing `settings` → `-32602`. Equal
  values → `changed: []`, no commit. Empty settings `{}` reset (commit like `work.typeset_reset`).
  The uuid of the row stays the same on changes.
- `work.typeset_reset {work?}` → `{work, changed, commit?, mirror?}`. Deletes the row (tombstone `work_typeset`,
  in the exchange `satz`); without settings `changed: []`, no commit.
- `work.typeset_adopt_rule {work?, confirm: true}` (Round 23: without `confirm: true` -32602) → `{work, chapter_start, template_rev, changed, commit?, mirror?}`. Recognises the
  chapter start rule of the work template (`detect_chapter_break`), sets `chapter_start` to its value (other fields
  stay), removes the line(s) from the template (`remove_rule`) and creates in **one** transaction a new
  template revision (note „Kapitelanfang in die Satz-Einstellungen übernommen (Runde 29)“, author the human) – one
  commit for both. No recognised rule → `-32000` „Die Werkvorlage hat keine erkennbare Regel für den
  Kapitelanfang; nichts geändert.“
  The method does not ask for a confirmation (like `work.typeset_reset`): `/rpc` is the human's way behind the
  origin check, the caller obtains the confirmation – the form requires the checkbox (`confirm=1`, otherwise 422);
  today no widget calls the method. The old template revision stays in the revision history.

Mirror commit (`mirror::describe_typeset`): author and committer the human; subject „Satz: <Bezeichnungen> geändert“,
„Satz: Einstellungen zurückgesetzt“ or „Satz: Kapitelanfang aus der Werkvorlage übernommen (Vorlagenfassung n)“;
body per changed field „Bezeichnung: vorher → nachher“ (unset = „aus der Werkvorlage“, e.g. „Schriftgröße:
10,5 pt → 11 pt“); trailer `Baruch-Action: typeset-set|typeset-reset|typeset-adopt`. The commit is made even without a
file change (`allow_empty`, e.g. for „Verweis mit Seitenzahl“, which produces no Typst rule). `main.typ` carries the
rule block automatically (`snapshot::head`); there is no mirror file of its own. If the mirror fails, the
change holds nevertheless; the answer carries `mirror` instead of `commit`.

### work.typeset_preview (satz-oberflaeche)

`work.typeset_preview {work?, settings}` → `{work, stand, pages, urls, errors, warnings, diagnostics, conflicts}` – typesets
the **unsaved** settings `settings` (JSON as with `work.typeset_set`) as a trial; read only, no commit.
Source: `baruch_typst::typeset::preview_source(settings, Werkvorlage, Werktitel)` (work template, rule block
„// Baruch: Satz“, invented sample text with one part and three chapters) and the images of the work; typeset through
`typeset::compile` (one lock with the typesetting preview and PDF, time limit 60 s).

- `stand`: hash of the typeset state; `pages`: page count; `urls`: per page
  `/einstellungen/werk/satz/vorschau/<stand>/<n>.svg` (n from 1).
- `errors`, `warnings`: **number** of errors or warnings from Typst; `diagnostics`: `[{severity, message}]`.
- `conflicts`: `[{field, label, line, text, effect, message}]` as with `work.typeset_get`, but for the passed
  settings.
- Errors: `settings` missing, unknown fields or wrong types → `-32602`; values outside the limits → `-32602` with
  all messages of `TypesetSettings::validate`, separated by „; “ (each begins with the label, e.g.
  „Schriftgröße: 6 bis 24 pt“); unknown work → `-32602`; time limit or abort → `-32000`.

Forms and pages (block „Satz“ in Settings → Work, without JavaScript; POST behind `guard::same_origin`, foreign
origin → 403):

- `POST /einstellungen/werk/satz` with `work` and `action`:
  - `save` (default) → `typeset_settings::save` → 303 `/einstellungen/werk?work=W&ok=satz#satz`;
  - `preview` → 200, whole settings page with the unsaved values and `<img>` on the preview images;
  - `reset` (required `confirm=1`) → `typeset_settings::reset` → 303 `…&ok=satz_zurueck#satz`.
  Fields (literal, "" = from the work template, decimal comma allowed): `chapter_start`, `part_side`, `part_blank_back=1`,
  `page_format`, `page_width_mm`, `page_height_mm` (only `custom`), `margin_inside_mm`, `margin_outside_mm`,
  `margin_top_mm`, `margin_bottom_mm`, `binding_mm` (all four margins or none; binding offset empty = 0), `font`,
  `font_size_pt`, `leading_em`, `paragraph`, `paragraph_em` (empty = 1 or 0.5), `justify`, `hyphenate`,
  `widows_orphans`, `xref_with_page` (each `""|on|off`), `lang`, `page_number_position`, `page_number_start` (empty =
  `first_page`), `running_head`, `footnote_numbering`, `footnote_restart` (empty = `continuous`). Sub-fields without
  a main choice (e.g. `part_blank_back` without `part_side`) do not count. Read by
  `settings::work_typeset::settings_from_form` (messages like `TypesetSettings::validate`, plus „unbekannte Wahl“ and
  incomplete margins).
- `POST /einstellungen/werk/satz/vorlage` with `work`, required `confirm=1` → `typeset_settings::adopt_template_rule` → 303
  `…&ok=satz_vorlage#satz`.
- Invalid or refused (also a missing `confirm`) → 422, whole settings page with the input and the message at the
  field (`settings::work_retry_page`, `WorkRetry::Typeset`); internal error → 500. Unknown work → 404, unknown
  `action` → 400.
- `GET /einstellungen/werk/satz/vorschau/{stand}/{n}.svg`: only states from the typesetting cache
  (`typeset::cached_page`, never starts a typesetting run), otherwise 404; `Sec-Fetch-Site: cross-site|same-site` → 403. Headers like
  the pages of the typesetting view: `image/svg+xml`, `X-Content-Type-Options: nosniff`, `Content-Security-Policy` =
  `typeset::SVG_CSP`, `Cache-Control: no-cache`.

## Round 19 – RAG maintenance, rest of the references

### R19a index

No new method, no new route; details in "Semantic search (RAG, Round 12)" and "Sync with
MultiGPT (Round 13)".

- `rag.search`: every hit with `weak: bool` (score below 0.45 or not a number).
- `rag.status`: `index.stale` (stuck jobs); `problem` stays set as long as something is stuck, even
  if the last (cheap) job succeeded; searches trigger no new attempt.
- `rag.reindex`: answers immediately, additionally `deferred: bool` (reset wish because the worker held the
  write lock; it aborts between two batches, or a hanging embedding immediately).
- LanceDB: `query_prefix`/`document_prefix` of the connection (change of the document prefix → failed until
  the rebuild); `Index::replace` deletes only within its area (one work or shared).
- Start with its own `--data` without a stored connection Eliezer: no embedder, `problem` names the reason.
- `sync.run`: upload without an id with a note and a block mark, `isError` for one document blocks only that one,
  incomplete lists delete no local copy, a mark against a silent abort between upload and state.
- `connection.check` for MultiGPT ends its MCP session last with HTTP `DELETE`.

### R19b pgvector

- `connection.set`/`connection.get`: `query_prefix` (lancedb, pgvector) and `document_prefix` (lancedb), see
  "Connections and sources"; `connection.check` (pgvector) with shortened fields and a TLS note.
- pgvector search (`rag.search` over the way pgvector): all fields shortened in SQL – document 500, section 200, title
  300, address 2000, collection 200, text 2000 characters; a collection with a longer name can be chosen through the shortened name;
  the query is embedded with `query_prefix`.
- Idea map: `idea.map_groups`, `idea.map_groups_name`, `idea.map_group_accept`, `idea.map_group_dismiss` (see
  "Idea map (Round 25)"); form `POST /werke/{id}/karte/act` with the same three writing methods.
- `ribbon.formats`: `scripture` and `cite` from the work template (see "ribbon.formats").

### R19c treffer

Weak hits, wording with hyphenation remnants and the rest of the references (sections "Ideas", "Bible references",
"Bibles", "Literature and highlights"). Everything additive, no new methods.

- **Weak** means: similarity `score` < `baruch_rag::WEAK_SCORE` (0.45, an assumption for all ways) or not a number
  (`baruch_rag::is_weak`). Nothing is hidden for good.
- `idea.search {mode: "rag"}`: per hit `weak: bool`, weak hits after the others (otherwise rank). Failure of the
  semantic search (LanceDB not connected or an error of the search other than invalid input) → **-32001**
  (`rpc::RAG_UNAVAILABLE`) with the message „Bedeutungssuche nicht verfügbar: LanceDB (lokal): <Grund>.“; invalid
  input stays -32602.
- `literature.render` (mode `rag`) and the semantic search of the literature (`literature::rag_view`): per hit
  `weak`, hits ordered by `weak` (stable), plus `weak_count` and `weak_label` („schwach“).
- `scripture.check`: per reference new `module_spacing: bool` – the wording matches only without white space (hyphenation remnants in the
  module like „die ses“, „Men schen“); `wording_code` stays `match`, `wording` „Wortlaut stimmt (Leerraum im Modul)“
  (partial quotation: „Wortlaut stimmt (Teilzitat, Leerraum im Modul)“). New per answer and per reference `module_note:
  string|null`: a note if the module has conspicuously many word fragments (estimate on the first 4000 verses,
  at most 2 s, per conf file in memory; otherwise `null`, without a Bible always `null`).
- `bible.marks`: per mark `module_spacing`, per answer `module_note` (as above).
- `bible.lookup`: per verse new `german_ref` (German reference, mapped back from the versification of the module, e.g. KJV
  „Mal 4,2“ → „Mal 3,20“) or `german_problem` (reason why the verse cannot be mapped; then
  `german_ref: null`).
- `lexicon.lookup`: lemma probe if the keyword is missing – Strong spellings („G26“ ↔ „G0026“, also with
  `StrongsPadding=false`), simple endings, umlaut restoration („Bäume“ → „Baum“), finally the last component
  of a compound („Feigenbaum“ → „Baum“). New `probe` (the form tried or `null` if the keyword itself
  matched) and `asked` (the keyword asked for).
- References in running text: „Mal“ with a following chapter and verse is Malachi, not a numeral („Er las Joh 3,16
  Mal 3,20 vor.“ yields both references). Quotation assignment: between a reference and the following quotation up to three
  short introductory words with a colon may stand („wie Lk 15,6 erzählt: „…““, „heißt es:“, „sagt:“; list
  `wording::INTRO_WORDS`). Wording comparison in Unicode normal form NFC.

### Integration

No RPC change through the integration. The acceptance test `tests/round19_abnahme.rs` goes through the methods described here:
`connection.set` with `query_prefix`/`document_prefix`, `rag.status` (`index.stale`), `rag.search`
(`weak`, pgvector through `connection.rag_set`/`connection.collections_set`), `rag.reindex` (`deferred`), `work.delete`,
`idea.search` (`mode: "rag"`, `weak`), `persona.review` (evidence block), `idea.map_groups`, the forms to
`/werke/{id}/karte/act` (`idea.map_groups_name`, `idea.map_group_accept`) and `scripture.check` (`module_spacing`,
`module_note`, „Mal“).

## EPUB (Round 20)

Since Round 20 all implemented (no STUB): `work.epub` (R20a), `epub.readers` and `epub.preview` (R20b),
`epub.check`, `epub.checker_get` and `epub.checker_set` (R20c).

### work.epub (R20a)

Implemented since R20a (no longer a STUB).

`work.epub {work, inline?: bool=false, footnotes?: "popup"|"endnotes"="popup"}` → `{file_name, bytes, sha256, url,
footnotes, files:[{href, media_type, kind, title, key}], warnings:[{code, message, key}], epub_base64?}`.

- Builds the EPUB of the current state (current revisions, work template, images, cover, book details,
  typesetting settings) and delivers information; **no commit**, nothing is stored.
- `file_name` = `<slug>.epub`, `bytes` = size of the ZIP file, `sha256` = checksum, `url` = `/werke/{id}/epub`.
- `files`: files in reading order (spine), relative to `OEBPS/` – `kind` ∈ `cover`, `front` (front matter, preliminaries),
  `part`, `chapter`, `notes`; `title` = displayed heading, `key` = key of the first section of the file.
- `warnings`: `{code, message, key}` (`key` = section or `null` for work template/metadata). Codes:
  `ISLAND_OMITTED`, `ARGS_OMITTED`, `IMAGE_NO_ALT`, `IMAGE_MISSING`, `IMAGE_PATH_INVALID`, `IMAGE_TYPE_FOREIGN`,
  `XREF_UNKNOWN`, `TEMPLATE_PAGEBREAK_MAPPED` (info), `TEMPLATE_PAGEBREAK_UNMAPPED`, `ISBN_INVALID`,
  `META_YEAR_IGNORED`, `LANGUAGE_INVALID`.
- `inline: true` → additionally `epub_base64` (the file as Base64).
- Errors: unknown kind of footnotes → -32602; work without a section → -32000 („Das Werk hat keinen Abschnitt …“).
- Same state → same bytes: fixed ZIP timestamps and order, `dcterms:modified` = latest point in time from
  work, book details, typesetting settings, images and revisions. Assumption: merely switching the current revision
  without a new revision does not change this point in time (the content and therefore the checksum change nevertheless).

### epub.readers and epub.preview (R20b)

Both implemented (R20b). Read only, no commit.

- `epub.readers {}` → `{readers:[ReaderProfile…]}` in display order: each `{slug, name, device, width_px,
  height_px, required, via_conversion, supports:{popup_footnotes, embedded_fonts, css_backgrounds, page_break_css,
  hyphenation, svg_images, night_mode, overrides_publisher_font}, note}`. Slugs (frozen): `apple-books-iphone`,
  `apple-books-ipad`, `google-play-books`, `kindle` („Kindle (über EPUB)“), `kindle-app` („Kindle-App (über
  EPUB)“) with `required: true`; `tolino`, `kobo`, `thorium`, `calibre`. `via_conversion: true` only for Kindle and
  Kindle app (Send to Kindle converts the EPUB). The values are assumptions without a device (see Architektur, R20b).
- `epub.preview {work, readers?: [slug], file?: href, footnotes?: "popup"|"endnotes"="popup"}` →
  `{readers:[{slug, name, url}], files:[{href, title, kind, key}], file, footnotes, warnings:[{code, message,
  key}]}`. Without `readers` (or empty) the required profiles; order always as `epub.readers`. Without `file` the
  first part or chapter file, otherwise the first file with text. `files` = files with text in reading order
  (spine without `nav.xhtml`/`toc.ncx`; `kind` like `ItemKind`, e.g. `part`, `chapter`, `front`, `cover`, `notes`).
  `url` = `/werke/{work}/epub/leser/{slug}/{file}` (with endnotes `?fussnoten=endnoten`). Unknown slug, unknown
  or unsafe file (`..`, `/` at the start) → -32602; work without a section → -32000.

### epub.check, epub.checker_get, epub.checker_set (R20c)

- `epub.check {work, readers?: [slug], epubcheck?: bool = false, footnotes?: "popup"|"endnotes" = "popup"}` →
  `{findings: [Finding], xml_tool: "xmllint"|"quick_xml", epubcheck: EpubcheckStatus, summary: {errors, warnings,
  infos}, readers: [slug]}`. Builds the EPUB like `work.epub` (no commit) and checks it: own structure and
  accessibility check (`baruch_epub::check::check_structure`, xmllint from PATH with `--noout --nonet`, otherwise
  quick-xml), warnings of the build (e.g. `ISLAND_OMITTED`, `TEMPLATE_PAGEBREAK_MAPPED` as a note) and per reader
  (`reader_warnings`). Without `readers` the required profiles; unknown slug → -32602; without sections -32000.
  - `Finding = {code, severity: "error"|"warning"|"info", message, path: string|null, reader: slug|null}`. Codes of the
    own check include `MIMETYPE_NOT_FIRST`, `MIMETYPE_COMPRESSED`, `CONTAINER_MISSING`, `MANIFEST_FILE_MISSING`,
    `FILE_NOT_IN_MANIFEST`, `SPINE_IDREF_UNKNOWN`, `XHTML_NOT_WELLFORMED`, `NAV_MISSING`, `LANGUAGE_MISSING`,
    `IMG_ALT_MISSING` (errors), `IMG_ALT_EMPTY`, `NCX_MISSING`, `A11Y_METADATA_MISSING` (notes),
    `SPINE_NAV_ORDER`, `XHTML_TITLE_MISSING` (warnings); complete in `baruch_epub::check::codes`. Reader:
    `READER_POPUP_FOOTNOTES`, `READER_PAGE_BREAKS`, `READER_EMBEDDED_FONTS`, `READER_CSS_BACKGROUNDS`, `READER_SVG`,
    `READER_HYPHENATION`, `READER_PUBLISHER_FONT` (only if the work sets a font; the base style does not
    count), `READER_NIGHT_MODE`, `READER_CONVERSION`. EPUBCheck:
    `EPUBCHECK_<Code>` (e.g. `EPUBCHECK_RSC-005`), path in the EPUB.
  - `EpubcheckStatus = {status: "not_requested"} | {status: "not_found"} | {status: "ran", tool: {kind:
    "binary"|"jar", path}, version: string|null, ok: bool} | {status: "failed", message}`. With `epubcheck: true`
    Baruch looks for `epubcheck` in PATH, otherwise the configured epubcheck.jar (`java -jar`); at most one run at a time,
    time limit 180 s (then the program is ended together with its process group, `failed`). Never a download.
- `epub.checker_get {}` → `{jar: string|null, found: {kind: "binary"|"jar", path}|null, java: bool}` (without a work).
- `epub.checker_set {jar: string|null}` → like `checker_get`. `jar`: absolute path, ending `.jar`, an existing
  regular file, no control characters, at most 4096 bytes – otherwise -32602, nothing stored; `null` (or empty)
  removes the setting (`store_meta.epubcheck_jar`). No commit (no work). Only RPC and the form
  `POST /einstellungen/daten/epubcheck`, no MCP.
- **Leftovers of book details (R20c):** `work.meta.set` refuses an ISBN-13 without the prefix 978/979 with -32602 („Eine ISBN-13
  beginnt mit 978 oder 979 …“); the prefix „ISBN“, „ISBN-13“, „isbn-13“, „ISBN 13“ in any spelling may stand before it;
  an ISBN only written differently (hyphens, prefix) is a change and is stored as entered
  (commit „Buchangaben geändert: ISBN“). `work.meta.get` delivers `paper_sheet_mm` (thickness per sheet = 2 pages);
  `work.meta.set` takes `paper_sheet_mm` (above 0 up to 2 mm, stored as `paper_mm` = half, no migration;
  together with `paper_mm` → -32602). For the paper thickness the commit body names both („Papierdicke: 0.1 → 0.075 mm je
  Seite (je Blatt 0.2 → 0.15 mm)“). `asset.delete` without `rev` on the cover image first removes the cover with
  its own commit „Umschlag entfernt“ and reports `cover_removed: true` (plus `cover_commit` or `cover_mirror`), then
  the image; likewise the form `POST /werke/bilder/act` (`action=remove`) and a confirmed delete request.
  `asset.request_decide` with `confirm` on a cover request writes exactly **one** commit „Bildantrag <n>
  bestätigt: …“ (body with „Umschlag vorher/nachher“); `cover` in the answer is `{work, cover, commit?}`.

### Integration

No RPC change through the integration. The acceptance test `tests/round20_abnahme.rs` uses `work.epub` (`inline`),
`epub.preview` (all profiles), `asset.add`, `work.set_cover`, `work.meta.set`, `work.typeset_set` and
`revision.accept` (cross-reference); `epub.check` through `epub_check::run_check` with its own PATH content.

## Round 31 – Index and glossary

### Interface R31-S1

New methods (contract §3.1/§3.2); known in the interface but refused with REFUSED „noch nicht umgesetzt (R31 …)“,
since 31a/31b all implemented:

- glossar-daten: `glossary.list`, `glossary.get`, `glossary.lookup`, `glossary.add`, `glossary.update`,
  `glossary.delete`, `glossary.decide`, `glossary.suggest`, `index.registers`, `index.register_add`,
  `index.register_rename`, `index.register_delete`, `index.proposals`, `index.decide`.
- register-satz: `index.entries`, `index.mark`, `index.unmark`, `index.overview`.

The new `StoreError` variants (`Glossary`, `UnknownGlossaryTerm`, `UnknownIndexProposal`) come as INVALID_PARAMS.
`work.typeset_get`/`work.typeset_set` carry the new fields `glossary_page`, `index_pages {registers, columns}` and
`glossary_first_mention` through the JSON of the typesetting settings (check: at least one index, valid and
unique identifiers, at most 16, 1–3 columns).

### 31a glossar-daten

Implemented (`src/glossary.rs`, `src/assist/glossary.rs`). Work through `work` (missing → default work). Term =
`{id, uuid, term, short, long, variants, source, bible_refs, status: "accepted"|"proposed", origin, created, updated,
updated_by, anchor}` (`anchor` = `baruch-gl-<uuid>`). Writing methods: exactly one mirror commit
(`commit` in the answer; if the mirror fails, `mirror` with the message, the change holds nevertheless).

| Method | Parameters | Result | Commit (`Baruch-Action`) |
|---|---|---|---|
| `glossary.list` | `{work?, status?: "accepted"\|"proposed"\|"all"}` (default `all`) | `{work, terms: [Term]}` by `sort_key` | – |
| `glossary.get` | `{work?, id}` | `{work, term}` | – |
| `glossary.lookup` | `{work?, word}` | `{work, word, terms}` – only accepted ones, by term or variant | – |
| `glossary.add` | `{work?, term, short?, long?, variants?, source?, bible_refs?}` | `{work, term, commit?}` | `glossary-add` (author human) |
| `glossary.update` | `{work?, id, term, …}` like add | `{work, term, commit?}` | `glossary-update` (body before/after) |
| `glossary.delete` | `{work?, id, confirm: true}` | `{work, deleted: id, commit?}` | `glossary-delete` (body with the term) |
| `glossary.decide` | `{work?, id, accept}` | `{work, term}` or `{work, deleted}` | `glossary-accept`/`glossary-reject` (`Baruch-Proposer`) |
| `glossary.suggest` | `{work?, key?, persona?}` | `{work, persona, model, proposed: [Term], skipped: [{term, reason}], notes}` | per proposal `glossary-propose` (author the persona) |
| `index.registers` | `{work?}` | `{work, registers: [{slug, title, kind, builtin}]}` | – |
| `index.register_add` | `{work?, slug, title}` | `{work, register: {slug, title, uuid, position, created}, commit?}` | `register-add` |
| `index.register_rename` | `{work?, slug, title}` | like add | `register-rename` |
| `index.register_delete` | `{work?, slug, confirm: true}` | like add, plus `deleted: slug` | `register-delete` |
| `index.proposals` | `{work?, status?: "open"\|"all"}` (default `open`) | `{work, proposals: [{id, key, based_on, word, occurrence, mark, reason, origin, status, created, decided, decided_by, url}]}` | – |
| `index.decide` | `{work?, id, accept}` | accept: `{work, proposal, key, rev, marks, commit?}`; reject: `{work, proposal, commit?}` | `index-accept` (new revision of the human, `Baruch-Node`, `Baruch-Revision`, `Baruch-Proposer`) or `index-reject` |

- `variants` and `bible_refs` as a list of texts (a single text counts as a list with one line per entry).
  Bible references are checked and normalised with `baruch_scripture::parse` (several with `;` become separate entries);
  unreadable ones → INVALID_PARAMS with a message, nothing stored.
- Errors: limits, duplicates (`term_key` among accepted ones), built-in/invalid index identifier, unknown term
  or proposal, already decided proposal, word no longer in the text (proposal becomes `stale`) → INVALID_PARAMS;
  `glossary.delete`/`index.register_delete` without `confirm: true` → INVALID_PARAMS; errors of the model with
  `glossary.suggest` (no backend, no answer in the form "Term | explanation") → REFUSED.
- `index.decide` with `accept: true` sets the mark with `baruch_typst::index::mark_word`; if the word no longer stands in the
  text or the chosen occurrence already carries the same mark → INVALID_PARAMS with a message, the proposal becomes
  `stale`, no new revision.
- `work.exchange_preview`/`work.exchange_import`: `add` or `added` additionally count `registers` and
  `glossary_terms`; deviations of the kinds `register` and `glossar-begriff` stand in `deviations`.

### 31b register-satz

Errors are INVALID_PARAMS with a German message (section or revision unknown, word missing or not standing in the
section outside headings, code, comments and marks, invalid mark, unknown index, no
mark range).

- `index.entries {work?, key?}` → `{work, entries:[{key, register, term, sub, see, also, auto, start, end}]}`: the
  marks of the current revisions (`start`/`end` = byte range of the mark in the stored text, for `index.unmark`);
  if the index `bibel` is switched on in the index pages, also the automatic Bible references of the projection
  (`auto: true`, `term` = book, `sub` = chapter/verse, `start`/`end` = null). Unknown `key` → INVALID_PARAMS.
- `index.mark {work?, key, rev, word, occurrence?, register?, term?, sub?, see?, also?}` → `{work, key, node, rev,
  marks, commit?|mirror?}`: sets the mark directly after the word (`occurrence` = `"first"` (default), `"all"` or
  a number from 1; `register` default `stichwort`, must exist in the work; `term` default = `word`) and stores
  a new revision of the human on `rev` (one mirror commit). `marks` = number of marks set. The same mark
  is never set twice: occurrences after which it already stands (also in a chain of several marks) stay;
  if the chosen occurrence is already marked or, with `"all"`, none is open any more → INVALID_PARAMS „… trägt an der
  gewählten Fundstelle schon diese Registermarke; nichts geändert …“, no new revision.
- `index.unmark {work?, key, rev, start, end}` → `{work, key, node, rev, commit?|mirror?}`: new revision without the mark
  exactly in this range.
- `index.overview {work?}` → `{work, registers:[{slug, title, kind, terms:[{term, sub, keys, see, also, auto}]}]}`:
  all indexes of the work, entries sorted as on the index pages (DIN 5007-1, Bible in biblical order),
  `keys` = sections with occurrences.

`work.export` carries, with index pages or a glossary page, the back matter from the line `// Baruch: Register und Glossar`;
`work.epub` the files `text/glossary.xhtml` and `text/index-{slug}.xhtml`.

### 31c kontextmenue

No new RPC method. The context menu (`web/src/context_menu.ts`) calls `glossary.add` („In Glossar aufnehmen“),
`glossary.lookup` („Im Glossar nachschlagen“) and `index.registers` (choice of the index) from 31a; it does not set index marks
over RPC but in the editor (typeset view or text field); saving goes as always through `outline.save`.

- `wysiwyg.parse`: an index mark appears in `html` as `span.ws-index-mark` (fields `data-register`, `data-term`,
  `data-sub`, `data-see`, `data-also`, `data-sort`, description in `title`/`aria-label`, `data-raw` = canonical form),
  in `blocks` as the inline `{"t": "index_mark", "mark": {…}}`.
- `wysiwyg.serialize`: takes `{"t": "index_mark", "mark": {"register", "term", "sub"?, "see"?, "also"?, "sort"?}}` in
  newly generated blocks and writes `mark_source`; unknown fields of the mark → `INVALID_PARAMS`.

Forms without JavaScript (no RPC, `docs/Bedienung.md`): `POST /werke/{id}/glossar/act` (`action` =
add|update|delete|accept|reject|suggest; `suggest` with optional `key` calls `glossary.suggest` and redirects to
`/werke/{id}/glossar?ok=suggest&neu=N&uebersprungen=M#vorschlaege`, errors of the model 422 with a message) and `POST /werke/{id}/register/act` (`action` =
mark|unmark|accept|reject|register_add|register_rename|register_delete) → 303 with `ok=<action>`, errors 422 with the
input; typesetting settings through `POST /einstellungen/werk/satz` with the new fields `glossary_page`
(`before_indexes|after_indexes`, empty = none), `index_register` (repeatable, typesetting order), `index_columns` (1–3, empty
= 2) and `glossary_first_mention` (`on|off`, empty = not specified).

### Integration

- No new method. `work.import` and the command line `import` take the back matter (from
  `// Baruch: Register und Glossar`) and the additions of the projection (automatic Bible reference marks, linked
  first mention) out of an export with back matter before the sections are created (`baruch_typst::index::strip_back_matter`,
  `strip_projection`).
- MCP `initialize` → `instructions` name the three glossary tools (proposals only).

## Round 22 – Revisions and mirror

Amending instead of a new revision, history and undo, setting „Fassungen“, follow-up questions on done findings,
archive, draft as watermark, exchange sub-version 8. Migration 0050 (`0050_revision_amend.sql`).

### fassungen

#### Methods (`revision.save` and `outline.save` also stand in the table above)

| Method | Parameters | Result |
|---|---|---|
| `revision.save` | `{node, body, based_on?, based_on_amend?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, commit?, mirror?}` – as before; Round 22: `mode` `"auto"` amends the author's own current revision if no rule requires a new revision (see "Amending"), `"new"` always creates a new revision; **if `mode` is missing, `"new"` applies** (all old clients unchanged); unknown value → -32602. `saved`: `"amended"` (with `amend_seq` = new state), `"new"` (in mode `"auto"` with `reason` and `reason_label`) or `"unchanged"` |
| `outline.save` | `{node, body, based_on?, based_on_amend?, split?, merge?, mode?}` | `{node, rev, saved, amend_seq?, reason?, reason_label?, outline?, commit?, mirror?}` – as before; `mode` as with `revision.save`. Splitting, merging, a new heading or level always yield a new revision (`reason: "heading_changed"`) |
| `revision.amends` | `{node\|key, rev}` | `{node, key, rev, amend_seq, steps: [{uuid, seq, kind, undo_of?, author, created, body_before, body_after}]}` – history of the amendments of the revision `rev` by `seq` (read only); `kind` `"amend"` (amendment by the human) or `"undo"` (taking back through the history, `undo_of` = step whose previous state was restored); unknown revision → -32602 as with the other revision methods |
| `revision.amend_undo` | `{node\|key, rev, seq}` | `{node, key, rev, amend_seq, commit?, mirror?}` – sets the text of the revision to the state **before** step `seq` (`body_before`) – as a **new** step (`kind: "undo"`, `undo_of: seq`), never by deleting history rows; only on the current revision of the human, otherwise -32000 with a German message („… nur auf der aktuellen Fassung …“ or „… nur auf einer Fassung des Menschen …“); unknown step or same state → -32000. Human only: not over `/act`, not over MCP (form `POST /werke/fassungen/act`) |
| `revision.policy_get` | `{work?}` | `{work, amend, max_change_percent, new_after_minutes, defaults: {amend, max_change_percent, new_after_minutes}, limits: {max_change_percent: [1, 100], new_after_minutes: [0, 1440]}}` – setting „Fassungen“ of the work (if missing, the default: on, 100 %, 0 min – since 2026-11-27 without a size and time limit, saving always amends) |
| `revision.policy_set` | `{work?, amend?, max_change_percent?, new_after_minutes?}` | `{work, changed: [Feld…], commit?, mirror?}` – sets the setting (missing fields stay; booleans also `"1"`/`"0"`, numbers also as text); limits 1–100 % or 0–1440 min (0 = no time limit) → otherwise -32602 with a message; only on a change a commit „Fassungen: <Felder> geändert“ (`Baruch-Action: revision-policy-set`). Human only: not over `/act`, not over MCP (form `POST /einstellungen/werk/fassungen`) |

`revision.tree` and `node.get`: every revision (`RevisionInfo`) carries `amend_seq` (state, 0 = never amended)
and, for proposals created on an amended base, `based_on_amend` (state of the base).

#### Amending

If the human saves in the editor (mode `"auto"`) on their current revision, it is **amended**: text
replaced, `amend_seq` + 1, a row in `revision_amend` (text before and after, human, time, uuid). Number,
origin, status, `based_on` and `edited` stay (`edited` is set only by `revision.update`). A **new revision**
is created instead on the first applicable reason (`reason`, fixed order, `baruch_core::revision_policy::decide`):

| `reason` | `reason_label` | when |
|---|---|---|
| `requested` | „als neue Fassung gewünscht“ | mode `"new"` (then without `reason` in the answer) |
| `no_current` | „keine aktuelle Fassung“ | section without a current revision |
| `policy_off` | „Fortschreiben ausgeschaltet“ | setting „Fassungen“: off |
| `not_own` | „aktuelle Fassung stammt nicht vom Menschen“ | revision of a persona/an MCP client, also every accepted proposal |
| `not_on_current` | „nicht auf der aktuellen Fassung bearbeitet“ | `based_on` ≠ current revision, or `based_on_amend` ≠ state of the current revision (amended elsewhere in the meantime, e.g. in a second tab) |
| `heading_changed` | „Überschrift oder Gliederung geändert“ | title, level, splitting, merging, title of a part |
| `large_change` | „große Änderung“ | `change_percent` > `max_change_percent` |
| `time_elapsed` | „Zeitgrenze überschritten“ | `new_after_minutes` > 0 and since the last write (max. of the creation of the revision and the last amendment) at least that many minutes |

`change_percent(alt, neu) = (1 − similar::TextDiff::from_words(alt, neu).ratio()) · 100`. Note: the word boundaries
(white space) count as equal parts; an entirely new sentence with the same number of words therefore lies at about
50 %, a limit of 20 % (the default until 2026-11-26) corresponds roughly to a third to half of the words changed.

Only saving from the editor amends (`outline.save`/`revision.save` with `mode: "auto"`). Inserting an image,
index marks, converting a reference, replacing and outline actions still create new revisions. Personas and
MCP always create proposals. `add_proposal`/`add_proposal_on` remember the state of the base (`based_on_amend`).

#### Git mirror and side branches

Every amendment and every undo is exactly one commit (even with the same tree): subject
„<Nummer> <Titel>: Fassung n fortgeschrieben (Stand k)“ or „…: Fortschreibung von Fassung n zurückgenommen
(Stand k)“, trailers `Baruch-Action: amended` or `amend-undone` and `Baruch-Amend: k`. The setting „Fassungen“
is an empty log commit „Fassungen: <Felder> geändert“ with before → after in the body
(`Baruch-Action: revision-policy-set`).

**Side branches (Round 22):** every non-current revision (replaced, discarded, open proposal with its variants)
stands as a reference `refs/revisions/<key>/<n>` in the mirror (key = fixed key of the section, n =
revision number): a commit with the snapshot of the work in which the section carries this revision (section file
with the import line of the work template, label `<kN>` and image paths from the root as in HEAD, without the switch „Nummer an/aus“ and without
the index projection), parent = HEAD at creation; author as with the revision commits, trailers `Baruch-Node`,
`Baruch-Key`, `Baruch-Revision`, `Baruch-Side` (`superseded`|`rejected`|`proposed`) and for personas `Persona`/`Model`.
Updated after every revision commit (saving, amending, switching, changing, deleting, outline, exchange),
after `revision.reject` and at start; an unchanged revision keeps its commit, a deleted one loses its
reference. Working folder, index and HEAD stay untouched. Reading: `git log refs/revisions/<key>/<n>` in the
mirror folder. A new proposal itself is not a commit; the server creates its reference right afterwards
(`mirror::side_refs_after_proposal`, integrator).

#### Routes (forms without JavaScript)

- `GET /werke/{id}/fassungen/verlauf?key=K&rev=R` – history of a revision, newest steps first, per step the
  word difference (before → after) and, on the current revision of the human, „Bis hier zurück“; `ok=1` shows the
  notice after an undo.
- `POST /werke/fassungen/act` – `method=revision.amend_undo`, `work`, `key`, `rev`, `seq` → 303 to the history with
  `ok=1`; refused: history with a message (409/422).
- `POST /einstellungen/werk/fassungen` – `work`, `amend` (checkbox, missing = off), `max_change_percent`,
  `new_after_minutes` → 303 to `/einstellungen/werk?work=W&ok=fassungen#fassungen`, refused
  `…&fassungenfehler=1#fassungen` (nothing changed).
- `POST /gliederung/act` with `method=outline.save`: takes `mode` (the editor sends `auto`) and `as_new=1` (button
  „Als neue Fassung speichern“ → `mode` `"new"`); the redirect carries `saved=amended-<k>` or
  `saved=new-<n>[-<grund>]`, from which the editor head shows the feedback.
- `/act` still takes `revision.save` with `mode`; it refuses `revision.amend_undo` and `revision.policy_set`.

#### Hiding older revisions (0.9.0, user 2026-11-24)

Per section at most one **mark** „bis hierhin“ (number of a revision, table `node_revision_fold`, migration
0051). Hidden are **replaced and discarded** revisions with a smaller number; **never** the current revision and
**never** open proposals (older ones neither; they wait for a decision). If the mark lies above the current
revision (because a switch to an older one happened later), it applies only up to below the current one (`cutoff`); it can**not
be set** above the current one (-32000). Nothing is deleted: revisions, history and mirror stay
unchanged, no commit in the mirror (no book text); logged in the database (`updated`, `updated_by`) like the
setting „Fassungen“; not in the exchange format. Only the human sets and removes it – over RPC and form, **not
over MCP** (no tool). Rule in `baruch_core::revision_fold`, methods in `src/revision_fold.rs`.

| Method | Parameters | Result |
|---|---|---|
| `revision.fold_get` | `{node\|key}` | `{node, key, below, cutoff, hidden, updated?, updated_by?}` – `below` = stored mark (`null` = none), `cutoff` = effective limit (at most the current revision), `hidden` = number of hidden revisions (read only) |
| `revision.fold_set` | `{node\|key, rev}` | `{node, key, rev, below, cutoff, hidden, updated, updated_by}` – sets the mark at revision `rev` (replaces an existing one); unknown revision → -32602 as with the other revision methods, above the current revision → -32000 („Über der aktuellen Fassung lässt sich nicht ausblenden …“). Also over `/act` |
| `revision.fold_clear` | `{node\|key, rev?}` | `{node, key, cleared, below: null, cutoff: null, hidden: 0, rev?}` – removes the mark („Wieder alle zeigen“); `cleared` = there was one; `rev` is only passed through for the jump back from `/act`. Also over `/act` |
| `revision.fold_before` | `{work?, date}` | `{work, date, sections: [{key, below}]}` – work-wide: for every section the mark on the last revision created before `date` (day `JJJJ-MM-TT`, midnight local time), at most on the current one; sections without such a revision stay unchanged, existing marks are replaced; one step. Invalid day → -32602 |
| `revision.fold_clear_all` | `{work?}` | `{work, cleared}` – removes all marks of the work (number) |

View: on the work page, paging ‹ › skips hidden revisions, the counter shows „2/3 (+ 24
ausgeblendet)“. The switch stands in the address: `&fassungen=alle` (after `lq`) shows all, also when paging;
`ui.render` takes `fassungen` like the other fields of the address. A hidden revision called directly through `?rev=`
opens the switch by itself.

Routes (without JavaScript):

- `/act` takes `revision.fold_set` and `revision.fold_clear` (`work`, `key`, `rev`) → 303 to the revision.
- `POST /einstellungen/werk/fassungen/ausblenden` – `work`, `date` and `do=fold` (default) or `do=clear` (remove all
  marks) → 303 to `/einstellungen/werk?work=W&ok=ausgeblendet#fassungen` or `ok=eingeblendet`;
  refused `…&ausblendenfehler=1#fassungen` (nothing changed).

### befund-rueckfrage

#### Works: deleting, archive, restoring

| Method | Parameters | Answer |
|---|---|---|
| `work.delete` | `{work, confirm_title}` | as before, since Round 22 additionally `ideas_commit?` or `ideas_mirror?`: afterwards the server updates the mirror of the ideas without a work (`<data>/einfaelle/`) – the ideas of the deleted work are there at once; without a change no commit |
| `work.archives` | `{}` | `{archives: [{slug, title, deleted, folder?, restorable, reason?}]}` – deleted works from `work_retired`, newest first (read only). `folder` = archive folder `works/.geloescht/<slug>-<JJJJ-MM-TT>[-n]` relative to the data folder (with several the youngest; a work whose slug only begins with `<slug>-` does not match); `restorable: false` with a German `reason` if the folder or `main.typ` is missing or a living work carries the same title |
| `work.restore` | `{slug}` | `{work, slug, title, nodes, folder, skipped_images, commit?\|mirror?}` – restores an archived work (human only; **not** over `/act` and MCP). Under the write lock the server reads the archive folder (no symbolic links – neither folder nor `abschnitte/` nor files –, only `main.typ`, `vorlage.typ` and the included `abschnitte/<name>.typ`, together at most 16 MiB, at most 10 000 sections), assembles the Typst text like an export (every line `#include "abschnitte/…"` replaced by the text of the file without the line `#import "../vorlage.typ": *`, without a work template without the header line `// Baruch: <Titel>`; back matter and projection removed as on import) and creates the work with the reserved slug (`Store::restore_retired`, one transaction): title from `work_retired`, work template as revision 1, every heading an outline item with revision 1 by the human (current, note „aus dem Archiv wiederhergestellt“), a rule block „// Baruch: Satz“ becomes typesetting settings again, the row in `work_retired` is dropped; the work gets a new id and the kind „Buch“ (`work_retired` does not know the kind). Afterwards the folder moves back to `works/<slug>` and gets **one** commit „Werk wiederhergestellt: <Titel>“ (`Baruch-Action: work-restored`, even with the same state). `skipped_images` = images under `bilder/` of the archive that do **not** come along. Errors: `slug` missing → -32602; unknown slug („steht nicht im Archiv“), no archive folder, symbolic link, foreign `#include`, too large, `works/<slug>` already exists, title taken, the same source already imported as a work → -32000 with a German message, nothing changed; no heading → -32602. An error when moving or in the mirror does not stop the work in the database (`mirror`) |

Without JavaScript: page `GET /werke/archiv` (list with archive folder and one form per work) and
`POST /werke/archiv/wiederherstellen` (fields `slug`, `confirm` = `1`; behind the same protection as `/werke/act`):
success → 303 `/?work=<id>`; without confirmation → 422, errors → 409/422, each the archive page with a message
(`role="alert"`).

#### Findings: state and follow-up question

- `Finding` and `FindingStatusEntry` carry since Round 22 `adopted_amend: number | null` (migration 0050): the
  amendment state (`revision.amend_seq`) of the adopted revision. `finding.set_status` with `adopted`
  remembers the state of the revision at this moment (finding and history entry); a proposal „übernommen“ of an
  assistant remembers in the entry the state at the time of the proposal, `finding.decide` with `confirm: true` sets finding **and**
  entry to the state at confirmation. Every other status sets `finding.adopted_amend` to `null` (the history
  keeps its values). Display „Fassung n · Stand k“ only for k > 0 (card, list, history per chapter of the matrix,
  `befunde.md` in the mirror: „- Übernommen in Fassung 1 · Stand 3“); state 0 or `null` shows only „Fassung n“ as before.
- `finding.matrix`: `history[].revisions[]` (`AdoptedIn`) additionally has `stands: [number]` – per finding in `findings`
  (same order) the remembered state, 0 = none.
- Follow-up question of a persona („Befund n scheint erledigt?“, see Bedienung): an open proposal `done` of an
  assistant with `note` `„Befund n scheint erledigt (Stelle „…“) – als erledigt markieren?“` and `reason` `„laut
  @persona umgesetzt (Stelle „…“)“`. It stands in `finding.list`/`finding.get` as `pending`; it is decided with
  `finding.decide {entry, confirm: true|false}` („Ja, erledigt“ / „Nein, noch offen“). No new RPC names.

#### Personas: re-checking (`persona.review`, `persona.chat`, `post.repeat`)

Re-checking (`persona.review`, `post.repeat`, RFC through `persona.chat`, MCP `ask_persona` with `mode: "review"`)
passes the persona its own open findings of the section (see Architektur). If it reports „Erledigt: Befund
n“, a status proposal `done` is created instead of a new finding (exactly one commit `Baruch-Action:
finding-propose`, trailer `Persona`). The result of `assist::run` (`Outcome`) additionally carries `done_proposals` (ids with
a new proposal), `done_notes` (skipped reports with a reason) and `all_done`; `persona.ask`/`persona.review`
output the three fields in their answer as soon as there was a report (otherwise they are missing as before).
`persona.chat` and MCP `ask_persona` do not output them separately (open after Round 22). `post.repeat` does not propose a
finding with an open follow-up question as „erledigt“ once more (integrator); an open proposal of another kind
does not stop it. Reports on findings that were not in the task (another section, no re-check)
are skipped by Baruch with a note in `done_notes` (PM close R22).

### spiegel-entwurf

#### Draft (watermark) on the outputs – Round 22

Parameter `draft?: bool` (three-valued) on `work.pdf`, `notes.pdf`, `work.epub`, `work.cover_pdf` and `preview.typst`:

| `draft` | Effect |
| --- | --- |
| missing / `null` | according to the typesetting setting `draft` (cover PDF only with `draft.cover`) |
| `true` | with watermark: stored values, otherwise default („ENTWURF“, 12 %, diagonal) |
| `false` | without watermark (source and PDF byte-identical to the work without the setting) |

No way changes the stored setting. A value other than `true`/`false`/`null` → -32602 „draft: true (mit
Wasserzeichen), false (ohne) oder weglassen (nach Einstellung)“.

- `work.pdf {work, inline?, draft?}` – answer as before; with `draft`, `url` carries the switch
  (`/werke/{id}/pdf?entwurf=1|0`).
- `notes.pdf {work, format?, font_pt?, inline?, draft?}` – `url` with `&entwurf=1|0`.
- `work.epub {work, inline?, footnotes?, draft?}` – with draft: title page with a note („Entwurf: <Text> · <Zusatz>“;
  without front matter a title page is created for it), `dc:title` and `docTitle` „<Titel> (Entwurf)“; `url` with `?entwurf=…`.
- `work.cover_pdf {work, inline?, draft?}` – additionally `draft: bool` (the PDF carries a watermark); `checks`
  then contains the warning `{code: "draft", block: null, message: "Das Umschlag-PDF trägt das Wasserzeichen „Entwurf“ –
  so nicht an die Druckerei geben …"}`.
- `preview.typst {work?, key?, page?, draft?}` – additionally `draft: bool` in the answer (typesetting source carries the
  watermark); `stand` differs per switch.
- `work.typeset_get/set/reset/preview` carry the field `settings.draft` (`{text, note?, date, opacity_percent,
  placement: "diagonal"|"header"|"footer", cover}`), checked in `TypesetSettings::validate` (text trimmed 1–40 characters
  without control characters, addition trimmed ≤ 120 without control characters, opacity 5–50 %; messages „Entwurf: …“, fields
  `draft.text`, `draft.note`, `draft.opacity_percent`). `rules` stays without a line for the draft. Mirror commit
  „Satz: Entwurf geändert“ with `„Entwurf: aus → an: „<Text>“, <Lage>“` in the body.

Routes (GET, parameter `?entwurf=1|0`, missing = according to the setting; another value → 400): `/werke/{id}/pdf`,
`/werke/{id}/stichwortzettel.pdf`, `/werke/{id}/epub`, `/werke/{id}/umschlag.pdf`, `/werke/{id}/satz` (there also on the
page links and `…/satz/{stand}/{n}.svg?entwurf=…`; an unreadable value counts as „fehlt“ on the page).

#### Exchange sub-version 8 – Round 22

`work.exchange_export` writes sub-version 8 (`docs/Austauschformat.md`, section on amendments, draft and
state). `work.exchange_preview`/`work.exchange_import`: `added.amends` (amendments, also when fast-forwarding),
deviation „Fassung n: vorgespult von Stand k auf Stand m“ (note); a new part that would take over chapters becomes a
request `add` with identifier and revisions (deviation „… Antrag mit Kennung und n Fassung(en)“).
`outline.requests`/`outline.request_decide`: the summary of such a request reads `„Teil „…“ (mit Kennung und
n Fassungen) vor „…“ anlegen; … wandern unter den neuen Teil“`; approving creates the part with this identifier and these
revisions (one mirror commit as before).

### Integration

- `node.get`: every revision carries, as in `revision.tree`, `amend_seq` and, for proposals on an amended base,
  `based_on_amend`.
- Every new proposal (personas, MCP `propose_revision`, quotations from literature and Bible, variants, solutions) creates
  its side branch `refs/revisions/<key>/<n>` right away, without its own commit.
- `post.repeat`: `proposed_done`/`notes` skip findings for which the persona already reported
  „Erledigt: Befund n“ in the same answer (open follow-up question).
- Re-import of an export with the block „Überschriften ohne Nummer über die Sprungmarke“: the block disappears from
  the template, the named sections come back „ohne Nummer“.

## Round 23 – Hardening

No new methods; changed parameters, answers and errors. The tables above each name the change briefly
and refer here.

### web

**`revision.update {node|key, rev, body?, note?, based_on_amend?}` → `{node, rev, changed, amend_seq?, mirror?}`**
(changed, Round 23a). If the **text** of the current revision or of an already amended one (state > 0) changes,
changing through the tree creates a step in the history (`revision_amend`, kind `amend`, text before and after) and
increases the state; the answer names it in `amend_seq`. A never amended, non-current revision (e.g. a
proposal) gets no history as before, nor does a mere change of the note. New is the optional parameter
`based_on_amend` (state the client had loaded): if the revision now stands on another state,
nothing is changed and the method reports -32000 „Veraltet: Fassung n steht inzwischen auf Stand k (geladen war Stand j); bitte
neu laden und die Änderung dort eintragen.“ (`StoreError::Stale`). Without the parameter nothing is checked (old clients).
The form „Fassung ändern“ sends it along.

**`revision.save` / `outline.save`** (changed): `based_on_amend` also counts without `based_on` – the state then refers
to the current revision. Before, it was skipped in that case.

**`ui.render {work?, …}`** (changed): if `work` is missing, the method draws the work from the cookie `baruch_work` (like
the page `/`), only without a cookie or with an unknown work the default work. Before, „Zurück“ to the address `/`
showed the default work without a notice.

**`work.typeset_adopt_rule {work?, confirm: true}`** (changed): requires `confirm: true`, otherwise -32602 „confirm: true
erwartet“. The method rewrites the work template; the form already requires its checkbox for that. No widget calls it
(`grep -rn typeset_adopt_rule crates/baruch-server/web/src` without hits). `work.typeset_reset` stays unchanged.

**`idea.map_group_accept {work, group, name?}`** (changed): the group as the map now shows it is decisive.
A stored proposal whose group now has other ideas is refused without its own `name` with -32000
`„Veraltet: Die Gruppe gN hat inzwischen andere Einfälle als beim Vorschlag „…“; nichts geändert. …“` and
removed; no idea changes. The limits (name, at most 20 keywords per idea) are checked before the first
idea is changed.

**`idea.map`** (changed): `near.message` names a cap if the map shows more than 1500 ideas („… die Nähe
ist nur für die ersten 1500 (nach Nummer) berechnet.“).

**`cover.layout_set`, `asset.request_decide`** (wording unchanged): invalid blocks still report -32602 with
`„Umschlag-Block „…“ (art), Feld x: …“`; in the store it is now the error kind of its own `StoreError::CoverBlock`.
`cover.layout` delivers default blocks rounded to 0.01 mm.

**`commentary.lookup {ref, module?, work?}`** (changed): the reference is read in German versification and **mapped** into the
versification of the commentary; `entries[].ref` names the reference of the commentary, with a differing versification with
its name („Joel 2,28 (KJV)“). Before, the German numbers were looked up unchanged in the commentary.

**`source.read {…, q?}`** (changed): `q` at most 500 characters, otherwise -32602 „q: höchstens 500 Zeichen (die Anfrage
dient nur dem Markieren der Fundstellen).“

**`finding.get`, `finding.list`** (added): a finding carries `stale_summary: true` if it is an open collective finding
whose sources are all decided (the field is missing otherwise). `finding.matrix`: an open collective finding does not count under
`open` (neither in the cells nor in `open_findings`).

**`menu.badges`** (unchanged): `open_findings` no longer counts collective findings (counting rule below).

### mcp-namen

**`mcp_token.create`:** Since Round 23 the method additionally checks the author that results from the name (`@mcp:<handle>`, rule `mcp::session::handle_name`: trimmed, lower case, `ä` → `ae`, `ö` → `oe`, `ü` → `ue`, `ß` → `ss`, accents removed, otherwise everything outside `[a-z0-9._-]` as „-“, at most 32 characters). Refused with **-32000**, and nothing is created: a name whose handle or old handle (rule until Round 22: umlaut → „-“) equals the handle or old handle of an **existing or revoked** token, and a name whose handle begins with `lokal-` (reserved for clients without a token). A revoked name is thus not free again. Check and creation run in one transaction: of two simultaneous requests with the same handle exactly one token is created. The form `POST /einstellungen/mcp/act` goes the same way (422 with the same message). Existing tokens keep their names and posts; no author is rewritten in the database.

**`persona.chat`, answer:** plus `done_proposals` (list of the finding numbers for which a report „Erledigt: Befund n“ produced a status proposal „erledigt“, without duplicates), `done_notes` (list of the skipped reports with a reason) and `all_done` (`true` if at least one answering persona says that all its findings on the section seem done). Names as with `persona.ask`; each field stands in the answer only if it is not empty or is `true`. In the chat such reports exist only with the RFC (there every persona checks again).

**`persona.chat`, recognition of the message:** lines that begin – after white space – with „>“ are quotations and count neither for mentions and addresses nor for RFC („@alle“, „RFC“), writing task or variant task. With a writing task, status proposals „übernommen in Fassung n“ are created only for `[B<id>]` from the **message**, no longer from the text of the proposal.

**`post.move`:** the findings from the post (`finding.post_id`) move along into the target section in the same transaction, one commit. Exception: adopted findings stay at the old section because their status names a revision of that section.

**`post.repeat` and `thread.repeat`:** the stored task (`PostRequest`, JSON in `post.request`) additionally carries since Round 23 `web` (bool), `asker` (text) and `relay` (JSON `{persona, post, text, findings}` of the addressing answer). The repetition takes over all three: a relay post is asked again with the relay frame and stays a relay post (`relay_from`); a task of an MCP client keeps its origin and still triggers no web search. Rows without these fields (before Round 23) repeat as before: web search allowed, asker the human, no frame. No migration; the fields are `#[serde(default)]`.

**`outline.requests {preview: true}`:** `preview_error` of a stale request still reads „Veraltet: <Grund>“; in the store it is now `StoreError::Stale` (before `Heading`), over RPC still -32000. „Nicht mehr ausführbar: …“ stays `Heading`.

### geheimnisse

- `model.connection_delete`: parameters `{id, confirm: true}`. Without `confirm: true` (missing, `false`, text) →
  -32602 „confirm: true erwartet“, and nothing is deleted. Otherwise as before `{id, deleted: true, personas: [Handle]}`.
- `model.connection_delete_secret`: parameters `{id, confirm: true}`; without `confirm: true` → -32602 „confirm: true
  erwartet“, the key stays. Otherwise `{id, secret: "fehlt"}`.
- The forms `POST /einstellungen/modelle/act` (`action=delete`, `action=delete_secret`)
  still require the mandatory checkbox `confirm=1` and then call the same functions (`models::delete_connection`,
  `models::delete_secret`) – usable without JavaScript. No widget calls the two methods
  (`grep -rn "connection_delete" crates/baruch-server/web/src crates/baruch-server/static crates/baruch-server/templates`
  finds nothing).
- `model.context`: if the connection of the persona is external and the persona is not allowed, **no** request goes
  out; answer `{tokens: <Einstellung>, reported: null, warning: null, …}`.
- `model.list`: the cache (60 s) applies per (kind, address). Two connections of different kinds on the same
  address share neither list nor context length.
- `model.connection_check`, `connection.check`: if the remote side answers with a redirect (3xx), that is
  `ok: false` with „… leitet weiter (HTTP 30x); Baruch folgt Weiterleitungen nicht – bitte die endgültige Adresse
  eintragen.“ or „HTTP 30x von …: Weiterleitung. Baruch folgt ihr nicht …“. The target (`Location`) is not named.
- `work.create`: `module` at most 100 characters in one line, otherwise -32602 „module: Modulname mit höchstens 100
  Zeichen in einer Zeile erwartet“. Work and SWORD Bible are created in one transaction.
- `work.exchange_import` (and preview): -32602 „Austausch: Das Glossar hätte N Begriffe; höchstens 5000 je Werk
  (Vorschläge mitgezählt). Nichts eingelesen.“ or „… Register <kennung>: Titel hat N Zeichen (höchstens 80) …“.

### Integration

- **`POST /act`** (forms without JavaScript) passes `based_on_amend` on to `revision.update`: the form
  „Fassung ändern“ is refused with a stale state even without JavaScript („Veraltet: …“).
- **`POST /mcp`:** its own size limit `MCP_BODY_LIMIT` = 12 MiB (an image of 8 MiB as Base64 with its frame), above that
  413; `/rpc` keeps `works::RPC_BODY_LIMIT` (code review ki-2).
- Re-exports in `baruch_store`: `MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN` (200), `MAX_OPEN_INDEX_PROPOSALS_PER_WORK` (2000),
  `MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN` (50).

## Round 32 – Word and quitting

### docx_import.preview, docx_import.get, docx_import.apply, docx_import.discard (Round 32 – Word)

Global (without a work; switch in `rpc::call` through `docx_import::handles`, module `docx_import`). No MCP counterpart: an
import creates current revisions under the name of the human.

- **`docx_import.preview {file_name, data_base64, title?}`** – `data_base64` = file as Base64 (standard, with `=`);
  `file_name` ends in `.docx`. Reads the file and creates only a **draft**
  (`<data>/docx-drafts/<draft>/` with `draft.json` and `source.docx`; no work). Answer:
  `{draft, file_name, title, sections: [{level 0..3, numbered, title, chars, sample}], editors: [{name, handle,
  insertions, deletions, sections}], comments, footnotes, images, warnings: [{code, message}]}`.
  `title` = title under which it would be taken over (own `title`, otherwise title of the file, otherwise file name without the ending);
  `level` 0 = part; `sample` = start of the section as Typst (at most 160 characters); `editors[].sections` = number
  of the sections for which a proposal would be created; `handle` = `@word:<name>`.
  Errors: not a `.docx` name, broken Base64, larger than 64 MiB, errors of the file (not a ZIP, old `.doc` or
  encrypted, unsafe path or relationship target, DTD, limit exceeded, broken XML, no text) → -32602 with
  the reason; nothing is created. Over `POST /rpc` the whole request additionally lies within `RPC_BODY_LIMIT`
  (otherwise 413 before the call, so a file up to about 24 MiB – larger ones through the form
  `POST /werke/import/word/vorschau`, up to 64 MiB).
- **`docx_import.get {draft}`** → the same object as `preview`. Unknown, discarded, taken over or older than 24 h
  → -32602.
- **`docx_import.apply {draft, title?, changes?: bool = true, comments?: bool = true}`** – creates the work (revision 1
  per section = state before all changes, current), with `changes` per editor and changed section one
  open proposal (persona `@word:<name>`, model `word`) and per changed heading a post, with `comments`
  per comment a post. Answer: `{work, slug, sections, proposals, posts, warnings: [{code, message}]}`. The
  draft is deleted afterwards. Errors: unknown draft → -32602; taken title → -32000 (like
  `source_import.apply`, the draft stays); the same draft is being taken over right now → -32000. If a
  single proposal or post could not be created, the work stays and `warnings` names it
  (`docx_vorschlag_abgelehnt`, `docx_beitrag_abgelehnt`, `docx_gliederung_abweichend`).
- **`docx_import.discard {draft}`** → `{discarded: true}` (even if the draft is already gone). `draft` is not a
  32-digit hex name → -32602.

At most 20 drafts, each at most 24 hours old; older ones are dropped at the next `preview`.
`work.import` and `POST /werke/import` refuse `.docx` with a reference to `/werke/import/word` (S1).

Pages (without JavaScript): `GET /werke/import/word` (form; with `?draft=<id>` the preview, unknown draft 422),
`POST /werke/import/word/vorschau` (multipart `file`, `title` → 303 `/werke/import/word?draft=<id>`; errors 422, too
large 413, each with the form), `POST /werke/import/word/uebernehmen` (`draft`, `title`, checkboxes `changes`,
`comments` → 303 `/?work=<id>`; if new notes arose on taking over, 200 with a report and a link to the work; errors
422 with the preview), `POST /werke/import/word/verwerfen` (`draft` → 303 `/werke/import/word`). All POSTs behind
`guard::same_origin` (foreign origin 403).

### Round 32 – Word: `work.docx_export`

Read only; no MCP tool.

`work.docx_export {work?: number|string, proposals?: bool = false}`

- `work`: work id (number), folder name of the work (text) or – without it – the default work.
- `proposals`: `true` lays, per section, the youngest open proposal as tracked changes over the current
  revision.

Answer:

```json
{
  "file_name": "der-steg-am-weiher.docx",
  "data_base64": "UEsDB…",
  "bytes": 5321,
  "proposals": 1,
  "warnings": [
    { "code": "docx_typst_insel", "message": "Abschnitt „Der Abend“: Typst-Insel „#let rahmen(inhalt) = …“: Word kann sie nicht abbilden – der Quelltext steht sichtbar und wörtlich in der Datei." },
    { "code": "docx_weitere_vorschlaege", "message": "Abschnitt „Der Weiher“: 1 weiterer offener Vorschlag steht nicht in der Datei – als Änderungsverfolgung steht nur der jüngste (Fassung 3 von @kritiker)." }
  ]
}
```

- `proposals`: number of the sections whose proposal stands as tracked changes in the file (without `proposals: true`
  always 0; a proposal equal to the current revision does not count).
- `warnings`, fixed codes: `docx_typst_insel` (island with a visible replacement; from the sixth on only as a number),
  `docx_bild_fehlt` (image path without an embeddable image: missing in the work or SVG/WebP), `docx_tabelle_vereinfacht`
  (formatting of a table dropped), `docx_weitere_vorschlaege` (more than one open proposal in the section).
- Errors: unknown work or unknown parameter → `-32602`; write error → `-32603`.
- The same state of the work yields the same bytes (the point in time in the file is the latest change of the work,
  not the time of the export).

Pages for it (no RPC): `GET /werke/{id}/word` (explanation and form), `GET /werke/{id}/word.docx[?vorschlaege=1]`
(file, `Content-Type: application/vnd.openxmlformats-officedocument.wordprocessingml.document`,
`Content-Disposition: attachment; filename="<slug>.docx"`; unknown work 404, another value for `vorschlaege` 400,
request from a foreign page 403).

### Quitting

No method. Quitting is deliberately neither an RPC method nor an MCP tool (`rpc::call` does not know the remote side;
test `there_is_no_rpc_method_and_no_mcp_tool_to_quit`).

Quitting Baruch exists neither over RPC nor over MCP: it is only possible through the page `/beenden` on the own machine
or through a signal to the process. Routes: `GET /beenden` (confirmation question; not from the own machine 403) and
`POST /beenden` (behind `guard::same_origin`, only from the own machine, answer 200 with the closing page, 500 ms
later Baruch shuts down).

## Round 34 – Mind map

The idea map has become a mind map: ideas carry a title, a parent node and an order (migration 0052),
ideas without a place lie in the **cloud** next to the mind map. The sections "Ideas" and "Idea map (Round
25)" further above still apply; here stands what was added. Deviations from the contract of the round: the list of
ideas deleted along is called `deleted_ids` (`deleted` stays `true`), refusals carry -32000 (not -32001, which
has been `RAG_UNAVAILABLE` since Round 19c).

### Methods

Over RPC the origin is always the human. Every answer with `idea` additionally carries `title`, `parent` (id or
`null`), `position`, `main_topic` and `cloud` (derived: lies in the cloud).

| Method | Parameters | Answer |
|---|---|---|
| `idea.create` | as before, plus `title?` (≤ 200 characters), `parent?` (idea id, same work; sets `work` from it if missing), `before?` (id of a sibling; without = at the end), `cloud?` (`false` = without `parent` and `key` a free main topic; default: into the cloud). `words` may be missing if `title` is not empty; without both -32602. | `{idea, commit?}` |
| `idea.update` | as before, plus `title?` (renaming; empty only as long as a keyword stays) | `{idea, changed, commit?}` |
| `idea.move` | `{id, parent?: id\|null, key?: key\|null, before?: id\|null, cloud?: bool}` – missing fields stay. `parent: null` detaches from the parent node; if no reference stays either, the idea becomes a free main topic at the work. `key` sets or deletes the reference. `before` orders before a sibling, `null` at the end. `cloud: true` puts into the cloud (releases parent node and reference, child nodes stay at the idea); `cloud: false` makes an idea without parent node and reference a main topic. | `{idea, changed, commit?}`; unchanged → `changed: false` without a commit |
| `idea.delete` | `{id, confirm: true}` | `{id, deleted: true, deleted_ids: [ids], idea, commit?}` – the idea first, then the child nodes deleted along |
| `idea.restore` | `{id}` | `{idea, restored: [ids], detached?: true, commit?}` |
| `idea.tree` | `{work, status?, word?, origin?, q?, near?, near_limit?, near_threshold?, findings?}` (filter like `idea.map`) | `{map: MindMap, cloud: Cloud}` |
| `idea.map` | unchanged | as before; per idea additionally `title`, `parent`, `position`, `main_topic`, `cloud`; `label` is the title, otherwise the keywords |

`idea.move` refuses with -32000 (nothing changed, no commit): a parent node in another work, the idea
itself or one of its own descendants as parent node (cycle), depth above 32, a deleted idea or parent node,
`before` not a sibling, `cloud: true` together with `parent` or `key`. Unknown idea or wrong types:
-32602. Every change is a commit „Einfall <id>: umgehängt“ (`Baruch-Action: idea-move`) with the place before and
after in the body. Chapters and parts cannot be changed through `idea.*` – their identifier `n:<key>` is not an
idea id; for those, `outline.*` and the request rule stay.

`idea.map_pos_set` and `idea.map_pos_reset` stay: stored, without effect in the mind map.

Reversal for „Rückgängig“ in the widget: create ↔ `idea.delete`, delete ↔ `idea.restore` (brings the child nodes along),
rename ↔ `idea.update` with the old `title`, move ↔ `idea.move` with `parent`/`key`/`before` or `cloud`
from the idea before the action (`cloud: true` if it lay in the cloud; `parent: null, key: null, cloud: false`
if it was a main topic).

Forms without JavaScript: `POST /einfaelle/act` additionally takes `method=idea.move` with `parent` („in die Mindmap
unter Einfall Nr. …“), `cloud=1` („in die Wolke“) or `parent=none&key=none&cloud=0` („als Hauptthema ans Werk“);
the page `/einfaelle/{id}` shows the place and these three forms, `idea.create`/`idea.update` have a field
`title`.

### Routes of the page and the image

| Route | Content |
| --- | --- |
| `GET /werke/{id}/karte` | page „Mindmap“ (`idea_map/bild/mod.rs`, template `karte.html`) |
| `GET /werke/{id}/karte/bild` | only the SVG (`image/svg+xml`, `Cache-Control: no-store`), same query; here the fragment gets `xmlns` |

Query: filters as before (`status`, `word`, `origin`, `q`, `near`, `near_limit`, `near_threshold`, `befunde`),
plus `zu=<Kennungen, Komma>` (folded nodes) and `wahl=<Kennung>` (selected node, in the image
`aria-current="true"`). Identifiers are `w`, `n:<key>`, `i:<id>`; everything else is dropped (`bild::node_id`,
at most 2000). Both stand in `bild::Extra` (`closed`, `wahl`), which `parse_filter` fills; `filter_query` does not carry
them (outputs never fold).

The page builds two views: tree and cloud (`tree::scene`) for the image and – for the section „Gruppen“ – the flat
map (`idea_map::build`). That is a second pass with the same filter; if it fails, only the groups are missing.

### Routes for output, reading in and forms (no RPC)

| Route | Purpose |
|---|---|
| `GET /werke/{id}/karte.svg` \| `.pdf` \| `.md` \| `.mm` \| `.xmind` | outputs; query like the page (`status`, `word`, `origin`, `q`, `near`, `befunde`), `.svg`/`.pdf` additionally `zu=<Kennungen>` |
| `GET /werke/{id}/karte/gliederung` | outline view; plus `wahl=<Kennung>` (selected node), `zu=<Kennungen>`, after forms `ok`, `undo`, `wieder` |
| `POST /werke/{id}/karte/act` | forms (urlencoded), see below |
| `GET /werke/{id}/karte/einlesen` | form; `unter=i:<id>` preselects the target node |
| `POST /werke/{id}/karte/einlesen` | multipart `file`, optionally `parent`; shows the preview, creates nothing (413 above 10 MiB, 422 not readable, 409 target node from another work) |
| `POST /werke/{id}/karte/einlesen/anlegen` | urlencoded `tree` (JSON from the preview), `parent`; creates through `tree::import`, 303 to `/werke/{id}/karte?ok=eingelesen` |

`POST /werke/{id}/karte/act`: field `method` ∈ `idea.create`, `idea.update`, `idea.move`, `idea.delete`,
`idea.restore`, `idea.link`, `idea.unlink`, `idea.link_restore`, `idea.map_pos_reset`, `idea.map_groups_name`,
`idea.map_group_accept`, `idea.map_group_dismiss`. Plus the fields of the action:

- `idea.create`: `title`, `words`, `note`, `parent`, `key`, `before`, `ort` (`ort=werk` → free main topic; without
  `parent`, `key` and `ort` → cloud).
- `idea.update`: `id`, optionally `title`, `key`, `words`, `note` – only fields that the form sends change.
- `idea.move`: `id`, either `ziel` ∈ `wolke` (RPC `cloud: true`) \| `w` (main topic at the work) \| `i:<Einfall>` \|
  `n:<Kapitel-key>` or the fields of the RPC
  `parent`, `key`, `before` (present and empty = `null`).
- `idea.delete`, `idea.restore`: `id`. `idea.link`: `from`, `to`, `kind`, `note`. `idea.unlink`,
  `idea.link_restore`: `id` (connection).
- `back` (query part of the filter), `seite` ∈ `karte` \| `gliederung` (target of the redirect, default `karte`),
  `wahl` (selected node, only with `seite=gliederung`).

Answer: 303 to `/werke/{id}/karte[/gliederung]?<back>&ok=<wort>` (`angelegt`, `umbenannt`, `bezug`, `umgehaengt`,
`in-wolke`, `ort-unveraendert`, `geloescht` with `wieder=<id>`, `einfall-wieder`, `verbunden`, `unveraendert`,
`geloest` with `undo=<id>`, `wiederhergestellt`, …). Errors as a page with a message: 400 unknown action, 409 refused
(also: a named idea does not belong to the work of the address – before 422), 422 invalid input, 404 unknown
work, 403 foreign origin.

## Round 35 – Accounts and roles

Structure and permission check: `docs/Architektur.md` ("Round 35 – Accounts and roles", there also the tables
`LEKTOR_ROUTES` and `LEKTOR_RPC`). **Without accounts** nothing changes at `/rpc`, `/act` and all forms.

**New error codes:** `-32002` (`UNAUTHENTICATED`: there are accounts and the request carries no valid session; HTTP
401 at `/rpc`, already in the layer `auth::gate`) and `-32003` (`FORBIDDEN`: the role may not use the method or the work;
HTTP 200 at `/rpc`, 403 at `/lektorat/act`). `rpc::call` checks **every** method through `roles::check_rpc` before
it is executed – for `/rpc` (single and batch), `/act` and `/lektorat/act`; for Lektor accounts every method
outside `LEKTOR_RPC` is locked, also an unknown name („Das darf nur der Autor: <methode>“).

### `account.*`

Signing in, signing out and setting up exist only as a form (cookie). Passwords, hashes and tokens never stand in
an answer. Errors: `-32602` wrong parameters (the message quotes no value), `-32000` refused on domain grounds,
`-32002` not signed in (HTTP 401, already in the layer), `-32003` author only.

| Method | Parameters | Answer |
|---|---|---|
| `account.me` | `{}` | `{accounts_exist, signed_in, local_open, id?, name?, display?, kind?, handle?}` – the fields of the account only with a session (`kind`: `author` \| `lektor`). Anyone, also without accounts. |
| `account.list` | `{}` | `{accounts: [{id, name, display, kind, handle, active, created, last_login}]}` by id. Full rights only. |
| `account.create` | `{name, display?, password}` | the account (as in `account.list`), always `kind: "lektor"`. Full rights only. Without any account `-32000` with a hint at `/einrichten`; name invalid or taken, password too short or too long → `-32000`. |
| `account.set_active` | `{id, active}` | the account. Full rights only. Last active author account or unknown account → `-32000`. Locking ends all sessions of the account. |
| `account.set_local_open` | `{open}` | `{local_open}`. Full rights only. |
| `account.set_password` | `{current?, password, id?}` | `{id, changed: true}`. Own account (without `id` or with one's own): `current` is required (`-32602` without, `-32000` if wrong); the session of the request stays, all others end. Another account (`id`): full rights only, all sessions of the account end. |
| `account.logout_all` | `{}` | `{sessions}` – ends all sessions of the own account, including that of the request. Without a session `-32000`. |

`roles::check_rpc` decides before the dispatch what a Lektor may call (`account.me`,
`account.set_password`, `account.logout_all`); the methods „nur volle Rechte“ (full rights only) check `who.full_rights()` here a
second time.

### `revision.propose`, `finding.add`, `member.*`

| Method | Parameters | Answer / errors |

|---|---|---|
| `revision.propose` | `{work?, key \| node, body, note?}` | `{work, node, key, rev, status: "proposal", author}`; Lektor only (others: -32000 with a hint at `revision.save`). Proposal under `@lektorat:<slug>`, never the current revision; Markdown headings as with `revision.save`. Empty text, above 1 MiB, `note` above 2000 characters → -32602; text equal to the current revision → -32000; unknown section → -32602 or -32000 (key gone). No commit on the main branch, side branch `refs/revisions/<key>/<rev>`. |
| `finding.add` | `{work?, key?, kind: "hint" \| "critical" \| "praise", body, quote?}` | `{finding, commit? \| mirror?}` (`finding` as in `finding.get`, `persona` = handle, `status: "open"`, without a post); without `key` „Anmerkung zum Buch“ (finding without a section). Lektor only (others: -32000). Empty text, unknown kind, section not in the work → -32602. Exactly one commit in the mirror. |
| `member.list` | `{work}` | `{work, members: [{account, name, display, handle, role, granted, active}], candidates: [{account, name, display, handle}]}`; `candidates` = active Lektor accounts without access. Full rights only (Lektor: -32003). `work` is required (no default work). |
| `member.grant` | `{work, account, role?: "lektor"}` | `{work, account, role, granted}`; idempotent. Unknown work or account, another role → -32602; author account → -32000. Full rights only. No commit. |
| `member.revoke` | `{work, account}` | `{work, account, revoked}` (`false` if there was no access). Full rights only. No commit. |

Changed:

- `post.add`: the author of the post is `roles::author_name` – for Lektor accounts the handle, otherwise „Autor“ as before.
- `post.edit`, `post.delete`: Lektor accounts only their own posts (author of the post = handle); others' and those of the author
  → -32000 with the message „Nur eigene Beiträge lassen sich bearbeiten: …“. The author still manages every post.
- `post.page`, pages `/beitraege` and `/beitrag`: for Lektor accounts `own` means „under the own handle“.
- `work.list`: for Lektor accounts only works shared with them.
- `ui.render`: for Lektor accounts without literature.
- Every method outside `LEKTOR_RPC` (also unknown names): for Lektor accounts -32003.

### Routes (forms without JavaScript, all behind `guard::same_origin`)

| Route | Fields | Answer |
|---|---|---|
| `POST /anmelden` | `name`, `password`, `next` | 303 to `konten_ui::safe_next(next)` with a cookie; otherwise the sign-in page with 401, when locked 429 with `Retry-After`. Without accounts 303 `/` without a cookie. |
| `POST /abmelden` | – | 303 `/anmelden?ok=abgemeldet`, cookie deleted. Without accounts 303 `/`. |
| `POST /einrichten` | `name`, `display`, `password`, `password2` | Only without accounts and from the own machine: author account, session, 303 `/einstellungen/konten?ok=konto_angelegt`. Account present or not the own machine → page with 403; invalid input → 422. |
| `POST /konto/passwort` | `current`, `password`, `password2` | 303 `/konto?ok=passwort_geaendert` or `?fehler=passwort_falsch\|passwort_kurz\|passwort_lang\|passwort_ungleich\|unbekanntes_konto` (without an own account) |
| `POST /einstellungen/konten/act` | `action`, `id`, … | 303 `/einstellungen/konten?ok=…\|fehler=…`; full rights only (otherwise 403 with `x-baruch-abgelehnt: rolle`); unknown action 422 |

Actions of `/einstellungen/konten/act`:

| `action` | Fields | ok | Errors |
|---|---|---|---|
| `create` | `name`, `display`, `password`, `password2` – always `kind = lektor` | `konto_angelegt` | `name_ungueltig`, `name_vergeben`, `passwort_kurz`, `passwort_lang`, `passwort_ungleich`; without any account 303 `/einrichten` |
| `deactivate` | `id` | `konto_gesperrt` | `letzter_autor`, `unbekanntes_konto` |
| `activate` | `id` | `konto_entsperrt` | `unbekanntes_konto` |
| `reset_password` | `id`, `password`, `password2` | `passwort_geaendert` | like `create` (password), `unbekanntes_konto` |
| `local_open` | `open` = `0` \| `1` | `lokal_offen` / `lokal_anmelden` | – |

Names: 1 to 40 characters without control characters; the slug (rule like `docx_import::word_author`) may not be `unbekannt`
– a name therefore needs at least one letter a–z (also `ä`, `ö`, `ü`, `ß`) or a digit. Name
(regardless of case and white space) and slug are unique, also against locked accounts.
Display name at most 80 characters.

- `POST /lektorat/act` (behind `guard::same_origin`): fields `method`, `work`, `key`, `node`, `id`, `body`, `note`,
  `kind`, `quote`, `confirm` (`1` | `on` | `true`), `back`. Calls `rpc::call` – the same permission check as `/rpc`.
  Only methods from `LEKTOR_RPC` are accepted (also from the author: otherwise 403). If `work` is missing, for Lektor accounts the
  smallest work shared with them applies. Then 303: after `revision.propose` to `/?work=W&key=K&rev=N`, otherwise to the section
  `/?work=W&key=K`; with `back=befunde` (and after `finding.add` without `key`) to `/werke/W/befunde`; with `back=post`
  to `/beitrag?work=W&id=N` (after `post.delete` to the section). Errors: 403 „Abgelehnt: …“ with
  `x-baruch-abgelehnt: rolle` (`-32003`), 401 (`-32002`), 500 (internal), otherwise 422 „Fehler: …“.
- `POST /werke/{id}/mitwirkende/act` (full rights only): `action` = `grant` | `revoke`, `account`. 303 to
  `/werke/{id}/mitwirkende?ok=freigegeben` or `?ok=entzogen`; `?fehler=unbekanntes_konto` (account missing, unknown
  or not a number) or `?fehler=kein_lektor` (access for an author account). Unknown action 400, unknown work
  422.

Pages (`konten_ui::pages`):

| Route | Behaviour |
|---|---|
| `GET /anmelden[?next=…][&ok=abgemeldet]` | without accounts 303 `/`; otherwise the form `POST /anmelden` (`name`, `password`, `next`), link „Weiter“ to `safe_next(next)`; with a session „Angemeldet als …“ |
| `GET /einrichten` | with accounts 303 `/einstellungen/konten`; not from the own machine 403 with an explanation (without a form); otherwise the form `POST /einrichten` (`name`, `display`, `password`, `password2`) |
| `GET /konto[?ok=…][&fehler=…]` | name, role, origin, works (`roles::visible_works`), `POST /konto/passwort` (`current`, `password`, `password2`), `POST /abmelden`; without accounts an explanation; `LocalAuthor` a note |
| `GET /werke/{id}/mitwirkende[?ok=…][&fehler=…]` | full rights only (otherwise 403), unknown work 404; data from `rpc::call("member.list", {work})`; forms `POST /werke/{id}/mitwirkende/act` (`action=grant|revoke`, `account`) |
| `GET /einstellungen/konten[?ok=…][&fehler=…]` | list from `Store::account_list`, switch from `account.me` (`local_open`); forms `POST /einstellungen/konten/act` with `action=create` (`name`, `display`, `password`, `password2`), `deactivate`/`activate` (`id`), `reset_password` (`id`, `password`, `password2`), `local_open` (`open=0|1`); without accounts an explanation and „Einrichten“ |

`?ok=`/`?fehler=` show only the sentence from `MESSAGES` (a success code does not count as an error and vice versa; unknown codes
show nothing). All account pages answer with `Cache-Control: no-store`.

## Round 36 – Leftovers

No new method, no new route. Changed:

- **Roles (`roles::LEKTOR_RPC`, `LEKTOR_ROUTES`):** `wysiwyg.parse`, `wysiwyg.serialize`, `wysiwyg.diff`,
  `wysiwyg.style` and `ribbon.formats` stand for Lektor accounts again „ohne Werk“ (without a work; Round 35: binding to the work through the
  parameter). If `work` is missing, as for everyone `AppState::default_work` applies – for a Lektor the smallest
  work shared with them, without access an error (`-32602`, „unbekanntes Werk 0“; before `-32003` „Noch kein Werk freigegeben.“);
  a named `work` is still checked (`-32003`). `GET /einstellungen/darstellung` no longer needs a
  `work` for Lektor accounts (200 instead of 303 to `?work=`; reachable without access instead of 303 to `/werke`); the cookie
  `baruch_work` counts for a Lektor only if the work is shared with them.
- **`persona.propose`:** answer with `basis` (see the basis of review under "Solution proposals and variants"); `scope: "chapter"` is based, for the own
  part, on the shown or chosen revision.
- **`persona.chat`:** own open findings only with a review task or RFC.
- **`ui.render`:** with accounts the HTML pieces carry the display name of the origin („Erika Beispiel
  (@lektorat:erika)“), only as the text of the origin elements; attributes (`data-post-author`) stay the handle.
- **`outline.delete` and `outline.request_decide` (confirmed deletion):** unchanged answer; the mirror commit
  additionally names the revisions and posts deleted along (trailers `Baruch-Lost-Revisions`, `Baruch-Lost-Posts`).
- **`ribbon.formats`:** the format `quote` carries the text `#quote(block: true)[]\n` without the following line „– “.
- **`idea_map` reading in (`POST /werke/{id}/karte/einlesen/anlegen`):** nodes of the tree in the field `tree` may
  carry `cloud: true` (top level, without `parent`): they go into the cloud. If the field is missing, `false` applies.
- **`GET /einstellungen/daten?ok=sicherung`:** shows the message only once after a backup created through the form
  (a note in the process); `settings.backup` (RPC) answers directly as before.
- **`GET /static/js/vw.js`:** new delivered script under the existing route `/static/js/{name}` (classic,
  not a module; corrects the cookie `baruch_vw` and the attributes on `<html>` before the first paint).

## Work language

Round 38 (`src/work_language.rs`, store `work.language`, migration `0055_work_language.sql`): every work has a work
language, `"de"` (default, every work from before Round 38) or `"en"`. Personas answer in it and write their markers
in it; it does not change the language of the user interface and it is independent of the hyphenation language of
the typesetting settings (`<satz sprache>`, coupled in Round 39). The language is not book text: setting it creates
no entry in the mirror (like `work.timing_set`); it travels with the work in the exchange format (sub-version 10,
attribute `sprache` at `<angaben>`).

| Method | Parameters | Answer |
|---|---|---|
| `work.language` | `{work?}` | `{work, language: "de" \| "en"}` – read-only. `work` missing → default work, unknown → -32602 |
| `work.language_set` | `{work, language: "de" \| "en"}` | `{work, language, changed}`; `changed: false` if the work already had this language (nothing written). `work` is required. Any other value (also `"EN"`, `" en"`, a number) → -32602 with `language: allowed values are "de" and "en"`, nothing changed. **Only the human with full rights:** not through `/act`, not through MCP, not in `LEKTOR_RPC` |

`work.get` carries `language` at the top level beside `work`, `lector` and `cover`; `work.meta` carries it as
`work.language`, `work.list` as `language` of every work.

Form (without JavaScript): `POST /einstellungen/werk/sprache` with `work` and `language`, behind
`guard::same_origin`, not in `LEKTOR_ROUTES`. Answer 303 to `/einstellungen/werk?work=W&ok=sprache#sprache`, or with
an invalid value to `…&sprachfehler=1#sprache` (nothing changed). An unknown or missing work
(also when no default work can be determined) → 303 to `/einstellungen/werk?sprachfehler=werk#sprache`; the block
`id="sprache"` then shows „Die Werksprache ließ sich nicht speichern: dieses Werk gibt es nicht; nichts geändert.“,
and nothing changes. The block `id="sprache"` („Werksprache“) stands on the page „Einstellungen → Werk“ directly before
„Predigtzeit“.

MCP: no tool sets the language. `list_works` (per work) and `outline` (top level) carry `language` (`docs/MCP.md`).

### Language of the interfaces (Round 38)

English are since Round 38: the MCP `instructions`, every tool and parameter description, the answers of stubs and
every error text that arises only in `src/mcp/**`; for RPC the protocol errors – parse error, invalid request,
`unknown method …`, the parameter checks of `rpc.rs` (`work: work id (number) expected`, `key: stable key (number)
expected`, `mode: "auto" or "new" expected`), `Login required.` (-32002 on `/rpc` without a session) and `Only the
author may do this: <method>` (role). **Error codes are unchanged.**

**Messages of the user interface stay German until the catalogs of Round 39a, also when they travel through RPC or
MCP:** every message that a page, a form or a notice bar shows as well – the texts of `StoreError` (`Refused`,
unknown work, section or revision), the refusals of the modules (`REFUSED`, -32000) and their parameter checks outside
`rpc.rs`, progress reasons such as `fehlendes „Kritisch:“`, the notes `CUT_NOTE` and `TRUNCATED_NOTE` below a post,
the note of a cover request created by `asset_add`, and labels of model calls. That is why this document quotes such
messages in German („…“). Until Round 38 this also held for the task texts that Baruch puts around the text of a
section when it calls a model (`assist/context.rs`, `solutions.rs`, `variants.rs`, `glossary.rs`, `threads.rs`; see
"Open" below). Since Round 38c this whole document is English; German stays only as quotation (texts of the user
interface, messages, commit subjects, model markers) and in code.

### Prompts and markers

The default prompts of the personas (`assist/prompts.rs`) are English since Round 38. A prompt stored by the user
stays untouched. Below **every** effective prompt of a model call – default or stored – stands the language rule
`assist::lang::language_rule(language)`: "Always answer in {German|English} – the language of this work – whatever
language this prompt or the question uses. Use these markers literally: …" with the markers of the work language.
The prompt shown for editing (`persona.prompt_get`, „Einstellungen → Assistenten“) carries no language rule, so that a
saved prompt does not take the rule of one work into another.

The prompt prescribes the markers of the work language; **every recognition accepts both languages**, whatever the
work language is. For a German work every behaviour is literally as before Round 38.

| Meaning | German (`lang::DE`) | English (`lang::EN`) | Read by |
|---|---|---|---|
| mandatory or critical finding | `Kritisch:` | `Critical:` | `prompts::critical_line`, `has_critical`, findings parser |
| hint | `Vorschlag:`, `Hinweis:` | `Suggestion:`, `Note:` | findings parser, `strip_critical_lines` (`Vorschlag:`/`Suggestion:`) |
| praise | `Lob:` | `Praise:` | findings parser |
| outline of a review | `Was trägt:`, `Was fehlt:`, `Was stört:` | `What works:`, `What is missing:`, `What disturbs:` | findings parser, `strip_critical_lines` (first one) |
| RFC summary of the lector | `Jetzt wichtig:`, `Kann warten:` | `Important now:`, `Can wait:` | `strip_critical_lines` |
| solutions the persona would apply | `Mein Vorschlag:` | `My proposal:` | `findings::parse::proposal_heading`, `assist::solutions::proposal_block` |
| solution lines | `statt:`, `besser:`, `Schritt:` | `instead of:`, `better:`, `Step:` | `assist/solutions.rs`, `prompts::missing_solutions` |
| feedback on an own finding | `Erledigt: Befund n (Stelle "…")` | `Done: finding n (place "…")` | `findings::parse::done_claims` |
| all own findings resolved | `Alle meine Befunde zu diesem Abschnitt scheinen erledigt.` | `All my findings on this section seem to be resolved.` | `prompts::all_done` |
| note of Baruch: quote not in the text | `(nicht im Text belegt)` | `(not backed by the text)` | written in the work language, removed and read in both |
| note of Baruch: no solution | `(ohne Lösungsvorschlag)` | `(no solution proposed)` | written in the work language, removed and read in both |
| note of Baruch: mandatory finding missing | `(Hinweis von Baruch: …)` (`CRITICAL_MISSING_NOTE`) | `(Note from Baruch: …)` (`CRITICAL_MISSING_NOTE_EN`) | written in the work language, footer in both |

Further markers found in Round 38 beyond the contract (`lang::MoreMarkers`): `besser:`/`better:`,
`Schritt:`/`Step:`, the words of the "done" line (`Befund`/`finding`, `Stelle`/`place`) and the two further outline
headings. Forbidden openings are recognised in both languages (`Als Assistent`, `Als KI`, `Als Sprachmodell`,
`Gerne`, `Natürlich`; `As an assistant`, `As an AI`, `As a language model`, `Sure`, `Certainly`), and the word list
of a check order in the chat knows English stems too (`check`, `review`, `verif…`, `examin…`, `proofread`).

**Since Round 38c** the task texts around the section (`Work: …`, `Task: …`, frames of findings and chat history)
are English as well; texts that name markers (`OTHERS_RULE`, `RFC_TASK`, `PROPOSE_OUTPUT`) go through
`lang::localized`, so a German work keeps its German markers literally and an English work gets the English ones.
The language rule alone fixes the answer language. Still German on purpose: the framing tags (`<<<befund`,
`<<<einfall` …), the answer form of variants (`Zeile:`/`Befunde:`/`Text:`, `[[Absatz n]]`, read back by the parser)
and labels that also appear in the user interface. **Open (honest):** The literal words
of `@schrift` and `@belege` that Baruch reads back (`Wortlaut ungeprüft`, `nicht belegt`, `Angabe prüfen`) are
recognised in German only; the English prompt therefore still names the German words for these.
