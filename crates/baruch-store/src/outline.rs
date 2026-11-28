// SPDX-License-Identifier: AGPL-3.0-or-later
//! Outline manager (contract Round 4, sections 2 and 3c): moving, changing the level, renaming, creating, deleting,
//! numbering and headings in the text (splitting, merging).
//!
//! Rules:
//! - The outline is the **sequence of levels in tree order**: part 0 (top level only, since Round 14 with the part
//!   marker [`baruch_typst::part_line`] as its heading line; legacy data without the marker stays as it is), chapter 1
//!   (`= `), section 2 (`== `), subsection 3 (`=== `; each also as the marker "without number"
//!   [`baruch_typst::unnumbered_line`]). As in the import, the parent is the nearest predecessor with a smaller level.
//!   Valid is a sequence in which every level is at most one deeper than the previous one (so the first non-part is a
//!   chapter) and none deeper than 3. Invalid → [`StoreError::Refused`], nothing changed.
//! - Every change assigns `seq`, `parent_seq` and `position` anew in tree order (in two stages through negative values
//!   because of `UNIQUE(work_id, seq)`), and writes `node.number` and `post_ref.node_seq` along (references to deleted
//!   nodes are dropped).
//! - If level or title of a node with text changes, it gets a new revision by the human (current, parent = the previous
//!   current one) that differs only in the heading line. Otherwise no text changes.
//! - Deleting whole nodes: in the same transaction first the current revision `current → superseded` (allowed according
//!   to migration 0004), then revisions, references, posts and the node. That way the trigger
//!   `revision_no_delete_current` stays effective for living sections.
//! - Parts (Round 14): a new part stands before or behind a part or chapter, also in works with a work template;
//!   revision 1 is `heading_line(0, title, None) + "\n"` (part marker and empty line). The following chapters up to the
//!   next part then stand below it (`adopted`), their texts and numbers stay (chapters count across parts). Renaming
//!   rewrites the marker line (empty text → marker); a part text without the marker (legacy data „= Teil …“) stays
//!   unchanged, only the title changes. Saving, proposals and switching align parts **with** the marker (exactly one
//!   marker at the beginning, no further heading; another title renames); part texts without the marker as before
//!   without aligning. Changing something into a part or changing the level of a part never works.
//! - Assistants and MCP clients may create as well ([`Store::outline_add_many`], wish 2026-11-06: revision 1 then
//!   carries their name). Renaming, moving, changing the level and deleting is done only by the human; assistants file
//!   a request for that (module `requests`, migration 0012), which the human confirms – only then do the functions here
//!   run, under the name of the human. Exception (user question 12, 2026-11-07): a part that an assistant creates and
//!   that would adopt existing chapters becomes a request as well (`Store::outline_add_by_assistant`).
//! - Outline and heading lines are the same: if the current revision is saved outside the editor (`revision.save`,
//!   changing a revision) or a proposal is created, the text must carry exactly one heading of the level of the item
//!   ([`check_current_heading`], [`check_proposal_heading`]); only another title of the same level renames the item.
//! - Switching and accepting (contract Round 5, [`follow_switched`]): the revision brings its level. The outline
//!   follows level and title of its heading line, sub-items are moved along in level; the revision itself is never
//!   rewritten.
//! - Moving keeps the level: before/behind only an item of the same level, below only an item exactly one level higher
//!   ([`MOVE_KEEPS_LEVEL`]).
//! - Title rules (contract Round 7, [`check_title`]): empty, whitespace only, a line break or more than
//!   [`MAX_TITLE_CHARS`] characters → [`StoreError::Title`], nothing changed; when renaming, creating, saving in the
//!   editor (every title, also split ones), checking (`valid: false`), saving and changing the current revision, in
//!   proposals and when switching. Titles come only from real heading lines ([`baruch_typst::parse`]). Import and
//!   export do not check (byte-identical), and existing empty titles stay until the human renames. Since Round 8
//!   saving, checking, changing the current revision, proposals and switching check only **changed** titles: an
//!   unchanged first heading (for example an imported empty or overlong title) does not hold up the saving of the text;
//!   new (split) and deviating titles are still checked.

use std::collections::HashMap;

use baruch_core::{Author, NodeKind};
use baruch_typst::scheme::{Numbering, Scheme};
use sqlx::SqliteConnection;

use serde::Serialize;

use crate::load::{self, Numbered};
use crate::{Change, Idea, IdeaId, IdeaUse, Store, StoreError, WorkId, broken, to_usize, words, write};

/// Where an outline item goes relative to the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    /// Before the target, on its level.
    Before,
    /// Behind the target together with its sub-items, on its level.
    After,
    /// As the last sub-item below the target (level of the target + 1).
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
    /// An item counts with or without a number (`node.numbered`, for example a preface, wish 2026-11-06).
    Numbered,
}

impl OutlineAction {
    /// Value of the trailer `Baruch-Action` in the Git mirror.
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

/// Result of an outline change (for answer and mirror).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineChange {
    pub work: WorkId,
    pub action: OutlineAction,
    /// seq of the node afterwards; for `Deleted`/`Merged` the one shown now (the previous one).
    pub node: usize,
    /// Title of the node (for `Deleted`/`Merged` of the removed one).
    pub title: String,
    /// For the subject, for example `„Sommer“ vor „Frühling“ verschoben`.
    pub detail: String,
    /// New revisions `(seq, number)`.
    pub revisions: Vec<(usize, u32)>,
    /// Renumbered nodes `[old, new]`.
    pub renumbered: Vec<(usize, usize)>,
    /// Removed nodes: title and text of the deleted current revision.
    pub removed: Vec<(String, String)>,
}

/// What [`Store::outline_save`] may change about the outline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Restructure {
    /// Further headings in the text split the section.
    pub split: bool,
    /// Without a heading the text is appended to the previous outline item.
    pub merge: bool,
}

/// Result of [`Store::outline_check`] (read only).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct HeadingCheck {
    /// Title and level of the first heading in the text.
    pub title: Option<String>,
    pub level: Option<u8>,
    /// Titles of further headings: saving splits the section (with confirmation).
    pub split: Vec<String>,
    /// No heading: saving merges with the previous outline item (with confirmation).
    pub merge: bool,
    /// Would saving yield a valid outline?
    pub valid: bool,
    pub message: Option<String>,
}

/// Result of [`Store::outline_save`]: the saved revision and, if the outline has changed, its change.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineSave {
    pub change: Option<Change>,
    pub outline: Option<OutlineChange>,
    /// Round 22: reason for the new revision, only in the mode Auto and with a new revision.
    pub new_reason: Option<crate::NewReason>,
}

/// Numbering of a work (for display and form).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NumberingInfo {
    /// Configured numbering in the pattern language: `None` = from the work template (or a scheme with words without a
    /// pattern, see `scheme`), `""` = no numbers, otherwise the pattern.
    pub numbering: Option<String>,
    /// Pattern from `#set heading(numbering: …)` of the work template, if present.
    pub template: Option<String>,
    /// Display text: valid pattern (default "1.1"), `""` = no numbers, for schemes with words their example.
    pub effective: String,
    /// Outline scheme (id, migration 0008) and its name.
    pub scheme: String,
    pub scheme_label: String,
    /// Stored custom pattern (`work.numbering`), even if another scheme applies right now.
    pub custom: Option<String>,
    /// Display name for the outline header (Round 7): name of the scheme whose pattern corresponds to the effective one
    /// ([`Scheme::matching`], also with „Aus der Werkvorlage“ and „Eigenes Muster“, for example "I|1" → „Nur Kapitel
    /// römisch“), otherwise the pattern itself, without numbers „keine“, for schemes with words their name.
    pub label: String,
}

/// A row of the preview ([`Store::numbering_preview`]): position, fixed key, level, number, title.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NumberPreview {
    pub node: usize,
    pub key: i64,
    pub level: u8,
    pub number: String,
    pub title: String,
}

/// An outline item in tree order as it is to be after the change.
#[derive(Debug, Clone)]
struct Item {
    /// Database id; `None` = to be created.
    id: Option<i64>,
    /// State before `(seq, level, title)`; `None` = new.
    orig: Option<(usize, u8, String)>,
    level: u8,
    title: String,
    numbered: bool,
    /// Text of revision 1 of a new node.
    body: Option<String>,
    /// Text already written (saving in the editor): do not set the heading line anew.
    text_done: bool,
}

struct Outline {
    template: String,
    numbering: Option<String>,
    /// Outline scheme (`work.numbering_scheme`, migration 0008).
    scheme: String,
    /// Work language (Round 39b): words of the schemes with words.
    language: crate::WorkLanguage,
    items: Vec<Item>,
}

impl Outline {
    fn pattern(&self) -> Numbering {
        load::work_numbering(&self.scheme, self.numbering.as_deref(), &self.template, self.language)
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

/// Outline of the work in tree order (depth-first search, siblings by position), with levels.
async fn load_outline(conn: &mut SqliteConnection, work: WorkId) -> Result<Outline, StoreError> {
    let head: Option<(String, Option<String>, String, String)> =
        sqlx::query_as("SELECT template, numbering, numbering_scheme, language FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
    let (template, numbering, scheme, language) = head.ok_or(StoreError::UnknownWork(work))?;
    let language = crate::WorkLanguage::parse(&language).unwrap_or_default();
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
            continue; // loop in parent_seq
        }
        let level = push(index, parent_level, &mut items)?;
        if let Some(list) = children.get(&Some(rows[index].1)) {
            stack.extend(list.iter().rev().map(|&(_, _, i)| (i, Some(level))));
        }
    }
    // Nodes whose parent is missing are not lost: at the end, by seq.
    for (index, seen) in seen.iter_mut().enumerate() {
        if !*seen {
            *seen = true;
            push(index, None, &mut items)?;
        }
    }
    Ok(Outline { template, numbering, scheme, language, items })
}

/// Is the sequence of levels valid? Parts (0) stand on the top level, every level at most one deeper than the previous
/// one, none deeper than 3.
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

/// End (exclusive) of the subtree from position `index`: all following items with a larger level.
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

/// Longest title of an outline item in characters (Round 7): anything longer is no longer a title but a paragraph that
/// got into the heading line by mistake.
pub const MAX_TITLE_CHARS: usize = 200;

/// Title rules of an outline item (contract Round 7, 4b), the same everywhere: renaming and creating in the tree,
/// heading lines when saving in the editor (also every split title), when saving and changing the current revision, in
/// proposals and when switching.
///
/// Whitespace at the edges ([`char::is_whitespace`]) is dropped; empty → [`StoreError::Title`] „Ein Gliederungspunkt
/// braucht einen Titel.“; a line ending in the sense of Typst ([`baruch_typst::is_newline`]) → „Der Titel darf keinen
/// Zeilenumbruch enthalten.“ (it is no longer turned into a space); more than [`MAX_TITLE_CHARS`] characters → „Der
/// Titel ist zu lang (höchstens 200 Zeichen, hier N).“ Otherwise the trimmed title.
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

/// The **changed** titles of the headings of a text after [`check_title`] (Round 8): the first heading only if its
/// title deviates from `old_title` (the present title of the outline item), every further one (it would split, so it is
/// new) always. That way a section with an old empty or overlong title (the import does not check) can still be saved;
/// a new or changed invalid title is rejected as before.
fn check_chunk_titles(chunks: &[(u8, String, String)], old_title: &str) -> Result<(), StoreError> {
    for (i, (_, title, _)) in chunks.iter().enumerate() {
        if i == 0 && title == old_title {
            continue;
        }
        check_title(title)?;
    }
    Ok(())
}

/// First heading of a text if nothing but whitespace stands before it.
struct FirstHeading<'a> {
    /// Start of the heading line in the text.
    start: usize,
    /// The line itself (with line ending, if present).
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

/// Title of the first heading of a text (only whitespace before it) as switching would take it over (exchange, Round
/// 26: "does a revision already carry the title from the file?").
pub(crate) fn heading_title(body: &str) -> Option<String> {
    first_heading(body).map(|found| found.title)
}

/// Text with a new heading line (only this line changes). If the title stays, the line stays verbatim apart from the
/// level (escapes, label, comment; [`baruch_typst::relevel_line`], also for the marker "without number", Round 14).
/// Without a heading at the beginning a new line goes before it.
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

/// Text of a part with a new title (Round 14): if the part marker stands at the beginning, only its line changes (the
/// comment stays); an empty text (whitespace only) gets the marker. Otherwise – the text carries no marker (legacy data
/// like „= Teil I“ or free text) – `None`: the text stays.
fn rewrite_part(body: &str, title: &str) -> Option<String> {
    if body.trim().is_empty() {
        let rest = if body.is_empty() { "\n" } else { body };
        return Some(format!("{}{rest}", baruch_typst::heading_line(0, title, None)));
    }
    let found = first_heading(body).filter(|found| found.level == 0)?;
    let line = baruch_typst::heading_line(0, title, Some(found.line));
    Some(format!("{}{line}{}", &body[..found.start], &body[found.start + found.line.len()..]))
}

/// Delete a whole node: first supersede the current revision (current → superseded, allowed according to 0004), then
/// revisions, references of its posts, posts and the node.
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

/// Text of the current revision (empty without revisions).
async fn current_body(conn: &mut SqliteConnection, node_id: i64) -> Result<String, StoreError> {
    Ok(write::current(conn, node_id).await?.map(|(_, body, _)| body).unwrap_or_default())
}

/// Parent (position) per item: the nearest predecessor with a smaller level.
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

/// Write `node.number` of all nodes after the pattern (nodes must already have their id).
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

/// Posts of the work (for `post_ref`).
const WORK_POSTS: &str = "post_id IN (SELECT p.id FROM post p JOIN node n ON n.id = p.node_id WHERE n.work_id = ?)";

struct Applied {
    revisions: Vec<(usize, u32)>,
    renumbered: Vec<(usize, usize)>,
    /// Renamed parts whose text carries no part marker (legacy data): text unchanged.
    kept_parts: Vec<String>,
}

/// Write a new outline: seq/parent_seq/position/kind/title in tree order, new nodes with revision 1, numbers,
/// references, new revisions only with a changed heading line. `removed` are the old seq of nodes already removed
/// (their references are dropped).
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

/// Like [`apply`]; with `bare` new nodes get **no** revision 1 (exchange, Round 26: the reading in afterwards creates
/// exactly the revisions from the file, [`insert_bare`]).
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

    // In two stages because of UNIQUE(work_id, seq): first all negative, then the new values.
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
                // Number provisionally not empty, otherwise the trigger from 0006 would take the node for unnumbered.
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
                // Revision 1 by the creator: the human or (wish 2026-11-06) a persona or an MCP client that creates
                // through `outline_add`.
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

    // References in posts: deleted ones are dropped, the others get the new seq (in two stages).
    for &seq in removed {
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM post_ref WHERE node_seq = ? AND {WORK_POSTS}")))
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
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE post_ref SET node_seq = ? WHERE node_seq = ? AND {WORK_POSTS}"
        )))
        .bind(-(new as i64) - 1)
        .bind(old as i64)
        .bind(work)
        .execute(&mut *conn)
        .await?;
    }
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE post_ref SET node_seq = -node_seq - 1 WHERE node_seq < 0 AND {WORK_POSTS}"
    )))
    .bind(work)
    .execute(&mut *conn)
    .await?;

    // New revisions: only the heading line changes.
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
            // Part (Round 14): marker line anew, an empty text gets it; without the marker the text stays (legacy data,
            // user question 8), only the title changes.
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
        // Rewriting existing revisions is the business of the human (also with a confirmed request); creating by an
        // assistant changes no existing heading.
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

/// Message if moving would change the level.
pub(crate) const MOVE_KEEPS_LEVEL: &str =
    "Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).";

/// Does moving with `place` relative to a target of the level `target_level` keep the level `level`? `before`/`after`
/// only at the same level, `into` only below an item exactly one level higher.
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
    // The level stays (contract Round 5): before/behind only an item of the same level, below only an item exactly one
    // level higher. Changing the level goes through ← / → (`outline.set_level`).
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

/// A new item on the level of the place (human; RPC `outline.add` without `level`, ideas).
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

/// A new outline item for [`Store::outline_add_many`] (MCP `outline_add`, wish 2026-11-06).
///
/// `level` is the **absolute** level (0 part, 1 chapter, 2 section, 3 subsection); if it is missing, the level of the
/// place applies (level of the reference item for `before`/`after`, one deeper for `into`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NewPoint {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
}

/// An item created: position afterwards, fixed key, level, title.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AddedPoint {
    pub node: usize,
    pub key: i64,
    pub level: u8,
    pub title: String,
}

/// Result of [`Store::outline_add_many`]: the outline change (one commit), the new items in tree order and (Round 14)
/// the fixed keys of the existing chapters that now stand below a new part (order, texts and chapter numbers
/// unchanged).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineAdded {
    pub change: OutlineChange,
    pub points: Vec<AddedPoint>,
    pub adopted: Vec<i64>,
}

/// Result of [`Store::idea_to_outline`] (Round 14): the outline change, the fixed key of the new item and the idea
/// afterwards (processed as an outline item, `done_ref` "key <key>").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdeaOutline {
    pub change: OutlineChange,
    pub key: i64,
    pub idea: Idea,
}

/// Maximum number of new items in one call of [`Store::outline_add_many`].
pub const MAX_NEW_POINTS: usize = 100;

/// Title of an item from a list with its number („Punkt 3: …“), for a single one without.
fn point_title(title: &str, index: usize, many: bool) -> Result<String, StoreError> {
    check_title(title).map_err(|e| match e {
        StoreError::Title(message) if many => StoreError::Title(format!("Punkt {}: {message}", index + 1)),
        other => other,
    })
}

/// Create several items as one block (atomic, wish 2026-11-06). The block stands before `target` or behind its
/// sub-items (`after`, `into`). The first item stands on the level of the place, no further one higher than it: that
/// way every existing item stays where it was (no existing item becomes a sub-item of a new one – except below a new
/// part, see below). `target = None` only in a work without an outline item (the first item is then a chapter or a
/// part).
///
/// Exception parts (Round 14, also with a work template): level 0 works before or behind a part or chapter (not `into`,
/// not next to a section), also as the first item next to a chapter and in the middle of the block. The existing
/// chapters behind it up to the next part then stand below the new part (`adopted`); their level, their text and their
/// number stay, parent (`parent_seq`) and position (`position`) change. Assistants (MCP `outline_add`) create a part
/// that way only if it adopts no existing chapters; otherwise it becomes a request to which the human consents
/// ([`Store::outline_add_by_assistant`], user question 12, decided 2026-11-07).
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
    // A part may stand as the first item next to a chapter as well (then the level of the place is 1).
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
            // Heading line (for a part the part marker, Round 14) and empty line.
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
    // Existing chapters whose parent is now a new part.
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

/// Result of [`insert_bare`] (exchange, Round 26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BareInsert {
    /// Created, without a revision: fixed key of the new item.
    Inserted(i64),
    /// A new part would adopt existing chapters (`adopted`); nothing written. `anchor` and `place` are reference item
    /// and place for a request [`crate::OutlineOp::Add`] at the same position.
    WouldAdopt { adopted: Vec<i64>, anchor: i64, place: Place },
    /// At this position the item would get another parent, would adopt existing items or would yield no valid outline;
    /// nothing written.
    Misplaced(String),
}

/// Insert an item **without** a revision (exchange, Round 26, in the transaction of the caller): behind the subtree of
/// `after`, otherwise as the first child of `parent`, otherwise as the first item of the outline. The new item must
/// have exactly `parent` as its parent there, and no existing item may change its parent; otherwise nothing is written
/// ([`BareInsert::WouldAdopt`] for a new part before chapters, otherwise [`BareInsert::Misplaced`]). Existing items
/// keep title, level and text; the outline computes seq, position and number anew as when creating. The reading in does
/// not check the title (like the import).
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
    // Parent per item as a key; the new item has none yet and is called -1 here (keys are > 0).
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

/// Message: a part always stands on the top level.
const PART_NOT_INTO: &str = "Ein Teil steht immer auf oberster Ebene; „darunter“ (into) geht mit Ebene 0 nicht.";
/// Message: parts only next to parts and chapters.
const PART_PLACE: &str = "Ein Teil entsteht nur vor oder hinter einem Teil oder Kapitel, nicht neben einem Abschnitt.";

/// State of an item through its fixed key: `(seq, level, title)`; `None` if it does not exist in the work (any longer).
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

/// An item of the outline for the preview of a request (module `requests`, Round 14).
#[derive(Debug, Clone)]
pub(crate) struct PointState {
    pub key: i64,
    pub level: u8,
    pub number: String,
    pub title: String,
    pub numbered: bool,
}

/// All items of the work in tree order with level, number (after the effective numbering) and title (in the transaction
/// of the caller).
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

/// Count an item with or without a number (wish 2026-11-06, for example an introduction or preface without a number):
/// sets `node.numbered` and rewrites all numbers; the text stays. Already so → `Refused`. A part without a number →
/// `Refused`: the part marker has no number attribute, export → import would make it numbered again and would shift the
/// numbers of the following parts. A part that (from an older state) is already without a number may be counted with a
/// number again by the human.
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

/// Text of the editor taken apart: whether the beginning fits (only whitespace before the first heading), and the
/// pieces per heading `(level, title, text)`; whitespace before it belongs to the first piece.
struct Analysis {
    lead_ok: bool,
    chunks: Vec<(u8, String, String)>,
    /// Per piece: as a `=` heading (or part) with a number, `false` for the marker "without number".
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

/// Does the text carry a part marker as a heading (Round 14)?
fn has_part_marker(body: &str) -> bool {
    analyze(&body.replace("\r\n", "\n")).chunks.iter().any(|(level, _, _)| *level == 0)
}

/// What does not fit about the text of a part (Round 14): if it carries a part marker (or `required` demands it,
/// because the current revision has one), it must stand exactly once at the beginning (only whitespace before it),
/// without a further heading. Without the marker and without `required` → `None`: part texts without the marker (legacy
/// data) are saved as before without aligning.
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

/// New outline if the text of item `index` is saved like this (one or several headings). As with
/// [`Store::outline_set_level`] the sub-items move along by the same difference; the new items stand directly behind
/// the item (before its sub-items). The caller checks the validity.
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
                // A split piece with the marker "without number" counts without a number (as in the import).
                numbered: analysis.numbered.get(offset + 1).copied().unwrap_or(true),
                body: Some(text.clone()),
                text_done: true,
            },
        );
    }
    Ok(new)
}

/// [`check`] for a part (Round 14): without the marker (and without a marker in the current revision) valid as before
/// without outline logic; otherwise exactly one marker at the beginning, another title renames.
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

/// [`save`] for a part (Round 14). Without the marker (in text and current revision) as before without outline logic;
/// otherwise the text demands exactly one marker at the beginning ([`part_problem`]), another title in it renames the
/// part.
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
            write::row(conn, id, node, base).await?; // unknown base → UnknownRevision
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
        // Posts move along, behind those of the previous item.
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

    // Title rules before everything else: the first and every split title (nothing written).
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
        // Only the title: take it over from the heading line of the revision that is current now.
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

/// [`save`] in the mode `Auto` (Round 22, contract §2.1/2.2): if heading and level stay (no splitting, no merging, for
/// a part no new title) and the rules allow it ([`crate::amend::verdict`]), the current revision of the human is
/// amended; otherwise a new revision with a reason like [`save`] (`OutlineSave::new_reason`). Anything invalid (text
/// before the heading, broken part marker) is reported by [`save`].
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
    // (stored text, heading changed?) – `None`: invalid, `save` reports that itself.
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
            // No heading or further headings: merging or splitting changes the outline.
            _ => Some((body.clone(), true)),
        }
    };
    let Some((stored, heading_changed)) = plan else {
        return save(conn, work, node, &body, based_on, allow, human).await;
    };
    if let Some(base) = based_on {
        write::row(conn, id, node, base).await?; // unknown base → UnknownRevision
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

/// Scheme from its id; unknown → [`StoreError::Numbering`].
fn scheme_of(id: &str) -> Result<Scheme, StoreError> {
    Scheme::from_id(id).ok_or_else(|| {
        let known: Vec<&str> = Scheme::ALL.iter().map(|s| s.id()).collect();
        StoreError::Numbering(format!("unbekanntes Gliederungsschema „{}“; bekannt: {}", id.trim(), known.join(", ")))
    })
}

/// Custom pattern: the one passed, otherwise the stored one (`work.numbering`), empty = none.
fn custom_spec<'a>(custom: Option<&'a str>, stored: Option<&'a str>) -> Option<&'a str> {
    custom.map(str::trim).filter(|c| !c.is_empty()).or_else(|| stored.map(str::trim).filter(|c| !c.is_empty()))
}

/// Numbering of a scheme (not yet stored) for the work; invalid pattern → [`StoreError::Numbering`].
fn numbering_for(outline: &Outline, scheme: Scheme, custom: Option<&str>) -> Result<Numbering, StoreError> {
    let custom = custom_spec(custom, outline.numbering.as_deref());
    Numbering::of_scheme_in(scheme, custom, &outline.template, outline.language).map_err(StoreError::Numbering)
}

/// Numbers of the outline in tree order after `numbering`.
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

/// Display of the numbering from scheme id, custom pattern and work template.
fn numbering_info(scheme: &str, numbering: Option<String>, template: &str) -> NumberingInfo {
    // Only the pattern is shown here, and patterns have no words: the language does not matter.
    let effective_numbering = load::work_numbering(scheme, numbering.as_deref(), template, crate::WorkLanguage::De);
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
    // With the custom pattern it is stored; otherwise `work.numbering` stays as it is.
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

/// Level of the node as in [`sync_heading`] (part 0, chapter 1, section 2, below a section 3); `None` for unknown
/// nodes.
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

/// What does not fit about the headings of a text for an item of the level `level`: text before the heading, no
/// heading, further headings or another level. `None` = fits (another title of the same level fits: it renames the
/// item).
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

/// After every change of the current revision outside the editor (accepting, switching, saving through `revision.save`,
/// changing a revision), in the transaction of the caller: if the headings of the revision that is current now do not
/// fit the outline item → `Heading` (the caller rolls back); otherwise take the title over from the heading line
/// ([`sync_heading`]). That way outline and heading lines stay the same, and export and import yield the same outline.
pub(crate) async fn check_current_heading(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
) -> Result<bool, StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    if let Some((level, old_title)) = heading_level(conn, node_id).await? {
        let body = current_body(conn, node_id).await?;
        // Parts (Round 14): only a revision with the marker is checked and aligns the title.
        let problem = if level == 0 { part_problem(&body, false) } else { heading_problem(&body, level) };
        if let Some(problem) = problem {
            return Err(StoreError::Heading(problem));
        }
        // The title of the item is still the old one here (`sync_heading` only follows afterwards).
        if level != 0 || has_part_marker(&body) {
            check_chunk_titles(&analyze(&body.replace("\r\n", "\n")).chunks, &old_title)?;
        }
    }
    sync_heading(conn, node_id).await
}

/// Before creating a proposal: does its text fit the outline item? Otherwise `Heading` – a proposal never changes the
/// outline, and it could not be accepted anyway.
pub(crate) async fn check_proposal_heading(
    conn: &mut SqliteConnection,
    work: WorkId,
    node: usize,
    body: &str,
) -> Result<(), StoreError> {
    let node_id = write::node_id(conn, work, node).await?;
    if let Some((level, old_title)) = heading_level(conn, node_id).await? {
        // Parts (Round 14): marker demanded if the proposal or the current revision has one.
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

/// Take the title of the node over from the heading line of its current revision, but only if its level fits the level
/// of the node (parts only with the part marker, Round 14). `true` if the title has changed. Runs in the transaction of
/// the caller (after every change of the current revision).
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

/// After switching (contract Round 5, user decision „Nein“): the outline follows the heading line of the revision that
/// is current now, the revision itself is never rewritten.
///
/// - Parts (Round 14): revision with the part marker → exactly one at the beginning, the title follows; revision
///   without the marker → nothing to do (as before), `Ok(None)`.
/// - Same level: the title follows as before ([`sync_heading`]), `Ok(None)`.
/// - Another level: level and title follow; all sub-items move along by the same difference and each get a new revision
///   by the human that differs only in the heading line. Result: a change with `action: Level`.
/// - No heading, further headings, text before it, a level that is invalid at this position or a sub-item that would
///   come deeper than 3 → `Heading` (the caller rolls everything back, also the switching).
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
        // As when saving, only a changed title (Round 8).
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
        // Sub-items stand deeper than the item, so after the shift at least on level 2. Deeper than 3 is not capped but
        // rejected: capping would turn a child into a sibling, and switching back would not restore the old outline.
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
    items[index].text_done = true; // the revision stays as it is
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
    /// Move an outline item together with its sub-items (atomic): before or behind `target` (on its level) or as the
    /// last sub-item below it. The kind stays (part, chapter, section); into its own subtree, to an invalid position or
    /// without effect → `Refused`.
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

    /// Change the level (1–3, atomic); sub-items move along by the same difference. Parts and level 0 → `Refused`,
    /// likewise an invalid outline.
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

    /// Rename (atomic): new title and new heading line (new revision by the human).
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

    /// New outline item (atomic): before/behind `target` on its level, `into` as the last sub-item (level + 1).
    /// Revision 1 by the human = heading line + empty line (for a part the part marker, Round 14; parts also with a
    /// work template).
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

    /// Several new outline items in **one** transaction (wish 2026-11-06, MCP `outline_add`): one block
    /// before/behind/below `target` ([`NewPoint`]: title and absolute level); on an error (title rules, levels, part in
    /// the wrong place) nothing is created. Revision 1 of every item = heading line + empty line by `author` (human,
    /// persona or MCP client; for a part the part marker). Parts (level 0, Round 14) before or behind a part or
    /// chapter, also with a work template; `adopted` names the existing chapters that now stand below a new part.
    /// `target = None` only in a work without an outline item.
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

    /// Process an idea as an outline item (Round 14, `idea.process` with `as: outline`), in **one** transaction: create
    /// the item like [`Store::outline_add`] (revision 1 by the human) and mark the idea as processed (`done_as`
    /// outline, `done_ref` "key <key>"). If something fails (idea deleted, from another work, title rules, place),
    /// nothing is changed.
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

    /// Count an item with (`true`) or without a number (`false`) (atomic, human only); all numbers anew.
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

    /// Fixed keys of the items without a number (`node.numbered = 0`) of a work.
    pub async fn unnumbered_keys(&self, work: WorkId) -> Result<Vec<i64>, StoreError> {
        Ok(sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND numbered = 0 ORDER BY seq")
            .bind(work)
            .fetch_all(&self.pool)
            .await?)
    }

    /// Delete an outline item together with revisions and posts (atomic, human only). With sub-items or as the last
    /// chapter → `Refused`. `removed` carries title and text of the current revision.
    pub async fn outline_delete(&self, work: WorkId, node: usize, human: &str) -> Result<OutlineChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = delete(&mut tx, work, node, human).await?;
        tx.commit().await?;
        Ok(done)
    }

    /// What would [`Store::outline_save`] change about the outline with this text? Writes nothing.
    pub async fn outline_check(&self, work: WorkId, node: usize, body: &str) -> Result<HeadingCheck, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check(&mut conn, work, node, body).await
    }

    /// Save text from the editor (atomic) and align the outline with the headings: one heading → save like
    /// [`Store::save_human_on`], align title and level; further headings → new items behind it only with `allow.split`;
    /// no heading → append to the previous item only with `allow.merge` (posts move along, the item is dropped).
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

    /// Set the numbering pattern of the work (atomic, as before Round 6): `None` = from the work template, `""` = no
    /// numbers, otherwise a pattern of the pattern language like "1.1", "I|1" or "I.;A.;1." (scheme „Eigenes Muster“).
    /// All numbers are rewritten; invalid pattern → [`StoreError::Numbering`], unchanged → `Refused`.
    pub async fn set_numbering(&self, work: WorkId, numbering: Option<&str>) -> Result<OutlineChange, StoreError> {
        let (scheme, custom) = match numbering.map(str::trim) {
            None => (Scheme::Template, None),
            Some("") => (Scheme::None, None),
            Some(spec) => (Scheme::Custom, Some(spec)),
        };
        self.set_numbering_scheme(work, scheme.id(), custom).await
    }

    /// Choose the outline scheme of the work (atomic, contract Round 6): `scheme` is an id from [`Scheme::ALL`],
    /// `custom` the custom pattern (only for `custom`; if it is missing, the stored one applies). Unknown scheme or
    /// invalid pattern → [`StoreError::Numbering`], unchanged → `Refused`.
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

    /// Numbers of the whole outline as they would be with `scheme` (and `custom`); writes nothing.
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

    /// Rule `#set heading(numbering: …)` that the export puts behind the work template with `scheme` (`None` for "from
    /// the work template": then the template stays unchanged); writes nothing.
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

    /// Configured numbering, the one read from the template and the valid numbering of the work.
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
