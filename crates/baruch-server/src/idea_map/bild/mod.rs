// SPDX-License-Identifier: AGPL-3.0-or-later
//! Seite der Gedankenkarte bzw. Mindmap (Vertrag Runde 25, §4.3/§4.4; seit Runde 34, Schnittstelle R34-S1, Besitz
//! mindmap_widget).
//!
//! - [`pages`]: GET `/werke/{id}/karte` (HTML nach dem DOM-Vertrag, ohne JavaScript bedienbar: Filter per GET,
//!   Gliederungsliste mit den Formularen „Bezug setzen“, „Verbinden mit …“, „Lösen“, Liste „Widersprüche“).
//! - Runde 19b: Abschnitt „Gruppen“ unter der Karte (Komponenten der Nähe, [`crate::idea_map::groups`]) mit dem Knopf
//!   „Gruppen benennen (Assistent)“ und je Vorschlag „Übernehmen“ und „Verwerfen“ – Formulare an
//!   `POST /werke/{id}/karte/act`, ohne JavaScript; der Filter kommt aus `back`.
//! - Runde 34 (R34-S1): Die Ausgaben (`.svg`, `.pdf`, `.md`, `.mm`) und das Formular `POST /werke/{id}/karte/act`
//!   liegen in [`crate::idea_map::files`]; hier bleiben die Seite und die Helfer, die beide brauchen (Filter in der
//!   Adresse, [`load`], [`work_info`], [`status_of`], [`plain`], [`with_query`], [`page_response`] als Fehlerseite
//!   eines Formulars). [`mindmap`] zeichnet die Mindmap aus [`crate::idea_map::tree::MindMap`] (in S1 ein Stub).
//!   [`layout`] und [`svg`] sind ab S1 eingefroren (alte Karte; `files` und diese Seite benutzen sie im S1-Stand
//!   noch, der Integrator löscht, was nach der Runde unbenutzt ist).
//!
//! - Runde 34 (mindmap_widget): Die Seite heißt „Mindmap“. Das Bild zeichnet [`mindmap::render`] aus
//!   [`crate::idea_map::tree::tree`] (Anordnung in [`tree_layout`]); `GET /werke/{id}/karte/bild` liefert nur das
//!   SVG (für das Widget `mindmap`, das nach jeder Aktion neu holt). Die Adresse trägt außer dem Filter `zu=`
//!   (eingeklappte Knoten, Komma) und `wahl=` (gewählter Knoten). Unter dem Baum liegt die **Wolke** (Einfälle ohne
//!   Platz, Nachtrag vom 2026-11-25). Die Formulare der alten Gliederungsliste stehen jetzt in der
//!   Gliederungsansicht (`/werke/{id}/karte/gliederung`, [`crate::idea_map::files`]); hier bleiben Filter,
//!   „Gruppen“ (R19b), „Widersprüche“ und die Ebene „Befunde“ (R28).
//!
//! Die Daten liefern [`crate::idea_map::tree::tree`] (Baum) und [`crate::idea_map::build`] (Gruppen der Nähe); hier
//! entstehen Zeichnung und Seite. Kein Buchtext ändert sich.

pub mod layout;
pub mod mindmap;
pub mod svg;
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
use crate::idea_map::files::export;
use crate::idea_map::tree::{MindMap, TreeKind, TreeNode};
use crate::idea_map::{IdeaMap, MapFilter, NearState};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{Settings, query_escape};

/// Seite (GET), gemischt in `lib.rs` bei den Seiten. Die Ausgaben stehen in [`crate::idea_map::files::pages`].
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/karte", get(page)).route("/werke/{id}/karte/bild", get(image))
}

// ---- Filter in der Adresse ---------------------------------------------------------------------

/// Was die Adresse der Karte außer dem Filter trägt (Rückmeldung nach einem Formular).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Extra {
    /// `ok=<wort>` nach einer gelungenen Aktion.
    pub ok: Option<String>,
    /// `undo=<id>`: eben gelöste Verbindung (Formular „Wiederherstellen“).
    pub undo: Option<i64>,
    /// Runde 34: `zu=<Kennungen, Komma>` – eingeklappte Knoten der Mindmap (reine Anzeige, nie gespeichert).
    pub closed: Vec<String>,
    /// Runde 34: `wahl=<Kennung>` – gewählter Knoten.
    pub wahl: Option<String>,
}

/// Höchstzahl der Kennungen in `zu=` (fremde Eingabe; mehr fällt weg).
pub const MAX_CLOSED: usize = 2000;

/// Kennung eines Knotens der Mindmap: `w`, `n:<key>` oder `i:<id>` (nur Ziffern) – sonst `None`.
pub fn node_id(text: &str) -> Option<&str> {
    let text = text.trim();
    let digits = |d: &str| !d.is_empty() && d.len() <= 18 && d.bytes().all(|b| b.is_ascii_digit());
    let known = text == "w" || text.strip_prefix("n:").or_else(|| text.strip_prefix("i:")).is_some_and(digits);
    known.then_some(text)
}

/// Filter aus den Paaren der Adresse (GET und `back` gleich): `status=open|done|all`, `word`, `origin=human|persona|mcp`,
/// `q`, `near=0|1` (letzter Wert gilt; das Formular schickt `near=0` vor dem Kästchen `near=1`), `near_limit`,
/// `near_threshold`, `open` (mehrfach). Unbekannte oder ungültige Werte fallen weg (wie bei `/einfaelle`).
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
            // Runde 34: letzter Wert gilt; nur gültige Kennungen, ohne Doppel.
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
            // Runde 28: Ebene „Befunde“ (letzter Wert gilt, wie bei `near`).
            "befunde" => filter.findings = matches!(value.trim(), "1" | "true" | "on" | "ja"),
            _ => {}
        }
    }
    filter.open = open.into_iter().collect();
    (filter, extra)
}

/// Kanonischer Abfrageteil zum Filter (ohne `?`, prozentkodiert, leer ohne Filter): Grundlage aller Links und
/// des Rücksprungs nach einem Formular.
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
    // `build` setzt die Vorgaben in den angewandten Filter ein; sie gehören nicht in die Adresse (kanonisch kurz).
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

/// Adresse mit Abfrageteil (`?` nur, wenn es einen gibt).
pub(crate) fn with_query(path: &str, query: &str) -> String {
    if query.is_empty() { path.to_owned() } else { format!("{path}?{query}") }
}

/// Abfrageteil eines Rücksprungs (`back`): nur der Abfrageteil der Karte, neu aufgebaut aus dem Filter – nichts
/// aus dem Formular landet ungeprüft in der Weiterleitung.
pub(crate) fn back_filter(back: Option<&str>) -> MapFilter {
    let back = back.unwrap_or("").trim().trim_start_matches('?');
    let pairs = Uri::try_from(format!("/?{back}"))
        .ok()
        .and_then(|uri| Query::<Vec<(String, String)>>::try_from_uri(&uri).ok())
        .map(|Query(pairs)| pairs)
        .unwrap_or_default();
    parse_filter(&pairs).0
}

// ---- Vorlagen ------------------------------------------------------------------------------------

/// Eigene Template-Umgebung (base.html + karte.html).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../../../templates/base.html")),
            ("karte.html", include_str!("../../../templates/karte.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Gedankenkarte fehlerhaft");
        }
        env
    })
}

/// Selbst gebaute Adresse (Ziffern, feste Namen, prozentkodierte Werte) als sicherer Wert für `href`: `&` maskiert,
/// `/` lesbar. Enthielte sie doch ein unsicheres Zeichen, steht `#` da.
fn href(url: String) -> Value {
    if url.chars().any(|c| matches!(c, '"' | '<' | '>' | '\'' | ' ') || c.is_control()) {
        return Value::from_safe_string("#".to_owned());
    }
    Value::from_safe_string(url.replace('&', "&amp;"))
}

/// Status der Fehlerseite nach dem RPC-Fehler.
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

/// Ordnername und Titel des Werks; `None`, wenn es das Werk nicht gibt.
// `Response` als Fehler ist hier gewollt (fertige Antwort für `?` im Handler); ein `Box` änderte alle Aufrufer.
#[allow(clippy::result_large_err)]
pub(crate) async fn work_info(state: &AppState, work: WorkId) -> Result<Option<(String, String)>, Response> {
    match state.store.works().await {
        Ok(works) => Ok(works.into_iter().find(|(id, ..)| *id == work).map(|(_, slug, title)| (slug, title))),
        Err(e) => Err(plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}"))),
    }
}

/// Karte und Anordnung; unbekanntes Werk → 404, sonst Fehler mit Status nach RpcError.
// `Response` als Fehler ist hier gewollt (fertige Antwort für `?` im Handler); ein `Box` änderte alle Aufrufer.
#[allow(clippy::result_large_err)]
pub(crate) async fn load(
    state: &AppState,
    work: WorkId,
    filter: &MapFilter,
) -> Result<(String, IdeaMap, layout::Layout), Response> {
    let slug = match work_info(state, work).await? {
        Some((slug, _)) => slug,
        None => return Err(plain(StatusCode::NOT_FOUND, "unbekanntes Werk")),
    };
    let map = crate::idea_map::build(state, work, filter)
        .await
        .map_err(|e| plain(status_of(&e), format!("Fehler: {}", e.message)))?;
    let layout = layout::layout(&map);
    Ok((slug, map, layout))
}

/// Rückmeldung zu `ok=<wort>`.
fn notice_of(ok: &str) -> &'static str {
    match ok {
        "bezug" => "Bezug gesetzt.",
        "verbunden" => "Verbunden.",
        "unveraendert" => "Diese Verbindung gibt es schon; nichts geändert.",
        "geloest" => "Verbindung gelöst.",
        "wiederhergestellt" => "Verbindung wiederhergestellt.",
        "zurueckgesetzt" => "Gespeicherte Lagen verworfen (in der Mindmap ohne Wirkung: Der Baum ordnet sich selbst).",
        "angelegt" => "Einfall angelegt.",
        "umgehaengt" => "Einfall umgehängt.",
        "geloescht" => "Einfall gelöscht (wiederherstellbar).",
        "eingelesen" => "Datei eingelesen: Die Knoten sind als Einfälle angelegt.",
        "benannt" => {
            "Der Assistent hat Namen vorgeschlagen (unten bei „Gruppen“). Schlagwort wird ein Name erst, wenn du ihn übernimmst."
        }
        "unbenannt" => "Der Assistent hat keinen brauchbaren Namen geliefert; es gibt keinen neuen Vorschlag.",
        "keine-gruppen" => "Keine Gruppe: Es gibt keine nahen Einfälle (mindestens zwei je Gruppe).",
        "uebernommen" => "Name übernommen: Er ist jetzt Schlagwort an den Einfällen der Gruppe.",
        "verworfen" => "Vorschlag verworfen.",
        _ => "Gespeichert.",
    }
}

/// Abfrageteil der Ansicht: Filter, dazu `zu=` (eingeklappte Knoten) – Grundlage der Links auf der Seite.
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

/// Das Bild als Fragment für die Seite (`standalone: false`); der gewählte Knoten (`wahl`) trägt `aria-current`.
fn draw(map: &MindMap, extra: &Extra) -> String {
    let options =
        mindmap::RenderOptions { standalone: false, closed: extra.closed.clone(), back: filter_query(&map.filter) };
    let svg = mindmap::render(map, &options).svg;
    match &extra.wahl {
        // Kennungen bestehen nur aus Buchstabe, Doppelpunkt und Ziffern ([`node_id`]); Titel sind maskiert und
        // können das Muster (mit Anführungszeichen) nicht enthalten.
        Some(wahl) => {
            svg.replacen(&format!(" data-node=\"{wahl}\""), &format!(" data-node=\"{wahl}\" aria-current=\"true\""), 1)
        }
        None => svg,
    }
}

/// Baum der Mindmap; unbekanntes Werk → 404, sonst Fehler mit Status nach RpcError.
// `Response` als Fehler ist hier gewollt (fertige Antwort für `?` im Handler).
#[allow(clippy::result_large_err)]
async fn load_tree(state: &AppState, work: WorkId, filter: &MapFilter) -> Result<MindMap, Response> {
    if work_info(state, work).await?.is_none() {
        return Err(plain(StatusCode::NOT_FOUND, "unbekanntes Werk"));
    }
    crate::idea_map::tree::tree(state, work, filter)
        .await
        .map_err(|e| plain(status_of(&e), format!("Fehler: {}", e.message)))
}

/// Die Seite der Mindmap (auch als Fehlerseite eines Formulars: `status` und `error`).
pub(crate) async fn page_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    filter: &MapFilter,
    extra: &Extra,
    status: StatusCode,
    error: Option<String>,
) -> Response {
    let map = match load_tree(state, work, filter).await {
        Ok(map) => map,
        Err(response) => return response,
    };
    // Gruppen der Nähe (R19b) rechnen auf der flachen Karte; sie wird dafür ein zweites Mal gebaut (gleicher Filter).
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
    // Eingeklappt: nur Knoten, die es gibt und die Kinder haben (kanonisch, nichts Fremdes in den Links).
    let closed: Vec<String> = extra
        .closed
        .iter()
        .filter(|id| map.find(id).is_some_and(|n: &TreeNode| !n.children.is_empty()))
        .cloned()
        .collect();
    let view = Extra { closed: closed.clone(), ..extra.clone() };
    // „Alles zuklappen“: jeder Hauptast mit Kindern (die Wolke klappt nicht).
    let mains: Vec<String> = map
        .root
        .children
        .iter()
        .filter(|c| c.kind != TreeKind::Idea && !c.children.is_empty())
        .map(|c| c.id.clone())
        .collect();
    let cloud = mindmap_cloud_count(&map);

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
    // Runde 19b: Gruppen der Nähe mit den Vorschlägen eines Assistenten (nur im Speicher).
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
        svg => Value::from_safe_string(draw(&map, &view)),
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
        // Runde 28: Ebene „Befunde“ ein- bzw. ausblenden (Link, ohne JavaScript).
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
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage karte.html: {e}")),
    }
}

/// Zahl der Einfälle in der Wolke (direkt am Werk).
fn mindmap_cloud_count(map: &MindMap) -> usize {
    tree_layout::cloud_ideas(map).len()
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

/// Nur das Bild (`image/svg+xml`), gleiche Abfrage wie die Seite – das Widget holt es nach jeder Aktion neu.
/// Das Fragment aus [`mindmap::render`] bekommt hier `xmlns`, damit die Antwort für sich gültiges SVG ist.
async fn image(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, extra) = parse_filter(&pairs);
    let map = match load_tree(&state, work, &filter).await {
        Ok(map) => map,
        Err(response) => return response,
    };
    let svg = draw(&map, &extra).replacen("<svg ", "<svg xmlns=\"http://www.w3.org/2000/svg\" ", 1);
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
