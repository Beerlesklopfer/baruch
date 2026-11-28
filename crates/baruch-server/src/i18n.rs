// SPDX-License-Identifier: AGPL-3.0-or-later
//! Language of the user interface (Round 39a, interface R39a-S1): catalogs, resolution per request, templates,
//! formatting.
//!
//! **Catalogs:** `locales/<lang>/<stem>.ftl` (lang `en` = base catalog, `de` = translation), embedded by `build.rs`
//! as [`CATALOGS`]; a new area file needs no code change. Keys are written dotted at column 0
//! (`outline.add.button = Add item`, regex [`KEY_PATTERN`], first segment = file stem); the loader maps `.` to `-`
//! for the Fluent id. One concurrent Fluent bundle per language, built once at first use without isolation marks
//! (no U+2068/U+2069 in HTML); a parse error or a duplicate key panics there (the checks in
//! `tests/round39a_i18n.rs` catch it before, [`catalog_problems`]).
//!
//! **Lookup:** [`t`] takes the text of the language, falls back to the English text and finally to the key itself.
//! [`tc`] uses the language of the running request ([`current`]).
//!
//! **Resolution:** [`layer`] (outside `auth::gate`) decides per request: stored instance setting (only without
//! accounts) → cookie [`COOKIE`] → `Accept-Language` → English; `/mcp` is always English. [`account_layer`] (inside
//! the gate, where `auth::current` is known) lets the own setting of an account – and for the author on the own
//! machine the instance setting – override that. The language lives in a task-local slot for the duration of the
//! request, like `auth::current`.
//!
//! Rule: spawned tasks (`tokio::spawn`, `spawn_blocking`) do not inherit the slot. Fetch [`current`] before and run
//! the task in [`scope`] / [`sync_scope`] ([`carry`] wraps a closure); a response body polled after the handler
//! returned is wrapped in [`scoped_stream`].
//!
//! **Templates:** [`install`] registers `t(key, **args)`, `ui_lang()`, `widget_catalog()` and the filters
//! `ui_minutes`, `ui_day`, `ui_number(decimals=0)` in an environment (contract §2.2).

use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use std::task::{Context, Poll};

use axum::extract::{Request, State};
use axum::http::{HeaderMap, header};
use axum::middleware::Next;
use axum::response::Response;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use minijinja::value::{Kwargs, ValueKind};
use minijinja::{Environment, Error as TemplateError, State as TemplateState, Value};

use crate::AppState;
use crate::auth::{self, Who};

/// Catalog files from `build.rs`: `(language, stem, content)`, sorted by language and stem.
pub const CATALOGS: &[(&str, &str, &str)] = include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// Cookie of the chosen language (`en` | `de`; missing = like the browser).
pub const COOKIE: &str = "baruch_lang";

/// Valid key (contract §4): lower-case segments separated by dots, at least two segments.
pub const KEY_PATTERN: &str = r"^[a-z][a-z0-9_]*(\.[a-z0-9_]+)+$";

/// Prefix of the keys the widgets get through [`widget_catalog_json`].
pub const WIDGET_PREFIX: &str = "widget.";

/// The only tags a key ending in `_html` may contain (no attributes).
pub const MARKUP_TAGS: [&str; 9] =
    ["<code>", "</code>", "<strong>", "</strong>", "<em>", "</em>", "<kbd>", "</kbd>", "<br>"];

/// Shared words of `common.ftl` for every area (contract §1.13): developers use them, but add new words to the
/// file of their own area.
pub const COMMON_KEYS: [&str; 13] = [
    "common.lang.label",
    "common.lang.en",
    "common.lang.de",
    "common.lang.auto",
    "common.lang.invalid",
    "common.lang.hint_html",
    "common.save",
    "common.cancel",
    "common.delete",
    "common.close",
    "common.back",
    "common.yes",
    "common.no",
];

/// Language of the user interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    En,
    De,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::De];

    /// `"en"` | `"de"`.
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
        }
    }

    /// Language of a tag by its primary subtag, case-insensitive: `de-AT` → German, `fr` → `None`.
    pub fn parse(tag: &str) -> Option<Lang> {
        let primary = tag.trim().split(['-', '_']).next().unwrap_or("");
        Lang::ALL.into_iter().find(|lang| primary.eq_ignore_ascii_case(lang.code()))
    }

    /// Exactly one of the codes (`en`, `de`); used for cookie, form and template variable.
    fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|lang| lang.code() == code)
    }

    fn index(self) -> usize {
        match self {
            Lang::En => 0,
            Lang::De => 1,
        }
    }
}

/// Argument of a message: text or number (numbers select plural forms).
#[derive(Debug, Clone)]
pub enum Arg<'a> {
    Str(Cow<'a, str>),
    Num(f64),
}

impl<'a> From<&'a str> for Arg<'a> {
    fn from(value: &'a str) -> Self {
        Arg::Str(Cow::Borrowed(value))
    }
}

impl From<String> for Arg<'_> {
    fn from(value: String) -> Self {
        Arg::Str(Cow::Owned(value))
    }
}

impl<'a> From<&'a String> for Arg<'a> {
    fn from(value: &'a String) -> Self {
        Arg::Str(Cow::Borrowed(value.as_str()))
    }
}

macro_rules! number_arg {
    ($($t:ty),*) => {$(
        impl From<$t> for Arg<'_> {
            #[allow(clippy::cast_precision_loss, clippy::cast_lossless)]
            fn from(value: $t) -> Self {
                Arg::Num(value as f64)
            }
        }
    )*};
}

number_arg!(i32, i64, u32, u64, usize, f64);

// ---- catalogs --------------------------------------------------------------------------------------------------

/// Entries of a catalog file in source order: `(key, raw value)`; the raw value joins the first line and its
/// indented continuation lines (variants, multiline text) with `\n`. Comments (`#`) and blank lines are skipped.
pub fn catalog_entries(source: &str) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = Vec::new();
    let mut open = false;
    for line in source.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with('#') {
            open = false;
        } else if line.starts_with([' ', '\t']) {
            if open && let Some((_, value)) = entries.last_mut() {
                value.push('\n');
                value.push_str(line);
            }
        } else if let Some((key, value)) = line.split_once('=') {
            entries.push((key.trim().to_owned(), value.trim().to_owned()));
            open = true;
        } else {
            open = false;
        }
    }
    entries
}

/// Source in Fluent syntax: the dotted key at column 0 becomes a Fluent id (`.` → `-`).
fn fluent_source(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for line in source.lines() {
        match line.split_once('=') {
            Some((key, rest)) if line.starts_with(|c: char| c.is_ascii_lowercase()) => {
                out.push_str(&key.replace('.', "-"));
                out.push('=');
                out.push_str(rest);
            }
            _ => out.push_str(line),
        }
        out.push('\n');
    }
    out
}

fn fluent_id(key: &str) -> String {
    key.replace('.', "-")
}

fn langid(lang: Lang) -> unic_langid::LanguageIdentifier {
    lang.code().parse().expect("en and de are valid language tags")
}

/// Builds the bundle of a language; `Err` lists every parse error and every duplicate key.
fn build_bundle(lang: Lang) -> Result<FluentBundle<FluentResource>, Vec<String>> {
    let mut bundle = FluentBundle::new_concurrent(vec![langid(lang)]);
    bundle.set_use_isolating(false);
    let mut problems = Vec::new();
    for (code, stem, source) in CATALOGS.iter().filter(|(code, _, _)| *code == lang.code()) {
        let resource = match FluentResource::try_new(fluent_source(source)) {
            Ok(resource) => resource,
            Err((resource, errors)) => {
                problems.extend(errors.iter().map(|e| format!("locales/{code}/{stem}.ftl: {e:?}")));
                resource
            }
        };
        if let Err(errors) = bundle.add_resource(resource) {
            problems.extend(errors.iter().map(|e| format!("locales/{code}/{stem}.ftl: {e}")));
        }
    }
    if problems.is_empty() { Ok(bundle) } else { Err(problems) }
}

/// Parse errors and duplicate keys of all catalogs (empty = all bundles build). For the checks; the bundles
/// themselves panic on the first use instead.
pub fn catalog_problems() -> Vec<String> {
    Lang::ALL.into_iter().filter_map(|lang| build_bundle(lang).err()).flatten().collect()
}

static BUNDLES: LazyLock<[FluentBundle<FluentResource>; 2]> = LazyLock::new(|| {
    Lang::ALL.map(|lang| {
        build_bundle(lang).unwrap_or_else(|problems| panic!("catalogs of {}: {}", lang.code(), problems.join("; ")))
    })
});

fn format(lang: Lang, key: &str, args: &[(&str, Arg<'_>)]) -> Option<String> {
    let bundle = &BUNDLES[lang.index()];
    let pattern = bundle.get_message(&fluent_id(key))?.value()?;
    let mut fluent_args = FluentArgs::with_capacity(args.len());
    for (name, arg) in args {
        match arg {
            Arg::Str(text) => fluent_args.set(*name, FluentValue::from(text.as_ref())),
            Arg::Num(value) => fluent_args.set(*name, FluentValue::from(*value)),
        }
    }
    let mut errors = Vec::new();
    Some(bundle.format_pattern(pattern, Some(&fluent_args), &mut errors).into_owned())
}

/// Text of `key` in `lang`; missing there → the English text; missing in English → the key itself.
pub fn t(lang: Lang, key: &str, args: &[(&str, Arg<'_>)]) -> String {
    format(lang, key, args)
        .or_else(|| (lang != Lang::En).then(|| format(Lang::En, key, args)).flatten())
        .unwrap_or_else(|| key.to_owned())
}

/// [`t`] in the language of the running request.
pub fn tc(key: &str, args: &[(&str, Arg<'_>)]) -> String {
    t(current(), key, args)
}

/// Is `key` in the English catalog?
pub fn exists(key: &str) -> bool {
    BUNDLES[Lang::En.index()].has_message(&fluent_id(key))
}

/// HTML of a markup key (`…_html`): the text and the arguments escaped, only [`MARKUP_TAGS`] of the text restored.
pub fn t_html(lang: Lang, key: &str, args: &[(&str, Arg<'_>)]) -> String {
    let mut slots: Vec<String> = Vec::new();
    let masked: Vec<(&str, Arg<'_>)> = args
        .iter()
        .map(|(name, arg)| match arg {
            Arg::Str(text) => {
                slots.push(escape(text));
                (*name, Arg::Str(Cow::Owned(format!("\u{E000}{}\u{E001}", slots.len() - 1))))
            }
            Arg::Num(value) => (*name, Arg::Num(*value)),
        })
        .collect();
    let mut html = escape(&t(lang, key, &masked));
    for tag in MARKUP_TAGS {
        html = html.replace(&escape(tag), tag);
    }
    for (i, slot) in slots.iter().enumerate() {
        html = html.replace(&format!("\u{E000}{i}\u{E001}"), slot);
    }
    html
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

/// Names of the variables (`{ $name }`) in a raw catalog value, in order of appearance, each once.
pub fn placeables(raw: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut rest = raw;
    while let Some(start) = rest.find('$') {
        let tail = &rest[start + 1..];
        let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')).unwrap_or(tail.len());
        let name = &tail[..end];
        if !name.is_empty() && !names.iter().any(|n| n == name) {
            names.push(name.to_owned());
        }
        rest = &tail[end..];
    }
    names
}

static WIDGET_CATALOGS: LazyLock<[String; 2]> = LazyLock::new(|| Lang::ALL.map(build_widget_catalog));

fn build_widget_catalog(lang: Lang) -> String {
    let mut messages = serde_json::Map::new();
    for (_, _, source) in CATALOGS.iter().filter(|(code, _, _)| *code == Lang::En.code()) {
        for (key, raw) in catalog_entries(source) {
            if !key.starts_with(WIDGET_PREFIX) {
                continue;
            }
            // The widgets replace `{ $name }` themselves (web/src/i18n.ts): every variable stays in the text.
            let names = placeables(&raw);
            let keep: Vec<String> = names.iter().map(|name| format!("{{ ${name} }}")).collect();
            let args: Vec<(&str, Arg<'_>)> =
                names.iter().zip(&keep).map(|(name, text)| (name.as_str(), Arg::from(text))).collect();
            messages.insert(key.clone(), serde_json::Value::String(t(lang, &key, &args)));
        }
    }
    // Braces inside the texts as JSON escapes and a space before the closing brace: the page never contains `{{`,
    // `{%` or `}}` (tests check finished pages for leftovers of the template language).
    let messages = escape_braces_in_strings(&serde_json::Value::Object(messages).to_string());
    let json = format!("{{\"lang\":\"{}\",\"messages\":{messages} }}", lang.code());
    // Inside <script type="application/json">: no `</script>`, no markup.
    json.replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026")
}

/// `{` and `}` inside the JSON strings of `json` as `\u007b`/`\u007d` (the structure keeps its braces).
fn escape_braces_in_strings(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    let (mut in_string, mut escaped) = (false, false);
    for c in json.chars() {
        match c {
            '{' if in_string => out.push_str("\\u007b"),
            '}' if in_string => out.push_str("\\u007d"),
            _ => {
                if in_string && !escaped && c == '"' {
                    in_string = false;
                } else if !in_string && c == '"' {
                    in_string = true;
                }
                escaped = in_string && !escaped && c == '\\';
                out.push(c);
            }
        }
    }
    out
}

/// Catalog of the widgets for the page head: `{"lang":"de","messages":{"widget.…":"…"}}`, with `<`, `>`, `&` as
/// JSON escapes (safe inside `<script type="application/json">`).
pub fn widget_catalog_json(lang: Lang) -> String {
    WIDGET_CATALOGS[lang.index()].clone()
}

// ---- language of the request -----------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Slot {
    lang: Lang,
    /// Explicit choice (stored setting or cookie); `None` = like the browser.
    chosen: Option<Lang>,
}

tokio::task_local! {
    static SLOT: Slot;
    /// Round 39b: path and query of the running GET request (the way back of the language switch).
    static PAGE: Option<String>;
}

/// Language of the running request; outside a request English.
pub fn current() -> Lang {
    SLOT.try_with(|slot| slot.lang).unwrap_or_default()
}

/// The explicit choice that applies to the running request (stored setting or cookie), `None` = like the browser
/// (also outside a request). For the settings page and `settings.get`.
pub fn chosen() -> Option<Lang> {
    SLOT.try_with(|slot| slot.chosen).ok().flatten()
}

/// Language of the running request, `None` outside a request (direct calls, background tasks, unit tests). Labels
/// that other crates write in German keep that wording outside a request (Round 39b, e.g. `findings::status_label`).
pub fn request_lang() -> Option<Lang> {
    SLOT.try_with(|slot| slot.lang).ok()
}

/// Path of the running request if it is a GET of this server, with only the harmless query values of
/// [`BACK_PARAMS`] (`/werke?work=2`); `None` for other methods (a form answer must not lead back to a POST address)
/// and outside a request. The menu form of the language switch sends it as `back` (Round 39b), so the switch needs
/// no Referer. Other query values (a search text, a target, a message code) never go back into the page.
pub fn page_path() -> Option<String> {
    PAGE.try_with(Clone::clone).ok().flatten()
}

/// Query parameters [`page_path`] keeps: the place in Baruch, never text from the address.
pub const BACK_PARAMS: [&str; 5] = ["work", "key", "rev", "tab", "seite"];

/// [`page_path`] of a path and query: the path plus the [`BACK_PARAMS`] whose value is short and plain (letters,
/// digits, `-`, `_`).
pub fn back_of(path: &str, query: Option<&str>) -> String {
    let plain = |value: &str| {
        !value.is_empty()
            && value.len() <= 40
            && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    };
    let kept: Vec<&str> = query
        .unwrap_or("")
        .split('&')
        .filter(|pair| pair.split_once('=').is_some_and(|(name, value)| BACK_PARAMS.contains(&name) && plain(value)))
        .collect();
    if kept.is_empty() { path.to_owned() } else { format!("{path}?{}", kept.join("&")) }
}

/// Runs `f` with `lang` as [`current`].
pub async fn scope<F: Future>(lang: Lang, f: F) -> F::Output {
    SLOT.scope(Slot { lang, chosen: chosen() }, f).await
}

/// Runs `f` synchronously with `lang` as [`current`].
pub fn sync_scope<R>(lang: Lang, f: impl FnOnce() -> R) -> R {
    SLOT.sync_scope(Slot { lang, chosen: chosen() }, f)
}

/// Closure for `spawn_blocking` that runs `f` in the language of the request that creates it.
pub fn carry<R, F: FnOnce() -> R>(f: F) -> impl FnOnce() -> R {
    let lang = current();
    move || sync_scope(lang, f)
}

/// A stream whose every poll runs with `lang` as [`current`] (response bodies are polled outside the scope of the
/// middleware).
pub fn scoped_stream<S: tokio_stream::Stream>(lang: Lang, s: S) -> impl tokio_stream::Stream<Item = S::Item> {
    Scoped { lang, inner: Box::pin(s) }
}

struct Scoped<S> {
    lang: Lang,
    inner: Pin<Box<S>>,
}

impl<S: tokio_stream::Stream> tokio_stream::Stream for Scoped<S> {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        let lang = self.lang;
        let inner = self.inner.as_mut();
        sync_scope(lang, || inner.poll_next(cx))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// Best supported language of an `Accept-Language` header: q-values honoured (`q=0` excluded), the highest wins,
/// ties by order; `None` if no supported language is named.
pub fn negotiate(accept_language: &str) -> Option<Lang> {
    let mut best: Option<(f32, Lang)> = None;
    for item in accept_language.split(',') {
        let mut parts = item.split(';');
        let Some(lang) = parts.next().and_then(Lang::parse) else { continue };
        let mut q = 1.0_f32;
        for param in parts {
            if let Some(value) = param.trim().strip_prefix("q=").or_else(|| param.trim().strip_prefix("Q=")) {
                q = value.trim().parse().unwrap_or(0.0);
            }
        }
        if q > 0.0 && best.is_none_or(|(top, _)| q > top) {
            best = Some((q, lang));
        }
    }
    best.map(|(_, lang)| lang)
}

/// Language of the cookie [`COOKIE`], if it names one exactly.
fn cookie_lang(headers: &HeaderMap) -> Option<Lang> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .and_then(|(_, value)| Lang::from_code(value.trim()))
}

/// Order of precedence: stored setting → cookie → `Accept-Language` → English.
pub fn resolve(headers: &HeaderMap, stored: Option<Lang>) -> Lang {
    stored
        .or_else(|| cookie_lang(headers))
        .or_else(|| headers.get(header::ACCEPT_LANGUAGE).and_then(|v| v.to_str().ok()).and_then(negotiate))
        .unwrap_or_default()
}

/// Stored language of an account (`Some(id)`) or of the instance (`None`); unreadable → `None` (logged).
pub(crate) async fn stored(state: &AppState, account: Option<i64>) -> Option<Lang> {
    match state.store.ui_language(account).await {
        Ok(code) => code.as_deref().and_then(Lang::from_code),
        Err(e) => {
            eprintln!("Sprache der Oberfläche nicht lesbar: {e}");
            None
        }
    }
}

/// Outer layer (outside `auth::gate`): decides the language of the request (module head). Rejections of the gate
/// use this language.
pub async fn layer(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    if request.uri().path() == "/mcp" {
        return SLOT.scope(Slot { lang: Lang::En, chosen: None }, next.run(request)).await;
    }
    let cookie = cookie_lang(request.headers());
    let instance = if auth::accounts_exist(&state).await { None } else { stored(&state, None).await };
    let lang = resolve(request.headers(), instance);
    let page =
        (request.method() == axum::http::Method::GET).then(|| back_of(request.uri().path(), request.uri().query()));
    SLOT.scope(Slot { lang, chosen: instance.or(cookie) }, PAGE.scope(page, next.run(request))).await
}

/// Inner layer (inside `auth::gate`): the own setting of an account – for the author on the own machine the
/// instance setting – overrides the language of [`layer`].
pub async fn account_layer(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let own = match auth::current() {
        Who::Account(identity) => stored(&state, Some(identity.id)).await,
        Who::LocalAuthor => stored(&state, None).await,
        Who::Open | Who::Anonymous => None,
    };
    match own {
        Some(lang) => SLOT.scope(Slot { lang, chosen: Some(lang) }, next.run(request)).await,
        None => next.run(request).await,
    }
}

// ---- formatting ------------------------------------------------------------------------------------------------

/// Number with `decimals` places and thousands separators: de `1.234,5`, en `1,234.5`.
pub fn number(lang: Lang, value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let (group, point) = match lang {
        Lang::En => (',', '.'),
        Lang::De => ('.', ','),
    };
    let plain = format!("{:.*}", decimals, value.abs());
    let (whole, fraction) = plain.split_once('.').unwrap_or((&plain, ""));
    let mut out = String::with_capacity(plain.len() + whole.len() / 3 + 1);
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            out.push(group);
        }
        out.push(digit);
    }
    if !fraction.is_empty() {
        out.push(point);
        out.push_str(fraction);
    }
    let zero = plain.bytes().all(|b| b == b'0' || b == b'.');
    if value < 0.0 && !zero { format!("-{out}") } else { out }
}

// ---- templates -------------------------------------------------------------------------------------------------

/// Language for a template: its variable `lang` if it is `en` or `de`, otherwise the language of the request.
fn template_lang(state: &TemplateState) -> Lang {
    state.lookup("lang").and_then(|value| value.as_str().and_then(Lang::from_code)).unwrap_or_else(current)
}

fn template_t(state: &TemplateState, key: &str, kwargs: Kwargs) -> Result<Value, TemplateError> {
    let lang = template_lang(state);
    let mut values: Vec<(&str, Value)> = Vec::new();
    for name in kwargs.args() {
        values.push((name, kwargs.get::<Value>(name)?));
    }
    kwargs.assert_all_used()?;
    let args: Vec<(&str, Arg<'_>)> = values
        .iter()
        .map(|(name, value)| {
            let arg = match value.kind() {
                ValueKind::Number => Arg::Num(f64::try_from(value.clone()).unwrap_or_default()),
                ValueKind::Undefined | ValueKind::None => Arg::Str(Cow::Borrowed("")),
                _ => Arg::Str(Cow::Owned(value.to_string())),
            };
            (*name, arg)
        })
        .collect();
    if key.ends_with("_html") {
        Ok(Value::from_safe_string(t_html(lang, key, &args)))
    } else {
        Ok(Value::from(t(lang, key, &args)))
    }
}

/// Registers the functions `t`, `ui_lang`, `widget_catalog`, `help_url`, `page_path` and the filters `ui_minutes`,
/// `ui_day`, `ui_number`, `scheme_ui` in an environment; called directly after every `Environment::new()`.
pub fn install(env: &mut Environment<'_>) {
    env.add_function("t", template_t);
    env.add_function("ui_lang", |state: &TemplateState| template_lang(state).code());
    env.add_function("widget_catalog", |state: &TemplateState| {
        Value::from_safe_string(widget_catalog_json(template_lang(state)))
    });
    // Round 39b: address of the help in the page language (about::help_url); a fixed https address, not escaped so
    // that the link reads like the literal it replaced.
    env.add_function("help_url", |state: &TemplateState| {
        Value::from_safe_string(crate::about::help_url(template_lang(state)).to_owned())
    });
    // Round 39b: the way back of the language switch (page_path), escaped for an attribute; empty outside a GET.
    env.add_function("page_path", || {
        Value::from_safe_string(page_path().map(|path| escape(&path)).unwrap_or_default())
    });
    // Round 39b: a scheme text of the store (German name, example or the word for no numbers) in the page language;
    // an undefined value gives an empty text.
    env.add_filter("scheme_ui", |state: &TemplateState, text: Value| {
        text.as_str().map(|text| crate::numbering::scheme_text(template_lang(state), text)).unwrap_or_default()
    });
    env.add_filter("ui_minutes", |state: &TemplateState, stamp: String| {
        crate::localtime::ui_minutes(template_lang(state), &stamp)
    });
    env.add_filter("ui_day", |state: &TemplateState, stamp: String| {
        crate::localtime::ui_day(template_lang(state), &stamp)
    });
    env.add_filter("ui_number", |state: &TemplateState, value: f64, decimals: Option<usize>| {
        number(template_lang(state), value, decimals.unwrap_or(0))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_way_back_keeps_only_the_place_and_never_text_from_the_address() {
        assert_eq!(back_of("/werke", None), "/werke", "a path without query stays");
        assert_eq!(back_of("/", Some("work=2&key=7&tab=buch")), "/?work=2&key=7&tab=buch", "the place is kept");
        assert_eq!(back_of("/literatur", Some("q=Turm&work=2")), "/literatur?work=2", "a search text is dropped");
        assert_eq!(back_of("/x", Some("ziel=%3Cb%3E&ok=1")), "/x", "a target or message code never comes back");
        assert_eq!(back_of("/x", Some("work=%3Cscript")), "/x", "only plain values are kept");
    }

    #[test]
    fn tags_are_read_by_their_primary_subtag() {
        assert_eq!(Lang::parse("de-AT"), Some(Lang::De), "Austrian German is German");
        assert_eq!(Lang::parse("EN_gb"), Some(Lang::En), "case and underscore do not matter");
        assert_eq!(Lang::parse("fr"), None, "French is not offered");
        assert_eq!(Lang::parse(""), None, "an empty tag names nothing");
    }

    #[test]
    fn every_shared_word_exists_in_both_languages() {
        for key in COMMON_KEYS {
            assert!(exists(key), "{key} is promised to all areas by contract §1.13 and must be in the en catalog");
            assert_ne!(t(Lang::De, key, &[]), key, "{key} must have a German text, not the raw key");
        }
    }

    #[test]
    fn a_missing_key_falls_back_to_english_and_then_to_the_key() {
        assert_eq!(t(Lang::De, "common.no_such_word", &[]), "common.no_such_word", "unknown key shows itself");
        assert_eq!(t(Lang::En, "common.lang.de", &[]), "Deutsch", "language names are written in their own language");
    }

    #[test]
    fn markup_keeps_only_the_allowed_tags_and_escapes_arguments() {
        let html = t_html(Lang::En, "common.no_such_key_html", &[("x", Arg::from("<b>"))]);
        assert_eq!(html, "common.no_such_key_html", "an unknown markup key shows the key (no markup)");
        assert_eq!(escape("<em>&\"'"), "&lt;em&gt;&amp;&quot;&#x27;", "every special character is escaped");
    }

    #[test]
    fn braces_inside_json_strings_become_escapes() {
        let json = serde_json::json!({ "widget.a": "Hi { $name } \"{}\" \\" }).to_string();
        let escaped = escape_braces_in_strings(&json);
        assert!(!escaped.contains("{ $") && !escaped.contains("}}"), "no template-like braces remain: {escaped}");
        let back: serde_json::Value = serde_json::from_str(&escaped).expect("still valid JSON");
        assert_eq!(back["widget.a"], "Hi { $name } \"{}\" \\", "JSON readers get the braces back");
    }

    #[test]
    fn the_catalog_line_parser_joins_continuation_lines() {
        let entries = catalog_entries(
            "# comment\nx.a = One\nx.b =\n    { $count ->\n        [one] one\n       *[other] { $count } more\n    }\n",
        );
        assert_eq!(entries.len(), 2, "two keys, the comment is skipped");
        assert_eq!(entries[0], ("x.a".to_owned(), "One".to_owned()), "simple value");
        assert!(entries[1].1.contains("*[other]"), "the variants belong to the second key: {:?}", entries[1]);
        assert_eq!(placeables(&entries[1].1), ["count"], "the selector variable is found once");
    }
}
