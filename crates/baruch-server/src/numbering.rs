// SPDX-License-Identifier: AGPL-3.0-or-later
//! Outline schemes in the work settings (contract Round 6, section 2e): RPC
//! `numbering.*`, the page `/werke/{id}/nummerierung` and its form to
//! `POST /werke/nummerierung/act`, all usable without JavaScript.
//!
//! RPC ([`handles`], [`call`]; called from `rpc::call`, all take `work?`):
//! - `numbering.schemes` → `{schemes: [{id, label, example, preview: [NumberPreview] (the first
//!   12 items), error?}], current: NumberingInfo}`
//! - `numbering.preview {scheme, custom?}` → `{numbers: [NumberPreview], rule: string|null}` (`rule`
//!   = the rule that the export puts after the work template; `null` for „aus der Werkvorlage“)
//! - `numbering.set {scheme, custom?}` → outline answer like `outline.set_numbering`
//!   (`{node, action: "numbering", title, detail, revisions, renumbered, commit?, mirror?}`)
//!
//! `scheme` is an id from `baruch_store::NumberingScheme::ALL`, `custom` the own pattern in the
//! pattern language (only for `custom`; if it is missing, the stored one applies). Unknown scheme or
//! invalid pattern → -32602, unchanged → -32000. `numbering.set` runs under
//! [`AppState::write_lock`] and yields exactly one commit in the mirror (trailer
//! `Baruch-Action: outline-numbering`, [`mirror::record_outline`]). MCP and assistants reach
//! none of it; only the human chooses the scheme.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::{NumberPreview, NumberingInfo, NumberingScheme, StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::Settings;
use crate::{AppState, mirror};

const METHODS: &[&str] = &["numbering.schemes", "numbering.preview", "numbering.set"];

/// How many items the preview shows per scheme.
pub const PREVIEW_POINTS: usize = 12;

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// `scheme` (mandatory) and `custom` (text or missing/null) from the parameters.
fn scheme_params(params: &Json) -> Result<(String, Option<String>), RpcError> {
    let scheme = match params.get("scheme") {
        Some(Json::String(s)) if !s.trim().is_empty() => s.trim().to_owned(),
        _ => return Err(invalid("scheme fehlt (id eines Gliederungsschemas, z. B. „decimal“)")),
    };
    let custom = match params.get("custom") {
        None | Some(Json::Null) => None,
        Some(Json::String(s)) => Some(s.clone()),
        Some(_) => return Err(invalid("custom: Text oder null erwartet")),
    };
    Ok((scheme, custom))
}

/// A scheme with an example and a preview on the own outline.
#[derive(Debug, Clone, Serialize)]
pub struct SchemeRow {
    pub id: &'static str,
    pub label: &'static str,
    pub example: &'static str,
    /// The first [`PREVIEW_POINTS`] items of the outline.
    pub preview: Vec<NumberPreview>,
    /// Further items after the preview.
    pub more: usize,
    /// Why there is no preview (e.g. own pattern is missing or invalid).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// All schemes with a preview; for „Eigenes Muster“ `custom` replaces the stored pattern.
pub async fn scheme_rows(state: &AppState, work: WorkId, custom: Option<&str>) -> Result<Vec<SchemeRow>, RpcError> {
    let mut rows = Vec::with_capacity(NumberingScheme::ALL.len());
    for scheme in NumberingScheme::ALL {
        let custom = if scheme == NumberingScheme::Custom { custom } else { None };
        let (preview, more, error) = match state.store.numbering_preview(work, scheme.id(), custom).await {
            Ok(mut all) => {
                let more = all.len().saturating_sub(PREVIEW_POINTS);
                all.truncate(PREVIEW_POINTS);
                (all, more, None)
            }
            Err(StoreError::Numbering(message)) => (Vec::new(), 0, Some(message)),
            Err(e) => return Err(e.into()),
        };
        rows.push(SchemeRow {
            id: scheme.id(),
            label: scheme.label(),
            example: scheme.example(),
            preview,
            more,
            error,
        });
    }
    Ok(rows)
}

/// Set the scheme (under the write lock) and log it in the mirror; answer like
/// `outline.set_numbering`.
async fn set(state: &AppState, work: WorkId, scheme: &str, custom: Option<&str>) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.set_numbering_scheme(work, scheme, custom).await?;
    let mut answer = crate::outline::outline_answer(&change);
    match mirror::record_outline(state, &held, &change).await {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
    Ok(answer)
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params: Json) -> Result<Json, RpcError> {
    match method {
        "numbering.schemes" => {
            let schemes = scheme_rows(state, work, None).await?;
            let current = state.store.outline_numbering(work).await?;
            Ok(json!({ "schemes": schemes, "current": current }))
        }
        "numbering.preview" => {
            let (scheme, custom) = scheme_params(&params)?;
            let numbers = state.store.numbering_preview(work, &scheme, custom.as_deref()).await?;
            let rule = state.store.numbering_rule(work, &scheme, custom.as_deref()).await?;
            Ok(json!({ "numbers": numbers, "rule": rule }))
        }
        "numbering.set" => {
            let (scheme, custom) = scheme_params(&params)?;
            set(state, work, &scheme, custom.as_deref()).await
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

// ---- Page ----------------------------------------------------------------------------------------

/// Own template environment of the page (that of the work page stays untouched).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("work_numbering.html", include_str!("../templates/work_numbering.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Nummerierung fehlerhaft");
        }
        env
    })
}

fn render(status: StatusCode, ctx: Value) -> Response {
    match templates().get_template("work_numbering.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage work_numbering.html: {e}")).into_response()
        }
    }
}

fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(e: RpcError) -> Response {
    if e.code == INVALID_PARAMS && e.message.starts_with("unbekanntes Werk") {
        return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response();
    }
    (status_of(&e), format!("Fehler: {}", e.message)).into_response()
}

/// Address and values of the page: `schema`/`scheme` preselects the scheme, `muster`/`custom` the own
/// pattern (this is how the fields of the form also go through „Vorschau“), `ok` reports the accept.
#[derive(Deserialize, Default)]
struct PageQuery {
    schema: Option<String>,
    scheme: Option<String>,
    muster: Option<String>,
    custom: Option<String>,
    ok: Option<String>,
}

fn clean(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

/// What the page shows: chosen scheme, own pattern, notice or error.
#[derive(Debug, Clone, Default)]
pub(crate) struct Shown {
    pub(crate) scheme: Option<String>,
    pub(crate) custom: Option<String>,
    pub(crate) notice: Option<String>,
    pub(crate) error: Option<String>,
}

impl Shown {
    /// Preselection from the address (`schema`/`scheme`, `muster`/`custom`), without notice and error.
    pub(crate) fn from_query(
        schema: Option<&str>,
        scheme: Option<&str>,
        muster: Option<&str>,
        custom: Option<&str>,
    ) -> Shown {
        Shown {
            scheme: clean(schema).or_else(|| clean(scheme)),
            custom: clean(muster).or_else(|| clean(custom)),
            notice: None,
            error: None,
        }
    }
}

/// Context of the form part of `work_numbering.html` (`{% block content %}`): work, scheme in force,
/// all schemes with a preview, own pattern, rule, notice, error and `back` (return into the
/// settings, Round 9). The same function builds `/werke/{id}/nummerierung` and the
/// settings page „Werk“ (`crate::settings`).
pub(crate) async fn page_context(
    state: &AppState,
    work: WorkId,
    shown: Shown,
    back: Option<&str>,
) -> Result<Value, RpcError> {
    let title = state.store.load_work(work).await?.title;
    let current: NumberingInfo = state.store.outline_numbering(work).await?;
    let selected = shown
        .scheme
        .as_deref()
        .and_then(NumberingScheme::from_id)
        .map_or(current.scheme.clone(), |s| s.id().to_owned());
    let custom = shown.custom.clone().or_else(|| current.custom.clone()).unwrap_or_default();
    let typed = shown.custom.as_deref().filter(|c| !c.trim().is_empty());
    let rows = scheme_rows(state, work, typed).await?;
    let rule = match state.store.numbering_rule(work, &selected, typed).await {
        Ok(rule) => rule,
        Err(StoreError::Numbering(_)) => None,
        Err(e) => return Err(e.into()),
    };
    let schemes: Vec<Value> = rows
        .iter()
        .map(|row| {
            context! {
                id => row.id, label => row.label, example => row.example, more => row.more,
                error => row.error.as_deref(), preview => Value::from(Serde(&row.preview)),
                checked => row.id == selected, current => row.id == current.scheme,
            }
        })
        .collect();
    Ok(context! {
        work => context! { id => work, title => title },
        current => Value::from(Serde(&current)),
        schemes => schemes,
        custom => custom,
        rule => rule,
        notice => shown.notice,
        error => shown.error,
        preview_points => PREVIEW_POINTS,
        back => back,
    })
}

async fn page(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    status: StatusCode,
    shown: Shown,
    back: Option<&str>,
) -> Response {
    let ctx = match page_context(state, work, shown, back).await {
        Ok(ctx) => ctx,
        Err(e) => return error_response(e),
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    render(status, context! { settings => Value::from(Serde(settings)), ..ctx })
}

/// `GET /werke/{id}/nummerierung[?schema=ID&muster=SPEC]`: all schemes with an example and a preview.
async fn numbering_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(q): Query<PageQuery>,
) -> Response {
    let mut shown =
        Shown::from_query(q.schema.as_deref(), q.scheme.as_deref(), q.muster.as_deref(), q.custom.as_deref());
    shown.notice = q.ok.is_some().then(|| "Das Gliederungsschema ist übernommen.".to_owned());
    page(&state, &headers, id, StatusCode::OK, shown, None).await
}

#[derive(Deserialize, Default)]
struct ActForm {
    work: Option<String>,
    scheme: Option<String>,
    custom: Option<String>,
    /// Return after success (only `/einstellungen/werk…`, otherwise skipped; Round 9).
    back: Option<String>,
}

/// `POST /werke/nummerierung/act` (fields `work`, `scheme`, `custom`): accept the scheme, then 303
/// back to the page with a notice; error 422 (invalid) or 409 (unchanged) with the page.
async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work: WorkId = match form.work.as_deref().map(str::trim).and_then(|w| w.parse().ok()) {
        Some(work) => work,
        None => return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: work fehlt (Werk-id)").into_response(),
    };
    if let Err(e) = state.store.work_slug(work).await {
        return error_response(e.into());
    }
    let scheme = clean(form.scheme.as_deref());
    let custom = clean(form.custom.as_deref());
    let result = match scheme.as_deref() {
        Some(scheme) => set(&state, work, scheme, custom.as_deref()).await,
        None => Err(invalid("Bitte ein Gliederungsschema wählen.")),
    };
    let back = crate::works::settings_back(form.back.as_deref());
    match result {
        Ok(_) => match &back {
            Some(back) => Redirect::to(&crate::works::with_ok(back, "schema")).into_response(),
            None => Redirect::to(&format!("/werke/{work}/nummerierung?ok=1")).into_response(),
        },
        Err(e) if e.code == INTERNAL => error_response(e),
        Err(e) => {
            let status = status_of(&e);
            let shown = Shown { scheme, custom, notice: None, error: Some(e.message) };
            if back.is_some() {
                // From the settings: the same settings page with error and input (the popup
                // takes the frame from it).
                let retry = crate::settings::WorkRetry::Numbering(shown);
                return crate::settings::work_retry_page(&state, &headers, status, work, retry).await;
            }
            page(&state, &headers, work, status, shown, None).await
        }
    }
}

/// `GET /werke/{id}/nummerierung`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/nummerierung", get(numbering_page))
}

/// `POST /werke/nummerierung/act` (in the router of the writing routes, so behind `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/nummerierung/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_and_custom_are_read_from_the_parameters() {
        let (scheme, custom) = scheme_params(&json!({ "scheme": " legal ", "custom": null })).unwrap();
        assert_eq!((scheme.as_str(), custom), ("legal", None), "id getrimmt, custom null = keins");
        assert!(scheme_params(&json!({ "custom": "1.1" })).is_err(), "ohne scheme ein Fehler");
        assert!(scheme_params(&json!({ "scheme": "custom", "custom": 3 })).is_err(), "custom muss Text sein");
        assert!(handles("numbering.set") && !handles("outline.set_numbering"), "nur numbering.*");
    }
}
