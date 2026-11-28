// SPDX-License-Identifier: AGPL-3.0-or-later
//! Create the files of the EPUB from the input (ownership "epub", Round 20a).
//!
//! - **Split** (contract §0.2): every part page (level 0) and every chapter (level 1) a file of its own
//!   `text/k{key}.xhtml` in the spine; sections of levels 2/3 in the file of their chapter; sections before the
//!   first part/chapter in `text/front.xhtml`. Plus `break-before` in the CSS
//!   ([`baruch_typst::typeset::epub_style`]).
//! - **Text** through the block model of baruch-wysiwyg ([`crate::xhtml`]); islands other than images are left out
//!   and reported as warning `ISLAND_OMITTED`.
//! - **Footnotes** as a popup (`noteref`/`aside` at the end of the chapter file) or as endnotes
//!   (`text/notes.xhtml`).
//! - **Navigation**: `nav.xhtml` (table of contents and landmarks) and `toc.ncx` (EPUB 2 fallback,
//!   `spine toc="ncx"`).
//! - **Front matter** (with `front_matter`): title page, imprint (`imprint`), dedication – as text.
//! - **Cover**: `images/cover.<ext>` (`properties="cover-image"`, `<meta name="cover">`) and `text/cover.xhtml` as
//!   the first file in the spine.
//! - **Metadata** only as escaped text: identifier, title, subtitle, author, language, ISBN (`urn:isbn:`),
//!   publisher, year, rights, image credit, description, `dcterms:modified` and accessibility details (schema.org).
//! - **Work template**: show rules with `pagebreak` → `TEMPLATE_PAGEBREAK_MAPPED` (info) or
//!   `TEMPLATE_PAGEBREAK_UNMAPPED` (warning), see [`crate::template`].
//! - **Index and glossary** (Round 31b, only with active settings in `back_matter`): `text/glossary.xhtml`
//!   (`epub:type="glossary"`, `<dt epub:type="glossterm" id="baruch-gl-UUID">`, `<dd epub:type="glossdef">`) and
//!   per index from `index_pages` `text/index-{slug}.xhtml` (`epub:type="index"`, headwords sorted as in the
//!   typeset result through [`baruch_typst::index::overview`], per section one link `index-locator` to the anchor
//!   of the first mark there, „siehe“/„siehe auch“). Position according to `glossary_page` (before/after the
//!   indexes), at the end of the spine and in the table of contents. The first mention (projection) links to the
//!   entry (`glossref`) when there is a glossary page.
//!
//! Deterministic: orders follow the input, no time and no randomness.

use baruch_core::book_text::book_t;
use baruch_typst::typeset::{FootnoteNumbering, FootnoteRestart, TypesetSettings};

use crate::template::{
    TEMPLATE_PAGEBREAK_MAPPED, TEMPLATE_PAGEBREAK_UNMAPPED, is_language_tag, pagebreak_rules, template_chapter_start,
};
use crate::xhtml::{
    ImageTarget, Links, Note, NoteTarget, PlacedMark, Renderer, endnote_list_in, escape_text, footnote_list_in,
    normalize_path,
};
use crate::{
    EpubBook, EpubError, EpubImage, EpubInput, EpubItem, EpubSection, EpubWarning, FootnoteMode, ItemKind, TocEntry,
};

const XHTML: &str = "application/xhtml+xml";
const NCX: &str = "application/x-dtbncx+xml";

/// Base style before the rules from the typesetting settings (which come after it and thus take precedence).
pub const BASE_CSS: &str = "/* Baruch: EPUB-Grundstil */
section.baruch-part { text-align: center; }
h1.baruch-part-title { margin-top: 30%; }
section.baruch-scene, section.baruch-work { margin: 1em 0; }
section.baruch-work { padding: 0.5em 1em; border-left: 0.2em solid #999999; }
blockquote.baruch-quote { margin: 1em 1.5em; }
footer.baruch-attribution { text-align: end; font-style: italic; }
figure.baruch-figure { margin: 1em 0; text-align: center; }
img.baruch-image, img.baruch-inline-image, img.baruch-cover-image { max-width: 100%; height: auto; }
img.baruch-inline-image { max-height: 1.2em; vertical-align: middle; }
figcaption { font-size: 0.9em; font-style: italic; }
.baruch-missing { font-family: monospace; }
a.baruch-noteref { text-decoration: none; }
hr.baruch-notes-rule { width: 30%; margin: 2em auto 0.5em 0; }
ol.baruch-footnotes { font-size: 0.85em; }
section.baruch-cover { text-align: center; margin: 0; padding: 0; }
img.baruch-cover-image { max-height: 100%; }
section.baruch-titlepage { text-align: center; }
p.baruch-title { font-size: 1.8em; font-weight: bold; margin-top: 25%; }
p.baruch-subtitle { font-size: 1.2em; }
p.baruch-author { margin-top: 2em; }
p.baruch-publisher { margin-top: 4em; }
section.baruch-imprint p { font-size: 0.85em; text-indent: 0; }
section.baruch-dedication { text-align: center; font-style: italic; margin-top: 30%; }
";

/// A file with text and the sections in it.
struct TextFile<'a> {
    id: String,
    href: String,
    kind: ItemKind,
    sections: Vec<&'a EpubSection>,
}

impl TextFile<'_> {
    /// File name within `text/`.
    fn name(&self) -> &str {
        self.href.strip_prefix("text/").unwrap_or(&self.href)
    }
}

fn warning(code: &str, message: String, key: Option<i64>) -> EpubWarning {
    EpubWarning { code: code.to_owned(), message, key }
}

/// EPUB files for `input`. Without a section → [`EpubError::NoSections`]; without identifier or timestamp →
/// [`EpubError::Invalid`].
pub fn build(input: &EpubInput) -> Result<EpubBook, EpubError> {
    if input.sections.is_empty() {
        return Err(EpubError::NoSections);
    }
    let meta = &input.meta;
    if meta.identifier.trim().is_empty() {
        return Err(EpubError::Invalid("identifier fehlt (urn:uuid:…)".into()));
    }
    if !modified_ok(meta.modified.trim()) {
        return Err(EpubError::Invalid(format!(
            "modified „{}“ hat nicht die Form JJJJ-MM-TTThh:mm:ssZ",
            meta.modified.trim()
        )));
    }
    let mut warnings = Vec::new();
    // Language: metadata, else the rule of contract R39b §2.1 (typesetting setting, then for a German work
    // `#set text(lang: …)` of the template or "de", for an English work "en").
    let wl = input.work_language;
    let t = |key: &str| book_t(wl, key, &[]);
    let fallback =
        || crate::template::book_language(input.typeset.lang.as_deref(), &input.template, input.work_language);
    let lang = match meta.language.trim() {
        "" => fallback(),
        tag if is_language_tag(tag) => tag.to_owned(),
        other => {
            warnings.push(warning(
                "LANGUAGE_INVALID",
                format!("Sprache „{other}“ ist kein gültiges Sprachkürzel; das EPUB nimmt „de“."),
                None,
            ));
            "de".to_owned()
        }
    };
    let title = if meta.title.trim().is_empty() { t("book.untitled") } else { meta.title.trim().to_owned() };

    // Work template: page breaks from show rules.
    let mut typeset = input.typeset.clone();
    for rule in pagebreak_rules(&input.template) {
        if rule.mapped {
            warnings.push(warning(
                TEMPLATE_PAGEBREAK_MAPPED,
                format!(
                    "Werkvorlage Zeile {}: Die Show-Regel mit Seitenumbruch ({}) ist im EPUB abgebildet – jedes Kapitel \
                     und jeder Teil beginnt eine eigene Datei mit break-before.",
                    rule.line, rule.text
                ),
                None,
            ));
        } else {
            warnings.push(warning(
                TEMPLATE_PAGEBREAK_UNMAPPED,
                format!(
                    "Werkvorlage Zeile {}: Die Show-Regel mit Seitenumbruch ({}) lässt sich im EPUB nicht abbilden; \
                     dort gibt es Umbrüche nur vor Kapiteln und Teilen.",
                    rule.line, rule.text
                ),
                None,
            ));
        }
    }
    if typeset.chapter_start.is_none() {
        typeset.chapter_start = template_chapter_start(&input.template);
    }

    let files = split(&input.sections);
    let mut links = Links { lang: wl, ..Links::default() };
    for file in &files {
        for s in &file.sections {
            links.key_files.insert(s.key, file.name().to_owned());
            links.key_titles.insert(s.key, heading_text(s));
        }
    }
    let note_target = match input.footnotes {
        FootnoteMode::Popup => NoteTarget::SameFile,
        FootnoteMode::Endnotes => NoteTarget::Endnotes,
    };
    collect_labels(&files, note_target, &mut links);
    let back = &input.back_matter;
    if back.is_active() && back.settings.glossary_page.is_some() {
        links.glossary = back.glossary.iter().filter(|g| anchor_safe(&g.uuid)).map(|g| g.uuid.clone()).collect();
    }

    // Images: only those the text names, in the order of first appearance.
    let mut image_items = Vec::new();
    let cover_href = input.cover.as_ref().map(|c| format!("images/cover.{}", extension(&c.media_type, &c.path)));
    let mut used_hrefs: Vec<String> = cover_href.iter().cloned().collect();
    for s in &input.sections {
        for path in baruch_wysiwyg::image_paths(&s.body) {
            let Some(normal) = normalize_path(&path) else {
                warnings.push(warning(
                    "IMAGE_PATH_INVALID",
                    format!("Bildpfad {path} verlässt den Ordner des Werks; das Bild fehlt im EPUB."),
                    Some(s.key),
                ));
                continue;
            };
            if links.images.contains_key(&normal) {
                continue;
            }
            let Some(image) = find_image(&input.images, &normal) else { continue };
            if !CORE_IMAGE_TYPES.contains(&image.media_type.as_str()) {
                warnings.push(warning(
                    "IMAGE_TYPE_FOREIGN",
                    format!(
                        "Bild {path} hat den Typ {}; EPUB-Leser kennen sicher nur PNG, JPEG, GIF, SVG und WebP.",
                        image.media_type
                    ),
                    Some(s.key),
                ));
            }
            let href = unique_href(
                &fitting_extension(&format!("images/{}", clean_path(&normal)), &image.media_type),
                &used_hrefs,
            );
            used_hrefs.push(href.clone());
            let n = image_items.len() + 1;
            image_items.push(EpubItem {
                id: format!("img-{n}"),
                href: href.clone(),
                media_type: image.media_type.clone(),
                kind: ItemKind::Image,
                properties: None,
                title: None,
                key: None,
                bytes: image.bytes.clone(),
            });
            links.images.insert(normal, ImageTarget { href, alt: image.alt.clone() });
        }
    }

    // Texts of the files.
    let numbering = typeset.footnotes.as_ref().map_or(FootnoteNumbering::Arabic, |f| f.numbering);
    let per_chapter = input.footnotes == FootnoteMode::Popup
        && typeset.footnotes.as_ref().is_some_and(|f| f.restart == FootnoteRestart::PerChapter);
    let mut next_note = 1;
    let mut all_notes: Vec<Note> = Vec::new();
    let mut text_items = Vec::new();
    let mut toc = Vec::new();
    let mut index_marks: Vec<PlacedMark> = Vec::new();
    for file in &files {
        if per_chapter {
            next_note = 1;
        }
        let mut renderer = Renderer::new(&links, file.name(), note_target, numbering, next_note);
        let (class, epub_type) = match file.kind {
            ItemKind::Part => ("baruch-part", "part"),
            ItemKind::Chapter => ("baruch-chapter", "chapter"),
            _ => ("baruch-front", "frontmatter"),
        };
        let mut body = format!("<section class=\"{class}\" epub:type=\"{epub_type}\">\n");
        for s in &file.sections {
            renderer.section(&mut body, s.key, s.level, s.number.as_deref(), &s.title, &s.body);
            toc.push(TocEntry {
                title: heading_text(s),
                href: format!("{}#k{}", file.href, s.key),
                level: s.level,
                key: Some(s.key),
            });
        }
        if note_target == NoteTarget::SameFile {
            body.push_str(&footnote_list_in(&renderer.notes, wl));
        }
        body.push_str("</section>\n");
        next_note = renderer.next_note;
        warnings.append(&mut renderer.warnings);
        all_notes.append(&mut renderer.notes);
        index_marks.append(&mut renderer.index_marks);
        let first = file.sections[0];
        text_items.push(EpubItem {
            id: file.id.clone(),
            href: file.href.clone(),
            media_type: XHTML.into(),
            kind: file.kind,
            properties: None,
            title: Some(heading_text(first)),
            key: Some(first.key),
            bytes: page(&lang, &heading_text(first), &body).into_bytes(),
        });
    }

    let mut items = Vec::new();
    let mut spine = Vec::new();
    // Cover.
    if let (Some(cover), Some(href)) = (&input.cover, &cover_href) {
        let alt = if cover.alt.trim().is_empty() {
            book_t(wl, "book.cover.alt", &[("title", &title)])
        } else {
            cover.alt.trim().to_owned()
        };
        items.push(EpubItem {
            id: "cover-image".into(),
            href: href.clone(),
            media_type: cover.media_type.clone(),
            kind: ItemKind::CoverImage,
            properties: Some("cover-image".into()),
            title: None,
            key: None,
            bytes: cover.bytes.clone(),
        });
        let body = format!(
            "<section class=\"baruch-cover\" epub:type=\"cover\">\n<img class=\"baruch-cover-image\" src=\"../{}\" \
             alt=\"{}\"/>\n</section>\n",
            escape_text(href),
            escape_text(&alt)
        );
        let label = t("book.cover.title");
        items.push(text_item("cover", "text/cover.xhtml", ItemKind::Cover, &label, page(&lang, &label, &body)));
        spine.push("cover".to_owned());
    }
    // Front matter. R22: with a draft the title page always exists – it carries the notice (imprint and dedication
    // only with front matter).
    let draft = input.typeset.draft.as_ref();
    if input.front_matter || draft.is_some() {
        let mut body = String::from("<section class=\"baruch-titlepage\" epub:type=\"titlepage\">\n");
        body.push_str(&format!("<p class=\"baruch-title\">{}</p>\n", escape_text(&title)));
        for (class, value) in [
            ("baruch-subtitle", &meta.subtitle),
            ("baruch-author", &meta.author),
            ("baruch-publisher", &meta.publisher),
        ] {
            if !value.trim().is_empty() {
                body.push_str(&format!("<p class=\"{class}\">{}</p>\n", escape_text(value.trim())));
            }
        }
        if let Some(draft) = draft {
            body.push_str(&draft_notice(draft, wl));
        }
        body.push_str("</section>\n");
        let label = t("book.title_page.title");
        items.push(text_item("title", "text/title.xhtml", ItemKind::Front, &label, page(&lang, &title, &body)));
        spine.push("title".to_owned());
    }
    if input.front_matter {
        let lines: Vec<&String> = input.imprint.iter().filter(|l| !l.trim().is_empty()).collect();
        if !lines.is_empty() {
            let mut body = String::from("<section class=\"baruch-imprint\" epub:type=\"copyright-page\">\n");
            for line in lines {
                body.push_str(&format!("<p>{}</p>\n", escape_text(line.trim())));
            }
            body.push_str("</section>\n");
            let label = t("book.front.imprint");
            items.push(text_item("imprint", "text/imprint.xhtml", ItemKind::Front, &label, page(&lang, &label, &body)));
            spine.push("imprint".to_owned());
        }
        if !input.dedication.trim().is_empty() {
            let mut body = String::from("<section class=\"baruch-dedication\" epub:type=\"dedication\">\n");
            for paragraph in input.dedication.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                let lines: Vec<String> = paragraph.lines().map(|l| escape_text(l.trim())).collect();
                body.push_str(&format!("<p>{}</p>\n", lines.join("<br/>")));
            }
            body.push_str("</section>\n");
            let label = t("book.front.dedication");
            items.push(text_item(
                "dedication",
                "text/dedication.xhtml",
                ItemKind::Front,
                &label,
                page(&lang, &label, &body),
            ));
            spine.push("dedication".to_owned());
        }
    }
    for item in text_items {
        spine.push(item.id.clone());
        items.push(item);
    }
    if note_target == NoteTarget::Endnotes && !all_notes.is_empty() {
        let label = t("book.endnotes.title");
        let body = format!(
            "<section class=\"baruch-endnotes\" epub:type=\"endnotes\">\n<h1>{}</h1>\n{}\
             </section>\n",
            escape_text(&label),
            endnote_list_in(&all_notes, wl)
        );
        items.push(text_item("notes", "text/notes.xhtml", ItemKind::Notes, &label, page(&lang, &label, &body)));
        spine.push("notes".to_owned());
        // At the top level: in works with parts (level 0) not as a child of the last part.
        let top = toc.iter().map(|e: &TocEntry| e.level).min().unwrap_or(1);
        toc.push(TocEntry { title: label, href: "text/notes.xhtml".into(), level: top, key: None });
    }
    // Index and glossary (Round 31b) at the end, in the order of the typeset result.
    if back.is_active() {
        let top = toc.iter().map(|e: &TocEntry| e.level).min().unwrap_or(1);
        for item in back_matter_items(&lang, input, &index_marks, &links) {
            spine.push(item.id.clone());
            toc.push(TocEntry {
                title: item.title.clone().unwrap_or_default(),
                href: item.href.clone(),
                level: top,
                key: None,
            });
            items.push(item);
        }
    }

    let first_text = items
        .iter()
        .find(|i| matches!(i.kind, ItemKind::Part | ItemKind::Chapter) || i.id == "front")
        .map(|i| i.href.clone())
        .unwrap_or_else(|| "text/front.xhtml".into());
    let has_cover = input.cover.is_some();
    let has_title = input.front_matter || draft.is_some();
    // R22: title metadata (dc:title, docTitle) with „(Entwurf)“; the title page shows the title itself.
    let meta_title = if draft.is_some() { book_t(wl, "book.draft.title", &[("title", &title)]) } else { title.clone() };
    let mut head = vec![
        EpubItem {
            id: "nav".into(),
            href: "nav.xhtml".into(),
            media_type: XHTML.into(),
            kind: ItemKind::Nav,
            properties: Some("nav".into()),
            title: Some(t("book.toc.title")),
            key: None,
            bytes: nav_xhtml(&lang, wl, &toc, &first_text, has_cover, has_title).into_bytes(),
        },
        EpubItem {
            id: "ncx".into(),
            href: "toc.ncx".into(),
            media_type: NCX.into(),
            kind: ItemKind::Ncx,
            properties: None,
            title: None,
            key: None,
            bytes: ncx(meta.identifier.trim(), &meta_title, &meta.author, &toc).into_bytes(),
        },
        EpubItem {
            id: "css".into(),
            href: "style/book.css".into(),
            media_type: "text/css".into(),
            kind: ItemKind::Style,
            properties: None,
            title: None,
            key: None,
            bytes: if back.is_active() {
                format!("{}{INDEX_CSS}", css(&typeset)).into_bytes()
            } else {
                css(&typeset).into_bytes()
            },
        },
    ];
    head.append(&mut items);
    head.append(&mut image_items);
    let items = head;

    let all_alt = !warnings.iter().any(|w| w.code == "IMAGE_NO_ALT");
    let has_images = items.iter().any(|i| matches!(i.kind, ItemKind::Image | ItemKind::CoverImage));
    let (opf, mut meta_warnings) = opf(input, &lang, &meta_title, &items, &spine, &first_text, has_images, all_alt);
    warnings.append(&mut meta_warnings);
    Ok(EpubBook { meta: meta.clone(), items, spine, toc, opf, warnings })
}

/// Style of the index and glossary pages (only with active R31 settings, otherwise `book.css` stays byte-identical).
pub const INDEX_CSS: &str = "/* Baruch: Register und Glossar */
section.baruch-index ul { list-style: none; margin: 0; padding: 0; }
section.baruch-index ul ul { padding-left: 1.2em; }
section.baruch-index li { margin: 0.2em 0; text-indent: -1.2em; padding-left: 1.2em; }
h2.baruch-index-letter { font-size: 1em; margin: 1em 0 0.3em 0; }
section.baruch-glossary dt { font-weight: bold; margin-top: 0.8em; }
section.baruch-glossary dd { margin: 0.2em 0 0 1.2em; }
a.baruch-glossref { text-decoration: none; }
";

/// May a UUID be part of an XML id or of a Typst label (as in `baruch_typst::index`)?
fn anchor_safe(uuid: &str) -> bool {
    !uuid.is_empty() && uuid.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

/// Letter of the group of a headword in the index (as in the typeset result: DIN 5007-1, "0–9" for digits,
/// otherwise "#").
fn group_letter(term: &str) -> String {
    match baruch_typst::index::sort_key(term).chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
        Some(c) if c.is_numeric() => "0–9".to_owned(),
        _ => "#".to_owned(),
    }
}

/// Glossary page and index pages (Round 31b) in the order of the typeset result.
fn back_matter_items(lang: &str, input: &EpubInput, marks: &[PlacedMark], links: &Links) -> Vec<EpubItem> {
    use baruch_typst::typeset::GlossaryPosition;
    let back = &input.back_matter;
    let glossary = back.settings.glossary_page.map(|_| glossary_item(lang, input.work_language, &back.glossary));
    let mut out = Vec::new();
    if back.settings.glossary_page == Some(GlossaryPosition::BeforeIndexes) {
        out.extend(glossary.clone());
    }
    if let Some(pages) = &back.settings.index_pages {
        let bodies: Vec<(i64, &str)> = input.sections.iter().map(|s| (s.key, s.body.as_str())).collect();
        let views = baruch_typst::index::overview(back, &bodies);
        let mut done: Vec<&str> = Vec::new();
        for slug in &pages.registers {
            if done.contains(&slug.as_str()) {
                continue;
            }
            done.push(slug);
            if let Some(view) = views.iter().find(|v| &v.register.slug == slug) {
                out.push(index_item(lang, view, marks, links));
            }
        }
    }
    if back.settings.glossary_page == Some(GlossaryPosition::AfterIndexes) {
        out.extend(glossary);
    }
    out
}

/// `text/glossary.xhtml`: terms sorted (DIN 5007-1), per term `dt` (glossterm, id = label of the typeset result)
/// and `dd` (glossdef: variants, short and detailed explanation, Bible references).
fn glossary_item(
    lang: &str,
    wl: baruch_core::WorkLanguage,
    glossary: &[baruch_typst::index::GlossaryEntry],
) -> EpubItem {
    let mut entries: Vec<&baruch_typst::index::GlossaryEntry> = glossary.iter().collect();
    entries.sort_by_key(|e| (baruch_typst::index::sort_key(&e.term), e.term.clone()));
    let label = book_t(wl, "book.glossary.title", &[]);
    let mut body = format!(
        "<section class=\"baruch-glossary\" epub:type=\"glossary\" role=\"doc-glossary\">\n<h1>{}</h1>\n",
        escape_text(&label)
    );
    if entries.is_empty() {
        body.push_str(&format!("<p>{}</p>\n", escape_text(&book_t(wl, "book.glossary.empty", &[]))));
    } else {
        body.push_str("<dl>\n");
        for e in entries {
            let id = if anchor_safe(&e.uuid) {
                format!(" id=\"{}{}\"", baruch_typst::index::GLOSSARY_ANCHOR_PREFIX, e.uuid)
            } else {
                String::new()
            };
            body.push_str(&format!(
                "<dt{id} epub:type=\"glossterm\"><dfn>{}</dfn></dt>\n<dd epub:type=\"glossdef\">",
                escape_text(e.term.trim())
            ));
            let variants: Vec<String> =
                e.variants.iter().map(|v| v.trim()).filter(|v| !v.is_empty()).map(escape_text).collect();
            if !variants.is_empty() {
                let line = book_t(wl, "book.glossary.variants", &[("variants", &variants.join(", "))]);
                body.push_str(&format!("<p class=\"baruch-gl-variants\">{line}</p>"));
            }
            if !e.short.trim().is_empty() {
                body.push_str(&format!("<p>{}</p>", escape_text(e.short.trim())));
            }
            for paragraph in e.long.replace("\r\n", "\n").split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
                let lines: Vec<String> = paragraph.lines().map(|l| escape_text(l.trim())).collect();
                body.push_str(&format!("<p>{}</p>", lines.join(" ")));
            }
            let refs: Vec<String> =
                e.bible_refs.iter().map(|r| r.trim()).filter(|r| !r.is_empty()).map(escape_text).collect();
            if !refs.is_empty() {
                let line = book_t(wl, "book.glossary.bible_refs", &[("refs", &refs.join("; "))]);
                body.push_str(&format!("<p class=\"baruch-gl-refs\">{line}</p>"));
            }
            body.push_str("</dd>\n");
        }
        body.push_str("</dl>\n");
    }
    body.push_str("</section>\n");
    text_item("glossary", "text/glossary.xhtml", ItemKind::Glossary, &label, page(lang, &label, &body))
}

/// An entry (headword or subheadword) as a list item without the closing `</li>`.
fn index_entry(
    out: &mut String,
    slug: &str,
    term: &baruch_typst::index::IndexTerm,
    marks: &[PlacedMark],
    links: &Links,
) {
    let shown = if term.sub.is_empty() { &term.term } else { &term.sub };
    out.push_str(&format!(
        "<li epub:type=\"index-entry\"><span epub:type=\"index-term\">{}</span>",
        escape_text(shown)
    ));
    let mut locators = Vec::new();
    for key in &term.keys {
        let found = marks.iter().find(|m| {
            m.key == *key
                && m.mark.register == slug
                && m.mark.term == term.term
                && m.mark.sub == term.sub
                && m.mark.see.is_empty()
        });
        if let Some(m) = found {
            let label = links.key_titles.get(key).cloned().unwrap_or_else(|| key.to_string());
            locators.push(format!(
                "<a epub:type=\"index-locator\" href=\"{}#{}\">{}</a>",
                escape_text(&m.file),
                escape_text(&m.id),
                escape_text(&label)
            ));
        }
    }
    let mut parts = Vec::new();
    if !locators.is_empty() {
        parts.push(locators.join(", "));
    }
    if !term.see.is_empty() {
        let see: Vec<String> = term.see.iter().map(|s| escape_text(s)).collect();
        parts.push(format!("<em>{}</em> {}", escape_text(&book_t(links.lang, "book.index.see", &[])), see.join("; ")));
    }
    if !term.also.is_empty() {
        let also: Vec<String> = term.also.iter().map(|s| escape_text(s)).collect();
        let word = escape_text(&book_t(links.lang, "book.index.see_also", &[]));
        parts.push(format!("<em>{word}</em> {}", also.join("; ")));
    }
    if !parts.is_empty() {
        out.push(' ');
        out.push_str(&parts.join("; "));
    }
}

/// `text/index-{slug}.xhtml`: headwords in the order of the overview, subheadwords as an indented list, grouped by
/// letter except in the index of Bible references.
fn index_item(lang: &str, view: &baruch_typst::index::RegisterView, marks: &[PlacedMark], links: &Links) -> EpubItem {
    let slug = &view.register.slug;
    let title = &baruch_typst::index::register_title(&view.register, links.lang);
    let mut body = format!(
        "<section class=\"baruch-index\" epub:type=\"index\" role=\"doc-index\">\n<h1>{}</h1>\n",
        escape_text(title)
    );
    if view.terms.is_empty() {
        body.push_str(&format!("<p>{}</p>\n", escape_text(&book_t(links.lang, "book.index.empty", &[]))));
    } else {
        let letters = view.register.kind != baruch_typst::index::RegisterKind::Scripture;
        let mut letter = String::new();
        let mut list_open = false;
        let mut term_open: Option<&str> = None;
        let mut subs_open = false;
        let close_term = |body: &mut String, subs_open: &mut bool| {
            if *subs_open {
                body.push_str("</ul>");
                *subs_open = false;
            }
            body.push_str("</li>\n");
        };
        for t in &view.terms {
            if term_open != Some(t.term.as_str()) {
                if term_open.is_some() {
                    close_term(&mut body, &mut subs_open);
                }
                let l = group_letter(&t.term);
                if letters && l != letter {
                    if list_open {
                        body.push_str("</ul>\n");
                        list_open = false;
                    }
                    body.push_str(&format!("<h2 class=\"baruch-index-letter\">{}</h2>\n", escape_text(&l)));
                    letter = l;
                }
                if !list_open {
                    body.push_str("<ul epub:type=\"index-entry-list\">\n");
                    list_open = true;
                }
                term_open = Some(t.term.as_str());
                if t.sub.is_empty() {
                    index_entry(&mut body, slug, t, marks, links);
                    continue;
                }
                // Headword with subheadwords only: an item of its own without a locator.
                body.push_str(&format!(
                    "<li epub:type=\"index-entry\"><span epub:type=\"index-term\">{}</span>",
                    escape_text(&t.term)
                ));
            }
            if !t.sub.is_empty() {
                if !subs_open {
                    body.push_str("\n<ul epub:type=\"index-entry-list\">\n");
                    subs_open = true;
                }
                index_entry(&mut body, slug, t, marks, links);
                body.push_str("</li>\n");
            }
        }
        if term_open.is_some() {
            close_term(&mut body, &mut subs_open);
        }
        if list_open {
            body.push_str("</ul>\n");
        }
    }
    body.push_str("</section>\n");
    let id = format!("index-{slug}");
    let href = format!("text/index-{slug}.xhtml");
    text_item(&id, &href, ItemKind::Index, title, page(lang, title, &body))
}

/// Image types that EPUB 3 knows as core media types.
const CORE_IMAGE_TYPES: [&str; 5] = ["image/png", "image/jpeg", "image/gif", "image/svg+xml", "image/webp"];

fn find_image<'a>(images: &'a [EpubImage], normal: &str) -> Option<&'a EpubImage> {
    images.iter().find(|i| normalize_path(&i.path).as_deref() == Some(normal))
}

/// File extension for the image type (otherwise that of the path, otherwise `bin`).
fn extension(media_type: &str, path: &str) -> String {
    match media_type {
        "image/png" => "png".into(),
        "image/jpeg" => "jpg".into(),
        "image/gif" => "gif".into(),
        "image/svg+xml" => "svg".into(),
        "image/webp" => "webp".into(),
        _ => path
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .filter(|ext| !ext.is_empty() && ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or_else(|| "bin".into()),
    }
}

/// Path in the package: only letters, digits, `.`, `-`, `_` and `/` (everything else → `_`).
/// File name in the EPUB with an extension that fits the image type: if the extension does not fit (after a change
/// of type a PNG is still called `haus.svg`), the right one is added (`haus.svg.png`); some readers go by the
/// extension.
fn fitting_extension(href: &str, media_type: &str) -> String {
    let fitting: &[&str] = match media_type {
        "image/png" => &["png"],
        "image/jpeg" => &["jpg", "jpeg"],
        "image/gif" => &["gif"],
        "image/svg+xml" => &["svg"],
        "image/webp" => &["webp"],
        _ => return href.to_owned(),
    };
    let ext =
        href.rsplit_once('/').map_or(href, |(_, name)| name).rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    if ext.is_some_and(|e| fitting.contains(&e.as_str())) { href.to_owned() } else { format!("{href}.{}", fitting[0]) }
}

fn clean_path(normal: &str) -> String {
    normal
        .split('/')
        .map(|part| {
            let clean: String = part
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
                .collect();
            if clean.starts_with('.') { format!("_{clean}") } else { clean }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// `href`, if equal to a name already given with `-2`, `-3` … before the extension.
fn unique_href(href: &str, used: &[String]) -> String {
    let taken = |h: &str| used.iter().any(|u| u.eq_ignore_ascii_case(h));
    if !taken(href) {
        return href.to_owned();
    }
    let (stem, ext) = match href.rsplit_once('.') {
        Some((stem, ext)) if !stem.ends_with('/') => (stem.to_owned(), format!(".{ext}")),
        _ => (href.to_owned(), String::new()),
    };
    (2..).map(|n| format!("{stem}-{n}{ext}")).find(|h| !taken(h)).expect("unendlich viele Namen")
}

fn modified_ok(s: &str) -> bool {
    s.len() == 20
        && s.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 => c == 'T',
            13 | 16 => c == ':',
            19 => c == 'Z',
            _ => c.is_ascii_digit(),
        })
}

/// Split according to contract §0.2.
fn split(sections: &[EpubSection]) -> Vec<TextFile<'_>> {
    let mut files: Vec<TextFile<'_>> = Vec::new();
    for s in sections {
        if s.level <= 1 {
            files.push(TextFile {
                id: format!("k{}", s.key),
                href: format!("text/k{}.xhtml", s.key),
                kind: if s.level == 0 { ItemKind::Part } else { ItemKind::Chapter },
                sections: vec![s],
            });
        } else if let Some(last) = files.last_mut() {
            last.sections.push(s);
        } else {
            files.push(TextFile {
                id: "front".into(),
                href: "text/front.xhtml".into(),
                kind: ItemKind::Front,
                sections: vec![s],
            });
        }
    }
    files
}

/// Labels (`<name>`) in the text: in which file they are (for references `@name`).
/// In endnote mode labels from footnotes are in `notes.xhtml`, not in the chapter file.
fn collect_labels(files: &[TextFile<'_>], note_target: NoteTarget, links: &mut Links) {
    use baruch_wysiwyg::{BlockKind, Inline};
    fn inl(inlines: &[Inline], file: &str, note_file: &str, links: &mut Links) {
        for i in inlines {
            match i {
                Inline::Label { v } => {
                    links.labels.entry(v.clone()).or_insert_with(|| file.to_owned());
                }
                Inline::Emph { c } | Inline::Strong { c } | Inline::Quote { c } => inl(c, file, note_file, links),
                Inline::Footnote { c } => inl(c, note_file, note_file, links),
                _ => {}
            }
        }
    }
    /// Next number of a figure; the counter runs along under a name that cannot be a label.
    fn next_figure(links: &mut Links) -> usize {
        let number = links.figures.get("\u{0}count").copied().unwrap_or(0) + 1;
        links.figures.insert("\u{0}count".to_owned(), number);
        number
    }
    fn walk(blocks: &[baruch_wysiwyg::Block], file: &str, note_file: &str, links: &mut Links) {
        for b in blocks {
            match &b.kind {
                BlockKind::Heading { inlines, label, .. } => {
                    inl(inlines, file, note_file, links);
                    if let Some(label) = label {
                        links.labels.entry(label.clone()).or_insert_with(|| file.to_owned());
                    }
                }
                // Round 23a: figures count in reading order; a label on the figure is the target of `@name`
                // and knows its number. What is counted is what also is a `<figure>` with image in the EPUB: an island
                // `#figure(image(…) …)` alone or with its label after it (the block model reads the latter as a
                // paragraph of island and label).
                BlockKind::Paragraph { inlines } => {
                    if let Some((raw, label)) =
                        crate::xhtml::labelled_figure(inlines).filter(|(r, _)| r.contains("image("))
                    {
                        let _ = raw;
                        let number = next_figure(links);
                        links.labels.entry(label.to_owned()).or_insert_with(|| file.to_owned());
                        links.figures.entry(label.to_owned()).or_insert(number);
                    } else {
                        inl(inlines, file, note_file, links);
                    }
                }
                BlockKind::Quote { body, .. } | BlockKind::Scene { body, .. } | BlockKind::Work { body, .. } => {
                    walk(body, file, note_file, links)
                }
                BlockKind::Island => {
                    if let Some(label) = crate::xhtml::figure_label(&b.raw).filter(|_| b.raw.contains("image(")) {
                        let number = next_figure(links);
                        if let Some(label) = label {
                            links.labels.entry(label.clone()).or_insert_with(|| file.to_owned());
                            links.figures.entry(label).or_insert(number);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for file in files {
        let note_file = match note_target {
            NoteTarget::SameFile => file.name(),
            NoteTarget::Endnotes => "notes.xhtml",
        };
        for s in &file.sections {
            walk(&baruch_wysiwyg::parse(&s.body).blocks, file.name(), note_file, links);
        }
    }
    // The counter of the figures ran along under a name that cannot be a label.
    links.figures.remove("\u{0}count");
}

/// Displayed heading: number and title.
fn heading_text(s: &EpubSection) -> String {
    match s.number.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(number) => format!("{number} {}", s.title),
        None => s.title.clone(),
    }
}

fn css(typeset: &TypesetSettings) -> String {
    format!("{BASE_CSS}{}", baruch_typst::typeset::epub_style(typeset).css)
}

fn text_item(id: &str, href: &str, kind: ItemKind, title: &str, content: String) -> EpubItem {
    EpubItem {
        id: id.into(),
        href: href.into(),
        media_type: XHTML.into(),
        kind,
        properties: None,
        title: Some(title.into()),
        key: None,
        bytes: content.into_bytes(),
    }
}

fn head(lang: &str, title: &str, css: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" \
         xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{lang}\" lang=\"{lang}\">\n<head>\n<meta \
         charset=\"utf-8\"/>\n<title>{title}</title>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"{css}\"/>\n\
         </head>\n",
        lang = escape_text(lang),
        title = escape_text(title),
    )
}

/// Whole XHTML file in `text/`.
fn page(lang: &str, title: &str, body: &str) -> String {
    let mut out = head(lang, title, "../style/book.css");
    out.push_str("<body>\n");
    out.push_str(body);
    out.push_str("</body>\n</html>\n");
    out
}

/// Nested list (`<ol>`) from entries with levels; `link` writes one entry.
fn nested(out: &mut String, toc: &[TocEntry], indent_list: &str, link: impl Fn(&TocEntry) -> String) {
    struct Open {
        level: u8,
        child_list: bool,
    }
    let mut stack: Vec<Open> = Vec::new();
    let close = |out: &mut String, open: Open| {
        if open.child_list {
            out.push_str("</ol>\n");
        }
        out.push_str("</li>\n");
    };
    out.push_str(&format!("<ol{indent_list}>\n"));
    for entry in toc {
        while stack.last().is_some_and(|top| top.level >= entry.level) {
            close(out, stack.pop().expect("geprüft"));
        }
        if let Some(top) = stack.last_mut()
            && !top.child_list
        {
            out.push_str("<ol>\n");
            top.child_list = true;
        }
        out.push_str(&format!("<li>{}", link(entry)));
        stack.push(Open { level: entry.level, child_list: false });
    }
    while let Some(open) = stack.pop() {
        close(out, open);
    }
    out.push_str("</ol>\n");
}

fn nav_xhtml(
    lang: &str,
    wl: baruch_core::WorkLanguage,
    toc: &[TocEntry],
    first_text: &str,
    cover: bool,
    title: bool,
) -> String {
    let t = |key: &str| escape_text(&book_t(wl, key, &[]));
    let contents = t("book.toc.title");
    let mut out = head(lang, &book_t(wl, "book.toc.title", &[]), "style/book.css");
    out.push_str(&format!("<body>\n<nav epub:type=\"toc\" id=\"toc\" role=\"doc-toc\">\n<h1>{contents}</h1>\n"));
    nested(&mut out, toc, "", |e| format!("<a href=\"{}\">{}</a>", escape_text(&e.href), escape_text(&e.title)));
    out.push_str(&format!(
        "</nav>\n<nav epub:type=\"landmarks\" id=\"landmarks\" hidden=\"hidden\">\n<h2>{}</h2>\n<ol>\n",
        t("book.nav.landmarks")
    ));
    if cover {
        out.push_str(&format!(
            "<li><a epub:type=\"cover\" href=\"text/cover.xhtml\">{}</a></li>\n",
            t("book.cover.title")
        ));
    }
    if title {
        out.push_str(&format!(
            "<li><a epub:type=\"titlepage\" href=\"text/title.xhtml\">{}</a></li>\n",
            t("book.title_page.title")
        ));
    }
    out.push_str(&format!("<li><a epub:type=\"toc\" href=\"nav.xhtml#toc\">{contents}</a></li>\n"));
    out.push_str(&format!(
        "<li><a epub:type=\"bodymatter\" href=\"{}\">{}</a></li>\n",
        escape_text(first_text),
        t("book.nav.text")
    ));
    out.push_str("</ol>\n</nav>\n</body>\n</html>\n");
    out
}

/// R22: notice „Entwurf“ on the title page – text of the watermark, addition (the server has already written the
/// date into it, the EPUB stays without a clock), only as escaped text. Style inline, so that `style/book.css`
/// stays byte-identical without a draft.
fn draft_notice(draft: &baruch_typst::typeset::Draft, wl: baruch_core::WorkLanguage) -> String {
    let text = baruch_typst::typeset::draft_text_in(draft, wl);
    let mut line = book_t(wl, "book.draft.notice", &[("text", &text)]);
    if !draft.note.trim().is_empty() {
        line.push_str(" · ");
        line.push_str(draft.note.trim());
    }
    format!(
        "<p class=\"baruch-draft\" style=\"margin-top: 2em; font-weight: bold; letter-spacing: 0.1em;\">{}</p>\n",
        escape_text(&line)
    )
}

/// `toc.ncx` (EPUB 2): nested navPoints with consecutive playOrder.
fn ncx(identifier: &str, title: &str, author: &str, toc: &[TocEntry]) -> String {
    let depth = {
        let mut stack: Vec<u8> = Vec::new();
        let mut max = 1;
        for e in toc {
            while stack.last().is_some_and(|&l| l >= e.level) {
                stack.pop();
            }
            stack.push(e.level);
            max = max.max(stack.len());
        }
        max
    };
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" \
         version=\"2005-1\">\n<head>\n<meta name=\"dtb:uid\" content=\"{}\"/>\n<meta name=\"dtb:depth\" \
         content=\"{depth}\"/>\n<meta name=\"dtb:totalPageCount\" content=\"0\"/>\n<meta name=\"dtb:maxPageNumber\" \
         content=\"0\"/>\n</head>\n<docTitle><text>{}</text></docTitle>\n",
        escape_text(identifier),
        escape_text(title)
    );
    if !author.trim().is_empty() {
        out.push_str(&format!("<docAuthor><text>{}</text></docAuthor>\n", escape_text(author.trim())));
    }
    out.push_str("<navMap>\n");
    let mut stack: Vec<u8> = Vec::new();
    for (n, entry) in toc.iter().enumerate() {
        while stack.last().is_some_and(|&l| l >= entry.level) {
            stack.pop();
            out.push_str("</navPoint>\n");
        }
        out.push_str(&format!(
            "<navPoint id=\"nav-{order}\" playOrder=\"{order}\">\n<navLabel><text>{}</text></navLabel>\n<content \
             src=\"{}\"/>\n",
            escape_text(&entry.title),
            escape_text(&entry.href),
            order = n + 1
        ));
        stack.push(entry.level);
    }
    for _ in stack {
        out.push_str("</navPoint>\n");
    }
    out.push_str("</navMap>\n</ncx>\n");
    out
}

#[allow(clippy::too_many_arguments)]
fn opf(
    input: &EpubInput,
    lang: &str,
    title: &str,
    items: &[EpubItem],
    spine: &[String],
    first_text: &str,
    has_images: bool,
    all_alt: bool,
) -> (String, Vec<EpubWarning>) {
    let meta = &input.meta;
    let mut warnings = Vec::new();
    let e = escape_text;
    let mut m = String::new();
    m.push_str(&format!("<dc:identifier id=\"book-id\">{}</dc:identifier>\n", e(meta.identifier.trim())));
    if let Some(isbn) = meta.isbn.as_deref().map(str::trim).filter(|i| !i.is_empty()) {
        match baruch_typst::cover::isbn13_check(isbn) {
            Ok(digits) => m.push_str(&format!("<dc:identifier id=\"isbn\">urn:isbn:{digits}</dc:identifier>\n")),
            Err(err) => warnings.push(warning("ISBN_INVALID", format!("Die ISBN steht nicht im EPUB: {err}"), None)),
        }
    }
    m.push_str(&format!("<dc:title id=\"title\">{}</dc:title>\n", e(title)));
    m.push_str("<meta refines=\"#title\" property=\"title-type\">main</meta>\n");
    if !meta.subtitle.trim().is_empty() {
        m.push_str(&format!("<dc:title id=\"subtitle\">{}</dc:title>\n", e(meta.subtitle.trim())));
        m.push_str("<meta refines=\"#subtitle\" property=\"title-type\">subtitle</meta>\n");
    }
    if !meta.author.trim().is_empty() {
        m.push_str(&format!("<dc:creator id=\"creator\">{}</dc:creator>\n", e(meta.author.trim())));
        m.push_str("<meta refines=\"#creator\" property=\"role\" scheme=\"marc:relators\">aut</meta>\n");
    }
    m.push_str(&format!("<dc:language>{}</dc:language>\n", e(lang)));
    if !meta.publisher.trim().is_empty() {
        m.push_str(&format!("<dc:publisher>{}</dc:publisher>\n", e(meta.publisher.trim())));
    }
    let year = meta.year.trim();
    if year.len() == 4 && year.chars().all(|c| c.is_ascii_digit()) {
        m.push_str(&format!("<dc:date>{year}</dc:date>\n"));
    } else if !year.is_empty() {
        warnings.push(warning(
            "META_YEAR_IGNORED",
            format!("Das Jahr „{year}“ ist keine vierstellige Jahreszahl und fehlt in den Metadaten (dc:date)."),
            None,
        ));
    }
    if !meta.rights.trim().is_empty() {
        m.push_str(&format!("<dc:rights>{}</dc:rights>\n", e(meta.rights.trim())));
    }
    if !meta.image_credit.trim().is_empty() {
        let credit = book_t(input.work_language, "book.meta.image_credit", &[("credit", meta.image_credit.trim())]);
        m.push_str(&format!("<dc:rights>{}</dc:rights>\n", e(&credit)));
    }
    if !meta.description.trim().is_empty() {
        m.push_str(&format!("<dc:description>{}</dc:description>\n", e(meta.description.trim())));
    }
    m.push_str(&format!("<meta property=\"dcterms:modified\">{}</meta>\n", e(meta.modified.trim())));
    if items.iter().any(|i| i.kind == ItemKind::CoverImage) {
        m.push_str("<meta name=\"cover\" content=\"cover-image\"/>\n");
    }
    // Accessibility (schema.org, EPUB Accessibility 1.1).
    m.push_str("<meta property=\"schema:accessMode\">textual</meta>\n");
    if has_images {
        m.push_str("<meta property=\"schema:accessMode\">visual</meta>\n");
    }
    m.push_str("<meta property=\"schema:accessModeSufficient\">textual</meta>\n");
    for feature in ["structuralNavigation", "tableOfContents", "readingOrder"] {
        m.push_str(&format!("<meta property=\"schema:accessibilityFeature\">{feature}</meta>\n"));
    }
    if has_images && all_alt {
        m.push_str("<meta property=\"schema:accessibilityFeature\">alternativeText</meta>\n");
    }
    m.push_str("<meta property=\"schema:accessibilityHazard\">none</meta>\n");
    let summary = book_t(
        input.work_language,
        if !has_images {
            "book.a11y.summary_text"
        } else if all_alt {
            "book.a11y.summary_all_alt"
        } else {
            "book.a11y.summary_some_alt"
        },
        &[],
    );
    m.push_str(&format!("<meta property=\"schema:accessibilitySummary\">{summary}</meta>\n"));

    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" \
         unique-identifier=\"book-id\" xml:lang=\"{lang}\">\n<metadata \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n{m}</metadata>\n<manifest>\n",
        lang = e(lang),
    );
    for item in items {
        let properties = item.properties.as_deref().map(|p| format!(" properties=\"{}\"", e(p)));
        out.push_str(&format!(
            "<item id=\"{}\" href=\"{}\" media-type=\"{}\"{}/>\n",
            e(&item.id),
            e(&item.href),
            e(&item.media_type),
            properties.unwrap_or_default()
        ));
    }
    out.push_str("</manifest>\n<spine toc=\"ncx\">\n");
    for id in spine {
        out.push_str(&format!("<itemref idref=\"{}\"/>\n", e(id)));
    }
    out.push_str("</spine>\n<guide>\n");
    let t = |key: &str| e(&book_t(input.work_language, key, &[]));
    if items.iter().any(|i| i.id == "cover") {
        out.push_str(&format!(
            "<reference type=\"cover\" title=\"{}\" href=\"text/cover.xhtml\"/>\n",
            t("book.cover.title")
        ));
    }
    if items.iter().any(|i| i.id == "title") {
        out.push_str(&format!(
            "<reference type=\"title-page\" title=\"{}\" href=\"text/title.xhtml\"/>\n",
            t("book.title_page.title")
        ));
    }
    out.push_str(&format!("<reference type=\"toc\" title=\"{}\" href=\"nav.xhtml\"/>\n", t("book.toc.title")));
    out.push_str(&format!("<reference type=\"text\" title=\"{}\" href=\"{}\"/>\n", t("book.nav.text"), e(first_text)));
    out.push_str("</guide>\n</package>\n");
    (out, warnings)
}
