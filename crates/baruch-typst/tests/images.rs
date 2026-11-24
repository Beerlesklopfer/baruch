// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bildpfade in Typst-Text (`baruch_typst::images::image_paths`, Vertrag Runde 6, Abschnitt 4b), mit
//! frei erfundenen Texten.

use baruch_typst::images::{ComputedImage, computed_images, image_paths};

/// Erfundene Werkvorlage mit Kapitelbildern über eine literale Liste (Runde 11).
const CHAPTER_TEMPLATE: &str = "#let kapitelbilder = (\n  \"bilder/morgen.png\",\n  \"bilder/mittag.png\", // zweites\n  \"bilder/abend.png\",\n)\n\
     #show heading.where(level: 1): it => context {\n  pagebreak(weak: true)\n  \
     image(kapitelbilder.at(counter(heading).get().first() - 1), width: 100%)\n  it\n}\n";

#[test]
fn a_chapter_image_from_a_literal_list_is_found_per_chapter() {
    let found = computed_images(CHAPTER_TEMPLATE);
    assert_eq!(found.len(), 1, "genau ein berechnetes Bild: {found:?}");
    assert_eq!(
        found[0],
        ComputedImage::PerChapter {
            list: vec!["bilder/morgen.png".into(), "bilder/mittag.png".into(), "bilder/abend.png".into()],
            offset: -1,
        },
        "Liste aus String-Literalen (Kommentar und Schlusskomma stören nicht), Index Kapitel − 1"
    );
    assert_eq!(found[0].for_chapter(1), Some("bilder/morgen.png"), "Kapitel 1 → erstes Element");
    assert_eq!(found[0].for_chapter(3), Some("bilder/abend.png"), "Kapitel 3 → drittes Element");
    assert_eq!(found[0].for_chapter(4), None, "über das Ende hinaus: nicht erfunden (Typst meldete einen Fehler)");
    assert!(image_paths(CHAPTER_TEMPLATE).is_empty(), "image_paths nennt weiter nur Literale");
}

#[test]
fn an_index_variable_and_a_fixed_number_are_resolved_too() {
    let via_variable = "#let bilder = (\"a.png\", \"b.png\")\n#show heading: it => context {\n  \
                        let n = counter(heading).get().first()\n  image(bilder.at(n))\n}\n";
    assert_eq!(
        computed_images(via_variable),
        [ComputedImage::PerChapter { list: vec!["a.png".into(), "b.png".into()], offset: 0 }],
        "Index über genau eine let-Variable mit der Kapitelnummer"
    );
    let fixed = "#let bilder = (\"a.png\", \"b.png\")\n#image(bilder.at(1))\n";
    assert_eq!(computed_images(fixed), [ComputedImage::Fixed { path: "b.png".into() }], "feste Zahl als Index");
}

#[test]
fn anything_not_safely_determinable_stays_unknown() {
    for (source, why) in [
        ("#let name = \"x\"\n#image(\"bilder/\" + name)\n", "zusammengesetzter Pfad"),
        ("#image(pfad)\n", "Variable ohne Liste"),
        ("#let l = (\"a.png\", \"b.png\")\n#let l = (\"c.png\",)\n#image(l.at(0))\n", "Liste zweimal definiert"),
        ("#let l = (\"a.png\", gross)\n#image(l.at(0))\n", "Liste mit einem Nicht-Literal"),
        ("#let l = (\"a.png\", \"b.png\")\n#image(l.at(counter(page).get().first()))\n", "Seitenzähler"),
        ("#let l = (\"a.png\",)\n#image(l.at(5))\n", "Zahl außerhalb der Liste"),
        ("#let l = (\"a.png\")\n#image(l.at(0))\n", "geklammerter String ist keine Liste"),
    ] {
        let found = computed_images(source);
        assert!(
            matches!(found.as_slice(), [ComputedImage::Unknown { .. }]),
            "{why}: ehrlich unbekannt statt geraten: {found:?}"
        );
        assert_eq!(found[0].for_chapter(1), None, "{why}: kein Pfad");
    }
    assert!(
        computed_images("// #image(l.at(0))\n#image(\"echt.png\")\n").is_empty(),
        "Kommentar und Literal zählen nicht"
    );
}

#[test]
fn markup_images_and_images_inside_figures_are_found_in_order() {
    let source = "#set page(width: 10cm)\n\
                  = Der Garten\n\n\
                  #image(\"/bilder/beet.svg\", width: 50%)\n\n\
                  #figure(\n  image(\"bilder/teich.png\"),\n  caption: [Der Teich im Mai],\n)\n\n\
                  #figure(image( \"zaun.jpg\" ), caption: [Zaun])\n";
    assert_eq!(
        image_paths(source),
        ["/bilder/beet.svg", "bilder/teich.png", "zaun.jpg"],
        "#image( im Markup, image( als Argument von #figure, auch mehrzeilig und mit Leerraum, in Reihenfolge"
    );
}

#[test]
fn comments_and_raw_text_do_not_count() {
    let source = "// #image(\"/kommentar.png\")\n\
                  /* #figure(image(\"block.png\")) */\n\
                  `#image(\"roh.png\")`\n\
                  ```typ\n#image(\"codeblock.png\")\n```\n\
                  #image(\"echt.svg\") // dahinter ein Kommentar\n";
    assert_eq!(
        image_paths(source),
        ["echt.svg"],
        "Zeilen- und Blockkommentare, Inline-Raw und Raw-Blöcke werden übersprungen"
    );
}

#[test]
fn the_same_path_twice_is_listed_once() {
    let source = "#image(\"a.png\")\n#figure(image(\"a.png\"))\n#image(\"b.png\")\n#image(\"a.png\")\n";
    assert_eq!(image_paths(source), ["a.png", "b.png"], "ohne Doppelte, erste Stelle zählt");
}

#[test]
fn computed_paths_and_other_names_are_not_recognised() {
    let source = "#let name = \"haus\"\n\
                  #image(\"bilder/\" + name + \".svg\")\n\
                  #image(name)\n\
                  #myimage(\"falsch.png\")\n\
                  Im Fließtext steht image(\"text.png\") ohne Raute.\n\
                  #let bild = image(\"let.png\")\n\
                  #{ image(\"code.png\") }\n";
    assert_eq!(
        image_paths(source),
        ["let.png", "code.png"],
        "berechnete Pfade, Variablen, andere Funktionsnamen und Fließtext zählen nicht; #let und Codeblöcke schon"
    );
}

#[test]
fn escapes_in_the_string_are_resolved() {
    assert_eq!(
        image_paths("#image(\"bilder/\\\"zitat\\\".png\")\n"),
        ["bilder/\"zitat\".png"],
        "\\\" im String wird zum Anführungszeichen"
    );
    assert!(image_paths("#image(\"\")").is_empty(), "leerer Pfad zählt nicht");
}
