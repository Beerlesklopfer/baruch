// SPDX-License-Identifier: AGPL-3.0-or-later
//! Glossary and index of a work (Round 31, contract §2.5; interface R31-S1, types and signatures **frozen** – changes
//! only through integrator/PM; the bodies belong to the task glossar-daten).
//!
//! - **Glossary terms** (`glossary_term`, migration 0049): accepted (`accepted`, by the human) or proposed (`proposed`,
//!   by a persona or an MCP client, `origin` = "@lektor" or "@mcp:<name>"). Glossary data are not book text, but work
//!   content: only the human changes and deletes, every change is a mirror commit.
//! - **Custom indexes** (`index_register`): at most [`baruch_typst::index::MAX_CUSTOM_REGISTERS`], the built-in
//!   identifiers are blocked.
//! - **Index proposals** (`index_proposal`): a proposal "put this mark behind this word"; if the human accepts it, a
//!   new revision of the section with the mark arises in one transaction ([`baruch_typst::index::mark_word`]).
//! - Reading sorts by [`baruch_typst::index::sort_key`] (in Rust, never stored).

use std::collections::{BTreeMap, HashSet};

use baruch_typst::index::{
    GlossaryEntry, IndexError, IndexMark, MAX_CUSTOM_REGISTERS, MAX_REGISTER_TITLE_CHARS, Occurrence, Register,
    RegisterKind, builtin_registers, mark_word, sort_key, term_key, valid_register_slug,
};
use sqlx::SqliteConnection;

use crate::{Store, StoreError, WorkId, broken, to_u32, to_usize};

pub type GlossaryTermId = i64;
pub type IndexProposalId = i64;
pub const MAX_GLOSSARY_TERM_CHARS: usize = 200;
pub const MAX_GLOSSARY_SHORT_CHARS: usize = 500;
pub const MAX_GLOSSARY_LONG_CHARS: usize = 10_000;
pub const MAX_GLOSSARY_SOURCE_CHARS: usize = 500;
pub const MAX_GLOSSARY_VARIANTS: usize = 32;
pub const MAX_GLOSSARY_BIBLE_REFS: usize = 32;
pub const MAX_GLOSSARY_TERMS: usize = 5000;
pub const MAX_INDEX_REASON_CHARS: usize = 1000;
/// Round 23: at most this many **open** index proposals per work and origin (persona or `@mcp:<Client>`).
pub const MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN: usize = 200;
/// Round 23: at most this many **open** index proposals per work in total.
pub const MAX_OPEN_INDEX_PROPOSALS_PER_WORK: usize = 2000;

/// Longest form of a word or of a variant (characters, like a term).
const MAX_FORM_CHARS: usize = MAX_GLOSSARY_TERM_CHARS;
/// Longest Bible reference (characters); the server checks and normalises the references with baruch_scripture.
const MAX_BIBLE_REF_CHARS: usize = 100;
/// Longest origin (name of the human or handle; CHECK of migration 0049).
const MAX_ORIGIN_CHARS: usize = 200;

/// Status of a glossary term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryStatus {
    Accepted,
    Proposed,
}

impl GlossaryStatus {
    /// Value of the column `glossary_term.status`.
    pub fn as_db(self) -> &'static str {
        match self {
            GlossaryStatus::Accepted => "accepted",
            GlossaryStatus::Proposed => "proposed",
        }
    }

    /// Reverses [`GlossaryStatus::as_db`]; anything unknown → `None`.
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "accepted" => Some(GlossaryStatus::Accepted),
            "proposed" => Some(GlossaryStatus::Proposed),
            _ => None,
        }
    }
}

/// A glossary term with all fields.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GlossaryTerm {
    pub id: GlossaryTermId,
    pub work: WorkId,
    pub uuid: String,
    pub term: String,
    pub short: String,
    pub long: String,
    pub variants: Vec<String>,
    pub source: String,
    pub bible_refs: Vec<String>,
    pub status: GlossaryStatus,
    pub origin: String,
    pub created: String,
    pub updated: Option<String>,
    pub updated_by: String,
}

impl GlossaryTerm {
    /// The entry for typesetting and EPUB ([`baruch_typst::index::BackMatter::glossary`]).
    pub fn entry(&self) -> GlossaryEntry {
        GlossaryEntry {
            uuid: self.uuid.clone(),
            term: self.term.clone(),
            variants: self.variants.clone(),
            short: self.short.clone(),
            long: self.long.clone(),
            bible_refs: self.bible_refs.clone(),
        }
    }
}

/// Input for creating, changing and proposing a term.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GlossaryEdit {
    pub term: String,
    pub short: String,
    pub long: String,
    pub variants: Vec<String>,
    pub source: String,
    pub bible_refs: Vec<String>,
}

/// What happened to a term (mirror commit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryAction {
    Added,
    Updated,
    Deleted,
    Proposed,
    Accepted,
    Rejected,
}

/// Result of a glossary change that the server logs in the mirror.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GlossaryChange {
    pub work: WorkId,
    pub action: GlossaryAction,
    pub before: Option<GlossaryTerm>,
    pub after: Option<GlossaryTerm>,
    pub by: String,
}

/// A custom index of a work.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IndexRegisterRow {
    pub id: i64,
    pub uuid: String,
    pub slug: String,
    pub title: String,
    pub position: i64,
    pub created: String,
}

/// Status of an index proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexProposalStatus {
    Open,
    Accepted,
    Rejected,
    Stale,
}

/// Input of an index proposal (section through the fixed key `key`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewIndexProposal {
    pub key: i64,
    pub word: String,
    pub occurrence: Occurrence,
    pub mark: IndexMark,
    pub reason: String,
    pub origin: String,
}

/// An index proposal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IndexProposal {
    pub id: IndexProposalId,
    pub work: WorkId,
    pub key: i64,
    pub based_on: u32,
    pub word: String,
    pub occurrence: Occurrence,
    pub mark: IndexMark,
    pub reason: String,
    pub origin: String,
    pub status: IndexProposalStatus,
    pub created: String,
    pub decided: Option<String>,
    pub decided_by: Option<String>,
}

/// Result of an acceptance: proposal, new revision by the human and number of marks set.
#[derive(Debug, Clone)]
pub struct IndexAccepted {
    pub proposal: IndexProposal,
    pub change: crate::Change,
    pub marks: usize,
}

// ---- Checking and preparing ---------------------------------------------------------------------------------------

/// Text on one line: whitespace (also line breaks) to one space, trimmed.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn glossary_error(message: impl Into<String>) -> StoreError {
    StoreError::Glossary(message.into())
}

/// Check the maximum length in characters; `label` names the field in the message.
fn check_len(label: &str, value: &str, max: usize) -> Result<(), StoreError> {
    let count = value.chars().count();
    if count > max {
        return Err(glossary_error(format!("{label}: höchstens {max} Zeichen (jetzt {count}).")));
    }
    Ok(())
}

/// A term after preparing: term and variants on one line, explanations trimmed (line breaks in the detailed explanation
/// stay), empty variants and Bible references removed, duplicates removed (variants through their key, also a variant
/// equal to the term).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Clean {
    pub(crate) term: String,
    pub(crate) key: String,
    pub(crate) short: String,
    pub(crate) long: String,
    pub(crate) variants: Vec<(String, String)>,
    pub(crate) source: String,
    pub(crate) bible_refs: Vec<String>,
}

pub(crate) fn clean(edit: &GlossaryEdit) -> Result<Clean, StoreError> {
    let term = one_line(&edit.term);
    if term.is_empty() {
        return Err(glossary_error("Begriff: darf nicht leer sein."));
    }
    check_len("Begriff", &term, MAX_GLOSSARY_TERM_CHARS)?;
    let short = one_line(&edit.short);
    check_len("Kurze Erklärung", &short, MAX_GLOSSARY_SHORT_CHARS)?;
    let long = edit.long.replace("\r\n", "\n").trim().to_owned();
    check_len("Ausführliche Erklärung", &long, MAX_GLOSSARY_LONG_CHARS)?;
    let source = one_line(&edit.source);
    check_len("Quelle", &source, MAX_GLOSSARY_SOURCE_CHARS)?;
    let key = term_key(&term);
    let mut seen: HashSet<String> = HashSet::from([key.clone()]);
    let mut variants = Vec::new();
    for raw in &edit.variants {
        let form = one_line(raw);
        if form.is_empty() {
            continue;
        }
        check_len("Variante", &form, MAX_FORM_CHARS)?;
        let form_key = term_key(&form);
        if seen.insert(form_key.clone()) {
            variants.push((form, form_key));
        }
    }
    if variants.len() > MAX_GLOSSARY_VARIANTS {
        return Err(glossary_error(format!(
            "Varianten: höchstens {MAX_GLOSSARY_VARIANTS} je Begriff (jetzt {}).",
            variants.len()
        )));
    }
    let mut bible_refs: Vec<String> = Vec::new();
    for raw in &edit.bible_refs {
        let reference = one_line(raw);
        if reference.is_empty() || bible_refs.contains(&reference) {
            continue;
        }
        check_len("Bibelstelle", &reference, MAX_BIBLE_REF_CHARS)?;
        bible_refs.push(reference);
    }
    if bible_refs.len() > MAX_GLOSSARY_BIBLE_REFS {
        return Err(glossary_error(format!(
            "Bibelstellen: höchstens {MAX_GLOSSARY_BIBLE_REFS} je Begriff (jetzt {}).",
            bible_refs.len()
        )));
    }
    Ok(Clean { term, key, short, long, variants, source, bible_refs })
}

/// Origin or name of the human on one line, at most 200 characters; empty → error.
fn actor(name: &str, what: &str) -> Result<String, StoreError> {
    let name: String = one_line(name).chars().take(MAX_ORIGIN_CHARS).collect();
    if name.is_empty() {
        return Err(glossary_error(format!("{what}: fehlt.")));
    }
    Ok(name)
}

// ---- Reading -------------------------------------------------------------------------------------------------------

async fn ensure_work(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

type TermRow =
    (i64, Option<String>, String, String, String, String, String, String, String, String, Option<String>, String);

const TERM_COLUMNS: &str =
    "id, uuid, term, short, long, source, bible_refs, status, origin, created, updated, updated_by";

fn term_of(work: WorkId, row: TermRow, variants: Vec<String>) -> Result<GlossaryTerm, StoreError> {
    let (id, uuid, term, short, long, source, refs, status, origin, created, updated, updated_by) = row;
    let bible_refs: Vec<String> = serde_json::from_str(&refs)
        .map_err(|e| broken(format!("Glossarbegriff {id}: Bibelstellen nicht lesbar: {e}")))?;
    let status =
        GlossaryStatus::from_db(&status).ok_or_else(|| broken(format!("Glossarbegriff {id}: Status „{status}“")))?;
    let uuid = uuid.ok_or_else(|| broken(format!("Glossarbegriff {id} ohne uuid")))?;
    Ok(GlossaryTerm {
        id,
        work,
        uuid,
        term,
        short,
        long,
        variants,
        source,
        bible_refs,
        status,
        origin,
        created,
        updated,
        updated_by,
    })
}

/// Sorted by [`sort_key`] (in Rust, never stored), on a tie by term and id.
fn sort_terms(terms: &mut [GlossaryTerm]) {
    terms.sort_by_cached_key(|t| (sort_key(&t.term), t.term.clone(), t.id));
}

/// Terms of the work that satisfy `filter` (SQL behind `WHERE work_id = ?`, without further parameters), with variants.
async fn terms_where(conn: &mut SqliteConnection, work: WorkId, filter: &str) -> Result<Vec<GlossaryTerm>, StoreError> {
    let rows: Vec<TermRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {TERM_COLUMNS} FROM glossary_term WHERE work_id = ? {filter}"
    )))
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let variant_rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT v.term_id, v.form FROM glossary_variant v JOIN glossary_term t ON t.id = v.term_id \
         WHERE t.work_id = ? ORDER BY v.term_id, v.position",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut variants: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for (term, form) in variant_rows {
        variants.entry(term).or_default().push(form);
    }
    let mut terms = rows
        .into_iter()
        .map(|row| {
            let forms = variants.remove(&row.0).unwrap_or_default();
            term_of(work, row, forms)
        })
        .collect::<Result<Vec<_>, _>>()?;
    sort_terms(&mut terms);
    Ok(terms)
}

async fn term_in(conn: &mut SqliteConnection, work: WorkId, id: GlossaryTermId) -> Result<GlossaryTerm, StoreError> {
    let row: Option<TermRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {TERM_COLUMNS} FROM glossary_term WHERE work_id = ? AND id = ?"
    )))
    .bind(work)
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else {
        ensure_work(conn, work).await?;
        return Err(StoreError::UnknownGlossaryTerm { work, id });
    };
    let variants: Vec<String> =
        sqlx::query_scalar("SELECT form FROM glossary_variant WHERE term_id = ? ORDER BY position")
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
    term_of(work, row, variants)
}

/// id of an accepted term with this key (except `except`).
async fn accepted_with_key(
    conn: &mut SqliteConnection,
    work: WorkId,
    key: &str,
    except: Option<GlossaryTermId>,
) -> Result<Option<(i64, String)>, StoreError> {
    Ok(sqlx::query_as(
        "SELECT id, term FROM glossary_term WHERE work_id = ? AND status = 'accepted' AND term_key = ? \
         AND id IS NOT ?",
    )
    .bind(work)
    .bind(key)
    .bind(except)
    .fetch_optional(&mut *conn)
    .await?)
}

// ---- Writing -------------------------------------------------------------------------------------------------------

pub(crate) async fn write_variants(
    conn: &mut SqliteConnection,
    id: GlossaryTermId,
    clean: &Clean,
) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM glossary_variant WHERE term_id = ?").bind(id).execute(&mut *conn).await?;
    for (position, (form, form_key)) in clean.variants.iter().enumerate() {
        sqlx::query("INSERT INTO glossary_variant (term_id, position, form, form_key) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(position as i64)
            .bind(form)
            .bind(form_key)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

async fn insert_term(
    conn: &mut SqliteConnection,
    work: WorkId,
    clean: &Clean,
    status: GlossaryStatus,
    origin: &str,
) -> Result<GlossaryTermId, StoreError> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM glossary_term WHERE work_id = ?")
        .bind(work)
        .fetch_one(&mut *conn)
        .await?;
    if to_usize(count, "Zahl der Begriffe")? >= MAX_GLOSSARY_TERMS {
        return Err(glossary_error(format!(
            "Höchstens {MAX_GLOSSARY_TERMS} Begriffe je Werk (Vorschläge mitgezählt)."
        )));
    }
    let refs = serde_json::to_string(&clean.bible_refs).map_err(|e| broken(format!("Bibelstellen: {e}")))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO glossary_term (work_id, term, term_key, short, long, source, bible_refs, status, origin) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(work)
    .bind(&clean.term)
    .bind(&clean.key)
    .bind(&clean.short)
    .bind(&clean.long)
    .bind(&clean.source)
    .bind(refs)
    .bind(status.as_db())
    .bind(origin)
    .fetch_one(&mut *conn)
    .await?;
    write_variants(conn, id, clean).await?;
    Ok(id)
}

/// Message if an accepted term with the same key is already there.
fn duplicate(term: &str, existing: &str) -> StoreError {
    glossary_error(format!("Den Begriff „{term}“ gibt es schon im Glossar (als „{existing}“)."))
}

// ---- Indexes and index proposals -----------------------------------------------------------------------------------

type RegisterRow = (i64, Option<String>, String, String, i64, String);

fn register_of(row: RegisterRow) -> Result<IndexRegisterRow, StoreError> {
    let (id, uuid, slug, title, position, created) = row;
    let uuid = uuid.ok_or_else(|| broken(format!("Register {slug} ohne uuid")))?;
    Ok(IndexRegisterRow { id, uuid, slug, title, position, created })
}

pub(crate) async fn register_rows(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<Vec<IndexRegisterRow>, StoreError> {
    let rows: Vec<RegisterRow> = sqlx::query_as(
        "SELECT id, uuid, slug, title, position, created FROM index_register WHERE work_id = ? ORDER BY position, id",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    rows.into_iter().map(register_of).collect()
}

async fn register_by_slug(
    conn: &mut SqliteConnection,
    work: WorkId,
    slug: &str,
) -> Result<IndexRegisterRow, StoreError> {
    let row: Option<RegisterRow> = sqlx::query_as(
        "SELECT id, uuid, slug, title, position, created FROM index_register WHERE work_id = ? AND slug = ?",
    )
    .bind(work)
    .bind(slug)
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(row) => register_of(row),
        None => {
            ensure_work(conn, work).await?;
            if is_builtin(slug) {
                Err(builtin_refused(slug))
            } else {
                Err(glossary_error(format!("Ein eigenes Register „{slug}“ gibt es in diesem Werk nicht.")))
            }
        }
    }
}

fn is_builtin(slug: &str) -> bool {
    builtin_registers().iter().any(|r| r.slug == slug)
}

fn builtin_refused(slug: &str) -> StoreError {
    glossary_error(format!(
        "„{slug}“ ist ein eingebautes Register (stichwort, personen, bibel); es lässt sich weder anlegen, umbenennen \
         noch löschen."
    ))
}

/// Title of a custom index: one line, not empty, at most [`MAX_REGISTER_TITLE_CHARS`] characters.
fn register_title(title: &str) -> Result<String, StoreError> {
    let title = one_line(title);
    if title.is_empty() {
        return Err(glossary_error("Titel des Registers: darf nicht leer sein."));
    }
    check_len("Titel des Registers", &title, MAX_REGISTER_TITLE_CHARS)?;
    Ok(title)
}

/// Does the index exist (built-in or custom)?
async fn known_register(conn: &mut SqliteConnection, work: WorkId, slug: &str) -> Result<bool, StoreError> {
    if is_builtin(slug) {
        return Ok(true);
    }
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM index_register WHERE work_id = ? AND slug = ?")
        .bind(work)
        .bind(slug)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(found.is_some())
}

type ProposalRow =
    (i64, i64, i64, String, String, String, String, String, String, String, Option<String>, Option<String>);

const PROPOSAL_COLUMNS: &str =
    "id, node_id, based_on, word, occurrence, mark, reason, origin, status, created, decided, decided_by";

fn proposal_status_db(status: IndexProposalStatus) -> &'static str {
    match status {
        IndexProposalStatus::Open => "open",
        IndexProposalStatus::Accepted => "accepted",
        IndexProposalStatus::Rejected => "rejected",
        IndexProposalStatus::Stale => "stale",
    }
}

fn proposal_status_word(status: IndexProposalStatus) -> &'static str {
    match status {
        IndexProposalStatus::Open => "offen",
        IndexProposalStatus::Accepted => "übernommen",
        IndexProposalStatus::Rejected => "verworfen",
        IndexProposalStatus::Stale => "veraltet",
    }
}

fn proposal_of(work: WorkId, row: ProposalRow) -> Result<IndexProposal, StoreError> {
    let (id, key, based_on, word, occurrence, mark, reason, origin, status, created, decided, decided_by) = row;
    let occurrence = Occurrence::from_param(&occurrence)
        .ok_or_else(|| broken(format!("Registervorschlag {id}: Fundstelle „{occurrence}“")))?;
    let mark: IndexMark =
        serde_json::from_str(&mark).map_err(|e| broken(format!("Registervorschlag {id}: Marke nicht lesbar: {e}")))?;
    let status = match status.as_str() {
        "open" => IndexProposalStatus::Open,
        "accepted" => IndexProposalStatus::Accepted,
        "rejected" => IndexProposalStatus::Rejected,
        "stale" => IndexProposalStatus::Stale,
        other => return Err(broken(format!("Registervorschlag {id}: Status „{other}“"))),
    };
    Ok(IndexProposal {
        id,
        work,
        key,
        based_on: to_u32(based_on, "Fassung des Registervorschlags")?,
        word,
        occurrence,
        mark,
        reason,
        origin,
        status,
        created,
        decided,
        decided_by,
    })
}

async fn proposal_in(
    conn: &mut SqliteConnection,
    work: WorkId,
    id: IndexProposalId,
) -> Result<IndexProposal, StoreError> {
    let row: Option<ProposalRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {PROPOSAL_COLUMNS} FROM index_proposal WHERE work_id = ? AND id = ?"
    )))
    .bind(work)
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(row) => proposal_of(work, row),
        None => {
            ensure_work(conn, work).await?;
            Err(StoreError::UnknownIndexProposal { work, id })
        }
    }
}

/// Open proposal, or an error with its status.
fn still_open(proposal: &IndexProposal) -> Result<(), StoreError> {
    if proposal.status == IndexProposalStatus::Open {
        Ok(())
    } else {
        Err(glossary_error(format!(
            "Registervorschlag {} ist schon entschieden ({}).",
            proposal.id,
            proposal_status_word(proposal.status)
        )))
    }
}

async fn decide_proposal(
    conn: &mut SqliteConnection,
    id: IndexProposalId,
    status: IndexProposalStatus,
    by: &str,
) -> Result<(), StoreError> {
    sqlx::query(
        "UPDATE index_proposal SET status = ?, decided = strftime('%Y-%m-%dT%H:%M:%fZ','now'), decided_by = ? \
         WHERE id = ?",
    )
    .bind(proposal_status_db(status))
    .bind(by)
    .bind(id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Position (`seq`) of a section through its fixed key; gone → `GoneNode`.
async fn seq_of_key(conn: &mut SqliteConnection, work: WorkId, key: i64) -> Result<usize, StoreError> {
    let seq: Option<i64> = sqlx::query_scalar("SELECT seq FROM node WHERE work_id = ? AND id = ?")
        .bind(work)
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?;
    match seq {
        Some(seq) => to_usize(seq, "seq"),
        None => {
            ensure_work(conn, work).await?;
            Err(StoreError::GoneNode { work, key })
        }
    }
}

impl Store {
    /// Terms of the work (`status` `None` = all), sorted by `sort_key`. Unknown work → `UnknownWork`.
    pub async fn glossary(
        &self,
        work: WorkId,
        status: Option<GlossaryStatus>,
    ) -> Result<Vec<GlossaryTerm>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        ensure_work(&mut conn, work).await?;
        let filter = match status {
            Some(s) => format!("AND status = '{}'", s.as_db()),
            None => String::new(),
        };
        terms_where(&mut conn, work, &filter).await
    }

    /// A term; unknown → `UnknownGlossaryTerm`.
    pub async fn glossary_term(&self, work: WorkId, id: GlossaryTermId) -> Result<GlossaryTerm, StoreError> {
        let mut conn = self.pool.acquire().await?;
        term_in(&mut conn, work, id).await
    }

    /// Accepted terms whose `term_key` or a variant (`form_key`) matches `word`.
    pub async fn glossary_lookup(&self, work: WorkId, word: &str) -> Result<Vec<GlossaryTerm>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        ensure_work(&mut conn, work).await?;
        let key = term_key(word);
        if key.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM glossary_term WHERE work_id = ? AND status = 'accepted' AND (term_key = ? OR id IN \
             (SELECT term_id FROM glossary_variant WHERE form_key = ?))",
        )
        .bind(work)
        .bind(&key)
        .bind(&key)
        .fetch_all(&mut *conn)
        .await?;
        let mut terms = Vec::with_capacity(ids.len());
        for id in ids {
            terms.push(term_in(&mut conn, work, id).await?);
        }
        sort_terms(&mut terms);
        Ok(terms)
    }

    /// Creates an accepted term (human); duplicate → `StoreError::Glossary`.
    pub async fn glossary_add(
        &self,
        work: WorkId,
        edit: &GlossaryEdit,
        by: &str,
    ) -> Result<GlossaryChange, StoreError> {
        let clean = clean(edit)?;
        let by = actor(by, "Name des Menschen")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_work(&mut tx, work).await?;
        if let Some((_, existing)) = accepted_with_key(&mut tx, work, &clean.key, None).await? {
            return Err(duplicate(&clean.term, &existing));
        }
        let id = insert_term(&mut tx, work, &clean, GlossaryStatus::Accepted, &by).await?;
        let after = term_in(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(GlossaryChange { work, action: GlossaryAction::Added, before: None, after: Some(after), by })
    }

    /// Changes a term (human). An accepted term must not get the key of another accepted one. `updated`/`updated_by`
    /// are set; status and origin stay.
    pub async fn glossary_update(
        &self,
        work: WorkId,
        id: GlossaryTermId,
        edit: &GlossaryEdit,
        by: &str,
    ) -> Result<GlossaryChange, StoreError> {
        let clean = clean(edit)?;
        let by = actor(by, "Name des Menschen")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = term_in(&mut tx, work, id).await?;
        if before.status == GlossaryStatus::Accepted
            && let Some((_, existing)) = accepted_with_key(&mut tx, work, &clean.key, Some(id)).await?
        {
            return Err(duplicate(&clean.term, &existing));
        }
        let refs = serde_json::to_string(&clean.bible_refs).map_err(|e| broken(format!("Bibelstellen: {e}")))?;
        sqlx::query(
            "UPDATE glossary_term SET term = ?, term_key = ?, short = ?, long = ?, source = ?, bible_refs = ?, \
             updated = strftime('%Y-%m-%dT%H:%M:%fZ','now'), updated_by = ? WHERE id = ?",
        )
        .bind(&clean.term)
        .bind(&clean.key)
        .bind(&clean.short)
        .bind(&clean.long)
        .bind(&clean.source)
        .bind(refs)
        .bind(&by)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        write_variants(&mut tx, id, &clean).await?;
        let after = term_in(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(GlossaryChange { work, action: GlossaryAction::Updated, before: Some(before), after: Some(after), by })
    }

    /// Deletes a term (human, tombstone).
    pub async fn glossary_delete(
        &self,
        work: WorkId,
        id: GlossaryTermId,
        by: &str,
    ) -> Result<GlossaryChange, StoreError> {
        let by = actor(by, "Name des Menschen")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = term_in(&mut tx, work, id).await?;
        sqlx::query("DELETE FROM glossary_term WHERE id = ?").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(GlossaryChange { work, action: GlossaryAction::Deleted, before: Some(before), after: None, by })
    }

    /// Proposes a term (status `proposed`, `origin` = persona or client); already accepted or already proposed (same
    /// key) → error.
    pub async fn glossary_propose(
        &self,
        work: WorkId,
        edit: &GlossaryEdit,
        origin: &str,
    ) -> Result<GlossaryChange, StoreError> {
        let clean = clean(edit)?;
        let origin = actor(origin, "Herkunft des Vorschlags")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_work(&mut tx, work).await?;
        if let Some((_, existing)) = accepted_with_key(&mut tx, work, &clean.key, None).await? {
            return Err(glossary_error(format!("„{}“ steht schon im Glossar (als „{existing}“).", clean.term)));
        }
        let pending: Option<(String, String)> = sqlx::query_as(
            "SELECT term, origin FROM glossary_term WHERE work_id = ? AND status = 'proposed' AND term_key = ?",
        )
        .bind(work)
        .bind(&clean.key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some((term, by)) = pending {
            return Err(glossary_error(format!(
                "„{term}“ ist schon vorgeschlagen (von {by}) und wartet auf den Menschen."
            )));
        }
        let id = insert_term(&mut tx, work, &clean, GlossaryStatus::Proposed, &origin).await?;
        let after = term_in(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(GlossaryChange { work, action: GlossaryAction::Proposed, before: None, after: Some(after), by: origin })
    }

    /// Decides on a proposal (human): accept or reject (deletes it, tombstone).
    pub async fn glossary_decide(
        &self,
        work: WorkId,
        id: GlossaryTermId,
        accept: bool,
        by: &str,
    ) -> Result<GlossaryChange, StoreError> {
        let by = actor(by, "Name des Menschen")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = term_in(&mut tx, work, id).await?;
        if before.status != GlossaryStatus::Proposed {
            return Err(glossary_error(format!(
                "„{}“ ist kein Vorschlag, sondern schon angenommen; nur Vorschläge lassen sich übernehmen oder verwerfen.",
                before.term
            )));
        }
        if !accept {
            sqlx::query("DELETE FROM glossary_term WHERE id = ?").bind(id).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(GlossaryChange {
                work,
                action: GlossaryAction::Rejected,
                before: Some(before),
                after: None,
                by,
            });
        }
        let key = term_key(&before.term);
        if let Some((_, existing)) = accepted_with_key(&mut tx, work, &key, Some(id)).await? {
            return Err(duplicate(&before.term, &existing));
        }
        sqlx::query(
            "UPDATE glossary_term SET status = 'accepted', updated = strftime('%Y-%m-%dT%H:%M:%fZ','now'), \
             updated_by = ? WHERE id = ?",
        )
        .bind(&by)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let after = term_in(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(GlossaryChange { work, action: GlossaryAction::Accepted, before: Some(before), after: Some(after), by })
    }

    /// Custom indexes in their order.
    pub async fn index_registers(&self, work: WorkId) -> Result<Vec<IndexRegisterRow>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        ensure_work(&mut conn, work).await?;
        register_rows(&mut conn, work).await
    }

    /// Creates a custom index; built-in or invalid identifier → `StoreError::Glossary`.
    pub async fn index_register_add(
        &self,
        work: WorkId,
        slug: &str,
        title: &str,
    ) -> Result<IndexRegisterRow, StoreError> {
        let slug = slug.trim();
        if is_builtin(slug) {
            return Err(builtin_refused(slug));
        }
        if !valid_register_slug(slug) {
            return Err(glossary_error(format!(
                "Kennung „{slug}“: Kleinbuchstabe am Anfang, dann Kleinbuchstaben, Ziffern oder Bindestrich, höchstens \
                 32 Zeichen."
            )));
        }
        let title = register_title(title)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_work(&mut tx, work).await?;
        let existing = register_rows(&mut tx, work).await?;
        if existing.iter().any(|r| r.slug == slug) {
            return Err(glossary_error(format!("Ein Register „{slug}“ gibt es in diesem Werk schon.")));
        }
        if existing.len() >= MAX_CUSTOM_REGISTERS {
            return Err(glossary_error(format!("Höchstens {MAX_CUSTOM_REGISTERS} eigene Register je Werk.")));
        }
        let position = existing.iter().map(|r| r.position + 1).max().unwrap_or(0);
        sqlx::query("INSERT INTO index_register (work_id, slug, title, position) VALUES (?, ?, ?, ?)")
            .bind(work)
            .bind(slug)
            .bind(&title)
            .bind(position)
            .execute(&mut *tx)
            .await?;
        let row = register_by_slug(&mut tx, work, slug).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Renames a custom index (the identifier stays).
    pub async fn index_register_rename(
        &self,
        work: WorkId,
        slug: &str,
        title: &str,
    ) -> Result<IndexRegisterRow, StoreError> {
        let slug = slug.trim();
        if is_builtin(slug) {
            return Err(builtin_refused(slug));
        }
        let title = register_title(title)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = register_by_slug(&mut tx, work, slug).await?;
        sqlx::query("UPDATE index_register SET title = ? WHERE id = ?")
            .bind(&title)
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        let row = register_by_slug(&mut tx, work, slug).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Deletes a custom index (tombstone). Marks in the text that name it stay book text (only the human changes them).
    pub async fn index_register_delete(&self, work: WorkId, slug: &str) -> Result<IndexRegisterRow, StoreError> {
        let slug = slug.trim();
        if is_builtin(slug) {
            return Err(builtin_refused(slug));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row = register_by_slug(&mut tx, work, slug).await?;
        sqlx::query("DELETE FROM index_register WHERE id = ?").bind(row.id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Creates an index proposal (status `open`), referring to the current revision of the section. Checked: mark
    /// ([`IndexMark::validate`]), index known (built-in or custom), the word is in the current revision (simple text
    /// comparison; the exact place is only determined by the acceptance), limits.
    pub async fn index_propose(&self, work: WorkId, proposal: &NewIndexProposal) -> Result<IndexProposal, StoreError> {
        proposal.mark.validate().map_err(|e| glossary_error(e.to_string()))?;
        let word = proposal.word.trim();
        if word.is_empty() {
            return Err(glossary_error("Wort: darf nicht leer sein."));
        }
        check_len("Wort", word, MAX_FORM_CHARS)?;
        if word.chars().any(char::is_control) {
            return Err(glossary_error("Wort: ohne Zeilenumbruch und Steuerzeichen."));
        }
        let reason = proposal.reason.trim();
        check_len("Begründung", reason, MAX_INDEX_REASON_CHARS)?;
        let origin = actor(&proposal.origin, "Herkunft des Vorschlags")?;
        if let Occurrence::Nth(0) = proposal.occurrence {
            return Err(glossary_error("Fundstelle: Zählung beginnt bei 1."));
        }
        let mark = serde_json::to_string(&proposal.mark).map_err(|e| broken(format!("Marke: {e}")))?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let seq = seq_of_key(&mut tx, work, proposal.key).await?;
        if !known_register(&mut tx, work, &proposal.mark.register).await? {
            return Err(glossary_error(format!(
                "Register „{}“ gibt es in diesem Werk nicht (eingebaut: stichwort, personen, bibel).",
                proposal.mark.register
            )));
        }
        let node_id = crate::write::node_id(&mut tx, work, seq).await?;
        let Some((based_on, body, _)) = crate::write::current(&mut tx, node_id).await? else {
            return Err(StoreError::Refused("Dieser Abschnitt hat noch keinen Text."));
        };
        if !body.contains(word) {
            return Err(glossary_error(IndexError::WordNotFound(word.to_owned()).to_string()));
        }
        // Round 23: upper limits of open proposals – otherwise a loop of a client fills the list of the human (every
        // proposal is a commit in the mirror). Checked in the same transaction as the creation.
        let (open_own, open_all): (i64, i64) = sqlx::query_as(
            "SELECT coalesce(sum(origin = ?), 0), count(*) FROM index_proposal WHERE work_id = ? AND status = 'open'",
        )
        .bind(&origin)
        .bind(work)
        .fetch_one(&mut *tx)
        .await?;
        if usize::try_from(open_own).unwrap_or(usize::MAX) >= MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN {
            return Err(glossary_error(format!(
                "Zu viele offene Registervorschläge: {origin} hat in diesem Werk schon {open_own}; erlaubt sind \
                 {MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN}. Nichts angelegt; erst entscheidet der Mensch über die offenen."
            )));
        }
        if usize::try_from(open_all).unwrap_or(usize::MAX) >= MAX_OPEN_INDEX_PROPOSALS_PER_WORK {
            return Err(glossary_error(format!(
                "Zu viele offene Registervorschläge in diesem Werk ({open_all}); erlaubt sind \
                 {MAX_OPEN_INDEX_PROPOSALS_PER_WORK}. Nichts angelegt; erst entscheidet der Mensch über die offenen."
            )));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO index_proposal (work_id, node_id, based_on, word, occurrence, mark, reason, origin, status) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'open') RETURNING id",
        )
        .bind(work)
        .bind(node_id)
        .bind(i64::from(based_on))
        .bind(word)
        .bind(proposal.occurrence.as_param())
        .bind(mark)
        .bind(reason)
        .bind(&origin)
        .fetch_one(&mut *tx)
        .await?;
        let created = proposal_in(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(created)
    }

    /// Index proposals (`status` `None` = all), by id.
    pub async fn index_proposals(
        &self,
        work: WorkId,
        status: Option<IndexProposalStatus>,
    ) -> Result<Vec<IndexProposal>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        ensure_work(&mut conn, work).await?;
        let rows: Vec<ProposalRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {PROPOSAL_COLUMNS} FROM index_proposal WHERE work_id = ? AND (? IS NULL OR status = ?) ORDER BY id"
        )))
        .bind(work)
        .bind(status.map(proposal_status_db))
        .bind(status.map(proposal_status_db))
        .fetch_all(&mut *conn)
        .await?;
        rows.into_iter().map(|row| proposal_of(work, row)).collect()
    }

    /// Accepts a proposal: in one transaction `mark_word` on the current revision and a new revision by the human; word
    /// no longer there or the same mark already there → status `stale` and `StoreError::Glossary`, text unchanged.
    pub async fn index_proposal_accept(
        &self,
        work: WorkId,
        id: IndexProposalId,
        by: &str,
    ) -> Result<IndexAccepted, StoreError> {
        let by = actor(by, "Name des Menschen")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let proposal = proposal_in(&mut tx, work, id).await?;
        still_open(&proposal)?;
        let seq = seq_of_key(&mut tx, work, proposal.key).await?;
        let node_id = crate::write::node_id(&mut tx, work, seq).await?;
        let Some((_, body, _)) = crate::write::current(&mut tx, node_id).await? else {
            return Err(StoreError::Refused("Dieser Abschnitt hat noch keinen Text."));
        };
        let (text, marks) = match mark_word(&body, &proposal.word, proposal.occurrence, &proposal.mark) {
            Ok(done) => done,
            Err(IndexError::WordNotFound(word)) => {
                // The text has changed: proposal stale (stored), the text stays as it is.
                decide_proposal(&mut tx, id, IndexProposalStatus::Stale, &by).await?;
                tx.commit().await?;
                return Err(glossary_error(format!(
                    "{} Der Registervorschlag {id} ist darum veraltet; nichts am Text geändert.",
                    IndexError::WordNotFound(word)
                )));
            }
            Err(IndexError::AlreadyMarked(word)) => {
                // The mark is already there (for example set by the human): no second, identical mark, no new revision;
                // the proposal is done and therefore becomes stale (stored).
                decide_proposal(&mut tx, id, IndexProposalStatus::Stale, &by).await?;
                tx.commit().await?;
                return Err(glossary_error(format!(
                    "{} Der Registervorschlag {id} ist darum veraltet.",
                    IndexError::AlreadyMarked(word)
                )));
            }
            // Invalid mark: reject honestly, nothing changed.
            Err(other) => return Err(glossary_error(other.to_string())),
        };
        let Some(change) = crate::write::save_human(&mut tx, work, seq, &text, &by, None).await? else {
            return Err(glossary_error("Die Marke hätte den Text nicht geändert; nichts übernommen."));
        };
        crate::outline::check_current_heading(&mut tx, work, seq).await?;
        decide_proposal(&mut tx, id, IndexProposalStatus::Accepted, &by).await?;
        let proposal = proposal_in(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(IndexAccepted { proposal, change, marks })
    }

    /// Rejects a proposal (status `rejected`).
    pub async fn index_proposal_reject(
        &self,
        work: WorkId,
        id: IndexProposalId,
        by: &str,
    ) -> Result<IndexProposal, StoreError> {
        let by = actor(by, "Name des Menschen")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let proposal = proposal_in(&mut tx, work, id).await?;
        still_open(&proposal)?;
        decide_proposal(&mut tx, id, IndexProposalStatus::Rejected, &by).await?;
        let proposal = proposal_in(&mut tx, work, id).await?;
        tx.commit().await?;
        Ok(proposal)
    }
}

/// All terms of the work (accepted and proposed), sorted by `sort_key`; for `exchange_dump`.
pub(crate) async fn all_terms_in(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<GlossaryTerm>, StoreError> {
    terms_where(conn, work, "").await
}

/// Accepted terms as entries for typesetting and EPUB, sorted by `sort_key`.
pub(crate) async fn accepted_entries_in(
    conn: &mut sqlx::SqliteConnection,
    work: WorkId,
) -> Result<Vec<GlossaryEntry>, StoreError> {
    Ok(terms_where(conn, work, "AND status = 'accepted'").await?.iter().map(GlossaryTerm::entry).collect())
}

/// Custom indexes as [`Register`] (kind `Custom`) in their order.
pub(crate) async fn registers_in(conn: &mut sqlx::SqliteConnection, work: WorkId) -> Result<Vec<Register>, StoreError> {
    Ok(register_rows(conn, work)
        .await?
        .into_iter()
        .map(|r| Register { slug: r.slug, title: r.title, kind: RegisterKind::Custom })
        .collect())
}

/// Path of the mirror file of the glossary.
pub const MIRROR_FILE: &str = "glossar.md";

/// Mirror file `glossar.md` (accepted terms alphabetically, then the proposals, then the custom indexes). Without terms
/// and custom indexes no file (a work without a glossary mirrors as before Round 31).
pub(crate) async fn mirror_files(
    conn: &mut sqlx::SqliteConnection,
    work: WorkId,
) -> Result<Vec<crate::MirrorFile>, StoreError> {
    let terms = terms_where(conn, work, "").await?;
    let registers = register_rows(conn, work).await?;
    if terms.is_empty() && registers.is_empty() {
        return Ok(Vec::new());
    }
    let title: String =
        sqlx::query_scalar("SELECT title FROM work WHERE id = ?").bind(work).fetch_one(&mut *conn).await?;
    Ok(vec![crate::MirrorFile { path: MIRROR_FILE.to_owned(), contents: mirror_markdown(&title, &terms, &registers) }])
}

/// A term as a Markdown block under a heading of the level `level`.
fn term_markdown(out: &mut String, term: &GlossaryTerm, heading: &str) {
    out.push_str(&format!("{heading}\n\n"));
    if !term.variants.is_empty() {
        out.push_str(&format!("Varianten: {}\n\n", term.variants.join(", ")));
    }
    if !term.short.is_empty() {
        out.push_str(&format!("{}\n\n", term.short));
    }
    if !term.long.is_empty() {
        out.push_str(&format!("{}\n\n", term.long));
    }
    if !term.bible_refs.is_empty() {
        out.push_str(&format!("Bibelstellen: {}\n\n", term.bible_refs.join("; ")));
    }
    if !term.source.is_empty() {
        out.push_str(&format!("Quelle: {}\n\n", term.source));
    }
}

fn mirror_markdown(title: &str, terms: &[GlossaryTerm], registers: &[IndexRegisterRow]) -> String {
    let mut out = format!(
        "# Glossar – {title}\n\nErzeugt von Baruch aus der Datenbank (Runde 31); nur lesen, Änderungen in Baruch.\n\n"
    );
    let accepted: Vec<&GlossaryTerm> = terms.iter().filter(|t| t.status == GlossaryStatus::Accepted).collect();
    let proposed: Vec<&GlossaryTerm> = terms.iter().filter(|t| t.status == GlossaryStatus::Proposed).collect();
    if accepted.is_empty() {
        out.push_str("Noch keine angenommenen Begriffe.\n\n");
    }
    for term in accepted {
        term_markdown(&mut out, term, &format!("## {}", term.term));
    }
    if !proposed.is_empty() {
        out.push_str("## Vorschläge (noch nicht angenommen)\n\n");
        for term in proposed {
            term_markdown(&mut out, term, &format!("### {} (vorgeschlagen von {})", term.term, term.origin));
        }
    }
    if !registers.is_empty() {
        out.push_str("## Eigene Register\n\n");
        for register in registers {
            out.push_str(&format!("- `{}`: {}\n", register.slug, register.title));
        }
        out.push('\n');
    }
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_term_is_cleaned_before_it_is_stored() {
        // Terms and variants are on one line; duplicates (also a variant equal to the term) are dropped, so that the
        // lookup through form_key stays unambiguous.
        let edit = GlossaryEdit {
            term: "  Gnade \n Gottes ".into(),
            variants: vec!["Gnade  Gottes".into(), "".into(), "Gnaden".into(), "GNADEN".into()],
            bible_refs: vec![" Röm 3,24 ".into(), "Röm 3,24".into(), "".into()],
            long: "Erste Zeile\r\nZweite Zeile\n".into(),
            ..GlossaryEdit::default()
        };
        let c = clean(&edit).unwrap();
        assert_eq!(c.term, "Gnade Gottes", "Begriff in einer Zeile");
        assert_eq!(c.key, "gnade gottes", "Schlüssel = term_key");
        assert_eq!(c.variants, [("Gnaden".to_owned(), "gnaden".to_owned())], "Doppel und Leeres weg: {:?}", c.variants);
        assert_eq!(c.bible_refs, ["Röm 3,24"], "Bibelstellen ohne Doppel");
        assert_eq!(c.long, "Erste Zeile\nZweite Zeile", "ausführliche Erklärung behält Zeilen, ohne CR");
    }

    #[test]
    fn limits_are_named_in_german() {
        // Contract §2.5: limits MAX_* with German messages.
        let cases = [
            (GlossaryEdit::default(), "Begriff: darf nicht leer sein"),
            (GlossaryEdit { term: "x".repeat(201), ..Default::default() }, "Begriff: höchstens 200 Zeichen"),
            (GlossaryEdit { term: "a".into(), short: "k".repeat(501), ..Default::default() }, "Kurze Erklärung"),
            (GlossaryEdit { term: "a".into(), long: "l".repeat(10_001), ..Default::default() }, "Ausführliche"),
            (GlossaryEdit { term: "a".into(), source: "q".repeat(501), ..Default::default() }, "Quelle"),
            (
                GlossaryEdit {
                    term: "a".into(),
                    variants: (0..33).map(|i| format!("v{i}")).collect(),
                    ..Default::default()
                },
                "Varianten: höchstens 32",
            ),
            (
                GlossaryEdit {
                    term: "a".into(),
                    bible_refs: (1..=33).map(|i| format!("Ps {i}")).collect(),
                    ..Default::default()
                },
                "Bibelstellen: höchstens 32",
            ),
        ];
        for (edit, expected) in cases {
            let Err(StoreError::Glossary(message)) = clean(&edit) else { panic!("{expected}: Fehler erwartet") };
            assert!(message.contains(expected), "Meldung nennt „{expected}“: {message}");
        }
    }

    #[test]
    fn the_mirror_file_lists_accepted_terms_then_proposals_then_registers() {
        let term = |id, term: &str, status, origin: &str| GlossaryTerm {
            id,
            work: 1,
            uuid: format!("u{id}"),
            term: term.into(),
            short: format!("Kurz zu {term}"),
            long: String::new(),
            variants: vec![],
            source: String::new(),
            bible_refs: vec![],
            status,
            origin: origin.into(),
            created: "t".into(),
            updated: None,
            updated_by: String::new(),
        };
        let mut terms = vec![
            term(1, "Zelt", GlossaryStatus::Accepted, "Autor"),
            term(2, "Bund", GlossaryStatus::Proposed, "@lektor"),
            term(3, "Altar", GlossaryStatus::Accepted, "Autor"),
        ];
        sort_terms(&mut terms);
        let register = IndexRegisterRow {
            id: 1,
            uuid: "r".into(),
            slug: "orte".into(),
            title: "Ortsregister".into(),
            position: 0,
            created: "t".into(),
        };
        let md = mirror_markdown("Die Wüstenwanderung", &terms, &[register]);
        let altar = md.find("## Altar").expect("Altar");
        let zelt = md.find("## Zelt").expect("Zelt");
        let proposals = md.find("## Vorschläge").expect("Vorschläge");
        let bund = md.find("### Bund (vorgeschlagen von @lektor)").expect("Bund als Vorschlag gekennzeichnet");
        let registers = md.find("- `orte`: Ortsregister").expect("eigenes Register");
        assert!(altar < zelt && zelt < proposals && proposals < bund && bund < registers, "Reihenfolge: {md}");
        assert!(md.starts_with("# Glossar – Die Wüstenwanderung\n"), "Kopf mit Werktitel: {md}");
        assert!(md.ends_with("Ortsregister\n") && !md.ends_with("\n\n"), "genau ein Zeilenende am Schluss: {md:?}");
    }
}
