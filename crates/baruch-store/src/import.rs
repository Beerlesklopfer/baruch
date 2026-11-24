// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Import (Vertrag B1.5, Abschnitt 3): eine Datei wird ein neues Werk.
//!
//! Läuft innerhalb der Transaktion, die [`crate::Store::import_typst_titled`] mit `BEGIN IMMEDIATE`
//! öffnet. Präambel → `work.template` (und Fassung 1 der Werkvorlage); `=` → Kapitel, `==`/`===` →
//! Abschnitt (Unterabschnitt = Abschnitt unter Abschnitt); Text je Überschrift → Revision 1 vom
//! Menschen, sofort aktuell. Der Titel muss frei sein (Vertrag Runde 4, `title_key`).
//!
//! Runde 14 (Vertrag Abschnitt 2): Die Teil-Markierung (`Heading.level` 0) wird ein Teil
//! (`kind = 'part'`, oberste Ebene, die folgenden Kapitel darunter), die Markierung „ohne Nummer“
//! setzt `node.numbered = 0`. Die Nummern (`node.number`) rechnet [`crate::load::numbers`] mit der
//! Nummerierung der Werkvorlage, wie beim Laden.
//!
//! Runde 30a (Vertrag §3.3): Sprungmarken eines Exports (`= Titel <k5>`, Ankerzeile `#metadata("baruch-key") <k5>`)
//! entfernt [`baruch_typst::xref::strip_section`] je Überschrift; die alten Schlüssel ergeben eine Zuordnung alt → neu
//! (`node.id`), und [`baruch_typst::xref::remap_keys`] setzt die Verweise `#link(<kN>)[…]` in allen Texten um. Die
//! Prüfsumme gegen doppelten Import bleibt die der Datei, wie sie kam.

use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;

use crate::works::{insert_template_revision, title_free, title_key};
use crate::{Imported, StoreError, WorkId, load, slug, words};

/// SHA-256 als Kleinbuchstaben-Hex.
pub(crate) fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Freier Ordnername: `base`, sonst `base-2`, `base-3` … Frei heißt: kein lebendes Werk und kein
/// gelöschtes (`work_retired`, Migration 0007) trägt ihn. slugs gelöschter Werke bleiben dauerhaft
/// reserviert, weil ihr Spiegel unter `works/.geloescht/` liegt und die Datenbank den Archivordner
/// nicht kennt (Entscheidung Runde 5).
pub(crate) async fn free_slug(conn: &mut SqliteConnection, base: &str) -> Result<String, StoreError> {
    let mut candidate = base.to_owned();
    let mut n = 2;
    loop {
        let taken: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM work WHERE slug = ?1 UNION ALL SELECT 1 FROM work_retired WHERE slug = ?1 LIMIT 1",
        )
        .bind(&candidate)
        .fetch_optional(&mut *conn)
        .await?;
        if taken.is_none() {
            return Ok(candidate);
        }
        candidate = format!("{base}-{n}");
        n += 1;
    }
}

/// Titel aus dem Dateinamen: ohne Ordner und Endung; leer → der ganze Name, dann „Werk“.
fn title_from_file(file_name: &str) -> String {
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).map(str::trim).unwrap_or("");
    if !stem.is_empty() {
        return stem.to_owned();
    }
    let name = file_name.trim();
    if name.is_empty() { "Werk".to_owned() } else { name.to_owned() }
}

/// Anfang der letzten Zeile von `text`, die genau `line` lautet.
fn last_line_start(text: &str, line: &str) -> Option<usize> {
    let mut found = None;
    let mut start = 0;
    for l in text.split_inclusive('\n') {
        if l.trim_end_matches(['\n', '\r']) == line {
            found = Some(start);
        }
        start += l.len();
    }
    found
}

/// Kennzeile der Show-Regeln des Rückfalls „ohne Nummer“ (`snapshot::fallback_rules`).
pub(crate) const FALLBACK_MARK: &str = "// Baruch: Überschriften ohne Nummer";

/// Regelblock „// Baruch: Satz“ am Ende der Vorlage eines Exports → Satz-Einstellungen (Runde 21b). Nur wenn der Block
/// bis zum Ende reicht, sich genau zurücklesen lässt ([`baruch_typst::typeset::settings_from_rules`]) und gültig ist;
/// sonst bleibt die Vorlage, wie sie ist.
pub(crate) fn split_typeset(template: &str) -> (String, Option<baruch_typst::typeset::TypesetSettings>) {
    let Some(at) = last_line_start(template, baruch_typst::typeset::RULES_MARK) else {
        return (template.to_owned(), None);
    };
    let block = format!("{}\n", template[at..].trim_end());
    match baruch_typst::typeset::settings_from_rules(&block).and_then(|s| crate::typeset::checked(&s).ok()) {
        Some(settings) if !settings.is_empty() => (template[..at].to_owned(), Some(settings)),
        _ => (template.to_owned(), None),
    }
}

/// Show-Regeln des Rückfalls am Ende der Vorlage eines Exports (`#show heading.where(level: N, body: [Titel]): set
/// heading(numbering: none)` unter [`FALLBACK_MARK`]) → (Ebene, Titel wörtlich) der Überschriften ohne Nummer
/// (Runde 21b; Gegenstück zu `snapshot::fallback_rules`). Eine Zeile anderer Form → Vorlage unverändert, keine Merker.
pub(crate) fn split_fallback(template: &str) -> (String, Vec<(u8, String)>) {
    let Some(at) = last_line_start(template, FALLBACK_MARK) else { return (template.to_owned(), Vec::new()) };
    let mut keys = Vec::new();
    for line in template[at..].lines().skip(1).filter(|l| !l.trim().is_empty()) {
        let parsed = line
            .strip_prefix("#show heading.where(level: ")
            .and_then(|rest| rest.strip_suffix("]): set heading(numbering: none)"))
            .and_then(|rest| rest.split_once(", body: ["))
            .and_then(|(level, title)| Some((level.parse::<u8>().ok().filter(|l| (1..=3).contains(l))?, title)));
        match parsed {
            Some((level, title)) => keys.push((level, title.to_owned())),
            None => return (template.to_owned(), Vec::new()),
        }
    }
    if keys.is_empty() {
        return (template.to_owned(), Vec::new());
    }
    (template[..at].to_owned(), keys)
}

pub(crate) async fn import_typst(
    conn: &mut SqliteConnection,
    source: &str,
    file_name: &str,
    human: &str,
    note: &str,
    title: Option<&str>,
) -> Result<Imported, StoreError> {
    let source = source.replace("\r\n", "\n");
    let hash = sha256_hex(&source);
    let known: Option<(WorkId, String, String)> =
        sqlx::query_as("SELECT id, slug, title FROM work WHERE source_sha256 = ?")
            .bind(&hash)
            .fetch_optional(&mut *conn)
            .await?;
    if let Some((work, slug, title)) = known {
        return Err(StoreError::AlreadyImported { work, slug, title });
    }
    let mut doc = baruch_typst::parse(&source).map_err(|e| StoreError::Import(e.to_string()))?;
    // Runde 21b: Regelblöcke, die nur ein Export hinter die Werkvorlage schreibt, kommen zurück in Einstellungen bzw.
    // Merker statt in die Werkvorlage – „// Baruch: Satz“ in die Satz-Einstellungen, „// Baruch: Überschriften ohne
    // Nummer“ (Show-Regeln des Rückfalls) in `node.numbered = 0`. Ein Block, der sich nicht genau so zurücklesen lässt,
    // bleibt in der Vorlage (der Server meldet das).
    let (template, typeset) = split_typeset(&doc.template);
    let (template, unnumbered) = split_fallback(&template);
    // Runde 22: Show-Regeln über die Sprungmarke `<kN>` (Titel mit Code an einem gleichlautenden nummerierten Knoten)
    // kommen ebenso aus der Vorlage heraus; die alten Schlüssel nennen die Überschriften „ohne Nummer“.
    let (template, by_label) = crate::snapshot::split_label_fallback(&template);
    doc.template = template;
    // Runde 30a: Sprungmarken aus einem Export (`<kN>` an der Überschrift, Ankerzeile darunter) entfernen; die alten
    // Schlüssel je Überschrift merken, um Verweise danach auf die neuen umzusetzen.
    let old_keys: Vec<Option<i64>> = doc
        .headings
        .iter_mut()
        .map(|heading| {
            let (stripped, key) = baruch_typst::xref::strip_section(&heading.body);
            if let std::borrow::Cow::Owned(stripped) = stripped {
                heading.body = stripped;
            }
            key
        })
        .collect();
    let numbered: Vec<bool> = doc
        .headings
        .iter()
        .zip(&old_keys)
        .map(|(h, key)| {
            h.numbered
                && !unnumbered.iter().any(|(level, title)| *level == h.level && *title == h.raw_title)
                && !key.is_some_and(|k| by_label.contains(&k))
        })
        .collect();

    // Eigener Titel (leer zählt wie keiner), sonst aus #set document, sonst aus dem Dateinamen.
    let title = match title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(title) => title.to_owned(),
        None => doc.title.clone().unwrap_or_else(|| title_from_file(file_name)),
    };
    let key = title_key(&title);
    title_free(conn, &key, None).await?;
    let author = doc.author.clone().unwrap_or_default();
    let slug = free_slug(conn, &slug(&title)).await?;
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&slug)
    .bind(words::work_kind(baruch_core::WorkKind::Book))
    .bind(&title)
    .bind(&author)
    .bind(&doc.template)
    .bind(file_name)
    .bind(&hash)
    .bind(&key)
    .fetch_one(&mut *conn)
    .await?;
    insert_template_revision(conn, work, 1, &doc.template, human, note).await?;
    if let Some(settings) = typeset {
        crate::typeset::write(conn, work, baruch_typst::typeset::TypesetSettings::default(), settings, human).await?;
    }

    // Ahnenkette: (Ebene, seq, Anzahl der Kinder bisher). Elternteil ist die nächste vorige
    // Überschrift mit kleinerer Ebene (ein Teil hat Ebene 0, Kapitel stehen also unter ihm).
    let mut stack: Vec<(u8, usize, usize)> = Vec::new();
    let mut roots = 0usize;
    let mut placed: Vec<(Option<usize>, usize, baruch_core::NodeKind)> = Vec::with_capacity(doc.headings.len());
    for (seq, heading) in doc.headings.iter().enumerate() {
        while stack.last().is_some_and(|&(level, ..)| level >= heading.level) {
            stack.pop();
        }
        let (parent, position) = match stack.last_mut() {
            Some((_, parent, children)) => {
                *children += 1;
                (Some(*parent), *children - 1)
            }
            None => {
                roots += 1;
                (None, roots - 1)
            }
        };
        let kind = match heading.level {
            0 => baruch_core::NodeKind::Part,
            1 => baruch_core::NodeKind::Chapter,
            _ => baruch_core::NodeKind::Section,
        };
        placed.push((parent, position, kind));
        stack.push((heading.level, seq, 0));
    }
    let counted: Vec<load::Numbered> = numbered
        .iter()
        .zip(&placed)
        .map(|(&numbered, &(parent, _, kind))| load::Numbered { kind, parent, numbered })
        .collect();
    let numbering = load::work_numbering("template", None, &doc.template);
    let numbers = load::numbers(&numbering, &counted);
    let mut node_ids: Vec<i64> = Vec::with_capacity(doc.headings.len());
    for (seq, ((heading, &(parent, position, kind)), number)) in
        doc.headings.iter().zip(&placed).zip(numbers).enumerate()
    {
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind, numbered) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent.map(|p| p as i64))
        .bind(position as i64)
        .bind(&number)
        .bind(&heading.title)
        .bind(words::node_kind(kind))
        .bind(i64::from(numbered[seq]))
        .fetch_one(&mut *conn)
        .await?;
        // Der Trigger `node_numbered_default` (Migration 0006) setzt bei leerer Nummer `numbered = 0` – bei
        // Schemata, die Ebenen ohne Nummer anzeigen (Predigt: Abschnitte „“), wäre das falsch und der Export
        // projizierte sie als „ohne Nummer“. Maßgeblich ist allein die Textform (Vertrag Runde 14, §2/§4).
        if number.trim().is_empty() {
            sqlx::query("UPDATE node SET numbered = ? WHERE id = ?")
                .bind(i64::from(numbered[seq]))
                .bind(node_id)
                .execute(&mut *conn)
                .await?;
        }
        node_ids.push(node_id);
    }
    // Verweise `#link(<k-alt>)` auf die neuen Schlüssel (`node.id`) umsetzen (Runde 30a); Schlüssel ohne Sprungmarke
    // in dieser Datei bleiben stehen (die Druckprüfung meldet sie als kaputt).
    let map: std::collections::HashMap<i64, i64> =
        old_keys.iter().zip(&node_ids).filter_map(|(old, new)| old.map(|old| (old, *new))).collect();
    for (heading, node_id) in doc.headings.iter().zip(&node_ids) {
        let body = baruch_typst::xref::remap_keys(&heading.body, &map);
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
             VALUES (?, 1, ?, 'human', ?, NULL, 'current', ?)",
        )
        .bind(node_id)
        .bind(body.as_ref())
        .bind(human)
        .bind(note)
        .execute(&mut *conn)
        .await?;
    }

    Ok(Imported { work, slug, title, author, nodes: doc.headings.len() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_falls_back_to_the_file_name_without_extension() {
        assert_eq!(title_from_file("Mein Buch.typ"), "Mein Buch");
        assert_eq!(title_from_file("ordner/buch.typ"), "buch", "Ordner fällt weg");
        assert_eq!(title_from_file(""), "Werk", "leerer Name");
    }

    #[test]
    fn sha256_is_lowercase_hex() {
        assert_eq!(sha256_hex(""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }
}
