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
//! Die Daten liefert [`crate::idea_map::build`]; hier entstehen Anordnung ([`layout`]), Zeichnung ([`svg`]) und die
//! Seite. Kein Buchtext ändert sich.

pub mod layout;
pub mod mindmap;
pub mod svg;

use std::collections::BTreeSet;
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
use crate::idea_map::{IdeaMap, MapFilter, MapNodeKind, NearState};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{Settings, query_escape};

/// Seite (GET), gemischt in `lib.rs` bei den Seiten. Die Ausgaben stehen in [`crate::idea_map::files::pages`].
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/karte", get(page))
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
pub(crate) async fn page_response(
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
