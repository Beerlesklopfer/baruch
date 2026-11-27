// SPDX-License-Identifier: AGPL-3.0-or-later
//! Exports of the mind map as text (contract Round 34 §8, owner mindmap_austausch): the sheet ([`Sheet`]) as the
//! shared preliminary stage of FreeMind ([`super::freemind`]) and XMind ([`super::xmind`]) and the Markdown export
//! ([`markdown`]). All from the same view ([`Scene`]) as page and picture – so the filters apply here too.
//!
//! **Cloud** (addendum of the user 2026-11-25): ideas without a place – neither parent node nor relation – lie in
//! the cloud next to the mind map. The view [`Scene`] hangs them behind the main branches on the root and names
//! their ids ([`cloud`]); all exports carry them as their own branch „Wolke“ ([`CLOUD_ID`]) or in Markdown as
//! their own section. Free main topics (ideas directly on the work), by contrast, are ordinary main branches.
//!
//! [`filter_words`], [`origin_word`] and [`links_of`] are also used by the page (`idea_map/bild/mod.rs`) and the
//! outline view.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use baruch_store::{IdeaId, IdeaLink, IdeaStatus, LinkKind};

use crate::idea_map::tree::{Scene, TreeKind, TreeNode};
use crate::idea_map::{MapFilter, NearVia};

/// „offen“ or „verarbeitet“.
pub fn status_label(status: IdeaStatus) -> &'static str {
    match status {
        IdeaStatus::Open => "offen",
        IdeaStatus::Done => "verarbeitet",
    }
}

/// A character that means something in Markdown, with `\` before it; line breaks as spaces.
fn md(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.split_whitespace().collect::<Vec<_>>().join(" ").chars() {
        if matches!(c, '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|' | '{' | '}' | '!') {
            out.push('\\');
        }
        if !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// Applied filter in words (empty without filter).
pub fn filter_words(filter: &MapFilter) -> String {
    let mut parts = Vec::new();
    if let Some(status) = filter.status {
        parts.push(format!("Status {}", status_label(status)));
    }
    if let Some(word) = filter.word.as_deref().filter(|w| !w.trim().is_empty()) {
        parts.push(format!("Schlagwort „{}“", word.trim()));
    }
    if let Some(origin) = filter.origin.as_deref() {
        parts.push(format!("Herkunft {}", origin_word(origin)));
    }
    if let Some(q) = filter.q.as_deref().filter(|q| !q.trim().is_empty()) {
        parts.push(format!("Suche „{}“", q.trim()));
    }
    if !filter.near {
        parts.push("ohne Nähe".to_owned());
    }
    parts.join(" · ")
}

/// „Mensch“, „Persona“, „MCP“.
pub fn origin_word(origin: &str) -> &'static str {
    match origin {
        "human" => "Mensch",
        "persona" => "Persona",
        "mcp" => "MCP",
        _ => "unbekannt",
    }
}

/// Links of an idea in the notation of the mirror: `→ 79 führt weiter`, `← 12 Beispiel für`,
/// `↔ 48 widerspricht` (arrow, other idea, kind, note), by link id.
pub fn links_of(idea: IdeaId, links: &[IdeaLink]) -> Vec<(&IdeaLink, &'static str, IdeaId)> {
    let mut out: Vec<(&IdeaLink, &'static str, IdeaId)> = links
        .iter()
        .filter(|l| l.deleted.is_none() && (l.from == idea || l.to == idea))
        .map(|l| {
            let other = if l.from == idea { l.to } else { l.from };
            let arrow = if l.kind.symmetric() {
                "↔"
            } else if l.from == idea {
                "→"
            } else {
                "←"
            };
            (l, arrow, other)
        })
        .collect();
    out.sort_by_key(|(l, ..)| l.id);
    out
}

// ---- Cloud and sheet -----------------------------------------------------------------------------

/// Id of the branch „Wolke“ in the exports (XMind: topic id, FreeMind: `ID_baruchwolke`). The own reader
/// recognises the branch by it and dissolves it again (its children become nodes of the top level).
pub const CLOUD_ID: &str = "baruch-wolke";
/// Title of the branch „Wolke“.
pub const CLOUD_TITLE: &str = "Wolke";

/// The cloud: ideas without a place ([`Scene::cloud`]), with their sub-nodes.
pub fn cloud(scene: &Scene) -> Vec<&TreeNode> {
    scene.cloud_nodes()
}

/// The main branches of the mind map without the cloud (parts, chapters and free main topics).
pub fn placed(scene: &Scene) -> Vec<&TreeNode> {
    scene.branches()
}

/// „1.2 Titel“ or „Titel“ (ideas carry no number).
pub fn node_title(node: &TreeNode) -> String {
    if node.number.trim().is_empty() { node.title.clone() } else { format!("{} {}", node.number, node.title) }
}

/// Label of a relation: kind, after it the note („führt weiter: über den Deich“).
pub fn link_label(link: &IdeaLink) -> String {
    let note = link.note.split_whitespace().collect::<Vec<_>>().join(" ");
    if note.is_empty() { link.kind.label().to_owned() } else { format!("{}: {note}", link.kind.label()) }
}

/// A node of the sheet (work, part, chapter, idea or the branch „Wolke“).
#[derive(Debug, Clone, PartialEq)]
pub struct OutNode {
    /// Id as in the tree (`w`, `n:<key>`, `i:<id>`) or [`CLOUD_ID`].
    pub id: String,
    pub title: String,
    pub note: String,
    pub words: Vec<String>,
    pub children: Vec<OutNode>,
}

/// A relation between two nodes of the sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct OutLink {
    pub id: i64,
    pub from: String,
    pub to: String,
    pub label: String,
    pub kind: LinkKind,
}

/// What FreeMind and XMind get of the mind map: title, tree, relations.
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub title: String,
    pub root: OutNode,
    pub links: Vec<OutLink>,
}

fn out_node(node: &TreeNode) -> OutNode {
    OutNode {
        id: node.id.clone(),
        title: node_title(node),
        note: node.note.trim().to_owned(),
        words: node.words.clone(),
        children: node.children.iter().map(out_node).collect(),
    }
}

/// The sheet of the mind map: the main branches in their order, then – if there are ideas without a place – the
/// branch „Wolke“. Relations only between ideas that stand in the tree (not deleted).
pub fn sheet(scene: &Scene) -> Sheet {
    let map = &scene.map;
    let mut root = OutNode {
        id: map.root.id.clone(),
        title: if map.title.trim().is_empty() { node_title(&map.root) } else { map.title.clone() },
        note: String::new(),
        words: Vec::new(),
        children: placed(scene).into_iter().map(out_node).collect(),
    };
    let loose = cloud(scene);
    if !loose.is_empty() {
        root.children.push(OutNode {
            id: CLOUD_ID.to_owned(),
            title: CLOUD_TITLE.to_owned(),
            note: String::new(),
            words: Vec::new(),
            children: loose.into_iter().map(out_node).collect(),
        });
    }
    let shown: HashSet<IdeaId> = map.walk().into_iter().filter_map(|(n, _)| n.idea).collect();
    let links = map
        .links
        .iter()
        .filter(|l| l.deleted.is_none() && shown.contains(&l.from) && shown.contains(&l.to))
        .map(|l| OutLink {
            id: l.id,
            from: format!("i:{}", l.from),
            to: format!("i:{}", l.to),
            label: link_label(l),
            kind: l.kind,
        })
        .collect();
    Sheet { title: root.title.clone(), root, links }
}

// ---- Markdown ------------------------------------------------------------------------------------

fn md_node(out: &mut String, node: &TreeNode, depth: usize) {
    let indent = "  ".repeat(depth);
    let mut line = format!("{indent}- ");
    match node.kind {
        TreeKind::Idea => {
            let _ = write!(line, "{}", md(&node.title));
            let mut facts = Vec::new();
            if let Some(status) = node.status {
                facts.push(status_label(status).to_owned());
            }
            if !node.words.is_empty() {
                facts.push(format!("Schlagwörter: {}", md(&node.words.join(", "))));
            }
            if let Some(section) = node.section_title.as_deref().filter(|s| !s.trim().is_empty()) {
                facts.push(format!("Bezug: {}", md(section)));
            }
            if let Some(origin) = &node.origin {
                facts.push(md(&origin.label()));
            }
            if !facts.is_empty() {
                let _ = write!(line, " ({})", facts.join("; "));
            }
        }
        _ => {
            let _ = write!(line, "**{}**", md(&node_title(node)));
            if node.open_findings > 0 {
                let _ = write!(line, " ({} offene Befunde)", node.open_findings);
            }
        }
    }
    let _ = writeln!(out, "{line}");
    if !node.note.trim().is_empty() {
        let _ = writeln!(out, "{indent}  Notiz: {}", md(&node.note));
    }
    for child in &node.children {
        md_node(out, child, depth + 1);
    }
}

/// Markdown: title, filter, the tree as a nested list (notes indented below their node), the section
/// „Wolke“ (ideas without a place), then „Beziehungen“, „Widersprüche“ and (if shown) „Nähe“.
pub fn markdown(scene: &Scene) -> String {
    let map = &scene.map;
    let mut out = String::new();
    let _ = writeln!(out, "# Mindmap: {}\n", md(&map.title));
    let filter = filter_words(&map.filter);
    if !filter.is_empty() {
        let _ = writeln!(out, "Filter: {}\n", md(&filter));
    }
    let branches = placed(scene);
    if branches.is_empty() {
        let _ = writeln!(out, "Keine Äste.");
    }
    for node in branches {
        md_node(&mut out, node, 0);
    }
    let loose = cloud(scene);
    let _ = writeln!(out, "\n## Wolke\n");
    if loose.is_empty() {
        let _ = writeln!(out, "Leer: Kein Einfall ohne Platz.");
    } else {
        let _ = writeln!(out, "{} in der Wolke (Einfälle ohne Platz in der Mindmap).\n", loose.len());
        for node in loose {
            md_node(&mut out, node, 0);
        }
    }
    let titles: HashMap<IdeaId, &str> =
        map.walk().into_iter().filter_map(|(n, _)| n.idea.map(|id| (id, n.title.as_str()))).collect();
    let name = |id: IdeaId| match titles.get(&id) {
        Some(title) => format!("{} (Einfall {id})", md(title)),
        None => format!("Einfall {id}"),
    };
    let shown: Vec<&IdeaLink> = map
        .links
        .iter()
        .filter(|l| l.deleted.is_none() && titles.contains_key(&l.from) && titles.contains_key(&l.to))
        .collect();
    let _ = writeln!(out, "\n## Beziehungen\n");
    if shown.is_empty() {
        let _ = writeln!(out, "Keine.");
    }
    for link in &shown {
        let arrow = if link.kind.symmetric() { "↔" } else { "→" };
        let _ = writeln!(out, "- {} {arrow} {}: {}", name(link.from), name(link.to), md(&link_label(link)));
    }
    let _ = writeln!(out, "\n## Widersprüche\n");
    let contradictions: Vec<&&IdeaLink> = shown.iter().filter(|l| l.kind == LinkKind::Contradicts).collect();
    if contradictions.is_empty() {
        let _ = writeln!(out, "Keine.");
    }
    for link in contradictions {
        let mut line = format!("- {} ↔ {}", name(link.from), name(link.to));
        if !link.note.trim().is_empty() {
            let _ = write!(line, ": „{}“", md(&link.note));
        }
        let _ = writeln!(out, "{line}");
    }
    if map.filter.near && !map.near.pairs.is_empty() {
        let _ = writeln!(out, "\n## Nähe (nur angezeigt)\n");
        for pair in &map.near.pairs {
            let via = match pair.via {
                NearVia::Vector => "Bedeutung".to_owned(),
                NearVia::Words => format!("Schlagwörter {}", pair.shared_words.join(", ")),
                NearVia::Both => format!("Bedeutung, Schlagwörter {}", pair.shared_words.join(", ")),
            };
            let _ = writeln!(out, "- {} ~ {} ({:.2}, {})", name(pair.a), name(pair.b), pair.score, md(&via));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_specials_are_escaped() {
        assert_eq!(
            md("# *Laube* [x]\n<b>"),
            "\\# \\*Laube\\* \\[x\\] \\<b\\>",
            "kein Nutzertext wird Markdown-Auszeichnung"
        );
    }
}
