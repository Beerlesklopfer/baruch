// SPDX-License-Identifier: AGPL-3.0-or-later
//! Änderungen. Jede Funktion läuft innerhalb der Transaktion, die `Store` mit
//! `BEGIN IMMEDIATE` geöffnet hat; ein `Err` führt dort zum Zurückrollen (die Transaktion wird
//! ohne `commit` verworfen).
//!
//! Revisionen werden über ihre Nummer angesprochen. Nummern sind je Abschnitt eindeutig und
//! steigend, nach einem Löschen gibt es Lücken; die nächste Nummer ist `node.last_rev + 1`
//! (Migration 0004, Trigger `revision_number_fresh`/`revision_number_bump`).

use baruch_core::Author;
use sqlx::SqliteConnection;

use crate::seed::insert_post;
use crate::{Change, ChangeKind, StoreError, WorkId, broken, parse_author, to_u32};

/// Datenbank-id des Abschnitts `seq` im Werk; unterscheidet unbekanntes Werk und unbekannten Abschnitt.
pub(crate) async fn node_id(conn: &mut SqliteConnection, work: WorkId, node: usize) -> Result<i64, StoreError> {
    if let Ok(seq) = i64::try_from(node) {
        let id: Option<i64> = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
            .bind(work)
            .bind(seq)
            .fetch_optional(&mut *conn)
            .await?;
        if let Some(id) = id {
            return Ok(id);
        }
    }
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    match exists {
        Some(_) => Err(StoreError::UnknownNode { work, node }),
        None => Err(StoreError::UnknownWork(work)),
    }
}

/// Vorhandene Revisionsnummern nach Größe (leer ohne Revisionen); für die Meldung bei unbekannter Nummer.
async fn existing_numbers(conn: &mut SqliteConnection, node_id: i64) -> Result<Vec<u32>, StoreError> {
    let numbers: Vec<i64> = sqlx::query_scalar("SELECT number FROM revision WHERE node_id = ? ORDER BY number")
        .bind(node_id)
        .fetch_all(&mut *conn)
        .await?;
    numbers.into_iter().map(|n| to_u32(n, "Revisionsnummer")).collect()
}

/// Nummern für die Meldung von [`StoreError::UnknownRevision`]: „1, 2, 4“, leer „keine“.
pub(crate) fn number_list(numbers: &[u32]) -> String {
    if numbers.is_empty() {
        return "keine".to_owned();
    }
    numbers.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")
}

/// Nächste freie Nummer: `node.last_rev + 1` (auch nach dem Löschen der höchsten Fassung nie eine alte).
async fn next_number(conn: &mut SqliteConnection, node_id: i64) -> Result<u32, StoreError> {
    let last: i64 =
        sqlx::query_scalar("SELECT last_rev FROM node WHERE id = ?").bind(node_id).fetch_one(&mut *conn).await?;
    Ok(to_u32(last, "last_rev")? + 1)
}

/// Eine Revision mit allem, was Ändern, Löschen und Spiegel brauchen.
pub(crate) struct Row {
    pub id: i64,
    pub number: u32,
    pub body: String,
    pub note: String,
    pub status: String,
    pub based_on: Option<u32>,
    pub author: Author,
}

type RowTuple = (i64, i64, String, String, String, Option<i64>, String, String, Option<String>);

/// Revision `rev` des Abschnitts; unbekannte Nummer → `UnknownRevision` (mit den vorhandenen Nummern).
pub(crate) async fn row(conn: &mut SqliteConnection, node_id: i64, node: usize, rev: u32) -> Result<Row, StoreError> {
    let found: Option<RowTuple> = sqlx::query_as(
        "SELECT id, number, body, note, status, based_on, author_kind, author_name, model \
         FROM revision WHERE node_id = ? AND number = ?",
    )
    .bind(node_id)
    .bind(i64::from(rev))
    .fetch_optional(&mut *conn)
    .await?;
    let Some((id, number, body, note, status, based_on, kind, name, model)) = found else {
        let existing = existing_numbers(conn, node_id).await?;
        return Err(StoreError::UnknownRevision { node, rev, existing });
    };
    Ok(Row {
        id,
        number: to_u32(number, "Revisionsnummer")?,
        body,
        note,
        status,
        based_on: based_on.map(|b| to_u32(b, "based_on")).transpose()?,
        author: parse_author(&kind, name, model)?,
    })
}

/// Die aktuelle Revision `(number, body, author)`; `None` nur bei einem Abschnitt ohne Revisionen.
pub(crate) async fn current(
    conn: &mut SqliteConnection,
    node_id: i64,
) -> Result<Option<(u32, String, Author)>, StoreError> {
    let rows: Vec<(i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT number, body, author_kind, author_name, model FROM revision WHERE node_id = ? AND status = 'current'",
    )
    .bind(node_id)
    .fetch_all(&mut *conn)
    .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM revision WHERE node_id = ?")
        .bind(node_id)
        .fetch_one(&mut *conn)
        .await?;
    match (rows.len(), count) {
        (0, 0) => Ok(None),
        (1, _) => {
            let (number, body, kind, name, model) = rows.into_iter().next().expect("eine Zeile");
            Ok(Some((to_u32(number, "Revisionsnummer")?, body, parse_author(&kind, name, model)?)))
        }
        (n, _) => Err(broken(format!("Knoten {node_id}: {n} aktuelle Revisionen statt genau einer"))),
    }
}

/// Bisherige `current` → `superseded`; genau eine Zeile muss sich ändern.
async fn supersede_current(conn: &mut SqliteConnection, node_id: i64) -> Result<(), StoreError> {
    let done = sqlx::query("UPDATE revision SET status = 'superseded' WHERE node_id = ? AND status = 'current'")
        .bind(node_id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() != 1 {
        return Err(broken(format!(
            "Knoten {node_id}: {} aktuelle Revisionen abgelöst statt einer",
            done.rows_affected()
        )));
    }
    Ok(())
}

/// Markierung „aktuell“ auf `rev` setzen (der Aufrufer hat geprüft, dass `rev` nicht schon aktuell ist).
async fn switch(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    node_id: i64,
    rev: u32,
    kind: ChangeKind,
) -> Result<Change, StoreError> {
    if current(conn, node_id).await?.is_some() {
        supersede_current(conn, node_id).await?;
    }
    let done = sqlx::query("UPDATE revision SET status = 'current' WHERE node_id = ? AND number = ?")
        .bind(node_id)
        .bind(i64::from(rev))
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() != 1 {
        return Err(broken(format!("Abschnitt {node}: Fassung {rev} nicht umgeschaltet")));
    }
    let (now, _, author) =
        current(conn, node_id).await?.ok_or_else(|| broken(format!("Abschnitt {node}: keine aktuelle Revision")))?;
    if now != rev {
        return Err(broken(format!("Abschnitt {node}: aktuell ist {now} statt {rev}")));
    }
    Ok(Change { work, node, rev, author, kind, before: None, reparented: Vec::new(), amend_seq: None })
}

pub(crate) async fn accept(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
) -> Result<Change, StoreError> {
    let node_id = node_id(conn, work, node).await?;
    if row(conn, node_id, node, rev).await?.status != "proposed" {
        return Err(StoreError::Refused("Nur Vorschläge lassen sich übernehmen."));
    }
    switch(conn, work, node, node_id, rev, ChangeKind::Accepted).await
}

/// Auf jede nicht aktuelle Fassung umschalten; ein Vorschlag gilt dabei als übernommen (`Accepted`).
pub(crate) async fn set_current(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
) -> Result<Change, StoreError> {
    let node_id = node_id(conn, work, node).await?;
    let kind = match row(conn, node_id, node, rev).await?.status.as_str() {
        "current" => return Err(StoreError::Refused("Diese Fassung ist schon aktuell.")),
        "proposed" => ChangeKind::Accepted,
        _ => ChangeKind::Switched,
    };
    switch(conn, work, node, node_id, rev, kind).await
}

pub(crate) async fn reject(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
) -> Result<u32, StoreError> {
    let node_id = node_id(conn, work, node).await?;
    if row(conn, node_id, node, rev).await?.status != "proposed" {
        return Err(StoreError::Refused("Nur Vorschläge lassen sich verwerfen."));
    }
    let done =
        sqlx::query("UPDATE revision SET status = 'rejected' WHERE node_id = ? AND number = ? AND status = 'proposed'")
            .bind(node_id)
            .bind(i64::from(rev))
            .execute(&mut *conn)
            .await?;
    if done.rows_affected() != 1 {
        return Err(broken(format!("Abschnitt {node}: Vorschlag {rev} nicht verworfen")));
    }
    let (now, _, _) =
        current(conn, node_id).await?.ok_or_else(|| broken(format!("Abschnitt {node}: keine aktuelle Revision")))?;
    Ok(now)
}

/// Neue Revision am Ende eines Abschnitts.
struct NewRevision<'a> {
    number: u32,
    body: &'a str,
    author: &'a Author,
    status: &'static str,
    note: &'a str,
    based_on: Option<u32>,
}

async fn append(conn: &mut SqliteConnection, node_id: i64, revision: NewRevision<'_>) -> Result<(), StoreError> {
    let NewRevision { number, body, author, status, note, based_on } = revision;
    let (author_kind, author_name, model) = crate::author_columns(author);
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(node_id)
    .bind(i64::from(number))
    .bind(body)
    .bind(author_kind)
    .bind(author_name)
    .bind(model)
    .bind(status)
    .bind(note)
    .bind(based_on.map(i64::from))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Eigene Fassung speichern, sofort aktuell. `based_on`: die Fassung, auf der sie beruht (gezeigte
/// Fassung); `None` = die aktuelle (Wurzel bei leerem Abschnitt). Gleicher Text wie die Basis oder die
/// aktuelle Fassung → `Ok(None)`. Unbekannte Basis → `UnknownRevision`.
pub(crate) async fn save_human(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
    author: &str,
    based_on: Option<u32>,
) -> Result<Option<Change>, StoreError> {
    let node_id = node_id(conn, work, node).await?;
    let body = body.replace("\r\n", "\n");
    let before = current(conn, node_id).await?;
    let base = match based_on {
        Some(rev) => Some(row(conn, node_id, node, rev).await?.body),
        None => None,
    };
    let same_as_current = before.as_ref().is_some_and(|(_, text, _)| *text == body);
    if same_as_current || base.as_deref() == Some(body.as_str()) {
        return Ok(None);
    }
    let parent = based_on.or(before.as_ref().map(|(number, _, _)| *number));
    if before.is_some() {
        supersede_current(conn, node_id).await?;
    }
    let number = next_number(conn, node_id).await?;
    let author = Author::Human { name: author.to_owned() };
    let revision = NewRevision { number, body: &body, author: &author, status: "current", note: "", based_on: parent };
    append(conn, node_id, revision).await?;
    match current(conn, node_id).await? {
        Some((now, _, _)) if now == number => Ok(Some(Change {
            work,
            node,
            rev: number,
            author,
            kind: ChangeKind::Saved,
            before: None,
            reparented: Vec::new(),
            amend_seq: None,
        })),
        _ => Err(broken(format!("Abschnitt {node}: Revision {number} ist nicht aktuell"))),
    }
}

/// Runde 22 (Vertrag §2.5): Stand der Basis (`revision.amend_seq`) am Vorschlag `number` merken
/// (`based_on_amend`), damit die Anzeige „beruht auf Fassung n, Stand k“ auch nach weiteren Fortschreibungen stimmt.
async fn remember_base_amend(
    conn: &mut SqliteConnection,
    node_id: i64,
    number: u32,
    base: u32,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE revision SET based_on_amend = \
         (SELECT b.amend_seq FROM revision b WHERE b.node_id = ? AND b.number = ?) \
         WHERE node_id = ? AND number = ?",
    )
    .bind(node_id)
    .bind(i64::from(base))
    .bind(node_id)
    .bind(i64::from(number))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(crate) async fn add_proposal(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
    persona: &str,
    model: &str,
    note: &str,
) -> Result<u32, StoreError> {
    let node_id = node_id(conn, work, node).await?;
    let Some((based_on, _, _)) = current(conn, node_id).await? else {
        return Err(StoreError::Refused("Vorschläge brauchen eine aktuelle Revision."));
    };
    let number = next_number(conn, node_id).await?;
    let author = Author::Assistant { persona: persona.to_owned(), model: model.to_owned() };
    let body = body.replace("\r\n", "\n");
    let revision =
        NewRevision { number, body: &body, author: &author, status: "proposed", note, based_on: Some(based_on) };
    append(conn, node_id, revision).await?;
    remember_base_amend(conn, node_id, number, based_on).await?;
    Ok(number)
}

/// Vorschlag eines Assistenten auf der Basis `based_on` (Runde 30, Schnittstelle R30-S1): wie [`add_proposal`], aber
/// mit dem Elternteil `based_on` statt der aktuellen Fassung (Varianten sind Geschwister mit gleicher Basis, Lösungen
/// beruhen auf der geprüften Fassung). Unbekannte Basis → `UnknownRevision`; ein Abschnitt ohne aktuelle Revision
/// nimmt keine Vorschläge an (`Refused`). `variant` füllt die Spalten `variant_*` (Gruppe und Etikett immer
/// zusammen, Migration 0047); `None` lässt sie leer.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn add_proposal_on(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
    persona: &str,
    model: &str,
    note: &str,
    based_on: u32,
    variant: Option<&crate::VariantInfo>,
) -> Result<u32, StoreError> {
    let node_id = node_id(conn, work, node).await?;
    if current(conn, node_id).await?.is_none() {
        return Err(StoreError::Refused("Vorschläge brauchen eine aktuelle Revision."));
    }
    // Die Basis muss es geben (sonst `UnknownRevision` mit den vorhandenen Nummern).
    row(conn, node_id, node, based_on).await?;
    let number = next_number(conn, node_id).await?;
    let author = Author::Assistant { persona: persona.to_owned(), model: model.to_owned() };
    let body = body.replace("\r\n", "\n");
    let revision =
        NewRevision { number, body: &body, author: &author, status: "proposed", note, based_on: Some(based_on) };
    append(conn, node_id, revision).await?;
    remember_base_amend(conn, node_id, number, based_on).await?;
    if let Some(v) = variant {
        let findings = serde_json::to_string(&v.findings).expect("Liste von Zahlen ist immer JSON");
        sqlx::query(
            "UPDATE revision SET variant_group = ?, variant_label = ?, variant_summary = ?, variant_findings = ? \
             WHERE node_id = ? AND number = ?",
        )
        .bind(&v.group)
        .bind(&v.label)
        .bind(&v.summary)
        .bind(findings)
        .bind(node_id)
        .bind(i64::from(number))
        .execute(&mut *conn)
        .await?;
    }
    Ok(number)
}

pub(crate) async fn add_post(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    post: &baruch_core::Post,
) -> Result<i64, StoreError> {
    let node_id = node_id(conn, work, node).await?;
    let body = post.body.trim();
    if body.is_empty() {
        return Err(StoreError::EmptyPost);
    }
    let position: i64 = sqlx::query_scalar("SELECT coalesce(max(position) + 1, 0) FROM post WHERE node_id = ?")
        .bind(node_id)
        .fetch_one(&mut *conn)
        .await?;
    insert_post(conn, node_id, position, post, body).await
}
