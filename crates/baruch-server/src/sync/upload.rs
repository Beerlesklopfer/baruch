// SPDX-License-Identifier: AGPL-3.0-or-later
//! Baruch → MultiGPT: je Werk mit Schalter „nach MultiGPT hochladen“ ein Dokument je Kapitel in die
//! festgehaltene Sammlung. Ein Kapitel ist ein Gliederungspunkt der Art Kapitel samt allen folgenden
//! Abschnitten bis zum nächsten Kapitel bzw. Teil; ein Teil ist ein eigenes Dokument (nur sein Text), ebenso
//! Abschnitte vor dem ersten Kapitel. Text = aktuelle Fassungen in Gliederungsreihenfolge, Titel
//! „<Nummer> <Titel>“, Kennung `work:<W>/key:<K>` (fester Schlüssel des ersten Punkts), Hash über Titel und
//! Text ([`baruch_store::sync_hash`]).
//!
//! Geändert → erst neu hochladen, dann das alte Dokument löschen (so fehlt der Stand bei MultiGPT nie);
//! scheitert das Löschen, bleibt die alte ID als eigener Stand und wird im nächsten Lauf erneut gelöscht.
//! Fehlendes Kapitel → Dokument löschen. Dokumente gelöschter Werke → löschen.
//!
//! Härtung (Runde 19a):
//! - Ein Rechte-, Transport- oder Schemafehler (etwa HTTP 403 bzw. `isError` mit „docs.write“) beendet das
//!   Schreiben für diesen Lauf: Hinweis, `ok = 0`, lokal bleibt alles, der Rest kommt beim nächsten Lauf. Ein
//!   `isError` der Gegenstelle für **ein** Dokument (etwa ein leeres Kapitel) blockiert nur dieses.
//! - Nennt `upload_document` keine Dokument-ID, gibt es einen Hinweis und einen Sperrvermerk (Stand mit Hash,
//!   `remote_id` leer): Derselbe Inhalt geht nicht bei jedem Lauf erneut hinaus.
//! - Vor jedem Hochladen steht die Marke [`UPLOADING`] im Stand; bricht der Lauf zwischen Hochladen und Stand
//!   ab (Antwort verloren), meldet der nächste Lauf, dass das Kapitel bei MultiGPT doppelt liegen kann.

use std::collections::HashSet;

use baruch_core::{NodeKind, Work};
use baruch_rag::RagError;
use baruch_store::{Store, SyncSide, SyncState, WorkId, sync_hash};

use super::args::{CONTENT_KEYS, Filler, MIME_KEYS, TITLE_KEYS, uploaded_id};
use super::{Link, Report, why};
use crate::connections::multigpt::{DELETE_TOOL, UPLOAD_TOOL};

/// Ein Kapitel als Dokument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Chapter {
    pub item: String,
    pub key: i64,
    pub title: String,
    pub text: String,
    pub hash: String,
}

/// Kapitel eines Werks (siehe Modulkopf).
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

/// Kennung eines noch zu löschenden alten Dokuments eines Kapitels: beginnt wie die Kennung des Kapitels
/// (`work:<W>/…`), gehört aber zu keinem Kapitel – der Lauf löscht es darum wie ein fehlendes Kapitel.
fn stale_item(item: &str, old: &str) -> String {
    format!("{item}/stale:{old}")
}

/// Dateiname für Schemas, die `filename` statt `title` verlangen.
fn file_name(title: &str) -> String {
    let base: String =
        title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    format!("{}.md", base.trim())
}

/// Marke in `sync_state.remote_stamp` (Seite `baruch`): Hochladen begonnen, Ergebnis noch nicht festgehalten.
pub(crate) const UPLOADING: &str = "hochladen-begonnen";

/// Ein Dokument hochladen; liefert seine neue ID (`None`, wenn die Antwort keine nennt).
async fn put(link: &mut Link, collection: &str, chapter: &Chapter) -> Result<Option<String>, RagError> {
    let session = link.session().await?;
    let schema = session.schema(UPLOAD_TOOL).ok_or_else(|| {
        RagError::Schema(format!("MultiGPT bietet das Werkzeug {UPLOAD_TOOL} nicht an (tools/list)."))
    })?;
    let mut f = Filler::new(UPLOAD_TOOL, &schema);
    if !f.collection(collection) {
        return Err(RagError::Schema(format!("{UPLOAD_TOOL} nennt im inputSchema keinen Parameter für die Sammlung.")));
    }
    let titled = f.text(&TITLE_KEYS[..2], Some("title"), &chapter.title);
    let named = f.text(&["filename"], None, &file_name(&chapter.title));
    if !titled && !named {
        return Err(RagError::Schema(format!("{UPLOAD_TOOL} nennt im inputSchema keinen Parameter für den Titel.")));
    }
    if !f.text(CONTENT_KEYS, Some("content"), &chapter.text) {
        return Err(RagError::Schema(format!("{UPLOAD_TOOL} nennt im inputSchema keinen Parameter für den Text.")));
    }
    f.text(MIME_KEYS, None, "text/plain");
    let result = session.call(UPLOAD_TOOL, f.finish()?, None).await?;
    Ok(uploaded_id(&result))
}

/// Ein Dokument löschen.
async fn remove(link: &mut Link, collection: &str, remote_id: &str) -> Result<(), RagError> {
    let session = link.session().await?;
    let schema = session.schema(DELETE_TOOL).ok_or_else(|| {
        RagError::Schema(format!("MultiGPT bietet das Werkzeug {DELETE_TOOL} nicht an (tools/list)."))
    })?;
    let mut f = Filler::new(DELETE_TOOL, &schema);
    if !f.document(remote_id) {
        return Err(RagError::Schema(format!("{DELETE_TOOL} nennt im inputSchema keinen Parameter für das Dokument.")));
    }
    // Manche Schemas verlangen die Sammlung dazu; ohne `properties` genügt das Dokument.
    if schema.get("properties").is_some() {
        f.collection(collection);
    }
    session.call(DELETE_TOOL, f.finish()?, None).await.map(|_| ())
}

/// Schreibender Teil eines Laufs.
struct Writer<'a> {
    store: &'a Store,
    link: &'a mut Link,
    report: &'a mut Report,
    /// Nach einem Rechte- oder Transportfehler: nichts mehr schreiben (Grund steht schon im Bericht).
    blocked: bool,
}

/// Spricht ein `isError` der Gegenstelle von fehlenden Rechten? Dann gilt es für jedes Dokument (Annahme über
/// die Texte von MultiGPT, siehe `docs/Architektur.md`, Abschnitt „Abgleich“).
fn rights_text(message: &str) -> bool {
    let lower = message.to_lowercase();
    RIGHTS_WORDS.iter().any(|w| lower.contains(w))
}

/// Wörter, an denen Baruch ein Rechteproblem in einem `isError` erkennt.
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
    /// Blockiert dieser Fehler die übrigen Schreibvorgänge des Laufs? Ja bei Rechte-, Transport-, Schema- und
    /// Protokollfehlern; nein bei einem `isError` der Gegenstelle für genau dieses Dokument (etwa ein leeres
    /// Kapitel): Die Sitzung ist heil, die Antwort kam vollständig an, und der Text nennt keine Rechte.
    fn blocks(&self, e: &RagError) -> bool {
        match e {
            RagError::Failed(m) => !self.link.answered() || rights_text(m),
            _ => true,
        }
    }

    fn fail(&mut self, what: &str, e: &RagError) {
        self.report.counts.failed += 1;
        let text = format!("{what}: {}", why(e));
        self.report.problem(self.link, text);
        if self.blocks(e) {
            self.blocked = true;
            self.report.note(
                self.link,
                "Weitere Änderungen nach MultiGPT ruhen bis zum nächsten Abgleich; lokal ist nichts verloren.",
            );
        }
    }

    async fn store_fail(&mut self, e: baruch_store::StoreError) {
        self.report.counts.failed += 1;
        self.report.problem(self.link, format!("Speicher: {e}"));
    }

    /// Stand eines Kapitels zurücksetzen (nach einem Fehler, bei dem sicher nichts bei MultiGPT ankam): der
    /// vorige Stand bzw. gar keiner.
    async fn restore(&mut self, collection: &str, item: &str, known: Option<&SyncState>) {
        let restored = match known {
            Some(previous) => self.store.sync_state_put(previous).await,
            None => self.store.sync_state_delete(SyncSide::Baruch, collection, item).await,
        };
        if let Err(e) = restored {
            self.store_fail(e).await;
        }
    }

    /// Ein Werk abgleichen.
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

    /// Ein Kapitel abgleichen (siehe Modulkopf).
    async fn chapter(&mut self, work: WorkId, collection: &str, chapter: &Chapter, known: Option<&SyncState>) {
        if let Some(marker) = known.filter(|s| s.remote_stamp == UPLOADING) {
            self.report.note(
                self.link,
                format!(
                    "Der letzte Abgleich brach beim Hochladen von „{}“ ab (die Antwort von MultiGPT kam nicht an); dort \
                     liegt es womöglich ohne Zuordnung doppelt – bitte in der Sammlung „{collection}“ prüfen.",
                    marker.title
                ),
            );
        }
        // Vermerk ohne Hochladen-Marke: der Stand, wie er vor dem abgebrochenen Lauf war.
        let known_clean = known.map(|s| SyncState { remote_stamp: String::new(), ..s.clone() });
        if known_clean.as_ref().is_some_and(|s| s.hash == chapter.hash) {
            if known.is_some_and(|s| s.remote_stamp == UPLOADING) {
                self.restore(collection, &chapter.item, known_clean.as_ref()).await;
            }
            self.report.counts.unchanged += 1;
            return;
        }
        if let Err(e) = self.link.session().await {
            return self.fail(&format!("Hochladen von „{}“", chapter.title), &e);
        }
        // Marke vor dem Hochladen: Bricht der Lauf zwischen Hochladen und Stand ab (Antwort verloren, Absturz),
        // weiß der nächste Lauf davon und sagt es (das Dokument kann bei MultiGPT dann doppelt liegen).
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
                // Sicher nicht angekommen (Ablehnung, Schema, keine Verbindung): Marke zurücknehmen. Bei
                // Zeitüberschreitung oder abgerissener Antwort bleibt sie stehen (womöglich schon hochgeladen).
                let lost =
                    matches!(e, RagError::Timeout(_)) || (matches!(e, RagError::Failed(_)) && !self.link.answered());
                if !lost {
                    self.restore(collection, &chapter.item, known_clean.as_ref()).await;
                }
                return self.fail(&format!("Hochladen von „{}“", chapter.title), &e);
            }
        };
        if remote_id.is_none() {
            self.report.note(
                self.link,
                format!(
                    "„{}“: MultiGPT nannte keine Dokument-ID. Das Dokument liegt vermutlich dort, Baruch kann es aber \
                     weder ersetzen noch löschen; erneut hochgeladen wird es erst nach einer Änderung.",
                    chapter.title
                ),
            );
        }
        if let Some(old) = known_clean.as_ref().filter(|s| s.remote_id.is_empty() && !s.hash.is_empty()) {
            self.report.note(
                self.link,
                format!(
                    "Die vorige Fassung von „{}“ hatte keine Dokument-ID und bleibt bei MultiGPT liegen (bitte dort \
                     löschen).",
                    old.title
                ),
            );
        }
        let remote_id = remote_id.unwrap_or_default();
        // Sperrvermerk auch ohne ID: Stand mit Hash, damit derselbe Inhalt nicht bei jedem Lauf erneut hochgeht.
        marker.remote_id = remote_id.clone();
        marker.hash = chapter.hash.clone();
        marker.remote_stamp = String::new();
        let state = marker;
        // Erst den neuen Stand festhalten, dann das alte Dokument löschen: Scheitert das Löschen, zeigt
        // der Stand schon auf das neue Dokument, und die alte ID bleibt als eigener Stand (`stale_item`)
        // stehen; der nächste Lauf löscht sie wie ein fehlendes Kapitel (sonst läge das Kapitel für immer
        // doppelt in der Sammlung).
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
                    self.fail(&format!("Löschen der alten Fassung von „{}“ ({old})", chapter.title), &e);
                }
            },
            None => self.report.counts.uploaded += 1,
        }
    }

    /// Dokument eines fehlenden Kapitels (bzw. gelöschten Werks) löschen und den Stand entfernen.
    async fn drop_state(&mut self, gone: &SyncState) {
        if !gone.remote_id.is_empty()
            && let Err(e) = remove(self.link, &gone.collection, &gone.remote_id).await
        {
            return self.fail(&format!("Löschen von „{}“ ({})", gone.title, gone.remote_id), &e);
        }
        match self.store.sync_state_delete(SyncSide::Baruch, &gone.collection, &gone.item).await {
            Ok(()) => self.report.counts.removed += 1,
            Err(e) => self.store_fail(e).await,
        }
    }
}

/// Baruch → MultiGPT für alle Werke mit Schalter; danach Dokumente gelöschter Werke aufräumen.
pub(crate) async fn run(store: &Store, link: &mut Link, report: &mut Report) {
    let works = match store.sync_works().await {
        Ok(works) => works,
        Err(e) => {
            report.counts.failed += 1;
            report.problem(link, format!("Speicher: {e}"));
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
