// SPDX-License-Identifier: AGPL-3.0-or-later
//! Findings for the work page (Round 28, befund-uebersicht): open findings per section (badges in outline
//! and editor head, summed up for chapters and parts), the findings for the accordion „Befunde“ and the revision tree
//! („setzt Befund x, y um“).
//!
//! Counting rule (contract R28 §1.5): open = `status 'open'`, not attached, no praise, with section – exactly
//! [`baruch_store::Store::finding_open_counts`]. Sums over sub-items are built by [`subtree_open`] from these
//! numbers so that outline, editor head, page `/werke/<id>/befunde` and `finding.matrix` show the same numbers.
//!
//! The cards ([`card`]) are the shared picture of a finding for page and accordion: the fields of
//! [`super::finding_json`] plus signs, section, time, attached findings, origins of a collective proposal
//! and the open proposal in words.

use std::collections::HashMap;

use baruch_core::Work;
use baruch_store::{FindingFilter, IdeaOrigin, WorkId};
use serde_json::{Value, json};

use super::{DeferTarget, Finding, FindingKind, FindingStatus, FindingStatusEntry, finding_json, finding_url};
use crate::AppState;
use crate::i18n::tc;
use crate::rpc::RpcError;

/// At most this many findings the work page loads (upper limit of [`baruch_store::FindingFilter::limit`]).
pub const VIEW_LIMIT: u32 = 2000;

/// What the work page shows about findings.
#[derive(Debug, Clone, Default)]
pub struct FindingsView {
    /// `node.id` → open findings per contract §1.5 (only the section itself).
    pub open_by_key: HashMap<i64, usize>,
    /// All findings of the work with praise and attached ones (at most [`VIEW_LIMIT`]), oldest first.
    pub findings: Vec<Finding>,
    /// `node.id` → per revision number the adopted findings ([`baruch_store::Store::findings_adopted_in`]).
    pub adopted: HashMap<i64, Vec<(u32, Vec<i64>)>>,
}

impl FindingsView {
    /// Findings of a section (all, also praise and attached ones), oldest first.
    pub fn of_key(&self, key: i64) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.key == Some(key))
    }

    /// Adopted findings of the revision `rev` in the section `key` (ids ascending).
    pub fn adopted_in(&self, key: i64, rev: u32) -> Vec<i64> {
        self.adopted
            .get(&key)
            .and_then(|list| list.iter().find(|(r, _)| *r == rev))
            .map(|(_, ids)| ids.clone())
            .unwrap_or_default()
    }

    /// Findings with an open proposal „übernommen in Fassung `rev`“ in the section `key`.
    pub fn proposed_for(&self, key: i64, rev: u32) -> Vec<i64> {
        self.of_key(key)
            .filter(|f| {
                f.pending.as_ref().is_some_and(|p| p.status == FindingStatus::Adopted && p.adopted_rev == Some(rev))
            })
            .map(|f| f.id)
            .collect()
    }
}

/// Load the findings of the work for the work page: open ones per section, all findings (for accordion and
/// revision tree) and the revisions that findings flowed into.
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

/// Open findings per position (`Node::id`, tree order) including all sub-items: a chapter counts its
/// sections too, a part its chapters. The only basis is `open_by_key` (counting rule §1.5).
pub fn subtree_open(work: &Work, open_by_key: &HashMap<i64, usize>) -> Vec<usize> {
    let mut sums: Vec<usize> = work.nodes.iter().map(|n| open_by_key.get(&n.key).copied().unwrap_or(0)).collect();
    // Nodes are in tree order (the parent before the child): sum up backwards.
    for node in work.nodes.iter().rev() {
        if let Some(parent) = node.parent.filter(|&p| p < node.id && p < sums.len()) {
            sums[parent] += sums[node.id];
        }
    }
    sums
}

/// Sign per status (display not only by colour): open ○, adopted ✓, rejected ✗, postponed →, resolved =.
pub fn status_sign(status: FindingStatus) -> &'static str {
    match status {
        FindingStatus::Open => "○",
        FindingStatus::Adopted => "✓",
        FindingStatus::Rejected => "✗",
        FindingStatus::Deferred => "→",
        FindingStatus::Done => "=",
    }
}

/// Sign per kind: praise +, note i, critical !.
pub fn kind_sign(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::Praise => "+",
        FindingKind::Hint => "i",
        FindingKind::Critical => "!",
    }
}

/// Reference in the text: „[B<id>]“ (contract §1.7).
pub fn reference(id: i64) -> String {
    format!("[B{id}]")
}

/// “Number title” of a section (without a title „(ohne Titel)“).
pub fn node_label(work: &Work, key: i64) -> Option<String> {
    let node = work.nodes.iter().find(|n| n.key == key)?;
    let untitled = tc("findings_msg.view.untitled", &[]);
    let title = if node.title.is_empty() { untitled.as_str() } else { node.title.as_str() };
    Some(if node.number.is_empty() { title.to_owned() } else { format!("{} {title}", node.number) })
}

/// Who did something, readable: „Autor“, „@lektor“, „@mcp:probe (MCP)“.
pub fn actor_label(actor: &IdeaOrigin) -> String {
    match actor {
        IdeaOrigin::Human { name } => name.clone(),
        IdeaOrigin::Persona { handle } => handle.clone(),
        IdeaOrigin::Mcp { handle } => format!("{handle} (MCP)"),
    }
}

/// A status in words with target: „übernommen in Fassung 3“, „verschoben nach 2.1 Weg“, „verschoben in Einfall 7“.
pub fn status_words(
    work: &Work,
    status: FindingStatus,
    rev: Option<u32>,
    deferred: Option<&DeferTarget>,
    deferred_uuid: Option<&str>,
) -> String {
    status_words_amend(work, status, rev, None, deferred, deferred_uuid)
}

/// Like [`status_words`], with the continuation state (Round 22): „übernommen in Fassung 3 · Stand 2“ (only k > 0,
/// [`baruch_store::findings::adopted_words`]).
pub fn status_words_amend(
    work: &Work,
    status: FindingStatus,
    rev: Option<u32>,
    amend: Option<u32>,
    deferred: Option<&DeferTarget>,
    deferred_uuid: Option<&str>,
) -> String {
    let label = super::status_label(status);
    match status {
        FindingStatus::Adopted => match rev {
            Some(rev) => tc(
                "findings_msg.view.adopted_in",
                &[("label", label.into()), ("revision", baruch_store::findings::adopted_words(rev, amend).into())],
            ),
            None => tc("findings_msg.view.revision_deleted", &[("label", label.into())]),
        },
        FindingStatus::Deferred => match deferred {
            Some(DeferTarget::Node { key }) => {
                let target = node_label(work, *key)
                    .unwrap_or_else(|| tc("findings_msg.view.section", &[("key", (*key).into())]));
                tc("findings_msg.view.deferred_to", &[("label", label.into()), ("target", target.into())])
            }
            Some(DeferTarget::Idea { id }) => {
                tc("findings_msg.view.deferred_idea", &[("label", label.into()), ("id", (*id).into())])
            }
            None if deferred_uuid.is_some() => tc("findings_msg.view.target_deleted", &[("label", label.into())]),
            None => label.to_owned(),
        },
        _ => label.to_owned(),
    }
}

/// An entry in the history or an open proposal in words.
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
    value["status_label"] = json!(super::status_label(entry.status));
    value["status_sign"] = json!(status_sign(entry.status));
    value["actor_label"] = json!(actor_label(&entry.actor));
    value["time"] = json!(crate::localtime::minutes(&entry.created));
    // Round 22: query of a persona „Befund n scheint erledigt?“ (open proposal „erledigt“ of an assistant).
    value["question"] = json!(entry.proposed && entry.decision.is_none() && entry.status == FindingStatus::Done);
    value["decision_label"] = json!(match entry.decision.as_deref() {
        Some("confirmed") => tc("findings_msg.view.confirmed", &[]),
        Some("rejected") => tc("findings_msg.view.rejected", &[]),
        Some("stale") => tc("findings_msg.view.stale", &[]),
        _ => String::new(),
    });
    value
}

/// Revisions of a section for the selection „übernommen in Fassung …“: `{number, label, current}`, newest first.
pub fn revision_choices(work: &Work, key: i64) -> Vec<Value> {
    let Some(node) = work.nodes.iter().find(|n| n.key == key) else { return Vec::new() };
    let mut out: Vec<Value> = node
        .revisions
        .iter()
        .map(|r| {
            let current = r.status == baruch_core::RevisionStatus::Current;
            let status = match r.status {
                baruch_core::RevisionStatus::Current => tc("findings_msg.view.rev_current", &[]),
                baruch_core::RevisionStatus::Proposed => tc("findings_msg.view.rev_proposed", &[]),
                baruch_core::RevisionStatus::Rejected => tc("findings_msg.view.rev_rejected", &[]),
                baruch_core::RevisionStatus::Superseded => tc("findings_msg.view.rev_superseded", &[]),
            };
            let label = tc("findings_msg.view.revision", &[("number", r.number.into()), ("status", status.into())]);
            json!({ "number": r.number, "label": label, "current": current })
        })
        .collect();
    out.reverse();
    out
}

/// Addition to the status of an open collective finding whose origins are all decided (Round 23a), in the language
/// of the request.
pub fn stale_summary_words() -> String {
    tc("findings_msg.view.stale_summary", &[])
}

/// Picture of a finding for page and accordion: [`finding_json`] plus `ref` („[B<id>]“), `status_sign`,
/// `kind_sign`, `status_words`, `node_label`, `node_url`, `post_url`, `time`, `attached` (attached findings, one
/// level), `summary` (origins of a collective proposal: `{id, ref, url, persona, label}` or `{uuid, missing}`),
/// `pending_json` (open proposal in words) and `revisions` (selection for “adopted”).
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
    // Round 23a (PM decision): an open collective finding whose origins are all decided is “stale”.
    if finding.stale_summary {
        value["status_words"] = json!(format!("{} · {}", super::status_label(finding.status), stale_summary_words()));
    }
    value["time"] = json!(crate::localtime::minutes(&finding.created));
    value["node_label"] = json!(finding.key.and_then(|k| node_label(work, k)));
    value["node_url"] = json!(finding.key.map(|k| crate::view::node_url(work_id, k, None, false, "", "")));
    // Jumping (rework 0.9.0): the quotation leads to the position in the section (jump address with `find`, marked
    // in the typeset view or source, see `xref::jump_url`); without a quotation „zur Stelle“ leads to the section.
    value["place_url"] = json!(finding.key.map(|key| {
        let find = Some(finding.quote.trim()).filter(|q| !q.is_empty()).map(str::to_string);
        crate::xref::jump_url(&crate::xref::JumpTarget { work: work_id, key, rev: None, find, at: None })
    }));
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
                "persona": origin.persona, "label": short(&origin.body, 80), "status_label": super::status_label(origin.status),
            }),
            None => json!({ "uuid": uuid, "missing": true }),
        })
        .collect();
    value["summary"] = json!(summary);
    value["pending_json"] = finding.pending.as_ref().map_or(Value::Null, |p| entry_json(work, p));
    value["revisions"] = json!(finding.key.map(|k| revision_choices(work, k)).unwrap_or_default());
    // Round 30b: text as safe Markdown (chat_render), solutions with “adoptable” against the current revision of the
    // section, work and way back for the forms in the finding block (finding_block).
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

/// Text shortened to `max` characters (with „…“).
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
        // Look for a leaf with a parent and that one's parent (part → chapter → section).
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
        // The German page (Round 39b: the status word follows the request language).
        let words = |amend| {
            crate::i18n::sync_scope(crate::i18n::Lang::De, || {
                status_words_amend(&work, FindingStatus::Adopted, Some(3), amend, None, None)
            })
        };
        assert_eq!(words(Some(2)), "übernommen in Fassung 3 · Stand 2", "Runde 22: Stand der Fortschreibung");
        assert_eq!(words(Some(0)), "übernommen in Fassung 3", "Stand 0 = nie fortgeschrieben, ohne Zusatz");
        assert_eq!(words(None), "übernommen in Fassung 3", "ohne gemerkten Stand wie bisher");
    }
}
