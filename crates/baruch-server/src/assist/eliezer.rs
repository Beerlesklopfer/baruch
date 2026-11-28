// SPDX-License-Identifier: AGPL-3.0-or-later
//! Adapter between the port of the personas ([`super::ModelPort`]) and the model backends
//! ([`crate::models`], Round 10, integrator; Round 15b: connection per persona). The types are mapped
//! 1:1; connection, address, key, queue (one request at a time per endpoint) and
//! aliases are handled by [`crate::models`]. Cancellation: if the future is dropped, the HTTP request drops with it
//! and frees its place in the queue.
//!
//! **Release „extern“** (R15b): [`EliezerPort::chat`] picks the connection of the persona from
//! [`ModelCall::persona`]. If it is external and the persona is not released, the call ends with
//! [`ModelFailure::NotConfigured`] and a German sentence, **without** any request going out – this holds
//! for every call through the port, so also for the lector's summaries.

use baruch_llm::{ChatMessage, ChatRequest, LlmError};

use super::port::{BoxFuture, ContextBudget, ModelAnswer, ModelCall, ModelFailure, ModelList, ModelPort, Role};
use crate::AppState;

/// Model backends of the service (`baruch-server serve`): per persona its connection (default Eliezer).
pub struct EliezerPort;

fn role(role: Role) -> baruch_llm::Role {
    match role {
        Role::System => baruch_llm::Role::System,
        Role::User => baruch_llm::Role::User,
        Role::Assistant => baruch_llm::Role::Assistant,
    }
}

/// [`LlmError`] → [`ModelFailure`] (same variants, same payload).
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
            // First the release: external connection without release → nothing goes out.
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
                (!listing.live).then(|| {
                    crate::i18n::tc("assist_msg.model.cached_list", &[("fetched", listing.fetched.as_str().into())])
                })
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
