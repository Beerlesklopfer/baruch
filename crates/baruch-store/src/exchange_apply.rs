// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reading an exchange file into the database (Round 26; contract F6, documentation `docs/Austauschformat.md` §4).
//!
//! - New ([`ApplyMode::New`]): the work UUID is unknown (tombstones do not matter); the work is created with all UUIDs,
//!   numbers, statuses, times and markers from the file, plus its tombstone list; title rules as in the import (taken →
//!   `DuplicateTitle`, custom title through `title`). At the end `load::work` checks in the same transaction.
//! - Merging ([`ApplyMode::Merge`]): the work UUID is known; only what is missing is added. Nothing is ever deleted or
//!   overwritten, a revision read in never becomes current; outline changes become requests with the origin `Austausch:
//!   <Instanz>` (human). Unknown items are created at their position with exactly their revisions; a new part that
//!   would adopt existing chapters becomes a request `Add`. A deviating revision becomes a further revision with the
//!   identifier [`derive_uuid`]. What has a tombstone here does not come back (`deleted_here`).
//! - Preview ([`Store::exchange_preview`]) and applying ([`Store::exchange_apply`]) run through the same code: the
//!   preview in a transaction that is rolled back at the end. That way the numbers agree, and the preview is not
//!   trusted when applying (it plans anew).
//! - Mind map (Round 25, contract §4.6): New takes over all links and positions with their identifiers („widerspricht“
//!   is stored here so that from < to). Merging: a link with an unknown identifier whose two ideas belong to the work
//!   here is added – unless the same pair with the same kind already exists here (no duplicate, notice) or a tombstone;
//!   a known identifier is never overwritten. A position only comes if there is none here for (work, idea) (local wins)
//!   and no tombstone exists.
//! - Cover layout, addresses and typeset view (Round 27, minor version 3): New takes over all blocks with their
//!   identifiers, the addresses with the book details and the typeset view settings. Merging: a block with an unknown
//!   identifier only comes if there is none here yet for (work, kind) (local wins, otherwise a notice) and no tombstone
//!   exists; a known block is never overwritten (deviation: notice). If the typeset view settings are missing here
//!   completely, they come from the file (notice), otherwise only a notice on a deviation.
//! - Findings and old post versions (Round 28, minor version 4): New takes over all findings together with their status
//!   history and all old versions with their identifiers (status, reason, revision and target as in the file; `gleicht`
//!   through the identifier, after all findings are in place). Merging: a finding with an unknown identifier is added
//!   if its origin (post or thread post) belongs to the work here, no tombstone exists and the position in this post is
//!   still free here (otherwise a notice – for instance if both instances have broken down the same post); it brings
//!   its history along. A known finding is never overwritten: if its status deviates, there is only a notice, and
//!   history entries of the file are not added (the human sets the status here). Old versions of a post with an unknown
//!   identifier are added (also for a known post; they are only history).
//! - Minor version 5 (Round 30, [`apply_u5`]): on the rows created in this run a follow-up pass sets the solutions of
//!   the findings, the variants of the revisions (findings through their identifier; a deviating revision with a
//!   derived identifier stays without a variant) and the metadata of the posts (basis through the identifier of the
//!   revision of the same item, reply/repetition through the identifier of a post of the same place). Known rows stay
//!   untouched. The cross-references `#link(<kN>)` in revisions read in are converted by
//!   `baruch_typst::xref::remap_keys` to the keys here (mapping through `schluessel` of the items of the file →
//!   identifier → key here).
//! - Typesetting settings (Round 29, minor version 6, [`merge_typeset`]): New takes them over with identifier, point in
//!   time and name of the last change. Merging: if they are missing here and there is no tombstone for their
//!   identifier, they come from the file; otherwise nothing is ever overwritten – if they deviate, a notice names the
//!   fields and both states (JSON), if their identifier is reset here (tombstone), they stand under „hier gelöscht“,
//!   and a tombstone `satz` of the file leaves the settings here in place (notice). Invalid values
//!   (`TypesetSettings::validate`) are not taken over (under „Nicht übernommen“), the rest of the work is.
//! - Index and glossary (Round 31, minor version 7, [`new_glossary`], [`merge_glossary`]): New takes over custom
//!   indexes and all terms (accepted and proposed) with identifier, origin and times. Merging aligns instead of
//!   duplicating: an index or term with a known identifier is never overwritten (deviation: notice); an unknown
//!   identifier is added, unless there already is an index of the same identifier (`slug`) here or a term with the same
//!   key (`term_key`; a proposal not next to an accepted term either) – then a notice –, a tombstone („hier gelöscht“)
//!   or already 13 custom indexes. Terms outside the limits (`MAX_GLOSSARY_*`) stand under „Nicht übernommen“.
//! - Minor version 8 (Round 22, [`apply_u8`], [`fast_forward`], [`adopt_part`]): on the revisions created in this run a
//!   follow-up pass sets state, state of the base and the amendments (with their identifiers; the texts before/after
//!   with the keys here like the revision), on the findings and history entries created the state of the adopted
//!   revision. Merging: if the amendments of a known revision here are a prefix of those read in (same identifiers,
//!   text here = text of the file at this point), it is **fast-forwarded** (the missing steps are added, text and state
//!   follow the file); otherwise "deviating revision" applies as before. A new part that would adopt existing chapters
//!   becomes a request `Add` with identifier and revisions ([`crate::requests::PartIdentity`]); on consent
//!   [`adopt_part`] creates it exactly like that.
//! - Only this module sets a uuid itself, and only on a row that it created in the same transaction. Reachable as
//!   `baruch_store::exchange_apply::…`.

use std::collections::{HashMap, HashSet};

use baruch_exchange::{ExchangeAmend, ExchangeGlossaryStatus, ExchangeGlossaryTerm};
use baruch_exchange::{
    ExchangeAssetStatus, ExchangeAuthor, ExchangeBackdrop, ExchangeCoverAlign, ExchangeCoverBlock, ExchangeDocument,
    ExchangeFontStyle, ExchangeIdea, ExchangeIdeaStatus, ExchangeIdeaUse, ExchangeInstance, ExchangeLink,
    ExchangeLinkKind, ExchangeNode, ExchangeNodeKind, ExchangeOriginKind, ExchangePosition, ExchangePost,
    ExchangePostRef, ExchangeRevision, ExchangeRevisionStatus, ExchangeSheet, ExchangeSourceKind,
    ExchangeSpineDirection, ExchangeTitleArea, ExchangeTombstoneKind, ExchangeWorkKind, FORMAT_VERSION,
};
use baruch_exchange::{ExchangeFinding, ExchangeFindingKind, ExchangePostVersion, ExchangeTypeset};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;

use crate::import::free_slug;
use crate::outline::{BareInsert, NewPoint};
use crate::works::{title_free, title_key};
use crate::{IdeaOrigin, Store, StoreError, WorkId, assets, broken, load, outline, requests, slug};

/// At most this many entries in [`ExchangeReport::items`]; above that `truncated`.
pub const MAX_ITEMS: usize = 500;

/// Outline schemes that `work.numbering_scheme` accepts (migration 0040).
const SCHEMES: &[&str] = &[
    "template",
    "decimal",
    "decimal_dot",
    "alphanumeric",
    "chapter_roman",
    "chapter_arabic",
    "parts_chapters",
    "chapter_words",
    "legal",
    "sermon",
    "book",
    "none",
    "custom",
];

/// Derived identifier of a deviating revision (contract F3): the first 16 bytes of SHA-256(`baruch-fassung:` ‖ original
/// ‖ `:` ‖ sha256_hex(text)), version 8, variant 10. The same input yields the same identifier; that way a repeated
/// reading in creates nothing twice.
pub fn derive_uuid(original: &str, text: &str) -> String {
    let digest = Sha256::digest(
        format!("baruch-fassung:{original}:{}", baruch_exchange::sha256_hex(text.as_bytes())).as_bytes(),
    );
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// Display name of the exporting instance: its name, otherwise the first 8 characters of the identifier.
pub fn instance_label(instance: &ExchangeInstance) -> String {
    let name = instance.name.trim();
    if name.is_empty() { instance.uuid.chars().take(8).collect() } else { name.to_owned() }
}

/// New or merge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyMode {
    New,
    Merge,
}

impl ApplyMode {
    /// `new` or `merge` (RPC, form).
    pub fn as_str(self) -> &'static str {
        match self {
            ApplyMode::New => "new",
            ApplyMode::Merge => "merge",
        }
    }

    pub fn parse(s: &str) -> Option<ApplyMode> {
        match s {
            "new" => Some(ApplyMode::New),
            "merge" => Some(ApplyMode::Merge),
            _ => None,
        }
    }

    /// „neu“ or „abgleichen“.
    pub fn label(self) -> &'static str {
        match self {
            ApplyMode::New => "neu",
            ApplyMode::Merge => "abgleichen",
        }
    }
}

/// How much is added (or has been added).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AddCounts {
    pub nodes: usize,
    pub revisions: usize,
    pub template_revisions: usize,
    pub ideas: usize,
    pub posts: usize,
    pub threads: usize,
    pub thread_posts: usize,
    pub assets: usize,
    pub asset_revisions: usize,
    /// Links of the mind map (Round 25).
    pub links: usize,
    /// Positions on the mind map (Round 25).
    pub positions: usize,
    /// Blocks of the cover layout (Round 27).
    pub cover_blocks: usize,
    /// Findings (Round 28).
    pub findings: usize,
    /// Entries in the status history of the findings (Round 28).
    pub finding_entries: usize,
    /// Old versions of edited posts and thread posts (Round 28).
    pub post_versions: usize,
    /// Typesetting settings (Round 29; 0 or 1).
    pub typeset: usize,
    /// Custom indexes (Round 31).
    pub registers: usize,
    /// Glossary terms (Round 31).
    pub glossary_terms: usize,
    /// Amendments of revisions (Round 22; also the steps added when fast-forwarding).
    pub amends: usize,
}

impl AddCounts {
    pub fn total(&self) -> usize {
        self.nodes
            + self.revisions
            + self.template_revisions
            + self.ideas
            + self.posts
            + self.threads
            + self.thread_posts
            + self.assets
            + self.asset_revisions
            + self.links
            + self.positions
            + self.cover_blocks
            + self.findings
            + self.finding_entries
            + self.post_versions
            + self.typeset
            + self.registers
            + self.glossary_terms
            + self.amends
    }
}

/// A piece that is added: kind (XML word as in the tombstone, for example `punkt`, `fassung`), identifier, display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExchangeItem {
    pub kind: &'static str,
    pub uuid: String,
    pub label: String,
}

/// What happens to a deviation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviationAction {
    /// A further revision is created (deviating text).
    Fassung,
    /// A request concerning the outline is created.
    Antrag,
    /// Only a notice; nothing changed.
    Hinweis,
}

impl DeviationAction {
    pub fn label(self) -> &'static str {
        match self {
            DeviationAction::Fassung => "weitere Fassung",
            DeviationAction::Antrag => "Antrag",
            DeviationAction::Hinweis => "Hinweis",
        }
    }
}

/// A deviation between here and the file. `key`/`rev` lead to the place (`/?work=W&key=K&rev=R`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Deviation {
    pub kind: &'static str,
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<u32>,
    pub label: String,
    pub here: String,
    pub there: String,
    pub action: DeviationAction,
}

/// A request that is created (or has been created).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedRequest {
    /// `rename`, `move`, `set_level`, `set_numbered`, `add`.
    pub op: &'static str,
    pub key: i64,
    pub label: String,
}

/// A piece of the file that is deleted here (tombstone) and therefore does not come back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeletedHere {
    pub kind: &'static str,
    pub uuid: String,
    pub label: String,
}

/// Exporting instance (from the file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstanceInfo {
    pub uuid: String,
    pub name: String,
}

/// Preview or report of the applying (form of the RPC `work.exchange_preview`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExchangeReport {
    pub mode: ApplyMode,
    pub work_uuid: String,
    /// Title of the work here (for "new" the title it would get).
    pub title: String,
    /// Work here with this identifier (`None` = new).
    pub known_work: Option<WorkId>,
    pub instance: InstanceInfo,
    pub exported: String,
    pub version: u32,
    pub sha256: String,
    pub add: AddCounts,
    pub items: Vec<ExchangeItem>,
    pub truncated: bool,
    pub deviations: Vec<Deviation>,
    pub requests: Vec<PlannedRequest>,
    pub deleted_here: Vec<DeletedHere>,
    pub unknown: Vec<String>,
    pub refusals: Vec<String>,
}

/// Result of [`Store::exchange_apply`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExchangeApplied {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    pub mode: ApplyMode,
    /// ids of the requests created.
    pub requests: Vec<i64>,
    pub report: ExchangeReport,
}

impl Store {
    /// Preview: plans the reading in inside a transaction and rolls it back (nothing stays written). Mode from the file
    /// (work UUID known → merge). A title that is taken stands in `refusals`.
    pub async fn exchange_preview(
        &self,
        doc: &ExchangeDocument,
        sha256: &str,
        title: Option<&str>,
    ) -> Result<ExchangeReport, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = execute(&mut tx, doc, sha256, None, title, true).await;
        tx.rollback().await?;
        result.map(|done| done.report)
    }

    /// Apply (atomic, `BEGIN IMMEDIATE`): plans anew (the preview is not trusted) and writes. If `mode` does not fit
    /// the file → `Refused`; every error rolls everything back.
    pub async fn exchange_apply(
        &self,
        doc: &ExchangeDocument,
        sha256: &str,
        mode: ApplyMode,
        title: Option<&str>,
    ) -> Result<ExchangeApplied, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let done = execute(&mut tx, doc, sha256, Some(mode), title, false).await?;
        tx.commit().await?;
        Ok(ExchangeApplied {
            work: done.work,
            slug: done.slug,
            title: done.title,
            mode: done.report.mode,
            requests: done.request_ids,
            report: done.report,
        })
    }
}

// ---- Sequence ----------------------------------------------------------------------------------

struct Done {
    work: WorkId,
    slug: String,
    title: String,
    report: ExchangeReport,
    request_ids: Vec<i64>,
}

/// State of a run (preview or applying).
struct Run<'a> {
    doc: &'a ExchangeDocument,
    preview: bool,
    report: ExchangeReport,
    origin: IdeaOrigin,
    note: String,
    instance: String,
    tombstones: HashSet<String>,
    request_ids: Vec<i64>,
}

impl Run<'_> {
    fn item(&mut self, kind: &'static str, uuid: &str, label: String) {
        let add = &mut self.report.add;
        match kind {
            "punkt" => add.nodes += 1,
            "fassung" => add.revisions += 1,
            "vorlage" => add.template_revisions += 1,
            "einfall" => add.ideas += 1,
            "beitrag" => add.posts += 1,
            "faden" => add.threads += 1,
            "faden-beitrag" => add.thread_posts += 1,
            "bild" => add.assets += 1,
            "verbindung" => add.links += 1,
            "lage" => add.positions += 1,
            "umschlag-block" => add.cover_blocks += 1,
            "befund" => add.findings += 1,
            "befund-status" => add.finding_entries += 1,
            "beitrag-fassung" => add.post_versions += 1,
            "satz" => add.typeset += 1,
            "register" => add.registers += 1,
            "glossar-begriff" => add.glossary_terms += 1,
            "fortschreibung" => add.amends += 1,
            _ => add.asset_revisions += 1,
        }
        if self.report.items.len() < MAX_ITEMS {
            self.report.items.push(ExchangeItem { kind, uuid: uuid.to_owned(), label });
        } else {
            self.report.truncated = true;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn deviation(
        &mut self,
        kind: &'static str,
        uuid: &str,
        key: Option<i64>,
        rev: Option<u32>,
        label: String,
        here: String,
        there: String,
        action: DeviationAction,
    ) {
        self.report.deviations.push(Deviation { kind, uuid: uuid.to_owned(), key, rev, label, here, there, action });
    }

    fn hint(&mut self, kind: &'static str, uuid: &str, key: Option<i64>, label: String, here: String, there: String) {
        self.deviation(kind, uuid, key, None, label, here, there, DeviationAction::Hinweis);
    }

    fn deleted_here(&mut self, kind: &'static str, uuid: &str, label: String) {
        self.report.deleted_here.push(DeletedHere { kind, uuid: uuid.to_owned(), label });
    }

    fn tombstoned(&self, uuid: &str) -> bool {
        self.tombstones.contains(uuid)
    }
}

async fn execute(
    conn: &mut SqliteConnection,
    doc: &ExchangeDocument,
    sha256: &str,
    wanted: Option<ApplyMode>,
    title: Option<&str>,
    preview: bool,
) -> Result<Done, StoreError> {
    baruch_exchange::read::validate(doc)?;
    let known: Option<WorkId> = sqlx::query_scalar("SELECT id FROM work WHERE uuid = ?")
        .bind(&doc.work.uuid)
        .fetch_optional(&mut *conn)
        .await?;
    let mode = if known.is_some() { ApplyMode::Merge } else { ApplyMode::New };
    match (wanted, mode) {
        (Some(ApplyMode::New), ApplyMode::Merge) => {
            return Err(StoreError::Refused(
                "Werk schon vorhanden: Diese Datei gehört zu einem Werk, das es hier gibt; bitte abgleichen statt neu einlesen.",
            ));
        }
        (Some(ApplyMode::Merge), ApplyMode::New) => {
            return Err(StoreError::Refused(
                "Abgleichen geht nicht: Das Werk dieser Datei gibt es hier nicht; bitte als neues Werk einlesen.",
            ));
        }
        _ => {}
    }
    let instance = instance_label(&doc.instance);
    let mut run = Run {
        doc,
        preview,
        report: ExchangeReport {
            mode,
            work_uuid: doc.work.uuid.clone(),
            title: String::new(),
            known_work: known,
            instance: InstanceInfo { uuid: doc.instance.uuid.clone(), name: doc.instance.name.clone() },
            exported: doc.exported.clone(),
            version: doc.version,
            sha256: sha256.to_owned(),
            add: AddCounts::default(),
            items: Vec::new(),
            truncated: false,
            deviations: Vec::new(),
            requests: Vec::new(),
            deleted_here: Vec::new(),
            unknown: doc.unknown.clone(),
            refusals: Vec::new(),
        },
        origin: IdeaOrigin::Human { name: format!("Austausch: {instance}") },
        note: format!("Austausch aus Instanz {}, Datei {sha256}", doc.instance.uuid),
        instance,
        tombstones: HashSet::new(),
        request_ids: Vec::new(),
    };
    if doc.version > FORMAT_VERSION {
        let there = format!("Unterversion {} (dieses Programm liest {FORMAT_VERSION})", doc.version);
        run.hint(
            "version",
            &doc.work.uuid,
            None,
            "Neuere Unterversion: Unbekanntes wird gemeldet und nicht übernommen".into(),
            format!("Unterversion {FORMAT_VERSION}"),
            there,
        );
    }
    // Round 23 (u7): limits of glossary and indexes, before anything is written.
    check_u7_limits(doc, known.is_none())?;
    let marks = Marks::capture(conn).await?;
    let (work, slug, title) = match known {
        None => apply_new(conn, &mut run, title).await?,
        Some(work) => {
            let tombstones: Vec<String> =
                sqlx::query_scalar("SELECT uuid FROM uuid_tombstone").fetch_all(&mut *conn).await?;
            run.tombstones = tombstones.into_iter().collect();
            apply_merge(conn, &mut run, work).await?;
            let (slug, title): (String, String) =
                sqlx::query_as("SELECT slug, title FROM work WHERE id = ?").bind(work).fetch_one(&mut *conn).await?;
            (work, slug, title)
        }
    };
    run.report.title = title.clone();
    apply_u5(conn, &run, work, &marks).await?;
    apply_u8(conn, &mut run, work, &marks).await?;
    heading_hints(conn, &mut run, work, &marks).await?;
    load::work(conn, work).await?;
    Ok(Done { work, slug, title, report: run.report, request_ids: run.request_ids })
}

// ---- Round 23: limits (u7) and notice on the heading level -----------------------------------------

/// Limits of indexes and glossary before the first write (review R31): an index title above
/// [`baruch_typst::index::MAX_REGISTER_TITLE_CHARS`] characters and – when reading in as a new work – more than
/// [`crate::MAX_GLOSSARY_TERMS`] terms reject the whole reading in (German message, nothing written). When merging,
/// [`insert_term`] counts against the stock here (there the transaction aborts; nothing stays written either).
fn check_u7_limits(doc: &ExchangeDocument, new_work: bool) -> Result<(), StoreError> {
    for register in &doc.work.registers {
        check_register_title(register)?;
    }
    let terms = doc.work.glossary.len();
    if new_work && terms > crate::MAX_GLOSSARY_TERMS {
        return Err(too_many_terms(terms));
    }
    Ok(())
}

fn too_many_terms(count: usize) -> StoreError {
    StoreError::Exchange(format!(
        "Das Glossar hätte {count} Begriffe; höchstens {} je Werk (Vorschläge mitgezählt). Nichts eingelesen.",
        crate::MAX_GLOSSARY_TERMS
    ))
}

/// Title of an index of the file: on one line at most `MAX_REGISTER_TITLE_CHARS` characters.
fn check_register_title(register: &baruch_exchange::ExchangeRegister) -> Result<String, StoreError> {
    let title = register.title.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = title.chars().count();
    if chars > baruch_typst::index::MAX_REGISTER_TITLE_CHARS {
        return Err(StoreError::Exchange(format!(
            "Register {}: Titel hat {chars} Zeichen (höchstens {}). Nichts eingelesen.",
            register.slug,
            baruch_typst::index::MAX_REGISTER_TITLE_CHARS
        )));
    }
    Ok(title)
}

/// Notice if the heading of a **current revision just read in** does not fit the kind of its item (backlog R26 → R23):
/// the text stays exactly as it is in the file; but only the next saving would demand the fitting level – the human is
/// meant to see that already when reading in. Levels as in `outline::heading_level`: chapter 1, section 2, below a
/// section 3; parts stay without a check (part texts without the marker are allowed). Only a notice, never a rejection;
/// known revisions stay untouched and without a message.
async fn heading_hints(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    work: WorkId,
    marks: &Marks,
) -> Result<(), StoreError> {
    type CurrentRow = (i64, Option<String>, String, String, Option<String>, String);
    let rows: Vec<CurrentRow> = sqlx::query_as(
        "SELECT n.id, n.uuid, n.title, n.kind, p.kind, r.body FROM revision r JOIN node n ON n.id = r.node_id \
         LEFT JOIN node p ON p.work_id = n.work_id AND p.seq = n.parent_seq \
         WHERE n.work_id = ? AND r.status = 'current' AND r.id > ? ORDER BY n.seq",
    )
    .bind(work)
    .bind(marks.revision)
    .fetch_all(&mut *conn)
    .await?;
    let word = |level: u8| match level {
        0 => "Teil",
        1 => "Kapitel",
        2 => "Abschnitt",
        _ => "Unterabschnitt",
    };
    for (key, uuid, title, kind, parent_kind, body) in rows {
        let level: u8 = match (kind.as_str(), parent_kind.as_deref()) {
            ("chapter", _) => 1,
            ("section", Some("section")) => 3,
            ("section", _) => 2,
            _ => continue,
        };
        let Ok(doc) = baruch_typst::parse(&body.replace("\r\n", "\n")) else { continue };
        let Some(found) = doc.headings.first().map(|h| h.level) else { continue };
        if found == level {
            continue;
        }
        run.hint(
            "fassung",
            uuid.as_deref().unwrap_or(""),
            Some(key),
            format!(
                "{}: Die Überschrift der eingelesenen Fassung hat Ebene {found} ({}), der Gliederungspunkt Ebene \
                 {level} ({}). Der Text bleibt, wie er ist; beim nächsten Speichern verlangt Baruch die passende Ebene",
                quoted(&title),
                word(found),
                word(level)
            ),
            format!("Ebene {level}"),
            format!("Ebene {found}"),
        );
    }
    Ok(())
}

// ---- Minor version 5 (Round 30) ------------------------------------------------------------------

/// Highest ids before the run: rows above them were created by this run (SQLite assigns new rowids above the maximum).
struct Marks {
    revision: i64,
    post: i64,
    thread_post: i64,
    finding: i64,
    /// Round 22: history entries of the findings (for the state of the adopted revision).
    finding_status: i64,
}

impl Marks {
    async fn capture(conn: &mut SqliteConnection) -> Result<Marks, StoreError> {
        let max = |table: &'static str| format!("SELECT COALESCE(MAX(id), 0) FROM {table}");
        Ok(Marks {
            revision: sqlx::query_scalar(sqlx::AssertSqlSafe(max("revision"))).fetch_one(&mut *conn).await?,
            post: sqlx::query_scalar(sqlx::AssertSqlSafe(max("post"))).fetch_one(&mut *conn).await?,
            thread_post: sqlx::query_scalar(sqlx::AssertSqlSafe(max("chat_thread_post"))).fetch_one(&mut *conn).await?,
            finding: sqlx::query_scalar(sqlx::AssertSqlSafe(max("finding"))).fetch_one(&mut *conn).await?,
            finding_status: sqlx::query_scalar(sqlx::AssertSqlSafe(max("finding_status")))
                .fetch_one(&mut *conn)
                .await?,
        })
    }
}

/// All items of the file (flat).
fn all_file_nodes<'a>(nodes: &'a [ExchangeNode], out: &mut Vec<&'a ExchangeNode>) {
    for node in nodes {
        out.push(node);
        all_file_nodes(&node.children, out);
    }
}

/// Follow-up pass of minor version 5 on the newly created rows (see the module documentation).
async fn apply_u5(conn: &mut SqliteConnection, run: &Run<'_>, work: WorkId, marks: &Marks) -> Result<(), StoreError> {
    let doc = run.doc;
    let mut file_nodes = Vec::new();
    all_file_nodes(&doc.work.outline, &mut file_nodes);
    // Identifiers here: finding → id, item → id (= key), revision → (item, number), post → (id, place).
    let findings: HashMap<String, i64> =
        sqlx::query_as::<_, (String, i64)>("SELECT uuid, id FROM finding WHERE work_id = ? AND uuid IS NOT NULL")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
    let nodes: HashMap<String, i64> =
        sqlx::query_as::<_, (String, i64)>("SELECT uuid, id FROM node WHERE work_id = ? AND uuid IS NOT NULL")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
    let revisions: HashMap<String, (i64, i64, i64)> = sqlx::query_as::<_, (String, i64, i64, i64)>(
        "SELECT r.uuid, r.id, r.node_id, r.number FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND r.uuid IS NOT NULL",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(uuid, id, node, number)| (uuid, (id, node, number)))
    .collect();
    let posts: HashMap<String, (i64, i64)> = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT p.uuid, p.id, p.node_id FROM post p JOIN node n ON n.id = p.node_id \
         WHERE n.work_id = ? AND p.uuid IS NOT NULL",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(uuid, id, node)| (uuid, (id, node)))
    .collect();
    let thread_posts: HashMap<String, (i64, i64)> = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT p.uuid, p.id, p.thread_id FROM chat_thread_post p JOIN chat_thread t ON t.id = p.thread_id \
         WHERE t.work_id = ? AND p.uuid IS NOT NULL",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(uuid, id, thread)| (uuid, (id, thread)))
    .collect();

    // Cross-references: key of the file → key here (through the identifier of the item).
    let key_map: HashMap<i64, i64> = file_nodes
        .iter()
        .filter_map(|n| Some((n.key?, *nodes.get(&n.uuid)?)))
        .filter(|(old, new)| old != new)
        .collect();

    for node in &file_nodes {
        for revision in &node.revisions {
            // Round 23: if a known revision deviates, the merge creates it under a derived identifier
            // ([`derive_uuid`]). It, too, carries the variant of the file (group, label, line, findings) – before, it
            // lost its marks and stood as an ordinary proposal next to its siblings.
            let own = revisions.get(&revision.uuid).filter(|(id, ..)| *id > marks.revision);
            let derived = match own {
                Some(_) => None,
                None if revision.variant.is_some() => {
                    revisions.get(&derive_uuid(&revision.uuid, &revision.body)).filter(|(id, ..)| *id > marks.revision)
                }
                None => None,
            };
            if let (Some(&(id, ..)), Some(v)) = (derived, &revision.variant) {
                let ids: Vec<i64> = v.findings.iter().filter_map(|u| findings.get(u).copied()).collect();
                sqlx::query(
                    "UPDATE revision SET variant_group = ?, variant_label = ?, variant_summary = ?, \
                     variant_findings = ? WHERE id = ?",
                )
                .bind(&v.group)
                .bind(&v.label)
                .bind(&v.summary)
                .bind(serde_json::to_string(&ids).expect("Zahlenliste ist JSON"))
                .bind(id)
                .execute(&mut *conn)
                .await?;
            }
            let Some(&(id, node_id, _)) = own else { continue };
            if let Some(v) = &revision.variant {
                let ids: Vec<i64> = v.findings.iter().filter_map(|u| findings.get(u).copied()).collect();
                sqlx::query(
                    "UPDATE revision SET variant_group = ?, variant_label = ?, variant_summary = ?, \
                     variant_findings = ? WHERE id = ?",
                )
                .bind(&v.group)
                .bind(&v.label)
                .bind(&v.summary)
                .bind(serde_json::to_string(&ids).expect("Zahlenliste ist JSON"))
                .bind(id)
                .execute(&mut *conn)
                .await?;
            }
            if !key_map.is_empty() {
                let remapped = baruch_typst::xref::remap_keys(&revision.body, &key_map);
                if let std::borrow::Cow::Owned(body) = remapped {
                    sqlx::query("UPDATE revision SET body = ? WHERE id = ? AND node_id = ?")
                        .bind(body)
                        .bind(id)
                        .bind(node_id)
                        .execute(&mut *conn)
                        .await?;
                }
            }
        }
        for post in &node.posts {
            let Some(&(id, node_id)) = posts.get(&post.uuid) else { continue };
            if id <= marks.post || post.meta.is_empty() {
                continue;
            }
            let basis = post
                .meta
                .basis_revision
                .as_ref()
                .and_then(|u| revisions.get(u))
                .filter(|(_, n, _)| *n == node_id)
                .map(|(_, _, number)| *number);
            let same = |u: &Option<String>| {
                u.as_ref().and_then(|u| posts.get(u)).filter(|(_, n)| *n == node_id).map(|(id, _)| *id)
            };
            sqlx::query(
                "UPDATE post SET basis_rev = ?, basis_label = ?, reply_to = ?, repeat_of = ?, relay_from = ? WHERE id = ?",
            )
            .bind(basis)
            .bind(&post.meta.basis_label)
            .bind(same(&post.meta.reply_to))
            .bind(same(&post.meta.repeat_of))
            .bind(&post.meta.relay_from)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        }
    }
    for thread in &doc.work.threads {
        for post in &thread.posts {
            let Some(&(id, thread_id)) = thread_posts.get(&post.uuid) else { continue };
            if id <= marks.thread_post || post.meta.is_empty() {
                continue;
            }
            let same = |u: &Option<String>| {
                u.as_ref().and_then(|u| thread_posts.get(u)).filter(|(_, t)| *t == thread_id).map(|(id, _)| *id)
            };
            sqlx::query(
                "UPDATE chat_thread_post SET basis_label = ?, reply_to = ?, repeat_of = ?, relay_from = ? WHERE id = ?",
            )
            .bind(&post.meta.basis_label)
            .bind(same(&post.meta.reply_to))
            .bind(same(&post.meta.repeat_of))
            .bind(&post.meta.relay_from)
            .bind(id)
            .execute(&mut *conn)
            .await?;
        }
    }
    for finding in &doc.work.findings {
        let Some(&id) = findings.get(&finding.uuid) else { continue };
        if id <= marks.finding || finding.solutions.is_empty() {
            continue;
        }
        let list: Vec<crate::Solution> = finding
            .solutions
            .iter()
            .map(|s| crate::Solution {
                quote: s.quote.clone(),
                replacement: s.replacement.clone(),
                step: s.step.clone(),
            })
            .collect();
        sqlx::query("UPDATE finding SET solutions = ? WHERE id = ?")
            .bind(serde_json::to_string(&list).expect("Lösungen sind JSON"))
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

// ---- Minor version 8 (Round 22) ------------------------------------------------------------------

/// Attach the steps `amends` to the revision `revision_id` (identifiers from the file, texts with the keys here).
async fn insert_amends(
    conn: &mut SqliteConnection,
    revision_id: i64,
    amends: &[ExchangeAmend],
    key_map: &HashMap<i64, i64>,
) -> Result<(), StoreError> {
    for step in amends {
        let before = baruch_typst::xref::remap_keys(&step.before, key_map);
        let after = baruch_typst::xref::remap_keys(&step.after, key_map);
        sqlx::query(
            "INSERT INTO revision_amend (revision_id, seq, kind, undo_of, body_before, body_after, author_name, created, \
             uuid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(revision_id)
        .bind(i64::from(step.seq))
        .bind(step.kind.as_db())
        .bind(step.undo_of.map(i64::from))
        .bind(before.as_ref())
        .bind(after.as_ref())
        .bind(&step.author)
        .bind(&step.created)
        .bind(&step.uuid)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// Is one of these identifiers of steps already assigned or buried here?
async fn amends_taken(
    conn: &mut SqliteConnection,
    run: &Run<'_>,
    amends: &[ExchangeAmend],
) -> Result<bool, StoreError> {
    for step in amends {
        if run.tombstoned(&step.uuid) || uuid_exists(conn, "revision_amend", &step.uuid).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Follow-up pass of minor version 8 on the newly created rows (see the module documentation): state, state of the base
/// and amendments of the revisions with the identifier from the file (derived revisions stay without), state on
/// findings and history entries.
async fn apply_u8(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    work: WorkId,
    marks: &Marks,
) -> Result<(), StoreError> {
    let doc = run.doc;
    let mut file_nodes = Vec::new();
    all_file_nodes(&doc.work.outline, &mut file_nodes);
    let nodes: HashMap<String, i64> =
        sqlx::query_as::<_, (String, i64)>("SELECT uuid, id FROM node WHERE work_id = ? AND uuid IS NOT NULL")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
    let key_map: HashMap<i64, i64> = file_nodes
        .iter()
        .filter_map(|n| Some((n.key?, *nodes.get(&n.uuid)?)))
        .filter(|(old, new)| old != new)
        .collect();
    let revisions: HashMap<String, i64> = sqlx::query_as::<_, (String, i64)>(
        "SELECT r.uuid, r.id FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? AND r.uuid IS NOT NULL",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();
    for node in &file_nodes {
        for revision in &node.revisions {
            let Some(&id) = revisions.get(&revision.uuid) else { continue };
            if id <= marks.revision || (revision.amend_seq == 0 && revision.based_on_amend.is_none()) {
                continue;
            }
            let label = format!("{}, Fassung {}", quoted(&node.title), revision.number);
            let steps_ok = revision.amends.is_empty() || !amends_taken(conn, run, &revision.amends).await?;
            if !steps_ok {
                run.hint(
                    "fortschreibung",
                    &revision.uuid,
                    None,
                    format!("{label}: Fortschreibungen nicht übernommen (Kennung hier schon vergeben oder gelöscht)"),
                    String::new(),
                    format!("Stand {}", revision.amend_seq),
                );
            }
            let amend_seq = if steps_ok { revision.amend_seq } else { 0 };
            sqlx::query("UPDATE revision SET amend_seq = ?, based_on_amend = ? WHERE id = ?")
                .bind(i64::from(amend_seq))
                .bind(revision.based_on_amend.map(i64::from))
                .bind(id)
                .execute(&mut *conn)
                .await?;
            if steps_ok && !revision.amends.is_empty() {
                insert_amends(conn, id, &revision.amends, &key_map).await?;
                for step in &revision.amends {
                    run.item("fortschreibung", &step.uuid, format!("{label}, Fortschreibung {}", step.seq));
                }
            }
        }
    }
    let findings: HashMap<String, i64> =
        sqlx::query_as::<_, (String, i64)>("SELECT uuid, id FROM finding WHERE work_id = ? AND uuid IS NOT NULL")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?
            .into_iter()
            .collect();
    let entries: HashMap<String, i64> = sqlx::query_as::<_, (String, i64)>(
        "SELECT s.uuid, s.id FROM finding_status s JOIN finding f ON f.id = s.finding_id \
         WHERE f.work_id = ? AND s.uuid IS NOT NULL",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .collect();
    for finding in &doc.work.findings {
        if let (Some(&id), Some(stand)) = (findings.get(&finding.uuid), finding.adopted_amend)
            && id > marks.finding
        {
            sqlx::query("UPDATE finding SET adopted_amend = ? WHERE id = ?")
                .bind(i64::from(stand))
                .bind(id)
                .execute(&mut *conn)
                .await?;
        }
        for entry in &finding.history {
            if let (Some(&id), Some(stand)) = (entries.get(&entry.uuid), entry.adopted_amend)
                && id > marks.finding_status
            {
                sqlx::query("UPDATE finding_status SET adopted_amend = ? WHERE id = ?")
                    .bind(i64::from(stand))
                    .bind(id)
                    .execute(&mut *conn)
                    .await?;
            }
        }
    }
    Ok(())
}

/// Merging (rule d with minor version 8): if the amendments of the known revision here are a proper prefix of those
/// read in (same identifiers in the same order, state here = number of steps here) and the text here is exactly the
/// text of the file at this point, the revision is fast-forwarded: the missing steps are added, text and state follow
/// the file. `true` = fast-forwarded (then no "deviating revision").
///
/// As locally ([`crate::amend`]) only on the **current revision of the human** (by the human here as in the file,
/// current here): revisions by personas or MCP clients, proposals, accepted, rejected and superseded revisions are
/// never rewritten in place. And only if the missing steps reproduce the change without gaps (first step before = text
/// here, per step before = after of the previous one, last step after = text of the file) – otherwise the history would
/// show harmless steps while the text was replaced completely, and an undo could reset to a text that never existed
/// here. In all other cases `false` (→ "deviating revision" as a new derived revision as before Round 22).
#[allow(clippy::too_many_arguments)]
async fn fast_forward(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    node_key: i64,
    title: &str,
    here_number: u32,
    revision: &ExchangeRevision,
    body_there: &str,
    key_map: &HashMap<i64, i64>,
) -> Result<bool, StoreError> {
    if revision.amends.is_empty() {
        return Ok(false);
    }
    // Only the revision of the human (also in the file) and only current here – like `amend::verdict` (NotOwn).
    if author_columns(&revision.author).0 != "human" {
        return Ok(false);
    }
    let Some((id, amend_seq, body_here)): Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, amend_seq, body FROM revision WHERE uuid = ? AND node_id = ? \
         AND author_kind = 'human' AND status = 'current'",
    )
    .bind(&revision.uuid)
    .bind(node_key)
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(false);
    };
    let local: Vec<Option<String>> =
        sqlx::query_scalar("SELECT uuid FROM revision_amend WHERE revision_id = ? ORDER BY seq")
            .bind(id)
            .fetch_all(&mut *conn)
            .await?;
    // A step without an identifier (exists only before the fill trigger) fits none of the file.
    let local: Vec<String> = local.into_iter().map(Option::unwrap_or_default).collect();
    let k = local.len();
    if k >= revision.amends.len()
        || usize::try_from(amend_seq).ok() != Some(k)
        || local.iter().zip(&revision.amends).any(|(here, there)| *here != there.uuid)
    {
        return Ok(false);
    }
    let at_prefix = if k == 0 { &revision.amends[0].before } else { &revision.amends[k - 1].after };
    if *baruch_typst::xref::remap_keys(at_prefix, key_map) != body_here {
        return Ok(false);
    }
    let missing = &revision.amends[k..];
    // The missing steps must reproduce the change from `body_here` to `body_there` without gaps.
    let remap = |text: &str| baruch_typst::xref::remap_keys(text, key_map).into_owned();
    let mut text = body_here.clone();
    for step in missing {
        if remap(&step.before) != text {
            return Ok(false);
        }
        text = remap(&step.after);
    }
    if text != body_there {
        return Ok(false);
    }
    // Code review 2026-11-24 (store-2): locally an amendment never changes the heading (`amend::verdict` → new
    // revision). A chain of steps read in could otherwise do that in place: the current revision would then carry a
    // heading that does not exist in the outline (`node.title` would stay old), or a second one. Therefore fast-forward
    // only if all headings (level, title) stay the same – otherwise the previous path "deviating revision" (a new
    // revision on which the human decides).
    if heading_lines(&body_here) != heading_lines(body_there) {
        return Ok(false);
    }
    if amends_taken(conn, run, missing).await? {
        return Ok(false);
    }
    insert_amends(conn, id, missing, key_map).await?;
    sqlx::query("UPDATE revision SET body = ?, amend_seq = ? WHERE id = ?")
        .bind(body_there)
        .bind(i64::from(revision.amend_seq))
        .bind(id)
        .execute(&mut *conn)
        .await?;
    let label = format!("{}, Fassung {here_number}", quoted(title));
    for step in missing {
        run.item("fortschreibung", &step.uuid, format!("{label}, Fortschreibung {}", step.seq));
    }
    run.deviation(
        "fassung",
        &revision.uuid,
        Some(node_key),
        Some(here_number),
        format!(
            "{label}: vorgespult von Stand {k} auf Stand {} (Fortschreibungen aus {})",
            revision.amend_seq, run.instance
        ),
        excerpt(&body_here),
        excerpt(body_there),
        DeviationAction::Hinweis,
    );
    Ok(true)
}

/// Heading lines of a text verbatim (without whitespace at the edges): lines that begin with `=` or `#heading(`.
/// Deliberately by wording instead of through the parser: the comparison is only meant to say "same or not" and, in
/// doubt (such as `=` at the start of a line in a raw block), rather not fast-forward.
fn heading_lines(body: &str) -> Vec<&str> {
    body.lines().map(str::trim).filter(|line| line.starts_with('=') || line.starts_with("#heading(")).collect()
}

/// Revisions of a part from the file for the request `Add` (Round 22, contract §2.10).
fn part_identity(node: &ExchangeNode) -> crate::requests::PartIdentity {
    let mut revisions: Vec<&ExchangeRevision> = node.revisions.iter().collect();
    revisions.sort_by_key(|r| r.number);
    crate::requests::PartIdentity {
        uuid: node.uuid.clone(),
        revisions: revisions
            .into_iter()
            .map(|r| {
                let (author_kind, author_name, model) = author_columns(&r.author);
                crate::requests::PartRevision {
                    uuid: r.uuid.clone(),
                    number: r.number,
                    body: r.body.clone(),
                    author_kind: author_kind.to_owned(),
                    author_name: author_name.to_owned(),
                    model: model.map(str::to_owned),
                    status: status_word(r.status).to_owned(),
                    note: r.note.clone(),
                    based_on: r.based_on.clone(),
                    created: r.created.clone(),
                    edited: r.edited.clone(),
                    amend_seq: r.amend_seq,
                    based_on_amend: r.based_on_amend,
                    amends: r
                        .amends
                        .iter()
                        .map(|a| crate::requests::PartAmend {
                            uuid: a.uuid.clone(),
                            seq: a.seq,
                            kind: a.kind.as_db().to_owned(),
                            undo_of: a.undo_of,
                            author: a.author.clone(),
                            created: a.created.clone(),
                            before: a.before.clone(),
                            after: a.after.clone(),
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

/// Consenting to a request `Add` with identifier and revisions (Round 22, contract §2.10; called from `requests::run`
/// in the same transaction, directly after `add_many`): the part just created gets the identifier from the file,
/// revision 1 produced by `add_many` is replaced (deleted together with its tombstone – it never existed outside this
/// transaction), then exactly the revisions of the file come with number, identifier, origin, status, base, state and
/// amendments. If one of the identifiers already exists here (read in or created in the meantime) → `Refused` (the
/// request is closed as stale, nothing changed).
pub(crate) async fn adopt_part(
    conn: &mut SqliteConnection,
    work: WorkId,
    added: &crate::outline::OutlineAdded,
    part: &crate::requests::PartIdentity,
) -> Result<(), StoreError> {
    const TAKEN: &str =
        "Den Teil gibt es hier inzwischen mit dieser Kennung (oder eine seiner Fassungen); nichts angelegt.";
    let Some(point) = added.points.iter().find(|p| p.level == 0) else {
        return Err(StoreError::Refused("Antrag mit Kennung: kein Teil unter den neuen Punkten."));
    };
    if part.revisions.iter().filter(|r| r.status == "current").count() != 1 {
        return Err(StoreError::Refused("Antrag mit Kennung: genau eine aktuelle Fassung erwartet."));
    }
    if uuid_exists(conn, "node", &part.uuid).await? {
        return Err(StoreError::Refused(TAKEN));
    }
    for revision in &part.revisions {
        if uuid_exists(conn, "revision", &revision.uuid).await? {
            return Err(StoreError::Refused(TAKEN));
        }
        for step in &revision.amends {
            if uuid_exists(conn, "revision_amend", &step.uuid).await? {
                return Err(StoreError::Refused(TAKEN));
            }
        }
    }
    let key = point.key;
    let generated: Vec<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, uuid FROM revision WHERE node_id = ?").bind(key).fetch_all(&mut *conn).await?;
    for (id, uuid) in &generated {
        sqlx::query("UPDATE revision SET status = 'superseded' WHERE id = ? AND status = 'current'")
            .bind(id)
            .execute(&mut *conn)
            .await?;
        sqlx::query("DELETE FROM revision WHERE id = ?").bind(id).execute(&mut *conn).await?;
        if let Some(uuid) = uuid {
            sqlx::query("DELETE FROM uuid_tombstone WHERE uuid = ?").bind(uuid).execute(&mut *conn).await?;
        }
    }
    sqlx::query("UPDATE node SET last_rev = 0, uuid = ? WHERE id = ?")
        .bind(&part.uuid)
        .bind(key)
        .execute(&mut *conn)
        .await?;
    let numbers: HashMap<&str, u32> = part.revisions.iter().map(|r| (r.uuid.as_str(), r.number)).collect();
    let mut sorted: Vec<&crate::requests::PartRevision> = part.revisions.iter().collect();
    sorted.sort_by_key(|r| r.number);
    let no_keys = HashMap::new();
    for r in sorted {
        let based_on = r.based_on.as_deref().and_then(|b| numbers.get(b).copied());
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on, \
             created, edited, amend_seq, based_on_amend, uuid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(key)
        .bind(i64::from(r.number))
        .bind(&r.body)
        .bind(&r.author_kind)
        .bind(&r.author_name)
        .bind(&r.model)
        .bind(&r.status)
        .bind(&r.note)
        .bind(based_on.map(i64::from))
        .bind(&r.created)
        .bind(&r.edited)
        .bind(i64::from(r.amend_seq))
        .bind(r.based_on_amend.map(i64::from))
        .bind(&r.uuid)
        .execute(&mut *conn)
        .await?;
        if !r.amends.is_empty() {
            let id: i64 = sqlx::query_scalar("SELECT id FROM revision WHERE uuid = ?")
                .bind(&r.uuid)
                .fetch_one(&mut *conn)
                .await?;
            let steps: Vec<ExchangeAmend> = r
                .amends
                .iter()
                .map(|a| {
                    Ok(ExchangeAmend {
                        uuid: a.uuid.clone(),
                        seq: a.seq,
                        kind: baruch_exchange::ExchangeAmendKind::from_db(&a.kind)
                            .ok_or_else(|| broken(format!("Antrag: Art der Fortschreibung {}", a.kind)))?,
                        undo_of: a.undo_of,
                        author: a.author.clone(),
                        created: a.created.clone(),
                        before: a.before.clone(),
                        after: a.after.clone(),
                    })
                })
                .collect::<Result<_, StoreError>>()?;
            insert_amends(conn, id, &steps, &no_keys).await?;
        }
    }
    let _ = work;
    Ok(())
}

// ---- Words and small helpers ---------------------------------------------------------------------

fn author_columns(author: &ExchangeAuthor) -> (&'static str, &str, Option<&str>) {
    match author {
        ExchangeAuthor::Human { name } => ("human", name, None),
        ExchangeAuthor::Assistant { persona, model } => ("assistant", persona, Some(model)),
    }
}

fn status_word(status: ExchangeRevisionStatus) -> &'static str {
    match status {
        ExchangeRevisionStatus::Current => "current",
        ExchangeRevisionStatus::Proposed => "proposed",
        ExchangeRevisionStatus::Rejected => "rejected",
        ExchangeRevisionStatus::Superseded => "superseded",
    }
}

/// Status of a revision read in during a merge (F6 c): never current.
fn merged_status(status: ExchangeRevisionStatus) -> &'static str {
    match status {
        ExchangeRevisionStatus::Proposed => "proposed",
        ExchangeRevisionStatus::Rejected => "rejected",
        ExchangeRevisionStatus::Current | ExchangeRevisionStatus::Superseded => "superseded",
    }
}

fn level_of(kind: ExchangeNodeKind, parent_level: Option<u8>) -> u8 {
    match kind {
        ExchangeNodeKind::Part => 0,
        ExchangeNodeKind::Chapter => 1,
        ExchangeNodeKind::Section => parent_level.map_or(2, |p| (p + 1).max(2)),
    }
}

fn node_kind_word(level: u8) -> &'static str {
    match level {
        0 => "part",
        1 => "chapter",
        _ => "section",
    }
}

fn quoted(text: &str) -> String {
    format!("„{text}“")
}

/// Short single-line excerpt for preview and notices.
fn excerpt(text: &str) -> String {
    let line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = line.chars().take(120).collect();
    if line.chars().count() > 120 {
        out.push('…');
    }
    out
}

/// Domain rejection (then a request becomes only a notice)?
fn is_refusal(e: &StoreError) -> bool {
    matches!(
        e,
        StoreError::Refused(_)
            | StoreError::Title(_)
            | StoreError::Heading(_)
            | StoreError::GoneNode { .. }
            | StoreError::UnknownNode { .. }
    )
}

/// Does the identifier exist in this table (in any work)?
async fn uuid_exists(conn: &mut SqliteConnection, table: &str, uuid: &str) -> Result<bool, StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT 1 FROM {table} WHERE uuid = ? LIMIT 1")))
            .bind(uuid)
            .fetch_optional(&mut *conn)
            .await?;
    Ok(found.is_some())
}

/// Assign an idea without a work with this identifier (left over from a deleted work) to this work again. Its content
/// stays as it is here (never overwrite); only work and, if the item is known, reference are set.
async fn adopt_orphan_idea(
    conn: &mut SqliteConnection,
    work: WorkId,
    node_id: Option<i64>,
    uuid: &str,
) -> Result<bool, StoreError> {
    let done = sqlx::query("UPDATE idea SET work_id = ?, node_id = ? WHERE uuid = ? AND work_id IS NULL")
        .bind(work)
        .bind(node_id)
        .bind(uuid)
        .execute(&mut *conn)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// When reading in as a new work: the identifier must not exist here yet (otherwise it would belong to another work).
async fn ensure_free(conn: &mut SqliteConnection, table: &str, uuid: &str, what: &str) -> Result<(), StoreError> {
    if uuid_exists(conn, table, uuid).await? {
        return Err(StoreError::Exchange(format!(
            "{what}: die Kennung {uuid} gibt es hier schon in einem anderen Werk; nichts übernommen"
        )));
    }
    Ok(())
}

fn tombstone_table(kind: ExchangeTombstoneKind) -> &'static str {
    match kind {
        ExchangeTombstoneKind::Work => "work",
        ExchangeTombstoneKind::Node => "node",
        ExchangeTombstoneKind::Revision => "revision",
        ExchangeTombstoneKind::TemplateRevision => "work_template_revision",
        ExchangeTombstoneKind::Post => "post",
        ExchangeTombstoneKind::Idea => "idea",
        ExchangeTombstoneKind::Thread => "chat_thread",
        ExchangeTombstoneKind::ThreadPost => "chat_thread_post",
        ExchangeTombstoneKind::Asset => "asset",
        ExchangeTombstoneKind::AssetRevision => "asset_revision",
        ExchangeTombstoneKind::Link => "idea_link",
        ExchangeTombstoneKind::Position => "idea_map_pos",
        ExchangeTombstoneKind::CoverBlock => "cover_block",
        ExchangeTombstoneKind::Finding => "finding",
        ExchangeTombstoneKind::FindingStatus => "finding_status",
        ExchangeTombstoneKind::PostVersion => "post_version",
        ExchangeTombstoneKind::Typeset => "work_typeset",
        ExchangeTombstoneKind::GlossaryTerm => "glossary_term",
        ExchangeTombstoneKind::Register => "index_register",
        ExchangeTombstoneKind::Amend => "revision_amend",
    }
}

// ---- Writing single rows -------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
async fn insert_revision(
    conn: &mut SqliteConnection,
    node_id: i64,
    revision: &ExchangeRevision,
    number: u32,
    based_on: Option<u32>,
    status: &str,
    note: &str,
    uuid: &str,
) -> Result<(), StoreError> {
    let (kind, name, model) = author_columns(&revision.author);
    sqlx::query(
        "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note, based_on, created, \
         edited, uuid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(node_id)
    .bind(i64::from(number))
    .bind(&revision.body)
    .bind(kind)
    .bind(name)
    .bind(model)
    .bind(status)
    .bind(note)
    .bind(based_on.map(i64::from))
    .bind(&revision.created)
    .bind(&revision.edited)
    .bind(uuid)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// All revisions of an item just created exactly as in the file (numbers, status, base).
async fn insert_node_revisions(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    node_id: i64,
    node: &ExchangeNode,
) -> Result<(), StoreError> {
    let mut revisions: Vec<&ExchangeRevision> = node.revisions.iter().collect();
    revisions.sort_by_key(|r| r.number);
    let numbers: HashMap<&str, u32> = revisions.iter().map(|r| (r.uuid.as_str(), r.number)).collect();
    for revision in revisions {
        if run.report.mode == ApplyMode::New {
            ensure_free(conn, "revision", &revision.uuid, &format!("Fassung von {}", quoted(&node.title))).await?;
        }
        let based_on = revision.based_on.as_deref().and_then(|b| numbers.get(b).copied());
        insert_revision(
            conn,
            node_id,
            revision,
            revision.number,
            based_on,
            status_word(revision.status),
            &revision.note,
            &revision.uuid,
        )
        .await?;
        run.item("fassung", &revision.uuid, format!("{}, Fassung {}", quoted(&node.title), revision.number));
    }
    Ok(())
}

async fn insert_post(
    conn: &mut SqliteConnection,
    node_id: i64,
    position: i64,
    post: &ExchangePost,
) -> Result<i64, StoreError> {
    let sources = serde_json::to_string(&post.sources).map_err(|e| broken(format!("Quellen: {e}")))?;
    Ok(sqlx::query_scalar(
        "INSERT INTO post (node_id, position, author, body, sources, created, edited, uuid) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(node_id)
    .bind(position)
    .bind(&post.author)
    .bind(&post.body)
    .bind(sources)
    .bind(&post.created)
    .bind(&post.edited)
    .bind(&post.uuid)
    .fetch_one(&mut *conn)
    .await?)
}

/// To whom an old version belongs.
#[derive(Debug, Clone, Copy)]
enum VersionOwner {
    Post(i64),
    ThreadPost(i64),
}

/// Old versions of a post or thread post (Round 28). New: every identifier must be free. Merging: known identifiers
/// skipped, ones deleted here reported.
async fn insert_versions(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    owner: VersionOwner,
    versions: &[ExchangePostVersion],
    label: &str,
) -> Result<(), StoreError> {
    for version in versions {
        if run.report.mode == ApplyMode::New {
            ensure_free(conn, "post_version", &version.uuid, "Alte Fassung eines Beitrags").await?;
        } else if uuid_exists(conn, "post_version", &version.uuid).await? {
            continue;
        } else if run.tombstoned(&version.uuid) {
            run.deleted_here("beitrag-fassung", &version.uuid, format!("{label}, alte Fassung"));
            continue;
        }
        let (post, thread_post) = match owner {
            VersionOwner::Post(id) => (Some(id), None),
            VersionOwner::ThreadPost(id) => (None, Some(id)),
        };
        sqlx::query(
            "INSERT INTO post_version (post_id, thread_post_id, body, valid_from, replaced, edited_by, uuid) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(post)
        .bind(thread_post)
        .bind(&version.body)
        .bind(&version.valid_from)
        .bind(&version.replaced)
        .bind(&version.edited_by)
        .bind(&version.uuid)
        .execute(&mut *conn)
        .await?;
        run.item("beitrag-fassung", &version.uuid, format!("{label}, alte Fassung von {}", version.replaced));
    }
    Ok(())
}

fn finding_kind_label(kind: ExchangeFindingKind) -> &'static str {
    match kind {
        ExchangeFindingKind::Praise => "Lob",
        ExchangeFindingKind::Hint => "Hinweis",
        ExchangeFindingKind::Critical => "kritisch",
    }
}

fn finding_status_label(status: &str) -> &'static str {
    match status {
        "open" => "offen",
        "adopted" => "übernommen",
        "rejected" => "verworfen",
        "deferred" => "verschoben",
        "done" => "erledigt ohne Änderung",
        _ => "unbekannt",
    }
}

fn finding_label(finding: &ExchangeFinding) -> String {
    format!("Befund ({}) von {}: {}", finding_kind_label(finding.kind), finding.persona, excerpt(&finding.body))
}

/// Findings with their history (Round 28, minor version 4), after posts, threads and ideas. Rules in the module
/// documentation.
async fn apply_findings(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    let mut similar: Vec<(i64, &str)> = Vec::new();
    for finding in &doc.work.findings {
        let label = finding_label(finding);
        if run.report.mode == ApplyMode::Merge {
            let known: Option<(WorkId, String, String)> =
                sqlx::query_as("SELECT work_id, status, reason FROM finding WHERE uuid = ?")
                    .bind(&finding.uuid)
                    .fetch_optional(&mut *conn)
                    .await?;
            if let Some((owner, status, reason)) = known {
                if owner != work {
                    run.hint(
                        "befund",
                        &finding.uuid,
                        None,
                        format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                        String::new(),
                        String::new(),
                    );
                } else {
                    // Rule (Round 23, laid down): a known finding stays as it is here – status, reason and history. The
                    // history is the log of the decisions of **this** instance; foreign entries would otherwise
                    // contradict the status here, and an open proposal from the file would become a decision here that
                    // nobody asked for. So that nothing drops out silently, the notice names how many entries only the
                    // file has.
                    let mut only_there = 0usize;
                    for entry in &finding.history {
                        if !uuid_exists(conn, "finding_status", &entry.uuid).await? && !run.tombstoned(&entry.uuid) {
                            only_there += 1;
                        }
                    }
                    let entries = match only_there {
                        0 => String::new(),
                        1 => "; 1 Eintrag im Verlauf der Datei nicht übernommen".to_owned(),
                        n => format!("; {n} Einträge im Verlauf der Datei nicht übernommen"),
                    };
                    if status != finding.status.as_db() || reason != finding.reason {
                        run.hint(
                            "befund",
                            &finding.uuid,
                            None,
                            format!("{label}: Status weicht ab (bleibt; den Status setzt hier der Mensch{entries})"),
                            finding_status_label(&status).to_owned(),
                            finding_status_label(finding.status.as_db()).to_owned(),
                        );
                    } else if only_there > 0 {
                        run.hint(
                            "befund",
                            &finding.uuid,
                            None,
                            format!("{label}: Verlauf weicht ab (bleibt, wie er hier ist{entries})"),
                            finding_status_label(&status).to_owned(),
                            finding_status_label(finding.status.as_db()).to_owned(),
                        );
                    }
                }
                continue;
            }
            if run.tombstoned(&finding.uuid) {
                run.deleted_here("befund", &finding.uuid, label);
                continue;
            }
        } else {
            ensure_free(conn, "finding", &finding.uuid, "Befund").await?;
        }
        let node_id: Option<i64> = match &finding.node {
            Some(uuid) => {
                sqlx::query_scalar("SELECT id FROM node WHERE uuid = ? AND work_id = ?")
                    .bind(uuid)
                    .bind(work)
                    .fetch_optional(&mut *conn)
                    .await?
            }
            None => None,
        };
        let origin = match (&finding.post, &finding.thread_post) {
            (Some(uuid), _) => Some((
                "beitrag",
                "SELECT p.id FROM post p JOIN node n ON n.id = p.node_id WHERE p.uuid = ? AND n.work_id = ?",
                "SELECT id FROM finding WHERE post_id = ? AND position = ?",
                uuid,
            )),
            (None, Some(uuid)) => Some((
                "faden-beitrag",
                "SELECT tp.id FROM chat_thread_post tp JOIN chat_thread t ON t.id = tp.thread_id \
                 WHERE tp.uuid = ? AND t.work_id = ?",
                "SELECT id FROM finding WHERE thread_post_id = ? AND position = ?",
                uuid,
            )),
            (None, None) => None,
        };
        let mut post_id = None;
        let mut thread_post_id = None;
        if let Some((what, find, taken, uuid)) = origin {
            let here: Option<i64> = sqlx::query_scalar(find).bind(uuid).bind(work).fetch_optional(&mut *conn).await?;
            let Some(here) = here else {
                run.hint(
                    "befund",
                    &finding.uuid,
                    None,
                    format!("{label}: seine Herkunft ({what} {uuid}) fehlt hier – nicht übernommen"),
                    "fehlt".into(),
                    uuid.clone(),
                );
                continue;
            };
            let occupied: Option<i64> = sqlx::query_scalar(taken)
                .bind(here)
                .bind(i64::from(finding.position))
                .fetch_optional(&mut *conn)
                .await?;
            if let Some(local) = occupied {
                run.hint(
                    "befund",
                    &finding.uuid,
                    node_id,
                    format!(
                        "{label}: diese Stelle der Antwort ist hier schon Befund {local} (unter anderer Kennung) – \
                         nicht doppelt angelegt"
                    ),
                    format!("Befund {local}"),
                    "neu".into(),
                );
                continue;
            }
            if what == "beitrag" {
                post_id = Some(here);
            } else {
                thread_post_id = Some(here);
            }
        }
        let summary = serde_json::to_string(&finding.summary_of).map_err(|e| broken(format!("sammelt: {e}")))?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO finding (work_id, node_id, post_id, thread_post_id, position, persona, kind, body, quote, \
             quote_check, status, reason, adopted_revision, deferred_node, deferred_idea, summary_of, created, updated, \
             uuid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(node_id)
        .bind(post_id)
        .bind(thread_post_id)
        .bind(i64::from(finding.position))
        .bind(&finding.persona)
        .bind(finding.kind.as_db())
        .bind(&finding.body)
        .bind(&finding.quote)
        .bind(&finding.quote_check)
        .bind(finding.status.as_db())
        .bind(&finding.reason)
        .bind(&finding.adopted_revision)
        .bind(&finding.deferred_node)
        .bind(&finding.deferred_idea)
        .bind(&summary)
        .bind(&finding.created)
        .bind(&finding.updated)
        .bind(&finding.uuid)
        .fetch_one(&mut *conn)
        .await?;
        run.item("befund", &finding.uuid, label.clone());
        if let Some(original) = &finding.similar_to {
            similar.push((id, original.as_str()));
        }
        for entry in &finding.history {
            if run.report.mode == ApplyMode::New {
                ensure_free(conn, "finding_status", &entry.uuid, "Eintrag im Verlauf eines Befunds").await?;
            } else if uuid_exists(conn, "finding_status", &entry.uuid).await? {
                continue;
            } else if run.tombstoned(&entry.uuid) {
                run.deleted_here("befund-status", &entry.uuid, format!("{label}, Eintrag im Verlauf"));
                continue;
            }
            sqlx::query(
                "INSERT INTO finding_status (finding_id, status, reason, adopted_revision, deferred_node, \
                 deferred_idea, actor_kind, actor, proposed, note, decision, decided, decided_by, created, uuid) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(entry.status.as_db())
            .bind(&entry.reason)
            .bind(&entry.adopted_revision)
            .bind(&entry.deferred_node)
            .bind(&entry.deferred_idea)
            .bind(origin_word(entry.actor_kind))
            .bind(&entry.actor)
            .bind(i64::from(entry.proposed))
            .bind(&entry.note)
            .bind(entry.decision.map(|d| d.as_db()))
            .bind(&entry.decided)
            .bind(&entry.decided_by)
            .bind(&entry.created)
            .bind(&entry.uuid)
            .execute(&mut *conn)
            .await?;
            run.item("befund-status", &entry.uuid, format!("{label}, {}", finding_status_label(entry.status.as_db())));
        }
    }
    // „gleicht“ only now: the original can stand in the file after the attached finding.
    for (id, original) in similar {
        let target: Option<i64> = sqlx::query_scalar("SELECT id FROM finding WHERE uuid = ? AND work_id = ?")
            .bind(original)
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
        if let Some(target) = target {
            sqlx::query("UPDATE finding SET similar_to = ? WHERE id = ?")
                .bind(target)
                .bind(id)
                .execute(&mut *conn)
                .await?;
        }
    }
    Ok(())
}

/// References of a post: item through its identifier (position now), revision through its identifier (number; missing →
/// 0).
async fn insert_refs(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    work: WorkId,
    post_id: i64,
    post_uuid: &str,
    refs: &[ExchangePostRef],
) -> Result<(), StoreError> {
    let mut position = 0i64;
    for reference in refs {
        let node: Option<(i64, i64)> = sqlx::query_as("SELECT id, seq FROM node WHERE work_id = ? AND uuid = ?")
            .bind(work)
            .bind(&reference.node)
            .fetch_optional(&mut *conn)
            .await?;
        let Some((node_id, seq)) = node else {
            run.hint(
                "beitrag",
                post_uuid,
                None,
                "Bezug eines Beitrags auf einen Punkt, den es hier nicht gibt (Bezug entfällt)".into(),
                "fehlt".into(),
                reference.node.clone(),
            );
            continue;
        };
        let rev: Option<i64> = match &reference.revision {
            Some(uuid) => {
                sqlx::query_scalar("SELECT number FROM revision WHERE node_id = ? AND uuid = ?")
                    .bind(node_id)
                    .bind(uuid)
                    .fetch_optional(&mut *conn)
                    .await?
            }
            None => None,
        };
        sqlx::query("INSERT INTO post_ref (post_id, position, node_seq, rev, quote) VALUES (?, ?, ?, ?, ?)")
            .bind(post_id)
            .bind(position)
            .bind(seq)
            .bind(rev.unwrap_or(0))
            .bind(&reference.quote)
            .execute(&mut *conn)
            .await?;
        position += 1;
    }
    Ok(())
}

fn origin_word(kind: ExchangeOriginKind) -> &'static str {
    match kind {
        ExchangeOriginKind::Human => "human",
        ExchangeOriginKind::Persona => "persona",
        ExchangeOriginKind::Mcp => "mcp",
    }
}

fn use_word(used: ExchangeIdeaUse) -> &'static str {
    match used {
        ExchangeIdeaUse::Outline => "outline",
        ExchangeIdeaUse::Post => "post",
        ExchangeIdeaUse::Persona => "persona",
        ExchangeIdeaUse::Note => "note",
    }
}

fn idea_label(idea: &ExchangeIdea) -> String {
    let text = if idea.note.trim().is_empty() { idea.words.join(", ") } else { idea.note.clone() };
    format!("Einfall {}", quoted(&excerpt(&text)))
}

/// `done_ref` from the file: if it begins with the identifier of an item of this work, it becomes "key <key>" again
/// (counterpart of the output, which writes "key N" as an identifier); otherwise it stays verbatim.
async fn local_done_ref(conn: &mut SqliteConnection, work: WorkId, text: &str) -> Result<String, StoreError> {
    if text.len() >= 36 && text.is_char_boundary(36) && baruch_exchange::is_uuid(&text[..36]) {
        let key: Option<i64> = sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND uuid = ?")
            .bind(work)
            .bind(&text[..36])
            .fetch_optional(&mut *conn)
            .await?;
        if let Some(key) = key {
            return Ok(format!("key {key}{}", &text[36..]));
        }
    }
    Ok(text.to_owned())
}

/// Idea with keywords and index (deleted ones do not get into the index, as in `ideas.rs`).
async fn insert_idea(
    conn: &mut SqliteConnection,
    work: WorkId,
    node_id: Option<i64>,
    idea: &ExchangeIdea,
) -> Result<(), StoreError> {
    let done_ref = match &idea.done_ref {
        Some(text) => Some(local_done_ref(conn, work, text).await?),
        None => None,
    };
    let status = match idea.status {
        ExchangeIdeaStatus::Open => "open",
        ExchangeIdeaStatus::Done => "done",
    };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO idea (work_id, node_id, note, quote, source, bible, origin_kind, origin_name, status, done_as, \
         done_ref, created, updated, deleted, uuid, title, position, main_topic) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(work)
    .bind(node_id)
    .bind(&idea.note)
    .bind(&idea.quote)
    .bind(&idea.source)
    .bind(&idea.bible)
    .bind(origin_word(idea.origin_kind))
    .bind(&idea.origin_name)
    .bind(status)
    .bind(idea.done_as.map(use_word))
    .bind(&done_ref)
    .bind(&idea.created)
    .bind(&idea.updated)
    .bind(&idea.deleted)
    .bind(&idea.uuid)
    // Round 34 (minor version 9): title, order and main topic at once; the parent node is set by `link_idea_parents`
    // when all ideas of the file are in place. A main topic exists only without a reference.
    .bind(&idea.title)
    .bind(idea.position)
    .bind(i64::from(idea.main_topic && node_id.is_none() && idea.parent.is_none()))
    .fetch_one(&mut *conn)
    .await?;
    let mut keys = HashSet::new();
    let mut position = 0i64;
    for word in &idea.words {
        let key = crate::ideas::word_key(word);
        if key.is_empty() || !keys.insert(key.clone()) {
            continue;
        }
        sqlx::query("INSERT INTO idea_word (idea_id, position, word, word_key) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(position)
            .bind(word)
            .bind(&key)
            .execute(&mut *conn)
            .await?;
        position += 1;
    }
    if idea.deleted.is_none() {
        sqlx::query("INSERT INTO idea_fts (rowid, words, note, quote, source, bible) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(id)
            // Like `ideas::reindex`: the title is included in the column `words`.
            .bind(if idea.title.is_empty() {
                idea.words.join(" ")
            } else {
                format!("{} {}", idea.title, idea.words.join(" ")).trim().to_owned()
            })
            .bind(&idea.note)
            .bind(&idea.quote)
            .bind(&idea.source)
            .bind(&idea.bible)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Second step of reading in the ideas (Round 34, minor version 9): set the parent nodes of the ideas `fresh`
/// (identifiers) created or assigned again in this run – only now, because the parent node can stand behind the child
/// in the file. Detached with a notice (the idea stays at its reference or lies in the cloud) is an idea whose parent
/// node does not exist here in the work, whose parent node is deleted while it is not itself, or whose chain forms a
/// cycle or is deeper than `ideas::MAX_IDEA_DEPTH`.
async fn link_idea_parents(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    work: WorkId,
    fresh: &[String],
) -> Result<(), StoreError> {
    let doc = run.doc;
    let fresh: HashSet<&str> = fresh.iter().map(String::as_str).collect();
    for idea in &doc.work.ideas {
        let Some(parent_uuid) = &idea.parent else { continue };
        if !fresh.contains(idea.uuid.as_str()) {
            continue;
        }
        let label = idea_label(idea);
        let loosen = |run: &mut Run<'_>, why: &str| {
            run.hint(
                "einfall",
                &idea.uuid,
                None,
                format!("{label}: {why} – ohne Elternknoten übernommen"),
                String::new(),
                parent_uuid.clone(),
            );
        };
        let Some(id) = idea_id_here(conn, work, &idea.uuid).await? else { continue };
        let parent: Option<(i64, Option<String>)> =
            sqlx::query_as("SELECT id, deleted FROM idea WHERE work_id = ? AND uuid = ?")
                .bind(work)
                .bind(parent_uuid)
                .fetch_optional(&mut *conn)
                .await?;
        let Some((parent_id, parent_deleted)) = parent else {
            loosen(run, "der Elternknoten fehlt hier im Werk");
            continue;
        };
        if parent_deleted.is_some() && idea.deleted.is_none() {
            loosen(run, "der Elternknoten ist gelöscht");
            continue;
        }
        // Chain upwards: no cycle, not too deep.
        let mut depth = 1usize;
        let mut at = Some(parent_id);
        let mut sound = true;
        while let Some(current) = at {
            depth += 1;
            if current == id || depth > crate::ideas::MAX_IDEA_DEPTH {
                sound = false;
                break;
            }
            at = sqlx::query_scalar::<_, Option<i64>>("SELECT parent_id FROM idea WHERE id = ?")
                .bind(current)
                .fetch_optional(&mut *conn)
                .await?
                .flatten();
        }
        if !sound {
            loosen(run, "die Kette der Elternknoten bildet einen Kreis oder ist zu tief");
            continue;
        }
        sqlx::query("UPDATE idea SET parent_id = ?, main_topic = 0 WHERE id = ?")
            .bind(parent_id)
            .bind(id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

// ---- Mind map (Round 25) -------------------------------------------------------------------------

fn link_kind_word(kind: ExchangeLinkKind) -> &'static str {
    match kind {
        ExchangeLinkKind::LeadsTo => "leads_to",
        ExchangeLinkKind::BelongsTo => "belongs_to",
        ExchangeLinkKind::ExampleOf => "example_of",
        ExchangeLinkKind::Contradicts => "contradicts",
    }
}

/// Display like `baruch_store::LinkKind::label`.
fn link_kind_label(kind: ExchangeLinkKind) -> &'static str {
    match kind {
        ExchangeLinkKind::LeadsTo => "führt weiter",
        ExchangeLinkKind::BelongsTo => "gehört zu",
        ExchangeLinkKind::ExampleOf => "Beispiel für",
        ExchangeLinkKind::Contradicts => "widerspricht",
    }
}

/// Display of an idea of the file through its identifier (for links and positions).
fn idea_label_of(doc: &ExchangeDocument, uuid: &str) -> String {
    doc.work.ideas.iter().find(|i| i.uuid == uuid).map_or_else(|| format!("Einfall {uuid}"), idea_label)
}

fn link_label(doc: &ExchangeDocument, link: &ExchangeLink) -> String {
    format!(
        "Verbindung: {} {} {}",
        idea_label_of(doc, &link.from),
        link_kind_label(link.kind),
        idea_label_of(doc, &link.to)
    )
}

fn position_label(doc: &ExchangeDocument, position: &ExchangePosition) -> String {
    format!("Lage von {}", idea_label_of(doc, &position.idea))
}

/// Numeric id of the idea with this identifier in this work.
async fn idea_id_here(conn: &mut SqliteConnection, work: WorkId, uuid: &str) -> Result<Option<i64>, StoreError> {
    Ok(sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ? AND uuid = ?")
        .bind(work)
        .bind(uuid)
        .fetch_optional(&mut *conn)
        .await?)
}

/// Endpoints the way the table demands them: „widerspricht“ undirected with from < to (migration 0043).
fn link_ends(kind: ExchangeLinkKind, from: i64, to: i64) -> (i64, i64) {
    if kind == ExchangeLinkKind::Contradicts && to < from { (to, from) } else { (from, to) }
}

/// Create a link with the identifier from the file (origin, times, note and "deleted" as there).
async fn insert_link(conn: &mut SqliteConnection, from: i64, to: i64, link: &ExchangeLink) -> Result<(), StoreError> {
    let (from, to) = link_ends(link.kind, from, to);
    sqlx::query(
        "INSERT INTO idea_link (from_id, to_id, kind, note, origin_kind, origin_name, created, updated, deleted, uuid) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(from)
    .bind(to)
    .bind(link_kind_word(link.kind))
    .bind(&link.note)
    .bind(origin_word(link.origin_kind))
    .bind(&link.origin_name)
    .bind(&link.created)
    .bind(&link.updated)
    .bind(&link.deleted)
    .bind(&link.uuid)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_position(
    conn: &mut SqliteConnection,
    work: WorkId,
    idea: i64,
    position: &ExchangePosition,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO idea_map_pos (work_id, idea_id, x, y, updated, uuid) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(work)
        .bind(idea)
        .bind(position.x)
        .bind(position.y)
        .bind(&position.updated)
        .bind(&position.uuid)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// New: all links and positions of the file (the ideas are already in place; the check ensures that every link and
/// position points to ideas of the document).
async fn new_map(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    for link in &doc.work.map.links {
        ensure_free(conn, "idea_link", &link.uuid, "Verbindung").await?;
        let (Some(from), Some(to)) =
            (idea_id_here(conn, work, &link.from).await?, idea_id_here(conn, work, &link.to).await?)
        else {
            return Err(broken(format!("Verbindung {}: Einfall nach dem Anlegen nicht gefunden", link.uuid)));
        };
        insert_link(conn, from, to, link).await?;
        run.item("verbindung", &link.uuid, link_label(doc, link));
    }
    for position in &doc.work.map.positions {
        ensure_free(conn, "idea_map_pos", &position.uuid, "Lage").await?;
        let Some(idea) = idea_id_here(conn, work, &position.idea).await? else {
            return Err(broken(format!("Lage {}: Einfall nach dem Anlegen nicht gefunden", position.uuid)));
        };
        insert_position(conn, work, idea, position).await?;
        run.item("lage", &position.uuid, position_label(doc, position));
    }
    Ok(())
}

/// Merging the map (contract §4.6), after the ideas (ideas just read in count as known).
async fn merge_map(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    for link in &doc.work.map.links {
        let label = link_label(doc, link);
        type Known = (Option<WorkId>, String, Option<String>);
        let known: Option<Known> = sqlx::query_as(
            "SELECT i.work_id, l.note, l.deleted FROM idea_link l JOIN idea i ON i.id = l.from_id WHERE l.uuid = ?",
        )
        .bind(&link.uuid)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some((owner, note, deleted)) = known {
            // Known identifier: never overwrite, deviations only as a notice.
            if owner != Some(work) {
                run.hint(
                    "verbindung",
                    &link.uuid,
                    None,
                    format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            let mut differs = Vec::new();
            if note != link.note {
                differs.push("Notiz");
            }
            if deleted.is_some() != link.deleted.is_some() {
                differs.push("gelöscht");
            }
            if !differs.is_empty() {
                run.hint(
                    "verbindung",
                    &link.uuid,
                    None,
                    format!("{label}: {} weicht ab (bleibt)", differs.join(", ")),
                    excerpt(&note),
                    excerpt(&link.note),
                );
            }
            continue;
        }
        if run.tombstoned(&link.uuid) {
            run.deleted_here("verbindung", &link.uuid, label);
            continue;
        }
        let (Some(from), Some(to)) =
            (idea_id_here(conn, work, &link.from).await?, idea_id_here(conn, work, &link.to).await?)
        else {
            run.hint(
                "verbindung",
                &link.uuid,
                None,
                format!("{label}: ein Einfall fehlt hier im Werk – nicht übernommen"),
                String::new(),
                String::new(),
            );
            continue;
        };
        let (a, b) = link_ends(link.kind, from, to);
        let same: Option<(Option<String>,)> =
            sqlx::query_as("SELECT deleted FROM idea_link WHERE from_id = ? AND to_id = ? AND kind = ?")
                .bind(a)
                .bind(b)
                .bind(link_kind_word(link.kind))
                .fetch_optional(&mut *conn)
                .await?;
        if let Some((deleted,)) = same {
            // The same pair with the same kind under another identifier: no duplicate.
            let here = if deleted.is_some() { "hier gelöscht" } else { "hier vorhanden" };
            run.hint(
                "verbindung",
                &link.uuid,
                None,
                format!("{label}: gibt es hier schon unter anderer Kennung ({here}) – nicht doppelt angelegt"),
                here.into(),
                "neu".into(),
            );
            continue;
        }
        insert_link(conn, from, to, link).await?;
        run.item("verbindung", &link.uuid, label);
    }
    for position in &doc.work.map.positions {
        if uuid_exists(conn, "idea_map_pos", &position.uuid).await? {
            continue; // known position: local wins, nothing to report (pure display)
        }
        if run.tombstoned(&position.uuid) {
            run.deleted_here("lage", &position.uuid, position_label(doc, position));
            continue;
        }
        let Some(idea) = idea_id_here(conn, work, &position.idea).await? else {
            continue; // idea not in the work: no position
        };
        let local: Option<i64> = sqlx::query_scalar("SELECT id FROM idea_map_pos WHERE work_id = ? AND idea_id = ?")
            .bind(work)
            .bind(idea)
            .fetch_optional(&mut *conn)
            .await?;
        if local.is_some() {
            continue; // already a position here for this idea: local wins
        }
        insert_position(conn, work, idea, position).await?;
        run.item("lage", &position.uuid, position_label(doc, position));
    }
    Ok(())
}

async fn insert_thread_post(
    conn: &mut SqliteConnection,
    thread_id: i64,
    position: i64,
    post: &baruch_exchange::ExchangeThreadPost,
) -> Result<i64, StoreError> {
    let sources = serde_json::to_string(&post.sources).map_err(|e| broken(format!("Quellen: {e}")))?;
    Ok(sqlx::query_scalar(
        "INSERT INTO chat_thread_post (thread_id, position, author, body, sources, created, edited, uuid) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(thread_id)
    .bind(position)
    .bind(&post.author)
    .bind(&post.body)
    .bind(sources)
    .bind(&post.created)
    .bind(&post.edited)
    .bind(&post.uuid)
    .fetch_one(&mut *conn)
    .await?)
}

/// Image name for a new image: the one from the file if it is valid, otherwise formed from it; taken → a free one
/// (`-2`, `-3` … as in the import).
async fn asset_name(conn: &mut SqliteConnection, work: WorkId, name: &str, mime: &str) -> Result<String, StoreError> {
    let base = if assets::valid_asset_name(name) { name.to_owned() } else { assets::asset_name_from_file(name, mime) };
    assets::free_name(conn, work, &base).await
}

/// Check an image version as at the upload; errors as text for `refusals`.
fn check_image(asset: &str, revision: &baruch_exchange::ExchangeAssetRevision) -> Result<assets::Content, String> {
    let content = assets::prepare(asset, &revision.data, false)
        .map_err(|e| format!("Bild {asset}, Fassung {}: {e}", revision.number))?;
    if content.mime != revision.mime {
        return Err(format!(
            "Bild {asset}, Fassung {}: angegeben {}, die Bytes sind {}",
            revision.number, revision.mime, content.mime
        ));
    }
    Ok(content)
}

#[allow(clippy::too_many_arguments)]
async fn insert_asset_revision(
    conn: &mut SqliteConnection,
    asset_id: i64,
    number: u32,
    based_on: Option<u32>,
    revision: &baruch_exchange::ExchangeAssetRevision,
    status: &str,
    content: &assets::Content,
) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO asset_revision (asset_id, number, based_on, mime, encoding, content, bytes, sha256, caption, alt, \
         author_name, note, status, created, edited, uuid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(asset_id)
    .bind(i64::from(number))
    .bind(based_on.map(i64::from))
    .bind(content.mime)
    .bind(content.encoding.as_str())
    .bind(&content.content)
    .bind(content.bytes)
    .bind(&content.sha256)
    .bind(&revision.caption)
    .bind(&revision.alt)
    .bind(&revision.author)
    .bind(&revision.note)
    .bind(status)
    .bind(&revision.created)
    .bind(&revision.edited)
    .bind(&revision.uuid)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// New image with all versions as in the file and its paths. If a version does not pass the check, the whole image does
/// not come (entry in `refusals`, result `None`). A path that already belongs to another image here or is invalid
/// becomes a notice.
async fn insert_asset(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    work: WorkId,
    asset: &baruch_exchange::ExchangeAsset,
) -> Result<Option<i64>, StoreError> {
    let mut revisions: Vec<&baruch_exchange::ExchangeAssetRevision> = asset.revisions.iter().collect();
    revisions.sort_by_key(|r| r.number);
    let mut contents = Vec::with_capacity(revisions.len());
    for revision in &revisions {
        match check_image(&asset.name, revision) {
            Ok(content) => contents.push(content),
            Err(message) => {
                run.report.refusals.push(format!("{message} – das Bild wird nicht übernommen."));
                return Ok(None);
            }
        }
    }
    if run.report.mode == ApplyMode::New {
        ensure_free(conn, "asset", &asset.uuid, &format!("Bild {}", asset.name)).await?;
        for revision in &revisions {
            ensure_free(conn, "asset_revision", &revision.uuid, &format!("Bild {}", asset.name)).await?;
        }
    }
    let current_mime =
        revisions.iter().find(|r| r.status == ExchangeAssetStatus::Current).map_or("image/png", |r| r.mime.as_str());
    let name = asset_name(conn, work, &asset.name, current_mime).await?;
    let asset_id: i64 =
        sqlx::query_scalar("INSERT INTO asset (work_id, name, created, uuid) VALUES (?, ?, ?, ?) RETURNING id")
            .bind(work)
            .bind(&name)
            .bind(&asset.created)
            .bind(&asset.uuid)
            .fetch_one(&mut *conn)
            .await?;
    run.item("bild", &asset.uuid, format!("Bild {name}"));
    let numbers: HashMap<&str, u32> = revisions.iter().map(|r| (r.uuid.as_str(), r.number)).collect();
    for (revision, content) in revisions.iter().zip(&contents) {
        let based_on = revision.based_on.as_deref().and_then(|b| numbers.get(b).copied());
        let status = match revision.status {
            ExchangeAssetStatus::Current => "current",
            ExchangeAssetStatus::Superseded => "superseded",
        };
        insert_asset_revision(conn, asset_id, revision.number, based_on, revision, status, content).await?;
        run.item("bildfassung", &revision.uuid, format!("Bild {name}, Fassung {}", revision.number));
    }
    for path in &asset.paths {
        let path = match assets::check_asset_path(path) {
            Ok(path) => path,
            Err(e) => {
                run.hint(
                    "bild",
                    &asset.uuid,
                    None,
                    format!("Pfad von Bild {name} nicht übernommen: {e}"),
                    String::new(),
                    path.clone(),
                );
                continue;
            }
        };
        let exact: Option<i64> = sqlx::query_scalar("SELECT asset_id FROM asset_path WHERE work_id = ? AND path = ?")
            .bind(work)
            .bind(&path)
            .fetch_optional(&mut *conn)
            .await?;
        if exact == Some(asset_id) {
            continue;
        }
        // The same image in another spelling (`/bilder/a.png` next to `bilder/a.png`) stands there as at the upload.
        if let Some((owner, other)) = assets::path_owner(conn, work, &path).await?
            && (owner != asset_id || exact.is_some())
        {
            run.hint(
                "bild",
                &asset.uuid,
                None,
                format!("Pfad „{path}“ gehört hier schon zum Bild {other}; nicht an {name} übernommen"),
                other,
                name.clone(),
            );
            continue;
        }
        sqlx::query("INSERT INTO asset_path (work_id, path, asset_id) VALUES (?, ?, ?)")
            .bind(work)
            .bind(&path)
            .bind(asset_id)
            .execute(&mut *conn)
            .await?;
    }
    Ok(Some(asset_id))
}

fn title_area_word(area: ExchangeTitleArea) -> &'static str {
    match area {
        ExchangeTitleArea::Top => "top",
        ExchangeTitleArea::Middle => "middle",
        ExchangeTitleArea::Bottom => "bottom",
    }
}

fn source_word(kind: ExchangeSourceKind) -> &'static str {
    match kind {
        ExchangeSourceKind::Commentary => "commentary",
        ExchangeSourceKind::Lexicon => "lexicon",
    }
}

async fn asset_id_by_uuid(
    conn: &mut SqliteConnection,
    work: WorkId,
    uuid: Option<&str>,
) -> Result<Option<i64>, StoreError> {
    let Some(uuid) = uuid else { return Ok(None) };
    Ok(sqlx::query_scalar("SELECT id FROM asset WHERE work_id = ? AND uuid = ?")
        .bind(work)
        .bind(uuid)
        .fetch_optional(&mut *conn)
        .await?)
}

async fn insert_book(
    conn: &mut SqliteConnection,
    work: WorkId,
    book: &baruch_exchange::ExchangeBook,
) -> Result<(), StoreError> {
    let cover = asset_id_by_uuid(conn, work, book.cover.as_deref()).await?;
    let photo = asset_id_by_uuid(conn, work, book.author_photo.as_deref()).await?;
    sqlx::query(
        "INSERT INTO work_book (work_id, cover_asset_id, title_area, front_matter, cover_pages, publisher, place, year, \
         edition, isbn, price, series, printer, bible_notice, image_credit, dedication, blurb, bio, \
         author_photo_asset_id, paper_mm, bleed_mm, updated, publisher_address, printer_address) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(work)
    .bind(cover)
    .bind(title_area_word(book.title_area))
    .bind(i64::from(book.front_matter))
    .bind(i64::from(book.cover_pages))
    .bind(&book.publisher)
    .bind(&book.place)
    .bind(&book.year)
    .bind(&book.edition)
    .bind(&book.isbn)
    .bind(&book.price)
    .bind(&book.series)
    .bind(&book.printer)
    .bind(&book.bible_notice)
    .bind(&book.image_credit)
    .bind(&book.dedication)
    .bind(&book.blurb)
    .bind(&book.bio)
    .bind(photo)
    .bind(book.paper_mm)
    .bind(book.bleed_mm)
    .bind(&book.updated)
    .bind(&book.publisher_address)
    .bind(&book.printer_address)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

// ---- Cover layout and typeset view (Round 27) ----------------------------------------------------

fn cover_block_label(block: &ExchangeCoverBlock) -> String {
    format!("Umschlag-Block {}", block.kind.as_xml())
}

/// Values of a block in the form of the table `cover_block` (for writing and comparing).
#[allow(clippy::type_complexity)]
fn cover_block_columns(
    block: &ExchangeCoverBlock,
) -> (&'static str, &'static str, &'static str, &'static str, Option<&'static str>) {
    let align = match block.align {
        ExchangeCoverAlign::Left => "left",
        ExchangeCoverAlign::Center => "center",
        ExchangeCoverAlign::Right => "right",
        ExchangeCoverAlign::Justify => "justify",
    };
    let style = match block.style {
        ExchangeFontStyle::Normal => "normal",
        ExchangeFontStyle::Bold => "bold",
        ExchangeFontStyle::Italic => "italic",
    };
    let backdrop = match block.backdrop {
        ExchangeBackdrop::None => "none",
        ExchangeBackdrop::Shadow => "shadow",
        ExchangeBackdrop::Gradient => "gradient",
    };
    let direction = block.direction.map(|d| match d {
        ExchangeSpineDirection::Down => "down",
        ExchangeSpineDirection::Up => "up",
    });
    (block.kind.as_db(), align, style, backdrop, direction)
}

/// Normal form of a block of the file (Round 23, review R27): as [`crate::check_cover_block`] produces it when changing
/// in the cover editor – colour in lower case, font name trimmed, custom text with `\n` instead of `\r\n` and `""`
/// instead of "missing". Before, the reading in took blocks over unchecked; a block with whitespace around the font
/// name could afterwards only be changed after cleaning up. If the check rejects the block (for example control
/// characters in the text), it comes back unchanged, plus the message of the check for a notice in the report – the
/// reading in does not abort. (The kind of error of the check is not distinguished, only its text is shown.)
fn normal_cover_block(block: &ExchangeCoverBlock) -> (ExchangeCoverBlock, Option<String>) {
    use baruch_typst::cover_layout::{Backdrop, CoverAlign, FontStyle, SpineDirection};
    let (kind, align, style, backdrop, direction) = cover_block_columns(block);
    let local = (|| {
        Some(crate::CoverBlock {
            kind: crate::CoverBlockKind::parse(kind)?,
            x_mm: block.x_mm,
            y_mm: block.y_mm,
            width_mm: block.width_mm,
            align: CoverAlign::parse(align)?,
            size_pt: block.size_pt,
            leading_em: block.leading_em,
            color: block.color.clone(),
            font: block.font.clone(),
            style: FontStyle::parse(style)?,
            backdrop: Backdrop::parse(backdrop)?,
            visible: block.visible,
            direction: match direction {
                Some(word) => Some(SpineDirection::parse(word)?),
                None => None,
            },
            text: block.text.clone(),
        })
    })();
    let Some(local) = local else {
        return (block.clone(), Some("Art, Ausrichtung, Schnitt oder Hinterlegung unbekannt".to_owned()));
    };
    match crate::check_cover_block(&local) {
        Ok(normal) => {
            let mut out = block.clone();
            out.color = normal.color;
            out.font = normal.font;
            out.text = normal.text;
            out.direction = normal.direction.map(|d| match d {
                SpineDirection::Down => ExchangeSpineDirection::Down,
                SpineDirection::Up => ExchangeSpineDirection::Up,
            });
            (out, None)
        }
        Err(e) => (block.clone(), Some(e.to_string())),
    }
}

/// Notice in the report if a block could not be brought into normal form.
fn cover_block_unchecked(run: &mut Run<'_>, block: &ExchangeCoverBlock, problem: String) {
    run.hint(
        "umschlag-block",
        &block.uuid,
        None,
        format!(
            "{}: unverändert übernommen, obwohl er die Prüfung des Umschlag-Editors nicht besteht ({problem}) – vor \
             dem Ändern im Editor bereinigen",
            cover_block_label(block)
        ),
        String::new(),
        String::new(),
    );
}

/// Create a block with the identifier from the file (`updated` as there; the caller passes the values in normal form,
/// see [`normal_cover_block`]).
async fn insert_cover_block(
    conn: &mut SqliteConnection,
    work: WorkId,
    block: &ExchangeCoverBlock,
) -> Result<(), StoreError> {
    let (kind, align, style, backdrop, direction) = cover_block_columns(block);
    sqlx::query(
        "INSERT INTO cover_block (work_id, kind, x_mm, y_mm, width_mm, align, size_pt, leading_em, color, font, style, \
         backdrop, visible, direction, text, updated, uuid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(work)
    .bind(kind)
    .bind(block.x_mm)
    .bind(block.y_mm)
    .bind(block.width_mm)
    .bind(align)
    .bind(block.size_pt)
    .bind(block.leading_em)
    .bind(&block.color)
    .bind(&block.font)
    .bind(style)
    .bind(backdrop)
    .bind(i64::from(block.visible))
    .bind(direction)
    .bind(&block.text)
    .bind(&block.updated)
    .bind(&block.uuid)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn insert_sheet(conn: &mut SqliteConnection, work: WorkId, sheet: &ExchangeSheet) -> Result<(), StoreError> {
    sqlx::query(
        "INSERT INTO work_sheet (work_id, font, heading_font, size, fill, heading_fill, background, width) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(work)
    .bind(&sheet.font)
    .bind(&sheet.heading_font)
    .bind(&sheet.size)
    .bind(&sheet.fill)
    .bind(&sheet.heading_fill)
    .bind(&sheet.background)
    .bind(&sheet.width)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Check the typeset view settings of the file before writing (as locally: `SheetSettings::normalized`); invalid → not
/// taken over (under „Nicht übernommen“), instead of rejecting the whole reading in.
fn checked_sheet(run: &mut Run<'_>, sheet: &ExchangeSheet) -> Option<ExchangeSheet> {
    let settings = crate::SheetSettings {
        font: sheet.font.clone(),
        heading_font: sheet.heading_font.clone(),
        size: sheet.size.clone(),
        fill: sheet.fill.clone(),
        heading_fill: sheet.heading_fill.clone(),
        background: sheet.background.clone(),
        width: sheet.width.clone(),
    };
    match settings.normalized() {
        Ok(n) if !n.is_empty() => Some(ExchangeSheet {
            font: n.font,
            heading_font: n.heading_font,
            size: n.size,
            fill: n.fill,
            heading_fill: n.heading_fill,
            background: n.background,
            width: n.width,
        }),
        Ok(_) => None,
        Err(e) => {
            run.report.refusals.push(format!("Satzbild-Einstellungen nicht übernommen: {e}"));
            None
        }
    }
}

/// New: all blocks of the file with their identifiers (the book details are already in place).
async fn new_cover(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    let Some(book) = &doc.work.book else { return Ok(()) };
    for block in &book.cover_blocks {
        ensure_free(conn, "cover_block", &block.uuid, "Umschlag-Block").await?;
        let (block, problem) = normal_cover_block(block);
        insert_cover_block(conn, work, &block).await?;
        run.item("umschlag-block", &block.uuid, cover_block_label(&block));
        if let Some(problem) = problem {
            cover_block_unchecked(run, &block, problem);
        }
    }
    Ok(())
}

type CoverBlockRow = (
    Option<WorkId>,
    String,
    f64,
    f64,
    f64,
    String,
    f64,
    f64,
    String,
    Option<String>,
    String,
    String,
    i64,
    Option<String>,
    Option<String>,
);

/// Merging the cover layout (Round 27): see the module documentation.
async fn merge_cover(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    let Some(book) = &doc.work.book else { return Ok(()) };
    for block in &book.cover_blocks {
        // Round 23: the normal form is compared and written – otherwise every merge would report a block as deviating
        // that the reading in itself stored in normal form.
        let (block, problem) = normal_cover_block(block);
        let block = &block;
        let label = cover_block_label(block);
        let known: Option<CoverBlockRow> = sqlx::query_as(
            "SELECT work_id, kind, x_mm, y_mm, width_mm, align, size_pt, leading_em, color, font, style, backdrop, \
             visible, direction, text FROM cover_block WHERE uuid = ?",
        )
        .bind(&block.uuid)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(row) = known {
            if row.0 != Some(work) {
                run.hint(
                    "umschlag-block",
                    &block.uuid,
                    None,
                    format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            let (kind, align, style, backdrop, direction) = cover_block_columns(block);
            let same = row.1 == kind
                && row.2 == block.x_mm
                && row.3 == block.y_mm
                && row.4 == block.width_mm
                && row.5 == align
                && row.6 == block.size_pt
                && row.7 == block.leading_em
                && row.8 == block.color
                && row.9 == block.font
                && row.10 == style
                && row.11 == backdrop
                && (row.12 != 0) == block.visible
                && row.13.as_deref() == direction
                && row.14 == block.text;
            if !same {
                run.hint(
                    "umschlag-block",
                    &block.uuid,
                    None,
                    format!("{label}: weicht ab (bleibt, wie er hier ist)"),
                    "hier".into(),
                    format!("in {}", run.instance),
                );
            }
            continue;
        }
        if run.tombstoned(&block.uuid) {
            run.deleted_here("umschlag-block", &block.uuid, label);
            continue;
        }
        let local: Option<i64> = sqlx::query_scalar("SELECT id FROM cover_block WHERE work_id = ? AND kind = ?")
            .bind(work)
            .bind(block.kind.as_db())
            .fetch_optional(&mut *conn)
            .await?;
        if local.is_some() {
            run.hint(
                "umschlag-block",
                &block.uuid,
                None,
                format!("{label}: hier gibt es schon einen Block dieser Art – lokal gewinnt, nicht übernommen"),
                "vorhanden".into(),
                "neu".into(),
            );
            continue;
        }
        insert_cover_block(conn, work, block).await?;
        run.item("umschlag-block", &block.uuid, label);
        if let Some(problem) = problem {
            cover_block_unchecked(run, block, problem);
        }
    }
    Ok(())
}

/// Merging the typeset view settings: if they are missing here completely, from the file; otherwise a notice on a
/// deviation.
async fn merge_sheet(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let Some(there) = run.doc.work.sheet.clone() else { return Ok(()) };
    let Some(there) = checked_sheet(run, &there) else { return Ok(()) };
    type Field = Option<String>;
    type SheetRow = (Field, Field, Field, Field, Field, Field, Field);
    let here: Option<SheetRow> = sqlx::query_as(
        "SELECT font, heading_font, size, fill, heading_fill, background, width FROM work_sheet WHERE work_id = ?",
    )
    .bind(work)
    .fetch_optional(&mut *conn)
    .await?;
    let uuid = run.doc.work.uuid.clone();
    match here {
        None => {
            insert_sheet(conn, work, &there).await?;
            run.hint(
                "satzbild",
                &uuid,
                None,
                "Satzbild-Einstellungen fehlten hier; aus der Datei angelegt".into(),
                "fehlt".into(),
                "angelegt".into(),
            );
        }
        Some((font, heading_font, size, fill, heading_fill, background, width)) => {
            let local = ExchangeSheet { font, heading_font, size, fill, heading_fill, background, width };
            if local != there {
                run.hint(
                    "satzbild",
                    &uuid,
                    None,
                    "Satzbild-Einstellungen weichen ab (bleiben)".into(),
                    "hier".into(),
                    format!("in {}", run.instance),
                );
            }
        }
    }
    Ok(())
}

// ---- Typesetting settings (Round 29, minor version 6) -------------------------------------------

const TYPESET_LABEL: &str = "Satz-Einstellungen";

/// Check the typesetting settings of the file before writing (as locally: normalised, `TypesetSettings::validate`);
/// invalid → not taken over (under „Nicht übernommen“), instead of rejecting the whole reading in; empty → nothing.
fn checked_typeset(run: &mut Run<'_>, there: &ExchangeTypeset) -> Option<baruch_typst::typeset::TypesetSettings> {
    match crate::typeset::checked(&there.settings) {
        Ok(settings) if !settings.is_empty() => Some(settings),
        Ok(_) => None,
        Err(e) => {
            run.report.refusals.push(format!("{e} – nicht übernommen"));
            None
        }
    }
}

/// Create the row `work_typeset` with the identifier, the point in time and the name from the file.
async fn insert_typeset(
    conn: &mut SqliteConnection,
    work: WorkId,
    there: &ExchangeTypeset,
    settings: &baruch_typst::typeset::TypesetSettings,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO work_typeset (work_id, uuid, settings, updated, updated_by) VALUES (?, ?, ?, ?, ?)")
        .bind(work)
        .bind(&there.uuid)
        .bind(settings.to_json())
        .bind(&there.updated)
        .bind(crate::typeset::by(&there.updated_by))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Merging the typesetting settings (see the module documentation): if they are missing here, from the file (except
/// with a tombstone); otherwise never overwrite, on a deviation a notice with the fields and both states.
async fn merge_typeset(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let Some(there) = run.doc.work.typeset.clone() else { return Ok(()) };
    match crate::typeset::row(conn, work).await? {
        None => {
            if run.tombstoned(&there.uuid) {
                run.deleted_here("satz", &there.uuid, format!("{TYPESET_LABEL} (hier zurückgesetzt)"));
                return Ok(());
            }
            if uuid_exists(conn, "work_typeset", &there.uuid).await? {
                run.hint(
                    "satz",
                    &there.uuid,
                    None,
                    format!("{TYPESET_LABEL}: die Kennung gehört hier zu einem anderen Werk; nicht übernommen"),
                    String::new(),
                    String::new(),
                );
                return Ok(());
            }
            let Some(settings) = checked_typeset(run, &there) else { return Ok(()) };
            insert_typeset(conn, work, &there, &settings).await?;
            run.item("satz", &there.uuid, TYPESET_LABEL.into());
        }
        Some(local) => {
            let Some(settings) = checked_typeset(run, &there) else { return Ok(()) };
            let changed = local.settings.changed_fields(&settings);
            if !changed.is_empty() {
                let labels: Vec<&str> = changed.iter().map(|f| baruch_typst::typeset::field_label(f)).collect();
                run.hint(
                    "satz",
                    &local.uuid,
                    None,
                    format!("{TYPESET_LABEL} weichen ab (bleiben): {}", labels.join(", ")),
                    local.settings.to_json(),
                    settings.to_json(),
                );
            }
        }
    }
    Ok(())
}

// ---- Index and glossary (Round 31, minor version 7) ---------------------------------------------

/// Prepare and check a term of the file as locally (`glossary::clean`); outside the limits → `None` with an entry under
/// „Nicht übernommen“.
fn checked_term(run: &mut Run<'_>, term: &ExchangeGlossaryTerm) -> Option<crate::glossary::Clean> {
    let edit = crate::GlossaryEdit {
        term: term.term.clone(),
        short: term.short.clone(),
        long: term.long.clone(),
        variants: term.variants.clone(),
        source: term.source.clone(),
        bible_refs: term.bible_refs.clone(),
    };
    match crate::glossary::clean(&edit) {
        Ok(clean) => Some(clean),
        Err(e) => {
            run.report.refusals.push(format!("Glossarbegriff {}: {e} – nicht übernommen", quoted(&term.term)));
            None
        }
    }
}

fn glossary_status(status: ExchangeGlossaryStatus) -> crate::GlossaryStatus {
    match status {
        ExchangeGlossaryStatus::Accepted => crate::GlossaryStatus::Accepted,
        ExchangeGlossaryStatus::Proposed => crate::GlossaryStatus::Proposed,
    }
}

/// Create a term with identifier, status, origin and times of the file (together with variants). Round 23: `count` is
/// the number of terms of the work before the creation (proposals counted as well, like `glossary.add`); with the term
/// above [`crate::MAX_GLOSSARY_TERMS`] → `Exchange` with a German message – the transaction of the reading in rolls
/// back, nothing stays written. Before, a large glossary from a file allowed no creation afterwards.
async fn insert_term(
    conn: &mut SqliteConnection,
    work: WorkId,
    term: &ExchangeGlossaryTerm,
    clean: &crate::glossary::Clean,
    count: &mut usize,
) -> Result<(), StoreError> {
    if *count >= crate::MAX_GLOSSARY_TERMS {
        return Err(too_many_terms(*count + 1));
    }
    *count += 1;
    let refs = serde_json::to_string(&clean.bible_refs).map_err(|e| broken(format!("Bibelstellen: {e}")))?;
    let origin: String = term.origin.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO glossary_term (work_id, uuid, term, term_key, short, long, source, bible_refs, status, origin,          created, updated, updated_by) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(work)
    .bind(&term.uuid)
    .bind(&clean.term)
    .bind(&clean.key)
    .bind(&clean.short)
    .bind(&clean.long)
    .bind(&clean.source)
    .bind(refs)
    .bind(glossary_status(term.status).as_db())
    .bind(if origin.is_empty() { "?".to_owned() } else { origin })
    .bind(&term.created)
    .bind(&term.updated)
    .bind(crate::typeset::by(&term.updated_by))
    .fetch_one(&mut *conn)
    .await?;
    crate::glossary::write_variants(conn, id, clean).await
}

async fn insert_register(
    conn: &mut SqliteConnection,
    work: WorkId,
    register: &baruch_exchange::ExchangeRegister,
    position: i64,
) -> Result<(), StoreError> {
    // Round 23: length of the title as when creating in the user interface (`check_u7` and `check_u7_limits` check it
    // as well).
    let title = check_register_title(register)?;
    sqlx::query("INSERT INTO index_register (work_id, uuid, slug, title, position, created) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(work)
        .bind(&register.uuid)
        .bind(&register.slug)
        .bind(title)
        .bind(position)
        .bind(&register.created)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

fn register_label(register: &baruch_exchange::ExchangeRegister) -> String {
    format!("Register {} ({})", quoted(&register.title), register.slug)
}

fn term_label(term: &ExchangeGlossaryTerm) -> String {
    match term.status {
        ExchangeGlossaryStatus::Accepted => format!("Glossarbegriff {}", quoted(&term.term)),
        ExchangeGlossaryStatus::Proposed => {
            format!("Glossarbegriff {} (Vorschlag von {})", quoted(&term.term), term.origin)
        }
    }
}

/// New: all custom indexes and terms of the file with their identifiers.
async fn new_glossary(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    for (position, register) in doc.work.registers.iter().enumerate() {
        ensure_free(conn, "index_register", &register.uuid, "Register").await?;
        insert_register(conn, work, register, position as i64).await?;
        run.item("register", &register.uuid, register_label(register));
    }
    let mut count = 0usize;
    for term in &doc.work.glossary {
        let Some(clean) = checked_term(run, term) else { continue };
        ensure_free(conn, "glossary_term", &term.uuid, "Glossarbegriff").await?;
        insert_term(conn, work, term, &clean, &mut count).await?;
        run.item("glossar-begriff", &term.uuid, term_label(term));
    }
    Ok(())
}

/// Merging indexes and glossary (see the module documentation): complete instead of duplicating, never overwrite.
async fn merge_glossary(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    let mut locals = crate::glossary::register_rows(conn, work).await?;
    for register in &doc.work.registers {
        let label = register_label(register);
        if let Some(local) = locals.iter().find(|r| r.uuid == register.uuid) {
            if local.title != register.title || local.slug != register.slug {
                run.hint(
                    "register",
                    &register.uuid,
                    None,
                    format!("{label} weicht ab (bleibt)"),
                    format!("{} ({})", local.title, local.slug),
                    format!("{} ({})", register.title, register.slug),
                );
            }
            continue;
        }
        if run.tombstoned(&register.uuid) {
            run.deleted_here("register", &register.uuid, label);
            continue;
        }
        if uuid_exists(conn, "index_register", &register.uuid).await? {
            run.hint(
                "register",
                &register.uuid,
                None,
                format!("{label}: Kennung gehört hier zu einem anderen Werk; nicht übernommen"),
                String::new(),
                String::new(),
            );
            continue;
        }
        if let Some(local) = locals.iter().find(|r| r.slug == register.slug) {
            run.hint(
                "register",
                &register.uuid,
                None,
                format!("{label}: ein Register dieser Kennung gibt es hier schon (bleibt, nicht doppelt)"),
                local.title.clone(),
                register.title.clone(),
            );
            continue;
        }
        if locals.len() >= baruch_typst::index::MAX_CUSTOM_REGISTERS {
            run.report.refusals.push(format!(
                "{label}: hier gibt es schon {} eigene Register – nicht übernommen",
                baruch_typst::index::MAX_CUSTOM_REGISTERS
            ));
            continue;
        }
        let position = locals.iter().map(|r| r.position + 1).max().unwrap_or(0);
        insert_register(conn, work, register, position).await?;
        run.item("register", &register.uuid, label);
        locals = crate::glossary::register_rows(conn, work).await?;
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM glossary_term WHERE work_id = ?")
        .bind(work)
        .fetch_one(&mut *conn)
        .await?;
    let mut count = usize::try_from(count).unwrap_or(usize::MAX);
    for term in &doc.work.glossary {
        let label = term_label(term);
        let here: Option<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT term, short, long, source, bible_refs, status FROM glossary_term WHERE work_id = ? AND uuid = ?",
        )
        .bind(work)
        .bind(&term.uuid)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some((name, short, long, source, refs, status)) = here {
            let refs_there = serde_json::to_string(&term.bible_refs).unwrap_or_default();
            let status_there = glossary_status(term.status).as_db();
            if name != term.term
                || short != term.short
                || long != term.long
                || source != term.source
                || refs != refs_there
                || status != status_there
            {
                run.hint(
                    "glossar-begriff",
                    &term.uuid,
                    None,
                    format!("{label} weicht ab (bleibt)"),
                    format!("{name} ({status})"),
                    format!("{} ({status_there})", term.term),
                );
            }
            continue;
        }
        if run.tombstoned(&term.uuid) {
            run.deleted_here("glossar-begriff", &term.uuid, label);
            continue;
        }
        if uuid_exists(conn, "glossary_term", &term.uuid).await? {
            run.hint(
                "glossar-begriff",
                &term.uuid,
                None,
                format!("{label}: Kennung gehört hier zu einem anderen Werk; nicht übernommen"),
                String::new(),
                String::new(),
            );
            continue;
        }
        let Some(clean) = checked_term(run, term) else { continue };
        // Same key here: an accepted one blocks everything, a proposal blocks a further proposal.
        let same: Option<(String, String)> = sqlx::query_as(
            "SELECT term, status FROM glossary_term WHERE work_id = ? AND term_key = ?              ORDER BY status = 'accepted' DESC LIMIT 1",
        )
        .bind(work)
        .bind(&clean.key)
        .fetch_optional(&mut *conn)
        .await?;
        let blocked = match (&same, term.status) {
            (Some((_, status)), _) if status == "accepted" => true,
            (Some(_), ExchangeGlossaryStatus::Proposed) => true,
            _ => false,
        };
        if blocked {
            let (name, status) = same.unwrap_or_default();
            run.hint(
                "glossar-begriff",
                &term.uuid,
                None,
                format!("{label}: diesen Begriff gibt es hier schon (bleibt, nicht doppelt)"),
                format!("{name} ({status})"),
                term.term.clone(),
            );
            continue;
        }
        insert_term(conn, work, term, &clean, &mut count).await?;
        run.item("glossar-begriff", &term.uuid, label);
    }
    Ok(())
}

async fn insert_timing(
    conn: &mut SqliteConnection,
    work: WorkId,
    timing: &baruch_exchange::ExchangeTiming,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO work_timing (work_id, words_per_minute, target_seconds) VALUES (?, ?, ?)")
        .bind(work)
        .bind(i64::from(timing.words_per_minute))
        .bind(timing.target_seconds.map(i64::from))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn insert_sources(conn: &mut SqliteConnection, work: WorkId, doc: &ExchangeDocument) -> Result<(), StoreError> {
    let mut positions: HashMap<&str, i64> = HashMap::new();
    for source in &doc.work.sword_sources {
        let kind = source_word(source.kind);
        let position = positions.entry(kind).or_insert(0);
        sqlx::query("INSERT OR IGNORE INTO work_sword_source (work_id, kind, module, position) VALUES (?, ?, ?, ?)")
            .bind(work)
            .bind(kind)
            .bind(&source.module)
            .bind(*position)
            .execute(&mut *conn)
            .await?;
        *position += 1;
    }
    Ok(())
}

// ---- New ---------------------------------------------------------------------------------------

/// Item of the file in tree order with parent (position), position below the parent and level.
struct Flat<'a> {
    node: &'a ExchangeNode,
    parent: Option<usize>,
    position: usize,
    level: u8,
}

fn flatten<'a>(nodes: &'a [ExchangeNode], parent: Option<usize>, parent_level: Option<u8>, out: &mut Vec<Flat<'a>>) {
    for (position, node) in nodes.iter().enumerate() {
        let level = level_of(node.kind, parent_level);
        let index = out.len();
        out.push(Flat { node, parent, position, level });
        flatten(&node.children, Some(index), Some(level), out);
    }
}

async fn apply_new(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    title: Option<&str>,
) -> Result<(WorkId, String, String), StoreError> {
    let doc = run.doc;
    let w = &doc.work;
    let title = match title.map(str::trim).filter(|t| !t.is_empty()) {
        Some(title) => title.to_owned(),
        None => w.title.trim().to_owned(),
    };
    if title.is_empty() {
        return Err(StoreError::EmptyTitle);
    }
    if !SCHEMES.contains(&w.numbering_scheme.as_str()) {
        return Err(StoreError::Exchange(format!("unbekanntes Gliederungsschema „{}“", w.numbering_scheme)));
    }
    let key = title_key(&title);
    let key = match title_free(conn, &key, None).await {
        Ok(()) => Some(key),
        Err(e @ StoreError::DuplicateTitle { .. }) if run.preview => {
            run.report.refusals.push(format!("{e} Mit einem eigenen Titel (title, CLI --title) einlesen."));
            None
        }
        Err(e) => return Err(e),
    };
    let slug = free_slug(conn, &slug(&title)).await?;
    let kind = match w.kind {
        ExchangeWorkKind::Book => "book",
        ExchangeWorkKind::Sermon => "sermon",
    };
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, created, template, title_key, subtitle, audience, bible, updated, \
         numbering, numbering_scheme, uuid, language) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&slug)
    .bind(kind)
    .bind(&title)
    .bind(&w.author)
    .bind(&w.created)
    .bind(&w.template.current)
    .bind(&key)
    .bind(&w.subtitle)
    .bind(&w.audience)
    .bind(&w.bible)
    .bind(&w.updated)
    .bind(&w.numbering)
    .bind(&w.numbering_scheme)
    .bind(&w.uuid)
    .bind(w.language.as_xml())
    .fetch_one(&mut *conn)
    .await?;

    let mut templates: Vec<_> = w.template.revisions.iter().collect();
    templates.sort_by_key(|r| r.number);
    for revision in templates {
        ensure_free(conn, "work_template_revision", &revision.uuid, "Vorlagenfassung").await?;
        sqlx::query(
            "INSERT INTO work_template_revision (work_id, number, body, author_name, note, created, uuid) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(work)
        .bind(i64::from(revision.number))
        .bind(&revision.body)
        .bind(&revision.author)
        .bind(&revision.note)
        .bind(&revision.created)
        .bind(&revision.uuid)
        .execute(&mut *conn)
        .await?;
        run.item("vorlage", &revision.uuid, format!("Werkvorlage, Fassung {}", revision.number));
    }

    // Outline in tree order; numbers as when loading.
    let mut flat = Vec::new();
    flatten(&w.outline, None, None, &mut flat);
    let counted: Vec<load::Numbered> = flat
        .iter()
        .map(|f| load::Numbered {
            kind: match f.node.kind {
                ExchangeNodeKind::Part => baruch_core::NodeKind::Part,
                ExchangeNodeKind::Chapter => baruch_core::NodeKind::Chapter,
                ExchangeNodeKind::Section => baruch_core::NodeKind::Section,
            },
            parent: f.parent,
            numbered: f.node.numbered,
        })
        .collect();
    let language = crate::WorkLanguage::parse(w.language.as_xml()).unwrap_or_default();
    let numbering = load::work_numbering(&w.numbering_scheme, w.numbering.as_deref(), &w.template.current, language);
    let numbers = load::numbers(&numbering, &counted);
    let mut node_ids: Vec<i64> = Vec::with_capacity(flat.len());
    for (seq, (item, number)) in flat.iter().zip(numbers).enumerate() {
        ensure_free(conn, "node", &item.node.uuid, &format!("Punkt {}", quoted(&item.node.title))).await?;
        // Provisional number not empty, otherwise the trigger from 0006 would take the item for unnumbered.
        let node_id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind, numbered, uuid) \
             VALUES (?, ?, ?, ?, '0', ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(item.parent.map(|p| p as i64))
        .bind(item.position as i64)
        .bind(&item.node.title)
        .bind(node_kind_word(item.level))
        .bind(i64::from(item.node.numbered))
        .bind(&item.node.uuid)
        .fetch_one(&mut *conn)
        .await?;
        sqlx::query("UPDATE node SET number = ? WHERE id = ?").bind(&number).bind(node_id).execute(&mut *conn).await?;
        run.item("punkt", &item.node.uuid, quoted(&item.node.title));
        insert_node_revisions(conn, run, node_id, item.node).await?;
        node_ids.push(node_id);
    }
    // Posts only when all items are in place (references through the position).
    for (item, &node_id) in flat.iter().zip(&node_ids) {
        let mut posts: Vec<&ExchangePost> = item.node.posts.iter().collect();
        posts.sort_by_key(|p| p.position);
        for (position, post) in posts.into_iter().enumerate() {
            ensure_free(conn, "post", &post.uuid, "Beitrag").await?;
            let post_id = insert_post(conn, node_id, position as i64, post).await?;
            insert_refs(conn, run, work, post_id, &post.uuid, &post.refs).await?;
            let label = format!("Beitrag von {} zu {}", post.author, quoted(&item.node.title));
            run.item("beitrag", &post.uuid, label.clone());
            insert_versions(conn, run, VersionOwner::Post(post_id), &post.versions, &label).await?;
        }
    }
    let node_by_uuid: HashMap<&str, i64> =
        flat.iter().zip(&node_ids).map(|(f, &id)| (f.node.uuid.as_str(), id)).collect();
    let mut fresh_ideas: Vec<String> = Vec::new();
    for idea in &w.ideas {
        let node_id = idea.node.as_deref().and_then(|n| node_by_uuid.get(n).copied());
        // A deleted work leaves its ideas behind as ideas without a work (0011, ON DELETE SET NULL); if its backup is
        // played back, the new work gets them again.
        if adopt_orphan_idea(conn, work, node_id, &idea.uuid).await? {
            run.item("einfall", &idea.uuid, format!("{} (Einfall ohne Werk, wieder zugeordnet)", idea_label(idea)));
            fresh_ideas.push(idea.uuid.clone());
            continue;
        }
        ensure_free(conn, "idea", &idea.uuid, "Einfall").await?;
        insert_idea(conn, work, node_id, idea).await?;
        run.item("einfall", &idea.uuid, idea_label(idea));
        fresh_ideas.push(idea.uuid.clone());
    }
    link_idea_parents(conn, run, work, &fresh_ideas).await?;
    for thread in &w.threads {
        ensure_free(conn, "chat_thread", &thread.uuid, "Faden").await?;
        let thread_id = insert_thread(conn, work, thread).await?;
        run.item("faden", &thread.uuid, format!("Faden {}", quoted(&thread.title)));
        let mut posts: Vec<_> = thread.posts.iter().collect();
        posts.sort_by_key(|p| p.position);
        for (position, post) in posts.into_iter().enumerate() {
            ensure_free(conn, "chat_thread_post", &post.uuid, "Fadenbeitrag").await?;
            let post_id = insert_thread_post(conn, thread_id, position as i64, post).await?;
            let label = format!("Faden {}, Beitrag {}", quoted(&thread.title), position + 1);
            run.item("faden-beitrag", &post.uuid, label.clone());
            insert_versions(conn, run, VersionOwner::ThreadPost(post_id), &post.versions, &label).await?;
        }
    }
    for asset in &w.assets {
        insert_asset(conn, run, work, asset).await?;
    }
    new_map(conn, run, work).await?;
    apply_findings(conn, run, work).await?;
    if let Some(book) = &w.book {
        insert_book(conn, work, book).await?;
        new_cover(conn, run, work).await?;
    }
    if let Some(sheet) = &w.sheet
        && let Some(sheet) = checked_sheet(run, sheet)
    {
        insert_sheet(conn, work, &sheet).await?;
    }
    if let Some(there) = &w.typeset
        && let Some(settings) = checked_typeset(run, there)
    {
        ensure_free(conn, "work_typeset", &there.uuid, "Satz-Einstellungen").await?;
        insert_typeset(conn, work, there, &settings).await?;
        run.item("satz", &there.uuid, TYPESET_LABEL.into());
    }
    new_glossary(conn, run, work).await?;
    if let Some(timing) = &w.timing {
        insert_timing(conn, work, timing).await?;
    }
    if let Some(module) = &w.bible_module {
        sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, ?)")
            .bind(work)
            .bind(module)
            .execute(&mut *conn)
            .await?;
    }
    insert_sources(conn, work, doc).await?;
    for tombstone in &w.deleted {
        sqlx::query("INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid, deleted) VALUES (?, ?, ?, ?)")
            .bind(&tombstone.uuid)
            .bind(tombstone.kind.as_db())
            .bind(&w.uuid)
            .bind(&tombstone.deleted)
            .execute(&mut *conn)
            .await?;
    }
    Ok((work, slug, title))
}

async fn insert_thread(
    conn: &mut SqliteConnection,
    work: WorkId,
    thread: &baruch_exchange::ExchangeThread,
) -> Result<i64, StoreError> {
    Ok(sqlx::query_scalar(
        "INSERT INTO chat_thread (work_id, title, created_by, created, updated, uuid) VALUES (?, ?, ?, ?, ?, ?) \
         RETURNING id",
    )
    .bind(work)
    .bind(&thread.title)
    .bind(&thread.created_by)
    .bind(&thread.created)
    .bind(&thread.updated)
    .bind(&thread.uuid)
    .fetch_one(&mut *conn)
    .await?)
}

// ---- Merging -----------------------------------------------------------------------------------

/// An item here: fixed key, parent (key), position below the parent, level, title, number on.
#[derive(Debug, Clone)]
struct LocalNode {
    key: i64,
    parent: Option<i64>,
    position: i64,
    level: u8,
    title: String,
    numbered: bool,
}

/// Outline here: identifier → item.
async fn local_nodes(conn: &mut SqliteConnection, work: WorkId) -> Result<HashMap<String, LocalNode>, StoreError> {
    type Row = (i64, i64, Option<i64>, i64, String, String, i64, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, seq, parent_seq, position, title, kind, numbered, uuid FROM node WHERE work_id = ? ORDER BY seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let by_seq: HashMap<i64, usize> = rows.iter().enumerate().map(|(i, r)| (r.1, i)).collect();
    let mut levels: Vec<Option<u8>> = vec![None; rows.len()];
    fn level(
        rows: &[Row],
        by_seq: &HashMap<i64, usize>,
        levels: &mut Vec<Option<u8>>,
        index: usize,
        depth: usize,
    ) -> u8 {
        if let Some(level) = levels[index] {
            return level;
        }
        let parent = rows[index].2.and_then(|p| by_seq.get(&p).copied()).filter(|_| depth < 8);
        let parent_level = parent.map(|p| level(rows, by_seq, levels, p, depth + 1));
        let value = match rows[index].5.as_str() {
            "part" => 0,
            "chapter" => 1,
            _ => parent_level.map_or(2, |p| (p + 1).clamp(2, 3)),
        };
        levels[index] = Some(value);
        value
    }
    let mut out = HashMap::new();
    for index in 0..rows.len() {
        let value = level(&rows, &by_seq, &mut levels, index, 0);
        let row = &rows[index];
        let Some(uuid) = row.7.clone() else { continue };
        let parent = row.2.and_then(|p| by_seq.get(&p)).map(|&i| rows[i].0);
        out.insert(
            uuid,
            LocalNode { key: row.0, parent, position: row.3, level: value, title: row.4.clone(), numbered: row.6 != 0 },
        );
    }
    Ok(out)
}

/// A revision here.
/// Number of a revision of this item with exactly this text (equality of content, not identifier).
fn same_text_number(revisions: &HashMap<String, LocalRevision>, node: i64, body: &str) -> Option<u32> {
    revisions.values().filter(|r| r.node == node && r.body == body).map(|r| r.number).min()
}

fn same_text_here(revisions: &HashMap<String, LocalRevision>, node: i64, body: &str) -> bool {
    same_text_number(revisions, node, body).is_some()
}

#[derive(Debug, Clone)]
struct LocalRevision {
    node: i64,
    number: u32,
    body: String,
    note: String,
    status: String,
}

async fn local_revisions(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<HashMap<String, LocalRevision>, StoreError> {
    let rows: Vec<(i64, i64, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT r.node_id, r.number, r.body, r.note, r.status, r.uuid FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ?",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|(node, number, body, note, status, uuid)| {
            Some((uuid?, LocalRevision { node, number: u32::try_from(number).ok()?, body, note, status }))
        })
        .collect())
}

async fn last_rev(conn: &mut SqliteConnection, node_id: i64) -> Result<u32, StoreError> {
    let last: i64 =
        sqlx::query_scalar("SELECT last_rev FROM node WHERE id = ?").bind(node_id).fetch_one(&mut *conn).await?;
    u32::try_from(last).map_err(|_| broken(format!("last_rev außerhalb des Bereichs: {last}")))
}

/// Item of the file in tree order with parent (identifier), siblings (identifiers) and level.
struct FileNode<'a> {
    node: &'a ExchangeNode,
    parent: Option<&'a str>,
    siblings: Vec<&'a str>,
    index: usize,
    level: u8,
}

fn file_nodes<'a>(
    nodes: &'a [ExchangeNode],
    parent: Option<&'a ExchangeNode>,
    parent_level: Option<u8>,
    out: &mut Vec<FileNode<'a>>,
) {
    let siblings: Vec<&str> = nodes.iter().map(|n| n.uuid.as_str()).collect();
    for (index, node) in nodes.iter().enumerate() {
        let level = level_of(node.kind, parent_level);
        out.push(FileNode { node, parent: parent.map(|p| p.uuid.as_str()), siblings: siblings.clone(), index, level });
        file_nodes(&node.children, Some(node), Some(level), out);
    }
}

async fn apply_merge(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    let mut files = Vec::new();
    file_nodes(&doc.work.outline, None, None, &mut files);
    let in_file: HashSet<&str> = files.iter().map(|f| f.node.uuid.as_str()).collect();

    // (a) Create unknown items at their position, with exactly their revisions.
    let before = local_nodes(conn, work).await?;
    // Identifier → (key, parent, level) for everything that stands here (existing or just created).
    let mut placed: HashMap<&str, (i64, Option<i64>, u8)> = HashMap::new();
    let mut created: HashSet<&str> = HashSet::new();
    for file in &files {
        let node = file.node;
        if let Some(local) = before.get(&node.uuid) {
            placed.insert(node.uuid.as_str(), (local.key, local.parent, local.level));
            continue;
        }
        let label = format!("Punkt {}", quoted(&node.title));
        if run.tombstoned(&node.uuid) {
            run.deleted_here("punkt", &node.uuid, label);
            continue;
        }
        if uuid_exists(conn, "node", &node.uuid).await? {
            run.hint(
                "punkt",
                &node.uuid,
                None,
                format!("{label}: die Kennung gehört hier zu einem anderen Werk; nicht übernommen"),
                String::new(),
                String::new(),
            );
            continue;
        }
        let parent = match file.parent {
            None => None,
            Some(parent) => {
                match placed.get(parent) {
                    Some(&(key, _, level)) => Some((key, level)),
                    None => {
                        run.hint(
                        "punkt",
                        &node.uuid,
                        None,
                        format!("{label}: sein Elternteil gibt es hier nicht; nicht angelegt (Fassungen bleiben außen vor)"),
                        "fehlt".into(),
                        quoted(&node.title),
                    );
                        continue;
                    }
                }
            }
        };
        let parent_key = parent.map(|(key, _)| key);
        let after = file.siblings[..file.index]
            .iter()
            .rev()
            .find_map(|uuid| placed.get(uuid).filter(|(_, p, _)| *p == parent_key).map(|(key, ..)| *key));
        let level = level_of(node.kind, parent.map(|(_, level)| level));
        match outline::insert_bare(conn, work, after, parent_key, level, &node.title, node.numbered).await? {
            BareInsert::Inserted(key) => {
                sqlx::query("UPDATE node SET uuid = ? WHERE id = ?")
                    .bind(&node.uuid)
                    .bind(key)
                    .execute(&mut *conn)
                    .await?;
                run.item("punkt", &node.uuid, quoted(&node.title));
                insert_node_revisions(conn, run, key, node).await?;
                placed.insert(node.uuid.as_str(), (key, parent_key, level));
                created.insert(node.uuid.as_str());
            }
            BareInsert::WouldAdopt { adopted, anchor, place } => {
                let points = [NewPoint { title: node.title.clone(), level: Some(0) }];
                // Round 22 (contract §2.10): identifier and revisions travel along in the request.
                let there = format!(
                    "neuer Teil, unter den {} vorhandene Kapitel wandern würden; Antrag mit Kennung und {} Fassung(en)",
                    adopted.len(),
                    node.revisions.len()
                );
                let identity = Some(part_identity(node));
                match requests::add_request_in(conn, work, anchor, place, &points, &run.origin, "", &run.note, identity)
                    .await
                {
                    Ok(Some(request)) => {
                        run.report.requests.push(PlannedRequest {
                            op: request.op.name(),
                            key: anchor,
                            label: request.summary.clone(),
                        });
                        run.request_ids.push(request.id);
                        run.deviation(
                            "punkt",
                            &node.uuid,
                            Some(anchor),
                            None,
                            label,
                            "fehlt".into(),
                            there,
                            DeviationAction::Antrag,
                        );
                    }
                    Ok(None) => {
                        run.hint("punkt", &node.uuid, None, format!("{label}: nicht angelegt"), "fehlt".into(), there)
                    }
                    Err(e) if is_refusal(&e) => {
                        run.hint(
                            "punkt",
                            &node.uuid,
                            None,
                            format!("{label}: kein Antrag ({e})"),
                            "fehlt".into(),
                            there,
                        );
                    }
                    Err(e) => return Err(e),
                }
            }
            BareInsert::Misplaced(message) => {
                run.hint(
                    "punkt",
                    &node.uuid,
                    None,
                    format!("{label}: nicht angelegt – {message}"),
                    "fehlt".into(),
                    quoted(&node.title),
                );
            }
        }
    }

    // (c)/(d) Revisions of known items.
    let mut revisions_here = local_revisions(conn, work).await?;
    // Cross-references of the file point to keys of the foreign instance: before the comparison and when creating a
    // deviating revision, re-key them to the keys here (otherwise every revision with a reference would count as
    // deviating, and the derived revision would keep the foreign keys – the follow-up pass `apply_u5` does not reach
    // it, because it searches through the identifier of the original revision).
    let key_map: HashMap<i64, i64> = {
        let ids: HashMap<String, i64> =
            sqlx::query_as::<_, (String, i64)>("SELECT uuid, id FROM node WHERE work_id = ? AND uuid IS NOT NULL")
                .bind(work)
                .fetch_all(&mut *conn)
                .await?
                .into_iter()
                .collect();
        files
            .iter()
            .filter_map(|f| Some((f.node.key?, *ids.get(&f.node.uuid)?)))
            .filter(|(old, new)| old != new)
            .collect()
    };
    for file in &files {
        let node = file.node;
        if created.contains(node.uuid.as_str()) {
            continue;
        }
        let Some(local) = before.get(&node.uuid) else { continue };
        let key = local.key;
        let mut sorted: Vec<&ExchangeRevision> = node.revisions.iter().collect();
        sorted.sort_by_key(|r| r.number);
        let mut numbers: HashMap<&str, u32> = HashMap::new();
        for revision in sorted {
            let label = format!("{}, Fassung {}", quoted(&local.title), revision.number);
            if let Some(here) = revisions_here.get(&revision.uuid).cloned() {
                if here.node != key {
                    run.hint(
                        "fassung",
                        &revision.uuid,
                        Some(key),
                        format!("{label}: gehört hier zu einem anderen Punkt"),
                        String::new(),
                        String::new(),
                    );
                    continue;
                }
                numbers.insert(revision.uuid.as_str(), here.number);
                let body_here = baruch_typst::xref::remap_keys(&revision.body, &key_map);
                // Round 22: known revision whose amendments here are a prefix of those read in → fast-forward.
                if here.body != *body_here
                    && fast_forward(conn, run, key, &local.title, here.number, revision, &body_here, &key_map).await?
                {
                    if let Some(entry) = revisions_here.get_mut(&revision.uuid) {
                        entry.body = body_here.clone().into_owned();
                    }
                    continue;
                }
                if here.body != *body_here {
                    let derived = derive_uuid(&revision.uuid, &revision.body);
                    if revisions_here.contains_key(&derived) || same_text_here(&revisions_here, key, &body_here) {
                        continue;
                    }
                    if run.tombstoned(&derived) {
                        run.deleted_here("fassung", &derived, format!("{label} (abweichende Fassung)"));
                        continue;
                    }
                    if uuid_exists(conn, "revision", &derived).await? {
                        run.hint(
                            "fassung",
                            &derived,
                            Some(key),
                            format!("{label}: abgeleitete Kennung gehört hier zu einem anderen Werk"),
                            String::new(),
                            String::new(),
                        );
                        continue;
                    }
                    let number = last_rev(conn, key).await? + 1;
                    let suffix = format!("(Austausch: abweichende Fassung aus {})", run.instance);
                    let note =
                        if revision.note.trim().is_empty() { suffix } else { format!("{} {suffix}", revision.note) };
                    let status = merged_status(revision.status);
                    let remapped = ExchangeRevision { body: body_here.clone().into_owned(), ..revision.clone() };
                    insert_revision(conn, key, &remapped, number, Some(here.number), status, &note, &derived).await?;
                    revisions_here.insert(
                        derived.clone(),
                        LocalRevision {
                            node: key,
                            number,
                            body: remapped.body.clone(),
                            note: note.clone(),
                            status: status.into(),
                        },
                    );
                    run.item(
                        "fassung",
                        &derived,
                        format!("{}, Fassung {number} (abweichend, aus Fassung {})", quoted(&local.title), here.number),
                    );
                    run.deviation(
                        "fassung",
                        &derived,
                        Some(key),
                        Some(number),
                        format!(
                            "{}: Fassung {} weicht ab, kommt als Fassung {number}",
                            quoted(&local.title),
                            here.number
                        ),
                        excerpt(&here.body),
                        excerpt(&body_here),
                        DeviationAction::Fassung,
                    );
                } else {
                    if here.note != revision.note {
                        run.deviation(
                            "fassung",
                            &revision.uuid,
                            Some(key),
                            Some(here.number),
                            format!("{label}: Notiz weicht ab"),
                            here.note.clone(),
                            revision.note.clone(),
                            DeviationAction::Hinweis,
                        );
                    }
                    let current_here = here.status == "current";
                    let current_there = revision.status == ExchangeRevisionStatus::Current;
                    if current_here != current_there {
                        let (h, t) =
                            if current_there { ("nicht aktuell", "aktuell") } else { ("aktuell", "nicht aktuell") };
                        run.deviation(
                            "fassung",
                            &revision.uuid,
                            Some(key),
                            Some(here.number),
                            format!("{label}: in {} {t} (Umschalten nur hier durch dich)", run.instance),
                            h.into(),
                            t.into(),
                            DeviationAction::Hinweis,
                        );
                    }
                }
                continue;
            }
            if run.tombstoned(&revision.uuid) {
                run.deleted_here("fassung", &revision.uuid, label);
                continue;
            }
            // Same text as a revision of this item (such as the derived revision of one edited in place here): create
            // nothing, otherwise a duplication would arise when merging back and forth.
            let body_here = baruch_typst::xref::remap_keys(&revision.body, &key_map);
            if let Some(number) = same_text_number(&revisions_here, key, &body_here) {
                numbers.insert(revision.uuid.as_str(), number);
                continue;
            }
            if uuid_exists(conn, "revision", &revision.uuid).await? {
                run.hint(
                    "fassung",
                    &revision.uuid,
                    Some(key),
                    format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            let based_on = revision.based_on.as_deref().and_then(|b| numbers.get(b).copied());
            let number = last_rev(conn, key).await? + 1;
            let status = merged_status(revision.status);
            insert_revision(conn, key, revision, number, based_on, status, &revision.note, &revision.uuid).await?;
            numbers.insert(revision.uuid.as_str(), number);
            revisions_here.insert(
                revision.uuid.clone(),
                LocalRevision {
                    node: key,
                    number,
                    body: body_here.into_owned(),
                    note: revision.note.clone(),
                    status: status.into(),
                },
            );
            run.item(
                "fassung",
                &revision.uuid,
                format!("{}, Fassung {number} (dort {})", quoted(&local.title), revision.number),
            );
        }
    }

    // (b) Outline of known items: requests, never directly.
    let now = local_nodes(conn, work).await?;
    let mut children: HashMap<Option<i64>, Vec<(i64, i64)>> = HashMap::new();
    let mut uuid_of: HashMap<i64, &str> = HashMap::new();
    for (uuid, local) in &now {
        children.entry(local.parent).or_default().push((local.position, local.key));
        uuid_of.insert(local.key, uuid.as_str());
    }
    for list in children.values_mut() {
        list.sort_unstable();
    }
    for file in &files {
        let node = file.node;
        if created.contains(node.uuid.as_str()) {
            continue;
        }
        let Some(local) = now.get(&node.uuid) else { continue };
        let key = local.key;
        let label = format!("Punkt {}", quoted(&local.title));
        // Title.
        if local.title != node.title {
            let bodies: Vec<(i64, String)> =
                sqlx::query_as("SELECT number, body FROM revision WHERE node_id = ? ORDER BY number")
                    .bind(key)
                    .fetch_all(&mut *conn)
                    .await?;
            let carrying =
                bodies.iter().find(|(_, body)| outline::heading_title(body).as_deref() == Some(node.title.as_str()));
            match carrying {
                Some((number, _)) => run.deviation(
                    "punkt",
                    &node.uuid,
                    Some(key),
                    u32::try_from(*number).ok(),
                    format!(
                        "{label}: Fassung {number} trägt den Titel {} schon – Umschalten genügt",
                        quoted(&node.title)
                    ),
                    local.title.clone(),
                    node.title.clone(),
                    DeviationAction::Hinweis,
                ),
                None => {
                    let op = crate::OutlineOp::Rename { key, title: node.title.clone() };
                    request(
                        conn,
                        run,
                        work,
                        op,
                        &node.uuid,
                        key,
                        format!("{label}: anderer Titel"),
                        local.title.clone(),
                        node.title.clone(),
                    )
                    .await?;
                }
            }
        }
        // Level.
        if local.level != file.level {
            if (1..=3).contains(&local.level) && (1..=3).contains(&file.level) {
                let op = crate::OutlineOp::SetLevel { key, level: file.level };
                request(
                    conn,
                    run,
                    work,
                    op,
                    &node.uuid,
                    key,
                    format!("{label}: andere Ebene"),
                    local.level.to_string(),
                    file.level.to_string(),
                )
                .await?;
            } else {
                run.hint(
                    "punkt",
                    &node.uuid,
                    Some(key),
                    format!("{label}: andere Ebene (Teil ↔ Kapitel geht nicht als Antrag)"),
                    local.level.to_string(),
                    file.level.to_string(),
                );
            }
        }
        // Number on/off.
        if local.numbered != node.numbered {
            let op = crate::OutlineOp::SetNumbered { key, numbered: node.numbered };
            let word = |n: bool| if n { "mit Nummer" } else { "ohne Nummer" };
            request(
                conn,
                run,
                work,
                op,
                &node.uuid,
                key,
                format!("{label}: Nummer an/aus"),
                word(local.numbered).into(),
                word(node.numbered).into(),
            )
            .await?;
        }
        // Place: parent and neighbours among the items that both sides know.
        let intended = match file.parent {
            None => Some(None),
            Some(parent) => now.get(parent).map(|p| Some(p.key)),
        };
        let Some(intended) = intended else { continue };
        let known_sibling = |uuid: &&str| now.get(*uuid).filter(|n| n.parent == intended).map(|n| n.key);
        let file_prev = file.siblings[..file.index].iter().rev().find_map(known_sibling);
        let file_next = file.siblings[file.index + 1..].iter().find_map(known_sibling);
        let local_prev = children
            .get(&local.parent)
            .map(|list| {
                list.iter()
                    .take_while(|(_, k)| *k != key)
                    .filter(|(_, k)| uuid_of.get(k).is_some_and(|u| file.siblings.contains(u)))
                    .map(|(_, k)| *k)
                    .last()
            })
            .unwrap_or(None);
        if local.parent == intended && local_prev == file_prev {
            continue;
        }
        let (target, place) = match (file_prev, file_next, intended) {
            (Some(prev), _, _) => (prev, crate::Place::After),
            (None, Some(next), _) => (next, crate::Place::Before),
            (None, None, Some(parent)) => (parent, crate::Place::Into),
            (None, None, None) => {
                run.hint(
                    "punkt",
                    &node.uuid,
                    Some(key),
                    format!("{label}: anderer Ort, aber kein Bezugspunkt für einen Antrag"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
        };
        let op = crate::OutlineOp::Move { key, target_key: target, place };
        request(
            conn,
            run,
            work,
            op,
            &node.uuid,
            key,
            format!("{label}: anderer Ort"),
            "hier".into(),
            format!("in {}", run.instance),
        )
        .await?;
    }
    for (uuid, local) in &before {
        if in_file.contains(uuid.as_str()) {
            continue;
        }
        let there =
            if doc.work.deleted.iter().any(|t| &t.uuid == uuid) { "dort gelöscht" } else { "fehlt in der Datei" };
        run.hint(
            "punkt",
            uuid,
            Some(local.key),
            format!("Punkt {}: {there} (kein Antrag zum Löschen)", quoted(&local.title)),
            quoted(&local.title),
            there.into(),
        );
    }

    merge_template(conn, run, work).await?;
    merge_posts(conn, run, work, &files).await?;
    merge_ideas(conn, run, work).await?;
    merge_map(conn, run, work).await?;
    merge_threads(conn, run, work).await?;
    apply_findings(conn, run, work).await?;
    merge_assets(conn, run, work).await?;
    merge_details(conn, run, work).await?;
    merge_glossary(conn, run, work).await?;

    // Tombstones of the file: what is deleted there and lives here stays (only a notice).
    for tombstone in &doc.work.deleted {
        if matches!(tombstone.kind, ExchangeTombstoneKind::Node | ExchangeTombstoneKind::Work) {
            continue; // items above, the work is this one
        }
        if uuid_exists(conn, tombstone_table(tombstone.kind), &tombstone.uuid).await? {
            run.hint(
                tombstone.kind.as_xml(),
                &tombstone.uuid,
                None,
                format!("In {} gelöscht ({}), hier vorhanden – bleibt", run.instance, tombstone.deleted),
                "vorhanden".into(),
                "gelöscht".into(),
            );
        }
    }
    Ok(())
}

/// File a request; if the trial run does not work or it already exists as open, only the deviation as a notice.
#[allow(clippy::too_many_arguments)]
async fn request(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    work: WorkId,
    op: crate::OutlineOp,
    uuid: &str,
    key: i64,
    label: String,
    here: String,
    there: String,
) -> Result<(), StoreError> {
    match requests::outline_request_in(conn, work, op, &run.origin, &run.note).await {
        Ok(request) => {
            run.report.requests.push(PlannedRequest { op: request.op.name(), key, label: request.summary.clone() });
            run.request_ids.push(request.id);
            run.deviation("punkt", uuid, Some(key), None, label, here, there, DeviationAction::Antrag);
            Ok(())
        }
        Err(e) if is_refusal(&e) => {
            run.deviation(
                "punkt",
                uuid,
                Some(key),
                None,
                format!("{label} (kein Antrag: {e})"),
                here,
                there,
                DeviationAction::Hinweis,
            );
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// (e) Append template revisions with an unknown identifier; `work.template` stays.
async fn merge_template(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    let rows: Vec<(Option<String>, i64, String)> =
        sqlx::query_as("SELECT uuid, number, body FROM work_template_revision WHERE work_id = ?")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?;
    let here: HashMap<String, (i64, String)> =
        rows.into_iter().filter_map(|(uuid, number, body)| Some((uuid?, (number, body)))).collect();
    let mut sorted: Vec<_> = doc.work.template.revisions.iter().collect();
    sorted.sort_by_key(|r| r.number);
    for revision in sorted {
        let label = format!("Werkvorlage, Fassung {}", revision.number);
        if let Some((number, body)) = here.get(&revision.uuid) {
            if *body != revision.body {
                run.hint(
                    "vorlage",
                    &revision.uuid,
                    None,
                    format!("Werkvorlage, Fassung {number}: Text weicht ab"),
                    excerpt(body),
                    excerpt(&revision.body),
                );
            }
            continue;
        }
        if run.tombstoned(&revision.uuid) {
            run.deleted_here("vorlage", &revision.uuid, label);
            continue;
        }
        if uuid_exists(conn, "work_template_revision", &revision.uuid).await? {
            run.hint(
                "vorlage",
                &revision.uuid,
                None,
                format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                String::new(),
                String::new(),
            );
            continue;
        }
        let next: i64 =
            sqlx::query_scalar("SELECT coalesce(max(number), 0) + 1 FROM work_template_revision WHERE work_id = ?")
                .bind(work)
                .fetch_one(&mut *conn)
                .await?;
        sqlx::query(
            "INSERT INTO work_template_revision (work_id, number, body, author_name, note, created, uuid) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(work)
        .bind(next)
        .bind(&revision.body)
        .bind(&revision.author)
        .bind(&revision.note)
        .bind(&revision.created)
        .bind(&revision.uuid)
        .execute(&mut *conn)
        .await?;
        run.item("vorlage", &revision.uuid, format!("Werkvorlage, Fassung {next} (dort {})", revision.number));
    }
    let current: String =
        sqlx::query_scalar("SELECT template FROM work WHERE id = ?").bind(work).fetch_one(&mut *conn).await?;
    if current != doc.work.template.current {
        run.hint(
            "vorlage",
            &doc.work.uuid,
            None,
            "Aktuelle Werkvorlage weicht ab (bleibt; Fassung wählen geht hier)".into(),
            excerpt(&current),
            excerpt(&doc.work.template.current),
        );
    }
    Ok(())
}

/// (f) Attach posts (together with references) with an unknown identifier to the items that exist here.
async fn merge_posts(
    conn: &mut SqliteConnection,
    run: &mut Run<'_>,
    work: WorkId,
    files: &[FileNode<'_>],
) -> Result<(), StoreError> {
    for file in files {
        let node = file.node;
        if node.posts.is_empty() {
            continue;
        }
        let local: Option<(i64, String)> = sqlx::query_as("SELECT id, title FROM node WHERE work_id = ? AND uuid = ?")
            .bind(work)
            .bind(&node.uuid)
            .fetch_optional(&mut *conn)
            .await?;
        let Some((node_id, title)) = local else {
            for post in &node.posts {
                if !run.tombstoned(&post.uuid) {
                    run.hint(
                        "beitrag",
                        &post.uuid,
                        None,
                        format!("Beitrag zu {}: der Punkt fehlt hier; nicht übernommen", quoted(&node.title)),
                        String::new(),
                        excerpt(&post.body),
                    );
                }
            }
            continue;
        };
        let rows: Vec<(i64, Option<String>, String)> =
            sqlx::query_as("SELECT id, uuid, body FROM post WHERE node_id = ?")
                .bind(node_id)
                .fetch_all(&mut *conn)
                .await?;
        let here: HashMap<String, (i64, String)> =
            rows.into_iter().filter_map(|(id, u, b)| Some((u?, (id, b)))).collect();
        let mut sorted: Vec<&ExchangePost> = node.posts.iter().collect();
        sorted.sort_by_key(|p| p.position);
        for post in sorted {
            let label = format!("Beitrag von {} zu {}", post.author, quoted(&title));
            if let Some((id, body)) = here.get(&post.uuid) {
                if *body != post.body {
                    run.hint(
                        "beitrag",
                        &post.uuid,
                        Some(node_id),
                        format!("{label}: Text weicht ab"),
                        excerpt(body),
                        excerpt(&post.body),
                    );
                }
                insert_versions(conn, run, VersionOwner::Post(*id), &post.versions, &label).await?;
                continue;
            }
            if run.tombstoned(&post.uuid) {
                run.deleted_here("beitrag", &post.uuid, label);
                continue;
            }
            if uuid_exists(conn, "post", &post.uuid).await? {
                run.hint(
                    "beitrag",
                    &post.uuid,
                    None,
                    format!("{label}: die Kennung gehört hier zu einem anderen Punkt"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            let position: i64 = sqlx::query_scalar("SELECT coalesce(max(position) + 1, 0) FROM post WHERE node_id = ?")
                .bind(node_id)
                .fetch_one(&mut *conn)
                .await?;
            let post_id = insert_post(conn, node_id, position, post).await?;
            insert_refs(conn, run, work, post_id, &post.uuid, &post.refs).await?;
            run.item("beitrag", &post.uuid, label.clone());
            insert_versions(conn, run, VersionOwner::Post(post_id), &post.versions, &label).await?;
        }
    }
    Ok(())
}

/// (f) Ideas with an unknown identifier; known ones with other content only as a notice.
async fn merge_ideas(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    type Row = (i64, Option<String>, String, String, String, String, String, Option<String>, String);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, uuid, note, quote, source, bible, status, deleted, title FROM idea WHERE work_id = ?",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut fresh_ideas: Vec<String> = Vec::new();
    let mut here: HashMap<String, Row> = HashMap::new();
    for row in rows {
        if let Some(uuid) = row.1.clone() {
            here.insert(uuid, row);
        }
    }
    for idea in &doc.work.ideas {
        let label = idea_label(idea);
        if let Some(row) = here.get(&idea.uuid) {
            let words: Vec<String> =
                sqlx::query_scalar("SELECT word FROM idea_word WHERE idea_id = ? ORDER BY position")
                    .bind(row.0)
                    .fetch_all(&mut *conn)
                    .await?;
            let status = match idea.status {
                ExchangeIdeaStatus::Open => "open",
                ExchangeIdeaStatus::Done => "done",
            };
            let mut differs = Vec::new();
            if row.2 != idea.note {
                differs.push("Notiz");
            }
            if row.3 != idea.quote {
                differs.push("Zitat");
            }
            if row.4 != idea.source {
                differs.push("Quelle");
            }
            if row.5 != idea.bible {
                differs.push("Bibelstelle");
            }
            if row.6 != status {
                differs.push("Status");
            }
            if row.7.is_some() != idea.deleted.is_some() {
                differs.push("gelöscht");
            }
            if words != idea.words {
                differs.push("Schlagwörter");
            }
            // Round 34: the title counts as content; parent node and order are arrangement and stay here.
            if row.8 != idea.title {
                differs.push("Titel");
            }
            if !differs.is_empty() {
                run.hint(
                    "einfall",
                    &idea.uuid,
                    None,
                    format!("{label}: {} weicht ab (bleibt)", differs.join(", ")),
                    excerpt(&row.2),
                    excerpt(&idea.note),
                );
            }
            continue;
        }
        if run.tombstoned(&idea.uuid) {
            run.deleted_here("einfall", &idea.uuid, label);
            continue;
        }
        let node_id: Option<i64> = match &idea.node {
            Some(uuid) => {
                sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND uuid = ?")
                    .bind(work)
                    .bind(uuid)
                    .fetch_optional(&mut *conn)
                    .await?
            }
            None => None,
        };
        if adopt_orphan_idea(conn, work, node_id, &idea.uuid).await? {
            run.item("einfall", &idea.uuid, format!("{label} (Einfall ohne Werk, wieder zugeordnet)"));
            fresh_ideas.push(idea.uuid.clone());
            continue;
        }
        if uuid_exists(conn, "idea", &idea.uuid).await? {
            run.hint(
                "einfall",
                &idea.uuid,
                None,
                format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                String::new(),
                String::new(),
            );
            continue;
        }
        insert_idea(conn, work, node_id, idea).await?;
        run.item("einfall", &idea.uuid, label);
        fresh_ideas.push(idea.uuid.clone());
    }
    link_idea_parents(conn, run, work, &fresh_ideas).await?;
    Ok(())
}

/// (f) Threads and thread posts with an unknown identifier.
async fn merge_threads(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    for thread in &doc.work.threads {
        let label = format!("Faden {}", quoted(&thread.title));
        let local: Option<(i64, String)> =
            sqlx::query_as("SELECT id, title FROM chat_thread WHERE work_id = ? AND uuid = ?")
                .bind(work)
                .bind(&thread.uuid)
                .fetch_optional(&mut *conn)
                .await?;
        let mut sorted: Vec<_> = thread.posts.iter().collect();
        sorted.sort_by_key(|p| p.position);
        let thread_id = match local {
            Some((id, title)) => {
                if title != thread.title {
                    run.hint(
                        "faden",
                        &thread.uuid,
                        None,
                        format!("{label}: Titel weicht ab (bleibt)"),
                        title,
                        thread.title.clone(),
                    );
                }
                id
            }
            None => {
                if run.tombstoned(&thread.uuid) {
                    run.deleted_here("faden", &thread.uuid, label);
                    continue;
                }
                if uuid_exists(conn, "chat_thread", &thread.uuid).await? {
                    run.hint(
                        "faden",
                        &thread.uuid,
                        None,
                        format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                        String::new(),
                        String::new(),
                    );
                    continue;
                }
                let id = insert_thread(conn, work, thread).await?;
                run.item("faden", &thread.uuid, label.clone());
                id
            }
        };
        let rows: Vec<(i64, Option<String>, String)> =
            sqlx::query_as("SELECT id, uuid, body FROM chat_thread_post WHERE thread_id = ?")
                .bind(thread_id)
                .fetch_all(&mut *conn)
                .await?;
        let here: HashMap<String, (i64, String)> =
            rows.into_iter().filter_map(|(id, u, b)| Some((u?, (id, b)))).collect();
        for post in sorted {
            let post_label = format!("{label}, Beitrag von {}", post.author);
            if let Some((id, body)) = here.get(&post.uuid) {
                if *body != post.body {
                    run.hint(
                        "faden-beitrag",
                        &post.uuid,
                        None,
                        format!("{post_label}: Text weicht ab (bleibt)"),
                        excerpt(body),
                        excerpt(&post.body),
                    );
                }
                insert_versions(conn, run, VersionOwner::ThreadPost(*id), &post.versions, &post_label).await?;
                continue;
            }
            if run.tombstoned(&post.uuid) {
                run.deleted_here("faden-beitrag", &post.uuid, post_label);
                continue;
            }
            if uuid_exists(conn, "chat_thread_post", &post.uuid).await? {
                run.hint(
                    "faden-beitrag",
                    &post.uuid,
                    None,
                    format!("{post_label}: die Kennung gehört hier zu einem anderen Faden"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            let position: i64 =
                sqlx::query_scalar("SELECT coalesce(max(position) + 1, 0) FROM chat_thread_post WHERE thread_id = ?")
                    .bind(thread_id)
                    .fetch_one(&mut *conn)
                    .await?;
            let post_id = insert_thread_post(conn, thread_id, position, post).await?;
            run.item("faden-beitrag", &post.uuid, post_label.clone());
            insert_versions(conn, run, VersionOwner::ThreadPost(post_id), &post.versions, &post_label).await?;
        }
    }
    Ok(())
}

/// (f) Images and image versions with an unknown identifier (check as at the upload, free name); versions read in of a
/// known image never become current.
async fn merge_assets(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    for asset in &doc.work.assets {
        let local: Option<(i64, String)> = sqlx::query_as("SELECT id, name FROM asset WHERE work_id = ? AND uuid = ?")
            .bind(work)
            .bind(&asset.uuid)
            .fetch_optional(&mut *conn)
            .await?;
        let Some((asset_id, name)) = local else {
            let label = format!("Bild {}", asset.name);
            if run.tombstoned(&asset.uuid) {
                run.deleted_here("bild", &asset.uuid, label);
                continue;
            }
            if uuid_exists(conn, "asset", &asset.uuid).await? {
                run.hint(
                    "bild",
                    &asset.uuid,
                    None,
                    format!("{label}: die Kennung gehört hier zu einem anderen Werk"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            let mut fresh = true;
            for revision in &asset.revisions {
                if uuid_exists(conn, "asset_revision", &revision.uuid).await? || run.tombstoned(&revision.uuid) {
                    fresh = false;
                }
            }
            if !fresh {
                run.hint(
                    "bild",
                    &asset.uuid,
                    None,
                    format!("{label}: eine Fassung ist hier schon vorhanden oder gelöscht; nicht übernommen"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            insert_asset(conn, run, work, asset).await?;
            continue;
        };
        type Row = (Option<String>, i64, String, String, String, String);
        let rows: Vec<Row> =
            sqlx::query_as("SELECT uuid, number, caption, alt, note, sha256 FROM asset_revision WHERE asset_id = ?")
                .bind(asset_id)
                .fetch_all(&mut *conn)
                .await?;
        let here: HashMap<String, Row> = rows.into_iter().filter_map(|r| Some((r.0.clone()?, r))).collect();
        let mut numbers: HashMap<&str, u32> = HashMap::new();
        let mut sorted: Vec<_> = asset.revisions.iter().collect();
        sorted.sort_by_key(|r| r.number);
        for revision in sorted {
            let label = format!("Bild {name}, Fassung {}", revision.number);
            if let Some(row) = here.get(&revision.uuid) {
                numbers.insert(revision.uuid.as_str(), u32::try_from(row.1).unwrap_or(0));
                let mut differs = Vec::new();
                if row.2 != revision.caption {
                    differs.push("Bildunterschrift");
                }
                if row.3 != revision.alt {
                    differs.push("Alternativtext");
                }
                if row.4 != revision.note {
                    differs.push("Notiz");
                }
                if row.5 != revision.sha256 {
                    differs.push("Inhalt");
                }
                if !differs.is_empty() {
                    run.hint(
                        "bildfassung",
                        &revision.uuid,
                        None,
                        format!("{label}: {} weicht ab (bleibt)", differs.join(", ")),
                        row.2.clone(),
                        revision.caption.clone(),
                    );
                }
                continue;
            }
            if run.tombstoned(&revision.uuid) {
                run.deleted_here("bildfassung", &revision.uuid, label);
                continue;
            }
            if uuid_exists(conn, "asset_revision", &revision.uuid).await? {
                run.hint(
                    "bildfassung",
                    &revision.uuid,
                    None,
                    format!("{label}: die Kennung gehört hier zu einem anderen Bild"),
                    String::new(),
                    String::new(),
                );
                continue;
            }
            let content = match check_image(&name, revision) {
                Ok(content) => content,
                Err(message) => {
                    run.report.refusals.push(format!("{message} – die Fassung wird nicht übernommen."));
                    continue;
                }
            };
            let last: i64 = sqlx::query_scalar("SELECT last_rev FROM asset WHERE id = ?")
                .bind(asset_id)
                .fetch_one(&mut *conn)
                .await?;
            let number = u32::try_from(last + 1).map_err(|_| broken("Fassungsnummer zu groß".into()))?;
            let based_on = revision.based_on.as_deref().and_then(|b| numbers.get(b).copied());
            insert_asset_revision(conn, asset_id, number, based_on, revision, "superseded", &content).await?;
            numbers.insert(revision.uuid.as_str(), number);
            run.item(
                "bildfassung",
                &revision.uuid,
                format!("Bild {name}, Fassung {number} (dort {})", revision.number),
            );
        }
    }
    Ok(())
}

/// (g) Work details, book details, speaking time, Bible module and sources: if the row is missing completely, it is
/// created; otherwise only a notice on a deviation.
async fn merge_details(conn: &mut SqliteConnection, run: &mut Run<'_>, work: WorkId) -> Result<(), StoreError> {
    let doc = run.doc;
    let w = &doc.work;
    type WorkRow = (String, String, String, String, String, String, Option<String>, String, String);
    let (kind, title, subtitle, author, audience, bible, numbering, scheme, language): WorkRow = sqlx::query_as(
        "SELECT kind, title, subtitle, author, audience, bible, numbering, numbering_scheme, language FROM work \
         WHERE id = ?",
    )
    .bind(work)
    .fetch_one(&mut *conn)
    .await?;
    let kind_there = match w.kind {
        ExchangeWorkKind::Book => "book",
        ExchangeWorkKind::Sermon => "sermon",
    };
    let fields: [(&str, &str, &str); 8] = [
        ("Art", &kind, kind_there),
        ("Titel", &title, &w.title),
        ("Untertitel", &subtitle, &w.subtitle),
        ("Autor", &author, &w.author),
        ("Zielgruppe", &audience, &w.audience),
        ("Bibelübersetzung", &bible, &w.bible),
        ("Gliederungsschema", &scheme, &w.numbering_scheme),
        ("Sprache", &language, w.language.as_xml()),
    ];
    for (name, here, there) in fields {
        if here != there {
            run.hint(
                "werk",
                &w.uuid,
                None,
                format!("Werkangabe „{name}“ weicht ab (bleibt)"),
                here.to_owned(),
                there.to_owned(),
            );
        }
    }
    if numbering != w.numbering {
        run.hint(
            "werk",
            &w.uuid,
            None,
            "Eigenes Muster der Nummerierung weicht ab (bleibt)".into(),
            numbering.unwrap_or_default(),
            w.numbering.clone().unwrap_or_default(),
        );
    }

    if let Some(book) = &w.book {
        type BookHead = (Option<String>, String, i64, i64, Option<String>, f64, f64);
        type BookText = (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        );
        let head: Option<BookHead> = sqlx::query_as(
            "SELECT (SELECT uuid FROM asset WHERE id = b.cover_asset_id), b.title_area, b.front_matter, b.cover_pages, \
             (SELECT uuid FROM asset WHERE id = b.author_photo_asset_id), b.paper_mm, b.bleed_mm \
             FROM work_book b WHERE b.work_id = ?",
        )
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
        match head {
            None => {
                insert_book(conn, work, book).await?;
                run.hint(
                    "buch",
                    &w.uuid,
                    None,
                    "Buchangaben fehlten hier; aus der Datei angelegt".into(),
                    "fehlt".into(),
                    "angelegt".into(),
                );
            }
            Some(r) => {
                let t: BookText = sqlx::query_as(
                    "SELECT publisher, place, year, edition, isbn, price, series, printer, bible_notice, image_credit, \
                     dedication, blurb, bio, publisher_address, printer_address FROM work_book WHERE work_id = ?",
                )
                .bind(work)
                .fetch_one(&mut *conn)
                .await?;
                let mut differs = Vec::new();
                let text_fields: [(&str, &str, &str); 15] = [
                    ("Verlag", &t.0, &book.publisher),
                    ("Ort", &t.1, &book.place),
                    ("Jahr", &t.2, &book.year),
                    ("Auflage", &t.3, &book.edition),
                    ("ISBN", &t.4, &book.isbn),
                    ("Preis", &t.5, &book.price),
                    ("Reihe", &t.6, &book.series),
                    ("Druckerei", &t.7, &book.printer),
                    ("Bibelvermerk", &t.8, &book.bible_notice),
                    ("Bildnachweis", &t.9, &book.image_credit),
                    ("Widmung", &t.10, &book.dedication),
                    ("Klappentext", &t.11, &book.blurb),
                    ("Vita", &t.12, &book.bio),
                    ("Anschrift des Verlags", &t.13, &book.publisher_address),
                    ("Anschrift der Druckerei", &t.14, &book.printer_address),
                ];
                for (name, here, there) in text_fields {
                    if here != there {
                        differs.push(name);
                    }
                }
                if r.0 != book.cover {
                    differs.push("Umschlag");
                }
                if r.4 != book.author_photo {
                    differs.push("Autorenfoto");
                }
                if r.1 != title_area_word(book.title_area) {
                    differs.push("Titelbereich");
                }
                if (r.2 != 0) != book.front_matter {
                    differs.push("Titelei");
                }
                if (r.3 != 0) != book.cover_pages {
                    differs.push("Umschlagseiten");
                }
                if (r.5 - book.paper_mm).abs() > 1e-9 {
                    differs.push("Papierstärke");
                }
                if (r.6 - book.bleed_mm).abs() > 1e-9 {
                    differs.push("Beschnitt");
                }
                if !differs.is_empty() {
                    run.hint(
                        "buch",
                        &w.uuid,
                        None,
                        format!("Buchangaben weichen ab (bleiben): {}", differs.join(", ")),
                        "hier".into(),
                        format!("in {}", run.instance),
                    );
                }
            }
        }
    }
    merge_cover(conn, run, work).await?;
    merge_sheet(conn, run, work).await?;
    merge_typeset(conn, run, work).await?;
    if let Some(timing) = &w.timing {
        let row: Option<(i64, Option<i64>)> =
            sqlx::query_as("SELECT words_per_minute, target_seconds FROM work_timing WHERE work_id = ?")
                .bind(work)
                .fetch_optional(&mut *conn)
                .await?;
        match row {
            None => {
                insert_timing(conn, work, timing).await?;
                run.hint(
                    "sprechdauer",
                    &w.uuid,
                    None,
                    "Sprechdauer fehlte hier; aus der Datei angelegt".into(),
                    "fehlt".into(),
                    "angelegt".into(),
                );
            }
            Some((wpm, target)) => {
                if wpm != i64::from(timing.words_per_minute) || target != timing.target_seconds.map(i64::from) {
                    let show = |w: i64, t: Option<i64>| {
                        format!("{w} Wörter/min, Ziel {}", t.map_or("–".into(), |t| format!("{t} s")))
                    };
                    run.hint(
                        "sprechdauer",
                        &w.uuid,
                        None,
                        "Sprechdauer weicht ab (bleibt)".into(),
                        show(wpm, target),
                        show(i64::from(timing.words_per_minute), timing.target_seconds.map(i64::from)),
                    );
                }
            }
        }
    }
    if let Some(module) = &w.bible_module {
        let here: Option<String> = sqlx::query_scalar("SELECT module FROM work_bible WHERE work_id = ?")
            .bind(work)
            .fetch_optional(&mut *conn)
            .await?;
        match here {
            None => {
                sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, ?)")
                    .bind(work)
                    .bind(module)
                    .execute(&mut *conn)
                    .await?;
                run.hint(
                    "bibelmodul",
                    &w.uuid,
                    None,
                    "Bibelmodul fehlte hier; aus der Datei angelegt".into(),
                    "fehlt".into(),
                    module.clone(),
                );
            }
            Some(here) if here != *module => {
                run.hint("bibelmodul", &w.uuid, None, "Bibelmodul weicht ab (bleibt)".into(), here, module.clone());
            }
            Some(_) => {}
        }
    }
    if !w.sword_sources.is_empty() {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT kind, module FROM work_sword_source WHERE work_id = ? ORDER BY kind, position")
                .bind(work)
                .fetch_all(&mut *conn)
                .await?;
        if rows.is_empty() {
            insert_sources(conn, work, doc).await?;
            run.hint(
                "quellen",
                &w.uuid,
                None,
                "SWORD-Quellen fehlten hier; aus der Datei angelegt".into(),
                "fehlt".into(),
                "angelegt".into(),
            );
        } else {
            let here: HashSet<(String, String)> = rows.into_iter().collect();
            let there: HashSet<(String, String)> =
                w.sword_sources.iter().map(|s| (source_word(s.kind).to_owned(), s.module.clone())).collect();
            if here != there {
                let show = |set: &HashSet<(String, String)>| {
                    let mut list: Vec<String> = set.iter().map(|(_, m)| m.clone()).collect();
                    list.sort();
                    list.join(", ")
                };
                run.hint(
                    "quellen",
                    &w.uuid,
                    None,
                    "SWORD-Quellen weichen ab (bleiben)".into(),
                    show(&here),
                    show(&there),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_uuids_are_stable_v8_and_depend_on_text() {
        let a = derive_uuid("26000000-0000-4000-8000-00000000006f", "Text A");
        // Same input → same identifier: that way a second reading in creates nothing twice.
        assert_eq!(a, derive_uuid("26000000-0000-4000-8000-00000000006f", "Text A"), "stabil");
        assert_ne!(a, derive_uuid("26000000-0000-4000-8000-00000000006f", "Text B"), "anderer Text → andere Kennung");
        assert!(baruch_exchange::is_uuid(&a), "kanonisch: {a}");
        // Version 8 at position 14, variant 10 at position 19 (contract F3).
        assert_eq!(&a[14..15], "8", "Version 8: {a}");
        assert!(matches!(&a[19..20], "8" | "9" | "a" | "b"), "Variante 10: {a}");
    }

    #[test]
    fn the_derivation_follows_the_documented_formula() {
        // Formula from docs/Austauschformat.md §4 recomputed by hand.
        let original = "26000000-0000-4000-8000-00000000006f";
        let text = "Ein Text";
        let input = format!("baruch-fassung:{original}:{}", baruch_exchange::sha256_hex(text.as_bytes()));
        let digest = baruch_exchange::sha256_hex(input.as_bytes());
        let derived = derive_uuid(original, text).replace('-', "");
        // All positions except version (position 12) and variant (position 16) come unchanged from the hash.
        assert_eq!(&derived[..12], &digest[..12], "erste 6 Bytes");
        assert_eq!(&derived[13..16], &digest[13..16], "Rest von Byte 6/7");
        assert_eq!(&derived[17..32], &digest[17..32], "Bytes 8 (Rest) bis 15");
    }

    #[test]
    fn instances_without_a_name_show_the_uuid_prefix() {
        let named = ExchangeInstance { uuid: "0f8c2a4e-1b3d-4c5e-9f60-718293a4b5c6".into(), name: "Laptop".into() };
        let unnamed = ExchangeInstance { name: "  ".into(), ..named.clone() };
        assert_eq!(instance_label(&named), "Laptop", "Name geht vor");
        assert_eq!(instance_label(&unnamed), "0f8c2a4e", "ohne Namen die ersten 8 Zeichen");
    }
}
