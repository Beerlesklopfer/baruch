// SPDX-License-Identifier: AGPL-3.0-or-later
//! Die Mindmap eines Werks als Baum (Vertrag Runde 34, Schnittstelle R34-S1 §S1.2; danach Besitz mindmap_daten).
//!
//! [`tree`] liefert das Werk als Wurzel (`w`), darunter Teile und Kapitel (`n:<key>`) und die Einfälle (`i:<id>`).
//! Signaturen und Feldnamen dieser Datei sind die Schnittstelle der Runde: `bild::mindmap` (mindmap_widget) und
//! `files` (mindmap_austausch) programmieren dagegen; sie ändern sich nicht (zusätzliche Felder nur am Ende).
//!
//! Seit mindmap_daten (Migration 0052): Einfälle tragen Titel, Elternknoten und Reihenfolge; [`tree`] hängt sie nach
//! einer Regel ein (siehe dort), Filter lassen Vorfahren sichtbarer Einfälle als Geister stehen, [`import`] legt einen
//! eingelesenen Baum in einer Transaktion mit einem Spiegel-Commit an.
//!
//! Nachtrag des Nutzers vom 2026-11-25 („eine Verbindung zwischen der Wolke und der Mindmap“): Einfälle ohne Platz –
//! ohne Elternknoten, ohne (gültigen) Bezug und nicht als Hauptthema ans Werk gehängt – stehen **nicht** im Baum,
//! sondern in der Wolke daneben ([`Cloud`], [`cloud`], [`tree_and_cloud`]; RPC `idea.tree` liefert beide). Die
//! Signaturen aus S1.2 sind dafür unverändert geblieben; die Wolke ist eine zusätzliche Funktion.

use std::collections::{HashMap, HashSet};

use baruch_store::{Idea, IdeaId, IdeaLink, IdeaOrigin, IdeaSeed, IdeaSeedLink, IdeaStatus, StoreError, WorkId};

use crate::rpc::{REFUSED, RpcError};
use crate::{AppState, mirror};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_TREE_DEPTH: usize = 32;
pub const MAX_IMPORT_NODES: usize = 2000;
/// Text des S1-Stubs von [`import`]; seit mindmap_daten nicht mehr benutzt (bleibt, weil er zur Schnittstelle S1.2
/// gehört und fremde Tests ihn nennen).
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

/// Vorschlag der Wolke: Der Einfall `idea` (in der Wolke) ist dem Knoten `node` der Mindmap nah – „passt zu Ast
/// <branch_title>“. Nur Anzeige, nie gespeichert (Nähe wie auf der Karte: gespeicherte Vektoren und gleiche
/// Schlagwörter, [`super::near`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CloudHint {
    /// Einfall der Wolke (oberste Ebene), für den der Vorschlag gilt.
    pub idea: IdeaId,
    /// Naher Einfall in der Mindmap (`i:<id>`), unter den sich der Einfall hängen ließe.
    pub node: String,
    pub node_title: String,
    /// Hauptast (Kind der Wurzel), in dem `node` hängt.
    pub branch: String,
    pub branch_title: String,
    pub score: f32,
    pub shared_words: Vec<String>,
}

/// Die Wolke: Ablage neben der Mindmap für Einfälle ohne Platz (Nachtrag des Nutzers vom 2026-11-25) – ohne
/// Elternknoten, ohne (gültigen) Bezug und nicht als Hauptthema ans Werk gehängt. Ein Einfall der Wolke behält seine
/// Unterknoten (`children`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Cloud {
    pub work: WorkId,
    /// Einfälle der Wolke nach `position`, dann `id`; mit Filter nur passende (und Geister über passenden
    /// Unterknoten).
    pub ideas: Vec<TreeNode>,
    /// „n in der Wolke“: Einfälle der obersten Ebene der Wolke, ohne Filter.
    pub count: u32,
    /// Dieselben samt ihren Unterknoten, ohne Filter.
    pub count_with_children: u32,
    /// Je Einfall der Wolke höchstens ein Vorschlag (der nächste Einfall in der Mindmap), nach Einfall.
    pub hints: Vec<CloudHint>,
    /// Verbindungen (nicht gelöscht) gezeigter Einfälle, von denen mindestens einer in der Wolke liegt.
    pub links: Vec<IdeaLink>,
}

fn blank(id: String, kind: TreeKind, title: String) -> TreeNode {
    TreeNode {
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
    }
}

/// Was [`assemble`] über die Einfälle weiß.
struct Ideas<'a> {
    by_id: HashMap<IdeaId, &'a Idea>,
    /// Unterknoten je Einfall, nach `position`, dann `id`.
    below: HashMap<IdeaId, Vec<IdeaId>>,
    /// Passt zum Filter (Wert = Treffer der Suche `q`).
    shown: HashMap<IdeaId, bool>,
    section_titles: &'a HashMap<i64, String>,
}

impl Ideas<'_> {
    /// Der Einfall als Knoten samt Unterknoten; `None`, wenn weder er noch ein Nachfahre zum Filter passt.
    fn node(&self, id: IdeaId, depth: usize) -> Option<TreeNode> {
        let idea = self.by_id.get(&id)?;
        let mut children = Vec::new();
        // Der Store hält die Kette bei höchstens MAX_TREE_DEPTH; die Grenze hier schützt nur vor kaputten Daten.
        if depth < 2 * MAX_TREE_DEPTH {
            for child in self.below.get(&id).map(Vec::as_slice).unwrap_or_default() {
                children.extend(self.node(*child, depth + 1));
            }
        }
        let shown = self.shown.get(&id).copied();
        if shown.is_none() && children.is_empty() {
            return None;
        }
        let mut item = blank(format!("i:{id}"), TreeKind::Idea, idea.display_title());
        item.idea = Some(id);
        item.key = idea.key;
        item.own_title = idea.title.clone();
        item.note = idea.note.clone();
        item.has_note = idea.has_note();
        item.words = idea.words.clone();
        item.status = Some(idea.status);
        item.origin = Some(idea.origin.clone());
        item.section_title = idea.key.and_then(|key| self.section_titles.get(&key).cloned());
        item.editable = true;
        item.hit = shown.unwrap_or(false);
        item.ghost = shown.is_none();
        item.descendants = children.iter().map(|c| 1 + c.descendants).sum();
        item.children = children;
        Some(item)
    }
}

/// Zählt die gezeigten Einfälle (ohne Geister) in einem Teilbaum.
fn shown_ideas(node: &TreeNode) -> u32 {
    u32::from(node.kind == TreeKind::Idea && !node.ghost) + node.children.iter().map(shown_ideas).sum::<u32>()
}

fn collect_ideas(node: &TreeNode, out: &mut HashSet<IdeaId>) {
    out.extend(node.idea);
    for child in &node.children {
        collect_ideas(child, out);
    }
}

/// Hängt die Kinder an einen Gliederungspunkt (erst Gliederungspunkte in Gliederungsreihenfolge, dann seine
/// Einfälle nach `position`, dann `id`) und zählt die Nachfahren der Art Einfall.
fn attach(
    mut node: TreeNode,
    outline: &mut HashMap<String, Vec<TreeNode>>,
    ideas: &mut HashMap<String, Vec<TreeNode>>,
) -> TreeNode {
    let below = outline.remove(&node.id).unwrap_or_default();
    let own = ideas.remove(&node.id).unwrap_or_default();
    let mut descendants: u32 = own.iter().map(|c| 1 + c.descendants).sum();
    for child in below {
        let child = attach(child, outline, ideas);
        descendants += child.descendants;
        node.children.push(child);
    }
    node.children.extend(own);
    node.descendants = descendants;
    node
}

/// Baut Mindmap und Wolke in einem Zug. `near_for_hints`: die Nähe auch dann berechnen, wenn der Filter sie für das
/// Bild ausschaltet (die Vorschläge der Wolke brauchen sie); die Mindmap meldet dann trotzdem „aus“.
async fn assemble(
    state: &AppState,
    work: WorkId,
    filter: &super::MapFilter,
    near_for_hints: bool,
) -> Result<(MindMap, Cloud), RpcError> {
    let wanted_near = filter.near;
    let filter = super::MapFilter { open: Vec::new(), near: wanted_near || near_for_hints, ..filter.clone() };
    let map = super::build(state, work, &filter).await?;
    let all = state.store.idea_tree_rows(work).await?;

    // Gliederungspunkte je Elternknoten (Reihenfolge von `build` = Gliederungsreihenfolge); dazu je key der Knoten,
    // an dem ein Einfall mit diesem Bezug hängt (Anker), und der Titel des Abschnitts.
    let mut outline: HashMap<String, Vec<TreeNode>> = HashMap::new();
    let mut anchor_of_key: HashMap<i64, String> = HashMap::new();
    let mut section_titles: HashMap<i64, String> = HashMap::new();
    let mut root = blank("w".to_owned(), TreeKind::Work, map.title.clone());
    for node in &map.nodes {
        let kind = match node.kind {
            super::MapNodeKind::Work => {
                root.open_findings = node.open_findings;
                continue;
            }
            // Die Wolke „Ohne Kapitel“ der flachen Karte ist im Baum die Ablage neben der Mindmap ([`Cloud`]).
            super::MapNodeKind::NoChapter => continue,
            super::MapNodeKind::Part => TreeKind::Part,
            super::MapNodeKind::Chapter | super::MapNodeKind::Section => TreeKind::Chapter,
        };
        if let Some(key) = node.key {
            anchor_of_key.insert(key, node.id.clone());
            section_titles.insert(key, numbered(&node.number, &node.title));
        }
        for section in &node.sections {
            anchor_of_key.insert(section.key, node.id.clone());
            section_titles.insert(section.key, numbered(&section.number, &section.title));
        }
        let title = if node.title.trim().is_empty() { "(ohne Titel)".to_owned() } else { node.title.clone() };
        let mut item = blank(node.id.clone(), kind, title);
        item.key = node.key;
        item.number = node.number.clone();
        item.open_findings = node.open_findings;
        outline.entry(node.parent.clone().unwrap_or_else(|| "w".to_owned())).or_default().push(item);
    }

    // Einfälle: Unterknoten je Elternknoten; ein Elternknoten, den es (nicht gelöscht, im Werk) nicht gibt, zählt
    // wie keiner.
    let by_id: HashMap<IdeaId, &Idea> = all.iter().map(|i| (i.id, i)).collect();
    let mut below: HashMap<IdeaId, Vec<IdeaId>> = HashMap::new();
    let mut top: Vec<&Idea> = Vec::new();
    for idea in &all {
        match idea.parent.filter(|p| by_id.contains_key(p)) {
            Some(parent) => below.entry(parent).or_default().push(idea.id),
            None => top.push(idea),
        }
    }
    let order = |id: &IdeaId| by_id.get(id).map_or((0, *id), |i| (i.position, i.id));
    for list in below.values_mut() {
        list.sort_by_key(order);
    }
    top.sort_by_key(|i| (i.position, i.id));
    let ideas = Ideas {
        by_id: by_id.clone(),
        below,
        shown: map.ideas.iter().map(|i| (i.id, i.hit)).collect(),
        section_titles: &section_titles,
    };

    // Platz der obersten Einfälle: am Anker ihres Bezugs, als Hauptthema am Werk oder in der Wolke.
    let mut at_anchor: HashMap<String, Vec<TreeNode>> = HashMap::new();
    let mut cloud_ideas: Vec<TreeNode> = Vec::new();
    let (mut cloud_count, mut cloud_all) = (0u32, 0u32);
    for idea in top {
        let anchor = idea.key.and_then(|key| anchor_of_key.get(&key).cloned());
        let in_cloud = anchor.is_none() && !idea.main_topic;
        if in_cloud {
            // Zähler ohne Filter: der Einfall und alles darunter.
            cloud_count += 1;
            let mut stack = vec![idea.id];
            while let Some(at) = stack.pop() {
                cloud_all += 1;
                stack.extend(ideas.below.get(&at).into_iter().flatten().copied());
            }
        }
        let Some(node) = ideas.node(idea.id, 1) else { continue };
        match (anchor, in_cloud) {
            (Some(anchor), _) => at_anchor.entry(anchor).or_default().push(node),
            (None, true) => cloud_ideas.push(node),
            (None, false) => at_anchor.entry("w".to_owned()).or_default().push(node),
        }
    }

    let root = attach(root, &mut outline, &mut at_anchor);
    let mut in_tree = HashSet::new();
    collect_ideas(&root, &mut in_tree);
    let mut in_cloud = HashSet::new();
    for node in &cloud_ideas {
        collect_ideas(node, &mut in_cloud);
    }

    let active: Vec<IdeaLink> = map.links.iter().filter(|l| l.deleted.is_none()).cloned().collect();
    let (tree_links, other_links): (Vec<IdeaLink>, Vec<IdeaLink>) =
        active.into_iter().partition(|l| in_tree.contains(&l.from) && in_tree.contains(&l.to));
    let cloud_links: Vec<IdeaLink> =
        other_links.into_iter().filter(|l| in_cloud.contains(&l.from) || in_cloud.contains(&l.to)).collect();

    // Vorschläge der Wolke: je Einfall der obersten Ebene das nächste Paar (höchster Wert, dann kleinste id) mit
    // einem Einfall der Mindmap; der Hauptast ist das Kind der Wurzel, in dem dieser hängt.
    let mut branch_of: HashMap<IdeaId, (String, String, String)> = HashMap::new();
    for branch in &root.children {
        let mut stack = vec![branch];
        while let Some(node) = stack.pop() {
            if let Some(id) = node.idea {
                branch_of.insert(id, (node.title.clone(), branch.id.clone(), branch.title.clone()));
            }
            stack.extend(node.children.iter());
        }
    }
    let mut hints: Vec<CloudHint> = Vec::new();
    for node in &cloud_ideas {
        let Some(id) = node.idea else { continue };
        let mut best: Option<(&super::NearPair, IdeaId)> = None;
        for pair in &map.near.pairs {
            let other = if pair.a == id {
                pair.b
            } else if pair.b == id {
                pair.a
            } else {
                continue;
            };
            if !branch_of.contains_key(&other) {
                continue;
            }
            let better = best.is_none_or(|(old, old_other)| {
                pair.score > old.score || (pair.score == old.score && other < old_other)
            });
            if better {
                best = Some((pair, other));
            }
        }
        if let Some((pair, other)) = best
            && let Some((node_title, branch, branch_title)) = branch_of.get(&other)
        {
            hints.push(CloudHint {
                idea: id,
                node: format!("i:{other}"),
                node_title: node_title.clone(),
                branch: branch.clone(),
                branch_title: branch_title.clone(),
                score: pair.score,
                shared_words: pair.shared_words.clone(),
            });
        }
    }

    // Nähe im Bild: nur Paare, deren beide Einfälle in der Mindmap stehen; ausgeschaltet bleibt ausgeschaltet.
    let near = if wanted_near {
        super::Near {
            state: map.near.state,
            message: map.near.message.clone(),
            pairs: map
                .near
                .pairs
                .iter()
                .filter(|p| in_tree.contains(&p.a) && in_tree.contains(&p.b))
                .cloned()
                .collect(),
        }
    } else {
        super::Near { state: super::NearState::Off, message: super::near::OFF.to_owned(), pairs: Vec::new() }
    };
    let total_ideas = shown_ideas(&root);
    let mind_map = MindMap {
        work: map.work,
        title: map.title.clone(),
        root,
        links: tree_links,
        near,
        filter: super::MapFilter { near: wanted_near, ..map.filter },
        total_ideas,
    };
    let cloud = Cloud {
        work: map.work,
        ideas: cloud_ideas,
        count: cloud_count,
        count_with_children: cloud_all,
        hints,
        links: cloud_links,
    };
    Ok((mind_map, cloud))
}

/// Die Mindmap des Werks als Baum (Filter wie [`super::build`]; `open` gilt im Baum nicht – Abschnitte unter einem
/// Kapitel sind nie eigene Knoten, ihr Titel steht als `section_title` am Einfall).
///
/// Ort eines Einfalls (eine Regel, überall gleich):
/// 1. mit Elternknoten (`parent`, nicht gelöscht, gleiches Werk) Unterknoten dieses Einfalls, Geschwister nach
///    `position`, dann `id`; der Bezug (`key`) bleibt daneben erlaubt und ist dann nur der Link zum Abschnitt;
/// 2. ohne Elternknoten, mit Bezug: Kind des Ankers (nächster Vorfahre der Art Kapitel, sonst der Punkt selbst);
/// 3. ohne beides, als Hauptthema (`main_topic`): direkt am Werk, hinter Teilen und Kapiteln;
/// 4. sonst in der Wolke – nicht im Baum, sondern in [`cloud`].
///
/// Filter (`status`, `word`, `origin`): Ein Einfall, der nicht passt, bleibt als Geist (`ghost`) stehen, solange ein
/// Nachfahre passt; `q` blendet nichts aus, sondern setzt `hit`. `links` und `near.pairs` nennen nur Einfälle, die im
/// Baum stehen. `total_ideas` zählt die gezeigten Einfälle im Baum (ohne Geister, ohne Wolke).
pub async fn tree(state: &AppState, work: WorkId, filter: &super::MapFilter) -> Result<MindMap, RpcError> {
    Ok(assemble(state, work, filter, false).await?.0)
}

/// Die Wolke des Werks ([`Cloud`]): Einfälle ohne Platz in der Mindmap, mit Zähler und Vorschlägen „passt zu Ast …“
/// (die Nähe wird dafür immer berechnet, auch wenn der Filter sie für das Bild ausschaltet; nie eine
/// Einbettungsanfrage).
pub async fn cloud(state: &AppState, work: WorkId, filter: &super::MapFilter) -> Result<Cloud, RpcError> {
    Ok(assemble(state, work, filter, true).await?.1)
}

/// Mindmap und Wolke in einem Abruf (für Seiten, die beide zeigen).
pub async fn tree_and_cloud(
    state: &AppState,
    work: WorkId,
    filter: &super::MapFilter,
) -> Result<(MindMap, Cloud), RpcError> {
    assemble(state, work, filter, true).await
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

/// Knoten und größte Tiefe eines eingelesenen Baums, ohne Rekursion (eine sehr tiefe Eingabe darf den Stapel nicht
/// sprengen, bevor sie abgelehnt ist).
fn import_size(tree: &ImportTree) -> (usize, usize) {
    let (mut count, mut deepest) = (0usize, 0usize);
    let mut stack: Vec<(&ImportNode, usize)> = tree.nodes.iter().map(|n| (n, 1)).collect();
    while let Some((node, depth)) = stack.pop() {
        count += 1;
        deepest = deepest.max(depth);
        stack.extend(node.children.iter().map(|c| (c, depth + 1)));
    }
    (count, deepest)
}

/// Erst nach [`import_size`] aufrufen: die Tiefe ist dann höchstens [`MAX_TREE_DEPTH`].
fn seed(node: &ImportNode) -> IdeaSeed {
    IdeaSeed {
        title: node.title.clone(),
        note: node.note.clone(),
        words: node.words.clone(),
        children: node.children.iter().map(seed).collect(),
    }
}

/// Legt den Baum als Einfälle des Werks an (unter `parent` oder als freie Hauptthemen): eine Transaktion,
/// ein Spiegel-Commit mit Herkunft. Verbindungen als `leads_to` mit der Beschriftung als Notiz.
///
/// Die obersten Knoten (`tree.nodes`) werden Unterknoten von `parent` bzw. – ohne – freie Hauptthemen am Werk
/// (nicht Einfälle der Wolke: Wer einen Baum einliest, will ihn in der Mindmap sehen), in der Reihenfolge der
/// Eingabe hinter den vorhandenen Geschwistern. `tree.title` und `tree.warnings` werden nicht gespeichert.
///
/// Abgelehnt mit [`REFUSED`], ohne etwas anzulegen: kein Knoten, mehr als [`MAX_IMPORT_NODES`] Knoten, tiefer als
/// [`MAX_TREE_DEPTH`] (auch zusammen mit der Tiefe von `parent`), `parent` gelöscht oder in einem anderen Werk, ein
/// Titel über [`MAX_TITLE_CHARS`] bzw. eine Notiz über 4000 Zeichen, Schlagwörter über den Grenzen der Einfälle, eine
/// Verbindung mit einem Pfad ohne Knoten. Ein Knoten ohne Titel und Schlagwort heißt „(ohne Titel)“; eine Verbindung
/// eines Knotens mit sich selbst oder eine doppelte wird übergangen (nicht gezählt). Unbekanntes Werk → Fehler wie
/// bei den übrigen Methoden.
pub async fn import(
    state: &AppState,
    work: WorkId,
    parent: Option<IdeaId>,
    tree: &ImportTree,
    origin: &IdeaOrigin,
) -> Result<ImportReport, RpcError> {
    let refused = |message: String| RpcError::new(REFUSED, message);
    let (count, deepest) = import_size(tree);
    if count == 0 {
        return Err(refused("Die Datei enthält keinen Knoten; nichts angelegt.".to_owned()));
    }
    if count > MAX_IMPORT_NODES {
        return Err(refused(format!(
            "Die Mindmap hat {count} Knoten; einlesen lassen sich höchstens {MAX_IMPORT_NODES}. Nichts angelegt."
        )));
    }
    if deepest > MAX_TREE_DEPTH {
        return Err(refused(format!(
            "Die Mindmap ist {deepest} Ebenen tief; einlesen lassen sich höchstens {MAX_TREE_DEPTH}. Nichts angelegt."
        )));
    }
    let seeds: Vec<IdeaSeed> = tree.nodes.iter().map(seed).collect();
    let links: Vec<IdeaSeedLink> = tree
        .links
        .iter()
        .map(|l| IdeaSeedLink { from: l.from.clone(), to: l.to.clone(), note: l.note.clone() })
        .collect();
    let held = state.write_lock.lock().await;
    let (ideas, linked) = match state.store.idea_import_tree(work, parent, &seeds, &links, origin).await {
        Ok(done) => done,
        Err(StoreError::Idea(message)) => return Err(refused(format!("{message} Nichts angelegt."))),
        Err(e) => return Err(e.into()),
    };
    // Ein Commit für den ganzen Baum: Autor und Trailer wie bei jeder Aktion an einem Einfall (der erste neue
    // Einfall steht im Trailer `Baruch-Idea`), Betreff und Rumpf nennen den Umfang.
    if let Some(first) = ideas.first() {
        let first = state.store.idea_get(*first).await?;
        let mut request = mirror::describe_idea(&Idea { origin: origin.clone(), ..first }, "idea-import", "");
        request.summary =
            format!("Mindmap eingelesen: {} {}", ideas.len(), if ideas.len() == 1 { "Einfall" } else { "Einfälle" });
        let place = match parent {
            Some(parent) => format!("unter Einfall {parent}"),
            None => "als freie Hauptthemen am Werk".to_owned(),
        };
        let numbers: Vec<String> = ideas.iter().map(ToString::to_string).collect();
        let title = tree.title.split_whitespace().collect::<Vec<_>>().join(" ");
        request.body = format!(
            "{}{} {place}; Verbindungen: {linked}.\nEinfälle: {}\n",
            if title.is_empty() { String::new() } else { format!("Quelle: „{title}“\n") },
            request.summary,
            numbers.join(", ")
        );
        if let Err(e) = mirror::record_idea(state, &held, Some(work), request).await {
            // Die Einfälle stehen in der Datenbank; der Spiegel holt beim nächsten Commit auf.
            eprintln!("Git-Spiegel (Mindmap eingelesen): {e}");
        }
    }
    Ok(ImportReport { ideas, links: linked })
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
