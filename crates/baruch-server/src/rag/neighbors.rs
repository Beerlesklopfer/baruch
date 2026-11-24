// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nachbarn im lokalen Index (Runde 25, Gedankenkarte): die **gespeicherten** Vektoren der Einfälle eines
//! Werks (Zeilen `idea:<id>` im Bereich des Werks) und ihre Kosinus-Ähnlichkeit.
//!
//! Beim Zeichnen der Karte wird nie eingebettet: [`idea_vectors`] liest nur, was die Hintergrund-Aufgabe
//! ([`super::index`]) schon in LanceDB abgelegt hat. Ohne angebundenen Einbetter oder ohne Index antwortet es
//! ehrlich [`IdeaVectors::NoEmbedder`] (ohne den Index-Ordner anzulegen), bei einem Fehler des Index oder
//! einem anderen eingestellten Modell [`IdeaVectors::Unavailable`] – die Karte zeigt dann nur die Nähe über
//! gleiche Schlagwörter.

use std::collections::HashMap;

use baruch_lancedb::Scope;
use baruch_store::{IdeaId, WorkId};

use super::index::{config, mismatch};
use crate::AppState;

/// Satz: kein Einbetter angebunden.
pub const NO_EMBEDDER: &str = "Keine Einbettung angebunden (Einstellungen → Verbindungen); Nähe nur über gleiche \
                               Schlagwörter.";
/// Satz: Index ohne Vektoren der Einfälle dieses Werks.
pub const NOT_INDEXED: &str = "Im Index liegen noch keine Einbettungen der Einfälle dieses Werks; Nähe nur über \
                               gleiche Schlagwörter.";

/// Vektoren der Einfälle eines Werks aus dem Index.
#[derive(Debug, Clone, PartialEq)]
pub enum IdeaVectors {
    /// Kein Einbetter oder (noch) kein Index bzw. keine Zeile eines Einfalls; Satz für die Anzeige.
    NoEmbedder(String),
    /// Index nicht lesbar oder mit einem anderen Modell aufgebaut; Satz für die Anzeige.
    Unavailable(String),
    /// Einfall → gespeicherter Vektor (nur Einfälle, die schon eingebettet sind).
    Ready(HashMap<IdeaId, Vec<f32>>),
}

/// Gespeicherte Vektoren der Einfälle des Werks lesen (keine Einbettungsanfrage).
pub async fn idea_vectors(state: &AppState, work: WorkId) -> IdeaVectors {
    let rt = super::runtime(state);
    if rt.embedders().is_none() {
        return IdeaVectors::NoEmbedder(NO_EMBEDDER.to_owned());
    }
    let store = rt.store();
    let cfg = match config(&store, &state.data).await {
        Ok(cfg) => cfg,
        Err(e) => return IdeaVectors::Unavailable(format!("Nähe aus dem Index nicht verfügbar: {}", e.message())),
    };
    if !cfg.dir.is_dir() {
        return IdeaVectors::NoEmbedder(NOT_INDEXED.to_owned());
    }
    let index = match rt.index(&cfg.dir).await {
        Ok(index) => index,
        Err(e) => return IdeaVectors::Unavailable(format!("Nähe aus dem Index nicht verfügbar: {}", e.message())),
    };
    match index.meta() {
        Ok(meta) => {
            if let Some(e) = mismatch(&meta, &cfg) {
                return IdeaVectors::Unavailable(format!("Nähe aus dem Index nicht verfügbar: {}", e.message()));
            }
        }
        Err(e) => return IdeaVectors::Unavailable(format!("Nähe aus dem Index nicht verfügbar: {}", e.message())),
    }
    let stored = match index.stored(Scope::Work(work)).await {
        Ok(stored) => stored,
        Err(e) => return IdeaVectors::Unavailable(format!("Nähe aus dem Index nicht verfügbar: {}", e.message())),
    };
    let vectors: HashMap<IdeaId, Vec<f32>> = stored
        .into_iter()
        .filter_map(|row| {
            let id = row.id.strip_prefix("idea:")?.parse().ok()?;
            Some((id, row.vector))
        })
        .collect();
    if vectors.is_empty() {
        return IdeaVectors::NoEmbedder(NOT_INDEXED.to_owned());
    }
    IdeaVectors::Ready(vectors)
}

/// Kosinus-Ähnlichkeit zweier gespeicherter Vektoren (0 bei Nullvektor oder ungleicher Länge).
pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    baruch_lancedb::cosine(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_is_the_cosine_and_refuses_mixed_dimensions() {
        // Gleiche Richtung = 1, senkrecht = 0: die Schwelle near_threshold misst genau das.
        assert!((similarity(&[1.0, 0.0], &[2.0, 0.0]) - 1.0).abs() < 1e-6, "gleiche Richtung");
        assert!(similarity(&[1.0, 0.0], &[0.0, 3.0]).abs() < 1e-6, "senkrecht");
        // Verschiedene Dimensionen stammen von verschiedenen Modellen: keine Nähe statt eines Zufallswerts.
        assert_eq!(similarity(&[1.0, 0.0], &[1.0, 0.0, 0.0]), 0.0, "ungleiche Länge");
        assert_eq!(similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0, "Nullvektor");
    }
}
