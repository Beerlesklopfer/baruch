// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typesetting settings in the exchange format (Round 29, sub-version 6; documentation
//! `docs/Austauschformat.md`, section „Satz-Einstellungen“): words of the attributes of `<satz>` per field of
//! [`baruch_typst::typeset::TypesetSettings`].
//!
//! - Every field is an attribute (missing = from the work template); compound fields are there completely or not
//!   at all: `teil-seite` + `teil-rueckseite-leer`, `seitenformat` (with `eigenes` together with
//!   `seite-breite-mm` + `seite-hoehe-mm`), the five margins `rand-innen-mm`, `rand-aussen-mm`, `rand-oben-mm`,
//!   `rand-unten-mm`, `bundsteg-mm`, `absatz` + `absatz-em`, `seitenzahlen` + `seitenzahlen-beginn`,
//!   `fussnoten` + `fussnoten-zaehlung`. Half details are rejected by [`settings_from_attrs`].
//! - Numbers as `f64::to_string` (like `double` in the writer), booleans `true`/`false`.
//! - Nobody checks the values against the limits here; the import does that with `TypesetSettings::validate`.
//! - From sub-version 7 (Round 31) the index/glossary fields ([`R31_ATTRIBUTES`]): `glossarseite`
//!   (`vor-registern`|`nach-registern`), `registerseiten` (identifiers separated by spaces) +
//!   `registerseiten-spalten` (together), `glossar-erste-erwaehnung` (true|false). [`settings_from_attrs`] does
//!   not read them (sub-version 6); that is done by [`r31_from_attrs`], which the reader calls only from
//!   sub-version 7 – in an older file they are thus reported as unknown.
//! - From sub-version 8 (Round 22) the draft as child `<entwurf/>` of `<satz>` ([`DRAFT_ATTRIBUTES`]): `text`,
//!   `zusatz` (only if not empty), `datum`, `deckkraft` (percent), `lage` (`diagonal`|`kopfzeile`|`fusszeile`),
//!   `umschlag`. [`draft_attrs`] and [`draft_from_attrs`]; the reader calls the latter only from sub-version 8.

use baruch_typst::typeset::{
    ChapterStart, Draft, DraftPlacement, FootnoteNumbering, FootnoteRestart, Footnotes, GlossaryPosition, IndexPages,
    Margins, PageFormat, PageNumberPosition, PageNumberStart, PageNumbers, Paragraph, PartPage, PartSide, RunningHead,
    TypesetSettings,
};

/// Attributes of the index/glossary fields (from sub-version 7), in the order of the writer.
pub const R31_ATTRIBUTES: [&str; 4] =
    ["glossarseite", "registerseiten", "registerseiten-spalten", "glossar-erste-erwaehnung"];

/// All attributes of `<satz>` except `uuid`, `geaendert`, `geaendert-von`, in the order of the writer.
pub const TYPESET_ATTRIBUTES: [&str; 30] = [
    "kapitelanfang",
    "teil-seite",
    "teil-rueckseite-leer",
    "seitenformat",
    "seite-breite-mm",
    "seite-hoehe-mm",
    "rand-innen-mm",
    "rand-aussen-mm",
    "rand-oben-mm",
    "rand-unten-mm",
    "bundsteg-mm",
    "grundschrift",
    "schriftgroesse-pt",
    "zeilenabstand-em",
    "absatz",
    "absatz-em",
    "blocksatz",
    "silbentrennung",
    "sprache",
    "seitenzahlen",
    "seitenzahlen-beginn",
    "kolumnentitel",
    "fussnoten",
    "fussnoten-zaehlung",
    "schusterjungen-hurenkinder",
    "verweis-seitenzahl",
    "glossarseite",
    "registerseiten",
    "registerseiten-spalten",
    "glossar-erste-erwaehnung",
];

/// Attributes of `<entwurf/>` (from sub-version 8) in the order of the writer.
pub const DRAFT_ATTRIBUTES: [&str; 6] = ["text", "zusatz", "datum", "deckkraft", "lage", "umschlag"];

/// Position of the watermark in the XML.
const DRAFT_PLACEMENT: [(DraftPlacement, &str); 3] = [
    (DraftPlacement::Diagonal, "diagonal"),
    (DraftPlacement::Header, "kopfzeile"),
    (DraftPlacement::Footer, "fusszeile"),
];

/// Word of an enumeration in the XML and back: (value, word).
const CHAPTER_START: [(ChapterStart, &str); 3] = [
    (ChapterStart::Continuous, "fortlaufend"),
    (ChapterStart::NewPage, "neue-seite"),
    (ChapterStart::RightPage, "rechte-seite"),
];
const PART_SIDE: [(PartSide, &str); 2] = [(PartSide::Right, "rechts"), (PartSide::Any, "beliebig")];
const PAGE_POSITION: [(PageNumberPosition, &str); 4] = [
    (PageNumberPosition::BottomCenter, "unten-mitte"),
    (PageNumberPosition::BottomOutside, "unten-aussen"),
    (PageNumberPosition::TopOutside, "oben-aussen"),
    (PageNumberPosition::Hidden, "keine"),
];
const PAGE_START: [(PageNumberStart, &str); 2] =
    [(PageNumberStart::FirstPage, "erste-seite"), (PageNumberStart::FirstChapter, "erstes-kapitel")];
const RUNNING_HEAD: [(RunningHead, &str); 3] =
    [(RunningHead::Off, "aus"), (RunningHead::WorkTitle, "werktitel"), (RunningHead::Chapter, "kapitel")];
const FOOTNOTE_NUMBERING: [(FootnoteNumbering, &str); 3] = [
    (FootnoteNumbering::Arabic, "arabisch"),
    (FootnoteNumbering::Symbols, "zeichen"),
    (FootnoteNumbering::Letters, "buchstaben"),
];
const FOOTNOTE_RESTART: [(FootnoteRestart, &str); 2] =
    [(FootnoteRestart::Continuous, "fortlaufend"), (FootnoteRestart::PerChapter, "je-kapitel")];
const GLOSSARY_POSITION: [(GlossaryPosition, &str); 2] =
    [(GlossaryPosition::BeforeIndexes, "vor-registern"), (GlossaryPosition::AfterIndexes, "nach-registern")];

fn word_of<T: PartialEq + Copy>(table: &[(T, &'static str)], value: T) -> &'static str {
    table.iter().find(|(v, _)| *v == value).map_or("", |(_, w)| w)
}

fn value_of<T: Copy>(table: &[(T, &'static str)], word: &str, name: &str) -> Result<T, String> {
    table.iter().find(|(_, w)| *w == word).map(|(v, _)| *v).ok_or_else(|| {
        let allowed: Vec<&str> = table.iter().map(|(_, w)| *w).collect();
        format!("@{name}: unbekannter Wert „{word}“ ({})", allowed.join("|"))
    })
}

fn bool_word(value: bool) -> String {
    if value { "true".into() } else { "false".into() }
}

fn format_word(format: PageFormat) -> &'static str {
    match format {
        PageFormat::A5 => "a5",
        PageFormat::Mm120x190 => "120x190",
        PageFormat::Mm135x215 => "135x215",
        PageFormat::A4 => "a4",
        PageFormat::Custom { .. } => "eigenes",
    }
}

/// Attributes (name, value) of the set fields in the order of [`TYPESET_ATTRIBUTES`]. A non-finite number →
/// `Err` with the name of the attribute (the XSD knows only finite `double`).
pub fn settings_attrs(s: &TypesetSettings) -> Result<Vec<(&'static str, String)>, String> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let number = |out: &mut Vec<(&'static str, String)>, name: &'static str, x: f64| {
        if x.is_finite() {
            out.push((name, x.to_string()));
            Ok(())
        } else {
            Err(format!("@{name}: keine endliche Zahl"))
        }
    };
    if let Some(c) = s.chapter_start {
        out.push(("kapitelanfang", word_of(&CHAPTER_START, c).into()));
    }
    if let Some(p) = &s.part_page {
        out.push(("teil-seite", word_of(&PART_SIDE, p.side).into()));
        out.push(("teil-rueckseite-leer", bool_word(p.blank_back)));
    }
    if let Some(f) = s.page_format {
        out.push(("seitenformat", format_word(f).into()));
        if let PageFormat::Custom { width_mm, height_mm } = f {
            number(&mut out, "seite-breite-mm", width_mm)?;
            number(&mut out, "seite-hoehe-mm", height_mm)?;
        }
    }
    if let Some(m) = &s.margins {
        number(&mut out, "rand-innen-mm", m.inside_mm)?;
        number(&mut out, "rand-aussen-mm", m.outside_mm)?;
        number(&mut out, "rand-oben-mm", m.top_mm)?;
        number(&mut out, "rand-unten-mm", m.bottom_mm)?;
        number(&mut out, "bundsteg-mm", m.binding_mm)?;
    }
    if let Some(font) = &s.font {
        out.push(("grundschrift", font.clone()));
    }
    if let Some(size) = s.font_size_pt {
        number(&mut out, "schriftgroesse-pt", size)?;
    }
    if let Some(leading) = s.leading_em {
        number(&mut out, "zeilenabstand-em", leading)?;
    }
    if let Some(p) = s.paragraph {
        let word = match p {
            Paragraph::Indent { .. } => "einzug",
            Paragraph::Spacing { .. } => "abstand",
        };
        out.push(("absatz", word.into()));
        number(&mut out, "absatz-em", p.em())?;
    }
    if let Some(v) = s.justify {
        out.push(("blocksatz", bool_word(v)));
    }
    if let Some(v) = s.hyphenate {
        out.push(("silbentrennung", bool_word(v)));
    }
    if let Some(lang) = &s.lang {
        out.push(("sprache", lang.clone()));
    }
    if let Some(n) = &s.page_numbers {
        out.push(("seitenzahlen", word_of(&PAGE_POSITION, n.position).into()));
        out.push(("seitenzahlen-beginn", word_of(&PAGE_START, n.start).into()));
    }
    if let Some(r) = s.running_head {
        out.push(("kolumnentitel", word_of(&RUNNING_HEAD, r).into()));
    }
    if let Some(f) = &s.footnotes {
        out.push(("fussnoten", word_of(&FOOTNOTE_NUMBERING, f.numbering).into()));
        out.push(("fussnoten-zaehlung", word_of(&FOOTNOTE_RESTART, f.restart).into()));
    }
    if let Some(v) = s.widows_orphans {
        out.push(("schusterjungen-hurenkinder", bool_word(v)));
    }
    if let Some(v) = s.xref_with_page {
        out.push(("verweis-seitenzahl", bool_word(v)));
    }
    if let Some(g) = s.glossary_page {
        out.push(("glossarseite", word_of(&GLOSSARY_POSITION, g).into()));
    }
    if let Some(p) = &s.index_pages {
        if let Some(bad) = p.registers.iter().find(|r| r.is_empty() || r.chars().any(char::is_whitespace)) {
            return Err(format!("@registerseiten: Kennung „{bad}“ ist leer oder enthält Leerraum"));
        }
        out.push(("registerseiten", p.registers.join(" ")));
        out.push(("registerseiten-spalten", p.columns.to_string()));
    }
    if let Some(v) = s.glossary_first_mention {
        out.push(("glossar-erste-erwaehnung", bool_word(v)));
    }
    Ok(out)
}

fn parse_bool(value: &str, name: &str) -> Result<bool, String> {
    match value.trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(format!("@{name}: „{other}“ ist kein Wahrheitswert (true|false)")),
    }
}

fn parse_number(value: &str, name: &str) -> Result<f64, String> {
    let number: f64 = value
        .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
        .parse()
        .map_err(|_| format!("@{name}: „{value}“ ist keine Zahl"))?;
    if number.is_finite() { Ok(number) } else { Err(format!("@{name}: „{value}“ ist keine endliche Zahl")) }
}

/// Settings from the attributes of `<satz>`; `get(name)` delivers the value of an attribute (and remembers it as
/// read). Unknown words, not a number or not a boolean, and half compound fields → `Err` with the name of the
/// attribute (`@rand-oben-mm: …`).
pub fn settings_from_attrs<'a>(mut get: impl FnMut(&str) -> Option<&'a str>) -> Result<TypesetSettings, String> {
    let mut s = TypesetSettings::default();
    if let Some(word) = get("kapitelanfang") {
        s.chapter_start = Some(value_of(&CHAPTER_START, word, "kapitelanfang")?);
    }
    match (get("teil-seite"), get("teil-rueckseite-leer")) {
        (Some(side), Some(back)) => {
            s.part_page = Some(PartPage {
                side: value_of(&PART_SIDE, side, "teil-seite")?,
                blank_back: parse_bool(back, "teil-rueckseite-leer")?,
            });
        }
        (None, None) => {}
        _ => return Err("@teil-seite: nur zusammen mit teil-rueckseite-leer".into()),
    }
    let (width, height) = (get("seite-breite-mm"), get("seite-hoehe-mm"));
    match get("seitenformat") {
        Some("eigenes") => {
            let (Some(width), Some(height)) = (width, height) else {
                return Err("@seitenformat: „eigenes“ braucht seite-breite-mm und seite-hoehe-mm".into());
            };
            s.page_format = Some(PageFormat::Custom {
                width_mm: parse_number(width, "seite-breite-mm")?,
                height_mm: parse_number(height, "seite-hoehe-mm")?,
            });
        }
        Some(word) => {
            if width.is_some() || height.is_some() {
                return Err("@seite-breite-mm: Maße nur bei seitenformat=\"eigenes\"".into());
            }
            let fixed = PageFormat::ALL.iter().copied().find(|f| format_word(*f) == word);
            s.page_format =
                Some(fixed.ok_or_else(|| {
                    format!("@seitenformat: unbekannter Wert „{word}“ (a5|120x190|135x215|a4|eigenes)")
                })?);
        }
        None if width.is_some() || height.is_some() => {
            return Err("@seite-breite-mm: Maße nur bei seitenformat=\"eigenes\"".into());
        }
        None => {}
    }
    const MARGINS: [&str; 5] = ["rand-innen-mm", "rand-aussen-mm", "rand-oben-mm", "rand-unten-mm", "bundsteg-mm"];
    let margins: Vec<Option<&str>> = MARGINS.iter().map(|name| get(name)).collect();
    if margins.iter().all(Option::is_some) {
        let n = |i: usize| parse_number(margins[i].unwrap_or_default(), MARGINS[i]);
        s.margins =
            Some(Margins { inside_mm: n(0)?, outside_mm: n(1)?, top_mm: n(2)?, bottom_mm: n(3)?, binding_mm: n(4)? });
    } else if let Some(missing) =
        margins.iter().position(Option::is_none).filter(|_| margins.iter().any(Option::is_some))
    {
        return Err(format!("@{}: die fünf Ränder stehen nur zusammen", MARGINS[missing]));
    }
    s.font = get("grundschrift").map(str::to_owned);
    s.font_size_pt = get("schriftgroesse-pt").map(|v| parse_number(v, "schriftgroesse-pt")).transpose()?;
    s.leading_em = get("zeilenabstand-em").map(|v| parse_number(v, "zeilenabstand-em")).transpose()?;
    match (get("absatz"), get("absatz-em")) {
        (Some(kind), Some(em)) => {
            let em = parse_number(em, "absatz-em")?;
            s.paragraph = Some(match kind {
                "einzug" => Paragraph::Indent { em },
                "abstand" => Paragraph::Spacing { em },
                other => return Err(format!("@absatz: unbekannter Wert „{other}“ (einzug|abstand)")),
            });
        }
        (None, None) => {}
        _ => return Err("@absatz: nur zusammen mit absatz-em".into()),
    }
    s.justify = get("blocksatz").map(|v| parse_bool(v, "blocksatz")).transpose()?;
    s.hyphenate = get("silbentrennung").map(|v| parse_bool(v, "silbentrennung")).transpose()?;
    s.lang = get("sprache").map(str::to_owned);
    match (get("seitenzahlen"), get("seitenzahlen-beginn")) {
        (Some(position), Some(start)) => {
            s.page_numbers = Some(PageNumbers {
                position: value_of(&PAGE_POSITION, position, "seitenzahlen")?,
                start: value_of(&PAGE_START, start, "seitenzahlen-beginn")?,
            });
        }
        (None, None) => {}
        _ => return Err("@seitenzahlen: nur zusammen mit seitenzahlen-beginn".into()),
    }
    if let Some(word) = get("kolumnentitel") {
        s.running_head = Some(value_of(&RUNNING_HEAD, word, "kolumnentitel")?);
    }
    match (get("fussnoten"), get("fussnoten-zaehlung")) {
        (Some(numbering), Some(restart)) => {
            s.footnotes = Some(Footnotes {
                numbering: value_of(&FOOTNOTE_NUMBERING, numbering, "fussnoten")?,
                restart: value_of(&FOOTNOTE_RESTART, restart, "fussnoten-zaehlung")?,
            });
        }
        (None, None) => {}
        _ => return Err("@fussnoten: nur zusammen mit fussnoten-zaehlung".into()),
    }
    s.widows_orphans =
        get("schusterjungen-hurenkinder").map(|v| parse_bool(v, "schusterjungen-hurenkinder")).transpose()?;
    s.xref_with_page = get("verweis-seitenzahl").map(|v| parse_bool(v, "verweis-seitenzahl")).transpose()?;
    Ok(s)
}

/// Enter index/glossary fields (from sub-version 7) from the attributes of `<satz>` into `s`; `get` as in
/// [`settings_from_attrs`]. Unknown words, empty index list, no column count and `registerseiten` without
/// `registerseiten-spalten` (and vice versa) → `Err` with the name of the attribute.
pub fn r31_from_attrs<'a>(s: &mut TypesetSettings, mut get: impl FnMut(&str) -> Option<&'a str>) -> Result<(), String> {
    if let Some(word) = get("glossarseite") {
        s.glossary_page = Some(value_of(&GLOSSARY_POSITION, word, "glossarseite")?);
    }
    match (get("registerseiten"), get("registerseiten-spalten")) {
        (Some(list), Some(columns)) => {
            let registers: Vec<String> = list.split_whitespace().map(str::to_owned).collect();
            if registers.is_empty() {
                return Err("@registerseiten: mindestens ein Register".into());
            }
            let columns: u8 = columns
                .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
                .parse()
                .map_err(|_| format!("@registerseiten-spalten: „{columns}“ ist keine ganze Zahl"))?;
            s.index_pages = Some(IndexPages { registers, columns });
        }
        (None, None) => {}
        _ => return Err("@registerseiten: nur zusammen mit registerseiten-spalten".into()),
    }
    s.glossary_first_mention =
        get("glossar-erste-erwaehnung").map(|v| parse_bool(v, "glossar-erste-erwaehnung")).transpose()?;
    Ok(())
}

/// Attributes of `<entwurf/>` (from sub-version 8) in the order of [`DRAFT_ATTRIBUTES`]; `zusatz` only if not
/// empty.
pub fn draft_attrs(d: &Draft) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = vec![("text", d.text.clone())];
    if !d.note.is_empty() {
        out.push(("zusatz", d.note.clone()));
    }
    out.push(("datum", bool_word(d.date)));
    out.push(("deckkraft", d.opacity_percent.to_string()));
    out.push(("lage", word_of(&DRAFT_PLACEMENT, d.placement).into()));
    out.push(("umschlag", bool_word(d.cover)));
    out
}

/// Draft from the attributes of `<entwurf/>`; mandatory are `text`, `deckkraft` and `lage`, `zusatz` missing =
/// empty, `datum`/`umschlag` missing = `false`. Errors with the name of the attribute. The limits are checked by
/// the import (`TypesetSettings::validate`).
pub fn draft_from_attrs<'a>(mut get: impl FnMut(&str) -> Option<&'a str>) -> Result<Draft, String> {
    let text = get("text").ok_or("@text: Pflichtattribut fehlt")?.to_owned();
    let note = get("zusatz").unwrap_or("").to_owned();
    let date = get("datum").map(|v| parse_bool(v, "datum")).transpose()?.unwrap_or(false);
    let raw = get("deckkraft").ok_or("@deckkraft: Pflichtattribut fehlt")?;
    let opacity_percent: u8 = raw
        .trim_matches(|c| matches!(c, ' ' | '\t' | '\n' | '\r'))
        .parse()
        .map_err(|_| format!("@deckkraft: „{raw}“ ist keine ganze Zahl von 0 bis 255"))?;
    let placement = value_of(&DRAFT_PLACEMENT, get("lage").ok_or("@lage: Pflichtattribut fehlt")?, "lage")?;
    let cover = get("umschlag").map(|v| parse_bool(v, "umschlag")).transpose()?.unwrap_or(false);
    Ok(Draft { text, note, date, opacity_percent, placement, cover })
}

/// A draft with all fields (round-trip tests of sub-version 8; freely invented texts with special characters).
pub fn full_draft() -> Draft {
    Draft {
        text: "Probe & <Satz>".into(),
        note: "Stand \"heute\" · erfunden".into(),
        date: true,
        opacity_percent: 20,
        placement: DraftPlacement::Footer,
        cover: true,
    }
}

/// All fields set (example of contract R29 §2, since Round 31 with the index/glossary fields) – for
/// round-trip tests.
pub fn full_sample() -> TypesetSettings {
    TypesetSettings {
        chapter_start: Some(ChapterStart::RightPage),
        part_page: Some(PartPage { side: PartSide::Right, blank_back: true }),
        page_format: Some(PageFormat::A5),
        margins: Some(Margins { inside_mm: 20.0, outside_mm: 15.0, top_mm: 18.0, bottom_mm: 22.0, binding_mm: 5.0 }),
        font: Some("Libertinus Serif".into()),
        font_size_pt: Some(10.5),
        leading_em: Some(0.65),
        paragraph: Some(Paragraph::Indent { em: 1.0 }),
        justify: Some(true),
        hyphenate: Some(true),
        lang: Some("de".into()),
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomOutside,
            start: PageNumberStart::FirstChapter,
        }),
        running_head: Some(RunningHead::Chapter),
        footnotes: Some(Footnotes { numbering: FootnoteNumbering::Arabic, restart: FootnoteRestart::PerChapter }),
        widows_orphans: Some(true),
        xref_with_page: Some(true),
        // Round 31 (sub-version 7): index/glossary fields.
        glossary_page: Some(GlossaryPosition::BeforeIndexes),
        index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "bibel".into()], columns: 2 }),
        glossary_first_mention: Some(true),
        // Round 22 (S1): the draft is in the format only from sub-version 8 (spiegel-entwurf).
        draft: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn round_trip(s: &TypesetSettings) -> TypesetSettings {
        let attrs: HashMap<&str, String> = settings_attrs(s).unwrap().into_iter().collect();
        let mut back = settings_from_attrs(|name| attrs.get(name).map(String::as_str)).unwrap();
        r31_from_attrs(&mut back, |name| attrs.get(name).map(String::as_str)).unwrap();
        back
    }

    #[test]
    fn the_draft_survives_the_round_trip_with_every_placement() {
        // R22 §2.11 (b): every field of the draft is in <entwurf/>; without an addition only `zusatz` is missing.
        for placement in DraftPlacement::ALL {
            let draft = Draft { placement: *placement, ..full_draft() };
            let attrs: HashMap<&str, String> = draft_attrs(&draft).into_iter().collect();
            let back = draft_from_attrs(|name| attrs.get(name).map(String::as_str)).unwrap();
            assert_eq!(back, draft, "Rundreise mit Lage {placement:?}");
        }
        let plain = Draft::default();
        let attrs = draft_attrs(&plain);
        assert!(attrs.iter().all(|(name, _)| *name != "zusatz"), "leerer Zusatz fehlt: {attrs:?}");
        assert!(
            attrs.iter().map(|(n, _)| *n).all(|n| DRAFT_ATTRIBUTES.contains(&n)),
            "nur bekannte Attribute: {attrs:?}"
        );
        let err = draft_from_attrs(|name| (name == "text").then_some("X")).unwrap_err();
        assert!(err.contains("deckkraft"), "fehlende Deckkraft nennt das Attribut: {err}");
        let bad: HashMap<&str, &str> = [("text", "X"), ("deckkraft", "12"), ("lage", "quer")].into_iter().collect();
        let err = draft_from_attrs(|name| bad.get(name).copied()).unwrap_err();
        assert!(err.contains("@lage") && err.contains("quer"), "fremdes Wort nennt Attribut und Wert: {err}");
    }

    #[test]
    fn every_field_and_every_word_survives_the_round_trip() {
        // Contract §6.6: every field from §2 representable. First all fields, then per enumeration every word singly.
        let full = full_sample();
        assert_eq!(round_trip(&full), full, "alle 19 Felder");
        assert_eq!(
            settings_attrs(&full).unwrap().len(),
            28,
            "alle 30 Attribute außer den zwei Maßen eines eigenen Formats"
        );
        for &g in GlossaryPosition::ALL {
            let s = TypesetSettings { glossary_page: Some(g), ..Default::default() };
            assert_eq!(round_trip(&s), s, "Glossarseite {g:?}");
        }
        for columns in 1..=3 {
            let pages = IndexPages { registers: vec!["personen".into(), "orte-und-wege".into()], columns };
            let s =
                TypesetSettings { index_pages: Some(pages), glossary_first_mention: Some(false), ..Default::default() };
            assert_eq!(round_trip(&s), s, "Registerseiten mit {columns} Spalten");
        }
        assert_eq!(round_trip(&TypesetSettings::default()), TypesetSettings::default(), "leer bleibt leer");
        for &c in ChapterStart::ALL {
            let s = TypesetSettings { chapter_start: Some(c), ..Default::default() };
            assert_eq!(round_trip(&s), s, "Kapitelanfang {c:?}");
        }
        for &side in PartSide::ALL {
            let s = TypesetSettings { part_page: Some(PartPage { side, blank_back: false }), ..Default::default() };
            assert_eq!(round_trip(&s), s, "Teil-Seite {side:?}");
        }
        for &f in PageFormat::ALL.iter().chain([PageFormat::Custom { width_mm: 125.5, height_mm: 200.0 }].iter()) {
            let s = TypesetSettings { page_format: Some(f), ..Default::default() };
            assert_eq!(round_trip(&s), s, "Format {f:?}");
        }
        for p in [Paragraph::Indent { em: 1.25 }, Paragraph::Spacing { em: 0.5 }] {
            let s = TypesetSettings { paragraph: Some(p), ..Default::default() };
            assert_eq!(round_trip(&s), s, "Absatz {p:?}");
        }
        for &position in PageNumberPosition::ALL {
            for &start in PageNumberStart::ALL {
                let s = TypesetSettings { page_numbers: Some(PageNumbers { position, start }), ..Default::default() };
                assert_eq!(round_trip(&s), s, "Seitenzahlen {position:?}/{start:?}");
            }
        }
        for &r in RunningHead::ALL {
            let s = TypesetSettings { running_head: Some(r), ..Default::default() };
            assert_eq!(round_trip(&s), s, "Kolumnentitel {r:?}");
        }
        for &numbering in FootnoteNumbering::ALL {
            for &restart in FootnoteRestart::ALL {
                let s = TypesetSettings { footnotes: Some(Footnotes { numbering, restart }), ..Default::default() };
                assert_eq!(round_trip(&s), s, "Fußnoten {numbering:?}/{restart:?}");
            }
        }
        let falses = TypesetSettings {
            justify: Some(false),
            hyphenate: Some(false),
            widows_orphans: Some(false),
            xref_with_page: Some(false),
            ..Default::default()
        };
        assert_eq!(round_trip(&falses), falses, "„aus“ ist etwas anderes als „aus der Werkvorlage“");
    }

    #[test]
    fn every_attribute_of_the_writer_is_listed() {
        // The reader reports only attributes as unknown that are not in the list; so every written line must
        // occur in it (also the dimensions of the custom format).
        let custom = TypesetSettings {
            page_format: Some(PageFormat::Custom { width_mm: 130.0, height_mm: 200.0 }),
            ..full_sample()
        };
        let written: Vec<&str> = settings_attrs(&custom).unwrap().into_iter().map(|(n, _)| n).collect();
        assert_eq!(written, TYPESET_ATTRIBUTES, "alle Attribute in der Reihenfolge der Liste");
    }

    #[test]
    fn half_composite_fields_and_unknown_words_are_refused() {
        // Half details would silently lose a field; the reader rejects them with the name of the attribute.
        let cases: [(&[(&str, &str)], &str); 11] = [
            (&[("glossarseite", "hinten")], "@glossarseite"),
            (&[("registerseiten", "stichwort")], "@registerseiten"),
            (&[("registerseiten", " "), ("registerseiten-spalten", "2")], "@registerseiten"),
            (&[("registerseiten", "bibel"), ("registerseiten-spalten", "zwei")], "@registerseiten-spalten"),
            (&[("teil-seite", "rechts")], "@teil-seite"),
            (&[("rand-innen-mm", "20"), ("rand-aussen-mm", "15")], "@rand-oben-mm"),
            (&[("seitenformat", "eigenes"), ("seite-breite-mm", "120")], "@seitenformat"),
            (&[("seitenformat", "a5"), ("seite-breite-mm", "120")], "@seite-breite-mm"),
            (&[("absatz", "einzug")], "@absatz"),
            (&[("kapitelanfang", "links")], "@kapitelanfang"),
            (&[("schriftgroesse-pt", "NaN")], "@schriftgroesse-pt"),
        ];
        for (attrs, expected) in cases {
            let map: HashMap<&str, &str> = attrs.iter().copied().collect();
            let error = settings_from_attrs(|name| map.get(name).copied())
                .and_then(|mut s| r31_from_attrs(&mut s, |name| map.get(name).copied()).map(|()| s))
                .unwrap_err();
            assert!(error.starts_with(expected), "{attrs:?}: Meldung nennt {expected}: {error}");
        }
    }
}
