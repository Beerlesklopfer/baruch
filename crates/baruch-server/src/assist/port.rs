// SPDX-License-Identifier: AGPL-3.0-or-later
//! Schnittstelle zwischen den Personas und einem Modell-Backend (Vertrag Runde 10, §3).
//!
//! Die Personas ([`super`]) kennen nur diesen Port; im Dienst steckt dahinter Eliezer (Adapter des
//! Integrators), in Tests [`super::mock::ScriptedModel`], ohne Anbindung [`NoModel`].
//!
//! Abbruch = Fallenlassen des Futures: Trennt der Client den Strom, fällt der Strom, damit das Future
//! von [`ModelPort::chat`] und mit ihm die HTTP-Anfrage an das Backend.

/// Future, das ein Port zurückgibt (ohne `async fn` im Trait, damit `dyn ModelPort` geht).
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// Rolle einer Nachricht im Gespräch mit dem Modell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Eine Nachricht an das Modell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Ein Modellaufruf. `label` beschreibt ihn ohne Text (z. B. „@lektor ask key 17“) für Warteschlange
/// und Protokoll.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelCall {
    pub model: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub label: String,
    /// Handle der Persona mit „@“ (wählt Verbindung, Freigabe „extern“, Kontext); leer = keine Persona.
    pub persona: String,
}

/// Kontext einer Persona für Aufträge (R15b): Einstellung abgeglichen mit der Gegenstelle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ContextBudget {
    /// Wirksame Kontextlänge in Tokens: min(Einstellung der Persona, von der Gegenstelle gemeldete Länge), ohne Meldung die Einstellung.
    pub tokens: u32,
    /// Von der Gegenstelle gemeldete Länge; None = meldet keine bzw. nicht erreichbar.
    pub reported: Option<u32>,
    /// Deutscher Satz bei Abweichung (z. B. „Einstellung 65536, Eliezer meldet 32768“), sonst None.
    pub warning: Option<String>,
}

/// Fertige Antwort des Modells; `model` ist das tatsächlich verwendete Modell (nach Alias).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelAnswer {
    pub text: String,
    pub model: String,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

/// Warum ein Modellaufruf scheitert. Die Texte sind deutsche Sätze für die Oberfläche und enthalten
/// nie ein Geheimnis.
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

/// Modelle, die das Backend kennt; `live = false`: aus dem Zwischenspeicher, `message` ein Hinweis.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModelList {
    pub models: Vec<String>,
    pub live: bool,
    pub message: Option<String>,
}

/// Ein Modell-Backend.
pub trait ModelPort: Send + Sync {
    /// Gespräch an das Modell; jedes Stück der Antwort geht sofort an `deltas` (Wort für Wort). Das
    /// Fallenlassen des Futures bricht den Aufruf ab.
    fn chat<'a>(
        &'a self,
        state: &'a crate::AppState,
        call: ModelCall,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>>;

    /// Modelle, die das Backend anbietet.
    fn models<'a>(&'a self, state: &'a crate::AppState) -> BoxFuture<'a, Result<ModelList, ModelFailure>>;

    /// Neuer Name eines alten Modellnamens (z. B. aus Ollama-Zeiten), falls bekannt.
    fn alias(&self, name: &str) -> Option<String>;

    /// Kontext für Aufträge an `persona` (R15b). Wartet höchstens so lange wie die Modellliste (Zwischenspeicher); scheitert nie.
    fn context<'a>(
        &'a self,
        state: &'a crate::AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        let _ = state;
        Box::pin(async move { ContextBudget { tokens: persona.context_length, reported: None, warning: None } })
    }

    /// Kontext ohne Netzabfrage (Runde 17a, L4): was der Port schon weiß, sonst die Einstellung. Für Seiten und
    /// Listen (Einstellungen, `persona.list`, Arbeitsseite) – „Lektor automatisch nach Kontext“ fragt dort nie die
    /// Gegenstelle; das Netz bleibt dem Modellaufruf ([`ModelPort::context`]).
    fn context_cached<'a>(
        &'a self,
        state: &'a crate::AppState,
        persona: &'a baruch_store::PersonaRow,
    ) -> BoxFuture<'a, ContextBudget> {
        let _ = state;
        Box::pin(async move { ContextBudget { tokens: persona.context_length, reported: None, warning: None } })
    }
}

/// Ehrliche Meldung ohne Backend.
const NO_MODEL: &str = "Kein Modell-Backend angebunden – Baruch läuft ohne Modellaufrufe (Tests, Hilfsbefehle).";

/// Kein Backend: Jeder Aufruf meldet ehrlich, dass keins angebunden ist.
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
