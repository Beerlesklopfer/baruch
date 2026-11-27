// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lay a proposal as tracked changes over the current revision (task docx_export, contract Round 32 §3).
//!
//! Promise (contract): the state "Original" of the result corresponds to `current`, the state "Final" to `proposed`.
//!
//! Procedure:
//! 1. Align blocks: equal blocks stay without a mark ([`lcs`]).
//! 2. In every gap, similar blocks of the same kind are assigned to each other and compared **word by word**
//!    (paragraph, heading, part; quote, list, table of the same size and figure of the same image per component).
//!    If exactly one block replaces exactly one of the same kind, that always counts as a change in place.
//! 3. What remains is entirely new or dropped entirely: the content is in [`Inline::Tracked`], the paragraph mark
//!    carries `mark`.
//!
//! Limits of the model (honest, see `docs/r32/docx_export.md`):
//! - An entirely new or dropped **figure** becomes a paragraph with the image as a tracked change and a paragraph
//!   with the caption; a **Typst island** a paragraph with the island in the text – that way they really disappear
//!   in the respective other state.
//! - In an entirely new or dropped **table** the content of all cells is tracked; the empty table itself stays in
//!   both states (the model knows no tracked table row).
//! - **Page breaks** are not tracked: the result has those of the proposal.
//! - The first number of a list, the header rows of a table and the alternative text come from the proposal.

use std::collections::HashMap;

use crate::from_typst::merge;
use crate::{Block, Change, ChangeKind, Inline, ListItem};

/// Largest table of the dynamic programming (cells); above that everything counts as changed.
const MAX_CELLS: usize = 6_000_000;
/// Minimum similarity (share of common words, in thousandths) from which two blocks are compared word by word.
const MIN_SIMILARITY: u32 = 350;

/// Aligns the blocks of `current` and `proposed` and marks the differences with `change` (editor and date; the
/// kind per place is set by the function itself).
pub fn track_changes(current: &[Block], proposed: &[Block], change: &Change) -> Vec<Block> {
    Tracker { author: &change.author, date: &change.date }.blocks(current, proposed)
}

/// Step of an alignment: equal in both sequences, only in the first, only in the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Both(usize, usize),
    Left(usize),
    Right(usize),
}

/// Longest common subsequence as steps in reading order. Common beginning and end are split off beforehand; if
/// the rest is too large for the table, it counts as entirely different.
fn lcs<T: PartialEq>(a: &[T], b: &[T]) -> Vec<Step> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..].iter().rev().zip(b[prefix..].iter().rev()).take_while(|(x, y)| x == y).count();
    let (n, m) = (a.len() - prefix - suffix, b.len() - prefix - suffix);
    let mut steps: Vec<Step> = (0..prefix).map(|i| Step::Both(i, i)).collect();
    if n.saturating_mul(m) > MAX_CELLS {
        steps.extend((0..n).map(|i| Step::Left(prefix + i)));
        steps.extend((0..m).map(|j| Step::Right(prefix + j)));
    } else if n > 0 || m > 0 {
        let width = m + 1;
        let mut table = vec![0u32; (n + 1) * width];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                table[i * width + j] = if a[prefix + i] == b[prefix + j] {
                    table[(i + 1) * width + j + 1] + 1
                } else {
                    table[(i + 1) * width + j].max(table[i * width + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && a[prefix + i] == b[prefix + j] {
                steps.push(Step::Both(prefix + i, prefix + j));
                i += 1;
                j += 1;
            } else if i < n && (j == m || table[(i + 1) * width + j] >= table[i * width + j + 1]) {
                steps.push(Step::Left(prefix + i));
                i += 1;
            } else {
                steps.push(Step::Right(prefix + j));
                j += 1;
            }
        }
    }
    steps.extend((0..suffix).map(|k| Step::Both(a.len() - suffix + k, b.len() - suffix + k)));
    steps
}

/// Text of an inline element without markup (for the similarity of two blocks).
fn plain_inlines(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } => out.push_str(text),
            Inline::LineBreak => out.push(' '),
            Inline::Footnote { content }
            | Inline::Quote { content }
            | Inline::Link { content, .. }
            | Inline::Tracked { content, .. } => plain_inlines(content, out),
            Inline::Typst { raw, .. } => out.push_str(raw),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::CommentStart { .. } | Inline::CommentEnd { .. } => {}
        }
    }
}

fn plain_block(block: &Block, out: &mut String) {
    match block {
        Block::Part { content, .. } | Block::Heading { content, .. } | Block::Paragraph { content, .. } => {
            plain_inlines(content, out);
        }
        Block::Quote { blocks, attribution } => {
            blocks.iter().for_each(|b| plain_block(b, out));
            plain_inlines(attribution, out);
        }
        Block::List { items, .. } => items.iter().for_each(|item| {
            plain_inlines(&item.content, out);
            out.push(' ');
        }),
        Block::Table { rows, .. } => rows.iter().flatten().flatten().for_each(|b| plain_block(b, out)),
        Block::Figure { caption, .. } => plain_inlines(caption, out),
        Block::Typst { raw, .. } => out.push_str(raw),
        Block::PageBreak => {}
    }
    out.push(' ');
}

/// Do two blocks match in kind, so that comparing their components makes sense?
fn same_shape(a: &Block, b: &Block) -> bool {
    match (a, b) {
        (Block::Paragraph { .. }, Block::Paragraph { .. })
        | (Block::Part { .. }, Block::Part { .. })
        | (Block::Quote { .. }, Block::Quote { .. }) => true,
        (Block::Heading { level: l, numbered: n, .. }, Block::Heading { level: l2, numbered: n2, .. }) => {
            l == l2 && n == n2
        }
        (Block::List { ordered, .. }, Block::List { ordered: ordered2, .. }) => ordered == ordered2,
        (Block::Table { rows, .. }, Block::Table { rows: rows2, .. }) => {
            rows.len() == rows2.len() && rows.iter().zip(rows2).all(|(r, r2)| r.len() == r2.len())
        }
        (Block::Figure { image, .. }, Block::Figure { image: image2, .. }) => image == image2,
        _ => false,
    }
}

/// Similarity of two blocks in thousandths (share of common words); `None` = do not assign.
fn similarity(a: &Block, b: &Block) -> Option<u32> {
    if !same_shape(a, b) {
        return None;
    }
    let (mut text_a, mut text_b) = (String::new(), String::new());
    plain_block(a, &mut text_a);
    plain_block(b, &mut text_b);
    let mut counts: HashMap<&str, i64> = HashMap::new();
    let mut total_a = 0u32;
    for word in text_a.split_whitespace() {
        *counts.entry(word).or_default() += 1;
        total_a += 1;
    }
    let (mut total_b, mut common) = (0u32, 0u32);
    for word in text_b.split_whitespace() {
        total_b += 1;
        if let Some(left) = counts.get_mut(word).filter(|left| **left > 0) {
            *left -= 1;
            common += 1;
        }
    }
    if total_a + total_b == 0 {
        // Tables, figures and headings without text: the same kind is enough.
        return Some(1);
    }
    let score = 2000 * common / (total_a + total_b);
    (score >= MIN_SIMILARITY).then_some(score)
}

/// Assignment in a gap: pairs of similar blocks in the same order, the sum of the similarity is the largest possible.
fn pair_up(removed: &[&Block], added: &[&Block]) -> Vec<Step> {
    let (n, m) = (removed.len(), added.len());
    if n.saturating_mul(m) > 40_000 {
        // Very large gap: in sequence, without a search.
        return (0..n).map(Step::Left).chain((0..m).map(Step::Right)).collect();
    }
    let width = m + 1;
    let score: Vec<Option<u32>> = removed.iter().flat_map(|r| added.iter().map(|a| similarity(r, a))).collect();
    let mut best = vec![0u64; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let mut value = best[(i + 1) * width + j].max(best[i * width + j + 1]);
            if let Some(s) = score[i * m + j] {
                value = value.max(best[(i + 1) * width + j + 1] + u64::from(s));
            }
            best[i * width + j] = value;
        }
    }
    let mut steps = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        let paired = (i < n && j < m)
            .then(|| score[i * m + j])
            .flatten()
            .filter(|s| best[i * width + j] == best[(i + 1) * width + j + 1] + u64::from(*s));
        if paired.is_some() {
            steps.push(Step::Both(i, j));
            i += 1;
            j += 1;
        } else if i < n && (j == m || best[(i + 1) * width + j] >= best[i * width + j + 1]) {
            steps.push(Step::Left(i));
            i += 1;
        } else {
            steps.push(Step::Right(j));
            j += 1;
        }
    }
    steps
}

/// Splits text into words, whitespace and single punctuation marks; everything else stays an indivisible piece.
fn tokens(inlines: &[Inline]) -> Vec<Inline> {
    #[derive(PartialEq, Clone, Copy)]
    enum Class {
        Word,
        Space,
        Other,
    }
    let class = |c: char| {
        if c.is_whitespace() {
            Class::Space
        } else if c.is_alphanumeric() {
            Class::Word
        } else {
            Class::Other
        }
    };
    let mut out = Vec::new();
    for inline in inlines {
        let Inline::Text { text, marks } = inline else {
            out.push(inline.clone());
            continue;
        };
        let mut start = 0;
        let mut last: Option<Class> = None;
        for (at, c) in text.char_indices() {
            let now = class(c);
            if last.is_some_and(|l| l != now || now == Class::Other) {
                out.push(Inline::Text { text: text[start..at].to_owned(), marks: *marks });
                start = at;
            }
            last = Some(now);
        }
        if start < text.len() {
            out.push(Inline::Text { text: text[start..].to_owned(), marks: *marks });
        }
    }
    out
}

fn is_space(inline: &Inline) -> bool {
    matches!(inline, Inline::Text { text, .. } if text.chars().all(char::is_whitespace))
}

struct Tracker<'a> {
    author: &'a str,
    date: &'a str,
}

impl Tracker<'_> {
    fn change(&self, kind: ChangeKind) -> Change {
        Change { kind, author: self.author.to_owned(), date: self.date.to_owned() }
    }

    fn tracked(&self, kind: ChangeKind, content: Vec<Inline>) -> Vec<Inline> {
        if content.is_empty() { content } else { vec![Inline::Tracked { change: self.change(kind), content }] }
    }

    fn blocks(&self, current: &[Block], proposed: &[Block]) -> Vec<Block> {
        let mut out = Vec::new();
        let (mut removed, mut added): (Vec<&Block>, Vec<&Block>) = (Vec::new(), Vec::new());
        for step in lcs(current, proposed) {
            match step {
                Step::Left(i) => removed.push(&current[i]),
                Step::Right(j) => added.push(&proposed[j]),
                Step::Both(i, _) => {
                    self.gap(&removed, &added, &mut out);
                    removed.clear();
                    added.clear();
                    out.push(current[i].clone());
                }
            }
        }
        self.gap(&removed, &added, &mut out);
        out
    }

    fn gap(&self, removed: &[&Block], added: &[&Block], out: &mut Vec<Block>) {
        // Exactly one block replaces exactly one of the same kind: that is a change in place, even if no word
        // stayed the same – the paragraph persists instead of standing there as deleted and new.
        if let ([old], [new]) = (removed, added)
            && same_shape(old, new)
        {
            out.push(self.pair(old, new));
            return;
        }
        for step in pair_up(removed, added) {
            match step {
                Step::Left(i) => self.whole(removed[i], ChangeKind::Delete, out),
                Step::Right(j) => self.whole(added[j], ChangeKind::Insert, out),
                Step::Both(i, j) => out.push(self.pair(removed[i], added[j])),
            }
        }
    }

    /// Two blocks of the same kind assigned to each other: compare components.
    fn pair(&self, current: &Block, proposed: &Block) -> Block {
        match (current, proposed) {
            (Block::Paragraph { content: a, .. }, Block::Paragraph { content: b, .. }) => {
                Block::Paragraph { content: self.inlines(a, b), mark: None }
            }
            (Block::Part { content: a, .. }, Block::Part { content: b, .. }) => {
                Block::Part { content: self.inlines(a, b), mark: None }
            }
            (Block::Heading { content: a, .. }, Block::Heading { level, numbered, content: b, .. }) => {
                Block::Heading { level: *level, numbered: *numbered, content: self.inlines(a, b), mark: None }
            }
            (Block::Quote { blocks: a, attribution: source_a }, Block::Quote { blocks: b, attribution: source_b }) => {
                Block::Quote { blocks: self.blocks(a, b), attribution: self.inlines(source_a, source_b) }
            }
            (Block::List { items: a, .. }, Block::List { ordered, start, items: b }) => {
                Block::List { ordered: *ordered, start: *start, items: self.items(a, b) }
            }
            (Block::Table { rows: a, .. }, Block::Table { header_rows, rows: b }) => Block::Table {
                header_rows: *header_rows,
                rows: a
                    .iter()
                    .zip(b)
                    .map(|(row_a, row_b)| {
                        row_a.iter().zip(row_b).map(|(cell_a, cell_b)| self.blocks(cell_a, cell_b)).collect()
                    })
                    .collect(),
            },
            (Block::Figure { caption: a, .. }, Block::Figure { image, alt, caption: b, raw }) => {
                Block::Figure { image: *image, alt: alt.clone(), caption: self.inlines(a, b), raw: raw.clone() }
            }
            // `same_shape` allows nothing else; to be safe the proposal applies.
            (_, proposed) => proposed.clone(),
        }
    }

    fn items(&self, current: &[ListItem], proposed: &[ListItem]) -> Vec<ListItem> {
        let mut out = Vec::new();
        let (mut removed, mut added): (Vec<&ListItem>, Vec<&ListItem>) = (Vec::new(), Vec::new());
        let flush = |removed: &mut Vec<&ListItem>, added: &mut Vec<&ListItem>, out: &mut Vec<ListItem>| {
            let pairs = removed.len().min(added.len());
            for (old, new) in removed.iter().zip(added.iter()) {
                if old.depth == new.depth {
                    out.push(ListItem {
                        depth: new.depth,
                        content: self.inlines(&old.content, &new.content),
                        mark: None,
                    });
                } else {
                    out.push(self.whole_item(old, ChangeKind::Delete));
                    out.push(self.whole_item(new, ChangeKind::Insert));
                }
            }
            out.extend(removed[pairs..].iter().map(|item| self.whole_item(item, ChangeKind::Delete)));
            out.extend(added[pairs..].iter().map(|item| self.whole_item(item, ChangeKind::Insert)));
            removed.clear();
            added.clear();
        };
        for step in lcs(current, proposed) {
            match step {
                Step::Left(i) => removed.push(&current[i]),
                Step::Right(j) => added.push(&proposed[j]),
                Step::Both(i, _) => {
                    flush(&mut removed, &mut added, &mut out);
                    out.push(current[i].clone());
                }
            }
        }
        flush(&mut removed, &mut added, &mut out);
        out
    }

    fn whole_item(&self, item: &ListItem, kind: ChangeKind) -> ListItem {
        ListItem { depth: item.depth, content: self.tracked(kind, item.content.clone()), mark: Some(self.change(kind)) }
    }

    /// An entirely new (`Insert`) or entirely dropped (`Delete`) block.
    fn whole(&self, block: &Block, kind: ChangeKind, out: &mut Vec<Block>) {
        let mark = Some(self.change(kind));
        match block {
            Block::Paragraph { content, .. } => {
                out.push(Block::Paragraph { content: self.tracked(kind, content.clone()), mark });
            }
            Block::Part { content, .. } => out.push(Block::Part { content: self.tracked(kind, content.clone()), mark }),
            Block::Heading { level, numbered, content, .. } => out.push(Block::Heading {
                level: *level,
                numbered: *numbered,
                content: self.tracked(kind, content.clone()),
                mark,
            }),
            Block::Quote { blocks, attribution } => {
                let mut inner = Vec::new();
                blocks.iter().for_each(|b| self.whole(b, kind, &mut inner));
                out.push(Block::Quote { blocks: inner, attribution: self.tracked(kind, attribution.clone()) });
            }
            Block::List { ordered, start, items } => out.push(Block::List {
                ordered: *ordered,
                start: *start,
                items: items.iter().map(|item| self.whole_item(item, kind)).collect(),
            }),
            Block::Table { header_rows, rows } => out.push(Block::Table {
                header_rows: *header_rows,
                rows: rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|cell| {
                                let mut inner = Vec::new();
                                cell.iter().for_each(|b| self.whole(b, kind, &mut inner));
                                inner
                            })
                            .collect()
                    })
                    .collect(),
            }),
            Block::Figure { image, alt, caption, .. } => {
                let picture = vec![Inline::Image { image: *image, alt: alt.clone() }];
                out.push(Block::Paragraph { content: self.tracked(kind, picture), mark: mark.clone() });
                if !caption.is_empty() {
                    out.push(Block::Paragraph { content: self.tracked(kind, caption.clone()), mark });
                }
            }
            Block::Typst { raw, shown } => {
                let mut visible = Vec::new();
                for (i, block) in shown.iter().enumerate() {
                    if i > 0 {
                        visible.push(Inline::LineBreak);
                    }
                    let mut text = String::new();
                    plain_block(block, &mut text);
                    visible.push(Inline::Text { text: text.trim_end().to_owned(), marks: Default::default() });
                }
                let island = vec![Inline::Typst { raw: raw.clone(), shown: visible }];
                out.push(Block::Paragraph { content: self.tracked(kind, island), mark });
            }
            // Not trackable: the page breaks of the proposal apply.
            Block::PageBreak => {
                if kind == ChangeKind::Insert {
                    out.push(Block::PageBreak);
                }
            }
        }
    }

    /// Word-by-word comparison within a paragraph.
    fn inlines(&self, current: &[Inline], proposed: &[Inline]) -> Vec<Inline> {
        #[derive(Clone, Copy, PartialEq)]
        enum Op {
            Keep,
            Delete,
            Insert,
        }
        let (a, b) = (tokens(current), tokens(proposed));
        let mut ops: Vec<(Op, &Inline)> = lcs(&a, &b)
            .into_iter()
            .map(|step| match step {
                Step::Both(i, _) => (Op::Keep, &a[i]),
                Step::Left(i) => (Op::Delete, &a[i]),
                Step::Right(j) => (Op::Insert, &b[j]),
            })
            .collect();
        // Whitespace between two changes belongs to the change: „alten Zaun“ → „neuen Steg“ is one place, not two.
        let mut at = 0;
        while at < ops.len() {
            if ops[at].0 != Op::Keep {
                at += 1;
                continue;
            }
            let end = at + ops[at..].iter().take_while(|(op, _)| *op == Op::Keep).count();
            let between_changes = at > 0 && end < ops.len();
            if between_changes && ops[at..end].iter().all(|(_, token)| is_space(token)) {
                let spaces: Vec<&Inline> = ops[at..end].iter().map(|(_, token)| *token).collect();
                let replacement: Vec<(Op, &Inline)> =
                    spaces.iter().map(|t| (Op::Delete, *t)).chain(spaces.iter().map(|t| (Op::Insert, *t))).collect();
                let added = replacement.len();
                ops.splice(at..end, replacement);
                at += added;
            } else {
                at = end;
            }
        }
        let mut out = Vec::new();
        let (mut removed, mut added): (Vec<Inline>, Vec<Inline>) = (Vec::new(), Vec::new());
        let flush = |removed: &mut Vec<Inline>, added: &mut Vec<Inline>, out: &mut Vec<Inline>| {
            out.extend(self.tracked(ChangeKind::Delete, merge(std::mem::take(removed))));
            out.extend(self.tracked(ChangeKind::Insert, merge(std::mem::take(added))));
        };
        let mut kept: Vec<Inline> = Vec::new();
        for (op, token) in ops {
            match op {
                Op::Keep => {
                    flush(&mut removed, &mut added, &mut out);
                    kept.push(token.clone());
                }
                Op::Delete | Op::Insert => {
                    out.extend(merge(std::mem::take(&mut kept)));
                    if op == Op::Delete { removed.push(token.clone()) } else { added.push(token.clone()) }
                }
            }
        }
        flush(&mut removed, &mut added, &mut out);
        out.extend(merge(std::mem::take(&mut kept)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcs_keeps_order_and_names_every_element_once() {
        let steps = lcs(&["a", "b", "c", "d"], &["a", "x", "c", "d", "e"]);
        assert_eq!(
            steps,
            vec![Step::Both(0, 0), Step::Left(1), Step::Right(1), Step::Both(2, 2), Step::Both(3, 3), Step::Right(4)],
            "gleiche Elemente bleiben, das ersetzte steht erst als entfernt, dann als neu, das angehängte am Schluss"
        );
        assert_eq!(lcs::<u8>(&[], &[]), Vec::new(), "zwei leere Folgen ergeben keine Schritte");
    }

    #[test]
    fn tokens_split_words_spaces_and_punctuation() {
        let pieces = tokens(&[Inline::Text { text: "Der Bach, kalt.".into(), marks: Default::default() }]);
        let texts: Vec<&str> = pieces
            .iter()
            .map(|p| match p {
                Inline::Text { text, .. } => text.as_str(),
                _ => "?",
            })
            .collect();
        assert_eq!(
            texts,
            vec!["Der", " ", "Bach", ",", " ", "kalt", "."],
            "Satzzeichen sind eigene Stücke, damit „kalt.“ und „kalt,“ das Wort gemeinsam haben"
        );
    }
}
