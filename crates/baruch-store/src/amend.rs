// SPDX-License-Identifier: AGPL-3.0-or-later
//! Amending the current revision (Round 22, contract §2.1–2.4, migration 0050).
//!
//! - If the human saves in the editor in the mode [`SaveMode::Auto`] and no rule for a new revision applies
//!   (`baruch_core::revision_policy::decide`), the current revision is amended: new text, `revision.amend_seq` + 1 and
//!   a row in `revision_amend` (kind 'amend', text before and after). Number, origin, status, `based_on` and `edited`
//!   stay (`edited` is only set by changing through the tree).
//! - [`Store::amend_undo`] takes back through the history – as a new step (kind 'undo'), never by deleting.
//! - Setting per work: table `work_revision_policy`, if the row is missing → [`RevisionPolicy::default`].
//! - Only saving from the editor (`outline_save_mode`, `save_human_mode` in the mode `Auto`) amends; all other write
//!   paths (inserting an image, index marks, references, replacing, outline) keep creating new revisions (contract
//!   §2.7).

use baruch_core::Author;
use baruch_core::revision_policy::{NewReason, SaveFacts, change_percent, decide};
use sqlx::SqliteConnection;

use crate::write;
use crate::{
    Change, ChangeKind, OutlineSave, Restructure, RevisionPolicy, SaveMode, Store, StoreError, WorkId, broken, to_u32,
};

/// Kind of a step in the history of the amendments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AmendKind {
    Amend,
    Undo,
}

impl AmendKind {
    /// Value of the column `revision_amend.kind`.
    pub fn as_db(self) -> &'static str {
        match self {
            AmendKind::Amend => "amend",
            AmendKind::Undo => "undo",
        }
    }

    /// From the column `revision_amend.kind`; anything unknown → `None`.
    pub fn from_db(s: &str) -> Option<AmendKind> {
        match s {
            "amend" => Some(AmendKind::Amend),
            "undo" => Some(AmendKind::Undo),
            _ => None,
        }
    }
}

/// A step in the history of a revision (row from `revision_amend`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AmendStep {
    pub uuid: String,
    pub seq: u32,
    pub kind: AmendKind,
    pub undo_of: Option<u32>,
    pub author: String,
    pub created: String,
    pub body_before: String,
    pub body_after: String,
}

/// Result of [`Store::set_revision_policy`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionPolicyChange {
    pub work: WorkId,
    pub before: RevisionPolicy,
    pub after: RevisionPolicy,
    /// Changed fields: `amend`, `max_change_percent`, `new_after_minutes`.
    pub changed: Vec<&'static str>,
}

/// The current revision of a section with everything that [`decide`] and the amending need.
#[derive(Debug, Clone)]
pub(crate) struct CurrentState {
    pub id: i64,
    pub number: u32,
    pub body: String,
    pub by_human: bool,
    pub amend_seq: u32,
    /// Minutes since the last write: max(creation of the revision, last amendment or undo).
    pub minutes: u64,
}

type StateRow = (i64, i64, String, String, i64, Option<f64>);

/// Current revision of the section (`None` without a current revision).
pub(crate) async fn current_state(
    conn: &mut SqliteConnection,
    node_id: i64,
) -> Result<Option<CurrentState>, StoreError> {
    let rows: Vec<StateRow> = sqlx::query_as(
        "SELECT r.id, r.number, r.body, r.author_kind, r.amend_seq, \
         (julianday('now') - max(julianday(r.created), \
              coalesce((SELECT max(julianday(a.created)) FROM revision_amend a WHERE a.revision_id = r.id), \
                       julianday(r.created)))) * 1440.0 \
         FROM revision r WHERE r.node_id = ? AND r.status = 'current'",
    )
    .bind(node_id)
    .fetch_all(&mut *conn)
    .await?;
    match rows.len() {
        0 => Ok(None),
        1 => {
            let (id, number, body, kind, amend_seq, minutes) = rows.into_iter().next().expect("eine Zeile");
            Ok(Some(CurrentState {
                id,
                number: to_u32(number, "Revisionsnummer")?,
                body,
                by_human: kind == "human",
                amend_seq: to_u32(amend_seq, "amend_seq")?,
                // Unreadable time (NULL) or a clock set back (negative): count as "just written".
                minutes: minutes.filter(|m| m.is_finite() && *m > 0.0).map_or(0, |m| m.floor() as u64),
            }))
        }
        n => Err(broken(format!("Knoten {node_id}: {n} aktuelle Revisionen statt genau einer"))),
    }
}

/// Setting „Fassungen“ in the transaction of the caller (if the row is missing → default).
pub(crate) async fn policy_in(conn: &mut SqliteConnection, work: WorkId) -> Result<RevisionPolicy, StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    if exists.is_none() {
        return Err(StoreError::UnknownWork(work));
    }
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT amend, max_change_percent, new_after_minutes FROM work_revision_policy WHERE work_id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(match row {
        None => RevisionPolicy::default(),
        Some((amend, percent, minutes)) => RevisionPolicy {
            amend: amend != 0,
            max_change_percent: u8::try_from(percent).map_err(|_| broken(format!("max_change_percent {percent}")))?,
            new_after_minutes: u32::try_from(minutes).map_err(|_| broken(format!("new_after_minutes {minutes}")))?,
        },
    })
}

/// Result of the check before saving in the mode `Auto`.
pub(crate) enum Verdict {
    /// Amend (the current revision of the human).
    Amend(CurrentState),
    /// New revision with a reason.
    New(NewReason),
    /// Same text as the current revision: nothing to do (the old path reports `None`).
    Unchanged,
}

/// Rules from contract §2.2 for a text `stored` (the way it would be saved) on the base `based_on`.
///
/// `based_on_amend` is the state of the base that the editor had loaded (follow-up fix R22): if the current revision
/// has been amended since (another state), the base counts as not current (`NotOnCurrent`) – otherwise an outdated tab
/// would silently overwrite the amendment made in the meantime. If the state is missing (old clients, RPC without
/// `based_on_amend`), only the revision number counts as before.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn verdict(
    conn: &mut SqliteConnection,
    work: WorkId,
    node_id: i64,
    based_on: Option<u32>,
    based_on_amend: Option<u32>,
    stored: &str,
    heading_changed: bool,
) -> Result<Verdict, StoreError> {
    let policy = policy_in(conn, work).await?;
    let current = current_state(conn, node_id).await?;
    if current.as_ref().is_some_and(|c| c.body == stored) {
        return Ok(Verdict::Unchanged);
    }
    let facts = SaveFacts {
        has_current: current.is_some(),
        current_by_human: current.as_ref().is_some_and(|c| c.by_human),
        // Round 23a (code review store-3): the state also counts if a client sends only `based_on_amend` (without
        // `based_on`): it then refers to the current revision. Before, it was skipped in that case.
        base_is_current: match (&current, based_on) {
            (Some(c), Some(base)) => c.number == base && based_on_amend.is_none_or(|seq| seq == c.amend_seq),
            (Some(c), None) => based_on_amend.is_none_or(|seq| seq == c.amend_seq),
            _ => true,
        },
        heading_changed,
        change_percent: current.as_ref().map_or(100.0, |c| change_percent(&c.body, stored)),
        minutes_since_last_write: current.as_ref().map_or(0, |c| c.minutes),
    };
    Ok(match (decide(&policy, SaveMode::Auto, &facts), current) {
        (None, Some(current)) => Verdict::Amend(current),
        (Some(reason), _) => Verdict::New(reason),
        // Excluded by `decide` (without a current revision `NoCurrent` applies), for safety.
        (None, None) => Verdict::New(NewReason::NoCurrent),
    })
}

/// Write a step: replace the text, `amend_seq` + 1, row in `revision_amend` (kind `kind`). `edited` stays (only
/// changing through the tree sets that). Result: the change with the new state.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn write_step(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    current: &CurrentState,
    after: &str,
    human: &str,
    kind: AmendKind,
    undo_of: Option<u32>,
) -> Result<Change, StoreError> {
    let seq = current.amend_seq + 1;
    let done = sqlx::query(
        "UPDATE revision SET body = ?, amend_seq = ? WHERE id = ? AND amend_seq = ? AND status = 'current'",
    )
    .bind(after)
    .bind(i64::from(seq))
    .bind(current.id)
    .bind(i64::from(current.amend_seq))
    .execute(&mut *conn)
    .await?;
    if done.rows_affected() != 1 {
        return Err(broken(format!("Abschnitt {node}: Fassung {} nicht fortgeschrieben", current.number)));
    }
    let author = if human.trim().is_empty() { crate::tree::HUMAN } else { human };
    sqlx::query(
        "INSERT INTO revision_amend (revision_id, seq, kind, undo_of, body_before, body_after, author_name) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(current.id)
    .bind(i64::from(seq))
    .bind(kind.as_db())
    .bind(undo_of.map(i64::from))
    .bind(&current.body)
    .bind(after)
    .bind(author)
    .execute(&mut *conn)
    .await?;
    Ok(Change {
        work,
        node,
        rev: current.number,
        author: Author::Human { name: author.to_owned() },
        kind: match kind {
            AmendKind::Amend => ChangeKind::Amended,
            AmendKind::Undo => ChangeKind::AmendUndone,
        },
        before: None,
        reparented: Vec::new(),
        amend_seq: Some(seq),
    })
}

/// First heading (level, title) of a text; without a heading at the beginning `None`.
pub(crate) fn heading_key(body: &str) -> Option<(u8, String)> {
    let doc = baruch_typst::parse(body).ok()?;
    if !doc.template.trim().is_empty() {
        return None;
    }
    doc.headings.into_iter().next().map(|h| (h.level, h.title))
}

type StepRow = (Option<String>, i64, String, Option<i64>, String, String, String, String);

async fn history(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
) -> Result<Vec<AmendStep>, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    let rows: Vec<StepRow> = sqlx::query_as(
        "SELECT uuid, seq, kind, undo_of, author_name, created, body_before, body_after \
         FROM revision_amend WHERE revision_id = ? ORDER BY seq",
    )
    .bind(row.id)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|(uuid, seq, kind, undo_of, author, created, body_before, body_after)| {
            Ok(AmendStep {
                uuid: uuid.unwrap_or_default(),
                seq: to_u32(seq, "seq")?,
                kind: AmendKind::from_db(&kind).ok_or_else(|| broken(format!("unbekannte Art {kind:?}")))?,
                undo_of: undo_of.map(|u| to_u32(u, "undo_of")).transpose()?,
                author,
                created,
                body_before,
                body_after,
            })
        })
        .collect()
}

async fn undo(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
    seq: u32,
    human: &str,
) -> Result<Change, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    let Some(current) = current_state(conn, node_id).await? else {
        return Err(StoreError::Refused("Der Abschnitt hat keine aktuelle Fassung."));
    };
    if current.id != row.id {
        return Err(StoreError::Refused(
            "Rückgängig über den Verlauf geht nur auf der aktuellen Fassung; erst auf diese Fassung umschalten.",
        ));
    }
    if !current.by_human {
        return Err(StoreError::Refused(
            "Rückgängig über den Verlauf geht nur auf einer Fassung des Menschen (nicht auf Vorschlägen von Personas).",
        ));
    }
    let before: Option<String> =
        sqlx::query_scalar("SELECT body_before FROM revision_amend WHERE revision_id = ? AND seq = ?")
            .bind(row.id)
            .bind(i64::from(seq))
            .fetch_optional(&mut *conn)
            .await?;
    let Some(before) = before else {
        return Err(StoreError::Refused("Diesen Schritt gibt es im Verlauf dieser Fassung nicht."));
    };
    if before == current.body {
        return Err(StoreError::Refused("Der Text steht schon auf diesem Stand; nichts zurückzunehmen."));
    }
    // An amendment never changes the heading; so a state before it has the same one. Checked for safety.
    if heading_key(&before) != heading_key(&current.body) {
        return Err(StoreError::Refused(
            "Der Stand vor diesem Schritt hat eine andere Überschrift; bitte als neue Fassung speichern.",
        ));
    }
    write_step(conn, work, node, &current, &before, human, AmendKind::Undo, Some(seq)).await
}

/// Saving by the human in the mode `Auto` without outline logic (RPC `revision.save`): amend or new revision like
/// [`write::save_human`], with a reason.
async fn save_auto(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
    author: &str,
    based_on: Option<u32>,
    based_on_amend: Option<u32>,
) -> Result<(Option<Change>, Option<NewReason>), StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let body = body.replace("\r\n", "\n");
    if let Some(base) = based_on {
        write::row(conn, node_id, node, base).await?; // unknown base → UnknownRevision
    }
    let current_body = write::current(conn, node_id).await?.map(|(_, b, _)| b);
    let heading_changed = current_body.as_deref().is_some_and(|old| heading_key(old) != heading_key(&body));
    match verdict(conn, work, node_id, based_on, based_on_amend, &body, heading_changed).await? {
        Verdict::Amend(current) => {
            let change = write_step(conn, work, node, &current, &body, author, AmendKind::Amend, None).await?;
            Ok((Some(change), None))
        }
        Verdict::New(reason) => {
            let change = write::save_human(conn, work, node, &body, author, based_on).await?;
            let reason = change.as_ref().map(|_| reason);
            Ok((change, reason))
        }
        Verdict::Unchanged => Ok((None, None)),
    }
}

impl Store {
    /// History of the amendments of the revision `rev` in the section `node` (seq), by `seq`. Unknown revision →
    /// `UnknownRevision`; a revision that was never amended has an empty history.
    pub async fn amend_history(&self, work: WorkId, node: usize, rev: u32) -> Result<Vec<AmendStep>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        history(&mut conn, work, node, rev).await
    }

    /// Sets the text of the current revision of the human to the state before step `seq` – as a new step (kind 'undo',
    /// `undo_of = seq`); history rows are never deleted. Only on the current revision and only on a revision of the
    /// human, otherwise `Refused` with a German message.
    pub async fn amend_undo(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        seq: u32,
        human: &str,
    ) -> Result<Change, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = undo(&mut tx, work, node, rev, seq, human).await?;
        crate::outline::check_current_heading(&mut tx, work, node).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// Setting „Fassungen“ of the work (if the row is missing → default). Unknown work → `UnknownWork`.
    pub async fn revision_policy(&self, work: WorkId) -> Result<RevisionPolicy, StoreError> {
        let mut conn = self.pool.acquire().await?;
        policy_in(&mut conn, work).await
    }

    /// Set the setting „Fassungen“ of the work (human only). Limits after [`RevisionPolicy::validate`] (`Refused` with
    /// the limits); `changed` names the changed fields (empty = nothing changed, then nothing is written either).
    pub async fn set_revision_policy(
        &self,
        work: WorkId,
        policy: &RevisionPolicy,
        human: &str,
    ) -> Result<RevisionPolicyChange, StoreError> {
        if policy.validate().is_err() {
            let (lo, hi) = baruch_core::revision_policy::MAX_CHANGE_PERCENT;
            return Err(StoreError::Refused(if !(lo..=hi).contains(&policy.max_change_percent) {
                "Größte Änderung: 1 bis 100 Prozent."
            } else {
                "Neue Fassung nach: 0 bis 1440 Minuten (0 = keine Zeitgrenze)."
            }));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = policy_in(&mut tx, work).await?;
        let mut changed = Vec::new();
        if before.amend != policy.amend {
            changed.push("amend");
        }
        if before.max_change_percent != policy.max_change_percent {
            changed.push("max_change_percent");
        }
        if before.new_after_minutes != policy.new_after_minutes {
            changed.push("new_after_minutes");
        }
        if !changed.is_empty() {
            sqlx::query(
                "INSERT INTO work_revision_policy (work_id, amend, max_change_percent, new_after_minutes, updated_by) \
                 VALUES (?, ?, ?, ?, ?) ON CONFLICT(work_id) DO UPDATE SET amend = excluded.amend, \
                 max_change_percent = excluded.max_change_percent, new_after_minutes = excluded.new_after_minutes, \
                 updated = strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_by = excluded.updated_by",
            )
            .bind(work)
            .bind(i64::from(policy.amend))
            .bind(i64::from(policy.max_change_percent))
            .bind(i64::from(policy.new_after_minutes))
            .bind(human.chars().take(200).collect::<String>())
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Ok(RevisionPolicyChange { work, before, after: *policy, changed })
    }

    /// Saving by the human with a mode: `Auto` amends if the rules allow it, `New` creates a new revision like
    /// [`Store::save_human`] or [`Store::save_human_on`].
    pub async fn save_human_mode(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: Option<u32>,
        mode: SaveMode,
    ) -> Result<Option<Change>, StoreError> {
        self.save_human_with_reason(work, node, body, author, based_on, mode).await.map(|(change, _)| change)
    }

    /// Like [`Store::save_human_mode`], plus the reason for a new revision (only in the mode `Auto`, otherwise `None`).
    pub async fn save_human_with_reason(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: Option<u32>,
        mode: SaveMode,
    ) -> Result<(Option<Change>, Option<NewReason>), StoreError> {
        self.save_human_at(work, node, body, author, based_on, None, mode).await
    }

    /// Like [`Store::save_human_with_reason`] with the state of the base (`based_on_amend`) that the client had loaded:
    /// if the current revision has been amended since, the mode `Auto` creates a new revision (`NotOnCurrent`) instead
    /// of a silent amendment over the text of the meantime.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_human_at(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: Option<u32>,
        based_on_amend: Option<u32>,
        mode: SaveMode,
    ) -> Result<(Option<Change>, Option<NewReason>), StoreError> {
        if mode == SaveMode::New {
            let change = match based_on {
                Some(base) => self.save_human_on(work, node, body, author, base).await?,
                None => self.save_human(work, node, body, author).await?,
            };
            return Ok((change, None));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = save_auto(&mut tx, work, node, body, author, based_on, based_on_amend).await?;
        if done.0.is_some() {
            crate::outline::check_current_heading(&mut tx, work, node).await?;
            tx.commit().await?;
        }
        Ok(done)
    }

    /// Saving from the editor with a mode (like [`Store::outline_save`], in the mode `Auto` with amending;
    /// `OutlineSave::new_reason` then names the reason for a new revision).
    #[allow(clippy::too_many_arguments)]
    pub async fn outline_save_mode(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        based_on: Option<u32>,
        allow: Restructure,
        human: &str,
        mode: SaveMode,
    ) -> Result<OutlineSave, StoreError> {
        self.outline_save_at(work, node, body, based_on, None, allow, human, mode).await
    }

    /// Like [`Store::outline_save_mode`] with the state of the base (`based_on_amend`) that the editor had loaded (form
    /// field `based_on_amend`): if the current revision has been amended since, the mode `Auto` creates a new revision
    /// (`NotOnCurrent`), both texts stay visible as revisions.
    #[allow(clippy::too_many_arguments)]
    pub async fn outline_save_at(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        based_on: Option<u32>,
        based_on_amend: Option<u32>,
        allow: Restructure,
        human: &str,
        mode: SaveMode,
    ) -> Result<OutlineSave, StoreError> {
        if mode == SaveMode::New {
            return self.outline_save(work, node, body, based_on, allow, human).await;
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = crate::outline::save_auto(&mut tx, work, node, body, based_on, based_on_amend, allow, human).await?;
        if done.change.is_some() || done.outline.is_some() {
            tx.commit().await?;
        }
        Ok(done)
    }
}
