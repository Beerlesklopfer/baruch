// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Import und -Export (Vertrag B1.5, Abschnitte 1 und 3) auf einer echten SQLite-Datei im
//! Temp-Ordner, mit der eigenen kleinen Datei `tests/data/probebuch.typ`.

use baruch_core::{Author, NodeKind, RevisionStatus};
use baruch_store::{Store, StoreError};

const BOOK: &str = include_str!("data/probebuch.typ");

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

#[tokio::test]
async fn exporting_an_imported_book_gives_back_the_file_byte_for_byte() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned();
    assert_eq!(exported.as_bytes(), BOOK.as_bytes(), "Import und Export müssen die Eingabe Byte für Byte ergeben");
    assert!(
        store.work_template(imported.work).await.unwrap().ends_with("#let notiz(body) = emph(body)\n\n"),
        "Werkvorlage ist die Präambel bis vor die erste Überschrift"
    );
}

#[tokio::test]
async fn importing_builds_the_outline_from_the_heading_levels() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    assert_eq!(imported.nodes, 6, "sechs Überschriften (die im Raw-Block zählt nicht): {imported:?}");
    assert_eq!(
        (imported.title.as_str(), imported.author.as_str()),
        ("Das Probebuch", "Erika Mustermann"),
        "Titel und Autor aus #set document"
    );
    assert_eq!(imported.slug, "das-probebuch", "slug aus dem Titel");

    let work = store.load_work(imported.work).await.unwrap();
    assert_eq!(work.kind, baruch_core::WorkKind::Book, "importierte Werke sind Bücher");
    let outline: Vec<(&str, NodeKind, Option<usize>, &str)> =
        work.nodes.iter().map(|n| (n.title.as_str(), n.kind, n.parent, n.number.as_str())).collect();
    assert_eq!(
        outline,
        [
            ("Vorwort", NodeKind::Chapter, None, "1"),
            ("Hauptteil", NodeKind::Chapter, None, "2"),
            ("Erster Gang", NodeKind::Section, Some(1), "2.1"),
            ("Kleine Rast", NodeKind::Section, Some(2), "2.1.1"),
            ("Zweiter Gang", NodeKind::Section, Some(1), "2.2"),
            ("Schluss", NodeKind::Chapter, None, "3"),
        ],
        "= wird Kapitel, ==/=== Abschnitt unter der vorigen kleineren Ebene, Nummern hierarchisch, Label fällt weg"
    );

    for node in &work.nodes {
        assert_eq!(node.revisions.len(), 1, "{}: genau eine Revision nach dem Import", node.title);
        let rev = &node.revisions[0];
        assert_eq!(rev.number, 1, "{}: Revision 1", node.title);
        assert_eq!(rev.status, RevisionStatus::Current, "{}: sofort aktuell", node.title);
        assert_eq!(rev.author, Author::Human { name: "Autor".into() }, "{}: vom Menschen", node.title);
        assert_eq!(rev.note, "Import aus probebuch.typ", "{}: Notiz nennt die Datei", node.title);
    }
    assert!(work.nodes[4].revisions[0].body.contains("= kein Kapitel"), "Raw-Block bleibt im Text des Abschnitts");
    assert!(!work.personas.is_empty(), "Personas des Beispielwerks bleiben unberührt");
}

#[tokio::test]
async fn importing_the_same_file_twice_is_refused_and_creates_nothing() {
    let (_dir, store) = fresh().await;
    let first = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let before = store.works().await.unwrap().len();
    // Dieselbe Quelle mit CRLF und anderem Namen ist dieselbe Datei (Prüfsumme nach CRLF → LF).
    let crlf = BOOK.replace('\n', "\r\n");
    let err = store.import_typst(&crlf, "kopie.typ", "Autor").await.unwrap_err();
    match &err {
        StoreError::AlreadyImported { work, slug, title } => {
            assert_eq!(
                (*work, slug.as_str(), title.as_str()),
                (first.work, "das-probebuch", "Das Probebuch"),
                "Fehler nennt das vorhandene Werk"
            );
        }
        other => panic!("AlreadyImported erwartet, nicht {other:?}"),
    }
    assert_eq!(
        err.to_string(),
        "Diese Datei ist schon importiert: Werk „Das Probebuch“ (das-probebuch).",
        "deutsche Meldung nennt Titel und Ordnername"
    );
    assert_eq!(store.works().await.unwrap().len(), before, "Anzahl der Werke unverändert");
}

#[tokio::test]
async fn a_slug_collision_gets_a_numbered_slug() {
    let (_dir, store) = fresh().await;
    let first = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let other = BOOK.replace("Ende.", "Ein anderes Ende.");
    // Anderer Titel, aber derselbe Ordnername („!“ fällt im slug weg).
    let second =
        store.import_typst_titled(&other, "probebuch-2.typ", "Autor", "Import", Some("Das Probebuch!")).await.unwrap();
    assert_eq!(first.slug, "das-probebuch", "erstes Werk bekommt den slug ohne Zusatz");
    assert_eq!(second.slug, "das-probebuch-2", "gleicher Ordnername → Ordnername mit -2");
    assert_eq!(store.work_by_slug("das-probebuch-2").await.unwrap(), second.work, "Werk über den Ordnernamen");
    assert!(
        matches!(store.work_by_slug("gibt-es-nicht").await, Err(StoreError::UnknownWork(0))),
        "unbekannter Ordnername → UnknownWork(0)"
    );
}

#[tokio::test]
async fn without_set_document_the_title_comes_from_the_file_name() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst("= Eins\r\nText\r\n", "Kleine Predigt.typ", "Autor").await.unwrap();
    assert_eq!(imported.title, "Kleine Predigt", "Titel = Dateiname ohne Endung");
    assert_eq!(imported.author, "", "ohne #set document kein Autor");
    assert_eq!(imported.slug, "kleine-predigt", "slug aus dem Titel aus dem Dateinamen");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned(),
        "= Eins\nText\n",
        "CRLF wird beim Import zu LF"
    );
    assert_eq!(store.work_template(imported.work).await.unwrap(), "", "keine Präambel, leere Vorlage");
}

#[tokio::test]
async fn a_file_without_headings_is_refused_as_import_error() {
    let (_dir, store) = fresh().await;
    let before = store.works().await.unwrap().len();
    let err = store.import_typst("#set text(lang: \"de\")\nNur Text.\n", "leer.typ", "Autor").await.unwrap_err();
    assert!(matches!(err, StoreError::Import(_)), "Import-Fehler erwartet: {err:?}");
    assert!(err.to_string().contains("keine Überschrift"), "Meldung nennt den Grund: {err}");
    assert_eq!(store.works().await.unwrap().len(), before, "nichts angelegt");
}

#[tokio::test]
async fn the_mirror_of_an_imported_work_starts_with_its_template() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let files = store.mirror_snapshot(imported.work).await.unwrap();
    let main = &files[0];
    assert_eq!(main.path, "main.typ", "main.typ steht vorn");
    let template = store.work_template(imported.work).await.unwrap();
    assert!(main.contents.starts_with(&template), "main.typ beginnt mit der Werkvorlage: {}", main.contents);
    let includes: Vec<&str> = main.contents[template.len()..].lines().collect();
    assert_eq!(
        includes,
        [
            "#include \"abschnitte/000-vorwort.typ\"",
            "#include \"abschnitte/001-hauptteil.typ\"",
            "#include \"abschnitte/002-erster-gang.typ\"",
            "#include \"abschnitte/003-kleine-rast.typ\"",
            "#include \"abschnitte/004-zweiter-gang.typ\"",
            "#include \"abschnitte/005-schluss.typ\"",
        ],
        "nach der Vorlage nur die Abschnitte in Gliederungsreihenfolge"
    );
    assert_eq!(files[1].path, "vorlage.typ", "mit Werkvorlage folgt vorlage.typ");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&files[2].contents),
        "#import \"../vorlage.typ\": *\n= Vorwort\n\nKurz und bündig.\n\n",
        "Abschnittsdatei = Import der Vorlage + aktuelle Revision"
    );

    // Das Beispielwerk hat keine Vorlage: main.typ bleibt wie bisher.
    let sample = store.default_work().await.unwrap();
    let files = store.mirror_snapshot(sample).await.unwrap();
    assert!(files[0].contents.starts_with("// Baruch: "), "Werke ohne Vorlage unverändert: {}", files[0].contents);
}

#[tokio::test]
async fn work_list_counts_nodes_per_work() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let list = store.work_list().await.unwrap();
    assert_eq!(list.len(), 2, "Beispielwerk und Import: {list:?}");
    let entry = list.iter().find(|w| w.id == imported.work).unwrap();
    assert_eq!((entry.nodes, entry.author.as_str()), (6, "Erika Mustermann"), "Übersicht mit Knoten und Autor");
    let works = store.works().await.unwrap();
    assert_eq!(
        works.iter().map(|w| w.0).collect::<Vec<_>>(),
        list.iter().map(|w| w.id).collect::<Vec<_>>(),
        "works() und work_list() nennen dieselben Werke in derselben Reihenfolge"
    );
}

#[tokio::test]
async fn a_file_with_a_byte_order_mark_keeps_its_first_chapter() {
    let (_dir, store) = fresh().await;
    let source = "\u{feff}= Einziges Kapitel\nText.\n";
    let imported = store
        .import_typst(source, "bom.typ", "Autor")
        .await
        .expect("BOM vor der einzigen Überschrift darf nicht zu „keine Überschrift“ führen");
    let work = store.load_work(imported.work).await.unwrap();
    let titles: Vec<&str> = work.nodes.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, ["Einziges Kapitel"], "das erste Kapitel ist ein bearbeitbarer Abschnitt, nicht Vorlage");
    assert_eq!(store.work_template(imported.work).await.unwrap(), "\u{feff}", "nur der BOM bleibt Vorlage");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned(),
        source,
        "Export ergibt die Datei samt BOM"
    );
}

#[tokio::test]
async fn an_import_with_its_own_note_writes_it_to_every_revision() {
    let (_dir, store) = fresh().await;
    let imported = store
        .import_typst_noted("= A\nx\n= B\ny\n", "ab.typ", "Autor", "Import aus ab.typ über MCP (@mcp:probe)")
        .await
        .unwrap();
    let work = store.load_work(imported.work).await.unwrap();
    for node in &work.nodes {
        assert_eq!(
            node.revisions[0].note, "Import aus ab.typ über MCP (@mcp:probe)",
            "die Notiz hält fest, wer den Import ausgelöst hat: {}",
            node.title
        );
    }
}

#[tokio::test]
async fn a_second_book_with_the_same_title_is_refused_and_creates_nothing() {
    let (_dir, store) = fresh().await;
    let first = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let before = store.works().await.unwrap().len();
    // Andere Datei (andere Prüfsumme), Titel nur in Groß-/Kleinschreibung und Leerraum anders.
    let other = BOOK.replace("title: \"Das Probebuch\"", "title: \" das  PROBEBUCH \"").replace("Ende.", "Anders.");
    assert_ne!(other, BOOK, "Testaufbau: die zweite Datei unterscheidet sich");
    let err = store.import_typst(&other, "zweites.typ", "Autor").await.unwrap_err();
    match &err {
        StoreError::DuplicateTitle { work, slug, title } => assert_eq!(
            (*work, slug.as_str(), title.as_str()),
            (first.work, "das-probebuch", "Das Probebuch"),
            "Fehler nennt das Werk, das den Titel schon hat"
        ),
        other => panic!("DuplicateTitle erwartet, nicht {other:?}"),
    }
    assert_eq!(err.to_string(), "Es gibt schon ein Werk „Das Probebuch“ (das-probebuch).", "deutsche Meldung");
    assert_eq!(store.works().await.unwrap().len(), before, "nichts angelegt");

    let titled = store.import_typst_titled(&other, "zweites.typ", "Autor", "Import", Some("Probebuch, Band 2")).await;
    let titled = titled.expect("mit eigenem Titel gelingt der Import");
    assert_eq!(
        (titled.title.as_str(), titled.slug.as_str()),
        ("Probebuch, Band 2", "probebuch-band-2"),
        "eigener Titel"
    );
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(titled.work).await.unwrap()).into_owned(),
        other,
        "der Text bleibt die Datei, nur der Titel ist eigen"
    );
}

#[tokio::test]
async fn the_same_file_again_is_already_imported_even_with_another_title() {
    let (_dir, store) = fresh().await;
    store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let err =
        store.import_typst_titled(BOOK, "probebuch.typ", "Autor", "Import", Some("Ganz anders")).await.unwrap_err();
    assert!(
        matches!(err, StoreError::AlreadyImported { .. }),
        "die Prüfsumme kommt vor dem Titel: dieselbe Datei bleibt abgelehnt, auch mit freiem Titel: {err:?}"
    );
    let blank = store.import_typst_titled("= Eins\n", "eins.typ", "Autor", "Import", Some("   ")).await.unwrap();
    assert_eq!(blank.title, "eins", "ein leerer eigener Titel zählt wie keiner (Titel aus dem Dateinamen)");
}

#[tokio::test]
async fn an_import_stores_its_template_as_first_template_revision() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(BOOK, "probebuch.typ", "Autor").await.unwrap();
    let meta = store.work_meta(imported.work).await.unwrap();
    assert_eq!(meta.template_rev, 1, "Werkvorlage nach dem Import in Fassung 1");
    assert_eq!(meta.source_name.as_deref(), Some("probebuch.typ"), "Herkunft bleibt vermerkt");
    let revisions = store.template_revisions(imported.work).await.unwrap();
    assert_eq!(revisions.len(), 1, "genau eine Fassung: {revisions:?}");
    assert_eq!(revisions[0].note, "Import aus probebuch.typ", "Notiz wie bei den Revisionen");
    assert_eq!(
        store.template_revision(imported.work, 1).await.unwrap().as_deref(),
        Some(meta.template.as_str()),
        "Fassung 1 ist die Werkvorlage"
    );
}

// ---- Import mit Bildern (Vertrag Runde 6, Abschnitt 4) -------------------------------------------

const POND: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\"><circle r=\"3\"/></svg>\n";
const POND_SOURCE: &str = "// Teichbuch\n#set document(title: \"Das Teichbuch\")\n\n= Der Teich\n\n\
                           #figure(image(\"/bilder/teich.svg\"), caption: [Teich])\n\n== Ufer\n\n\
                           #image(\"bilder/teich.svg\")\n";

fn pond(path: &str) -> baruch_store::ImportAsset {
    baruch_store::ImportAsset { path: path.into(), file_name: "teich.svg".into(), bytes: POND.into() }
}

#[tokio::test]
async fn importing_with_images_creates_work_images_and_paths_together() {
    let (_dir, store) = fresh().await;
    let assets = [pond("/bilder/teich.svg"), pond("bilder/teich.svg")];
    let (imported, names) = store
        .import_typst_with_assets(POND_SOURCE, "teich.typ", "Autor", "Import aus teich.typ", None, &assets)
        .await
        .unwrap();
    assert_eq!(names, ["teich.svg"], "dieselbe Datei unter zwei Schreibweisen ist ein Bild");
    assert_eq!(imported.nodes, 2, "Werk wie beim Import ohne Bilder: {imported:?}");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned(),
        POND_SOURCE,
        "Vorlage und Text bleiben byte-gleich"
    );
    let list = store.asset_list(imported.work).await.unwrap();
    assert_eq!(list[0].paths, ["/bilder/teich.svg", "bilder/teich.svg"], "asset_path bildet beide Pfade ab");
    let revisions = store.asset_revisions(imported.work, "teich.svg").await.unwrap();
    assert_eq!(
        (revisions.len(), revisions[0].author.as_str(), revisions[0].note.as_str()),
        (1, "Autor", "Import aus teich.typ"),
        "Fassung 1 vom Menschen mit der Import-Notiz"
    );
}

#[tokio::test]
async fn a_broken_image_stops_the_whole_import() {
    let (_dir, store) = fresh().await;
    let before = store.works().await.unwrap();
    let broken = baruch_store::ImportAsset {
        path: "kaputt.png".into(),
        file_name: "kaputt.png".into(),
        bytes: b"kein Bild".to_vec(),
    };
    let assets = [pond("/bilder/teich.svg"), broken];
    let err =
        store.import_typst_with_assets(POND_SOURCE, "teich.typ", "Autor", "Import", None, &assets).await.unwrap_err();
    assert!(matches!(err, StoreError::Asset(_)), "kein Bild → Asset: {err}");
    assert_eq!(store.works().await.unwrap(), before, "eine Transaktion: kein Werk angelegt");
    let images: i64 = sqlx::query_scalar("SELECT count(*) FROM asset").fetch_one(store.pool()).await.unwrap();
    assert_eq!(images, 0, "und kein Bild");
    let again = store.import_typst(POND_SOURCE, "teich.typ", "Autor").await;
    assert!(again.is_ok(), "die Quelle gilt nicht als importiert: {again:?}");
}
