// SPDX-License-Identifier: AGPL-3.0-or-later
//! Revisionsbaum (Vertrag Runde 3, Abschnitte 2 und 3): Umschalten der Markierung „aktuell“, eigene
//! Fassung auf einer gewählten Basis, Ändern und Löschen durch den Menschen, Baum aller Fassungen.
//!
//! Regeln (Migration `0004_revision_tree.sql`): Jede Revision hat ein Elternteil `based_on` (eine
//! kleinere Nummer desselben Abschnitts) oder ist eine Wurzel. Je Abschnitt genau eine `current`.
//! Umschalten ändert keinen Text der Fassung; die Gliederung folgt ihrer Überschriftszeile
//! ([`Store::switch_to`], Vertrag Runde 5). Ändern betrifft nur Text und Notiz (Herkunft unveränderlich) und
//! setzt `edited`. Löschen hängt die Kinder an das Elternteil der gelöschten Fassung; die aktuelle
//! Fassung lässt sich nicht löschen. Nummern werden nie wiederverwendet.

use baruch_core::{Author, RevisionStatus};
use sqlx::SqliteConnection;

use crate::write::{self, Row};
use crate::{Change, ChangeKind, OutlineChange, Store, StoreError, WorkId, parse_author, to_u32, to_usize, words};

/// Eine Fassung im Baum (ohne Text).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionInfo {
    pub number: u32,
    /// Elternteil; `None` = Wurzel.
    pub based_on: Option<u32>,
    pub status: RevisionStatus,
    pub author: Author,
    pub note: String,
    pub created: String,
    /// Letzte Änderung durch den Menschen; `None` = nie geändert.
    pub edited: Option<String>,
    /// Runde 30b: Kennzeichen einer Variante (Gruppe, Etikett A/B/C, Zeile, Befunde); `None` = keine Variante.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<crate::VariantInfo>,
    /// Runde 22: Stand der Fassung (`revision.amend_seq`, 0 = nie fortgeschrieben).
    pub amend_seq: u32,
    /// Runde 22 (Vertrag §2.5): Stand der Basis, auf dem ein Vorschlag beruht (`revision.based_on_amend`);
    /// `None` = ohne bzw. vor Runde 22 angelegt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on_amend: Option<u32>,
}

/// Alle Fassungen eines Abschnitts (seq), nach Nummer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NodeRevisions {
    pub node: usize,
    pub revisions: Vec<RevisionInfo>,
}

/// Vollständiger Stand einer Revision, z. B. vor dem Ändern oder Löschen (für das Protokoll im Spiegel).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionRecord {
    pub number: u32,
    pub body: String,
    pub note: String,
    pub status: RevisionStatus,
    pub based_on: Option<u32>,
    pub author: Author,
}

/// Ergebnis von [`Store::switch_to`]: die Umschaltung und, wenn die Fassung eine andere Ebene
/// mitbringt, die Gliederungsänderung (`action: Level`, mitgestufte Unterpunkte mit neuen Fassungen).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Switch {
    pub change: Change,
    pub outline: Option<OutlineChange>,
}

/// Name des Menschen für die Hüllen [`Store::set_current`] und [`Store::accept`] (wie im Server).
pub(crate) const HUMAN: &str = "Autor";

/// Was [`Store::update_revision`] ändern soll; `None` = bleibt.
#[derive(Debug, Clone, Copy, Default)]
pub struct RevisionEdit<'a> {
    pub body: Option<&'a str>,
    pub note: Option<&'a str>,
}

fn record(row: &Row) -> Result<RevisionRecord, StoreError> {
    Ok(RevisionRecord {
        number: row.number,
        body: row.body.clone(),
        note: row.note.clone(),
        status: words::parse_status(&row.status)?,
        based_on: row.based_on,
        author: row.author.clone(),
    })
}

/// Ändern über den Baum. Runde 23a (Backlog „Nach Runde 22 offen“, Codereview store-1/web-3):
///
/// - Ändert sich der **Text** der aktuellen Fassung oder einer schon fortgeschriebenen (Stand > 0), entsteht ein
///   Schritt im Verlauf (`revision_amend`, Art 'amend', Text vorher und nachher) und der Stand (`amend_seq`) steigt um
///   eins. So bleibt die Kette der Schritte lückenlos (Vorspulen beim Abgleich, Austausch), und ein Editor, der noch
///   den alten Stand geladen hat, schreibt nicht still darüber hinweg (`NotOnCurrent` in [`crate::amend`]). Eine nie
///   fortgeschriebene, nicht aktuelle Fassung (etwa ein Vorschlag) bekommt wie bisher keinen Verlauf.
/// - `expected_amend`: Stand, den der Aufrufer geladen hatte. Weicht er vom gespeicherten ab, wird nichts geändert
///   (`StoreError::Stale`); `None` prüft nichts (alte Clients).
async fn update(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
    edit: &RevisionEdit<'_>,
    human: &str,
    expected_amend: Option<u32>,
) -> Result<Option<Change>, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    let stored: i64 =
        sqlx::query_scalar("SELECT amend_seq FROM revision WHERE id = ?").bind(row.id).fetch_one(&mut *conn).await?;
    let amend_seq = crate::to_u32(stored, "amend_seq")?;
    if let Some(expected) = expected_amend
        && expected != amend_seq
    {
        return Err(StoreError::Stale(format!(
            "Veraltet: Fassung {rev} steht inzwischen auf Stand {amend_seq} (geladen war Stand {expected}); \
             bitte neu laden und die Änderung dort eintragen."
        )));
    }
    let body = edit.body.map(|b| b.replace("\r\n", "\n"));
    if body.as_deref().is_some_and(|b| b.trim().is_empty()) {
        return Err(StoreError::Refused("Eine Fassung braucht Text."));
    }
    let new_body = body.unwrap_or_else(|| row.body.clone());
    let new_note = edit.note.map_or_else(|| row.note.clone(), str::to_owned);
    if new_body == row.body && new_note == row.note {
        return Ok(None);
    }
    let step = (new_body != row.body && (row.status == "current" || amend_seq > 0)).then_some(amend_seq + 1);
    sqlx::query(
        "UPDATE revision SET body = ?, note = ?, amend_seq = ?, edited = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
         WHERE id = ?",
    )
    .bind(&new_body)
    .bind(&new_note)
    .bind(i64::from(step.unwrap_or(amend_seq)))
    .bind(row.id)
    .execute(&mut *conn)
    .await?;
    if let Some(seq) = step {
        let author = if human.trim().is_empty() { HUMAN } else { human };
        sqlx::query(
            "INSERT INTO revision_amend (revision_id, seq, kind, undo_of, body_before, body_after, author_name) \
             VALUES (?, ?, 'amend', NULL, ?, ?, ?)",
        )
        .bind(row.id)
        .bind(i64::from(seq))
        .bind(&row.body)
        .bind(&new_body)
        .bind(author)
        .execute(&mut *conn)
        .await?;
    }
    Ok(Some(Change {
        work,
        node,
        rev,
        author: Author::Human { name: human.to_owned() },
        kind: ChangeKind::Updated,
        before: Some(record(&row)?),
        reparented: Vec::new(),
        amend_seq: step,
    }))
}

async fn delete(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
    human: &str,
) -> Result<Change, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    if row.status == "current" {
        return Err(StoreError::Refused(
            "Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.",
        ));
    }
    let children: Vec<i64> =
        sqlx::query_scalar("SELECT number FROM revision WHERE node_id = ? AND based_on = ? ORDER BY number")
            .bind(node_id)
            .bind(i64::from(rev))
            .fetch_all(&mut *conn)
            .await?;
    let reparented = children.into_iter().map(|n| to_u32(n, "Revisionsnummer")).collect::<Result<Vec<_>, _>>()?;
    // Der Trigger `revision_reparent` hängt die Kinder um.
    sqlx::query("DELETE FROM revision WHERE id = ?").bind(row.id).execute(&mut *conn).await?;
    Ok(Change {
        work,
        node,
        rev,
        author: Author::Human { name: human.to_owned() },
        kind: ChangeKind::Deleted,
        before: Some(record(&row)?),
        reparented,
        amend_seq: None,
    })
}

type InfoRow = (
    i64,
    Option<i64>,
    Option<i64>,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
);

async fn tree(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<NodeRevisions>, StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    if exists.is_none() {
        return Err(StoreError::UnknownWork(work));
    }
    let rows: Vec<InfoRow> = sqlx::query_as(
        "SELECT n.seq, r.number, r.based_on, r.status, r.author_kind, r.author_name, r.model, r.note, r.created, r.edited, \
         r.variant_group, r.variant_label, r.variant_summary, r.variant_findings, r.amend_seq, r.based_on_amend \
         FROM node n LEFT JOIN revision r ON r.node_id = n.id \
         WHERE n.work_id = ? ORDER BY n.seq, r.number",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut out: Vec<NodeRevisions> = Vec::new();
    for (
        seq,
        number,
        based_on,
        status,
        kind,
        name,
        model,
        note,
        created,
        edited,
        group,
        label,
        summary,
        findings,
        amend,
        base_amend,
    ) in rows
    {
        let seq = to_usize(seq, "seq")?;
        if out.last().is_none_or(|n| n.node != seq) {
            out.push(NodeRevisions { node: seq, revisions: Vec::new() });
        }
        let Some(number) = number else { continue }; // Abschnitt ohne Revisionen
        let entry = out.last_mut().expect("eben angelegt");
        entry.revisions.push(RevisionInfo {
            number: to_u32(number, "Revisionsnummer")?,
            based_on: based_on.map(|b| to_u32(b, "based_on")).transpose()?,
            status: words::parse_status(&status)?,
            author: parse_author(&kind, name, model)?,
            note,
            created,
            edited,
            variant: crate::variants::from_columns(
                group,
                label,
                summary.unwrap_or_default(),
                findings.as_deref().unwrap_or("[]"),
            )?,
            amend_seq: to_u32(amend.unwrap_or(0), "amend_seq")?,
            based_on_amend: base_amend.map(|b| to_u32(b, "based_on_amend")).transpose()?,
        });
    }
    Ok(out)
}

/// Nicht aktuelle Fassungen für die Seitenzweige im Spiegel (Runde 22, Vertrag §2.12): je Fassung der feste
/// Schlüssel, die Angaben aus dem Baum und die Abschnittsdatei, wie der Spiegel sie mit dieser Fassung trüge (Pfad
/// wie [`crate::snapshot::files`], Inhalt mit Import-Zeile der Werkvorlage und Sprungmarke `<kN>`; ohne die Umstellung
/// „Nummer an/aus“ und ohne die Register-Projektion, die nur für die aktuelle Fassung im Zusammenhang gelten).
async fn side(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<Vec<(i64, RevisionInfo, crate::MirrorFile)>, StoreError> {
    let infos = tree(conn, work).await?;
    let template: String =
        sqlx::query_scalar("SELECT template FROM work WHERE id = ?").bind(work).fetch_one(&mut *conn).await?;
    let has_template = !template.is_empty();
    let rows: Vec<(i64, i64, String, i64, String)> = sqlx::query_as(
        "SELECT n.seq, n.id, n.title, r.number, r.body FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND r.status <> 'current' ORDER BY n.seq, r.number",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (seq, key, title, number, body) in rows {
        let seq = to_usize(seq, "seq")?;
        let number = to_u32(number, "Revisionsnummer")?;
        let Some(info) =
            infos.iter().find(|n| n.node == seq).and_then(|n| n.revisions.iter().find(|r| r.number == number)).cloned()
        else {
            continue;
        };
        let path = format!("abschnitte/{seq:03}-{}.typ", crate::slug(&title));
        let body = baruch_typst::xref::label_section(&body, key).into_owned();
        // Wie im HEAD des Spiegels (Runde 22, „Spiegel setzen“): relative Bildpfade ab der Wurzel (Gutachten R22).
        let body = crate::snapshot::root_image_paths(&body).into_owned();
        let contents = if has_template { format!("{}{body}", crate::snapshot::TEMPLATE_IMPORT) } else { body };
        out.push((key, info, crate::MirrorFile { path, contents }));
    }
    Ok(out)
}

impl Store {
    /// Seitenzweige des Spiegels (Runde 22, Vertrag §2.12): alle nicht aktuellen Fassungen (ersetzt, verworfen,
    /// offene Vorschläge samt Varianten) mit festem Schlüssel, Baumangaben und der Abschnittsdatei, wie der Spiegel
    /// sie mit dieser Fassung trüge. Unbekanntes Werk → `UnknownWork`.
    pub async fn side_revisions(
        &self,
        work: WorkId,
    ) -> Result<Vec<(i64, RevisionInfo, crate::MirrorFile)>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        side(&mut conn, work).await
    }

    /// Markierung „aktuell“ umschalten (atomar, eine Transaktion): bisherige `current` → `superseded`,
    /// `rev` → `current`; der Text der Fassung ändert sich nie. Mit `only_proposal` nur für Vorschläge
    /// (Übernehmen, sonst `Refused("Nur Vorschläge lassen sich übernehmen.")`), ohne für jede nicht
    /// aktuelle Fassung (schon aktuell → `Refused("Diese Fassung ist schon aktuell.")`). War `rev` ein
    /// Vorschlag, ist `change.kind` = `Accepted`, sonst `Switched`.
    ///
    /// Die Gliederung folgt der Überschriftszeile der Fassung (Vertrag Runde 5): gleicher Ebene nur der
    /// Titel; bei anderer Ebene Ebene und Titel, die Unterpunkte wandern um dieselbe Differenz mit
    /// (höchstens Ebene 3) und bekommen neue Fassungen von `human`, die sich nur in der
    /// Überschriftszeile unterscheiden (`outline` mit `action: Level`). Keine oder weitere
    /// Überschriften, Text davor oder eine hier ungültige Ebene → `Heading`, nichts geändert. Teile:
    /// eine Fassung mit Teil-Markierung gleicht den Titel ab (Runde 14), eine ohne schaltet ohne Abgleich um.
    pub async fn switch_to(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        only_proposal: bool,
        human: &str,
    ) -> Result<Switch, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = if only_proposal {
            write::accept(&mut tx, work, node, rev).await?
        } else {
            write::set_current(&mut tx, work, node, rev).await?
        };
        let outline = crate::outline::follow_switched(&mut tx, work, node, human).await?;
        tx.commit().await?;
        Ok(Switch { change, outline })
    }

    /// Umschalten auf jede nicht aktuelle Fassung: [`Store::switch_to`] ohne `only_proposal`, als
    /// Mensch „Autor“; nur die Änderung der Markierung.
    pub async fn set_current(&self, work: WorkId, node: usize, rev: u32) -> Result<Change, StoreError> {
        self.switch_to(work, node, rev, false, HUMAN).await.map(|s| s.change)
    }

    /// Eigene Fassung auf der Basis `based_on` speichern (atomar): neue Revision vom Menschen, sofort
    /// `current`, Elternteil = `based_on`. CRLF wird zu LF. Gleicher Text wie die Basis oder wie die
    /// aktuelle Fassung → `Ok(None)`. Unbekannte Basis → `UnknownRevision`.
    pub async fn save_human_on(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: u32,
    ) -> Result<Option<Change>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = write::save_human(&mut tx, work, node, body, author, Some(based_on)).await?;
        if change.is_some() {
            crate::outline::check_current_heading(&mut tx, work, node).await?;
            tx.commit().await?;
        }
        Ok(change)
    }

    /// Text und/oder Notiz einer Revision ändern (atomar, nur für den Menschen `human`) und `edited`
    /// setzen. Status und Herkunft bleiben. CRLF wird zu LF. Keine Änderung → `Ok(None)`; leerer
    /// Text → `Refused("Eine Fassung braucht Text.")`. `Change.before` trägt den Stand vorher.
    pub async fn update_revision(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        edit: &RevisionEdit<'_>,
        human: &str,
    ) -> Result<Option<Change>, StoreError> {
        self.update_revision_at(work, node, rev, edit, human, None).await
    }

    /// Wie [`Store::update_revision`], mit dem Stand (`amend_seq`), den der Aufrufer geladen hatte (Runde 23a): Steht
    /// die Fassung inzwischen auf einem anderen Stand, wird nichts geändert (`StoreError::Stale` „Veraltet: …“).
    /// Ändert sich der Text der aktuellen oder einer schon fortgeschriebenen Fassung, entsteht ein Schritt im Verlauf
    /// (Art 'amend') und der Stand steigt (`Change.amend_seq` nennt ihn).
    pub async fn update_revision_at(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        edit: &RevisionEdit<'_>,
        human: &str,
        expected_amend: Option<u32>,
    ) -> Result<Option<Change>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = update(&mut tx, work, node, rev, edit, human, expected_amend).await?;
        if let Some(done) = &change {
            // Nur die aktuelle Fassung bestimmt Titel und Gliederung; eine andere Fassung zu ändern
            // prüft nichts (sonst sperrte eine schon abweichende aktuelle Fassung jede Änderung).
            if done.before.as_ref().is_some_and(|b| b.status == baruch_core::RevisionStatus::Current) {
                crate::outline::check_current_heading(&mut tx, work, node).await?;
            }
            tx.commit().await?;
        }
        Ok(change)
    }

    /// Revision löschen (atomar, nur für den Menschen `human`). Die aktuelle Fassung →
    /// `Refused("Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.")`.
    /// Kinder hängen danach am Elternteil der gelöschten Fassung (`Change.reparented`), die Nummer
    /// wird nie neu vergeben. `Change.before` trägt den vollständigen gelöschten Stand.
    pub async fn delete_revision(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        human: &str,
    ) -> Result<Change, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = delete(&mut tx, work, node, rev, human).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// Baum aller Fassungen (ohne Text): alle Abschnitte nach seq, je Abschnitt die Revisionen nach
    /// Nummer. Unbekanntes Werk → `UnknownWork`.
    pub async fn revision_tree(&self, work: WorkId) -> Result<Vec<NodeRevisions>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        tree(&mut conn, work).await
    }
}
