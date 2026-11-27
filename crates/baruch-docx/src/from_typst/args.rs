// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read the arguments of the islands `#figure(…)`, `#image(…)` and `#table(…)` – only as far as needed for figure
//! and table. What is not understood with certainty stays the caller's business (island).

/// An argument of a call: named (`name: value`) or positional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Arg<'a> {
    pub name: Option<&'a str>,
    pub value: &'a str,
}

/// End of the bracket that starts at `open` (index after the closing bracket); strings (except in the content
/// block `[…]`, where `"` is text) and nested brackets are skipped. `None` = not closed.
fn closing(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut stack: Vec<u8> = Vec::new();
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'"' if stack.last() != Some(&b']') => {
                i = string_end(text, i)?;
                continue;
            }
            b'(' => stack.push(b')'),
            b'[' => stack.push(b']'),
            b'{' => stack.push(b'}'),
            c @ (b')' | b']' | b'}') => {
                if stack.pop() != Some(c) {
                    return None;
                }
                if stack.is_empty() {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Index after the closing quotation mark of the string that starts at `open`.
fn string_end(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = open + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'"' => return Some(i + 1),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Content of the bracket of a call `prefix…)`, if the call makes up the whole text (a label `<…>` after it is
/// allowed). `prefix` ends with the opening bracket.
fn call_body<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let text = text.trim();
    if !text.starts_with(prefix) {
        return None;
    }
    let open = prefix.len() - 1;
    let end = closing(text, open)?;
    let rest = text[end..].trim();
    let label = rest.len() > 2
        && rest.starts_with('<')
        && rest.ends_with('>')
        && !rest[1..rest.len() - 1].contains(['<', '>', ' ']);
    (rest.is_empty() || label).then(|| &text[open + 1..end - 1])
}

/// Split arguments at the commas of the top level; `None` for unbalanced brackets.
pub(super) fn split(body: &str) -> Option<Vec<Arg<'_>>> {
    let bytes = body.as_bytes();
    let mut pieces: Vec<&str> = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i = string_end(body, i)?;
                continue;
            }
            b'(' | b'[' | b'{' => {
                i = closing(body, i)?;
                continue;
            }
            b',' => {
                pieces.push(&body[start..i]);
                start = i + 1;
            }
            b')' | b']' | b'}' => return None,
            _ => {}
        }
        i += 1;
    }
    pieces.push(&body[start..]);
    Some(pieces.into_iter().map(str::trim).filter(|p| !p.is_empty()).map(arg).collect())
}

fn arg(piece: &str) -> Arg<'_> {
    let name_len = piece.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_').count();
    let named =
        name_len > 0 && !piece.as_bytes()[0].is_ascii_digit() && piece[name_len..].trim_start().starts_with(':');
    if named {
        let value = piece[name_len..].trim_start()[1..].trim();
        return Arg { name: Some(&piece[..name_len]), value };
    }
    Arg { name: None, value: piece }
}

/// Value of a string `"…"` with resolved escapes.
fn string_value(literal: &str) -> Option<String> {
    let literal = literal.trim();
    if literal.len() < 2 || !literal.starts_with('"') || string_end(literal, 0) != Some(literal.len()) {
        return None;
    }
    let mut out = String::new();
    let mut chars = literal[1..literal.len() - 1].chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            other => out.push(other),
        }
    }
    Some(out)
}

/// Content of a content block `[…]` (the whole value), without the brackets.
fn content_value(value: &str) -> Option<&str> {
    let value = value.trim();
    (value.starts_with('[') && closing(value, 0) == Some(value.len())).then(|| &value[1..value.len() - 1])
}

/// What can be read from a figure.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Figure {
    pub alt: Option<String>,
    /// Caption as Typst markup.
    pub caption: Option<String>,
}

fn image_alt(body: &str) -> Option<String> {
    split(body)?.iter().find(|a| a.name == Some("alt")).and_then(|a| string_value(a.value))
}

/// Alternative text and caption from `#figure(image("…", alt: "…"), caption: […])` or
/// `#image("…", alt: "…")`.
pub(super) fn figure(raw: &str) -> Figure {
    let mut figure = Figure::default();
    if let Some(body) = call_body(raw, "#image(") {
        figure.alt = image_alt(body);
    } else if let Some(args) = call_body(raw, "#figure(").and_then(split) {
        for arg in &args {
            match arg.name {
                Some("caption") => figure.caption = content_value(arg.value).map(str::to_owned),
                None => {
                    if let Some(body) = call_body(arg.value, "image(") {
                        figure.alt = image_alt(body);
                    }
                }
                _ => {}
            }
        }
    }
    figure
}

/// An understood table: rows of cells (Typst markup per cell).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Table {
    pub header_rows: usize,
    pub rows: Vec<Vec<String>>,
    /// Styling (named arguments except `columns`, lines) was dropped.
    pub simplified: bool,
}

fn column_count(value: &str) -> Option<usize> {
    let value = value.trim();
    if let Ok(n) = value.parse::<usize>() {
        return Some(n);
    }
    if value.starts_with('(') && closing(value, 0) == Some(value.len()) {
        return Some(split(&value[1..value.len() - 1])?.len());
    }
    None
}

/// `#table(columns: …, […], …)`; `None` if it contains anything other than cells with content, `table.header(…)`
/// and lines, the number of columns is unclear or the header cells do not add up.
pub(super) fn table(raw: &str) -> Option<Table> {
    let args = split(call_body(raw, "#table(")?)?;
    let mut table = Table::default();
    let mut columns = 1usize;
    let mut header: Vec<String> = Vec::new();
    let mut cells: Vec<String> = Vec::new();
    for arg in &args {
        match arg.name {
            Some("columns") => columns = column_count(arg.value)?,
            Some(_) => table.simplified = true,
            None => {
                if let Some(content) = content_value(arg.value) {
                    cells.push(content.to_owned());
                } else if let Some(body) = call_body(arg.value, "table.header(") {
                    if !cells.is_empty() || !header.is_empty() {
                        return None;
                    }
                    for cell in split(body)? {
                        match (cell.name, content_value(cell.value)) {
                            (None, Some(content)) => header.push(content.to_owned()),
                            (Some(_), _) => table.simplified = true,
                            _ => return None,
                        }
                    }
                } else if matches!(arg.value.trim(), "table.hline()" | "table.vline()") {
                    table.simplified = true;
                } else {
                    return None;
                }
            }
        }
    }
    if columns == 0 || columns > 63 || !header.len().is_multiple_of(columns) || (header.is_empty() && cells.is_empty())
    {
        return None;
    }
    table.header_rows = header.len() / columns;
    let mut all = header;
    all.extend(cells);
    while !all.len().is_multiple_of(columns) {
        all.push(String::new());
    }
    table.rows = all.chunks(columns).map(<[String]>::to_vec).collect();
    Some(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_split_only_at_top_level_commas() {
        let args = split(r#"image("a,b.png", alt: "Haus, rot"), caption: [Ein Haus, rot (links)]"#).expect("lesbar");
        assert_eq!(args.len(), 2, "Kommas in Zeichenketten, Klammern und Inhaltsblöcken trennen keine Argumente");
        assert_eq!(args[1].name, Some("caption"), "das zweite Argument ist benannt");
        assert_eq!(args[1].value, "[Ein Haus, rot (links)]", "der Wert steht ohne den Namen da");
        assert!(split("[a], b)").is_none(), "eine überzählige schließende Klammer ist kein lesbarer Aufruf");
    }

    #[test]
    fn figure_yields_alt_text_and_caption() {
        let read = figure(
            r#"#figure(image("bilder/haus.png", alt: "Ein \"rotes\" Haus", width: 80%), caption: [Das _Haus_]) <abb>"#,
        );
        assert_eq!(read.alt.as_deref(), Some("Ein \"rotes\" Haus"), "Alternativtext mit aufgelöster Maskierung");
        assert_eq!(
            read.caption.as_deref(),
            Some("Das _Haus_"),
            "die Unterschrift bleibt Typst-Markup für den Aufrufer"
        );
        let plain = figure(r#"#image("bilder/haus.png")"#);
        assert_eq!(plain, Figure::default(), "ein Bild ohne Angaben hat weder Alternativtext noch Unterschrift");
    }

    #[test]
    fn table_reads_cells_and_header_and_refuses_what_it_does_not_know() {
        let source = "#table(\n  columns: (1fr, auto),\n  stroke: none,\n  table.header([Sorte], [Zeit]),\n  \
                      [Bohne], [Mai],\n  [Kürbis],\n)";
        let read = table(source).expect("einfache Tabelle");
        assert_eq!(read.header_rows, 1, "table.header mit so vielen Zellen wie Spalten ist eine Kopfzeile");
        assert_eq!(read.rows.len(), 3, "Kopf und zwei Zeilen");
        assert_eq!(
            read.rows[2],
            vec!["Kürbis".to_owned(), String::new()],
            "die letzte Zeile wird mit einer leeren Zelle aufgefüllt"
        );
        assert!(read.simplified, "stroke geht nicht mit – das muss der Aufrufer melden können");
        assert!(
            table("#table(columns: 2, [a], table.cell(colspan: 2)[b])").is_none(),
            "verbundene Zellen versteht der Export nicht – das bleibt eine Insel"
        );
        assert!(table("#table(columns: n, [a])").is_none(), "eine berechnete Spaltenzahl ist nicht sicher lesbar");
    }
}
