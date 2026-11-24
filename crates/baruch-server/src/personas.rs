// SPDX-License-Identifier: AGPL-3.0-or-later
//! Personas (Vertrag Runde 7, „proposals“; Runde 9, Einstellungen „Assistenten“): RPC `persona.*`,
//! das Formular an `POST /einstellungen/personas/act` (ohne JavaScript bedienbar) und die Tönungen für
//! helles und dunkles Thema. Die Seite selbst ist seit Runde 9 die Einstellungsseite
//! `/einstellungen/assistenten` ([`crate::settings`]); `/einstellungen/personas` leitet dorthin.
//!
//! RPC ([`handles`], [`call`]; aus `rpc::call` vor der Wahl des Werks, Personas sind global):
//! - `persona.list {}` → `{personas: [{handle, name, role, model, context_length, active, lector,
//!   color, chosen, tints: {light, dark, line, line_dark}}], palette: [..]}`; alles aus der Tabelle
//!   `persona` (Runde 9, auch ohne Werk vollständig)
//! - `persona.set_color {handle, color: "#rrggbb" | null}` → `{handle, color, chosen}`; `null` = zurück
//!   zur Vorgabe. Unbekannte Persona oder ungültige Farbe → -32602. Nur der Mensch: nicht über `/act`
//!   (dort nicht erlaubt) und nicht über MCP (kein Werkzeug). Kein Commit im Spiegel (kein Buchtext).
//! - `persona.update {handle, name?, role?, model?, context_length?, active?}` → `PersonaRow` (Runde 9;
//!   Runde 11: `name` getrimmt 1–80 Zeichen, alles in einer Transaktion):
//!   Rolle und Modell getrimmt 1–200 Zeichen, Kontextlänge 1–10000000 Tokens; sonst bzw. bei
//!   unbekannter Persona -32602. Nur der Mensch, kein Commit. Modell und Kontextlänge nutzen die
//!   Persona-Aufträge (Runde 10, [`crate::assist`]).
//! - `persona.prompt_get {handle}` → `{handle, prompt, default, custom}` (Runde 10): wirksamer
//!   Rollen-Prompt, die Vorgabe aus dem Code und ob ein eigener gespeichert ist.
//! - `persona.set_prompt {handle, prompt: Text | null}` → dito; `null` = zurück zur Vorgabe, eigener
//!   Prompt getrimmt 1–8000 Zeichen (sonst -32000). Nur der Mensch.
//! - `persona.lector_get {}` → `{mode: "fixed"|"auto", handle?, lector}`; `persona.set_lector {mode,
//!   handle?}` → dito (Runde 10): fest eine Persona (muss es geben) oder automatisch nach Kontext.
//!   Vorgabe des Versuchs seit 2026-11-06: fest `@lektor`. Nur der Mensch.
//! - `persona.ask/review/propose/chat`: Aufträge an Personas ([`crate::assist::rpc`]).
//! - `persona.rag_get {persona}` → `{persona, rag_context, allowed}`; `persona.rag_set {persona, on}` →
//!   `{persona, rag_context}` (Runde 12, Migration 0032): Treffer der Bedeutungssuche als Belege mitgeben.
//!   Erlaubt nur für `@belege` und die Persona, die nach der Wahl Lektor ist (fest bzw. automatisch), sonst
//!   -32602 [`RAG_REFUSED`]; unbekannte Persona -32602. Nur der Mensch (nicht über `/act`, kein MCP).
//! - Runde 15b: `persona.list` nennt je Persona zusätzlich `connection {id, name, kind}`, `locality`
//!   („lokal“/„extern“), `external_allowed` und `context {tokens, reported, warning}` (ohne Netzabfrage, aus
//!   den Zwischenspeichern der Modelle). `persona.backend_get {persona}` → `{persona, connection, locality,
//!   external_allowed, explicit, usable, warning?}`, `persona.backend_set {persona, connection,
//!   external_allowed}` → dito (Store `persona_backend`, Migration 0036). `persona.bible_get {persona}` →
//!   `{persona, bible_context, allowed, default}`, `persona.bible_set {persona, on}` → `{persona,
//!   bible_context}` (Migration 0037): Bibel-Belege aus SWORD ([`crate::bible_research`]) nur für `@belege`
//!   (Vorgabe an) und den Lektor (Vorgabe aus), sonst -32602 [`BIBLE_REFUSED`]. Nur der Mensch.
//! - Runde 16a: `persona.chat_scope_get {}` → `{scope: "chapter"|"section"}`, `persona.chat_scope_set {scope}` →
//!   dito (Einstellung „Chat nur Abschnitt“, `store_meta.chat_scope`, nur SQLite, kein Spiegel). Nur der Mensch:
//!   nicht über `/act`, nicht über MCP; Formular `method=persona.chat_scope_set` an
//!   `POST /einstellungen/personas/act` → 303 `…?ok=chat#chat`. `persona.list` nennt zusätzlich `chat_scope`;
//!   „Lektor automatisch nach Kontext“ vergleicht die wirksame Kontextlänge ([`effective_lector_of`]).
//! - Runde 17a (L4): Seiten und Listen (`persona.list`, `persona.lector_get`, Bibel- und RAG-Schalter, Einstellungen →
//!   Assistenten, Arbeitsseite) bestimmen den Lektor ohne Netzabfrage ([`rows_and_lector_cached`],
//!   `ModelPort::context_cached`: Zwischenspeicher, sonst die Einstellung); nur der Modellaufruf fragt die
//!   Gegenstelle ([`rows_and_lector`]). `persona.list` nennt zusätzlich `lector_context` (wirksame Kontextlänge des
//!   Lektors: `{tokens, reported, warning}` oder `null`).
//!
//! Farben gelangen nur als geprüfte `#rrggbb` (`baruch_store::valid_color`) in die Seiten, als
//! CSS-Variablen `--pc-light`, `--pc-dark`, `--pc-line`, `--pc-line-dark` im `style`-Attribut
//! ([`style`]). Die Linien erreichen seit Runde 8 mindestens 3:1 gegen die Fläche ihres Themas
//! (WCAG 1.4.11, Nicht-Text-Kontrast), damit eine eingeklappte Linie auch bei Gelb oder Ocker sichtbar
//! bleibt.

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

/// Meldung, wenn eine andere Persona Bibel-Belege aus SWORD bekommen soll.
pub const BIBLE_REFUSED: &str = "Bibel-Belege aus SWORD gibt es nur für @belege und den Lektor.";

/// Meldung, wenn eine andere Persona Treffer der Bedeutungssuche bekommen soll.
pub const RAG_REFUSED: &str = "Bedeutungs-Treffer gibt es nur für @belege und den Lektor.";

/// Text im hellen Thema (`--text` in app.css) und Fläche, mit der gemischt wird.
pub const LIGHT_TEXT: &str = "#1d2125";
pub const LIGHT_PANEL: &str = "#ffffff";
/// Blatt (Satzbild, Vorschau) im dunklen Thema (`--sheet` in app.css): bleibt hell, ist aber dunkler
/// als [`LIGHT_PANEL`]. Auf dem Blatt gilt immer `--pc-line`, darum muss die Linie auch hier 3:1 halten.
pub const DARK_SHEET: &str = "#f4f1ea";
/// Alle hellen Flächen, auf denen `--pc-line` steht: das helle Thema und das Blatt in beiden Themen
/// (im hellen Thema ist `--sheet` = [`LIGHT_PANEL`]).
pub const LIGHT_SURFACES: [&str; 2] = [LIGHT_PANEL, DARK_SHEET];
/// Gedämpfter Text (`--muted`) und Linktext (`--primary`) im hellen Thema; beide stehen ebenfalls auf
/// den Tönungen (Handle und „Vorgabe #…“ auf /einstellungen/personas, „Quelle: …“ und Verweise im
/// Chat).
pub const LIGHT_MUTED: &str = "#5f6b76";
pub const LIGHT_PRIMARY: &str = "#3b5bdb";
/// Text im dunklen Thema und Fläche (`--panel`), mit der gemischt wird.
pub const DARK_TEXT: &str = "#e3e6ea";
pub const DARK_PANEL: &str = "#1f2328";
/// Gedämpfter Text und Linktext im dunklen Thema.
pub const DARK_MUTED: &str = "#9aa5b1";
pub const DARK_PRIMARY: &str = "#7d97ff";
/// Alle Schriftfarben, die auf einer hellen bzw. dunklen Tönung stehen; jede muss
/// [`MIN_CONTRAST`] erreichen.
pub const LIGHT_FOREGROUNDS: [&str; 3] = [LIGHT_TEXT, LIGHT_MUTED, LIGHT_PRIMARY];
pub const DARK_FOREGROUNDS: [&str; 3] = [DARK_TEXT, DARK_MUTED, DARK_PRIMARY];
/// Mindestkontrast für Text (WCAG 2.x, AA).
pub const MIN_CONTRAST: f64 = 4.5;
/// Mindestkontrast für Linien und Bedienelemente gegen ihre Fläche (WCAG 2.1, 1.4.11).
pub const MIN_LINE_CONTRAST: f64 = 3.0;

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method) || crate::assist::rpc::METHODS.contains(&method)
}

/// Tönungen einer Persona-Farbe: Hintergrund hell bzw. dunkel (Text `--text`, gedämpfter Text
/// `--muted` und Linktext `--primary` darauf mindestens 4,5:1) und die Linie je Thema: die Farbe
/// selbst, falls sie gegen die Fläche mindestens [`MIN_LINE_CONTRAST`] erreicht, sonst so weit
/// abgedunkelt (hell) bzw. aufgehellt (dunkel), bis sie es tut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tints {
    pub light: String,
    pub dark: String,
    /// Linie im hellen Thema und auf dem Blatt (gegen jede Fläche in [`LIGHT_SURFACES`]).
    pub line: String,
    /// Linie im dunklen Thema (gegen [`DARK_PANEL`]).
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

/// `color` zu `amount` (0–1) mit `with` gemischt.
fn mix(color: [u8; 3], with: [u8; 3], amount: f64) -> [u8; 3] {
    let channel =
        |a: u8, b: u8| (f64::from(a) * (1.0 - amount) + f64::from(b) * amount).round().clamp(0.0, 255.0) as u8;
    [channel(color[0], with[0]), channel(color[1], with[1]), channel(color[2], with[2])]
}

/// Relative Leuchtdichte nach WCAG 2.x.
fn luminance(color: [u8; 3]) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(color[0]) + 0.7152 * linear(color[1]) + 0.0722 * linear(color[2])
}

/// Kontrastverhältnis zweier Farben nach WCAG 2.x (1 bis 21). Ungültige Farben zählen als
/// `FALLBACK_COLOR`.
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(rgb(a)), luminance(rgb(b)));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Mischt `color` mit `panel`, beginnend bei `start`, in Schritten von 5 % weiter zur Fläche, bis
/// jede Schriftfarbe in `foregrounds` darauf mindestens [`MIN_CONTRAST`] erreicht (spätestens die
/// Fläche selbst).
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

/// Linie: `color` in Schritten von 5 % zu `towards` (Schwarz bzw. Weiß) gemischt, bis sie gegen
/// jede Fläche in `panels` mindestens [`MIN_LINE_CONTRAST`] erreicht; unverändert, wenn sie es schon tut.
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

/// Tönungen für helles (Mischung mit `#ffffff`) und dunkles Thema (Mischung mit `#1f2328`).
pub fn tints(color: &str) -> Tints {
    let base = rgb(color);
    Tints {
        light: tint(base, rgb(LIGHT_PANEL), &LIGHT_FOREGROUNDS, 0.82),
        dark: tint(base, rgb(DARK_PANEL), &DARK_FOREGROUNDS, 0.70),
        line: line(base, &LIGHT_SURFACES, [0, 0, 0]),
        line_dark: line(base, &[DARK_PANEL], [255, 255, 255]),
    }
}

/// `style`-Wert mit den CSS-Variablen einer Farbe (nur `#rrggbb`, darum ohne Maskierung sicher):
/// `--pc-light`, `--pc-dark` (Hintergründe), `--pc-line` (Linie im hellen Thema und auf dem stets
/// hellen Blatt) und `--pc-line-dark` (Linie im dunklen Thema).
pub fn style(color: &str) -> String {
    let t = tints(color);
    format!("--pc-light:{};--pc-dark:{};--pc-line:{};--pc-line-dark:{}", t.light, t.dark, t.line, t.line_dark)
}

/// Lektor nach der Wahl (Runde 10): fest gewählt, wenn es die Persona gibt und sie aktiv ist, sonst die
/// aktive Persona mit dem größten Kontext, bei Gleichstand die frühere in der Reihenfolge `position`,
/// `id` – dieselbe Regel wie `baruch_core::Work::lector` ([`LectorChoice::pick`]).
pub fn lector_of<'a>(rows: &'a [PersonaRow], choice: &LectorChoice) -> Option<&'a PersonaRow> {
    choice.pick(rows, |p| p.handle.as_str(), |p| p.context_length, |p| p.active).map(|i| &rows[i])
}

/// Lektor nach der Wahl mit der **wirksamen** Kontextlänge (Runde 16a): wie [`lector_of`], aber „automatisch nach
/// Kontext“ (und eine feste, inaktive Persona, die darauf zurückfällt) vergleicht `state.model.context` – die
/// Einstellung der Persona, abgeglichen mit dem, was die Gegenstelle meldet. Eine feste, aktive Wahl braucht keine
/// Abfrage.
pub async fn effective_lector_of(state: &AppState, rows: &[PersonaRow], choice: &LectorChoice) -> Option<String> {
    lector_by(state, rows, choice, false).await
}

/// Wie [`effective_lector_of`], aber ohne Netzabfrage (Runde 17a, L4): `ModelPort::context_cached` – was der Port
/// schon weiß, sonst die Einstellung. Für Seiten und Listen.
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

/// Ist `handle` (mit oder ohne `@`) der Handle von `row`?
fn same_handle(handle: &str, row: &PersonaRow) -> bool {
    let wanted = handle.trim();
    row.handle == wanted || row.handle.trim_start_matches('@') == wanted.trim_start_matches('@')
}

/// Personas samt Wahl und Lektor nach der gespeicherten Wahl (seit Runde 16a mit der wirksamen Kontextlänge,
/// [`effective_lector_of`]).
pub async fn rows_and_lector(state: &AppState) -> Result<(Vec<PersonaRow>, LectorChoice, Option<String>), RpcError> {
    let rows = state.store.persona_list().await?;
    let choice = state.store.lector_choice().await?;
    let lector = effective_lector_of(state, &rows, &choice).await;
    Ok((rows, choice, lector))
}

/// Wie [`rows_and_lector`], ohne Netzabfrage ([`cached_lector_of`]): für Seiten, Listen und Schalter.
pub async fn rows_and_lector_cached(
    state: &AppState,
) -> Result<(Vec<PersonaRow>, LectorChoice, Option<String>), RpcError> {
    let rows = state.store.persona_list().await?;
    let choice = state.store.lector_choice().await?;
    let lector = cached_lector_of(state, &rows, &choice).await;
    Ok((rows, choice, lector))
}

/// `persona.chat_scope_get` bzw. Antwort von `persona.chat_scope_set` (Runde 16a).
pub(crate) async fn chat_scope_json(state: &AppState) -> Result<Json, RpcError> {
    Ok(json!({ "scope": state.store.chat_scope().await?.word() }))
}

/// `scope` aus `chapter` | `section`; sonst -32602.
pub(crate) fn chat_scope_from(value: Option<&str>) -> Result<baruch_store::ChatScope, RpcError> {
    value.and_then(baruch_store::ChatScope::parse).ok_or_else(|| {
        RpcError::new(INVALID_PARAMS, "scope: „chapter“ (ganzes Kapitel) oder „section“ (nur Abschnitt)")
    })
}

/// Personas mit Rolle, Modell, Kontext, aktiv, Lektor und Farbe, alles aus der Tabelle `persona`
/// (Runde 9: nicht mehr aus dem Vorgabewerk, darum auch ohne Werke vollständig).
async fn list(state: &AppState) -> Result<Json, RpcError> {
    let (rows, choice, lector) = rows_and_lector_cached(state).await?;
    let backends = state.store.persona_backends().await?;
    let connections = state.store.model_connection_list().await?;
    let mut personas: Vec<Json> = Vec::with_capacity(rows.len());
    let mut lector_context: Option<crate::assist::port::ContextBudget> = None;
    for p in &rows {
        let backend = backends.iter().find(|(h, _)| *h == p.handle).map(|(_, b)| *b).unwrap_or_default();
        let connection = connections.iter().find(|c| c.id == backend.connection).or(connections.first());
        // Kontext ohne Netzabfrage: was der Port schon weiß (Zwischenspeicher), sonst die Einstellung.
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

/// Verbindung einer Persona als JSON (`persona.backend_get`): Verbindung, lokal/extern, Freigabe und ob ein
/// Auftrag so hinausgehen darf (`usable`, sonst `warning`).
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

/// `persona.backend_set`: Verbindung (id) und Freigabe „extern“ setzen. Nur der Mensch.
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

/// Vorgabe des Schalters „Bibel-Belege“: `@belege` an, sonst (der Lektor) aus.
pub fn bible_default(handle: &str) -> bool {
    handle.trim().trim_start_matches('@').eq_ignore_ascii_case(crate::assist::rag_facts::BELEGE.trim_start_matches('@'))
}

/// Persona, ob sie Bibel-Belege bekommen darf (`@belege` oder Lektor nach der Wahl), ob ihr Schalter an ist
/// (ohne Zeile die Vorgabe [`bible_default`]) und die Vorgabe.
pub(crate) async fn bible_state(state: &AppState, handle: &str) -> Result<(PersonaRow, bool, bool, bool), RpcError> {
    let row = row_of(state, handle).await?;
    let (_, _, lector) = rows_and_lector_cached(state).await?;
    let allowed = crate::assist::rag_facts::allowed(&row.handle, lector.as_deref());
    let default = bible_default(&row.handle);
    let on = state.store.persona_bible(&row.handle).await?.unwrap_or(default);
    Ok((row, allowed, on, default))
}

/// `persona.bible_set`: nur für erlaubte Personas, sonst -32602 [`BIBLE_REFUSED`].
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

/// `persona.prompt_get` bzw. Antwort von `persona.set_prompt`.
pub(crate) fn prompt_json(row: &PersonaRow) -> Json {
    let default = crate::assist::prompts::default_prompt(&row.handle, &row.role);
    let prompt = crate::assist::prompts::effective_prompt(&row.handle, &row.role, row.prompt.as_deref());
    json!({ "handle": row.handle, "prompt": prompt, "default": default, "custom": row.prompt.is_some() })
}

/// Persona nach Handle (mit oder ohne `@`) aus der Tabelle; unbekannt → -32602.
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

/// Persona, ob sie Treffer der Bedeutungssuche bekommen darf (`@belege` oder Lektor nach der Wahl) und ob
/// ihr Schalter an ist. Unbekannt → -32602.
pub(crate) async fn rag_state(state: &AppState, handle: &str) -> Result<(PersonaRow, bool, bool), RpcError> {
    let row = row_of(state, handle).await?;
    let (_, _, lector) = rows_and_lector_cached(state).await?;
    let allowed = crate::assist::rag_facts::allowed(&row.handle, lector.as_deref());
    let on = state.store.persona_rag(&row.handle).await?;
    Ok((row, allowed, on))
}

/// `persona.rag_set`: nur für erlaubte Personas, sonst -32602 [`RAG_REFUSED`].
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

/// `persona.set_prompt`: `None` = Vorgabe.
pub(crate) async fn set_prompt(state: &AppState, handle: &str, prompt: Option<&str>) -> Result<PersonaRow, RpcError> {
    Ok(state.store.set_persona_prompt(handle, prompt).await?)
}

/// Lektor-Wahl als JSON (`persona.lector_get`).
pub(crate) async fn lector_json(state: &AppState) -> Result<Json, RpcError> {
    let (_, choice, lector) = rows_and_lector_cached(state).await?;
    Ok(json!({ "mode": choice.mode(), "handle": choice.handle(), "lector": lector }))
}

/// Wahl aus `mode` (`fixed`/`auto`) und `handle`; ungültig → -32602.
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

/// Parameter von `persona.update`: `handle` Pflicht, sonst nur genannte Felder; Grenzen nach
/// [`PersonaEdit::check`] (-32602 mit deutscher Meldung).
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

/// Geprüfte Parameter von `persona.update`: Handle, neuer Name (Runde 11, 1–80 Zeichen) und die
/// übrigen Felder.
pub(crate) struct Update {
    pub handle: String,
    pub name: Option<String>,
    pub edit: PersonaEdit,
}

/// `persona.update` (geprüft) → die gespeicherte Persona; mit `name` (Runde 11) in derselben
/// Transaktion ([`baruch_store::Store::save_persona`]).
pub(crate) async fn update(state: &AppState, update: &Update) -> Result<PersonaRow, RpcError> {
    update.edit.check().map_err(|why| RpcError::new(INVALID_PARAMS, why))?;
    if let Some(name) = &update.name {
        PersonaEdit::check_name(name).map_err(|why| RpcError::new(INVALID_PARAMS, why))?;
    }
    Ok(state.store.save_persona(&update.handle, update.name.as_deref(), &update.edit, None, None).await?)
}

/// `color` aus den Parametern: Text oder `null` (fehlt → Fehler, damit nicht versehentlich
/// zurückgesetzt wird).
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

/// `color` für `settings.set` (Seite „assistenten“): fehlt → `None` (nichts ändern), sonst wie
/// `persona.set_color` (`null` = Vorgabe).
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
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

// ---- Seite ---------------------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct PageQuery {
    /// Werk, mit dem die Einstellungen geöffnet werden.
    work: Option<i64>,
}

/// `GET /einstellungen/personas[?work=<id>]` → 303 `/einstellungen/assistenten[?work=<id>]` (Runde 9:
/// Farben, Rolle, Modell, Kontext und aktiv stehen jetzt auf der Einstellungsseite „Assistenten“).
async fn personas_page(Query(q): Query<PageQuery>) -> Response {
    let work = q.work.filter(|&w| w > 0).map_or(String::new(), |w| format!("?work={w}"));
    Redirect::to(&format!("/einstellungen/assistenten{work}")).into_response()
}

#[derive(Deserialize, Default)]
struct ActForm {
    method: Option<String>,
    handle: Option<String>,
    /// Farbwahl (`<input type="color">`).
    color: Option<String>,
    /// Textfeld; nicht leer → geht vor der Farbwahl.
    color_text: Option<String>,
    /// `1`: zurück zur Vorgabe.
    reset: Option<String>,
    work: Option<String>,
    /// `persona.update` (Runde 9): Rolle, Modell, Kontextlänge (Tokens), Kästchen aktiv (`1`/`on`;
    /// fehlt es, ist die Persona inaktiv).
    role: Option<String>,
    model: Option<String>,
    context_length: Option<String>,
    active: Option<String>,
    /// `persona.set_prompt` (Runde 10): eigener Rollen-Prompt; `reset=1` = Vorgabe.
    prompt: Option<String>,
    /// `persona.set_lector` (Runde 10): `fixed` oder `auto`, dazu `handle`.
    mode: Option<String>,
    /// Runde 11 (Tabs je Persona): `save` (alle Felder auf einmal) oder `reset_prompt` (mit
    /// `confirm=1`, sonst Rückfrage). Geht vor `method`.
    action: Option<String>,
    /// Name der Persona (`action=save`).
    name: Option<String>,
    /// `1`: Zurücksetzen bestätigt (`action=reset_prompt`).
    confirm: Option<String>,
    /// Runde 12 (`action=rag`): Kästchen „Treffer der Bedeutungssuche als Belege mitgeben“ (`1`/`on`; fehlt
    /// es, ist der Schalter aus).
    on: Option<String>,
    /// Runde 15b (`action=backend`): id der Modell-Verbindung und Kästchen „extern freigeben“.
    connection: Option<String>,
    external_allowed: Option<String>,
    /// Runde 16a (`method=persona.chat_scope_set`): `chapter` oder `section`.
    scope: Option<String>,
}

/// Eingabe eines abgelehnten Formulars „Speichern“ (Runde 11): Die Seite zeigt den Tab der Persona mit
/// der Eingabe und der Meldung erneut, statt sie zu verlieren.
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

/// Abgelehnte Eingabe der laufenden Anfrage (nur während der 422-Seite gesetzt).
pub(crate) fn retry() -> Option<PersonaRetry> {
    RETRY.try_with(Clone::clone).ok()
}

/// Ein Kästchen ist angehakt (`1`/`on`).
fn ticked(value: Option<&str>) -> bool {
    value.is_some_and(|v| matches!(v.trim(), "1" | "on"))
}

/// Adresse der Seite „Assistenten“ mit dem Tab der Persona: `?persona=<handle ohne @>[&ok=…][&work=W]`.
fn tab_url(handle: &str, extra: &str, work: Option<i64>) -> String {
    let back = work.map_or(String::new(), |w| format!("&work={w}"));
    let at = anchor(handle);
    format!("/einstellungen/assistenten?persona={at}{extra}{back}#persona-{at}")
}

/// `action=save`: Name, Farbe, Rolle, Modell, Kontext, aktiv und Rollen-Prompt in **einer**
/// Transaktion ([`baruch_store::Store::save_persona`]). Ein Prompt gleich der Vorgabe aus dem Code
/// bleibt die Vorgabe (kein eigener Prompt nur durch Speichern).
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
    // Die Farbwahl schickt immer einen Wert; nur eine Änderung macht ihn zur eigenen Farbe.
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

/// Anker der Persona auf der Seite „Assistenten“: `persona-<Handle ohne @>`, für die Adresse kodiert.
fn anchor(handle: &str) -> String {
    crate::view::query_escape(handle.trim().trim_start_matches('@'))
}

/// `POST /einstellungen/personas/act`: `method=persona.set_color` (Felder `handle`, `color` bzw.
/// `color_text`, `reset=1` → Vorgabe) oder `method=persona.update` (`handle`, `role`, `model`,
/// `context_length`, `active=1|leer`), dazu `work`. Danach 303 auf
/// `/einstellungen/assistenten?ok=farbe|persona[&work=W]#persona-<handle>`; ungültige Werte oder
/// unbekannte Persona → 422 mit der Seite und der Meldung; andere Methoden → 400.
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

/// Formulare der Tabs (Runde 11): `action=save` → 303 `…?persona=<h>&ok=persona#persona-<h>`, Fehler
/// → 422 mit dem Tab, der Eingabe und der Meldung; `action=reset_prompt` ohne `confirm=1` → 303 zur
/// Rückfrage (`&rueckfrage=vorgabe`), mit `confirm=1` → Vorgabe, `&ok=vorgabe`; `action=reset_color` →
/// Farbe zurück zur Palette, `&ok=farbe`; `action=rag` (Runde 12, Kästchen `on`) → `persona.rag_set`,
/// `&ok=rag`, für andere Personas als `@belege` und den Lektor 422 mit [`RAG_REFUSED`]; Runde 15b:
/// `action=backend` (`connection`, Kästchen `external_allowed`) → `persona.backend_set`, `&ok=verbindung`;
/// `action=bible` (Kästchen `on`) → `persona.bible_set`, `&ok=bibel` (andere Personas 422 mit
/// [`BIBLE_REFUSED`]). Andere → 400.
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

/// `GET /einstellungen/personas` (Weiterleitung, statisches Segment vor `/einstellungen/{seite}`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/einstellungen/personas", get(personas_page))
}

/// `POST /einstellungen/personas/act` (im Router der schreibenden Routen, hinter `guard::same_origin`).
/// Dazu die Persona-Aufträge (Runde 10): `POST /assistenten/act` und `POST /assistenten/stream`.
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
