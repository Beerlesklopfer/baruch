// SPDX-License-Identifier: AGPL-3.0-or-later
//! Datenhaltung von Baruch: SQLite ist die Quelle (Vertrag B1, Abschnitte 2–4 und 6).
//!
//! - Eine Datei `<data>/baruch.sqlite`, WAL, Fremdschlüssel an, 5 s Wartezeit bei Sperren.
//! - Schema in `migrations/0001_core.sql`, eingebettet über [`MIGRATOR`]. Weitere Migratoren
//!   (z. B. `baruch-library`) teilen sich die Tabelle `_sqlx_migrations`; darum läuft der
//!   Migrator mit `ignore_missing = true`.
//! - Jede Änderung (Übernehmen, Umschalten, Verwerfen, Speichern, Ändern, Löschen, Vorschlag,
//!   Beitrag) ist genau eine Transaktion mit `BEGIN IMMEDIATE`. Schlägt ein Schritt fehl, wird alles
//!   zurückgerollt.
//! - Revisionsbaum (Migration `0004_revision_tree.sql`, Modul `tree`): Jede Revision hat ein
//!   Elternteil (`based_on`, Wurzel = NULL). Je Abschnitt trägt genau eine die Markierung `current`
//!   (eindeutiger Teilindex; mindestens eine prüft der Store beim Lesen und Schreiben), umschaltbar
//!   auf jede andere Fassung ([`Store::switch_to`], Hüllen [`Store::set_current`] und
//!   [`Store::accept`]); die Gliederung folgt dabei der Überschriftszeile der Fassung, deren Text
//!   bleibt unverändert. Ändern ([`Store::update_revision`]) und
//!   Löschen ([`Store::delete_revision`]) nur durch den Menschen; Trigger halten die Herkunft
//!   unveränderlich, verbieten das Löschen der aktuellen Fassung, hängen Kinder einer gelöschten
//!   Fassung an deren Elternteil und vergeben keine Nummer zweimal (`node.last_rev`).
//! - Assistenten legen nur Vorschläge an ([`Store::add_proposal`]); aktuell wird ein Vorschlag
//!   erst durch [`Store::accept`] bzw. [`Store::set_current`].
//!
//! - Typst-Import ([`Store::import_typst`], Vertrag B1.5): Präambel → Werkvorlage (`work.template`),
//!   `=`/`==`/`===` → Gliederung, Text je Überschrift → Revision 1 vom Menschen. Ein Werk merkt sich
//!   den SHA-256 seiner Quelle; dieselbe Datei ein zweites Mal → [`StoreError::AlreadyImported`].
//!   [`Store::export_typst`] setzt Vorlage und aktuelle Texte wieder zu einer Datei zusammen.
//! - Werke verwalten (Migration `0005_works.sql`, Modul `works`, Vertrag Runde 4): eindeutige Titel
//!   über [`title_key`] (nach dem Öffnen für alle Werke nachgetragen, vorhandene Doppel nur über den
//!   Titel aufgelöst), [`Store::create_work`], [`Store::import_typst_titled`], Metadaten
//!   ([`Store::work_meta`], [`Store::update_work`]), Fassungen der Werkvorlage
//!   ([`Store::template_revisions`]) und Löschen ganzer Werke ([`Store::delete_work`]).
//!
//! - Runde 30 (Migration 0047, Schnittstelle R30-S1): Metadaten von Beiträgen (`post_meta`), Varianten als
//!   Geschwister im Fassungsbaum (`variants`, [`Store::add_proposal_on`]), Lösungen je Befund
//!   ([`Store::finding_set_solutions`]) und das Protokoll des werkweiten Ersetzens (`replace`).
//!   Reserviert: Store 0048 suchen-verweise, 0049 chat-loesungen, 0050 chat-bedienung, 0051 Integrator; Bibliothek
//!   0108 suchen-verweise, 0109 chat-bedienung (alle erwartet unbenutzt).
//! - Runde 29 (Schnittstelle R29-S1): Satz-Einstellungen je Werk (Modul `typeset`, [`WorkTypeset`],
//!   [`TypesetSettings`] aus `baruch_typst::typeset`); ihre Typst-Regeln setzt `snapshot::head` hinter die Werkvorlage.
//!   Migration 0048 (satz-daten): Tabelle `work_typeset` mit uuid, Grabstein-Art `work_typeset` (Umbau von
//!   `uuid_tombstone` wie 0046), Kaskade beim Löschen des Werks. Reserviert: Store 0049 satz-typst, 0050
//!   satz-oberflaeche, 0051 Integrator; Bibliothek 0108 satz-daten, 0109 Integrator (alle erwartet unbenutzt).
//!
//! - Runde 22 (Schnittstelle R22-S1): Fortschreiben der aktuellen Fassung (Modul `amend`, [`AmendStep`],
//!   [`RevisionPolicy`], [`SaveMode`], [`NewReason`] aus `baruch_core::revision_policy`), Archiv der Werke
//!   ([`RetiredWork`], [`RestoredWork`]). Migration 0050: `revision.amend_seq`/`based_on_amend`, Tabelle
//!   `revision_amend` mit Grabstein-Art 'revision_amend', `work_revision_policy`, `finding(_status).adopted_amend`.
//!   Reserviert: Store 0051 fassungen, 0052 befund-rueckfrage, 0053 spiegel-entwurf, 0054 Integrator; Bibliothek 0109
//!   Integrator (alle erwartet unbenutzt).
//! - 0.9.0 (Nutzer 2026-11-24): ältere Fassungen ausblenden (Modul `revision_fold`, [`RevisionFold`]); Migration
//!   0051: Tabelle `node_revision_fold` (Marke je Abschnitt, ohne uuid, nicht im Austausch, Kaskade mit dem Abschnitt).
//! - Runde 32 (Schnittstelle R32-S1, Word und Beenden): keine Migration. Reserviert: Store 0052 docx_import, 0053
//!   docx_export, 0054 beenden, 0055 Integrator; Bibliothek 0109 Integrator (alle erwartet unbenutzt).
//! - Runde 34 (Schnittstelle R34-S1): reserviert 0052 mindmap_daten, 0053 mindmap_widget, 0054 mindmap_austausch, 0055
//!   Integrator (0052 `idea_tree`: Titel, Elternknoten und Reihenfolge am Einfall; die übrigen erwartet unbenutzt;
//!   die Reservierungen der Runde 32 blieben unbenutzt). Bibliothek: keine.
//!
//! Abfragen nutzen nur die Laufzeit-API von sqlx (`query`, `query_as`, `query_scalar`), keine
//! Compile-Zeit-Makros; zum Bauen ist keine Datenbank nötig.

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
mod legacy_parts;
mod load;
mod mcp_tokens;
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
mod uuids;
mod variants;
mod works;
mod write;

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

pub use amend::{AmendKind, AmendStep, RevisionPolicyChange};
pub use asset_requests::{ASSET_REQUEST_PURGE_DAYS, AssetOp, AssetRequest, AssetUsage};
pub use assets::{
    AssetChange, AssetChangeKind, AssetData, AssetEncoding, AssetRevisionInfo, AssetSummary, ExportedFile, ImportAsset,
    MAX_ASSET_BYTES, NewAsset, PathConflict, PathOwner, asset_name_from_file, check_asset_path, decode_base64,
    detect_mime, encode_base64, normalize_asset_path, valid_asset_name,
};
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
    Idea, IdeaEdit, IdeaFilter, IdeaId, IdeaOrigin, IdeaScope, IdeaStatus, IdeaUse, MAX_IDEA_WORDS, MAX_WORD_CHARS,
    NewIdea, WordCount,
};
pub use legacy_parts::{CONVERSION_NOTE, CONVERTED_KEY, ConvertedPart, PartConversion, PartText};
pub use mcp_tokens::{MCP_TOKEN_PREFIX, McpToken};
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
pub use revision_fold::{FoldBefore, RevisionFold};
pub use sheet::{MAX_SHEET_FONT_CHARS, SheetSettings};
pub use sqlx::SqlitePool as Db;
pub use sword_sources::{MAX_MODULE_NAME, MAX_WORK_SOURCES, WorkSources};
pub use sync::{SyncCounts, SyncDocument, SyncRun, SyncSide, SyncState, SyncTally, SyncWork, sync_hash};
pub use threads::{MAX_THREAD_TITLE_CHARS, Thread, ThreadPost, ThreadSummary, UNKNOWN_THREAD, thread_title};
pub use timing::TimingSettings;
pub use tree::{NodeRevisions, RevisionEdit, RevisionInfo, RevisionRecord, Switch};
pub use typeset::{TypesetAdopted, TypesetChange, WorkTypeset};
pub use uuids::{Instance, MAX_INSTANCE_NAME_CHARS, new_uuid};
pub use variants::VariantInfo;
pub use works::{
    DeletedWork, NewWork, Preset, RestoredWork, RetiredWork, SERMON_OUTLINE, SERMON_TEXT_TITLE, TemplateRevision,
    WorkEdit, WorkMeta, WorkUpdate, title_key,
};

pub type WorkId = i64;

/// Kernmigrationen (0001 bis 0099). `ignore_missing` ist gesetzt, weil in derselben Datenbank
/// auch die Migrationen der Bibliothek (0101 ff.) stehen.
pub static MIGRATOR: sqlx::migrate::Migrator =
    sqlx::migrate::Migrator { ignore_missing: true, ..sqlx::migrate!("./migrations") };

/// Migrationen, die eine Tabelle nach dem offiziellen SQLite-Verfahren neu aufbauen und dafür
/// ausgeschaltete Fremdschlüssel brauchen (0040: `work` mit neuem CHECK). sqlx führt jede Migration in
/// einer Transaktion aus, in der `PRAGMA foreign_keys` wirkungslos ist; [`migrate`] schaltet sie darum
/// vorher auf einer eigenen Verbindung aus.
pub const FOREIGN_KEYS_OFF: &[i64] = &[40];

/// Führt [`MIGRATOR`] aus. Vor jeder Version aus [`FOREIGN_KEYS_OFF`] laufen erst alle älteren
/// Migrationen normal, dann diese eine auf einer eigenen Verbindung mit `PRAGMA foreign_keys = OFF`
/// (die Migration prüft selbst mit `pragma_foreign_key_check`, dass kein Verweis bricht), danach werden
/// die Fremdschlüssel wieder eingeschaltet; gelingt das nicht, wird die Verbindung geschlossen statt in
/// den Pool zurückgegeben. Zuletzt die übrigen Migrationen. Wiederholbar: Eingetragene Migrationen
/// überspringt sqlx.
pub async fn migrate(pool: &Db) -> Result<(), StoreError> {
    use sqlx::Connection;
    let subset = |keep: &dyn Fn(i64) -> bool| sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect()),
        ignore_missing: true,
        locking: MIGRATOR.locking,
        no_tx: MIGRATOR.no_tx,
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
    /// Fachlich abgelehnt, z. B. „Nur Vorschläge lassen sich übernehmen.“
    #[error("{0}")]
    Refused(&'static str),
    #[error("Diese Datei ist schon importiert: Werk „{title}“ ({slug}).")]
    AlreadyImported { work: WorkId, slug: String, title: String },
    #[error("Import: {0}")]
    Import(String),
    /// Ein anderes Werk hat schon diesen Titel (gleicher [`title_key`]).
    #[error("Es gibt schon ein Werk „{title}“ ({slug}).")]
    DuplicateTitle { work: WorkId, slug: String, title: String },
    #[error("Ein Werk braucht einen Titel.")]
    EmptyTitle,
    /// Die Überschriften des Texts passen nicht zum Gliederungspunkt (Ebene, weitere Überschriften,
    /// keine Überschrift); so würde die Fassung nicht aktuell bzw. kein Vorschlag angelegt.
    #[error("{0}")]
    Heading(String),
    /// Der feste Schlüssel (`key`) gehört zu keinem Gliederungspunkt des Werks mehr.
    #[error("Diesen Gliederungspunkt gibt es nicht mehr (gelöscht oder zusammengeführt); bitte neu laden.")]
    GoneNode { work: WorkId, key: i64 },
    /// Unbekanntes Gliederungsschema oder ungültiges Muster der Mustersprache (Runde 6).
    #[error("Nummerierung: {0}")]
    Numbering(String),
    /// Kein Bild dieses Namens im Werk (Migration 0009).
    #[error("Bild „{name}“ gibt es in diesem Werk nicht.")]
    UnknownAsset { work: WorkId, name: String },
    /// Bild abgelehnt: zu groß, kein unterstützter Typ, ungültiger Name oder Pfad, kaputtes Base64.
    #[error("{0}")]
    Asset(String),
    /// Titel eines Gliederungspunkts abgelehnt (leer, nur Leerraum, Zeilenumbruch, über MAX_TITLE_CHARS).
    #[error("{0}")]
    Title(String),
    /// Keine Persona mit diesem Handle (Farbe setzen, Runde 7).
    #[error("Persona „{0}“ gibt es nicht.")]
    UnknownPersona(String),
    /// Farbe abgelehnt (kein `#rgb`/`#rrggbb`).
    #[error("{0}")]
    Color(String),
    /// Kein Einfall mit dieser id (Migration 0011).
    #[error("Einfall {0} gibt es nicht.")]
    UnknownIdea(i64),
    /// Einfall abgelehnt: Grenzen der Schlagwörter und Texte, Bezug außerhalb des Werks, gelöscht.
    #[error("{0}")]
    Idea(String),
    /// Kein Beitrag mit dieser id im Werk (Runde 8).
    #[error("Beitrag {post} gibt es in Werk {work} nicht.")]
    UnknownPost { work: WorkId, post: i64 },
    /// Kein Antrag zur Gliederung mit dieser id im Werk (Migration 0012).
    #[error("Antrag {id} gibt es in Werk {work} nicht.")]
    UnknownRequest { work: WorkId, id: i64 },
    /// Austauschformat (Runde 26): Lesen, Prüfen, Ausgeben oder Einlesen gescheitert.
    #[error("Austausch: {0}")]
    Exchange(String),
    /// Kein Befund mit dieser id im Werk (Runde 28, Migration 0046).
    #[error("Befund {id} gibt es in Werk {work} nicht.")]
    UnknownFinding { work: WorkId, id: i64 },
    /// Befund abgelehnt: Grenzen, Herkunft außerhalb des Werks, Statusregeln (Runde 28).
    #[error("{0}")]
    Finding(String),
    /// Beitrag bearbeiten abgelehnt: leer, zu lang, nicht erlaubt (Runde 28).
    #[error("{0}")]
    PostEdit(String),
    /// Werkweites Ersetzen abgelehnt oder (noch) nicht möglich (Runde 30a, Migration 0047).
    #[error("{0}")]
    Replace(String),
    /// Satz-Einstellungen abgelehnt (Prüfung mit allen Meldungen) oder noch nicht möglich (Runde 29).
    #[error("Satz-Einstellungen: {0}")]
    Typeset(String),
    /// Glossar oder Register abgelehnt (Grenzen, Doppel, eingebaute Kennung, Wort nicht mehr da) oder noch nicht
    /// möglich (Runde 31).
    #[error("Glossar/Register: {0}")]
    Glossary(String),
    /// Kein Glossarbegriff mit dieser id im Werk (Runde 31).
    #[error("Glossarbegriff {id} gibt es in Werk {work} nicht.")]
    UnknownGlossaryTerm { work: WorkId, id: i64 },
    /// Kein Registervorschlag mit dieser id im Werk (Runde 31).
    #[error("Registervorschlag {id} gibt es in Werk {work} nicht.")]
    UnknownIndexProposal { work: WorkId, id: i64 },
    /// Umschlagblock abgelehnt: Steuerzeichen, Leerraum um Schriftnamen, Grenzen (Runde 23, web; vorher `Asset`).
    #[error("{0}")]
    CoverBlock(String),
    /// Veraltet: Vorschau oder Entscheidung auf einem überholten Antrag (Runde 23, mcp-namen; vorher `Heading("Veraltet: …")`).
    /// Wortlaut bleibt „Veraltet: <Grund>“.
    #[error("{0}")]
    Stale(String),
}

impl From<baruch_exchange::ExchangeError> for StoreError {
    fn from(e: baruch_exchange::ExchangeError) -> Self {
        StoreError::Exchange(e.to_string())
    }
}

/// Ergebnis von [`Store::import_typst`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Imported {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    pub author: String,
    /// Anzahl der angelegten Abschnitte (Kapitel, Abschnitte, Unterabschnitte).
    pub nodes: usize,
}

/// Ein Werk in der Übersicht ([`Store::work_list`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkSummary {
    pub id: WorkId,
    pub slug: String,
    pub title: String,
    pub author: String,
    /// Anzahl der Knoten der Gliederung.
    pub nodes: usize,
    pub kind: baruch_core::WorkKind,
    pub subtitle: String,
    /// Anzahl der Abschnitte (Knoten ohne Teile).
    pub sections: usize,
    /// Offene Vorschläge im ganzen Werk.
    pub proposals: usize,
    pub created: String,
    /// Letzte Änderung: größter Zeitpunkt aus `work.updated`, `revision.created`, `revision.edited`.
    pub updated: Option<String>,
}

/// Verletzte Regel in den gespeicherten Daten (z. B. zwei oder keine `current`). Wird als
/// Datenbankfehler gemeldet; die laufende Transaktion wird dadurch zurückgerollt.
pub(crate) fn broken(message: String) -> StoreError {
    StoreError::Db(sqlx::Error::Protocol(message))
}

/// Ergebnis einer Änderung, die der Server im Git-Spiegel protokolliert.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Change {
    pub work: WorkId,
    /// Abschnitt (seq).
    pub node: usize,
    /// Bei `Accepted`, `Saved` und `Switched` die jetzt aktuelle Revisionsnummer, bei `Updated` und
    /// `Deleted` die betroffene Revision.
    pub rev: u32,
    /// Bei `Updated` und `Deleted` der Mensch (`Human{human}`), sonst der Autor der jetzt aktuellen
    /// Revision.
    pub author: baruch_core::Author,
    pub kind: ChangeKind,
    /// Bei `Updated` und `Deleted` der Stand der Revision vorher, sonst `None`.
    pub before: Option<RevisionRecord>,
    /// Bei `Deleted` die Kinder der gelöschten Revision, die jetzt an ihrem Elternteil hängen.
    pub reparented: Vec<u32>,
    /// Bei `Amended` und `AmendUndone` der neue Stand der Fassung (`revision.amend_seq`), sonst `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amend_seq: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Vorschlag übernommen (Umschalten auf einen Vorschlag).
    Accepted,
    /// Neue Revision vom Menschen, sofort aktuell.
    Saved,
    /// Markierung „aktuell“ auf eine andere, nicht vorgeschlagene Fassung umgeschaltet.
    Switched,
    /// Text und/oder Notiz einer Revision vom Menschen geändert.
    Updated,
    /// Revision vom Menschen gelöscht.
    Deleted,
    /// Aktuelle Fassung vom Menschen fortgeschrieben (Runde 22, `revision_amend` Art 'amend').
    Amended,
    /// Fortschreibung über den Verlauf zurückgenommen (Runde 22, `revision_amend` Art 'undo').
    AmendUndone,
}

/// Eine Datei des Typst-Spiegels; `path` ist relativ und mit '/' getrennt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorFile {
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone)]
pub struct Store {
    pool: Db,
}

/// Legt den Datenordner an. Runde 23 (Codereview quellen-5): Ein Ordner, den Baruch selbst neu anlegt, bekommt
/// unter Unix die Rechte 0700 (nur der Besitzer), damit Datenbank und Spiegel auch außerhalb eines geschützten
/// Home nicht für andere lesbar sind. Ein vorhandener Ordner bleibt, wie er ist – seine Rechte hat der Nutzer
/// gesetzt (z. B. bewusst für eine Gruppe).
fn create_data_dir(data_dir: &Path) -> std::io::Result<()> {
    if data_dir.is_dir() {
        return Ok(());
    }
    if let Some(parent) = data_dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    // Außerhalb von Unix (Windows) gibt es keine Rechtebits: Der Ordner erbt dort die Rechte von
    // %LOCALAPPDATA% (nur der Benutzer), `builder` bleibt unverändert.
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
    /// Legt `<data>` an, öffnet `<data>/baruch.sqlite`, führt [`MIGRATOR`] über [`migrate`] aus, trägt die
    /// Titelschlüssel nach (`works::ensure_title_keys`, Doppel aus der Zeit vor 0005 bekommen einen
    /// eigenen Titel) und befüllt eine wirklich neue Datenbank (ohne `store_meta.seeded`, Migration 0007) in
    /// einer Transaktion mit `baruch_core::sample::work()`; sind später alle Werke gelöscht, wird nicht neu gesät.
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

    /// Derselbe Pool für weitere Crates (z. B. `baruch_library::migrate(store.pool())`).
    pub fn pool(&self) -> &Db {
        &self.pool
    }

    /// Alle Werke als `(id, slug, title)`, nach id.
    pub async fn works(&self) -> Result<Vec<(WorkId, String, String)>, StoreError> {
        Ok(sqlx::query_as("SELECT id, slug, title FROM work ORDER BY id").fetch_all(&self.pool).await?)
    }

    /// Werk zum Ordnernamen; unbekannt → `UnknownWork(0)`.
    pub async fn work_by_slug(&self, slug: &str) -> Result<WorkId, StoreError> {
        sqlx::query_scalar("SELECT id FROM work WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(0))
    }

    /// Aktuelle Werkvorlage (Präambel bis zur ersten Überschrift); leer beim Beispielwerk.
    pub async fn work_template(&self, work: WorkId) -> Result<String, StoreError> {
        sqlx::query_scalar("SELECT template FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(StoreError::UnknownWork(work))
    }

    /// Typst-Datei als neues Werk (atomar, `BEGIN IMMEDIATE`). CRLF wird vorher zu LF; dieselbe
    /// Quelle ein zweites Mal → `AlreadyImported` (nichts angelegt), keine Überschrift → `Import`,
    /// Titel schon vergeben → `DuplicateTitle` (eigener Titel: [`Store::import_typst_titled`]).
    /// Titel und Autor aus `#set document`, sonst Dateiname ohne Endung bzw. leer; Ordnername aus
    /// dem Titel, bei Kollision mit `-2`, `-3` …. Jeder Abschnitt bekommt Revision 1 von `human`
    /// (aktuell, Notiz „Import aus <file_name>“). Personas bleiben unberührt.
    pub async fn import_typst(&self, source: &str, file_name: &str, human: &str) -> Result<Imported, StoreError> {
        self.import_typst_noted(source, file_name, human, &format!("Import aus {file_name}")).await
    }

    /// Wie [`Store::import_typst`], mit eigener Notiz an jeder Revision (z. B. welcher MCP-Client
    /// den Import ausgelöst hat).
    pub async fn import_typst_noted(
        &self,
        source: &str,
        file_name: &str,
        human: &str,
        note: &str,
    ) -> Result<Imported, StoreError> {
        self.import_typst_titled(source, file_name, human, note, None).await
    }

    /// Das Werk als eine Typst-Datei: Werkvorlage und aktuelle Texte in Baumreihenfolge (wie
    /// [`Store::mirror_snapshot`]). Nach einem Import ohne Änderung Byte für Byte die Quelle (nach CRLF → LF) bis auf die
    /// Sprungmarken `<kN>` bzw. Ankerzeilen, die der Export an Überschriften setzt (Runde 30a, `baruch_typst::xref`).
    pub async fn export_typst(&self, work: WorkId) -> Result<String, StoreError> {
        let mut conn = self.pool.acquire().await?;
        snapshot::export(&mut conn, work).await
    }

    /// Schlüssel der Knoten, deren Überschrift Typst als Kapitel zählt (`counter(heading).get().first()`,
    /// Baumreihenfolge); `None`, wenn das ohne Typst nicht sicher feststeht (Runde 11, Kapitelbild).
    pub async fn counted_headings(&self, work: WorkId) -> Result<Option<Vec<i64>>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        snapshot::counted_headings(&mut conn, work).await
    }

    /// Das Werk mit der kleinsten id.
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

    /// Stelle (`seq`) des Gliederungspunkts mit dem festen Schlüssel `key` (Datenbank-id, `Node::key`).
    /// Gehört `key` zu keinem Punkt des Werks (mehr) → `GoneNode`.
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

    /// Vollständiges Werk: Knoten nach seq, Revisionen nach number, Beiträge nach position
    /// (mit sources und refs), alle Personas nach position.
    pub async fn load_work(&self, work: WorkId) -> Result<baruch_core::Work, StoreError> {
        let mut conn = self.pool.acquire().await?;
        load::work(&mut conn, work).await
    }

    /// Vorschlag übernehmen (atomar): [`Store::switch_to`] mit `only_proposal`, als Mensch „Autor“;
    /// bisherige `current` → `superseded`, Vorschlag → `current`, die Gliederung folgt seiner
    /// Überschriftszeile. Ist `rev` kein Vorschlag, bleibt alles, wie es ist (`Refused`).
    pub async fn accept(&self, work: WorkId, node: usize, rev: u32) -> Result<Change, StoreError> {
        self.switch_to(work, node, rev, true, tree::HUMAN).await.map(|s| s.change)
    }

    /// Vorschlag verwerfen (atomar): `proposed` → `rejected`. Gibt die aktuelle Revisionsnummer zurück.
    pub async fn reject(&self, work: WorkId, node: usize, rev: u32) -> Result<u32, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = write::reject(&mut tx, work, node, rev).await?;
        tx.commit().await?;
        Ok(current)
    }

    /// Eigene Bearbeitung speichern (atomar): neue Revision vom Menschen, sofort `current`, mit der
    /// aktuellen Fassung als Elternteil (Wurzel bei einem leeren Abschnitt); wie
    /// [`Store::save_human_on`] mit Basis = aktuelle Fassung. CRLF wird zu LF. Gleicher Text wie die
    /// aktuelle Revision → `Ok(None)`, nichts geändert.
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

    /// Vorschlag eines Assistenten anhängen: immer `proposed`, nie `current`. Setzt `based_on`
    /// auf die aktuelle Revision und gibt die neue Nummer zurück. Ein Abschnitt ohne aktuelle
    /// Revision nimmt keine Vorschläge an (`Refused`). Passen die Überschriften nicht zum
    /// Gliederungspunkt (andere Ebene, weitere Überschriften, keine), → `Heading`: Ein Vorschlag
    /// ändert nie die Gliederung.
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

    /// Beitrag im Abschnitts-Chat anhängen (Text getrimmt, leer → `EmptyPost`), Verweise in `post_ref`.
    /// Gibt die neue id des Beitrags zurück (`post.id`, Runde 8; `Post::id`/`created` werden nicht gelesen).
    pub async fn add_post(&self, work: WorkId, node: usize, post: &baruch_core::Post) -> Result<i64, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = write::add_post(&mut tx, work, node, post).await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Typst-Spiegel des Werks aus den aktuellen Revisionen: `main.typ` und
    /// `abschnitte/{seq:03}-{slug}.typ` (Vertrag B1, Abschnitt 6). Mit Werkvorlage beginnt `main.typ`
    /// mit ihr und bindet danach die Abschnitte ein, sonst mit `// Baruch: <Titel>`.
    pub async fn mirror_snapshot(&self, work: WorkId) -> Result<Vec<MirrorFile>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        snapshot::files(&mut conn, work).await
    }

    /// Register und Glossar eines Werks für Projektion, Buchende und EPUB (Runde 31, Vertrag §2.6):
    /// Satz-Einstellungen, eingebaute und eigene Register, angenommene Glossarbegriffe. Unbekanntes Werk →
    /// `UnknownWork`.
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

/// Vorgabe-Datenordner: `dirs::data_local_dir()` + `baruch`. Linux `$XDG_DATA_HOME/baruch` bzw.
/// `~/.local/share/baruch`, macOS `~/Library/Application Support/baruch`, Windows
/// `%LOCALAPPDATA%\baruch` (Local, nicht Roaming: servergespeicherte Profile würden SQLite-WAL und
/// Git-Spiegel bei jeder An- und Abmeldung kopieren). Ohne Heimatordner `./baruch`.
pub fn default_data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("baruch")
}

/// Höchstlänge eines Ordnernamens aus [`slug`].
const SLUG_MAX: usize = 40;

/// Ordnername: Kleinbuchstaben, ä→ae ö→oe ü→ue ß→ss, alles außer `[a-z0-9]` → '-', Folgen
/// zusammengefasst, Ränder gekürzt, leer → "werk". Höchstens 40 Zeichen, gekürzt an der letzten
/// Wortgrenze ('-') innerhalb der Grenze; ein einzelnes längeres Wort wird hart gekürzt.
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
        // Nur ASCII, Byte = Zeichen. Steht an Stelle 40 ein '-', endet dort ein ganzes Wort.
        match out[..=SLUG_MAX].rfind('-') {
            Some(cut) if cut > 0 => out.truncate(cut),
            _ => out.truncate(SLUG_MAX),
        }
        out = out.trim_end_matches('-').to_owned();
    }
    if out.is_empty() { "werk".to_owned() } else { out }
}

/// Wörter der Datenbank für die Aufzählungen aus baruch-core (= serde-Namen, `snake_case`).
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

/// Spalten `author_kind`, `author_name`, `model` ↔ `baruch_core::Author`.
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

/// `usize`/`u32` aus einer SQLite-Ganzzahl; negativ oder zu groß → Regelverletzung.
pub(crate) fn to_usize(value: i64, what: &str) -> Result<usize, StoreError> {
    usize::try_from(value).map_err(|_| broken(format!("{what} außerhalb des Bereichs: {value}")))
}

pub(crate) fn to_u32(value: i64, what: &str) -> Result<u32, StoreError> {
    u32::try_from(value).map_err(|_| broken(format!("{what} außerhalb des Bereichs: {value}")))
}
