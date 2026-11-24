// SPDX-License-Identifier: AGPL-3.0-or-later
//! PDF lokal lesen (Runde 21a) über Poppler: `pdfinfo` (Titel, Autor, Seiten) und `pdftotext -bbox-layout` (Wörter
//! mit Rahmen, nach Blöcken und Zeilen). Beide laufen mit Zeitlimit ([`TOOL_TIMEOUT`]), `kill_on_drop`, ohne
//! Standardeingabe und mit begrenzter Ausgabe; sie öffnen keine Netzverbindung. Fehlt ein Programm, ist das eine
//! ehrliche Meldung (kein stiller Wechsel zu MultiGPT).
//!
//! **Gliederung (Heuristik):** Die Höhe der Wörter einer Zeile steht für ihre Schriftgröße. Die häufigste Höhe
//! (nach Zeichen gewichtet) ist der Fließtext; ein Block aus höchstens drei kurzen Zeilen, deren Höhe mindestens
//! [`HEADING_RATIO`] × Fließtext beträgt, ist eine Überschrift – die größte Höhe Ebene 1, die nächste Ebene 2 usw.
//! (höchstens 3). Eine kurze Zeile in Fließtextgröße mit Nummer `1.2.3 Titel` (ohne Satzzeichen am Ende) gilt als
//! Überschrift der Ebene nach der Zahl der Nummernteile; die Nummer fällt weg (Baruch nummeriert selbst, Warnung).
//! Kommt die größte Höhe genau einmal vor, auf Seite 1 vor jeder anderen Überschrift, ist sie der Titel des
//! Dokuments, kein Kapitel. Reine Seitenzahlen am Seitenanfang bzw. -ende fallen weg (Warnung mit Anzahl).
//! Hervorhebungen, Fußnoten und Bilder erkennt dieser Weg nicht (Warnung).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncReadExt;

use super::Extracted;
use super::typst_out::{Section, escape_text};
use super::xml::{self, Element};
use crate::import::ImportWarning;

/// Zeitlimit je Aufruf von `pdfinfo` bzw. `pdftotext`.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(120);
/// Größte Ausgabe von `pdftotext -bbox-layout` (Wörter mit Koordinaten sind gut zehnmal so lang wie der Text; eine
/// dichte Seite ergibt etwa 40 KiB, [`MAX_PAGES`] Seiten also um 80 MiB).
pub const MAX_TOOL_OUTPUT: usize = 128 * 1024 * 1024;
/// Höchstens so viele Seiten liest der lokale Weg (wie der Weg über MultiGPT); den Rest meldet eine Warnung.
pub const MAX_PAGES: usize = 2000;
/// Höchstens so viele PDF-Dateien werden zugleich lokal gelesen; weitere warten (Speicher und Rechenzeit begrenzt).
pub const MAX_PARALLEL_READS: usize = 2;
static READS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(MAX_PARALLEL_READS);
/// Mindestverhältnis der Zeilenhöhe einer Überschrift zum Fließtext.
pub const HEADING_RATIO: f64 = 1.12;
/// Höchstens so viele Zeichen hat eine Überschrift.
const MAX_HEADING_CHARS: usize = 160;

/// Programme für den lokalen Weg (Vorgabe: `pdftotext` und `pdfinfo` aus dem Suchpfad). Tests geben einen nicht
/// vorhandenen Pfad an, um „nicht installiert“ zu prüfen, ohne Umgebungsvariablen zu ändern.
#[derive(Debug, Clone)]
pub struct PdfTools {
    pub pdftotext: PathBuf,
    pub pdfinfo: PathBuf,
}

impl Default for PdfTools {
    fn default() -> Self {
        PdfTools { pdftotext: PathBuf::from("pdftotext"), pdfinfo: PathBuf::from("pdfinfo") }
    }
}

fn warn(code: &str, message: impl Into<String>) -> ImportWarning {
    ImportWarning { code: code.into(), message: message.into(), line: None }
}

/// Programm mit Zeitlimit ausführen; Standardausgabe (höchstens `limit` Bytes) oder ehrliche Meldung.
async fn run(program: &Path, args: &[&std::ffi::OsStr], limit: usize) -> Result<Vec<u8>, String> {
    let name = program.display().to_string();
    let mut child = tokio::process::Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!(
                    "{name} ist nicht installiert (Poppler, Paket poppler-utils): Ohne pdftotext und pdfinfo lässt sich \
                     eine PDF-Datei nicht lokal lesen. Poppler installieren oder den Weg über MultiGPT wählen."
                )
            } else {
                format!("{name} lässt sich nicht starten: {e}")
            }
        })?;
    let mut stdout = child.stdout.take().ok_or_else(|| format!("{name}: keine Ausgabe"))?;
    let mut stderr = child.stderr.take().ok_or_else(|| format!("{name}: keine Fehlerausgabe"))?;
    let work = async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut out_reader = (&mut stdout).take(limit as u64 + 1);
        // Fehlerausgabe ganz leeren, aber nur die ersten 64 KiB behalten: Bliebe der Rest in der Pipe, blockierte
        // Poppler bei vielen „Syntax Error“-Zeilen, und der Aufruf hinge bis zur Zeitgrenze (Gutachten R21).
        let drain_err = async {
            let mut err_reader = (&mut stderr).take(64 * 1024);
            let first = err_reader.read_to_end(&mut err).await;
            let rest = tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await;
            first.and(rest.map(|_| 0))
        };
        let (read_out, read_err) = tokio::join!(out_reader.read_to_end(&mut out), drain_err);
        read_out.map_err(|e| format!("{name}: Ausgabe nicht lesbar: {e}"))?;
        let _ = read_err;
        if out.len() > limit {
            return Err(format!("{name}: Ausgabe größer als {} MiB; abgebrochen.", limit / (1024 * 1024)));
        }
        let status = child.wait().await.map_err(|e| format!("{name}: {e}"))?;
        if !status.success() {
            let text = String::from_utf8_lossy(&err);
            let text = text.trim();
            return Err(format!(
                "{name} meldet einen Fehler ({}){}",
                status.code().map_or_else(|| "abgebrochen".to_owned(), |c| format!("Code {c}")),
                if text.is_empty() { String::new() } else { format!(": {}", super::short(text, 300)) }
            ));
        }
        Ok(out)
    };
    match tokio::time::timeout(TOOL_TIMEOUT, work).await {
        Ok(result) => result,
        Err(_) => Err(format!("{name} brauchte länger als {} s; abgebrochen.", TOOL_TIMEOUT.as_secs())),
    }
}

/// Angaben aus `pdfinfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdfInfo {
    pub title: Option<String>,
    pub author: Option<String>,
    pub pages: Option<usize>,
}

fn parse_info(text: &str) -> PdfInfo {
    let mut info = PdfInfo::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else { continue };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "Title" => info.title = Some(value.to_owned()),
            "Author" => info.author = Some(value.to_owned()),
            "Pages" => info.pages = value.parse().ok(),
            _ => {}
        }
    }
    info
}

/// Eine Zeile aus `pdftotext -bbox-layout`.
#[derive(Debug, Clone)]
struct Line {
    page: usize,
    block: usize,
    text: String,
    height: f64,
}

fn number(e: &Element, name: &str) -> f64 {
    e.attr(name).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
}

/// Zeilen aller Seiten (Block-Nummer fortlaufend über das Dokument).
fn lines_of(doc: &Element) -> Vec<Line> {
    let mut out = Vec::new();
    let mut pages = Vec::new();
    doc.find_all("page", &mut pages);
    let mut block_no = 0;
    for (index, page) in pages.iter().enumerate() {
        let mut blocks = Vec::new();
        page.find_all("block", &mut blocks);
        for block in blocks {
            block_no += 1;
            let mut lines = Vec::new();
            block.find_all("line", &mut lines);
            for line in lines {
                let mut words = Vec::new();
                line.find_all("word", &mut words);
                let mut heights: Vec<f64> =
                    words.iter().map(|w| number(w, "ymax") - number(w, "ymin")).filter(|h| *h > 0.0).collect();
                let text = words.iter().map(|w| w.text()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
                if text.is_empty() {
                    continue;
                }
                heights.sort_by(f64::total_cmp);
                let height = heights.get(heights.len() / 2).copied().unwrap_or(0.0);
                out.push(Line { page: index + 1, block: block_no, text, height });
            }
        }
    }
    out
}

/// Höhe auf halbe Punkte gerundet (Schlüssel für Größenklassen).
fn class(height: f64) -> i64 {
    (height * 2.0).round() as i64
}

/// `1.2.3 Titel` → (3, „Titel“); auch `1. Titel`, `2 Titel`. Höchstens vier Nummernteile.
pub(crate) fn numbered(text: &str) -> Option<(usize, String)> {
    let mut parts = 0;
    let mut rest = text.trim();
    loop {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 || digits > 3 {
            return None;
        }
        parts += 1;
        rest = &rest[digits..];
        if let Some(after) = rest.strip_prefix('.') {
            rest = after;
            if rest.starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
        }
        break;
    }
    if parts > 4 || !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let title = rest.trim();
    if title.is_empty() || !title.starts_with(char::is_alphabetic) {
        return None;
    }
    Some((parts, title.to_owned()))
}

/// Zeilen eines Absatzes verbinden: Silbentrennung am Zeilenende (`Wort-` + Kleinbuchstabe) fällt weg.
pub(crate) fn join_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> String {
    let mut out = String::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if out.is_empty() {
            out.push_str(line);
        } else if out.ends_with('-')
            && out.chars().rev().nth(1).is_some_and(char::is_alphabetic)
            && line.starts_with(char::is_lowercase)
        {
            out.pop();
            out.push_str(line);
        } else {
            out.push(' ');
            out.push_str(line);
        }
    }
    out
}

/// Ausgabe von `pdftotext -bbox-layout` in Abschnitte gliedern (siehe Moduldoku).
pub(crate) fn structure(bbox: &str, info: &PdfInfo) -> Result<Extracted, String> {
    let doc = xml::parse(bbox).map_err(|e| format!("Ausgabe von pdftotext nicht lesbar: {e}"))?;
    let mut lines = lines_of(&doc);
    let mut warnings = vec![warn(
        "import-pdf-plain",
        "PDF lokal gelesen: übernommen werden Text und Überschriften (nach Schriftgröße bzw. Nummer); Hervorhebungen, \
         Fußnoten und Bilder erkennt dieser Weg nicht.",
    )];
    if lines.is_empty() {
        return Err(
            "Die PDF-Datei enthält keinen lesbaren Text (vielleicht nur gescannte Bilder); lokal lässt sich nichts \
             übernehmen."
                .into(),
        );
    }
    // Seitenzahlen: eine Zeile nur aus Ziffern als erste oder letzte Zeile einer Seite.
    let mut page_numbers = 0;
    let mut keep = vec![true; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        let first = i == 0 || lines[i - 1].page != line.page;
        let last = i + 1 == lines.len() || lines[i + 1].page != line.page;
        if (first || last) && line.text.len() <= 4 && line.text.chars().all(|c| c.is_ascii_digit()) {
            keep[i] = false;
            page_numbers += 1;
        }
    }
    let mut index = 0;
    lines.retain(|_| {
        index += 1;
        keep[index - 1]
    });
    if page_numbers > 0 {
        warnings.push(warn("import-pdf-page-numbers", format!("{page_numbers} Seitenzahl(en) aus dem Text entfernt.")));
    }
    // Fließtext-Größe: häufigste Größenklasse nach Zeichen.
    let mut weight: std::collections::BTreeMap<i64, usize> = Default::default();
    for line in &lines {
        *weight.entry(class(line.height)).or_default() += line.text.chars().count();
    }
    let body_class = weight.iter().max_by_key(|(_, n)| **n).map(|(c, _)| *c).unwrap_or(0);
    let body = body_class as f64 / 2.0;

    // Blöcke: Zeilen mit gleicher Block-Nummer.
    struct Block {
        page: usize,
        lines: Vec<Line>,
    }
    let mut blocks: Vec<Block> = Vec::new();
    for line in lines {
        match blocks.last_mut() {
            Some(b) if b.lines[0].block == line.block => b.lines.push(line),
            _ => blocks.push(Block { page: line.page, lines: vec![line] }),
        }
    }
    // Überschriften nach Größe.
    let big = |b: &Block| {
        let chars: usize = b.lines.iter().map(|l| l.text.chars().count()).sum();
        b.lines.len() <= 3
            && chars <= MAX_HEADING_CHARS
            && b.lines.iter().all(|l| body > 0.0 && l.height >= body * HEADING_RATIO)
    };
    let mut classes: Vec<i64> = blocks.iter().filter(|b| big(b)).map(|b| class(b.lines[0].height)).collect();
    classes.sort_unstable_by(|a, b| b.cmp(a));
    classes.dedup();
    // Der Titel des Dokuments: größte Klasse genau einmal, auf Seite 1, vor jeder anderen Überschrift.
    let mut doc_title: Option<(usize, String)> = None;
    if let Some(&top) = classes.first() {
        let tops: Vec<usize> = blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| big(b) && class(b.lines[0].height) == top)
            .map(|(i, _)| i)
            .collect();
        let first_big = blocks.iter().position(&big);
        if tops.len() == 1 && blocks[tops[0]].page == 1 && first_big == Some(tops[0]) && classes.len() > 1 {
            let text = join_lines(blocks[tops[0]].lines.iter().map(|l| l.text.as_str()));
            doc_title = Some((tops[0], text));
            classes.remove(0);
        }
    }
    let level_of = |c: i64| classes.iter().position(|x| *x == c).map(|i| (i + 1).min(3) as u8);

    let mut sections: Vec<Section> = Vec::new();
    let mut front: (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    let mut stripped = 0;
    for (i, block) in blocks.iter().enumerate() {
        if doc_title.as_ref().is_some_and(|(at, _)| *at == i) {
            continue;
        }
        let text = join_lines(block.lines.iter().map(|l| l.text.as_str()));
        let heading = if big(block) {
            level_of(class(block.lines[0].height)).map(|level| match numbered(&text) {
                // Mit Nummer entscheidet die Zahl der Nummernteile über die Ebene.
                Some((parts, title)) => {
                    stripped += 1;
                    (parts.min(3) as u8, title)
                }
                None => (level, text.clone()),
            })
        } else if block.lines.len() == 1
            && text.chars().count() <= 100
            && !text.ends_with(['.', ',', ';', ':'])
            && let Some((parts, title)) = numbered(&text)
        {
            stripped += 1;
            Some((parts.min(3) as u8, title))
        } else {
            None
        };
        match heading {
            Some((level, title)) => sections.push(Section {
                level,
                part: level == 1 && ["teil ", "part ", "buch "].iter().any(|p| title.to_lowercase().starts_with(p)),
                title,
                body: String::new(),
                plain: String::new(),
            }),
            None => {
                let typst = escape_text(&text, true);
                match sections.last_mut() {
                    Some(section) => {
                        if !section.body.is_empty() {
                            section.body.push_str("\n\n");
                            section.plain.push_str("\n\n");
                        }
                        section.body.push_str(&typst);
                        section.plain.push_str(&text);
                    }
                    None => {
                        front.0.push(typst);
                        front.1.push(text);
                    }
                }
            }
        }
    }
    if stripped > 0 {
        warnings.push(warn(
            "import-pdf-numbering",
            format!("Bei {stripped} Überschrift(en) fiel die Nummer weg; Baruch nummeriert nach dem Gliederungsschema selbst."),
        ));
    }
    if sections.is_empty() {
        warnings.push(warn("import-no-outline", "Keine Überschriften erkannt: der ganze Text steht in einem Kapitel."));
        let title =
            doc_title.as_ref().map(|(_, t)| t.clone()).or_else(|| info.title.clone()).unwrap_or_else(|| "Text".into());
        sections.push(Section {
            level: 1,
            title,
            part: false,
            body: front.0.join("\n\n"),
            plain: front.1.join("\n\n"),
        });
    } else if !front.1.is_empty() {
        sections.insert(
            0,
            Section {
                level: 1,
                title: "Vorspann".into(),
                part: false,
                body: front.0.join("\n\n"),
                plain: front.1.join("\n\n"),
            },
        );
        warnings.push(warn(
            "import-front-matter",
            "Text vor der ersten Überschrift steht im Punkt „Vorspann“ (streichen hängt ihn an das erste Kapitel).",
        ));
    }
    let plain_text = {
        let mut parts = Vec::new();
        if let Some((_, t)) = &doc_title {
            parts.push(t.clone());
        }
        for s in &sections {
            if s.title != "Vorspann" || !s.plain.is_empty() {
                if s.title != "Vorspann" {
                    parts.push(s.title.clone());
                }
                if !s.plain.is_empty() {
                    parts.push(s.plain.clone());
                }
            }
        }
        parts.join("\n\n")
    };
    Ok(Extracted {
        title: info.title.clone().or_else(|| doc_title.map(|(_, t)| t)),
        author: info.author.clone(),
        lang: None,
        pages: info.pages,
        sections,
        assets: Vec::new(),
        skipped: Vec::new(),
        warnings,
        plain_text,
    })
}

/// PDF-Datei `path` lokal lesen.
pub(crate) async fn read(path: &Path, tools: &PdfTools) -> Result<Extracted, String> {
    let _permit = READS.acquire().await.map_err(|e| format!("Lesen der PDF abgebrochen: {e}"))?;
    let info_out = run(&tools.pdfinfo, &["-enc".as_ref(), "UTF-8".as_ref(), path.as_os_str()], 1024 * 1024).await?;
    let info = parse_info(&String::from_utf8_lossy(&info_out));
    let pages = info.pages;
    let last = MAX_PAGES.to_string();
    let bbox = run(
        &tools.pdftotext,
        &[
            "-enc".as_ref(),
            "UTF-8".as_ref(),
            "-f".as_ref(),
            "1".as_ref(),
            "-l".as_ref(),
            last.as_ref(),
            "-bbox-layout".as_ref(),
            path.as_os_str(),
            "-".as_ref(),
        ],
        MAX_TOOL_OUTPUT,
    )
    .await?;
    // Ohne Kopie, wenn die Ausgabe gültiges UTF-8 ist (der Normalfall bei `-enc UTF-8`).
    let bbox = String::from_utf8(bbox).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned());
    let mut read = tokio::task::spawn_blocking(move || structure(&bbox, &info))
        .await
        .map_err(|e| format!("Lesen der PDF abgebrochen: {e}"))??;
    if let Some(warning) = page_limit_warning(pages) {
        read.warnings.push(warning);
    }
    Ok(read)
}

/// Warnung, wenn die PDF mehr Seiten hat, als der lokale Weg liest.
fn page_limit_warning(pages: Option<usize>) -> Option<ImportWarning> {
    let pages = pages.filter(|p| *p > MAX_PAGES)?;
    Some(warn(
        "import-pdf-pages-cut",
        format!(
            "Die PDF hat {pages} Seiten; gelesen wurden nur die ersten {MAX_PAGES}. Den Rest bitte getrennt einlesen."
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_pages_than_the_limit_are_reported() {
        assert!(page_limit_warning(None).is_none(), "ohne Seitenzahl keine Warnung");
        assert!(page_limit_warning(Some(MAX_PAGES)).is_none(), "genau an der Grenze wird alles gelesen");
        let w = page_limit_warning(Some(MAX_PAGES + 1)).expect("über der Grenze: Warnung");
        assert_eq!(w.code, "import-pdf-pages-cut");
        assert!(w.message.contains("2001") && w.message.contains("2000"), "nennt beide Zahlen: {}", w.message);
    }

    #[test]
    fn numbered_headings_and_hyphenation_are_recognised() {
        assert_eq!(numbered("1.2 Am Hafen"), Some((2, "Am Hafen".into())), "zwei Nummernteile: Ebene 2");
        assert_eq!(numbered("3. Kapitel drei"), Some((1, "Kapitel drei".into())), "Punkt nach der Nummer");
        assert_eq!(numbered("1999 war ein Jahr"), None, "Jahreszahl mit vier Ziffern ist keine Nummer");
        assert_eq!(numbered("12,5 Prozent"), None, "Komma: keine Nummer");
        assert_eq!(
            join_lines(["Ein Wort mit Tren-", "nung und Ende"]),
            "Ein Wort mit Trennung und Ende",
            "Trennung weg"
        );
        assert_eq!(join_lines(["Nord-", "Süd"]), "Nord- Süd", "Großbuchstabe danach: Bindestrich bleibt");
    }

    #[test]
    fn heading_sizes_become_levels_and_page_numbers_vanish() {
        let bbox = r#"<html><body><doc>
<page width="100" height="100"><flow>
<block><line><word yMin="0" yMax="20">Mein</word><word yMin="0" yMax="20">Buch</word></line></block>
<block><line><word yMin="0" yMax="15">Erstes</word><word yMin="0" yMax="15">Kapitel</word></line></block>
<block><line><word yMin="0" yMax="10">Ein</word><word yMin="0" yMax="10">langer</word><word yMin="0" yMax="10">Satz</word><word yMin="0" yMax="10">mit</word><word yMin="0" yMax="10">vielen</word><word yMin="0" yMax="10">Wörtern.</word></line></block>
<block><line><word yMin="0" yMax="12.5">Ein</word><word yMin="0" yMax="12.5">Abschnitt</word></line></block>
<block><line><word yMin="0" yMax="10">Noch</word><word yMin="0" yMax="10">mehr</word><word yMin="0" yMax="10">Text</word><word yMin="0" yMax="10">#hier.</word></line></block>
<block><line><word yMin="0" yMax="10">1</word></line></block>
</flow></page>
<page width="100" height="100"><flow>
<block><line><word yMin="0" yMax="15">Zweites</word><word yMin="0" yMax="15">Kapitel</word></line></block>
<block><line><word yMin="0" yMax="10">Schluss</word><word yMin="0" yMax="10">des</word><word yMin="0" yMax="10">Buches</word><word yMin="0" yMax="10">hier.</word></line></block>
</flow></page></doc></body></html>"#;
        let read = structure(bbox, &PdfInfo::default()).unwrap();
        let outline: Vec<(u8, &str)> = read.sections.iter().map(|s| (s.level, s.title.as_str())).collect();
        assert_eq!(
            outline,
            [(1, "Erstes Kapitel"), (2, "Ein Abschnitt"), (1, "Zweites Kapitel")],
            "größte Klasse = Titel des Dokuments, dann Ebenen nach Größe"
        );
        assert_eq!(read.title.as_deref(), Some("Mein Buch"), "Titel aus der größten Zeile auf Seite 1");
        assert!(read.sections[1].body.contains("\\#hier."), "Text maskiert: {}", read.sections[1].body);
        assert!(!read.sections[1].plain.contains(" 1"), "Seitenzahl entfernt: {}", read.sections[1].plain);
        assert!(
            read.warnings.iter().any(|w| w.code == "import-pdf-page-numbers"),
            "Warnung zur Seitenzahl: {:?}",
            read.warnings
        );
    }
}
