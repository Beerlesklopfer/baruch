// SPDX-License-Identifier: AGPL-3.0-or-later
//! Alte Teiltexte auf die Teil-Markierung umstellen (Nutzerfrage 8, entschieden 2026-11-07: „automatisch
//! umwandeln“).
//!
//! Vor Runde 14 trugen Teile (`kind = 'part'`) entweder die Zeile `= <Titel>` der alten Saat (z. B.
//! „= Teil I – Erkennen“) oder einen leeren Text. Seit Runde 14 ist die Überschrift eines Teils die
//! Markierung [`baruch_typst::part_line`] (`#heading(level: 1, numbering: none)[T] <baruch-part>`).
//!
//! **Startschritt statt Migration:** Die neue Zeile braucht die Maskierung von `baruch_typst`
//! (Rust, nicht SQL), jede Umwandlung ist eine neue Fassung mit Eintrag im Git-Spiegel (den kennt nur
//! der Server), und der Schritt soll gegen eine nachgebaute alte Datenbank testbar sein. Der Server
//! ruft [`crate::Store::convert_legacy_parts`] beim Start nach dem Nachführen der Spiegel auf; der
//! Merker `store_meta.parts_marked_r14` (gesetzt in derselben Transaktion) macht den Schritt einmalig,
//! und weil eine Markierung weder leer noch eine `=`-Zeile ist, wäre auch ein zweiter Lauf ohne
//! Wirkung (idempotent). Eine frisch gesäte Datenbank setzt den Merker gleich mit (`seed.rs`): Sie ist
//! nicht von vor Runde 14, ihr Beispielwerk behält seine leeren Teile wie bisher. Das Beispielwerk einer
//! älteren Datenbank (leere Teile seit Runde 5, „= Teil …“ davor) wird dagegen umgewandelt.
//!
//! Regeln je Teil, betrachtet wird nur die **aktuelle** Fassung:
//! - **leer** (nur Leerraum) → umwandeln;
//! - **ohne Fassung** → nichts zu tun: Sie stehen in keinem Text, Export und Spiegel kennen sie nicht;
//! - **alte Zeile:** der Text ist bis auf Leerraum genau eine `=`-Zeile (Ebene 1, ohne Label und
//!   Kommentar) und ihr Titel ist der Titel des Teils oder beginnt mit „Teil“ (ein Teil, der vor
//!   Runde 14 umbenannt wurde, behielt seine alte Zeile) → umwandeln;
//! - schon mit Markierung am Anfang → nichts zu tun;
//! - alles andere (selbst geschriebener Text) bleibt unangetastet und wird nur mit Knoten-id gemeldet
//!   ([`PartConversion::kept`]), nie mit Wortlaut.
//!
//! Umwandeln heißt: neue Fassung `heading_line(0, Titel des Teils, None) + "\n"` (wie ein neu angelegter
//! Teil), sofort aktuell, Elternteil die bisherige aktuelle, die jetzt `superseded` ist und erhalten
//! bleibt. Autor ist der Mensch (die Entscheidung ist seine), Notiz [`CONVERSION_NOTE`].

use baruch_core::Author;
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId, author_columns, broken, to_u32, to_usize};

/// Schlüssel in `store_meta`: Die Umwandlung ist gelaufen.
pub const CONVERTED_KEY: &str = "parts_marked_r14";

/// Herkunft der neuen Fassungen (Notiz der Revision, Trailer im Spiegel).
pub const CONVERSION_NOTE: &str = "Umwandlung Runde 14 (Nutzerentscheidung)";

/// Ein umgewandelter Teil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedPart {
    pub work: WorkId,
    /// Fester Schlüssel (`node.id`).
    pub key: i64,
    /// Stelle in der Gliederung (`node.seq`).
    pub seq: usize,
    pub title: String,
    /// Bisherige aktuelle Fassung (bleibt als `superseded` erhalten).
    pub from: u32,
    /// Neue aktuelle Fassung.
    pub to: u32,
    /// `true`: vorher leer; `false`: alte `=`-Zeile.
    pub was_empty: bool,
}

/// Ergebnis von [`Store::convert_legacy_parts`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartConversion {
    /// `false`: Der Merker war schon gesetzt, nichts geprüft.
    pub ran: bool,
    pub converted: Vec<ConvertedPart>,
    /// Knoten-ids (`node.id`) der Teile mit eigenem Text, die unverändert blieben.
    pub kept: Vec<i64>,
}

/// Was mit dem Text eines Teils geschieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartText {
    /// Leer bzw. nur Leerraum.
    Empty,
    /// Genau eine alte `=`-Zeile.
    Legacy,
    /// Beginnt schon mit der Teil-Markierung.
    Marked,
    /// Selbst geschriebener Text: bleibt.
    Own,
}

/// Einordnung eines Teiltexts (reine Funktion, Regeln siehe Modulbeschreibung).
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

/// Neuer Text eines Teils: Markierungszeile und Leerzeile (wie ein neu angelegter Teil).
pub fn marked_body(title: &str) -> String {
    format!("{}\n", baruch_typst::heading_line(0, title, None))
}

impl Store {
    /// Alte Teiltexte einmalig auf die Teil-Markierung umstellen (Modulbeschreibung von
    /// `legacy_parts`). Eine Transaktion: alle Umwandlungen und der Merker oder nichts. `human` ist der
    /// Name des Menschen, unter dem die neuen Fassungen stehen.
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
            // Gegenprobe: Die neue Zeile muss als Teil mit genau diesem Titel zurückkommen, sonst bleibt
            // der Text (z. B. ein Titel, den die Markierung nicht tragen kann).
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

/// Bisherige aktuelle Fassung `based_on` ablösen und die neue als aktuelle anhängen.
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
