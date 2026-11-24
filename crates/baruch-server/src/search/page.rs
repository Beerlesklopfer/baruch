// SPDX-License-Identifier: AGPL-3.0-or-later
//! Werk-Suche ohne JavaScript (Runde 30a): `GET /suche` und `POST /suche/act`.
//!
//! - Adresse mit denselben Namen wie [`SearchQuery`]: `work, q, case=1, word=1, re=1, range, key, sel=1,2,3, start,
//!   end, rev, shown_rev, kinds=text,heading,…`; dazu `replacement` und `vorschau=1` (Vorschau des Ersetzens),
//!   `batch=N` (Ergebnis eines Ersetzens mit „Rückgängig“), `undone=N`, `fehler=…`. Der Bereich bleibt beim Blättern
//!   in der Adresse ([`query_string`]).
//! - Seite: Formular (Suchtext, Ersatz, Optionen, Bereich mit eigener Häkchenliste der Gliederung, Textarten),
//!   Trefferbaum nach Abschnitten mit Sprung-Links (`data-jump`), Vorschau als Diff je Abschnitt mit Häkchen je
//!   Abschnitt und je Stelle, Knopf „Alle ersetzen“, nach dem Ersetzen „Rückgängig“ und der Hinweis auf die
//!   Wortlautprüfung (Link zur Prüfanzeige je geändertem Abschnitt).
//! - `POST /suche/act` (hinter `guard::same_origin`): `method` = `search.replace` (Felder der Suche, `replacement`,
//!   `pick=KEY:BASE` je Abschnitt, `keep=KEY:STELLE` je Treffer, `confirm`) bzw. `search.undo` (`batch`, `confirm`);
//!   danach 303 zurück auf `/suche` mit dem Bereich in der Adresse.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::WorkId;
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde_json::{Value, json};

use super::replace::{Pick, escape};
use super::{ALL_KINDS, RevChoice, SEARCH_KINDS, SearchQuery, SearchRange, TextKind};
use crate::AppState;
use crate::view::Settings;

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("search.html", include_str!("../../templates/search.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Werk-Suche fehlerhaft");
        }
        env
    })
}

/// Werte eines Namens aus Adresse oder Formular (mehrfach oder mit Kommas getrennt).
fn values<'a>(pairs: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    pairs
        .iter()
        .filter(|(k, _)| k == name)
        .flat_map(|(_, v)| v.split(','))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect()
}

fn first<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

fn flag(pairs: &[(String, String)], name: &str) -> bool {
    matches!(first(pairs, name).map(str::trim), Some("1" | "true" | "on"))
}

/// Suche aus Adresse bzw. Formular; ungültige Werte fallen weg.
pub fn query_from_pairs(pairs: &[(String, String)]) -> SearchQuery {
    let num = |name: &str| first(pairs, name).and_then(|v| v.trim().parse::<i64>().ok());
    let kinds: Vec<TextKind> = values(pairs, "kinds").into_iter().filter_map(TextKind::parse).collect();
    SearchQuery {
        q: first(pairs, "q").unwrap_or("").to_owned(),
        case: flag(pairs, "case"),
        word: flag(pairs, "word"),
        regex: flag(pairs, "re"),
        range: first(pairs, "range").and_then(SearchRange::parse).unwrap_or_default(),
        key: num("key"),
        sel: values(pairs, "sel").into_iter().filter_map(|v| v.parse().ok()).collect(),
        start: num("start").and_then(|v| usize::try_from(v).ok()),
        end: num("end").and_then(|v| usize::try_from(v).ok()),
        rev: if first(pairs, "rev") == Some("shown") { RevChoice::Shown } else { RevChoice::Current },
        shown_rev: num("shown_rev").and_then(|v| u32::try_from(v).ok()),
        kinds: (!kinds.is_empty()).then_some(kinds),
        text: None,
        limit: num("limit").and_then(|v| usize::try_from(v).ok()),
    }
}

/// Adresse der Seite mit dieser Suche (ohne Vorgabewerte), für Links und Weiterleitungen.
pub fn query_string(work: WorkId, query: &SearchQuery, replacement: Option<&str>) -> String {
    let e = crate::view::query_escape;
    let mut url = format!("/suche?work={work}");
    if !query.q.is_empty() {
        url.push_str(&format!("&q={}", e(&query.q)));
    }
    for (on, name) in [(query.case, "case"), (query.word, "word"), (query.regex, "re")] {
        if on {
            url.push_str(&format!("&{name}=1"));
        }
    }
    url.push_str(&format!("&range={}", query.range.word()));
    if let Some(key) = query.key {
        url.push_str(&format!("&key={key}"));
    }
    if !query.sel.is_empty() {
        let sel: Vec<String> = query.sel.iter().map(i64::to_string).collect();
        url.push_str(&format!("&sel={}", sel.join(",")));
    }
    if let (Some(s), Some(t)) = (query.start, query.end) {
        url.push_str(&format!("&start={s}&end={t}"));
    }
    if query.rev == RevChoice::Shown {
        url.push_str("&rev=shown");
        if let Some(r) = query.shown_rev {
            url.push_str(&format!("&shown_rev={r}"));
        }
    }
    if let Some(kinds) = &query.kinds {
        let words: Vec<&str> = kinds.iter().map(|k| k.word()).collect();
        url.push_str(&format!("&kinds={}", words.join(",")));
    }
    if let Some(r) = replacement {
        url.push_str(&format!("&replacement={}", e(r)));
    }
    url
}

/// Adresse sicher für ein `href` in der Vorlage (`&` maskiert; enthält nur selbst gebaute, kodierte Werte).
fn href(url: &str) -> JinjaValue {
    crate::typeset::href(url.to_owned())
}

fn error_page(status: StatusCode, message: &str) -> Response {
    (status, Html(format!("<!doctype html><meta charset=\"utf-8\"><title>Suchen</title><p>{}</p>", escape(message))))
        .into_response()
}

/// Häkchenliste der Gliederung: je Punkt Schlüssel, Beschriftung, Tiefe, gewählt.
fn outline_rows(work: &baruch_core::Work, query: &SearchQuery) -> Vec<Value> {
    work.nodes
        .iter()
        .map(|n| {
            let mut depth = 0;
            let mut parent = n.parent;
            while let Some(p) = parent {
                depth += 1;
                parent = work.nodes.get(p).and_then(|x| x.parent);
            }
            json!({
                "key": n.key,
                "label": super::label_of(&n.number, &n.title),
                "depth": depth,
                "checked": query.sel.contains(&n.key),
                "current": query.key == Some(n.key),
            })
        })
        .collect()
}

/// `GET /suche`.
async fn search_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let work = match first(&pairs, "work").and_then(|w| w.trim().parse::<WorkId>().ok()) {
        Some(w) => w,
        None => match settings.work {
            Some(w) => w,
            None => match state.default_work().await {
                Ok(w) => w,
                Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            },
        },
    };
    let loaded = match state.store.load_work(work).await {
        Ok(w) => w,
        Err(_) => return error_page(StatusCode::NOT_FOUND, "Unbekanntes Werk."),
    };
    let query = query_from_pairs(&pairs);
    let replacement = first(&pairs, "replacement").map(str::to_owned);
    let mut error = first(&pairs, "fehler").map(str::to_owned);
    let mut groups: Vec<Value> = Vec::new();
    let mut summary = Value::Null;
    let mut preview = Value::Null;
    if !query.q.is_empty() {
        if flag(&pairs, "vorschau") && query.range != SearchRange::All && crate::guard::cross_site_get(&headers) {
            // Die Vorschau rechnet jeden Ersatz durch: nicht auf Anfrage einer fremden Seite (etwa ein <img>).
            return error_page(
                StatusCode::FORBIDDEN,
                "Abgelehnt: Die Ersetzen-Vorschau startet nicht auf Anfrage einer fremden Seite.",
            );
        }
        if flag(&pairs, "vorschau") && query.range != SearchRange::All {
            // Ersetzt wird immer auf der aktuellen Fassung (search.replace): die Vorschau zeigt darum auch sie.
            let current = SearchQuery { rev: RevChoice::Current, ..query.clone() };
            match super::replace::preview(&state, work, &current, replacement.as_deref().unwrap_or("")).await {
                Ok(p) => {
                    let sections: Vec<Value> = p
                        .sections
                        .iter()
                        .map(|s| {
                            json!({
                                "key": s.key, "label": super::label_of(&s.number, &s.title), "base_rev": s.base_rev,
                                "count": s.hits.len(), "indices": (0..s.hits.len()).collect::<Vec<_>>(),
                                "diff_html": s.diff_html,
                                "url": s.hits.first().map(|h| h.url.replace('&', "&amp;")).unwrap_or_default(),
                            })
                        })
                        .collect();
                    preview = json!({
                        "sections": sections, "total": p.total, "truncated": p.truncated, "timed_out": p.timed_out,
                        "range_label": p.range_label,
                    });
                }
                Err(e) => error = Some(e.message),
            }
        } else {
            match super::find(&state, work, &query).await {
                Ok(result) => {
                    for hit in &result.hits {
                        let label = super::label_of(&hit.number, &hit.title);
                        let entry = json!({
                            "url": hit.url.replace('&', "&amp;"), "before": hit.before, "matched": hit.matched,
                            "after": hit.after, "line": hit.line, "kind": hit.kind.word(), "kind_label": hit.kind.label(),
                        });
                        match groups.last_mut() {
                            Some(g) if g["key"] == json!(hit.key) && g["work"] == json!(hit.work) => {
                                g["hits"].as_array_mut().expect("Liste").push(entry)
                            }
                            _ => groups
                                .push(json!({ "key": hit.key, "work": hit.work, "label": label, "hits": [entry] })),
                        }
                    }
                    summary = json!({
                        "total": result.total, "shown": result.hits.len(), "truncated": result.truncated,
                        "timed_out": result.timed_out, "range_label": result.range_label,
                    });
                }
                Err(e) => error = Some(e.message),
            }
        }
    }
    let batch_info = match first(&pairs, "batch").and_then(|b| b.parse::<i64>().ok()) {
        Some(id) => state.store.replace_batch(work, id).await.ok().map(|b| {
            let checks: Vec<Value> = b
                .items
                .iter()
                .map(|i| {
                    let label = loaded
                        .nodes
                        .iter()
                        .find(|n| n.key == i.key)
                        .map_or(format!("Abschnitt {}", i.key), |n| super::label_of(&n.number, &n.title));
                    json!({ "label": label, "url": format!("/werke/{work}/pruefung?key={}", i.key).replace('&', "&amp;") })
                })
                .collect();
            json!({ "id": b.id, "query": b.query, "replacement": b.replacement, "count": b.items.len(),
                    "undone": b.undone, "checks": checks })
        }),
        None => None,
    };
    let latest = state.store.replace_latest(work).await.ok().flatten().map(|b| {
        json!({ "id": b.id, "query": b.query, "replacement": b.replacement, "count": b.items.len(), "created": b.created })
    });
    let kinds_shown: Vec<Value> = ALL_KINDS
        .iter()
        .map(|k| {
            let on = query.kinds.as_ref().map_or(SEARCH_KINDS.contains(k), |list| list.contains(k));
            json!({ "word": k.word(), "label": k.label(), "checked": on })
        })
        .collect();
    // Runde 30 (Nachbesserung): „Markierung im Editor“ nur, wenn die Adresse eine Markierung trägt (start/end aus
    // dem Editor); ohne sie gäbe es nichts zu wählen.
    let selection_label = match (query.start, query.end) {
        (Some(s), Some(e)) => Some(format!("Markierung im Editor (Zeichen {s}–{e})")),
        _ => None,
    };
    let ranges: Vec<Value> = [
        (SearchRange::Selection, selection_label.as_deref().unwrap_or("")),
        (SearchRange::Section, "Abschnitt"),
        (SearchRange::Chapter, "Kapitel"),
        (SearchRange::Part, "Teil"),
        (SearchRange::From, "ab Abschnitt bis zum Ende"),
        (SearchRange::Picked, "Auswahl aus der Gliederung"),
        (SearchRange::Work, "ganzes Werk"),
        (SearchRange::All, "alle Werke (nur Suchen)"),
    ]
    .into_iter()
    .filter(|(r, _)| *r != SearchRange::Selection || selection_label.is_some())
    .map(|(r, label)| json!({ "word": r.word(), "label": label, "checked": query.range == r }))
    .collect();
    // Fassung (aktuelle | gezeigte): nur mit gewähltem Abschnitt; „gezeigte“ nur, wenn die Adresse sie nennt.
    let revs: Value = match query.key {
        Some(_) => json!({
            "shown_rev": query.shown_rev,
            "shown": query.rev == RevChoice::Shown,
        }),
        None => Value::Null,
    };
    // Felder, die das Suchformular unsichtbar weiterträgt (sonst gingen sie beim Absenden verloren).
    let carried: Vec<Value> = {
        let mut list = Vec::new();
        if let (Some(s), Some(e)) = (query.start, query.end) {
            list.push(json!({"name": "start", "value": s.to_string()}));
            list.push(json!({"name": "end", "value": e.to_string()}));
        }
        if let Some(r) = query.shown_rev {
            list.push(json!({"name": "shown_rev", "value": r.to_string()}));
        }
        list
    };
    let hidden: Vec<Value> = {
        let mut list = vec![json!({"name": "work", "value": work.to_string()})];
        for (on, name) in [(query.case, "case"), (query.word, "word"), (query.regex, "re")] {
            if on {
                list.push(json!({"name": name, "value": "1"}));
            }
        }
        list.push(json!({"name": "range", "value": query.range.word()}));
        if let Some(key) = query.key {
            list.push(json!({"name": "key", "value": key.to_string()}));
        }
        if !query.sel.is_empty() {
            let sel: Vec<String> = query.sel.iter().map(i64::to_string).collect();
            list.push(json!({"name": "sel", "value": sel.join(",")}));
        }
        if let Some(kinds) = &query.kinds {
            let words: Vec<&str> = kinds.iter().map(|k| k.word()).collect();
            list.push(json!({"name": "kinds", "value": words.join(",")}));
        }
        if query.rev == RevChoice::Shown {
            list.push(json!({"name": "rev", "value": "shown"}));
        }
        list.extend(carried.iter().cloned());
        list.push(json!({"name": "q", "value": query.q}));
        list
    };
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings)),
        work => minijinja::context! { id => work, title => loaded.title.clone() },
        q => query.q.clone(),
        q_enc => crate::view::query_escape(&query.q),
        replacement => replacement.clone().unwrap_or_default(),
        case => query.case, word => query.word, re => query.regex,
        range => query.range.word(),
        key => query.key,
        ranges => JinjaValue::from(Serde(ranges)),
        revs => JinjaValue::from(Serde(revs)),
        carried => JinjaValue::from(Serde(carried)),
        kinds => JinjaValue::from(Serde(kinds_shown)),
        outline => JinjaValue::from(Serde(outline_rows(&loaded, &query))),
        groups => JinjaValue::from(Serde(groups)),
        summary => JinjaValue::from(Serde(summary)),
        preview => JinjaValue::from(Serde(preview)),
        hidden => JinjaValue::from(Serde(hidden)),
        batch => JinjaValue::from(Serde(batch_info)),
        latest => JinjaValue::from(Serde(latest)),
        undone => first(&pairs, "undone").and_then(|b| b.parse::<i64>().ok()),
        error => error,
        self_url => href(&query_string(work, &query, replacement.as_deref())),
        preview_url => href(&format!("{}&vorschau=1", query_string(work, &query, replacement.as_deref()))),
        back_url => href(&match query.key { Some(k) => format!("/?work={work}&key={k}"), None => format!("/?work={work}") }),
    };
    match templates().get_template("search.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Vorlage search.html: {e}")),
    }
}

/// `POST /suche/act`: Ersetzen bzw. Rückgängig ohne JavaScript, danach 303 zurück auf `/suche`.
async fn act(State(state): State<Arc<AppState>>, Form(pairs): Form<Vec<(String, String)>>) -> Response {
    let work = match first(&pairs, "work").and_then(|w| w.trim().parse::<WorkId>().ok()) {
        Some(w) => w,
        None => match state.default_work().await {
            Ok(w) => w,
            Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        },
    };
    let query = query_from_pairs(&pairs);
    let replacement = first(&pairs, "replacement").unwrap_or("").to_owned();
    let back = query_string(work, &query, None);
    if !flag(&pairs, "confirm") {
        let url = format!("{back}&fehler={}", crate::view::query_escape("Bitte das Häkchen „Ja, ersetzen“ setzen."));
        return Redirect::to(&url).into_response();
    }
    match first(&pairs, "method").unwrap_or("") {
        "search.replace" => {
            // Je Abschnitt `pick=KEY:BASE`; je Treffer `keep=KEY:STELLE` (nicht gesetzt = auslassen).
            let mut keep: HashMap<i64, Vec<usize>> = HashMap::new();
            for v in values(&pairs, "keep") {
                if let Some((k, i)) = v.split_once(':')
                    && let (Ok(k), Ok(i)) = (k.parse::<i64>(), i.parse::<usize>())
                {
                    keep.entry(k).or_default().push(i);
                }
            }
            let mut picks = Vec::new();
            for v in values(&pairs, "pick") {
                if let Some((k, b)) = v.split_once(':')
                    && let (Ok(key), Ok(base_rev)) = (k.parse::<i64>(), b.parse::<u32>())
                {
                    let count = values(&pairs, &format!("count_{key}")).first().and_then(|c| c.parse::<usize>().ok());
                    let kept = keep.get(&key).cloned().unwrap_or_default();
                    let skip = match count {
                        Some(n) => (0..n).filter(|i| !kept.contains(i)).collect(),
                        None => Vec::new(),
                    };
                    picks.push(Pick { key, base_rev, skip });
                }
            }
            if picks.is_empty() {
                let url = format!("{back}&fehler={}", crate::view::query_escape("Kein Abschnitt ausgewählt."));
                return Redirect::to(&url).into_response();
            }
            match super::replace::replace(&state, work, &query, &replacement, Some(&picks)).await {
                Ok(done) => Redirect::to(&format!("{back}&batch={}", done.batch.id)).into_response(),
                Err(e) => {
                    let url = format!(
                        "{}&vorschau=1&fehler={}",
                        query_string(work, &query, Some(&replacement)),
                        crate::view::query_escape(&e.message)
                    );
                    Redirect::to(&url).into_response()
                }
            }
        }
        "search.undo" => {
            let Some(batch) = first(&pairs, "batch").and_then(|b| b.trim().parse::<i64>().ok()) else {
                return Redirect::to(&format!("{back}&fehler=batch")).into_response();
            };
            match super::replace::undo(&state, work, batch).await {
                Ok(_) => Redirect::to(&format!("{back}&undone={batch}")).into_response(),
                Err(e) => {
                    Redirect::to(&format!("{back}&fehler={}", crate::view::query_escape(&e.message))).into_response()
                }
            }
        }
        _ => error_page(StatusCode::BAD_REQUEST, "Unbekannte Aktion (method)."),
    }
}

/// Seiten (lesend).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/suche", get(search_page))
}

/// Formulare (hinter `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/suche/act", post(act))
}
