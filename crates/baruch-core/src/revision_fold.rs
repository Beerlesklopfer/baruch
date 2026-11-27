// SPDX-License-Identifier: AGPL-3.0-or-later
//! Hiding older revisions (user 2026-11-24, release 0.9.0): per section one mark "up to here".
//!
//! The mark is the number of a revision. Hidden are **superseded and rejected** revisions with a smaller number.
//! Never hidden are
//! - the current revision (it is the basis of the book),
//! - open proposals (they wait for a decision of the human, no matter whether older or newer than the mark).
//!
//! If the mark lies above the current revision (for example because a switch to an older revision happened later),
//! it only applies up to below the current one ([`cutoff`]); it cannot be set anew there ([`offered`], refused in
//! the store). Nothing is deleted: the mark is a view mark.

use crate::{Node, Revision, RevisionStatus};

/// Effective limit: the mark, but at most the number of the current revision.
pub fn cutoff(node: &Node, mark: Option<u32>) -> Option<u32> {
    let mark = mark?;
    Some(match node.current_number() {
        Some(current) => mark.min(current),
        None => mark,
    })
}

/// Can this revision be hidden at all (superseded or rejected)?
pub fn hideable(revision: &Revision) -> bool {
    matches!(revision.status, RevisionStatus::Superseded | RevisionStatus::Rejected)
}

/// Is the revision hidden under the mark?
pub fn is_hidden(node: &Node, mark: Option<u32>, revision: &Revision) -> bool {
    cutoff(node, mark).is_some_and(|limit| revision.number < limit && hideable(revision))
}

/// Number of hidden revisions of the section.
pub fn hidden_count(node: &Node, mark: Option<u32>) -> usize {
    node.revisions.iter().filter(|r| is_hidden(node, mark, r)).count()
}

/// Is "hide up to here" offered at revision `number`? Only if it does not lie above the current one, the mark does
/// not already stand there and there would be something to hide below it.
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
        // 1 superseded, 2 rejected, 3 open proposal, 4 superseded, 5 current, 6 proposal.
        let n = node(&[Superseded, Rejected, Proposed, Superseded, Current, Proposed]);
        let hidden: Vec<u32> = n.revisions.iter().filter(|r| is_hidden(&n, Some(5), r)).map(|r| r.number).collect();
        // The open proposal 3 stays (waits for a decision), 5 is the mark itself, 6 lies above it.
        assert_eq!(hidden, vec![1, 2, 4]);
        assert_eq!(hidden_count(&n, Some(5)), 3);
        assert_eq!(hidden_count(&n, None), 0, "ohne Marke nichts ausgeblendet");
    }

    #[test]
    fn a_mark_above_the_current_revision_stops_below_it() {
        // Current is 2; 3 and 4 are superseded (switched back later). Mark 5 only applies up to below 2.
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
