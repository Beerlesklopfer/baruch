// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satzbild-Editor (Vertrag Runde 5, „wysiwyg“; Feinschliff Runde 6, „satzbild“): Abschnittstexte
//! (Typst) in bearbeitbare Blöcke übersetzen und zurück. Die Crate ist rein, ohne Datenbank und
//! Dateisystem.
//!
//! - [`parse`] zerlegt einen Text in [`Block`]e. Jeder Block trägt seinen Quelltext `raw`
//!   **wörtlich**, samt den folgenden Leerzeilen; die Verkettung aller `raw` ist der Text, Byte für
//!   Byte. `parse` scheitert nie: Was sich nicht sicher abbilden lässt, wird eine
//!   [`BlockKind::Island`] („Typst-Insel“) bzw. eine [`Inline::Island`] in einem Absatz.
//! - [`serialize`] baut aus dem Original und einer Liste von [`Edit`]s den neuen Text: `Keep`
//!   übernimmt `raw` wörtlich, ein neuer oder bearbeiteter Block wird mit [`emit`] als kanonisches
//!   Typst erzeugt. Ein unberührter Block wird also nie neu geschrieben. Im Rumpf einer Szene bzw.
//!   Arbeit lassen sich einzelne Blöcke ändern ([`Edit::Body`]).
//! - [`render_html`] zeigt die Blöcke als HTML für das Satzbild (contenteditable), [`diff_html`]
//!   den Unterschied zweier Texte im Satzbild, [`sheet_style`]/[`sheet_css`] leiten Schrift, Farben
//!   und Einzüge aus den Regeln der obersten Ebene der Werkvorlage ab. [`render_html_with`] und
//!   [`diff_html_with`] zeigen zusätzlich Bilder in Inseln mit Bildpfad ([`image_path`]).
//! - [`align`] richtet zwei Fassungen nach obersten Blöcken aus und liefert die geänderten,
//!   eingefügten und entfernten Stücke ([`Hunk`], Runde 8); [`hunk_html_with`] zeigt ein Stück im
//!   Satzbild. Damit stehen Vorschläge an ihrer Stelle im Lesefluss.
//!
//! Erkannt werden: Absatz, Überschrift `=` … `======`, Teil und „ohne Nummer“ als Markierung (Runde 14,
//! [`BlockKind::Part`], [`BlockKind::Unnumbered`], nur auf oberster Ebene), `_…_`/`#emph[…]`, `*…*`/`#strong[…]`,
//! `#footnote[…]`, `#quote[…]` (im Absatz) und `#quote(block: …, attribution: […])[…]` (als Block),
//! Listen mit `- ` bzw. `+ `, `#szene[…]`/`#szene(…)[…]`, `#arbeit[…]` (eigene Blöcke mit Blöcken im
//! Rumpf), Labels `<…>` (auch allein auf der Zeile hinter einer Überschrift), Maskierungen `\x`, `~`.
//! Runde 18b („konstrukte“): Aufzählungen `1.` ([`BlockKind::Enum`]), Begriffslisten `/ Begriff: Erklärung`
//! ([`BlockKind::Terms`]), geschachtelte Listen (`depths`), erzwungener Zeilenumbruch `\`
//! ([`Inline::Linebreak`]), `#link("…")[…]` ([`Inline::Link`]) und Verweise `@name` ([`Inline::Ref`]). Eine
//! Insel lässt sich an Ort und Stelle im Quelltext ändern ([`Edit::Raw`]).
//! Alles andere ist eine Insel: `#let`/`#set`/`#show` und andere Code-Zeilen, eigene Funktionen,
//! Mathe, Raw-Text, Kommentare, Listen mit gemischten Zeichen, eingerückte Begriffslisten. Beschreibung in
//! `docs/WYSIWYG.md`.

mod align;
mod emit;
mod html;
mod parse;
mod scan;
mod style;

use serde::{Deserialize, Serialize};

pub use align::{Hunk, HunkKind, align, hunk_html_with};
pub use emit::emit;
pub use html::{diff_html, diff_html_with, index_mark_label, render_html, render_html_with};
pub use parse::parse;
pub use scan::{image_path, image_paths};
pub use style::{BoxStyle, SheetStyle, sheet_css, sheet_style, sheet_style_over, template_font};

/// Ein Abschnittstext als Folge von Blöcken.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

/// Ein Block mit seinem wörtlichen Quelltext (inklusive der folgenden Leerzeilen). In JSON stehen
/// die Felder der Art und `raw` nebeneinander: `{"kind": "paragraph", "inlines": […], "raw": "…"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    #[serde(flatten)]
    pub kind: BlockKind,
    #[serde(default)]
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BlockKind {
    /// `=` (1) bis `======` (6) am Zeilenanfang; eine Zeile. Ein Label am Zeilenende steht in
    /// `inlines`; ein Label allein auf der Zeile dahinter (`= Titel⏎<name>`) gehört ebenfalls zur
    /// Überschrift und steht in `label`.
    Heading {
        level: u8,
        inlines: Vec<Inline>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    /// `#quote(block: …, attribution: […])[…]` als ganzer Block.
    Quote {
        block: bool,
        attribution: Option<Vec<Inline>>,
        body: Vec<Block>,
    },
    /// `- ` (ungeordnet) bzw. `+ ` (geordnet). Runde 18b: geschachtelt mit `depths` (Tiefe je Punkt, 0 =
    /// oberste Ebene, je Ebene zwei Leerzeichen Einzug; leer = alle 0). Geschachtelte Punkte tragen dasselbe
    /// Zeichen wie die Liste; gemischte Zeichen bleiben eine Insel.
    List {
        ordered: bool,
        items: Vec<Vec<Inline>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        depths: Vec<u8>,
    },
    /// Runde 18b: Aufzählung mit Ziffern `1. …`; `start` = Nummer des ersten Punkts, danach fortlaufend (je
    /// geschachtelter Gruppe ab 1). `depths` wie bei [`BlockKind::List`].
    Enum {
        #[serde(default = "one")]
        start: u64,
        items: Vec<Vec<Inline>>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        depths: Vec<u8>,
    },
    /// Runde 18b: Begriffsliste `/ Begriff: Erklärung` (ohne Schachtelung).
    Terms {
        items: Vec<Term>,
    },
    /// `#szene[…]` als ganzer Block, auch mit Argumenten davor (`#szene(ort: [Küche])[…]`, wörtlich in
    /// `args`, ohne die Klammern). Der Rumpf besteht aus eigenen, einzeln bearbeitbaren Blöcken.
    Scene {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<String>,
        body: Vec<Block>,
    },
    /// `#arbeit[…]` als ganzer Block (wie [`BlockKind::Scene`]).
    Work {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<String>,
        body: Vec<Block>,
    },
    /// Runde 14: Teil-Markierung `#heading(level: 1, numbering: none)[T] <baruch-part>` (eine Zeile,
    /// [`baruch_typst::marker`]); `title` ist der aufgelöste Titel. Neu erzeugt mit
    /// [`baruch_typst::part_line`], beim Umbenennen bleibt ein Kommentar der Zeile erhalten.
    Part {
        title: String,
    },
    /// Runde 14: Markierung „ohne Nummer“ `#heading(level: N, numbering: none)[T] <baruch-unnumbered>`
    /// (`level` 1–3, eine Zeile); neu erzeugt mit [`baruch_typst::unnumbered_line`].
    Unnumbered {
        level: u8,
        title: String,
    },
    /// Typst-Insel: wird nie neu erzeugt, nur wörtlich übernommen oder als Ganzes gelöscht.
    Island,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Inline {
    /// Text ohne Maskierungen (`\#` → `#`, `~` → geschütztes Leerzeichen).
    Text {
        v: String,
    },
    /// Einfacher Zeilenumbruch im Quelltext (im Satz ein Leerzeichen).
    Soft,
    Emph {
        c: Vec<Inline>,
    },
    Strong {
        c: Vec<Inline>,
    },
    Footnote {
        c: Vec<Inline>,
    },
    /// `#quote[…]` im Fließtext.
    Quote {
        c: Vec<Inline>,
    },
    /// `<name>`
    Label {
        v: String,
    },
    /// Typst-Insel im Absatz (Formel, Raw, eigene Funktion, Kommentar, eine Maskierung wie
    /// `\\'` oder `\\-`, die als Text eine Kurzschreibweise würde), wörtlich.
    Island {
        raw: String,
    },
    /// Runde 18b: erzwungener Zeilenumbruch `\` (Backslash vor Leerraum, Zeilenende oder am Ende).
    Linebreak,
    /// Runde 18b: `#link("adresse")` bzw. `#link("adresse")[Text]`; `c` = Text des Links (`None` = ohne
    /// Inhaltsblock, angezeigt wird die Adresse).
    Link {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        c: Option<Vec<Inline>>,
    },
    /// Runde 18b: Verweis `@ziel` bzw. `@ziel[Zusatz]` (auch `#ref(<ziel>)`); `supplement` ist der Inhalt der
    /// Klammer als Typst-Markup, wörtlich. Im Satzbild eine Schaltfläche, nicht bearbeitbar.
    Ref {
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        supplement: Option<String>,
    },
    /// Runde 30a: Querverweis `#link(<k103>)[Text]` (mit Seitenzahl-Zusatz nach §3.3); `key` = fester Schlüssel.
    Xref {
        key: i64,
        c: Vec<Inline>,
        #[serde(default)]
        page: bool,
    },
    /// R31: Registermarke `#metadata((…))<baruch-idx>` ([`baruch_typst::index`]); im Satzbild dezent, bearbeitbar.
    IndexMark {
        mark: baruch_typst::index::IndexMark,
    },
}

/// Ein Eintrag einer Begriffsliste (Runde 18b): `/ term: desc`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    pub term: Vec<Inline>,
    pub desc: Vec<Inline>,
}

fn one() -> u64 {
    1
}

/// Eine Anweisung an [`serialize`]: Block `keep` des Originals wörtlich übernehmen, einen Block
/// neu erzeugen (`replaces` = Index des ersetzten Originalblocks, `None` = eingefügt) oder im Rumpf
/// einer Szene bzw. Arbeit (`#szene[…]`, `#arbeit[…]`) einzelne Blöcke ändern (`body` = Anweisungen
/// für die Blöcke des Rumpfs, nach denselben Regeln). Originalblöcke, die in keiner Anweisung
/// vorkommen, sind gelöscht. JSON: `{"keep": 3}`, `{"replaces": 2, "block": {"kind": "paragraph",
/// "inlines": […]}}` bzw. `{"replaces": 4, "body": [{"keep": 0}, {"replaces": 1, "block": …}]}`.
/// Runde 18b: `{"replaces": 5, "raw": "#let x = 2"}` ersetzt den Quelltext von Block 5 wörtlich (Insel an Ort
/// und Stelle bearbeitet); ohne `replaces` (neue Insel) → [`WysiwygError::NewIsland`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Edit {
    Keep { keep: usize },
    Block { replaces: Option<usize>, block: BlockKind },
    Body { replaces: usize, body: Vec<Edit> },
    Raw { replaces: Option<usize>, raw: String },
}

/// Ein Bild zu einer Insel mit Bildpfad (siehe [`image_path`] und [`render_html_with`]): Adresse
/// und Alternativtext für `<img>`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    pub src: String,
    pub alt: String,
    /// Runde 11: Zum Pfad gibt es kein Bild im Werk. Das Satzbild meldet dann deutlich „Bild fehlt:
    /// <Pfad>“ statt eines leeren Rahmens (die Insel bleibt byte-gleich).
    #[serde(default)]
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WysiwygError {
    #[error("Block {index} gibt es nicht; der Text hat {count} Blöcke.")]
    UnknownBlock { index: usize, count: usize },
    #[error("Block {0} ist mehrfach angegeben.")]
    DuplicateBlock(usize),
    #[error("Block {0} ist eine Typst-Insel; sie lässt sich nur im Quelltext ändern oder als Ganzes löschen.")]
    IslandEdited(usize),
    #[error("Eine Typst-Insel lässt sich im Satzbild nicht anlegen; bitte im Quelltext schreiben.")]
    NewIsland,
    #[error("Block {0} ist keine Szene und keine Arbeit; nur deren Rumpf lässt sich blockweise ändern.")]
    NotContainer(usize),
    /// Runde 18b: Ein Teil entsteht nur in der Gliederung, nicht als neuer oder umgewandelter Block im Satzbild.
    #[error("Einen Teil legt man in der Gliederung an, nicht im Satzbild.")]
    NewPart,
}

/// Neuer Text aus `original` und `edits` (Regeln siehe [`Edit`]).
///
/// - `Keep` übernimmt `raw` wörtlich; jeder Index darf höchstens einmal vorkommen (auch nicht
///   zusätzlich als `replaces`), ein unbekannter Index ist ein Fehler.
/// - Ein Block-Edit erzeugt kanonisches Typst ([`emit`]) und hängt den Trenner des ersetzten Blocks
///   an (seine folgenden Leerzeilen), bei einem eingefügten Block `\n\n`. Ein Block, der leer
///   herauskommt, entfällt.
/// - Eine bearbeitete Überschrift behält Label und Kommentar der Originalzeile, wie
///   [`baruch_typst::heading_line`] (das Label nur, wenn die neue Überschrift keins trägt).
/// - Eine Insel als neuer Block oder als Ersatz einer Insel → Fehler; ihren Quelltext ändert
///   [`Edit::Raw`] (Runde 18b: wörtlich übernommen, mit dem Trenner des alten Blocks; leer = gelöscht).
/// - Ein Teil als neuer Block oder anstelle eines anderen Blocks → [`WysiwygError::NewPart`] (Runde 18b:
///   Teile entstehen in der Gliederung); einen vorhandenen Teil umbenennen geht.
/// - Folgen zwei Stücke nicht wie im Original aufeinander (eingefügt, gelöscht, umgestellt), steht
///   dazwischen mindestens eine Leerzeile, damit nichts zu einem Absatz verschmilzt.
pub fn serialize(original: &Doc, edits: &[Edit]) -> Result<String, WysiwygError> {
    serialize_indented(original, edits, "", false)
}

/// [`serialize`] für einen Rumpf mit Einzug `indent`: Neu erzeugte Stücke bekommen ihn vor jeder
/// Zeile; Raw-Blöcke bleiben unberührt, damit ihr Inhalt sich nicht ändert. `indent_first`: auch
/// die erste Zeile des Ergebnisses bekommt ihn (der Rumpf beginnt auf einer eigenen Zeile, und der
/// erste Raw-Block trägt den Einzug seiner ersten Zeile selbst); sonst gehört er zum Rahmen.
fn serialize_indented(
    original: &Doc,
    edits: &[Edit],
    indent: &str,
    indent_first: bool,
) -> Result<String, WysiwygError> {
    let mut used = vec![false; original.blocks.len()];
    let mut take = |index: usize| take_block(original, &mut used, index);
    let mut out = String::new();
    let mut previous: Option<usize> = None;
    for edit in edits {
        let (index, piece) = match edit {
            Edit::Keep { keep } => (Some(*keep), take(*keep)?.raw.clone()),
            Edit::Block { block: BlockKind::Island, replaces: Some(index) } => {
                take(*index)?;
                return Err(WysiwygError::IslandEdited(*index));
            }
            Edit::Block { block: BlockKind::Island, replaces: None } => return Err(WysiwygError::NewIsland),
            Edit::Raw { replaces: None, .. } => return Err(WysiwygError::NewIsland),
            Edit::Raw { replaces: Some(index), raw } => {
                let old = take(*index)?;
                // Wörtlich, wie getippt; im Rumpf ohne dessen Einzug geschickt oder mit ihm (beides geht).
                let text = dedent(raw.trim_start_matches(['\n', '\r']).trim_end(), indent);
                if text.trim().is_empty() {
                    continue;
                }
                let text = indent_lines(&text, indent, indent_first || !out.is_empty());
                (Some(*index), text + separator(&old.raw))
            }
            Edit::Block { block: BlockKind::Part { .. }, replaces: None } => return Err(WysiwygError::NewPart),
            Edit::Body { replaces: index, body } => {
                let old = take(*index)?;
                let (BlockKind::Scene { body: blocks, .. } | BlockKind::Work { body: blocks, .. }) = &old.kind else {
                    return Err(WysiwygError::NotContainer(*index));
                };
                let Some((head, inner, tail)) = parse::container_frame(&old.raw) else {
                    return Err(WysiwygError::NotContainer(*index));
                };
                // Dieselbe Zerlegung wie beim Lesen, mit den `raw` aus diesem Block (byte-gleich).
                let mut inner_doc = Doc { blocks: parse::body_blocks(inner, 1) };
                debug_assert_eq!(inner_doc.blocks.len(), blocks.len(), "gleiche Zerlegung des Rumpfs");
                let start = inner.len() - inner.trim_start().len();
                let end = start.max(inner.trim_end().len());
                // Beginnt der Rumpf auf einer eigenen Zeile, gehört der Einzug dieser Zeile zum
                // ersten Block (`body_blocks` liest `inner.trim()`): Er wandert mit, wenn der Block
                // nicht mehr vorn steht, sonst rückte seine erste Zeile in Spalte 0 und seine
                // Folgezeilen blieben eingerückt (aus einer Liste würde eine geschachtelte).
                let (lead, first_indent) = match inner[..start].rfind('\n') {
                    Some(newline) => inner[..start].split_at(newline + 1),
                    None => (&inner[..start], ""),
                };
                let own_line = inner[..start].contains('\n');
                if let Some(first) = inner_doc.blocks.first_mut() {
                    first.raw.insert_str(0, first_indent);
                }
                let text = serialize_indented(&inner_doc, body, parse::body_indent(inner), own_line)?;
                let piece = format!("{head}{lead}{}{}{tail}", text.trim_end(), &inner[end..]);
                (Some(*index), piece)
            }
            Edit::Block { replaces: Some(index), block } => {
                let old = take(*index)?;
                if old.kind == BlockKind::Island {
                    return Err(WysiwygError::IslandEdited(*index));
                }
                if matches!(block, BlockKind::Part { .. }) && !matches!(old.kind, BlockKind::Part { .. }) {
                    return Err(WysiwygError::NewPart);
                }
                let mut text = emit(block);
                if text.trim().is_empty() {
                    continue;
                }
                if let Some(line) = marker_line(block, &old.raw, &old.kind) {
                    text = line;
                }
                if let (BlockKind::Heading { level, inlines, label }, BlockKind::Heading { label: old_label, .. }) =
                    (block, &old.kind)
                {
                    // Nur die Zeile bis zum ersten Umbruch ist Überschrift (Runde 7); Label und
                    // Kommentar der Originalzeile hängen an ihr, der Rest wird ein Absatz dahinter.
                    let (head, rest) = emit::split_heading(inlines);
                    text = emit::heading_text(*level, &head);
                    text.push_str(&heading_suffix(&old.raw, &head));
                    text.push_str(&heading_label_line(&old.raw, old_label.as_deref(), label.as_deref()));
                    if let Some(rest) = rest {
                        text.push_str("\n\n");
                        text.push_str(&emit(&BlockKind::Paragraph { inlines: rest }));
                    }
                }
                let text = indent_piece(&text, indent, indent_first || !out.is_empty());
                (Some(*index), text + separator(&old.raw))
            }
            Edit::Block { replaces: None, block } => {
                let text = emit(block);
                if text.trim().is_empty() {
                    continue;
                }
                (None, indent_piece(&text, indent, indent_first || !out.is_empty()) + "\n\n")
            }
        };
        let in_order = matches!((previous, index), (Some(a), Some(b)) if b == a + 1);
        if !out.is_empty() && !in_order {
            ensure_blank_line(&mut out);
        }
        out.push_str(&piece);
        previous = index;
    }
    Ok(out)
}

/// Bearbeitete Markierung (Teil bzw. „ohne Nummer“), die einen Block ersetzt: Ein Kommentar der
/// Originalzeile bleibt erhalten wie bei [`baruch_typst::heading_line`] (Teil aus jeder Zeile,
/// „ohne Nummer“ nur aus einer Markierung „ohne Nummer“). Ohne Zeilenende; `None` für andere Arten.
fn marker_line(block: &BlockKind, old_raw: &str, old_kind: &BlockKind) -> Option<String> {
    let line = old_raw.split('\n').next().unwrap_or_default();
    let built = match (block, old_kind) {
        (BlockKind::Part { title }, _) => baruch_typst::heading_line(0, title, Some(line)),
        (BlockKind::Unnumbered { level, title }, BlockKind::Unnumbered { .. }) => {
            baruch_typst::heading_line((*level).clamp(1, 3), title, Some(line))
        }
        _ => return None,
    };
    Some(built.trim_end_matches('\n').to_owned())
}

/// Originalblock `index`, einmal; unbekannt oder schon benutzt → Fehler.
fn take_block<'a>(original: &'a Doc, used: &mut [bool], index: usize) -> Result<&'a Block, WysiwygError> {
    let count = original.blocks.len();
    let block = original.blocks.get(index).ok_or(WysiwygError::UnknownBlock { index, count })?;
    if std::mem::replace(&mut used[index], true) {
        return Err(WysiwygError::DuplicateBlock(index));
    }
    Ok(block)
}

/// Neu erzeugtes Stück mit dem Einzug eines Rumpfs: vor jeder nicht leeren Zeile, die erste nur,
/// wenn das Stück nicht am Anfang des Rumpfs steht. Mit Raw-Block (```` ``` ````) bleibt es, wie es ist.
fn indent_piece(text: &str, indent: &str, first: bool) -> String {
    if text.contains("```") {
        return text.to_owned();
    }
    indent_lines(text, indent, first)
}

/// Einzug `indent` vor jede nicht leere Zeile, die erste nur mit `first` – ohne Ausnahme, auch mit
/// Raw-Block: Der Quelltext einer Insel ([`Edit::Raw`]) stand im Rumpf ganz mit dessen Einzug.
fn indent_lines(text: &str, indent: &str, first: bool) -> String {
    if indent.is_empty() {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + indent.len() * 4);
    for (k, line) in text.split('\n').enumerate() {
        if k > 0 {
            out.push('\n');
        }
        if (k > 0 || first) && !line.is_empty() {
            out.push_str(indent);
        }
        out.push_str(line);
    }
    out
}

/// Einzug `indent` eines Rumpfs entfernen (Quelltext einer Insel im Rumpf, [`Edit::Raw`]); [`indent_lines`]
/// setzt ihn danach einheitlich wieder. Entschieden wird für den ganzen Text, nicht Zeile für Zeile: Nur wenn
/// jede nicht leere Folgezeile mit dem Einzug beginnt, wurde der Text mit ihm geschickt (wie `data-raw`, die
/// erste Zeile trägt ihn dann oder nicht), und er fällt überall weg. Sonst kam der Text ohne Einzug, und
/// nichts wird entfernt – so bleibt der Einzug eingerückter Zeilen (etwa in einem Rohblock) erhalten.
fn dedent(text: &str, indent: &str) -> String {
    if indent.is_empty() {
        return text.to_owned();
    }
    let with_indent = text.split('\n').skip(1).all(|line| line.trim().is_empty() || line.starts_with(indent));
    if !with_indent {
        return text.to_owned();
    }
    text.split('\n').map(|line| line.strip_prefix(indent).unwrap_or(line)).collect::<Vec<_>>().join("\n")
}

/// Leerraum am Ende eines Blocks (Zeilenende und Leerzeilen): wird beim Ersetzen übernommen.
fn separator(raw: &str) -> &str {
    &raw[raw.trim_end().len()..]
}

/// Label und Kommentar der Originalzeile einer Überschrift, mit führendem Leerzeichen (oder leer):
/// was [`baruch_typst::heading_line`] hinter einen neuen Titel setzen würde. Trägt die neue
/// Überschrift selbst ein Label am Ende, entfällt das alte.
fn heading_suffix(old_raw: &str, inlines: &[Inline]) -> String {
    let line = old_raw.split('\n').next().unwrap_or_default();
    let full = baruch_typst::heading_line(1, "X", Some(line));
    let mut suffix = full.trim_end_matches('\n').strip_prefix("= X").unwrap_or_default().to_owned();
    let has_label = matches!(inlines.last(), Some(Inline::Label { .. }));
    if has_label
        && suffix.starts_with(" <")
        && let Some(close) = suffix.find('>')
    {
        suffix.replace_range(..close + 1, "");
    }
    suffix
}

/// Labelzeile hinter einer bearbeiteten Überschrift (mit führendem Zeilenende, oder leer): Die
/// Originalzeile bleibt wörtlich, solange das Label gleich bleibt oder das neue fehlt; ein anderes
/// Label ergibt eine neue Zeile `<name>`.
fn heading_label_line(old_raw: &str, old_label: Option<&str>, new_label: Option<&str>) -> String {
    let old_line = old_label.and_then(|_| {
        let content = old_raw.trim_end();
        content.find('\n').map(|n| &content[n..])
    });
    match (new_label, old_line) {
        (Some(new), Some(line)) if Some(new) == old_label => line.to_owned(),
        (Some(new), _) => emit::label_line(new),
        (None, Some(line)) => line.to_owned(),
        (None, None) => String::new(),
    }
}

/// Sorgt dafür, dass `out` mit einer Leerzeile endet.
fn ensure_blank_line(out: &mut String) {
    let tail = &out[out.trim_end().len()..];
    match tail.matches('\n').count() {
        0 => out.push_str("\n\n"),
        1 => out.push('\n'),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_separator_is_the_trailing_whitespace() {
        assert_eq!(separator("Text\n\n"), "\n\n", "Leerzeilen hinter dem Block");
        assert_eq!(separator("= Titel\n"), "\n", "Zeilenende einer Überschrift");
        assert_eq!(separator("Ende"), "", "letzter Block ohne Zeilenende");
    }

    #[test]
    fn the_heading_suffix_keeps_label_and_comment() {
        assert_eq!(heading_suffix("= Alt <ziel> // Notiz\n\n", &[]), " <ziel> // Notiz", "beides bleibt");
        let labelled = [Inline::Label { v: "neu".into() }];
        assert_eq!(
            heading_label_line("= Alt\n  <ziel>\n\n", Some("ziel"), None),
            "\n  <ziel>",
            "die Labelzeile bleibt wörtlich"
        );
        assert_eq!(heading_label_line("= Alt\n<ziel>\n", Some("ziel"), Some("neu")), "\n<neu>", "neues Label");
        assert_eq!(heading_suffix("= Alt <ziel> // Notiz\n", &labelled), " // Notiz", "eigenes Label ersetzt das alte");
        assert_eq!(heading_suffix("= Alt\n", &[]), "", "nichts anzuhängen");
    }
}
