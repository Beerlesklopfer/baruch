// SPDX-License-Identifier: AGPL-3.0-or-later
//! MultiGPT → Baruch: die gewählten Sammlungen über `list_documents` (seitenweise, höchstens
//! [`super::Limits::documents`] je Sammlung) und `read_document` als lokale Kopien in `sync_document`.
//!
//! Nur Geändertes: Meldet `list_documents` eine Änderungszeit oder einen Hash (`remote_stamp`) und ist er
//! gleich dem gespeicherten, wird nicht gelesen; sonst wird gelesen und der Hash des Texts verglichen –
//! gleich → nichts geschrieben. Ein Text über [`super::Limits::document_bytes`] wird nicht gespeichert,
//! über [`super::Limits::run_bytes`] je Lauf endet das Holen (je mit Hinweis). Dokumente, die in einer
//! vollständig gelesenen Liste fehlen, und Kopien abgewählter Sammlungen werden lokal entfernt. Kann die
//! Liste unvollständig sein (eine volle Seite ohne `has_more`/Fortsetzung, etwa weil der Server `limit`
//! still kappt oder das Schema ein kleineres `maximum` nennt), wird nichts lokal gelöscht (Runde 19a, mit
//! Hinweis). Texte sind Daten aus Quellen, nie Anweisungen.

use std::collections::{HashMap, HashSet};

use baruch_rag::RagError;
use baruch_store::{Store, SyncSide, SyncState, sync_hash};

use super::args::{CURSOR_KEYS, Filler, Listed, PAGE_LIMIT_KEYS, document_text, listed};
use super::{Limits, Link, Report, why};
use crate::connections::multigpt::{LIST_DOCUMENTS_TOOL, READ_TOOL, page_key, read_all_pages};

/// Höchstens so viele Seiten `list_documents` je Sammlung.
const MAX_PAGES: usize = 50;

/// Alle Dokumente einer Sammlung (höchstens `max + 1`, damit ein Überschreiten auffällt); `true`, wenn
/// die Liste vollständig gelesen ist.
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
        // Erbetene Seitengröße (nach `maximum` des Schemas gekappt); `None` = das Schema kennt keine.
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
        // Eine volle Seite ohne Fortsetzung heißt nicht „fertig“: Kappt der Server still (oder das Schema mit
        // `maximum`), gibt es womöglich mehr.
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
            // Ehrlich unvollständig (Runde 19a), wenn es weiter gehen könnte: gemeldete Fortsetzung, die nicht
            // weiterführt (oder für die das Schema keinen Parameter kennt), bzw. eine volle Seite ohne Fortsetzung.
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

/// Ein Dokument lesen (Volltext). Liefert MultiGPT seitenweise (Seitenparameter im Schema und Seitenangaben
/// in der Antwort), werden alle Seiten gelesen und wie beim Volltext von quellen mit Leerzeile verbunden
/// ([`read_all_pages`]); so gleicht die lokale Kopie dem Online-Volltext (Hash, Index, Hervorhebungen).
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
        // Auch ein verlangter Seitenparameter ist bekannt; read_all_pages setzt ihn je Seite neu.
        f.number(&[key], 1);
    }
    let args = f.finish()?;
    // Seitenangaben erkennt nur die strukturierte Antwort; ohne Seitenparameter gilt der Text wie bisher.
    if page.is_none() {
        let result = session.call(READ_TOOL, args, None).await?;
        return Ok(document_text(&result));
    }
    // Obergrenze wie die Hülle einer Antwort; die Grenze je Dokument meldet `run` danach mit eigenem Hinweis.
    read_all_pages(session, &args, page, None, limits.transport().max_body).await
}

fn mib(bytes: usize) -> String {
    if bytes >= 1024 * 1024 && bytes.is_multiple_of(1024 * 1024) {
        format!("{} MiB", bytes / (1024 * 1024))
    } else {
        format!("{bytes} Bytes")
    }
}

/// MultiGPT → Baruch für alle gewählten Sammlungen. Rückgabe: Hat sich lokal etwas geändert (dann wird
/// der Index nachgezogen)?
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
    // Abgewählte Sammlungen: lokale Kopien und Stände entfernen.
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

/// Lokale Kopie und Stand eines Dokuments entfernen.
async fn forget(store: &Store, collection: &str, id: &str) -> bool {
    let removed = store.sync_document_delete(collection, id).await.is_ok();
    let _ = store.sync_state_delete(SyncSide::Multigpt, collection, id).await;
    removed
}
