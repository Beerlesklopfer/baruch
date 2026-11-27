// SPDX-License-Identifier: AGPL-3.0-or-later
//! Managing posts in the section chat (contract Round 8, section 3b): reading, deleting, moving to another section and
//! converting into an idea.
//!
//! Rules:
//! - Posts are not book text. The human may delete, move and convert every post, MCP clients and personas only their
//!   own (the server checks that, not the store).
//! - Every action is exactly one transaction (`BEGIN IMMEDIATE`); if a step fails, everything stays as it was. When
//!   converting, idea and deletion come about together or not at all.
//! - A post is addressed through its id (`post.id`) and its work; an id from another work is
//!   [`StoreError::UnknownPost`].
//! - Deleted posts are gone (together with `post_ref`); they stay restorable through the entry
//!   [`PostInfo::entry_markdown`], which the server writes into the deletion or conversion commit. The full-text index
//!   is kept up to date by the trigger `post_fts_delete` (migration 0105), when moving by `post_fts_move`, when editing
//!   (Round 28, `post_edit.rs`) by `post_fts_edit` (library 0107).

use baruch_core::TextRef;
use sqlx::SqliteConnection;

use crate::ideas::{self, Idea, IdeaOrigin, MAX_SHORT_CHARS, NewIdea};
use crate::{Store, StoreError, WorkId, to_u32, to_usize, write};

/// A post with everything that managing, the mirror and the user interface need.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PostInfo {
    pub id: i64,
    pub work: WorkId,
    /// Section (seq, now).
    pub node: usize,
    /// Fixed key of the section (`Node::key`).
    pub key: i64,
    /// Position in the section chat.
    pub position: i64,
    /// „Autor“, persona handle (`@lektor`) or MCP client (`@mcp:<name>`).
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    pub refs: Vec<TextRef>,
    pub created: String,
    /// Time of the last change (Round 28, `post.edited`); `None` = never edited.
    pub edited: Option<String>,
}

/// Write a value as `- <label>: <value>`; further lines indented by two spaces, verbatim.
fn push_value(out: &mut String, label: &str, value: &str) {
    out.push_str("- ");
    out.push_str(label);
    out.push_str(": ");
    let mut lines = value.split('\n');
    out.push_str(lines.next().unwrap_or_default());
    out.push('\n');
    for line in lines {
        if !line.is_empty() {
            out.push_str("  ");
            out.push_str(line);
        }
        out.push('\n');
    }
}

impl PostInfo {
    /// The complete post as Markdown for the body of the deletion or conversion commit: heading „## Beitrag <id>“,
    /// author, time, section (seq and key), position, one line per source and per reference, then the text verbatim
    /// (nothing escaped), so that it can be restored.
    pub fn entry_markdown(&self) -> String {
        let mut out = format!("## Beitrag {}\n\n", self.id);
        push_value(&mut out, "Autor", &self.author);
        push_value(&mut out, "Zeit", &self.created);
        push_value(&mut out, "Abschnitt", &format!("{} (key {}), Stelle {}", self.node, self.key, self.position));
        for source in &self.sources {
            push_value(&mut out, "Quelle", source);
        }
        for r in &self.refs {
            push_value(&mut out, "Verweis", &format!("Abschnitt {} Fassung {}: „{}“", r.node, r.rev, r.quote));
        }
        out.push('\n');
        out.push_str(&self.body);
        if !self.body.ends_with('\n') {
            out.push('\n');
        }
        out
    }
}

type InfoRow = (i64, i64, i64, i64, i64, String, String, String, String, Option<String>);

/// Post `id` in the work `work`, read in the connection or transaction of the caller.
pub(crate) async fn get(conn: &mut SqliteConnection, work: WorkId, id: i64) -> Result<PostInfo, StoreError> {
    let row: Option<InfoRow> = sqlx::query_as(
        "SELECT p.id, n.work_id, n.seq, n.id, p.position, p.author, p.body, p.sources, p.created, p.edited \
         FROM post p JOIN node n ON n.id = p.node_id WHERE p.id = ? AND n.work_id = ?",
    )
    .bind(id)
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((id, work, seq, key, position, author, body, sources, created, edited)) = row else {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
        return Err(match exists {
            Some(_) => StoreError::UnknownPost { work, post: id },
            None => StoreError::UnknownWork(work),
        });
    };
    let sources: Vec<String> = serde_json::from_str(&sources)
        .map_err(|e| crate::broken(format!("Beitrag {id}: Quellen sind kein JSON ({e})")))?;
    let refs: Vec<(i64, i64, String)> =
        sqlx::query_as("SELECT node_seq, rev, quote FROM post_ref WHERE post_id = ? ORDER BY position")
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
    let refs = refs
        .into_iter()
        .map(|(node, rev, quote)| Ok(TextRef { node: to_usize(node, "node_seq")?, rev: to_u32(rev, "rev")?, quote }))
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(PostInfo { id, work, node: to_usize(seq, "seq")?, key, position, author, body, sources, refs, created, edited })
}

/// Delete a post together with its references (in the transaction of the caller).
async fn remove(conn: &mut SqliteConnection, id: i64) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM post_ref WHERE post_id = ?").bind(id).execute(&mut *conn).await?;
    let done = sqlx::query("DELETE FROM post WHERE id = ?").bind(id).execute(&mut *conn).await?;
    if done.rows_affected() != 1 {
        return Err(crate::broken(format!("Beitrag {id} nicht gelöscht")));
    }
    Ok(())
}

/// Origin of an idea from the author of a post: `@mcp:` → MCP client, an existing persona handle → persona, otherwise
/// the human (for example „Autor“, „Notiz“).
async fn origin_of(conn: &mut SqliteConnection, author: &str) -> Result<IdeaOrigin, StoreError> {
    if author.starts_with("@mcp:") {
        return Ok(IdeaOrigin::Mcp { handle: author.to_owned() });
    }
    let persona: Option<String> = sqlx::query_scalar("SELECT handle FROM persona WHERE handle = ?")
        .bind(author)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(match persona {
        Some(handle) => IdeaOrigin::Persona { handle },
        None => IdeaOrigin::Human { name: author.to_owned() },
    })
}

/// Sources of a post as the source of an idea: joined with "; ", shortened to 200 characters.
fn joined_sources(sources: &[String]) -> String {
    let joined = sources.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; ");
    joined.chars().take(MAX_SHORT_CHARS).collect::<String>().trim_end().to_owned()
}

impl Store {
    /// A post of the work. Unknown work → `UnknownWork`, unknown id (or one from another work) → `UnknownPost`.
    pub async fn post_get(&self, work: WorkId, id: i64) -> Result<PostInfo, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, work, id).await
    }

    /// Delete a post (atomic, together with `post_ref`); returns the deleted post (for the mirror commit, which records
    /// it so that it can be restored).
    pub async fn post_delete(&self, work: WorkId, id: i64) -> Result<PostInfo, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let info = get(&mut tx, work, id).await?;
        remove(&mut tx, id).await?;
        tx.commit().await?;
        Ok(info)
    }

    /// Move a post to the end of the section chat of `target` (seq) in the same work (atomic, `position` = largest +
    /// 1). Same section → `Refused`, unknown target → `UnknownNode`. Returns the post at its new place. Round 23: the
    /// findings of the post (`finding.post_id`) move along in the same transaction, except for the adopted ones (see
    /// below).
    pub async fn post_move(&self, work: WorkId, id: i64, target: usize) -> Result<PostInfo, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, work, id).await?;
        let target_id = write::node_id(&mut tx, work, target).await?;
        if target_id == before.key {
            return Err(StoreError::Refused("Der Beitrag steht schon in diesem Abschnitt."));
        }
        let position: i64 = sqlx::query_scalar("SELECT coalesce(max(position) + 1, 0) FROM post WHERE node_id = ?")
            .bind(target_id)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("UPDATE post SET node_id = ?, position = ? WHERE id = ?")
            .bind(target_id)
            .bind(position)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        // Round 23: the findings from the post move along (the same transaction, one commit in the mirror) – otherwise
        // they would keep counting at the old section. Adopted ones are excepted: their status names a revision of the
        // old section (`adopted_revision`) that does not exist at the target; they stay there as evidence.
        sqlx::query(
            "UPDATE finding SET node_id = ?, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
             WHERE post_id = ? AND work_id = ? AND status <> 'adopted'",
        )
        .bind(target_id)
        .bind(id)
        .bind(work)
        .execute(&mut *tx)
        .await?;
        let after = get(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(after)
    }

    /// Convert a post into an idea, in **one** transaction: idea with work, reference = section of the post, thought =
    /// text, source = sources with "; " (at most 200 characters), keywords `words`, origin from the author (`@mcp:` →
    /// MCP, persona handle → persona, otherwise human); afterwards the post is deleted. If the idea fails (no keywords,
    /// text too long …), the post stays, and no idea is created.
    pub async fn post_to_idea(&self, work: WorkId, id: i64, words: &[String]) -> Result<(PostInfo, Idea), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let info = get(&mut tx, work, id).await?;
        let origin = origin_of(&mut tx, &info.author).await?;
        let source = joined_sources(&info.sources);
        let new = NewIdea {
            work: Some(work),
            key: Some(info.key),
            words,
            note: &info.body,
            quote: "",
            source: &source,
            bible: "",
            origin,
        };
        let idea = ideas::create_in(&mut tx, &new).await?;
        remove(&mut tx, id).await?;
        tx.commit().await?;
        Ok((info, idea))
    }

    /// Fixed key (`Node::key`) of the section at the position `seq`; for addresses after a change (`/?work=W&key=K`).
    /// Unknown → `UnknownNode` or `UnknownWork`.
    pub async fn node_key(&self, work: WorkId, seq: usize) -> Result<i64, StoreError> {
        let mut conn = self.pool.acquire().await?;
        write::node_id(&mut conn, work, seq).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_joined_and_cut_at_200_characters() {
        let sources = vec![" Lutherbibel ".to_owned(), String::new(), "Gartenbuch".to_owned()];
        assert_eq!(joined_sources(&sources), "Lutherbibel; Gartenbuch", "getrimmt, leere übergangen, mit „; “");
        let long = vec!["ä".repeat(150), "ö".repeat(150)];
        assert_eq!(joined_sources(&long).chars().count(), 200, "höchstens 200 Zeichen (nicht Bytes)");
    }

    #[test]
    fn the_entry_keeps_the_whole_post_verbatim() {
        let info = PostInfo {
            id: 9,
            work: 1,
            node: 3,
            key: 17,
            position: 2,
            author: "@mcp:probe".into(),
            body: "Erste Zeile\n# keine Überschrift".into(),
            sources: vec!["Gartenbuch, S. 4".into()],
            refs: vec![TextRef { node: 2, rev: 1, quote: "Rosen".into() }],
            created: "2026-11-06T10:00:00.000Z".into(),
            edited: None,
        };
        let entry = info.entry_markdown();
        assert!(entry.starts_with("## Beitrag 9\n\n- Autor: @mcp:probe\n"), "Überschrift und Autor: {entry}");
        assert!(entry.contains("- Abschnitt: 3 (key 17), Stelle 2\n"), "Abschnitt mit Schlüssel: {entry}");
        assert!(entry.contains("- Quelle: Gartenbuch, S. 4\n"), "Quelle: {entry}");
        assert!(entry.contains("- Verweis: Abschnitt 2 Fassung 1: „Rosen“\n"), "Verweis: {entry}");
        assert!(entry.ends_with("\n\nErste Zeile\n# keine Überschrift\n"), "Text wörtlich am Ende: {entry}");
    }
}
