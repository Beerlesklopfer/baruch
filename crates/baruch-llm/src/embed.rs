// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einbettungen (Runde 12): `POST <basis>/v1/embeddings`, OpenAI-kompatibel wie Eliezer.
//!
//! Rumpf `{model, input: [Text, …]}` (immer eine Liste), Antwort `data[].embedding` mit `data[].index`
//! (die Reihenfolge der Antwort zählt nicht, nur `index`). Geprüft wird: je Eingabe genau ein Vektor,
//! alle gleich lang (≥ 1), nur endliche Zahlen. Alte Namen der Einbettungsmodelle löst
//! [`embedding_alias`] auf (der Aufrufer, damit er den Namen im Index vermerken kann).
//!
//! Grenzen: 1..=[`MAX_INPUTS`] Texte je Anfrage, Verbindungsaufbau wie [`crate::Limits::connect`],
//! gesamt [`EMBED_TIMEOUT`] (Eliezer lädt dabei ggf. das Modell), Antwort höchstens 64 MiB.
//! Weiterleitungen folgt der Client nicht; der Schlüssel steht nur in `Authorization`.

use std::time::Duration;

use serde_json::{Value, json};

use crate::{Endpoint, Limits, LlmError, client, read_limited, status_error, transport_error, whole_secs};

/// Höchstens so viele Texte je Anfrage (wie `baruch_rag::MAX_EMBED_BATCH`).
pub const MAX_INPUTS: usize = 32;
/// Gesamtdauer einer Einbettungsanfrage.
pub const EMBED_TIMEOUT: Duration = Duration::from_secs(120);
/// Größte Antwort (32 Texte × 4096 Dimensionen als JSON passen gut hinein).
pub const MAX_EMBED_RESPONSE: usize = 64 * 1024 * 1024;

/// Alte bzw. kurze Namen von Einbettungsmodellen → Name bei Eliezer.
pub const EMBEDDING_ALIASES: &[(&str, &str)] = &[("nomic-embed-text", "text-embedding-nomic-embed-text-v1.5")];

/// Kurzer Name eines Einbettungsmodells (`nomic-embed-text`, wie bei Ollama und MultiGPT) → Name bei
/// Eliezer; sonst `None`. Bewusst getrennt von [`crate::alias`] (Chat-Modelle der Personas).
pub fn embedding_alias(name: &str) -> Option<&'static str> {
    let name = name.trim();
    EMBEDDING_ALIASES.iter().find(|(old, _)| *old == name).map(|(_, new)| *new)
}

/// Name, wie er an das Backend geht: Alias aufgelöst, sonst beschnitten.
pub fn embedding_model_name(name: &str) -> String {
    embedding_alias(name).map_or_else(|| name.trim().to_owned(), str::to_owned)
}

/// Antwort einer Einbettungsanfrage.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingReply {
    /// Modell laut Antwort (sonst das angefragte).
    pub model: String,
    pub dimension: usize,
    /// Je Eingabe ein Vektor, in der Reihenfolge der Eingaben.
    pub vectors: Vec<Vec<f32>>,
    pub prompt_tokens: Option<u32>,
}

/// Rumpf von `POST /v1/embeddings` (öffentlich für Tests).
pub fn embeddings_body(model: &str, inputs: &[String]) -> Value {
    json!({ "model": model, "input": inputs })
}

/// Einbettungen mit den Grenzen des Betriebs.
pub async fn embeddings(endpoint: &Endpoint, model: &str, inputs: &[String]) -> Result<EmbeddingReply, LlmError> {
    embeddings_with(endpoint, model, inputs, &Limits::default(), EMBED_TIMEOUT).await
}

/// Antwort lesen: `data[]` nach `index` ordnen und prüfen.
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

/// Einbettungen: `POST <basis>/v1/embeddings` mit 1..=[`MAX_INPUTS`] Texten, gesamt höchstens `total`.
/// Ein Modell, das die Gegenstelle nicht kennt, ist [`LlmError::UnknownModel`].
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
            // 404 heißt hier wie beim Chat meist „Modell unbekannt“ (Eliezer kennt den Pfad).
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
