// SPDX-License-Identifier: AGPL-3.0-or-later
//! PDF über MultiGPT lesen (Runde 21a): `upload_document` (genau einmal), warten auf die Indexierung (`run_status`,
//! sonst `read_document` mit Wiederholung, höchstens [`INDEX_WAIT`]), `document_info` (Titel, Autor, Seiten,
//! erkanntes Inhaltsverzeichnis), Text über `read_document` seitenweise mit `page_from`/`page_to` (höchstens
//! [`RANGE_PAGES`] Seiten je Aufruf; kürzt MultiGPT einen Bereich, liest Baruch ihn Seite für Seite nach).
//!
//! Namen und Formen sind **Annahmen** nach der Werkzeugbeschreibung (Stand 2026-11-09, docs/Architektur.md
//! „MultiGPT“): `upload_document {collection, content_base64, filename}` → `document_id` (Zahl oder Text), `run_id`;
//! `run_status {run_id}` → `status` (`done`/`completed`/`finished`/`success`/`ready`/`indexed` = fertig,
//! `failed`/`error`/`cancelled` = gescheitert); `document_info {document_id}` → `title`, `author`, `pages`, `toc`
//! (auch `table_of_contents`, `outline`, `sections`) mit `title`, `level` bzw. `section` („1.2“) und `page`;
//! `read_document {document_id, page_from, page_to}` → `text` (höchstens 12000 Zeichen), `truncated`.
//!
//! Scheitert ein Schritt, gibt es einen Fehler mit Grund – nie einen stillen Wechsel auf den lokalen Weg.

use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::Extracted;
use super::pdf::{join_lines, numbered};
use super::typst_out::{Section, escape_text};
use crate::AppState;
use crate::connections::multigpt::{
    DOCUMENT_KEYS, INFO_TOOL, Limits, READ_TOOL, RUN_STATUS_TOOL, Session, UPLOAD_TOOL, clean_line, content_text,
    tool_payload,
};
use crate::import::ImportWarning;

/// Längste Wartezeit auf die Indexierung nach dem Hochladen.
pub const INDEX_WAIT: Duration = Duration::from_secs(120);
/// Höchstens so viele Seiten je `read_document` (Annahme laut Werkzeugbeschreibung).
pub const RANGE_PAGES: usize = 10;
/// Höchstens so viele Zeichen liefert `read_document` je Aufruf (Annahme); so lang = vermutlich gekürzt.
pub const READ_CHARS: usize = 12_000;
/// Grenzen der Aufrufe beim Import (30 s je Aufruf – die Obergrenze einer Sitzung –, Antworten bis 4 MiB).
pub const IMPORT_LIMITS: Limits = Limits { timeout: Duration::from_secs(30), max_body: 4 * 1024 * 1024 };
/// Höchstens so viele Seiten liest der Import über MultiGPT.
const MAX_PAGES: usize = 2000;

/// Dokument in MultiGPT.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteDoc {
    pub collection: String,
    pub id: String,
}

fn warn(code: &str, message: impl Into<String>) -> ImportWarning {
    ImportWarning { code: code.into(), message: message.into(), line: None }
}

/// Text (oder Zahl als Text) unter einem der Schlüssel, in den Daten oder unter `document`/`result`/`data`.
fn field(payload: &Value, keys: &[&str]) -> Option<String> {
    let map = payload.as_object()?;
    let mut places = vec![map];
    for nested in ["document", "result", "data", "metadata"] {
        if let Some(Value::Object(inner)) = map.get(nested) {
            places.push(inner);
        }
    }
    places.into_iter().find_map(|m| {
        keys.iter().find_map(|k| match m.get(*k) {
            Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        })
    })
}

/// Wert passend zum Schema des Parameters `key` von `tool` (Zahl, wenn nur `integer` erlaubt ist).
fn typed(schema: &Value, key: &str, text: &str) -> Value {
    let spec = schema.get("properties").and_then(|p| p.get(key)).cloned().unwrap_or(json!({}));
    let integer_only = match spec.get("type") {
        Some(Value::String(t)) => t == "integer",
        Some(Value::Array(types)) => types.iter().any(|t| t == "integer") && !types.iter().any(|t| t == "string"),
        _ => false,
    };
    match text.parse::<i64>() {
        Ok(n) if integer_only => json!(n),
        _ => json!(text),
    }
}

fn props(schema: &Value) -> Map<String, Value> {
    schema.get("properties").and_then(Value::as_object).cloned().unwrap_or_default()
}

/// Argumente mit dem Dokument (Name des Parameters aus [`DOCUMENT_KEYS`]).
fn document_args(schema: &Value, tool: &str, document: &str) -> Result<Map<String, Value>, String> {
    let p = props(schema);
    let key = DOCUMENT_KEYS
        .iter()
        .find(|k| p.contains_key(**k))
        .or_else(|| p.is_empty().then_some(&"document_id"))
        .ok_or_else(|| format!("{tool} nennt im inputSchema keinen Parameter für das Dokument."))?;
    let mut args = Map::new();
    args.insert((*key).to_owned(), typed(schema, key, document));
    Ok(args)
}

/// Einträge des Inhaltsverzeichnisses aus `document_info`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TocEntry {
    pub level: u8,
    pub title: String,
    pub page: Option<usize>,
}

pub(crate) fn toc_of(payload: &Value) -> Vec<TocEntry> {
    let list = ["toc", "table_of_contents", "outline", "sections", "chapters"].iter().find_map(|k| {
        payload.get(*k).or_else(|| payload.get("document").and_then(|d| d.get(*k))).and_then(Value::as_array)
    });
    let Some(list) = list else { return Vec::new() };
    let mut out = Vec::new();
    fn walk(items: &[Value], depth: u8, out: &mut Vec<TocEntry>) {
        for item in items {
            let Some(map) = item.as_object() else { continue };
            let text = |keys: &[&str]| {
                keys.iter().find_map(|k| match map.get(*k) {
                    Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
                    Some(Value::Number(n)) => Some(n.to_string()),
                    _ => None,
                })
            };
            let Some(title) = text(&["title", "heading", "name", "label"]) else { continue };
            let section = text(&["section", "number", "section_number", "numbering"]);
            let level = map
                .get("level")
                .and_then(Value::as_u64)
                .map(|l| l as u8)
                .or_else(|| section.as_deref().map(|s| s.trim_end_matches('.').split('.').count() as u8))
                .unwrap_or(depth)
                .clamp(1, 3);
            let page = ["page", "page_from", "start_page", "page_number"]
                .iter()
                .find_map(|k| map.get(*k).and_then(Value::as_u64))
                .map(|p| p as usize);
            out.push(TocEntry { level, title: clean_line(&title, 300), page });
            if let Some(children) =
                ["children", "items", "sections"].iter().find_map(|k| map.get(*k).and_then(Value::as_array))
            {
                walk(children, depth.saturating_add(1), out);
            }
        }
    }
    walk(list, 1, &mut out);
    out
}

/// Text in Absätze (Leerzeilen), je Absatz Zeilen verbunden.
fn paragraphs(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n").split("\n\n").map(|p| join_lines(p.lines())).filter(|p| !p.trim().is_empty()).collect()
}

fn section_of(level: u8, title: String, text: &str) -> Section {
    let paras = paragraphs(text);
    Section {
        level,
        part: level == 1 && ["teil ", "part ", "buch "].iter().any(|p| title.to_lowercase().starts_with(p)),
        title,
        body: paras.iter().map(|p| escape_text(p, true)).collect::<Vec<_>>().join("\n\n"),
        plain: paras.join("\n\n"),
    }
}

/// Stelle von `needle` in `hay` ab Byte `from`, ohne Groß-/Kleinschreibung und mit beliebigem Leerraum; Ergebnis =
/// Byte-Stelle des Anfangs.
fn find_loose(hay: &str, needle: &str, from: usize) -> Option<usize> {
    let words: Vec<String> = needle.split_whitespace().map(str::to_lowercase).collect();
    let first = words.first()?;
    let lower = hay.to_lowercase();
    if lower.len() != hay.len() {
        // Kleinschreibung ändert Längen (selten): einfacher Vergleich auf dem Original.
        return hay[from..].find(needle).map(|i| i + from);
    }
    let mut start = from;
    while let Some(found) = lower[start..].find(first.as_str()) {
        let at = start + found;
        let mut pos = at + first.len();
        let mut ok = true;
        for word in &words[1..] {
            let rest = &lower[pos..];
            let trimmed = rest.trim_start();
            if trimmed.len() == rest.len() || !trimmed.starts_with(word.as_str()) {
                ok = false;
                break;
            }
            pos += rest.len() - trimmed.len() + word.len();
        }
        if ok {
            return Some(at);
        }
        start = at + first.len().max(1);
    }
    None
}

/// Text nach dem Inhaltsverzeichnis teilen: je Eintrag die erste Fundstelle seines Titels nach dem vorigen Schnitt;
/// die Zeile mit dem Titel ist die Überschrift (fällt aus dem Text). Nicht gefunden → Punkt ohne eigenen Text
/// (Warnung).
pub(crate) fn split_by_toc(text: &str, toc: &[TocEntry], warnings: &mut Vec<ImportWarning>) -> Vec<Section> {
    // (Anfang der Titelzeile, Ende der Titelzeile); nicht gefunden = None (der Punkt nimmt keinem anderen Text weg).
    let mut found: Vec<Option<(usize, usize)>> = Vec::new();
    let mut cursor = 0;
    for entry in toc {
        match find_loose(text, &entry.title, cursor) {
            Some(at) => {
                let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1).max(cursor);
                let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
                found.push(Some((line_start, line_end)));
                cursor = line_end;
            }
            None => {
                warnings.push(warn(
                    "import-heading-not-found",
                    format!(
                        "„{}“ aus dem Inhaltsverzeichnis steht so nicht im Text; der Punkt bleibt ohne eigenen Text.",
                        entry.title
                    ),
                ));
                found.push(None);
            }
        }
    }
    // Ein nicht gefundener Punkt steht ohne Text an der Stelle hinter dem vorigen Text.
    let next_start = |i: usize| found[i + 1..].iter().flatten().next().map_or(text.len(), |c| c.0);
    let mut cuts: Vec<(usize, usize)> = Vec::with_capacity(found.len());
    for (i, cut) in found.iter().enumerate() {
        cuts.push(match cut {
            Some(c) => *c,
            None => {
                let at = next_start(i);
                (at, at)
            }
        });
    }
    let mut sections = Vec::new();
    if let Some(&(first, _)) = cuts.first()
        && !text[..first].trim().is_empty()
    {
        sections.push(section_of(1, "Vorspann".into(), &text[..first]));
        warnings.push(warn(
            "import-front-matter",
            "Text vor dem ersten Punkt der Gliederung steht im Punkt „Vorspann“ (streichen hängt ihn an das erste Kapitel).",
        ));
    }
    for (i, entry) in toc.iter().enumerate() {
        let start = cuts[i].1;
        let end = if found[i].is_some() { next_start(i) } else { start }.max(start);
        sections.push(section_of(entry.level, entry.title.clone(), &text[start..end]));
    }
    sections
}

/// Ohne Inhaltsverzeichnis: kurze Zeilen mit Nummer (`1`, `1.2 …`) als Überschriften, sonst ein Kapitel.
pub(crate) fn split_by_heuristic(text: &str, title: &str, warnings: &mut Vec<ImportWarning>) -> Vec<Section> {
    let mut toc = Vec::new();
    for paragraph in text.split("\n\n") {
        let line = paragraph.trim();
        if line.contains('\n') || line.chars().count() > 100 || line.ends_with(['.', ',', ';', ':']) {
            continue;
        }
        if let Some((parts, _)) = numbered(line) {
            toc.push(TocEntry { level: parts.min(3) as u8, title: line.to_owned(), page: None });
        }
    }
    if toc.is_empty() {
        warnings.push(warn(
            "import-no-outline",
            "Kein Inhaltsverzeichnis und keine nummerierten Überschriften: der ganze Text steht in einem Kapitel.",
        ));
        return vec![section_of(1, title.to_owned(), text)];
    }
    warnings.push(warn(
        "import-outline-heuristic",
        "MultiGPT nannte kein Inhaltsverzeichnis; die Gliederung folgt nummerierten Zeilen im Text – bitte prüfen.",
    ));
    split_by_toc(text, &toc, warnings)
}

/// Ist ein Lauf fertig (`Some(true)`), gescheitert (`Some(false)`) oder läuft er noch (`None`)?
fn run_state(payload: &Value) -> Option<bool> {
    let status = field(payload, &["status", "state"])?.to_lowercase();
    match status.as_str() {
        "done" | "completed" | "complete" | "finished" | "success" | "succeeded" | "ready" | "indexed" => Some(true),
        "failed" | "error" | "cancelled" | "canceled" | "aborted" => Some(false),
        _ => None,
    }
}

async fn pause(round: u32) {
    let ms = (100u64 << round.min(4)).min(2000);
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Eine Antwort von `read_document`: Text, gemeldete Seitenzahl, gekürzt?
struct Read {
    text: String,
    pages: Option<usize>,
    truncated: bool,
}

fn read_answer(result: &Value) -> Read {
    let payload = tool_payload(result);
    let text = payload
        .as_ref()
        .and_then(|p| field(p, &["text", "content", "body", "markdown"]))
        .unwrap_or_else(|| content_text(result));
    let pages =
        payload.as_ref().and_then(|p| field(p, &["pages", "total_pages", "page_count"])).and_then(|p| p.parse().ok());
    let truncated = payload.as_ref().and_then(|p| p.get("truncated").and_then(Value::as_bool)).unwrap_or(false)
        || text.chars().count() >= READ_CHARS;
    Read { text: text.replace("\r\n", "\n"), pages, truncated }
}

/// PDF hochladen und lesen. Ergebnis: gelesener Text samt Gliederung und das Dokument in MultiGPT.
pub(crate) async fn read(
    state: &AppState,
    bytes: &[u8],
    file_name: &str,
    collection: &str,
) -> Result<(Extracted, RemoteDoc), String> {
    let mut session = Session::open(state, IMPORT_LIMITS).await.map_err(|e| e.message().to_owned())?;
    let outcome = read_in(&mut session, bytes, file_name, collection).await;
    session.release().await;
    outcome
}

async fn read_in(
    session: &mut Session,
    bytes: &[u8],
    file_name: &str,
    collection: &str,
) -> Result<(Extracted, RemoteDoc), String> {
    let mut warnings = Vec::new();
    let upload_schema = session
        .schema(UPLOAD_TOOL)
        .ok_or_else(|| format!("MultiGPT bietet das Werkzeug {UPLOAD_TOOL} nicht an (tools/list)."))?;
    let p = props(&upload_schema);
    for needed in ["collection", "content_base64", "filename"] {
        if !p.is_empty() && !p.contains_key(needed) {
            return Err(format!(
                "{UPLOAD_TOOL} nennt im inputSchema keinen Parameter „{needed}“ (Annahme der Werkzeugbeschreibung)."
            ));
        }
    }
    let args = json!({
        "collection": collection,
        "content_base64": baruch_store::encode_base64(bytes),
        "filename": file_name,
    });
    let result = session.call(UPLOAD_TOOL, args, None).await.map_err(|e| e.message().to_owned())?;
    let payload = tool_payload(&result).unwrap_or(json!({}));
    let document = field(&payload, &["document_id", "doc_id", "documentId", "id"]).ok_or_else(|| {
        format!(
            "{UPLOAD_TOOL} nannte keine Dokument-ID; die Datei liegt womöglich trotzdem in der Sammlung „{collection}“."
        )
    })?;
    let document = clean_line(&document, 500);
    let run = field(&payload, &["run_id", "runId", "job_id"]);
    let remote = RemoteDoc { collection: collection.to_owned(), id: document.clone() };

    // Auf die Indexierung warten.
    let started = Instant::now();
    let read_schema = session
        .schema(READ_TOOL)
        .ok_or_else(|| format!("MultiGPT bietet das Werkzeug {READ_TOOL} nicht an (tools/list)."))?;
    let read_props = props(&read_schema);
    let ranged = read_props.contains_key("page_from") && read_props.contains_key("page_to");
    let base = document_args(&read_schema, READ_TOOL, &document)?;
    let first_args = |base: &Map<String, Value>| {
        let mut args = base.clone();
        if ranged {
            args.insert("page_from".into(), json!(1));
            args.insert("page_to".into(), json!(1));
        }
        Value::Object(args)
    };
    match (run, session.schema(RUN_STATUS_TOOL)) {
        (Some(run), Some(_)) => {
            let mut round = 0;
            loop {
                let status = session
                    .call(RUN_STATUS_TOOL, json!({ "run_id": run }), None)
                    .await
                    .map_err(|e| e.message().to_owned())?;
                match tool_payload(&status).as_ref().and_then(run_state) {
                    Some(true) => break,
                    Some(false) => {
                        return Err(format!(
                            "MultiGPT meldet, dass die Indexierung von „{file_name}“ gescheitert ist (Lauf {run})."
                        ));
                    }
                    None => {}
                }
                if started.elapsed() > INDEX_WAIT {
                    return Err(format!(
                        "MultiGPT hat „{file_name}“ nach {} s noch nicht fertig indexiert (Lauf {run}); später erneut versuchen.",
                        INDEX_WAIT.as_secs()
                    ));
                }
                pause(round).await;
                round += 1;
            }
        }
        _ => {
            // Ohne run_status: lesen, bis read_document antwortet.
            let mut round = 0;
            loop {
                match session.call(READ_TOOL, first_args(&base), None).await {
                    Ok(_) => break,
                    Err(e) if session.broken() => return Err(e.message().to_owned()),
                    Err(e) => {
                        if started.elapsed() > INDEX_WAIT {
                            return Err(format!(
                                "MultiGPT liefert „{file_name}“ nach {} s noch nicht ({}).",
                                INDEX_WAIT.as_secs(),
                                e.message()
                            ));
                        }
                    }
                }
                pause(round).await;
                round += 1;
            }
        }
    }

    // Angaben und Inhaltsverzeichnis.
    let mut info = json!({});
    if let Some(schema) = session.schema(INFO_TOOL) {
        let args = document_args(&schema, INFO_TOOL, &document)?;
        match session.call(INFO_TOOL, Value::Object(args), None).await {
            Ok(result) => info = tool_payload(&result).unwrap_or(json!({})),
            Err(e) => warnings.push(warn(
                "import-multigpt-info",
                format!("document_info scheiterte ({}); Gliederung aus dem Text.", e.message()),
            )),
        }
    }
    let title = field(&info, &["title", "document_title"]).map(|t| clean_line(&t, 300));
    let author = field(&info, &["author", "authors", "creator"]).map(|a| clean_line(&a, 300));
    let mut pages: Option<usize> = field(&info, &["pages", "total_pages", "page_count"]).and_then(|p| p.parse().ok());
    let toc = toc_of(&info);

    // Text lesen.
    let mut text = String::new();
    if ranged {
        let mut from = 1usize;
        loop {
            let total = pages.unwrap_or(from + RANGE_PAGES - 1);
            if from > total || from > MAX_PAGES {
                break;
            }
            let to = (from + RANGE_PAGES - 1).min(total);
            let mut args = base.clone();
            args.insert("page_from".into(), json!(from));
            args.insert("page_to".into(), json!(to));
            let answer = read_answer(
                &session.call(READ_TOOL, Value::Object(args), None).await.map_err(|e| e.message().to_owned())?,
            );
            if pages.is_none() {
                pages = answer.pages;
            }
            let chunk = if answer.truncated && to > from {
                // Gekürzt: Seite für Seite nachlesen.
                let mut pieces = Vec::new();
                for page in from..=to {
                    let mut args = base.clone();
                    args.insert("page_from".into(), json!(page));
                    args.insert("page_to".into(), json!(page));
                    let single = read_answer(
                        &session
                            .call(READ_TOOL, Value::Object(args), None)
                            .await
                            .map_err(|e| e.message().to_owned())?,
                    );
                    if single.truncated {
                        warnings.push(warn(
                            "import-multigpt-truncated",
                            format!("Seite {page}: MultiGPT liefert höchstens {READ_CHARS} Zeichen je Aufruf; der Rest der Seite fehlt."),
                        ));
                    }
                    pieces.push(single.text);
                }
                pieces.join("\n\n")
            } else {
                if answer.truncated {
                    warnings.push(warn(
                        "import-multigpt-truncated",
                        format!("Seite {from}: MultiGPT liefert höchstens {READ_CHARS} Zeichen je Aufruf; der Rest der Seite fehlt."),
                    ));
                }
                answer.text
            };
            if !text.is_empty() && !chunk.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(&chunk);
            if pages.is_none() && chunk.trim().is_empty() {
                break;
            }
            from = to + 1;
        }
    } else {
        warnings.push(warn(
            "import-multigpt-unpaged",
            "read_document kennt kein page_from/page_to; Baruch liest das Dokument in einem Stück.",
        ));
        let result = session
            .call(READ_TOOL, Value::Object(base.clone()), Some(crate::connections::multigpt::FULLTEXT_LIMITS))
            .await
            .map_err(|e| e.message().to_owned())?;
        text = read_answer(&result).text;
    }
    let text = session.scrub(&text);
    let fallback = title.clone().unwrap_or_else(|| file_name.trim_end_matches(".pdf").to_owned());
    let sections = if toc.is_empty() {
        split_by_heuristic(&text, &fallback, &mut warnings)
    } else {
        split_by_toc(&text, &toc, &mut warnings)
    };
    warnings.push(warn(
        "import-multigpt-plain",
        "Über MultiGPT gelesen: übernommen werden Text und die Gliederung aus dem Inhaltsverzeichnis; Hervorhebungen, \
         Fußnoten und Bilder liefert read_document nicht.",
    ));
    Ok((
        Extracted {
            title,
            author,
            lang: None,
            pages,
            sections,
            assets: Vec::new(),
            skipped: Vec::new(),
            warnings,
            plain_text: text,
        },
        remote,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_of_contents_cuts_the_text_at_the_titles() {
        let text = "Titelseite\n\n1 Am Meer\n\nWellen kommen.\n\n1.1 Ebbe\n\nDas Wasser geht.\n\n2 Im Wald\n\nBäume.";
        let toc = vec![
            TocEntry { level: 1, title: "Am Meer".into(), page: Some(1) },
            TocEntry { level: 2, title: "Ebbe".into(), page: Some(2) },
            TocEntry { level: 1, title: "Im   Wald".into(), page: Some(3) },
            TocEntry { level: 1, title: "Fehlt".into(), page: None },
        ];
        let mut warnings = Vec::new();
        let sections = split_by_toc(text, &toc, &mut warnings);
        let titles: Vec<&str> = sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Vorspann", "Am Meer", "Ebbe", "Im   Wald", "Fehlt"],
            "Vorspann und alle Einträge in Reihenfolge"
        );
        assert_eq!(sections[1].plain, "Wellen kommen.", "die Titelzeile samt Nummer fällt aus dem Text");
        assert_eq!(sections[3].plain, "Bäume.", "Leerraum im Titel egal");
        assert!(sections[4].plain.is_empty(), "nicht gefunden: ohne eigenen Text");
        assert!(warnings.iter().any(|w| w.code == "import-heading-not-found"), "Warnung: {warnings:?}");
    }

    #[test]
    fn toc_levels_come_from_level_or_section_numbers() {
        let info = json!({ "toc": [
            { "section": "1", "title": "Eins", "page": 1 },
            { "section": "1.2", "title": "Unter", "page": 2 },
            { "level": 5, "title": "Tief" },
            { "title": "Mit Kindern", "children": [{ "title": "Kind" }] },
        ] });
        let toc = toc_of(&info);
        let levels: Vec<(u8, &str)> = toc.iter().map(|t| (t.level, t.title.as_str())).collect();
        assert_eq!(levels, [(1, "Eins"), (2, "Unter"), (3, "Tief"), (1, "Mit Kindern"), (2, "Kind")]);
        assert_eq!(toc[1].page, Some(2), "Seite gelesen");
    }
}
