// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pages of the web search, usable without JavaScript:
//!
//! - `GET /websuche/belege?work&key&post[&ok|fehler]` – the evidence of a post (title, address as a link,
//!   retrieved on, quotation, verdict, „bevorzugte Quelle“) with the button „In die Bibliothek“ for `belegt`.
//! - `POST /websuche/act` (behind `guard::same_origin`): `action=switch` (`on=1`, missing = off) → 303 to
//!   Einstellungen → Assistenten (tab `@belege`); `action=adopt` (`evidence`) → 303 back to the evidence page
//!   with `ok=uebernommen` or `fehler=<code>` (messages only from fixed codes, never from the input).

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::Deserialize;

use super::evidence::{self, Evidence};
use super::verify::Verdict;
use super::{day_of, set_enabled};
use crate::AppState;
use crate::rpc::INVALID_PARAMS;
use crate::view::Settings;

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("websearch_evidence.html", include_str!("../../templates/websearch_evidence.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Websuche fehlerhaft");
        }
        env
    })
}

fn settings_of(headers: &HeaderMap) -> Value {
    Value::from(Serde(Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))))
}

#[derive(Debug, Default, Deserialize)]
struct EvidenceQuery {
    work: Option<String>,
    key: Option<String>,
    post: Option<String>,
    ok: Option<String>,
    fehler: Option<String>,
}

fn number(text: Option<&str>) -> Option<i64> {
    text.map(str::trim).filter(|t| !t.is_empty()).and_then(|t| t.parse().ok())
}

fn notice(code: Option<&str>) -> Option<String> {
    match code {
        Some("uebernommen") => Some(crate::i18n::tc("websearch_msg.page.adopted", &[])),
        _ => None,
    }
}

fn error_text(code: Option<&str>) -> Option<String> {
    code.map(|c| {
        let key = match c {
            "nicht_belegt" => "websearch_msg.page.not_proven",
            "schon" => "websearch_msg.page.already",
            "unbekannt" => "websearch_msg.page.unknown",
            "aktion" => "websearch_msg.page.unknown_action",
            _ => "websearch_msg.page.failed",
        };
        crate::i18n::tc(key, &[])
    })
}

fn error_response(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        crate::i18n::tc("websearch_msg.page.error", &[("detail", e.to_string().into())]),
    )
        .into_response()
}

fn view(row: &Evidence, work: Option<i64>) -> Value {
    let library = row.library_title.map(|t| match work {
        Some(w) => format!("/quelle?work={w}&title={t}"),
        None => format!("/quelle?title={t}"),
    });
    context! {
        id => row.id, label => row.label.as_str(), url => row.url.as_str(),
        title => if row.title.trim().is_empty() { row.url.as_str() } else { row.title.as_str() },
        preferred => row.preferred, accessed => day_of(&row.accessed), quote => row.quote.as_str(),
        verdict => row.verdict.as_str(), verdict_label => Verdict::label(&row.verdict),
        library_title => row.library_title, library_href => library,
        can_adopt => row.verdict == "belegt" && row.library_title.is_none() && !row.quote.trim().is_empty(),
    }
}

/// `GET /websuche/belege`.
async fn evidence_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EvidenceQuery>,
) -> Response {
    let Some(post) = number(query.post.as_deref()) else {
        return (StatusCode::BAD_REQUEST, crate::i18n::tc("websearch_msg.page.post_missing", &[])).into_response();
    };
    let rows = match evidence::for_post(state.store.pool(), post).await {
        Ok(rows) => rows,
        Err(e) => return error_response(e),
    };
    let work = number(query.work.as_deref()).or_else(|| rows.first().and_then(|r| r.work));
    let key = number(query.key.as_deref()).or_else(|| rows.first().and_then(|r| r.key));
    let post_info = match work {
        Some(w) => state.store.post_get(w, post).await.ok(),
        None => None,
    };
    let back = match (work, key) {
        (Some(w), Some(k)) => Some(crate::view::node_url(w, k, None, false, "", "")),
        (Some(w), None) => Some(format!("/?work={w}")),
        _ => None,
    };
    let items: Vec<Value> = rows.iter().map(|r| view(r, work)).collect();
    let ctx = context! {
        settings => settings_of(&headers),
        work => work.map(|w| context! { id => w }),
        key => key, post => post,
        post_author => post_info.as_ref().map(|p| p.author.clone()),
        post_body => post_info.as_ref().map(|p| p.body.clone()),
        back => back, evidence => items,
        notice => notice(query.ok.as_deref()), error => error_text(query.fehler.as_deref()),
    };
    match templates().get_template("websearch_evidence.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            crate::i18n::tc("websearch_msg.page.template_error", &[("detail", e.to_string().into())]),
        )
            .into_response(),
    }
}

#[derive(Debug, Default, Deserialize)]
struct ActForm {
    action: String,
    on: Option<String>,
    evidence: Option<String>,
    work: Option<String>,
    key: Option<String>,
    post: Option<String>,
}

fn evidence_url(work: Option<i64>, key: Option<i64>, post: Option<i64>, code: &str) -> String {
    let mut url = String::from("/websuche/belege?");
    let mut parts = Vec::new();
    if let Some(w) = work {
        parts.push(format!("work={w}"));
    }
    if let Some(k) = key {
        parts.push(format!("key={k}"));
    }
    if let Some(p) = post {
        parts.push(format!("post={p}"));
    }
    parts.push(code.to_owned());
    url.push_str(&parts.join("&"));
    url
}

/// `POST /websuche/act`.
async fn act(State(state): State<Arc<AppState>>, Form(form): Form<ActForm>) -> Response {
    let work = number(form.work.as_deref());
    match form.action.as_str() {
        "switch" => {
            let on = matches!(form.on.as_deref(), Some("1" | "on" | "true"));
            if let Err(e) = set_enabled(&state, on).await {
                return error_response(e);
            }
            let work_q = work.map(|w| format!("&work={w}")).unwrap_or_default();
            Redirect::to(&format!("/einstellungen/assistenten?persona=belege&ok=websuche{work_q}#persona-belege"))
                .into_response()
        }
        "adopt" => {
            let Some(id) = number(form.evidence.as_deref()) else {
                let url =
                    evidence_url(work, number(form.key.as_deref()), number(form.post.as_deref()), "fehler=unbekannt");
                return Redirect::to(&url).into_response();
            };
            let row = evidence::get(state.store.pool(), id).await.ok().flatten();
            let work = work.or_else(|| row.as_ref().and_then(|r| r.work));
            let key = number(form.key.as_deref()).or_else(|| row.as_ref().and_then(|r| r.key));
            let post = number(form.post.as_deref()).or_else(|| row.as_ref().and_then(|r| r.post));
            let code = match evidence::adopt(&state, id).await {
                Ok(_) => "ok=uebernommen".to_owned(),
                Err(e) if e.code == INVALID_PARAMS => match &row {
                    None => "fehler=unbekannt".to_owned(),
                    Some(r) if r.library_title.is_some() => "fehler=schon".to_owned(),
                    Some(_) => "fehler=nicht_belegt".to_owned(),
                },
                Err(e) => {
                    eprintln!("Websuche: Übernahme von Beleg {id} gescheitert: {}", e.message);
                    "fehler=intern".to_owned()
                }
            };
            let url = format!("{}#beleg-{id}", evidence_url(work, key, post, &code));
            Redirect::to(&url).into_response()
        }
        _ => {
            let url = evidence_url(work, number(form.key.as_deref()), number(form.post.as_deref()), "fehler=aktion");
            Redirect::to(&url).into_response()
        }
    }
}

/// `GET /websuche/belege`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/websuche/belege", get(evidence_page))
}

/// `POST /websuche/act` (in the router of the writing routes, that is behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/websuche/act", post(act))
}
