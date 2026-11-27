// SPDX-License-Identifier: AGPL-3.0-or-later
//! Outline schemes and pattern language of the numbering (contract Round 6, section 2).
//!
//! **Pattern language** (stored in `work.numbering` for the scheme „Eigenes Muster“, also the input in the outline
//! header):
//! - `P` – one Typst pattern hierarchically for all levels (`1.1`, `I.1`, `1.a)`), as before;
//! - `P|k` (k = 1 to 6) – hierarchical, but only the levels 1 to k carry a number (`I|1` = only chapters in Roman
//!   numerals);
//! - `P1;P2;…;Pn` (n ≥ 2) – one pattern per level: if `Pi` has exactly one counting symbol, level i shows only its
//!   own number, otherwise hierarchically; an empty `Pi` and levels behind n have no number (`I.;A.;1.;a);aa)`).
//! - Two equal letter counting symbols directly after each other (`aa`, `AA`) count as a doubled letter: aa, bb, cc
//!   … (extension of Baruch; in Typst as `numbering("a", x) * 2`).
//! - `;` and `|` are reserved; every non-empty pattern needs a counting symbol (`1 a A i I *`).
//!
//! **Levels**: chapter = 1, section = 2, subsection = 3. Parts (level 0, without heading line) count on their own
//! ([`Numbering::part`]), chapters continue across parts.
//!
//! **Export** ([`export_template`]): with „Aus der Werkvorlage“ the template stays byte-identical, otherwise a
//! comment line and a `#set heading(numbering: …)` rule follow it ([`Numbering::heading_rule`]), which
//! [`crate::heading_numbering`] reads back to the same numbers for the pattern-based schemes. The rules of the
//! schemes with words (parts and chapters, spelled out, legal, sermon, book) are Typst functions.
//!
//! **Reading the work template** ([`Numbering::from_template`], Round 14): a numbering from the template (scheme
//! „Aus der Werkvorlage“ and every re-imported work) follows Typst, not the pattern language: a string is a literal
//! Typst pattern (`;`, `|` are text, `aa` are two counting symbols); the rules of the schemes with words generated
//! by Baruch become exactly this scheme again; a function `(..n) => …` (directly or through the name of a `#let`
//! function) is evaluated exactly per level if it only consists of depth conditions (`==`, `!=`, `<`, `<=`, `>`,
//! `>=` on `n.pos().len()`, also through `let p = n.pos()`), nested `if`/`else`, text, `numbering("P", ..n)`,
//! `numbering("P", p.at(i))`, `str(p.at(i))` (also `.first()`, `.last()`, `* k`) and concatenation with `+`.
//! Otherwise the approximation of [`crate::heading_numbering`] applies.

use super::{
    Arg, Counter, Cursor, blank_comments, expression_end, format_number, ident_char, ident_start, is_numbering_pattern,
    pattern_pieces, top_level_hashes,
};

/// Default if neither the work nor its template name a pattern.
pub const DEFAULT_PATTERN: &str = "1.1";

/// Deepest level that a generated rule treats individually (Typst knows levels of any depth; Baruch outlines down
/// to 3, `====` in the text is level 4 in Typst).
const MAX_LEVEL: usize = 6;

/// Highest repetition `… * k` in a numbering function of the work template. Larger ones (also as a product) count
/// as unreadable: `render` really repeats text and numbers, and an unbounded number from a foreign template would
/// otherwise paralyse the process at the memory allocation.
const MAX_REPEAT: usize = 16;

/// Outline scheme of a work (column `work.numbering_scheme`, migration 0008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scheme {
    Template,
    Decimal,
    DecimalDot,
    Alphanumeric,
    ChapterRoman,
    ChapterArabic,
    PartsChapters,
    ChapterWords,
    Legal,
    Sermon,
    Book,
    None,
    Custom,
}

impl Scheme {
    /// All schemes in the order of the selection.
    pub const ALL: [Scheme; 13] = [
        Scheme::Template,
        Scheme::Decimal,
        Scheme::DecimalDot,
        Scheme::Alphanumeric,
        Scheme::ChapterRoman,
        Scheme::ChapterArabic,
        Scheme::PartsChapters,
        Scheme::ChapterWords,
        Scheme::Legal,
        Scheme::Sermon,
        Scheme::Book,
        Scheme::None,
        Scheme::Custom,
    ];

    /// Value in the database (as in the CHECK of migration 0008, `book` since 0040).
    pub fn id(self) -> &'static str {
        match self {
            Scheme::Template => "template",
            Scheme::Decimal => "decimal",
            Scheme::DecimalDot => "decimal_dot",
            Scheme::Alphanumeric => "alphanumeric",
            Scheme::ChapterRoman => "chapter_roman",
            Scheme::ChapterArabic => "chapter_arabic",
            Scheme::PartsChapters => "parts_chapters",
            Scheme::ChapterWords => "chapter_words",
            Scheme::Legal => "legal",
            Scheme::Sermon => "sermon",
            Scheme::Book => "book",
            Scheme::None => "none",
            Scheme::Custom => "custom",
        }
    }

    pub fn from_id(id: &str) -> Option<Scheme> {
        Scheme::ALL.into_iter().find(|s| s.id() == id.trim())
    }

    /// Name in the user interface.
    pub fn label(self) -> &'static str {
        match self {
            Scheme::Template => "Aus der Werkvorlage",
            Scheme::Decimal => "Dezimal (DIN 1421)",
            Scheme::DecimalDot => "Dezimal mit Schlusspunkt",
            Scheme::Alphanumeric => "Alphanumerisch",
            Scheme::ChapterRoman => "Nur Kapitel römisch",
            Scheme::ChapterArabic => "Nur Kapitel arabisch",
            Scheme::PartsChapters => "Teile und Kapitel",
            Scheme::ChapterWords => "Kapitel ausgeschrieben",
            Scheme::Legal => "Juristisch",
            Scheme::Sermon => "Predigt",
            Scheme::Book => "Buch",
            Scheme::None => "Ohne Nummern",
            Scheme::Custom => "Eigenes Muster",
        }
    }

    /// Short example of the numbers.
    pub fn example(self) -> &'static str {
        match self {
            Scheme::Template => "wie #set heading(numbering: …) der Werkvorlage, sonst 1 · 1.1",
            Scheme::Decimal => "1 · 1.1 · 1.1.1",
            Scheme::DecimalDot => "1. · 1.1. · 1.1.1.",
            Scheme::Alphanumeric => "I. · A. · 1. · a) · aa)",
            Scheme::ChapterRoman => "I · II · III, Abschnitte ohne Nummer",
            Scheme::ChapterArabic => "1 · 2 · 3, Abschnitte ohne Nummer",
            Scheme::PartsChapters => "Teil I · Kapitel 1 · 1.1 · 1.1.1",
            Scheme::ChapterWords => "Erster Teil · Erstes Kapitel · Zweites Kapitel",
            Scheme::Legal => "§ 1 · Abs. 1 · Nr. 1",
            Scheme::Sermon => "Einleitung · I. · II. · Schluss",
            Scheme::Book => "Einleitung · I. · II. · Schluss, Teile „Erster Teil“",
            Scheme::None => "keine Nummern",
            Scheme::Custom => "Mustersprache, z. B. I.;A.;1.;a) oder I|1",
        }
    }

    /// The scheme with a fixed pattern language whose numbering corresponds to `pattern` (Round 7, for the outline
    /// header: „I|1“ → [`Scheme::ChapterRoman`]). Compared is the pattern language as read ([`Numbering::spec`]),
    /// so whitespace does not count. „Aus der Werkvorlage“, „Eigenes Muster“ and „Ohne Nummern“ never come out, nor
    /// do the schemes with words (they have no pattern); an empty or invalid pattern yields `None`.
    pub fn matching(pattern: &str) -> Option<Scheme> {
        let wanted = Numbering::parse_spec(pattern).ok()?.spec().filter(|s| !s.is_empty())?;
        Scheme::ALL.into_iter().filter(|s| s.is_pattern() && !matches!(s, Scheme::Template | Scheme::Custom)).find(
            |s| Numbering::of_scheme(*s, None, "").ok().and_then(|n| n.spec()).as_deref() == Some(wanted.as_str()),
        )
    }

    /// Is the scheme based on the pattern language (then [`crate::heading_numbering`] reads the rule of the export
    /// back to the same numbers)?
    pub fn is_pattern(self) -> bool {
        matches!(
            self,
            Scheme::Template
                | Scheme::Decimal
                | Scheme::DecimalDot
                | Scheme::Alphanumeric
                | Scheme::ChapterRoman
                | Scheme::ChapterArabic
                | Scheme::Custom
        )
    }
}

// ---- Patterns with doubled letters ----------------------------------------------------------

/// Counting symbol of a pattern: single or doubled (`aa`, `AA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    One(Counter),
    Double(Counter),
}

impl Mark {
    fn apply(self, n: usize) -> String {
        match self {
            Mark::One(counter) => counter.apply(n),
            Mark::Double(counter) => counter.apply(n).repeat(2),
        }
    }
}

fn counter_char(counter: Counter) -> char {
    match counter {
        Counter::Arabic => '1',
        Counter::LowerLatin => 'a',
        Counter::UpperLatin => 'A',
        Counter::LowerRoman => 'i',
        Counter::UpperRoman => 'I',
        Counter::Symbol => '*',
    }
}

/// A piece of the finished number: text or the number at position `index` of the counter list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Out {
    Text(String),
    Num(Mark, usize),
}

/// A Typst pattern like `I.1` or `aa)`, split into counting symbols with prefix and the suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
    source: String,
    pieces: Vec<(String, Mark)>,
    suffix: String,
}

impl Pattern {
    /// Splits `source`; without a counting symbol → `None`.
    fn parse(source: &str) -> Option<Pattern> {
        let pattern = Pattern::raw(source);
        (!pattern.pieces.is_empty()).then_some(pattern)
    }

    /// Like [`Pattern::parse`], but also without a counting symbol (then it never yields a number, like
    /// [`crate::format_number`]).
    fn raw(source: &str) -> Pattern {
        let chars: Vec<char> = source.chars().collect();
        let mut pieces = Vec::new();
        let mut text = String::new();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            match Counter::of(c) {
                Some(counter) => {
                    let double = matches!(c, 'a' | 'A') && chars.get(i + 1) == Some(&c);
                    let mark = if double { Mark::Double(counter) } else { Mark::One(counter) };
                    pieces.push((std::mem::take(&mut text), mark));
                    i += if double { 2 } else { 1 };
                }
                None => {
                    text.push(c);
                    i += 1;
                }
            }
        }
        Pattern { source: source.to_owned(), pieces, suffix: text }
    }

    /// A literal Typst pattern (from the work template): every counting symbol individually, also `aa` (two
    /// counting symbols as in Typst); `;` and `|` are text. Without a counting symbol it never yields a number.
    fn literal(source: &str) -> Pattern {
        let mut pieces = Vec::new();
        let mut text = String::new();
        for c in source.chars() {
            match Counter::of(c) {
                Some(counter) => pieces.push((std::mem::take(&mut text), Mark::One(counter))),
                None => text.push(c),
            }
        }
        Pattern { source: source.to_owned(), pieces, suffix: text }
    }

    fn counters(&self) -> usize {
        self.pieces.len()
    }

    fn has_double(&self) -> bool {
        self.pieces.iter().any(|(_, m)| matches!(m, Mark::Double(_)))
    }

    /// Structure of the number for `len` numbers by the rules of [`crate::format_number`]: per number the next
    /// counting symbol with prefix, afterwards the last one repeats (with its prefix, with an empty prefix with the
    /// suffix as separator); at the end the suffix.
    fn layout(&self, len: usize) -> Vec<Out> {
        let mut out = Vec::new();
        if self.pieces.is_empty() || len == 0 {
            return out;
        }
        for (index, (prefix, mark)) in self.pieces.iter().take(len).enumerate() {
            out.push(Out::Text(prefix.clone()));
            out.push(Out::Num(*mark, index));
        }
        if let Some((last_prefix, last_mark)) = self.pieces.last() {
            for index in self.pieces.len()..len {
                let separator = if last_prefix.is_empty() { &self.suffix } else { last_prefix };
                out.push(Out::Text(separator.clone()));
                out.push(Out::Num(*last_mark, index));
            }
        }
        out.push(Out::Text(self.suffix.clone()));
        out.retain(|o| !matches!(o, Out::Text(t) if t.is_empty()));
        out
    }

    fn format(&self, counts: &[usize]) -> String {
        self.layout(counts.len())
            .into_iter()
            .map(|o| match o {
                Out::Text(t) => t,
                Out::Num(mark, index) => mark.apply(counts[index]),
            })
            .collect()
    }

    /// Typst expression for the number at level `level` (counter list `p`): `own` = only the own number.
    fn typst(&self, level: usize, own: bool) -> String {
        if !self.has_double() {
            return if own {
                format!("numbering({}, p.at({}))", typst_str(&self.source), level - 1)
            } else {
                format!("numbering({}, ..p)", typst_str(&self.source))
            };
        }
        let (len, shift) = if own { (1, level - 1) } else { (level, 0) };
        let parts: Vec<String> = self
            .layout(len)
            .into_iter()
            .map(|o| match o {
                Out::Text(t) => typst_str(&t),
                Out::Num(Mark::One(c), index) => format!("numbering(\"{}\", p.at({}))", counter_char(c), index + shift),
                Out::Num(Mark::Double(c), index) => {
                    format!("numbering(\"{}\", p.at({})) * 2", counter_char(c), index + shift)
                }
            })
            .collect();
        if parts.is_empty() { "\"\"".to_owned() } else { parts.join(" + ") }
    }
}

/// String literal for Typst (`\` and `"` escaped).
fn typst_str(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

// ---- Numbering --------------------------------------------------------------------------------

/// Pattern of the pattern language.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Spec {
    /// No numbers.
    Off,
    /// One pattern hierarchically for all levels, optionally only down to level `depth`.
    Hier { pattern: Pattern, depth: Option<u8> },
    /// One pattern per level (`None` = level without number).
    Levels(Vec<Option<Pattern>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Spec(Spec),
    /// Chapters „Kapitel 1“, below them 1.1 and 1.1.1.
    PartsChapters,
    /// „Erstes Kapitel“, sections without number.
    ChapterWords,
    /// § 1 · Abs. 1 · Nr. 1.
    Legal,
    /// Einleitung · I. · II. · Schluss, sections without number.
    Sermon,
    /// Like [`Kind::Sermon`] (chapters Einleitung · I. · II. · Schluss, sections without number), parts as words; a
    /// Typst rule of its own, so that [`Numbering::from_template`] tells the two apart.
    Book,
    /// Function of the work template, evaluated exactly per level ([`Numbering::from_template`]); `approx` is the
    /// previous approximation in the pattern language ([`crate::heading_numbering`]) for the display.
    Function {
        node: Node,
        approx: Option<String>,
    },
}

/// How parts (level 0) are counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Parts {
    Roman,
    Named,
    Words,
    Hidden,
}

/// Fully evaluated numbering of a work (from scheme, own pattern and work template).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Numbering {
    kind: Kind,
    parts: Parts,
}

const NEEDS_COUNTER: &str = "braucht ein Zählzeichen (1, a, A, i, I oder *), z. B. „1.1“ oder „I.“";

impl Numbering {
    /// No numbers, not for parts either.
    pub fn off() -> Numbering {
        Numbering { kind: Kind::Spec(Spec::Off), parts: Parts::Hidden }
    }

    /// Reads the pattern language (see module description). Empty = no numbers. Errors in German.
    pub fn parse_spec(spec: &str) -> Result<Numbering, String> {
        let spec = spec.trim();
        if spec.is_empty() {
            return Ok(Numbering::off());
        }
        let parsed = if spec.contains(';') {
            if spec.contains('|') {
                return Err(format!(
                    "„{spec}“: „|“ (Tiefe) und „;“ (Muster je Ebene) lassen sich nicht mischen; \
                     eine Ebene ohne Nummer bleibt in der Liste einfach leer."
                ));
            }
            let mut levels = Vec::new();
            for (index, part) in spec.split(';').enumerate() {
                let part = part.trim();
                if part.is_empty() {
                    levels.push(None);
                    continue;
                }
                let pattern =
                    Pattern::parse(part).ok_or_else(|| format!("Ebene {} („{part}“) {NEEDS_COUNTER}.", index + 1))?;
                levels.push(Some(pattern));
            }
            Spec::Levels(levels)
        } else if let Some((pattern, depth)) = spec.split_once('|') {
            let depth =
                depth.trim().parse::<u8>().ok().filter(|d| (1..=MAX_LEVEL as u8).contains(d)).ok_or_else(|| {
                    format!("„{spec}“: Hinter „|“ steht die Zahl der nummerierten Ebenen (1 bis 6), z. B. „I|1“.")
                })?;
            let pattern = pattern.trim();
            let pattern = Pattern::parse(pattern).ok_or_else(|| format!("Das Muster „{pattern}“ {NEEDS_COUNTER}."))?;
            Spec::Hier { pattern, depth: Some(depth) }
        } else {
            let pattern = Pattern::parse(spec).ok_or_else(|| format!("Das Muster „{spec}“ {NEEDS_COUNTER}."))?;
            Spec::Hier { pattern, depth: None }
        };
        Ok(Numbering { kind: Kind::Spec(parsed), parts: Parts::Roman })
    }

    /// Like [`Numbering::parse_spec`], but never an error: an unreadable pattern (old data, a template with
    /// `numbering: "Kapitel"`) counts as before as a Typst pattern for all levels.
    pub fn lenient(spec: &str) -> Numbering {
        Numbering::parse_spec(spec).unwrap_or_else(|_| Numbering {
            kind: Kind::Spec(Spec::Hier { pattern: Pattern::raw(spec.trim()), depth: None }),
            parts: Parts::Roman,
        })
    }

    /// Numbering of a scheme. `custom` is the own pattern (only for [`Scheme::Custom`]), `template` the work
    /// template (only for [`Scheme::Template`]: its `#set heading(numbering: …)`, otherwise „1.1“).
    pub fn of_scheme(scheme: Scheme, custom: Option<&str>, template: &str) -> Result<Numbering, String> {
        let named = |kind, parts| Ok(Numbering { kind, parts });
        match scheme {
            Scheme::Template => {
                Ok(Numbering::from_template(template).unwrap_or_else(|| Numbering::lenient(DEFAULT_PATTERN)))
            }
            Scheme::Decimal => Numbering::parse_spec("1.1"),
            Scheme::DecimalDot => Numbering::parse_spec("1.1."),
            Scheme::Alphanumeric => Numbering::parse_spec("I.;A.;1.;a);aa)"),
            Scheme::ChapterRoman => Numbering::parse_spec("I|1"),
            Scheme::ChapterArabic => Numbering::parse_spec("1|1"),
            Scheme::PartsChapters => named(Kind::PartsChapters, Parts::Named),
            Scheme::ChapterWords => named(Kind::ChapterWords, Parts::Words),
            Scheme::Legal => named(Kind::Legal, Parts::Hidden),
            Scheme::Sermon => named(Kind::Sermon, Parts::Hidden),
            Scheme::Book => named(Kind::Book, Parts::Words),
            Scheme::None => Ok(Numbering::off()),
            Scheme::Custom => match custom.map(str::trim).filter(|c| !c.is_empty()) {
                Some(spec) => Numbering::parse_spec(spec),
                None => Err("Für „Eigenes Muster“ fehlt das Muster, z. B. „I.;A.;1.“ oder „I|1“.".to_owned()),
            },
        }
    }

    /// Numbering from the work template (contract Round 14, section 3): the last `#set heading(numbering: …)` of
    /// the top level (like [`crate::heading_numbering`]). A string is a literal Typst pattern, `none` means no
    /// numbers, the rule of a scheme with words generated by Baruch yields exactly this scheme (including the
    /// counting of the parts), a function that can be evaluated is evaluated exactly per level (see module
    /// description), otherwise the approximation applies (first `numbering` pattern, as before). Without a rule or
    /// with an unreadable value `None`.
    pub fn from_template(template: &str) -> Option<Numbering> {
        const NEEDLE: &str = "#set heading(";
        let blanked = blank_comments(template);
        let tops = top_level_hashes(template);
        // Like `heading_numbering` (and Typst): the last rule with `numbering` applies.
        let mut found: Option<(Arg, usize, usize)> = None;
        for &rule in &tops {
            if !blanked[rule..].starts_with(NEEDLE) {
                continue;
            }
            let start = rule + NEEDLE.len();
            let mut cursor = Cursor { s: &blanked[start..], i: 0 };
            for (name, value, at) in cursor.arguments_at() {
                if name == "numbering" {
                    found = Some((value, rule, start + at));
                }
            }
        }
        let (value, rule, at) = found?;
        let roman = |kind| Numbering { kind, parts: Parts::Roman };
        match value {
            Arg::Str(pattern) => {
                return Some(roman(Kind::Spec(Spec::Hier { pattern: Pattern::literal(&pattern), depth: None })));
            }
            Arg::Other(text) if text.trim() == "none" => return Some(Numbering::off()),
            _ => {}
        }
        let rest = &blanked[at..];
        let expression = rest[..expression_end(rest)].trim();
        for scheme in [Scheme::PartsChapters, Scheme::ChapterWords, Scheme::Legal, Scheme::Sermon, Scheme::Book] {
            let numbering = Numbering::of_scheme(scheme, None, "").ok()?;
            if numbering.rule_value() == expression {
                return Some(numbering);
            }
        }
        let approx = crate::heading_numbering(template);
        if let Some(node) = evaluable_function(&blanked, &tops, rule, expression) {
            return Some(roman(Kind::Function { node, approx }));
        }
        approx.map(|spec| Numbering::lenient(&spec))
    }

    /// The numbering in the pattern language, if it has one (`""` = no numbers); the schemes with words have none.
    /// For a function of the work template evaluated per level the previous approximation
    /// ([`crate::heading_numbering`]), so that display and outline header stay the same.
    pub fn spec(&self) -> Option<String> {
        if let Kind::Function { approx, .. } = &self.kind {
            return approx.clone();
        }
        let Kind::Spec(spec) = &self.kind else { return None };
        Some(match spec {
            Spec::Off => String::new(),
            Spec::Hier { pattern, depth: None } => pattern.source.clone(),
            Spec::Hier { pattern, depth: Some(depth) } => format!("{}|{depth}", pattern.source),
            Spec::Levels(levels) => {
                levels.iter().map(|l| l.as_ref().map_or("", |p| p.source.as_str())).collect::<Vec<_>>().join(";")
            }
        })
    }

    /// Are there numbers at all?
    pub fn is_off(&self) -> bool {
        self.kind == Kind::Spec(Spec::Off)
    }

    /// Number of the n-th part (from 1): „I“, „Teil I“, „Erster Teil“ or empty.
    pub fn part(&self, n: usize) -> String {
        match self.parts {
            Parts::Roman => crate::roman(n),
            Parts::Named => format!("Teil {}", crate::roman(n)),
            Parts::Words => ordinal_word(n, "er", "Teil"),
            Parts::Hidden => String::new(),
        }
    }

    /// Number of a heading: `path` = counter values from level 1 to the own one (chapters consecutive, below them
    /// per parent), `chapters` = number of the numbered chapters of the work (for „Schluss“ of the sermon and of
    /// the book).
    pub fn heading(&self, path: &[usize], chapters: usize) -> String {
        let level = path.len();
        let Some(&own) = path.last() else { return String::new() };
        match &self.kind {
            Kind::Spec(Spec::Off) => String::new(),
            Kind::Spec(Spec::Hier { pattern, depth }) => {
                if depth.is_some_and(|d| level > usize::from(d)) {
                    String::new()
                } else {
                    pattern.format(path)
                }
            }
            Kind::Spec(Spec::Levels(levels)) => match levels.get(level - 1) {
                Some(Some(pattern)) if pattern.counters() == 1 => pattern.format(&[own]),
                Some(Some(pattern)) => pattern.format(path),
                _ => String::new(),
            },
            Kind::PartsChapters if level == 1 => format!("Kapitel {own}"),
            Kind::PartsChapters => crate::format_number("1.1", path),
            Kind::ChapterWords if level == 1 => ordinal_word(own, "es", "Kapitel"),
            Kind::ChapterWords => String::new(),
            Kind::Legal => match level {
                1 => format!("§ {own}"),
                2 => format!("Abs. {own}"),
                3 => format!("Nr. {own}"),
                _ => String::new(),
            },
            Kind::Sermon | Kind::Book if level == 1 => {
                if own == 1 {
                    "Einleitung".to_owned()
                } else if own == chapters {
                    "Schluss".to_owned()
                } else {
                    format!("{}.", crate::roman(own - 1))
                }
            }
            Kind::Sermon | Kind::Book => String::new(),
            Kind::Function { node, .. } => render(&node.eval(level), path).unwrap_or_default(),
        }
    }

    /// Rule for Typst as one line `#set heading(numbering: …)` with line end.
    pub fn heading_rule(&self) -> String {
        format!("#set heading(numbering: {})\n", self.rule_value())
    }

    fn rule_value(&self) -> String {
        const HEAD: &str = "(..n) => { let p = n.pos(); ";
        match &self.kind {
            Kind::Spec(Spec::Off) => "none".to_owned(),
            Kind::Spec(Spec::Hier { pattern, depth }) if !pattern.has_double() => match depth {
                None => typst_str(&pattern.source),
                Some(depth) => format!(
                    "(..n) => if n.pos().len() <= {depth} {{ numbering({}, ..n) }} else {{ \"\" }}",
                    typst_str(&pattern.source)
                ),
            },
            Kind::Spec(Spec::Hier { pattern, depth }) => {
                let last = depth.map_or(MAX_LEVEL, usize::from);
                let levels: Vec<String> = (1..=last).map(|level| pattern.typst(level, false)).collect();
                format!("{HEAD}{} }}", chain(&levels))
            }
            Kind::Spec(Spec::Levels(levels)) => {
                let levels: Vec<String> = levels
                    .iter()
                    .enumerate()
                    .map(|(i, level)| match level {
                        Some(pattern) => pattern.typst(i + 1, pattern.counters() == 1),
                        None => "\"\"".to_owned(),
                    })
                    .collect();
                format!("{HEAD}{} }}", chain(&levels))
            }
            Kind::PartsChapters => {
                format!("{HEAD}if p.len() == 1 {{ \"Kapitel \" + str(p.at(0)) }} else {{ numbering(\"1.1\", ..p) }} }}")
            }
            Kind::ChapterWords => {
                let words: Vec<String> = (1..=99).map(|n| typst_str(&ordinal_word(n, "es", ""))).collect();
                format!(
                    "{HEAD}let w = ({}); if p.len() == 1 {{ if p.at(0) <= w.len() {{ w.at(p.at(0) - 1) + \" Kapitel\" }} \
                     else {{ str(p.at(0)) + \". Kapitel\" }} }} else {{ \"\" }} }}",
                    words.join(", ")
                )
            }
            Kind::Legal => format!(
                "{HEAD}if p.len() == 1 {{ \"§ \" + str(p.at(0)) }} else if p.len() == 2 {{ \"Abs. \" + str(p.at(1)) }} \
                 else if p.len() == 3 {{ \"Nr. \" + str(p.at(2)) }} else {{ \"\" }} }}"
            ),
            Kind::Sermon => "(..n) => context { let p = n.pos(); let last = counter(heading).final().at(0); \
                             if p.len() == 1 { if p.at(0) == 1 { \"Einleitung\" } else if p.at(0) == last { \"Schluss\" } \
                             else { numbering(\"I.\", p.at(0) - 1) } } else { \"\" } }"
                .to_owned(),
            // The same numbers as Sermon, but deliberately different text (also without comments): only this way
            // `from_template` recognises an exported book state again as a book including word parts.
            Kind::Book => "(..n) => context { let p = n.pos(); let chapters = counter(heading).final().first(); \
                           if p.len() != 1 { \"\" } else if p.first() == 1 { \"Einleitung\" } \
                           else if p.first() == chapters { \"Schluss\" } else { numbering(\"I.\", p.first() - 1) } }"
                .to_owned(),
            Kind::Function { node, .. } => {
                let levels: Vec<String> = (1..=MAX_LEVEL).map(|level| terms_typst(&node.eval(level))).collect();
                format!("{HEAD}{} }}", chain(&levels))
            }
        }
    }
}

/// `if p.len() == 1 { E1 } else if p.len() == 2 { E2 } … else { "" }`.
fn chain(levels: &[String]) -> String {
    let mut out = String::new();
    for (i, expression) in levels.iter().enumerate() {
        if i > 0 {
            out.push_str(" else ");
        }
        out.push_str(&format!("if p.len() == {} {{ {expression} }}", i + 1));
    }
    out.push_str(" else { \"\" }");
    out
}

/// Template for export and mirror: unchanged for [`Scheme::Template`], otherwise with the comment `// Baruch:
/// Gliederungsschema „…“` and [`Numbering::heading_rule`] at the end (before it a line end if the template ends
/// without one).
pub fn export_template(template: &str, scheme: Scheme, numbering: &Numbering) -> String {
    if scheme == Scheme::Template {
        return template.to_owned();
    }
    let mut out = template.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("// Baruch: Gliederungsschema „{}“\n", scheme.label()));
    out.push_str(&numbering.heading_rule());
    out
}

// ---- Ordinal number words -----------------------------------------------------------------------

/// Stem of the German ordinal number (1 → „erst“, 21 → „einundzwanzigst“) up to 99.
fn ordinal_stem(n: usize) -> Option<String> {
    const SMALL: [&str; 19] = [
        "erst",
        "zweit",
        "dritt",
        "viert",
        "fünft",
        "sechst",
        "siebt",
        "acht",
        "neunt",
        "zehnt",
        "elft",
        "zwölft",
        "dreizehnt",
        "vierzehnt",
        "fünfzehnt",
        "sechzehnt",
        "siebzehnt",
        "achtzehnt",
        "neunzehnt",
    ];
    const UNITS: [&str; 10] = ["", "ein", "zwei", "drei", "vier", "fünf", "sechs", "sieben", "acht", "neun"];
    const TENS: [&str; 10] =
        ["", "", "zwanzig", "dreißig", "vierzig", "fünfzig", "sechzig", "siebzig", "achtzig", "neunzig"];
    match n {
        1..=19 => Some(SMALL[n - 1].to_owned()),
        20..=99 => {
            let (tens, units) = (n / 10, n % 10);
            let cardinal =
                if units == 0 { TENS[tens].to_owned() } else { format!("{}und{}", UNITS[units], TENS[tens]) };
            Some(format!("{cardinal}st"))
        }
        _ => None,
    }
}

/// „Erstes Kapitel“, „Zweiter Teil“; from 100 on (and for 0) with digits: „100. Kapitel“. Without `noun` only the
/// word („Erstes“).
pub fn ordinal_word(n: usize, ending: &str, noun: &str) -> String {
    let word = match ordinal_stem(n) {
        Some(stem) => {
            let mut chars = stem.chars();
            let first = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
            format!("{first}{}{ending}", chars.as_str())
        }
        None => format!("{n}."),
    };
    if noun.is_empty() { word } else { format!("{word} {noun}") }
}

// ---- Reading functions of the work template ---------------------------------------------------

/// Result of an expression at one level: pieces of a concatenation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Term {
    /// Text (`"…"`; `none` and `[]` are empty text).
    Text(String),
    /// `numbering("P", ..n)`: hierarchical.
    Hier(String),
    /// `numbering("P", p.at(i))`, times `times`.
    Num { pattern: String, index: Index, times: usize },
    /// `str(p.at(i))` (Arabic digits), times `times`.
    Str { index: Index, times: usize },
}

/// Position in the counter list: `p.at(i)` or `p.first()` or `p.last()` (the own number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Index {
    At(usize),
    Last,
}

impl Index {
    fn resolve(self, path: &[usize]) -> Option<usize> {
        match self {
            Index::At(i) => path.get(i).copied(),
            Index::Last => path.last().copied(),
        }
    }

    fn typst(self) -> String {
        match self {
            Index::At(i) => format!("p.at({i})"),
            Index::Last => "p.last()".to_owned(),
        }
    }
}

/// Pieces of a level to the number for the counter values `path` (like Typst: pattern literal, without doubled
/// letters). `None` if a position is missing (Typst would abort with an error).
fn render(terms: &[Term], path: &[usize]) -> Option<String> {
    let mut out = String::new();
    for term in terms {
        match term {
            Term::Text(text) => out.push_str(text),
            Term::Hier(pattern) => out.push_str(&format_number(pattern, path)),
            Term::Num { pattern, index, times } => {
                out.push_str(&format_number(pattern, &[index.resolve(path)?]).repeat(*times));
            }
            Term::Str { index, times } => out.push_str(&index.resolve(path)?.to_string().repeat(*times)),
        }
    }
    Some(out)
}

/// Typst expression for the pieces of a level (counter list `p`); empty → `""`.
fn terms_typst(terms: &[Term]) -> String {
    let times = |expression: String, times: usize| {
        if times == 1 { expression } else { format!("{expression} * {times}") }
    };
    let parts: Vec<String> = terms
        .iter()
        .map(|term| match term {
            Term::Text(text) => typst_str(text),
            Term::Hier(pattern) => format!("numbering({}, ..p)", typst_str(pattern)),
            Term::Num { pattern, index, times: n } => {
                times(format!("numbering({}, {})", typst_str(pattern), index.typst()), *n)
            }
            Term::Str { index, times: n } => times(format!("str({})", index.typst()), *n),
        })
        .collect();
    if parts.is_empty() { "\"\"".to_owned() } else { parts.join(" + ") }
}

/// All patterns in an expression (for the check whether Typst accepts them).
fn patterns(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::If { then, other, .. } => {
            patterns(then, out);
            if let Some(other) = other {
                patterns(other, out);
            }
        }
        Node::Value(terms) => out.extend(terms.iter().filter_map(|t| match t {
            Term::Hier(p) | Term::Num { pattern: p, .. } => Some(p.clone()),
            _ => None,
        })),
    }
}

/// Can the expression be evaluated at every level 1 to 6 (every pattern with a counting symbol, every position
/// present)? Otherwise it cannot be read exactly.
fn evaluable(node: &Node) -> bool {
    let mut found = Vec::new();
    patterns(node, &mut found);
    found.iter().all(|p| is_numbering_pattern(p))
        && (1..=MAX_LEVEL).all(|level| render(&node.eval(level), &vec![1; level]).is_some())
}

/// A numbering function that can be evaluated per level: directly in the rule (`(..n) => …`) or as the name of a
/// `#let` function that stands at the top level before the rule (`rule`) (the last one applies). The whole
/// expression must be read (afterwards only whitespace or `;`).
fn evaluable_function(blanked: &str, tops: &[usize], rule: usize, expression: &str) -> Option<Node> {
    let whole = |reader: &Reader, node: Node| {
        (reader.rest().trim_start_matches(|c: char| c.is_whitespace() || c == ';').is_empty() && evaluable(&node))
            .then_some(node)
    };
    let mut reader = Reader::new(expression);
    if let Some(node) = reader.function() {
        return whole(&reader, node);
    }
    let is_name = !expression.is_empty()
        && expression.bytes().all(ident_char)
        && expression.bytes().next().is_some_and(ident_start);
    if !is_name {
        return None;
    }
    let needle = format!("#let {expression}");
    let (index, &at) = tops.iter().enumerate().rev().find(|&(_, &at)| {
        at < rule
            && blanked[at..].starts_with(&needle)
            && blanked[at + needle.len()..].trim_start_matches([' ', '\t']).starts_with(['(', '='])
    })?;
    let end = tops.get(index + 1).copied().unwrap_or(blanked.len());
    let body = blanked[at + needle.len()..end].trim_start();
    let mut reader = Reader::new(body);
    let node = match body.strip_prefix('=') {
        Some(_) => {
            reader.eat("=");
            reader.function()?
        }
        None => {
            reader.params()?;
            if !reader.eat("=") {
                return None;
            }
            reader.expression()?
        }
    };
    whole(&reader, node)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Le,
    Lt,
    Ge,
    Gt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    If { op: Op, k: usize, then: Box<Node>, other: Option<Box<Node>> },
    Value(Vec<Term>),
}

impl Node {
    fn eval(&self, level: usize) -> Vec<Term> {
        match self {
            Node::If { op, k, then, other } => {
                let k = *k;
                let holds = match op {
                    Op::Eq => level == k,
                    Op::Ne => level != k,
                    Op::Le => level <= k,
                    Op::Lt => level < k,
                    Op::Ge => level >= k,
                    Op::Gt => level > k,
                };
                if holds { then.eval(level) } else { other.as_ref().map(|o| o.eval(level)).unwrap_or_default() }
            }
            Node::Value(terms) => terms.clone(),
        }
    }
}

/// Small reader for numbering functions (text without comments).
struct Reader<'a> {
    s: &'a str,
    i: usize,
    /// Names that denote the counter list: the parameter (as `n.pos()`) and `let p = n.pos()`.
    param: String,
    aliases: Vec<String>,
}

impl<'a> Reader<'a> {
    fn new(s: &'a str) -> Reader<'a> {
        Reader { s, i: 0, param: String::new(), aliases: Vec::new() }
    }

    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn ws(&mut self) {
        let rest = self.rest();
        self.i += rest.len() - rest.trim_start().len();
    }

    fn eat(&mut self, token: &str) -> bool {
        self.ws();
        if self.rest().starts_with(token) {
            self.i += token.len();
            true
        } else {
            false
        }
    }

    fn keyword(&mut self, word: &str) -> bool {
        self.ws();
        let rest = self.rest();
        if rest.starts_with(word) && !rest.as_bytes().get(word.len()).is_some_and(|&b| ident_char(b)) {
            self.i += word.len();
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Option<&'a str> {
        self.ws();
        let bytes = self.rest().as_bytes();
        if !bytes.first().is_some_and(|&b| ident_start(b)) {
            return None;
        }
        let len = bytes.iter().take_while(|&&b| ident_char(b)).count();
        let name = &self.rest()[..len];
        self.i += len;
        Some(name)
    }

    fn int(&mut self) -> Option<usize> {
        self.ws();
        let len = self.rest().bytes().take_while(u8::is_ascii_digit).count();
        let value = self.rest()[..len].parse().ok()?;
        self.i += len;
        Some(value)
    }

    fn string(&mut self) -> Option<String> {
        if !self.eat("\"") {
            return None;
        }
        let mut cursor = Cursor { s: self.rest(), i: 0 };
        let text = cursor.string();
        self.i += cursor.i;
        Some(text)
    }

    /// `(..n)` → name of the parameter.
    fn params(&mut self) -> Option<()> {
        if !(self.eat("(") && self.eat("..")) {
            return None;
        }
        self.param = self.ident()?.to_owned();
        self.eat(",");
        self.eat(")").then_some(())
    }

    /// The counter list: `n.pos()` or a name from `let p = n.pos()`.
    fn positions(&mut self) -> Option<()> {
        let name = self.ident()?;
        if name == self.param {
            (self.eat(".pos") && self.eat("(") && self.eat(")")).then_some(())
        } else {
            self.aliases.iter().any(|a| a == name).then_some(())
        }
    }

    fn function(&mut self) -> Option<Node> {
        self.params()?;
        if !self.eat("=>") {
            return None;
        }
        self.expression()
    }

    fn expression(&mut self) -> Option<Node> {
        if self.keyword("if") {
            self.positions()?;
            if !(self.eat(".len") && self.eat("(") && self.eat(")")) {
                return None;
            }
            let op = [("==", Op::Eq), ("!=", Op::Ne), ("<=", Op::Le), (">=", Op::Ge), ("<", Op::Lt), (">", Op::Gt)]
                .into_iter()
                .find(|(token, _)| self.eat(token))
                .map(|(_, op)| op)?;
            let k = self.int()?;
            let then = Box::new(self.branch()?);
            let other = if self.keyword("else") {
                self.ws();
                Some(Box::new(if self.rest().starts_with("if") { self.expression()? } else { self.branch()? }))
            } else {
                None
            };
            return Some(Node::If { op, k, then, other });
        }
        self.ws();
        if self.rest().starts_with('{') {
            return self.block();
        }
        self.concat().map(Node::Value)
    }

    /// Branch of an `if`: `{ … }` or an empty content block `[]`.
    fn branch(&mut self) -> Option<Node> {
        self.ws();
        if self.rest().starts_with('{') {
            return self.block();
        }
        (self.eat("[") && self.eat("]")).then(|| Node::Value(Vec::new()))
    }

    /// `{ let p = n.pos(); VALUE }` – exactly one value, before it any number of names for the counter list.
    fn block(&mut self) -> Option<Node> {
        if !self.eat("{") {
            return None;
        }
        let mut value = None;
        loop {
            if self.eat("}") {
                return value;
            }
            if self.keyword("let") {
                let name = self.ident()?.to_owned();
                if !self.eat("=") {
                    return None;
                }
                self.positions()?;
                self.aliases.push(name);
            } else if value.is_none() {
                value = Some(self.expression()?);
            } else {
                return None;
            }
            self.eat(";");
        }
    }

    /// Concatenation `a + b * 2 + …` of text, `none`, `[]` and `numbering(…)` calls.
    fn concat(&mut self) -> Option<Vec<Term>> {
        let mut terms = Vec::new();
        loop {
            let mut term = self.term()?;
            if self.eat("*") {
                let times = self.int().filter(|&t| t <= MAX_REPEAT)?;
                let product = |before: usize| before.checked_mul(times).filter(|&t| t <= MAX_REPEAT);
                term = match term {
                    Term::Text(t) => Term::Text(t.repeat(times)),
                    Term::Num { pattern, index, times: before } => {
                        Term::Num { pattern, index, times: product(before)? }
                    }
                    Term::Str { index, times: before } => Term::Str { index, times: product(before)? },
                    Term::Hier(_) => return None,
                };
            }
            terms.push(term);
            if !self.eat("+") {
                return Some(terms);
            }
        }
    }

    fn term(&mut self) -> Option<Term> {
        self.ws();
        if self.rest().starts_with('"') {
            return self.string().map(Term::Text);
        }
        if self.keyword("none") {
            return Some(Term::Text(String::new()));
        }
        if self.rest().starts_with('[') {
            return (self.eat("[") && self.eat("]")).then(|| Term::Text(String::new()));
        }
        if self.keyword("str") {
            // `str(p.at(i))`: the number in Arabic digits.
            if !self.eat("(") {
                return None;
            }
            self.positions()?;
            let index = self.index()?;
            self.eat(",");
            return self.eat(")").then_some(Term::Str { index, times: 1 });
        }
        if !(self.keyword("numbering") && self.eat("(")) {
            return None;
        }
        let pattern = self.string()?;
        if !self.eat(",") {
            return None;
        }
        let term = if self.eat("..") {
            // `..n` (the arguments themselves), `..n.pos()` or `..p` with `let p = n.pos()`.
            let before = self.i;
            if self.ident() != Some(self.param.as_str()) || self.rest().trim_start().starts_with(".pos") {
                self.i = before;
                self.positions()?;
            }
            Term::Hier(pattern)
        } else {
            self.positions()?;
            Term::Num { pattern, index: self.index()?, times: 1 }
        };
        self.eat(",");
        self.eat(")").then_some(term)
    }

    /// `.at(i)`, `.first()` or `.last()` behind the counter list.
    fn index(&mut self) -> Option<Index> {
        if self.eat(".at") && self.eat("(") {
            let index = self.int()?;
            self.eat(")").then_some(Index::At(index))
        } else if self.eat(".first") && self.eat("(") && self.eat(")") {
            Some(Index::At(0))
        } else if self.eat(".last") && self.eat("(") && self.eat(")") {
            Some(Index::Last)
        } else {
            None
        }
    }
}

/// What a level shows.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shown {
    Empty,
    Hier(String),
    /// Pattern of the pattern language assembled from text and numbers.
    Built(String),
}

/// Pieces of a level `level` to a pattern: text must not contain counting symbols, numbers are either only the own
/// one (`p.at(level - 1)`) or all from 0 to `level - 1` in order.
fn shown(terms: Vec<Term>, level: usize) -> Option<Shown> {
    let terms: Vec<Term> = terms.into_iter().filter(|t| !matches!(t, Term::Text(s) if s.is_empty())).collect();
    match terms.as_slice() {
        [] => return Some(Shown::Empty),
        [Term::Hier(pattern)] => return Some(Shown::Hier(pattern.clone())),
        _ => {}
    }
    let mut out = String::new();
    let mut indices = Vec::new();
    for term in terms {
        match term {
            Term::Text(text) => {
                if text.chars().any(|c| Counter::of(c).is_some() || c == ';' || c == '|') {
                    return None;
                }
                out.push_str(&text);
            }
            Term::Num { pattern, index, times } => {
                // The approximation only knows `p.at(i)`/`p.first()` (as before Round 14).
                let Index::At(index) = index else { return None };
                let (pieces, _) = pattern_pieces(&pattern)?;
                if pieces.len() != 1 || pattern.contains([';', '|']) {
                    return None;
                }
                match times {
                    1 => out.push_str(&pattern),
                    2 => {
                        let mark = Pattern::raw(&pattern);
                        let single = pattern.chars().count() == 1;
                        let letter = matches!(mark.pieces.first(), Some((_, Mark::One(c))) if matches!(c, Counter::LowerLatin | Counter::UpperLatin));
                        if !(single && letter) {
                            return None;
                        }
                        out.push_str(&pattern.repeat(2));
                    }
                    _ => return None,
                }
                indices.push(index);
            }
            Term::Hier(_) | Term::Str { .. } => return None,
        }
    }
    let own = indices == [level - 1];
    let all = indices.iter().copied().eq(0..level);
    (own || all).then_some(Shown::Built(out))
}

/// Pattern language from a numbering function `(..n) => …` (or the body of a `#let` function with parameter
/// `param`): evaluated per level 1 to 6. `None` if the function does more than depth conditions (`n.pos().len() ==
/// k`, `<= k`, `< k` …), text and `numbering(…)`.
pub(crate) fn read_function(expression: &str) -> Option<String> {
    let mut reader = Reader::new(expression);
    let node = reader.function()?;
    spec_of(&node)
}

/// Like [`read_function`] for `#let name(..n) = BODY`: `definition` starts with `(..n)`.
pub(crate) fn read_let_function(definition: &str) -> Option<String> {
    let mut reader = Reader::new(definition);
    reader.params()?;
    if !reader.eat("=") {
        return None;
    }
    let node = reader.expression()?;
    spec_of(&node)
}

fn spec_of(node: &Node) -> Option<String> {
    let levels: Vec<Shown> = (1..=MAX_LEVEL).map(|level| shown(node.eval(level), level)).collect::<Option<_>>()?;
    // How many levels from the top show the same hierarchical pattern, and are all after them empty?
    let depth = levels.iter().take_while(|l| *l == &levels[0]).count();
    let leading = match &levels[0] {
        Shown::Hier(pattern) if levels[depth..].iter().all(|l| *l == Shown::Empty) => Some(pattern),
        _ => None,
    };
    let spec = if levels.iter().all(|l| *l == Shown::Empty) {
        String::new()
    } else if let Some(pattern) = leading {
        if depth == MAX_LEVEL { pattern.clone() } else { format!("{pattern}|{depth}") }
    } else {
        let last = levels.iter().rposition(|l| *l != Shown::Empty)?;
        let items: Vec<&str> = levels[..=last]
            .iter()
            .map(|l| match l {
                Shown::Empty => "",
                Shown::Hier(p) | Shown::Built(p) => p.as_str(),
            })
            .collect();
        if last == 0 { format!("{}|1", items[0]) } else { items.join(";") }
    };
    Numbering::parse_spec(&spec).ok().map(|_| spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubled_letters_count_as_one_counter() {
        let pattern = Pattern::parse("aa)").unwrap();
        assert_eq!(pattern.counters(), 1, "aa ist ein Zählzeichen");
        assert_eq!(pattern.format(&[2]), "bb)", "verdoppelter Buchstabe");
        assert_eq!(pattern.format(&[27]), "aaaa)", "nach z wie Typst „aa“, verdoppelt");
        assert_eq!(Pattern::parse("1.1").unwrap().format(&[1, 2, 3]), "1.2.3", "ohne Verdopplung wie format_number");
    }

    #[test]
    fn every_word_scheme_has_its_own_rule_even_without_comments() {
        // `from_template` compares the rule literally after `blank_comments`: two equal rules would turn an
        // exported book into a sermon (without counting of parts).
        let schemes = [Scheme::PartsChapters, Scheme::ChapterWords, Scheme::Legal, Scheme::Sermon, Scheme::Book];
        let rules: Vec<String> = schemes
            .iter()
            .map(|s| blank_comments(&Numbering::of_scheme(*s, None, "").unwrap().rule_value()).trim().to_owned())
            .collect();
        for (i, a) in rules.iter().enumerate() {
            for (j, b) in rules.iter().enumerate().skip(i + 1) {
                assert_ne!(a, b, "{:?} und {:?} haben dieselbe Regel", schemes[i], schemes[j]);
            }
        }
    }

    #[test]
    fn ordinals_are_german_words_up_to_ninety_nine() {
        assert_eq!(ordinal_word(1, "es", "Kapitel"), "Erstes Kapitel", "1");
        assert_eq!(ordinal_word(3, "er", "Teil"), "Dritter Teil", "3, männlich");
        assert_eq!(ordinal_word(7, "es", ""), "Siebtes", "7");
        assert_eq!(ordinal_word(20, "es", ""), "Zwanzigstes", "20");
        assert_eq!(ordinal_word(21, "es", ""), "Einundzwanzigstes", "21");
        assert_eq!(ordinal_word(99, "es", ""), "Neunundneunzigstes", "99");
        assert_eq!(ordinal_word(100, "es", "Kapitel"), "100. Kapitel", "ab 100 Ziffern");
    }
}
