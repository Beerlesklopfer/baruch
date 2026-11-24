// SPDX-License-Identifier: AGPL-3.0-or-later
//! Beiträge im Abschnitts-Chat verwalten (Vertrag Runde 8, Abschnitt 3b): lesen, löschen, in einen
//! anderen Abschnitt verschieben und in einen Einfall umwandeln.
//!
//! Regeln:
//! - Beiträge sind kein Buchtext. Der Mensch darf jeden Beitrag löschen, verschieben und umwandeln,
//!   MCP-Clients und Personas nur ihre eigenen (das prüft der Server, nicht der Store).
//! - Jede Aktion ist genau eine Transaktion (`BEGIN IMMEDIATE`); scheitert ein Schritt, bleibt alles,
//!   wie es war. Beim Umwandeln entstehen Einfall und Löschung zusammen oder gar nicht.
//! - Ein Beitrag wird über seine id (`post.id`) und sein Werk angesprochen; eine id aus einem anderen
//!   Werk ist [`StoreError::UnknownPost`].
//! - Gelöschte Beiträge sind fort (samt `post_ref`); wiederherstellbar bleiben sie über den Eintrag
//!   [`PostInfo::entry_markdown`], den der Server in den Lösch- bzw. Umwandlungs-Commit schreibt. Den
//!   Volltextindex hält der Trigger `post_fts_delete` (Migration 0105) aktuell, beim Verschieben
//!   `post_fts_move`, beim Bearbeiten (Runde 28, `post_edit.rs`) `post_fts_edit` (Bibliothek 0107).

use baruch_core::TextRef;
use sqlx::SqliteConnection;

use crate::ideas::{self, Idea, IdeaOrigin, MAX_SHORT_CHARS, NewIdea};
use crate::{Store, StoreError, WorkId, to_u32, to_usize, write};

/// Ein Beitrag mit allem, was Verwalten, Spiegel und Oberfläche brauchen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PostInfo {
    pub id: i64,
    pub work: WorkId,
    /// Abschnitt (seq, jetzt).
    pub node: usize,
    /// Fester Schlüssel des Abschnitts (`Node::key`).
    pub key: i64,
    /// Stelle im Abschnitts-Chat.
    pub position: i64,
    /// „Autor“, Persona-Handle (`@lektor`) oder MCP-Client (`@mcp:<name>`).
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    pub refs: Vec<TextRef>,
    pub created: String,
    /// Zeit der letzten Änderung (Runde 28, `post.edited`); `None` = nie bearbeitet.
    pub edited: Option<String>,
}

/// Einen Wert als `- <label>: <wert>` schreiben; weitere Zeilen zwei Leerzeichen eingerückt, wörtlich.
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
    /// Der vollständige Beitrag als Markdown für den Rumpf des Lösch- bzw. Umwandlungs-Commits:
    /// Überschrift „## Beitrag <id>“, Autor, Zeit, Abschnitt (seq und key), Stelle, je Quelle und je
    /// Verweis eine Zeile, dann der Text wörtlich (nichts maskiert), damit er sich wiederherstellen lässt.
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

/// Beitrag `id` im Werk `work`, gelesen in der Verbindung bzw. Transaktion des Aufrufers.
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

/// Beitrag samt Verweisen löschen (in der Transaktion des Aufrufers).
async fn remove(conn: &mut SqliteConnection, id: i64) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM post_ref WHERE post_id = ?").bind(id).execute(&mut *conn).await?;
    let done = sqlx::query("DELETE FROM post WHERE id = ?").bind(id).execute(&mut *conn).await?;
    if done.rows_affected() != 1 {
        return Err(crate::broken(format!("Beitrag {id} nicht gelöscht")));
    }
    Ok(())
}

/// Herkunft eines Einfalls aus dem Autor eines Beitrags: `@mcp:` → MCP-Client, ein vorhandener
/// Persona-Handle → Persona, sonst der Mensch (z. B. „Autor“, „Notiz“).
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

/// Quellen eines Beitrags als Quelle eines Einfalls: mit „; “ verbunden, auf 200 Zeichen gekürzt.
fn joined_sources(sources: &[String]) -> String {
    let joined = sources.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; ");
    joined.chars().take(MAX_SHORT_CHARS).collect::<String>().trim_end().to_owned()
}

impl Store {
    /// Ein Beitrag des Werks. Unbekanntes Werk → `UnknownWork`, unbekannte id (oder aus einem anderen
    /// Werk) → `UnknownPost`.
    pub async fn post_get(&self, work: WorkId, id: i64) -> Result<PostInfo, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, work, id).await
    }

    /// Beitrag löschen (atomar, samt `post_ref`); gibt den gelöschten Beitrag zurück (für den
    /// Spiegel-Commit, der ihn wiederherstellbar festhält).
    pub async fn post_delete(&self, work: WorkId, id: i64) -> Result<PostInfo, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let info = get(&mut tx, work, id).await?;
        remove(&mut tx, id).await?;
        tx.commit().await?;
        Ok(info)
    }

    /// Beitrag ans Ende des Abschnitts-Chats von `target` (seq) im selben Werk verschieben (atomar,
    /// `position` = größte + 1). Gleicher Abschnitt → `Refused`, unbekanntes Ziel → `UnknownNode`.
    /// Gibt den Beitrag an seinem neuen Ort zurück. Runde 23: Die Befunde des Beitrags (`finding.post_id`) wandern
    /// in derselben Transaktion mit, außer den übernommenen (siehe unten).
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
        // Runde 23: Die Befunde aus dem Beitrag ziehen mit (dieselbe Transaktion, ein Commit im Spiegel) – sonst
        // zählten sie weiter am alten Abschnitt. Ausgenommen sind übernommene: ihr Status nennt eine Fassung des alten
        // Abschnitts (`adopted_revision`), die es am Ziel nicht gibt; sie bleiben dort als Beleg stehen.
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

    /// Beitrag in einen Einfall umwandeln, in **einer** Transaktion: Einfall mit Werk, Bezug = Abschnitt
    /// des Beitrags, Gedanke = Text, Quelle = Quellen mit „; “ (höchstens 200 Zeichen), Schlagwörter
    /// `words`, Herkunft aus dem Autor (`@mcp:` → MCP, Persona-Handle → Persona, sonst Mensch); danach
    /// ist der Beitrag gelöscht. Scheitert der Einfall (keine Schlagwörter, Text zu lang …), bleibt
    /// der Beitrag, und es entsteht kein Einfall.
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

    /// Fester Schlüssel (`Node::key`) des Abschnitts an der Stelle `seq`; für Adressen nach einer
    /// Änderung (`/?work=W&key=K`). Unbekannt → `UnknownNode` bzw. `UnknownWork`.
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
