//! Core of Baruch: works, chapter tree, revisions per section, personas.
//!
//! Rules (see docs/Plan.md, revision tree): per section exactly one revision carries the mark `Current` (the
//! current revision); it can be switched to any other revision at any time ([`Node::set_current`]), no text changes
//! and nothing is discarded. Accepting a proposal ([`Node::accept`]) is such a switch, rejecting stays a status for
//! proposals. Revisions are addressed by their **number**, not by their position in the list: numbers rise, are
//! unique and have gaps after a deletion (a number is never assigned again). Assistants only create `Proposed`;
//! changing, deleting and switching are reserved for the human. Lector by setting ([`LectorChoice`]): a fixed
//! persona or automatically the active persona with the largest context. Trial since 2026-11-06: fixed `@lektor`
//! (all personas on one model); if the fixed persona is not there (any more) or inactive, the automatic rule
//! applies.

pub mod book_text;
pub mod language;
pub mod revision_fold;
pub mod revision_policy;
pub mod sample;

use serde::Serialize;
use similar::{ChangeTag, TextDiff};

pub use language::WorkLanguage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    Book,
    Sermon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Part,
    Chapter,
    Section,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionStatus {
    Proposed,
    Current,
    Rejected,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Author {
    Human { name: String },
    Assistant { persona: String, model: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Revision {
    pub number: u32,
    pub body: String,
    pub author: Author,
    pub status: RevisionStatus,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Post {
    /// Database id of the post (`post.id`); 0 = not stored yet (sample data, new posts).
    pub id: i64,
    /// Time of creation (ISO 8601, `2026-11-06T10:11:12.123Z`); empty as long as it is not stored.
    pub created: String,
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    /// References to places in the own work („du hast in 2.1 schon geschrieben …“).
    pub refs: Vec<TextRef>,
}

/// Place in the work: section and revision number (from 1), plus the quoted wording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextRef {
    pub node: usize,
    pub rev: u32,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    /// Position in tree order (`seq`, from 0). Every change of the outline assigns it anew.
    pub id: usize,
    /// Fixed key (database id of the node): stays the same when other points are moved, changed in level, created
    /// or deleted; write paths take it as `key`, so that a stale state does not write into a foreign section. 0 =
    /// not from the database (sample data).
    pub key: i64,
    pub parent: Option<usize>,
    pub number: String,
    pub title: String,
    pub kind: NodeKind,
    pub revisions: Vec<Revision>,
    pub posts: Vec<Post>,
    pub findings: u32,
}

impl Node {
    /// Position of the current revision in `revisions` (from 0).
    pub fn current_index(&self) -> Option<usize> {
        self.revisions.iter().position(|r| r.status == RevisionStatus::Current)
    }

    /// Revision with the number `number` (not the position).
    pub fn revision(&self, number: u32) -> Option<&Revision> {
        self.revisions.iter().find(|r| r.number == number)
    }

    /// Position of the revision with the number `number` in `revisions` (from 0).
    pub fn position_of(&self, number: u32) -> Option<usize> {
        self.revisions.iter().position(|r| r.number == number)
    }

    /// Number of the current revision.
    pub fn current_number(&self) -> Option<u32> {
        self.current_index().map(|i| self.revisions[i].number)
    }

    /// Switch the mark "current" to the revision `number`: the previous `Current` becomes `Superseded`, the target
    /// `Current`. Every status except `Current` is allowed as target; no text changes. Unknown number or already
    /// current → `false`, nothing changed.
    pub fn set_current(&mut self, number: u32) -> bool {
        let Some(target) = self.position_of(number) else { return false };
        if self.revisions[target].status == RevisionStatus::Current {
            return false;
        }
        for rev in &mut self.revisions {
            if rev.status == RevisionStatus::Current {
                rev.status = RevisionStatus::Superseded;
            }
        }
        self.revisions[target].status = RevisionStatus::Current;
        true
    }

    pub fn open_proposals(&self) -> usize {
        self.revisions.iter().filter(|r| r.status == RevisionStatus::Proposed).count()
    }

    /// Accept the proposal at position `index` (only `Proposed`): a switch with [`Node::set_current`], the previous
    /// `Current` becomes `Superseded`.
    pub fn accept(&mut self, index: usize) -> bool {
        match self.revisions.get(index) {
            Some(rev) if rev.status == RevisionStatus::Proposed => {
                let number = rev.number;
                self.set_current(number)
            }
            _ => false,
        }
    }

    pub fn reject(&mut self, index: usize) -> bool {
        match self.revisions.get_mut(index) {
            Some(rev) if rev.status == RevisionStatus::Proposed => {
                rev.status = RevisionStatus::Rejected;
                true
            }
            _ => false,
        }
    }

    /// Save an own edit: new revision by the human, `Current` at once, with the number highest + 1 (also with gaps).
    pub fn save_human(&mut self, body: String, author: &str) {
        for rev in &mut self.revisions {
            if rev.status == RevisionStatus::Current {
                rev.status = RevisionStatus::Superseded;
            }
        }
        let number = self.revisions.iter().map(|r| r.number).max().unwrap_or(0) + 1;
        self.revisions.push(Revision {
            number,
            body,
            author: Author::Human { name: author.to_owned() },
            status: RevisionStatus::Current,
            note: String::new(),
        });
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Persona {
    pub handle: String,
    pub name: String,
    pub role: String,
    pub model: String,
    pub context_length: u32,
    pub active: bool,
}

/// Choice of the lector (Round 10, `store_meta` key `lector`: `auto` or a handle).
///
/// `Fixed(handle)`: this persona, as long as it exists and is active, otherwise the rule of `Auto`.
/// `Auto`: the active persona with the largest context, on a tie the first one (order of the personas).
/// Default of the trial since 2026-11-06: fixed `@lektor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", content = "handle", rename_all = "snake_case")]
pub enum LectorChoice {
    Auto,
    Fixed(String),
}

impl Default for LectorChoice {
    fn default() -> Self {
        LectorChoice::Fixed(DEFAULT_LECTOR.to_owned())
    }
}

/// Default of the lector choice in the trial since 2026-11-06.
pub const DEFAULT_LECTOR: &str = "@lektor";

impl LectorChoice {
    /// `"fixed"` or `"auto"`.
    pub fn mode(&self) -> &'static str {
        match self {
            LectorChoice::Auto => "auto",
            LectorChoice::Fixed(_) => "fixed",
        }
    }

    /// Fixed handle, if chosen.
    pub fn handle(&self) -> Option<&str> {
        match self {
            LectorChoice::Auto => None,
            LectorChoice::Fixed(handle) => Some(handle),
        }
    }

    /// Lector among personas with handle, context length and active (in their order `position`, `id`): index in
    /// `personas`. Shared rule for [`Work::lector`] and the table `persona` in the server.
    pub fn pick<T>(
        &self,
        personas: &[T],
        handle: impl Fn(&T) -> &str,
        context: impl Fn(&T) -> u32,
        active: impl Fn(&T) -> bool,
    ) -> Option<usize> {
        if let LectorChoice::Fixed(wanted) = self {
            let wanted = wanted.trim();
            let found = personas.iter().position(|p| {
                let h = handle(p);
                h == wanted || h.trim_start_matches('@') == wanted.trim_start_matches('@')
            });
            if let Some(index) = found.filter(|&i| active(&personas[i])) {
                return Some(index);
            }
        }
        let mut best: Option<usize> = None;
        for (index, p) in personas.iter().enumerate() {
            if !active(p) {
                continue;
            }
            match best {
                Some(b) if context(&personas[b]) >= context(p) => {}
                _ => best = Some(index),
            }
        }
        best
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Work {
    pub kind: WorkKind,
    pub title: String,
    pub author: String,
    pub nodes: Vec<Node>,
    pub personas: Vec<Persona>,
    /// Choice of the lector (Round 10); default fixed `@lektor`.
    pub lector_choice: LectorChoice,
}

impl Work {
    pub fn children(&self, parent: Option<usize>) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.parent == parent)
    }

    /// Lector by [`Work::lector_choice`]: the fixed choice (if present and active), otherwise the active persona
    /// with the largest context (on a tie the first one).
    pub fn lector(&self) -> Option<&Persona> {
        let index =
            self.lector_choice.pick(&self.personas, |p| p.handle.as_str(), |p| p.context_length, |p| p.active)?;
        self.personas.get(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Same,
    Added,
    Removed,
}

/// Line-wise comparison (line diff of the proposals in the reading flow).
pub fn line_diff(old: &str, new: &str) -> Vec<(DiffKind, String)> {
    TextDiff::from_lines(old, new)
        .iter_all_changes()
        .map(|change| {
            let kind = match change.tag() {
                ChangeTag::Equal => DiffKind::Same,
                ChangeTag::Insert => DiffKind::Added,
                ChangeTag::Delete => DiffKind::Removed,
            };
            (kind, change.value().trim_end_matches('\n').to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_keeps_exactly_one_current() {
        let mut work = sample::work();
        let node = work.nodes.iter_mut().find(|n| n.open_proposals() > 0).unwrap();
        let proposal = node.revisions.iter().position(|r| r.status == RevisionStatus::Proposed).unwrap();
        assert!(node.accept(proposal));
        let currents = node.revisions.iter().filter(|r| r.status == RevisionStatus::Current);
        assert_eq!(currents.count(), 1);
        assert_eq!(node.current_index(), Some(proposal));
        assert!(!node.accept(proposal), "nur Vorschläge lassen sich übernehmen");
    }

    #[test]
    fn reject_does_not_touch_current() {
        let mut work = sample::work();
        let node = work.nodes.iter_mut().find(|n| n.open_proposals() > 0).unwrap();
        let before = node.current_index();
        let proposal = node.revisions.iter().position(|r| r.status == RevisionStatus::Proposed).unwrap();
        assert!(node.reject(proposal));
        assert_eq!(node.current_index(), before);
    }

    #[test]
    fn switching_to_any_other_version_keeps_exactly_one_current_and_changes_no_text() {
        let mut work = sample::work();
        let node = &mut work.nodes[2]; // Einleitung: 1 superseded, 2 current, 3 proposed
        let bodies: Vec<String> = node.revisions.iter().map(|r| r.body.clone()).collect();
        assert!(node.set_current(1), "eine ersetzte Fassung darf wieder aktuell werden");
        assert_eq!(node.current_number(), Some(1));
        assert_eq!(node.revision(2).map(|r| r.status), Some(RevisionStatus::Superseded), "alte aktuelle abgelöst");
        assert!(!node.set_current(1), "die schon aktuelle Fassung ist kein Ziel");
        assert!(!node.set_current(9), "unbekannte Nummer");
        node.revisions[2].status = RevisionStatus::Rejected;
        assert!(node.set_current(3), "auch ein verworfener Vorschlag darf aktuell werden");
        let currents = node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).count();
        assert_eq!(currents, 1, "genau eine aktuelle Fassung");
        let after: Vec<String> = node.revisions.iter().map(|r| r.body.clone()).collect();
        assert_eq!(after, bodies, "Umschalten ändert keinen Text");
    }

    #[test]
    fn numbers_not_positions_address_revisions_after_a_gap() {
        let mut work = sample::work();
        let node = &mut work.nodes[2];
        node.revisions.remove(1); // gap: 1, 3
        node.revisions[0].status = RevisionStatus::Current;
        assert_eq!(node.position_of(3), Some(1), "Nummer 3 steht jetzt an Stelle 1");
        assert_eq!(node.revision(2).map(|r| r.number), None, "gelöschte Nummer gibt es nicht mehr");
        node.save_human("neu\n".into(), "Autor");
        assert_eq!(node.current_number(), Some(4), "neue Nummer = höchste + 1, nicht Anzahl + 1");
        assert!(node.accept(1), "Vorschlag 3 an Stelle 1 lässt sich übernehmen");
        assert_eq!(node.current_number(), Some(3));
    }

    #[test]
    fn lector_is_largest_active_context() {
        let mut work = sample::work();
        let max = work.personas.iter().filter(|p| p.active).map(|p| p.context_length).max();
        assert_eq!(work.lector().map(|p| p.context_length), max);
        let handle = work.lector().unwrap().handle.clone();
        work.personas.iter_mut().find(|p| p.handle == handle).unwrap().active = false;
        assert_ne!(work.lector().unwrap().handle, handle, "Lektor wechselt mit dem Modell");
    }

    #[test]
    fn a_fixed_lector_wins_until_it_is_inactive_then_the_largest_context_rules() {
        let mut work = sample::work();
        assert_eq!(work.lector_choice, LectorChoice::Fixed("@lektor".into()), "Vorgabe des Versuchs: fest @lektor");
        work.lector_choice = LectorChoice::Fixed("hoerer".into());
        assert_eq!(work.lector().unwrap().handle, "@hoerer", "fest gewählt (auch ohne @), trotz kleinem Kontext");
        work.personas.iter_mut().find(|p| p.handle == "@hoerer").unwrap().active = false;
        assert_eq!(work.lector().unwrap().handle, "@lektor", "inaktive feste Persona: Regel automatisch");
        work.lector_choice = LectorChoice::Fixed("@gibtsnicht".into());
        assert_eq!(work.lector().unwrap().handle, "@lektor", "unbekannte feste Persona: Regel automatisch");
    }

    #[test]
    fn automatic_lector_takes_the_first_on_a_tie() {
        let mut work = sample::work();
        work.lector_choice = LectorChoice::Auto;
        for p in &mut work.personas {
            p.context_length = 4096;
        }
        assert_eq!(work.lector().unwrap().handle, work.personas[0].handle, "Gleichstand: die erste Persona");
        work.personas[0].active = false;
        assert_eq!(work.lector().unwrap().handle, work.personas[1].handle, "erste inaktiv: die nächste");
        for p in &mut work.personas {
            p.active = false;
        }
        assert!(work.lector().is_none(), "ohne aktive Persona kein Lektor");
        assert_eq!((LectorChoice::Auto.mode(), LectorChoice::default().mode()), ("auto", "fixed"), "Modus als Wort");
    }

    #[test]
    fn diff_marks_changed_lines() {
        let diff = line_diff("a\nb\n", "a\nc\n");
        assert!(diff.contains(&(DiffKind::Removed, "b".into())));
        assert!(diff.contains(&(DiffKind::Added, "c".into())));
    }
}
