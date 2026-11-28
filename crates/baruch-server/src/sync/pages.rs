// SPDX-License-Identifier: AGPL-3.0-or-later
//! Page `/abgleich` (usable without JavaScript) and its forms to `POST /abgleich/act` (behind
//! `guard::same_origin`): button „Abgleichen“, „zuletzt abgeglichen …“ with the numbers from `sync_run` and
//! the notes, per work the switch „nach MultiGPT hochladen“ with a mandatory checkbox, the selection
//! of the collections to fetch. The page itself never asks MultiGPT; only the buttons „Abgleichen“ and
//! „Sammlungen von MultiGPT laden“ do that.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::WorkId;
use minijinja::value::Serde;
use minijinja::{Environment, Value as Jinja, context};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::view::Settings;

/// Own template environment (that of the workspace page stays untouched).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("sync.html", include_str!("../../templates/sync.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Abgleichs fehlerhaft");
        }
        env.add_filter("when", |s: String| super::display_time(&s));
        env
    })
}

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    work: Option<String>,
    ok: Option<String>,
    fehler: Option<String>,
}

fn notice(code: Option<&str>) -> Option<String> {
    let key = match code? {
        "gestartet" => "sync_msg.page.started",
        "laeuft" => "sync_msg.page.running",
        "hochladen_an" => "sync_msg.page.upload_on",
        "hochladen_aus" => "sync_msg.page.upload_off",
        "sammlungen" => "sync_msg.page.collections",
        "geladen" => "sync_msg.page.loaded",
        _ => return None,
    };
    Some(crate::i18n::tc(key, &[]))
}

fn error_text(code: Option<&str>, load_error: Option<String>) -> Option<String> {
    Some(match code? {
        "bestaetigen" => crate::i18n::tc("sync_msg.page.confirm", &[]),
        "werk" => crate::i18n::tc("sync_msg.page.no_work", &[]),
        "sammlung" => crate::i18n::tc("sync_msg.page.collection", &[]),
        "laden" => {
            let detail = load_error.unwrap_or_else(|| crate::i18n::tc("sync_msg.page.no_answer", &[]));
            crate::i18n::tc("sync_msg.page.not_loaded", &[("detail", detail.into())])
        }
        _ => crate::i18n::tc("sync_msg.page.failed", &[]),
    })
}

fn work_of(text: Option<&str>) -> Option<WorkId> {
    text.and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|w| *w > 0)
}

fn internal(e: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, crate::i18n::tc("sync_msg.page.error", &[("detail", e.to_string().into())]))
        .into_response()
}

/// `GET /abgleich?work=W` – without network.
async fn page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(query): Query<PageQuery>) -> Response {
    let work = work_of(query.work.as_deref());
    let status = match super::status(&state, None).await {
        Ok(status) => status,
        Err(e) => return internal(e.message),
    };
    let chosen: Vec<String> = status["collections"]
        .as_array()
        .map(|list| list.iter().filter_map(|c| c["collection"].as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    let offered: Vec<Value> = super::offered(&state)
        .into_iter()
        .filter(|c| c["name"].as_str().is_some_and(|n| !chosen.iter().any(|x| x == n)))
        .collect();
    let last = status["last"].clone();
    let lines: Vec<String> = last["message"]
        .as_str()
        .map(|m| m.lines().map(str::to_owned).filter(|l| !l.trim().is_empty()).collect())
        .unwrap_or_default();
    let load_error = super::with_shared(&state.data, |s| s.load_error.clone());
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let works: Vec<Value> = status["works"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|mut w| {
            w["current"] = json!(work.is_some_and(|id| w["work"].as_i64() == Some(id)));
            w
        })
        .collect();
    let ctx = context! {
        settings => Jinja::from(Serde(settings)),
        work => work.map(|id| context! { id => id }),
        selected => work,
        configured => status["configured"].as_bool().unwrap_or(false),
        running => status["running"].as_bool().unwrap_or(false),
        last => Jinja::from(Serde(last)),
        summary => lines.first().cloned(),
        hints => lines.iter().skip(1).cloned().collect::<Vec<_>>(),
        works => Jinja::from(Serde(works)),
        collections => Jinja::from(Serde(status["collections"].clone())),
        offered => Jinja::from(Serde(offered)),
        notice => notice(query.ok.as_deref()),
        error => error_text(query.fehler.as_deref(), load_error),
    };
    match templates().get_template("sync.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => internal(format!("Vorlage sync.html: {e}")),
    }
}

/// Return: `back` only to `/abgleich…` or `/einstellungen/werk…`, otherwise `/abgleich[?work=W]`; plus
/// a fixed code.
fn back(back: Option<&str>, work: Option<WorkId>, code: (&str, &str)) -> Response {
    let safe = back.map(str::trim).filter(|b| {
        (b.starts_with("/abgleich") || b.starts_with("/einstellungen/werk"))
            && !b.chars().any(|c| c.is_control() || matches!(c, ' ' | '"' | '<' | '>' | '\\' | '#'))
    });
    let base = match (safe, code.0) {
        // An error always shows the page of the reconciliation (it says there what is to be done).
        (Some(b), "ok") => b.to_owned(),
        _ => match work {
            Some(w) => format!("/abgleich?work={w}"),
            None => "/abgleich".to_owned(),
        },
    };
    let join = if base.contains('?') { '&' } else { '?' };
    Redirect::to(&format!("{base}{join}{}={}", code.0, code.1)).into_response()
}

fn field<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

fn checked(pairs: &[(String, String)], name: &str) -> bool {
    matches!(field(pairs, name), Some("1" | "on" | "true"))
}

/// `POST /abgleich/act`: `action` ∈ `run`, `upload` (`work`, `on=1` or missing = off, `confirm=1`),
/// `collections` (repeated `collection`, plus `collection_new`), `load_collections`. Always 303.
async fn act(State(state): State<Arc<AppState>>, Form(pairs): Form<Vec<(String, String)>>) -> Response {
    let work = work_of(field(&pairs, "work"));
    let target = field(&pairs, "back");
    match field(&pairs, "action").unwrap_or("") {
        "run" => match super::start(&state).await {
            Ok(answer) if answer["started"] == true => back(target, work, ("ok", "gestartet")),
            Ok(_) => back(target, work, ("ok", "laeuft")),
            Err(e) => internal(e.message),
        },
        "upload" => {
            let Some(id) = work else { return back(None, None, ("fehler", "werk")) };
            let on = checked(&pairs, "on");
            match super::set_upload(&state, id, on, checked(&pairs, "confirm")).await {
                Ok(_) => back(target, work, ("ok", if on { "hochladen_an" } else { "hochladen_aus" })),
                Err(e) if e.code == crate::rpc::INVALID_PARAMS && on => back(None, work, ("fehler", "bestaetigen")),
                Err(e) if e.code == crate::rpc::INVALID_PARAMS => back(None, None, ("fehler", "werk")),
                Err(e) => internal(e.message),
            }
        }
        "collections" => {
            let mut list: Vec<String> =
                pairs.iter().filter(|(n, _)| n == "collection").map(|(_, v)| v.clone()).collect();
            if let Some(new) = field(&pairs, "collection_new").filter(|n| !n.trim().is_empty()) {
                list.push(new.to_owned());
            }
            match super::set_collections(&state, &list).await {
                Ok(_) => back(target, work, ("ok", "sammlungen")),
                Err(e) if e.code == crate::rpc::INVALID_PARAMS || e.code == crate::rpc::REFUSED => {
                    back(None, work, ("fehler", "sammlung"))
                }
                Err(e) => internal(e.message),
            }
        }
        "load_collections" => match super::offered_collections(&state).await {
            Ok(_) => {
                super::with_shared(&state.data, |s| s.load_error = None);
                back(target, work, ("ok", "geladen"))
            }
            Err(e) => {
                super::with_shared(&state.data, |s| s.load_error = Some(e.message.clone()));
                back(None, work, ("fehler", "laden"))
            }
        },
        _ => (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response(),
    }
}

/// `GET /abgleich`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/abgleich", get(page))
}

/// `POST /abgleich/act` (in the router of the writing routes, that is behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/abgleich/act", post(act))
}
