// SPDX-License-Identifier: AGPL-3.0-or-later
//! Findings (Round 28, migration 0046; interface S1, afterwards the module belongs to befunde).
//!
//! A finding is a record from the answer of a persona: kind ([`FindingKind`]: praise, hint, critical), status
//! ([`FindingStatus`]: open, adopted, rejected, deferred, done without change), origin (post in the section chat or
//! thread post, position in the answer), optionally a quotation and a check note.
//!
//! Rules (contract R28 §1):
//! - Only the human sets the status ([`Store::finding_set_status`]); assistants only make proposals
//!   ([`Store::finding_propose_status`], only adopted/deferred/done), on which the human decides
//!   ([`Store::finding_decide`]). Every change is in `finding_status` (the initial creation is not).
//! - Counted as open (badge, [`Store::finding_open_counts`]): `status = 'open'`, not attached (`similar_to IS NULL`),
//!   not praise, with a section.
//! - German labels only through [`FindingKind::label`] and [`FindingStatus::label`].
//!
//! - Mirror: `befunde.md` ([`mirror_files`]) only if the work has findings; deterministic by part, chapter, section
//!   (tree order), then threads, then findings without a reference.
//!
//! S1 implemented the reading queries and the plain [`Store::finding_insert`]; status changes, proposals, decisions and
//! `befunde.md` came with R28a (task befunde).
//!
//! Round 22 (befund-rueckfrage, migration 0050): when adopting, the store remembers the amendment state of the revision
//! (`revision.amend_seq`) in `finding.adopted_amend` and `finding_status.adopted_amend`: when the human sets it, the
//! state at that moment; for a proposal by an assistant the state at the proposal (only in the entry) and on
//! confirmation the state at the confirmation (entry and finding). Every other status sets `finding.adopted_amend` to
//! `NULL`. Display „Fassung n · Stand k“ only for k > 0 ([`adopted_words`]).

use std::collections::HashMap;

use sqlx::{Row, SqliteConnection};

use crate::ideas::IdeaOrigin;
use crate::{MirrorFile, Store, StoreError, WorkId, to_u32, to_usize};

/// The text of a finding has at most this many characters.
pub const MAX_FINDING_CHARS: usize = 4000;
/// The quotation of a finding has at most this many characters.
pub const MAX_QUOTE_CHARS: usize = 2000;
/// A reason (rejected, deferred, done) has at most this many characters.
pub const MAX_REASON_CHARS: usize = 2000;
/// The check note of a quotation has at most this many characters (CHECK of migration 0046).
const MAX_QUOTE_CHECK_CHARS: usize = 200;
/// The name of the persona has at most this many characters (CHECK of migration 0046).
const MAX_PERSONA_CHARS: usize = 200;
/// Default and upper limit for [`FindingFilter::limit`].
const DEFAULT_LIMIT: u32 = 500;
const MAX_LIMIT: u32 = 2000;

/// Kind of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    Praise,
    Hint,
    Critical,
}

impl FindingKind {
    pub const ALL: [FindingKind; 3] = [FindingKind::Praise, FindingKind::Hint, FindingKind::Critical];

    /// Value in the database: `praise`, `hint`, `critical`.
    pub fn as_db(self) -> &'static str {
        match self {
            FindingKind::Praise => "praise",
            FindingKind::Hint => "hint",
            FindingKind::Critical => "critical",
        }
    }

    pub fn from_db(s: &str) -> Option<FindingKind> {
        FindingKind::ALL.into_iter().find(|k| k.as_db() == s)
    }

    /// Display: „Lob“, „Hinweis“, „kritisch“.
    pub fn label(self) -> &'static str {
        match self {
            FindingKind::Praise => "Lob",
            FindingKind::Hint => "Hinweis",
            FindingKind::Critical => "kritisch",
        }
    }
}

/// Status of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    Open,
    Adopted,
    Rejected,
    Deferred,
    Done,
}

impl FindingStatus {
    pub const ALL: [FindingStatus; 5] = [
        FindingStatus::Open,
        FindingStatus::Adopted,
        FindingStatus::Rejected,
        FindingStatus::Deferred,
        FindingStatus::Done,
    ];

    /// Value in the database: `open`, `adopted`, `rejected`, `deferred`, `done`.
    pub fn as_db(self) -> &'static str {
        match self {
            FindingStatus::Open => "open",
            FindingStatus::Adopted => "adopted",
            FindingStatus::Rejected => "rejected",
            FindingStatus::Deferred => "deferred",
            FindingStatus::Done => "done",
        }
    }

    pub fn from_db(s: &str) -> Option<FindingStatus> {
        FindingStatus::ALL.into_iter().find(|k| k.as_db() == s)
    }

    /// Display: „offen“, „übernommen“, „verworfen“, „verschoben“, „erledigt ohne Änderung“.
    pub fn label(self) -> &'static str {
        match self {
            FindingStatus::Open => "offen",
            FindingStatus::Adopted => "übernommen",
            FindingStatus::Rejected => "verworfen",
            FindingStatus::Deferred => "verschoben",
            FindingStatus::Done => "erledigt ohne Änderung",
        }
    }
}

/// Target of a deferred finding: a section (fixed key) or an idea.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeferTarget {
    Node { key: i64 },
    Idea { id: i64 },
}

/// Wanted status together with its mandatory details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusChange {
    Open,
    /// Revision number in the section of the finding.
    Adopted {
        rev: u32,
    },
    /// Reason mandatory, trimmed not empty, at most [`MAX_REASON_CHARS`].
    Rejected {
        reason: String,
    },
    /// The reason may be empty.
    Deferred {
        target: DeferTarget,
        reason: String,
    },
    /// The reason may be empty.
    Done {
        reason: String,
    },
}

/// An entry in the history of the status (the human sets, an assistant proposes).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FindingStatusEntry {
    pub id: i64,
    pub uuid: String,
    pub finding: i64,
    pub status: FindingStatus,
    pub reason: String,
    pub adopted_rev: Option<u32>,
    pub adopted_revision: Option<String>,
    pub deferred: Option<DeferTarget>,
    pub deferred_uuid: Option<String>,
    pub actor: IdeaOrigin,
    pub proposed: bool,
    pub note: String,
    pub decision: Option<String>,
    pub decided: Option<String>,
    pub decided_by: Option<String>,
    pub created: String,
    /// Round 22: state of the revision when adopting (`finding_status.adopted_amend`); `None` = not remembered.
    pub adopted_amend: Option<u32>,
}

/// A finding with resolved references.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Finding {
    pub id: i64,
    pub uuid: String,
    pub work: WorkId,
    /// `node.id`; `None` = thread without a section or section deleted.
    pub key: Option<i64>,
    /// Position (seq) now, from the join.
    pub node: Option<usize>,
    pub post: Option<i64>,
    pub thread: Option<i64>,
    pub thread_post: Option<i64>,
    pub position: u32,
    pub persona: String,
    pub kind: FindingKind,
    pub body: String,
    pub quote: String,
    pub quote_check: String,
    pub status: FindingStatus,
    pub reason: String,
    /// Resolved through `adopted_revision` in the section; `None` if the revision is missing.
    pub adopted_rev: Option<u32>,
    pub adopted_revision: Option<String>,
    /// Resolved; `None` if not deferred or the target is missing.
    pub deferred: Option<DeferTarget>,
    pub deferred_uuid: Option<String>,
    pub similar_to: Option<i64>,
    pub summary_of: Vec<String>,
    /// Most recent open proposal (`proposed = 1`, `decision IS NULL`).
    pub pending: Option<FindingStatusEntry>,
    pub created: String,
    pub updated: String,
    /// Round 30b: solution proposals for the finding (migration 0047, `finding.solutions`); empty = none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub solutions: Vec<Solution>,
    /// Round 22: state of the revision when adopting (`finding.adopted_amend`); `None` = not remembered.
    pub adopted_amend: Option<u32>,
    /// Round 23a (PM decision): summary finding (`summary_of` not empty) that is itself still open, but whose origins
    /// are all decided (no origin open any more; deleted origins count as decided) – "outdated". Derived when reading,
    /// never stored; the status stays as the human set it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stale_summary: bool,
}

/// A solution proposal for a finding (Round 30b): verbatim quotation and new wording (`statt`/`besser`) or a clear next
/// step.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Solution {
    pub quote: String,
    pub replacement: String,
    pub step: String,
}

/// A new finding ([`Store::finding_insert`]).
#[derive(Debug, Clone)]
pub struct NewFinding<'a> {
    pub work: WorkId,
    pub key: Option<i64>,
    pub post: Option<i64>,
    pub thread_post: Option<i64>,
    pub position: u32,
    pub persona: &'a str,
    pub kind: FindingKind,
    pub body: &'a str,
    pub quote: &'a str,
    pub quote_check: &'a str,
    pub similar_to: Option<i64>,
    pub summary_of: &'a [String],
}

/// Selection for [`Store::finding_list`].
#[derive(Debug, Clone, Default)]
pub struct FindingFilter {
    /// Sections (`node.id`); empty = all (also without a section).
    pub keys: Vec<i64>,
    pub post: Option<i64>,
    /// Empty = all.
    pub statuses: Vec<FindingStatus>,
    /// Empty = all; praise only with `include_praise` or explicitly in `kinds`.
    pub kinds: Vec<FindingKind>,
    pub persona: Option<String>,
    /// `created >= since` (ISO).
    pub since: Option<String>,
    /// `created < until` (ISO).
    pub until: Option<String>,
    pub include_praise: bool,
    /// Also attached findings (`similar_to IS NOT NULL`).
    pub include_attached: bool,
    /// `None` = 500, at most 2000; if more match, the newest.
    pub limit: Option<u32>,
}

// ---- Reading -------------------------------------------------------------------------------------

const FINDING_SELECT: &str = "SELECT f.id, f.uuid, f.work_id, f.node_id, n.seq, f.post_id, tp.thread_id, \
     f.thread_post_id, f.position, f.persona, f.kind, f.body, f.quote, f.quote_check, f.status, f.reason, \
     (SELECT r.number FROM revision r WHERE r.uuid = f.adopted_revision AND r.node_id = f.node_id), \
     f.adopted_revision, \
     (SELECT dn.id FROM node dn WHERE dn.uuid = f.deferred_node AND dn.work_id = f.work_id), \
     f.deferred_node, \
     (SELECT i.id FROM idea i WHERE i.uuid = f.deferred_idea), \
     f.deferred_idea, f.similar_to, f.summary_of, f.created, f.updated, f.solutions, f.adopted_amend \
     FROM finding f LEFT JOIN node n ON n.id = f.node_id LEFT JOIN chat_thread_post tp ON tp.id = f.thread_post_id";

const ENTRY_SELECT: &str = "SELECT s.id, s.uuid, s.finding_id, s.status, s.reason, \
     (SELECT r.number FROM revision r WHERE r.uuid = s.adopted_revision AND r.node_id = f.node_id), \
     s.adopted_revision, \
     (SELECT dn.id FROM node dn WHERE dn.uuid = s.deferred_node AND dn.work_id = f.work_id), \
     s.deferred_node, \
     (SELECT i.id FROM idea i WHERE i.uuid = s.deferred_idea), \
     s.deferred_idea, s.actor_kind, s.actor, s.proposed, s.note, s.decision, s.decided, s.decided_by, s.created, s.adopted_amend \
     FROM finding_status s JOIN finding f ON f.id = s.finding_id";

fn deferred(
    node: Option<i64>,
    node_uuid: Option<String>,
    idea: Option<i64>,
    idea_uuid: Option<String>,
) -> (Option<DeferTarget>, Option<String>) {
    let target = match (node, idea) {
        (Some(key), _) => Some(DeferTarget::Node { key }),
        (None, Some(id)) => Some(DeferTarget::Idea { id }),
        (None, None) => None,
    };
    (target, node_uuid.or(idea_uuid))
}

fn actor(kind: &str, name: String) -> Result<IdeaOrigin, StoreError> {
    match kind {
        "human" => Ok(IdeaOrigin::Human { name }),
        "persona" => Ok(IdeaOrigin::Persona { handle: name }),
        "mcp" => Ok(IdeaOrigin::Mcp { handle: name }),
        other => Err(crate::broken(format!("unbekannte Herkunft im Befundverlauf {other:?}"))),
    }
}

fn entry_of(row: &sqlx::sqlite::SqliteRow) -> Result<FindingStatusEntry, StoreError> {
    let id: i64 = row.try_get(0)?;
    let status: String = row.try_get(3)?;
    let status = FindingStatus::from_db(&status)
        .ok_or_else(|| crate::broken(format!("Befundverlauf {id}: Status {status:?}")))?;
    let adopted_rev: Option<i64> = row.try_get(5)?;
    let (deferred, deferred_uuid) = deferred(row.try_get(7)?, row.try_get(8)?, row.try_get(9)?, row.try_get(10)?);
    let kind: String = row.try_get(11)?;
    let proposed: i64 = row.try_get(13)?;
    Ok(FindingStatusEntry {
        id,
        uuid: row.try_get::<Option<String>, _>(1)?.unwrap_or_default(),
        finding: row.try_get(2)?,
        status,
        reason: row.try_get(4)?,
        adopted_rev: adopted_rev.map(|r| to_u32(r, "Fassung")).transpose()?,
        adopted_revision: row.try_get(6)?,
        deferred,
        deferred_uuid,
        actor: actor(&kind, row.try_get(12)?)?,
        proposed: proposed == 1,
        note: row.try_get(14)?,
        decision: row.try_get(15)?,
        decided: row.try_get(16)?,
        decided_by: row.try_get(17)?,
        created: row.try_get(18)?,
        adopted_amend: row.try_get::<Option<i64>, _>(19)?.map(|k| to_u32(k, "Stand")).transpose()?,
    })
}

/// Round 23a: open proposals and "outdated" for a whole list in **one** query per 500 findings (before, one per
/// finding: up to 2000 queries for one work page). Per finding the most recent open proposal (`proposed = 1`, `decision
/// IS NULL`, largest id); summary findings get [`Finding::stale_summary`].
async fn complete(conn: &mut SqliteConnection, work: WorkId, findings: &mut [Finding]) -> Result<(), StoreError> {
    let ids: Vec<i64> = findings.iter().map(|f| f.id).collect();
    let mut latest: HashMap<i64, FindingStatusEntry> = HashMap::new();
    for chunk in ids.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "{ENTRY_SELECT} WHERE s.id IN (SELECT max(p.id) FROM finding_status p WHERE p.finding_id IN ({marks}) \
             AND p.proposed = 1 AND p.decision IS NULL GROUP BY p.finding_id)"
        );
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
        for id in chunk {
            query = query.bind(*id);
        }
        for row in query.fetch_all(&mut *conn).await? {
            let entry = entry_of(&row)?;
            latest.insert(entry.finding, entry);
        }
    }
    // Origins of open summary findings: which of them are still open?
    let origins: Vec<&str> = findings
        .iter()
        .filter(|f| f.status == FindingStatus::Open)
        .flat_map(|f| f.summary_of.iter().map(String::as_str))
        .collect();
    let mut open: std::collections::HashSet<String> = std::collections::HashSet::new();
    for chunk in origins.chunks(500) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let sql = format!("SELECT uuid FROM finding WHERE work_id = ? AND status = 'open' AND uuid IN ({marks})");
        let mut query = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql.as_str())).bind(work);
        for uuid in chunk {
            query = query.bind(*uuid);
        }
        open.extend(query.fetch_all(&mut *conn).await?);
    }
    for finding in findings {
        finding.pending = latest.remove(&finding.id);
        finding.stale_summary = finding.status == FindingStatus::Open
            && !finding.summary_of.is_empty()
            && !finding.summary_of.iter().any(|uuid| open.contains(uuid));
    }
    Ok(())
}

/// Rows of [`FINDING_SELECT`] as findings, together with open proposals and "outdated" ([`complete`]).
async fn findings_of<'r>(
    conn: &mut SqliteConnection,
    work: WorkId,
    rows: impl Iterator<Item = &'r sqlx::sqlite::SqliteRow>,
) -> Result<Vec<Finding>, StoreError> {
    let mut out = rows.map(finding_row).collect::<Result<Vec<_>, _>>()?;
    complete(conn, work, &mut out).await?;
    Ok(out)
}

async fn finding_of(conn: &mut SqliteConnection, row: &sqlx::sqlite::SqliteRow) -> Result<Finding, StoreError> {
    let mut one = [finding_row(row)?];
    let work = one[0].work;
    complete(conn, work, &mut one).await?;
    let [finding] = one;
    Ok(finding)
}

/// A row of [`FINDING_SELECT`] without a further query (`pending` and `stale_summary` are set by [`complete`]).
fn finding_row(row: &sqlx::sqlite::SqliteRow) -> Result<Finding, StoreError> {
    let id: i64 = row.try_get(0)?;
    let seq: Option<i64> = row.try_get(4)?;
    let position: i64 = row.try_get(8)?;
    let kind: String = row.try_get(10)?;
    let kind = FindingKind::from_db(&kind).ok_or_else(|| crate::broken(format!("Befund {id}: Art {kind:?}")))?;
    let status: String = row.try_get(14)?;
    let status =
        FindingStatus::from_db(&status).ok_or_else(|| crate::broken(format!("Befund {id}: Status {status:?}")))?;
    let adopted_rev: Option<i64> = row.try_get(16)?;
    let (deferred, deferred_uuid) = deferred(row.try_get(18)?, row.try_get(19)?, row.try_get(20)?, row.try_get(21)?);
    let summary_of: String = row.try_get(23)?;
    let summary_of: Vec<String> = serde_json::from_str(&summary_of)
        .map_err(|e| crate::broken(format!("Befund {id}: summary_of ist keine Liste von Texten ({e})")))?;
    let solutions: String = row.try_get(26)?;
    let solutions: Vec<Solution> = serde_json::from_str(&solutions)
        .map_err(|e| crate::broken(format!("Befund {id}: solutions ist keine Liste von Lösungen ({e})")))?;
    Ok(Finding {
        id,
        uuid: row.try_get::<Option<String>, _>(1)?.unwrap_or_default(),
        work: row.try_get(2)?,
        key: row.try_get(3)?,
        node: seq.map(|s| to_usize(s, "seq")).transpose()?,
        post: row.try_get(5)?,
        thread: row.try_get(6)?,
        thread_post: row.try_get(7)?,
        position: to_u32(position, "position")?,
        persona: row.try_get(9)?,
        kind,
        body: row.try_get(11)?,
        quote: row.try_get(12)?,
        quote_check: row.try_get(13)?,
        status,
        reason: row.try_get(15)?,
        adopted_rev: adopted_rev.map(|r| to_u32(r, "Fassung")).transpose()?,
        adopted_revision: row.try_get(17)?,
        deferred,
        deferred_uuid,
        similar_to: row.try_get(22)?,
        summary_of,
        pending: None,
        created: row.try_get(24)?,
        updated: row.try_get(25)?,
        solutions,
        adopted_amend: row.try_get::<Option<i64>, _>(27)?.map(|k| to_u32(k, "Stand")).transpose()?,
        stale_summary: false,
    })
}

async fn check_work(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Finding `id` in the work `work` (in the connection or transaction of the caller); foreign work or unknown →
/// [`StoreError::UnknownFinding`], unknown work → [`StoreError::UnknownWork`].
pub(crate) async fn get(conn: &mut SqliteConnection, work: WorkId, id: i64) -> Result<Finding, StoreError> {
    check_work(conn, work).await?;
    let row = sqlx::query(sqlx::AssertSqlSafe(format!("{FINDING_SELECT} WHERE f.id = ? AND f.work_id = ?")))
        .bind(id)
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    match row {
        Some(row) => finding_of(conn, &row).await,
        None => Err(StoreError::UnknownFinding { work, id }),
    }
}

/// Conditions of `filter` (without `limit`) as " AND …" behind `WHERE f.work_id = ?`; bind the placeholders first from
/// `numbers`, then from `texts`.
fn filter_clause(filter: &FindingFilter) -> (String, Vec<i64>, Vec<String>) {
    let mut sql = String::new();
    let mut texts: Vec<String> = Vec::new();
    let mut numbers: Vec<i64> = Vec::new();
    let marks = |n: usize| vec!["?"; n].join(", ");
    // Bind numbers first, then texts: the order of the placeholders follows this split.
    if !filter.keys.is_empty() {
        sql.push_str(&format!(" AND f.node_id IN ({})", marks(filter.keys.len())));
        numbers.extend(&filter.keys);
    }
    if let Some(post) = filter.post {
        sql.push_str(" AND f.post_id = ?");
        numbers.push(post);
    }
    if !filter.statuses.is_empty() {
        sql.push_str(&format!(" AND f.status IN ({})", marks(filter.statuses.len())));
        texts.extend(filter.statuses.iter().map(|s| s.as_db().to_owned()));
    }
    if !filter.kinds.is_empty() {
        sql.push_str(&format!(" AND f.kind IN ({})", marks(filter.kinds.len())));
        texts.extend(filter.kinds.iter().map(|k| k.as_db().to_owned()));
    }
    if !filter.include_praise && !filter.kinds.contains(&FindingKind::Praise) {
        sql.push_str(" AND f.kind <> 'praise'");
    }
    if !filter.include_attached {
        sql.push_str(" AND f.similar_to IS NULL");
    }
    if let Some(persona) = &filter.persona {
        sql.push_str(" AND f.persona = ?");
        texts.push(persona.clone());
    }
    if let Some(since) = &filter.since {
        sql.push_str(" AND f.created >= ?");
        texts.push(since.clone());
    }
    if let Some(until) = &filter.until {
        sql.push_str(" AND f.created < ?");
        texts.push(until.clone());
    }
    (sql, numbers, texts)
}

fn too_long(what: &str, max: usize) -> StoreError {
    StoreError::Finding(format!("{what} ist zu lang (höchstens {max} Zeichen)."))
}

/// Does the row `id` of the table belong to the work, through `sql` (a `SELECT 1 … WHERE <id> AND <work>`)?
async fn belongs(conn: &mut SqliteConnection, sql: &'static str, id: i64, work: WorkId) -> Result<bool, StoreError> {
    let found: Option<i64> = sqlx::query_scalar(sql).bind(id).bind(work).fetch_optional(&mut *conn).await?;
    Ok(found.is_some())
}

impl Store {
    /// Create a new finding (one transaction, no status history). Limits (text trimmed not empty and at most
    /// [`MAX_FINDING_CHARS`], quotation at most [`MAX_QUOTE_CHARS`], check note at most 200, persona 1 to 200
    /// characters; not post and thread post at once; section, post, thread post and `similar_to` from this work) →
    /// [`StoreError::Finding`]; the same post with the same position twice → database error (unique index). Returns the
    /// finding created.
    pub async fn finding_insert(&self, new: &NewFinding<'_>) -> Result<Finding, StoreError> {
        if new.body.trim().is_empty() {
            return Err(StoreError::Finding("Ein Befund braucht einen Text.".into()));
        }
        if new.body.chars().count() > MAX_FINDING_CHARS {
            return Err(too_long("Der Text des Befunds", MAX_FINDING_CHARS));
        }
        if new.quote.chars().count() > MAX_QUOTE_CHARS {
            return Err(too_long("Das Zitat", MAX_QUOTE_CHARS));
        }
        if new.quote_check.chars().count() > MAX_QUOTE_CHECK_CHARS {
            return Err(too_long("Der Prüfvermerk", MAX_QUOTE_CHECK_CHARS));
        }
        let persona = new.persona.chars().count();
        if persona == 0 || persona > MAX_PERSONA_CHARS {
            return Err(StoreError::Finding(format!(
                "Die Persona eines Befunds braucht 1 bis {MAX_PERSONA_CHARS} Zeichen."
            )));
        }
        if new.post.is_some() && new.thread_post.is_some() {
            return Err(StoreError::Finding(
                "Ein Befund stammt aus einem Beitrag oder einem Fadenbeitrag, nicht aus beiden.".into(),
            ));
        }
        let summary = serde_json::to_string(new.summary_of).map_err(|e| crate::broken(format!("summary_of: {e}")))?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        check_work(&mut tx, new.work).await?;
        if let Some(key) = new.key
            && !belongs(&mut tx, "SELECT 1 FROM node WHERE id = ? AND work_id = ?", key, new.work).await?
        {
            return Err(StoreError::Finding(format!("Abschnitt {key} gibt es in Werk {} nicht.", new.work)));
        }
        if let Some(post) = new.post
            && !belongs(
                &mut tx,
                "SELECT 1 FROM post p JOIN node n ON n.id = p.node_id WHERE p.id = ? AND n.work_id = ?",
                post,
                new.work,
            )
            .await?
        {
            return Err(StoreError::UnknownPost { work: new.work, post });
        }
        if let Some(post) = new.thread_post
            && !belongs(
                &mut tx,
                "SELECT 1 FROM chat_thread_post tp JOIN chat_thread t ON t.id = tp.thread_id \
                 WHERE tp.id = ? AND t.work_id = ?",
                post,
                new.work,
            )
            .await?
        {
            return Err(StoreError::Finding(format!("Fadenbeitrag {post} gibt es in Werk {} nicht.", new.work)));
        }
        if let Some(similar) = new.similar_to
            && !belongs(&mut tx, "SELECT 1 FROM finding WHERE id = ? AND work_id = ?", similar, new.work).await?
        {
            return Err(StoreError::UnknownFinding { work: new.work, id: similar });
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO finding (work_id, node_id, post_id, thread_post_id, position, persona, kind, body, quote, \
             quote_check, similar_to, summary_of) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(new.work)
        .bind(new.key)
        .bind(new.post)
        .bind(new.thread_post)
        .bind(i64::from(new.position))
        .bind(new.persona)
        .bind(new.kind.as_db())
        .bind(new.body)
        .bind(new.quote)
        .bind(new.quote_check)
        .bind(new.similar_to)
        .bind(&summary)
        .fetch_one(&mut *tx)
        .await?;
        let finding = get(&mut tx, new.work, id).await?;
        tx.commit().await?;
        Ok(finding)
    }

    /// Replace the solution proposals of the finding `id` (Round 30b, atomic; empty = none). Every solution needs a
    /// quotation with a new wording or a step; quotation at most [`MAX_QUOTE_CHARS`], wording and step at most
    /// [`MAX_FINDING_CHARS`] characters, otherwise [`StoreError::Finding`]. Foreign work or unknown →
    /// [`StoreError::UnknownFinding`]. Status and `updated` stay untouched.
    pub async fn finding_set_solutions(&self, work: WorkId, id: i64, solutions: &[Solution]) -> Result<(), StoreError> {
        for s in solutions {
            if s.quote.chars().count() > MAX_QUOTE_CHARS {
                return Err(too_long("Das Zitat einer Lösung", MAX_QUOTE_CHARS));
            }
            if s.replacement.chars().count() > MAX_FINDING_CHARS || s.step.chars().count() > MAX_FINDING_CHARS {
                return Err(too_long("Eine Lösung", MAX_FINDING_CHARS));
            }
            let replace = !s.quote.trim().is_empty() && !s.replacement.trim().is_empty();
            if !replace && s.step.trim().is_empty() {
                return Err(StoreError::Finding(
                    "Eine Lösung braucht ein Zitat mit neuer Formulierung oder einen Schritt.".into(),
                ));
            }
        }
        let json = serde_json::to_string(solutions).expect("Lösungen sind immer JSON");
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        get(&mut tx, work, id).await?;
        sqlx::query("UPDATE finding SET solutions = ? WHERE id = ? AND work_id = ?")
            .bind(json)
            .bind(id)
            .bind(work)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Finding `id` of the work; foreign work or unknown → [`StoreError::UnknownFinding`].
    pub async fn finding_get(&self, work: WorkId, id: i64) -> Result<Finding, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, work, id).await
    }

    /// Findings of the work after `filter`, oldest first (`created`, then `id`). Without `include_praise` no praise
    /// (unless `kinds` names praise explicitly), without `include_attached` no attached findings. If more than `limit`
    /// match, the **newest** `limit` come (older ones are dropped, not the most recent); whether it was capped is told
    /// by the comparison with [`Store::finding_count`].
    pub async fn finding_list(&self, work: WorkId, filter: &FindingFilter) -> Result<Vec<Finding>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let (clause, numbers, texts) = filter_clause(filter);
        let limit = filter.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
        let sql =
            format!("{FINDING_SELECT} WHERE f.work_id = ?{clause} ORDER BY f.created DESC, f.id DESC LIMIT {limit}");
        let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(work);
        for n in &numbers {
            query = query.bind(*n);
        }
        for t in &texts {
            query = query.bind(t.as_str());
        }
        let rows = query.fetch_all(&mut *conn).await?;
        findings_of(&mut conn, work, rows.iter().rev()).await
    }

    /// Number of findings after `filter` without a limit (`limit` does not count): more than [`Store::finding_list`]
    /// returns means capped.
    pub async fn finding_count(&self, work: WorkId, filter: &FindingFilter) -> Result<usize, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let (clause, numbers, texts) = filter_clause(filter);
        let sql = format!("SELECT count(*) FROM finding f WHERE f.work_id = ?{clause}");
        let mut query = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql.as_str())).bind(work);
        for n in &numbers {
            query = query.bind(*n);
        }
        for t in &texts {
            query = query.bind(t.as_str());
        }
        to_usize(query.fetch_one(&mut *conn).await?, "Zahl")
    }

    /// Findings that are attached to `id` (`similar_to = id`), oldest first, without a limit.
    pub async fn finding_attached_to(&self, work: WorkId, id: i64) -> Result<Vec<Finding>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, work, id).await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{FINDING_SELECT} WHERE f.work_id = ? AND f.similar_to = ? ORDER BY f.created, f.id"
        )))
        .bind(work)
        .bind(id)
        .fetch_all(&mut *conn)
        .await?;
        findings_of(&mut conn, work, rows.iter()).await
    }

    /// Findings of the work with these UUIDs (such as `summary_of`), oldest first; unknown ones are missing.
    pub async fn findings_by_uuid(&self, work: WorkId, uuids: &[String]) -> Result<Vec<Finding>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let mut out = Vec::new();
        for chunk in uuids.chunks(500) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!("{FINDING_SELECT} WHERE f.work_id = ? AND f.uuid IN ({marks}) ORDER BY f.created, f.id");
            let mut query = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(work);
            for u in chunk {
                query = query.bind(u.as_str());
            }
            let rows = query.fetch_all(&mut *conn).await?;
            out.extend(findings_of(&mut conn, work, rows.iter()).await?);
        }
        out.sort_by(|a, b| (a.created.as_str(), a.id).cmp(&(b.created.as_str(), b.id)));
        Ok(out)
    }

    /// History of the finding `id` (status changes and proposals), oldest first.
    pub async fn finding_history(&self, work: WorkId, id: i64) -> Result<Vec<FindingStatusEntry>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, work, id).await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!("{ENTRY_SELECT} WHERE s.finding_id = ? ORDER BY s.id")))
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
        rows.iter().map(entry_of).collect()
    }

    /// Open findings per section (`node.id` → number) after contract §1.5: open, not attached, not praise, with a
    /// section; only the section itself (no sums across sub-items). Sections without open ones are missing. Round 23a
    /// (PM decision): summary findings (`summary_of` not empty) do not count – their origins already count; otherwise
    /// "k open" would rise twice after an RFC.
    pub async fn finding_open_counts(&self, work: WorkId) -> Result<HashMap<i64, usize>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT node_id, count(*) FROM finding WHERE work_id = ? AND status = 'open' AND similar_to IS NULL \
             AND kind <> 'praise' AND node_id IS NOT NULL AND json_array_length(summary_of) = 0 GROUP BY node_id",
        )
        .bind(work)
        .fetch_all(&mut *conn)
        .await?;
        rows.into_iter().map(|(key, n)| Ok((key, to_usize(n, "Zahl")?))).collect()
    }

    /// Per revision number of the section `key` the adopted findings (ids ascending), revisions ascending; findings
    /// whose revision is missing are missing.
    pub async fn findings_adopted_in(&self, work: WorkId, key: i64) -> Result<Vec<(u32, Vec<i64>)>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT r.number, f.id FROM finding f JOIN revision r ON r.uuid = f.adopted_revision AND r.node_id = f.node_id \
             WHERE f.work_id = ? AND f.node_id = ? AND f.status = 'adopted' ORDER BY r.number, f.id",
        )
        .bind(work)
        .bind(key)
        .fetch_all(&mut *conn)
        .await?;
        let mut out: Vec<(u32, Vec<i64>)> = Vec::new();
        for (rev, id) in rows {
            let rev = to_u32(rev, "Fassung")?;
            match out.last_mut() {
                Some((last, ids)) if *last == rev => ids.push(id),
                _ => out.push((rev, vec![id])),
            }
        }
        Ok(out)
    }

    /// Set the status (only the human): writes `finding` and an entry in `finding_status` (`actor_kind = 'human'`,
    /// `proposed = 0`) in one transaction and returns (before, after). Checks beforehand (contract R28 §1.2, RPC
    /// `finding.set_status`): adopted only with a revision that exists in the section of the finding; rejected only
    /// with a reason (trimmed not empty, at most [`MAX_REASON_CHARS`]); deferred only with a section or idea of the
    /// same work (not its own section). Violations and a status that the finding already has exactly like that →
    /// [`StoreError::Finding`]; then nothing changes.
    pub async fn finding_set_status(
        &self,
        work: WorkId,
        id: i64,
        change: &StatusChange,
        by: &str,
    ) -> Result<(Finding, Finding), StoreError> {
        let by = by.trim();
        if by.is_empty() {
            return Err(StoreError::Finding("Wer den Status setzt, braucht einen Namen.".into()));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, work, id).await?;
        let target = resolve(&mut tx, &before, change).await?;
        if target.same_as(&before) {
            return Err(StoreError::Finding(format!(
                "Befund {id} hat diesen Status schon ({}); nichts geändert.",
                before.status.label()
            )));
        }
        write_status(&mut tx, id, &target).await?;
        sqlx::query(
            "INSERT INTO finding_status (finding_id, status, reason, adopted_revision, deferred_node, deferred_idea, \
             actor_kind, actor, proposed, adopted_amend) VALUES (?, ?, ?, ?, ?, ?, 'human', ?, 0, ?)",
        )
        .bind(id)
        .bind(target.status.as_db())
        .bind(&target.reason)
        .bind(&target.adopted_revision)
        .bind(&target.deferred_node)
        .bind(&target.deferred_idea)
        .bind(by)
        .bind(target.adopted_amend)
        .execute(&mut *tx)
        .await?;
        let after = get(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok((before, after))
    }

    /// Status proposal of an assistant (persona or MCP client; never the human): only adopted, deferred, done without
    /// change – the store rejects rejected and open („Nur der Mensch kann …“). It is checked as in
    /// [`Store::finding_set_status`]; an identical open proposal (same status, same revision or same target) or a
    /// status that the finding already has is not created ([`StoreError::Finding`]). The finding itself does not
    /// change.
    pub async fn finding_propose_status(
        &self,
        work: WorkId,
        id: i64,
        change: &StatusChange,
        by: &IdeaOrigin,
        note: &str,
    ) -> Result<FindingStatusEntry, StoreError> {
        let (kind, actor) = match by {
            IdeaOrigin::Human { .. } => {
                return Err(StoreError::Finding(
                    "Der Mensch setzt den Status direkt; Vorschläge stellen nur Assistenten.".into(),
                ));
            }
            IdeaOrigin::Persona { handle } => ("persona", handle.trim()),
            IdeaOrigin::Mcp { handle } => ("mcp", handle.trim()),
        };
        if actor.is_empty() {
            return Err(StoreError::Finding("Ein Vorschlag braucht eine Herkunft (Persona bzw. MCP-Client).".into()));
        }
        if matches!(change, StatusChange::Open | StatusChange::Rejected { .. }) {
            return Err(StoreError::Finding(
                "Nur der Mensch kann einen Befund verwerfen oder wieder öffnen; Assistenten schlagen nur „übernommen“, \
                 „verschoben“ oder „erledigt ohne Änderung“ vor."
                    .into(),
            ));
        }
        if note.chars().count() > MAX_REASON_CHARS {
            return Err(too_long("Die Notiz zum Vorschlag", MAX_REASON_CHARS));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let finding = get(&mut tx, work, id).await?;
        let target = resolve(&mut tx, &finding, change).await?;
        if target.same_as(&finding) {
            return Err(StoreError::Finding(format!(
                "Befund {id} hat diesen Status schon ({}); kein Vorschlag nötig.",
                finding.status.label()
            )));
        }
        let twin: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM finding_status WHERE finding_id = ? AND proposed = 1 AND decision IS NULL AND status = ? \
             AND adopted_revision IS ? AND deferred_node IS ? AND deferred_idea IS ? ORDER BY id LIMIT 1",
        )
        .bind(id)
        .bind(target.status.as_db())
        .bind(&target.adopted_revision)
        .bind(&target.deferred_node)
        .bind(&target.deferred_idea)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(twin) = twin {
            return Err(StoreError::Finding(format!(
                "Diesen Vorschlag gibt es zu Befund {id} schon offen (Eintrag {twin}); nicht doppelt angelegt."
            )));
        }
        let entry: i64 = sqlx::query_scalar(
            "INSERT INTO finding_status (finding_id, status, reason, adopted_revision, deferred_node, deferred_idea, \
             actor_kind, actor, proposed, note, adopted_amend) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?) RETURNING id",
        )
        .bind(id)
        .bind(target.status.as_db())
        .bind(&target.reason)
        .bind(&target.adopted_revision)
        .bind(&target.deferred_node)
        .bind(&target.deferred_idea)
        .bind(kind)
        .bind(actor)
        .bind(note.trim())
        .bind(target.adopted_amend)
        .fetch_one(&mut *tx)
        .await?;
        let entry = entry_by_id(&mut tx, work, entry).await?;
        tx.commit().await?;
        Ok(entry)
    }

    /// Decide on a proposal (only the human). It is stale if the finding has since got a status from the human or from
    /// a confirmed proposal, the proposed revision or the target is missing or the finding already has the status: then
    /// `decision = 'stale'`, and the finding stays. Otherwise confirmed (`confirmed`, the finding gets the proposed
    /// status) or rejected (`rejected`). Unknown or already decided proposal → [`StoreError::Finding`]. Returns
    /// (finding afterwards, entry afterwards).
    pub async fn finding_decide(
        &self,
        work: WorkId,
        entry: i64,
        confirm: bool,
        by: &str,
    ) -> Result<(Finding, FindingStatusEntry), StoreError> {
        let by = by.trim();
        if by.is_empty() {
            return Err(StoreError::Finding("Wer entscheidet, braucht einen Namen.".into()));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = entry_by_id(&mut tx, work, entry).await?;
        if !current.proposed {
            return Err(StoreError::Finding(format!(
                "Eintrag {entry} ist kein Vorschlag, sondern eine Statusänderung des Menschen."
            )));
        }
        if let Some(decision) = &current.decision {
            return Err(StoreError::Finding(format!("Über Vorschlag {entry} ist schon entschieden ({decision}).")));
        }
        let finding = get(&mut tx, work, current.finding).await?;
        let (status, reason, adopted_revision, deferred_node, deferred_idea): Row5 = sqlx::query_as(
            "SELECT status, reason, adopted_revision, deferred_node, deferred_idea FROM finding_status WHERE id = ?",
        )
        .bind(entry)
        .fetch_one(&mut *tx)
        .await?;
        let mut target = Target {
            status: FindingStatus::from_db(&status)
                .ok_or_else(|| crate::broken(format!("Befundverlauf {entry}: Status {status:?}")))?,
            reason,
            adopted_revision,
            deferred_node,
            deferred_idea,
            adopted_amend: None,
        };
        let newer: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM finding_status WHERE finding_id = ? AND id > ? \
             AND (proposed = 0 OR decision = 'confirmed')",
        )
        .bind(current.finding)
        .bind(entry)
        .fetch_one(&mut *tx)
        .await?;
        let gone = match target.status {
            FindingStatus::Adopted => current.adopted_rev.is_none(),
            FindingStatus::Deferred => current.deferred.is_none(),
            _ => false,
        };
        let stale = newer > 0 || gone || target.same_as(&finding);
        let decision = match (stale, confirm) {
            (true, _) => "stale",
            (false, true) => "confirmed",
            (false, false) => "rejected",
        };
        if decision == "confirmed" {
            // Round 22: state of the revision at the confirmation (entry and finding).
            if target.status == FindingStatus::Adopted {
                target.adopted_amend =
                    sqlx::query_scalar("SELECT amend_seq FROM revision WHERE uuid = ? AND node_id = ?")
                        .bind(&target.adopted_revision)
                        .bind(finding.key)
                        .fetch_optional(&mut *tx)
                        .await?;
                sqlx::query("UPDATE finding_status SET adopted_amend = ? WHERE id = ?")
                    .bind(target.adopted_amend)
                    .bind(entry)
                    .execute(&mut *tx)
                    .await?;
            }
            write_status(&mut tx, current.finding, &target).await?;
        }
        sqlx::query(
            "UPDATE finding_status SET decision = ?, decided = strftime('%Y-%m-%dT%H:%M:%fZ','now'), decided_by = ? \
             WHERE id = ?",
        )
        .bind(decision)
        .bind(by)
        .bind(entry)
        .execute(&mut *tx)
        .await?;
        let after = get(&mut tx, work, current.finding).await?;
        let entry = entry_by_id(&mut tx, work, entry).await?;
        tx.commit().await?;
        Ok((after, entry))
    }
}

type Row5 = (String, String, Option<String>, Option<String>, Option<String>);

/// Wanted state of a finding in columns (revision and target as uuid).
struct Target {
    status: FindingStatus,
    reason: String,
    adopted_revision: Option<String>,
    deferred_node: Option<String>,
    deferred_idea: Option<String>,
    /// Round 22: state of the adopted revision (`revision.amend_seq`) when resolving; otherwise `None`.
    adopted_amend: Option<i64>,
}

impl Target {
    /// Does the finding already have exactly this state?
    fn same_as(&self, finding: &Finding) -> bool {
        self.status == finding.status
            && self.reason == finding.reason
            && self.adopted_revision == finding.adopted_revision
            && self.deferred_node.clone().or_else(|| self.deferred_idea.clone()) == finding.deferred_uuid
    }
}

/// Check the reason: at most [`MAX_REASON_CHARS`], mandatory for rejected.
fn checked_reason(reason: &str, required: bool) -> Result<String, StoreError> {
    let reason = reason.trim();
    if required && reason.is_empty() {
        return Err(StoreError::Finding("Verwerfen braucht eine Begründung.".into()));
    }
    if reason.chars().count() > MAX_REASON_CHARS {
        return Err(too_long("Die Begründung", MAX_REASON_CHARS));
    }
    Ok(reason.to_owned())
}

/// Check the change and translate it into columns (revision through its number in the section of the finding, target
/// through section or idea of the same work).
async fn resolve(conn: &mut SqliteConnection, finding: &Finding, change: &StatusChange) -> Result<Target, StoreError> {
    let empty = |status, reason| Target {
        status,
        reason,
        adopted_revision: None,
        deferred_node: None,
        deferred_idea: None,
        adopted_amend: None,
    };
    Ok(match change {
        StatusChange::Open => empty(FindingStatus::Open, String::new()),
        StatusChange::Rejected { reason } => empty(FindingStatus::Rejected, checked_reason(reason, true)?),
        StatusChange::Done { reason } => empty(FindingStatus::Done, checked_reason(reason, false)?),
        StatusChange::Adopted { rev } => {
            let Some(key) = finding.key else {
                return Err(StoreError::Finding(format!(
                    "Befund {} gehört zu keinem Abschnitt; „übernommen“ braucht eine Fassung des Abschnitts.",
                    finding.id
                )));
            };
            let row: Option<(Option<String>, i64)> =
                sqlx::query_as("SELECT uuid, amend_seq FROM revision WHERE node_id = ? AND number = ?")
                    .bind(key)
                    .bind(i64::from(*rev))
                    .fetch_optional(&mut *conn)
                    .await?;
            let Some((Some(uuid), amend)) = row else {
                return Err(StoreError::Finding(format!(
                    "Fassung {rev} gibt es im Abschnitt des Befunds {} nicht.",
                    finding.id
                )));
            };
            Target {
                adopted_revision: Some(uuid),
                adopted_amend: Some(amend),
                ..empty(FindingStatus::Adopted, String::new())
            }
        }
        StatusChange::Deferred { target, reason } => {
            let reason = checked_reason(reason, false)?;
            match target {
                DeferTarget::Node { key } => {
                    if finding.key == Some(*key) {
                        return Err(StoreError::Finding(
                            "Ein Befund lässt sich nicht in seinen eigenen Abschnitt verschieben.".into(),
                        ));
                    }
                    let uuid: Option<Option<String>> =
                        sqlx::query_scalar("SELECT uuid FROM node WHERE id = ? AND work_id = ?")
                            .bind(key)
                            .bind(finding.work)
                            .fetch_optional(&mut *conn)
                            .await?;
                    let Some(Some(uuid)) = uuid else {
                        return Err(StoreError::Finding(format!(
                            "Abschnitt {key} gibt es in Werk {} nicht (Ziel des Verschiebens).",
                            finding.work
                        )));
                    };
                    Target { deferred_node: Some(uuid), ..empty(FindingStatus::Deferred, reason) }
                }
                DeferTarget::Idea { id } => {
                    let uuid: Option<Option<String>> =
                        sqlx::query_scalar("SELECT uuid FROM idea WHERE id = ? AND work_id = ? AND deleted IS NULL")
                            .bind(id)
                            .bind(finding.work)
                            .fetch_optional(&mut *conn)
                            .await?;
                    let Some(Some(uuid)) = uuid else {
                        return Err(StoreError::Finding(format!(
                            "Einfall {id} gibt es in Werk {} nicht (Ziel des Verschiebens).",
                            finding.work
                        )));
                    };
                    Target { deferred_idea: Some(uuid), ..empty(FindingStatus::Deferred, reason) }
                }
            }
        }
    })
}

async fn write_status(conn: &mut SqliteConnection, id: i64, target: &Target) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE finding SET status = ?, reason = ?, adopted_revision = ?, deferred_node = ?, deferred_idea = ?, \
         adopted_amend = ?, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(target.status.as_db())
    .bind(&target.reason)
    .bind(&target.adopted_revision)
    .bind(&target.deferred_node)
    .bind(&target.deferred_idea)
    .bind(target.adopted_amend)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Entry `entry` in the history of a finding of the work; otherwise [`StoreError::Finding`].
async fn entry_by_id(conn: &mut SqliteConnection, work: WorkId, entry: i64) -> Result<FindingStatusEntry, StoreError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!("{ENTRY_SELECT} WHERE s.id = ? AND f.work_id = ?")))
        .bind(entry)
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    match row {
        Some(row) => entry_of(&row),
        None => Err(StoreError::Finding(format!("Vorschlag {entry} gibt es in Werk {work} nicht."))),
    }
}

/// „Fassung n“ or, with an amendment, „Fassung n · Stand k“ (Round 22; state 0 or unknown shows only the revision).
pub fn adopted_words(rev: u32, amend: Option<u32>) -> String {
    match amend.filter(|k| *k > 0) {
        Some(k) => format!("Fassung {rev} · Stand {k}"),
        None => format!("Fassung {rev}"),
    }
}

/// Path of the mirror file of the findings.
pub const MIRROR_FILE: &str = "befunde.md";

/// Group in `befunde.md`: section (tree order), thread (by id), without a reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Section(usize),
    Thread(i64),
    Loose,
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Files of the findings in the work mirror: `befunde.md`, only if the work has findings. Deterministic: sections in
/// tree order (part, chapter, section), then threads by id, then findings without a reference; within them by creation.
/// Per finding kind, status, persona, text, quotation with check note, reason, revision or target, attachment, summary,
/// open proposal, origin and the history.
pub(crate) async fn mirror_files(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<MirrorFile>, StoreError> {
    let rows =
        sqlx::query(sqlx::AssertSqlSafe(format!("{FINDING_SELECT} WHERE f.work_id = ? ORDER BY f.created, f.id")))
            .bind(work)
            .fetch_all(&mut *conn)
            .await?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let mut findings = Vec::with_capacity(rows.len());
    for row in &rows {
        findings.push(finding_of(conn, row).await?);
    }
    let order: HashMap<i64, usize> =
        crate::snapshot::ordered_currents(conn, work).await?.into_iter().enumerate().map(|(i, c)| (c.key, i)).collect();
    let labels: HashMap<i64, String> =
        sqlx::query_as::<_, (i64, String, String)>("SELECT id, number, title FROM node WHERE work_id = ?")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .map(|(id, number, title)| (id, if number.is_empty() { title } else { format!("{number} {title}") }))
            .collect();
    let threads: HashMap<i64, String> =
        sqlx::query_as::<_, (i64, String)>("SELECT id, title FROM chat_thread WHERE work_id = ?")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
    let by_uuid: HashMap<String, i64> = findings.iter().map(|f| (f.uuid.clone(), f.id)).collect();
    let group = |f: &Finding| match (f.key, f.thread) {
        (Some(key), _) => Group::Section(order.get(&key).copied().unwrap_or(usize::MAX)),
        (None, Some(thread)) => Group::Thread(thread),
        (None, None) => Group::Loose,
    };
    let mut sorted: Vec<&Finding> = findings.iter().collect();
    sorted.sort_by_key(|f| (group(f), f.key, f.created.clone(), f.id));

    let mut out = String::from(
        "# Befunde\n\nBefunde aus den Antworten der Personas (Runde 28). Den Status setzt nur der Mensch; Assistenten \
         schlagen ihn nur vor. Verweis im Text: [B<id>].\n",
    );
    let mut last: Option<(Group, Option<i64>)> = None;
    for f in sorted {
        let here = (group(f), f.key);
        if last != Some(here) {
            let heading = match (f.key, f.thread) {
                (Some(key), _) => labels.get(&key).cloned().unwrap_or_else(|| format!("Abschnitt {key}")),
                (None, Some(thread)) => {
                    format!("Faden „{}“", threads.get(&thread).cloned().unwrap_or_else(|| thread.to_string()))
                }
                (None, None) => "Ohne Abschnitt".to_owned(),
            };
            out.push_str(&format!("\n## {heading}\n"));
            last = Some(here);
        }
        out.push_str(&format!("\n### [B{}] {} · {} · {}\n\n", f.id, f.kind.label(), f.status.label(), f.persona));
        for line in f.body.lines() {
            out.push_str(&format!("> {line}\n"));
        }
        out.push('\n');
        if !f.quote.is_empty() {
            let check = if f.quote_check.is_empty() { String::new() } else { format!(" {}", f.quote_check) };
            out.push_str(&format!("- Zitat: „{}“{check}\n", one_line(&f.quote)));
        } else if !f.quote_check.is_empty() {
            out.push_str(&format!("- Prüfvermerk: {}\n", f.quote_check));
        }
        if !f.reason.is_empty() {
            out.push_str(&format!("- Begründung: {}\n", one_line(&f.reason)));
        }
        if f.status == FindingStatus::Adopted {
            match f.adopted_rev {
                Some(rev) => out.push_str(&format!("- Übernommen in {}\n", adopted_words(rev, f.adopted_amend))),
                None => out.push_str("- Übernommen in einer Fassung, die es nicht mehr gibt\n"),
            }
        }
        if f.status == FindingStatus::Deferred {
            let to = match &f.deferred {
                Some(DeferTarget::Node { key }) => {
                    format!("Abschnitt {}", labels.get(key).cloned().unwrap_or_else(|| key.to_string()))
                }
                Some(DeferTarget::Idea { id }) => format!("Einfall {id}"),
                None => "ein Ziel, das es nicht mehr gibt".to_owned(),
            };
            out.push_str(&format!("- Verschoben nach {to}\n"));
        }
        if let Some(original) = f.similar_to {
            out.push_str(&format!("- Angehängt an [B{original}] (gleicht ihm)\n"));
        }
        if !f.summary_of.is_empty() {
            let refs: Vec<String> = f
                .summary_of
                .iter()
                .map(|u| by_uuid.get(u).map_or_else(|| u.clone(), |id| format!("[B{id}]")))
                .collect();
            out.push_str(&format!("- Sammelvorschlag zu {}\n", refs.join(", ")));
        }
        if let Some(p) = &f.pending {
            out.push_str(&format!("- Offener Vorschlag {}: {} von {}\n", p.id, p.status.label(), p.actor.name()));
        }
        let origin = match (f.post, f.thread_post) {
            (Some(post), _) => format!("Beitrag {post}"),
            (None, Some(tp)) => format!("Fadenbeitrag {tp}"),
            (None, None) => "Beitrag gelöscht".to_owned(),
        };
        out.push_str(&format!("- Herkunft: {origin}, Stelle {}, angelegt {}\n", f.position, f.created));
        let history = sqlx::query(sqlx::AssertSqlSafe(format!("{ENTRY_SELECT} WHERE s.finding_id = ? ORDER BY s.id")))
            .bind(f.id)
            .fetch_all(&mut *conn)
            .await?;
        if !history.is_empty() {
            out.push_str("- Verlauf:\n");
            for row in &history {
                let e = entry_of(row)?;
                let what = if e.proposed { "schlägt vor" } else { "setzt" };
                let mut line = format!("  - {}: {} {what} {}", e.created, e.actor.name(), e.status.label());
                if let Some(rev) = e.adopted_rev {
                    line.push_str(&format!(" ({})", adopted_words(rev, e.adopted_amend)));
                }
                if !e.reason.is_empty() {
                    line.push_str(&format!(" – {}", one_line(&e.reason)));
                }
                if let Some(decision) = &e.decision {
                    let word = match decision.as_str() {
                        "confirmed" => "bestätigt",
                        "rejected" => "abgelehnt",
                        _ => "veraltet",
                    };
                    line.push_str(&format!(
                        "; {word} von {} ({})",
                        e.decided_by.as_deref().unwrap_or("?"),
                        e.decided.as_deref().unwrap_or("?")
                    ));
                }
                out.push_str(&line);
                out.push('\n');
            }
        }
    }
    Ok(vec![MirrorFile { path: MIRROR_FILE.to_owned(), contents: out }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_and_statuses_round_trip_through_the_database_words() {
        for kind in FindingKind::ALL {
            assert_eq!(FindingKind::from_db(kind.as_db()), Some(kind), "Art {kind:?} hin und zurück");
        }
        for status in FindingStatus::ALL {
            assert_eq!(FindingStatus::from_db(status.as_db()), Some(status), "Status {status:?} hin und zurück");
        }
        assert_eq!(FindingKind::from_db("lob"), None, "nur die Datenbankwörter, keine Anzeige");
        assert_eq!(FindingStatus::label(FindingStatus::Done), "erledigt ohne Änderung", "Anzeige nach Vertrag §1.1");
        assert_eq!(FindingKind::label(FindingKind::Critical), "kritisch", "Anzeige nach Vertrag §1.1");
    }
}
