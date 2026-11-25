// SPDX-License-Identifier: AGPL-3.0-or-later
//! Word-Import mit Vorschau (Runde 32; Schnittstelle R32-S1, Besitz docx_import).
//!
//! Eine .docx-Datei wird gelesen ([`baruch_docx::read`]), ihre Gliederung samt Bearbeitern und Warnungen gezeigt und
//! erst nach ausdrücklicher Bestätigung des Menschen als Werk übernommen ([`crate::import::import_generated`]):
//! Fassung 1 ist der Stand **vor** allen verfolgten Änderungen; die Änderungen je Bearbeiter werden offene
//! Vorschläge, Kommentare werden Beiträge – beides unter `@word:<name>` ([`word_author`]) mit Modell
//! [`WORD_MODEL`], nie als aktuelle Fassung. Kein Import über MCP.
//!
//! - RPC (global, ohne Werk): [`METHODS`] – siehe docs/RPC.md „Runde 32 – Word“.
//! - Seiten: `GET /werke/import/word`, Formulare `POST /werke/import/word/vorschau|uebernehmen|verwerfen`
//!   ([`pages`], [`writes`], hinter `guard::same_origin`).
//! - CLI: `baruch-server import DATEI.docx [--uebernehmen] [--title …]` ([`cli_import`]); ohne `--uebernehmen` nur
//!   Vorschau (nichts geschrieben, auch kein Entwurf).
//!
//! Ablauf des Übernehmens ([`apply_prepared`]):
//! 1. Werk aus dem Original ([`baruch_docx::resolve::View::Original`]) über `import_generated` – ein Commit im
//!    Spiegel („Import aus <datei>“), jede Fassung 1 ist aktuell.
//! 2. Je Bearbeiter und geändertem Abschnitt ein offener Vorschlag (`Store::add_proposal`, gegen Fassung 1), Notiz
//!    „Änderungen aus Word von <Name>, <Datum>“; danach die Seitenzweige des Spiegels
//!    (`mirror::side_refs_after_proposal`, wie bei jedem Vorschlag eines Assistenten).
//! 3. Geänderte Überschriften und Kommentare als Beiträge (`Store::add_post`, wie `post.add` ohne eigenen Commit).
//!
//! Scheitert Schritt 2 oder 3 an einer Stelle, bleibt das Werk bestehen und die Antwort nennt, was fehlt (Warnung
//! `docx_vorschlag_abgelehnt` bzw. `docx_beitrag_abgelehnt`) – kein halbes Werk ohne Meldung.

mod draft;
mod pages;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use axum::Router;
use baruch_core::{Post, RevisionStatus, TextRef};
use baruch_docx::resolve::{HeadingChange, View, change_authors, proposal_view, resolve};
use baruch_docx::to_typst::{TypstSection, to_typst};
use baruch_docx::{Block, Change, ChangeKind, DocxDocument, DocxError, DocxWarning, Inline, Limits};
use baruch_store::ImportAsset;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::import::{ImportReport, ImportWarning, SkippedAsset};
use crate::rpc::{INTERNAL, INVALID_PARAMS, REFUSED, RpcError};

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &["docx_import.preview", "docx_import.get", "docx_import.apply", "docx_import.discard"];

/// Größte Word-Datei, die der Import annimmt (wie `baruch_docx::Limits::DEFAULT.max_file_bytes`).
pub const MAX_DOCX_BYTES: u64 = 64 * 1024 * 1024;

/// Grenze der Anfrage an `POST /werke/import/word/vorschau` (Datei samt Formularfeldern).
pub const UPLOAD_LIMIT: usize = MAX_DOCX_BYTES as usize + 64 * 1024;

/// Modell-Feld der Vorschläge aus Word (`Store::add_proposal` verlangt ein Modell).
pub const WORD_MODEL: &str = "word";

/// Ordner der Entwürfe unter dem Datenordner (`<data>/docx-drafts/<id>/`).
pub const DRAFT_DIR: &str = "docx-drafts";

/// Länge der Textprobe je Abschnitt in der Vorschau (Zeichen).
pub const SAMPLE_CHARS: usize = 160;

/// Größte Länge der zitierten Stelle eines Kommentars im Beitrag (Zeichen, Vertrag §2.3).
pub const MAX_QUOTE_CHARS: usize = 200;

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Endet der Dateiname (ohne Rücksicht auf Groß-/Kleinschreibung) auf `.docx` und hat davor einen Namen?
pub fn is_docx_name(file_name: &str) -> bool {
    const SUFFIX: &str = ".docx";
    let lower = file_name.to_lowercase();
    lower.len() > SUFFIX.len() && lower.ends_with(SUFFIX)
}

/// Größte Länge des Namensteils eines Handles `@word:<name>`.
const MAX_WORD_NAME_CHARS: usize = 40;

/// Persona-Handle eines Bearbeiters aus Word (Vertrag Runde 32 §0.5): `@word:<name>` mit dem Namen klein, ä→ae,
/// ö→oe, ü→ue, ß→ss, jede Folge anderer Zeichen außer a–z und 0–9 als ein `-`, ohne `-` am Rand, höchstens 40
/// Zeichen; bleibt nichts übrig, `@word:unbekannt`. Der Originalname steht in der Notiz der Fassung bzw. am Anfang
/// des Beitrags.
pub fn word_author(name: &str) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        match c {
            'a'..='z' | '0'..='9' => slug.push(c),
            'ä' => slug.push_str("ae"),
            'ö' => slug.push_str("oe"),
            'ü' => slug.push_str("ue"),
            'ß' => slug.push_str("ss"),
            _ if slug.ends_with('-') || slug.is_empty() => {}
            _ => slug.push('-'),
        }
    }
    // Nur ASCII: Zeichen und Bytes sind dasselbe.
    slug.truncate(MAX_WORD_NAME_CHARS);
    let slug = slug.trim_matches('-');
    if slug.is_empty() { "@word:unbekannt".to_owned() } else { format!("@word:{slug}") }
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Wörtlicher Name für Menschen (leer → „unbekannt“).
fn shown_name(name: &str) -> &str {
    if name.trim().is_empty() { "unbekannt" } else { name.trim() }
}

/// Datum (`JJJJ-MM-TT`) aus einem ISO-Zeitpunkt, sonst leer.
fn day(date: &str) -> &str {
    date.get(..10).unwrap_or("")
}

fn short(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > max { format!("{}…", text.chars().take(max).collect::<String>()) } else { text }
}

/// Dateiname ohne Ordner und ohne Endung.
fn stem(file_name: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    base.rsplit_once('.').map_or(base, |(stem, _)| stem).trim().to_owned()
}

// ---- Lesen und Planen (rein, ohne Datenbank) ---------------------------------------------------------------------

/// Was ein Bearbeiter beiträgt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPlan {
    /// Wörtlicher Name aus Word (kann leer sein).
    pub name: String,
    pub handle: String,
    pub insertions: usize,
    pub deletions: usize,
    /// Jüngster Zeitpunkt seiner Änderungen (ISO 8601) oder leer.
    pub date: String,
    /// Geänderte Abschnitte: Stelle (ab 0) und neuer Text ohne Überschriftszeile.
    pub bodies: Vec<(usize, String)>,
    /// Überschriften mit geändertem Wortlaut (weder eingefügt noch gelöscht).
    pub headings: Vec<HeadingChange>,
}

/// Eine gelesene Word-Datei, fertig geplant für Vorschau und Übernahme.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub document: DocxDocument,
    /// Abschnitte des Originals (Stand vor allen Änderungen); ein Vorspann ohne Überschrift hat einen leeren Titel.
    pub sections: Vec<TypstSection>,
    pub editors: Vec<EditorPlan>,
    /// Stelle je Kommentar: Abschnitt und kommentierter Wortlaut (leer = ohne auffindbare Stelle).
    pub places: HashMap<u32, (usize, String)>,
    pub warnings: Vec<DocxWarning>,
    pub footnotes: usize,
}

fn visit(blocks: &[Block], inline: &mut dyn FnMut(&Inline), mark: &mut dyn FnMut(&Change)) {
    fn inlines(content: &[Inline], inline: &mut dyn FnMut(&Inline)) {
        for item in content {
            inline(item);
            match item {
                Inline::Footnote { content }
                | Inline::Quote { content }
                | Inline::Link { content, .. }
                | Inline::Tracked { content, .. } => inlines(content, inline),
                Inline::Typst { shown, .. } => inlines(shown, inline),
                _ => {}
            }
        }
    }
    for block in blocks {
        match block {
            Block::Paragraph { content, mark: m }
            | Block::Part { content, mark: m }
            | Block::Heading { content, mark: m, .. } => {
                inlines(content, inline);
                m.iter().for_each(&mut *mark);
            }
            Block::Quote { blocks, attribution } => {
                visit(blocks, inline, mark);
                inlines(attribution, inline);
            }
            Block::List { items, .. } => {
                for item in items {
                    inlines(&item.content, inline);
                    item.mark.iter().for_each(&mut *mark);
                }
            }
            Block::Table { rows, .. } => rows.iter().flatten().for_each(|cell| visit(cell, inline, mark)),
            Block::Figure { caption, .. } => inlines(caption, inline),
            Block::Typst { shown, .. } => visit(shown, inline, mark),
            Block::PageBreak => {}
        }
    }
}

fn warn(warnings: &mut Vec<DocxWarning>, code: &str, message: String) {
    warnings.push(DocxWarning { code: code.to_owned(), message });
}

/// Datei lesen und alles planen: Abschnitte des Originals, je Bearbeiter die geänderten Abschnitte, Stellen der
/// Kommentare, Warnungen. Rein rechnend (blockiert; in `spawn_blocking` aufrufen).
pub fn prepare(bytes: &[u8]) -> Result<Prepared, DocxError> {
    let read = baruch_docx::read::read(bytes, &Limits::DEFAULT)?;
    let document = read.document;
    let mut warnings = read.warnings;
    let original = to_typst(&resolve(&document, View::Original));
    for warning in original.warnings {
        if !warnings.iter().any(|w| w.code == warning.code) {
            warnings.push(warning);
        }
    }
    let sections = original.sections;
    if sections.is_empty() {
        return Err(DocxError::Invalid("Die Datei enthält keinen Text.".into()));
    }
    let has_preamble = sections[0].title.is_empty();

    // Zahlen je Bearbeiter und Fußnoten.
    let mut stats: HashMap<String, (usize, usize, String)> = HashMap::new();
    let mut footnotes = 0;
    {
        let stats = std::cell::RefCell::new(&mut stats);
        let count = |change: &Change| {
            let mut stats = stats.borrow_mut();
            let entry = stats.entry(change.author.clone()).or_default();
            match change.kind {
                ChangeKind::Insert => entry.0 += 1,
                ChangeKind::Delete => entry.1 += 1,
            }
            if change.date > entry.2 {
                entry.2 = change.date.clone();
            }
        };
        visit(
            &document.blocks,
            &mut |inline| match inline {
                Inline::Footnote { .. } => footnotes += 1,
                Inline::Tracked { change, .. } => count(change),
                _ => {}
            },
            &mut |change| count(change),
        );
    }

    let mut places: HashMap<u32, (usize, String)> = HashMap::new();
    let mut note_places = |aligned: &[(usize, &TypstSection)]| {
        for (index, section) in aligned {
            for anchor in &section.anchors {
                places.entry(anchor.comment).or_insert_with(|| (*index, anchor.quote.clone()));
            }
        }
    };
    note_places(&sections.iter().enumerate().collect::<Vec<_>>());

    let mut editors = Vec::new();
    for name in change_authors(&document) {
        let (insertions, deletions, date) = stats.get(&name).cloned().unwrap_or_default();
        let (view, changes) = proposal_view(&document, &name);
        let out = to_typst(&view);
        let who = shown_name(&name).to_owned();
        let mut plan = EditorPlan {
            handle: word_author(&name),
            name,
            insertions,
            deletions,
            date,
            bodies: Vec::new(),
            headings: Vec::new(),
        };
        // Abschnitte des Vorschlagsstands auf die des Originals legen (Vorspann beachten).
        let mut proposed: Vec<(usize, &TypstSection)> = Vec::new();
        let own_preamble = out.sections.first().is_some_and(|s| s.title.is_empty());
        let empty = TypstSection::default();
        match (has_preamble, own_preamble) {
            (true, true) | (false, false) => proposed.extend(out.sections.iter().enumerate()),
            (true, false) => {
                proposed.push((0, &empty));
                proposed.extend(out.sections.iter().enumerate().map(|(i, s)| (i + 1, s)));
            }
            (false, true) => {
                warn(
                    &mut warnings,
                    "docx_vorspann_eingefuegt",
                    format!(
                        "{who} hat Text vor der ersten Überschrift eingefügt; er gehört zu keinem Abschnitt und ist \
                         nicht übernommen (Anfang: „{}“).",
                        short(&out.sections[0].body, 80)
                    ),
                );
                proposed.extend(out.sections.iter().skip(1).enumerate());
            }
        }
        if proposed.len() != sections.len() {
            warn(
                &mut warnings,
                "docx_gliederung_abweichend",
                format!(
                    "Die Änderungen von {who} ließen sich den Abschnitten nicht zuordnen ({} statt {}); kein Vorschlag \
                     angelegt.",
                    proposed.len(),
                    sections.len()
                ),
            );
        } else {
            note_places(&proposed);
            for (index, section) in &proposed {
                if section.body != sections[*index].body {
                    plan.bodies.push((*index, section.body.clone()));
                }
            }
        }
        for change in changes {
            if change.before.is_empty() {
                warn(
                    &mut warnings,
                    "docx_ueberschrift_eingefuegt",
                    format!(
                        "{who} hat die Überschrift „{}“ eingefügt; sie legt keinen Abschnitt an, ihr Text steht fett \
                         im Vorschlag zum vorangehenden Abschnitt.",
                        change.after
                    ),
                );
            } else if change.after.is_empty() {
                warn(
                    &mut warnings,
                    "docx_ueberschrift_geloescht",
                    format!("{who} hat die Überschrift „{}“ gelöscht; sie bleibt bestehen.", change.before),
                );
            } else if change.section < sections.len() {
                plan.headings.push(change);
            }
        }
        editors.push(plan);
    }

    let lost = document.comments.iter().filter(|c| !places.contains_key(&c.id) && c.reply_to.is_none()).count();
    if lost > 0 {
        warn(
            &mut warnings,
            "docx_kommentar_ohne_stelle",
            format!("{lost} Kommentar(e) ohne auffindbare Stelle im Text stehen am ersten Abschnitt."),
        );
    }
    Ok(Prepared { document, sections, editors, places, warnings, footnotes })
}

impl Prepared {
    /// Titel des Werks: Wunsch, sonst Titel der Datei, sonst Dateiname ohne Endung.
    pub fn title(&self, wanted: Option<&str>, file_name: &str) -> String {
        let wanted = wanted.map(str::trim).filter(|t| !t.is_empty());
        let own = Some(self.document.meta.title.trim()).filter(|t| !t.is_empty());
        wanted.or(own).map_or_else(|| stem(file_name), str::to_owned)
    }

    fn section_title<'a>(&'a self, index: usize, work_title: &'a str) -> &'a str {
        let title = self.sections[index].title.as_str();
        if title.is_empty() { work_title } else { title }
    }

    /// Antwort von `docx_import.preview` (ohne `draft`).
    pub fn preview_json(&self, file_name: &str, title: &str) -> Value {
        let sections: Vec<Value> = (0..self.sections.len())
            .map(|i| {
                let s = &self.sections[i];
                json!({
                    "level": s.level,
                    "numbered": s.numbered,
                    "title": self.section_title(i, title),
                    "chars": s.body.chars().count(),
                    "sample": short(&s.body, SAMPLE_CHARS),
                })
            })
            .collect();
        let editors: Vec<Value> = self
            .editors
            .iter()
            .map(|e| {
                json!({
                    "name": e.name,
                    "handle": e.handle,
                    "insertions": e.insertions,
                    "deletions": e.deletions,
                    "sections": e.bodies.len(),
                })
            })
            .collect();
        let warnings: Vec<Value> =
            self.warnings.iter().map(|w| json!({ "code": w.code, "message": w.message })).collect();
        json!({
            "file_name": file_name,
            "title": title,
            "sections": sections,
            "editors": editors,
            "comments": self.document.comments.len(),
            "footnotes": self.footnotes,
            "images": self.document.images.len(),
            "warnings": warnings,
        })
    }

    /// Typst-Quelltext des Werks aus dem Original.
    fn source(&self, title: &str) -> String {
        let literal = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
        let meta = &self.document.meta;
        let mut out = format!("#set document(title: {}", literal(title));
        if !meta.author.trim().is_empty() {
            out.push_str(&format!(", author: {}", literal(meta.author.trim())));
        }
        out.push_str(")\n");
        let lang: String = meta.lang.chars().take_while(char::is_ascii_alphabetic).collect::<String>().to_lowercase();
        if lang.len() == 2 {
            out.push_str(&format!("#set text(lang: \"{lang}\")\n"));
        }
        for (index, section) in self.sections.iter().enumerate() {
            let heading = self.section_title(index, title);
            let line = if section.level >= 1 && !section.numbered {
                baruch_typst::unnumbered_line(section.level, heading)
            } else {
                baruch_typst::heading_line(section.level, heading, None)
            };
            out.push('\n');
            out.push_str(line.trim_end_matches('\n'));
            out.push('\n');
            if !section.body.is_empty() {
                out.push('\n');
                out.push_str(&section.body);
                out.push('\n');
            }
        }
        out
    }
}

// ---- Übernehmen --------------------------------------------------------------------------------------------------

/// Ergebnis einer Übernahme.
#[derive(Debug, Clone)]
pub struct Applied {
    pub report: ImportReport,
    pub sections: usize,
    pub proposals: usize,
    pub posts: usize,
    /// Alle Hinweise: aus der Datei, vom Import und vom Anlegen der Vorschläge und Beiträge.
    pub warnings: Vec<DocxWarning>,
    /// Zahl der Hinweise, die erst beim Übernehmen entstanden (die übrigen zeigte schon die Vorschau).
    pub new_warnings: usize,
}

fn post_of(author: &str, body: String, refs: Vec<TextRef>) -> Post {
    Post { id: 0, created: String::new(), author: author.to_owned(), body, sources: Vec::new(), refs }
}

/// Text des Vorschlags: die aktuelle Fassung, in der der Text des Abschnitts ersetzt ist (Überschriftszeile bleibt).
fn proposal_body(current: &str, old: &str, new: &str) -> String {
    if !old.is_empty() && current.contains(old) {
        let replaced = current.replacen(old, new, 1);
        return format!("{}\n", replaced.trim_end());
    }
    // Der Text steht nicht wörtlich in der Fassung (leer, oder der Import hat ihn angepasst): Überschriftszeile der
    // aktuellen Fassung, dahinter der neue Text.
    let head = current.lines().next().unwrap_or("").trim_end();
    if new.is_empty() { format!("{head}\n") } else { format!("{head}\n\n{new}\n") }
}

/// Werk anlegen, dann Vorschläge und Beiträge (siehe Modulkopf). `changes`/`comments` schalten Schritt 2 bzw. die
/// Kommentare ab.
pub async fn apply_prepared(
    state: &AppState,
    prepared: &Prepared,
    file_name: &str,
    wanted_title: Option<&str>,
    changes: bool,
    comments: bool,
) -> Result<Applied, RpcError> {
    let title = prepared.title(wanted_title, file_name);
    let source = prepared.source(&title);
    let mut assets = Vec::new();
    let mut skipped = Vec::new();
    let mut total = 0u64;
    for image in &prepared.document.images {
        match crate::import::check_image(&image.bytes, &mut total) {
            Ok(()) => assets.push(ImportAsset {
                path: image.path.clone(),
                file_name: image.file_name.clone(),
                bytes: image.bytes.clone(),
            }),
            Err(reason) => skipped.push(SkippedAsset { path: image.path.clone(), reason }),
        }
    }
    let import_warnings: Vec<ImportWarning> = prepared
        .warnings
        .iter()
        .map(|w| ImportWarning { code: w.code.clone(), message: w.message.clone(), line: None })
        .collect();
    let known = import_warnings.len();
    let report =
        crate::import::import_generated(state, &source, file_name, Some(&title), assets, skipped, import_warnings)
            .await?;
    let work = report.work;
    let mut warnings: Vec<DocxWarning> =
        report.warnings.iter().map(|w| DocxWarning { code: w.code.clone(), message: w.message.clone() }).collect();
    for s in &report.skipped {
        warn(&mut warnings, "docx_bild_uebersprungen", format!("Bild „{}“ übersprungen: {}.", s.path, s.reason));
    }
    if let Some(error) = &report.mirror {
        warn(&mut warnings, "docx_spiegel", format!("Der Git-Spiegel ließ sich nicht schreiben: {error}"));
    }
    let (mut proposals, mut posts) = (0, 0);

    let held = state.write_lock.lock().await;
    let loaded = state.store.load_work(work).await?;
    let aligned = loaded.nodes.len() == prepared.sections.len();
    if !aligned && (changes || comments) {
        warn(
            &mut warnings,
            "docx_gliederung_abweichend",
            format!(
                "Das Werk hat {} Abschnitte, die Datei {}; Vorschläge und Beiträge ließen sich nicht zuordnen und \
                 sind nicht angelegt.",
                loaded.nodes.len(),
                prepared.sections.len()
            ),
        );
    }
    let first_body = |index: usize| -> String {
        loaded.nodes[index]
            .revisions
            .iter()
            .find(|r| r.status == RevisionStatus::Current)
            .map(|r| r.body.clone())
            .unwrap_or_default()
    };
    if aligned && changes {
        for editor in &prepared.editors {
            let who = shown_name(&editor.name);
            let note = if day(&editor.date).is_empty() {
                format!("Änderungen aus Word von {who}")
            } else {
                format!("Änderungen aus Word von {who}, {}", day(&editor.date))
            };
            for (index, body) in &editor.bodies {
                let text = proposal_body(&first_body(*index), &prepared.sections[*index].body, body);
                match state.store.add_proposal(work, *index, &text, &editor.handle, WORD_MODEL, &note).await {
                    Ok(_) => proposals += 1,
                    Err(e) => warn(
                        &mut warnings,
                        "docx_vorschlag_abgelehnt",
                        format!(
                            "Vorschlag von {who} zu „{}“ nicht angelegt: {e}",
                            prepared.section_title(*index, &title)
                        ),
                    ),
                }
            }
            for change in &editor.headings {
                let body = format!(
                    "Überschrift in Word geändert: „{}“ → „{}“\n\n(aus Word von {who}{}; die Gliederung bleibt, wie \
                     sie ist)",
                    change.before,
                    change.after,
                    if day(&editor.date).is_empty() { String::new() } else { format!(", {}", day(&editor.date)) }
                );
                match state.store.add_post(work, change.section, &post_of(&editor.handle, body, Vec::new())).await {
                    Ok(_) => posts += 1,
                    Err(e) => warn(
                        &mut warnings,
                        "docx_beitrag_abgelehnt",
                        format!("Beitrag zur Überschrift „{}“ nicht angelegt: {e}", change.before),
                    ),
                }
            }
        }
    }
    if aligned && comments {
        let all = &prepared.document.comments;
        let ids: HashSet<u32> = all.iter().map(|c| c.id).collect();
        // Oberste Kommentare in der Reihenfolge der Datei, Antworten direkt dahinter.
        let mut order: Vec<(&baruch_docx::Comment, Option<&baruch_docx::Comment>)> = Vec::new();
        for comment in all.iter().filter(|c| c.reply_to.is_none_or(|parent| !ids.contains(&parent) || parent == c.id)) {
            order.push((comment, None));
            let mut thread = vec![comment.id];
            let mut at = 0;
            while at < thread.len() {
                let current = thread[at];
                for reply in all.iter().filter(|r| r.reply_to == Some(current) && r.id != current) {
                    if !thread.contains(&reply.id) {
                        thread.push(reply.id);
                        order.push((reply, all.iter().find(|c| c.id == current)));
                    }
                }
                at += 1;
            }
        }
        for (comment, parent) in order {
            let root = parent.map_or(comment.id, |_| {
                // Stelle der Antwort = Stelle des Kommentars, auf den sie (mittelbar) antwortet.
                let mut id = comment.id;
                let mut steps = 0;
                while let Some(up) =
                    all.iter().find(|c| c.id == id).and_then(|c| c.reply_to).filter(|up| ids.contains(up))
                    && steps < all.len()
                {
                    id = up;
                    steps += 1;
                }
                id
            });
            let (index, quote) = prepared
                .places
                .get(&comment.id)
                .or_else(|| prepared.places.get(&root))
                .cloned()
                .unwrap_or((0, String::new()));
            let who = shown_name(&comment.author);
            let when = if day(&comment.date).is_empty() { String::new() } else { format!(", {}", day(&comment.date)) };
            let mut body = match parent {
                Some(parent) => format!("Antwort auf {} – aus Word von {who}{when}\n", shown_name(&parent.author)),
                None => format!("Kommentar aus Word von {who}{when}\n"),
            };
            if !quote.is_empty() {
                body.push_str(&format!("\n> {}\n", short(&quote, MAX_QUOTE_CHARS)));
            }
            body.push_str(&format!("\n{}", comment.text));
            let refs = if !quote.is_empty() && first_body(index).contains(&quote) {
                vec![TextRef { node: index, rev: 1, quote: quote.clone() }]
            } else {
                Vec::new()
            };
            match state.store.add_post(work, index, &post_of(&word_author(&comment.author), body, refs)).await {
                Ok(_) => posts += 1,
                Err(e) => {
                    warn(&mut warnings, "docx_beitrag_abgelehnt", format!("Kommentar von {who} nicht angelegt: {e}"));
                }
            }
        }
    }
    if proposals > 0 {
        crate::mirror::side_refs_after_proposal(state, &held, work).await;
    }
    drop(held);
    let new_warnings = warnings.len().saturating_sub(known);
    Ok(Applied { report, sections: prepared.sections.len(), proposals, posts, warnings, new_warnings })
}

/// Antwort von `docx_import.apply`.
pub fn applied_json(applied: &Applied) -> Value {
    let warnings: Vec<Value> =
        applied.warnings.iter().map(|w| json!({ "code": w.code, "message": w.message })).collect();
    json!({
        "work": applied.report.work,
        "slug": applied.report.slug,
        "sections": applied.sections,
        "proposals": applied.proposals,
        "posts": applied.posts,
        "warnings": warnings,
    })
}

async fn prepare_blocking(bytes: Vec<u8>) -> Result<(Prepared, Vec<u8>), RpcError> {
    tokio::task::spawn_blocking(move || prepare(&bytes).map(|prepared| (prepared, bytes)))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Lesen abgebrochen: {e}")))?
        .map_err(RpcError::from)
}

fn too_large() -> String {
    format!("Die Datei ist größer als {} MiB.", MAX_DOCX_BYTES / (1024 * 1024))
}

/// Vorschau: Datei lesen und als Entwurf ablegen; sonst entsteht nichts.
pub async fn preview(
    state: &AppState,
    file_name: &str,
    bytes: Vec<u8>,
    title: Option<&str>,
) -> Result<draft::Draft, RpcError> {
    if !is_docx_name(file_name) {
        return Err(invalid(format!(
            "„{file_name}“ ist keine Word-Datei (.docx); das alte Format .doc bitte in Word als .docx speichern."
        )));
    }
    if bytes.len() as u64 > MAX_DOCX_BYTES {
        return Err(invalid(too_large()));
    }
    let (prepared, bytes) = prepare_blocking(bytes).await?;
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name).to_owned();
    let wanted = title.map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned);
    let shown_title = prepared.title(wanted.as_deref(), &base);
    let id = draft::new_id();
    let mut preview = prepared.preview_json(&base, &shown_title);
    preview["draft"] = json!(id);
    let draft = draft::Draft { id, created: draft::now(), file_name: base, wanted_title: wanted, preview };
    let data = state.data.clone();
    let stored = draft.clone();
    tokio::task::spawn_blocking(move || {
        draft::clean(&data);
        draft::save(&data, &stored, &bytes)
    })
    .await
    .map_err(|e| RpcError::new(INTERNAL, format!("Entwurf abgebrochen: {e}")))?
    .map_err(|e| RpcError::new(INTERNAL, e))?;
    Ok(draft)
}

/// Entwurf laden; unbekannt → `INVALID_PARAMS`.
pub async fn load_draft(state: &AppState, id: &str) -> Result<draft::Draft, RpcError> {
    let data = state.data.clone();
    let owned = id.to_owned();
    let loaded = tokio::task::spawn_blocking(move || draft::load(&data, &owned))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Lesen abgebrochen: {e}")))?;
    loaded.map_err(|e| match e {
        draft::LoadError::Unknown => invalid(format!(
            "Entwurf „{id}“ gibt es nicht (verworfen, schon übernommen oder älter als 24 Stunden); die Datei bitte neu \
             lesen."
        )),
        draft::LoadError::Broken(m) => RpcError::new(INTERNAL, m),
    })
}

/// Entwürfe, die gerade übernommen werden (zweimal zugleich → abgelehnt).
static APPLYING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

struct ApplyingGuard(String);

impl Drop for ApplyingGuard {
    fn drop(&mut self) {
        APPLYING.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0);
    }
}

/// Übernehmen eines gespeicherten Entwurfs; danach ist der Entwurf weg.
pub async fn apply(
    state: &AppState,
    id: &str,
    title: Option<&str>,
    changes: bool,
    comments: bool,
) -> Result<Applied, RpcError> {
    let draft = load_draft(state, id).await?;
    {
        let mut applying = APPLYING.lock().unwrap_or_else(|e| e.into_inner());
        if !applying.insert(id.to_owned()) {
            return Err(RpcError::new(REFUSED, format!("Entwurf „{id}“ wird gerade übernommen.")));
        }
    }
    let _guard = ApplyingGuard(id.to_owned());
    let data = state.data.clone();
    let owned = id.to_owned();
    let bytes = tokio::task::spawn_blocking(move || draft::load_source(&data, &owned))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Lesen abgebrochen: {e}")))?
        .map_err(|e| RpcError::new(INTERNAL, e))?;
    let (prepared, _) = prepare_blocking(bytes).await?;
    let wanted = title.map(str::trim).filter(|t| !t.is_empty()).or(draft.wanted_title.as_deref());
    let applied = apply_prepared(state, &prepared, &draft.file_name, wanted, changes, comments).await?;
    let data = state.data.clone();
    let owned = id.to_owned();
    let _ = tokio::task::spawn_blocking(move || draft::remove(&data, &owned)).await;
    Ok(applied)
}

/// Entwurf verwerfen; `true`, wenn es ihn gab.
pub async fn discard(state: &AppState, id: &str) -> Result<bool, RpcError> {
    if !draft::valid_id(id) {
        return Err(invalid("draft: 32 Kleinbuchstaben-Hex (aus docx_import.preview)."));
    }
    let data = state.data.clone();
    let owned = id.to_owned();
    tokio::task::spawn_blocking(move || draft::remove(&data, &owned))
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Löschen abgebrochen: {e}")))
}

/// Zahl der Entwürfe im Datenordner (Tests, Anzeige).
pub fn draft_count(state: &AppState) -> usize {
    draft::count(&state.data)
}

// ---- RPC ---------------------------------------------------------------------------------------------------------

fn params<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| invalid(e.to_string()))
}

#[derive(Deserialize)]
struct PreviewParams {
    file_name: String,
    data_base64: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct DraftParams {
    draft: String,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct ApplyParams {
    draft: String,
    title: Option<String>,
    #[serde(default = "yes")]
    changes: bool,
    #[serde(default = "yes")]
    comments: bool,
}

/// RPC-Aufruf einer Methode aus [`METHODS`].
pub async fn call(state: &AppState, method: &str, params_value: Value) -> Result<Value, RpcError> {
    match method {
        "docx_import.preview" => {
            let p: PreviewParams = params(params_value)?;
            let max_base64 = MAX_DOCX_BYTES.div_ceil(3) * 4 + 4;
            if p.data_base64.len() as u64 > max_base64 {
                return Err(invalid(format!("data_base64: {}", too_large())));
            }
            let bytes =
                baruch_store::decode_base64(p.data_base64.trim()).map_err(|e| invalid(format!("data_base64: {e}")))?;
            Ok(preview(state, &p.file_name, bytes, p.title.as_deref()).await?.preview)
        }
        "docx_import.get" => {
            let p: DraftParams = params(params_value)?;
            Ok(load_draft(state, &p.draft).await?.preview)
        }
        "docx_import.apply" => {
            let p: ApplyParams = params(params_value)?;
            let applied = apply(state, &p.draft, p.title.as_deref(), p.changes, p.comments).await?;
            Ok(applied_json(&applied))
        }
        "docx_import.discard" => {
            let p: DraftParams = params(params_value)?;
            discard(state, &p.draft).await?;
            Ok(json!({ "discarded": true }))
        }
        other => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("Unbekannte Methode: {other}"))),
    }
}

// ---- CLI ---------------------------------------------------------------------------------------------------------

/// Angaben der Kommandozeile.
#[derive(Debug, Clone, Default)]
pub struct CliDocxImport {
    pub title: Option<String>,
    /// `--uebernehmen`: Werk samt Vorschlägen und Beiträgen anlegen; sonst nur Vorschau.
    pub apply: bool,
}

fn level_mark(level: u8, numbered: bool) -> String {
    match level {
        0 => "Teil".to_owned(),
        level => format!("{}{}", "=".repeat(usize::from(level)), if numbered { "" } else { " (ohne Nummer)" }),
    }
}

/// `baruch-server import DATEI.docx [--uebernehmen] [--title T]`: `Ok(text)` → Standardausgabe, Exit 0;
/// `Err(text)` → Fehlerausgabe, Exit 1.
pub async fn cli_import(state: &AppState, file: &std::path::Path, options: &CliDocxImport) -> Result<String, String> {
    let shown = file.display().to_string();
    let meta = tokio::fs::metadata(file).await.map_err(|e| format!("{shown} lässt sich nicht lesen: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{shown} ist keine Datei"));
    }
    if meta.len() > MAX_DOCX_BYTES {
        return Err(format!("{shown}: {}", too_large()));
    }
    let file_name = file
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("{shown}: Dateiname fehlt oder ist kein UTF-8"))?
        .to_owned();
    let bytes = tokio::fs::read(file).await.map_err(|e| format!("{shown} lässt sich nicht lesen: {e}"))?;
    let (prepared, _) = prepare_blocking(bytes).await.map_err(|e| format!("{shown}: {}", e.message))?;
    let title = prepared.title(options.title.as_deref(), &file_name);
    let mut out = String::new();
    let warnings = |out: &mut String, warnings: &[DocxWarning]| {
        for w in warnings {
            out.push_str(&format!("Hinweis [{}]: {}\n", w.code, w.message));
        }
    };
    if !options.apply {
        out.push_str(&format!("Vorschau von {file_name} – Titel „{title}“ (nichts angelegt)\n"));
        out.push_str(&format!("Gliederung ({} Abschnitte):\n", prepared.sections.len()));
        for (index, section) in prepared.sections.iter().enumerate() {
            out.push_str(&format!(
                "  {} {} ({} Zeichen)\n",
                level_mark(section.level, section.numbered),
                prepared.section_title(index, &title),
                section.body.chars().count()
            ));
        }
        if prepared.editors.is_empty() {
            out.push_str("Bearbeiter: keine verfolgten Änderungen\n");
        } else {
            out.push_str("Bearbeiter (jeder geänderte Abschnitt wird ein offener Vorschlag):\n");
            for e in &prepared.editors {
                out.push_str(&format!(
                    "  {} → {}: {} Einfügung(en), {} Löschung(en), {} Abschnitt(e)\n",
                    shown_name(&e.name),
                    e.handle,
                    e.insertions,
                    e.deletions,
                    e.bodies.len()
                ));
            }
        }
        out.push_str(&format!(
            "Kommentare: {}, Fußnoten: {}, Bilder: {}\n",
            prepared.document.comments.len(),
            prepared.footnotes,
            prepared.document.images.len()
        ));
        warnings(&mut out, &prepared.warnings);
        out.push_str("Änderungen und Kommentare werden nie zur aktuellen Fassung. Anlegen mit --uebernehmen.\n");
        return Ok(out);
    }
    let applied = apply_prepared(state, &prepared, &file_name, options.title.as_deref(), true, true)
        .await
        .map_err(|e| format!("{shown}: {}", e.message))?;
    out.push_str(&format!(
        "Werk {} „{}“ ({}) angelegt: {} Abschnitte, {} Vorschläge, {} Beiträge\n",
        applied.report.work,
        applied.report.title,
        applied.report.slug,
        applied.sections,
        applied.proposals,
        applied.posts
    ));
    warnings(&mut out, &applied.warnings);
    Ok(out)
}

/// Seiten (GET), gemischt in `lib.rs` bei den Seiten: `GET /werke/import/word`.
pub fn pages() -> Router<Arc<AppState>> {
    pages::pages()
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    pages::writes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docx_names_end_in_docx_with_a_name_before() {
        assert!(is_docx_name("buch.docx"), ".docx");
        assert!(is_docx_name("BUCH.DOCX"), "Groß/klein egal: Dateien anderer Systeme heißen auch einmal so");
        assert!(is_docx_name("ordner/mein buch.docx"), "ein Pfad davor stört nicht");
        assert!(!is_docx_name(".docx"), "ohne Namen davor ist es keine Datei, die sich benennen ließe");
        assert!(!is_docx_name("buch.docx.typ"), "entscheidend ist die letzte Endung");
        assert!(!is_docx_name("buch.doc"), "das alte .doc-Format liest Baruch nicht");
        assert!(!is_docx_name("buch.pdf"), "PDF gehört zu pdf_import");
    }

    #[test]
    fn word_authors_become_handles_by_the_rule_of_the_contract() {
        assert_eq!(word_author("Erika Beispiel"), "@word:erika-beispiel", "klein, Leerzeichen wird Bindestrich");
        assert_eq!(word_author("Jörg Müßig-Äpfel"), "@word:joerg-muessig-aepfel", "Umlaute und ß umschrieben");
        assert_eq!(
            word_author("  Dr.  A. — B!  "),
            "@word:dr-a-b",
            "jede Folge anderer Zeichen ist ein Bindestrich, keiner am Rand"
        );
        assert_eq!(
            word_author("ÉMILE"),
            "@word:mile",
            "andere Buchstaben als a–z entfallen (am Rand ohne Bindestrich)"
        );
        assert_eq!(word_author(""), "@word:unbekannt", "ohne Namen");
        assert_eq!(word_author("…?!"), "@word:unbekannt", "bleibt nichts übrig, heißt der Bearbeiter unbekannt");
        let long = word_author(&"ab ".repeat(30));
        assert_eq!(long, format!("@word:{}", "ab-".repeat(13) + "a"), "höchstens 40 Zeichen im Namen");
        let cut = word_author(&format!("{} z", "a".repeat(39)));
        assert_eq!(cut, format!("@word:{}", "a".repeat(39)), "nach dem Kürzen steht kein Bindestrich am Rand");
    }

    #[test]
    fn only_the_four_methods_belong_to_this_module() {
        for method in METHODS {
            assert!(handles(method), "{method} gehört dem Word-Import");
        }
        assert!(!handles("work.docx_export"), "der Export gehört docx_export");
        assert!(!handles("source_import.preview"), "PDF/EPUB gehört pdf_import");
    }

    #[test]
    fn the_proposal_keeps_the_heading_line_and_replaces_only_the_text() {
        assert_eq!(
            proposal_body("= Ankunft\n\nAlter Text.\n", "Alter Text.", "Neuer Text."),
            "= Ankunft\n\nNeuer Text.\n",
            "der Text des Abschnitts wird ersetzt, die Überschriftszeile bleibt wörtlich"
        );
        assert_eq!(
            proposal_body("= Ankunft\n", "", "Neuer Text."),
            "= Ankunft\n\nNeuer Text.\n",
            "ein bisher leerer Abschnitt bekommt den Text hinter der Überschrift"
        );
        assert_eq!(
            proposal_body("= Ankunft\n\nVom Import angepasster Text.\n", "Alter Text.", "Neuer Text."),
            "= Ankunft\n\nNeuer Text.\n",
            "steht der alte Text nicht wörtlich in der Fassung, gilt ihre Überschriftszeile und dahinter der neue Text \
             (nichts wird verdoppelt)"
        );
        assert_eq!(
            proposal_body("= Ankunft\n\nAlter Text.\n", "Alter Text.", ""),
            "= Ankunft\n",
            "hat der Bearbeiter alles gelöscht, bleibt die Überschrift allein"
        );
    }

    #[test]
    fn file_names_give_the_title_only_as_a_last_resort() {
        assert_eq!(stem("ordner/Mein Buch.docx"), "Mein Buch", "Dateiname ohne Ordner und Endung");
        assert_eq!(stem("ohne-endung"), "ohne-endung", "ohne Endung bleibt der Name");
        assert_eq!(day("2026-11-20T09:30:00Z"), "2026-11-20", "vom Zeitpunkt zählt für die Notiz der Tag");
        assert_eq!(day(""), "", "ohne Datum bleibt es leer");
    }
}
