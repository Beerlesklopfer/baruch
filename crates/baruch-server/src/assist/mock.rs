// SPDX-License-Identifier: AGPL-3.0-or-later
//! Mock model backend **for tests only** (contract Round 10, §3): [`ScriptedModel`] answers each
//! call by a script ([`Step`]) – text in pieces, an error, “hangs until cancelled” or “waits
//! for release” –, records every [`ModelCall`] ([`ScriptedModel::calls`]) and counts cancellations
//! ([`ScriptedModel::cancelled`]). No network. Public (not `cfg(test)`) so that `tests/*.rs` can use it.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

use super::port::{BoxFuture, ContextBudget, ModelAnswer, ModelCall, ModelFailure, ModelList, ModelPort};

/// Answer to a call.
#[derive(Debug, Clone)]
pub enum Step {
    /// Text in these pieces (one delta each), then done. `model` = reported model (empty = the
    /// requested one).
    Text { chunks: Vec<String>, model: String },
    /// Like `Text`, but cut off at the length limit (`finish_reason` “length”).
    Cut { chunks: Vec<String> },
    /// Error of the backend.
    Fail(ModelFailure),
    /// Hangs until the future is dropped (cancellation).
    Hang,
    /// Sends the pieces `before`, then waits for [`ScriptedModel::release`], then like `then`.
    Wait { before: Vec<String>, then: Box<Step> },
}

impl Step {
    /// Whole answer word by word (split at spaces, the space stays with the word).
    pub fn words(text: &str) -> Step {
        let mut chunks = Vec::new();
        let mut current = String::new();
        for c in text.chars() {
            current.push(c);
            if c == ' ' {
                chunks.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            chunks.push(current);
        }
        Step::Text { chunks, model: String::new() }
    }

    /// Like [`Step::words`], but the model reports `finish_reason` “length” (cut off at `max_tokens`).
    pub fn cut(text: &str) -> Step {
        match Step::words(text) {
            Step::Text { chunks, .. } => Step::Cut { chunks },
            _ => unreachable!("words liefert Text"),
        }
    }
}

/// State shared by test and port.
#[derive(Default)]
struct Shared {
    steps: Mutex<VecDeque<Step>>,
    calls: Mutex<Vec<ModelCall>>,
    cancelled: AtomicUsize,
    finished: AtomicUsize,
    started: Notify,
    release: Notify,
}

/// Scripted model. Cloning shares the state (the test keeps one clone, the server the other).
#[derive(Clone, Default)]
pub struct ScriptedModel {
    shared: Arc<Shared>,
    models: Option<Vec<String>>,
    aliases: HashMap<String, String>,
    pause: Duration,
    /// Context length reported by the “remote side” ([`ScriptedModel::with_reported_context`]).
    reported_context: Option<u32>,
    /// Reported context length per model ([`ScriptedModel::with_reported_context_for`], Round 16a); takes precedence
    /// over `reported_context`.
    reported_by_model: HashMap<String, u32>,
}

/// Remembers whether a call ran to the end; if it is dropped before, it counts as a cancellation.
struct CancelGuard {
    shared: Arc<Shared>,
    done: bool,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        if self.done {
            self.shared.finished.fetch_add(1, Ordering::SeqCst);
        } else {
            self.shared.cancelled.fetch_add(1, Ordering::SeqCst);
        }
    }
}

impl ScriptedModel {
    /// Empty script (every call fails with „Skript zu Ende“), no model list.
    pub fn new() -> ScriptedModel {
        ScriptedModel::default()
    }

    /// With these answers in this order.
    pub fn with(steps: impl IntoIterator<Item = Step>) -> ScriptedModel {
        let model = ScriptedModel::new();
        model.push_all(steps);
        model
    }

    /// Another answer at the end of the script.
    pub fn push(&self, step: Step) {
        self.shared.steps.lock().unwrap().push_back(step);
    }

    pub fn push_all(&self, steps: impl IntoIterator<Item = Step>) {
        self.shared.steps.lock().unwrap().extend(steps);
    }

    /// Model list for [`ModelPort::models`] (without: `NotConfigured`).
    pub fn with_models(mut self, models: &[&str]) -> ScriptedModel {
        self.models = Some(models.iter().map(|m| (*m).to_owned()).collect());
        self
    }

    /// Alias old → new for [`ModelPort::alias`].
    pub fn with_alias(mut self, old: &str, new: &str) -> ScriptedModel {
        self.aliases.insert(old.to_owned(), new.to_owned());
        self
    }

    /// Pause between two pieces (default none).
    pub fn with_pause(mut self, pause: Duration) -> ScriptedModel {
        self.pause = pause;
        self
    }

    /// The mock remote side reports the context length `tokens`: [`ModelPort::context`] then returns
    /// `tokens = min(setting, tokens)`, `reported = Some(tokens)` and a warning on deviation.
    pub fn with_reported_context(mut self, tokens: u32) -> ScriptedModel {
        self.reported_context = Some(tokens);
        self
    }

    /// Like [`ScriptedModel::with_reported_context`], but only for personas with the model `model` (Round 16a: this
    /// allows checking that „Lektor automatisch nach Kontext“ compares the effective length).
    pub fn with_reported_context_for(mut self, model: &str, tokens: u32) -> ScriptedModel {
        self.reported_by_model.insert(model.to_owned(), tokens);
        self
    }

    /// All calls so far in their order.
    pub fn calls(&self) -> Vec<ModelCall> {
        self.shared.calls.lock().unwrap().clone()
    }

    /// Number of calls that were dropped before they finished.
    pub fn cancelled(&self) -> usize {
        self.shared.cancelled.load(Ordering::SeqCst)
    }

    /// Number of calls that ran to the end (text or error).
    pub fn finished(&self) -> usize {
        self.shared.finished.load(Ordering::SeqCst)
    }

    /// Waits until a call has started (after recording, before the first answer).
    pub async fn started(&self) {
        self.shared.started.notified().await;
    }

    /// Releases a waiting call ([`Step::Wait`]).
    pub fn release(&self) {
        self.shared.release.notify_one();
    }

    async fn play(
        &self,
        step: Step,
        call: &ModelCall,
        deltas: &tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<ModelAnswer, ModelFailure> {
        let mut step = step;
        // Pieces before a waiting point belong to the answer.
        let mut sent = String::new();
        loop {
            match step {
                Step::Text { .. } | Step::Cut { .. } => {
                    let (chunks, model, reason) = match step {
                        Step::Text { chunks, model } => (chunks, model, "stop"),
                        Step::Cut { chunks } => (chunks, String::new(), "length"),
                        _ => unreachable!("eben geprüft"),
                    };
                    let mut text = std::mem::take(&mut sent);
                    for chunk in chunks {
                        if !self.pause.is_zero() {
                            tokio::time::sleep(self.pause).await;
                        } else {
                            tokio::task::yield_now().await;
                        }
                        text.push_str(&chunk);
                        let _ = deltas.send(chunk);
                    }
                    let model = if model.is_empty() { call.model.clone() } else { model };
                    return Ok(ModelAnswer {
                        completion_tokens: Some(u32::try_from(text.chars().count().div_ceil(3)).unwrap_or(u32::MAX)),
                        text,
                        model,
                        finish_reason: Some(reason.to_owned()),
                        prompt_tokens: None,
                    });
                }
                Step::Fail(failure) => return Err(failure),
                Step::Hang => std::future::pending::<()>().await,
                Step::Wait { before, then } => {
                    for chunk in before {
                        sent.push_str(&chunk);
                        let _ = deltas.send(chunk);
                    }
                    self.shared.release.notified().await;
                    step = *then;
                }
            }
        }
    }
}

impl ModelPort for ScriptedModel {
    fn chat<'a>(
        &'a self,
        _state: &'a crate::AppState,
        call: ModelCall,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>> {
        Box::pin(async move {
            let mut guard = CancelGuard { shared: self.shared.clone(), done: false };
            self.shared.calls.lock().unwrap().push(call.clone());
            let step = self.shared.steps.lock().unwrap().pop_front();
            self.shared.started.notify_one();
            let result = match step {
                Some(step) => self.play(step, &call, &deltas).await,
                None => Err(ModelFailure::Failed("Skript zu Ende (ScriptedModel)".to_owned())),
            };
            guard.done = true;
            result
        })
    }

    fn models<'a>(&'a self, _state: &'a crate::AppState) -> BoxFuture<'a, Result<ModelList, ModelFailure>> {
        Box::pin(async move {
            match &self.models {
                Some(models) => Ok(ModelList { models: models.clone(), live: true, message: None }),
                None => Err(ModelFailure::NotConfigured("ScriptedModel ohne Modellliste".to_owned())),
            }
        })
    }

    fn alias(&self, name: &str) -> Option<String> {
        self.aliases.get(name).cloned()
    }

    /// The mock knows its context lengths without network: like [`ModelPort::context`].
    fn context_cached<'a>(
        &'a self,
        state: &'a crate::AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        self.context(state, persona)
    }

    fn context<'a>(
        &'a self,
        _state: &'a crate::AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        Box::pin(async move {
            let setting = persona.context_length;
            match self.reported_by_model.get(&persona.model).copied().or(self.reported_context) {
                None => ContextBudget { tokens: setting, reported: None, warning: None },
                Some(reported) => ContextBudget {
                    tokens: setting.min(reported),
                    reported: Some(reported),
                    warning: (reported != setting).then(|| format!("Einstellung {setting}, Nachbau meldet {reported}")),
                },
            }
        })
    }
}
