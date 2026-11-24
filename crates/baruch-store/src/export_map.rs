// SPDX-License-Identifier: AGPL-3.0-or-later
//! Karte des Exports (Vertrag Runde 8, Abschnitt 4b): dieselbe Typst-Datei wie
//! [`Store::export_typst`] und dazu je Abschnitt sein Bytebereich und seine erste Zeile darin.
//!
//! Die Typst-Vorschau setzt den Export als eine Datei; Meldungen von Typst tragen eine Zeile in
//! dieser Datei. Über die Karte findet der Server den Abschnitt (fester Schlüssel, Nummer, Zeile im
//! Abschnitt) bzw. „Werkvorlage“ für Zeilen vor dem ersten Abschnitt, und die Seite, auf der ein
//! Abschnitt beginnt. Invariante: `export_map(w).typst == export_typst(w)`, Byte für Byte – beide
//! entstehen aus `snapshot::export_parts` (seit Runde 31 samt Buchende über `snapshot::append_tail`).

use serde::Serialize;

use crate::{Store, StoreError, WorkId, snapshot};

/// Ein Abschnitt im Export: fester Schlüssel (`Node::key`), Stelle (`seq`), Bytebereich
/// `start..end` in [`ExportMap::typst`] und die Zeile seines ersten Bytes (1-basiert, gezählt wie
/// Typst, [`baruch_typst::line_at`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportSection {
    pub key: i64,
    pub seq: usize,
    pub start: usize,
    pub end: usize,
    pub first_line: usize,
}

/// Der Export und seine Abschnitte in Baumreihenfolge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportMap {
    pub typst: String,
    pub sections: Vec<ExportSection>,
}

impl ExportMap {
    /// Der Abschnitt, zu dem Zeile `line` (1-basiert) gehört; `None` = Werkvorlage (vor dem ersten
    /// Abschnitt) oder – seit Runde 23a – Buchende ([`ExportMap::in_tail`]: Register, Glossar und was sonst hinter dem
    /// letzten Abschnitt steht). Leere Abschnitte haben keine eigene Zeile und werden übersprungen.
    pub fn section_at_line(&self, line: usize) -> Option<&ExportSection> {
        if self.in_tail(line) {
            return None;
        }
        self.sections.iter().filter(|s| s.start < s.end).take_while(|s| s.first_line <= line).last()
    }

    /// Runde 23a (Integration R31): erste Zeile (1-basiert) des Buchendes, also des ersten Zeichens hinter dem letzten
    /// Abschnitt, das kein Leerraum ist. `None` ohne Abschnitte oder ohne Buchende. Vorher galten Meldungen aus dem
    /// Buchende (z. B. ein Fehler in einer Registerzeile) als Meldungen des letzten Abschnitts.
    pub fn tail_first_line(&self) -> Option<usize> {
        let end = self.sections.iter().map(|s| s.end).max()?;
        let rest = self.typst.get(end..)?;
        let offset = rest.find(|c: char| !c.is_whitespace())?;
        Some(baruch_typst::line_at(&self.typst, end + offset))
    }

    /// Liegt Zeile `line` im Buchende ([`ExportMap::tail_first_line`])?
    pub fn in_tail(&self, line: usize) -> bool {
        self.tail_first_line().is_some_and(|first| line >= first)
    }
}

impl Store {
    /// Export als Typst mit Karte der Abschnitte. Unbekanntes Werk → `UnknownWork`.
    pub async fn export_map(&self, work: WorkId) -> Result<ExportMap, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let (template, sections, tail) = snapshot::export_parts(&mut conn, work).await?;
        let (mut typst, ranges) = baruch_typst::join_with_offsets(&template, sections.iter().map(|s| s.body.as_str()));
        // Buchende (Runde 31) hinter dem letzten Abschnitt; die Abschnittsbereiche bleiben, wie sie sind.
        snapshot::append_tail(&mut typst, &tail);
        // Zeilen fortlaufend zählen (sonst quadratisch im Umfang des Buchs): von der vorigen
        // Abschnittsgrenze bis zur nächsten. Vor jeder Grenze außer der ersten endet ein Text auf
        // `\n` (bzw. ist leer), ein `\r\n` wird also nie zerschnitten; die erste zählt ab Byte 0.
        let mut out = Vec::with_capacity(sections.len());
        let (mut at, mut line) = (0, 1);
        for (section, range) in sections.iter().zip(ranges) {
            line += baruch_typst::line_at(&typst[at..], range.start - at) - 1;
            at = range.start;
            out.push(ExportSection {
                key: section.key,
                seq: section.seq,
                start: range.start,
                end: range.end,
                first_line: line,
            });
        }
        Ok(ExportMap { typst, sections: out })
    }
}
