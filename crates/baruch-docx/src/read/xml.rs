// SPDX-License-Identifier: AGPL-3.0-or-later
//! Small XML tree for the parts of a Word file (quick-xml, without namespace resolution).
//!
//! Elements and attributes are named as they are in the file (`w:p`, `w:val`). Assumption: the usual prefixes
//! (`w`, `r`, `wp`, `a`, `pic`, `m`, `mc`, `v`, `w14`, `w15`) – Word and LibreOffice write them that way.
//!
//! Security: a DTD (`<!DOCTYPE …>`) is rejected ([`DocxError::Unsafe`]); entities are never resolved from the
//! file – only the five fixed ones (`&amp;` …) and character references (`&#x41;`), every other one stays
//! verbatim text. Depth and number of the elements are limited ([`Limits`]).

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{DocxError, Limits};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Child elements (without text).
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// First child with this name.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == name)
    }

    /// `w:val` of the child `name`.
    pub fn child_val(&self, name: &str) -> Option<&str> {
        self.child(name).and_then(|e| e.attr("w:val"))
    }

    /// First descendant (depth-first search) with this name.
    pub fn find(&self, name: &str) -> Option<&Element> {
        for e in self.elements() {
            if e.name == name {
                return Some(e);
            }
            if let Some(found) = e.find(name) {
                return Some(found);
            }
        }
        None
    }

    /// Text of all descendants with the name `leaf` (e.g. `w:t`), concatenated.
    pub fn text_of(&self, leaf: &str, out: &mut String) {
        for e in self.elements() {
            if e.name == leaf {
                out.push_str(&e.text());
            } else {
                e.text_of(leaf, out);
            }
        }
    }

    /// Immediate text of this element.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for n in &self.children {
            if let Node::Text(t) = n {
                out.push_str(t);
            }
        }
        out
    }
}

fn predefined(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => return None,
    })
}

fn element(start: &BytesStart<'_>) -> Element {
    let name = start.name().into_inner().to_owned();
    let mut attrs = Vec::new();
    for attr in start.attributes().with_checks(false).flatten() {
        let key = attr.key.into_inner().to_owned();
        let raw = attr.value.to_string();
        let value = match quick_xml::escape::unescape_with(&raw, |_| None) {
            Ok(v) => v.into_owned(),
            Err(_) => raw,
        };
        attrs.push((key, value));
    }
    Element { name, attrs, children: Vec::new() }
}

fn push_text(stack: &mut [Element], text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(top) = stack.last_mut() {
        if let Some(Node::Text(last)) = top.children.last_mut() {
            last.push_str(text);
        } else {
            top.children.push(Node::Text(text.to_owned()));
        }
    }
}

/// Read one part; the result is the root element.
pub(crate) fn parse(part: &str, bytes: &[u8], limits: &Limits) -> Result<Element, DocxError> {
    let xml = |message: String| DocxError::Xml { part: part.to_owned(), message };
    let text = std::str::from_utf8(bytes).map_err(|_| xml("kein UTF-8 (UTF-16 liest Baruch nicht)".into()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Element> = vec![Element::default()];
    let mut elements = 0usize;
    loop {
        let event =
            reader.read_event().map_err(|e| xml(format!("nicht lesbar an Stelle {}: {e}", reader.error_position())))?;
        match event {
            Event::Start(start) => {
                elements += 1;
                if stack.len() > limits.max_depth {
                    return Err(DocxError::Limit(format!(
                        "{part} ist tiefer als {} Ebenen geschachtelt",
                        limits.max_depth
                    )));
                }
                stack.push(element(&start));
            }
            Event::Empty(start) => {
                elements += 1;
                let e = element(&start);
                if let Some(top) = stack.last_mut() {
                    top.children.push(Node::Element(e));
                }
            }
            Event::End(_) => {
                if stack.len() > 1 {
                    let done = stack.pop().expect("Stapel");
                    if let Some(top) = stack.last_mut() {
                        top.children.push(Node::Element(done));
                    }
                }
            }
            Event::Text(t) => push_text(&mut stack, &t.xml10_content()),
            Event::CData(c) => push_text(&mut stack, &c.into_inner()),
            Event::GeneralRef(r) => {
                let text = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => {
                        let name: &str = &r;
                        predefined(name).map_or_else(|| format!("&{name};"), str::to_owned)
                    }
                };
                push_text(&mut stack, &text);
            }
            Event::DocType(_) => {
                return Err(DocxError::Unsafe(format!(
                    "{part} enthält eine DTD (<!DOCTYPE …>); Word-Dateien haben keine, Baruch wertet sie nie aus"
                )));
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) => {}
        }
        if elements > limits.max_elements {
            return Err(DocxError::Limit(format!("{part} hat mehr als {} Elemente", limits.max_elements)));
        }
    }
    if stack.len() != 1 {
        return Err(xml("Element nicht geschlossen".into()));
    }
    let root = stack.pop().expect("Wurzel");
    root.children
        .into_iter()
        .find_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
        .ok_or_else(|| xml("kein Wurzelelement".into()))
}
