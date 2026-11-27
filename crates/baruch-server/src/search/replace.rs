// SPDX-License-Identifier: AGPL-3.0-or-later
//! Preview, replace and undo (Round 30a, RPC `search.preview`, `search.replace`, `search.undo`).
//!
//! The preview computes per section the hits (text kinds of the replace) and the text afterwards; `search.replace`
//! computes the same anew (same order of the hits, `skip` counts in it), leaves out deselected positions and
//! hands the new texts to `Store::replace_apply` (one transaction, per section one revision by the human, only
//! on `base_rev`). Afterwards exactly one mirror commit through `mirror::record_request`.

use baruch_gitmirror::{CommitRequest, Signature};
use baruch_store::{ReplaceBatch, ReplaceEdit, ReplaceSkip, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Collected, MAX_REPLACEMENT_CHARS, SearchHit, SearchQuery, SearchRange, TIME_LIMIT, collect, hit, query_from,
};
use crate::AppState;
use crate::mirror::{HUMAN_EMAIL, HUMAN_NAME};
use crate::rpc::{INVALID_PARAMS, RpcError};

/// Maximum number of sections per replace (contract §3.1).
pub const MAX_SECTIONS: usize = 500;

/// A section of the preview.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PreviewSection {
    pub key: i64,
    pub number: String,
    pub title: String,
    pub base_rev: u32,
    pub hits: Vec<SearchHit>,
    /// Replacement per hit (same order as `hits`).
    pub replacements: Vec<String>,
    pub diff_html: String,
    pub body_after: String,
}

/// Result of [`preview`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct Preview {
    pub sections: Vec<PreviewSection>,
    pub total: usize,
    pub truncated: bool,
    pub timed_out: bool,
    pub range_label: String,
}

/// Choice of a section when replacing: base and deselected hits (positions in the hit list of the preview).
#[derive(Debug, Clone, Deserialize)]
pub struct Pick {
    pub key: i64,
    pub base_rev: u32,
    #[serde(default)]
    pub skip: Vec<usize>,
}

/// Result of [`replace`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct Replaced {
    pub batch: ReplaceBatch,
    pub changed: Vec<Value>,
    pub skipped: Vec<ReplaceSkip>,
    pub commit: Option<String>,
    pub mirror: Option<String>,
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Escape HTML.
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Difference of a section: per line with hits the line, in it `<del>` old and `<ins>` new (everything escaped).
fn diff_html(text: &str, found: &[super::scan::Found]) -> String {
    let mut out = String::new();
    let mut index = 0;
    let mut line_start = 0;
    // `done`: up to here the text is already written out (a hit can reach across the end of a line).
    let mut done = 0;
    for line in text.split_inclusive('\n') {
        let line_end = line_start + line.len();
        if line_end <= done {
            line_start = line_end;
            continue;
        }
        let mut parts = String::new();
        let mut at = line_start.max(done);
        while index < found.len() && found[index].range.start < line_end {
            let f = &found[index];
            parts.push_str(&escape(&text[at..f.range.start.max(at)]));
            parts.push_str(&format!(
                "<del data-hit=\"{index}\">{}</del><ins data-hit=\"{index}\">{}</ins>",
                escape(&text[f.range.clone()]),
                escape(f.replacement.as_deref().unwrap_or(""))
            ));
            at = f.range.end.min(text.len()).max(at);
            index += 1;
        }
        if !parts.is_empty() {
            let rest_end = text[at..].find('\n').map_or(text.len(), |n| at + n);
            parts.push_str(&escape(&text[at..rest_end]));
            out.push_str(&format!(
                "<div class=\"sr-diff-line\"><span class=\"sr-ln\">{}</span> {parts}</div>",
                baruch_typst::line_at(text, line_start)
            ));
            done = rest_end;
        }
        line_start = line_end;
    }
    out
}

/// Check the replacement (length).
fn check_replacement(replacement: &str) -> Result<(), RpcError> {
    let chars = replacement.chars().count();
    if chars > MAX_REPLACEMENT_CHARS {
        return Err(invalid(format!(
            "Der Ersatz darf höchstens {MAX_REPLACEMENT_CHARS} Zeichen haben (hier {chars})."
        )));
    }
    Ok(())
}

/// Collect hits for replacing (per work, never `all`).
async fn collect_for_replace(
    state: &AppState,
    work: WorkId,
    query: &SearchQuery,
    replacement: &str,
) -> Result<Collected, RpcError> {
    if query.range == SearchRange::All {
        return Err(invalid("Ersetzen geht nur in einem Werk (nicht „alle Werke“)."));
    }
    check_replacement(replacement)?;
    let collected = collect(state, work, query, Some(replacement), usize::MAX, TIME_LIMIT).await?;
    if collected.over_budget {
        return Err(RpcError::new(
            crate::rpc::REFUSED,
            format!(
                "Zu viele Ersetzungen auf einmal (die eingesetzten Texte wären zusammen größer als {} MiB); ersetzt \
                 wurde nichts. Bitte die Suche oder den Bereich eingrenzen.",
                super::MAX_EXPANDED_BYTES / (1024 * 1024)
            ),
        ));
    }
    Ok(collected)
}

/// Preview of the replace.
pub async fn preview(
    state: &AppState,
    work: WorkId,
    query: &SearchQuery,
    replacement: &str,
) -> Result<Preview, RpcError> {
    let collected = collect_for_replace(state, work, query, replacement).await?;
    let mut sections = Vec::new();
    let mut shown = 0usize;
    let mut truncated = collected.truncated;
    for s in &collected.sections {
        // Whole sections only (choice and diff refer to all positions of a section), together at most
        // MAX_HITS positions – otherwise diff, text afterwards and page would grow with the number of hits.
        if sections.len() >= MAX_SECTIONS || shown + s.found.len() > super::MAX_HITS {
            truncated = true;
            break;
        }
        shown += s.found.len();
        sections.push(PreviewSection {
            key: s.section.key,
            number: s.section.number.clone(),
            title: s.section.title.clone(),
            base_rev: s.section.rev,
            hits: s.found.iter().map(|f| hit(&s.section, f)).collect(),
            replacements: s.found.iter().map(|f| f.replacement.clone().unwrap_or_default()).collect(),
            diff_html: diff_html(&s.section.text, &s.found),
            body_after: super::scan::apply(&s.section.text, &s.found, &[]),
        });
    }
    Ok(Preview {
        sections,
        total: collected.total,
        truncated,
        timed_out: collected.timed_out,
        range_label: collected.range_label,
    })
}

/// „„Leuchtturm““ shortened to at most 60 characters.
fn quoted(text: &str) -> String {
    let flat: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let mut cut: String = flat.chars().take(60).collect();
    if flat.chars().count() > 60 {
        cut.push('…');
    }
    format!("„{cut}“")
}

fn sections_word(n: usize) -> String {
    if n == 1 { "1 Abschnitt".to_owned() } else { format!("{n} Abschnitte") }
}

fn human() -> Signature {
    Signature { name: HUMAN_NAME.to_owned(), email: HUMAN_EMAIL.to_owned() }
}

/// Replace (only the human): new revisions and one commit. `picks`: sections with base and deselected positions;
/// `None` = all sections with hits on their current revision.
pub async fn replace(
    state: &AppState,
    work: WorkId,
    query: &SearchQuery,
    replacement: &str,
    picks: Option<&[Pick]>,
) -> Result<Replaced, RpcError> {
    let query = SearchQuery { rev: super::RevChoice::Current, text: None, ..query.clone() };
    let held = state.write_lock.lock().await;
    let collected = collect_for_replace(state, work, &query, replacement).await?;
    if collected.timed_out {
        return Err(RpcError::new(
            crate::rpc::REFUSED,
            "Die Suche brauchte länger als 2 s; ersetzt wurde nichts. Bitte den Bereich verkleinern.",
        ));
    }
    let mut bodies: Vec<(i64, u32, String)> = Vec::new();
    for s in &collected.sections {
        let (base, skip): (u32, &[usize]) = match picks {
            Some(picks) => match picks.iter().find(|p| p.key == s.section.key) {
                Some(p) => (p.base_rev, &p.skip),
                None => continue,
            },
            None => (s.section.rev, &[]),
        };
        if skip.len() >= s.found.len() && s.found.iter().enumerate().all(|(i, _)| skip.contains(&i)) {
            continue;
        }
        bodies.push((s.section.key, base, super::scan::apply(&s.section.text, &s.found, skip)));
    }
    if bodies.len() > MAX_SECTIONS {
        return Err(invalid(format!("Höchstens {MAX_SECTIONS} Abschnitte je Ersetzen (hier {}).", bodies.len())));
    }
    // Sections from the choice without hits any more (changed in the meantime): skipped with a reason.
    let mut skipped_early = Vec::new();
    if let Some(picks) = picks {
        for p in picks {
            if !collected.sections.iter().any(|s| s.section.key == p.key) {
                skipped_early
                    .push(ReplaceSkip { key: p.key, reason: "Keine Treffer mehr in diesem Abschnitt.".into() });
            }
        }
    }
    let edits: Vec<ReplaceEdit> =
        bodies.iter().map(|(key, base, body)| ReplaceEdit { key: *key, base_rev: *base, body }).collect();
    let options = json!({
        "case": query.case, "word": query.word, "re": query.regex, "range": query.range.word(),
        "key": query.key, "sel": query.sel, "kinds": super::kinds_for(&query, true),
    });
    let (batch, changes, mut skipped) =
        state.store.replace_apply(work, &query.q, replacement, &options, &edits, HUMAN_NAME).await?;
    skipped.extend(skipped_early);
    let loaded = state.store.load_work(work).await?;
    let label = |key: i64| {
        loaded
            .nodes
            .iter()
            .find(|n| n.key == key)
            .map_or(format!("Abschnitt {key}"), |n| super::label_of(&n.number, &n.title))
    };
    let changed: Vec<Value> =
        batch.items.iter().map(|i| json!({ "key": i.key, "rev": i.new_rev, "prev": i.prev_rev })).collect();
    let (mut commit, mut mirror) = (None, None);
    if !changes.is_empty() {
        let body: String = batch
            .items
            .iter()
            .map(|i| format!("- {}: Fassung {} → {}\n", label(i.key), i.prev_rev, i.new_rev))
            .collect();
        let request = CommitRequest {
            files: Vec::new(),
            author: human(),
            committer: human(),
            summary: format!(
                "Ersetzt {} → {} in {}",
                quoted(&query.q),
                quoted(replacement),
                if batch.items.len() == 1 {
                    "1 Abschnitt".to_owned()
                } else {
                    format!("{} Abschnitten", batch.items.len())
                }
            ),
            body,
            trailers: vec![("Baruch-Action".into(), "replace".into()), ("Baruch-Replace".into(), batch.id.to_string())],
            allow_empty: false,
        };
        match crate::mirror::record_request(state, &held, work, request).await {
            Ok(c) => commit = c,
            Err(e) => mirror = Some(e),
        }
    }
    Ok(Replaced { batch, changed, skipped, commit, mirror })
}

/// Undo (only the human): back to the previous revisions, one commit.
pub async fn undo(state: &AppState, work: WorkId, batch: i64) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let (batch, skipped) = state.store.replace_undo(work, batch, HUMAN_NAME).await?;
    let restored: Vec<Value> = batch
        .items
        .iter()
        .filter(|i| !skipped.iter().any(|s| s.key == i.key))
        .map(|i| json!({ "key": i.key, "rev": i.prev_rev }))
        .collect();
    let mut answer = json!({ "batch": batch, "restored": restored, "skipped": skipped });
    if !restored.is_empty() {
        let request = CommitRequest {
            files: Vec::new(),
            author: human(),
            committer: human(),
            summary: format!(
                "Ersetzen rückgängig: {} → {} ({})",
                quoted(&batch.query),
                quoted(&batch.replacement),
                sections_word(restored.len())
            ),
            body: String::new(),
            trailers: vec![
                ("Baruch-Action".into(), "replace-undo".into()),
                ("Baruch-Replace".into(), batch.id.to_string()),
            ],
            allow_empty: false,
        };
        match crate::mirror::record_request(state, &held, work, request).await {
            Ok(Some(c)) => answer["commit"] = json!(c),
            Ok(None) => {}
            Err(e) => answer["mirror"] = json!(e),
        }
    }
    Ok(answer)
}

/// Require the confirmation `confirm: true`.
fn confirmed(params: &Value) -> Result<(), RpcError> {
    match params.get("confirm") {
        Some(Value::Bool(true)) => Ok(()),
        _ => Err(invalid("Ersetzen bitte bestätigen (confirm: true).")),
    }
}

fn replacement_of(params: &Value) -> Result<String, RpcError> {
    params
        .get("replacement")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| invalid("replacement: Ersatztext fehlt (leer ist erlaubt)."))
}

/// RPC `search.preview`.
pub async fn preview_rpc(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let query = query_from(params)?;
    let replacement = replacement_of(params)?;
    Ok(serde_json::to_value(preview(state, work, &query, &replacement).await?).expect("Vorschau ist JSON"))
}

/// RPC `search.replace`.
pub async fn replace_rpc(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    confirmed(params)?;
    let query = query_from(params)?;
    let replacement = replacement_of(params)?;
    let picks: Option<Vec<Pick>> = match params.get("sections") {
        None | Some(Value::Null) => None,
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| invalid(format!("sections: {e}")))?),
    };
    let done = replace(state, work, &query, &replacement, picks.as_deref()).await?;
    let mut answer = json!({ "batch": done.batch, "changed": done.changed, "skipped": done.skipped });
    if let Some(c) = done.commit {
        answer["commit"] = json!(c);
    }
    if let Some(m) = done.mirror {
        answer["mirror"] = json!(m);
    }
    Ok(answer)
}

/// RPC `search.undo`.
pub async fn undo_rpc(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    match params.get("confirm") {
        Some(Value::Bool(true)) => {}
        _ => return Err(invalid("Rückgängig bitte bestätigen (confirm: true).")),
    }
    let batch =
        params.get("batch").and_then(Value::as_i64).ok_or_else(|| invalid("batch: Nummer des Ersetzens fehlt."))?;
    undo(state, work, batch).await
}
