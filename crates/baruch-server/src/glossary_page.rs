// SPDX-License-Identifier: AGPL-3.0-or-later
//! Glossary page and form “at the selection” without JavaScript (Round 31c, contract §3.4; owner kontextmenue).
//!
//! - `GET /werke/{id}/glossar` (query `begriff` = pre-filled new term, `nachschlagen` = word, `id` = term
//!   whose edit form is open, `ok` = notice after success): accepted terms and – separately,
//!   marked – the proposals of the assistants, forms create/edit/delete/accept/reject, anchor
//!   `#begriff-{id}`.
//! - `POST /werke/{id}/glossar/act` (field `action` = add|update|delete|accept|reject|suggest; `id`, `term`, `short`,
//!   `long`, `variants` and `bible_refs` one per line, `source`, `confirm`, `back`, for suggest optionally `key`) → 303
//!   to `back` (otherwise the glossary page) with `ok=<action>`, errors 422 with the glossary page, message and inputs.
//!   `suggest` („Begriffe vorschlagen lassen“, rework R31) calls RPC `glossary.suggest` (lector by setting,
//!   whole work or section `key`) and redirects to the proposals of the glossary page
//!   (`?ok=suggest&neu=N&uebersprungen=M#vorschlaege`); created only as a proposal.
//! - `GET /werke/{id}/markierung` (query `key`, `rev` (missing = current revision), `wort`): the three actions of the
//!   context menu as forms („In Glossar aufnehmen“ → `glossar/act`, „In Register aufnehmen“ → `register/act`,
//!   „Im Glossar nachschlagen“ → `glossar?nachschlagen=`) and the index entries of the section with „Entfernen“.
//!
//! No data storage of its own: reading and writing only through [`crate::glossary`] and [`crate::index_marks`];
//! their messages appear honestly on the page.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::{GlossaryEdit, WorkId};
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INTERNAL, RpcError};
use crate::view::Settings;

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("glossary.html", include_str!("../templates/glossary.html")),
            ("marking.html", include_str!("../templates/marking.html")),
            ("fragments/context_menu.html", include_str!("../templates/fragments/context_menu.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Glossars fehlerhaft");
        }
        env
    })
}

pub(crate) fn plain(status: StatusCode, message: &str) -> Response {
    let escaped = message.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    (status, Html(format!("<!doctype html><meta charset=\"utf-8\"><title>Baruch</title><p>{escaped}</p>")))
        .into_response()
}

// ---- Shared building blocks (also for index_page.rs) --------------------------------------------------------------

/// First value of a form field (untrimmed); missing → "".
pub(crate) fn field<'a>(fields: &'a [(String, String)], name: &str) -> &'a str {
    fields.iter().find(|(k, _)| k == name).map_or("", |(_, v)| v.as_str())
}

/// All lines of all fields `name` (one value per line; empty lines dropped, trimmed).
pub(crate) fn lines(fields: &[(String, String)], name: &str) -> Vec<String> {
    fields
        .iter()
        .filter(|(k, _)| k == name)
        .flat_map(|(_, v)| v.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect::<Vec<_>>())
        .collect()
}

/// Is `back` an address of this server (path with `/`, no `//`, no backslash, no control characters)?
pub(crate) fn safe_back(back: &str) -> bool {
    back.starts_with('/')
        && !back.starts_with("//")
        && back.len() <= 2000
        && !back.contains('\\')
        && !back.chars().any(char::is_control)
}

/// Target after success: `back` (if safe, otherwise `default`) with `ok=<ok>` before the anchor; `anchor` replaces a
/// missing anchor.
pub(crate) fn back_to(back: &str, default: &str, ok: &str, anchor: Option<&str>) -> String {
    let target = if safe_back(back) { back } else { default };
    let (base, frag) = match target.split_once('#') {
        Some((base, frag)) => (base, Some(frag.to_owned())),
        None => (target, anchor.map(str::to_owned)),
    };
    let sep = if base.contains('?') { '&' } else { '?' };
    let mut out = format!("{base}{sep}ok={}", crate::view::query_escape(ok));
    if let Some(frag) = frag.filter(|f| !f.is_empty()) {
        out.push('#');
        out.push_str(&frag);
    }
    out
}

/// Status of a refused action: internal errors 500, otherwise 422 (invalid, refused, no model).
pub(crate) fn failure_status(e: &RpcError) -> StatusCode {
    if e.code == INTERNAL { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNPROCESSABLE_ENTITY }
}

/// Base context of every page: settings from the cookie and the work (id, title); unknown work → 404.
// `Response` as the error is intended here (finished response for `?` in the handler); a `Box` would change all
// callers.
#[allow(clippy::result_large_err)]
pub(crate) async fn page_base(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
) -> Result<(JinjaValue, JinjaValue), Response> {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let Ok(meta) = state.store.work_meta(work).await else {
        return Err(plain(StatusCode::NOT_FOUND, "Unbekanntes Werk."));
    };
    Ok((JinjaValue::from(Serde(&settings)), JinjaValue::from(Serde(json!({ "id": work, "title": meta.title })))))
}

/// Notice for `ok` after an action of the glossary page.
pub(crate) fn ok_message(ok: &str) -> Option<&'static str> {
    Some(match ok {
        "add" => "Begriff ins Glossar aufgenommen.",
        "update" => "Begriff geändert.",
        "delete" => "Begriff gelöscht (wiederherstellbar über den Spiegel).",
        "accept" => "Vorschlag übernommen – der Begriff steht jetzt im Glossar.",
        "reject" => "Vorschlag verworfen.",
        _ => return None,
    })
}

/// Notice of the glossary page for `ok` (for `suggest` with the numbers from `neu` and `uebersprungen`).
fn ok_text(query: &BTreeMap<String, String>) -> Option<String> {
    let ok = query.get("ok")?;
    if ok == "suggest" {
        let count = |name: &str| query.get(name).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        let (new, skipped) = (count("neu"), count("uebersprungen"));
        let mut text = match new {
            0 => "Der Lektor hat keine neuen Begriffe vorgeschlagen.".to_owned(),
            1 => "Der Lektor hat 1 Begriff vorgeschlagen – er steht unter „Vorschläge der Assistenten“.".to_owned(),
            n => format!("Der Lektor hat {n} Begriffe vorgeschlagen – sie stehen unter „Vorschläge der Assistenten“."),
        };
        if skipped > 0 {
            text.push_str(&format!(" {skipped} übersprungen (schon im Glossar oder vorgeschlagen bzw. unbrauchbar)."));
        }
        return Some(text);
    }
    ok_message(ok).map(str::to_owned)
}

// ---- Glossary page ------------------------------------------------------------------------------------------------

/// A rejected form: action, term (for update/delete/accept/reject), inputs and message.
#[derive(Debug, Clone, Default)]
struct Retry {
    action: String,
    id: Option<i64>,
    values: BTreeMap<String, String>,
    message: String,
}

/// Terms of the work through RPC `glossary.list` (status “all”), separated into accepted and proposed.
async fn load_terms(state: &AppState, work: WorkId) -> Result<(Vec<Value>, Vec<Value>), RpcError> {
    let answer = crate::glossary::call(state, work, "glossary.list", json!({ "work": work, "status": "all" })).await?;
    let terms = answer.get("terms").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(terms.into_iter().partition(|t| t.get("status").and_then(Value::as_str) != Some("proposed")))
}

/// Lines of a term for the text fields “one per line”.
fn joined_lines(term: &Value, key: &str) -> String {
    term.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

/// Term for the template: all fields, plus `variants_text`/`bible_refs_text` (one per line) and `filter` (search text).
fn term_view(term: &Value) -> Value {
    let mut view = term.clone();
    let text = |k: &str| term.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let filter = format!("{} {} {}", text("term"), joined_lines(term, "variants").replace('\n', " "), text("short"));
    if let Some(map) = view.as_object_mut() {
        map.insert("variants_text".into(), json!(joined_lines(term, "variants")));
        map.insert("bible_refs_text".into(), json!(joined_lines(term, "bible_refs")));
        map.insert("filter".into(), json!(filter));
    }
    view
}

async fn glossary_page(
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
    let (accepted, proposed, load_error) = match load_terms(state, work).await {
        Ok((a, p)) => (a.iter().map(term_view).collect(), p.iter().map(term_view).collect(), None),
        Err(e) => (Vec::new(), Vec::new(), Some(e.message)),
    };
    let lookup_word = query.get("nachschlagen").map(|w| w.trim().to_owned()).filter(|w| !w.is_empty());
    let lookup = match &lookup_word {
        Some(word) => {
            let answer =
                crate::glossary::call(state, work, "glossary.lookup", json!({ "work": work, "word": word })).await;
            Some(match answer {
                Ok(v) => json!({
                    "word": word,
                    "terms": v.get("terms").and_then(Value::as_array).map(|a| a.iter().map(term_view).collect::<Vec<_>>()).unwrap_or_default(),
                }),
                Err(e) => json!({ "word": word, "terms": [], "error": e.message }),
            })
        }
        None => None,
    };
    let open_id = retry.as_ref().and_then(|r| r.id).or_else(|| query.get("id").and_then(|v| v.parse::<i64>().ok()));
    let prefill = query.get("begriff").map(|b| b.trim().to_owned()).unwrap_or_default();
    let ctx = minijinja::context! {
        settings => settings,
        work => work_value,
        accepted => JinjaValue::from(Serde(&accepted)),
        proposed => JinjaValue::from(Serde(&proposed)),
        load_error => load_error,
        lookup => lookup.map(|l| JinjaValue::from(Serde(&l))),
        open_id => open_id,
        prefill => prefill,
        ok => ok_text(query),
        retry => retry.map(|r| minijinja::context! { action => r.action, id => r.id, v => r.values, message => r.message }),
    };
    render(status, "glossary.html", ctx)
}

fn render(status: StatusCode, template: &str, ctx: JinjaValue) -> Response {
    match templates().get_template(template).and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, &format!("Vorlage {template}: {e}")),
    }
}

/// `GET /werke/{id}/glossar`.
async fn glossary(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    glossary_page(&state, &headers, work, &query, None, StatusCode::OK).await
}

/// Inputs of a term from the form.
fn edit_from(fields: &[(String, String)]) -> GlossaryEdit {
    GlossaryEdit {
        term: field(fields, "term").trim().to_owned(),
        short: field(fields, "short").trim().to_owned(),
        long: field(fields, "long").trim().to_owned(),
        variants: lines(fields, "variants"),
        source: field(fields, "source").trim().to_owned(),
        bible_refs: lines(fields, "bible_refs"),
    }
}

/// `POST /werke/{id}/glossar/act`: `action` = add|update|delete|accept|reject|suggest → 303 or 422.
async fn act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let action = field(&fields, "action").trim().to_owned();
    let id = field(&fields, "id").trim().parse::<i64>().ok();
    let back = field(&fields, "back").trim().to_owned();
    let default = format!("/werke/{work}/glossar");
    let fail = |message: String, status: StatusCode| {
        let values = fields
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "action" | "confirm" | "back"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .rev()
            .collect::<BTreeMap<_, _>>();
        let retry = Retry { action: action.clone(), id, values, message };
        let state = state.clone();
        let headers = headers.clone();
        async move { glossary_page(&state, &headers, work, &BTreeMap::new(), Some(retry), status).await }
    };
    let need_id = || id.ok_or_else(|| "Begriff fehlt (Feld id).".to_owned());
    let result: Result<Value, Result<RpcError, String>> = match action.as_str() {
        "add" => crate::glossary::add(&state, work, edit_from(&fields)).await.map_err(Ok),
        "update" => match need_id() {
            Ok(id) => crate::glossary::update(&state, work, id, edit_from(&fields)).await.map_err(Ok),
            Err(m) => Err(Err(m)),
        },
        "delete" => match need_id() {
            Ok(_) if field(&fields, "confirm") != "1" => {
                Err(Err("Löschen bitte mit dem Häkchen bestätigen; nichts geändert.".to_owned()))
            }
            Ok(id) => crate::glossary::delete(&state, work, id).await.map_err(Ok),
            Err(m) => Err(Err(m)),
        },
        "accept" | "reject" => match need_id() {
            Ok(id) => crate::glossary::decide(&state, work, id, action == "accept").await.map_err(Ok),
            Err(m) => Err(Err(m)),
        },
        "suggest" => {
            // Proposals only (origin the persona); adopting stays with the human.
            let key = match field(&fields, "key").trim() {
                "" => Ok(None),
                k => k.parse::<i64>().map(Some).map_err(|_| format!("Abschnitt „{k}“: Zahl erwartet.")),
            };
            match key {
                Ok(key) => match crate::assist::glossary::suggest(&state, work, key, None).await {
                    Ok(answer) => {
                        let count = |name: &str| answer.get(name).and_then(Value::as_array).map_or(0, Vec::len);
                        let target = format!(
                            "{default}?ok=suggest&neu={}&uebersprungen={}#vorschlaege",
                            count("proposed"),
                            count("skipped")
                        );
                        return Redirect::to(&target).into_response();
                    }
                    Err(e) => Err(Ok(e)),
                },
                Err(m) => Err(Err(m)),
            }
        }
        _ => {
            return (StatusCode::BAD_REQUEST, "Unbekannte Aktion (add, update, delete, accept, reject oder suggest).")
                .into_response();
        }
    };
    match result {
        Ok(answer) => {
            let term_id = answer.pointer("/term/id").and_then(Value::as_i64).or(id).filter(|_| action != "delete");
            let anchor = term_id.map(|t| format!("begriff-{t}"));
            Redirect::to(&back_to(&back, &default, &action, anchor.as_deref())).into_response()
        }
        Err(Ok(e)) => fail(e.message.clone(), failure_status(&e)).await,
        Err(Err(message)) => fail(message, StatusCode::UNPROCESSABLE_ENTITY).await,
    }
}

// ---- Form “at the selection” --------------------------------------------------------------------------------------

/// `GET /werke/{id}/markierung?key=K[&rev=R][&wort=W]`.
async fn marking(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    let (settings, work_value) = match page_base(&state, &headers, work).await {
        Ok(base) => base,
        Err(response) => return response,
    };
    let word = query.get("wort").map(|w| w.trim().to_owned()).unwrap_or_default();
    let key = query.get("key").and_then(|k| k.trim().parse::<i64>().ok());
    let mut status = StatusCode::OK;
    let mut problem: Option<String> = None;
    let mut section: Option<Value> = None;
    match key {
        None => {
            status = StatusCode::BAD_REQUEST;
            problem = Some("Welcher Abschnitt? Der Link braucht key (und wahlweise rev).".to_owned());
        }
        Some(key) => match state.store.load_work(work).await {
            Ok(loaded) => match loaded.nodes.iter().find(|n| n.key == key) {
                Some(node) => {
                    let current = node.current_number();
                    let rev = query.get("rev").and_then(|r| r.trim().parse::<u32>().ok()).or(current);
                    match rev.filter(|r| node.revision(*r).is_some()) {
                        Some(rev) => {
                            section = Some(json!({
                                "key": key, "rev": rev, "current": current, "number": node.number, "title": node.title,
                                "is_current": Some(rev) == current,
                            }));
                        }
                        None => {
                            status = StatusCode::NOT_FOUND;
                            problem = Some(format!(
                                "Fassung {} gibt es in diesem Abschnitt nicht.",
                                query.get("rev").map_or("", |r| r.as_str())
                            ));
                        }
                    }
                }
                None => {
                    status = StatusCode::NOT_FOUND;
                    problem = Some(format!("Abschnitt mit Schlüssel {key} gibt es nicht (mehr)."));
                }
            },
            Err(e) => {
                status = StatusCode::INTERNAL_SERVER_ERROR;
                problem = Some(format!("Fehler: {e}"));
            }
        },
    }
    // Index entries of the section (current revision) with „Entfernen“; errors honestly.
    let (entries, entries_error) = match (&section, key) {
        (Some(_), Some(key)) => match crate::index_marks::entries(&state, work, Some(key)).await {
            Ok(v) => (v.get("entries").cloned().unwrap_or_else(|| json!([])), None),
            Err(e) => (json!([]), Some(e.message)),
        },
        _ => (json!([]), None),
    };
    let registers =
        crate::glossary::registers(&state, work).await.unwrap_or_else(|_| baruch_typst::index::builtin_registers());
    let back = match key {
        Some(k) if word.is_empty() => format!("/werke/{work}/markierung?key={k}"),
        Some(k) => format!("/werke/{work}/markierung?key={k}&wort={}", crate::view::query_escape(&word)),
        None => format!("/werke/{work}/glossar"),
    };
    let ok = query.get("ok").and_then(|ok| ok_message(ok).or_else(|| crate::index_page::ok_message(ok)));
    let ctx = minijinja::context! {
        settings => settings,
        work => work_value,
        word => word,
        section => section.map(|s| JinjaValue::from(Serde(&s))),
        problem => problem,
        entries => JinjaValue::from(Serde(&entries)),
        entries_error => entries_error,
        registers => JinjaValue::from(Serde(&registers)),
        // Only digits, fixed words and query_escape: safe as an attribute (| safe), & escaped.
        back => back.replace('&', "&amp;"),
        ok => ok,
    };
    render(status, "marking.html", ctx)
}

/// Pages (`GET /werke/{id}/glossar`, `GET /werke/{id}/markierung`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/glossar", get(glossary)).route("/werke/{id}/markierung", get(marking))
}

/// Writing routes (`POST /werke/{id}/glossar/act`, behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/glossar/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_targets_stay_on_this_server_and_carry_the_hint_before_the_anchor() {
        assert_eq!(
            back_to("/werke/3/markierung?key=7", "/werke/3/glossar", "add", Some("begriff-5")),
            "/werke/3/markierung?key=7&ok=add#begriff-5",
            "eigene Adresse mit Frage: ok mit &, Anker dahinter"
        );
        assert_eq!(
            back_to("/werke/3/register#register-orte", "/x", "register_add", None),
            "/werke/3/register?ok=register_add#register-orte",
            "vorhandener Anker bleibt, ok davor"
        );
        for foreign in ["https://example.org/", "//example.org/x", "/a\\b", "werke/3", "/a\nb"] {
            assert_eq!(
                back_to(foreign, "/werke/3/glossar", "add", None),
                "/werke/3/glossar?ok=add",
                "fremde oder kaputte Rücksprungadresse {foreign:?} → Vorgabe (kein offener Weiterleiter)"
            );
        }
    }

    #[test]
    fn lines_read_one_value_per_line_from_every_field() {
        let fields = vec![
            ("variants".to_owned(), " Huld \n\ngnädig\r\n".to_owned()),
            ("variants".to_owned(), "Erbarmen".to_owned()),
            ("term".to_owned(), "Gnade".to_owned()),
        ];
        assert_eq!(lines(&fields, "variants"), ["Huld", "gnädig", "Erbarmen"], "je Zeile ein Wert, leere fallen weg");
        assert!(lines(&fields, "bible_refs").is_empty(), "fehlendes Feld = keine Werte");
    }
}
