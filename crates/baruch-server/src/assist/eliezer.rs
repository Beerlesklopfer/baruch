// SPDX-License-Identifier: AGPL-3.0-or-later
//! Adapter zwischen dem Port der Personas ([`super::ModelPort`]) und den Modell-Backends
//! ([`crate::models`], Runde 10, Integrator; Runde 15b: Verbindung je Persona). Die Typen werden 1:1
//! abgebildet; Verbindung, Adresse, Schlüssel, Warteschlange (eine Anfrage zur Zeit je Endpunkt) und
//! Aliasse regelt [`crate::models`]. Abbruch: Fällt das Future, fällt die HTTP-Anfrage mit und gibt ihren
//! Platz in der Warteschlange frei.
//!
//! **Freigabe „extern“** (R15b): [`EliezerPort::chat`] wählt die Verbindung der Persona aus
//! [`ModelCall::persona`]. Ist sie extern und die Persona nicht freigegeben, endet der Aufruf mit
//! [`ModelFailure::NotConfigured`] und einem deutschen Satz, **ohne** dass eine Anfrage hinausgeht – das gilt
//! für jeden Aufruf über den Port, also auch für die Zusammenfassungen des Lektors.

use baruch_llm::{ChatMessage, ChatRequest, LlmError};

use super::port::{BoxFuture, ContextBudget, ModelAnswer, ModelCall, ModelFailure, ModelList, ModelPort, Role};
use crate::AppState;

/// Modell-Backends des Dienstes (`baruch-server serve`): je Persona ihre Verbindung (Vorgabe Eliezer).
pub struct EliezerPort;

fn role(role: Role) -> baruch_llm::Role {
    match role {
        Role::System => baruch_llm::Role::System,
        Role::User => baruch_llm::Role::User,
        Role::Assistant => baruch_llm::Role::Assistant,
    }
}

/// [`LlmError`] → [`ModelFailure`] (gleiche Varianten, gleiche Nutzlast).
pub fn failure(error: LlmError) -> ModelFailure {
    match error {
        LlmError::NotConfigured(m) => ModelFailure::NotConfigured(m),
        LlmError::Unreachable(m) => ModelFailure::Unreachable(m),
        LlmError::Denied(m) => ModelFailure::Denied(m),
        LlmError::Timeout(m) => ModelFailure::Timeout(m),
        LlmError::UnknownModel(m) => ModelFailure::UnknownModel(m),
        LlmError::Busy(m) => ModelFailure::Busy(m),
        LlmError::Cancelled => ModelFailure::Cancelled,
        LlmError::Failed(m) => ModelFailure::Failed(m),
    }
}

impl ModelPort for EliezerPort {
    fn chat<'a>(
        &'a self,
        state: &'a AppState,
        call: ModelCall,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>> {
        Box::pin(async move {
            // Erst die Freigabe: externe Verbindung ohne Freigabe → nichts geht hinaus.
            let target = crate::models::route(state, &call.persona).await.map_err(failure)?;
            let request = ChatRequest {
                model: call.model,
                messages: call
                    .messages
                    .into_iter()
                    .map(|m| ChatMessage { role: role(m.role), content: m.content })
                    .collect(),
                max_tokens: call.max_tokens,
                temperature: call.temperature,
            };
            let reply = crate::models::chat_on(&target, request, &call.label, deltas).await.map_err(failure)?;
            Ok(ModelAnswer {
                text: reply.text,
                model: reply.model,
                finish_reason: reply.finish_reason,
                prompt_tokens: reply.prompt_tokens,
                completion_tokens: reply.completion_tokens,
            })
        })
    }

    fn models<'a>(&'a self, state: &'a AppState) -> BoxFuture<'a, Result<ModelList, ModelFailure>> {
        Box::pin(async move {
            let listing = crate::models::list(state).await.map_err(failure)?;
            let message = listing.message.clone().or_else(|| {
                (!listing.live).then(|| format!("Liste aus dem Zwischenspeicher (Stand {}).", listing.fetched))
            });
            Ok(ModelList { models: listing.models, live: listing.live, message })
        })
    }

    fn alias(&self, name: &str) -> Option<String> {
        crate::models::alias(name).map(str::to_owned)
    }

    fn context<'a>(
        &'a self,
        state: &'a AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        Box::pin(crate::models::context(state, persona))
    }

    fn context_cached<'a>(
        &'a self,
        state: &'a AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        Box::pin(crate::models::context_cached_for(state, persona))
    }
}
