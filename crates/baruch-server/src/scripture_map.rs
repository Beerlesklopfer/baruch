// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stellen zwischen Verszählungen umrechnen (Runde 17a): RPC `scripture.map` und `scripture.versifications`, MCP
//! `scripture_map` ([`crate::mcp::scripture_map`]). Nur lesend, ohne Werk.
//!
//! Eine Angabe („Joel 2,28–32“) wird mit [`baruch_scripture::parse`] gelesen (Buchnamen deutsch oder OSIS) und gegen
//! die Zählung `from` geprüft (Vorgabe KJV) – nicht gegen die deutsche Zählung der Stellenprüfung. Dann bildet
//! [`baruch_scripture::versification::mapping::map_range`] sie in die Zählung `to` ab (Vorgabe German). Was keine
//! Regel deckt und in der Kapitellänge abweicht, ist „unmapped“ mit Grund – nie geraten.

use baruch_scripture::versification::mapping::{MappedRange, VerseRef, map_range};
use baruch_scripture::versification::{Versification, names, versification};
use baruch_scripture::{Found, books, parse};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Längste Angabe (Zeichen).
pub const MAX_REF_CHARS: usize = 500;
/// Höchstens so viele Verse je Stelle in der Antwort.
pub const MAX_VERSES: usize = 1000;
/// Vorgabe der Ausgangszählung.
pub const DEFAULT_FROM: &str = "KJV";
/// Vorgabe der Zielzählung.
pub const DEFAULT_TO: &str = "German";

const METHODS: &[&str] = &["scripture.map", "scripture.versifications"];

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC `scripture.map` bzw. `scripture.versifications` (`docs/RPC.md`, „Stellenprüfung“).
pub async fn call(_state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "scripture.map" => map_json(&params),
        "scripture.versifications" => Ok(versifications_json()),
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Zählung zu einem Namen; unbekannt → -32602 mit der Liste.
fn named(params: &Value, field: &str, default: &str) -> Result<&'static Versification, RpcError> {
    let name = match params.get(field) {
        None | Some(Value::Null) => default,
        Some(Value::String(s)) => s.as_str(),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, format!("{field}: Name einer Verszählung als Text"))),
    };
    versification(name).ok_or_else(|| {
        RpcError::new(
            INVALID_PARAMS,
            format!("{field}: Verszählung „{}“ unbekannt; bekannt sind {}", name.trim(), names().join(", ")),
        )
    })
}

/// `scripture.map {ref, from?, to?}` (siehe Moduldoku).
pub fn map_json(params: &Value) -> Result<Value, RpcError> {
    let reference = params
        .get("ref")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "ref fehlt (Stellenangabe, z. B. „Joel 2,28–32“)"))?;
    if reference.chars().count() > MAX_REF_CHARS {
        return Err(RpcError::new(INVALID_PARAMS, format!("ref: höchstens {MAX_REF_CHARS} Zeichen")));
    }
    let from = named(params, "from", DEFAULT_FROM)?;
    let to = named(params, "to", DEFAULT_TO)?;
    let refs: Vec<Value> = parse(reference).iter().map(|found| entry(from, to, found)).collect();
    Ok(json!({ "from": from.name, "to": to.name, "refs": refs, "versifications": names() }))
}

/// Eine Stelle umgerechnet.
fn entry(from: &Versification, to: &Versification, found: &Found) -> Value {
    let invalid = |message: String| {
        json!({ "input": found.input, "normalized_from": found.normalized, "status": "invalid", "target": null,
                "verses": [], "rules": [], "message": message })
    };
    let Some(passage) = found.passage else {
        return invalid(found.message());
    };
    let book = passage.book;
    if from.count_range(book.id, passage.start, passage.end).is_none() {
        return invalid(range_problem(from, book.id, passage.start, passage.end));
    }
    match map_range(from, to, book.id, passage.start, passage.end, MAX_VERSES) {
        Ok(mapped) => {
            let same = mapped.rules.is_empty() && same_verses(from, book.id, passage.start, passage.end, &mapped);
            let target = label(to, &mapped.verses);
            let mut message = if same {
                format!("gleich in {}: {}", to.name, target.as_deref().unwrap_or("-"))
            } else {
                format!("in {}: {}", to.name, target.as_deref().unwrap_or("-"))
            };
            if !mapped.rules.is_empty() {
                message.push_str(&format!(" (Regel: {})", mapped.rules.join("; ")));
            }
            if mapped.truncated {
                message.push_str(&format!(" – gekürzt auf {MAX_VERSES} Verse"));
            }
            let verses: Vec<Value> = mapped
                .verses
                .iter()
                .map(|v| json!({ "book": v.book, "chapter": v.chapter, "verse": v.verse }))
                .collect();
            json!({ "input": found.input, "normalized_from": found.normalized,
                    "status": if same { "same" } else { "moved" }, "target": target, "verses": verses,
                    "rules": mapped.rules, "message": message })
        }
        Err(reason) => json!({ "input": found.input, "normalized_from": found.normalized, "status": "unmapped",
                               "target": null, "verses": [], "rules": [], "message": reason }),
    }
}

/// Grund, warum es die Stelle in `from` nicht gibt.
fn range_problem(from: &Versification, book: &str, start: (u16, u16), end: (u16, u16)) -> String {
    let abbr = books().iter().find(|b| b.id == book).map_or(book, |b| b.abbr);
    let Some(chapters) = from.chapters_of(book) else {
        return format!("das Buch {abbr} gibt es in {} nicht", from.name);
    };
    for (chapter, verse) in [start, end] {
        match from.verses_in(book, chapter) {
            None => return format!("{abbr} hat in {} nur {} Kapitel", from.name, chapters.len()),
            Some(n) if verse > n => return format!("{abbr} {chapter} hat in {} nur {n} Verse", from.name),
            Some(_) => {}
        }
    }
    format!("{abbr}: Ende vor dem Anfang")
}

/// Sind die abgebildeten Verse genau die Verse der Stelle in `from`?
fn same_verses(from: &Versification, book: &str, start: (u16, u16), end: (u16, u16), mapped: &MappedRange) -> bool {
    let mut expected = Vec::new();
    for chapter in start.0..=end.0 {
        let len = from.verses_in(book, chapter).unwrap_or(0);
        let first = if chapter == start.0 { start.1.max(1) } else { 1 };
        let last = if chapter == end.0 && end.1 != 0 { end.1 } else { len };
        for verse in first..=last {
            if expected.len() == mapped.verses.len() {
                break;
            }
            expected.push((chapter, verse));
        }
    }
    expected.len() == mapped.verses.len()
        && expected.iter().zip(&mapped.verses).all(|(&(c, v), m)| m.book == book && m.chapter == c && m.verse == v)
}

/// Deutsche Schreibweise der Verse in `to`: zusammenhängende Läufe als Bereich („Joel 3,1–5“, über Kapitel hinweg
/// „Joh 3,16–4,2“), mehrere Läufe mit „; “.
pub fn label(to: &Versification, verses: &[VerseRef]) -> Option<String> {
    let first = verses.first()?;
    let abbr = books().iter().find(|b| b.id == first.book).map_or(first.book, |b| b.abbr);
    let mut runs: Vec<(VerseRef, VerseRef)> = Vec::new();
    for &v in verses {
        if let Some((_, end)) = runs.last_mut() {
            let next_in_chapter = v.chapter == end.chapter && v.verse == end.verse + 1;
            let next_chapter =
                v.chapter == end.chapter + 1 && v.verse == 1 && to.verses_in(end.book, end.chapter) == Some(end.verse);
            if v.book == end.book && (next_in_chapter || next_chapter) {
                *end = v;
                continue;
            }
        }
        runs.push((v, v));
    }
    let parts: Vec<String> = runs
        .iter()
        .map(|(a, b)| {
            if a == b {
                format!("{},{}", a.chapter, a.verse)
            } else if a.chapter == b.chapter {
                format!("{},{}–{}", a.chapter, a.verse, b.verse)
            } else {
                format!("{},{}–{},{}", a.chapter, a.verse, b.chapter, b.verse)
            }
        })
        .collect();
    Some(format!("{abbr} {}", parts.join("; ")))
}

/// `scripture.versifications {}` → `{versifications: [{name, books, chapters, verses}]}`.
pub fn versifications_json() -> Value {
    let list: Vec<Value> = baruch_scripture::versification::all()
        .map(|v| {
            let chapters: usize = v.books().map(|(_, c)| c.len()).sum();
            json!({ "name": v.name, "books": v.books().count(), "chapters": chapters, "verses": v.total_verses() })
        })
        .collect();
    json!({ "versifications": list })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verses_are_written_as_runs_in_german_notation() {
        let g = baruch_scripture::versification::german();
        let v = |chapter, verse| VerseRef { book: "Joel", chapter, verse };
        assert_eq!(label(g, &[v(3, 1), v(3, 2), v(3, 3)]).as_deref(), Some("Joel 3,1–3"), "ein Lauf");
        assert_eq!(
            label(g, &[v(3, 5), v(4, 1)]).as_deref(),
            Some("Joel 3,5–4,1"),
            "über die Kapitelgrenze (Joel 3 hat 5)"
        );
        assert_eq!(label(g, &[v(2, 1), v(2, 3)]).as_deref(), Some("Joel 2,1; 2,3"), "Lücke trennt");
        assert_eq!(label(g, &[]), None, "keine Verse");
    }
}
