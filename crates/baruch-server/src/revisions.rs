// SPDX-License-Identifier: AGPL-3.0-or-later
//! Amending the current revision: history, undo through the history and the setting „Fassungen“ per work
//! (Round 22, contract §2.1–2.4 and §4; ownership fassungen).
//!
//! - RPC: `revision.amends {node|key, rev}` (read only) → `{node, key, rev, amend_seq, steps}`;
//!   `revision.amend_undo {node|key, rev, seq}` (human only; not through MCP) → `{node, key, rev, amend_seq,
//!   commit?|mirror?}`; `revision.policy_get {work?}` → `{work, amend, max_change_percent, new_after_minutes,
//!   defaults}`; `revision.policy_set {work?, amend, max_change_percent, new_after_minutes}` (human only, not
//!   through `/act` and MCP) → `{work, changed, commit?|mirror?}`.
//! - Routes: `GET /werke/{id}/fassungen/verlauf?key=K&rev=R` (history with the word difference per step and the
//!   button „Bis hier zurück“), `POST /werke/fassungen/act` (`method=revision.amend_undo`, `work`, `key`, `rev`,
//!   `seq` → 303 to the history), `POST /einstellungen/werk/fassungen` (form Einstellungen → Werk → Fassungen →
//!   303 to `/einstellungen/werk?work=W&ok=fassungen#fassungen`, rejected `…&fassungenfehler=1#fassungen`).
//!
//! Every step (amendment, undo) and every change of the setting is exactly one commit in the mirror
//! (`mirror::record` or `mirror::record_revision_policy`). MCP only reads (`mcp::revisions`, `revision_amends`).

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::revision_policy::{MAX_CHANGE_PERCENT, NEW_AFTER_MINUTES, word_diff};
use baruch_core::{DiffKind, RevisionStatus};
use baruch_store::{AmendKind, RevisionPolicy, StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value as Tpl, context};
use serde_json::{Value, json};

use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, RpcError, add_mirror, resolve_keys};
use crate::{AppState, mirror};

/// RPC methods of this module.
pub fn handles(method: &str) -> bool {
    matches!(method, "revision.amends" | "revision.amend_undo" | "revision.policy_get" | "revision.policy_set")
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Number from 0 from `params[key]`, also as text (forms); missing, `null` or empty → `None`.
fn number(params: &Value, key: &str) -> Result<Option<u64>, RpcError> {
    let wrong = || invalid(tc("outline_msg.param.number", &[("name", key.into())]));
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_u64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

fn required_u32(params: &Value, key: &str) -> Result<u32, RpcError> {
    let value =
        number(params, key)?.ok_or_else(|| invalid(tc("outline_msg.param.missing", &[("name", key.into())])))?;
    u32::try_from(value).map_err(|_| invalid(tc("outline_msg.param.too_large", &[("name", key.into())])))
}

fn node_of(params: &Value) -> Result<usize, RpcError> {
    let value = number(params, "node")?.ok_or_else(|| invalid(tc("outline_msg.revision.node_missing", &[])))?;
    usize::try_from(value).map_err(|_| invalid(tc("outline_msg.param.too_large", &[("name", "node".into())])))
}

/// Truth value: `true`, `1`, `"1"`, `"on"`, `"true"` → true; `false`, `0`, `"0"`, `"off"`, `"false"`, `""` → false.
fn boolean(params: &Value, key: &str) -> Result<Option<bool>, RpcError> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(Value::Number(n)) if n.as_u64() == Some(1) => Ok(Some(true)),
        Some(Value::Number(n)) if n.as_u64() == Some(0) => Ok(Some(false)),
        Some(Value::String(s)) => match s.trim() {
            "1" | "on" | "true" => Ok(Some(true)),
            "0" | "off" | "false" | "" => Ok(Some(false)),
            _ => Err(invalid(tc("outline_msg.revision.switch", &[("name", key.into())]))),
        },
        Some(_) => Err(invalid(tc("outline_msg.revision.switch", &[("name", key.into())]))),
    }
}

fn policy_json(work: WorkId, policy: &RevisionPolicy) -> Value {
    let defaults = RevisionPolicy::default();
    json!({
        "work": work,
        "amend": policy.amend,
        "max_change_percent": policy.max_change_percent,
        "new_after_minutes": policy.new_after_minutes,
        "defaults": {
            "amend": defaults.amend,
            "max_change_percent": defaults.max_change_percent,
            "new_after_minutes": defaults.new_after_minutes,
        },
        "limits": {
            "max_change_percent": [MAX_CHANGE_PERCENT.0, MAX_CHANGE_PERCENT.1],
            "new_after_minutes": [NEW_AFTER_MINUTES.0, NEW_AFTER_MINUTES.1],
        },
    })
}

/// New setting from the parameters: missing fields stay as stored; limits according to
/// [`RevisionPolicy::validate`] (German message, -32602).
fn policy_from(params: &Value, stored: RevisionPolicy) -> Result<RevisionPolicy, RpcError> {
    let percent = match number(params, "max_change_percent")? {
        Some(p) => u8::try_from(p).map_err(|_| invalid(tc("outline_msg.revision.max_change", &[])))?,
        None => stored.max_change_percent,
    };
    let minutes = match number(params, "new_after_minutes")? {
        Some(m) => u32::try_from(m).map_err(|_| invalid(tc("outline_msg.revision.new_after", &[])))?,
        None => stored.new_after_minutes,
    };
    let policy = RevisionPolicy {
        amend: boolean(params, "amend")?.unwrap_or(stored.amend),
        max_change_percent: percent,
        new_after_minutes: minutes,
    };
    policy.validate().map_err(invalid)?;
    Ok(policy)
}

/// Answers a method from [`handles`].
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let store = &state.store;
    match method {
        "revision.amends" => {
            let params = resolve_keys(state, work, params).await?;
            let node = node_of(&params)?;
            let rev = required_u32(&params, "rev")?;
            let steps = store.amend_history(work, node, rev).await?;
            let key = store.node_key(work, node).await?;
            let amend_seq = steps.last().map_or(0, |s| s.seq);
            Ok(json!({ "node": node, "key": key, "rev": rev, "amend_seq": amend_seq, "steps": steps }))
        }
        "revision.amend_undo" => {
            let held = state.write_lock.lock().await;
            let params = resolve_keys(state, work, params).await?;
            let node = node_of(&params)?;
            let rev = required_u32(&params, "rev")?;
            let seq = required_u32(&params, "seq")?;
            let change = store.amend_undo(work, node, rev, seq, mirror::HUMAN_NAME).await?;
            let key = store.node_key(work, change.node).await?;
            let mut answer =
                json!({ "node": change.node, "key": key, "rev": change.rev, "amend_seq": change.amend_seq });
            add_mirror(&mut answer, mirror::record(state, &held, &change).await);
            Ok(answer)
        }
        "revision.policy_get" => Ok(policy_json(work, &store.revision_policy(work).await?)),
        "revision.policy_set" => {
            let held = state.write_lock.lock().await;
            let stored = store.revision_policy(work).await?;
            let policy = policy_from(&params, stored)?;
            let change = store.set_revision_policy(work, &policy, mirror::HUMAN_NAME).await?;
            let mut answer = json!({ "work": work, "changed": change.changed });
            if !change.changed.is_empty() {
                add_mirror(&mut answer, mirror::record_revision_policy(state, &held, &change).await);
            }
            Ok(answer)
        }
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

// ---- Pages -------------------------------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("revision_history.html", include_str!("../templates/revision_history.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Verlaufs fehlerhaft");
        }
        env
    })
}

fn plain(status: StatusCode, text: impl Into<String>) -> Response {
    (status, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text.into()).into_response()
}

/// Word difference as safe HTML (everything escaped): equal text plain, new in `<ins>`, removed in `<del>`.
pub(crate) fn diff_html(old: &str, new: &str) -> String {
    let mut out = String::from("<div class=\"amend-diff\">");
    for (kind, text) in word_diff(old, new) {
        let text = crate::bibles::html_escape(&text);
        match kind {
            DiffKind::Same => out.push_str(&format!("<span class=\"d-same\">{text}</span>")),
            DiffKind::Added => out.push_str(&format!("<ins class=\"d-add\">{text}</ins>")),
            DiffKind::Removed => out.push_str(&format!("<del class=\"d-del\">{text}</del>")),
        }
    }
    out.push_str("</div>");
    out
}

fn pair<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim()).filter(|v| !v.is_empty())
}

/// Data of the page „Verlauf“ of a revision.
async fn history_context(
    state: &AppState,
    work: WorkId,
    key: i64,
    rev: u32,
    notice: Option<&str>,
    error: Option<String>,
) -> Result<(String, Tpl), RpcError> {
    let store = &state.store;
    let node = store.node_seq(work, key).await?;
    let loaded = store.load_work(work).await?;
    let n = loaded.nodes.get(node).ok_or(StoreError::UnknownNode { work, node })?;
    let revision = n.revision(rev).ok_or_else(|| StoreError::UnknownRevision {
        node,
        rev,
        existing: n.revisions.iter().map(|r| r.number).collect(),
    })?;
    let steps = store.amend_history(work, node, rev).await?;
    let own_current =
        revision.status == RevisionStatus::Current && matches!(revision.author, baruch_core::Author::Human { .. });
    let rows: Vec<Tpl> = steps
        .iter()
        .rev()
        .map(|s| {
            context! {
                seq => s.seq,
                kind => match s.kind {
                    AmendKind::Amend => tc("outline_msg.revision.amend", &[]),
                    AmendKind::Undo => tc("outline_msg.revision.undo", &[]),
                },
                undo_of => s.undo_of,
                author => s.author.as_str(),
                created => crate::localtime::minutes(&s.created),
                diff => diff_html(&s.body_before, &s.body_after),
                // Going back to the state before this step is only possible if it differs from the present text.
                can_undo => own_current && s.body_before != revision.body,
            }
        })
        .collect();
    let label = if n.number.is_empty() { n.title.clone() } else { format!("{} {}", n.number, n.title) };
    let back = crate::view::node_url(work, key, Some(rev), false, "", "");
    let ctx = context! {
        work => context! { id => work, title => loaded.title.as_str() },
        node => context! { key => key, label => label.as_str() },
        rev => context! {
            number => rev,
            status => crate::mirror::revision_status_word(revision.status),
            amend_seq => steps.last().map_or(0, |s| s.seq),
            body => revision.body.as_str(),
            own_current => own_current,
        },
        steps => rows,
        back => back,
        notice => notice,
        error => error,
    };
    Ok((label, ctx))
}

fn render_history(headers: &HeaderMap, status: StatusCode, ctx: Tpl) -> Response {
    let settings = crate::view::Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let ctx = context! { settings => Tpl::from(Serde(settings)), ..ctx };
    match templates().get_template("revision_history.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            tc(
                "outline_msg.numbering.template_error",
                &[("name", "revision_history.html".into()), ("error", e.to_string().into())],
            ),
        ),
    }
}

fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        crate::rpc::REFUSED => StatusCode::CONFLICT,
        crate::rpc::INTERNAL => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::NOT_FOUND,
    }
}

/// `GET /werke/{id}/fassungen/verlauf?key=K&rev=R[&ok=1]`: history of the amendments of a revision, newest first,
/// per step the word difference and (only on the current revision of the human) „Bis hier zurück“.
async fn history_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (Some(key), Some(rev)) = (
        pair(&pairs, "key").and_then(|k| k.parse::<i64>().ok()),
        pair(&pairs, "rev").and_then(|r| r.parse::<u32>().ok()),
    ) else {
        return plain(StatusCode::BAD_REQUEST, tc("outline_msg.revision.key_rev_missing", &[]));
    };
    let notice = pair(&pairs, "ok").map(|_| tc("outline_msg.revision.undone", &[]));
    match history_context(&state, work, key, rev, notice.as_deref(), None).await {
        Ok((_, ctx)) => render_history(&headers, StatusCode::OK, ctx),
        Err(e) => plain(status_of(&e), e.message),
    }
}

/// `POST /werke/fassungen/act` (behind `guard::same_origin`): `method=revision.amend_undo`, `work`, `key`, `rev`,
/// `seq`. Success → 303 to the history with `ok=1`; rejected → the history with a message (409/422).
async fn act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    if pair(&pairs, "method") != Some("revision.amend_undo") {
        return plain(StatusCode::BAD_REQUEST, tc("outline_msg.page.unknown_action", &[]));
    }
    let work = match pair(&pairs, "work").and_then(|w| w.parse::<WorkId>().ok()) {
        Some(w) => w,
        None => match state.default_work().await {
            Ok(w) => w,
            Err(e) => return plain(StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        },
    };
    let (Some(key), Some(rev)) = (
        pair(&pairs, "key").and_then(|k| k.parse::<i64>().ok()),
        pair(&pairs, "rev").and_then(|r| r.parse::<u32>().ok()),
    ) else {
        return plain(StatusCode::UNPROCESSABLE_ENTITY, tc("outline_msg.revision.key_rev_missing", &[]));
    };
    let params = json!({ "work": work, "key": key, "rev": rev, "seq": pair(&pairs, "seq").unwrap_or("") });
    match call(&state, work, "revision.amend_undo", params).await {
        Ok(_) => Redirect::to(&format!("{}&ok=1", crate::view::history_url(work, key, rev))).into_response(),
        Err(e) => match history_context(&state, work, key, rev, None, Some(e.message.clone())).await {
            Ok((_, ctx)) => render_history(&headers, status_of(&e), ctx),
            Err(_) => plain(status_of(&e), e.message),
        },
    }
}

/// `POST /einstellungen/werk/fassungen` (behind `guard::same_origin`): `work`, `amend` (checkbox; missing = off),
/// `max_change_percent`, `new_after_minutes`. 303 to `/einstellungen/werk?work=W&ok=fassungen#fassungen` or
/// `…&fassungenfehler=1#fassungen` (nothing changed).
async fn policy_act(State(state): State<Arc<AppState>>, Form(pairs): Form<Vec<(String, String)>>) -> Response {
    let work = match pair(&pairs, "work").and_then(|w| w.parse::<WorkId>().ok()).filter(|&w| w > 0) {
        Some(w) => match state.store.work_slug(w).await {
            Ok(_) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
        None => match state.default_work().await {
            Ok(w) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
    };
    let failed = format!("/einstellungen/werk?work={work}&fassungenfehler=1#fassungen");
    if pair(&pairs, "method").is_some_and(|m| m != "revision.policy_set") {
        return Redirect::to(&failed).into_response();
    }
    let params = json!({
        "work": work,
        "amend": pair(&pairs, "amend").is_some(),
        "max_change_percent": pair(&pairs, "max_change_percent").unwrap_or(""),
        "new_after_minutes": pair(&pairs, "new_after_minutes").unwrap_or(""),
    });
    match call(&state, work, "revision.policy_set", params).await {
        Ok(_) => Redirect::to(&format!("/einstellungen/werk?work={work}&ok=fassungen#fassungen")).into_response(),
        Err(_) => Redirect::to(&failed).into_response(),
    }
}

/// Pages (GET): history of the amendments.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/fassungen/verlauf", get(history_page))
}

/// Writing forms (POST): undo through the history, setting „Fassungen“.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/fassungen/act", post(act)).route("/einstellungen/werk/fassungen", post(policy_act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_parameters_keep_missing_fields_and_check_the_limits() {
        let stored = RevisionPolicy::default();
        let p = policy_from(&json!({ "max_change_percent": "35" }), stored).unwrap();
        assert_eq!(p, RevisionPolicy { max_change_percent: 35, ..stored }, "fehlende Felder bleiben, Text als Zahl");
        let off = policy_from(&json!({ "amend": "0", "new_after_minutes": 0 }), stored).unwrap();
        assert_eq!((off.amend, off.new_after_minutes), (false, 0), "aus und keine Zeitgrenze");
        let e = policy_from(&json!({ "max_change_percent": 0 }), stored).unwrap_err();
        assert!(e.code == INVALID_PARAMS && e.message.contains("1 bis 100"), "0 % abgelehnt: {}", e.message);
        let e = policy_from(&json!({ "new_after_minutes": 5000 }), stored).unwrap_err();
        assert!(e.message.contains("0 bis 1440"), "über einen Tag abgelehnt: {}", e.message);
        assert!(policy_from(&json!({ "amend": "vielleicht" }), stored).is_err(), "Unbekanntes abgelehnt");
    }

    #[test]
    fn the_word_diff_is_escaped_html() {
        let html = diff_html("Die <Möwe> ruft.", "Die <Möwe> schreit.");
        assert!(html.contains("&lt;Möwe&gt;") && !html.contains("<Möwe>"), "maskiert: {html}");
        assert!(html.contains("<del class=\"d-del\">ruft.</del>"), "Entferntes markiert: {html}");
        assert!(html.contains("<ins class=\"d-add\">schreit.</ins>"), "Neues markiert: {html}");
    }
}
