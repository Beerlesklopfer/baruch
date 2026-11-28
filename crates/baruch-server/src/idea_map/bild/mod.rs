// SPDX-License-Identifier: AGPL-3.0-or-later
//! Page of the idea map or mind map (contract Round 25, §4.3/§4.4; since Round 34, interface R34-S1, owner
//! mindmap_widget).
//!
//! - [`pages`]: GET `/werke/{id}/karte` (HTML per the DOM contract, usable without JavaScript: filter through GET,
//!   outline list with the forms „Bezug setzen“, „Verbinden mit …“, „Lösen“, list „Widersprüche“).
//! - Round 19b: section „Gruppen“ below the map (components of the nearness, [`crate::idea_map::groups`]) with the
//!   button „Gruppen benennen (Assistent)“ and per suggestion „Übernehmen“ and „Verwerfen“ – forms to
//!   `POST /werke/{id}/karte/act`, without JavaScript; the filter comes from `back`.
//! - Round 34 (R34-S1): the exports (`.svg`, `.pdf`, `.md`, `.mm`) and the form `POST /werke/{id}/karte/act`
//!   live in [`crate::idea_map::files`]; here remain the page and the helpers both need (filter in the
//!   address, [`load`], [`work_info`], [`status_of`], [`plain`], [`with_query`], [`page_response`] as the error page
//!   of a form). [`mindmap`] draws the mind map from [`crate::idea_map::tree::MindMap`] (a stub in S1).
//!   [`layout`] and [`svg`] are frozen from S1 on (old map; `files` and this page still use them in the S1 state,
//!   the integrator deletes what is unused after the round).
//!
//! - Round 34 (mindmap_widget): the page is called „Mindmap“. The picture is drawn by [`mindmap::render`] from
//!   [`crate::idea_map::tree::tree`] (layout in [`tree_layout`]); `GET /werke/{id}/karte/bild` returns only the
//!   SVG (for the widget `mindmap`, which fetches anew after every action). Besides the filter the address carries
//!   `zu=` (collapsed nodes, comma) and `wahl=` (chosen node). Below the tree lies the **cloud** (ideas without a
//!   place, addendum of 2026-11-25). The forms of the old outline list now stand in the
//!   outline view (`/werke/{id}/karte/gliederung`, [`crate::idea_map::files`]); here remain the filter,
//!   „Gruppen“ (R19b), „Widersprüche“ and the layer „Befunde“ (R28).
//!
//! The data comes from [`crate::idea_map::tree::tree`] (tree) and [`crate::idea_map::build`] (groups of the
//! nearness); here drawing and page arise. No book text changes.

pub mod mindmap;
pub mod tree_layout;

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use baruch_store::{IdeaFilter, IdeaId, IdeaScope, IdeaStatus, LinkKind, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};

use crate::AppState;
use crate::i18n::tc;
use crate::idea_map::files::export;
use crate::idea_map::tree::{Scene, TreeNode};
use crate::idea_map::{IdeaMap, MapFilter, NearState};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{Settings, query_escape};

/// Page (GET), merged in `lib.rs` with the pages. The exports are in [`crate::idea_map::files::pages`].
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/karte", get(page)).route("/werke/{id}/karte/bild", get(image))
}

// ---- Filter in the address ---------------------------------------------------------------------

/// What the address of the map carries besides the filter (feedback after a form).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Extra {
    /// `ok=<wort>` after a successful action.
    pub ok: Option<String>,
    /// `undo=<id>`: link just removed (form „Wiederherstellen“).
    pub undo: Option<i64>,
    /// Round 34: `zu=<ids, comma>` – collapsed nodes of the mind map (display only, never stored).
    pub closed: Vec<String>,
    /// Round 34: `wahl=<id>` – chosen node.
    pub wahl: Option<String>,
}

/// Maximum number of ids in `zu=` (foreign input; more is dropped).
pub const MAX_CLOSED: usize = 2000;

/// Id of a node of the mind map: `w`, `n:<key>` or `i:<id>` (digits only) – otherwise `None`.
pub fn node_id(text: &str) -> Option<&str> {
    let text = text.trim();
    let digits = |d: &str| !d.is_empty() && d.len() <= 18 && d.bytes().all(|b| b.is_ascii_digit());
    let known = text == "w" || text.strip_prefix("n:").or_else(|| text.strip_prefix("i:")).is_some_and(digits);
    known.then_some(text)
}

/// Filter from the pairs of the address (GET and `back` alike): `status=open|done|all`, `word`,
/// `origin=human|persona|mcp`, `q`, `near=0|1` (last value counts; the form sends `near=0` before the checkbox
/// `near=1`), `near_limit`, `near_threshold`, `open` (multiple). Unknown or invalid values are dropped.
pub fn parse_filter(pairs: &[(String, String)]) -> (MapFilter, Extra) {
    let mut filter = MapFilter { near: true, ..MapFilter::default() };
    let mut extra = Extra::default();
    let mut open = BTreeSet::new();
    let text = |v: &str| Some(v.trim().to_owned()).filter(|v| !v.is_empty());
    for (name, value) in pairs {
        let value = value.as_str();
        match name.as_str() {
            "status" => filter.status = IdeaStatus::parse(value.trim()),
            "word" => filter.word = text(value),
            "origin" => {
                filter.origin = matches!(value.trim(), "human" | "persona" | "mcp").then(|| value.trim().to_owned())
            }
            "q" => filter.q = text(value),
            "near" => filter.near = !matches!(value.trim(), "0" | "false" | "off" | "nein" | ""),
            "near_limit" => filter.near_limit = value.trim().parse().ok().filter(|n| (1..=5).contains(n)),
            "near_threshold" => {
                filter.near_threshold = value.trim().parse().ok().filter(|t: &f32| (0.0..=1.0).contains(t))
            }
            "open" => {
                if let Ok(key) = value.trim().parse::<i64>() {
                    open.insert(key);
                }
            }
            "ok" => {
                extra.ok = Some(value.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(30).collect())
            }
            "undo" => extra.undo = value.trim().parse().ok(),
            // Round 34: last value counts; only valid ids, no duplicates.
            "zu" => {
                let mut closed: Vec<String> = Vec::new();
                for id in value.split(',').filter_map(node_id).take(MAX_CLOSED) {
                    if !closed.iter().any(|c| c == id) {
                        closed.push(id.to_owned());
                    }
                }
                extra.closed = closed;
            }
            "wahl" => extra.wahl = node_id(value).map(str::to_owned),
            // Round 28: layer „Befunde“ (last value counts, as for `near`).
            "befunde" => filter.findings = matches!(value.trim(), "1" | "true" | "on" | "ja"),
            _ => {}
        }
    }
    filter.open = open.into_iter().collect();
    (filter, extra)
}

/// Canonical query part for the filter (without `?`, percent-encoded, empty without filter): basis of all links and
/// of the way back after a form.
pub fn filter_query(filter: &MapFilter) -> String {
    let mut parts = Vec::new();
    if let Some(status) = filter.status {
        parts.push(format!("status={}", status.as_str()));
    }
    if let Some(word) = &filter.word {
        parts.push(format!("word={}", query_escape(word)));
    }
    if let Some(origin) = &filter.origin {
        parts.push(format!("origin={}", query_escape(origin)));
    }
    if let Some(q) = &filter.q {
        parts.push(format!("q={}", query_escape(q)));
    }
    if !filter.near {
        parts.push("near=0".to_owned());
    }
    // `build` puts the defaults into the applied filter; they do not belong in the address (canonically short).
    if let Some(limit) = filter.near_limit.filter(|l| *l != crate::idea_map::near::DEFAULT_LIMIT) {
        parts.push(format!("near_limit={limit}"));
    }
    if let Some(threshold) = filter.near_threshold.filter(|t| *t != crate::idea_map::near::DEFAULT_THRESHOLD) {
        parts.push(format!("near_threshold={threshold}"));
    }
    for key in &filter.open {
        parts.push(format!("open={key}"));
    }
    if filter.findings {
        parts.push("befunde=1".to_owned());
    }
    parts.join("&")
}

/// Address with query part (`?` only if there is one).
pub(crate) fn with_query(path: &str, query: &str) -> String {
    if query.is_empty() { path.to_owned() } else { format!("{path}?{query}") }
}

/// Query part of a way back (`back`): only the query part of the map, rebuilt from the filter – nothing
/// from the form ends up unchecked in the redirect.
pub(crate) fn back_filter(back: Option<&str>) -> MapFilter {
    let back = back.unwrap_or("").trim().trim_start_matches('?');
    let pairs = Uri::try_from(format!("/?{back}"))
        .ok()
        .and_then(|uri| Query::<Vec<(String, String)>>::try_from_uri(&uri).ok())
        .map(|Query(pairs)| pairs)
        .unwrap_or_default();
    parse_filter(&pairs).0
}

// ---- Templates -----------------------------------------------------------------------------------

/// Own template environment (base.html + karte.html).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../../templates/base.html")),
            ("karte.html", include_str!("../../../templates/karte.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Gedankenkarte fehlerhaft");
        }
        env
    })
}

/// Self-built address (digits, fixed names, percent-encoded values) as a safe value for `href`: `&` escaped,
/// `/` readable. Should it contain an unsafe character after all, `#` stands there.
fn href(url: String) -> Value {
    if url.chars().any(|c| matches!(c, '"' | '<' | '>' | '\'' | ' ') || c.is_control()) {
        return Value::from_safe_string("#".to_owned());
    }
    Value::from_safe_string(url.replace('&', "&amp;"))
}

/// Status of the error page after the RPC error.
pub(crate) fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        METHOD_NOT_FOUND => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) fn plain(status: StatusCode, text: impl Into<String>) -> Response {
    (status, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text.into()).into_response()
}

/// Folder name and title of the work; `None` if the work does not exist.
// `Response` as the error is intended here (finished response for `?` in the handler); a `Box` would change all
// callers.
#[allow(clippy::result_large_err)]
pub(crate) async fn work_info(state: &AppState, work: WorkId) -> Result<Option<(String, String)>, Response> {
    match state.store.works().await {
        Ok(works) => Ok(works.into_iter().find(|(id, ..)| *id == work).map(|(_, slug, title)| (slug, title))),
        Err(e) => Err(plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            tc("ideas_msg.page.error", &[("error", e.to_string().into())]),
        )),
    }
}

/// Feedback for `ok=<wort>`.
fn notice_of(ok: &str) -> String {
    match ok {
        "bezug" => tc("ideas_msg.map_notice.reference", &[]),
        "verbunden" => tc("ideas_msg.map_notice.linked", &[]),
        "unveraendert" => tc("ideas_msg.map_notice.link_exists", &[]),
        "geloest" => tc("ideas_msg.map_notice.unlinked", &[]),
        "wiederhergestellt" => tc("ideas_msg.map_notice.link_restored", &[]),
        "zurueckgesetzt" => tc("ideas_msg.map_notice.positions_reset", &[]),
        "angelegt" => tc("ideas_msg.map_notice.created", &[]),
        "umbenannt" => tc("ideas_msg.map_notice.renamed", &[]),
        "umgehaengt" => tc("ideas_msg.map_notice.moved", &[]),
        "in-wolke" => tc("ideas_msg.map_notice.to_cloud", &[]),
        "ort-unveraendert" => tc("ideas_msg.map_notice.place_unchanged", &[]),
        "geloescht" => tc("ideas_msg.map_notice.deleted", &[]),
        "einfall-wieder" => tc("ideas_msg.map_notice.restored", &[]),
        "eingelesen" => tc("ideas_msg.map_notice.imported", &[]),
        "benannt" => tc("ideas_msg.map_notice.named", &[]),
        "unbenannt" => tc("ideas_msg.map_notice.not_named", &[]),
        "keine-gruppen" => tc("ideas_msg.map_notice.no_groups", &[]),
        "uebernommen" => tc("ideas_msg.map_notice.name_adopted", &[]),
        "verworfen" => tc("ideas_msg.map_notice.rejected", &[]),
        _ => tc("ideas_msg.notice.saved", &[]),
    }
}

/// Query part of the view: filter, plus `zu=` (collapsed nodes) – basis of the links on the page.
fn view_query(filter: &MapFilter, closed: &[String]) -> String {
    let mut query = filter_query(filter);
    if !closed.is_empty() {
        if !query.is_empty() {
            query.push('&');
        }
        query.push_str("zu=");
        query.push_str(&closed.join(","));
    }
    query
}

/// The picture as a fragment for the page (`standalone: false`); the chosen node (`wahl`) carries `aria-current`.
fn draw(scene: &Scene, extra: &Extra) -> String {
    let options = mindmap::RenderOptions::of(scene, false, extra.closed.clone());
    let svg = mindmap::render(&scene.map, &options).svg;
    match &extra.wahl {
        // Ids consist only of letter, colon and digits ([`node_id`]); titles are escaped and
        // cannot contain the pattern (with quotation marks).
        Some(wahl) => {
            svg.replacen(&format!(" data-node=\"{wahl}\""), &format!(" data-node=\"{wahl}\" aria-current=\"true\""), 1)
        }
        None => svg,
    }
}

/// Tree of the mind map; unknown work → 404, otherwise error with status per RpcError.
// `Response` as the error is intended here (finished response for `?` in the handler).
#[allow(clippy::result_large_err)]
async fn load_tree(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<Scene, Response> {
    if work_info(state, work).await?.is_none() {
        return Err(plain(StatusCode::NOT_FOUND, tc("ideas_msg.page.unknown_work", &[])));
    }
    crate::idea_map::tree::scene(state, work, filter)
        .await
        .map_err(|e| plain(status_of(&e), tc("ideas_msg.page.error", &[("error", e.message.as_str().into())])))
}

/// The page of the mind map (also as the error page of a form: `status` and `error`).
pub(crate) async fn page_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    filter: &MapFilter,
    extra: &Extra,
    status: StatusCode,
    error: Option<String>,
) -> Response {
    let scene = match load_tree(state, work, filter).await {
        Ok(scene) => scene,
        Err(response) => return response,
    };
    let map = &scene.map;
    // Groups of the nearness (R19b) compute on the flat map; it is built a second time for that (same filter).
    let flat: Option<IdeaMap> = crate::idea_map::build(state, work, filter).await.ok();
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let query = filter_query(&map.filter);
    let base = format!("/werke/{work}/karte");
    let words: Vec<String> = state
        .store
        .idea_words(&IdeaFilter { scope: IdeaScope::Work(work), ..IdeaFilter::default() })
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|w| w.word)
        .collect();

    let rows = map.walk();
    let titles: HashMap<IdeaId, &str> =
        rows.iter().filter_map(|(node, _)| node.idea.map(|id| (id, node.title.as_str()))).collect();
    let label_of = |id: IdeaId| titles.get(&id).map(|t| (*t).to_owned());
    // Collapsed: only nodes that exist and have children (canonical, nothing foreign in the links).
    let closed: Vec<String> = extra
        .closed
        .iter()
        .filter(|id| map.find(id).is_some_and(|n: &TreeNode| !n.children.is_empty()))
        .cloned()
        .collect();
    let view = Extra { closed: closed.clone(), ..extra.clone() };
    // „Alles zuklappen“: every main branch with children (the cloud does not collapse).
    let mains: Vec<String> = map
        .root
        .children
        .iter()
        .filter(|c| !scene.in_cloud(&c.id) && !c.children.is_empty())
        .map(|c| c.id.clone())
        .collect();
    let cloud = scene.cloud_count;

    let contradictions: Vec<Value> = map
        .links
        .iter()
        .filter(|l| l.deleted.is_none() && l.kind == LinkKind::Contradicts)
        .map(|l| {
            context! {
                id => l.id, a => l.from, b => l.to,
                a_href => href(format!("/einfaelle/{}", l.from)), b_href => href(format!("/einfaelle/{}", l.to)),
                a_label => label_of(l.from).unwrap_or_default(), b_label => label_of(l.to).unwrap_or_default(),
                note => l.note.as_str(),
            }
        })
        .collect();
    let near_note = match map.near.state {
        NearState::Ok | NearState::Off => None,
        _ => Some(map.near.message.clone()).filter(|m| !m.is_empty()),
    };
    // Round 19b: groups of the nearness with the suggestions of an assistant (only in memory).
    let clusters: Vec<Value> = flat
        .as_ref()
        .map(|flat| crate::idea_map::groups::of_map(state, flat))
        .unwrap_or_default()
        .into_iter()
        .map(|g| {
            let ideas: Vec<Value> = g
                .ideas
                .iter()
                .map(|&id| {
                    context! { id => id, href => href(format!("/einfaelle/{id}")), label => label_of(id).unwrap_or_default() }
                })
                .collect();
            let proposal = g.proposal.map(|p| {
                context! { name => p.name, persona => p.persona, model => p.model, at => crate::localtime::seconds(&p.at) }
            });
            context! { group => g.group, ideas => ideas, words => g.words.join(", "), proposal => proposal }
        })
        .collect();
    let toggled = |change: &dyn Fn(&mut MapFilter)| {
        let mut f = map.filter.clone();
        change(&mut f);
        href(with_query(&base, &view_query(&f, &closed)))
    };
    let ctx = context! {
        settings => Value::from(Serde(settings)),
        work => context! { id => work, title => map.title.as_str() },
        title => map.title.as_str(),
        svg => Value::from_safe_string(draw(&scene, &view)),
        back => query.as_str(),
        zu => closed.join(","),
        wahl => extra.wahl.as_deref().unwrap_or(""),
        here => href(with_query(&base, &view_query(&map.filter, &closed))),
        bild => href(format!("{base}/bild")),
        outputs => context! {
            xmind => href(with_query(&format!("{base}.xmind"), &query)),
            mm => href(with_query(&format!("{base}.mm"), &query)),
            svg => href(with_query(&format!("{base}.svg"), &query)),
            pdf => href(with_query(&format!("{base}.pdf"), &query)),
            md => href(with_query(&format!("{base}.md"), &query)),
        },
        einlesen => href(format!("{base}/einlesen")),
        gliederung => href(with_query(&format!("{base}/gliederung"), &query)),
        open_all => href(with_query(&base, &query)),
        close_all => href(with_query(&base, &view_query(&map.filter, &mains))),
        act => href(format!("{base}/act")),
        ideas_href => href(format!("/einfaelle?work={work}")),
        workspace => href(format!("/?work={work}")),
        near_toggle => toggled(&|f| f.near = !f.near),
        // Round 28: show or hide the layer „Befunde“ (link, without JavaScript).
        findings_toggle => toggled(&|f| f.findings = !f.findings),
        findings_href => href(format!("/werke/{work}/befunde")),
        many_findings => crate::findings::page::MANY_OPEN,
        filter => context! {
            findings => map.filter.findings,
            status => map.filter.status.map_or("all", IdeaStatus::as_str),
            word => map.filter.word.as_deref().unwrap_or(""),
            origin => map.filter.origin.as_deref().unwrap_or(""),
            q => map.filter.q.as_deref().unwrap_or(""),
            near => map.filter.near,
            open => map.filter.open.clone(),
            words => export::filter_words(&map.filter),
        },
        words => words,
        near_note => near_note,
        contradictions => contradictions,
        stats => context! {
            ideas => map.total_ideas,
            links => map.links.iter().filter(|l| l.deleted.is_none()).count(),
            cloud => cloud,
        },
        clusters => clusters,
        notice => extra.ok.as_deref().map(notice_of),
        undo => extra.undo.filter(|_| extra.ok.as_deref() == Some("geloest")),
        error => error,
    };
    match templates().get_template("karte.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            tc("ideas_msg.page.template_error", &[("name", "karte.html".into()), ("error", e.to_string().into())]),
        ),
    }
}

// ---- Handler (GET) -------------------------------------------------------------------------------

async fn page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, extra) = parse_filter(&pairs);
    page_response(&state, &headers, work, &filter, &extra, StatusCode::OK, None).await
}

/// Only the picture (`image/svg+xml`), same query as the page – the widget fetches it anew after every action.
/// The fragment from [`mindmap::render`] gets `xmlns` here so that the response is valid SVG on its own.
async fn image(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, extra) = parse_filter(&pairs);
    let scene = match load_tree(&state, work, &filter).await {
        Ok(scene) => scene,
        Err(response) => return response,
    };
    let svg = draw(&scene, &extra).replacen("<svg ", "<svg xmlns=\"http://www.w3.org/2000/svg\" ", 1);
    ([(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], svg).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect()
    }

    #[test]
    fn the_filter_reads_every_field_and_drops_unknown_values() {
        let (f, extra) = parse_filter(&pairs(&[
            ("status", "open"),
            ("word", " Rose "),
            ("origin", "persona"),
            ("q", "Laube"),
            ("near", "0"),
            ("near", "1"),
            ("open", "7"),
            ("open", "3"),
            ("open", "7"),
            ("ok", "bezug\"<x>"),
        ]));
        assert_eq!(f.status, Some(IdeaStatus::Open), "status=open");
        assert_eq!(f.word.as_deref(), Some("Rose"), "Schlagwort getrimmt");
        assert_eq!(f.origin.as_deref(), Some("persona"), "Herkunft");
        assert_eq!(f.q.as_deref(), Some("Laube"), "Suche");
        assert!(f.near, "der letzte Wert von near gilt (verstecktes near=0 vor dem Kästchen near=1)");
        assert_eq!(f.open, vec![3, 7], "open mehrfach, sortiert, ohne Doppel");
        assert_eq!(extra.ok.as_deref(), Some("bezugx"), "ok nur aus Buchstaben, Ziffern und -");
        let (f, _) = parse_filter(&pairs(&[("status", "alle"), ("origin", "fremd"), ("near", "0")]));
        assert_eq!((f.status, f.origin, f.near), (None, None, false), "Unbekanntes fällt weg, near=0 schaltet aus");
        assert!(parse_filter(&[]).0.near, "Vorgabe: Nähe an (Vertrag §2)");
    }

    #[test]
    fn the_query_is_canonical_and_round_trips() {
        let (f, _) = parse_filter(&pairs(&[("q", "Rose & Dorn"), ("near", "0"), ("open", "5"), ("status", "done")]));
        let q = filter_query(&f);
        assert_eq!(q, "status=done&q=Rose%20%26%20Dorn&near=0&open=5", "feste Reihenfolge, prozentkodiert");
        assert_eq!(back_filter(Some(&format!("?{q}&ok=bezug&undo=3"))), f, "Rücksprung baut denselben Filter neu");
        assert_eq!(filter_query(&back_filter(Some("\"><script>"))), "", "Unsinn im back ergibt die ungefilterte Karte");
    }
}
