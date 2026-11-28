// SPDX-License-Identifier: AGPL-3.0-or-later
//! Blocks as HTML for the typeset view and the difference of two revisions. Every text is escaped;
//! islands stand there with `contenteditable="false"` and their raw text (additionally in `data-raw`, so that
//! the widget can return an island in an edited body verbatim).

use crate::{Block, BlockKind, Doc, ImageRef, Inline, Term, image_paths, parse};
use baruch_core::WorkLanguage;

/// What rendering needs besides the blocks: the callback for images (path from the source text (`image("…")`) →
/// address and alternative text) and the work language (Round 39b: page abbreviation of a cross reference's page
/// number addition, `S.` in German, `p.` in English).
#[derive(Clone, Copy)]
pub(crate) struct Env<'a> {
    pub(crate) images: &'a dyn Fn(&str) -> Option<ImageRef>,
    pub(crate) lang: WorkLanguage,
}

/// HTML of a text: per block `<div class="ws-block" data-b="i" data-kind="…">` (heading with
/// `data-level`, list with `data-ordered`, quote with `data-block`; Round 14: part as
/// `<div class="ws-block ws-part" data-kind="part" data-level="0" role="heading" aria-level="1">Titel</div>`,
/// "without number" as `class="ws-block ws-unnumbered" data-kind="unnumbered" data-level="N"`, both with
/// the title as plain text). Nested blocks (body of
/// quote, scene, work) stand in `<div class="ws-body">` without `data-b`. Equal to
/// [`render_html_with`] without images.
pub fn render_html(doc: &Doc) -> String {
    render_html_with(doc, &|_| None)
}

/// Like [`render_html`]; additionally every island with an image path ([`image_path`]) for which
/// `images` delivers an image appears **as an image** instead of as raw text (Round 7): a block island as
/// `<div class="ws-block ws-figure" data-b=… data-kind="island" data-image="1" contenteditable="false"
/// data-raw="…" title="Bild – …"><img class="ws-image" src="…" alt="…" loading="eager"><span
/// class="ws-caption">…</span></div>` (caption from `caption:`, otherwise the alternative text,
/// otherwise the path), an inline island as `<span class="ws-island ws-inline-image" …><img …></span>`.
/// All attributes are escaped; the raw text is only in `data-raw`, the island stays
/// byte-identical on saving (`keep`).
pub fn render_html_with(doc: &Doc, images: &dyn Fn(&str) -> Option<ImageRef>) -> String {
    render_html_in(doc, images, WorkLanguage::De)
}

/// Like [`render_html_with`] in the work language `lang` (Round 39b): the page number addition of a cross reference
/// shows the page abbreviation of the work language (`, p. …` in an English work).
pub fn render_html_in(doc: &Doc, images: &dyn Fn(&str) -> Option<ImageRef>, lang: WorkLanguage) -> String {
    let env = Env { images, lang };
    let mut out = String::new();
    for (index, block) in doc.blocks.iter().enumerate() {
        render_block(&mut out, block, Some(index), env);
    }
    out
}

fn kind_name(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Heading { .. } => "heading",
        BlockKind::Paragraph { .. } => "paragraph",
        BlockKind::Quote { .. } => "quote",
        BlockKind::List { .. } => "list",
        BlockKind::Enum { .. } => "enum",
        BlockKind::Terms { .. } => "terms",
        BlockKind::Scene { .. } => "scene",
        BlockKind::Work { .. } => "work",
        BlockKind::Part { .. } => "part",
        BlockKind::Unnumbered { .. } => "unnumbered",
        BlockKind::Island => "island",
    }
}

/// Title of an island that appears as an image.
const IMAGE_TITLE: &str = "Bild – Doppelklick öffnet den Quelltext an Ort und Stelle";

/// Title of a block island (Round 18b: editable in place).
const ISLAND_TITLE: &str = "Typst-Insel: Doppelklick öffnet den Quelltext an Ort und Stelle";

/// Images for all image paths in `raw` (Round 18b: every `image(` call, not only the first) for which the
/// callback delivers one: path and image each.
fn images_of(raw: &str, env: Env) -> Vec<(String, ImageRef)> {
    image_paths(raw).into_iter().filter_map(|path| (env.images)(&path).map(|image| (path, image))).collect()
}

/// `<img>` or the message „Bild fehlt“ for every image.
fn image_tags(found: &[(String, ImageRef)]) -> String {
    found.iter().map(|(path, image)| if image.missing { missing_tag(path) } else { image_tag(image) }).collect()
}

fn image_tag(image: &ImageRef) -> String {
    format!("<img class=\"ws-image\" src=\"{}\" alt=\"{}\" loading=\"eager\">", escape(&image.src), escape(&image.alt))
}

/// Clear message instead of an image that does not exist in the work (Round 11).
fn missing_tag(path: &str) -> String {
    format!("<span class=\"ws-missing\" role=\"note\">Bild fehlt: <code>{}</code></span>", escape(path))
}

/// Caption from `caption: […]` or `caption: "…"` of a figure, verbatim (without brackets).
fn caption_of(raw: &str) -> Option<String> {
    let at = raw.find("caption:")? + "caption:".len();
    let rest = raw[at..].trim_start();
    let (open, close) = match rest.chars().next()? {
        '[' => ('[', ']'),
        '"' => ('"', '"'),
        _ => return None,
    };
    let mut depth = 0usize;
    let mut escaped = false;
    for (i, c) in rest.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            c if c == close && depth == 0 => return Some(rest[1..i].trim().to_owned()).filter(|c| !c.is_empty()),
            c if c == close => depth -= 1,
            c if c == open && open != close => depth += 1,
            _ => {}
        }
    }
    None
}

pub(crate) fn render_block(out: &mut String, block: &Block, index: Option<usize>, env: Env) {
    if block.kind == BlockKind::Island {
        let found = images_of(&block.raw, env);
        if !found.is_empty() {
            render_figure(out, block, index, &found);
            return;
        }
    }
    out.push_str(match block.kind {
        BlockKind::Part { .. } => "<div class=\"ws-block ws-part\"",
        BlockKind::Unnumbered { .. } => "<div class=\"ws-block ws-unnumbered\"",
        _ => "<div class=\"ws-block\"",
    });
    if let Some(index) = index {
        out.push_str(&format!(" data-b=\"{index}\""));
    }
    out.push_str(&format!(" data-kind=\"{}\"", kind_name(&block.kind)));
    match &block.kind {
        BlockKind::Heading { level, inlines, label } => {
            out.push_str(&format!(" data-level=\"{level}\" role=\"heading\" aria-level=\"{level}\">"));
            render_inlines(out, inlines, env);
            if let Some(label) = label {
                // Label on the line after the heading: displayed, unchanged on editing.
                out.push_str(&format!(
                    "<span class=\"ws-label ws-heading-label\" contenteditable=\"false\" data-heading-label=\"1\" data-v=\"{0}\">&lt;{0}&gt;</span>",
                    escape(label)
                ));
            }
        }
        BlockKind::Part { title } => {
            // A part stands as a block of its own with the title as plain text (Round 14); „Teil“ before it
            // is set by the stylesheet (`::before`), so that it does not get into the title.
            out.push_str(&format!(
                " data-level=\"0\" role=\"heading\" aria-level=\"1\" title=\"Teil (Markierung im Typst-Text)\">{}",
                escape(title)
            ));
        }
        BlockKind::Unnumbered { level, title } => {
            out.push_str(&format!(
                " data-level=\"{level}\" role=\"heading\" aria-level=\"{level}\" title=\"ohne Nummer (Markierung im Typst-Text)\">{}",
                escape(title)
            ));
        }
        BlockKind::Paragraph { inlines } => {
            out.push('>');
            render_inlines(out, inlines, env);
        }
        BlockKind::List { ordered, items, depths } => {
            out.push_str(&format!(" data-ordered=\"{ordered}\">"));
            let tag = if *ordered { "ol" } else { "ul" };
            render_items(out, tag, "", items, depths, env);
        }
        BlockKind::Enum { start, items, depths } => {
            out.push_str(&format!(" data-start=\"{start}\">"));
            render_items(out, "ol", &format!(" start=\"{start}\""), items, depths, env);
        }
        BlockKind::Terms { items } => {
            out.push_str("><dl>");
            for Term { term, desc } in items {
                out.push_str("<dt>");
                render_inlines(out, term, env);
                out.push_str("</dt><dd>");
                render_inlines(out, desc, env);
                out.push_str("</dd>");
            }
            out.push_str("</dl>");
        }
        BlockKind::Quote { block: is_block, attribution, body } => {
            out.push_str(&format!(" data-block=\"{is_block}\">"));
            render_body(out, body, env);
            if let Some(attribution) = attribution {
                out.push_str("<div class=\"ws-attribution\">");
                render_inlines(out, attribution, env);
                out.push_str("</div>");
            }
        }
        BlockKind::Scene { args, body } | BlockKind::Work { args, body } => {
            out.push('>');
            if let Some(args) = args {
                // Arguments before the body: editable only in the source text, stay verbatim.
                out.push_str(&format!(
                    "<div class=\"ws-args\" contenteditable=\"false\" title=\"Typst-Insel: nur im Quelltext bearbeitbar\" data-raw=\"{0}\">({0})</div>",
                    escape(args)
                ));
            }
            render_body(out, body, env);
        }
        BlockKind::Island => {
            let blank = if block.raw.trim().is_empty() { " data-blank=\"1\"" } else { "" };
            out.push_str(&format!(
                " contenteditable=\"false\" title=\"{ISLAND_TITLE}\" data-raw=\"{}\"{blank}>",
                escape(&block.raw)
            ));
            out.push_str("<code class=\"ws-raw\">");
            out.push_str(&escape(block.raw.trim_end()));
            out.push_str("</code>");
        }
    }
    out.push_str("</div>");
}

/// Block island with image (Round 7): the image with caption instead of the raw text; Round 18b: all images of
/// the island one after another; Round 21b: with several images the caption (without `caption:`) names every
/// image, and `<span class="ws-alt ws-paths">Bilder: …</span>` shows all paths.
fn render_figure(out: &mut String, block: &Block, index: Option<usize>, found: &[(String, ImageRef)]) {
    let (path, image) = (&found[0].0, &found[0].1);
    out.push_str("<div class=\"ws-block ws-figure\"");
    if let Some(index) = index {
        out.push_str(&format!(" data-b=\"{index}\""));
    }
    out.push_str(&format!(
        " data-kind=\"island\" data-image=\"1\" contenteditable=\"false\" data-raw=\"{}\" title=\"{IMAGE_TITLE}\">",
        escape(&block.raw)
    ));
    out.push_str(&image_tags(found));
    let written = caption_of(&block.raw);
    let alt = image.alt.trim();
    // Round 21b: several images without a written caption name every image (alternative text, otherwise path), and
    // all paths stand below – before, the caption named only the first image.
    let each = || {
        found
            .iter()
            .map(|(p, i)| if i.alt.trim().is_empty() { p.clone() } else { i.alt.trim().to_owned() })
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let caption = written.clone().unwrap_or_else(|| {
        if found.len() > 1 {
            each()
        } else if !alt.is_empty() {
            image.alt.clone()
        } else {
            path.to_owned()
        }
    });
    out.push_str(&format!("<span class=\"ws-caption\">{}</span>", escape(&caption)));
    // Round 11: the alternative text is visible where it does not already stand there as the caption.
    if found.len() == 1 && !alt.is_empty() && written.as_deref().is_some_and(|w| w.trim() != alt) {
        out.push_str(&format!("<span class=\"ws-alt\">Alternativtext: {}</span>", escape(alt)));
    }
    if found.len() > 1 {
        let paths: Vec<String> = found.iter().map(|(p, _)| escape(p)).collect();
        out.push_str(&format!("<span class=\"ws-alt ws-paths\">Bilder: {}</span>", paths.join(" · ")));
    }
    out.push_str("</div>");
}

/// List items as `<ul>`/`<ol>` (Round 18b: nested items as a list in the `<li>` of the item before,
/// with the same marker). `attrs` stands only on the outer list (`start` of an enumeration).
fn render_items(out: &mut String, tag: &str, attrs: &str, items: &[Vec<Inline>], depths: &[u8], env: Env) {
    out.push_str(&format!("<{tag}{attrs}>"));
    let mut open = 0usize;
    for (k, item) in items.iter().enumerate() {
        let depth = usize::from(depths.get(k).copied().unwrap_or(0)).min(open + usize::from(k > 0));
        if k > 0 {
            if depth > open {
                out.push_str(&format!("<{tag}>"));
            } else {
                out.push_str("</li>");
                for _ in depth..open {
                    out.push_str(&format!("</{tag}></li>"));
                }
            }
        }
        open = depth;
        out.push_str("<li>");
        render_inlines(out, item, env);
    }
    if !items.is_empty() {
        out.push_str("</li>");
    }
    for _ in 0..open {
        out.push_str(&format!("</{tag}></li>"));
    }
    out.push_str(&format!("</{tag}>"));
}

fn render_body(out: &mut String, body: &[Block], env: Env) {
    out.push_str("<div class=\"ws-body\">");
    for block in body {
        render_block(out, block, None, env);
    }
    out.push_str("</div>");
}

fn render_inlines(out: &mut String, inlines: &[Inline], env: Env) {
    for (k, inline) in inlines.iter().enumerate() {
        // Round 21b: Typst swallows the line end after a forced break (`\`); in the typeset view the next line
        // would otherwise stand indented by a space. The element stays (the widget reads it back as `soft`).
        if matches!(inline, Inline::Soft) && k > 0 && matches!(inlines[k - 1], Inline::Linebreak) {
            out.push_str("<span class=\"ws-soft\" data-after-break=\"1\"></span>");
            continue;
        }
        match inline {
            Inline::Text { v } => {
                // Non-breaking spaces (`~`) stay recognisable, the browser otherwise sets its own.
                for (k, part) in v.split('\u{a0}').enumerate() {
                    if k > 0 {
                        out.push_str("<span class=\"ws-nbsp\">&nbsp;</span>");
                    }
                    out.push_str(&escape(part));
                }
            }
            Inline::Soft => out.push_str("<span class=\"ws-soft\"> </span>"),
            Inline::Emph { c } => wrap(out, "<em>", c, "</em>", env),
            Inline::Strong { c } => wrap(out, "<strong>", c, "</strong>", env),
            Inline::Footnote { c } => wrap(out, "<span class=\"ws-footnote\">", c, "</span>", env),
            Inline::Quote { c } => wrap(out, "<q class=\"ws-quote\">", c, "</q>", env),
            Inline::Label { v } => out.push_str(&format!(
                "<span class=\"ws-label\" contenteditable=\"false\" data-v=\"{0}\">&lt;{0}&gt;</span>",
                escape(v)
            )),
            Inline::Island { raw } => {
                let found = images_of(raw, env);
                if found.is_empty() {
                    out.push_str(&format!(
                        "<span class=\"ws-island\" contenteditable=\"false\" title=\"Typst-Insel\" data-raw=\"{0}\">{0}</span>",
                        escape(raw)
                    ));
                } else {
                    // An inline image appears as an image (all images of the island); the raw text is only in data-raw.
                    out.push_str(&format!(
                        "<span class=\"ws-island ws-inline-image\" contenteditable=\"false\" title=\"{IMAGE_TITLE}\" data-raw=\"{}\">{}</span>",
                        escape(raw),
                        image_tags(&found)
                    ));
                }
            }
            Inline::IndexMark { mark } => render_index_mark(out, mark),
            Inline::Linebreak => out.push_str(
                "<span class=\"ws-linebreak\" contenteditable=\"false\" data-linebreak=\"1\" title=\"erzwungener Zeilenumbruch (\\)\">↵<br></span>",
            ),
            Inline::Link { url, c } => {
                // Only addresses with a safe scheme become the target; the text of the link is editable, the
                // address only in the source text (or through the ribbon).
                let safe = ["http://", "https://", "mailto:"].iter().any(|s| url.to_ascii_lowercase().starts_with(s));
                let href = if safe { format!(" href=\"{}\" rel=\"noopener noreferrer\"", escape(url)) } else { String::new() };
                match c {
                    Some(c) => {
                        out.push_str(&format!(
                            "<a class=\"ws-link\" data-url=\"{0}\"{href} title=\"Link: {0}\">",
                            escape(url)
                        ));
                        render_inlines(out, c, env);
                        out.push_str("</a>");
                    }
                    None => out.push_str(&format!(
                        "<a class=\"ws-link\" data-url=\"{0}\" data-bare=\"1\" contenteditable=\"false\"{href} title=\"Link: {0}\">{0}</a>",
                        escape(url)
                    )),
                }
            }
            Inline::Xref { key, c, page } => {
                // Round 30a: cross-reference as a block; the link text is editable, target and page number addition are
                // not (Ctrl+click jumps, a click opens „Öffnen | Bearbeiten“ in the widget).
                out.push_str(&format!(
                    "<span class=\"ws-xref\" data-key=\"{key}\" data-page=\"{}\" title=\"Querverweis auf {} \
                     (Strg+Klick: springen)\">",
                    if *page { "1" } else { "0" },
                    baruch_typst::xref::key_label(*key)
                ));
                render_inlines(out, c, env);
                if *page {
                    // Round 39b: page abbreviation of the work language, like `book.xref.page_abbr` in the book.
                    let abbr = baruch_core::book_text::book_t(env.lang, "book.xref.page_abbr", &[]);
                    out.push_str(&format!(
                        "<span class=\"ws-xref-page\" contenteditable=\"false\">, {}\u{a0}…</span>",
                        escape(&abbr)
                    ));
                }
                out.push_str("</span>");
            }
            Inline::Ref { target, supplement } => {
                let shown = match supplement {
                    Some(s) => format!("@{target}[{s}]"),
                    None => format!("@{target}"),
                };
                let extra = supplement
                    .as_ref()
                    .map(|s| format!(" data-supplement=\"{}\"", escape(s)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "<button type=\"button\" class=\"ws-ref\" contenteditable=\"false\" tabindex=\"-1\" data-target=\"{0}\"{extra} title=\"Verweis auf &lt;{0}&gt; (nur im Quelltext änderbar)\">{1}</button>",
                    escape(target),
                    escape(&shown)
                ));
            }
        }
    }
}

/// Designation of an index for the description of a mark: built in with title („Stichwortregister“), own ones
/// as „Register „slug““ (the pure crate does not know the titles of own indexes).
fn register_title(slug: &str) -> String {
    baruch_typst::index::builtin_registers()
        .into_iter()
        .find(|r| r.slug == slug)
        .map_or_else(|| format!("Register „{slug}“"), |r| r.title)
}

/// Description of an index mark for `title` and `aria-label` in the typeset view (R31c): „Registereintrag im
/// Stichwortregister: Gnade › im Alltag; siehe auch: Huld“. The same rule is in `markLabel` in
/// `web/src/ws_command.ts` (Node comparison test in the server).
pub fn index_mark_label(mark: &baruch_typst::index::IndexMark) -> String {
    let mut label = format!("Registereintrag im {}: {}", register_title(&mark.register), mark.term);
    if !mark.sub.is_empty() {
        label.push_str(" › ");
        label.push_str(&mark.sub);
    }
    if !mark.see.is_empty() {
        label.push_str("; siehe: ");
        label.push_str(&mark.see);
    }
    if !mark.also.is_empty() {
        label.push_str("; siehe auch: ");
        label.push_str(&mark.also);
    }
    label
}

/// Index mark in the typeset view (R31c): an empty element that cannot be typed into
/// (`contenteditable="false"`, without text node – the sign comes from the style `::before`, so that text,
/// search and selection of the paragraph stay untouched), with description in `title`/`aria-label` and the
/// fields as `data-register`, `data-term`, `data-sub`, `data-see`, `data-also`, `data-sort` (empty ones are
/// missing). `data-raw` = canonical Typst form ([`baruch_typst::index::mark_source`]). The widget reads
/// `{t: "index_mark", mark}` from it; editing is done through the context menu.
fn render_index_mark(out: &mut String, mark: &baruch_typst::index::IndexMark) {
    let label = escape(&index_mark_label(mark));
    out.push_str(&format!(
        "<span class=\"ws-index-mark\" contenteditable=\"false\" role=\"img\" tabindex=\"-1\" title=\"{label}\" \
         aria-label=\"{label}\" data-register=\"{}\" data-term=\"{}\"",
        escape(&mark.register),
        escape(&mark.term)
    ));
    for (name, value) in [("sub", &mark.sub), ("see", &mark.see), ("also", &mark.also), ("sort", &mark.sort)] {
        if !value.is_empty() {
            out.push_str(&format!(" data-{name}=\"{}\"", escape(value)));
        }
    }
    out.push_str(&format!(" data-raw=\"{}\"></span>", escape(&baruch_typst::index::mark_source(mark))));
}

fn wrap(out: &mut String, open: &str, inlines: &[Inline], close: &str, env: Env) {
    out.push_str(open);
    render_inlines(out, inlines, env);
    out.push_str(close);
}

/// HTML escaping for text and attribute values.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Step in the difference: equal block (index in the proposal), removed (index in the base), new.
enum Step {
    Same(usize),
    Del(usize),
    Ins(usize),
}

/// Largest table for the longest common subsequence; above that everything counts as replaced.
const MAX_CELLS: usize = 4_000_000;

/// Difference of two texts in the typeset view: blocks according to the longest common subsequence over `raw`
/// (compared without trailing whitespace). Equal blocks stand in `div.ws-same`, removed ones in
/// `div.ws-del`, new ones in `div.ws-ins`; all in `div.ws-diff-doc`. Equal to [`diff_html_with`] without
/// images.
pub fn diff_html(base: &str, proposal: &str) -> String {
    diff_html_with(base, proposal, &|_| None)
}

/// Like [`diff_html`], images in islands as with [`render_html_with`].
pub fn diff_html_with(base: &str, proposal: &str, images: &dyn Fn(&str) -> Option<ImageRef>) -> String {
    diff_html_in(base, proposal, images, WorkLanguage::De)
}

/// Like [`diff_html_with`] in the work language `lang`, cross references as with [`render_html_in`].
pub fn diff_html_in(
    base: &str,
    proposal: &str,
    images: &dyn Fn(&str) -> Option<ImageRef>,
    lang: WorkLanguage,
) -> String {
    let env = Env { images, lang };
    let old = parse(base).blocks;
    let new = parse(proposal).blocks;
    let key = |b: &Block| b.raw.trim_end().to_owned();
    let a: Vec<String> = old.iter().map(key).collect();
    let b: Vec<String> = new.iter().map(key).collect();
    let mut steps = Vec::new();
    if a.len().saturating_mul(b.len()) <= MAX_CELLS {
        // lcs[i][j] = length of the common subsequence of a[i..] and b[j..].
        let width = b.len() + 1;
        let mut lcs = vec![0u32; (a.len() + 1) * width];
        for i in (0..a.len()).rev() {
            for j in (0..b.len()).rev() {
                lcs[i * width + j] = if a[i] == b[j] {
                    lcs[(i + 1) * width + j + 1] + 1
                } else {
                    lcs[(i + 1) * width + j].max(lcs[i * width + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < a.len() && j < b.len() {
            if a[i] == b[j] {
                steps.push(Step::Same(j));
                i += 1;
                j += 1;
            } else if lcs[(i + 1) * width + j] >= lcs[i * width + j + 1] {
                steps.push(Step::Del(i));
                i += 1;
            } else {
                steps.push(Step::Ins(j));
                j += 1;
            }
        }
        steps.extend((i..a.len()).map(Step::Del));
        steps.extend((j..b.len()).map(Step::Ins));
    } else {
        steps.extend((0..a.len()).map(Step::Del));
        steps.extend((0..b.len()).map(Step::Ins));
    }
    let mut out = String::from("<div class=\"ws-diff-doc\">");
    for step in steps {
        let (class, block) = match step {
            Step::Same(j) => ("ws-same", &new[j]),
            Step::Del(i) => ("ws-del", &old[i]),
            Step::Ins(j) => ("ws-ins", &new[j]),
        };
        out.push_str(&format!("<div class=\"{class}\">"));
        render_block(&mut out, block, None, env);
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}
