// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug `text_search` (Runde 30a): im Werk suchen wie `search.find` (Wortlaut, ganzes Wort, regulärer
//! Ausdruck, Bereich), nur lesend. Ersetzen gibt es über MCP nicht; Vorschläge je Abschnitt laufen weiter über
//! `propose_revision`. Das Werkzeug `search` (Volltext, FTS) bleibt unverändert. Grenzen wie `search.find`: Suchtext
//! höchstens 500 Zeichen, höchstens 2000 Treffer (Vorgabe 100 über MCP), 2 s je Aufruf, Regex nur mit dem Crate
//! `regex` und seinen Größengrenzen.

use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;
use crate::search::{SearchQuery, SearchRange, TextKind};

pub const NAME: &str = "text_search";

pub const DESCRIPTION: &str = "Im Werk suchen (nur lesend): Wortlaut, Groß/klein, ganzes Wort oder regulärer \
Ausdruck über Abschnitt, Kapitel, Teil, ab einem Abschnitt oder das ganze Werk; je Treffer Abschnitt (key, Nummer, \
Titel), Fassung, Textart, Stelle (Byte und UTF-16), Zeile, Umfeld und Adresse (url) zum Abschnitt. Vorgabe-Textarten: \
text, heading, footnote, quote. Grenzen: höchstens 500 Zeichen Suchtext, höchstens 2000 Treffer (Vorgabe 100), 2 s. \
Ersetzen gibt es über MCP nicht – Vorschläge je Abschnitt über propose_revision.";

pub fn schema() -> Value {
    json!({ "type": "object", "required": ["work", "q"], "properties": {
        "work": work_schema(),
        "q": { "type": "string", "description": "Suchtext (höchstens 500 Zeichen)" },
        "case": { "type": "boolean", "description": "Groß/klein beachten (Vorgabe false)" },
        "word": { "type": "boolean", "description": "nur ganze Wörter (Vorgabe false)" },
        "re": { "type": "boolean", "description": "regulärer Ausdruck (Vorgabe false; Syntax des Rust-Crates regex)" },
        "range": { "type": "string", "enum": ["section", "chapter", "part", "from", "work"],
                   "description": "Bereich (Vorgabe work)" },
        "key": { "type": "integer", "description": "fester Schlüssel des Abschnitts (bei section, chapter, part, from)" },
        "kinds": { "type": "array", "items": { "type": "string", "enum": ["text", "heading", "footnote", "quote", "code"] },
                   "description": "Textarten (Vorgabe text, heading, footnote, quote)" },
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

/// Vorgabe der Trefferzahl über MCP (kleiner als in der Oberfläche: Antworten gehen an ein Modell).
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
                    "arguments: range section, chapter, part, from oder work, nicht „{word}“"
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
                    Failure::Args(format!("arguments: kinds text, heading, footnote, quote oder code, nicht „{word}“"))
                })?);
            }
            Some(out)
        }
    };
    if let Some(limit) = a.limit
        && !(1..=crate::search::MAX_HITS).contains(&limit)
    {
        return Err(Failure::Args("arguments: limit 1 bis 2000".into()));
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
