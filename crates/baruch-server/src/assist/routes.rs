// SPDX-License-Identifier: AGPL-3.0-or-later
//! Routes of the persona orders (contract Round 10, §5.6), both behind `guard::same_origin`
//! (mounted through [`crate::personas::writes`]):
//!
//! - `POST /assistenten/act` (form, without JavaScript): `method` ∈ `persona.ask`, `persona.review`,
//!   `persona.propose`, `persona.chat`, plus `work`, `key`, `persona`, `question`/`message`/`instruction`,
//!   `rev`, `view`, `lq`, since Round 15 `scope` (`section` | `chapter`, field from
//!   `fragments/assist_scope.html`, goes unchanged to the RPC). After completion 303 to the key address
//!   (`view::node_url`; for a proposal
//!   with `&rev=` of the new proposal). Errors (Round 15): instead of 422 the notice page [`notice_page`]
//!   (`templates/assist_notice.html`, status 200, message and link back to the section); if a persona fails in the
//!   chat, the same page names it (the human's post and finished answers are stored).
//! - `POST /assistenten/stream` (`Content-Type: application/json`, body `{method, …}` like RPC) →
//!   `text/event-stream` with the events `start` `{persona, style}`, `delta` `{text}`, since Round 15
//!   `posted` `{post: {id, author, body}}` (sender's post stored, before the first `delta`), `retry`
//!   `{persona, reason}` (answer is requested anew; discard what was shown of this persona, since Round 18b only
//!   that of the running piece), `compressed` `{message, summarized, chunks}`, since Round 18b `piece`
//!   `{persona, index, count}` (the next piece of a chunked order starts), then `done` `{kind: "post"|"proposal",
//!   post?|rev?, posts?, url, truncated, compressed: null|{…}}` and `error` `{message}`. If the client disconnects,
//!   the stream drops and with it the model call; for a single order nothing is stored then, in the chat the
//!   sender's post and answers already finished stay.
//!
//! Round 30c (chat-bedienung): the form field `basis` (review basis `shown` | `current` | `<n>`) goes to the RPC for
//! all four methods. If a single order fails, [`crate::chat_live::record_failure`] notes the error for the
//! live view of the section (`post.since`).

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_stream::Stream;

use super::Progress;
use crate::AppState;

/// Allowed methods of both routes.
const ALLOWED: &[&str] = &["persona.ask", "persona.review", "persona.propose", "persona.chat"];

#[derive(Deserialize, Default)]
pub struct ActForm {
    method: Option<String>,
    work: Option<String>,
    key: Option<String>,
    node: Option<String>,
    persona: Option<String>,
    question: Option<String>,
    message: Option<String>,
    instruction: Option<String>,
    rev: Option<String>,
    view: Option<String>,
    lq: Option<String>,
    /// Round 15: review/proposal for the section or the whole chapter (`section` | `chapter`).
    scope: Option<String>,
    /// Round 28: checkbox „als Auftrag“ in the chat (`1` or `on`): a write order becomes a proposal.
    as_order: Option<String>,
    /// Round 30c: review basis (`shown` | `current` | `<n>`, selection `fragments/basis_select.html`), passed through
    /// for all four methods (read in `assist/rpc.rs` or `chat::run_chat`).
    basis: Option<String>,
}

fn filled(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

/// RPC parameters from the form: address fields and `scope` only if filled; texts literally.
fn act_params(form: &ActForm) -> Value {
    let mut params = json!({});
    for (name, value) in [
        ("work", &form.work),
        ("key", &form.key),
        ("node", &form.node),
        ("persona", &form.persona),
        ("rev", &form.rev),
        ("scope", &form.scope),
        ("as_order", &form.as_order),
        ("basis", &form.basis),
    ] {
        if let Some(v) = filled(value) {
            params[name] = json!(v);
        }
    }
    for (name, value) in [("question", &form.question), ("message", &form.message), ("instruction", &form.instruction)]
    {
        if let Some(v) = value {
            params[name] = json!(v);
        }
    }
    params
}

/// Environment of the notice page: that of the work page (there stands `fragments/toast.html` of the lector) and
/// `assist_notice.html`.
fn notice_env() -> &'static minijinja::Environment<'static> {
    static ENV: std::sync::OnceLock<minijinja::Environment<'static>> = std::sync::OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = crate::view::environment();
        env.add_template("assist_notice.html", include_str!("../../templates/assist_notice.html"))
            .expect("Vorlage assist_notice.html fehlerhaft");
        env
    })
}

/// Self-built address for `href`: only without unsafe characters, `&` escaped; otherwise the start page.
fn safe_back(url: &str) -> minijinja::Value {
    let ok = url.starts_with('/')
        && !url.starts_with("//")
        && !url.chars().any(|c| matches!(c, '"' | '<' | '>' | '\'' | ' ' | '\\') || c.is_control());
    minijinja::Value::from_safe_string(if ok { url.replace('&', "&amp;") } else { "/".to_owned() })
}

/// Notice page without JavaScript (status 200): message, link `back` (built by the server) and – with `compressed`
/// (`{message, summarized, chunks}`) – the toast fragment. For persona orders and „an Persona geben“.
pub fn notice_page(message: &str, back: &str, compressed: Option<&Value>) -> Response {
    notice_page_with(message, &[], back, compressed)
}

/// Like [`notice_page`], with further lines (e.g. per failed persona).
pub fn notice_page_with(message: &str, lines: &[String], back: &str, compressed: Option<&Value>) -> Response {
    let env = notice_env();
    let toast = compressed.filter(|c| c.is_object()).map(|c| {
        minijinja::context! {
            message => c["message"].as_str().unwrap_or("Komprimiert."),
            level => "warn",
            summarized => minijinja::Value::from(minijinja::value::Serde(c["summarized"].clone())),
        }
    });
    let title = if toast.is_some() { "Antwort gespeichert" } else { "Hinweis" };
    let back_label = if toast.is_some() { "Weiter zum Abschnitt" } else { "Zurück zum Abschnitt" };
    let ctx = minijinja::context! {
        settings => minijinja::Value::from(minijinja::value::Serde(crate::view::Settings::default())),
        title => title, message => message, lines => lines.to_vec(), back => safe_back(back), back_label => back_label,
        toast => toast, toast_template => env.get_template("fragments/toast.html").is_ok(),
    };
    match env.get_template("assist_notice.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (StatusCode::OK, axum::response::Html(html)).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage assist_notice.html: {e}")).into_response()
        }
    }
}

/// Round 30c: note a failed single order (ask, review, proposal) for the live view
/// ([`crate::chat_live::record_failure`]); errors in the chat are noted by `chat::chat` per persona itself. Without
/// a resolvable section only the message to the caller remains.
async fn remember_failure(state: &AppState, method: &str, params: &Value, message: &str) {
    if method == "persona.chat" {
        return;
    }
    if let Ok((work, key, _)) = super::rpc::address(state, params).await {
        let persona = params["persona"].as_str().unwrap_or_default();
        crate::chat_live::record_failure(work, key, persona, message);
    }
}

/// `POST /assistenten/act`.
pub async fn act(State(state): State<Arc<AppState>>, Form(form): Form<ActForm>) -> Response {
    let method = form.method.as_deref().unwrap_or("").trim().to_owned();
    if !ALLOWED.contains(&method.as_str()) {
        return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response();
    }
    let params = act_params(&form);
    let preview = form.view.as_deref() == Some("preview");
    let lq = crate::view::lq_param(form.lq.as_deref());
    let form_work: Option<i64> = filled(&form.work).and_then(|w| w.parse().ok());
    let form_key: Option<i64> = filled(&form.key).and_then(|k| k.parse().ok());
    let form_rev: Option<u32> = filled(&form.rev).and_then(|r| r.parse().ok());
    let result = super::rpc::call_with(&state, &method, &params, None).await;
    let answer = match result {
        Ok(answer) => answer,
        Err(e) => {
            remember_failure(&state, &method, &params, &e.message).await;
            // Nothing stored (single order) or only the post not stored (chat: error before storing).
            let back = match (form_work, form_key) {
                (Some(work), Some(key)) => crate::view::node_url(work, key, form_rev, preview, "", &lq),
                (Some(work), None) => format!("/?work={work}"),
                _ => "/".to_owned(),
            };
            return notice_page(&e.message, &back, None);
        }
    };
    let work = answer["work"].as_i64().or(form_work);
    let key = answer["key"].as_i64().or(form_key);
    let rev = match method.as_str() {
        "persona.propose" => answer["rev"].as_u64().and_then(|r| u32::try_from(r).ok()),
        _ => form_rev,
    };
    let url = match (work, key) {
        (Some(work), Some(key)) => crate::view::node_url(work, key, rev, preview, "", &lq),
        _ => answer["url"].as_str().unwrap_or("/").to_owned(),
    };
    // Chat: the human's post exists, a persona failed – report honestly instead of redirecting silently.
    if let Some(errors) = answer["errors"].as_array().filter(|e| !e.is_empty()) {
        let lines: Vec<String> = errors
            .iter()
            .map(|e| format!("{}: {}", e["persona"].as_str().unwrap_or("?"), e["message"].as_str().unwrap_or("")))
            .collect();
        let compressed = answer.get("compressed").filter(|c| c.is_object());
        return notice_page_with(
            "Dein Beitrag und die fertigen Antworten sind gespeichert, aber nicht jede Persona hat geantwortet:",
            &lines,
            &url,
            compressed,
        );
    }
    // Compressed (Round 15): do not redirect silently, but show the notice and the link onward.
    if let Some(compressed) = answer.get("compressed").filter(|c| c.is_object()) {
        let message = match method.as_str() {
            "persona.propose" => "Der Vorschlag ist angelegt.",
            _ => "Die Antwort ist gespeichert.",
        };
        return notice_page(message, &url, Some(compressed));
    }
    // Round 28: after a chat message without JavaScript to the newest post (anchor at the end of the chat).
    if method == "persona.chat" {
        return Redirect::to(&format!("{url}#chat-neueste")).into_response();
    }
    Redirect::to(&url).into_response()
}

/// Stream from an order: first delivers the collected events, at the end `done` or `error`.
/// If the stream drops, the order drops (and with it the model call).
struct RunStream {
    run: Option<Pin<Box<dyn Future<Output = Event> + Send>>>,
    events: UnboundedReceiver<Event>,
    last: Option<Event>,
}

impl Stream for RunStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(Some(event)) = self.events.poll_recv(cx) {
            return Poll::Ready(Some(Ok(event)));
        }
        if let Some(run) = self.run.as_mut() {
            match run.as_mut().poll(cx) {
                Poll::Ready(last) => {
                    self.run = None;
                    self.last = Some(last);
                }
                Poll::Pending => return Poll::Pending,
            }
        }
        // Order finished (its sender is gone): first the rest of the events, then the last one.
        if let Ok(event) = self.events.try_recv() {
            return Poll::Ready(Some(Ok(event)));
        }
        Poll::Ready(self.last.take().map(Ok))
    }
}

fn event(name: &str, data: Value) -> Event {
    Event::default().event(name).data(data.to_string())
}

/// `POST /assistenten/stream`.
pub async fn stream(State(state): State<Arc<AppState>>, headers: HeaderMap, body: String) -> Response {
    if !crate::guard::is_json(&headers) {
        return crate::guard::need_json();
    }
    let request: Value = match serde_json::from_str(&body) {
        Ok(value @ Value::Object(_)) => value,
        _ => return (StatusCode::BAD_REQUEST, "JSON-Objekt {method, …} erwartet").into_response(),
    };
    let method = request["method"].as_str().unwrap_or("").to_owned();
    if !ALLOWED.contains(&method.as_str()) {
        return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response();
    }
    let (tx, events) = unbounded_channel::<Event>();
    let run = async move {
        let (progress_tx, mut progress_rx) = unbounded_channel::<Progress>();
        let work = super::rpc::call_with(&state, &method, &request, Some(&progress_tx));
        tokio::pin!(work);
        let forward = |item: Progress| {
            let _ = tx.send(match item {
                Progress::Start { persona, style } => event("start", json!({ "persona": persona, "style": style })),
                Progress::Delta(text) => event("delta", json!({ "text": text })),
                Progress::Posted(post) => event("posted", json!({ "post": post })),
                Progress::Retry { persona, reason } => event("retry", json!({ "persona": persona, "reason": reason })),
                Progress::Compressed(c) => {
                    event("compressed", json!({ "message": c.message, "summarized": c.summarized, "chunks": c.chunks }))
                }
                Progress::Piece { persona, index, count } => {
                    event("piece", json!({ "persona": persona, "index": index, "count": count }))
                }
            });
        };
        let result = loop {
            tokio::select! {
                biased;
                Some(item) = progress_rx.recv() => forward(item),
                result = &mut work => break result,
            }
        };
        while let Ok(item) = progress_rx.try_recv() {
            forward(item);
        }
        match result {
            Ok(answer) => {
                let kind = if method == "persona.propose" { "proposal" } else { "post" };
                if let Some(errors) = answer["errors"].as_array() {
                    for e in errors {
                        let message = format!(
                            "{}: {}",
                            e["persona"].as_str().unwrap_or("?"),
                            e["message"].as_str().unwrap_or("")
                        );
                        let _ = tx.send(event("error", json!({ "message": message })));
                    }
                }
                let mut done = json!({ "kind": kind, "url": answer["url"], "truncated": answer["truncated"],
                                       "compressed": answer.get("compressed").cloned().unwrap_or(Value::Null) });
                for field in ["post", "rev", "posts"] {
                    if answer.get(field).is_some_and(|v| !v.is_null()) {
                        done[field] = answer[field].clone();
                    }
                }
                if let Some(last) = answer["posts"].as_array().and_then(|p| p.last()) {
                    done["post"] = last.clone();
                }
                event("done", done)
            }
            Err(e) => {
                remember_failure(&state, &method, &request, &e.message).await;
                event("error", json!({ "message": e.message }))
            }
        }
    };
    let stream = RunStream { run: Some(Box::pin(run)), events, last: None };
    // Round 39a: the body is polled after the handler returned, outside the scope of `i18n::layer`.
    let stream = crate::i18n::scoped_stream(crate::i18n::current(), stream);
    let mut response = Sse::new(stream).into_response();
    response.headers_mut().insert(axum::http::header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scope_of_the_form_goes_to_the_rpc_and_empty_fields_do_not() {
        let form = ActForm {
            method: Some("persona.review".into()),
            work: Some("3".into()),
            key: Some(" 17 ".into()),
            scope: Some("chapter".into()),
            instruction: Some(String::new()),
            rev: Some(String::new()),
            ..ActForm::default()
        };
        let params = act_params(&form);
        assert_eq!(params["scope"], "chapter", "scope aus dem Formular erreicht die RPC (assist/rpc.rs parst es)");
        assert_eq!(params["key"], "17", "Adressfelder getrimmt");
        assert!(params.get("rev").is_none(), "leere Adressfelder fehlen (Vorgabe: aktuelle Fassung)");
        assert_eq!(params["instruction"], "", "Texte gehen wörtlich (auch leer) an die RPC");
        let without = act_params(&ActForm { scope: Some("  ".into()), ..ActForm::default() });
        assert!(without.get("scope").is_none(), "ohne Wahl kein scope (Vorgabe section in der RPC)");
    }
}
