// SPDX-License-Identifier: AGPL-3.0-or-later
//! Werkvorlage im Spiegel (Entscheidung 2026-11-05): `vorlage.typ`, `main.typ` = Vorlage + `#include`,
//! jede Abschnittsdatei beginnt mit `#import "../vorlage.typ": *`. Dazu das Beispielwerk, das nur in
//! eine leere Datenbank gesät wird. Echte SQLite-Datei im Temp-Ordner, eigene Datei
//! `tests/data/probebuch.typ`.

use baruch_store::{MirrorFile, Store};

const BOOK: &str = include_str!("data/probebuch.typ");
const IMPORT_LINE: &str = "#import \"../vorlage.typ\": *\n";

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

fn find<'a>(files: &'a [MirrorFile], path: &str) -> Option<&'a MirrorFile> {
    files.iter().find(|f| f.path == path)
}

#[tokio::test]
async fn an_imported_work_gets_vorlage_typ_and_every_section_imports_it() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let template = store.work_template(imported.work).await.unwrap();
    let files = store.mirror_snapshot(imported.work).await.unwrap();

    let vorlage = find(&files, "vorlage.typ").expect("vorlage.typ fehlt im Spiegel eines importierten Werks");
    assert_eq!(vorlage.contents, template, "vorlage.typ ist genau die Werkvorlage");

    let main = find(&files, "main.typ").expect("main.typ fehlt");
    assert!(main.contents.starts_with(&template), "main.typ beginnt weiter mit der Vorlage (Set-/Show-Regeln)");
    let rest: Vec<&str> = main.contents[template.len()..].lines().collect();
    assert!(
        rest.len() == 6 && rest.iter().all(|l| l.starts_with("#include \"abschnitte/")),
        "nach der Vorlage nur die sechs #include-Zeilen, kein #import in main.typ: {rest:?}"
    );

    let work = store.load_work(imported.work).await.unwrap();
    let sections: Vec<&MirrorFile> = files.iter().filter(|f| f.path.starts_with("abschnitte/")).collect();
    assert_eq!(sections.len(), work.nodes.len(), "eine Datei je Abschnitt");
    for (file, node) in sections.iter().zip(&work.nodes) {
        let body = &node.revisions[node.current_index().unwrap()].body;
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&file.contents),
            format!("{IMPORT_LINE}{body}"),
            "{}: erste Zeile importiert die Vorlage, danach unverändert die aktuelle Revision",
            file.path
        );
        assert!(!body.contains("vorlage.typ"), "{}: die Revision in SQLite bleibt ohne Import-Zeile", file.path);
    }
}

#[tokio::test]
async fn the_export_stays_byte_for_byte_the_source_despite_the_mirror_projection() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let _ = store.mirror_snapshot(imported.work).await.unwrap();
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned();
    assert_eq!(exported.as_bytes(), BOOK.as_bytes(), "Export nimmt die Revisionen, nicht die Spiegeldateien");
    assert!(!exported.contains("vorlage.typ"), "keine Import-Zeile im Export");
}

#[tokio::test]
async fn a_work_without_template_has_neither_vorlage_typ_nor_import_lines() {
    let (_dir, store) = fresh().await;
    let sample = store.default_work().await.unwrap();
    let files = store.mirror_snapshot(sample).await.unwrap();
    assert!(find(&files, "vorlage.typ").is_none(), "Beispielwerk ohne Vorlage: keine vorlage.typ");
    assert!(files[0].contents.starts_with("// Baruch: "), "main.typ wie bisher: {}", files[0].contents);
    for file in &files {
        assert!(!file.contents.contains("#import \"../vorlage.typ\""), "{}: keine Import-Zeile", file.path);
    }

    // Auch ein importiertes Werk ohne Präambel hat keine Vorlage.
    let bare = store.import_typst("= Eins\nText\n", "nackt.typ", "Autor").await.unwrap();
    let files = store.mirror_snapshot(bare.work).await.unwrap();
    assert!(find(&files, "vorlage.typ").is_none(), "leere Vorlage → keine vorlage.typ: {files:?}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&find(&files, "abschnitte/000-eins.typ").unwrap().contents),
        "= Eins\nText\n",
        "Text ohne Import-Zeile"
    );
}

#[tokio::test]
async fn the_sample_work_is_named_beispielwerk_only_when_seeded_into_an_empty_database() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let works = store.works().await.unwrap();
    assert_eq!(works.len(), 1, "leere Datenbank bekommt genau das Beispielwerk: {works:?}");
    assert_eq!(works[0].2, "Beispielwerk: Leise Abhängigkeit (Auszug)", "neuer Titel des Beispielwerks");
    assert_eq!(works[0].1, "beispielwerk-leise-abhaengigkeit-auszug", "slug aus dem neuen Titel");

    // Ein vorhandenes Werk mit altem Titel bleibt beim nächsten Öffnen, wie es ist (keine Migration).
    sqlx::query("UPDATE work SET title = 'Mövenoog – ein ganzes Jahr auf der kleinen Insel' WHERE id = ?")
        .bind(works[0].0)
        .execute(store.pool())
        .await
        .unwrap();
    drop(store);
    let again = Store::open(dir.path()).await.unwrap();
    let works = again.works().await.unwrap();
    assert_eq!(works.len(), 1, "zweites Öffnen sät nicht noch einmal: {works:?}");
    assert_eq!(
        works[0].2, "Mövenoog – ein ganzes Jahr auf der kleinen Insel",
        "zweites Store::open ändert den Titel eines vorhandenen Werks nicht"
    );
    assert_eq!(works[0].1, "beispielwerk-leise-abhaengigkeit-auszug", "der slug in der Datenbank bleibt");
}

#[tokio::test]
async fn section_files_are_named_after_the_resolved_title_and_follow_the_outline() {
    let (_dir, store) = fresh().await;
    let source = "#set text(lang: \"de\")\n= Anna\\'s Laube\n\nA.\n\n= \\#2 Zweites\n\nB.\n";
    let imported = store.import_typst(source, "masken.typ", "Autor").await.unwrap();
    let files = store.mirror_snapshot(imported.work).await.unwrap();
    let names: Vec<&str> = files.iter().map(|f| f.path.as_str()).filter(|p| p.starts_with("abschnitte/")).collect();
    assert_eq!(
        names,
        ["abschnitte/000-anna-s-laube.typ", "abschnitte/001-2-zweites.typ"],
        "Dateiname aus dem aufgelösten Titel (kein „-s-“ aus dem Backslash)"
    );
    store.outline_move(imported.work, 1, 0, baruch_store::Place::Before, "Autor").await.unwrap();
    let files = store.mirror_snapshot(imported.work).await.unwrap();
    let main = find(&files, "main.typ").unwrap();
    assert!(
        main.contents
            .ends_with("#include \"abschnitte/000-2-zweites.typ\"\n#include \"abschnitte/001-anna-s-laube.typ\"\n"),
        "main.typ bindet in neuer Reihenfolge ein: {}",
        main.contents
    );
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&find(&files, "abschnitte/001-anna-s-laube.typ").unwrap().contents),
        format!("{IMPORT_LINE}= Anna\\'s Laube\n\nA.\n\n"),
        "der Text bleibt wörtlich, nur die Datei heißt nach der neuen seq"
    );
}
