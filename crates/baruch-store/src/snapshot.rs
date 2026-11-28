// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst mirror (contract B1, section 6): `abschnitte/{seq:03}-{slug(title)}.typ` per node with a current revision and
//! `main.typ` with `#include` in tree order (depth-first search, siblings by position).
//!
//! If the work has a work template (import, decision 2026-11-05):
//! - `vorlage.typ` contains the template;
//! - `main.typ` begins with the template (set and show rules thus apply to everything included) and includes the
//!   sections after it;
//! - every section file begins with the line [`TEMPLATE_IMPORT`], so that `#let` functions of the template (for example
//!   `#szene`) apply there as well.
//!
//! Outline scheme (contract Round 6, section 2d): if a scheme other than "from the work template" is chosen, the
//! template is followed in `main.typ` and in the export by a comment line and a `#set heading(numbering: …)` rule
//! ([`baruch_typst::scheme::export_template`]); without a template it stands behind `// Baruch: <title>`. `vorlage.typ`
//! stays the stored template.
//!
//! Number on/off (contract Round 14, section 4, [`project`]): if the effective numbering is not "off", export, export
//! map, mirror and typesetting convert the first heading line of every node (except parts) whose `numbered` does not
//! fit the text form with [`baruch_typst::set_numbered_line`]: `numbered = 0` → marker "without number"
//! (`#heading(level: N, numbering: none)[…] <baruch-unnumbered>`), `numbered = 1` → `=` line. Only a heading with a
//! label of its own cannot be marked; for it the show rule `numbering: none` of Round 6 stays behind the template (also
//! with "from the work template") (`fallback_rules`). A freshly imported work (the import sets `numbered` from the text
//! form) stays byte-identical.
//!
//! Jump labels (Round 30a, contract §3.3): export, export map, mirror and typesetting carry the fixed jump label
//! `<k{key}>` at the first heading of every section ([`baruch_typst::xref::label_section`]: ` <kN>` at the end of the
//! line or, with a label of its own, the anchor line `#metadata("baruch-key") <kN>` below it). An import removes them
//! again (`import.rs`). A freshly imported work therefore yields the export byte-identical "apart from the jump labels"
//! (`baruch_typst::xref::strip_key_labels`).
//!
//! Index and glossary (Round 31, contract §2.6): export, export map and mirror let every section run through
//! [`baruch_typst::index::Projector::project`] after the jump label (Bible reference marks, first mention) and append
//! the end of the book ([`baruch_typst::index::back_matter`]: index pages, glossary page) behind the last section or
//! behind the `#include` lines of `main.typ`; the mirror additionally gets `glossar.md`. Without R31 settings
//! ([`baruch_typst::index::BackMatter::is_active`]) that is the identity and the end of the book is empty –
//! byte-identical.
//!
//! Show rule through the jump label (Round 22, remainder of R21b): if a node with `numbered = 0` whose heading cannot
//! be marked (such as a title with code like `#emph[…]`) carries the same heading as a numbered node, the show rule
//! through level and title would hit both. Then the rule targets its jump label: `#show <kN>: set heading(numbering:
//! none)` under `// Baruch: Überschriften ohne Nummer über die Sprungmarke` ([`label_rules`], before the block of the
//! fallback, so that `import::split_fallback` keeps finding the latter at the end). That only works if the jump label
//! is at the heading itself (no label of its own); otherwise the limit of Round 14 remains. An import reads the block
//! back with [`split_label_fallback`] (`import.rs`).
//!
//! Images in the mirror (Round 22, „Spiegel setzen“): the section files lie in `abschnitte/`; Typst resolves a relative
//! path in `image("bilder/x.svg")` against the folder of the file that names it. Therefore the mirror writes relative
//! image paths in section files as paths from the root (`image("/bilder/x.svg")`, [`root_image_paths`]); that way
//! `main.typ` together with `abschnitte/` can be typeset with Typst. Raster images lie in the mirror as Base64 text
//! (`bilder/<name>.b64`, `assets::mirror_files`) – they have to be converted back before typesetting (documented
//! deviation).
//!
//! The mirror is a projection: the revisions in SQLite stay without the import line and without the conversion of the
//! heading, and [`export`] assembles the file without the import line. Without a work template (sample work) there is
//! neither `vorlage.typ` nor the import line.

use std::borrow::Cow;
use std::collections::HashMap;

use sqlx::SqliteConnection;

use baruch_typst::scheme::{Numbering, Scheme, export_template};
use baruch_typst::typeset::TypesetSettings;

use crate::{MirrorFile, StoreError, WorkId, WorkLanguage, broken, load, slug, to_usize};

/// Current revision of a section in tree order ([`ordered_currents`]).
pub(crate) struct Current {
    pub(crate) seq: usize,
    /// Fixed key (`node.id`).
    pub(crate) key: i64,
    pub(crate) title: String,
    pub(crate) body: String,
}

/// Current revisions in tree order. Nodes without a current revision are missing. Unknown work → `UnknownWork`.
pub(crate) async fn ordered_currents(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<Current>, StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    if exists.is_none() {
        return Err(StoreError::UnknownWork(work));
    }

    let rows: Vec<(i64, Option<i64>, i64, String)> =
        sqlx::query_as("SELECT seq, parent_seq, position, title FROM node WHERE work_id = ? ORDER BY seq")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?;
    let currents: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT n.seq, n.id, r.body FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND r.status = 'current' ORDER BY n.seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut bodies: HashMap<usize, (i64, String)> = HashMap::new();
    for (seq, key, body) in currents {
        if bodies.insert(to_usize(seq, "seq")?, (key, body)).is_some() {
            return Err(broken(format!("Abschnitt {seq}: mehr als eine aktuelle Revision")));
        }
    }

    // Children per parent, by (position, seq).
    let mut children: HashMap<Option<usize>, Vec<(i64, usize)>> = HashMap::new();
    let mut titles: HashMap<usize, String> = HashMap::new();
    for (seq, parent, position, node_title) in rows {
        let seq = to_usize(seq, "seq")?;
        let parent = parent.map(|p| to_usize(p, "parent_seq")).transpose()?;
        children.entry(parent).or_default().push((position, seq));
        titles.insert(seq, node_title);
    }
    for list in children.values_mut() {
        list.sort_unstable();
    }

    let mut order = Vec::with_capacity(titles.len());
    let mut stack: Vec<usize> =
        children.get(&None).map(|c| c.iter().rev().map(|&(_, s)| s).collect()).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    while let Some(seq) = stack.pop() {
        if !seen.insert(seq) {
            continue; // loop in parent_seq: do not take it twice
        }
        order.push(seq);
        if let Some(list) = children.get(&Some(seq)) {
            stack.extend(list.iter().rev().map(|&(_, s)| s));
        }
    }
    // Nodes whose parent is missing are not lost: at the end, by seq.
    let mut rest: Vec<usize> = titles.keys().copied().filter(|s| !seen.contains(s)).collect();
    rest.sort_unstable();
    order.extend(rest);

    let mut out = Vec::with_capacity(bodies.len());
    for seq in order {
        let Some((key, body)) = bodies.remove(&seq) else { continue };
        let title = titles.remove(&seq).unwrap_or_default();
        out.push(Current { seq, key, title, body });
    }
    Ok(out)
}

/// Head of the work: stored template, title, the template for export and `main.typ` (with the rule of the outline
/// scheme, unchanged for "from the work template", behind it the show rules of the fallback) and the converted texts
/// per key ([`project`]).
struct Head {
    template: String,
    title: String,
    exported: String,
    bodies: HashMap<i64, String>,
}

/// Work row for head and projection: template, title, custom pattern, scheme id, work language (Round 39b).
type WorkHead = (String, String, Option<String>, String, WorkLanguage);

async fn work_head(conn: &mut SqliteConnection, work: WorkId) -> Result<WorkHead, StoreError> {
    let row: (String, String, Option<String>, String, String) =
        sqlx::query_as("SELECT template, title, numbering, numbering_scheme, language FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or(StoreError::UnknownWork(work))?;
    let (template, title, numbering, scheme, language) = row;
    Ok((template, title, numbering, scheme, WorkLanguage::parse(&language).unwrap_or_default()))
}

/// Typesetting settings for the rule block of a work (Round 39b, contract §2.1): the stored settings; an English work
/// without its own `lang` gets `lang: "en"`, so that `#set text(lang: "en")` stands behind the work template like the
/// other typesetting rules. German works: the stored settings unchanged (byte-identical).
pub(crate) fn rule_settings(mut settings: TypesetSettings, language: WorkLanguage) -> TypesetSettings {
    let own = settings.lang.as_deref().map(str::trim).is_some_and(|l| !l.is_empty());
    if language == WorkLanguage::En && !own {
        settings.lang = Some(language.as_str().to_owned());
    }
    settings
}

async fn head(conn: &mut SqliteConnection, work: WorkId) -> Result<Head, StoreError> {
    let (template, title, numbering, scheme, language) = work_head(conn, work).await?;
    let numbering = load::work_numbering(&scheme, numbering.as_deref(), &template, language);
    let scheme = Scheme::from_id(&scheme).unwrap_or(Scheme::Template);
    let mut exported = export_template(&template, scheme, &numbering);
    let projection = project(&heading_rows(conn, work).await?, &numbering);
    // Round 22: show rules through the jump label (before the block of the fallback, see the module header).
    let labelled = label_rules(&projection.by_label);
    if !labelled.is_empty() {
        if !exported.is_empty() && !exported.ends_with('\n') {
            exported.push('\n');
        }
        exported.push_str(&labelled);
    }
    let rules = fallback_rules(&projection.fallback);
    if !rules.is_empty() {
        if !exported.is_empty() && !exported.ends_with('\n') {
            exported.push('\n');
        }
        exported.push_str(&rules);
    }
    // Round 29: typesetting settings as Typst rules behind the work template, the scheme rules and the fallback rules –
    // the one place for export, export map, mirror (main.typ), typesetting preview, PDF and cover. Without settings ""
    // (byte-identical).
    let typeset = rule_settings(crate::typeset::typeset_in(conn, work).await?, language);
    let satz = baruch_typst::typeset::rules(&typeset);
    if !satz.is_empty() {
        if !exported.is_empty() && !exported.ends_with('\n') {
            exported.push('\n');
        }
        exported.push_str(&satz);
    }
    Ok(Head { template, title, exported, bodies: projection.bodies })
}

/// Show rules (fallback since Round 14) for headings of the nodes with `numbered = 0` that cannot be marked as "without
/// number" (label of their own): Baruch does not count them ([`load::numbers`]), Typst would otherwise number them
/// along and shift all following numbers. A heading with `numbering: none` does not count in Typst (not in
/// `counter(heading).final()` either, which „Predigt“ reads for „Schluss“). It is recognised by level and title
/// (`heading.where(level: …, body: […])`, title verbatim from the heading line, without label and comment). Without
/// such nodes: empty.
fn fallback_rules(keys: &[(u8, String)]) -> String {
    let rules: String = keys
        .iter()
        .map(|(level, title)| {
            format!("#show heading.where(level: {level}, body: [{title}]): set heading(numbering: none)\n")
        })
        .collect();
    if rules.is_empty() {
        return rules;
    }
    format!("// Baruch: Überschriften ohne Nummer\n{rules}")
}

/// Marker line of the show rules through the jump label (Round 22).
pub(crate) const LABEL_FALLBACK_MARK: &str = "// Baruch: Überschriften ohne Nummer über die Sprungmarke";

/// Show rules through the jump label `<kN>` (Round 22) for nodes with `numbered = 0` whose heading is the same as that
/// of a numbered node (the rule through level and title would hit both). Without such nodes: empty.
fn label_rules(keys: &[i64]) -> String {
    if keys.is_empty() {
        return String::new();
    }
    let rules: String = keys.iter().map(|key| format!("#show <k{key}>: set heading(numbering: none)\n")).collect();
    format!("{LABEL_FALLBACK_MARK}\n{rules}")
}

/// Counterpart of [`label_rules`] in the import (Round 22): if the block [`LABEL_FALLBACK_MARK`] is in the template of
/// an export and consists only of rules `#show <kN>: set heading(numbering: none)`, it comes out of the template; what
/// comes back are the old keys `N` (their headings are "without number"). If the block of the fallback stands behind
/// it, that one stays. A line of another form → template unchanged, no keys.
pub(crate) fn split_label_fallback(template: &str) -> (String, Vec<i64>) {
    let Some(at) = template.find(&format!("{LABEL_FALLBACK_MARK}\n")) else { return (template.to_owned(), Vec::new()) };
    if at > 0 && !template[..at].ends_with('\n') {
        return (template.to_owned(), Vec::new());
    }
    let block_start = at + LABEL_FALLBACK_MARK.len() + 1;
    let mut keys = Vec::new();
    let mut end = block_start;
    for line in template[block_start..].split_inclusive('\n') {
        let Some(key) = line
            .trim_end_matches('\n')
            .strip_prefix("#show <k")
            .and_then(|rest| rest.strip_suffix(">: set heading(numbering: none)"))
            .and_then(|key| key.parse::<i64>().ok())
        else {
            break;
        };
        keys.push(key);
        end += line.len();
    }
    if keys.is_empty() {
        return (template.to_owned(), Vec::new());
    }
    (format!("{}{}", &template[..at], &template[end..]), keys)
}

/// Relative image paths of a section file of the mirror from the root (Round 22, see the module header): `image("p")`
/// with a path without a leading `/`, without `..` at the beginning, without a scheme and without `"`/`\\` becomes
/// `image("/p")`. Other calls (for instance through a function of the work template) stay as they are.
pub(crate) fn root_image_paths(body: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(body);
    for path in baruch_typst::images::image_paths(body) {
        if path.is_empty()
            || path.starts_with('/')
            || path.starts_with("..")
            || path.contains("://")
            || path.contains(['"', '\\'])
        {
            continue;
        }
        let from = format!("image(\"{path}\"");
        if out.contains(&from) {
            out = Cow::Owned(out.replace(&from, &format!("image(\"/{path}\"")));
        }
    }
    out
}

/// Per node except parts (tree order, only with a current revision): key, `numbered` and the text of the current
/// revision.
type HeadingRow = (i64, bool, String);

async fn heading_rows(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<HeadingRow>, StoreError> {
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT n.id, n.numbered, r.body FROM node n JOIN revision r ON r.node_id = n.id AND r.status = 'current' \
         WHERE n.work_id = ? AND n.kind <> 'part' ORDER BY n.seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows.into_iter().map(|(key, flag, body)| (key, flag != 0, body)).collect())
}

/// Result of [`project`].
struct Projection {
    /// Converted texts per key (only the changed ones).
    bodies: HashMap<i64, String>,
    /// Headings `(level, title verbatim)` for the show rules of the fallback ([`fallback_rules`]).
    fallback: Vec<(u8, String)>,
    /// Round 22: keys of the nodes with a show rule through their jump label ([`label_rules`]).
    by_label: Vec<i64>,
    /// Keys of the nodes whose first heading (level 1) Typst counts after the projection (numbered text form, no show
    /// rule).
    counted: Vec<i64>,
}

/// Projection "number on/off" (contract Round 14, section 4): if `numbering` is not off, the first heading line of
/// every node whose `numbered` does not fit the text form is converted with [`baruch_typst::set_numbered_line`]. If it
/// cannot be marked for `numbered = 0` (label of its own, heading not at the beginning), it gets a show rule – unless a
/// node with a number carries the same heading (the rule would hit both) or the title is empty.
///
/// If the numbering is off („Ohne Nummern“ or `numbering: none`), the text form stays (text as stored); since Round
/// 21b, however, nodes with `numbered = 0` then get the show rule of the fallback. In the typesetting it changes
/// nothing (there are no numbers anyway), but that way the flag "number off" survives export → import
/// (`import::split_fallback`); before, such a section came back as numbered.
fn project(rows: &[HeadingRow], numbering: &Numbering) -> Projection {
    // Per row: key, `numbered`, first heading (level, title verbatim) and whether Typst numbers it after the
    // projection.
    type Shown = (i64, bool, Option<(u8, String)>, bool);
    let mut shown: Vec<Shown> = Vec::with_capacity(rows.len());
    let mut bodies = HashMap::new();
    // Round 22: is the jump label at the heading itself (` <kN>` at the end of the line)?
    let mut on_heading: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for (key, flag, body) in rows {
        let marker = format!("<k{key}>");
        if baruch_typst::xref::label_section(body, *key).lines().next().is_some_and(|line| line.contains(&marker)) {
            on_heading.insert(*key);
        }
        let heading = baruch_typst::parse(body)
            .ok()
            .and_then(|doc| doc.headings.into_iter().next())
            .filter(|h| (1..=3).contains(&h.level));
        let Some(heading) = heading else {
            shown.push((*key, *flag, None, false));
            continue;
        };
        let mut form = heading.numbered;
        if !numbering.is_off()
            && form != *flag
            && let Some(projected) = baruch_typst::set_numbered_line(body, *flag)
        {
            bodies.insert(*key, projected);
            form = *flag;
        }
        shown.push((*key, *flag, Some((heading.level, heading.raw_title)), form));
    }
    // Fallback: numbered text form, but `numbered = 0` (since Round 21b also with the numbering switched off).
    let mut fallback: Vec<(u8, String)> = Vec::new();
    let mut taken = std::collections::HashSet::new();
    for (_, flag, heading, form) in &shown {
        let Some(heading) = heading else { continue };
        if *form && *flag {
            taken.insert(heading.clone());
        } else if *form && !fallback.contains(heading) {
            fallback.push(heading.clone());
        }
    }
    // Round 22: the rule through level and title would also hit a numbered node with the same heading (or the title is
    // empty) → rule through the jump label, if it is at the heading.
    let by_label: Vec<i64> = shown
        .iter()
        .filter(|(key, flag, heading, form)| {
            *form
                && !*flag
                && on_heading.contains(key)
                && heading.as_ref().is_some_and(|h| taken.contains(h) || h.1.is_empty())
        })
        .map(|row| row.0)
        .collect();
    fallback.retain(|key| !taken.contains(key) && !key.1.is_empty());
    let counted = shown
        .iter()
        .filter(|(key, _, heading, form)| {
            *form && !by_label.contains(key) && heading.as_ref().is_some_and(|h| h.0 == 1 && !fallback.contains(h))
        })
        .map(|row| row.0)
        .collect();
    Projection { bodies, fallback, by_label, counted }
}

/// Keys of the nodes (tree order) whose heading Typst counts in `counter(heading).get().first()`: headings of the first
/// level that are numbered after the projection ([`project`]) (Typst counts only numbered headings; markers "without
/// number" and headings with a show rule do not count). With "from the work template" without numbers in the template
/// (`none` or without any rule) none; if its rule cannot be read reliably, `None` (do not guess). With „Ohne Nummern“
/// none.
pub(crate) async fn counted_headings(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<Option<Vec<i64>>, StoreError> {
    let (template, _, custom, scheme, language) = work_head(conn, work).await?;
    let numbering = load::work_numbering(&scheme, custom.as_deref(), &template, language);
    let counted = project(&heading_rows(conn, work).await?, &numbering).counted;
    Ok(match Scheme::from_id(&scheme).unwrap_or(Scheme::Template) {
        Scheme::Template => match baruch_typst::heading_numbering(&template) {
            Some(pattern) if pattern.is_empty() => Some(Vec::new()),
            Some(_) => Some(counted),
            // No readable rule: without any mention the template does not number, otherwise unclear.
            None if template.contains("numbering") => None,
            None => Some(Vec::new()),
        },
        Scheme::None => Some(Vec::new()),
        // A custom pattern without counting-symbol levels (empty) numbers nothing.
        _ if numbering.is_off() => Some(Vec::new()),
        _ => Some(counted),
    })
}

/// Name of the template file in the mirror (only for works with a work template).
pub(crate) const TEMPLATE_FILE: &str = "vorlage.typ";

/// First line of every section file of a work with a work template (sections lie in `abschnitte/`).
pub(crate) const TEMPLATE_IMPORT: &str = "#import \"../vorlage.typ\": *\n";

/// Files of the mirror: `main.typ`, with a work template `vorlage.typ`, then the sections. With a work template
/// `main.typ` begins with it, otherwise with `// Baruch: <title>`.
pub(crate) async fn files(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<MirrorFile>, StoreError> {
    let Head { template, title, exported, mut bodies } = head(conn, work).await?;
    let has_template = !template.is_empty();
    let mut main = if has_template { exported } else { format!("// Baruch: {title}\n{exported}\n") };
    let currents = ordered_currents(conn, work).await?;
    if !currents.is_empty() {
        end_line(&mut main);
    }
    let back = back_matter_in(conn, work).await?;
    let mut projector = baruch_typst::index::Projector::new(&back);
    let mut projected = Vec::with_capacity(currents.len());
    let mut sections = Vec::new();
    for Current { seq, key, title: node_title, body } in currents {
        let body = bodies.remove(&key).unwrap_or(body);
        // Jump label `<kN>` (Round 30a, [`baruch_typst::xref::label_section`]): only in the projection.
        let body = baruch_typst::xref::label_section(&body, key).into_owned();
        // Index and glossary (Round 31): only in the projection.
        let body = projector.project(&body).into_owned();
        let path = format!("abschnitte/{seq:03}-{}.typ", slug(&node_title));
        main.push_str(&format!("#include \"{path}\"\n"));
        projected.push(body.clone());
        // Round 22: relative image paths from the root, so that Typst finds them from `abschnitte/`.
        let body = root_image_paths(&body).into_owned();
        let contents = if has_template { format!("{TEMPLATE_IMPORT}{body}") } else { body };
        sections.push(MirrorFile { path, contents });
    }
    // End of the book (Round 31) behind the `#include` lines; inactive "".
    let tail = baruch_typst::index::back_matter(&back, &projected.iter().map(String::as_str).collect::<Vec<_>>());
    append_tail(&mut main, &tail);
    let mut files = vec![MirrorFile { path: "main.typ".into(), contents: main }];
    if has_template {
        files.push(MirrorFile { path: TEMPLATE_FILE.into(), contents: template });
    }
    files.extend(sections);
    files.extend(crate::assets::mirror_files(conn, work).await?);
    files.extend(crate::ideas::mirror_files(conn, work).await?);
    files.extend(crate::findings::mirror_files(conn, work).await?);
    files.extend(crate::glossary::mirror_files(conn, work).await?);
    Ok(files)
}

/// The work as one Typst file: work template and current texts in tree order. If a text (except the last one) does not
/// end with a line ending, one is added, so that the next heading stands at the start of a line. A freshly imported
/// work thus yields the input byte for byte.
pub(crate) async fn export(conn: &mut SqliteConnection, work: WorkId) -> Result<String, StoreError> {
    let (template, sections, tail) = export_parts(conn, work).await?;
    let mut out = baruch_typst::join(&template, sections.iter().map(|s| s.body.as_str()));
    append_tail(&mut out, &tail);
    Ok(out)
}

/// Appends the end of the book (Round 31, [`baruch_typst::index::back_matter`]): empty → nothing (byte-identical),
/// otherwise first a line ending if the text before it (not empty) has none, so that `// Baruch: Register und Glossar`
/// is a line of its own. Export and export map both use this function (invariant: map byte-identical to the export).
pub(crate) fn append_tail(out: &mut String, tail: &str) {
    if tail.is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with(baruch_typst::is_newline) {
        out.push('\n');
    }
    out.push_str(tail);
}

/// What projection and end of the book of a work need (Round 31, contract §2.6): typesetting settings, built-in and
/// custom indexes, accepted glossary terms.
pub(crate) async fn back_matter_in(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<baruch_typst::index::BackMatter, StoreError> {
    let settings = crate::typeset::typeset_in(conn, work).await?;
    let mut registers = baruch_typst::index::builtin_registers();
    registers.extend(crate::glossary::registers_in(conn, work).await?);
    let glossary = crate::glossary::accepted_entries_in(conn, work).await?;
    let (.., lang) = work_head(conn, work).await?;
    Ok(baruch_typst::index::BackMatter { settings, registers, glossary, lang })
}

/// A non-empty template that is still followed by something ends in a line ending: otherwise the first heading (or
/// `#include`) would hang on its last line – `= Titel` would become running text, and an error in the last template
/// line would count in the preview as an error of the first section. The stored template (and `vorlage.typ`) stays as
/// the human wrote it. A mere byte-order mark (import of a file that begins with a BOM and a heading) is not a line and
/// stays without one.
fn end_line(template: &mut String) {
    let content = template.strip_prefix('\u{feff}').unwrap_or(template);
    if !content.is_empty() && !content.ends_with(baruch_typst::is_newline) {
        template.push('\n');
    }
}

/// Parts of [`export`]: the exported template (if a text follows, with a line ending at the end, see [`end_line`]) and
/// the sections in tree order, their texts already with the added line ending. [`export`] concatenates them;
/// `export_map` (Typst preview, Round 8) concatenates them with byte ranges ([`baruch_typst::join_with_offsets`]) – so
/// both from the same source, so that the map stays byte-identical to the export. Third part (Round 31): the end of the
/// book (index and glossary pages, inactive ""), which both append behind the last section with [`append_tail`].
pub(crate) async fn export_parts(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<(String, Vec<Current>, String), StoreError> {
    let Head { exported: mut template, mut bodies, .. } = head(conn, work).await?;
    let mut sections = ordered_currents(conn, work).await?;
    let back = back_matter_in(conn, work).await?;
    let mut projector = baruch_typst::index::Projector::new(&back);
    // Projection "number on/off" ([`project`]); the texts in SQLite stay as they are.
    for section in &mut sections {
        if let Some(projected) = bodies.remove(&section.key) {
            section.body = projected;
        }
        // Jump label `<kN>` at the heading (Round 30a): export, map and typesetting carry it, SQLite does not.
        if let Cow::Owned(labelled) = baruch_typst::xref::label_section(&section.body, section.key) {
            section.body = labelled;
        }
        // Index and glossary (Round 31): Bible reference marks, first mention; only in the projection.
        if let Cow::Owned(projected) = projector.project(&section.body) {
            section.body = projected;
        }
    }
    if sections.iter().any(|s| !s.body.is_empty()) {
        end_line(&mut template);
    }
    let last = sections.len().saturating_sub(1);
    for (i, section) in sections.iter_mut().enumerate() {
        if i < last && !section.body.is_empty() && !section.body.ends_with('\n') {
            section.body.push('\n');
        }
    }
    let tail = baruch_typst::index::back_matter(&back, &sections.iter().map(|s| s.body.as_str()).collect::<Vec<_>>());
    Ok((template, sections, tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbering() -> Numbering {
        load::work_numbering("decimal", None, "", WorkLanguage::De)
    }

    #[test]
    fn a_title_with_code_shared_with_a_numbered_node_gets_a_rule_over_its_jump_label() {
        // Remainder of R21b: node 2 ("without number", title with code, cannot be marked) has the same heading as the
        // numbered node 3. The rule through level and title would hit both – so a rule through <k2>.
        let rows = vec![
            (1, true, "= Anfang\n\nText.\n".to_owned()),
            (2, false, "= #emph[Rast]\n\nPause.\n".to_owned()),
            (3, true, "= #emph[Rast]\n\nWeiter.\n".to_owned()),
        ];
        let projection = project(&rows, &numbering());
        assert_eq!(projection.by_label, vec![2], "Regel über die Sprungmarke für Knoten 2");
        assert!(projection.fallback.is_empty(), "keine Regel über Ebene und Titel (träfe beide)");
        assert_eq!(projection.counted, vec![1, 3], "Typst zählt Knoten 2 nicht mehr mit");
        assert_eq!(
            label_rules(&projection.by_label),
            format!("{LABEL_FALLBACK_MARK}\n#show <k2>: set heading(numbering: none)\n"),
            "eine Regel je Knoten unter der Kennzeile"
        );
        // Without the duplication the rule through level and title remains (export form unchanged).
        let single = project(&rows[..2], &numbering());
        assert!(single.by_label.is_empty() && single.fallback.len() == 1, "Rückfall wie bisher");
    }

    #[test]
    fn the_label_block_reads_back_and_leaves_the_fallback_block_in_place() {
        let template = format!(
            "#set page(paper: \"a5\")\n{LABEL_FALLBACK_MARK}\n#show <k7>: set heading(numbering: none)\n\
             #show <k12>: set heading(numbering: none)\n// Baruch: Überschriften ohne Nummer\n\
             #show heading.where(level: 1, body: [Vorwort]): set heading(numbering: none)\n"
        );
        let (rest, keys) = split_label_fallback(&template);
        assert_eq!(keys, vec![7, 12], "alte Schlüssel der Überschriften ohne Nummer");
        assert_eq!(
            rest,
            "#set page(paper: \"a5\")\n// Baruch: Überschriften ohne Nummer\n\
             #show heading.where(level: 1, body: [Vorwort]): set heading(numbering: none)\n",
            "der Block verschwindet, der Rückfall bleibt für split_fallback"
        );
        let foreign = format!("{LABEL_FALLBACK_MARK}\n#show <k7>: set text(red)\n");
        assert_eq!(split_label_fallback(&foreign), (foreign.clone(), Vec::new()), "fremde Form bleibt in der Vorlage");
    }

    #[test]
    fn relative_image_paths_point_from_the_root_in_section_files() {
        let body = "= Hof\n\n#image(\"bilder/a.svg\")\n#figure(image(\"./bilder/b.png\", width: 2cm))\n\
                    #image(\"/bilder/c.svg\")\n#image(\"../draussen.svg\")\n#bild(\"bilder/d.svg\")\n";
        assert_eq!(
            root_image_paths(body),
            "= Hof\n\n#image(\"/bilder/a.svg\")\n#figure(image(\"/./bilder/b.png\", width: 2cm))\n\
             #image(\"/bilder/c.svg\")\n#image(\"../draussen.svg\")\n#bild(\"bilder/d.svg\")\n",
            "nur relative image(\"…\")-Pfade, absolute und ../ bleiben, Funktionen der Vorlage auch"
        );
        assert!(matches!(root_image_paths("= Ohne Bild\n"), Cow::Borrowed(_)), "ohne Bild unverändert geliehen");
    }
}
