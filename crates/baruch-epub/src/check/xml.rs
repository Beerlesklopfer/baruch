// SPDX-License-Identifier: AGPL-3.0-or-later
//! Small XML tree for the check (ownership "epub-pruefung", Round 20c): [`parse`] reads a document with quick-xml
//! and checks well-formedness along the way, as far as the check needs it – exactly one root element, every
//! element closed (also at the end of the file), matching end tags, valid attributes without duplicates, only the
//! five predefined entities and character references (XHTML in the EPUB has no DTD, so `&nbsp;` is not
//! well-formed), no text outside the root.

use quick_xml::Reader;
use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};

/// A node: element or text.
#[derive(Debug, Clone)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

/// An element with qualified name (`epub:type`, `dc:title`) and attributes in document order.
#[derive(Debug, Clone, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    /// Value of the attribute with exactly this (qualified) name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Name without prefix (`dc:title` → `title`).
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }

    /// All descendants (without itself) in document order.
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

    /// Direct child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// First child element with this name without prefix.
    pub fn child(&self, local: &str) -> Option<&Element> {
        self.elements().find(|e| e.local() == local)
    }

    /// The whole text inside (descendants included).
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

/// Line (from 1) for the byte position `pos`.
fn line_at(text: &str, pos: usize) -> usize {
    let pos = pos.min(text.len());
    text.as_bytes()[..pos].iter().filter(|&&b| b == b'\n').count() + 1
}

fn element_of(start: &BytesStart<'_>) -> Result<Element, String> {
    let name = start.name().into_inner().to_owned();
    let mut attrs = Vec::new();
    for attr in start.attributes() {
        let attr = attr.map_err(|e| format!("Attribut in <{name}>: {e}"))?;
        let key = attr.key.into_inner().to_owned();
        let value = unescape(&attr.value).map_err(|e| format!("Attribut {key} in <{name}>: {e}"))?.into_owned();
        attrs.push((key, value));
    }
    Ok(Element { name, attrs, children: Vec::new() })
}

/// Reads `text` as XML. `Err` = not well-formed (German message with line).
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
                // quick-xml checks that the name matches the open element.
                let Some(element) = stack.pop() else {
                    return Err(format!("Zeile {}: End-Tag ohne offenes Element", at(&reader)));
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(element)),
                    None => root = Some(element),
                }
            }
            Event::Text(t) => {
                let decoded = t.into_inner();
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Text(decoded.into_owned())),
                    None if decoded.trim().is_empty() => {}
                    None => return Err(format!("Zeile {}: Text außerhalb des Wurzelelements", at(&reader))),
                }
            }
            Event::CData(c) => {
                let decoded = c.into_inner();
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
                    match &*r {
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
