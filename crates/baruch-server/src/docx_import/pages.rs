// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pages of the Word import without JavaScript (Round 32, template `templates/docx_import.html`):
//!
//! - `GET /werke/import/word`: form (file, title); with `?draft=<id>` the preview of the draft – outline,
//!   editors (name → handle, insertions/deletions, sections), number of comments, all warnings, plus
//!   „Übernehmen“ (title, checkboxes changes/comments) and „Verwerfen“. Unknown draft → 422 with form.
//! - `POST /werke/import/word/vorschau` (multipart: `file`, `title`) → 303 `/werke/import/word?draft=<id>`; error
//!   → 422 (too large 413) with the form and the message.
//! - `POST /werke/import/word/uebernehmen` (form: `draft`, `title`, checkboxes `changes`, `comments`) → 303
//!   `/?work=<id>`; if new notes arose when applying (rejected proposal, skipped image), instead
//!   200 with the report and the link to the work. Error → 422 with the preview and the inputs.
//! - `POST /werke/import/word/verwerfen` (`draft`) → 303 `/werke/import/word`.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::multipart::MultipartRejection;
use axum::extract::{DefaultBodyLimit, Form, Multipart, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use minijinja::value::{Serde, Value as JinjaValue};
use minijinja::{Environment, context};
use serde::Deserialize;

use super::draft::Draft;
use super::{MAX_DOCX_BYTES, UPLOAD_LIMIT};
use crate::AppState;
use crate::rpc::{INTERNAL, RpcError};
use crate::view::Settings;

const PAGE: &str = "/werke/import/word";

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("docx_import.html", include_str!("../../templates/docx_import.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des Word-Imports fehlerhaft");
        }
        env
    })
}

fn render(status: StatusCode, ctx: JinjaValue) -> Response {
    match templates().get_template("docx_import.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => {
            let text = crate::i18n::tc(
                "import_msg.page.template",
                &[("name", "docx_import.html".into()), ("detail", e.to_string().into())],
            );
            (StatusCode::INTERNAL_SERVER_ERROR, text).into_response()
        }
    }
}

fn settings(headers: &HeaderMap) -> JinjaValue {
    JinjaValue::from(Serde(Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))))
}

async fn back_url(state: &AppState) -> Option<String> {
    state.default_work().await.ok().map(|work| format!("/?work={work}"))
}

fn status_of(e: &RpcError) -> StatusCode {
    if e.code == INTERNAL { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNPROCESSABLE_ENTITY }
}

fn too_large() -> String {
    crate::i18n::tc("import_msg.page.too_large", &[("max", (MAX_DOCX_BYTES / (1024 * 1024)).into())])
}

/// Form page (also after errors of the preview).
async fn form_page(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    title: &str,
    error: Option<String>,
) -> Response {
    let ctx = context! {
        settings => settings(headers),
        mode => "form",
        title_value => title,
        error => error,
        max_mib => MAX_DOCX_BYTES / (1024 * 1024),
        back_url => back_url(state).await,
    };
    render(status, ctx)
}

/// Inputs of applying (to show again).
#[derive(Debug, Clone, serde::Serialize)]
struct ApplyValues {
    title: String,
    changes: bool,
    comments: bool,
}

/// Preview page (also after errors of applying, then with the inputs).
async fn preview_page(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    draft: &Draft,
    values: ApplyValues,
    error: Option<String>,
) -> Response {
    let ctx = context! {
        settings => settings(headers),
        mode => "preview",
        preview => JinjaValue::from(Serde(&draft.preview)),
        values => JinjaValue::from(Serde(values)),
        error => error,
        back_url => back_url(state).await,
    };
    render(status, ctx)
}

#[derive(Deserialize)]
struct PageQuery {
    draft: Option<String>,
}

async fn page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<PageQuery>) -> Response {
    let Some(id) = q.draft.filter(|d| !d.is_empty()) else {
        return form_page(&state, &headers, StatusCode::OK, "", None).await;
    };
    match super::load_draft(&state, &id).await {
        Ok(draft) => {
            let title = draft.preview["title"].as_str().unwrap_or("").to_owned();
            let values = ApplyValues { title, changes: true, comments: true };
            preview_page(&state, &headers, StatusCode::OK, &draft, values, None).await
        }
        Err(e) => form_page(&state, &headers, status_of(&e), "", Some(e.message)).await,
    }
}

async fn preview_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let mut title = String::new();
    let mut multipart = match multipart {
        Ok(m) => m,
        Err(e) => {
            let message = crate::i18n::tc("import_msg.page.not_upload", &[("detail", e.body_text().into())]);
            return form_page(&state, &headers, unprocessable, &title, Some(message)).await;
        }
    };
    let mut file: Option<(String, Vec<u8>)> = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                let (status, message) = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    (StatusCode::PAYLOAD_TOO_LARGE, too_large())
                } else {
                    (
                        unprocessable,
                        crate::i18n::tc("import_msg.page.form_unreadable", &[("detail", e.body_text().into())]),
                    )
                };
                return form_page(&state, &headers, status, &title, Some(message)).await;
            }
        };
        match field.name().unwrap_or("") {
            "file" => {
                let file_name = field.file_name().unwrap_or("").to_owned();
                match field.bytes().await {
                    Ok(bytes) => file = Some((file_name, bytes.to_vec())),
                    Err(e) => {
                        let (status, message) = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                            (StatusCode::PAYLOAD_TOO_LARGE, too_large())
                        } else {
                            (
                                unprocessable,
                                crate::i18n::tc("import_msg.page.file_unreadable", &[("detail", e.body_text().into())]),
                            )
                        };
                        return form_page(&state, &headers, status, &title, Some(message)).await;
                    }
                }
            }
            "title" => title = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }
    let Some((file_name, bytes)) = file.filter(|(n, b)| !n.trim().is_empty() || !b.is_empty()) else {
        let message = crate::i18n::tc("import_msg.page.choose_docx", &[]);
        return form_page(&state, &headers, unprocessable, &title, Some(message)).await;
    };
    if bytes.len() as u64 > MAX_DOCX_BYTES {
        return form_page(&state, &headers, StatusCode::PAYLOAD_TOO_LARGE, &title, Some(too_large())).await;
    }
    match super::preview(&state, &file_name, bytes, Some(&title)).await {
        Ok(draft) => Redirect::to(&format!("{PAGE}?draft={}", draft.id)).into_response(),
        Err(e) => form_page(&state, &headers, status_of(&e), &title, Some(e.message)).await,
    }
}

#[derive(Deserialize)]
struct ApplyForm {
    #[serde(default)]
    draft: String,
    #[serde(default)]
    title: String,
    changes: Option<String>,
    comments: Option<String>,
}

async fn apply_form(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ApplyForm>) -> Response {
    let values =
        ApplyValues { title: form.title.clone(), changes: form.changes.is_some(), comments: form.comments.is_some() };
    let draft = match super::load_draft(&state, &form.draft).await {
        Ok(draft) => draft,
        Err(e) => return form_page(&state, &headers, status_of(&e), &form.title, Some(e.message)).await,
    };
    match super::apply(&state, &form.draft, Some(&form.title), values.changes, values.comments).await {
        Ok(applied) if applied.new_warnings == 0 => {
            Redirect::to(&format!("/?work={}", applied.report.work)).into_response()
        }
        Ok(applied) => {
            let ctx = context! {
                settings => settings(&headers),
                mode => "report",
                report => JinjaValue::from(Serde(super::applied_json(&applied))),
                report_title => applied.report.title,
                back_url => back_url(&state).await,
            };
            render(StatusCode::OK, ctx)
        }
        Err(e) => preview_page(&state, &headers, status_of(&e), &draft, values, Some(e.message)).await,
    }
}

#[derive(Deserialize)]
struct DiscardForm {
    #[serde(default)]
    draft: String,
}

async fn discard_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<DiscardForm>,
) -> Response {
    match super::discard(&state, &form.draft).await {
        Ok(_) => Redirect::to(PAGE).into_response(),
        Err(e) => form_page(&state, &headers, status_of(&e), "", Some(e.message)).await,
    }
}

pub(super) fn pages() -> Router<Arc<AppState>> {
    Router::new().route(PAGE, get(page))
}

pub(super) fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/import/word/vorschau", post(preview_form).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)))
        .route("/werke/import/word/uebernehmen", post(apply_form))
        .route("/werke/import/word/verwerfen", post(discard_form))
}
