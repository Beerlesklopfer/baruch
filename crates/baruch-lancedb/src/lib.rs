// SPDX-License-Identifier: AGPL-3.0-or-later
//! Main RAG of Baruch (Round 12): a local vector index on [LanceDB](https://lancedb.com) (Apache-2.0) in a folder
//! of the data folder, plus the splitting of the texts into chunks ([`chunk`]).
//!
//! Pure: no SQLite, no network, no keyring. What belongs into the index and the vectors for it are brought by the
//! caller (the server, `crate::rag`); here is only storing, comparing and searching.
//!
//! Structure:
//! - One table `chunks` with the columns `id` (unique, e.g. `section:<key>:<n>`, `idea:<id>`), `work` (work or
//!   `NULL` for ideas without a work and the library), `kind` (`section`, `idea`, `highlight`, `library`),
//!   `document`, `section`, `title`, `text` (the embedded text), `meta` (JSON), `content_hash` (hash of the
//!   embedded text: equal hashes share the vector), `row_hash` (hash of the whole row without vector: unchanged =
//!   nothing to do) and `vector` (`FixedSizeList<Float32, dimension>`).
//! - `baruch-index.json` in the same folder: model (requested and answering), dimension, times of the last build.
//!   An index has exactly **one** model; if the configured one does not match, the caller mixes nothing into it
//!   until it discards the index ([`Index::reset`]).
//! - The search uses cosine distance without an ANN index (flat search, fast enough for the size of a book);
//!   `score` = 1 − distance, clamped to 0..=1.
//!
//! All accesses to LanceDB run on a small runtime of their own ([`runtime`]): this way they neither hang on the
//! runtime of a test nor block that of the server.

pub mod chunk;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use baruch_rag::RagError;
use futures_util::TryStreamExt;
use lancedb::arrow::arrow_array::types::Float32Type;
use lancedb::arrow::arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int64Array, RecordBatch, StringArray, cast::AsArray,
};
use lancedb::arrow::arrow_schema::{DataType, Field, Schema, SchemaRef};
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::{Connection, DistanceType, Table};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Name of the table.
pub const TABLE: &str = "chunks";
/// File with model and dimension of the index.
pub const META_FILE: &str = "baruch-index.json";
/// Version of the layout (if the schema changes, the index is rebuilt).
pub const LAYOUT: u32 = 1;
/// At most this many identifiers per delete statement.
const DELETE_BATCH: usize = 500;

/// Own runtime for LanceDB (two worker threads).
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("baruch-lancedb")
            .enable_all()
            .build()
            .expect("Tokio-Laufzeit für LanceDB")
    });
    &RT
}

/// Run on [`runtime`] and wait for the result (from within any runtime).
pub async fn on_runtime<T: Send + 'static>(
    future: impl std::future::Future<Output = Result<T, RagError>> + Send + 'static,
) -> Result<T, RagError> {
    runtime().spawn(future).await.map_err(|e| RagError::Failed(format!("LanceDB-Aufgabe abgebrochen: {e}")))?
}

fn failed(what: &str) -> impl Fn(lancedb::Error) -> RagError + '_ {
    move |e| RagError::Failed(format!("LanceDB ({what}): {e}"))
}

/// Model and dimension of the index (`baruch-index.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexMeta {
    pub layout: u32,
    /// Requested model (after alias) the index is built with.
    pub requested: Option<String>,
    /// According to the answer of the backend.
    pub model: Option<String>,
    pub dimension: Option<usize>,
    /// Last complete sync per scope (`work:<id>`, `shared`), ISO 8601.
    #[serde(default)]
    pub built: BTreeMap<String, String>,
    /// Task prefix the documents are embedded with (Round 19b, e.g. "search_document: "); `None` (older indexes)
    /// counts like "no prefix". The caller mixes a different configured prefix as little as a different model.
    #[serde(default)]
    pub document_prefix: Option<String>,
}

/// Which rows are meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Rows of a work (`work = W`).
    Work(i64),
    /// Rows without a work (ideas without a work, library).
    Shared,
    /// What a search in the work sees: the work and everything without a work.
    Visible(i64),
    /// All rows of works except these (deleted works).
    OtherWorks(Vec<i64>),
}

impl Scope {
    fn filter(&self) -> String {
        match self {
            Scope::Work(w) => format!("work = {w}"),
            Scope::Shared => "work IS NULL".to_owned(),
            Scope::Visible(w) => format!("(work = {w} OR work IS NULL)"),
            Scope::OtherWorks(keep) if keep.is_empty() => "work IS NOT NULL".to_owned(),
            Scope::OtherWorks(keep) => {
                let list: Vec<String> = keep.iter().map(i64::to_string).collect();
                format!("work IS NOT NULL AND work NOT IN ({})", list.join(", "))
            }
        }
    }

    /// Does a row with this work (`None` = without a work) belong to the scope?
    pub fn contains(&self, work: Option<i64>) -> bool {
        match (self, work) {
            (Scope::Work(w), Some(x)) => *w == x,
            (Scope::Work(_), None) => false,
            (Scope::Shared, w) => w.is_none(),
            (Scope::Visible(w), x) => x.is_none_or(|x| x == *w),
            (Scope::OtherWorks(keep), Some(x)) => !keep.contains(&x),
            (Scope::OtherWorks(_), None) => false,
        }
    }
}

/// A row without vector.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub id: String,
    pub work: Option<i64>,
    pub kind: String,
    pub document: String,
    pub section: Option<String>,
    pub title: String,
    /// Embedded text.
    pub text: String,
    pub meta: Value,
}

impl Entry {
    /// Hash of the embedded text.
    pub fn content_hash(&self) -> String {
        chunk::hash(&self.text)
    }

    /// Hash of the whole row without vector (if text, place or extra data changes, it changes).
    pub fn row_hash(&self) -> String {
        let parts = serde_json::json!([
            self.id,
            self.work,
            self.kind,
            self.document,
            self.section,
            self.title,
            self.text,
            self.meta
        ]);
        chunk::hash(&parts.to_string())
    }
}

/// A stored row.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub id: String,
    pub row_hash: String,
    pub content_hash: String,
    pub vector: Vec<f32>,
}

/// A hit of the search.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub entry: Entry,
    /// Cosine distance (0 = same direction).
    pub distance: f32,
}

impl Found {
    /// Similarity 0..=1 (1 − cosine distance, clamped).
    pub fn score(&self) -> f32 {
        let s = 1.0 - self.distance;
        if s.is_nan() { 0.0 } else { s.clamp(0.0, 1.0) }
    }
}

/// The index in a folder.
#[derive(Clone)]
pub struct Index {
    dir: PathBuf,
    db: Connection,
}

fn schema(dimension: usize) -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("work", DataType::Int64, true),
        Field::new("kind", DataType::Utf8, false),
        Field::new("document", DataType::Utf8, false),
        Field::new("section", DataType::Utf8, true),
        Field::new("title", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("meta", DataType::Utf8, false),
        Field::new("content_hash", DataType::Utf8, false),
        Field::new("row_hash", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dimension as i32),
            false,
        ),
    ]))
}

fn quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn strings<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray, RagError> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_string_opt::<i32>())
        .ok_or_else(|| RagError::Schema(format!("LanceDB: Spalte {name} fehlt oder ist kein Text.")))
}

fn text_at(array: &StringArray, row: usize) -> Option<String> {
    (!array.is_null(row)).then(|| array.value(row).to_owned())
}

fn entries(batch: &RecordBatch) -> Result<Vec<Entry>, RagError> {
    let (id, kind, document, section, title, text, meta) = (
        strings(batch, "id")?,
        strings(batch, "kind")?,
        strings(batch, "document")?,
        strings(batch, "section")?,
        strings(batch, "title")?,
        strings(batch, "text")?,
        strings(batch, "meta")?,
    );
    let work = batch
        .column_by_name("work")
        .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
        .ok_or_else(|| RagError::Schema("LanceDB: Spalte work fehlt.".into()))?;
    Ok((0..batch.num_rows())
        .map(|row| Entry {
            id: id.value(row).to_owned(),
            work: (!work.is_null(row)).then(|| work.value(row)),
            kind: kind.value(row).to_owned(),
            document: document.value(row).to_owned(),
            section: text_at(section, row),
            title: title.value(row).to_owned(),
            text: text.value(row).to_owned(),
            meta: serde_json::from_str(meta.value(row)).unwrap_or(Value::Null),
        })
        .collect())
}

fn vectors(batch: &RecordBatch) -> Result<Vec<Vec<f32>>, RagError> {
    let list = batch
        .column_by_name("vector")
        .and_then(|c| c.as_fixed_size_list_opt())
        .ok_or_else(|| RagError::Schema("LanceDB: Spalte vector fehlt.".into()))?;
    let mut out = Vec::with_capacity(list.len());
    for row in 0..list.len() {
        let values = list.value(row);
        let floats = values
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or_else(|| RagError::Schema("LanceDB: vector ist kein Float32.".into()))?;
        out.push(floats.values().to_vec());
    }
    Ok(out)
}

fn batch_of(rows: &[(Entry, Vec<f32>)], dimension: usize) -> Result<RecordBatch, RagError> {
    let texts = |f: &dyn Fn(&Entry) -> String| -> Arc<dyn Array> {
        Arc::new(StringArray::from(rows.iter().map(|(e, _)| f(e)).collect::<Vec<_>>()))
    };
    if let Some((e, v)) = rows.iter().find(|(_, v)| v.len() != dimension) {
        return Err(RagError::ModelMismatch(format!(
            "Vektor von {} hat {} statt {dimension} Dimensionen; nichts gespeichert.",
            e.id,
            v.len()
        )));
    }
    let columns: Vec<Arc<dyn Array>> = vec![
        texts(&|e| e.id.clone()),
        Arc::new(Int64Array::from(rows.iter().map(|(e, _)| e.work).collect::<Vec<_>>())),
        texts(&|e| e.kind.clone()),
        texts(&|e| e.document.clone()),
        Arc::new(StringArray::from(rows.iter().map(|(e, _)| e.section.clone()).collect::<Vec<_>>())),
        texts(&|e| e.title.clone()),
        texts(&|e| e.text.clone()),
        texts(&|e| e.meta.to_string()),
        texts(&Entry::content_hash),
        texts(&Entry::row_hash),
        Arc::new(FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
            rows.iter().map(|(_, v)| Some(v.iter().map(|x| Some(*x)).collect::<Vec<_>>())),
            dimension as i32,
        )),
    ];
    RecordBatch::try_new(schema(dimension), columns).map_err(|e| RagError::Failed(format!("LanceDB (Stapel): {e}")))
}

impl Index {
    /// Create the folder (if needed) and open the index.
    pub async fn open(dir: &Path) -> Result<Index, RagError> {
        let dir = dir.to_owned();
        on_runtime(async move {
            std::fs::create_dir_all(&dir)
                .map_err(|e| RagError::Failed(format!("Ordner {} nicht anlegbar: {e}", dir.display())))?;
            let uri = dir.to_str().ok_or_else(|| RagError::Failed("Ordnername ist kein UTF-8.".into()))?.to_owned();
            let db = lancedb::connect(&uri).execute().await.map_err(failed("öffnen"))?;
            Ok(Index { dir, db })
        })
        .await
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Model and dimension from `baruch-index.json` (if the file is missing: empty).
    pub fn meta(&self) -> Result<IndexMeta, RagError> {
        let path = self.dir.join(META_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| {
                RagError::Schema(format!("{} ist beschädigt ({e}); „Neu aufbauen“ hilft.", path.display()))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(IndexMeta { layout: LAYOUT, ..IndexMeta::default() })
            }
            Err(e) => Err(RagError::Failed(format!("{} nicht lesbar: {e}", path.display()))),
        }
    }

    /// Write `baruch-index.json` (through an intermediate file, so that never half a file is left).
    pub fn set_meta(&self, meta: &IndexMeta) -> Result<(), RagError> {
        let path = self.dir.join(META_FILE);
        let tmp = self.dir.join(format!("{META_FILE}.tmp"));
        let text = serde_json::to_string_pretty(meta).map_err(|e| RagError::Failed(e.to_string()))?;
        std::fs::write(&tmp, text)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| RagError::Failed(format!("{} nicht schreibbar: {e}", path.display())))
    }

    async fn table(&self) -> Result<Option<Table>, RagError> {
        let db = self.db.clone();
        on_runtime(async move {
            let names = db.table_names().execute().await.map_err(failed("Tabellen"))?;
            if !names.iter().any(|n| n == TABLE) {
                return Ok(None);
            }
            Ok(Some(db.open_table(TABLE).execute().await.map_err(failed("Tabelle öffnen"))?))
        })
        .await
    }

    async fn table_or_create(&self, dimension: usize) -> Result<Table, RagError> {
        if let Some(table) = self.table().await? {
            return Ok(table);
        }
        let db = self.db.clone();
        on_runtime(async move {
            db.create_empty_table(TABLE, schema(dimension)).execute().await.map_err(failed("Tabelle anlegen"))
        })
        .await
    }

    /// Names of the tables in the folder.
    pub async fn tables(&self) -> Result<Vec<String>, RagError> {
        let db = self.db.clone();
        on_runtime(async move { db.table_names().execute().await.map_err(failed("Tabellen")) }).await
    }

    /// Is there a table already?
    pub async fn exists(&self) -> Result<bool, RagError> {
        Ok(self.table().await?.is_some())
    }

    /// Number of rows in the scope (`None` = all).
    pub async fn count(&self, scope: Option<Scope>) -> Result<u64, RagError> {
        let Some(table) = self.table().await? else { return Ok(0) };
        on_runtime(async move {
            let n = table.count_rows(scope.map(|s| s.filter())).await.map_err(failed("zählen"))?;
            Ok(n as u64)
        })
        .await
    }

    /// Stored rows of the scope with hashes and vectors.
    pub async fn stored(&self, scope: Scope) -> Result<Vec<Stored>, RagError> {
        let Some(table) = self.table().await? else { return Ok(Vec::new()) };
        on_runtime(async move {
            let columns = ["id", "row_hash", "content_hash", "vector"].map(str::to_owned).to_vec();
            let batches: Vec<RecordBatch> = table
                .query()
                .only_if(scope.filter())
                .select(Select::Columns(columns))
                .execute()
                .await
                .map_err(failed("lesen"))?
                .try_collect()
                .await
                .map_err(failed("lesen"))?;
            let mut out = Vec::new();
            for batch in &batches {
                let (id, row_hash, content_hash) =
                    (strings(batch, "id")?, strings(batch, "row_hash")?, strings(batch, "content_hash")?);
                for (row, vector) in vectors(batch)?.into_iter().enumerate() {
                    out.push(Stored {
                        id: id.value(row).to_owned(),
                        row_hash: row_hash.value(row).to_owned(),
                        content_hash: content_hash.value(row).to_owned(),
                        vector,
                    });
                }
            }
            Ok(out)
        })
        .await
    }

    /// Remove rows in the scope `scope` (by identifier, **only** within the scope: an equal identifier in another
    /// work stays) and insert new ones. A new row outside the scope → [`RagError::Invalid`], nothing changed.
    /// `dimension` is that of the index.
    pub async fn replace(
        &self,
        scope: Scope,
        remove: Vec<String>,
        rows: Vec<(Entry, Vec<f32>)>,
        dimension: usize,
    ) -> Result<(), RagError> {
        if let Some((outside, _)) = rows.iter().find(|(e, _)| !scope.contains(e.work)) {
            return Err(RagError::Invalid(format!(
                "LanceDB: Zeile {} (Werk {}) liegt außerhalb des Bereichs {}; nichts geändert.",
                outside.id,
                outside.work.map_or_else(|| "ohne".to_owned(), |w| w.to_string()),
                scope.filter()
            )));
        }
        if remove.is_empty() && rows.is_empty() {
            return Ok(());
        }
        let batch = if rows.is_empty() { None } else { Some(batch_of(&rows, dimension)?) };
        let table = self.table_or_create(dimension).await?;
        on_runtime(async move {
            let within = scope.filter();
            for ids in remove.chunks(DELETE_BATCH) {
                let list: Vec<String> = ids.iter().map(|id| quoted(id)).collect();
                let filter = format!("id IN ({}) AND {within}", list.join(", "));
                table.delete(filter.as_str()).await.map_err(failed("löschen"))?;
            }
            if let Some(batch) = batch {
                table.add(batch).execute().await.map_err(failed("schreiben"))?;
            }
            Ok(())
        })
        .await
    }

    /// Delete all rows of the scope.
    pub async fn delete_scope(&self, scope: Scope) -> Result<(), RagError> {
        let Some(table) = self.table().await? else { return Ok(()) };
        on_runtime(async move {
            table.delete(scope.filter().as_str()).await.map_err(failed("löschen"))?;
            Ok(())
        })
        .await
    }

    /// Discard the whole index: table and `baruch-index.json`.
    pub async fn reset(&self) -> Result<(), RagError> {
        let db = self.db.clone();
        let exists = self.exists().await?;
        on_runtime(async move {
            if exists {
                db.drop_table(TABLE, &[]).await.map_err(failed("verwerfen"))?;
            }
            Ok(())
        })
        .await?;
        match std::fs::remove_file(self.dir.join(META_FILE)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(RagError::Failed(format!("{META_FILE} nicht löschbar: {e}"))),
        }
    }

    /// The `limit` nearest rows in the scope (cosine distance), optionally only of certain kinds.
    pub async fn search(
        &self,
        vector: Vec<f32>,
        scope: Scope,
        kinds: Vec<String>,
        limit: usize,
    ) -> Result<Vec<Found>, RagError> {
        let Some(table) = self.table().await? else { return Ok(Vec::new()) };
        on_runtime(async move {
            let mut filter = scope.filter();
            if !kinds.is_empty() {
                let list: Vec<String> = kinds.iter().map(|k| quoted(k)).collect();
                filter = format!("{filter} AND kind IN ({})", list.join(", "));
            }
            let columns =
                ["id", "work", "kind", "document", "section", "title", "text", "meta"].map(str::to_owned).to_vec();
            let batches: Vec<RecordBatch> = table
                .query()
                .nearest_to(vector)
                .map_err(failed("Suche"))?
                .distance_type(DistanceType::Cosine)
                .only_if(filter)
                .select(Select::Columns(columns))
                .limit(limit.max(1))
                .execute()
                .await
                .map_err(failed("Suche"))?
                .try_collect()
                .await
                .map_err(failed("Suche"))?;
            let mut out = Vec::new();
            for batch in &batches {
                let distance = batch
                    .column_by_name("_distance")
                    .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
                    .ok_or_else(|| RagError::Schema("LanceDB: _distance fehlt.".into()))?;
                for (row, entry) in entries(batch)?.into_iter().enumerate() {
                    out.push(Found { entry, distance: distance.value(row) });
                }
            }
            out.sort_by(|a, b| a.distance.total_cmp(&b.distance).then_with(|| a.entry.id.cmp(&b.entry.id)));
            Ok(out)
        })
        .await
    }
}

/// Cosine similarity of two vectors (for tests and checks).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
