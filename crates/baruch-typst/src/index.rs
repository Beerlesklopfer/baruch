// SPDX-License-Identifier: AGPL-3.0-or-later
//! Register und Glossar (Runde 31, Vertrag §2.1; Schnittstelle R31-S1, Typen und Signaturen **eingefroren** –
//! Änderungen nur über Integrator/PM; Körper gehören der Aufgabe register-satz).
//!
//! - **Registermarke** im Typst-Text, nur Standard-Typst (kein Paket, kein Netz, in jeder Spiegeldatei setzbar):
//!   `#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>` – Schlüssel nur in der Reihenfolge register,
//!   term, sub, see, also, sort; leere weglassen; Typst-String-Literale mit `\"`, `\\`, `\n`, `\t`; kein Leerzeichen vor
//!   dem Label; die Marke steht direkt hinter dem markierten Wort ([`mark_source`], [`parse_mark`]).
//! - **Register:** eingebaut `stichwort` (Stichwortregister), `personen` (Personenregister), `bibel`
//!   (Bibelstellenregister, automatisch aus erkannten Stellen); eigene Kennungen `^[a-z][a-z0-9-]{0,31}$`, höchstens
//!   [`MAX_CUSTOM_REGISTERS`].
//! - **Schlüssel:** [`term_key`] steht in der Datenbank und ist darum ab S1 unveränderlich (NFC, Leerraum zu einem
//!   Leerzeichen, getrimmt, klein); [`sort_key`] wird nie gespeichert.
//! - **Projektion** ([`Projector`], [`back_matter`]): nur Export, Export-Karte, Spiegel, Satz, PDF und EPUB; die Texte
//!   in SQLite bleiben unverändert. Ohne aktive Einstellungen ([`BackMatter::is_active`]) ist die Projektion die
//!   Identität und das Buchende leer – der Export bleibt byte-gleich zum Stand vor Runde 31.

mod back;
mod project;
mod scan;

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::typeset::TypesetSettings;

/// Label jeder Registermarke.
pub const INDEX_LABEL: &str = "baruch-idx";
/// Vorsilbe der Labels der Glossareinträge (`<baruch-gl-UUID>`).
pub const GLOSSARY_ANCHOR_PREFIX: &str = "baruch-gl-";
/// Erste Zeile des Buchendes (Registerseiten und Glossarseite), das [`back_matter`] liefert.
pub const BACK_MATTER_MARK: &str = "// Baruch: Register und Glossar";
/// Kennung des eingebauten Stichwortregisters.
pub const KEYWORD_REGISTER: &str = "stichwort";
/// Kennung des eingebauten Personenregisters.
pub const PERSON_REGISTER: &str = "personen";
/// Kennung des eingebauten Bibelstellenregisters (automatisch aus erkannten Stellen).
pub const SCRIPTURE_REGISTER: &str = "bibel";
/// Längstes Feld einer Marke (Zeichen).
pub const MAX_MARK_FIELD_CHARS: usize = 200;
/// Längster Titel eines eigenen Registers (Zeichen).
pub const MAX_REGISTER_TITLE_CHARS: usize = 80;
/// Höchstzahl eigener Register je Werk.
pub const MAX_CUSTOM_REGISTERS: usize = 13;

/// Eine Registermarke: Register, Stichwort und Zusätze. Leere Zusätze erscheinen weder in JSON noch in Typst.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexMark {
    /// Kennung des Registers ([`KEYWORD_REGISTER`], [`PERSON_REGISTER`], [`SCRIPTURE_REGISTER`] oder eigene).
    pub register: String,
    /// Stichwort.
    pub term: String,
    /// Unterstichwort (eingerückt unter `term`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sub: String,
    /// „siehe“: Eintrag ohne Seitenzahl, verweist auf dieses Stichwort.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub see: String,
    /// „siehe auch“.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub also: String,
    /// Sortierschlüssel, nur für automatische Einträge (Bibel: biblische Buchreihenfolge).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sort: String,
}

impl IndexMark {
    /// Marke mit Register und Stichwort, ohne Zusätze.
    pub fn new(register: &str, term: &str) -> Self {
        IndexMark { register: register.to_owned(), term: term.to_owned(), ..IndexMark::default() }
    }

    /// Prüft die Marke: gültige Register-Kennung ([`valid_register_slug`]), Stichwort nicht leer, jedes Feld höchstens
    /// [`MAX_MARK_FIELD_CHARS`] Zeichen und ohne Steuerzeichen (Zeilenumbruch, Tabulator …).
    pub fn validate(&self) -> Result<(), IndexError> {
        if !valid_register_slug(&self.register) {
            return Err(IndexError::Invalid(format!(
                "Register „{}“: keine gültige Kennung (Kleinbuchstabe am Anfang, dann Kleinbuchstaben, Ziffern oder \
                 Bindestrich, höchstens 32 Zeichen).",
                self.register
            )));
        }
        if self.term.trim().is_empty() {
            return Err(IndexError::Invalid("Stichwort: darf nicht leer sein.".into()));
        }
        let fields = [
            ("Stichwort", &self.term),
            ("Unterstichwort", &self.sub),
            ("siehe", &self.see),
            ("siehe auch", &self.also),
            ("Sortierschlüssel", &self.sort),
        ];
        for (label, value) in fields {
            if value.chars().count() > MAX_MARK_FIELD_CHARS {
                return Err(IndexError::Invalid(format!("{label}: höchstens {MAX_MARK_FIELD_CHARS} Zeichen.")));
            }
            if value.chars().any(char::is_control) {
                return Err(IndexError::Invalid(format!("{label}: ohne Zeilenumbruch und Steuerzeichen.")));
            }
        }
        Ok(())
    }

    /// Die Felder in Typst-Reihenfolge mit ihren Schlüsseln.
    fn fields(&self) -> [(&'static str, &String); 6] {
        [
            ("register", &self.register),
            ("term", &self.term),
            ("sub", &self.sub),
            ("see", &self.see),
            ("also", &self.also),
            ("sort", &self.sort),
        ]
    }
}

/// Typst-String-Literal: `\"`, `\\`, `\n`, `\t`, `\r`; übrige Steuerzeichen als `\u{…}`.
fn typst_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Kanonische Typst-Form der Marke: `#metadata((register: "…", term: "…"[, sub: "…"]…))<baruch-idx>`.
pub fn mark_source(mark: &IndexMark) -> String {
    let mut out = String::from("#metadata((");
    let mut first = true;
    for (key, value) in mark.fields() {
        // register und term stehen immer da (auch leer), damit die Marke ein Wörterbuch bleibt.
        if value.is_empty() && key != "register" && key != "term" {
            continue;
        }
        if !first {
            out.push_str(", ");
        }
        first = false;
        out.push_str(key);
        out.push_str(": ");
        typst_string(value, &mut out);
    }
    out.push_str("))<");
    out.push_str(INDEX_LABEL);
    out.push('>');
    out
}

/// Kleiner Leser für [`parse_mark`] über Bytes (die Marke selbst ist ASCII bis auf die Inhalte der Strings).
struct Cursor<'s> {
    src: &'s str,
    pos: usize,
}

impl Cursor<'_> {
    fn eat(&mut self, token: &str) -> bool {
        if self.src[self.pos..].starts_with(token) {
            self.pos += token.len();
            true
        } else {
            false
        }
    }

    /// Leerraum innerhalb der Zeile (Leerzeichen und Tabulator).
    fn space(&mut self) {
        while matches!(self.src.as_bytes().get(self.pos), Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }

    fn ident(&mut self) -> Option<&str> {
        let start = self.pos;
        while matches!(self.src.as_bytes().get(self.pos), Some(b'a'..=b'z')) {
            self.pos += 1;
        }
        (self.pos > start).then(|| &self.src[start..self.pos])
    }

    /// Typst-String-Literal ohne rohen Zeilenumbruch; Escapes `\"`, `\\`, `\n`, `\t`, `\r`, `\u{…}`.
    fn string(&mut self) -> Option<String> {
        if !self.eat("\"") {
            return None;
        }
        let mut out = String::new();
        let mut chars = self.src[self.pos..].char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' => {
                    self.pos += i + 1;
                    return Some(out);
                }
                '\n' | '\r' => return None,
                '\\' => match chars.next()?.1 {
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    'u' => {
                        if chars.next()?.1 != '{' {
                            return None;
                        }
                        let mut hex = String::new();
                        loop {
                            let (_, h) = chars.next()?;
                            if h == '}' {
                                break;
                            }
                            if !h.is_ascii_hexdigit() || hex.len() >= 6 {
                                return None;
                            }
                            hex.push(h);
                        }
                        out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    _ => return None,
                },
                c => out.push(c),
            }
        }
        None
    }
}

/// Liest eine kanonische Marke am Anfang von `source` (Leerraum um `:` und `,` sowie innen an den Klammern erlaubt,
/// Schlüssel nur in der Reihenfolge register, term, sub, see, also, sort, register und term Pflicht, kein Leerzeichen
/// vor dem Label). Gibt die Marke und die verbrauchten Bytes zurück; alles andere (auch eine Marke, die
/// [`IndexMark::validate`] nicht besteht) → `None` und bleibt Insel.
pub fn parse_mark(source: &str) -> Option<(IndexMark, usize)> {
    const KEYS: [&str; 6] = ["register", "term", "sub", "see", "also", "sort"];
    let mut cur = Cursor { src: source, pos: 0 };
    if !cur.eat("#metadata((") {
        return None;
    }
    let mut mark = IndexMark::default();
    let mut next_key = 0usize;
    let mut seen = [false; 6];
    cur.space();
    loop {
        if cur.eat(")") {
            break;
        }
        let key = cur.ident()?;
        let index = KEYS.iter().position(|k| *k == key)?;
        if index < next_key {
            return None;
        }
        next_key = index + 1;
        seen[index] = true;
        cur.space();
        if !cur.eat(":") {
            return None;
        }
        cur.space();
        let value = cur.string()?;
        match index {
            0 => mark.register = value,
            1 => mark.term = value,
            2 => mark.sub = value,
            3 => mark.see = value,
            4 => mark.also = value,
            _ => mark.sort = value,
        }
        cur.space();
        if cur.eat(",") {
            cur.space();
            continue;
        }
        if !cur.eat(")") {
            return None;
        }
        break;
    }
    if !(seen[0] && seen[1]) {
        return None;
    }
    if !cur.eat(")") || !cur.eat("<") || !cur.eat(INDEX_LABEL) || !cur.eat(">") {
        return None;
    }
    mark.validate().ok()?;
    Some((mark, cur.pos))
}

/// Welche Fundstelle eines Worts markiert wird: die erste, alle oder die n-te (ab 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum Occurrence {
    First,
    All,
    Nth(u32),
}

impl Occurrence {
    /// `"first"`, `"all"` oder die Zahl als Text.
    pub fn as_param(self) -> String {
        match self {
            Occurrence::First => "first".to_owned(),
            Occurrence::All => "all".to_owned(),
            Occurrence::Nth(n) => n.to_string(),
        }
    }

    /// Kehrt [`Occurrence::as_param`] um; die Zahl muss mindestens 1 sein. Unbekanntes → `None`.
    pub fn from_param(s: &str) -> Option<Self> {
        match s.trim() {
            "first" => Some(Occurrence::First),
            "all" => Some(Occurrence::All),
            n if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                n.parse::<u32>().ok().filter(|n| *n >= 1).map(Occurrence::Nth)
            }
            _ => None,
        }
    }
}

impl From<Occurrence> for String {
    fn from(occurrence: Occurrence) -> String {
        occurrence.as_param()
    }
}

impl TryFrom<String> for Occurrence {
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
        Occurrence::from_param(&value)
            .ok_or_else(|| format!("Fundstelle „{value}“: erwartet „first“, „all“ oder eine Zahl ab 1"))
    }
}

/// Eine Marke im Text mit ihrem Bytebereich (`span.0..span.1` = die ganze Marke).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundMark {
    pub span: (usize, usize),
    pub mark: IndexMark,
}

/// Fehler beim Setzen, Entfernen und Prüfen von Marken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    #[error("{0}")]
    Invalid(String),
    #[error("„{0}“ steht nicht im Abschnitt (außerhalb von Überschriftszeilen, Code, Kommentaren und Marken).")]
    WordNotFound(String),
    #[error(
        "„{0}“ trägt an der gewählten Fundstelle schon diese Registermarke; nichts geändert. Eine andere Fundstelle \
         wählen („alle“ setzt nur die fehlenden Marken)."
    )]
    AlreadyMarked(String),
}

/// Alle kanonischen Marken in `text` (außerhalb von Code, Kommentaren und Raw), in Textreihenfolge. Eine Marke in
/// einem Inhaltsblock (`#emph[Wort#metadata(…)<baruch-idx>]`) zählt mit; eine Marke in einem Kommentar, in Raw oder in
/// einem String nicht (Typst setzt sie dort nicht).
pub fn find_marks(text: &str) -> Vec<FoundMark> {
    scan::scan(text).marks
}

/// Fundstellen von `word` im Fließtext: Anfang und Ende. Das Wort wird wörtlich gesucht (Groß-/Kleinschreibung zählt);
/// Leerraum im Wort passt auf beliebigen Leerraum ohne Leerzeile. Ganze Wörter: Vor und hinter der Fundstelle steht kein
/// Buchstabe und keine Ziffer, wenn das Wort dort mit einem solchen beginnt bzw. endet.
fn find_word(text: &str, scanned: &scan::Scan, word: &str) -> Vec<(usize, usize)> {
    let pieces: Vec<&str> = word.split_whitespace().collect();
    let Some(first) = pieces.first() else { return Vec::new() };
    let starts_alnum = word.trim().chars().next().is_some_and(char::is_alphanumeric);
    let ends_alnum = word.trim().chars().next_back().is_some_and(char::is_alphanumeric);
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(first) {
        let start = from + found;
        from = start + first.len();
        let mut end = start + first.len();
        let mut ok = true;
        for piece in &pieces[1..] {
            let gap = text[end..].len() - text[end..].trim_start().len();
            let between = &text[end..end + gap];
            if gap == 0 || between.matches('\n').count() > 1 || !text[end + gap..].starts_with(piece) {
                ok = false;
                break;
            }
            end += gap + piece.len();
        }
        if !ok || !scanned.prose_range(start, end) {
            continue;
        }
        if starts_alnum && text[..start].chars().next_back().is_some_and(char::is_alphanumeric) {
            continue;
        }
        if ends_alnum && text[end..].chars().next().is_some_and(char::is_alphanumeric) {
            continue;
        }
        out.push((start, end));
    }
    out
}

/// Setzt `mark` direkt hinter die gewählte(n) Fundstelle(n) von `word` (ganze Wörter, nicht in Überschriftszeilen,
/// Code, Kommentaren, Raw oder anderen Marken). Ergebnis: neuer Text und Zahl der gesetzten Marken.
///
/// Steht direkt hinter dem Wort ein Label (`Wort<name>`), kommt die Marke hinter das Label (sonst hinge das Label an
/// der Marke). Ungültige Marke oder leeres Wort → `Invalid`; keine (bzw. keine n-te) Fundstelle → `WordNotFound`.
///
/// Doppelt wird nie markiert: Trägt eine Fundstelle dieselbe Marke schon (in der Kette der Marken direkt hinter dem
/// Wort bzw. Label), bleibt sie, wie sie ist. „erste“ und „n-te“ meinen weiter die erste bzw. n-te Fundstelle im Text;
/// ist gerade sie schon markiert, kommt `AlreadyMarked` (nichts geändert, keine neue Fassung). „alle“ setzt nur die
/// fehlenden Marken; fehlt keine mehr → ebenfalls `AlreadyMarked`.
pub fn mark_word(
    text: &str,
    word: &str,
    occurrence: Occurrence,
    mark: &IndexMark,
) -> Result<(String, usize), IndexError> {
    mark.validate()?;
    let word = word.trim();
    if word.is_empty() {
        return Err(IndexError::Invalid("Wort: darf nicht leer sein.".into()));
    }
    let scanned = scan::scan(text);
    let mut spots: Vec<usize> = Vec::new();
    for (_, end) in find_word(text, &scanned, word) {
        let at = scanned.label_at(end).unwrap_or(end);
        if scanned.insertable(text, at) {
            spots.push(at);
        }
    }
    let chosen: Vec<usize> = match occurrence {
        Occurrence::First => spots.into_iter().take(1).collect(),
        Occurrence::All => spots,
        Occurrence::Nth(n) => spots.into_iter().nth(n.saturating_sub(1) as usize).into_iter().collect(),
    };
    if chosen.is_empty() {
        return Err(IndexError::WordNotFound(word.to_owned()));
    }
    let chosen: Vec<usize> = chosen.into_iter().filter(|&spot| !carries_mark(&scanned.marks, spot, mark)).collect();
    if chosen.is_empty() {
        return Err(IndexError::AlreadyMarked(word.to_owned()));
    }
    let source = mark_source(mark);
    let mut out = String::with_capacity(text.len() + chosen.len() * source.len());
    let mut at = 0;
    for &spot in &chosen {
        out.push_str(&text[at..spot]);
        out.push_str(&source);
        at = spot;
    }
    out.push_str(&text[at..]);
    Ok((out, chosen.len()))
}

/// Steht `mark` schon in der Kette der Marken, die an `spot` beginnt (`Wort#metadata(…)<baruch-idx>#metadata(…)…`)?
fn carries_mark(marks: &[FoundMark], spot: usize, mark: &IndexMark) -> bool {
    let mut at = spot;
    while let Some(found) = marks.iter().find(|m| m.span.0 == at) {
        if found.mark == *mark {
            return true;
        }
        at = found.span.1;
    }
    false
}

/// Setzt `mark` an der Byte-Stelle `byte`; in Überschriftszeilen, Code, Kommentaren, Raw oder einer Marke → `Invalid`.
pub fn insert_mark_at(text: &str, byte: usize, mark: &IndexMark) -> Result<String, IndexError> {
    mark.validate()?;
    if byte > text.len() || !text.is_char_boundary(byte) {
        return Err(IndexError::Invalid(format!("Stelle {byte} liegt nicht im Text bzw. mitten in einem Zeichen.")));
    }
    let scanned = scan::scan(text);
    if !scanned.insertable(text, byte) {
        return Err(IndexError::Invalid(format!(
            "An Stelle {byte} darf keine Registermarke stehen (Überschriftszeile, Code, Kommentar, Raw, Label oder \
             andere Marke)."
        )));
    }
    let source = mark_source(mark);
    let mut out = String::with_capacity(text.len() + source.len());
    out.push_str(&text[..byte]);
    out.push_str(&source);
    out.push_str(&text[byte..]);
    Ok(out)
}

/// Entfernt die Marke genau im Bereich `span` (wie [`FoundMark::span`]); kein Marken-Bereich → `Invalid`.
pub fn remove_mark(text: &str, span: (usize, usize)) -> Result<String, IndexError> {
    if !find_marks(text).iter().any(|m| m.span == span) {
        return Err(IndexError::Invalid(format!(
            "Im Bereich {}–{} steht keine Registermarke (Text inzwischen geändert?).",
            span.0, span.1
        )));
    }
    let mut out = String::with_capacity(text.len() - (span.1 - span.0));
    out.push_str(&text[..span.0]);
    out.push_str(&text[span.1..]);
    Ok(out)
}

/// Art eines Registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterKind {
    Keyword,
    Person,
    Scripture,
    Custom,
}

/// Ein Register: Kennung, Titel und Art.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Register {
    pub slug: String,
    pub title: String,
    pub kind: RegisterKind,
}

/// Die drei eingebauten Register in fester Reihenfolge.
pub fn builtin_registers() -> Vec<Register> {
    [
        (KEYWORD_REGISTER, "Stichwortregister", RegisterKind::Keyword),
        (PERSON_REGISTER, "Personenregister", RegisterKind::Person),
        (SCRIPTURE_REGISTER, "Bibelstellenregister", RegisterKind::Scripture),
    ]
    .into_iter()
    .map(|(slug, title, kind)| Register { slug: slug.to_owned(), title: title.to_owned(), kind })
    .collect()
}

/// `^[a-z][a-z0-9-]{0,31}$` (die eingebauten Kennungen erfüllen das auch).
pub fn valid_register_slug(slug: &str) -> bool {
    let bytes = slug.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes[0].is_ascii_lowercase()
        && bytes.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Vergleichsschlüssel eines Begriffs (in der Datenbank gespeichert, darum **unveränderlich**): NFC, jeder Leerraum
/// zu einem Leerzeichen zusammengefasst, getrimmt, `to_lowercase`.
pub fn term_key(term: &str) -> String {
    let nfc: String = term.nfc().collect();
    nfc.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Sortierschlüssel eines Begriffs (nie gespeichert): DIN 5007-1 – Umlaute wie ihre Grundbuchstaben (ä = a, ö = o,
/// ü = u), ß = ss, andere diakritische Zeichen fallen weg (é = e); klein, Leerraum zusammengefasst, Satzzeichen am
/// Anfang (Anführungszeichen, Klammern) übergangen.
pub fn sort_key(term: &str) -> String {
    let base: String = term
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .map(|c| if c == 'ß' { "ss".to_owned() } else { c.to_string() })
        .collect();
    let words = base.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = words.trim_start_matches(|c: char| !c.is_alphanumeric());
    if trimmed.is_empty() { words } else { trimmed.to_owned() }
}

/// Label eines Glossareintrags: `baruch-gl-` + uuid.
pub fn glossary_anchor(uuid: &str) -> String {
    format!("{GLOSSARY_ANCHOR_PREFIX}{uuid}")
}

/// Ein angenommener Glossarbegriff, wie ihn Satz und EPUB brauchen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub uuid: String,
    pub term: String,
    pub variants: Vec<String>,
    pub short: String,
    pub long: String,
    pub bible_refs: Vec<String>,
}

/// Alles, was Projektion und Buchende eines Werks brauchen: Satz-Einstellungen, Register (eingebaute + eigene) und die
/// angenommenen Glossarbegriffe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BackMatter {
    pub settings: TypesetSettings,
    pub registers: Vec<Register>,
    pub glossary: Vec<GlossaryEntry>,
}

impl BackMatter {
    /// Aktiv, sobald Registerseiten, Glossarseite oder die Auszeichnung der ersten Erwähnung eingestellt sind.
    pub fn is_active(&self) -> bool {
        self.settings.index_pages.is_some()
            || self.settings.glossary_page.is_some()
            || self.settings.glossary_first_mention == Some(true)
    }
}

/// Projektion der Abschnittstexte eines Exports (je Export eine Instanz, Abschnitte in Baumreihenfolge).
///
/// Setzt (a) bei Register `bibel` in `index_pages.registers` automatische Marken hinter erkannte Bibelstellen und (b)
/// bei `glossary_first_mention == Some(true)` die Auszeichnung der ersten Erwähnung je Kapitel (Form und Regeln in
/// `index/project.rs`: mit Glossarseite `#link(<baruch-gl-UUID>)[#emph[Wort]]`, sonst `#emph[Wort]`).
pub struct Projector<'a> {
    plan: project::Plan,
    _back: std::marker::PhantomData<&'a BackMatter>,
}

impl<'a> Projector<'a> {
    /// Neue Projektion für ein Werk.
    pub fn new(back: &'a BackMatter) -> Self {
        Projector { plan: project::Plan::new(back), _back: std::marker::PhantomData }
    }

    /// Projiziert einen Abschnittstext: automatische Bibelstellen-Marken, Auszeichnung der ersten Erwähnung je Kapitel.
    /// Inaktiv ([`BackMatter::is_active`]) → `Cow::Borrowed`.
    pub fn project<'b>(&mut self, body: &'b str) -> Cow<'b, str> {
        self.plan.project(body)
    }
}

/// Gegenstück zu [`back_matter`] für den Import (Integration R31, Runde 21b): Steht in `source` eine Zeile, die genau
/// [`BACK_MATTER_MARK`] lautet, **und** folgen ihr unmittelbar die Hilfsfunktionen, die nur [`back_matter`] schreibt
/// (`#let baruch-idx-ranges(ns) = …`), ist alles ab dieser Zeile das erzeugte Buchende eines Exports; es gehört zu
/// keinem Abschnitt und entsteht beim nächsten Export aus den Einstellungen neu. Eine Kennzeile ohne diesen Block (etwa
/// ein Kommentar von Hand im Buchtext) schneidet nichts ab – das meldet der Aufrufer ([`has_back_matter_mark`]). Zurück:
/// der Text bis vor die Kennzeile und ob etwas abgeschnitten wurde.
pub fn strip_back_matter(source: &str) -> (&str, bool) {
    let mut start = 0;
    for line in source.split_inclusive('\n') {
        let generated = source[start + line.len()..].starts_with(back::HELPERS);
        if generated && line.trim_end_matches(['\n', '\r']) == BACK_MATTER_MARK {
            return (&source[..start], true);
        }
        start += line.len();
    }
    (source, false)
}

/// Hat `source` eine Zeile, die genau [`BACK_MATTER_MARK`] lautet (erzeugt oder von Hand)?
pub fn has_back_matter_mark(source: &str) -> bool {
    source.lines().any(|line| line.trim_end_matches('\r') == BACK_MATTER_MARK)
}

/// Unsichtbarer Vermerk der Projektion vor einer ersten Erwähnung ohne Glossarseite (Runde 21b): `/*…*/#emph[Wort]`.
/// So erkennt der Import die Auszeichnung wieder ([`strip_first_mentions`]), ohne von Hand geschriebene Hervorhebung
/// anzufassen.
pub const FIRST_MENTION_MARK: &str = "/*baruch-erste-erwaehnung*/";

/// Nimmt die vermerkte erste Erwähnung ohne Glossarseite (`/*baruch-erste-erwaehnung*/#emph[Wort]`, samt Schutz-`;`)
/// wieder heraus: übrig bleibt `Wort`. Unabhängig vom Buchende (ohne Glossarseite und Register gibt es keins); alles
/// andere bleibt, auch eine Hervorhebung ohne Vermerk.
pub fn strip_first_mentions(source: &str) -> Cow<'_, str> {
    let open = format!("{FIRST_MENTION_MARK}#emph[");
    if !source.contains(&open) {
        return Cow::Borrowed(source);
    }
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    while let Some(found) = source[at..].find(&open).map(|i| at + i) {
        let word_start = found + open.len();
        let Some(word_len) = source[word_start..].find(']') else { break };
        let word = &source[word_start..word_start + word_len];
        // Runde 23a (Codereview satz-1): Die Projektion lässt zwischen zwei Wörtern eines Begriffs **einen**
        // Zeilenumbruch zu (hart umbrochener Quelltext) und schreibt ihn in die Auszeichnung. Der gehört darum zum
        // Wort und kommt mit ihm zurück; vorher blieb die Auszeichnung nach dem Re-Import als Text im Abschnitt. Eine
        // Leerzeile oder eine `[` im Wort schreibt die Projektion nie – das ist dann keine ihrer Auszeichnungen.
        if word.contains('[') || (word.contains('\n') && word.split('\n').any(|line| line.trim().is_empty())) {
            out.push_str(&source[at..word_start]);
            at = word_start;
            continue;
        }
        let mut end = word_start + word_len + 1;
        if source[end..].starts_with(';') && guarded(&source[end + 1..]) {
            end += 1;
        }
        out.push_str(&source[at..found]);
        out.push_str(word);
        at = end;
    }
    out.push_str(&source[at..]);
    Cow::Owned(out)
}

/// Nimmt aus einem Export mit Buchende die Zusätze des [`Projector`] wieder heraus (Integration R31, Import): die
/// automatischen Marken des Bibelstellenregisters (Register `bibel` mit `sort`, das nur die Projektion setzt) und die
/// verlinkte erste Erwähnung `#link(<baruch-gl-UUID>)[#emph[Wort]]` (samt dem Schutz-`;` der Projektion). Sonst entstünden
/// beim nächsten Export doppelte Bibelstellen und Verweise auf Glossar-Labels, die es im neuen Werk nicht gibt. Die
/// Auszeichnung ohne Glossarseite trägt seit Runde 21b den Vermerk [`FIRST_MENTION_MARK`] und fällt ebenso weg
/// ([`strip_first_mentions`]); eine Hervorhebung ohne Vermerk bleibt stehen.
pub fn strip_projection(source: &str) -> Cow<'_, str> {
    match strip_first_mentions(source) {
        Cow::Borrowed(_) => strip_linked_projection(source),
        Cow::Owned(unmarked) => Cow::Owned(strip_linked_projection(&unmarked).into_owned()),
    }
}

/// Automatische Bibelstellen-Marken und verlinkte erste Erwähnungen herausnehmen (Teil von [`strip_projection`]).
fn strip_linked_projection(source: &str) -> Cow<'_, str> {
    let mut cuts: Vec<(usize, usize, String)> = find_marks(source)
        .into_iter()
        .filter(|f| f.mark.register == SCRIPTURE_REGISTER && !f.mark.sort.is_empty())
        .map(|f| (f.span.0, f.span.1, String::new()))
        .collect();
    let open = format!("#link(<{GLOSSARY_ANCHOR_PREFIX}");
    let mut from = 0;
    while let Some(at) = source[from..].find(&open).map(|i| from + i) {
        let rest = &source[at + open.len()..];
        let parsed = rest.find(">)[#emph[").and_then(|label_end| {
            let label = &rest[..label_end];
            let word_start = label_end + ">)[#emph[".len();
            let word_len = rest[word_start..].find("]]")?;
            let valid = !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            valid.then_some((word_start, word_len))
        });
        let Some((word_start, word_len)) = parsed else {
            from = at + open.len();
            continue;
        };
        let word = &rest[word_start..word_start + word_len];
        let mut end = at + open.len() + word_start + word_len + 2;
        if source[end..].starts_with(';') && guarded(&source[end + 1..]) {
            end += 1;
        }
        cuts.push((at, end, word.to_owned()));
        from = end;
    }
    if cuts.is_empty() {
        return Cow::Borrowed(source);
    }
    cuts.sort_by_key(|c| c.0);
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for (a, b, with) in cuts {
        if a < at {
            continue;
        }
        out.push_str(&source[at..a]);
        out.push_str(&with);
        at = b;
    }
    out.push_str(&source[at..]);
    Cow::Owned(out)
}

/// Hätte die Projektion vor `after` ein Schutz-`;` gesetzt? (Gegenstück zu `needs_guard` in `index/project.rs`.)
fn guarded(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        Some('(' | '[' | ';') => true,
        Some('.') => chars.next().is_some_and(|c| c.is_alphabetic() || c == '_'),
        _ => false,
    }
}

/// Buchende (Registerseiten und Glossarseite) zu den projizierten Abschnittstexten `bodies`; inaktiv oder ohne
/// Registerseiten und Glossarseite → "". Sonst ein Block ab der Zeile [`BACK_MATTER_MARK`]: Hilfsfunktionen, dann je
/// eingestelltem Register eine Seite (Reihenfolge von `index_pages.registers`, unbekannte Kennungen fallen weg) und die
/// Glossarseite vor oder nach den Registern (Einzelheiten in `index/back.rs`).
pub fn back_matter(back: &BackMatter, bodies: &[&str]) -> String {
    if !back.is_active() {
        return String::new();
    }
    let mut collected = back::Collected::new();
    for (i, body) in bodies.iter().enumerate() {
        for found in find_marks(body) {
            back::collect(&mut collected, i as i64, &found.mark, back::is_auto(&found.mark));
        }
    }
    back::render(back, &collected)
}

/// Ein Stichwort der Übersicht mit den Abschnitten (`keys`), in denen es vorkommt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IndexTerm {
    pub term: String,
    pub sub: String,
    pub keys: Vec<i64>,
    pub see: Vec<String>,
    pub also: Vec<String>,
    pub auto: bool,
}

/// Übersicht eines Registers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegisterView {
    pub register: Register,
    pub terms: Vec<IndexTerm>,
}

/// Übersicht je Register über die Abschnitte `(key, Text)` (gespeicherte, nicht projizierte Texte): alle Register aus
/// `back.registers` in ihrer Reihenfolge, Stichwörter sortiert wie im Buchende, `keys` = Abschnitte mit Marken, die eine
/// Seite tragen. Ist das Bibelstellenregister eingeschaltet, kommen die automatischen Bibelstellen der Projektion dazu
/// (`auto`). Marken mit unbekanntem Register erscheinen nicht.
pub fn overview(back: &BackMatter, bodies: &[(i64, &str)]) -> Vec<RegisterView> {
    let scripture = project::scripture_enabled(back);
    let mut collected = back::Collected::new();
    for &(key, body) in bodies {
        let scanned = scan::scan(body);
        for found in &scanned.marks {
            back::collect(&mut collected, key, &found.mark, back::is_auto(&found.mark));
        }
        if scripture {
            for (_, mark) in project::scripture_marks(body, &scanned) {
                back::collect(&mut collected, key, &mark, true);
            }
        }
    }
    back::views(back, &collected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_source_writes_the_contract_form() {
        assert_eq!(
            mark_source(&IndexMark::new("stichwort", "Gnade")),
            r#"#metadata((register: "stichwort", term: "Gnade"))<baruch-idx>"#,
            "Form laut Vertrag §2.1, kein Leerzeichen vor dem Label"
        );
        let mark = IndexMark {
            register: "personen".into(),
            term: "Ruth \"die Moabiterin\"".into(),
            sub: "Weg\\Ziel".into(),
            also: "Noomi".into(),
            ..IndexMark::default()
        };
        assert_eq!(
            mark_source(&mark),
            r#"#metadata((register: "personen", term: "Ruth \"die Moabiterin\"", sub: "Weg\\Ziel", also: "Noomi"))<baruch-idx>"#,
            "leere Schlüssel fehlen, Anführungszeichen und Rückstrich sind maskiert"
        );
    }

    #[test]
    fn parse_mark_reverses_mark_source_and_counts_bytes() {
        let mark = IndexMark {
            register: "orte".into(),
            term: "Ölberg".into(),
            sub: "Garten".into(),
            see: "Berg".into(),
            also: "Tal".into(),
            sort: "0001".into(),
        };
        let source = mark_source(&mark);
        let text = format!("{source} danach");
        assert_eq!(parse_mark(&text), Some((mark, source.len())), "Rundreise, verbrauchte Bytes = Länge der Marke");
    }

    #[test]
    fn parse_mark_allows_space_but_refuses_everything_else() {
        let spaced = r#"#metadata(( register : "stichwort" ,term:"Gnade" ))<baruch-idx>"#;
        assert_eq!(
            parse_mark(spaced).map(|(m, n)| (m, n == spaced.len())),
            Some((IndexMark::new("stichwort", "Gnade"), true)),
            "Leerraum um : und , ist erlaubt"
        );
        for island in [
            r#"#metadata((register: "stichwort", term: "Gnade")) <baruch-idx>"#,
            r#"#metadata((term: "Gnade", register: "stichwort"))<baruch-idx>"#,
            r#"#metadata((register: "stichwort"))<baruch-idx>"#,
            r#"#metadata((register: "stichwort", term: "Gnade", farbe: "rot"))<baruch-idx>"#,
            r#"#metadata((register: "stichwort", term: "Gnade"))<anderes>"#,
            r#"#metadata((register: "Stichwort", term: "Gnade"))<baruch-idx>"#,
            "#metadata((register: \"stichwort\", term: \"Gna\nde\"))<baruch-idx>",
        ] {
            assert_eq!(parse_mark(island), None, "keine kanonische Marke, bleibt Insel: {island}");
        }
    }

    #[test]
    fn validate_checks_register_term_and_lengths() {
        assert_eq!(IndexMark::new("stichwort", "Gnade").validate(), Ok(()), "gültige Marke");
        assert!(IndexMark::new("Stichwort", "Gnade").validate().is_err(), "Großbuchstabe in der Kennung");
        assert!(IndexMark::new("stichwort", "  ").validate().is_err(), "leeres Stichwort");
        let long = "x".repeat(MAX_MARK_FIELD_CHARS + 1);
        assert!(IndexMark::new("stichwort", &long).validate().is_err(), "zu langes Stichwort");
    }

    #[test]
    fn occurrence_params_round_trip() {
        for o in [Occurrence::First, Occurrence::All, Occurrence::Nth(3)] {
            assert_eq!(Occurrence::from_param(&o.as_param()), Some(o), "from_param kehrt as_param um: {o:?}");
            let json = serde_json::to_string(&o).expect("serialisierbar");
            assert_eq!(serde_json::from_str::<Occurrence>(&json).ok(), Some(o), "JSON als Text: {json}");
        }
        assert_eq!(Occurrence::from_param("0"), None, "die Zählung beginnt bei 1");
        assert_eq!(Occurrence::from_param("zweite"), None, "Unbekanntes wird abgelehnt");
    }

    #[test]
    fn slugs_keys_and_anchor() {
        assert!(builtin_registers().iter().all(|r| valid_register_slug(&r.slug)), "eingebaute Kennungen gültig");
        assert!(valid_register_slug("orte-2"), "eigene Kennung");
        assert!(!valid_register_slug("2orte") && !valid_register_slug("") && !valid_register_slug(&"a".repeat(33)));
        assert_eq!(term_key("  Gna\u{0308}de \t im   Alltag "), "gnäde im alltag", "NFC, Leerraum, klein");
        assert_eq!(glossary_anchor("abc"), "baruch-gl-abc", "Label der Glossarseite");
    }

    #[test]
    fn inactive_back_matter_changes_nothing() {
        let back = BackMatter::default();
        assert!(!back.is_active(), "ohne R31-Einstellungen inaktiv");
        let mut projector = Projector::new(&back);
        assert!(matches!(projector.project("= Kapitel\nText\n"), Cow::Borrowed(_)), "Projektion = Identität");
        assert_eq!(back_matter(&back, &["Text"]), "", "kein Buchende");
        let active = BackMatter {
            settings: TypesetSettings { glossary_first_mention: Some(true), ..TypesetSettings::default() },
            ..BackMatter::default()
        };
        assert!(active.is_active(), "erste Erwähnung = aktiv");
    }
}
