// SPDX-License-Identifier: AGPL-3.0-or-later
//! Parsing answers into findings and comparing findings (Round 28, task befunde).
//!
//! [`split_answer`] reads an answer line by line (contract R28 §1.3):
//! - **Structure** „Was trägt“ (praise), „Was fehlt“ (note), „Was stört“ (note) as a heading (also „### …“,
//!   „**…**“, „…:“) sets the kind of the following items; if text follows „Was trägt:“ directly, that is a finding.
//!   Below it every paragraph and every list item counts as one finding.
//! - **Markers** at the start of a line (after list markers, also bold): „Kritisch:“ (critical), „Vorschlag:“ and
//!   „Hinweis:“ (note), „Lob:“ (praise); „Kritisch:“ also counts behind a short part label as in the review
//!   ([`crate::assist::prompts::critical_finding`]). A marker alone on the line acts like a heading. The
//!   marker takes precedence over the structure.
//! - **Lists** („-“, „*“, „•“, „–“, „+“, „1.“, „1)“): every item a finding (without marker and structure a
//!   note); indented sub-items and continuation lines belong to the item above, an empty line closes it.
//! - **Quotation** „…“ (or "…", »…«, “…”): the first becomes `quote`. **Check notes** of the quotation and reference
//!   check (R17: „(nicht im Text belegt)“, „(Stelle steht im Text: …)“, „(Stelle ungültig: …)“) move from the text
//!   into `quote_check`. **References** „[B<id>]“ become `refs`.
//! - **Notes by Baruch** (answer cut off, context trimmed, „Komprimiert: …“, missing mandatory finding) are
//!   not findings and are skipped.
//! - An answer without list, marker and structure (small talk) yields no finding.
//! - Round 30b: **solution lines** (`statt: „…“`, `besser: „…“`, `Schritt: …`, also as a list item) belong to the
//!   finding above and stand not in its text but in `solutions`; without a finding above they are dropped. The
//!   closing block „Mein Vorschlag:“ of the lector yields no findings. [`finding_spans`] returns per finding its
//!   bytes (first to last line including solution lines) for the blocks in the chat.
//!
//! - Round 22 (befund-rueckfrage): **feedback** „Erledigt: Befund n (Stelle „…“)“ (also bold, as a list item,
//!   with „[Bn]“) and the sentence „Alle meine Befunde zu diesem Abschnitt scheinen erledigt.“ are not findings; the
//!   feedback is read by [`done_claims`] (`assist` makes status proposals „erledigt“ from it).
//!
//! [`similar`]: nur Text (text only, no embedding in Round 28): content words (lower case, no punctuation, from three
//! letters on without filler words, plus every number) as sets, Dice measure 2·|A∩B| / (|A|+|B|) from
//! [`SIMILARITY_THRESHOLD`] on. The threshold is set so that even one other content word in a short finding (up to
//! four content words) makes it a different one.

use std::collections::HashSet;

use super::FindingKind;
use crate::assist::prompts::{CRITICAL_MISSING_NOTE, NO_SOLUTION_NOTE, UNBACKED_NOTE, critical_finding};
use crate::assist::ref_claims::{IN_TEXT, INVALID};
use crate::assist::{CUT_NOTE, TRUNCATED_NOTE};

/// Maximum lengths like the CHECKs of migration 0046 (text, quotation, check note).
const MAX_BODY: usize = baruch_store::MAX_FINDING_CHARS;
const MAX_QUOTE: usize = baruch_store::MAX_QUOTE_CHARS;
const MAX_CHECK: usize = 200;

/// A finding as it was read from an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFinding {
    /// Position in the answer (from 0).
    pub position: u32,
    pub kind: FindingKind,
    pub body: String,
    pub quote: String,
    pub quote_check: String,
    /// References „[B<id>]“ in the text.
    pub refs: Vec<i64>,
    /// Round 30b: solutions below the finding (`statt:`/`besser:` or `Schritt:`, [`crate::assist::solutions`]).
    pub solutions: Vec<baruch_store::Solution>,
}

/// An item that is being collected.
struct Item {
    /// Kind from marker or structure (default note).
    kind: FindingKind,
    /// First line without list marker and marker, continuation lines trimmed after it.
    lines: Vec<String>,
    /// Indentation of the first line (sub-items are indented deeper).
    indent: usize,
    /// Round 30b: solution lines (`statt:`, `besser:`, `Schritt:`) below the finding; not part of the text.
    solution_lines: Vec<String>,
    /// Bytes of the item in the answer (first to last line including solution lines), for [`finding_spans`].
    range: std::ops::Range<usize>,
}

impl Item {
    fn new(kind: FindingKind, first: String, indent: usize, range: std::ops::Range<usize>) -> Item {
        Item { kind, lines: vec![first], indent, solution_lines: Vec::new(), range }
    }
}

/// Is the line a note by Baruch (not a finding)?
fn baruch_note(line: &str) -> bool {
    if done_line(line).is_some() || crate::assist::prompts::all_done_line(line) {
        // Round 22: feedback of the persona on its own findings is not a new finding.
        return true;
    }
    let t = line.trim();
    t == NO_SOLUTION_NOTE
        || t == crate::assist::lang::EN.no_solution_note
        || t == crate::assist::prompts::CRITICAL_MISSING_NOTE_EN
        || t == CUT_NOTE
        || t == TRUNCATED_NOTE
        || t == CRITICAL_MISSING_NOTE
        || t.starts_with("Komprimiert:")
        || t.starts_with("(Hinweis von Baruch:")
}

/// Does `s` start (case-insensitive, `*`/`_` in between skipped) with `word`? Then the rest after it without
/// leading `*`, `_` and whitespace.
fn strip_word<'a>(s: &'a str, word: &str) -> Option<&'a str> {
    let mut wanted = word.chars().flat_map(char::to_lowercase).peekable();
    let mut end = 0;
    for (at, c) in s.char_indices() {
        if wanted.peek().is_none() {
            break;
        }
        if matches!(c, '*' | '_') {
            end = at + c.len_utf8();
            continue;
        }
        let mut lower = c.to_lowercase();
        for l in lower.by_ref() {
            if wanted.next() != Some(l) {
                return None;
            }
        }
        end = at + c.len_utf8();
    }
    if wanted.peek().is_some() {
        return None;
    }
    Some(s[end..].trim_start_matches(|c: char| matches!(c, '*' | '_') || c.is_whitespace()))
}

/// Feedback „Erledigt: …“ (Round 22): after list markers, Markdown characters (`*`, `_`, `>`, `#`) and whitespace
/// the line starts with „Erledigt:“ (case-insensitive, also „**Erledigt:**“ or „**Erledigt**:“). Returns the
/// rest after it.
pub fn done_line(line: &str) -> Option<&str> {
    let content = list_item(line).unwrap_or(line.trim_start());
    let content = content.trim_start_matches(|c: char| matches!(c, '*' | '_' | '>' | '#') || c.is_whitespace());
    let rest = crate::assist::lang::ALL.iter().find_map(|m| strip_word(content, m.done))?;
    Some(rest.trim_end_matches(|c: char| matches!(c, '*' | '_') || c.is_whitespace()))
}

/// A feedback „Erledigt: Befund n (Stelle „…“)“ of a persona (Round 22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoneClaim {
    /// Number of the finding (`finding.id`, as in „[B<id>]“).
    pub id: i64,
    /// Position in the text (first quotation of the line, without quotation marks); empty if none is named.
    pub place: String,
}

/// Number behind „Befund“ (also „Befund Nr. 12“, „Befund #12“) or „B12“ at the start of a word.
fn claimed_number(rest: &str) -> Option<i64> {
    if let Some(&id) = refs_in(rest).first() {
        return Some(id);
    }
    let lower = rest.to_lowercase();
    let word = crate::assist::lang::MORE.iter().filter_map(|m| lower.find(m.finding).map(|at| (at, m.finding))).min();
    if let Some((at, word)) = word {
        let after = lower[at + word.len()..].trim_start();
        let after = after.strip_prefix("nr.").unwrap_or(after).trim_start();
        let after = after.strip_prefix('#').unwrap_or(after).trim_start();
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(id) = digits.parse() {
            return Some(id);
        }
    }
    let trimmed = rest.trim_start_matches(|c: char| matches!(c, '*' | '_' | '[' | '(') || c.is_whitespace());
    let digits: String = trimmed.strip_prefix(['B', 'b'])?.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// All feedback „Erledigt: Befund n …“ of an answer in order, per number only the first. Lines without a
/// recognisable number are dropped.
pub fn done_claims(text: &str) -> Vec<DoneClaim> {
    let mut out: Vec<DoneClaim> = Vec::new();
    for line in text.lines() {
        let Some(rest) = done_line(line) else { continue };
        let Some(id) = claimed_number(rest) else { continue };
        if out.iter().any(|c| c.id == id) {
            continue;
        }
        out.push(DoneClaim { id, place: clip(&first_quote(rest), 200) });
    }
    out
}

/// Markers at the start (after list markers) and their kind.
const MARKERS: [(&str, FindingKind); 8] = [
    ("kritisch:", FindingKind::Critical),
    ("vorschlag:", FindingKind::Hint),
    ("hinweis:", FindingKind::Hint),
    ("lob:", FindingKind::Praise),
    ("critical:", FindingKind::Critical),
    ("suggestion:", FindingKind::Hint),
    ("note:", FindingKind::Hint),
    ("praise:", FindingKind::Praise),
];

/// Marker at the start of `s`: kind and rest.
fn marker(s: &str) -> Option<(FindingKind, &str)> {
    MARKERS.iter().find_map(|(word, kind)| strip_word(s, word).map(|rest| (*kind, rest)))
}

/// Structure of the answer: heading and kind.
const OUTLINE: [(&str, FindingKind); 6] = [
    ("was trägt", FindingKind::Praise),
    ("was fehlt", FindingKind::Hint),
    ("was stört", FindingKind::Hint),
    ("what works", FindingKind::Praise),
    ("what is missing", FindingKind::Hint),
    ("what disturbs", FindingKind::Hint),
];

/// List marker at the start of a line: content after it.
fn list_item(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let mut chars = t.chars();
    let first = chars.next()?;
    if matches!(first, '-' | '*' | '•' | '–' | '+') {
        let rest = &t[first.len_utf8()..];
        return rest.starts_with(char::is_whitespace).then(|| rest.trim_start());
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits <= 3 {
        let rest = &t[digits..];
        if rest.starts_with(['.', ')']) && rest[1..].starts_with(char::is_whitespace) {
            return Some(rest[1..].trim_start());
        }
    }
    None
}

/// Head without Markdown characters („#“, „*“, „_“, „>“), number and whitespace.
fn heading_probe(line: &str) -> &str {
    let probe = list_item(line).unwrap_or(line);
    probe.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '#' | '*' | '_' | '>'))
}

/// Result for a line of the structure „Was trägt / Was fehlt / Was stört“.
enum Outline<'a> {
    /// Only the heading.
    Heading(FindingKind),
    /// Heading with text after it („Was trägt: …“).
    Inline(FindingKind, &'a str),
}

fn outline_line(line: &str) -> Option<Outline<'_>> {
    let probe = heading_probe(line);
    let (kind, rest) = OUTLINE.iter().find_map(|(word, kind)| strip_word(probe, word).map(|rest| (*kind, rest)))?;
    // Directly behind the heading: nothing, „:“, „–“ or „-“; otherwise it is a sentence („Was trägt den Text? …“).
    let rest = rest.trim_start_matches(['*', '_']);
    let rest = match rest.chars().next() {
        None => "",
        Some(':' | '–' | '—' | '-') => rest[rest.chars().next().map_or(0, char::len_utf8)..].trim(),
        Some('?' | '!' | '.') if rest[1..].trim().is_empty() => "",
        Some(_) => return None,
    };
    let rest = rest.trim_matches(|c: char| matches!(c, '*' | '_') || c.is_whitespace());
    Some(if rest.is_empty() { Outline::Heading(kind) } else { Outline::Inline(kind, rest) })
}

/// Another heading („## …“, all bold, short phrase with „:“ at the end): the structure ends.
fn other_heading(line: &str) -> bool {
    let t = line.trim();
    if t.starts_with('#') {
        return true;
    }
    let bold = t.len() > 4 && t.starts_with("**") && t.trim_end_matches(':').ends_with("**");
    bold || (t.ends_with(':') && t.chars().count() <= 60)
}

/// „Kritisch:“ behind a part label: label and rest without the marker.
fn strip_inner_critical(s: &str) -> Option<String> {
    let mut previous: Option<char> = None;
    for (at, c) in s.char_indices() {
        if at > 0
            && previous.is_some_and(|p| p.is_whitespace() || matches!(p, '*' | '_'))
            && let Some(rest) = ["kritisch:", "critical:"].iter().find_map(|word| strip_word(&s[at..], word))
        {
            let label = s[..at].trim_end_matches(|c: char| matches!(c, '*' | '_') || c.is_whitespace());
            return Some(format!("{label} {rest}"));
        }
        previous = Some(c);
    }
    None
}

/// Take check notes (R17) out of the text: text without notes and the notes (joined with spaces).
fn take_checks(text: &str) -> (String, String) {
    let mut checks: Vec<String> = Vec::new();
    let mut rest = text.to_owned();
    let en = &crate::assist::lang::EN;
    for note in [UNBACKED_NOTE, NO_SOLUTION_NOTE, en.unbacked_note, en.no_solution_note] {
        while let Some(at) = rest.find(note) {
            checks.push(note.to_owned());
            rest.replace_range(at..at + note.len(), "");
        }
    }
    while let Some(at) = [IN_TEXT, INVALID].iter().filter_map(|m| rest.find(m)).min() {
        let mut depth = 0usize;
        let mut end = rest.len();
        for (k, c) in rest[at..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = at + k + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        checks.push(rest[at..end].to_owned());
        rest.replace_range(at..end, "");
    }
    let cleaned: Vec<String> =
        rest.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect();
    (cleaned.join("\n"), checks.join(" "))
}

/// First quotation in „…“ (or "…", »…«, “…”), trimmed; without a quotation empty.
fn first_quote(text: &str) -> String {
    const PAIRS: [(char, char); 4] = [('„', '“'), ('"', '"'), ('»', '«'), ('“', '”')];
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if let Some(&(_, close)) = PAIRS.iter().find(|(open, _)| open == c)
            && let Some(len) = chars[i + 1..].iter().position(|&x| x == close)
        {
            let quote: String = chars[i + 1..i + 1 + len].iter().collect();
            let quote = quote.trim();
            if !quote.is_empty() {
                return quote.to_owned();
            }
        }
    }
    String::new()
}

/// References „[B<id>]“ in order, no duplicates.
pub fn refs_in(text: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("[B") {
        let after = &rest[at + 2..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty()
            && after[digits.len()..].starts_with(']')
            && let Ok(id) = digits.parse::<i64>()
            && !out.contains(&id)
        {
            out.push(id);
        }
        rest = after;
    }
    out
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Finished finding from a collected item; without text after removing the notes `None`.
fn finish(item: Item, position: u32) -> Option<ParsedFinding> {
    let joined = item.lines.join("\n");
    // Solutions: own lines below the finding and the short form „statt „…“ besser „…““ in the text itself.
    let mut solutions = crate::assist::solutions::parse_solutions(&joined);
    solutions.extend(crate::assist::solutions::parse_solutions(&item.solution_lines.join("\n")));
    let refs = refs_in(&joined);
    let (body, checks) = take_checks(&joined);
    let body = body.trim_matches(|c: char| matches!(c, '*' | '_') || c.is_whitespace()).to_owned();
    if body.is_empty() {
        return None;
    }
    Some(ParsedFinding {
        position,
        kind: item.kind,
        quote: clip(&first_quote(&body), MAX_QUOTE),
        body: clip(&body, MAX_BODY),
        quote_check: clip(&checks, MAX_CHECK),
        refs,
        solutions,
    })
}

/// Parse an answer into findings (order of the answer, positions from 0 without gaps).
pub fn split_answer(text: &str) -> Vec<ParsedFinding> {
    split_spans(text).into_iter().map(|(found, _)| found).collect()
}

/// Does the closing block „Mein Vorschlag:“ of the lector start here (Round 30b)? Its lines are solutions.
pub fn proposal_heading(line: &str) -> bool {
    let probe = heading_probe(line);
    ["mein vorschlag", "my proposal"].iter().find_map(|word| strip_word(probe, word)).is_some_and(|rest| {
        let rest = rest.trim_start_matches(['*', '_']);
        rest.is_empty() || rest.starts_with(':')
    })
}

/// Lines with bytes (without line end, also without „\r“).
fn lines_with_offsets(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    for piece in text.split_inclusive('\n') {
        let line = piece.strip_suffix('\n').unwrap_or(piece);
        let line = line.strip_suffix('\r').unwrap_or(line);
        out.push((at, line));
        at += piece.len();
    }
    out
}

type Spanned = (ParsedFinding, std::ops::Range<usize>);

/// Like [`split_answer`], plus per finding its bytes in the answer (first to last line including solution lines).
fn split_spans(text: &str) -> Vec<Spanned> {
    let mut out: Vec<Spanned> = Vec::new();
    let mut current: Option<Item> = None;
    let mut section: Option<FindingKind> = None;
    // Round 30b: the closing block „Mein Vorschlag:“ holds solutions of the lector, not findings.
    let mut in_proposal = false;
    let flush = |current: &mut Option<Item>, out: &mut Vec<Spanned>| {
        if let Some(item) = current.take() {
            let position = u32::try_from(out.len()).unwrap_or(u32::MAX);
            let range = item.range.clone();
            if let Some(found) = finish(item, position) {
                out.push((found, range));
            }
        }
    };
    // An empty line closes the item only if something other than a solution line follows: „Kritisch: …⏎⏎
    // - statt: …“ carries its solution (otherwise Baruch would retry and note „ohne Lösungsvorschlag“ while the
    // post shows the same solution as adoptable).
    let mut gap = false;
    for (at, line) in lines_with_offsets(text) {
        let span = at..at + line.len();
        if line.trim().is_empty() {
            gap = true;
            continue;
        }
        if baruch_note(line) {
            flush(&mut current, &mut out);
            gap = false;
            continue;
        }
        let indent = line.chars().take_while(|c| c.is_whitespace()).count();
        // Solution lines belong to the finding above, however indented (also as their own list item, also
        // after an empty line); without a finding above (e.g. in the block „Mein Vorschlag:“) they are no finding.
        if crate::assist::solutions::is_solution_line(line) {
            if let Some(item) = current.as_mut() {
                item.solution_lines.push(line.trim().to_owned());
                item.range.end = span.end;
            }
            continue;
        }
        if std::mem::take(&mut gap) {
            flush(&mut current, &mut out);
        }
        if proposal_heading(line) {
            flush(&mut current, &mut out);
            section = None;
            in_proposal = true;
            continue;
        }
        if let Some(outline) = outline_line(line) {
            flush(&mut current, &mut out);
            in_proposal = false;
            match outline {
                Outline::Heading(kind) => section = Some(kind),
                Outline::Inline(kind, rest) => {
                    section = Some(kind);
                    current = Some(Item::new(kind, rest.to_owned(), indent, span));
                }
            }
            continue;
        }
        let listed = list_item(line);
        let content = listed.unwrap_or(line.trim_start());
        let marked = marker(content);
        if in_proposal {
            // The block ends only at a marker or structure heading; list and text in it are not findings.
            if marked.is_none() && !critical_finding(line) {
                continue;
            }
            in_proposal = false;
        }
        // A sub-item (indented deeper) belongs to the item above.
        if listed.is_some()
            && let Some(item) = current.as_mut()
            && indent > item.indent
        {
            item.lines.push(line.trim().to_owned());
            item.range.end = span.end;
            continue;
        }
        if let Some((kind, rest)) = marked
            && listed.is_none()
            && rest.trim_matches(|c: char| matches!(c, '*' | '_') || c.is_whitespace()).is_empty()
        {
            // „Kritisch:“ alone on the line acts like a heading.
            flush(&mut current, &mut out);
            section = Some(kind);
            continue;
        }
        if let Some((kind, rest)) = marked {
            flush(&mut current, &mut out);
            current = Some(Item::new(kind, rest.to_owned(), indent, span));
            continue;
        }
        if critical_finding(line) {
            flush(&mut current, &mut out);
            let first = strip_inner_critical(content).unwrap_or_else(|| content.to_owned());
            current = Some(Item::new(FindingKind::Critical, first, indent, span));
            continue;
        }
        if let Some(content) = listed {
            flush(&mut current, &mut out);
            let kind = section.unwrap_or(FindingKind::Hint);
            current = Some(Item::new(kind, content.to_owned(), indent, span));
            continue;
        }
        if indent == 0 && other_heading(line) {
            flush(&mut current, &mut out);
            section = None;
            continue;
        }
        if let Some(item) = current.as_mut() {
            item.lines.push(line.trim().to_owned());
            item.range.end = span.end;
            continue;
        }
        if let Some(kind) = section {
            current = Some(Item::new(kind, line.trim().to_owned(), indent, span));
        }
        // otherwise: running text without list and marker – no finding
    }
    flush(&mut current, &mut out);
    out
}

/// The whole answer as one note (manual conversion by the human, contract §1.3): notes by Baruch
/// are dropped, quotation, check note and references as in [`split_answer`]; without text `None`.
pub fn whole_answer(text: &str) -> Option<ParsedFinding> {
    let lines: Vec<String> =
        text.lines().filter(|l| !baruch_note(l)).map(|l| l.trim_end().to_owned()).collect::<Vec<_>>();
    let range = 0..text.len();
    finish(Item { kind: FindingKind::Hint, lines, indent: 0, solution_lines: Vec::new(), range }, 0)
}

/// From this measure on two finding texts count as similar. 0.8: with three or four content words one
/// differing word already separates (2/3 ≈ 0.67 or 3/4 = 0.75), only from five content words on may one differ.
pub const SIMILARITY_THRESHOLD: f64 = 0.8;

/// Filler words (lower case) that do not make two findings equal: articles, prepositions with contractions,
/// pronouns, conjunctions, auxiliary verbs and particles.
const STOPWORDS: &[&str] = &[
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "eines", "und", "oder", "aber",
    "doch", "sondern", "denn", "weil", "dass", "wenn", "als", "wie", "auch", "noch", "nur", "schon", "sehr", "hier",
    "dort", "nicht", "kein", "keine", "keinen", "keinem", "keiner", "zur", "zum", "vom", "beim", "ins", "ans", "aufs",
    "mit", "von", "aus", "bei", "nach", "seit", "für", "gegen", "ohne", "durch", "über", "unter", "vor", "hinter",
    "neben", "zwischen", "auf", "sich", "sie", "ihr", "ihre", "ihren", "ihrem", "ihrer", "sein", "seine", "seinen",
    "seinem", "seiner", "ist", "sind", "war", "waren", "wird", "werden", "wurde", "hat", "haben", "hatte", "kann",
    "können", "soll", "sollte", "muss", "mehr", "etwas", "diese", "dieser", "dieses", "diesen", "diesem", "man", "was",
    "wer", "wo", "dann", "also", "etwa", "bitte",
];

/// Content words of a finding text in comparison form: lower case, letters and digits only; words from three
/// characters on without [`STOPWORDS`], numbers of any length („Satz 3“ and „Satz 5“ are different positions).
fn words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| w.chars().all(|c| c.is_numeric()) || (w.chars().count() >= 3 && !STOPWORDS.contains(&w.as_str())))
        .collect()
}

/// Similarity of two finding texts (Dice over the word sets, 0 to 1).
pub fn similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (words(a), words(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let common = a.intersection(&b).count();
    (2 * common) as f64 / (a.len() + b.len()) as f64
}

/// Are two finding texts alike (text only)? Equal text (trimmed) always; otherwise [`similarity`] from
/// [`SIMILARITY_THRESHOLD`] on.
pub fn similar(a: &str, b: &str) -> bool {
    a.trim() == b.trim() || similarity(a, b) >= SIMILARITY_THRESHOLD
}

/// A finding as a span in a persona answer (Round 30, interface R30-S1): position `position` as in [`split_answer`]
/// and bytes `range` in the text, for the blocks in the chat ([`crate::chat_render::Segment::Finding`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingSpan {
    pub position: u32,
    pub range: std::ops::Range<usize>,
}

/// Spans of the findings in an answer, in text order: exactly the findings of [`split_answer`] (same
/// `position`), each from the first to the last line including solution lines (without line end).
pub fn finding_spans(text: &str) -> Vec<FindingSpan> {
    split_spans(text).into_iter().map(|(found, range)| FindingSpan { position: found.position, range }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(found: &[ParsedFinding]) -> Vec<FindingKind> {
        found.iter().map(|f| f.kind).collect()
    }

    #[test]
    fn markers_lists_and_outline_become_findings_with_kind_and_position() {
        let answer = "\
Was trägt:
Der Einstieg mit dem Leuchtturm hält die Spannung.

Was fehlt:
- Ein Übergang zur Werft.
- Die Jahreszahl der Sturmflut.
  Sie steht nur im Vorwort.

Was stört:
Kritisch: „immer“ in Satz 3 verallgemeinert. (nicht im Text belegt)
Vorschlag: den Satz mit dem Netz kürzen.
";
        let found = split_answer(answer);
        assert_eq!(
            kinds(&found),
            vec![FindingKind::Praise, FindingKind::Hint, FindingKind::Hint, FindingKind::Critical, FindingKind::Hint],
            "Gliederung und Marken geben die Art: {found:#?}"
        );
        assert_eq!(found.iter().map(|f| f.position).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4], "Stellen ohne Lücke");
        assert_eq!(found[2].body, "Die Jahreszahl der Sturmflut.\nSie steht nur im Vorwort.", "Folgezeile gehört dazu");
        assert_eq!(found[3].body, "„immer“ in Satz 3 verallgemeinert.", "Marke und Prüfvermerk sind nicht im Text");
        assert_eq!(found[3].quote, "immer", "erstes Zitat");
        assert_eq!(found[3].quote_check, UNBACKED_NOTE, "Prüfvermerk R17");
    }

    #[test]
    fn small_talk_and_baruch_notes_are_no_findings() {
        assert!(split_answer("Danke, das freut mich. Schreib einfach weiter!").is_empty(), "Small Talk");
        let noted = format!("Hinweis: Ein Satz.\n\n{CUT_NOTE}\n\n{TRUNCATED_NOTE}\n\nKomprimiert: zwei Teile.");
        let found = split_answer(&noted);
        assert_eq!(found.len(), 1, "nur der Hinweis, keine Vermerke: {found:#?}");
    }

    #[test]
    fn refs_and_ref_claim_notes_are_read() {
        let found = split_answer("- Sammeln: [B12] und [B7] meinen dasselbe, [B12] doppelt. (Stelle ungültig: Ps 200)");
        assert_eq!(found[0].refs, vec![12, 7], "Verweise ohne Doppel");
        assert_eq!(found[0].quote_check, "(Stelle ungültig: Ps 200)", "Vermerk der Stellenprüfung");
        assert!(!found[0].body.contains("ungültig"), "Vermerk nicht im Text: {}", found[0].body);
    }

    #[test]
    fn critical_after_a_part_label_and_bold_markers_count() {
        let found = split_answer("- I. Beobachtung am Text: Kritisch: „Licht“ fehlt.\n**Lob:** schöner Bogen.");
        assert_eq!(kinds(&found), vec![FindingKind::Critical, FindingKind::Praise], "{found:#?}");
        assert_eq!(found[0].body, "I. Beobachtung am Text: „Licht“ fehlt.", "Teil-Angabe bleibt, Marke nicht");
        assert_eq!(found[1].body, "schöner Bogen.", "fette Marke entfernt");
    }

    #[test]
    fn done_lines_are_no_findings_but_claims_in_every_spelling() {
        let answer = "\
Erledigt: Befund 12 (Stelle „Die Möwen kreisen über dem Kai“)
- **Erledigt:** [B7] (Stelle „Nebelhorn“)
* **Erledigt**: Befund Nr. 9
Erledigt: B30 ist umgesetzt.
Erledigt: Befund 12 noch einmal
Erledigt: irgendwas ohne Nummer
Kritisch: „Laterne“ fehlt im zweiten Absatz.
- statt: „Licht“
- besser: „Laterne“
";
        let found = split_answer(answer);
        assert_eq!(found.len(), 1, "nur der neue kritische Befund, keine Rückmeldung als Befund: {found:#?}");
        assert_eq!(found[0].position, 0, "Stellen zählen nur Befunde (finding_spans bleibt gleich)");
        let claims = done_claims(answer);
        let ids: Vec<i64> = claims.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![12, 7, 9, 30], "Befund n, [Bn], fett, Aufzählung, Nr.; Doppel und ohne Nummer fallen weg");
        assert_eq!(claims[0].place, "Die Möwen kreisen über dem Kai", "Stelle aus dem ersten Zitat");
        assert_eq!(claims[2].place, "", "ohne Zitat keine Stelle");
        let all = "Alle meine Befunde zu diesem Abschnitt scheinen erledigt.";
        assert!(split_answer(all).is_empty(), "der Schlusssatz ist kein Befund");
        assert!(
            done_claims("Das ist erledigt: alles gut.").is_empty(),
            "„erledigt“ mitten im Satz ist keine Rückmeldung"
        );
    }

    #[test]
    fn similarity_uses_words_not_order() {
        assert!(similar("Der Übergang zur Werft fehlt.", "Es fehlt der Übergang zur Werft"), "gleiche Wörter");
        assert!(!similar("Der Übergang zur Werft fehlt.", "Die Möwen sind zu laut."), "andere Wörter");
        // Factually different findings with many shared words: another content word or another
        // position separates them (filler words „der“, „zur“, „zum“ do not count; numbers count).
        assert!(!similar("Der Übergang zur Werft fehlt.", "Der Übergang zum Hafen fehlt."), "anderes Ziel");
        assert!(!similar("„immer“ in Satz 3 verallgemeinert.", "„alle“ in Satz 5 verallgemeinert."), "andere Stelle");
        assert!(
            !similar("„immer“ in Satz 3 verallgemeinert.", "„immer“ in Satz 5 verallgemeinert."),
            "nur Zahl anders"
        );
        // A repetition with other filler words stays similar.
        assert!(
            similar("Der Übergang zur Werft fehlt.", "Ein Übergang zur Werft fehlt noch."),
            "nur Füllwörter anders"
        );
        assert!(similar("  gleich ", "gleich"), "gleicher Text");
    }
}
