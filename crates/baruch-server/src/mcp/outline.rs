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
    json!({ "type": "string", "description": "Persona-Handle (z. B. @lektor) als Autor bzw. Antragsteller (nur ohne Token vom eigenen Rechner); sonst der Client als @mcp:<name>" })
}

fn key_schema(what: &str) -> Value {
    json!({ "type": "integer", "description": format!("fester Schlüssel {what} (key aus outline); bleibt beim Umstellen gleich") })
}

fn title_schema() -> Value {
    json!({ "type": "string", "description": "Titel: nicht leer, einzeilig, höchstens 200 Zeichen" })
}

fn place_schema(default: bool) -> Value {
    let text = if default {
        "before: vor den Punkt; after (Vorgabe): hinter ihn samt Unterpunkten; into: als letzten Unterpunkt"
    } else {
        "before: vor das Ziel; after: hinter das Ziel samt Unterpunkten; into: als letzten Unterpunkt des Ziels"
    };
    json!({ "enum": ["before", "after", "into"], "description": text })
}

fn note_schema() -> Value {
    json!({ "type": "string", "description": "Begründung für den Menschen" })
}

pub fn add_schema() -> Value {
    json!({ "type": "object", "required": ["work"], "properties": {
        "work": work_schema(),
        "key": key_schema("des Bezugspunkts; fehlt nur in einem Werk ohne Gliederungspunkt"),
        "place": place_schema(true),
        "title": title_schema(),
        "level": { "type": "integer", "minimum": 0, "maximum": 3,
            "description": "nur mit title: absolute Ebene des Punkts (0 Teil vor/hinter einem Teil oder Kapitel, auch mit Werkvorlage); fehlt sie, die Ebene des Orts" },
        "items": { "type": "array", "minItems": 1, "maxItems": baruch_store::MAX_NEW_POINTS,
            "description": "statt title: mehrere Punkte in Baumreihenfolge als ein Block (eine Transaktion, ein Commit). \
                            level = absolute Ebene (0 Teil, 1 Kapitel, 2 Abschnitt, 3 Unterabschnitt); \
                            fehlt sie, gilt die Ebene des Orts. Der erste Punkt steht auf der Ebene des Orts, keiner höher \
                            (außer Teilen), jeder höchstens eine Ebene tiefer als der vorige. Teile (level 0) vor oder hinter \
                            einem Teil oder Kapitel, nicht mit place into; die folgenden Kapitel bis zum nächsten Teil stehen \
                            dann unter dem neuen Teil (adopted).",
            "items": { "type": "object", "required": ["title"], "properties": {
                "title": title_schema(),
                "level": { "type": "integer", "minimum": 0, "maximum": 3 } } } },
        "note": { "type": "string", "description": "Begründung für den Menschen (nur, wenn ein Antrag entsteht)" },
        "persona": persona_schema(),
        "model": { "type": "string", "description": "Modell für Revision 1; sonst Name und Version des Clients" } } })
}

pub fn rename_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "title"], "properties": {
        "work": work_schema(), "key": key_schema("des Punkts"), "title": title_schema(),
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn move_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "target_key", "place"], "properties": {
        "work": work_schema(), "key": key_schema("des Punkts"), "target_key": key_schema("des Ziels"),
        "place": place_schema(false), "note": note_schema(), "persona": persona_schema() } })
}

pub fn set_level_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "level"], "properties": {
        "work": work_schema(), "key": key_schema("des Punkts"),
        "level": { "type": "integer", "minimum": 1, "maximum": 3, "description": "1 Kapitel, 2 Abschnitt, 3 Unterabschnitt; Unterpunkte wandern mit" },
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn set_numbered_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key", "numbered"], "properties": {
        "work": work_schema(), "key": key_schema("des Punkts"),
        "numbered": { "type": "boolean", "description": "true: mit Nummer zählen; false: ohne Nummer (z. B. Vorwort, Einleitung)" },
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn delete_schema() -> Value {
    json!({ "type": "object", "required": ["work", "key"], "properties": {
        "work": work_schema(), "key": key_schema("des Punkts (nur ohne Unterpunkte)"),
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn requests_schema() -> Value {
    json!({ "type": "object", "required": ["work"], "properties": {
        "work": work_schema(),
        "open": { "type": "boolean", "description": "true (Vorgabe): nur offene; false: auch entschiedene" },
        "preview": { "type": "boolean", "description": "true: je offenem Antrag preview (Gliederung nach dem Bestätigen: key, level, number, title, mark same|renamed|moved|level|numbered|deleted) bzw. preview_error (veraltet, nicht mehr ausführbar); Vorgabe false" },
        "mine": { "type": "boolean", "description": "nur eigene (Antragsteller = persona bzw. @mcp:<name>); Vorgabe false" },
        "persona": persona_schema() } })
}

/// Herkunft: `persona` (muss es geben) oder der Client als `@mcp:<name>`.
async fn origin(state: &AppState, client: &Client, persona: Option<&str>) -> Result<IdeaOrigin, Failure> {
    super::tools::origin_of(&state.store, client, persona).await
}

fn parse_place(value: Option<&str>, default: Option<Place>) -> Result<Place, Failure> {
    match value.map(str::trim) {
        None => default.ok_or_else(|| Failure::Args("arguments: place fehlt (before, after oder into)".into())),
        Some("before") => Ok(Place::Before),
        Some("after") => Ok(Place::After),
        Some("into") => Ok(Place::Into),
        Some(other) => Err(Failure::Args(format!("arguments: place „{other}“ – before, after oder into erwartet"))),
    }
}

/// `target` (Stelle) gibt es bei `outline_move` nicht, nur `target_key`.
fn refuse_target(arguments: &Value) -> Result<(), Failure> {
    if arguments.get("target").is_some() {
        return Err(Failure::Args(
            "arguments: target (Stelle) wird nicht angenommen; target_key mitgeben (fester Schlüssel aus outline)."
                .into(),
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
            "arguments: node (Stelle) wird nicht angenommen; key mitgeben (fester Schlüssel aus outline bzw. read_node)."
                .into(),
        ));
    }
    let points = match (a.title, a.items) {
        (Some(title), None) => vec![NewPoint { title, level: a.level }],
        (None, Some(_)) if a.level.is_some() => {
            return Err(Failure::Args("arguments: level nur mit title; bei items je Punkt items[].level".into()));
        }
        (None, Some(items)) => items,
        (Some(_), Some(_)) => return Err(Failure::Args("arguments: entweder title oder items, nicht beides".into())),
        (None, None) => return Err(Failure::Args("arguments: title oder items fehlt".into())),
    };
    if points.is_empty() {
        return Err(Failure::Args("arguments: items ist leer".into()));
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
                        "Antrag {} gestellt, wartet auf Zustimmung: {}. Nichts geändert – der neue Teil übernähme \
                         vorhandene Kapitel; das braucht die aktive Zustimmung des Menschen.",
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
            let title = a.title.ok_or_else(|| Failure::Args("arguments: title fehlt".into()))?;
            OutlineOp::Rename { key, title }
        }
        "outline_move" => {
            refuse_target(&arguments)?;
            let target_key = a.target_key.ok_or_else(|| {
                Failure::Args("arguments: target_key fehlt – das Ziel über seinen festen Schlüssel angeben.".into())
            })?;
            OutlineOp::Move { key, target_key, place: parse_place(a.place.as_deref(), None)? }
        }
        "outline_set_level" => {
            let level = a.level.ok_or_else(|| Failure::Args("arguments: level fehlt (1 bis 3)".into()))?;
            OutlineOp::SetLevel { key, level }
        }
        "outline_set_numbered" => {
            let numbered =
                a.numbered.ok_or_else(|| Failure::Args("arguments: numbered fehlt (true oder false)".into()))?;
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
            "Antrag {} angelegt ({}); nichts geändert. Der Mensch bestätigt oder lehnt ab.",
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
