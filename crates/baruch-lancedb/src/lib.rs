// SPDX-License-Identifier: AGPL-3.0-or-later
//! Haupt-RAG von Baruch (Runde 12): ein lokaler Vektorindex über [LanceDB](https://lancedb.com)
//! (Apache-2.0) in einem Ordner des Datenordners, dazu das Zerlegen der Texte in Stücke ([`chunk`]).
//!
//! Rein: keine SQLite, kein Netz, kein Schlüsselbund. Was in den Index gehört und die Vektoren dazu
//! bringt der Aufrufer (der Server, `crate::rag`); hier liegt nur das Speichern, Vergleichen und Suchen.
//!
//! Aufbau:
//! - Eine Tabelle `chunks` mit den Spalten `id` (eindeutig, z. B. `section:<key>:<n>`, `idea:<id>`),
//!   `work` (Werk bzw. `NULL` für Einfälle ohne Werk und die Bibliothek), `kind` (`section`, `idea`,
//!   `highlight`, `library`), `document`, `section`, `title`, `text` (der eingebettete Text), `meta`
//!   (JSON), `content_hash` (Hash des eingebetteten Textes: gleiche Hashes teilen den Vektor),
//!   `row_hash` (Hash der ganzen Zeile ohne Vektor: unverändert = nichts zu tun) und `vector`
//!   (`FixedSizeList<Float32, dimension>`).
//! - `baruch-index.json` im selben Ordner: Modell (angefragt und antwortend), Dimension, Zeitpunkte des
//!   letzten Aufbaus. Ein Index hat genau **ein** Modell; passt das eingestellte nicht, mischt der
//!   Aufrufer nichts hinein, bis er den Index verwirft ([`Index::reset`]).
//! - Gesucht wird mit Kosinus-Abstand ohne ANN-Index (flache Suche, für die Größe eines Buchs schnell
//!   genug); `score` = 1 − Abstand, geklemmt auf 0..=1.
//!
//! Alle Zugriffe auf LanceDB laufen auf einer eigenen kleinen Tokio-Laufzeit ([`runtime`]): So hängen
//! sie weder an der Laufzeit eines Tests noch blockieren sie die des Servers.

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

/// Name der Tabelle.
pub const TABLE: &str = "chunks";
/// Datei mit Modell und Dimension des Index.
pub const META_FILE: &str = "baruch-index.json";
/// Fassung des Aufbaus (ändert sich das Schema, wird neu aufgebaut).
pub const LAYOUT: u32 = 1;
/// Höchstens so viele Kennungen je Löschanweisung.
const DELETE_BATCH: usize = 500;

/// Eigene Laufzeit für LanceDB (zwei Arbeitsfäden).
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

/// Auf [`runtime`] ausführen und auf das Ergebnis warten (aus jeder Laufzeit heraus).
pub async fn on_runtime<T: Send + 'static>(
    future: impl std::future::Future<Output = Result<T, RagError>> + Send + 'static,
) -> Result<T, RagError> {
    runtime().spawn(future).await.map_err(|e| RagError::Failed(format!("LanceDB-Aufgabe abgebrochen: {e}")))?
}

fn failed(what: &str) -> impl Fn(lancedb::Error) -> RagError + '_ {
    move |e| RagError::Failed(format!("LanceDB ({what}): {e}"))
}

/// Modell und Dimension des Index (`baruch-index.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexMeta {
    pub layout: u32,
    /// Angefragtes Modell (nach Alias), mit dem der Index aufgebaut ist.
    pub requested: Option<String>,
    /// Laut Antwort des Backends.
    pub model: Option<String>,
    pub dimension: Option<usize>,
    /// Letzter vollständiger Abgleich je Bereich (`work:<id>`, `shared`), ISO 8601.
    #[serde(default)]
    pub built: BTreeMap<String, String>,
    /// Aufgaben-Präfix, mit dem die Dokumente eingebettet sind (Runde 19b, z. B. „search_document: “);
    /// `None` (ältere Indexe) gilt wie „kein Präfix“. Ein anderes eingestelltes Präfix mischt der Aufrufer
    /// so wenig wie ein anderes Modell.
    #[serde(default)]
    pub document_prefix: Option<String>,
}

/// Welche Zeilen gemeint sind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Zeilen eines Werks (`work = W`).
    Work(i64),
    /// Zeilen ohne Werk (Einfälle ohne Werk, Bibliothek).
    Shared,
    /// Was eine Suche im Werk sieht: das Werk und alles ohne Werk.
    Visible(i64),
    /// Alle Zeilen von Werken außer diesen (gelöschte Werke).
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

    /// Gehört eine Zeile mit diesem Werk (`None` = ohne Werk) in den Bereich?
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

/// Eine Zeile ohne Vektor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub id: String,
    pub work: Option<i64>,
    pub kind: String,
    pub document: String,
    pub section: Option<String>,
    pub title: String,
    /// Eingebetteter Text.
    pub text: String,
    pub meta: Value,
}

impl Entry {
    /// Hash des eingebetteten Textes.
    pub fn content_hash(&self) -> String {
        chunk::hash(&self.text)
    }

    /// Hash der ganzen Zeile ohne Vektor (ändert sich Text, Ort oder Zusatzangabe, ändert er sich).
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

/// Eine gespeicherte Zeile.
#[derive(Debug, Clone, PartialEq)]
pub struct Stored {
    pub id: String,
    pub row_hash: String,
    pub content_hash: String,
    pub vector: Vec<f32>,
}

/// Ein Treffer der Suche.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub entry: Entry,
    /// Kosinus-Abstand (0 = gleiche Richtung).
    pub distance: f32,
}

impl Found {
    /// Ähnlichkeit 0..=1 (1 − Kosinus-Abstand, geklemmt).
    pub fn score(&self) -> f32 {
        let s = 1.0 - self.distance;
        if s.is_nan() { 0.0 } else { s.clamp(0.0, 1.0) }
    }
}

/// Der Index in einem Ordner.
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
    /// Ordner anlegen (falls nötig) und den Index öffnen.
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

    /// Modell und Dimension aus `baruch-index.json` (fehlt die Datei: leer).
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

    /// `baruch-index.json` schreiben (über eine Zwischendatei, damit nie eine halbe Datei liegt).
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

    /// Namen der Tabellen im Ordner.
    pub async fn tables(&self) -> Result<Vec<String>, RagError> {
        let db = self.db.clone();
        on_runtime(async move { db.table_names().execute().await.map_err(failed("Tabellen")) }).await
    }

    /// Gibt es schon eine Tabelle?
    pub async fn exists(&self) -> Result<bool, RagError> {
        Ok(self.table().await?.is_some())
    }

    /// Zahl der Zeilen im Bereich (`None` = alle).
    pub async fn count(&self, scope: Option<Scope>) -> Result<u64, RagError> {
        let Some(table) = self.table().await? else { return Ok(0) };
        on_runtime(async move {
            let n = table.count_rows(scope.map(|s| s.filter())).await.map_err(failed("zählen"))?;
            Ok(n as u64)
        })
        .await
    }

    /// Gespeicherte Zeilen des Bereichs mit Hashes und Vektoren.
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

    /// Im Bereich `scope` Zeilen entfernen (nach Kennung, **nur** innerhalb des Bereichs: eine gleiche
    /// Kennung in einem anderen Werk bleibt stehen) und neue einfügen. Eine neue Zeile außerhalb des Bereichs
    /// → [`RagError::Invalid`], nichts geändert. `dimension` ist die des Index.
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

    /// Alle Zeilen des Bereichs löschen.
    pub async fn delete_scope(&self, scope: Scope) -> Result<(), RagError> {
        let Some(table) = self.table().await? else { return Ok(()) };
        on_runtime(async move {
            table.delete(scope.filter().as_str()).await.map_err(failed("löschen"))?;
            Ok(())
        })
        .await
    }

    /// Ganzen Index verwerfen: Tabelle und `baruch-index.json`.
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

    /// Die `limit` nächsten Zeilen im Bereich (Kosinus-Abstand), wahlweise nur bestimmter Arten.
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

/// Kosinus-Ähnlichkeit zweier Vektoren (für Tests und Prüfungen).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}
