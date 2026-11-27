// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings page „MCP-Zugang“: address `http://<Host>/mcp` for copying, access without a token only through
//! loopback, the clients seen since the start ([`crate::mcp::session::clients`]) and the tokens for
//! other machines (Round 10, SQLite: only name, beginning and SHA-256 hash, [`crate::mcp::tokens`]).
//!
//! Forms without JavaScript to `POST /einstellungen/mcp/act` (behind `guard::same_origin`):
//! - `action=create`, `name` → **200** with the whole page, which shows the plain text exactly once
//!   (`Cache-Control: no-store`); never in address, redirect, cookie or message. Rejected → 422 – since Round 23
//!   also if the name would result in the same author as an existing or revoked token or would begin with
//!   `lokal-` ([`crate::mcp::tokens::create`], the same path as RPC `mcp_token.create`).
//! - `action=revoke`, `id`, checkbox `confirm` (the server checks it) → 303 `?ok=widerrufen`.
//!
//! `act` hands the freshly created plain text through its own `tokio::task_local` on to [`render`];
//! outside of this one answer it no longer exists.
//!
//! Round 24: if the page is called under an additionally allowed host name (`--allow-host`, operation behind
//! a reverse proxy, [`crate::guard::AllowedHosts::is_extra`]), it shows the address as `https://<Name>/mcp`
//! (TLS ends in the proxy; Baruch does not know the scheme and assumes `https`) and the command with a token – under
//! this name `/mcp` accepts no request without a token.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Form;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use baruch_store::WorkId;
use minijinja::{Value, context};
use serde::Deserialize;
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::mcp::session;

/// A token just created: name and plain text, only for the answer to the creation.
#[derive(Clone)]
struct Fresh {
    name: String,
    secret: String,
}

tokio::task_local! {
    static FRESH: Fresh;
}

/// Unix seconds → `JJJJ-MM-TT HH:MM:SS` in local time.
pub(crate) fn local_time(secs: u64) -> String {
    crate::localtime::unix_seconds(secs)
}

/// `2026-11-06T12:05:06.123Z` (UTC) → `2026-11-06 13:05:06` in local time.
fn short_time(iso: &str) -> String {
    crate::localtime::seconds(iso)
}

pub(super) async fn render(
    state: &AppState,
    work: Option<WorkId>,
    query: &BTreeMap<String, String>,
) -> Result<String, String> {
    let info = super::request();
    // Round 24: under an additional host name (through the proxy) https and only with a token.
    let proxied = info.host.as_deref().is_some_and(|host| state.allowed_hosts.is_extra(host));
    let scheme = if proxied { "https" } else { "http" };
    let address = info.host.as_deref().map(|host| format!("{scheme}://{host}/mcp"));
    let clients: Vec<Value> = session::clients()
        .into_iter()
        .map(|c| {
            context! {
                name => c.name, version => c.version, handle => c.handle,
                first_seen => local_time(c.first_seen), last_seen => local_time(c.last_seen),
            }
        })
        .collect();
    let listed = state.store.mcp_token_list().await.map_err(|e| e.to_string())?;
    let names: Vec<String> = listed.iter().map(|t| t.name.clone()).collect();
    let tokens: Vec<Value> = listed
        .into_iter()
        .map(|t| {
            let handle = session::token_handle(&t.name, names.iter().map(String::as_str));
            // Round 23: legacy handle (rule up to Round 22) if it differs – old posts carry it.
            let legacy = format!("@mcp:{}", session::legacy_handle_name(&t.name));
            let legacy = (legacy != handle).then_some(legacy);
            // The token keeps its legacy handle because the new one belongs to another token (`session::keeps_legacy`).
            let taken = session::keeps_legacy(&t.name, names.iter().map(String::as_str))
                .then(|| format!("@mcp:{}", session::handle_name(&t.name)));
            context! {
                id => t.id, name => t.name, prefix => t.prefix, handle => handle, legacy => legacy, taken => taken,
                created => short_time(&t.created),
                last_used => t.last_used.as_deref().map(short_time),
                revoked => t.revoked.as_deref().map(short_time),
            }
        })
        .collect();
    let fresh = FRESH.try_with(Clone::clone).ok();
    let ok = query.get("ok").map(String::as_str);
    super::render_template(
        "settings/mcp.html",
        context! {
            address => address, proxied => proxied, clients => clients, tokens => tokens, work => work,
            error => info.error, revoked => ok == Some("widerrufen"),
            fresh => fresh.map(|f| {
                let handle = session::Client { name: f.name.clone(), ..Default::default() }.handle();
                context! { name => f.name, secret => f.secret, handle => handle }
            }),
        },
    )
}

/// `settings.get {page: "mcp"}`: path, access without a token only through loopback, clients seen, tokens
/// (without hash and plain text).
pub(super) async fn values(state: &AppState) -> Result<Json, crate::rpc::RpcError> {
    let tokens = state.store.mcp_token_list().await?;
    Ok(json!({
        "path": "/mcp",
        "loopback_without_token": true,
        "clients": session::clients(),
        "tokens": tokens,
    }))
}

/// Form of the page (see the module documentation).
#[derive(Debug, Deserialize)]
pub(super) struct ActForm {
    action: String,
    name: Option<String>,
    id: Option<String>,
    confirm: Option<String>,
    work: Option<String>,
}

/// `POST /einstellungen/mcp/act`.
pub(super) async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work = form.work.as_deref().and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|&w| w > 0);
    match form.action.as_str() {
        "create" => {
            let name = form.name.as_deref().unwrap_or_default();
            let (token, secret) = match crate::mcp::tokens::create(&state.store, name).await {
                Ok(made) => made,
                Err(e) => return super::error_page(&state, &headers, "mcp", work, &e.to_string()).await,
            };
            let fresh = Fresh { name: token.name, secret };
            let info = super::request_info(&headers, None);
            let framed = FRESH.scope(fresh, super::frame(&state, "mcp", work, BTreeMap::new(), info)).await;
            let mut response = match framed {
                Ok(framed) => super::full_page(&state, &headers, StatusCode::OK, framed).await,
                Err(e) => super::frame_error(e),
            };
            response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        "revoke" => {
            if !form.confirm.as_deref().is_some_and(|c| matches!(c, "1" | "on")) {
                let message = "Widerrufen bitte bestätigen (Kästchen ankreuzen).";
                return super::error_page(&state, &headers, "mcp", work, message).await;
            }
            let Some(id) = form.id.as_deref().and_then(|i| i.trim().parse::<i64>().ok()) else {
                return super::error_page(&state, &headers, "mcp", work, "Welcher Token? (id fehlt)").await;
            };
            match state.store.mcp_token_revoke(id).await {
                Ok(_) => Redirect::to(&format!("/einstellungen/mcp?ok=widerrufen{}", super::work_suffix(work)))
                    .into_response(),
                Err(e) => super::error_page(&state, &headers, "mcp", work, &e.to_string()).await,
            }
        }
        _ => (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use crate::localtime;

    #[test]
    fn unix_seconds_become_local_time() {
        assert_eq!(
            super::local_time(1_793_966_706),
            localtime::unix_seconds(1_793_966_706),
            "MCP-Clients zeigen „zuerst/zuletzt gesehen“ in Ortszeit (Umrechnung: localtime::tests)"
        );
    }

    #[test]
    fn stored_times_are_shown_without_fraction() {
        assert_eq!(
            super::short_time("2026-11-06T12:05:06.123Z"),
            localtime::seconds("2026-11-06T12:05:06.123Z"),
            "Tokens: Ortszeit, ohne Bruchteile"
        );
        assert_eq!(super::short_time("2026-11-06T12:05:06.123Z").len(), 19, "JJJJ-MM-TT HH:MM:SS ohne Bruchteile");
        assert_eq!(super::short_time("kurz"), "kurz", "unbekannte Form bleibt");
    }
}
