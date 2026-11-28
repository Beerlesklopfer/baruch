// SPDX-License-Identifier: AGPL-3.0-or-later
//! The mind map of a work as a tree (contract Round 34, interface R34-S1 §S1.2; afterwards owner mindmap_daten).
//!
//! [`tree`] returns the work as the root (`w`), below it parts and chapters (`n:<key>`) and the ideas (`i:<id>`).
//! Signatures and field names of this file are the interface of the round: `bild::mindmap` (mindmap_widget) and
//! `files` (mindmap_austausch) program against them; they do not change (additional fields only at the end).
//!
//! Since mindmap_daten (migration 0052): ideas carry title, parent node and order; [`tree`] places them by
//! one rule (see there), filters leave ancestors of visible ideas standing as ghosts, [`import`] creates an
//! imported tree in one transaction with one mirror commit.
//!
//! Addendum of the user of 2026-11-25 („eine Verbindung zwischen der Wolke und der Mindmap“): ideas without a place
//! – without parent node, without (valid) relation and not hung on the work as a main topic – stand **not** in the
//! tree but in the cloud next to it ([`Cloud`], [`cloud`], [`tree_and_cloud`]; RPC `idea.tree` returns both). The
//! signatures from S1.2 stayed unchanged for it; the cloud is an additional function.

use std::collections::{HashMap, HashSet};

use baruch_store::{Idea, IdeaId, IdeaLink, IdeaOrigin, IdeaSeed, IdeaSeedLink, IdeaStatus, StoreError, WorkId};

use crate::rpc::{REFUSED, RpcError};
use crate::{AppState, mirror};

pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_TREE_DEPTH: usize = 32;
pub const MAX_IMPORT_NODES: usize = 2000;
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
    /// "w", "n:<key>" or "i:<id>".
    pub id: String,
    pub kind: TreeKind,
    pub idea: Option<IdeaId>,
    /// Outline item: own key; idea: relation (section), otherwise None.
    pub key: Option<i64>,
    pub number: String,
    /// Displayed title, never empty.
    pub title: String,
    /// Stored title of the idea (may be empty); empty for outline items.
    pub own_title: String,
    pub note: String,
    /// true if note, quote, source or bible carry text.
    pub has_note: bool,
    pub words: Vec<String>,
    pub status: Option<IdeaStatus>,
    pub origin: Option<IdeaOrigin>,
    /// Title of the section an idea with key refers to.
    pub section_title: Option<String>,
    /// Ideas only: can be renamed, re-hung, deleted.
    pub editable: bool,
    /// Hit of the search q.
    pub hit: bool,
    /// Does not match the filter but stays because of visible descendants.
    pub ghost: bool,
    /// Open findings (only part/chapter, only with filter.findings).
    pub open_findings: u32,
    /// Descendants of kind idea (for „± n“).
    pub descendants: u32,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MindMap {
    pub work: WorkId,
    pub title: String,
    pub root: TreeNode,
    /// Links between ideas shown (not deleted).
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
    /// Depth-first search in display order: (node, depth; root = 0).
    pub fn walk(&self) -> Vec<(&TreeNode, usize)> {
        let mut out = Vec::new();
        walk_into(&self.root, 0, &mut out);
        out
    }

    pub fn find(&self, id: &str) -> Option<&TreeNode> {
        find_in(&self.root, id)
    }
}

/// “Number title” of a section (without a number only the title).
fn numbered(number: &str, title: &str) -> String {
    if number.is_empty() { title.to_owned() } else { format!("{number} {title}") }
}

/// Suggestion of the cloud: the idea `idea` (in the cloud) is near the node `node` of the mind map – „passt zu Ast
/// <branch_title>“. Display only, never stored (nearness as on the map: stored vectors and equal
/// tags, [`super::near`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CloudHint {
    /// Idea of the cloud (top level) the suggestion is for.
    pub idea: IdeaId,
    /// Near idea in the mind map (`i:<id>`) under which the idea could be hung.
    pub node: String,
    pub node_title: String,
    /// Main branch (child of the root) in which `node` hangs.
    pub branch: String,
    pub branch_title: String,
    pub score: f32,
    pub shared_words: Vec<String>,
}

/// The cloud: storage next to the mind map for ideas without a place (addendum of the user of 2026-11-25) – without
/// parent node, without (valid) relation and not hung on the work as a main topic. An idea of the cloud keeps its
/// sub-nodes (`children`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Cloud {
    pub work: WorkId,
    /// Ideas of the cloud by `position`, then `id`; with a filter only matching ones (and ghosts above matching
    /// sub-nodes).
    pub ideas: Vec<TreeNode>,
    /// „n in der Wolke“: ideas of the top level of the cloud, without filter.
    pub count: u32,
    /// The same with their sub-nodes, without filter.
    pub count_with_children: u32,
    /// Per idea of the cloud at most one suggestion (the nearest idea in the mind map), by idea.
    pub hints: Vec<CloudHint>,
    /// Links (not deleted) of ideas shown of which at least one lies in the cloud.
    pub links: Vec<IdeaLink>,
}

/// Mind map and cloud as **one view** for page, picture, outline view and exports (integration Round 34):
/// the ideas of the cloud hang here behind the main branches on the root of `map` so that `walk`/`find` know
/// them; which children of the root lie in the cloud is told by `cloud` (ids `i:<id>`). All other children of the
/// root – parts, chapters and free main topics – are main branches. `map.links` additionally carries the links
/// with one end in the cloud. For display only; RPC `idea.tree` still returns `{map, cloud}` separately.
#[derive(Debug, Clone)]
pub struct Scene {
    pub map: MindMap,
    /// Ids of the children of the root that lie in the cloud (in display order).
    pub cloud: Vec<String>,
    /// „n in der Wolke“ without filter ([`Cloud::count`]).
    pub cloud_count: u32,
    pub hints: Vec<CloudHint>,
}

impl Scene {
    /// Hangs the cloud on the root of the mind map (see [`Scene`]).
    pub fn join(mut map: MindMap, cloud: Cloud) -> Scene {
        let ids: Vec<String> = cloud.ideas.iter().map(|n| n.id.clone()).collect();
        map.root.children.extend(cloud.ideas);
        for link in cloud.links {
            if !map.links.iter().any(|l| l.id == link.id) {
                map.links.push(link);
            }
        }
        map.links.sort_by_key(|l| l.id);
        Scene { map, cloud: ids, cloud_count: cloud.count, hints: cloud.hints }
    }

    /// A mind map without cloud (hand-built trees in tests).
    pub fn without_cloud(map: MindMap) -> Scene {
        Scene { map, cloud: Vec::new(), cloud_count: 0, hints: Vec::new() }
    }

    pub fn in_cloud(&self, id: &str) -> bool {
        self.cloud.iter().any(|c| c == id)
    }

    /// The main branches: parts, chapters and free main topics – without the cloud.
    pub fn branches(&self) -> Vec<&TreeNode> {
        self.map.root.children.iter().filter(|n| !self.in_cloud(&n.id)).collect()
    }

    /// The ideas of the cloud (top level), with their sub-nodes.
    pub fn cloud_nodes(&self) -> Vec<&TreeNode> {
        self.map.root.children.iter().filter(|n| self.in_cloud(&n.id)).collect()
    }
}

/// Mind map and cloud as one view ([`Scene`]).
pub async fn scene(state: &AppState, work: WorkId, filter: &super::MapFilter) -> Result<Scene, RpcError> {
    let (map, cloud) = tree_and_cloud(state, work, filter).await?;
    Ok(Scene::join(map, cloud))
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

/// What [`assemble`] knows about the ideas.
struct Ideas<'a> {
    by_id: HashMap<IdeaId, &'a Idea>,
    /// Sub-nodes per idea, by `position`, then `id`.
    below: HashMap<IdeaId, Vec<IdeaId>>,
    /// Matches the filter (value = hit of the search `q`).
    shown: HashMap<IdeaId, bool>,
    section_titles: &'a HashMap<i64, String>,
}

impl Ideas<'_> {
    /// The idea as a node with sub-nodes; `None` if neither it nor a descendant matches the filter.
    fn node(&self, id: IdeaId, depth: usize) -> Option<TreeNode> {
        let idea = self.by_id.get(&id)?;
        let mut children = Vec::new();
        // The store keeps the chain at most MAX_TREE_DEPTH deep; the limit here only protects against broken data.
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

/// Counts the ideas shown (without ghosts) in a subtree.
fn shown_ideas(node: &TreeNode) -> u32 {
    u32::from(node.kind == TreeKind::Idea && !node.ghost) + node.children.iter().map(shown_ideas).sum::<u32>()
}

fn collect_ideas(node: &TreeNode, out: &mut HashSet<IdeaId>) {
    out.extend(node.idea);
    for child in &node.children {
        collect_ideas(child, out);
    }
}

/// Hangs the children on an outline item (first outline items in outline order, then its
/// ideas by `position`, then `id`) and counts the descendants of kind idea.
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

/// Builds mind map and cloud in one go. `near_for_hints`: compute the nearness even if the filter switches it off
/// for the picture (the suggestions of the cloud need it); the mind map then still reports “off”.
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

    // Outline items per parent node (order of `build` = outline order); plus per key the node
    // on which an idea with this relation hangs (anchor), and the title of the section.
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
            // The cloud „Ohne Kapitel“ of the flat map is the storage next to the mind map in the tree ([`Cloud`]).
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

    // Ideas: sub-nodes per parent node; a parent node that does not exist (not deleted, in the work) counts
    // like none.
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

    // Place of the top-level ideas: on the anchor of their relation, as a main topic on the work or in the cloud.
    let mut at_anchor: HashMap<String, Vec<TreeNode>> = HashMap::new();
    let mut cloud_ideas: Vec<TreeNode> = Vec::new();
    let (mut cloud_count, mut cloud_all) = (0u32, 0u32);
    for idea in top {
        let anchor = idea.key.and_then(|key| anchor_of_key.get(&key).cloned());
        let in_cloud = anchor.is_none() && !idea.main_topic;
        if in_cloud {
            // Counter without filter: the idea and everything below it.
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

    // Suggestions of the cloud: per idea of the top level the nearest pair (highest value, then smallest id) with
    // an idea of the mind map; the main branch is the child of the root in which that one hangs.
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

    // Nearness in the picture: only pairs whose two ideas both stand in the mind map; switched off stays switched off.
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
        super::Near { state: super::NearState::Off, message: super::near::off(), pairs: Vec::new() }
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

/// The mind map of the work as a tree (filter like [`super::build`]; `open` does not apply in the tree – sections
/// under a chapter are never nodes of their own, their title stands as `section_title` on the idea).
///
/// Place of an idea (one rule, the same everywhere):
/// 1. with a parent node (`parent`, not deleted, same work) sub-node of that idea, siblings by
///    `position`, then `id`; the relation (`key`) stays allowed besides and is then only the link to the section;
/// 2. without parent node, with relation: child of the anchor (nearest ancestor of kind chapter, else the item);
/// 3. without both, as a main topic (`main_topic`): directly on the work, behind parts and chapters;
/// 4. otherwise in the cloud – not in the tree but in [`cloud`].
///
/// Filter (`status`, `word`, `origin`): an idea that does not match stays as a ghost (`ghost`) as long as a
/// descendant matches; `q` hides nothing but sets `hit`. `links` and `near.pairs` name only ideas that stand in
/// the tree. `total_ideas` counts the ideas shown in the tree (without ghosts, without cloud).
pub async fn tree(state: &AppState, work: WorkId, filter: &super::MapFilter) -> Result<MindMap, RpcError> {
    Ok(assemble(state, work, filter, false).await?.0)
}

/// The cloud of the work ([`Cloud`]): ideas without a place in the mind map, with counter and suggestions „passt
/// zu Ast …“ (the nearness is always computed for it, even if the filter switches it off for the picture; never an
/// embedding request).
pub async fn cloud(state: &AppState, work: WorkId, filter: &super::MapFilter) -> Result<Cloud, RpcError> {
    Ok(assemble(state, work, filter, true).await?.1)
}

/// Mind map and cloud in one call (for pages that show both).
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
    /// Round 36: the node lay in the cloud of the own export (branch with the id `export::CLOUD_ID`) and goes
    /// back into the cloud. Applies only on the top level and only when importing without a target node; if the field
    /// is missing (older preview in the form), it is `false`.
    #[serde(default)]
    pub cloud: bool,
}

/// Path = indices from `ImportTree::nodes` downwards.
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

/// Nodes and greatest depth of an imported tree, without recursion (a very deep input must not burst the stack
/// before it is rejected).
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

/// Call only after [`import_size`]: the depth is then at most [`MAX_TREE_DEPTH`].
fn seed(node: &ImportNode) -> IdeaSeed {
    IdeaSeed {
        title: node.title.clone(),
        note: node.note.clone(),
        words: node.words.clone(),
        children: node.children.iter().map(seed).collect(),
    }
}

/// Number of nodes of a subtree, without recursion.
fn subtree_size(node: &ImportNode) -> usize {
    let (mut count, mut stack) = (0usize, vec![node]);
    while let Some(node) = stack.pop() {
        count += 1;
        stack.extend(node.children.iter());
    }
    count
}

/// Creates the tree as ideas of the work (under `parent` or as free main topics): one transaction,
/// one mirror commit with origin. Links as `leads_to` with the label as note.
///
/// The top-level nodes (`tree.nodes`) become sub-nodes of `parent` or – without it – free main topics on the work
/// (not ideas of the cloud: whoever imports a tree wants to see it in the mind map), in the order of the
/// input behind the existing siblings. `tree.title` and `tree.warnings` are not stored.
///
/// Round 36: top-level nodes with [`ImportNode::cloud`] (the branch „Wolke“ of an own export) go back into the
/// cloud without `parent`, with their sub-nodes – the round trip of the own file keeps the cloud. With `parent`
/// the flag does not count (there is no cloud under a target node).
///
/// Refused with [`REFUSED`] without creating anything: no node, more than [`MAX_IMPORT_NODES`] nodes, deeper than
/// [`MAX_TREE_DEPTH`] (also together with the depth of `parent`), `parent` deleted or in another work, a
/// title above [`MAX_TITLE_CHARS`] or a note above 4000 characters, tags above the limits of the ideas, a
/// link with a path without a node. A node without title and tag is called „(ohne Titel)“; a link
/// of a node with itself or a duplicate one is skipped (not counted). Unknown work → error as
/// for the other methods.
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
        return Err(refused(crate::i18n::tc("ideas_msg.import.no_node", &[])));
    }
    if count > MAX_IMPORT_NODES {
        return Err(refused(crate::i18n::tc(
            "ideas_msg.import.too_many",
            &[("count", count.into()), ("max", MAX_IMPORT_NODES.into())],
        )));
    }
    if deepest > MAX_TREE_DEPTH {
        return Err(refused(crate::i18n::tc(
            "ideas_msg.import.too_deep",
            &[("deepest", deepest.into()), ("max", MAX_TREE_DEPTH.into())],
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
        Err(StoreError::Idea(message)) => {
            return Err(refused(format!("{message} {}", crate::i18n::tc("ideas_msg.import.nothing_created", &[]))));
        }
        Err(e) => return Err(e.into()),
    };
    // Round 36: the ideas of the cloud back into the cloud. `ideas` is in depth-first order of the
    // input; so the i-th top-level node starts behind the subtrees of its predecessors. The store always creates
    // top-level nodes as main topics (`idea_import_tree`, file of sqlx09 in this round) – hence a
    // second step here under the same write lock and before the one mirror commit.
    let mut in_cloud = 0usize;
    if parent.is_none() {
        let mut offset = 0usize;
        for node in &tree.nodes {
            if node.cloud
                && let Some(id) = ideas.get(offset)
            {
                let to_cloud = baruch_store::IdeaMove { cloud: Some(true), ..Default::default() };
                state.store.idea_move(*id, &to_cloud).await?;
                in_cloud += 1;
            }
            offset += subtree_size(node);
        }
    }
    // One commit for the whole tree: author and trailers as for every action on an idea (the first new
    // idea stands in the trailer `Baruch-Idea`), subject and body name the extent.
    if let Some(first) = ideas.first() {
        let first = state.store.idea_get(*first).await?;
        let mut request = mirror::describe_idea(&Idea { origin: origin.clone(), ..first }, "idea-import", "");
        request.summary =
            format!("Mindmap eingelesen: {} {}", ideas.len(), if ideas.len() == 1 { "Einfall" } else { "Einfälle" });
        let place = match parent {
            Some(parent) => format!("unter Einfall {parent}"),
            None if in_cloud == 0 => "als freie Hauptthemen am Werk".to_owned(),
            None if in_cloud == tree.nodes.len() => "in der Wolke".to_owned(),
            None => format!("als freie Hauptthemen am Werk, davon {in_cloud} in der Wolke"),
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
            // The ideas are in the database; the mirror catches up with the next commit.
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
