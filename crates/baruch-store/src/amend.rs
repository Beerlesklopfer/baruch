// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fortschreiben der aktuellen Fassung (Runde 22, Vertrag §2.1–2.4, Migration 0050).
//!
//! - Speichert der Mensch im Editor im Modus [`SaveMode::Auto`] und gilt keine Regel für eine neue Fassung
//!   (`baruch_core::revision_policy::decide`), wird die aktuelle Fassung fortgeschrieben: neuer Text,
//!   `revision.amend_seq` + 1 und eine Zeile in `revision_amend` (Art 'amend', Text vorher und nachher).
//!   Nummer, Herkunft, Status, `based_on` und `edited` bleiben (`edited` setzt nur das Ändern über den Baum).
//! - [`Store::amend_undo`] nimmt über den Verlauf zurück – als neuer Schritt (Art 'undo'), nie durch Löschen.
//! - Einstellung je Werk: Tabelle `work_revision_policy`, fehlt die Zeile → [`RevisionPolicy::default`].
//! - Nur das Speichern aus dem Editor (`outline_save_mode`, `save_human_mode` im Modus `Auto`) schreibt fort; alle
//!   anderen Schreibwege (Bild einfügen, Registermarken, Verweise, Ersetzen, Gliederung) legen weiter neue Fassungen
//!   an (Vertrag §2.7).

use baruch_core::Author;
use baruch_core::revision_policy::{NewReason, SaveFacts, change_percent, decide};
use sqlx::SqliteConnection;

use crate::write;
use crate::{
    Change, ChangeKind, OutlineSave, Restructure, RevisionPolicy, SaveMode, Store, StoreError, WorkId, broken, to_u32,
};

/// Art eines Schritts im Verlauf der Fortschreibungen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AmendKind {
    Amend,
    Undo,
}

impl AmendKind {
    /// Wert der Spalte `revision_amend.kind`.
    pub fn as_db(self) -> &'static str {
        match self {
            AmendKind::Amend => "amend",
            AmendKind::Undo => "undo",
        }
    }

    /// Aus der Spalte `revision_amend.kind`; Unbekanntes → `None`.
    pub fn from_db(s: &str) -> Option<AmendKind> {
        match s {
            "amend" => Some(AmendKind::Amend),
            "undo" => Some(AmendKind::Undo),
            _ => None,
        }
    }
}

/// Ein Schritt im Verlauf einer Fassung (Zeile aus `revision_amend`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AmendStep {
    pub uuid: String,
    pub seq: u32,
    pub kind: AmendKind,
    pub undo_of: Option<u32>,
    pub author: String,
    pub created: String,
    pub body_before: String,
    pub body_after: String,
}

/// Ergebnis von [`Store::set_revision_policy`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RevisionPolicyChange {
    pub work: WorkId,
    pub before: RevisionPolicy,
    pub after: RevisionPolicy,
    /// Geänderte Felder: `amend`, `max_change_percent`, `new_after_minutes`.
    pub changed: Vec<&'static str>,
}

/// Die aktuelle Fassung eines Abschnitts mit allem, was [`decide`] und das Fortschreiben brauchen.
#[derive(Debug, Clone)]
pub(crate) struct CurrentState {
    pub id: i64,
    pub number: u32,
    pub body: String,
    pub by_human: bool,
    pub amend_seq: u32,
    /// Minuten seit dem letzten Schreiben: max(Anlage der Fassung, letzte Fortschreibung bzw. Rücknahme).
    pub minutes: u64,
}

type StateRow = (i64, i64, String, String, i64, Option<f64>);

/// Aktuelle Fassung des Abschnitts (`None` ohne aktuelle Fassung).
pub(crate) async fn current_state(
    conn: &mut SqliteConnection,
    node_id: i64,
) -> Result<Option<CurrentState>, StoreError> {
    let rows: Vec<StateRow> = sqlx::query_as(
        "SELECT r.id, r.number, r.body, r.author_kind, r.amend_seq, \
         (julianday('now') - max(julianday(r.created), \
              coalesce((SELECT max(julianday(a.created)) FROM revision_amend a WHERE a.revision_id = r.id), \
                       julianday(r.created)))) * 1440.0 \
         FROM revision r WHERE r.node_id = ? AND r.status = 'current'",
    )
    .bind(node_id)
    .fetch_all(&mut *conn)
    .await?;
    match rows.len() {
        0 => Ok(None),
        1 => {
            let (id, number, body, kind, amend_seq, minutes) = rows.into_iter().next().expect("eine Zeile");
            Ok(Some(CurrentState {
                id,
                number: to_u32(number, "Revisionsnummer")?,
                body,
                by_human: kind == "human",
                amend_seq: to_u32(amend_seq, "amend_seq")?,
                // Unlesbare Zeit (NULL) oder zurückgestellte Uhr (negativ): als „eben geschrieben“ zählen.
                minutes: minutes.filter(|m| m.is_finite() && *m > 0.0).map_or(0, |m| m.floor() as u64),
            }))
        }
        n => Err(broken(format!("Knoten {node_id}: {n} aktuelle Revisionen statt genau einer"))),
    }
}

/// Einstellung „Fassungen“ in der Transaktion des Aufrufers (fehlt die Zeile → Vorgabe).
pub(crate) async fn policy_in(conn: &mut SqliteConnection, work: WorkId) -> Result<RevisionPolicy, StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    if exists.is_none() {
        return Err(StoreError::UnknownWork(work));
    }
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT amend, max_change_percent, new_after_minutes FROM work_revision_policy WHERE work_id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(match row {
        None => RevisionPolicy::default(),
        Some((amend, percent, minutes)) => RevisionPolicy {
            amend: amend != 0,
            max_change_percent: u8::try_from(percent).map_err(|_| broken(format!("max_change_percent {percent}")))?,
            new_after_minutes: u32::try_from(minutes).map_err(|_| broken(format!("new_after_minutes {minutes}")))?,
        },
    })
}

/// Ergebnis der Prüfung vor dem Speichern im Modus `Auto`.
pub(crate) enum Verdict {
    /// Fortschreiben (die aktuelle Fassung des Menschen).
    Amend(CurrentState),
    /// Neue Fassung mit Grund.
    New(NewReason),
    /// Gleicher Text wie die aktuelle Fassung: nichts zu tun (der alte Weg meldet `None`).
    Unchanged,
}

/// Regeln aus Vertrag §2.2 für einen Text `stored` (so, wie er gespeichert würde) auf der Basis `based_on`.
///
/// `based_on_amend` ist der Stand der Basis, den der Editor geladen hatte (Nachbesserung R22): Ist die aktuelle
/// Fassung seither fortgeschrieben worden (anderer Stand), gilt die Basis als nicht aktuell (`NotOnCurrent`) – sonst
/// überschriebe ein veralteter Reiter die zwischenzeitliche Fortschreibung still. Fehlt der Stand (alte Clients,
/// RPC ohne `based_on_amend`), zählt wie bisher nur die Fassungsnummer.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn verdict(
    conn: &mut SqliteConnection,
    work: WorkId,
    node_id: i64,
    based_on: Option<u32>,
    based_on_amend: Option<u32>,
    stored: &str,
    heading_changed: bool,
) -> Result<Verdict, StoreError> {
    let policy = policy_in(conn, work).await?;
    let current = current_state(conn, node_id).await?;
    if current.as_ref().is_some_and(|c| c.body == stored) {
        return Ok(Verdict::Unchanged);
    }
    let facts = SaveFacts {
        has_current: current.is_some(),
        current_by_human: current.as_ref().is_some_and(|c| c.by_human),
        // Runde 23a (Codereview store-3): Der Stand zählt auch, wenn ein Client nur `based_on_amend` schickt (ohne
        // `based_on`): Er bezieht sich dann auf die aktuelle Fassung. Vorher wurde er in dem Fall übergangen.
        base_is_current: match (&current, based_on) {
            (Some(c), Some(base)) => c.number == base && based_on_amend.is_none_or(|seq| seq == c.amend_seq),
            (Some(c), None) => based_on_amend.is_none_or(|seq| seq == c.amend_seq),
            _ => true,
        },
        heading_changed,
        change_percent: current.as_ref().map_or(100.0, |c| change_percent(&c.body, stored)),
        minutes_since_last_write: current.as_ref().map_or(0, |c| c.minutes),
    };
    Ok(match (decide(&policy, SaveMode::Auto, &facts), current) {
        (None, Some(current)) => Verdict::Amend(current),
        (Some(reason), _) => Verdict::New(reason),
        // Von `decide` ausgeschlossen (ohne aktuelle Fassung gilt `NoCurrent`), zur Sicherheit.
        (None, None) => Verdict::New(NewReason::NoCurrent),
    })
}

/// Einen Schritt schreiben: Text ersetzen, `amend_seq` + 1, Zeile in `revision_amend` (Art `kind`). `edited` bleibt
/// (das setzt nur das Ändern über den Baum). Ergebnis: die Änderung mit dem neuen Stand.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn write_step(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    current: &CurrentState,
    after: &str,
    human: &str,
    kind: AmendKind,
    undo_of: Option<u32>,
) -> Result<Change, StoreError> {
    let seq = current.amend_seq + 1;
    let done = sqlx::query(
        "UPDATE revision SET body = ?, amend_seq = ? WHERE id = ? AND amend_seq = ? AND status = 'current'",
    )
    .bind(after)
    .bind(i64::from(seq))
    .bind(current.id)
    .bind(i64::from(current.amend_seq))
    .execute(&mut *conn)
    .await?;
    if done.rows_affected() != 1 {
        return Err(broken(format!("Abschnitt {node}: Fassung {} nicht fortgeschrieben", current.number)));
    }
    let author = if human.trim().is_empty() { crate::tree::HUMAN } else { human };
    sqlx::query(
        "INSERT INTO revision_amend (revision_id, seq, kind, undo_of, body_before, body_after, author_name) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(current.id)
    .bind(i64::from(seq))
    .bind(kind.as_db())
    .bind(undo_of.map(i64::from))
    .bind(&current.body)
    .bind(after)
    .bind(author)
    .execute(&mut *conn)
    .await?;
    Ok(Change {
        work,
        node,
        rev: current.number,
        author: Author::Human { name: author.to_owned() },
        kind: match kind {
            AmendKind::Amend => ChangeKind::Amended,
            AmendKind::Undo => ChangeKind::AmendUndone,
        },
        before: None,
        reparented: Vec::new(),
        amend_seq: Some(seq),
    })
}

/// Erste Überschrift (Ebene, Titel) eines Texts; ohne Überschrift am Anfang `None`.
pub(crate) fn heading_key(body: &str) -> Option<(u8, String)> {
    let doc = baruch_typst::parse(body).ok()?;
    if !doc.template.trim().is_empty() {
        return None;
    }
    doc.headings.into_iter().next().map(|h| (h.level, h.title))
}

type StepRow = (Option<String>, i64, String, Option<i64>, String, String, String, String);

async fn history(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
) -> Result<Vec<AmendStep>, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    let rows: Vec<StepRow> = sqlx::query_as(
        "SELECT uuid, seq, kind, undo_of, author_name, created, body_before, body_after \
         FROM revision_amend WHERE revision_id = ? ORDER BY seq",
    )
    .bind(row.id)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter()
        .map(|(uuid, seq, kind, undo_of, author, created, body_before, body_after)| {
            Ok(AmendStep {
                uuid: uuid.unwrap_or_default(),
                seq: to_u32(seq, "seq")?,
                kind: AmendKind::from_db(&kind).ok_or_else(|| broken(format!("unbekannte Art {kind:?}")))?,
                undo_of: undo_of.map(|u| to_u32(u, "undo_of")).transpose()?,
                author,
                created,
                body_before,
                body_after,
            })
        })
        .collect()
}

async fn undo(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    rev: u32,
    seq: u32,
    human: &str,
) -> Result<Change, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let row = write::row(conn, node_id, node, rev).await?;
    let Some(current) = current_state(conn, node_id).await? else {
        return Err(StoreError::Refused("Der Abschnitt hat keine aktuelle Fassung."));
    };
    if current.id != row.id {
        return Err(StoreError::Refused(
            "Rückgängig über den Verlauf geht nur auf der aktuellen Fassung; erst auf diese Fassung umschalten.",
        ));
    }
    if !current.by_human {
        return Err(StoreError::Refused(
            "Rückgängig über den Verlauf geht nur auf einer Fassung des Menschen (nicht auf Vorschlägen von Personas).",
        ));
    }
    let before: Option<String> =
        sqlx::query_scalar("SELECT body_before FROM revision_amend WHERE revision_id = ? AND seq = ?")
            .bind(row.id)
            .bind(i64::from(seq))
            .fetch_optional(&mut *conn)
            .await?;
    let Some(before) = before else {
        return Err(StoreError::Refused("Diesen Schritt gibt es im Verlauf dieser Fassung nicht."));
    };
    if before == current.body {
        return Err(StoreError::Refused("Der Text steht schon auf diesem Stand; nichts zurückzunehmen."));
    }
    // Eine Fortschreibung ändert nie die Überschrift; ein Stand davor hat also dieselbe. Zur Sicherheit geprüft.
    if heading_key(&before) != heading_key(&current.body) {
        return Err(StoreError::Refused(
            "Der Stand vor diesem Schritt hat eine andere Überschrift; bitte als neue Fassung speichern.",
        ));
    }
    write_step(conn, work, node, &current, &before, human, AmendKind::Undo, Some(seq)).await
}

/// Speichern des Menschen im Modus `Auto` ohne Gliederungslogik (RPC `revision.save`): fortschreiben oder neue
/// Fassung wie [`write::save_human`], mit Grund.
async fn save_auto(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
    author: &str,
    based_on: Option<u32>,
    based_on_amend: Option<u32>,
) -> Result<(Option<Change>, Option<NewReason>), StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    let body = body.replace("\r\n", "\n");
    if let Some(base) = based_on {
        write::row(conn, node_id, node, base).await?; // unbekannte Basis → UnknownRevision
    }
    let current_body = write::current(conn, node_id).await?.map(|(_, b, _)| b);
    let heading_changed = current_body.as_deref().is_some_and(|old| heading_key(old) != heading_key(&body));
    match verdict(conn, work, node_id, based_on, based_on_amend, &body, heading_changed).await? {
        Verdict::Amend(current) => {
            let change = write_step(conn, work, node, &current, &body, author, AmendKind::Amend, None).await?;
            Ok((Some(change), None))
        }
        Verdict::New(reason) => {
            let change = write::save_human(conn, work, node, &body, author, based_on).await?;
            let reason = change.as_ref().map(|_| reason);
            Ok((change, reason))
        }
        Verdict::Unchanged => Ok((None, None)),
    }
}

impl Store {
    /// Verlauf der Fortschreibungen der Fassung `rev` im Abschnitt `node` (seq), nach `seq`. Unbekannte Fassung →
    /// `UnknownRevision`; eine nie fortgeschriebene Fassung hat einen leeren Verlauf.
    pub async fn amend_history(&self, work: WorkId, node: usize, rev: u32) -> Result<Vec<AmendStep>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        history(&mut conn, work, node, rev).await
    }

    /// Setzt den Text der aktuellen Fassung des Menschen auf den Stand vor Schritt `seq` – als neuer Schritt
    /// (Art 'undo', `undo_of = seq`); Verlaufszeilen werden nie gelöscht. Nur auf der aktuellen Fassung und nur auf
    /// einer Fassung des Menschen, sonst `Refused` mit deutscher Meldung.
    pub async fn amend_undo(
        &self,
        work: WorkId,
        node: usize,
        rev: u32,
        seq: u32,
        human: &str,
    ) -> Result<Change, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = undo(&mut tx, work, node, rev, seq, human).await?;
        crate::outline::check_current_heading(&mut tx, work, node).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// Einstellung „Fassungen“ des Werks (fehlt die Zeile → Vorgabe). Unbekanntes Werk → `UnknownWork`.
    pub async fn revision_policy(&self, work: WorkId) -> Result<RevisionPolicy, StoreError> {
        let mut conn = self.pool.acquire().await?;
        policy_in(&mut conn, work).await
    }

    /// Einstellung „Fassungen“ des Werks setzen (nur Mensch). Grenzen nach [`RevisionPolicy::validate`]
    /// (`Refused` mit den Grenzen); `changed` nennt die geänderten Felder (leer = nichts geändert, dann wird auch
    /// nichts geschrieben).
    pub async fn set_revision_policy(
        &self,
        work: WorkId,
        policy: &RevisionPolicy,
        human: &str,
    ) -> Result<RevisionPolicyChange, StoreError> {
        if policy.validate().is_err() {
            let (lo, hi) = baruch_core::revision_policy::MAX_CHANGE_PERCENT;
            return Err(StoreError::Refused(if !(lo..=hi).contains(&policy.max_change_percent) {
                "Größte Änderung: 1 bis 100 Prozent."
            } else {
                "Neue Fassung nach: 0 bis 1440 Minuten (0 = keine Zeitgrenze)."
            }));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = policy_in(&mut tx, work).await?;
        let mut changed = Vec::new();
        if before.amend != policy.amend {
            changed.push("amend");
        }
        if before.max_change_percent != policy.max_change_percent {
            changed.push("max_change_percent");
        }
        if before.new_after_minutes != policy.new_after_minutes {
            changed.push("new_after_minutes");
        }
        if !changed.is_empty() {
            sqlx::query(
                "INSERT INTO work_revision_policy (work_id, amend, max_change_percent, new_after_minutes, updated_by) \
                 VALUES (?, ?, ?, ?, ?) ON CONFLICT(work_id) DO UPDATE SET amend = excluded.amend, \
                 max_change_percent = excluded.max_change_percent, new_after_minutes = excluded.new_after_minutes, \
                 updated = strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_by = excluded.updated_by",
            )
            .bind(work)
            .bind(i64::from(policy.amend))
            .bind(i64::from(policy.max_change_percent))
            .bind(i64::from(policy.new_after_minutes))
            .bind(human.chars().take(200).collect::<String>())
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Ok(RevisionPolicyChange { work, before, after: *policy, changed })
    }

    /// Speichern des Menschen mit Modus: `Auto` schreibt fort, wenn die Regeln es erlauben, `New` legt wie
    /// [`Store::save_human`] bzw. [`Store::save_human_on`] eine neue Fassung an.
    pub async fn save_human_mode(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: Option<u32>,
        mode: SaveMode,
    ) -> Result<Option<Change>, StoreError> {
        self.save_human_with_reason(work, node, body, author, based_on, mode).await.map(|(change, _)| change)
    }

    /// Wie [`Store::save_human_mode`], dazu der Grund einer neuen Fassung (nur im Modus `Auto`, sonst `None`).
    pub async fn save_human_with_reason(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: Option<u32>,
        mode: SaveMode,
    ) -> Result<(Option<Change>, Option<NewReason>), StoreError> {
        self.save_human_at(work, node, body, author, based_on, None, mode).await
    }

    /// Wie [`Store::save_human_with_reason`] mit dem Stand der Basis (`based_on_amend`), den der Client geladen
    /// hatte: Ist die aktuelle Fassung seither fortgeschrieben, entsteht im Modus `Auto` eine neue Fassung
    /// (`NotOnCurrent`) statt einer stillen Fortschreibung über den zwischenzeitlichen Text.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_human_at(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: Option<u32>,
        based_on_amend: Option<u32>,
        mode: SaveMode,
    ) -> Result<(Option<Change>, Option<NewReason>), StoreError> {
        if mode == SaveMode::New {
            let change = match based_on {
                Some(base) => self.save_human_on(work, node, body, author, base).await?,
                None => self.save_human(work, node, body, author).await?,
            };
            return Ok((change, None));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = save_auto(&mut tx, work, node, body, author, based_on, based_on_amend).await?;
        if done.0.is_some() {
            crate::outline::check_current_heading(&mut tx, work, node).await?;
            tx.commit().await?;
        }
        Ok(done)
    }

    /// Speichern aus dem Editor mit Modus (wie [`Store::outline_save`], im Modus `Auto` mit Fortschreiben;
    /// `OutlineSave::new_reason` nennt dann den Grund einer neuen Fassung).
    #[allow(clippy::too_many_arguments)]
    pub async fn outline_save_mode(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        based_on: Option<u32>,
        allow: Restructure,
        human: &str,
        mode: SaveMode,
    ) -> Result<OutlineSave, StoreError> {
        self.outline_save_at(work, node, body, based_on, None, allow, human, mode).await
    }

    /// Wie [`Store::outline_save_mode`] mit dem Stand der Basis (`based_on_amend`), den der Editor geladen hatte
    /// (Formularfeld `based_on_amend`): Ist die aktuelle Fassung seither fortgeschrieben, entsteht im Modus `Auto`
    /// eine neue Fassung (`NotOnCurrent`), beide Texte bleiben als Fassungen sichtbar.
    #[allow(clippy::too_many_arguments)]
    pub async fn outline_save_at(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        based_on: Option<u32>,
        based_on_amend: Option<u32>,
        allow: Restructure,
        human: &str,
        mode: SaveMode,
    ) -> Result<OutlineSave, StoreError> {
        if mode == SaveMode::New {
            return self.outline_save(work, node, body, based_on, allow, human).await;
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = crate::outline::save_auto(&mut tx, work, node, body, based_on, based_on_amend, allow, human).await?;
        if done.change.is_some() || done.outline.is_some() {
            tx.commit().await?;
        }
        Ok(done)
    }
}
