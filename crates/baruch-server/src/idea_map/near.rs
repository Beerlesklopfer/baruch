// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nearness of the ideas on the idea map (contract Round 25 §2; display only, never stored).
//!
//! Two ideas shown are near each other if
//! - their **stored** vectors in the LanceDB index (rows `idea:<id>`, [`crate::rag::neighbors`]) have a
//!   cosine similarity from `near_threshold` on (default [`DEFAULT_THRESHOLD`]) (`via: vector`), or
//! - they carry an equal tag (same `word_key`: lower case, whitespace collapsed; `via: words`),
//!   also entirely without embedding; both together is `via: both`.
//!
//! Value (`score`): for `vector` the cosine similarity, for `words` the share of shared tags among all
//! tags of both (Jaccard), for `both` the larger of the two. Per idea at most `near_limit` (default
//! [`DEFAULT_LIMIT`]) partners: the pairs are taken by value descending (equal values by `a`, then `b`)
//! as long as neither of the two already has `near_limit` partners. Output by `(a, b)` with `a < b`.
//!
//! Drawing never embeds; without embedder or index the map honestly reports `no_embedder`, on an
//! error of the index `unavailable` – the tag nearness stays in both cases.

use std::collections::{HashMap, HashSet};

use baruch_store::{IdeaId, WorkId};

use super::{MapIdea, Near, NearPair, NearState, NearVia};
use crate::AppState;
use crate::rag::neighbors::{IdeaVectors, idea_vectors};

/// Default for `near_limit` (partners per idea).
pub const DEFAULT_LIMIT: u32 = 3;
/// Default for `near_threshold` (cosine similarity).
pub const DEFAULT_THRESHOLD: f32 = 0.75;
/// Round 23a: upper limit of the ideas whose nearness is computed. Comparing all pairs costs O(n²·d)
/// (n ideas, d dimensions of the vector); 1500 ideas are about 1.1 million pairs. If the map has more, only
/// the first [`MAX_NEAR_IDEAS`] by ascending id count, and the message of the map says so. The computation also
/// runs in `spawn_blocking` so that it occupies no worker thread of the runtime ([`compute`]).
pub const MAX_NEAR_IDEAS: usize = 1500;
/// Sentence for `near = false`.
pub const OFF: &str = "Nähe ausgeschaltet.";

/// Comparison key of a tag as in the store (`word_key`): lower case, whitespace to one space.
pub fn word_key(word: &str) -> String {
    word.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Bring a vector to length 1 (zero vector → `None`): then the cosine similarity is a dot product.
fn unit(vector: &[f32]) -> Option<Vec<f32>> {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    (norm > 0.0 && norm.is_finite()).then(|| vector.iter().map(|x| x / norm).collect())
}

/// Near pairs of the ideas `ideas` (id, tags) by the rules in the module head. `vectors` = stored
/// vectors (if one is missing, only the tag nearness counts for this pair).
pub fn pairs(
    ideas: &[(IdeaId, Vec<String>)],
    vectors: Option<&HashMap<IdeaId, Vec<f32>>>,
    limit: u32,
    threshold: f32,
) -> Vec<NearPair> {
    let mut sorted: Vec<&(IdeaId, Vec<String>)> = ideas.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);
    sorted.dedup_by_key(|(id, _)| *id);
    let keys: Vec<HashSet<String>> =
        sorted.iter().map(|(_, words)| words.iter().map(|w| word_key(w)).collect()).collect();
    let units: Vec<Option<Vec<f32>>> =
        sorted.iter().map(|(id, _)| vectors.and_then(|v| v.get(id)).and_then(|v| unit(v))).collect();
    let mut candidates: Vec<NearPair> = Vec::new();
    for i in 0..sorted.len() {
        for j in i + 1..sorted.len() {
            let vector = match (&units[i], &units[j]) {
                (Some(a), Some(b)) if a.len() == b.len() => {
                    Some(a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>().clamp(-1.0, 1.0))
                }
                _ => None,
            };
            let close = vector.is_some_and(|v| v >= threshold);
            let mut seen = HashSet::new();
            let shared: Vec<String> = sorted[i]
                .1
                .iter()
                .filter(|w| {
                    let key = word_key(w);
                    keys[j].contains(&key) && seen.insert(key)
                })
                .cloned()
                .collect();
            if !close && shared.is_empty() {
                continue;
            }
            let union = keys[i].union(&keys[j]).count().max(1);
            let jaccard = shared.len() as f32 / union as f32;
            let (via, score) = match (close, shared.is_empty()) {
                (true, false) => (NearVia::Both, vector.unwrap_or(0.0).max(jaccard)),
                (true, true) => (NearVia::Vector, vector.unwrap_or(0.0)),
                (false, _) => (NearVia::Words, jaccard),
            };
            candidates.push(NearPair { a: sorted[i].0, b: sorted[j].0, score, via, shared_words: shared });
        }
    }
    candidates.sort_by(|x, y| y.score.total_cmp(&x.score).then(x.a.cmp(&y.a)).then(x.b.cmp(&y.b)));
    let mut degree: HashMap<IdeaId, u32> = HashMap::new();
    let mut chosen = Vec::new();
    for pair in candidates {
        let (da, db) = (degree.get(&pair.a).copied().unwrap_or(0), degree.get(&pair.b).copied().unwrap_or(0));
        if da >= limit || db >= limit {
            continue;
        }
        degree.insert(pair.a, da + 1);
        degree.insert(pair.b, db + 1);
        chosen.push(pair);
    }
    chosen.sort_by_key(|p| (p.a, p.b));
    chosen
}

/// Nearness of the ideas shown of the work (see module head). `near = false` → `off` without pairs.
pub async fn compute(
    state: &AppState,
    work: WorkId,
    ideas: &[MapIdea],
    near: bool,
    limit: u32,
    threshold: f32,
) -> Near {
    if !near {
        return Near { state: NearState::Off, message: OFF.to_owned(), pairs: Vec::new() };
    }
    let mut list: Vec<(IdeaId, Vec<String>)> = ideas.iter().map(|i| (i.id, i.words.clone())).collect();
    list.sort_by_key(|(id, _)| *id);
    let capped = list.len() > MAX_NEAR_IDEAS;
    list.truncate(MAX_NEAR_IDEAS);
    let cap_note = if capped {
        format!(
            " Die Karte zeigt {} Einfälle; die Nähe ist nur für die ersten {MAX_NEAR_IDEAS} (nach Nummer) berechnet.",
            ideas.len()
        )
    } else {
        String::new()
    };
    let (state_word, message, vectors) = match idea_vectors(state, work).await {
        IdeaVectors::Ready(vectors) => {
            let have = ideas.iter().filter(|i| vectors.contains_key(&i.id)).count();
            let message = format!(
                "Nähe aus gespeicherten Einbettungen ({have} von {} Einfällen eingebettet) und gleichen \
                 Schlagwörtern; ab {threshold}, höchstens {limit} je Einfall.",
                ideas.len()
            );
            (NearState::Ok, message, Some(vectors))
        }
        IdeaVectors::NoEmbedder(message) => (NearState::NoEmbedder, message, None),
        IdeaVectors::Unavailable(message) => (NearState::Unavailable, message, None),
    };
    // Round 23a: O(n²·d) not on the async thread. If the task aborts (panic), the map stays without nearness and
    // says so instead of letting the whole page fail.
    let found =
        tokio::task::spawn_blocking(crate::i18n::carry(move || pairs(&list, vectors.as_ref(), limit, threshold))).await;
    match found {
        Ok(pairs) => Near { state: state_word, message: format!("{message}{cap_note}"), pairs },
        Err(e) => Near {
            state: NearState::Unavailable,
            message: format!("Die Nähe ließ sich nicht berechnen ({e})."),
            pairs: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idea(id: IdeaId, words: &[&str]) -> (IdeaId, Vec<String>) {
        (id, words.iter().map(|w| (*w).to_owned()).collect())
    }

    #[test]
    fn shared_words_are_near_without_any_embedding() {
        let ideas = [idea(3, &["Rose", "Laube"]), idea(1, &["rose"]), idea(2, &["Kiesel"])];
        let found = pairs(&ideas, None, 3, 0.75);
        // Contract §2: equal tags (word_key, i.e. ignoring case) count even without embedding.
        assert_eq!(found.len(), 1, "nur 1 und 3 teilen ein Wort: {found:?}");
        let pair = &found[0];
        assert_eq!((pair.a, pair.b), (1, 3), "a < b");
        assert_eq!(pair.via, NearVia::Words, "nur über Wörter");
        assert_eq!(pair.shared_words, vec!["rose".to_owned()], "geteiltes Wort in der Schreibweise von a");
        assert!((pair.score - 0.5).abs() < 1e-6, "Jaccard 1 von 2 Wörtern: {}", pair.score);
    }

    #[test]
    fn vectors_count_from_the_threshold_and_combine_with_words() {
        let ideas = [idea(1, &["Meer"]), idea(2, &["Welle"]), idea(3, &["Meer"]), idea(4, &["Docht"])];
        let vectors: HashMap<IdeaId, Vec<f32>> =
            [(1, vec![1.0, 0.0]), (2, vec![0.9, 0.1]), (3, vec![1.0, 0.05]), (4, vec![0.0, 1.0])].into();
        let found = pairs(&ideas, Some(&vectors), 3, 0.75);
        let by = |a, b| found.iter().find(|p| p.a == a && p.b == b);
        assert_eq!(by(1, 2).map(|p| p.via), Some(NearVia::Vector), "ähnliche Vektoren ohne gleiches Wort: {found:?}");
        assert_eq!(by(1, 3).map(|p| p.via), Some(NearVia::Both), "ähnlich und gleiches Wort: {found:?}");
        assert!(by(1, 4).is_none() && by(2, 4).is_none(), "senkrechte Vektoren liegen unter der Schwelle: {found:?}");
        // The threshold counts: at 0.999 only what also shares a word remains (1 and 3 through words).
        let strict = pairs(&ideas, Some(&vectors), 3, 0.999_9);
        assert!(strict.iter().all(|p| p.via == NearVia::Words), "nur noch Wort-Nähe: {strict:?}");
    }

    #[test]
    fn no_idea_gets_more_partners_than_the_limit() {
        // All ten share a word: without a limit each would have nine partners.
        let ideas: Vec<(IdeaId, Vec<String>)> = (1..=10).map(|i| idea(i, &["Garten"])).collect();
        for limit in 1..=5 {
            let found = pairs(&ideas, None, limit, 0.75);
            let mut degree: HashMap<IdeaId, u32> = HashMap::new();
            for p in &found {
                *degree.entry(p.a).or_default() += 1;
                *degree.entry(p.b).or_default() += 1;
                assert!(p.a < p.b, "a < b: {p:?}");
            }
            assert!(degree.values().all(|d| *d <= limit), "höchstens {limit} je Einfall: {degree:?}");
            assert_eq!(found, pairs(&ideas, None, limit, 0.75), "deterministisch");
        }
    }
}
