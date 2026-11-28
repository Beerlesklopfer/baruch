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

use crate::i18n::{self, Lang, tc};
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
        _ => return Err(invalid(tc("outline_msg.numbering.scheme_missing", &[]))),
    };
    let custom = match params.get("custom") {
        None | Some(Json::Null) => None,
        Some(Json::String(s)) => Some(s.clone()),
        Some(_) => return Err(invalid(tc("outline_msg.numbering.custom", &[]))),
    };
    Ok((scheme, custom))
}

/// A scheme with an example and a preview on the own outline.
#[derive(Debug, Clone, Serialize)]
pub struct SchemeRow {
    pub id: &'static str,
    pub label: String,
    pub example: String,
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
            Err(StoreError::Numbering(message)) => (Vec::new(), 0, Some(scheme_message(i18n::current(), &message))),
            Err(e) => return Err(e.into()),
        };
        rows.push(SchemeRow {
            id: scheme.id(),
            label: scheme_label(i18n::current(), scheme),
            example: scheme_example(i18n::current(), scheme),
            preview,
            more,
            error,
        });
    }
    Ok(rows)
}

// ---- Names in the interface language (Round 39b) ------------------------------------------------------------

/// Catalog keys (name, example) of a scheme; the crate `baruch-typst` keeps its German labels, the server translates.
fn scheme_keys(scheme: NumberingScheme) -> (&'static str, &'static str) {
    use baruch_typst::scheme::Scheme;
    match scheme {
        Scheme::Template => ("outline.scheme.template", "outline.scheme.template_example"),
        Scheme::Decimal => ("outline.scheme.decimal", "outline.scheme.decimal_example"),
        Scheme::DecimalDot => ("outline.scheme.decimal_dot", "outline.scheme.decimal_dot_example"),
        Scheme::Alphanumeric => ("outline.scheme.alphanumeric", "outline.scheme.alphanumeric_example"),
        Scheme::ChapterRoman => ("outline.scheme.chapter_roman", "outline.scheme.chapter_roman_example"),
        Scheme::ChapterArabic => ("outline.scheme.chapter_arabic", "outline.scheme.chapter_arabic_example"),
        Scheme::PartsChapters => ("outline.scheme.parts_chapters", "outline.scheme.parts_chapters_example"),
        Scheme::ChapterWords => ("outline.scheme.chapter_words", "outline.scheme.chapter_words_example"),
        Scheme::Legal => ("outline.scheme.legal", "outline.scheme.legal_example"),
        Scheme::Sermon => ("outline.scheme.sermon", "outline.scheme.sermon_example"),
        Scheme::Book => ("outline.scheme.book", "outline.scheme.book_example"),
        Scheme::None => ("outline.scheme.none", "outline.scheme.none_example"),
        Scheme::Custom => ("outline.scheme.custom", "outline.scheme.custom_example"),
    }
}

/// Name of a scheme in `lang` (German: verbatim [`NumberingScheme::label`]).
pub fn scheme_label(lang: Lang, scheme: NumberingScheme) -> String {
    i18n::t(lang, scheme_keys(scheme).0, &[])
}

/// Example of a scheme in `lang` (German: verbatim [`NumberingScheme::example`]).
pub fn scheme_example(lang: Lang, scheme: NumberingScheme) -> String {
    i18n::t(lang, scheme_keys(scheme).1, &[])
}

/// A scheme text the store hands over in German (`NumberingInfo::scheme_label`, `label`, `effective`) in `lang`: a
/// scheme name, an example or „keine“ is translated, anything else (a pattern such as `I|1`) stays as it is. The
/// filter `scheme_ui` of the templates (`i18n::install`).
pub fn scheme_text(lang: Lang, text: &str) -> String {
    for scheme in NumberingScheme::ALL {
        if text == scheme.label() {
            return scheme_label(lang, scheme);
        }
        if text == scheme.example() {
            return scheme_example(lang, scheme);
        }
    }
    if text == "keine" {
        return i18n::t(lang, "outline.scheme.no_numbers", &[]);
    }
    text.to_owned()
}

/// The German refusals of the pattern language of `baruch-typst` (`Numbering::parse_spec`), recognised by their
/// wording (Round 39c; the crate keeps its German messages): key of the catalog, regex with the captured arguments.
fn pattern_refusals() -> &'static [(&'static str, &'static str, regex::Regex)] {
    static REFUSALS: OnceLock<Vec<(&'static str, &'static str, regex::Regex)>> = OnceLock::new();
    const COUNTER: &str = r"braucht ein Zählzeichen \(1, a, A, i, I oder \*\), z\. B\. „1\.1“ oder „I\.“\.";
    REFUSALS.get_or_init(|| {
        [
            (
                "settings_msg.numbering.err_mix",
                "spec",
                r"^„(.+?)“: „\|“ \(Tiefe\) und „;“ \(Muster je Ebene\) lassen sich nicht mischen; eine Ebene ohne Nummer bleibt in der Liste einfach leer\.$".to_owned(),
            ),
            ("settings_msg.numbering.err_level", "level,part", format!(r"^Ebene (\d+) \(„(.*?)“\) {COUNTER}$")),
            (
                "settings_msg.numbering.err_depth",
                "spec",
                r"^„(.+?)“: Hinter „\|“ steht die Zahl der nummerierten Ebenen \(1 bis 6\), z\. B\. „I\|1“\.$".to_owned(),
            ),
            ("settings_msg.numbering.err_counter", "pattern", format!(r"^Das Muster „(.*?)“ {COUNTER}$")),
        ]
        .into_iter()
        .map(|(key, names, source)| {
            (key, names, regex::Regex::new(&format!("(?s){source}")).expect("valid refusal pattern"))
        })
        .collect()
    })
}

/// The refusal of a scheme in `lang`: the missing own pattern and the checks of the pattern language (which
/// `baruch-typst` words in German, German output stays verbatim) are translated; anything else stays as it is.
pub fn scheme_message(lang: Lang, message: &str) -> String {
    let missing = baruch_typst::scheme::Numbering::of_scheme(NumberingScheme::Custom, None, "").err();
    if missing.as_deref() == Some(message) {
        return i18n::t(lang, "outline.scheme.custom_missing", &[]);
    }
    for (key, names, regex) in pattern_refusals() {
        let Some(found) = regex.captures(message) else { continue };
        let values: Vec<String> =
            (1..=names.split(',').count()).map(|i| found.get(i).map_or("", |m| m.as_str()).to_owned()).collect();
        let args: Vec<(&str, i18n::Arg<'_>)> =
            names.split(',').zip(&values).map(|(name, value)| (name, i18n::Arg::from(value))).collect();
        return i18n::t(lang, key, &args);
    }
    message.to_owned()
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
        crate::i18n::install(&mut env);
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
            let text = tc(
                "outline_msg.numbering.template_error",
                &[("name", "work_numbering.html".into()), ("error", e.to_string().into())],
            );
            (StatusCode::INTERNAL_SERVER_ERROR, text).into_response()
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
    if e.code == INVALID_PARAMS && crate::ideas::is_unknown_work(&e.message) {
        return (StatusCode::NOT_FOUND, tc("outline_msg.page.unknown_work", &[])).into_response();
    }
    (status_of(&e), tc("outline_msg.page.error", &[("error", e.message.as_str().into())])).into_response()
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
                id => row.id, label => row.label.as_str(), example => row.example.as_str(), more => row.more,
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
    shown.notice = q.ok.is_some().then(|| tc("outline_msg.numbering.adopted", &[]));
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
        None => {
            return (StatusCode::UNPROCESSABLE_ENTITY, tc("outline_msg.numbering.work_missing", &[])).into_response();
        }
    };
    if let Err(e) = state.store.work_slug(work).await {
        return error_response(e.into());
    }
    let scheme = clean(form.scheme.as_deref());
    let custom = clean(form.custom.as_deref());
    let result = match scheme.as_deref() {
        Some(scheme) => set(&state, work, scheme, custom.as_deref()).await,
        None => Err(invalid(tc("outline_msg.numbering.choose", &[]))),
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

    #[test]
    fn scheme_names_are_translated_and_german_stays_verbatim() {
        for scheme in NumberingScheme::ALL {
            assert_eq!(
                scheme_label(Lang::De, scheme),
                scheme.label(),
                "German pages show the label of baruch-typst unchanged"
            );
            assert_eq!(scheme_example(Lang::De, scheme), scheme.example(), "the German example is verbatim too");
            for (key, text) in [scheme_keys(scheme).0, scheme_keys(scheme).1]
                .into_iter()
                .zip([scheme_label(Lang::En, scheme), scheme_example(Lang::En, scheme)])
            {
                assert!(i18n::exists(key), "{key} is in the English catalog (otherwise the page shows the raw key)");
                assert!(
                    !text.contains(['ä', 'ö', 'ü', 'ß', '„']),
                    "{key}: the English text carries no German letter: {text}"
                );
            }
        }
        assert_eq!(scheme_text(Lang::En, "Nur Kapitel römisch"), "Chapters only, Roman", "a stored name is translated");
        assert_eq!(scheme_text(Lang::En, "keine"), "none", "the store's word for no numbers is translated");
        assert_eq!(scheme_text(Lang::En, "I|1"), "I|1", "a pattern is no word and stays");
        assert_eq!(scheme_text(Lang::De, "Nur Kapitel römisch"), "Nur Kapitel römisch", "German stays verbatim");
    }

    #[test]
    fn the_missing_own_pattern_is_refused_in_the_interface_language() {
        let german = baruch_typst::scheme::Numbering::of_scheme(NumberingScheme::Custom, None, "").unwrap_err();
        assert_eq!(scheme_message(Lang::De, &german), german, "German keeps the message of baruch-typst verbatim");
        assert!(
            scheme_message(Lang::En, &german).starts_with("For “Own pattern” the pattern is missing"),
            "English pages explain the missing pattern in English"
        );
        assert_eq!(scheme_message(Lang::En, "other"), "other", "other checks of the pattern stay as they come");
    }
}
