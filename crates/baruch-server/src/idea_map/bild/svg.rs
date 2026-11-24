// SPDX-License-Identifier: AGPL-3.0-or-later
//! SVG der Gedankenkarte nach dem DOM-Vertrag (Vertrag Runde 25 §4.4): dieselbe Zeichnung im HTML der Seite
//! (Stil aus `static/karte.css`, Hell/Dunkel über die Farben von `app.css`) und als eigenständige Datei
//! `karte.svg` (eingebetteter Stil, hell, auch Quelle des PDFs).
//!
//! - Alle Kanten sind `<line>` von Mittelpunkt zu Mittelpunkt (`data-x`/`data-y` der Knoten), Reihenfolge: Baum
//!   (`im-tree`), Bezug (`im-ref`), Verbindungen (`im-link im-kind-<art>`, mit `<title>`), Nähe (`im-near`, nur
//!   mit `near`). Kanten ohne gezeigte Endknoten fehlen.
//! - Runde 18a: Gerichtete Verbindungen („führt weiter“, „gehört zu“, „Beispiel für“) tragen `data-arrow` und
//!   `marker-end="url(#im-arrow-<art>)"` (Pfeilspitzen in `<defs>` vor dem Ansichtsfenster) und enden am Rand des
//!   Zielkreises statt im Mittelpunkt (sonst läge die Spitze unter dem Kreis); „widerspricht“ ist ungerichtet,
//!   ohne Spitze. Jede Verbindung und jede Nähe-Kante hat eine sichtbare Beschriftung
//!   `<text class="im-edge-label im-label-<art>" data-from data-to>` in der Mitte (Gruppe `im-edge-labels` als
//!   letzte, über den Knoten): die Art („führt weiter“) bzw. „nah“. Die Strichart steht als Attribut
//!   `stroke-dasharray` an der Kante (und gleich im CSS): Baum durchgezogen, Bezug gestrichelt, „führt weiter“
//!   durchgezogen mit Spitze, „gehört zu“ lang gestrichelt, „Beispiel für“ gepunktet, „widerspricht“ Strich-Punkt,
//!   Nähe fein gepunktet.
//! - Knoten: Werk (`<g>`), Teile/Kapitel/Abschnitte (`<a>` zum Abschnitt in Key-Form), „Ohne Kapitel“ (`<g>`),
//!   Einfälle (`<a>` auf `/einfaelle/<id>`). Jeder Knoten steht mit `transform="translate(x y)"` an seinem Platz,
//!   Kreis und Text relativ dazu.
//! - Unterscheidung der Verbindungsarten nicht nur durch Farbe: Strichart je Art, Pfeilspitze, sichtbare
//!   Beschriftung und `<title>`.
//! - Alle Nutzertexte (Titel, Schlagwörter, Notizen) laufen durch [`esc`].

use std::collections::HashSet;
use std::fmt::Write as _;

use baruch_store::{IdeaStatus, LinkKind};

use super::layout::Layout;
use crate::idea_map::{IdeaMap, MapIdea, MapNode, MapNodeKind, NearVia};

/// Höchstlänge der sichtbaren Beschriftung eines Knotens bzw. eines Einfalls (der volle Text steht im `<title>`).
const NODE_LABEL_CHARS: usize = 18;
const IDEA_LABEL_CHARS: usize = 8;
/// Höchstlänge des Gedankens im `<title>` eines Einfalls.
const NOTE_TITLE_CHARS: usize = 140;

/// Text für Inhalt und Attribute: `& < > " '` maskiert, Steuerzeichen (außer Tab/Zeilenumbruch) und die in XML 1.0
/// verbotenen Nichtzeichen U+FFFE/U+FFFF entfernt.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\n' | '\t' => out.push(' '),
            c if c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}') => {}
            c => out.push(c),
        }
    }
    out
}

/// Zahl für Koordinaten: auf eine Nachkommastelle, ohne `.0` und ohne `-0`.
pub fn num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if r == 0.0 {
        "0".to_owned()
    } else if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}")
    }
}

/// Auf `max` Zeichen gekürzt (mit „…“), Zeilenumbrüche als Leerzeichen.
pub fn cut(text: &str, max: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        one
    } else {
        let mut s: String = one.chars().take(max.saturating_sub(1)).collect();
        s = s.trim_end().to_owned();
        s.push('…');
        s
    }
}

/// „offen“ bzw. „verarbeitet“.
pub fn status_label(status: IdeaStatus) -> &'static str {
    match status {
        IdeaStatus::Open => "offen",
        IdeaStatus::Done => "verarbeitet",
    }
}

/// „1.2 Titel“ bzw. „Titel“.
pub fn node_label(node: &MapNode) -> String {
    if node.number.is_empty() { node.title.clone() } else { format!("{} {}", node.number, node.title) }
}

/// Beschriftung eines Einfalls: die Schlagwörter, sonst „Einfall n“.
pub fn idea_label(idea: &MapIdea) -> String {
    if idea.label.trim().is_empty() { format!("Einfall {}", idea.id) } else { idea.label.clone() }
}

/// „3 Einfälle, 2 offen“.
pub fn counts(node: &MapNode) -> String {
    let word = if node.total_ideas == 1 { "Einfall" } else { "Einfälle" };
    format!("{} {word}, {} offen", node.total_ideas, node.open_ideas)
}

/// Strichart einer Verbindungsart (`stroke-dasharray`; `None` = durchgezogen). Dieselben Werte stehen in
/// `static/karte.css` und [`STANDALONE_STYLE`].
pub fn link_dash(kind: LinkKind) -> Option<&'static str> {
    match kind {
        LinkKind::LeadsTo => None,
        LinkKind::BelongsTo => Some("9 4"),
        LinkKind::ExampleOf => Some("2 4"),
        LinkKind::Contradicts => Some("12 3 2 3"),
    }
}

/// Strichart der Bezugskanten (Einfall → Anker) und der Nähe.
const REF_DASH: &str = "4 3";
const NEAR_DASH: &str = "1 3";

/// Farbe der Pfeilspitze in der eigenständigen Datei (hell; auf der Seite setzt `karte.css` die Farbe des Themas).
fn arrow_fill(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::LeadsTo => "#3b5bdb",
        LinkKind::BelongsTo => "#2b8a3e",
        LinkKind::ExampleOf => "#9a5800",
        LinkKind::Contradicts => "#c92a2a",
    }
}

/// Pfeilspitzen der gerichteten Arten (`<marker id="im-arrow-<art>">`), Größe in Einheiten der Karte.
pub fn arrow_defs() -> String {
    let mut out = String::from("<defs>");
    for kind in LinkKind::ALL.into_iter().filter(|k| !k.symmetric()) {
        let kind_str = kind.as_str();
        let _ = write!(
            out,
            "<marker id=\"im-arrow-{kind_str}\" class=\"im-arrow im-arrow-{kind_str}\" viewBox=\"0 0 10 10\" refX=\"10\" \
             refY=\"5\" markerWidth=\"11\" markerHeight=\"11\" markerUnits=\"userSpaceOnUse\" orient=\"auto\">\
             <path d=\"M0 0L10 5L0 10z\" fill=\"{}\"/></marker>",
            arrow_fill(kind)
        );
    }
    out.push_str("</defs>");
    out
}

fn kind_class(kind: MapNodeKind) -> &'static str {
    match kind {
        MapNodeKind::Work => "im-work",
        MapNodeKind::Part => "im-part",
        MapNodeKind::Chapter => "im-chapter",
        MapNodeKind::Section => "im-section",
        MapNodeKind::NoChapter => "im-none",
    }
}

/// „führt weiter: Einfall 12 → Einfall 79 – Notiz“ (Titel einer Verbindung).
pub fn link_title(kind: LinkKind, from: i64, to: i64, note: &str) -> String {
    let arrow = if kind.symmetric() { "↔" } else { "→" };
    let mut title = format!("{}: Einfall {from} {arrow} Einfall {to}", kind.label());
    if !note.trim().is_empty() {
        title.push_str(" – ");
        title.push_str(&cut(note, NOTE_TITLE_CHARS));
    }
    title
}

/// Stil der eigenständigen Datei (und des PDFs): feste helle Farben, Strichart je Verbindungsart. Nur einfache
/// Klassen-Selektoren (der SVG-Leser im Typst-Satz kennt keine Variablen und keine Medienabfragen).
pub const STANDALONE_STYLE: &str = "\
svg{background:#ffffff}\
text{font-family:\"DejaVu Sans\",\"Libertinus Serif\",sans-serif;fill:#1d2125}\
.im-edge{stroke:#5f6b76;stroke-width:1.2;fill:none}\
.im-tree{stroke:#9aa5b1;stroke-width:2}\
.im-ref{stroke:#c3cad1;stroke-width:1;stroke-dasharray:4 3}\
.im-link{stroke-width:2.4}\
.im-kind-leads_to{stroke:#3b5bdb}\
.im-kind-belongs_to{stroke:#2b8a3e;stroke-dasharray:9 4}\
.im-kind-example_of{stroke:#9a5800;stroke-dasharray:2 4}\
.im-kind-contradicts{stroke:#c92a2a;stroke-width:3;stroke-dasharray:12 3 2 3}\
.im-near{stroke:#7d97ff;stroke-width:1;stroke-dasharray:1 3}\
.im-edge-label{pointer-events:none;font-size:9px;fill:#5f6b76;paint-order:stroke;stroke:#ffffff;stroke-width:3px;stroke-linejoin:round}\
.im-label-leads_to{fill:#3b5bdb}\
.im-label-belongs_to{fill:#2b8a3e}\
.im-label-example_of{fill:#9a5800}\
.im-label-contradicts{fill:#c92a2a}\
.im-node circle{stroke:#5f6b76;stroke-width:1.5;fill:#ffffff}\
.im-work circle{fill:#e7ecff;stroke:#3b5bdb;stroke-width:2.5}\
.im-part circle{fill:#e7ecff;stroke:#3b5bdb;stroke-width:2}\
.im-chapter circle{fill:#f6f7f9}\
.im-section circle{fill:#ffffff;stroke-dasharray:4 2}\
.im-none circle{fill:#f6f7f9;stroke-dasharray:3 3}\
.im-empty circle{fill:#ffffff;stroke:#9a5800;stroke-width:2;stroke-dasharray:5 3}\
.im-empty .im-count{font-size:10px;fill:#9a5800}\
.im-open circle{fill:#fff3db;stroke:#9a5800}\
.im-done circle{fill:#e6f4ea;stroke:#2b8a3e}\
.im-pinned circle{stroke-width:3}\
.im-dim{opacity:.35}\
.im-count{font-size:15px;font-weight:bold}\
.im-label{font-size:11px}\
.im-id{font-size:9px}\
.im-idea .im-label{font-size:9px}\
.im-work .im-label{font-size:13px;font-weight:bold}";

/// SVG der Karte. `standalone`: eigene Datei mit XML-Kopf, Größe, `<title>` und eingebettetem Stil (Attribute
/// ohne Wert dann als `=""`, damit es wohlgeformtes XML bleibt).
pub fn render(map: &IdeaMap, layout: &Layout, standalone: bool) -> String {
    let flag = |name: &str| if standalone { format!(" {name}=\"\"") } else { format!(" {name}") };
    let (vx, vy, vw, vh) = layout.view;
    let mut out = String::with_capacity(4096 + 600 * (map.nodes.len() + map.ideas.len()));
    if standalone {
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    let _ = write!(
        out,
        "<svg class=\"im-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\"{} role=\"group\" \
         aria-label=\"Gedankenkarte „{}“\"{}>",
        num(vx),
        num(vy),
        num(vw),
        num(vh),
        if standalone { format!(" width=\"{}\" height=\"{}\"", num(vw), num(vh)) } else { String::new() },
        esc(&map.title),
        flag("data-im-svg"),
    );
    if standalone {
        let _ = write!(out, "\n<title>Gedankenkarte „{}“</title>\n<style>{STANDALONE_STYLE}</style>", esc(&map.title));
    }
    let _ = write!(out, "\n{}", arrow_defs());
    let _ = write!(out, "\n<g class=\"im-viewport\"{}>\n<g class=\"im-edges\">\n", flag("data-im-viewport"));

    // Beschriftungen der Kanten (Runde 18a), gezeichnet nach allen Kanten und vor den Knoten.
    let mut labels = String::new();
    // `style`: Strichart, Pfeil (Ende am Kreisrand) und Beschriftung (Klasse, Text) der Kante.
    struct EdgeStyle<'a> {
        dash: Option<&'a str>,
        arrow: Option<&'a str>,
        label: Option<(String, &'a str)>,
    }
    let plain = || EdgeStyle { dash: None, arrow: None, label: None };
    let mut line = |out: &mut String,
                    class: &str,
                    extra: &str,
                    from: &str,
                    to: &str,
                    title: Option<String>,
                    style: EdgeStyle<'_>| {
        let (Some(a), Some(b)) = (layout.get(from), layout.get(to)) else { return };
        let (mut x2, mut y2) = (b.x, b.y);
        if style.arrow.is_some() {
            // Ende am Rand des Zielkreises (2 Einheiten Luft), damit die Spitze sichtbar vor dem Kreis liegt.
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let d = dx.hypot(dy);
            if d > b.r + 2.0 {
                x2 = b.x - dx / d * (b.r + 2.0);
                y2 = b.y - dy / d * (b.r + 2.0);
            }
        }
        let _ = write!(
            out,
            "<line class=\"im-edge {class}\"{extra} data-from=\"{}\" data-to=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
            esc(from),
            esc(to),
            num(a.x),
            num(a.y),
            num(x2),
            num(y2)
        );
        if let Some(dash) = style.dash {
            let _ = write!(out, " stroke-dasharray=\"{dash}\"");
        }
        if let Some(kind) = style.arrow {
            let _ = write!(out, " marker-end=\"url(#im-arrow-{kind})\"{}", flag("data-arrow"));
        }
        match title {
            Some(t) => {
                let _ = writeln!(out, "><title>{}</title></line>", esc(&t));
            }
            None => out.push_str("/>\n"),
        }
        if let Some((text, label_class)) = style.label {
            let _ = writeln!(
                labels,
                "<text class=\"im-edge-label {label_class}\" data-from=\"{}\" data-to=\"{}\" x=\"{}\" y=\"{}\" \
                 text-anchor=\"middle\">{}</text>",
                esc(from),
                esc(to),
                num((a.x + b.x) / 2.0),
                num((a.y + b.y) / 2.0 - 4.0),
                esc(&text)
            );
        }
    };

    // Baum: jeder Knoten zu seinem Elternknoten.
    for node in &map.nodes {
        if let Some(parent) = &node.parent {
            line(&mut out, "im-tree", "", &node.id, parent, None, plain());
        }
    }
    // Bezug: jeder Einfall zu seinem Anker.
    for idea in &map.ideas {
        line(&mut out, "im-ref", "", &idea.node, &idea.anchor, None, EdgeStyle { dash: Some(REF_DASH), ..plain() });
    }
    // Verbindungen zwischen gezeigten Einfällen.
    let shown: HashSet<i64> = map.ideas.iter().map(|i| i.id).collect();
    for link in &map.links {
        if link.deleted.is_some() || !shown.contains(&link.from) || !shown.contains(&link.to) {
            continue;
        }
        let kind = link.kind.as_str();
        let extra = format!(" data-link=\"{}\" data-kind=\"{kind}\"", link.id);
        let label_class = format!("im-label-{kind}");
        line(
            &mut out,
            &format!("im-link im-kind-{kind}"),
            &extra,
            &format!("i:{}", link.from),
            &format!("i:{}", link.to),
            Some(link_title(link.kind, link.from, link.to, &link.note)),
            EdgeStyle {
                dash: link_dash(link.kind),
                arrow: (!link.kind.symmetric()).then_some(kind),
                label: Some((link.kind.label().to_owned(), &label_class)),
            },
        );
    }
    // Nähe (nur Anzeige), nur mit near.
    if map.filter.near {
        for pair in &map.near.pairs {
            if !shown.contains(&pair.a) || !shown.contains(&pair.b) {
                continue;
            }
            let via = match pair.via {
                NearVia::Vector => "Bedeutung".to_owned(),
                NearVia::Words => format!("gleiche Schlagwörter: {}", pair.shared_words.join(", ")),
                NearVia::Both => format!("Bedeutung und Schlagwörter: {}", pair.shared_words.join(", ")),
            };
            let extra = format!(" data-score=\"{:.2}\"", pair.score);
            let title = format!("nah ({:.2}, {via}): Einfall {} und Einfall {}", pair.score, pair.a, pair.b);
            let style =
                EdgeStyle { dash: Some(NEAR_DASH), arrow: None, label: Some(("nah".to_owned(), "im-label-near")) };
            line(&mut out, "im-near", &extra, &format!("i:{}", pair.a), &format!("i:{}", pair.b), Some(title), style);
        }
    }
    out.push_str("</g>\n<g class=\"im-nodes\">\n");

    let work = map.work;
    for node in &map.nodes {
        let Some(p) = layout.get(&node.id) else { continue };
        let mut class = format!("im-node {}", kind_class(node.kind));
        if node.total_ideas == 0 && matches!(node.kind, MapNodeKind::Part | MapNodeKind::Chapter | MapNodeKind::Section)
        {
            class.push_str(" im-empty");
        }
        // Runde 28: Ebene „Befunde“ – Kapitel mit vielen offenen Befunden hervorgehoben (Klasse und Text „k offen“).
        let findings_layer = map.filter.findings && node.kind != MapNodeKind::Work;
        if findings_layer && node.open_findings as usize >= crate::findings::page::MANY_OPEN {
            class.push_str(" im-many-findings");
        }
        let place = format!(
            " data-x=\"{}\" data-y=\"{}\" data-r=\"{}\" transform=\"translate({} {})\"",
            num(p.x),
            num(p.y),
            num(p.r),
            num(p.x),
            num(p.y)
        );
        let label = match node.kind {
            MapNodeKind::Work => node.title.clone(),
            _ => node_label(node),
        };
        let mut title = format!("{label} – {}", counts(node));
        if findings_layer && node.open_findings > 0 {
            title.push_str(&format!(" · {} offene Befunde", node.open_findings));
        }
        // Leere Teile und Kapitel tragen das Wort „leer“ im Kreis (hervorgehoben, nicht nur über die Farbe).
        let empty_point = matches!(node.kind, MapNodeKind::Part | MapNodeKind::Chapter | MapNodeKind::Section)
            && node.total_ideas == 0;
        let count = if empty_point {
            "leer".to_owned()
        } else if node.total_ideas == 0 {
            String::new()
        } else {
            node.open_ideas.to_string()
        };
        let findings_text = if findings_layer && node.open_findings > 0 {
            format!(
                "<tspan class=\"im-findings\" x=\"0\" y=\"{}\">{} offen</tspan>",
                num(-(p.r + 6.0)),
                node.open_findings
            )
        } else {
            String::new()
        };
        let body = format!(
            "<title>{}</title><circle r=\"{}\"/><text text-anchor=\"middle\"><tspan class=\"im-count\" x=\"0\" \
             y=\"5\">{count}</tspan><tspan class=\"im-label\" x=\"0\" y=\"{}\">{}</tspan>{findings_text}</text>",
            esc(&title),
            num(p.r),
            num(p.r + 14.0),
            esc(&cut(&label, if node.kind == MapNodeKind::Work { 24 } else { NODE_LABEL_CHARS })),
        );
        match (node.kind, node.key) {
            (MapNodeKind::Part | MapNodeKind::Chapter | MapNodeKind::Section, Some(key)) => {
                let _ = writeln!(
                    out,
                    "<a class=\"{class}\" data-node=\"{}\" data-key=\"{key}\" data-open-ideas=\"{}\"{} \
                     href=\"/?work={work}&amp;key={key}\"{place}>{body}</a>",
                    esc(&node.id),
                    node.open_ideas,
                    if findings_layer {
                        format!(" data-open-findings=\"{}\"", node.open_findings)
                    } else {
                        String::new()
                    },
                );
            }
            _ => {
                let _ = writeln!(
                    out,
                    "<g class=\"{class}\" data-node=\"{}\" data-open-ideas=\"{}\"{place}>{body}</g>",
                    esc(&node.id),
                    node.open_ideas,
                );
            }
        }
    }
    let searching = map.filter.q.as_deref().is_some_and(|q| !q.trim().is_empty());
    for idea in &map.ideas {
        let Some(p) = layout.get(&idea.node) else { continue };
        let status = status_label(idea.status);
        let mut class =
            format!("im-node im-idea {}", if idea.status == IdeaStatus::Open { "im-open" } else { "im-done" });
        if p.pinned {
            class.push_str(" im-pinned");
        }
        if searching {
            class.push_str(if idea.hit { " im-hit" } else { " im-dim" });
        }
        let label = idea_label(idea);
        let mut title = format!("Einfall {}: {label}", idea.id);
        if !idea.note.trim().is_empty() {
            title.push_str(" – ");
            title.push_str(&cut(&idea.note, NOTE_TITLE_CHARS));
        }
        let _ = write!(title, " ({status})");
        let key = idea.key.map(|k| k.to_string()).unwrap_or_default();
        let _ = writeln!(
            out,
            "<a class=\"{class}\" data-node=\"{}\" data-idea=\"{}\" data-key=\"{key}\" data-anchor=\"{}\" \
             href=\"/einfaelle/{}\" data-x=\"{}\" data-y=\"{}\" data-r=\"{}\" transform=\"translate({} {})\" \
             aria-label=\"{}\"><title>{}</title><circle r=\"{}\"/><text text-anchor=\"middle\"><tspan class=\"im-id\" \
             x=\"0\" y=\"3\">{}</tspan><tspan class=\"im-label\" x=\"0\" y=\"{}\">{}</tspan></text></a>",
            esc(&idea.node),
            idea.id,
            esc(&idea.anchor),
            idea.id,
            num(p.x),
            num(p.y),
            num(p.r),
            num(p.x),
            num(p.y),
            esc(&format!("Einfall {}: {label} ({status})", idea.id)),
            esc(&title),
            num(p.r),
            idea.id,
            num(p.r + 11.0),
            esc(&cut(&label, IDEA_LABEL_CHARS)),
        );
    }
    // Beschriftungen zuletzt, über den Knoten (Integration Runde 18, Sichtprüfung): Bei kurzen Kanten zwischen
    // nahen Einfällen lag die Mitte sonst unter den Kreisen und war nicht lesbar; der helle Rand hält sie lesbar,
    // `pointer-events: none` lässt Klicks zu den Knoten durch.
    out.push_str("</g>\n<g class=\"im-edge-labels\">\n");
    out.push_str(&labels);
    out.push_str("</g>\n</g>\n</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_escaped_for_content_and_attributes() {
        assert_eq!(esc("<b>\"A\" & 'B'</b>"), "&lt;b&gt;&quot;A&quot; &amp; &#39;B&#39;&lt;/b&gt;", "alles maskiert");
        assert_eq!(esc("a\u{7}b\nc"), "ab c", "Steuerzeichen fallen weg, Zeilenumbruch wird Leerzeichen");
        assert_eq!(esc("x\u{fffe}y\u{ffff}z"), "xyz", "Nichtzeichen sind in XML 1.0 verboten und fallen weg");
    }

    #[test]
    fn numbers_are_short_and_stable() {
        assert_eq!(num(0.04), "0", "fast null wird 0, nie -0");
        assert_eq!(num(-0.04), "0", "auch von unten");
        assert_eq!(num(12.0), "12", "ganze Zahl ohne .0");
        assert_eq!(num(-3.26), "-3.3", "eine Nachkommastelle");
    }

    #[test]
    fn long_labels_are_cut_with_an_ellipsis() {
        assert_eq!(cut("Laube, Rose", 18), "Laube, Rose", "kurz bleibt");
        assert_eq!(cut("Laube,\nRose und Dorn", 9), "Laube, R…", "gekürzt auf 9 Zeichen samt „…“");
    }

    #[test]
    fn every_kind_has_its_own_stroke_and_only_directed_kinds_get_an_arrow() {
        let dashes: Vec<Option<&str>> = LinkKind::ALL.into_iter().map(link_dash).collect();
        for (i, a) in dashes.iter().enumerate() {
            for b in &dashes[i + 1..] {
                assert_ne!(a, b, "jede Verbindungsart hat eine eigene Strichart: {dashes:?}");
            }
        }
        assert!(!dashes.contains(&Some(REF_DASH)), "der Bezug sieht anders aus als jede Verbindung");
        let defs = arrow_defs();
        for kind in LinkKind::ALL {
            let id = format!("id=\"im-arrow-{}\"", kind.as_str());
            assert_eq!(
                defs.contains(&id),
                !kind.symmetric(),
                "Pfeilspitze nur für gerichtete Arten ({kind:?}): {defs}"
            );
        }
    }

    #[test]
    fn link_titles_name_the_kind_first() {
        assert_eq!(
            link_title(LinkKind::Contradicts, 48, 79, "nicht beides"),
            "widerspricht: Einfall 48 ↔ Einfall 79 – nicht beides",
            "DOM-Vertrag: <title>widerspricht: …Notiz…</title>, ungerichtet mit ↔"
        );
        assert_eq!(link_title(LinkKind::LeadsTo, 1, 2, ""), "führt weiter: Einfall 1 → Einfall 2", "gerichtet mit →");
    }
}
