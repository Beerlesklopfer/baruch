// SPDX-License-Identifier: AGPL-3.0-or-later
//! Index page without JavaScript (Round 31c, contract §3.4; owned by kontextmenue).
//!
//! - `GET /werke/{id}/register` (query `key` = section whose index entries with „Setzen“/„Entfernen“ stand at the
//!   top, `ok` = notice after success): overview per index (`#register-{slug}`), open index proposals of the
//!   assistants (`#vorschlag-{id}`, labelled, „Übernehmen“/„Verwerfen“ only through a form of the human),
//!   decided proposals, create/rename/delete own indexes (built-in ones are only shown).
//! - `POST /werke/{id}/register/act` (`action` = mark|unmark|accept|reject|register_add|register_rename|register_delete;
//!   fields key, rev, word, occurrence, register, term, sub, see, also, start, end, id, slug, title, confirm, back) →
//!   303 to `back` (otherwise the index page) with `ok=<action>`, error 422 with the index page including the message
//!   and the inputs.
//!
//! No data storage of its own: only [`crate::index_marks`] (marks, overview) and [`crate::glossary`] (indexes,
//! proposals); their messages appear honestly on the page.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::WorkId;
use baruch_typst::index::{IndexMark, Occurrence, Register, builtin_registers};
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde_json::{Value, json};

use crate::AppState;
use crate::glossary_page::{back_to, failure_status, field, page_base, plain};
use crate::index_marks::MarkRequest;
use crate::rpc::RpcError;

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("index_page.html", include_str!("../templates/index_page.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Register fehlerhaft");
        }
        env
    })
}

/// Notice for `ok` after an action of the index page (also on the page „an der Markierung“).
pub(crate) fn ok_message(ok: &str) -> Option<&'static str> {
    Some(match ok {
        "mark" => "Registermarke gesetzt – als neue Fassung des Abschnitts.",
        "unmark" => "Registermarke entfernt – als neue Fassung des Abschnitts.",
        "accept" => "Registervorschlag übernommen – die Marke steht in einer neuen Fassung.",
        "reject" => "Registervorschlag verworfen.",
        "register_add" => "Register angelegt.",
        "register_rename" => "Register umbenannt.",
        "register_delete" => "Register gelöscht.",
        _ => return None,
    })
}

/// A refused form: action, inputs and message.
#[derive(Debug, Clone, Default)]
struct Retry {
    action: String,
    values: BTreeMap<String, String>,
    message: String,
}

/// Index for the template: slug, title, kind, builtin.
fn register_view(register: &Register) -> Value {
    let builtin = builtin_registers().iter().any(|b| b.slug == register.slug);
    json!({ "slug": register.slug, "title": register.title, "kind": register.kind, "builtin": builtin })
}

async fn index_page(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    query: &BTreeMap<String, String>,
    retry: Option<Retry>,
    status: StatusCode,
) -> Response {
    let (settings, work_value) = match page_base(state, headers, work).await {
        Ok(base) => base,
        Err(response) => return response,
    };
    let registers = match crate::glossary::registers(state, work).await {
        Ok(list) => list,
        Err(_) => builtin_registers(),
    };
    // Overview per index (through index_marks::overview), in the order of the indexes.
    let (overview, overview_error) = match crate::index_marks::overview(state, work).await {
        Ok(views) => (views, None),
        Err(e) => (Vec::new(), Some(e.message)),
    };
    // Locations and proposals name the section with number and title instead of the inner key (visual check R31).
    let loaded = state.store.load_work(work).await.ok();
    let label_of = |key: i64| -> String {
        loaded
            .as_ref()
            .and_then(|w| w.nodes.iter().find(|n| n.key == key))
            .map(|n| if n.number.is_empty() { n.title.clone() } else { format!("{} {}", n.number, n.title) })
            .unwrap_or_else(|| format!("Abschnitt {key}"))
    };
    let sections: Vec<Value> = registers
        .iter()
        .map(|r| {
            let terms =
                overview.iter().find(|v| v.register.slug == r.slug).map(|v| v.terms.clone()).unwrap_or_default();
            let terms: Vec<Value> = terms
                .iter()
                .map(|t| {
                    let keys: Vec<Value> = t
                        .keys
                        .iter()
                        .map(|k| json!({ "key": k, "label": label_of(*k), "url": format!("/?work={work}&key={k}") }))
                        .collect();
                    json!({ "term": t.term, "sub": t.sub, "keys": keys, "see": t.see, "also": t.also, "auto": t.auto,
                            "filter": format!("{} {}", t.term, t.sub) })
                })
                .collect();
            json!({ "register": register_view(r), "terms": terms })
        })
        .collect();
    let (proposals, proposals_error) =
        match crate::glossary::call(state, work, "index.proposals", json!({ "work": work, "status": "all" })).await {
            Ok(v) => (v.get("proposals").and_then(Value::as_array).cloned().unwrap_or_default(), None),
            Err(e) => (Vec::new(), Some(e.message)),
        };
    let proposals: Vec<Value> = proposals
        .into_iter()
        .map(|mut p| {
            let key = p.get("key").and_then(Value::as_i64).unwrap_or_default();
            let occurrence = match p.get("occurrence") {
                Some(Value::String(s)) if s == "first" => "erste Fundstelle".to_owned(),
                Some(Value::String(s)) if s == "all" => "alle Fundstellen".to_owned(),
                Some(Value::String(s)) => format!("{s}. Fundstelle"),
                Some(Value::Number(n)) => format!("{n}. Fundstelle"),
                _ => String::new(),
            };
            p["section_label"] = json!(label_of(key));
            p["occurrence_label"] = json!(occurrence);
            p
        })
        .collect();
    let (open, decided): (Vec<Value>, Vec<Value>) =
        proposals.into_iter().partition(|p| p.get("status").and_then(Value::as_str) == Some("open"));
    // Section from `key`: entries of its current revision and a form for setting.
    let key = query.get("key").and_then(|k| k.trim().parse::<i64>().ok());
    let mut section = None;
    if let Some(key) = key
        && let Ok(loaded) = state.store.load_work(work).await
        && let Some(node) = loaded.nodes.iter().find(|n| n.key == key)
    {
        let (entries, error) = match crate::index_marks::entries(state, work, Some(key)).await {
            Ok(v) => (v.get("entries").cloned().unwrap_or_else(|| json!([])), None),
            Err(e) => (json!([]), Some(e.message)),
        };
        section = Some(json!({
            "key": key, "rev": node.current_number(), "number": node.number, "title": node.title,
            "entries": entries, "error": error,
        }));
    }
    let custom = registers.iter().filter(|r| !builtin_registers().iter().any(|b| b.slug == r.slug)).count();
    let ctx = minijinja::context! {
        settings => settings,
        work => work_value,
        registers => JinjaValue::from(Serde(registers.iter().map(register_view).collect::<Vec<_>>())),
        sections => JinjaValue::from(Serde(&sections)),
        overview_error => overview_error,
        open => JinjaValue::from(Serde(&open)),
        decided => JinjaValue::from(Serde(&decided)),
        proposals_error => proposals_error,
        section => section.map(|s| JinjaValue::from(Serde(&s))),
        custom_left => baruch_typst::index::MAX_CUSTOM_REGISTERS.saturating_sub(custom),
        ok => query.get("ok").and_then(|ok| ok_message(ok)),
        retry => retry.map(|r| minijinja::context! { action => r.action, v => r.values, message => r.message }),
    };
    match templates().get_template("index_page.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, &format!("Vorlage index_page.html: {e}")),
    }
}

/// `GET /werke/{id}/register`.
async fn page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    index_page(&state, &headers, work, &query, None, StatusCode::OK).await
}

/// Number from the form or a message with the field name.
fn number<T: std::str::FromStr>(fields: &[(String, String)], name: &str, label: &str) -> Result<T, String> {
    field(fields, name).trim().parse::<T>().map_err(|_| format!("{label}: keine gültige Zahl (Feld {name})."))
}

/// Mark from the form (index default „stichwort“, term default the word), checked as in typesetting.
fn mark_from(fields: &[(String, String)]) -> Result<IndexMark, String> {
    let get = |name: &str| field(fields, name).trim().to_owned();
    let register = Some(get("register")).filter(|r| !r.is_empty()).unwrap_or_else(|| "stichwort".to_owned());
    let term = Some(get("term")).filter(|t| !t.is_empty()).unwrap_or_else(|| get("word"));
    let mark = IndexMark { sub: get("sub"), see: get("see"), also: get("also"), ..IndexMark::new(&register, &term) };
    mark.validate().map_err(|e| e.to_string())?;
    Ok(mark)
}

/// `POST /werke/{id}/register/act` → 303 or 422.
async fn act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let action = field(&fields, "action").trim().to_owned();
    let back = field(&fields, "back").trim().to_owned();
    let key = field(&fields, "key").trim().parse::<i64>().ok();
    let default = match key {
        Some(k) if matches!(action.as_str(), "mark" | "unmark") => format!("/werke/{work}/register?key={k}"),
        _ => format!("/werke/{work}/register"),
    };
    let slug = field(&fields, "slug").trim().to_owned();
    let title = field(&fields, "title").trim().to_owned();
    let id = field(&fields, "id").trim().parse::<i64>().ok();
    let result: Result<Value, Result<RpcError, String>> = match action.as_str() {
        "mark" => {
            let request = (|| -> Result<MarkRequest, String> {
                let key = number::<i64>(&fields, "key", "Abschnitt")?;
                let rev = number::<u32>(&fields, "rev", "Fassung")?;
                let word = field(&fields, "word").trim().to_owned();
                if word.is_empty() {
                    return Err("Wort: bitte das Wort angeben, hinter das die Marke kommt.".to_owned());
                }
                let raw = field(&fields, "occurrence").trim();
                let occurrence = if raw.is_empty() {
                    Occurrence::First
                } else {
                    Occurrence::from_param(raw)
                        .ok_or_else(|| format!("Fundstelle: „{raw}“ ist weder „erste“, „alle“ noch eine Zahl ab 1."))?
                };
                Ok(MarkRequest { key, rev, word, occurrence, mark: mark_from(&fields)? })
            })();
            match request {
                Ok(request) => crate::index_marks::mark(&state, work, request).await.map_err(Ok),
                Err(m) => Err(Err(m)),
            }
        }
        "unmark" => {
            let parsed = (|| -> Result<(i64, u32, usize, usize), String> {
                Ok((
                    number(&fields, "key", "Abschnitt")?,
                    number(&fields, "rev", "Fassung")?,
                    number(&fields, "start", "Anfang")?,
                    number(&fields, "end", "Ende")?,
                ))
            })();
            match parsed {
                Ok((key, rev, start, end)) => {
                    crate::index_marks::unmark(&state, work, key, rev, (start, end)).await.map_err(Ok)
                }
                Err(m) => Err(Err(m)),
            }
        }
        "accept" | "reject" => match id {
            Some(id) => crate::glossary::index_decide(&state, work, id, action == "accept").await.map_err(Ok),
            None => Err(Err("Registervorschlag fehlt (Feld id).".to_owned())),
        },
        "register_add" => crate::glossary::register_add(&state, work, &slug, &title).await.map_err(Ok),
        "register_rename" => crate::glossary::register_rename(&state, work, &slug, &title).await.map_err(Ok),
        "register_delete" if field(&fields, "confirm") != "1" => {
            Err(Err("Löschen bitte mit dem Häkchen bestätigen; nichts geändert.".to_owned()))
        }
        "register_delete" => crate::glossary::register_delete(&state, work, &slug).await.map_err(Ok),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                "Unbekannte Aktion (mark, unmark, accept, reject, register_add, register_rename, register_delete).",
            )
                .into_response();
        }
    };
    match result {
        Ok(_) => {
            let anchor = match action.as_str() {
                "register_add" | "register_rename" => Some(format!("register-{slug}")),
                "accept" | "reject" => id.map(|i| format!("vorschlag-{i}")),
                _ => None,
            };
            Redirect::to(&back_to(&back, &default, &action, anchor.as_deref())).into_response()
        }
        Err(problem) => {
            let (message, status) = match problem {
                Ok(e) => (e.message.clone(), failure_status(&e)),
                Err(m) => (m, StatusCode::UNPROCESSABLE_ENTITY),
            };
            let values = fields
                .iter()
                .filter(|(k, _)| !matches!(k.as_str(), "action" | "confirm" | "back"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .rev()
                .collect::<BTreeMap<_, _>>();
            let mut query = BTreeMap::new();
            if let (Some(k), "mark" | "unmark") = (key, action.as_str()) {
                query.insert("key".to_owned(), k.to_string());
            }
            index_page(&state, &headers, work, &query, Some(Retry { action, values, message }), status).await
        }
    }
}

/// Pages (`GET /werke/{id}/register`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/register", get(page))
}

/// Writing routes (`POST /werke/{id}/register/act`, behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/register/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn a_mark_from_the_form_defaults_to_the_keyword_register_and_the_word() {
        let mark = mark_from(&form(&[("word", "Zeder"), ("register", ""), ("term", " "), ("sub", "Libanon")]))
            .expect("gültig");
        assert_eq!(
            mark,
            IndexMark { sub: "Libanon".into(), ..IndexMark::new("stichwort", "Zeder") },
            "ohne Register das Stichwortregister, ohne Stichwort das Wort, Zusätze getrimmt"
        );
        let bad = mark_from(&form(&[("word", "Zeder"), ("register", "Orte!")])).expect_err("ungültige Kennung");
        assert!(bad.contains("keine gültige Kennung"), "Meldung von IndexMark::validate: {bad}");
    }
}
