// SPDX-License-Identifier: AGPL-3.0-or-later
//! Schreiben einer Austauschdatei (Vertrag R26 F1/F2).
//!
//! - [`write_xml`]: `werk.xml` im Namensraum [`crate::NAMESPACE`]; Text exakt (`\r` im Text, `\t`, `\n`, `\r` in
//!   Attributen als Zeichenreferenz; in XML 1.0 verbotene Steuerzeichen im Text als Base64 mit
//!   `kodierung="base64"`, in Attributen [`ExchangeError::Write`] mit dem Namen des Felds).
//! - [`write_manifest`][]: `MANIFEST.xml`.
//! - [`write_baruch`]: ZIP in der Reihenfolge `mimetype` (STORED), `werk.xml`, `bilder/…`, `MANIFEST.xml`.
//! - `<karte>` (ab Unterversion 2, Runde 25) steht am Ende von `<werk>`, nur wenn es mindestens eine Verbindung
//!   oder Lage gibt.
//! - Ab Unterversion 3 (Runde 27): `<satzbild/>` am Ende von `<angaben>` (nur mit mindestens einer Einstellung), in
//!   `<buch>` nach `<vita>` immer `<verlag-anschrift>` und `<druckerei-anschrift>`, danach `<umschlag>` mit den
//!   Blöcken der Anordnung (nur wenn es welche gibt).
//! - Ab Unterversion 4 (Runde 28): an `<beitrag>` (Abschnitt und Faden) `geaendert`, falls bearbeitet, und nach
//!   den übrigen Kindern die `<alte-fassung>`-Elemente (älteste zuerst); `<befunde>` mit `<befund>` und deren
//!   `<status-eintrag>` ganz am Ende von `<werk>` (nach `<karte>`), nur wenn es Befunde gibt.
//!
//! Die Ausgabe ist deterministisch: dasselbe Dokument ergibt byte-gleiches XML und ZIP (feste Zeitstempel und
//! Rechte im ZIP, Reihenfolge wie im Dokument). Zwischen Elementen steht nur Leerraum zum Lesen (Zeilenumbruch
//! und Einrückung); er gehört nie zu einem Text – Text steht immer in genau einem Element ohne Kinder.
//! - Ab Unterversion 5 (Runde 30): an `<befund>` nach dem Statusverlauf die `<loesung>`-Elemente (Attribute `zitat`,
//!   `ersatz`, `schritt`, nur nicht leere); an `<fassung>` einer Variante `variante-gruppe`, `variante`,
//!   `variante-zeile` und nach `<text>` je umgesetztem Befund `<setzt-um befund>`; an `<punkt>` `schluessel` (fester
//!   Schlüssel der ausgebenden Instanz, für Querverweise); an `<beitrag>` (Abschnitt und Faden)
//!   `grundlage-fassung`, `grundlage`, `antwort-auf`, `wiederholt`, `relay-von`, soweit gesetzt.
//! - Ab Unterversion 6 (Runde 29): `<satz/>` am Ende von `<angaben>` hinter `<satzbild/>` (nur mit Satz-Einstellungen;
//!   Attribute `uuid`, `geaendert`, `geaendert-von` und je gesetztem Feld die Wörter aus [`crate::typeset`]).
//! - Ab Unterversion 7 (Runde 31): an `<satz/>` die Register/Glossar-Felder ([`crate::typeset::R31_ATTRIBUTES`]); am
//!   Ende von `<werk>` hinter `<befunde>` die eigenen Register `<eigene-register>` und das Glossar `<glossar>` (nur,
//!   wenn vorhanden).
//! - Ab Unterversion 8 (Runde 22): an `<fassung>` `stand` (nur, wenn fortgeschrieben) und `beruht-stand` (nur, wenn
//!   größer 0), nach `<setzt-um>` je Schritt `<fortschreibung uuid seq art undo-von? autor zeit>` mit `<vorher>` und
//!   `<nachher>`; an `<befund>` und `<status-eintrag>` `stand` (nur, wenn größer 0); `<satz>` mit dem Kind
//!   `<entwurf/>`, wenn die Satz-Einstellungen einen Entwurf tragen (sonst bleibt `<satz/>` leer).
//! - Ab Unterversion 9 (Runde 34): an `<einfall>` `titel` (nur, wenn nicht leer), `eltern` (nur mit Elternknoten),
//!   `platz` (nur, wenn nicht 0) und `hauptthema="true"` (nur bei einem freien Hauptthema).
//!
//! Geschrieben wird immer die Unterversion [`crate::FORMAT_VERSION`], die dieses Programm kennt, gleich welche
//! `version` das Dokument trägt (Unbekanntes aus einer höheren Unterversion ist nicht im Modell).

use std::io::{Cursor, Write as _};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::model::*;
use crate::{ExchangeDocument, ExchangeError, ExchangeManifestEntry};

/// Bilder im XML: als Base64 in `<daten>` (nur XML) oder als Verweis `datei` auf einen ZIP-Eintrag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode {
    Inline,
    Files,
}

/// `true` für ein Zeichen, das in XML 1.0 nicht vorkommen darf (auch nicht als Zeichenreferenz):
/// Steuerzeichen unter U+0020 außer Tab, Zeilenumbruch und Wagenrücklauf sowie U+FFFE und U+FFFF.
/// Ersatzzeichen (Surrogate) gibt es in einem Rust-`str` nicht.
pub fn forbidden_in_xml(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{fffe}' | '\u{ffff}')
}

/// Text als Elementinhalt maskiert: `&`, `<`, `>` (damit nie `]]>` entsteht) und `\r` als `&#13;`, sonst
/// wörtlich (Tab und Zeilenumbruch bleiben, der Leser trimmt nie).
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

/// Attributwert in doppelten Anführungszeichen: zusätzlich `"` sowie `\t`, `\n`, `\r` als Zeichenreferenz (sonst
/// normalisierte ein Leser sie nach XML 1.0 §3.3.3 zu Leerzeichen).
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

/// Ein Attribut: Name und Wert.
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

/// Kleiner XML-Schreiber mit Einrückung; jedes Element trägt seinen Pfad für Fehlermeldungen.
struct Xml {
    out: String,
    path: Vec<&'static str>,
}

impl Xml {
    fn new() -> Self {
        Xml { out: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"), path: Vec::new() }
    }

    /// Name des Felds für Fehlermeldungen, z. B. `werk/gliederung/punkt/fassung/@autor`.
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

    /// `<name …>` mit Kindern; danach [`Xml::end`].
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

    /// Leeres Element `<name …/>`.
    fn empty(&mut self, name: &'static str, attrs: &[Attr<'_>]) -> Result<(), ExchangeError> {
        self.tag_open(name, attrs)?;
        self.out.push_str("/>");
        Ok(())
    }

    /// Textelement (Typ `Text` im XSD): exakt, bei verbotenen Zeichen als Base64 mit `kodierung="base64"`.
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

    /// Element mit schlichtem Inhalt ohne Base64 (Zeit, Base64-Daten): verbotene Zeichen → Fehler.
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

    /// Prüft eine Kennung (kanonische UUID); sonst wäre die Ausgabe nicht gültig nach dem XSD.
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

    /// Prüft einen Zeitpunkt (nicht leer, wie im XSD-Typ `Zeit`).
    fn time<'a>(&self, element: &str, attribute: &'static str, value: &'a str) -> Result<Attr<'a>, ExchangeError> {
        if value.is_empty() {
            Err(ExchangeError::Write(format!("leerer Zeitpunkt in {}", self.field(element, Some(attribute)))))
        } else {
            Ok(a(attribute, value))
        }
    }

    /// Positive Zahl (XSD `positiveInteger`).
    fn positive(&self, element: &str, attribute: &'static str, value: u32) -> Result<Attr<'static>, ExchangeError> {
        if value == 0 {
            Err(ExchangeError::Write(format!("0 ist keine gültige Zahl in {}", self.field(element, Some(attribute)))))
        } else {
            Ok(owned(attribute, value.to_string()))
        }
    }

    /// Endliche Kommazahl (XSD `double`, ohne NaN und Unendlich).
    fn double(&self, element: &str, attribute: &'static str, value: f64) -> Result<Attr<'static>, ExchangeError> {
        if value.is_finite() {
            Ok(owned(attribute, value.to_string()))
        } else {
            Err(ExchangeError::Write(format!("keine endliche Zahl in {}", self.field(element, Some(attribute)))))
        }
    }
}

/// `werk.xml` als Text.
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

/// `<eigene-register>` mit je `<register uuid kennung titel angelegt/>` in ihrer Reihenfolge (ab Unterversion 7).
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

/// `<glossar>` mit je `<begriff uuid status herkunft angelegt [geaendert] [geaendert-von]>` und den Kindern `<wort>`,
/// `<kurz>`, `<ausfuehrlich>`, `<quelle>` (nur gesetzte), je Variante `<variante>`, je Stelle `<bibelstelle>` (ab
/// Unterversion 7).
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

/// `<alte-fassung>` je alter Fassung eines Beitrags (ab Unterversion 4).
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

/// Wahlweise Kennung als Attribut.
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

/// `<befunde>` (ab Unterversion 4): je Befund Herkunft, Art, Status, Text, Zitat, Prüfvermerk, Begründung, Sammlung
/// und Statusverlauf, in der Reihenfolge des Dokuments.
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
        // Stand 0 (Fassung nie fortgeschrieben) wird nicht geschrieben: gleichbedeutend mit „kein Stand“, und Dateien
        // ohne Fortschreibung bleiben gültig nach den älteren Schemata (Integrator Runde 22).
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

/// Metadaten eines Beitrags als Attribute (ab Unterversion 5).
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

/// `<karte>` (ab Unterversion 2): erst alle Verbindungen, dann alle Lagen, je in der Reihenfolge des Dokuments.
/// Koordinaten als kürzeste Darstellung, die beim Lesen genau dieselbe Zahl ergibt (`f64` → `to_string`).
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

/// Farbe eines Umschlag-Blocks im XML: `hell`, `dunkel` oder `#rrggbb`.
pub fn cover_color_word(color: &str) -> &str {
    match color {
        "light" => "hell",
        "dark" => "dunkel",
        other => other,
    }
}

/// `<block>` der Umschlag-Anordnung (ab Unterversion 3).
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
    // Ab Unterversion 9 (Runde 34): nur, was von der Vorgabe abweicht – ein Werk ohne Baum gibt dieselben
    // Attribute aus wie zuvor.
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

/// Prüft eine Bildfassung (Typ des Formats, Prüfsumme passt zu den Bytes) und gibt den Eintrag im ZIP zurück.
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

/// `MANIFEST.xml` als Text.
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

/// Ganzer Container `.baruch` (ZIP).
///
/// Reihenfolge nach F1: `mimetype` (STORED, ohne Zeilenende), `werk.xml` (DEFLATE), je Bildfassung
/// `bilder/<fassungs-uuid>.<ext>` in der Reihenfolge des Dokuments (Raster STORED, SVG DEFLATE), zuletzt
/// `MANIFEST.xml` (DEFLATE) mit SHA-256 und Bytezahl aller Einträge außer `mimetype` und sich selbst. Alle
/// Einträge tragen den festen Zeitstempel 1980-01-01 00:00:00 und die Rechte 0644.
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
        // Tab, Zeilenumbruch und Wagenrücklauf sind erlaubt, die übrigen C0-Steuerzeichen nicht (XML 1.0 §2.2).
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
        // `>` immer maskiert, damit nie `]]>` im Inhalt steht; `\r` als Zeichenreferenz, Leerraum wörtlich.
        assert_eq!(text, " a\t&lt;&amp;&gt;]]&gt;&#13;\n ", "Elementinhalt");
        let mut attr = String::new();
        escape_attr("\"a\tb\nc\rd\"", &mut attr);
        // Im Attribut würden \t, \n und \r sonst zu Leerzeichen normalisiert.
        assert_eq!(attr, "&quot;a&#9;b&#10;c&#13;d&quot;", "Attributwert");
    }
}
