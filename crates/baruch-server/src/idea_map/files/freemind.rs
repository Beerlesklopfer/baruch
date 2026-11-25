// SPDX-License-Identifier: AGPL-3.0-or-later
//! FreeMind/Freeplane (`.mm`) schreiben und lesen (Vertrag Runde 34 §8, Besitz mindmap_austausch).
//!
//! **Schreiben** ([`write`]): `<map version="1.0.1">`, je Knoten `<node TEXT ID>` (Hauptäste mit `POSITION`, erste
//! Hälfte rechts, zweite links; nie eingeklappt), Schlagwörter als `<attribute NAME="Schlagwort" …>`, die Notiz als
//! `<richcontent TYPE="NOTE">` (je Zeile ein Absatz), Beziehungen als `<arrowlink DESTINATION … MIDDLE_LABEL>`
//! (`MIDDLE_LABEL` ist ein Freeplane-Attribut; FreeMind übergeht es).
//!
//! **Lesen** ([`read`]): ein Durchgang über die XML-Ereignisse mit eigenem Stapel (keine Rekursion). Eine DOCTYPE
//! wird abgelehnt, Entitäten werden nicht aufgelöst (nur die fünf von XML, `nbsp` und Zeichenreferenzen; alles
//! andere bleibt als Text stehen). Knotenzahl und Tiefe werden beim Lesen begrenzt. HTML in Notizen wird reiner Text.

use std::collections::HashMap;
use std::fmt::Write as _;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use super::export::{CLOUD_ID, OutLink, OutNode, Sheet};
use super::import::{Raw, RawLink, RawNode, entity, too_deep, too_many_nodes};
use crate::idea_map::tree::{MAX_IMPORT_NODES, MAX_TREE_DEPTH};
use baruch_store::LinkKind;

/// Name des Attributs, das ein Schlagwort trägt (FreeMind kennt keine Etiketten).
pub const WORD_ATTRIBUTE: &str = "Schlagwort";

/// Text für XML-Attribute und -Inhalt: `& < > "` maskiert, Zeilenumbruch als `&#10;`, andere Steuerzeichen und die
/// in XML 1.0 verbotenen Nichtzeichen U+FFFE/U+FFFF entfernt.
pub fn xml_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            c if c.is_control() || matches!(c, '\u{fffe}' | '\u{ffff}') => {}
            c => out.push(c),
        }
    }
    out
}

/// FreeMind-Kennung zu einer Kennung des Blatts: `ID_` und nur Buchstaben und Ziffern (`i:12` → `ID_i12`).
pub fn mm_id(node: &str) -> String {
    let id: String = node.chars().filter(char::is_ascii_alphanumeric).collect();
    format!("ID_{id}")
}

/// Farbe einer Beziehung je Art (dieselben hellen Farben wie in der Mindmap).
fn link_color(kind: LinkKind) -> &'static str {
    match kind {
        LinkKind::LeadsTo => "#3b5bdb",
        LinkKind::BelongsTo => "#2b8a3e",
        LinkKind::ExampleOf => "#9a5800",
        LinkKind::Contradicts => "#c92a2a",
    }
}

fn write_node(
    out: &mut String,
    node: &OutNode,
    depth: usize,
    position: Option<&str>,
    links: &HashMap<&str, Vec<&OutLink>>,
) {
    let indent = " ".repeat(depth.min(40));
    let _ = write!(out, "{indent}<node ID=\"{}\" TEXT=\"{}\"", mm_id(&node.id), xml_text(&node.title));
    if let Some(position) = position {
        let _ = write!(out, " POSITION=\"{position}\"");
    }
    out.push_str(">\n");
    for word in &node.words {
        let _ = writeln!(out, "{indent} <attribute NAME=\"{WORD_ATTRIBUTE}\" VALUE=\"{}\"/>", xml_text(word));
    }
    for link in links.get(node.id.as_str()).into_iter().flatten() {
        let start = if link.kind.symmetric() { "Default" } else { "None" };
        let _ = writeln!(
            out,
            "{indent} <arrowlink DESTINATION=\"{}\" ENDARROW=\"Default\" STARTARROW=\"{start}\" MIDDLE_LABEL=\"{}\" COLOR=\"{}\"/>",
            mm_id(&link.to),
            xml_text(&link.label),
            link_color(link.kind)
        );
    }
    if !node.note.trim().is_empty() {
        let _ = write!(out, "{indent} <richcontent TYPE=\"NOTE\"><html><head></head><body>");
        for line in node.note.trim().lines() {
            let _ = write!(out, "<p>{}</p>", xml_text(line));
        }
        out.push_str("</body></html></richcontent>\n");
    }
    let half = node.children.len().div_ceil(2);
    for (index, child) in node.children.iter().enumerate() {
        let position = (depth == 0).then_some(if index < half { "right" } else { "left" });
        write_node(out, child, depth + 1, position, links);
    }
    let _ = writeln!(out, "{indent}</node>");
}

/// Das Blatt als FreeMind-Datei (siehe Modulkopf).
pub fn write(sheet: &Sheet) -> String {
    let mut links: HashMap<&str, Vec<&OutLink>> = HashMap::new();
    for link in &sheet.links {
        links.entry(link.from.as_str()).or_default().push(link);
    }
    let mut out = String::from("<map version=\"1.0.1\">\n<!-- Mindmap, erzeugt von Baruch -->\n");
    write_node(&mut out, &sheet.root, 0, None, &links);
    out.push_str("</map>\n");
    out
}

// ---- Lesen ---------------------------------------------------------------------------------------

fn broken(what: impl std::fmt::Display) -> String {
    format!("Die Datei ist kein lesbares FreeMind-/Freeplane-XML ({what}).")
}

/// Attribute eines Elements (Name klein → Wert, Entitäten nur die vordefinierten).
fn attributes(start: &BytesStart<'_>) -> Result<HashMap<String, String>, String> {
    let mut out = HashMap::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| broken(format!("Attribut: {e}")))?;
        let value = attr.normalized_value(XmlVersion::Implicit1_0).map_err(|_| {
            "Die Datei benutzt eigene XML-Entitäten; die löst Baruch nicht auf (fremde Eingabe).".to_owned()
        })?;
        out.insert(attr.key.as_ref().to_lowercase(), value.into_owned());
    }
    Ok(out)
}

/// Was ein offenes `<richcontent>` sammelt.
struct Rich {
    /// `true` = Notiz (`TYPE="NOTE"`), `false` = Knotentext (`TYPE="NODE"`); `None` = übergehen (z. B. DETAILS).
    note: Option<bool>,
    done: Vec<String>,
    line: String,
}

impl Rich {
    fn end_line(&mut self) {
        let line = self.line.split_whitespace().collect::<Vec<_>>().join(" ");
        self.done.push(line);
        self.line.clear();
    }

    fn text(mut self) -> String {
        self.end_line();
        let mut text = self.done.join("\n");
        while text.contains("\n\n\n") {
            text = text.replace("\n\n\n", "\n\n");
        }
        text.trim().to_owned()
    }
}

fn is_block(name: &str) -> bool {
    matches!(name, "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

/// FreeMind-/Freeplane-Datei lesen (siehe Modulkopf). `bytes` ohne BOM.
pub fn read(bytes: &[u8]) -> Result<Raw, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Die Datei ist kein UTF-8-Text; Baruch liest .mm-Dateien nur in UTF-8 oder ASCII.".to_owned())?;
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<RawNode> = Vec::new();
    let mut root: Option<RawNode> = None;
    let mut links = Vec::new();
    let mut warnings = Vec::new();
    let mut rich: Option<Rich> = None;
    let mut count = 0usize;
    let mut auto = 0usize;
    let mut extra_roots = 0usize;
    let mut saw_map = false;

    // Einen fertigen Knoten an seinen Elternknoten hängen bzw. zur Wurzel machen.
    fn close(node: RawNode, stack: &mut [RawNode], root: &mut Option<RawNode>, extra_roots: &mut usize) {
        match stack.last_mut() {
            Some(parent) => parent.children.push(node),
            None if root.is_none() => *root = Some(node),
            None => *extra_roots += 1,
        }
    }

    loop {
        let event = reader.read_event().map_err(broken)?;
        match event {
            Event::Eof => break,
            Event::DocType(_) => {
                return Err("Die Datei trägt eine DOCTYPE; die liest Baruch nicht (fremde Eingabe, keine Entitäten)."
                    .to_owned());
            }
            Event::Start(ref start) | Event::Empty(ref start) => {
                let empty = matches!(event, Event::Empty(_));
                let name = start.local_name().as_ref().to_lowercase();
                if let Some(open) = rich.as_mut() {
                    if name == "br" {
                        open.end_line();
                    }
                    continue;
                }
                match name.as_str() {
                    "map" => saw_map = true,
                    "node" => {
                        if !saw_map {
                            return Err(broken("das Element <map> fehlt"));
                        }
                        count += 1;
                        // Die Wurzel zählt nicht mit (sie wird nie Einfall).
                        if count > MAX_IMPORT_NODES + 1 {
                            return Err(too_many_nodes());
                        }
                        if stack.len() > MAX_TREE_DEPTH {
                            return Err(too_deep());
                        }
                        let attrs = attributes(start)?;
                        let id = attrs.get("id").cloned().filter(|i| !i.is_empty()).unwrap_or_else(|| {
                            auto += 1;
                            format!("\u{1}auto-{auto}")
                        });
                        let node = RawNode {
                            cloud: id == mm_id(CLOUD_ID),
                            id,
                            title: attrs.get("text").cloned().unwrap_or_default(),
                            ..RawNode::default()
                        };
                        if empty {
                            close(node, &mut stack, &mut root, &mut extra_roots);
                        } else {
                            stack.push(node);
                        }
                    }
                    "richcontent" if !empty => {
                        let kind = attributes(start)?.get("type").map(|t| t.to_uppercase());
                        let note = match kind.as_deref() {
                            Some("NOTE") => Some(true),
                            Some("NODE") => Some(false),
                            _ => None,
                        };
                        rich = Some(Rich { note, done: Vec::new(), line: String::new() });
                    }
                    "arrowlink" => {
                        let attrs = attributes(start)?;
                        if let (Some(from), Some(to)) = (stack.last(), attrs.get("destination")) {
                            let note = ["middle_label", "source_label", "target_label"]
                                .iter()
                                .filter_map(|name| attrs.get(*name))
                                .find(|label| !label.trim().is_empty())
                                .cloned()
                                .unwrap_or_default();
                            links.push(RawLink { from: from.id.clone(), to: to.clone(), note });
                        }
                    }
                    "attribute" => {
                        let attrs = attributes(start)?;
                        let is_word = attrs.get("name").is_some_and(|n| n.trim().eq_ignore_ascii_case(WORD_ATTRIBUTE));
                        if let (true, Some(node), Some(value)) = (is_word, stack.last_mut(), attrs.get("value")) {
                            node.words.push(value.clone());
                        }
                    }
                    _ => {}
                }
            }
            Event::End(end) => {
                let name = end.local_name().as_ref().to_lowercase();
                if name == "richcontent" {
                    if let Some(open) = rich.take() {
                        let kind = open.note;
                        let text = open.text();
                        match (kind, stack.last_mut()) {
                            (Some(true), Some(node)) => node.note = text,
                            (Some(false), Some(node)) if node.title.trim().is_empty() => {
                                node.title = text.replace('\n', " ");
                            }
                            _ => {}
                        }
                    }
                } else if let Some(open) = rich.as_mut() {
                    if is_block(&name) {
                        open.end_line();
                    }
                } else if name == "node" {
                    let node = stack.pop().ok_or_else(|| broken("ein </node> ohne offenen Knoten"))?;
                    close(node, &mut stack, &mut root, &mut extra_roots);
                }
            }
            Event::Text(text) => {
                if let Some(open) = rich.as_mut() {
                    open.line.push_str(&text.xml10_content());
                }
            }
            Event::CData(text) => {
                if let Some(open) = rich.as_mut() {
                    open.line.push_str(&text.xml10_content());
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(open) = rich.as_mut() {
                    let name: &str = &reference;
                    match entity(name) {
                        Some(c) => open.line.push(c),
                        // Unbekannte Entität: nicht aufgelöst, bleibt als Text stehen.
                        None => {
                            let _ = write!(open.line, "&{name};");
                        }
                    }
                }
            }
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) => {}
        }
    }
    if !stack.is_empty() || rich.is_some() {
        return Err(broken("die Datei endet mitten in einem Knoten"));
    }
    let root = root.ok_or_else(|| broken("kein Knoten gefunden"))?;
    if extra_roots > 0 {
        warnings.push(format!("{extra_roots} weitere(s) Hauptthema(-en) neben dem ersten wurden übergangen."));
    }
    Ok(Raw { root, links, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_stay_well_formed() {
        assert_eq!(
            xml_text("a \"b\" & <c>\nd\u{1}"),
            "a &quot;b&quot; &amp; &lt;c&gt;&#10;d",
            "maskiert, Zeilenumbruch als Zeichenreferenz, Steuerzeichen entfernt"
        );
        assert_eq!(mm_id("i:12"), "ID_i12", "FreeMind-IDs nur aus Buchstaben und Ziffern");
        assert_eq!(xml_text("x\u{fffe}y\u{ffff}z"), "xyz", "Nichtzeichen sind in XML 1.0 verboten und fallen weg");
    }

    #[test]
    fn a_foreign_file_with_rich_text_is_read_as_plain_text() {
        let file = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<map version=\"freeplane 1.9.13\">\n\
            <node TEXT=\"Hafen\" ID=\"ID_1\"><hook NAME=\"MapStyle\"><properties show=\"x\"/></hook>\n\
            <node ID=\"ID_2\" POSITION=\"right\"><richcontent TYPE=\"NODE\"><html><body><p>Mole &amp; <b>Kran</b></p></body></html></richcontent>\n\
            <richcontent TYPE=\"NOTE\">\n<html>\n  <head></head>\n  <body>\n    <p>\n      Erste&nbsp;Zeile\n    </p>\n    <p>Zweite <i>Zeile</i> &unbekannt;</p>\n  </body>\n</html>\n</richcontent>\n\
            <arrowlink DESTINATION=\"ID_3\" MIDDLE_LABEL=\"sieht\"/>\n</node>\n\
            <node TEXT=\"Leuchtturm\" ID=\"ID_3\" FOLDED=\"true\"/>\n</node>\n</map>\n";
        let raw = read(file.as_bytes()).expect("eine gewöhnliche Freeplane-Datei wird gelesen");
        assert_eq!(raw.root.title, "Hafen", "Wurzel");
        assert_eq!(raw.root.children.len(), 2, "zwei Kinder; fremde Elemente (hook) stören nicht");
        assert_eq!(raw.root.children[0].title, "Mole & Kran", "Knotentext aus richcontent NODE als reiner Text");
        assert_eq!(
            raw.root.children[0].note, "Erste Zeile\nZweite Zeile &unbekannt;",
            "Notiz als reiner Text: Absätze werden Zeilen, eine unbekannte Entität wird nicht aufgelöst"
        );
        assert_eq!(
            raw.links,
            vec![RawLink { from: "ID_2".to_owned(), to: "ID_3".to_owned(), note: "sieht".to_owned() }],
            "arrowlink mit Beschriftung"
        );
    }

    #[test]
    fn doctype_and_own_entities_are_refused() {
        let bomb = "<?xml version=\"1.0\"?>\n<!DOCTYPE map [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;\">]>\n\
            <map version=\"1.0.1\"><node TEXT=\"&b;\"/></map>";
        let error = read(bomb.as_bytes()).unwrap_err();
        assert!(error.contains("DOCTYPE"), "eine DOCTYPE wird abgelehnt, bevor irgendeine Entität zählt: {error}");
        let own = "<map version=\"1.0.1\"><node TEXT=\"&geheim;\"/></map>";
        let error = read(own.as_bytes()).unwrap_err();
        assert!(error.contains("Entitäten"), "eigene Entitäten in Attributen werden nicht aufgelöst: {error}");
    }

    #[test]
    fn broken_xml_too_many_nodes_and_too_deep_trees_are_refused() {
        assert!(read(b"<map version=\"1.0.1\"><node TEXT=\"a\">").unwrap_err().contains("mitten in einem Knoten"));
        assert!(read(b"<map><node TEXT=\"a\"></map>").is_err(), "falsch geschachtelt → Fehler, kein Absturz");
        assert!(read(b"<html><body/></html>").unwrap_err().contains("kein Knoten"), "XML ohne Mindmap");
        let mut wide = String::from("<map version=\"1.0.1\"><node TEXT=\"w\">");
        for i in 0..=MAX_IMPORT_NODES {
            let _ = write!(wide, "<node TEXT=\"k{i}\"/>");
        }
        wide.push_str("</node></map>");
        assert_eq!(read(wide.as_bytes()).unwrap_err(), too_many_nodes(), "Knotenzahl schon beim Lesen begrenzt");
        let levels = MAX_TREE_DEPTH + 2;
        let deep =
            format!("<map version=\"1.0.1\">{}{}</map>", "<node TEXT=\"t\">".repeat(levels), "</node>".repeat(levels));
        assert_eq!(
            read(deep.as_bytes()).unwrap_err(),
            too_deep(),
            "Tiefe schon beim Lesen begrenzt (Wurzel + 32 Ebenen)"
        );
        let levels = MAX_TREE_DEPTH + 1;
        let fine =
            format!("<map version=\"1.0.1\">{}{}</map>", "<node TEXT=\"t\">".repeat(levels), "</node>".repeat(levels));
        assert!(read(fine.as_bytes()).is_ok(), "Wurzel und genau 32 Ebenen darunter sind erlaubt");
    }
}
