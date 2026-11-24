// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lesen eines Abschnittstexts für Registermarken (Runde 31b): Welche Bytes sind Fließtext, in dem Marken stehen und
//! Wörter gezählt werden dürfen, und wo stehen schon Marken?
//!
//! Gesperrt (nie Fließtext) sind Überschriftszeilen (`=`-Zeilen, Teil- und „ohne Nummer“-Markierungen), Maskierungen
//! (`\x`), Raw (`` `…` ``, Zäune), Kommentare (`//`, `/* */`), Formeln (`$…$`), Labels (`<name>`), Verweise (`@name`),
//! Adressen (`https://…`), Code hinter `#` samt Argumenten und Strings, Anweisungen (`#let`, `#set`, `#show`, `#if` …
//! bis zum Zeilenende samt allen Klammern) und Marken. Inhaltsblöcke eines Aufrufs (`#emph[…]`, `#footnote[…]`,
//! `caption: […]`) sind wieder Fließtext – dort stehen Wörter des Buchs.
//!
//! Jede gesperrte Stelle hat eine eigene Nummer (`region`); eine Einfügestelle zwischen zwei Bytes derselben Nummer
//! liegt mitten in ihr ([`Scan::insertable`]).

use super::{FoundMark, parse_mark};

/// Art einer gesperrten Stelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Escape,
    Raw,
    LineComment,
    BlockComment,
    Math,
    Code,
    Label,
    Reference,
    Url,
    Heading,
    Mark,
}

/// Eine Überschriftszeile auf oberster Ebene: Bytebereich ohne Zeilenende, Ebene (0 = Teil, 1–6 = Anzahl `=` bzw.
/// Ebene der Markierung „ohne Nummer“).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HeadingLine {
    pub start: usize,
    pub level: u8,
}

/// Ergebnis des Lesens.
pub(super) struct Scan {
    /// Je Byte: 0 = Fließtext, sonst Nummer der gesperrten Stelle (1-basiert in `kinds`).
    region: Vec<u32>,
    kinds: Vec<Kind>,
    /// Endet der Text in einer offenen Stelle (Raw, Kommentar, Formel, Klammer ohne Ende)?
    open_end: bool,
    /// Alle kanonischen Marken in Textreihenfolge (auch solche in gesperrten Anweisungen: Typst setzt sie).
    pub marks: Vec<FoundMark>,
    /// Überschriftszeilen auf oberster Ebene in Textreihenfolge.
    pub headings: Vec<HeadingLine>,
}

impl Scan {
    fn kind_at(&self, i: usize) -> Option<Kind> {
        match self.region.get(i).copied() {
            None | Some(0) => None,
            Some(id) => self.kinds.get(id as usize - 1).copied(),
        }
    }

    /// Ist Byte `i` Fließtext?
    pub fn prose(&self, i: usize) -> bool {
        self.region.get(i) == Some(&0)
    }

    /// Sind alle Bytes in `a..b` Fließtext?
    pub fn prose_range(&self, a: usize, b: usize) -> bool {
        self.region.get(a..b).is_some_and(|r| r.iter().all(|&x| x == 0))
    }

    /// Ist Byte `i` der Anfang eines Labels (`<name>`)?
    pub fn label_at(&self, i: usize) -> Option<usize> {
        if self.kind_at(i) != Some(Kind::Label) || (i > 0 && self.region[i - 1] == self.region[i]) {
            return None;
        }
        let id = self.region[i];
        let mut end = i;
        while self.region.get(end) == Some(&id) {
            end += 1;
        }
        Some(end)
    }

    /// Darf an der Byte-Stelle `p` (Zeichengrenze) eine Marke stehen? Nicht mitten in einer gesperrten Stelle, nicht am
    /// Ende einer Kommentar- oder Überschriftszeile (dort gehörte sie noch zur Zeile), nicht am Anfang einer
    /// Überschriftszeile, nicht vor einem Label (es hinge sonst an der Marke) und nicht am Ende eines offenen Raw,
    /// Kommentars oder einer offenen Klammer.
    pub fn insertable(&self, text: &str, p: usize) -> bool {
        if p > text.len() || !text.is_char_boundary(p) {
            return false;
        }
        let left = p.checked_sub(1).map_or(0, |i| self.region[i]);
        let right = self.region.get(p).copied().unwrap_or(0);
        if left != 0 && left == right {
            return false;
        }
        if matches!(p.checked_sub(1).and_then(|i| self.kind_at(i)), Some(Kind::LineComment | Kind::Heading)) {
            return false;
        }
        if matches!(self.kind_at(p), Some(Kind::Heading | Kind::Label)) {
            return false;
        }
        !(p == text.len() && left != 0 && self.open_end)
    }

    /// Der Text mit Leerzeichen statt aller gesperrten Bytes (Zeilenenden bleiben); Bytepositionen bleiben gleich und
    /// das Ergebnis ist gültiges UTF-8, weil jede gesperrte Stelle ganze Zeichen umfasst.
    pub fn masked(&self, text: &str) -> String {
        let bytes: Vec<u8> =
            text.bytes().zip(&self.region).map(|(b, &r)| if r == 0 || b == b'\n' { b } else { b' ' }).collect();
        String::from_utf8(bytes).unwrap_or_else(|_| " ".repeat(text.len()))
    }

    /// Wörter im Fließtext: größte Folgen von Buchstaben und Ziffern, deren Bytes alle Fließtext sind.
    pub fn words(&self, text: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut start: Option<usize> = None;
        for (i, c) in text.char_indices() {
            let word = c.is_alphanumeric() && self.prose(i);
            match (word, start) {
                (true, None) => start = Some(i),
                (false, Some(s)) => {
                    out.push((s, i));
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            out.push((s, text.len()));
        }
        out
    }
}

/// Schlüsselwörter, nach denen der Rest der Zeile (samt aller Klammern) Anweisung ist.
const STATEMENTS: &[&str] =
    &["let", "set", "show", "import", "include", "if", "else", "for", "while", "return", "break", "continue"];

/// Liest `text`.
pub(super) fn scan(text: &str) -> Scan {
    let mut scanner = Scanner {
        s: text,
        b: text.as_bytes(),
        region: vec![0; text.len()],
        kinds: Vec::new(),
        open_end: false,
        marks: Vec::new(),
        headings: Vec::new(),
    };
    scanner.markup(0, 0, false);
    let Scanner { region, kinds, open_end, marks, headings, .. } = scanner;
    Scan { region, kinds, open_end, marks, headings }
}

struct Scanner<'s> {
    s: &'s str,
    b: &'s [u8],
    region: Vec<u32>,
    kinds: Vec<Kind>,
    open_end: bool,
    marks: Vec<FoundMark>,
    headings: Vec<HeadingLine>,
}

fn label_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.')
}

fn ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

impl Scanner<'_> {
    fn new_region(&mut self, kind: Kind) -> u32 {
        self.kinds.push(kind);
        self.kinds.len() as u32
    }

    fn fill(&mut self, from: usize, to: usize, id: u32) {
        let to = to.min(self.region.len());
        for r in &mut self.region[from.min(to)..to] {
            *r = id;
        }
    }

    fn char_at(&self, i: usize) -> Option<char> {
        self.s.get(i..).and_then(|r| r.chars().next())
    }

    fn char_len(&self, i: usize) -> usize {
        self.char_at(i).map_or(1, char::len_utf8)
    }

    fn line_end(&self, i: usize) -> usize {
        self.b[i..].iter().position(|&b| b == b'\n').map_or(self.b.len(), |n| i + n)
    }

    /// `//` ist kein Kommentar direkt hinter `http:` bzw. `https:`.
    fn after_scheme(&self, i: usize) -> bool {
        let before = &self.s[..i];
        before.ends_with("http:") || before.ends_with("https:")
    }

    /// Überschriftszeile ab dem Zeilenanfang `i`: Ende (ohne Zeilenende) und Ebene.
    fn heading_line(&self, i: usize) -> Option<(usize, u8)> {
        let end = self.line_end(i);
        let line = &self.s[i..end];
        let trimmed = line.trim_start_matches([' ', '\t']);
        let equals = trimmed.bytes().take_while(|&b| b == b'=').count();
        if equals > 0 && trimmed[equals..].chars().next().is_none_or(|c| matches!(c, ' ' | '\t' | '\r')) {
            return Some((end, equals.min(6) as u8));
        }
        if let Some(marker) = crate::marker::read_marker(line) {
            return Some((end, marker.level));
        }
        None
    }

    /// Markup ab `i` bis zum Ende bzw. (mit `close`) bis zur passenden `]`. Rückgabe: Stelle dahinter und ob die
    /// Klammer geschlossen wurde.
    fn markup(&mut self, mut i: usize, depth: usize, close: bool) -> (usize, bool) {
        let len = self.b.len();
        while i < len {
            if (i == 0 || self.b[i - 1] == b'\n')
                && let Some((end, level)) = self.heading_line(i)
            {
                let id = self.new_region(Kind::Heading);
                self.fill(i, end, id);
                if depth == 0 {
                    self.headings.push(HeadingLine { start: i, level });
                }
                i = end;
                continue;
            }
            match self.b[i] {
                b']' if close => return (i + 1, true),
                b'[' if close => {
                    let (end, closed) = self.markup(i + 1, depth + 1, true);
                    if !closed {
                        return (end, false);
                    }
                    i = end;
                }
                b'\\' => {
                    let n = if i + 1 < len { self.char_len(i + 1) } else { 0 };
                    let id = self.new_region(Kind::Escape);
                    self.fill(i, i + 1 + n, id);
                    i += 1 + n;
                }
                b'`' => i = self.raw(i, Kind::Raw),
                b'/' if self.b.get(i + 1) == Some(&b'/') && !self.after_scheme(i) => {
                    let end = self.line_end(i);
                    let id = self.new_region(Kind::LineComment);
                    self.fill(i, end, id);
                    if end == len {
                        self.open_end = true;
                    }
                    i = end;
                }
                b'/' if self.b.get(i + 1) == Some(&b'*') => i = self.block_comment(i, None),
                b'$' => {
                    let mut j = i + 1;
                    while j < len && self.b[j] != b'$' {
                        j += if self.b[j] == b'\\' && j + 1 < len { 1 + self.char_len(j + 1) } else { 1 };
                    }
                    let end = (j + 1).min(len);
                    if j >= len {
                        self.open_end = true;
                    }
                    let id = self.new_region(Kind::Math);
                    self.fill(i, end, id);
                    i = end;
                }
                b'<' => {
                    let mut j = i + 1;
                    while let Some(c) = self.char_at(j).filter(|&c| label_char(c)) {
                        j += c.len_utf8();
                    }
                    if j > i + 1 && self.b.get(j) == Some(&b'>') {
                        let id = self.new_region(Kind::Label);
                        self.fill(i, j + 1, id);
                        i = j + 1;
                    } else {
                        i += 1;
                    }
                }
                b'@' if self.char_at(i + 1).is_some_and(|c| c.is_alphanumeric() || c == '_') => {
                    let mut j = i + 1;
                    while let Some(c) = self.char_at(j).filter(|&c| label_char(c)) {
                        j += c.len_utf8();
                    }
                    while self.s[..j].ends_with(['.', ':']) {
                        j -= 1;
                    }
                    let id = self.new_region(Kind::Reference);
                    self.fill(i, j, id);
                    i = j;
                }
                b'h' if (self.s[i..].starts_with("http://") || self.s[i..].starts_with("https://"))
                    && self.s[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric()) =>
                {
                    let mut j = i;
                    while let Some(c) = self.char_at(j).filter(|&c| !c.is_whitespace() && c != ']' && c != '>') {
                        j += c.len_utf8();
                    }
                    let id = self.new_region(Kind::Url);
                    self.fill(i, j, id);
                    i = j;
                }
                b'#' => i = self.hash(i, depth),
                _ => i += self.char_len(i),
            }
        }
        (len, false)
    }

    /// Raw ab `i` (Folge von Backticks); bis zur nächsten gleich langen Folge, `` `` `` ist leer.
    fn raw(&mut self, i: usize, kind: Kind) -> usize {
        let run = self.b[i..].iter().take_while(|&&b| b == b'`').count();
        let end = if run == 2 {
            i + 2
        } else {
            let fence = "`".repeat(run);
            match self.s[i + run..].find(&fence) {
                Some(close) => i + run + close + run,
                None => {
                    self.open_end = true;
                    self.b.len()
                }
            }
        };
        let id = self.new_region(kind);
        self.fill(i, end, id);
        end
    }

    /// `/* … */` ab `i`, geschachtelt. Mit `into` in eine bestehende Stelle (Code), sonst eigene Stelle.
    fn block_comment(&mut self, i: usize, into: Option<u32>) -> usize {
        let len = self.b.len();
        let mut depth = 0u32;
        let mut j = i;
        let mut closed = false;
        while j < len {
            if self.b[j..].starts_with(b"/*") {
                depth += 1;
                j += 2;
            } else if self.b[j..].starts_with(b"*/") {
                depth -= 1;
                j += 2;
                if depth == 0 {
                    closed = true;
                    break;
                }
            } else {
                j += 1;
            }
        }
        let end = j.min(len);
        if !closed {
            self.open_end = true;
        }
        let id = into.unwrap_or_else(|| self.new_region(Kind::BlockComment));
        self.fill(i, end, id);
        end
    }

    /// String-Literal ab dem `"` bei `i` (mit `\`-Maskierungen) → Stelle hinter dem schließenden `"`.
    fn string_end(&mut self, i: usize) -> usize {
        let len = self.b.len();
        let mut k = i + 1;
        while k < len {
            match self.b[k] {
                b'\\' => k += 2,
                b'"' => return k + 1,
                _ => k += 1,
            }
        }
        self.open_end = true;
        len
    }

    /// Eingebetteter Ausdruck ab `#` bei `i`; eine kanonische Marke wird als Marke gemerkt.
    fn hash(&mut self, i: usize, depth: usize) -> usize {
        if let Some((mark, used)) = parse_mark(&self.s[i..]) {
            let id = self.new_region(Kind::Mark);
            self.fill(i, i + used, id);
            self.marks.push(FoundMark { span: (i, i + used), mark });
            return i + used;
        }
        let id = self.new_region(Kind::Code);
        self.fill(i, i + 1, id);
        self.expr(i + 1, id, depth)
    }

    /// Ein Ausdruck ab `j` (nach `#` bzw. nach `context`), gesperrt mit `id`; Inhaltsblöcke von Aufrufen bleiben
    /// Fließtext, Anweisungen sind bis zum Zeilenende ganz gesperrt.
    fn expr(&mut self, j: usize, id: u32, depth: usize) -> usize {
        match self.char_at(j) {
            Some('(' | '{') => self.group(j, id, depth),
            Some('[') => self.content(j, id, depth),
            Some('"') => {
                let end = self.string_end(j);
                self.fill(j, end, id);
                end
            }
            Some(c) if ident_start(c) => {
                let mut k = j;
                while let Some(c) = self.char_at(k).filter(|&c| ident_char(c)) {
                    k += c.len_utf8();
                }
                let name = &self.s[j..k];
                self.fill(j, k, id);
                if name == "context" {
                    let mut p = k;
                    while matches!(self.b.get(p), Some(b' ' | b'\t')) {
                        p += 1;
                    }
                    self.fill(k, p, id);
                    return self.expr(p, id, depth);
                }
                if STATEMENTS.contains(&name) {
                    let start = j;
                    let end = self.statement(k, id, depth);
                    // Anweisungen ganz sperren (auch ihre Inhaltsblöcke); Marken darin bleiben gemerkt.
                    self.fill(start, end, id);
                    return end;
                }
                let mut end = k;
                loop {
                    match self.b.get(end) {
                        Some(b'.') if self.char_at(end + 1).is_some_and(ident_start) => {
                            let mut e = end + 1;
                            while let Some(c) = self.char_at(e).filter(|&c| ident_char(c)) {
                                e += c.len_utf8();
                            }
                            self.fill(end, e, id);
                            end = e;
                        }
                        Some(b'(') => end = self.group(end, id, depth),
                        Some(b'[') => end = self.content(end, id, depth),
                        _ => break,
                    }
                }
                end
            }
            // `#` ohne Ausdruck: nur das Zeichen selbst.
            _ => j,
        }
    }

    /// Inhaltsblock `[…]` eines Ausdrucks ab `j`: Klammern gesperrt, Inhalt Fließtext.
    fn content(&mut self, j: usize, id: u32, depth: usize) -> usize {
        self.fill(j, j + 1, id);
        let (end, closed) = self.markup(j + 1, depth + 1, true);
        if closed {
            self.fill(end - 1, end, id);
        } else {
            self.open_end = true;
        }
        end
    }

    /// Klammer `(…)` bzw. `{…}` im Code ab `open`, gesperrt; Inhaltsblöcke darin sind Fließtext.
    fn group(&mut self, open: usize, id: u32, depth: usize) -> usize {
        let close = if self.b[open] == b'(' { b')' } else { b'}' };
        self.fill(open, open + 1, id);
        let end = self.code(open + 1, id, depth, Some(close));
        if end > self.b.len() || self.b.get(end - 1) != Some(&close) {
            self.open_end = true;
        }
        end
    }

    /// Anweisung ab `k` bis zum Zeilenende bzw. `;` auf oberster Ebene.
    fn statement(&mut self, k: usize, id: u32, depth: usize) -> usize {
        self.code(k, id, depth, None)
    }

    /// Code ab `j` bis hinter `close` (bzw. ohne `close` bis vor das Zeilenende oder hinter `;`).
    fn code(&mut self, mut j: usize, id: u32, depth: usize, close: Option<u8>) -> usize {
        let len = self.b.len();
        while j < len {
            let c = self.b[j];
            if Some(c) == close {
                self.fill(j, j + 1, id);
                return j + 1;
            }
            match c {
                b'\n' if close.is_none() => return j,
                b';' if close.is_none() => {
                    self.fill(j, j + 1, id);
                    return j + 1;
                }
                b'"' => {
                    let end = self.string_end(j);
                    self.fill(j, end, id);
                    j = end;
                }
                b'(' | b'{' => j = self.group(j, id, depth),
                b'[' => j = self.content(j, id, depth),
                b'/' if self.b.get(j + 1) == Some(&b'/') && !self.after_scheme(j) => {
                    let end = self.line_end(j);
                    self.fill(j, end, id);
                    j = end;
                }
                b'/' if self.b.get(j + 1) == Some(&b'*') => j = self.block_comment(j, Some(id)),
                b'`' => {
                    let end = self.raw(j, Kind::Code);
                    self.fill(j, end, id);
                    j = end;
                }
                // Fremde schließende Klammer: hier endet der Code (vorsichtig).
                b')' | b'}' | b']' => {
                    self.fill(j, j + 1, id);
                    return j + 1;
                }
                _ => {
                    let n = self.char_len(j);
                    self.fill(j, j + n, id);
                    j += n;
                }
            }
        }
        if close.is_some() {
            self.open_end = true;
        }
        len
    }
}
