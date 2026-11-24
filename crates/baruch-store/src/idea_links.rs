// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gedankenkarte (Vertrag Runde 25, Migration `0043_idea_map.sql`): Verbindungen zwischen Einfällen
//! (`idea_link`) und gespeicherte Lagen der Einfälle auf der Karte eines Werks (`idea_map_pos`).
//!
//! Regeln (Vertrag §2):
//! - Verbindung nur Einfall ↔ Einfall desselben Werks (auch beide ohne Werk), keiner gelöscht, kein
//!   Selbstbezug, Notiz höchstens [`MAX_LINK_NOTE_CHARS`] Zeichen ohne Steuerzeichen außer `\n`.
//! - Arten [`LinkKind`]; „widerspricht“ ist ungerichtet, der Store tauscht so, dass `from < to`.
//! - Verbindungen werden nur weich gelöscht (`deleted`), wiederherstellbar. Ein gelöschter Einfall nimmt
//!   seine Verbindungen und Lage aus allen Listen (Filter, kein Schreiben).
//! - Lagen: je (Werk, Einfall) höchstens eine, Koordinaten endlich und |v| ≤ [`MAP_COORD_LIMIT`]; reine
//!   Anzeige, kein Spiegel-Commit.
//! - Fehler über [`StoreError::Idea`] (→ -32602 wie bei Einfällen), kein eigener Fehlerwert; unbekannte
//!   Einfälle [`StoreError::UnknownIdea`], unbekannte Werke [`StoreError::UnknownWork`].
//! - Jeder Schreibvorgang ist genau eine Transaktion (`BEGIN IMMEDIATE`); erst alles prüfen, dann schreiben.
//!   Die Spalte `uuid` setzt nie dieser Code, sondern der Fülltrigger (Regel aus 0041).

use std::collections::BTreeMap;

use sqlx::SqliteConnection;

use crate::{IdeaId, IdeaOrigin, Store, StoreError, WorkId, broken};

/// id einer Verbindung (`idea_link.id`).
pub type IdeaLinkId = i64;

/// Höchstlänge der Notiz einer Verbindung in Zeichen.
pub const MAX_LINK_NOTE_CHARS: usize = 500;

/// Größter Betrag einer Koordinate einer Lage (SVG-Benutzereinheiten der Karte).
pub const MAP_COORD_LIMIT: f64 = 1_000_000.0;

/// Art einer Verbindung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// „führt weiter“
    LeadsTo,
    /// „gehört zu“
    BelongsTo,
    /// „Beispiel für“
    ExampleOf,
    /// „widerspricht“ (ungerichtet)
    Contradicts,
}

impl LinkKind {
    pub const ALL: [LinkKind; 4] = [LinkKind::LeadsTo, LinkKind::BelongsTo, LinkKind::ExampleOf, LinkKind::Contradicts];

    /// Wert in der Datenbank und in RPC/MCP.
    pub fn as_str(self) -> &'static str {
        match self {
            LinkKind::LeadsTo => "leads_to",
            LinkKind::BelongsTo => "belongs_to",
            LinkKind::ExampleOf => "example_of",
            LinkKind::Contradicts => "contradicts",
        }
    }

    /// Genau die Wörter aus [`LinkKind::as_str`]; sonst `None`.
    pub fn parse(s: &str) -> Option<LinkKind> {
        LinkKind::ALL.into_iter().find(|kind| kind.as_str() == s)
    }

    /// Anzeige (deutsch).
    pub fn label(self) -> &'static str {
        match self {
            LinkKind::LeadsTo => "führt weiter",
            LinkKind::BelongsTo => "gehört zu",
            LinkKind::ExampleOf => "Beispiel für",
            LinkKind::Contradicts => "widerspricht",
        }
    }

    /// `true` nur für „widerspricht“ (ungerichtet, gespeichert mit `from < to`).
    pub fn symmetric(self) -> bool {
        self == LinkKind::Contradicts
    }
}

/// Eine Verbindung, wie RPC, MCP und Karte sie zeigen.
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
    /// Zeitpunkt des (weichen) Löschens; `None` = aktiv.
    pub deleted: Option<String>,
}

/// Neue Verbindung ([`Store::idea_link_create`]).
#[derive(Debug, Clone)]
pub struct NewIdeaLink<'a> {
    pub from: IdeaId,
    pub to: IdeaId,
    pub kind: LinkKind,
    pub note: &'a str,
    pub origin: IdeaOrigin,
}

/// Gespeicherte Lage eines Einfalls auf der Karte eines Werks.
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

/// Notiz prüfen: CRLF → LF, Ränder getrimmt, keine Steuerzeichen außer `\n`, höchstens
/// [`MAX_LINK_NOTE_CHARS`] Zeichen.
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

/// Bedingung „aktiv und beide Einfälle nicht gelöscht“ (über `l`, `a` = from, `b` = to).
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
    let row: Option<Row> =
        sqlx::query_as(&format!("{SELECT} WHERE l.id = ?")).bind(id).fetch_optional(&mut *conn).await?;
    to_link(row.ok_or_else(|| refuse(format!("Verbindung {id} gibt es nicht.")))?)
}

async fn many(conn: &mut SqliteConnection, sql: &str, binds: &[i64]) -> Result<Vec<IdeaLink>, StoreError> {
    let mut query = sqlx::query_as::<_, Row>(sql);
    for value in binds {
        query = query.bind(*value);
    }
    query.fetch_all(&mut *conn).await?.into_iter().map(to_link).collect()
}

/// Gibt es das Werk? Sonst `UnknownWork`.
async fn check_work(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Beide Einfälle dürfen verbunden werden: vorhanden, nicht gelöscht, dasselbe Werk (auch beide ohne).
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

/// Zeilen „- Verbindungen: …“ für `einfaelle.md`: je Einfall des Bereichs (Werk bzw. `None` = ohne Werk) die
/// aktiven Verbindungen zu nicht gelöschten Einfällen, nach Verbindungs-id: ausgehend „→ 79 führt weiter“,
/// eingehend „← 12 Beispiel für“, ungerichtet „↔ 48 widerspricht“, mit Notiz „ („Notiz“)“ (Zeilenumbrüche der
/// Notiz als Leerzeichen); Einfälle ohne Verbindung fehlen.
pub(crate) async fn mirror_lines(
    conn: &mut SqliteConnection,
    work: Option<WorkId>,
) -> Result<BTreeMap<IdeaId, String>, StoreError> {
    let sql = format!(
        "{SELECT} JOIN idea a ON a.id = l.from_id JOIN idea b ON b.id = l.to_id \
         WHERE {ACTIVE} AND (a.work_id IS ? OR b.work_id IS ?) ORDER BY l.id"
    );
    let links: Vec<IdeaLink> = sqlx::query_as::<_, Row>(&sql)
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

impl Store {
    /// (Verbindung, geändert). Siehe Vertrag §2: vorhandenes aktives Paar+Art → unverändert
    /// (`false`), weich gelöschtes → wiederhergestellt (`true`, Notiz ersetzt, wenn eine neue mitkommt).
    /// „widerspricht“ wird mit `from < to` gespeichert. Selbstbezug, fremdes Werk, gelöschter Einfall, zu
    /// lange Notiz → [`StoreError::Idea`], unbekannter Einfall → [`StoreError::UnknownIdea`]; nichts geschrieben.
    pub async fn idea_link_create(&self, new: &NewIdeaLink<'_>) -> Result<(IdeaLink, bool), StoreError> {
        let note = clean_note(new.note)?;
        if new.origin.name().trim().is_empty() {
            return Err(refuse("Die Herkunft einer Verbindung braucht einen Namen."));
        }
        let (from, to) =
            if new.kind.symmetric() && new.from > new.to { (new.to, new.from) } else { (new.from, new.to) };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        check_pair(&mut tx, from, to).await?;
        let existing: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT id, deleted FROM idea_link WHERE from_id = ? AND to_id = ? AND kind = ?")
                .bind(from)
                .bind(to)
                .bind(new.kind.as_str())
                .fetch_optional(&mut *tx)
                .await?;
        let (id, changed) = match existing {
            Some((id, None)) => (id, false),
            Some((id, Some(_))) => {
                sqlx::query(&format!(
                    "UPDATE idea_link SET deleted = NULL, note = CASE WHEN ? = '' THEN note ELSE ? END, \
                     updated = {NOW} WHERE id = ?"
                ))
                .bind(&note)
                .bind(&note)
                .bind(id)
                .execute(&mut *tx)
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
                .fetch_one(&mut *tx)
                .await?;
                (id, true)
            }
        };
        let link = get(&mut tx, id).await?;
        if changed {
            tx.commit().await?;
        }
        Ok((link, changed))
    }

    /// Eine Verbindung (auch eine gelöschte); unbekannt → [`StoreError::Idea`].
    pub async fn idea_link_get(&self, id: IdeaLinkId) -> Result<IdeaLink, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, id).await
    }

    /// Weich löschen; schon gelöscht → [`StoreError::Idea`].
    pub async fn idea_link_delete(&self, id: IdeaLinkId) -> Result<IdeaLink, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        if before.deleted.is_some() {
            return Err(refuse(format!("Verbindung {id} ist schon gelöscht.")));
        }
        sqlx::query(&format!("UPDATE idea_link SET deleted = {NOW}, updated = {NOW} WHERE id = ?"))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let link = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(link)
    }

    /// Wiederherstellen; nicht gelöscht → [`StoreError::Idea`]. Beide Einfälle müssen wie beim Anlegen
    /// verbindbar sein (nicht gelöscht, dasselbe Werk), sonst [`StoreError::Idea`].
    pub async fn idea_link_restore(&self, id: IdeaLinkId) -> Result<IdeaLink, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        if before.deleted.is_none() {
            return Err(refuse(format!("Verbindung {id} ist nicht gelöscht.")));
        }
        check_pair(&mut tx, before.from, before.to).await?;
        sqlx::query(&format!("UPDATE idea_link SET deleted = NULL, updated = {NOW} WHERE id = ?"))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let link = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(link)
    }

    /// `deleted = false`: nur aktive Verbindungen, deren beide Einfälle nicht gelöscht sind; `true`: alle.
    /// Nach id. Unbekannter Einfall → [`StoreError::UnknownIdea`].
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

    /// Verbindungen, deren beide Einfälle `work_id = work` haben; `deleted` wie oben; nach id. Unbekanntes
    /// Werk → [`StoreError::UnknownWork`].
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

    /// Lagen der nicht gelöschten Einfälle, die noch zum Werk gehören; nach Einfall. Unbekanntes Werk →
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

    /// Einfall muss zum Werk gehören und nicht gelöscht sein; x, y endlich, |v| ≤ [`MAP_COORD_LIMIT`];
    /// sonst [`StoreError::Idea`] (unbekannter Einfall [`StoreError::UnknownIdea`]). Ersetzt eine vorhandene
    /// Lage (ihre uuid bleibt).
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
        let (x, y, updated): (f64, f64, String) = sqlx::query_as(&format!(
            "INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES (?, ?, ?, ?) \
             ON CONFLICT (work_id, idea_id) DO UPDATE SET x = excluded.x, y = excluded.y, updated = {NOW} \
             RETURNING x, y, updated"
        ))
        .bind(work)
        .bind(idea)
        .bind(x)
        .bind(y)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(MapPos { idea, x, y, updated })
    }

    /// Alle Lagen des Werks (`idea` = `None`) oder eine entfernen (hart, Grabstein `idea_map_pos`); Zahl der
    /// entfernten. Unbekanntes Werk → [`StoreError::UnknownWork`].
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
            // Datenbank, RPC und MCP benutzen dasselbe Wort; ein Wort, das nicht zurückführt, verlöre die Art.
            assert_eq!(LinkKind::parse(kind.as_str()), Some(kind), "{kind:?}");
            // Vertrag §2: nur „widerspricht“ ist ungerichtet (Store normalisiert from < to).
            assert_eq!(kind.symmetric(), kind == LinkKind::Contradicts, "{kind:?}");
        }
        // Nur genau die Wörter: Anzeige oder XML-Form sind keine Datenbankwerte.
        assert_eq!(LinkKind::parse("widerspricht"), None, "Anzeige ist kein Datenbankwert");
        assert_eq!(LinkKind::parse("Leads_To"), None, "Groß/klein zählt");
    }

    #[test]
    fn the_serde_word_matches_as_str() {
        // JSON von RPC/MCP muss dieselben Wörter tragen wie die Spalte `kind` (CHECK in 0043).
        for kind in LinkKind::ALL {
            assert_eq!(serde_json::to_value(kind).unwrap(), serde_json::json!(kind.as_str()), "{kind:?}");
        }
    }

    #[test]
    fn notes_are_trimmed_limited_and_free_of_control_characters() {
        // CRLF aus Formularen wird LF, Ränder fallen weg – so vergleicht der Spiegel gleiche Notizen gleich.
        assert_eq!(clean_note("  a\r\nb  ").unwrap(), "a\nb", "CRLF → LF, getrimmt");
        // Vertrag §2: ohne Steuerzeichen außer \n (ein Tabulator oder NUL bräche Spiegelzeile und XML).
        assert!(clean_note("a\tb").is_err(), "Tabulator abgelehnt");
        assert!(clean_note("a\u{0}b").is_err(), "NUL abgelehnt");
        // Grenze in Zeichen, nicht Bytes (CHECK length(note) <= 500 zählt in SQLite ebenfalls Zeichen).
        assert!(clean_note(&"ä".repeat(MAX_LINK_NOTE_CHARS)).is_ok(), "500 Zeichen gehen");
        assert!(clean_note(&"a".repeat(MAX_LINK_NOTE_CHARS + 1)).is_err(), "501 Zeichen abgelehnt");
    }
}
