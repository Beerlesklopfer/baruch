// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeuge für die Gliederung (Wunsch 2026-11-06: „Ein LLM hat den besseren Überblick durch
//! seinen Context“, „UD nach Bestätigung“).
//!
//! - `outline_add` legt Gliederungspunkte **sofort** an: einen (`title`) oder eine ganze Liste
//!   (`items`, je `{title, level?}` mit absoluter Ebene) als Block in **einer** Transaktion und
//!   **einem** Spiegel-Commit ([`baruch_store::Store::outline_add_by_assistant`]); bei einem Fehler
//!   nichts. Revision 1 jedes Punkts und der Commit tragen die Persona bzw. den Client als
//!   `@mcp:<name>` ([`mirror::OutlineBy::Direct`]), nicht den Menschen. Auch Teile (Ebene 0, Runde 14)
//!   vor oder hinter einem Teil oder Kapitel, auch mit Werkvorlage – aber nur, wenn der Teil keine
//!   vorhandenen Kapitel übernimmt. Sonst (Nutzerfrage 12, Entscheidung 2026-11-07: „es braucht eine
//!   aktive Zustimmung (Popup)“) ändert sich nichts: Es entsteht ein Antrag (`action: requested`, mit
//!   `adopted` = Kapitel, die wandern würden), dem der Mensch im Dialog zustimmt (`crate::consent`).
//! - `outline_rename`, `outline_move`, `outline_set_level`, `outline_set_numbered`, `outline_delete`
//!   ändern **nichts**,
//!   sondern legen einen Antrag an ([`baruch_store::Store::outline_request`], Migration 0012). Der
//!   Mensch bestätigt oder lehnt ab (Seite `/gliederung#antraege`, RPC `outline.request_decide`).
//! - `outline_requests` liest Anträge (offen bzw. alle, wahlweise nur eigene); mit `preview: true` je
//!   offenem Antrag die Gliederung nach dem Bestätigen (`preview`) bzw. `preview_error` (Runde 14).
//!
//! Punkte nur über den festen Schlüssel (`key`, beim Verschieben `target_key`); `node` bzw. `target`
//! (Stellen) → -32602 wie bei den übrigen schreibenden Werkzeugen. Alles Schreibende unter
//! [`AppState::write_lock`].

use baruch_store::{AssistantAdd, IdeaOrigin, NewPoint, OutlineOp, Place, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, node_url, resolve, work_schema, write_key};
use crate::{AppState, mirror};

/// Namen der Werkzeuge dieses Moduls (in der Reihenfolge von `tools/list`).
pub const NAMES: &[&str] = &[
    "outline_add",
    "outline_rename",
    "outline_move",
    "outline_set_level",
    "outline_set_numbered",
    "outline_delete",
    "outline_requests",
];

fn persona_schema() -> Value {
    json!({ "type": "string", "description": "persona handle (e.g. @lektor) as author or requester (only without a token from the own machine); otherwise the client as @mcp:<name>" })
}

fn key_schema(what: &str) -> Value {
    json!({ "type": "integer", "description": format!("stable key {what} (key from outline); survives rearranging") })
}

fn title_schema() -> Value {
    json!({ "type": "string", "description": "title: not empty, one line, at most 200 characters" })
}

fn place_schema(default: bool) -> Value {
    let text = if default {
        "before: before the item; after (default): after it with its sub-items; into: as the last sub-item"
    } else {
        "before: before the target; after: after the target with its sub-items; into: as the last sub-item of the target"
    };
    json!({ "enum": ["before", "after", "into"], "description": text })
}

fn note_schema() -> Value {
    json!({ "type": "string", "description": "reason for the human" })
}

pub fn add_schema() -> Value {
    json!({ "type": "object", "required": ["work"], "properties": {
        "work": work_schema(),
        "key": key_schema("of the reference item; missing only in a work without any outline item"),
        "place": place_schema(true),
        "title": title_schema(),
        "level": { "type": "integer", "minimum": 0, "maximum": 3,
            "description": "only with title: absolute level of the item (0 part before/after a part or chapter, also with a work template); if missing, the level of the place" },
        "items": { "type": "array", "minItems": 1, "maxItems": baruch_store::MAX_NEW_POINTS,
            "description": "instead of title: several items in tree order as one block (one transaction, one commit). \
                            level = absolute level (0 part, 1 chapter, 2 section, 3 subsection); \
                            if missing, the level of the place applies. The first item stands on the level of the place, none higher \
                            (except parts), each at most one level deeper than the previous one. Parts (level 0) before or after \
                            a part or chapter, not with place into; the following chapters up to the next part then stand \
                            below the new part (adopted).",
            "items": { "type": "object", "required": ["title"], "properties": {
                "title": title_schema(),
                "level": { "type": "integer", "minimum": 0, "maximum": 3 } } } },
        "note": { "type": "string", "description": "reason for the human (only if a request results)" },
        "persona": persona_schema(),
        "model": { "type": "string", "description": "model for revision 1; otherwise name and version of the client" } } })
}

pub fn rename_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "title"], "properties": {
        "work": work_schema(), "key": key_schema("of the item"), "title": title_schema(),
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn move_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "target_key", "place"], "properties": {
        "work": work_schema(), "key": key_schema("of the item"), "target_key": key_schema("of the target"),
        "place": place_schema(false), "note": note_schema(), "persona": persona_schema() } })
}

pub fn set_level_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "level"], "properties": {
        "work": work_schema(), "key": key_schema("of the item"),
        "level": { "type": "integer", "minimum": 1, "maximum": 3, "description": "1 chapter, 2 section, 3 subsection; sub-items move along" },
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn set_numbered_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "numbered"], "properties": {
        "work": work_schema(), "key": key_schema("of the item"),
        "numbered": { "type": "boolean", "description": "true: count with a number; false: without a number (e.g. preface, introduction)" },
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn delete_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key"], "properties": {
        "work": work_schema(), "key": key_schema("of the item (only without sub-items)"),
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn requests_schema() -> Value {
    json!({ "type": "object", "required": ["work"], "properties": {
        "work": work_schema(),
        "open": { "type": "boolean", "description": "true (default): only open ones; false: also decided ones" },
        "preview": { "type": "boolean", "description": "true: per open request preview (outline after confirming: key, level, number, title, mark same|renamed|moved|level|numbered|deleted) or preview_error (stale, no longer executable); default false" },
        "mine": { "type": "boolean", "description": "only own ones (requester = persona or @mcp:<name>); default false" },
        "persona": persona_schema() } })
}

/// Herkunft: `persona` (muss es geben) oder der Client als `@mcp:<name>`.
async fn origin(state: &AppState, client: &Client, persona: Option<&str>) -> Result<IdeaOrigin, Failure> {
    super::tools::origin_of(&state.store, client, persona).await
}

fn parse_place(value: Option<&str>, default: Option<Place>) -> Result<Place, Failure> {
    match value.map(str::trim) {
        None => default.ok_or_else(|| Failure::Args("arguments: place is missing (before, after or into)".into())),
        Some("before") => Ok(Place::Before),
        Some("after") => Ok(Place::After),
        Some("into") => Ok(Place::Into),
        Some(other) => Err(Failure::Args(format!("arguments: place \"{other}\" – before, after or into expected"))),
    }
}

/// `target` (Stelle) gibt es bei `outline_move` nicht, nur `target_key`.
fn refuse_target(arguments: &Value) -> Result<(), Failure> {
    if arguments.get("target").is_some() {
        return Err(Failure::Args(
            "arguments: target (position) is not accepted; pass target_key (stable key from outline).".into(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
struct AddArgs {
    work: WorkRef,
    key: Option<i64>,
    place: Option<String>,
    title: Option<String>,
    level: Option<u8>,
    items: Option<Vec<NewPoint>>,
    persona: Option<String>,
    model: Option<String>,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct RequestArgs {
    work: WorkRef,
    key: Option<i64>,
    target_key: Option<i64>,
    place: Option<String>,
    title: Option<String>,
    level: Option<u8>,
    numbered: Option<bool>,
    #[serde(default)]
    note: String,
    persona: Option<String>,
}

#[derive(Deserialize)]
struct ListArgs {
    work: WorkRef,
    open: Option<bool>,
    #[serde(default)]
    mine: bool,
    #[serde(default)]
    preview: bool,
    persona: Option<String>,
}

pub async fn call(state: &AppState, client: &Client, name: &str, arguments: Value) -> Option<Result<Value, Failure>> {
    Some(match name {
        "outline_add" => add(state, client, arguments).await,
        "outline_rename" | "outline_move" | "outline_set_level" | "outline_set_numbered" | "outline_delete" => {
            request(state, client, name, arguments).await
        }
        "outline_requests" => requests(state, client, arguments).await,
        _ => return None,
    })
}

/// Adresse der Anträge auf der Gliederungsseite.
fn requests_url(work: WorkId) -> String {
    format!("/gliederung?work={work}#antraege")
}

async fn add(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: AddArgs = args(arguments.clone())?;
    if arguments.get("node").is_some() {
        return Err(Failure::Args(
            "arguments: node (position) is not accepted; pass key (stable key from outline or read_node).".into(),
        ));
    }
    let points = match (a.title, a.items) {
        (Some(title), None) => vec![NewPoint { title, level: a.level }],
        (None, Some(_)) if a.level.is_some() => {
            return Err(Failure::Args(
                "arguments: level only with title; with items use items[].level per item".into(),
            ));
        }
        (None, Some(items)) => items,
        (Some(_), Some(_)) => return Err(Failure::Args("arguments: either title or items, not both".into())),
        (None, None) => return Err(Failure::Args("arguments: title or items is missing".into())),
    };
    if points.is_empty() {
        return Err(Failure::Args("arguments: items is empty".into()));
    }
    let place = parse_place(a.place.as_deref(), Some(Place::After))?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    let origin = origin(state, client, a.persona.as_deref()).await?;
    let model = a.model.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| client.model());
    // Unter der Schreibsperre: zwischen Übersetzen von `key` und Anlegen stellt niemand um.
    let held = state.write_lock.lock().await;
    let target = match a.key {
        Some(key) => Some(state.store.node_seq(work, key).await?),
        None => None,
    };
    let added =
        match state.store.outline_add_by_assistant(work, target, place, &points, &origin, &model, &a.note).await? {
            AssistantAdd::Added(added) => added,
            AssistantAdd::Requested(request) => {
                // Nutzerfrage 12: nichts angelegt; der Mensch stimmt im Dialog zu oder lehnt ab.
                return Ok(json!({
                    "work": work,
                    "action": "requested",
                    "status": "open",
                    "request": crate::outline::request_json(&request),
                    "adopted": request.expected.adopted,
                    "message": format!(
                        "Request {} filed, waiting for approval: {}. Nothing changed – the new part would adopt \
                         existing chapters; that needs the active approval of the human.",
                        request.id, request.summary
                    ),
                    "url": requests_url(work),
                }));
            }
        };
    let list: Vec<Value> = added
        .points
        .iter()
        .map(|p| json!({ "node": p.node, "key": p.key, "level": p.level, "title": p.title, "url": node_url(work, p.key) }))
        .collect();
    let first = &added.points[0];
    let mut answer = json!({
        "work": work, "action": "added", "detail": added.change.detail, "points": list,
        "node": first.node, "key": first.key, "url": node_url(work, first.key), "author": origin.name(),
        "adopted": added.adopted,
    });
    match mirror::record_outline_by(state, &held, &added.change, mirror::OutlineBy::Direct(&origin)).await {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
    Ok(answer)
}

async fn request(state: &AppState, client: &Client, name: &str, arguments: Value) -> Result<Value, Failure> {
    let a: RequestArgs = args(arguments.clone())?;
    let key = write_key(&arguments, a.key, "arguments")?;
    let op = match name {
        "outline_rename" => {
            let title = a.title.ok_or_else(|| Failure::Args("arguments: title is missing".into()))?;
            OutlineOp::Rename { key, title }
        }
        "outline_move" => {
            refuse_target(&arguments)?;
            let target_key = a.target_key.ok_or_else(|| {
                Failure::Args("arguments: target_key is missing – name the target by its stable key.".into())
            })?;
            OutlineOp::Move { key, target_key, place: parse_place(a.place.as_deref(), None)? }
        }
        "outline_set_level" => {
            let level = a.level.ok_or_else(|| Failure::Args("arguments: level is missing (1 to 3)".into()))?;
            OutlineOp::SetLevel { key, level }
        }
        "outline_set_numbered" => {
            let numbered =
                a.numbered.ok_or_else(|| Failure::Args("arguments: numbered is missing (true or false)".into()))?;
            OutlineOp::SetNumbered { key, numbered }
        }
        _ => OutlineOp::Delete { key },
    };
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    let origin = origin(state, client, a.persona.as_deref()).await?;
    let _held = state.write_lock.lock().await;
    let request = state.store.outline_request(work, op, &origin, &a.note).await?;
    Ok(json!({
        "work": work,
        "request": crate::outline::request_json(&request),
        "status": "open",
        "message": format!(
            "Request {} filed ({}); nothing changed. The human confirms or rejects.",
            request.id, request.summary
        ),
        "url": requests_url(work),
    }))
}

async fn requests(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: ListArgs = args(arguments)?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    let own = if a.mine { Some(origin(state, client, a.persona.as_deref()).await?) } else { None };
    let list = state.store.outline_requests(work, a.open.unwrap_or(true)).await?;
    let mut shown = Vec::new();
    for request in list.iter().filter(|r| own.as_ref().is_none_or(|o| r.origin.name() == o.name())) {
        let mut value = crate::outline::request_json(request);
        if a.preview {
            crate::outline::add_preview(state, work, request, &mut value).await?;
        }
        shown.push(value);
    }
    let list = shown;
    Ok(json!({ "work": work, "requests": list, "url": requests_url(work) }))
}
