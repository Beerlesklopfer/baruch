// SPDX-License-Identifier: AGPL-3.0-or-later
//! One client per endpoint (Round 15b, contract §3): [`Client`] holds kind ([`BackendKind`]), address and key
//! ([`crate::Endpoint`]), limits ([`crate::Limits`]) and **one** `reqwest::Client` that all calls of this endpoint
//! share (connections stay open in the pool of reqwest). Cloning shares it.
//!
//! All three kinds speak the same OpenAI-like interface for model list and chat (`GET /v1/models`, streamed `POST
//! /v1/chat/completions`). The only difference is whether and how the remote side **reports a context length**
//! ([`Client::context_length`]):
//!
//! - **Eliezer:** `GET /api/v1/models` (management list in the form of LM Studio 0.4, according to
//!   `docs/wiki/API.md` of Eliezer): per model `key`, `max_context_length` and for a loaded model
//!   `loaded_instances[].config.context_length`. Reported is the length of the loaded instance, otherwise
//!   `max_context_length`. Read-only: Baruch never calls `/api/v1/models/load|unload|estimate`.
//! - **Ollama:** reported is the length with which Ollama **actually** runs the model, not the maximum of the
//!   model: the chat runs through `/v1/chat/completions`, and there Baruch sets no `num_ctx` – Ollama then takes
//!   `num_ctx` from the Modelfile, otherwise its own default, and **silently cuts off** longer inputs **at the
//!   front**. Therefore in this order: `num_ctx` from `parameters` of `POST /api/show {"model": …}`; otherwise
//!   `context_length` of the loaded model from `GET /api/ps` (assumption: newer Ollama versions report it there;
//!   older ones do not, or have no `/api/ps` at all); otherwise the default of Ollama [`OLLAMA_DEFAULT_CONTEXT`]
//!   (assumption after the Ollama documentation; Baruch sees a larger default through `OLLAMA_CONTEXT_LENGTH` only
//!   once the model is loaded), but at most `<arch>.context_length` from `model_info` (maximum of the model).
//!   Rather too small than silently truncated.
//! - **OpenAI-compatible:** no report (`None`); the setting of the persona applies.
//!
//! Baruch never requests a context length. The client follows no redirects, the key stands only in `Authorization`,
//! every message runs through [`crate::scrub`]. `Debug` never shows the key.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ChatReply, ChatRequest, Endpoint, Limits, LlmError};

/// Kind of the remote side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    Eliezer,
    Openai,
    Ollama,
}

impl BackendKind {
    /// All kinds in the order of the settings page.
    pub const ALL: [BackendKind; 3] = [BackendKind::Eliezer, BackendKind::Openai, BackendKind::Ollama];

    /// `eliezer`, `openai`, `ollama`.
    pub fn as_str(self) -> &'static str {
        match self {
            BackendKind::Eliezer => "eliezer",
            BackendKind::Openai => "openai",
            BackendKind::Ollama => "ollama",
        }
    }

    /// From `eliezer`, `openai`, `ollama` (case and surrounding whitespace do not matter); otherwise `None`.
    pub fn parse(s: &str) -> Option<BackendKind> {
        let s = s.trim().to_ascii_lowercase();
        BackendKind::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// A model of the remote side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelInfo {
    pub id: String,
    /// reported context length in tokens, None = reports none
    pub context_length: Option<u32>,
}

/// Client of an endpoint (see module header).
#[derive(Clone)]
pub struct Client {
    kind: BackendKind,
    endpoint: Endpoint,
    limits: Limits,
    http: reqwest::Client,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("kind", &self.kind)
            .field("endpoint", &self.endpoint)
            .field("limits", &self.limits)
            .finish()
    }
}

fn number(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|&n| n > 0),
        Value::String(s) => s.trim().parse::<u32>().ok().filter(|&n| n > 0),
        _ => None,
    }
}

/// Context length of a model from the management list of Eliezer (`GET /api/v1/models`).
pub fn eliezer_context(list: &Value, model: &str) -> Option<u32> {
    let entry = list.get("models")?.as_array()?.iter().find(|m| m.get("key").and_then(Value::as_str) == Some(model))?;
    let loaded = entry
        .get("loaded_instances")
        .and_then(Value::as_array)
        .and_then(|l| l.iter().find_map(|i| i.pointer("/config/context_length").and_then(number)));
    loaded.or_else(|| entry.get("max_context_length").and_then(number))
}

/// Default context length of Ollama if neither the Modelfile (`num_ctx`) nor the loaded model say anything else
/// (assumption after the Ollama documentation, "default context window 4096 tokens").
pub const OLLAMA_DEFAULT_CONTEXT: u32 = 4096;

/// `num_ctx` from `parameters` of the answer of Ollama `POST /api/show` (fixed by the Modelfile, effective also
/// through `/v1/chat/completions`).
pub fn ollama_num_ctx(show: &Value) -> Option<u32> {
    show.get("parameters")
        .and_then(Value::as_str)
        .and_then(|p| {
            p.lines().find_map(|line| {
                let mut parts = line.split_whitespace();
                (parts.next() == Some("num_ctx")).then(|| parts.next().and_then(|n| n.parse::<u32>().ok())).flatten()
            })
        })
        .filter(|&n| n > 0)
}

/// Maximum of the model: `<arch>.context_length` from `model_info` of `POST /api/show` (upper limit only, never the
/// effective length).
pub fn ollama_model_max(show: &Value) -> Option<u32> {
    show.get("model_info")?
        .as_object()?
        .iter()
        .find(|(k, _)| k.ends_with(".context_length"))
        .and_then(|(_, v)| number(v))
}

/// Length with which a loaded model runs: `context_length` of its entry in `GET /api/ps` (field `name` or `model`
/// equal to `model`; assumption, see module header).
pub fn ollama_loaded_context(ps: &Value, model: &str) -> Option<u32> {
    ps.get("models")?
        .as_array()?
        .iter()
        .filter(|m| [m.get("name"), m.get("model")].into_iter().flatten().any(|v| v.as_str() == Some(model)))
        .find_map(|m| m.get("context_length").and_then(number))
}

/// Effective context length with Ollama (see module header): `num_ctx`, otherwise that of the loaded model,
/// otherwise [`OLLAMA_DEFAULT_CONTEXT`] – never more than the maximum of the model.
pub fn ollama_context(show: &Value, ps: Option<&Value>, model: &str) -> u32 {
    let effective = ollama_num_ctx(show)
        .or_else(|| ps.and_then(|ps| ollama_loaded_context(ps, model)))
        .unwrap_or(OLLAMA_DEFAULT_CONTEXT);
    ollama_model_max(show).map_or(effective, |max| effective.min(max))
}

impl Client {
    /// New client with its own `reqwest::Client` (without redirects, connect timeout from `limits`).
    pub fn new(kind: BackendKind, endpoint: Endpoint, limits: Limits) -> Result<Client, LlmError> {
        let http = crate::client(&limits)?;
        Ok(Client { kind, endpoint, limits, http })
    }

    pub fn kind(&self) -> BackendKind {
        self.kind
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    /// Limits of this client.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Address outside of `/v1` (e.g. `/api/show`): base without a trailing `/v1`.
    pub fn root_url(&self, path: &str) -> String {
        let base = self.endpoint.base_url.trim().trim_end_matches('/');
        let base = base.strip_suffix("/v1").unwrap_or(base);
        format!("{base}{path}")
    }

    /// `GET /v1/models` and, where the remote side reports it in a list (Eliezer), the context length per model. If
    /// only the management list fails, the context length stays empty.
    pub async fn models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        let ids = crate::models_using(&self.http, &self.endpoint, &self.limits).await?;
        let contexts = match self.kind {
            BackendKind::Eliezer => self.eliezer_list().await.ok(),
            BackendKind::Openai | BackendKind::Ollama => None,
        };
        Ok(ids
            .into_iter()
            .map(|id| {
                let context_length = contexts.as_ref().and_then(|list| eliezer_context(list, &id));
                ModelInfo { id, context_length }
            })
            .collect())
    }

    /// Reported context length of a model (see module header); OpenAI-compatible always `None`.
    pub async fn context_length(&self, model: &str) -> Result<Option<u32>, LlmError> {
        match self.kind {
            BackendKind::Openai => Ok(None),
            BackendKind::Eliezer => Ok(eliezer_context(&self.eliezer_list().await?, model)),
            BackendKind::Ollama => {
                let show = self.json_call("/api/show", Some(json!({ "model": model }))).await?;
                // Only ask `/api/ps` if the Modelfile fixes nothing; if it is missing (older version), the default
                // applies.
                let ps =
                    if ollama_num_ctx(&show).is_none() { self.json_call("/api/ps", None).await.ok() } else { None };
                Ok(Some(ollama_context(&show, ps.as_ref(), model)))
            }
        }
    }

    /// Streamed chat through the shared HTTP client.
    pub async fn chat_stream(
        &self,
        request: &ChatRequest,
        on_delta: impl FnMut(&str) + Send,
    ) -> Result<ChatReply, LlmError> {
        crate::chat_stream_using(&self.http, &self.endpoint, request, &self.limits, on_delta).await
    }

    async fn eliezer_list(&self) -> Result<Value, LlmError> {
        self.json_call("/api/v1/models", None).await
    }

    /// `GET` (without body) or `POST` (with body) to `path` outside of `/v1`, answer as JSON, in total at most
    /// `limits.models_total`.
    async fn json_call(&self, path: &str, body: Option<Value>) -> Result<Value, LlmError> {
        let url = self.root_url(path);
        let endpoint = &self.endpoint;
        let limits = &self.limits;
        let attempt = async {
            let mut request = match &body {
                Some(body) => self
                    .http
                    .post(&url)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(body.to_string()),
                None => self.http.get(&url),
            }
            .header(reqwest::header::ACCEPT, "application/json");
            if let Some(secret) = &endpoint.secret {
                request = request.bearer_auth(secret);
            }
            let response = request.send().await.map_err(|e| crate::transport_error(&e, &url, endpoint, limits))?;
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                let body = crate::read_limited(response, 64 * 1024).await;
                return Err(crate::status_error(status, &body, &url, endpoint, false));
            }
            let body = crate::read_limited(response, limits.max_text + 1).await;
            if body.len() > limits.max_text {
                return Err(LlmError::Failed(format!("Antwort von {url} ist größer als 1 MiB; abgebrochen.")));
            }
            serde_json::from_slice::<Value>(&body).map_err(|_| LlmError::Failed(format!("{url} liefert kein JSON.")))
        };
        tokio::time::timeout(limits.models_total, attempt).await.unwrap_or_else(|_| {
            Err(LlmError::Timeout(format!(
                "Keine Antwort von {url} innerhalb von {}.",
                crate::whole_secs(limits.models_total)
            )))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_through_their_names() {
        for kind in BackendKind::ALL {
            assert_eq!(BackendKind::parse(kind.as_str()), Some(kind), "Name {} zurück zur Art", kind.as_str());
        }
        assert_eq!(BackendKind::parse(" Ollama "), Some(BackendKind::Ollama), "Rand und Großschreibung egal");
        assert_eq!(BackendKind::parse("anthropic"), None, "unbekannte Art");
    }

    #[test]
    fn eliezer_reports_the_loaded_context_before_the_maximum() {
        let list = json!({ "models": [
            { "key": "a", "max_context_length": 131072, "loaded_instances": [{ "id": "a", "config": { "context_length": 32768 } }] },
            { "key": "b", "max_context_length": 65536, "loaded_instances": [] },
            { "key": "c", "loaded_instances": [] },
        ]});
        assert_eq!(eliezer_context(&list, "a"), Some(32768), "geladene Instanz zählt");
        assert_eq!(eliezer_context(&list, "b"), Some(65536), "nicht geladen: Höchstwert der Einstellung");
        assert_eq!(eliezer_context(&list, "c"), None, "ohne Angabe keine Meldung");
        assert_eq!(eliezer_context(&list, "x"), None, "unbekanntes Modell");
    }

    #[test]
    fn ollama_reports_the_length_it_runs_with_never_the_model_maximum() {
        let show =
            json!({ "parameters": "stop \"<|x|>\"\nnum_ctx 8192", "model_info": { "llama.context_length": 131072 } });
        assert_eq!(ollama_context(&show, None, "m"), 8192, "num_ctx aus dem Modelfile ist wirksam, auch über /v1");
        let show = json!({ "model_info": { "general.architecture": "qwen3", "qwen3.context_length": 40960 } });
        assert_eq!(
            ollama_context(&show, None, "m"),
            OLLAMA_DEFAULT_CONTEXT,
            "ohne num_ctx läuft Ollama mit seiner Vorgabe, nicht mit dem Höchstwert 40960 (sonst still vorn gekürzt)"
        );
        let ps = json!({ "models": [
            { "name": "andere:1b", "model": "andere:1b", "context_length": 2048 },
            { "name": "m", "model": "m", "context_length": 16384 } ] });
        assert_eq!(ollama_context(&show, Some(&ps), "m"), 16384, "geladen: die Länge aus /api/ps, je Modell");
        assert_eq!(ollama_context(&show, Some(&json!({ "models": [] })), "m"), OLLAMA_DEFAULT_CONTEXT, "nicht geladen");
        let tiny = json!({ "model_info": { "llama.context_length": 2048 } });
        assert_eq!(ollama_context(&tiny, None, "m"), 2048, "nie mehr als der Höchstwert des Modells");
        assert_eq!(ollama_context(&json!({}), None, "m"), OLLAMA_DEFAULT_CONTEXT, "keine Angabe: die Vorgabe");
    }

    #[test]
    fn the_root_address_drops_a_trailing_v1() {
        let client =
            Client::new(BackendKind::Ollama, Endpoint::new("http://h:1/v1/", None), Limits::default()).unwrap();
        assert_eq!(client.root_url("/api/show"), "http://h:1/api/show", "ohne /v1");
        let client = Client::new(BackendKind::Ollama, Endpoint::new("http://h:1", None), Limits::default()).unwrap();
        assert_eq!(client.root_url("/api/show"), "http://h:1/api/show", "Basis ohne /v1");
    }

    #[test]
    fn debug_of_a_client_never_shows_the_secret() {
        let client =
            Client::new(BackendKind::Openai, Endpoint::new("http://h", Some("Zz-geheim-9".into())), Limits::default())
                .unwrap();
        let shown = format!("{client:?}");
        assert!(!shown.contains("Zz-geheim-9"), "Debug ohne Schlüssel: {shown}");
    }
}
