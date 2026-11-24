// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ausrichtung zweier Fassungen nach obersten Blöcken (Vertrag Runde 8, „inline“): Wo ändert ein
//! Vorschlag die gezeigte Fassung? Ergebnis sind [`Hunk`]s – zusammenhängende Stücke geänderter,
//! eingefügter oder entfernter Blöcke –, damit die Oberfläche jeden Vorschlag **an seiner Stelle** im
//! Lesefluss einblenden kann ([`hunk_html_with`] zeigt das Stück im Satzbild).
//!
//! Gleich sind zwei Blöcke, wenn ihr `raw` ohne Leerraum am Ende byte-gleich ist (wie bei
//! [`crate::diff_html`]): Die Leerzeilen hinter einem Block gehören zu seinem `raw`, und ohne diese
//! Regel würde jedes Anhängen eines Blocks den bisher letzten Block als „geändert“ melden.

use std::ops::Range;

use crate::html::{escape, render_block};
use crate::{Block, ImageRef, parse};

/// Art eines Stücks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkKind {
    /// Blöcke der Basis durch andere ersetzt (beide Bereiche nicht leer).
    Changed,
    /// Neue Blöcke; `base` ist leer, `base.start` = Index des Blocks, vor dem eingefügt wird (am
    /// Ende die Blockzahl der Basis).
    Inserted,
    /// Blöcke der Basis entfallen; `proposal` ist leer (`proposal.start` = Stelle im Vorschlag).
    Removed,
}

/// Ein Stück: Indizes der obersten Blöcke von `parse(base)` bzw. `parse(proposal)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub kind: HunkKind,
    pub base: Range<usize>,
    pub proposal: Range<usize>,
}

/// Größte Tabelle für die längste gemeinsame Teilfolge; darüber gilt alles als ein Stück.
const MAX_CELLS: usize = 4_000_000;

/// Vergleichsschlüssel eines Blocks: `raw` ohne Leerraum am Ende.
fn key(block: &Block) -> &str {
    block.raw.trim_end()
}

/// Paare gleicher Blöcke `(i, j)` nach der längsten gemeinsamen Teilfolge, aufsteigend.
fn matches(a: &[&str], b: &[&str]) -> Vec<(usize, usize)> {
    if a.len().saturating_mul(b.len()) > MAX_CELLS {
        return Vec::new();
    }
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

/// Stücke, in denen sich `proposal` von `base` unterscheidet, in Textreihenfolge. Rein, scheitert
/// nie; gleiche Texte ergeben keine Stücke. Zwischen zwei Stücken (und davor, dahinter) stehen
/// gleich viele Blöcke beider Seiten, paarweise gleich (`raw` ohne Leerraum am Ende byte-gleich);
/// zwei Stücke dürfen ohne Block dazwischen aneinanderstoßen (siehe [`split_changed`]).
pub fn align(base: &str, proposal: &str) -> Vec<Hunk> {
    let old = parse(base).blocks;
    let new = parse(proposal).blocks;
    let a: Vec<&str> = old.iter().map(key).collect();
    let b: Vec<&str> = new.iter().map(key).collect();
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    // Nach dem letzten Paar ein Wächter am Ende beider Seiten.
    for (mi, mj) in matches(&a, &b).into_iter().chain(std::iter::once((a.len(), b.len()))) {
        if let Some(found) = hunk(i..mi, j..mj) {
            out.extend(split_changed(found, &a, &b));
        }
        i = mi + 1;
        j = mj + 1;
    }
    out
}

/// Ähnlichkeit zweier Blöcke, für die ein Absatz als „umformuliert“ gilt (Wort-Dice, 0–1).
const SIMILAR: f64 = 0.5;
/// Größtes Stück (Blöcke links × rechts), das noch nach Ähnlichkeit zerlegt wird.
const MAX_SPLIT_CELLS: usize = 10_000;

/// Wörter eines Blocks (Buchstaben und Ziffern, klein geschrieben).
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

/// Wort-Dice zweier Blöcke: 2 · gemeinsame Wörter (mit Vielfachheit) / alle Wörter.
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

/// Ein „geändert“-Stück mit ungleich vielen Blöcken beider Seiten (Backlog Runde 8, „LCS“): Die längste
/// gemeinsame Teilfolge kennt nur gleiche Blöcke; ein umformulierter Absatz neben einem eingefügten wurde
/// so ein einziges Stück über beide. Hier werden die Blöcke nach Ähnlichkeit ([`similarity`] ≥
/// [`SIMILAR`]) gepaart (monoton, größte Summe der Ähnlichkeiten): Paare werden „geändert“ (aufeinander
/// folgende Paare bleiben ein Stück), was dazwischen nur auf einer Seite steht, „eingefügt“ bzw.
/// „entfernt“, was auf beiden Seiten ungepaart steht, bleibt „geändert“. Bei gleich vielen Blöcken beider
/// Seiten (paarweise ersetzt) oder ohne ähnliches Paar bleibt das Stück, wie es ist.
fn split_changed(found: Hunk, a: &[&str], b: &[&str]) -> Vec<Hunk> {
    let (m, n) = (found.base.len(), found.proposal.len());
    if found.kind != HunkKind::Changed || m == n || m.saturating_mul(n) > MAX_SPLIT_CELLS {
        return vec![found];
    }
    let left: Vec<Vec<String>> = a[found.base.clone()].iter().map(|t| words(t)).collect();
    let right: Vec<Vec<String>> = b[found.proposal.clone()].iter().map(|t| words(t)).collect();
    let sim: Vec<f64> = (0..m * n).map(|k| similarity(&left[k / n], &right[k % n])).collect();
    // best[i][j] = größte Summe für left[i..] und right[j..].
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
            // Aufeinander folgende Paare (nichts dazwischen, also ist das letzte Stück das vorige Paar)
            // bleiben ein Stück, wie bisher mehrere geänderte Absätze hintereinander.
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

/// Satzbild eines Stücks: die betroffenen Blöcke der Basis als `div.ws-del`, die des Vorschlags als
/// `div.ws-ins` (wie [`crate::diff_html_with`], ohne `data-b`, Bilder in Inseln über `image`), alles
/// in `div.ws-diff-doc` mit `data-kind`. Ungültige Bereiche werden auf die Blockzahl gekürzt.
pub fn hunk_html_with(base: &str, proposal: &str, hunk: &Hunk, image: &dyn Fn(&str) -> Option<ImageRef>) -> String {
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
        render_block(&mut out, block, None, image);
        out.push_str("</div>");
    }
    for block in &new[clip(&hunk.proposal, new.len())] {
        out.push_str("<div class=\"ws-ins\">");
        render_block(&mut out, block, None, image);
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}
