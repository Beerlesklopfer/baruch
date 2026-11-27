// SPDX-License-Identifier: AGPL-3.0-or-later
//! Layout of the mind map (Round 34, mindmap_widget): work in the middle, main branches right and left, sub-branches
//! outwards, below it the cloud as storage.
//!
//! Purely computed and deterministic (no random numbers, no clock, no hash order in the output):
//! same input, same positions. The drawing ([`super::mindmap`]) reads only the result [`Layout`].
//!
//! Rules:
//! - **Main branches** are the children of the root except the cloud: parts, chapters and free main topics; the
//!   first half (rounded up) stands right, the second left. Every main branch gets one of the eight branch colours
//!   (`branch`), which all descendants inherit.
//! - **Cloud**: the children of the root that [`crate::idea_map::tree::Scene::cloud`] names (ideas without a place)
//!   are not main branches but lie as cards in the cloud below the tree (addendum of the user of
//!   2026-11-25). Their sub-nodes stay with them and are not drawn individually in the cloud („+ n“).
//! - **No overlap**: every subtree occupies its own vertical band (height = larger of node height and sum
//!   of the children's bands); children stand horizontally beyond the outer edge of their parent node.
//! - **Collapsed** (`closed`): the node is drawn, its descendants are not; [`Layout::visible_of`] names
//!   for every hidden node the visible ancestor (relations end there).

use std::collections::{HashMap, HashSet};

use crate::idea_map::tree::{CloudHint, MindMap, TreeKind, TreeNode};

/// Longest drawn title (characters); the full title stands in `data-title` and in the tooltip.
pub const MAX_LABEL_CHARS: usize = 48;
const MARGIN: f64 = 28.0;
/// Horizontal distance parent edge → child edge (room for curve and collapser).
const H_GAP: f64 = 52.0;
const H_GAP_ROOT: f64 = 70.0;
/// Room next to the outermost node for the collapser „+ n“.
const TOGGLE_ROOM: f64 = 46.0;
const CLOUD_GAP: f64 = 44.0;
const CLOUD_PAD: f64 = 14.0;
const CLOUD_HEAD: f64 = 30.0;
const CLOUD_MIN_WIDTH: f64 = 720.0;
const CHIP_GAP: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Center,
    Right,
    Left,
}

/// A drawn node with its rectangle (top left corner, width, height).
#[derive(Debug, Clone)]
pub struct Placed<'a> {
    pub node: &'a TreeNode,
    /// Id of the parent node (root: `None`).
    pub parent: Option<&'a str>,
    pub depth: usize,
    pub side: Side,
    /// Number of the branch colour 0..=7 (root and cloud: `None`).
    pub branch: Option<usize>,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Shortened title as it stands in the picture.
    pub label: String,
    pub font: f64,
    /// Has children and is collapsed.
    pub closed: bool,
    /// Number of all descendants (nodes of every kind) – the number at the collapser.
    pub below: u32,
    /// Lies in the cloud.
    pub cloud: bool,
    /// Cloud: id and title of the main branch the idea fits best by nearness.
    pub fits: Option<(&'a str, String, usize)>,
}

impl Placed<'_> {
    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }
}

/// Frame of the cloud.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct Layout<'a> {
    /// Root first, then the tree in display order, last the cards of the cloud.
    pub items: Vec<Placed<'a>>,
    pub width: f64,
    pub height: f64,
    pub cloud: CloudBox,
    /// Id of every node of the tree → position (in `items`) of the node that is visible for it.
    pub visible_of: HashMap<&'a str, usize>,
    /// Collapsed nodes in display order (only existing ones with children) – canonical for `zu=`.
    pub closed: Vec<&'a str>,
}

/// Estimated width of a text in the font of the UI (sans-serif). Deliberately a bit generous: rather a
/// box too wide than text sticking out (the server does not know the browser's font).
pub fn text_width(text: &str, font: f64) -> f64 {
    let units: f64 = text
        .chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | 't' | 'f' | 'r' | '.' | ',' | ';' | ':' | '!' | '|' | '\'' | ' ' | 'I' => 0.36,
            'm' | 'w' | 'M' | 'W' | '—' | '…' => 0.92,
            c if c.is_ascii_uppercase() => 0.72,
            c if c.is_ascii() => 0.6,
            c if (c as u32) >= 0x2E80 => 1.05,
            _ => 0.66,
        })
        .sum();
    units * font
}

/// Title for the picture: number in front (parts/chapters), shortened to [`MAX_LABEL_CHARS`], never empty.
pub fn label_of(node: &TreeNode) -> String {
    let title = node.title.split_whitespace().collect::<Vec<_>>().join(" ");
    let full = if node.number.trim().is_empty() || node.kind == TreeKind::Idea {
        title
    } else {
        format!("{} {}", node.number.trim(), title)
    };
    let full = if full.is_empty() { "(ohne Titel)".to_owned() } else { full };
    if full.chars().count() > MAX_LABEL_CHARS {
        let mut cut: String = full.chars().take(MAX_LABEL_CHARS - 1).collect();
        cut.push('…');
        cut
    } else {
        full
    }
}

/// Width of the title in the box: work, parts and chapters are bold and need more room than ideas.
pub fn label_width(node: &TreeNode, label: &str, font: f64) -> f64 {
    text_width(label, font) * if node.kind == TreeKind::Idea { 1.04 } else { 1.2 }
}

/// Width of the addition „k offen“ (layer findings).
pub fn findings_width(open: u32) -> f64 {
    text_width(&format!("{open} offen"), 11.0) + 12.0
}

/// Font, height and padding per depth.
fn style(depth: usize) -> (f64, f64, f64) {
    match depth {
        0 => (18.0, 46.0, 20.0),
        1 => (15.0, 34.0, 13.0),
        _ => (13.0, 26.0, 9.0),
    }
}

fn size(node: &TreeNode, depth: usize) -> (String, f64, f64, f64) {
    let (font, h, pad) = style(depth);
    let label = label_of(node);
    let mut w = label_width(node, &label, font) + 2.0 * pad;
    if node.has_note {
        w += 18.0;
    }
    if node.kind == TreeKind::Idea && node.key.is_some() {
        w += 16.0;
    }
    if node.open_findings > 0 {
        w += findings_width(node.open_findings) + 4.0;
    }
    (label, font, w.max(h), h)
}

fn v_gap(depth: usize) -> f64 {
    if depth <= 1 { 18.0 } else { 9.0 }
}

fn count_below(node: &TreeNode) -> u32 {
    node.children.iter().map(|c| 1 + count_below(c)).sum()
}

struct Builder<'a> {
    closed: HashSet<&'a str>,
    items: Vec<Placed<'a>>,
    visible_of: HashMap<&'a str, usize>,
    closed_seen: Vec<&'a str>,
}

impl<'a> Builder<'a> {
    fn is_closed(&self, node: &TreeNode) -> bool {
        !node.children.is_empty() && self.closed.contains(node.id.as_str())
    }

    /// Height of the vertical band of a subtree.
    fn extent(&self, node: &TreeNode, depth: usize) -> f64 {
        let (_, h, _) = style(depth);
        if node.children.is_empty() || self.is_closed(node) {
            return h;
        }
        let gap = v_gap(depth + 1);
        let below: f64 = node.children.iter().map(|c| self.extent(c, depth + 1)).sum::<f64>()
            + gap * (node.children.len() - 1) as f64;
        below.max(h)
    }

    fn hide(&mut self, node: &'a TreeNode, at: usize) {
        for child in &node.children {
            self.visible_of.insert(child.id.as_str(), at);
            self.hide(child, at);
        }
    }

    /// Puts `node` into its band from `top`; `edge` is the inner edge (right: left edge of the node).
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        node: &'a TreeNode,
        parent: &'a str,
        depth: usize,
        side: Side,
        branch: usize,
        edge: f64,
        top: f64,
    ) {
        let extent = self.extent(node, depth);
        let (label, font, w, h) = size(node, depth);
        let x = if side == Side::Left { edge - w } else { edge };
        let cy = top + extent / 2.0;
        let closed = self.is_closed(node);
        let at = self.items.len();
        self.items.push(Placed {
            node,
            parent: Some(parent),
            depth,
            side,
            branch: Some(branch),
            x,
            y: cy - h / 2.0,
            w,
            h,
            label,
            font,
            closed,
            below: count_below(node),
            cloud: false,
            fits: None,
        });
        self.visible_of.insert(node.id.as_str(), at);
        if closed {
            self.closed_seen.push(node.id.as_str());
            self.hide(node, at);
            return;
        }
        if node.children.is_empty() {
            return;
        }
        let gap = v_gap(depth + 1);
        let total: f64 = node.children.iter().map(|c| self.extent(c, depth + 1)).sum::<f64>()
            + gap * (node.children.len() - 1) as f64;
        let mut y = cy - total / 2.0;
        let child_edge = if side == Side::Left { x - H_GAP } else { x + w + H_GAP };
        for child in &node.children {
            let e = self.extent(child, depth + 1);
            self.place(child, node.id.as_str(), depth + 1, side, branch, child_edge, y);
            y += e + gap;
        }
    }
}

/// Layout without cloud: all children of the root are main branches.
pub fn layout<'a>(map: &'a MindMap, closed: &[String]) -> Layout<'a> {
    layout_with(map, closed, &[], &[], None)
}

/// Layout with cloud: `cloud` names the children of the root that lie in the cloud ([`Scene::cloud`]), `hints` the
/// suggestions „passt zu Ast …“, `count` the counter without filter (otherwise the number of cards).
pub fn layout_with<'a>(
    map: &'a MindMap,
    closed: &[String],
    cloud: &[String],
    hints: &[CloudHint],
    count: Option<u32>,
) -> Layout<'a> {
    let in_cloud: HashSet<&str> = cloud.iter().map(String::as_str).collect();
    let mut b = Builder {
        closed: closed.iter().filter_map(|id| map.find(id).map(|n| n.id.as_str())).collect(),
        items: Vec::new(),
        visible_of: HashMap::new(),
        closed_seen: Vec::new(),
    };
    let root = &map.root;
    let (label, font, w, h) = size(root, 0);
    b.items.push(Placed {
        node: root,
        parent: None,
        depth: 0,
        side: Side::Center,
        branch: None,
        x: -w / 2.0,
        y: -h / 2.0,
        w,
        h,
        label,
        font,
        closed: false,
        below: count_below(root),
        cloud: false,
        fits: None,
    });
    b.visible_of.insert(root.id.as_str(), 0);

    let mains: Vec<&TreeNode> = root.children.iter().filter(|c| !in_cloud.contains(c.id.as_str())).collect();
    let right = mains.len().div_ceil(2);
    let gap = v_gap(1);
    for (side, range, first) in [(Side::Right, 0..right, 0), (Side::Left, right..mains.len(), right)] {
        let group = &mains[range];
        if group.is_empty() {
            continue;
        }
        let total: f64 = group.iter().map(|c| b.extent(c, 1)).sum::<f64>() + gap * (group.len() - 1) as f64;
        let mut y = -total / 2.0;
        let edge = if side == Side::Left { -w / 2.0 - H_GAP_ROOT } else { w / 2.0 + H_GAP_ROOT };
        for (i, child) in group.iter().enumerate() {
            let e = b.extent(child, 1);
            b.place(child, root.id.as_str(), 1, side, (first + i) % 8, edge, y);
            y += e + gap;
        }
    }

    // Shift into the visible area (margin, room for collapsers outside).
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in &b.items {
        min_x = min_x.min(p.x - TOGGLE_ROOM);
        max_x = max_x.max(p.x + p.w + TOGGLE_ROOM);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y + p.h);
    }
    let tree_width = (max_x - min_x).max(CLOUD_MIN_WIDTH);
    // Narrow trees stand centred above the cloud.
    let dx = MARGIN - min_x + (tree_width - (max_x - min_x)) / 2.0;
    let dy = MARGIN - min_y;
    for p in &mut b.items {
        p.x += dx;
        p.y += dy;
    }
    let tree_bottom = max_y + dy;

    // Cloud: cards row by row below the tree.
    let cloud: Vec<&TreeNode> = root.children.iter().filter(|c| in_cloud.contains(c.id.as_str())).collect();
    let cloud_x = MARGIN;
    let cloud_y = tree_bottom + CLOUD_GAP;
    let inner = tree_width - 2.0 * CLOUD_PAD;
    let (mut x, mut y, mut row_h) = (0.0_f64, CLOUD_HEAD, 0.0_f64);
    for node in &cloud {
        let (label, font, mut w, mut h) = size(node, 2);
        // Suggestion of the nearness: the main branch in which the nearest idea of the mind map hangs.
        let fits = node.idea.and_then(|id| hints.iter().find(|hint| hint.idea == id)).and_then(|hint| {
            let at = mains.iter().position(|main| main.id == hint.branch)?;
            Some((mains[at].id.as_str(), label_of(mains[at]), at % 8))
        });
        if let Some((_, title, _)) = &fits {
            w = w.max(text_width(&format!("passt zu Ast „{title}“"), 10.5) + 18.0);
            h += 15.0;
        }
        let below = count_below(node);
        if below > 0 {
            w += text_width(&format!("+ {below}"), 11.0) + 8.0;
        }
        let w = w.min(inner);
        if x > 0.0 && x + w > inner {
            x = 0.0;
            y += row_h + CHIP_GAP;
            row_h = 0.0;
        }
        let at = b.items.len();
        b.items.push(Placed {
            node,
            parent: Some(root.id.as_str()),
            depth: 1,
            side: Side::Center,
            branch: None,
            x: cloud_x + CLOUD_PAD + x,
            y: cloud_y + y,
            w,
            h,
            label,
            font,
            closed: below > 0,
            below,
            cloud: true,
            fits,
        });
        b.visible_of.insert(node.id.as_str(), at);
        b.hide(node, at);
        x += w + CHIP_GAP;
        row_h = row_h.max(h);
    }
    // Empty cloud: one row of room for the note and as a drop area.
    let cloud_h = y + row_h.max(26.0) + CLOUD_PAD;
    let cloud_box = CloudBox {
        x: cloud_x,
        y: cloud_y,
        w: tree_width,
        h: cloud_h,
        count: count.map_or(cloud.len(), |c| c as usize),
    };

    Layout {
        items: b.items,
        width: tree_width + 2.0 * MARGIN,
        height: cloud_y + cloud_h + MARGIN,
        cloud: cloud_box,
        visible_of: b.visible_of,
        closed: b.closed_seen,
    }
}
