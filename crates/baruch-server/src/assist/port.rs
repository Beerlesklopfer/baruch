// SPDX-License-Identifier: AGPL-3.0-or-later
//! Interface between the personas and a model backend (contract Round 10, §3).
//!
//! The personas ([`super`]) know only this port; in the service Eliezer is behind it (adapter of the
//! integrator), in tests [`super::mock::ScriptedModel`], without a connection [`NoModel`].
//!
//! Cancellation = dropping the future: if the client disconnects the stream, the stream drops, with it the future
//! of [`ModelPort::chat`] and with that the HTTP request to the backend.

/// Future that a port returns (without `async fn` in the trait so that `dyn ModelPort` works).
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// Role of a message in the conversation with the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A message to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// A model call. `label` describes it without text (e.g. „@lektor ask key 17“) for queue
/// and log.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelCall {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub label: String,
    /// Handle of the persona with „@“ (selects connection, release „extern“, context); empty = no persona.
    pub persona: String,
}

/// Context of a persona for orders (R15b): setting reconciled with the remote side.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ContextBudget {
    /// Effective context length in tokens: min(persona setting, length reported by the remote side), else the setting.
    pub tokens: u32,
    /// Length reported by the remote side; None = reports none or not reachable.
    pub reported: Option<u32>,
    /// German sentence on deviation (e.g. „Einstellung 65536, Eliezer meldet 32768“), otherwise None.
    pub warning: Option<String>,
}

/// Finished answer of the model; `model` is the model actually used (after alias).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelAnswer {
    pub text: String,
    pub model: String,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

/// Why a model call fails. The texts are German sentences for the UI and never contain
/// a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelFailure {
    NotConfigured(String),
    Unreachable(String),
    Denied(String),
    Timeout(String),
    UnknownModel(String),
    Busy(String),
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for ModelFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelFailure::NotConfigured(why) => write!(f, "Kein Modell-Backend nutzbar: {why}"),
            ModelFailure::Unreachable(why) => write!(f, "Das Modell-Backend ist nicht erreichbar: {why}"),
            ModelFailure::Denied(why) => write!(f, "Das Modell-Backend verweigert den Zugang: {why}"),
            ModelFailure::Timeout(why) => write!(f, "Zeitüberschreitung beim Modell: {why}"),
            ModelFailure::UnknownModel(why) => write!(f, "Unbekanntes Modell: {why}"),
            ModelFailure::Busy(why) => write!(f, "Das Modell ist ausgelastet: {why}"),
            ModelFailure::Cancelled => f.write_str("Abgebrochen; es wurde nichts gespeichert."),
            ModelFailure::Failed(why) => write!(f, "Der Modellaufruf ist gescheitert: {why}"),
        }
    }
}

impl std::error::Error for ModelFailure {}

/// Models the backend knows; `live = false`: from the cache, `message` a note.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModelList {
    pub models: Vec<String>,
    pub live: bool,
    pub message: Option<String>,
}

/// A model backend.
pub trait ModelPort: Send + Sync {
    /// Conversation to the model; every piece of the answer goes to `deltas` immediately (word by word).
    /// Dropping the future cancels the call.
    fn chat<'a>(
        &'a self,
        state: &'a crate::AppState,
        call: ModelCall,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>>;

    /// Models the backend offers.
    fn models<'a>(&'a self, state: &'a crate::AppState) -> BoxFuture<'a, Result<ModelList, ModelFailure>>;

    /// New name of an old model name (e.g. from Ollama times), if known.
    fn alias(&self, name: &str) -> Option<String>;

    /// Context for orders to `persona` (R15b). Waits at most as long as the model list (cache); never fails.
    fn context<'a>(
        &'a self,
        state: &'a crate::AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        let _ = state;
        Box::pin(async move { ContextBudget { tokens: persona.context_length, reported: None, warning: None } })
    }

    /// Context without a network request (Round 17a, L4): what the port already knows, otherwise the setting. For pages
    /// and lists (settings, `persona.list`, work page) – „Lektor automatisch nach Kontext“ never asks the
    /// remote side there; the network is left to the model call ([`ModelPort::context`]).
    fn context_cached<'a>(
        &'a self,
        state: &'a crate::AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        let _ = state;
        Box::pin(async move { ContextBudget { tokens: persona.context_length, reported: None, warning: None } })
    }
}

/// Honest message without a backend.
const NO_MODEL: &str = "Kein Modell-Backend angebunden – Baruch läuft ohne Modellaufrufe (Tests, Hilfsbefehle).";

/// No backend: every call honestly reports that none is connected.
pub struct NoModel;

impl ModelPort for NoModel {
    fn chat<'a>(
        &'a self,
        _state: &'a crate::AppState,
        _call: ModelCall,
        _deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>> {
        Box::pin(async { Err(ModelFailure::NotConfigured(NO_MODEL.to_owned())) })
    }

    fn models<'a>(&'a self, _state: &'a crate::AppState) -> BoxFuture<'a, Result<ModelList, ModelFailure>> {
        Box::pin(async { Err(ModelFailure::NotConfigured(NO_MODEL.to_owned())) })
    }

    fn alias(&self, _name: &str) -> Option<String> {
        None
    }
}
