// SPDX-License-Identifier: AGPL-3.0-or-later
//! Auszeichnung eines Eintrags zu Klartext: OSIS (Vorgabe der meisten Module), ThML, GBF, reiner Text.
//! Weggelassen werden Fußnoten, Querverweise, Überschriften (außer kanonischen, z. B. Psalmüberschriften,
//! die in deutscher Zählung ein Vers sind), Strong-Nummern und Morphologie; `divineName`, Wörter (`w`),
//! Ergänzungen (`transChange`) und Zitate (`q`) bleiben als Text.

use sword_modules::Markup;

/// Klartext eines Eintrags in der Auszeichnung `markup`; Leerraum zusammengefasst und getrimmt.
pub fn to_plain(raw: &str, markup: Markup) -> String {
    let text = match markup {
        Markup::Gbf => gbf(raw),
        Markup::Plaintext => raw.to_owned(),
        _ => xml_like(raw, matches!(markup, Markup::Thml)),
    };
    collapse(&decode_entities(&text))
}

/// R17c: Strong-Nummern eines Eintrags in der Reihenfolge des Textes, ohne Doppel, als „H1234“/„G26“ ohne
/// führende Nullen (Zusätze wie „a“ hinter der Zahl fallen weg). Erkannt: OSIS `lemma="strong:H01234 …"`, ThML
/// `<sync type="Strongs" value="G26"/>`, GBF `<WH1234>`/`<WG26>` (Morphologie `<WT…>` nicht). Eine Nummer ohne
/// Buchstaben bekommt `default_prefix` (`H` im Alten, `G` im Neuen Testament). Reiner Text: leer.
pub fn strongs(raw: &str, markup: Markup, default_prefix: char) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if markup == Markup::Plaintext {
        return out;
    }
    let mut push = |value: &str| {
        if let Some(number) = strong_number(value, default_prefix)
            && !out.contains(&number)
        {
            out.push(number);
        }
    };
    let mut rest = raw;
    while let Some(lt) = rest.find('<') {
        let Some(gt) = rest[lt..].find('>') else { break };
        let tag = &rest[lt + 1..lt + gt];
        rest = &rest[lt + gt + 1..];
        // GBF: <WH1234>, <WG26> (Großbuchstaben, direkt die Zahl).
        if let Some(num) = tag.strip_prefix("WH").or_else(|| tag.strip_prefix("WG"))
            && num.starts_with(|c: char| c.is_ascii_digit())
        {
            push(&tag[1..]);
            continue;
        }
        let name_len = tag.find(|c: char| c.is_whitespace()).unwrap_or(tag.len());
        let name = tag[..name_len].to_ascii_lowercase();
        let attrs = &tag[name_len..];
        if name == "w" {
            if let Some(lemma) = attribute(attrs, "lemma") {
                for token in lemma.split_whitespace() {
                    if let Some(value) = token.strip_prefix("strong:") {
                        push(value);
                    }
                }
            }
        } else if name == "sync"
            && attribute(attrs, "type").is_some_and(|t| t.eq_ignore_ascii_case("Strongs"))
            && let Some(value) = attribute(attrs, "value")
        {
            push(value);
        }
    }
    out
}

/// Wert des Attributs `name` („name="…"“ oder „name='…'“).
fn attribute<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = attrs;
    while let Some(pos) = rest.find(name) {
        let before_ok = pos == 0 || rest[..pos].ends_with(|c: char| c.is_whitespace());
        let after = rest[pos + name.len()..].trim_start();
        rest = &rest[pos + name.len()..];
        if !before_ok {
            continue;
        }
        let Some(value) = after.strip_prefix('=') else { continue };
        let value = value.trim_start();
        let quote = value.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let body = &value[1..];
        return body.find(quote).map(|end| &body[..end]);
    }
    None
}

/// „H01234“ → „H1234“, „g26a“ → „G26“, „26“ → `default_prefix` + „26“; `None` ohne Zahl oder bei 0.
fn strong_number(value: &str, default_prefix: char) -> Option<String> {
    let value = value.trim();
    let (prefix, digits) = match value.chars().next()? {
        c @ ('H' | 'h' | 'G' | 'g') => (c.to_ascii_uppercase(), &value[1..]),
        _ => (default_prefix, value),
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    let trimmed = digits.trim_start_matches('0');
    (!trimmed.is_empty()).then(|| format!("{prefix}{trimmed}"))
}

/// Elemente, deren Inhalt ganz wegfällt.
fn dropped(name: &str, attrs: &str, thml: bool) -> bool {
    match name {
        "note" | "rdg" | "catchword" | "figure" => true,
        // Überschriften, außer kanonischen (Psalmüberschrift als Vers 1 in deutscher Zählung).
        "title" | "head" => !attrs.contains("canonical=\"true\""),
        "scripref" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => thml,
        _ => false,
    }
}

/// Elemente, die Wörter trennen (Zeilen, Absätze, Umbrüche).
fn separates(name: &str) -> bool {
    matches!(name, "lb" | "l" | "lg" | "p" | "div" | "br" | "chapter" | "verse" | "milestone" | "list" | "item")
}

fn xml_like(raw: &str, thml: bool) -> String {
    let mut out = String::with_capacity(raw.len());
    // Offene Elemente mit „fällt weg“.
    let mut stack: Vec<(String, bool)> = Vec::new();
    let mut rest = raw;
    while let Some(lt) = rest.find('<') {
        let skipping = stack.iter().any(|(_, d)| *d);
        if !skipping {
            out.push_str(&rest[..lt]);
        }
        let Some(gt) = rest[lt..].find('>') else {
            // Kein Ende: Rest als Text (kaputte Auszeichnung lieber zeigen als verschlucken).
            if !skipping {
                out.push_str(&rest[lt..]);
            }
            rest = "";
            break;
        };
        let tag = &rest[lt + 1..lt + gt];
        rest = &rest[lt + gt + 1..];
        let closing = tag.starts_with('/');
        let self_closing = tag.ends_with('/');
        let body = tag.trim_start_matches('/').trim_end_matches('/');
        let name_len = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
        let name = body[..name_len].to_ascii_lowercase();
        let attrs = &body[name_len..];
        if separates(&name) {
            out.push(' ');
        }
        if closing {
            if let Some(pos) = stack.iter().rposition(|(n, _)| *n == name) {
                stack.truncate(pos);
            }
        } else if !self_closing && !name.starts_with(['!', '?']) {
            stack.push((name.clone(), dropped(&name, attrs, thml)));
        }
    }
    if !stack.iter().any(|(_, d)| *d) {
        out.push_str(rest);
    }
    out
}

/// GBF: `<RF>…<Rf>` (Fußnote) und `<TS>…<Ts>` (Überschrift) fallen weg, andere Marken (`<WG…>`, `<FI>`,
/// `<CM>` …) auch, ihr Text bleibt.
fn gbf(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut skip: Option<&str> = None;
    let mut rest = raw;
    while let Some(lt) = rest.find('<') {
        if skip.is_none() {
            out.push_str(&rest[..lt]);
        }
        let Some(gt) = rest[lt..].find('>') else {
            if skip.is_none() {
                out.push_str(&rest[lt..]);
            }
            return out;
        };
        let tag = &rest[lt + 1..lt + gt];
        rest = &rest[lt + gt + 1..];
        match (skip, tag) {
            (None, "RF") => skip = Some("Rf"),
            (None, "TS") => skip = Some("Ts"),
            (Some(end), t) if t == end => skip = None,
            (None, "CM" | "CL" | "Pp" | "PP") => out.push(' '),
            _ => {}
        }
    }
    if skip.is_none() {
        out.push_str(rest);
    }
    out
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let end = tail.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| entity(&tail[1..e]));
        match (end, decoded) {
            (Some(e), Some(c)) => {
                out.push(c);
                rest = &tail[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => {
            let num = name.strip_prefix('#')?;
            let code = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => num.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osis_notes_titles_and_strongs_are_dropped_and_words_kept() {
        let raw = "<title type=\"section\">Die Überschrift</title><w lemma=\"strong:G1\">Am</w> Anfang \
                   <note type=\"x-footnote\">eine <hi>Fußnote</hi></note>war der <divineName>Herr</divineName>\
                   <note type=\"crossReference\"><reference osisRef=\"Gen.1.1\">1Mo 1,1</reference></note>.";
        assert_eq!(to_plain(raw, Markup::Osis), "Am Anfang war der Herr.", "nur der Vers bleibt");
    }

    #[test]
    fn canonical_titles_stay_and_lines_separate_words() {
        let raw = "<title canonical=\"true\" type=\"psalm\">Ein Lied.</title><l>Erste Zeile</l><l>zweite</l>";
        assert_eq!(to_plain(raw, Markup::Osis), "Ein Lied. Erste Zeile zweite", "Psalmüberschrift ist Verstext");
    }

    #[test]
    fn gbf_footnotes_and_entities_are_handled() {
        assert_eq!(to_plain("Ein<RF>Note<Rf> Wort<WG25> &amp; mehr", Markup::Gbf), "Ein Wort & mehr", "GBF");
        assert_eq!(to_plain("a &#x2013; b &unknown; c", Markup::Plaintext), "a – b &unknown; c", "Entitäten");
    }

    #[test]
    fn strong_numbers_are_read_from_osis_thml_and_gbf_in_order_without_duplicates() {
        let osis = "<w lemma=\"strong:H07225 lemma.TR:x\">Anfang</w> <w lemma='strong:H430'>Gott</w> \
                    <w lemma=\"strong:H7225\">wieder</w> <w morph=\"x\">ohne</w>";
        assert_eq!(strongs(osis, Markup::Osis, 'H'), ["H7225", "H430"], "führende Null weg, Doppel einmal");
        let thml = "Liebe<sync type=\"Strongs\" value=\"G26\"/> und<sync type=\"morph\" value=\"G1\"/> \
                    Treue<sync type=\"Strongs\" value=\"4102\"/>";
        assert_eq!(strongs(thml, Markup::Thml, 'G'), ["G26", "G4102"], "nur type=Strongs; ohne Buchstaben G im NT");
        assert_eq!(
            strongs("Wort<WG26><WTN-NSF> zwei<WH0430a>", Markup::Gbf, 'G'),
            ["G26", "H430"],
            "GBF ohne Morphologie"
        );
        assert!(strongs("Der Kiesel lag.", Markup::Osis, 'H').is_empty(), "ohne Auszeichnung leer");
        assert!(
            strongs("<w lemma=\"strong:G26\">x</w>", Markup::Plaintext, 'G').is_empty(),
            "reiner Text: keine Marken"
        );
    }
}
