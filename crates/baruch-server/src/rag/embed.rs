// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einbetter des Dienstes (Runde 12): [`EliezerEmbedders`] (Eliezer über `POST /v1/embeddings`, eigene
//! Warteschlange mit **einer** Anfrage zur Zeit, Stapel ≤ [`MAX_EMBED_BATCH`]), [`NoEmbedders`] (kein
//! Backend angebunden: ehrliche Meldung) und [`Cached`] (Zwischenspeicher je Suche: dieselbe Anfrage wird
//! je Modell höchstens einmal eingebettet, auch wenn LanceDB und pgvector gleichzeitig fragen).
//!
//! Adresse und Schlüssel wie [`crate::models`]: aus der Verbindung `eliezer`, der Schlüssel
//! (`baruch/eliezer`) nur, wenn er an das jetzige Ziel gebunden ist; sonst geht nichts hinaus.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use baruch_llm::{Endpoint, LlmError};
use baruch_rag::{BoxFuture, Embedder, EmbedderFactory, Embeddings, MAX_EMBED_BATCH, RagError};
use baruch_store::{ConnectionKind, Store};
use serde_json::Value;
use tokio::sync::{OnceCell, Semaphore};

use crate::connections::SecretStore;

/// Eine Einbettungsanfrage zur Zeit bei Eliezer, für den ganzen Prozess (getrennt von der
/// Warteschlange der Chat-Aufrufe in [`crate::models`]).
static EMBED_QUEUE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(1));

/// Satz, wenn kein Einbetter angebunden ist.
pub const NO_EMBEDDER: &str = "Kein Einbettungsmodell angebunden (Baruch läuft ohne Einbettungen, z. B. in Tests oder \
                               Hilfsbefehlen); die Wortlaut-Suche (FTS) geht weiter.";

/// Prüft die Zahl der Texte einer Anfrage.
fn check_batch(texts: &[String]) -> Result<(), RagError> {
    if texts.is_empty() || texts.len() > MAX_EMBED_BATCH {
        return Err(RagError::Invalid(format!(
            "Einbettung: 1 bis {MAX_EMBED_BATCH} Texte je Anfrage, nicht {}.",
            texts.len()
        )));
    }
    Ok(())
}

/// [`LlmError`] → [`RagError`]: Nicht eingerichtet, nicht erreichbar oder Modell unbekannt heißt „kein
/// Einbettungsmodell“ (die Wortlaut-Suche bleibt).
pub fn rag_error(e: LlmError) -> RagError {
    match e {
        LlmError::NotConfigured(_) | LlmError::Unreachable(_) | LlmError::UnknownModel(_) => RagError::NoEmbedder(
            format!("Kein Einbettungsmodell erreichbar: {e}. Die Wortlaut-Suche (FTS) geht weiter."),
        ),
        LlmError::Denied(_) => RagError::Denied(e.to_string()),
        LlmError::Timeout(_) => RagError::Timeout(e.to_string()),
        LlmError::Busy(_) | LlmError::Cancelled | LlmError::Failed(_) => RagError::Failed(e.to_string()),
    }
}

/// Satz, wenn die Verbindung Eliezer als extern markiert ist: Einbetten schickt Buchtext ohne Persona, also ohne
/// Freigabe – deshalb gar nicht.
pub const EXTERNAL_REFUSED: &str = "Die Verbindung Eliezer ist als extern markiert; zum Einbetten geht kein Buchtext \
hinaus (Einbetten hat keine Persona und damit keine Freigabe). Nichts gesendet. Für die Bedeutungssuche Eliezer als \
lokal führen (Einstellungen → Verbindungen) oder RAG ausschalten";

/// Adresse und Schlüssel aus der Verbindung `eliezer` (Muster `models::endpoint`). Ist Eliezer als extern
/// markiert, geht nichts hinaus ([`EXTERNAL_REFUSED`]) – dieselbe Sperre wie bei Chat und Lektor ohne Freigabe.
async fn endpoint(store: &Store, secrets: &Arc<dyn SecretStore>) -> Result<Endpoint, LlmError> {
    let model_connection = store.model_connection_get(baruch_store::ELIEZER_CONNECTION).await.map_err(|e| {
        LlmError::NotConfigured(format!("Modell-Verbindung Eliezer nicht lesbar: {e}; nichts gesendet"))
    })?;
    if model_connection.external {
        return Err(LlmError::NotConfigured(EXTERNAL_REFUSED.into()));
    }
    let row = store
        .connection_get(ConnectionKind::Eliezer)
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Verbindung eliezer nicht lesbar: {e}")))?;
    let url = row.config.get("url").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    if url.is_empty() {
        return Err(LlmError::NotConfigured(
            "Keine Adresse für Eliezer eingetragen (Einstellungen → Verbindungen)".into(),
        ));
    }
    let secret = crate::connections::secrets::get(secrets, "eliezer")
        .await
        .map_err(|e| LlmError::NotConfigured(format!("Schlüsselbund nicht lesbar ({e}); nichts gesendet")))?;
    if secret.is_some() && !crate::connections::secret_bound(&row) {
        return Err(LlmError::NotConfigured(
            "Schlüssel gilt für ein anderes Ziel; nicht gesendet (Einstellungen → Verbindungen)".into(),
        ));
    }
    Ok(Endpoint::new(url, secret))
}

/// Einbetter über Eliezer.
#[derive(Clone)]
pub struct EliezerEmbedders {
    store: Store,
    secrets: Arc<dyn SecretStore>,
}

impl EliezerEmbedders {
    pub fn new(store: Store, secrets: Arc<dyn SecretStore>) -> EliezerEmbedders {
        EliezerEmbedders { store, secrets }
    }
}

impl EmbedderFactory for EliezerEmbedders {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
        Arc::new(EliezerEmbedder {
            store: self.store.clone(),
            secrets: self.secrets.clone(),
            model: baruch_llm::embedding_model_name(model),
        })
    }
}

struct EliezerEmbedder {
    store: Store,
    secrets: Arc<dyn SecretStore>,
    /// Name bei Eliezer (Alias aufgelöst).
    model: String,
}

impl Embedder for EliezerEmbedder {
    fn model(&self) -> &str {
        &self.model
    }

    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async move {
            check_batch(texts)?;
            let endpoint = endpoint(&self.store, &self.secrets).await.map_err(rag_error)?;
            let _turn =
                EMBED_QUEUE.acquire().await.map_err(|_| RagError::Failed("Warteschlange geschlossen.".into()))?;
            let reply = baruch_llm::embeddings(&endpoint, &self.model, texts).await.map_err(rag_error)?;
            Ok(Embeddings { model: reply.model, dimension: reply.dimension, vectors: reply.vectors })
        })
    }
}

/// Kein Einbetter angebunden: jede Anfrage meldet [`NO_EMBEDDER`].
pub struct NoEmbedders;

struct NoEmbedder(String);

impl EmbedderFactory for NoEmbedders {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
        Arc::new(NoEmbedder(baruch_llm::embedding_model_name(model)))
    }
}

impl Embedder for NoEmbedder {
    fn model(&self) -> &str {
        &self.0
    }

    fn embed<'a>(&'a self, _texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async { Err(RagError::NoEmbedder(NO_EMBEDDER.to_owned())) })
    }
}

type Cell = Arc<OnceCell<Result<Embeddings, RagError>>>;
/// Zellen je (Modell, Texte).
type Cells = Arc<Mutex<HashMap<(String, Vec<String>), Cell>>>;

/// Zwischenspeicher für die Dauer einer Suche: gleiche Texte beim gleichen Modell → ein Aufruf.
pub struct Cached {
    inner: Arc<dyn EmbedderFactory>,
    cells: Cells,
}

impl Cached {
    pub fn new(inner: Arc<dyn EmbedderFactory>) -> Cached {
        Cached { inner, cells: Arc::default() }
    }
}

impl EmbedderFactory for Cached {
    fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
        Arc::new(CachedEmbedder { inner: self.inner.for_model(model), cells: self.cells.clone() })
    }
}

struct CachedEmbedder {
    inner: Arc<dyn Embedder>,
    cells: Cells,
}

impl Embedder for CachedEmbedder {
    fn model(&self) -> &str {
        self.inner.model()
    }

    fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
        Box::pin(async move {
            // Schlüssel nach dem Alias: `nomic-embed-text` und sein voller Name sind dasselbe Modell, auch wenn
            // ein Einbetter den Namen nicht selbst auflöst.
            let key = (baruch_llm::embedding_model_name(self.inner.model()), texts.to_vec());
            let cell = self.cells.lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_default().clone();
            cell.get_or_init(|| self.inner.embed(texts)).await.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct Counting(Arc<AtomicUsize>, String);

    impl Embedder for Counting {
        fn model(&self) -> &str {
            &self.1
        }
        fn embed<'a>(&'a self, texts: &'a [String]) -> BoxFuture<'a, Result<Embeddings, RagError>> {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                Ok(Embeddings {
                    model: self.1.clone(),
                    dimension: 1,
                    vectors: texts.iter().map(|_| vec![1.0]).collect(),
                })
            })
        }
    }

    struct Factory(Arc<AtomicUsize>);

    impl EmbedderFactory for Factory {
        fn for_model(&self, model: &str) -> Arc<dyn Embedder> {
            Arc::new(Counting(self.0.clone(), baruch_llm::embedding_model_name(model)))
        }
    }

    #[tokio::test]
    async fn the_same_query_is_embedded_once_per_model_even_when_asked_at_the_same_time() {
        let calls = Arc::new(AtomicUsize::new(0));
        let cached = Cached::new(Arc::new(Factory(calls.clone())));
        let query = vec!["Leuchtturm im Nebel".to_owned()];
        let (a, b) = (cached.for_model("nomic-embed-text"), cached.for_model("text-embedding-nomic-embed-text-v1.5"));
        let (ra, rb) = tokio::join!(a.embed(&query), b.embed(&query));
        assert_eq!(ra, rb, "beide bekommen dasselbe Ergebnis");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "Alias und Name sind dasselbe Modell: ein Aufruf");
        cached.for_model("anderes-modell").embed(&query).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "anderes Modell: eigener Aufruf");
    }

    #[test]
    fn unreachable_or_unconfigured_backends_mean_no_embedder() {
        assert_eq!(rag_error(LlmError::Unreachable("x".into())).code(), "no_embedder", "nicht erreichbar");
        assert_eq!(rag_error(LlmError::NotConfigured("x".into())).code(), "no_embedder", "keine Adresse");
        assert_eq!(rag_error(LlmError::UnknownModel("x".into())).code(), "no_embedder", "Modell fehlt bei Eliezer");
        assert_eq!(rag_error(LlmError::Timeout("x".into())).code(), "timeout", "Zeitlimit bleibt Zeitlimit");
        assert_eq!(rag_error(LlmError::Denied("x".into())).code(), "denied", "Zugang verweigert");
        assert!(check_batch(&[]).is_err() && check_batch(&vec![String::new(); 33]).is_err(), "1..=32 Texte");
    }
}
