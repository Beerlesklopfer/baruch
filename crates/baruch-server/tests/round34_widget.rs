// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 34 „Mindmap“ – image, page and widget (owner mindmap_widget). Checked are the drawing
//! `idea_map::bild::mindmap::render` on the hand-built tree (DOM contract §6, layout, folding, relations, cloud), the
//! page `/werke/{id}/karte` with `/karte/bild` and the delivered script `mindmap.js` (keys and RPC from §7). Works and
//! ideas are freely invented; no model, no network.

use std::collections::{BTreeSet, HashMap};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::idea_map::bild::mindmap::{self, BRANCH_COLORS, RenderOptions, Rendered};
use baruch_server::idea_map::tree::{self, CloudHint, MindMap, TreeKind, TreeNode};
use baruch_server::idea_map::{MapFilter, Near, NearPair, NearState, NearVia};
use baruch_store::{IdeaLink, IdeaOrigin, IdeaStatus, LinkKind};
use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::json;

use crate::common;

// ---- Built by hand (all fields are pub, contract §9) ---------------------------------------------------------

fn node(id: &str, kind: TreeKind, title: &str, children: Vec<TreeNode>) -> TreeNode {
    let idea = id.strip_prefix("i:").and_then(|n| n.parse().ok());
    let key = id.strip_prefix("n:").and_then(|n| n.parse().ok());
    TreeNode {
        id: id.to_owned(),
        kind,
        idea,
        key,
        number: String::new(),
        title: title.to_owned(),
        own_title: String::new(),
        note: String::new(),
        has_note: false,
        words: Vec::new(),
        status: idea.map(|_| IdeaStatus::Open),
        origin: None,
        section_title: None,
        editable: kind == TreeKind::Idea,
        hit: true,
        ghost: false,
        open_findings: 0,
        descendants: 0,
        children,
    }
}

fn idea(id: i64, title: &str, children: Vec<TreeNode>) -> TreeNode {
    node(&format!("i:{id}"), TreeKind::Idea, title, children)
}

fn chapter(key: i64, title: &str, children: Vec<TreeNode>) -> TreeNode {
    node(&format!("n:{key}"), TreeKind::Chapter, title, children)
}

fn map(children: Vec<TreeNode>, links: Vec<IdeaLink>) -> MindMap {
    MindMap {
        work: 7,
        title: "Leuchtturm & Nebel".to_owned(),
        root: node("w", TreeKind::Work, "Leuchtturm & Nebel", children),
        links,
        near: Near { state: NearState::Off, message: String::new(), pairs: Vec::new() },
        filter: MapFilter::default(),
        total_ideas: 0,
    }
}

fn link(id: i64, from: i64, to: i64, kind: LinkKind, note: &str) -> IdeaLink {
    IdeaLink {
        id,
        uuid: format!("00000000-0000-4000-8000-{id:012}"),
        from,
        to,
        kind,
        note: note.to_owned(),
        origin: IdeaOrigin::Human { name: "Mensch".to_owned() },
        created: "2026-11-25T10:00:00Z".to_owned(),
        updated: "2026-11-25T10:00:00Z".to_owned(),
        deleted: None,
    }
}

/// Four chapters, below them ideas in two further levels (work → chapter → idea → sub-idea).
fn three_levels() -> MindMap {
    map(
        vec![
            chapter(1, "Hafen", vec![idea(11, "Mole", vec![idea(111, "Poller", vec![])]), idea(12, "Kran", vec![])]),
            chapter(2, "Feuer", vec![idea(21, "Linse", vec![idea(211, "Schliff", vec![])])]),
            chapter(3, "Wärter", vec![idea(31, "Logbuch", vec![])]),
            chapter(4, "Sturm", vec![idea(41, "Brecher", vec![idea(411, "Gischt", vec![])])]),
        ],
        Vec::new(),
    )
}

// ---- Reading SVG ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct El {
    name: String,
    attrs: HashMap<String, String>,
    /// Raw (escaped) text directly in this element.
    text: String,
    parent: Option<usize>,
}

impl El {
    fn attr(&self, name: &str) -> &str {
        self.attrs.get(name).map_or("", String::as_str)
    }
    fn has_class(&self, class: &str) -> bool {
        self.attr("class").split_whitespace().any(|c| c == class)
    }
    fn number(&self, name: &str) -> f64 {
        self.attr(name).parse().unwrap_or_else(|_| panic!("{name} ist eine Zahl: {:?}", self.attrs))
    }
}

/// Reads the SVG with an XML parser (closing tags have to match, attributes have to be validly escaped) – if it fails,
/// the SVG is not well-formed.
#[allow(deprecated)] // unescape_value is deprecated in quick-xml 0.42; it is enough here (no DTD, no entities).
fn parse(svg: &str) -> Vec<El> {
    let mut reader = Reader::from_str(svg);
    let mut out: Vec<El> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    loop {
        let event = reader.read_event().unwrap_or_else(|e| panic!("das SVG ist wohlgeformtes XML: {e}\n{svg}"));
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let mut attrs = HashMap::new();
                for attr in e.attributes() {
                    let attr = attr.unwrap_or_else(|e| panic!("gültiges Attribut: {e}\n{svg}"));
                    let value = attr.unescape_value().unwrap_or_else(|e| panic!("gültig maskierter Wert: {e}"));
                    let fresh = attrs.insert(attr.key.0.to_owned(), value.into_owned());
                    assert!(fresh.is_none(), "kein Attribut doppelt (XML verbietet es)");
                }
                out.push(El { name: e.name().0.to_owned(), attrs, text: String::new(), parent: stack.last().copied() });
                if matches!(event, Event::Start(_)) {
                    stack.push(out.len() - 1);
                }
            }
            Event::End(_) => {
                stack.pop().expect("jedes Schlusstag hat ein Starttag");
            }
            Event::Text(ref t) => {
                if let Some(&at) = stack.last() {
                    out[at].text.push_str(t);
                }
            }
            // quick-xml delivers character references as an event of their own; in the raw text they stay as they are
            // written.
            Event::GeneralRef(ref r) => {
                if let Some(&at) = stack.last() {
                    out[at].text.push('&');
                    out[at].text.push_str(r);
                    out[at].text.push(';');
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    assert!(stack.is_empty(), "alle Elemente sind geschlossen");
    out
}

fn nodes(els: &[El]) -> Vec<(usize, &El)> {
    els.iter().enumerate().filter(|(_, e)| e.name == "g" && e.has_class("mm-node")).collect()
}

fn node_of<'a>(els: &'a [El], id: &str) -> (usize, &'a El) {
    nodes(els).into_iter().find(|(_, e)| e.attr("data-node") == id).unwrap_or_else(|| panic!("Knoten {id} fehlt"))
}

fn children_of(els: &[El], at: usize) -> Vec<&El> {
    els.iter().filter(|e| e.parent == Some(at)).collect()
}

/// Position of the first child of `at` with the class `class`.
fn child_at(els: &[El], at: usize, class: &str) -> Option<usize> {
    els.iter().position(|e| e.parent == Some(at) && e.has_class(class))
}

/// Raw text of the first child `name` of `at`.
fn text_in(els: &[El], at: usize, name: &str) -> String {
    els.iter().find(|e| e.parent == Some(at) && e.name == name).map(|e| e.text.clone()).unwrap_or_default()
}

/// Box of a node: (x, y, width, height).
fn rect_of(els: &[El], at: usize) -> (f64, f64, f64, f64) {
    let rect = children_of(els, at).into_iter().find(|e| e.has_class("mm-box")).expect("jeder Knoten hat mm-box");
    (rect.number("x"), rect.number("y"), rect.number("width"), rect.number("height"))
}

/// Draws a hand-built tree. In these trees the rule is: ideas directly at the root lie in the cloud (that is what the
/// view `tree::Scene` calls it for the page); if a test names `cloud` itself, its value applies. Free main topics
/// (ideas directly at the work as a main branch) are checked by `free_main_topics_are_main_branches_not_cloud_cards`.
fn render(map: &MindMap, options: &RenderOptions) -> Rendered {
    let mut options = options.clone();
    if options.cloud.is_empty() {
        options.cloud = map.root.children.iter().filter(|c| c.kind == TreeKind::Idea).map(|c| c.id.clone()).collect();
    }
    // The German wording of the hints is what these tests check (the interface language of a request).
    baruch_server::i18n::sync_scope(baruch_server::i18n::Lang::De, || mindmap::render(map, &options))
}

fn fragment(map: &MindMap) -> String {
    render(map, &RenderOptions::default()).svg
}

// ---- Drawing -------------------------------------------------------------------------------------------------

#[test]
fn three_levels_follow_the_dom_contract_with_branches_on_both_sides() {
    let map = three_levels();
    let rendered = render(&map, &RenderOptions::default());
    let els = parse(&rendered.svg);
    let svg = &els[0];
    assert!(svg.name == "svg" && svg.has_class("mm-svg"), "Wurzel ist svg.mm-svg (Vertrag §6)");
    assert_eq!(svg.attr("viewBox"), format!("0 0 {} {}", svg.attr("width"), svg.attr("height")), "viewBox = Größe");
    assert!(
        (svg.number("width") - rendered.width).abs() < 0.1 && (svg.number("height") - rendered.height).abs() < 0.1,
        "Rendered nennt dieselbe Größe wie das Bild (der Aufrufer legt danach die PDF-Seite an)"
    );
    assert_eq!(els.iter().filter(|e| e.has_class("mm-viewport")).count(), 1, "genau ein g.mm-viewport für Zoom");

    let all = nodes(&els);
    assert_eq!(all.len(), map.walk().len(), "jeder Knoten des Baums ist einmal gezeichnet (nichts eingeklappt)");
    let (root_at, root) = node_of(&els, "w");
    assert!(root.has_class("mm-work") && root.attr("data-depth") == "0", "das Werk ist der Knoten der Tiefe 0");
    assert_eq!(root.attr("data-editable"), "0", "das Werk ist in der Mindmap nicht umbenennbar");

    // Main branches: first half on the right, second on the left; the work stands in between.
    let (rx, _, rw, _) = rect_of(&els, root_at);
    for (id, side) in [("n:1", "right"), ("n:2", "right"), ("n:3", "left"), ("n:4", "left")] {
        let (at, main) = node_of(&els, id);
        assert_eq!(main.attr("data-side"), side, "{id} steht {side} (erste Hälfte rechts, zweite links)");
        assert_eq!(main.attr("data-parent"), "w", "Hauptäste hängen am Werk");
        assert!(main.has_class("mm-chapter") && main.attr("data-editable") == "0", "Kapitel sind nicht änderbar");
        assert_eq!(main.attr("tabindex"), "0", "Knoten sind mit der Tastatur erreichbar");
        let (x, _, w, _) = rect_of(&els, at);
        if side == "right" {
            assert!(x > rx + rw, "rechter Hauptast liegt rechts vom Werk");
        } else {
            assert!(x + w < rx, "linker Hauptast liegt links vom Werk (Werk in der Mitte)");
        }
    }

    // Colour per main branch on all descendants, side inherited, depth counted.
    for (main, branch, below) in [("n:1", "0", vec!["i:11", "i:111", "i:12"]), ("n:4", "3", vec!["i:41", "i:411"])] {
        let side = node_of(&els, main).1.attr("data-side").to_owned();
        for id in std::iter::once(main).chain(below) {
            let (_, n) = node_of(&els, id);
            assert_eq!(n.attr("data-branch"), branch, "{id} trägt die Astnummer seines Hauptasts {main}");
            assert!(n.has_class(&format!("mm-b{branch}")), "{id} trägt die Farbklasse mm-b{branch}");
            assert_eq!(n.attr("data-side"), side, "{id} steht auf der Seite seines Hauptasts");
        }
    }
    let (_, deep) = node_of(&els, "i:111");
    assert_eq!((deep.attr("data-depth"), deep.attr("data-parent")), ("3", "i:11"), "dritte Ebene unter i:11");
    assert!(deep.has_class("mm-idea") && deep.attr("data-idea") == "111", "Einfälle tragen data-idea");
    assert_eq!(deep.attr("data-editable"), "1", "Einfälle sind änderbar");

    // Branches: per node except the work one cubic curve from the parent to the child node, in the branch colour.
    let edges: Vec<&El> = els.iter().filter(|e| e.name == "path" && e.has_class("mm-edge")).collect();
    assert_eq!(edges.len(), all.len() - 1, "ein Ast je Knoten außer dem Werk");
    for edge in &edges {
        assert!(edge.attr("d").starts_with('M') && edge.attr("d").contains('C'), "Äste sind Kurven: {:?}", edge.attrs);
        let (_, to) = node_of(&els, edge.attr("data-to"));
        assert_eq!(edge.attr("data-from"), to.attr("data-parent"), "data-from ist der Elternknoten von data-to");
        assert!(edge.has_class(&format!("mm-b{}", to.attr("data-branch"))), "der Ast trägt die Farbe seines Hauptasts");
    }
    let width = |to: &str| edges.iter().find(|e| e.attr("data-to") == to).unwrap().number("stroke-width");
    assert!(width("n:1") > width("i:11") && width("i:11") > width("i:111"), "die Stärke nimmt mit der Tiefe ab");
}

#[test]
fn user_text_is_escaped_in_content_and_attributes() {
    let nasty = "Tau <b>\"x\"</b> & 'Öse'";
    let mut item = idea(5, nasty, vec![]);
    item.note = "Notiz </g><script>alert(1)</script>".to_owned();
    item.has_note = true;
    item.words = vec!["a\"b".to_owned(), "<c>".to_owned()];
    item.key = Some(9);
    item.section_title = Some("Kap <1> & \"zwei\"".to_owned());
    let map = map(vec![chapter(1, "Knoten & <Seile>", vec![item])], vec![link(1, 5, 5, LinkKind::LeadsTo, "<x>")]);
    for standalone in [false, true] {
        let svg =
            render(&map, &RenderOptions { standalone, back: "q=a%22b&word=<x>".to_owned(), ..Default::default() }).svg;
        assert!(!svg.contains("<b>") && !svg.contains("<script>"), "Nutzertext wird nie zu einem Element");
        assert!(
            !svg.contains("<Seile>") && !svg.contains("<c>") && !svg.contains("<x>"),
            "auch nicht aus Titel/Wort/back"
        );
        // The parser rejects raw & < and open quotation marks; it returns the values unescaped.
        let els = parse(&svg);
        let (_, n) = node_of(&els, "i:5");
        assert_eq!(n.attr("data-title"), nasty, "data-title trägt den Titel verlustfrei (für das Umbenennen)");
        assert!(
            svg.contains("Tau &lt;b&gt;&quot;x&quot;&lt;/b&gt; &amp; &#39;Öse&#39;"),
            "der Titel steht maskiert im Bild"
        );
        assert!(svg.contains("Leuchtturm &amp; Nebel") && !svg.contains("Leuchtturm & Nebel"), "Werktitel maskiert");
    }
}

/// 40 nodes in four levels (work, 4 chapters, 12 ideas, 23 sub-ideas) with titles of very different lengths.
fn forty() -> MindMap {
    let mut chapters = Vec::new();
    let mut next = 100;
    for c in 0..4 {
        let mut ideas = Vec::new();
        for i in 0..3 {
            let subs = if c == 3 && i == 2 { 1 } else { 2 };
            let mut below = Vec::new();
            for s in 0..subs {
                next += 1;
                below.push(idea(next, &"Untergedanke mit Schweif ".repeat(1 + (s + i) % 3), vec![]));
            }
            next += 1;
            ideas.push(idea(next, &format!("Einfall {c}.{i} {}", "lang ".repeat(i * 4)), below));
        }
        chapters.push(chapter(c + 1, &format!("Kapitel {c} {}", "Meer ".repeat(c as usize)), ideas));
    }
    map(chapters, Vec::new())
}

#[test]
fn forty_nodes_in_four_levels_do_not_overlap_and_the_layout_is_deterministic() {
    let map = forty();
    assert_eq!(map.walk().len(), 40, "der Prüfbaum hat 40 Knoten");
    assert_eq!(map.walk().iter().map(|(_, d)| *d).max(), Some(3), "in vier Ebenen (Tiefe 0 bis 3)");
    for closed in [vec![], vec!["n:2".to_owned(), "i:103".to_owned()]] {
        let options = RenderOptions { closed, ..Default::default() };
        let first = render(&map, &options);
        let els = parse(&first.svg);
        let boxes: Vec<(String, (f64, f64, f64, f64))> =
            nodes(&els).into_iter().map(|(at, e)| (e.attr("data-node").to_owned(), rect_of(&els, at))).collect();
        for (i, (a, ra)) in boxes.iter().enumerate() {
            assert!(ra.0 >= 0.0 && ra.1 >= 0.0, "{a} liegt im Bild (keine negativen Koordinaten)");
            assert!(ra.0 + ra.2 <= first.width && ra.1 + ra.3 <= first.height, "{a} liegt innerhalb von Breite/Höhe");
            for (b, rb) in &boxes[i + 1..] {
                let apart = ra.0 + ra.2 <= rb.0 || rb.0 + rb.2 <= ra.0 || ra.1 + ra.3 <= rb.1 || rb.1 + rb.3 <= ra.1;
                assert!(apart, "die Kästen von {a} {ra:?} und {b} {rb:?} überlappen sich nicht");
            }
        }
        // No randomness, no clock, no hash order: the same SVG twice.
        assert_eq!(first.svg, render(&map, &options).svg, "gleiche Eingabe, gleiches SVG");
        assert_eq!(first.svg, render(&map.clone(), &options).svg, "auch aus einer Kopie des Baums");
    }
}

#[test]
fn closed_nodes_draw_no_descendants_and_the_toggle_is_a_link_that_switches_zu() {
    let map = three_levels();
    let options = RenderOptions {
        closed: vec!["n:1".to_owned(), "i:999".to_owned()],
        back: "status=open".to_owned(),
        ..Default::default()
    };
    let svg = render(&map, &options).svg;
    let els = parse(&svg);
    let ids: BTreeSet<&str> = nodes(&els).iter().map(|(_, e)| e.attr("data-node")).collect();
    for hidden in ["i:11", "i:111", "i:12"] {
        assert!(!ids.contains(hidden), "{hidden} liegt unter dem eingeklappten n:1 und wird nicht gezeichnet");
    }
    assert!(ids.contains("n:1") && ids.contains("i:21"), "der eingeklappte Knoten selbst und andere Äste bleiben");
    assert!(!els.iter().any(|e| e.has_class("mm-edge") && e.attr("data-from") == "n:1"), "keine Äste ins Verborgene");
    assert_eq!(els[0].attr("data-zu"), "n:1", "data-zu nennt nur vorhandene eingeklappte Knoten (i:999 gibt es nicht)");

    let toggle = |id: &str| {
        let at =
            child_at(&els, node_of(&els, id).0, "mm-toggle").unwrap_or_else(|| panic!("{id} hat einen Einklapper"));
        (els[at].clone(), text_in(&els, at, "text"))
    };
    let (closed, text) = toggle("n:1");
    assert_eq!(closed.name, "a", "ohne JavaScript ist der Einklapper ein Link");
    assert_eq!(text, "+ 3", "eingeklappt zeigt er die Zahl der verborgenen Nachfahren");
    assert_eq!(closed.attr("data-count"), "3", "dieselbe Zahl in data-count");
    assert_eq!(closed.attr("href"), "?status=open", "sein Link klappt auf: zu= ohne n:1, der Filter bleibt");
    assert!(node_of(&els, "n:1").1.has_class("mm-closed"), "der Knoten trägt mm-closed");
    let (open, text) = toggle("n:2");
    assert_eq!(text, "−", "ein offener Knoten zeigt −");
    assert_eq!(open.attr("href"), "?status=open&zu=n:1,n:2", "sein Link klappt zusätzlich n:2 zu (Filter bleibt)");
    let (at, _) = node_of(&els, "i:31");
    assert!(!children_of(&els, at).iter().any(|e| e.has_class("mm-toggle")), "Blätter haben keinen Einklapper");
    let (at, _) = node_of(&els, "w");
    assert!(!children_of(&els, at).iter().any(|e| e.has_class("mm-toggle")), "das Werk klappt nicht");
}

#[test]
fn note_symbol_section_link_and_findings_appear_only_where_they_belong() {
    let mut with_note = idea(11, "Mole", vec![]);
    with_note.has_note = true;
    with_note.note = "Flut beachten".to_owned();
    let mut with_section = idea(12, "Kran", vec![]);
    with_section.key = Some(77);
    with_section.section_title = Some("1.2 Kai".to_owned());
    let mut done = idea(13, "Erledigt", vec![]);
    done.status = Some(IdeaStatus::Done);
    let mut ghost = idea(14, "Geist", vec![]);
    ghost.ghost = true;
    let mut busy = chapter(1, "Hafen", vec![with_note, with_section, done, ghost]);
    busy.open_findings = 4;
    busy.number = "2".to_owned();
    let map = map(vec![busy, chapter(2, "Feuer", vec![])], Vec::new());
    let els = parse(&fragment(&map));
    let kids = |id: &str| children_of(&els, node_of(&els, id).0);

    let note = child_at(&els, node_of(&els, "i:11").0, "mm-note").expect("Notiz-Symbol bei has_note");
    assert_eq!(text_in(&els, note, "title"), "Flut beachten", "das Symbol nennt die Notiz als Tooltip");
    for id in ["i:12", "i:13", "n:1", "w"] {
        assert!(!kids(id).iter().any(|e| e.has_class("mm-note")), "{id} hat keine Notiz und kein Symbol");
    }

    let section = kids("i:12").into_iter().find(|e| e.has_class("mm-section")).expect("Einfall mit Bezug: Link");
    assert_eq!((section.name.as_str(), section.attr("href")), ("a", "/?work=7&key=77"), "Link zum Abschnitt im Werk");
    assert_eq!(node_of(&els, "i:12").1.attr("data-key"), "77", "der Bezug steht als data-key am Knoten");
    assert!(!kids("i:11").iter().any(|e| e.has_class("mm-section")), "ohne Bezug kein Abschnittslink");
    assert!(!kids("n:1").iter().any(|e| e.has_class("mm-section")), "Kapitel sind selbst der Abschnitt");

    assert!(node_of(&els, "i:13").1.has_class("mm-done"), "verarbeitete Einfälle tragen mm-done");
    assert!(node_of(&els, "i:14").1.has_class("mm-ghost"), "Geister (nur wegen ihrer Nachfahren gezeigt) mm-ghost");
    let findings = kids("n:1").into_iter().find(|e| e.has_class("mm-findings")).expect("Zahl der Befunde am Kapitel");
    assert_eq!(findings.attr("data-open-findings"), "4", "offene Befunde als Zahl");
    assert_eq!(findings.attr("href"), "/werke/7/befunde?key=1#zelle", "Link auf die Befunde des Kapitels");
    assert!(node_of(&els, "n:1").1.has_class("mm-many-findings"), "ab 3 offenen hervorgehoben (wie in Runde 28)");
    assert!(!kids("n:2").iter().any(|e| e.has_class("mm-findings")), "ohne offene Befunde keine Zahl");
    let label = kids("n:1").into_iter().find(|e| e.has_class("mm-label-link")).expect("Titel als Link");
    assert_eq!(label.attr("href"), "/?work=7&key=1", "der Titel eines Kapitels führt ohne JavaScript ins Werk");
    assert!(fragment(&map).contains(">2 Hafen<"), "Kapitel zeigen ihre Nummer vor dem Titel");
}

#[test]
fn relations_are_labelled_contradictions_stand_out_and_hidden_ends_stop_at_the_visible_ancestor() {
    let mut map = three_levels();
    map.links = vec![
        link(1, 12, 21, LinkKind::LeadsTo, "weil <Licht>"),
        link(2, 111, 31, LinkKind::Contradicts, ""),
        link(3, 11, 111, LinkKind::ExampleOf, "innen"),
        link(4, 41, 9999, LinkKind::BelongsTo, "Ziel ist nicht in der Karte"),
    ];
    let els = parse(&fragment(&map));
    let links: Vec<&El> = els.iter().filter(|e| e.name == "path" && e.has_class("mm-link")).collect();
    assert_eq!(links.len(), 3, "drei Beziehungen zwischen gezeigten Einfällen; die zum fehlenden Einfall entfällt");
    let by = |id: &str| *links.iter().find(|e| e.attr("data-link") == id).unwrap_or_else(|| panic!("Beziehung {id}"));
    // The labels are in a group of their own after the nodes (drawn on top, stay readable).
    let label_of = |id: &str| {
        els.iter()
            .find(|e| e.has_class("mm-link-label") && e.attr("data-link") == id)
            .unwrap_or_else(|| panic!("Beschriftung der Beziehung {id}"))
            .clone()
    };
    let last_node = els.iter().rposition(|e| e.has_class("mm-node")).unwrap();
    let first_label = els.iter().position(|e| e.has_class("mm-link-label")).unwrap();
    assert!(last_node < first_label, "Beschriftungen liegen über den Knoten (zuletzt gezeichnet)");
    let first = by("1");
    assert!(first.has_class("mm-link-leads_to"), "die Art steht als Klasse mm-link-<kind>");
    assert_eq!((first.attr("data-from"), first.attr("data-to")), ("i:12", "i:21"), "Enden als Kennungen der Einfälle");
    assert!(first.attr("marker-end").contains("mm-arrow"), "gerichtete Arten tragen eine Pfeilspitze");
    let label = label_of("1");
    assert!(label.name == "text" && label.has_class("mm-link-label"), "jede Beziehung hat eine Beschriftung");
    assert_eq!(label.text, "führt weiter: weil &lt;Licht&gt;", "Beschriftung = Art + Notiz, maskiert");

    let contra = by("2");
    assert!(
        contra.has_class("mm-contradiction") && contra.has_class("mm-link-contradicts"),
        "Widerspruch hervorgehoben"
    );
    assert_eq!(contra.attr("marker-end"), "", "widerspricht ist ungerichtet (keine Pfeilspitze)");
    assert!(label_of("2").has_class("mm-contradiction"), "auch seine Beschriftung");
    assert_eq!(label_of("2").text, "widerspricht", "ohne Notiz nur die Art");
    assert_eq!(contra.attr("data-end-from"), "i:111", "sichtbar: die Linie endet am Einfall selbst");

    // i:11 folded: i:111 is hidden – relation 2 ends at the visible ancestor i:11; relation 3 would lie entirely within
    // one node and is dropped.
    let svg = render(&map, &RenderOptions { closed: vec!["i:11".to_owned()], ..Default::default() }).svg;
    let els = parse(&svg);
    let links: Vec<&El> = els.iter().filter(|e| e.has_class("mm-link")).collect();
    let moved = links.iter().find(|e| e.attr("data-link") == "2").expect("Beziehung 2 bleibt sichtbar");
    assert_eq!(moved.attr("data-from"), "i:111", "data-from nennt weiter den wirklichen Einfall");
    assert_eq!(moved.attr("data-end-from"), "i:11", "gezeichnet wird bis zum sichtbaren Vorfahren");
    assert_eq!(moved.attr("data-end-to"), "i:31", "das sichtbare Ende bleibt");
    assert!(
        !links.iter().any(|e| e.attr("data-link") == "3"),
        "Beziehung innerhalb eines eingeklappten Knotens entfällt"
    );
    // Folded even further up: end at the chapter.
    let svg = render(&map, &RenderOptions { closed: vec!["n:1".to_owned()], ..Default::default() }).svg;
    let els = parse(&svg);
    let moved = els.iter().find(|e| e.has_class("mm-link") && e.attr("data-link") == "2").unwrap();
    assert_eq!(moved.attr("data-end-from"), "n:1", "der nächste sichtbare Vorfahre ist das Kapitel");
}

#[test]
fn free_main_topics_are_main_branches_not_cloud_cards() {
    // Integration Round 34: an idea directly at the work that the view does not name as cloud is a free main topic – a
    // main branch of its own with colour and branch to the work; only the named ones lie in the cloud.
    let mut map = three_levels();
    map.root.children.push(idea(95, "Freies Thema", vec![idea(96, "Unterpunkt", vec![])]));
    map.root.children.push(idea(97, "Ohne Platz", vec![]));
    let options = RenderOptions { cloud: vec!["i:97".to_owned()], cloud_count: Some(5), ..Default::default() };
    let svg = mindmap::render(&map, &options).svg;
    let els = parse(&svg);
    let (_, main) = node_of(&els, "i:95");
    assert!(main.has_class("mm-main") && !main.has_class("mm-cloud-item"), "das Hauptthema ist ein Hauptast");
    assert_ne!(main.attr("data-side"), "", "es steht rechts oder links vom Werk");
    assert_ne!(main.attr("data-branch"), "", "mit eigener Astfarbe");
    assert!(
        els.iter().any(|e| e.has_class("mm-edge") && e.attr("data-from") == "w" && e.attr("data-to") == "i:95"),
        "ein Ast verbindet es mit dem Werk"
    );
    assert_eq!(node_of(&els, "i:96").1.attr("data-parent"), "i:95", "sein Unterknoten wird im Baum gezeichnet");
    assert!(node_of(&els, "i:97").1.has_class("mm-cloud-item"), "nur der genannte Einfall liegt in der Wolke");
    assert_eq!(els[0].attr("data-cloud-count"), "5", "der Zähler ohne Filter kommt aus den Daten, nicht aus dem Bild");
    // A direct render has no request: the cloud head is English (Round 39c, ideas_msg.mindmap.cloud_count).
    assert!(svg.contains("5 in the cloud"), "und steht so im Bild");
}

#[test]
fn ideas_without_a_place_lie_in_the_cloud_with_counter_and_near_hint() {
    let mut map = three_levels();
    map.root.children.push(idea(90, "Lose Idee", vec![idea(91, "Anhang", vec![])]));
    map.root.children.push(idea(92, "Noch eine", vec![]));
    map.filter.near = true;
    map.near = Near {
        state: NearState::Ok,
        message: String::new(),
        pairs: vec![
            NearPair { a: 21, b: 92, score: 0.8, via: NearVia::Words, shared_words: vec!["licht".to_owned()] },
            NearPair { a: 41, b: 92, score: 0.9, via: NearVia::Vector, shared_words: Vec::new() },
            NearPair { a: 12, b: 21, score: 0.85, via: NearVia::Both, shared_words: Vec::new() },
        ],
    };
    // The suggestion comes from the cloud of the data (`Cloud::hints`): best pair with an idea in the tree.
    let hinted = RenderOptions {
        hints: vec![CloudHint {
            idea: 92,
            node: "i:41".to_owned(),
            node_title: "Einfall 41".to_owned(),
            branch: "n:4".to_owned(),
            branch_title: "Sturm".to_owned(),
            score: 0.9,
            shared_words: Vec::new(),
        }],
        ..Default::default()
    };
    let svg = render(&map, &hinted).svg;
    let els = parse(&svg);
    assert_eq!(
        els[0].attr("data-cloud-count"),
        "2",
        "Zähler: zwei Einfälle in der Wolke (ihre Unterknoten zählen nicht)"
    );
    let cloud =
        els.iter().find(|e| e.name == "g" && e.has_class("mm-cloud")).expect("die Wolke ist ein eigener Bereich");
    assert_eq!(cloud.attr("data-count"), "2", "auch am Bereich");
    // The helper renders in the German scope (the wording from before Round 39c).
    assert!(svg.contains("2 in der Wolke"), "der Zähler steht lesbar im Bild");
    let area = els.iter().find(|e| e.has_class("mm-cloud-area")).expect("Ablagefläche der Wolke");
    let (ax, ay, aw, ah) = (area.number("x"), area.number("y"), area.number("width"), area.number("height"));

    let (at, loose) = node_of(&els, "i:90");
    assert!(
        loose.has_class("mm-cloud-item") && loose.attr("data-cloud") == "1",
        "Einfall ohne Platz = Kärtchen der Wolke"
    );
    assert_eq!(loose.attr("data-side"), "", "Kärtchen der Wolke stehen auf keiner Seite des Baums");
    assert!(!loose.has_class("mm-main"), "und sind kein Hauptast");
    assert_eq!(loose.attr("data-editable"), "1", "sie bleiben Einfälle (umbenennen, ziehen, löschen)");
    assert_eq!(loose.attr("data-children"), "1", "die Unterknoten bleiben am Einfall (sie wandern beim Ziehen mit)");
    let (x, y, w, h) = rect_of(&els, at);
    assert!(x >= ax && y >= ay && x + w <= ax + aw && y + h <= ay + ah, "das Kärtchen liegt in der Fläche der Wolke");
    assert!(
        !nodes(&els).iter().any(|(_, e)| e.attr("data-node") == "i:91"),
        "Unterknoten werden in der Wolke nicht einzeln gezeichnet"
    );
    assert!(
        !els.iter().any(|e| e.has_class("mm-edge") && e.attr("data-to") == "i:90"),
        "kein Ast vom Werk in die Wolke"
    );
    // The tree lies above the cloud, nothing overlaps.
    for (at, e) in nodes(&els) {
        if e.attr("data-cloud") != "1" {
            let (_, ny, _, nh) = rect_of(&els, at);
            assert!(ny + nh <= ay, "{} liegt über der Wolke", e.attr("data-node"));
        }
    }

    // Proximity as help: suggestion of the cloud → „passt zu Ast …“ (here i:41 below „Sturm“).
    let (at, near) = node_of(&els, "i:92");
    assert_eq!(near.attr("data-fits"), "n:4", "der Vorschlag nennt den Hauptast des nächsten Einfalls");
    let hint = &els[child_at(&els, at, "mm-cloud-hint").expect("Vorschlag am Kärtchen")];
    assert_eq!(hint.text, "passt zu Ast „Sturm“", "in Worten");
    assert!(hint.has_class("mm-b3"), "in der Farbe dieses Asts");
    assert_eq!(node_of(&els, "i:90").1.attr("data-fits"), "", "ohne nahes Paar kein Vorschlag");
    // Proximity lines only in the tree (towards the cloud the suggestion is on the card).
    let near: Vec<&El> = els.iter().filter(|e| e.has_class("mm-near")).collect();
    assert_eq!(near.len(), 1, "eine gepunktete Linie für das Paar im Baum");
    assert_eq!((near[0].attr("data-from"), near[0].attr("data-to")), ("i:12", "i:21"), "zwischen den nahen Einfällen");
    map.filter.near = false;
    assert!(!fragment(&map).contains("mm-near"), "ohne zugeschaltete Nähe keine Linien");

    // Empty cloud: stays as a drop area.
    let empty = parse(&fragment(&three_levels()));
    assert_eq!(empty[0].attr("data-cloud-count"), "0", "Zähler 0");
    assert!(empty.iter().any(|e| e.has_class("mm-cloud-area")), "die Fläche bleibt (Ziel beim Ziehen)");

    // Visual check by hand (not part of the assertions): BARUCH_MM_SAMPLE=<file> writes a sample image with tree,
    // relations, proximity and cloud as a standalone SVG.
    if let Ok(path) = std::env::var("BARUCH_MM_SAMPLE") {
        map.filter.near = true;
        map.links =
            vec![link(1, 12, 21, LinkKind::LeadsTo, "weil es hell wird"), link(2, 111, 31, LinkKind::Contradicts, "")];
        map.root.children[0].children[0].has_note = true;
        map.root.children[0].children[1].key = Some(5);
        map.root.children[2].open_findings = 4;
        let options = RenderOptions { standalone: true, closed: vec!["n:4".to_owned()], ..Default::default() };
        std::fs::write(path, render(&map, &options).svg).expect("Beispielbild schreiben");
    }
}

#[test]
fn the_standalone_file_carries_its_own_style_and_colours_and_the_fragment_only_classes() {
    let mut map = three_levels();
    map.links = vec![link(1, 12, 21, LinkKind::Contradicts, "")];
    let file =
        render(&map, &RenderOptions { standalone: true, closed: vec!["n:2".to_owned()], ..Default::default() }).svg;
    let els = parse(&file);
    assert_eq!(els[0].attr("xmlns"), "http://www.w3.org/2000/svg", "die Datei trägt xmlns (sonst kein Bild)");
    assert!(els.iter().any(|e| e.name == "style" && e.text.contains("font-family")), "eigener Stil (Schrift)");
    assert!(!els.iter().any(|e| e.name == "a"), "keine Links in der Datei (relative Adressen führten ins Leere)");
    assert!(!file.contains("var(--"), "keine CSS-Variablen der Seite (kein anderes Programm kennt sie)");
    let edge = els.iter().find(|e| e.has_class("mm-edge") && e.attr("data-to") == "n:2").unwrap();
    assert_eq!(edge.attr("stroke"), BRANCH_COLORS[1], "Astfarbe fest eingebettet (zweiter Hauptast)");
    let main_box = &els[child_at(&els, node_of(&els, "n:2").0, "mm-box").unwrap()];
    assert_eq!(main_box.attr("stroke"), BRANCH_COLORS[1], "auch der Kasten");
    assert!(els.iter().any(|e| e.has_class("mm-toggle") && e.name == "g"), "der Einklapper steht als Zeichen da (+ n)");
    assert!(file.contains(">+ 2<"), "mit der Zahl der verborgenen Knoten");
    assert_eq!(BRANCH_COLORS.iter().collect::<BTreeSet<_>>().len(), 8, "acht verschiedene Astfarben");

    let page = fragment(&map);
    let els = parse(&page);
    assert_eq!(els[0].attr("xmlns"), "", "das Fragment für die Seite trägt kein xmlns");
    assert!(!els.iter().any(|e| e.name == "style"), "und keinen eigenen Stil (karte.css färbt, hell und dunkel)");
    for color in BRANCH_COLORS {
        assert!(!page.contains(color), "keine feste Farbe im Fragment ({color}): Farben kommen aus den CSS-Variablen");
    }
    assert!(
        els.iter().filter(|e| e.name == "path" || e.name == "rect").all(|e| e.attr("stroke").is_empty()),
        "kein stroke-Attribut im Fragment"
    );
}

// ---- Page and image route ------------------------------------------------------------------------------------

const BOOK: &str = "#set document(title: \"Ebbe & Flut\")\n\n= Priel\n\nText.\n\n= Watt\n\nMehr Text.\n";

async fn book() -> (common::Server, i64, Vec<i64>) {
    let server = common::open().await;
    let work = server.import(BOOK, "ebbe.typ").await.work;
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    // Chapters „Priel“ and „Watt“ in outline order.
    let keys = answer["result"]["work"]["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("work.get nennt die Punkte: {answer}"))
        .iter()
        .filter(|n| matches!(n["title"].as_str(), Some("Priel" | "Watt")))
        .map(|n| n["key"].as_i64().expect("key"))
        .collect();
    (server, work, keys)
}

async fn new_idea(server: &common::Server, params: serde_json::Value) -> i64 {
    let answer = server.rpc("idea.create", params).await;
    answer["result"]["idea"]["id"].as_i64().unwrap_or_else(|| panic!("Einfall angelegt: {answer}"))
}

#[tokio::test]
async fn the_tree_of_a_real_work_renders_with_escaped_titles() {
    let (server, work, _) = book().await;
    new_idea(&server, json!({ "work": work, "words": "Watt <nass>" })).await;
    let filter = MapFilter { near: false, ..MapFilter::default() };
    // The view of the page: tree and – next to it – the cloud, in which the idea without a place lies.
    let scene = tree::scene(&server.state, work, &filter).await.expect("scene gelingt");
    let map = &scene.map;
    let svg = mindmap::render(map, &RenderOptions::of(&scene, false, Vec::new())).svg;
    let els = parse(&svg);
    assert_eq!(nodes(&els).len(), map.walk().len(), "jeder Knoten aus Baum und Wolke ist gezeichnet");
    assert_eq!(els[0].attr("data-cloud-count"), "1", "der Einfall ohne Platz liegt in der Wolke");
    assert!(svg.contains("Ebbe &amp; Flut") && !svg.contains("Ebbe & Flut"), "der Werktitel steht maskiert im Bild");
    assert!(svg.contains("Watt &lt;nass&gt;") && !svg.contains("<nass>"), "Titel der Einfälle sind maskiert");
}

#[tokio::test]
async fn the_page_is_called_mindmap_and_carries_stage_ribbon_and_links_without_javascript() {
    let (server, work, keys) = book().await;
    let key = *keys.first().expect("das Werk hat ein Kapitel mit key");
    let placed = new_idea(&server, json!({ "work": work, "words": "Schlick", "key": key })).await;
    let loose = new_idea(&server, json!({ "work": work, "words": "Treibgut" })).await;

    let (status, html) = server.get(&format!("/werke/{work}/karte?status=open")).await;
    assert_eq!(status, StatusCode::OK, "die Seite antwortet");
    assert!(html.contains("<title>Mindmap · Ebbe &amp; Flut – Baruch</title>"), "Titel „Mindmap“ (Vertrag §0)");
    assert!(!html.contains("Gedankenkarte"), "das alte Wort steht nirgends mehr auf der Seite");
    let stage = html.split("class=\"mm-stage").nth(1).expect("Bühne .mm-stage").split('>').next().unwrap().to_owned();
    for attr in [
        "data-widget=\"mindmap\"".to_owned(),
        format!("data-work=\"{work}\""),
        format!("data-bild=\"/werke/{work}/karte/bild\""),
        "data-back=\"status=open\"".to_owned(),
    ] {
        assert!(stage.contains(&attr), "die Bühne trägt {attr} (DOM-Vertrag §6): {stage}");
    }
    assert!(
        html.contains("<svg class=\"mm-svg\"") && html.contains("<g class=\"mm-viewport\">"),
        "das Bild steht in der Seite"
    );
    assert!(
        html.contains(&format!("data-node=\"i:{placed}\"")) && html.contains(&format!("data-node=\"i:{loose}\"")),
        "beide Einfälle sind gezeichnet"
    );
    assert!(
        html.contains("data-mm-cloud-count=\"1\">1 in der Wolke"),
        "Zähler „n in der Wolke“: der Einfall ohne Platz"
    );

    // Ribbon: groups according to contract §9, as details.band-group as in Round 33 (laptop mode).
    let band = html.split("<main").next().unwrap();
    for group in ["Knoten", "Ansicht", "Ausgabe", "Einlesen", "Gliederungsansicht", "Zurück"] {
        assert!(
            band.contains(&format!("<details class=\"band-group\" data-band-group open><summary class=\"band-group-label\">{group}</summary>")),
            "Gruppe „{group}“ im Menüband als details.band-group[data-band-group]"
        );
    }
    for ext in ["xmind", "mm", "svg", "pdf", "md"] {
        assert!(
            band.contains(&format!("href=\"/werke/{work}/karte.{ext}?status=open\"")),
            "Ausgabe .{ext} mit dem Filter der Seite"
        );
    }
    assert!(band.contains(&format!("href=\"/werke/{work}/karte/einlesen\"")), "Einlesen führt zum Formular");
    let outline = format!("href=\"/werke/{work}/karte/gliederung?status=open\"");
    assert!(band.contains(&outline), "Gliederungsansicht mit dem Filter");
    for action in ["child", "sibling", "rename", "delete", "link", "undo", "cloud"] {
        let marker = format!("data-mm-action=\"{action}\"");
        let tag = band.split('<').find(|t| t.contains(&marker)).unwrap_or_else(|| panic!("Knopf {action} im Menüband"));
        assert!(
            tag.starts_with("a ") && tag.contains(&outline),
            "ohne JavaScript ist „{action}“ ein Link in die Gliederungsansicht: {tag}"
        );
    }
    assert!(
        html.contains("<a href=\"/werke/") && html.contains(">Gliederungsansicht</a>"),
        "Verweis auf die Gliederungsansicht auch im Text"
    );

    // Without JavaScript: fold toggle and section are links.
    let toggle = format!("class=\"mm-toggle\" href=\"?status=open&amp;zu=n:{key}\"");
    assert!(html.contains(&toggle), "der Einklapper des Kapitels ist ein Link, der zu= setzt: {toggle}");
    assert!(
        html.contains(&format!("class=\"mm-section\" href=\"/?work={work}&amp;key={key}\"")),
        "Einfall mit Bezug: Link zum Abschnitt"
    );
    assert!(
        html.contains("Alles zuklappen") && html.contains(&format!("zu=n:{key}")),
        "„Alles zuklappen“ ist ein Link mit zu="
    );

    // zu= really folds, wahl= marks the node; both stay pure display.
    let (_, closed) = server.get(&format!("/werke/{work}/karte?zu=n:{key},x:1,%22&wahl=i:{loose}")).await;
    assert!(
        !closed.contains(&format!("data-node=\"i:{placed}\"")),
        "unter dem eingeklappten Kapitel wird nichts gezeichnet"
    );
    assert!(
        closed.contains(&format!("data-zu=\"n:{key}\"")),
        "nur gültige Kennungen landen in data-zu (x:1 und Anführungszeichen fallen weg)"
    );
    assert!(
        closed.contains(&format!("data-node=\"i:{loose}\" aria-current=\"true\"")),
        "wahl= markiert den gewählten Knoten"
    );
    assert!(
        closed.contains(&format!("<input type=\"hidden\" name=\"zu\" value=\"n:{key}\">")),
        "der Filter behält das Einklappen"
    );

    let (status, _) = server.get("/werke/99999/karte").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk: 404");
}

#[tokio::test]
async fn the_image_route_returns_only_the_svg_with_the_same_filters() {
    let (server, work, keys) = book().await;
    let key = keys[0];
    let open = new_idea(&server, json!({ "work": work, "words": "Schlick", "key": key })).await;
    let done = new_idea(&server, json!({ "work": work, "words": "Tang", "key": key })).await;
    let answer = server.rpc("idea.update", json!({ "id": done, "status": "done" })).await;
    assert!(answer["error"].is_null(), "Status setzen gelingt: {answer}");

    let request = Request::builder().uri(format!("/werke/{work}/karte/bild")).body(Body::empty()).unwrap();
    let (status, headers, svg) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "die Bildroute antwortet");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml; charset=utf-8", "als SVG");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "no-store",
        "nie aus dem Zwischenspeicher (das Widget holt nach jeder Aktion neu)"
    );
    assert!(
        svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"mm-svg\"")
            && svg.trim_end().ends_with("</svg>"),
        "nur das SVG, für sich gültig"
    );
    assert!(!svg.contains("<html") && !svg.contains("band-group"), "keine Seite drumherum");
    let els = parse(&svg);
    let ids: BTreeSet<String> = nodes(&els).iter().map(|(_, e)| e.attr("data-node").to_owned()).collect();
    assert!(ids.contains(&format!("i:{open}")) && ids.contains(&format!("i:{done}")), "ohne Filter beide Einfälle");

    // Same filters as the page: status, zu, wahl.
    let (_, filtered) = server.get(&format!("/werke/{work}/karte/bild?status=open")).await;
    assert!(
        filtered.contains(&format!("data-node=\"i:{open}\"")) && !filtered.contains(&format!("data-node=\"i:{done}\"")),
        "status=open filtert wie auf der Seite"
    );
    assert!(filtered.contains(&format!("href=\"?status=open&amp;zu=n:{key}\"")), "Links im Bild behalten den Filter");
    let (_, page) = server.get(&format!("/werke/{work}/karte?status=open")).await;
    let in_page = page.split("data-mm-canvas>").nth(1).unwrap().split("</svg>").next().unwrap();
    assert_eq!(
        filtered.replacen(" xmlns=\"http://www.w3.org/2000/svg\"", "", 1).trim_end_matches("</svg>"),
        in_page,
        "Seite und Bildroute zeichnen dasselbe Bild (eine Anordnung)"
    );
    let (_, closed) = server.get(&format!("/werke/{work}/karte/bild?zu=n:{key}&wahl=n:{key}")).await;
    assert!(!closed.contains("mm-idea"), "zu= klappt auch hier");
    assert!(closed.contains(&format!("data-node=\"n:{key}\" aria-current=\"true\"")), "wahl= markiert auch hier");
    let (status, _) = server.get("/werke/99999/karte/bild").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk: 404");
}

// ---- Widget --------------------------------------------------------------------------------------------------

/// RPC that the widget may call (contract §7 and acceptance).
const ALLOWED_METHODS: &[&str] = &[
    "idea.create",
    "idea.update",
    "idea.move",
    "idea.delete",
    "idea.restore",
    "idea.link",
    "idea.unlink",
    "idea.link_restore",
];

/// All RPC methods in `call("…")` of the compiled script.
fn methods_in(js: &str) -> BTreeSet<String> {
    js.match_indices("call(\"")
        .map(|(at, m)| &js[at + m.len()..])
        .map(|rest| rest[..rest.find('"').unwrap()].to_owned())
        .collect()
}

#[tokio::test]
async fn the_widget_is_compiled_bundled_and_binds_the_keys_and_only_the_allowed_rpc() {
    let server = common::open().await;
    let (status, js) = server.get("/static/js/mindmap.js").await;
    assert_eq!(status, StatusCode::OK, "build.rs übersetzt mindmap.ts, der Server liefert es aus");
    assert!(js.contains("register(\"mindmap\""), "es registriert das Widget mindmap (data-widget der Bühne)");
    assert!(!js.contains("interface Info") && !js.contains(": Info"), "tsc hat die Typen entfernt (JavaScript)");
    let (_, main) = server.get("/static/js/main.js").await;
    assert!(main.contains("import \"./mindmap.js\""), "main.js bindet das Widget ein");
    assert!(!main.contains("idea_map.js"), "das alte Widget ist nicht mehr eingebunden");
    let (status, _) = server.get("/static/js/idea_map.js").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "das alte Widget idea_map.ts ist gelöscht und wird nicht mehr ausgeliefert"
    );
    assert!(!js.contains("STUB"), "kein Stub im Widget");

    // Only the RPC of the contract, and each of them is needed (action or reversal for Ctrl+Z).
    let methods = methods_in(&js);
    let allowed: BTreeSet<String> = ALLOWED_METHODS.iter().map(|m| (*m).to_owned()).collect();
    assert_eq!(methods, allowed, "das Widget ruft genau die RPC aus §7");
    assert!(!js.contains("idea.map_pos"), "es ordnet nichts selbst an und speichert keine Lagen");
    assert!(
        js.contains("dataset.bild") && js.contains("fetch("),
        "nach jeder Aktion holt es das Bild über data-bild neu"
    );

    // Key bindings §7: every key is in the script as a case of its own or a query of its own.
    for key in [
        "\"Tab\"",
        "\"Enter\"",
        "\"F2\"",
        "\"Delete\"",
        "\"Backspace\"",
        "\" \"",
        "\"ArrowLeft\"",
        "\"ArrowRight\"",
        "\"ArrowUp\"",
        "\"ArrowDown\"",
        "\"+\"",
        "\"-\"",
        "\"0\"",
        "\"Escape\"",
    ] {
        assert!(js.contains(&format!("case {key}:")), "Taste {key} ist belegt");
    }
    assert!(js.contains("toLowerCase() === \"z\"") && js.contains("ctrlKey"), "Strg+Z nimmt die letzte Aktion zurück");
    assert!(js.contains("\"KeyL\"") && js.contains("\"KeyE\""), "Alt+L verbindet, Alt+E legt in die Wolke");
    assert!(js.contains("\"dblclick\""), "Doppelklick benennt um");
    assert!(js.contains("\"wheel\"") && js.contains("passive: false"), "Strg+Mausrad zoomt (ohne die Seite zu rollen)");
    assert!(js.contains("window.confirm("), "Löschen fragt nach");
    assert!(
        js.contains("\"pointerdown\"") && js.contains("elementFromPoint"),
        "Ziehen mit Ablageziel unter dem Zeiger"
    );
    // Reversal RPC per action (one level).
    for (action, reverse) in [
        ("idea.create", "idea.delete"),
        ("idea.delete", "idea.restore"),
        ("idea.link", "idea.unlink"),
        ("idea.unlink", "idea.link_restore"),
    ] {
        let needle = format!("call(\"{action}\"");
        let paired = js.match_indices(&needle).any(|(at, _)| {
            let end = (at..=(at + 900).min(js.len())).rev().find(|i| js.is_char_boundary(*i)).unwrap();
            js[at..end].contains(&format!("call(\"{reverse}\""))
        });
        assert!(paired, "{action} merkt sich {reverse} als Umkehr");
    }
    // Cloud: dropping in the cloud or on the work detaches from the parent node and from the reference.
    assert!(js.contains("parent: null, key: null"), "in die Wolke = parent und key leer (idea.move)");
    assert!(js.contains(".mm-cloud"), "die Wolke ist Ablageziel");
    // Texts never as HTML – except the finished image of the server.
    assert_eq!(js.matches(".innerHTML").count(), 1, "innerHTML nur für das vom Server maskierte Bild");
}

#[tokio::test]
async fn branch_colours_are_css_variables_for_light_and_dark_and_the_menu_says_mindmap() {
    let (server, work, _) = book().await;
    let (_, css) = server.get("/static/app.css").await;
    for n in 0..8 {
        let name = format!("--mm-b{n}:");
        assert!(css.matches(&name).count() >= 3, "{name} ist für hell, dunkel (System) und dunkel (Schalter) gesetzt");
        assert!(css.contains(&format!(".mm-b{n} {{ --mm-c: var(--mm-b{n}); }}")), "Klasse mm-b{n} wählt die Astfarbe");
    }
    let dark = css.split(":root[data-theme=\"dark\"] {\n  --mm-b0").nth(1).expect("dunkle Astfarben am Schalter");
    assert!(dark.starts_with(": #7fb0ff"), "im Dunkeln hellere Farben (lesbar auf dunklem Grund)");
    assert!(css.contains(".mm-edge { stroke: var(--mm-c"), "Äste nehmen die Farbe ihres Hauptasts");
    assert!(
        css.contains(".mm-link.mm-contradiction { stroke: var(--danger)"),
        "Widersprüche in der Warnfarbe (und fetter)"
    );
    let (_, home) = server.get(&format!("/?work={work}")).await;
    assert!(
        home.contains(&format!("href=\"/werke/{work}/karte\"")) && home.contains(">Mindmap</a>"),
        "das Menüband nennt die Seite „Mindmap“"
    );
    assert!(!home.contains(">Gedankenkarte</a>"), "nicht mehr „Gedankenkarte“");
}
