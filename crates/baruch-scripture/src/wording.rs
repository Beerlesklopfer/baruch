// SPDX-License-Identifier: AGPL-3.0-or-later
//! Compare the wording of a quotation against the text of a Bible (pure, without Bible text): find quotations
//! in the source text ([`inline_quotes`], also nested and across paragraphs; block quotes are delivered by the
//! caller, in the server `crate::quotes::quote_spans`), assign them to a reference
//! ([`quote_for`]) and compare normalised ([`compare`]: punctuation, case, hyphens,
//! hyphenations, whitespace, typographic characters and Typst markup do not count).
//!
//! Round 19c: Unicode normal form NFC before splitting (decomposed umlauts from PDFs count like composed ones);
//! if a quotation deviates only in whitespace (hyphenation leftovers in the module: „die ses“, „Men schen“), it
//! matches with [`Comparison::spacing`]; [`fragment_survey`] estimates whether a module has conspicuously many
//! word fragments; [`quote_for`] allows short introductory words between reference and quotation („wie Lk 15,6
//! erzählt: „…““).

use std::collections::HashSet;

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use crate::Found;

/// A quotation in the searched text: range of the content (without quotation marks) in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct QuoteSpan {
    pub start: usize,
    pub end: usize,
    /// Range including the quotation marks or the whole block quote.
    pub outer: (usize, usize),
    /// Block quote (`#quote(block: true …)[…]`) instead of quotation marks.
    pub block: bool,
}

/// Pairs of quotation marks: German „…“, „…”, English “…”, French »…« and «…», straight "…".
const PAIRS: [(char, char); 6] = [('„', '“'), ('„', '”'), ('“', '”'), ('»', '«'), ('«', '»'), ('"', '"')];

/// Maximum length of a quotation in quotation marks (characters); longer ones are probably a forgotten mark.
const MAX_QUOTE_CHARS: usize = 3000;

/// Quotations in quotation marks. A quotation ends at the matching closing mark. Typst `#quote[…]` in running
/// text counts as well.
///
/// Since Round 17b:
/// - **Nested:** inner quotation marks of another kind („… ‚…‘ …“, „… »…« …“) belong to the outer
///   quotation; an inner pair is skipped, its closing mark does not end the outer quotation.
/// - **Across paragraphs:** an empty line no longer ends the search if the quotation continues in the next
///   paragraph – either with the repeated quotation mark at the start of the paragraph (German typesetting) or
///   up to the closing mark, as long as no further opening mark of the same kind comes (then the first was never
///   closed: no quotation).
pub fn inline_quotes(text: &str) -> Vec<QuoteSpan> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let c = rest.chars().next().expect("innerhalb des Texts");
        if let Some(inner) = rest.strip_prefix("#quote[")
            && let Some(len) = bracket_end(inner)
        {
            let open = i + "#quote[".len();
            out.push(QuoteSpan { start: open, end: open + len, outer: (i, open + len + 1), block: false });
            i = open + len + 1;
            continue;
        }
        if let Some((end, width)) = quote_end(text, i, c) {
            let start = i + c.len_utf8();
            out.push(QuoteSpan { start, end, outer: (i, end + width), block: false });
            i = end + width;
            continue;
        }
        i += c.len_utf8();
    }
    out
}

/// Inner pairs that a quotation of another kind may contain (single German/English/French
/// quotation marks and the other pairs from [`PAIRS`]).
const INNER_PAIRS: [(char, char); 8] =
    [('‚', '‘'), ('‘', '’'), ('›', '‹'), ('‹', '›'), ('»', '«'), ('«', '»'), ('„', '“'), ('“', '”')];

/// End (byte, width of the closing mark) of the quotation that starts at `i` with `open`; `None` if `open` is
/// not an opening mark or no matching closing mark comes. A closing mark hangs on the word before it;
/// if it comes after whitespace and itself opens a pair („er sagte “Ja”“), it is an inner quotation.
fn quote_end(text: &str, i: usize, open: char) -> Option<(usize, usize)> {
    let closers: Vec<char> = PAIRS.iter().filter(|(o, _)| *o == open).map(|(_, close)| *close).collect();
    if closers.is_empty() {
        return None;
    }
    let start = i + open.len_utf8();
    let mut crossed = false;
    let mut chars = text[start..].char_indices().peekable();
    let mut n = 0usize;
    while let Some((k, d)) = chars.next() {
        n += 1;
        if n > MAX_QUOTE_CHARS {
            return None;
        }
        let at = start + k;
        let after_space = text[..at].chars().next_back().is_none_or(char::is_whitespace);
        let inner = INNER_PAIRS.iter().find(|(o, _)| *o == d && *o != open).map(|&(_, close)| close);
        if closers.contains(&d) && !(after_space && inner.is_some()) {
            return Some((at, d.len_utf8()));
        }
        if text[at..].starts_with("\n\n") {
            crossed = true;
            continue;
        }
        if d == open {
            // Repeated mark at the start of the paragraph: the quotation continues; otherwise the first was never
            // closed.
            let paragraph_start = text[..at].trim_end_matches([' ', '\t']).ends_with("\n\n");
            if crossed && paragraph_start {
                continue;
            }
            return None;
        }
        // Skip an inner pair of another kind („… ‚…‘ …“), only within the paragraph.
        if let Some(inner_close) = inner {
            let rest = &text[at + d.len_utf8()..];
            let limit = rest.find("\n\n").unwrap_or(rest.len());
            if let Some(len) = rest[..limit].find(inner_close) {
                let skip_to = at + d.len_utf8() + len + inner_close.len_utf8();
                while chars.peek().is_some_and(|&(k2, _)| start + k2 < skip_to) {
                    chars.next();
                    n += 1;
                }
            }
        }
    }
    None
}

/// Length up to the closing `]` (exclusive) with balanced brackets; `\]` does not count.
pub fn bracket_end(inner: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (k, c) in inner.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '[' => depth += 1,
            ']' if depth == 0 => return Some(k),
            ']' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Maximum distance between quotation and reference (bytes), without other references of the same list.
const MAX_GAP: usize = 24;

/// Is there only whitespace, punctuation or one of the other references between `a` and `b`?
fn only_glue(text: &str, a: usize, b: usize, refs: &[Found]) -> bool {
    if a > b {
        return false;
    }
    let mut rest = String::new();
    let mut i = a;
    while i < b {
        if let Some(r) = refs.iter().find(|r| r.span.0 <= i && i < r.span.1) {
            i = r.span.1.min(b).max(i + 1);
            continue;
        }
        let c = text[i..].chars().next().expect("innerhalb des Texts");
        rest.push(c);
        i += c.len_utf8();
    }
    let rest = rest.replace("vgl.", "").replace("Vgl.", "");
    rest.len() <= MAX_GAP
        && rest.chars().all(|c| {
            c.is_whitespace()
                || matches!(c, '(' | ')' | '[' | ']' | ',' | ';' | ':' | '.' | '–' | '—' | '-' | '/' | '#' | '~')
        })
}

/// Short introductory words between a reference and the quotation after it (Round 19c): „wie Lk 15,6 erzählt:
/// „…““, „In Joh 3 heißt es: „…““. Only lower-case words from this list, at most [`MAX_INTRO_WORDS`], and
/// last a colon.
pub const INTRO_WORDS: [&str; 36] = [
    "erzählt",
    "erzählte",
    "berichtet",
    "berichtete",
    "heißt",
    "hieß",
    "es",
    "dort",
    "da",
    "sagt",
    "sagte",
    "spricht",
    "sprach",
    "schreibt",
    "schrieb",
    "steht",
    "stand",
    "lesen",
    "liest",
    "las",
    "wir",
    "man",
    "so",
    "ruft",
    "rief",
    "betet",
    "betete",
    "singt",
    "fragt",
    "antwortet",
    "lehrt",
    "mahnt",
    "verheißt",
    "bezeugt",
    "klagt",
    "dazu",
];
/// At most this many introductory words.
pub const MAX_INTRO_WORDS: usize = 3;
/// Maximum distance with introductory words (bytes, without other references).
const MAX_INTRO_GAP: usize = 48;

/// Between `a` (end of the reference) and `b` (start of the quotation): whitespace/punctuation, then 1 to
/// [`MAX_INTRO_WORDS`] words from [`INTRO_WORDS`], a colon and after that only whitespace.
fn intro_glue(text: &str, a: usize, b: usize, refs: &[Found]) -> bool {
    if a > b {
        return false;
    }
    let mut gap = String::new();
    let mut i = a;
    while i < b {
        if let Some(r) = refs.iter().find(|r| r.span.0 <= i && i < r.span.1) {
            i = r.span.1.min(b).max(i + 1);
            continue;
        }
        let c = text[i..].chars().next().expect("innerhalb des Texts");
        gap.push(c);
        i += c.len_utf8();
    }
    if gap.len() > MAX_INTRO_GAP {
        return false;
    }
    let Some(colon) = gap.rfind(':') else { return false };
    if !gap[colon + 1..].chars().all(char::is_whitespace) {
        return false;
    }
    let head = gap[..colon].trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | ',' | '–' | '—'));
    let words: Vec<&str> = head.split_whitespace().collect();
    !words.is_empty() && words.len() <= MAX_INTRO_WORDS && words.iter().all(|w| INTRO_WORDS.contains(w))
}

/// The quotation for the reference `refs[index]`: first one that ends directly before it („…“ (Joh 3,16)),
/// otherwise one that starts directly after it (Joh 3,16: „…“). In between only whitespace, brackets,
/// punctuation and other references of the same list may stand, before a quotation after it since Round 19c
/// also short introductory words with colon ([`INTRO_WORDS`]). A block quote also counts if the reference is
/// inside it (attribution).
pub fn quote_for(text: &str, quotes: &[QuoteSpan], refs: &[Found], index: usize) -> Option<QuoteSpan> {
    let (a, b) = refs.get(index)?.span;
    if let Some(q) =
        quotes.iter().find(|q| q.block && q.outer.0 <= a && b <= q.outer.1 && !(q.start <= a && b <= q.end))
    {
        return Some(*q);
    }
    let before =
        quotes.iter().filter(|q| q.outer.1 <= a && only_glue(text, q.outer.1, a, refs)).max_by_key(|q| q.outer.1);
    let after = quotes
        .iter()
        .filter(|q| q.outer.0 >= b && (only_glue(text, b, q.outer.0, refs) || intro_glue(text, b, q.outer.0, refs)))
        .min_by_key(|q| q.outer.0);
    // Integrator Round 17: if there is an empty line between the quotation before and the reference, but not in
    // the quotation after, the reference belongs to the quotation after („„…“ (Joh 3,16)⏎⏎Psalm 139, Vers 23:
    // „…““) – otherwise it would get the quotation of the previous paragraph.
    if let (Some(q), Some(next)) = (before, after)
        && paragraph_break(text, q.outer.1, a)
        && !paragraph_break(text, b, next.outer.0)
    {
        return Some(*next);
    }
    if let Some(q) = before {
        return Some(*q);
    }
    // Reference at the end of a block quote („… geliebt. (Joh 3,16)“): quotation up to the reference.
    if let Some(q) = quotes.iter().find(|q| q.block && q.start <= a && b <= q.end && only_glue(text, b, q.end, refs)) {
        let cut = text[q.start..a].trim_end_matches(|c: char| c.is_whitespace() || c == '(').len();
        return Some(QuoteSpan { start: q.start, end: q.start + cut, outer: q.outer, block: true });
    }
    after.copied()
}

/// Is there an empty line between `a` and `b` (two line breaks, only whitespace in between)?
fn paragraph_break(text: &str, a: usize, b: usize) -> bool {
    let gap = text.get(a..b).unwrap_or("");
    let mut newline = false;
    for c in gap.chars() {
        if c == '\n' {
            if newline {
                return true;
            }
            newline = true;
        } else if !c.is_whitespace() {
            newline = false;
        }
    }
    false
}

// ---- Normalising ------------------------------------------------------------------------------------

/// A word: as it stands there (without surrounding punctuation) and its comparison key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub shown: String,
    pub key: String,
}

/// Remove Typst markup: `#footnote[…]` entirely, `#name[`/`#name(…)[` and brackets, `_`, `*`,
/// escapes `\x` (`\` before a character), `~` as a space, comments `// …` up to the end of the line.
pub fn strip_markup(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let c = rest.chars().next().expect("innerhalb des Texts");
        if let Some(inner) = rest.strip_prefix("#footnote[")
            && let Some(len) = bracket_end(inner)
        {
            i += "#footnote[".len() + len + 1;
            continue;
        }
        if rest.starts_with("//") {
            i += rest.find('\n').unwrap_or(rest.len());
            continue;
        }
        if c == '#' {
            let name: usize =
                rest[1..].chars().take_while(|c| c.is_alphanumeric() || *c == '.').map(char::len_utf8).sum();
            let mut j = 1 + name;
            if rest[j..].starts_with('(') {
                let depth_end = rest[j..].char_indices().scan(0i32, |d, (k, ch)| {
                    match ch {
                        '(' => *d += 1,
                        ')' => *d -= 1,
                        _ => {}
                    }
                    Some((k, *d))
                });
                if let Some((k, _)) = depth_end.into_iter().find(|(_, d)| *d == 0) {
                    j += k + 1;
                }
            }
            i += j;
            out.push(' ');
            continue;
        }
        match c {
            '\\' => {
                let next = rest[1..].chars().next();
                if let Some(n) = next {
                    out.push(n);
                    i += 1 + n.len_utf8();
                } else {
                    i += 1;
                }
                continue;
            }
            '[' | ']' | '_' | '*' | '`' => out.push(' '),
            '~' => out.push(' '),
            _ => out.push(c),
        }
        i += c.len_utf8();
    }
    out
}

/// Characters that are dropped within a word (hyphens, apostrophes, soft hyphen, zero width).
fn joins(c: char) -> bool {
    matches!(
        c,
        '-' | '\u{2010}'
            | '\u{2011}'
            | '\u{00ad}'
            | '\''
            | '’'
            | 'ʼ'
            | '\u{200b}'
            | '\u{200c}'
            | '\u{200d}'
            | '\u{2060}'
    )
}

fn key_of(word: &str) -> String {
    let mut key = String::new();
    for c in word.chars() {
        if joins(c) {
            continue;
        }
        let c = match c {
            'ſ' => 's',
            _ => c,
        };
        if c.is_alphanumeric() {
            key.extend(c.to_lowercase());
        }
    }
    key
}

/// Words of a text for the comparison (markup removed). A word that ends with a hyphen
/// („Hei- land“) is joined with the following lower-case word (hyphenation), except before
/// „und“/„oder“ (suspended hyphen: „Vor- und Nachteile“).
pub fn words(text: &str) -> Vec<Word> {
    plain_words(&strip_markup(text))
}

/// Like [`words`], but for plain text without Typst markup (Bible text from a module): `#`, `_`, `*`, `\`
/// are ordinary characters there (Round 17b: "#1" in the verse text is not Typst code).
pub fn plain_words(plain: &str) -> Vec<Word> {
    // Round 19c: NFC first, otherwise a decomposed „ü“ (u + U+0308) would fall apart into two words at the
    // combining character.
    let plain: String = plain.nfc().collect();
    let mut raw: Vec<String> = Vec::new();
    for token in plain.split(|c: char| c.is_whitespace() || (!c.is_alphanumeric() && !joins(c))) {
        if token.is_empty() {
            continue;
        }
        raw.push(token.to_owned());
    }
    let mut out: Vec<Word> = Vec::new();
    let mut pending: Option<String> = None;
    for token in raw {
        let token = match pending.take() {
            Some(head) => {
                let joinable =
                    token.chars().next().is_some_and(char::is_lowercase) && !matches!(token.as_str(), "und" | "oder");
                if joinable {
                    format!("{}{token}", head.trim_end_matches(joins))
                } else {
                    push_word(&mut out, &head);
                    token
                }
            }
            None => token,
        };
        if token.ends_with('-') && token.chars().any(char::is_alphanumeric) {
            pending = Some(token);
            continue;
        }
        push_word(&mut out, &token);
    }
    if let Some(head) = pending {
        push_word(&mut out, &head);
    }
    out
}

fn push_word(out: &mut Vec<Word>, token: &str) {
    let shown = token.trim_matches(|c: char| !c.is_alphanumeric()).to_owned();
    let key = key_of(token);
    if !key.is_empty() {
        out.push(Word { shown, key });
    }
}

// ---- Comparing ---------------------------------------------------------------------------------------

/// A piece of the word diff: `same` (in both), `bible` (only in the Bible, missing in the quotation), `quote`
/// (only in the quotation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffPart {
    pub op: &'static str,
    pub text: String,
}

/// Result of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    /// All words of the quotation are in this sequence without gaps in the Bible text (per part with omissions „…“).
    pub matches: bool,
    /// The quotation is shorter than the Bible text (partial quotation).
    pub partial: bool,
    /// Word diff between the quotation and the matching excerpt of the Bible text (empty if `matches`).
    pub diff: Vec<DiffPart>,
    /// Round 19c: the quotation matches only if the whitespace is left out (hyphenation leftovers in the module
    /// such as „die ses“ or a word written together); then `matches` is true.
    pub spacing: bool,
}

/// Parts of a quotation at omissions („…“, "...", „[…]“, „(…)“).
fn pieces(quote: &str) -> Vec<String> {
    let marked = quote.replace("[…]", "\u{1}").replace("(…)", "\u{1}").replace("[...]", "\u{1}");
    let marked = marked.replace("...", "\u{1}").replace('…', "\u{1}");
    marked.split('\u{1}').map(str::to_owned).collect()
}

fn find_run(hay: &[Word], needle: &[Word], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from);
    }
    (from..=hay.len().checked_sub(needle.len())?)
        .find(|&i| hay[i..i + needle.len()].iter().zip(needle).all(|(a, b)| a.key == b.key))
}

/// Compares a quotation (Typst source text) with the Bible text (plain text of the module, without markup).
pub fn compare(quote: &str, bible: &str) -> Comparison {
    let b = plain_words(bible);
    let parts: Vec<Vec<Word>> = pieces(quote).iter().map(|p| words(p)).filter(|w| !w.is_empty()).collect();
    let q: Vec<Word> = parts.iter().flatten().cloned().collect();
    let mut from = 0;
    let mut ok = !q.is_empty();
    for part in &parts {
        match find_run(&b, part, from) {
            Some(i) => from = i + part.len(),
            None => {
                ok = false;
                break;
            }
        }
    }
    if ok {
        return Comparison { matches: true, partial: q.len() < b.len(), diff: Vec::new(), spacing: false };
    }
    if !q.is_empty()
        && let Some(partial) = matches_without_spacing(&parts, &b)
    {
        return Comparison { matches: true, partial, diff: Vec::new(), spacing: true };
    }
    Comparison { matches: false, partial: false, diff: word_diff(&q, &b), spacing: false }
}

/// Round 19c: comparison without whitespace. The keys of the Bible words are concatenated; every part of the
/// quotation must occur in it (in sequence), from a word boundary of the module to a word boundary – „dieses“
/// fits „die ses“, but „ist“ not in the middle of „Christus“. Round 23a: only additional word boundaries of the
/// **module** (fragments) are tolerated; every word boundary of the quotation must be one in the module too –
/// „die Bein“ does not fit „Diebe in“. `Some(partial quotation?)` on success.
fn matches_without_spacing(parts: &[Vec<Word>], bible: &[Word]) -> Option<bool> {
    let mut joined = String::new();
    let mut bounds = vec![0usize];
    for w in bible {
        joined.push_str(&w.key);
        bounds.push(joined.len());
    }
    let mut from = 0usize;
    let mut quoted = 0usize;
    for part in parts {
        let needle: String = part.iter().map(|w| w.key.as_str()).collect();
        if needle.is_empty() {
            continue;
        }
        quoted += needle.len();
        let mut search = from;
        let mut end = None;
        while let Some(pos) = joined.get(search..).and_then(|rest| rest.find(&needle)).map(|p| p + search) {
            let stop = pos + needle.len();
            // Round 23a (review R19): every word boundary **in the quotation** must be a word boundary of the module
            // too. The module may have additional boundaries (word fragments such as „die ses“), the quotation none
            // of its own – otherwise „die Bein“ would fit „Diebe in“.
            let mut inner = pos;
            let quote_bounds_fit = part.iter().all(|w| {
                inner += w.key.len();
                bounds.binary_search(&inner).is_ok()
            });
            if bounds.binary_search(&pos).is_ok() && bounds.binary_search(&stop).is_ok() && quote_bounds_fit {
                end = Some(stop);
                break;
            }
            search = pos + needle.chars().next().map_or(1, char::len_utf8);
        }
        from = end?;
    }
    Some(quoted < joined.len())
}

/// Word diff through the longest common subsequence; the Bible text is restricted to the range between the
/// first and last common word (a partial quotation does not show the whole verse as missing).
fn word_diff(q: &[Word], b: &[Word]) -> Vec<DiffPart> {
    let (n, m) = (q.len(), b.len());
    // For very long texts only roughly: everything as different.
    if n.saturating_mul(m) > 4_000_000 {
        return vec![DiffPart { op: "quote", text: join(q) }];
    }
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if q[i].key == b[j].key { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut ops: Vec<(&'static str, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && q[i].key == b[j].key {
            ops.push(("same", &q[i].shown));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            ops.push(("bible", &b[j].shown));
            j += 1;
        } else {
            ops.push(("quote", &q[i].shown));
            i += 1;
        }
    }
    // Leave out Bible words before the first and after the last common word.
    let first = ops.iter().position(|(op, _)| *op != "bible").unwrap_or(ops.len());
    let last = ops.iter().rposition(|(op, _)| *op != "bible").map_or(0, |p| p + 1);
    let mut out: Vec<DiffPart> = Vec::new();
    for (op, word) in ops.into_iter().take(last).skip(first) {
        match out.last_mut() {
            Some(part) if part.op == op => {
                part.text.push(' ');
                part.text.push_str(word);
            }
            _ => out.push(DiffPart { op, text: word.to_owned() }),
        }
    }
    out
}

fn join(words: &[Word]) -> String {
    words.iter().map(|w| w.shown.as_str()).collect::<Vec<_>>().join(" ")
}

/// Diff as one line: `[−Bibel]` is missing in the quotation, `[+Zitat]` is only in the quotation.
pub fn diff_line(diff: &[DiffPart]) -> String {
    diff.iter()
        .map(|p| match p.op {
            "bible" => format!("[−{}]", p.text),
            "quote" => format!("[+{}]", p.text),
            _ => p.text.clone(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---- Word fragments in the module (Round 19c) ---------------------------------------------------------

/// Short, frequent words that never count as a fragment as the second piece („da mit“, „hin aus“, „ich bin“).
const NOT_FRAGMENTS: [&str; 62] = [
    "und", "oder", "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "eines", "er",
    "es", "sie", "ihn", "ihm", "ihr", "ihre", "wir", "uns", "euch", "ich", "mich", "mir", "du", "dich", "dir", "zu",
    "an", "auf", "aus", "in", "im", "mit", "von", "vom", "vor", "bei", "nach", "um", "ist", "sind", "war", "hat",
    "nicht", "so", "da", "wie", "als", "wo", "was", "wer", "ob", "noch", "auch", "nur", "sich", "hin", "her",
];
/// Longest second piece that counts as a fragment (characters).
pub const MAX_FRAGMENT_CHARS: usize = 5;
/// A module needs at least this many suspected cases for the hint.
pub const MIN_FRAGMENTS: usize = 3;
/// Share of the suspected cases in all word pairs from which a module is conspicuous: 1 in 200 (0.5 %).
pub const FRAGMENT_SHARE: usize = 200;

/// Result of [`fragment_survey`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FragmentSurvey {
    /// Neighbouring word pairs in the text that was read (within a verse).
    pub pairs: usize,
    /// Suspected cases: pairs that look like a split word.
    pub suspects: usize,
    /// At most three examples („die ses“).
    pub examples: Vec<String>,
}

impl FragmentSurvey {
    /// Conspicuously many word fragments: at least [`MIN_FRAGMENTS`] cases and at least 1 in
    /// [`FRAGMENT_SHARE`] word pairs.
    pub fn fragmented(&self) -> bool {
        self.suspects >= MIN_FRAGMENTS && self.suspects * FRAGMENT_SHARE >= self.pairs
    }
}

/// Core of a whitespace piece: letters without punctuation before/after; `None` if something else is inside.
fn letters(token: &str) -> Option<&str> {
    let core = token.trim_matches(|c: char| !c.is_alphanumeric());
    (!core.is_empty() && core.chars().all(char::is_alphabetic)).then_some(core)
}

/// Estimates hyphenation leftovers in Bible text (plain text per verse, NFC). A suspected case is a pair of
/// neighbouring pieces „A b“ in the same verse where A ends without punctuation, b is lower case and at most
/// [`MAX_FRAGMENT_CHARS`] characters long and is not a frequent short word ([`NOT_FRAGMENTS`]) – and „Ab“ occurs
/// as a whole word elsewhere in the text that was read („die ses“ next to „dieses“, „Men schen“ next to
/// „Menschen“). An estimate, not hyphenation: [`FragmentSurvey::fragmented`] decides about the hint.
pub fn fragment_survey<'a>(verses: impl IntoIterator<Item = &'a str>) -> FragmentSurvey {
    let verses: Vec<String> = verses.into_iter().map(|v| v.nfc().collect()).collect();
    let mut known: HashSet<String> = HashSet::new();
    for verse in &verses {
        for token in verse.split_whitespace() {
            if let Some(core) = letters(token) {
                known.insert(core.to_lowercase());
            }
        }
    }
    let mut survey = FragmentSurvey::default();
    for verse in &verses {
        let tokens: Vec<&str> = verse.split_whitespace().collect();
        for pair in tokens.windows(2) {
            survey.pairs += 1;
            let (first, second) = (pair[0], pair[1]);
            // A ends with a letter (no punctuation that indicates the end of a word), b starts in lower case.
            if !first.chars().next_back().is_some_and(char::is_alphabetic)
                || !second.chars().next().is_some_and(char::is_lowercase)
            {
                continue;
            }
            let (Some(a), Some(b)) = (letters(first), letters(second)) else { continue };
            if b.chars().count() > MAX_FRAGMENT_CHARS || NOT_FRAGMENTS.contains(&b) {
                continue;
            }
            let joined = format!("{a}{b}").to_lowercase();
            if known.contains(&joined) {
                survey.suspects += 1;
                if survey.examples.len() < 3 {
                    survey.examples.push(format!("{a} {b}"));
                }
            }
        }
    }
    survey
}

// ---- Separated digits in answers --------------------------------------------------------------------

/// Puts references back together whose digits are separated by whitespace in an answer
/// („Mk 1 0,42–45“ instead of „Mk 10,42–45“): only if the result is exactly one of the references in `labels`
/// is it replaced; otherwise the text stays as it is.
pub fn rejoin_split_refs(text: &str, labels: &[String]) -> String {
    let mut out = text.to_owned();
    for label in labels {
        let Some(space) = label.find(' ') else { continue };
        let head = &label[..=space];
        let mut from = 0;
        while let Some(pos) = out[from..].find(head).map(|p| p + from) {
            let start = pos + head.len();
            if let Some(end) = match_split(&out, start, &label[head.len()..]) {
                out.replace_range(start..end, &label[head.len()..]);
            }
            from = start;
        }
    }
    out
}

/// Read the text `tail` from `start`, skipping whitespace between two digits; `Some(end)` only
/// if at least one whitespace was skipped and no further digit follows after it.
fn match_split(text: &str, start: usize, tail: &str) -> Option<usize> {
    let mut i = start;
    let mut skipped = false;
    let mut prev: Option<char> = None;
    for want in tail.chars() {
        loop {
            let c = text[i..].chars().next()?;
            let next_is_digit =
                text[i + c.len_utf8()..].trim_start_matches([' ', '\u{a0}']).starts_with(|d: char| d.is_ascii_digit());
            if matches!(c, ' ' | '\u{a0}')
                && prev.is_some_and(|p| p.is_ascii_digit())
                && want.is_ascii_digit()
                && next_is_digit
            {
                skipped = true;
                i += c.len_utf8();
                continue;
            }
            if c != want {
                return None;
            }
            i += c.len_utf8();
            prev = Some(c);
            break;
        }
    }
    let after_digit = text[i..].starts_with(|d: char| d.is_ascii_digit());
    (skipped && !after_digit).then_some(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_and_typography_do_not_count() {
        let a: Vec<String> = words("Der _Sämann_ ging hin\\-aus, zu säen.").into_iter().map(|w| w.key).collect();
        let b: Vec<String> = words("der Sämann ging hinaus zu säen").into_iter().map(|w| w.key).collect();
        assert_eq!(a, b, "Auszeichnung, Maskierung, Komma und Groß/Klein fallen weg");
        let c: Vec<String> = words("Sä-\nmann").into_iter().map(|w| w.key).collect();
        assert_eq!(c, vec!["sämann"], "Trennung am Zeilenende wird verbunden");
        let d: Vec<String> = words("Vor- und Nachteile").into_iter().map(|w| w.key).collect();
        assert_eq!(d, vec!["vor", "und", "nachteile"], "Ergänzungsstrich bleibt getrennt");
    }

    #[test]
    fn footnotes_inside_a_quote_are_ignored() {
        let w: Vec<String> =
            words("Das Licht#footnote[Anmerkung des Autors] leuchtet").into_iter().map(|w| w.key).collect();
        assert_eq!(w, vec!["das", "licht", "leuchtet"], "Fußnote im Zitat zählt nicht");
    }

    #[test]
    fn split_digits_are_rejoined_only_for_known_labels() {
        let labels = vec!["Mk 10,42–45".to_owned()];
        assert_eq!(
            rejoin_split_refs("- Mk 1 0,42–45: vorhanden", &labels),
            "- Mk 10,42–45: vorhanden",
            "Ziffern verbunden"
        );
        assert_eq!(rejoin_split_refs("Mk 1 2,1", &labels), "Mk 1 2,1", "andere Stelle bleibt");
        assert_eq!(rejoin_split_refs("Mk 10,42–45", &labels), "Mk 10,42–45", "richtige Stelle bleibt");
    }
}
