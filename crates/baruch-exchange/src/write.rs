// SPDX-License-Identifier: AGPL-3.0-or-later
//! Writing an exchange file (contract R26 F1/F2).
//!
//! - [`write_xml`]: `werk.xml` in the namespace [`crate::NAMESPACE`]; text exact (`\r` in text, `\t`, `\n`, `\r`
//!   in attributes as a character reference; control characters forbidden in XML 1.0 in text as Base64 with
//!   `kodierung="base64"`, in attributes [`ExchangeError::Write`] with the name of the field).
//! - [`write_manifest`][]: `MANIFEST.xml`.
//! - [`write_baruch`]: ZIP in the order `mimetype` (STORED), `werk.xml`, `bilder/…`, `MANIFEST.xml`.
//! - `<karte>` (from sub-version 2, Round 25) is at the end of `<werk>`, only if there is at least one link
//!   or position.
//! - From sub-version 3 (Round 27): `<satzbild/>` at the end of `<angaben>` (only with at least one setting), in
//!   `<buch>` after `<vita>` always `<verlag-anschrift>` and `<druckerei-anschrift>`, then `<umschlag>` with the
//!   blocks of the layout (only if there are any).
//! - From sub-version 4 (Round 28): on `<beitrag>` (section and thread) `geaendert`, if edited, and after
//!   the other children the `<alte-fassung>` elements (oldest first); `<befunde>` with `<befund>` and their
//!   `<status-eintrag>` at the very end of `<werk>` (after `<karte>`), only if there are findings.
//!
//! The output is deterministic: the same document yields byte-identical XML and ZIP (fixed timestamps and
//! permissions in the ZIP, order as in the document). Between elements there is only whitespace for reading
//! (line break and indentation); it never belongs to a text – text is always in exactly one element without
//! children.
//! - From sub-version 5 (Round 30): on `<befund>` after the status history the `<loesung>` elements (attributes
//!   `zitat`, `ersatz`, `schritt`, only non-empty ones); on `<fassung>` of a variant `variante-gruppe`,
//!   `variante`, `variante-zeile` and after `<text>` per implemented finding `<setzt-um befund>`; on `<punkt>`
//!   `schluessel` (fixed key of the issuing instance, for cross-references); on `<beitrag>` (section and thread)
//!   `grundlage-fassung`, `grundlage`, `antwort-auf`, `wiederholt`, `relay-von`, as far as set.
//! - From sub-version 6 (Round 29): `<satz/>` at the end of `<angaben>` after `<satzbild/>` (only with
//!   typesetting settings; attributes `uuid`, `geaendert`, `geaendert-von` and per set field the words from
//!   [`crate::typeset`]).
//! - From sub-version 7 (Round 31): on `<satz/>` the index/glossary fields
//!   ([`crate::typeset::R31_ATTRIBUTES`]); at the end of `<werk>` after `<befunde>` the own indexes
//!   `<eigene-register>` and the glossary `<glossar>` (only if present).
//! - From sub-version 8 (Round 22): on `<fassung>` `stand` (only if amended) and `beruht-stand` (only if greater
//!   than 0), after `<setzt-um>` per step `<fortschreibung uuid seq art undo-von? autor zeit>` with `<vorher>`
//!   and `<nachher>`; on `<befund>` and `<status-eintrag>` `stand` (only if greater than 0); `<satz>` with the
//!   child `<entwurf/>` if the typesetting settings carry a draft (otherwise `<satz/>` stays empty).
//! - From sub-version 9 (Round 34): on `<einfall>` `titel` (only if not empty), `eltern` (only with a parent
//!   node), `platz` (only if not 0) and `hauptthema="true"` (only for a free main topic).
//!
//! What is written is always the sub-version [`crate::FORMAT_VERSION`] that this program knows, whatever
//! `version` the document carries (unknown things from a higher sub-version are not in the model).

use std::io::{Cursor, Write as _};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::model::*;
use crate::{ExchangeDocument, ExchangeError, ExchangeManifestEntry};

/// Images in the XML: as Base64 in `<daten>` (XML only) or as a reference `datei` to a ZIP entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode {
    Inline,
    Files,
}

/// `true` for a character that must not occur in XML 1.0 (not even as a character reference):
/// control characters below U+0020 except tab, line break and carriage return as well as U+FFFE and U+FFFF.
/// Surrogates do not exist in a Rust `str`.
pub fn forbidden_in_xml(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}')
}

/// Text escaped as element content: `&`, `<`, `>` (so that `]]>` never arises) and `\r` as `&#13;`, otherwise
/// verbatim (tab and line break stay, the reader never trims).
fn escape_text(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

/// Attribute value in double quotation marks: additionally `"` as well as `\t`, `\n`, `\r` as a character
/// reference (otherwise a reader would normalise them to spaces according to XML 1.0 §3.3.3).
fn escape_attr(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
}

/// An attribute: name and value.
type Attr<'a> = (&'static str, std::borrow::Cow<'a, str>);

fn a<'a>(name: &'static str, value: &'a str) -> Attr<'a> {
    (name, std::borrow::Cow::Borrowed(value))
}

fn owned(name: &'static str, value: String) -> Attr<'static> {
    (name, std::borrow::Cow::Owned(value))
}

fn bool_word(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Small XML writer with indentation; every element carries its path for error messages.
struct Xml {
    out: String,
    path: Vec<&'static str>,
}

impl Xml {
    fn new() -> Self {
        Xml { out: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"), path: Vec::new() }
    }

    /// Name of the field for error messages, e.g. `werk/gliederung/punkt/fassung/@autor`.
    fn field(&self, element: &str, attribute: Option<&str>) -> String {
        let mut field = self.path.join("/");
        if !field.is_empty() {
            field.push('/');
        }
        field.push_str(element);
        if let Some(attribute) = attribute {
            field.push_str("/@");
            field.push_str(attribute);
        }
        field
    }

    fn indent(&mut self) {
        if !self.path.is_empty() || !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        for _ in 0..self.path.len() {
            self.out.push_str("  ");
        }
    }

    fn tag_open(&mut self, name: &'static str, attrs: &[Attr<'_>]) -> Result<(), ExchangeError> {
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        for (key, value) in attrs {
            if let Some(c) = value.chars().find(|&c| forbidden_in_xml(c)) {
                return Err(ExchangeError::Write(format!(
                    "verbotenes Zeichen U+{:04X} im Attribut {}",
                    c as u32,
                    self.field(name, Some(key))
                )));
            }
            self.out.push(' ');
            self.out.push_str(key);
            self.out.push_str("=\"");
            escape_attr(value, &mut self.out);
            self.out.push('"');
        }
        Ok(())
    }

    /// `<name …>` with children; afterwards [`Xml::end`].
    fn start(&mut self, name: &'static str, attrs: &[Attr<'_>]) -> Result<(), ExchangeError> {
        self.tag_open(name, attrs)?;
        self.out.push('>');
        self.path.push(name);
        Ok(())
    }

    fn end(&mut self) {
        let name = self.path.pop().expect("end ohne start");
        self.indent();
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push('>');
    }

    /// Empty element `<name …/>`.
    fn empty(&mut self, name: &'static str, attrs: &[Attr<'_>]) -> Result<(), ExchangeError> {
        self.tag_open(name, attrs)?;
        self.out.push_str("/>");
        Ok(())
    }

    /// Text element (type `Text` in the XSD): exact, with forbidden characters as Base64 with `kodierung="base64"`.
    fn text(&mut self, name: &'static str, attrs: &[Attr<'_>], value: &str) -> Result<(), ExchangeError> {
        if value.chars().any(forbidden_in_xml) {
            let mut with = attrs.to_vec();
            with.push(a("kodierung", "base64"));
            self.tag_open(name, &with)?;
            self.out.push('>');
            self.out.push_str(&STANDARD.encode(value.as_bytes()));
        } else if value.is_empty() {
            return self.empty(name, attrs);
        } else {
            self.tag_open(name, attrs)?;
            self.out.push('>');
            escape_text(value, &mut self.out);
        }
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push('>');
        Ok(())
    }

    /// Element with plain content without Base64 (time, Base64 data): forbidden characters → error.
    fn plain(&mut self, name: &'static str, value: &str) -> Result<(), ExchangeError> {
        if let Some(c) = value.chars().find(|&c| forbidden_in_xml(c)) {
            return Err(ExchangeError::Write(format!(
                "verbotenes Zeichen U+{:04X} in {}",
                c as u32,
                self.field(name, None)
            )));
        }
        self.tag_open(name, &[])?;
        self.out.push('>');
        escape_text(value, &mut self.out);
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push('>');
        Ok(())
    }

    fn finish(mut self) -> String {
        debug_assert!(self.path.is_empty(), "offene Elemente: {:?}", self.path);
        self.out.push('\n');
        self.out
    }

    /// Checks an identifier (canonical UUID); otherwise the output would not be valid according to the XSD.
    fn uuid<'a>(&self, element: &str, attribute: &'static str, value: &'a str) -> Result<Attr<'a>, ExchangeError> {
        if is_uuid(value) {
            Ok(a(attribute, value))
        } else {
            Err(ExchangeError::Write(format!(
                "keine kanonische UUID in {}: „{value}“",
                self.field(element, Some(attribute))
            )))
        }
    }

    /// Checks a time (not empty, as in the XSD type `Zeit`).
    fn time<'a>(&self, element: &str, attribute: &'static str, value: &'a str) -> Result<Attr<'a>, ExchangeError> {
        if value.is_empty() {
            Err(ExchangeError::Write(format!("leerer Zeitpunkt in {}", self.field(element, Some(attribute)))))
        } else {
            Ok(a(attribute, value))
        }
    }

    /// Positive number (XSD `positiveInteger`).
    fn positive(&self, element: &str, attribute: &'static str, value: u32) -> Result<Attr<'static>, ExchangeError> {
        if value == 0 {
            Err(ExchangeError::Write(format!("0 ist keine gültige Zahl in {}", self.field(element, Some(attribute)))))
        } else {
            Ok(owned(attribute, value.to_string()))
        }
    }

    /// Finite decimal number (XSD `double`, without NaN and infinity).
    fn double(&self, element: &str, attribute: &'static str, value: f64) -> Result<Attr<'static>, ExchangeError> {
        if value.is_finite() {
            Ok(owned(attribute, value.to_string()))
        } else {
            Err(ExchangeError::Write(format!("keine endliche Zahl in {}", self.field(element, Some(attribute)))))
        }
    }
}

/// `werk.xml` as text.
pub fn write_xml(doc: &ExchangeDocument, images: ImageMode) -> Result<String, ExchangeError> {
    let mut x = Xml::new();
    let work = &doc.work;
    let version = FORMAT_VERSION.to_string();
    let root = [
        a("xmlns", NAMESPACE),
        a("version", &version),
        x.uuid("werk", "uuid", &work.uuid)?,
        a("art", work.kind.as_xml()),
    ];
    x.start("werk", &root)?;
    let ausgabe = [
        x.time("ausgabe", "zeit", &doc.exported)?,
        x.uuid("ausgabe", "instanz", &doc.instance.uuid)?,
        a("instanzname", &doc.instance.name),
        a("programm", &doc.generator),
    ];
    x.empty("ausgabe", &ausgabe)?;
    write_details(&mut x, work)?;
    if let Some(book) = &work.book {
        write_book(&mut x, book)?;
    }
    write_template(&mut x, &work.template)?;
    x.start("gliederung", &[])?;
    for node in &work.outline {
        write_node(&mut x, node)?;
    }
    x.end();
    if !work.ideas.is_empty() {
        x.start("einfaelle", &[])?;
        for idea in &work.ideas {
            write_idea(&mut x, idea)?;
        }
        x.end();
    }
    if !work.threads.is_empty() {
        x.start("faeden", &[])?;
        for thread in &work.threads {
            write_thread(&mut x, thread)?;
        }
        x.end();
    }
    if !work.assets.is_empty() {
        x.start("bilder", &[])?;
        for asset in &work.assets {
            write_asset(&mut x, asset, images)?;
        }
        x.end();
    }
    if !work.deleted.is_empty() {
        x.start("geloescht", &[])?;
        for stone in &work.deleted {
            let attrs = [
                x.uuid("eintrag", "uuid", &stone.uuid)?,
                a("art", stone.kind.as_xml()),
                x.time("eintrag", "zeit", &stone.deleted)?,
            ];
            x.empty("eintrag", &attrs)?;
        }
        x.end();
    }
    if !work.map.is_empty() {
        write_map(&mut x, &work.map)?;
    }
    if !work.findings.is_empty() {
        write_findings(&mut x, &work.findings)?;
    }
    if !work.registers.is_empty() {
        write_registers(&mut x, &work.registers)?;
    }
    if !work.glossary.is_empty() {
        write_glossary(&mut x, &work.glossary)?;
    }
    x.end();
    Ok(x.finish())
}

/// `<eigene-register>` with one `<register uuid kennung titel angelegt/>` each in their order (from sub-version 7).
fn write_registers(x: &mut Xml, registers: &[ExchangeRegister]) -> Result<(), ExchangeError> {
    x.start("eigene-register", &[])?;
    for register in registers {
        let attrs = [
            x.uuid("register", "uuid", &register.uuid)?,
            a("kennung", &register.slug),
            a("titel", &register.title),
            x.time("register", "angelegt", &register.created)?,
        ];
        x.empty("register", &attrs)?;
    }
    x.end();
    Ok(())
}

/// `<glossar>` with one `<begriff uuid status herkunft angelegt [geaendert] [geaendert-von]>` each and the
/// children `<wort>`, `<kurz>`, `<ausfuehrlich>`, `<quelle>` (only set ones), per variant `<variante>`, per
/// reference `<bibelstelle>` (from sub-version 7).
fn write_glossary(x: &mut Xml, glossary: &[ExchangeGlossaryTerm]) -> Result<(), ExchangeError> {
    x.start("glossar", &[])?;
    for term in glossary {
        let mut attrs = vec![
            x.uuid("begriff", "uuid", &term.uuid)?,
            a("status", term.status.as_xml()),
            a("herkunft", &term.origin),
            x.time("begriff", "angelegt", &term.created)?,
        ];
        if let Some(updated) = &term.updated {
            attrs.push(x.time("begriff", "geaendert", updated)?);
        }
        if !term.updated_by.is_empty() {
            attrs.push(a("geaendert-von", &term.updated_by));
        }
        x.start("begriff", &attrs)?;
        x.text("wort", &[], &term.term)?;
        for (name, value) in [("kurz", &term.short), ("ausfuehrlich", &term.long), ("quelle", &term.source)] {
            if !value.is_empty() {
                x.text(name, &[], value)?;
            }
        }
        for variant in &term.variants {
            x.text("variante", &[], variant)?;
        }
        for reference in &term.bible_refs {
            x.text("bibelstelle", &[], reference)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

/// `<alte-fassung>` per old version of a post (from sub-version 4).
fn write_versions(x: &mut Xml, versions: &[ExchangePostVersion]) -> Result<(), ExchangeError> {
    for version in versions {
        let attrs = [
            x.uuid("alte-fassung", "uuid", &version.uuid)?,
            x.time("alte-fassung", "gueltig-ab", &version.valid_from)?,
            x.time("alte-fassung", "ersetzt", &version.replaced)?,
            a("bearbeitet-von", &version.edited_by),
        ];
        x.start("alte-fassung", &attrs)?;
        x.text("text", &[], &version.body)?;
        x.end();
    }
    Ok(())
}

/// Optional identifier as an attribute.
fn push_uuid<'a>(
    x: &Xml,
    attrs: &mut Vec<Attr<'a>>,
    element: &str,
    name: &'static str,
    value: &'a Option<String>,
) -> Result<(), ExchangeError> {
    if let Some(value) = value {
        attrs.push(x.uuid(element, name, value)?);
    }
    Ok(())
}

/// `<befunde>` (from sub-version 4): per finding origin, kind, status, text, quotation, review note, reason,
/// collection and status history, in the order of the document.
fn write_findings(x: &mut Xml, findings: &[ExchangeFinding]) -> Result<(), ExchangeError> {
    x.start("befunde", &[])?;
    for finding in findings {
        let mut attrs = vec![
            x.uuid("befund", "uuid", &finding.uuid)?,
            a("art", finding.kind.as_xml()),
            a("status", finding.status.as_xml()),
            a("persona", &finding.persona),
            owned("stelle", finding.position.to_string()),
        ];
        push_uuid(x, &mut attrs, "befund", "punkt", &finding.node)?;
        push_uuid(x, &mut attrs, "befund", "beitrag", &finding.post)?;
        push_uuid(x, &mut attrs, "befund", "faden-beitrag", &finding.thread_post)?;
        push_uuid(x, &mut attrs, "befund", "fassung", &finding.adopted_revision)?;
        push_uuid(x, &mut attrs, "befund", "ziel-punkt", &finding.deferred_node)?;
        push_uuid(x, &mut attrs, "befund", "ziel-einfall", &finding.deferred_idea)?;
        push_uuid(x, &mut attrs, "befund", "gleicht", &finding.similar_to)?;
        // State 0 (revision never amended) is not written: equivalent to "no state", and files
        // without an amendment stay valid according to the older schemas (integrator Round 22).
        if let Some(stand) = finding.adopted_amend.filter(|&k| k > 0) {
            attrs.push(owned("stand", stand.to_string()));
        }
        attrs.push(x.time("befund", "angelegt", &finding.created)?);
        attrs.push(x.time("befund", "geaendert", &finding.updated)?);
        x.start("befund", &attrs)?;
        x.text("text", &[], &finding.body)?;
        if !finding.quote.is_empty() {
            x.text("zitat", &[], &finding.quote)?;
        }
        if !finding.quote_check.is_empty() {
            x.text("pruefvermerk", &[], &finding.quote_check)?;
        }
        if !finding.reason.is_empty() {
            x.text("begruendung", &[], &finding.reason)?;
        }
        for summarized in &finding.summary_of {
            let attrs = [x.uuid("sammelt", "befund", summarized)?];
            x.empty("sammelt", &attrs)?;
        }
        for entry in &finding.history {
            let mut attrs = vec![
                x.uuid("status-eintrag", "uuid", &entry.uuid)?,
                a("status", entry.status.as_xml()),
                a("herkunft-art", entry.actor_kind.as_xml()),
                a("herkunft", &entry.actor),
                a("vorschlag", bool_word(entry.proposed)),
            ];
            push_uuid(x, &mut attrs, "status-eintrag", "fassung", &entry.adopted_revision)?;
            push_uuid(x, &mut attrs, "status-eintrag", "ziel-punkt", &entry.deferred_node)?;
            push_uuid(x, &mut attrs, "status-eintrag", "ziel-einfall", &entry.deferred_idea)?;
            if let Some(decision) = entry.decision {
                attrs.push(a("entscheidung", decision.as_xml()));
            }
            if let Some(decided) = &entry.decided {
                attrs.push(x.time("status-eintrag", "entschieden", decided)?);
            }
            if let Some(by) = &entry.decided_by {
                attrs.push(a("entschieden-von", by));
            }
            if let Some(stand) = entry.adopted_amend.filter(|&k| k > 0) {
                attrs.push(owned("stand", stand.to_string()));
            }
            attrs.push(x.time("status-eintrag", "angelegt", &entry.created)?);
            if entry.reason.is_empty() && entry.note.is_empty() {
                x.empty("status-eintrag", &attrs)?;
                continue;
            }
            x.start("status-eintrag", &attrs)?;
            if !entry.reason.is_empty() {
                x.text("begruendung", &[], &entry.reason)?;
            }
            if !entry.note.is_empty() {
                x.text("notiz", &[], &entry.note)?;
            }
            x.end();
        }
        for solution in &finding.solutions {
            let mut attrs = Vec::new();
            for (name, value) in
                [("zitat", &solution.quote), ("ersatz", &solution.replacement), ("schritt", &solution.step)]
            {
                if !value.is_empty() {
                    attrs.push(a(name, value));
                }
            }
            x.empty("loesung", &attrs)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

/// Metadata of a post as attributes (from sub-version 5).
fn push_post_meta<'a>(x: &Xml, attrs: &mut Vec<Attr<'a>>, meta: &'a ExchangePostMeta) -> Result<(), ExchangeError> {
    push_uuid(x, attrs, "beitrag", "grundlage-fassung", &meta.basis_revision)?;
    if !meta.basis_label.is_empty() {
        attrs.push(a("grundlage", &meta.basis_label));
    }
    push_uuid(x, attrs, "beitrag", "antwort-auf", &meta.reply_to)?;
    push_uuid(x, attrs, "beitrag", "wiederholt", &meta.repeat_of)?;
    if let Some(relay) = &meta.relay_from {
        attrs.push(a("relay-von", relay));
    }
    Ok(())
}

/// `<karte>` (from sub-version 2): first all links, then all positions, each in the order of the document.
/// Coordinates as the shortest representation that yields exactly the same number on reading (`f64` → `to_string`).
fn write_map(x: &mut Xml, map: &ExchangeMap) -> Result<(), ExchangeError> {
    x.start("karte", &[])?;
    for link in &map.links {
        let mut attrs = vec![
            x.uuid("verbindung", "uuid", &link.uuid)?,
            x.uuid("verbindung", "von", &link.from)?,
            x.uuid("verbindung", "nach", &link.to)?,
            a("art", link.kind.as_xml()),
            a("herkunft-art", link.origin_kind.as_xml()),
            a("herkunft", &link.origin_name),
            x.time("verbindung", "angelegt", &link.created)?,
            x.time("verbindung", "geaendert", &link.updated)?,
        ];
        if let Some(deleted) = &link.deleted {
            attrs.push(x.time("verbindung", "geloescht", deleted)?);
        }
        x.start("verbindung", &attrs)?;
        x.text("notiz", &[], &link.note)?;
        x.end();
    }
    for position in &map.positions {
        for (name, value) in [("x", position.x), ("y", position.y)] {
            if value.abs() > MAP_COORD_LIMIT {
                return Err(ExchangeError::Write(format!(
                    "Koordinate {value} außerhalb von ±{MAP_COORD_LIMIT} in {}",
                    x.field("lage", Some(name))
                )));
            }
        }
        let attrs = [
            x.uuid("lage", "uuid", &position.uuid)?,
            x.uuid("lage", "einfall", &position.idea)?,
            x.double("lage", "x", position.x)?,
            x.double("lage", "y", position.y)?,
            x.time("lage", "geaendert", &position.updated)?,
        ];
        x.empty("lage", &attrs)?;
    }
    x.end();
    Ok(())
}

fn write_details(x: &mut Xml, work: &ExchangeWork) -> Result<(), ExchangeError> {
    x.start("angaben", &[a("sprache", work.language.as_xml())])?;
    x.text("titel", &[], &work.title)?;
    x.text("untertitel", &[], &work.subtitle)?;
    x.text("autor", &[], &work.author)?;
    x.text("zielgruppe", &[], &work.audience)?;
    x.text("bibel", &[], &work.bible)?;
    if work.created.is_empty() {
        return Err(ExchangeError::Write(format!("leerer Zeitpunkt in {}", x.field("angelegt", None))));
    }
    x.plain("angelegt", &work.created)?;
    if let Some(updated) = &work.updated {
        x.plain("geaendert", updated)?;
    }
    let schema = [a("name", &work.numbering_scheme)];
    match &work.numbering {
        Some(pattern) => {
            x.start("schema", &schema)?;
            x.text("muster", &[], pattern)?;
            x.end();
        }
        None => x.empty("schema", &schema)?,
    }
    if let Some(module) = &work.bible_module {
        x.text("bibelmodul", &[], module)?;
    }
    if !work.sword_sources.is_empty() {
        x.start("quellen", &[])?;
        for source in &work.sword_sources {
            x.text("quelle", &[a("art", source.kind.as_xml())], &source.module)?;
        }
        x.end();
    }
    if let Some(timing) = &work.timing {
        let mut attrs = vec![x.positive("sprechdauer", "woerter-je-minute", timing.words_per_minute)?];
        if let Some(seconds) = timing.target_seconds {
            attrs.push(owned("ziel-sekunden", seconds.to_string()));
        }
        x.empty("sprechdauer", &attrs)?;
    }
    if let Some(sheet) = work.sheet.as_ref().filter(|s| !s.is_empty()) {
        let mut attrs = Vec::new();
        for (name, value) in [
            ("schrift", &sheet.font),
            ("schrift-ueberschriften", &sheet.heading_font),
            ("groesse", &sheet.size),
            ("farbe", &sheet.fill),
            ("farbe-ueberschriften", &sheet.heading_fill),
            ("blattfarbe", &sheet.background),
            ("satzbreite", &sheet.width),
        ] {
            if let Some(value) = value {
                attrs.push(a(name, value));
            }
        }
        x.empty("satzbild", &attrs)?;
    }
    if let Some(typeset) = &work.typeset {
        let mut attrs = vec![
            x.uuid("satz", "uuid", &typeset.uuid)?,
            x.time("satz", "geaendert", &typeset.updated)?,
            a("geaendert-von", &typeset.updated_by),
        ];
        let fields = crate::typeset::settings_attrs(&typeset.settings)
            .map_err(|e| ExchangeError::Write(format!("{}{e}", x.field("satz", None) + "/")))?;
        attrs.extend(fields.into_iter().map(|(name, value)| owned(name, value)));
        match &typeset.settings.draft {
            Some(draft) => {
                x.start("satz", &attrs)?;
                let fields: Vec<Attr<'static>> =
                    crate::typeset::draft_attrs(draft).into_iter().map(|(name, value)| owned(name, value)).collect();
                x.empty("entwurf", &fields)?;
                x.end();
            }
            None => x.empty("satz", &attrs)?,
        }
    }
    x.end();
    Ok(())
}

/// Colour of a cover block in the XML: `hell`, `dunkel` or `#rrggbb`.
pub fn cover_color_word(color: &str) -> &str {
    match color {
        "light" => "hell",
        "dark" => "dunkel",
        other => other,
    }
}

/// `<block>` of the cover layout (from sub-version 3).
fn write_cover_block(x: &mut Xml, block: &ExchangeCoverBlock) -> Result<(), ExchangeError> {
    let color = cover_color_word(&block.color);
    if !(color == "hell"
        || color == "dunkel"
        || (color.len() == 7
            && color.starts_with('#')
            && color[1..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())))
    {
        return Err(ExchangeError::Write(format!("Farbe „{color}“ ungültig in {}", x.field("block", Some("farbe")))));
    }
    let mut attrs = vec![
        x.uuid("block", "uuid", &block.uuid)?,
        a("art", block.kind.as_xml()),
        x.double("block", "x-mm", block.x_mm)?,
        x.double("block", "y-mm", block.y_mm)?,
        x.double("block", "breite-mm", block.width_mm)?,
        a("ausrichtung", block.align.as_xml()),
        x.double("block", "groesse-pt", block.size_pt)?,
        x.double("block", "zeilenabstand-em", block.leading_em)?,
        a("farbe", color),
    ];
    if let Some(font) = &block.font {
        attrs.push(a("schrift", font));
    }
    attrs.push(a("schnitt", block.style.as_xml()));
    attrs.push(a("hintergrund", block.backdrop.as_xml()));
    attrs.push(a("sichtbar", bool_word(block.visible)));
    if let Some(direction) = block.direction {
        attrs.push(a("richtung", direction.as_xml()));
    }
    attrs.push(x.time("block", "geaendert", &block.updated)?);
    match &block.text {
        Some(text) => {
            x.start("block", &attrs)?;
            x.text("text", &[], text)?;
            x.end();
        }
        None => x.empty("block", &attrs)?,
    }
    Ok(())
}

fn write_book(x: &mut Xml, book: &ExchangeBook) -> Result<(), ExchangeError> {
    let mut attrs = vec![
        a("titelbereich", book.title_area.as_xml()),
        a("titelei", bool_word(book.front_matter)),
        a("umschlagseiten", bool_word(book.cover_pages)),
        x.double("buch", "papier-mm", book.paper_mm)?,
        x.double("buch", "beschnitt-mm", book.bleed_mm)?,
    ];
    if let Some(cover) = &book.cover {
        attrs.push(x.uuid("buch", "umschlag", cover)?);
    }
    if let Some(photo) = &book.author_photo {
        attrs.push(x.uuid("buch", "autorenfoto", photo)?);
    }
    if let Some(updated) = &book.updated {
        attrs.push(x.time("buch", "geaendert", updated)?);
    }
    x.start("buch", &attrs)?;
    for (name, value) in [
        ("verlag", &book.publisher),
        ("ort", &book.place),
        ("jahr", &book.year),
        ("auflage", &book.edition),
        ("isbn", &book.isbn),
        ("preis", &book.price),
        ("reihe", &book.series),
        ("druckerei", &book.printer),
        ("bibelvermerk", &book.bible_notice),
        ("bildnachweis", &book.image_credit),
        ("widmung", &book.dedication),
        ("klappentext", &book.blurb),
        ("vita", &book.bio),
        ("verlag-anschrift", &book.publisher_address),
        ("druckerei-anschrift", &book.printer_address),
    ] {
        x.text(name, &[], value)?;
    }
    if !book.cover_blocks.is_empty() {
        x.start("umschlag", &[])?;
        for block in &book.cover_blocks {
            write_cover_block(x, block)?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

fn write_template(x: &mut Xml, template: &ExchangeTemplate) -> Result<(), ExchangeError> {
    x.start("vorlage", &[])?;
    x.text("text", &[], &template.current)?;
    for revision in &template.revisions {
        let attrs = [
            x.uuid("fassung", "uuid", &revision.uuid)?,
            x.positive("fassung", "nummer", revision.number)?,
            a("autor", &revision.author),
            x.time("fassung", "angelegt", &revision.created)?,
        ];
        x.start("fassung", &attrs)?;
        if !revision.note.is_empty() {
            x.text("notiz", &[], &revision.note)?;
        }
        x.text("text", &[], &revision.body)?;
        x.end();
    }
    x.end();
    Ok(())
}

fn write_node(x: &mut Xml, node: &ExchangeNode) -> Result<(), ExchangeError> {
    let mut attrs = vec![
        x.uuid("punkt", "uuid", &node.uuid)?,
        a("art", node.kind.as_xml()),
        a("nummeriert", bool_word(node.numbered)),
    ];
    if let Some(key) = node.key {
        attrs.push(owned("schluessel", key.to_string()));
    }
    x.start("punkt", &attrs)?;
    x.text("titel", &[], &node.title)?;
    for revision in &node.revisions {
        let mut attrs = vec![
            x.uuid("fassung", "uuid", &revision.uuid)?,
            x.positive("fassung", "nummer", revision.number)?,
            a("status", revision.status.as_xml()),
        ];
        match &revision.author {
            ExchangeAuthor::Human { name } => {
                attrs.push(a("autor-art", "mensch"));
                attrs.push(a("autor", name));
            }
            ExchangeAuthor::Assistant { persona, model } => {
                attrs.push(a("autor-art", "assistent"));
                attrs.push(a("autor", persona));
                attrs.push(a("modell", model));
            }
        }
        if let Some(base) = &revision.based_on {
            attrs.push(x.uuid("fassung", "basis", base)?);
        }
        attrs.push(x.time("fassung", "angelegt", &revision.created)?);
        if let Some(edited) = &revision.edited {
            attrs.push(x.time("fassung", "geaendert", edited)?);
        }
        if let Some(variant) = &revision.variant {
            attrs.push(x.uuid("fassung", "variante-gruppe", &variant.group)?);
            attrs.push(a("variante", &variant.label));
            if !variant.summary.is_empty() {
                attrs.push(a("variante-zeile", &variant.summary));
            }
        }
        if revision.amend_seq > 0 {
            attrs.push(owned("stand", revision.amend_seq.to_string()));
        }
        if let Some(stand) = revision.based_on_amend.filter(|&k| k > 0) {
            attrs.push(owned("beruht-stand", stand.to_string()));
        }
        x.start("fassung", &attrs)?;
        if !revision.note.is_empty() {
            x.text("notiz", &[], &revision.note)?;
        }
        x.text("text", &[], &revision.body)?;
        for finding in revision.variant.iter().flat_map(|v| &v.findings) {
            let attrs = [x.uuid("setzt-um", "befund", finding)?];
            x.empty("setzt-um", &attrs)?;
        }
        for step in &revision.amends {
            let mut attrs = vec![
                x.uuid("fortschreibung", "uuid", &step.uuid)?,
                x.positive("fortschreibung", "seq", step.seq)?,
                a("art", step.kind.as_xml()),
            ];
            if let Some(of) = step.undo_of {
                attrs.push(x.positive("fortschreibung", "undo-von", of)?);
            }
            attrs.push(a("autor", &step.author));
            attrs.push(x.time("fortschreibung", "zeit", &step.created)?);
            x.start("fortschreibung", &attrs)?;
            x.text("vorher", &[], &step.before)?;
            x.text("nachher", &[], &step.after)?;
            x.end();
        }
        x.end();
    }
    for post in &node.posts {
        let mut attrs = vec![
            x.uuid("beitrag", "uuid", &post.uuid)?,
            x.positive("beitrag", "position", post.position)?,
            a("autor", &post.author),
            x.time("beitrag", "angelegt", &post.created)?,
        ];
        if let Some(edited) = &post.edited {
            attrs.push(x.time("beitrag", "geaendert", edited)?);
        }
        push_post_meta(x, &mut attrs, &post.meta)?;
        x.start("beitrag", &attrs)?;
        x.text("text", &[], &post.body)?;
        for source in &post.sources {
            x.text("quelle", &[], source)?;
        }
        for reference in &post.refs {
            let mut attrs = vec![x.uuid("bezug", "punkt", &reference.node)?];
            if let Some(revision) = &reference.revision {
                attrs.push(x.uuid("bezug", "fassung", revision)?);
            }
            x.text("bezug", &attrs, &reference.quote)?;
        }
        write_versions(x, &post.versions)?;
        x.end();
    }
    for child in &node.children {
        write_node(x, child)?;
    }
    x.end();
    Ok(())
}

fn write_idea(x: &mut Xml, idea: &ExchangeIdea) -> Result<(), ExchangeError> {
    let mut attrs = vec![x.uuid("einfall", "uuid", &idea.uuid)?, a("status", idea.status.as_xml())];
    if let Some(used) = idea.done_as {
        attrs.push(a("erledigt-als", used.as_xml()));
    }
    attrs.push(a("herkunft-art", idea.origin_kind.as_xml()));
    attrs.push(a("herkunft", &idea.origin_name));
    attrs.push(x.time("einfall", "angelegt", &idea.created)?);
    attrs.push(x.time("einfall", "geaendert", &idea.updated)?);
    if let Some(deleted) = &idea.deleted {
        attrs.push(x.time("einfall", "geloescht", deleted)?);
    }
    if let Some(node) = &idea.node {
        attrs.push(x.uuid("einfall", "bezug", node)?);
    }
    // From sub-version 9 (Round 34): only what deviates from the default – a work without a tree outputs the same
    // attributes as before.
    if !idea.title.is_empty() {
        attrs.push(a("titel", &idea.title));
    }
    if let Some(parent) = &idea.parent {
        attrs.push(x.uuid("einfall", "eltern", parent)?);
    }
    if idea.position != 0 {
        attrs.push(owned("platz", idea.position.to_string()));
    }
    if idea.main_topic {
        attrs.push(a("hauptthema", bool_word(true)));
    }
    x.start("einfall", &attrs)?;
    for word in &idea.words {
        x.text("schlagwort", &[], word)?;
    }
    x.text("notiz", &[], &idea.note)?;
    x.text("zitat", &[], &idea.quote)?;
    x.text("quelle", &[], &idea.source)?;
    x.text("bibelstelle", &[], &idea.bible)?;
    if let Some(reference) = &idea.done_ref {
        x.text("erledigt-ref", &[], reference)?;
    }
    x.end();
    Ok(())
}

fn write_thread(x: &mut Xml, thread: &ExchangeThread) -> Result<(), ExchangeError> {
    let attrs = [
        x.uuid("faden", "uuid", &thread.uuid)?,
        a("angelegt-von", &thread.created_by),
        x.time("faden", "angelegt", &thread.created)?,
        x.time("faden", "geaendert", &thread.updated)?,
    ];
    x.start("faden", &attrs)?;
    x.text("titel", &[], &thread.title)?;
    for post in &thread.posts {
        let mut attrs = vec![
            x.uuid("beitrag", "uuid", &post.uuid)?,
            x.positive("beitrag", "position", post.position)?,
            a("autor", &post.author),
            x.time("beitrag", "angelegt", &post.created)?,
        ];
        if let Some(edited) = &post.edited {
            attrs.push(x.time("beitrag", "geaendert", edited)?);
        }
        push_post_meta(x, &mut attrs, &post.meta)?;
        x.start("beitrag", &attrs)?;
        x.text("text", &[], &post.body)?;
        for source in &post.sources {
            x.text("quelle", &[], source)?;
        }
        write_versions(x, &post.versions)?;
        x.end();
    }
    x.end();
    Ok(())
}

/// Checks an image revision (type of the format, checksum matches the bytes) and returns the entry in the ZIP.
fn image_file(revision: &ExchangeAssetRevision) -> Result<String, ExchangeError> {
    let entry = image_entry(&revision.uuid, &revision.mime).ok_or_else(|| {
        ExchangeError::Write(format!("Bildfassung {}: Typ „{}“ gehört nicht zum Format", revision.uuid, revision.mime))
    })?;
    if revision.sha256 != sha256_hex(&revision.data) {
        return Err(ExchangeError::Write(format!("Bildfassung {}: sha256 passt nicht zu den Daten", revision.uuid)));
    }
    Ok(entry)
}

fn write_asset(x: &mut Xml, asset: &ExchangeAsset, images: ImageMode) -> Result<(), ExchangeError> {
    let attrs =
        [x.uuid("bild", "uuid", &asset.uuid)?, a("name", &asset.name), x.time("bild", "angelegt", &asset.created)?];
    x.start("bild", &attrs)?;
    for path in &asset.paths {
        x.text("pfad", &[], path)?;
    }
    for revision in &asset.revisions {
        let entry = image_file(revision)?;
        let mut attrs =
            vec![x.uuid("fassung", "uuid", &revision.uuid)?, x.positive("fassung", "nummer", revision.number)?];
        if let Some(base) = &revision.based_on {
            attrs.push(x.uuid("fassung", "basis", base)?);
        }
        attrs.push(a("mime", &revision.mime));
        attrs.push(owned("bytes", revision.data.len().to_string()));
        attrs.push(a("sha256", &revision.sha256));
        attrs.push(a("status", revision.status.as_xml()));
        attrs.push(a("autor", &revision.author));
        attrs.push(x.time("fassung", "angelegt", &revision.created)?);
        if let Some(edited) = &revision.edited {
            attrs.push(x.time("fassung", "geaendert", edited)?);
        }
        if images == ImageMode::Files {
            attrs.push(owned("datei", entry));
        }
        x.start("fassung", &attrs)?;
        x.text("unterschrift", &[], &revision.caption)?;
        x.text("alt", &[], &revision.alt)?;
        if !revision.note.is_empty() {
            x.text("notiz", &[], &revision.note)?;
        }
        if images == ImageMode::Inline {
            x.plain("daten", &STANDARD.encode(&revision.data))?;
        }
        x.end();
    }
    x.end();
    Ok(())
}

/// `MANIFEST.xml` as text.
pub fn write_manifest(entries: &[ExchangeManifestEntry]) -> Result<String, ExchangeError> {
    if entries.is_empty() {
        return Err(ExchangeError::Write("Manifest ohne Einträge".into()));
    }
    let mut x = Xml::new();
    let version = FORMAT_VERSION.to_string();
    x.start("manifest", &[a("xmlns", NAMESPACE), a("version", &version)])?;
    for entry in entries {
        if entry.sha256.len() != 64 || !entry.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return Err(ExchangeError::Write(format!("Manifest: keine SHA-256 für {}", entry.path)));
        }
        let attrs = [a("pfad", &entry.path), a("sha256", &entry.sha256), owned("bytes", entry.bytes.to_string())];
        x.empty("datei", &attrs)?;
    }
    x.end();
    Ok(x.finish())
}

/// Whole container `.baruch` (ZIP).
///
/// Order according to F1: `mimetype` (STORED, without line end), `werk.xml` (DEFLATE), per image revision
/// `bilder/<fassungs-uuid>.<ext>` in the order of the document (raster STORED, SVG DEFLATE), last
/// `MANIFEST.xml` (DEFLATE) with SHA-256 and byte count of all entries except `mimetype` and itself. All
/// entries carry the fixed timestamp 1980-01-01 00:00:00 and the permissions 0644.
pub fn write_baruch(doc: &ExchangeDocument) -> Result<Vec<u8>, ExchangeError> {
    let xml = write_xml(doc, ImageMode::Files)?;
    let mut files: Vec<(String, &[u8], bool)> = vec![(ENTRY_WORK.to_owned(), xml.as_bytes(), true)];
    for revision in doc.work.assets.iter().flat_map(|asset| &asset.revisions) {
        let entry = image_file(revision)?;
        if files.iter().any(|(name, _, _)| *name == entry) {
            return Err(ExchangeError::Write(format!("Bildfassung {} steht doppelt im Dokument", revision.uuid)));
        }
        files.push((entry, &revision.data, revision.mime == "image/svg+xml"));
    }
    let manifest: Vec<ExchangeManifestEntry> = files
        .iter()
        .map(|(path, data, _)| ExchangeManifestEntry {
            path: path.clone(),
            sha256: sha256_hex(data),
            bytes: data.len() as u64,
        })
        .collect();
    let manifest = write_manifest(&manifest)?;

    let archive_error = |e: &dyn std::fmt::Display| ExchangeError::Write(format!("ZIP: {e}"));
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let base =
        zip::write::SimpleFileOptions::default().last_modified_time(zip::DateTime::default()).unix_permissions(0o644);
    let stored = base.compression_method(zip::CompressionMethod::Stored);
    let deflated = base.compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(ENTRY_MIMETYPE, stored).map_err(|e| archive_error(&e))?;
    zip.write_all(ZIP_MIMETYPE.as_bytes()).map_err(|e| archive_error(&e))?;
    for (path, data, compress) in &files {
        zip.start_file(path.as_str(), if *compress { deflated } else { stored }).map_err(|e| archive_error(&e))?;
        zip.write_all(data).map_err(|e| archive_error(&e))?;
    }
    zip.start_file(ENTRY_MANIFEST, deflated).map_err(|e| archive_error(&e))?;
    zip.write_all(manifest.as_bytes()).map_err(|e| archive_error(&e))?;
    Ok(zip.finish().map_err(|e| archive_error(&e))?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_characters_follow_xml_1_0() {
        // Tab, line break and carriage return are allowed, the other C0 control characters are not (XML 1.0 §2.2).
        for c in ['\t', '\n', '\r', ' ', 'ä', '\u{7f}', '\u{fffd}', '\u{10000}'] {
            assert!(!forbidden_in_xml(c), "{c:?} ist in XML 1.0 erlaubt");
        }
        for c in ['\u{0}', '\u{1}', '\u{8}', '\u{b}', '\u{c}', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
            assert!(forbidden_in_xml(c), "{c:?} ist in XML 1.0 verboten");
        }
    }

    #[test]
    fn escaping_keeps_text_exact() {
        let mut text = String::new();
        escape_text(" a\t<&>]]>\r\n ", &mut text);
        // `>` always escaped, so that `]]>` is never in the content; `\r` as a character reference, whitespace
        // verbatim.
        assert_eq!(text, " a\t&lt;&amp;&gt;]]&gt;&#13;\n ", "Elementinhalt");
        let mut attr = String::new();
        escape_attr("\"a\tb\nc\rd\"", &mut attr);
        // In an attribute \t, \n and \r would otherwise be normalised to spaces.
        assert_eq!(attr, "&quot;a&#9;b&#10;c&#13;d&quot;", "Attributwert");
    }
}
