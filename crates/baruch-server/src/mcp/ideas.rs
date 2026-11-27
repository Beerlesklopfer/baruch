// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tools for ideas (contract Round 7, section 6f): `idea_create` (also as `add_idea`), `idea_read`,
//! `idea_list`, `idea_search`, `idea_update`, `idea_delete`; since Round 34 (mind map) `idea_move` (re-parent,
//! order, into the cloud) and on `idea_create`/`idea_update` `title`, `parent`, `before`, `cloud`.
//!
//! Exception from the assistant rule (decision IDEA_CRUD): ideas change no book text,
//! therefore assistants and MCP clients may create, change and delete them (restorable through
//! `idea.restore` and the Git mirror). The origin is `persona` (must exist, otherwise `isError`) or
//! the client as `@mcp:<name>`; every action is a commit with this origin
//! ([`crate::mirror::record_idea`]). Results like the RPC methods ([`crate::ideas`]).
//!
//! Processing (outline item, post, note) stays with the human in the user interface or through RPC.
// The lector knows the ideas as memory (Round 10, `crate::assist::context`); there is no tool of its
// own for that (ask_persona/chat are enough).

use baruch_store::{IdeaEdit, IdeaFilter, IdeaMove, IdeaOrigin, IdeaScope, IdeaSpot, IdeaStatus, NewIdea, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, resolve, tool_error, work_schema};
use crate::AppState;
use crate::rpc::RpcError;

/// Names of the tools of this module (in the order of `tools/list`).
/// `add_idea` is a second name for `idea_create` (wish of the user 2026-11-06, name from the backlog).
pub const NAMES: &[&str] =
    &["idea_create", "add_idea", "idea_read", "idea_list", "idea_search", "idea_update", "idea_delete", "idea_move"];

fn words_schema() -> Value {
    json!({ "description": "keywords: list or text \"a, b; c\" (1 to 20, at most 60 characters each, no line break)",
            "oneOf": [{ "type": "array", "items": { "type": "string" } }, { "type": "string" }] })
}

fn persona_schema() -> Value {
    json!({ "type": "string", "description": "persona handle (e.g. @lektor) as origin (only without a token from the own machine); otherwise the client as @mcp:<name>" })
}

pub fn create_schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(), "key": { "type": "integer", "description": "relation: stable key of a section of the work (key from outline)" },
        "words": words_schema(),
        "title": { "type": "string", "description": "title of the node in the mind map (at most 200 characters); with a title the keywords may be missing – one of the two is required" },
        "parent": { "type": "integer", "description": "parent node: id of an idea of the same work (sub-node in the mind map; work may then be missing)" },
        "before": { "type": "integer", "description": "id of a sibling the new idea goes before; missing: to the end" },
        "cloud": { "type": "boolean", "description": "false: without parent and key a free main topic directly at the work; missing or true: without a place into the cloud beside the mind map" },
        "note": { "type": "string", "description": "thought (at most 4000 characters)" },
        "quote": { "type": "string", "description": "marked text (at most 4000 characters)" },
        "source": { "type": "string", "description": "source or highlight (at most 200 characters)" },
        "bible": { "type": "string", "description": "Bible reference (at most 200 characters)" },
        "persona": persona_schema() } })
}

pub fn read_schema() -> Value {
    json!({ "type": "object", "required": ["id"], "properties": { "id": { "type": "integer" } } })
}

pub fn list_schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": { "description": "work (id or slug) or \"none\" for ideas without a work; missing: all",
                  "oneOf": [{ "type": "integer" }, { "type": "string" }] },
        "word": { "type": "string", "description": "only ideas with this keyword" },
        "open": { "type": "boolean", "description": "true: only open ones, false: only processed ones; missing: all" } } })
}

pub fn search_schema() -> Value {
    json!({ "type": "object", "required": ["query"], "properties": {
        "query": { "type": "string", "description": "words (full-text search over keywords, thought, marked text, source, Bible reference)" },
        "work": { "description": "work (id or slug) or \"none\"; missing: all", "oneOf": [{ "type": "integer" }, { "type": "string" }] },
        "mode": { "type": "string", "enum": ["fts", "rag"], "description": "fts (default): full text; rag: semantic search \
            (Round 19c, only with work; hits with score and weak = below the lower bound; failure of the \
            semantic search as isError with reason)" } } })
}

pub fn update_schema() -> Value {
    json!({ "type": "object", "required": ["id"], "properties": {
        "id": { "type": "integer" }, "words": words_schema(),
        "title": { "type": "string", "description": "title of the node in the mind map (at most 200 characters; empty only while a keyword remains)" },
        "note": { "type": "string" }, "quote": { "type": "string" }, "source": { "type": "string" }, "bible": { "type": "string" },
        "open": { "type": "boolean", "description": "true: open again, false: mark as processed" },
        "work": { "description": "work (id or slug), null = without a work", "oneOf": [{ "type": "integer" }, { "type": "string" }, { "type": "null" }] },
        "key": { "description": "relation (key from outline), null = none", "oneOf": [{ "type": "integer" }, { "type": "null" }] },
        "persona": persona_schema() } })
}

pub fn delete_schema() -> Value {
    json!({ "type": "object", "required": ["id"], "properties": { "id": { "type": "integer" }, "persona": persona_schema() } })
}

pub const MOVE_DESCRIPTION: &str = "Re-attach and order an idea in the mind map: parent (id of an idea of the same \
work, null = detach from the parent node), key (relation to a section, null = none), before (id of a sibling, null = \
to the end), cloud (true = into the cloud beside the mind map, removes parent node and relation; false = without \
both a free main topic at the work). Missing fields stay; the sub-nodes move along. Refused (isError): another work, \
own sub-node (cycle), deeper than 32, deleted idea. Chapters and parts cannot be changed this way (only outline_* \
with a request). Changes no book text; origin persona or @mcp:<name>, one commit in the mirror.";

pub fn move_schema() -> Value {
    json!({ "type": "object", "required": ["id"], "properties": {
        "id": { "type": "integer", "description": "id of the idea" },
        "parent": { "description": "new parent node (id of an idea), null = detach", "oneOf": [{ "type": "integer" }, { "type": "null" }] },
        "key": { "description": "relation (key from outline), null = none", "oneOf": [{ "type": "integer" }, { "type": "null" }] },
        "before": { "description": "sibling the idea goes before (id), null = to the end", "oneOf": [{ "type": "integer" }, { "type": "null" }] },
        "cloud": { "type": "boolean", "description": "true: into the cloud; false: without parent node and relation a main topic at the work" },
        "persona": persona_schema() } })
}

/// Origin: `persona` (must exist) or the client as `@mcp:<name>`.
async fn origin(state: &AppState, client: &Client, persona: Option<&str>) -> Result<IdeaOrigin, Failure> {
    super::tools::origin_of(&state.store, client, persona).await
}

/// Work filter: missing → all, `"none"` → without a work, otherwise id or slug.
async fn scope(state: &AppState, work: Option<&WorkRef>) -> Result<IdeaScope, Failure> {
    match work {
        None => Ok(IdeaScope::All),
        Some(WorkRef::Slug(s)) if s == "none" => Ok(IdeaScope::NoWork),
        Some(WorkRef::Slug(s)) if s == "all" => Ok(IdeaScope::All),
        Some(work) => Ok(IdeaScope::Work(resolve(&state.store, work).await?.0)),
    }
}

/// Keywords from a list or text.
fn words(value: &Value) -> Result<Vec<String>, Failure> {
    crate::ideas::split_words(value, "words").map_err(|e| Failure::Args(format!("arguments: {}", e.message)))
}

#[derive(Deserialize)]
struct CreateArgs {
    work: Option<WorkRef>,
    key: Option<i64>,
    #[serde(default)]
    words: Value,
    #[serde(default)]
    title: String,
    parent: Option<i64>,
    before: Option<i64>,
    cloud: Option<bool>,
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
    title: Option<String>,
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

/// Add the `url` of an idea (as JSON) (contract Round 8, §1): with a reference `/?work=W&key=K`, with only
/// a work `/?work=W`, otherwise `null`.
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

/// Give every idea in an answer (`idea` or each one in `ideas`) a `url`.
fn with_urls(mut answer: Value) -> Value {
    if let Some(idea) = answer.get_mut("idea") {
        add_url(idea);
    }
    if let Some(Value::Array(ideas)) = answer.get_mut("ideas") {
        ideas.iter_mut().for_each(add_url);
    }
    answer
}

/// Run the tool; `None`: not a tool of this module. Every idea of the answer carries `url`.
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
        "idea_move" => idea_move(state, client, arguments).await,
        _ => return None,
    })
}

fn from_rpc(e: RpcError) -> Failure {
    e.into()
}

async fn idea_create(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: CreateArgs = args(arguments)?;
    // Round 34: with a title the keywords may be missing.
    let words = if a.words.is_null() {
        if a.title.trim().is_empty() {
            return Err(Failure::Args("arguments: words is missing (at least one keyword or a title)".into()));
        }
        Vec::new()
    } else {
        words(&a.words)?
    };
    let mut work: Option<WorkId> = match &a.work {
        Some(work) => Some(resolve(&state.store, work).await?.0),
        None => None,
    };
    if let (None, Some(parent)) = (work, a.parent) {
        // A child node belongs to the work of its parent node.
        work = state.store.idea_get(parent).await?.work;
    }
    if a.cloud == Some(true) && (a.parent.is_some() || a.key.is_some()) {
        return Err(Failure::Args("arguments: cloud true works only without parent and key".into()));
    }
    let spot = IdeaSpot { title: &a.title, parent: a.parent, before: a.before, main_topic: a.cloud == Some(false) };
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
    crate::ideas::create_at(state, &new, &spot).await.map_err(from_rpc)
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
    // Round 19c: semantic search like RPC `idea.search` with `mode: "rag"` (read-only); a failure (-32001) becomes
    // isError with the reason, weak hits carry `weak`.
    match a.mode.as_deref().map(str::trim) {
        None | Some("") | Some("fts") => {}
        Some("rag") => {
            let work = match scope(state, a.work.as_ref()).await? {
                IdeaScope::Work(work) => work,
                _ => return Err(tool_error("Semantic search only within a work (work: id or slug).")),
            };
            let params = json!({ "query": a.query, "work": work, "mode": "rag" });
            return crate::ideas::call(state, "idea.search", params).await.map_err(from_rpc);
        }
        Some(_) => return Err(Failure::Args("arguments: mode fts or rag expected".into())),
    }
    let Some(fts) = baruch_library::fts_query(&a.query) else {
        return Err(tool_error("Search term is missing: at least one word with letters or digits."));
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
    // `work`/`key`: missing = stays, null = without.
    let work = match arguments.get("work") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => {
            let work: WorkRef = serde_json::from_value(value.clone())
                .map_err(|_| Failure::Args("arguments: work as id or slug expected".into()))?;
            Some(Some(resolve(&state.store, &work).await?.0))
        }
    };
    let key = match arguments.get("key") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(value) => {
            Some(Some(value.as_i64().ok_or_else(|| Failure::Args("arguments: key as a number expected".into()))?))
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
        title: a.title.as_deref(),
    };
    crate::ideas::update(state, a.id, &edit, &origin).await.map_err(from_rpc)
}

async fn idea_delete(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: IdArgs = args(arguments)?;
    let origin = origin(state, client, a.persona.as_deref()).await?;
    crate::ideas::delete(state, a.id, &origin).await.map_err(from_rpc)
}

/// `parent`/`key`/`before` of `idea_move`: missing = stays, `null` = without, number = set.
fn optional_id(arguments: &Value, name: &str) -> Result<Option<Option<i64>>, Failure> {
    match arguments.get(name) {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(value) => value
            .as_i64()
            .map(|n| Some(Some(n)))
            .ok_or_else(|| Failure::Args(format!("arguments: {name} as a number or null expected"))),
    }
}

/// Round 34: re-parent and order an idea like RPC `idea.move`, with origin persona or `@mcp:<client>`. A
/// violated rule (other work, cycle, depth, deleted) is a domain error (isError), nothing changed.
async fn idea_move(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    #[derive(Deserialize)]
    struct MoveArgs {
        id: i64,
        cloud: Option<bool>,
        persona: Option<String>,
    }
    let a: MoveArgs = args(arguments.clone())?;
    let mv = IdeaMove {
        parent: optional_id(&arguments, "parent")?,
        key: optional_id(&arguments, "key")?,
        before: optional_id(&arguments, "before")?,
        cloud: a.cloud,
    };
    let origin = origin(state, client, a.persona.as_deref()).await?;
    crate::ideas::move_idea(state, a.id, &mv, &origin).await.map_err(from_rpc)
}
