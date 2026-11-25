// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ausgaben der Gedankenkarte als Text (Vertrag Runde 25 §4.3): Markdown (`karte.md`, Gliederung Kapitel →
//! Einfälle → Verbindungen) und FreeMind (`karte.mm`, `<map version="1.0.1">`, Verbindungen als `arrowlink`).
//! Beide aus derselben [`IdeaMap`] wie Seite und SVG – die Filter gelten also auch hier.

use std::collections::HashMap;
use std::fmt::Write as _;

use baruch_store::{IdeaId, IdeaLink, LinkKind};

use crate::idea_map::bild::svg::{counts, cut, idea_label, node_label, status_label};
use crate::idea_map::{IdeaMap, MapFilter, MapIdea, MapNode, MapNodeKind, NearVia};

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

/// Knoten in der Reihenfolge der Karte mit ihrer Tiefe (Werk 0); Kinder direkt hinter ihren Eltern.
pub fn tree_order(map: &IdeaMap) -> Vec<(&MapNode, usize)> {
    let mut out = Vec::new();
    fn walk<'a>(map: &'a IdeaMap, parent: &str, depth: usize, out: &mut Vec<(&'a MapNode, usize)>) {
        for node in map.nodes.iter().filter(|n| n.parent.as_deref() == Some(parent)) {
            if out.iter().any(|(n, _): &(&MapNode, usize)| n.id == node.id) || depth > 16 {
                continue;
            }
            out.push((node, depth));
            walk(map, &node.id, depth + 1, out);
        }
    }
    walk(map, "w", 1, &mut out);
    // Knoten mit unbekanntem Elternknoten nicht verlieren.
    for node in &map.nodes {
        if node.id != "w" && !out.iter().any(|(n, _)| n.id == node.id) {
            out.push((node, 1));
        }
    }
    out
}

fn ideas_at<'a>(map: &'a IdeaMap, anchor: &str) -> Vec<&'a MapIdea> {
    map.ideas.iter().filter(|i| i.anchor == anchor).collect()
}

/// Bezeichnung eines Abschnitts unter einem Kapitel („Abschnitt 1.2.1 Tiefe“) für den genauen Bezug.
fn section_of(node: &MapNode, idea: &MapIdea) -> Option<String> {
    let key = idea.key?;
    if node.key == Some(key) {
        return None;
    }
    node.sections.iter().find(|s| s.key == key).map(|s| {
        if s.number.is_empty() {
            format!("Abschnitt {}", s.title)
        } else {
            format!("Abschnitt {} {}", s.number, s.title)
        }
    })
}

/// Markdown: Titel, Filter, je Knoten eine Überschrift (Tiefe = Ebene), darunter die Einfälle mit ihren
/// Verbindungen, am Ende „Widersprüche“ und (falls gezeigt) „Nähe“.
pub fn markdown(map: &IdeaMap) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Gedankenkarte: {}\n", md(&map.title));
    let filter = filter_words(&map.filter);
    if !filter.is_empty() {
        let _ = writeln!(out, "Filter: {}\n", md(&filter));
    }
    let labels: HashMap<IdeaId, String> = map.ideas.iter().map(|i| (i.id, idea_label(i))).collect();
    let write_ideas = |out: &mut String, node: Option<&MapNode>, ideas: &[&MapIdea]| {
        for idea in ideas {
            let mut line = format!("- Einfall {}: {} ({})", idea.id, md(&idea_label(idea)), status_label(idea.status));
            if let Some(section) = node.and_then(|n| section_of(n, idea)) {
                let _ = write!(line, ", {}", md(&section));
            }
            if !idea.note.trim().is_empty() {
                let _ = write!(line, " – {}", md(&cut(&idea.note, 400)));
            }
            let _ = writeln!(out, "{line}");
            for (link, arrow, other) in links_of(idea.id, &map.links) {
                let mut l = format!("  - {arrow} {other} {}", link.kind.label());
                if let Some(label) = labels.get(&other) {
                    let _ = write!(l, " ({})", md(label));
                }
                if !link.note.trim().is_empty() {
                    let _ = write!(l, ": „{}“", md(&link.note));
                }
                let _ = writeln!(out, "{l}");
            }
        }
    };
    let work_ideas = ideas_at(map, "w");
    if !work_ideas.is_empty() {
        write_ideas(&mut out, None, &work_ideas);
        out.push('\n');
    }
    for (node, depth) in tree_order(map) {
        let hashes = "#".repeat((depth + 1).min(6));
        let title = if node.kind == MapNodeKind::NoChapter { node.title.clone() } else { node_label(node) };
        let _ = writeln!(out, "{hashes} {} ({})\n", md(&title), counts(node));
        if !node.sections.is_empty() {
            let list: Vec<String> = node
                .sections
                .iter()
                .map(|s| {
                    let name = if s.number.is_empty() { s.title.clone() } else { format!("{} {}", s.number, s.title) };
                    format!("{} ({})", md(&name), s.ideas)
                })
                .collect();
            let _ = writeln!(out, "Abschnitte: {}\n", list.join(", "));
        }
        let ideas = ideas_at(map, &node.id);
        if ideas.is_empty() && node.total_ideas > 0 {
            let _ = writeln!(out, "Keine eigenen Einfälle; sie hängen an den Punkten darunter.\n");
        } else if ideas.is_empty() {
            let _ = writeln!(out, "Keine Einfälle.\n");
        } else {
            write_ideas(&mut out, Some(node), &ideas);
            out.push('\n');
        }
    }
    let contradictions: Vec<&IdeaLink> =
        map.links.iter().filter(|l| l.deleted.is_none() && l.kind == LinkKind::Contradicts).collect();
    let _ = writeln!(out, "## Widersprüche\n");
    if contradictions.is_empty() {
        let _ = writeln!(out, "Keine.\n");
    }
    for link in contradictions {
        let label = |id: IdeaId| labels.get(&id).map(|l| format!(" ({})", md(l))).unwrap_or_default();
        let mut line = format!("- Einfall {}{} ↔ Einfall {}{}", link.from, label(link.from), link.to, label(link.to));
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
            let _ = writeln!(out, "- Einfall {} ~ Einfall {} ({:.2}, {})", pair.a, pair.b, pair.score, md(&via));
        }
    }
    out
}

/// Text für XML-Attribute: `& < > "` maskiert, Zeilenumbruch als `&#10;`, andere Steuerzeichen entfernt.
fn xml_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            // Steuerzeichen und die in XML 1.0 verbotenen Nichtzeichen U+FFFE/U+FFFF fallen weg.
            c if c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}') => {}
            c => out.push(c),
        }
    }
    out
}

/// Farbe einer Verbindungsart in FreeMind (dieselben hellen Farben wie die eigenständige SVG-Datei).
fn link_color(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::LeadsTo => "#3b5bdb",
        LinkKind::BelongsTo => "#2b8a3e",
        LinkKind::ExampleOf => "#9a5800",
        LinkKind::Contradicts => "#c92a2a",
    }
}

fn mm_id(node: &str) -> String {
    let id: String = node.chars().filter(char::is_ascii_alphanumeric).collect();
    format!("ID_{id}")
}

/// FreeMind 1.0.1: Werk als Wurzel, Knoten verschachtelt wie auf der Karte, Einfälle an ihrem Knoten (Gedanke als
/// Notiz), Verbindungen als `arrowlink` (Pfeil, bei „widerspricht“ beidseitig) und als Attribut „Verbindung“.
pub fn freemind(map: &IdeaMap) -> String {
    let mut out = String::from("<map version=\"1.0.1\">\n");
    let _ = writeln!(out, "<!-- Gedankenkarte, erzeugt von Baruch -->");
    let root = map.nodes.iter().find(|n| n.id == "w");
    let _ = writeln!(out, "<node ID=\"ID_w\" TEXT=\"{}\">", xml_attr(&map.title));
    let filter = filter_words(&map.filter);
    if !filter.is_empty() {
        let _ = writeln!(out, "<attribute NAME=\"Filter\" VALUE=\"{}\"/>", xml_attr(&filter));
    }
    if let Some(root) = root {
        let _ = writeln!(out, "<attribute NAME=\"Einfälle\" VALUE=\"{}\"/>", xml_attr(&counts(root)));
    }
    let shown: std::collections::HashSet<IdeaId> = map.ideas.iter().map(|i| i.id).collect();
    let write_idea = |out: &mut String, node: Option<&MapNode>, idea: &MapIdea| {
        let mut text = format!("Einfall {}: {} ({})", idea.id, idea_label(idea), status_label(idea.status));
        if let Some(section) = node.and_then(|n| section_of(n, idea)) {
            let _ = write!(text, ", {section}");
        }
        let _ = writeln!(out, "<node ID=\"{}\" TEXT=\"{}\">", mm_id(&idea.node), xml_attr(&text));
        for (link, arrow, other) in links_of(idea.id, &map.links) {
            let mut value = format!("{arrow} Einfall {other} {}", link.kind.label());
            if !link.note.trim().is_empty() {
                let _ = write!(value, ": {}", link.note.trim());
            }
            let _ = writeln!(out, "<attribute NAME=\"Verbindung\" VALUE=\"{}\"/>", xml_attr(&value));
        }
        for link in map.links.iter().filter(|l| l.deleted.is_none() && l.from == idea.id && shown.contains(&l.to)) {
            let start = if link.kind.symmetric() { "Default" } else { "None" };
            // Runde 18a: Farbe je Art wie in der Karte (FreeMind kennt für Pfeile keine Strichart; die Art steht
            // zusätzlich im Attribut „Verbindung“).
            let _ = writeln!(
                out,
                "<arrowlink DESTINATION=\"{}\" ENDARROW=\"Default\" STARTARROW=\"{start}\" COLOR=\"{}\"/>",
                mm_id(&format!("i:{}", link.to)),
                link_color(link.kind)
            );
        }
        if !idea.note.trim().is_empty() {
            let _ = writeln!(
                out,
                "<richcontent TYPE=\"NOTE\"><html><head></head><body><p>{}</p></body></html></richcontent>",
                xml_attr(idea.note.trim()).replace("&#10;", "<br/>")
            );
        }
        out.push_str("</node>\n");
    };
    for idea in ideas_at(map, "w") {
        write_idea(&mut out, None, idea);
    }
    fn walk(
        map: &IdeaMap,
        parent: &str,
        depth: usize,
        out: &mut String,
        seen: &mut Vec<String>,
        write_idea: &dyn Fn(&mut String, Option<&MapNode>, &MapIdea),
    ) {
        for (k, node) in map.nodes.iter().filter(|n| n.parent.as_deref() == Some(parent)).enumerate() {
            if seen.contains(&node.id) || depth > 16 {
                continue;
            }
            seen.push(node.id.clone());
            let title = if node.kind == MapNodeKind::NoChapter { node.title.clone() } else { node_label(node) };
            let position =
                if depth == 1 { if k % 2 == 0 { " POSITION=\"right\"" } else { " POSITION=\"left\"" } } else { "" };
            let _ = writeln!(
                out,
                "<node ID=\"{}\" TEXT=\"{}\"{position} FOLDED=\"false\">",
                mm_id(&node.id),
                xml_attr(&format!("{title} ({})", counts(node)))
            );
            for idea in map.ideas.iter().filter(|i| i.anchor == node.id) {
                write_idea(out, Some(node), idea);
            }
            walk(map, &node.id, depth + 1, out, seen, write_idea);
            out.push_str("</node>\n");
        }
    }
    let mut seen = Vec::new();
    walk(map, "w", 1, &mut out, &mut seen, &write_idea);
    out.push_str("</node>\n</map>\n");
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

    #[test]
    fn freemind_attributes_stay_well_formed() {
        assert_eq!(
            xml_attr("a \"b\" & <c>\nd\u{1}"),
            "a &quot;b&quot; &amp; &lt;c&gt;&#10;d",
            "maskiert, Zeilenumbruch als Zeichenreferenz"
        );
        assert_eq!(mm_id("i:12"), "ID_i12", "FreeMind-IDs nur aus Buchstaben und Ziffern");
        assert_eq!(xml_attr("x\u{fffe}y\u{ffff}z"), "xyz", "Nichtzeichen sind in XML 1.0 verboten und fallen weg");
    }
}
