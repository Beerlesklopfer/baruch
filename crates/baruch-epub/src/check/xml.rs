// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kleiner XML-Baum für die Prüfung (Besitz „epub-pruefung“, Runde 20c): [`parse`] liest ein Dokument mit
//! quick-xml und prüft dabei die Wohlgeformtheit, soweit die Prüfung sie braucht – genau ein Wurzelelement,
//! jedes Element geschlossen (auch am Dateiende), passende End-Tags, gültige Attribute ohne Doppelte, nur die fünf
//! vordefinierten Entitäten und Zeichenreferenzen (XHTML im EPUB hat keine DTD, `&nbsp;` ist darum nicht
//! wohlgeformt), kein Text außerhalb der Wurzel.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

/// Ein Knoten: Element oder Text.
#[derive(Debug, Clone)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

/// Ein Element mit qualifiziertem Namen (`epub:type`, `dc:title`) und Attributen in Dokumentreihenfolge.
#[derive(Debug, Clone, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    /// Wert des Attributs mit genau diesem (qualifizierten) Namen.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Name ohne Präfix (`dc:title` → `title`).
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    /// Alle Nachfahren (ohne sich selbst) in Dokumentreihenfolge.
    pub fn descendants(&self) -> Vec<&Element> {
        let mut out = Vec::new();
        fn walk<'a>(e: &'a Element, out: &mut Vec<&'a Element>) {
            for child in &e.children {
                if let Node::Element(c) = child {
                    out.push(c);
                    walk(c, out);
                }
            }
        }
        walk(self, &mut out);
        out
    }

    /// Direkte Kind-Elemente.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// Erstes Kind-Element mit diesem Namen ohne Präfix.
    pub fn child(&self, local: &str) -> Option<&Element> {
        self.elements().find(|e| e.local() == local)
    }

    /// Der ganze Text darin (Nachfahren eingeschlossen).
    pub fn text(&self) -> String {
        let mut out = String::new();
        fn walk(e: &Element, out: &mut String) {
            for child in &e.children {
                match child {
                    Node::Text(t) => out.push_str(t),
                    Node::Element(c) => walk(c, out),
                }
            }
        }
        walk(self, &mut out);
        out
    }
}

/// Zeile (ab 1) zur Bytestelle `pos`.
fn line_at(text: &str, pos: usize) -> usize {
    let pos = pos.min(text.len());
    text.as_bytes()[..pos].iter().filter(|&&b| b == b'\n').count() + 1
}

fn element_of(start: &BytesStart<'_>) -> Result<Element, String> {
    let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    let mut attrs = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| format!("Attribut in <{name}>: {e}"))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr.unescape_value().map_err(|e| format!("Attribut {key} in <{name}>: {e}"))?.into_owned();
        attrs.push((key, value));
    }
    Ok(Element { name, attrs, children: Vec::new() })
}

/// Liest `text` als XML. `Err` = nicht wohlgeformt (deutsche Meldung mit Zeile).
pub(crate) fn parse(text: &str) -> Result<Element, String> {
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let at = |reader: &Reader<&[u8]>| line_at(text, usize::try_from(reader.buffer_position()).unwrap_or(usize::MAX));
    loop {
        let event = match reader.read_event() {
            Ok(event) => event,
            Err(e) => return Err(format!("Zeile {}: {e}", line_at(text, reader.error_position() as usize))),
        };
        match event {
            Event::Start(start) => {
                if root.is_some() {
                    return Err(format!("Zeile {}: zweites Wurzelelement", at(&reader)));
                }
                stack.push(element_of(&start).map_err(|e| format!("Zeile {}: {e}", at(&reader)))?);
            }
            Event::Empty(start) => {
                let element = element_of(&start).map_err(|e| format!("Zeile {}: {e}", at(&reader)))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(element)),
                    None if root.is_none() => root = Some(element),
                    None => return Err(format!("Zeile {}: zweites Wurzelelement", at(&reader))),
                }
            }
            Event::End(_) => {
                // quick-xml prüft, dass der Name zum offenen Element passt.
                let Some(element) = stack.pop() else {
                    return Err(format!("Zeile {}: End-Tag ohne offenes Element", at(&reader)));
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(element)),
                    None => root = Some(element),
                }
            }
            Event::Text(t) => {
                let decoded = t.decode().map_err(|e| format!("Zeile {}: {e}", at(&reader)))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Text(decoded.into_owned())),
                    None if decoded.trim().is_empty() => {}
                    None => return Err(format!("Zeile {}: Text außerhalb des Wurzelelements", at(&reader))),
                }
            }
            Event::CData(c) => {
                let decoded = c.decode().map_err(|e| format!("Zeile {}: {e}", at(&reader)))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Text(decoded.into_owned())),
                    None => return Err(format!("Zeile {}: CDATA außerhalb des Wurzelelements", at(&reader))),
                }
            }
            Event::GeneralRef(r) => {
                let resolved = if r.is_char_ref() {
                    match r.resolve_char_ref() {
                        Ok(Some(c)) => c.to_string(),
                        _ => return Err(format!("Zeile {}: ungültige Zeichenreferenz", at(&reader))),
                    }
                } else {
                    let name = r.decode().map_err(|e| format!("Zeile {}: {e}", at(&reader)))?;
                    match name.as_ref() {
                        "amp" => "&".to_owned(),
                        "lt" => "<".to_owned(),
                        "gt" => ">".to_owned(),
                        "quot" => "\"".to_owned(),
                        "apos" => "'".to_owned(),
                        other => {
                            return Err(format!(
                                "Zeile {}: unbekannte Entität &{other}; (ohne DTD gelten nur &amp; &lt; &gt; &quot; \
                                 &apos; und Zeichenreferenzen)",
                                at(&reader)
                            ));
                        }
                    }
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Text(resolved)),
                    None => return Err(format!("Zeile {}: Entität außerhalb des Wurzelelements", at(&reader))),
                }
            }
            Event::Decl(_) | Event::PI(_) | Event::Comment(_) | Event::DocType(_) => {}
            Event::Eof => break,
        }
    }
    if let Some(open) = stack.last() {
        return Err(format!("Dateiende: <{}> ist nicht geschlossen", open.name));
    }
    root.ok_or_else(|| "kein Wurzelelement".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_document_becomes_a_tree_with_text_and_attributes() {
        let root = parse("<?xml version=\"1.0\"?><a x=\"1 &amp; 2\"><b>Hallo &#228;</b><c/></a>").expect("wohlgeformt");
        assert_eq!(root.attr("x"), Some("1 & 2"), "Attributwert entschlüsselt");
        assert_eq!(root.text(), "Hallo ä", "Text samt Zeichenreferenz");
        assert_eq!(root.descendants().len(), 2, "b und c sind Nachfahren");
    }

    #[test]
    fn broken_documents_are_reported_with_a_reason() {
        assert!(parse("<a><b></a>").is_err(), "falsches End-Tag");
        assert!(parse("<a><b>").unwrap_err().contains("nicht geschlossen"), "offenes Element am Dateiende");
        assert!(parse("<a>&nbsp;</a>").unwrap_err().contains("nbsp"), "HTML-Entität ohne DTD");
        assert!(parse("<a/><b/>").unwrap_err().contains("zweites"), "zwei Wurzeln");
        assert!(parse("<a x=\"1\" x=\"2\"/>").is_err(), "doppeltes Attribut");
        assert!(parse("").unwrap_err().contains("kein Wurzelelement"), "leere Datei");
    }
}
