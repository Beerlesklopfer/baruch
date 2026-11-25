// SPDX-License-Identifier: AGPL-3.0-or-later
//! Änderungsverfolgung auflösen (Aufgabe docx_import, Vertrag Runde 32 §2): ein [`DocxDocument`] mit
//! [`crate::Inline::Tracked`] und `mark` wird zu einem Stand ohne beides.
//!
//! Regeln:
//! - Eine Einfügung bleibt, wo sie angewandt ist (Endstand), eine Löschung, wo sie nicht angewandt ist (Original).
//! - Absatzmarken: Ist eine eingefügte Marke nicht angewandt, gab es die Trennung noch nicht – der Absatz geht im
//!   folgenden auf (dessen Art gilt). Ist eine gelöschte Marke angewandt, nimmt der Absatz den folgenden auf (seine
//!   eigene Art gilt). Vereint wird nur mit einem unmittelbar folgenden Absatz, einer Überschrift oder einem Teil
//!   bzw. innerhalb einer Liste mit dem folgenden Punkt; sonst bleibt der Absatz für sich.
//! - Was dabei leer wird (ein ganz eingefügter Absatz im Original), entfällt.

use crate::{Block, Change, ChangeKind, DocxDocument, Inline, ListItem, ooxml};

/// Welcher Stand entsteht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View<'a> {
    /// Stand vor allen Änderungen (Löschungen bleiben, Einfügungen entfallen).
    Original,
    /// Stand nach allen Änderungen (Einfügungen bleiben, Löschungen entfallen).
    Final,
    /// Original, auf das nur die Änderungen dieses Bearbeiters (wörtlicher Name) angewandt sind.
    FinalBy(&'a str),
}

type Applies<'a> = &'a dyn Fn(&Change) -> bool;

/// Wie ein Absatz mit dem folgenden zusammengehört.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Join {
    No,
    /// Geht im folgenden auf.
    IntoNext,
    /// Nimmt den folgenden auf.
    WithNext,
}

fn join_of(mark: &Option<Change>, applies: Applies<'_>) -> Join {
    match mark {
        Some(change) if change.kind == ChangeKind::Insert && !applies(change) => Join::IntoNext,
        Some(change) if change.kind == ChangeKind::Delete && applies(change) => Join::WithNext,
        _ => Join::No,
    }
}

fn push_inline(out: &mut Vec<Inline>, inline: Inline) {
    if let (Some(Inline::Text { text: last, marks: last_marks }), Inline::Text { text, marks }) =
        (out.last_mut(), &inline)
        && last_marks == marks
    {
        last.push_str(text);
        return;
    }
    out.push(inline);
}

fn inlines(content: &[Inline], applies: Applies<'_>) -> Vec<Inline> {
    let mut out = Vec::new();
    for inline in content {
        match inline {
            Inline::Tracked { change, content } => {
                let kept = match change.kind {
                    ChangeKind::Insert => applies(change),
                    ChangeKind::Delete => !applies(change),
                };
                if kept {
                    for inner in inlines(content, applies) {
                        push_inline(&mut out, inner);
                    }
                }
            }
            Inline::Footnote { content } => out.push(Inline::Footnote { content: inlines(content, applies) }),
            Inline::Quote { content } => {
                let content = inlines(content, applies);
                if !content.is_empty() {
                    out.push(Inline::Quote { content });
                }
            }
            Inline::Link { url, content } => {
                let content = inlines(content, applies);
                if !content.is_empty() {
                    out.push(Inline::Link { url: url.clone(), content });
                }
            }
            Inline::Typst { raw, shown } => {
                out.push(Inline::Typst { raw: raw.clone(), shown: inlines(shown, applies) });
            }
            other => push_inline(&mut out, other.clone()),
        }
    }
    out
}

fn is_blank(content: &[Inline]) -> bool {
    content.iter().all(|inline| match inline {
        Inline::Text { text, .. } => text.trim().is_empty(),
        Inline::LineBreak | Inline::CommentStart { .. } | Inline::CommentEnd { .. } => true,
        Inline::Quote { content } | Inline::Link { content, .. } | Inline::Tracked { content, .. } => is_blank(content),
        Inline::Footnote { .. } | Inline::Image { .. } | Inline::Typst { .. } => false,
    })
}

/// Text ohne Auszeichnung (ohne Fußnoten, Bilder und Inseln).
pub(crate) fn plain(content: &[Inline]) -> String {
    fn walk(content: &[Inline], out: &mut String) {
        for inline in content {
            match inline {
                Inline::Text { text, .. } => out.push_str(text),
                Inline::LineBreak => out.push(' '),
                Inline::Quote { content } | Inline::Link { content, .. } | Inline::Tracked { content, .. } => {
                    walk(content, out);
                }
                _ => {}
            }
        }
    }
    let mut out = String::new();
    walk(content, &mut out);
    out.trim().to_owned()
}

/// Teil oder Überschrift der Gliederung (Ebene 1–3)?
pub(crate) fn is_outline(block: &Block) -> bool {
    match block {
        Block::Part { .. } => true,
        Block::Heading { level, .. } => *level <= ooxml::MAX_OUTLINE_LEVEL,
        _ => false,
    }
}

/// Beginnt das Dokument mit Text vor der ersten Überschrift der Gliederung (Seitenumbrüche zählen nicht)?
pub(crate) fn has_preamble(blocks: &[Block]) -> bool {
    blocks.iter().find(|b| !matches!(b, Block::PageBreak)).is_some_and(|b| !is_outline(b))
}

fn content_mut(block: &mut Block) -> Option<&mut Vec<Inline>> {
    match block {
        Block::Paragraph { content, .. } | Block::Heading { content, .. } | Block::Part { content, .. } => {
            Some(content)
        }
        _ => None,
    }
}

/// Zustand für [`proposal_view`]: Überschriften der Gliederung bleiben wie im Original.
struct Proposal {
    changes: Vec<HeadingChange>,
    /// Zahl der bisher gesehenen Abschnitte (samt Vorspann).
    seen: usize,
}

fn bold(content: &mut [Inline]) {
    for inline in content {
        match inline {
            Inline::Text { marks, .. } => marks.bold = true,
            Inline::Quote { content } | Inline::Link { content, .. } => bold(content),
            _ => {}
        }
    }
}

fn list_items(items: &[ListItem], applies: Applies<'_>) -> Vec<ListItem> {
    let mut out: Vec<ListItem> = Vec::new();
    let mut carry: Option<(ListItem, Join)> = None;
    for item in items {
        let mut current = ListItem { depth: item.depth, content: inlines(&item.content, applies), mark: None };
        if let Some((mut previous, join)) = carry.take() {
            if join == Join::IntoNext {
                previous.content.append(&mut current.content);
                current.content = previous.content;
            } else {
                previous.content.append(&mut current.content);
                current = previous;
            }
        }
        match join_of(&item.mark, applies) {
            Join::No => out.push(current),
            join => carry = Some((current, join)),
        }
    }
    out.extend(carry.map(|(item, _)| item));
    out.retain(|item| !is_blank(&item.content));
    out
}

fn blocks(source: &[Block], applies: Applies<'_>, mut proposal: Option<&mut Proposal>) -> Vec<Block> {
    let protect = proposal.is_some();
    let mut out: Vec<Block> = Vec::new();
    let mut carry: Option<(Block, Join)> = None;
    let flush = |out: &mut Vec<Block>, block: Block| {
        let empty = match &block {
            Block::Paragraph { content, .. } | Block::Heading { content, .. } | Block::Part { content, .. } => {
                is_blank(content)
            }
            Block::List { items, .. } => items.is_empty(),
            _ => false,
        };
        if !empty {
            out.push(block);
        }
    };
    for block in source {
        let (mut current, mut join) = match block {
            Block::Paragraph { content, mark } => {
                (Block::Paragraph { content: inlines(content, applies), mark: None }, join_of(mark, applies))
            }
            Block::Part { content, mark } | Block::Heading { content, mark, .. } => {
                let rebuilt = |content: Vec<Inline>| match block {
                    Block::Heading { level, numbered, .. } => {
                        Block::Heading { level: *level, numbered: *numbered, content, mark: None }
                    }
                    _ => Block::Part { content, mark: None },
                };
                match proposal.as_deref_mut().filter(|_| is_outline(block)) {
                    Some(state) => {
                        let original = inlines(content, &|_| false);
                        let mut proposed = inlines(content, applies);
                        let (before, after) = (plain(&original), plain(&proposed));
                        if before.is_empty() {
                            // Vom Bearbeiter eingefügt: kein Abschnitt, der Text bleibt als fetter Absatz.
                            if !after.is_empty() {
                                state.changes.push(HeadingChange {
                                    section: state.seen.saturating_sub(1),
                                    before,
                                    after,
                                });
                                bold(&mut proposed);
                            }
                            (Block::Paragraph { content: proposed, mark: None }, join_of(mark, applies))
                        } else {
                            if before != after {
                                state.changes.push(HeadingChange { section: state.seen, before, after });
                            }
                            state.seen += 1;
                            (rebuilt(original), Join::No)
                        }
                    }
                    None => (rebuilt(inlines(content, applies)), join_of(mark, applies)),
                }
            }
            Block::Quote { blocks: inner, attribution } => (
                Block::Quote { blocks: blocks(inner, applies, None), attribution: inlines(attribution, applies) },
                Join::No,
            ),
            Block::List { ordered, start, items } => {
                (Block::List { ordered: *ordered, start: *start, items: list_items(items, applies) }, Join::No)
            }
            Block::Table { header_rows, rows } => (
                Block::Table {
                    header_rows: *header_rows,
                    rows: rows.iter().map(|row| row.iter().map(|cell| blocks(cell, applies, None)).collect()).collect(),
                },
                Join::No,
            ),
            Block::Figure { image, alt, caption, raw } => (
                Block::Figure { image: *image, alt: alt.clone(), caption: inlines(caption, applies), raw: raw.clone() },
                Join::No,
            ),
            Block::Typst { raw, shown } => {
                (Block::Typst { raw: raw.clone(), shown: blocks(shown, applies, None) }, Join::No)
            }
            Block::PageBreak => (Block::PageBreak, Join::No),
        };
        if let Some((mut previous, previous_join)) = carry.take() {
            let fixed = protect && (is_outline(&previous) || is_outline(&current));
            if fixed || content_mut(&mut current).is_none() {
                flush(&mut out, previous);
            } else if previous_join == Join::IntoNext {
                let mut merged = std::mem::take(content_mut(&mut previous).expect("Absatz"));
                let own = content_mut(&mut current).expect("Absatz");
                for inline in std::mem::take(own) {
                    push_inline(&mut merged, inline);
                }
                *own = merged;
            } else {
                let merged = content_mut(&mut previous).expect("Absatz");
                for inline in std::mem::take(content_mut(&mut current).expect("Absatz")) {
                    push_inline(merged, inline);
                }
                current = previous;
            }
        }
        if content_mut(&mut current).is_none() || (protect && is_outline(&current)) {
            join = Join::No;
        }
        match join {
            Join::No => flush(&mut out, current),
            join => carry = Some((current, join)),
        }
    }
    if let Some((block, _)) = carry {
        flush(&mut out, block);
    }
    out
}

fn with(document: &DocxDocument, blocks: Vec<Block>) -> DocxDocument {
    DocxDocument {
        meta: document.meta.clone(),
        blocks,
        comments: document.comments.clone(),
        images: document.images.clone(),
    }
}

/// Löst die Änderungen auf; das Ergebnis enthält weder [`crate::Inline::Tracked`] noch ein `mark`.
pub fn resolve(document: &DocxDocument, view: View<'_>) -> DocxDocument {
    let resolved = match view {
        View::Original => blocks(&document.blocks, &|_| false, None),
        View::Final => blocks(&document.blocks, &|_| true, None),
        View::FinalBy(author) => blocks(&document.blocks, &|change| change.author == author, None),
    };
    with(document, resolved)
}

/// Bearbeiter aller verfolgten Änderungen: wörtliche Namen in der Reihenfolge des ersten Auftretens.
pub fn change_authors(document: &DocxDocument) -> Vec<String> {
    fn note(change: &Change, out: &mut Vec<String>) {
        if !out.contains(&change.author) {
            out.push(change.author.clone());
        }
    }
    fn in_inlines(content: &[Inline], out: &mut Vec<String>) {
        for inline in content {
            match inline {
                Inline::Tracked { change, content } => {
                    note(change, out);
                    in_inlines(content, out);
                }
                Inline::Footnote { content } | Inline::Quote { content } | Inline::Link { content, .. } => {
                    in_inlines(content, out);
                }
                Inline::Typst { shown, .. } => in_inlines(shown, out),
                _ => {}
            }
        }
    }
    fn in_blocks(blocks: &[Block], out: &mut Vec<String>) {
        for block in blocks {
            match block {
                Block::Paragraph { content, mark }
                | Block::Part { content, mark }
                | Block::Heading { content, mark, .. } => {
                    in_inlines(content, out);
                    mark.iter().for_each(|m| note(m, out));
                }
                Block::Quote { blocks, attribution } => {
                    in_blocks(blocks, out);
                    in_inlines(attribution, out);
                }
                Block::List { items, .. } => {
                    for item in items {
                        in_inlines(&item.content, out);
                        item.mark.iter().for_each(|m| note(m, out));
                    }
                }
                Block::Table { rows, .. } => rows.iter().flatten().for_each(|cell| in_blocks(cell, out)),
                Block::Figure { caption, .. } => in_inlines(caption, out),
                Block::Typst { shown, .. } => in_blocks(shown, out),
                Block::PageBreak => {}
            }
        }
    }
    let mut out = Vec::new();
    in_blocks(&document.blocks, &mut out);
    out
}

/// Geänderte Überschrift: `section` ist die Stelle des Abschnitts in der Reihenfolge des Originals (ab 0), so wie
/// [`crate::to_typst::to_typst`] die Abschnitte von `resolve(…, View::Original)` zählt (Text vor der ersten
/// Überschrift ist dort ein eigener Abschnitt 0).
///
/// - `before` und `after` gefüllt: Wortlaut der Überschrift geändert.
/// - `after` leer: Der Bearbeiter hat die Überschrift gelöscht (sie bleibt bestehen).
/// - `before` leer: Der Bearbeiter hat eine Überschrift eingefügt; sie legt keinen Abschnitt an, ihr Text steht als
///   fetter Absatz im Abschnitt `section` (dem vorangehenden; vor der ersten Überschrift: 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingChange {
    pub section: usize,
    pub before: String,
    pub after: String,
}

/// Stand für den Vorschlag eines Bearbeiters: wie [`View::FinalBy`], aber die Überschriften bleiben wie im Original
/// (Änderungen an Überschriften ändern nie die Gliederung); die geänderten Überschriften kommen getrennt zurück.
/// Überschriften der Gliederung (Teil, Ebene 1–3) werden dabei nie mit Nachbarabsätzen vereint.
pub fn proposal_view(document: &DocxDocument, author: &str) -> (DocxDocument, Vec<HeadingChange>) {
    let original = blocks(&document.blocks, &|_| false, None);
    let mut state = Proposal { changes: Vec::new(), seen: usize::from(has_preamble(&original)) };
    let resolved = blocks(&document.blocks, &|change| change.author == author, Some(&mut state));
    (with(document, resolved), state.changes)
}
