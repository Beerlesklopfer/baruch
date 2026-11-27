// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satzbild je Werk (Runde 18c): Darstellung des HTML-Satzbilds aus der Werkvorlage, überschrieben mit den
//! Satzbild-Einstellungen des Werks ([`baruch_store::SheetSettings`], Migration 0044). Typst, Export und PDF
//! bleiben davon unberührt (sie lesen nie `work_sheet`).
//!
//! - RPC `work.sheet {work?}` → `{settings, css}`, `work.sheet_set {work?, font?, heading_font?, size?, fill?,
//!   heading_fill?, background?, width?, reset?}` → `{settings, css}` (`docs/RPC.md`). Ein fehlendes Feld bleibt,
//!   `null` oder leerer Text nimmt es zurück (wieder aus der Vorlage), `reset: true` nimmt vorher alle zurück.
//! - Dieselbe Darstellung an allen Stellen: `wysiwyg.style`, das `<style id="ws-style">` im Editor
//!   (`view.rs` → `fragments/editor.html`) und `work.sheet` rechnen über [`sheet_style_for`].
//! - Formular ohne JavaScript: `POST /einstellungen/werk/satzbild` (`settings/work.rs`).

use baruch_store::{SheetSettings, WorkId};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Felder der Einstellungen, wie sie RPC und Formular nennen.
pub const FIELDS: [&str; 7] = ["font", "heading_font", "size", "fill", "heading_fill", "background", "width"];

/// Bedient dieses Modul die RPC-Methode? `work.sheet`, `work.sheet_set`.
pub fn handles(method: &str) -> bool {
    matches!(method, "work.sheet" | "work.sheet_set")
}

/// RPC `work.sheet` und `work.sheet_set`; `work` ist schon aufgelöst (fehlt → Vorgabewerk).
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "work.sheet" => sheet_json(state, work).await,
        "work.sheet_set" => set_sheet(state, work, &params).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// `{settings, css}` des Werks.
async fn sheet_json(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let settings = state.store.sheet_settings(work).await?;
    let css = sheet_css_for(state, work).await?;
    Ok(json!({ "settings": settings, "css": css }))
}

fn slot<'a>(settings: &'a mut SheetSettings, field: &str) -> Option<&'a mut Option<String>> {
    Some(match field {
        "font" => &mut settings.font,
        "heading_font" => &mut settings.heading_font,
        "size" => &mut settings.size,
        "fill" => &mut settings.fill,
        "heading_fill" => &mut settings.heading_fill,
        "background" => &mut settings.background,
        "width" => &mut settings.width,
        _ => return None,
    })
}

/// `work.sheet_set`: Einstellungen aus `params` über die gespeicherten legen (fehlend = bleibt, `null`/leer =
/// zurück; `reset: true` = alle zurück) und speichern. Ungültige Werte lehnt der Store mit deutscher Meldung ab
/// (`REFUSED`), nichts geändert.
pub async fn set_sheet(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let reset = match params.get("reset") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.trim(), "1" | "true" | "on" | "ja"),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "reset: true oder false erwartet")),
    };
    // Runde 23a (Gutachten R18): Lesen, Ändern und Schreiben unter der Schreibsperre des Servers, damit zwei
    // gleichzeitige Teiländerungen (eine nur `font`, eine nur `fill`) einander nicht überschreiben.
    let held = state.write_lock.lock().await;
    let mut settings = if reset { SheetSettings::default() } else { state.store.sheet_settings(work).await? };
    for field in FIELDS {
        let value = match params.get(field) {
            None => continue,
            Some(Value::Null) => None,
            Some(Value::String(s)) if s.trim().is_empty() => None,
            Some(Value::String(s)) => Some(s.trim().to_owned()),
            Some(_) => return Err(RpcError::new(INVALID_PARAMS, format!("{field}: Text erwartet"))),
        };
        if let Some(target) = slot(&mut settings, field) {
            *target = value;
        }
    }
    state.store.set_sheet_settings(work, &settings).await?;
    drop(held);
    sheet_json(state, work).await
}

/// Einstellungen des Werks als [`baruch_wysiwyg::SheetStyle`] (nur die gesetzten Felder).
fn style_of(settings: SheetSettings) -> baruch_wysiwyg::SheetStyle {
    baruch_wysiwyg::SheetStyle {
        font: settings.font,
        heading_font: settings.heading_font,
        size: settings.size,
        fill: settings.fill,
        heading_fill: settings.heading_fill,
        background: settings.background,
        width: settings.width,
        ..baruch_wysiwyg::SheetStyle::default()
    }
}

/// Darstellung des Satzbilds: Vorlage (`sheet_style(work_template)`) überschrieben mit den Einstellungen des
/// Werks.
pub async fn sheet_style_for(state: &AppState, work: WorkId) -> Result<baruch_wysiwyg::SheetStyle, RpcError> {
    let template = state.store.work_template(work).await?;
    let settings = state.store.sheet_settings(work).await?;
    Ok(baruch_wysiwyg::sheet_style_over(baruch_wysiwyg::sheet_style(&template), &style_of(settings)))
}

/// CSS des Satzbilds für das Werk ([`sheet_style_for`] → `sheet_css`). `sheet_css` lässt nur Werte aus
/// harmlosen Zeichen durch (nie `<`, `;`, `{`, `}`, `\`), darum darf das Ergebnis unmaskiert in ein
/// `<style>`-Element; als zweite Wache ersetzt diese Funktion trotzdem jedes `<` (kein `</style` möglich).
pub async fn sheet_css_for(state: &AppState, work: WorkId) -> Result<String, RpcError> {
    Ok(baruch_wysiwyg::sheet_css(&sheet_style_for(state, work).await?).replace('<', "\\3c "))
}
