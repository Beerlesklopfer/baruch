// SPDX-License-Identifier: AGPL-3.0-or-later
//! Requests concerning the outline (migration 0012, wish 2026-11-06 „UD nach Bestätigung“).
//!
//! Assistants and MCP clients may create outline items ([`Store::outline_add_many`]), but renaming, moving, changing
//! the level and deleting change nothing at once: [`Store::outline_request`] creates a **request**. The human confirms
//! or rejects ([`Store::outline_request_decide`]).
//!
//! Rules:
//! - The request names items only through their fixed key (`node.id`) and remembers the state at the time of the
//!   request ([`Expected`]: title and level of the item, title of the target when moving).
//! - On the request the change runs on trial in a savepoint (rolled back afterwards): what would not work even now
//!   (title rules, levels, deleting with sub-items, no effect) is rejected right away and not stored. An identical open
//!   request is not created twice.
//! - Confirming first checks whether the request is stale (item or target gone, title or level different by now); then
//!   it is closed as `stale` with a message and nothing is changed. Otherwise the outline function runs under the name
//!   of the human; if it refuses, likewise `stale` with its message. Rejecting changes only the request.
//! - A request is decided exactly once.
//! - Preview (Round 14, [`Store::outline_request_preview`]): the outline as it would be after confirming, computed in
//!   the savepoint and rolled back (the database stays); per item level, number, title and a mark ([`PreviewMark`]),
//!   deleted items at their old position. Since Round 16c additionally the kind (`kind_word`) and number/level before
//!   confirming, so that „Teil 1“ and „Kapitel 1“ can be told apart and adopted chapters show "before → after".
//! - Consent to a new part (user question 12, decision of the user 2026-11-07: „es braucht eine aktive Zustimmung
//!   (Popup)“, migration 0035): if an assistant creates a part in such a way that existing chapters move below it, that
//!   is no longer a mere creation. [`Store::outline_add_by_assistant`] then creates nothing but a request
//!   [`OutlineOp::Add`] with the chapters it would adopt (`Expected::adopted`). Only the consent of the human executes
//!   it; revision 1 of the new items then carries the assistant (it authored them), the commit the human as the
//!   confirmer. If the part would adopt other chapters on consent than at the time of the request, it is closed as
//!   stale and nothing is changed.
//! - New part from the exchange with identifier and revisions (Round 22, contract §2.10): if a new part that was read
//!   in would adopt existing chapters, the request `Add` carries in `part` the identifier of the part and all its
//!   revisions ([`PartIdentity`]: number, text, origin, status, base, state and amendments). On consent
//!   `crate::exchange_apply::adopt_part` creates the part with **this** identifier and **these** revisions (revision 1
//!   produced by `add_many` is replaced, without a tombstone); that way a later sync recognises it again. If the
//!   identifier exists here by now, the request is stale. Rejecting creates nothing.

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use baruch_core::Author;

use crate::outline::{self, NewPoint, OutlineAdded, OutlineChange, Place, PointState};
use crate::{IdeaOrigin, Store, StoreError, WorkId, broken};

/// What a request wants to change; items through their fixed key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OutlineOp {
    /// Rename.
    Rename { key: i64, title: String },
    /// Move before/behind/below `target_key` (the level stays, as with `outline.move`).
    Move { key: i64, target_key: i64, place: Place },
    /// Set level 1–3, sub-items move along.
    SetLevel { key: i64, level: u8 },
    /// Delete (only without sub-items, not the last chapter).
    Delete { key: i64 },
    /// Count with (`true`) or without a number (`false`).
    SetNumbered { key: i64, numbered: bool },
    /// Create new items (among them at least one part) before/behind `key` that would adopt existing chapters (user
    /// question 12); `model` for revision 1 (the author is the requester). Round 22: `part` = identifier and revisions
    /// of a part from the exchange (then it is created exactly like that).
    Add {
        key: i64,
        place: Place,
        points: Vec<NewPoint>,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<PartIdentity>,
    },
}

/// Identifier and revisions of a new part from the exchange (Round 22, contract §2.10), stored in `params`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartIdentity {
    /// Identifier of the part (`node.uuid`).
    pub uuid: String,
    /// Revisions by number, exactly one current.
    pub revisions: Vec<PartRevision>,
}

/// A revision of a part from the exchange (columns as in `revision`; base through the identifier).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartRevision {
    pub uuid: String,
    pub number: u32,
    pub body: String,
    /// `human` or `assistant`.
    pub author_kind: String,
    pub author_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `current`, `proposed`, `rejected` or `superseded`.
    pub status: String,
    #[serde(default)]
    pub note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    pub created: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edited: Option<String>,
    #[serde(default)]
    pub amend_seq: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on_amend: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amends: Vec<PartAmend>,
}

/// A step in the history of a revision of a part from the exchange (columns as in `revision_amend`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartAmend {
    pub uuid: String,
    pub seq: u32,
    /// `amend` or `undo`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo_of: Option<u32>,
    pub author: String,
    pub created: String,
    pub before: String,
    pub after: String,
}

impl OutlineOp {
    /// Value of the column `op` (`rename`, `move`, `set_level`, `delete`, `set_numbered`, `add`).
    pub fn name(&self) -> &'static str {
        match self {
            OutlineOp::Rename { .. } => "rename",
            OutlineOp::Move { .. } => "move",
            OutlineOp::SetLevel { .. } => "set_level",
            OutlineOp::Delete { .. } => "delete",
            OutlineOp::SetNumbered { .. } => "set_numbered",
            OutlineOp::Add { .. } => "add",
        }
    }

    /// Fixed key of the item concerned.
    pub fn key(&self) -> i64 {
        match self {
            OutlineOp::Rename { key, .. }
            | OutlineOp::Move { key, .. }
            | OutlineOp::SetLevel { key, .. }
            | OutlineOp::Delete { key }
            | OutlineOp::SetNumbered { key, .. }
            | OutlineOp::Add { key, .. } => *key,
        }
    }

    /// Does the request need the active consent of the human in the dialog (user question 12)? So far only a new part
    /// that would adopt existing chapters.
    pub fn needs_consent(&self) -> bool {
        matches!(self, OutlineOp::Add { .. })
    }
}

/// State at the time of the request: by it the confirmation recognises a stale request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expected {
    pub title: String,
    pub level: u8,
    /// Title of the target when moving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_title: Option<String>,
    /// When creating a part (user question 12): fixed keys of the existing chapters that would move below the new part
    /// – exactly these are shown by the preview, exactly these must be adopted by the consent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adopted: Vec<i64>,
}

/// Decision on a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Confirmed and executed.
    Confirmed,
    /// Rejected by the human.
    Rejected,
    /// Stale on confirmation or no longer executable (`message` says why); nothing changed.
    Stale,
}

impl Decision {
    fn column(self) -> &'static str {
        match self {
            Decision::Confirmed => "confirmed",
            Decision::Rejected => "rejected",
            Decision::Stale => "stale",
        }
    }

    fn from_column(text: &str) -> Result<Decision, StoreError> {
        match text {
            "confirmed" => Ok(Decision::Confirmed),
            "rejected" => Ok(Decision::Rejected),
            "stale" => Ok(Decision::Stale),
            other => Err(broken(format!("unbekannte Entscheidung {other:?}"))),
        }
    }

    /// Display: „bestätigt“, „abgelehnt“, „veraltet“.
    pub fn label(self) -> &'static str {
        match self {
            Decision::Confirmed => "bestätigt",
            Decision::Rejected => "abgelehnt",
            Decision::Stale => "veraltet",
        }
    }
}

/// A request concerning the outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutlineRequest {
    pub id: i64,
    pub work: WorkId,
    pub op: OutlineOp,
    pub expected: Expected,
    /// Requester (persona or MCP client; the human changes directly).
    pub origin: IdeaOrigin,
    pub note: String,
    pub created: String,
    /// `None` = open.
    pub decided: Option<String>,
    pub decision: Option<Decision>,
    pub decided_by: Option<String>,
    pub message: Option<String>,
    /// German for list and commit, for example „„Alt“ in „Neu“ umbenennen“.
    pub summary: String,
}

impl OutlineRequest {
    /// Not decided yet?
    pub fn is_open(&self) -> bool {
        self.decision.is_none()
    }
}

/// Result of [`Store::outline_request_decide`]: the request afterwards and, if confirmed and executed, the outline
/// change (for answer and mirror).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestDecided {
    pub request: OutlineRequest,
    pub change: Option<OutlineChange>,
}

/// What a request changes about an item of the preview (Round 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMark {
    /// Stays (number and position can change nevertheless, for example after a deletion).
    Same,
    /// New title.
    Renamed,
    /// Moved (the item of the request together with its sub-items).
    Moved,
    /// Another level (when changing the level also the sub-items that move along).
    Level,
    /// Now counts with or without a number.
    Numbered,
    /// Is dropped (shown at its old position, with the old level and number).
    Deleted,
    /// Newly created (request [`OutlineOp::Add`]).
    Added,
    /// Existing chapter that moves below the new part (request [`OutlineOp::Add`]).
    Adopted,
}

/// An item of the outline after the confirmation of a request (preview, Round 14).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PreviewPoint {
    pub key: i64,
    pub level: u8,
    pub number: String,
    pub title: String,
    pub mark: PreviewMark,
    /// Kind of the item after confirming: „Teil“, „Kapitel“, „Abschnitt“, „Unterabschnitt“ (Round 16c: unambiguous
    /// together with `number`, for example „Teil 1“ and „Kapitel 1“).
    pub kind_word: &'static str,
    /// Number before confirming (`None` = new item); for deleted items like `number`.
    pub before_number: Option<String>,
    /// Level before confirming (`None` = new item).
    pub before_level: Option<u8>,
}

impl PreviewPoint {
    /// Kind and number, for example „Teil 1“, „Kapitel 2“; without a number „Kapitel (ohne Nummer)“.
    pub fn label(&self) -> String {
        label(self.kind_word, &self.number)
    }

    /// Kind and number before confirming (`None` = new item).
    pub fn before_label(&self) -> Option<String> {
        let level = self.before_level?;
        Some(label(outline::level_word(level), self.before_number.as_deref().unwrap_or("")))
    }
}

fn label(kind_word: &str, number: &str) -> String {
    let number = number.trim().trim_end_matches('.');
    if number.is_empty() { format!("{kind_word} (ohne Nummer)") } else { format!("{kind_word} {number}") }
}

/// Preview from the outlines before and after the trial run: order of afterwards, deleted items behind their old
/// predecessor (or at the beginning).
fn preview_points(op: &OutlineOp, adopted: &[i64], before: &[PointState], after: &[PointState]) -> Vec<PreviewPoint> {
    let old: std::collections::HashMap<i64, &PointState> = before.iter().map(|p| (p.key, p)).collect();
    // When moving: the item and its sub-items (following items with a larger level).
    let mut moved = std::collections::HashSet::new();
    if let OutlineOp::Move { key, .. } = op
        && let Some(start) = after.iter().position(|p| p.key == *key)
    {
        moved.insert(*key);
        for point in after[start + 1..].iter().take_while(|p| p.level > after[start].level) {
            moved.insert(point.key);
        }
    }
    let mut out: Vec<PreviewPoint> = after
        .iter()
        .map(|point| {
            let mark = match old.get(&point.key) {
                _ if moved.contains(&point.key) => PreviewMark::Moved,
                None if matches!(op, OutlineOp::Add { .. }) => PreviewMark::Added,
                _ if adopted.contains(&point.key) => PreviewMark::Adopted,
                Some(was) if was.title != point.title => PreviewMark::Renamed,
                Some(was) if was.level != point.level => PreviewMark::Level,
                Some(was) if was.numbered != point.numbered => PreviewMark::Numbered,
                _ => PreviewMark::Same,
            };
            let was = old.get(&point.key);
            PreviewPoint {
                key: point.key,
                level: point.level,
                number: point.number.clone(),
                title: point.title.clone(),
                mark,
                kind_word: outline::level_word(point.level),
                before_number: was.map(|w| w.number.clone()),
                before_level: was.map(|w| w.level),
            }
        })
        .collect();
    for (index, gone) in before.iter().enumerate() {
        if after.iter().any(|p| p.key == gone.key) {
            continue;
        }
        let at = index
            .checked_sub(1)
            .and_then(|previous| out.iter().position(|p| p.key == before[previous].key))
            .map_or(0, |p| p + 1);
        out.insert(
            at,
            PreviewPoint {
                key: gone.key,
                level: gone.level,
                number: gone.number.clone(),
                title: gone.title.clone(),
                mark: PreviewMark::Deleted,
                kind_word: outline::level_word(gone.level),
                before_number: Some(gone.number.clone()),
                before_level: Some(gone.level),
            },
        );
    }
    out
}

/// Stored form in `params`.
#[derive(Serialize, Deserialize)]
struct Stored {
    op: OutlineOp,
    expected: Expected,
}

#[cfg(test)]
fn summary(op: &OutlineOp, expected: &Expected) -> String {
    summary_for(op, expected, false)
}

/// Like [`summary`]; `sermon`: the work is a sermon, then the moving items are called „Predigtteile“ instead of
/// „Kapitel“ (Round 17b, L8).
fn summary_for(op: &OutlineOp, expected: &Expected, sermon: bool) -> String {
    let title = outline::quoted(&expected.title);
    match op {
        OutlineOp::Rename { title: new, .. } => format!("{title} in {} umbenennen", outline::quoted(new)),
        OutlineOp::Move { place, .. } => {
            let word = match place {
                Place::Before => "vor",
                Place::After => "hinter",
                Place::Into => "unter",
            };
            let target = expected.target_title.as_deref().map_or_else(|| "?".to_owned(), outline::quoted);
            format!("{title} {word} {target} verschieben")
        }
        OutlineOp::SetLevel { level, .. } => {
            format!("{title} auf Ebene {level} ({}) stufen", outline::level_word(*level))
        }
        OutlineOp::Delete { .. } => format!("{title} löschen"),
        OutlineOp::SetNumbered { numbered: true, .. } => format!("{title} mit Nummer zählen"),
        OutlineOp::SetNumbered { numbered: false, .. } => format!("{title} ohne Nummer zählen"),
        OutlineOp::Add { place, points, part, .. } => {
            let word = match place {
                Place::Before => "vor",
                Place::After | Place::Into => "hinter",
            };
            // Round 22: part from the exchange with identifier and revisions.
            let identity = match part {
                Some(part) if part.revisions.len() == 1 => " (mit Kennung und 1 Fassung)".to_owned(),
                Some(part) => format!(" (mit Kennung und {} Fassungen)", part.revisions.len()),
                None => String::new(),
            };
            let new: Vec<String> = points.iter().map(|p| outline::quoted(&p.title)).collect();
            let what = if points.len() == 1 { "Teil" } else { "Punkte" };
            let chapters = match (expected.adopted.len(), sermon) {
                (1, false) => "1 vorhandenes Kapitel wandert".to_owned(),
                (n, false) => format!("{n} vorhandene Kapitel wandern"),
                (1, true) => "1 vorhandener Predigtteil wandert".to_owned(),
                (n, true) => format!("{n} vorhandene Predigtteile wandern"),
            };
            format!("{what} {}{identity} {word} {title} anlegen; {chapters} unter den neuen Teil", new.join(", "))
        }
    }
}

type RequestRow = (
    i64,
    i64,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Columns of a request, last the kind of work (for the wording of the summary, Round 17b).
const COLUMNS: &str = "id, work_id, params, origin_kind, origin_name, note, created, decided, decision, decided_by, \
     message, (SELECT w.kind FROM work w WHERE w.id = outline_request.work_id)";

fn from_row(row: RequestRow) -> Result<OutlineRequest, StoreError> {
    let (id, work, params, origin_kind, origin_name, note, created, decided, decision, decided_by, message, kind) = row;
    let stored: Stored =
        serde_json::from_str(&params).map_err(|e| broken(format!("Antrag {id}: params unlesbar: {e}")))?;
    let origin = match origin_kind.as_str() {
        "human" => IdeaOrigin::Human { name: origin_name },
        "persona" => IdeaOrigin::Persona { handle: origin_name },
        "mcp" => IdeaOrigin::Mcp { handle: origin_name },
        other => return Err(broken(format!("Antrag {id}: unbekannte Herkunft {other:?}"))),
    };
    let decision = decision.as_deref().map(Decision::from_column).transpose()?;
    let summary = summary_for(&stored.op, &stored.expected, kind.as_deref() == Some("sermon"));
    Ok(OutlineRequest {
        id,
        work,
        op: stored.op,
        expected: stored.expected,
        origin,
        note,
        created,
        decided,
        decision,
        decided_by,
        message,
        summary,
    })
}

async fn fetch(conn: &mut SqliteConnection, work: WorkId, id: i64) -> Result<OutlineRequest, StoreError> {
    let row: Option<RequestRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM outline_request WHERE id = ? AND work_id = ?"
    )))
    .bind(id)
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    from_row(row.ok_or(StoreError::UnknownRequest { work, id })?)
}

/// State of the item `key` or `GoneNode`.
async fn state_of(conn: &mut SqliteConnection, work: WorkId, key: i64) -> Result<(usize, u8, String), StoreError> {
    outline::point_state(conn, work, key).await?.ok_or(StoreError::GoneNode { work, key })
}

/// Execute a request (keys → positions in the same state), under the name `human`; when creating ([`OutlineOp::Add`])
/// the requester `origin` is the author of revision 1. In addition the fixed keys of the adopted chapters (only when
/// creating, otherwise empty).
async fn run(
    conn: &mut SqliteConnection,
    work: WorkId,
    op: &OutlineOp,
    origin: &IdeaOrigin,
    human: &str,
) -> Result<(OutlineChange, Vec<i64>), StoreError> {
    let (node, _, _) = state_of(conn, work, op.key()).await?;
    let change = match op {
        OutlineOp::Rename { title, .. } => outline::rename(conn, work, node, title, human).await,
        OutlineOp::Move { target_key, place, .. } => {
            let (target, _, _) = state_of(conn, work, *target_key).await?;
            outline::move_node(conn, work, node, target, *place, human).await
        }
        OutlineOp::SetLevel { level, .. } => outline::set_level(conn, work, node, *level, human).await,
        OutlineOp::Delete { .. } => outline::delete(conn, work, node, human).await,
        OutlineOp::SetNumbered { numbered, .. } => outline::set_numbered(conn, work, node, *numbered, human).await,
        OutlineOp::Add { place, points, model, part, .. } => {
            let author = Author::Assistant { persona: origin.name().to_owned(), model: model.clone() };
            let added = outline::add_many(conn, work, Some(node), *place, points, &author).await?;
            // Round 22: part from the exchange with its identifier and its revisions.
            if let Some(part) = part {
                crate::exchange_apply::adopt_part(conn, work, &added, part).await?;
            }
            return Ok((added.change, added.adopted));
        }
    }?;
    Ok((change, Vec::new()))
}

/// Would a request [`OutlineOp::Add`] now adopt other chapters than at the time of the request? Message, otherwise
/// `None`.
fn adoption_changed(request: &OutlineRequest, adopted: &[i64]) -> Option<String> {
    if !matches!(request.op, OutlineOp::Add { .. }) || adopted == request.expected.adopted.as_slice() {
        return None;
    }
    Some(format!(
        "Der neue Teil übernähme inzwischen andere Kapitel ({} statt {} beim Antrag); bitte neu beantragen.",
        adopted.len(),
        request.expected.adopted.len()
    ))
}

/// Is the request stale? Message, otherwise `None`.
async fn staleness(
    conn: &mut SqliteConnection,
    work: WorkId,
    request: &OutlineRequest,
) -> Result<Option<String>, StoreError> {
    let expected = &request.expected;
    let Some((_, level, title)) = outline::point_state(conn, work, request.op.key()).await? else {
        return Ok(Some(format!(
            "Den Gliederungspunkt {} gibt es nicht mehr (gelöscht oder zusammengeführt).",
            outline::quoted(&expected.title)
        )));
    };
    if title != expected.title {
        return Ok(Some(format!(
            "Der Punkt heißt inzwischen {} (beim Antrag {}).",
            outline::quoted(&title),
            outline::quoted(&expected.title)
        )));
    }
    if level != expected.level {
        return Ok(Some(format!(
            "{} steht inzwischen auf Ebene {level} (beim Antrag {}).",
            outline::quoted(&title),
            expected.level
        )));
    }
    if let OutlineOp::Move { target_key, .. } = &request.op {
        let wanted = expected.target_title.as_deref().unwrap_or_default();
        match outline::point_state(conn, work, *target_key).await? {
            None => {
                return Ok(Some(format!(
                    "Das Ziel {} gibt es nicht mehr (gelöscht oder zusammengeführt).",
                    outline::quoted(wanted)
                )));
            }
            Some((_, _, target_title)) if target_title != wanted => {
                return Ok(Some(format!(
                    "Das Ziel heißt inzwischen {} (beim Antrag {}).",
                    outline::quoted(&target_title),
                    outline::quoted(wanted)
                )));
            }
            Some(_) => {}
        }
    }
    Ok(None)
}

/// Domain rejection by an outline function (then the request is not executable)?
fn refusal(e: &StoreError) -> bool {
    matches!(
        e,
        StoreError::Refused(_)
            | StoreError::Title(_)
            | StoreError::Heading(_)
            | StoreError::GoneNode { .. }
            | StoreError::UnknownNode { .. }
    )
}

async fn decide_row(
    conn: &mut SqliteConnection,
    id: i64,
    decision: Decision,
    human: &str,
    message: Option<&str>,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE outline_request SET decided = strftime('%Y-%m-%dT%H:%M:%fZ','now'), decision = ?, decided_by = ?, \
         message = ? WHERE id = ? AND decided IS NULL",
    )
    .bind(decision.column())
    .bind(human)
    .bind(message)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Store a request (in the transaction of the caller); an identical open request → `Refused`.
/// Round 23: at most this many **open** requests concerning the outline per work and origin (persona or MCP client).
/// Every open request costs a preview under the write lock on `/gliederung`; without a limit a client could paralyse
/// the page with many different requests.
pub const MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN: usize = 50;

/// Message for [`MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN`] (`Refused` carries only fixed text).
const TOO_MANY_OPEN: &str = "Zu viele offene Anträge zur Gliederung von dieser Herkunft in diesem Werk (höchstens 50). \
Nichts angelegt; erst entscheidet der Mensch über die offenen (outline_requests zeigt sie).";

/// Check the limit [`MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN`] (in the transaction, before creating). The human is exempt;
/// reading in an exchange file ([`outline_request_in`], [`add_request_in`]) does not check it, so that a file with many
/// open requests stays readable.
async fn check_open_limit(conn: &mut SqliteConnection, work: WorkId, origin: &IdeaOrigin) -> Result<(), StoreError> {
    let (kind, name) = match origin {
        IdeaOrigin::Human { .. } => return Ok(()),
        IdeaOrigin::Persona { handle } => ("persona", handle),
        IdeaOrigin::Mcp { handle } => ("mcp", handle),
    };
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM outline_request WHERE work_id = ? AND decided IS NULL AND origin_kind = ? \
         AND origin_name = ?",
    )
    .bind(work)
    .bind(kind)
    .bind(name)
    .fetch_one(&mut *conn)
    .await?;
    if usize::try_from(open).unwrap_or(usize::MAX) >= MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN {
        return Err(StoreError::Refused(TOO_MANY_OPEN));
    }
    Ok(())
}

async fn insert_request(
    conn: &mut SqliteConnection,
    work: WorkId,
    op: &OutlineOp,
    expected: &Expected,
    origin: &IdeaOrigin,
    note: &str,
) -> Result<OutlineRequest, StoreError> {
    let params = serde_json::to_string(&Stored { op: op.clone(), expected: expected.clone() })
        .map_err(|e| broken(format!("Antrag: {e}")))?;
    let twin: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM outline_request WHERE work_id = ? AND decided IS NULL AND op = ? AND params = ?",
    )
    .bind(work)
    .bind(op.name())
    .bind(&params)
    .fetch_optional(&mut *conn)
    .await?;
    if twin.is_some() {
        return Err(StoreError::Refused("Diesen Antrag gibt es schon (offen); der Mensch entscheidet darüber."));
    }
    let (kind, name) = match origin {
        IdeaOrigin::Human { name } => ("human", name),
        IdeaOrigin::Persona { handle } => ("persona", handle),
        IdeaOrigin::Mcp { handle } => ("mcp", handle),
    };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO outline_request (work_id, op, params, origin_kind, origin_name, note) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(work)
    .bind(op.name())
    .bind(&params)
    .bind(kind)
    .bind(name)
    .bind(note.trim())
    .fetch_one(&mut *conn)
    .await?;
    fetch(conn, work, id).await
}

/// Create a request concerning the outline, in the transaction of the caller (core of [`Store::outline_request`], also
/// for reading in an exchange file, Round 26). Rules as there: item and target must exist, the change must work in the
/// trial run (savepoint, rolled back), an identical open request → `Refused`.
pub(crate) async fn outline_request_in(
    conn: &mut SqliteConnection,
    work: WorkId,
    op: OutlineOp,
    origin: &IdeaOrigin,
    note: &str,
) -> Result<OutlineRequest, StoreError> {
    let (_, level, title) = state_of(conn, work, op.key()).await?;
    let target_title = match &op {
        OutlineOp::Move { target_key, .. } => Some(state_of(conn, work, *target_key).await?.2),
        _ => None,
    };
    // Check the title as everywhere and store it trimmed. Creating runs through `outline_add_by_assistant` (there it is
    // checked whether it becomes a request at all).
    let op = match op {
        OutlineOp::Rename { key, title } => OutlineOp::Rename { key, title: outline::check_title(&title)? },
        OutlineOp::Add { .. } => {
            return Err(StoreError::Refused("Anlegen beantragt man über outline_add, nicht als Antrag."));
        }
        other => other,
    };
    {
        // Trial run in the savepoint: what would not work even now does not become a request in the first place.
        let mut probe = sqlx::Connection::begin(&mut *conn).await?;
        let tried = run(&mut probe, work, &op, origin, "Probelauf").await;
        probe.rollback().await?;
        tried?;
    }
    let expected = Expected { title, level, target_title, adopted: Vec::new() };
    insert_request(conn, work, &op, &expected, origin, note).await
}

/// Request [`OutlineOp::Add`] for new items before/behind `key`, in the transaction of the caller (exchange, Round 26:
/// a new part from the file that would adopt existing chapters). Trial run as in [`Store::outline_add_by_assistant`]
/// (savepoint, always rolled back): if the items would adopt no existing chapters, no request is created (`Ok(None)`,
/// nothing written); otherwise the request with exactly these chapters. Errors of the trial run (levels, place) and an
/// identical open request (`Refused`) as there.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn add_request_in(
    conn: &mut SqliteConnection,
    work: WorkId,
    key: i64,
    place: Place,
    points: &[NewPoint],
    origin: &IdeaOrigin,
    model: &str,
    note: &str,
    part: Option<PartIdentity>,
) -> Result<Option<OutlineRequest>, StoreError> {
    let (target, level, title) = state_of(conn, work, key).await?;
    let author = Author::Assistant { persona: origin.name().to_owned(), model: model.to_owned() };
    let added = {
        let mut probe = sqlx::Connection::begin(&mut *conn).await?;
        let added = outline::add_many(&mut probe, work, Some(target), place, points, &author).await;
        probe.rollback().await?;
        added?
    };
    if added.adopted.is_empty() {
        return Ok(None);
    }
    let points: Vec<NewPoint> =
        points.iter().map(|p| NewPoint { title: p.title.trim().to_owned(), level: p.level }).collect();
    let op = OutlineOp::Add { key, place, points, model: model.to_owned(), part };
    let expected = Expected { title, level, target_title: None, adopted: added.adopted };
    insert_request(conn, work, &op, &expected, origin, note).await.map(Some)
}

/// Result of [`Store::outline_add_by_assistant`]: created (no existing chapter moves) or a request to which the human
/// must consent (user question 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantAdd {
    Added(OutlineAdded),
    Requested(OutlineRequest),
}

impl Store {
    /// Create outline items by an assistant (MCP `outline_add`, atomic): like [`Store::outline_add_many`] with the
    /// requester `origin` and `model` as the author of revision 1 – unless a new part would adopt existing chapters
    /// (user question 12, 2026-11-07). Then nothing changes, but a request [`OutlineOp::Add`] with the chapters it
    /// would adopt is created, to which the human consents ([`Store::outline_request_decide`]). Errors (title rules,
    /// levels, part in the wrong place, identical open request) as there; then nothing is created.
    #[allow(clippy::too_many_arguments)]
    pub async fn outline_add_by_assistant(
        &self,
        work: WorkId,
        target: Option<usize>,
        place: Place,
        points: &[NewPoint],
        origin: &IdeaOrigin,
        model: &str,
        note: &str,
    ) -> Result<AssistantAdd, StoreError> {
        let author = Author::Assistant { persona: origin.name().to_owned(), model: model.to_owned() };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let added = {
            // Trial run in the savepoint: if it adopts nothing, it stands; otherwise back and a request.
            let mut probe = sqlx::Connection::begin(&mut *tx).await?;
            let added = match outline::add_many(&mut probe, work, target, place, points, &author).await {
                Ok(added) => added,
                Err(e) => {
                    probe.rollback().await?;
                    return Err(e);
                }
            };
            if added.adopted.is_empty() {
                probe.commit().await?;
                tx.commit().await?;
                return Ok(AssistantAdd::Added(added));
            }
            probe.rollback().await?;
            added
        };
        // Only existing chapters are adopted; without a reference item there are none.
        let target = target.ok_or_else(|| broken("Teil übernimmt Kapitel ohne Bezugspunkt".into()))?;
        let key: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
            .bind(work)
            .bind(i64::try_from(target).map_err(|_| broken("Stelle zu groß".into()))?)
            .fetch_one(&mut *tx)
            .await?;
        let (_, level, title) = state_of(&mut tx, work, key).await?;
        let points: Vec<NewPoint> =
            points.iter().map(|p| NewPoint { title: p.title.trim().to_owned(), level: p.level }).collect();
        let op = OutlineOp::Add { key, place, points, model: model.to_owned(), part: None };
        let expected = Expected { title, level, target_title: None, adopted: added.adopted };
        check_open_limit(&mut tx, work, origin).await?;
        let request = insert_request(&mut tx, work, &op, &expected, origin, note).await?;
        tx.commit().await?;
        Ok(AssistantAdd::Requested(request))
    }

    /// Create a request concerning the outline (atomic). The item (and, when moving, the target) must exist, otherwise
    /// `GoneNode`; the change must be executable now (trial run, rolled back), otherwise its error (for example title
    /// rules → `Title`, deleting with sub-items → `Refused`); an identical open request → `Refused`. Changes nothing
    /// about the outline. Round 23: if the origin (persona or MCP client) already has
    /// [`MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN`] open requests in the work → `Refused`.
    pub async fn outline_request(
        &self,
        work: WorkId,
        op: OutlineOp,
        origin: &IdeaOrigin,
        note: &str,
    ) -> Result<OutlineRequest, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        check_open_limit(&mut tx, work, origin).await?;
        let request = outline_request_in(&mut tx, work, op, origin, note).await?;
        tx.commit().await?;
        Ok(request)
    }

    /// Requests of a work by id; `open_only` only the open ones.
    pub async fn outline_requests(&self, work: WorkId, open_only: bool) -> Result<Vec<OutlineRequest>, StoreError> {
        self.work_slug(work).await?;
        let filter = if open_only { " AND decided IS NULL" } else { "" };
        let rows: Vec<RequestRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM outline_request WHERE work_id = ?{filter} ORDER BY id"
        )))
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(from_row).collect()
    }

    /// Open requests of all works that need the active consent of the human (user question 12: kind `add`), by id –
    /// notice bar and dialog on every page.
    pub async fn outline_requests_needing_consent(&self) -> Result<Vec<OutlineRequest>, StoreError> {
        let rows: Vec<RequestRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {COLUMNS} FROM outline_request WHERE decided IS NULL AND op = 'add' ORDER BY id"
        )))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(from_row).collect()
    }

    /// Number of open requests of a work (hint in the outline tree).
    pub async fn open_outline_requests(&self, work: WorkId) -> Result<usize, StoreError> {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM outline_request WHERE work_id = ? AND decided IS NULL")
                .bind(work)
                .fetch_one(&self.pool)
                .await?;
        crate::to_usize(count, "Anträge")
    }

    /// Preview of an open request (Round 14): the outline as it would be after [`Store::outline_request_decide`] with
    /// `confirm` – computed in the savepoint and rolled back, the database stays unchanged. Deleted items stand at
    /// their old position with [`PreviewMark::Deleted`]. Already decided → `Refused`; stale → `Stale` („Veraltet:
    /// <Grund>“, Round 23, before `Heading`); no longer executable → `Heading` with the reason (both rejected like
    /// `Refused`, -32000; `Refused` carries only fixed texts); unknown → `UnknownRequest`.
    pub async fn outline_request_preview(&self, work: WorkId, id: i64) -> Result<Vec<PreviewPoint>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let request = fetch(&mut tx, work, id).await?;
        if !request.is_open() {
            return Err(StoreError::Refused(
                "Über diesen Antrag ist schon entschieden; Vorschau nur für offene Anträge.",
            ));
        }
        if let Some(message) = staleness(&mut tx, work, &request).await? {
            return Err(StoreError::Stale(format!("Veraltet: {message}")));
        }
        let before = outline::point_states(&mut tx, work).await?;
        let tried = {
            let mut probe = sqlx::Connection::begin(&mut *tx).await?;
            let tried = match run(&mut probe, work, &request.op, &request.origin, "Vorschau").await {
                Ok((_, adopted)) => match adoption_changed(&request, &adopted) {
                    Some(message) => Ok(Err(StoreError::Heading(message))),
                    None => outline::point_states(&mut probe, work).await.map(|after| Ok((after, adopted))),
                },
                Err(e) => Ok(Err(e)),
            };
            probe.rollback().await?;
            tried?
        };
        tx.rollback().await?;
        match tried {
            Ok((after, adopted)) => Ok(preview_points(&request.op, &adopted, &before, &after)),
            Err(e) if refusal(&e) => Err(StoreError::Heading(format!("Nicht mehr ausführbar: {e}"))),
            Err(e) => Err(e),
        }
    }

    /// Decide on an open request (atomic, only the human `human`). `confirm = false` rejects and changes nothing else.
    /// `confirm = true` executes it if it is not stale; stale or no longer executable → decision `stale` with a
    /// message, the outline stays. Already decided → `Refused`, unknown → `UnknownRequest`.
    pub async fn outline_request_decide(
        &self,
        work: WorkId,
        id: i64,
        confirm: bool,
        human: &str,
    ) -> Result<RequestDecided, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let request = fetch(&mut tx, work, id).await?;
        if !request.is_open() {
            return Err(StoreError::Refused("Über diesen Antrag ist schon entschieden."));
        }
        let mut change = None;
        if !confirm {
            decide_row(&mut tx, id, Decision::Rejected, human, None).await?;
        } else if let Some(message) = staleness(&mut tx, work, &request).await? {
            decide_row(&mut tx, id, Decision::Stale, human, Some(&message)).await?;
        } else {
            let mut attempt = sqlx::Connection::begin(&mut *tx).await?;
            match run(&mut attempt, work, &request.op, &request.origin, human).await {
                Ok((_, adopted)) if adoption_changed(&request, &adopted).is_some() => {
                    attempt.rollback().await?;
                    let message = adoption_changed(&request, &adopted).unwrap_or_default();
                    decide_row(&mut tx, id, Decision::Stale, human, Some(&message)).await?;
                }
                Ok((done, _)) => {
                    attempt.commit().await?;
                    decide_row(&mut tx, id, Decision::Confirmed, human, None).await?;
                    change = Some(done);
                }
                Err(e) if refusal(&e) => {
                    attempt.rollback().await?;
                    let message = format!("Nicht mehr ausführbar: {e}");
                    decide_row(&mut tx, id, Decision::Stale, human, Some(&message)).await?;
                }
                Err(e) => return Err(e),
            }
        }
        let request = fetch(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(RequestDecided { request, change })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_stored_as_tagged_json_and_read_back_unchanged() {
        let op = OutlineOp::Move { key: 7, target_key: 9, place: Place::After };
        let expected =
            Expected { title: "Ernte".into(), level: 1, target_title: Some("Saat".into()), adopted: Vec::new() };
        let text = serde_json::to_string(&Stored { op: op.clone(), expected: expected.clone() }).unwrap();
        assert!(text.contains(r#""op":"move""#), "Art steht als op im JSON: {text}");
        let back: Stored = serde_json::from_str(&text).unwrap();
        assert_eq!((back.op, back.expected), (op, expected), "params lassen sich verlustfrei lesen");
    }

    #[test]
    fn preview_labels_name_kind_and_number_before_and_after() {
        let point = PreviewPoint {
            key: 1,
            level: 1,
            number: "2.".into(),
            title: "Flut".into(),
            mark: PreviewMark::Adopted,
            kind_word: outline::level_word(1),
            before_number: Some("2".into()),
            before_level: Some(1),
        };
        assert_eq!(point.label(), "Kapitel 2", "Art und Nummer, Schlusspunkt der Nummer entfällt");
        assert_eq!(point.before_label().as_deref(), Some("Kapitel 2"), "vorher mit Art der alten Ebene");
        let part = PreviewPoint {
            level: 0,
            number: String::new(),
            kind_word: outline::level_word(0),
            before_number: None,
            before_level: None,
            ..point
        };
        assert_eq!(part.label(), "Teil (ohne Nummer)", "ohne Nummer ehrlich benannt");
        assert_eq!(part.before_label(), None, "ein neuer Punkt hat kein Vorher");
    }

    #[test]
    fn the_summary_says_in_german_what_the_request_wants() {
        let expected =
            Expected { title: "Ernte".into(), level: 2, target_title: Some("Saat".into()), adopted: Vec::new() };
        let rename = OutlineOp::Rename { key: 1, title: "Lese".into() };
        assert_eq!(summary(&rename, &expected), "„Ernte“ in „Lese“ umbenennen", "Umbenennen mit altem und neuem Titel");
        let moved = OutlineOp::Move { key: 1, target_key: 2, place: Place::Before };
        assert_eq!(summary(&moved, &expected), "„Ernte“ vor „Saat“ verschieben", "Verschieben nennt das Ziel");
        let level = OutlineOp::SetLevel { key: 1, level: 3 };
        assert_eq!(summary(&level, &expected), "„Ernte“ auf Ebene 3 (Unterabschnitt) stufen", "Ebene mit Namen");
        assert_eq!(summary(&OutlineOp::Delete { key: 1 }, &expected), "„Ernte“ löschen", "Löschen");
    }

    #[test]
    fn a_new_part_in_a_sermon_moves_sermon_parts_not_chapters() {
        let expected = Expected { title: "Einstieg".into(), level: 1, target_title: None, adopted: vec![2, 3, 4] };
        let add = OutlineOp::Add {
            key: 1,
            place: Place::Before,
            points: vec![NewPoint { title: "Hauptteil".into(), level: Some(0) }],
            model: "m".into(),
            part: None,
        };
        assert_eq!(
            summary_for(&add, &expected, true),
            "Teil „Hauptteil“ vor „Einstieg“ anlegen; 3 vorhandene Predigtteile wandern unter den neuen Teil",
            "in einer Predigt heißen die Punkte der Ebene 1 Predigtteile (view::level_name), nicht Kapitel"
        );
        assert_eq!(
            summary_for(&add, &expected, false),
            "Teil „Hauptteil“ vor „Einstieg“ anlegen; 3 vorhandene Kapitel wandern unter den neuen Teil",
            "im Buch bleibt die Wortwahl wie in Runde 14"
        );
        let one = Expected { adopted: vec![2], ..expected };
        assert!(
            summary_for(&add, &one, true).ends_with("1 vorhandener Predigtteil wandert unter den neuen Teil"),
            "Einzahl richtig gebeugt: {}",
            summary_for(&add, &one, true)
        );
    }
}
