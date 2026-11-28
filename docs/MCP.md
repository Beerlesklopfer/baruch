# MCP server of Baruch

`POST /mcp`, JSON-RPC 2.0 following the Model Context Protocol **2025-06-18** (Streamable HTTP, JSON answers only,
no SSE). Source: `crates/baruch-server/src/mcp.rs` (protocol, access), `src/mcp/tools.rs` (tools),
`src/mcp/ideas.rs` (ideas), `src/mcp/session.rs` (sessions), `src/mcp/scripture.rs` (`scripture_check`),
`src/mcp/scripture_map.rs` (`scripture_map`, Round 17a),
`src/mcp/bibles.rs` (`bible_*`, SWORD Bibles), `src/mcp/exchange.rs` (`exchange_export`, Round 26),
`src/mcp/cover.rs` (`cover_layout`, `cover_layout_set`, Round 27),
`src/mcp/tokens.rs` (RPC `mcp_token.*`). According to the plan, the common entrance for all personas and tools such as
Claude Code („Eigener Endpunkt je Persona“, incoming).

**State:** reading (works, outline, sections, search, personas, images) and writing as a proposal or
post are implemented, plus ideas with full CRUD (they change no book text, see "Ideas") and the
outline (creating immediately, everything else as a request, see "Outline"). Since Round 10 (B2) personas answer
for real (`ask_persona`, `chat` through Eliezer), `scripture_check` checks Bible references (since 2026-11-06, with a
chosen SWORD Bible, also the wording; `bible_*` read the Bible), and other machines get in with
a token (see "Access"). Open stubs in the code:
`grep -rn "STUB:" crates`.

**Only the human:** accepting, changing, deleting and switching revisions go only through the user interface
or RPC (`docs/RPC.md`). **Changing, deleting and switching do not exist over MCP**, nor does importing:
an import creates current revisions under the name of the human, and an assistant must not do that
(decision 2026-11-05). Typst books come into the program through `baruch-server import`
(`docs/Bedienung.md`) or through uploading in the user interface. A call of `import_typst` today yields
-32602 „unbekanntes Werkzeug“. Likewise there is **no reading in of an exchange file** over MCP (Round 26): exporting
works read-only with `exchange_export`, reading in only by the human (page „Austausch: Einlesen…“, RPC
`work.exchange_preview`/`work.exchange_import`, `baruch-server import DATEI.baruch`). Since Round 32 the same holds for Word: **Word import and Word export do not exist over MCP** (pages
`/werke/import/word` and `/werke/{id}/word`, RPC `docx_import.*` and `work.docx_export`, `baruch-server import DATEI.docx`
and `export … --docx`); a call of `docx_import` or `docx_export` yields -32602 „unbekanntes Werkzeug“.
**Quitting Baruch exists neither over RPC nor over MCP:** that works only through the page `/beenden` at the own machine
or through a signal to the process. Likewise there is no tool that creates, changes (title, metadata,
work template) or deletes works: that is the work management of the human (`/werke`, RPC `work.create/update/delete`,
`docs/RPC.md` section "Works"). `list_works` only reads.

## Access

1. **Foreign web pages:** `/mcp`, like `/rpc`, sits behind `guard::same_origin`. If `Origin` does not match `Host`
   or `Sec-Fetch-Site` is neither `same-origin` nor `none`, the server answers **403** – before any
   further check. Clients without `Origin` (Claude Code, curl, scripts) pass.
2. **Without a token only from the own machine:** without `Authorization` the endpoint only accepts connections from
   `127.0.0.1`, `::1` (also `::ffff:127.0.0.1`). Every other peer – and a request whose
   peer the server does not know – gets **401** with `WWW-Authenticate: Bearer realm="baruch"` and the
   text `MCP ohne Token nur vom eigenen Rechner (127.0.0.1 oder ::1). Von anderen Rechnern mit
   „Authorization: Bearer <Token>“; Tokens erzeugt die Einstellungsseite „MCP-Zugang“.`
   **Exception behind a reverse proxy (Round 24):** if Baruch was started with `--allow-host NAME` or
   `BARUCH_ALLOWED_HOSTS` and the request carries one of these additional names in `Host` (with or
   without port), it **never counts as local** – even if the peer is `127.0.0.1` or `::1`, because that is
   then the proxy and not the human at their own machine. Under such a name `/mcp` works
   **only with a token**; without `Authorization` the same **401** comes. If additional names are allowed, a request
   entirely without a readable `Host` header does not count as local either (`guard::via_proxy`). Under `127.0.0.1`, `::1` or
   `localhost` the rule above stays in force on the same server. Limit: if the proxy rewrites `Host` to
   `127.0.0.1:<Port>`, Baruch cannot tell the request from a local one – the proxy must pass the
   name through (`docs/Betrieb.md`, with a probe after setting it up). Baruch does not evaluate `X-Forwarded-*`.
   **With a token (Round 10):** `Authorization: Bearer <Token>` (created under „Einstellungen → MCP-Zugang“ or
   RPC `mcp_token.create`, stored only as a SHA-256 hash) opens `/mcp` from any machine. The client is then
   named like the token (`@mcp:<tokenname>`, also without a session or with a foreign one; of `clientInfo` only the
   version counts) and must never name `persona` as author or origin (see "Managing posts"). A wrong or revoked token or a scheme other than Bearer → **401** with
   `error="invalid_token"`, also over loopback. From other machines **only** `/mcp` is reachable
   (`guard::remote_only_mcp`, otherwise 403). For that Baruch must listen on a network address (`--listen`), the
   client addresses it by its IP address (see 4.); Baruch speaks only HTTP without TLS – token and texts
   travel through the network in plain text, use it only in the own network or through a tunnel.
3. **Content type:** `Content-Type: application/json`, otherwise 415.
4. **No foreign name:** `Host` must be an IP address, `localhost` or a host name allowed with `--allow-host` at
   start (Round 24, exactly this name, no subdomains), otherwise **421** (holds for all
   routes, `guard::local_host`). So a web page cannot use DNS rebinding (a foreign name points to
   127.0.0.1) to call tools or read texts. Without `--allow-host` no name is allowed.

The server must know the peer (`into_make_service_with_connect_info::<SocketAddr>()` in `main.rs`);
otherwise every request counts as foreign.

Setting up in Claude Code (server local on the default port 8765, only for this project):

```
claude mcp add -s local --transport http baruch http://127.0.0.1:8765/mcp
```

From another machine (token from „Einstellungen → MCP-Zugang“):

```
claude mcp add -s local --transport http baruch http://<IP>:8765/mcp --header "Authorization: Bearer <Token>"
```

Behind a reverse proxy with TLS (name allowed with `--allow-host`, always with a token):

```
claude mcp add -s local --transport http baruch https://<Name>/mcp --header "Authorization: Bearer <Token>"
```

The systemd user service (`packaging/systemd/`) uses the same port. An older entry with a
different port is removed with `claude mcp remove baruch -s local` and set again as above.

## Sessions

`initialize` remembers `clientInfo` (`name`, `version`) under a new session and sends its id in
the answer header **`Mcp-Session-Id`** (32 hex characters). Later requests with this header
are attributed to the client; without an id or with an unknown one the client counts as „unbekannt“. The table
lives in the process (empty after a restart) and holds at most 256 sessions; the oldest drops out first.
The id is no secret and replaces no token.

From `clientInfo` come:
- **Client handle** `@mcp:<name>`: name in lower case, characters outside `[a-z0-9._-]` → `-`, at most 32 characters,
  empty → `unbekannt`. Example: "Claude Code" → `@mcp:claude-code`.
- **Model** (if the call names none): "<name> <version>", without a version only the name, without a client
  „unbekannt“.

## Messages

| Method | Answer |
|---|---|
| `initialize` | `protocolVersion` (the client's, otherwise `2025-06-18`), `capabilities.tools`, `serverInfo`, `instructions`; header `Mcp-Session-Id` |
| `ping` | `{}` |
| `tools/list` | `{tools: [{name, description, inputSchema}]}` in the order of the table below |
| `tools/call {name, arguments}` | see "Answers" |
| Notification (without `id`) | HTTP 202 without a body |

A list of messages is answered in order; only notifications → 202.

### Answers of `tools/call`

- **Success:** `{content: [{type: "text", text: <JSON as text>}], structuredContent: <the same JSON>, isError: false}`.
  `structuredContent` is always an object (MCP demands that); lists stand in it under a name
  (`works`, `nodes`, `hits`, `personas`).
- **Domain error** (unknown work, unknown or no longer existing section, unknown
  revision or persona, empty text, text equal to the current revision, headings do not match the
  section, invalid reference, stub): `{content: [{type: "text", text}], isError: true}`
  with a German text.
- **Protocol error:** unknown tool (also the removed `import_typst`), unreadable `arguments` or a
  writing tool with `node` or without `key` (see below) → JSON-RPC error **-32602** with a German
  hint; database error → **-32603**.

## Tools

`work` is everywhere the id (number) or the slug (text) from `list_works`; `node` is the **position** of the
section in the work (`node` from `outline`, from 0), `key` its **fixed key** (`key` from `outline` or
`read_node`); `rev` is the **number** of a revision (`number` in `read_node`, from 1).

**Caution, positions change:** if the human moves, changes the level of, creates, deletes, splits or merges items,
Baruch assigns the positions `node` anew. An assistant that read `outline` before would hit a different section with
the old position. Therefore (decision after Round 4, implemented in Round 5):

- **Writing only through `key`:** `propose_revision` and `add_post` – also every reference in `refs` – take the
  section **only** through `key` (mandatory). If `node` stands in the arguments (also in addition to `key`) or `key`
  is missing, Baruch answers with JSON-RPC **-32602** and a German hint, e.g. „arguments: node (Stelle)
  wird nicht angenommen; key mitgeben …“ or „refs[1]: key fehlt …“; nothing is written. The
  `inputSchema` of both tools knows no `node` and requires `work`, `key` and `body`; every entry in
  `refs` requires `key`, `rev` and `quote`.
- **Reading as before:** `read_node` takes `node` or `key` (if `key` is present, it wins); `outline` and `search`
  stay unchanged. The answers name `node` (position now) and `key`.
- If the section of a key no longer exists, „Diesen Gliederungspunkt gibt es nicht mehr …“ comes
  (`isError`). The key is resolved under the write lock so that nobody rearranges in between.

**Proposals never change the outline:** the text of `propose_revision` begins with exactly one heading
of the level of the section (`=` chapter, `==` section, `===` subsection; a new title is allowed and
renames on acceptance). Another level, further headings, no heading or text before it → error
with a reason, no proposal. Level, splitting and merging stay with the human (editor, outline).

Numbers are never assigned anew; after the human deletes a revision the sequence has gaps
(e.g. 1, 2, 4). Baruch therefore always looks up revisions by number, never by their position in the list.

| Tool | Parameters | Result (`structuredContent`) | State |
|---|---|---|---|
| `list_works` | – | `{works: [{id, slug, title, language}]}`; `language` = work language `de` \| `en` (Round 38) | implemented |
| `outline` | `{work}` | `{work: {id, slug, title}, language, nodes: [{node, key, parent, number, title, kind, current_rev, proposals, url}]}`; `current_rev` is the number of the current revision | implemented |
| `work_timing` | `{work}` | speaking time of the work (time keeper), **read-only**, exactly like RPC `work.timing` (`docs/RPC.md`, "Time watch"): `{work, kind, words_per_minute, target_seconds, words, seconds, status: "none" \| "ok" \| "over", over_seconds, shown, clock, target_clock, over_clock, sections: [{key, seq, depth, number, title, words, seconds, start_seconds, subtree_seconds, clock, start_clock, subtree_clock}]}`; per item the current revision without Typst markup is counted (rules in `docs/Bedienung.md`, „Zeitwächter“). Unknown work → `isError`. Words per minute and target time are set only by the human („Einstellungen → Werk → Predigtzeit“, RPC `work.timing_set`); there is no tool for it | implemented (Round 16b) |
| `read_node` | `{work, node \| key, rev?}` | `{node, key, number, title, rev, text, revisions: [{number, status, author: {kind, name, model?}, note}], posts: [{id, author, body, created}], url}`; `text` is revision `rev`, otherwise the current one (without a current one, the one with the highest number); `posts` names the ids so that a client finds its own posts | implemented |
| `search` | `{query, kinds?, work?, all_revisions?, limit?}` | `{hits: [SearchHit]}` like RPC `library.search` (`docs/RPC.md`), per hit additionally `node_key` and `url` (`/?work=W&key=K[&rev=R]`, without a section `null`); without `work` across all works, library always | implemented |
| `typst_check` | `{work?, key?}` | `{ok, stand, pages, errors, warnings, diagnostics, print, missing, url}` – typesets the work with Typst (like RPC `preview.typst`), **read-only**, changes nothing, no commit; `ok` = no errors, `diagnostics` as with `preview.typst`, `url` = `/werke/W/satz?key=K` or `/werke/W/satz`; unknown `key` or timeout (60 s) → `isError`; since Round 18a the run can be cancelled through RPC `preview.cancel` or „Satz abbrechen“ of the typesetting page → `isError` „Satz abgebrochen.“; `@preview` packages are not loaded | implemented (Round 8) |
| `exchange_export` | `{work, format?: "xml" (default) \| "baruch"}` | `{work, uuid, format, file_name, media_type, bytes, sha256, xml?, data?}` – the whole work as an exchange file (`docs/Austauschformat.md`), **read-only**, changes nothing, no commit; like RPC `work.exchange_export`: with `xml` the file `<slug>.baruch.xml` as text in `xml` (images as Base64), with `baruch` the ZIP container `<slug>.baruch` as Base64 in `data`; `sha256` over the delivered bytes. Format minor version 7: including the idea map (`<karte>` with connections between ideas and positions, since Round 25), cover layout, addresses in the imprint and typeset view settings (since Round 27), findings and old post revisions (minor version 4, Round 28), solutions, variants and keys of the items (minor version 5, Round 30), typesetting settings `<satz>` (minor version 6, Round 29), since Round 31 minor version 7 with own indexes, glossary
(also proposals) and the index/glossary fields of `<satz>`. Without keys, connections to services, persona models and requests. **Upper limit 4 MiB** file (`MCP_EXPORT_LIMIT`, the answer carries the content twice): above it `isError` with a hint to the page `/werke/{id}/austausch` and `baruch-server export SLUG --baruch DATEI`; unknown work → `isError`, unknown `format` → -32602. Reading in does not exist over MCP | implemented (Round 26) |
| `typeset_settings` | `{work?}` | `{work, set, uuid, updated, updated_by, settings, rules, conflicts, template_rule, warnings}` like RPC `work.typeset_get` without `fields` – typesetting settings of the work, the generated Typst rules „// Baruch: Satz“ and hints where the work template sets the same thing itself; **read-only**, only the human changes them („Einstellungen → Werk → Satz“); no commit; unknown work → `isError` | implemented (Round 29a, read-only) |
| `epub_export` | `{work, footnotes?: "popup" \| "endnotes", include_bytes? = false}` | `{work, file_name, url, bytes, sha256, footnotes, meta, files, toc, warnings, epub_base64?}` – the work as EPUB, **read-only**, no commit; bytes only with `include_bytes`, at most 8 MiB (details under "EPUB (Round 20)") | implemented (Round 20a) |
| `epub_check` | `{work, readers?: [slug], footnotes?}` | `{findings, xml_tool, epubcheck, summary, readers, work, url}` like RPC `epub.check` – own check and reader warnings, **read-only**, never starts EPUBCheck (details under "EPUB (Round 20)") | implemented (Round 20c) |
| `glossary_list` | `{work?, status?: "accepted" \| "proposed" \| "all", word?}` | `{work, terms: [Term], url: "/werke/W/glossar"}` (with `word` additionally `word`) – glossary of the work, **read-only**, no commit; Term like RPC `glossary.list`; with `word` like RPC `glossary.lookup` (only accepted terms, word or variant, case does not matter); unknown status → `isError` | implemented (Round 31a) |
| `glossary_propose` | `{work?, term, short?, long?, variants?, source?, bible_refs?, reason?, persona?}` | `{work, id, status: "proposed", origin, term, url: "/werke/W/glossar#begriff-ID", reason?, commit?\|mirror?}` – **proposal only**, origin persona (`persona`, only without a token from the own machine) or `@mcp:<name>`; only the human accepts, changes, deletes; an already accepted or already proposed term (same key), an unreadable Bible reference or an exceeded limit → `isError`; `reason` does not stand in the glossary, only in the answer; one mirror commit `Baruch-Action: glossary-propose` with the client or the persona as author | implemented (Round 31a) |
| `index_propose` | `{work?, key, word, occurrence?, register?, term?, sub?, see?, also?, reason?, persona?}` (no `node`) | `{work, id, status: "open", proposal, url: "/werke/W/register#vorschlag-ID", commit?\|mirror?}` – **index proposal only**, never a text change; the human sets the mark on acceptance (RPC `index.decide`, new revision of the human); `node` instead of `key`, missing `key`, word not in the current revision, unknown index or invalid mark → `isError`; one mirror commit `Baruch-Action: index-propose` | implemented (Round 31a) |
| `propose_revision` | `{work, key, body, note?, persona?, model?}` (no `node`) | `{work, node, key, rev, status: "proposed", author, model}` | implemented |
| `add_post` | `{work, key, body, refs?: [{key, rev, quote}], sources?: [string], persona?}` (no `node`, not in `refs` either) | `{work, node, key, post, id, author, url}` (`post` = position in the section chat from 0, `id` = post) | implemented |
| `post_move` | `{work, id, key, persona?}` (no `node`) | `{work, id, node, key, from_node, from_key, url, commit?}` – **own posts only**, to the end of the chat of `key` | implemented (Round 8) |
| `post_delete` | `{work, id, thread?, persona?}` | `{work, id, node, key, deleted: {author, body}, url, commit?}` – **own posts only**, without asking back (the commit records the post); with `thread` an own thread post (`{work, thread, id, deleted, url, commit?}`) | implemented (Round 8, `thread` Round 30c) |
| `post_to_idea` | `{work, id, words, persona?}` | `{work, id, idea, node, key, url, commit?}` – **own posts only**; idea with reference = section, thought = text, afterwards the post is gone (one commit) | implemented (Round 8) |
| `outline_add` | `{work, key?, place? = "after", title, level? \| items: [{title, level?}], note?, persona?, model?}` (no `node`) | `{work, action: "added", detail, points: [{node, key, level, title, url}], node, key, url, author, adopted: [], commit?, mirror?}` – see "Outline"; parts (`level: 0`) since Round 14 also with a work template. If a new part would take over existing chapters (user question 12): nothing created, instead `{work, action: "requested", status: "open", request: OutlineRequest, adopted: [key], message, url}` – a request the human must actively approve | implemented |
| `outline_rename` | `{work, key, title, note?, persona?}` | request: `{work, request: OutlineRequest, status: "open", message, url}` (`url` = `/gliederung?work=W#antraege`) | implemented |
| `outline_move` | `{work, key, target_key, place, note?, persona?}` (no `node`, no `target`) | request as above; the level stays as with RPC `outline.move` | implemented |
| `outline_set_level` | `{work, key, level: 1–3, note?, persona?}` | request as above; `level: 0` (to a part) and changing the level of parts → `isError` | implemented |
| `outline_set_numbered` | `{work, key, numbered: bool, note?, persona?}` | request as above (count with/without a number; part without a number → `isError`) | implemented |
| `outline_delete` | `{work, key, note?, persona?}` | request as above (only without sub-items, not the last chapter) | implemented |
| `outline_requests` | `{work, open? = true, mine? = false, preview? = false, persona?}` | `{work, requests: [OutlineRequest], url}` – `OutlineRequest` like RPC `outline.requests` (`docs/RPC.md`); with `preview: true` per open request `preview: [{key, level, number, title, mark, kind_word, before_number, before_level, label, before_label, changed_label}]` or `preview_error` (Round 14, kind and previous state since Round 16c) | implemented |
| `asset_add` | `{work?, file_name, content, encoding: "base64" \| "svg", name?, path?, caption?, alt?, note?, role?: "cover", persona?}` | `{work, name, rev, mime, bytes, url, author, commit? \| mirror?, request?, message?, request_error?}` – creates an image **immediately** (revision 1, author persona or `@mcp:<client>`, one commit with `Baruch-Origin`); at most 8 MiB decoded; `role: "cover"` additionally files a request `set_cover` (if only the request fails, the image stays, `request_error` says why); quota see "Quota for images (21c)" | implemented (Round 11, quota 21c) |
| `asset_replace` | `{work?, name, content, encoding, caption?, alt?, note?, rename?: bool = false, persona?}` | request: `{work, request: AssetRequest, status: "open", message, url}` – not unchanged; changes nothing immediately. A format other than the image's only with `rename: true` (Round 21c; otherwise `isError` with a hint): **one** request, on confirmation the image gets the new extension (`haus.svg` → `haus.png`), all paths stay; `request.rename` = true. Quota see below | implemented (Round 11, `rename` and quota 21c) |
| `asset_set_cover` | `{work?, name: string \| null, note?, persona?}` | request as above (`null` = remove the cover) | implemented (Round 11) |
| `asset_delete` | `{work?, name, rev?, note?, persona?}` | request as above (without `rev` the whole image, with `rev` a non-current revision) | implemented (Round 11) |
| `asset_requests` | `{work?, open? = true, mine?, persona?}` | `{work, requests: [AssetRequest], url}` – read; never contents, only `bytes`/`content_url`; `AssetRequest` like RPC `asset.request_list` (`docs/RPC.md`, since 21c with `rename` and `purged`); first removes the content of decided replacement requests that were decided more than 30 days ago | implemented (Round 11, clean-up 21c) |
| `cover_layout` | `{work}` | like RPC `cover.layout` (`docs/RPC.md`, "Cover layout and imprint"): all 15 blocks (stored or default) with position and width in mm from the trim edge of their own area, dimensions, print check, default font; without a page size (typesetting error in the book) `{…, blocks: only the stored ones, geometry: null, geometry_error}`; read-only | implemented (Round 27) |
| `cover_layout_set` | `{work, blocks: [CoverBlock], note?, persona?}` | request (image requests, op `cover_layout`, origin persona or `@mcp:<client>`) → `{work, request, status: "open", message, url}`; every named block replaces its kind entirely, missing fields of a block come from the effective block (e.g. `{kind: "front_title", y_mm: 40}`); the request remembers the fingerprint of the stored layout (`expected_layout`); an equal open request is not created twice; invalid values (limits like RPC `cover.layout_set`) or a request that would change nothing → `isError`; changes nothing immediately, the human confirms on the page „Bilder → Anträge“ (outdated if they have changed the layout in the meantime) | implemented (Round 27) |
| `findings` | `{work, key?, status?, kind?, persona?, include_praise?, limit?}` | `{work, total, truncated, findings: [Finding], url}` – read-only; `total` = all matching ones (without limit), `truncated` if only the newest `limit` come along; Finding like `finding_json` (all fields of `Finding`, plus `kind_label`, `status_label`, `url`); `key` only this section; `status`/`kind` database words separated by commas; without attached ones, praise only with `include_praise`; unknown word or section → `isError` | implemented (Round 28) |
| `finding_propose_status` | `{work, id, status: adopted\|deferred\|done, rev?, target_key?, target_idea?, note?, persona?}` | only a proposal for the status of a finding (origin `persona` or `@mcp:<client>`) through `findings::rpc::propose` (lock, one commit), which the human confirms; `adopted` needs `rev`, `deferred` exactly one of `target_key`/`target_idea`; `rejected`/`open` → `isError` „Nur der Mensch kann …“ | implemented (Round 28; status path itself: befunde) |
| `post_edit` | `{work, id, body, thread?}` | edit an own post (only author `@mcp:<client>`); the old revision stays; in the section chat one commit, in the thread none (see "Managing posts") | implemented (Round 28) |
| `text_search` | `{work, q, case?, word?, re?, range?, key?, kinds?, limit?}` | search in the work like RPC `search.find` (wording, whole word, regular expression; `range` `section`/`chapter`/`part`/`from`/`work`, default `work`; `kinds` default `text`, `heading`, `footnote`, `quote`) – read-only; replacing does not exist over MCP, proposals per section through `propose_revision`; `search` (full text) stays | implemented (Round 30a) |
| `solution_variants` | `{work, key, rev?, findings?, count?: 2\|3}` | the lector (according to the setting) writes 2 or 3 variants A/B/C as sibling proposals with the same base; only creates proposals, counts towards the limit of two model jobs per client | implemented (Round 30b) |
| `list_personas` | – | `{personas: [{handle, name, role, model, context_length, active, lector}], lector, lector_mode}`; `lector` follows the choice (fixed, default `@lektor`, or automatic by context) | implemented |
| `list_assets` | `{work?}` | `{work, assets: [AssetSummary]}` like RPC `asset.list`; without `work` the default work; read-only | implemented |
| `read_asset` | `{work?, name, rev?, content? = false}` | `{work, name, revision, revisions, encoding}`, with `content: true` also the content (Base64 or SVG text); unknown image or revision → `isError`; read-only | implemented |
| `sources_list` | – | `{sources: [{id, title, url, note, position}]}` – the preferred sources for `@belege` („Einstellungen → Quellen“) in fixed order; read-only, only the human can change them | implemented (Round 9) |
| `idea_create` | `{words?, title?, parent?, before?, cloud?, work?, key?, note?, quote?, source?, bible?, persona?}` (since Round 34 `title` instead of `words` suffices; without a place the idea lands in the cloud) | `{idea, commit?}` like RPC `idea.create`; every idea in the `idea_*` answers carries `url` (with a reference `/?work=W&key=K`, only a work `/?work=W`, otherwise `null`) | implemented |
| `add_idea` | like `idea_create` | like `idea_create` (second name, wish 2026-11-06) | implemented |
| `idea_read` | `{id}` | `{idea}` (also deleted ones) | implemented |
| `idea_list` | `{work?: id \| slug \| "none", word?, open?: bool}` | `{ideas, words}` | implemented |
| `idea_search` | `{query, work?, mode?}` | `{ideas, hits: [{id, snippet}]}`; empty search → `isError`; since Round 19c `mode: "rag"` (only with `work`) like RPC `idea.search` with semantic search (`hits` with `score`, `weak`), failure as `isError` with a reason | implemented |
| `idea_update` | `{id, title?, words?, note?, quote?, source?, bible?, open?: bool, work?: id \| slug \| null, key?: int \| null, persona?}` | `{idea, changed, commit?, unlinked?}`; when moving to another work `unlinked` = ids of the connections softly removed in the process (only within one work), positions on maps are dropped | implemented |
| `idea_delete` | `{id, persona?}` | `{id, deleted: true, deleted_ids, idea, commit?}` (without `confirm`, restorable); since Round 34 the sub-nodes are deleted with it | implemented |
| `idea_move` | `{id, parent?: id \| null, key?: key \| null, before?: id \| null, cloud?: bool, persona?}` | `{idea, changed, commit?}` like RPC `idea.move`: re-hangs an idea in the mind map, orders it, puts it into the cloud (`cloud: true`) or makes it a main topic at the work (`cloud: false`); a violated rule (cycle, other work, depth over 32) → `isError` | implemented (Round 34) |
| `idea_tree` | `{work, status?, word?, origin?, q?}` | `{map, cloud}` like RPC `idea.tree`, **read-only**: the mind map as a tree (work, parts, chapters, main topics, sub-nodes) and beside it the cloud with a counter and suggestions „passt zu Ast …“ | implemented (Round 34) |
| `ask_persona` | `{work, key, persona, question?, mode?: "ask" \| "review" \| "propose", rev?, scope?: "section" \| "chapter"}` | ask a persona with its role prompt through Eliezer. `ask` (default): question (mandatory) as a post of the client (`@mcp:<name>`), answer as a post of the persona; `review`: findings as a post – when reviewing again the persona sees its own open findings and reports implemented ones with „Erledigt: Befund n“; from that a status proposal „erledigt“ arises (query, only the human decides; Round 22); `propose`: new revision only as a proposal (`proposed`). `scope` (Round 15): `section` (default) or `chapter` (whole chapter or whole sermon; unknown → -32602). Result `{work, key, persona, mode, scope, model, text, truncated, cut, compressed, post_id? (+ question_post_id), rev?/status?, url, basis?}`; `basis` (Round 30b) = reviewed revision `{rev, label, proposal}`; `compressed`: `null` or `{message, summarized, chunks}` if the context was chunked and summarised; `cut`: the model stopped at the length limit (post with a hint; a cut-off proposal is not created → `isError`). `key` mandatory, `node` → -32602; backend errors, unknown or inactive persona → `isError` with an honest message. No web search: @belege never searches the web over MCP (only jobs from the user interface). Counts towards the queue per client (see below) | implemented (Round 10, `scope`/`compressed` Round 15) |
| `revision_amends` | `{work, key? \| node?, rev}` | history of the amendments of a revision, **read-only**: `{work, node, key, rev, amend_seq, steps: [{uuid, seq, kind, undo_of?, author, created, body_before, body_after}], note}` (details under "Round 22"); only the human undoes | implemented (Round 22) |
| `chat` | `{work, key?, thread?, message, persona?}` | With `key`: post of the client in the section chat. Without `key` (Round 15): post in a **thread without a section** – new or continued with `thread` (the human reads and answers under `/faeden`; `thread` together with `key` → -32602, unknown thread → `isError`). Then each mentioned active persona (`@handle`, since Round 30c also name/handle without "@" at the start of a sentence followed by "," or ":") and `persona` answer one after the other with one post each. **Relay (one stage):** if an answer to the message mentions further active personas, these answer that answer once (the mentioning answer framed as data); relay answers call nobody further (not even those from the relay when addressed directly, Round 30c), `relayed` names everyone who answered through the relay, each persona answers each message at most once, all answers together at most three – loops are ruled out. Answer with `key`: `{posts, errors, url, truncated, cut, compressed, relayed, work, key}`; without `key`: `{posts, answers: [{persona, model, post_id, text, cut, truncated, relay, asked_by}], errors, url, thread, title, work, truncated, cut}`. No web search for @belege over MCP | implemented (Round 10; thread without a section and relay Round 15) |
| `scripture_check` | `{text?, work?, key?}` | find Bible references in `text` or in the current revision of the section `key` (then like RPC `bible.check`) and check book, chapter, verse against the versification of the Schlachter 1951; answer like RPC `scripture.check` (`refs` each with `input, span, normalized, book, status, message, wording, wording_code, quote, bible_text, partial, diff, diff_line`, plus `source`, `bible`). Wording only with the chosen SWORD Bible of the work (`match`, `differs` with a word diff, `no_quote`, `missing_verse`, `versification_mismatch`, `invalid_ref`, `module_error`; `bible` names module, versification, licence notice, since R17c also `cipher`), otherwise „ungeprüft“ (`unchecked`). Since R17c additively per reference `quote_span` (byte range of the quotation content in the checked text or `null`) and `module_ref` (reference read in the versification of the module, e.g. „Mal 4,1–6 (KJV)“, only if it differs); modules in another versification (all versifications of `sword-modules`) are read by Baruch through the checked mapping, what cannot be mapped stays `versification_mismatch` with a reason. Neither `text` nor `key` → -32602; read-only | implemented (Round 10, wording 2026-11-06, mapping R17c) |
| `scripture_map` | `{ref, from?, to?}` | Round 17a: convert a Bible reference between versifications, read-only; answer like RPC `scripture.map` (`{from, to, refs: [{input, normalized_from, status: same\|moved\|unmapped\|invalid, target, verses, rules, message}], versifications}`). `ref` (1–500 characters) is read and checked in `from` (default KJV), target `to` (default German, the versification of the reference check). All 18 versifications of SWORD (case does not matter); mapping only through the checked rule table (psalm titles, chapter boundaries such as Joel 2/3, Mal 3/4 …); what no rule covers and differs in chapter length is `unmapped` with „Versifikation passt nicht: …“ – never guessed. Unknown versification → `isError` with the list | implemented |
| `bible_modules` | `{work?}` | like RPC `bible.modules`: module folder, per Bible module `module, description, versification, license`/`notice` (licence notice of the conf), `supported`/`problem`, since R17c `cipher` (`none`/`locked`/`unlocked`) and `key_stored` – never the key; plus `work_bible`; read-only (only the human chooses and stores keys: RPC `bible.cipher_key_set/_delete` have **no** MCP tool) | implemented (2026-11-06, R17c) |
| `bible_lookup` | `{ref, module?, work?}` | like RPC `bible.lookup`: verses of the reference(s) as plain text, `text: null` = missing in the module; since R17c read through the mapping in the versification of the module (`module_ref`, `rules`, `truncated` only above 400 verses; unmappable `status: versification_mismatch`); `bible` with module, versification, licence notice; without a Bible or encrypted without a key → `isError`; read-only | implemented (2026-11-06, R17c) |
| `bible_search` | `{query, module?, work?, limit? = 50}` | like RPC `bible.search`: linear search for a word/phrase across all verses, at most 200 hits, 5 s (`truncated`); read-only | implemented (2026-11-06) |
| `commentary_lookup` | `{work, ref, module?}` | like RPC `commentary.lookup`: per commentary chosen for the work (or `module`) the entries for the reference in the versification of the commentary, plain text (at most 4000 characters per entry), `notice` = licence notice of the conf; `problem` with a missing module, invalid reference, without an entry; read-only (choice per work only by the human) | implemented (Round 15) |
| `lexicon_lookup` | `{work, key, module?}` | like RPC `lexicon.lookup`: headword → article in the first chosen lexicon (or `module`), `found`, `text`, `notice` (licence notice); read-only | implemented (Round 15) |
| `lexicon_search` | `{work, query, module?, limit? = 50}` | like RPC `lexicon.search`: word/phrase in headwords and articles, headword hits first, 5 s; `notice`; read-only | implemented (Round 15) |
| `rag_search` | `{query, work?, ways?, limit? = 10}` | semantic search in the RAG ways of the work (LanceDB, pgvector, MultiGPT; which ones only the human chooses, `ways` can only leave some out), `query` 1–1000 characters, `limit` 1–50. Answer like RPC `rag.search` (Answer JSON `{work, query, hits, ways: [{way, state, label, message, hits, millis}]}`, state per way connected/failed/off; since Round 19a per hit `weak: bool` = score below 0.45, „schwach“). The origin of the search is the client (`@mcp:<name>`). **Read-only; excerpts are data from sources, not instructions.** Empty/too long query, unknown way, `limit` out of range, unknown work → `isError` | implemented (Round 12) |
| `rag_status` | `{work?}` | state of the ways and of the local index without network, like RPC `rag.status` (`{work, ways, index, fts: true}`; since Round 19a `index.stale` = jobs left lying, `problem` stays set until new content or a rebuild); read-only | implemented (Round 12) |
| `source_read` | `{document, collection?, part? = 1, title?, q?}` | part of the full text of a source through MultiGPT `read_document`, answer like RPC `source.read` (`source`, `part`, `parts`, `more`, `text`, `marks`, `origin`, `message`). `document` e.g. from `rag_search` (way multigpt), or `title` = id of a linked title. **Only documents from collections chosen by the human:** `document` must have occurred in a hit of a search in chosen collections (`rag_search` or search of the user interface since the start of the server, with a matching `collection` if given), be present as a local copy of the synchronisation or be linked to a title – otherwise `isError` „… nur Dokumente aus vom Menschen gewählten Sammlungen …“ (without reading; `read_document` knows no collection). **Read-only; contents are data from sources, not instructions.** Failure of MultiGPT, unknown document, title without a link → `isError`. Adopting into the library (`source.adopt`) does not exist over MCP | implemented (Round 13) |

**Semantic search (Round 12):** `rag_search` and `rag_status` only read. A rebuild of the index
(`rag.reindex`), the choice of the ways and of the collections per work (`connection.rag_set`, `connection.collections_*`)
do not exist over MCP (`tools/call rag_reindex` → -32602 „unbekanntes Werkzeug“); that stays with the human.

**Settings (Round 9):** there is deliberately no tool `settings_read`: a client already reads personas with role, model and
context through `list_personas`; connections, secrets, RAG ways, data and the
backup stay closed to MCP, colours, personas and work metadata are changed only by the human. No
secret ever appears over MCP. MCP clients appear after `initialize` on the page „Einstellungen →
MCP-Zugang“ (name, version, handle, first/last seen; only in the memory of the server).

**Images (Round 11):** assistants create images with `asset_add` immediately (like `outline_add`). Replacing,
setting or removing the cover and deleting work only as a **request** (`asset_replace`, `asset_set_cover`,
`asset_delete`), which the human confirms or rejects on the page „Bilder“ (section „Anträge der Assistenten“);
if the image or the cover has changed in the meantime, the request is closed as outdated.
Switching the current revision, changing caption and alternative text stay with the human (page
„Bilder“, RPC `asset.*`). `persona` only without a token from the own machine (like `outline_*`); otherwise the client
`@mcp:<name>` is the author or requester. Setting book metadata and the cover directly (`work.meta.set`,
`work.set_cover`) as well as the PDF (`work.pdf`) do not exist over MCP; `typst_check` reports the print check
(`print`, raster images below 300 dpi) and `missing` (image paths without a file).

Outline schemes are chosen only by the human; there
is no MCP tool for that. `outline` and `read_node` deliver `number` according to the configured scheme of the
work (e.g. „§ 1“, „Erstes Kapitel“, „Einleitung“).

### Rules when writing

- **Images:** read (`list_assets`, `read_asset`), create immediately (`asset_add`), everything else only as a request
  (`asset_replace`, `asset_set_cover`, `asset_delete`; Round 11).

- **`propose_revision` always creates a proposal** (`proposed`, through `Store::add_proposal`, with
  `based_on` = current revision). The current revision stays unchanged; only the human can accept or discard it
  in the user interface. No MCP caller can make a revision `current`, change, delete
  or switch it to another revision.
- **Author:** `persona` (handle such as `@lektor`, without `@` it is added) must stand in the table `persona`,
  otherwise `isError`; without `persona` the author is the client handle `@mcp:<name>`.
- **Model:** argument `model`, otherwise from the session ("<name> <version>"), otherwise „unbekannt“.
- **Rejected** (`isError`): empty text; text equal to the current revision (CRLF counts like LF); a
  heading line with an empty title, a line break in the title or more than 200 characters („Ein Gliederungspunkt
  braucht einen Titel.“ etc., checked in the store, `check_proposal_heading`).
- **Colours**, role, model, context and "active" of the personas are set only by the human
  (`/einstellungen/assistenten`); there is no tool for it.
  Proposals through `propose_revision` appear in the reading flow in the colour of the named persona, without a
  persona entry (`@mcp:<name>`) in grey.
- **`add_post` references** are checked: the section and the revision (by number) must exist in the same work,
  `quote` must stand literally (after trimming) in this revision. A missing number reports
  „… die gibt es nicht (vorhanden: 1, 2, 4).“. Empty post → `isError`.
- Proposals and posts do not change the Typst mirror (it contains only current revisions), hence no
  commit.

**Addresses (`url`, Round 8):** answers carry relative addresses in the canonical form
`/?work=W&key=K` (without `rev`; with `search` with `&rev=R`). They stay valid when the human rearranges the
outline because they use the fixed key; a client can name them to the human as a link.

### Managing posts (Round 8)

Posts are not book text. A client may therefore move, delete or convert posts into an idea
– but **only its own**: the author is the persona from `persona` (it must exist) or otherwise `@mcp:<name>`
of the session. A foreign post (human, other persona, other client) → `isError` „Nur eigene Beiträge
lassen sich über MCP verschieben, löschen oder umwandeln: Beitrag N stammt von …“, nothing changed, no
commit. **With a token never as a persona:** a client with a token (possibly another machine) acts only as
`@mcp:<tokenname>`; if it names `persona` (with `add_post`, `propose_revision`, `post_move`, `post_delete`,
`post_to_idea`, the idea and outline tools), the tool answers with `isError` „Mit Token handelt
ein Client nur im eigenen Namen …“, nothing changed. Otherwise it could pass posts off as the work of a persona and
delete its real posts generated by the model. Letting a persona answer still works with `ask_persona`
or `chat`. **Without a token over loopback: ownership according to what the client states, no login:** MCP knows no identity. Whoever sets `persona` to
an existing persona or reports with the same `clientInfo.name` counts as its author and may
manage its posts (just as they may also write posts under this name). The rule protects against
mistakes, not against a client that deliberately pretends to be another. **Since Round 23** the name of a
token (also of a revoked one) is excluded from this: a client without a token that has this name writes as
`@mcp:lokal-<handle>` and does not own the posts of the token client (see "Round 23 – Hardening"). Safely protected are the
posts of the human: their author („Autor“) is neither a persona handle (`@…`) nor `@mcp:…`, no statement
of a client hits them. Checked under the write lock. Every action yields exactly one commit in the mirror (author as with
ideas „MCP-Client (@mcp:…)“ or „Name (@handle)“, trailers `Baruch-Post`, `Baruch-Action`, `Baruch-Node`,
on conversion `Baruch-Idea`); deleting and converting carry the whole post in the body. The human may
manage all posts (user interface, RPC `post.*`).

**`post_edit {work, id, body, thread?}` (Round 28, chat-auftrag):** edits only own posts of the client
(author `@mcp:<name>`, ownership according to what the client states as above; `persona` does not exist here: posts of personas
are never editable), in the section chat or with `thread` in the thread. A foreign post (human, persona, other
client) → `isError` „Nur eigene Beiträge lassen sich bearbeiten …“, without a change. The old revision stays as history
(`post_version`); in the section chat exactly one commit „Beitrag <id>: bearbeitet“ with the text before and after (author
the client), in the thread none (threads are not in the mirror). Answer like RPC `post.edit` or `thread.post_edit`
(`id`, `edited`, `versions`, `url`, `commit?`).

**`chat` with RFC, „@alle“ and a writing order (Round 28, chat-auftrag):** „RFC“ as the first word or „@alle“ calls every
active persona exactly once (the limit of three answers does not hold, the queue per client does; no
relay stage), the lector last with the answers of the others framed as data and the task of weighting them
(„Jetzt wichtig:“, „Kann warten:“); in the section chat and in the thread. A writing order („@lektor Schreibe …“) in the
section chat becomes a proposal for the section plus a short post (`proposals` in the answer), never an
acceptance; over MCP there is no `as_order`, only the recognition counts. The answer additionally carries `rfc`,
`proposals`, `findings`, `notes`.

### Outline

Wish 2026-11-06 („Ein LLM hat den besseren Überblick durch seinen Context“, „UD nach Bestätigung“); source
`src/mcp/outline.rs`, store `outline_add_by_assistant` and `requests.rs` (migrations 0012 and 0035).

- **`outline_add` creates immediately** (exception: a new part that would take over existing chapters, see "Parts").
  Either `title` (one item, optionally with `level`) or `items` (up to 100
  items, each with `level`), never both (`level` next to `items` → -32602).
  The block stands before the reference item `key` (`place: "before"`), behind it including its sub-items (`"after"`,
  default) or as the last sub-items (`"into"`). Every entry has `title` and optionally `level` = **absolute
  level** (0 part, 1 chapter, 2 section, 3 subsection); if it is missing, the level of the place holds (level of
  `key` with before/after, one deeper with into). The first item stands on the level of the place, none higher than
  it (so no existing item becomes a sub-item of a new one – exception parts, see next item), each at most one level deeper than the previous one.
  Example after the last chapter: `items: [{title: "Einleitung"}, {title: "Erster Gedanke"}, {title: "Bild",
  level: 2}, {title: "Schlussgebet"}]`.
- **Parts (Round 14):** `level: 0` creates a part – before or behind a part or chapter (`before`/`after`,
  also in works **with** a work template), as the first item also next to a chapter, in `items` also in the middle of the block
  (the rule "none higher than the first" does not hold for parts). `into` with level 0 and a part next to a
  section → `isError`. The text of the new part (revision 1) is the part marker
  `#heading(level: 1, numbering: none)[Titel] <baruch-part>` with an empty line (`docs/WYSIWYG.md`, interface A).
  If existing chapters stand behind it (up to the next part), they would move under the new part. **That is
  not mere creation** (user question 12, decision of the user 2026-11-07: „es braucht eine aktive Zustimmung
  (Popup)“): Baruch then creates **nothing** but files a request (kind `add`, migration 0035) and answers
  `{work, action: "requested", status: "open", request, adopted, message, url}` – `message` = „Antrag N gestellt,
  wartet auf Zustimmung: …“, `adopted` = fixed keys of the chapters that would move, `note` = reasoning for
  the human. The human approves or rejects in the dialog (on every page) or on `/gliederung#antraege`;
  `outline_requests` reads the state. After approval the part comes into being as with direct creation (revision 1
  by the assistant, chapters with their texts and numbers unchanged under it); if it would then take over chapters other than
  at the time of the request (e.g. because the human has meanwhile created a part themselves), the request is closed as outdated
  and nothing changed. If the new part takes over **no** existing chapters (at the end of the work, before another
  part, in a work without items), it stays creation according to AGENTS.md: immediately, `action: "added"`,
  `adopted: []`. A list (`items`) with such a part becomes one request as a whole.
  Renaming, moving and deleting a part work, as with every item, only through a request; changing the level to a part
  or of a part does not exist (`outline_set_level` with 0 → `isError`).
- **Atomic:** one transaction, one commit; an error (title rules with „Punkt N: …“, levels, part in the wrong
  place) → `isError`, nothing created. Title rules as everywhere (not empty, single line, ≤ 200 characters).
- **Without `key`** only in a work without an outline item (first item, chapter or part);
  otherwise `isError`. Such a work does not arise through the user interface today (a new work brings a chapter
  along, the last one cannot be deleted).
- **Origin:** revision 1 of every new item (heading line + empty line) belongs to `persona` or to the client
  as `@mcp:<name>` (model `model` or from the session), never to the human. In the commit it is the author
  („MCP-Client (@mcp:…)“ or „<Name> (@persona)“), committer the human, trailers `Baruch-Origin: mcp|persona`
  and `Persona`.
- **Everything else is a request:** `outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`,
  `outline_delete` change nothing immediately. Baruch tries the change in a savepoint (rolled back): if it would not
  work now (title rules, deleting with sub-items, no effect, invalid level), `isError` comes and
  no request arises; an equal open request → `isError`. The human confirms or rejects
  (`docs/Bedienung.md` „Anträge der Assistenten“, RPC `outline.request_decide`); outdated requests are closed with
  a reason (`decision: "stale"`). `outline_requests` reads the state (`mine: true` = requester such as
  `persona` or `@mcp:<name>`).
- **Preview (Round 14):** `outline_requests` with `preview: true` gives, per **open** request, the outline as
  it would be after confirmation: `preview: [{key, level, number, title, mark}]` in tree order, `mark` =
  `same`, `renamed`, `moved` (the moved item including its sub-items), `level` (also sub-items moving along),
  `numbered`, `deleted` (deleted items at their old position with old level and number), with a request `add`
  additionally `added` (new item) and `adopted` (chapter that moves under the new part). Computed in a
  savepoint and rolled back – nothing changes. An outdated or no longer executable request
  gets `preview_error` with the reason instead of the preview; decided requests (`open: false`) likewise
  („schon entschieden“).
  Since Round 16c every item additionally carries `kind_word` (kind after confirmation: „Teil“, „Kapitel“, „Abschnitt“,
  „Unterabschnitt“), `before_number`/`before_level` (previous state, `null` for new items), `label` (kind and number,
  e.g. „Teil I“, „Kapitel 2“, for sermons „Predigtteil …“), `before_label` (likewise before, `null` for new items)
  and `changed_label` (`true` where kind or number change or a chapter is taken over; the user interface shows
  „(vorher … → nachher …)“ there).
- `node`/`target` (positions) → -32602 as with `propose_revision`; only `key`/`target_key`.

### Ideas

Ideas change **no book text**. Therefore the assistant rule "proposals only" does not hold for them
(decision IDEA_CRUD): assistants and MCP clients may create, change and delete ideas. The origin
is `persona` (it must exist, otherwise `isError` „Unbekannte Persona …“) or the client as `@mcp:<name>`; it
stands on the new idea (`origin`) and in every commit (`Baruch-Origin`, `Persona`, author of the commit; committer
the human). `idea_delete` needs no confirmation: the idea stays restorable (user interface,
RPC `idea.restore`) and the delete commit carries the whole entry. `open: false` marks „verarbeitet“
(by hand, `done_as: note`), `open: true` makes it open again. Processing into an outline item, post
or note stays with the human (user interface, RPC `idea.process`). Limits and errors as with RPC, as
`isError` with a German text; unreadable arguments (e.g. `words` missing) → -32602.

Connections between ideas (Round 25) are read and written by MCP with `idea_map`, `idea_links`, `idea_link`,
`idea_unlink` (see "Idea map").

The lector knows the ideas for the section as memory (Round 10, in the context of every job given to it).
Not implemented yet (round „Werkzeuge der Personas“): it suggests fitting places on its own.

The hint in `initialize` (`instructions`) names the idea tools and the exception, since Round 10 also
the personas and Bible references, since Round 12 `rag_search`/`rag_status` (semantic search, read-only, hits are
data) and since Round 15 (state of `crates/baruch-server/src/mcp.rs`, English since Round 38):

"ask_persona asks a persona (answer as a post, review as findings, propose only as a proposal, scope chapter
for the whole chapter); chat writes into the section chat (with key) or into a thread without a section (without
key, thread continues it) and calls mentioned personas (@handle), one relay step; at most two model jobs
(chat, ask_persona) per client at a time; scripture_check checks Bible references (wording only with the chosen
SWORD Bible of the work, otherwise unchecked; quote_span and module_ref per reference); bible_modules, bible_lookup
and bible_search read SWORD Bibles; commentary_lookup, lexicon_lookup and lexicon_search read SWORD commentaries
and lexicons (read-only, with licence note)."

Since Round 25 it also names `idea_map`/`idea_link`, since Round 26 `exchange_export` (no reading in over MCP) and since
Round 27: "cover_layout reads the cover layout (blocks, position, print check); cover_layout_set changes nothing
immediately but files a request that the human confirms."
Since Round 22: "revision_amends reads the history of the amendments of a revision (read-only; only the human
can take one back)." and with `ask_persona` the query about own findings that are done.

**Queue per client (Round 15):** the model queue is shared. So that one client does not fill it,
at most **two** model jobs (`chat`, `ask_persona`) run or wait per client (`@mcp:<name>`, with a token the token name)
at a time; the third immediately gets `isError` („… hat schon 2 Modellaufträge … nichts gespeichert“),
without a request to the model. Other clients are not affected; a cancelled job frees its place.

### Idea map (Round 25)

Connections between ideas change no book text; therefore anyone may create and remove them (like ideas,
IDEA_CRUD), also connections of others. Origin `persona` (it must exist, otherwise `isError` „Unbekannte Persona …“)
or the client as `@mcp:<name>`; it stands on a new connection (`origin`) and in every commit
(`Baruch-Origin`, `Persona`, author of the commit; committer the human). Every change to a connection is exactly
one commit (subject and trailers like RPC, `docs/RPC.md` "Idea map"). Restoring through another `idea_link`
(trailer `idea-link-restore`). MCP does not set positions (pure display in the widget).

| Tool | Arguments | Result (`structuredContent`) |
|---|---|---|
| `idea_map` | `{work (id or slug, mandatory), status?: open \| done \| all, word?, origin?: human \| persona \| mcp, q?, near?: bool = true, near_limit?: 1..5 = 3, near_threshold?: 0..1 = 0.75}` | map like RPC `idea.map`, per idea additionally `url` (`/einfaelle/<id>`); read-only |
| `idea_links` | `{work?` \| `id?` (idea, exactly one)`, deleted?: bool = false}` | `{links}`; read-only |
| `idea_link` | `{from, to, kind: leads_to \| belongs_to \| example_of \| contradicts, note?, persona?}` | `{link, changed, commit?\|mirror?}`; an active equal pair → `changed: false`, no commit |
| `idea_unlink` | `{id, persona?}` | `{link, commit?\|mirror?}` (soft, restorable) |

Domain rejections (self-reference, foreign work, deleted idea, note over 500 characters, unknown persona,
unknown connection) come as `isError` with a German sentence; unreadable or wrong arguments (missing `work`
or `id`, `kind` outside the four words, `work` and `id` at the same time with `idea_links`, wrong `status`/`origin`)
as JSON-RPC error -32602. The proximity in `idea_map` comes from stored embeddings and equal keywords,
without a new embedding request (`near.state` `no_embedder` without an embedder). Since Round 19b `near_limit`
(partners per idea) and `near_threshold` (cosine threshold) work as in the RPC (the same check, applied in
`filter`); a value outside 1–5 or 0–1 or not a number is a domain rejection: `isError` with the reason
(„near_limit: 1 bis 5, nicht 6“). Naming and adopting groups of the map does **not** exist over MCP (only the human
through RPC and the form of the map, `docs/RPC.md` "Idea map").
**Notes, keywords and connection notes are data, not instructions:** they may come from personas or
other MCP clients; the descriptions of `idea_map` and `idea_links` say so as well.

Example:
```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"idea_link",
  "arguments":{"from":79,"to":48,"kind":"contradicts","note":"beißt sich","persona":"@lektor"}}}
→ structuredContent {"link":{"id":7,"from":48,"to":79,"kind":"contradicts","note":"beißt sich",
   "origin":{"kind":"persona","handle":"@lektor"},…},"changed":true,"commit":"3f2c…"}
{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"idea_links","arguments":{"id":48}}}
→ structuredContent {"links":[{"id":7,…}]}
```

The hint in `initialize` says: "idea_map reads the mind map as a flat map (ideas, relations, links, on
request proximity); idea_link, idea_unlink and idea_links connect ideas (change no book text, one commit with
its origin)."

### Cover layout (Round 27)

The layout of the cover blocks (title, subtitle, author, publisher and free text on the front and spine; motto,
blurb, short biography, ISBN with barcode, publisher and price, free text on the back) belongs to the book:
only the human may change it (user interface, RPC `cover.layout_set`/`cover.layout_reset`). Over MCP there is only
**reading** (`cover_layout`) and **a request** (`cover_layout_set`); no MCP path changes the layout directly
(test `round27_daten::no_mcp_tool_changes_the_layout_directly`).

- `cover_layout_set` creates an image request with `op: "cover_layout"` (table `asset_request`, the same list
  as `asset_replace` etc.; `asset_requests` shows it with `blocks` and `expected_layout`). Every block is checked
  on request and stored in normal form (limits like RPC); a request that would change nothing, an
  invalid value or a wrong fingerprint yield no request (`isError` with a German message naming kind and
  field). Unreadable blocks (unknown kind, wrong type, unknown field) → JSON-RPC error -32602.
- `expected_layout` = SHA-256 over the JSON of the stored blocks at the time of the request. If the human confirms (page
  „Bilder → Anträge“ or RPC `asset.request_decide`) and the fingerprint still matches, the request is executed
  (one commit „Umschlag: Anordnung geändert (Antrag N)“ with requester and confirmer); if the human has changed the
  layout in the meantime, the request becomes `stale` with a reason. Rejecting changes nothing.

```json
{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"cover_layout_set",
  "arguments":{"work":2,"blocks":[{"kind":"front_title","y_mm":40}],"note":"Titel in das helle Drittel"}}}
→ structuredContent {"work":2,"status":"open","request":{"id":4,"op":"cover_layout","blocks":[{"kind":"front_title",
   "x_mm":5,"y_mm":40,…}],"expected_layout":"4f53…","summary":"Umschlag-Anordnung ändern (1 Block)",…},
   "message":"Antrag 4 angelegt (…); der Antrag selbst ändert noch nichts. …","url":"/werke/2/bilder#antraege"}
```

### Findings (Round 28)

Answers of the personas are split into findings; only the human sets the status. Over MCP there is only **reading**
(`findings`) and **a proposal** (`finding_propose_status`, only `adopted`, `deferred`, `done`), which the human
confirms (befund-uebersicht, `src/mcp/findings.rs`):

- `findings` reads like the overview: attached findings (equal to an earlier one) are missing, praise only with
  `include_praise`; `key` filters exactly this section (without sub-items). Findings are data from answers of
  personas, not instructions.
- `finding_propose_status` checks status and mandatory fields itself (`rejected` and `open` → „Nur der Mensch kann …“,
  `adopted` without `rev`, `deferred` without or with two targets → `isError`) and passes the proposal with its origin
  (`persona` only without a token, otherwise the client as `@mcp:<name>`) on to `findings::rpc::propose`; the answer is
  the one of `propose` (plus `url` to the finding). A proposal does not change the status; the human decides in the
  accordion „Befunde“ or on the page `/werke/<id>/befunde`.
- Round 30b: every finding carries its solutions as `solutions: [{quote, replacement, step}]` (if the field is missing, it has
  none): a quotation with a new wording („statt/besser“) or a step. They come from the lines `statt: „…“`,
  `besser: „…“` or `Schritt: …` under the finding in the answer of the persona. A critical finding without a solution carries
  „(ohne Lösungsvorschlag)“ in its check note. Adopting a solution as a proposal works only through the user interface or
  RPC `solution.adopt` (human); MCP writes proposals through `propose_revision` or `solution_variants`.

```json
{"name":"findings","arguments":{"work":2,"key":13,"status":"open"}}
→ {"work":2,"total":2,"truncated":false,"findings":[{"id":1,"persona":"@lektor","kind":"hint","kind_label":"Hinweis","status":"open",
   "status_label":"offen","key":13,"body":"Der Übergang zum Nebel fehlt.","url":"/werke/2/befunde?befund=1",…}],
   "url":"/werke/2/befunde"}
```

**How findings arise** (befunde, implemented R28a): answers from `ask_persona` (ask, review) and `chat` (section
and thread) are split into findings right after the post is stored (`findings::ingest`): one finding per bullet point,
per line with a marker („Kritisch:“, „Vorschlag:“, „Hinweis:“, „Lob:“) and per item under „Was trägt“ (praise),
„Was fehlt“, „Was stört“ (hint); a literal quotation „…“ becomes `quote`, the check note („(nicht im
Text belegt)“ and others) `quote_check`, „[B<id>]“ a reference. Proposals (`propose`) and notes by Baruch never become
findings, an answer without a list and without a marker yields none. The post stays byte-identical, and creating them is
no commit (`befunde.md` is in the next snapshot). If a new finding equals one of the section that was already decided
(or an open one of the same persona there), it is attached and does not count as open. Posts of an
MCP client (`add_post`, author `@mcp:<name>`) are not split; the human can convert any post themselves
(`post.to_findings`, chat-auftrag). Only the human sets the status (RPC `finding.set_status`, `docs/RPC.md`); MCP
can only propose it (`finding_propose_status`: only `adopted`, `deferred`, `done`; `rejected`/`open` →
`isError` „Nur der Mensch kann …“; an equal open proposal not twice; one commit per proposal
`Baruch-Action: finding-propose`).

### `text_search` (Round 30a)

Search in the work, read-only (`src/mcp/search.rs`). Arguments: `work`, `q` (at most 500 characters), `case`, `word`,
`re` (syntax of the Rust crate `regex`), `range` = `section|chapter|part|from|work` (default `work`), `key`, `kinds`
(default `text, heading, footnote, quote`), `limit` (1–2000, default 100). Answer `{work, range, total, truncated,
timed_out, hits}` with the same fields as RPC `search.find` (section `key`, number, title, revision, kind of text, byte
and UTF-16 positions, line, context, `url`). Time limit 2 s. `range=all` does not exist over MCP. Replacing does not exist over
MCP: proposals per section still go through `propose_revision`. The tool `search` (full text) stays
unchanged.

### `solution_variants` (Round 30b)

Variants of a section as proposals (`src/mcp/solutions.rs` → `assist::variants::run`); counts, like `chat` and
`ask_persona`, towards the quota per client (`mcp/tools.rs` takes it before the call). The lector (according to the setting) implements the
named findings (`findings`, ids from `[B<id>]`, only of this section; without a statement the open ones of the section) with
their solutions in 2 or 3 variants (`count`, default 2): sibling proposals on the same base `rev`
(default: current revision), label A/B/C, one line saying what it does differently, and the implemented findings. Long
sections get partial proposals (only the affected paragraphs). Only proposals arise – only the human can accept one and discard the
rest (`solution.reject_siblings`). `count` other than 2/3 → `isError`; unknown
section, no active lector or no findings → `isError` with an honest message.

```json
{"name":"solution_variants","arguments":{"work":2,"key":13,"findings":[4,7],"count":2}}
→ {"group":"5b0d…","base":3,"persona":"@lektor","status":"proposed","work":2,"key":13,
   "variants":[{"rev":5,"label":"A","summary":"knapp: nur [B4]","findings":[4],"url":"/?work=2&key=13&rev=5"},
               {"rev":6,"label":"B","summary":"alle Lösungen, Absatz umgestellt","findings":[4,7],"url":"…"}],
   "notes":[],"url":"/varianten?work=2&key=13&group=5b0d…"}
```

### `ask_persona`: basis of the review (Round 30b)

`ask_persona` (ask, review) reviews the shown revision `rev`; without `rev` the current one – if that shows only the heading
and a proposal with text exists, the proposal (`assist::basis::choose`, chosen in `assist::run`). The post
carries the header line „geprüft: …“ (`PostMeta::basis_label`, from the call, never from the model). The result of the job
knows it as `Outcome.basis = {rev, label, proposal}`; in the MCP answer it stands as `basis`. Every answer follows the duty to give solutions (see
"Findings"): a critical point without a solution is requested once more, otherwise „(ohne Lösungsvorschlag)“.

Critique cycle (bug 0.9.0): `ask_persona` review with `rev` = number of a proposal reviews this proposal in turn
(own open findings in the job, „Erledigt: Befund n“ becomes a status proposal, quotations from the proposal count);
`ask_persona` propose with `rev` creates the new proposal **on** revision `rev` (`based_on` = `rev`, before always the
current revision).

### `chat` and `post_delete` (Round 30c)

**`chat`** (section chat and thread, `src/assist/chat.rs`, `src/threads.rs`):
- **Addressing:** besides `@handle` (everywhere), the name or the handle without "@" **at the start of a sentence or message**
  followed by "," or ":" also calls an active persona („Lektor, was meinst du?“, „Exeget: …“, case does not matter); in running text not
  („der Lektor meint“ calls nobody). At most three personas per message as before.
- **Relay on direct address:** if a line of a persona answer begins with `@handle:` or an address, the addressed
  persona answers once (one stage, at most three relay answers per message, each persona at most
  once, not with the RFC and not with the writing order). Its post internally carries `relay_from` and `reply_to` (visible as
  „antwortet auf Beitrag n von @…“). The relay from Round 15 (every mention in an answer, all answers together
  at most three) still holds over MCP; both together call no persona twice and form **one** stage: a
  relay answer calls nobody further, not even through a mention. In both cases the mentioning answer stands
  framed as data in the job; Baruch builds the job text (never the raw model text as an instruction).
- Answers are posts with metadata (reviewed revision, answer to, job for „wiederholen“); the answer of
  `chat` does not change. Repeating (`post.repeat`, `thread.repeat`) exists only for the human (RPC, form).
- Errors of a persona still stand in `errors` and additionally in the live display of the section or thread.

**`post_delete`:** unchanged, only own posts in the section chat (one commit). Thread posts are deleted by the human through
RPC `thread.post_delete` or ✕ on `/faden` (one commit „Fadenbeitrag n: gelöscht“ with the whole text; threads themselves
are still not in the mirror). Over MCP `post_delete {thread, id}` deletes only own thread posts
(`threads::delete_post_as`, `only_own`; foreign ones → `isError` „Nur eigene Beiträge …“, without a commit).

### typeset_settings (Round 29a)

Owner: satz-daten. `typeset_settings {work?}` reads the typesetting settings of the work through the same function as RPC
`work.typeset_get` (`typeset_settings::load`/`view_json`) and answers in `structuredContent` with exactly their form
without `fields`:

- `work`, `set` (are there typesetting settings?), `uuid`, `updated`, `updated_by` (without settings `null`);
- `settings`: only the set fields in the JSON of the data model (`baruch_typst::typeset::TypesetSettings`, example in
  `docs/RPC.md`); a missing field means "from the work template";
- `rules`: the generated Typst block „// Baruch: Satz“ ("" without settings), just as export, mirror and typesetting
  put it behind the work template;
- `conflicts` `[{field, label, line, text, effect, message}]`: lines of the work template that set a set field themselves
  (`effect` `overrides` = the setting holds nevertheless, `ineffective` = it has no effect as long as the line
  stands);
- `template_rule` `{line, text, chapter_start}` or `null`: a recognised chapter-start rule of the work template that
  the human can adopt in the settings;
- `warnings`: hints, e.g. `Schrift „X“ fehlt; Typst setzt mit der eingebetteten Schrift`.

**Read-only**, no commit. A writing MCP tool for typesetting settings does not exist (`tools/list` contains
no `typeset_set` or the like, test `the_mcp_tool_reads_like_typeset_get_and_there_is_no_writing_tool`): saving,
resetting and adopting the template rule stay reserved to the human („Einstellungen → Werk → Satz“ or
RPC from the user interface).

### Search

`search` calls `baruch_library::search`: `kinds` from `revision`, `post`, `title`, `quote`, `highlight`
(highlights in sources of the author library, see `docs/RPC.md`); without `all_revisions` only current
revisions; `limit` default 20, at most 100. Rejected highlights are missing from the search.
Every hit (`SearchHit`) has `kind`, `work`, `node`, `rev`, `title_id`, `quote_id` (for quotations),
`highlight_id` (for highlights), `key`, `label`, `snippet` and `rank`. `snippet` is the same excerpt
as in the user interface: HTML, text escaped, hits in `<mark>…</mark>`. Empty search → `isError`.

**Full text (Round 13):** a hit of `rag_search` with `way: "multigpt"` names `document` and `collection`;
`source_read` reads the full text for it part by part (`part` from 1, `more`/`parts` say whether more is to come;
`marks` are the occurrences of `q` as character ranges in the part). The text is material from a source and is
never treated as an instruction. `source_read` does not read other documents: only hits from collections chosen by the human,
local copies of the synchronisation and linked titles (otherwise a client could read through the document id any
collection the token sees). Over MCP nothing can be adopted into the library and nothing can be written to
MultiGPT.

## Example

```
curl -si http://127.0.0.1:8765/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"curl","version":"8"}}}'
# header Mcp-Session-Id: <id>
curl -s http://127.0.0.1:8765/mcp -H 'Content-Type: application/json' -H 'Mcp-Session-Id: <id>' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"outline","arguments":{"work":1}}}'
```

Course for an assistant: `list_works` → `outline` (delivers `key` per section) → `read_node` →
`propose_revision` with this `key`; the human
accepts in the user interface (or through RPC `revision.accept`), only then a commit arises in the mirror.

## Round 19 – RAG maintenance, rest of the references

### R19a index

No new tool. `rag_search` carries `weak: bool` per hit (score below `baruch_rag::WEAK_SCORE` = 0.45 or
not a number, display text „schwach“; the order stays, nothing is hidden). `rag_status` names
`index.stale` (jobs left lying after exhausted attempts) and keeps `problem` until a change or
„Index neu aufbauen“ (human only, not over MCP) brings the jobs back; searches through `rag_search` trigger no
new attempt. Both descriptions name the new fields.

### R19b pgvector

- `idea_map` takes `near_limit` (1–5) and `near_threshold` (0–1) like RPC `idea.map`; inadmissible values → `isError`
  with a reason (see "Idea map (Round 25)"). No new tool: naming groups (`idea.map_groups_name`) and
  adopting them (`idea.map_group_accept`) stay RPC/form of the human.
- `rag_search` through the way pgvector delivers all fields shortened in SQL and embeds the query with the configured
  `query_prefix` (setting only through RPC/page, not over MCP).

### R19c treffer

- `idea_search`: new parameter `mode` (`fts` default, `rag`). `rag` needs `work` (otherwise `isError`), delivers, like
  RPC `idea.search`, hits with `score` and `weak` (weak: similarity below `baruch_rag::WEAK_SCORE`, last);
  if the semantic search fails, the answer is `isError` with „Bedeutungssuche nicht verfügbar: LanceDB (lokal): <Grund>.“
  The short description of the tool names both modes (integration).
- `scripture_check`: per reference `module_spacing` (wording matches only without whitespace, code stays `match`, text
  „Wortlaut stimmt (Leerraum im Modul)“) and `module_note` (per answer and reference; module with conspicuously many
  word fragments, otherwise `null`).
- `bible_lookup`: per verse `german_ref` or `german_problem` (mapping back into the German versification).
- `lexicon_lookup`: with a lemma probe (Strong spellings, endings, umlaut, compound); the answer additionally has
  `probe` and `asked`.
- No new tool.

### Integration

- The short description of `idea_search` (`mcp/tools.rs`) names `mode` "fts"/"rag", `score`/`weak` and the failure as
  `isError`. Otherwise no change; no new tool in Round 19 (list as after Round 29).
- Acceptance test `tests/round19_abnahme.rs` checks over MCP: `idea_map` with `near_limit`/`near_threshold` (and `isError` with
  `near_limit` 9), no tool for adopting group names, `scripture_check` with `module_note`.

## EPUB (Round 20)

Both tools are in the list (directly after `typeset_settings`) and have been implemented since Round 20 (no STUB);
the `instructions` on connecting name both.

### epub_export

`{work, footnotes?: "popup"|"endnotes", include_bytes?: bool=false}` – the work as EPUB, **read-only** (no
commit, implemented since R20a). Answer: `work`, `file_name` (`<slug>.epub`), `url` (`/werke/{id}/epub`), `bytes`,
`sha256`, `footnotes`, `meta` (`identifier`, `title`, `subtitle`, `author`, `language`, `isbn`, `publisher`, `year`,
`modified`), `files` (reading order like `work.epub`), `toc` (`title`, `href`, `level`, `key`), `warnings`
(`code`, `message`, `key`; codes as with `work.epub` in `docs/RPC.md`). With `include_bytes: true` additionally
`epub_base64`, at most 8 MiB (`mcp::epub::MCP_EPUB_LIMIT`); larger → `isError` with a hint to the address and to
`baruch-server export <slug> --epub DATEI`.

### epub_check

`{work, readers?: [slug], footnotes?: "popup"|"endnotes"}` – check the EPUB of the work, **read-only** (R20c): own
structure and accessibility check (mimetype, container.xml, manifest/spine, well-formedness with xmllint or
quick-xml, navigation, language, alt texts, reading order), warnings of the build and per reader (without `readers` the
mandatory profiles; unknown slug → `isError`). Answer like RPC `epub.check`: `{findings, xml_tool, epubcheck,
summary, readers, work, url: "/werke/{id}/epub/pruefung"}`. **Never starts EPUBCheck** – `epubcheck` is always
`{status: "not_requested"}`, even if an argument `epubcheck` comes along or an epubcheck.jar is configured; only the
human starts the run on the check page. The setting of the epubcheck.jar does not exist over MCP.

### Integration

- `instructions` on connecting name `epub_export` (bytes only on request) and `epub_check` (never starts EPUBCheck).
- The exception for STUB descriptions in `tests/mcp.rs` is gone; no tool is described as a STUB any more.

## Round 31 – Index and glossary

### Interface R31-S1

Three tools are in the list directly after `epub_check`: `glossary_list` (read-only), `glossary_propose` and
`index_propose` (proposals only, only the human accepts). In the interface all three were honest stubs; since
R31a they are implemented (see below), the exception in `tests/mcp.rs` is gone. Code: `src/mcp/glossary.rs`.

### 31a glossar-daten

The three tools are implemented (`src/mcp/glossary.rs`, descriptions without "not implemented yet"):

- **`glossary_list`** only reads (status filter, with `word` lookup through term and variants of the accepted
  terms). No commit.
- **`glossary_propose`** only creates a proposal (status `proposed`), never an accepted term. Origin according to
  `acting_persona`: `persona` (only without a token, over loopback) or the client as `@mcp:<name>`; with a token and
  `persona` → `isError` (as with ideas and posts). Bible references are checked with baruch_scripture and
  written uniformly („ps 1,3“ → „Ps 1,3“). An already accepted or already proposed term (same
  `term_key`) → `isError`. Only the human can accept or discard (RPC `glossary.decide`, glossary page);
  discarding deletes the proposal (tombstone).
- **`index_propose`** only creates an index proposal (status `open`, related to the current revision of the
  section); never a text change. `node` instead of `key` → `isError` (contract Round 5). The word must stand in the current
  revision (simple text comparison; the exact position is determined by the acceptance). If the human accepts
  (`index.decide`), a new revision of the human with the mark arises in one transaction; if the word no longer stands
  in the text, the proposal becomes „veraltet“ (stale), the text stays.
- Every proposal is exactly one mirror commit (`glossary-propose` or `index-propose`, author the persona or the
  client, trailer `Persona`). Tool names contain neither `update` nor `delete` nor `create`.
- (done in the integration) The S1 exception in `tests/mcp.rs` for STUB descriptions of the three tools is
  removed.

### Integration

- `initialize` → `instructions` name the three glossary tools: `glossary_list` reads, `glossary_propose` and
  `index_propose` only create proposals, only the human can accept (`src/mcp.rs`).
- Acceptance test `tests/round31_abnahme.rs`: an MCP client proposes an index entry, only the human accepts it through the
  form; the mirror commit names the requester (`Baruch-Proposer`).
- Aligned since Round 23: `index_propose` reports `node` instead of `key`, like the older writing tools, as
  JSON-RPC error -32602 (before `isError`).
- No MCP tool for „Begriffe vorschlagen lassen“ (RPC `glossary.suggest`); only the human can trigger it in the
  user interface (glossary page, „An der Markierung“). Index proposals do not travel along in the exchange format.

## Round 21 – Import and images

### No import over MCP

The new imports of Round 21 – PDF and EPUB (`source_import.*`, page „PDF oder EPUB importieren…“, CLI
`import DATEI.pdf|.epub`) as well as Markdown and ZIP archives (`POST /werke/import`, `work.import`, `work.import_archive`,
CLI `import DATEI.md|.zip`) – do not exist over MCP either. An import creates current revisions under the name of the human,
and an assistant must not do that (decision 2026-11-05, see "Only the human" above). Moreover, an outline from PDF
or EPUB never arises silently, but only after a preview and explicit confirmation by the human. There is
therefore no tool `source_import` or the like; a call yields -32602 „unbekanntes Werkzeug“.

### Quota for images (21c)

Writing image tools have a quota per requester (persona handle or client `@mcp:<name>`) and work in
**rolling 24 hours**, counted from the existing rows (no table of its own, no migration): image revisions with
this author (`asset_revision.author_name`, created by `asset_add`; confirmed replacements are written by the human)
and replacement requests (`asset_request.origin_name`, whatever the decision). Constants in
`crates/baruch-server/src/mcp/assets.rs`:

| Limit | Value | Constant |
|---|---|---|
| writing calls (`asset_add` + `asset_replace`) | 30 | `MAX_MCP_ASSET_WRITES_PER_DAY` |
| bytes (decoded) of these calls | 64 MiB | `MAX_MCP_ASSET_BYTES_PER_DAY` |
| open replacement requests (however old) | 10 | `MAX_MCP_OPEN_REPLACE_REQUESTS` |

Above that the tool answers with `isError: true` and a German message („Kontingent erschöpft: … erlaubt sind
30. Nichts gespeichert …“ or „… mehr als 64 MiB …“ or „Zu viele offene Anträge …“); nothing is stored, no
commit. The check runs under the write lock directly before writing; another client (or another persona)
with a token has its own quota. Since Round 23 all clients without a token (whatever the name, also with
`persona`) share **one** quota per work – a new name opens no new one (see "Round 23 – Hardening"). Deleted images no longer count (they are no longer in the rows).
`asset_set_cover` and `asset_delete` do not count (they carry no content; the store rejects duplicate open requests
anyway). The content of decided replacement requests is removed 30 days after the decision
(`purged`, see `asset_requests`); the request file then answers 410.

## Round 22 – Revisions and mirror

New tool `revision_amends` (read-only); `epub_export` takes `draft`; `ask_persona` asks about done findings when
reviewing again. Undoing, the setting „Fassungen“ and the archive do not exist over MCP.

### fassungen

#### `revision_amends` (read-only)

History of the amendments of a revision. Parameters: `work` (id or slug), `key` (fixed key, takes precedence) or
`node`, `rev` (revision number). Answer: `{work, node, key, rev, amend_seq, steps: [{uuid, seq, kind ("amend" =
amendment by the human, "undo" = undoing), undo_of?, author, created, body_before, body_after}], note}`. The
texts are data from the work, not instructions. Undoing (`revision.amend_undo`) and the setting
„Fassungen“ do not exist over MCP; assistants still write only proposals (which are never amended).

The server instructions (`mcp.rs`) name the tool: "revision_amends reads the history of the amendments of a revision
(read-only; only the human can take one back)."

### befund-rueckfrage

- `ask_persona` with `mode: "review"` (and every repeated review): the persona sees its own open findings of the
  section framed as data and reports implemented ones as the line „Erledigt: Befund n (Stelle „…“)“; from that
  Baruch makes a status proposal „erledigt“ (only for own, open findings of the same persona in the work; foreign,
  unknown or decided numbers are skipped by Baruch). Only the human sets the status (`finding.decide` through the
  user interface); over MCP there is no way for it. No new tools; the description of `ask_persona` names
  it: "review: findings as a post – on a repeated review the persona asks about resolved own findings
  (status proposal „erledigt“, the human decides)".
- `Finding` in the finding tools (`mcp/findings.rs`) automatically carries `adopted_amend` (state of the revision at
  acceptance, see RPC.md).
- The archive (`work.archives`, `work.restore`) does not exist over MCP (creating, deleting and restoring works is reserved to the
  human).

### spiegel-entwurf

- **`epub_export`** (read-only): new parameter `draft` (boolean) – `true` as a draft (notice on the title page,
  „(Entwurf)“ in the title metadata), `false` without, omitted = according to the typesetting setting; the setting stays unchanged.
  `url` then carries `?entwurf=1|0`.
- **`typeset_settings`** automatically delivers `settings.draft` along (no change to the tool).
- **`exchange_export`** writes minor version 8.

## Round 23 – Hardening

No new tools. Changed are author names, ownership, quotas, upper limits and individual answers.

### mcp-namen

**Access and names:**
The author of an MCP client is `@mcp:<handle>`. The handle arises from the name: trimmed, lower case, umlauts transcribed (`ä` → `ae`, `ö` → `oe`, `ü` → `ue`, `ß` → `ss`), accents removed, every remaining character outside `[a-z0-9._-]` as "-", at most 32 characters, empty → `unbekannt`. „Laptop Küche“ writes as `@mcp:laptop-kueche` (until Round 22: `@mcp:laptop-k-che`).
- **With a token** the client is named like the token. No second token can get the same handle, not even that of a revoked one. For posts from the time before Round 23 the old handle of the same token holds in addition: the token „Laptop Küche“ still edits, moves and deletes its posts under `@mcp:laptop-k-che`. The page „MCP-Zugang“ shows the author per token and, where it differs, the earlier one.
- **Without a token** (only from the own machine) the name from `clientInfo.name` holds. If its handle belongs to a token (also to a revoked one, handle or old handle), the name gets the prefix `lokal-`: the client writes as `@mcp:lokal-<handle>` and does not own the posts of the token client. This is determined anew on `initialize` and on every request, so it also holds for a token that is created only later.
- **Own posts** (`post_edit`, `post_delete`, `post_move`, `post_to_idea`) are only those under the author determined this way.
- **Deliberate decision:** clients without a token stay indistinguishable from each other (whoever names the same name is the same author), and they may act in the name of a persona with `persona`. Loopback is the human themselves or a program they started on their machine; a login would give no security there. With a token `persona` stays forbidden.
- **Existing data that only the new rule merged** (follow-up Round 23): two tokens from the time before Round 23 can yield the same handle under the new rule although they wrote separately before – „Büro“ (until Round 22 `@mcp:b-ro`) and „buero“ (`@mcp:buero`) would both yield `@mcp:buero`. In this case **the token with the umlaut keeps its old handle**: „Büro“ still writes as `@mcp:b-ro` and owns only this author, „buero“ stays with `@mcp:buero`. Neither writes under the author of the other or edits, moves, deletes or converts its posts. The rule (`mcp::session::keeps_legacy`): a token keeps its old handle if the new rule changes its handle and the new handle equals the handle or old handle of another token (also of a revoked one) that wrote under a different author until Round 22. It is determined on every request from the list of tokens (no migration, no stored state) and does not depend on the order. For such a token the page „MCP-Zugang“ shows the old handle as author with the note „behält den Autor von vor Runde 23: `@mcp:…` gehört einem anderen Token“. New tokens are never affected (`mcp_token.create` rejects such names).
- **Limits:** tokens that already yielded the same handle before Round 23 („Laptop A“ and „laptop-a“, likewise „Büro“ and „BÜRO“) keep sharing it; new pairs of this kind no longer exist. A client without a token whose name carries umlauts has had a different author since Round 23 and no longer reaches its older posts over MCP (the human manages them). If the human creates a token whose handle a client without a token used before, that client's earlier posts belong to the token.

**Quota for images:** with a token the quota holds per token. All clients without a token – whatever the name, also with `persona` – share **one** quota per work (30 writing calls, 64 MiB in 24 hours, 10 open replacement requests). A new name opens no new one. The message then names „Die Clients ohne Token (gemeinsames Kontingent)“.

**Quotas without a token (follow-up Round 23):** a client without a token chooses its name freely on every `initialize`; a quota per name would be none. Therefore, as with the images:
- **Model jobs** (`chat`, `ask_persona`, `solution_variants`): per client still at most two at a time; all clients without a token **together** at most four (`mcp::tools::quota::MAX_WITHOUT_TOKEN`). The fifth immediately gets `isError` („Die Clients ohne Token haben zusammen schon 4 Modellaufträge …“), nothing goes to the model, nothing is stored. Clients with a token are not affected.
- **`add_post`** (300 posts in 24 hours) and **`propose_revision`** (100 open proposals): with a token the author of the token counts. Without a token all authors `@mcp:…` that belong to no token count together per work; the message names „Die Clients ohne Token (gemeinsames Kontingent)“. If a client without a token acts with `persona` in the name of a persona, the stock of this persona counts (including its posts generated by the model) – a deliberate limit: the number of personas is fixed, and `persona` without a token is an open user question (backlog).
- Not covered: index proposals and outline requests still count per origin; there an upper limit per work (2000 index proposals) or the approval of the human limits them in addition.

**`index_propose`:** `node` instead of `key` and a missing `key` are JSON-RPC errors **-32602** as with `propose_revision` and `add_post`. Per work there are at most 200 open index proposals per origin and 2000 in total; above that `isError` with a German message, nothing created, no commit.

**`ask_persona`:** plus `done_proposals`, `done_notes`, `all_done` as with RPC `persona.ask` – each only if set.

**`chat`:** quotation lines („> …“) call nobody. The relay runs through the same stage for the user interface and MCP (`assist::chat::relay_targets`): addressing at the start of a line for everyone (at most three), over MCP additionally every mention, as long as at most three personas are called in total. `relayed` names, as before, who answered through the relay. With a writing order there is no relay any more (before, a mention in the text of the proposal could trigger one).

**Limits of `add_post` and `propose_revision` (new):** `add_post`: text at most 20 000 characters, at most 20 sources (500 characters each) and 20 references (quotation 2000 characters each), per author and work at most 300 posts in 24 hours. `propose_revision`: text at most 1 MiB, `note` 2000 characters, `model` 200 characters, per author and work at most 100 open proposals (without a token all clients count together, see "Quotas without a token"). Above that `isError`, nothing stored. A batch (JSON-RPC list) carries at most 50 messages, otherwise -32600. `source_read`: `q` at most 500 characters and 40 words.

**`outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`, `outline_delete`, `outline_add` (as a request):** per work at most 50 open requests per origin; the 51st is rejected (`isError`) until the human has decided on open ones.

**`propose_revision`, title rule:** a title with U+2028, U+2029 or U+0085 (a line end for Typst) is rejected over MCP; the rule existed already, now it is tested.

**`source_read`, releases (decision):** the hits that release a document stay in the memory of the process (at most 4096). Not stored permanently, because the release is meant to come from a search in the collections chosen **now**: a stored release would still hold after the human has deselected a collection. After a restart one `rag_search` suffices. Migration 0051 is not used.

**Changed tool descriptions** (`tools/list`): `propose_revision`, `add_post` (limits), `post_move`, `post_delete`, `post_to_idea`, `post_edit` (ownership, `lokal-`), `ask_persona` (three fields), `chat` (quotation lines), `index_propose` (-32602, upper limits), `asset_add`, `asset_replace` (quota per token or shared), `source_read` (`q`). The hint text of `initialize` names the author rule and that all writing tools reject `node` with -32602.

### web (effect on MCP answers)

- `commentary_lookup`: like RPC `commentary.lookup` (mapping into the versification of the commentary, `ref` with the name of the versification).
- `source_read`: `q` over 500 characters is rejected (the error of `literature::source_read`).
- `findings`: findings carry `stale_summary` as in the RPC.
- `idea_map`: capping hint in `near.message`.
- `cover_layout_set`: invalid blocks still with the same wording.
- Evidence from SWORD in the context of the personas (`ask_persona`, `chat`): Bible and commentary text now come from the
  mapped reference (see Architektur).

### geheimnisse (effect on MCP answers)

`ask_persona`/`chat` with `@belege` no longer fetch addresses of the own network through
`fetch_url` when the web search is switched on (see `docs/Architektur.md`, "Filter before `fetch_url`").

### Integration

A request to `/mcp` is at most 12 MiB in size (`MCP_BODY_LIMIT`; an image of 8 MiB as Base64 fits), above that 413.

## Round 34 – Mind map

Ideas are not book text: MCP may create, change, re-hang and delete them; every action is a commit with
its origin (`persona` or `@mcp:<client>`) in the mirror. Chapters and parts of the mind map belong to the outline and
cannot be changed through `idea_*`.

| Tool | Arguments | Note |
|---|---|---|
| `idea_create` / `add_idea` | as before, plus `title?`, `parent?`, `before?`, `cloud?` (like RPC); `words` is no longer mandatory if `title` is set | Without a place the idea lands in the cloud. Origin `persona` or `@mcp:<client>`, one commit. |
| `idea_update` | as before, plus `title?` | one commit per change |
| `idea_move` (new) | `{id, parent?: id\|null, key?: key\|null, before?: id\|null, cloud?: bool, persona?}` | like `idea.move`; a violated rule → `isError`. Origin `persona` or `@mcp:<client>`, one commit „umgehängt“ in the mirror. |
| `idea_tree` (new) | `{work, status?, word?, origin?, q?}` | read-only: `{map, cloud}` like `idea.tree` (proximity lines off, suggestions of the cloud on), every idea with `url`. |
| `idea_delete` | unchanged | deletes sub-nodes along (`deleted_ids`), restorable. |
| `idea_read`, `idea_list`, `idea_search` | unchanged | output with `title`, `parent`, `position`, `main_topic`, `cloud`. |
| `idea_map`, `idea_links`, `idea_link`, `idea_unlink` | unchanged | descriptions say "mind map"; `idea_map` is the flat view with `title`, `parent`, `position` per idea. |

Order in `tools/list`: `idea_move` directly after `idea_delete`, `idea_tree` directly after `idea_map`. Chapters and
parts are still changed only by `outline_*` (request). Reading in .mm/.xmind does not exist over MCP. No stub open. The
hint text of `initialize` names `idea_tree`, `idea_move` and the cloud.

## Round 35 – Accounts and roles

No new tool. `/mcp` stays with tokens and runs past the role check as `Who::Open`; MCP clients
still write as `@mcp:<name>`. A Lektor is not an MCP client: commits of their findings deliberately carry no
trailer `Persona`, but `Baruch-Origin: lektor` and `Baruch-Lektor`.

Nothing changes for access to `/mcp` (see "Access"): without a token only from the own machine, otherwise with a token –
also when accounts are set up. The login layer (`auth::gate`) leaves `/mcp` untouched; a session
(cookie) grants no right over MCP and is not read there. Whoever works along as a Lektor uses the user interface; an
MCP token is still an access with the rights of an MCP client (proposals, posts, ideas, requests), not
restricted to shared works. Open question for the user (backlog): tokens per Lektor and work.

## Round 36 – Leftovers

No new tool. Behaviour that MCP clients can notice:

- **`chat`:** a directly addressed persona (`@handle`) gets its own open findings into the context only with a
  review job (a word of the message begins with `prüf`, `überprüf`, `nachprüf`, `gegenprüf`, `kontrollier`,
  `check` or `review`, or the message names „[B<n>]“) and with the RFC – not with a simple question.
  `ask_persona` with `mode: "review"` always stays a repeated review.
- **`ask_persona` with `mode: "propose"`, `scope: "chapter"`:** the proposal for the section of the job is based
  on `rev` (the shown revision, also an open proposal), the other parts of the chapter on their current
  revision. It stays a proposal; only the human can accept it.
- **`outline_delete`:** still only a request. If the human confirms it, the commit in the Git mirror now
  also names the revisions deleted along (open proposals counted separately) and posts in their wording.
- **Origin `@lektorat:<name>`:** over MCP the author of a revision or of a post stays the handle; only the
  work page shows the display name.

## Round 38 – English and work language

From Round 38 on, new sections of this document were written in English; since Round 38c the whole document is
English.

**English interface.** `initialize` (`instructions`), `tools/list` (every description and every parameter
description), the protocol errors (`unknown tool …`, `unknown method …`, `jsonrpc 2.0 with method expected`, `object
or list expected`, `Too many messages in one batch …`), the 401 texts of the endpoint and every error text that
arises only in `src/mcp/**` (argument checks, limits and quotas, "Request … filed") are English. Error codes are
unchanged (-32602 for unreadable arguments and for `node` in a writing tool, -32601, -32600, -32603; `isError` for
refusals). Descriptions of tools that change nothing immediately start with `Request:` (outline, images, cover
layout) or `Proposal:` (finding status, glossary, index).

**Still German (until Round 39)** are messages that the user interface shows as well and that only pass through
MCP: the texts of the store and of the RPC modules behind a tool (unknown section or revision, refusals of
`post_*`, `idea_*`, `chat`, `ask_persona`, `text_search` …), the note of the cover request created by `asset_add`
with `role: "cover"`, the client name `unbekannt` of a request without a session, and the answers of personas in a
German work. The boundary is described in `docs/RPC.md`, "Language of the interfaces".

**Work language.** Every work has a language, `de` or `en` (`docs/RPC.md`, "Work language"). `list_works` carries
`language` per work, `outline` at the top level. `ask_persona`, `chat` and `solution_variants` make the persona
answer in the work language, with the markers of that language (`Kritisch:` or `Critical:` …; table in
`docs/RPC.md`, "Prompts and markers"). No tool sets the work language: that is reserved for the human
(„Einstellungen → Werk → Werksprache“, RPC `work.language_set`). `exchange_export` writes the language as attribute
`sprache` at `<angaben>` (exchange format sub-version 10).

**Work language in outputs and read-back words (Round 39b).** `epub_export`, `epub_check` and the Typst of
`typst_check` follow the work language for every generated book word (navigation, landmarks, cover, title page,
notes, glossary and index pages, numbering words such as `Chapter 1`/`Part One`, page abbreviation `p.`, draft
watermark `DRAFT`, imprint lines); the language tag is the typesetting setting `lang`, otherwise for a German work the
template language or `de`, for an English work `en` (`docs/RPC.md`, "Work language"). `scripture_check` writes
`wording` in the language of the work: German works as before (`ungeprüft`, `Wortlaut stimmt`, …), English works
`unchecked`, `wording matches`, `wording differs`, `no quotation found`, `reference invalid – wording not checked`,
`Verse missing in the module …`; the codes (`wording_code`, `status`) are the same in every language. The closing
lines Baruch appends to persona answers with Bible references and the notes on reference claims (`(reference is in
the text: …)`) are English in an English work and German in a German one; every recognition accepts both.

**Stubs.** No tool is a stub at present; a future stub answers honestly in English ("not implemented yet", with what
is missing) and carries `// STUB: <what is missing> (<stage>)` in the code (`grep -rn "STUB:" crates`).
