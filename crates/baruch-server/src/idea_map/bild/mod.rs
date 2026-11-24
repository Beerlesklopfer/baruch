// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bild und Seiten der Gedankenkarte (Vertrag Runde 25, §4.3/§4.4; Besitz karte-bild).
//!
//! - [`pages`]: GET `/werke/{id}/karte` (HTML nach dem DOM-Vertrag, ohne JavaScript bedienbar: Filter per GET,
//!   Gliederungsliste mit den Formularen „Bezug setzen“, „Verbinden mit …“, „Lösen“, Liste „Widersprüche“),
//!   `/werke/{id}/karte.svg` (eigenständig, `image/svg+xml`), `/werke/{id}/karte.pdf` (über
//!   [`crate::pdf::source_to_pdf`] mit [`PdfKind::Map`]), `/werke/{id}/karte.md` (Gliederung Kapitel → Einfälle →
//!   Verbindungen) und `/werke/{id}/karte.mm` (FreeMind). Alle Ausgaben nehmen dieselben Filter wie die Seite.
//! - [`writes`]: POST `/werke/{id}/karte/act` (hinter `guard::same_origin`): `method` ∈ `idea.update` (Bezug
//!   setzen, über [`crate::ideas::call`]), `idea.link`, `idea.unlink`, `idea.link_restore`, `idea.map_pos_reset`
//!   (über [`crate::idea_map::call`]); danach 303 auf die gefilterte Karte mit `ok=<wort>`, Fehler als Seite mit
//!   der Meldung (422 ungültig, 409 abgelehnt, 400 unbekannte Aktion).
//! - Runde 19b: Abschnitt „Gruppen“ unter der Karte (Komponenten der Nähe, [`crate::idea_map::groups`]) mit dem Knopf
//!   „Gruppen benennen (Assistent)“ (`idea.map_groups_name`, Feld `persona` wahlweise) und je Vorschlag „Übernehmen“
//!   (`idea.map_group_accept`, Feld `group`, wahlweise `name`) und „Verwerfen“ (`idea.map_group_dismiss`) – alles
//!   Formulare an dieselbe Route, ohne JavaScript; der Filter kommt aus `back`.
//!
//! Die Daten liefert [`crate::idea_map::build`] (karte-daten); hier entstehen nur Anordnung ([`layout`]),
//! Zeichnung ([`svg`]), Text-Ausgaben ([`export`]) und die Seite. Kein Buchtext ändert sich; nur „Bezug setzen“
//! und Verbindungen ergeben (über die RPC) einen Commit, Lagen keinen.

pub mod export;
pub mod layout;
pub mod svg;

use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_render::Source;
use baruch_store::{IdeaFilter, IdeaId, IdeaScope, IdeaStatus, LinkKind, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::idea_map::{IdeaMap, MapFilter, MapNodeKind, NearState};
use crate::pdf::{PDF_TIMEOUT, PdfKind};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{Settings, query_escape};

/// Seiten und Ausgaben (GET), gemischt in `lib.rs` bei den Seiten.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/karte", get(page))
        .route("/werke/{id}/karte.svg", get(svg_file))
        .route("/werke/{id}/karte.pdf", get(pdf_file))
        .route("/werke/{id}/karte.md", get(md_file))
        .route("/werke/{id}/karte.mm", get(mm_file))
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/karte/act", post(act))
}

// ---- Filter in der Adresse ---------------------------------------------------------------------

/// Was die Adresse der Karte außer dem Filter trägt (Rückmeldung nach einem Formular).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Extra {
    /// `ok=<wort>` nach einer gelungenen Aktion.
    pub ok: Option<String>,
    /// `undo=<id>`: eben gelöste Verbindung (Formular „Wiederherstellen“).
    pub undo: Option<i64>,
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
fn with_query(path: &str, query: &str) -> String {
    if query.is_empty() { path.to_owned() } else { format!("{path}?{query}") }
}

/// Abfrageteil eines Rücksprungs (`back`): nur der Abfrageteil der Karte, neu aufgebaut aus dem Filter – nichts
/// aus dem Formular landet ungeprüft in der Weiterleitung.
fn back_filter(back: Option<&str>) -> MapFilter {
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
fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        METHOD_NOT_FOUND => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn plain(status: StatusCode, text: impl Into<String>) -> Response {
    (status, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text.into()).into_response()
}

/// Ordnername und Titel des Werks; `None`, wenn es das Werk nicht gibt.
// `Response` als Fehler ist hier gewollt (fertige Antwort für `?` im Handler); ein `Box` änderte alle Aufrufer.
#[allow(clippy::result_large_err)]
async fn work_info(state: &AppState, work: WorkId) -> Result<Option<(String, String)>, Response> {
    match state.store.works().await {
        Ok(works) => Ok(works.into_iter().find(|(id, ..)| *id == work).map(|(_, slug, title)| (slug, title))),
        Err(e) => Err(plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}"))),
    }
}

/// Karte und Anordnung; unbekanntes Werk → 404, sonst Fehler mit Status nach RpcError.
// `Response` als Fehler ist hier gewollt (fertige Antwort für `?` im Handler); ein `Box` änderte alle Aufrufer.
#[allow(clippy::result_large_err)]
async fn load(
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
        "zurueckgesetzt" => "Anordnung zurückgesetzt: Die Einfälle stehen wieder an ihrem berechneten Platz.",
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

/// Die Seite der Karte (auch als Fehlerseite eines Formulars: `status` und `error`).
async fn page_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    filter: &MapFilter,
    extra: &Extra,
    status: StatusCode,
    error: Option<String>,
) -> Response {
    let (_, map, layout) = match load(state, work, filter).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
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

    // Auswahl „Bezug setzen“: alle Punkte der Gliederung in Baumreihenfolge (Abschnitte unter ihrem Kapitel).
    let tree = export::tree_order(&map);
    let mut node_options = Vec::new();
    for (node, depth) in &tree {
        if let Some(key) = node.key {
            let indent = "– ".repeat(depth.saturating_sub(1));
            node_options.push(context! { key => key, label => format!("{indent}{}", svg::node_label(node)) });
            for section in &node.sections {
                let name = if section.number.is_empty() {
                    section.title.clone()
                } else {
                    format!("{} {}", section.number, section.title)
                };
                node_options.push(context! { key => section.key, label => format!("{indent}– {name}") });
            }
        }
    }
    let label_of = |id: IdeaId| map.ideas.iter().find(|i| i.id == id).map(svg::idea_label);
    let idea_options: Vec<Value> = map
        .ideas
        .iter()
        .map(|i| context! { id => i.id, label => format!("{} – {}", i.id, svg::idea_label(i)) })
        .collect();

    // Gliederungsliste: je Knoten seine Einfälle mit ihren Verbindungen.
    let mut groups = Vec::new();
    let group_of = |node: Option<&crate::idea_map::MapNode>, id: &str, depth: usize| -> Value {
        let ideas: Vec<Value> = map
            .ideas
            .iter()
            .filter(|i| i.anchor == id)
            .map(|i| {
                let links: Vec<Value> = export::links_of(i.id, &map.links)
                    .into_iter()
                    .map(|(link, arrow, other)| {
                        context! {
                            id => link.id, arrow => arrow, other => other,
                            other_href => href(format!("/einfaelle/{other}")),
                            other_label => label_of(other).unwrap_or_default(),
                            kind => link.kind.as_str(), kind_label => link.kind.label(), note => link.note.as_str(),
                        }
                    })
                    .collect();
                let section = node.and_then(|n| {
                    let key = i.key?;
                    (n.key != Some(key)).then_some(())?;
                    n.sections.iter().find(|s| s.key == key).map(|s| {
                        if s.number.is_empty() { s.title.clone() } else { format!("{} {}", s.number, s.title) }
                    })
                });
                context! {
                    id => i.id, href => href(format!("/einfaelle/{}", i.id)), label => svg::idea_label(i),
                    note => svg::cut(&i.note, 300), open => i.status == IdeaStatus::Open,
                    status => svg::status_label(i.status), origin => i.origin.label(), key => i.key,
                    hit => i.hit, section => section, links => links,
                }
            })
            .collect();
        let (title, kind, key, sections, toggle, is_open, counts, empty) = match node {
            Some(n) => {
                let title = if n.kind == MapNodeKind::NoChapter { n.title.clone() } else { svg::node_label(n) };
                let is_open = n.key.is_some_and(|k| map.filter.open.contains(&k));
                let toggle = n.key.filter(|_| !n.sections.is_empty() || is_open).map(|k| {
                    let mut f = map.filter.clone();
                    if is_open {
                        f.open.retain(|o| *o != k);
                    } else {
                        f.open.push(k);
                        f.open.sort_unstable();
                    }
                    href(with_query(&base, &filter_query(&f)))
                });
                let sections: Vec<Value> = n
                    .sections
                    .iter()
                    .map(|s| {
                        let name =
                            if s.number.is_empty() { s.title.clone() } else { format!("{} {}", s.number, s.title) };
                        context! { key => s.key, name => name, ideas => s.ideas, open_findings => s.open_findings }
                    })
                    .collect();
                let kind = match n.kind {
                    MapNodeKind::Work => "work",
                    MapNodeKind::Part => "part",
                    MapNodeKind::Chapter => "chapter",
                    MapNodeKind::Section => "section",
                    MapNodeKind::NoChapter => "none",
                };
                (title, kind, n.key, sections, toggle, is_open, svg::counts(n), n.total_ideas == 0)
            }
            None => (map.title.clone(), "work", None, Vec::new(), None, false, String::new(), false),
        };
        // Runde 28: Ebene „Befunde“ – offene Befunde am Knoten samt allem darunter, Link auf die Befunde.
        let open_findings = node.map_or(0, |n| n.open_findings);
        context! {
            id => id, title => title, kind => kind, key => key, depth => depth,
            node_href => key.map(|k| href(format!("/?work={work}&key={k}"))),
            sections => sections, toggle => toggle, is_open => is_open, counts => counts, empty => empty,
            ideas => ideas,
            open_findings => open_findings,
            many_findings => map.filter.findings && open_findings as usize >= crate::findings::page::MANY_OPEN,
            findings_href => key.map(|k| href(format!("/werke/{work}/befunde?key={k}#zelle"))),
        }
    };
    if map.ideas.iter().any(|i| i.anchor == "w") {
        groups.push(group_of(None, "w", 0));
    }
    for (node, depth) in &tree {
        groups.push(group_of(Some(node), &node.id, *depth));
    }
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
    let kinds: Vec<Value> =
        LinkKind::ALL.iter().map(|k| context! { value => k.as_str(), label => k.label() }).collect();
    let near_note = match map.near.state {
        NearState::Ok | NearState::Off => None,
        _ => Some(map.near.message.clone()).filter(|m| !m.is_empty()),
    };
    let pinned = layout.items.iter().filter(|p| p.pinned).count();
    // Runde 19b: Gruppen der Nähe mit den Vorschlägen eines Assistenten (nur im Speicher).
    let clusters: Vec<Value> = crate::idea_map::groups::of_map(state, &map)
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
    let ctx = context! {
        settings => Value::from(Serde(settings)),
        work => context! { id => work, title => map.title.as_str() },
        title => map.title.as_str(),
        svg => Value::from_safe_string(svg::render(&map, &layout, false)),
        back => query.as_str(),
        here => href(with_query(&base, &query)),
        outputs => context! {
            svg => href(with_query(&format!("{base}.svg"), &query)),
            pdf => href(with_query(&format!("{base}.pdf"), &query)),
            md => href(with_query(&format!("{base}.md"), &query)),
            mm => href(with_query(&format!("{base}.mm"), &query)),
        },
        act => href(format!("{base}/act")),
        ideas_href => href(format!("/einfaelle?work={work}")),
        workspace => href(format!("/?work={work}")),
        // Runde 28: Ebene „Befunde“ ein- bzw. ausblenden (Link, ohne JavaScript).
        findings_toggle => {
            let mut f = map.filter.clone();
            f.findings = !f.findings;
            href(with_query(&base, &filter_query(&f)))
        },
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
        groups => groups,
        node_options => node_options,
        idea_options => idea_options,
        kinds => kinds,
        contradictions => contradictions,
        stats => context! { ideas => map.ideas.len(), links => map.links.iter().filter(|l| l.deleted.is_none()).count(), pinned => pinned },
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

/// Kopfzeilen einer Ausgabe: Art, ggf. Anhang mit Dateinamen aus dem slug (`[a-z0-9-]`, ohne Maskierung).
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

async fn svg_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load(&state, work, &filter).await {
        Ok((slug, map, layout)) => file(
            "image/svg+xml",
            Some(format!("inline; filename=\"{slug}-karte.svg\"")),
            svg::render(&map, &layout, true),
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
    match load(&state, work, &filter).await {
        Ok((slug, map, _)) => file(
            "text/markdown; charset=utf-8",
            Some(format!("inline; filename=\"{slug}-karte.md\"")),
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
    match load(&state, work, &filter).await {
        Ok((slug, map, _)) => file(
            "application/x-freemind",
            Some(format!("attachment; filename=\"{slug}-karte.mm\"")),
            export::freemind(&map),
        ),
        Err(response) => response,
    }
}

/// Typst-Zeichenkette (`"…"`): `\` und `"` maskiert, Zeilenumbrüche als `\n`, andere Steuerzeichen entfernt.
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

/// Typst-Quelle der Karte als PDF: A4 quer, Titel und Filter (Nutzertexte nur als Zeichenketten), darunter die
/// eigenständige SVG-Datei auf die Seite eingepasst.
pub fn pdf_source(map: &IdeaMap, layout: &layout::Layout) -> Source {
    let filter = export::filter_words(&map.filter);
    let mut main =
        String::from("#set page(paper: \"a4\", flipped: true, margin: 10mm)\n#set text(lang: \"de\", size: 9pt)\n");
    main.push_str(&format!(
        "#text(size: 14pt, weight: \"bold\", {})\n",
        typst_string(&format!("Gedankenkarte: {}", map.title))
    ));
    if !filter.is_empty() {
        main.push_str(&format!("\n#text(fill: gray, {})\n", typst_string(&format!("Filter: {filter}"))));
    }
    main.push_str("\n#align(center, image(\"karte.svg\", width: 100%, height: 165mm, fit: \"contain\"))\n");
    Source { main, files: vec![("karte.svg".to_owned(), svg::render(map, layout, true).into_bytes())] }
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
    let (slug, map, layout) = match load(&state, work, &filter).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    match crate::pdf::source_to_pdf(work, PdfKind::Map, pdf_source(&map, &layout), PDF_TIMEOUT).await {
        Ok(pdf) => crate::pdf::attachment(&format!("{slug}-karte.pdf"), pdf.bytes),
        Err(e) => {
            let status = if e.message == crate::pdf::BUSY {
                StatusCode::CONFLICT
            } else if e.message.starts_with("Typst brauchte länger") {
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

// ---- Formulare (POST) ----------------------------------------------------------------------------

/// Methoden des Formulars der Karte.
pub const ACT_METHODS: &[&str] = &[
    "idea.update",
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

/// Zahl aus einem Formularfeld; „Einfall 12“ oder „12 – Laube“ (Auswahlliste) zählen als 12.
fn number(value: Option<&str>, name: &str) -> Result<i64, RpcError> {
    let value = value.unwrap_or("").trim();
    let value = value.strip_prefix("Einfall").unwrap_or(value).trim_start();
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().map_err(|_| invalid(format!("{name}: Zahl erwartet")))
}

/// Gehört der Einfall zu diesem Werk? Sonst -32602 (die Karte ändert nur Einfälle ihres Werks).
async fn ensure_in_work(state: &AppState, idea: IdeaId, work: WorkId) -> Result<(), RpcError> {
    let found = state.store.idea_get(idea).await?;
    if found.work != Some(work) {
        return Err(invalid(format!("Einfall {idea} gehört nicht zu diesem Werk.")));
    }
    Ok(())
}

/// Letzter Wert eines Formularfelds.
fn form_field(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone())
}

/// Eine Aktion der Karte ausführen; Ergebnis: Wort für `ok` und ggf. die gelöste Verbindung.
async fn run_act(
    state: &AppState,
    work: WorkId,
    method: &str,
    pairs: &[(String, String)],
    filter: &MapFilter,
) -> Result<(&'static str, Option<i64>), RpcError> {
    let field = |name: &str| form_field(pairs, name);
    match method {
        "idea.update" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            let key = field("key").unwrap_or_default();
            crate::ideas::call(state, "idea.update", json!({ "id": id, "key": key.trim() })).await?;
            Ok(("bezug", None))
        }
        "idea.link" => {
            let from = number(field("from").as_deref(), "from")?;
            let to = number(field("to").as_deref(), "to (Einfall)")?;
            ensure_in_work(state, from, work).await?;
            let mut params = json!({ "from": from, "to": to, "kind": field("kind").unwrap_or_default().trim() });
            if let Some(note) = field("note").filter(|n| !n.trim().is_empty()) {
                params["note"] = Json::String(note);
            }
            let answer = crate::idea_map::call(state, "idea.link", params).await?;
            Ok((if answer["changed"] == Json::Bool(false) { "unveraendert" } else { "verbunden" }, None))
        }
        "idea.unlink" | "idea.link_restore" => {
            let id = number(field("id").as_deref(), "id (Verbindung)")?;
            let link = state.store.idea_link_get(id).await?;
            ensure_in_work(state, link.from, work).await?;
            crate::idea_map::call(state, method, json!({ "id": id })).await?;
            Ok(if method == "idea.unlink" { ("geloest", Some(id)) } else { ("wiederhergestellt", None) })
        }
        "idea.map_pos_reset" => {
            let mut params = json!({ "work": work });
            if let Some(idea) = field("idea").filter(|i| !i.trim().is_empty()) {
                params["idea"] = json!(number(Some(&idea), "idea")?);
            }
            crate::idea_map::call(state, "idea.map_pos_reset", params).await?;
            Ok(("zurueckgesetzt", None))
        }
        // Runde 19b: Gruppen benennen (Vorschlag eines Assistenten), Übernehmen nur hier und über RPC (Mensch).
        "idea.map_groups_name" => {
            let persona = field("persona").filter(|p| !p.trim().is_empty());
            let answer = crate::idea_map::groups::name_groups(state, work, filter, persona.as_deref()).await?;
            let proposed = answer["proposals"].as_array().map_or(0, Vec::len);
            Ok(match (proposed, answer.get("message").is_some()) {
                (0, true) => ("keine-gruppen", None),
                (0, false) => ("unbenannt", None),
                _ => ("benannt", None),
            })
        }
        "idea.map_group_accept" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            let name = field("name").filter(|n| !n.trim().is_empty());
            crate::idea_map::groups::accept(state, work, &group, name.as_deref(), filter).await?;
            Ok(("uebernommen", None))
        }
        "idea.map_group_dismiss" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            crate::idea_map::groups::dismiss(state, work, &group);
            Ok(("verworfen", None))
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("Unbekannte Aktion „{other}“ auf der Gedankenkarte."))),
    }
}

/// `POST /werke/{id}/karte/act` (urlencoded): Felder `method`, `id`, `key`, `from`, `to`, `kind`, `note`, `idea`,
/// seit Runde 19b `group`, `name`, `persona`, dazu `back` (nur der Abfrageteil der Karte; sein Filter gilt auch für
/// die Gruppen). Danach 303 auf `/werke/{id}/karte?<back>&ok=<wort>`.
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
        Ok(None) => return plain(StatusCode::NOT_FOUND, "unbekanntes Werk"),
        Err(response) => return response,
    }
    let method = field("method").unwrap_or_default();
    let outcome = if ACT_METHODS.contains(&method.trim()) {
        run_act(&state, work, method.trim(), &pairs, &filter).await
    } else {
        Err(RpcError::new(METHOD_NOT_FOUND, format!("Unbekannte Aktion „{}“ auf der Gedankenkarte.", method.trim())))
    };
    match outcome {
        Ok((ok, undo)) => {
            let mut query = filter_query(&filter);
            if !query.is_empty() {
                query.push('&');
            }
            query.push_str(&format!("ok={ok}"));
            if let Some(undo) = undo {
                query.push_str(&format!("&undo={undo}"));
            }
            Redirect::to(&format!("/werke/{work}/karte?{query}")).into_response()
        }
        Err(e) if e.code == INTERNAL => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
        Err(e) => {
            let status = status_of(&e);
            page_response(&state, &headers, work, &filter, &Extra::default(), status, Some(e.message)).await
        }
    }
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
    }
}
