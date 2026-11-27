// SPDX-License-Identifier: AGPL-3.0-or-later
//! Befunde (Runde 28, Migration 0046; Schnittstelle S1, danach gehört das Modul befunde).
//!
//! Ein Befund ist ein Datensatz aus der Antwort einer Persona: Art ([`FindingKind`]: Lob, Hinweis, kritisch),
//! Status ([`FindingStatus`]: offen, übernommen, verworfen, verschoben, erledigt ohne Änderung), Herkunft (Beitrag im
//! Abschnitts-Chat oder Fadenbeitrag, Stelle in der Antwort), wahlweise Zitat und Prüfvermerk.
//!
//! Regeln (Vertrag R28 §1):
//! - Status setzt nur der Mensch ([`Store::finding_set_status`]); Assistenten stellen nur Vorschläge
//!   ([`Store::finding_propose_status`], nur übernommen/verschoben/erledigt), über die der Mensch entscheidet
//!   ([`Store::finding_decide`]). Jede Änderung steht in `finding_status` (die Erstanlage nicht).
//! - Offen zählt (Abzeichen, [`Store::finding_open_counts`]): `status = 'open'`, nicht angehängt (`similar_to IS
//!   NULL`), kein Lob, mit Abschnitt.
//! - Deutsche Bezeichnungen nur über [`FindingKind::label`] und [`FindingStatus::label`].
//!
//! - Spiegel: `befunde.md` ([`mirror_files`]) nur, wenn das Werk Befunde hat; deterministisch nach Teil, Kapitel,
//!   Abschnitt (Baumreihenfolge), dann Fäden, dann Befunde ohne Bezug.
//!
//! S1 setzte die lesenden Abfragen und das schlichte [`Store::finding_insert`] um; Statuswechsel, Vorschläge,
//! Entscheidungen und `befunde.md` kamen mit R28a (Aufgabe befunde).
//!
//! Runde 22 (befund-rueckfrage, Migration 0050): Beim Übernehmen merkt der Store den Stand der Fortschreibung der
//! Fassung (`revision.amend_seq`) in `finding.adopted_amend` und `finding_status.adopted_amend`: beim Setzen durch den
//! Menschen den Stand in diesem Augenblick; beim Vorschlag eines Assistenten den Stand beim Vorschlag (nur im Eintrag)
//! und beim Bestätigen den Stand beim Bestätigen (Eintrag und Befund). Jeder andere Status setzt
//! `finding.adopted_amend` auf `NULL`. Anzeige „Fassung n · Stand k“ nur für k > 0 ([`adopted_words`]).

use std::collections::HashMap;

use sqlx::{Row, SqliteConnection};

use crate::ideas::IdeaOrigin;
use crate::{MirrorFile, Store, StoreError, WorkId, to_u32, to_usize};

/// Höchstens so viele Zeichen hat der Text eines Befunds.
pub const MAX_FINDING_CHARS: usize = 4000;
/// Höchstens so viele Zeichen hat das Zitat eines Befunds.
pub const MAX_QUOTE_CHARS: usize = 2000;
/// Höchstens so viele Zeichen hat eine Begründung (verworfen, verschoben, erledigt).
pub const MAX_REASON_CHARS: usize = 2000;
/// Höchstens so viele Zeichen hat der Prüfvermerk eines Zitats (CHECK der Migration 0046).
const MAX_QUOTE_CHECK_CHARS: usize = 200;
/// Höchstens so viele Zeichen hat der Name der Persona (CHECK der Migration 0046).
const MAX_PERSONA_CHARS: usize = 200;
/// Vorgabe und Obergrenze für [`FindingFilter::limit`].
const DEFAULT_LIMIT: u32 = 500;
const MAX_LIMIT: u32 = 2000;

/// Art eines Befunds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    Praise,
    Hint,
    Critical,
}

impl FindingKind {
    pub const ALL: [FindingKind; 3] = [FindingKind::Praise, FindingKind::Hint, FindingKind::Critical];

    /// Wert in der Datenbank: `praise`, `hint`, `critical`.
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

    /// Anzeige: „Lob“, „Hinweis“, „kritisch“.
    pub fn label(self) -> &'static str {
        match self {
            FindingKind::Praise => "Lob",
            FindingKind::Hint => "Hinweis",
            FindingKind::Critical => "kritisch",
        }
    }
}

/// Status eines Befunds.
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

    /// Wert in der Datenbank: `open`, `adopted`, `rejected`, `deferred`, `done`.
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

    /// Anzeige: „offen“, „übernommen“, „verworfen“, „verschoben“, „erledigt ohne Änderung“.
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

/// Ziel eines verschobenen Befunds: ein Abschnitt (fester Schlüssel) oder ein Einfall.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeferTarget {
    Node { key: i64 },
    Idea { id: i64 },
}

/// Gewünschter Status samt Pflichtangaben.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusChange {
    Open,
    /// Fassungsnummer im Abschnitt des Befunds.
    Adopted {
        rev: u32,
    },
    /// Begründung Pflicht, getrimmt nicht leer, höchstens [`MAX_REASON_CHARS`].
    Rejected {
        reason: String,
    },
    /// Begründung darf leer sein.
    Deferred {
        target: DeferTarget,
        reason: String,
    },
    /// Begründung darf leer sein.
    Done {
        reason: String,
    },
}

/// Ein Eintrag im Verlauf des Status (Mensch setzt, Assistent schlägt vor).
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
    /// Runde 22: Stand der Fassung beim Übernehmen (`finding_status.adopted_amend`); `None` = nicht gemerkt.
    pub adopted_amend: Option<u32>,
}

/// Ein Befund mit aufgelösten Bezügen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Finding {
    pub id: i64,
    pub uuid: String,
    pub work: WorkId,
    /// `node.id`; `None` = Faden ohne Abschnitt bzw. Abschnitt gelöscht.
    pub key: Option<i64>,
    /// Stelle (seq) jetzt, aus dem Join.
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
    /// Über `adopted_revision` im Abschnitt aufgelöst; `None`, wenn die Fassung fehlt.
    pub adopted_rev: Option<u32>,
    pub adopted_revision: Option<String>,
    /// Aufgelöst; `None`, wenn nicht verschoben oder das Ziel fehlt.
    pub deferred: Option<DeferTarget>,
    pub deferred_uuid: Option<String>,
    pub similar_to: Option<i64>,
    pub summary_of: Vec<String>,
    /// Jüngster offener Vorschlag (`proposed = 1`, `decision IS NULL`).
    pub pending: Option<FindingStatusEntry>,
    pub created: String,
    pub updated: String,
    /// Runde 30b: Lösungsvorschläge zum Befund (Migration 0047, `finding.solutions`); leer = keine.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub solutions: Vec<Solution>,
    /// Runde 22: Stand der Fassung beim Übernehmen (`finding.adopted_amend`); `None` = nicht gemerkt.
    pub adopted_amend: Option<u32>,
    /// Runde 23a (PM-Entscheidung): Sammelbefund (`summary_of` nicht leer), der selbst noch offen ist, dessen Ursprünge
    /// aber alle entschieden sind (kein Ursprung mehr offen; gelöschte Ursprünge zählen als entschieden) – „veraltet“.
    /// Abgeleitet beim Lesen, nie gespeichert; der Status bleibt, wie der Mensch ihn gesetzt hat.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stale_summary: bool,
}

/// Ein Lösungsvorschlag zu einem Befund (Runde 30b): wörtliches Zitat und neue Formulierung (`statt`/`besser`) oder
/// ein klarer nächster Schritt.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Solution {
    pub quote: String,
    pub replacement: String,
    pub step: String,
}

/// Ein neuer Befund ([`Store::finding_insert`]).
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

/// Auswahl für [`Store::finding_list`].
#[derive(Debug, Clone, Default)]
pub struct FindingFilter {
    /// Abschnitte (`node.id`); leer = alle (auch ohne Abschnitt).
    pub keys: Vec<i64>,
    pub post: Option<i64>,
    /// Leer = alle.
    pub statuses: Vec<FindingStatus>,
    /// Leer = alle; Lob nur mit `include_praise` oder ausdrücklich in `kinds`.
    pub kinds: Vec<FindingKind>,
    pub persona: Option<String>,
    /// `created >= since` (ISO).
    pub since: Option<String>,
    /// `created < until` (ISO).
    pub until: Option<String>,
    pub include_praise: bool,
    /// Auch angehängte Befunde (`similar_to IS NOT NULL`).
    pub include_attached: bool,
    /// `None` = 500, höchstens 2000; passen mehr, die neuesten.
    pub limit: Option<u32>,
}

// ---- Lesen ---------------------------------------------------------------------------------------

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

/// Runde 23a: offene Vorschläge und „veraltet“ für eine ganze Liste in **einer** Abfrage je 500 Befunde (vorher je
/// Befund eine: bis zu 2000 Abfragen für eine Arbeitsseite). Je Befund der jüngste offene Vorschlag (`proposed = 1`,
/// `decision IS NULL`, größte id); Sammelbefunde bekommen [`Finding::stale_summary`].
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
    // Ursprünge offener Sammelbefunde: welche davon sind noch offen?
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

/// Zeilen von [`FINDING_SELECT`] als Befunde, samt offenen Vorschlägen und „veraltet“ ([`complete`]).
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

/// Eine Zeile von [`FINDING_SELECT`] ohne weitere Abfrage (`pending` und `stale_summary` setzt [`complete`]).
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

/// Befund `id` im Werk `work` (in der Verbindung bzw. Transaktion des Aufrufers); fremdes Werk oder unbekannt →
/// [`StoreError::UnknownFinding`], unbekanntes Werk → [`StoreError::UnknownWork`].
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

/// Bedingungen von `filter` (ohne `limit`) als „ AND …“ hinter `WHERE f.work_id = ?`; Platzhalter erst `numbers`,
/// dann `texts` binden.
fn filter_clause(filter: &FindingFilter) -> (String, Vec<i64>, Vec<String>) {
    let mut sql = String::new();
    let mut texts: Vec<String> = Vec::new();
    let mut numbers: Vec<i64> = Vec::new();
    let marks = |n: usize| vec!["?"; n].join(", ");
    // Zahlen zuerst binden, dann Texte: die Reihenfolge der Platzhalter folgt dieser Aufteilung.
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

/// Gehört die Zeile `id` der Tabelle über `sql` (ein `SELECT 1 … WHERE <id> AND <werk>`) zum Werk?
async fn belongs(conn: &mut SqliteConnection, sql: &'static str, id: i64, work: WorkId) -> Result<bool, StoreError> {
    let found: Option<i64> = sqlx::query_scalar(sql).bind(id).bind(work).fetch_optional(&mut *conn).await?;
    Ok(found.is_some())
}

impl Store {
    /// Neuen Befund anlegen (eine Transaktion, kein Statusverlauf). Grenzen (Text getrimmt nicht leer und höchstens
    /// [`MAX_FINDING_CHARS`], Zitat höchstens [`MAX_QUOTE_CHARS`], Prüfvermerk höchstens 200, Persona 1 bis 200
    /// Zeichen; nicht zugleich Beitrag und Fadenbeitrag; Abschnitt, Beitrag, Fadenbeitrag und `similar_to` aus
    /// diesem Werk) → [`StoreError::Finding`]; derselbe Beitrag mit derselben Stelle zweimal → Datenbankfehler
    /// (eindeutiger Index). Gibt den angelegten Befund zurück.
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

    /// Lösungsvorschläge des Befunds `id` ersetzen (Runde 30b, atomar; leer = keine). Jede Lösung braucht ein Zitat mit
    /// neuer Formulierung oder einen Schritt; Zitat höchstens [`MAX_QUOTE_CHARS`], Formulierung und Schritt höchstens
    /// [`MAX_FINDING_CHARS`] Zeichen, sonst [`StoreError::Finding`]. Fremdes Werk oder unbekannt →
    /// [`StoreError::UnknownFinding`]. Status und `updated` bleiben unberührt.
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

    /// Befund `id` des Werks; fremdes Werk oder unbekannt → [`StoreError::UnknownFinding`].
    pub async fn finding_get(&self, work: WorkId, id: i64) -> Result<Finding, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, work, id).await
    }

    /// Befunde des Werks nach `filter`, älteste zuerst (`created`, dann `id`). Ohne `include_praise` kein Lob (außer
    /// `kinds` nennt Lob ausdrücklich), ohne `include_attached` keine angehängten Befunde. Passen mehr als `limit`,
    /// kommen die **neuesten** `limit` (ältere fallen weg, nicht die jüngsten); ob gekappt wurde, sagt der Vergleich
    /// mit [`Store::finding_count`].
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

    /// Zahl der Befunde nach `filter` ohne Grenze (`limit` zählt nicht): mehr als [`Store::finding_list`] liefert
    /// heißt gekappt.
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

    /// Befunde, die an `id` angehängt sind (`similar_to = id`), älteste zuerst, ohne Grenze.
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

    /// Befunde des Werks mit diesen UUIDs (etwa `summary_of`), älteste zuerst; unbekannte fehlen.
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

    /// Verlauf des Befunds `id` (Statuswechsel und Vorschläge), älteste zuerst.
    pub async fn finding_history(&self, work: WorkId, id: i64) -> Result<Vec<FindingStatusEntry>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, work, id).await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!("{ENTRY_SELECT} WHERE s.finding_id = ? ORDER BY s.id")))
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
        rows.iter().map(entry_of).collect()
    }

    /// Offene Befunde je Abschnitt (`node.id` → Zahl) nach Vertrag §1.5: offen, nicht angehängt, kein Lob, mit
    /// Abschnitt; nur der Abschnitt selbst (keine Summen über Unterpunkte). Abschnitte ohne offene fehlen.
    /// Runde 23a (PM-Entscheidung): Sammelbefunde (`summary_of` nicht leer) zählen nicht – ihre Ursprünge zählen
    /// schon; sonst stiege „k offen“ nach einem RFC doppelt.
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

    /// Je Fassungsnummer des Abschnitts `key` die übernommenen Befunde (ids aufsteigend), Fassungen aufsteigend;
    /// Befunde, deren Fassung fehlt, fehlen.
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

    /// Status setzen (nur der Mensch): schreibt `finding` und einen Eintrag in `finding_status` (`actor_kind =
    /// 'human'`, `proposed = 0`) in einer Transaktion und gibt (vorher, nachher) zurück. Prüft vorher (Vertrag R28
    /// §1.2, RPC `finding.set_status`): übernommen nur mit einer Fassung, die es im Abschnitt des Befunds gibt;
    /// verworfen nur mit Begründung (getrimmt nicht leer, höchstens [`MAX_REASON_CHARS`]); verschoben nur mit einem
    /// Abschnitt bzw. Einfall desselben Werks (nicht der eigene Abschnitt). Verstöße und ein Status, den der Befund
    /// genau so schon hat → [`StoreError::Finding`]; dann ändert sich nichts.
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

    /// Statusvorschlag eines Assistenten (Persona bzw. MCP-Client; nie der Mensch): nur übernommen, verschoben,
    /// erledigt ohne Änderung – verworfen und offen lehnt der Store ab („Nur der Mensch kann …“). Geprüft wird wie
    /// bei [`Store::finding_set_status`]; ein gleicher offener Vorschlag (gleicher Status, gleiche Fassung bzw. gleiches
    /// Ziel) oder ein Status, den der Befund schon hat, wird nicht angelegt ([`StoreError::Finding`]). Der Befund selbst
    /// ändert sich nicht.
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

    /// Über einen Vorschlag entscheiden (nur der Mensch). Veraltet ist er, wenn der Befund seitdem einen Status vom
    /// Menschen bzw. aus einem bestätigten Vorschlag bekam, die vorgeschlagene Fassung bzw. das Ziel fehlt oder der
    /// Befund den Status schon hat: dann `decision = 'stale'`, und der Befund bleibt. Sonst bestätigt (`confirmed`,
    /// der Befund bekommt den vorgeschlagenen Status) bzw. abgelehnt (`rejected`). Unbekannter oder schon
    /// entschiedener Vorschlag → [`StoreError::Finding`]. Gibt (Befund nachher, Eintrag nachher) zurück.
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
            // Runde 22: Stand der Fassung beim Bestätigen (Eintrag und Befund).
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

/// Gewünschter Stand eines Befunds in Spalten (Fassung und Ziel als uuid).
struct Target {
    status: FindingStatus,
    reason: String,
    adopted_revision: Option<String>,
    deferred_node: Option<String>,
    deferred_idea: Option<String>,
    /// Runde 22: Stand der übernommenen Fassung (`revision.amend_seq`) beim Auflösen; sonst `None`.
    adopted_amend: Option<i64>,
}

impl Target {
    /// Hat der Befund genau diesen Stand schon?
    fn same_as(&self, finding: &Finding) -> bool {
        self.status == finding.status
            && self.reason == finding.reason
            && self.adopted_revision == finding.adopted_revision
            && self.deferred_node.clone().or_else(|| self.deferred_idea.clone()) == finding.deferred_uuid
    }
}

/// Begründung prüfen: höchstens [`MAX_REASON_CHARS`], bei verworfen Pflicht.
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

/// Änderung prüfen und in Spalten übersetzen (Fassung über ihre Nummer im Abschnitt des Befunds, Ziel über Abschnitt
/// bzw. Einfall desselben Werks).
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

/// Eintrag `entry` im Verlauf eines Befunds des Werks; sonst [`StoreError::Finding`].
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

/// „Fassung n“ bzw. mit Fortschreibung „Fassung n · Stand k“ (Runde 22; Stand 0 oder unbekannt zeigt nur die Fassung).
pub fn adopted_words(rev: u32, amend: Option<u32>) -> String {
    match amend.filter(|k| *k > 0) {
        Some(k) => format!("Fassung {rev} · Stand {k}"),
        None => format!("Fassung {rev}"),
    }
}

/// Pfad der Spiegeldatei der Befunde.
pub const MIRROR_FILE: &str = "befunde.md";

/// Gruppe in `befunde.md`: Abschnitt (Baumreihenfolge), Faden (nach id), ohne Bezug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Section(usize),
    Thread(i64),
    Loose,
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Dateien der Befunde im Werkspiegel: `befunde.md`, nur wenn das Werk Befunde hat. Deterministisch: Abschnitte in
/// Baumreihenfolge (Teil, Kapitel, Abschnitt), dann Fäden nach id, dann Befunde ohne Bezug; darin nach Anlage. Je
/// Befund Art, Status, Persona, Text, Zitat mit Prüfvermerk, Begründung, Fassung bzw. Ziel, Anhang, Sammlung, offener
/// Vorschlag, Herkunft und der Verlauf.
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
