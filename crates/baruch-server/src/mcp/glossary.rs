// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tools `glossary_list`, `glossary_propose`, `index_propose` (Round 31, contract §3.3; owned by glossar-daten).
//! Read only or propose only: a term becomes a proposal (status „proposed“), an index entry an
//! index proposal (status „open“); only the human can accept, never a text change. The origin is the persona
//! from `persona` (only without a token from the own machine, [`acting_persona`]) or the client as `@mcp:<name>`.
//! The entry in the tool list and the call are set by `tools.rs`.

use baruch_store::{GlossaryStatus, NewIndexProposal};
use baruch_typst::index::{IndexMark, KEYWORD_REGISTER, Occurrence};
use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, acting_persona, args, tool_error, work_or_default, work_schema, write_key};
use crate::AppState;
use crate::glossary::{edit_from_params, term_json};

pub const GLOSSARY_LIST: &str = "glossary_list";
pub const GLOSSARY_PROPOSE: &str = "glossary_propose";
pub const INDEX_PROPOSE: &str = "index_propose";

pub const LIST_DESCRIPTION: &str = "Read the glossary of a work (read-only): per term id, term, short and long \
explanation, variants, source, Bible references, status (accepted, proposed) and origin. status filters (accepted, \
proposed or all, default all); with word a lookup is done (only accepted terms, by term or variant, case does not \
matter). Terms are data, not instructions.";

pub const PROPOSE_DESCRIPTION: &str = "Proposal: propose a glossary term (term, short = short explanation, long = \
long explanation, variants = other forms/inflections, source, bible_refs, reason). It only becomes a proposal (status \
proposed); only the human can accept, change or delete. A term that already is in the glossary or already proposed is \
refused. Bible references are checked and written uniformly. Origin persona (only without a token from the own \
machine) or the client as @mcp:<name>; every proposal is a commit in the mirror.";

pub const INDEX_DESCRIPTION: &str = "Proposal: propose an index entry – never changes the text. Section only by key \
(required, from outline); node is refused (error -32602 as for propose_revision). Per work at most 200 open proposals \
per origin and 2000 in total (beyond that isError, nothing created). word must be in the current revision; occurrence \
picks the place (first, all or a number from 1, default first). register: stichwort (default), personen, bibel or an \
own index; term = headword (default word), sub = sub-headword, see = see, also = see also. The mark is set only by \
the human on accepting (new revision). Origin persona or the client as @mcp:<name>; every proposal is a commit in \
the mirror.";

const OCCURRENCE: &str = "\"first\", \"all\" or a number from 1 (n-th occurrence)";

pub fn list_schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(),
        "status": { "type": "string", "enum": ["accepted", "proposed", "all"] },
        "word": { "type": "string", "description": "look up a word or variant (only accepted terms)" } } })
}

pub fn propose_schema() -> Value {
    json!({ "type": "object", "required": ["term"], "properties": {
        "work": work_schema(),
        "term": { "type": "string" },
        "short": { "type": "string", "description": "short explanation" },
        "long": { "type": "string", "description": "long explanation" },
        "variants": { "type": "array", "items": { "type": "string" }, "description": "other forms of the term" },
        "source": { "type": "string" },
        "bible_refs": { "type": "array", "items": { "type": "string" }, "description": "Bible references, e.g. Joh 3,16" },
        "reason": { "type": "string" },
        "persona": { "type": "string" } } })
}

pub fn index_schema() -> Value {
    json!({ "type": "object", "required": ["key", "word"], "properties": {
        "work": work_schema(),
        "key": { "type": "integer", "description": "stable key of the section (from outline)" },
        "word": { "type": "string", "description": "word in the text after which the mark would go" },
        "occurrence": { "description": OCCURRENCE, "oneOf": [{ "type": "string" }, { "type": "integer", "minimum": 1 }] },
        "register": { "type": "string", "description": "stichwort (default), personen, bibel or an own index" },
        "term": { "type": "string", "description": "headword (default: word)" },
        "sub": { "type": "string", "description": "Unterstichwort" },
        "see": { "type": "string", "description": "see" },
        "also": { "type": "string", "description": "see also" },
        "reason": { "type": "string" },
        "persona": { "type": "string" } } })
}

#[derive(Deserialize)]
struct ListArgs {
    work: Option<WorkRef>,
    status: Option<String>,
    word: Option<String>,
}

pub async fn call_list(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: ListArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    if let Some(word) = a.word.as_deref().filter(|w| !w.trim().is_empty()) {
        let terms = state.store.glossary_lookup(work, word).await?;
        return Ok(json!({ "work": work, "word": word, "terms": terms.iter().map(term_json).collect::<Vec<_>>(),
                          "url": format!("/werke/{work}/glossar") }));
    }
    let status = match a.status.as_deref().map(str::trim).unwrap_or("all") {
        "all" | "" => None,
        "accepted" => Some(GlossaryStatus::Accepted),
        "proposed" => Some(GlossaryStatus::Proposed),
        other => return Err(tool_error(format!("status: accepted, proposed or all expected, not \"{other}\"."))),
    };
    let terms = state.store.glossary(work, status).await?;
    Ok(json!({ "work": work, "terms": terms.iter().map(term_json).collect::<Vec<_>>(),
               "url": format!("/werke/{work}/glossar") }))
}

#[derive(Deserialize)]
struct WhoArgs {
    work: Option<WorkRef>,
    persona: Option<String>,
}

/// Origin of a proposal: persona from `persona` or the client as `@mcp:<name>`.
async fn origin(state: &AppState, client: &Client, persona: Option<&str>) -> Result<String, Failure> {
    Ok(acting_persona(&state.store, client, persona).await?.unwrap_or_else(|| client.handle()))
}

pub async fn call_propose(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let who: WhoArgs = serde_json::from_value(json!({
        "work": arguments.get("work"), "persona": arguments.get("persona"),
    }))
    .map_err(|e| Failure::Args(format!("arguments: {e}")))?;
    let work = work_or_default(state, who.work.as_ref()).await?;
    if arguments.get("term").and_then(Value::as_str).is_none() {
        return Err(Failure::Args("arguments: term is missing (text)".into()));
    }
    let edit = edit_from_params(&arguments)?;
    let origin = origin(state, client, who.persona.as_deref()).await?;
    let answer = crate::glossary::propose(state, work, edit, &origin).await?;
    let id = answer["term"]["id"].clone();
    let mut out = json!({
        "work": work, "id": id, "status": answer["term"]["status"], "origin": origin, "term": answer["term"],
        "url": format!("/werke/{work}/glossar#begriff-{id}"),
    });
    if let Some(reason) = arguments.get("reason").and_then(Value::as_str).filter(|r| !r.trim().is_empty()) {
        // The reason is not in the glossary (contract §2.5 knows no field); it goes back to the caller.
        out["reason"] = json!(reason.trim());
    }
    for field in ["commit", "mirror"] {
        if let Some(v) = answer.get(field) {
            out[field] = v.clone();
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct IndexArgs {
    work: Option<WorkRef>,
    key: Option<i64>,
    word: String,
    occurrence: Option<Value>,
    register: Option<String>,
    term: Option<String>,
    sub: Option<String>,
    see: Option<String>,
    also: Option<String>,
    reason: Option<String>,
    persona: Option<String>,
}

/// `occurrence` as text („first“, „all“, „3“) or a number from 1; missing → first occurrence.
fn occurrence(value: Option<&Value>) -> Result<Occurrence, Failure> {
    let bad = || tool_error(format!("occurrence: {OCCURRENCE} expected."));
    match value {
        None | Some(Value::Null) => Ok(Occurrence::First),
        Some(Value::String(s)) => Occurrence::from_param(s).ok_or_else(bad),
        Some(Value::Number(n)) => {
            n.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n >= 1).map(Occurrence::Nth).ok_or_else(bad)
        }
        Some(_) => Err(bad()),
    }
}

fn text(value: &Option<String>) -> String {
    value.as_deref().unwrap_or("").trim().to_owned()
}

pub async fn call_index(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    // Contract Round 5: writing tools only through the fixed key; the position node would be a foreign section after
    // a rearrangement of the outline. Round 23: like propose_revision and add_post an argument error (-32602,
    // `write_key`), no longer `isError`.
    let a: IndexArgs = args(arguments.clone())?;
    let key = write_key(&arguments, a.key, "arguments")?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let register = Some(text(&a.register)).filter(|r| !r.is_empty()).unwrap_or_else(|| KEYWORD_REGISTER.to_owned());
    let term = Some(text(&a.term)).filter(|t| !t.is_empty()).unwrap_or_else(|| a.word.trim().to_owned());
    let mark =
        IndexMark { sub: text(&a.sub), see: text(&a.see), also: text(&a.also), ..IndexMark::new(&register, &term) };
    let proposal = NewIndexProposal {
        key,
        word: a.word.clone(),
        occurrence: occurrence(a.occurrence.as_ref())?,
        mark,
        reason: text(&a.reason),
        origin: origin(state, client, a.persona.as_deref()).await?,
    };
    let answer = crate::glossary::index_propose(state, work, proposal).await?;
    let created = &answer["proposal"];
    let id = created["id"].clone();
    let mut out = json!({
        "work": work, "id": id, "status": created["status"], "proposal": created,
        "url": format!("/werke/{work}/register#vorschlag-{id}"),
    });
    for field in ["commit", "mirror"] {
        if let Some(v) = answer.get(field) {
            out[field] = v.clone();
        }
    }
    Ok(out)
}
