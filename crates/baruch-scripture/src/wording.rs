// SPDX-License-Identifier: AGPL-3.0-or-later
//! Wortlaut eines Zitats gegen den Text einer Bibel vergleichen (rein, ohne Bibeltext): Zitate im
//! Quelltext finden ([`inline_quotes`], auch verschachtelt und über Absätze; Blockzitate liefert der Aufrufer,
//! im Server `crate::quotes::quote_spans`), einer Stelle zuordnen
//! ([`quote_for`]) und normalisiert vergleichen ([`compare`]: Satzzeichen, Groß/Klein, Bindestriche,
//! Trennungen, Leerraum, typografische Zeichen und Typst-Auszeichnung zählen nicht).
//!
//! Runde 19c: Unicode-Normalform NFC vor dem Zerlegen (zerlegte Umlaute aus PDFs zählen wie zusammengesetzte);
//! weicht ein Zitat nur im Leerraum ab (Trennreste im Modul: „die ses“, „Men schen“), stimmt es mit
//! [`Comparison::spacing`]; [`fragment_survey`] schätzt, ob ein Modul auffällig viele Wortfragmente hat;
//! [`quote_for`] lässt kurze Einleitungswörter zwischen Stelle und Zitat zu („wie Lk 15,6 erzählt: „…““).

use std::collections::HashSet;

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

use crate::Found;

/// Ein Zitat im durchsuchten Text: Bereich des Inhalts (ohne Anführungszeichen) in Bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct QuoteSpan {
    pub start: usize,
    pub end: usize,
    /// Bereich einschließlich der Anführungszeichen bzw. des ganzen Blockzitats.
    pub outer: (usize, usize),
    /// Blockzitat (`#quote(block: true …)[…]`) statt Anführungszeichen.
    pub block: bool,
}

/// Paare von Anführungszeichen: deutsch „…“, „…”, englisch “…”, französisch »…« und «…», gerade "…".
const PAIRS: [(char, char); 6] = [('„', '“'), ('„', '”'), ('“', '”'), ('»', '«'), ('«', '»'), ('"', '"')];

/// Höchstlänge eines Zitats in Anführungszeichen (Zeichen); längere sind wohl ein vergessenes Zeichen.
const MAX_QUOTE_CHARS: usize = 3000;

/// Zitate in Anführungszeichen. Ein Zitat endet am passenden Schlusszeichen. Typst-`#quote[…]` im Fließtext
/// zählt ebenfalls.
///
/// Seit Runde 17b:
/// - **Verschachtelt:** Innere Anführungszeichen einer anderen Art („… ‚…‘ …“, „… »…« …“) gehören zum äußeren
///   Zitat; ein inneres Paar wird übersprungen, sein Schlusszeichen beendet das äußere Zitat nicht.
/// - **Über Absätze:** Eine Leerzeile beendet die Suche nicht mehr, wenn das Zitat im nächsten Absatz
///   weitergeht – entweder mit dem wiederholten Anführungszeichen am Absatzanfang (deutscher Satz) oder bis
///   zum Schlusszeichen, solange kein weiteres öffnendes Zeichen derselben Art kommt (dann war das erste nie
///   geschlossen: kein Zitat).
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

/// Innere Paare, die ein Zitat einer anderen Art enthalten darf (einfache deutsche/englische/französische
/// Anführungszeichen und die anderen Paare aus [`PAIRS`]).
const INNER_PAIRS: [(char, char); 8] =
    [('‚', '‘'), ('‘', '’'), ('›', '‹'), ('‹', '›'), ('»', '«'), ('«', '»'), ('„', '“'), ('“', '”')];

/// Ende (Byte, Breite des Schlusszeichens) des Zitats, das an `i` mit `open` beginnt; `None`, wenn `open` kein
/// öffnendes Zeichen ist oder kein passendes Schlusszeichen kommt. Ein Schlusszeichen hängt am Wort davor;
/// steht es nach Leerraum und öffnet es selbst ein Paar („er sagte “Ja”“), ist es ein inneres Zitat.
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
            // Wiederholtes Zeichen am Absatzanfang: das Zitat geht weiter; sonst war das erste nie geschlossen.
            let paragraph_start = text[..at].trim_end_matches([' ', '\t']).ends_with("\n\n");
            if crossed && paragraph_start {
                continue;
            }
            return None;
        }
        // Inneres Paar einer anderen Art überspringen („… ‚…‘ …“), nur innerhalb des Absatzes.
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

/// Länge bis zur schließenden `]` (ausschließlich) bei ausgeglichenen Klammern; `\]` zählt nicht.
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

/// Höchstabstand zwischen Zitat und Stelle (Bytes), ohne andere Stellen derselben Liste.
const MAX_GAP: usize = 24;

/// Steht zwischen `a` und `b` nur Leerraum, Satzzeichen oder eine der anderen Stellen?
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

/// Kurze Einleitungswörter zwischen einer Stelle und dem Zitat danach (Runde 19c): „wie Lk 15,6 erzählt: „…““,
/// „In Joh 3 heißt es: „…““. Nur kleingeschriebene Wörter aus dieser Liste, höchstens [`MAX_INTRO_WORDS`], und
/// zuletzt ein Doppelpunkt.
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
/// Höchstens so viele Einleitungswörter.
pub const MAX_INTRO_WORDS: usize = 3;
/// Höchstabstand mit Einleitungswörtern (Bytes, ohne andere Stellen).
const MAX_INTRO_GAP: usize = 48;

/// Zwischen `a` (Ende der Stelle) und `b` (Anfang des Zitats): Leerraum/Satzzeichen, dann 1 bis
/// [`MAX_INTRO_WORDS`] Wörter aus [`INTRO_WORDS`], ein Doppelpunkt und danach nur noch Leerraum.
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

/// Das Zitat zur Stelle `refs[index]`: zuerst eines, das direkt davor endet („…“ (Joh 3,16)), sonst eines,
/// das direkt danach beginnt (Joh 3,16: „…“). Dazwischen dürfen nur Leerraum, Klammern, Satzzeichen und
/// andere Stellen derselben Liste stehen, vor einem Zitat danach seit Runde 19c auch kurze Einleitungswörter
/// mit Doppelpunkt ([`INTRO_WORDS`]). Ein Blockzitat gilt auch, wenn die Stelle darin (Zuschreibung) steht.
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
    // Integrator Runde 17: Liegt zwischen dem Zitat davor und der Stelle eine Leerzeile, im Zitat danach aber
    // nicht, gehört die Stelle zum Zitat danach („„…“ (Joh 3,16)⏎⏎Psalm 139, Vers 23: „…““) – sonst bekäme sie
    // das Zitat des vorigen Absatzes.
    if let (Some(q), Some(next)) = (before, after)
        && paragraph_break(text, q.outer.1, a)
        && !paragraph_break(text, b, next.outer.0)
    {
        return Some(*next);
    }
    if let Some(q) = before {
        return Some(*q);
    }
    // Stelle am Ende eines Blockzitats („… geliebt. (Joh 3,16)“): Zitat bis zur Stelle.
    if let Some(q) = quotes.iter().find(|q| q.block && q.start <= a && b <= q.end && only_glue(text, b, q.end, refs)) {
        let cut = text[q.start..a].trim_end_matches(|c: char| c.is_whitespace() || c == '(').len();
        return Some(QuoteSpan { start: q.start, end: q.start + cut, outer: q.outer, block: true });
    }
    after.copied()
}

/// Steht zwischen `a` und `b` eine Leerzeile (zwei Zeilenumbrüche, dazwischen nur Leerraum)?
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

// ---- Normalisieren ----------------------------------------------------------------------------------

/// Ein Wort: wie es dasteht (ohne umgebende Satzzeichen) und sein Vergleichsschlüssel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    pub shown: String,
    pub key: String,
}

/// Typst-Auszeichnung entfernen: `#footnote[…]` ganz, `#name[`/`#name(…)[` und Klammern, `_`, `*`,
/// Maskierungen `\x` (`\` vor einem Zeichen), `~` als Leerzeichen, Kommentare `// …` bis zum Zeilenende.
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

/// Zeichen, die innerhalb eines Worts wegfallen (Bindestriche, Apostrophe, weiches Trennzeichen, Breite null).
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

/// Wörter eines Texts für den Vergleich (Auszeichnung entfernt). Ein Wort, das mit Bindestrich endet
/// („Hei- land“), wird mit dem folgenden kleingeschriebenen Wort verbunden (Trennung), außer vor
/// „und“/„oder“ (Ergänzungsstrich: „Vor- und Nachteile“).
pub fn words(text: &str) -> Vec<Word> {
    plain_words(&strip_markup(text))
}

/// Wie [`words`], aber für Klartext ohne Typst-Auszeichnung (Bibeltext aus einem Modul): `#`, `_`, `*`, `\`
/// sind dort gewöhnliche Zeichen (Runde 17b: „#1“ im Verstext ist kein Typst-Code).
pub fn plain_words(plain: &str) -> Vec<Word> {
    // Runde 19c: NFC zuerst, sonst zerfiele ein zerlegtes „ü“ (u + U+0308) am Kombinationszeichen in zwei Wörter.
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

// ---- Vergleichen -------------------------------------------------------------------------------------

/// Ein Stück des Wort-Diffs: `same` (in beiden), `bible` (nur in der Bibel, fehlt im Zitat), `quote` (nur im
/// Zitat).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffPart {
    pub op: &'static str,
    pub text: String,
}

/// Ergebnis eines Vergleichs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    /// Alle Wörter des Zitats stehen in dieser Folge lückenlos im Bibeltext (je Teil bei Auslassungen „…“).
    pub matches: bool,
    /// Das Zitat ist kürzer als der Bibeltext (Teilzitat).
    pub partial: bool,
    /// Wort-Diff zwischen Zitat und dem passenden Ausschnitt des Bibeltexts (leer, wenn `matches`).
    pub diff: Vec<DiffPart>,
    /// Runde 19c: Das Zitat stimmt nur, wenn man den Leerraum weglässt (Trennreste im Modul wie „die ses“ oder
    /// ein zusammengeschriebenes Wort); dann ist `matches` wahr.
    pub spacing: bool,
}

/// Teile eines Zitats an Auslassungen („…“, „...“, „[…]“, „(…)“).
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

/// Vergleicht ein Zitat (Typst-Quelltext) mit dem Bibeltext (Klartext des Moduls, ohne Auszeichnung).
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

/// Runde 19c: Vergleich ohne Leerraum. Die Schlüssel der Bibelwörter werden aneinandergehängt; jeder Teil des
/// Zitats muss darin (in Folge) vorkommen, und zwar von einer Wortgrenze des Moduls bis zu einer Wortgrenze –
/// „dieses“ passt auf „die ses“, aber „ist“ nicht mitten in „Christus“. Runde 23a: Toleriert werden nur zusätzliche
/// Wortgrenzen des **Moduls** (Fragmente); jede Wortgrenze des Zitats muss auch im Modul eine sein – „die Bein“ passt
/// nicht auf „Diebe in“. `Some(Teilzitat?)` bei Erfolg.
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
            // Runde 23a (Gutachten R19): Auch jede Wortgrenze **im Zitat** muss eine Wortgrenze des Moduls sein. Das
            // Modul darf zusätzliche Grenzen haben (Wortfragmente wie „die ses“), das Zitat keine eigenen – sonst
            // passte „die Bein“ auf „Diebe in“.
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

/// Wort-Diff über die längste gemeinsame Teilfolge; der Bibeltext wird auf den Bereich zwischen dem
/// ersten und letzten gemeinsamen Wort beschränkt (ein Teilzitat zeigt nicht den ganzen Vers als fehlend).
fn word_diff(q: &[Word], b: &[Word]) -> Vec<DiffPart> {
    let (n, m) = (q.len(), b.len());
    // Bei sehr langen Texten nur grob: alles als verschieden.
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
    // Bibelwörter vor dem ersten und nach dem letzten gemeinsamen Wort weglassen.
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

/// Diff als eine Zeile: `[−Bibel]` fehlt im Zitat, `[+Zitat]` steht nur im Zitat.
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

// ---- Wortfragmente im Modul (Runde 19c) ---------------------------------------------------------------

/// Kurze, häufige Wörter, die als zweites Stück nie als Fragment zählen („da mit“, „hin aus“, „ich bin“).
const NOT_FRAGMENTS: [&str; 62] = [
    "und", "oder", "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "eines", "er",
    "es", "sie", "ihn", "ihm", "ihr", "ihre", "wir", "uns", "euch", "ich", "mich", "mir", "du", "dich", "dir", "zu",
    "an", "auf", "aus", "in", "im", "mit", "von", "vom", "vor", "bei", "nach", "um", "ist", "sind", "war", "hat",
    "nicht", "so", "da", "wie", "als", "wo", "was", "wer", "ob", "noch", "auch", "nur", "sich", "hin", "her",
];
/// Längstes zweites Stück, das als Fragment zählt (Zeichen).
pub const MAX_FRAGMENT_CHARS: usize = 5;
/// So viele Verdachtsfälle braucht ein Modul mindestens für den Hinweis.
pub const MIN_FRAGMENTS: usize = 3;
/// Anteil der Verdachtsfälle an allen Wortpaaren, ab dem ein Modul auffällig ist: 1 von 200 (0,5 %).
pub const FRAGMENT_SHARE: usize = 200;

/// Ergebnis von [`fragment_survey`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FragmentSurvey {
    /// Benachbarte Wortpaare im gelesenen Text (innerhalb eines Verses).
    pub pairs: usize,
    /// Verdachtsfälle: Paare, die wie ein getrenntes Wort aussehen.
    pub suspects: usize,
    /// Höchstens drei Beispiele („die ses“).
    pub examples: Vec<String>,
}

impl FragmentSurvey {
    /// Auffällig viele Wortfragmente: mindestens [`MIN_FRAGMENTS`] Fälle und mindestens 1 von
    /// [`FRAGMENT_SHARE`] Wortpaaren.
    pub fn fragmented(&self) -> bool {
        self.suspects >= MIN_FRAGMENTS && self.suspects * FRAGMENT_SHARE >= self.pairs
    }
}

/// Kern eines Leerraum-Stücks: Buchstaben ohne Satzzeichen davor/danach; `None`, wenn innen anderes steht.
fn letters(token: &str) -> Option<&str> {
    let core = token.trim_matches(|c: char| !c.is_alphanumeric());
    (!core.is_empty() && core.chars().all(char::is_alphabetic)).then_some(core)
}

/// Schätzt Trennreste in Bibeltext (Klartext je Vers, NFC). Ein Verdachtsfall ist ein Paar benachbarter Stücke
/// „A b“ im selben Vers, bei dem A ohne Satzzeichen endet, b kleingeschrieben und höchstens
/// [`MAX_FRAGMENT_CHARS`] Zeichen lang ist, kein häufiges kurzes Wort ([`NOT_FRAGMENTS`]) ist – und „Ab“ als
/// ganzes Wort sonst im gelesenen Text vorkommt („die ses“ neben „dieses“, „Men schen“ neben „Menschen“).
/// Eine Schätzung, keine Silbentrennung: [`FragmentSurvey::fragmented`] entscheidet über den Hinweis.
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
            // A endet mit einem Buchstaben (kein Satzzeichen, das ein Wortende anzeigt), b beginnt klein.
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

// ---- Getrennte Ziffern in Antworten -----------------------------------------------------------------

/// Setzt Stellenangaben wieder zusammen, deren Ziffern in einer Antwort durch Leerraum getrennt sind
/// („Mk 1 0,42–45“ statt „Mk 10,42–45“): Nur wenn das Ergebnis genau eine der Angaben in `labels` ist,
/// wird ersetzt; sonst bleibt der Text, wie er ist.
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

/// Ab `start` den Text `tail` lesen und dabei Leerraum zwischen zwei Ziffern überspringen; `Some(Ende)` nur,
/// wenn dabei mindestens ein Leerraum übersprungen wurde und danach keine weitere Ziffer folgt.
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
