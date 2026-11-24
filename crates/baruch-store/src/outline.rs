// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederungsmanager (Vertrag Runde 4, Abschnitte 2 und 3c): Verschieben, Ebene ändern, Umbenennen,
//! Anlegen, Löschen, Nummerierung und Überschriften im Text (Teilen, Zusammenführen).
//!
//! Regeln:
//! - Die Gliederung ist die **Ebenenfolge in Baumreihenfolge**: Teil 0 (nur oberste Ebene, seit
//!   Runde 14 mit der Teil-Markierung [`baruch_typst::part_line`] als Überschriftszeile; Altbestand
//!   ohne Markierung bleibt, wie er ist), Kapitel 1 (`= `), Abschnitt 2 (`== `), Unterabschnitt 3
//!   (`=== `; je auch als Markierung „ohne Nummer“ [`baruch_typst::unnumbered_line`]). Elternteil
//!   ist wie beim Import der nächste Vorgänger mit kleinerer Ebene. Gültig ist eine Folge, in der jede
//!   Ebene höchstens eine tiefer steht als die vorige (der erste Nicht-Teil also ein Kapitel) und
//!   keine tiefer als 3. Ungültig → [`StoreError::Refused`], nichts geändert.
//! - Jede Änderung vergibt `seq`, `parent_seq` und `position` in Baumreihenfolge neu (zweistufig über
//!   negative Werte wegen `UNIQUE(work_id, seq)`), schreibt `node.number` und `post_ref.node_seq` mit
//!   (Verweise auf gelöschte Knoten entfallen).
//! - Ändern sich Ebene oder Titel eines Knotens mit Text, bekommt er eine neue Fassung des Menschen
//!   (aktuell, Elternteil = die bisherige aktuelle), die sich nur in der Überschriftszeile
//!   unterscheidet. Sonst ändert sich kein Text.
//! - Ganze Knoten löschen: in derselben Transaktion erst die aktuelle Fassung `current → superseded`
//!   (erlaubt laut Migration 0004), dann Revisionen, Verweise, Beiträge und den Knoten. So bleibt der
//!   Trigger `revision_no_delete_current` für lebende Abschnitte wirksam.
//! - Teile (Runde 14): Ein neuer Teil steht vor oder hinter einem Teil oder Kapitel, auch in Werken
//!   mit Werkvorlage; Revision 1 ist `heading_line(0, Titel, None) + "\n"` (Teil-Markierung und
//!   Leerzeile). Die folgenden Kapitel bis zum nächsten Teil stehen dann unter ihm (`adopted`), ihre
//!   Texte und Nummern bleiben (Kapitel zählen über Teile hinweg). Umbenennen schreibt die
//!   Markierungszeile neu (leerer Text → Markierung); ein Teiltext ohne Markierung (Altbestand
//!   „= Teil …“) bleibt unverändert, nur der Titel ändert sich. Speichern, Vorschläge und Umschalten
//!   gleichen Teile **mit** Markierung ab (genau eine Markierung am Anfang, keine weitere
//!   Überschrift; ein anderer Titel benennt um); Teiltexte ohne Markierung wie bisher ohne Abgleich.
//!   Zum Teil umstufen oder einen Teil umstufen geht nie.
//! - Anlegen dürfen auch Assistenten und MCP-Clients ([`Store::outline_add_many`], Wunsch 2026-11-06:
//!   Revision 1 trägt dann ihren Namen). Umbenennen, Verschieben, Umstufen und Löschen nimmt nur der
//!   Mensch; Assistenten stellen dafür einen Antrag (Modul `requests`, Migration 0012), den der Mensch
//!   bestätigt – erst dann laufen die Funktionen hier, unter dem Namen des Menschen. Ausnahme
//!   (Nutzerfrage 12, 2026-11-07): Ein Teil, den ein Assistent anlegt und der vorhandene Kapitel
//!   übernähme, wird ebenfalls ein Antrag (`Store::outline_add_by_assistant`).
//! - Gliederung und Überschriftszeilen sind gleich: Wird die aktuelle Fassung außerhalb des Editors
//!   gespeichert (`revision.save`, Fassung ändern) oder ein Vorschlag angelegt, muss der Text genau
//!   eine Überschrift der Ebene des Punkts tragen ([`check_current_heading`],
//!   [`check_proposal_heading`]); nur ein anderer Titel derselben Ebene benennt den Punkt um.
//! - Umschalten und Übernehmen (Vertrag Runde 5, [`follow_switched`]): Die Fassung bringt ihre Ebene
//!   mit. Die Gliederung folgt Ebene und Titel ihrer Überschriftszeile, Unterpunkte werden mitgestuft;
//!   die Fassung selbst wird nie umgeschrieben.
//! - Verschieben behält die Ebene: vor/hinter nur einen Punkt derselben Ebene, unter nur einen Punkt
//!   genau eine Ebene höher ([`MOVE_KEEPS_LEVEL`]).
//! - Titelregeln (Vertrag Runde 7, [`check_title`]): leer, nur Leerraum, ein Zeilenumbruch oder mehr
//!   als [`MAX_TITLE_CHARS`] Zeichen → [`StoreError::Title`], nichts geändert; beim Umbenennen,
//!   Anlegen, Speichern im Editor (jeder Titel, auch geteilte), Prüfen (`valid: false`), Speichern und
//!   Ändern der aktuellen Fassung, in Vorschlägen und beim Umschalten. Titel stammen nur aus echten
//!   Überschriftszeilen ([`baruch_typst::parse`]). Import und Export prüfen nicht (byte-gleich), und
//!   vorhandene leere Titel bleiben, bis der Mensch umbenennt. Seit Runde 8 prüfen Speichern, Prüfen,
//!   Ändern der aktuellen Fassung, Vorschläge und Umschalten nur **geänderte** Titel: Eine unveränderte
//!   erste Überschrift (z. B. ein importierter leerer oder überlanger Titel) hält das Speichern des
//!   Texts nicht auf; neue (geteilte) und abweichende Titel werden weiter geprüft.

use std::collections::HashMap;

use baruch_core::{Author, NodeKind};
use baruch_typst::scheme::{Numbering, Scheme};
use sqlx::SqliteConnection;

use serde::Serialize;

use crate::load::{self, Numbered};
use crate::{Change, Idea, IdeaId, IdeaUse, Store, StoreError, WorkId, broken, to_usize, words, write};

/// Wohin ein Gliederungspunkt relativ zum Ziel kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// Vor das Ziel, auf dessen Ebene.
    Before,
    /// Hinter das Ziel samt Unterpunkten, auf dessen Ebene.
    After,
    /// Als letzter Unterpunkt unter das Ziel (Ebene des Ziels + 1).
    Into,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutlineAction {
    Moved,
    Level,
    Renamed,
    Added,
    Deleted,
    Split,
    Merged,
    Numbering,
    /// Ein Punkt zählt mit bzw. ohne Nummer (`node.numbered`, z. B. Vorwort, Wunsch 2026-11-06).
    Numbered,
}

impl OutlineAction {
    /// Wert des Trailers `Baruch-Action` im Git-Spiegel.
    pub fn trailer(self) -> &'static str {
        match self {
            OutlineAction::Moved => "outline-move",
            OutlineAction::Level => "outline-level",
            OutlineAction::Renamed => "outline-rename",
            OutlineAction::Added => "outline-add",
            OutlineAction::Deleted => "outline-delete",
            OutlineAction::Split => "outline-split",
            OutlineAction::Merged => "outline-merge",
            OutlineAction::Numbering => "outline-numbering",
            OutlineAction::Numbered => "outline-numbered",
        }
    }
}

/// Ergebnis einer Gliederungsänderung (für Antwort und Spiegel).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineChange {
    pub work: WorkId,
    pub action: OutlineAction,
    /// seq des Knotens danach; bei `Deleted`/`Merged` der jetzt gezeigte (der vorige).
    pub node: usize,
    /// Titel des Knotens (bei `Deleted`/`Merged` des entfernten).
    pub title: String,
    /// Für den Betreff, z. B. `„Sommer“ vor „Frühling“ verschoben`.
    pub detail: String,
    /// Neue Fassungen `(seq, Nummer)`.
    pub revisions: Vec<(usize, u32)>,
    /// Umnummerierte Knoten `[alt, neu]`.
    pub renumbered: Vec<(usize, usize)>,
    /// Entfernte Knoten: Titel und Text der gelöschten aktuellen Fassung.
    pub removed: Vec<(String, String)>,
}

/// Was [`Store::outline_save`] an der Gliederung ändern darf.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Restructure {
    /// Weitere Überschriften im Text teilen den Abschnitt.
    pub split: bool,
    /// Ohne Überschrift wird der Text an den vorigen Gliederungspunkt angehängt.
    pub merge: bool,
}

/// Ergebnis von [`Store::outline_check`] (nur lesen).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct HeadingCheck {
    /// Titel und Ebene der ersten Überschrift im Text.
    pub title: Option<String>,
    pub level: Option<u8>,
    /// Titel weiterer Überschriften: Speichern teilt den Abschnitt (mit Bestätigung).
    pub split: Vec<String>,
    /// Keine Überschrift: Speichern führt mit dem vorigen Gliederungspunkt zusammen (mit Bestätigung).
    pub merge: bool,
    /// Ergäbe das Speichern eine gültige Gliederung?
    pub valid: bool,
    pub message: Option<String>,
}

/// Ergebnis von [`Store::outline_save`]: die gespeicherte Fassung und, falls sich die Gliederung
/// geändert hat, deren Änderung.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineSave {
    pub change: Option<Change>,
    pub outline: Option<OutlineChange>,
    /// Runde 22: Grund der neuen Fassung, nur bei Modus Auto und neuer Fassung.
    pub new_reason: Option<crate::NewReason>,
}

/// Nummerierung eines Werks (für Anzeige und Formular).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NumberingInfo {
    /// Eingestellte Nummerierung in der Mustersprache: `None` = aus der Werkvorlage (oder ein Schema
    /// mit Wörtern ohne Muster, siehe `scheme`), `""` = keine Nummern, sonst das Muster.
    pub numbering: Option<String>,
    /// Muster aus `#set heading(numbering: …)` der Werkvorlage, falls vorhanden.
    pub template: Option<String>,
    /// Anzeigetext: gültiges Muster (Vorgabe „1.1“), `""` = keine Nummern, bei Schemata mit Wörtern
    /// deren Beispiel.
    pub effective: String,
    /// Gliederungsschema (id, Migration 0008) und sein Name.
    pub scheme: String,
    pub scheme_label: String,
    /// Gespeichertes eigenes Muster (`work.numbering`), auch wenn gerade ein anderes Schema gilt.
    pub custom: Option<String>,
    /// Anzeigename für den Gliederungskopf (Runde 7): Name des Schemas, dessen Muster dem wirksamen
    /// entspricht ([`Scheme::matching`], auch bei „Aus der Werkvorlage“ und „Eigenes Muster“, z. B.
    /// „I|1“ → „Nur Kapitel römisch“), sonst das Muster selbst, ohne Nummern „keine“, bei Schemata
    /// mit Wörtern deren Name.
    pub label: String,
}

/// Eine Zeile der Vorschau ([`Store::numbering_preview`]): Stelle, fester Schlüssel, Ebene, Nummer, Titel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NumberPreview {
    pub node: usize,
    pub key: i64,
    pub level: u8,
    pub number: String,
    pub title: String,
}

/// Ein Gliederungspunkt in Baumreihenfolge, wie er nach der Änderung sein soll.
#[derive(Debug, Clone)]
struct Item {
    /// Datenbank-id; `None` = neu anzulegen.
    id: Option<i64>,
    /// Stand vorher `(seq, Ebene, Titel)`; `None` = neu.
    orig: Option<(usize, u8, String)>,
    level: u8,
    title: String,
    numbered: bool,
    /// Text der Revision 1 eines neuen Knotens.
    body: Option<String>,
    /// Text schon geschrieben (Speichern im Editor): Überschriftszeile nicht neu setzen.
    text_done: bool,
}

struct Outline {
    template: String,
    numbering: Option<String>,
    /// Gliederungsschema (`work.numbering_scheme`, Migration 0008).
    scheme: String,
    items: Vec<Item>,
}

impl Outline {
    fn pattern(&self) -> Numbering {
        load::work_numbering(&self.scheme, self.numbering.as_deref(), &self.template)
    }
}

fn kind_of(level: u8) -> NodeKind {
    match level {
        0 => NodeKind::Part,
        1 => NodeKind::Chapter,
        _ => NodeKind::Section,
    }
}

type OutlineRow = (i64, i64, Option<i64>, i64, String, String, i64);

/// Gliederung des Werks in Baumreihenfolge (Tiefensuche, Geschwister nach position), mit Ebenen.
async fn load_outline(conn: &mut SqliteConnection, work: WorkId) -> Result<Outline, StoreError> {
    let head: Option<(String, Option<String>, String)> =
        sqlx::query_as("SELECT template, numbering, numbering_scheme FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
    let (template, numbering, scheme) = head.ok_or(StoreError::UnknownWork(work))?;
    let rows: Vec<OutlineRow> = sqlx::query_as(
        "SELECT id, seq, parent_seq, position, title, kind, numbered FROM node WHERE work_id = ? ORDER BY seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut children: HashMap<Option<i64>, Vec<(i64, i64, usize)>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        children.entry(row.2).or_default().push((row.3, row.1, index));
    }
    for list in children.values_mut() {
        list.sort_unstable();
    }
    let mut items = Vec::with_capacity(rows.len());
    let mut seen = vec![false; rows.len()];
    let mut stack: Vec<(usize, Option<u8>)> =
        children.get(&None).map(|c| c.iter().rev().map(|&(_, _, i)| (i, None)).collect()).unwrap_or_default();
    let push = |index: usize, parent_level: Option<u8>, items: &mut Vec<Item>| -> Result<u8, StoreError> {
        let (id, seq, _, _, title, kind, numbered) = &rows[index];
        let level = load::level_of(words::parse_node_kind(kind)?, parent_level);
        items.push(Item {
            id: Some(*id),
            orig: Some((to_usize(*seq, "seq")?, level, title.clone())),
            level,
            title: title.clone(),
            numbered: *numbered != 0,
            body: None,
            text_done: false,
        });
        Ok(level)
    };
    while let Some((index, parent_level)) = stack.pop() {
        if std::mem::replace(&mut seen[index], true) {
            continue; // Schleife in parent_seq
        }
        let level = push(index, parent_level, &mut items)?;
        if let Some(list) = children.get(&Some(rows[index].1)) {
            stack.extend(list.iter().rev().map(|&(_, _, i)| (i, Some(level))));
        }
    }
    // Knoten, deren Elternteil fehlt, gehen nicht verloren: hinten, nach seq.
    for (index, seen) in seen.iter_mut().enumerate() {
        if !*seen {
            *seen = true;
            push(index, None, &mut items)?;
        }
    }
    Ok(Outline { template, numbering, scheme, items })
}

/// Ist die Ebenenfolge gültig? Teile (0) stehen auf oberster Ebene, jede Ebene höchstens eine tiefer
/// als die vorige, keine tiefer als 3.
fn check_levels(items: &[Item]) -> Result<(), &'static str> {
    let mut previous = 0u8;
    let mut first = true;
    for item in items {
        if item.level > 3 {
            return Err("Tiefer als Ebene 3 (Unterabschnitt) geht es nicht.");
        }
        if item.level > previous + 1 {
            return Err(if first {
                "Der erste Gliederungspunkt muss ein Kapitel (oder Teil) sein."
            } else {
                "Ein Gliederungspunkt steht höchstens eine Ebene tiefer als der vorige (z. B. kein Kapitel unter einem Abschnitt)."
            });
        }
        if item.level > 0 {
            first = false;
        }
        previous = item.level;
    }
    Ok(())
}

/// Ende (ausschließlich) des Teilbaums ab Stelle `index`: alle folgenden Punkte mit größerer Ebene.
fn subtree_end(items: &[Item], index: usize) -> usize {
    let level = items[index].level;
    let mut end = index + 1;
    while end < items.len() && items[end].level > level {
        end += 1;
    }
    end
}

fn find(items: &[Item], work: WorkId, node: usize) -> Result<usize, StoreError> {
    items
        .iter()
        .position(|item| item.orig.as_ref().is_some_and(|o| o.0 == node))
        .ok_or(StoreError::UnknownNode { work, node })
}

/// Längster Titel eines Gliederungspunkts in Zeichen (Runde 7): Länger ist kein Titel mehr, sondern
/// ein Absatz, der versehentlich in die Überschriftszeile geraten ist.
pub const MAX_TITLE_CHARS: usize = 200;

/// Titelregeln eines Gliederungspunkts (Vertrag Runde 7, 4b), überall dieselben: Umbenennen und
/// Anlegen im Baum, Überschriftszeilen beim Speichern im Editor (auch jeder geteilte Titel), beim
/// Speichern und Ändern der aktuellen Fassung, in Vorschlägen und beim Umschalten.
///
/// Leerraum am Rand ([`char::is_whitespace`]) entfällt; leer → [`StoreError::Title`] „Ein
/// Gliederungspunkt braucht einen Titel.“; ein Zeilenende im Sinn von Typst
/// ([`baruch_typst::is_newline`]) → „Der Titel darf keinen Zeilenumbruch enthalten.“ (es wird nicht
/// mehr in ein Leerzeichen verwandelt); mehr als [`MAX_TITLE_CHARS`] Zeichen → „Der Titel ist zu
/// lang (höchstens 200 Zeichen, hier N).“ Sonst der getrimmte Titel.
pub fn check_title(title: &str) -> Result<String, StoreError> {
    let title = title.trim_matches(char::is_whitespace);
    if title.is_empty() {
        return Err(StoreError::Title("Ein Gliederungspunkt braucht einen Titel.".into()));
    }
    if title.chars().any(baruch_typst::is_newline) {
        return Err(StoreError::Title("Der Titel darf keinen Zeilenumbruch enthalten.".into()));
    }
    let count = title.chars().count();
    if count > MAX_TITLE_CHARS {
        return Err(StoreError::Title(format!(
            "Der Titel ist zu lang (höchstens {MAX_TITLE_CHARS} Zeichen, hier {count})."
        )));
    }
    Ok(title.to_owned())
}

/// Die **geänderten** Titel der Überschriften eines Texts nach [`check_title`] (Runde 8): die erste
/// Überschrift nur, wenn ihr Titel von `old_title` (dem jetzigen Titel des Gliederungspunkts)
/// abweicht, jede weitere (sie würde teilen, ist also neu) immer. So lässt sich ein Abschnitt mit
/// einem alten leeren oder überlangen Titel (Import prüft nicht) weiter speichern; ein neuer oder
/// geänderter ungültiger Titel wird wie bisher abgelehnt.
fn check_chunk_titles(chunks: &[(u8, String, String)], old_title: &str) -> Result<(), StoreError> {
    for (i, (_, title, _)) in chunks.iter().enumerate() {
        if i == 0 && title == old_title {
            continue;
        }
        check_title(title)?;
    }
    Ok(())
}

/// Erste Überschrift eines Texts, wenn davor nichts als Leerraum steht.
struct FirstHeading<'a> {
    /// Anfang der Überschriftszeile im Text.
    start: usize,
    /// Die Zeile selbst (mit Zeilenende, falls vorhanden).
    line: &'a str,
    level: u8,
    title: String,
}

fn first_heading(body: &str) -> Option<FirstHeading<'_>> {
    let doc = baruch_typst::parse(body).ok()?;
    if !doc.template.trim().is_empty() {
        return None;
    }
    let start = doc.template.len();
    let heading = doc.headings.into_iter().next()?;
    let line = body[start..].split_inclusive('\n').next()?;
    Some(FirstHeading { start, line, level: heading.level, title: heading.title })
}

/// Titel der ersten Überschrift eines Texts (davor nur Leerraum), wie ihn Umschalten übernähme (Austausch,
/// Runde 26: „trägt eine Fassung den Titel aus der Datei schon?“).
pub(crate) fn heading_title(body: &str) -> Option<String> {
    first_heading(body).map(|found| found.title)
}

/// Text mit neuer Überschriftszeile (nur diese Zeile ändert sich). Bleibt der Titel, bleibt die Zeile
/// bis auf die Ebene wörtlich (Maskierungen, Label, Kommentar; [`baruch_typst::relevel_line`], auch
/// für die Markierung „ohne Nummer“, Runde 14). Ohne Überschrift am Anfang kommt eine neue Zeile davor.
fn rewrite_heading(body: &str, level: u8, title: &str, title_changed: bool) -> String {
    match first_heading(body) {
        Some(found) => {
            let replaced = if title_changed {
                baruch_typst::heading_line(level, title, Some(found.line))
            } else {
                baruch_typst::relevel_line(found.line, level).unwrap_or_else(|| found.line.to_owned())
            };
            format!("{}{replaced}{}", &body[..found.start], &body[found.start + found.line.len()..])
        }
        None => format!("{}{body}", baruch_typst::heading_line(level, title, None)),
    }
}

/// Text eines Teils mit neuem Titel (Runde 14): Steht am Anfang die Teil-Markierung, ändert sich nur
/// ihre Zeile (Kommentar bleibt); ein leerer Text (nur Leerraum) bekommt die Markierung. Sonst – der
/// Text trägt keine Markierung (Altbestand wie „= Teil I“ oder freier Text) – `None`: Der Text bleibt.
fn rewrite_part(body: &str, title: &str) -> Option<String> {
    if body.trim().is_empty() {
        let rest = if body.is_empty() { "\n" } else { body };
        return Some(format!("{}{rest}", baruch_typst::heading_line(0, title, None)));
    }
    let found = first_heading(body).filter(|found| found.level == 0)?;
    let line = baruch_typst::heading_line(0, title, Some(found.line));
    Some(format!("{}{line}{}", &body[..found.start], &body[found.start + found.line.len()..]))
}

/// Ganzen Knoten löschen: erst die aktuelle Fassung ablösen (current → superseded, erlaubt laut 0004),
/// dann Revisionen, Verweise seiner Beiträge, Beiträge und den Knoten.
async fn remove_node(conn: &mut SqliteConnection, node_id: i64) -> Result<(), StoreError> {
    sqlx::query("UPDATE revision SET status = 'superseded' WHERE node_id = ? AND status = 'current'")
        .bind(node_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM post_ref WHERE post_id IN (SELECT id FROM post WHERE node_id = ?)")
        .bind(node_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM post WHERE node_id = ?").bind(node_id).execute(&mut *conn).await?;
    sqlx::query("DELETE FROM revision WHERE node_id = ?").bind(node_id).execute(&mut *conn).await?;
    sqlx::query("DELETE FROM node WHERE id = ?").bind(node_id).execute(&mut *conn).await?;
    Ok(())
}

/// Text der aktuellen Fassung (leer ohne Revisionen).
async fn current_body(conn: &mut SqliteConnection, node_id: i64) -> Result<String, StoreError> {
    Ok(write::current(conn, node_id).await?.map(|(_, body, _)| body).unwrap_or_default())
}

/// Elternteil (Stelle) je Punkt: der nächste Vorgänger mit kleinerer Ebene.
fn parents(items: &[Item]) -> Vec<Option<usize>> {
    let mut stack: Vec<usize> = Vec::new();
    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        while stack.last().is_some_and(|&top| items[top].level >= item.level) {
            stack.pop();
        }
        out.push(stack.last().copied());
        stack.push(index);
    }
    out
}

/// `node.number` aller Knoten nach dem Muster schreiben (Knoten müssen schon ihre id haben).
async fn write_numbers(
    conn: &mut SqliteConnection,
    pattern: &Numbering,
    items: &[Item],
    parents: &[Option<usize>],
) -> Result<(), StoreError> {
    let counted: Vec<Numbered> = items
        .iter()
        .zip(parents)
        .map(|(item, &parent)| Numbered { kind: kind_of(item.level), parent, numbered: item.numbered })
        .collect();
    for (item, number) in items.iter().zip(load::numbers(pattern, &counted)) {
        let id = item.id.ok_or_else(|| broken("Gliederungspunkt ohne id beim Nummerieren".into()))?;
        sqlx::query("UPDATE node SET number = ? WHERE id = ? AND number <> ?")
            .bind(&number)
            .bind(id)
            .bind(&number)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Beiträge des Werks (für `post_ref`).
const WORK_POSTS: &str = "post_id IN (SELECT p.id FROM post p JOIN node n ON n.id = p.node_id WHERE n.work_id = ?)";

struct Applied {
    revisions: Vec<(usize, u32)>,
    renumbered: Vec<(usize, usize)>,
    /// Umbenannte Teile, deren Text keine Teil-Markierung trägt (Altbestand): Text unverändert.
    kept_parts: Vec<String>,
}

/// Neue Gliederung schreiben: seq/parent_seq/position/kind/title in Baumreihenfolge, neue Knoten mit
/// Revision 1, Nummern, Verweise, neue Fassungen nur mit geänderter Überschriftszeile. `removed` sind
/// die alten seq schon entfernter Knoten (ihre Verweise entfallen).
async fn apply(
    conn: &mut SqliteConnection,
    work: WorkId,
    pattern: &Numbering,
    items: &mut [Item],
    removed: &[usize],
    author: &Author,
) -> Result<Applied, StoreError> {
    apply_with(conn, work, pattern, items, removed, author, false).await
}

/// Wie [`apply`]; mit `bare` bekommen neue Knoten **keine** Revision 1 (Austausch, Runde 26: das Einlesen legt
/// danach genau die Fassungen aus der Datei an, [`insert_bare`]).
async fn apply_with(
    conn: &mut SqliteConnection,
    work: WorkId,
    pattern: &Numbering,
    items: &mut [Item],
    removed: &[usize],
    author: &Author,
    bare: bool,
) -> Result<Applied, StoreError> {
    check_levels(items).map_err(StoreError::Refused)?;
    let parents = parents(items);
    let mut positions = Vec::with_capacity(items.len());
    let mut next: HashMap<Option<usize>, i64> = HashMap::new();
    for parent in &parents {
        let position = next.entry(*parent).or_insert(0);
        positions.push(*position);
        *position += 1;
    }

    // Zweistufig wegen UNIQUE(work_id, seq): erst alle negativ, dann die neuen Werte.
    sqlx::query("UPDATE node SET seq = -seq - 1 WHERE work_id = ?").bind(work).execute(&mut *conn).await?;
    for (index, item) in items.iter_mut().enumerate() {
        let parent = parents[index].map(|p| p as i64);
        let kind = words::node_kind(kind_of(item.level));
        match item.id {
            Some(id) => {
                sqlx::query("UPDATE node SET seq = ?, parent_seq = ?, position = ?, kind = ? WHERE id = ?")
                    .bind(index as i64)
                    .bind(parent)
                    .bind(positions[index])
                    .bind(kind)
                    .bind(id)
                    .execute(&mut *conn)
                    .await?;
                if item.orig.as_ref().is_some_and(|o| o.2 != item.title) {
                    sqlx::query("UPDATE node SET title = ? WHERE id = ?")
                        .bind(&item.title)
                        .bind(id)
                        .execute(&mut *conn)
                        .await?;
                }
            }
            None => {
                // Nummer vorläufig nicht leer, sonst hielte der Trigger aus 0006 den Knoten für unnummeriert.
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind, numbered) \
                     VALUES (?, ?, ?, ?, '0', ?, ?, ?) RETURNING id",
                )
                .bind(work)
                .bind(index as i64)
                .bind(parent)
                .bind(positions[index])
                .bind(&item.title)
                .bind(kind)
                .bind(i64::from(item.numbered))
                .fetch_one(&mut *conn)
                .await?;
                if bare {
                    item.id = Some(id);
                    continue;
                }
                // Revision 1 vom Anlegenden: der Mensch oder (Wunsch 2026-11-06) eine Persona bzw. ein
                // MCP-Client, der über `outline_add` anlegt.
                let (author_kind, author_name, model) = crate::author_columns(author);
                sqlx::query(
                    "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
                     VALUES (?, 1, ?, ?, ?, ?, 'current', '')",
                )
                .bind(id)
                .bind(item.body.as_deref().unwrap_or(""))
                .bind(author_kind)
                .bind(author_name)
                .bind(model)
                .execute(&mut *conn)
                .await?;
                item.id = Some(id);
            }
        }
    }
    write_numbers(conn, pattern, items, &parents).await?;

    // Verweise in Beiträgen: gelöschte entfallen, die übrigen bekommen die neue seq (zweistufig).
    for &seq in removed {
        sqlx::query(&format!("DELETE FROM post_ref WHERE node_seq = ? AND {WORK_POSTS}"))
            .bind(seq as i64)
            .bind(work)
            .execute(&mut *conn)
            .await?;
    }
    let renumbered: Vec<(usize, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item.orig.as_ref().map(|o| (o.0, index)))
        .filter(|(old, new)| old != new)
        .collect();
    for &(old, new) in &renumbered {
        sqlx::query(&format!("UPDATE post_ref SET node_seq = ? WHERE node_seq = ? AND {WORK_POSTS}"))
            .bind(-(new as i64) - 1)
            .bind(old as i64)
            .bind(work)
            .execute(&mut *conn)
            .await?;
    }
    sqlx::query(&format!("UPDATE post_ref SET node_seq = -node_seq - 1 WHERE node_seq < 0 AND {WORK_POSTS}"))
        .bind(work)
        .execute(&mut *conn)
        .await?;

    // Neue Fassungen: nur die Überschriftszeile ändert sich.
    let mut revisions = Vec::new();
    let mut kept_parts = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Some((_, old_level, old_title)) = &item.orig else {
            if !bare {
                revisions.push((index, 1));
            }
            continue;
        };
        let title_changed = *old_title != item.title;
        if item.text_done || (*old_level == item.level && !title_changed) {
            continue;
        }
        let id = item.id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
        let body = write::current(conn, id).await?.map(|(_, body, _)| body);
        let rewritten = if item.level == 0 {
            // Teil (Runde 14): Markierungszeile neu, leerer Text bekommt sie; ohne Markierung bleibt
            // der Text (Altbestand, Nutzerfrage 8), nur der Titel ändert sich.
            match rewrite_part(body.as_deref().unwrap_or(""), &item.title) {
                Some(rewritten) => rewritten,
                None => {
                    kept_parts.push(item.title.clone());
                    continue;
                }
            }
        } else {
            let Some(body) = &body else { continue };
            rewrite_heading(body, item.level, &item.title, title_changed)
        };
        if body.as_deref() == Some(rewritten.as_str()) {
            continue;
        }
        // Umschreiben bestehender Fassungen ist Sache des Menschen (auch bei einem bestätigten Antrag);
        // Anlegen durch einen Assistenten ändert keine bestehende Überschrift.
        let Author::Human { name: human } = author else {
            return Err(broken("Ein Assistent schreibt keine bestehende Fassung um".into()));
        };
        if let Some(change) = write::save_human(conn, work, index, &rewritten, human, None).await? {
            revisions.push((index, change.rev));
        }
    }
    Ok(Applied { revisions, renumbered, kept_parts })
}

pub(crate) fn quoted(title: &str) -> String {
    format!("„{title}“")
}

fn change(
    work: WorkId,
    action: OutlineAction,
    node: usize,
    title: &str,
    detail: String,
    applied: Applied,
) -> OutlineChange {
    OutlineChange {
        work,
        action,
        node,
        title: title.to_owned(),
        detail,
        revisions: applied.revisions,
        renumbered: applied.renumbered,
        removed: Vec::new(),
    }
}

/// Meldung, wenn Verschieben die Ebene ändern würde.
pub(crate) const MOVE_KEEPS_LEVEL: &str =
    "Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).";

/// Behält Verschieben mit `place` relativ zu einem Ziel der Ebene `target_level` die Ebene `level`?
/// `before`/`after` nur bei gleicher Ebene, `into` nur unter einen Punkt genau eine Ebene höher.
fn move_keeps_level(level: u8, target_level: u8, place: Place) -> bool {
    match place {
        Place::Before | Place::After => target_level == level,
        Place::Into => target_level + 1 == level,
    }
}

pub(crate) async fn move_node(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    target: usize,
    place: Place,
    human: &str,
) -> Result<OutlineChange, StoreError> {
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let items = &mut outline.items;
    let index = find(items, work, node)?;
    let target_index = find(items, work, target)?;
    let end = subtree_end(items, index);
    if (index..end).contains(&target_index) {
        return Err(StoreError::Refused(
            "Ein Gliederungspunkt lässt sich nicht in seinen eigenen Teilbaum verschieben.",
        ));
    }
    let (level, target_level) = (items[index].level, items[target_index].level);
    // Die Ebene bleibt (Vertrag Runde 5): vor/hinter nur einen Punkt derselben Ebene, unter nur einen
    // Punkt genau eine Ebene höher. Umstufen geht über ← / → (`outline.set_level`).
    if !move_keeps_level(level, target_level, place) {
        return Err(StoreError::Refused(MOVE_KEEPS_LEVEL));
    }
    let before: Vec<(Option<i64>, u8)> = items.iter().map(|i| (i.id, i.level)).collect();
    let title = items[index].title.clone();
    let target_title = items[target_index].title.clone();
    let block: Vec<Item> = items.drain(index..end).collect();
    let target_now = if target_index > index { target_index - block.len() } else { target_index };
    let at = match place {
        Place::Before => target_now,
        Place::After | Place::Into => subtree_end(items, target_now),
    };
    items.splice(at..at, block);
    check_levels(items).map_err(StoreError::Refused)?;
    let after: Vec<(Option<i64>, u8)> = items.iter().map(|i| (i.id, i.level)).collect();
    if before == after {
        return Err(StoreError::Refused("Der Gliederungspunkt steht schon an dieser Stelle."));
    }
    let applied = apply(conn, work, &pattern, items, &[], &Author::Human { name: human.to_owned() }).await?;
    let word = match place {
        Place::Before => "vor",
        Place::After => "nach",
        Place::Into => "unter",
    };
    let detail = format!("{} {word} {} verschoben", quoted(&title), quoted(&target_title));
    Ok(change(work, OutlineAction::Moved, at, &title, detail, applied))
}

pub(crate) async fn set_level(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    level: u8,
    human: &str,
) -> Result<OutlineChange, StoreError> {
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let items = &mut outline.items;
    let index = find(items, work, node)?;
    let old = items[index].level;
    if old == 0 || level == 0 {
        return Err(StoreError::Refused("Teile lassen sich nicht umstufen, und nichts wird zum Teil."));
    }
    if level > 3 {
        return Err(StoreError::Refused("Tiefer als Ebene 3 (Unterabschnitt) geht es nicht."));
    }
    if old == level {
        return Err(StoreError::Refused("Der Gliederungspunkt hat schon diese Ebene."));
    }
    let end = subtree_end(items, index);
    for item in &mut items[index..end] {
        let shifted = i16::from(item.level) + i16::from(level) - i16::from(old);
        if !(1..=3).contains(&shifted) {
            return Err(StoreError::Refused(
                "Mit den Unterpunkten ginge das tiefer als Ebene 3 (Unterabschnitt); erst Unterpunkte höher stufen.",
            ));
        }
        item.level = shifted as u8;
    }
    check_levels(items).map_err(StoreError::Refused)?;
    let title = items[index].title.clone();
    let applied = apply(conn, work, &pattern, items, &[], &Author::Human { name: human.to_owned() }).await?;
    let detail = format!("{} Ebene {old} → {level}", quoted(&title));
    Ok(change(work, OutlineAction::Level, index, &title, detail, applied))
}

pub(crate) async fn rename(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    title: &str,
    human: &str,
) -> Result<OutlineChange, StoreError> {
    let title = check_title(title)?;
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let items = &mut outline.items;
    let index = find(items, work, node)?;
    let old = items[index].title.clone();
    if old == title {
        return Err(StoreError::Refused("Der Gliederungspunkt heißt schon so."));
    }
    items[index].title = title.clone();
    let applied = apply(conn, work, &pattern, items, &[], &Author::Human { name: human.to_owned() }).await?;
    let mut detail = format!("{} → {} umbenannt", quoted(&old), quoted(&title));
    if !applied.kept_parts.is_empty() {
        detail.push_str(" (Text ohne Teil-Markierung unverändert, nur der Titel)");
    }
    Ok(change(work, OutlineAction::Renamed, index, &title, detail, applied))
}

/// Ein neuer Punkt auf der Ebene des Orts (Mensch; RPC `outline.add` ohne `level`, Einfälle).
pub(crate) async fn add(
    conn: &mut SqliteConnection,
    work: WorkId,
    target: usize,
    place: Place,
    title: &str,
    human: &str,
) -> Result<OutlineChange, StoreError> {
    let point = NewPoint { title: title.to_owned(), level: None };
    let author = Author::Human { name: human.to_owned() };
    Ok(add_many(conn, work, Some(target), place, std::slice::from_ref(&point), &author).await?.change)
}

/// Ein neuer Gliederungspunkt für [`Store::outline_add_many`] (MCP `outline_add`, Wunsch 2026-11-06).
///
/// `level` ist die **absolute** Ebene (0 Teil, 1 Kapitel, 2 Abschnitt, 3 Unterabschnitt); fehlt sie,
/// gilt die Ebene des Orts (Ebene des Bezugspunkts bei `before`/`after`, eine tiefer bei `into`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewPoint {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
}

/// Ein angelegter Punkt: Stelle danach, fester Schlüssel, Ebene, Titel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AddedPoint {
    pub node: usize,
    pub key: i64,
    pub level: u8,
    pub title: String,
}

/// Ergebnis von [`Store::outline_add_many`]: die Gliederungsänderung (ein Commit), die neuen Punkte
/// in Baumreihenfolge und (Runde 14) die festen Schlüssel der vorhandenen Kapitel, die jetzt unter
/// einem neuen Teil stehen (Reihenfolge, Texte und Kapitelnummern unverändert).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineAdded {
    pub change: OutlineChange,
    pub points: Vec<AddedPoint>,
    pub adopted: Vec<i64>,
}

/// Ergebnis von [`Store::idea_to_outline`] (Runde 14): die Gliederungsänderung, der feste Schlüssel
/// des neuen Punkts und der Einfall danach (verarbeitet als Gliederungspunkt, `done_ref` „key <key>“).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdeaOutline {
    pub change: OutlineChange,
    pub key: i64,
    pub idea: Idea,
}

/// Höchstzahl neuer Punkte in einem Aufruf von [`Store::outline_add_many`].
pub const MAX_NEW_POINTS: usize = 100;

/// Titel eines Punkts aus einer Liste mit seiner Nummer („Punkt 3: …“), bei einem einzelnen ohne.
fn point_title(title: &str, index: usize, many: bool) -> Result<String, StoreError> {
    check_title(title).map_err(|e| match e {
        StoreError::Title(message) if many => StoreError::Title(format!("Punkt {}: {message}", index + 1)),
        other => other,
    })
}

/// Mehrere Punkte als einen Block anlegen (atomar, Wunsch 2026-11-06). Der Block steht vor `target`
/// bzw. hinter dessen Unterpunkten (`after`, `into`). Der erste Punkt steht auf der Ebene des Orts,
/// kein weiterer höher als er: So bleibt jeder vorhandene Punkt, wo er war (kein vorhandener Punkt
/// wird Unterpunkt eines neuen – außer unter einem neuen Teil, siehe unten). `target = None` nur in einem Werk ohne Gliederungspunkt (der erste
/// Punkt ist dann ein Kapitel oder ein Teil).
///
/// Ausnahme Teile (Runde 14, auch mit Werkvorlage): Ebene 0 geht vor oder hinter einem Teil oder
/// Kapitel (nicht `into`, nicht neben einem Abschnitt), auch als erster Punkt neben einem Kapitel und
/// mitten im Block. Die vorhandenen Kapitel dahinter bis zum nächsten Teil stehen dann unter dem
/// neuen Teil (`adopted`); ihre Ebene, ihr Text und ihre Nummer bleiben, Elternteil (`parent_seq`)
/// und Stelle (`position`) ändern sich. Assistenten (MCP `outline_add`) legen so einen Teil nur an, wenn
/// er keine vorhandenen Kapitel übernimmt; sonst wird es ein Antrag, dem der Mensch zustimmt
/// ([`Store::outline_add_by_assistant`], Nutzerfrage 12, entschieden 2026-11-07).
pub(crate) async fn add_many(
    conn: &mut SqliteConnection,
    work: WorkId,
    target: Option<usize>,
    place: Place,
    points: &[NewPoint],
    author: &Author,
) -> Result<OutlineAdded, StoreError> {
    if points.is_empty() {
        return Err(StoreError::Refused("Keine Gliederungspunkte angegeben (title oder items)."));
    }
    if points.len() > MAX_NEW_POINTS {
        return Err(StoreError::Refused("Höchstens 100 Gliederungspunkte in einem Aufruf."));
    }
    let many = points.len() > 1;
    let titles: Vec<String> =
        points.iter().enumerate().map(|(i, p)| point_title(&p.title, i, many)).collect::<Result<_, _>>()?;
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let items = &mut outline.items;
    let wants_part = points.iter().any(|p| p.level == Some(0));
    let (at, base, target_title) = match target {
        Some(target) => {
            let target_index = find(items, work, target)?;
            let target_level = items[target_index].level;
            if wants_part && place == Place::Into {
                return Err(StoreError::Refused(PART_NOT_INTO));
            }
            if wants_part && target_level > 1 {
                return Err(StoreError::Refused(PART_PLACE));
            }
            let base = match place {
                Place::Before | Place::After => target_level,
                Place::Into => target_level + 1,
            };
            let at = match place {
                Place::Before => target_index,
                Place::After | Place::Into => subtree_end(items, target_index),
            };
            (at, base, Some(items[target_index].title.clone()))
        }
        None if items.is_empty() => (0, points[0].level.unwrap_or(1), None),
        None => {
            return Err(StoreError::Refused(
                "Bezugspunkt (key) fehlt: Ohne ihn lässt sich nur der erste Punkt eines leeren Werks anlegen.",
            ));
        }
    };
    let levels: Vec<u8> = points.iter().map(|p| p.level.unwrap_or(base)).collect();
    if levels.iter().any(|&l| l > 3) || base > 3 {
        return Err(StoreError::Refused("Tiefer als Ebene 3 (Unterabschnitt) geht es nicht."));
    }
    // Ein Teil darf als erster Punkt auch neben einem Kapitel stehen (dann ist die Ebene des Orts 1).
    let first = levels[0];
    if first != base && !(first == 0 && base == 1) {
        return Err(StoreError::Refused(
            "Der erste neue Punkt steht auf der Ebene des Orts (vor/nach: Ebene des Bezugspunkts, unter: eine tiefer).",
        ));
    }
    if levels.iter().any(|&l| l < first && l > 0) {
        return Err(StoreError::Refused(
            "Kein neuer Punkt steht höher als der erste; sonst würden vorhandene Punkte zu seinen Unterpunkten.",
        ));
    }
    let block: Vec<Item> = titles
        .iter()
        .zip(&levels)
        .map(|(title, &level)| Item {
            id: None,
            orig: None,
            level,
            title: title.clone(),
            numbered: true,
            // Überschriftszeile (beim Teil die Teil-Markierung, Runde 14) und Leerzeile.
            body: Some(format!("{}\n", baruch_typst::heading_line(level, title, None))),
            text_done: true,
        })
        .collect();
    let count = block.len();
    items.splice(at..at, block);
    check_levels(items).map_err(StoreError::Refused)?;
    let applied = apply(conn, work, &pattern, items, &[], author).await?;
    let points: Vec<AddedPoint> = (at..at + count)
        .map(|index| {
            let item = &items[index];
            let key = item.id.ok_or_else(|| broken("neuer Gliederungspunkt ohne id".into()))?;
            Ok(AddedPoint { node: index, key, level: item.level, title: item.title.clone() })
        })
        .collect::<Result<_, StoreError>>()?;
    // Vorhandene Kapitel, deren Elternteil jetzt ein neuer Teil ist.
    let adopted: Vec<i64> = parents(items)
        .iter()
        .enumerate()
        .filter(|&(index, parent)| {
            items[index].orig.is_some() && parent.is_some_and(|p| (at..at + count).contains(&p) && items[p].level == 0)
        })
        .filter_map(|(index, _)| items[index].id)
        .collect();
    let word = match place {
        Place::Before => "vor",
        Place::After => "nach",
        Place::Into => "unter",
    };
    let what = if many {
        let mut names: Vec<String> = titles.iter().take(3).map(|t| quoted(t)).collect();
        if titles.len() > 3 {
            names.push("…".to_owned());
        }
        format!("{} Punkte ({})", titles.len(), names.join(", "))
    } else {
        quoted(&titles[0])
    };
    let detail = match target_title {
        Some(target_title) => format!("{what} {word} {} angelegt", quoted(&target_title)),
        None => format!("{what} als Anfang der Gliederung angelegt"),
    };
    let change = change(work, OutlineAction::Added, at, &titles[0], detail, applied);
    Ok(OutlineAdded { change, points, adopted })
}

/// Ergebnis von [`insert_bare`] (Austausch, Runde 26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BareInsert {
    /// Angelegt, ohne Fassung: fester Schlüssel des neuen Punkts.
    Inserted(i64),
    /// Ein neuer Teil übernähme vorhandene Kapitel (`adopted`); nichts geschrieben. `anchor` und `place` sind
    /// Bezugspunkt und Ort für einen Antrag [`crate::OutlineOp::Add`] an derselben Stelle.
    WouldAdopt { adopted: Vec<i64>, anchor: i64, place: Place },
    /// An dieser Stelle bekäme der Punkt ein anderes Elternteil, übernähme vorhandene Punkte oder ergäbe keine
    /// gültige Gliederung; nichts geschrieben.
    Misplaced(String),
}

/// Einen Punkt **ohne** Fassung einfügen (Austausch, Runde 26, in der Transaktion des Aufrufers): hinter dem
/// Teilbaum von `after`, sonst als erstes Kind von `parent`, sonst als erster Punkt der Gliederung. Der neue
/// Punkt muss dort genau `parent` als Elternteil haben, und kein vorhandener Punkt darf sein Elternteil
/// wechseln; sonst wird nichts geschrieben ([`BareInsert::WouldAdopt`] bei einem neuen Teil vor Kapiteln,
/// sonst [`BareInsert::Misplaced`]). Vorhandene Punkte behalten Titel, Ebene und Text; seq, Stelle und Nummer
/// rechnet die Gliederung wie beim Anlegen neu. Den Titel prüft das Einlesen nicht (wie der Import).
pub(crate) async fn insert_bare(
    conn: &mut SqliteConnection,
    work: WorkId,
    after: Option<i64>,
    parent: Option<i64>,
    level: u8,
    title: &str,
    numbered: bool,
) -> Result<BareInsert, StoreError> {
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let items = &mut outline.items;
    let index_of = |items: &[Item], key: i64| items.iter().position(|item| item.id == Some(key));
    let (at, anchor) = match (after, parent) {
        (Some(after), _) => {
            let index = index_of(items, after).ok_or(StoreError::GoneNode { work, key: after })?;
            (subtree_end(items, index), Some((after, Place::After)))
        }
        (None, Some(parent)) => {
            let index = index_of(items, parent).ok_or(StoreError::GoneNode { work, key: parent })?;
            (index + 1, None)
        }
        (None, None) => (0, items.first().and_then(|item| item.id).map(|key| (key, Place::Before))),
    };
    // Elternteil je Punkt als Schlüssel; der neue Punkt hat noch keinen und heißt hier -1 (Schlüssel sind > 0).
    let parent_ids = |items: &[Item]| -> Vec<Option<i64>> {
        parents(items).iter().map(|p| p.map(|p| items[p].id.unwrap_or(-1))).collect()
    };
    let before: Vec<(Option<i64>, Option<i64>)> = items.iter().map(|i| i.id).zip(parent_ids(items)).collect();
    items.insert(
        at,
        Item { id: None, orig: None, level, title: title.to_owned(), numbered, body: None, text_done: true },
    );
    if let Err(message) = check_levels(items) {
        return Ok(BareInsert::Misplaced(message.to_owned()));
    }
    let after_parents = parent_ids(items);
    if after_parents[at] != parent {
        return Ok(BareInsert::Misplaced(
            "An dieser Stelle bekäme der Punkt hier ein anderes Elternteil als in der Datei.".to_owned(),
        ));
    }
    let adopted: Vec<i64> = before
        .iter()
        .filter_map(|&(id, was)| {
            let id = id?;
            let now = items.iter().position(|i| i.id == Some(id)).map(|i| after_parents[i])?;
            (now != was).then_some(id)
        })
        .collect();
    if !adopted.is_empty() {
        return Ok(match anchor {
            Some((anchor, place)) if level == 0 => BareInsert::WouldAdopt { adopted, anchor, place },
            _ => BareInsert::Misplaced("An dieser Stelle übernähme der Punkt vorhandene Punkte.".to_owned()),
        });
    }
    let author = Author::Human { name: String::new() };
    apply_with(conn, work, &pattern, items, &[], &author, true).await?;
    items[at].id.map(BareInsert::Inserted).ok_or_else(|| broken("neuer Gliederungspunkt ohne id".into()))
}

/// Meldung: Ein Teil steht immer auf oberster Ebene.
const PART_NOT_INTO: &str = "Ein Teil steht immer auf oberster Ebene; „darunter“ (into) geht mit Ebene 0 nicht.";
/// Meldung: Teile nur neben Teilen und Kapiteln.
const PART_PLACE: &str = "Ein Teil entsteht nur vor oder hinter einem Teil oder Kapitel, nicht neben einem Abschnitt.";

/// Stand eines Punkts über seinen festen Schlüssel: `(seq, Ebene, Titel)`; `None`,
/// wenn es ihn im Werk nicht (mehr) gibt.
pub(crate) async fn point_state(
    conn: &mut SqliteConnection,
    work: WorkId,
    key: i64,
) -> Result<Option<(usize, u8, String)>, StoreError> {
    let outline = load_outline(conn, work).await?;
    Ok(outline
        .items
        .iter()
        .find(|item| item.id == Some(key))
        .and_then(|item| item.orig.as_ref().map(|o| (o.0, item.level, item.title.clone()))))
}

/// Ein Punkt der Gliederung für die Vorschau eines Antrags (Modul `requests`, Runde 14).
#[derive(Debug, Clone)]
pub(crate) struct PointState {
    pub key: i64,
    pub level: u8,
    pub number: String,
    pub title: String,
    pub numbered: bool,
}

/// Alle Punkte des Werks in Baumreihenfolge mit Ebene, Nummer (nach der wirksamen Nummerierung) und
/// Titel (in der Transaktion des Aufrufers).
pub(crate) async fn point_states(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<PointState>, StoreError> {
    let outline = load_outline(conn, work).await?;
    let numbers = outline_numbers(&outline, &outline.pattern());
    outline
        .items
        .iter()
        .zip(numbers)
        .map(|(item, number)| {
            let key = item.id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
            Ok(PointState { key, level: item.level, number, title: item.title.clone(), numbered: item.numbered })
        })
        .collect()
}

/// Einen Punkt mit bzw. ohne Nummer zählen (Wunsch 2026-11-06, z. B. Einleitung oder Vorwort ohne
/// Nummer): setzt `node.numbered` und schreibt alle Nummern neu; der Text bleibt. Schon so → `Refused`.
/// Ein Teil ohne Nummer → `Refused`: Die Teil-Markierung hat kein Nummer-Merkmal, Export → Import
/// machte ihn wieder nummeriert und verschöbe die Nummern der folgenden Teile. Einen Teil, der (aus
/// einem älteren Stand) schon ohne Nummer ist, darf der Mensch wieder mit Nummer zählen.
pub(crate) async fn set_numbered(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    numbered: bool,
    human: &str,
) -> Result<OutlineChange, StoreError> {
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let items = &mut outline.items;
    let index = find(items, work, node)?;
    if !numbered && items[index].level == 0 {
        return Err(StoreError::Refused(
            "Teile werden immer gezählt; „ohne Nummer“ gibt es nur für Kapitel und Abschnitte.",
        ));
    }
    if items[index].numbered == numbered {
        return Err(StoreError::Refused(if numbered {
            "Der Gliederungspunkt hat schon eine Nummer."
        } else {
            "Der Gliederungspunkt ist schon ohne Nummer."
        }));
    }
    items[index].numbered = numbered;
    let id = items[index].id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
    sqlx::query("UPDATE node SET numbered = ? WHERE id = ?")
        .bind(i64::from(numbered))
        .bind(id)
        .execute(&mut *conn)
        .await?;
    let title = items[index].title.clone();
    let applied = apply(conn, work, &pattern, items, &[], &Author::Human { name: human.to_owned() }).await?;
    let detail = format!("{} {}", quoted(&title), if numbered { "mit Nummer" } else { "ohne Nummer" });
    Ok(change(work, OutlineAction::Numbered, index, &title, detail, applied))
}

pub(crate) async fn delete(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    human: &str,
) -> Result<OutlineChange, StoreError> {
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let items = &mut outline.items;
    let index = find(items, work, node)?;
    if subtree_end(items, index) > index + 1 {
        return Err(StoreError::Refused("Der Gliederungspunkt hat Unterpunkte; erst verschieben oder löschen."));
    }
    if items[index].level > 0 && items.iter().filter(|i| i.level > 0).count() == 1 {
        return Err(StoreError::Refused(
            "Der letzte Gliederungspunkt lässt sich nicht löschen; ein Werk braucht mindestens ein Kapitel.",
        ));
    }
    let removed_item = items.remove(index);
    check_levels(items).map_err(StoreError::Refused)?;
    let id = removed_item.id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
    let text = current_body(conn, id).await?;
    remove_node(conn, id).await?;
    let applied = apply(conn, work, &pattern, items, &[node], &Author::Human { name: human.to_owned() }).await?;
    let title = removed_item.title;
    let detail = format!("{} gelöscht", quoted(&title));
    let mut done = change(work, OutlineAction::Deleted, index.saturating_sub(1), &title, detail, applied);
    done.removed.push((title, text));
    Ok(done)
}

/// Text des Editors zerlegt: passt der Anfang (nur Leerraum vor der ersten Überschrift), und die
/// Stücke je Überschrift `(Ebene, Titel, Text)`; Leerraum davor gehört zum ersten Stück.
struct Analysis {
    lead_ok: bool,
    chunks: Vec<(u8, String, String)>,
    /// Je Stück: als `=`-Überschrift (bzw. Teil) mit Nummer, `false` bei der Markierung „ohne Nummer“.
    numbered: Vec<bool>,
}

fn analyze(body: &str) -> Analysis {
    match baruch_typst::parse(body) {
        Err(_) => Analysis { lead_ok: true, chunks: Vec::new(), numbered: Vec::new() },
        Ok(doc) => {
            let lead_ok = doc.template.trim().is_empty();
            let numbered = doc.headings.iter().map(|h| h.numbered).collect();
            let mut chunks: Vec<(u8, String, String)> =
                doc.headings.into_iter().map(|h| (h.level, h.title, h.body)).collect();
            if lead_ok && let Some(first) = chunks.first_mut() {
                first.2 = format!("{}{}", doc.template, first.2);
            }
            Analysis { lead_ok, chunks, numbered }
        }
    }
}

const LEAD_TEXT: &str = "Vor der ersten Überschrift steht Text; die Überschrift gehört an den Anfang des Abschnitts.";
const MERGE_FIRST: &str = "Der erste Gliederungspunkt lässt sich nicht zusammenführen.";
const MERGE_PART: &str = "Davor steht ein Teil; zusammenführen geht nur mit einem Kapitel oder Abschnitt.";
const TO_PART: &str = "Die erste Überschrift ist eine Teil-Markierung; ein Kapitel oder Abschnitt wird nicht zum Teil (neuen Teil in der Gliederung anlegen).";
const PART_HINT: &str = "Ein Teiltext mit Markierung beginnt mit genau dieser einen Markierung (davor nur Leerraum) und enthält keine weitere Überschrift.";

/// Trägt der Text eine Teil-Markierung als Überschrift (Runde 14)?
fn has_part_marker(body: &str) -> bool {
    analyze(&body.replace("\r\n", "\n")).chunks.iter().any(|(level, _, _)| *level == 0)
}

/// Was am Text eines Teils nicht passt (Runde 14): Trägt er eine Teil-Markierung (oder verlangt
/// `required` sie, weil die aktuelle Fassung eine hat), muss sie genau einmal am Anfang stehen (davor
/// nur Leerraum), ohne weitere Überschrift. Ohne Markierung und ohne `required` → `None`: Teiltexte
/// ohne Markierung (Altbestand) werden wie bisher ohne Abgleich gespeichert.
fn part_problem(body: &str, required: bool) -> Option<String> {
    let analysis = analyze(&body.replace("\r\n", "\n"));
    let marked = analysis.chunks.iter().any(|(level, _, _)| *level == 0);
    if !marked && !required {
        return None;
    }
    if !analysis.lead_ok {
        return Some(format!("Vor der Teil-Markierung steht Text. {PART_HINT}"));
    }
    match analysis.chunks.as_slice() {
        [] => Some(format!("Der Text des Teils hat keine Teil-Markierung mehr. {PART_HINT}")),
        [(0, _, _)] => None,
        [(0, _, _), rest @ ..] => {
            let titles: Vec<String> = rest.iter().map(|(_, t, _)| quoted(t)).collect();
            Some(format!("Der Teiltext enthält weitere Überschriften ({}). {PART_HINT}", titles.join(", ")))
        }
        _ => Some(format!("Der Text des Teils beginnt nicht mit der Teil-Markierung. {PART_HINT}")),
    }
}

/// Neue Gliederung, wenn der Text von Punkt `index` so gespeichert wird (eine oder mehrere Überschriften).
/// Wie bei [`Store::outline_set_level`] wandern die Unterpunkte um dieselbe Differenz mit; die neuen
/// Punkte stehen direkt hinter dem Punkt (vor seinen Unterpunkten). Die Gültigkeit prüft der Aufrufer.
fn edited_items(items: &[Item], index: usize, analysis: &Analysis) -> Result<Vec<Item>, &'static str> {
    let chunks = &analysis.chunks;
    let mut new = items.to_vec();
    let (level, title, _) = &chunks[0];
    if *level == 0 {
        return Err(TO_PART);
    }
    let end = subtree_end(items, index);
    let diff = i16::from(*level) - i16::from(items[index].level);
    for item in &mut new[index + 1..end] {
        let shifted = i16::from(item.level) + diff;
        if !(1..=3).contains(&shifted) {
            return Err("Mit den Unterpunkten ginge das über Ebene 1 bis 3 hinaus; erst die Unterpunkte umstufen.");
        }
        item.level = shifted as u8;
    }
    new[index].level = *level;
    new[index].title = title.clone();
    new[index].text_done = true;
    for (offset, (level, title, text)) in chunks[1..].iter().enumerate() {
        new.insert(
            index + 1 + offset,
            Item {
                id: None,
                orig: None,
                level: *level,
                title: title.clone(),
                // Ein geteiltes Stück mit der Markierung „ohne Nummer“ zählt ohne Nummer (wie beim Import).
                numbered: analysis.numbered.get(offset + 1).copied().unwrap_or(true),
                body: Some(text.clone()),
                text_done: true,
            },
        );
    }
    Ok(new)
}

/// [`check`] für einen Teil (Runde 14): ohne Markierung (und ohne Markierung in der aktuellen
/// Fassung) wie bisher gültig ohne Gliederungslogik; sonst genau eine Markierung am Anfang, ein
/// anderer Titel benennt um.
async fn check_part(
    conn: &mut SqliteConnection,
    items: &[Item],
    index: usize,
    body: &str,
) -> Result<HeadingCheck, StoreError> {
    let id = items[index].id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
    let required = has_part_marker(&current_body(conn, id).await?);
    if let Some(problem) = part_problem(body, required) {
        return Ok(HeadingCheck { valid: false, message: Some(problem), ..HeadingCheck::default() });
    }
    let analysis = analyze(&body.replace("\r\n", "\n"));
    let Some((level, title, _)) = analysis.chunks.first().filter(|(level, _, _)| *level == 0) else {
        return Ok(HeadingCheck { valid: true, ..HeadingCheck::default() });
    };
    let problem = check_chunk_titles(&analysis.chunks, &items[index].title).err();
    Ok(HeadingCheck {
        title: Some(title.clone()),
        level: Some(*level),
        split: Vec::new(),
        merge: false,
        valid: problem.is_none(),
        message: problem.map(|p| p.to_string()),
    })
}

async fn check(conn: &mut SqliteConnection, work: WorkId, node: usize, body: &str) -> Result<HeadingCheck, StoreError> {
    let outline = load_outline(conn, work).await?;
    let items = &outline.items;
    let index = find(items, work, node)?;
    if items[index].level == 0 {
        return check_part(conn, items, index, body).await;
    }
    let analysis = analyze(&body.replace("\r\n", "\n"));
    if !analysis.lead_ok {
        return Ok(HeadingCheck { valid: false, message: Some(LEAD_TEXT.to_owned()), ..HeadingCheck::default() });
    }
    let Some((level, title, _)) = analysis.chunks.first() else {
        let problem = if index == 0 {
            Some(MERGE_FIRST)
        } else if items[index - 1].level == 0 {
            Some(MERGE_PART)
        } else {
            None
        };
        let message = problem.map_or_else(
            || format!("Ohne Überschrift wird der Text an {} angehängt.", quoted(&items[index - 1].title)),
            str::to_owned,
        );
        return Ok(HeadingCheck {
            merge: true,
            valid: problem.is_none(),
            message: Some(message),
            ..HeadingCheck::default()
        });
    };
    let split = analysis.chunks[1..].iter().map(|(_, t, _)| t.clone()).collect();
    if let Err(problem) = check_chunk_titles(&analysis.chunks, &items[index].title) {
        return Ok(HeadingCheck {
            title: Some(title.clone()),
            level: Some(*level),
            split,
            merge: false,
            valid: false,
            message: Some(problem.to_string()),
        });
    }
    let verdict = edited_items(items, index, &analysis).and_then(|new| check_levels(&new));
    Ok(HeadingCheck {
        title: Some(title.clone()),
        level: Some(*level),
        split,
        merge: false,
        valid: verdict.is_ok(),
        message: verdict.err().map(str::to_owned),
    })
}

/// [`save`] für einen Teil (Runde 14). Ohne Markierung (in Text und aktueller Fassung) wie bisher
/// ohne Gliederungslogik; sonst verlangt der Text genau eine Markierung am Anfang ([`part_problem`]),
/// ein anderer Titel in ihr benennt den Teil um.
#[allow(clippy::too_many_arguments)]
async fn save_part(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    id: i64,
    old_title: &str,
    body: &str,
    based_on: Option<u32>,
    human: &str,
) -> Result<OutlineSave, StoreError> {
    let required = has_part_marker(&current_body(conn, id).await?);
    if let Some(problem) = part_problem(body, required) {
        return Err(StoreError::Heading(problem));
    }
    let analysis = analyze(body);
    let marked = analysis.chunks.first().is_some_and(|(level, _, _)| *level == 0);
    if marked {
        check_chunk_titles(&analysis.chunks, old_title)?;
    }
    let saved = write::save_human(conn, work, node, body, human, based_on).await?;
    if saved.is_none() || !marked || !sync_heading(conn, id).await? {
        return Ok(OutlineSave { change: saved, outline: None, new_reason: None });
    }
    let title = analysis.chunks.first().map(|(_, t, _)| t.clone()).unwrap_or_default();
    let outline = OutlineChange {
        work,
        action: OutlineAction::Renamed,
        node,
        title: title.clone(),
        detail: format!("{} → {} umbenannt", quoted(old_title), quoted(&title)),
        revisions: saved.iter().map(|c| (node, c.rev)).collect(),
        renumbered: Vec::new(),
        removed: Vec::new(),
    };
    Ok(OutlineSave { change: saved, outline: Some(outline), new_reason: None })
}

async fn save(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
    based_on: Option<u32>,
    allow: Restructure,
    human: &str,
) -> Result<OutlineSave, StoreError> {
    let body = body.replace("\r\n", "\n");
    let mut outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let index = find(&outline.items, work, node)?;
    let id = outline.items[index].id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
    if outline.items[index].level == 0 {
        let old_title = outline.items[index].title.clone();
        return save_part(conn, work, node, id, &old_title, &body, based_on, human).await;
    }
    let analysis = analyze(&body);
    if !analysis.lead_ok {
        return Err(StoreError::Refused(LEAD_TEXT));
    }

    if analysis.chunks.is_empty() {
        if !allow.merge {
            return Err(StoreError::Refused(
                "Der Text hat keine Überschrift mehr; zum Zusammenführen mit dem vorigen Gliederungspunkt bestätigen.",
            ));
        }
        if index == 0 {
            return Err(StoreError::Refused(MERGE_FIRST));
        }
        let items = &mut outline.items;
        let previous = &items[index - 1];
        if previous.level == 0 {
            return Err(StoreError::Refused(MERGE_PART));
        }
        if let Some(base) = based_on {
            write::row(conn, id, node, base).await?; // unbekannte Basis → UnknownRevision
        }
        let previous_id = previous.id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
        let previous_seq = previous.orig.as_ref().map_or(index - 1, |o| o.0);
        let previous_title = previous.title.clone();
        let removed_item = items.remove(index);
        check_levels(items).map_err(StoreError::Refused)?;

        let mut joined = current_body(conn, previous_id).await?;
        if !joined.is_empty() && !joined.ends_with('\n') {
            joined.push('\n');
        }
        joined.push_str(&body);
        let saved = write::save_human(conn, work, previous_seq, &joined, human, None).await?;
        // Beiträge wandern mit, hinter die des vorigen Punkts.
        let offset: i64 = sqlx::query_scalar("SELECT coalesce(max(position) + 1, 0) FROM post WHERE node_id = ?")
            .bind(previous_id)
            .fetch_one(&mut *conn)
            .await?;
        sqlx::query("UPDATE post SET node_id = ?, position = position + ? WHERE node_id = ?")
            .bind(previous_id)
            .bind(offset)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        let text = current_body(conn, id).await?;
        remove_node(conn, id).await?;
        let applied = apply(conn, work, &pattern, items, &[node], &Author::Human { name: human.to_owned() }).await?;
        let shown = index - 1;
        let mut revisions: Vec<(usize, u32)> = saved.iter().map(|c| (shown, c.rev)).collect();
        revisions.extend(applied.revisions);
        let detail = format!("{} mit {} zusammengeführt", quoted(&removed_item.title), quoted(&previous_title));
        let outline = OutlineChange {
            work,
            action: OutlineAction::Merged,
            node: shown,
            title: removed_item.title.clone(),
            detail,
            revisions,
            renumbered: applied.renumbered,
            removed: vec![(removed_item.title, text)],
        };
        let change = saved.map(|mut c| {
            c.node = shown;
            c
        });
        return Ok(OutlineSave { change, outline: Some(outline), new_reason: None });
    }

    // Titelregeln vor allem anderen: der erste und jeder geteilte Titel (nichts geschrieben).
    check_chunk_titles(&analysis.chunks, &outline.items[index].title)?;
    if analysis.chunks.len() > 1 && !allow.split {
        return Err(StoreError::Refused("Der Text enthält neue Überschriften; zum Teilen bestätigen."));
    }
    let (old_level, old_title) = (outline.items[index].level, outline.items[index].title.clone());
    let mut items = edited_items(&outline.items, index, &analysis).map_err(StoreError::Refused)?;
    check_levels(&items).map_err(StoreError::Refused)?;
    let (level, title, first_text) = analysis.chunks[0].clone();
    let saved = write::save_human(conn, work, node, &first_text, human, based_on).await?;
    let split = analysis.chunks.len() > 1;
    let level_changed = level != old_level;
    let title_changed = title != old_title;
    if !split && !level_changed {
        if !title_changed {
            return Ok(OutlineSave { change: saved, outline: None, new_reason: None });
        }
        // Nur der Titel: aus der Überschriftszeile der jetzt aktuellen Fassung übernehmen.
        sync_heading(conn, id).await?;
        let detail = format!("{} → {} umbenannt", quoted(&old_title), quoted(&title));
        let outline = OutlineChange {
            work,
            action: OutlineAction::Renamed,
            node,
            title,
            detail,
            revisions: saved.iter().map(|c| (node, c.rev)).collect(),
            renumbered: Vec::new(),
            removed: Vec::new(),
        };
        return Ok(OutlineSave { change: saved, outline: Some(outline), new_reason: None });
    }
    let applied = apply(conn, work, &pattern, &mut items, &[], &Author::Human { name: human.to_owned() }).await?;
    let mut revisions: Vec<(usize, u32)> = saved.iter().map(|c| (index, c.rev)).collect();
    revisions.extend(applied.revisions);
    let (action, detail) = if split {
        let parts: Vec<String> = analysis.chunks[1..].iter().map(|(_, t, _)| quoted(t)).collect();
        (OutlineAction::Split, format!("{} geteilt: {} neu", quoted(&title), parts.join(", ")))
    } else if title_changed {
        (
            OutlineAction::Level,
            format!("{} → {} umbenannt, Ebene {old_level} → {level}", quoted(&old_title), quoted(&title)),
        )
    } else {
        (OutlineAction::Level, format!("{} Ebene {old_level} → {level}", quoted(&title)))
    };
    let outline = OutlineChange {
        work,
        action,
        node: index,
        title,
        detail,
        revisions,
        renumbered: applied.renumbered,
        removed: Vec::new(),
    };
    let change = saved.map(|mut c| {
        c.node = index;
        c
    });
    Ok(OutlineSave { change, outline: Some(outline), new_reason: None })
}

/// [`save`] im Modus `Auto` (Runde 22, Vertrag §2.1/2.2): Bleiben Überschrift und Ebene (kein Teilen, kein
/// Zusammenführen, beim Teil kein neuer Titel) und erlauben die Regeln es ([`crate::amend::verdict`]), wird die
/// aktuelle Fassung des Menschen fortgeschrieben; sonst wie [`save`] eine neue Fassung mit Grund
/// (`OutlineSave::new_reason`). Ungültiges (Text vor der Überschrift, kaputte Teil-Markierung) meldet [`save`].
#[allow(clippy::too_many_arguments)]
pub(crate) async fn save_auto(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
    based_on: Option<u32>,
    based_on_amend: Option<u32>,
    allow: Restructure,
    human: &str,
) -> Result<OutlineSave, StoreError> {
    let body = body.replace("\r\n", "\n");
    let outline = load_outline(conn, work).await?;
    let index = find(&outline.items, work, node)?;
    let item = &outline.items[index];
    let id = item.id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
    // (gespeicherter Text, Überschrift geändert?) – `None`: ungültig, das meldet `save` selbst.
    let plan: Option<(String, bool)> = if item.level == 0 {
        let required = has_part_marker(&current_body(conn, id).await?);
        if part_problem(&body, required).is_some() {
            None
        } else {
            let analysis = analyze(&body);
            let renamed = analysis.chunks.first().is_some_and(|(level, title, _)| *level == 0 && *title != item.title);
            Some((body.clone(), renamed))
        }
    } else {
        let analysis = analyze(&body);
        match analysis.chunks.as_slice() {
            _ if !analysis.lead_ok => None,
            [(level, title, text)] => Some((text.clone(), *level != item.level || *title != item.title)),
            // Keine oder weitere Überschriften: Zusammenführen bzw. Teilen ändert die Gliederung.
            _ => Some((body.clone(), true)),
        }
    };
    let Some((stored, heading_changed)) = plan else {
        return save(conn, work, node, &body, based_on, allow, human).await;
    };
    if let Some(base) = based_on {
        write::row(conn, id, node, base).await?; // unbekannte Basis → UnknownRevision
    }
    match crate::amend::verdict(conn, work, id, based_on, based_on_amend, &stored, heading_changed).await? {
        crate::amend::Verdict::Amend(current) => {
            let change =
                crate::amend::write_step(conn, work, node, &current, &stored, human, crate::AmendKind::Amend, None)
                    .await?;
            Ok(OutlineSave { change: Some(change), outline: None, new_reason: None })
        }
        crate::amend::Verdict::New(reason) => {
            let mut done = save(conn, work, node, &body, based_on, allow, human).await?;
            if done.change.is_some() || done.outline.is_some() {
                done.new_reason = Some(reason);
            }
            Ok(done)
        }
        crate::amend::Verdict::Unchanged => save(conn, work, node, &body, based_on, allow, human).await,
    }
}

/// Schema aus seiner id; unbekannt → [`StoreError::Numbering`].
fn scheme_of(id: &str) -> Result<Scheme, StoreError> {
    Scheme::from_id(id).ok_or_else(|| {
        let known: Vec<&str> = Scheme::ALL.iter().map(|s| s.id()).collect();
        StoreError::Numbering(format!("unbekanntes Gliederungsschema „{}“; bekannt: {}", id.trim(), known.join(", ")))
    })
}

/// Eigenes Muster: das übergebene, sonst das gespeicherte (`work.numbering`), leer = keins.
fn custom_spec<'a>(custom: Option<&'a str>, stored: Option<&'a str>) -> Option<&'a str> {
    custom.map(str::trim).filter(|c| !c.is_empty()).or_else(|| stored.map(str::trim).filter(|c| !c.is_empty()))
}

/// Nummerierung eines (noch nicht gespeicherten) Schemas für das Werk; ungültiges Muster →
/// [`StoreError::Numbering`].
fn numbering_for(outline: &Outline, scheme: Scheme, custom: Option<&str>) -> Result<Numbering, StoreError> {
    let custom = custom_spec(custom, outline.numbering.as_deref());
    Numbering::of_scheme(scheme, custom, &outline.template).map_err(StoreError::Numbering)
}

/// Nummern der Gliederung in Baumreihenfolge nach `numbering`.
fn outline_numbers(outline: &Outline, numbering: &Numbering) -> Vec<String> {
    let parents = parents(&outline.items);
    let counted: Vec<Numbered> = outline
        .items
        .iter()
        .zip(&parents)
        .map(|(item, &parent)| Numbered { kind: kind_of(item.level), parent, numbered: item.numbered })
        .collect();
    load::numbers(numbering, &counted)
}

/// Anzeige der Nummerierung aus Schema-id, eigenem Muster und Werkvorlage.
fn numbering_info(scheme: &str, numbering: Option<String>, template: &str) -> NumberingInfo {
    let effective_numbering = load::work_numbering(scheme, numbering.as_deref(), template);
    let scheme = Scheme::from_id(scheme).unwrap_or(Scheme::Template);
    let spec = effective_numbering.spec();
    let custom = numbering.clone().filter(|n| !n.trim().is_empty());
    let label = match spec.as_deref() {
        _ if scheme == Scheme::None => "keine".to_owned(),
        Some("") => "keine".to_owned(),
        Some(pattern) => Scheme::matching(pattern).map_or_else(|| pattern.to_owned(), |s| s.label().to_owned()),
        None => scheme.label().to_owned(),
    };
    NumberingInfo {
        label,
        numbering: match scheme {
            Scheme::Template => None,
            Scheme::Custom => numbering,
            _ => spec.clone(),
        },
        template: baruch_typst::heading_numbering(template),
        effective: spec.unwrap_or_else(|| scheme.example().to_owned()),
        scheme: scheme.id().to_owned(),
        scheme_label: scheme.label().to_owned(),
        custom,
    }
}

async fn set_numbering_scheme(
    conn: &mut SqliteConnection,
    work: WorkId,
    scheme: &str,
    custom: Option<&str>,
) -> Result<OutlineChange, StoreError> {
    let scheme = scheme_of(scheme)?;
    let outline = load_outline(conn, work).await?;
    let numbering = numbering_for(&outline, scheme, custom)?;
    // Beim eigenen Muster wird es gespeichert; sonst bleibt `work.numbering`, wie es ist.
    let stored = match scheme {
        Scheme::Custom => custom_spec(custom, outline.numbering.as_deref()).map(str::to_owned),
        _ => outline.numbering.clone(),
    };
    if outline.scheme == scheme.id() && stored == outline.numbering {
        return Err(StoreError::Refused("Die Nummerierung ist schon so eingestellt."));
    }
    sqlx::query("UPDATE work SET numbering_scheme = ?, numbering = ? WHERE id = ?")
        .bind(scheme.id())
        .bind(&stored)
        .bind(work)
        .execute(&mut *conn)
        .await?;
    write_numbers(conn, &numbering, &outline.items, &parents(&outline.items)).await?;
    let spec = numbering.spec().unwrap_or_default();
    let detail = match scheme {
        Scheme::None => "keine Nummern".to_owned(),
        Scheme::Custom => format!("Nummerierung {}", quoted(&spec)),
        Scheme::Template => format!("Nummerierung aus der Vorlage ({})", quoted(&spec)),
        _ => format!("Gliederungsschema {}", quoted(scheme.label())),
    };
    let title = outline.items.first().map(|i| i.title.clone()).unwrap_or_default();
    Ok(change(
        work,
        OutlineAction::Numbering,
        0,
        &title,
        detail,
        Applied { revisions: Vec::new(), renumbered: Vec::new(), kept_parts: Vec::new() },
    ))
}

const HEADING_HINT: &str =
    "Ebene ändern, Teilen und Zusammenführen gehen nur über „Speichern“ im Editor oder die Gliederung.";

/// Ebene des Knotens wie in [`sync_heading`] (Teil 0, Kapitel 1, Abschnitt 2, unter einem Abschnitt
/// 3); `None` für unbekannte Knoten.
async fn heading_level(conn: &mut SqliteConnection, node_id: i64) -> Result<Option<(u8, String)>, StoreError> {
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT n.kind, n.title, p.kind FROM node n \
         LEFT JOIN node p ON p.work_id = n.work_id AND p.seq = n.parent_seq WHERE n.id = ?",
    )
    .bind(node_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((kind, title, parent_kind)) = row else { return Ok(None) };
    let level = match words::parse_node_kind(&kind)? {
        NodeKind::Part => 0,
        NodeKind::Chapter => 1,
        NodeKind::Section if parent_kind.as_deref() == Some("section") => 3,
        NodeKind::Section => 2,
    };
    Ok(Some((level, title)))
}

pub(crate) fn level_word(level: u8) -> &'static str {
    match level {
        0 => "Teil",
        1 => "Kapitel",
        2 => "Abschnitt",
        _ => "Unterabschnitt",
    }
}

/// Was an den Überschriften eines Texts für einen Punkt der Ebene `level` nicht passt: Text vor der
/// Überschrift, keine Überschrift, weitere Überschriften oder eine andere Ebene. `None` = passt
/// (ein anderer Titel derselben Ebene passt: er benennt den Punkt um).
fn heading_problem(body: &str, level: u8) -> Option<String> {
    let analysis = analyze(&body.replace("\r\n", "\n"));
    if !analysis.lead_ok {
        return Some(format!("{LEAD_TEXT} {HEADING_HINT}"));
    }
    match analysis.chunks.as_slice() {
        [] => Some(format!("Der Text hat keine Überschrift; Gliederung und Text liefen auseinander. {HEADING_HINT}")),
        [_, rest @ ..] if !rest.is_empty() => {
            let titles: Vec<String> = rest.iter().map(|(_, t, _)| quoted(t)).collect();
            Some(format!(
                "Der Text enthält weitere Überschriften ({}); so änderte sich die Gliederung. {HEADING_HINT}",
                titles.join(", ")
            ))
        }
        [(found, _, _)] if *found != level => Some(format!(
            "Die Überschrift hat Ebene {found} ({}), der Gliederungspunkt Ebene {level} ({}). {HEADING_HINT}",
            level_word(*found),
            level_word(level)
        )),
        _ => None,
    }
}

/// Nach jeder Änderung der aktuellen Fassung außerhalb des Editors (Übernehmen, Umschalten,
/// Speichern über `revision.save`, Fassung ändern), in der Transaktion des Aufrufers: Passen die
/// Überschriften der jetzt aktuellen Fassung nicht zum Gliederungspunkt → `Heading` (der Aufrufer
/// rollt zurück); sonst den Titel aus der Überschriftszeile übernehmen ([`sync_heading`]).
/// So bleiben Gliederung und Überschriftszeilen gleich, und Export und Import ergeben dieselbe
/// Gliederung.
pub(crate) async fn check_current_heading(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
) -> Result<bool, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    if let Some((level, old_title)) = heading_level(conn, node_id).await? {
        let body = current_body(conn, node_id).await?;
        // Teile (Runde 14): nur eine Fassung mit Markierung wird geprüft und gleicht den Titel ab.
        let problem = if level == 0 { part_problem(&body, false) } else { heading_problem(&body, level) };
        if let Some(problem) = problem {
            return Err(StoreError::Heading(problem));
        }
        // Der Titel des Punkts ist hier noch der alte (`sync_heading` folgt erst danach).
        if level != 0 || has_part_marker(&body) {
            check_chunk_titles(&analyze(&body.replace("\r\n", "\n")).chunks, &old_title)?;
        }
    }
    sync_heading(conn, node_id).await
}

/// Vor dem Anlegen eines Vorschlags: passt sein Text zum Gliederungspunkt? Sonst `Heading` – ein
/// Vorschlag ändert nie die Gliederung, und übernehmen ließe er sich ohnehin nicht.
pub(crate) async fn check_proposal_heading(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
) -> Result<(), StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    if let Some((level, old_title)) = heading_level(conn, node_id).await? {
        // Teile (Runde 14): Markierung verlangt, wenn der Vorschlag oder die aktuelle Fassung eine hat.
        let problem = if level == 0 {
            part_problem(body, has_part_marker(&current_body(conn, node_id).await?))
        } else {
            heading_problem(body, level)
        };
        if let Some(problem) = problem {
            return Err(StoreError::Heading(format!("Vorschlag abgelehnt: {problem}")));
        }
        if level != 0 || has_part_marker(body) {
            check_chunk_titles(&analyze(&body.replace("\r\n", "\n")).chunks, &old_title)?;
        }
    }
    Ok(())
}

/// Titel des Knotens aus der Überschriftszeile seiner aktuellen Fassung übernehmen, aber nur, wenn
/// deren Ebene zur Ebene des Knotens passt (Teile nur mit Teil-Markierung, Runde 14). `true`, wenn sich der Titel geändert hat.
/// Läuft in der Transaktion des Aufrufers (nach jeder Änderung der aktuellen Fassung).
pub(crate) async fn sync_heading(conn: &mut SqliteConnection, node_id: i64) -> Result<bool, StoreError> {
    let Some((level, title)) = heading_level(conn, node_id).await? else { return Ok(false) };
    let body = current_body(conn, node_id).await?;
    let Some(found) = first_heading(&body) else { return Ok(false) };
    if found.level != level || found.title == title {
        return Ok(false);
    }
    sqlx::query("UPDATE node SET title = ? WHERE id = ?").bind(&found.title).bind(node_id).execute(&mut *conn).await?;
    Ok(true)
}

const SWITCH_HINT: &str = "Teilen, Zusammenführen und Text vor der Überschrift gehen über „Speichern“ im Editor.";

/// Nach dem Umschalten (Vertrag Runde 5, Nutzerentscheidung „Nein“): Die Gliederung folgt der
/// Überschriftszeile der jetzt aktuellen Fassung, die Fassung selbst wird nie umgeschrieben.
///
/// - Teile (Runde 14): Fassung mit Teil-Markierung → genau eine am Anfang, der Titel folgt; Fassung
///   ohne Markierung → nichts zu tun (wie bisher), `Ok(None)`.
/// - Gleiche Ebene: der Titel folgt wie bisher ([`sync_heading`]), `Ok(None)`.
/// - Andere Ebene: Ebene und Titel folgen; alle Unterpunkte wandern um dieselbe Differenz mit
///   und bekommen je eine neue Fassung des Menschen, die sich nur in der
///   Überschriftszeile unterscheidet. Ergebnis: eine Änderung mit `action: Level`.
/// - Keine Überschrift, weitere Überschriften, Text davor, eine an dieser Stelle ungültige Ebene oder
///   ein Unterpunkt, der tiefer als 3 käme → `Heading` (der Aufrufer rollt alles zurück, auch das Umschalten).
pub(crate) async fn follow_switched(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    human: &str,
) -> Result<Option<OutlineChange>, StoreError> {
    let outline = load_outline(conn, work).await?;
    let pattern = outline.pattern();
    let index = find(&outline.items, work, node)?;
    let item = &outline.items[index];
    let id = item.id.ok_or_else(|| broken("Gliederungspunkt ohne id".into()))?;
    if item.level == 0 {
        let body = current_body(conn, id).await?;
        if let Some(problem) = part_problem(&body, false) {
            return Err(StoreError::Heading(format!("{problem} {SWITCH_HINT}")));
        }
        if has_part_marker(&body) {
            check_chunk_titles(&analyze(&body).chunks, &item.title)?;
        }
        sync_heading(conn, id).await?;
        return Ok(None);
    }
    let (old_level, old_title) = (item.level, item.title.clone());
    let body = current_body(conn, id).await?;
    let analysis = analyze(&body);
    if !analysis.lead_ok {
        return Err(StoreError::Heading(format!("{LEAD_TEXT} {SWITCH_HINT}")));
    }
    let (level, title) = match analysis.chunks.as_slice() {
        [] => {
            return Err(StoreError::Heading(format!(
                "Die Fassung hat keine Überschrift; die Gliederung wüsste nicht, wohin sie gehört. {SWITCH_HINT}"
            )));
        }
        // Wie beim Speichern nur ein geänderter Titel (Runde 8).
        [(0, _, _), ..] => {
            return Err(StoreError::Heading(format!("{TO_PART} {SWITCH_HINT}")));
        }
        [(level, title, _)] if *title == old_title => (*level, title.clone()),
        [(level, title, _)] => (*level, check_title(title)?),
        [_, rest @ ..] => {
            let titles: Vec<String> = rest.iter().map(|(_, t, _)| quoted(t)).collect();
            return Err(StoreError::Heading(format!(
                "Die Fassung enthält weitere Überschriften ({}); Umschalten teilt keinen Abschnitt. {SWITCH_HINT}",
                titles.join(", ")
            )));
        }
    };
    if level == old_level {
        sync_heading(conn, id).await?;
        return Ok(None);
    }
    let mut items = outline.items.clone();
    let end = subtree_end(&items, index);
    let diff = i16::from(level) - i16::from(old_level);
    for sub in &mut items[index + 1..end] {
        // Unterpunkte stehen tiefer als der Punkt, also nach der Verschiebung mindestens auf Ebene 2.
        // Tiefer als 3 wird nicht gekappt, sondern abgelehnt: Kappen machte aus einem Kind ein
        // Geschwister, und Zurückschalten stellte die alte Gliederung nicht wieder her.
        let shifted = i16::from(sub.level) + diff;
        if shifted > 3 {
            return Err(StoreError::Heading(format!(
                "Die Fassung bringt Ebene {level} ({}) mit; {} käme damit tiefer als Ebene 3 \
                 (Unterabschnitt). Erst Unterpunkte höher stufen, dann umschalten.",
                level_word(level),
                quoted(&sub.title)
            )));
        }
        sub.level = shifted.max(1) as u8;
    }
    items[index].level = level;
    items[index].title = title.clone();
    items[index].text_done = true; // die Fassung bleibt, wie sie ist
    if let Err(problem) = check_levels(&items) {
        return Err(StoreError::Heading(format!(
            "Die Fassung bringt Ebene {level} ({}) mit, das geht an dieser Stelle nicht: {problem}",
            level_word(level)
        )));
    }
    let applied = apply(conn, work, &pattern, &mut items, &[], &Author::Human { name: human.to_owned() }).await?;
    let detail = if title == old_title {
        format!("{} Ebene {old_level} → {level}", quoted(&title))
    } else {
        format!("{} → {} umbenannt, Ebene {old_level} → {level}", quoted(&old_title), quoted(&title))
    };
    Ok(Some(change(work, OutlineAction::Level, index, &title, detail, applied)))
}

impl Store {
    /// Gliederungspunkt samt Unterpunkten verschieben (atomar): vor oder hinter `target` (auf dessen
    /// Ebene) bzw. als letzten Unterpunkt darunter. Die Art bleibt (Teil, Kapitel, Abschnitt); in den
    /// eigenen Teilbaum, an eine ungültige Stelle oder ohne Wirkung → `Refused`.
    pub async fn outline_move(
        &self,
        work: WorkId,
        node: usize,
        target: usize,
        place: Place,
        human: &str,
    ) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = move_node(&mut tx, work, node, target, place, human).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Ebene ändern (1–3, atomar); Unterpunkte wandern um dieselbe Differenz mit. Teile und Ebene 0 →
    /// `Refused`, ebenso eine ungültige Gliederung.
    pub async fn outline_set_level(
        &self,
        work: WorkId,
        node: usize,
        level: u8,
        human: &str,
    ) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = set_level(&mut tx, work, node, level, human).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Umbenennen (atomar): neuer Titel und neue Überschriftszeile (neue Fassung des Menschen).
    pub async fn outline_rename(
        &self,
        work: WorkId,
        node: usize,
        title: &str,
        human: &str,
    ) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = rename(&mut tx, work, node, title, human).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Neuer Gliederungspunkt (atomar): vor/hinter `target` auf dessen Ebene, `into` als letzter
    /// Unterpunkt (Ebene + 1). Revision 1 vom Menschen = Überschriftszeile + Leerzeile (beim Teil die
    /// Teil-Markierung, Runde 14; Teile auch mit Werkvorlage).
    pub async fn outline_add(
        &self,
        work: WorkId,
        target: usize,
        place: Place,
        title: &str,
        human: &str,
    ) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = add(&mut tx, work, target, place, title, human).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Mehrere neue Gliederungspunkte in **einer** Transaktion (Wunsch 2026-11-06, MCP `outline_add`):
    /// ein Block vor/hinter/unter `target` ([`NewPoint`]: Titel und absolute Ebene); bei einem Fehler
    /// (Titelregeln, Ebenen, Teil an falscher Stelle) wird nichts angelegt. Revision 1 jedes Punkts =
    /// Überschriftszeile + Leerzeile vom `author` (Mensch, Persona oder MCP-Client; beim Teil die
    /// Teil-Markierung). Teile (Ebene 0, Runde 14) vor oder hinter einem Teil oder Kapitel, auch mit
    /// Werkvorlage; `adopted` nennt die vorhandenen Kapitel, die jetzt unter einem neuen Teil stehen.
    /// `target = None` nur in einem Werk ohne Gliederungspunkt.
    pub async fn outline_add_many(
        &self,
        work: WorkId,
        target: Option<usize>,
        place: Place,
        points: &[NewPoint],
        author: &Author,
    ) -> Result<OutlineAdded, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = add_many(&mut tx, work, target, place, points, author).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Einfall als Gliederungspunkt verarbeiten (Runde 14, `idea.process` mit `as: outline`), in
    /// **einer** Transaktion: Punkt wie [`Store::outline_add`] anlegen (Revision 1 vom Menschen) und
    /// den Einfall als verarbeitet markieren (`done_as` outline, `done_ref` „key <key>“). Scheitert
    /// etwas (Einfall gelöscht, aus einem anderen Werk, Titelregeln, Ort), ist nichts geändert.
    pub async fn idea_to_outline(
        &self,
        idea: IdeaId,
        work: WorkId,
        target: usize,
        place: Place,
        title: &str,
        human: &str,
    ) -> Result<IdeaOutline, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = crate::ideas::get(&mut tx, idea).await?;
        if before.work != Some(work) {
            return Err(StoreError::Idea(format!("Einfall {idea} gehört nicht zu diesem Werk.")));
        }
        let change = add(&mut tx, work, target, place, title, human).await?;
        let key: i64 = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
            .bind(work)
            .bind(change.node as i64)
            .fetch_one(&mut *tx)
            .await?;
        let idea = crate::ideas::mark_done_in(&mut tx, idea, IdeaUse::Outline, &format!("key {key}")).await?;
        tx.commit().await?;
        Ok(IdeaOutline { change, key, idea })
    }

    /// Punkt mit (`true`) bzw. ohne Nummer (`false`) zählen (atomar, nur Mensch); alle Nummern neu.
    pub async fn outline_set_numbered(
        &self,
        work: WorkId,
        node: usize,
        numbered: bool,
        human: &str,
    ) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = set_numbered(&mut tx, work, node, numbered, human).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Feste Schlüssel der Punkte ohne Nummer (`node.numbered = 0`) eines Werks.
    pub async fn unnumbered_keys(&self, work: WorkId) -> Result<Vec<i64>, StoreError> {
        Ok(sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND numbered = 0 ORDER BY seq")
            .bind(work)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Gliederungspunkt samt Fassungen und Beiträgen löschen (atomar, nur Mensch). Mit Unterpunkten
    /// oder als letztes Kapitel → `Refused`. `removed` trägt Titel und Text der aktuellen Fassung.
    pub async fn outline_delete(&self, work: WorkId, node: usize, human: &str) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = delete(&mut tx, work, node, human).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Was würde [`Store::outline_save`] mit diesem Text an der Gliederung ändern? Schreibt nichts.
    pub async fn outline_check(&self, work: WorkId, node: usize, body: &str) -> Result<HeadingCheck, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check(&mut conn, work, node, body).await
    }

    /// Text aus dem Editor speichern (atomar) und die Gliederung an die Überschriften angleichen:
    /// eine Überschrift → speichern wie [`Store::save_human_on`], Titel und Ebene angleichen; weitere
    /// Überschriften → nur mit `allow.split` neue Punkte dahinter; keine Überschrift → nur mit
    /// `allow.merge` an den vorigen Punkt anhängen (Beiträge wandern mit, der Punkt entfällt).
    pub async fn outline_save(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        based_on: Option<u32>,
        allow: Restructure,
        human: &str,
    ) -> Result<OutlineSave, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = save(&mut tx, work, node, body, based_on, allow, human).await?;
        if done.change.is_some() || done.outline.is_some() {
            tx.commit().await?;
        }
        Ok(done)
    }

    /// Nummerierungsmuster des Werks setzen (atomar, wie vor Runde 6): `None` = aus der Werkvorlage,
    /// `""` = keine Nummern, sonst ein Muster der Mustersprache wie „1.1“, „I|1“ oder „I.;A.;1.“
    /// (Schema „Eigenes Muster“). Alle Nummern werden neu geschrieben; ungültiges Muster →
    /// [`StoreError::Numbering`], unverändert → `Refused`.
    pub async fn set_numbering(&self, work: WorkId, numbering: Option<&str>) -> Result<OutlineChange, StoreError> {
        let (scheme, custom) = match numbering.map(str::trim) {
            None => (Scheme::Template, None),
            Some("") => (Scheme::None, None),
            Some(spec) => (Scheme::Custom, Some(spec)),
        };
        self.set_numbering_scheme(work, scheme.id(), custom).await
    }

    /// Gliederungsschema des Werks wählen (atomar, Vertrag Runde 6): `scheme` ist eine id aus
    /// [`Scheme::ALL`], `custom` das eigene Muster (nur bei `custom`; fehlt es, gilt das gespeicherte).
    /// Unbekanntes Schema oder ungültiges Muster → [`StoreError::Numbering`], unverändert → `Refused`.
    pub async fn set_numbering_scheme(
        &self,
        work: WorkId,
        scheme: &str,
        custom: Option<&str>,
    ) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = set_numbering_scheme(&mut tx, work, scheme, custom).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// Nummern der ganzen Gliederung, wie sie mit `scheme` (und `custom`) wären; schreibt nichts.
    pub async fn numbering_preview(
        &self,
        work: WorkId,
        scheme: &str,
        custom: Option<&str>,
    ) -> Result<Vec<NumberPreview>, StoreError> {
        let scheme = scheme_of(scheme)?;
        let mut conn = self.pool.acquire().await?;
        let outline = load_outline(&mut conn, work).await?;
        let numbering = numbering_for(&outline, scheme, custom)?;
        let numbers = outline_numbers(&outline, &numbering);
        Ok(outline
            .items
            .iter()
            .zip(numbers)
            .enumerate()
            .map(|(index, (item, number))| NumberPreview {
                node: item.orig.as_ref().map_or(index, |o| o.0),
                key: item.id.unwrap_or_default(),
                level: item.level,
                number,
                title: item.title.clone(),
            })
            .collect())
    }

    /// Regel `#set heading(numbering: …)`, die der Export mit `scheme` hinter die Werkvorlage setzt
    /// (`None` bei „aus der Werkvorlage“: dann bleibt die Vorlage unverändert); schreibt nichts.
    pub async fn numbering_rule(
        &self,
        work: WorkId,
        scheme: &str,
        custom: Option<&str>,
    ) -> Result<Option<String>, StoreError> {
        let scheme = scheme_of(scheme)?;
        let mut conn = self.pool.acquire().await?;
        let outline = load_outline(&mut conn, work).await?;
        let numbering = numbering_for(&outline, scheme, custom)?;
        Ok((scheme != Scheme::Template).then(|| numbering.heading_rule()))
    }

    /// Eingestellte, aus der Vorlage gelesene und gültige Nummerierung des Werks.
    pub async fn outline_numbering(&self, work: WorkId) -> Result<NumberingInfo, StoreError> {
        let head: Option<(String, Option<String>, String)> =
            sqlx::query_as("SELECT template, numbering, numbering_scheme FROM work WHERE id = ?")
                .bind(work)
                .fetch_optional(&self.pool)
                .await?;
        let (template, numbering, scheme) = head.ok_or(StoreError::UnknownWork(work))?;
        Ok(numbering_info(&scheme, numbering, &template))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(level: u8) -> Item {
        Item { id: None, orig: None, level, title: String::new(), numbered: true, body: None, text_done: false }
    }

    #[test]
    fn level_sequences_must_not_skip_a_level() {
        let ok: Vec<Item> = [1, 2, 3, 2, 1, 0, 1, 2].into_iter().map(item).collect();
        assert!(check_levels(&ok).is_ok(), "jede Ebene höchstens eine tiefer, Teile auf oberster Ebene");
        let jump: Vec<Item> = [1, 3].into_iter().map(item).collect();
        assert!(check_levels(&jump).is_err(), "Ebene 1 → 3 überspringt eine Ebene");
        let start: Vec<Item> = [2].into_iter().map(item).collect();
        assert!(check_levels(&start).is_err(), "der erste Nicht-Teil muss ein Kapitel sein");
        let deep: Vec<Item> = [1, 2, 3, 4].into_iter().map(item).collect();
        assert!(check_levels(&deep).is_err(), "Ebene 4 gibt es nicht");
        let after_part: Vec<Item> = [0, 2].into_iter().map(item).collect();
        assert!(check_levels(&after_part).is_err(), "unter einem Teil zuerst ein Kapitel");
    }

    #[test]
    fn parents_follow_the_levels_like_the_import() {
        let items: Vec<Item> = [1, 2, 3, 2, 0, 1, 2].into_iter().map(item).collect();
        assert_eq!(
            parents(&items),
            vec![None, Some(0), Some(1), Some(0), None, Some(4), Some(5)],
            "Elternteil = nächster Vorgänger mit kleinerer Ebene"
        );
    }

    #[test]
    fn rewriting_the_heading_changes_only_its_line() {
        let body = "== Alt \\'s <lab> // Notiz\n\nText bleibt.\n";
        assert_eq!(
            rewrite_heading(body, 1, "Alt 's", false),
            "= Alt \\'s <lab> // Notiz\n\nText bleibt.\n",
            "nur die Ebene: Zeile sonst wörtlich"
        );
        assert_eq!(
            rewrite_heading(body, 2, "Neu", true),
            "== Neu <lab> // Notiz\n\nText bleibt.\n",
            "neuer Titel: Label und Kommentar bleiben"
        );
        assert_eq!(rewrite_heading("Nur Text\n", 2, "Titel", true), "== Titel\nNur Text\n", "ohne Überschrift davor");
    }
}
