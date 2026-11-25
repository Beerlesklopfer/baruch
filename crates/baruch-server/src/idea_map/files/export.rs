// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ausgaben der Mindmap als Text (Vertrag Runde 34 §8, Besitz mindmap_austausch): das Blatt ([`Sheet`]) als
//! gemeinsame Vorstufe von FreeMind ([`super::freemind`]) und XMind ([`super::xmind`]) und die Markdown-Ausgabe
//! ([`markdown`]). Alles aus derselben Sicht ([`Scene`]) wie Seite und Bild – die Filter gelten also auch hier.
//!
//! **Wolke** (Nachtrag des Nutzers 2026-11-25): Einfälle ohne Platz – weder Elternknoten noch Bezug – liegen in der
//! Wolke neben der Mindmap. Die Sicht [`Scene`] hängt sie hinter den Hauptästen an die Wurzel und nennt ihre
//! Kennungen ([`cloud`]); alle Ausgaben führen sie als eigenen Ast „Wolke“ ([`CLOUD_ID`]) bzw. in Markdown als
//! eigenen Abschnitt. Freie Hauptthemen (Einfälle direkt am Werk) sind dagegen gewöhnliche Hauptäste.
//!
//! [`filter_words`], [`origin_word`] und [`links_of`] nutzen auch die Seite (`idea_map/bild/mod.rs`) und die
//! Gliederungsansicht.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use baruch_store::{IdeaId, IdeaLink, IdeaStatus, LinkKind};

use crate::idea_map::tree::{Scene, TreeKind, TreeNode};
use crate::idea_map::{MapFilter, NearVia};

/// „offen“ bzw. „verarbeitet“.
pub fn status_label(status: IdeaStatus) -> &'static str {
    match status {
        IdeaStatus::Open => "offen",
        IdeaStatus::Done => "verarbeitet",
    }
}

/// Ein Zeichen, das in Markdown etwas bedeutet, mit `\` davor; Zeilenumbrüche als Leerzeichen.
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

/// Angewandter Filter in Worten (leer ohne Filter).
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

/// Verbindungen eines Einfalls in der Schreibweise des Spiegels: `→ 79 führt weiter`, `← 12 Beispiel für`,
/// `↔ 48 widerspricht` (Pfeil, anderer Einfall, Art, Notiz), nach Verbindungs-id.
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

// ---- Wolke und Blatt -----------------------------------------------------------------------------

/// Kennung des Astes „Wolke“ in den Ausgaben (XMind: Themen-id, FreeMind: `ID_baruchwolke`). Der eigene Leser
/// erkennt den Ast daran und löst ihn wieder auf (seine Kinder werden Knoten der obersten Ebene).
pub const CLOUD_ID: &str = "baruch-wolke";
/// Titel des Astes „Wolke“.
pub const CLOUD_TITLE: &str = "Wolke";

/// Die Wolke: Einfälle ohne Platz ([`Scene::cloud`]), samt ihren Unterknoten.
pub fn cloud(scene: &Scene) -> Vec<&TreeNode> {
    scene.cloud_nodes()
}

/// Die Hauptäste der Mindmap ohne die Wolke (Teile, Kapitel und freie Hauptthemen).
pub fn placed(scene: &Scene) -> Vec<&TreeNode> {
    scene.branches()
}

/// „1.2 Titel“ bzw. „Titel“ (Einfälle tragen keine Nummer).
pub fn node_title(node: &TreeNode) -> String {
    if node.number.trim().is_empty() { node.title.clone() } else { format!("{} {}", node.number, node.title) }
}

/// Beschriftung einer Beziehung: Art, dahinter die Notiz („führt weiter: über den Deich“).
pub fn link_label(link: &IdeaLink) -> String {
    let note = link.note.split_whitespace().collect::<Vec<_>>().join(" ");
    if note.is_empty() { link.kind.label().to_owned() } else { format!("{}: {note}", link.kind.label()) }
}

/// Ein Knoten des Blatts (Werk, Teil, Kapitel, Einfall oder der Ast „Wolke“).
#[derive(Debug, Clone, PartialEq)]
pub struct OutNode {
    /// Kennung wie im Baum (`w`, `n:<key>`, `i:<id>`) bzw. [`CLOUD_ID`].
    pub id: String,
    pub title: String,
    pub note: String,
    pub words: Vec<String>,
    pub children: Vec<OutNode>,
}

/// Eine Beziehung zwischen zwei Knoten des Blatts.
#[derive(Debug, Clone, PartialEq)]
pub struct OutLink {
    pub id: i64,
    pub from: String,
    pub to: String,
    pub label: String,
    pub kind: LinkKind,
}

/// Was FreeMind und XMind von der Mindmap bekommen: Titel, Baum, Beziehungen.
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

/// Das Blatt der Mindmap: die Hauptäste in ihrer Reihenfolge, danach – falls es Einfälle ohne Platz gibt – der Ast
/// „Wolke“. Beziehungen nur zwischen Einfällen, die im Baum stehen (nicht gelöscht).
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

/// Markdown: Titel, Filter, der Baum als verschachtelte Liste (Notizen eingerückt unter ihrem Knoten), der Abschnitt
/// „Wolke“ (Einfälle ohne Platz), danach „Beziehungen“, „Widersprüche“ und (falls gezeigt) „Nähe“.
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
