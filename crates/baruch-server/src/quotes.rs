// SPDX-License-Identifier: AGPL-3.0-or-later
//! Finding quotations in the section text (Round 17, interface R17-S2): one place for all paths that assign
//! quotations to a Bible passage (wording check, check display). Quotations in quotation marks and `#quote[…]`
//! come from [`baruch_scripture::wording::inline_quotes`] (since Round 17b also nested and across paragraphs),
//! block quotations `#quote(…)[…]` on the top level from the paragraph parser of `baruch_wysiwyg`; block
//! quotations in an island such as `#szene[…]` are found since Round 17b by a search for
//! `#quote(block: true …)[…]` in the whole text.

use baruch_scripture::wording::{self, QuoteSpan};

/// All quotations in `text`, sorted by `start`, without duplicates.
pub fn quote_spans(text: &str) -> Vec<QuoteSpan> {
    let mut out = wording::inline_quotes(text);
    out.extend(block_quotes(text));
    out.extend(nested_block_quotes(text));
    // Sorted stably: with the same content the block quotation of the top level (its range covers the whole
    // paragraph) stays before the one of the search.
    out.sort_by_key(|q| (q.start, q.end));
    out.dedup_by_key(|q| (q.start, q.end));
    out
}

/// Block quotations `#quote(… block: true …)[…]` at any place, also in islands such as `#szene[…]` (Round 17b).
/// Strings in the arguments are respected; an escaped `\#quote` does not count.
fn nested_block_quotes(text: &str) -> Vec<QuoteSpan> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = text[from..].find("#quote(").map(|p| p + from) {
        from = pos + "#quote(".len();
        if text[..pos].ends_with('\\') {
            continue;
        }
        let args = from;
        let mut depth = 1i32;
        let mut in_string = false;
        let mut escaped = false;
        let mut close = None;
        for (k, c) in text[args..].char_indices() {
            if in_string {
                match (escaped, c) {
                    (true, _) => escaped = false,
                    (false, '\\') => escaped = true,
                    (false, '"') => in_string = false,
                    _ => {}
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                close = Some(args + k);
                break;
            }
        }
        let Some(close) = close else { continue };
        let compact: String = text[args..close].chars().filter(|c| !c.is_whitespace()).collect();
        if !compact.split(',').any(|arg| arg == "block:true") || !text[close + 1..].starts_with('[') {
            continue;
        }
        let open = close + 2;
        let Some(len) = wording::bracket_end(&text[open..]) else { continue };
        out.push(QuoteSpan { start: open, end: open + len, outer: (pos, open + len + 1), block: true });
    }
    out
}

/// Block quotations `#quote(…)[…]` on the top level (paragraph kinds of `baruch_wysiwyg::parse`), as the range
/// of the content in `text`.
fn block_quotes(text: &str) -> Vec<QuoteSpan> {
    let doc = baruch_wysiwyg::parse(text);
    let mut out = Vec::new();
    let mut at = 0;
    for block in &doc.blocks {
        let start = at;
        at += block.raw.len();
        if !matches!(block.kind, baruch_wysiwyg::BlockKind::Quote { .. }) || !text.is_char_boundary(at) {
            continue;
        }
        let raw = &text[start..at];
        let Some(args) = raw.find("#quote(").map(|p| p + "#quote(".len()) else { continue };
        let mut depth = 1i32;
        let mut close = None;
        for (k, c) in raw[args..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                close = Some(args + k);
                break;
            }
        }
        let Some(close) = close else { continue };
        let Some(open) = raw[close..].find('[').map(|p| close + p + 1) else { continue };
        let Some(len) = wording::bracket_end(&raw[open..]) else { continue };
        out.push(QuoteSpan {
            start: start + open,
            end: start + open + len,
            outer: (start, start + open + len + 1),
            block: true,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_quote_is_found_by_the_paragraph_parser() {
        let text = "Vorher.\n\n#quote(block: true, attribution: [Ps 23,1])[Der Kiesel lag.]\n\nNachher.\n";
        let spans = quote_spans(text);
        let blocks: Vec<_> = spans.iter().filter(|q| q.block).collect();
        assert_eq!(blocks.len(), 1, "ein Blockzitat: {spans:?}");
        assert_eq!(&text[blocks[0].start..blocks[0].end], "Der Kiesel lag.", "Inhalt des Blockzitats");
    }

    #[test]
    fn a_block_quote_inside_a_scene_is_found() {
        let text = "Vorher.\n\n#szene(ort: [Hof])[\n  Sie sprachen.\n  #quote(block: true, attribution: [Ps 23,1])[Der Hirte \
                    wacht.]\n]\n\nNachher.\n";
        let spans = quote_spans(text);
        let blocks: Vec<_> = spans.iter().filter(|q| q.block).collect();
        assert_eq!(
            blocks.len(),
            1,
            "das Blockzitat in der Szene (Insel für den Absatz-Parser) wird gefunden: {spans:?}"
        );
        assert_eq!(&text[blocks[0].start..blocks[0].end], "Der Hirte wacht.", "Inhalt ohne Klammern");
        let refs = baruch_scripture::scan(text);
        let q =
            baruch_scripture::wording::quote_for(text, &spans, &refs, 0).expect("Zitat zur Stelle in der Zuschreibung");
        assert_eq!(&text[q.start..q.end], "Der Hirte wacht.", "die Stelle in der Zuschreibung gehört zu diesem Zitat");
        assert!(
            quote_spans("Ein #quote(attribution: [x])[Spruch] im Satz.").iter().all(|q| !q.block),
            "ohne block: true kein Blockzitat (Typst setzt es im Fließtext)"
        );
    }

    #[test]
    fn a_quote_with_inner_quotes_and_one_over_two_paragraphs_are_assigned_to_their_reference() {
        let nested = "Jesus sagt: „Ich sage euch: ‚Steh auf‘, und geh heim.“ (Mk 2,11)";
        let spans = quote_spans(nested);
        assert_eq!(spans.len(), 1, "„… ‚…‘ …“ ist ein Zitat: {spans:?}");
        let refs = baruch_scripture::scan(nested);
        let q = baruch_scripture::wording::quote_for(nested, &spans, &refs, 0).expect("Zitat vor der Stelle");
        assert_eq!(&nested[q.start..q.end], "Ich sage euch: ‚Steh auf‘, und geh heim.", "das ganze äußere Zitat");

        let long = "Er las: „Der Kiesel lag am Bach.\n\n„Und die Amsel sang.“ (Joh 3,16)";
        let spans = quote_spans(long);
        assert_eq!(
            spans.len(),
            1,
            "ein Zitat über zwei Absätze (Anführungszeichen am Absatzanfang wiederholt): {spans:?}"
        );
        let refs = baruch_scripture::scan(long);
        let q = baruch_scripture::wording::quote_for(long, &spans, &refs, 0).expect("Zitat vor der Stelle");
        assert!(long[q.start..q.end].starts_with("Der Kiesel"), "das Zitat beginnt im ersten Absatz");

        let unclosed = "„Offen geblieben.\n\nDann sagte er “Ja” und ging.";
        let spans = quote_spans(unclosed);
        assert_eq!(spans.len(), 1, "ein nie geschlossenes Zitat schluckt nicht das nächste: {spans:?}");
        assert_eq!(&unclosed[spans[0].start..spans[0].end], "Ja", "das englische Zitat im zweiten Absatz bleibt");
    }

    #[test]
    fn spans_are_sorted_by_start_without_duplicates() {
        let text = "„Eins“ und\n\n#quote(block: true)[Zwei]\n\nund „Drei“.\n";
        let spans = quote_spans(text);
        let starts: Vec<usize> = spans.iter().map(|q| q.start).collect();
        let mut sorted = starts.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(starts, sorted, "nach Anfang sortiert und ohne Doppel: {spans:?}");
        assert_eq!(spans.len(), 3, "zwei Zitate in Anführungszeichen und ein Blockzitat: {spans:?}");
    }
}
