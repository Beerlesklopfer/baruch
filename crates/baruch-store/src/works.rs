// SPDX-License-Identifier: AGPL-3.0-or-later
//! Managing works by the human (contract Round 4, sections 1 and 3b): unique titles, creating, metadata, revisions of
//! the work template, deleting whole works.
//!
//! - **Unique titles:** two works must not have the same [`title_key`] (partial index `work_title_key` from migration
//!   `0005_works.sql`). Creating, importing and changing with a title already taken → [`StoreError::DuplicateTitle`],
//!   nothing changed. Existing duplicates from the time before 0005 are resolved by [`ensure_title_keys`] when opening,
//!   only through the title (slug, mirror, outline and texts stay).
//! - **Work template:** `work.template` is the current template, every change a new revision in
//!   `work_template_revision` (numbers from 1, append only). Snapshot and export keep reading `work.template`.
//! - **Deleting** ([`Store::delete_work`]) only with the title typed in and in one transaction: the trigger
//!   `revision_no_delete_current` (0004) protects the current revision of living sections; therefore the deletion first
//!   sets all current revisions of the work to `superseded` (allowed according to `revision_status_transition`) and
//!   afterwards deletes revisions, references, posts, nodes, template revisions and the work. id, slug and title move
//!   to `work_retired` (migration 0007).
//! - **Never reuse ids and slugs** (contract Round 5): `work.id` has been `AUTOINCREMENT` since 0007, a new work never
//!   gets the id of an old one after a deletion; new folder names also avoid the slugs of deleted works
//!   (`import::free_slug`).

use std::collections::{BTreeMap, HashMap, HashSet};

use baruch_core::WorkKind;
use sqlx::{SqliteConnection, SqlitePool};

use crate::{Imported, Store, StoreError, WorkId, WorkSummary, import, slug, to_u32, to_usize, words};

/// Comparison key of a title: trimmed, runs of whitespace (Unicode) → one space, lower-case letters (Unicode). „ Das
/// Buch “ and „das buch“ yield the same key.
pub fn title_key(title: &str) -> String {
    one_line(title).to_lowercase()
}

/// A detail on one line: runs of whitespace (also line breaks) → one space, trimmed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Skeleton of a new work ([`Store::create_work`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Work template with `#set document` and `#set heading(numbering: "1.1")`, one chapter „Erstes Kapitel“.
    #[default]
    Book,
    /// Work template with `#set document`, outline scheme `sermon` („Einleitung · I. · II. · III. · Schluss“) and the
    /// items from [`SERMON_OUTLINE`]: sermon text and application without a number, between them introduction, three
    /// main points and conclusion (Round 16a).
    Sermon,
}

/// Details for [`Store::create_work`]. Title and details are trimmed, line breaks become spaces.
#[derive(Debug, Clone, Copy)]
pub struct NewWork<'a> {
    pub title: &'a str,
    pub author: &'a str,
    pub kind: WorkKind,
    pub preset: Preset,
    pub subtitle: &'a str,
    /// Target audience.
    pub audience: &'a str,
    /// Bible translation.
    pub bible: &'a str,
    /// Work language (Round 39c): written to `work.language` (migration 0055); the titles of the preset chapters
    /// (`book.preset.*` of the book catalog) are taken in this language. German reproduces the former literals.
    pub language: baruch_core::WorkLanguage,
}

/// What [`Store::update_work`] is to change; `None` = stays.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkEdit<'a> {
    pub title: Option<&'a str>,
    pub subtitle: Option<&'a str>,
    pub author: Option<&'a str>,
    pub kind: Option<WorkKind>,
    pub audience: Option<&'a str>,
    pub bible: Option<&'a str>,
    /// New work template (CRLF → LF); only a different text yields a new revision.
    pub template: Option<&'a str>,
    /// Note on the new revision of the work template.
    pub note: Option<&'a str>,
}

/// All details of a work ([`Store::work_meta`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkMeta {
    pub id: WorkId,
    /// Globally unique identifier (migration 0041, exchange format); read only.
    pub uuid: String,
    pub slug: String,
    pub kind: WorkKind,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    /// Target audience.
    pub audience: String,
    /// Bible translation.
    pub bible: String,
    /// Current work template.
    pub template: String,
    /// Number of the current revision of the work template (0 = no revision stored).
    pub template_rev: u32,
    /// File name at the import; `None` for seeded and created works.
    pub source_name: Option<String>,
    pub created: String,
    /// Last change of metadata or work template; `None` = never changed.
    pub updated: Option<String>,
    /// Work language (migration 0055): personas answer in it.
    pub language: crate::WorkLanguage,
}

/// A revision of the work template (without text, [`Store::template_revisions`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TemplateRevision {
    pub number: u32,
    pub author: String,
    pub note: String,
    pub created: String,
}

/// Result of [`Store::update_work`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkUpdate {
    pub work: WorkId,
    /// Changed fields: `title`, `subtitle`, `author`, `kind`, `audience`, `bible`, `template`.
    pub changed: Vec<&'static str>,
    /// Number of the new revision if the work template has changed.
    pub template_rev: Option<u32>,
}

/// Result of [`Store::delete_work`]: what was deleted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeletedWork {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    pub nodes: usize,
    pub revisions: usize,
}

/// A deleted (archived) work from `work_retired` (Round 22, archive).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RetiredWork {
    pub id: WorkId,
    pub slug: String,
    pub title: String,
    pub deleted: String,
}

/// Result of [`Store::restore_retired`]: the restored work.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RestoredWork {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    pub nodes: usize,
}

/// Note of the revisions and of the work template of a work restored from the archive (Round 22).
pub const RESTORED_NOTE: &str = "aus dem Archiv wiederhergestellt";

/// Note of the first revisions of a created work.
const CREATED_NOTE: &str = "Werk angelegt";

/// Title of the item that carries the sermon text (without a number; [`Store::create_work_with_text`]).
pub const SERMON_TEXT_TITLE: &str = "Predigttext";

/// Outline of a new sermon (Round 16a): title and whether the item is counted. The scheme `sermon` calls the first
/// counted item „Einleitung“, the last one „Schluss“ and the others „I.“, „II.“, „III.“. Therefore the counted items
/// carry titles that do not repeat the number (otherwise the outline would read „Einleitung Einleitung“ or „I. Erster
/// Hauptpunkt“); sermon text and application have no number and are called what they are. Reasoning in
/// `docs/Architektur.md`, „Runde 16a“.
pub const SERMON_OUTLINE: &[(&str, bool)] = &[
    (SERMON_TEXT_TITLE, false),
    ("Hinführung", true),
    ("Beobachtung am Text", true),
    ("Deutung", true),
    ("Zuspruch", true),
    ("Anwendung", false),
    ("Ausblick", true),
];

/// Title of the item that carries the sermon text in the language of the work (Round 39c). German is
/// [`SERMON_TEXT_TITLE`].
pub fn sermon_text_title_in(lang: baruch_core::WorkLanguage) -> String {
    baruch_core::book_text::book_t(lang, "book.preset.sermon_text", &[])
}

/// Outline of a new sermon in the language of the work (Round 39c): like [`SERMON_OUTLINE`] with the titles from
/// the book catalog (`book.preset.*`). German is [`SERMON_OUTLINE`], word for word.
pub fn sermon_outline_in(lang: baruch_core::WorkLanguage) -> Vec<(String, bool)> {
    let t = |key: &str| baruch_core::book_text::book_t(lang, key, &[]);
    vec![
        (sermon_text_title_in(lang), false),
        (t("book.preset.sermon_introduction"), true),
        (t("book.preset.sermon_observation"), true),
        (t("book.preset.sermon_interpretation"), true),
        (t("book.preset.sermon_assurance"), true),
        (t("book.preset.sermon_application"), false),
        (t("book.preset.sermon_outlook"), true),
    ]
}

/// Chapters or sermon parts of a new work: title and "with number".
fn preset_chapters(preset: Preset, lang: baruch_core::WorkLanguage) -> Vec<(String, bool)> {
    match preset {
        Preset::Book => vec![(baruch_core::book_text::book_t(lang, "book.preset.first_chapter", &[]), true)],
        Preset::Sermon => sermon_outline_in(lang),
    }
}

/// Outline scheme of a new work (`work.numbering_scheme`) and the number per item after this scheme.
fn preset_numbers(preset: Preset, chapters: &[(String, bool)]) -> (&'static str, Vec<String>) {
    match preset {
        // As before: scheme from the work template, provisional number 1, 2, … (the template counts "1.1").
        Preset::Book => ("template", (1..=chapters.len()).map(|n| n.to_string()).collect()),
        Preset::Sermon => {
            let scheme = baruch_typst::scheme::Scheme::Sermon;
            let numbering = baruch_typst::scheme::Numbering::of_scheme(scheme, None, "")
                .expect("das Predigtschema braucht kein Muster");
            let counted = chapters.iter().filter(|(_, numbered)| *numbered).count();
            let mut own = 0;
            let numbers = chapters
                .iter()
                .map(|(_, numbered)| {
                    if *numbered {
                        own += 1;
                        numbering.heading(&[own], counted)
                    } else {
                        String::new()
                    }
                })
                .collect();
            (scheme.id(), numbers)
        }
    }
}

/// Typst string without quotation marks: `\` and `"` escaped.
fn typst_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Work template of a new work.
fn preset_template(preset: Preset, title: &str, author: &str) -> String {
    let document = format!("#set document(title: \"{}\", author: \"{}\")\n", typst_string(title), typst_string(author));
    match preset {
        // Round 27: imprint template (front matter functions schmutztitel, haupttitel, impressum, widmung) appended.
        Preset::Book => {
            format!("{document}#set heading(numbering: \"1.1\")\n\n{}", baruch_typst::cover::imprint_template())
        }
        Preset::Sermon => format!("{document}\n"),
    }
}

/// If the key is already assigned to a work other than `except` → `DuplicateTitle`.
pub(crate) async fn title_free(
    conn: &mut SqliteConnection,
    key: &str,
    except: Option<WorkId>,
) -> Result<(), StoreError> {
    let taken: Option<(WorkId, String, String)> =
        sqlx::query_as("SELECT id, slug, title FROM work WHERE title_key = ? AND id IS NOT ? ORDER BY id LIMIT 1")
            .bind(key)
            .bind(except)
            .fetch_optional(&mut *conn)
            .await?;
    match taken {
        Some((work, slug, title)) => Err(StoreError::DuplicateTitle { work, slug, title }),
        None => Ok(()),
    }
}

/// Append revision `number` of the work template.
pub(crate) async fn insert_template_revision(
    conn: &mut SqliteConnection,
    work: WorkId,
    number: u32,
    body: &str,
    author: &str,
    note: &str,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, ?, ?, ?, ?)")
        .bind(work)
        .bind(i64::from(number))
        .bind(body)
        .bind(author)
        .bind(note)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

// ---- Backfilling the keys when opening -------------------------------------------------------

/// A work during the backfill: id, title, stored key, seeded (without a source).
#[derive(Debug, Clone)]
struct Entry {
    id: WorkId,
    title: String,
    key: Option<String>,
    seeded: bool,
}

/// New `(id, title, key)` for all works whose title or key changes. `entries` is sorted by id. Rules (contract Round 4,
/// section 1): with the same key the work with the smaller id keeps its title; if a seeded work (without a source)
/// meets an imported one in the process, the imported one with the smallest id keeps the title, and the seeded one gets
/// the title of the sample work (`sample_title`), provided it is free. Otherwise and in all other cases „<Titel> (2)“,
/// „ (3)“ … with the smallest free number.
fn plan_titles(entries: &[Entry], sample_title: &str) -> Vec<(WorkId, String, String)> {
    let keys: Vec<String> = entries.iter().map(|e| title_key(&e.title)).collect();
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, key) in keys.iter().enumerate() {
        groups.entry(key.as_str()).or_default().push(index);
    }
    // Losers with the hint whether a seeded work met an imported one.
    let mut losers: Vec<(usize, bool)> = Vec::new();
    for members in groups.values().filter(|m| m.len() > 1) {
        let first_imported = members.iter().copied().find(|&i| !entries[i].seeded);
        let keeper = first_imported.unwrap_or(members[0]);
        for &i in members.iter().filter(|&&i| i != keeper) {
            losers.push((i, entries[i].seeded && first_imported.is_some()));
        }
    }
    losers.sort_unstable();
    let loser_set: HashSet<usize> = losers.iter().map(|&(i, _)| i).collect();
    let mut used: HashSet<String> =
        keys.iter().enumerate().filter(|(i, _)| !loser_set.contains(i)).map(|(_, k)| k.clone()).collect();
    let mut renamed: HashMap<usize, String> = HashMap::new();
    for (index, seeded_against_import) in losers {
        let base = entries[index].title.trim();
        let title = if seeded_against_import && !used.contains(&title_key(sample_title)) {
            sample_title.to_owned()
        } else {
            (2u32..)
                .map(|n| format!("{base} ({n})"))
                .find(|t| !used.contains(&title_key(t)))
                .expect("eine freie Zahl gibt es immer")
        };
        used.insert(title_key(&title));
        renamed.insert(index, title);
    }
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let title = renamed.remove(&index).unwrap_or_else(|| entry.title.clone());
            let key = title_key(&title);
            (title != entry.title || entry.key.as_deref() != Some(key.as_str())).then_some((entry.id, title, key))
        })
        .collect()
}

/// Backfill the keys (after `MIGRATOR.run`, before the seeding; `BEGIN IMMEDIATE`, repeatable): works without a
/// `title_key` or with an outdated one get it; if it collides, only the title changes (rules at [`plan_titles`]). Slug,
/// mirror, outline and texts stay. If everything is already right, nothing changes.
pub(crate) async fn ensure_title_keys(pool: &SqlitePool) -> Result<(), StoreError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let rows: Vec<(WorkId, String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, title, title_key, source_name FROM work ORDER BY id").fetch_all(&mut *tx).await?;
    let entries: Vec<Entry> =
        rows.into_iter().map(|(id, title, key, source)| Entry { id, title, key, seeded: source.is_none() }).collect();
    let plan = plan_titles(&entries, &baruch_core::sample::work().title);
    if plan.is_empty() {
        return Ok(());
    }
    // First release all affected keys, then set them anew: that way the unique index does not get in the way in an
    // intermediate state.
    for (id, _, _) in &plan {
        sqlx::query("UPDATE work SET title_key = NULL WHERE id = ?").bind(id).execute(&mut *tx).await?;
    }
    for (id, title, key) in &plan {
        sqlx::query("UPDATE work SET title = ?, title_key = ? WHERE id = ?")
            .bind(title)
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

// ---- Store ------------------------------------------------------------------------------------

type MetaRow = (
    WorkId,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    i64,
    String,
    String,
);
type SummaryRow = (WorkId, String, String, String, String, String, String, String, i64, i64, i64, Option<String>);

impl Store {
    /// All works for overview and work selection, by id: author, kind, subtitle, number of nodes (`nodes`) and of
    /// sections without parts (`sections`), open proposals, created and last changed (largest point in time from
    /// metadata, created and changed revisions).
    pub async fn work_list(&self) -> Result<Vec<WorkSummary>, StoreError> {
        let rows: Vec<SummaryRow> = sqlx::query_as(
            "SELECT w.id, w.slug, w.title, w.author, w.kind, w.subtitle, w.created, w.language, \
                    (SELECT count(*) FROM node n WHERE n.work_id = w.id), \
                    (SELECT count(*) FROM node n WHERE n.work_id = w.id AND n.kind <> 'part'), \
                    (SELECT count(*) FROM revision r JOIN node n ON n.id = r.node_id \
                      WHERE n.work_id = w.id AND r.status = 'proposed'), \
                    nullif(max(coalesce(w.updated, ''), \
                        coalesce((SELECT max(r.created) FROM revision r JOIN node n ON n.id = r.node_id \
                                   WHERE n.work_id = w.id), ''), \
                        coalesce((SELECT max(r.edited) FROM revision r JOIN node n ON n.id = r.node_id \
                                   WHERE n.work_id = w.id), '')), '') \
               FROM work w ORDER BY w.id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(id, slug, title, author, kind, subtitle, created, language, nodes, sections, proposals, updated)| {
                Ok(WorkSummary {
                    id,
                    slug,
                    title,
                    author,
                    nodes: to_usize(nodes, "Anzahl der Knoten")?,
                    kind: words::parse_work_kind(&kind)?,
                    subtitle,
                    sections: to_usize(sections, "Anzahl der Abschnitte")?,
                    proposals: to_usize(proposals, "Anzahl der Vorschläge")?,
                    created,
                    updated,
                    language: crate::WorkLanguage::parse(&language).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// New work from a skeleton (atomic): work template (revision 1), chapters or sermon parts with revision 1 by
    /// `human` (current, text `= <title>\n\n`). Empty title → `EmptyTitle`, title taken → `DuplicateTitle`; folder name
    /// from the title, on a collision `-2`, `-3` ….
    pub async fn create_work(&self, new: &NewWork<'_>, human: &str) -> Result<Imported, StoreError> {
        self.create_work_with_text(new, human, None).await
    }

    /// Like [`Store::create_work`]; for a sermon `sermon_text` (Typst, for example the quotation of the sermon text
    /// with its source) stands under the heading of the item [`SERMON_TEXT_TITLE`]. For a book and without a text every
    /// item keeps only its heading. The sermon gets the outline scheme `sermon`, sermon text and application
    /// `numbered = 0` (Round 16a).
    pub async fn create_work_with_text(
        &self,
        new: &NewWork<'_>,
        human: &str,
        sermon_text: Option<&str>,
    ) -> Result<Imported, StoreError> {
        self.create_work_with_bible(new, human, sermon_text, None).await
    }

    /// Like [`Store::create_work_with_text`], plus the SWORD Bible of the work (`work_bible`) **in the same
    /// transaction** (Round 23). Before, these were two steps (`create_work_with_text`, then `set_work_bible`): if the
    /// second one failed, a work without a mirror was left behind, and a second attempt ended with `DuplicateTitle`.
    /// Now nothing is created on an error. An inadmissible module name (empty, over 100 characters, with a line break –
    /// the rule of the table) → `Refused`, before anything is written.
    pub async fn create_work_with_bible(
        &self,
        new: &NewWork<'_>,
        human: &str,
        sermon_text: Option<&str>,
        bible: Option<&str>,
    ) -> Result<Imported, StoreError> {
        if let Some(module) = bible
            && (module.is_empty() || module.chars().count() > 100 || module.contains('\n'))
        {
            return Err(StoreError::Refused(
                "Bibelmodul: Name mit 1 bis 100 Zeichen in einer Zeile erwartet; das Werk wurde nicht angelegt.",
            ));
        }
        let title = one_line(new.title);
        if title.is_empty() {
            return Err(StoreError::EmptyTitle);
        }
        let author = one_line(new.author);
        let key = title_key(&title);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        title_free(&mut tx, &key, None).await?;
        let slug = import::free_slug(&mut tx, &slug(&title)).await?;
        let template = preset_template(new.preset, &title, &author);
        let chapters = preset_chapters(new.preset, new.language);
        let sermon_title = sermon_text_title_in(new.language);
        let (scheme, numbers) = preset_numbers(new.preset, &chapters);
        let work: WorkId = sqlx::query_scalar(
            "INSERT INTO work (slug, kind, title, author, template, title_key, subtitle, audience, bible, \
                               numbering_scheme, language) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(&slug)
        .bind(words::work_kind(new.kind))
        .bind(&title)
        .bind(&author)
        .bind(&template)
        .bind(&key)
        .bind(one_line(new.subtitle))
        .bind(one_line(new.audience))
        .bind(one_line(new.bible))
        .bind(scheme)
        .bind(new.language.as_str())
        .fetch_one(&mut *tx)
        .await?;
        insert_template_revision(&mut tx, work, 1, &template, human, CREATED_NOTE).await?;
        for (seq, ((chapter, numbered), number)) in chapters.iter().zip(&numbers).enumerate() {
            let node_id: i64 = sqlx::query_scalar(
                "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
                 VALUES (?, ?, NULL, ?, ?, ?, 'chapter') RETURNING id",
            )
            .bind(work)
            .bind(seq as i64)
            .bind(seq as i64)
            .bind(number)
            .bind(chapter)
            .fetch_one(&mut *tx)
            .await?;
            // The trigger `node_numbered_default` (0006) sets `numbered = 0` only for an empty number; here explicitly,
            // so that "without number" does not depend on the number.
            sqlx::query("UPDATE node SET numbered = ? WHERE id = ?")
                .bind(i64::from(*numbered))
                .bind(node_id)
                .execute(&mut *tx)
                .await?;
            let body = match sermon_text.map(str::trim).filter(|t| !t.is_empty()) {
                Some(text) if new.preset == Preset::Sermon && *chapter == sermon_title => {
                    format!("= {chapter}\n\n{}\n\n", text.replace("\r\n", "\n"))
                }
                _ => format!("= {chapter}\n\n"),
            };
            sqlx::query(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
                 VALUES (?, 1, ?, 'human', ?, NULL, 'current', ?)",
            )
            .bind(node_id)
            .bind(body)
            .bind(human)
            .bind(CREATED_NOTE)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(module) = bible {
            // If that fails (for example on the CHECK rule of the table), the transaction rolls everything back.
            sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, ?)")
                .bind(work)
                .bind(module)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Imported { work, slug, title, author, nodes: chapters.len() })
    }

    /// Like [`Store::import_typst_noted`], optionally with a custom title (`None` or empty: from `#set document`,
    /// otherwise from the file name). Order of the checks: the same source already imported → `AlreadyImported`; no
    /// heading → `Import`; title taken → `DuplicateTitle`. The work template becomes revision 1 (note like the
    /// revisions).
    pub async fn import_typst_titled(
        &self,
        source: &str,
        file_name: &str,
        human: &str,
        note: &str,
        title: Option<&str>,
    ) -> Result<Imported, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let imported = import::import_typst(&mut tx, source, file_name, human, note, title).await?;
        tx.commit().await?;
        Ok(imported)
    }

    /// All details of a work; unknown → `UnknownWork`.
    pub async fn work_meta(&self, work: WorkId) -> Result<WorkMeta, StoreError> {
        let row: Option<MetaRow> = sqlx::query_as(
            "SELECT w.id, w.slug, w.kind, w.title, w.subtitle, w.author, w.audience, w.bible, w.template, \
                    w.source_name, w.created, w.updated, \
                    coalesce((SELECT max(t.number) FROM work_template_revision t WHERE t.work_id = w.id), 0), \
                    w.uuid, w.language \
               FROM work w WHERE w.id = ?",
        )
        .bind(work)
        .fetch_optional(&self.pool)
        .await?;
        let (
            id,
            slug,
            kind,
            title,
            subtitle,
            author,
            audience,
            bible,
            template,
            source_name,
            created,
            updated,
            rev,
            uuid,
            language,
        ) = row.ok_or(StoreError::UnknownWork(work))?;
        Ok(WorkMeta {
            id,
            uuid,
            slug,
            kind: words::parse_work_kind(&kind)?,
            title,
            subtitle,
            author,
            audience,
            bible,
            template,
            template_rev: to_u32(rev, "Fassung der Werkvorlage")?,
            source_name,
            created,
            updated,
            language: crate::WorkLanguage::parse(&language).unwrap_or_default(),
        })
    }

    /// Revisions of the work template by number (without text); unknown work → `UnknownWork`.
    pub async fn template_revisions(&self, work: WorkId) -> Result<Vec<TemplateRevision>, StoreError> {
        self.work_slug(work).await?;
        let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
            "SELECT number, author_name, note, created FROM work_template_revision WHERE work_id = ? ORDER BY number",
        )
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(number, author, note, created)| {
                Ok(TemplateRevision { number: to_u32(number, "Fassung der Werkvorlage")?, author, note, created })
            })
            .collect()
    }

    /// Text of revision `number` of the work template; `None` if it does not exist. Unknown work → `UnknownWork`.
    pub async fn template_revision(&self, work: WorkId, number: u32) -> Result<Option<String>, StoreError> {
        self.work_slug(work).await?;
        Ok(sqlx::query_scalar("SELECT body FROM work_template_revision WHERE work_id = ? AND number = ?")
            .bind(work)
            .bind(i64::from(number))
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Change metadata and work template (atomic, only the human). Only fields with a different value count
    /// (`changed`); then `updated` is set. Title empty → `EmptyTitle`, assigned to another work → `DuplicateTitle`
    /// (nothing changed). A different work template (CRLF → LF) becomes a new revision (`template_rev`) and
    /// `work.template`; if it contains a heading (`=` at the start of a line), that would belong to the outline →
    /// `Refused`. Slug and mirror folder stay.
    pub async fn update_work(&self, work: WorkId, edit: &WorkEdit<'_>, human: &str) -> Result<WorkUpdate, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row: Option<(String, String, String, String, String, String, String)> =
            sqlx::query_as("SELECT title, subtitle, author, kind, audience, bible, template FROM work WHERE id = ?")
                .bind(work)
                .fetch_optional(&mut *tx)
                .await?;
        let (title, subtitle, author, kind, audience, bible, template) = row.ok_or(StoreError::UnknownWork(work))?;
        let mut changed: Vec<&'static str> = Vec::new();

        let mut new_title = title.clone();
        if let Some(wanted) = edit.title {
            let wanted = one_line(wanted);
            if wanted.is_empty() {
                return Err(StoreError::EmptyTitle);
            }
            if wanted != title {
                title_free(&mut tx, &title_key(&wanted), Some(work)).await?;
                new_title = wanted;
                changed.push("title");
            }
        }
        let new_subtitle = pick(&mut changed, "subtitle", subtitle, edit.subtitle);
        let new_author = pick(&mut changed, "author", author, edit.author);
        let new_kind = match edit.kind {
            Some(wanted) if words::work_kind(wanted) != kind => {
                changed.push("kind");
                words::work_kind(wanted).to_owned()
            }
            _ => kind,
        };
        let new_audience = pick(&mut changed, "audience", audience, edit.audience);
        let new_bible = pick(&mut changed, "bible", bible, edit.bible);

        let mut template_rev = None;
        let mut new_template = template.clone();
        if let Some(wanted) = edit.template.map(|t| t.replace("\r\n", "\n"))
            && wanted != template
        {
            if baruch_typst::parse(&wanted).is_ok() {
                return Err(StoreError::Refused(
                    "Die Werkvorlage darf keine Überschrift (= am Zeilenanfang) enthalten; Überschriften gehören zur Gliederung.",
                ));
            }
            let last: i64 =
                sqlx::query_scalar("SELECT coalesce(max(number), 0) FROM work_template_revision WHERE work_id = ?")
                    .bind(work)
                    .fetch_one(&mut *tx)
                    .await?;
            let number = to_u32(last, "Fassung der Werkvorlage")? + 1;
            insert_template_revision(&mut tx, work, number, &wanted, human, &one_line(edit.note.unwrap_or(""))).await?;
            template_rev = Some(number);
            new_template = wanted;
            changed.push("template");
        }

        if !changed.is_empty() {
            sqlx::query(
                "UPDATE work SET title = ?, title_key = ?, subtitle = ?, author = ?, kind = ?, audience = ?, \
                        bible = ?, template = ?, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            )
            .bind(&new_title)
            .bind(title_key(&new_title))
            .bind(&new_subtitle)
            .bind(&new_author)
            .bind(&new_kind)
            .bind(&new_audience)
            .bind(&new_bible)
            .bind(&new_template)
            .bind(work)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Ok(WorkUpdate { work, changed, template_rev })
    }

    /// Delete a work completely (atomic, only the human). `confirm_title` must have the same [`title_key`] as the
    /// title, otherwise `Refused`, nothing deleted. In one transaction: all current revisions → `superseded` (so that
    /// `revision_no_delete_current` does not trigger), then revisions, references, posts, nodes, revisions of the work
    /// template and the work; id, slug and title go to `work_retired` (the slug stays reserved). Full-text index:
    /// revisions are removed by the triggers from `baruch-library` 0104, posts by those from 0105 (`post_fts_delete`).
    /// The server cleans up the mirror. If no works are left afterwards, the next start does not seed again.
    pub async fn delete_work(&self, work: WorkId, confirm_title: &str) -> Result<DeletedWork, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (slug, title): (String, String) = sqlx::query_as("SELECT slug, title FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(StoreError::UnknownWork(work))?;
        if title_key(confirm_title) != title_key(&title) {
            return Err(StoreError::Refused("Der eingegebene Titel passt nicht; nichts gelöscht."));
        }
        let nodes: i64 =
            sqlx::query_scalar("SELECT count(*) FROM node WHERE work_id = ?").bind(work).fetch_one(&mut *tx).await?;
        let revisions: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM revision WHERE node_id IN (SELECT id FROM node WHERE work_id = ?)",
        )
        .bind(work)
        .fetch_one(&mut *tx)
        .await?;
        delete_work_rows(&mut tx, work).await?;
        tx.commit().await?;
        Ok(DeletedWork {
            work,
            slug,
            title,
            nodes: to_usize(nodes, "Anzahl der Knoten")?,
            revisions: to_usize(revisions, "Anzahl der Revisionen")?,
        })
    }
}

impl Store {
    /// Deleted works from `work_retired` (newest first; Round 22, archive).
    pub async fn retired_works(&self) -> Result<Vec<RetiredWork>, StoreError> {
        let rows: Vec<(i64, String, String, String)> =
            sqlx::query_as("SELECT id, slug, title, deleted FROM work_retired ORDER BY deleted DESC, id DESC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id, slug, title, deleted)| RetiredWork { id, slug, title, deleted }).collect())
    }

    /// Creates an archived work again with its reserved slug from the Typst text of the archive folder (Round 22,
    /// archive), everything in one transaction (`BEGIN IMMEDIATE`):
    ///
    /// - `slug` must be in `work_retired`, otherwise `Refused` (nothing changed); the title comes from `work_retired`
    ///   (not from `#set document`), so that the work is called what it was called before the deletion.
    /// - If a living work carries the same title ([`title_key`]) → `Refused`; the same source already imported →
    ///   `Refused`; no heading → `Import`.
    /// - It is created as in the import ([`import::import_typst`]): preamble as work template (revision 1), every
    ///   heading an outline item with revision 1 by the human `human`, current, note [`RESTORED_NOTE`]; a rule block
    ///   „// Baruch: Satz“ becomes a typesetting setting again. The kind of work is „Buch“ afterwards (`work_retired`
    ///   does not know it).
    /// - The row in `work_retired` is dropped, the work gets exactly the reserved slug (the mirror folder can thus go
    ///   back to `works/<slug>`) and a new id (ids are never reused).
    pub async fn restore_retired(&self, slug: &str, typst: &str, human: &str) -> Result<RestoredWork, StoreError> {
        let slug = slug.trim();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let title: Option<String> = sqlx::query_scalar("SELECT title FROM work_retired WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&mut *tx)
            .await?;
        let Some(title) = title else {
            return Err(StoreError::Refused(
                "Dieses Werk steht nicht im Archiv (unbekannter Ordnername); nichts wiederhergestellt.",
            ));
        };
        match title_free(&mut tx, &title_key(&title), None).await {
            Ok(()) => {}
            Err(StoreError::DuplicateTitle { .. }) => {
                return Err(StoreError::Refused(
                    "Ein lebendes Werk trägt schon denselben Titel; erst jenes umbenennen, dann wiederherstellen. \
                     Nichts wiederhergestellt.",
                ));
            }
            Err(e) => return Err(e),
        }
        sqlx::query("DELETE FROM work_retired WHERE slug = ?").bind(slug).execute(&mut *tx).await?;
        let imported = match import::import_typst(&mut tx, typst, "main.typ", human, RESTORED_NOTE, Some(&title)).await
        {
            Ok(imported) => imported,
            Err(StoreError::AlreadyImported { .. }) => {
                return Err(StoreError::Refused(
                    "Dieselbe Quelle ist schon als lebendes Werk importiert; nichts wiederhergestellt.",
                ));
            }
            Err(e) => return Err(e),
        };
        if imported.slug != slug {
            sqlx::query("UPDATE work SET slug = ? WHERE id = ?")
                .bind(slug)
                .bind(imported.work)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(RestoredWork { work: imported.work, slug: slug.to_owned(), title: imported.title, nodes: imported.nodes })
    }
}

/// New value of a text field: `wanted` (on one line) if given and different from `now`; then `name` is in `changed`.
fn pick(changed: &mut Vec<&'static str>, name: &'static str, now: String, wanted: Option<&str>) -> String {
    match wanted.map(one_line) {
        Some(wanted) if wanted != now => {
            changed.push(name);
            wanted
        }
        _ => now,
    }
}

/// Delete all rows of a work, inside the running transaction (see [`Store::delete_work`]).
async fn delete_work_rows(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    // Images first (migration 0009): paths, versions (the current one set to superseded beforehand), images.
    crate::assets::delete_work_assets(conn, work).await?;
    const NODES: &str = "SELECT id FROM node WHERE work_id = ?";
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE revision SET status = 'superseded' WHERE status = 'current' AND node_id IN ({NODES})"
    )))
    .bind(work)
    .execute(&mut *conn)
    .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM revision WHERE node_id IN ({NODES})")))
        .bind(work)
        .execute(&mut *conn)
        .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM post_ref WHERE post_id IN (SELECT id FROM post WHERE node_id IN ({NODES}))"
    )))
    .bind(work)
    .execute(&mut *conn)
    .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM post WHERE node_id IN ({NODES})")))
        .bind(work)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM node WHERE work_id = ?").bind(work).execute(&mut *conn).await?;
    sqlx::query("DELETE FROM work_template_revision WHERE work_id = ?").bind(work).execute(&mut *conn).await?;
    // Remember id, slug and title (migration 0007): the slug stays reserved ([`import::free_slug`]), `AUTOINCREMENT`
    // never assigns the id again anyway.
    sqlx::query("INSERT INTO work_retired (id, slug, title) SELECT id, slug, title FROM work WHERE id = ?")
        .bind(work)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM work WHERE id = ?").bind(work).execute(&mut *conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: WorkId, title: &str, seeded: bool) -> Entry {
        Entry { id, title: title.into(), key: None, seeded }
    }

    #[test]
    fn title_keys_ignore_case_and_surrounding_or_repeated_whitespace() {
        assert_eq!(title_key(" Das Buch "), title_key("das  buch"), "Ränder und doppelte Leerzeichen zählen nicht");
        assert_eq!(title_key("ÄRGER\u{a0}im\tGarten"), "ärger im garten", "Unicode-Leerraum und Umlaute");
        assert_ne!(title_key("Das Buch"), title_key("Das Buch 2"), "verschiedene Titel bleiben verschieden");
    }

    #[test]
    fn a_seeded_work_meeting_an_imported_one_takes_the_sample_title() {
        let plan = plan_titles(&[entry(1, "Mövenoog", true), entry(2, "Mövenoog", false)], "Beispielwerk");
        assert_eq!(
            plan,
            vec![(1, "Beispielwerk".into(), "beispielwerk".into()), (2, "Mövenoog".into(), "mövenoog".into())],
            "das gesäte Werk weicht, obwohl es die kleinere id hat; beide bekommen ihren Schlüssel"
        );
    }

    #[test]
    fn otherwise_the_larger_id_gets_the_smallest_free_number() {
        let entries =
            [entry(1, "Buch", false), entry(2, "buch", false), entry(3, "Buch (2)", false), entry(4, " BUCH ", false)];
        let plan = plan_titles(&entries, "Beispielwerk");
        let titles: Vec<(WorkId, &str)> = plan.iter().map(|(id, t, _)| (*id, t.as_str())).collect();
        assert_eq!(
            titles,
            vec![(1, "Buch"), (2, "buch (3)"), (3, "Buch (2)"), (4, "BUCH (4)")],
            "kleinere id behält, „(2)“ ist schon vergeben, danach die kleinste freie Zahl"
        );
    }

    #[test]
    fn a_taken_sample_title_falls_back_to_a_number() {
        let entries = [entry(1, "Beispielwerk", false), entry(2, "Mövenoog", true), entry(3, "Mövenoog", false)];
        let plan = plan_titles(&entries, "Beispielwerk");
        assert!(plan.contains(&(2, "Mövenoog (2)".into(), "mövenoog (2)".into())), "Beispieltitel vergeben: {plan:?}");
    }

    #[test]
    fn keys_that_are_already_right_need_no_change() {
        let entries = [Entry { id: 1, title: "Eins".into(), key: Some("eins".into()), seeded: true }];
        assert!(plan_titles(&entries, "Beispielwerk").is_empty(), "zweiter Lauf ändert nichts");
    }

    #[test]
    fn new_works_get_a_template_with_document_and_numbering() {
        let book = preset_template(Preset::Book, "Ein \"Buch\"", "A\\B");
        let head =
            "#set document(title: \"Ein \\\"Buch\\\"\", author: \"A\\\\B\")\n#set heading(numbering: \"1.1\")\n\n";
        assert_eq!(
            book,
            format!("{head}{}", baruch_typst::cover::imprint_template()),
            "Buch: Titel und Autor als Typst-String maskiert, Nummerierung 1.1, danach die Impressum-Vorlage (Runde 27)"
        );
        assert_eq!(
            baruch_typst::cover::template_functions(&book),
            baruch_typst::cover::TemplateFunctions {
                half_title: true,
                main_title: true,
                imprint: true,
                dedication: true
            },
            "ein neues Buch kennt alle vier Titelei-Funktionen, sonst bliebe die Titelei beim Rückfall"
        );
        assert_eq!(
            preset_template(Preset::Sermon, "Predigt", ""),
            "#set document(title: \"Predigt\", author: \"\")\n\n",
            "Predigt: nur #set document"
        );
    }
}
