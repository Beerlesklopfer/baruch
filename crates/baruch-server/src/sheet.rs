// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typeset view per work (Round 18c): appearance of the HTML typeset view from the work template, overridden with
//! the typeset view settings of the work ([`baruch_store::SheetSettings`], migration 0044). Typst, export and PDF
//! stay untouched by it (they never read `work_sheet`).
//!
//! - RPC `work.sheet {work?}` → `{settings, css}`, `work.sheet_set {work?, font?, heading_font?, size?, fill?,
//!   heading_fill?, background?, width?, reset?}` → `{settings, css}` (`docs/RPC.md`). A missing field stays,
//!   `null` or empty text takes it back (from the template again), `reset: true` takes all back beforehand.
//! - The same appearance at all places: `wysiwyg.style`, the `<style id="ws-style">` in the editor
//!   (`view.rs` → `fragments/editor.html`) and `work.sheet` compute through [`sheet_style_for`].
//! - Form without JavaScript: `POST /einstellungen/werk/satzbild` (`settings/work.rs`).

use baruch_store::{SheetSettings, WorkId};
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Fields of the settings as RPC and form name them.
pub const FIELDS: [&str; 7] = ["font", "heading_font", "size", "fill", "heading_fill", "background", "width"];

/// Does this module serve the RPC method? `work.sheet`, `work.sheet_set`.
pub fn handles(method: &str) -> bool {
    matches!(method, "work.sheet" | "work.sheet_set")
}

/// RPC `work.sheet` and `work.sheet_set`; `work` is already resolved (missing → default work).
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "work.sheet" => sheet_json(state, work).await,
        "work.sheet_set" => set_sheet(state, work, &params).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// `{settings, css}` of the work.
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

/// `work.sheet_set`: lay the settings from `params` over the stored ones (missing = stays, `null`/empty =
/// back; `reset: true` = all back) and store them. Invalid values are rejected by the store with a German message
/// (`REFUSED`), nothing changed.
pub async fn set_sheet(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let reset = match params.get("reset") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.trim(), "1" | "true" | "on" | "ja"),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, tc("export_msg.sheet.reset", &[]))),
    };
    // Round 23a (review R18): reading, changing and writing under the write lock of the server, so that two
    // simultaneous partial changes (one only `font`, one only `fill`) do not overwrite each other.
    let held = state.write_lock.lock().await;
    let mut settings = if reset { SheetSettings::default() } else { state.store.sheet_settings(work).await? };
    for field in FIELDS {
        let value = match params.get(field) {
            None => continue,
            Some(Value::Null) => None,
            Some(Value::String(s)) if s.trim().is_empty() => None,
            Some(Value::String(s)) => Some(s.trim().to_owned()),
            Some(_) => {
                return Err(RpcError::new(INVALID_PARAMS, tc("export_msg.sheet.text", &[("field", field.into())])));
            }
        };
        if let Some(target) = slot(&mut settings, field) {
            *target = value;
        }
    }
    state.store.set_sheet_settings(work, &settings).await?;
    drop(held);
    sheet_json(state, work).await
}

/// Settings of the work as [`baruch_wysiwyg::SheetStyle`] (only the fields that are set).
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

/// Appearance of the typeset view: template (`sheet_style(work_template)`) overridden with the settings of the
/// work.
pub async fn sheet_style_for(state: &AppState, work: WorkId) -> Result<baruch_wysiwyg::SheetStyle, RpcError> {
    let template = state.store.work_template(work).await?;
    let settings = state.store.sheet_settings(work).await?;
    Ok(baruch_wysiwyg::sheet_style_over(baruch_wysiwyg::sheet_style(&template), &style_of(settings)))
}

/// CSS of the typeset view for the work ([`sheet_style_for`] → `sheet_css`). `sheet_css` lets only values of
/// harmless characters through (never `<`, `;`, `{`, `}`, `\`), therefore the result may go unescaped into a
/// `<style>` element; as a second guard this function nevertheless replaces every `<` (no `</style` possible).
pub async fn sheet_css_for(state: &AppState, work: WorkId) -> Result<String, RpcError> {
    Ok(baruch_wysiwyg::sheet_css(&sheet_style_for(state, work).await?).replace('<', "\\3c "))
}
