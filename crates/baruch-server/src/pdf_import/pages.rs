// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pages of the PDF/EPUB import without JavaScript (Round 21a, template `templates/source_import.html`):
//!
//! - `GET /werke/import/quelle` (`?verworfen=1` = notice): form – file (.pdf/.epub), way (auto/lokal/MultiGPT),
//!   collection, tick „in MultiGPT hochladen (Sammlung …)“ (explicit), title.
//! - `POST /werke/import/quelle/vorschau` (multipart: `file`, `way`, `collection`, `upload`, `title`; after an
//!   error of MultiGPT, instead of `file` the draft `draft` with `way=local` = „Lokal lesen“ without choosing
//!   again) → 200 preview: way and reason, warnings, outline as an editable list (per row a text field `= Titel`,
//!   `== Titel`, `=== Titel`; `= Teil: Titel` makes a part, empty = strike), text sample per item, target
//!   Werk/Bibliothek/beides, „Übernehmen“ and „Verwerfen“. Errors → 422 (too large 413) with the form and a message.
//! - `POST /werke/import/quelle/uebernehmen` (form: `draft`, `target`, `title`, `author`, per row `line` and
//!   `src`) → 303 `/?work=<id>` or, for library only, 303 `/quelle?title=<id>`; error 422 with the preview and the
//!   inputs.
//! - `POST /werke/import/quelle/verwerfen` (`draft`) → 303 `/werke/import/quelle?verworfen=1`.

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

use super::draft::{self, Draft};
use super::typst_out::{Confirmed, sample};
use super::{MAX_SOURCE_FILE_BYTES, PreviewInput, SAMPLE_CHARS, Target, UPLOAD_LIMIT, Way};
use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, RpcError};
use crate::view::Settings;

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("source_import.html", include_str!("../../templates/source_import.html")),
        ] {
            env.add_template(name, source).expect("Vorlage des PDF/EPUB-Imports fehlerhaft");
        }
        env
    })
}

fn render(status: StatusCode, ctx: JinjaValue) -> Response {
    match templates().get_template("source_import.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage source_import.html: {e}")).into_response()
        }
    }
}

fn settings(headers: &HeaderMap) -> JinjaValue {
    JinjaValue::from(Serde(Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))))
}

/// Inputs of the form (for showing again).
#[derive(Debug, Clone, Default, serde::Serialize)]
struct FormValues {
    way: String,
    collection: String,
    upload: bool,
    title: String,
}

/// A row of the editable outline.
#[derive(Debug, Clone, serde::Serialize)]
struct Row {
    src: String,
    line: String,
    part_hint: bool,
    chars: usize,
    sample: String,
}

fn line_of(level: u8, title: &str) -> String {
    format!("{} {title}", "=".repeat(usize::from(level.clamp(1, 3))))
}

fn rows_of(draft: &Draft) -> Vec<Row> {
    draft
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| Row {
            src: i.to_string(),
            line: line_of(s.level, &s.title),
            part_hint: s.part,
            chars: s.plain.chars().count(),
            sample: sample(&s.plain, SAMPLE_CHARS),
        })
        .collect()
}

/// Inputs of the accept (for showing again).
#[derive(Debug, Clone, Default, serde::Serialize)]
struct ApplyValues {
    target: String,
    title: String,
    author: String,
}

async fn back_url(state: &AppState) -> Option<String> {
    state.default_work().await.ok().map(|work| format!("/?work={work}"))
}

/// Form page (also after errors of the preview).
async fn form_page(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    form: FormValues,
    error: Option<String>,
    notice: Option<&str>,
    retry_local: Option<(String, String)>,
) -> Response {
    let ctx = context! {
        settings => settings(headers),
        mode => "form",
        form => JinjaValue::from(Serde(form)),
        error => error,
        notice => notice,
        retry_local => retry_local.map(|(draft, file_name)| context! { draft => draft, file_name => file_name }),
        multigpt_configured => super::multigpt_configured(state).await,
        max_mib => MAX_SOURCE_FILE_BYTES / (1024 * 1024),
        back_url => back_url(state).await,
    };
    render(status, ctx)
}

/// Preview page (also after errors of the accept, then with the inputs).
async fn preview_page(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    draft: &Draft,
    rows: Vec<Row>,
    values: ApplyValues,
    error: Option<String>,
) -> Response {
    let preview = super::preview_json(draft);
    let ctx = context! {
        settings => settings(headers),
        mode => "preview",
        preview => JinjaValue::from(Serde(&preview)),
        way_label => if draft.way == "multigpt" { "MultiGPT" } else { "lokal" },
        kind_label => if draft.kind == "epub" { "EPUB" } else { "PDF" },
        rows => JinjaValue::from(Serde(rows)),
        values => JinjaValue::from(Serde(values)),
        error => error,
        back_url => back_url(state).await,
    };
    render(status, ctx)
}

#[derive(Deserialize)]
struct PageQuery {
    verworfen: Option<String>,
}

async fn page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<PageQuery>) -> Response {
    let notice = q.verworfen.is_some().then_some("Entwurf verworfen; es wurde nichts angelegt.");
    let form = FormValues { way: "auto".into(), ..FormValues::default() };
    form_page(&state, &headers, StatusCode::OK, form, None, notice, None).await
}

pub(super) fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/import/quelle", get(page))
}

pub(super) fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/import/quelle/vorschau", post(preview_form).layer(DefaultBodyLimit::max(UPLOAD_LIMIT)))
        .route("/werke/import/quelle/uebernehmen", post(apply_form))
        .route("/werke/import/quelle/verwerfen", post(discard_form))
}

fn too_large() -> String {
    format!("Die Datei ist größer als {} MiB.", MAX_SOURCE_FILE_BYTES / (1024 * 1024))
}

fn status_of(e: &RpcError) -> StatusCode {
    if e.code == INTERNAL { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNPROCESSABLE_ENTITY }
}

async fn preview_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let mut form = FormValues { way: "auto".into(), ..FormValues::default() };
    let mut multipart = match multipart {
        Ok(m) => m,
        Err(e) => {
            let message = format!("Das Formular kam nicht als Datei-Upload an ({}).", e.body_text());
            return form_page(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, form, Some(message), None, None)
                .await;
        }
    };
    let mut file: Option<(String, Vec<u8>)> = None;
    let mut pending: Option<String> = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                let (status, message) = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    (StatusCode::PAYLOAD_TOO_LARGE, too_large())
                } else {
                    (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        format!("Das Formular ließ sich nicht lesen ({}).", e.body_text()),
                    )
                };
                return form_page(&state, &headers, status, form, Some(message), None, None).await;
            }
        };
        let name = field.name().unwrap_or("").to_owned();
        match name.as_str() {
            "file" => {
                let file_name = field.file_name().unwrap_or("").to_owned();
                match field.bytes().await {
                    Ok(bytes) => file = Some((file_name, bytes.to_vec())),
                    Err(e) => {
                        let (status, message) = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                            (StatusCode::PAYLOAD_TOO_LARGE, too_large())
                        } else {
                            (
                                StatusCode::UNPROCESSABLE_ENTITY,
                                format!("Die Datei ließ sich nicht lesen ({}).", e.body_text()),
                            )
                        };
                        return form_page(&state, &headers, status, form, Some(message), None, None).await;
                    }
                }
            }
            "way" => form.way = field.text().await.unwrap_or_default().trim().to_owned(),
            "collection" => form.collection = field.text().await.unwrap_or_default().trim().to_owned(),
            "upload" => form.upload = !field.text().await.unwrap_or_default().trim().is_empty(),
            "title" => form.title = field.text().await.unwrap_or_default(),
            "draft" => pending = Some(field.text().await.unwrap_or_default().trim().to_owned()),
            _ => {}
        }
    }
    // „Lokal lesen“ after an error of MultiGPT: the stored file, no choosing again.
    let (file_name, bytes) = match (
        file.filter(|(n, b)| !n.trim().is_empty() || !b.is_empty()),
        pending.filter(|d| !d.is_empty()),
    ) {
        (Some(file), _) => file,
        (None, Some(id)) => {
            let data = state.data.clone();
            let owned = id.clone();
            match tokio::task::spawn_blocking(crate::i18n::carry(move || draft::load_pending(&data, &owned))).await {
                Ok(Ok((pending, bytes))) => {
                    let data = state.data.clone();
                    let _ = tokio::task::spawn_blocking(crate::i18n::carry(move || draft::remove(&data, &id))).await;
                    if form.title.trim().is_empty() {
                        form.title = pending.title.unwrap_or_default();
                    }
                    (pending.file_name, bytes)
                }
                _ => {
                    let message = "Die abgelegte Datei gibt es nicht mehr (älter als 24 Stunden oder schon gelesen); bitte die Datei neu wählen.".to_owned();
                    return form_page(
                        &state,
                        &headers,
                        StatusCode::UNPROCESSABLE_ENTITY,
                        form,
                        Some(message),
                        None,
                        None,
                    )
                    .await;
                }
            }
        }
        (None, None) => {
            let message = "Bitte eine PDF- oder EPUB-Datei wählen.".to_owned();
            return form_page(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, form, Some(message), None, None)
                .await;
        }
    };
    if bytes.len() as u64 > MAX_SOURCE_FILE_BYTES {
        return form_page(&state, &headers, StatusCode::PAYLOAD_TOO_LARGE, form, Some(too_large()), None, None).await;
    }
    let way = match Way::parse(Some(&form.way)) {
        Ok(way) => way,
        Err(e) => {
            return form_page(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, form, Some(e.message), None, None)
                .await;
        }
    };
    let input = PreviewInput {
        file_name: file_name.clone(),
        bytes,
        way,
        collection: Some(form.collection.clone()).filter(|c| !c.is_empty()),
        title: Some(form.title.clone()),
        upload: form.upload,
    };
    match super::preview(&state, input).await {
        Ok(draft) => {
            let values = ApplyValues {
                target: "work".into(),
                title: super::proposed_title(&draft),
                author: draft.author.clone().unwrap_or_default(),
            };
            preview_page(&state, &headers, StatusCode::OK, &draft, rows_of(&draft), values, None).await
        }
        Err(e) => {
            let retry = e.retry_local.map(|d| (d, super::base_name(&file_name)));
            if e.way == Some("multigpt") {
                form.way = "multigpt".into();
            }
            let status = status_of(&e.error);
            form_page(&state, &headers, status, form, Some(e.error.message), None, retry).await
        }
    }
}

/// Value of a form field (the first), otherwise empty.
fn field<'a>(fields: &'a [(String, String)], name: &str) -> &'a str {
    fields.iter().find(|(k, _)| k == name).map_or("", |(_, v)| v.as_str())
}

/// Outline from the rows of the form: `line` and `src` per row; empty row = struck.
fn parse_lines(fields: &[(String, String)]) -> Result<Vec<Confirmed>, String> {
    let lines: Vec<&str> = fields.iter().filter(|(k, _)| k == "line").map(|(_, v)| v.as_str()).collect();
    let srcs: Vec<&str> = fields.iter().filter(|(k, _)| k == "src").map(|(_, v)| v.as_str()).collect();
    let mut out = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let source = srcs.get(i).and_then(|s| s.trim().parse::<usize>().ok());
        let equals = line.chars().take_while(|c| *c == '=').count();
        if equals == 0 {
            return Err(format!(
                "Zeile „{line}“: Jede Zeile beginnt mit =, == oder === und dem Titel (leer = streichen)."
            ));
        }
        if equals > 3 {
            return Err(format!("Zeile „{line}“: Baruch kennt drei Ebenen (=, ==, ===)."));
        }
        let title = line[equals..].trim();
        let (part, title) = match title.strip_prefix("Teil:") {
            Some(rest) if equals == 1 => (true, rest.trim()),
            Some(_) => return Err(format!("Zeile „{line}“: Ein Teil steht auf der obersten Ebene („= Teil: Titel“).")),
            None => (false, title),
        };
        if title.is_empty() {
            return Err(format!("Zeile „{line}“: Titel fehlt (zum Streichen die ganze Zeile leeren)."));
        }
        out.push(Confirmed { level: equals as u8, title: title.to_owned(), part, source });
    }
    if out.is_empty() {
        return Err("Die Gliederung ist leer: mindestens ein Punkt (Kapitel) ist nötig.".into());
    }
    Ok(out)
}

/// Rows as the human submitted them (for the page after an error).
fn rows_from_fields(draft: &Draft, fields: &[(String, String)]) -> Vec<Row> {
    let lines: Vec<&str> = fields.iter().filter(|(k, _)| k == "line").map(|(_, v)| v.as_str()).collect();
    let srcs: Vec<&str> = fields.iter().filter(|(k, _)| k == "src").map(|(_, v)| v.as_str()).collect();
    if lines.is_empty() {
        return rows_of(draft);
    }
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let src = srcs.get(i).copied().unwrap_or("").to_owned();
            let section = src.parse::<usize>().ok().and_then(|s| draft.sections.get(s));
            Row {
                src,
                line: (*line).to_owned(),
                part_hint: section.is_some_and(|s| s.part),
                chars: section.map_or(0, |s| s.plain.chars().count()),
                sample: section.map(|s| sample(&s.plain, SAMPLE_CHARS)).unwrap_or_default(),
            }
        })
        .collect()
}

async fn apply_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let id = field(&fields, "draft").trim().to_owned();
    let values = ApplyValues {
        target: field(&fields, "target").trim().to_owned(),
        title: field(&fields, "title").to_owned(),
        author: field(&fields, "author").to_owned(),
    };
    let draft = match super::load_draft(&state, &id).await {
        Ok(draft) => draft,
        Err(e) => {
            let form = FormValues { way: "auto".into(), ..FormValues::default() };
            return form_page(&state, &headers, status_of(&e), form, Some(e.message), None, None).await;
        }
    };
    let failed = |message: String, status: StatusCode| {
        let state = state.clone();
        let headers = headers.clone();
        let rows = rows_from_fields(&draft, &fields);
        let values = values.clone();
        let draft = draft.clone();
        async move { preview_page(&state, &headers, status, &draft, rows, values, Some(message)).await }
    };
    let target = match Target::parse(Some(&values.target)) {
        Ok(t) => t,
        Err(e) => return failed(e.message, StatusCode::UNPROCESSABLE_ENTITY).await,
    };
    let confirmed = match parse_lines(&fields) {
        Ok(c) => c,
        Err(m) => return failed(m, StatusCode::UNPROCESSABLE_ENTITY).await,
    };
    let title = Some(values.title.as_str()).filter(|t| !t.trim().is_empty());
    let author = Some(values.author.as_str());
    match super::apply(&state, &id, target, title, author, Some(confirmed)).await {
        Ok(applied) => match (&applied.report, applied.library_title) {
            (Some(report), _) => Redirect::to(&format!("/?work={}", report.work)).into_response(),
            (None, Some(title)) => Redirect::to(&format!("/quelle?title={title}")).into_response(),
            (None, None) => Redirect::to("/werke").into_response(),
        },
        Err(e) => {
            let mut message = e.message.clone();
            if message.contains(crate::import::TITLE_HINT) {
                message = message.replace(crate::import::TITLE_HINT, "Bitte oben einen anderen Titel eintragen.");
            }
            let status = if e.code == INVALID_PARAMS || e.code == crate::rpc::REFUSED {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                status_of(&e)
            };
            failed(message, status).await
        }
    }
}

async fn discard_form(State(state): State<Arc<AppState>>, Form(fields): Form<Vec<(String, String)>>) -> Response {
    let id = field(&fields, "draft").trim().to_owned();
    let _ = super::discard(&state, &id).await;
    Redirect::to("/werke/import/quelle?verworfen=1").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn edited_lines_become_the_confirmed_outline() {
        let f = fields(&[
            ("line", "= Teil: Erster Teil"),
            ("src", ""),
            ("line", "== Umbenannt"),
            ("src", "0"),
            ("line", "  "),
            ("src", "1"),
            ("line", "= Drei"),
            ("src", "2"),
        ]);
        let outline = parse_lines(&f).unwrap();
        assert_eq!(outline.len(), 3, "die leere Zeile ist gestrichen: {outline:?}");
        assert!(
            outline[0].part && outline[0].title == "Erster Teil" && outline[0].source.is_none(),
            "Teil nur so bestätigt"
        );
        assert_eq!((outline[1].level, outline[1].title.as_str(), outline[1].source), (2, "Umbenannt", Some(0)));
        assert!(
            parse_lines(&fields(&[("line", "Titel ohne Gleichheitszeichen"), ("src", "0")])).is_err(),
            "Form verlangt"
        );
        assert!(parse_lines(&fields(&[("line", "==== Zu tief"), ("src", "0")])).is_err(), "höchstens drei Ebenen");
        assert!(parse_lines(&fields(&[("line", "== Teil: X"), ("src", "0")])).is_err(), "Teil nur oben");
        assert!(parse_lines(&fields(&[("line", ""), ("src", "0")])).is_err(), "alles gestrichen: leer");
    }
}
