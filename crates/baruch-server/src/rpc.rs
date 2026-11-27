// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON-RPC 2.0 under `POST /rpc`: the core as a service. The same methods are served by `POST /act`
//! for forms without JavaScript.
//!
//! Every method optionally takes `work` (work id, number); if it is missing, the default work applies
//! ([`AppState::default_work`]: now the smallest id), an unknown work results in `INVALID_PARAMS`.
//! `work.*` except `work.get`/`work.export` is answered by [`crate::works`] before the default work is chosen
//! (`work.update`/`work.delete` require `work`), `outline.*` by [`crate::outline`]. `persona.*`
//! ([`crate::personas`]) and `idea.*` ([`crate::ideas`]) likewise run without a work, Round 7.
//!
//! Methods:
//! - `work.list/meta/create/import/update/delete`: manage works ([`crate::works`], `docs/RPC.md` „Werke“)
//! - `work.get` → work with outline, revisions, personas and lector
//! - `work.export` → `{typst}`: work template and current texts as one Typst file
//! - `node.get {node}` → one section with revisions and chat
//! - `revision.accept {node, rev}` / `revision.reject {node, rev}` → `{node, rev}` (to be shown afterwards);
//!   accepting is switching to a proposal, rejecting stays a status for proposals
//! - `revision.set_current {node, rev}` → `{node, rev}`: mark "current" on any other revision.
//!   On switching and accepting the outline follows the heading line of the revision
//!   (`Store::switch_to`); if it brings another level, the answer carries `outline`
//!   (outline answer without mirror), and exactly one commit is created (`mirror::record_switch`)
//! - `revision.save {node, body, based_on?, based_on_amend?, mode?}` → new revision by the human, current at once,
//!   parent `based_on` (missing: the current revision); the editor saves through `outline.save`
//! - `outline.*`: rearrange the outline ([`crate::outline`], `docs/RPC.md` „Gliederung“)
//! - `revision.update {node, rev, body?, note?}` → `{node, rev, changed}`: change a revision (human only)
//! - `revision.delete {node, rev, confirm: true}` → `{node, rev, deleted, reparented}`: delete a revision
//!   (human only, not the current one), children hang on the parent; without `confirm: true` → `INVALID_PARAMS`
//! - `revision.tree {node?, chapter?}` → `{revisions}` or `{sections}`: revision tree without texts
//! - `post.add {node, body}` → `{node, key, id}`: post in the section chat
//! - `post.delete {id, confirm: true}`, `post.move {id, target_key | target}`, `post.to_idea {id, words}`:
//!   manage posts ([`crate::posts`], Round 8; human only, one mirror commit each)
//! - `ui.render {key?, node?, rev?, view?, pick?, lq?}` → `{tree, editor, assistants, url, title}` (HTML of the
//!   three areas; `key` takes precedence over `node`, `url` is the address in key form `/?work=W&key=K&rev=R…`)
//! - `library.list` → `{titles: [TitleEntry]}` (titles with quotations)
//! - `library.search {query, kinds?, work?, all_revisions?, limit?}` → `{hits: [SearchHit]}`:
//!   full-text search over work and library, the tool of the lector; `work` is the current work.
//! - `literature.render`, `highlight.list/add/update/delete/cite`: literature popup and highlights,
//!   answered by [`crate::literature`] (`docs/RPC.md`, „Literatur und Hervorhebungen“).
//! - `wysiwyg.parse/serialize/style/diff`: typeset view ([`crate::wysiwyg`], `docs/RPC.md` „Satzbild“);
//!   image islands carry `<img>` with the alternative text from `Store::asset_list`
//! - `numbering.schemes/preview/set`: outline schemes ([`crate::numbering`], `docs/RPC.md`
//!   „Gliederungsschemata“); `outline.set_numbering` stays
//! - `asset.list/get/revisions/add/replace/set_current/update/delete`: images as Base64 or
//!   SVG text ([`crate::assets`], `docs/RPC.md` „Bilder“); human only, not through `/act` and MCP
//! - `persona.list/set_color`: colours of the personas ([`crate::personas`], `docs/RPC.md` „Personas“);
//!   `set_color` human only, not through `/act` and MCP
//! - `idea.create/get/list/search/update/delete/restore/process/move`: ideas ([`crate::ideas`],
//!   `docs/RPC.md` „Einfälle“); origin always the human, `delete` requires `confirm: true`. Round 34: `move`
//!   re-hangs an idea in the mind map, orders it or puts it into the cloud
//! - `idea.map/links/link/unlink/link_restore/map_pos_set/map_pos_reset/tree`: mind map (idea map,
//!   [`crate::idea_map`], Round 25, `docs/RPC.md` „Gedankenkarte“); origin always the human. Round 34: `tree`
//!   delivers the mind map as a tree together with the cloud next to it, `map` stays the flat view
//! - `finding.list/get/set_status/decide`, `finding.matrix`: findings ([`crate::findings`], Round 28, `docs/RPC.md`
//!   „Befunde“); only the human sets the status
//! - `preview.typst {key?, page?}`: Typst typesetting of the work with diagnostics ([`crate::typeset`], Round 8),
//!   read only
//! - `persona.ask/review/propose/chat/prompt_get/set_prompt/lector_get/set_lector`: personas with
//!   model call and role prompts ([`crate::assist`] through [`crate::personas`], Round 10)
//! - `model.list/status/probe`: model backend Eliezer ([`crate::models`], Round 10); human only,
//!   not through `/act` and MCP
//! - `scripture.check {text, work?, module?}`: check Bible passages; wording only with a chosen SWORD Bible,
//!   otherwise „ungeprüft“ ([`crate::scripture`], Round 10, wording 2026-11-06), read only
//! - `scripture.map {ref, from?, to?}`, `scripture.versifications {}`: convert passages between versifications
//!   ([`crate::scripture_map`], Round 17a), read only
//! - `bible.modules/set_dirs/set_work/lookup/check/search`: SWORD Bibles ([`crate::bibles`], 2026-11-06);
//!   `set_dirs`/`set_work` human only, not through MCP
//! - `bible.marks {work?, key, rev?}` (check display, read only) and `bible.cipher_key_set/_delete` (keys
//!   of encrypted SWORD modules, human only, not through `/act` and MCP; [`crate::bibles`], Round 17c)
//! - `work.timing/timing_set`, `timing.count`: time guard ([`crate::timing`], Round 16b, `docs/RPC.md`
//!   „Zeitwächter“); `timing_set` human only, not through `/act` and MCP
//! - `work.exchange_export/exchange_preview/exchange_import`: exchange between instances ([`crate::exchange`],
//!   Round 26, `docs/RPC.md` „Austausch“), without a default work; reading in human only, not through `/act` and MCP
//! - `instance.get/set_name`: identifier and name of this instance ([`crate::instance`], Round 26); `set_name`
//!   human only, not through `/act` and MCP
//! - `search.find/preview/replace/undo`: search and replace in the work ([`crate::search`], Round 30a, `docs/RPC.md`
//!   „Suchen und Ersetzen“); `replace`/`undo` human only, not through `/act` and MCP
//! - `xref.targets/resolve/candidates/convert/broken`: cross-references ([`crate::xref`], Round 30a, `docs/RPC.md`
//!   „Querverweise“); `convert` human only
//! - `solution.list/adopt/variants/reject_siblings/compare`: solution proposals and variants
//!   ([`crate::assist::solutions`], Round 30b, `docs/RPC.md` „Lösungsvorschläge und Varianten“); proposals only as
//!   `proposed`, `reject_siblings` human only
//! - `mcp_token.list/create/revoke`: tokens for the MCP access from other machines
//!   ([`crate::mcp::tokens`], Round 10); human only, not through `/act` and MCP
//!
//! Titles of outline items are checked by the store (`baruch_store::check_title`, Round 7): empty, with a
//! line break or over 200 characters → `INVALID_PARAMS` (`StoreError::Title`), nothing changed.
//!
//! Accepting, switching, saving, changing and deleting write atomically into the database and
//! mirror into Git afterwards; the answer then carries `commit` (commit id) or `mirror` (error text of the
//! mirror). Everything runs under [`AppState::write_lock`], so that simultaneous changes each get their
//! own commit.
//!
//! Sections: `node` is the position (`seq`), which every outline change assigns anew. All methods
//! with `node` additionally take `key` (fixed key, `Node::key`); if it is there, it applies instead of
//! `node` ([`resolve_keys`]). Forms and editor always send it along.
//!
//! Revisions are addressed through their number, never through "position ± 1" (after deleting, the
//! sequence of numbers has gaps). Assistants only create proposals; saving, switching, changing and
//! deleting are reserved for the human and therefore do not exist through MCP.

use baruch_core::{Post, Work};
use baruch_library::{LibraryError, SearchHit, SearchOptions, TitleEntry};
use baruch_store::{NodeRevisions, NumberingInfo, RevisionEdit, StoreError, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::view::{self, Location, Settings};
use crate::{AppState, mirror};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL: i64 = -32603;
/// Domain error (e.g. „nur Vorschläge lassen sich übernehmen“).
pub const REFUSED: i64 = -32000;
/// Meaning search failed (Round 19c, `idea.search` with `mode: "rag"`): embedding/index not usable; the message
/// names the reason.
pub const RAG_UNAVAILABLE: i64 = -32001;
/// Round 35: there are accounts and the request carries no valid session (HTTP 401 at `/rpc`).
pub const UNAUTHENTICATED: i64 = -32002;
/// Round 35: the role is not allowed the method or the work („Das darf nur der Autor: …“).
pub const FORBIDDEN: i64 = -32003;

#[derive(Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

#[derive(Deserialize)]
struct NodeParams {
    node: usize,
}

#[derive(Deserialize)]
struct RevParams {
    node: usize,
    /// Revision number from 1.
    rev: u32,
}

/// `revision.delete`; `confirm` must be `true` (checked before): deleting is final (the text
/// stays only in the mirror commit).
#[derive(Deserialize)]
struct DeleteParams {
    node: usize,
    rev: u32,
}

#[derive(Deserialize)]
struct BodyParams {
    node: usize,
    body: String,
}

/// If the human accepts a proposal with Markdown headings (`# Titel`, user 2026-11-07), the same rule
/// applies as on saving: the proposal itself stays unchanged (accepted, in the revision tree),
/// a revision of the human with the converted headings follows on it, and the outline reconciles
/// as on saving in the editor (further headings become new items; no merging).
/// If that does not work (e.g. a level the outline does not permit), it stays with what was accepted and
/// `markdown` in the answer says why.
async fn accept_markdown_headings(
    state: &AppState,
    held: &mirror::WriteGuard<'_>,
    work: WorkId,
    accepted: &baruch_store::Change,
    answer: &mut Value,
) -> Result<(), RpcError> {
    let store = &state.store;
    let loaded = store.load_work(work).await?;
    let Some(body) = loaded.nodes.get(accepted.node).and_then(|n| n.revision(accepted.rev)).map(|r| r.body.clone())
    else {
        return Ok(());
    };
    let converted = baruch_typst::markdown_headings(&body);
    if converted == body.as_str() {
        return Ok(());
    }
    let allow = baruch_store::Restructure { split: true, merge: false };
    match store.outline_save(work, accepted.node, &converted, Some(accepted.rev), allow, mirror::HUMAN_NAME).await {
        Ok(saved) => {
            let shown = saved.outline.as_ref().map_or(accepted.node, |o| o.node);
            let rev = store.load_work(work).await?.nodes.get(shown).and_then(|n| n.current_number());
            answer["node"] = json!(shown);
            answer["rev"] = json!(rev);
            answer["markdown"] = json!("Markdown-Überschriften umgewandelt.");
            match (&saved.outline, &saved.change) {
                (Some(outline), _) => {
                    answer["outline"] = crate::outline::outline_answer(outline);
                    add_mirror(answer, mirror::record_outline(state, held, outline).await);
                }
                (None, Some(change)) => add_mirror(answer, mirror::record(state, held, change).await),
                (None, None) => {}
            }
        }
        Err(e) => {
            answer["markdown"] = json!(format!("Markdown-Überschriften nicht umgewandelt: {e}"));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct SaveParams {
    node: usize,
    body: String,
    /// Revision the new one is based on (the one shown); missing: the current one.
    based_on: Option<u32>,
    /// Follow-up fix R22: state of the base the client had loaded; missing: only the revision number counts.
    based_on_amend: Option<u32>,
}

#[derive(Deserialize)]
struct UpdateParams {
    node: usize,
    rev: u32,
    body: Option<String>,
    note: Option<String>,
    /// Round 23a (code review web-3): state of the revision the client had loaded. If it stands on another
    /// state by now, nothing is changed (-32000 „Veraltet: …“); if the value is missing, there is no check.
    based_on_amend: Option<u32>,
}

#[derive(Deserialize)]
struct TreeParams {
    node: Option<usize>,
    /// `true`: the whole chapter of the section.
    #[serde(default)]
    chapter: bool,
}

#[derive(Deserialize)]
struct SearchParams {
    query: String,
    #[serde(flatten)]
    options: SearchOptions,
}

fn params<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))
}

impl From<StoreError> for RpcError {
    fn from(e: StoreError) -> Self {
        let code = match e {
            StoreError::UnknownWork(_)
            | StoreError::UnknownNode { .. }
            | StoreError::UnknownRevision { .. }
            | StoreError::EmptyPost
            | StoreError::Import(_) => INVALID_PARAMS,
            StoreError::Refused(_)
            | StoreError::AlreadyImported { .. }
            | StoreError::DuplicateTitle { .. }
            | StoreError::Heading(_)
            | StoreError::GoneNode { .. } => REFUSED,
            StoreError::EmptyTitle => INVALID_PARAMS,
            StoreError::Numbering(_) => INVALID_PARAMS,
            StoreError::UnknownAsset { .. } => INVALID_PARAMS,
            StoreError::Asset(_) => INVALID_PARAMS,
            StoreError::Title(_) => INVALID_PARAMS,
            StoreError::UnknownPersona(_) | StoreError::Color(_) => INVALID_PARAMS,
            StoreError::UnknownIdea(_) | StoreError::Idea(_) => INVALID_PARAMS,
            StoreError::UnknownPost { .. } | StoreError::UnknownRequest { .. } => INVALID_PARAMS,
            StoreError::Exchange(_) => INVALID_PARAMS,
            // Round 28: findings and post revisions.
            StoreError::UnknownFinding { .. } | StoreError::Finding(_) | StoreError::PostEdit(_) => INVALID_PARAMS,
            // Round 30a: work-wide replacing rejected or not implemented yet.
            StoreError::Replace(_) => REFUSED,
            // Round 29: typeset settings invalid (all messages); `typeset_settings` maps rejections itself.
            StoreError::Typeset(_) => INVALID_PARAMS,
            // Round 31: glossary and index (rejected, unknown or not implemented yet).
            StoreError::Glossary(_)
            | StoreError::UnknownGlossaryTerm { .. }
            | StoreError::UnknownIndexProposal { .. } => INVALID_PARAMS,
            // Round 23 (interface S1): cover block rejected (web) or request outdated (mcp-namen).
            StoreError::CoverBlock(_) => INVALID_PARAMS,
            StoreError::Stale(_) => REFUSED,
            StoreError::Db(_) | StoreError::Migrate(_) | StoreError::Io { .. } => INTERNAL,
        };
        RpcError::new(code, e.to_string())
    }
}

/// Round 32: errors when reading a Word file are due to the file (invalid parameters), only writing is an
/// internal error.
impl From<baruch_docx::DocxError> for RpcError {
    fn from(e: baruch_docx::DocxError) -> Self {
        use baruch_docx::DocxError;
        let code = match e {
            DocxError::Write(_) => INTERNAL,
            DocxError::NotDocx(_)
            | DocxError::Unsafe(_)
            | DocxError::Limit(_)
            | DocxError::Archive(_)
            | DocxError::Xml { .. }
            | DocxError::Invalid(_) => INVALID_PARAMS,
        };
        RpcError::new(code, e.to_string())
    }
}

impl From<LibraryError> for RpcError {
    fn from(e: LibraryError) -> Self {
        let code = match e {
            LibraryError::EmptyQuery | LibraryError::UnknownTitle(_) => INVALID_PARAMS,
            LibraryError::Db(_) | LibraryError::Migrate(_) => INTERNAL,
        };
        RpcError::new(code, e.to_string())
    }
}

/// Work from the parameter `work` (number); if it is missing, the default work. Unknown → `INVALID_PARAMS`.
pub async fn work_param(state: &AppState, params: &Value) -> Result<WorkId, RpcError> {
    match params.get("work") {
        None | Some(Value::Null) => Ok(state.default_work().await?),
        Some(value) => {
            let id = value.as_i64().ok_or_else(|| RpcError::new(INVALID_PARAMS, "work: work id (number) expected"))?;
            state.store.work_slug(id).await?;
            Ok(id)
        }
    }
}

/// Fixed key from `params[name]` (number or digits as text, forms); missing, `null` or
/// empty → `None`.
fn key_value(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, format!("{name}: stable key (number) expected"));
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Translate fixed keys into positions: `key` → `node`, `target_key` → `target`.
///
/// Every outline change assigns the positions (`seq`) anew; an outdated tab or an assistant
/// that has read the outline before would hit a foreign section with the old position. The
/// key (`Node::key`, database id) stays the same. If it is given, **it takes precedence**:
/// the position is determined from it, a position sent along (perhaps outdated) does not count.
/// If the item no longer exists → `GoneNode` (`REFUSED`). Write paths call this under
/// [`AppState::write_lock`], so that the outline does not change between translating and writing.
pub async fn resolve_keys(state: &AppState, work: WorkId, mut params: Value) -> Result<Value, RpcError> {
    for (key_name, node_name) in [("key", "node"), ("target_key", "target")] {
        if let Some(key) = key_value(&params, key_name)? {
            let seq = state.store.node_seq(work, key).await?;
            if !params.is_object() {
                params = json!({});
            }
            params[node_name] = json!(seq);
        }
    }
    Ok(params)
}

/// Position after a change: the current revision of the section (number, not position).
fn shown_current(work: &Work, node: usize) -> Value {
    let rev = work.nodes.get(node).and_then(|n| n.current_number()).unwrap_or(1);
    json!({ "node": node, "rev": rev })
}

/// Number of the current revision (read anew after a change).
async fn current_of(state: &AppState, work: WorkId, node: usize) -> Result<Option<u32>, RpcError> {
    let work = state.store.load_work(work).await?;
    Ok(work.nodes.get(node).and_then(|n| n.current_number()))
}

/// A revision in the tree as JSON, with `current: bool`.
fn info_json(info: &baruch_store::RevisionInfo) -> Value {
    let mut value = json!(info);
    value["current"] = json!(info.status == baruch_core::RevisionStatus::Current);
    value
}

/// Chapter (or part) of the node and all nodes below it, in tree order.
fn chapter_members(work: &Work, node: usize) -> Vec<usize> {
    let mut top = node;
    let mut steps = 0;
    while let Some(n) = work.nodes.get(top) {
        match (n.kind, n.parent) {
            (baruch_core::NodeKind::Section, Some(parent)) if steps < work.nodes.len() => {
                top = parent;
                steps += 1;
            }
            _ => break,
        }
    }
    let mut out = Vec::new();
    let mut stack = vec![top];
    while let Some(id) = stack.pop() {
        if out.contains(&id) || out.len() > work.nodes.len() {
            continue;
        }
        out.push(id);
        let children: Vec<usize> = work.children(Some(id)).map(|c| c.id).collect();
        stack.extend(children.into_iter().rev());
    }
    out
}

/// `revision.tree`: one section (`{revisions}`) or a chapter or the whole work (`{sections}`).
fn tree_answer(work: &Work, tree: &[NodeRevisions], node: Option<usize>, chapter: bool) -> Value {
    let infos = |id: usize| tree.iter().find(|n| n.node == id).map_or(&[][..], |n| n.revisions.as_slice());
    let section = |id: usize| {
        let n = &work.nodes[id];
        json!({
            "node": id, "number": n.number, "title": n.title, "current": n.current_number(),
            "revisions": infos(id).iter().map(info_json).collect::<Vec<_>>(),
        })
    };
    match (node, chapter) {
        (Some(id), false) => json!({ "revisions": infos(id).iter().map(info_json).collect::<Vec<_>>() }),
        (Some(id), true) => {
            json!({ "sections": chapter_members(work, id).into_iter().map(section).collect::<Vec<_>>() })
        }
        (None, _) => json!({ "sections": (0..work.nodes.len()).map(section).collect::<Vec<_>>() }),
    }
}

/// Round 22: parameter `mode` of `revision.save`/`outline.save` (missing = "new", unknown → -32602).
pub(crate) fn save_mode(params: &Value) -> Result<baruch_store::SaveMode, RpcError> {
    let mode = match params.get("mode") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(Value::String(s)) => Some(s.trim().to_owned()),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "mode: \"auto\" or \"new\" expected")),
    };
    baruch_store::SaveMode::from_param(mode.as_deref()).map_err(|e| RpcError::new(INVALID_PARAMS, e))
}

/// Round 22: complete the answer on saving – `saved` ("amended" | "new" | "unchanged"), with "amended" `amend_seq`,
/// with a new revision in the mode "auto" `reason` and `reason_label`.
pub(crate) fn add_saved(
    answer: &mut Value,
    change: Option<&baruch_store::Change>,
    reason: Option<baruch_store::NewReason>,
) {
    match change {
        Some(change) if change.kind == baruch_store::ChangeKind::Amended => {
            answer["saved"] = json!("amended");
            answer["amend_seq"] = json!(change.amend_seq);
        }
        Some(_) => answer["saved"] = json!("new"),
        None => answer["saved"] = json!("unchanged"),
    }
    if let Some(reason) = reason {
        answer["reason"] = json!(reason.id());
        answer["reason_label"] = json!(reason.label());
    }
}

/// Result of the mirror into the answer: `commit` for a new commit, `mirror` for an error.
pub(crate) fn add_mirror(answer: &mut Value, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Content of the literature popup: all titles, plus the hits if a search was made. Plus the
/// revision tree of the work (`revisions`, without texts) for editor and tree view: parent, time
/// and "changed" are not in `baruch_core::Revision`; and the numbering of the work
/// (`numbering`) for the form in the header of the outline (the work template is not in `Work`).
pub struct LibraryView {
    pub titles: Vec<TitleEntry>,
    pub query: Option<String>,
    pub hits: Vec<SearchHit>,
    pub revisions: Vec<NodeRevisions>,
    pub numbering: Option<NumberingInfo>,
    pub persona_colors: Vec<baruch_store::PersonaColor>,
    pub image_alts: std::collections::HashMap<String, String>,
    /// Round 21c: images of the work and of the template for editor header, inserting and missing images (before under
    /// a reserved key in `image_alts`).
    pub images: crate::wysiwyg::ImageView,
    /// Number of open requests on the outline (hint in the header of the tree, migration 0012).
    pub outline_requests: usize,
    /// Cover of the work (Round 11, [`crate::cover::cover_info`]): `{name, caption, alt, url, thumb}`.
    pub cover: Option<Value>,
    /// Round 16b: time guard and releases of the personas for the workspace page ([`crate::view::extras`]).
    pub extras: view::ViewExtras,
    /// Round 23a: finished, escaped HTML of the box „Bilder des Werks“ for the search `query` (empty without a search
    /// or without a hit box); the workspace page puts it below the hits (`fragments/library.html`).
    pub images_html: String,
}

/// Load the titles and, if `lq` contains words, search the work `work` and the library; plus
/// the revision tree of the work.
///
/// Round 35: for Lektors the literature list is empty – no titles, no search, no hits, no image box
/// (the literature belongs to the author, contract §0.7/§0.8); everything else of the workspace page stays.
pub async fn library_view(state: &AppState, work: WorkId, lq: Option<&str>) -> Result<LibraryView, RpcError> {
    let lektor = crate::auth::current().lektor().is_some();
    let titles = if lektor { Vec::new() } else { baruch_library::list_titles(state.store.pool()).await? };
    let query = lq.map(str::trim).filter(|q| !q.is_empty() && !lektor).map(str::to_owned);
    let hits = match &query {
        Some(q) => {
            let options = SearchOptions { work: Some(work), ..SearchOptions::default() };
            match baruch_library::search(state.store.pool(), q, &options).await {
                Ok(hits) => hits,
                Err(LibraryError::EmptyQuery) => Vec::new(),
                Err(e) => return Err(e.into()),
            }
        }
        None => Vec::new(),
    };
    let revisions = state.store.revision_tree(work).await?;
    let numbering = Some(state.store.outline_numbering(work).await?);
    let persona_colors = state.store.persona_colors().await?;
    let (image_alts, images) = crate::wysiwyg::image_view(state, work).await?;
    let outline_requests = state.store.open_outline_requests(work).await?;
    let cover = crate::cover::cover_info(state, work).await?;
    let extras = view::extras(state, work).await?;
    let images_html = crate::literature::image_hits_box(state, work, query.as_deref()).await;
    Ok(LibraryView {
        images_html,
        titles,
        query,
        hits,
        revisions,
        numbering,
        persona_colors,
        image_alts,
        images,
        outline_requests,
        cover,
        extras,
    })
}

/// Every RPC method, with rights check (Round 35): [`crate::roles::check_rpc`] decides before every dispatch – also
/// for unknown names – whether whoever makes the request ([`crate::auth::current`]) is allowed the method, and may
/// complete `params` (work of a Lektor). This is the ONLY rights check for `/rpc`, `/act` and `/lektorat/act`.
pub async fn call(state: &AppState, settings: &Settings, method: &str, mut params: Value) -> Result<Value, RpcError> {
    let who = crate::auth::current();
    crate::roles::check_rpc(state, &who, method, &mut params).await?;
    dispatch(state, settings, method, params).await
}

async fn dispatch(state: &AppState, settings: &Settings, method: &str, params_value: Value) -> Result<Value, RpcError> {
    // Round 35 (interface R35-S1): accounts (`account.*`) and roles (`member.*`, `revision.propose`,
    // `finding.add`) before all domain modules.
    if crate::auth::rpc::handles(method) {
        return crate::auth::rpc::call(state, method, params_value).await;
    }
    if crate::roles::rpc::handles(method) {
        return crate::roles::rpc::call(state, method, params_value).await;
    }
    if crate::exchange::handles(method) {
        return crate::exchange::call(state, method, params_value).await;
    }
    // Round 32 (interface R32-S1): Word import (global, without a work) and Word export, before the default work.
    if crate::docx_import::handles(method) {
        return crate::docx_import::call(state, method, params_value).await;
    }
    if crate::docx_export::handles(method) {
        return crate::docx_export::call(state, method, params_value).await;
    }
    if crate::instance::handles(method) {
        return crate::instance::call(state, method, params_value).await;
    }
    if crate::works::handles(method) {
        return crate::works::call(state, method, params_value).await;
    }
    // Round 21 (interface R21-S1): PDF/EPUB import, global without a work.
    if crate::pdf_import::handles(method) {
        return crate::pdf_import::call(state, method, params_value).await;
    }
    if crate::personas::handles(method) {
        return crate::personas::call(state, method, params_value).await;
    }
    if crate::ideas::handles(method) {
        return crate::ideas::call(state, method, params_value).await;
    }
    // Round 28: findings (`finding.*`) and their overview (`finding.matrix`).
    if crate::findings::rpc::handles(method) {
        return crate::findings::rpc::call(state, method, params_value).await;
    }
    if crate::findings::page::handles(method) {
        return crate::findings::page::call(state, method, params_value).await;
    }
    if crate::idea_map::handles(method) {
        return crate::idea_map::call(state, method, params_value).await;
    }
    if crate::menu::handles(method) {
        return crate::menu::call(state, method, params_value).await;
    }
    if crate::settings::handles(method) {
        return crate::settings::call(state, settings, method, params_value).await;
    }
    if crate::connections::handles(method) {
        return crate::connections::call(state, method, params_value).await;
    }
    if crate::websearch::handles(method) {
        return crate::websearch::call(state, method, params_value).await;
    }
    if crate::threads::handles(method) {
        return crate::threads::call(state, method, params_value).await;
    }
    if crate::sync::handles(method) {
        return crate::sync::call(state, method, params_value).await;
    }
    if crate::rag::handles(method) {
        return crate::rag::call(state, method, params_value).await;
    }
    // Round 10: model backend (`model.*`), passage check (`scripture.check`) and MCP tokens
    // (`mcp_token.*`) need no work.
    if crate::models::handles(method) {
        return crate::models::call(state, method, params_value).await;
    }
    if crate::scripture::handles(method) {
        return crate::scripture::call(state, method, params_value).await;
    }
    if crate::scripture_map::handles(method) {
        return crate::scripture_map::call(state, method, params_value).await;
    }
    if crate::bibles::handles(method) {
        return crate::bibles::call(state, method, params_value).await;
    }
    if crate::mcp::tokens::handles(method) {
        return crate::mcp::tokens::call(state, method, params_value).await;
    }
    // Round 20 (interface R20-S1): setting for EPUBCheck, without a work.
    if crate::epub_check::handles_global(method) {
        return crate::epub_check::call_global(state, method, params_value).await;
    }
    let store = &state.store;
    // Round 23a (code review web-2): `ui.render` without `work` draws the work from the cookie `baruch_work` (like
    // the page `/`), not the default work. Otherwise „Zurück“ to the address `/` showed another work without a hint.
    let remembered = match (method, params_value.get("work"), settings.work) {
        ("ui.render", None | Some(Value::Null), Some(work)) => store.work_slug(work).await.is_ok().then_some(work),
        _ => None,
    };
    let work_id = match remembered {
        Some(work) => work,
        None => work_param(state, &params_value).await?,
    };
    if crate::posts::handles(method) {
        return crate::posts::call(state, work_id, method, params_value).await;
    }
    match method {
        "work.get" => {
            let work = store.load_work(work_id).await?;
            let lector = work.lector().map(|p| p.handle.clone());
            let cover = crate::cover::cover_info(state, work_id).await?;
            let language = store.work_language(work_id).await?;
            Ok(json!({ "work": work, "lector": lector, "cover": cover, "language": language }))
        }
        "work.export" => Ok(json!({ "typst": store.export_typst(work_id).await? })),
        "node.get" => {
            let p: NodeParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let work = store.load_work(work_id).await?;
            let Some(node) = work.nodes.get(p.node) else {
                return Err(StoreError::UnknownNode { work: work_id, node: p.node }.into());
            };
            // Round 22 (integrator): every revision carries its state (`amend_seq`) and, for proposals, the state of
            // the base (`based_on_amend`) as in revision.tree.
            let mut answer = json!(node);
            let tree = store.revision_tree(work_id).await?;
            if let (Some(infos), Some(list)) =
                (tree.iter().find(|t| t.node == p.node), answer["revisions"].as_array_mut())
            {
                for revision in list {
                    let number = revision["number"].as_u64();
                    if let Some(info) = infos.revisions.iter().find(|i| Some(u64::from(i.number)) == number) {
                        revision["amend_seq"] = json!(info.amend_seq);
                        if let Some(k) = info.based_on_amend {
                            revision["based_on_amend"] = json!(k);
                        }
                    }
                }
            }
            Ok(answer)
        }
        "revision.accept" | "revision.set_current" => {
            // Accepting = switching to a proposal. The outline follows the heading line
            // of the revision (another level: `outline` in the answer); exactly one commit in the mirror.
            let held = state.write_lock.lock().await;
            let p: RevParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let only_proposal = method == "revision.accept";
            let switch = store.switch_to(work_id, p.node, p.rev, only_proposal, mirror::HUMAN_NAME).await?;
            let mut answer = json!({ "node": switch.change.node, "rev": switch.change.rev });
            if let Some(outline) = &switch.outline {
                answer["outline"] = crate::outline::outline_answer(outline);
            }
            add_mirror(&mut answer, mirror::record_switch(state, &held, &switch).await);
            if only_proposal {
                accept_markdown_headings(state, &held, work_id, &switch.change, &mut answer).await?;
            }
            Ok(answer)
        }
        "revision.reject" => {
            let held = state.write_lock.lock().await;
            let p: RevParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let current = store.reject(work_id, p.node, p.rev).await?;
            // Round 22: rejecting is no commit; the side branch of the revision carries the status „verworfen“
            // afterwards.
            if let Err(e) = mirror::sync_side_refs(state, &held, work_id).await {
                eprintln!("Git-Spiegel (Werk {work_id}): Seitenzweige nicht nachgeführt: {e}");
            }
            Ok(json!({ "node": p.node, "rev": current }))
        }
        "revision.save" => {
            // Round 22: `mode` "auto" amends the own current revision if the rules allow it;
            // if it is missing, "new" applies (as before Round 22, all old clients).
            let mode = save_mode(&params_value)?;
            let held = state.write_lock.lock().await;
            let mut p: SaveParams = params(resolve_keys(state, work_id, params_value).await?)?;
            // Input of the human: Markdown headings become Typst headings (user 2026-11-07).
            p.body = baruch_typst::markdown_headings(&p.body).into_owned();
            let (saved, reason) = store
                .save_human_at(work_id, p.node, &p.body, mirror::HUMAN_NAME, p.based_on, p.based_on_amend, mode)
                .await?;
            match saved {
                Some(change) => {
                    let mut answer = json!({ "node": change.node, "rev": change.rev });
                    add_saved(&mut answer, Some(&change), reason);
                    add_mirror(&mut answer, mirror::record(state, &held, &change).await);
                    Ok(answer)
                }
                None => {
                    let mut answer = shown_current(&store.load_work(work_id).await?, p.node);
                    add_saved(&mut answer, None, None);
                    Ok(answer)
                }
            }
        }
        "revision.update" => {
            let held = state.write_lock.lock().await;
            let p: UpdateParams = params(resolve_keys(state, work_id, params_value).await?)?;
            // Round 18b: input of the human as in `revision.save` – Markdown headings become
            // Typst headings (`baruch_typst::markdown`, user 2026-11-07).
            let body = p.body.as_deref().map(baruch_typst::markdown_headings);
            let edit = RevisionEdit { body: body.as_deref(), note: p.note.as_deref() };
            let done =
                store.update_revision_at(work_id, p.node, p.rev, &edit, mirror::HUMAN_NAME, p.based_on_amend).await?;
            match done {
                Some(change) => {
                    let mut answer = json!({ "node": change.node, "rev": change.rev, "changed": true });
                    // Round 23a: new state if the change has created a step in the history.
                    if let Some(seq) = change.amend_seq {
                        answer["amend_seq"] = json!(seq);
                    }
                    add_mirror(&mut answer, mirror::record(state, &held, &change).await);
                    Ok(answer)
                }
                None => Ok(json!({ "node": p.node, "rev": p.rev, "changed": false })),
            }
        }
        "revision.delete" => {
            if params_value.get("confirm") != Some(&Value::Bool(true)) {
                return Err(RpcError::new(INVALID_PARAMS, "Löschen bitte bestätigen (confirm)."));
            }
            let held = state.write_lock.lock().await;
            let p: DeleteParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let change = store.delete_revision(work_id, p.node, p.rev, mirror::HUMAN_NAME).await?;
            let current = current_of(state, work_id, p.node).await?;
            let mut answer = json!({
                "node": change.node, "rev": current, "deleted": change.rev, "reparented": change.reparented,
            });
            add_mirror(&mut answer, mirror::record(state, &held, &change).await);
            Ok(answer)
        }
        "revision.tree" => {
            let p: TreeParams = params(params_value)?;
            let work = store.load_work(work_id).await?;
            if let Some(node) = p.node.filter(|&n| n >= work.nodes.len()) {
                return Err(StoreError::UnknownNode { work: work_id, node }.into());
            }
            let tree = store.revision_tree(work_id).await?;
            Ok(tree_answer(&work, &tree, p.node, p.chapter))
        }
        "post.add" => {
            let _held = state.write_lock.lock().await;
            let p: BodyParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let post = Post {
                id: 0,
                created: String::new(),
                // Round 35: a Lektor writes under his handle, all others as before as „Autor“.
                author: crate::roles::author_name(&crate::auth::current()),
                body: p.body,
                sources: vec![],
                refs: vec![],
            };
            let id = store.add_post(work_id, p.node, &post).await?;
            let key = store.node_key(work_id, p.node).await?;
            Ok(json!({ "node": p.node, "key": key, "id": id }))
        }
        "library.list" => {
            let titles = baruch_library::list_titles(store.pool()).await?;
            Ok(json!({ "titles": titles }))
        }
        "library.search" => {
            let mut p: SearchParams = params(params_value)?;
            p.options.work.get_or_insert(work_id);
            let hits = baruch_library::search(store.pool(), &p.query, &p.options).await?;
            Ok(json!({ "hits": hits }))
        }
        "ui.render" => {
            let at: Location = params(params_value)?;
            let (work, shelf) = crate::shelf(state, work_id).await?;
            let library = library_view(state, work_id, at.lq.as_deref()).await?;
            let resolved = at.resolve(&work);
            let panes = view::panes(&state.templates, &work, &shelf, resolved.clone(), settings, &library)
                .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
            let n = &work.nodes[resolved.node];
            // Round 36: display name for the origin `@lektorat:<slug>` as on the whole page (`consent::inject`).
            let names = crate::konten_ui::names::show_for;
            Ok(json!({
                "tree": names(state, panes.tree).await,
                "editor": names(state, panes.editor).await,
                "assistants": names(state, panes.assistants).await,
                "url": resolved.url(work_id, library.query.as_deref()),
                "title": format!("{} – Baruch", n.title),
            }))
        }
        m if crate::literature::handles(m) => crate::literature::call(state, work_id, m, params_value).await,
        m if crate::outline::handles(m) => crate::outline::call(state, work_id, m, params_value).await,
        m if crate::wysiwyg::handles(m) => crate::wysiwyg::call(state, work_id, m, params_value).await,
        m if crate::numbering::handles(m) => crate::numbering::call(state, work_id, m, params_value).await,
        m if crate::assets::handles(m) => crate::assets::call(state, work_id, m, params_value).await,
        m if crate::typeset::handles(m) => crate::typeset::call(state, work_id, m, params_value).await,
        m if crate::pdf::handles(m) => crate::pdf::call(state, work_id, m, params_value).await,
        m if crate::asset_requests::handles(m) => crate::asset_requests::call(state, work_id, m, params_value).await,
        m if crate::cover::handles(m) => crate::cover::call(state, work_id, m, params_value).await,
        m if crate::cover_layout::handles(m) => crate::cover_layout::call(state, work_id, m, params_value).await,
        m if crate::timing::handles(m) => crate::timing::call(state, work_id, m, params_value).await,
        m if crate::work_language::handles(m) => crate::work_language::call(state, work_id, m, params_value).await,
        m if crate::ribbon::handles(m) => crate::ribbon::call(state, work_id, m, params_value).await,
        m if crate::sheet::handles(m) => crate::sheet::call(state, work_id, m, params_value).await,
        // Round 29 (interface R29-S1): typeset settings and their preview.
        m if crate::typeset_settings::handles(m) => {
            crate::typeset_settings::call(state, work_id, m, params_value).await
        }
        m if crate::settings::work_typeset::handles(m) => {
            crate::settings::work_typeset::call(state, work_id, m, params_value).await
        }
        // Round 30 (interface R30-S1).
        m if crate::search::handles(m) => crate::search::call(state, work_id, m, params_value).await,
        m if crate::xref::handles(m) => crate::xref::call(state, work_id, m, params_value).await,
        m if crate::assist::solutions::handles(m) => {
            crate::assist::solutions::call(state, work_id, m, params_value).await
        }
        // Round 20 (interface R20-S1): EPUB, preview per reader, check.
        m if crate::epub::handles(m) => crate::epub::call(state, work_id, m, params_value).await,
        m if crate::epub_preview::handles(m) => crate::epub_preview::call(state, work_id, m, params_value).await,
        m if crate::epub_check::handles(m) => crate::epub_check::call(state, work_id, m, params_value).await,
        // Round 31 (interface R31-S1): glossary, index and index marks.
        m if crate::glossary::handles(m) => crate::glossary::call(state, work_id, m, params_value).await,
        m if crate::index_marks::handles(m) => crate::index_marks::call(state, work_id, m, params_value).await,
        // Round 22 (interface R22-S1): history of the amendments and setting „Fassungen“.
        m if crate::revisions::handles(m) => crate::revisions::call(state, work_id, m, params_value).await,
        m if crate::revision_fold::handles(m) => crate::revision_fold::call(state, work_id, m, params_value).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Answer a JSON-RPC request (single).
pub async fn handle(state: &AppState, settings: &Settings, request: Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str);
    let result = match (request.get("jsonrpc").and_then(Value::as_str), method) {
        (Some("2.0"), Some(method)) => {
            call(state, settings, method, request.get("params").cloned().unwrap_or(Value::Null)).await
        }
        _ => Err(RpcError::new(INVALID_REQUEST, "jsonrpc 2.0 with method expected")),
    };
    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } }),
    }
}
