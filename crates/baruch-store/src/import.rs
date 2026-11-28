// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst import (contract B1.5, section 3): a file becomes a new work.
//!
//! Runs inside the transaction that [`crate::Store::import_typst_titled`] opens with `BEGIN IMMEDIATE`. Preamble →
//! `work.template` (and revision 1 of the work template); `=` → chapter, `==`/`===` → section (subsection = section
//! below a section); text per heading → revision 1 by the human, current at once. The title must be free (contract
//! Round 4, `title_key`).
//!
//! Round 14 (contract section 2): the part marker (`Heading.level` 0) becomes a part (`kind = 'part'`, top level, the
//! following chapters below it), the marker "without number" sets `node.numbered = 0`. The numbers (`node.number`) are
//! computed by [`crate::load::numbers`] with the numbering of the work template, as when loading.
//!
//! Round 30a (contract §3.3): jump labels of an export (`= Titel <k5>`, anchor line `#metadata("baruch-key") <k5>`) are
//! removed by [`baruch_typst::xref::strip_section`] per heading; the old keys yield a mapping old → new (`node.id`),
//! and [`baruch_typst::xref::remap_keys`] converts the references `#link(<kN>)[…]` in all texts. The checksum against a
//! duplicate import stays that of the file as it came.

use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;

use crate::works::{insert_template_revision, title_free, title_key};
use crate::{Imported, StoreError, WorkId, load, slug, words};

/// SHA-256 as lower-case hex.
pub(crate) fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Free folder name: `base`, otherwise `base-2`, `base-3` … Free means: no living work and no deleted one
/// (`work_retired`, migration 0007) carries it. Slugs of deleted works stay reserved permanently, because their mirror
/// lies under `works/.geloescht/` and the database does not know the archive folder (decision Round 5).
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

/// Title from the file name: without folder and extension; empty → the whole name, then „Werk“.
fn title_from_file(file_name: &str) -> String {
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).map(str::trim).unwrap_or("");
    if !stem.is_empty() {
        return stem.to_owned();
    }
    let name = file_name.trim();
    if name.is_empty() { "Werk".to_owned() } else { name.to_owned() }
}

/// Start of the last line of `text` that reads exactly `line`.
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

/// Marker line of the show rules of the fallback "without number" (`snapshot::fallback_rules`).
pub(crate) const FALLBACK_MARK: &str = "// Baruch: Überschriften ohne Nummer";

/// Rule block „// Baruch: Satz“ at the end of the template of an export → typesetting settings (Round 21b). Only if the
/// block reaches to the end, can be read back exactly ([`baruch_typst::typeset::settings_from_rules`]) and is valid;
/// otherwise the template stays as it is.
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

/// Show rules of the fallback at the end of the template of an export (`#show heading.where(level: N, body: [Titel]):
/// set heading(numbering: none)` under [`FALLBACK_MARK`]) → (level, title verbatim) of the headings without a number
/// (Round 21b; counterpart of `snapshot::fallback_rules`). A line of another form → template unchanged, no markers.
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
    // Round 21b: rule blocks that only an export writes behind the work template come back into settings or markers
    // instead of into the work template – „// Baruch: Satz“ into the typesetting settings, „// Baruch: Überschriften
    // ohne Nummer“ (show rules of the fallback) into `node.numbered = 0`. A block that cannot be read back exactly like
    // that stays in the template (the server reports it).
    let (template, typeset) = split_typeset(&doc.template);
    let (template, unnumbered) = split_fallback(&template);
    // Round 22: show rules through the jump label `<kN>` (title with code on a numbered node of the same wording) come
    // out of the template in the same way; the old keys name the headings "without number".
    let (template, by_label) = crate::snapshot::split_label_fallback(&template);
    doc.template = template;
    // Round 30a: remove jump labels from an export (`<kN>` on the heading, anchor line below it); remember the old keys
    // per heading in order to convert references to the new ones afterwards.
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

    // Custom title (empty counts like none), otherwise from #set document, otherwise from the file name.
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

    // Chain of ancestors: (level, seq, number of children so far). The parent is the nearest previous heading with a
    // smaller level (a part has level 0, so chapters stand below it).
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
    // An imported work is German (work language default); the template scheme reads rules of both languages.
    let numbering = load::work_numbering("template", None, &doc.template, crate::WorkLanguage::De);
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
        // The trigger `node_numbered_default` (migration 0006) sets `numbered = 0` for an empty number – with schemes
        // that display levels without a number (sermon: sections "") that would be wrong and the export would project
        // them as "without number". The text form alone is decisive (contract Round 14, §2/§4).
        if number.trim().is_empty() {
            sqlx::query("UPDATE node SET numbered = ? WHERE id = ?")
                .bind(i64::from(numbered[seq]))
                .bind(node_id)
                .execute(&mut *conn)
                .await?;
        }
        node_ids.push(node_id);
    }
    // Convert references `#link(<k-old>)` to the new keys (`node.id`) (Round 30a); keys without a jump label in this
    // file stay as they are (the print check reports them as broken).
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
