// SPDX-License-Identifier: AGPL-3.0-or-later
//! Small XML tree on top of quick-xml (Round 21a) for EPUB (OPF, NCX, nav, XHTML) and the output of
//! `pdftotext -bbox-layout`. Lenient like a reader: end tags that do not match close up to the matching open
//! element (or are skipped), HTML entities such as `&nbsp;` are resolved, unknown ones stay as text.
//! Limits: at most [`MAX_DEPTH`] levels and [`MAX_NODES`] nodes (otherwise an error instead of a stack overflow).

use quick_xml::Reader;
use quick_xml::escape::unescape_with;
use quick_xml::events::{BytesStart, Event};

/// Largest nesting depth.
pub const MAX_DEPTH: usize = 256;
/// At most this many nodes per document.
pub const MAX_NODES: usize = 2_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Element {
    /// Local name, lower case (without prefix).
    pub name: String,
    /// Attributes: full name in lower case (with prefix, e.g. `epub:type`), value decoded.
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    /// Value of an attribute (full name, lower case).
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Value of an attribute by local name (without prefix), e.g. `type` for `epub:type`, only with a prefix.
    pub fn prefixed_attr(&self, local: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k.rsplit_once(':').is_some_and(|(_, l)| l == local)).map(|(_, v)| v.as_str())
    }

    /// Child elements.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            Node::Text(_) => None,
        })
    }

    /// First child element with this name.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == name)
    }

    /// First element of this name in the depth (the element itself included).
    pub fn find(&self, name: &str) -> Option<&Element> {
        if self.name == name {
            return Some(self);
        }
        self.elements().find_map(|e| e.find(name))
    }

    /// All elements of this name in the depth, in document order.
    pub fn find_all<'a>(&'a self, name: &str, out: &mut Vec<&'a Element>) {
        if self.name == name {
            out.push(self);
        }
        for e in self.elements() {
            e.find_all(name, out);
        }
    }

    /// Whole text (without markup), whitespace collapsed.
    pub fn text(&self) -> String {
        let mut out = String::new();
        collect_text(self, &mut out);
        // Collapse only ASCII whitespace: a non-breaking space (&nbsp;) stays what it is.
        out.split_ascii_whitespace().collect::<Vec<_>>().join(" ")
    }
}

fn collect_text(e: &Element, out: &mut String) {
    for node in &e.children {
        match node {
            Node::Text(t) => out.push_str(t),
            Node::Element(c) => {
                collect_text(c, out);
                if matches!(c.name.as_str(), "p" | "div" | "br" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
                    out.push(' ');
                }
            }
        }
    }
}

/// HTML entities that occur in XHTML without a DTD (XML knows only five).
fn html_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        "shy" => "\u{ad}",
        "ndash" => "–",
        "mdash" => "—",
        "hellip" => "…",
        "laquo" => "«",
        "raquo" => "»",
        "lsaquo" => "‹",
        "rsaquo" => "›",
        "bdquo" => "„",
        "ldquo" => "“",
        "rdquo" => "”",
        "sbquo" => "‚",
        "lsquo" => "‘",
        "rsquo" => "’",
        "copy" => "©",
        "reg" => "®",
        "trade" => "™",
        "sect" => "§",
        "para" => "¶",
        "deg" => "°",
        "middot" => "·",
        "bull" => "•",
        "times" => "×",
        "euro" => "€",
        "auml" => "ä",
        "ouml" => "ö",
        "uuml" => "ü",
        "Auml" => "Ä",
        "Ouml" => "Ö",
        "Uuml" => "Ü",
        "szlig" => "ß",
        "thinsp" => "\u{2009}",
        "ensp" => "\u{2002}",
        "emsp" => "\u{2003}",
        "zwnj" => "\u{200c}",
        "zwj" => "\u{200d}",
        _ => return None,
    })
}

fn element(start: &BytesStart<'_>) -> Element {
    let name = start.local_name().into_inner().to_lowercase();
    let mut attrs = Vec::new();
    for attr in start.attributes().with_checks(false).flatten() {
        let key = attr.key.into_inner().to_lowercase();
        let value = unescape_with(&attr.value, html_entity).map_or_else(|_| attr.value.to_string(), |v| v.into_owned());
        attrs.push((key, value));
    }
    Element { name, attrs, children: Vec::new() }
}

/// Read a document; the root is an artificial element `#document` with all nodes of the top level.
pub fn parse(text: &str) -> Result<Element, String> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = false;
    let mut stack: Vec<Element> = vec![Element { name: "#document".into(), ..Element::default() }];
    let mut nodes = 0usize;
    loop {
        let event =
            reader.read_event().map_err(|e| format!("XML nicht lesbar an Stelle {}: {e}", reader.error_position()))?;
        nodes += 1;
        if nodes > MAX_NODES {
            return Err(format!("XML hat mehr als {MAX_NODES} Knoten."));
        }
        match event {
            Event::Start(start) => {
                if stack.len() > MAX_DEPTH {
                    return Err(format!("XML ist tiefer als {MAX_DEPTH} Ebenen geschachtelt."));
                }
                stack.push(element(&start));
            }
            Event::Empty(start) => {
                let e = element(&start);
                if let Some(top) = stack.last_mut() {
                    top.children.push(Node::Element(e));
                }
            }
            Event::End(end) => {
                let name = end.local_name().into_inner().to_lowercase();
                if let Some(pos) = stack.iter().rposition(|e| e.name == name)
                    && pos > 0
                {
                    while stack.len() > pos {
                        let done = stack.pop().expect("Stapel");
                        if let Some(top) = stack.last_mut() {
                            top.children.push(Node::Element(done));
                        }
                    }
                }
            }
            Event::Text(t) => {
                let text = t.xml10_content().into_owned();
                push_text(&mut stack, &text);
            }
            Event::CData(c) => {
                let text = c.into_inner().into_owned();
                push_text(&mut stack, &text);
            }
            Event::GeneralRef(r) => {
                let text = match r.resolve_char_ref() {
                    Ok(Some(c)) => c.to_string(),
                    _ => {
                        let name: &str = &r;
                        html_entity(name).map_or_else(|| format!("&{name};"), str::to_owned)
                    }
                };
                push_text(&mut stack, &text);
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    while stack.len() > 1 {
        let done = stack.pop().expect("Stapel");
        if let Some(top) = stack.last_mut() {
            top.children.push(Node::Element(done));
        }
    }
    Ok(stack.pop().expect("Wurzel"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_namespaces_and_unbalanced_tags_are_read_leniently() {
        let doc = parse(
            "<?xml version=\"1.0\"?><html xmlns:epub=\"x\"><body><p epub:type=\"note\">A&nbsp;&amp;&#x42;&unbekannt;<b>fett</p><br/></body></html>",
        )
        .unwrap();
        let p = doc.find("p").expect("Absatz gefunden");
        assert_eq!(p.prefixed_attr("type"), Some("note"), "epub:type über den lokalen Namen");
        assert_eq!(p.text(), "A\u{a0}&B&unbekannt;fett", "Entitäten aufgelöst, unbekannte bleiben wörtlich");
        assert!(doc.find("br").is_some(), "nach dem unpassenden </p> geht es weiter");
        assert!(parse(&"<a>".repeat(MAX_DEPTH + 5)).is_err(), "zu tief geschachtelt: Fehler statt Stapelüberlauf");
    }
}
