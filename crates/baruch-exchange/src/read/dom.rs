// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sicheres Einlesen von XML in einen kleinen Baum (Vertrag R26 F2/F7).
//!
//! - Keine DTD (`Event::DocType` → [`ExchangeError::Unsafe`]), keine Verarbeitungsanweisung außer der
//!   XML-Deklaration, nur die fünf vordefinierten Entitäten (`lt`, `gt`, `amp`, `apos`, `quot`) und
//!   Zeichenreferenzen; jede andere Entitätsreferenz → `Unsafe`. Ohne DTD gibt es keine inneren oder externen
//!   Entitäten, darum greifen weder „Billion Laughs“ noch externe Dateien.
//! - Grenzen aus [`Limits`]: Tiefe, Zahl der Elemente, Attribute je Element, Bytes je Text bzw. Attributwert.
//!   Die Größe des ganzen XML prüft der Aufrufer vorher.
//! - Text exakt: nie getrimmt; nur die Zeilenenden normalisiert, wie XML 1.0 es verlangt (`\r\n` und ein
//!   einzelnes `\r` → `\n`; ein echtes `\r` kommt als `&#13;`). In Attributwerten werden Tab, Zeilenumbruch und
//!   Wagenrücklauf wie in XML 1.0 zu Leerzeichen; als Zeichenreferenz bleiben sie erhalten.
//! - In XML 1.0 verbotene Zeichen (Steuerzeichen außer Tab, LF, CR; U+FFFE, U+FFFF) → [`ExchangeError::Xml`],
//!   auch als Zeichenreferenz.

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::ResolveResult;

use crate::{ExchangeError, Limits};

/// Ein Element mit lokalem Namen, Namensraum, Attributen, Kindern und Text (verkettet, exakt).
#[derive(Debug, Clone, Default)]
pub(crate) struct Element {
    pub name: String,
    /// Aufgelöster Namensraum (`None` = keiner).
    pub ns: Option<String>,
    pub attrs: Vec<Attr>,
    pub children: Vec<Element>,
    pub text: String,
}

/// Ein Attribut: lokaler Name, Namensraum (`None` = ohne Präfix, also eines des Formats), Wert.
#[derive(Debug, Clone)]
pub(crate) struct Attr {
    pub name: String,
    pub ns: Option<String>,
    pub value: String,
}

/// Darf das Zeichen in einem XML-1.0-Dokument stehen?
pub(crate) fn legal_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

fn xml_error(e: impl std::fmt::Display) -> ExchangeError {
    ExchangeError::Xml(e.to_string())
}

fn check_legal(text: &str, where_: &str) -> Result<(), ExchangeError> {
    match text.chars().find(|&c| !legal_xml_char(c)) {
        Some(c) => Err(ExchangeError::Xml(format!(
            "verbotenes Zeichen U+{:04X} in {where_} (in XML 1.0 nicht erlaubt)",
            u32::from(c)
        ))),
        None => Ok(()),
    }
}

/// Zeilenenden wie XML 1.0: `\r\n` und `\r` → `\n`.
fn normalize_eol(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Ersatz für eine Entitätsreferenz (Name ohne `&` und `;`): vordefiniert oder Zeichenreferenz.
fn resolve_reference(name: &str) -> Result<char, ExchangeError> {
    match name {
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "amp" => Ok('&'),
        "apos" => Ok('\''),
        "quot" => Ok('"'),
        _ => {
            let Some(number) = name.strip_prefix('#') else {
                return Err(ExchangeError::Unsafe(format!(
                    "unbekannte Entität „&{name};“ (erlaubt sind nur &lt; &gt; &amp; &apos; &quot; und Zeichenreferenzen)"
                )));
            };
            let value = match number.strip_prefix('x') {
                Some(hex) if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                    u32::from_str_radix(hex, 16).ok()
                }
                None if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) => number.parse().ok(),
                _ => None,
            };
            let c = value
                .and_then(char::from_u32)
                .ok_or_else(|| ExchangeError::Xml(format!("ungültige Zeichenreferenz „&{name};“")))?;
            if !legal_xml_char(c) {
                return Err(ExchangeError::Xml(format!(
                    "Zeichenreferenz „&{name};“ nennt ein in XML 1.0 verbotenes Zeichen"
                )));
            }
            Ok(c)
        }
    }
}

/// Attributwert wie XML 1.0: Zeilenenden normalisiert, Tab/LF/CR → Leerzeichen, dann Referenzen aufgelöst.
fn attribute_value(text: &str, name: &str, limits: &Limits) -> Result<String, ExchangeError> {
    if text.len() > limits.max_text_bytes {
        return Err(ExchangeError::Limit(format!("Attribut „{name}“ ist länger als {} Bytes", limits.max_text_bytes)));
    }
    if text.contains('<') {
        return Err(ExchangeError::Xml(format!("„<“ im Attribut „{name}“")));
    }
    let normalized = text.replace("\r\n", " ").replace(['\r', '\n', '\t'], " ");
    let mut out = String::with_capacity(normalized.len());
    let mut rest = normalized.as_str();
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let end = after.find(';').ok_or_else(|| ExchangeError::Xml(format!("„&“ ohne „;“ im Attribut „{name}“")))?;
        out.push(resolve_reference(&after[..end])?);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    check_legal(&out, &format!("Attribut „{name}“"))?;
    Ok(out)
}

/// Aufgelöster Namensraum eines Elements, ohne Ausleihe des Lesers.
enum OwnedNs {
    Bound(String),
    Unbound,
    Unknown(String),
}

impl OwnedNs {
    fn of(resolved: ResolveResult<'_>) -> OwnedNs {
        match resolved {
            ResolveResult::Bound(ns) => OwnedNs::Bound(ns.0.to_owned()),
            ResolveResult::Unbound => OwnedNs::Unbound,
            ResolveResult::Unknown(prefix) => OwnedNs::Unknown(prefix),
        }
    }
}

/// Startelement in ein [`Element`] (ohne Kinder) übersetzen; prüft Attribute und ihre Grenze.
fn element_of<R>(
    reader: &NsReader<R>,
    start: &BytesStart<'_>,
    resolved: OwnedNs,
    limits: &Limits,
) -> Result<Element, ExchangeError> {
    let name = start.local_name().into_inner().to_owned();
    let ns = match resolved {
        OwnedNs::Bound(ns) => Some(ns),
        OwnedNs::Unbound => None,
        OwnedNs::Unknown(prefix) => {
            return Err(ExchangeError::Xml(format!("unbekanntes Präfix „{prefix}“ am Element „{name}“")));
        }
    };
    let mut attrs = Vec::new();
    let mut count = 0usize;
    for attr in start.attributes().with_checks(true) {
        let attr = attr.map_err(xml_error)?;
        count += 1;
        if count > limits.max_attributes {
            return Err(ExchangeError::Limit(format!(
                "Element „{name}“ hat mehr als {} Attribute",
                limits.max_attributes
            )));
        }
        let key = attr.key;
        // Namensraum-Deklarationen zählen zur Grenze, sind aber keine Angaben des Formats.
        if key.as_namespace_binding().is_some() {
            continue;
        }
        let (resolved, local) = reader.resolver().resolve_attribute(key);
        let local = local.into_inner().to_owned();
        let ns = match resolved {
            ResolveResult::Bound(ns) => Some(ns.0.to_owned()),
            ResolveResult::Unbound => None,
            ResolveResult::Unknown(prefix) => {
                return Err(ExchangeError::Xml(format!("unbekanntes Präfix „{prefix}“ am Attribut „{local}“")));
            }
        };
        let value = attribute_value(&attr.value, &local, limits)?;
        attrs.push(Attr { name: local, ns, value });
    }
    Ok(Element { name, ns, attrs, children: Vec::new(), text: String::new() })
}

/// Text an das offene Element hängen (Grenze je Text) oder, außerhalb der Wurzel, nur Leerraum zulassen.
fn push_text(stack: &mut [Element], text: &str, limits: &Limits) -> Result<(), ExchangeError> {
    match stack.last_mut() {
        Some(element) => {
            if element.text.len() + text.len() > limits.max_text_bytes {
                return Err(ExchangeError::Limit(format!(
                    "Text in „{}“ ist länger als {} Bytes",
                    element.name, limits.max_text_bytes
                )));
            }
            element.text.push_str(text);
            Ok(())
        }
        None if text.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) => Ok(()),
        None => Err(ExchangeError::Xml("Text außerhalb des Wurzelelements".into())),
    }
}

/// XML lesen und als Baum zurückgeben (Wurzelelement).
pub(crate) fn parse(xml: &[u8], limits: &Limits) -> Result<Element, ExchangeError> {
    let mut reader = NsReader::from_reader(xml);
    {
        let config = reader.config_mut();
        config.trim_text(false);
        config.check_end_names = true;
        config.expand_empty_elements = false;
        config.check_comments = true;
        config.allow_dangling_amp = false;
    }
    let mut buf = Vec::new();
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let mut elements = 0usize;
    let mut seen_anything = false;
    loop {
        buf.clear();
        let (resolved, event) = reader.read_resolved_event_into(&mut buf).map_err(xml_error)?;
        let resolved = OwnedNs::of(resolved);
        match event {
            Event::Decl(decl) => {
                if seen_anything {
                    return Err(ExchangeError::Xml("XML-Deklaration nicht am Anfang".into()));
                }
                if let Some(encoding) = decl.encoding() {
                    let encoding = encoding.map_err(xml_error)?;
                    if !encoding.eq_ignore_ascii_case("utf-8") {
                        return Err(ExchangeError::Xml(format!(
                            "Kodierung „{encoding}“ wird nicht unterstützt (nur UTF-8)"
                        )));
                    }
                }
            }
            Event::DocType(_) => {
                return Err(ExchangeError::Unsafe(
                    "DTD (<!DOCTYPE …>) ist nicht erlaubt: keine Entitäten, kein externer Inhalt".into(),
                ));
            }
            Event::PI(pi) => {
                return Err(ExchangeError::Unsafe(format!(
                    "Verarbeitungsanweisung „<?{}…?>“ ist nicht erlaubt",
                    pi.target()
                )));
            }
            Event::Comment(_) => {}
            Event::Start(start) | Event::Empty(start) if root.is_some() => {
                let _ = start;
                return Err(ExchangeError::Xml("Element hinter dem Wurzelelement".into()));
            }
            Event::Start(start) => {
                elements += 1;
                if elements > limits.max_elements {
                    return Err(ExchangeError::Limit(format!("mehr als {} Elemente", limits.max_elements)));
                }
                if stack.len() + 1 > limits.max_depth {
                    return Err(ExchangeError::Limit(format!("tiefer als {} Ebenen", limits.max_depth)));
                }
                let element = element_of(&reader, &start, resolved, limits)?;
                stack.push(element);
            }
            Event::Empty(start) => {
                elements += 1;
                if elements > limits.max_elements {
                    return Err(ExchangeError::Limit(format!("mehr als {} Elemente", limits.max_elements)));
                }
                if stack.len() + 1 > limits.max_depth {
                    return Err(ExchangeError::Limit(format!("tiefer als {} Ebenen", limits.max_depth)));
                }
                let element = element_of(&reader, &start, resolved, limits)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(element),
                    None => root = Some(element),
                }
            }
            Event::End(_) => {
                let element = stack.pop().ok_or_else(|| ExchangeError::Xml("schließendes Tag ohne Anfang".into()))?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(element),
                    None => root = Some(element),
                }
            }
            Event::Text(text) => {
                let raw: &str = &text;
                if raw.contains('<') {
                    return Err(ExchangeError::Xml("„<“ im Text".into()));
                }
                let text = normalize_eol(raw);
                check_legal(&text, "Text")?;
                push_text(&mut stack, &text, limits)?;
            }
            Event::CData(data) => {
                let raw: &str = &data;
                let text = normalize_eol(raw);
                check_legal(&text, "CDATA")?;
                if stack.is_empty() {
                    return Err(ExchangeError::Xml("CDATA außerhalb des Wurzelelements".into()));
                }
                push_text(&mut stack, &text, limits)?;
            }
            Event::GeneralRef(reference) => {
                let c = resolve_reference(&reference)?;
                if stack.is_empty() {
                    return Err(ExchangeError::Xml("Referenz außerhalb des Wurzelelements".into()));
                }
                push_text(&mut stack, c.encode_utf8(&mut [0u8; 4]), limits)?;
            }
            Event::Eof => break,
        }
        seen_anything = true;
    }
    if !stack.is_empty() {
        return Err(ExchangeError::Xml(format!("Element „{}“ nicht geschlossen", stack[stack.len() - 1].name)));
    }
    root.ok_or_else(|| ExchangeError::NotExchange("kein Wurzelelement".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Limits {
        Limits { max_depth: 4, max_elements: 6, max_attributes: 2, max_text_bytes: 16, ..Limits::DEFAULT }
    }

    #[test]
    fn text_is_kept_exactly_and_references_resolve() {
        let root = parse(b"<a>  x &lt;&amp;&gt; &#13;&#x9;y  </a>", &Limits::DEFAULT).unwrap();
        // Leerraum am Rand bleibt, Referenzen werden zu Zeichen, &#13; bleibt ein echtes CR.
        assert_eq!(root.text, "  x <&> \r\ty  ", "Text exakt");
        let root = parse(b"<a>eins\r\nzwei\rdrei</a>", &Limits::DEFAULT).unwrap();
        assert_eq!(root.text, "eins\nzwei\ndrei", "Zeilenenden wie XML 1.0");
    }

    #[test]
    fn attribute_whitespace_follows_xml_but_references_survive() {
        let root = parse(b"<a b=\"x\ty\nz\" c=\"x&#9;y&#10;z&#13;\"/>", &Limits::DEFAULT).unwrap();
        assert_eq!(root.attrs[0].value, "x y z", "wörtlicher Tab/LF wird Leerzeichen");
        assert_eq!(root.attrs[1].value, "x\ty\nz\r", "als Referenz bleiben sie erhalten");
    }

    #[test]
    fn dtd_entities_and_processing_instructions_are_refused() {
        let unsafe_ = |xml: &[u8]| matches!(parse(xml, &Limits::DEFAULT), Err(ExchangeError::Unsafe(_)));
        assert!(unsafe_(b"<!DOCTYPE a [<!ENTITY x \"y\">]><a>&x;</a>"), "DTD mit innerer Entität");
        assert!(unsafe_(b"<a>&nbsp;</a>"), "unbekannte Entität ohne DTD");
        assert!(unsafe_(b"<?xml-stylesheet href=\"x\"?><a/>"), "Verarbeitungsanweisung");
        assert!(unsafe_(b"<a b=\"&x;\"/>"), "unbekannte Entität im Attribut");
        // Die Deklaration ist erlaubt.
        assert!(parse(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?><a/>", &Limits::DEFAULT).is_ok(), "Deklaration");
    }

    #[test]
    fn forbidden_characters_are_refused_even_as_references() {
        assert!(matches!(parse(b"<a>&#1;</a>", &Limits::DEFAULT), Err(ExchangeError::Xml(_))), "&#1;");
        assert!(matches!(parse(b"<a>\x01</a>", &Limits::DEFAULT), Err(ExchangeError::Xml(_))), "U+0001 wörtlich");
    }

    #[test]
    fn limits_are_enforced() {
        let limit = |xml: &[u8]| matches!(parse(xml, &small()), Err(ExchangeError::Limit(_)));
        assert!(limit(b"<a><b><c><d><e/></d></c></b></a>"), "Tiefe 5 > 4");
        assert!(limit(b"<a><b/><b/><b/><b/><b/><b/></a>"), "7 Elemente > 6");
        assert!(limit(b"<a x=\"1\" y=\"2\" z=\"3\"/>"), "3 Attribute > 2");
        assert!(limit(b"<a>0123456789abcdefg</a>"), "17 Bytes Text > 16");
        assert!(limit(b"<a x=\"0123456789abcdefg\"/>"), "17 Bytes Attribut > 16");
        assert!(parse(b"<a><b/></a>", &small()).is_ok(), "unter den Grenzen geht es");
    }
}
