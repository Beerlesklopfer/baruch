// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prüfung eines EPUB (Besitz „epub-pruefung“, Runde 20c): eigene Struktur- und Barrierefreiheitsprüfung,
//! Warnungen je Reader, Suche nach xmllint und EPUBCheck, Lauf von EPUBCheck. Typen seit S1 eingefroren.
//!
//! EPUBCheck wird nie heruntergeladen: `epubcheck` aus PATH oder ein epubcheck.jar unter einem vom Nutzer
//! eingestellten Pfad (`java -jar`). Starten darf es nur der Mensch (RPC/Formular), nie MCP.
//!
//! **Eigene Prüfung** ([`check_structure`], Codes in [`codes`]):
//! - ZIP nach OCF: `mimetype` als erster Eintrag, unkomprimiert, ohne Extra-Feld, Inhalt `application/epub+zip`;
//!   keine Pfade mit `..` oder `/` am Anfang.
//! - `META-INF/container.xml` mit `rootfile` auf das Paketdokument (OPF).
//! - OPF: Kennung, Titel, Sprache, `dcterms:modified`; Manifest ohne doppelte ids, jede Datei vorhanden, keine
//!   Datei ohne Manifest-Eintrag; Spine nicht leer, nur bekannte ids aus XHTML; `toc` der Spine (EPUB-2-Rückfall)
//!   zeigt auf ein NCX; `<meta name="cover">` zeigt auf eine bekannte id; Navigationsdokument (`properties="nav"`)
//!   mit `<nav epub:type="toc">`; Angaben zur Barrierefreiheit (`schema:accessMode` …) als Hinweis.
//! - Wohlgeformtheit jeder XML-Datei mit `xmllint --noout --nonet` (wenn gefunden), sonst quick-xml
//!   ([`CheckReport::xml_tool`] nennt das Werkzeug).
//! - Je XHTML: Sprache (`xml:lang`/`lang`), Titel, `alt` an jedem `<img>` (fehlend = Fehler, leer = Hinweis
//!   „schmückend“), doppelte ids, Verweise auf fehlende Dateien bzw. Sprungmarken.
//! - Lesereihenfolge: Die Dateien im Inhaltsverzeichnis (nav) folgen der Spine.

mod tools;
mod xml;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::Path;
use std::time::Duration;

use crate::readers::ReaderProfile;
use crate::{CONTAINER_PATH, EpubBook, ItemKind, MIMETYPE, OPF_PATH};
use xml::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub path: Option<String>,
    pub reader: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum XmlTool {
    Xmllint,
    QuickXml,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EpubcheckTool {
    Binary { path: std::path::PathBuf },
    Jar { path: std::path::PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EpubcheckStatus {
    NotRequested,
    NotFound,
    Ran { tool: EpubcheckTool, version: Option<String>, ok: bool },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CheckReport {
    pub findings: Vec<Finding>,
    pub xml_tool: XmlTool,
    pub epubcheck: EpubcheckStatus,
}

/// Meldecodes der eigenen Prüfung (fest, Tests und Oberfläche verlassen sich darauf).
pub mod codes {
    pub const ZIP_INVALID: &str = "ZIP_INVALID";
    pub const ZIP_PATH_INVALID: &str = "ZIP_PATH_INVALID";
    pub const MIMETYPE_NOT_FIRST: &str = "MIMETYPE_NOT_FIRST";
    pub const MIMETYPE_COMPRESSED: &str = "MIMETYPE_COMPRESSED";
    pub const MIMETYPE_EXTRA_FIELD: &str = "MIMETYPE_EXTRA_FIELD";
    pub const MIMETYPE_CONTENT: &str = "MIMETYPE_CONTENT";
    pub const CONTAINER_MISSING: &str = "CONTAINER_MISSING";
    pub const CONTAINER_INVALID: &str = "CONTAINER_INVALID";
    pub const OPF_MISSING: &str = "OPF_MISSING";
    pub const OPF_INVALID: &str = "OPF_INVALID";
    pub const METADATA_IDENTIFIER_MISSING: &str = "METADATA_IDENTIFIER_MISSING";
    pub const METADATA_TITLE_MISSING: &str = "METADATA_TITLE_MISSING";
    pub const METADATA_MODIFIED_MISSING: &str = "METADATA_MODIFIED_MISSING";
    pub const LANGUAGE_MISSING: &str = "LANGUAGE_MISSING";
    pub const MANIFEST_ITEM_INVALID: &str = "MANIFEST_ITEM_INVALID";
    pub const MANIFEST_DUPLICATE_ID: &str = "MANIFEST_DUPLICATE_ID";
    pub const MANIFEST_FILE_MISSING: &str = "MANIFEST_FILE_MISSING";
    pub const FILE_NOT_IN_MANIFEST: &str = "FILE_NOT_IN_MANIFEST";
    pub const SPINE_EMPTY: &str = "SPINE_EMPTY";
    pub const SPINE_IDREF_UNKNOWN: &str = "SPINE_IDREF_UNKNOWN";
    pub const SPINE_ITEM_NOT_CONTENT: &str = "SPINE_ITEM_NOT_CONTENT";
    pub const SPINE_TOC_UNKNOWN: &str = "SPINE_TOC_UNKNOWN";
    pub const NCX_MISSING: &str = "NCX_MISSING";
    pub const COVER_META_UNKNOWN: &str = "COVER_META_UNKNOWN";
    pub const NAV_MISSING: &str = "NAV_MISSING";
    pub const NAV_TOC_MISSING: &str = "NAV_TOC_MISSING";
    pub const SPINE_NAV_ORDER: &str = "SPINE_NAV_ORDER";
    pub const XML_NOT_WELLFORMED: &str = "XML_NOT_WELLFORMED";
    pub const XHTML_NOT_WELLFORMED: &str = "XHTML_NOT_WELLFORMED";
    pub const XHTML_LANG_MISSING: &str = "XHTML_LANG_MISSING";
    pub const XHTML_TITLE_MISSING: &str = "XHTML_TITLE_MISSING";
    pub const ID_DUPLICATE: &str = "ID_DUPLICATE";
    pub const IMG_ALT_MISSING: &str = "IMG_ALT_MISSING";
    pub const IMG_ALT_EMPTY: &str = "IMG_ALT_EMPTY";
    pub const REFERENCE_MISSING: &str = "REFERENCE_MISSING";
    pub const LINK_TARGET_MISSING: &str = "LINK_TARGET_MISSING";
    pub const A11Y_METADATA_MISSING: &str = "A11Y_METADATA_MISSING";
    /// Warnungen je Reader.
    pub const READER_POPUP_FOOTNOTES: &str = "READER_POPUP_FOOTNOTES";
    pub const READER_PAGE_BREAKS: &str = "READER_PAGE_BREAKS";
    pub const READER_EMBEDDED_FONTS: &str = "READER_EMBEDDED_FONTS";
    pub const READER_CSS_BACKGROUNDS: &str = "READER_CSS_BACKGROUNDS";
    pub const READER_SVG: &str = "READER_SVG";
    pub const READER_HYPHENATION: &str = "READER_HYPHENATION";
    pub const READER_PUBLISHER_FONT: &str = "READER_PUBLISHER_FONT";
    pub const READER_NIGHT_MODE: &str = "READER_NIGHT_MODE";
    pub const READER_CONVERSION: &str = "READER_CONVERSION";
    /// EPUBCheck-Meldungen tragen `EPUBCHECK_<Code>` (z. B. `EPUBCHECK_RSC-005`).
    pub const EPUBCHECK_PREFIX: &str = "EPUBCHECK_";
}

fn finding(code: &str, severity: Severity, message: impl Into<String>, path: Option<&str>) -> Finding {
    Finding { code: code.to_owned(), severity, message: message.into(), path: path.map(str::to_owned), reader: None }
}

// ---- ZIP ---------------------------------------------------------------------------------------------

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    bytes.get(at..at + 2).map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
}

/// Prüft den ersten lokalen Dateikopf direkt in den Bytes (die ZIP-Bibliothek verrät die Reihenfolge nicht
/// verlässlich): Name `mimetype`, Methode 0 (STORED), Extra-Feld leer, Inhalt genau [`MIMETYPE`] (Runde 23a: der Inhalt
/// auch dann, wenn der Eintrag komprimiert ist).
fn check_mimetype(epub: &[u8], out: &mut Vec<Finding>) {
    use codes::*;
    if epub.get(..4) != Some(b"PK\x03\x04") {
        return; // Kein ZIP – meldet ZIP_INVALID.
    }
    let (Some(method), Some(name_len), Some(extra_len)) = (u16_at(epub, 8), u16_at(epub, 26), u16_at(epub, 28)) else {
        return;
    };
    let name = epub.get(30..30 + name_len).unwrap_or_default();
    if name != b"mimetype" {
        out.push(finding(
            MIMETYPE_NOT_FIRST,
            Severity::Error,
            format!(
                "Der erste Eintrag im ZIP muss „mimetype“ sein, ist aber „{}“; Reader erkennen das EPUB sonst nicht.",
                String::from_utf8_lossy(name)
            ),
            Some("mimetype"),
        ));
        return;
    }
    if method != 0 {
        out.push(finding(
            MIMETYPE_COMPRESSED,
            Severity::Error,
            "„mimetype“ ist komprimiert; er muss unkomprimiert (STORED) gespeichert sein.",
            Some("mimetype"),
        ));
    }
    if extra_len != 0 {
        out.push(finding(
            MIMETYPE_EXTRA_FIELD,
            Severity::Error,
            format!("„mimetype“ trägt ein Extra-Feld ({extra_len} Bytes); erlaubt ist keines."),
            Some("mimetype"),
        ));
    }
    if method == 0 {
        let start = 30 + name_len + extra_len;
        let size = epub.get(18..22).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).unwrap_or(0);
        let content = epub.get(start..start + size).unwrap_or_default();
        if content != MIMETYPE.as_bytes() {
            out.push(finding(
                MIMETYPE_CONTENT,
                Severity::Error,
                format!("„mimetype“ enthält „{}“ statt „{MIMETYPE}“.", String::from_utf8_lossy(content)),
                Some("mimetype"),
            ));
        }
    } else if let Some(content) = unpacked_mimetype(epub)
        && content != MIMETYPE.as_bytes()
    {
        // Runde 23a (Integration R20): Auch ein komprimierter mimetype wird entpackt und sein Inhalt geprüft – sonst
        // bliebe ein falscher Medientyp hinter der Meldung „komprimiert“ unentdeckt.
        out.push(finding(
            MIMETYPE_CONTENT,
            Severity::Error,
            format!("„mimetype“ enthält „{}“ statt „{MIMETYPE}“.", String::from_utf8_lossy(&content)),
            Some("mimetype"),
        ));
    }
}

/// Inhalt des Eintrags `mimetype` über die ZIP-Bibliothek entpackt (höchstens 256 Bytes; der richtige Inhalt hat 20).
/// `None`, wenn sich das Archiv oder der Eintrag nicht lesen lässt (das melden `ZIP_INVALID` bzw. die Kopfprüfung).
fn unpacked_mimetype(epub: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(Cursor::new(epub)).ok()?;
    let entry = zip.by_name("mimetype").ok()?;
    let mut content = Vec::new();
    entry.take(256).read_to_end(&mut content).ok()?;
    Some(content)
}

/// Dateien des ZIP: Name → Inhalt (nur XML/CSS gelesen; andere mit leerem Inhalt, nur ihre Existenz zählt).
struct Archive {
    files: BTreeMap<String, Vec<u8>>,
}

impl Archive {
    fn has(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
}

/// Endungen, deren Inhalt die Prüfung liest.
fn is_readable_text(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".xhtml", ".html", ".htm", ".opf", ".ncx", ".xml", ".css", ".svg"].iter().any(|ext| lower.ends_with(ext))
}

fn open_archive(epub: &[u8], out: &mut Vec<Finding>) -> Option<Archive> {
    let mut zip = match zip::ZipArchive::new(Cursor::new(epub)) {
        Ok(zip) => zip,
        Err(e) => {
            out.push(finding(
                codes::ZIP_INVALID,
                Severity::Error,
                format!("Die Datei ist kein lesbares ZIP: {e}"),
                None,
            ));
            return None;
        }
    };
    let mut files = BTreeMap::new();
    for i in 0..zip.len() {
        let mut entry = match zip.by_index(i) {
            Ok(entry) => entry,
            Err(e) => {
                out.push(finding(codes::ZIP_INVALID, Severity::Error, format!("ZIP-Eintrag {i}: {e}"), None));
                continue;
            }
        };
        if entry.is_dir() {
            continue;
        }
        let name = match entry.name() {
            Ok(name) => name.into_owned(),
            Err(e) => {
                out.push(finding(codes::ZIP_INVALID, Severity::Error, format!("ZIP-Eintrag {i}: {e}"), None));
                continue;
            }
        };
        if name.starts_with('/') || name.contains('\\') || name.split('/').any(|p| p == ".." || p.is_empty()) {
            out.push(finding(
                codes::ZIP_PATH_INVALID,
                Severity::Error,
                format!("Unzulässiger Pfad im ZIP: „{name}“."),
                Some(&name),
            ));
            continue;
        }
        let mut bytes = Vec::new();
        if is_readable_text(&name) {
            // Höchstens 16 MiB je Textdatei; mehr wäre kein vernünftiges XHTML.
            if let Err(e) = entry.by_ref().take(16 * 1024 * 1024).read_to_end(&mut bytes) {
                out.push(finding(
                    codes::ZIP_INVALID,
                    Severity::Error,
                    format!("„{name}“ lässt sich nicht entpacken: {e}"),
                    Some(&name),
                ));
            }
        }
        files.insert(name, bytes);
    }
    Some(Archive { files })
}

// ---- Pfade -------------------------------------------------------------------------------------------

/// `%20` u. ä. entschlüsseln (Verweise in OPF und XHTML sind URLs).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Ordner einer Datei im ZIP („OEBPS/text/k1.xhtml“ → „OEBPS/text“).
fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Verweis `href` relativ zu `base_dir` als Pfad im ZIP; `None` bei Verweisen nach außen (Schema, `/…`) oder
/// über die Wurzel hinaus.
fn resolve(base_dir: &str, href: &str) -> Option<String> {
    let href = href.split(['#', '?']).next().unwrap_or("");
    if href.is_empty() || href.starts_with('/') || href.contains("://") || has_scheme(href) {
        return None;
    }
    let mut parts: Vec<String> = base_dir.split('/').filter(|p| !p.is_empty()).map(str::to_owned).collect();
    for part in percent_decode(href).split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other.to_owned()),
        }
    }
    Some(parts.join("/"))
}

/// Beginnt der Verweis mit einem Schema (`mailto:`, `https:` …)?
fn has_scheme(href: &str) -> bool {
    match href.split_once(':') {
        Some((scheme, _)) => {
            !scheme.is_empty()
                && scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                && !scheme.contains('/')
        }
        None => false,
    }
}

fn has_token(value: Option<&str>, token: &str) -> bool {
    value.is_some_and(|v| v.split_whitespace().any(|t| t == token))
}

// ---- Prüfung -----------------------------------------------------------------------------------------

/// Ein Eintrag des Manifests.
struct ManifestItem {
    id: String,
    path: String,
    media_type: String,
    properties: Option<String>,
    fallback: Option<String>,
}

const XHTML: &str = "application/xhtml+xml";
const NCX: &str = "application/x-dtbncx+xml";

/// Wohlgeformtheit prüfen und den Baum liefern (`None` = nicht wohlgeformt, gemeldet; oder ohne Baum, weil nur
/// xmllint ihn verstand).
fn well_formed(
    path: &str,
    bytes: &[u8],
    xmllint: Option<&Path>,
    tool: &mut XmlTool,
    out: &mut Vec<Finding>,
) -> Option<Element> {
    let code = if path.to_ascii_lowercase().ends_with(".xhtml") || path.ends_with(".html") {
        codes::XHTML_NOT_WELLFORMED
    } else {
        codes::XML_NOT_WELLFORMED
    };
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => {
            out.push(finding(code, Severity::Error, format!("„{path}“ ist kein UTF-8: {e}"), Some(path)));
            return None;
        }
    };
    if *tool == XmlTool::Xmllint
        && let Some(xmllint) = xmllint
    {
        match tools::xmllint(xmllint, bytes) {
            Ok(None) => return xml::parse(text).ok(),
            Ok(Some(message)) => {
                out.push(finding(
                    code,
                    Severity::Error,
                    format!("„{path}“ ist nicht wohlgeformt (xmllint): {message}"),
                    Some(path),
                ));
                return None;
            }
            // xmllint lief nicht: von hier an quick-xml (der Bericht nennt es).
            Err(_) => *tool = XmlTool::QuickXml,
        }
    }
    match xml::parse(text) {
        Ok(root) => Some(root),
        Err(message) => {
            out.push(finding(code, Severity::Error, format!("„{path}“ ist nicht wohlgeformt: {message}"), Some(path)));
            None
        }
    }
}

/// Eigene Struktur- und Barrierefreiheitsprüfung der ZIP-Bytes.
pub fn check_structure(epub: &[u8], xmllint: Option<&std::path::Path>) -> (Vec<Finding>, XmlTool) {
    use codes::*;
    let mut out = Vec::new();
    let mut tool = if xmllint.is_some() { XmlTool::Xmllint } else { XmlTool::QuickXml };
    check_mimetype(epub, &mut out);
    let Some(archive) = open_archive(epub, &mut out) else { return (out, tool) };
    if !archive.has("mimetype") && !out.iter().any(|f| f.code == MIMETYPE_NOT_FIRST) {
        out.push(finding(MIMETYPE_NOT_FIRST, Severity::Error, "Der Eintrag „mimetype“ fehlt.", Some("mimetype")));
    }

    // container.xml → Paketdokument
    let opf_path = match archive.files.get(CONTAINER_PATH) {
        None => {
            out.push(finding(
                CONTAINER_MISSING,
                Severity::Error,
                format!("„{CONTAINER_PATH}“ fehlt; ohne sie findet kein Reader das Paketdokument."),
                Some(CONTAINER_PATH),
            ));
            OPF_PATH.to_owned()
        }
        Some(bytes) => {
            let root = well_formed(CONTAINER_PATH, bytes, xmllint, &mut tool, &mut out);
            let full_path = root.as_ref().and_then(|root| {
                root.descendants()
                    .into_iter()
                    .find(|e| {
                        e.local() == "rootfile"
                            && e.attr("media-type").is_none_or(|m| m == "application/oebps-package+xml")
                    })
                    .and_then(|e| e.attr("full-path"))
                    .map(str::to_owned)
            });
            match full_path {
                Some(path) if !path.trim().is_empty() => path,
                _ => {
                    if root.is_some() {
                        out.push(finding(
                            CONTAINER_INVALID,
                            Severity::Error,
                            "„container.xml“ nennt kein Paketdokument (rootfile mit full-path).",
                            Some(CONTAINER_PATH),
                        ));
                    }
                    OPF_PATH.to_owned()
                }
            }
        }
    };
    let Some(opf_bytes) = archive.files.get(&opf_path) else {
        out.push(finding(
            OPF_MISSING,
            Severity::Error,
            format!("Das Paketdokument „{opf_path}“ fehlt."),
            Some(&opf_path),
        ));
        return (out, tool);
    };
    let Some(opf) = well_formed(&opf_path, opf_bytes, xmllint, &mut tool, &mut out) else { return (out, tool) };
    if opf.local() != "package" {
        out.push(finding(OPF_INVALID, Severity::Error, "Das Paketdokument hat kein <package>.", Some(&opf_path)));
        return (out, tool);
    }
    let opf_dir = dir_of(&opf_path).to_owned();
    let p = Some(opf_path.as_str());

    // Metadaten
    let metadata = opf.child("metadata");
    let meta_elements: Vec<&Element> = metadata.map(|m| m.descendants()).unwrap_or_default();
    let dc = |local: &str| -> Vec<&Element> {
        meta_elements
            .iter()
            .copied()
            .filter(|e| e.name == format!("dc:{local}") || (e.local() == local && e.name.contains(':')))
            .collect()
    };
    let unique = opf.attr("unique-identifier");
    let ids = dc("identifier");
    let identifier_ok =
        ids.iter().any(|e| !e.text().trim().is_empty() && unique.is_none_or(|u| e.attr("id") == Some(u)));
    if !identifier_ok {
        out.push(finding(
            METADATA_IDENTIFIER_MISSING,
            Severity::Error,
            "Keine eindeutige Kennung (dc:identifier, auf die unique-identifier zeigt).",
            p,
        ));
    }
    if !dc("title").iter().any(|e| !e.text().trim().is_empty()) {
        out.push(finding(METADATA_TITLE_MISSING, Severity::Error, "Kein Titel (dc:title) in den Metadaten.", p));
    }
    if !dc("language").iter().any(|e| !e.text().trim().is_empty()) {
        out.push(finding(
            LANGUAGE_MISSING,
            Severity::Error,
            "Keine Sprache (dc:language) in den Metadaten; Reader und Vorlesefunktionen brauchen sie (Silbentrennung, \
             Aussprache).",
            p,
        ));
    }
    let metas: Vec<&Element> = meta_elements.iter().copied().filter(|e| e.local() == "meta").collect();
    let version3 = opf.attr("version").is_some_and(|v| v.starts_with('3'));
    if version3 && !metas.iter().any(|m| m.attr("property") == Some("dcterms:modified") && !m.text().trim().is_empty())
    {
        out.push(finding(
            METADATA_MODIFIED_MISSING,
            Severity::Error,
            "EPUB 3 verlangt <meta property=\"dcterms:modified\"> mit Zeitstempel.",
            p,
        ));
    }
    let a11y = [
        "schema:accessMode",
        "schema:accessModeSufficient",
        "schema:accessibilityFeature",
        "schema:accessibilityHazard",
        "schema:accessibilitySummary",
    ];
    let missing: Vec<&str> =
        a11y.iter().copied().filter(|prop| !metas.iter().any(|m| m.attr("property") == Some(prop))).collect();
    if !missing.is_empty() {
        out.push(finding(
            A11Y_METADATA_MISSING,
            Severity::Info,
            format!(
                "Angaben zur Barrierefreiheit fehlen ({}); Shops und Vorleseprogramme zeigen sonst „unbekannt“.",
                missing.join(", ")
            ),
            p,
        ));
    }

    // Manifest
    let mut items: Vec<ManifestItem> = Vec::new();
    let mut seen_ids = BTreeSet::new();
    for item in opf
        .child("manifest")
        .map(|m| m.elements().filter(|e| e.local() == "item").collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let (Some(id), Some(href), Some(media_type)) = (item.attr("id"), item.attr("href"), item.attr("media-type"))
        else {
            out.push(finding(
                MANIFEST_ITEM_INVALID,
                Severity::Error,
                "Ein Manifest-Eintrag ohne id, href oder media-type.",
                p,
            ));
            continue;
        };
        if !seen_ids.insert(id.to_owned()) {
            out.push(finding(
                MANIFEST_DUPLICATE_ID,
                Severity::Error,
                format!("Die Manifest-id „{id}“ kommt doppelt vor."),
                p,
            ));
        }
        let Some(path) = resolve(&opf_dir, href) else {
            if !has_scheme(href) {
                out.push(finding(
                    MANIFEST_ITEM_INVALID,
                    Severity::Error,
                    format!("Der Manifest-Eintrag „{id}“ hat den unzulässigen Pfad „{href}“."),
                    p,
                ));
            }
            continue;
        };
        if !archive.has(&path) {
            out.push(finding(
                MANIFEST_FILE_MISSING,
                Severity::Error,
                format!("Der Manifest-Eintrag „{id}“ verweist auf „{path}“, die Datei fehlt im ZIP."),
                Some(&path),
            ));
        }
        items.push(ManifestItem {
            id: id.to_owned(),
            path,
            media_type: media_type.to_owned(),
            properties: item.attr("properties").map(str::to_owned),
            fallback: item.attr("fallback").map(str::to_owned),
        });
    }
    let listed: BTreeSet<&str> = items.iter().map(|i| i.path.as_str()).collect();
    for name in archive.files.keys() {
        if name == "mimetype" || name.starts_with("META-INF/") || *name == opf_path || listed.contains(name.as_str()) {
            continue;
        }
        out.push(finding(
            FILE_NOT_IN_MANIFEST,
            Severity::Error,
            format!("„{name}“ liegt im ZIP, steht aber nicht im Manifest."),
            Some(name),
        ));
    }
    let by_id: BTreeMap<&str, &ManifestItem> = items.iter().map(|i| (i.id.as_str(), i)).collect();

    // Spine
    let spine_el = opf.child("spine");
    let mut spine: Vec<&ManifestItem> = Vec::new();
    match spine_el {
        None => {
            out.push(finding(SPINE_EMPTY, Severity::Error, "Das Paketdokument hat keine Spine (Lesereihenfolge).", p))
        }
        Some(spine_el) => {
            for itemref in spine_el.elements().filter(|e| e.local() == "itemref") {
                let idref = itemref.attr("idref").unwrap_or("");
                match by_id.get(idref) {
                    None => out.push(finding(
                        SPINE_IDREF_UNKNOWN,
                        Severity::Error,
                        format!("Die Spine nennt die id „{idref}“, die im Manifest fehlt."),
                        p,
                    )),
                    Some(item) => {
                        if item.media_type != XHTML && item.fallback.is_none() {
                            out.push(finding(
                                SPINE_ITEM_NOT_CONTENT,
                                Severity::Error,
                                format!(
                                    "„{}“ steht in der Spine, ist aber kein XHTML ({}).",
                                    item.path, item.media_type
                                ),
                                Some(&item.path),
                            ));
                        }
                        spine.push(item);
                    }
                }
            }
            if spine_el.elements().all(|e| e.local() != "itemref") {
                out.push(finding(
                    SPINE_EMPTY,
                    Severity::Error,
                    "Die Spine ist leer; ein EPUB braucht mindestens eine Datei mit Text.",
                    p,
                ));
            }
            match spine_el.attr("toc") {
                Some(toc) => {
                    if by_id.get(toc).is_none_or(|i| i.media_type != NCX) {
                        out.push(finding(
                            SPINE_TOC_UNKNOWN,
                            Severity::Error,
                            format!("Die Spine verweist mit toc=\"{toc}\" auf kein NCX im Manifest."),
                            p,
                        ));
                    }
                }
                None => out.push(finding(
                    NCX_MISSING,
                    Severity::Info,
                    "Kein NCX (toc.ncx): Ältere Reader (EPUB 2) zeigen dann kein Inhaltsverzeichnis.",
                    p,
                )),
            }
        }
    }
    for meta in &metas {
        if meta.attr("name") == Some("cover")
            && let Some(content) = meta.attr("content")
            && !by_id.contains_key(content)
        {
            out.push(finding(
                COVER_META_UNKNOWN,
                Severity::Warning,
                format!("<meta name=\"cover\"> nennt die id „{content}“, die im Manifest fehlt (Kindle zeigt dann kein Cover)."),
                p,
            ));
        }
    }

    // XML-Dateien: Wohlgeformtheit, dann Inhalt
    let mut trees: BTreeMap<String, Element> = BTreeMap::new();
    for item in &items {
        let xmlish = item.media_type == XHTML || item.media_type == NCX || item.media_type == "image/svg+xml";
        let Some(bytes) = archive.files.get(&item.path).filter(|_| xmlish) else { continue };
        if let Some(root) = well_formed(&item.path, bytes, xmllint, &mut tool, &mut out) {
            trees.insert(item.path.clone(), root);
        }
    }
    // ids je XHTML (für Sprungmarken)
    let ids_of: BTreeMap<&str, BTreeSet<String>> = trees
        .iter()
        .map(|(path, root)| {
            let ids = std::iter::once(root)
                .chain(root.descendants())
                .filter_map(|e| e.attr("id").map(str::to_owned))
                .collect();
            (path.as_str(), ids)
        })
        .collect();
    for item in items.iter().filter(|i| i.media_type == XHTML) {
        let Some(root) = trees.get(&item.path) else { continue };
        check_xhtml(&item.path, root, &archive, &ids_of, &mut out);
    }

    // Navigationsdokument
    let nav_items: Vec<&ManifestItem> = items.iter().filter(|i| has_token(i.properties.as_deref(), "nav")).collect();
    match nav_items.first() {
        None => out.push(finding(
            NAV_MISSING,
            Severity::Error,
            "Kein Navigationsdokument (Manifest-Eintrag mit properties=\"nav\"); EPUB 3 verlangt es.",
            p,
        )),
        Some(nav) => {
            if let Some(root) = trees.get(&nav.path) {
                let toc = root
                    .descendants()
                    .into_iter()
                    .find(|e| e.local() == "nav" && has_token(e.attr("epub:type"), "toc"));
                match toc {
                    None => out.push(finding(
                        NAV_TOC_MISSING,
                        Severity::Error,
                        "Das Navigationsdokument hat kein <nav epub:type=\"toc\">.",
                        Some(&nav.path),
                    )),
                    Some(toc) => check_reading_order(&nav.path, toc, &spine, &mut out),
                }
            }
        }
    }
    (out, tool)
}

/// Inhalt einer XHTML-Datei: Sprache, Titel, alt-Texte, doppelte ids, Verweise.
fn check_xhtml(
    path: &str,
    root: &Element,
    archive: &Archive,
    ids_of: &BTreeMap<&str, BTreeSet<String>>,
    out: &mut Vec<Finding>,
) {
    use codes::*;
    let lang = root.attr("xml:lang").or_else(|| root.attr("lang")).filter(|l| !l.trim().is_empty());
    if lang.is_none() {
        out.push(finding(
            XHTML_LANG_MISSING,
            Severity::Warning,
            format!("„{path}“ nennt keine Sprache (xml:lang/lang am <html>); Vorleseprogramme raten dann."),
            Some(path),
        ));
    }
    let all = root.descendants();
    let title = all.iter().find(|e| e.local() == "title").map(|t| t.text());
    if title.as_deref().is_none_or(|t| t.trim().is_empty()) {
        out.push(finding(
            XHTML_TITLE_MISSING,
            Severity::Warning,
            format!("„{path}“ hat keinen Titel (<title> im <head>)."),
            Some(path),
        ));
    }
    let mut ids = BTreeSet::new();
    let dir = dir_of(path);
    for e in std::iter::once(root).chain(all.iter().copied()) {
        if let Some(id) = e.attr("id")
            && !ids.insert(id)
        {
            out.push(finding(
                ID_DUPLICATE,
                Severity::Error,
                format!("„{path}“: die id „{id}“ kommt doppelt vor."),
                Some(path),
            ));
        }
        let local = e.local();
        if local == "img" {
            let src = e.attr("src").unwrap_or("");
            match e.attr("alt") {
                None => out.push(finding(
                    IMG_ALT_MISSING,
                    Severity::Error,
                    format!("„{path}“: Bild „{src}“ ohne alt-Attribut; Vorleseprogramme lesen sonst den Dateinamen."),
                    Some(path),
                )),
                Some(alt) if alt.trim().is_empty() => out.push(finding(
                    IMG_ALT_EMPTY,
                    Severity::Info,
                    format!("„{path}“: Bild „{src}“ hat einen leeren Alternativtext und gilt als Schmuck (wird nicht vorgelesen)."),
                    Some(path),
                )),
                Some(_) => {}
            }
        }
        let reference = match local {
            "img" => e.attr("src"),
            "link" | "a" => e.attr("href"),
            "image" => e.attr("xlink:href").or_else(|| e.attr("href")),
            _ => None,
        };
        let Some(href) = reference else { continue };
        let fragment = href.split_once('#').map(|(_, f)| percent_decode(f));
        let target = if href.starts_with('#') { Some(path.to_owned()) } else { resolve(dir, href) };
        let Some(target) = target else { continue };
        if !archive.has(&target) {
            out.push(finding(
                REFERENCE_MISSING,
                Severity::Error,
                format!("„{path}“ verweist auf „{href}“; „{target}“ fehlt im ZIP."),
                Some(path),
            ));
            continue;
        }
        if let (Some(fragment), Some(ids)) = (fragment.filter(|f| !f.is_empty()), ids_of.get(target.as_str()))
            && !ids.contains(&fragment)
        {
            out.push(finding(
                LINK_TARGET_MISSING,
                Severity::Warning,
                format!("„{path}“ verweist auf „{href}“; in „{target}“ gibt es die Sprungmarke „{fragment}“ nicht."),
                Some(path),
            ));
        }
    }
}

/// Die Dateien im Inhaltsverzeichnis müssen in Spine-Reihenfolge stehen (sonst springt „weiter“ zurück).
fn check_reading_order(nav_path: &str, toc: &Element, spine: &[&ManifestItem], out: &mut Vec<Finding>) {
    let position: BTreeMap<&str, usize> = spine.iter().enumerate().map(|(i, item)| (item.path.as_str(), i)).collect();
    let dir = dir_of(nav_path);
    let mut last: Option<(usize, String)> = None;
    let mut seen = BTreeSet::new();
    for a in toc.descendants().into_iter().filter(|e| e.local() == "a") {
        let Some(target) = a.attr("href").and_then(|h| resolve(dir, h)) else { continue };
        if !seen.insert(target.clone()) {
            continue;
        }
        let Some(&index) = position.get(target.as_str()) else { continue };
        if let Some((before, before_path)) = &last
            && index < *before
        {
            out.push(finding(
                codes::SPINE_NAV_ORDER,
                Severity::Warning,
                format!(
                    "Das Inhaltsverzeichnis nennt „{target}“ nach „{before_path}“, in der Spine steht es davor; \
                     Lesereihenfolge und Inhaltsverzeichnis widersprechen sich."
                ),
                Some(nav_path),
            ));
            return;
        }
        last = Some((index, target));
    }
}

// ---- Reader ------------------------------------------------------------------------------------------

fn text_of(book: &EpubBook, pred: impl Fn(&crate::EpubItem) -> bool) -> String {
    book.items
        .iter()
        .filter(|i| pred(i))
        .map(|i| String::from_utf8_lossy(&i.bytes).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Warnungen je Reader aus `profile.supports`.
pub fn reader_warnings(book: &EpubBook, profile: &ReaderProfile) -> Vec<Finding> {
    use codes::*;
    let s = profile.supports;
    let css = text_of(book, |i| i.media_type == "text/css");
    let xhtml = text_of(book, |i| i.media_type == XHTML);
    let mut out = Vec::new();
    let mut push = |code: &str, severity: Severity, message: String| {
        out.push(Finding {
            code: code.to_owned(),
            severity,
            message: format!("{}: {message}", profile.name),
            path: None,
            reader: Some(profile.slug.to_owned()),
        });
    };
    if profile.via_conversion {
        push(
            READER_CONVERSION,
            Severity::Info,
            "Das EPUB geht über „Send to Kindle“ (Amazon wandelt es um); die Vorschau ist eine Annäherung.".into(),
        );
    }
    let popups = xhtml.contains("epub:type=\"noteref\"");
    if popups && !s.popup_footnotes {
        push(
            READER_POPUP_FOOTNOTES,
            Severity::Warning,
            "zeigt Fußnoten nicht als Popup; ein Tipp auf die Fußnotenzahl springt ans Kapitelende (dort stehen sie \
             lesbar, mit Rücksprung). Wahlweise Endnoten (fussnoten=endnoten)."
                .into(),
        );
    }
    let chapters = book.items.iter().any(|i| matches!(i.kind, ItemKind::Chapter | ItemKind::Part));
    if chapters && !s.page_break_css {
        push(
            READER_PAGE_BREAKS,
            Severity::Info,
            "beachtet keine Seitenumbruch-Regeln (break-before) im CSS. Kapitel und Teile beginnen trotzdem auf einer \
             neuen Seite, weil jedes eine eigene Datei ist; Abschnitte innerhalb eines Kapitels laufen weiter."
                .into(),
        );
    }
    let fonts = book.items.iter().any(|i| i.media_type.starts_with("font/") || i.media_type.contains("font-"))
        || css.contains("@font-face");
    if fonts && !s.embedded_fonts {
        push(
            READER_EMBEDDED_FONTS,
            Severity::Warning,
            "zeigt eingebettete Schriften nicht; es gilt die Schrift des Readers.".into(),
        );
    }
    // Der Grundstil (`.baruch-missing { font-family: monospace; }`) stellt keine Schrift des Buchs ein.
    let declares_font = css.replace(crate::build::BASE_CSS, "").contains("font-family");
    if declares_font && s.overrides_publisher_font {
        push(
            READER_PUBLISHER_FONT,
            Severity::Info,
            "nimmt in der Regel die eigene Schrift; die Schrift des Buchs gilt erst, wenn der Leser „Schrift des \
             Verlags“ wählt."
                .into(),
        );
    }
    let backgrounds = css.contains("background");
    if backgrounds && !s.css_backgrounds {
        push(
            READER_CSS_BACKGROUNDS,
            Severity::Warning,
            "zeigt keine CSS-Hintergründe; Flächen und Hintergrundbilder fehlen.".into(),
        );
    }
    if backgrounds && s.night_mode {
        push(
            READER_NIGHT_MODE,
            Severity::Info,
            "kennt einen Nachtmodus; feste Hintergrundfarben im CSS können dort hell aufleuchten.".into(),
        );
    }
    let svg = book.items.iter().any(|i| i.media_type == "image/svg+xml");
    if svg && !s.svg_images {
        push(READER_SVG, Severity::Warning, "zeigt SVG-Bilder nicht oder nur unvollständig; besser PNG/JPEG.".into());
    }
    let hyphens = css.contains("hyphens: auto") || css.contains("hyphens:auto");
    if hyphens && !s.hyphenation {
        push(READER_HYPHENATION, Severity::Info, "trennt nicht; im Blocksatz entstehen größere Wortabstände.".into());
    }
    out
}

// ---- Programme ---------------------------------------------------------------------------------------

/// Sucht ein ausführbares Programm `name` in `path_env` (Inhalt von PATH), z. B. `java`.
pub fn find_program(name: &str, path_env: Option<&std::ffi::OsStr>) -> Option<std::path::PathBuf> {
    tools::find_in_path(name, path_env)
}

/// Sucht `xmllint` in `path_env` (Inhalt von PATH; Tests geben eigene Ordner).
pub fn find_xmllint(path_env: Option<&std::ffi::OsStr>) -> Option<std::path::PathBuf> {
    tools::find_in_path("xmllint", path_env)
}

/// Ist `path` ein brauchbares epubcheck.jar (vorhandene reguläre Datei mit der Endung `.jar`)?
pub fn is_jar(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar"))
        && std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

/// Sucht `epubcheck` in `path_env`, sonst `jar` (vorhandene Datei, .jar) – Reihenfolge: Binary vor Jar.
pub fn find_epubcheck(path_env: Option<&std::ffi::OsStr>, jar: Option<&std::path::Path>) -> Option<EpubcheckTool> {
    if let Some(path) = tools::find_in_path("epubcheck", path_env) {
        return Some(EpubcheckTool::Binary { path });
    }
    jar.filter(|j| is_jar(j)).map(|j| EpubcheckTool::Jar { path: j.to_owned() })
}

/// Eine Zeile von EPUBCheck: `ERROR(RSC-005): buch.epub/OEBPS/x.xhtml(12,5): Meldung`.
fn epubcheck_line(line: &str) -> Option<Finding> {
    let line = line.trim();
    let (head, rest) = line.split_once("): ")?;
    let (level, code) = head.split_once('(')?;
    let severity = match level.trim() {
        "FATAL" | "ERROR" => Severity::Error,
        "WARNING" => Severity::Warning,
        "INFO" | "USAGE" | "HINT" => Severity::Info,
        _ => return None,
    };
    // Ort „datei.epub/OEBPS/x.xhtml(12,5)“ bzw. „datei.epub“, danach die Meldung.
    let (location, message) = match rest.split_once("): ") {
        Some((location, message)) if location.contains('(') => (Some(format!("{location})")), message.to_owned()),
        _ => match rest.split_once(": ") {
            Some((location, message)) if location.contains(".epub") => (Some(location.to_owned()), message.to_owned()),
            _ => (None, rest.to_owned()),
        },
    };
    let path = location.map(|l| match l.find(".epub/") {
        Some(at) => l[at + ".epub/".len()..].to_owned(),
        None => l.rsplit('/').next().unwrap_or(&l).to_owned(),
    });
    Some(Finding {
        code: format!("{}{}", codes::EPUBCHECK_PREFIX, code.trim()),
        severity,
        message: format!("EPUBCheck: {}", message.trim()),
        path,
        reader: None,
    })
}

/// Version aus der Ausgabe („EPUBCheck v5.1.0“ bzw. „Validating using EPUB version 3.3 rules.“ zählt nicht).
fn epubcheck_version(output: &str) -> Option<String> {
    output.lines().find_map(|l| {
        let at = l.find("EPUBCheck v").or_else(|| l.find("Epubcheck Version "))?;
        let rest = &l[at..];
        let version: String = rest
            .split_once(['v', 'V'])
            .map(|(_, r)| r.trim_start_matches("ersion ").trim())
            .unwrap_or("")
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
            .collect();
        (!version.is_empty()).then_some(version)
    })
}

/// Führt EPUBCheck aus (blockierend, Zeitgrenze, Kind wird bei Ablauf beendet).
pub fn run_epubcheck(
    tool: &EpubcheckTool,
    epub_file: &std::path::Path,
    timeout: std::time::Duration,
) -> Result<(Vec<Finding>, Option<String>, bool), String> {
    let mut command = match tool {
        EpubcheckTool::Binary { path } => std::process::Command::new(path),
        EpubcheckTool::Jar { path } => {
            let mut command = std::process::Command::new("java");
            command.arg("-jar").arg(path);
            command
        }
    };
    command.arg(epub_file);
    let output = tools::run(command, None, timeout.max(Duration::from_millis(1)))?;
    let all = format!("{}\n{}", output.stdout, output.stderr);
    let findings: Vec<Finding> = all.lines().filter_map(epubcheck_line).collect();
    let version = epubcheck_version(&all);
    let errors = findings.iter().any(|f| f.severity == Severity::Error);
    Ok((findings, version, output.success && !errors))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_resolve_relative_to_their_file_and_never_leave_the_archive() {
        assert_eq!(
            resolve("OEBPS/text", "../images/a%20b.png").as_deref(),
            Some("OEBPS/images/a b.png"),
            "relativ, entschlüsselt"
        );
        assert_eq!(resolve("OEBPS/text", "k1.xhtml#k2").as_deref(), Some("OEBPS/text/k1.xhtml"), "Sprungmarke weg");
        assert_eq!(resolve("", "../../x"), None, "nicht über die Wurzel hinaus");
        assert_eq!(resolve("OEBPS", "https://example.org/"), None, "Verweis nach außen");
        assert_eq!(resolve("OEBPS", "mailto:a@b"), None, "Schema");
    }

    #[test]
    fn epubcheck_lines_become_findings_with_path_and_code() {
        let f = epubcheck_line("ERROR(RSC-005): /tmp/x/buch.epub/OEBPS/text/k1.xhtml(12,5): Fehler beim Lesen")
            .expect("Zeile");
        assert_eq!(f.code, "EPUBCHECK_RSC-005", "Code mit Vorsilbe");
        assert_eq!(f.severity, Severity::Error, "ERROR → Fehler");
        assert_eq!(f.path.as_deref(), Some("OEBPS/text/k1.xhtml(12,5)"), "Pfad im EPUB samt Stelle");
        assert!(f.message.ends_with("Fehler beim Lesen"), "Meldung: {}", f.message);
        let w = epubcheck_line("WARNING(OPF-085): buch.epub: Kennung ist keine UUID").expect("Zeile");
        assert_eq!((w.severity, w.path.as_deref()), (Severity::Warning, Some("buch.epub")), "Ort ohne Stelle");
        assert!(epubcheck_line("Validating using EPUB version 3.3 rules.").is_none(), "Plaudertext ist keine Meldung");
        assert_eq!(epubcheck_version("EPUBCheck v5.1.0\n").as_deref(), Some("5.1.0"), "Version aus der ersten Zeile");
    }
}
