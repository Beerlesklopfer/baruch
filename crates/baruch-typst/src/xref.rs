// SPDX-License-Identifier: AGPL-3.0-or-later
//! Querverweise auf Gliederungspunkte (Runde 30a).
//!
//! - Jeder Gliederungspunkt hat eine feste Sprungmarke `<k{key}>` ([`key_label`]); projiziert wird sie nur beim
//!   Export, Spiegel und Satz ([`label_heading`], [`label_section`]), die Texte in der Datenbank bleiben unverändert.
//!   - Eine `=`-Überschriftszeile ohne eigenes Label bekommt ` <kN>` angehängt (vor einem Kommentar):
//!     `= Titel <k5> // Notiz`.
//!   - Eine Überschrift mit eigenem Label (Teil `<baruch-part>`, „ohne Nummer“ `<baruch-unnumbered>`, ein Label des
//!     Menschen am Zeilenende oder allein auf der Zeile darunter) behält es; darunter (hinter einer Label-Zeile)
//!     steht die Ankerzeile [`anchor_line`] `#metadata("baruch-key") <kN>`. Typst kennt nur ein Label je Element.
//! - Ein Verweis im Text hat die Form `#link(<k103>)[die Einleitung]`, mit Seitenzahl
//!   `#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]` ([`xref_snippet`]);
//!   [`find_xrefs`] findet beide Formen.
//! - Beim Einlesen entfernt [`strip_key_label`] (je Zeile), [`strip_section`] (je Abschnitt) bzw.
//!   [`strip_key_labels`] (ganze Datei) die Sprungmarken wieder; [`remap_keys`] setzt Verweise auf neue Schlüssel um.
//!
//! Erkannt werden nur die kanonischen Formen, die diese Funktionen selbst schreiben; Raw-Blöcke und Kommentare
//! verfolgen die Funktionen nicht (eine Zeile `#metadata("baruch-key") <k5>` im Raw-Block gälte als Ankerzeile).

use std::borrow::Cow;
use std::collections::HashMap;

use crate::marker::read_marker;
use crate::{comment_start, label_start, read_heading};

/// Anfang des Namens einer Sprungmarke: `k` + Schlüssel.
pub const KEY_LABEL_PREFIX: &str = "k";

/// Wert der Ankerzeile ([`anchor_line`]).
pub const ANCHOR_VALUE: &str = "baruch-key";

/// Name der Sprungmarke des Gliederungspunkts `key` (ohne spitze Klammern): `k103`.
pub fn key_label(key: i64) -> String {
    format!("{KEY_LABEL_PREFIX}{key}")
}

/// Schlüssel aus dem Namen einer Sprungmarke: `k103` → 103; alles andere (auch `<k103>`, `k`, `k-1`, `k01`) → `None`.
pub fn parse_key_label(label: &str) -> Option<i64> {
    let digits = label.strip_prefix(KEY_LABEL_PREFIX)?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || (digits.len() > 1 && digits.starts_with('0'))
    {
        return None;
    }
    digits.parse().ok()
}

/// Unsichtbare Ankerzeile (ohne Zeilenende) für Überschriften mit eigenem Label: `#metadata("baruch-key") <k5>`.
pub fn anchor_line(key: i64) -> String {
    format!("#metadata(\"{ANCHOR_VALUE}\") <{}>", key_label(key))
}

/// Schlüssel einer Ankerzeile ([`anchor_line`], Zeilenende erlaubt), sonst `None`.
pub fn parse_anchor_line(line: &str) -> Option<i64> {
    let (content, _) = split_ending(line);
    let rest = content.strip_prefix(&format!("#metadata(\"{ANCHOR_VALUE}\") <"))?;
    parse_key_label(rest.strip_suffix('>')?)
}

/// Zeilenende abtrennen: (Inhalt, Zeilenende).
fn split_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = line.strip_suffix('\n') {
        (content, "\n")
    } else {
        (line, "")
    }
}

/// Art einer Überschriftszeile für die Projektion.
enum HeadingForm {
    /// `=`-Zeile ohne eigenes Label: Stelle, an der ` <kN>` eingefügt wird (Ende des Titels vor Leerraum und Kommentar).
    Plain { insert_at: usize },
    /// Teil, „ohne Nummer“ oder `=`-Zeile mit eigenem Label: Ankerzeile darunter.
    Labelled,
}

/// Form der Überschriftszeile `content` (ohne Zeilenende); `None` = keine Überschrift.
fn heading_form(content: &str) -> Option<HeadingForm> {
    if read_marker(content).is_some() {
        return Some(HeadingForm::Labelled);
    }
    read_heading(content)?;
    let level = content.bytes().take_while(|&b| b == b'=').count();
    let rest = &content[level..];
    let cut = comment_start(rest).unwrap_or(rest.len());
    let head = &rest[..cut];
    if label_start(head.trim()).is_some() {
        return Some(HeadingForm::Labelled);
    }
    Some(HeadingForm::Plain { insert_at: level + head.trim_end().len() })
}

/// Überschriftszeile mit Sprungmarke des Gliederungspunkts `key` (für Export, Spiegel, Satz). `=`-Zeile ohne eigenes
/// Label: ` <kN>` hinter dem Titel (vor Leerraum, Kommentar und Zeilenende). Mit eigenem Label (auch Teil und „ohne
/// Nummer“): die Zeile, ein Zeilenende und die Ankerzeile ([`anchor_line`]) mit dem Zeilenende der Zeile. Keine
/// Überschrift: unverändert. Ein Label allein auf der Zeile darunter kennt nur [`label_section`].
pub fn label_heading(line: &str, key: i64) -> String {
    let (content, ending) = split_ending(line);
    match heading_form(content) {
        None => line.to_owned(),
        Some(HeadingForm::Plain { insert_at }) => {
            format!("{} <{}>{}", &content[..insert_at], key_label(key), &line[insert_at..])
        }
        Some(HeadingForm::Labelled) => {
            let newline = if ending.is_empty() { "\n" } else { ending };
            format!("{content}{newline}{}{ending}", anchor_line(key))
        }
    }
}

/// Ist `line` (ohne Zeilenende) ein Label allein auf der Zeile (`<name>`, Einzug erlaubt)?
fn is_label_line(line: &str) -> bool {
    let trimmed = split_ending(line).0.trim();
    trimmed.starts_with('<') && label_start(trimmed) == Some(0)
}

/// Text eines Abschnitts mit Sprungmarke `key` an seiner ersten Zeile (die Überschrift, [`label_heading`]). Steht
/// direkt unter einer Überschrift ein Label allein auf der Zeile (`= Titel⏎<name>`), gehört es zur Überschrift; die
/// Ankerzeile kommt dann hinter das Label. Ohne Überschrift in der ersten Zeile: unverändert.
pub fn label_section(body: &str, key: i64) -> Cow<'_, str> {
    let mut lines = body.split_inclusive('\n');
    let Some(first) = lines.next() else { return Cow::Borrowed(body) };
    let (content, ending) = split_ending(first);
    match heading_form(content) {
        None => Cow::Borrowed(body),
        Some(HeadingForm::Plain { .. }) if !lines.clone().next().is_some_and(is_label_line) => {
            Cow::Owned(format!("{}{}", label_heading(first, key), &body[first.len()..]))
        }
        Some(_) => {
            // Ankerzeile hinter der Überschrift bzw. hinter der Label-Zeile darunter.
            let mut end = first.len();
            let mut last_ending = ending;
            if let Some(next) = lines.next().filter(|l| is_label_line(l)) {
                end += next.len();
                last_ending = split_ending(next).1;
            }
            let (head, rest) = body.split_at(end);
            let newline = if last_ending.is_empty() { "\n" } else { "" };
            let tail = if rest.is_empty() && last_ending.is_empty() { "" } else { "\n" };
            Cow::Owned(format!("{head}{newline}{}{tail}{rest}", anchor_line(key)))
        }
    }
}

/// Überschriftszeile ohne Sprungmarke und der Schlüssel, der darin stand (beim Einlesen). Eine `=`-Zeile verliert
/// ` <kN>` hinter dem Titel; eine Ankerzeile ([`anchor_line`]) wird ganz entfernt (leere Zeichenkette samt
/// Zeilenende). Alles andere bleibt geliehen und ohne Schlüssel.
pub fn strip_key_label(line: &str) -> (Cow<'_, str>, Option<i64>) {
    if let Some(key) = parse_anchor_line(line) {
        return (Cow::Borrowed(""), Some(key));
    }
    let (content, _) = split_ending(line);
    if read_heading(content).is_none() {
        return (Cow::Borrowed(line), None);
    }
    let level = content.bytes().take_while(|&b| b == b'=').count();
    let rest = &content[level..];
    let cut = comment_start(rest).unwrap_or(rest.len());
    let head = rest[..cut].trim_end();
    let end = level + head.len();
    let Some(open) = head.rfind(" <") else { return (Cow::Borrowed(line), None) };
    let Some(key) = head[open + 2..].strip_suffix('>').and_then(parse_key_label) else {
        return (Cow::Borrowed(line), None);
    };
    let start = level + open;
    (Cow::Owned(format!("{}{}", &line[..start], &line[end..])), Some(key))
}

/// Text eines Abschnitts ohne Sprungmarke (Umkehrung von [`label_section`]) und der Schlüssel daraus: die erste Zeile
/// über [`strip_key_label`], eine Ankerzeile direkt darunter oder hinter einer Label-Zeile wird entfernt.
pub fn strip_section(body: &str) -> (Cow<'_, str>, Option<i64>) {
    let mut lines = body.split_inclusive('\n');
    let Some(first) = lines.next() else { return (Cow::Borrowed(body), None) };
    let (head, key) = strip_key_label(first);
    if key.is_some() {
        return (Cow::Owned(format!("{head}{}", &body[first.len()..])), key);
    }
    let (content, _) = split_ending(first);
    if heading_form(content).is_none() {
        return (Cow::Borrowed(body), None);
    }
    // Ankerzeile direkt unter der Überschrift oder unter deren Label-Zeile.
    let mut at = first.len();
    let mut next = lines.next();
    if let Some(label) = next.filter(|l| is_label_line(l)) {
        at += label.len();
        next = lines.next();
    }
    let Some(anchor) = next else { return (Cow::Borrowed(body), None) };
    let Some(key) = parse_anchor_line(anchor) else { return (Cow::Borrowed(body), None) };
    let mut out = String::with_capacity(body.len());
    out.push_str(&body[..at]);
    let rest = &body[at + anchor.len()..];
    if rest.is_empty() && split_ending(anchor).1.is_empty() {
        // Die Ankerzeile war die letzte Zeile ohne Zeilenende: das davor ergänzte Zeilenende fällt mit ihr weg.
        if let Some(stripped) = out.strip_suffix('\n') {
            out.truncate(stripped.len());
        }
    }
    out.push_str(rest);
    (Cow::Owned(out), Some(key))
}

/// Anfang der Satz-Form einer Ankerzeile vor einem Teil ([`part_anchors_for_typesetting`]).
const PART_ANCHOR_BREAK: &str = "#pagebreak(weak: true, to: \"odd\")";

/// Nur für den Satz mit Teil-Seiten (`baruch_typst::part_page_rule`): Die Regel setzt den Teil allein auf eine rechte
/// Seite und bricht danach um – eine Ankerzeile hinter der Teil-Zeile läge so auf der Seite danach. Hier wandert sie
/// vor die Teil-Zeile, mit demselben weichen Umbruch auf eine rechte Seite davor:
/// `#pagebreak(weak: true, to: "odd")#metadata("baruch-key") <kN>` – der Umbruch der Regel entfällt dann, weil die
/// Seite nur den Anker trägt (belegt in `baruch-render/tests/xref.rs`). Zeilenzahl und Zeilen der übrigen Abschnitte
/// bleiben; Überschrift und Anker tauschen nur die Zeile. Ohne Teil mit Ankerzeile: geliehen.
pub fn part_anchors_for_typesetting(text: &str) -> Cow<'_, str> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len() + 40);
    let mut changed = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let anchored = lines.get(i + 1).and_then(|next| parse_anchor_line(next));
        if let (Some(marker), Some(key)) = (read_marker(line), anchored)
            && marker.level == 0
            && !split_ending(line).1.is_empty()
        {
            let (part, ending) = split_ending(line);
            let after = split_ending(lines[i + 1]).1;
            out.push_str(&format!("{PART_ANCHOR_BREAK}{}{ending}{part}{after}", anchor_line(key)));
            changed = true;
            i += 2;
            continue;
        }
        out.push_str(line);
        i += 1;
    }
    if changed { Cow::Owned(out) } else { Cow::Borrowed(text) }
}

/// Ganzer Text ohne Sprungmarken: in jeder Überschriftszeile ` <kN>` entfernt, Ankerzeilen entfernt (für Tests und
/// zum Vergleich eines Exports mit seiner Quelle). Ohne Sprungmarken bleibt der Text geliehen.
pub fn strip_key_labels(text: &str) -> Cow<'_, str> {
    if !text.contains(&format!("<{KEY_LABEL_PREFIX}")) {
        return Cow::Borrowed(text);
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for (i, line) in lines.iter().enumerate() {
        // Ein Byte-Order-Mark am Dateianfang gehört zur Werkvorlage; die Überschrift dahinter zählt trotzdem.
        let line = match line.strip_prefix(crate::BOM).filter(|_| i == 0) {
            Some(rest) => {
                out.push(crate::BOM);
                rest
            }
            None => line,
        };
        let (stripped, key) = strip_key_label(line);
        if key.is_some() {
            changed = true;
            if stripped.is_empty() && i + 1 == lines.len() && split_ending(line).1.is_empty() {
                // Letzte Ankerzeile ohne Zeilenende: das vorher ergänzte Zeilenende fällt mit ihr weg.
                if out.ends_with('\n') {
                    out.pop();
                }
            }
        }
        out.push_str(&stripped);
    }
    if changed { Cow::Owned(out) } else { Cow::Borrowed(text) }
}

/// Verweis im Text auf den Gliederungspunkt `key` mit dem Linktext `text` (Typst-Markup, wörtlich eingesetzt):
/// `#link(<k103>)[die Einleitung]`; mit Seitenzahl `#link(<k103>)[die Einleitung, S.~#context
/// counter(page).at(<k103>).first()]`.
pub fn xref_snippet(key: i64, text: &str, with_page: bool) -> String {
    let label = key_label(key);
    if with_page {
        format!("#link(<{label}>)[{text}{}]", page_suffix(key))
    } else {
        format!("#link(<{label}>)[{text}]")
    }
}

/// Seitenzahl-Zusatz im Linktext: `, S.~#context counter(page).at(<k103>).first()`.
fn page_suffix(key: i64) -> String {
    format!(", S.~#context counter(page).at(<{}>).first()", key_label(key))
}

/// Ein Verweis im Text: Bytes `start..end` der ganzen Form, Schlüssel, Linktext, mit Seitenzahl?
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct XrefUse {
    pub start: usize,
    pub end: usize,
    pub key: i64,
    pub text: String,
    pub page: bool,
}

/// Ende (exklusiv, hinter `]`) des Inhaltsblocks, der bei `open` (Byte von `[`) beginnt: Maskierungen `\X` und
/// geschachtelte Klammern zählen mit. Kein Schluss → `None`.
fn block_end(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                let next = text[i + 1..].chars().next().map_or(0, char::len_utf8);
                i += 1 + next;
                continue;
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Alle Verweise (`#link(<kN>)[…]`, auch mit Seitenzahl-Zusatz) in einem Text, in Textreihenfolge. Ein maskiertes
/// `\#link` zählt nicht.
pub fn find_xrefs(text: &str) -> Vec<XrefUse> {
    const OPEN: &str = "#link(<";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(OPEN) {
        let start = from + found;
        from = start + OPEN.len();
        if text[..start].ends_with('\\') && !text[..start].ends_with("\\\\") {
            continue;
        }
        let rest = &text[from..];
        let Some(close) = rest.find(">)[") else { continue };
        let Some(key) = parse_key_label(&rest[..close]) else { continue };
        let open = from + close + 2;
        let Some(end) = block_end(text, open) else { continue };
        let inner = &text[open + 1..end - 1];
        let suffix = page_suffix(key);
        let (inner, page) = match inner.strip_suffix(&suffix) {
            Some(shorter) => (shorter, true),
            None => (inner, false),
        };
        out.push(XrefUse { start, end, key, text: inner.to_owned(), page });
        from = end;
    }
    out
}

/// Verweise auf alte Schlüssel durch neue ersetzen (`map`: alt → neu), auch im Seitenzahl-Zusatz; Schlüssel ohne
/// Eintrag bleiben. Unveränderter Text bleibt geliehen.
pub fn remap_keys<'a>(text: &'a str, map: &HashMap<i64, i64>) -> Cow<'a, str> {
    let uses: Vec<XrefUse> =
        find_xrefs(text).into_iter().filter(|u| map.get(&u.key).is_some_and(|n| *n != u.key)).collect();
    if uses.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8 * uses.len());
    let mut at = 0;
    for u in uses {
        let new = map[&u.key];
        out.push_str(&text[at..u.start]);
        let old_label = format!("<{}>", key_label(u.key));
        let new_label = format!("<{}>", key_label(new));
        out.push_str(&text[u.start..u.end].replace(&old_label, &new_label));
        at = u.end;
    }
    out.push_str(&text[at..]);
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_labels_round_trip_and_reject_other_names() {
        assert_eq!(key_label(103), "k103", "Name = k + Schlüssel");
        assert_eq!(parse_key_label("k103"), Some(103), "Rückweg zum Schlüssel");
        for bad in ["<k103>", "k", "k-1", "k01", "x103", "k1a"] {
            assert_eq!(parse_key_label(bad), None, "{bad:?} ist keine Sprungmarke eines Gliederungspunkts");
        }
    }

    #[test]
    fn a_snippet_links_to_the_label_with_or_without_page() {
        assert_eq!(xref_snippet(103, "die Einleitung", false), "#link(<k103>)[die Einleitung]", "Form aus §3.3");
        assert_eq!(
            xref_snippet(103, "die Einleitung", true),
            "#link(<k103>)[die Einleitung, S.~#context counter(page).at(<k103>).first()]",
            "Form mit Seitenzahl aus §3.3"
        );
    }
}
