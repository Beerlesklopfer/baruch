// SPDX-License-Identifier: AGPL-3.0-or-later
//! Nähe der Einfälle auf der Gedankenkarte (Vertrag Runde 25 §2; nur Anzeige, nie gespeichert).
//!
//! Zwei gezeigte Einfälle sind einander nah, wenn
//! - ihre **gespeicherten** Vektoren im LanceDB-Index (Zeilen `idea:<id>`, [`crate::rag::neighbors`]) eine
//!   Kosinus-Ähnlichkeit ab `near_threshold` (Vorgabe [`DEFAULT_THRESHOLD`]) haben (`via: vector`), oder
//! - sie ein gleiches Schlagwort tragen (gleicher `word_key`: klein, Leerraum zusammengefasst; `via: words`),
//!   auch ganz ohne Einbettung; beides zusammen ist `via: both`.
//!
//! Wert (`score`): bei `vector` die Kosinus-Ähnlichkeit, bei `words` der Anteil gemeinsamer Schlagwörter an allen
//! Schlagwörtern beider (Jaccard), bei `both` der größere der beiden. Je Einfall höchstens `near_limit` (Vorgabe
//! [`DEFAULT_LIMIT`]) Partner: Die Paare werden nach Wert absteigend (gleiche Werte nach `a`, dann `b`) genommen,
//! solange keiner der beiden schon `near_limit` Partner hat. Ausgegeben nach `(a, b)` mit `a < b`.
//!
//! Beim Zeichnen wird nie eingebettet; ohne Einbetter oder Index meldet die Karte ehrlich `no_embedder`, bei einem
//! Fehler des Index `unavailable` – die Schlagwort-Nähe bleibt in beiden Fällen.

use std::collections::{HashMap, HashSet};

use baruch_store::{IdeaId, WorkId};

use super::{MapIdea, Near, NearPair, NearState, NearVia};
use crate::AppState;
use crate::rag::neighbors::{IdeaVectors, idea_vectors};

/// Vorgabe für `near_limit` (Partner je Einfall).
pub const DEFAULT_LIMIT: u32 = 3;
/// Vorgabe für `near_threshold` (Kosinus-Ähnlichkeit).
pub const DEFAULT_THRESHOLD: f32 = 0.75;
/// Runde 23a: Obergrenze der Einfälle, deren Nähe berechnet wird. Der Vergleich aller Paare kostet O(n²·d)
/// (n Einfälle, d Dimensionen des Vektors); 1500 Einfälle sind rund 1,1 Millionen Paare. Hat die Karte mehr, zählen
/// nur die ersten [`MAX_NEAR_IDEAS`] nach aufsteigender id, und die Meldung der Karte sagt das. Die Rechnung läuft
/// außerdem in `spawn_blocking`, damit sie keinen Arbeitsfaden der Laufzeit belegt ([`compute`]).
pub const MAX_NEAR_IDEAS: usize = 1500;
/// Satz bei `near = false`.
pub const OFF: &str = "Nähe ausgeschaltet.";

/// Vergleichsschlüssel eines Schlagworts wie im Store (`word_key`): klein, Leerraum zu einem Leerzeichen.
pub fn word_key(word: &str) -> String {
    word.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Vektor auf Länge 1 bringen (Nullvektor → `None`): danach ist die Kosinus-Ähnlichkeit ein Skalarprodukt.
fn unit(vector: &[f32]) -> Option<Vec<f32>> {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    (norm > 0.0 && norm.is_finite()).then(|| vector.iter().map(|x| x / norm).collect())
}

/// Nahe Paare der Einfälle `ideas` (id, Schlagwörter) nach den Regeln im Modulkopf. `vectors` = gespeicherte
/// Vektoren (fehlt einer, zählt für dieses Paar nur die Schlagwort-Nähe).
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

/// Nähe der gezeigten Einfälle des Werks (siehe Modulkopf). `near = false` → `off` ohne Paare.
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
    // Runde 23a: O(n²·d) nicht auf dem async-Faden. Bricht die Aufgabe ab (Panik), bleibt die Karte ohne Nähe und
    // sagt es, statt die ganze Seite scheitern zu lassen.
    let found = tokio::task::spawn_blocking(move || pairs(&list, vectors.as_ref(), limit, threshold)).await;
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
        // Vertrag §2: gleiche Schlagwörter (word_key, also ohne Groß/klein) zählen auch ohne Einbettung.
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
        // Die Schwelle zählt: bei 0.999 bleibt nur, was auch ein Wort teilt (1 und 3 über Wörter).
        let strict = pairs(&ideas, Some(&vectors), 3, 0.999_9);
        assert!(strict.iter().all(|p| p.via == NearVia::Words), "nur noch Wort-Nähe: {strict:?}");
    }

    #[test]
    fn no_idea_gets_more_partners_than_the_limit() {
        // Alle zehn teilen ein Wort: ohne Grenze hätte jeder neun Partner.
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
