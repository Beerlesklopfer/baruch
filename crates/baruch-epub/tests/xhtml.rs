// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 20a (epub): mapping of the block model to XHTML ([`baruch_epub::xhtml`]) – text freely invented.

use baruch_epub::xhtml::{Links, NoteTarget, Renderer, escape_text, note_marker};
use baruch_typst::typeset::FootnoteNumbering;

fn render(body: &str) -> (String, Vec<String>) {
    let mut links = Links::default();
    links.key_files.insert(7, "k7.xhtml".into());
    links.key_titles.insert(7, "3 Der Stall".into());
    links.labels.insert("brunnen".into(), "k7.xhtml".into());
    let mut r = Renderer::new(&links, "k5.xhtml", NoteTarget::SameFile, FootnoteNumbering::Arabic, 1);
    let mut out = String::new();
    r.section(&mut out, 5, 1, Some("2"), "Der Hof", body);
    (out, r.warnings.iter().map(|w| w.code.clone()).collect())
}

#[test]
fn escape_text_masks_markup_and_drops_forbidden_control_characters() {
    assert_eq!(escape_text("a<b>&\"c'\u{1}\td"), "a&lt;b&gt;&amp;&quot;c&#39;\td", "maskiert, Steuerzeichen weg");
}

#[test]
fn the_section_heading_gets_its_key_id_and_number() {
    let (out, _) = render("= Der Hof <k5>\n\nText.\n");
    assert!(out.starts_with("<h1 id=\"k5\"><span class=\"baruch-number\">2</span> Der Hof</h1>"), "{out}");
    assert!(!out.contains("&lt;k5&gt;"), "Sprungmarke nicht als Text");
}

#[test]
fn emphasis_lists_and_terms_map_to_html_elements() {
    let body = "= Der Hof\n\nDas _alte_ Tor ist *schwer*.\n\n- Heu\n  - trocken\n- Stroh\n\n3. drei\n4. vier\n\n/ Tenne: Dreschboden\n";
    let (out, warnings) = render(body);
    assert!(out.contains("<p>Das <em>alte</em> Tor ist <strong>schwer</strong>.</p>"), "Hervorhebungen: {out}");
    assert!(out.contains("<ul><li>Heu<ul><li>trocken</li></ul></li><li>Stroh</li></ul>"), "geschachtelte Liste: {out}");
    assert!(out.contains("<ol start=\"3\"><li>drei</li><li>vier</li></ol>"), "Aufzählung mit Start: {out}");
    assert!(out.contains("<dl>\n<dt>Tenne</dt>\n<dd>Dreschboden</dd>\n</dl>"), "Begriffsliste: {out}");
    assert!(warnings.is_empty(), "alles abbildbar: {warnings:?}");
}

#[test]
fn quotes_scenes_and_works_become_blockquote_and_section_classes() {
    let body = "= Der Hof\n\n#quote(block: true, attribution: [Die Bäuerin])[Erst das Vieh, dann wir.]\n\n\
                #szene[Im Stall ist es warm.]\n\n#arbeit[Wann hast du zuletzt geruht?]\n\nSie sagte #quote[nein].\n";
    let (out, _) = render(body);
    assert!(out.contains("<blockquote class=\"baruch-quote\">"), "Blockzitat: {out}");
    assert!(out.contains("<footer class=\"baruch-attribution\">— Die Bäuerin</footer>"), "Quelle: {out}");
    assert!(out.contains("<section class=\"baruch-scene\">\n<p>Im Stall ist es warm.</p>"), "Szene: {out}");
    assert!(out.contains("<section class=\"baruch-work\">"), "Arbeit: {out}");
    assert!(out.contains("<q>nein</q>"), "Zitat im Satz: {out}");
}

#[test]
fn scene_arguments_are_reported_because_the_epub_cannot_show_them() {
    let (out, warnings) = render("= Der Hof\n\n#szene(ort: [Küche])[Es duftet.]\n");
    assert!(out.contains("<p>Es duftet.</p>"), "Rumpf bleibt: {out}");
    assert_eq!(warnings, vec!["ARGS_OMITTED"], "Angaben nicht still weg");
}

#[test]
fn only_safe_links_become_links() {
    let body = "= Der Hof\n\n#link(\"https://example.org\")[Seite] und #link(\"javascript:alert(1)\")[böse].\n";
    let (out, _) = render(body);
    assert!(out.contains("<a href=\"https://example.org\">Seite</a>"), "https wird Link: {out}");
    assert!(!out.contains("javascript:"), "anderes Schema nur als Text: {out}");
    assert!(out.contains("böse"), "Text bleibt");
}

#[test]
fn references_and_labels_link_within_the_book() {
    let body = "= Der Hof\n\nAm Brunnen <quelle> steht ein Eimer, siehe @brunnen und @k7.\n";
    let (out, _) = render(body);
    assert!(out.contains("<span id=\"lbl-quelle\"></span>"), "Label als Ziel: {out}");
    assert!(out.contains("<a class=\"baruch-ref\" href=\"k7.xhtml#lbl-brunnen\">brunnen</a>"), "@label: {out}");
    assert!(out.contains("<a class=\"baruch-ref\" href=\"k7.xhtml#k7\">3 Der Stall</a>"), "@kN mit Titel: {out}");
}

#[test]
fn a_footnote_inside_a_link_stays_valid_xhtml() {
    let body = "= Der Hof\n\n#link(\"https://example.org\")[Hof#footnote[Am Bach.]]\n";
    let (out, _) = render(body);
    assert!(!out.contains("<a href=\"https://example.org\">Hof<sup><a"), "kein Link im Link: {out}");
    assert!(out.contains("<sup id=\"fnref1\">1</sup>"), "Zeichen ohne Link: {out}");
}

#[test]
fn a_text_without_heading_gets_the_outline_title() {
    let (out, _) = render("Nur ein Absatz.\n");
    assert!(
        out.starts_with("<h1 id=\"k5\"><span class=\"baruch-number\">2</span> Der Hof</h1>\n<p>Nur ein Absatz.</p>"),
        "{out}"
    );
}

#[test]
fn footnote_markers_follow_the_numbering() {
    assert_eq!(note_marker(FootnoteNumbering::Arabic, 12), "12", "arabisch");
    assert_eq!(note_marker(FootnoteNumbering::Letters, 1), "a", "Buchstaben");
    assert_eq!(note_marker(FootnoteNumbering::Letters, 27), "aa", "nach z geht es mit aa weiter (wie lower-alpha)");
    assert_eq!(note_marker(FootnoteNumbering::Symbols, 4), "**", "nach ‡ verdoppelt");
}
