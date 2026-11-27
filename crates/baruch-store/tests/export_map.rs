// SPDX-License-Identifier: AGPL-3.0-or-later
//! `Store::export_map` (contract Round 8, section 4b): byte-identical with `export_typst` – with work template,
//! outline scheme and image – and per section key, range and first line. Real SQLite file in the temp folder,
//! freely invented work.

use baruch_store::{ImportAsset, Store, WorkEdit};

/// Invented work with template, image and a section without a line end at the end (the export adds none there, but
/// does in the middle).
const WORK: &str = "#set text(lang: \"de\")\n#let szene(body) = emph(body)\n\
= Die Laube\nAnna zählt Äpfel.\n#figure(image(\"./bilder/laube.svg\"))\n\
== Abends\n#szene[Wind.]\n\
= Der Winter\nSchnee.";

const SVG: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\"/></svg>";

async fn imported() -> (tempfile::TempDir, Store, i64) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let asset = ImportAsset { path: "./bilder/laube.svg".into(), file_name: "laube.svg".into(), bytes: SVG.into() };
    let (imported, _) = store
        .import_typst_with_assets(WORK, "laube.typ", "Autorin", "Import", None, &[asset])
        .await
        .expect("Import des erfundenen Werks mit Bild");
    (dir, store, imported.work)
}

#[tokio::test]
async fn the_map_is_byte_identical_to_the_export_with_template_scheme_and_image() {
    let (_dir, store, work) = imported().await;
    let map = store.export_map(work).await.unwrap();
    assert_eq!(map.typst, store.export_typst(work).await.unwrap(), "ohne Schema: Karte und Export byte-gleich");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&map.typst),
        WORK,
        "frisch importiert ist der Export die Quelle (bis auf die Sprungmarken)"
    );

    store.set_numbering_scheme(work, "chapter_roman", None).await.expect("Schema setzen");
    let map = store.export_map(work).await.unwrap();
    let exported = store.export_typst(work).await.unwrap();
    assert!(exported.contains("Gliederungsschema"), "mit Schema trägt der Export die Regel: {exported}");
    assert_eq!(map.typst, exported, "mit Gliederungsschema: Karte und Export byte-gleich");

    let assets = store.export_assets(work).await.unwrap();
    assert_eq!(assets.len(), 1, "das Bild gehört zum Export");
    assert_eq!(assets[0].path, "bilder/laube.svg", "Bildpfad in Normalform, wie die Vorschau ihn sucht");
}

#[tokio::test]
async fn sections_carry_key_seq_range_and_first_line() {
    let (_dir, store, work) = imported().await;
    store.set_numbering_scheme(work, "decimal", None).await.expect("Schema setzen");
    let map = store.export_map(work).await.unwrap();
    let loaded = store.load_work(work).await.unwrap();

    assert_eq!(map.sections.len(), loaded.nodes.len(), "je Abschnitt mit aktueller Fassung ein Eintrag");
    for (section, node) in map.sections.iter().zip(&loaded.nodes) {
        assert_eq!(section.key, node.key, "fester Schlüssel des Abschnitts {}", node.title);
        assert_eq!(section.seq, node.id, "Stelle des Abschnitts {}", node.title);
        let body = &node.revisions[node.current_index().unwrap()].body;
        let text = &map.typst[section.start..section.end];
        // Round 30a: the first line carries the jump label `<kN>`, the rest is the text.
        assert!(
            baruch_typst::xref::strip_key_labels(text).starts_with(body.as_str()),
            "Bereich von {} muss mit seinem Text beginnen: {text:?}",
            node.title
        );
        let line = map
            .typst
            .lines()
            .position(|l| body.lines().next() == Some(&*baruch_typst::xref::strip_key_labels(l)))
            .unwrap()
            + 1;
        assert_eq!(section.first_line, line, "erste Zeile von {} im Export", node.title);
    }
    let first = &map.sections[0];
    assert!(first.first_line > 2, "vor dem ersten Abschnitt stehen Vorlage und Schema-Regel");
    assert_eq!(map.section_at_line(1), None, "Zeile 1 gehört der Werkvorlage");
    assert_eq!(map.section_at_line(first.first_line).map(|s| s.key), Some(first.key), "erste Zeile → Abschnitt");
    let last = map.sections.last().unwrap();
    assert_eq!(map.section_at_line(last.first_line + 1).map(|s| s.key), Some(last.key), "Zeile im letzten Abschnitt");
    assert_eq!(last.end, map.typst.len(), "der letzte Abschnitt reicht bis ans Ende");
}

#[tokio::test]
async fn an_unknown_work_is_an_error() {
    let (_dir, store, work) = imported().await;
    assert!(store.export_map(work + 100).await.is_err(), "ein unbekanntes Werk hat keine Karte");
}

#[tokio::test]
async fn a_template_without_final_line_end_keeps_the_first_heading_and_its_own_lines() {
    let (_dir, store, work) = imported().await;
    // Saved by the human without a line end at the end (this is how it arrives through work.update).
    let template = "#set text(lang: \"de\")\n#unbekannt()";
    let edit = WorkEdit { template: Some(template), ..WorkEdit::default() };
    store.update_work(work, &edit, "Autorin").await.expect("Vorlage ohne Zeilenende speichern");
    assert_eq!(store.work_template(work).await.unwrap(), template, "gespeichert wird die Vorlage, wie sie kam");

    let map = store.export_map(work).await.unwrap();
    assert_eq!(map.typst, store.export_typst(work).await.unwrap(), "Karte und Export bleiben byte-gleich");
    // Otherwise „#unbekannt()= Die Laube“ would stand in one line: no heading any more, and the error of the
    // template would fall on the first section.
    assert!(
        map.typst.starts_with("#set text(lang: \"de\")\n#unbekannt()\n= Die Laube <k"),
        "zwischen Vorlage und erster Überschrift steht ein Zeilenende: {:?}",
        map.typst
    );
    let first = &map.sections[0];
    assert_eq!(first.first_line, 3, "der erste Abschnitt beginnt in der Zeile nach der Vorlage");
    assert_eq!(map.section_at_line(2), None, "die letzte Vorlagenzeile gehört der Werkvorlage");
    assert_eq!(map.section_at_line(3).map(|s| s.key), Some(first.key), "Zeile 3 ist die erste Überschrift");

    let files = store.mirror_snapshot(work).await.unwrap();
    let main = &files.iter().find(|f| f.path == "main.typ").expect("main.typ").contents;
    assert!(
        main.starts_with("#set text(lang: \"de\")\n#unbekannt()\n#include \"abschnitte/"),
        "auch in main.typ steht das erste #include in eigener Zeile: {main:?}"
    );
    let vorlage = &files.iter().find(|f| f.path == "vorlage.typ").expect("vorlage.typ").contents;
    assert_eq!(vorlage, template, "vorlage.typ bleibt die gespeicherte Vorlage");
}
