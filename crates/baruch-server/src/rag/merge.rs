// SPDX-License-Identifier: AGPL-3.0-or-later
//! Treffer mehrerer Wege zusammenführen (Vertrag Runde 12, §1): Reciprocal Rank Fusion mit
//! [`RRF_K`] = 60, Doppelte über [`Hit::identity`] (Dokument + Stelle, beschnitten, klein) auch über
//! Wege hinweg. Rein und deterministisch.
//!
//! 1. Je Weg: nach `score` absteigend (Gleichstand: `document`, dann `section` aufsteigend), auf
//!    `limit` gekürzt, Rang 1..n. Doppelte innerhalb eines Wegs zählen mit ihrem besten Rang.
//! 2. Je Identität: `fused` = Σ 1/(k + Rang) über die findenden Wege; Vertreter = der Treffer mit dem
//!    besten Rang, bei Gleichstand der frühere Weg in [`Way::ALL`].
//! 3. Reihenfolge: `fused` absteigend, dann bester Rang, dann Weg-Reihenfolge, dann `document`,
//!    `section`; die ersten `limit`, `rank` 1..n, `found_by` in [`Way::ALL`]-Reihenfolge.

use std::cmp::Ordering;
use std::collections::HashMap;

use baruch_rag::{Hit, MergedHit, RRF_K, Way, WayRank};

fn way_position(way: Way) -> usize {
    Way::ALL.iter().position(|w| *w == way).unwrap_or(Way::ALL.len())
}

fn by_score(a: &Hit, b: &Hit) -> Ordering {
    b.score
        .total_cmp(&a.score)
        .then_with(|| a.document.cmp(&b.document))
        .then_with(|| a.section.as_deref().unwrap_or("").cmp(b.section.as_deref().unwrap_or("")))
}

struct Group {
    representative: Hit,
    /// Rang des Vertreters.
    best_rank: usize,
    found_by: Vec<WayRank>,
}

/// Treffer der Wege zu einer Liste zusammenführen (siehe Modulkopf).
pub fn merge(per_way: Vec<(Way, Vec<Hit>)>, limit: usize) -> Vec<MergedHit> {
    let mut groups: Vec<Group> = Vec::new();
    let mut by_identity: HashMap<(String, String), usize> = HashMap::new();
    let mut per_way = per_way;
    // Wege in fester Reihenfolge abarbeiten, damit „früherer Weg“ eindeutig ist.
    per_way.sort_by_key(|(way, _)| way_position(*way));
    for (way, mut hits) in per_way {
        hits.sort_by(by_score);
        hits.truncate(limit);
        for (i, hit) in hits.into_iter().enumerate() {
            let rank = i + 1;
            let identity = hit.identity();
            match by_identity.get(&identity) {
                Some(&g) => {
                    let group = &mut groups[g];
                    if group.found_by.iter().any(|r| r.way == way) {
                        // Doppelter Treffer im selben Weg: nur der bessere Rang zählt (er kam zuerst).
                        continue;
                    }
                    group.found_by.push(WayRank { way, rank, score: hit.score });
                    let better = rank < group.best_rank
                        || (rank == group.best_rank && way_position(way) < way_position(group.representative.way));
                    if better {
                        group.best_rank = rank;
                        group.representative = hit;
                    }
                }
                None => {
                    by_identity.insert(identity, groups.len());
                    groups.push(Group {
                        found_by: vec![WayRank { way, rank, score: hit.score }],
                        best_rank: rank,
                        representative: hit,
                    });
                }
            }
        }
    }
    let mut merged: Vec<MergedHit> = groups
        .into_iter()
        .map(|mut g| {
            g.found_by.sort_by_key(|r| way_position(r.way));
            let fused = g.found_by.iter().map(|r| 1.0 / (RRF_K + r.rank as f64)).sum();
            // Bester Rang vorläufig in `rank` (für die Sortierung), danach durch den Platz ersetzt.
            MergedHit { hit: g.representative, rank: g.best_rank, fused, found_by: g.found_by }
        })
        .collect();
    merged.sort_by(|a, b| {
        b.fused
            .total_cmp(&a.fused)
            .then_with(|| a.rank.cmp(&b.rank))
            .then_with(|| way_position(a.hit.way).cmp(&way_position(b.hit.way)))
            .then_with(|| a.hit.document.cmp(&b.hit.document))
            .then_with(|| a.hit.section.as_deref().unwrap_or("").cmp(b.hit.section.as_deref().unwrap_or("")))
    });
    merged.truncate(limit);
    for (i, m) in merged.iter_mut().enumerate() {
        m.rank = i + 1;
    }
    merged
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn hit(way: Way, document: &str, section: Option<&str>, score: f32) -> Hit {
        Hit {
            way,
            collection: "c".into(),
            document: document.into(),
            section: section.map(str::to_owned),
            title: format!("{way}:{document}"),
            snippet: String::new(),
            score,
            url: None,
            meta: json!({}),
        }
    }

    fn order(merged: &[MergedHit]) -> Vec<String> {
        merged.iter().map(|m| m.hit.document.clone()).collect()
    }

    #[test]
    fn reciprocal_rank_fusion_rewards_hits_found_by_several_ways() {
        let lance = vec![hit(Way::Lancedb, "a", None, 0.9), hit(Way::Lancedb, "b", None, 0.8)];
        let remote = vec![hit(Way::Multigpt, "c", None, 0.99), hit(Way::Multigpt, "B", None, 0.5)];
        let merged = merge(vec![(Way::Multigpt, remote), (Way::Lancedb, lance)], 10);
        assert_eq!(order(&merged), vec!["b", "a", "c"], "b von zwei Wegen (Rang 2+2) schlägt zwei Erste");
        let fused_b = 2.0 / (RRF_K + 2.0);
        assert!((merged[0].fused - fused_b).abs() < 1e-12, "fused = 1/62 + 1/62, nicht {}", merged[0].fused);
        assert!((merged[1].fused - 1.0 / 61.0).abs() < 1e-12, "a: 1/61");
        assert_eq!(merged.iter().map(|m| m.rank).collect::<Vec<_>>(), vec![1, 2, 3], "Rang 1..n");
        assert_eq!(
            merged[0].found_by.iter().map(|r| (r.way, r.rank)).collect::<Vec<_>>(),
            vec![(Way::Lancedb, 2), (Way::Multigpt, 2)],
            "found_by in Way::ALL-Reihenfolge, je mit Rang"
        );
        assert_eq!(merged[0].hit.way, Way::Lancedb, "Gleichstand im Rang: der frühere Weg vertritt");
        assert_eq!(merged[1].hit.document, "a", "a vor c: gleicher fused, gleicher Rang, LanceDB vor MultiGPT");
    }

    #[test]
    fn duplicates_are_recognised_across_ways_regardless_of_case_and_margins() {
        let pg = vec![hit(Way::Pgvector, " Doc-7 ", Some("S1 "), 0.7)];
        let mg = vec![hit(Way::Multigpt, "doc-7", Some(" s1"), 0.9), hit(Way::Multigpt, "doc-7", Some("s2"), 0.2)];
        let merged = merge(vec![(Way::Pgvector, pg), (Way::Multigpt, mg)], 10);
        assert_eq!(merged.len(), 2, "gleiche Stelle eins, andere Stelle eigener Treffer: {merged:?}");
        assert_eq!(merged[0].found_by.len(), 2, "beide Wege fanden doc-7/s1");
        assert_eq!(merged[0].hit.way, Way::Pgvector, "beide Rang 1: pgvector kommt in Way::ALL vor MultiGPT");
    }

    #[test]
    fn the_representative_is_the_hit_with_the_best_rank() {
        let lance = vec![hit(Way::Lancedb, "x", None, 0.9), hit(Way::Lancedb, "d", None, 0.4)];
        let mg = vec![hit(Way::Multigpt, "d", None, 0.95)];
        let merged = merge(vec![(Way::Lancedb, lance), (Way::Multigpt, mg)], 10);
        let d = merged.iter().find(|m| m.hit.document == "d").unwrap();
        assert_eq!(d.hit.way, Way::Multigpt, "Rang 1 bei MultiGPT schlägt Rang 2 bei LanceDB");
        assert_eq!(d.hit.score, 0.95, "Vertreter behält seinen eigenen Score");
    }

    #[test]
    fn within_a_way_ties_break_by_document_and_section_and_the_limit_applies_per_way_and_overall() {
        let lance = vec![
            hit(Way::Lancedb, "z", None, 0.5),
            hit(Way::Lancedb, "a", Some("2"), 0.5),
            hit(Way::Lancedb, "a", Some("1"), 0.5),
            hit(Way::Lancedb, "m", None, 0.1),
        ];
        let merged = merge(vec![(Way::Lancedb, lance.clone())], 2);
        assert_eq!(
            merged.iter().map(|m| (m.hit.document.as_str(), m.hit.section.as_deref())).collect::<Vec<_>>(),
            vec![("a", Some("1")), ("a", Some("2"))],
            "Gleichstand: document, dann section aufsteigend; limit 2"
        );
        let other = vec![hit(Way::Pgvector, "m", None, 0.9)];
        let merged = merge(vec![(Way::Lancedb, lance), (Way::Pgvector, other)], 3);
        assert_eq!(merged.len(), 3, "insgesamt höchstens limit");
        let m = merged.iter().find(|h| h.hit.document == "m").unwrap();
        assert_eq!(m.found_by.len(), 1, "LanceDB-Rang 4 lag hinter dem limit und zählt nicht: {m:?}");
    }

    #[test]
    fn merging_is_deterministic_and_handles_empty_input() {
        assert!(merge(vec![], 10).is_empty(), "keine Wege, keine Treffer");
        assert!(merge(vec![(Way::Lancedb, vec![])], 10).is_empty(), "leerer Weg");
        let input = vec![
            (Way::Multigpt, vec![hit(Way::Multigpt, "p", None, 0.3), hit(Way::Multigpt, "q", None, 0.3)]),
            (Way::Lancedb, vec![hit(Way::Lancedb, "q", None, 0.8), hit(Way::Lancedb, "r", None, 0.2)]),
            (Way::Pgvector, vec![hit(Way::Pgvector, "r", None, 0.6)]),
        ];
        let first = merge(input.clone(), 10);
        let mut reversed = input;
        reversed.reverse();
        assert_eq!(first, merge(reversed, 10), "Reihenfolge der Eingabe ändert nichts");
        assert_eq!(order(&first), vec!["q", "r", "p"], "q (1+2), r (2+1), p (1)");
    }

    #[test]
    fn a_duplicate_within_one_way_counts_once_with_its_better_rank() {
        let lance = vec![hit(Way::Lancedb, "a", None, 0.9), hit(Way::Lancedb, "A", None, 0.8)];
        let merged = merge(vec![(Way::Lancedb, lance)], 10);
        assert_eq!(merged.len(), 1, "zweimal dieselbe Stelle: ein Treffer");
        assert_eq!(merged[0].found_by, vec![WayRank { way: Way::Lancedb, rank: 1, score: 0.9 }], "nur Rang 1 zählt");
    }
}
