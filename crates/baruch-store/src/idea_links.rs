// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mind map (contract Round 25, migration `0043_idea_map.sql`): links between ideas (`idea_link`) and stored positions
//! of the ideas on the map of a work (`idea_map_pos`).
//!
//! Rules (contract §2):
//! - A link only idea ↔ idea of the same work (also both without a work), none deleted, no self-reference, note at most
//!   [`MAX_LINK_NOTE_CHARS`] characters without control characters except `\n`.
//! - Kinds [`LinkKind`]; „widerspricht“ is undirected, the store swaps so that `from < to`.
//! - Links are only soft-deleted (`deleted`), restorable. A deleted idea takes its links and position out of all lists
//!   (filter, no writing).
//! - Positions: per (work, idea) at most one, coordinates finite and |v| ≤ [`MAP_COORD_LIMIT`]; pure display, no mirror
//!   commit.
//! - Errors through [`StoreError::Idea`] (→ -32602 as with ideas), no error value of its own; unknown ideas
//!   [`StoreError::UnknownIdea`], unknown works [`StoreError::UnknownWork`].
//! - Every write is exactly one transaction (`BEGIN IMMEDIATE`); first check everything, then write. The column `uuid`
//!   is never set by this code but by the fill trigger (rule from 0041).

use std::collections::BTreeMap;

use sqlx::SqliteConnection;

use crate::{IdeaId, IdeaOrigin, Store, StoreError, WorkId, broken};

/// id of a link (`idea_link.id`).
pub type IdeaLinkId = i64;

/// Maximum length of the note of a link in characters.
pub const MAX_LINK_NOTE_CHARS: usize = 500;

/// Largest absolute value of a coordinate of a position (SVG user units of the map).
pub const MAP_COORD_LIMIT: f64 = 1_000_000.0;

/// Kind of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// „führt weiter“
    LeadsTo,
    /// „gehört zu“
    BelongsTo,
    /// „Beispiel für“
    ExampleOf,
    /// „widerspricht“ (undirected)
    Contradicts,
}

impl LinkKind {
    pub const ALL: [LinkKind; 4] = [LinkKind::LeadsTo, LinkKind::BelongsTo, LinkKind::ExampleOf, LinkKind::Contradicts];

    /// Value in the database and in RPC/MCP.
    pub fn as_str(self) -> &'static str {
        match self {
            LinkKind::LeadsTo => "leads_to",
            LinkKind::BelongsTo => "belongs_to",
            LinkKind::ExampleOf => "example_of",
            LinkKind::Contradicts => "contradicts",
        }
    }

    /// Exactly the words from [`LinkKind::as_str`]; otherwise `None`.
    pub fn parse(s: &str) -> Option<LinkKind> {
        LinkKind::ALL.into_iter().find(|kind| kind.as_str() == s)
    }

    /// Display (German).
    pub fn label(self) -> &'static str {
        match self {
            LinkKind::LeadsTo => "führt weiter",
            LinkKind::BelongsTo => "gehört zu",
            LinkKind::ExampleOf => "Beispiel für",
            LinkKind::Contradicts => "widerspricht",
        }
    }

    /// `true` only for „widerspricht“ (undirected, stored with `from < to`).
    pub fn symmetric(self) -> bool {
        self == LinkKind::Contradicts
    }
}

/// A link as RPC, MCP and the map show it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct IdeaLink {
    pub id: IdeaLinkId,
    pub uuid: String,
    pub from: IdeaId,
    pub to: IdeaId,
    pub kind: LinkKind,
    pub note: String,
    pub origin: IdeaOrigin,
    pub created: String,
    pub updated: String,
    /// Point in time of the (soft) deletion; `None` = active.
    pub deleted: Option<String>,
}

/// New link ([`Store::idea_link_create`]).
#[derive(Debug, Clone)]
pub struct NewIdeaLink<'a> {
    pub from: IdeaId,
    pub to: IdeaId,
    pub kind: LinkKind,
    pub note: &'a str,
    pub origin: IdeaOrigin,
}

/// Stored position of an idea on the map of a work.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MapPos {
    pub idea: IdeaId,
    pub x: f64,
    pub y: f64,
    pub updated: String,
}

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";

fn refuse(message: impl Into<String>) -> StoreError {
    StoreError::Idea(message.into())
}

/// Check the note: CRLF → LF, edges trimmed, no control characters except `\n`, at most [`MAX_LINK_NOTE_CHARS`]
/// characters.
fn clean_note(note: &str) -> Result<String, StoreError> {
    let note = note.replace("\r\n", "\n").trim().to_owned();
    if note.chars().any(|c| c.is_control() && c != '\n') {
        return Err(refuse("Die Notiz einer Verbindung darf keine Steuerzeichen enthalten (außer Zeilenumbruch)."));
    }
    let chars = note.chars().count();
    if chars > MAX_LINK_NOTE_CHARS {
        return Err(refuse(format!(
            "Die Notiz einer Verbindung ist zu lang (höchstens {MAX_LINK_NOTE_CHARS} Zeichen, hier {chars})."
        )));
    }
    Ok(note)
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    uuid: String,
    from_id: i64,
    to_id: i64,
    kind: String,
    note: String,
    origin_kind: String,
    origin_name: String,
    created: String,
    updated: String,
    deleted: Option<String>,
}

const SELECT: &str = "SELECT l.id, coalesce(l.uuid, '') AS uuid, l.from_id, l.to_id, l.kind, l.note, l.origin_kind, \
     l.origin_name, l.created, l.updated, l.deleted FROM idea_link l";

/// Condition "active and both ideas not deleted" (through `l`, `a` = from, `b` = to).
const ACTIVE: &str = "l.deleted IS NULL AND a.deleted IS NULL AND b.deleted IS NULL";

fn to_link(row: Row) -> Result<IdeaLink, StoreError> {
    let kind =
        LinkKind::parse(&row.kind).ok_or_else(|| broken(format!("unbekannte Art einer Verbindung {:?}", row.kind)))?;
    let origin = match row.origin_kind.as_str() {
        "human" => IdeaOrigin::Human { name: row.origin_name },
        "persona" => IdeaOrigin::Persona { handle: row.origin_name },
        "mcp" => IdeaOrigin::Mcp { handle: row.origin_name },
        other => return Err(broken(format!("unbekannte Herkunft einer Verbindung {other:?}"))),
    };
    Ok(IdeaLink {
        id: row.id,
        uuid: row.uuid,
        from: row.from_id,
        to: row.to_id,
        kind,
        note: row.note,
        origin,
        created: row.created,
        updated: row.updated,
        deleted: row.deleted,
    })
}

async fn get(conn: &mut SqliteConnection, id: IdeaLinkId) -> Result<IdeaLink, StoreError> {
    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE l.id = ?")))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    to_link(row.ok_or_else(|| refuse(format!("Verbindung {id} gibt es nicht.")))?)
}

async fn many(conn: &mut SqliteConnection, sql: &str, binds: &[i64]) -> Result<Vec<IdeaLink>, StoreError> {
    let mut query = sqlx::query_as::<_, Row>(sqlx::AssertSqlSafe(sql));
    for value in binds {
        query = query.bind(*value);
    }
    query.fetch_all(&mut *conn).await?.into_iter().map(to_link).collect()
}

/// Does the work exist? Otherwise `UnknownWork`.
async fn check_work(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Both ideas may be linked: existing, not deleted, the same work (also both without one).
async fn check_pair(conn: &mut SqliteConnection, from: IdeaId, to: IdeaId) -> Result<(), StoreError> {
    if from == to {
        return Err(refuse("Ein Einfall lässt sich nicht mit sich selbst verbinden."));
    }
    let a = crate::ideas::get(conn, from).await?;
    let b = crate::ideas::get(conn, to).await?;
    for idea in [&a, &b] {
        if idea.deleted.is_some() {
            return Err(refuse(format!("Einfall {} ist gelöscht; erst wiederherstellen.", idea.id)));
        }
    }
    if a.work != b.work {
        return Err(refuse(format!(
            "Einfall {from} und Einfall {to} gehören nicht zum selben Werk; verbinden lassen sich nur Einfälle eines Werks."
        )));
    }
    Ok(())
}

/// Lines „- Verbindungen: …“ for `einfaelle.md`: per idea of the scope (work or `None` = without a work) the active
/// links to ideas that are not deleted, by link id: outgoing „→ 79 führt weiter“, incoming „← 12 Beispiel für“,
/// undirected „↔ 48 widerspricht“, with a note „ („Notiz“)“ (line breaks of the note as spaces); ideas without a link
/// are missing.
pub(crate) async fn mirror_lines(
    conn: &mut SqliteConnection,
    work: Option<WorkId>,
) -> Result<BTreeMap<IdeaId, String>, StoreError> {
    let sql = format!(
        "{SELECT} JOIN idea a ON a.id = l.from_id JOIN idea b ON b.id = l.to_id \
         WHERE {ACTIVE} AND (a.work_id IS ? OR b.work_id IS ?) ORDER BY l.id"
    );
    let links: Vec<IdeaLink> = sqlx::query_as::<_, Row>(sqlx::AssertSqlSafe(sql.as_str()))
        .bind(work)
        .bind(work)
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(to_link)
        .collect::<Result<_, _>>()?;
    let mut parts: BTreeMap<IdeaId, Vec<String>> = BTreeMap::new();
    for link in &links {
        let note = if link.note.is_empty() {
            String::new()
        } else {
            format!(" („{}“)", link.note.split('\n').collect::<Vec<_>>().join(" "))
        };
        let label = link.kind.label();
        let (out, back) = if link.kind.symmetric() { ("↔", "↔") } else { ("→", "←") };
        parts.entry(link.from).or_default().push(format!("{out} {} {label}{note}", link.to));
        parts.entry(link.to).or_default().push(format!("{back} {} {label}{note}", link.from));
    }
    Ok(parts.into_iter().map(|(idea, list)| (idea, list.join("; "))).collect())
}

/// Create a link in the transaction of the caller (also for `Store::idea_import_tree`, Round 34): like
/// [`Store::idea_link_create`]; `false` = it already existed unchanged.
pub(crate) async fn create_in(
    conn: &mut SqliteConnection,
    new: &NewIdeaLink<'_>,
) -> Result<(IdeaLink, bool), StoreError> {
    let note = clean_note(new.note)?;
    if new.origin.name().trim().is_empty() {
        return Err(refuse("Die Herkunft einer Verbindung braucht einen Namen."));
    }
    let (from, to) = if new.kind.symmetric() && new.from > new.to { (new.to, new.from) } else { (new.from, new.to) };
    check_pair(conn, from, to).await?;
    let existing: Option<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, deleted FROM idea_link WHERE from_id = ? AND to_id = ? AND kind = ?")
            .bind(from)
            .bind(to)
            .bind(new.kind.as_str())
            .fetch_optional(&mut *conn)
            .await?;
    let (id, changed) = match existing {
        Some((id, None)) => (id, false),
        Some((id, Some(_))) => {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE idea_link SET deleted = NULL, note = CASE WHEN ? = '' THEN note ELSE ? END, \
                 updated = {NOW} WHERE id = ?"
            )))
            .bind(&note)
            .bind(&note)
            .bind(id)
            .execute(&mut *conn)
            .await?;
            (id, true)
        }
        None => {
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO idea_link (from_id, to_id, kind, note, origin_kind, origin_name) \
                 VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
            )
            .bind(from)
            .bind(to)
            .bind(new.kind.as_str())
            .bind(&note)
            .bind(new.origin.kind())
            .bind(new.origin.name())
            .fetch_one(&mut *conn)
            .await?;
            (id, true)
        }
    };
    Ok((get(conn, id).await?, changed))
}

impl Store {
    /// (link, changed). See contract §2: an existing active pair+kind → unchanged (`false`), a soft-deleted one →
    /// restored (`true`, note replaced if a new one comes along). „widerspricht“ is stored with `from < to`.
    /// Self-reference, foreign work, deleted idea, note too long → [`StoreError::Idea`], unknown idea →
    /// [`StoreError::UnknownIdea`]; nothing written.
    pub async fn idea_link_create(&self, new: &NewIdeaLink<'_>) -> Result<(IdeaLink, bool), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (link, changed) = create_in(&mut tx, new).await?;
        if changed {
            tx.commit().await?;
        }
        Ok((link, changed))
    }

    /// A link (also a deleted one); unknown → [`StoreError::Idea`].
    pub async fn idea_link_get(&self, id: IdeaLinkId) -> Result<IdeaLink, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, id).await
    }

    /// Soft-delete; already deleted → [`StoreError::Idea`].
    pub async fn idea_link_delete(&self, id: IdeaLinkId) -> Result<IdeaLink, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        if before.deleted.is_some() {
            return Err(refuse(format!("Verbindung {id} ist schon gelöscht.")));
        }
        sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE idea_link SET deleted = {NOW}, updated = {NOW} WHERE id = ?")))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let link = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(link)
    }

    /// Restore; not deleted → [`StoreError::Idea`]. Both ideas must be linkable as on creation (not deleted, the same
    /// work), otherwise [`StoreError::Idea`].
    pub async fn idea_link_restore(&self, id: IdeaLinkId) -> Result<IdeaLink, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        if before.deleted.is_none() {
            return Err(refuse(format!("Verbindung {id} ist nicht gelöscht.")));
        }
        check_pair(&mut tx, before.from, before.to).await?;
        sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE idea_link SET deleted = NULL, updated = {NOW} WHERE id = ?")))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let link = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(link)
    }

    /// `deleted = false`: only active links whose two ideas are not deleted; `true`: all. By id. Unknown idea →
    /// [`StoreError::UnknownIdea`].
    pub async fn idea_links_of_idea(&self, idea: IdeaId, deleted: bool) -> Result<Vec<IdeaLink>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        crate::ideas::get(&mut conn, idea).await?;
        let filter = if deleted { "1" } else { ACTIVE };
        let sql = format!(
            "{SELECT} JOIN idea a ON a.id = l.from_id JOIN idea b ON b.id = l.to_id \
             WHERE (l.from_id = ? OR l.to_id = ?) AND {filter} ORDER BY l.id"
        );
        many(&mut conn, &sql, &[idea, idea]).await
    }

    /// Links whose two ideas have `work_id = work`; `deleted` as above; by id. Unknown work →
    /// [`StoreError::UnknownWork`].
    pub async fn idea_links_of_work(&self, work: WorkId, deleted: bool) -> Result<Vec<IdeaLink>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let filter = if deleted { "1" } else { ACTIVE };
        let sql = format!(
            "{SELECT} JOIN idea a ON a.id = l.from_id JOIN idea b ON b.id = l.to_id \
             WHERE a.work_id = ? AND b.work_id = ? AND {filter} ORDER BY l.id"
        );
        many(&mut conn, &sql, &[work, work]).await
    }

    /// Positions of the ideas that are not deleted and still belong to the work; by idea. Unknown work →
    /// [`StoreError::UnknownWork`].
    pub async fn idea_map_positions(&self, work: WorkId) -> Result<Vec<MapPos>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let rows: Vec<(i64, f64, f64, String)> = sqlx::query_as(
            "SELECT p.idea_id, p.x, p.y, p.updated FROM idea_map_pos p JOIN idea i ON i.id = p.idea_id \
             WHERE p.work_id = ? AND i.work_id = p.work_id AND i.deleted IS NULL ORDER BY p.idea_id",
        )
        .bind(work)
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows.into_iter().map(|(idea, x, y, updated)| MapPos { idea, x, y, updated }).collect())
    }

    /// The idea must belong to the work and not be deleted; x, y finite, |v| ≤ [`MAP_COORD_LIMIT`]; otherwise
    /// [`StoreError::Idea`] (unknown idea [`StoreError::UnknownIdea`]). Replaces an existing position (its uuid stays).
    pub async fn idea_map_pos_set(&self, work: WorkId, idea: IdeaId, x: f64, y: f64) -> Result<MapPos, StoreError> {
        for (name, value) in [("x", x), ("y", y)] {
            if !value.is_finite() || value.abs() > MAP_COORD_LIMIT {
                return Err(refuse(format!(
                    "Lage: {name} muss eine endliche Zahl zwischen -{MAP_COORD_LIMIT} und {MAP_COORD_LIMIT} sein."
                )));
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let found = crate::ideas::get(&mut tx, idea).await?;
        if found.deleted.is_some() {
            return Err(refuse(format!("Einfall {idea} ist gelöscht; erst wiederherstellen.")));
        }
        if found.work != Some(work) {
            return Err(refuse(format!("Einfall {idea} gehört nicht zu Werk {work}.")));
        }
        let (x, y, updated): (f64, f64, String) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES (?, ?, ?, ?) \
             ON CONFLICT (work_id, idea_id) DO UPDATE SET x = excluded.x, y = excluded.y, updated = {NOW} \
             RETURNING x, y, updated"
        )))
        .bind(work)
        .bind(idea)
        .bind(x)
        .bind(y)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(MapPos { idea, x, y, updated })
    }

    /// Remove all positions of the work (`idea` = `None`) or one (hard, tombstone `idea_map_pos`); number of removed
    /// ones. Unknown work → [`StoreError::UnknownWork`].
    pub async fn idea_map_pos_reset(&self, work: WorkId, idea: Option<IdeaId>) -> Result<u64, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        check_work(&mut tx, work).await?;
        let done = match idea {
            None => sqlx::query("DELETE FROM idea_map_pos WHERE work_id = ?").bind(work).execute(&mut *tx).await?,
            Some(idea) => {
                sqlx::query("DELETE FROM idea_map_pos WHERE work_id = ? AND idea_id = ?")
                    .bind(work)
                    .bind(idea)
                    .execute(&mut *tx)
                    .await?
            }
        };
        tx.commit().await?;
        Ok(done.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_parses_back_from_its_word_and_only_contradicts_is_symmetric() {
        for kind in LinkKind::ALL {
            // Database, RPC and MCP use the same word; a word that does not lead back would lose the kind.
            assert_eq!(LinkKind::parse(kind.as_str()), Some(kind), "{kind:?}");
            // Contract §2: only „widerspricht“ is undirected (the store normalises from < to).
            assert_eq!(kind.symmetric(), kind == LinkKind::Contradicts, "{kind:?}");
        }
        // Only exactly the words: display or XML form are not database values.
        assert_eq!(LinkKind::parse("widerspricht"), None, "Anzeige ist kein Datenbankwert");
        assert_eq!(LinkKind::parse("Leads_To"), None, "Groß/klein zählt");
    }

    #[test]
    fn the_serde_word_matches_as_str() {
        // JSON of RPC/MCP must carry the same words as the column `kind` (CHECK in 0043).
        for kind in LinkKind::ALL {
            assert_eq!(serde_json::to_value(kind).unwrap(), serde_json::json!(kind.as_str()), "{kind:?}");
        }
    }

    #[test]
    fn notes_are_trimmed_limited_and_free_of_control_characters() {
        // CRLF from forms becomes LF, edges are dropped – that way the mirror compares equal notes as equal.
        assert_eq!(clean_note("  a\r\nb  ").unwrap(), "a\nb", "CRLF → LF, getrimmt");
        // Contract §2: without control characters except \n (a tab or NUL would break the mirror line and XML).
        assert!(clean_note("a\tb").is_err(), "Tabulator abgelehnt");
        assert!(clean_note("a\u{0}b").is_err(), "NUL abgelehnt");
        // Limit in characters, not bytes (CHECK length(note) <= 500 counts characters in SQLite as well).
        assert!(clean_note(&"ä".repeat(MAX_LINK_NOTE_CHARS)).is_ok(), "500 Zeichen gehen");
        assert!(clean_note(&"a".repeat(MAX_LINK_NOTE_CHARS + 1)).is_err(), "501 Zeichen abgelehnt");
    }
}
