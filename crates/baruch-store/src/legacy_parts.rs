// SPDX-License-Identifier: AGPL-3.0-or-later
//! Converting old part texts to the part marker (user question 8, decided 2026-11-07: „automatisch umwandeln“).
//!
//! Before Round 14 parts (`kind = 'part'`) carried either the line `= <title>` of the old seed (for example „= Teil I –
//! Erkennen“) or an empty text. Since Round 14 the heading of a part is the marker [`baruch_typst::part_line`]
//! (`#heading(level: 1, numbering: none)[T] <baruch-part>`).
//!
//! **Start step instead of a migration:** the new line needs the escaping of `baruch_typst` (Rust, not SQL), every
//! conversion is a new revision with an entry in the Git mirror (which only the server knows), and the step is meant to
//! be testable against a rebuilt old database. The server calls [`crate::Store::convert_legacy_parts`] at the start
//! after catching up the mirrors; the flag `store_meta.parts_marked_r14` (set in the same transaction) makes the step a
//! one-time one, and because a marker is neither empty nor a `=` line, a second run would have no effect either
//! (idempotent). A freshly seeded database sets the flag right away (`seed.rs`): it does not date from before Round 14,
//! its sample work keeps its empty parts as before. The sample work of an older database (empty parts since Round 5, „=
//! Teil …“ before that) is converted, however.
//!
//! Rules per part, only the **current** revision is considered:
//! - **empty** (whitespace only) → convert;
//! - **without a revision** → nothing to do: they appear in no text, export and mirror do not know them;
//! - **old line:** the text is, apart from whitespace, exactly one `=` line (level 1, without label and comment) and
//!   its title is the title of the part or begins with „Teil“ (a part that was renamed before Round 14 kept its old
//!   line) → convert;
//! - already with the marker at the beginning → nothing to do;
//! - everything else (text written by the author) stays untouched and is only reported with its node id
//!   ([`PartConversion::kept`]), never with its wording.
//!
//! Converting means: new revision `heading_line(0, title of the part, None) + "\n"` (like a newly created part),
//! current at once, parent the previous current one, which is now `superseded` and is kept. The author is the human
//! (the decision is theirs), note [`CONVERSION_NOTE`].

use baruch_core::Author;
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId, author_columns, broken, to_u32, to_usize};

/// Key in `store_meta`: the conversion has run.
pub const CONVERTED_KEY: &str = "parts_marked_r14";

/// Origin of the new revisions (note of the revision, trailer in the mirror).
pub const CONVERSION_NOTE: &str = "Umwandlung Runde 14 (Nutzerentscheidung)";

/// A converted part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedPart {
    pub work: WorkId,
    /// Fixed key (`node.id`).
    pub key: i64,
    /// Position in the outline (`node.seq`).
    pub seq: usize,
    pub title: String,
    /// Previous current revision (kept as `superseded`).
    pub from: u32,
    /// New current revision.
    pub to: u32,
    /// `true`: empty before; `false`: old `=` line.
    pub was_empty: bool,
}

/// Result of [`Store::convert_legacy_parts`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartConversion {
    /// `false`: the flag was already set, nothing checked.
    pub ran: bool,
    pub converted: Vec<ConvertedPart>,
    /// Node ids (`node.id`) of the parts with text of their own that stayed unchanged.
    pub kept: Vec<i64>,
}

/// What happens to the text of a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartText {
    /// Empty or whitespace only.
    Empty,
    /// Exactly one old `=` line.
    Legacy,
    /// Already begins with the part marker.
    Marked,
    /// Text written by the author: stays.
    Own,
}

/// Classification of a part text (pure function, for the rules see the module description).
pub fn classify(body: &str, title: &str) -> PartText {
    let body = body.replace("\r\n", "\n");
    if body.trim().is_empty() {
        return PartText::Empty;
    }
    let Ok(doc) = baruch_typst::parse(&body) else {
        return PartText::Own;
    };
    let first = doc.headings.first();
    if doc.template.trim().is_empty() && first.is_some_and(|h| h.level == 0) {
        return PartText::Marked;
    }
    match (doc.template.trim().is_empty(), doc.headings.as_slice()) {
        (true, [only]) if only.level == 1 && body.trim() == format!("= {}", only.raw_title) => {
            if only.title == title || only.title.starts_with("Teil") { PartText::Legacy } else { PartText::Own }
        }
        _ => PartText::Own,
    }
}

/// New text of a part: marker line and empty line (like a newly created part).
pub fn marked_body(title: &str) -> String {
    format!("{}\n", baruch_typst::heading_line(0, title, None))
}

impl Store {
    /// Convert old part texts to the part marker once (module description of `legacy_parts`). One transaction: all
    /// conversions and the flag, or nothing. `human` is the name of the human under which the new revisions stand.
    pub async fn convert_legacy_parts(&self, human: &str) -> Result<PartConversion, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(CONVERTED_KEY)
            .fetch_optional(&mut *tx)
            .await?;
        if done.is_some() {
            return Ok(PartConversion::default());
        }
        let mut result = PartConversion { ran: true, ..PartConversion::default() };
        let parts: Vec<(i64, WorkId, i64, String)> =
            sqlx::query_as("SELECT id, work_id, seq, title FROM node WHERE kind = 'part' ORDER BY work_id, seq")
                .fetch_all(&mut *tx)
                .await?;
        let author = Author::Human { name: human.to_owned() };
        for (key, work, seq, title) in parts {
            let current: Option<(i64, String)> =
                sqlx::query_as("SELECT number, body FROM revision WHERE node_id = ? AND status = 'current'")
                    .bind(key)
                    .fetch_optional(&mut *tx)
                    .await?;
            let Some((from, old)) = current else { continue };
            let kind = classify(&old, &title);
            match kind {
                PartText::Marked => continue,
                PartText::Own => {
                    result.kept.push(key);
                    continue;
                }
                PartText::Empty | PartText::Legacy => {}
            }
            let body = marked_body(&title);
            // Cross-check: the new line must come back as a part with exactly this title, otherwise the text stays (for
            // example a title that the marker cannot carry).
            let back = baruch_typst::parse(&body).ok();
            let round_trip =
                back.as_ref().and_then(|d| d.headings.first()).is_some_and(|h| h.level == 0 && h.title == title);
            if !round_trip {
                result.kept.push(key);
                continue;
            }
            let from = to_u32(from, "number")?;
            let to = append_current(&mut tx, key, &body, &author, from).await?;
            result.converted.push(ConvertedPart {
                work,
                key,
                seq: to_usize(seq, "seq")?,
                title,
                from,
                to,
                was_empty: kind == PartText::Empty,
            });
        }
        sqlx::query("INSERT INTO store_meta (key, value) VALUES (?, '1')")
            .bind(CONVERTED_KEY)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
}

/// Supersede the previous current revision `based_on` and append the new one as the current one.
async fn append_current(
    conn: &mut SqliteConnection,
    node_id: i64,
    body: &str,
    author: &Author,
    based_on: u32,
) -> Result<u32, StoreError> {
    let done = sqlx::query("UPDATE revision SET status = 'superseded' WHERE node_id = ? AND status = 'current'")
        .bind(node_id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() != 1 {
        return Err(broken(format!(
            "Teil {node_id}: {} aktuelle Fassungen abgelöst statt einer",
            done.rows_affected()
        )));
    }
    let last: i64 =
        sqlx::query_scalar("SELECT last_rev FROM node WHERE id = ?").bind(node_id).fetch_one(&mut *conn).await?;
    let number = to_u32(last, "last_rev")? + 1;
    let (author_kind, author_name, model) = author_columns(author);
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on) \
         VALUES (?, ?, ?, ?, ?, ?, 'current', ?, ?)",
    )
    .bind(node_id)
    .bind(i64::from(number))
    .bind(body)
    .bind(author_kind)
    .bind(author_name)
    .bind(model)
    .bind(CONVERSION_NOTE)
    .bind(i64::from(based_on))
    .execute(&mut *conn)
    .await?;
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_texts_are_classified_by_the_rules_of_the_decision() {
        let title = "Teil I – Am Ufer";
        let cases: &[(&str, PartText, &str)] = &[
            ("", PartText::Empty, "leer"),
            ("  \n\n", PartText::Empty, "nur Leerraum"),
            ("= Teil I – Am Ufer\n", PartText::Legacy, "alte Saat"),
            ("= Teil I – Am Ufer", PartText::Legacy, "alte Saat ohne Zeilenende"),
            ("\r\n= Teil I – Am Ufer\r\n\r\n", PartText::Legacy, "alte Saat mit CRLF und Leerzeilen"),
            ("= Teil I – Früherer Name\n", PartText::Legacy, "vor Runde 14 umbenannt, Zeile beginnt mit „Teil“"),
            ("= Am Ufer\n", PartText::Own, "anderer Titel ohne „Teil“: kein Saat-Text"),
            ("= Teil I – Am Ufer <teil-eins>\n", PartText::Own, "mit eigenem Label"),
            ("= Teil I – Am Ufer // Notiz\n", PartText::Own, "mit Kommentar"),
            ("== Teil I – Am Ufer\n", PartText::Own, "andere Ebene"),
            ("= Teil I – Am Ufer\n\nEin Satz zur Einführung.\n", PartText::Own, "mit eigenem Text"),
            ("Nur ein Satz.\n", PartText::Own, "Text ohne Überschrift"),
            ("#heading(level: 1, numbering: none)[Teil I – Am Ufer] <baruch-part>\n\n", PartText::Marked, "neu"),
        ];
        for (body, want, why) in cases {
            assert_eq!(classify(body, title), *want, "{why}: {body:?}");
        }
    }

    #[test]
    fn the_new_body_is_the_part_marker_with_a_blank_line() {
        assert_eq!(
            marked_body("Teil II – [Brücken]"),
            "#heading(level: 1, numbering: none)[Teil II – \\[Brücken\\]] <baruch-part>\n\n",
            "Markierung wie ein neu angelegter Teil, Klammern maskiert"
        );
        assert_eq!(classify(&marked_body("Teil II"), "Teil II"), PartText::Marked, "zweiter Lauf: nichts zu tun");
    }
}
