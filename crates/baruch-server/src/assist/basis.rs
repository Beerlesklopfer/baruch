// SPDX-License-Identifier: AGPL-3.0-or-later
//! Review basis of a request (Round 30b, chat-loesungen): which revision all personas of a call or RFC
//! review (`basis` = `shown` | `current` | `<n>`) and the header line of the post („geprüft: aktuelle Fassung n“ or
//! „geprüft: Fassung n (Vorschlag von @x)“). The header line comes from the call, never from the model; it ends up
//! in `PostMeta::basis_label`.
//!
//! Rule “heading only” (user 2026-11-09, finding key 126): if the revision that “shown” means is the current one and
//! contains only the heading while a proposal with text exists, all personas review the newest such
//! proposal – otherwise the Lektor would report “section empty” while the others reviewed the proposal. If the human
//! explicitly picks the current revision (`current` or its number), it stays; the header line then carries the
//! note „aktuelle Fassung enthält nur die Überschrift – Vorschlag n prüfen?“.

use baruch_core::{Author, Node, RevisionStatus};
use baruch_store::{StoreError, WorkId};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, REFUSED, RpcError};

/// Requested review basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BasisChoice {
    /// The revision shown (without a value the current one).
    #[default]
    Shown,
    /// The current revision.
    Current,
    /// Revision n.
    Rev(u32),
}

impl BasisChoice {
    /// `shown` | `current` | digits (from 1); otherwise `None`.
    pub fn parse(s: &str) -> Option<BasisChoice> {
        match s.trim() {
            "shown" => Some(BasisChoice::Shown),
            "current" => Some(BasisChoice::Current),
            digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
                digits.parse::<u32>().ok().filter(|n| *n >= 1).map(BasisChoice::Rev)
            }
            _ => None,
        }
    }

    /// Parameter `basis` from RPC or form: missing or empty → `Shown`, text per [`BasisChoice::parse`], number from 1
    /// → `Rev`; otherwise -32602.
    pub fn from_param(params: &serde_json::Value) -> Result<BasisChoice, RpcError> {
        let wrong =
            |shown: &str| RpcError::new(INVALID_PARAMS, tc("assist_msg.basis.invalid", &[("shown", shown.into())]));
        match params.get("basis") {
            None | Some(serde_json::Value::Null) => Ok(BasisChoice::Shown),
            Some(serde_json::Value::String(s)) if s.trim().is_empty() => Ok(BasisChoice::Shown),
            Some(serde_json::Value::String(s)) => BasisChoice::parse(s).ok_or_else(|| wrong(s)),
            Some(serde_json::Value::Number(n)) => n
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n >= 1)
                .map(BasisChoice::Rev)
                .ok_or_else(|| wrong(&n.to_string())),
            Some(other) => Err(wrong(&other.to_string())),
        }
    }
}

/// Chosen review basis: revision, header line, is it a proposal?
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Basis {
    pub rev: u32,
    pub label: String,
    pub proposal: bool,
}

/// Does the text contain only a heading (or nothing at all)?
pub fn heading_only(body: &str) -> bool {
    let mut lines = body.lines().filter(|l| !l.trim().is_empty());
    match lines.next() {
        None => true,
        Some(first) => first.trim_start().starts_with('=') && lines.next().is_none(),
    }
}

/// Newest open proposal of the section with text beyond the heading.
fn proposal_with_text(node: &Node) -> Option<u32> {
    node.revisions
        .iter()
        .filter(|r| r.status == RevisionStatus::Proposed && !heading_only(&r.body))
        .map(|r| r.number)
        .max()
}

/// Header line for revision `rev` of the section.
fn label_of(node: &Node, rev: u32) -> (String, bool) {
    let Some(revision) = node.revision(rev) else { return (format!("geprüft: Fassung {rev}"), false) };
    let proposal = revision.status == RevisionStatus::Proposed;
    let label = match (&revision.author, proposal) {
        (Author::Assistant { persona, .. }, true) => format!("geprüft: Fassung {rev} (Vorschlag von {persona})"),
        (_, true) => format!("geprüft: Fassung {rev} (Vorschlag)"),
        _ if node.current_number() == Some(rev) => format!("geprüft: aktuelle Fassung {rev}"),
        _ => format!("geprüft: Fassung {rev}"),
    };
    (label, proposal)
}

/// Choose the review basis: `Shown` → shown (`shown`) or current revision, `Current` → current, `Rev(n)` → n;
/// plus the rule “heading only” (module doc). Unknown revision → `UnknownRevision` (INVALID_PARAMS); a
/// section without a revision → REFUSED.
pub async fn choose(
    state: &AppState,
    work: WorkId,
    key: i64,
    shown: Option<u32>,
    choice: BasisChoice,
) -> Result<Basis, RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let node = loaded.nodes.get(seq).ok_or(StoreError::GoneNode { work, key })?;
    choose_in(node, seq, shown, choice)
}

/// Basis of a **proposal** (Round 36): the revision `rev` it is based on – without the choice rules of
/// [`choose`] (a proposal is based exactly on the revision the persona saw). The header line starts with
/// „Grundlage:“ instead of „geprüft:“, otherwise as [`choose`] writes it („Grundlage: aktuelle Fassung 2“, „Grundlage:
/// Fassung 3 (Vorschlag von @lektor)“).
pub async fn of_proposal(state: &AppState, work: WorkId, key: i64, rev: u32) -> Result<Basis, RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let node = loaded.nodes.get(seq).ok_or(StoreError::GoneNode { work, key })?;
    let (label, proposal) = label_of(node, rev);
    Ok(Basis { rev, label: label.replacen("geprüft:", "Grundlage:", 1), proposal })
}

/// [`choose`] on an already loaded section (`seq` only for the error message).
pub fn choose_in(node: &Node, seq: usize, shown: Option<u32>, choice: BasisChoice) -> Result<Basis, RpcError> {
    let current = node.current_number().or_else(|| node.revisions.iter().map(|r| r.number).max());
    let wanted = match choice {
        BasisChoice::Shown => shown.or(current),
        BasisChoice::Current => current,
        BasisChoice::Rev(n) => Some(n),
    };
    let Some(mut wanted) = wanted else {
        return Err(RpcError::new(REFUSED, tc("assist_msg.basis.no_revision", &[])));
    };
    let Some(revision) = node.revision(wanted) else {
        let existing = node.revisions.iter().map(|r| r.number).collect();
        return Err(StoreError::UnknownRevision { node: seq, rev: wanted, existing }.into());
    };
    let is_current = node.current_number() == Some(wanted);
    let hint = if is_current && heading_only(&revision.body) { proposal_with_text(node) } else { None };
    let mut note = String::new();
    match (choice, hint) {
        // “shown” means the current revision, which has only the heading: review the proposal with text.
        (BasisChoice::Shown, Some(proposal)) => wanted = proposal,
        (_, Some(proposal)) => {
            note = format!(" (aktuelle Fassung enthält nur die Überschrift – Vorschlag {proposal} prüfen?)");
        }
        _ => {}
    }
    let (label, proposal) = label_of(node, wanted);
    Ok(Basis { rev: wanted, label: format!("{label}{note}"), proposal })
}

#[cfg(test)]
mod tests {
    use super::*;
    use baruch_core::{NodeKind, Revision};

    #[test]
    fn basis_choices_parse_from_form_values() {
        assert_eq!(BasisChoice::parse("shown"), Some(BasisChoice::Shown), "Vorgabe der Formulare");
        assert_eq!(BasisChoice::parse(" current "), Some(BasisChoice::Current), "Leerraum zählt nicht");
        assert_eq!(BasisChoice::parse("7"), Some(BasisChoice::Rev(7)), "Ziffern = Fassung n");
        for bad in ["", "0", "-1", "aktuell", "7a"] {
            assert_eq!(BasisChoice::parse(bad), None, "{bad:?} ist keine Prüfgrundlage");
        }
        let from = |v: serde_json::Value| BasisChoice::from_param(&serde_json::json!({ "basis": v }));
        assert_eq!(from(serde_json::json!(3)).unwrap(), BasisChoice::Rev(3), "RPC darf eine Zahl schicken");
        assert_eq!(from(serde_json::json!("")).unwrap(), BasisChoice::Shown, "leeres Formularfeld = Vorgabe");
        assert_eq!(BasisChoice::from_param(&serde_json::json!({})).unwrap(), BasisChoice::Shown, "fehlt = Vorgabe");
        assert!(from(serde_json::json!(0)).is_err(), "Fassung 0 gibt es nicht");
        assert!(from(serde_json::json!("neu")).is_err(), "unbekanntes Wort wird abgelehnt, nicht still übergangen");
    }

    fn revision(number: u32, body: &str, status: RevisionStatus, author: Author) -> Revision {
        Revision { number, body: body.into(), author, status, note: String::new() }
    }

    fn node(revisions: Vec<Revision>) -> Node {
        let mut node = baruch_core::sample::work().nodes.remove(0);
        node.kind = NodeKind::Section;
        node.revisions = revisions;
        node
    }

    fn lector() -> Author {
        Author::Assistant { persona: "@lektor".into(), model: "m".into() }
    }

    #[test]
    fn a_heading_only_current_version_gives_way_to_a_proposal_with_text() {
        let n = node(vec![
            revision(1, "== Leuchtturm\n", RevisionStatus::Current, Author::Human { name: "Autor".into() }),
            revision(2, "== Leuchtturm\n\nDer Wachturm steht am Kai.\n", RevisionStatus::Proposed, lector()),
        ]);
        let shown = choose_in(&n, 0, None, BasisChoice::Shown).unwrap();
        assert_eq!(
            (shown.rev, shown.label.as_str(), shown.proposal),
            (2, "geprüft: Fassung 2 (Vorschlag von @lektor)", true),
            "nur Überschrift: alle prüfen den Vorschlag (Befund key 126, sonst „Abschnitt leer“)"
        );
        let current = choose_in(&n, 0, None, BasisChoice::Current).unwrap();
        assert_eq!(current.rev, 1, "ausdrücklich „aktuell“ bleibt die aktuelle Fassung");
        assert_eq!(
            current.label,
            "geprüft: aktuelle Fassung 1 (aktuelle Fassung enthält nur die Überschrift – Vorschlag 2 prüfen?)",
            "der Hinweis steht in der Kopfzeile, nicht als Befund"
        );
    }

    #[test]
    fn a_current_version_with_text_stays_and_unknown_numbers_are_refused() {
        let n = node(vec![
            revision(
                1,
                "== Hafen\n\nDie Mole ist lang.\n",
                RevisionStatus::Current,
                Author::Human { name: "Autor".into() },
            ),
            revision(3, "== Hafen\n\nDie Mole ist kurz.\n", RevisionStatus::Proposed, lector()),
        ]);
        let b = choose_in(&n, 0, None, BasisChoice::Shown).unwrap();
        assert_eq!(
            (b.rev, b.label.as_str()),
            (1, "geprüft: aktuelle Fassung 1"),
            "Text vorhanden: die gezeigte bleibt"
        );
        let p = choose_in(&n, 0, Some(3), BasisChoice::Shown).unwrap();
        assert_eq!(p.label, "geprüft: Fassung 3 (Vorschlag von @lektor)", "gezeigter Vorschlag mit Herkunft");
        assert!(choose_in(&n, 0, None, BasisChoice::Rev(9)).is_err(), "Fassung 9 gibt es nicht");
        assert!(heading_only("== T\n\n  \n") && !heading_only("== T\nText") && heading_only(""), "nur Überschrift");
    }
}
