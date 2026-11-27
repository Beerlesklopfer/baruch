// SPDX-License-Identifier: AGPL-3.0-or-later
//! Personas (contract Round 7, „proposals“; Round 9, settings „Assistenten“): RPC `persona.*`,
//! the form to `POST /einstellungen/personas/act` (usable without JavaScript) and the tints for the
//! light and the dark theme. Since Round 9 the page itself is the settings page
//! `/einstellungen/assistenten` ([`crate::settings`]); `/einstellungen/personas` redirects there.
//!
//! RPC ([`handles`], [`call`]; called from `rpc::call` before the work is chosen, personas are global):
//! - `persona.list {}` → `{personas: [{handle, name, role, model, context_length, active, lector,
//!   color, chosen, tints: {light, dark, line, line_dark}}], palette: [..]}`; everything from the table
//!   `persona` (Round 9, complete even without a work)
//! - `persona.set_color {handle, color: "#rrggbb" | null}` → `{handle, color, chosen}`; `null` = back
//!   to the default. Unknown persona or invalid colour → -32602. Only the human: not through `/act`
//!   (not allowed there) and not through MCP (no tool). No commit in the mirror (no book text).
//! - `persona.update {handle, name?, role?, model?, context_length?, active?}` → `PersonaRow` (Round 9;
//!   Round 11: `name` trimmed 1–80 characters, everything in one transaction):
//!   role and model trimmed 1–200 characters, context length 1–10000000 tokens; otherwise, or for an
//!   unknown persona, -32602. Only the human, no commit. Model and context length are used by the
//!   persona jobs (Round 10, [`crate::assist`]).
//! - `persona.prompt_get {handle}` → `{handle, prompt, default, custom}` (Round 10): effective
//!   role prompt, the default from the code and whether an own one is stored.
//! - `persona.set_prompt {handle, prompt: Text | null}` → ditto; `null` = back to the default, own
//!   prompt trimmed 1–8000 characters (otherwise -32000). Only the human.
//! - `persona.lector_get {}` → `{mode: "fixed"|"auto", handle?, lector}`; `persona.set_lector {mode,
//!   handle?}` → ditto (Round 10): fixed one persona (must exist) or automatic by context.
//!   Default of the trial since 2026-11-06: fixed `@lektor`. Only the human.
//! - `persona.ask/review/propose/chat`: jobs for personas ([`crate::assist::rpc`]).
//! - `persona.rag_get {persona}` → `{persona, rag_context, allowed}`; `persona.rag_set {persona, on}` →
//!   `{persona, rag_context}` (Round 12, migration 0032): pass hits of the semantic search along as evidence.
//!   Allowed only for `@belege` and the persona that is the lector by the choice (fixed or automatic), otherwise
//!   -32602 [`RAG_REFUSED`]; unknown persona -32602. Only the human (not through `/act`, no MCP).
//! - Round 15b: `persona.list` additionally names per persona `connection {id, name, kind}`, `locality`
//!   („lokal“/„extern“), `external_allowed` and `context {tokens, reported, warning}` (without a network query, from
//!   the caches of the models). `persona.backend_get {persona}` → `{persona, connection, locality,
//!   external_allowed, explicit, usable, warning?}`, `persona.backend_set {persona, connection,
//!   external_allowed}` → ditto (store `persona_backend`, migration 0036). `persona.bible_get {persona}` →
//!   `{persona, bible_context, allowed, default}`, `persona.bible_set {persona, on}` → `{persona,
//!   bible_context}` (migration 0037): Bible evidence from SWORD ([`crate::bible_research`]) only for `@belege`
//!   (default on) and the lector (default off), otherwise -32602 [`BIBLE_REFUSED`]. Only the human.
//! - Round 16a: `persona.chat_scope_get {}` → `{scope: "chapter"|"section"}`, `persona.chat_scope_set {scope}` →
//!   ditto (setting „Chat nur Abschnitt“, `store_meta.chat_scope`, SQLite only, no mirror). Only the human:
//!   not through `/act`, not through MCP; form `method=persona.chat_scope_set` to
//!   `POST /einstellungen/personas/act` → 303 `…?ok=chat#chat`. `persona.list` additionally names `chat_scope`;
//!   „Lektor automatisch nach Kontext“ compares the effective context length ([`effective_lector_of`]).
//! - Round 17a (L4): pages and lists (`persona.list`, `persona.lector_get`, Bible and RAG switches, Einstellungen →
//!   Assistenten, work page) determine the lector without a network query ([`rows_and_lector_cached`],
//!   `ModelPort::context_cached`: cache, otherwise the setting); only the model call asks the
//!   remote side ([`rows_and_lector`]). `persona.list` additionally names `lector_context` (effective context length
//!   of the lector: `{tokens, reported, warning}` or `null`).
//!
//! Colours reach the pages only as checked `#rrggbb` (`baruch_store::valid_color`), as
//! CSS variables `--pc-light`, `--pc-dark`, `--pc-line`, `--pc-line-dark` in the `style` attribute
//! ([`style`]). Since Round 8 the lines reach at least 3:1 against the surface of their theme
//! (WCAG 1.4.11, non-text contrast), so that a collapsed line stays visible even with yellow or
//! ochre.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::LectorChoice;
use baruch_store::{PERSONA_PALETTE, PersonaColor, PersonaEdit, PersonaRow, valid_color};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

const METHODS: &[&str] = &[
    "persona.list",
    "persona.set_color",
    "persona.update",
    "persona.prompt_get",
    "persona.set_prompt",
    "persona.lector_get",
    "persona.set_lector",
    "persona.rag_get",
    "persona.rag_set",
    "persona.backend_get",
    "persona.backend_set",
    "persona.bible_get",
    "persona.bible_set",
    "persona.chat_scope_get",
    "persona.chat_scope_set",
];

/// Message when another persona is to get Bible evidence from SWORD.
pub const BIBLE_REFUSED: &str = "Bibel-Belege aus SWORD gibt es nur für @belege und den Lektor.";

/// Message when another persona is to get hits of the semantic search.
pub const RAG_REFUSED: &str = "Bedeutungs-Treffer gibt es nur für @belege und den Lektor.";

/// Text in the light theme (`--text` in app.css) and the surface it is mixed with.
pub const LIGHT_TEXT: &str = "#1d2125";
pub const LIGHT_PANEL: &str = "#ffffff";
/// Sheet (typeset view, preview) in the dark theme (`--sheet` in app.css): stays light, but is darker
/// than [`LIGHT_PANEL`]. On the sheet `--pc-line` always applies, so the line must hold 3:1 here too.
pub const DARK_SHEET: &str = "#f4f1ea";
/// All light surfaces on which `--pc-line` stands: the light theme and the sheet in both themes
/// (in the light theme `--sheet` = [`LIGHT_PANEL`]).
pub const LIGHT_SURFACES: [&str; 2] = [LIGHT_PANEL, DARK_SHEET];
/// Muted text (`--muted`) and link text (`--primary`) in the light theme; both also stand on
/// the tints (handle and „Vorgabe #…“ on /einstellungen/personas, „Quelle: …“ and references in the
/// chat).
pub const LIGHT_MUTED: &str = "#5f6b76";
pub const LIGHT_PRIMARY: &str = "#3b5bdb";
/// Text in the dark theme and the surface (`--panel`) it is mixed with.
pub const DARK_TEXT: &str = "#e3e6ea";
pub const DARK_PANEL: &str = "#1f2328";
/// Muted text and link text in the dark theme.
pub const DARK_MUTED: &str = "#9aa5b1";
pub const DARK_PRIMARY: &str = "#7d97ff";
/// All font colours that stand on a light or dark tint; each must reach
/// [`MIN_CONTRAST`].
pub const LIGHT_FOREGROUNDS: [&str; 3] = [LIGHT_TEXT, LIGHT_MUTED, LIGHT_PRIMARY];
pub const DARK_FOREGROUNDS: [&str; 3] = [DARK_TEXT, DARK_MUTED, DARK_PRIMARY];
/// Minimum contrast for text (WCAG 2.x, AA).
pub const MIN_CONTRAST: f64 = 4.5;
/// Minimum contrast for lines and controls against their surface (WCAG 2.1, 1.4.11).
pub const MIN_LINE_CONTRAST: f64 = 3.0;

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method) || crate::assist::rpc::METHODS.contains(&method)
}

/// Tints of a persona colour: background light or dark (text `--text`, muted text
/// `--muted` and link text `--primary` on it at least 4.5:1) and the line per theme: the colour
/// itself if it reaches at least [`MIN_LINE_CONTRAST`] against the surface, otherwise darkened (light)
/// or lightened (dark) until it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tints {
    pub light: String,
    pub dark: String,
    /// Line in the light theme and on the sheet (against every surface in [`LIGHT_SURFACES`]).
    pub line: String,
    /// Line in the dark theme (against [`DARK_PANEL`]).
    pub line_dark: String,
}

fn rgb(color: &str) -> [u8; 3] {
    let hex = valid_color(color).unwrap_or_else(|| baruch_store::FALLBACK_COLOR.to_owned());
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0);
    [byte(1), byte(3), byte(5)]
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// `color` mixed with `with` by `amount` (0–1).
fn mix(color: [u8; 3], with: [u8; 3], amount: f64) -> [u8; 3] {
    let channel =
        |a: u8, b: u8| (f64::from(a) * (1.0 - amount) + f64::from(b) * amount).round().clamp(0.0, 255.0) as u8;
    [channel(color[0], with[0]), channel(color[1], with[1]), channel(color[2], with[2])]
}

/// Relative luminance according to WCAG 2.x.
fn luminance(color: [u8; 3]) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])
}

/// Contrast ratio of two colours according to WCAG 2.x (1 to 21). Invalid colours count as
/// `FALLBACK_COLOR`.
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(rgb(a)), luminance(rgb(b)));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Mixes `color` with `panel`, beginning at `start`, in steps of 5 % further towards the surface until
/// every font colour in `foregrounds` reaches at least [`MIN_CONTRAST`] on it (at the latest the
/// surface itself).
fn tint(color: [u8; 3], panel: [u8; 3], foregrounds: &[&str], start: f64) -> String {
    let mut amount = start;
    loop {
        let candidate = hex(mix(color, panel, amount));
        if amount >= 1.0 || foregrounds.iter().all(|fg| contrast_ratio(fg, &candidate) >= MIN_CONTRAST) {
            return candidate;
        }
        amount = (amount + 0.05).min(1.0);
    }
}

/// Line: `color` mixed in steps of 5 % towards `towards` (black or white) until it reaches at least
/// [`MIN_LINE_CONTRAST`] against every surface in `panels`; unchanged if it already does.
fn line(color: [u8; 3], panels: &[&str], towards: [u8; 3]) -> String {
    let mut amount: f64 = 0.0;
    loop {
        let candidate = hex(mix(color, towards, amount));
        if amount >= 1.0 || panels.iter().all(|panel| contrast_ratio(&candidate, panel) >= MIN_LINE_CONTRAST) {
            return candidate;
        }
        amount = (amount + 0.05).min(1.0);
    }
}

/// Tints for the light theme (mixture with `#ffffff`) and the dark theme (mixture with `#1f2328`).
pub fn tints(color: &str) -> Tints {
    let base = rgb(color);
    Tints {
        light: tint(base, rgb(LIGHT_PANEL), &LIGHT_FOREGROUNDS, 0.82),
        dark: tint(base, rgb(DARK_PANEL), &DARK_FOREGROUNDS, 0.70),
        line: line(base, &LIGHT_SURFACES, [0, 0, 0]),
        line_dark: line(base, &[DARK_PANEL], [255, 255, 255]),
    }
}

/// `style` value with the CSS variables of a colour (only `#rrggbb`, therefore safe without escaping):
/// `--pc-light`, `--pc-dark` (backgrounds), `--pc-line` (line in the light theme and on the always
/// light sheet) and `--pc-line-dark` (line in the dark theme).
pub fn style(color: &str) -> String {
    let t = tints(color);
    format!("--pc-light:{};--pc-dark:{};--pc-line:{};--pc-line-dark:{}", t.light, t.dark, t.line, t.line_dark)
}

/// Lector by the choice (Round 10): the fixed choice if the persona exists and is active, otherwise the
/// active persona with the largest context, on a tie the earlier one in the order `position`,
/// `id` – the same rule as `baruch_core::Work::lector` ([`LectorChoice::pick`]).
pub fn lector_of<'a>(rows: &'a [PersonaRow], choice: &LectorChoice) -> Option<&'a PersonaRow> {
    choice.pick(rows, |p| p.handle.as_str(), |p| p.context_length, |p| p.active).map(|i| &rows[i])
}

/// Lector by the choice with the **effective** context length (Round 16a): like [`lector_of`], but „automatisch
/// nach Kontext“ (and a fixed, inactive persona that falls back to it) compares `state.model.context` – the
/// setting of the persona, reconciled with what the remote side reports. A fixed, active choice needs no
/// query.
pub async fn effective_lector_of(state: &AppState, rows: &[PersonaRow], choice: &LectorChoice) -> Option<String> {
    lector_by(state, rows, choice, false).await
}

/// Like [`effective_lector_of`], but without a network query (Round 17a, L4): `ModelPort::context_cached` – what
/// the port already knows, otherwise the setting. For pages and lists.
pub async fn cached_lector_of(state: &AppState, rows: &[PersonaRow], choice: &LectorChoice) -> Option<String> {
    lector_by(state, rows, choice, true).await
}

async fn lector_by(state: &AppState, rows: &[PersonaRow], choice: &LectorChoice, cached: bool) -> Option<String> {
    if let LectorChoice::Fixed(_) = choice
        && let Some(fixed) = lector_of(rows, choice).filter(|p| choice.handle().is_some_and(|h| same_handle(h, p)))
    {
        return Some(fixed.handle.clone());
    }
    let mut sized: Vec<(&PersonaRow, u32)> = Vec::with_capacity(rows.len());
    for row in rows {
        let tokens = match (row.active, cached) {
            (false, _) => 0,
            (true, false) => state.model.context(state, row).await.tokens,
            (true, true) => state.model.context_cached(state, row).await.tokens,
        };
        sized.push((row, tokens));
    }
    choice
        .pick(&sized, |(p, _)| p.handle.as_str(), |(_, tokens)| *tokens, |(p, _)| p.active)
        .map(|i| sized[i].0.handle.clone())
}

/// Is `handle` (with or without `@`) the handle of `row`?
fn same_handle(handle: &str, row: &PersonaRow) -> bool {
    let wanted = handle.trim();
    row.handle == wanted || row.handle.trim_start_matches('@') == wanted.trim_start_matches('@')
}

/// Personas with the choice and the lector by the stored choice (since Round 16a with the effective context length,
/// [`effective_lector_of`]).
pub async fn rows_and_lector(state: &AppState) -> Result<(Vec<PersonaRow>, LectorChoice, Option<String>), RpcError> {
    let rows = state.store.persona_list().await?;
    let choice = state.store.lector_choice().await?;
    let lector = effective_lector_of(state, &rows, &choice).await;
    Ok((rows, choice, lector))
}

/// Like [`rows_and_lector`], without a network query ([`cached_lector_of`]): for pages, lists and switches.
pub async fn rows_and_lector_cached(
    state: &AppState,
) -> Result<(Vec<PersonaRow>, LectorChoice, Option<String>), RpcError> {
    let rows = state.store.persona_list().await?;
    let choice = state.store.lector_choice().await?;
    let lector = cached_lector_of(state, &rows, &choice).await;
    Ok((rows, choice, lector))
}

/// `persona.chat_scope_get` or the answer of `persona.chat_scope_set` (Round 16a).
pub(crate) async fn chat_scope_json(state: &AppState) -> Result<Json, RpcError> {
    Ok(json!({ "scope": state.store.chat_scope().await?.word() }))
}

/// `scope` from `chapter` | `section`; otherwise -32602.
pub(crate) fn chat_scope_from(value: Option<&str>) -> Result<baruch_store::ChatScope, RpcError> {
    value.and_then(baruch_store::ChatScope::parse).ok_or_else(|| {
        RpcError::new(INVALID_PARAMS, "scope: „chapter“ (ganzes Kapitel) oder „section“ (nur Abschnitt)")
    })
}

/// Personas with role, model, context, active, lector and colour, everything from the table `persona`
/// (Round 9: no longer from the default work, therefore complete even without works).
async fn list(state: &AppState) -> Result<Json, RpcError> {
    let (rows, choice, lector) = rows_and_lector_cached(state).await?;
    let backends = state.store.persona_backends().await?;
    let connections = state.store.model_connection_list().await?;
    let mut personas: Vec<Json> = Vec::with_capacity(rows.len());
    let mut lector_context: Option<crate::assist::port::ContextBudget> = None;
    for p in &rows {
        let backend = backends.iter().find(|(h, _)| *h == p.handle).map(|(_, b)| *b).unwrap_or_default();
        let connection = connections.iter().find(|c| c.id == backend.connection).or(connections.first());
        // Context without a network query: what the port already knows (cache), otherwise the setting.
        let budget = state.model.context_cached(state, p).await;
        if lector.as_deref() == Some(p.handle.as_str()) {
            lector_context = Some(budget.clone());
        }
        let context = json!(budget);
        personas.push(json!({
            "handle": p.handle, "name": p.name, "role": p.role, "model": p.model,
            "context_length": p.context_length, "active": p.active,
            "lector": lector.as_deref() == Some(p.handle.as_str()),
            "color": p.color, "chosen": p.chosen, "tints": tints(&p.color),
            "prompt_custom": p.prompt.is_some(),
            "connection": connection.map(|c| json!({ "id": c.id, "name": c.name, "kind": c.kind })),
            "locality": connection.map_or("lokal", crate::models::locality),
            "external_allowed": backend.external_allowed,
            "context": context,
        }));
    }
    let chat_scope = state.store.chat_scope().await?.word();
    Ok(json!({
        "personas": personas, "palette": PERSONA_PALETTE, "lector": lector, "lector_mode": choice.mode(),
        "lector_context": lector_context, "chat_scope": chat_scope,
    }))
}

/// Connection of a persona as JSON (`persona.backend_get`): connection, local/external, release and whether a
/// job may go out this way (`usable`, otherwise `warning`).
pub(crate) async fn backend_json(state: &AppState, handle: &str) -> Result<Json, RpcError> {
    let row = row_of(state, handle).await?;
    let backend = state.store.persona_backend(&row.handle).await?;
    let connection = state.store.model_connection_get(backend.connection).await?;
    let usable = !connection.external || backend.external_allowed;
    Ok(json!({
        "persona": row.handle,
        "connection": { "id": connection.id, "name": connection.name, "kind": connection.kind,
                        "kind_title": crate::models::kind_title(&connection.kind), "external": connection.external },
        "locality": crate::models::locality(&connection),
        "external_allowed": backend.external_allowed, "explicit": backend.explicit, "usable": usable,
        "warning": (!usable).then(|| crate::models::not_allowed(&connection, &row.handle)),
    }))
}

/// `persona.backend_set`: set the connection (id) and the release „extern“. Only the human.
pub(crate) async fn set_backend(
    state: &AppState,
    handle: &str,
    connection: i64,
    external_allowed: bool,
) -> Result<Json, RpcError> {
    let row = row_of(state, handle).await?;
    state.store.set_persona_backend(&row.handle, connection, external_allowed).await?;
    backend_json(state, &row.handle).await
}

/// Default of the switch „Bibel-Belege“: `@belege` on, otherwise (the lector) off.
pub fn bible_default(handle: &str) -> bool {
    handle.trim().trim_start_matches('@').eq_ignore_ascii_case(crate::assist::rag_facts::BELEGE.trim_start_matches('@'))
}

/// Persona, whether it may get Bible evidence (`@belege` or the lector by the choice), whether its switch is on
/// (without a row the default [`bible_default`]) and the default.
pub(crate) async fn bible_state(state: &AppState, handle: &str) -> Result<(PersonaRow, bool, bool, bool), RpcError> {
    let row = row_of(state, handle).await?;
    let (_, _, lector) = rows_and_lector_cached(state).await?;
    let allowed = crate::assist::rag_facts::allowed(&row.handle, lector.as_deref());
    let default = bible_default(&row.handle);
    let on = state.store.persona_bible(&row.handle).await?.unwrap_or(default);
    Ok((row, allowed, on, default))
}

/// `persona.bible_set`: only for allowed personas, otherwise -32602 [`BIBLE_REFUSED`].
pub(crate) async fn set_bible(state: &AppState, handle: &str, on: bool) -> Result<(PersonaRow, bool), RpcError> {
    let (row, allowed, ..) = bible_state(state, handle).await?;
    if !allowed {
        return Err(RpcError::new(INVALID_PARAMS, BIBLE_REFUSED));
    }
    let on = state.store.set_persona_bible(&row.handle, on).await?;
    Ok((row, on))
}

fn bool_param(params: &Json, name: &str) -> Result<bool, RpcError> {
    match params.get(name) {
        Some(Json::Bool(b)) => Ok(*b),
        _ => Err(RpcError::new(INVALID_PARAMS, format!("{name}: true oder false erwartet"))),
    }
}

/// `persona.prompt_get` or the answer of `persona.set_prompt`.
pub(crate) fn prompt_json(row: &PersonaRow) -> Json {
    let default = crate::assist::prompts::default_prompt(&row.handle, &row.role);
    let prompt = crate::assist::prompts::effective_prompt(&row.handle, &row.role, row.prompt.as_deref());
    json!({ "handle": row.handle, "prompt": prompt, "default": default, "custom": row.prompt.is_some() })
}

/// Persona by handle (with or without `@`) from the table; unknown → -32602.
async fn row_of(state: &AppState, handle: &str) -> Result<PersonaRow, RpcError> {
    let want = handle.trim().trim_start_matches('@');
    state
        .store
        .persona_list()
        .await?
        .into_iter()
        .find(|p| p.handle.trim_start_matches('@') == want)
        .ok_or_else(|| baruch_store::StoreError::UnknownPersona(handle.trim().to_owned()).into())
}

/// Persona, whether it may get hits of the semantic search (`@belege` or the lector by the choice) and whether
/// its switch is on. Unknown → -32602.
pub(crate) async fn rag_state(state: &AppState, handle: &str) -> Result<(PersonaRow, bool, bool), RpcError> {
    let row = row_of(state, handle).await?;
    let (_, _, lector) = rows_and_lector_cached(state).await?;
    let allowed = crate::assist::rag_facts::allowed(&row.handle, lector.as_deref());
    let on = state.store.persona_rag(&row.handle).await?;
    Ok((row, allowed, on))
}

/// `persona.rag_set`: only for allowed personas, otherwise -32602 [`RAG_REFUSED`].
pub(crate) async fn set_rag(state: &AppState, handle: &str, on: bool) -> Result<(PersonaRow, bool), RpcError> {
    let (row, allowed, _) = rag_state(state, handle).await?;
    if !allowed {
        return Err(RpcError::new(INVALID_PARAMS, RAG_REFUSED));
    }
    let on = state.store.set_persona_rag(&row.handle, on).await?;
    Ok((row, on))
}

fn persona_param(params: &Json) -> Result<String, RpcError> {
    match params.get("persona").or_else(|| params.get("handle")) {
        Some(Json::String(h)) if !h.trim().is_empty() => Ok(h.trim().to_owned()),
        _ => Err(RpcError::new(INVALID_PARAMS, "persona fehlt (z. B. „@belege“)")),
    }
}

/// `persona.set_prompt`: `None` = default.
pub(crate) async fn set_prompt(state: &AppState, handle: &str, prompt: Option<&str>) -> Result<PersonaRow, RpcError> {
    Ok(state.store.set_persona_prompt(handle, prompt).await?)
}

/// Lector choice as JSON (`persona.lector_get`).
pub(crate) async fn lector_json(state: &AppState) -> Result<Json, RpcError> {
    let (_, choice, lector) = rows_and_lector_cached(state).await?;
    Ok(json!({ "mode": choice.mode(), "handle": choice.handle(), "lector": lector }))
}

/// Choice from `mode` (`fixed`/`auto`) and `handle`; invalid → -32602.
pub(crate) fn lector_choice_from(mode: Option<&str>, handle: Option<&str>) -> Result<LectorChoice, RpcError> {
    match mode.map(str::trim) {
        Some("auto") => Ok(LectorChoice::Auto),
        Some("fixed") => match handle.map(str::trim).filter(|h| !h.is_empty()) {
            Some(handle) => Ok(LectorChoice::Fixed(handle.to_owned())),
            None => Err(RpcError::new(INVALID_PARAMS, "handle fehlt: welche Persona ist fest Lektor?")),
        },
        _ => Err(RpcError::new(INVALID_PARAMS, "mode: „fixed“ oder „auto“ erwartet")),
    }
}

/// Parameters of `persona.update`: `handle` mandatory, otherwise only named fields; limits by
/// [`PersonaEdit::check`] (-32602 with a German message).
pub(crate) fn update_params(params: &Json) -> Result<Update, RpcError> {
    let handle = match params.get("handle") {
        Some(Json::String(h)) if !h.trim().is_empty() => h.trim().to_owned(),
        _ => return Err(RpcError::new(INVALID_PARAMS, "handle fehlt (z. B. „@lektor“)")),
    };
    let text = |name: &str| -> Result<Option<String>, RpcError> {
        match params.get(name) {
            None | Some(Json::Null) => Ok(None),
            Some(Json::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(RpcError::new(INVALID_PARAMS, format!("{name}: Text erwartet"))),
        }
    };
    let context_length = match params.get("context_length") {
        None | Some(Json::Null) => None,
        Some(value) => match value.as_u64().and_then(|n| u32::try_from(n).ok()) {
            Some(n) => Some(n),
            None => {
                return Err(RpcError::new(
                    INVALID_PARAMS,
                    "Die Kontextlänge muss zwischen 1 und 10000000 Tokens liegen.",
                ));
            }
        },
    };
    let active = match params.get("active") {
        None | Some(Json::Null) => None,
        Some(Json::Bool(b)) => Some(*b),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "active: true oder false erwartet")),
    };
    let name = text("name")?
        .map(|n| PersonaEdit::check_name(&n).map_err(|why| RpcError::new(INVALID_PARAMS, why)))
        .transpose()?;
    let edit = PersonaEdit { role: text("role")?, model: text("model")?, context_length, active };
    edit.check().map_err(|why| RpcError::new(INVALID_PARAMS, why))?;
    Ok(Update { handle, name, edit })
}

/// Checked parameters of `persona.update`: handle, new name (Round 11, 1–80 characters) and the
/// other fields.
pub(crate) struct Update {
    pub handle: String,
    pub name: Option<String>,
    pub edit: PersonaEdit,
}

/// `persona.update` (checked) → the stored persona; with `name` (Round 11) in the same
/// transaction ([`baruch_store::Store::save_persona`]).
pub(crate) async fn update(state: &AppState, update: &Update) -> Result<PersonaRow, RpcError> {
    update.edit.check().map_err(|why| RpcError::new(INVALID_PARAMS, why))?;
    if let Some(name) = &update.name {
        PersonaEdit::check_name(name).map_err(|why| RpcError::new(INVALID_PARAMS, why))?;
    }
    Ok(state.store.save_persona(&update.handle, update.name.as_deref(), &update.edit, None, None).await?)
}

/// `color` from the parameters: text or `null` (missing → error, so that it is not reset
/// by accident).
fn color_param(params: &Json) -> Result<Option<String>, RpcError> {
    match params.get("color") {
        Some(Json::Null) => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        _ => Err(RpcError::new(INVALID_PARAMS, "color: \"#rrggbb\" oder null erwartet")),
    }
}

pub(crate) async fn set_color(state: &AppState, handle: &str, color: Option<&str>) -> Result<PersonaColor, RpcError> {
    Ok(state.store.set_persona_color(handle, color).await?)
}

/// `color` for `settings.set` (page „assistenten“): missing → `None` (change nothing), otherwise like
/// `persona.set_color` (`null` = default).
pub(crate) fn optional_color(params: &Json) -> Result<Option<Option<String>>, RpcError> {
    if params.get("color").is_none() { Ok(None) } else { color_param(params).map(Some) }
}

fn handle_param(params: &Json) -> Result<String, RpcError> {
    match params.get("handle") {
        Some(Json::String(h)) if !h.trim().is_empty() => Ok(h.trim().to_owned()),
        _ => Err(RpcError::new(INVALID_PARAMS, "handle fehlt (z. B. „@lektor“)")),
    }
}

pub async fn call(state: &AppState, method: &str, params: Json) -> Result<Json, RpcError> {
    match method {
        "persona.list" => list(state).await,
        "persona.set_color" => {
            let handle = match params.get("handle") {
                Some(Json::String(h)) if !h.trim().is_empty() => h.clone(),
                _ => return Err(RpcError::new(INVALID_PARAMS, "handle fehlt (z. B. „@lektor“)")),
            };
            let color = color_param(&params)?;
            let set = set_color(state, &handle, color.as_deref()).await?;
            Ok(json!({ "handle": set.handle, "color": set.color, "chosen": set.chosen }))
        }
        "persona.update" => {
            let p = update_params(&params)?;
            Ok(json!(update(state, &p).await?))
        }
        "persona.prompt_get" => {
            let handle = handle_param(&params)?;
            Ok(prompt_json(&row_of(state, &handle).await?))
        }
        "persona.set_prompt" => {
            let handle = handle_param(&params)?;
            let prompt = match params.get("prompt") {
                Some(Json::Null) => None,
                Some(Json::String(s)) => Some(s.clone()),
                _ => return Err(RpcError::new(INVALID_PARAMS, "prompt: Text oder null (Vorgabe) erwartet")),
            };
            Ok(prompt_json(&set_prompt(state, &handle, prompt.as_deref()).await?))
        }
        "persona.lector_get" => lector_json(state).await,
        "persona.set_lector" => {
            let choice = lector_choice_from(
                params.get("mode").and_then(Json::as_str),
                params.get("handle").and_then(Json::as_str),
            )?;
            state.store.set_lector_choice(&choice).await?;
            lector_json(state).await
        }
        "persona.chat_scope_get" => chat_scope_json(state).await,
        "persona.chat_scope_set" => {
            let scope = chat_scope_from(params.get("scope").and_then(Json::as_str))?;
            state.store.set_chat_scope(scope).await?;
            chat_scope_json(state).await
        }
        "persona.rag_get" => {
            let (row, allowed, on) = rag_state(state, &persona_param(&params)?).await?;
            Ok(json!({ "persona": row.handle, "rag_context": on, "allowed": allowed }))
        }
        "persona.rag_set" => {
            let handle = persona_param(&params)?;
            let on = match params.get("on") {
                Some(Json::Bool(on)) => *on,
                _ => return Err(RpcError::new(INVALID_PARAMS, "on: true oder false erwartet")),
            };
            let (row, on) = set_rag(state, &handle, on).await?;
            Ok(json!({ "persona": row.handle, "rag_context": on }))
        }
        "persona.backend_get" => backend_json(state, &persona_param(&params)?).await,
        "persona.backend_set" => {
            let handle = persona_param(&params)?;
            let connection = match params.get("connection") {
                Some(Json::Number(n)) => n.as_i64(),
                Some(Json::String(s)) => s.trim().parse().ok(),
                _ => None,
            }
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, "connection: id der Modell-Verbindung erwartet"))?;
            let allowed = match params.get("external_allowed") {
                None | Some(Json::Null) => false,
                Some(_) => bool_param(&params, "external_allowed")?,
            };
            set_backend(state, &handle, connection, allowed).await
        }
        "persona.bible_get" => {
            let (row, allowed, on, default) = bible_state(state, &persona_param(&params)?).await?;
            Ok(json!({ "persona": row.handle, "bible_context": on, "allowed": allowed, "default": default }))
        }
        "persona.bible_set" => {
            let handle = persona_param(&params)?;
            let on = bool_param(&params, "on")?;
            let (row, on) = set_bible(state, &handle, on).await?;
            Ok(json!({ "persona": row.handle, "bible_context": on }))
        }
        m if crate::assist::rpc::METHODS.contains(&m) => crate::assist::rpc::call(state, m, params).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

// ---- Page ----------------------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct PageQuery {
    /// Work with which the settings are opened.
    work: Option<i64>,
}

/// `GET /einstellungen/personas[?work=<id>]` → 303 `/einstellungen/assistenten[?work=<id>]` (Round 9:
/// colours, role, model, context and active are now on the settings page „Assistenten“).
async fn personas_page(Query(q): Query<PageQuery>) -> Response {
    let work = q.work.filter(|&w| w > 0).map_or(String::new(), |w| format!("?work={w}"));
    Redirect::to(&format!("/einstellungen/assistenten{work}")).into_response()
}

#[derive(Deserialize, Default)]
struct ActForm {
    method: Option<String>,
    handle: Option<String>,
    /// Colour picker (`<input type="color">`).
    color: Option<String>,
    /// Text field; not empty → takes precedence over the colour picker.
    color_text: Option<String>,
    /// `1`: back to the default.
    reset: Option<String>,
    work: Option<String>,
    /// `persona.update` (Round 9): role, model, context length (tokens), checkbox active (`1`/`on`;
    /// if it is missing, the persona is inactive).
    role: Option<String>,
    model: Option<String>,
    context_length: Option<String>,
    active: Option<String>,
    /// `persona.set_prompt` (Round 10): own role prompt; `reset=1` = default.
    prompt: Option<String>,
    /// `persona.set_lector` (Round 10): `fixed` or `auto`, plus `handle`.
    mode: Option<String>,
    /// Round 11 (tabs per persona): `save` (all fields at once) or `reset_prompt` (with
    /// `confirm=1`, otherwise a confirmation question). Takes precedence over `method`.
    action: Option<String>,
    /// Name of the persona (`action=save`).
    name: Option<String>,
    /// `1`: reset confirmed (`action=reset_prompt`).
    confirm: Option<String>,
    /// Round 12 (`action=rag`): checkbox „Treffer der Bedeutungssuche als Belege mitgeben“ (`1`/`on`; if it is
    /// missing, the switch is off).
    on: Option<String>,
    /// Round 15b (`action=backend`): id of the model connection and checkbox „extern freigeben“.
    connection: Option<String>,
    external_allowed: Option<String>,
    /// Round 16a (`method=persona.chat_scope_set`): `chapter` or `section`.
    scope: Option<String>,
}

/// Input of a refused form „Speichern“ (Round 11): the page shows the tab of the persona with
/// the input and the message again instead of losing them.
#[derive(Debug, Clone, Default)]
pub(crate) struct PersonaRetry {
    pub handle: String,
    pub name: String,
    pub role: String,
    pub model: String,
    pub context_length: String,
    pub active: bool,
    pub color: String,
    pub prompt: String,
}

tokio::task_local! {
    static RETRY: PersonaRetry;
}

/// Refused input of the running request (set only during the 422 page).
pub(crate) fn retry() -> Option<PersonaRetry> {
    RETRY.try_with(Clone::clone).ok()
}

/// A checkbox is ticked (`1`/`on`).
fn ticked(value: Option<&str>) -> bool {
    value.is_some_and(|v| matches!(v.trim(), "1" | "on"))
}

/// Address of the page „Assistenten“ with the tab of the persona: `?persona=<handle ohne @>[&ok=…][&work=W]`.
fn tab_url(handle: &str, extra: &str, work: Option<i64>) -> String {
    let back = work.map_or(String::new(), |w| format!("&work={w}"));
    let at = anchor(handle);
    format!("/einstellungen/assistenten?persona={at}{extra}{back}#persona-{at}")
}

/// `action=save`: name, colour, role, model, context, active and role prompt in **one**
/// transaction ([`baruch_store::Store::save_persona`]). A prompt equal to the default from the code
/// stays the default (no own prompt just by saving).
async fn save(state: &AppState, handle: &str, form: &ActForm) -> Result<(), RpcError> {
    let context_length = match form.context_length.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(text) => Some(text.parse::<u32>().map_err(|_| {
            RpcError::new(INVALID_PARAMS, "Die Kontextlänge muss zwischen 1 und 10000000 Tokens liegen.")
        })?),
    };
    let edit = PersonaEdit {
        role: form.role.clone(),
        model: form.model.clone(),
        context_length,
        active: Some(ticked(form.active.as_deref())),
    };
    edit.check().map_err(|why| RpcError::new(INVALID_PARAMS, why))?;
    let name =
        form.name.as_deref().map(|n| PersonaEdit::check_name(n).map_err(|why| RpcError::new(INVALID_PARAMS, why)));
    let name = name.transpose()?;
    let typed = form.color_text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let color = typed.or(form.color.as_deref().map(str::trim).filter(|c| !c.is_empty()));
    if let Some(c) = color
        && valid_color(c).is_none()
    {
        return Err(RpcError::new(INVALID_PARAMS, "Farbe als #rrggbb angeben."));
    }
    let row = row_of(state, handle).await?;
    // The colour picker always sends a value; only a change makes it an own colour.
    let color = color.and_then(valid_color).filter(|c| *c != row.color || row.chosen);
    let prompt = match form.prompt.as_deref() {
        None => None,
        Some(text) => {
            let text = text.trim().replace("\r\n", "\n");
            let default = crate::assist::prompts::default_prompt(&row.handle, &row.role);
            if row.prompt.is_none() && text == default.trim() { None } else { Some(text) }
        }
    };
    state
        .store
        .save_persona(handle, name.as_deref(), &edit, color.as_deref().map(Some), prompt.as_deref().map(Some))
        .await?;
    Ok(())
}

/// Anchor of the persona on the page „Assistenten“: `persona-<Handle ohne @>`, encoded for the address.
fn anchor(handle: &str) -> String {
    crate::view::query_escape(handle.trim().trim_start_matches('@'))
}

/// `POST /einstellungen/personas/act`: `method=persona.set_color` (fields `handle`, `color` or
/// `color_text`, `reset=1` → default) or `method=persona.update` (`handle`, `role`, `model`,
/// `context_length`, `active=1|leer`), plus `work`. Then 303 to
/// `/einstellungen/assistenten?ok=farbe|persona[&work=W]#persona-<handle>`; invalid values or an
/// unknown persona → 422 with the page and the message; other methods → 400.
async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work = form.work.as_deref().and_then(|w| w.trim().parse::<i64>().ok()).filter(|&w| w > 0);
    if let Some(action) = form.action.as_deref() {
        return tab_act(&state, &headers, work, action, &form).await;
    }
    let method = form.method.as_deref().unwrap_or("persona.set_color");
    if !["persona.set_color", "persona.update", "persona.set_prompt", "persona.set_lector", "persona.chat_scope_set"]
        .contains(&method)
    {
        return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response();
    }
    let handle = form.handle.as_deref().unwrap_or("").trim().to_owned();
    let reset = form.reset.as_deref().is_some_and(|r| matches!(r, "1" | "on"));
    let result: Result<&str, RpcError> = if method == "persona.chat_scope_set" {
        match chat_scope_from(form.scope.as_deref()) {
            Ok(scope) => state.store.set_chat_scope(scope).await.map(|_| "chat").map_err(RpcError::from),
            Err(e) => Err(e),
        }
    } else if method == "persona.set_lector" {
        match lector_choice_from(form.mode.as_deref(), Some(handle.as_str())) {
            Ok(choice) => state.store.set_lector_choice(&choice).await.map(|_| "lektor").map_err(RpcError::from),
            Err(e) => Err(e),
        }
    } else if handle.is_empty() {
        Err(RpcError::new(INVALID_PARAMS, "handle fehlt (z. B. „@lektor“)"))
    } else if method == "persona.set_prompt" {
        let prompt = if reset { None } else { Some(form.prompt.clone().unwrap_or_default()) };
        set_prompt(&state, &handle, prompt.as_deref()).await.map(|_| if reset { "vorgabe" } else { "prompt" })
    } else if method == "persona.set_color" {
        let color = if reset {
            None
        } else {
            let typed = form.color_text.as_deref().map(str::trim).filter(|t| !t.is_empty());
            Some(typed.or(form.color.as_deref()).unwrap_or("").to_owned())
        };
        set_color(&state, &handle, color.as_deref()).await.map(|_| "farbe")
    } else {
        let context_length = match form.context_length.as_deref().map(str::trim) {
            None | Some("") => Ok(None),
            Some(text) => text.parse::<u32>().map(Some).map_err(|_| {
                RpcError::new(INVALID_PARAMS, "Die Kontextlänge muss zwischen 1 und 10000000 Tokens liegen.")
            }),
        };
        match context_length {
            Ok(context_length) => {
                let edit = PersonaEdit {
                    role: form.role.clone(),
                    model: form.model.clone(),
                    context_length,
                    active: Some(form.active.as_deref().is_some_and(|a| matches!(a, "1" | "on"))),
                };
                let name = form.name.clone().filter(|n| !n.trim().is_empty());
                update(&state, &Update { handle: handle.clone(), name, edit }).await.map(|_| "persona")
            }
            Err(e) => Err(e),
        }
    };
    match result {
        Ok(code) => {
            let back = work.map_or(String::new(), |w| format!("&work={w}"));
            let at = match code {
                "lektor" => "lektor".to_owned(),
                "chat" => "chat".to_owned(),
                _ => format!("persona-{}", anchor(&handle)),
            };
            let to = format!("/einstellungen/assistenten?ok={code}{back}#{at}");
            Redirect::to(&to).into_response()
        }
        Err(e) if e.code == INTERNAL => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response()
        }
        Err(e) => crate::settings::error_page(&state, &headers, "assistenten", work, &e.message).await,
    }
}

/// Forms of the tabs (Round 11): `action=save` → 303 `…?persona=<h>&ok=persona#persona-<h>`, error
/// → 422 with the tab, the input and the message; `action=reset_prompt` without `confirm=1` → 303 to the
/// confirmation question (`&rueckfrage=vorgabe`), with `confirm=1` → default, `&ok=vorgabe`; `action=reset_color` →
/// colour back to the palette, `&ok=farbe`; `action=rag` (Round 12, checkbox `on`) → `persona.rag_set`,
/// `&ok=rag`, for personas other than `@belege` and the lector 422 with [`RAG_REFUSED`]; Round 15b:
/// `action=backend` (`connection`, checkbox `external_allowed`) → `persona.backend_set`, `&ok=verbindung`;
/// `action=bible` (checkbox `on`) → `persona.bible_set`, `&ok=bibel` (other personas 422 with
/// [`BIBLE_REFUSED`]). Others → 400.
async fn tab_act(state: &AppState, headers: &HeaderMap, work: Option<i64>, action: &str, form: &ActForm) -> Response {
    let handle = form.handle.as_deref().unwrap_or("").trim().to_owned();
    if handle.is_empty() {
        let message = "handle fehlt (z. B. „@lektor“)";
        return crate::settings::error_page(state, headers, "assistenten", work, message).await;
    }
    let result = match action {
        "save" => save(state, &handle, form).await.map(|()| "&ok=persona"),
        "reset_prompt" if ticked(form.confirm.as_deref()) => {
            set_prompt(state, &handle, None).await.map(|_| "&ok=vorgabe")
        }
        "reset_color" => set_color(state, &handle, None).await.map(|_| "&ok=farbe"),
        "rag" => set_rag(state, &handle, ticked(form.on.as_deref())).await.map(|_| "&ok=rag"),
        "bible" => set_bible(state, &handle, ticked(form.on.as_deref())).await.map(|_| "&ok=bibel"),
        "backend" => match form.connection.as_deref().and_then(|c| c.trim().parse::<i64>().ok()) {
            Some(connection) => set_backend(state, &handle, connection, ticked(form.external_allowed.as_deref()))
                .await
                .map(|_| "&ok=verbindung"),
            None => Err(RpcError::new(INVALID_PARAMS, "Bitte eine Modell-Verbindung wählen.")),
        },
        "reset_prompt" => match row_of(state, &handle).await {
            Ok(_) => Ok("&rueckfrage=vorgabe"),
            Err(e) => Err(e),
        },
        _ => return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response(),
    };
    match result {
        Ok(extra) => Redirect::to(&tab_url(&handle, extra, work)).into_response(),
        Err(e) if e.code == INTERNAL => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response()
        }
        Err(e) => {
            let retry = PersonaRetry {
                handle: handle.clone(),
                name: form.name.clone().unwrap_or_default(),
                role: form.role.clone().unwrap_or_default(),
                model: form.model.clone().unwrap_or_default(),
                context_length: form.context_length.clone().unwrap_or_default(),
                active: ticked(form.active.as_deref()),
                color: form
                    .color_text
                    .clone()
                    .filter(|c| !c.trim().is_empty())
                    .or(form.color.clone())
                    .unwrap_or_default(),
                prompt: form.prompt.clone().unwrap_or_default(),
            };
            let page = crate::settings::error_page(state, headers, "assistenten", work, &e.message);
            if action == "save" { RETRY.scope(retry, page).await } else { page.await }
        }
    }
}

/// `GET /einstellungen/personas` (redirect, static segment before `/einstellungen/{seite}`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/einstellungen/personas", get(personas_page))
}

/// `POST /einstellungen/personas/act` (in the router of the writing routes, behind `guard::same_origin`).
/// Plus the persona jobs (Round 10): `POST /assistenten/act` and `POST /assistenten/stream`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/einstellungen/personas/act", post(act))
        .route("/assistenten/act", post(crate::assist::routes::act))
        .route("/assistenten/stream", post(crate::assist::routes::stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_follows_wcag() {
        let black_white = contrast_ratio("#000000", "#ffffff");
        assert!((black_white - 21.0).abs() < 1e-9, "Schwarz auf Weiß ist 21:1, nicht {black_white}");
        assert!((contrast_ratio("#777777", "#777777") - 1.0).abs() < 1e-9, "gleiche Farbe 1:1");
        assert!(
            (contrast_ratio("#ffffff", "#000000") - black_white).abs() < 1e-9,
            "die Reihenfolge der Farben spielt keine Rolle"
        );
    }

    #[test]
    fn the_style_carries_only_hex_values() {
        let s = style("#ABC");
        assert!(
            s.starts_with("--pc-light:#")
                && s.contains(";--pc-dark:#")
                && s.contains(";--pc-line:#")
                && s.contains(";--pc-line-dark:#aabbcc"),
            "#aabbcc reicht im dunklen Thema schon für 3:1 und bleibt dort unverändert: {s}"
        );
        assert!(!s.contains('"') && !s.contains('<'), "nichts zu maskieren: {s}");
        assert_eq!(tints("kaputt"), tints(baruch_store::FALLBACK_COLOR), "Ungültiges fällt auf das Grau zurück");
    }
}
