// SPDX-License-Identifier: AGPL-3.0-or-later
//! Embedders of the service (Round 12): [`EliezerEmbedders`] (Eliezer through `POST /v1/embeddings`, its own
//! queue with **one** request at a time, batch ≤ [`MAX_EMBED_BATCH`]), [`NoEmbedders`] (no
//! backend connected: honest message) and [`Cached`] (cache per search: the same query is
//! embedded at most once per model, even if LanceDB and pgvector ask at the same time).
//!
//! Address and key as in [`crate::models`]: from the connection `eliezer`, the key
//! (`baruch/eliezer`) only if it is bound to the current target; otherwise nothing goes out.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use baruch_llm::{Endpoint, LlmError};
use baruch_rag::{BoxFuture, Embedder, EmbedderFactory, Embeddings, MAX_EMBED_BATCH, RagError};
use baruch_store::{ConnectionKind, Store};
use serde_json::Value;
use tokio::sync::{OnceCell, Semaphore};

use crate::connections::SecretStore;
use crate::i18n::tc;

/// One embedding request at a time at Eliezer, for the whole process (separate from the
/// queue of the chat calls in [`crate::models`]).
static EMBED_QUEUE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(1));

/// Sentence shown when no embedder is connected (interface language, Round 39c).
pub fn no_embedder() -> String {
    tc("rag_msg.no_embedder", &[])
}

/// Checks the number of texts of a request.
fn check_batch(texts: &[String]) -> Result<(), RagError> {
    if texts.is_empty() || texts.len() > MAX_EMBED_BATCH {
        return Err(RagError::Invalid(tc(
            "rag_msg.embed.batch",
            &[("max", MAX_EMBED_BATCH.to_string().into()), ("count", texts.len().to_string().into())],
        )));
    }
    Ok(())
}

/// [`LlmError`] → [`RagError`]: not set up, not reachable or model unknown means "no
/// embedding model" (the wording search remains).
pub fn rag_error(e: LlmError) -> RagError {
    match e {
        LlmError::NotConfigured(_) | LlmError::Unreachable(_) | LlmError::UnknownModel(_) => {
            RagError::NoEmbedder(tc("rag_msg.embed.unreachable", &[("detail", e.to_string().into())]))
        }
        LlmError::Denied(_) => RagError::Denied(e.to_string()),
        LlmError::Timeout(_) => RagError::Timeout(e.to_string()),
        LlmError::Busy(_) | LlmError::Cancelled | LlmError::Failed(_) => RagError::Failed(e.to_string()),
    }
}

/// Sentence shown when the connection Eliezer is marked as external: embedding sends book text without a persona,
/// that is without a release – therefore not at all.
pub fn external_refused() -> String {
    tc("rag_msg.embed.external", &[])
}

/// Address and key from the connection `eliezer` (pattern `models::endpoint`). If Eliezer is marked as
/// external, nothing goes out ([`external_refused`]) – the same lock as for chat and lector without a release.
async fn endpoint(store: &Store, secrets: &Arc<dyn SecretStore>) -> Result<Endpoint, LlmError> {
    let model_connection = store.model_connection_get(baruch_store::ELIEZER_CONNECTION).await.map_err(|e| {
        LlmError::NotConfigured(tc("rag_msg.embed.model_connection", &[("detail", e.to_string().into())]))
    })?;
    if model_connection.external {
        return Err(LlmError::NotConfigured(external_refused()));
    }
    let row = store
        .connection_get(ConnectionKind::Eliezer)
        .await
        .map_err(|e| LlmError::NotConfigured(tc("rag_msg.embed.connection", &[("detail", e.to_string().into())])))?;
    let url = row.config.get("url").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    if url.is_empty() {
        return Err(LlmError::NotConfigured(tc("rag_msg.embed.no_address", &[])));
    }
    let secret = crate::connections::secrets::get(secrets, "eliezer")
        .await
        .map_err(|e| LlmError::NotConfigured(tc("rag_msg.embed.keyring", &[("detail", e.to_string().into())])))?;
    if secret.is_some() && !crate::connections::secret_bound(&row) {
        return Err(LlmError::NotConfigured(tc("rag_msg.embed.foreign_key", &[])));
    }
    Ok(Endpoint::new(url, secret))
}

/// Embedder through Eliezer.
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
    /// Name at Eliezer (alias resolved).
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
                EMBED_QUEUE.acquire().await.map_err(|_| RagError::Failed(tc("rag_msg.embed.queue_closed", &[])))?;
            let reply = baruch_llm::embeddings(&endpoint, &self.model, texts).await.map_err(rag_error)?;
            Ok(Embeddings { model: reply.model, dimension: reply.dimension, vectors: reply.vectors })
        })
    }
}

/// No embedder connected: every request reports [`no_embedder`].
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
        Box::pin(async { Err(RagError::NoEmbedder(no_embedder())) })
    }
}

type Cell = Arc<OnceCell<Result<Embeddings, RagError>>>;
/// Cells per (model, texts).
type Cells = Arc<Mutex<HashMap<(String, Vec<String>), Cell>>>;

/// Cache for the duration of one search: same texts with the same model → one call.
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
            // Key by the alias: `nomic-embed-text` and its full name are the same model, even if
            // an embedder does not resolve the name itself.
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
