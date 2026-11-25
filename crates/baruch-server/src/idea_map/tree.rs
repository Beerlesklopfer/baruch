// SPDX-License-Identifier: AGPL-3.0-or-later
//! Die Mindmap eines Werks als Baum (Vertrag Runde 34, Schnittstelle R34-S1 §S1.2; danach Besitz mindmap_daten).
//!
//! [`tree`] liefert das Werk als Wurzel (`w`), darunter Teile und Kapitel (`n:<key>`) und die Einfälle (`i:<id>`).
//! Signaturen und Feldnamen dieser Datei sind die Schnittstelle der Runde: `bild::mindmap` (mindmap_widget) und
//! `files` (mindmap_austausch) programmieren dagegen; sie ändern sich nicht (zusätzliche Felder nur am Ende).
//!
//! Stand S1: Der Baum entsteht ehrlich aus dem heutigen [`super::build`] – Einfälle sind Kinder ihres Ankers
//! (Kapitel bzw. Punkt), Einfälle ohne Bezug hängen direkt am Werk (die Wolke „Ohne Kapitel“ gibt es im Baum nicht).
//! Unterknoten von Einfällen (`parent_id`), eigene Titel und Reihenfolge kommen mit Migration 0052 (mindmap_daten);
//! bis dahin ist `own_title` leer und `ghost` immer `false`. [`import`] ist in S1 ein Stub.

use std::collections::HashMap;

use baruch_store::{IdeaId, IdeaLink, IdeaOrigin, IdeaStatus, WorkId};

use crate::AppState;
use crate::rpc::{REFUSED, RpcError};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_TREE_DEPTH: usize = 32;
pub const MAX_IMPORT_NODES: usize = 2000;
pub const NOT_YET: &str = "Die Mindmap ist noch nicht umgesetzt (Runde 34, mindmap_daten).";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeKind {
    Work,
    Part,
    Chapter,
    Idea,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TreeNode {
    /// "w", "n:<key>" oder "i:<id>".
    pub id: String,
    pub kind: TreeKind,
    pub idea: Option<IdeaId>,
    /// Gliederungspunkt: eigener key; Einfall: Bezug (Abschnitt), sonst None.
    pub key: Option<i64>,
    pub number: String,
    /// Angezeigter Titel, nie leer.
    pub title: String,
    /// Gespeicherter Titel des Einfalls (darf leer sein); bei Gliederungspunkten leer.
    pub own_title: String,
    pub note: String,
    /// true, wenn note, quote, source oder bible Text tragen.
    pub has_note: bool,
    pub words: Vec<String>,
    pub status: Option<IdeaStatus>,
    pub origin: Option<IdeaOrigin>,
    /// Titel des Abschnitts, auf den ein Einfall mit key verweist.
    pub section_title: Option<String>,
    /// Nur Einfälle: umbenennbar, umhängbar, löschbar.
    pub editable: bool,
    /// Treffer der Suche q.
    pub hit: bool,
    /// Passt nicht zum Filter, bleibt aber wegen sichtbarer Nachfahren stehen.
    pub ghost: bool,
    /// Offene Befunde (nur Teil/Kapitel, nur mit filter.findings).
    pub open_findings: u32,
    /// Nachfahren der Art Einfall (für „± n“).
    pub descendants: u32,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MindMap {
    pub work: WorkId,
    pub title: String,
    pub root: TreeNode,
    /// Verbindungen zwischen gezeigten Einfällen (nicht gelöscht).
    pub links: Vec<IdeaLink>,
    pub near: super::Near,
    pub filter: super::MapFilter,
    pub total_ideas: u32,
}

fn walk_into<'a>(node: &'a TreeNode, depth: usize, out: &mut Vec<(&'a TreeNode, usize)>) {
    out.push((node, depth));
    for child in &node.children {
        walk_into(child, depth + 1, out);
    }
}

fn find_in<'a>(node: &'a TreeNode, id: &str) -> Option<&'a TreeNode> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_in(child, id))
}

impl MindMap {
    /// Tiefensuche in Anzeigereihenfolge: (Knoten, Tiefe; Wurzel = 0).
    pub fn walk(&self) -> Vec<(&TreeNode, usize)> {
        let mut out = Vec::new();
        walk_into(&self.root, 0, &mut out);
        out
    }

    pub fn find(&self, id: &str) -> Option<&TreeNode> {
        find_in(&self.root, id)
    }
}

/// „Nummer Titel“ eines Abschnitts (ohne Nummer nur der Titel).
fn numbered(number: &str, title: &str) -> String {
    if number.is_empty() { title.to_owned() } else { format!("{number} {title}") }
}

/// Hängt die Kinder (erst Gliederungspunkte in Gliederungsreihenfolge, dann Einfälle nach id) an und zählt die
/// Nachfahren der Art Einfall.
fn attach(
    mut node: TreeNode,
    outline: &mut HashMap<String, Vec<TreeNode>>,
    ideas: &mut HashMap<String, Vec<TreeNode>>,
) -> TreeNode {
    let below = outline.remove(&node.id).unwrap_or_default();
    let own = ideas.remove(&node.id).unwrap_or_default();
    let mut descendants = own.len() as u32;
    for child in below {
        let child = attach(child, outline, ideas);
        descendants += child.descendants;
        node.children.push(child);
    }
    node.children.extend(own);
    node.descendants = descendants;
    node
}

/// Die Mindmap des Werks als Baum (Filter wie [`super::build`]; `open` gilt im Baum nicht – Abschnitte unter einem
/// Kapitel sind nie eigene Knoten, ihr Titel steht als `section_title` am Einfall).
///
/// Stand S1 (siehe Modulkopf): aus [`super::build`]. Teile und Kapitel hängen wie `MapNode::parent`; ein Abschnitt
/// ohne Kapitel darüber ist selbst Anker und erscheint als Knoten der Art `chapter`. Einfälle ohne (gültigen) Bezug
/// hängen direkt am Werk.
pub async fn tree(state: &AppState, work: WorkId, filter: &super::MapFilter) -> Result<MindMap, RpcError> {
    let filter = super::MapFilter { open: Vec::new(), ..filter.clone() };
    let map = super::build(state, work, &filter).await?;
    // Notiz-Symbol: `MapIdea` trägt nur `note`; quote, source und bible stehen am Einfall selbst.
    let annotated: HashMap<IdeaId, bool> = super::ideas_of(state, work)
        .await?
        .into_iter()
        .map(|i| (i.id, [&i.note, &i.quote, &i.source, &i.bible].iter().any(|t| !t.trim().is_empty())))
        .collect();

    let blank = |id: String, kind: TreeKind, title: String| TreeNode {
        id,
        kind,
        idea: None,
        key: None,
        number: String::new(),
        title,
        own_title: String::new(),
        note: String::new(),
        has_note: false,
        words: Vec::new(),
        status: None,
        origin: None,
        section_title: None,
        editable: false,
        hit: false,
        ghost: false,
        open_findings: 0,
        descendants: 0,
        children: Vec::new(),
    };

    // Gliederungspunkte je Elternknoten (Reihenfolge von `build` = Gliederungsreihenfolge).
    let mut outline: HashMap<String, Vec<TreeNode>> = HashMap::new();
    let mut root = blank("w".to_owned(), TreeKind::Work, map.title.clone());
    for node in &map.nodes {
        let kind = match node.kind {
            super::MapNodeKind::Work => {
                root.open_findings = node.open_findings;
                continue;
            }
            // Die Wolke „Ohne Kapitel“ gibt es im Baum nicht: Ihre Einfälle hängen am Werk.
            super::MapNodeKind::NoChapter => continue,
            super::MapNodeKind::Part => TreeKind::Part,
            super::MapNodeKind::Chapter | super::MapNodeKind::Section => TreeKind::Chapter,
        };
        let title = if node.title.trim().is_empty() { "(ohne Titel)".to_owned() } else { node.title.clone() };
        let mut item = blank(node.id.clone(), kind, title);
        item.key = node.key;
        item.number = node.number.clone();
        item.open_findings = node.open_findings;
        outline.entry(node.parent.clone().unwrap_or_else(|| "w".to_owned())).or_default().push(item);
    }
    let known: HashMap<&str, &super::MapNode> = map
        .nodes
        .iter()
        .filter(|n| !matches!(n.kind, super::MapNodeKind::Work | super::MapNodeKind::NoChapter))
        .map(|n| (n.id.as_str(), n))
        .collect();

    // Einfälle je Anker (Reihenfolge von `build` = nach id).
    let mut ideas: HashMap<String, Vec<TreeNode>> = HashMap::new();
    for idea in &map.ideas {
        let anchor = known.get(idea.anchor.as_str());
        let title = if !idea.label.trim().is_empty() {
            idea.label.clone()
        } else if !idea.note.trim().is_empty() {
            idea.note.trim().chars().take(60).collect()
        } else {
            format!("Einfall {}", idea.id)
        };
        let mut item = blank(idea.node.clone(), TreeKind::Idea, title);
        item.idea = Some(idea.id);
        item.key = idea.key;
        item.note = idea.note.clone();
        item.has_note = annotated.get(&idea.id).copied().unwrap_or(!idea.note.trim().is_empty());
        item.words = idea.words.clone();
        item.status = Some(idea.status);
        item.origin = Some(idea.origin.clone());
        item.section_title = anchor.and_then(|n| {
            let key = idea.key?;
            if n.key == Some(key) {
                return Some(numbered(&n.number, &n.title));
            }
            n.sections.iter().find(|s| s.key == key).map(|s| numbered(&s.number, &s.title))
        });
        item.editable = true;
        item.hit = idea.hit;
        ideas.entry(anchor.map_or_else(|| "w".to_owned(), |n| n.id.clone())).or_default().push(item);
    }

    let root = attach(root, &mut outline, &mut ideas);
    Ok(MindMap {
        work: map.work,
        title: map.title.clone(),
        root,
        links: map.links.iter().filter(|l| l.deleted.is_none()).cloned().collect(),
        near: map.near,
        filter: map.filter,
        total_ideas: map.ideas.len() as u32,
    })
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportNode {
    pub title: String,
    pub note: String,
    pub words: Vec<String>,
    pub children: Vec<ImportNode>,
}

/// Pfad = Indizes von `ImportTree::nodes` abwärts.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportLink {
    pub from: Vec<usize>,
    pub to: Vec<usize>,
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImportTree {
    pub title: String,
    pub nodes: Vec<ImportNode>,
    pub links: Vec<ImportLink>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ImportReport {
    pub ideas: Vec<IdeaId>,
    pub links: u32,
}

/// Legt den Baum als Einfälle des Werks an (unter `parent` oder als freie Hauptthemen): eine Transaktion,
/// ein Spiegel-Commit mit Herkunft. Verbindungen als `leads_to` mit der Beschriftung als Notiz.
///
/// Stand S1: legt nichts an und antwortet „abgelehnt“ ([`REFUSED`]) mit [`NOT_YET`].
pub async fn import(
    _state: &AppState,
    _work: WorkId,
    _parent: Option<IdeaId>,
    _tree: &ImportTree,
    _origin: &IdeaOrigin,
) -> Result<ImportReport, RpcError> {
    // STUB: Baum-Import der Einfälle fehlt (R34 mindmap_daten)
    Err(RpcError::new(REFUSED, NOT_YET))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, kind: TreeKind, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_owned(),
            kind,
            idea: None,
            key: None,
            number: String::new(),
            title: id.to_owned(),
            own_title: String::new(),
            note: String::new(),
            has_note: false,
            words: Vec::new(),
            status: None,
            origin: None,
            section_title: None,
            editable: kind == TreeKind::Idea,
            hit: false,
            ghost: false,
            open_findings: 0,
            descendants: 0,
            children,
        }
    }

    #[test]
    fn walk_is_depth_first_and_find_reaches_every_node() {
        let root = node(
            "w",
            TreeKind::Work,
            vec![
                node(
                    "n:1",
                    TreeKind::Chapter,
                    vec![node("i:5", TreeKind::Idea, vec![node("i:6", TreeKind::Idea, vec![])])],
                ),
                node("i:7", TreeKind::Idea, vec![]),
            ],
        );
        let map = MindMap {
            work: 1,
            title: "Probe".to_owned(),
            root,
            links: Vec::new(),
            near: crate::idea_map::Near {
                state: crate::idea_map::NearState::Off,
                message: String::new(),
                pairs: Vec::new(),
            },
            filter: crate::idea_map::MapFilter::default(),
            total_ideas: 3,
        };
        let order: Vec<(&str, usize)> = map.walk().into_iter().map(|(n, d)| (n.id.as_str(), d)).collect();
        assert_eq!(
            order,
            vec![("w", 0), ("n:1", 1), ("i:5", 2), ("i:6", 3), ("i:7", 1)],
            "Tiefensuche: jeder Knoten vor seinen Kindern, Kinder in ihrer Reihenfolge, Wurzel mit Tiefe 0"
        );
        assert_eq!(map.find("i:6").map(|n| n.kind), Some(TreeKind::Idea), "find erreicht auch tiefe Knoten");
        assert_eq!(map.find("w").map(|n| n.kind), Some(TreeKind::Work), "find kennt die Wurzel");
        assert!(map.find("i:99").is_none(), "unbekannte Kennung → None");
    }
}
