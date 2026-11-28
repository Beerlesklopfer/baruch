// SPDX-License-Identifier: AGPL-3.0-or-later
//! Data storage of Baruch: SQLite is the source (contract B1, sections 2–4 and 6).
//!
//! - One file `<data>/baruch.sqlite`, WAL, foreign keys on, 5 s waiting time on locks.
//! - Schema in `migrations/0001_core.sql`, embedded through [`MIGRATOR`]. Further migrators (for example
//!   `baruch-library`) share the table `_sqlx_migrations`; therefore the migrator runs with `ignore_missing = true`.
//! - Every change (accepting, switching, rejecting, saving, changing, deleting, proposal, post) is exactly one
//!   transaction with `BEGIN IMMEDIATE`. If a step fails, everything is rolled back.
//! - Revision tree (migration `0004_revision_tree.sql`, module `tree`): every revision has a parent (`based_on`, root =
//!   NULL). Per section exactly one carries the marker `current` (unique partial index; the store checks "at least one"
//!   when reading and writing), switchable to any other revision ([`Store::switch_to`], wrappers [`Store::set_current`]
//!   and [`Store::accept`]); the outline follows the heading line of the revision in the process, whose text stays
//!   unchanged. Changing ([`Store::update_revision`]) and deleting ([`Store::delete_revision`]) only by the human;
//!   triggers keep the origin immutable, forbid deleting the current revision, hang children of a deleted revision on
//!   its parent and assign no number twice (`node.last_rev`).
//! - Assistants only create proposals ([`Store::add_proposal`]); a proposal becomes current only through
//!   [`Store::accept`] or [`Store::set_current`].
//!
//! - Typst import ([`Store::import_typst`], contract B1.5): preamble → work template (`work.template`), `=`/`==`/`===`
//!   → outline, text per heading → revision 1 by the human. A work remembers the SHA-256 of its source; the same file a
//!   second time → [`StoreError::AlreadyImported`]. [`Store::export_typst`] assembles template and current texts into
//!   one file again.
//! - Managing works (migration `0005_works.sql`, module `works`, contract Round 4): unique titles through [`title_key`]
//!   (backfilled for all works after opening, existing duplicates resolved only through the title),
//!   [`Store::create_work`], [`Store::import_typst_titled`], metadata ([`Store::work_meta`], [`Store::update_work`]),
//!   revisions of the work template ([`Store::template_revisions`]) and deleting whole works ([`Store::delete_work`]).
//!
//! - Round 30 (migration 0047, interface R30-S1): metadata of posts (`post_meta`), variants as siblings in the revision
//!   tree (`variants`, [`Store::add_proposal_on`]), solutions per finding ([`Store::finding_set_solutions`]) and the
//!   log of the work-wide replacing (`replace`). Reserved: store 0048 suchen-verweise, 0049 chat-loesungen, 0050
//!   chat-bedienung, 0051 integrator; library 0108 suchen-verweise, 0109 chat-bedienung (all expected unused).
//! - Round 29 (interface R29-S1): typesetting settings per work (module `typeset`, [`WorkTypeset`], [`TypesetSettings`]
//!   from `baruch_typst::typeset`); their Typst rules are placed by `snapshot::head` behind the work template.
//!   Migration 0048 (satz-daten): table `work_typeset` with uuid, tombstone kind `work_typeset` (rebuild of
//!   `uuid_tombstone` like 0046), cascade when the work is deleted. Reserved: store 0049 satz-typst, 0050
//!   satz-oberflaeche, 0051 integrator; library 0108 satz-daten, 0109 integrator (all expected unused).
//!
//! - Round 22 (interface R22-S1): amending the current revision (module `amend`, [`AmendStep`], [`RevisionPolicy`],
//!   [`SaveMode`], [`NewReason`] from `baruch_core::revision_policy`), archive of the works ([`RetiredWork`],
//!   [`RestoredWork`]). Migration 0050: `revision.amend_seq`/`based_on_amend`, table `revision_amend` with tombstone
//!   kind 'revision_amend', `work_revision_policy`, `finding(_status).adopted_amend`. Reserved: store 0051 fassungen,
//!   0052 befund-rueckfrage, 0053 spiegel-entwurf, 0054 integrator; library 0109 integrator (all expected unused).
//! - 0.9.0 (user 2026-11-24): hiding older revisions (module `revision_fold`, [`RevisionFold`]); migration 0051: table
//!   `node_revision_fold` (mark per section, without uuid, not in the exchange, cascade with the section).
//! - Round 32 (interface R32-S1, Word and quitting): no migration. Reserved: store 0052 docx_import, 0053 docx_export,
//!   0054 beenden, 0055 integrator; library 0109 integrator (all expected unused).
//! - Round 34 (interface R34-S1): reserved 0052 mindmap_daten, 0053 mindmap_widget, 0054 mindmap_austausch, 0055
//!   integrator (0052 `idea_tree`: title, parent node and order on the idea; the others expected unused; the
//!   reservations of Round 32 stayed unused). Library: none.
//! - Round 34 (mindmap_daten): migration 0052 `idea_tree` – `idea.title`, `idea.parent_id`, `idea.position` and
//!   (addition by the user of 2026-11-25, cloud as a shelf next to the mind map) `idea.main_topic`; `ADD COLUMN` only,
//!   not in [`FOREIGN_KEYS_OFF`]. Module `ideas`: [`Store::idea_create_at`] ([`IdeaSpot`]), [`Store::idea_move`]
//!   ([`IdeaMove`]), [`Store::idea_delete_tree`]/[`Store::idea_restore_tree`] ([`IdeaTreeChange`]),
//!   [`Store::idea_tree_rows`], [`Store::idea_import_tree`] ([`IdeaSeed`], [`IdeaSeedLink`]). Exchange format minor
//!   version 9 (`titel`, `eltern`, `platz`, `hauptthema` on `<einfall>`).
//! - Round 35 (interface R35-S1, accounts and roles): migration 0053 `accounts` (konten: `account`, `account_session`,
//!   `login_attempt`) and 0054 `work_member` (rollen: sharing a work with a Lektor account); both `CREATE` only, not in
//!   [`FOREIGN_KEYS_OFF`], content immutable after S1. Reserved: 0055 integrator (expected unused); library: none. The
//!   four tables are instance data: without uuid, not in the exchange format, not in the mirror. Modules `accounts`
//!   ([`Account`], [`NewAccount`], [`Store::account_count`]) and `members` ([`WorkMember`]); the store never hashes
//!   itself and never sees a plain-text password.
//! - Round 36 (interface R36-S1, leftovers): module `reste36` ([`OutlineLosses`], [`Store::outline_delete_losses`]).
//!   Reserved: 0055 reste (`CREATE`/`ADD COLUMN` only, not in [`FOREIGN_KEYS_OFF`]), 0056 integrator; library 0109
//!   integrator (all expected unused).
//! - Round 38 (interface R38-S1, English for agents and developers): migration 0055 `work_language` – column
//!   `work.language` (`'de'`|`'en'`, default `'de'`); `ADD COLUMN` only, not in [`FOREIGN_KEYS_OFF`], content fixed
//!   after S1. Module `language` ([`WorkLanguage`], [`Store::work_language`], [`Store::set_work_language`]); no commit
//!   in the mirror (like `work_timing`). Reserved: 0056 integrator (expected unused); library: none.
//!
//! Queries use only the runtime API of sqlx (`query`, `query_as`, `query_scalar`), no compile-time macros; no database
//! is needed for building.
//!
//! Composed SQL (since sqlx 0.9, Round 36): `query`, `query_as`, `query_scalar`, `raw_sql` and `begin_with` now take
//! only `&'static str` or an explicitly confirmed text (`sqlx::AssertSqlSafe`, trait `SqlSafeStr`). The rule for Baruch
//! – store, library and server:
//! - Wherever possible, the SQL stands there as a literal.
//! - `AssertSqlSafe(…)` gets only text from **fixed building blocks** of the source code: column lists and `SELECT`
//!   heads from constants (`COLUMNS`, `SELECT`, `NOW` …), chains of placeholders (`?, ?, ?`) after the number of
//!   values, fixed conditions and table names from `&'static str` (literals, `as_db()` of an enum), numbers from the
//!   program (clamped limits). With pgvector, names from the settings are added: only as checked identifiers
//!   (`config::is_identifier`) and always through `quoted()`.
//! - **Values** – everything that comes from the human, from a file, a model or a client – are always bound
//!   (`.bind(…)`), never put into the text.
//! - Whoever writes a new call with `AssertSqlSafe` checks exactly that at the spot; tests may pass their own, invented
//!   SQL that way.

mod accounts;
mod amend;
mod asset_requests;
mod assets;
mod bibles;
mod book;
mod connections;
mod cover_blocks;
mod epub_settings;
pub mod exchange_apply;
pub mod exchange_dump;
mod export_map;
pub mod findings;
mod glossary;
mod idea_links;
mod ideas;
mod import;
mod language;
mod legacy_parts;
mod load;
mod mcp_tokens;
mod members;
mod model_connections;
mod outline;
mod persona_rag;
mod personas;
pub mod post_edit;
mod post_meta;
mod posts;
mod rag_collections;
mod replace;
mod requests;
mod reste36;
mod revision_fold;
mod seed;
mod sheet;
mod snapshot;
mod sword_sources;
mod sync;
mod threads;
mod timing;
mod tree;
mod typeset;
mod ui_language;
mod uuids;
mod variants;
mod works;
mod write;

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

pub use accounts::{Account, LoginAttempt, NewAccount};
pub use amend::{AmendKind, AmendStep, RevisionPolicyChange};
pub use asset_requests::{ASSET_REQUEST_PURGE_DAYS, AssetOp, AssetRequest, AssetUsage};
pub use assets::{
    AssetChange, AssetChangeKind, AssetData, AssetEncoding, AssetRevisionInfo, AssetSummary, ExportedFile, ImportAsset,
    MAX_ASSET_BYTES, NewAsset, PathConflict, PathOwner, asset_name_from_file, check_asset_path, decode_base64,
    detect_mime, encode_base64, normalize_asset_path, valid_asset_name,
};
pub use baruch_core::WorkLanguage;
pub use baruch_core::revision_policy::{NewReason, RevisionPolicy, SaveMode};
pub use baruch_typst::cover_layout::{CoverBlock, CoverBlockKind, CoverFace, CoverGeometry, CoverLayout};
pub use baruch_typst::scheme::Scheme as NumberingScheme;
pub use baruch_typst::typeset::TypesetSettings;
pub use book::{BookChange, BookEdit, BookMeta, CoverChange, TitleArea};
pub use connections::{ConnectionCheck, ConnectionKind, ConnectionRow, NewSource, PreferredSource, RagWay, SourceEdit};
pub use cover_blocks::{CoverLayoutChange, check_cover_block};
pub use export_map::{ExportMap, ExportSection};
pub use findings::{
    DeferTarget, Finding, FindingFilter, FindingKind, FindingStatus, FindingStatusEntry, MAX_FINDING_CHARS,
    MAX_QUOTE_CHARS, MAX_REASON_CHARS, NewFinding, Solution, StatusChange,
};
pub use glossary::{
    GlossaryAction, GlossaryChange, GlossaryEdit, GlossaryStatus, GlossaryTerm, GlossaryTermId, IndexAccepted,
    IndexProposal, IndexProposalId, IndexProposalStatus, IndexRegisterRow, MAX_GLOSSARY_BIBLE_REFS,
    MAX_GLOSSARY_LONG_CHARS, MAX_GLOSSARY_SHORT_CHARS, MAX_GLOSSARY_SOURCE_CHARS, MAX_GLOSSARY_TERM_CHARS,
    MAX_GLOSSARY_TERMS, MAX_GLOSSARY_VARIANTS, MAX_INDEX_REASON_CHARS, MAX_OPEN_INDEX_PROPOSALS_PER_ORIGIN,
    MAX_OPEN_INDEX_PROPOSALS_PER_WORK, NewIndexProposal,
};
pub use idea_links::{IdeaLink, IdeaLinkId, LinkKind, MAP_COORD_LIMIT, MAX_LINK_NOTE_CHARS, MapPos, NewIdeaLink};
pub use ideas::{
    Idea, IdeaEdit, IdeaFilter, IdeaId, IdeaMove, IdeaOrigin, IdeaScope, IdeaSeed, IdeaSeedLink, IdeaSpot, IdeaStatus,
    IdeaTreeChange, IdeaUse, MAX_IDEA_DEPTH, MAX_IDEA_IMPORT_NODES, MAX_IDEA_TITLE_CHARS, MAX_IDEA_WORDS,
    MAX_WORD_CHARS, NewIdea, WordCount,
};
pub use legacy_parts::{CONVERSION_NOTE, CONVERTED_KEY, ConvertedPart, PartConversion, PartText};
pub use mcp_tokens::{MCP_TOKEN_PREFIX, McpToken};
pub use members::WorkMember;
pub use model_connections::{
    ELIEZER_CONNECTION, MAX_CONNECTION_NAME, MAX_CONNECTION_URL, MODEL_CONNECTION_KINDS, ModelConnection,
    ModelConnectionEdit, PersonaBackend,
};
pub use outline::{
    AddedPoint, HeadingCheck, IdeaOutline, MAX_NEW_POINTS, MAX_TITLE_CHARS, NewPoint, NumberPreview, NumberingInfo,
    OutlineAction, OutlineAdded, OutlineChange, OutlineSave, Place, Restructure, check_title,
};
pub use personas::{
    CHAT_SCOPE_META_KEY, ChatScope, FALLBACK_COLOR, LECTOR_META_KEY, MAX_PROMPT_CHARS, PERSONA_PALETTE, PersonaColor,
    PersonaEdit, PersonaRow, valid_color,
};
pub use post_edit::PostVersion;
pub use post_meta::{PostMeta, PostRequest};
pub use posts::PostInfo;
pub use replace::{ReplaceBatch, ReplaceEdit, ReplaceItem, ReplaceSkip};
pub use requests::{
    AssistantAdd, Decision, Expected, MAX_OPEN_OUTLINE_REQUESTS_PER_ORIGIN, OutlineOp, OutlineRequest, PartAmend,
    PartIdentity, PartRevision, PreviewMark, PreviewPoint, RequestDecided,
};
pub use reste36::{LostPost, LostRevision, OutlineLosses};
pub use revision_fold::{FoldBefore, RevisionFold};
pub use sheet::{MAX_SHEET_FONT_CHARS, SheetSettings};
pub use sqlx::SqlitePool as Db;
pub use sword_sources::{MAX_MODULE_NAME, MAX_WORK_SOURCES, WorkSources};
pub use sync::{SyncCounts, SyncDocument, SyncRun, SyncSide, SyncState, SyncTally, SyncWork, sync_hash};
pub use threads::{MAX_THREAD_TITLE_CHARS, Thread, ThreadPost, ThreadSummary, UNKNOWN_THREAD, thread_title};
pub use timing::TimingSettings;
pub use tree::{NodeRevisions, RevisionEdit, RevisionInfo, RevisionRecord, Switch};
pub use typeset::{TypesetAdopted, TypesetChange, WorkTypeset};
pub use ui_language::UI_LANGUAGE_INVALID;
pub use uuids::{Instance, MAX_INSTANCE_NAME_CHARS, new_uuid};
pub use variants::VariantInfo;
pub use works::{
    DeletedWork, NewWork, Preset, RestoredWork, RetiredWork, SERMON_OUTLINE, SERMON_TEXT_TITLE, TemplateRevision,
    WorkEdit, WorkMeta, WorkUpdate, sermon_outline_in, sermon_text_title_in, title_key,
};

pub type WorkId = i64;

/// Core migrations (0001 to 0099). `ignore_missing` is set because the migrations of the library (0101 ff.) are in the
/// same database as well.
pub static MIGRATOR: sqlx::migrate::Migrator =
    sqlx::migrate::Migrator { ignore_missing: true, ..sqlx::migrate!("./migrations") };

/// Migrations that rebuild a table after the official SQLite procedure and need foreign keys switched off for that
/// (0040: `work` with a new CHECK). sqlx runs every migration in a transaction, in which `PRAGMA foreign_keys` has no
/// effect; therefore [`migrate`] switches them off beforehand on a connection of its own.
pub const FOREIGN_KEYS_OFF: &[i64] = &[40];

/// Runs [`MIGRATOR`]. Before every version from [`FOREIGN_KEYS_OFF`] first all older migrations run normally, then this
/// one on a connection of its own with `PRAGMA foreign_keys = OFF` (the migration itself checks with
/// `pragma_foreign_key_check` that no reference breaks), afterwards the foreign keys are switched on again; if that
/// does not succeed, the connection is closed instead of being returned to the pool. Last, the remaining migrations.
/// Repeatable: sqlx skips migrations that are recorded.
pub async fn migrate(pool: &Db) -> Result<(), StoreError> {
    use sqlx::Connection;
    let subset = |keep: &dyn Fn(i64) -> bool| sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect()),
        ignore_missing: true,
        locking: MIGRATOR.locking,
        no_tx: MIGRATOR.no_tx,
        table_name: MIGRATOR.table_name.clone(),
        create_schemas: MIGRATOR.create_schemas.clone(),
    };
    for &version in FOREIGN_KEYS_OFF {
        subset(&|v| v < version).run(pool).await?;
        let mut conn = pool.acquire().await?;
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await?;
        let applied = subset(&|v| v == version).run(&mut *conn).await;
        if let Err(error) = sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await {
            let _ = conn.detach().close().await;
            return Err(error.into());
        }
        applied?;
    }
    MIGRATOR.run(pool).await?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("Datenbank: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Migration: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("Datenordner {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("unbekanntes Werk {0}")]
    UnknownWork(WorkId),
    #[error("unbekannter Abschnitt {node}")]
    UnknownNode { work: WorkId, node: usize },
    #[error("Fassung {rev} gibt es nicht; vorhanden: {}", crate::write::number_list(existing))]
    UnknownRevision { node: usize, rev: u32, existing: Vec<u32> },
    #[error("Leerer Beitrag.")]
    EmptyPost,
    /// Rejected on domain grounds, for example „Nur Vorschläge lassen sich übernehmen.“
    #[error("{0}")]
    Refused(&'static str),
    #[error("Diese Datei ist schon importiert: Werk „{title}“ ({slug}).")]
    AlreadyImported { work: WorkId, slug: String, title: String },
    #[error("Import: {0}")]
    Import(String),
    /// Another work already has this title (same [`title_key`]).
    #[error("Es gibt schon ein Werk „{title}“ ({slug}).")]
    DuplicateTitle { work: WorkId, slug: String, title: String },
    #[error("Ein Werk braucht einen Titel.")]
    EmptyTitle,
    /// The headings of the text do not fit the outline item (level, further headings, no heading); that way the
    /// revision would not become current or no proposal would be created.
    #[error("{0}")]
    Heading(String),
    /// The fixed key (`key`) no longer belongs to any outline item of the work.
    #[error("Diesen Gliederungspunkt gibt es nicht mehr (gelöscht oder zusammengeführt); bitte neu laden.")]
    GoneNode { work: WorkId, key: i64 },
    /// Unknown outline scheme or invalid pattern of the pattern language (Round 6).
    #[error("Nummerierung: {0}")]
    Numbering(String),
    /// No image of this name in the work (migration 0009).
    #[error("Bild „{name}“ gibt es in diesem Werk nicht.")]
    UnknownAsset { work: WorkId, name: String },
    /// Image rejected: too large, no supported type, invalid name or path, broken Base64.
    #[error("{0}")]
    Asset(String),
    /// Title of an outline item rejected (empty, whitespace only, line break, over MAX_TITLE_CHARS).
    #[error("{0}")]
    Title(String),
    /// No persona with this handle (setting the colour, Round 7).
    #[error("Persona „{0}“ gibt es nicht.")]
    UnknownPersona(String),
    /// Colour rejected (no `#rgb`/`#rrggbb`).
    #[error("{0}")]
    Color(String),
    /// No idea with this id (migration 0011).
    #[error("Einfall {0} gibt es nicht.")]
    UnknownIdea(i64),
    /// Idea rejected: limits of the keywords and texts, reference outside the work, deleted.
    #[error("{0}")]
    Idea(String),
    /// No post with this id in the work (Round 8).
    #[error("Beitrag {post} gibt es in Werk {work} nicht.")]
    UnknownPost { work: WorkId, post: i64 },
    /// No request concerning the outline with this id in the work (migration 0012).
    #[error("Antrag {id} gibt es in Werk {work} nicht.")]
    UnknownRequest { work: WorkId, id: i64 },
    /// Exchange format (Round 26): reading, checking, writing out or reading in failed.
    #[error("Austausch: {0}")]
    Exchange(String),
    /// No finding with this id in the work (Round 28, migration 0046).
    #[error("Befund {id} gibt es in Werk {work} nicht.")]
    UnknownFinding { work: WorkId, id: i64 },
    /// Finding rejected: limits, origin outside the work, status rules (Round 28).
    #[error("{0}")]
    Finding(String),
    /// Editing a post rejected: empty, too long, not allowed (Round 28).
    #[error("{0}")]
    PostEdit(String),
    /// Work-wide replacing rejected or not (yet) possible (Round 30a, migration 0047).
    #[error("{0}")]
    Replace(String),
    /// Typesetting settings rejected (check with all messages) or not yet possible (Round 29).
    #[error("Satz-Einstellungen: {0}")]
    Typeset(String),
    /// Glossary or index rejected (limits, duplicates, built-in identifier, word no longer there) or not yet possible
    /// (Round 31).
    #[error("Glossar/Register: {0}")]
    Glossary(String),
    /// No glossary term with this id in the work (Round 31).
    #[error("Glossarbegriff {id} gibt es in Werk {work} nicht.")]
    UnknownGlossaryTerm { work: WorkId, id: i64 },
    /// No index proposal with this id in the work (Round 31).
    #[error("Registervorschlag {id} gibt es in Werk {work} nicht.")]
    UnknownIndexProposal { work: WorkId, id: i64 },
    /// Cover block rejected: control characters, whitespace around font names, limits (Round 23, web; before `Asset`).
    #[error("{0}")]
    CoverBlock(String),
    /// Stale: preview or decision on an outdated request (Round 23, mcp-namen; before `Heading("Veraltet: …")`).
    /// The wording stays „Veraltet: <Grund>“.
    #[error("{0}")]
    Stale(String),
}

impl From<baruch_exchange::ExchangeError> for StoreError {
    fn from(e: baruch_exchange::ExchangeError) -> Self {
        StoreError::Exchange(e.to_string())
    }
}

/// Result of [`Store::import_typst`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Imported {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    pub author: String,
    /// Number of sections created (chapters, sections, subsections).
    pub nodes: usize,
}

/// A work in the overview ([`Store::work_list`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkSummary {
    pub id: WorkId,
    pub slug: String,
    pub title: String,
    pub author: String,
    /// Number of nodes of the outline.
    pub nodes: usize,
    pub kind: baruch_core::WorkKind,
    pub subtitle: String,
    /// Number of sections (nodes without parts).
    pub sections: usize,
    /// Open proposals in the whole work.
    pub proposals: usize,
    pub created: String,
    /// Last change: largest point in time from `work.updated`, `revision.created`, `revision.edited`.
    pub updated: Option<String>,
    /// Work language (migration 0055): personas answer in it.
    pub language: WorkLanguage,
}

/// Violated rule in the stored data (for example two or no `current`). It is reported as a database error; the running
/// transaction is rolled back by that.
pub(crate) fn broken(message: String) -> StoreError {
    StoreError::Db(sqlx::Error::Protocol(message))
}

/// Result of a change that the server logs in the Git mirror.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Change {
    pub work: WorkId,
    /// Section (seq).
    pub node: usize,
    /// For `Accepted`, `Saved` and `Switched` the revision number that is current now, for `Updated` and `Deleted` the
    /// revision concerned.
    pub rev: u32,
    /// For `Updated` and `Deleted` the human (`Human{human}`), otherwise the author of the revision that is current
    /// now.
    pub author: baruch_core::Author,
    pub kind: ChangeKind,
    /// For `Updated` and `Deleted` the state of the revision before, otherwise `None`.
    pub before: Option<RevisionRecord>,
    /// For `Deleted` the children of the deleted revision, which now hang on its parent.
    pub reparented: Vec<u32>,
    /// For `Amended` and `AmendUndone` the new state of the revision (`revision.amend_seq`), otherwise `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amend_seq: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Proposal accepted (switching to a proposal).
    Accepted,
    /// New revision by the human, current at once.
    Saved,
    /// "Current" marker switched to another revision that is not a proposal.
    Switched,
    /// Text and/or note of a revision changed by the human.
    Updated,
    /// Revision deleted by the human.
    Deleted,
    /// Current revision amended by the human (Round 22, `revision_amend` kind 'amend').
    Amended,
    /// Amendment taken back through the history (Round 22, `revision_amend` kind 'undo').
    AmendUndone,
}

/// A file of the Typst mirror; `path` is relative and separated with '/'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorFile {
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone)]
pub struct Store {
    pool: Db,
}

/// Creates the data folder. Round 23 (code review quellen-5): a folder that Baruch itself newly creates gets the
/// permissions 0700 under Unix (only the owner), so that database and mirror are not readable by others even outside a
/// protected home. An existing folder stays as it is – the user has set its permissions (for example deliberately for a
/// group).
fn create_data_dir(data_dir: &Path) -> std::io::Result<()> {
    if data_dir.is_dir() {
        return Ok(());
    }
    if let Some(parent) = data_dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    // Outside of Unix (Windows) there are no permission bits: there the folder inherits the permissions of
    // %LOCALAPPDATA% (only the user), `builder` stays unchanged.
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(data_dir) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && data_dir.is_dir() => Ok(()),
        other => other,
    }
}

impl Store {
    /// Creates `<data>`, opens `<data>/baruch.sqlite`, runs [`MIGRATOR`] through [`migrate`], backfills the title keys
    /// (`works::ensure_title_keys`, duplicates from the time before 0005 get a title of their own) and fills a really
    /// new database (without `store_meta.seeded`, migration 0007) in one transaction with
    /// `baruch_core::sample::work()`; if all works are deleted later, it is not seeded again.
    pub async fn open(data_dir: &Path) -> Result<Store, StoreError> {
        create_data_dir(data_dir).map_err(|source| StoreError::Io { path: data_dir.to_owned(), source })?;
        let options = SqliteConnectOptions::new()
            .filename(data_dir.join("baruch.sqlite"))
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new().max_connections(4).connect_with(options).await?;
        migrate(&pool).await?;
        works::ensure_title_keys(&pool).await?;
        seed::seed_if_empty(&pool).await?;
        Ok(Store { pool })
    }

    /// The same pool for further crates (for example `baruch_library::migrate(store.pool())`).
    pub fn pool(&self) -> &Db {
        &self.pool
    }

    /// All works as `(id, slug, title)`, by id.
    pub async fn works(&self) -> Result<Vec<(WorkId, String, String)>, StoreError> {
        Ok(sqlx::query_as("SELECT id, slug, title FROM work ORDER BY id").fetch_all(&self.pool).await?)
    }

    /// Work for the folder name; unknown → `UnknownWork(0)`.
    pub async fn work_by_slug(&self, slug: &str) -> Result<WorkId, StoreError> {
        sqlx::query_scalar("SELECT id FROM work WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(0))
    }

    /// Current work template (preamble up to the first heading); empty for the sample work.
    pub async fn work_template(&self, work: WorkId) -> Result<String, StoreError> {
        sqlx::query_scalar("SELECT template FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(work))
    }

    /// A Typst file as a new work (atomic, `BEGIN IMMEDIATE`). CRLF becomes LF beforehand; the same source a second
    /// time → `AlreadyImported` (nothing created), no heading → `Import`, title already taken → `DuplicateTitle`
    /// (custom title: [`Store::import_typst_titled`]). Title and author from `#set document`, otherwise the file name
    /// without extension or empty; folder name from the title, on a collision with `-2`, `-3` …. Every section gets
    /// revision 1 by `human` (current, note „Import aus <file_name>“). Personas stay untouched.
    pub async fn import_typst(&self, source: &str, file_name: &str, human: &str) -> Result<Imported, StoreError> {
        self.import_typst_noted(source, file_name, human, &format!("Import aus {file_name}")).await
    }

    /// Like [`Store::import_typst`], with a custom note on every revision (for example which MCP client triggered the
    /// import).
    pub async fn import_typst_noted(
        &self,
        source: &str,
        file_name: &str,
        human: &str,
        note: &str,
    ) -> Result<Imported, StoreError> {
        self.import_typst_titled(source, file_name, human, note, None).await
    }

    /// The work as one Typst file: work template and current texts in tree order (like [`Store::mirror_snapshot`]).
    /// After an import without a change byte for byte the source (after CRLF → LF) apart from the jump labels `<kN>` or
    /// anchor lines that the export puts on headings (Round 30a, `baruch_typst::xref`).
    pub async fn export_typst(&self, work: WorkId) -> Result<String, StoreError> {
        let mut conn = self.pool.acquire().await?;
        snapshot::export(&mut conn, work).await
    }

    /// Keys of the nodes whose heading Typst counts as a chapter (`counter(heading).get().first()`, tree order); `None`
    /// if that is not certain without Typst (Round 11, chapter image).
    pub async fn counted_headings(&self, work: WorkId) -> Result<Option<Vec<i64>>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        snapshot::counted_headings(&mut conn, work).await
    }

    /// The work with the smallest id.
    pub async fn default_work(&self) -> Result<WorkId, StoreError> {
        let id: Option<i64> = sqlx::query_scalar("SELECT min(id) FROM work").fetch_one(&self.pool).await?;
        id.ok_or(StoreError::UnknownWork(0))
    }

    pub async fn work_slug(&self, work: WorkId) -> Result<String, StoreError> {
        sqlx::query_scalar("SELECT slug FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(work))
    }

    /// Position (`seq`) of the outline item with the fixed key `key` (database id, `Node::key`). If `key` belongs to no
    /// item of the work (any longer) → `GoneNode`.
    pub async fn node_seq(&self, work: WorkId, key: i64) -> Result<usize, StoreError> {
        let seq: Option<i64> = sqlx::query_scalar("SELECT seq FROM node WHERE work_id = ? AND id = ?")
            .bind(work)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        match seq {
            Some(seq) => to_usize(seq, "seq"),
            None => {
                self.work_slug(work).await?;
                Err(StoreError::GoneNode { work, key })
            }
        }
    }

    /// Complete work: nodes by seq, revisions by number, posts by position (with sources and refs), all personas by
    /// position.
    pub async fn load_work(&self, work: WorkId) -> Result<baruch_core::Work, StoreError> {
        let mut conn = self.pool.acquire().await?;
        load::work(&mut conn, work).await
    }

    /// Accept a proposal (atomic): [`Store::switch_to`] with `only_proposal`, as the human „Autor“; previous `current`
    /// → `superseded`, proposal → `current`, the outline follows its heading line. If `rev` is not a proposal,
    /// everything stays as it is (`Refused`).
    pub async fn accept(&self, work: WorkId, node: usize, rev: u32) -> Result<Change, StoreError> {
        self.switch_to(work, node, rev, true, tree::HUMAN).await.map(|s| s.change)
    }

    /// Reject a proposal (atomic): `proposed` → `rejected`. Returns the current revision number.
    pub async fn reject(&self, work: WorkId, node: usize, rev: u32) -> Result<u32, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = write::reject(&mut tx, work, node, rev).await?;
        tx.commit().await?;
        Ok(current)
    }

    /// Save one's own edit (atomic): new revision by the human, `current` at once, with the current revision as the
    /// parent (a root for an empty section); like [`Store::save_human_on`] with base = current revision. CRLF becomes
    /// LF. Same text as the current revision → `Ok(None)`, nothing changed.
    pub async fn save_human(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
    ) -> Result<Option<Change>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = write::save_human(&mut tx, work, node, body, author, None).await?;
        if change.is_some() {
            outline::check_current_heading(&mut tx, work, node).await?;
            tx.commit().await?;
        }
        Ok(change)
    }

    /// Append a proposal of an assistant: always `proposed`, never `current`. Sets `based_on` to the current revision
    /// and returns the new number. A section without a current revision takes no proposals (`Refused`). If the headings
    /// do not fit the outline item (another level, further headings, none), → `Heading`: a proposal never changes the
    /// outline.
    pub async fn add_proposal(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        persona: &str,
        model: &str,
        note: &str,
    ) -> Result<u32, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        outline::check_proposal_heading(&mut tx, work, node, body).await?;
        let number = write::add_proposal(&mut tx, work, node, body, persona, model, note).await?;
        tx.commit().await?;
        Ok(number)
    }

    /// Append a post in the section chat (text trimmed, empty → `EmptyPost`), references in `post_ref`. Returns the new
    /// id of the post (`post.id`, Round 8; `Post::id`/`created` are not read).
    pub async fn add_post(&self, work: WorkId, node: usize, post: &baruch_core::Post) -> Result<i64, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = write::add_post(&mut tx, work, node, post).await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Typst mirror of the work from the current revisions: `main.typ` and `abschnitte/{seq:03}-{slug}.typ` (contract
    /// B1, section 6). With a work template `main.typ` begins with it and includes the sections after it, otherwise
    /// with `// Baruch: <title>`.
    pub async fn mirror_snapshot(&self, work: WorkId) -> Result<Vec<MirrorFile>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        snapshot::files(&mut conn, work).await
    }

    /// Index and glossary of a work for projection, end of the book and EPUB (Round 31, contract §2.6): typesetting
    /// settings, built-in and custom indexes, accepted glossary terms. Unknown work → `UnknownWork`.
    pub async fn back_matter(&self, work: WorkId) -> Result<baruch_typst::index::BackMatter, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
        if exists.is_none() {
            return Err(StoreError::UnknownWork(work));
        }
        snapshot::back_matter_in(&mut conn, work).await
    }
}

/// Default data folder: `dirs::data_local_dir()` + `baruch`. Linux `$XDG_DATA_HOME/baruch` or `~/.local/share/baruch`,
/// macOS `~/Library/Application Support/baruch`, Windows `%LOCALAPPDATA%\baruch` (Local, not Roaming: roaming profiles
/// would copy the SQLite WAL and the Git mirror at every sign-in and sign-out). Without a home folder `./baruch`.
pub fn default_data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("baruch")
}

/// Maximum length of a folder name from [`slug`].
const SLUG_MAX: usize = 40;

/// Folder name: lower-case letters, ä→ae ö→oe ü→ue ß→ss, everything except `[a-z0-9]` → '-', runs collapsed, edges
/// trimmed, empty → "werk". At most 40 characters, cut at the last word boundary ('-') within the limit; a single
/// longer word is cut hard.
pub fn slug(s: &str) -> String {
    fn push(out: &mut String, c: char) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        let replacement = match c {
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            c => {
                push(&mut out, c);
                continue;
            }
        };
        replacement.chars().for_each(|c| push(&mut out, c));
    }
    let mut out = out.trim_matches('-').to_owned();
    if out.len() > SLUG_MAX {
        // ASCII only, byte = character. If there is a '-' at position 40, a whole word ends there.
        match out[..=SLUG_MAX].rfind('-') {
            Some(cut) if cut > 0 => out.truncate(cut),
            _ => out.truncate(SLUG_MAX),
        }
        out = out.trim_end_matches('-').to_owned();
    }
    if out.is_empty() { "werk".to_owned() } else { out }
}

/// Words of the database for the enumerations from baruch-core (= serde names, `snake_case`).
pub(crate) mod words {
    use baruch_core::{NodeKind, RevisionStatus, WorkKind};

    use crate::{StoreError, broken};

    pub fn work_kind(kind: WorkKind) -> &'static str {
        match kind {
            WorkKind::Book => "book",
            WorkKind::Sermon => "sermon",
        }
    }

    pub fn parse_work_kind(s: &str) -> Result<WorkKind, StoreError> {
        match s {
            "book" => Ok(WorkKind::Book),
            "sermon" => Ok(WorkKind::Sermon),
            _ => Err(broken(format!("unbekannte Werkart {s:?}"))),
        }
    }

    pub fn node_kind(kind: NodeKind) -> &'static str {
        match kind {
            NodeKind::Part => "part",
            NodeKind::Chapter => "chapter",
            NodeKind::Section => "section",
        }
    }

    pub fn parse_node_kind(s: &str) -> Result<NodeKind, StoreError> {
        match s {
            "part" => Ok(NodeKind::Part),
            "chapter" => Ok(NodeKind::Chapter),
            "section" => Ok(NodeKind::Section),
            _ => Err(broken(format!("unbekannte Knotenart {s:?}"))),
        }
    }

    pub fn status(status: RevisionStatus) -> &'static str {
        match status {
            RevisionStatus::Proposed => "proposed",
            RevisionStatus::Current => "current",
            RevisionStatus::Rejected => "rejected",
            RevisionStatus::Superseded => "superseded",
        }
    }

    pub fn parse_status(s: &str) -> Result<RevisionStatus, StoreError> {
        match s {
            "proposed" => Ok(RevisionStatus::Proposed),
            "current" => Ok(RevisionStatus::Current),
            "rejected" => Ok(RevisionStatus::Rejected),
            "superseded" => Ok(RevisionStatus::Superseded),
            _ => Err(broken(format!("unbekannter Revisionsstatus {s:?}"))),
        }
    }
}

/// Columns `author_kind`, `author_name`, `model` ↔ `baruch_core::Author`.
pub(crate) fn author_columns(author: &baruch_core::Author) -> (&'static str, &str, Option<&str>) {
    match author {
        baruch_core::Author::Human { name } => ("human", name, None),
        baruch_core::Author::Assistant { persona, model } => ("assistant", persona, Some(model)),
    }
}

pub(crate) fn parse_author(kind: &str, name: String, model: Option<String>) -> Result<baruch_core::Author, StoreError> {
    match (kind, model) {
        ("human", None) => Ok(baruch_core::Author::Human { name }),
        ("assistant", Some(model)) => Ok(baruch_core::Author::Assistant { persona: name, model }),
        (kind, _) => Err(broken(format!("ungültiger Autor ({kind:?}, {name:?})"))),
    }
}

/// `usize`/`u32` from an SQLite integer; negative or too large → rule violation.
pub(crate) fn to_usize(value: i64, what: &str) -> Result<usize, StoreError> {
    usize::try_from(value).map_err(|_| broken(format!("{what} außerhalb des Bereichs: {value}")))
}

pub(crate) fn to_u32(value: i64, what: &str) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| broken(format!("{what} außerhalb des Bereichs: {value}")))
}
