// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 22 (spiegel-entwurf, Vertrag §2.9): Das Wasserzeichen „Entwurf“ gesetzt. Die Quelle ist gebaut wie im Server
//! (Werkvorlage, dann [`draft_rule`], dann der Text); gemessen mit [`baruch_render::page_texts`]. Alle Texte frei
//! erfunden.

use baruch_render::{Severity, Source, compile, page_texts};
use baruch_typst::typeset::{Draft, DraftPlacement, draft_rule};

const TEMPLATE: &str = "#set page(paper: \"a5\")\n#set text(lang: \"de\")\n";

fn body() -> String {
    let mut out = String::from("= Der Leuchtturm\n\n");
    for i in 0..30 {
        out.push_str(&format!(
            "Absatz {i}: Die Wärterin stieg jeden Abend die hundert Stufen hinauf und putzte das Glas der Lampe.\n\n"
        ));
    }
    out
}

fn source(rule: &str) -> Source {
    Source { main: format!("{TEMPLATE}{rule}{}", body()), files: Vec::new() }
}

/// Text jeder Seite (Stücke hintereinander, geschützte Leerzeichen als Leerzeichen).
fn texts(source: &Source) -> Vec<String> {
    page_texts(source)
        .expect("die Quelle lässt sich setzen")
        .into_iter()
        .map(|page| page.iter().map(|t| t.text.replace('\u{a0}', " ")).collect::<Vec<_>>().join(""))
        .collect()
}

fn errors(source: &Source) -> Vec<String> {
    compile(source).diagnostics.into_iter().filter(|d| d.severity == Severity::Error).map(|d| d.message).collect()
}

#[test]
fn every_placement_puts_the_text_on_every_page_without_changing_the_page_count() {
    let plain = source("");
    let plain_pages = compile(&plain).pages.len();
    assert!(plain_pages >= 2, "das Testwerk braucht mehrere Seiten ({plain_pages})");
    for placement in DraftPlacement::ALL {
        let draft = Draft { text: "Probefassung".into(), placement: *placement, ..Draft::default() };
        let with = source(&draft_rule(&draft, None));
        assert_eq!(errors(&with), Vec::<String>::new(), "{placement:?}: die Regel setzt fehlerfrei");
        let pages = texts(&with);
        assert_eq!(pages.len(), plain_pages, "{placement:?}: das Wasserzeichen ändert den Umbruch nicht");
        for (n, page) in pages.iter().enumerate() {
            assert!(page.contains("Probefassung"), "{placement:?}: Seite {} trägt das Wasserzeichen: {page}", n + 1);
        }
    }
}

#[test]
fn text_note_and_date_stay_plain_strings_even_with_typst_syntax() {
    // Zeichen, die als Markup oder Code Schaden anrichten könnten: Anführungszeichen, Rückstrich, Raute, Klammern.
    let draft = Draft {
        text: "A\"B\\C#D]E[F Ä€".into(),
        note: "#set text(red) ] \" \\ Zeile\nzwei".into(),
        date: true,
        ..Draft::default()
    };
    let rule = draft_rule(&draft, Some("10.11.2026"));
    let with = source(&rule);
    assert_eq!(errors(&with), Vec::<String>::new(), "kein Satzfehler durch Sonderzeichen: {rule}");
    let first = &texts(&with)[0];
    assert!(first.contains("A\"B\\C#D]E[F Ä€"), "der Text erscheint wörtlich (Zeichenkette, kein Markup): {first}");
    assert!(first.contains("Stand 10.11.2026"), "das übergebene Datum erscheint: {first}");
    assert!(first.contains("#set text(red)"), "der Zusatz erscheint wörtlich statt ausgeführt zu werden: {first}");
}
