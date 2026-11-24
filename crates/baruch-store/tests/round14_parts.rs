// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 14 (Vertrag Abschnitte 1 und 2): Import einer Datei mit Teil-Markierungen und Markierungen
//! „ohne Nummer“ (`baruch-typst/tests/data/teile.typ`, frei erfunden), Export byte-gleich, Spiegel
//! mit den Teil-Abschnitten. Bestehende Texte werden nie umgewandelt.

use baruch_core::NodeKind;
use baruch_store::Store;

const PARTS: &str = include_str!("../../baruch-typst/tests/data/teile.typ");

/// Ohne Werkvorlage: beginnt direkt mit einem Teil.
const PLAIN: &str = "#heading(level: 1, numbering: none)[Anfang] <baruch-part>\n\n= Erstes\nText.\n\n\
#heading(level: 1, numbering: none)[Mitte] <baruch-part> // Notiz\n= Zweites\nMehr.\n";

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

#[tokio::test]
async fn importing_parts_builds_part_nodes_with_chapters_below() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(PARTS, "teile.typ", "Autor").await.unwrap();
    let work = store.load_work(imported.work).await.unwrap();
    let outline: Vec<(&str, NodeKind, Option<usize>)> =
        work.nodes.iter().map(|n| (n.title.as_str(), n.kind, n.parent)).collect();
    assert_eq!(
        outline,
        [
            ("Vorwort", NodeKind::Chapter, None),
            ("Erster Teil: Wasser suchen", NodeKind::Part, None),
            ("Die Wünschelrute", NodeKind::Chapter, Some(1)),
            ("Am Bach", NodeKind::Section, Some(2)),
            ("Zwischenruf [ohne Zahl]", NodeKind::Section, Some(2)),
            ("Tiefer graben", NodeKind::Section, Some(4)),
            ("- Zweiter Teil # mit Zeichen", NodeKind::Part, None),
            ("Der Stein im Weg", NodeKind::Chapter, Some(6)),
            ("= Gleich", NodeKind::Chapter, Some(6)),
            ("2026. ein Jahr", NodeKind::Chapter, Some(6)),
            ("Nachwort", NodeKind::Chapter, Some(6)),
        ],
        "Teil-Markierung → kind 'part' auf oberster Ebene, die folgenden Kapitel stehen darunter \
         (Elternteil = nächster Vorgänger mit kleinerer Ebene); Titel aufgelöst"
    );
}

#[tokio::test]
async fn unnumbered_markers_import_as_numbered_zero_and_chapters_count_across_parts() {
    let (dir, store) = fresh().await;
    let imported = store.import_typst(PARTS, "teile.typ", "Autor").await.unwrap();
    let work = store.load_work(imported.work).await.unwrap();
    let numbers: Vec<(&str, &str)> = work.nodes.iter().map(|n| (n.title.as_str(), n.number.as_str())).collect();
    assert_eq!(
        numbers,
        [
            ("Vorwort", ""),
            ("Erster Teil: Wasser suchen", "I"),
            ("Die Wünschelrute", "1"),
            ("Am Bach", "1.1"),
            ("Zwischenruf [ohne Zahl]", ""),
            ("Tiefer graben", "1.1.1"),
            ("- Zweiter Teil # mit Zeichen", "II"),
            ("Der Stein im Weg", "2"),
            ("= Gleich", ""),
            ("2026. ein Jahr", ""),
            ("Nachwort", "3"),
        ],
        "„ohne Nummer“ hat keine Nummer und zählt nicht mit; Kapitel laufen über Teile fort; \
         Teile zählen eigen (Vorgabe der Nummerierung aus der Werkvorlage)"
    );
    drop(store);
    // numbered direkt in der Datenbank: die Markierung setzt node.numbered = 0, sonst 1.
    let pool =
        sqlx::SqlitePool::connect(&format!("sqlite://{}", dir.path().join("baruch.sqlite").display())).await.unwrap();
    let flags: Vec<(String, i64)> = sqlx::query_as("SELECT title, numbered FROM node WHERE work_id = ? ORDER BY seq")
        .bind(imported.work)
        .fetch_all(&pool)
        .await
        .unwrap();
    let off: Vec<&str> = flags.iter().filter(|(_, n)| *n == 0).map(|(t, _)| t.as_str()).collect();
    assert_eq!(
        off,
        ["Vorwort", "Zwischenruf [ohne Zahl]", "= Gleich", "2026. ein Jahr"],
        "genau die Markierungen „ohne Nummer“"
    );
}

#[tokio::test]
async fn exporting_an_imported_book_with_parts_is_byte_identical() {
    let (_dir, store) = fresh().await;
    for (source, name) in [(PARTS, "teile.typ"), (PLAIN, "ohne-vorlage.typ")] {
        let imported = store.import_typst(source, name, "Autor").await.unwrap();
        let exported =
            baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned();
        assert_eq!(exported, source, "{name}: Import → Export byte-gleich, mit und ohne Werkvorlage");
        let map = store.export_map(imported.work).await.unwrap();
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&map.typst),
            source,
            "{name}: die Karte der Typst-Vorschau ist byte-gleich zum Export"
        );
    }
}

#[tokio::test]
async fn import_keeps_the_texts_literally() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(PARTS, "teile.typ", "Autor").await.unwrap();
    let work = store.load_work(imported.work).await.unwrap();
    let part = &work.nodes[6];
    assert_eq!(
        part.revisions[0].body,
        "#heading(level: 1, numbering: none)[\\- Zweiter Teil \\# mit Zeichen] <baruch-part>\t// Kommentar hinter dem Teil\n\n",
        "Der Text des Teils ist die Markierung wörtlich (mit Kommentar und Leerzeile), nichts wird umgeschrieben"
    );
    assert!(
        work.nodes[0].revisions[0].body.starts_with("#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>"),
        "„ohne Nummer“ bleibt im Text Markierung"
    );
}

#[tokio::test]
async fn the_mirror_contains_the_part_sections() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(PARTS, "teile.typ", "Autor").await.unwrap();
    let files = store.mirror_snapshot(imported.work).await.unwrap();
    let main = files.iter().find(|f| f.path == "main.typ").unwrap();
    let part_files: Vec<&str> = files
        .iter()
        // Erste Zeile nach der Import-Zeile ist die Teil-Markierung (im Raw-Block des Kapitels „Der
        // Stein im Weg“ steht eine zum Schein, die zählt nicht).
        .filter(|f| {
            f.path.starts_with("abschnitte/")
                && f.contents.lines().nth(1).is_some_and(|l| l.contains("] <baruch-part>"))
        })
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(part_files.len(), 2, "beide Teile haben eine eigene Abschnittsdatei: {part_files:?}");
    for path in &part_files {
        assert!(main.contents.contains(&format!("#include \"{path}\"")), "main.typ bindet den Teil {path} ein");
        let file = files.iter().find(|f| f.path == *path).unwrap();
        assert!(
            file.contents.starts_with("#import \"../vorlage.typ\": *\n#heading(level: 1, numbering: none)["),
            "Mit Werkvorlage beginnt auch die Teil-Datei mit der Import-Zeile, dann die Markierung: {:?}",
            file.contents
        );
    }
    let order: Vec<usize> = part_files.iter().map(|p| main.contents.find(p).unwrap()).collect();
    assert!(order[0] < order[1], "Teile in Baumreihenfolge");
}

#[tokio::test]
async fn existing_parts_without_marker_stay_as_they_are() {
    // Altbestand: ein Teil mit „= Teil …“ als Kapitelzeile wird beim Import ein Kapitel und bleibt es
    // (keine automatische Umwandlung, Nutzerfrage 8).
    let (_dir, store) = fresh().await;
    let source = "= Teil I: Altes\nText.\n= Kapitel\nMehr.\n";
    let imported = store.import_typst(source, "alt.typ", "Autor").await.unwrap();
    let work = store.load_work(imported.work).await.unwrap();
    assert!(work.nodes.iter().all(|n| n.kind == NodeKind::Chapter), "„= Teil …“ ist ein Kapitel, kein Teil");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned(),
        source,
        "und geht byte-gleich wieder hinaus"
    );
}

#[tokio::test]
async fn a_part_cannot_be_counted_without_number_because_the_marker_cannot_carry_it() {
    // Befund Gutachten Runde 14: Ein Teil mit numbered = 0 überstand Export → Import nicht (die
    // Teil-Markierung kennt kein „ohne Nummer“, parse liefert Teile nummeriert); danach verschoben sich
    // die Nummern aller folgenden Teile. Darum lehnt set_numbered „Teil ohne Nummer“ ab – direkt (RPC,
    // Mensch) und schon beim Anlegen eines Antrags (MCP, Probelauf).
    use baruch_store::{IdeaOrigin, OutlineOp, StoreError};
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(PLAIN, "teile.typ", "Autor").await.unwrap();
    let work = imported.work;
    let loaded = store.load_work(work).await.unwrap();
    let middle = loaded.nodes.iter().position(|n| n.title == "Mitte").unwrap();
    assert_eq!(loaded.nodes[middle].kind, NodeKind::Part, "Voraussetzung: „Mitte“ ist ein Teil");
    let key = loaded.nodes[middle].key;
    let numbers = |work: &baruch_core::Work| work.nodes.iter().map(|n| n.number.clone()).collect::<Vec<_>>();
    let before = numbers(&loaded);

    let direct = store.outline_set_numbered(work, middle, false, "Mensch").await;
    assert!(matches!(direct, Err(StoreError::Refused(_))), "Teil ohne Nummer direkt abgelehnt: {direct:?}");
    let origin = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let request = store.outline_request(work, OutlineOp::SetNumbered { key, numbered: false }, &origin, "").await;
    assert!(matches!(request, Err(StoreError::Refused(_))), "kein Antrag „Teil ohne Nummer“: {request:?}");
    assert!(store.outline_requests(work, true).await.unwrap().is_empty(), "nichts angelegt");
    assert_eq!(numbers(&store.load_work(work).await.unwrap()), before, "Nummern unverändert");

    // Ein Teil, der aus einem älteren Stand schon ohne Nummer ist, lässt sich wieder mit Nummer zählen
    // (sonst bliebe er hängen).
    sqlx::query("UPDATE node SET numbered = 0 WHERE work_id = ? AND kind = 'part' AND title = 'Mitte'")
        .bind(work)
        .execute(store.pool())
        .await
        .unwrap();
    store.outline_set_numbered(work, middle, true, "Mensch").await.expect("zurück auf „mit Nummer“ geht");
    let healed = store.load_work(work).await.unwrap();
    assert_eq!(numbers(&healed), before, "danach dieselben Nummern wie vorher");

    // Rundreise: Export → Import gibt dieselben Nummern (alle Teile nummeriert).
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(work).await.unwrap()).into_owned();
    let (_other_dir, other) = fresh().await;
    let again = other.import_typst(&exported, "wieder.typ", "Autor").await.unwrap();
    assert_eq!(numbers(&other.load_work(again.work).await.unwrap()), before, "Export → Import: gleiche Nummern");
}

/// Grenzfälle der Rundreise (Gutachten Runde 14): zwei Teile direkt hintereinander (der erste ohne
/// Kapitel) und ein Teil als letzte Überschrift des Werks.
const EDGES: &str = "= Auftakt\nText.\n\n\
#heading(level: 1, numbering: none)[Leerer Teil] <baruch-part>\n\n\
#heading(level: 1, numbering: none)[Voller Teil] <baruch-part>\n= Mitte\nMehr.\n\n\
#heading(level: 1, numbering: none)[Schlussteil] <baruch-part>\n";

#[tokio::test]
async fn parts_without_chapters_and_a_part_at_the_end_survive_the_round_trip() {
    let (_dir, store) = fresh().await;
    let imported = store.import_typst(EDGES, "raender.typ", "Autor").await.unwrap();
    let work = store.load_work(imported.work).await.unwrap();
    let outline: Vec<(&str, NodeKind, Option<usize>)> =
        work.nodes.iter().map(|n| (n.title.as_str(), n.kind, n.parent)).collect();
    assert_eq!(
        outline,
        [
            ("Auftakt", NodeKind::Chapter, None),
            ("Leerer Teil", NodeKind::Part, None),
            ("Voller Teil", NodeKind::Part, None),
            ("Mitte", NodeKind::Chapter, Some(2)),
            ("Schlussteil", NodeKind::Part, None),
        ],
        "Ein Teil ohne Kapitel und ein Teil am Werkende bleiben Teile; das Kapitel hängt am vollen Teil"
    );
    let exported = baruch_typst::xref::strip_key_labels(&store.export_typst(imported.work).await.unwrap()).into_owned();
    assert_eq!(exported, EDGES, "Import → Export byte-gleich, auch mit leerem Teil und Teil am Ende");
    let (_other_dir, other) = fresh().await;
    let again = other.import_typst(&exported, "wieder.typ", "Autor").await.unwrap();
    let again_work = other.load_work(again.work).await.unwrap();
    let again_outline: Vec<(&str, NodeKind, Option<usize>)> =
        again_work.nodes.iter().map(|n| (n.title.as_str(), n.kind, n.parent)).collect();
    assert_eq!(again_outline, outline, "Re-Import ergibt dieselbe Gliederung");
}
