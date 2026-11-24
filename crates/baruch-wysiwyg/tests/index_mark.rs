// SPDX-License-Identifier: AGPL-3.0-or-later
//! Registermarken im Satzbild (Schnittstelle R31-S1, §2.3; R31c kontextmenue): die kanonische Marke
//! `#metadata((…))<baruch-idx>` ist `Inline::IndexMark`, `emit`/`parse` ergeben eine Rundreise, JSON
//! `{"t":"index_mark","mark":{…}}`; im Satzbild ein dezentes, nicht tippbares Element mit Beschreibung. Bearbeiten und
//! Entfernen über das Satzbild ändern nur die Marke, unberührte Blöcke bleiben byte-gleich. Texte frei erfunden.

use baruch_typst::index::{IndexMark, mark_source};
use baruch_wysiwyg::{Block, BlockKind, Edit, Inline, emit, index_mark_label, parse, render_html, serialize};

fn text(v: &str) -> Inline {
    Inline::Text { v: v.into() }
}

/// Inlines des einzigen Absatzes von `typst`.
fn inlines(typst: &str) -> Vec<Inline> {
    let doc = parse(typst);
    assert_eq!(doc.blocks.len(), 1, "ein Block aus {typst:?}");
    paragraph(&doc.blocks[0])
}

fn paragraph(block: &Block) -> Vec<Inline> {
    match &block.kind {
        BlockKind::Paragraph { inlines } => inlines.clone(),
        other => panic!("kein Absatz: {other:?}"),
    }
}

#[test]
fn a_canonical_mark_reads_as_index_mark_and_writes_back() {
    let mark = IndexMark { sub: "im Alltag".into(), ..IndexMark::new("stichwort", "Geduld") };
    let typst = format!("Die Geduld{} wächst langsam.", mark_source(&mark));
    let read = inlines(&typst);
    assert_eq!(
        read,
        vec![text("Die Geduld"), Inline::IndexMark { mark: mark.clone() }, text(" wächst langsam.")],
        "die Marke steht direkt hinter dem Wort und wird nicht zu Insel und Label"
    );
    let written = emit(&BlockKind::Paragraph { inlines: read });
    assert_eq!(written, typst, "emit schreibt mark_source, die Rundreise ist byte-gleich");
}

#[test]
fn json_follows_the_contract_and_reads_what_the_widget_sends() {
    let inline = Inline::IndexMark { mark: IndexMark::new("personen", "Hanna") };
    let json = serde_json::to_string(&inline).expect("serialisierbar");
    assert_eq!(
        json, r#"{"t":"index_mark","mark":{"register":"personen","term":"Hanna"}}"#,
        "JSON-Form laut §2.3, leere Zusätze fehlen"
    );
    // So schickt wysiwyg.ts eine Marke (nur gefüllte Felder) in einer Anweisung an wysiwyg.serialize.
    let edit: Edit = serde_json::from_str(
        r#"{"replaces":0,"block":{"kind":"paragraph","inlines":[{"t":"text","v":"Hanna"},
            {"t":"index_mark","mark":{"register":"personen","term":"Hanna","also":"Elkana"}}]}}"#,
    )
    .expect("die Form des Widgets ist gültiges JSON für Edit");
    let doc = parse("Alt\n");
    let body = serialize(&doc, &[edit]).expect("serialisierbar");
    assert_eq!(
        body,
        format!("Hanna{}\n", mark_source(&IndexMark { also: "Elkana".into(), ..IndexMark::new("personen", "Hanna") })),
        "der Server schreibt die Marke kanonisch, mit dem Trenner des ersetzten Blocks"
    );
    let unknown = serde_json::from_str::<Inline>(r#"{"t":"index_mark","mark":{"register":"x","term":"y","z":"1"}}"#);
    assert!(unknown.is_err(), "unbekannte Felder einer Marke lehnt der Server ab (deny_unknown_fields)");
}

#[test]
fn the_sheet_shows_a_quiet_untypeable_element_with_a_description() {
    let mark = IndexMark { sub: "erste Liebe".into(), also: "Huld".into(), ..IndexMark::new("stichwort", "Gnade") };
    let html = render_html(&parse(&format!("Die Gnade{} trägt.", mark_source(&mark))));
    let start = html.find("<span class=\"ws-index-mark\"").expect("Marke als eigenes Element (keine Typst-Insel)");
    let end = start + html[start..].find("</span>").expect("Element geschlossen");
    let element = &html[start..end];
    assert!(element.contains("contenteditable=\"false\""), "nicht tippbar: {element}");
    assert!(
        element.ends_with('>'),
        "ohne Textknoten – das Zeichen kommt aus dem Stil, Text und Suche bleiben: {element}"
    );
    let label = "Registereintrag im Stichwortregister: Gnade › erste Liebe; siehe auch: Huld";
    assert!(
        element.contains(&format!("title=\"{label}\"")) && element.contains(&format!("aria-label=\"{label}\"")),
        "Beschreibung nennt Register, Stichwort, Unterstichwort und Verweis (sichtbar als Tooltip, vorgelesen): {element}"
    );
    assert!(
        element.contains("data-register=\"stichwort\"")
            && element.contains("data-term=\"Gnade\"")
            && element.contains("data-sub=\"erste Liebe\"")
            && element.contains("data-also=\"Huld\"")
            && !element.contains("data-see="),
        "die Felder stehen einzeln da (leere fehlen), damit das Widget die Marke ohne Typst-Leser zurückgibt: {element}"
    );
    assert!(
        element.contains("data-raw=\"#metadata((register: &quot;stichwort&quot;"),
        "die kanonische Form steht maskiert in data-raw: {element}"
    );
    assert!(!html.contains("ws-island"), "eine Marke ist keine gelbe Typst-Insel: {html}");
}

#[test]
fn labels_name_builtin_titles_and_custom_slugs_and_mask_html() {
    assert_eq!(
        index_mark_label(&IndexMark::new("personen", "Rut")),
        "Registereintrag im Personenregister: Rut",
        "eingebautes Register mit seinem Titel"
    );
    assert_eq!(
        index_mark_label(&IndexMark { see: "Bäume".into(), ..IndexMark::new("pflanzen", "Zeder") }),
        "Registereintrag im Register „pflanzen“: Zeder; siehe: Bäume",
        "eigenes Register mit Kennung, „siehe“ mit Ziel"
    );
    let html = render_html(&parse(&format!("A{} b", mark_source(&IndexMark::new("stichwort", "<b>&\"")))));
    assert!(
        html.contains("data-term=\"&lt;b&gt;&amp;&quot;\"") && !html.contains("<b>"),
        "Stichwörter sind Daten und werden maskiert: {html}"
    );
}

/// Editiert Block 1 von `typst` mit `change` an seinen Inlines; die Blöcke 0 und 2 bleiben `keep`.
fn edit_middle(typst: &str, change: impl FnOnce(&mut Vec<Inline>)) -> String {
    let doc = parse(typst);
    assert_eq!(doc.blocks.len(), 3, "drei Blöcke: {typst:?}");
    let mut changed = paragraph(&doc.blocks[1]);
    change(&mut changed);
    let edits = [
        Edit::Keep { keep: 0 },
        Edit::Block { replaces: Some(1), block: BlockKind::Paragraph { inlines: changed } },
        Edit::Keep { keep: 2 },
    ];
    serialize(&doc, &edits).expect("serialisierbar")
}

#[test]
fn editing_or_removing_a_mark_in_the_sheet_changes_only_the_mark() {
    let old = IndexMark::new("stichwort", "Brot");
    // Erster und letzter Block absichtlich nicht kanonisch (zwei Leerzeichen, Kommentar): Keep bleibt wörtlich.
    let typst =
        format!("= Vom Teilen  // Kopf\n\nDas Brot{} wird geteilt, *alle* essen.\n\nEnde  hier.\n", mark_source(&old));
    let new = IndexMark { sub: "Gemeinschaft".into(), ..IndexMark::new("stichwort", "Brot") };
    let edited = edit_middle(&typst, |inlines| {
        for inline in inlines.iter_mut() {
            if let Inline::IndexMark { mark } = inline {
                *mark = new.clone();
            }
        }
    });
    assert_eq!(
        edited,
        typst.replace(&mark_source(&old), &mark_source(&new)),
        "nur die Marke ändert sich; Überschrift mit Kommentar und letzter Block bleiben byte-gleich"
    );
    let removed = edit_middle(&typst, |inlines| inlines.retain(|i| !matches!(i, Inline::IndexMark { .. })));
    assert_eq!(removed, typst.replace(&mark_source(&old), ""), "Entfernen nimmt genau die Marke heraus");
    let added = edit_middle(&typst.replace(&mark_source(&old), ""), |inlines| {
        // Wie das Widget: der Text wird hinter dem Wort geteilt, die Marke steht dazwischen.
        let Inline::Text { v } = inlines[0].clone() else { panic!("Text vorn: {inlines:?}") };
        let (head, tail) = v.split_at("Das Brot".len());
        inlines.splice(0..1, [text(head), Inline::IndexMark { mark: new.clone() }, text(tail)]);
    });
    assert_eq!(added, typst.replace(&mark_source(&old), &mark_source(&new)), "Setzen hinter dem Wort fügt nur sie ein");
}

#[test]
fn a_non_canonical_metadata_stays_an_island() {
    let typst = r#"Wort#metadata((register: "stichwort", term: "Wort")) <baruch-idx> weiter"#;
    assert!(
        !inlines(typst).iter().any(|i| matches!(i, Inline::IndexMark { .. })),
        "mit Leerzeichen vor dem Label ist es keine Registermarke"
    );
}

#[test]
fn a_paragraph_with_several_marks_keeps_their_order_and_text_between() {
    let a = IndexMark::new("personen", "Noomi");
    let b = IndexMark { see: "Noomi".into(), ..IndexMark::new("personen", "Mara") };
    let typst = format!("Noomi{} nannte sich Mara{}.", mark_source(&a), mark_source(&b));
    let read = inlines(&typst);
    assert_eq!(
        read,
        vec![
            text("Noomi"),
            Inline::IndexMark { mark: a },
            text(" nannte sich Mara"),
            Inline::IndexMark { mark: b },
            text("."),
        ],
        "zwei Marken, Satzzeichen direkt hinter der zweiten bleibt Text"
    );
    assert_eq!(emit(&BlockKind::Paragraph { inlines: read }), typst, "byte-gleich zurück");
}
