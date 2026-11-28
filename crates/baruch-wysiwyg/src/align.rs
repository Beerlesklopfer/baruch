// SPDX-License-Identifier: AGPL-3.0-or-later
//! Alignment of two revisions by top-level blocks (contract Round 8, "inline"): where does a
//! proposal change the shown revision? The result are [`Hunk`]s – contiguous pieces of changed,
//! inserted or removed blocks –, so that the user interface can show every proposal **at its place** in
//! the reading flow ([`hunk_html_with`] shows the hunk in the typeset view).
//!
//! Two blocks are equal if their `raw` without trailing whitespace is byte-identical (as with
//! [`crate::diff_html`]): the empty lines after a block belong to its `raw`, and without this
//! rule every appending of a block would report the previously last block as "changed".

use std::ops::Range;

use crate::html::{Env, escape, render_block};
use crate::{Block, ImageRef, parse};
use baruch_core::WorkLanguage;

/// Kind of a hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkKind {
    /// Blocks of the base replaced by others (both ranges not empty).
    Changed,
    /// New blocks; `base` is empty, `base.start` = index of the block before which is inserted (at the
    /// end the block count of the base).
    Inserted,
    /// Blocks of the base are dropped; `proposal` is empty (`proposal.start` = position in the proposal).
    Removed,
}

/// A hunk: indices of the top-level blocks of `parse(base)` or `parse(proposal)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub kind: HunkKind,
    pub base: Range<usize>,
    pub proposal: Range<usize>,
}

/// Largest table for the longest common subsequence; above that everything counts as one hunk.
const MAX_CELLS: usize = 4_000_000;

/// Comparison key of a block: `raw` without trailing whitespace.
fn key(block: &Block) -> &str {
    block.raw.trim_end()
}

/// Pairs of equal blocks `(i, j)` according to the longest common subsequence, ascending.
fn matches(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    if a.len().saturating_mul(b.len()) > MAX_CELLS {
        return Vec::new();
    }
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
    let mut out = Vec::new();
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if lcs[(i + 1) * width + j] >= lcs[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

fn hunk(base: Range<usize>, proposal: Range<usize>) -> Option<Hunk> {
    let kind = match (base.is_empty(), proposal.is_empty()) {
        (true, true) => return None,
        (true, false) => HunkKind::Inserted,
        (false, true) => HunkKind::Removed,
        (false, false) => HunkKind::Changed,
    };
    Some(Hunk { kind, base, proposal })
}

/// Hunks in which `proposal` differs from `base`, in text order. Pure, never fails;
/// equal texts yield no hunks. Between two hunks (and before, after) there are
/// equally many blocks of both sides, equal in pairs (`raw` without trailing whitespace byte-identical);
/// two hunks may touch without a block in between (see [`split_changed`]).
pub fn align(base: &str, proposal: &str) -> Vec<Hunk> {
    let old = parse(base).blocks;
    let new = parse(proposal).blocks;
    let a: Vec<&str> = old.iter().map(key).collect();
    let b: Vec<&str> = new.iter().map(key).collect();
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    // After the last pair a sentinel at the end of both sides.
    for (mi, mj) in matches(&a, &b).into_iter().chain(std::iter::once((a.len(), b.len()))) {
        if let Some(found) = hunk(i..mi, j..mj) {
            out.extend(split_changed(found, &a, &b));
        }
        i = mi + 1;
        j = mj + 1;
    }
    out
}

/// Similarity of two blocks from which a paragraph counts as "reworded" (word Dice, 0–1).
const SIMILAR: f64 = 0.5;
/// Largest hunk (blocks left × right) that is still split by similarity.
const MAX_SPLIT_CELLS: usize = 10_000;

/// Words of a block (letters and digits, lower case).
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// Word Dice of two blocks: 2 · common words (with multiplicity) / all words.
fn similarity(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut rest: Vec<&String> = b.iter().collect();
    let mut common = 0usize;
    for word in a {
        if let Some(at) = rest.iter().position(|w| *w == word) {
            rest.swap_remove(at);
            common += 1;
        }
    }
    2.0 * common as f64 / (a.len() + b.len()) as f64
}

/// A "changed" hunk with unequal numbers of blocks on both sides (backlog Round 8, "LCS"): the longest
/// common subsequence knows only equal blocks; a reworded paragraph next to an inserted one thus became
/// a single hunk across both. Here the blocks are paired by similarity ([`similarity`] ≥
/// [`SIMILAR`]) (monotonic, largest sum of the similarities): pairs become "changed" (consecutive
/// pairs stay one hunk), what stands in between on one side only, "inserted" or
/// "removed", what stands unpaired on both sides stays "changed". With equally many blocks on both
/// sides (replaced in pairs) or without a similar pair the hunk stays as it is.
fn split_changed(found: Hunk, a: &[&str], b: &[&str]) -> Vec<Hunk> {
    let (m, n) = (found.base.len(), found.proposal.len());
    if found.kind != HunkKind::Changed || m == n || m.saturating_mul(n) > MAX_SPLIT_CELLS {
        return vec![found];
    }
    let left: Vec<Vec<String>> = a[found.base.clone()].iter().map(|t| words(t)).collect();
    let right: Vec<Vec<String>> = b[found.proposal.clone()].iter().map(|t| words(t)).collect();
    let sim: Vec<f64> = (0..m * n).map(|k| similarity(&left[k / n], &right[k % n])).collect();
    // best[i][j] = largest sum for left[i..] and right[j..].
    let width = n + 1;
    let mut best = vec![0.0f64; (m + 1) * width];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            let s = sim[i * n + j];
            let pair = if s >= SIMILAR { s + best[(i + 1) * width + j + 1] } else { 0.0 };
            best[i * width + j] = pair.max(best[(i + 1) * width + j]).max(best[i * width + j + 1]);
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut pairs = Vec::new();
    while i < m && j < n {
        let s = sim[i * n + j];
        if s >= SIMILAR && (best[i * width + j] - (s + best[(i + 1) * width + j + 1])).abs() < 1e-12 {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if best[(i + 1) * width + j] >= best[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    if pairs.is_empty() {
        return vec![found];
    }
    let (b0, p0) = (found.base.start, found.proposal.start);
    let mut out: Vec<Hunk> = Vec::new();
    let (mut i, mut j) = (0, 0);
    for (pi, pj) in pairs {
        out.extend(hunk(b0 + i..b0 + pi, p0 + j..p0 + pj));
        let paired = Hunk { kind: HunkKind::Changed, base: b0 + pi..b0 + pi + 1, proposal: p0 + pj..p0 + pj + 1 };
        match out.last_mut() {
            // Consecutive pairs (nothing in between, so the last hunk is the previous pair)
            // stay one hunk, as before several changed paragraphs in a row.
            Some(last) if i > 0 && i == pi && j == pj && last.kind == HunkKind::Changed => {
                last.base.end = paired.base.end;
                last.proposal.end = paired.proposal.end;
            }
            _ => out.push(paired),
        }
        i = pi + 1;
        j = pj + 1;
    }
    out.extend(hunk(b0 + i..b0 + m, p0 + j..p0 + n));
    out
}

/// Typeset view of a hunk: the affected blocks of the base as `div.ws-del`, those of the proposal as
/// `div.ws-ins` (like [`crate::diff_html_with`], without `data-b`, images in islands through `image`), all
/// in `div.ws-diff-doc` with `data-kind`. Invalid ranges are shortened to the block count.
pub fn hunk_html_with(base: &str, proposal: &str, hunk: &Hunk, image: &dyn Fn(&str) -> Option<ImageRef>) -> String {
    hunk_html_in(base, proposal, hunk, image, WorkLanguage::De)
}

/// Like [`hunk_html_with`] in the work language `lang`, cross references as with [`crate::render_html_in`]
/// (Round 39b).
pub fn hunk_html_in(
    base: &str,
    proposal: &str,
    hunk: &Hunk,
    image: &dyn Fn(&str) -> Option<ImageRef>,
    lang: WorkLanguage,
) -> String {
    let env = Env { images: image, lang, xref_title: &crate::html::german_xref_title };
    let old = parse(base).blocks;
    let new = parse(proposal).blocks;
    let clip = |range: &Range<usize>, len: usize| range.start.min(len)..range.end.min(len);
    let kind = match hunk.kind {
        HunkKind::Changed => "changed",
        HunkKind::Inserted => "inserted",
        HunkKind::Removed => "removed",
    };
    let mut out = format!("<div class=\"ws-diff-doc\" data-kind=\"{}\">", escape(kind));
    for block in &old[clip(&hunk.base, old.len())] {
        out.push_str("<div class=\"ws-del\">");
        render_block(&mut out, block, None, env);
        out.push_str("</div>");
    }
    for block in &new[clip(&hunk.proposal, new.len())] {
        out.push_str("<div class=\"ws-ins\">");
        render_block(&mut out, block, None, env);
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}
