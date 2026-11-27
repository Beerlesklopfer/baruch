// SPDX-License-Identifier: AGPL-3.0-or-later
//! Neighbours in the local index (Round 25, idea map): the **stored** vectors of the ideas of a
//! work (rows `idea:<id>` in the scope of the work) and their cosine similarity.
//!
//! Drawing the map never embeds: [`idea_vectors`] only reads what the background task
//! ([`super::index`]) has already put into LanceDB. Without a connected embedder or without an index it answers
//! honestly [`IdeaVectors::NoEmbedder`] (without creating the index folder), on an error of the index or
//! another configured model [`IdeaVectors::Unavailable`] – the map then shows only the nearness through
//! shared keywords.

use std::collections::HashMap;

use baruch_lancedb::Scope;
use baruch_store::{IdeaId, WorkId};

use super::index::{config, mismatch};
use crate::AppState;

/// Sentence: no embedder connected.
pub const NO_EMBEDDER: &str = "Keine Einbettung angebunden (Einstellungen → Verbindungen); Nähe nur über gleiche \
                               Schlagwörter.";
/// Sentence: index without vectors of the ideas of this work.
pub const NOT_INDEXED: &str = "Im Index liegen noch keine Einbettungen der Einfälle dieses Werks; Nähe nur über \
                               gleiche Schlagwörter.";

/// Vectors of the ideas of a work from the index.
#[derive(Debug, Clone, PartialEq)]
pub enum IdeaVectors {
    /// No embedder or no index (yet) or no row of an idea; sentence for the display.
    NoEmbedder(String),
    /// Index not readable or built with another model; sentence for the display.
    Unavailable(String),
    /// Idea → stored vector (only ideas that are already embedded).
    Ready(HashMap<IdeaId, Vec<f32>>),
}

/// Read the stored vectors of the ideas of the work (no embedding request).
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

/// Cosine similarity of two stored vectors (0 for a zero vector or unequal length).
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
        // Same direction = 1, perpendicular = 0: the threshold near_threshold measures exactly that.
        assert!((similarity(&[1.0, 0.0], &[2.0, 0.0]) - 1.0).abs() < 1e-6, "gleiche Richtung");
        assert!(similarity(&[1.0, 0.0], &[0.0, 3.0]).abs() < 1e-6, "senkrecht");
        // Different dimensions come from different models: no nearness instead of a random value.
        assert_eq!(similarity(&[1.0, 0.0], &[1.0, 0.0, 0.0]), 0.0, "ungleiche Länge");
        assert_eq!(similarity(&[0.0, 0.0], &[1.0, 0.0]), 0.0, "Nullvektor");
    }
}
