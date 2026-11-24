// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ältere Fassungen ausblenden (Nutzer 2026-11-24, Release 0.9.0): je Abschnitt eine Marke „bis hierhin“.
//!
//! Die Marke ist die Nummer einer Fassung. Ausgeblendet werden **ersetzte und verworfene** Fassungen mit kleinerer
//! Nummer. Nie ausgeblendet werden
//! - die aktuelle Fassung (sie ist die Grundlage des Buchs),
//! - offene Vorschläge (sie warten auf eine Entscheidung des Menschen, egal ob älter oder neuer als die Marke).
//!
//! Liegt die Marke über der aktuellen Fassung (etwa weil später auf eine ältere Fassung umgeschaltet wurde), gilt sie
//! nur bis unter die aktuelle ([`cutoff`]); neu setzen lässt sie sich dort nicht ([`offered`], im Store abgelehnt).
//! Nichts wird gelöscht: die Marke ist eine Ansichtsmarke.

use crate::{Node, Revision, RevisionStatus};

/// Wirksame Grenze: die Marke, höchstens aber die Nummer der aktuellen Fassung.
pub fn cutoff(node: &Node, mark: Option<u32>) -> Option<u32> {
    let mark = mark?;
    Some(match node.current_number() {
        Some(current) => mark.min(current),
        None => mark,
    })
}

/// Lässt sich diese Fassung überhaupt ausblenden (ersetzt oder verworfen)?
pub fn hideable(revision: &Revision) -> bool {
    matches!(revision.status, RevisionStatus::Superseded | RevisionStatus::Rejected)
}

/// Ist die Fassung unter der Marke ausgeblendet?
pub fn is_hidden(node: &Node, mark: Option<u32>, revision: &Revision) -> bool {
    cutoff(node, mark).is_some_and(|limit| revision.number < limit && hideable(revision))
}

/// Zahl der ausgeblendeten Fassungen des Abschnitts.
pub fn hidden_count(node: &Node, mark: Option<u32>) -> usize {
    node.revisions.iter().filter(|r| is_hidden(node, mark, r)).count()
}

/// Wird „bis hierhin ausblenden“ an Fassung `number` angeboten? Nur, wenn sie nicht über der aktuellen liegt, die
/// Marke nicht schon dort steht und darunter etwas auszublenden wäre.
pub fn offered(node: &Node, mark: Option<u32>, number: u32) -> bool {
    if node.current_number().is_some_and(|current| number > current) {
        return false;
    }
    if cutoff(node, mark) == Some(number) {
        return false;
    }
    node.revisions.iter().any(|r| r.number < number && hideable(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Author, NodeKind};

    fn node(statuses: &[RevisionStatus]) -> Node {
        let mut node = Node {
            id: 0,
            key: 1,
            parent: None,
            number: String::new(),
            title: "Probe".into(),
            kind: NodeKind::Section,
            revisions: Vec::new(),
            posts: Vec::new(),
            findings: 0,
        };
        for (i, status) in statuses.iter().enumerate() {
            node.revisions.push(Revision {
                number: u32::try_from(i + 1).unwrap(),
                body: format!("Fassung {}", i + 1),
                author: Author::Human { name: "Mensch".into() },
                status: *status,
                note: String::new(),
            });
        }
        node
    }

    use RevisionStatus::{Current, Proposed, Rejected, Superseded};

    #[test]
    fn hides_only_superseded_and_rejected_below_the_mark() {
        // 1 ersetzt, 2 verworfen, 3 offener Vorschlag, 4 ersetzt, 5 aktuell, 6 Vorschlag.
        let n = node(&[Superseded, Rejected, Proposed, Superseded, Current, Proposed]);
        let hidden: Vec<u32> = n.revisions.iter().filter(|r| is_hidden(&n, Some(5), r)).map(|r| r.number).collect();
        // Der offene Vorschlag 3 bleibt (wartet auf Entscheidung), 5 ist die Marke selbst, 6 liegt darüber.
        assert_eq!(hidden, vec![1, 2, 4]);
        assert_eq!(hidden_count(&n, Some(5)), 3);
        assert_eq!(hidden_count(&n, None), 0, "ohne Marke nichts ausgeblendet");
    }

    #[test]
    fn a_mark_above_the_current_revision_stops_below_it() {
        // Aktuell ist 2; 3 und 4 sind ersetzt (später zurückgeschaltet). Marke 5 gilt nur bis unter 2.
        let n = node(&[Superseded, Current, Superseded, Superseded, Superseded]);
        assert_eq!(cutoff(&n, Some(5)), Some(2));
        let hidden: Vec<u32> = n.revisions.iter().filter(|r| is_hidden(&n, Some(5), r)).map(|r| r.number).collect();
        assert_eq!(hidden, vec![1], "3 und 4 liegen über der aktuellen Fassung und bleiben sichtbar");
        assert!(!is_hidden(&n, Some(5), &n.revisions[1]), "die aktuelle Fassung nie");
        assert!(!offered(&n, None, 4), "über der aktuellen Fassung nicht angeboten");
        assert!(offered(&n, None, 2), "an der aktuellen Fassung angeboten (darunter liegt 1)");
        assert!(!offered(&n, Some(2), 2), "nicht, wenn die Marke schon dort steht");
        assert!(!offered(&n, None, 1), "an der ältesten nicht: darunter ist nichts");
    }
}
