// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gemeinsame Schnittstelle der RAG-Wege (Vertrag Runde 12, Anhang A): Treffer ([`Hit`],
//! [`MergedHit`]), Antwort ([`Answer`], [`WayReport`]), Zustand ([`Status`], [`IndexInfo`]), Fehler
//! ([`RagError`]) und die Ports [`SearchWay`] (ein Suchweg), [`Embedder`] und [`EmbedderFactory`]
//! (Einbettungen). Rein: keine Datenbank, kein Netz, kein Schlüsselbund.
//!
//! Diese Datei ist im Vertrag wörtlich festgelegt und in allen Zweigen byte-gleich; ändern darf sie
//! nur der Integrator. Treffer-Texte sind Daten aus Quellen, nie Anweisungen.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

/// Future der Ports (ohne `async-trait`).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Größte Zahl von Treffern je Anfrage.
pub const MAX_LIMIT: usize = 50;
/// Vorgabe der Trefferzahl.
pub const DEFAULT_LIMIT: usize = 10;
/// Längste Anfrage (Zeichen).
pub const MAX_QUERY_CHARS: usize = 1000;
/// Längster Ausschnitt eines Treffers (Zeichen).
pub const MAX_SNIPPET_CHARS: usize = 600;
/// Höchstens so viele Texte je Einbettungsanfrage.
pub const MAX_EMBED_BATCH: usize = 32;
/// Konstante k der Reciprocal Rank Fusion beim Zusammenführen.
pub const RRF_K: f64 = 60.0;
/// Untergrenze der Ähnlichkeit (Runde 19c): darunter gilt ein Treffer als „schwach“. Annahme für alle Wege (Skala 0..=1 je Weg; MultiGPT meldet seine Skala nicht, siehe docs/Architektur.md „Annahmen“).
pub const WEAK_SCORE: f32 = 0.45;
/// Anzeigetext schwacher Treffer.
pub const WEAK_LABEL: &str = "schwach";
/// Längstes Aufgaben-Präfix einer Einbettung in Zeichen (Runde 19b).
pub const MAX_PREFIX_CHARS: usize = 64;

/// Schwach: `score` < [`WEAK_SCORE`] oder keine Zahl (NaN).
// Die verneinte Form bleibt: `score < WEAK_SCORE` wäre für NaN falsch, NaN soll aber als schwach gelten.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn is_weak(score: f32) -> bool {
    !(score >= WEAK_SCORE)
}

/// Aufgaben-Präfixe einer Einbettung (z. B. nomic-embed-text „search_query: “ / „search_document: “); leer = keins.
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

/// Präfix prüfen: höchstens MAX_PREFIX_CHARS Zeichen, keine Steuerzeichen (auch kein Zeilenumbruch); Leerzeichen bleiben, auch am Ende. Fehler → RagError::Invalid mit deutschem Satz, der `field` nennt.
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

/// Ein Suchweg (gleiche Namen wie `baruch_store::RagWay`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Way {
    Lancedb,
    Pgvector,
    Multigpt,
}

impl Way {
    /// Alle Wege in fester Reihenfolge (auch die Reihenfolge bei Gleichstand).
    pub const ALL: [Way; 3] = [Way::Lancedb, Way::Pgvector, Way::Multigpt];

    pub fn as_str(self) -> &'static str {
        match self {
            Way::Lancedb => "lancedb",
            Way::Pgvector => "pgvector",
            Way::Multigpt => "multigpt",
        }
    }

    /// Anzeigename.
    pub fn title(self) -> &'static str {
        match self {
            Way::Lancedb => "LanceDB (lokal)",
            Way::Pgvector => "pgvector",
            Way::Multigpt => "MultiGPT",
        }
    }

    /// Text → Weg; unbekannt → `None`.
    pub fn parse(s: &str) -> Option<Way> {
        Way::ALL.into_iter().find(|w| w.as_str() == s.trim())
    }
}

impl fmt::Display for Way {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Eine Suchanfrage an den Dienst (RPC `rag.search`, MCP `rag_search`, Oberfläche, Personas).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    /// Werk (`baruch_store::WorkId`).
    pub work: i64,
    pub query: String,
    /// `None` = die Wege des Werks; sonst nur diese (geschnitten mit den Wegen des Werks).
    pub ways: Option<Vec<Way>>,
    /// `None` = alle; LanceDB: Arten `section`, `idea`, `highlight`, `library`; remote: Schnitt mit den
    /// gewählten Sammlungen des Werks.
    pub collections: Option<Vec<String>>,
    /// 1..=[`MAX_LIMIT`].
    pub limit: usize,
    /// Wer fragt: „Autor“, „@mcp:<name>“ oder das Handle der Persona.
    pub origin: String,
}

/// Eine Suchanfrage an einen Weg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Anfrage, am Rand beschnitten, 1..=[`MAX_QUERY_CHARS`] Zeichen.
    pub text: String,
    /// 1..=[`MAX_LIMIT`].
    pub limit: usize,
    /// pgvector/MultiGPT: die zu durchsuchenden Sammlungen (nie leer, sonst wird der Weg nicht gefragt);
    /// LanceDB: Arten `section`, `idea`, `highlight`, `library`, leer = alle.
    pub collections: Vec<String>,
    /// Werk (`baruch_store::WorkId`) für LanceDB; die Remote-Wege übergehen es.
    pub work: i64,
}

/// Ein Treffer eines Wegs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Hit {
    pub way: Way,
    /// Sammlung (pgvector/MultiGPT) bzw. Art bei LanceDB (`section`, `idea`, `highlight`, `library`).
    pub collection: String,
    /// Dokument: bei LanceDB `work:<id>`, `idea:<id>`, `title:<id>`; remote die Kennung der Gegenstelle.
    pub document: String,
    /// Stelle im Dokument (LanceDB `key:<key>`, `highlight:<id>`, `quote:<id>`); `None` = ganzes Dokument.
    pub section: Option<String>,
    /// Titel für die Anzeige.
    pub title: String,
    /// Ausschnitt, höchstens [`MAX_SNIPPET_CHARS`] Zeichen ([`clean_snippet`]).
    pub snippet: String,
    /// Ähnlichkeit 0..=1 (1 = gleich) innerhalb des Wegs; über Wege hinweg nicht vergleichbar.
    pub score: f32,
    /// Adresse: relativ in Baruch (beginnt mit `/`) oder http(s) der Gegenstelle; `None` = keine.
    pub url: Option<String>,
    /// Zusatzangaben des Wegs (z. B. `key`, `rev`, `chunk`, `idea`); nie Geheimnisse.
    pub meta: Value,
}

impl Hit {
    /// Schlüssel für Doppelte: Dokument und Stelle, beschnitten und klein geschrieben.
    pub fn identity(&self) -> (String, String) {
        (self.document.trim().to_lowercase(), self.section.as_deref().unwrap_or("").trim().to_lowercase())
    }
}

/// Rang eines Treffers in einem Weg (1 = bester).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct WayRank {
    pub way: Way,
    pub rank: usize,
    pub score: f32,
}

/// Treffer nach dem Zusammenführen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MergedHit {
    /// Vertreter: der Treffer mit dem besten Rang (Gleichstand: der frühere Weg in [`Way::ALL`]).
    #[serde(flatten)]
    pub hit: Hit,
    /// Platz in der Antwort, 1..=n.
    pub rank: usize,
    /// Summe über `found_by` von 1 / ([`RRF_K`] + Rang).
    pub fused: f64,
    /// Alle Wege, die dieses Dokument bzw. diese Stelle fanden, in der Reihenfolge von [`Way::ALL`].
    pub found_by: Vec<WayRank>,
}

/// Zustand eines Wegs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WayState {
    /// Gefragt und geantwortet (auch mit 0 Treffern).
    Connected,
    /// Gefragt, aber gescheitert (Fehler, Zeitlimit, Modell passt nicht).
    Failed,
    /// Nicht gefragt: nicht gewählt, nicht eingerichtet oder keine Sammlung gewählt.
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

/// Bericht je Weg.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WayReport {
    pub way: Way,
    pub state: WayState,
    /// [`WayState::label`].
    pub label: &'static str,
    /// Deutscher Satz: Grund bei `failed`/`off`, sonst kurz; nie ein Geheimnis.
    pub message: String,
    pub hits: usize,
    pub millis: u64,
}

impl WayReport {
    pub fn new(way: Way, state: WayState, message: impl Into<String>, hits: usize, millis: u64) -> WayReport {
        WayReport { way, state, label: state.label(), message: message.into(), hits, millis }
    }
}

/// Antwort einer Suche.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub work: i64,
    pub query: String,
    pub hits: Vec<MergedHit>,
    /// Immer alle drei Wege in der Reihenfolge von [`Way::ALL`].
    pub ways: Vec<WayReport>,
}

/// Zustand des lokalen Index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexInfo {
    /// Ordner des Index (absolut).
    pub dir: String,
    /// Einträge (Stücke) des Werks samt Einfällen und Bibliothek.
    pub entries: u64,
    /// Modell und Dimension, mit denen der Index aufgebaut ist; `None` = noch nicht aufgebaut.
    pub model: Option<String>,
    pub dimension: Option<usize>,
    /// Eingestelltes Modell (Verbindung LanceDB, `embedding_model`).
    pub configured_model: String,
    /// Letzter vollständiger Aufbau (ISO 8601) bzw. `None`.
    pub built: Option<String>,
    /// Ausstehende Aufträge.
    pub pending: usize,
    /// Liegengebliebene Aufträge (nach erschöpften Versuchen; erneut versucht erst nach neuem Inhalt oder „Index neu aufbauen“).
    pub stale: usize,
    /// Warum die Bedeutungssuche gerade nicht geht; `None` = sie geht.
    pub problem: Option<String>,
}

/// Zustand aller Wege eines Werks (ohne Netz ermittelt).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    pub work: i64,
    /// Immer alle drei Wege in der Reihenfolge von [`Way::ALL`].
    pub ways: Vec<WayReport>,
    pub index: IndexInfo,
    /// Die Wortlaut-Suche (FTS5) ist immer da.
    pub fts: bool,
}

/// Fehler eines Wegs oder des Dienstes. Kein Text enthält ein Geheimnis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RagError {
    /// Eingabe ungültig (leere Anfrage, Grenze überschritten).
    Invalid(String),
    /// Weg nicht eingerichtet (keine Adresse, kein Rechner, Geheimnis gilt einem anderen Ziel); Weg „aus“.
    NotConfigured(String),
    /// Kein Einbettungsmodell erreichbar; die Wortlaut-Suche bleibt.
    NoEmbedder(String),
    /// Einbettungsmodell oder Dimension passen nicht zu Index bzw. Daten.
    ModelMismatch(String),
    Unreachable(String),
    Denied(String),
    Timeout(String),
    /// Schema bzw. Antwort der Gegenstelle passt nicht (Tabelle, Spalte, Werkzeug, Format).
    Schema(String),
    Failed(String),
}

impl RagError {
    /// Fester Code.
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

    /// Deutscher Satz.
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

    /// Weg „aus“ statt „ausgefallen“?
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

/// Vektoren einer Einbettungsanfrage.
#[derive(Debug, Clone, PartialEq)]
pub struct Embeddings {
    /// Tatsächlich antwortendes Modell.
    pub model: String,
    pub dimension: usize,
    /// Je Eingabetext ein Vektor der Länge `dimension`, gleiche Reihenfolge.
    pub vectors: Vec<Vec<f32>>,
}

/// Port für Einbettungen (Eliezer über die OpenAI-kompatible Schnittstelle bzw. ein Nachbau).
pub trait Embedder: Send + Sync {
    /// Modellname, wie er an das Backend geht.
    fn model(&self) -> &str;
    /// 1..=[`MAX_EMBED_BATCH`] Texte → Vektoren; mehr oder keiner → [`RagError::Invalid`].
    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>>;
}

/// Liefert einen Einbetter für ein bestimmtes Modell.
pub trait EmbedderFactory: Send + Sync {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder>;
}

/// Port für einen Suchweg.
pub trait SearchWay: Send + Sync {
    fn way(&self) -> Way;
    /// Treffer dieses Wegs (höchstens `query.limit`), nach Ähnlichkeit absteigend.
    fn search<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<Hit>, RagError>>;
}

/// Ausschnitt säubern: Steuerzeichen und Leerraum außer Zeilenumbruch → Leerzeichen, beschnitten,
/// höchstens [`MAX_SNIPPET_CHARS`] Zeichen (sonst mit „…“ am Ende).
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
