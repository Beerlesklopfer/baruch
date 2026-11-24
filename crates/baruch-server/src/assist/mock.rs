// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nachgebildetes Modell-Backend **nur für Tests** (Vertrag Runde 10, §3): [`ScriptedModel`] antwortet je
//! Aufruf nach einem Skript ([`Step`]) – Text in Stücken, ein Fehler, „hängt bis Abbruch“ oder „wartet
//! auf Freigabe“ –, zeichnet jeden [`ModelCall`] auf ([`ScriptedModel::calls`]) und zählt Abbrüche
//! ([`ScriptedModel::cancelled`]). Kein Netz. Öffentlich (nicht `cfg(test)`), damit `tests/*.rs` es nutzen.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

use super::port::{BoxFuture, ContextBudget, ModelAnswer, ModelCall, ModelFailure, ModelList, ModelPort};

/// Antwort auf einen Aufruf.
#[derive(Debug, Clone)]
pub enum Step {
    /// Text in diesen Stücken (je ein Delta), danach fertig. `model` = gemeldetes Modell (leer = das
    /// angefragte).
    Text { chunks: Vec<String>, model: String },
    /// Wie `Text`, aber an der Längengrenze abgeschnitten (`finish_reason` „length“).
    Cut { chunks: Vec<String> },
    /// Fehler des Backends.
    Fail(ModelFailure),
    /// Hängt, bis das Future fallen gelassen wird (Abbruch).
    Hang,
    /// Schickt die Stücke `before`, wartet dann auf [`ScriptedModel::release`], danach wie `then`.
    Wait { before: Vec<String>, then: Box<Step> },
}

impl Step {
    /// Ganze Antwort Wort für Wort (an Leerzeichen getrennt, das Leerzeichen bleibt am Wort).
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

    /// Wie [`Step::words`], aber das Modell meldet `finish_reason` „length“ (bei `max_tokens` abgebrochen).
    pub fn cut(text: &str) -> Step {
        match Step::words(text) {
            Step::Text { chunks, .. } => Step::Cut { chunks },
            _ => unreachable!("words liefert Text"),
        }
    }
}

/// Zustand, den Test und Port teilen.
#[derive(Default)]
struct Shared {
    steps: Mutex<VecDeque<Step>>,
    calls: Mutex<Vec<ModelCall>>,
    cancelled: AtomicUsize,
    finished: AtomicUsize,
    started: Notify,
    release: Notify,
}

/// Modell nach Skript. Klonen teilt den Zustand (Test behält einen Klon, der Server den anderen).
#[derive(Clone, Default)]
pub struct ScriptedModel {
    shared: Arc<Shared>,
    models: Option<Vec<String>>,
    aliases: HashMap<String, String>,
    pause: Duration,
    /// Von der „Gegenstelle“ gemeldete Kontextlänge ([`ScriptedModel::with_reported_context`]).
    reported_context: Option<u32>,
    /// Gemeldete Kontextlänge je Modell ([`ScriptedModel::with_reported_context_for`], Runde 16a); geht vor
    /// `reported_context`.
    reported_by_model: HashMap<String, u32>,
}

/// Merkt, ob ein Aufruf zu Ende lief; fällt er vorher, zählt er als Abbruch.
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
    /// Leeres Skript (jeder Aufruf scheitert mit „Skript zu Ende“), keine Modellliste.
    pub fn new() -> ScriptedModel {
        ScriptedModel::default()
    }

    /// Mit diesen Antworten in dieser Reihenfolge.
    pub fn with(steps: impl IntoIterator<Item = Step>) -> ScriptedModel {
        let model = ScriptedModel::new();
        model.push_all(steps);
        model
    }

    /// Weitere Antwort ans Ende des Skripts.
    pub fn push(&self, step: Step) {
        self.shared.steps.lock().unwrap().push_back(step);
    }

    pub fn push_all(&self, steps: impl IntoIterator<Item = Step>) {
        self.shared.steps.lock().unwrap().extend(steps);
    }

    /// Modellliste für [`ModelPort::models`] (ohne: `NotConfigured`).
    pub fn with_models(mut self, models: &[&str]) -> ScriptedModel {
        self.models = Some(models.iter().map(|m| (*m).to_owned()).collect());
        self
    }

    /// Alias alt → neu für [`ModelPort::alias`].
    pub fn with_alias(mut self, old: &str, new: &str) -> ScriptedModel {
        self.aliases.insert(old.to_owned(), new.to_owned());
        self
    }

    /// Pause zwischen zwei Stücken (Vorgabe keine).
    pub fn with_pause(mut self, pause: Duration) -> ScriptedModel {
        self.pause = pause;
        self
    }

    /// Die nachgebildete Gegenstelle meldet die Kontextlänge `tokens`: [`ModelPort::context`] liefert dann
    /// `tokens = min(Einstellung, tokens)`, `reported = Some(tokens)` und bei Abweichung eine Warnung.
    pub fn with_reported_context(mut self, tokens: u32) -> ScriptedModel {
        self.reported_context = Some(tokens);
        self
    }

    /// Wie [`ScriptedModel::with_reported_context`], aber nur für Personas mit dem Modell `model` (Runde 16a: so
    /// lässt sich prüfen, dass „Lektor automatisch nach Kontext“ die wirksame Länge vergleicht).
    pub fn with_reported_context_for(mut self, model: &str, tokens: u32) -> ScriptedModel {
        self.reported_by_model.insert(model.to_owned(), tokens);
        self
    }

    /// Alle bisherigen Aufrufe in ihrer Reihenfolge.
    pub fn calls(&self) -> Vec<ModelCall> {
        self.shared.calls.lock().unwrap().clone()
    }

    /// Zahl der Aufrufe, die fallen gelassen wurden, bevor sie fertig waren.
    pub fn cancelled(&self) -> usize {
        self.shared.cancelled.load(Ordering::SeqCst)
    }

    /// Zahl der Aufrufe, die zu Ende liefen (Text oder Fehler).
    pub fn finished(&self) -> usize {
        self.shared.finished.load(Ordering::SeqCst)
    }

    /// Wartet, bis ein Aufruf begonnen hat (nach dem Aufzeichnen, vor der ersten Antwort).
    pub async fn started(&self) {
        self.shared.started.notified().await;
    }

    /// Gibt einen wartenden Aufruf ([`Step::Wait`]) frei.
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
        // Stücke vor einer Wartestelle gehören zur Antwort.
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

    /// Der Nachbau kennt seine Kontextlängen ohne Netz: wie [`ModelPort::context`].
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
