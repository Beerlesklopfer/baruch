// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tool `text_search` (Round 30a): search in the work like `search.find` (wording, whole word, regular
//! expression, range), read-only. There is no replacing through MCP; proposals per section keep going through
//! `propose_revision`. The tool `search` (full text, FTS) stays unchanged. Limits as for `search.find`: search text
//! at most 500 characters, at most 2000 hits (default 100 through MCP), 2 s per call, regex only with the crate
//! `regex` and its size limits.

use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;
use crate::search::{SearchQuery, SearchRange, TextKind};

pub const NAME: &str = "text_search";

pub const DESCRIPTION: &str = "Search in the work (read-only): wording, case, whole word or regular expression over \
section, chapter, part, from a section on or the whole work; per hit section (key, number, title), revision, text \
kind, place (byte and UTF-16), line, context and address (url) of the section. Default text kinds: text, heading, \
footnote, quote. Limits: at most 500 characters of search text, at most 2000 hits (default 100), 2 s. Replacing \
does not exist over MCP – proposals per section through propose_revision.";

pub fn schema() -> Value {
    json!({ "type": "object", "required": ["work", "q"], "properties": {
        "work": work_schema(),
        "q": { "type": "string", "description": "search text (at most 500 characters)" },
        "case": { "type": "boolean", "description": "case-sensitive (default false)" },
        "word": { "type": "boolean", "description": "whole words only (default false)" },
        "re": { "type": "boolean", "description": "regular expression (default false; syntax of the Rust crate regex)" },
        "range": { "type": "string", "enum": ["section", "chapter", "part", "from", "work"],
                   "description": "range (default work)" },
        "key": { "type": "integer", "description": "stable key of the section (for section, chapter, part, from)" },
        "kinds": { "type": "array", "items": { "type": "string", "enum": ["text", "heading", "footnote", "quote", "code"] },
                   "description": "text kinds (default text, heading, footnote, quote)" },
        "limit": { "type": "integer", "minimum": 1, "maximum": 2000 } } })
}

#[derive(Deserialize)]
struct SearchArgs {
    work: WorkRef,
    q: String,
    #[serde(default)]
    case: bool,
    #[serde(default)]
    word: bool,
    #[serde(default)]
    re: bool,
    range: Option<String>,
    key: Option<i64>,
    kinds: Option<Vec<String>>,
    limit: Option<usize>,
}

/// Default number of hits through MCP (smaller than in the user interface: answers go to a model).
const MCP_DEFAULT_LIMIT: usize = 100;

pub async fn call_tool(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let _ = client;
    let a: SearchArgs = args(arguments)?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    let range = match a.range.as_deref().map(str::trim) {
        None | Some("") => SearchRange::Work,
        Some(word) => match SearchRange::parse(word) {
            Some(
                r @ (SearchRange::Section
                | SearchRange::Chapter
                | SearchRange::Part
                | SearchRange::From
                | SearchRange::Work),
            ) => r,
            _ => {
                return Err(Failure::Args(format!(
                    "arguments: range section, chapter, part, from or work, not \"{word}\""
                )));
            }
        },
    };
    let kinds = match a.kinds {
        None => None,
        Some(list) => {
            let mut out = Vec::new();
            for word in list {
                out.push(TextKind::parse(word.trim()).ok_or_else(|| {
                    Failure::Args(format!("arguments: kinds text, heading, footnote, quote or code, not \"{word}\""))
                })?);
            }
            Some(out)
        }
    };
    if let Some(limit) = a.limit
        && !(1..=crate::search::MAX_HITS).contains(&limit)
    {
        return Err(Failure::Args("arguments: limit 1 to 2000".into()));
    }
    let query = SearchQuery {
        q: a.q,
        case: a.case,
        word: a.word,
        regex: a.re,
        range,
        key: a.key,
        kinds,
        limit: Some(a.limit.unwrap_or(MCP_DEFAULT_LIMIT)),
        ..SearchQuery::default()
    };
    let result = crate::search::find(state, work, &query).await?;
    Ok(json!({
        "work": work,
        "range": result.range_label,
        "total": result.total,
        "truncated": result.truncated,
        "timed_out": result.timed_out,
        "hits": result.hits,
    }))
}
