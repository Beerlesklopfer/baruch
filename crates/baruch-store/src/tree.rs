// SPDX-License-Identifier: AGPL-3.0-or-later
//! Revision tree (contract Round 3, sections 2 and 3): switching the "current" marker, a revision of one's own on a
//! chosen base, changing and deleting by the human, tree of all revisions.
//!
//! Rules (migration `0004_revision_tree.sql`): every revision has a parent `based_on` (a smaller number of the same
//! section) or is a root. Per section exactly one `current`. Switching changes no text of the revision; the outline
//! follows its heading line ([`Store::switch_to`], contract Round 5). Changing concerns only text and note (the origin
//! is immutable) and sets `edited`. Deleting hangs the children on the parent of the deleted revision; the current
//! revision cannot be deleted. Numbers are never reused.

use baruch_core::{Author, RevisionStatus};
use sqlx::SqliteConnection;

use crate::write::{self, Row};
use crate::{Change, ChangeKind, OutlineChange, Store, StoreError, WorkId, parse_author, to_u32, to_usize, words};

/// A revision in the tree (without text).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionInfo {
    pub number: u32,
    /// Parent; `None` = root.
    pub based_on: Option<u32>,
    pub status: RevisionStatus,
    pub author: Author,
    pub note: String,
    pub created: String,
    /// Last change by the human; `None` = never changed.
    pub edited: Option<String>,
    /// Round 30b: marks of a variant (group, label A/B/C, line, findings); `None` = not a variant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<crate::VariantInfo>,
    /// Round 22: state of the revision (`revision.amend_seq`, 0 = never amended).
    pub amend_seq: u32,
    /// Round 22 (contract §2.5): state of the base on which a proposal is based (`revision.based_on_amend`); `None` =
    /// without one or created before Round 22.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on_amend: Option<u32>,
}

/// All revisions of a section (seq), by number.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NodeRevisions {
    pub node: usize,
    pub revisions: Vec<RevisionInfo>,
}

/// Complete state of a revision, for example before changing or deleting (for the log in the mirror).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionRecord {
    pub number: u32,
    pub body: String,
    pub note: String,
    pub status: RevisionStatus,
    pub based_on: Option<u32>,
    pub author: Author,
}

/// Result of [`Store::switch_to`]: the switch and, if the revision brings another level, the outline change (`action:
/// Level`, sub-items moved along with new revisions).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Switch {
    pub change: Change,
    pub outline: Option<OutlineChange>,
}

/// Name of the human for the wrappers [`Store::set_current`] and [`Store::accept`] (as in the server).
pub(crate) const HUMAN: &str = "Autor";

/// What [`Store::update_revision`] is to change; `None` = stays.
#[derive(Debug, Clone, Copy, Default)]
pub struct RevisionEdit<'a> {
    pub body: Option<&'a str>,
    pub note: Option<&'a str>,
}

fn record(row: &Row) -> Result<RevisionRecord, StoreError> {
    Ok(RevisionRecord {
        number: row.number,
        body: row.body.clone(),
        note: row.note.clone(),
        status: words::parse_status(&row.status)?,
        based_on: row.based_on,
        author: row.author.clone(),
    })
}

/// Changing through the tree. Round 23a (backlog „Nach Runde 22 offen“, code review store-1/web-3):
///
/// - If the **text** of the current revision or of one already amended (state > 0) changes, a step in the history
///   arises (`revision_amend`, kind 'amend', text before and after) and the state (`amend_seq`) rises by one. That way
///   the chain of steps stays without gaps (fast-forwarding in the sync, exchange), and an editor that has still loaded
///   the old state does not silently write over it (`NotOnCurrent` in [`crate::amend`]). A revision that was never
///   amended and is not current (such as a proposal) gets no history, as before.
/// - `expected_amend`: state that the caller had loaded. If it differs from the stored one, nothing is changed
///   (`StoreError::Stale`); `None` checks nothing (old clients).
async fn update(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
    edit: &RevisionEdit<'_>,
    human: &str,
    expected_amend: Option<u32>,
) -> Result<Option<Change>, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    let stored: i64 =
        sqlx::query_scalar("SELECT amend_seq FROM revision WHERE id = ?").bind(row.id).fetch_one(&mut *conn).await?;
    let amend_seq = crate::to_u32(stored, "amend_seq")?;
    if let Some(expected) = expected_amend
        && expected != amend_seq
    {
        return Err(StoreError::Stale(format!(
            "Veraltet: Fassung {rev} steht inzwischen auf Stand {amend_seq} (geladen war Stand {expected}); \
             bitte neu laden und die Änderung dort eintragen."
        )));
    }
    let body = edit.body.map(|b| b.replace("\r\n", "\n"));
    if body.as_deref().is_some_and(|b| b.trim().is_empty()) {
        return Err(StoreError::Refused("Eine Fassung braucht Text."));
    }
    let new_body = body.unwrap_or_else(|| row.body.clone());
    let new_note = edit.note.map_or_else(|| row.note.clone(), str::to_owned);
    if new_body == row.body && new_note == row.note {
        return Ok(None);
    }
    let step = (new_body != row.body && (row.status == "current" || amend_seq > 0)).then_some(amend_seq + 1);
    sqlx::query(
        "UPDATE revision SET body = ?, note = ?, amend_seq = ?, edited = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE id = ?",
    )
    .bind(&new_body)
    .bind(&new_note)
    .bind(i64::from(step.unwrap_or(amend_seq)))
    .bind(row.id)
    .execute(&mut *conn)
    .await?;
    if let Some(seq) = step {
        let author = if human.trim().is_empty() { HUMAN } else { human };
        sqlx::query(
            "INSERT INTO revision_amend (revision_id, seq, kind, undo_of, body_before, body_after, author_name) \
             VALUES (?, ?, 'amend', NULL, ?, ?, ?)",
        )
        .bind(row.id)
        .bind(i64::from(seq))
        .bind(&row.body)
        .bind(&new_body)
        .bind(author)
        .execute(&mut *conn)
        .await?;
    }
    Ok(Some(Change {
        work,
        node,
        rev,
        author: Author::Human { name: human.to_owned() },
        kind: ChangeKind::Updated,
        before: Some(record(&row)?),
        reparented: Vec::new(),
        amend_seq: step,
    }))
}

async fn delete(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
    human: &str,
) -> Result<Change, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    if row.status == "current" {
        return Err(StoreError::Refused(
            "Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.",
        ));
    }
    let children: Vec<i64> =
        sqlx::query_scalar("SELECT number FROM revision WHERE node_id = ? AND based_on = ? ORDER BY number")
            .bind(node_id)
            .bind(i64::from(rev))
            .fetch_all(&mut *conn)
            .await?;
    let reparented = children.into_iter().map(|n| to_u32(n, "Revisionsnummer")).collect::<Result<Vec<_>, _>>()?;
    // The trigger `revision_reparent` re-hangs the children.
    sqlx::query("DELETE FROM revision WHERE id = ?").bind(row.id).execute(&mut *conn).await?;
    Ok(Change {
        work,
        node,
        rev,
        author: Author::Human { name: human.to_owned() },
        kind: ChangeKind::Deleted,
        before: Some(record(&row)?),
        reparented,
        amend_seq: None,
    })
}

type InfoRow = (
    i64,
    Option<i64>,
    Option<i64>,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
);

async fn tree(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<NodeRevisions>, StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    if exists.is_none() {
        return Err(StoreError::UnknownWork(work));
    }
    let rows: Vec<InfoRow> = sqlx::query_as(
        "SELECT n.seq, r.number, r.based_on, r.status, r.author_kind, r.author_name, r.model, r.note, r.created, r.edited, \
         r.variant_group, r.variant_label, r.variant_summary, r.variant_findings, r.amend_seq, r.based_on_amend \
         FROM node n LEFT JOIN revision r ON r.node_id = n.id \
         WHERE n.work_id = ? ORDER BY n.seq, r.number",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut out: Vec<NodeRevisions> = Vec::new();
    for (
        seq,
        number,
        based_on,
        status,
        kind,
        name,
        model,
        note,
        created,
        edited,
        group,
        label,
        summary,
        findings,
        amend,
        base_amend,
    ) in rows
    {
        let seq = to_usize(seq, "seq")?;
        if out.last().is_none_or(|n| n.node != seq) {
            out.push(NodeRevisions { node: seq, revisions: Vec::new() });
        }
        let Some(number) = number else { continue }; // section without revisions
        let entry = out.last_mut().expect("eben angelegt");
        entry.revisions.push(RevisionInfo {
            number: to_u32(number, "Revisionsnummer")?,
            based_on: based_on.map(|b| to_u32(b, "based_on")).transpose()?,
            status: words::parse_status(&status)?,
            author: parse_author(&kind, name, model)?,
            note,
            created,
            edited,
            variant: crate::variants::from_columns(
                group,
                label,
                summary.unwrap_or_default(),
                findings.as_deref().unwrap_or("[]"),
            )?,
            amend_seq: to_u32(amend.unwrap_or(0), "amend_seq")?,
            based_on_amend: base_amend.map(|b| to_u32(b, "based_on_amend")).transpose()?,
        });
    }
    Ok(out)
}

/// Revisions that are not current, for the side branches in the mirror (Round 22, contract §2.12): per revision the
/// fixed key, the details from the tree and the section file as the mirror would carry it with this revision (path like
/// [`crate::snapshot::files`], content with the import line of the work template and the jump label `<kN>`; without the
/// conversion "number on/off" and without the index projection, which only apply to the current revision in context).
async fn side(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<Vec<(i64, RevisionInfo, crate::MirrorFile)>, StoreError> {
    let infos = tree(conn, work).await?;
    let template: String =
        sqlx::query_scalar("SELECT template FROM work WHERE id = ?").bind(work).fetch_one(&mut *conn).await?;
    let has_template = !template.is_empty();
    let rows: Vec<(i64, i64, String, i64, String)> = sqlx::query_as(
        "SELECT n.seq, n.id, n.title, r.number, r.body FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND r.status <> 'current' ORDER BY n.seq, r.number",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (seq, key, title, number, body) in rows {
        let seq = to_usize(seq, "seq")?;
        let number = to_u32(number, "Revisionsnummer")?;
        let Some(info) =
            infos.iter().find(|n| n.node == seq).and_then(|n| n.revisions.iter().find(|r| r.number == number)).cloned()
        else {
            continue;
        };
        let path = format!("abschnitte/{seq:03}-{}.typ", crate::slug(&title));
        let body = baruch_typst::xref::label_section(&body, key).into_owned();
        // As in the HEAD of the mirror (Round 22, „Spiegel setzen“): relative image paths from the root (review R22).
        let body = crate::snapshot::root_image_paths(&body).into_owned();
        let contents = if has_template { format!("{}{body}", crate::snapshot::TEMPLATE_IMPORT) } else { body };
        out.push((key, info, crate::MirrorFile { path, contents }));
    }
    Ok(out)
}

impl Store {
    /// Side branches of the mirror (Round 22, contract §2.12): all revisions that are not current (superseded,
    /// rejected, open proposals together with variants) with fixed key, tree details and the section file as the mirror
    /// would carry it with this revision. Unknown work → `UnknownWork`.
    pub async fn side_revisions(
        &self,
        work: WorkId,
    ) -> Result<Vec<(i64, RevisionInfo, crate::MirrorFile)>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        side(&mut conn, work).await
    }

    /// Switch the "current" marker (atomic, one transaction): previous `current` → `superseded`, `rev` → `current`; the
    /// text of the revision never changes. With `only_proposal` only for proposals (accepting, otherwise `Refused("Nur
    /// Vorschläge lassen sich übernehmen.")`), without it for every revision that is not current (already current →
    /// `Refused("Diese Fassung ist schon aktuell.")`). If `rev` was a proposal, `change.kind` = `Accepted`, otherwise
    /// `Switched`.
    ///
    /// The outline follows the heading line of the revision (contract Round 5): at the same level only the title; at
    /// another level, level and title, the sub-items move along by the same difference (at most level 3) and get new
    /// revisions by `human` that differ only in the heading line (`outline` with `action: Level`). No heading or
    /// further headings, text before it or a level that is invalid here → `Heading`, nothing changed. Parts: a revision
    /// with the part marker aligns the title (Round 14), one without it switches without aligning.
    pub async fn switch_to(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        only_proposal: bool,
        human: &str,
    ) -> Result<Switch, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = if only_proposal {
            write::accept(&mut tx, work, node, rev).await?
        } else {
            write::set_current(&mut tx, work, node, rev).await?
        };
        let outline = crate::outline::follow_switched(&mut tx, work, node, human).await?;
        tx.commit().await?;
        Ok(Switch { change, outline })
    }

    /// Switch to any revision that is not current: [`Store::switch_to`] without `only_proposal`, as the human „Autor“;
    /// only the change of the marker.
    pub async fn set_current(&self, work: WorkId, node: usize, rev: u32) -> Result<Change, StoreError> {
        self.switch_to(work, node, rev, false, HUMAN).await.map(|s| s.change)
    }

    /// Save a revision of one's own on the base `based_on` (atomic): new revision by the human, `current` at once,
    /// parent = `based_on`. CRLF becomes LF. Same text as the base or as the current revision → `Ok(None)`. Unknown
    /// base → `UnknownRevision`.
    pub async fn save_human_on(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: u32,
    ) -> Result<Option<Change>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = write::save_human(&mut tx, work, node, body, author, Some(based_on)).await?;
        if change.is_some() {
            crate::outline::check_current_heading(&mut tx, work, node).await?;
            tx.commit().await?;
        }
        Ok(change)
    }

    /// Change text and/or note of a revision (atomic, only for the human `human`) and set `edited`. Status and origin
    /// stay. CRLF becomes LF. No change → `Ok(None)`; empty text → `Refused("Eine Fassung braucht Text.")`.
    /// `Change.before` carries the state before.
    pub async fn update_revision(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        edit: &RevisionEdit<'_>,
        human: &str,
    ) -> Result<Option<Change>, StoreError> {
        self.update_revision_at(work, node, rev, edit, human, None).await
    }

    /// Like [`Store::update_revision`], with the state (`amend_seq`) that the caller had loaded (Round 23a): if the
    /// revision is on another state by now, nothing is changed (`StoreError::Stale` „Veraltet: …“). If the text of the
    /// current revision or of one already amended changes, a step in the history arises (kind 'amend') and the state
    /// rises (`Change.amend_seq` names it).
    pub async fn update_revision_at(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        edit: &RevisionEdit<'_>,
        human: &str,
        expected_amend: Option<u32>,
    ) -> Result<Option<Change>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = update(&mut tx, work, node, rev, edit, human, expected_amend).await?;
        if let Some(done) = &change {
            // Only the current revision determines title and outline; changing another revision checks nothing
            // (otherwise a current revision that already deviates would block every change).
            if done.before.as_ref().is_some_and(|b| b.status == baruch_core::RevisionStatus::Current) {
                crate::outline::check_current_heading(&mut tx, work, node).await?;
            }
            tx.commit().await?;
        }
        Ok(change)
    }

    /// Delete a revision (atomic, only for the human `human`). The current revision → `Refused("Die aktuelle Fassung
    /// lässt sich nicht löschen; erst auf eine andere umschalten.")`. Afterwards children hang on the parent of the
    /// deleted revision (`Change.reparented`), the number is never assigned again. `Change.before` carries the complete
    /// deleted state.
    pub async fn delete_revision(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        human: &str,
    ) -> Result<Change, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = delete(&mut tx, work, node, rev, human).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// Tree of all revisions (without text): all sections by seq, per section the revisions by number. Unknown work →
    /// `UnknownWork`.
    pub async fn revision_tree(&self, work: WorkId) -> Result<Vec<NodeRevisions>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        tree(&mut conn, work).await
    }
}
