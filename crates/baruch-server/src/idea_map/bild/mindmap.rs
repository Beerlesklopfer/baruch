// SPDX-License-Identifier: AGPL-3.0-or-later
//! Drawing of the mind map as SVG (contract Round 34, §S1.3 and DOM contract §6; owner mindmap_widget).
//!
//! One layout, in Rust: screen (`standalone: false`, fragment for the page and for
//! `GET /werke/{id}/karte/bild`) and export (`standalone: true`, standalone file for `.svg`/`.pdf`) come
//! from the same function [`render`]; the positions are computed by [`super::tree_layout`].
//!
//! DOM contract (short version, complete in `docs/Architektur.md`, „Runde 34 – Mindmap“):
//! - `<svg class="mm-svg">` with `<g class="mm-viewport">`; in it in this order branches (`path.mm-edge`), nearness
//!   (`path.mm-near`), relations (`g.mm-rel` with `path.mm-link`), the cloud (`g.mm-cloud` with
//!   `rect.mm-cloud-area`), the nodes (`g.mm-node`) and last the labels of the relations
//!   (`g.mm-link-labels` with `rect.mm-link-label-bg[data-link]` and `text.mm-link-label[data-link]` – drawn
//!   last; since Round 36 every label looks for a place where it overlaps no node and no other
//!   label, and carries an opaque background).
//! - Nodes: `g.mm-node.mm-<kind>` with `data-node`, `data-parent`, `data-depth`, `data-side` (not on root and
//!   cloud), `data-branch`, `data-editable`, `data-title`, `data-own` (stored title), ideas `data-idea`
//!   and with a relation `data-key`; cards of the cloud `mm-cloud-item` and `data-cloud="1"`.
//! - Fragment: colours only through classes (`mm-b0`…`mm-b7`, CSS variables in `karte.css`, light and dark), links
//!   (`a.mm-toggle`, `a.mm-section`, `a.mm-label-link`, `a.mm-findings`). Standalone: colours as attributes and
//!   an own `<style>` (no program knows the CSS variables of the page), no links (in a file relative
//!   addresses would lead nowhere) – in their place `<g>` with the same class.
//! - All texts and attributes from user data pass through [`esc`].

use std::fmt::Write as _;

use baruch_store::{IdeaId, IdeaStatus, LinkKind};

use super::tree_layout::{self, Layout, Placed, Side, findings_width, text_width};
use crate::idea_map::tree::{CloudHint, MindMap, Scene, TreeKind};

#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// true = standalone file (own style, xmlns), false = fragment for the page.
    pub standalone: bool,
    /// Ids of collapsed nodes ("n:<key>", "i:<id>").
    pub closed: Vec<String>,
    /// Query part of the filter for links in the picture (without leading "?").
    pub back: String,
    /// Ids of the children of the root that lie in the cloud ([`Scene::cloud`]); all others are main branches.
    pub cloud: Vec<String>,
    /// „n in der Wolke“ without filter; `None` = number of the cards shown.
    pub cloud_count: Option<u32>,
    /// Suggestions „passt zu Ast …“ for cards of the cloud.
    pub hints: Vec<CloudHint>,
}

impl RenderOptions {
    /// Options for the view `scene`: cloud, counter and suggestions come from it, the filter for the links too.
    pub fn of(scene: &Scene, standalone: bool, closed: Vec<String>) -> Self {
        RenderOptions {
            standalone,
            closed,
            back: crate::idea_map::bild::filter_query(&scene.map.filter),
            cloud: scene.cloud.clone(),
            cloud_count: Some(scene.cloud_count),
            hints: scene.hints.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rendered {
    pub svg: String,
    pub width: f64,
    pub height: f64,
}

/// Fixed branch colours of the standalone file (light background); the page takes `--mm-b0`…`--mm-b7` from
/// `karte.css`.
pub const BRANCH_COLORS: [&str; 8] =
    ["#2f6fd0", "#d9622b", "#2e9a5b", "#a244b8", "#b8900a", "#1d9aa6", "#d24a6a", "#6b7a8c"];
/// Light fill of the main branches in the standalone file (same order).
const BRANCH_TINTS: [&str; 8] =
    ["#e3edfb", "#fbe9df", "#e0f3e8", "#f3e4f7", "#f7f0d6", "#ddf2f4", "#fae3e9", "#e9ecf0"];
const STANDALONE_STYLE: &str = "text{font-family:'Noto Sans','DejaVu Sans',Arial,sans-serif;fill:#1c2330}\
.mm-work text{fill:#ffffff;font-weight:bold}.mm-part text,.mm-chapter text{font-weight:bold}\
.mm-link-label,.mm-cloud-head{fill:#4a5568}.mm-link-label{paint-order:stroke;stroke:#ffffff;stroke-width:3px}\
.mm-link-label.mm-contradiction{fill:#b3261e}\
.mm-done .mm-label{text-decoration:line-through;fill:#6b7280}.mm-ghost,.mm-dim{opacity:.45}";

/// Text escaped for XML content and attributes; control characters (not allowed in XML 1.0) are dropped.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\n' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// Number for coordinates: one decimal, without „.0“ (short and the same in every run).
fn n(value: f64) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").map(str::to_owned).unwrap_or(text)
}

fn cut(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > max {
        let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        text
    }
}

fn pad(p: &Placed) -> f64 {
    if p.cloud {
        return 9.0;
    }
    match p.depth {
        0 => 20.0,
        1 => 13.0,
        _ => 9.0,
    }
}

/// Point on the edge of the box of `p` in direction (tx, ty).
fn border(p: &Placed, tx: f64, ty: f64) -> (f64, f64) {
    let (cx, cy) = (p.cx(), p.cy());
    let (dx, dy) = (tx - cx, ty - cy);
    if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
        return (cx, cy);
    }
    let sx = if dx.abs() < 1e-6 { f64::MAX } else { (p.w / 2.0) / dx.abs() };
    let sy = if dy.abs() < 1e-6 { f64::MAX } else { (p.h / 2.0) / dy.abs() };
    let s = sx.min(sy).min(1.0);
    (cx + dx * s, cy + dy * s)
}

/// Height of the rectangle of a label (font 11) and air around the text.
const LINK_LABEL_H: f64 = 16.0;
const LINK_LABEL_PAD: f64 = 5.0;
/// Distance a label keeps to nodes and other labels.
const LINK_LABEL_GAP: f64 = 3.0;

/// Rectangle (top left, width, height).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl LabelBox {
    fn around(cx: f64, cy: f64, w: f64) -> LabelBox {
        LabelBox { x: cx - w / 2.0, y: cy - LINK_LABEL_H / 2.0, w, h: LINK_LABEL_H }
    }

    /// Does the rectangle overlap `other` if `other` grows by `gap` all around?
    pub fn hits(&self, other: &LabelBox, gap: f64) -> bool {
        self.x < other.x + other.w + gap
            && other.x - gap < self.x + self.w
            && self.y < other.y + other.h + gap
            && other.y - gap < self.y + self.h
    }
}

/// Place for the label of a relation (Round 36): a rectangle of width `w` that overlaps no node and no
/// label already placed and stays in the picture. The search goes first along the line (`curve`:
/// start, control point, end of the quadratic curve) with growing distance above and below it, then in rings around
/// the middle of the line. `None` if the picture has no free place of this size.
fn free_label_spot(
    curve: [(f64, f64); 3],
    w: f64,
    nodes: &[LabelBox],
    taken: &[LabelBox],
    (width, height): (f64, f64),
) -> Option<LabelBox> {
    let free = |spot: &LabelBox| {
        spot.x >= 1.0
            && spot.y >= 1.0
            && spot.x + spot.w <= width - 1.0
            && spot.y + spot.h <= height - 1.0
            && !nodes.iter().chain(taken).any(|other| spot.hits(other, LINK_LABEL_GAP))
    };
    let at = |t: f64| {
        let ((x1, y1), (cx, cy), (x2, y2)) = (curve[0], curve[1], curve[2]);
        let (a, b, c) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
        (a * x1 + b * cx + c * x2, a * y1 + b * cy + c * y2)
    };
    for t in [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8] {
        let (px, py) = at(t);
        for step in 0..=3 {
            for sign in [-1.0, 1.0] {
                let spot = LabelBox::around(px, py - 3.0 + sign * f64::from(step) * (LINK_LABEL_H + 2.0), w);
                if free(&spot) {
                    return Some(spot);
                }
            }
        }
    }
    let (mx, my) = at(0.5);
    let reach = width.max(height);
    let mut radius = LINK_LABEL_H;
    while radius <= reach {
        for k in 0..24 {
            let angle = f64::from(k) * std::f64::consts::TAU / 24.0;
            let spot = LabelBox::around(mx + radius * angle.cos(), my + radius * angle.sin(), w);
            if free(&spot) {
                return Some(spot);
            }
        }
        radius += LINK_LABEL_H;
    }
    None
}

/// Address “toggle collapsed” (without JavaScript): same filter, `zu=` with or without `id`.
fn toggle_href(layout: &Layout, back: &str, id: &str) -> String {
    let mut closed: Vec<&str> = layout.closed.iter().copied().filter(|c| *c != id).collect();
    if closed.len() == layout.closed.len() {
        closed.push(id);
    }
    let mut parts = Vec::new();
    if !back.is_empty() {
        parts.push(back.to_owned());
    }
    if !closed.is_empty() {
        parts.push(format!("zu={}", closed.join(",")));
    }
    format!("?{}", parts.join("&"))
}

/// Opening and closing tag of a link: in the fragment `<a>`, standalone `<g>` with the same class.
fn link_tag(standalone: bool, class: &str, href: &str, extra: &str) -> (String, &'static str) {
    if standalone {
        (format!("<g class=\"{class}\"{extra}>"), "</g>")
    } else {
        (format!("<a class=\"{class}\" href=\"{}\"{extra}>", esc(href)), "</a>")
    }
}

fn node_svg(out: &mut String, map: &MindMap, layout: &Layout, p: &Placed, options: &RenderOptions) {
    let node = p.node;
    let alone = options.standalone;
    let kind = match node.kind {
        TreeKind::Work => "work",
        TreeKind::Part => "part",
        TreeKind::Chapter => "chapter",
        TreeKind::Idea => "idea",
    };
    let searching = map.filter.q.is_some();
    let mut class = format!("mm-node mm-{kind}");
    if let Some(b) = p.branch {
        let _ = write!(class, " mm-b{b}");
    }
    if p.depth == 1 && !p.cloud {
        class.push_str(" mm-main");
    }
    if p.cloud {
        class.push_str(" mm-cloud-item");
    }
    if node.status == Some(IdeaStatus::Done) {
        class.push_str(" mm-done");
    }
    if searching && node.hit {
        class.push_str(" mm-hit");
    }
    if searching && !node.hit && node.kind == TreeKind::Idea {
        class.push_str(" mm-dim");
    }
    if node.ghost {
        class.push_str(" mm-ghost");
    }
    if p.closed {
        class.push_str(" mm-closed");
    }
    if node.open_findings as usize >= crate::findings::page::MANY_OPEN {
        class.push_str(" mm-many-findings");
    }
    let _ = write!(out, "<g class=\"{class}\" data-node=\"{}\"", esc(&node.id));
    if let Some(parent) = p.parent {
        let _ = write!(out, " data-parent=\"{}\"", esc(parent));
    }
    let _ = write!(out, " data-depth=\"{}\"", p.depth);
    match p.side {
        Side::Right => out.push_str(" data-side=\"right\""),
        Side::Left => out.push_str(" data-side=\"left\""),
        Side::Center => {}
    }
    if let Some(b) = p.branch {
        let _ = write!(out, " data-branch=\"{b}\"");
    }
    if p.cloud {
        out.push_str(" data-cloud=\"1\"");
    }
    let _ = write!(
        out,
        " data-editable=\"{}\" data-title=\"{}\" data-own=\"{}\"",
        u8::from(node.editable),
        esc(&node.title),
        esc(&node.own_title)
    );
    if let Some(id) = node.idea {
        let _ = write!(out, " data-idea=\"{id}\"");
    }
    if let Some(key) = node.key {
        let _ = write!(out, " data-key=\"{key}\"");
    }
    if p.below > 0 {
        let _ = write!(out, " data-children=\"{}\"", p.below);
    }
    if node.open_findings > 0 {
        let _ = write!(out, " data-open-findings=\"{}\"", node.open_findings);
    }
    if let Some((id, ..)) = &p.fits {
        let _ = write!(out, " data-fits=\"{}\"", esc(id));
    }
    out.push_str(" tabindex=\"0\">");

    // Tooltip: full title, tags.
    let mut tip = node.title.clone();
    if !node.words.is_empty() {
        let _ = write!(tip, " · {}", node.words.join(", "));
    }
    let _ = write!(out, "<title>{}</title>", esc(&cut(&tip, 300)));

    // Box.
    let rx = if p.depth == 0 { p.h / 2.0 } else { 7.0 };
    let paint = match (alone, p.branch, p.depth) {
        (false, ..) => " fill=\"none\"".to_owned(),
        (true, _, 0) => " fill=\"#1c2330\" stroke=\"#1c2330\"".to_owned(),
        (true, Some(b), 1) => {
            format!(" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.6\"", BRANCH_TINTS[b], BRANCH_COLORS[b])
        }
        (true, Some(b), _) => format!(" fill=\"#ffffff\" stroke=\"{}\" stroke-width=\"1.1\"", BRANCH_COLORS[b]),
        (true, None, _) => {
            " fill=\"#ffffff\" stroke=\"#8a94a3\" stroke-width=\"1.1\" stroke-dasharray=\"4 3\"".to_owned()
        }
    };
    let _ = write!(
        out,
        "<rect class=\"mm-box\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{paint}/>",
        n(p.x),
        n(p.y),
        n(p.w),
        n(p.h),
        n(rx)
    );

    // Title (without JavaScript a link: idea → page of the idea, part/chapter → section in the work).
    let tx = p.x + pad(p);
    let with_hint = p.cloud && p.fits.is_some();
    let ty = if with_hint { p.y + 17.0 } else { p.cy() + p.font * 0.36 };
    let text = format!(
        "<text class=\"mm-label\" x=\"{}\" y=\"{}\" font-size=\"{}\">{}</text>",
        n(tx),
        n(ty),
        n(p.font),
        esc(&p.label)
    );
    let target = match (node.idea, node.key) {
        (Some(id), _) => Some(format!("/einfaelle/{id}")),
        (None, Some(key)) => Some(format!("/?work={}&key={key}", map.work)),
        _ => None,
    };
    match target {
        Some(href) if !alone => {
            let _ = write!(out, "<a class=\"mm-label-link\" href=\"{}\">{text}</a>", esc(&href));
        }
        _ => out.push_str(&text),
    }
    let mut cursor = tx + tree_layout::label_width(p.node, &p.label, p.font) + 4.0;
    let mid = if with_hint { p.y + 13.0 } else { p.cy() };

    // Note symbol (sheet with lines).
    if node.has_note {
        let stroke = if alone { " stroke=\"#4a5568\"" } else { "" };
        let tip = if node.note.trim().is_empty() {
            "Notiz, Zitat oder Quelle vorhanden".to_owned()
        } else {
            cut(&node.note, 300)
        };
        let _ = write!(
            out,
            "<g class=\"mm-note\" transform=\"translate({},{})\"><title>{}</title>\
             <path d=\"M0 0h8l3 3v10h-11zM2.5 5h6M2.5 7.5h6M2.5 10h4\" fill=\"none\" stroke-width=\"1\"{stroke}/></g>",
            n(cursor + 2.0),
            n(mid - 6.5),
            esc(&tip)
        );
        cursor += 18.0;
    }

    // Relation: link to the section.
    if let (TreeKind::Idea, Some(key)) = (node.kind, node.key) {
        let (open, close) =
            link_tag(alone, "mm-section", &format!("/?work={}&key={key}", map.work), &format!(" data-key=\"{key}\""));
        let tip =
            node.section_title.as_deref().map_or_else(|| "Zum Abschnitt".to_owned(), |t| format!("Abschnitt: {t}"));
        let _ = write!(
            out,
            "{open}<title>{}</title><text x=\"{}\" y=\"{}\" font-size=\"12\">§</text>{close}",
            esc(&tip),
            n(cursor + 3.0),
            n(mid + 4.0)
        );
        cursor += 16.0;
    }

    // Layer findings: number on the part/chapter.
    if node.open_findings > 0 {
        let href = node.key.map_or_else(
            || format!("/werke/{}/befunde", map.work),
            |key| format!("/werke/{}/befunde?key={key}#zelle", map.work),
        );
        let (open, close) =
            link_tag(alone, "mm-findings", &href, &format!(" data-open-findings=\"{}\"", node.open_findings));
        let w = findings_width(node.open_findings);
        let fill = if alone { " fill=\"#fff3cd\" stroke=\"#b8900a\"" } else { "" };
        let _ = write!(
            out,
            "{open}<title>{k} offene Befunde samt Unterpunkten</title>\
             <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"16\" rx=\"8\"{fill}/>\
             <text x=\"{}\" y=\"{}\" font-size=\"11\" text-anchor=\"middle\">{k} offen</text>{close}",
            n(cursor + 2.0),
            n(mid - 8.0),
            n(w),
            n(cursor + 2.0 + w / 2.0),
            n(mid + 4.0),
            k = node.open_findings
        );
    }

    if p.cloud {
        // Cloud: number of sub-nodes and the suggestion from the nearness.
        if p.below > 0 {
            let _ = write!(
                out,
                "<text class=\"mm-cloud-below\" x=\"{}\" y=\"{}\" font-size=\"11\" text-anchor=\"end\">+ {}</text>",
                n(p.x + p.w - 7.0),
                n(mid + 4.0),
                p.below
            );
        }
        if let Some((_, title, branch)) = &p.fits {
            let fill = if alone { format!(" fill=\"{}\"", BRANCH_COLORS[*branch]) } else { String::new() };
            let _ = write!(
                out,
                "<text class=\"mm-cloud-hint mm-b{branch}\" x=\"{}\" y=\"{}\" font-size=\"10.5\"{fill}>\
                 passt zu Ast „{}“</text>",
                n(tx),
                n(p.y + p.h - 7.0),
                esc(title)
            );
        }
    } else if p.depth > 0 && p.below > 0 {
        // Collapser at the outer edge: „−“ (open) or „+ n“ (closed).
        let label = if p.closed { format!("+ {}", p.below) } else { "−".to_owned() };
        let w = if p.closed { text_width(&label, 11.0) + 10.0 } else { 16.0 };
        let x = if p.side == Side::Left { p.x - 4.0 - w } else { p.x + p.w + 4.0 };
        let (open, close) = link_tag(
            alone,
            "mm-toggle",
            &toggle_href(layout, &options.back, &node.id),
            &format!(" data-count=\"{}\"", p.below),
        );
        let tip =
            if p.closed { format!("{} Unterknoten aufklappen", p.below) } else { "Unterknoten zuklappen".to_owned() };
        let paint = if alone { " fill=\"#ffffff\" stroke=\"#8a94a3\"" } else { "" };
        let _ = write!(
            out,
            "{open}<title>{tip}</title><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"16\" rx=\"8\"{paint}/>\
             <text x=\"{}\" y=\"{}\" font-size=\"11\" text-anchor=\"middle\">{label}</text>{close}",
            n(x),
            n(p.cy() - 8.0),
            n(w),
            n(x + w / 2.0),
            n(p.cy() + 4.0)
        );
    }
    out.push_str("</g>");
}

pub fn render(map: &MindMap, options: &RenderOptions) -> Rendered {
    let layout = tree_layout::layout_with(map, &options.closed, &options.cloud, &options.hints, options.cloud_count);
    let alone = options.standalone;
    let (width, height) = (layout.width, layout.height);
    let mut svg = String::new();
    let xmlns = if alone { " xmlns=\"http://www.w3.org/2000/svg\"" } else { "" };
    let _ = write!(
        svg,
        "<svg{xmlns} class=\"mm-svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" \
         aria-label=\"{}\" data-cloud-count=\"{}\" data-zu=\"{}\">",
        esc(&format!("Mindmap: {}", map.title)),
        layout.cloud.count,
        esc(&layout.closed.join(",")),
        w = n(width),
        h = n(height),
    );
    if alone {
        let _ = write!(svg, "<style>{STANDALONE_STYLE}</style>");
    }
    let arrow = if alone { " fill=\"#4a5568\"" } else { "" };
    let _ = write!(
        svg,
        "<defs><marker id=\"mm-arrow\" class=\"mm-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
         markerWidth=\"7\" markerHeight=\"7\" orient=\"auto\"><path d=\"M0 0L10 5L0 10z\"{arrow}/></marker></defs>"
    );
    if alone {
        let _ =
            write!(svg, "<rect class=\"mm-paper\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>", n(width), n(height));
    }
    svg.push_str("<g class=\"mm-viewport\">");

    // Branches.
    for p in layout.items.iter().filter(|p| !p.cloud) {
        let Some(parent) = p.parent.and_then(|id| layout.visible_of.get(id)).map(|at| &layout.items[*at]) else {
            continue;
        };
        let (x1, x2) = if p.side == Side::Left { (parent.x, p.x + p.w) } else { (parent.x + parent.w, p.x) };
        let (y1, y2) = (parent.cy(), p.cy());
        let mx = (x1 + x2) / 2.0;
        let stroke_width = match p.depth {
            1 => 3.6,
            2 => 2.4,
            3 => 1.8,
            _ => 1.4,
        };
        let branch = p.branch.unwrap_or(0);
        let stroke = if alone { format!(" stroke=\"{}\"", BRANCH_COLORS[branch]) } else { String::new() };
        let _ = write!(
            svg,
            "<path class=\"mm-edge mm-b{branch}\" data-from=\"{}\" data-to=\"{}\" d=\"M{} {}C{} {} {} {} {} {}\" \
             fill=\"none\" stroke-width=\"{stroke_width}\" stroke-linecap=\"round\"{stroke}/>",
            esc(&parent.node.id),
            esc(&p.node.id),
            n(x1),
            n(y1),
            n(mx),
            n(y1),
            n(mx),
            n(y2),
            n(x2),
            n(y2)
        );
    }

    let shown = |idea: IdeaId| layout.visible_of.get(format!("i:{idea}").as_str()).map(|at| &layout.items[*at]);

    // Nearness (only when switched on): dotted lines between nodes of the tree. For cards of the cloud, instead of
    // a line across the picture, the suggestion „passt zu Ast …“ stands at the card.
    if map.filter.near {
        for pair in &map.near.pairs {
            let (Some(a), Some(b)) = (shown(pair.a), shown(pair.b)) else { continue };
            if std::ptr::eq(a, b) || a.cloud || b.cloud {
                continue;
            }
            let (x1, y1) = border(a, b.cx(), b.cy());
            let (x2, y2) = border(b, a.cx(), a.cy());
            let stroke = if alone { " stroke=\"#8a94a3\"" } else { "" };
            let _ = write!(
                svg,
                "<path class=\"mm-near\" data-from=\"i:{}\" data-to=\"i:{}\" d=\"M{} {}L{} {}\" fill=\"none\" \
                 stroke-width=\"1\" stroke-dasharray=\"1.5 4\" stroke-linecap=\"round\"{stroke}/>",
                pair.a,
                pair.b,
                n(x1),
                n(y1),
                n(x2),
                n(y2)
            );
        }
    }

    let mut labels = String::new();
    // Round 36: rectangles of all drawn nodes and of the labels already placed – a label
    // never lies on a node and never on another label.
    let node_boxes: Vec<LabelBox> = layout.items.iter().map(|p| LabelBox { x: p.x, y: p.y, w: p.w, h: p.h }).collect();
    let mut label_boxes: Vec<LabelBox> = Vec::new();
    // Relations: curved lines with label (kind, plus the note); hidden ends at the visible ancestor.
    for (index, link) in map.links.iter().filter(|l| l.deleted.is_none()).enumerate() {
        let (Some(a), Some(b)) = (shown(link.from), shown(link.to)) else { continue };
        if std::ptr::eq(a, b) {
            continue;
        }
        let contradiction = link.kind == LinkKind::Contradicts;
        let (dx, dy) = (b.cx() - a.cx(), b.cy() - a.cy());
        // Bend: changes side with the position in the list so that several lines do not lie on each other.
        let bend = (0.16 + 0.06 * (index % 3) as f64) * if index % 2 == 0 { 1.0 } else { -1.0 };
        let (cx, cy) = ((a.cx() + b.cx()) / 2.0 - dy * bend, (a.cy() + b.cy()) / 2.0 + dx * bend);
        let (x1, y1) = border(a, cx, cy);
        let (x2, y2) = border(b, cx, cy);
        let (lx, ly) = (0.25 * x1 + 0.5 * cx + 0.25 * x2, 0.25 * y1 + 0.5 * cy + 0.25 * y2);
        let label = if link.note.trim().is_empty() {
            link.kind.label().to_owned()
        } else {
            format!("{}: {}", link.kind.label(), cut(&link.note, 60))
        };
        let color = if contradiction { "#b3261e" } else { "#4a5568" };
        let dash = match link.kind {
            LinkKind::LeadsTo => "",
            LinkKind::BelongsTo => " stroke-dasharray=\"7 4\"",
            LinkKind::ExampleOf => " stroke-dasharray=\"2 4\"",
            LinkKind::Contradicts => " stroke-dasharray=\"8 3 2 3\"",
        };
        let marker = if link.kind.symmetric() { "" } else { " marker-end=\"url(#mm-arrow)\"" };
        let stroke = if alone { format!(" stroke=\"{color}\"") } else { String::new() };
        let extra = if contradiction { " mm-contradiction" } else { "" };
        let _ = write!(
            svg,
            "<g class=\"mm-rel{extra}\"><path class=\"mm-link mm-link-{kind}{extra}\" data-link=\"{id}\" \
             data-from=\"i:{from}\" data-to=\"i:{to}\" data-end-from=\"{ef}\" data-end-to=\"{et}\" \
             d=\"M{} {}Q{} {} {} {}\" fill=\"none\" stroke-width=\"1.5\"{dash}{marker}{stroke}/></g>",
            n(x1),
            n(y1),
            n(cx),
            n(cy),
            n(x2),
            n(y2),
            kind = link.kind.as_str(),
            id = link.id,
            from = link.from,
            to = link.to,
            ef = esc(&a.node.id),
            et = esc(&b.node.id),
        );
        // The label stands in its own group at the end (drawn above lines and nodes). Round 36: it
        // looks for a place where it overlaps no node and no other label, and carries
        // an opaque background (`rect.mm-link-label-bg`) so that crossing lines do not strike through it. If
        // no free place is found in the whole picture, it stays at the middle of the line (`data-crowded`).
        let label_w = text_width(&label, if contradiction { 12.0 } else { 11.0 }) + 2.0 * LINK_LABEL_PAD;
        let spot = free_label_spot([(x1, y1), (cx, cy), (x2, y2)], label_w, &node_boxes, &label_boxes, (width, height));
        let crowded = if spot.is_none() { " data-crowded=\"1\"" } else { "" };
        let spot = spot.unwrap_or_else(|| LabelBox::around(lx, ly - 3.0, label_w));
        label_boxes.push(spot);
        let ground = if alone { " fill=\"#ffffff\"" } else { "" };
        let _ = write!(
            labels,
            "<rect class=\"mm-link-label-bg\" data-link=\"{id}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" \
             rx=\"3\"{ground}{crowded}/>\
             <text class=\"mm-link-label{extra}\" data-link=\"{id}\" x=\"{}\" y=\"{}\" font-size=\"11\" \
             text-anchor=\"middle\">{}</text>",
            n(spot.x),
            n(spot.y),
            n(spot.w),
            n(spot.h),
            n(spot.x + spot.w / 2.0),
            n(spot.y + spot.h - 4.5),
            esc(&label),
            id = link.id,
        );
    }

    // Cloud: storage of the ideas without a place (below the tree), also the drop area when dragging.
    let c = layout.cloud;
    let paint = if alone { " fill=\"#f6f7f9\" stroke=\"#8a94a3\"" } else { " fill=\"none\"" };
    let head = match c.count {
        0 => "Wolke – leer. Einfälle ohne Platz in der Mindmap liegen hier.".to_owned(),
        k => format!("Wolke – {k} in der Wolke"),
    };
    let _ = write!(
        svg,
        "<g class=\"mm-cloud\" data-count=\"{}\"><rect class=\"mm-cloud-area\" x=\"{}\" y=\"{}\" width=\"{}\" \
         height=\"{}\" rx=\"18\" stroke-dasharray=\"6 5\"{paint}/>\
         <text class=\"mm-cloud-head\" x=\"{}\" y=\"{}\" font-size=\"12.5\">{head}</text></g>",
        c.count,
        n(c.x),
        n(c.y),
        n(c.w),
        n(c.h),
        n(c.x + 14.0),
        n(c.y + 20.0),
    );

    for p in &layout.items {
        node_svg(&mut svg, map, &layout, p, options);
    }
    let _ = write!(svg, "<g class=\"mm-link-labels\">{labels}</g>");
    svg.push_str("</g></svg>");
    Rendered { svg, width, height }
}
