// SPDX-License-Identifier: AGPL-3.0-or-later
//! Radiale Anordnung der Gedankenkarte (Vertrag Runde 25 §2/§4.4, karte-bild): rein, deterministisch, ohne Zufall.
//!
//! - **Mitte** das Werk (`w`) bei (0,0); **Ringe** nach der Tiefe in der Gliederung der Karte: Ring 1 die obersten
//!   Knoten (Teile, Kapitel ohne Teil, Abschnitte ohne Kapitel, die Wolke „Ohne Kapitel“), Ring 2 deren Kinder
//!   (Kapitel unter einem Teil, aufgeklappte Abschnitte) usw. **Winkel** wie bei einem radialen Baum: Jeder
//!   oberste Knoten bekommt ab 12 Uhr im Uhrzeigersinn einen Sektor nach dem Platzbedarf seines Teilbaums, Kinder
//!   teilen den Sektor ihres Elternknotens in der Reihenfolge der Gliederung.
//! - **Traube:** Die Einfälle eines Knotens liegen auf einem festen Sechseckgitter um ihn (Abstand [`LATTICE`]),
//!   nächste Gitterpunkte zuerst; der Platz unter dem Knoten bleibt für seine Beschriftung frei. Jede Traube ist
//!   eine Scheibe mit Radius `Rc`.
//! - **Keine Überlappung** (nicht angeheftete Kreise): im Gitter hält jeder Einfall ≥ `2·IDEA_R + GAP` Abstand, zum
//!   eigenen Knoten ≥ `r + IDEA_R + GAP`; Trauben eines Rings liegen so weit außen, dass je zwei Scheiben
//!   ≥ `Rc₁ + Rc₂ + GAP` auseinander sind (der Radius wächst, bis das für **alle** Paare gilt); Ringe sind
//!   disjunkte Kreisringe. Damit gilt der Abstand ≥ r₁ + r₂ für jedes Kreispaar, nicht nur ungefähr.
//! - **Gespeicherte Lagen** (`IdeaMap::positions`) überstimmen die Anordnung: Der Einfall steht dort, wo er
//!   abgelegt wurde (`pinned`), und belegt keinen Gitterplatz.
//!
//! Aufwand: je Knoten ein sortierter Gitterausschnitt, je Ring eine Paarprüfung über die Knoten des Rings – für
//! 300 Einfälle weit unter 200 ms auch im Debug-Bau (Test `round25_bild`, Messwert in der Ausgabe).

use std::collections::HashMap;
use std::f64::consts::PI;

use baruch_store::IdeaId;

use crate::idea_map::{IdeaMap, MapNodeKind};

/// Radius eines Einfalls.
pub const IDEA_R: f64 = 13.0;
/// Mindestabstand zwischen zwei Kreisrändern.
pub const GAP: f64 = 8.0;
/// Abstand der Gitterpunkte einer Traube (Platz für die Beschriftung unter dem Kreis).
pub const LATTICE: f64 = 52.0;
/// Halbe Breite und Höhe des freien Streifens unter einem Knoten (seine Beschriftung).
const LABEL_HALF_WIDTH: f64 = 58.0;
const LABEL_HEIGHT: f64 = 24.0;

/// Radius eines Knotens der Gliederung.
pub fn node_radius(kind: MapNodeKind) -> f64 {
    match kind {
        MapNodeKind::Work => 56.0,
        MapNodeKind::Part => 36.0,
        MapNodeKind::Chapter => 30.0,
        MapNodeKind::Section => 24.0,
        MapNodeKind::NoChapter => 30.0,
    }
}

/// Ein platzierter Kreis (Mittelpunkt in viewBox-Einheiten, ohne Transform des Viewports).
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// `w`, `n:<key>`, `none` oder `i:<id>`.
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub r: f64,
    /// Gespeicherte Lage (nur Einfälle).
    pub pinned: bool,
    /// Tiefe in der Gliederung der Karte (Werk 0, Ring 1 …); Einfälle die Tiefe ihres Knotens.
    pub depth: usize,
}

/// Ergebnis der Anordnung.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Erst die Knoten in der Reihenfolge von `IdeaMap::nodes`, dann die Einfälle in der von `IdeaMap::ideas`.
    pub items: Vec<Placed>,
    index: HashMap<String, usize>,
    /// viewBox: x, y, Breite, Höhe (ganzzahlig gerundet, mit Rand für Beschriftungen).
    pub view: (f64, f64, f64, f64),
}

impl Layout {
    pub fn get(&self, id: &str) -> Option<&Placed> {
        self.index.get(id).map(|&i| &self.items[i])
    }
}

/// Gitterpunkt (axiale Koordinaten) mit ganzzahliger Norm `q² + qr + r²` (Abstand² / LATTICE²).
#[derive(Debug, Clone, Copy)]
struct Cell {
    norm: i64,
    x: f64,
    y: f64,
}

/// Gitterpunkte um einen Knoten mit Radius `r_anchor`, die frei sind (Abstand zum Knoten, Streifen der
/// Beschriftung), nach Norm sortiert; mindestens `n` Stück, und alle Punkte bis zur Norm des `n`-ten (damit die
/// Wahl im letzten Ring nur eine Frage der Richtung ist).
fn cells(n: usize, r_anchor: f64) -> Vec<Cell> {
    if n == 0 {
        return Vec::new();
    }
    let min = r_anchor + IDEA_R + GAP;
    let free = |x: f64, y: f64| {
        let far = x.hypot(y) >= min;
        let label = x.abs() < LABEL_HALF_WIDTH && y > r_anchor - 6.0 && y < r_anchor + LABEL_HEIGHT + IDEA_R;
        far && !label
    };
    let mut k = (n as f64).sqrt().ceil() as i64 + (min / LATTICE).ceil() as i64 + 2;
    loop {
        let mut found = Vec::new();
        for q in -k..=k {
            for r in -k..=k {
                let x = LATTICE * (q as f64 + r as f64 / 2.0);
                let y = LATTICE * (r as f64 * 3f64.sqrt() / 2.0);
                if free(x, y) {
                    found.push(Cell { norm: q * q + q * r + r * r, x, y });
                }
            }
        }
        found.sort_by(|a, b| a.norm.cmp(&b.norm).then(a.y.total_cmp(&b.y)).then(a.x.total_cmp(&b.x)));
        // Außerhalb des Kastens |q|,|r| ≤ k ist die Norm ≥ 3k²/4: alles darunter ist vollständig.
        if found.len() >= n && found[n - 1].norm * 4 < 3 * k * k {
            let last = found[n - 1].norm;
            found.retain(|c| c.norm <= last);
            return found;
        }
        k += 2;
    }
}

/// Radius der Traube: Knoten samt Beschriftung und die gewählten Gitterpunkte samt Kreis.
fn cluster_radius(r_anchor: f64, cells: &[Cell], n: usize) -> f64 {
    let label = LABEL_HALF_WIDTH.hypot(r_anchor + LABEL_HEIGHT);
    let ideas = cells.iter().take(n).map(|c| c.x.hypot(c.y) + IDEA_R + GAP / 2.0).fold(0.0, f64::max);
    r_anchor.max(label).max(ideas)
}

/// Kleinster Winkelabstand zweier Richtungen.
fn angle_between(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(2.0 * PI);
    d.min(2.0 * PI - d)
}

/// Die Karte anordnen (deterministisch: gleiche Eingabe, gleiche Koordinaten).
pub fn layout(map: &IdeaMap) -> Layout {
    // Knoten und Tiefen (fehlt ein Elternknoten, hängt der Knoten am Werk).
    let node_index: HashMap<&str, usize> = map.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let mut depth = vec![usize::MAX; map.nodes.len()];
    for i in 0..map.nodes.len() {
        let mut chain = Vec::new();
        let mut at = i;
        let base = loop {
            if depth[at] != usize::MAX {
                break depth[at];
            }
            if map.nodes[at].id == "w" {
                depth[at] = 0;
                break 0;
            }
            chain.push(at);
            match map.nodes[at].parent.as_deref().and_then(|p| node_index.get(p)) {
                Some(&parent) if !chain.contains(&parent) && chain.len() < 16 => at = parent,
                // Ohne bekannten Elternknoten (oder bei einem Kreis): am Werk.
                _ => break 0,
            }
        };
        for (step, &node) in chain.iter().rev().enumerate() {
            depth[node] = base + step + 1;
        }
    }

    // Gespeicherte Lagen (nur endliche Werte).
    let pinned: HashMap<IdeaId, (f64, f64)> =
        map.positions.iter().filter(|p| p.x.is_finite() && p.y.is_finite()).map(|p| (p.idea, (p.x, p.y))).collect();

    // Einfälle je Knoten (unbekannter Anker → Werk); angeheftete belegen keinen Gitterplatz.
    let work_slot = node_index.get("w").copied();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); map.nodes.len()];
    let mut stray = Vec::new();
    for (i, idea) in map.ideas.iter().enumerate() {
        if pinned.contains_key(&idea.id) {
            continue;
        }
        match node_index.get(idea.anchor.as_str()).copied().or(work_slot) {
            Some(slot) => members[slot].push(i),
            None => stray.push(i),
        }
    }

    // Trauben.
    let radius: Vec<f64> = map.nodes.iter().map(|n| node_radius(n.kind)).collect();
    let grid: Vec<Vec<Cell>> = members.iter().zip(&radius).map(|(m, &r)| cells(m.len(), r)).collect();
    let rc: Vec<f64> = (0..map.nodes.len()).map(|i| cluster_radius(radius[i], &grid[i], members[i].len())).collect();

    // Winkel: jeder oberste Knoten bekommt einen Sektor nach dem Platzbedarf seines Teilbaums, Kinder teilen den
    // Sektor ihres Elternknotens (klassischer radialer Baum) – so kreuzen Baumkanten keine fremden Sektoren.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); map.nodes.len()];
    let mut roots = Vec::new();
    for i in 0..map.nodes.len() {
        if depth[i] == 0 || depth[i] == usize::MAX {
            continue;
        }
        let parent = map.nodes[i].parent.as_deref().and_then(|p| node_index.get(p)).copied();
        match parent {
            Some(p) if depth[p] != usize::MAX && depth[p] + 1 == depth[i] && depth[p] > 0 => children[p].push(i),
            _ => roots.push(i),
        }
    }
    let mut weight: Vec<f64> = rc.iter().map(|r| 2.0 * r + GAP).collect();
    let mut by_depth: Vec<usize> = (0..map.nodes.len()).collect();
    by_depth.sort_by_key(|&i| std::cmp::Reverse(if depth[i] == usize::MAX { 0 } else { depth[i] }));
    for &i in &by_depth {
        let sum: f64 = children[i].iter().map(|&c| weight[c]).sum();
        weight[i] = weight[i].max(sum);
    }
    let mut angle = vec![-PI / 2.0; map.nodes.len()];
    let mut stack: Vec<(Vec<usize>, f64, f64)> = vec![(roots, -PI / 2.0, 3.0 * PI / 2.0)];
    while let Some((list, from, to)) = stack.pop() {
        let total: f64 = list.iter().map(|&i| weight[i]).sum();
        let mut cursor = from;
        for &i in &list {
            let span = if total > 0.0 { (to - from) * weight[i] / total } else { 0.0 };
            angle[i] = cursor + span / 2.0;
            if !children[i].is_empty() {
                stack.push((children[i].clone(), cursor, cursor + span));
            }
            cursor += span;
        }
    }

    // Ringe.
    let mut pos = vec![(0.0f64, 0.0f64); map.nodes.len()];
    let max_depth = depth.iter().copied().filter(|&d| d != usize::MAX).max().unwrap_or(0);
    let mut outer = work_slot.map_or(node_radius(MapNodeKind::Work), |w| rc[w]);
    for d in 1..=max_depth {
        let ring: Vec<usize> = (0..map.nodes.len()).filter(|&i| depth[i] == d).collect();
        if ring.is_empty() {
            continue;
        }
        let widest = ring.iter().map(|&i| rc[i]).fold(0.0, f64::max);
        let total: f64 = ring.iter().map(|&i| 2.0 * rc[i] + GAP).sum();
        let angles: Vec<f64> = ring.iter().map(|&i| angle[i]).collect();
        let mut r = (outer + GAP + widest).max(total / (2.0 * PI));
        loop {
            let at = |k: usize, r: f64| (r * angles[k].cos(), r * angles[k].sin());
            let mut ok = true;
            'pairs: for a in 0..ring.len() {
                for b in a + 1..ring.len() {
                    let (pa, pb) = (at(a, r), at(b, r));
                    if (pa.0 - pb.0).hypot(pa.1 - pb.1) < rc[ring[a]] + rc[ring[b]] + GAP {
                        ok = false;
                        break 'pairs;
                    }
                }
            }
            if ok {
                break;
            }
            r *= 1.05;
        }
        for (k, &i) in ring.iter().enumerate() {
            pos[i] = (r * angles[k].cos(), r * angles[k].sin());
        }
        outer = r + widest;
    }

    // Platzieren.
    let mut items = Vec::with_capacity(map.nodes.len() + map.ideas.len());
    for (i, node) in map.nodes.iter().enumerate() {
        let d = if depth[i] == usize::MAX { 0 } else { depth[i] };
        items.push(Placed { id: node.id.clone(), x: pos[i].0, y: pos[i].1, r: radius[i], pinned: false, depth: d });
    }
    let mut idea_place: Vec<Option<Placed>> = vec![None; map.ideas.len()];
    for (slot, list) in members.iter().enumerate() {
        if list.is_empty() {
            continue;
        }
        // Gitterpunkte bis zur Norm des n-ten; im letzten Ring nach außen gerichtete zuerst.
        let outward = if map.nodes[slot].id == "w" { -PI / 2.0 } else { angle[slot] };
        let mut chosen = grid[slot].clone();
        chosen.sort_by(|a, b| {
            a.norm
                .cmp(&b.norm)
                .then(angle_between(a.y.atan2(a.x), outward).total_cmp(&angle_between(b.y.atan2(b.x), outward)))
                .then(a.y.total_cmp(&b.y))
                .then(a.x.total_cmp(&b.x))
        });
        let d = items[slot].depth;
        for (&idea, cell) in list.iter().zip(&chosen) {
            idea_place[idea] = Some(Placed {
                id: map.ideas[idea].node.clone(),
                x: pos[slot].0 + cell.x,
                y: pos[slot].1 + cell.y,
                r: IDEA_R,
                pinned: false,
                depth: d,
            });
        }
    }
    // Ohne jeden Knoten (keine Mitte): auf einem eigenen Gitter um (0,0).
    if !stray.is_empty() {
        let around = cells(stray.len(), node_radius(MapNodeKind::Work));
        for (&idea, cell) in stray.iter().zip(&around) {
            idea_place[idea] = Some(Placed {
                id: map.ideas[idea].node.clone(),
                x: cell.x,
                y: cell.y,
                r: IDEA_R,
                pinned: false,
                depth: 1,
            });
        }
    }
    for (i, idea) in map.ideas.iter().enumerate() {
        let placed = match pinned.get(&idea.id) {
            Some(&(x, y)) => {
                let d = node_index.get(idea.anchor.as_str()).map_or(1, |&s| items[s].depth);
                Placed { id: idea.node.clone(), x, y, r: IDEA_R, pinned: true, depth: d }
            }
            None => idea_place[i].take().unwrap_or(Placed {
                id: idea.node.clone(),
                x: 0.0,
                y: 0.0,
                r: IDEA_R,
                pinned: false,
                depth: 1,
            }),
        };
        items.push(placed);
    }

    // viewBox mit Rand für die Beschriftungen unter den Kreisen.
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &items {
        x0 = x0.min(p.x - p.r - LABEL_HALF_WIDTH);
        x1 = x1.max(p.x + p.r + LABEL_HALF_WIDTH);
        y0 = y0.min(p.y - p.r - 16.0);
        y1 = y1.max(p.y + p.r + LABEL_HEIGHT + 8.0);
    }
    if items.is_empty() {
        (x0, y0, x1, y1) = (-100.0, -100.0, 100.0, 100.0);
    }
    let (x0, y0) = (x0.floor(), y0.floor());
    let view = (x0, y0, (x1.ceil() - x0).max(1.0), (y1.ceil() - y0).max(1.0));
    let index = items.iter().enumerate().map(|(i, p)| (p.id.clone(), i)).collect();
    Layout { items, index, view }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::idea_map::{MapFilter, MapIdea, MapNode, Near, NearState};
    use baruch_store::{IdeaOrigin, IdeaStatus, MapPos};

    fn node(id: &str, kind: MapNodeKind, parent: Option<&str>) -> MapNode {
        MapNode {
            id: id.into(),
            kind,
            key: id.strip_prefix("n:").and_then(|k| k.parse().ok()),
            parent: parent.map(Into::into),
            number: String::new(),
            title: format!("Knoten {id}"),
            sections: Vec::new(),
            open_ideas: 0,
            total_ideas: 0,
            open_findings: 0,
        }
    }

    fn idea(id: IdeaId, anchor: &str) -> MapIdea {
        MapIdea {
            id,
            node: format!("i:{id}"),
            anchor: anchor.into(),
            key: None,
            label: format!("Wort{id}"),
            words: vec![format!("Wort{id}")],
            note: String::new(),
            status: IdeaStatus::Open,
            origin: IdeaOrigin::Human { name: "Autor".into() },
            hit: true,
        }
    }

    /// Erfundene Karte: zwei Teile mit je drei Kapiteln, zwei Kapitel ohne Teil, Wolke „Ohne Kapitel“;
    /// `n` Einfälle reihum verteilt (jeder siebte ohne Kapitel).
    pub(crate) fn sample(n: usize) -> IdeaMap {
        let mut nodes = vec![node("w", MapNodeKind::Work, None)];
        let mut key = 1;
        for _part in 0..2 {
            let part = format!("n:{key}");
            nodes.push(node(&part, MapNodeKind::Part, Some("w")));
            key += 1;
            for _ in 0..3 {
                nodes.push(node(&format!("n:{key}"), MapNodeKind::Chapter, Some(&part)));
                key += 1;
            }
        }
        for _ in 0..2 {
            nodes.push(node(&format!("n:{key}"), MapNodeKind::Chapter, Some("w")));
            key += 1;
        }
        nodes.push(node("none", MapNodeKind::NoChapter, Some("w")));
        let anchors: Vec<String> = nodes.iter().skip(1).map(|n| n.id.clone()).collect();
        let ideas = (0..n)
            .map(|i| {
                let anchor = if i % 7 == 6 { "none".to_owned() } else { anchors[i % anchors.len()].clone() };
                idea(i as IdeaId + 1, &anchor)
            })
            .collect();
        IdeaMap {
            work: 1,
            title: "Erfundenes Werk".into(),
            nodes,
            ideas,
            links: Vec::new(),
            near: Near { state: NearState::Off, message: String::new(), pairs: Vec::new() },
            positions: Vec::new(),
            filter: MapFilter::default(),
        }
    }

    /// Alle Paare nicht angehefteter Kreise mit zu kleinem Abstand.
    pub(crate) fn overlaps(layout: &Layout) -> Vec<(String, String, f64)> {
        let free: Vec<&Placed> = layout.items.iter().filter(|p| !p.pinned).collect();
        let mut bad = Vec::new();
        for (i, a) in free.iter().enumerate() {
            for b in &free[i + 1..] {
                let d = (a.x - b.x).hypot(a.y - b.y);
                if d < a.r + b.r {
                    bad.push((a.id.clone(), b.id.clone(), d));
                }
            }
        }
        bad
    }

    #[test]
    fn the_same_map_is_laid_out_the_same_way_twice() {
        let map = sample(60);
        assert_eq!(layout(&map), layout(&map), "keine Zufälle, keine Reihenfolge aus HashMaps: gleiche Koordinaten");
    }

    #[test]
    fn the_work_sits_in_the_middle_and_chapters_of_a_part_lie_further_out() {
        let map = sample(30);
        let l = layout(&map);
        let w = l.get("w").expect("Werk platziert");
        assert_eq!((w.x, w.y), (0.0, 0.0), "Vertrag §2: Werk in der Mitte bei (0,0)");
        let part = l.get("n:1").unwrap();
        let chapter = l.get("n:2").unwrap();
        assert!(
            chapter.x.hypot(chapter.y) > part.x.hypot(part.y),
            "Kapitel unter einem Teil liegen auf einem äußeren Ring: {part:?} {chapter:?}"
        );
        assert_eq!((part.depth, chapter.depth), (1, 2), "Tiefe folgt der Gliederung der Karte");
    }

    #[test]
    fn three_hundred_ideas_do_not_overlap() {
        let map = sample(300);
        let l = layout(&map);
        assert_eq!(l.items.len(), map.nodes.len() + 300, "jeder Knoten und jeder Einfall ist platziert");
        let bad = overlaps(&l);
        assert!(
            bad.is_empty(),
            "Abnahme: kein Paar nicht angehefteter Kreise näher als r1 + r2: {:?}",
            &bad[..bad.len().min(5)]
        );
    }

    #[test]
    fn saved_positions_win_and_are_marked_pinned() {
        let mut map = sample(10);
        map.positions.push(MapPos { idea: 3, x: 1234.5, y: -77.0, updated: String::new() });
        let l = layout(&map);
        let p = l.get("i:3").unwrap();
        assert_eq!((p.x, p.y, p.pinned), (1234.5, -77.0, true), "gespeicherte Lage überstimmt die Anordnung");
        assert!(l.view.0 + l.view.2 >= 1234.5 + IDEA_R, "die viewBox schließt die Lage ein: {:?}", l.view);
        assert!(!l.get("i:4").unwrap().pinned, "andere Einfälle bleiben automatisch angeordnet");
    }

    #[test]
    fn an_idea_with_an_unknown_anchor_still_gets_a_place() {
        let mut map = sample(3);
        map.ideas[0].anchor = "n:999".into();
        let l = layout(&map);
        assert!(l.get("i:1").is_some(), "kein Einfall geht verloren (hängt dann am Werk)");
        assert!(overlaps(&l).is_empty(), "auch dann ohne Überlappung");
    }
}
