// SPDX-License-Identifier: AGPL-3.0-or-later
//! Baruch → MultiGPT: per work with the switch „nach MultiGPT hochladen“ one document per chapter into the
//! recorded collection. A chapter is an outline item of the kind chapter together with all following
//! sections up to the next chapter or part; a part is a document of its own (only its text), as are
//! sections before the first chapter. Text = current revisions in outline order, title
//! „<Nummer> <Titel>“, identifier `work:<W>/key:<K>` (fixed key of the first item), hash over title and
//! text ([`baruch_store::sync_hash`]).
//!
//! Changed → first upload anew, then delete the old document (this way the state at MultiGPT is never missing);
//! if the deletion fails, the old ID stays as a state of its own and is deleted again in the next run.
//! Missing chapter → delete the document. Documents of deleted works → delete.
//!
//! Hardening (Round 19a):
//! - A rights, transport or schema error (such as HTTP 403 or `isError` with "docs.write") ends the
//!   writing for this run: note, `ok = 0`, locally everything stays, the rest comes with the next run. An
//!   `isError` of the remote side for **one** document (such as an empty chapter) blocks only this one.
//! - If `upload_document` names no document ID, there is a note and a lock note (state with hash,
//!   `remote_id` empty): the same content does not go out again with every run.
//! - Before every upload the mark [`UPLOADING`] stands in the state; if the run aborts between upload and state
//!   (answer lost), the next run reports that the chapter can lie twice at MultiGPT.

use std::collections::HashSet;

use baruch_core::{NodeKind, Work};
use baruch_rag::RagError;
use baruch_store::{Store, SyncSide, SyncState, WorkId, sync_hash};

use super::args::{CONTENT_KEYS, Filler, MIME_KEYS, TITLE_KEYS, uploaded_id};
use super::{Link, Report, why};
use crate::connections::multigpt::{DELETE_TOOL, UPLOAD_TOOL};

/// A chapter as a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chapter {
    pub item: String,
    pub key: i64,
    pub title: String,
    pub text: String,
    pub hash: String,
}

/// Chapters of a work (see the module header).
pub(crate) fn chapters(work: WorkId, loaded: &Work) -> Vec<Chapter> {
    let mut out: Vec<(i64, String, Vec<String>)> = Vec::new();
    for node in &loaded.nodes {
        if matches!(node.kind, NodeKind::Chapter | NodeKind::Part) || out.is_empty() {
            let title = if node.number.trim().is_empty() {
                node.title.trim().to_owned()
            } else {
                format!("{} {}", node.number.trim(), node.title.trim())
            };
            out.push((node.key, title, Vec::new()));
        }
        if let Some(current) = node.current_index().and_then(|i| node.revisions.get(i)) {
            let body = current.body.trim_end();
            if !body.is_empty() {
                out.last_mut().expect("eben angelegt").2.push(body.to_owned());
            }
        }
    }
    out.into_iter()
        .map(|(key, title, bodies)| {
            let text = if bodies.is_empty() { String::new() } else { format!("{}\n", bodies.join("\n\n")) };
            Chapter {
                item: format!("work:{work}/key:{key}"),
                key,
                hash: sync_hash(&format!("{title}\n{text}")),
                title,
                text,
            }
        })
        .collect()
}

/// Identifier of an old document of a chapter that is still to be deleted: begins like the identifier of the
/// chapter (`work:<W>/…`), but belongs to no chapter – the run therefore deletes it like a missing chapter.
fn stale_item(item: &str, old: &str) -> String {
    format!("{item}/stale:{old}")
}

/// File name for schemas that require `filename` instead of `title`.
fn file_name(title: &str) -> String {
    let base: String =
        title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    format!("{}.md", base.trim())
}

/// Mark in `sync_state.remote_stamp` (side `baruch`): upload begun, result not recorded yet.
pub(crate) const UPLOADING: &str = "hochladen-begonnen";

/// Upload a document; returns its new ID (`None` if the answer names none).
async fn put(link: &mut Link, collection: &str, chapter: &Chapter) -> Result<Option<String>, RagError> {
    let session = link.session().await?;
    let schema = session.schema(UPLOAD_TOOL).ok_or_else(|| super::missing_tool(UPLOAD_TOOL))?;
    let mut f = Filler::new(UPLOAD_TOOL, &schema);
    if !f.collection(collection) {
        return Err(super::schema_error("sync_msg.tool.no_collection_param", UPLOAD_TOOL));
    }
    let titled = f.text(&TITLE_KEYS[..2], Some("title"), &chapter.title);
    let named = f.text(&["filename"], None, &file_name(&chapter.title));
    if !titled && !named {
        return Err(super::schema_error("sync_msg.tool.no_title_param", UPLOAD_TOOL));
    }
    if !f.text(CONTENT_KEYS, Some("content"), &chapter.text) {
        return Err(super::schema_error("sync_msg.tool.no_text_param", UPLOAD_TOOL));
    }
    f.text(MIME_KEYS, None, "text/plain");
    let result = session.call(UPLOAD_TOOL, f.finish()?, None).await?;
    Ok(uploaded_id(&result))
}

/// Delete a document.
async fn remove(link: &mut Link, collection: &str, remote_id: &str) -> Result<(), RagError> {
    let session = link.session().await?;
    let schema = session.schema(DELETE_TOOL).ok_or_else(|| super::missing_tool(DELETE_TOOL))?;
    let mut f = Filler::new(DELETE_TOOL, &schema);
    if !f.document(remote_id) {
        return Err(super::schema_error("sync_msg.tool.no_document_param", DELETE_TOOL));
    }
    // Some schemas require the collection as well; without `properties` the document is enough.
    if schema.get("properties").is_some() {
        f.collection(collection);
    }
    session.call(DELETE_TOOL, f.finish()?, None).await.map(|_| ())
}

/// Writing part of a run.
struct Writer<'a> {
    store: &'a Store,
    link: &'a mut Link,
    report: &'a mut Report,
    /// After a rights or transport error: write nothing more (the reason is already in the report).
    blocked: bool,
}

/// Does an `isError` of the remote side speak of missing rights? Then it applies to every document (assumption
/// about the texts of MultiGPT, see `docs/Architektur.md`, section „Abgleich“).
fn rights_text(message: &str) -> bool {
    let lower = message.to_lowercase();
    RIGHTS_WORDS.iter().any(|w| lower.contains(w))
}

/// Words by which Baruch recognises a rights problem in an `isError`.
const RIGHTS_WORDS: &[&str] = &[
    "berechtigung",
    "permission",
    "forbidden",
    "unauthori",
    "docs.write",
    "docs.read",
    "nicht erlaubt",
    "verweigert",
    "denied",
    "zugriff",
    "scope",
    "401",
    "403",
];

impl Writer<'_> {
    /// Does this error block the other write operations of the run? Yes for rights, transport, schema and
    /// protocol errors; no for an `isError` of the remote side for exactly this document (such as an empty
    /// chapter): the session is intact, the answer arrived completely, and the text names no rights.
    fn blocks(&self, e: &RagError) -> bool {
        match e {
            RagError::Failed(m) => !self.link.answered() || rights_text(m),
            _ => true,
        }
    }

    fn fail(&mut self, what: &str, e: &RagError) {
        self.report.counts.failed += 1;
        let text = crate::i18n::tc("sync_msg.upload.problem", &[("what", what.into()), ("detail", why(e).into())]);
        self.report.problem(self.link, text);
        if self.blocks(e) {
            self.blocked = true;
            self.report.note(self.link, crate::i18n::tc("sync_msg.upload.resting", &[]));
        }
    }

    async fn store_fail(&mut self, e: baruch_store::StoreError) {
        self.report.counts.failed += 1;
        self.report.problem(self.link, super::store_note(&e));
    }

    /// Reset the state of a chapter (after an error where certainly nothing arrived at MultiGPT): the
    /// previous state or none at all.
    async fn restore(&mut self, collection: &str, item: &str, known: Option<&SyncState>) {
        let restored = match known {
            Some(previous) => self.store.sync_state_put(previous).await,
            None => self.store.sync_state_delete(SyncSide::Baruch, collection, item).await,
        };
        if let Err(e) = restored {
            self.store_fail(e).await;
        }
    }

    /// Reconcile one work.
    async fn work(&mut self, work: WorkId, collection: &str) {
        let loaded = match self.store.load_work(work).await {
            Ok(loaded) => loaded,
            Err(e) => return self.store_fail(e).await,
        };
        let chapters = chapters(work, &loaded);
        let prefix = format!("work:{work}/");
        let states: Vec<SyncState> = match self.store.sync_states(SyncSide::Baruch, Some(collection)).await {
            Ok(states) => states.into_iter().filter(|s| s.item.starts_with(&prefix)).collect(),
            Err(e) => return self.store_fail(e).await,
        };
        for chapter in &chapters {
            if self.blocked {
                return;
            }
            self.chapter(work, collection, chapter, states.iter().find(|s| s.item == chapter.item)).await;
        }
        let present: HashSet<&str> = chapters.iter().map(|c| c.item.as_str()).collect();
        for gone in states.iter().filter(|s| !present.contains(s.item.as_str())) {
            if self.blocked {
                return;
            }
            self.drop_state(gone).await;
        }
    }

    /// Reconcile one chapter (see the module header).
    async fn chapter(&mut self, work: WorkId, collection: &str, chapter: &Chapter, known: Option<&SyncState>) {
        if let Some(marker) = known.filter(|s| s.remote_stamp == UPLOADING) {
            self.report.note(
                self.link,
                crate::i18n::tc(
                    "sync_msg.upload.aborted",
                    &[("title", (&marker.title).into()), ("collection", collection.into())],
                ),
            );
        }
        // Record without the upload mark: the state as it was before the aborted run.
        let known_clean = known.map(|s| SyncState { remote_stamp: String::new(), ..s.clone() });
        if known_clean.as_ref().is_some_and(|s| s.hash == chapter.hash) {
            if known.is_some_and(|s| s.remote_stamp == UPLOADING) {
                self.restore(collection, &chapter.item, known_clean.as_ref()).await;
            }
            self.report.counts.unchanged += 1;
            return;
        }
        if let Err(e) = self.link.session().await {
            return self.fail(&crate::i18n::tc("sync_msg.upload.uploading", &[("title", (&chapter.title).into())]), &e);
        }
        // Mark before uploading: if the run aborts between upload and state (answer lost, crash),
        // the next run knows about it and says so (the document can then lie twice at MultiGPT).
        let mut marker = SyncState {
            side: SyncSide::Baruch,
            collection: collection.to_owned(),
            item: chapter.item.clone(),
            work: Some(work),
            node_key: Some(chapter.key),
            remote_id: known_clean.as_ref().map(|s| s.remote_id.clone()).unwrap_or_default(),
            title: chapter.title.clone(),
            hash: known_clean.as_ref().map(|s| s.hash.clone()).unwrap_or_default(),
            remote_stamp: UPLOADING.to_owned(),
            synced: String::new(),
        };
        if let Err(e) = self.store.sync_state_put(&marker).await {
            return self.store_fail(e).await;
        }
        let remote_id = match put(self.link, collection, chapter).await {
            Ok(id) => id,
            Err(e) => {
                // Certainly not arrived (rejection, schema, no connection): take the mark back. On a
                // timeout or a torn-off answer it stays (possibly already uploaded).
                let lost =
                    matches!(e, RagError::Timeout(_)) || (matches!(e, RagError::Failed(_)) && !self.link.answered());
                if !lost {
                    self.restore(collection, &chapter.item, known_clean.as_ref()).await;
                }
                return self
                    .fail(&crate::i18n::tc("sync_msg.upload.uploading", &[("title", (&chapter.title).into())]), &e);
            }
        };
        if remote_id.is_none() {
            self.report
                .note(self.link, crate::i18n::tc("sync_msg.upload.no_id", &[("title", (&chapter.title).into())]));
        }
        if let Some(old) = known_clean.as_ref().filter(|s| s.remote_id.is_empty() && !s.hash.is_empty()) {
            self.report
                .note(self.link, crate::i18n::tc("sync_msg.upload.old_no_id", &[("title", (&old.title).into())]));
        }
        let remote_id = remote_id.unwrap_or_default();
        // Lock note also without an ID: state with hash, so that the same content is not uploaded again with every
        // run.
        marker.remote_id = remote_id.clone();
        marker.hash = chapter.hash.clone();
        marker.remote_stamp = String::new();
        let state = marker;
        // First record the new state, then delete the old document: if the deletion fails, the
        // state already points to the new document, and the old ID stays as a state of its own (`stale_item`);
        // the next run deletes it like a missing chapter (otherwise the chapter would lie twice in the
        // collection for ever).
        if let Err(e) = self.store.sync_state_put(&state).await {
            return self.store_fail(e).await;
        }
        let old = known_clean.as_ref().map(|s| s.remote_id.clone()).filter(|id| !id.is_empty() && *id != remote_id);
        match old {
            Some(old) => match remove(self.link, collection, &old).await {
                Ok(()) => self.report.counts.replaced += 1,
                Err(e) => {
                    self.report.counts.uploaded += 1;
                    let stale = SyncState {
                        item: stale_item(&chapter.item, &old),
                        remote_id: old.clone(),
                        title: format!("{} (alte Fassung)", chapter.title),
                        hash: String::new(),
                        ..state
                    };
                    if let Err(e) = self.store.sync_state_put(&stale).await {
                        self.store_fail(e).await;
                    }
                    let what = crate::i18n::tc(
                        "sync_msg.upload.delete_old",
                        &[("title", (&chapter.title).into()), ("id", old.as_str().into())],
                    );
                    self.fail(&what, &e);
                }
            },
            None => self.report.counts.uploaded += 1,
        }
    }

    /// Delete the document of a missing chapter (or of a deleted work) and remove the state.
    async fn drop_state(&mut self, gone: &SyncState) {
        if !gone.remote_id.is_empty()
            && let Err(e) = remove(self.link, &gone.collection, &gone.remote_id).await
        {
            let what = crate::i18n::tc(
                "sync_msg.upload.delete",
                &[("title", (&gone.title).into()), ("id", (&gone.remote_id).into())],
            );
            return self.fail(&what, &e);
        }
        match self.store.sync_state_delete(SyncSide::Baruch, &gone.collection, &gone.item).await {
            Ok(()) => self.report.counts.removed += 1,
            Err(e) => self.store_fail(e).await,
        }
    }
}

/// Baruch → MultiGPT for all works with the switch; afterwards clean up documents of deleted works.
pub(crate) async fn run(store: &Store, link: &mut Link, report: &mut Report) {
    let works = match store.sync_works().await {
        Ok(works) => works,
        Err(e) => {
            report.counts.failed += 1;
            report.problem(link, super::store_note(&e));
            return;
        }
    };
    let mut writer = Writer { store, link, report, blocked: false };
    for sw in works.iter().filter(|w| w.upload && !w.collection.is_empty()) {
        if writer.blocked {
            break;
        }
        writer.work(sw.work, &sw.collection).await;
    }
    let alive: HashSet<WorkId> = works.iter().map(|w| w.work).collect();
    let orphans = match store.sync_states(SyncSide::Baruch, None).await {
        Ok(states) => states.into_iter().filter(|s| s.work.is_none_or(|w| !alive.contains(&w))).collect(),
        Err(e) => {
            writer.store_fail(e).await;
            Vec::new()
        }
    };
    for gone in &orphans {
        if writer.blocked {
            break;
        }
        writer.drop_state(gone).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_keep_letters_and_replace_the_rest() {
        assert_eq!(file_name("1.2 Aussaat/Ernte"), "1_2 Aussaat_Ernte.md", "Punkt und Schrägstrich ersetzt");
        assert_eq!(file_name("Frühling"), "Frühling.md", "Umlaute bleiben");
    }
}
