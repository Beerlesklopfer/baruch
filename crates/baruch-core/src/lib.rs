//! Kern von Baruch: Werke, Kapitelbaum, Revisionen je Abschnitt, Personas.
//!
//! Regeln (siehe docs/Plan.md, Revisionsbaum): Je Abschnitt trägt genau eine Revision die Markierung
//! `Current` (die aktuelle Fassung); sie lässt sich jederzeit auf jede andere Fassung umschalten
//! ([`Node::set_current`]), dabei ändert sich kein Text und nichts wird verworfen. Übernehmen eines
//! Vorschlags ([`Node::accept`]) ist ein solches Umschalten, Verwerfen bleibt ein Status für Vorschläge.
//! Revisionen werden über ihre **Nummer** angesprochen, nicht über ihre Stelle in der Liste: Nummern
//! steigen, sind eindeutig und haben nach einem Löschen Lücken (eine Nummer wird nie neu vergeben).
//! Assistenten legen nur `Proposed` an; Ändern, Löschen und Umschalten sind dem Menschen vorbehalten.
//! Lektor nach Einstellung ([`LectorChoice`]): fest eine Persona oder automatisch die aktive Persona mit
//! dem größten Kontext. Versuch seit 2026-11-06: fest `@lektor` (alle Personas auf einem Modell); ist die
//! feste Persona nicht (mehr) da oder inaktiv, gilt die automatische Regel.

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
    /// Datenbank-id des Beitrags (`post.id`); 0 = noch nicht gespeichert (Beispieldaten, neue Beiträge).
    pub id: i64,
    /// Zeitpunkt des Anlegens (ISO 8601, `2026-11-06T10:11:12.123Z`); leer, solange nicht gespeichert.
    pub created: String,
    pub author: String,
    pub body: String,
    pub sources: Vec<String>,
    /// Verweise auf Stellen im eigenen Werk („du hast in 2.1 schon geschrieben …“).
    pub refs: Vec<TextRef>,
}

/// Stelle im Werk: Abschnitt und Revisionsnummer (ab 1), dazu der zitierte Wortlaut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TextRef {
    pub node: usize,
    pub rev: u32,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    /// Stelle in Baumreihenfolge (`seq`, ab 0). Jede Gliederungsänderung vergibt sie neu.
    pub id: usize,
    /// Fester Schlüssel (Datenbank-id des Knotens): bleibt über Verschieben, Umstufen, Anlegen und
    /// Löschen anderer Punkte gleich; Schreibwege nehmen ihn als `key`, damit ein veralteter Stand
    /// nicht in einen fremden Abschnitt schreibt. 0 = nicht aus der Datenbank (Beispieldaten).
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
    /// Stelle der aktuellen Fassung in `revisions` (ab 0).
    pub fn current_index(&self) -> Option<usize> {
        self.revisions.iter().position(|r| r.status == RevisionStatus::Current)
    }

    /// Revision mit der Nummer `number` (nicht der Stelle).
    pub fn revision(&self, number: u32) -> Option<&Revision> {
        self.revisions.iter().find(|r| r.number == number)
    }

    /// Stelle der Revision mit der Nummer `number` in `revisions` (ab 0).
    pub fn position_of(&self, number: u32) -> Option<usize> {
        self.revisions.iter().position(|r| r.number == number)
    }

    /// Nummer der aktuellen Fassung.
    pub fn current_number(&self) -> Option<u32> {
        self.current_index().map(|i| self.revisions[i].number)
    }

    /// Markierung „aktuell“ auf die Revision `number` umschalten: die bisherige `Current` wird
    /// `Superseded`, das Ziel `Current`. Jeder Status außer `Current` ist als Ziel erlaubt; kein Text
    /// ändert sich. Unbekannte Nummer oder schon aktuell → `false`, nichts geändert.
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

    /// Vorschlag an der Stelle `index` übernehmen (nur `Proposed`): Umschalten mit
    /// [`Node::set_current`], die bisherige `Current` wird `Superseded`.
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

    /// Eigene Bearbeitung speichern: neue Revision vom Menschen, sofort `Current`, mit der Nummer
    /// höchste + 1 (auch bei Lücken).
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

/// Wahl des Lektors (Runde 10, `store_meta` Schlüssel `lector`: `auto` oder ein Handle).
///
/// `Fixed(handle)`: diese Persona, solange es sie gibt und sie aktiv ist, sonst die Regel von `Auto`.
/// `Auto`: die aktive Persona mit dem größten Kontext, bei Gleichstand die erste (Reihenfolge der Personas).
/// Vorgabe des Versuchs seit 2026-11-06: fest `@lektor`.
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

/// Vorgabe der Lektor-Wahl im Versuch seit 2026-11-06.
pub const DEFAULT_LECTOR: &str = "@lektor";

impl LectorChoice {
    /// `"fixed"` oder `"auto"`.
    pub fn mode(&self) -> &'static str {
        match self {
            LectorChoice::Auto => "auto",
            LectorChoice::Fixed(_) => "fixed",
        }
    }

    /// Fester Handle, falls gewählt.
    pub fn handle(&self) -> Option<&str> {
        match self {
            LectorChoice::Auto => None,
            LectorChoice::Fixed(handle) => Some(handle),
        }
    }

    /// Lektor unter Personas mit Handle, Kontextlänge und aktiv (in ihrer Reihenfolge `position`, `id`):
    /// Index in `personas`. Gemeinsame Regel für [`Work::lector`] und die Tabelle `persona` im Server.
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
    /// Wahl des Lektors (Runde 10); Vorgabe fest `@lektor`.
    pub lector_choice: LectorChoice,
}

impl Work {
    pub fn children(&self, parent: Option<usize>) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.parent == parent)
    }

    /// Lektor nach [`Work::lector_choice`]: fest gewählt (wenn vorhanden und aktiv), sonst die aktive
    /// Persona mit dem größten Kontext (bei Gleichstand die erste).
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

/// Zeilenweiser Vergleich (Zeilen-Diff der Vorschläge im Lesefluss).
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
        node.revisions.remove(1); // Lücke: 1, 3
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
