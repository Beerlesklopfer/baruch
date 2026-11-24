// SPDX-License-Identifier: AGPL-3.0-or-later
//! Blöcke als HTML für das Satzbild und den Unterschied zweier Fassungen. Jeder Text ist maskiert;
//! Inseln stehen mit `contenteditable="false"` und ihrem Rohtext da (zusätzlich in `data-raw`, damit
//! das Widget eine Insel in einem bearbeiteten Rumpf wörtlich zurückgeben kann).

use crate::{Block, BlockKind, Doc, ImageRef, Inline, Term, image_paths, parse};

/// Rückruf für Bilder: Pfad aus dem Quelltext (`image("…")`) → Adresse und Alternativtext.
type Images<'a> = &'a dyn Fn(&str) -> Option<ImageRef>;

/// HTML eines Textes: je Block `<div class="ws-block" data-b="i" data-kind="…">` (Überschrift mit
/// `data-level`, Liste mit `data-ordered`, Zitat mit `data-block`; Runde 14: Teil als
/// `<div class="ws-block ws-part" data-kind="part" data-level="0" role="heading" aria-level="1">Titel</div>`,
/// „ohne Nummer“ als `class="ws-block ws-unnumbered" data-kind="unnumbered" data-level="N"`, beide mit
/// dem Titel als Klartext). Geschachtelte Blöcke (Rumpf von
/// Zitat, Szene, Arbeit) stehen in `<div class="ws-body">` ohne `data-b`. Gleich
/// [`render_html_with`] ohne Bilder.
pub fn render_html(doc: &Doc) -> String {
    render_html_with(doc, &|_| None)
}

/// Wie [`render_html`]; zusätzlich erscheint jede Insel mit Bildpfad ([`image_path`]), für die
/// `images` ein Bild liefert, **als Bild** statt als Rohtext (Runde 7): eine Block-Insel als
/// `<div class="ws-block ws-figure" data-b=… data-kind="island" data-image="1" contenteditable="false"
/// data-raw="…" title="Bild – …"><img class="ws-image" src="…" alt="…" loading="eager"><span
/// class="ws-caption">…</span></div>` (Bildunterschrift aus `caption:`, sonst der Alternativtext,
/// sonst der Pfad), eine Insel im Satz als `<span class="ws-island ws-inline-image" …><img …></span>`.
/// Alle Attribute sind maskiert; der Rohtext steht nur in `data-raw`, die Insel bleibt beim Speichern
/// byte-gleich (`keep`).
pub fn render_html_with(doc: &Doc, images: &dyn Fn(&str) -> Option<ImageRef>) -> String {
    let mut out = String::new();
    for (index, block) in doc.blocks.iter().enumerate() {
        render_block(&mut out, block, Some(index), images);
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

/// Titel einer Insel, die als Bild erscheint.
const IMAGE_TITLE: &str = "Bild – Doppelklick öffnet den Quelltext an Ort und Stelle";

/// Titel einer Block-Insel (Runde 18b: an Ort und Stelle bearbeitbar).
const ISLAND_TITLE: &str = "Typst-Insel: Doppelklick öffnet den Quelltext an Ort und Stelle";

/// Bilder zu allen Bildpfaden in `raw` (Runde 18b: jeder `image(`-Aufruf, nicht nur der erste), für die der
/// Rückruf eins liefert: je Pfad und Bild.
fn images_of(raw: &str, images: Images) -> Vec<(String, ImageRef)> {
    image_paths(raw).into_iter().filter_map(|path| images(&path).map(|image| (path, image))).collect()
}

/// `<img>` bzw. Meldung „Bild fehlt“ für jedes Bild.
fn image_tags(found: &[(String, ImageRef)]) -> String {
    found.iter().map(|(path, image)| if image.missing { missing_tag(path) } else { image_tag(image) }).collect()
}

fn image_tag(image: &ImageRef) -> String {
    format!("<img class=\"ws-image\" src=\"{}\" alt=\"{}\" loading=\"eager\">", escape(&image.src), escape(&image.alt))
}

/// Deutliche Meldung statt eines Bilds, das es im Werk nicht gibt (Runde 11).
fn missing_tag(path: &str) -> String {
    format!("<span class=\"ws-missing\" role=\"note\">Bild fehlt: <code>{}</code></span>", escape(path))
}

/// Bildunterschrift aus `caption: […]` bzw. `caption: "…"` einer Abbildung, wörtlich (ohne Klammern).
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

pub(crate) fn render_block(out: &mut String, block: &Block, index: Option<usize>, images: Images) {
    if block.kind == BlockKind::Island {
        let found = images_of(&block.raw, images);
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
            render_inlines(out, inlines, images);
            if let Some(label) = label {
                // Label auf der Zeile hinter der Überschrift: angezeigt, beim Bearbeiten unverändert.
                out.push_str(&format!(
                    "<span class=\"ws-label ws-heading-label\" contenteditable=\"false\" data-heading-label=\"1\" data-v=\"{0}\">&lt;{0}&gt;</span>",
                    escape(label)
                ));
            }
        }
        BlockKind::Part { title } => {
            // Ein Teil steht als eigener Block mit dem Titel als Klartext (Runde 14); „Teil“ davor
            // setzt das Stylesheet (`::before`), damit es nicht in den Titel gerät.
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
            render_inlines(out, inlines, images);
        }
        BlockKind::List { ordered, items, depths } => {
            out.push_str(&format!(" data-ordered=\"{ordered}\">"));
            let tag = if *ordered { "ol" } else { "ul" };
            render_items(out, tag, "", items, depths, images);
        }
        BlockKind::Enum { start, items, depths } => {
            out.push_str(&format!(" data-start=\"{start}\">"));
            render_items(out, "ol", &format!(" start=\"{start}\""), items, depths, images);
        }
        BlockKind::Terms { items } => {
            out.push_str("><dl>");
            for Term { term, desc } in items {
                out.push_str("<dt>");
                render_inlines(out, term, images);
                out.push_str("</dt><dd>");
                render_inlines(out, desc, images);
                out.push_str("</dd>");
            }
            out.push_str("</dl>");
        }
        BlockKind::Quote { block: is_block, attribution, body } => {
            out.push_str(&format!(" data-block=\"{is_block}\">"));
            render_body(out, body, images);
            if let Some(attribution) = attribution {
                out.push_str("<div class=\"ws-attribution\">");
                render_inlines(out, attribution, images);
                out.push_str("</div>");
            }
        }
        BlockKind::Scene { args, body } | BlockKind::Work { args, body } => {
            out.push('>');
            if let Some(args) = args {
                // Argumente vor dem Rumpf: nur im Quelltext bearbeitbar, bleiben wörtlich.
                out.push_str(&format!(
                    "<div class=\"ws-args\" contenteditable=\"false\" title=\"Typst-Insel: nur im Quelltext bearbeitbar\" data-raw=\"{0}\">({0})</div>",
                    escape(args)
                ));
            }
            render_body(out, body, images);
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

/// Block-Insel mit Bild (Runde 7): das Bild mit Unterschrift statt des Rohtexts; Runde 18b: alle Bilder der
/// Insel nacheinander; Runde 21b: mit mehreren Bildern nennt die Unterschrift (ohne `caption:`) jedes Bild, und
/// `<span class="ws-alt ws-paths">Bilder: …</span>` zeigt alle Pfade.
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
    // Runde 21b: Mehrere Bilder ohne geschriebene Unterschrift nennen jedes Bild (Alternativtext, sonst Pfad), und alle
    // Pfade stehen darunter – vorher nannte die Unterschrift nur das erste Bild.
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
    // Runde 11: Der Alternativtext ist sichtbar, wo er nicht schon als Unterschrift dasteht.
    if found.len() == 1 && !alt.is_empty() && written.as_deref().is_some_and(|w| w.trim() != alt) {
        out.push_str(&format!("<span class=\"ws-alt\">Alternativtext: {}</span>", escape(alt)));
    }
    if found.len() > 1 {
        let paths: Vec<String> = found.iter().map(|(p, _)| escape(p)).collect();
        out.push_str(&format!("<span class=\"ws-alt ws-paths\">Bilder: {}</span>", paths.join(" · ")));
    }
    out.push_str("</div>");
}

/// Listenpunkte als `<ul>`/`<ol>` (Runde 18b: geschachtelte Punkte als Liste im `<li>` des Punkts davor,
/// mit demselben Zeichen). `attrs` steht nur an der äußeren Liste (`start` einer Aufzählung).
fn render_items(out: &mut String, tag: &str, attrs: &str, items: &[Vec<Inline>], depths: &[u8], images: Images) {
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
        render_inlines(out, item, images);
    }
    if !items.is_empty() {
        out.push_str("</li>");
    }
    for _ in 0..open {
        out.push_str(&format!("</{tag}></li>"));
    }
    out.push_str(&format!("</{tag}>"));
}

fn render_body(out: &mut String, body: &[Block], images: Images) {
    out.push_str("<div class=\"ws-body\">");
    for block in body {
        render_block(out, block, None, images);
    }
    out.push_str("</div>");
}

fn render_inlines(out: &mut String, inlines: &[Inline], images: Images) {
    for (k, inline) in inlines.iter().enumerate() {
        // Runde 21b: Das Zeilenende hinter einem erzwungenen Umbruch (`\`) verschluckt Typst; im Satzbild stünde die
        // nächste Zeile sonst ein Leerzeichen eingerückt. Das Element bleibt (das Widget liest es als `soft` zurück).
        if matches!(inline, Inline::Soft) && k > 0 && matches!(inlines[k - 1], Inline::Linebreak) {
            out.push_str("<span class=\"ws-soft\" data-after-break=\"1\"></span>");
            continue;
        }
        match inline {
            Inline::Text { v } => {
                // Geschützte Leerzeichen (`~`) bleiben erkennbar, der Browser setzt sonst eigene.
                for (k, part) in v.split('\u{a0}').enumerate() {
                    if k > 0 {
                        out.push_str("<span class=\"ws-nbsp\">&nbsp;</span>");
                    }
                    out.push_str(&escape(part));
                }
            }
            Inline::Soft => out.push_str("<span class=\"ws-soft\"> </span>"),
            Inline::Emph { c } => wrap(out, "<em>", c, "</em>", images),
            Inline::Strong { c } => wrap(out, "<strong>", c, "</strong>", images),
            Inline::Footnote { c } => wrap(out, "<span class=\"ws-footnote\">", c, "</span>", images),
            Inline::Quote { c } => wrap(out, "<q class=\"ws-quote\">", c, "</q>", images),
            Inline::Label { v } => out.push_str(&format!(
                "<span class=\"ws-label\" contenteditable=\"false\" data-v=\"{0}\">&lt;{0}&gt;</span>",
                escape(v)
            )),
            Inline::Island { raw } => {
                let found = images_of(raw, images);
                if found.is_empty() {
                    out.push_str(&format!(
                        "<span class=\"ws-island\" contenteditable=\"false\" title=\"Typst-Insel\" data-raw=\"{0}\">{0}</span>",
                        escape(raw)
                    ));
                } else {
                    // Ein Bild im Satz erscheint als Bild (alle Bilder der Insel); der Rohtext steht nur in data-raw.
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
                // Nur Adressen mit sicherem Schema werden zum Ziel; der Text des Links ist bearbeitbar, die
                // Adresse nur im Quelltext (bzw. über das Menüband).
                let safe = ["http://", "https://", "mailto:"].iter().any(|s| url.to_ascii_lowercase().starts_with(s));
                let href = if safe { format!(" href=\"{}\" rel=\"noopener noreferrer\"", escape(url)) } else { String::new() };
                match c {
                    Some(c) => {
                        out.push_str(&format!(
                            "<a class=\"ws-link\" data-url=\"{0}\"{href} title=\"Link: {0}\">",
                            escape(url)
                        ));
                        render_inlines(out, c, images);
                        out.push_str("</a>");
                    }
                    None => out.push_str(&format!(
                        "<a class=\"ws-link\" data-url=\"{0}\" data-bare=\"1\" contenteditable=\"false\"{href} title=\"Link: {0}\">{0}</a>",
                        escape(url)
                    )),
                }
            }
            Inline::Xref { key, c, page } => {
                // Runde 30a: Querverweis als Block; der Linktext ist bearbeitbar, Ziel und Seitenzahl-Zusatz nicht
                // (Strg+Klick springt, ein Klick öffnet „Öffnen | Bearbeiten“ im Widget).
                out.push_str(&format!(
                    "<span class=\"ws-xref\" data-key=\"{key}\" data-page=\"{}\" title=\"Querverweis auf {} \
                     (Strg+Klick: springen)\">",
                    if *page { "1" } else { "0" },
                    baruch_typst::xref::key_label(*key)
                ));
                render_inlines(out, c, images);
                if *page {
                    out.push_str("<span class=\"ws-xref-page\" contenteditable=\"false\">, S.\u{a0}…</span>");
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

/// Bezeichnung eines Registers für die Beschreibung einer Marke: eingebaut mit Titel („Stichwortregister“), eigene als
/// „Register „slug““ (die Titel eigener Register kennt die reine Crate nicht).
fn register_title(slug: &str) -> String {
    baruch_typst::index::builtin_registers()
        .into_iter()
        .find(|r| r.slug == slug)
        .map_or_else(|| format!("Register „{slug}“"), |r| r.title)
}

/// Beschreibung einer Registermarke für `title` und `aria-label` im Satzbild (R31c): „Registereintrag im
/// Stichwortregister: Gnade › im Alltag; siehe auch: Huld“. Dieselbe Regel hat `markLabel` in
/// `web/src/ws_command.ts` (Node-Vergleichstest im Server).
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

/// Registermarke im Satzbild (R31c): ein leeres, nicht tippbares Element (`contenteditable="false"`, ohne Textknoten –
/// das Zeichen kommt aus dem Stil `::before`, damit Text, Suche und Markierung des Absatzes unberührt bleiben), mit
/// Beschreibung in `title`/`aria-label` und den Feldern als `data-register`, `data-term`, `data-sub`, `data-see`,
/// `data-also`, `data-sort` (leere fehlen). `data-raw` = kanonische Typst-Form ([`baruch_typst::index::mark_source`]).
/// Das Widget liest daraus `{t: "index_mark", mark}`; bearbeitet wird über das Kontextmenü.
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

fn wrap(out: &mut String, open: &str, inlines: &[Inline], close: &str, images: Images) {
    out.push_str(open);
    render_inlines(out, inlines, images);
    out.push_str(close);
}

/// HTML-Maskierung für Text und Attributwerte.
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

/// Schritt im Unterschied: gleicher Block (Index im Vorschlag), entfernt (Index in der Basis), neu.
enum Step {
    Same(usize),
    Del(usize),
    Ins(usize),
}

/// Größte Tabelle für die längste gemeinsame Teilfolge; darüber gilt alles als ersetzt.
const MAX_CELLS: usize = 4_000_000;

/// Unterschied zweier Texte im Satzbild: Blöcke nach der längsten gemeinsamen Teilfolge über `raw`
/// (ohne Leerraum am Ende verglichen). Gleiche Blöcke stehen in `div.ws-same`, entfernte in
/// `div.ws-del`, neue in `div.ws-ins`; alles in `div.ws-diff-doc`. Gleich [`diff_html_with`] ohne
/// Bilder.
pub fn diff_html(base: &str, proposal: &str) -> String {
    diff_html_with(base, proposal, &|_| None)
}

/// Wie [`diff_html`], Bilder in Inseln wie bei [`render_html_with`].
pub fn diff_html_with(base: &str, proposal: &str, images: &dyn Fn(&str) -> Option<ImageRef>) -> String {
    let old = parse(base).blocks;
    let new = parse(proposal).blocks;
    let key = |b: &Block| b.raw.trim_end().to_owned();
    let a: Vec<String> = old.iter().map(key).collect();
    let b: Vec<String> = new.iter().map(key).collect();
    let mut steps = Vec::new();
    if a.len().saturating_mul(b.len()) <= MAX_CELLS {
        // lcs[i][j] = Länge der gemeinsamen Teilfolge von a[i..] und b[j..].
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
        render_block(&mut out, block, None, images);
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}
