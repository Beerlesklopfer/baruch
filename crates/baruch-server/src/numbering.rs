// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederungsschemata in den Werk-Einstellungen (Vertrag Runde 6, Abschnitt 2e): RPC
//! `numbering.*`, die Seite `/werke/{id}/nummerierung` und ihr Formular an
//! `POST /werke/nummerierung/act`, alles ohne JavaScript bedienbar.
//!
//! RPC ([`handles`], [`call`]; aus `rpc::call` heraus, alle nehmen `work?`):
//! - `numbering.schemes` → `{schemes: [{id, label, example, preview: [NumberPreview] (die ersten
//!   12 Punkte), error?}], current: NumberingInfo}`
//! - `numbering.preview {scheme, custom?}` → `{numbers: [NumberPreview], rule: string|null}` (`rule`
//!   = die Regel, die der Export hinter die Werkvorlage setzt; `null` bei „aus der Werkvorlage“)
//! - `numbering.set {scheme, custom?}` → Gliederungsantwort wie `outline.set_numbering`
//!   (`{node, action: "numbering", title, detail, revisions, renumbered, commit?, mirror?}`)
//!
//! `scheme` ist eine id aus `baruch_store::NumberingScheme::ALL`, `custom` das eigene Muster in der
//! Mustersprache (nur bei `custom`; fehlt es, gilt das gespeicherte). Unbekanntes Schema oder
//! ungültiges Muster → -32602, unverändert → -32000. `numbering.set` läuft unter
//! [`AppState::write_lock`] und ergibt genau einen Commit im Spiegel (Trailer
//! `Baruch-Action: outline-numbering`, [`mirror::record_outline`]). MCP und Assistenten erreichen
//! nichts davon; das Schema wählt nur der Mensch.

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

/// Wie viele Punkte die Vorschau je Schema zeigt.
pub const PREVIEW_POINTS: usize = 12;

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// `scheme` (Pflicht) und `custom` (Text oder fehlend/null) aus den Parametern.
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

/// Ein Schema mit Beispiel und Vorschau an der eigenen Gliederung.
#[derive(Debug, Clone, Serialize)]
pub struct SchemeRow {
    pub id: &'static str,
    pub label: &'static str,
    pub example: &'static str,
    /// Die ersten [`PREVIEW_POINTS`] Punkte der Gliederung.
    pub preview: Vec<NumberPreview>,
    /// Weitere Punkte hinter der Vorschau.
    pub more: usize,
    /// Warum es keine Vorschau gibt (z. B. eigenes Muster fehlt oder ist ungültig).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Alle Schemata mit Vorschau; `custom` ersetzt für „Eigenes Muster“ das gespeicherte Muster.
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

/// Schema setzen (unter der Schreibsperre) und im Spiegel protokollieren; Antwort wie
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
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

// ---- Seite ---------------------------------------------------------------------------------------

/// Eigene Template-Umgebung der Seite (die der Arbeitsseite bleibt unberührt).
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

/// Adresse und Werte der Seite: `schema`/`scheme` wählt das Schema vor, `muster`/`custom` das eigene
/// Muster (so gehen auch die Felder des Formulars über „Vorschau“), `ok` meldet die Übernahme.
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

/// Was die Seite zeigt: gewähltes Schema, eigenes Muster, Hinweis oder Fehler.
#[derive(Debug, Clone, Default)]
pub(crate) struct Shown {
    pub(crate) scheme: Option<String>,
    pub(crate) custom: Option<String>,
    pub(crate) notice: Option<String>,
    pub(crate) error: Option<String>,
}

impl Shown {
    /// Vorwahl aus der Adresse (`schema`/`scheme`, `muster`/`custom`), ohne Hinweis und Fehler.
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

/// Kontext des Formularteils von `work_numbering.html` (`{% block content %}`): Werk, geltendes Schema,
/// alle Schemata mit Vorschau, eigenes Muster, Regel, Hinweis, Fehler und `back` (Rücksprung in die
/// Einstellungen, Runde 9). Dieselbe Funktion baut `/werke/{id}/nummerierung` und die
/// Einstellungsseite „Werk“ (`crate::settings`).
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

/// `GET /werke/{id}/nummerierung[?schema=ID&muster=SPEC]`: alle Schemata mit Beispiel und Vorschau.
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
    /// Rücksprung nach Erfolg (nur `/einstellungen/werk…`, sonst übergangen; Runde 9).
    back: Option<String>,
}

/// `POST /werke/nummerierung/act` (Felder `work`, `scheme`, `custom`): Schema übernehmen, danach 303
/// zurück zur Seite mit Hinweis; Fehler 422 (ungültig) bzw. 409 (unverändert) mit der Seite.
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
                // Aus den Einstellungen: dieselbe Einstellungsseite mit Fehler und Eingabe (das Popup
                // übernimmt daraus den Rahmen).
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

/// `POST /werke/nummerierung/act` (im Router der schreibenden Routen, also hinter `guard::same_origin`).
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
