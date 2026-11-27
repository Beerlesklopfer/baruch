// SPDX-License-Identifier: AGPL-3.0-or-later
//! Work-wide replacing and undo (Round 30a, migration 0047).
//!
//! A replacement creates a new revision by the human per section (only if the current revision is still the base) and
//! logs everything in one row `replace_batch` (local: no uuid, not in the exchange format). „Rückgängig“ (undo)
//! switches back per section as long as the new revision is still current, and sets `undone`.
//!
//! The new text per section is computed by the server (`search/`); the store only checks base, heading and limits.
//! Every section runs under a savepoint of its own: if its heading no longer fits, only it is rolled back and skipped
//! with a reason, everything else stays in the same transaction.

use sqlx::SqliteConnection;

use crate::{Change, Store, StoreError, WorkId, write};

/// A section of a replacement: fixed key, revision before, new revision.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReplaceItem {
    pub key: i64,
    pub prev_rev: u32,
    pub new_rev: u32,
}

/// A logged replacement (`replace_batch`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReplaceBatch {
    pub id: i64,
    pub work: WorkId,
    pub created: String,
    pub query: String,
    pub replacement: String,
    pub items: Vec<ReplaceItem>,
    /// Point in time of the undo; `None` = still valid.
    pub undone: Option<String>,
}

/// A skipped section with its reason (German).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReplaceSkip {
    pub key: i64,
    pub reason: String,
}

/// New text of a section on the base `base_rev`.
#[derive(Debug, Clone, Copy)]
pub struct ReplaceEdit<'a> {
    pub key: i64,
    pub base_rev: u32,
    pub body: &'a str,
}

/// At most this many sections per replacement (contract §3.1).
pub const MAX_REPLACE_SECTIONS: usize = 500;
/// Maximum length of the search text in characters (contract §3.1, CHECK in migration 0047).
pub const MAX_QUERY_CHARS: usize = 500;
/// Maximum length of the replacement text in characters.
pub const MAX_REPLACEMENT_CHARS: usize = 2000;

/// Unknown work → `UnknownWork`.
async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Section (seq) for the fixed key `key` in the work; `None` = does not exist (any longer).
async fn seq_of(conn: &mut SqliteConnection, work: WorkId, key: i64) -> Result<Option<usize>, StoreError> {
    let seq: Option<i64> = sqlx::query_scalar("SELECT seq FROM node WHERE id = ? AND work_id = ?")
        .bind(key)
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    seq.map(|s| crate::to_usize(s, "seq")).transpose()
}

/// Number of the current revision of the section `node`; `None` without revisions.
async fn current_number(conn: &mut SqliteConnection, work: WorkId, node: usize) -> Result<Option<u32>, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    Ok(write::current(conn, node_id).await?.map(|(number, _, _)| number))
}

/// Set a savepoint for a section.
async fn savepoint(conn: &mut SqliteConnection) -> Result<(), StoreError> {
    sqlx::query("SAVEPOINT replace_step").execute(&mut *conn).await?;
    Ok(())
}

/// Release the savepoint; without `keep` roll back everything since it first.
async fn release(conn: &mut SqliteConnection, keep: bool) -> Result<(), StoreError> {
    if !keep {
        sqlx::query("ROLLBACK TO replace_step").execute(&mut *conn).await?;
    }
    sqlx::query("RELEASE replace_step").execute(&mut *conn).await?;
    Ok(())
}

/// Reason for an error that only skips this section (heading does not fit, title rules, revision gone); other errors
/// (database, broken data) abort everything.
fn skip_reason(error: &StoreError) -> Option<String> {
    match error {
        StoreError::Heading(m) => Some(m.clone()),
        StoreError::Title(_) | StoreError::UnknownRevision { .. } | StoreError::Refused(_) => Some(error.to_string()),
        _ => None,
    }
}

impl Store {
    /// One transaction: per section a new revision by the human `human` on `base_rev` (only if the current revision is
    /// still `base_rev`, otherwise `ReplaceSkip`), headings as when saving, one `replace_batch`.
    ///
    /// Skipped (with a German reason) are: unknown keys, sections whose current revision is no longer `base_rev`,
    /// unchanged texts and texts whose heading no longer fits the outline item (another level, no heading or further
    /// headings – the check of [`Store::save_human_on`]); a changed title of the same level renames the item. `query`
    /// 1–500 characters, `replacement` at most 2000, at most 500 sections, each at most once → otherwise `Replace`
    /// without a change. Even if everything is skipped, an (empty) row `replace_batch` is created.
    pub async fn replace_apply(
        &self,
        work: WorkId,
        query: &str,
        replacement: &str,
        options: &serde_json::Value,
        edits: &[ReplaceEdit<'_>],
        human: &str,
    ) -> Result<(ReplaceBatch, Vec<Change>, Vec<ReplaceSkip>), StoreError> {
        let chars = query.chars().count();
        if chars == 0 || chars > MAX_QUERY_CHARS {
            return Err(StoreError::Replace(format!(
                "Der Suchtext braucht 1 bis {MAX_QUERY_CHARS} Zeichen (hier {chars})."
            )));
        }
        let chars = replacement.chars().count();
        if chars > MAX_REPLACEMENT_CHARS {
            return Err(StoreError::Replace(format!(
                "Der Ersatz darf höchstens {MAX_REPLACEMENT_CHARS} Zeichen haben (hier {chars})."
            )));
        }
        if edits.len() > MAX_REPLACE_SECTIONS {
            return Err(StoreError::Replace(format!(
                "Höchstens {MAX_REPLACE_SECTIONS} Abschnitte je Ersetzen (hier {}).",
                edits.len()
            )));
        }
        let mut seen = std::collections::HashSet::new();
        if let Some(twice) = edits.iter().find(|e| !seen.insert(e.key)) {
            return Err(StoreError::Replace(format!("Abschnitt {} steht zweimal im Ersetzen.", twice.key)));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        work_exists(&mut tx, work).await?;
        let mut items = Vec::new();
        let mut changes = Vec::new();
        let mut skipped = Vec::new();
        for edit in edits {
            let Some(node) = seq_of(&mut tx, work, edit.key).await? else {
                skipped
                    .push(ReplaceSkip { key: edit.key, reason: "Den Abschnitt gibt es in diesem Werk nicht.".into() });
                continue;
            };
            let now = current_number(&mut tx, work, node).await?;
            if now != Some(edit.base_rev) {
                let reason = match now {
                    Some(n) => format!(
                        "Der Abschnitt wurde inzwischen geändert (aktuell ist Fassung {n}, gesucht wurde in Fassung {}).",
                        edit.base_rev
                    ),
                    None => "Der Abschnitt hat keine aktuelle Fassung.".to_owned(),
                };
                skipped.push(ReplaceSkip { key: edit.key, reason });
                continue;
            }
            savepoint(&mut tx).await?;
            let step = match write::save_human(&mut tx, work, node, edit.body, human, Some(edit.base_rev)).await {
                Ok(Some(change)) => {
                    crate::outline::check_current_heading(&mut tx, work, node).await.map(|_| Some(change))
                }
                other => other,
            };
            match step {
                Ok(Some(change)) => {
                    release(&mut tx, true).await?;
                    items.push(ReplaceItem { key: edit.key, prev_rev: edit.base_rev, new_rev: change.rev });
                    changes.push(change);
                }
                Ok(None) => {
                    release(&mut tx, false).await?;
                    skipped.push(ReplaceSkip { key: edit.key, reason: "Der Text ändert sich nicht.".into() });
                }
                Err(error) => {
                    let Some(reason) = skip_reason(&error) else { return Err(error) };
                    release(&mut tx, false).await?;
                    skipped.push(ReplaceSkip { key: edit.key, reason });
                }
            }
        }
        let items_json = serde_json::to_string(&items).expect("Einträge sind immer JSON");
        let options_json = serde_json::to_string(options).expect("JSON-Wert ist immer JSON");
        let (id, created): (i64, String) = sqlx::query_as(
            "INSERT INTO replace_batch (work_id, query, replacement, options, items) VALUES (?, ?, ?, ?, ?) \
             RETURNING id, created",
        )
        .bind(work)
        .bind(query)
        .bind(replacement)
        .bind(options_json)
        .bind(items_json)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        let batch = ReplaceBatch {
            id,
            work,
            created,
            query: query.to_owned(),
            replacement: replacement.to_owned(),
            items,
            undone: None,
        };
        Ok((batch, changes, skipped))
    }

    /// One transaction: per entry back to `prev_rev`, only if `new_rev` is still current; sets `undone`.
    ///
    /// Back means switching the "current" marker (like `revision.set_current`); the outline follows the heading of the
    /// previous revision. Skipped (with a reason) are sections that no longer exist, whose current revision is no
    /// longer `new_rev` or whose previous revision no longer fits. Already undone → `Replace` without a change.
    pub async fn replace_undo(
        &self,
        work: WorkId,
        batch: i64,
        human: &str,
    ) -> Result<(ReplaceBatch, Vec<ReplaceSkip>), StoreError> {
        let found = self.replace_batch(work, batch).await?;
        if let Some(when) = &found.undone {
            return Err(StoreError::Replace(format!("Dieses Ersetzen ist schon rückgängig gemacht ({when}).")));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut skipped = Vec::new();
        for item in &found.items {
            let Some(node) = seq_of(&mut tx, work, item.key).await? else {
                skipped.push(ReplaceSkip { key: item.key, reason: "Den Abschnitt gibt es nicht mehr.".into() });
                continue;
            };
            let now = current_number(&mut tx, work, node).await?;
            if now != Some(item.new_rev) {
                let reason = match now {
                    Some(n) => format!(
                        "Der Abschnitt wurde nach dem Ersetzen geändert (aktuell ist Fassung {n}, nicht {}).",
                        item.new_rev
                    ),
                    None => "Der Abschnitt hat keine aktuelle Fassung.".to_owned(),
                };
                skipped.push(ReplaceSkip { key: item.key, reason });
                continue;
            }
            savepoint(&mut tx).await?;
            let step = match write::set_current(&mut tx, work, node, item.prev_rev).await {
                Ok(_) => crate::outline::follow_switched(&mut tx, work, node, human).await.map(|_| ()),
                Err(e) => Err(e),
            };
            match step {
                Ok(()) => release(&mut tx, true).await?,
                Err(error) => {
                    let Some(reason) = skip_reason(&error) else { return Err(error) };
                    release(&mut tx, false).await?;
                    skipped.push(ReplaceSkip { key: item.key, reason });
                }
            }
        }
        let undone: String = sqlx::query_scalar(
            "UPDATE replace_batch SET undone = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ? AND work_id = ? \
             RETURNING undone",
        )
        .bind(batch)
        .bind(work)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((ReplaceBatch { undone: Some(undone), ..found }, skipped))
    }

    /// The replacement `batch` of the work; foreign work or unknown → `Replace` with a message, unknown work →
    /// `UnknownWork`.
    pub async fn replace_batch(&self, work: WorkId, batch: i64) -> Result<ReplaceBatch, StoreError> {
        type BatchRow = (i64, i64, String, String, String, String, Option<String>);
        let row: Option<BatchRow> = sqlx::query_as(
            "SELECT id, work_id, created, query, replacement, items, undone FROM replace_batch \
             WHERE id = ? AND work_id = ?",
        )
        .bind(batch)
        .bind(work)
        .fetch_optional(&self.pool)
        .await?;
        let Some((id, work, created, query, replacement, items, undone)) = row else {
            self.work_slug(work).await?;
            return Err(StoreError::Replace(format!("Ersetzen {batch} gibt es in diesem Werk nicht.")));
        };
        let items: Vec<ReplaceItem> = serde_json::from_str(&items)
            .map_err(|e| crate::broken(format!("Ersetzen {id}: items ist keine Liste ({e})")))?;
        Ok(ReplaceBatch { id, work, created, query, replacement, items, undone })
    }

    /// The most recent replacement of the work that is not yet undone (for „Rückgängig“ on `/suche`).
    pub async fn replace_latest(&self, work: WorkId) -> Result<Option<ReplaceBatch>, StoreError> {
        let id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM replace_batch WHERE work_id = ? AND undone IS NULL ORDER BY id DESC LIMIT 1",
        )
        .bind(work)
        .fetch_optional(&self.pool)
        .await?;
        match id {
            Some(id) => self.replace_batch(work, id).await.map(Some),
            None => Ok(None),
        }
    }
}
