// SPDX-License-Identifier: AGPL-3.0-or-later
//! Anträge zur Gliederung (Migration 0012, Wunsch 2026-11-06 „UD nach Bestätigung“).
//!
//! Assistenten und MCP-Clients dürfen Gliederungspunkte anlegen ([`Store::outline_add_many`]), aber
//! Umbenennen, Verschieben, Umstufen und Löschen ändern nichts sofort: [`Store::outline_request`] legt
//! einen **Antrag** an. Der Mensch bestätigt oder lehnt ab ([`Store::outline_request_decide`]).
//!
//! Regeln:
//! - Der Antrag nennt Punkte nur über ihren festen Schlüssel (`node.id`) und merkt sich den Stand beim
//!   Antrag ([`Expected`]: Titel und Ebene des Punkts, Titel des Ziels beim Verschieben).
//! - Beim Antrag läuft die Änderung probeweise in einem Sicherungspunkt (danach zurückgerollt): Was
//!   jetzt schon nicht ginge (Titelregeln, Ebenen, Löschen mit Unterpunkten, keine Wirkung), wird
//!   gleich abgelehnt und nicht gespeichert. Ein gleicher offener Antrag wird nicht doppelt angelegt.
//! - Bestätigen prüft zuerst, ob der Antrag veraltet ist (Punkt oder Ziel weg, Titel oder Ebene
//!   inzwischen anders); dann wird er als `stale` mit Meldung abgeschlossen und nichts geändert. Sonst
//!   läuft die Gliederungsfunktion unter dem Namen des Menschen; lehnt sie ab, ebenfalls `stale` mit
//!   ihrer Meldung. Ablehnen ändert nur den Antrag.
//! - Über einen Antrag wird genau einmal entschieden.
//! - Vorschau (Runde 14, [`Store::outline_request_preview`]): die Gliederung, wie sie nach dem
//!   Bestätigen wäre, im Sicherungspunkt gerechnet und zurückgerollt (die Datenbank bleibt); je Punkt
//!   Ebene, Nummer, Titel und eine Marke ([`PreviewMark`]), gelöschte Punkte an ihrer alten Stelle.
//!   Seit Runde 16c zusätzlich Art (`kind_word`) und Nummer/Ebene vor dem Bestätigen, damit „Teil 1“ und
//!   „Kapitel 1“ unterscheidbar sind und übernommene Kapitel „vorher → nachher“ zeigen.
//! - Zustimmung zu einem neuen Teil (Nutzerfrage 12, Entscheidung des Nutzers 2026-11-07: „es braucht
//!   eine aktive Zustimmung (Popup)“, Migration 0035): Legt ein Assistent einen Teil so an, dass
//!   vorhandene Kapitel unter ihn wandern, ist das kein bloßes Anlegen mehr.
//!   [`Store::outline_add_by_assistant`] legt dann nichts an, sondern einen Antrag [`OutlineOp::Add`]
//!   mit den Kapiteln, die er übernähme (`Expected::adopted`). Erst die Zustimmung des Menschen führt
//!   ihn aus; Revision 1 der neuen Punkte trägt dann den Assistenten (er hat sie verfasst), der Commit
//!   den Menschen als Bestätiger. Übernähme der Teil beim Zustimmen andere Kapitel als beim Antrag,
//!   wird er als veraltet abgeschlossen und nichts geändert.
//! - Neuer Teil aus dem Austausch mit Kennung und Fassungen (Runde 22, Vertrag §2.10): Übernähme ein eingelesener
//!   neuer Teil vorhandene Kapitel, trägt der Antrag `Add` in `part` die Kennung des Teils und alle seine Fassungen
//!   ([`PartIdentity`]: Nummer, Text, Herkunft, Status, Basis, Stand und Fortschreibungen). Beim Zustimmen legt
//!   `crate::exchange_apply::adopt_part` den Teil mit **dieser** Kennung und **diesen** Fassungen an (die von
//!   `add_many` erzeugte Fassung 1 wird ersetzt, ohne Grabstein); ein späterer Abgleich erkennt ihn so wieder.
//!   Gibt es die Kennung inzwischen hier, ist der Antrag veraltet. Ablehnen legt nichts an.

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

use baruch_core::Author;

use crate::outline::{self, NewPoint, OutlineAdded, OutlineChange, Place, PointState};
use crate::{IdeaOrigin, Store, StoreError, WorkId, broken};

/// Was ein Antrag ändern will; Punkte über ihren festen Schlüssel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OutlineOp {
    /// Umbenennen.
    Rename { key: i64, title: String },
    /// Verschieben vor/hinter/unter `target_key` (die Ebene bleibt, wie bei `outline.move`).
    Move { key: i64, target_key: i64, place: Place },
    /// Ebene 1–3 setzen, Unterpunkte wandern mit.
    SetLevel { key: i64, level: u8 },
    /// Löschen (nur ohne Unterpunkte, nicht das letzte Kapitel).
    Delete { key: i64 },
    /// Mit (`true`) bzw. ohne Nummer (`false`) zählen.
    SetNumbered { key: i64, numbered: bool },
    /// Neue Punkte (darunter mindestens ein Teil) vor/hinter `key` anlegen, die vorhandene Kapitel
    /// übernähmen (Nutzerfrage 12); `model` für Revision 1 (Autor ist der Antragsteller). Runde 22: `part`
    /// = Kennung und Fassungen eines Teils aus dem Austausch (dann entsteht er genau so).
    Add {
        key: i64,
        place: Place,
        points: Vec<NewPoint>,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<PartIdentity>,
    },
}

/// Kennung und Fassungen eines neuen Teils aus dem Austausch (Runde 22, Vertrag §2.10), gespeichert in `params`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartIdentity {
    /// Kennung des Teils (`node.uuid`).
    pub uuid: String,
    /// Fassungen nach Nummer, genau eine aktuelle.
    pub revisions: Vec<PartRevision>,
}

/// Eine Fassung eines Teils aus dem Austausch (Spalten wie `revision`; Basis über die Kennung).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartRevision {
    pub uuid: String,
    pub number: u32,
    pub body: String,
    /// `human` oder `assistant`.
    pub author_kind: String,
    pub author_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `current`, `proposed`, `rejected` oder `superseded`.
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

/// Ein Schritt im Verlauf einer Fassung eines Teils aus dem Austausch (Spalten wie `revision_amend`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartAmend {
    pub uuid: String,
    pub seq: u32,
    /// `amend` oder `undo`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undo_of: Option<u32>,
    pub author: String,
    pub created: String,
    pub before: String,
    pub after: String,
}

impl OutlineOp {
    /// Wert der Spalte `op` (`rename`, `move`, `set_level`, `delete`, `set_numbered`, `add`).
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

    /// Fester Schlüssel des betroffenen Punkts.
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

    /// Braucht der Antrag die aktive Zustimmung des Menschen im Dialog (Nutzerfrage 12)? Bisher nur
    /// ein neuer Teil, der vorhandene Kapitel übernähme.
    pub fn needs_consent(&self) -> bool {
        matches!(self, OutlineOp::Add { .. })
    }
}

/// Stand beim Antrag: daran erkennt die Bestätigung einen veralteten Antrag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expected {
    pub title: String,
    pub level: u8,
    /// Titel des Ziels beim Verschieben.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_title: Option<String>,
    /// Beim Anlegen eines Teils (Nutzerfrage 12): feste Schlüssel der vorhandenen Kapitel, die unter
    /// den neuen Teil wandern würden – genau diese zeigt die Vorschau, genau diese muss das Zustimmen
    /// übernehmen.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adopted: Vec<i64>,
}

/// Entscheidung über einen Antrag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Bestätigt und ausgeführt.
    Confirmed,
    /// Vom Menschen abgelehnt.
    Rejected,
    /// Beim Bestätigen veraltet oder nicht mehr ausführbar (`message` sagt, warum); nichts geändert.
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

    /// Anzeige: „bestätigt“, „abgelehnt“, „veraltet“.
    pub fn label(self) -> &'static str {
        match self {
            Decision::Confirmed => "bestätigt",
            Decision::Rejected => "abgelehnt",
            Decision::Stale => "veraltet",
        }
    }
}

/// Ein Antrag zur Gliederung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutlineRequest {
    pub id: i64,
    pub work: WorkId,
    pub op: OutlineOp,
    pub expected: Expected,
    /// Antragsteller (Persona oder MCP-Client; der Mensch ändert direkt).
    pub origin: IdeaOrigin,
    pub note: String,
    pub created: String,
    /// `None` = offen.
    pub decided: Option<String>,
    pub decision: Option<Decision>,
    pub decided_by: Option<String>,
    pub message: Option<String>,
    /// Deutsch für Liste und Commit, z. B. „„Alt“ in „Neu“ umbenennen“.
    pub summary: String,
}

impl OutlineRequest {
    /// Noch nicht entschieden?
    pub fn is_open(&self) -> bool {
        self.decision.is_none()
    }
}

/// Ergebnis von [`Store::outline_request_decide`]: der Antrag danach und, wenn bestätigt und
/// ausgeführt, die Gliederungsänderung (für Antwort und Spiegel).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RequestDecided {
    pub request: OutlineRequest,
    pub change: Option<OutlineChange>,
}

/// Was ein Antrag an einem Punkt der Vorschau ändert (Runde 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewMark {
    /// Bleibt (Nummer und Stelle können sich trotzdem ändern, z. B. nach einem Löschen).
    Same,
    /// Neuer Titel.
    Renamed,
    /// Verschoben (der Punkt des Antrags samt Unterpunkten).
    Moved,
    /// Andere Ebene (beim Umstufen auch die mitwandernden Unterpunkte).
    Level,
    /// Zählt jetzt mit bzw. ohne Nummer.
    Numbered,
    /// Entfällt (an seiner alten Stelle gezeigt, mit alter Ebene und Nummer).
    Deleted,
    /// Neu angelegt (Antrag [`OutlineOp::Add`]).
    Added,
    /// Vorhandenes Kapitel, das unter den neuen Teil wandert (Antrag [`OutlineOp::Add`]).
    Adopted,
}

/// Ein Punkt der Gliederung nach dem Bestätigen eines Antrags (Vorschau, Runde 14).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PreviewPoint {
    pub key: i64,
    pub level: u8,
    pub number: String,
    pub title: String,
    pub mark: PreviewMark,
    /// Art des Punkts nach dem Bestätigen: „Teil“, „Kapitel“, „Abschnitt“, „Unterabschnitt“ (Runde 16c:
    /// zusammen mit `number` eindeutig, z. B. „Teil 1“ und „Kapitel 1“).
    pub kind_word: &'static str,
    /// Nummer vor dem Bestätigen (`None` = neuer Punkt); bei gelöschten Punkten wie `number`.
    pub before_number: Option<String>,
    /// Ebene vor dem Bestätigen (`None` = neuer Punkt).
    pub before_level: Option<u8>,
}

impl PreviewPoint {
    /// Art und Nummer, z. B. „Teil 1“, „Kapitel 2“; ohne Nummer „Kapitel (ohne Nummer)“.
    pub fn label(&self) -> String {
        label(self.kind_word, &self.number)
    }

    /// Art und Nummer vor dem Bestätigen (`None` = neuer Punkt).
    pub fn before_label(&self) -> Option<String> {
        let level = self.before_level?;
        Some(label(outline::level_word(level), self.before_number.as_deref().unwrap_or("")))
    }
}

fn label(kind_word: &str, number: &str) -> String {
    let number = number.trim().trim_end_matches('.');
    if number.is_empty() { format!("{kind_word} (ohne Nummer)") } else { format!("{kind_word} {number}") }
}

/// Vorschau aus den Gliederungen vor und nach dem Probelauf: Reihenfolge von danach, gelöschte Punkte
/// hinter ihrem alten Vorgänger (bzw. am Anfang).
fn preview_points(op: &OutlineOp, adopted: &[i64], before: &[PointState], after: &[PointState]) -> Vec<PreviewPoint> {
    let old: std::collections::HashMap<i64, &PointState> = before.iter().map(|p| (p.key, p)).collect();
    // Beim Verschieben: der Punkt und seine Unterpunkte (folgende Punkte mit größerer Ebene).
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

/// Gespeicherte Form in `params`.
#[derive(Serialize, Deserialize)]
struct Stored {
    op: OutlineOp,
    expected: Expected,
}

#[cfg(test)]
fn summary(op: &OutlineOp, expected: &Expected) -> String {
    summary_for(op, expected, false)
}

/// Wie [`summary`]; `sermon`: das Werk ist eine Predigt, dann heißen die wandernden Punkte „Predigtteile“
/// statt „Kapitel“ (Runde 17b, L8).
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
            // Runde 22: Teil aus dem Austausch mit Kennung und Fassungen.
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

/// Spalten eines Antrags, zuletzt die Werkart (für die Wortwahl der Zusammenfassung, Runde 17b).
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
    let row: Option<RequestRow> =
        sqlx::query_as(&format!("SELECT {COLUMNS} FROM outline_request WHERE id = ? AND work_id = ?"))
            .bind(id)
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
    from_row(row.ok_or(StoreError::UnknownRequest { work, id })?)
}

/// Stand des Punkts `key` oder `GoneNode`.
async fn state_of(conn: &mut SqliteConnection, work: WorkId, key: i64) -> Result<(usize, u8, String), StoreError> {
    outline::point_state(conn, work, key).await?.ok_or(StoreError::GoneNode { work, key })
}

/// Antrag ausführen (Schlüssel → Stellen im selben Zustand), unter dem Namen `human`; beim Anlegen
/// ([`OutlineOp::Add`]) ist der Antragsteller `origin` Autor von Revision 1. Dazu die festen Schlüssel
/// der übernommenen Kapitel (nur beim Anlegen, sonst leer).
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
            // Runde 22: Teil aus dem Austausch mit seiner Kennung und seinen Fassungen.
            if let Some(part) = part {
                crate::exchange_apply::adopt_part(conn, work, &added, part).await?;
            }
            return Ok((added.change, added.adopted));
        }
    }?;
    Ok((change, Vec::new()))
}

/// Übernähme ein Antrag [`OutlineOp::Add`] jetzt andere Kapitel als beim Antrag? Meldung, sonst `None`.
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

/// Ist der Antrag veraltet? Meldung, sonst `None`.
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

/// Fachliche Ablehnung einer Gliederungsfunktion (dann ist der Antrag nicht ausführbar)?
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

/// Antrag speichern (in der Transaktion des Aufrufers); ein gleicher offener Antrag → `Refused`.
/// Runde 23: höchstens so viele **offene** Anträge zur Gliederung je Werk und Herkunft (Persona bzw. MCP-Client).
/// Jeder offene Antrag kostet auf `/gliederung` eine Vorschau unter der Schreibsperre; ohne Grenze könnte ein Client
/// die Seite mit vielen verschiedenen Anträgen lahmlegen.
pub const MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN: usize = 50;

/// Meldung zu [`MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN`] (`Refused` trägt nur festen Text).
const TOO_MANY_OPEN: &str = "Zu viele offene Anträge zur Gliederung von dieser Herkunft in diesem Werk (höchstens 50). \
Nichts angelegt; erst entscheidet der Mensch über die offenen (outline_requests zeigt sie).";

/// Grenze [`MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN`] prüfen (in der Transaktion, vor dem Anlegen). Der Mensch ist
/// ausgenommen; das Einlesen einer Austauschdatei ([`outline_request_in`], [`add_request_in`]) prüft sie nicht, damit
/// eine Datei mit vielen offenen Anträgen einlesbar bleibt.
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

/// Antrag zur Gliederung anlegen, in der Transaktion des Aufrufers (Kern von [`Store::outline_request`], auch für
/// das Einlesen einer Austauschdatei, Runde 26). Regeln wie dort: Punkt und Ziel müssen es geben, die Änderung
/// muss im Probelauf (Sicherungspunkt, zurückgerollt) gehen, ein gleicher offener Antrag → `Refused`.
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
    // Titel wie überall prüfen und getrimmt speichern. Anlegen läuft über
    // `outline_add_by_assistant` (dort wird geprüft, ob es überhaupt ein Antrag wird).
    let op = match op {
        OutlineOp::Rename { key, title } => OutlineOp::Rename { key, title: outline::check_title(&title)? },
        OutlineOp::Add { .. } => {
            return Err(StoreError::Refused("Anlegen beantragt man über outline_add, nicht als Antrag."));
        }
        other => other,
    };
    {
        // Probelauf im Sicherungspunkt: Was jetzt schon nicht ginge, wird gar nicht erst Antrag.
        let mut probe = sqlx::Connection::begin(&mut *conn).await?;
        let tried = run(&mut probe, work, &op, origin, "Probelauf").await;
        probe.rollback().await?;
        tried?;
    }
    let expected = Expected { title, level, target_title, adopted: Vec::new() };
    insert_request(conn, work, &op, &expected, origin, note).await
}

/// Antrag [`OutlineOp::Add`] für neue Punkte vor/hinter `key`, in der Transaktion des Aufrufers (Austausch, Runde
/// 26: ein neuer Teil aus der Datei, der vorhandene Kapitel übernähme). Probelauf wie bei
/// [`Store::outline_add_by_assistant`] (Sicherungspunkt, immer zurückgerollt): Übernähmen die Punkte keine
/// vorhandenen Kapitel, entsteht kein Antrag (`Ok(None)`, nichts geschrieben); sonst der Antrag mit genau diesen
/// Kapiteln. Fehler des Probelaufs (Ebenen, Ort) und ein gleicher offener Antrag (`Refused`) wie dort.
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

/// Ergebnis von [`Store::outline_add_by_assistant`]: angelegt (kein vorhandenes Kapitel wandert) oder
/// ein Antrag, dem der Mensch zustimmen muss (Nutzerfrage 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantAdd {
    Added(OutlineAdded),
    Requested(OutlineRequest),
}

impl Store {
    /// Gliederungspunkte durch einen Assistenten anlegen (MCP `outline_add`, atomar): wie
    /// [`Store::outline_add_many`] mit dem Antragsteller `origin` und `model` als Autor von Revision 1 –
    /// außer ein neuer Teil übernähme vorhandene Kapitel (Nutzerfrage 12, 2026-11-07). Dann ändert sich
    /// nichts, sondern es entsteht ein Antrag [`OutlineOp::Add`] mit den Kapiteln, die er übernähme,
    /// dem der Mensch zustimmt ([`Store::outline_request_decide`]). Fehler (Titelregeln, Ebenen, Teil an
    /// falscher Stelle, gleicher offener Antrag) wie dort; dann ist nichts angelegt.
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
            // Probelauf im Sicherungspunkt: Übernimmt nichts, gilt er; sonst zurück und Antrag.
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
        // Übernommen werden nur vorhandene Kapitel; ohne Bezugspunkt gibt es keine.
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

    /// Antrag zur Gliederung anlegen (atomar). Der Punkt (und beim Verschieben das Ziel) muss es
    /// geben, sonst `GoneNode`; die Änderung muss jetzt ausführbar sein (Probelauf, zurückgerollt),
    /// sonst deren Fehler (z. B. Titelregeln → `Title`, Löschen mit Unterpunkten → `Refused`); ein
    /// gleicher offener Antrag → `Refused`. Ändert an der Gliederung nichts. Runde 23: Hat die Herkunft (Persona
    /// bzw. MCP-Client) im Werk schon [`MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN`] offene Anträge → `Refused`.
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

    /// Anträge eines Werks nach id; `open_only` nur die offenen.
    pub async fn outline_requests(&self, work: WorkId, open_only: bool) -> Result<Vec<OutlineRequest>, StoreError> {
        self.work_slug(work).await?;
        let filter = if open_only { " AND decided IS NULL" } else { "" };
        let rows: Vec<RequestRow> =
            sqlx::query_as(&format!("SELECT {COLUMNS} FROM outline_request WHERE work_id = ?{filter} ORDER BY id"))
                .bind(work)
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter().map(from_row).collect()
    }

    /// Offene Anträge aller Werke, die die aktive Zustimmung des Menschen brauchen (Nutzerfrage 12:
    /// Art `add`), nach id – Hinweisleiste und Dialog auf jeder Seite.
    pub async fn outline_requests_needing_consent(&self) -> Result<Vec<OutlineRequest>, StoreError> {
        let rows: Vec<RequestRow> = sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM outline_request WHERE decided IS NULL AND op = 'add' ORDER BY id"
        ))
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(from_row).collect()
    }

    /// Zahl der offenen Anträge eines Werks (Hinweis im Gliederungsbaum).
    pub async fn open_outline_requests(&self, work: WorkId) -> Result<usize, StoreError> {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM outline_request WHERE work_id = ? AND decided IS NULL")
                .bind(work)
                .fetch_one(&self.pool)
                .await?;
        crate::to_usize(count, "Anträge")
    }

    /// Vorschau eines offenen Antrags (Runde 14): die Gliederung, wie sie nach
    /// [`Store::outline_request_decide`] mit `confirm` wäre – im Sicherungspunkt gerechnet und
    /// zurückgerollt, die Datenbank bleibt unverändert. Gelöschte Punkte stehen an ihrer alten Stelle
    /// mit [`PreviewMark::Deleted`]. Schon entschieden → `Refused`; veraltet → `Stale` („Veraltet: <Grund>“,
    /// Runde 23, vorher `Heading`); nicht mehr ausführbar → `Heading` mit dem Grund (beide abgelehnt wie
    /// `Refused`, -32000; `Refused` trägt nur feste Texte); unbekannt → `UnknownRequest`.
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

    /// Über einen offenen Antrag entscheiden (atomar, nur der Mensch `human`). `confirm = false` lehnt
    /// ab und ändert sonst nichts. `confirm = true` führt ihn aus, wenn er nicht veraltet ist; veraltet
    /// oder nicht mehr ausführbar → Entscheidung `stale` mit Meldung, die Gliederung bleibt. Schon
    /// entschieden → `Refused`, unbekannt → `UnknownRequest`.
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
