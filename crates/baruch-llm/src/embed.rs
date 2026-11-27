// SPDX-License-Identifier: AGPL-3.0-or-later
//! Embeddings (Round 12): `POST <base>/v1/embeddings`, OpenAI-compatible like Eliezer.
//!
//! Body `{model, input: [text, …]}` (always a list), answer `data[].embedding` with `data[].index` (the order of
//! the answer does not count, only `index`). Checked: exactly one vector per input, all of the same length (≥ 1),
//! only finite numbers. Old names of the embedding models are resolved by [`embedding_alias`] (by the caller, so
//! that it can note the name in the index).
//!
//! Limits: 1..=[`MAX_INPUTS`] texts per request, connect timeout like [`crate::Limits::connect`], in total
//! [`EMBED_TIMEOUT`] (Eliezer may load the model during that), answer at most 64 MiB. The client follows no
//! redirects; the key stands only in `Authorization`.

use std::time::Duration;

use serde_json::{Value, json};

use crate::{Endpoint, Limits, LlmError, client, read_limited, status_error, transport_error, whole_secs};

/// At most this many texts per request (like `baruch_rag::MAX_EMBED_BATCH`).
pub const MAX_INPUTS: usize = 32;
/// Total duration of an embedding request.
pub const EMBED_TIMEOUT: Duration = Duration::from_secs(120);
/// Largest answer (32 texts × 4096 dimensions as JSON fit in well).
pub const MAX_EMBED_RESPONSE: usize = 64 * 1024 * 1024;

/// Old or short names of embedding models → name at Eliezer.
pub const EMBEDDING_ALIASES: &[(&str, &str)] = &[("nomic-embed-text", "text-embedding-nomic-embed-text-v1.5")];

/// Short name of an embedding model (`nomic-embed-text`, as with Ollama and MultiGPT) → name at Eliezer; otherwise
/// `None`. Deliberately separate from [`crate::alias`] (chat models of the personas).
pub fn embedding_alias(name: &str) -> Option<&'static str> {
    let name = name.trim();
    EMBEDDING_ALIASES.iter().find(|(old, _)| *old == name).map(|(_, new)| *new)
}

/// Name as it goes to the backend: alias resolved, otherwise trimmed.
pub fn embedding_model_name(name: &str) -> String {
    embedding_alias(name).map_or_else(|| name.trim().to_owned(), str::to_owned)
}

/// Answer of an embedding request.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingReply {
    /// Model according to the answer (otherwise the requested one).
    pub model: String,
    pub dimension: usize,
    /// One vector per input, in the order of the inputs.
    pub vectors: Vec<Vec<f32>>,
    pub prompt_tokens: Option<u32>,
}

/// Body of `POST /v1/embeddings` (public for tests).
pub fn embeddings_body(model: &str, inputs: &[String]) -> Value {
    json!({ "model": model, "input": inputs })
}

/// Embeddings with the limits of production.
pub async fn embeddings(endpoint: &Endpoint, model: &str, inputs: &[String]) -> Result<EmbeddingReply, LlmError> {
    embeddings_with(endpoint, model, inputs, &Limits::default(), EMBED_TIMEOUT).await
}

/// Read the answer: order `data[]` by `index` and check.
pub fn parse_embeddings(body: &Value, requested: &str, count: usize, url: &str) -> Result<EmbeddingReply, LlmError> {
    let data = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| LlmError::Failed(format!("{url} liefert keine Einbettungen (Feld data fehlt).")))?;
    if data.len() != count {
        return Err(LlmError::Failed(format!("{url} liefert {} statt {count} Vektoren.", data.len())));
    }
    let mut slots: Vec<Option<Vec<f32>>> = vec![None; count];
    for (position, item) in data.iter().enumerate() {
        let index = match item.get("index") {
            None | Some(Value::Null) => position,
            Some(v) => v
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .ok_or_else(|| LlmError::Failed(format!("{url}: index ist keine Zahl.")))?,
        };
        let Some(slot) = slots.get_mut(index) else {
            return Err(LlmError::Failed(format!("{url}: index {index} außerhalb der Eingaben.")));
        };
        if slot.is_some() {
            return Err(LlmError::Failed(format!("{url}: index {index} doppelt.")));
        }
        let numbers = item
            .get("embedding")
            .and_then(Value::as_array)
            .ok_or_else(|| LlmError::Failed(format!("{url}: embedding fehlt oder ist keine Liste.")))?;
        let mut vector = Vec::with_capacity(numbers.len());
        for n in numbers {
            let x = n.as_f64().ok_or_else(|| LlmError::Failed(format!("{url}: embedding enthält keine Zahl.")))?;
            let x = x as f32;
            if !x.is_finite() {
                return Err(LlmError::Failed(format!("{url}: embedding enthält eine unendliche Zahl.")));
            }
            vector.push(x);
        }
        *slot = Some(vector);
    }
    let vectors: Vec<Vec<f32>> = slots.into_iter().map(|v| v.unwrap_or_default()).collect();
    let dimension = vectors.first().map_or(0, Vec::len);
    if dimension == 0 || vectors.iter().any(|v| v.len() != dimension) {
        return Err(LlmError::Failed(format!("{url}: Vektoren leer oder ungleich lang.")));
    }
    let model = body.get("model").and_then(Value::as_str).filter(|m| !m.is_empty()).unwrap_or(requested).to_owned();
    let prompt_tokens =
        body.pointer("/usage/prompt_tokens").and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok());
    Ok(EmbeddingReply { model, dimension, vectors, prompt_tokens })
}

/// Embeddings: `POST <base>/v1/embeddings` with 1..=[`MAX_INPUTS`] texts, in total at most `total`. A model the
/// remote side does not know is [`LlmError::UnknownModel`].
pub async fn embeddings_with(
    endpoint: &Endpoint,
    model: &str,
    inputs: &[String],
    limits: &Limits,
    total: Duration,
) -> Result<EmbeddingReply, LlmError> {
    if inputs.is_empty() || inputs.len() > MAX_INPUTS {
        return Err(LlmError::Failed(format!(
            "Einbettung: 1 bis {MAX_INPUTS} Texte je Anfrage, nicht {}.",
            inputs.len()
        )));
    }
    let model = model.trim();
    if model.is_empty() {
        return Err(LlmError::NotConfigured("Kein Einbettungsmodell eingestellt.".into()));
    }
    let url = endpoint.embeddings_url();
    let body = embeddings_body(model, inputs);
    let attempt = async {
        let client = client(limits)?;
        let mut call = client
            .post(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string());
        if let Some(secret) = &endpoint.secret {
            call = call.bearer_auth(secret);
        }
        let response = call.send().await.map_err(|e| transport_error(&e, &url, endpoint, limits))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = read_limited(response, 64 * 1024).await;
            // 404 here mostly means "model unknown", as with the chat (Eliezer knows the path).
            return Err(status_error(status, &body, &url, endpoint, true));
        }
        let raw = read_limited(response, MAX_EMBED_RESPONSE + 1).await;
        if raw.len() > MAX_EMBED_RESPONSE {
            return Err(LlmError::Failed(format!("Antwort von {url} ist größer als 64 MiB; abgebrochen.")));
        }
        let parsed: Value =
            serde_json::from_slice(&raw).map_err(|_| LlmError::Failed(format!("{url} liefert kein JSON.")))?;
        parse_embeddings(&parsed, model, inputs.len(), &url)
    };
    tokio::time::timeout(total, attempt).await.unwrap_or_else(|_| {
        Err(LlmError::Timeout(format!("Keine Einbettungen von {url} innerhalb von {}.", whole_secs(total))))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_short_nomic_name_resolves_to_the_name_at_eliezer() {
        assert_eq!(
            embedding_alias(" nomic-embed-text "),
            Some("text-embedding-nomic-embed-text-v1.5"),
            "Ollama-/MultiGPT-Name → Eliezer"
        );
        assert_eq!(embedding_alias("text-embedding-nomic-embed-text-v1.5"), None, "Eliezer-Name bleibt");
        assert_eq!(embedding_model_name("bge-m3 "), "bge-m3", "unbekannte Namen nur beschnitten");
        assert_eq!(crate::alias("nomic-embed-text"), None, "Chat-Aliasse bleiben davon unberührt");
    }

    #[test]
    fn the_answer_is_ordered_by_index_and_checked() {
        let body = json!({ "model": "m", "data": [
            { "index": 1, "embedding": [0.0, 1.0] },
            { "index": 0, "embedding": [1.0, 0.0] },
        ], "usage": { "prompt_tokens": 4 } });
        let reply = parse_embeddings(&body, "x", 2, "u").unwrap();
        assert_eq!(reply.vectors, vec![vec![1.0, 0.0], vec![0.0, 1.0]], "nach index geordnet, nicht nach Lage");
        assert_eq!((reply.model.as_str(), reply.dimension, reply.prompt_tokens), ("m", 2, Some(4)), "Angaben");
        let short = json!({ "data": [{ "index": 0, "embedding": [1.0] }] });
        assert!(parse_embeddings(&short, "x", 2, "u").is_err(), "zu wenige Vektoren sind ein Fehler");
        let uneven = json!({ "data": [{ "embedding": [1.0] }, { "embedding": [1.0, 2.0] }] });
        assert!(parse_embeddings(&uneven, "x", 2, "u").is_err(), "ungleich lange Vektoren sind ein Fehler");
        let twice = json!({ "data": [{ "index": 0, "embedding": [1.0] }, { "index": 0, "embedding": [1.0] }] });
        assert!(parse_embeddings(&twice, "x", 2, "u").is_err(), "doppelter index ist ein Fehler");
        let empty = json!({ "data": [{ "embedding": [] }] });
        assert!(parse_embeddings(&empty, "x", 1, "u").is_err(), "leerer Vektor ist ein Fehler");
        let reply = parse_embeddings(&json!({ "data": [{ "embedding": [0.5] }] }), "angefragt", 1, "u").unwrap();
        assert_eq!(reply.model, "angefragt", "ohne model in der Antwort gilt das angefragte");
    }
}
