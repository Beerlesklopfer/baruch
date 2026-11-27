// SPDX-License-Identifier: AGPL-3.0-or-later
//! Map of the export (contract Round 8, section 4b): the same Typst file as [`Store::export_typst`] and, in addition,
//! per section its byte range and its first line in it.
//!
//! The Typst preview typesets the export as one file; messages from Typst carry a line in this file. Through the map
//! the server finds the section (fixed key, number, line in the section) or „Werkvorlage“ for lines before the first
//! section, and the page on which a section begins. Invariant: `export_map(w).typst == export_typst(w)`, byte for byte
//! – both arise from `snapshot::export_parts` (since Round 31 together with the end of the book through
//! `snapshot::append_tail`).

use serde::Serialize;

use crate::{Store, StoreError, WorkId, snapshot};

/// A section in the export: fixed key (`Node::key`), position (`seq`), byte range `start..end` in [`ExportMap::typst`]
/// and the line of its first byte (1-based, counted like Typst, [`baruch_typst::line_at`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportSection {
    pub key: i64,
    pub seq: usize,
    pub start: usize,
    pub end: usize,
    pub first_line: usize,
}

/// The export and its sections in tree order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportMap {
    pub typst: String,
    pub sections: Vec<ExportSection>,
}

impl ExportMap {
    /// The section to which line `line` (1-based) belongs; `None` = work template (before the first section) or – since
    /// Round 23a – end of the book ([`ExportMap::in_tail`]: index, glossary and whatever else stands behind the last
    /// section). Empty sections have no line of their own and are skipped.
    pub fn section_at_line(&self, line: usize) -> Option<&ExportSection> {
        if self.in_tail(line) {
            return None;
        }
        self.sections.iter().filter(|s| s.start < s.end).take_while(|s| s.first_line <= line).last()
    }

    /// Round 23a (integration R31): first line (1-based) of the end of the book, that is of the first character behind
    /// the last section that is not whitespace. `None` without sections or without an end of the book. Before, messages
    /// from the end of the book (for example an error in an index line) counted as messages of the last section.
    pub fn tail_first_line(&self) -> Option<usize> {
        let end = self.sections.iter().map(|s| s.end).max()?;
        let rest = self.typst.get(end..)?;
        let offset = rest.find(|c: char| !c.is_whitespace())?;
        Some(baruch_typst::line_at(&self.typst, end + offset))
    }

    /// Does line `line` lie in the end of the book ([`ExportMap::tail_first_line`])?
    pub fn in_tail(&self, line: usize) -> bool {
        self.tail_first_line().is_some_and(|first| line >= first)
    }
}

impl Store {
    /// Export as Typst with a map of the sections. Unknown work → `UnknownWork`.
    pub async fn export_map(&self, work: WorkId) -> Result<ExportMap, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let (template, sections, tail) = snapshot::export_parts(&mut conn, work).await?;
        let (mut typst, ranges) = baruch_typst::join_with_offsets(&template, sections.iter().map(|s| s.body.as_str()));
        // End of the book (Round 31) behind the last section; the section ranges stay as they are.
        snapshot::append_tail(&mut typst, &tail);
        // Count lines continuously (otherwise quadratic in the size of the book): from the previous section boundary to
        // the next. Before every boundary except the first a text ends in `\n` (or is empty), so a `\r\n` is never cut
        // apart; the first one counts from byte 0.
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
