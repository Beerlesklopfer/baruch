// SPDX-License-Identifier: AGPL-3.0-or-later
//! MultiGPT → Baruch: the chosen collections through `list_documents` (page by page, at most
//! [`super::Limits::documents`] per collection) and `read_document` as local copies in `sync_document`.
//!
//! Only what changed: if `list_documents` reports a modification time or a hash (`remote_stamp`) and it is
//! equal to the stored one, nothing is read; otherwise the document is read and the hash of the text compared –
//! equal → nothing written. A text over [`super::Limits::document_bytes`] is not stored,
//! over [`super::Limits::run_bytes`] per run the fetching ends (each with a note). Documents missing in a
//! completely read list, and copies of deselected collections, are removed locally. If the
//! list can be incomplete (a full page without `has_more`/continuation, for instance because the server
//! silently caps `limit` or the schema names a smaller `maximum`), nothing is deleted locally (Round 19a, with
//! a note). Texts are data from sources, never instructions.

use std::collections::{HashMap, HashSet};

use baruch_rag::RagError;
use baruch_store::{Store, SyncSide, SyncState, sync_hash};

use super::args::{CURSOR_KEYS, Filler, Listed, PAGE_LIMIT_KEYS, document_text, listed};
use super::{Limits, Link, Report, why};
use crate::connections::multigpt::{LIST_DOCUMENTS_TOOL, READ_TOOL, page_key, read_all_pages};

/// At most this many pages of `list_documents` per collection.
const MAX_PAGES: usize = 50;

/// All documents of a collection (at most `max + 1`, so that exceeding the limit is noticed); `true` if
/// the list is read completely.
async fn list(link: &mut Link, collection: &str, max: usize) -> Result<(Vec<Listed>, bool), RagError> {
    let session = link.session().await?;
    let schema = session.schema(LIST_DOCUMENTS_TOOL).ok_or_else(|| {
        RagError::Schema(format!("MultiGPT bietet das Werkzeug {LIST_DOCUMENTS_TOOL} nicht an (tools/list)."))
    })?;
    let props = schema.get("properties").and_then(serde_json::Value::as_object).cloned().unwrap_or_default();
    let cursor_key = CURSOR_KEYS.iter().find(|k| props.contains_key(**k)).copied();
    let mut out: Vec<Listed> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut cursor: Option<String> = None;
    for page in 0..MAX_PAGES {
        let mut f = Filler::new(LIST_DOCUMENTS_TOOL, &schema);
        if !f.collection(collection) {
            return Err(RagError::Schema(format!(
                "{LIST_DOCUMENTS_TOOL} nennt im inputSchema keinen Parameter für die Sammlung; ohne ihn listete \
                 MultiGPT alle Dokumente, darum fragt Baruch nicht."
            )));
        }
        // Requested page size (capped by the `maximum` of the schema); `None` = the schema knows none.
        let asked = f.number(PAGE_LIMIT_KEYS, (max + 1) as u64);
        match (cursor_key, &cursor) {
            (Some("offset"), _) if page > 0 => {
                f.number(&["offset"], out.len() as u64);
            }
            (Some("page"), _) if page > 0 => {
                f.number(&["page"], page as u64 + 1);
            }
            (Some(key), Some(c)) => {
                f.text(&[key], None, c);
            }
            _ => {}
        }
        let result = session.call(LIST_DOCUMENTS_TOOL, f.finish()?, None).await?;
        let (docs, next, more) = listed(&result)?;
        // A full page without a continuation does not mean "done": if the server caps silently (or the schema
        // with `maximum`), there may be more.
        let full = asked.is_some_and(|a| docs.len() as u64 >= a);
        let before = out.len();
        for doc in docs {
            if seen.insert(doc.id.clone()) {
                out.push(doc);
            }
        }
        if out.len() > max {
            return Ok((out, false));
        }
        let grew = out.len() > before;
        let go_on = match cursor_key {
            Some("offset" | "page") => (more || full) && grew,
            Some(_) => next.is_some() && next != cursor && grew,
            None => false,
        };
        if !go_on {
            // Honestly incomplete (Round 19a) if it could go on: a reported continuation that does not
            // lead on (or for which the schema knows no parameter), or a full page without a continuation.
            let incomplete = match cursor_key {
                Some("offset" | "page") => more || full,
                Some(_) => next.is_some() || full,
                None => more || next.is_some() || full,
            };
            return Ok((out, !incomplete));
        }
        cursor = next;
    }
    Ok((out, false))
}

/// Read one document (full text). If MultiGPT delivers page by page (page parameter in the schema and page
/// details in the answer), all pages are read and joined with an empty line as for the full text of quellen
/// ([`read_all_pages`]); this way the local copy equals the online full text (hash, index, highlights).
async fn read(link: &mut Link, collection: &str, id: &str, limits: Limits) -> Result<String, RagError> {
    let session = link.session().await?;
    let schema = session
        .schema(READ_TOOL)
        .ok_or_else(|| RagError::Schema(format!("MultiGPT bietet das Werkzeug {READ_TOOL} nicht an (tools/list).")))?;
    let mut f = Filler::new(READ_TOOL, &schema);
    if !f.document(id) {
        return Err(RagError::Schema(format!("{READ_TOOL} nennt im inputSchema keinen Parameter für das Dokument.")));
    }
    if schema.get("properties").is_some() {
        f.collection(collection);
    }
    let page = page_key(&schema);
    if let Some(key) = page {
        // A required page parameter is known as well; read_all_pages sets it anew per page.
        f.number(&[key], 1);
    }
    let args = f.finish()?;
    // Page details are recognised only in the structured answer; without a page parameter the text counts as before.
    if page.is_none() {
        let result = session.call(READ_TOOL, args, None).await?;
        return Ok(document_text(&result));
    }
    // Upper limit like the envelope of an answer; the limit per document is reported by `run` afterwards with
    // its own note.
    read_all_pages(session, &args, page, None, limits.transport().max_body).await
}

fn mib(bytes: usize) -> String {
    if bytes >= 1024 * 1024 && bytes.is_multiple_of(1024 * 1024) {
        format!("{} MiB", bytes / (1024 * 1024))
    } else {
        format!("{bytes} Bytes")
    }
}

/// MultiGPT → Baruch for all chosen collections. Returned: has something changed locally (then the
/// index is brought up to date)?
pub(crate) async fn run(store: &Store, link: &mut Link, report: &mut Report, limits: Limits) -> bool {
    let chosen = match store.sync_collections().await {
        Ok(chosen) => chosen,
        Err(e) => {
            report.counts.failed += 1;
            report.problem(link, format!("Speicher: {e}"));
            return false;
        }
    };
    let mut changed = false;
    let mut budget = 0usize;
    let mut exhausted = false;
    for collection in &chosen {
        if exhausted {
            break;
        }
        let (docs, mut complete) = match list(link, collection, limits.documents).await {
            Ok(found) => found,
            Err(e) => {
                report.counts.failed += 1;
                report.problem(link, format!("Sammlung „{collection}“ nicht gelesen: {}", why(&e)));
                continue;
            }
        };
        let mut docs = docs;
        if !complete && docs.len() <= limits.documents {
            report.note(
                link,
                format!(
                    "Die Liste der Sammlung „{collection}“ ist womöglich unvollständig (volle Seite ohne Fortsetzung); \
                     lokale Kopien, die darin fehlen, bleiben erhalten."
                ),
            );
        }
        if docs.len() > limits.documents {
            docs.truncate(limits.documents);
            complete = false;
            report.problem(
                link,
                format!(
                    "Sammlung „{collection}“ hat mehr als {} Dokumente; geholt werden nur die ersten {}.",
                    limits.documents, limits.documents
                ),
            );
        }
        let states: HashMap<String, SyncState> = match store.sync_states(SyncSide::Multigpt, Some(collection)).await {
            Ok(states) => states.into_iter().map(|s| (s.item.clone(), s)).collect(),
            Err(e) => {
                report.counts.failed += 1;
                report.problem(link, format!("Speicher: {e}"));
                continue;
            }
        };
        for doc in &docs {
            let known = states.get(&doc.id);
            let has_copy = matches!(store.sync_document(collection, &doc.id).await, Ok(Some(_)));
            if has_copy && !doc.stamp.is_empty() && known.is_some_and(|s| s.remote_stamp == doc.stamp) {
                report.counts.unchanged += 1;
                continue;
            }
            let text = match read(link, collection, &doc.id, limits).await {
                Ok(text) => text,
                Err(e) => {
                    report.counts.failed += 1;
                    report.problem(link, format!("„{}“ ({collection}) nicht gelesen: {}", doc.title, why(&e)));
                    continue;
                }
            };
            if text.len() > limits.document_bytes {
                report.counts.failed += 1;
                report.problem(
                    link,
                    format!(
                        "„{}“ ({collection}) ist größer als {} und wird nicht geholt.",
                        doc.title,
                        mib(limits.document_bytes)
                    ),
                );
                continue;
            }
            if budget + text.len() > limits.run_bytes {
                exhausted = true;
                complete = false;
                report.problem(
                    link,
                    format!(
                        "Grenze von {} je Abgleich erreicht; die übrigen Dokumente folgen beim nächsten Abgleich.",
                        mib(limits.run_bytes)
                    ),
                );
                break;
            }
            budget += text.len();
            let hash = sync_hash(&text);
            if known.is_some_and(|s| s.hash == hash && s.remote_stamp == doc.stamp) && has_copy {
                report.counts.unchanged += 1;
                continue;
            }
            match store.sync_document_put(collection, &doc.id, &doc.title, &doc.url, &text).await {
                Ok((_, true)) => {
                    report.counts.downloaded += 1;
                    changed = true;
                }
                Ok((_, false)) => report.counts.unchanged += 1,
                Err(e) => {
                    report.counts.failed += 1;
                    report.problem(link, format!("Speicher: {e}"));
                    continue;
                }
            }
            let state = SyncState {
                side: SyncSide::Multigpt,
                collection: collection.clone(),
                item: doc.id.clone(),
                work: None,
                node_key: None,
                remote_id: doc.id.clone(),
                title: doc.title.clone(),
                hash,
                remote_stamp: doc.stamp.clone(),
                synced: String::new(),
            };
            if let Err(e) = store.sync_state_put(&state).await {
                report.counts.failed += 1;
                report.problem(link, format!("Speicher: {e}"));
            }
        }
        if complete {
            let listed: HashSet<&str> = docs.iter().map(|d| d.id.as_str()).collect();
            let local = store.sync_document_ids().await.unwrap_or_default();
            for (_, id) in local.iter().filter(|(c, id)| c == collection && !listed.contains(id.as_str())) {
                if forget(store, collection, id).await {
                    report.counts.removed += 1;
                    changed = true;
                }
            }
            for id in states.keys().filter(|id| !listed.contains(id.as_str())) {
                let _ = store.sync_state_delete(SyncSide::Multigpt, collection, id).await;
            }
        }
    }
    // Deselected collections: remove local copies and states.
    let local = store.sync_document_ids().await.unwrap_or_default();
    for (collection, id) in local.iter().filter(|(c, _)| !chosen.contains(c)) {
        if forget(store, collection, id).await {
            report.counts.removed += 1;
            changed = true;
        }
    }
    if let Ok(states) = store.sync_states(SyncSide::Multigpt, None).await {
        for s in states.iter().filter(|s| !chosen.contains(&s.collection)) {
            let _ = store.sync_state_delete(SyncSide::Multigpt, &s.collection, &s.item).await;
        }
    }
    changed
}

/// Remove the local copy and the state of a document.
async fn forget(store: &Store, collection: &str, id: &str) -> bool {
    let removed = store.sync_document_delete(collection, id).await.is_ok();
    let _ = store.sync_state_delete(SyncSide::Multigpt, collection, id).await;
    removed
}
