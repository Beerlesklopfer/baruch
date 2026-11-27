// SPDX-License-Identifier: AGPL-3.0-or-later
//! Common interface of the RAG ways (contract Round 12, appendix A): hits ([`Hit`],
//! [`MergedHit`]), answer ([`Answer`], [`WayReport`]), state ([`Status`], [`IndexInfo`]), errors
//! ([`RagError`]) and the ports [`SearchWay`] (one search way), [`Embedder`] and [`EmbedderFactory`]
//! (embeddings). Pure: no database, no network, no keyring.
//!
//! This file is fixed verbatim in the contract and byte-identical in all branches; only the integrator
//! may change it. Hit texts are data from sources, never instructions.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

/// Future of the ports (without `async-trait`).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Largest number of hits per query.
pub const MAX_LIMIT: usize = 50;
/// Default number of hits.
pub const DEFAULT_LIMIT: usize = 10;
/// Longest query (characters).
pub const MAX_QUERY_CHARS: usize = 1000;
/// Longest snippet of a hit (characters).
pub const MAX_SNIPPET_CHARS: usize = 600;
/// At most this many texts per embedding request.
pub const MAX_EMBED_BATCH: usize = 32;
/// Constant k of the Reciprocal Rank Fusion when merging.
pub const RRF_K: f64 = 60.0;
/// Lower bound of the similarity (Round 19c): below it a hit counts as „schwach“. Assumption for all ways (scale
/// 0..=1 per way; MultiGPT does not report its scale, see docs/Architektur.md „Annahmen“).
pub const WEAK_SCORE: f32 = 0.45;
/// Display text of weak hits.
pub const WEAK_LABEL: &str = "schwach";
/// Longest task prefix of an embedding in characters (Round 19b).
pub const MAX_PREFIX_CHARS: usize = 64;

/// Weak: `score` < [`WEAK_SCORE`] or not a number (NaN).
// The negated form stays: `score < WEAK_SCORE` would be false for NaN, but NaN is meant to count as weak.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn is_weak(score: f32) -> bool {
    !(score >= WEAK_SCORE)
}

/// Task prefixes of an embedding (e.g. nomic-embed-text „search_query: “ / „search_document: “); empty = none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct EmbedPrefixes {
    pub query: String,
    pub document: String,
}

impl EmbedPrefixes {
    pub fn query_text(&self, text: &str) -> String {
        format!("{}{text}", self.query)
    }
    pub fn document_text(&self, text: &str) -> String {
        format!("{}{text}", self.document)
    }
}

/// Check a prefix: at most MAX_PREFIX_CHARS characters, no control characters (no line break either); blanks
/// stay, also at the end. Error → RagError::Invalid with a German sentence that names `field`.
pub fn check_prefix(field: &str, value: &str) -> Result<String, RagError> {
    if value.chars().any(char::is_control) {
        return Err(RagError::Invalid(format!(
            "{field}: Das Präfix darf keine Steuerzeichen (auch keinen Zeilenumbruch) enthalten."
        )));
    }
    let chars = value.chars().count();
    if chars > MAX_PREFIX_CHARS {
        return Err(RagError::Invalid(format!(
            "{field}: Das Präfix ist {chars} Zeichen lang, erlaubt sind höchstens {MAX_PREFIX_CHARS}."
        )));
    }
    Ok(value.to_owned())
}

/// One search way (same names as `baruch_store::RagWay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Way {
    Lancedb,
    Pgvector,
    Multigpt,
}

impl Way {
    /// All ways in a fixed order (also the order on a tie).
    pub const ALL: [Way; 3] = [Way::Lancedb, Way::Pgvector, Way::Multigpt];

    pub fn as_str(self) -> &'static str {
        match self {
            Way::Lancedb => "lancedb",
            Way::Pgvector => "pgvector",
            Way::Multigpt => "multigpt",
        }
    }

    /// Display name.
    pub fn title(self) -> &'static str {
        match self {
            Way::Lancedb => "LanceDB (lokal)",
            Way::Pgvector => "pgvector",
            Way::Multigpt => "MultiGPT",
        }
    }

    /// Text → way; unknown → `None`.
    pub fn parse(s: &str) -> Option<Way> {
        Way::ALL.into_iter().find(|w| w.as_str() == s.trim())
    }
}

impl fmt::Display for Way {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A search query to the service (RPC `rag.search`, MCP `rag_search`, user interface, personas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    /// Work (`baruch_store::WorkId`).
    pub work: i64,
    pub query: String,
    /// `None` = the ways of the work; otherwise only these (intersected with the ways of the work).
    pub ways: Option<Vec<Way>>,
    /// `None` = all; LanceDB: kinds `section`, `idea`, `highlight`, `library`; remote: intersection with the
    /// collections chosen for the work.
    pub collections: Option<Vec<String>>,
    /// 1..=[`MAX_LIMIT`].
    pub limit: usize,
    /// Who asks: „Autor“, „@mcp:<name>“ or the handle of the persona.
    pub origin: String,
}

/// A search query to one way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Query, trimmed at both ends, 1..=[`MAX_QUERY_CHARS`] characters.
    pub text: String,
    /// 1..=[`MAX_LIMIT`].
    pub limit: usize,
    /// pgvector/MultiGPT: the collections to search (never empty, otherwise the way is not asked);
    /// LanceDB: kinds `section`, `idea`, `highlight`, `library`, empty = all.
    pub collections: Vec<String>,
    /// Work (`baruch_store::WorkId`) for LanceDB; the remote ways ignore it.
    pub work: i64,
}

/// A hit of one way.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    pub way: Way,
    /// Collection (pgvector/MultiGPT) or kind with LanceDB (`section`, `idea`, `highlight`, `library`).
    pub collection: String,
    /// Document: with LanceDB `work:<id>`, `idea:<id>`, `title:<id>`; remote the identifier of the remote side.
    pub document: String,
    /// Place in the document (LanceDB `key:<key>`, `highlight:<id>`, `quote:<id>`); `None` = whole document.
    pub section: Option<String>,
    /// Title for the display.
    pub title: String,
    /// Snippet, at most [`MAX_SNIPPET_CHARS`] characters ([`clean_snippet`]).
    pub snippet: String,
    /// Similarity 0..=1 (1 = equal) within the way; not comparable across ways.
    pub score: f32,
    /// Address: relative in Baruch (starts with `/`) or http(s) of the remote side; `None` = none.
    pub url: Option<String>,
    /// Additional details of the way (e.g. `key`, `rev`, `chunk`, `idea`); never secrets.
    pub meta: Value,
}

impl Hit {
    /// Key for duplicates: document and place, trimmed and in lower case.
    pub fn identity(&self) -> (String, String) {
        (self.document.trim().to_lowercase(), self.section.as_deref().unwrap_or("").trim().to_lowercase())
    }
}

/// Rank of a hit in one way (1 = best).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WayRank {
    pub way: Way,
    pub rank: usize,
    pub score: f32,
}

/// Hit after merging.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergedHit {
    /// Representative: the hit with the best rank (tie: the earlier way in [`Way::ALL`]).
    #[serde(flatten)]
    pub hit: Hit,
    /// Position in the answer, 1..=n.
    pub rank: usize,
    /// Sum over `found_by` of 1 / ([`RRF_K`] + rank).
    pub fused: f64,
    /// All ways that found this document or this place, in the order of [`Way::ALL`].
    pub found_by: Vec<WayRank>,
}

/// State of one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WayState {
    /// Asked and answered (also with 0 hits).
    Connected,
    /// Asked, but failed (error, time limit, model does not fit).
    Failed,
    /// Not asked: not chosen, not set up or no collection chosen.
    Off,
}

impl WayState {
    /// „verbunden“, „ausgefallen“, „aus“.
    pub fn label(self) -> &'static str {
        match self {
            WayState::Connected => "verbunden",
            WayState::Failed => "ausgefallen",
            WayState::Off => "aus",
        }
    }
}

/// Report per way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WayReport {
    pub way: Way,
    pub state: WayState,
    /// [`WayState::label`].
    pub label: &'static str,
    /// German sentence: reason with `failed`/`off`, otherwise short; never a secret.
    pub message: String,
    pub hits: usize,
    pub millis: u64,
}

impl WayReport {
    pub fn new(way: Way, state: WayState, message: impl Into<String>, hits: usize, millis: u64) -> WayReport {
        WayReport { way, state, label: state.label(), message: message.into(), hits, millis }
    }
}

/// Answer of a search.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub work: i64,
    pub query: String,
    pub hits: Vec<MergedHit>,
    /// Always all three ways in the order of [`Way::ALL`].
    pub ways: Vec<WayReport>,
}

/// State of the local index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexInfo {
    /// Folder of the index (absolute).
    pub dir: String,
    /// Entries (chunks) of the work with its ideas and library.
    pub entries: u64,
    /// Model and dimension the index is built with; `None` = not built yet.
    pub model: Option<String>,
    pub dimension: Option<usize>,
    /// Configured model (connection LanceDB, `embedding_model`).
    pub configured_model: String,
    /// Last complete build (ISO 8601) or `None`.
    pub built: Option<String>,
    /// Pending jobs.
    pub pending: usize,
    /// Stale jobs (after the attempts are used up; tried again only after new content or „Index neu aufbauen“).
    pub stale: usize,
    /// Why the semantic search does not work at the moment; `None` = it works.
    pub problem: Option<String>,
}

/// State of all ways of one work (determined without the network).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    pub work: i64,
    /// Always all three ways in the order of [`Way::ALL`].
    pub ways: Vec<WayReport>,
    pub index: IndexInfo,
    /// The wording search (FTS5) is always there.
    pub fts: bool,
}

/// Error of one way or of the service. No text contains a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RagError {
    /// Input invalid (empty query, limit exceeded).
    Invalid(String),
    /// Way not set up (no address, no host, secret belongs to another target); way „aus“.
    NotConfigured(String),
    /// No embedding model reachable; the wording search stays.
    NoEmbedder(String),
    /// Embedding model or dimension do not fit the index or the data.
    ModelMismatch(String),
    Unreachable(String),
    Denied(String),
    Timeout(String),
    /// Schema or answer of the remote side does not fit (table, column, tool, format).
    Schema(String),
    Failed(String),
}

impl RagError {
    /// Fixed code.
    pub fn code(&self) -> &'static str {
        match self {
            RagError::Invalid(_) => "invalid",
            RagError::NotConfigured(_) => "not_configured",
            RagError::NoEmbedder(_) => "no_embedder",
            RagError::ModelMismatch(_) => "model_mismatch",
            RagError::Unreachable(_) => "unreachable",
            RagError::Denied(_) => "denied",
            RagError::Timeout(_) => "timeout",
            RagError::Schema(_) => "schema",
            RagError::Failed(_) => "failed",
        }
    }

    /// German sentence.
    pub fn message(&self) -> &str {
        match self {
            RagError::Invalid(m)
            | RagError::NotConfigured(m)
            | RagError::NoEmbedder(m)
            | RagError::ModelMismatch(m)
            | RagError::Unreachable(m)
            | RagError::Denied(m)
            | RagError::Timeout(m)
            | RagError::Schema(m)
            | RagError::Failed(m) => m,
        }
    }

    /// Way „aus“ instead of „ausgefallen“?
    pub fn is_off(&self) -> bool {
        matches!(self, RagError::NotConfigured(_))
    }
}

impl fmt::Display for RagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for RagError {}

/// Vectors of one embedding request.
#[derive(Debug, Clone, PartialEq)]
pub struct Embeddings {
    /// Model that actually answered.
    pub model: String,
    pub dimension: usize,
    /// One vector of length `dimension` per input text, same order.
    pub vectors: Vec<Vec<f32>>,
}

/// Port for embeddings (Eliezer through the OpenAI-compatible interface or a mock).
pub trait Embedder: Send + Sync {
    /// Model name as it goes to the backend.
    fn model(&self) -> &str;
    /// 1..=[`MAX_EMBED_BATCH`] texts → vectors; more or none → [`RagError::Invalid`].
    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>>;
}

/// Provides an embedder for a given model.
pub trait EmbedderFactory: Send + Sync {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder>;
}

/// Port for one search way.
pub trait SearchWay: Send + Sync {
    fn way(&self) -> Way;
    /// Hits of this way (at most `query.limit`), by similarity descending.
    fn search<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<Hit>, RagError>>;
}

/// Clean a snippet: control characters and whitespace except line breaks → blank, trimmed,
/// at most [`MAX_SNIPPET_CHARS`] characters (otherwise with „…“ at the end).
pub fn clean_snippet(text: &str) -> String {
    let cleaned: String =
        text.chars().map(|c| if c != '\n' && (c.is_control() || c.is_whitespace()) { ' ' } else { c }).collect();
    let trimmed = cleaned.trim();
    if trimmed.chars().count() <= MAX_SNIPPET_CHARS {
        return trimmed.to_owned();
    }
    let mut cut: String = trimmed.chars().take(MAX_SNIPPET_CHARS - 1).collect();
    cut.push('…');
    cut
}
