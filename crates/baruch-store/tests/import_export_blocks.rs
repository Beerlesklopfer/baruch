// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 21b: blocks that only an export writes behind the work template come back to their place on import:
//! „// Baruch: Satz“ into the typesetting settings, „// Baruch: Überschriften ohne Nummer“ into `node.numbered`.
//! Real SQLite file in the temp folder, freely invented texts.

use baruch_store::Store;
use baruch_typst::typeset::{ChapterStart, Paragraph, TypesetSettings, rules};

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn numbered(store: &Store, work: i64, title: &str) -> i64 {
    sqlx::query_scalar("SELECT numbered FROM node WHERE work_id = ? AND title = ?")
        .bind(work)
        .bind(title)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

const TEMPLATE: &str = "#set document(title: \"Das Wiesenheft\")\n#set heading(numbering: \"1.\")\n";
const BODY: &str = "= Vorwort <vorwort>\n\nKurz.\n\n= Wiese\n\nGras.\n";

#[tokio::test]
async fn a_typeset_rules_block_at_the_end_of_the_template_becomes_typeset_settings() {
    let (_dir, store) = fresh().await;
    let settings = TypesetSettings {
        chapter_start: Some(ChapterStart::RightPage),
        paragraph: Some(Paragraph::Spacing { em: 0.8 }),
        hyphenate: Some(true),
        ..TypesetSettings::default()
    };
    let source = format!("{TEMPLATE}{}{BODY}", rules(&settings));
    let imported = store.import_typst(&source, "wiese.typ", "Autor").await.unwrap();
    let stored = store.work_typeset(imported.work).await.unwrap().expect("Satz-Einstellungen angelegt");
    assert_eq!(stored.settings, settings, "dieselben Einstellungen, die den Block geschrieben haben");
    assert_eq!(stored.updated_by, "Autor", "der Mensch, der importiert, ist der Urheber");
    assert_eq!(store.work_template(imported.work).await.unwrap(), TEMPLATE, "Werkvorlage ohne den Block");
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned();
    assert_eq!(exported, source, "der Export schreibt denselben Block wieder an dieselbe Stelle");
}

#[tokio::test]
async fn an_edited_rules_block_stays_in_the_template() {
    let (_dir, store) = fresh().await;
    let block = rules(&TypesetSettings { justify: Some(false), ..TypesetSettings::default() });
    let edited = format!("{block}#set text(fill: red)\n");
    let source = format!("{TEMPLATE}{edited}{BODY}");
    let imported = store.import_typst(&source, "wiese.typ", "Autor").await.unwrap();
    assert!(store.work_typeset(imported.work).await.unwrap().is_none(), "nichts geraten: keine Einstellungen");
    assert_eq!(
        store.work_template(imported.work).await.unwrap(),
        format!("{TEMPLATE}{edited}"),
        "der geänderte Block bleibt wörtlich in der Werkvorlage"
    );
}

#[tokio::test]
async fn fallback_rules_become_the_number_off_flag_and_leave_the_template() {
    let (_dir, store) = fresh().await;
    let rule = "// Baruch: Überschriften ohne Nummer\n\
                #show heading.where(level: 1, body: [Vorwort]): set heading(numbering: none)\n";
    let source = format!("{TEMPLATE}{rule}{BODY}");
    let imported = store.import_typst(&source, "wiese.typ", "Autor").await.unwrap();
    assert_eq!(numbered(&store, imported.work, "Vorwort").await, 0, "die Regel nannte das Vorwort: Nummer aus");
    assert_eq!(numbered(&store, imported.work, "Wiese").await, 1, "die Wiese zählt weiter");
    assert_eq!(store.work_template(imported.work).await.unwrap(), TEMPLATE, "die Regel ist Merker, nicht Vorlage");
    let numbers: Vec<String> =
        store.load_work(imported.work).await.unwrap().nodes.iter().map(|n| n.number.clone()).collect();
    assert!(
        numbers[0].is_empty() && numbers[1].starts_with('1'),
        "das Vorwort zählt nicht, die Wiese ist Kapitel 1: {numbers:?}"
    );
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned();
    assert_eq!(exported, source, "der Export schreibt dieselbe Regel wieder (Rundreise textgleich)");

    // A foreign line in the block: do not guess.
    let odd = format!("{rule}#show heading: set text(red)\n");
    let other = format!("{TEMPLATE}{odd}= Vorwort <vorwort>\n\nAnders.\n");
    let kept = store.import_typst_titled(&other, "wiese2.typ", "Autor", "Import", Some("Zweites Heft")).await.unwrap();
    assert_eq!(numbered(&store, kept.work, "Vorwort").await, 1, "ohne sicheren Block kein Merker");
    assert!(store.work_template(kept.work).await.unwrap().ends_with(&odd), "der Block bleibt in der Vorlage");
}
