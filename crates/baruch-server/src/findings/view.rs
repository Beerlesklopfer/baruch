// SPDX-License-Identifier: AGPL-3.0-or-later
//! Befunde für die Arbeitsseite (Runde 28, befund-uebersicht): offene Befunde je Abschnitt (Abzeichen in Gliederung
//! und Editorkopf, aufsummiert für Kapitel und Teile), die Befunde für das Akkordeon „Befunde“ und den Fassungsbaum
//! („setzt Befund x, y um“).
//!
//! Zählregel (Vertrag R28 §1.5): offen = `status 'open'`, nicht angehängt, kein Lob, mit Abschnitt – genau
//! [`baruch_store::Store::finding_open_counts`]. Summen über Unterpunkte bildet [`subtree_open`] aus diesen Zahlen,
//! damit Gliederung, Editorkopf, Seite `/werke/<id>/befunde` und `finding.matrix` dieselben Zahlen zeigen.
//!
//! Die Karten ([`card`]) sind das gemeinsame Bild eines Befunds für Seite und Akkordeon: die Felder von
//! [`super::finding_json`] und dazu Zeichen, Abschnitt, Zeit, angehängte Befunde, Ursprünge eines Sammelvorschlags
//! und der offene Vorschlag in Worten.

use std::collections::HashMap;

use baruch_core::Work;
use baruch_store::{FindingFilter, IdeaOrigin, WorkId};
use serde_json::{Value, json};

use super::{DeferTarget, Finding, FindingKind, FindingStatus, FindingStatusEntry, finding_json, finding_url};
use crate::AppState;
use crate::rpc::RpcError;

/// Höchstens so viele Befunde lädt die Arbeitsseite (Obergrenze von [`baruch_store::FindingFilter::limit`]).
pub const VIEW_LIMIT: u32 = 2000;

/// Was die Arbeitsseite über Befunde zeigt.
#[derive(Debug, Clone, Default)]
pub struct FindingsView {
    /// `node.id` → offene Befunde nach Vertrag §1.5 (nur der Abschnitt selbst).
    pub open_by_key: HashMap<i64, usize>,
    /// Alle Befunde des Werks samt Lob und angehängten (höchstens [`VIEW_LIMIT`]), älteste zuerst.
    pub findings: Vec<Finding>,
    /// `node.id` → je Fassungsnummer die übernommenen Befunde ([`baruch_store::Store::findings_adopted_in`]).
    pub adopted: HashMap<i64, Vec<(u32, Vec<i64>)>>,
}

impl FindingsView {
    /// Befunde eines Abschnitts (alle, auch Lob und angehängte), älteste zuerst.
    pub fn of_key(&self, key: i64) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.key == Some(key))
    }

    /// Übernommene Befunde der Fassung `rev` im Abschnitt `key` (ids aufsteigend).
    pub fn adopted_in(&self, key: i64, rev: u32) -> Vec<i64> {
        self.adopted
            .get(&key)
            .and_then(|list| list.iter().find(|(r, _)| *r == rev))
            .map(|(_, ids)| ids.clone())
            .unwrap_or_default()
    }

    /// Befunde mit offenem Vorschlag „übernommen in Fassung `rev`“ im Abschnitt `key`.
    pub fn proposed_for(&self, key: i64, rev: u32) -> Vec<i64> {
        self.of_key(key)
            .filter(|f| {
                f.pending.as_ref().is_some_and(|p| p.status == FindingStatus::Adopted && p.adopted_rev == Some(rev))
            })
            .map(|f| f.id)
            .collect()
    }
}

/// Befunde des Werks für die Arbeitsseite laden: offene je Abschnitt, alle Befunde (für Akkordeon und
/// Fassungsbaum) und die Fassungen, in die Befunde eingeflossen sind.
pub async fn load(state: &AppState, work: WorkId) -> Result<FindingsView, RpcError> {
    let open_by_key = state.store.finding_open_counts(work).await?;
    let filter =
        FindingFilter { include_praise: true, include_attached: true, limit: Some(VIEW_LIMIT), ..Default::default() };
    let findings = state.store.finding_list(work, &filter).await?;
    let mut keys: Vec<i64> =
        findings.iter().filter(|f| f.status == FindingStatus::Adopted).filter_map(|f| f.key).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut adopted = HashMap::new();
    for key in keys {
        let list = state.store.findings_adopted_in(work, key).await?;
        if !list.is_empty() {
            adopted.insert(key, list);
        }
    }
    Ok(FindingsView { open_by_key, findings, adopted })
}

/// Offene Befunde je Stelle (`Node::id`, Baumreihenfolge) samt aller Unterpunkte: ein Kapitel zählt seine
/// Abschnitte mit, ein Teil seine Kapitel. Grundlage ist allein `open_by_key` (Zählregel §1.5).
pub fn subtree_open(work: &Work, open_by_key: &HashMap<i64, usize>) -> Vec<usize> {
    let mut sums: Vec<usize> = work.nodes.iter().map(|n| open_by_key.get(&n.key).copied().unwrap_or(0)).collect();
    // Knoten stehen in Baumreihenfolge (das Elternteil vor dem Kind): rückwärts aufsummieren.
    for node in work.nodes.iter().rev() {
        if let Some(parent) = node.parent.filter(|&p| p < node.id && p < sums.len()) {
            sums[parent] += sums[node.id];
        }
    }
    sums
}

/// Zeichen je Status (Anzeige nicht nur über Farbe): offen ○, übernommen ✓, verworfen ✗, verschoben →, erledigt =.
pub fn status_sign(status: FindingStatus) -> &'static str {
    match status {
        FindingStatus::Open => "○",
        FindingStatus::Adopted => "✓",
        FindingStatus::Rejected => "✗",
        FindingStatus::Deferred => "→",
        FindingStatus::Done => "=",
    }
}

/// Zeichen je Art: Lob +, Hinweis i, kritisch !.
pub fn kind_sign(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::Praise => "+",
        FindingKind::Hint => "i",
        FindingKind::Critical => "!",
    }
}

/// Verweis im Text: „[B<id>]“ (Vertrag §1.7).
pub fn reference(id: i64) -> String {
    format!("[B{id}]")
}

/// „Nummer Titel“ eines Abschnitts (ohne Titel „(ohne Titel)“).
pub fn node_label(work: &Work, key: i64) -> Option<String> {
    let node = work.nodes.iter().find(|n| n.key == key)?;
    let title = if node.title.is_empty() { "(ohne Titel)" } else { node.title.as_str() };
    Some(if node.number.is_empty() { title.to_owned() } else { format!("{} {title}", node.number) })
}

/// Wer etwas getan hat, lesbar: „Autor“, „@lektor“, „@mcp:probe (MCP)“.
pub fn actor_label(actor: &IdeaOrigin) -> String {
    match actor {
        IdeaOrigin::Human { name } => name.clone(),
        IdeaOrigin::Persona { handle } => handle.clone(),
        IdeaOrigin::Mcp { handle } => format!("{handle} (MCP)"),
    }
}

/// Ein Status in Worten samt Ziel: „übernommen in Fassung 3“, „verschoben nach 2.1 Weg“, „verschoben in Einfall 7“.
pub fn status_words(
    work: &Work,
    status: FindingStatus,
    rev: Option<u32>,
    deferred: Option<&DeferTarget>,
    deferred_uuid: Option<&str>,
) -> String {
    status_words_amend(work, status, rev, None, deferred, deferred_uuid)
}

/// Wie [`status_words`], mit dem Stand der Fortschreibung (Runde 22): „übernommen in Fassung 3 · Stand 2“ (nur k > 0,
/// [`baruch_store::findings::adopted_words`]).
pub fn status_words_amend(
    work: &Work,
    status: FindingStatus,
    rev: Option<u32>,
    amend: Option<u32>,
    deferred: Option<&DeferTarget>,
    deferred_uuid: Option<&str>,
) -> String {
    let label = status.label();
    match status {
        FindingStatus::Adopted => match rev {
            Some(rev) => format!("{label} in {}", baruch_store::findings::adopted_words(rev, amend)),
            None => format!("{label} (Fassung gelöscht)"),
        },
        FindingStatus::Deferred => match deferred {
            Some(DeferTarget::Node { key }) => {
                format!("{label} nach {}", node_label(work, *key).unwrap_or_else(|| format!("Abschnitt {key}")))
            }
            Some(DeferTarget::Idea { id }) => format!("{label} in Einfall {id}"),
            None if deferred_uuid.is_some() => format!("{label} (Ziel gelöscht)"),
            None => label.to_owned(),
        },
        _ => label.to_owned(),
    }
}

/// Ein Eintrag im Verlauf bzw. ein offener Vorschlag in Worten.
pub fn entry_json(work: &Work, entry: &FindingStatusEntry) -> Value {
    let words = status_words_amend(
        work,
        entry.status,
        entry.adopted_rev,
        entry.adopted_amend,
        entry.deferred.as_ref(),
        entry.deferred_uuid.as_deref(),
    );
    let mut value = json!(entry);
    value["words"] = json!(words);
    value["status_label"] = json!(entry.status.label());
    value["status_sign"] = json!(status_sign(entry.status));
    value["actor_label"] = json!(actor_label(&entry.actor));
    value["time"] = json!(crate::localtime::minutes(&entry.created));
    // Runde 22: Rückfrage einer Persona „Befund n scheint erledigt?“ (offener Vorschlag „erledigt“ eines Assistenten).
    value["question"] = json!(entry.proposed && entry.decision.is_none() && entry.status == FindingStatus::Done);
    value["decision_label"] = json!(match entry.decision.as_deref() {
        Some("confirmed") => "bestätigt",
        Some("rejected") => "verworfen",
        Some("stale") => "veraltet",
        _ => "",
    });
    value
}

/// Fassungen eines Abschnitts zur Auswahl „übernommen in Fassung …“: `{number, label, current}`, neueste zuerst.
pub fn revision_choices(work: &Work, key: i64) -> Vec<Value> {
    let Some(node) = work.nodes.iter().find(|n| n.key == key) else { return Vec::new() };
    let mut out: Vec<Value> = node
        .revisions
        .iter()
        .map(|r| {
            let current = r.status == baruch_core::RevisionStatus::Current;
            let status = match r.status {
                baruch_core::RevisionStatus::Current => "aktuell",
                baruch_core::RevisionStatus::Proposed => "Vorschlag",
                baruch_core::RevisionStatus::Rejected => "verworfen",
                baruch_core::RevisionStatus::Superseded => "ersetzt",
            };
            json!({ "number": r.number, "label": format!("Fassung {} ({status})", r.number), "current": current })
        })
        .collect();
    out.reverse();
    out
}

/// Zusatz am Status eines offenen Sammelbefunds, dessen Ursprünge alle entschieden sind (Runde 23a).
pub const STALE_SUMMARY_WORDS: &str = "veraltet (alle Ursprünge entschieden)";

/// Bild eines Befunds für Seite und Akkordeon: [`finding_json`] und dazu `ref` („[B<id>]“), `status_sign`,
/// `kind_sign`, `status_words`, `node_label`, `node_url`, `post_url`, `time`, `attached` (angehängte Befunde, eine
/// Ebene), `summary` (Ursprünge eines Sammelvorschlags: `{id, ref, url, persona, label}` bzw. `{uuid, missing}`),
/// `pending_json` (offener Vorschlag in Worten) und `revisions` (Auswahl für „übernommen“).
pub fn card(work_id: WorkId, work: &Work, finding: &Finding, all: &[Finding]) -> Value {
    let mut value = finding_json(work_id, finding);
    value["ref"] = json!(reference(finding.id));
    value["status_sign"] = json!(status_sign(finding.status));
    value["kind_sign"] = json!(kind_sign(finding.kind));
    value["status_words"] = json!(status_words_amend(
        work,
        finding.status,
        finding.adopted_rev,
        finding.adopted_amend,
        finding.deferred.as_ref(),
        finding.deferred_uuid.as_deref()
    ));
    // Runde 23a (PM-Entscheidung): Ein offener Sammelbefund, dessen Ursprünge alle entschieden sind, ist „veraltet“.
    if finding.stale_summary {
        value["status_words"] = json!(format!("{} · {STALE_SUMMARY_WORDS}", finding.status.label()));
    }
    value["time"] = json!(crate::localtime::minutes(&finding.created));
    value["node_label"] = json!(finding.key.and_then(|k| node_label(work, k)));
    value["node_url"] = json!(finding.key.map(|k| crate::view::node_url(work_id, k, None, false, "", "")));
    value["post_url"] = json!(match (finding.key, finding.post, finding.thread, finding.thread_post) {
        (Some(key), Some(post), _, _) =>
            Some(format!("{}#post-{post}", crate::view::node_url(work_id, key, None, false, "", ""))),
        (_, _, Some(thread), Some(post)) => Some(format!("/faden?work={work_id}&thread={thread}#beitrag-{post}")),
        _ => None,
    });
    let attached: Vec<Value> = all
        .iter()
        .filter(|a| a.similar_to == Some(finding.id))
        .map(|a| {
            let mut v = finding_json(work_id, a);
            v["ref"] = json!(reference(a.id));
            v["time"] = json!(crate::localtime::minutes(&a.created));
            v["kind_sign"] = json!(kind_sign(a.kind));
            v
        })
        .collect();
    value["attached"] = json!(attached);
    let summary: Vec<Value> = finding
        .summary_of
        .iter()
        .map(|uuid| match all.iter().find(|o| &o.uuid == uuid) {
            Some(origin) => json!({
                "id": origin.id, "ref": reference(origin.id), "url": finding_url(work_id, origin),
                "persona": origin.persona, "label": short(&origin.body, 80), "status_label": origin.status.label(),
            }),
            None => json!({ "uuid": uuid, "missing": true }),
        })
        .collect();
    value["summary"] = json!(summary);
    value["pending_json"] = finding.pending.as_ref().map_or(Value::Null, |p| entry_json(work, p));
    value["revisions"] = json!(finding.key.map(|k| revision_choices(work, k)).unwrap_or_default());
    // Runde 30b: Text als sicheres Markdown (chat_render), Lösungen mit „übernehmbar“ gegen die aktuelle Fassung des
    // Abschnitts, Werk und Rücksprung für die Formulare im Befund-Block (finding_block).
    value["work"] = json!(work_id);
    value["body_html"] = json!(crate::chat_render::markdown_html(&finding.body));
    let current = finding
        .key
        .and_then(|k| work.nodes.iter().find(|n| n.key == k))
        .and_then(|n| n.current_index().map(|i| n.revisions[i].body.as_str()))
        .unwrap_or("");
    let views: Vec<Value> = finding
        .solutions
        .iter()
        .enumerate()
        .map(|(index, s)| {
            json!({ "index": index, "quote": s.quote, "replacement": s.replacement, "step": s.step,
                    "adoptable": finding.key.is_some() && crate::assist::solutions::adoptable(s, current),
                    "finding": finding.id })
        })
        .collect();
    value["solution_views"] = json!(views);
    value["adoptable"] = json!(views.iter().any(|v| v["adoptable"] == json!(true)));
    value["solved"] = json!(!finding.solutions.is_empty());
    value["back"] = json!(finding.key.map(|k| crate::view::node_url(work_id, k, None, false, "", "")));
    value
}

/// Text auf `max` Zeichen gekürzt (mit „…“).
pub fn short(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(max.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyed_sample() -> Work {
        let mut work = baruch_core::sample::work();
        for node in &mut work.nodes {
            node.key = 100 + node.id as i64;
        }
        work
    }

    #[test]
    fn subtree_sums_carry_every_section_up_to_its_chapter_and_part() {
        let work = keyed_sample();
        // Ein Blatt mit Elternteil und dessen Elternteil suchen (Teil → Kapitel → Abschnitt).
        let leaf = work
            .nodes
            .iter()
            .find(|n| n.parent.and_then(|p| work.nodes[p].parent).is_some())
            .expect("Beispielwerk hat drei Ebenen");
        let parent = leaf.parent.unwrap();
        let grand = work.nodes[parent].parent.unwrap();
        let open = HashMap::from([(leaf.key, 2), (work.nodes[parent].key, 1)]);
        let sums = subtree_open(&work, &open);
        assert_eq!(sums[leaf.id], 2, "das Blatt zählt nur sich");
        assert_eq!(sums[parent], 3, "das Kapitel zählt sich und den Abschnitt (1 + 2)");
        assert_eq!(sums[grand], 3, "der Teil zählt alles darunter");
        assert_eq!(sums.iter().filter(|&&s| s > 0).count(), 3, "sonst bleibt alles 0: {sums:?}");
    }

    #[test]
    fn every_status_has_its_own_sign_so_color_is_never_the_only_hint() {
        let signs: Vec<&str> = FindingStatus::ALL.into_iter().map(status_sign).collect();
        let mut unique = signs.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), FindingStatus::ALL.len(), "verschiedene Zeichen je Status: {signs:?}");
        assert_eq!(reference(12), "[B12]", "Verweisform nach Vertrag §1.7");
        assert_eq!(short("  abcdef  ", 4), "abc…", "gekürzt mit Auslassungszeichen");
    }

    #[test]
    fn an_adopted_finding_names_the_stand_of_the_amended_version_only_above_zero() {
        let work = keyed_sample();
        let words = |amend| status_words_amend(&work, FindingStatus::Adopted, Some(3), amend, None, None);
        assert_eq!(words(Some(2)), "übernommen in Fassung 3 · Stand 2", "Runde 22: Stand der Fortschreibung");
        assert_eq!(words(Some(0)), "übernommen in Fassung 3", "Stand 0 = nie fortgeschrieben, ohne Zusatz");
        assert_eq!(words(None), "übernommen in Fassung 3", "ohne gemerkten Stand wie bisher");
    }
}
