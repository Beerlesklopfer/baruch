// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reader, resolution and Typst output on a freely invented Word file built in the test (Round 32,
//! docx_import). Every assertion names its reason.

#[path = "data/import/mod.rs"]
mod data;

use baruch_docx::read::read;
use baruch_docx::resolve::{View, change_authors, proposal_view, resolve};
use baruch_docx::to_typst::{CommentAnchor, to_typst};
use baruch_docx::{Block, ChangeKind, DocxDocument, Inline, Limits, Marks};
use data::{ERIKA, JOERG, del, ins, p, r, rp};

fn text(t: &str) -> Inline {
    Inline::Text { text: t.to_owned(), marks: Marks::default() }
}

fn sample() -> DocxDocument {
    read(&data::sample(), &Limits::DEFAULT).expect("das Beispiel ist eine gültige Datei").document
}

fn read_body(body: &str) -> baruch_docx::read::ReadDocx {
    read(&data::with_body(body), &Limits::DEFAULT).expect("lesbar")
}

fn codes(warnings: &[baruch_docx::DocxWarning]) -> Vec<&str> {
    warnings.iter().map(|w| w.code.as_str()).collect()
}

/// Does the state still contain a tracked change or a paragraph mark?
fn has_tracking(blocks: &[Block]) -> bool {
    fn inline(content: &[Inline]) -> bool {
        content.iter().any(|i| match i {
            Inline::Tracked { .. } => true,
            Inline::Footnote { content } | Inline::Quote { content } | Inline::Link { content, .. } => inline(content),
            _ => false,
        })
    }
    blocks.iter().any(|b| match b {
        Block::Paragraph { content, mark } | Block::Part { content, mark } | Block::Heading { content, mark, .. } => {
            mark.is_some() || inline(content)
        }
        Block::Quote { blocks, attribution } => has_tracking(blocks) || inline(attribution),
        Block::List { items, .. } => items.iter().any(|i| i.mark.is_some() || inline(&i.content)),
        Block::Table { rows, .. } => rows.iter().flatten().any(|cell| has_tracking(cell)),
        Block::Figure { caption, .. } => inline(caption),
        Block::Typst { shown, .. } => has_tracking(shown),
        Block::PageBreak => false,
    })
}

fn all_bodies(document: &DocxDocument) -> String {
    to_typst(document).sections.iter().map(|s| format!("{}\n{}", s.title, s.body)).collect::<Vec<_>>().join("\n")
}

#[test]
fn the_reader_builds_the_model_from_headings_text_notes_lists_table_and_image() {
    let read = read(&data::sample(), &Limits::DEFAULT).expect("das Beispiel ist eine gültige Datei");
    let doc = &read.document;
    assert_eq!(doc.meta.title, "Das Haus am Deich", "der Absatz mit der Vorlage Title ist der Titel, kein Abschnitt");
    assert_eq!(doc.meta.lang, "de-DE", "ohne core.xml kommt die Sprache aus den Vorgaben in styles.xml");
    assert_eq!(doc.blocks.len(), 12, "zwölf Blöcke; der Titel und die Quellenzeile des Zitats sind keine eigenen");

    assert_eq!(
        doc.blocks[0],
        Block::Heading { level: 1, numbered: true, content: vec![text("Ankunft")], mark: None },
        "Vorlagen-ID Heading1 ist Überschrift 1"
    );
    let Block::Paragraph { content, mark: None } = &doc.blocks[1] else {
        panic!("Absatz erwartet: {:?}", doc.blocks[1])
    };
    assert_eq!(content[0], Inline::CommentStart { id: 0 }, "der Kommentarbereich beginnt vor dem Wortlaut");
    assert_eq!(content[2], text("Der Wind kam"), "kommentierter Wortlaut");
    assert_eq!(content[3], Inline::CommentEnd { id: 0 }, "und endet danach");
    assert!(
        content.contains(&Inline::Text { text: "kräftig".into(), marks: Marks { bold: true, ..Marks::default() } }),
        "w:b ist fett: {content:?}"
    );
    assert!(
        content.contains(&Inline::Text { text: "salzig".into(), marks: Marks { italic: true, ..Marks::default() } }),
        "w:i ist kursiv, und w:b mit w:val=0 ist ausdrücklich nicht fett: {content:?}"
    );
    assert!(
        content.contains(&Inline::Footnote { content: vec![text("Gemessen wurde am erfundenen Pegel Süderhörn.")] }),
        "die Fußnote steht inline, ohne das Leerzeichen hinter dem Fußnotenzeichen: {content:?}"
    );
    let tracked: Vec<(ChangeKind, &str, &str)> = content
        .iter()
        .filter_map(|i| match i {
            Inline::Tracked { change, content } => match &content[0] {
                Inline::Text { text, .. } => Some((change.kind, change.author.as_str(), text.as_str())),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        tracked,
        vec![
            (ChangeKind::Delete, ERIKA, "leise"),
            (ChangeKind::Insert, ERIKA, "laut"),
            (ChangeKind::Insert, JOERG, " Abends wurde es still."),
        ],
        "w:del (mit w:delText) und w:ins zweier Bearbeiter, in Lesereihenfolge"
    );
    let Some(Inline::Tracked { change, .. }) = content.iter().find(|i| matches!(i, Inline::Tracked { .. })) else {
        panic!("Änderung erwartet")
    };
    assert_eq!(change.date, data::DATE, "das Datum der Änderung bleibt ISO 8601");

    assert!(
        matches!(&doc.blocks[2], Block::Heading { level: 2, numbered: true, .. }),
        "die Vorlage mit dem deutschen Namen „Überschrift 2“ (ID berschrift2) ist Überschrift 2: {:?}",
        doc.blocks[2]
    );
    assert_eq!(
        doc.blocks[3],
        Block::Quote {
            blocks: vec![Block::Paragraph { content: vec![text("Wer nicht deichen will, muss weichen.")], mark: None }],
            attribution: vec![text("Alter Spruch")],
        },
        "Vorlage Quote ist ein Blockzitat, der Absatz BaruchZitatquelle danach seine Quelle"
    );
    let Block::List { ordered: false, items, .. } = &doc.blocks[4] else { panic!("Aufzählung erwartet") };
    assert_eq!(
        items.iter().map(|i| i.depth).collect::<Vec<_>>(),
        vec![0, 1, 0],
        "numFmt bullet ist ungeordnet; w:ilvl ist die Tiefe (verschachtelt)"
    );
    let Block::List { ordered: true, start: 3, items } = &doc.blocks[5] else {
        panic!("geordnete Liste ab 3 erwartet: {:?}", doc.blocks[5])
    };
    assert_eq!(items.len(), 2, "eine andere numId beginnt eine neue Liste; numFmt decimal ist geordnet, w:start 3");
    let Block::Table { header_rows: 1, rows } = &doc.blocks[6] else { panic!("Tabelle mit Kopfzeile erwartet") };
    assert_eq!(rows.len(), 2, "zwei Zeilen");
    assert_eq!(
        rows[1][1],
        vec![Block::Paragraph { content: vec![text("2,10 m")], mark: None }],
        "jede Zelle trägt ihre Absätze"
    );
    assert_eq!(
        doc.blocks[7],
        Block::Figure {
            image: 0,
            alt: "Ein Leuchtturm im Nebel".into(),
            caption: vec![text("Der Leuchtturm bei Nacht")],
            raw: String::new(),
        },
        "ein Absatz nur mit Bild ist eine Abbildung: Alt-Text aus wp:docPr descr, Unterschrift aus dem Caption-Absatz"
    );
    assert_eq!(doc.images.len(), 1, "ein eingebettetes Bild");
    assert_eq!(doc.images[0].path, "bilder/leuchtturm.png", "Pfad im Werk unter bilder/ mit dem Dateinamen");
    assert_eq!(doc.images[0].media_type, "image/png", "Medientyp aus den ersten Bytes");
    assert_eq!(doc.images[0].bytes, data::PNG, "die Bytes des Bildes unverändert");
    assert!(
        matches!(&doc.blocks[8], Block::Heading { level: 3, .. }),
        "w:outlineLvl 2 in der Vorlage macht eine Überschrift 3, auch ohne passenden Namen"
    );
    let Block::Paragraph { content, .. } = &doc.blocks[9] else { panic!("Absatz erwartet") };
    assert!(
        content
            .contains(&Inline::Link { url: "https://example.org/deich".into(), content: vec![text("Deichverband")] }),
        "w:hyperlink mit externer Beziehung ist ein Link: {content:?}"
    );
    assert!(matches!(&doc.blocks[10], Block::Heading { level: 4, .. }), "Überschrift 4 bleibt im Modell Ebene 4");

    assert_eq!(doc.comments.len(), 3, "drei Kommentare aus comments.xml");
    assert_eq!(
        (doc.comments[0].author.as_str(), doc.comments[0].initials.as_str(), doc.comments[0].date.as_str()),
        (ERIKA, "EB", data::DATE),
        "Name, Kürzel und Datum des Kommentars"
    );
    assert_eq!(doc.comments[0].text, "Woher kam der Wind genau?", "Wortlaut des Kommentars");
    assert_eq!(doc.comments[0].reply_to, None, "der erste Kommentar ist keine Antwort");
    assert_eq!(
        doc.comments[1].reply_to,
        Some(0),
        "commentsExtended.xml (paraIdParent) macht den zweiten zur Antwort auf den ersten"
    );
    assert_eq!(doc.comments[1].text, "Aus Nordwest & zwar stetig.", "&amp; ist ein Und-Zeichen");
    assert!(read.warnings.is_empty(), "am Beispiel fällt beim Lesen nichts auf: {:?}", read.warnings);
}

#[test]
fn derived_styles_and_direct_outline_levels_are_headings() {
    let read = read_body(&format!(
        "{}{}<w:p><w:pPr><w:outlineLvl w:val=\"1\"/></w:pPr>{}</w:p>{}{}",
        p("Abgeleitet", &r("Erstes")),
        p("", &r("Text.")),
        r("Zweites"),
        p("BaruchOhneNummer1", &r("Vorwort")),
        p("BaruchTeil", &r("Erster Teil")),
    ));
    let kinds: Vec<String> = read
        .document
        .blocks
        .iter()
        .map(|b| match b {
            Block::Heading { level, numbered, .. } => format!("h{level}{}", if *numbered { "" } else { "*" }),
            Block::Part { .. } => "teil".into(),
            _ => "p".into(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["h1", "p", "h2", "h1*", "teil"],
        "basedOn Heading1 erbt die Ebene; w:outlineLvl am Absatz zählt (0-basiert); BaruchOhneNummer1 ist ohne \
         Nummer, BaruchTeil ein Teil"
    );
}

#[test]
fn the_views_resolve_tracked_changes_per_editor() {
    let doc = sample();
    assert_eq!(change_authors(&doc), vec![ERIKA, JOERG], "Bearbeiter in der Reihenfolge des ersten Auftretens");

    let original = resolve(&doc, View::Original);
    let text = all_bodies(&original);
    assert!(!has_tracking(&original.blocks), "das Original enthält weder Inline::Tracked noch mark");
    assert!(text.contains("vom Meer, leise"), "das Original enthält den gelöschten Text: {text}");
    assert!(
        !text.contains("laut") && !text.contains("Abends") && !text.contains("am Hafen"),
        "und keinen eingefügten: {text}"
    );

    let last = resolve(&doc, View::Final);
    let text = all_bodies(&last);
    assert!(!has_tracking(&last.blocks), "der Endstand enthält weder Inline::Tracked noch mark");
    assert!(
        text.contains("laut") && text.contains("Abends wurde es still.") && text.contains("Die Schleuse am Hafen"),
        "der Endstand enthält alle Einfügungen, auch die in der Überschrift: {text}"
    );
    assert!(!text.contains("leise"), "und keinen gelöschten Text: {text}");

    let erika = resolve(&doc, View::FinalBy(ERIKA));
    let text = all_bodies(&erika);
    assert!(!has_tracking(&erika.blocks), "auch der Stand eines Bearbeiters ist ganz aufgelöst");
    assert!(
        text.contains("Süderhörn.]laut") && !text.contains("leise"),
        "Erikas Löschung und Einfügung gelten: {text}"
    );
    assert!(!text.contains("Abends") && !text.contains("am Hafen"), "Jörgs Einfügungen nicht: {text}");

    let joerg = resolve(&doc, View::FinalBy(JOERG));
    let text = all_bodies(&joerg);
    assert!(text.contains("leise") && !text.contains("laut"), "bei Jörg gilt Erikas Änderung nicht: {text}");
    assert!(text.contains("Abends wurde es still."), "seine eigene Einfügung schon: {text}");
}

#[test]
fn the_proposal_view_keeps_headings_and_reports_their_changes() {
    let doc = sample();
    let (view, changes) = proposal_view(&doc, JOERG);
    assert!(!has_tracking(&view.blocks), "der Vorschlagsstand ist aufgelöst");
    let out = to_typst(&view);
    assert_eq!(
        out.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        vec!["Ankunft", "Der Deich", "Die Schleuse"],
        "die Überschriften bleiben wie im Original – Jörgs „am Hafen“ ändert die Gliederung nicht"
    );
    assert_eq!(changes.len(), 1, "genau eine geänderte Überschrift");
    assert_eq!(
        (changes[0].section, changes[0].before.as_str(), changes[0].after.as_str()),
        (2, "Die Schleuse", "Die Schleuse am Hafen"),
        "sie nennt die Stelle des Abschnitts (ab 0) und den Wortlaut vorher und nachher"
    );
    assert!(out.sections[0].body.contains("Abends wurde es still."), "Jörgs Textänderung steht im Vorschlagsstand");
    let (_, changes) = proposal_view(&doc, ERIKA);
    assert!(changes.is_empty(), "Erika hat keine Überschrift geändert");
}

#[test]
fn inserted_and_deleted_headings_never_change_the_outline() {
    let mark =
        |kind: &str| format!("<w:rPr><w:{kind} w:id=\"7\" w:author=\"{ERIKA}\" w:date=\"{}\"/></w:rPr>", data::DATE);
    let body = format!(
        "{}{}<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/>{}</w:pPr>{}</w:p>{}<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/>\
         </w:pPr>{}</w:p>{}",
        p("Heading1", &r("Morgen")),
        p("", &r("Früh am Tag.")),
        mark("ins"),
        ins(ERIKA, "Mittag"),
        p("", &r("Die Sonne stand hoch.")),
        del(ERIKA, "Abend"),
        p("", &r("Es wurde dunkel.")),
    );
    let doc = read_body(&body).document;
    let original = to_typst(&resolve(&doc, View::Original));
    assert_eq!(
        original.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        vec!["Morgen", "Abend"],
        "im Original gibt es die eingefügte Überschrift nicht, die gelöschte schon"
    );
    assert_eq!(
        original.sections[0].body, "Früh am Tag.\n\nDie Sonne stand hoch.",
        "ohne die eingefügte Überschrift gehören beide Absätze zum ersten Abschnitt"
    );
    let (view, changes) = proposal_view(&doc, ERIKA);
    let proposed = to_typst(&view);
    assert_eq!(
        proposed.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        vec!["Morgen", "Abend"],
        "der Vorschlagsstand hat dieselbe Gliederung wie das Original"
    );
    assert_eq!(
        proposed.sections[0].body, "Früh am Tag.\n\n#strong[Mittag]\n\nDie Sonne stand hoch.",
        "der Text der eingefügten Überschrift steht fett im vorangehenden Abschnitt"
    );
    assert_eq!(
        changes.iter().map(|c| (c.section, c.before.as_str(), c.after.as_str())).collect::<Vec<_>>(),
        vec![(0, "", "Mittag"), (1, "Abend", "")],
        "eingefügt: before leer am vorangehenden Abschnitt; gelöscht: after leer am Abschnitt selbst"
    );
}

#[test]
fn paragraph_marks_join_paragraphs_in_the_right_view() {
    let mark = |kind: &str| {
        format!("<w:pPr><w:rPr><w:{kind} w:id=\"7\" w:author=\"{ERIKA}\" w:date=\"{}\"/></w:rPr></w:pPr>", data::DATE)
    };
    let body = format!(
        "{}<w:p>{}{}</w:p>{}<w:p>{}{}</w:p>{}",
        p("Heading1", &r("Flut")),
        mark("ins"),
        r("Das Wasser stieg. "),
        p("", &r("Niemand sah hin.")),
        mark("del"),
        r("Dann fiel es. "),
        p("", &r("Alle atmeten auf.")),
    );
    let doc = read_body(&body).document;
    assert!(
        matches!(&doc.blocks[1], Block::Paragraph { mark: Some(c), .. } if c.kind == ChangeKind::Insert && c.author == ERIKA),
        "w:ins in den Laufeigenschaften der Absatzmarke ist mark Insert: {:?}",
        doc.blocks[1]
    );
    assert_eq!(
        to_typst(&resolve(&doc, View::Original)).sections[0].body,
        "Das Wasser stieg. Niemand sah hin.\n\nDann fiel es.\n\nAlle atmeten auf.",
        "Original: die eingefügte Marke gab es nicht (ein Absatz), die gelöschte schon (zwei Absätze)"
    );
    assert_eq!(
        to_typst(&resolve(&doc, View::Final)).sections[0].body,
        "Das Wasser stieg.\n\nNiemand sah hin.\n\nDann fiel es. Alle atmeten auf.",
        "Endstand: umgekehrt"
    );
}

#[test]
fn typst_comes_out_in_the_forms_of_the_work_with_comment_anchors() {
    let out = to_typst(&resolve(&sample(), View::Original));
    assert_eq!(out.sections.len(), 3, "drei Abschnitte: Überschrift 1 bis 3; Überschrift 4 ist keiner");
    let s = &out.sections;
    assert_eq!((s[0].level, s[0].numbered, s[0].title.as_str()), (1, true, "Ankunft"), "erster Abschnitt");
    assert_eq!((s[1].level, s[1].title.as_str()), (2, "Der Deich"), "zweiter Abschnitt auf Ebene 2");
    assert_eq!((s[2].level, s[2].title.as_str()), (3, "Die Schleuse"), "dritter Abschnitt auf Ebene 3");
    assert_eq!(
        s[0].body,
        "Der Wind kam #strong[kräftig] und #emph[salzig] vom Meer, leise#footnote[Gemessen wurde am erfundenen Pegel \
         Süderhörn.]",
        "fett und kursiv als Funktion, die Fußnote als #footnote an ihrer Stelle"
    );
    assert_eq!(
        s[0].anchors,
        vec![
            CommentAnchor { comment: 0, quote: "Der Wind kam".into() },
            CommentAnchor { comment: 1, quote: "Der Wind kam".into() },
        ],
        "Kommentar und Antwort sind am kommentierten Wortlaut verankert"
    );
    assert_eq!(
        s[1].body,
        "#quote(block: true, attribution: [Alter Spruch])[\nWer nicht deichen will, muss weichen.\n]\n\n\
         - Schafe\n  - mit Lämmern\n- Möwen\n\n\
         3. Deich bauen\n4. Deich halten\n\n\
         #table(\n  columns: 2,\n  table.header([Pegel], [Stand]),\n  [Süderhörn], [2,10 m],\n)\n\n\
         #figure(image(\"bilder/leuchtturm.png\", alt: \"Ein Leuchtturm im Nebel\"), caption: [Der Leuchtturm bei Nacht])",
        "Blockzitat mit Quelle, verschachtelte Aufzählung, geordnete Liste ab 3, Tabelle als #table (im Editor eine \
         Insel) und Abbildung mit alt und Unterschrift – in den Formen des Markdown-Imports"
    );
    assert_eq!(
        s[2].body,
        "Die Schleuse öffnete sich nur bei Ebbe, siehe #link(\"https://example.org/deich\")[Deichverband].\n\n\
         #strong[Kleingedrucktes]\n\n\\#1 kostet 5 \\$ – \\*nicht\\* verhandelbar.",
        "Link als Funktion, Überschrift 4 als fetter Text, Typst-Sonderzeichen im Text maskiert"
    );
    assert_eq!(s[2].anchors, vec![CommentAnchor { comment: 2, quote: "Die Schleuse".into() }], "Anker im dritten");
    assert_eq!(codes(&out.warnings), vec!["docx_ueberschrift_tief"], "Überschrift 4 wird gemeldet, sonst nichts");
    for section in s {
        let source = format!("= {}\n\n{}\n", section.title, section.body);
        let doc = baruch_wysiwyg::parse(&source);
        assert!(!doc.blocks.is_empty(), "der WYSIWYG-Leser versteht den Abschnitt „{}“", section.title);
    }
}

#[test]
fn text_before_the_first_heading_and_documents_without_heading_lose_nothing() {
    let out = to_typst(
        &read_body(&format!("{}{}{}", p("", &r("Vorab ein Gruß.")), p("Heading1", &r("Eins")), p("", &r("Text."))))
            .document,
    );
    assert_eq!(out.sections.len(), 2, "der Text vor der ersten Überschrift ist ein eigener erster Abschnitt");
    assert_eq!(
        (out.sections[0].title.as_str(), out.sections[0].level, out.sections[0].body.as_str()),
        ("", 1, "Vorab ein Gruß."),
        "mit leerem Titel (der Aufrufer setzt den Titel des Werks) und dem ganzen Text"
    );
    assert!(codes(&out.warnings).contains(&"docx_text_vor_ueberschrift"), "und er wird gemeldet: {:?}", out.warnings);
    let out =
        to_typst(&read_body(&format!("{}{}", p("", &r("Nur ein Absatz.")), p("", &r("Und noch einer.")))).document);
    assert_eq!(out.sections.len(), 1, "ein Dokument ohne Überschrift ist ein Abschnitt");
    assert_eq!(out.sections[0].body, "Nur ein Absatz.\n\nUnd noch einer.", "mit dem ganzen Text");
}

#[test]
fn line_starts_and_expression_ends_are_masked() {
    let out = to_typst(
        &read_body(&format!(
            "{}{}{}{}<w:p>{}<w:r><w:br/></w:r>{}</w:p><w:p>{}{}</w:p>",
            p("Heading1", &r("Zeichen")),
            p("", &r("- kein Listenpunkt")),
            p("", &r("3. Mai war es soweit")),
            p("", &r("= keine Überschrift")),
            r("Erste Zeile"),
            r("+ zweite Zeile"),
            rp("<w:b/>", "fett"),
            r("(Klammer)"),
        ))
        .document,
    );
    assert_eq!(
        out.sections[0].body,
        "\\- kein Listenpunkt\n\n3\\. Mai war es soweit\n\n\\= keine Überschrift\n\nErste Zeile \\\n\\+ zweite Zeile\n\n\
         #strong[fett];(Klammer)",
        "Zeilenanfänge, die Typst als Liste oder Überschrift läse, sind maskiert (auch nach einem Zeilenumbruch); \
         nach einem Ausdruck trennt ein Strichpunkt die Klammer ab"
    );
}

#[test]
fn nothing_is_dropped_silently() {
    let textbox = "<w:r><mc:AlternateContent><mc:Choice Requires=\"wps\"><w:drawing><wp:anchor><a:graphic>\
        <a:graphicData><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Randnotiz im Kasten</w:t></w:r></w:p>\
        </w:txbxContent></wps:txbx></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></mc:Choice>\
        <mc:Fallback><w:pict><v:shape><v:textbox><w:txbxContent><w:p><w:r><w:t>Randnotiz im Kasten</w:t></w:r></w:p>\
        </w:txbxContent></v:textbox></v:shape></w:pict></mc:Fallback></mc:AlternateContent></w:r>";
    let body = format!(
        "{}<w:p><w:pPr><w:pPrChange w:id=\"3\" w:author=\"{ERIKA}\"><w:pPr/></w:pPrChange></w:pPr>{}{}</w:p>\
         <w:p>{}<w:moveFrom w:id=\"4\" w:author=\"{ERIKA}\" w:date=\"{d}\">{}</w:moveFrom>{}\
         <w:moveTo w:id=\"5\" w:author=\"{ERIKA}\" w:date=\"{d}\">{}</w:moveTo></w:p>\
         <w:p>{}{textbox}</w:p>\
         <w:p>{}<m:oMath><m:r><m:t>a²+b²=c²</m:t></m:r></m:oMath></w:p>\
         <w:p>{}<w:r><w:object><o:OLEObject Type=\"Embed\" ProgID=\"Excel.Sheet.12\"/></w:object></w:r></w:p>\
         <w:tbl><w:tr><w:tc><w:tcPr><w:gridSpan w:val=\"2\"/></w:tcPr>{}</w:tc></w:tr></w:tbl>\
         <w:p>{}<w:r><w:drawing><wp:inline><wp:docPr id=\"2\" name=\"x\"/><a:graphic><a:graphicData><pic:pic>\
         <pic:blipFill><a:blip r:embed=\"rId20\"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline>\
         </w:drawing></w:r></w:p>",
        p("Heading1", &r("Allerlei")),
        rp("<w:b/><w:rPrChange w:id=\"2\" w:author=\"Erika Beispiel\"><w:rPr/></w:rPrChange>", "Neu fett"),
        r(" gesetzt."),
        r("Anfang "),
        r("wandernder Satz "),
        r("Mitte "),
        r("wandernder Satz "),
        r("Neben dem Kasten."),
        r("Pythagoras: "),
        r("Tabelle aus einer Rechenmappe: "),
        p("", &r("breit")),
        r("Mit Bild."),
        d = data::DATE,
    );
    let rel = "<Relationship Id=\"rId20\" \
        Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/plan.emf\"/>";
    let mut parts = data::parts(&body, rel);
    parts.push(("word/media/plan.emf", vec![1, 0, 0, 0, 0x20, 0x45, 0x4d, 0x46]));
    let read = read(&data::zip(&parts), &Limits::DEFAULT).expect("lesbar");
    let found = codes(&read.warnings);
    for code in [
        "docx_formataenderung",
        "docx_verschoben",
        "docx_textfeld",
        "docx_formel",
        "docx_objekt",
        "docx_tabelle_vereinfacht",
        "docx_bild_format",
    ] {
        assert!(found.contains(&code), "die Warnung {code} mit festem Code fehlt: {:?}", read.warnings);
    }
    let format = read.warnings.iter().find(|w| w.code == "docx_formataenderung").expect("eben geprüft");
    assert!(
        format.message.ends_with("(2 Stellen)"),
        "je Code eine Warnung mit der Zahl der Stellen (pPrChange und rPrChange): {}",
        format.message
    );
    let box_warning = read.warnings.iter().find(|w| w.code == "docx_textfeld").expect("eben geprüft");
    assert!(
        box_warning.message.contains("Randnotiz im Kasten") && !box_warning.message.contains("Stellen"),
        "das Textfeld wird einmal gemeldet (Choice, nicht noch der Fallback) und nennt seinen Anfang: {}",
        box_warning.message
    );
    assert!(read.document.images.is_empty(), "ein EMF-Bild kommt nicht ins Werk (nur gemeldet)");
    let original = all_bodies(&resolve(&read.document, View::Original));
    assert!(
        original.contains("Anfang wandernder Satz Mitte") && !original.contains("Mitte wandernder"),
        "verschobener Text ist im Original an der alten Stelle (moveFrom = Löschung): {original}"
    );
    let last = all_bodies(&resolve(&read.document, View::Final));
    assert!(last.contains("Anfang Mitte wandernder Satz"), "und im Endstand an der neuen (moveTo = Einfügung): {last}");
    assert!(last.contains("Pythagoras: a²+b²=c²"), "der Text der Formel bleibt als einfacher Text: {last}");
    assert!(last.contains("#strong[Neu fett] gesetzt."), "bei Formatänderungen gilt die Formatierung des Endstands");
}

#[test]
fn typst_islands_come_back_verbatim() {
    let body = format!(
        "{}<w:sdt><w:sdtPr><w:tag w:val=\"baruch-typst\"/></w:sdtPr><w:sdtContent>\
         <w:p>{}</w:p><w:p>{}</w:p><w:p>{}</w:p></w:sdtContent></w:sdt>\
         <w:p>{}<w:sdt><w:sdtPr><w:tag w:val=\"baruch-typst\"/></w:sdtPr><w:sdtContent>{}{}</w:sdtContent></w:sdt>{}</w:p>",
        p("Heading1", &r("Inseln")),
        rp("<w:vanish/>", "#szene(ort: \"Deich\")["),
        rp("<w:vanish/>", "  Stille. ]"),
        r("Szene: Stille."),
        r("Breite "),
        rp("<w:vanish/>", "#h(1cm)"),
        r("[Abstand]"),
        r(" danach."),
    );
    let read = read_body(&body);
    assert_eq!(
        read.document.blocks[1],
        Block::Typst { raw: "#szene(ort: \"Deich\")[\n  Stille. ]".into(), shown: vec![] },
        "w:sdt mit w:tag baruch-typst: der mit w:vanish verborgene Text ist der Quelltext (Absätze = Zeilen), das \
         Sichtbare wird verworfen"
    );
    assert_eq!(
        to_typst(&read.document).sections[0].body,
        "#szene(ort: \"Deich\")[\n  Stille. ]\n\nBreite #h(1cm) danach.",
        "die Inseln stehen wörtlich im Typst, als Block und im Text"
    );
    assert!(
        read.warnings.is_empty(),
        "der verborgene Quelltext einer Insel ist kein „verborgener Text“: {:?}",
        read.warnings
    );
}
