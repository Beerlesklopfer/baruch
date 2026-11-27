// SPDX-License-Identifier: AGPL-3.0-or-later
//! Baum → Modell (Vertrag R26 Abschnitt 4).
//!
//! - Elemente des Formats stehen im Namensraum [`NAMESPACE`], Attribute ohne Präfix. Die Reihenfolge der Kinder
//!   prüft der Leser nicht (das XSD tut es); fehlt ein Pflichtelement oder -attribut → [`ExchangeError::Xml`].
//! - Was der Leser nicht kennt (Elemente, Attribute, Text in einem Element ohne Text, Elemente fremder
//!   Namensräume), landet als Pfad in [`ExchangeDocument::unknown`]: `werk/angaben/farbe` für ein Element,
//!   `werk/angaben/@farbe` für ein Attribut, `werk/angaben/#text` für Text, `{ns}name` für fremde Namensräume.
//!   Je Pfad ein Eintrag; übernommen wird davon nichts.
//! - `<karte>` mit `<verbindung>` und `<lage>` (Runde 25) und die Grabsteinarten `verbindung` und `lage` gelten
//!   erst ab `version="2"`: In einer Datei der Unterversion 1 wird `<karte>` als unbekannt gemeldet und ein solcher
//!   Grabstein ist ein Fehler. Eine noch höhere Unterversion liest die Karte wie 2, meldet Unbekanntes darin und
//!   überspringt Grabsteine unbekannter Art.
//! - Ab `version="3"` (Runde 27): `<satzbild>` in `<angaben>`, in `<buch>` `<verlag-anschrift>`,
//!   `<druckerei-anschrift>` (fehlen sie, gilt leer) und `<umschlag>` mit `<block>`; Grabsteinart `umschlag-block`.
//!   In einer Datei der Unterversion 1 oder 2 werden diese Elemente als unbekannt gemeldet und nicht übernommen, ein
//!   solcher Grabstein ist ein Fehler.
//! - Ab `version="4"` (Runde 28): `<befunde>` am Ende von `<werk>` (Befunde mit `<status-eintrag>`), an `<beitrag>`
//!   (Abschnitt und Faden) `geaendert` und `<alte-fassung>`; Grabsteinarten `befund`, `befund-status`,
//!   `beitrag-fassung`. In einer Datei der Unterversionen 1 bis 3 werden sie als unbekannt gemeldet und nicht
//!   übernommen, ein solcher Grabstein ist ein Fehler.
//! - Text mit `kodierung="base64"` wird entschlüsselt (UTF-8). `<daten>` ist Base64 (Leerraum erlaubt).
//! - Ab `version="5"` (Runde 30): an `<befund>` die `<loesung>`-Elemente, an `<fassung>` die Variante
//!   (`variante-gruppe`, `variante`, `variante-zeile`, `<setzt-um befund>`), an `<beitrag>` (Abschnitt und Faden)
//!   `grundlage-fassung`, `grundlage`, `antwort-auf`, `wiederholt`, `relay-von`. In einer Datei der Unterversionen 1
//!   bis 4 werden sie als unbekannt gemeldet und nicht übernommen.
//! - Ab `version="6"` (Runde 29): `<satz>` in `<angaben>` (Satz-Einstellungen, Wörter aus [`crate::typeset`]);
//!   Grabsteinart `satz`. In einer Datei der Unterversionen 1 bis 5 wird `<satz>` als unbekannt gemeldet und nicht
//!   übernommen, ein solcher Grabstein ist ein Fehler. Halbe zusammengesetzte Felder → [`ExchangeError::Xml`].
//! - Ab `version="8"` (Runde 22): an `<fassung>` `stand`, `beruht-stand` und die `<fortschreibung>`-Kinder, in `<satz>`
//!   das Kind `<entwurf/>`, an `<befund>` und `<status-eintrag>` `stand`. In einer älteren Datei werden sie als
//!   unbekannt gemeldet und nicht übernommen; die Grabsteinart `fortschreibung` ist dort ein Fehler.
//! - Ab `version="9"` (Runde 34): an `<einfall>` `titel`, `eltern`, `platz`, `hauptthema` (Baum der Mindmap). In einer
//!   älteren Datei werden sie als unbekannt gemeldet und nicht übernommen.
//! - Ab `version="7"` (Runde 31): am Ende von `<werk>` `<eigene-register>` (je `<register>`) und `<glossar>` (je
//!   `<begriff>` mit `<wort>`, `<kurz>`, `<ausfuehrlich>`, `<quelle>`, `<variante>`, `<bibelstelle>`), an `<satz>` die
//!   Register/Glossar-Attribute ([`crate::typeset::r31_from_attrs`]); Grabsteinarten `glossar-begriff`, `register`. In
//!   einer Datei der Unterversionen 1 bis 6 werden sie als unbekannt gemeldet und nicht übernommen, ein solcher
//!   Grabstein ist ein Fehler.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::dom::Element;
use crate::ExchangeError;
use crate::model::*;

/// Verweis einer Bildfassung auf einen Eintrag im Archiv (`datei`): (Bild, Fassung, Eintrag).
#[derive(Debug, Clone)]
pub(crate) struct FileRef {
    pub asset: usize,
    pub revision: usize,
    pub entry: String,
    /// Bytezahl laut XML (`bytes`), geprüft gegen den Eintrag.
    pub bytes: u64,
}

/// Gesammelte unbekannte Pfade (ohne Doppel, in der Reihenfolge des Auftretens).
#[derive(Debug, Default)]
pub(crate) struct Unknown(Vec<String>);

impl Unknown {
    fn push(&mut self, path: String) {
        if !self.0.contains(&path) {
            self.0.push(path);
        }
    }
}

fn is_xml_space(text: &str) -> bool {
    text.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
}

/// Ein Element beim Abbilden: merkt sich, welche Kinder und Attribute gelesen wurden.
struct Cursor<'a> {
    el: &'a Element,
    path: String,
    used_children: Vec<bool>,
    used_attrs: Vec<bool>,
    text_used: bool,
}

impl<'a> Cursor<'a> {
    fn new(el: &'a Element, path: String) -> Self {
        Cursor {
            el,
            path,
            used_children: vec![false; el.children.len()],
            used_attrs: vec![false; el.attrs.len()],
            text_used: false,
        }
    }

    fn attr(&mut self, name: &str) -> Option<&'a str> {
        let index = self.el.attrs.iter().position(|a| a.ns.is_none() && a.name == name)?;
        self.used_attrs[index] = true;
        Some(self.el.attrs[index].value.as_str())
    }

    fn req_attr(&mut self, name: &str) -> Result<&'a str, ExchangeError> {
        self.attr(name).ok_or_else(|| ExchangeError::Xml(format!("Pflichtattribut fehlt: {}/@{name}", self.path)))
    }

    fn child(&mut self, name: &str) -> Option<Cursor<'a>> {
        let index = self
            .el
            .children
            .iter()
            .enumerate()
            .position(|(i, c)| !self.used_children[i] && c.ns.as_deref() == Some(NAMESPACE) && c.name == name)?;
        self.used_children[index] = true;
        Some(Cursor::new(&self.el.children[index], format!("{}/{name}", self.path)))
    }

    fn req_child(&mut self, name: &str) -> Result<Cursor<'a>, ExchangeError> {
        self.child(name).ok_or_else(|| ExchangeError::Xml(format!("Pflichtelement fehlt: {}/{name}", self.path)))
    }

    fn children(&mut self, name: &str) -> Vec<Cursor<'a>> {
        let mut out = Vec::new();
        while let Some(child) = self.child(name) {
            out.push(child);
        }
        out
    }

    /// Unbenutztes melden.
    fn finish(self, unknown: &mut Unknown) {
        for (attr, used) in self.el.attrs.iter().zip(&self.used_attrs) {
            if !used {
                match &attr.ns {
                    None => unknown.push(format!("{}/@{}", self.path, attr.name)),
                    Some(ns) => unknown.push(format!("{}/@{{{ns}}}{}", self.path, attr.name)),
                }
            }
        }
        for (child, used) in self.el.children.iter().zip(&self.used_children) {
            if !used {
                match child.ns.as_deref() {
                    Some(NAMESPACE) => unknown.push(format!("{}/{}", self.path, child.name)),
                    Some(ns) => unknown.push(format!("{}/{{{ns}}}{}", self.path, child.name)),
                    None => unknown.push(format!("{}/{{}}{}", self.path, child.name)),
                }
            }
        }
        if !self.text_used && !is_xml_space(&self.el.text) {
            unknown.push(format!("{}/#text", self.path));
        }
    }

    /// Text des Elements (Typ `Text`): exakt, mit `kodierung="base64"` entschlüsselt.
    fn text(&mut self) -> Result<String, ExchangeError> {
        self.text_used = true;
        match self.attr("kodierung") {
            None => Ok(self.el.text.clone()),
            Some("base64") => {
                let compact: String = self.el.text.chars().filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r')).collect();
                let bytes = STANDARD
                    .decode(compact.as_bytes())
                    .map_err(|e| ExchangeError::Xml(format!("{}: kein gültiges Base64 ({e})", self.path)))?;
                String::from_utf8(bytes)
                    .map_err(|_| ExchangeError::Xml(format!("{}: Base64 ergibt kein UTF-8", self.path)))
            }
            Some(other) => Err(ExchangeError::Xml(format!("{}: unbekannte Kodierung „{other}“", self.path))),
        }
    }

    /// Bytes aus `xs:base64Binary` (Leerraum erlaubt).
    fn base64(&mut self) -> Result<Vec<u8>, ExchangeError> {
        self.text_used = true;
        let compact: String = self.el.text.chars().filter(|c| !matches!(c, ' ' | '\t' | '\n' | '\r')).collect();
        STANDARD
            .decode(compact.as_bytes())
            .map_err(|e| ExchangeError::Xml(format!("{}: kein gültiges Base64 ({e})", self.path)))
    }
}

/// Text eines Pflicht-Kindelements.
fn req_text(parent: &mut Cursor<'_>, name: &str, unknown: &mut Unknown) -> Result<String, ExchangeError> {
    let mut child = parent.req_child(name)?;
    let text = child.text()?;
    child.finish(unknown);
    Ok(text)
}

/// Text eines optionalen Kindelements.
fn opt_text(parent: &mut Cursor<'_>, name: &str, unknown: &mut Unknown) -> Result<Option<String>, ExchangeError> {
    match parent.child(name) {
        Some(mut child) => {
            let text = child.text()?;
            child.finish(unknown);
            Ok(Some(text))
        }
        None => Ok(None),
    }
}

/// Texte aller Kindelemente dieses Namens.
fn all_texts(parent: &mut Cursor<'_>, name: &str, unknown: &mut Unknown) -> Result<Vec<String>, ExchangeError> {
    let mut out = Vec::new();
    for mut child in parent.children(name) {
        out.push(child.text()?);
        child.finish(unknown);
    }
    Ok(out)
}

fn parse_bool(value: &str, path: &str) -> Result<bool, ExchangeError> {
    match value.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(ExchangeError::Xml(format!("{path}: „{other}“ ist kein Wahrheitswert (true|false)"))),
    }
}

fn parse_u32(value: &str, path: &str) -> Result<u32, ExchangeError> {
    value
        .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
        .parse()
        .map_err(|_| ExchangeError::Xml(format!("{path}: „{value}“ ist keine ganze Zahl")))
}

fn parse_u64(value: &str, path: &str) -> Result<u64, ExchangeError> {
    value
        .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
        .parse()
        .map_err(|_| ExchangeError::Xml(format!("{path}: „{value}“ ist keine ganze Zahl")))
}

fn parse_f64(value: &str, path: &str) -> Result<f64, ExchangeError> {
    let number: f64 = value
        .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
        .parse()
        .map_err(|_| ExchangeError::Xml(format!("{path}: „{value}“ ist keine Zahl")))?;
    if number.is_finite() {
        Ok(number)
    } else {
        Err(ExchangeError::Xml(format!("{path}: „{value}“ ist keine endliche Zahl")))
    }
}

/// Aufzählungswert über `from_xml`; unbekannt → `Xml` mit Pfad.
fn word<T>(value: &str, path: &str, from: fn(&str) -> Option<T>) -> Result<T, ExchangeError> {
    from(value).ok_or_else(|| ExchangeError::Xml(format!("{path}: unbekannter Wert „{value}“")))
}

fn attr_path(c: &Cursor<'_>, name: &str) -> String {
    format!("{}/@{name}", c.path)
}

/// `<werk>` → Dokument; dazu die Verweise der Bildfassungen auf Archiv-Einträge.
pub(crate) fn document(root: &Element) -> Result<(ExchangeDocument, Vec<FileRef>), ExchangeError> {
    check_root(root, "werk")?;
    let mut unknown = Unknown::default();
    let mut c = Cursor::new(root, "werk".into());
    let version = version_of(&mut c)?;
    let uuid = c.req_attr("uuid")?.to_owned();
    let kind_path = attr_path(&c, "art");
    let kind = word(c.req_attr("art")?, &kind_path, ExchangeWorkKind::from_xml)?;

    let mut out = c.req_child("ausgabe")?;
    let exported = out.req_attr("zeit")?.to_owned();
    let instance = ExchangeInstance {
        uuid: out.req_attr("instanz")?.to_owned(),
        name: crate::clean_instance_name(out.req_attr("instanzname")?),
    };
    let generator = out.req_attr("programm")?.to_owned();
    out.finish(&mut unknown);

    let mut work = ExchangeWork { uuid, kind, ..ExchangeWork::default() };
    {
        let mut details = c.req_child("angaben")?;
        angaben(&mut details, &mut work, version, &mut unknown)?;
        details.finish(&mut unknown);
    }
    if let Some(mut book) = c.child("buch") {
        work.book = Some(buch(&mut book, version, &mut unknown)?);
        book.finish(&mut unknown);
    }
    {
        let mut template = c.req_child("vorlage")?;
        work.template.current = req_text(&mut template, "text", &mut unknown)?;
        for mut revision in template.children("fassung") {
            let number_path = attr_path(&revision, "nummer");
            work.template.revisions.push(ExchangeTemplateRevision {
                uuid: revision.req_attr("uuid")?.to_owned(),
                number: parse_u32(revision.req_attr("nummer")?, &number_path)?,
                author: revision.req_attr("autor")?.to_owned(),
                created: revision.req_attr("angelegt")?.to_owned(),
                note: opt_text(&mut revision, "notiz", &mut unknown)?.unwrap_or_default(),
                body: req_text(&mut revision, "text", &mut unknown)?,
            });
            revision.finish(&mut unknown);
        }
        template.finish(&mut unknown);
    }
    {
        let mut outline = c.req_child("gliederung")?;
        for mut node in outline.children("punkt") {
            work.outline.push(punkt(&mut node, version, &mut unknown)?);
            node.finish(&mut unknown);
        }
        outline.finish(&mut unknown);
    }
    if let Some(mut ideas) = c.child("einfaelle") {
        for mut idea in ideas.children("einfall") {
            work.ideas.push(einfall(&mut idea, version, &mut unknown)?);
            idea.finish(&mut unknown);
        }
        ideas.finish(&mut unknown);
    }
    if let Some(mut threads) = c.child("faeden") {
        for mut thread in threads.children("faden") {
            work.threads.push(faden(&mut thread, version, &mut unknown)?);
            thread.finish(&mut unknown);
        }
        threads.finish(&mut unknown);
    }
    let mut refs = Vec::new();
    if let Some(mut assets) = c.child("bilder") {
        for mut asset in assets.children("bild") {
            let index = work.assets.len();
            work.assets.push(bild(&mut asset, index, &mut refs, &mut unknown)?);
            asset.finish(&mut unknown);
        }
        assets.finish(&mut unknown);
    }
    if let Some(mut deleted) = c.child("geloescht") {
        for mut entry in deleted.children("eintrag") {
            let kind_path = attr_path(&entry, "art");
            let uuid = entry.req_attr("uuid")?.to_owned();
            let raw_kind = entry.req_attr("art")?.to_owned();
            let deleted_at = entry.req_attr("zeit")?.to_owned();
            // Eine neuere Unterversion darf neue Arten von Grabsteinen haben (etwa Verbindungen der Karte ab
            // R25): melden und überspringen statt die ganze Datei abzulehnen (Austauschformat.md §6).
            let kind = match ExchangeTombstoneKind::from_xml(&raw_kind) {
                // Eine Art, die es in der Unterversion der Datei noch nicht gab, ist ein Fehler wie ein fremder Wert.
                Some(kind) if kind.since_version() > version => {
                    return Err(ExchangeError::Xml(format!(
                        "{kind_path}: „{raw_kind}“ gibt es erst ab Unterversion {}, die Datei hat {version}",
                        kind.since_version()
                    )));
                }
                Some(kind) => kind,
                None if version > FORMAT_VERSION => {
                    unknown.push(format!("{kind_path}=„{raw_kind}“"));
                    entry.finish(&mut unknown);
                    continue;
                }
                None => word(&raw_kind, &kind_path, ExchangeTombstoneKind::from_xml)?,
            };
            work.deleted.push(ExchangeTombstone { uuid, kind, deleted: deleted_at });
            entry.finish(&mut unknown);
        }
        deleted.finish(&mut unknown);
    }
    // <karte> erst ab Unterversion 2; in Unterversion 1 bleibt es unbenutzt und wird so als unbekannt gemeldet.
    if version >= 2
        && let Some(mut map) = c.child("karte")
    {
        work.map = karte(&mut map, &mut unknown)?;
        map.finish(&mut unknown);
    }
    // <befunde> erst ab Unterversion 4.
    if version >= 4
        && let Some(mut findings) = c.child("befunde")
    {
        for mut finding in findings.children("befund") {
            work.findings.push(befund(&mut finding, version, &mut unknown)?);
            finding.finish(&mut unknown);
        }
        findings.finish(&mut unknown);
    }
    // <eigene-register> und <glossar> erst ab Unterversion 7.
    if version >= 7
        && let Some(mut registers) = c.child("eigene-register")
    {
        for mut register in registers.children("register") {
            work.registers.push(ExchangeRegister {
                uuid: register.req_attr("uuid")?.to_owned(),
                slug: register.req_attr("kennung")?.to_owned(),
                title: register.req_attr("titel")?.to_owned(),
                created: register.req_attr("angelegt")?.to_owned(),
            });
            register.finish(&mut unknown);
        }
        registers.finish(&mut unknown);
    }
    if version >= 7
        && let Some(mut glossary) = c.child("glossar")
    {
        for mut term in glossary.children("begriff") {
            work.glossary.push(begriff(&mut term, &mut unknown)?);
            term.finish(&mut unknown);
        }
        glossary.finish(&mut unknown);
    }
    c.finish(&mut unknown);
    let doc = ExchangeDocument { version, exported, instance, generator, work, unknown: unknown.0 };
    Ok((doc, refs))
}

/// `<begriff>` → Glossarbegriff (ab Unterversion 7).
fn begriff(c: &mut Cursor<'_>, unknown: &mut Unknown) -> Result<ExchangeGlossaryTerm, ExchangeError> {
    let status_path = attr_path(c, "status");
    Ok(ExchangeGlossaryTerm {
        uuid: c.req_attr("uuid")?.to_owned(),
        status: word(c.req_attr("status")?, &status_path, ExchangeGlossaryStatus::from_xml)?,
        origin: c.req_attr("herkunft")?.to_owned(),
        created: c.req_attr("angelegt")?.to_owned(),
        updated: c.attr("geaendert").map(str::to_owned),
        updated_by: c.attr("geaendert-von").unwrap_or_default().to_owned(),
        term: req_text(c, "wort", unknown)?,
        short: opt_text(c, "kurz", unknown)?.unwrap_or_default(),
        long: opt_text(c, "ausfuehrlich", unknown)?.unwrap_or_default(),
        source: opt_text(c, "quelle", unknown)?.unwrap_or_default(),
        variants: all_texts(c, "variante", unknown)?,
        bible_refs: all_texts(c, "bibelstelle", unknown)?,
    })
}

/// `<manifest>` → Einträge.
pub(crate) fn manifest(root: &Element) -> Result<Vec<ExchangeManifestEntry>, ExchangeError> {
    check_root(root, "manifest")?;
    let mut unknown = Unknown::default();
    let mut c = Cursor::new(root, "manifest".into());
    version_of(&mut c)?;
    let mut entries = Vec::new();
    for mut file in c.children("datei") {
        let bytes_path = attr_path(&file, "bytes");
        entries.push(ExchangeManifestEntry {
            path: file.req_attr("pfad")?.to_owned(),
            sha256: file.req_attr("sha256")?.to_owned(),
            bytes: parse_u64(file.req_attr("bytes")?, &bytes_path)?,
        });
        file.finish(&mut unknown);
    }
    c.finish(&mut unknown);
    if entries.is_empty() {
        return Err(ExchangeError::Xml("Pflichtelement fehlt: manifest/datei".into()));
    }
    Ok(entries)
}

/// Wurzel prüfen: Name und Namensraum. Ein anderer Namensraum des Formats (`urn:baruch:…`) ist eine andere
/// Hauptversion ([`ExchangeError::Version`]), alles andere keine Austauschdatei.
fn check_root(root: &Element, name: &str) -> Result<(), ExchangeError> {
    let ns = root.ns.as_deref();
    if root.name == name && ns == Some(NAMESPACE) {
        return Ok(());
    }
    if root.name == name
        && let Some(ns) = ns
        && ns.starts_with("urn:baruch:")
    {
        let found = root
            .attrs
            .iter()
            .find(|a| a.ns.is_none() && a.name == "version")
            .map_or_else(|| "?".to_owned(), |a| a.value.clone());
        return Err(ExchangeError::Version { found, namespace: ns.to_owned() });
    }
    Err(ExchangeError::NotExchange(match ns {
        Some(ns) => format!("Wurzel „{}“ im Namensraum „{ns}“ statt „{name}“ in {NAMESPACE}", root.name),
        None => format!("Wurzel „{}“ ohne Namensraum statt „{name}“ in {NAMESPACE}", root.name),
    }))
}

/// Unterversion (`version`, ganze Zahl ab 1). Höhere Unterversionen liest der Leser (verträglich).
fn version_of(c: &mut Cursor<'_>) -> Result<u32, ExchangeError> {
    let raw = c
        .req_attr("version")
        .map_err(|_| ExchangeError::Version { found: "fehlt".into(), namespace: NAMESPACE.into() })?;
    match raw.trim().parse::<u32>() {
        Ok(v) if v >= 1 => Ok(v),
        _ => Err(ExchangeError::Version { found: raw.to_owned(), namespace: NAMESPACE.into() }),
    }
}

fn angaben(
    c: &mut Cursor<'_>,
    work: &mut ExchangeWork,
    version: u32,
    unknown: &mut Unknown,
) -> Result<(), ExchangeError> {
    // Sub-version 10: work language; a file without the attribute (up to sub-version 9) is German.
    let language_path = attr_path(c, "sprache");
    if let Some(raw) = c.attr("sprache") {
        work.language = word(raw, &language_path, ExchangeLanguage::from_xml)?;
    }
    work.title = req_text(c, "titel", unknown)?;
    work.subtitle = req_text(c, "untertitel", unknown)?;
    work.author = req_text(c, "autor", unknown)?;
    work.audience = req_text(c, "zielgruppe", unknown)?;
    work.bible = req_text(c, "bibel", unknown)?;
    work.created = req_text(c, "angelegt", unknown)?;
    work.updated = opt_text(c, "geaendert", unknown)?;
    {
        let mut scheme = c.req_child("schema")?;
        work.numbering_scheme = scheme.req_attr("name")?.to_owned();
        work.numbering = opt_text(&mut scheme, "muster", unknown)?;
        scheme.finish(unknown);
    }
    work.bible_module = opt_text(c, "bibelmodul", unknown)?;
    if let Some(mut sources) = c.child("quellen") {
        for mut source in sources.children("quelle") {
            let kind_path = attr_path(&source, "art");
            let kind = word(source.req_attr("art")?, &kind_path, ExchangeSourceKind::from_xml)?;
            let module = source.text()?;
            work.sword_sources.push(ExchangeSwordSource { kind, module });
            source.finish(unknown);
        }
        sources.finish(unknown);
    }
    if let Some(mut timing) = c.child("sprechdauer") {
        let wpm_path = attr_path(&timing, "woerter-je-minute");
        let target_path = attr_path(&timing, "ziel-sekunden");
        let words_per_minute = parse_u32(timing.req_attr("woerter-je-minute")?, &wpm_path)?;
        let target_seconds = timing.attr("ziel-sekunden").map(|v| parse_u32(v, &target_path)).transpose()?;
        work.timing = Some(ExchangeTiming { words_per_minute, target_seconds });
        timing.finish(unknown);
    }
    // <satzbild> erst ab Unterversion 3; davor bleibt es unbenutzt und wird als unbekannt gemeldet.
    if version >= 3
        && let Some(mut sheet) = c.child("satzbild")
    {
        let mut field = |name: &str| sheet.attr(name).map(str::to_owned);
        let read = ExchangeSheet {
            font: field("schrift"),
            heading_font: field("schrift-ueberschriften"),
            size: field("groesse"),
            fill: field("farbe"),
            heading_fill: field("farbe-ueberschriften"),
            background: field("blattfarbe"),
            width: field("satzbreite"),
        };
        sheet.finish(unknown);
        work.sheet = Some(read).filter(|s| !s.is_empty());
    }
    // <satz> erst ab Unterversion 6; davor bleibt es unbenutzt und wird als unbekannt gemeldet.
    if version >= 6
        && let Some(mut typeset) = c.child("satz")
    {
        let path = typeset.path.clone();
        let uuid = typeset.req_attr("uuid")?.to_owned();
        let updated = typeset.req_attr("geaendert")?.to_owned();
        let updated_by = typeset.req_attr("geaendert-von")?.to_owned();
        let mut settings = crate::typeset::settings_from_attrs(|name| typeset.attr(name))
            .map_err(|e| ExchangeError::Xml(format!("{path}/{e}")))?;
        // Register/Glossar-Felder erst ab Unterversion 7; davor bleiben sie ungelesen und werden als unbekannt gemeldet.
        if version >= 7 {
            crate::typeset::r31_from_attrs(&mut settings, |name| typeset.attr(name))
                .map_err(|e| ExchangeError::Xml(format!("{path}/{e}")))?;
        }
        // Entwurf erst ab Unterversion 8; davor bleibt <entwurf> ungelesen und wird als unbekannt gemeldet.
        if version >= 8
            && let Some(mut draft) = typeset.child("entwurf")
        {
            let draft_path = draft.path.clone();
            settings.draft = Some(
                crate::typeset::draft_from_attrs(|name| draft.attr(name))
                    .map_err(|e| ExchangeError::Xml(format!("{draft_path}/{e}")))?,
            );
            draft.finish(unknown);
        }
        typeset.finish(unknown);
        work.typeset = Some(ExchangeTypeset { uuid, updated, updated_by, settings });
    }
    Ok(())
}

/// Farbe eines Umschlag-Blocks aus dem XML (`hell`, `dunkel`, `#rrggbb`) in die gespeicherte Form.
fn cover_color(value: &str, path: &str) -> Result<String, ExchangeError> {
    match value {
        "hell" => Ok("light".into()),
        "dunkel" => Ok("dark".into()),
        hex if hex.len() == 7
            && hex.starts_with('#')
            && hex[1..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) =>
        {
            Ok(hex.to_owned())
        }
        other => Err(ExchangeError::Xml(format!("{path}: unbekannte Farbe „{other}“ (hell, dunkel oder #rrggbb)"))),
    }
}

/// `<block>` der Umschlag-Anordnung (ab Unterversion 3).
fn block(c: &mut Cursor<'_>, unknown: &mut Unknown) -> Result<ExchangeCoverBlock, ExchangeError> {
    let path = |c: &Cursor<'_>, name: &str| attr_path(c, name);
    let (kind_p, x_p, y_p, w_p, align_p, size_p, lead_p, color_p, style_p, back_p, vis_p, dir_p) = (
        path(c, "art"),
        path(c, "x-mm"),
        path(c, "y-mm"),
        path(c, "breite-mm"),
        path(c, "ausrichtung"),
        path(c, "groesse-pt"),
        path(c, "zeilenabstand-em"),
        path(c, "farbe"),
        path(c, "schnitt"),
        path(c, "hintergrund"),
        path(c, "sichtbar"),
        path(c, "richtung"),
    );
    Ok(ExchangeCoverBlock {
        uuid: c.req_attr("uuid")?.to_owned(),
        kind: word(c.req_attr("art")?, &kind_p, ExchangeCoverBlockKind::from_xml)?,
        x_mm: parse_f64(c.req_attr("x-mm")?, &x_p)?,
        y_mm: parse_f64(c.req_attr("y-mm")?, &y_p)?,
        width_mm: parse_f64(c.req_attr("breite-mm")?, &w_p)?,
        align: word(c.req_attr("ausrichtung")?, &align_p, ExchangeCoverAlign::from_xml)?,
        size_pt: parse_f64(c.req_attr("groesse-pt")?, &size_p)?,
        leading_em: parse_f64(c.req_attr("zeilenabstand-em")?, &lead_p)?,
        color: cover_color(c.req_attr("farbe")?, &color_p)?,
        font: c.attr("schrift").map(str::to_owned),
        style: word(c.req_attr("schnitt")?, &style_p, ExchangeFontStyle::from_xml)?,
        backdrop: word(c.req_attr("hintergrund")?, &back_p, ExchangeBackdrop::from_xml)?,
        visible: parse_bool(c.req_attr("sichtbar")?, &vis_p)?,
        direction: c.attr("richtung").map(|d| word(d, &dir_p, ExchangeSpineDirection::from_xml)).transpose()?,
        updated: c.req_attr("geaendert")?.to_owned(),
        text: opt_text(c, "text", unknown)?,
    })
}

fn buch(c: &mut Cursor<'_>, version: u32, unknown: &mut Unknown) -> Result<ExchangeBook, ExchangeError> {
    let mut book = buch_v1(c, unknown)?;
    // Ab Unterversion 3: Anschriften (fehlen sie, gelten sie als leer) und die Umschlag-Anordnung.
    if version >= 3 {
        book.publisher_address = opt_text(c, "verlag-anschrift", unknown)?.unwrap_or_default();
        book.printer_address = opt_text(c, "druckerei-anschrift", unknown)?.unwrap_or_default();
        if let Some(mut cover) = c.child("umschlag") {
            for mut entry in cover.children("block") {
                book.cover_blocks.push(block(&mut entry, unknown)?);
                entry.finish(unknown);
            }
            cover.finish(unknown);
        }
    }
    Ok(book)
}

fn buch_v1(c: &mut Cursor<'_>, unknown: &mut Unknown) -> Result<ExchangeBook, ExchangeError> {
    let area_path = attr_path(c, "titelbereich");
    let front_path = attr_path(c, "titelei");
    let pages_path = attr_path(c, "umschlagseiten");
    let paper_path = attr_path(c, "papier-mm");
    let bleed_path = attr_path(c, "beschnitt-mm");
    Ok(ExchangeBook {
        title_area: word(c.req_attr("titelbereich")?, &area_path, ExchangeTitleArea::from_xml)?,
        front_matter: parse_bool(c.req_attr("titelei")?, &front_path)?,
        cover_pages: parse_bool(c.req_attr("umschlagseiten")?, &pages_path)?,
        paper_mm: parse_f64(c.req_attr("papier-mm")?, &paper_path)?,
        bleed_mm: parse_f64(c.req_attr("beschnitt-mm")?, &bleed_path)?,
        cover: c.attr("umschlag").map(str::to_owned),
        author_photo: c.attr("autorenfoto").map(str::to_owned),
        updated: c.attr("geaendert").map(str::to_owned),
        publisher: req_text(c, "verlag", unknown)?,
        place: req_text(c, "ort", unknown)?,
        year: req_text(c, "jahr", unknown)?,
        edition: req_text(c, "auflage", unknown)?,
        isbn: req_text(c, "isbn", unknown)?,
        price: req_text(c, "preis", unknown)?,
        series: req_text(c, "reihe", unknown)?,
        printer: req_text(c, "druckerei", unknown)?,
        bible_notice: req_text(c, "bibelvermerk", unknown)?,
        image_credit: req_text(c, "bildnachweis", unknown)?,
        dedication: req_text(c, "widmung", unknown)?,
        blurb: req_text(c, "klappentext", unknown)?,
        bio: req_text(c, "vita", unknown)?,
        ..ExchangeBook::default()
    })
}

fn punkt(c: &mut Cursor<'_>, version: u32, unknown: &mut Unknown) -> Result<ExchangeNode, ExchangeError> {
    let kind_path = attr_path(c, "art");
    let numbered_path = attr_path(c, "nummeriert");
    let mut node = ExchangeNode {
        uuid: c.req_attr("uuid")?.to_owned(),
        kind: word(c.req_attr("art")?, &kind_path, ExchangeNodeKind::from_xml)?,
        numbered: parse_bool(c.req_attr("nummeriert")?, &numbered_path)?,
        title: req_text(c, "titel", unknown)?,
        ..ExchangeNode::default()
    };
    // Ab Unterversion 5: fester Schlüssel der ausgebenden Instanz (für Querverweise).
    if version >= 5
        && let Some(key) = c.attr("schluessel")
    {
        let path = attr_path(c, "schluessel");
        node.key = Some(i64::from(parse_u32(key, &path)?));
    }
    for mut revision in c.children("fassung") {
        node.revisions.push(fassung(&mut revision, version, unknown)?);
        revision.finish(unknown);
    }
    if node.revisions.is_empty() {
        return Err(ExchangeError::Xml(format!("Pflichtelement fehlt: {}/fassung", c.path)));
    }
    for mut post in c.children("beitrag") {
        node.posts.push(beitrag(&mut post, version, unknown)?);
        post.finish(unknown);
    }
    for mut child in c.children("punkt") {
        node.children.push(punkt(&mut child, version, unknown)?);
        child.finish(unknown);
    }
    Ok(node)
}

fn fassung(c: &mut Cursor<'_>, version: u32, unknown: &mut Unknown) -> Result<ExchangeRevision, ExchangeError> {
    let number_path = attr_path(c, "nummer");
    let status_path = attr_path(c, "status");
    let author_kind = c.req_attr("autor-art")?;
    let name = c.req_attr("autor")?.to_owned();
    let model = c.attr("modell").map(str::to_owned);
    let author = match author_kind {
        "mensch" => ExchangeAuthor::Human { name },
        "assistent" => ExchangeAuthor::Assistant { persona: name, model: model.unwrap_or_default() },
        other => return Err(ExchangeError::Xml(format!("{}/@autor-art: unbekannter Wert „{other}“", c.path))),
    };
    let mut revision = ExchangeRevision {
        uuid: c.req_attr("uuid")?.to_owned(),
        number: parse_u32(c.req_attr("nummer")?, &number_path)?,
        status: word(c.req_attr("status")?, &status_path, ExchangeRevisionStatus::from_xml)?,
        author,
        based_on: c.attr("basis").map(str::to_owned),
        created: c.req_attr("angelegt")?.to_owned(),
        edited: c.attr("geaendert").map(str::to_owned),
        note: opt_text(c, "notiz", unknown)?.unwrap_or_default(),
        body: req_text(c, "text", unknown)?,
        variant: None,
        ..ExchangeRevision::default()
    };
    // Ab Unterversion 5: Variante (Gruppe und Etikett nur zusammen, sonst Fehler).
    if version >= 5 {
        let group = c.attr("variante-gruppe");
        let label = c.attr("variante");
        let summary = c.attr("variante-zeile").unwrap_or("").to_owned();
        let mut findings = Vec::new();
        for mut target in c.children("setzt-um") {
            findings.push(target.req_attr("befund")?.to_owned());
            target.finish(unknown);
        }
        revision.variant = match (group, label) {
            (Some(group), Some(label)) => {
                Some(ExchangeVariant { group: group.to_owned(), label: label.to_owned(), summary, findings })
            }
            (None, None) if findings.is_empty() && summary.is_empty() => None,
            _ => {
                return Err(ExchangeError::Xml(format!(
                    "{}: variante-gruppe und variante stehen nur zusammen (mit variante-zeile und setzt-um)",
                    c.path
                )));
            }
        };
    }
    // Ab Unterversion 8: Stand, Stand der Basis und der Verlauf der Fortschreibungen.
    if version >= 8 {
        if let Some(stand) = c.attr("stand") {
            revision.amend_seq = parse_u32(stand, &attr_path(c, "stand"))?;
        }
        if let Some(stand) = c.attr("beruht-stand") {
            revision.based_on_amend = Some(parse_u32(stand, &attr_path(c, "beruht-stand"))?);
        }
        for mut step in c.children("fortschreibung") {
            let seq_path = attr_path(&step, "seq");
            let kind_path = attr_path(&step, "art");
            let undo_path = attr_path(&step, "undo-von");
            revision.amends.push(ExchangeAmend {
                uuid: step.req_attr("uuid")?.to_owned(),
                seq: parse_u32(step.req_attr("seq")?, &seq_path)?,
                kind: word(step.req_attr("art")?, &kind_path, ExchangeAmendKind::from_xml)?,
                undo_of: step.attr("undo-von").map(|v| parse_u32(v, &undo_path)).transpose()?,
                author: step.req_attr("autor")?.to_owned(),
                created: step.req_attr("zeit")?.to_owned(),
                before: req_text(&mut step, "vorher", unknown)?,
                after: req_text(&mut step, "nachher", unknown)?,
            });
            step.finish(unknown);
        }
    }
    Ok(revision)
}

/// Ab Unterversion 5: Metadaten eines Beitrags bzw. Fadenbeitrags.
fn post_meta(c: &mut Cursor<'_>, version: u32) -> ExchangePostMeta {
    if version < 5 {
        return ExchangePostMeta::default();
    }
    ExchangePostMeta {
        basis_revision: c.attr("grundlage-fassung").map(str::to_owned),
        basis_label: c.attr("grundlage").unwrap_or("").to_owned(),
        reply_to: c.attr("antwort-auf").map(str::to_owned),
        repeat_of: c.attr("wiederholt").map(str::to_owned),
        relay_from: c.attr("relay-von").map(str::to_owned),
    }
}

/// Ab Unterversion 4: `geaendert` und die `<alte-fassung>`-Elemente eines Beitrags.
fn edits(
    c: &mut Cursor<'_>,
    version: u32,
    unknown: &mut Unknown,
) -> Result<(Option<String>, Vec<ExchangePostVersion>), ExchangeError> {
    if version < 4 {
        return Ok((None, Vec::new()));
    }
    let edited = c.attr("geaendert").map(str::to_owned);
    let mut versions = Vec::new();
    for mut old in c.children("alte-fassung") {
        versions.push(ExchangePostVersion {
            uuid: old.req_attr("uuid")?.to_owned(),
            valid_from: old.req_attr("gueltig-ab")?.to_owned(),
            replaced: old.req_attr("ersetzt")?.to_owned(),
            edited_by: old.req_attr("bearbeitet-von")?.to_owned(),
            body: req_text(&mut old, "text", unknown)?,
        });
        old.finish(unknown);
    }
    Ok((edited, versions))
}

fn beitrag(c: &mut Cursor<'_>, version: u32, unknown: &mut Unknown) -> Result<ExchangePost, ExchangeError> {
    let position_path = attr_path(c, "position");
    let mut post = ExchangePost {
        uuid: c.req_attr("uuid")?.to_owned(),
        position: parse_u32(c.req_attr("position")?, &position_path)?,
        author: c.req_attr("autor")?.to_owned(),
        created: c.req_attr("angelegt")?.to_owned(),
        body: req_text(c, "text", unknown)?,
        sources: all_texts(c, "quelle", unknown)?,
        ..ExchangePost::default()
    };
    (post.edited, post.versions) = edits(c, version, unknown)?;
    post.meta = post_meta(c, version);
    for mut reference in c.children("bezug") {
        post.refs.push(ExchangePostRef {
            node: reference.req_attr("punkt")?.to_owned(),
            revision: reference.attr("fassung").map(str::to_owned),
            quote: reference.text()?,
        });
        reference.finish(unknown);
    }
    Ok(post)
}

fn einfall(c: &mut Cursor<'_>, version: u32, unknown: &mut Unknown) -> Result<ExchangeIdea, ExchangeError> {
    let status_path = attr_path(c, "status");
    let use_path = attr_path(c, "erledigt-als");
    let origin_path = attr_path(c, "herkunft-art");
    // Ab Unterversion 9 (Runde 34): Baum der Mindmap. In einer älteren Datei bleiben die Attribute ungelesen und
    // werden als unbekannt gemeldet.
    let (mut title, mut parent, mut position, mut main_topic) = (String::new(), None, 0i64, false);
    if version >= 9 {
        title = c.attr("titel").unwrap_or_default().to_owned();
        parent = c.attr("eltern").map(str::to_owned);
        if let Some(value) = c.attr("platz") {
            let path = attr_path(c, "platz");
            position = value
                .trim()
                .parse::<i64>()
                .map_err(|_| ExchangeError::Xml(format!("{path}: ganze Zahl erwartet, nicht „{value}“")))?;
        }
        if let Some(value) = c.attr("hauptthema") {
            main_topic = parse_bool(value, &attr_path(c, "hauptthema"))?;
        }
    }
    Ok(ExchangeIdea {
        title,
        parent,
        position,
        main_topic,
        uuid: c.req_attr("uuid")?.to_owned(),
        status: word(c.req_attr("status")?, &status_path, ExchangeIdeaStatus::from_xml)?,
        done_as: c.attr("erledigt-als").map(|v| word(v, &use_path, ExchangeIdeaUse::from_xml)).transpose()?,
        origin_kind: word(c.req_attr("herkunft-art")?, &origin_path, ExchangeOriginKind::from_xml)?,
        origin_name: c.req_attr("herkunft")?.to_owned(),
        created: c.req_attr("angelegt")?.to_owned(),
        updated: c.req_attr("geaendert")?.to_owned(),
        deleted: c.attr("geloescht").map(str::to_owned),
        node: c.attr("bezug").map(str::to_owned),
        words: all_texts(c, "schlagwort", unknown)?,
        note: req_text(c, "notiz", unknown)?,
        quote: req_text(c, "zitat", unknown)?,
        source: req_text(c, "quelle", unknown)?,
        bible: req_text(c, "bibelstelle", unknown)?,
        done_ref: opt_text(c, "erledigt-ref", unknown)?,
    })
}

fn faden(c: &mut Cursor<'_>, version: u32, unknown: &mut Unknown) -> Result<ExchangeThread, ExchangeError> {
    let mut thread = ExchangeThread {
        uuid: c.req_attr("uuid")?.to_owned(),
        created_by: c.req_attr("angelegt-von")?.to_owned(),
        created: c.req_attr("angelegt")?.to_owned(),
        updated: c.req_attr("geaendert")?.to_owned(),
        title: req_text(c, "titel", unknown)?,
        posts: Vec::new(),
    };
    for mut post in c.children("beitrag") {
        let position_path = attr_path(&post, "position");
        let mut read = ExchangeThreadPost {
            uuid: post.req_attr("uuid")?.to_owned(),
            position: parse_u32(post.req_attr("position")?, &position_path)?,
            author: post.req_attr("autor")?.to_owned(),
            created: post.req_attr("angelegt")?.to_owned(),
            body: req_text(&mut post, "text", unknown)?,
            sources: all_texts(&mut post, "quelle", unknown)?,
            ..ExchangeThreadPost::default()
        };
        (read.edited, read.versions) = edits(&mut post, version, unknown)?;
        read.meta = post_meta(&mut post, version);
        thread.posts.push(read);
        post.finish(unknown);
    }
    Ok(thread)
}

fn bild(
    c: &mut Cursor<'_>,
    index: usize,
    refs: &mut Vec<FileRef>,
    unknown: &mut Unknown,
) -> Result<ExchangeAsset, ExchangeError> {
    let mut asset = ExchangeAsset {
        uuid: c.req_attr("uuid")?.to_owned(),
        name: c.req_attr("name")?.to_owned(),
        created: c.req_attr("angelegt")?.to_owned(),
        paths: all_texts(c, "pfad", unknown)?,
        revisions: Vec::new(),
    };
    for mut revision in c.children("fassung") {
        let number_path = attr_path(&revision, "nummer");
        let status_path = attr_path(&revision, "status");
        let bytes_path = attr_path(&revision, "bytes");
        let uuid = revision.req_attr("uuid")?.to_owned();
        let bytes = parse_u64(revision.req_attr("bytes")?, &bytes_path)?;
        let file = revision.attr("datei").map(str::to_owned);
        let mut item = ExchangeAssetRevision {
            uuid,
            number: parse_u32(revision.req_attr("nummer")?, &number_path)?,
            based_on: revision.attr("basis").map(str::to_owned),
            mime: revision.req_attr("mime")?.to_owned(),
            sha256: revision.req_attr("sha256")?.to_owned(),
            status: word(revision.req_attr("status")?, &status_path, ExchangeAssetStatus::from_xml)?,
            author: revision.req_attr("autor")?.to_owned(),
            created: revision.req_attr("angelegt")?.to_owned(),
            edited: revision.attr("geaendert").map(str::to_owned),
            caption: req_text(&mut revision, "unterschrift", unknown)?,
            alt: req_text(&mut revision, "alt", unknown)?,
            note: opt_text(&mut revision, "notiz", unknown)?.unwrap_or_default(),
            data: Vec::new(),
        };
        let data = match revision.child("daten") {
            Some(mut daten) => {
                let data = daten.base64()?;
                daten.finish(unknown);
                Some(data)
            }
            None => None,
        };
        let path = revision.path.clone();
        match (data, file) {
            (Some(data), None) => {
                if data.len() as u64 != bytes {
                    return Err(ExchangeError::Invalid(format!(
                        "{path}: {bytes} Bytes angegeben, {} Bytes in <daten>",
                        data.len()
                    )));
                }
                item.data = data;
            }
            (None, Some(entry)) => {
                refs.push(FileRef { asset: index, revision: asset.revisions.len(), entry, bytes });
            }
            (Some(_), Some(_)) => {
                return Err(ExchangeError::Invalid(format!(
                    "{path}: datei und <daten> zugleich (genau eines erlaubt)"
                )));
            }
            (None, None) => {
                return Err(ExchangeError::Invalid(format!("{path}: weder datei noch <daten> (genau eines nötig)")));
            }
        }
        asset.revisions.push(item);
        revision.finish(unknown);
    }
    if asset.revisions.is_empty() {
        return Err(ExchangeError::Xml(format!("Pflichtelement fehlt: {}/fassung", c.path)));
    }
    Ok(asset)
}

/// `<karte>` → Verbindungen und Lagen (ab Unterversion 2). Koordinaten endlich (sonst `Xml`); die Grenze
/// ±[`MAP_COORD_LIMIT`] prüft [`crate::read::validate`].
/// `<befund>` mit seinem Statusverlauf (ab Unterversion 4).
fn befund(c: &mut Cursor<'_>, version: u32, unknown: &mut Unknown) -> Result<ExchangeFinding, ExchangeError> {
    let kind_path = attr_path(c, "art");
    let status_path = attr_path(c, "status");
    let position_path = attr_path(c, "stelle");
    let mut finding = ExchangeFinding {
        uuid: c.req_attr("uuid")?.to_owned(),
        kind: word(c.req_attr("art")?, &kind_path, ExchangeFindingKind::from_xml)?,
        status: word(c.req_attr("status")?, &status_path, ExchangeFindingStatus::from_xml)?,
        persona: c.req_attr("persona")?.to_owned(),
        position: parse_u32(c.req_attr("stelle")?, &position_path)?,
        node: c.attr("punkt").map(str::to_owned),
        post: c.attr("beitrag").map(str::to_owned),
        thread_post: c.attr("faden-beitrag").map(str::to_owned),
        adopted_revision: c.attr("fassung").map(str::to_owned),
        deferred_node: c.attr("ziel-punkt").map(str::to_owned),
        deferred_idea: c.attr("ziel-einfall").map(str::to_owned),
        similar_to: c.attr("gleicht").map(str::to_owned),
        created: c.req_attr("angelegt")?.to_owned(),
        updated: c.req_attr("geaendert")?.to_owned(),
        body: req_text(c, "text", unknown)?,
        quote: opt_text(c, "zitat", unknown)?.unwrap_or_default(),
        quote_check: opt_text(c, "pruefvermerk", unknown)?.unwrap_or_default(),
        reason: opt_text(c, "begruendung", unknown)?.unwrap_or_default(),
        ..ExchangeFinding::default()
    };
    // Ab Unterversion 8: Stand der übernommenen Fassung.
    if version >= 8
        && let Some(stand) = c.attr("stand")
    {
        finding.adopted_amend = Some(parse_u32(stand, &attr_path(c, "stand"))?);
    }
    for mut summarized in c.children("sammelt") {
        finding.summary_of.push(summarized.req_attr("befund")?.to_owned());
        summarized.finish(unknown);
    }
    for mut entry in c.children("status-eintrag") {
        let status_path = attr_path(&entry, "status");
        let origin_path = attr_path(&entry, "herkunft-art");
        let proposed_path = attr_path(&entry, "vorschlag");
        let decision_path = attr_path(&entry, "entscheidung");
        let stand_path = attr_path(&entry, "stand");
        let adopted_amend =
            if version >= 8 { entry.attr("stand").map(|v| parse_u32(v, &stand_path)).transpose()? } else { None };
        finding.history.push(ExchangeFindingEntry {
            adopted_amend,
            uuid: entry.req_attr("uuid")?.to_owned(),
            status: word(entry.req_attr("status")?, &status_path, ExchangeFindingStatus::from_xml)?,
            actor_kind: word(entry.req_attr("herkunft-art")?, &origin_path, ExchangeOriginKind::from_xml)?,
            actor: entry.req_attr("herkunft")?.to_owned(),
            proposed: parse_bool(entry.req_attr("vorschlag")?, &proposed_path)?,
            adopted_revision: entry.attr("fassung").map(str::to_owned),
            deferred_node: entry.attr("ziel-punkt").map(str::to_owned),
            deferred_idea: entry.attr("ziel-einfall").map(str::to_owned),
            decision: entry
                .attr("entscheidung")
                .map(|d| word(d, &decision_path, ExchangeDecision::from_xml))
                .transpose()?,
            decided: entry.attr("entschieden").map(str::to_owned),
            decided_by: entry.attr("entschieden-von").map(str::to_owned),
            created: entry.req_attr("angelegt")?.to_owned(),
            reason: opt_text(&mut entry, "begruendung", unknown)?.unwrap_or_default(),
            note: opt_text(&mut entry, "notiz", unknown)?.unwrap_or_default(),
        });
        entry.finish(unknown);
    }
    // Ab Unterversion 5: Lösungsvorschläge.
    if version >= 5 {
        for mut solution in c.children("loesung") {
            finding.solutions.push(ExchangeSolution {
                quote: solution.attr("zitat").unwrap_or("").to_owned(),
                replacement: solution.attr("ersatz").unwrap_or("").to_owned(),
                step: solution.attr("schritt").unwrap_or("").to_owned(),
            });
            solution.finish(unknown);
        }
    }
    Ok(finding)
}

fn karte(c: &mut Cursor<'_>, unknown: &mut Unknown) -> Result<ExchangeMap, ExchangeError> {
    let mut map = ExchangeMap::default();
    for mut link in c.children("verbindung") {
        let kind_path = attr_path(&link, "art");
        let origin_path = attr_path(&link, "herkunft-art");
        map.links.push(ExchangeLink {
            uuid: link.req_attr("uuid")?.to_owned(),
            from: link.req_attr("von")?.to_owned(),
            to: link.req_attr("nach")?.to_owned(),
            kind: word(link.req_attr("art")?, &kind_path, ExchangeLinkKind::from_xml)?,
            origin_kind: word(link.req_attr("herkunft-art")?, &origin_path, ExchangeOriginKind::from_xml)?,
            origin_name: link.req_attr("herkunft")?.to_owned(),
            created: link.req_attr("angelegt")?.to_owned(),
            updated: link.req_attr("geaendert")?.to_owned(),
            deleted: link.attr("geloescht").map(str::to_owned),
            note: opt_text(&mut link, "notiz", unknown)?.unwrap_or_default(),
        });
        link.finish(unknown);
    }
    for mut position in c.children("lage") {
        let x_path = attr_path(&position, "x");
        let y_path = attr_path(&position, "y");
        map.positions.push(ExchangePosition {
            uuid: position.req_attr("uuid")?.to_owned(),
            idea: position.req_attr("einfall")?.to_owned(),
            x: parse_f64(position.req_attr("x")?, &x_path)?,
            y: parse_f64(position.req_attr("y")?, &y_path)?,
            updated: position.req_attr("geaendert")?.to_owned(),
        });
        position.finish(unknown);
    }
    Ok(map)
}
