// SPDX-License-Identifier: AGPL-3.0-or-later
//! Import and export of the mind map and its use without JavaScript (contract Round 34 §5/§8, owner
//! mindmap_austausch).
//!
//! - [`pages`] (GET): the exports `/werke/{id}/karte.svg` and `.pdf` (through
//!   [`crate::idea_map::bild::mindmap::render`], standalone), `.md` ([`export::markdown`]), `.mm`
//!   ([`freemind::write`]) and `.xmind` ([`xmind::write`]); the outline view `/werke/{id}/karte/gliederung`
//!   ([`outline`]) and the form `/werke/{id}/karte/einlesen` ([`upload`]). All take the filters of the page.
//! - [`writes`] (POST, behind `guard::same_origin`): `/werke/{id}/karte/act` (forms of the mind map,
//!   [`ACT_METHODS`]), `/werke/{id}/karte/einlesen` (preview, creates nothing) and
//!   `/werke/{id}/karte/einlesen/anlegen` (confirmation, creates through [`crate::idea_map::tree::import`]).
//!
//! **Cloud** (addendum of the user 2026-11-25): ideas without a place lie in the cloud next to the mind map
//! ([`export::cloud`]). For it the form knows the field `ziel` when re-hanging (`wolke`, `w`, `i:<id>`, `n:<key>`):
//! “into the cloud” removes parent node and relation, “into the mind map under …” hangs under an idea or on a
//! chapter. A node moves into the cloud **with** its sub-nodes (they stay its sub-nodes).
//!
//! Filter, error page of the mind map and work details come from [`crate::idea_map::bild`] (`pub(crate)` there).

pub mod export;
pub mod freemind;
pub mod import;
pub mod outline;
pub mod upload;
pub mod xmind;

use std::sync::Arc;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_render::Source;
use baruch_store::{IdeaId, WorkId};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::i18n::tc;
use crate::idea_map::MapFilter;
use crate::idea_map::bild::mindmap::{self, RenderOptions};
use crate::idea_map::bild::{
    Extra, back_filter, filter_query, page_response, parse_filter, plain, status_of, work_info,
};
use crate::idea_map::tree::{self, MindMap, Scene};
use crate::pdf::{PDF_TIMEOUT, PdfKind};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// Exports, outline view and form „Einlesen“ (GET), merged in `lib.rs` with the pages.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/karte.svg", get(svg_file))
        .route("/werke/{id}/karte.pdf", get(pdf_file))
        .route("/werke/{id}/karte.md", get(md_file))
        .route("/werke/{id}/karte.mm", get(mm_file))
        .route("/werke/{id}/karte.xmind", get(xmind_file))
        .route("/werke/{id}/karte/gliederung", get(outline::page))
        .route("/werke/{id}/karte/einlesen", get(upload::form))
}

/// Forms (POST), merged in `lib.rs` in the writes router behind `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/karte/act", post(act))
        .route("/werke/{id}/karte/einlesen", post(upload::preview).layer(DefaultBodyLimit::max(upload::UPLOAD_LIMIT)))
        .route(
            "/werke/{id}/karte/einlesen/anlegen",
            post(upload::apply).layer(DefaultBodyLimit::max(upload::APPLY_LIMIT)),
        )
}

// ---- Loading -------------------------------------------------------------------------------------

/// Folder name of the work and its mind map with cloud ([`Scene`]); unknown work → 404, else error per RpcError.
// `Response` as the error is intended here (finished response for `?` in the handler), as in `bild::load`.
#[allow(clippy::result_large_err)]
pub(crate) async fn load_tree(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<(String, Scene), Response> {
    let slug = match work_info(state, work).await? {
        Some((slug, _)) => slug,
        None => return Err(plain(StatusCode::NOT_FOUND, tc("ideas_msg.page.unknown_work", &[]))),
    };
    let map = tree::scene(state, work, filter)
        .await
        .map_err(|e| plain(status_of(&e), tc("ideas_msg.page.error", &[("error", e.message.as_str().into())])))?;
    Ok((slug, map))
}

/// Is this an id of a node (`w`, `n:<key>`, `i:<id>`)?
pub(crate) fn is_node_id(id: &str) -> bool {
    id == "w"
        || id
            .strip_prefix("n:")
            .or_else(|| id.strip_prefix("i:"))
            .is_some_and(|n| !n.is_empty() && n.len() <= 18 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Collapsed nodes from `zu=<ids, comma>` (only valid ids, at most 500).
pub(crate) fn closed_of(pairs: &[(String, String)]) -> Vec<String> {
    let mut closed: Vec<String> = pairs
        .iter()
        .filter(|(name, _)| name == "zu")
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .filter(|id| is_node_id(id) && *id != "w")
        .map(str::to_owned)
        .collect();
    closed.sort();
    closed.dedup();
    closed.truncate(500);
    closed
}

// ---- Handler (GET) -------------------------------------------------------------------------------

/// Headers of an export: type, if any an attachment with file name from the slug (`[a-z0-9-]`, no escaping).
fn file(content_type: &'static str, disposition: Option<String>, body: impl Into<axum::body::Body>) -> Response {
    let mut response = (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        body.into(),
    )
        .into_response();
    if let Some(value) = disposition.and_then(|d| HeaderValue::from_str(&d).ok()) {
        response.headers_mut().insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

/// The standalone SVG file of the mind map (the same drawing as on the page).
fn standalone_svg(scene: &Scene, closed: Vec<String>) -> String {
    mindmap::render(&scene.map, &RenderOptions::of(scene, true, closed)).svg
}

async fn svg_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => file(
            "image/svg+xml",
            Some(format!("inline; filename=\"{slug}-mindmap.svg\"")),
            standalone_svg(&map, closed_of(&pairs)),
        ),
        Err(response) => response,
    }
}

async fn md_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => file(
            "text/markdown; charset=utf-8",
            Some(format!("inline; filename=\"{slug}-mindmap.md\"")),
            export::markdown(&map),
        ),
        Err(response) => response,
    }
}

async fn mm_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => file(
            "application/x-freemind",
            Some(format!("attachment; filename=\"{slug}-mindmap.mm\"")),
            freemind::write(&export::sheet(&map)),
        ),
        Err(response) => response,
    }
}

async fn xmind_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => match xmind::write(&export::sheet(&map)) {
            Ok(bytes) => file(
                "application/vnd.xmind.workbook",
                Some(format!("attachment; filename=\"{slug}-mindmap.xmind\"")),
                bytes,
            ),
            Err(message) => {
                plain(StatusCode::INTERNAL_SERVER_ERROR, tc("ideas_msg.page.error", &[("error", message.into())]))
            }
        },
        Err(response) => response,
    }
}

/// Typst string (`"…"`): `\` and `"` escaped, line breaks as `\n`, other control characters removed.
fn typst_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Typst source of the mind map as PDF: A4 landscape, title and filter (user texts only as strings), below it the
/// standalone SVG file (`svg`, from [`mindmap::render`]) fitted to the page.
pub fn pdf_source(map: &MindMap, svg: String) -> Source {
    let filter = export::filter_words(&map.filter);
    let mut main =
        String::from("#set page(paper: \"a4\", flipped: true, margin: 10mm)\n#set text(lang: \"de\", size: 9pt)\n");
    main.push_str(&format!(
        "#text(size: 14pt, weight: \"bold\", {})\n",
        typst_string(&format!("Mindmap: {}", map.title))
    ));
    if !filter.is_empty() {
        main.push_str(&format!("\n#text(fill: gray, {})\n", typst_string(&format!("Filter: {filter}"))));
    }
    main.push_str("\n#align(center, image(\"mindmap.svg\", width: 100%, height: 165mm, fit: \"contain\"))\n");
    Source { main, files: vec![("mindmap.svg".to_owned(), svg.into_bytes())] }
}

async fn pdf_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return crate::typeset::foreign_get();
    }
    let (filter, extra) = parse_filter(&pairs);
    let (slug, map) = match load_tree(&state, work, &filter).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let source = pdf_source(&map.map, standalone_svg(&map, closed_of(&pairs)));
    match crate::pdf::source_to_pdf(work, PdfKind::Map, source, PDF_TIMEOUT).await {
        Ok(pdf) => crate::pdf::attachment(&format!("{slug}-mindmap.pdf"), pdf.bytes),
        Err(e) => {
            let status = if crate::pdf::is_busy(&e.message) {
                StatusCode::CONFLICT
            } else if crate::typeset::is_timeout(&e.message) {
                StatusCode::SERVICE_UNAVAILABLE
            } else if e.code == REFUSED {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            page_response(&state, &headers, work, &filter, &extra, status, Some(e.message)).await
        }
    }
}

// ---- Forms (POST) --------------------------------------------------------------------------------

/// Methods of the form of the mind map (contract §5).
pub const ACT_METHODS: &[&str] = &[
    "idea.create",
    "idea.update",
    "idea.move",
    "idea.delete",
    "idea.restore",
    "idea.link",
    "idea.unlink",
    "idea.link_restore",
    "idea.map_pos_reset",
    "idea.map_groups_name",
    "idea.map_group_accept",
    "idea.map_group_dismiss",
];

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Number from a form field; „Einfall 12“ or „12 – Laube“ (selection list) count as 12.
fn number(value: Option<&str>, name: &str) -> Result<i64, RpcError> {
    let value = value.unwrap_or("").trim();
    let value = value.strip_prefix("Einfall").unwrap_or(value).trim_start();
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().map_err(|_| invalid(tc("ideas_msg.param.number", &[("name", name.into())])))
}

/// Optional reference from an existing form field: empty, `null`, `none` or `keiner` → `null`, otherwise a number.
fn reference(value: &str, name: &str) -> Result<Option<i64>, RpcError> {
    match value.trim() {
        "" | "null" | "none" | "keiner" => Ok(None),
        other => number(Some(other), name).map(Some),
    }
}

/// Does the idea belong to this work? Otherwise “refused” (409): the mind map changes only ideas of its work.
pub(crate) async fn ensure_in_work(state: &AppState, idea: IdeaId, work: WorkId) -> Result<(), RpcError> {
    let found = state.store.idea_get(idea).await?;
    if found.work != Some(work) {
        return Err(RpcError::new(REFUSED, tc("ideas_msg.map.idea_elsewhere", &[("idea", idea.into())])));
    }
    Ok(())
}

/// Last value of a form field.
fn form_field(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone())
}

/// Result of an action: word for `ok`, if any the removed link (`undo`) or the deleted idea (`wieder`).
#[derive(Debug, Default, PartialEq)]
struct Done {
    ok: &'static str,
    undo: Option<i64>,
    restore: Option<IdeaId>,
}

impl Done {
    fn word(ok: &'static str) -> Self {
        Done { ok, ..Done::default() }
    }
}

/// Carry out an action of the mind map. Every named idea must belong to the work of the address.
async fn run_act(
    state: &AppState,
    work: WorkId,
    method: &str,
    pairs: &[(String, String)],
    filter: &MapFilter,
) -> Result<Done, RpcError> {
    let field = |name: &str| form_field(pairs, name);
    let filled = |name: &str| form_field(pairs, name).filter(|v| !v.trim().is_empty());
    match method {
        // New idea: under an idea (`parent`), on a chapter (`key`), with `ort=werk` as a main topic on the
        // work or – without anything – in the cloud.
        "idea.create" => {
            let mut params = json!({ "work": work });
            for name in ["title", "words", "note"] {
                if let Some(value) = filled(name) {
                    params[name] = Json::String(value.trim().to_owned());
                }
            }
            for name in ["parent", "before"] {
                if let Some(value) = filled(name) {
                    let id = number(Some(&value), name)?;
                    ensure_in_work(state, id, work).await?;
                    params[name] = json!(id);
                }
            }
            if let Some(key) = filled("key") {
                params["key"] = json!(number(Some(&key), "key")?);
            }
            // `ort=werk`: without parent node and relation as a free main topic on the work (default: into the cloud).
            if filled("ort").as_deref().map(str::trim) == Some("werk") {
                params["cloud"] = Json::Bool(false);
            }
            crate::ideas::call(state, "idea.create", params).await?;
            Ok(Done::word("angelegt"))
        }
        // Rename (`title`) and/or set the relation (`key`); fields the form does not send stay.
        "idea.update" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            let mut params = json!({ "id": id });
            let title = field("title");
            if let Some(title) = &title {
                params["title"] = Json::String(title.trim().to_owned());
            }
            let key = field("key");
            if let Some(key) = &key {
                params["key"] = Json::String(key.trim().to_owned());
            }
            for name in ["words", "note"] {
                if let Some(value) = field(name) {
                    params[name] = Json::String(value);
                }
            }
            crate::ideas::call(state, "idea.update", params).await?;
            Ok(Done::word(match (title.is_some(), key.is_some()) {
                (true, _) => "umbenannt",
                (false, true) => "bezug",
                _ => "gespeichert",
            }))
        }
        // Re-hanging and ordering. `ziel` (`wolke`, `i:<id>`, `n:<key>`) is the short form for forms; besides it the
        // fields of the RPC (`parent`, `key`, `before`; present and empty = `null`).
        "idea.move" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            let mut params = json!({ "id": id });
            let mut to_cloud = false;
            if let Some(target) = filled("ziel") {
                let target = target.trim();
                if target == "wolke" {
                    // Into the cloud: removes parent node and relation, the sub-nodes stay with the idea.
                    params["cloud"] = Json::Bool(true);
                    to_cloud = true;
                } else if target == "w" {
                    // Onto the work: free main topic (own main branch).
                    params["parent"] = Json::Null;
                    params["key"] = Json::Null;
                    params["cloud"] = Json::Bool(false);
                } else if let Some(parent) = target.strip_prefix("i:") {
                    params["parent"] = json!(number(Some(parent), "ziel")?);
                } else if let Some(key) = target.strip_prefix("n:") {
                    params["parent"] = Json::Null;
                    params["key"] = json!(number(Some(key), "ziel")?);
                } else {
                    return Err(invalid(tc("ideas_msg.map.target", &[("lt", "<".into()), ("gt", ">".into())])));
                }
            }
            for name in ["parent", "key", "before"] {
                if let Some(value) = field(name) {
                    params[name] = json!(reference(&value, name)?);
                }
            }
            for name in ["parent", "before"] {
                if let Some(other) = params.get(name).and_then(Json::as_i64) {
                    ensure_in_work(state, other, work).await?;
                }
            }
            let answer = crate::ideas::call(state, "idea.move", params).await?;
            Ok(Done::word(if answer["changed"] == Json::Bool(false) {
                "ort-unveraendert"
            } else if to_cloud {
                "in-wolke"
            } else {
                "umgehaengt"
            }))
        }
        "idea.delete" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            crate::ideas::call(state, "idea.delete", json!({ "id": id, "confirm": true })).await?;
            Ok(Done { ok: "geloescht", undo: None, restore: Some(id) })
        }
        "idea.restore" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            crate::ideas::call(state, "idea.restore", json!({ "id": id })).await?;
            Ok(Done::word("einfall-wieder"))
        }
        "idea.link" => {
            let from = number(field("from").as_deref(), "from")?;
            let to = number(field("to").as_deref(), "to (Einfall)")?;
            ensure_in_work(state, from, work).await?;
            let mut params = json!({ "from": from, "to": to, "kind": field("kind").unwrap_or_default().trim() });
            if let Some(note) = filled("note") {
                params["note"] = Json::String(note);
            }
            let answer = crate::idea_map::call(state, "idea.link", params).await?;
            Ok(Done::word(if answer["changed"] == Json::Bool(false) { "unveraendert" } else { "verbunden" }))
        }
        "idea.unlink" | "idea.link_restore" => {
            let id = number(field("id").as_deref(), "id (Verbindung)")?;
            let link = state.store.idea_link_get(id).await?;
            ensure_in_work(state, link.from, work).await?;
            crate::idea_map::call(state, method, json!({ "id": id })).await?;
            Ok(if method == "idea.unlink" {
                Done { ok: "geloest", undo: Some(id), restore: None }
            } else {
                Done::word("wiederhergestellt")
            })
        }
        // Positions stay stored but no longer have an effect in the mind map (contract §0).
        "idea.map_pos_reset" => {
            let mut params = json!({ "work": work });
            if let Some(idea) = filled("idea") {
                params["idea"] = json!(number(Some(&idea), "idea")?);
            }
            crate::idea_map::call(state, "idea.map_pos_reset", params).await?;
            Ok(Done::word("zurueckgesetzt"))
        }
        // Round 19b: name groups (suggestion of an assistant), adopting only here and through RPC (human).
        "idea.map_groups_name" => {
            let persona = filled("persona");
            let answer = crate::idea_map::groups::name_groups(state, work, filter, persona.as_deref()).await?;
            let proposed = answer["proposals"].as_array().map_or(0, Vec::len);
            Ok(Done::word(match (proposed, answer.get("message").is_some()) {
                (0, true) => "keine-gruppen",
                (0, false) => "unbenannt",
                _ => "benannt",
            }))
        }
        "idea.map_group_accept" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            let name = filled("name");
            crate::idea_map::groups::accept(state, work, &group, name.as_deref(), filter).await?;
            Ok(Done::word("uebernommen"))
        }
        "idea.map_group_dismiss" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            crate::idea_map::groups::dismiss(state, work, &group);
            Ok(Done::word("verworfen"))
        }
        other => Err(unknown_action(other)),
    }
}

fn unknown_action(method: &str) -> RpcError {
    let shown: String = method.chars().filter(|c| !c.is_control()).take(60).collect();
    RpcError::new(METHOD_NOT_FOUND, tc("ideas_msg.map.unknown_action", &[("action", shown.into())]))
}

/// `POST /werke/{id}/karte/act` (urlencoded): field `method` ∈ [`ACT_METHODS`], plus the fields of the action (`id`,
/// `title`, `words`, `note`, `parent`, `key`, `before`, `ziel`, `ort`, `from`, `to`, `kind`, `idea`, `group`, `name`,
/// `persona`), `back` (only the query part; its filter also applies to the groups), `seite` ∈ `karte` | `gliederung`
/// (target of the redirect, default `karte`) and `wahl` (chosen node of the outline view). Then 303 to
/// `/werke/{id}/karte[/gliederung]?<back>&ok=<wort>`; errors as a page with the message (422 invalid, 409 refused,
/// 400 unknown action).
async fn act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let field = |name: &str| form_field(&pairs, name);
    let filter = back_filter(field("back").as_deref());
    match work_info(&state, work).await {
        Ok(Some(_)) => {}
        Ok(None) => return plain(StatusCode::NOT_FOUND, tc("ideas_msg.page.unknown_work", &[])),
        Err(response) => return response,
    }
    let to_outline = field("seite").as_deref().map(str::trim) == Some("gliederung");
    let chosen = field("wahl").map(|w| w.trim().to_owned()).filter(|w| is_node_id(w));
    let method = field("method").unwrap_or_default();
    let outcome = if ACT_METHODS.contains(&method.trim()) {
        run_act(&state, work, method.trim(), &pairs, &filter).await
    } else {
        Err(unknown_action(method.trim()))
    };
    match outcome {
        Ok(done) => {
            let mut query = filter_query(&filter);
            if !query.is_empty() {
                query.push('&');
            }
            query.push_str(&format!("ok={}", done.ok));
            if let Some(undo) = done.undo {
                query.push_str(&format!("&undo={undo}"));
            }
            if to_outline {
                if let Some(restore) = done.restore {
                    query.push_str(&format!("&wieder={restore}"));
                }
                // The deleted node cannot stay chosen.
                if let Some(chosen) = chosen.filter(|c| done.restore.is_none_or(|gone| *c != format!("i:{gone}"))) {
                    query.push_str(&format!("&wahl={chosen}"));
                }
                Redirect::to(&format!("/werke/{work}/karte/gliederung?{query}")).into_response()
            } else {
                Redirect::to(&format!("/werke/{work}/karte?{query}")).into_response()
            }
        }
        Err(e) if e.code == INTERNAL => plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            tc("ideas_msg.page.error", &[("error", e.message.as_str().into())]),
        ),
        Err(e) => {
            let status = status_of(&e);
            if to_outline {
                let view = outline::View { chosen, ..outline::View::default() };
                outline::render(&state, &headers, work, &filter, &view, status, Some(e.message)).await
            } else {
                page_response(&state, &headers, work, &filter, &Extra::default(), status, Some(e.message)).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typst_strings_cannot_break_out() {
        assert_eq!(typst_string("a\"b\\c\nd#e"), "\"a\\\"b\\\\c\\nd#e\"", "nur als Zeichenkette, # bleibt Text");
    }

    #[test]
    fn form_numbers_accept_the_list_labels() {
        assert_eq!(number(Some("12"), "to").unwrap(), 12, "Ziffern");
        assert_eq!(number(Some("Einfall 12"), "to").unwrap(), 12, "mit Wort davor");
        assert_eq!(number(Some("12 – Laube"), "to").unwrap(), 12, "Eintrag der Auswahlliste");
        assert!(number(Some("Laube"), "to").is_err(), "ohne Zahl → Fehler");
        assert_eq!(reference("", "parent").unwrap(), None, "ein vorhandenes, leeres Feld heißt „keiner“");
        assert_eq!(reference(" 7 ", "parent").unwrap(), Some(7), "sonst die Zahl");
    }

    #[test]
    fn closed_nodes_come_only_from_valid_ids() {
        let pairs = vec![("zu".to_owned(), "i:5, n:12,w,i:x,<b>,i:5".to_owned()), ("zu".to_owned(), "n:3".to_owned())];
        assert_eq!(
            closed_of(&pairs),
            vec!["i:5", "n:12", "n:3"],
            "nur Kennungen von Knoten, ohne Doppel; die Wurzel lässt sich nicht einklappen, Unsinn fällt weg"
        );
        assert!(is_node_id("w") && is_node_id("n:1") && !is_node_id("n:") && !is_node_id("i:-1"), "Kennungen");
    }
}
