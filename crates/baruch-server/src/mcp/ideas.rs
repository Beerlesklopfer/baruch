// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeuge für Einfälle (Vertrag Runde 7, Abschnitt 6f): `idea_create` (auch als `add_idea`), `idea_read`,
//! `idea_list`, `idea_search`, `idea_update`, `idea_delete`.
//!
//! Ausnahme von der Assistenten-Regel (Entscheidung IDEA_CRUD): Einfälle ändern keinen Buchtext,
//! darum dürfen Assistenten und MCP-Clients sie anlegen, ändern und löschen (wiederherstellbar über
//! `idea.restore` und den Git-Spiegel). Herkunft ist `persona` (muss es geben, sonst `isError`) oder
//! der Client als `@mcp:<name>`; jede Aktion ist ein Commit mit dieser Herkunft
//! ([`crate::mirror::record_idea`]). Ergebnisse wie die RPC-Methoden ([`crate::ideas`]).
//!
//! Verarbeiten (Gliederungspunkt, Beitrag, Notiz) bleibt dem Menschen in der Oberfläche bzw. über RPC.
// Der Lektor kennt die Einfälle als Gedächtnis (Runde 10, `crate::assist::context`); ein eigenes Werkzeug
// dafür gibt es nicht (ask_persona/chat genügen).

use baruch_store::{IdeaEdit, IdeaFilter, IdeaOrigin, IdeaScope, IdeaStatus, NewIdea, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, resolve, tool_error, work_schema};
use crate::AppState;
use crate::rpc::RpcError;

/// Namen der Werkzeuge dieses Moduls (in der Reihenfolge von `tools/list`).
/// `add_idea` ist ein zweiter Name für `idea_create` (Wunsch des Nutzers 2026-11-06, Name aus dem Backlog).
pub const NAMES: &[&str] =
    &["idea_create", "add_idea", "idea_read", "idea_list", "idea_search", "idea_update", "idea_delete"];

fn words_schema() -> Value {
    json!({ "description": "Schlagwörter: Liste oder Text „a, b; c“ (1 bis 20, je höchstens 60 Zeichen, ohne Zeilenumbruch)",
            "oneOf": [{ "type": "array", "items": { "type": "string" } }, { "type": "string" }] })
}

fn persona_schema() -> Value {
    json!({ "type": "string", "description": "Persona-Handle (z. B. @lektor) als Herkunft (nur ohne Token vom eigenen Rechner); sonst der Client als @mcp:<name>" })
}

pub fn create_schema() -> Value {
    json!({ "type": "object", "required": ["words"], "properties": {
        "work": work_schema(), "key": { "type": "integer", "description": "Bezug: fester Schlüssel eines Abschnitts des Werks (key aus outline)" },
        "words": words_schema(),
        "note": { "type": "string", "description": "Gedanke (höchstens 4000 Zeichen)" },
        "quote": { "type": "string", "description": "markierter Text (höchstens 4000 Zeichen)" },
        "source": { "type": "string", "description": "Quelle oder Hervorhebung (höchstens 200 Zeichen)" },
        "bible": { "type": "string", "description": "Bibelstelle (höchstens 200 Zeichen)" },
        "persona": persona_schema() } })
}

pub fn read_schema() -> Value {
    json!({ "type": "object", "required": ["id"], "properties": { "id": { "type": "integer" } } })
}

pub fn list_schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": { "description": "Werk (id oder slug) oder \"none\" für Einfälle ohne Werk; fehlt: alle",
                  "oneOf": [{ "type": "integer" }, { "type": "string" }] },
        "word": { "type": "string", "description": "nur Einfälle mit diesem Schlagwort" },
        "open": { "type": "boolean", "description": "true: nur offene, false: nur verarbeitete; fehlt: alle" } } })
}

pub fn search_schema() -> Value {
    json!({ "type": "object", "required": ["query"], "properties": {
        "query": { "type": "string", "description": "Wörter (Volltextsuche über Schlagwörter, Gedanke, Markierung, Quelle, Bibelstelle)" },
        "work": { "description": "Werk (id oder slug) oder \"none\"; fehlt: alle", "oneOf": [{ "type": "integer" }, { "type": "string" }] },
        "mode": { "type": "string", "enum": ["fts", "rag"], "description": "fts (Vorgabe): Volltext; rag: Bedeutungssuche \
            (Runde 19c, nur mit work; Treffer mit score und weak = schwach unter der Untergrenze; Ausfall der \
            Bedeutungssuche als isError mit Grund)" } } })
}

pub fn update_schema() -> Value {
    json!({ "type": "object", "required": ["id"], "properties": {
        "id": { "type": "integer" }, "words": words_schema(),
        "note": { "type": "string" }, "quote": { "type": "string" }, "source": { "type": "string" }, "bible": { "type": "string" },
        "open": { "type": "boolean", "description": "true: wieder offen, false: als verarbeitet markieren" },
        "work": { "description": "Werk (id oder slug), null = ohne Werk", "oneOf": [{ "type": "integer" }, { "type": "string" }, { "type": "null" }] },
        "key": { "description": "Bezug (key aus outline), null = ohne", "oneOf": [{ "type": "integer" }, { "type": "null" }] },
        "persona": persona_schema() } })
}

pub fn delete_schema() -> Value {
    json!({ "type": "object", "required": ["id"], "properties": { "id": { "type": "integer" }, "persona": persona_schema() } })
}

/// Herkunft: `persona` (muss es geben) oder der Client als `@mcp:<name>`.
async fn origin(state: &AppState, client: &Client, persona: Option<&str>) -> Result<IdeaOrigin, Failure> {
    super::tools::origin_of(&state.store, client, persona).await
}

/// Werkfilter: fehlt → alle, `"none"` → ohne Werk, sonst id oder slug.
async fn scope(state: &AppState, work: Option<&WorkRef>) -> Result<IdeaScope, Failure> {
    match work {
        None => Ok(IdeaScope::All),
        Some(WorkRef::Slug(s)) if s == "none" => Ok(IdeaScope::NoWork),
        Some(WorkRef::Slug(s)) if s == "all" => Ok(IdeaScope::All),
        Some(work) => Ok(IdeaScope::Work(resolve(&state.store, work).await?.0)),
    }
}

/// Schlagwörter aus Liste oder Text.
fn words(value: &Value) -> Result<Vec<String>, Failure> {
    crate::ideas::split_words(value, "words").map_err(|e| Failure::Args(format!("arguments: {}", e.message)))
}

#[derive(Deserialize)]
struct CreateArgs {
    work: Option<WorkRef>,
    key: Option<i64>,
    words: Value,
    #[serde(default)]
    note: String,
    #[serde(default)]
    quote: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    bible: String,
    persona: Option<String>,
}

#[derive(Deserialize)]
struct IdArgs {
    id: i64,
    persona: Option<String>,
}

#[derive(Deserialize)]
struct ListArgs {
    work: Option<WorkRef>,
    word: Option<String>,
    open: Option<bool>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    work: Option<WorkRef>,
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Deserialize)]
struct UpdateArgs {
    id: i64,
    words: Option<Value>,
    note: Option<String>,
    quote: Option<String>,
    source: Option<String>,
    bible: Option<String>,
    open: Option<bool>,
    persona: Option<String>,
}

fn status(open: Option<bool>) -> Option<IdeaStatus> {
    open.map(|open| if open { IdeaStatus::Open } else { IdeaStatus::Done })
}

/// `url` eines Einfalls (als JSON) ergänzen (Vertrag Runde 8, §1): mit Bezug `/?work=W&key=K`, nur mit
/// Werk `/?work=W`, sonst `null`.
pub(crate) fn add_url(idea: &mut Value) {
    if !idea.is_object() {
        return;
    }
    idea["url"] = match (idea["work"].as_i64(), idea["key"].as_i64()) {
        (Some(work), Some(key)) => json!(super::tools::node_url(work, key)),
        (Some(work), None) => json!(format!("/?work={work}")),
        _ => Value::Null,
    };
}

/// In einer Antwort jeden Einfall (`idea` bzw. jeden in `ideas`) mit `url` versehen.
fn with_urls(mut answer: Value) -> Value {
    if let Some(idea) = answer.get_mut("idea") {
        add_url(idea);
    }
    if let Some(Value::Array(ideas)) = answer.get_mut("ideas") {
        ideas.iter_mut().for_each(add_url);
    }
    answer
}

/// Werkzeug ausführen; `None`: kein Werkzeug dieses Moduls. Jeder Einfall der Antwort trägt `url`.
pub async fn call(state: &AppState, client: &Client, name: &str, arguments: Value) -> Option<Result<Value, Failure>> {
    let answer = run(state, client, name, arguments).await?;
    Some(answer.map(with_urls))
}

async fn run(state: &AppState, client: &Client, name: &str, arguments: Value) -> Option<Result<Value, Failure>> {
    Some(match name {
        "idea_create" | "add_idea" => idea_create(state, client, arguments).await,
        "idea_read" => idea_read(state, arguments).await,
        "idea_list" => idea_list(state, arguments).await,
        "idea_search" => idea_search(state, arguments).await,
        "idea_update" => idea_update(state, client, arguments).await,
        "idea_delete" => idea_delete(state, client, arguments).await,
        _ => return None,
    })
}

fn from_rpc(e: RpcError) -> Failure {
    e.into()
}

async fn idea_create(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: CreateArgs = args(arguments)?;
    let words = words(&a.words)?;
    let work: Option<WorkId> = match &a.work {
        Some(work) => Some(resolve(&state.store, work).await?.0),
        None => None,
    };
    let origin = origin(state, client, a.persona.as_deref()).await?;
    let new = NewIdea {
        work,
        key: a.key,
        words: &words,
        note: &a.note,
        quote: &a.quote,
        source: &a.source,
        bible: &a.bible,
        origin,
    };
    crate::ideas::create(state, &new).await.map_err(from_rpc)
}

async fn idea_read(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: IdArgs = args(arguments)?;
    Ok(json!({ "idea": state.store.idea_get(a.id).await? }))
}

async fn idea_list(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: ListArgs = args(arguments)?;
    let filter = IdeaFilter {
        scope: scope(state, a.work.as_ref()).await?,
        word: a.word.filter(|w| !w.trim().is_empty()),
        status: status(a.open),
        ..IdeaFilter::default()
    };
    let ideas = state.store.idea_list(&filter).await?;
    let words = state.store.idea_words(&filter).await?;
    Ok(json!({ "ideas": ideas, "words": words }))
}

async fn idea_search(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: SearchArgs = args(arguments)?;
    // Runde 19c: Bedeutungssuche wie RPC `idea.search` mit `mode: "rag"` (nur lesend); ein Ausfall (-32001) wird
    // isError mit dem Grund, schwache Treffer tragen `weak`.
    match a.mode.as_deref().map(str::trim) {
        None | Some("") | Some("fts") => {}
        Some("rag") => {
            let work = match scope(state, a.work.as_ref()).await? {
                IdeaScope::Work(work) => work,
                _ => return Err(tool_error("Bedeutungssuche nur innerhalb eines Werks (work: id oder slug).")),
            };
            let params = json!({ "query": a.query, "work": work, "mode": "rag" });
            return crate::ideas::call(state, "idea.search", params).await.map_err(from_rpc);
        }
        Some(_) => return Err(Failure::Args("arguments: mode fts oder rag erwartet".into())),
    }
    let Some(fts) = baruch_library::fts_query(&a.query) else {
        return Err(tool_error("Suchbegriff fehlt: mindestens ein Wort mit Buchstaben oder Ziffern."));
    };
    let filter = IdeaFilter { scope: scope(state, a.work.as_ref()).await?, ..IdeaFilter::default() };
    let found = state.store.idea_search(&fts, &filter).await?;
    let hits: Vec<Value> = found.iter().map(|(idea, snippet)| json!({ "id": idea.id, "snippet": snippet })).collect();
    let ideas: Vec<&baruch_store::Idea> = found.iter().map(|(idea, _)| idea).collect();
    Ok(json!({ "ideas": ideas, "hits": hits }))
}

async fn idea_update(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: UpdateArgs = args(arguments.clone())?;
    let words = a.words.as_ref().filter(|w| !w.is_null()).map(words).transpose()?;
    // `work`/`key`: fehlt = bleibt, null = ohne.
    let work = match arguments.get("work") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => {
            let work: WorkRef = serde_json::from_value(value.clone())
                .map_err(|_| Failure::Args("arguments: work als id oder slug erwartet".into()))?;
            Some(Some(resolve(&state.store, &work).await?.0))
        }
    };
    let key = match arguments.get("key") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => {
            Some(Some(value.as_i64().ok_or_else(|| Failure::Args("arguments: key als Zahl erwartet".into()))?))
        }
    };
    let origin = origin(state, client, a.persona.as_deref()).await?;
    let edit = IdeaEdit {
        words: words.as_deref(),
        note: a.note.as_deref(),
        quote: a.quote.as_deref(),
        source: a.source.as_deref(),
        bible: a.bible.as_deref(),
        work,
        key,
        status: status(a.open),
    };
    crate::ideas::update(state, a.id, &edit, &origin).await.map_err(from_rpc)
}

async fn idea_delete(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: IdArgs = args(arguments)?;
    let origin = origin(state, client, a.persona.as_deref()).await?;
    crate::ideas::delete(state, a.id, &origin).await.map_err(from_rpc)
}
