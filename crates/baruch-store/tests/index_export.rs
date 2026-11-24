// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 31b (register-satz): Register und Glossar in Export, Export-Karte und Spiegel (Haken in `snapshot.rs` und
//! `export_map.rs`, Vertrag §2.6). Ohne R31-Einstellungen bleibt alles byte-gleich (Golden aus R29-S1); mit
//! Registerseiten ist der Export = alter Export mit projizierten Abschnitten + Buchende, die Karte byte-gleich zum
//! Export, `main.typ` trägt das Buchende hinter den `#include`-Zeilen, und die Texte in SQLite bleiben unverändert.
//! Echte SQLite-Datei im Temp-Ordner; alle Texte frei erfunden; jede Zusicherung mit Begründung.

use baruch_store::{Store, TypesetSettings, WorkId};
use baruch_typst::index::{BACK_MATTER_MARK, BackMatter, IndexMark, Projector, back_matter, find_marks, mark_source};
use baruch_typst::typeset::{GlossaryPosition, IndexPages};

/// Golden-Quelle und -Export aus Runde 29 (ohne Marken und Stellen).
const SATZ_SOURCE: &str = include_str!("data/satz_quelle.typ");
const GOLDEN_EXPORT: &str = include_str!("data/satz_golden_export.typ");
const GOLDEN_MAIN: &str = include_str!("data/satz_golden_main.typ");

fn source() -> String {
    format!(
        "#set document(title: \"Die Imkerin\", author: \"Mara Feld\")\n\
         #set text(lang: \"de\")\n\n\
         = Frühjahr\n\n\
         Die Imkerin öffnet den Stock{} und zählt die Waben. Wie Joh 3,16 sagt, ist Liebe groß.\n\n\
         // Röm 8,28 nur im Kommentar\n\n\
         == Schwarm\n\n\
         Ein Schwarm{} hängt am Apfelbaum.\n\n\
         = Herbst\n\n\
         Honig{} wird geschleudert, und Ps 23 klingt im Radio.",
        mark_source(&IndexMark::new("stichwort", "Bienenstock")),
        mark_source(&IndexMark { sub: "Apfelbaum".into(), ..IndexMark::new("stichwort", "Schwarm") }),
        mark_source(&IndexMark::new("stichwort", "Honig")),
    )
}

async fn fresh(text: &str) -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(text, "imkerin.typ", "Mara").await.unwrap().work;
    (dir, store, work)
}

fn index_settings() -> TypesetSettings {
    TypesetSettings {
        index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "bibel".into()], columns: 2 }),
        glossary_page: Some(GlossaryPosition::AfterIndexes),
        glossary_first_mention: Some(true),
        ..TypesetSettings::default()
    }
}

async fn main_typ(store: &Store, work: WorkId) -> String {
    store.mirror_snapshot(work).await.unwrap().into_iter().find(|f| f.path == "main.typ").expect("main.typ").contents
}

#[tokio::test]
async fn without_r31_settings_export_map_and_mirror_stay_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(SATZ_SOURCE, "faehre.typ", "Lene").await.unwrap().work;
    store.set_numbering_scheme(work, "decimal", None).await.unwrap();
    assert_eq!(store.export_typst(work).await.unwrap(), GOLDEN_EXPORT, "Export byte-gleich zum Golden aus R29-S1");
    assert_eq!(main_typ(&store, work).await, GOLDEN_MAIN, "main.typ byte-gleich zum Golden aus R29-S1");
    assert_eq!(store.export_map(work).await.unwrap().typst, GOLDEN_EXPORT, "Export-Karte byte-gleich zum Export");

    // Werk mit Marken und Bibelstellen: ohne R31-Einstellungen keine Projektion und kein Buchende.
    let (_dir, store, work) = fresh(&source()).await;
    let before = store.export_typst(work).await.unwrap();
    assert!(!before.contains(BACK_MATTER_MARK), "kein Buchende ohne Einstellungen");
    assert_eq!(find_marks(&before).len(), 3, "nur die drei Marken des Texts, keine automatischen Bibelstellen");
    // Mit R31-Einstellungen ändert sich der Export; nach dem Zurücksetzen ist alles wie vorher.
    store.set_work_typeset(work, &index_settings(), "Mara").await.unwrap();
    assert_ne!(store.export_typst(work).await.unwrap(), before, "mit Einstellungen ändert sich der Export");
    store.reset_work_typeset(work, "Mara").await.unwrap();
    assert_eq!(store.export_typst(work).await.unwrap(), before, "nach dem Zurücksetzen wieder byte-gleich");
}

#[tokio::test]
async fn with_index_pages_the_export_is_the_projected_old_export_plus_the_back_matter() {
    let (_dir, store, work) = fresh(&source()).await;
    let old = store.export_map(work).await.unwrap();
    let old_main = main_typ(&store, work).await;
    store.set_work_typeset(work, &index_settings(), "Mara").await.unwrap();

    // Erwartung aus dem alten Export: Kopf wie vorher (die R31-Felder erzeugen keine Regelzeile), jeder Abschnitt
    // durch die Projektion, dahinter das Buchende aus Store::back_matter.
    let back: BackMatter = store.back_matter(work).await.unwrap();
    assert_eq!(back.settings.index_pages, index_settings().index_pages, "back_matter trägt die Satz-Einstellungen");
    let first = old.sections[0].start;
    let mut projector = Projector::new(&back);
    let bodies: Vec<String> =
        old.sections.iter().map(|s| projector.project(&old.typst[s.start..s.end]).into_owned()).collect();
    let tail = back_matter(&back, &bodies.iter().map(String::as_str).collect::<Vec<_>>());
    let mut expected = format!("{}{}", &old.typst[..first], bodies.concat());
    if !expected.ends_with('\n') {
        expected.push('\n');
    }
    expected.push_str(&tail);

    let export = store.export_typst(work).await.unwrap();
    assert_eq!(export, expected, "Export = projizierte Abschnitte + Buchende hinter dem letzten Abschnitt");
    assert_eq!(export.matches(BACK_MATTER_MARK).count(), 1, "genau ein Buchende");
    assert!(
        export.contains("Joh 3,16#metadata((register: \"bibel\", term: \"Johannes\", sub: \"3,16\""),
        "automatische Bibelstellen-Marke hinter der Stelle im Export"
    );
    assert!(!export.contains("Röm 8,28#metadata"), "Stelle im Kommentar bekommt keine Marke");

    let map = store.export_map(work).await.unwrap();
    assert_eq!(map.typst, export, "export_map(w).typst == export_typst(w), auch mit Buchende");
    let tail_at = export.find(BACK_MATTER_MARK).unwrap();
    let last = map.sections.last().unwrap();
    assert!(last.end <= tail_at, "das Buchende gehört zu keinem Abschnittsbereich");
    for (section, body) in map.sections.iter().zip(&bodies) {
        assert_eq!(&map.typst[section.start..section.end], body.as_str(), "Abschnitt {} projiziert", section.key);
    }

    // Spiegel: main.typ mit Buchende hinter den #include-Zeilen, Abschnittsdateien projiziert.
    let files = store.mirror_snapshot(work).await.unwrap();
    let main = &files.iter().find(|f| f.path == "main.typ").unwrap().contents;
    assert_eq!(main, &format!("{old_main}{tail}"), "main.typ = vorher + Buchende");
    let sections: Vec<&String> =
        files.iter().filter(|f| f.path.starts_with("abschnitte/")).map(|f| &f.contents).collect();
    assert!(
        sections.iter().any(|c| c.contains("Joh 3,16#metadata((register: \"bibel\"")),
        "Abschnittsdatei im Spiegel trägt dieselbe Projektion"
    );

    // SQLite bleibt unverändert: die aktuelle Fassung hat keine automatische Marke.
    let loaded = store.load_work(work).await.unwrap();
    for node in &loaded.nodes {
        let body = node.current_number().and_then(|n| node.revision(n)).map(|r| r.body.as_str()).unwrap_or("");
        assert!(!body.contains("register: \"bibel\""), "gespeicherter Text ohne Projektion: {body}");
    }
}

#[tokio::test]
async fn the_back_matter_lists_registers_in_their_order_with_the_glossary_after_them() {
    let (_dir, store, work) = fresh(&source()).await;
    store.set_work_typeset(work, &index_settings(), "Mara").await.unwrap();
    let export = store.export_typst(work).await.unwrap();
    let tail = &export[export.find(BACK_MATTER_MARK).unwrap()..];
    let keyword = tail.find("[#\"Stichwortregister\"]").expect("Stichwortregister");
    let scripture = tail.find("[#\"Bibelstellenregister\"]").expect("Bibelstellenregister");
    let glossary = tail.find("[#\"Glossar\"]").expect("Glossarseite");
    assert!(keyword < scripture && scripture < glossary, "Reihenfolge der Einstellung, Glossar nach den Registern");
    for term in ["\"Bienenstock\"", "\"Honig\"", "\"Schwarm\"", "\"Apfelbaum\"", "\"Johannes\"", "\"Psalmen\""] {
        assert!(tail.contains(term), "{term} steht im Buchende");
    }
    assert!(tail.contains("#emph(\"Keine Begriffe.\")"), "Glossar ohne angenommene Begriffe (Stand vor glossar-daten)");
}
