// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeuge zur Umschlag-Anordnung (Vertrag Runde 27, Abschnitte 1.9 und 4; Aufgabe umschlag-daten).
//!
//! - `cover_layout {work}`: wie RPC `cover.layout`, nur lesend; ohne Seitenmaß (Satzfehler im Buch) `geometry: null`
//!   und `geometry_error`, dann nur die gespeicherten Blöcke (ohne Höhe).
//! - `cover_layout_set {work, blocks, note?, persona?}`: stellt einen Antrag (Bildanträge, op `cover_layout`) und
//!   ändert nichts sofort; der Mensch bestätigt auf der Seite Bilder → Anträge. Jeder genannte Block ersetzt seine
//!   Art ganz; fehlende Felder eines Blocks füllt der wirksame Block (gespeichert bzw. Vorgabe), so genügt etwa
//!   `{kind: "front_title", y_mm: 40}`. Der Antrag merkt sich den Fingerabdruck der gespeicherten Anordnung
//!   (`expected_layout`); ein gleicher offener Antrag wird nicht doppelt angelegt.

use baruch_store::{AssetOp, CoverBlock, CoverBlockKind, WorkId};
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, origin_of, tool_error, work_or_default, work_schema};
use crate::rpc::REFUSED;
use crate::{AppState, asset_requests, cover_layout};

pub const NAMES: [&str; 2] = ["cover_layout", "cover_layout_set"];

pub const LAYOUT_DESCRIPTION: &str = "Read the cover layout of a work (read-only): per block (title, subtitle, \
author, publisher, free text on front and spine; motto, blurb, short biography, ISBN with barcode, publisher and \
price, free text on the back) position and width in mm from the trim edge of its own area, alignment, size, leading, \
colour, font, style, background, visible, reading direction (spine) and own text; plus stored (own layout stored?), \
dimensions (geometry; without a page size null with geometry_error), print check and default font.";

pub const LAYOUT_SET_DESCRIPTION: &str = "Request: change the cover layout. Every block named replaces its kind \
completely; missing fields of a block come from the effective block (cover_layout), e.g. {kind: \"front_title\", \
y_mm: 40}. Changes nothing immediately; the human confirms on the images page (requests). Origin `persona` or the \
client as @mcp:<name>; an equal open request is not filed twice; if the human has changed the layout in the \
meantime, the request becomes stale on confirming.";
/// Ein Block wie in `cover.layout` (Vertrag 1.3).
fn block_schema() -> Value {
    json!({ "type": "object", "required": ["kind"], "properties": {
        "kind": { "type": "string", "enum": [
            "front_title", "front_subtitle", "front_author", "front_publisher", "front_free",
            "spine_title", "spine_author", "spine_publisher", "spine_free",
            "back_motto", "back_blurb", "back_bio", "back_barcode", "back_publisher_price", "back_free"] },
        "x_mm": { "type": "number", "description": "left edge in mm from the trim edge of the area (-50 to 1000)" },
        "y_mm": { "type": "number", "description": "top edge in mm from the trim edge of the area (-50 to 1000)" },
        "width_mm": { "type": "number", "description": "width in reading direction in mm (1 to 1000)" },
        "align": { "type": "string", "enum": ["left", "center", "right", "justify"] },
        "size_pt": { "type": "number", "description": "font size in pt (4 to 200)" },
        "leading_em": { "type": "number", "description": "leading in em (0 to 3, default 0.65)" },
        "color": { "type": "string", "description": "light, dark or #rrggbb" },
        "font": { "type": ["string", "null"], "description": "font family; null = font of the work template" },
        "style": { "type": "string", "enum": ["normal", "bold", "italic"] },
        "backdrop": { "type": "string", "enum": ["none", "shadow", "gradient"] },
        "visible": { "type": "boolean" },
        "direction": { "type": ["string", "null"], "enum": ["down", "up", null],
                       "description": "spine only: down = readable from top to bottom, up = from bottom to top" },
        "text": { "type": ["string", "null"],
                  "description": "own text (only free texts and motto, at most 1000 characters)" } } })
}

pub fn layout_schema() -> Value {
    json!({ "type": "object", "required": ["work"], "properties": { "work": work_schema() } })
}

pub fn layout_set_schema() -> Value {
    json!({ "type": "object", "required": ["work", "blocks"], "properties": {
        "work": work_schema(),
        "blocks": { "type": "array", "items": block_schema() },
        "note": { "type": "string", "description": "reason for the human" },
        "persona": { "type": "string", "description": "persona handle (e.g. @lektor) as requester (only without a token from the own machine); otherwise the client as @mcp:<name>" } } })
}

#[derive(serde::Deserialize)]
struct LayoutArgs {
    work: Option<WorkRef>,
}

#[derive(serde::Deserialize)]
struct SetArgs {
    work: Option<WorkRef>,
    blocks: Value,
    #[serde(default)]
    note: String,
    persona: Option<String>,
}

pub async fn call(state: &AppState, client: &Client, name: &str, arguments: Value) -> Option<Result<Value, Failure>> {
    Some(match name {
        "cover_layout" => layout(state, arguments).await,
        "cover_layout_set" => layout_set(state, client, arguments).await,
        _ => return None,
    })
}

/// Ansicht wie `cover.layout`; ohne Seitenmaß nur die gespeicherten Blöcke mit `geometry: null`.
async fn layout(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: LayoutArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    match cover_layout::layout_view(state, work, None).await {
        Ok(view) => Ok(json!(view)),
        Err(e) if e.code == REFUSED => {
            let stored = state.store.cover_layout(work).await?;
            let meta = state.store.book_meta(work).await?;
            let blocks: Vec<Value> = stored
                .blocks
                .iter()
                .map(|b| {
                    let mut v = json!(b);
                    v["stored"] = json!(true);
                    v["height_mm"] = Value::Null;
                    v
                })
                .collect();
            Ok(json!({
                "work": work,
                "stored": !stored.is_empty(),
                "title_area": meta.title_area,
                "blocks": blocks,
                "geometry": null,
                "geometry_error": e.message,
                "warnings": [],
                "default_font": cover_layout::default_font(state, work).await?,
            }))
        }
        Err(e) => Err(e.into()),
    }
}

/// Wirksamer Block je Art (gespeichert bzw. Vorgabe) als Grundlage für unvollständige Blöcke.
async fn bases(state: &AppState, work: WorkId) -> Result<Vec<CoverBlock>, Failure> {
    match cover_layout::layout_view(state, work, None).await {
        Ok(view) => Ok(view.blocks.into_iter().map(|b| b.block).collect()),
        // Ohne Seitenmaß keine Vorgabeblöcke: dann nur die gespeicherten.
        Err(e) if e.code == REFUSED => Ok(state.store.cover_layout(work).await?.blocks),
        Err(e) => Err(e.into()),
    }
}

async fn layout_set(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: SetArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let origin = origin_of(&state.store, client, a.persona.as_deref()).await?;
    let known = bases(state, work).await?;
    let base = |kind: CoverBlockKind| known.iter().find(|b| b.kind == kind).cloned();
    let blocks = cover_layout::parse_blocks(Some(&a.blocks), base).map_err(Failure::Args)?;
    if blocks.is_empty() {
        return Err(tool_error("blocks: at least one block of the layout expected."));
    }
    let op = AssetOp::CoverLayout { blocks, expected_layout: String::new() };
    let id = {
        let _held = state.write_lock.lock().await;
        state.store.asset_request(work, &op, &origin, &a.note).await?
    };
    let request = state.store.asset_request_get(work, id).await?;
    Ok(json!({
        "work": work,
        "request": asset_requests::request_json(&request),
        "status": "open",
        "message": format!(
            "Request {} filed ({}); the request itself changes nothing yet. The human confirms or rejects.",
            request.id, request.summary
        ),
        "url": format!("/werke/{work}/bilder#antraege"),
    }))
}
