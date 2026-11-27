// SPDX-License-Identifier: AGPL-3.0-or-later
//! Outline schemes in the store (contract Round 6, sections 2c and 2d): numbers of all thirteen schemes on a freely
//! invented outline with parts, chapters, sections, subsections and a chapter without number; preview, setting, errors;
//! export and mirror (byte-identical with „aus der Werkvorlage“). Real SQLite file in the temp directory.

use baruch_store::{NumberingScheme, OutlineAction, Store, StoreError, WorkId};

const HUMAN: &str = "Autor";

/// Template with a depth condition: chapters and sections „A.1“, subsections without number.
const HARBOUR_TEMPLATE: &str =
    "#set heading(numbering: (..n) => if n.pos().len() <= 2 { numbering(\"A.1\", ..n) } else { \"\" })\n\n";

/// The invented outline: (kind, parent, title, is counted).
const HARBOUR: &[(&str, Option<i64>, &str, bool)] = &[
    ("part", None, "Ankunft", true),
    ("chapter", Some(0), "Vorwort", false),
    ("chapter", Some(0), "Hafen", true),
    ("section", Some(2), "Mole", true),
    ("section", Some(3), "Kai", true),
    ("section", Some(3), "Kran", true),
    ("section", Some(2), "Zoll", true),
    ("part", None, "Abfahrt", true),
    ("chapter", Some(7), "Leuchtfeuer", true),
    ("section", Some(8), "Lotse", true),
    ("chapter", Some(7), "Heimkehr", true),
];

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

/// Work „Hafenbuch“ with the outline above, directly in SQLite (parts do not exist on import).
async fn harbour(store: &Store) -> WorkId {
    let pool = store.pool();
    let work: WorkId = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, title_key) \
         VALUES ('hafenbuch', 'book', 'Hafenbuch', 'Hanna Hafen', ?, 'hafenbuch') RETURNING id",
    )
    .bind(HARBOUR_TEMPLATE)
    .fetch_one(pool)
    .await
    .unwrap();
    let mut positions = std::collections::HashMap::new();
    for (seq, (kind, parent, title, numbered)) in HARBOUR.iter().enumerate() {
        let position = positions.entry(*parent).or_insert(0i64);
        let node: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind, numbered) \
             VALUES (?, ?, ?, ?, 'x', ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent)
        .bind(*position)
        .bind(title)
        .bind(kind)
        .bind(i64::from(*numbered))
        .fetch_one(pool)
        .await
        .unwrap();
        *position += 1;
        let level = match (*kind, parent.map(|p| HARBOUR[p as usize].0)) {
            ("part", _) => 0,
            ("chapter", _) => 1,
            (_, Some("section")) => 3,
            _ => 2,
        };
        let body = if level == 0 {
            format!("// Teil {title}\n\n")
        } else {
            format!("{} {title}\n\nText.\n\n", "=".repeat(level))
        };
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status) VALUES (?, 1, ?, 'human', 'Autor', 'current')",
        )
        .bind(node)
        .bind(body)
        .execute(pool)
        .await
        .unwrap();
    }
    work
}

async fn numbers(store: &Store, work: WorkId) -> Vec<String> {
    store.load_work(work).await.unwrap().nodes.iter().map(|n| n.number.clone()).collect()
}

/// Expected numbers per scheme, in the order of [`HARBOUR`].
fn expected(scheme: &str) -> [&'static str; 11] {
    match scheme {
        "template" => ["I", "", "A", "A.1", "", "", "A.2", "II", "B", "B.1", "C"],
        "decimal" => ["I", "", "1", "1.1", "1.1.1", "1.1.2", "1.2", "II", "2", "2.1", "3"],
        "decimal_dot" => ["I", "", "1.", "1.1.", "1.1.1.", "1.1.2.", "1.2.", "II", "2.", "2.1.", "3."],
        "alphanumeric" => ["I", "", "I.", "A.", "1.", "2.", "B.", "II", "II.", "A.", "III."],
        "chapter_roman" => ["I", "", "I", "", "", "", "", "II", "II", "", "III"],
        "chapter_arabic" => ["I", "", "1", "", "", "", "", "II", "2", "", "3"],
        "parts_chapters" => {
            ["Teil I", "", "Kapitel 1", "1.1", "1.1.1", "1.1.2", "1.2", "Teil II", "Kapitel 2", "2.1", "Kapitel 3"]
        }
        "chapter_words" => [
            "Erster Teil",
            "",
            "Erstes Kapitel",
            "",
            "",
            "",
            "",
            "Zweiter Teil",
            "Zweites Kapitel",
            "",
            "Drittes Kapitel",
        ],
        "legal" => ["", "", "§ 1", "Abs. 1", "Nr. 1", "Nr. 2", "Abs. 2", "", "§ 2", "Abs. 1", "§ 3"],
        "sermon" => ["", "", "Einleitung", "", "", "", "", "", "I.", "", "Schluss"],
        "book" => ["Erster Teil", "", "Einleitung", "", "", "", "", "Zweiter Teil", "I.", "", "Schluss"],
        "none" => ["", "", "", "", "", "", "", "", "", "", ""],
        "custom" => ["I", "", "I.", "", "a)", "b)", "", "II", "II.", "", "III."],
        other => panic!("unbekanntes Schema {other}"),
    }
}

#[tokio::test]
async fn every_scheme_numbers_the_invented_outline_in_preview_load_and_database() {
    let (_dir, store) = fresh().await;
    let work = harbour(&store).await;
    assert_eq!(
        numbers(&store, work).await,
        expected("template"),
        "Vorgabe: aus der Werkvorlage (Tiefenbedingung A.1|2)"
    );
    for scheme in NumberingScheme::ALL {
        let id = scheme.id();
        let custom = (scheme == NumberingScheme::Custom).then_some("I.;;a)");
        let preview = store.numbering_preview(work, id, custom).await.unwrap();
        let shown: Vec<&str> = preview.iter().map(|p| p.number.as_str()).collect();
        assert_eq!(shown, expected(id), "Vorschau {id}: Nummern an der eigenen Gliederung");
        let titles: Vec<&str> = preview.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(titles[2], "Hafen", "{id}: die Vorschau nennt die Titel in Baumreihenfolge");
        assert_eq!((preview[4].level, preview[4].node), (3, 4), "{id}: Kai ist Unterabschnitt an Stelle 4");

        if scheme != NumberingScheme::Template {
            let done = store.set_numbering_scheme(work, id, custom).await.unwrap();
            assert_eq!(done.action, OutlineAction::Numbering, "{id}: eine Nummerierungsänderung");
        }
        assert_eq!(numbers(&store, work).await, expected(id), "{id}: Nummern nach dem Laden (Gliederung, Editor-Kopf)");
        let stored: Vec<String> = sqlx::query_scalar("SELECT number FROM node WHERE work_id = ? ORDER BY seq")
            .bind(work)
            .fetch_all(store.pool())
            .await
            .unwrap();
        if scheme != NumberingScheme::Template {
            // The inserted nodes carry „x“; only the setting writes all numbers.
            assert_eq!(stored, expected(id), "{id}: node.number in der Datenbank mitgeschrieben");
        }
        let info = store.outline_numbering(work).await.unwrap();
        assert_eq!((info.scheme.as_str(), info.scheme_label.as_str()), (id, scheme.label()), "{id}: Schema gemeldet");
    }
}

#[tokio::test]
async fn setting_a_scheme_refuses_unknown_schemes_bad_patterns_and_no_change() {
    let (_dir, store) = fresh().await;
    let work = harbour(&store).await;
    let unknown = store.set_numbering_scheme(work, "roemisch", None).await;
    assert!(
        matches!(&unknown, Err(StoreError::Numbering(m)) if m.contains("roemisch")),
        "unbekanntes Schema: {unknown:?}"
    );
    let bad = store.set_numbering_scheme(work, "custom", Some("I.;Nr.")).await;
    assert!(matches!(&bad, Err(StoreError::Numbering(m)) if m.contains("Ebene 2")), "ungültiges Muster: {bad:?}");
    let missing = store.set_numbering_scheme(work, "custom", None).await;
    assert!(matches!(missing, Err(StoreError::Numbering(_))), "eigenes Muster ohne Muster (keins gespeichert)");
    let same = store.set_numbering_scheme(work, "template", None).await;
    assert!(matches!(same, Err(StoreError::Refused(_))), "unverändert wird abgelehnt: {same:?}");
    assert!(
        matches!(store.numbering_preview(work, "kursiv", None).await, Err(StoreError::Numbering(_))),
        "Vorschau auch"
    );
    assert_eq!(numbers(&store, work).await, expected("template"), "nach den Fehlern unverändert");
}

#[tokio::test]
async fn the_own_pattern_survives_switching_to_another_scheme_and_back() {
    let (_dir, store) = fresh().await;
    let work = harbour(&store).await;
    store.set_numbering_scheme(work, "custom", Some("I.;;a)")).await.unwrap();
    let done = store.set_numbering_scheme(work, "legal", None).await.unwrap();
    assert_eq!(done.detail, "Gliederungsschema „Juristisch“", "Betreff für den Spiegel");
    let info = store.outline_numbering(work).await.unwrap();
    assert_eq!(
        (info.scheme.as_str(), info.custom.as_deref(), info.numbering.as_deref()),
        ("legal", Some("I.;;a)"), None),
        "das eigene Muster bleibt gespeichert, Juristisch hat keins"
    );
    assert_eq!(info.effective, NumberingScheme::Legal.example(), "Anzeige eines Schemas mit Wörtern: sein Beispiel");
    let back = store.set_numbering_scheme(work, "custom", None).await.unwrap();
    assert_eq!(back.detail, "Nummerierung „I.;;a)“", "zurück zum gespeicherten eigenen Muster");
    assert_eq!(numbers(&store, work).await, expected("custom"), "dieselben Nummern wie vorher");
    // The old interface: „“ = none, None = template, otherwise a custom pattern.
    assert_eq!(store.set_numbering(work, Some("")).await.unwrap().detail, "keine Nummern", "set_numbering(\"\")");
    assert_eq!(store.outline_numbering(work).await.unwrap().scheme, "none", "→ Schema none");
    store.set_numbering(work, Some("1|1")).await.unwrap();
    assert_eq!(store.outline_numbering(work).await.unwrap().scheme, "custom", "Muster → Schema custom");
    assert_eq!(numbers(&store, work).await, expected("chapter_arabic"), "1|1 = nur Kapitel arabisch");
    store.set_numbering(work, None).await.unwrap();
    assert_eq!(store.outline_numbering(work).await.unwrap().scheme, "template", "None → aus der Werkvorlage");
}

#[tokio::test]
async fn the_template_scheme_keeps_export_and_mirror_byte_identical() {
    let (_dir, store) = fresh().await;
    let source = include_str!("data/probebuch.typ");
    let work = store.import_typst(source, "probebuch.typ", HUMAN).await.unwrap().work;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned(),
        source,
        "Export byte-gleich mit der Quelle"
    );
    let template = store.work_template(work).await.unwrap();
    let files = store.mirror_snapshot(work).await.unwrap();
    let includes: String = files
        .iter()
        .filter(|f| f.path.starts_with("abschnitte/"))
        .map(|f| format!("#include \"{}\"\n", f.path))
        .collect();
    assert_eq!(files[0].contents, format!("{template}{includes}"), "main.typ = Vorlage + #include, wie vor Runde 6");
    assert_eq!(files[1].contents, template, "vorlage.typ = gespeicherte Vorlage");
}

#[tokio::test]
async fn other_schemes_append_their_rule_to_export_and_main_but_not_to_the_stored_template() {
    let (_dir, store) = fresh().await;
    let source = include_str!("data/probebuch.typ");
    let work = store.import_typst(source, "probebuch.typ", HUMAN).await.unwrap().work;
    let template = store.work_template(work).await.unwrap();
    for (scheme, custom) in [
        ("decimal_dot", None),
        ("chapter_roman", None),
        ("chapter_arabic", None),
        ("alphanumeric", None),
        ("custom", Some("A.;1.1;aa")),
        ("legal", None),
        ("none", None),
    ] {
        store.set_numbering_scheme(work, scheme, custom).await.unwrap();
        let label = NumberingScheme::from_id(scheme).unwrap().label();
        let rule = store.numbering_rule(work, scheme, custom).await.unwrap().expect("eine Regel");
        let tail = format!("// Baruch: Gliederungsschema „{label}“\n{rule}");
        let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
        assert!(exported.starts_with(&format!("{template}{tail}")), "{scheme}: Export = Vorlage + Regel + Texte");
        assert_eq!(exported.replacen(&tail, "", 1), source, "{scheme}: sonst byte-gleich");
        let files = store.mirror_snapshot(work).await.unwrap();
        assert!(files[0].contents.starts_with(&format!("{template}{tail}#include")), "{scheme}: main.typ mit Regel");
        assert_eq!(files[1].contents, template, "{scheme}: vorlage.typ bleibt die gespeicherte Vorlage");
        assert_eq!(store.work_template(work).await.unwrap(), template, "{scheme}: work.template unverändert");

        {
            // Round trip: the export imported as a new work yields the same numbers (from the template) – since Round
            // 14 also „Juristisch“ (Numbering::from_template recognises Baruch's own rule).
            let copy = store
                .import_typst_titled(&exported, "kopie.typ", HUMAN, "Kopie", Some(&format!("Kopie {scheme}")))
                .await
                .unwrap()
                .work;
            assert_eq!(numbers(&store, copy).await, numbers(&store, work).await, "{scheme}: Nummern nach Re-Import");
            store.delete_work(copy, &format!("Kopie {scheme}")).await.unwrap();
        }
    }
    assert_eq!(store.numbering_rule(work, "template", None).await.unwrap(), None, "aus der Werkvorlage: keine Regel");
}

#[tokio::test]
async fn a_work_without_template_gets_the_rule_behind_its_title_comment() {
    let (_dir, store) = fresh().await;
    let work = harbour(&store).await;
    sqlx::query("UPDATE work SET template = '' WHERE id = ?").bind(work).execute(store.pool()).await.unwrap();
    let main = |files: Vec<baruch_store::MirrorFile>| files[0].contents.clone();
    assert!(
        main(store.mirror_snapshot(work).await.unwrap()).starts_with("// Baruch: Hafenbuch\n\n#include"),
        "ohne Vorlage und Schema wie bisher"
    );
    store.set_numbering_scheme(work, "decimal", None).await.unwrap();
    assert!(
        main(store.mirror_snapshot(work).await.unwrap()).starts_with(
            "// Baruch: Hafenbuch\n// Baruch: Gliederungsschema „Dezimal (DIN 1421)“\n#set heading(numbering: \"1.1\")\n\n#include"
        ),
        "die Regel steht hinter dem Titelkommentar; das Vorwort ohne Nummer braucht seit Runde 14 keine \
         Show-Regel mehr (es steht als Markierung in seiner Datei)"
    );
}

/// Text of the preface (`numbered = 0`) in SQLite and its projection in the export (Round 14).
const PREFACE_BODY: &str = "= Vorwort\n\nText.\n\n";

fn preface_marker() -> String {
    format!("{}\nText.\n\n", baruch_typst::unnumbered_line(1, "Vorwort"))
}

/// Text of the preface in SQLite (current revision).
async fn preface_in_db(store: &Store, work: WorkId) -> String {
    sqlx::query_scalar(
        "SELECT r.body FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND n.title = 'Vorwort' AND r.status = 'current'",
    )
    .bind(work)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn headings_baruch_does_not_count_are_marked_without_number_in_export_and_mirror() {
    // Contract Round 14, section 4: instead of a show rule `heading.where(level, body)` (Round 6) the preface stands in
    // export and mirror as the marking „ohne Nummer“ – in all schemes with numbers, also „aus der Werkvorlage“. That
    // way Typst does not count it, like Baruch (otherwise all chapters would shift, with „Predigt“ also the total for
    // „Schluss“). The text in SQLite stays unchanged.
    let (_dir, store) = fresh().await;
    let work = harbour(&store).await;
    for scheme in ["template", "decimal", "sermon", "book", "chapter_words", "legal", "parts_chapters"] {
        if scheme != "template" {
            store.set_numbering_scheme(work, scheme, None).await.unwrap();
        }
        let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
        assert!(exported.contains(&preface_marker()), "{scheme}: Vorwort als Markierung im Export: {exported}");
        assert!(!exported.contains(PREFACE_BODY), "{scheme}: keine =-Zeile für das Vorwort mehr");
        assert!(!exported.contains("Überschriften ohne Nummer"), "{scheme}: keine Show-Regel nötig");
        let files = store.mirror_snapshot(work).await.unwrap();
        let section = files.iter().find(|f| f.path.ends_with("-vorwort.typ")).expect("Datei des Vorworts");
        assert!(
            baruch_typst::xref::strip_key_labels(&section.contents).ends_with(&preface_marker()),
            "{scheme}: Spiegel ebenso: {}",
            section.contents
        );
        assert_eq!(preface_in_db(&store, work).await, PREFACE_BODY, "{scheme}: SQLite unverändert");
    }
    store.set_numbering_scheme(work, "none", None).await.unwrap();
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(exported.contains(PREFACE_BODY), "„Ohne Nummern“ nummeriert ohnehin nichts: Text wie gespeichert");

    // Same heading as a numbered node: the marking hits only the preface (Round 6 had to leave out the rule then, and
    // Typst counted the preface).
    sqlx::query(
        "UPDATE revision SET body = '= Vorwort\n\nText.\n\n' WHERE status = 'current' AND node_id = \
         (SELECT id FROM node WHERE work_id = ? AND title = 'Heimkehr')",
    )
    .bind(work)
    .execute(store.pool())
    .await
    .unwrap();
    store.set_numbering_scheme(work, "decimal", None).await.unwrap();
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(
        exported.contains(&preface_marker()) && exported.contains(PREFACE_BODY),
        "Vorwort markiert, der nummerierte Knoten gleichen Titels bleibt =-Zeile: {exported}"
    );
}

#[tokio::test]
async fn a_heading_with_its_own_label_falls_back_to_the_show_rule() {
    // A = line with a label of its own cannot be marked (set_numbered_line → None): for that the show rule of Round 6
    // stays, also with „aus der Werkvorlage“ (then after the template).
    let (_dir, store) = fresh().await;
    let work = harbour(&store).await;
    sqlx::query(
        "UPDATE revision SET body = '= Vorwort <vorwort>\n\nText.\n\n' WHERE status = 'current' AND node_id = \
         (SELECT id FROM node WHERE work_id = ? AND title = 'Vorwort')",
    )
    .bind(work)
    .execute(store.pool())
    .await
    .unwrap();
    let rule = "// Baruch: Überschriften ohne Nummer\n\
                #show heading.where(level: 1, body: [Vorwort]): set heading(numbering: none)\n";
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(
        exported.starts_with(&format!("{HARBOUR_TEMPLATE}{rule}")),
        "aus der Werkvorlage: Show-Regel hinter der Vorlage: {exported}"
    );
    assert!(exported.contains("= Vorwort <vorwort>\n"), "die Zeile mit Label bleibt, wie sie ist");
    store.set_numbering_scheme(work, "legal", None).await.unwrap();
    let rule_line = store.numbering_rule(work, "legal", None).await.unwrap().unwrap();
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    assert!(
        exported
            .starts_with(&format!("{HARBOUR_TEMPLATE}// Baruch: Gliederungsschema „Juristisch“\n{rule_line}{rule}")),
        "andere Schemata: hinter ihrer Regel: {exported}"
    );
}

#[tokio::test]
async fn a_heading_with_code_in_its_title_keeps_its_line_and_falls_back_to_the_show_rule() {
    // Finding review Round 14: the marking escaped the brackets of `#emph[…]`, Typst then typeset „emph“ as raw text. A
    // title with code stays a = line (byte-identical, re-import and switching back without loss), „ohne Nummer“ comes
    // through the show rule; that Typst hits it and typesets the title correctly is checked by
    // `baruch-render/tests/parts.rs`.
    let (_dir, store) = fresh().await;
    let work = harbour(&store).await;
    sqlx::query(
        "UPDATE revision SET body = '= Das #emph[Jahr]\n\nText.\n\n' WHERE status = 'current' AND node_id = \
         (SELECT id FROM node WHERE work_id = ? AND title = 'Vorwort')",
    )
    .bind(work)
    .execute(store.pool())
    .await
    .unwrap();
    let rule = "// Baruch: Überschriften ohne Nummer\n\
                #show heading.where(level: 1, body: [Das #emph[Jahr]]): set heading(numbering: none)\n";
    for scheme in ["template", "decimal"] {
        if scheme != "template" {
            store.set_numbering_scheme(work, scheme, None).await.unwrap();
        }
        let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
        assert!(exported.contains(rule), "{scheme}: Show-Regel als Rückfall: {exported}");
        assert!(exported.contains("\n= Das #emph[Jahr]\n"), "{scheme}: die Zeile bleibt wörtlich: {exported}");
        assert!(!exported.contains("\\[Jahr"), "{scheme}: keine maskierten Klammern im Aufruf");
        assert!(!exported.contains("numbering: none)[Das"), "{scheme}: keine Markierung mit Code im Titel");
    }
}
