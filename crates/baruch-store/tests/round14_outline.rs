// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 14 „Teile und Gliederung“ (R14b) im Store: Teile anlegen (auch mit Werkvorlage, `adopted`),
//! Teile umbenennen, umschalten und speichern (mit und ohne Teil-Markierung), Ebenenwechsel einer
//! Markierung „ohne Nummer“, Vorschau eines Antrags und `idea_to_outline` in einer Transaktion.
//! Echte SQLite-Datei im Temp-Ordner; alle Texte sind frei erfunden.

use baruch_core::{Author, NodeKind, RevisionStatus, Work};
use baruch_store::{
    IdeaOrigin, IdeaStatus, IdeaUse, NewIdea, NewPoint, OutlineAction, OutlineOp, Place, PreviewMark, PreviewPoint,
    Restructure, Store, StoreError, WorkId,
};

/// Werk mit Werkvorlage (`#set …` vor der ersten Überschrift), drei Kapiteln und einem Abschnitt.
const WITH_TEMPLATE: &str = "\
#set document(title: \"Wegzeichen\", author: \"Test\")
#set text(lang: \"de\")

= Aufbruch

Der Rucksack ist gepackt.

== Karte

Die Wege sind eingezeichnet.

= Rast

Am Brunnen.

= Ankunft

Das Ziel.
";

/// Werk ohne Werkvorlage (beginnt sofort mit dem ersten Kapitel).
const PLAIN: &str = "\
= Saat

Körner in der Hand.

= Wachstum

Regen und Sonne.

== Unkraut

Jäten.

= Ernte

Körbe voll.
";

/// Werk mit einem Vorwort und einem Exkurs als Markierung „ohne Nummer“ (Runde 14, Schnittstelle A).
const UNNUMBERED: &str = "\
#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>

Warum dieses Buch.

= Erstes Kapitel

Anfang.

#heading(level: 2, numbering: none)[Exkurs] <baruch-unnumbered> // Randnotiz

Abschweifung.
";

const HUMAN: &str = "Autor";

async fn fresh() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn import(store: &Store, source: &str, name: &str) -> WorkId {
    store.import_typst(source, name, HUMAN).await.unwrap().work
}

fn human() -> Author {
    Author::Human { name: HUMAN.to_owned() }
}

fn part(title: &str) -> NewPoint {
    NewPoint { title: title.to_owned(), level: Some(0) }
}

fn current(work: &Work, seq: usize) -> &baruch_core::Revision {
    let node = &work.nodes[seq];
    &node.revisions[node.current_index().expect("aktuelle Fassung")]
}

fn by_key(work: &Work, key: i64) -> &baruch_core::Node {
    work.nodes.iter().find(|n| n.key == key).expect("Punkt mit diesem Schlüssel")
}

/// Ebene je Knoten aus Art und Elternteil (wie der Store sie sieht).
fn levels(work: &Work) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for node in &work.nodes {
        let level = match node.kind {
            NodeKind::Part => 0,
            NodeKind::Chapter => 1,
            NodeKind::Section => match node.parent.map(|p| out[p]) {
                Some(p) if p >= 2 => 3,
                _ => 2,
            },
        };
        out.push(level);
    }
    out
}

/// (Schlüssel, Nummer, Text der aktuellen Fassung) aller Kapitel und Abschnitte.
fn chapter_state(work: &Work) -> Vec<(i64, String, String)> {
    work.nodes
        .iter()
        .filter(|n| n.kind != NodeKind::Part)
        .map(|n| (n.key, n.number.clone(), current(work, n.id).body.clone()))
        .collect()
}

async fn count(store: &Store, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(store.pool()).await.unwrap()
}

// ---- Teile anlegen ------------------------------------------------------------------------------

#[tokio::test]
async fn a_first_part_before_the_first_chapter_adopts_all_chapters_even_with_a_template() {
    let (_dir, store) = fresh().await;
    let work = import(&store, WITH_TEMPLATE, "wegzeichen.typ").await;
    let before = store.load_work(work).await.unwrap();
    let chapters: Vec<i64> = before.nodes.iter().filter(|n| n.kind == NodeKind::Chapter).map(|n| n.key).collect();

    let added = store.outline_add_many(work, Some(0), Place::Before, &[part("Erster Teil")], &human()).await.unwrap();
    let after = store.load_work(work).await.unwrap();

    // Die Sperre „Teile nur ohne Werkvorlage“ ist gefallen (Vertrag 5): Der Teil steht vorn.
    assert_eq!(after.nodes[0].kind, NodeKind::Part, "der neue Punkt ist ein Teil, auch mit Werkvorlage");
    assert_eq!(after.nodes[0].title, "Erster Teil", "Titel wie angegeben");
    assert_eq!(added.points.len(), 1, "genau ein neuer Punkt");
    assert_eq!(added.points[0].level, 0, "Ebene 0 = Teil");
    // Revision 1 trägt die Teil-Markierung (Vertrag 2), damit Export und Re-Import den Teil erkennen.
    let rev = current(&after, 0);
    assert_eq!(rev.number, 1, "der neue Teil hat genau Revision 1");
    assert_eq!(
        rev.body,
        format!("{}\n", baruch_typst::heading_line(0, "Erster Teil", None)),
        "Revision 1 = heading_line(0, Titel, None) + Leerzeile"
    );
    assert!(
        rev.body.starts_with(&baruch_typst::part_line("Erster Teil")),
        "das ist die Teil-Markierung aus Schnittstelle A"
    );
    assert_eq!(rev.author, human(), "Autor von Revision 1 ist der Anlegende (hier der Mensch)");
    // Alle Kapitel standen auf oberster Ebene; jetzt stehen sie unter dem Teil und werden genannt.
    assert_eq!(added.adopted, chapters, "adopted = alle vorhandenen Kapitel in Baumreihenfolge");
    for key in &chapters {
        assert_eq!(by_key(&after, *key).parent, Some(0), "Kapitel {key} hat den neuen Teil als Elternteil");
    }
    // Texte und Kapitelnummern bleiben: Kapitel zählen über Teile hinweg (load::numbers), und das
    // Anlegen schreibt keine bestehende Fassung um.
    let mut kept = chapter_state(&before);
    kept.sort();
    let mut now = chapter_state(&after);
    now.sort();
    assert_eq!(now, kept, "Schlüssel, Nummern und Texte der Kapitel und Abschnitte unverändert");
    assert_eq!(after.nodes[0].number, "I", "der erste Teil zählt römisch (Vorgabe der Teile)");
    assert_eq!(added.change.action, OutlineAction::Added, "Aktion für den Spiegel: angelegt");
}

#[tokio::test]
async fn a_part_after_a_chapter_in_the_middle_adopts_only_the_following_chapters() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    let before = store.load_work(work).await.unwrap();
    let key_of = |title: &str| before.nodes.iter().find(|n| n.title == title).unwrap().key;
    let (saat, wachstum, unkraut, ernte) = (key_of("Saat"), key_of("Wachstum"), key_of("Unkraut"), key_of("Ernte"));

    // Hinter „Saat“ (seq 0) ohne Werkvorlage.
    let first = store.outline_add_many(work, Some(0), Place::After, &[part("Mitte")], &human()).await.unwrap();
    // Nur direkte Kinder (Kapitel) gelten als übernommen; der Abschnitt bleibt unter seinem Kapitel.
    assert_eq!(first.adopted, vec![wachstum, ernte], "die Kapitel hinter dem neuen Teil bis zum Ende");
    let mid = store.load_work(work).await.unwrap();
    assert_eq!(by_key(&mid, saat).parent, None, "das Kapitel vor dem Teil bleibt auf oberster Ebene");
    assert_eq!(mid.nodes[1].kind, NodeKind::Part, "der Teil steht hinter „Saat“");
    assert_eq!(by_key(&mid, unkraut).parent, Some(by_key(&mid, wachstum).id), "Abschnitt bleibt unter „Wachstum“");

    // Ein zweiter Teil vor „Ernte“ übernimmt nur noch „Ernte“: Ein Teil reicht bis zum nächsten Teil.
    let ernte_seq = by_key(&mid, ernte).id;
    let second =
        store.outline_add_many(work, Some(ernte_seq), Place::Before, &[part("Schluss")], &human()).await.unwrap();
    assert_eq!(second.adopted, vec![ernte], "nur das Kapitel bis zum Werkende bzw. nächsten Teil");
    let after = store.load_work(work).await.unwrap();
    let mitte = after.nodes.iter().find(|n| n.title == "Mitte").unwrap().id;
    assert_eq!(by_key(&after, wachstum).parent, Some(mitte), "„Wachstum“ bleibt beim ersten Teil");
    let numbers: Vec<(&str, &str)> = after.nodes.iter().map(|n| (n.title.as_str(), n.number.as_str())).collect();
    assert_eq!(
        numbers,
        vec![("Saat", "1"), ("Mitte", "I"), ("Wachstum", "2"), ("Unkraut", "2.1"), ("Schluss", "II"), ("Ernte", "3")],
        "Kapitelnummern laufen über Teile fort (unverändert), Teile zählen eigen"
    );
    let mut kept = chapter_state(&before);
    kept.sort();
    let mut now = chapter_state(&after);
    now.sort();
    assert_eq!(now, kept, "keine Fassung eines Kapitels oder Abschnitts wurde umgeschrieben");
}

#[tokio::test]
async fn parts_are_refused_into_beside_sections_and_never_change_their_level() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    let nodes_before = count(&store, "SELECT count(*) FROM node").await;
    let revisions_before = count(&store, "SELECT count(*) FROM revision").await;
    // `into` mit Ebene 0: ein Teil stünde unter einem Punkt (Vertrag 5: -32000 = Refused).
    let into = store.outline_add_many(work, Some(1), Place::Into, &[part("Falsch")], &human()).await;
    assert!(matches!(into, Err(StoreError::Refused(_))), "into mit Ebene 0 abgelehnt: {into:?}");
    // Neben einem Abschnitt („Unkraut“, seq 2) entsteht kein Teil (er risse den Abschnitt vom Kapitel).
    let beside = store.outline_add_many(work, Some(2), Place::After, &[part("Falsch")], &human()).await;
    assert!(matches!(beside, Err(StoreError::Refused(_))), "Teil neben einem Abschnitt abgelehnt: {beside:?}");
    // Ein Teil in einem Block hinter einem Abschnitt ebenso.
    let block = [NewPoint { title: "Neu".into(), level: None }, part("Falsch")];
    let mixed = store.outline_add_many(work, Some(2), Place::After, &block, &human()).await;
    assert!(matches!(mixed, Err(StoreError::Refused(_))), "Block mit Teil neben einem Abschnitt abgelehnt");
    assert_eq!(count(&store, "SELECT count(*) FROM node").await, nodes_before, "nichts angelegt");
    assert_eq!(count(&store, "SELECT count(*) FROM revision").await, revisions_before, "keine Fassung geschrieben");

    // Umstufen zu und von 0 bleibt dem Menschen verwehrt (AGENTS.md: nichts wird zum Teil).
    store.outline_add_many(work, Some(0), Place::Before, &[part("Vorn")], &human()).await.unwrap();
    let to_part = store.outline_set_level(work, 1, 0, HUMAN).await;
    assert!(matches!(to_part, Err(StoreError::Refused(_))), "Kapitel → Teil abgelehnt");
    let from_part = store.outline_set_level(work, 0, 1, HUMAN).await;
    assert!(matches!(from_part, Err(StoreError::Refused(_))), "Teil → Kapitel abgelehnt");
}

#[tokio::test]
async fn assistants_create_parts_with_their_name_on_revision_one() {
    let (_dir, store) = fresh().await;
    let work = import(&store, WITH_TEMPLATE, "wegzeichen.typ").await;
    let author = Author::Assistant { persona: "@mcp:probe".into(), model: "probe 1".into() };
    // Ein Block mit einem Kapitel und dann einem Teil hinter dem ersten Kapitel: Der Teil darf höher
    // stehen als der erste Punkt (Ausnahme Teile) und übernimmt die folgenden Kapitel.
    let block = [NewPoint { title: "Zwischenhalt".into(), level: None }, part("Zweiter Abschnitt der Reise")];
    let added = store.outline_add_many(work, Some(0), Place::After, &block, &author).await.unwrap();
    let after = store.load_work(work).await.unwrap();
    let new_part = &added.points[1];
    assert_eq!(new_part.level, 0, "der zweite Punkt ist ein Teil");
    assert_eq!(
        current(&after, new_part.node).author,
        author,
        "Revision 1 trägt den Assistenten (AGENTS.md: Anlegen erlaubt)"
    );
    let rast = after.nodes.iter().find(|n| n.title == "Rast").unwrap().key;
    let ankunft = after.nodes.iter().find(|n| n.title == "Ankunft").unwrap().key;
    assert_eq!(added.adopted, vec![rast, ankunft], "die vorhandenen Kapitel hinter dem Block stehen unter dem Teil");
    assert!(
        !added.adopted.contains(&added.points[0].key),
        "ein neues Kapitel ist nicht „übernommen“ – adopted nennt nur vorhandene"
    );
}

// ---- Teile umbenennen, speichern, umschalten ------------------------------------------------------

#[tokio::test]
async fn renaming_a_part_with_marker_rewrites_only_the_marker_line_and_keeps_the_comment() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    store.outline_add_many(work, Some(0), Place::Before, &[part("Alt")], &human()).await.unwrap();
    let marked = format!("{} // Randnotiz\n\nEinleitung des Teils.\n", baruch_typst::part_line("Alt").trim_end());
    store.outline_save(work, 0, &marked, None, Restructure::default(), HUMAN).await.unwrap();
    let before = store.load_work(work).await.unwrap();
    let old_rev = current(&before, 0).number;

    let change = store.outline_rename(work, 0, "Neu", HUMAN).await.unwrap();
    let after = store.load_work(work).await.unwrap();
    let rev = current(&after, 0);
    assert_eq!(after.nodes[0].title, "Neu", "der Titel ändert sich");
    assert_eq!(rev.number, old_rev + 1, "Umbenennen schreibt genau eine neue Fassung");
    assert_eq!(rev.author, human(), "die neue Fassung ist vom Menschen (AGENTS.md: Ändern nur Mensch)");
    let first_line = marked.split_inclusive('\n').next().unwrap();
    assert_eq!(
        rev.body,
        format!("{}\nEinleitung des Teils.\n", baruch_typst::heading_line(0, "Neu", Some(first_line))),
        "nur die Markierungszeile ändert sich; der Rest bleibt byte-gleich"
    );
    assert!(rev.body.lines().next().unwrap().ends_with("// Randnotiz"), "der Kommentar bleibt: {:?}", rev.body);
    assert_eq!(change.revisions, vec![(0, rev.number)], "die Änderung nennt die neue Fassung (für den Spiegel)");
}

#[tokio::test]
async fn renaming_an_empty_part_gives_it_the_marker_and_an_old_part_text_stays() {
    let (_dir, store) = fresh().await;
    let example = store.default_work().await.unwrap();
    // Das Beispielwerk hat Teile mit leerem Text (seq 1 und 6, vor Runde 14 angelegt).
    let empty = store.load_work(example).await.unwrap();
    assert_eq!(current(&empty, 1).body, "", "Voraussetzung: Teil I hat leeren Text");
    store.outline_rename(example, 1, "Teil I – Sehen", HUMAN).await.unwrap();
    let after = store.load_work(example).await.unwrap();
    assert_eq!(
        current(&after, 1).body,
        format!("{}\n", baruch_typst::part_line("Teil I – Sehen")),
        "leerer Teiltext → Teil-Markierung mit Leerzeile (wie ein neuer Teil)"
    );

    // Teil II bekommt vom Menschen einen alten Text „= Teil …“ ohne Markierung (Altbestand):
    // Speichern geht ohne Abgleich wie bisher, Umbenennen lässt den Text stehen (Nutzerfrage 8).
    let legacy = "= Teil II – Lösen\n\nAlter Einleitungstext.\n";
    let saved = store.outline_save(example, 6, legacy, None, Restructure::default(), HUMAN).await.unwrap();
    assert!(saved.outline.is_none(), "Teiltext ohne Markierung: keine Gliederungsänderung beim Speichern");
    let rev_before = current(&store.load_work(example).await.unwrap(), 6).number;
    let change = store.outline_rename(example, 6, "Teil II – Handeln", HUMAN).await.unwrap();
    let renamed = store.load_work(example).await.unwrap();
    assert_eq!(renamed.nodes[6].title, "Teil II – Handeln", "der Titel ändert sich");
    assert_eq!(current(&renamed, 6).body, legacy, "der Text ohne Markierung bleibt unverändert");
    assert_eq!(current(&renamed, 6).number, rev_before, "keine neue Fassung");
    assert!(change.revisions.is_empty(), "die Änderung nennt keine neue Fassung");
    assert!(change.detail.contains("ohne Teil-Markierung unverändert"), "das Detail sagt es: {}", change.detail);
}

#[tokio::test]
async fn saving_and_proposing_a_marked_part_text_needs_exactly_one_marker_at_the_start() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    store.outline_add_many(work, Some(0), Place::Before, &[part("Vorspiel")], &human()).await.unwrap();
    let marker = baruch_typst::part_line("Vorspiel");
    let none = Restructure::default();
    let revisions = count(&store, "SELECT count(*) FROM revision").await;

    // Ohne Markierung, mit Text davor, mit einer weiteren Überschrift: abgelehnt, nichts gespeichert.
    for (body, why) in [
        ("Nur Text.\n".to_owned(), "Markierung entfernt"),
        (format!("Davor.\n{marker}\nText.\n"), "Text vor der Markierung"),
        (format!("{marker}\nText.\n\n= Kapitel im Teil\n\nMehr.\n"), "weitere Überschrift"),
    ] {
        let saved = store.outline_save(work, 0, &body, None, none, HUMAN).await;
        assert!(matches!(saved, Err(StoreError::Heading(_))), "{why}: abgelehnt mit Heading, war {saved:?}");
        let proposed = store.add_proposal(work, 0, &body, "@lektor", "m", "").await;
        assert!(matches!(proposed, Err(StoreError::Heading(_))), "{why}: auch als Vorschlag abgelehnt");
    }
    assert_eq!(count(&store, "SELECT count(*) FROM revision").await, revisions, "nichts gespeichert");

    // Mit genau einer Markierung und anderem Titel: speichert und benennt den Teil um.
    let body = format!("{}\nDie Ouvertüre.\n", baruch_typst::part_line("Ouvertüre"));
    let saved = store.outline_save(work, 0, &body, None, none, HUMAN).await.unwrap();
    let outline = saved.outline.expect("ein anderer Titel in der Markierung benennt um");
    assert_eq!(outline.action, OutlineAction::Renamed, "Aktion umbenannt");
    assert_eq!(store.load_work(work).await.unwrap().nodes[0].title, "Ouvertüre", "Titel aus der Markierung");
    // Ein Vorschlag mit Markierung wird angenommen (Assistenten: nur Vorschläge).
    let proposal = format!("{}\nDie Ouvertüre, kürzer.\n", baruch_typst::part_line("Ouvertüre"));
    assert!(
        store.add_proposal(work, 0, &proposal, "@lektor", "m", "").await.is_ok(),
        "Vorschlag mit Markierung angelegt"
    );
    // Prüfen (`outline.check`) meldet dasselbe wie Speichern, ohne zu schreiben.
    let check = store.outline_check(work, 0, "Nur Text.\n").await.unwrap();
    assert!(!check.valid, "Prüfen: ohne Markierung ungültig");
}

#[tokio::test]
async fn switching_to_a_marked_part_version_follows_its_title_and_an_unmarked_one_switches_plainly() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    store.outline_add_many(work, Some(0), Place::Before, &[part("Erste Fassung")], &human()).await.unwrap();
    let second = format!("{}\nText.\n", baruch_typst::part_line("Zweite Fassung"));
    store.outline_save(work, 0, &second, None, Restructure::default(), HUMAN).await.unwrap();
    assert_eq!(store.load_work(work).await.unwrap().nodes[0].title, "Zweite Fassung", "Voraussetzung: Titel 2");
    // Zurück auf Revision 1 (Markierung „Erste Fassung“): der Titel gleicht sich an.
    store.switch_to(work, 0, 1, false, HUMAN).await.unwrap();
    assert_eq!(store.load_work(work).await.unwrap().nodes[0].title, "Erste Fassung", "Umschalten gleicht den Titel ab");

    // Ein Teil des Beispielwerks (leerer Text, keine Markierung) mit einer zweiten Fassung ohne
    // Markierung: Umschalten wie bisher, ohne Abgleich und ohne Fehler.
    let example = store.default_work().await.unwrap();
    store
        .outline_save(example, 1, "Freier Text ohne Überschrift.\n", None, Restructure::default(), HUMAN)
        .await
        .unwrap();
    let title = store.load_work(example).await.unwrap().nodes[1].title.clone();
    store.switch_to(example, 1, 1, false, HUMAN).await.unwrap();
    let after = store.load_work(example).await.unwrap();
    assert_eq!(after.nodes[1].title, title, "Fassung ohne Markierung: Titel bleibt");
    assert_eq!(current(&after, 1).number, 1, "umgeschaltet auf Revision 1");
}

#[tokio::test]
async fn an_unnumbered_marker_keeps_its_form_when_its_level_or_title_changes() {
    let (_dir, store) = fresh().await;
    let work = import(&store, UNNUMBERED, "vorwort.typ").await;
    let loaded = store.load_work(work).await.unwrap();
    let exkurs = loaded.nodes.iter().find(|n| n.title == "Exkurs").expect("Exkurs als Abschnitt").id;
    let old = current(&loaded, exkurs).body.clone();
    let old_line = old.split_inclusive('\n').next().unwrap();

    // Ebenenwechsel 2 → 1 über relevel_line: die Markierung bleibt, nur `level:` ändert sich.
    store.outline_set_level(work, exkurs, 1, HUMAN).await.unwrap();
    let after = store.load_work(work).await.unwrap();
    let body = &current(&after, exkurs).body;
    assert_eq!(
        body,
        &format!("{}{}", baruch_typst::relevel_line(old_line, 1).unwrap(), &old[old_line.len()..]),
        "nur die Ebene in der Markierung ändert sich, Rest byte-gleich"
    );
    assert!(
        body.starts_with("#heading(level: 1, numbering: none)[Exkurs] <baruch-unnumbered>"),
        "Markierung bleibt: {body:?}"
    );
    assert!(body.lines().next().unwrap().ends_with("// Randnotiz"), "Kommentar bleibt");
    assert_eq!(levels(&after)[exkurs], 1, "die Gliederung folgt (Ebene 1)");

    // Umbenennen des Vorworts: heading_line hält die Form „ohne Nummer“ (Schnittstelle A).
    store.outline_rename(work, 0, "Geleitwort", HUMAN).await.unwrap();
    let renamed = store.load_work(work).await.unwrap();
    assert!(
        current(&renamed, 0).body.starts_with(&baruch_typst::unnumbered_line(1, "Geleitwort")),
        "umbenannte Markierung „ohne Nummer“ bleibt Markierung: {:?}",
        current(&renamed, 0).body
    );
}

// ---- Vorschau eines Antrags -----------------------------------------------------------------------

/// Gliederung als Vorschau-Punkte (ohne Marke) aus dem geladenen Werk.
fn outline_rows(work: &Work) -> Vec<(i64, u8, String, String)> {
    let levels = levels(work);
    work.nodes.iter().map(|n| (n.key, levels[n.id], n.number.clone(), n.title.clone())).collect()
}

fn preview_rows(preview: &[PreviewPoint]) -> Vec<(i64, u8, String, String)> {
    preview
        .iter()
        .filter(|p| p.mark != PreviewMark::Deleted)
        .map(|p| (p.key, p.level, p.number.clone(), p.title.clone()))
        .collect()
}

async fn table_counts(store: &Store) -> Vec<i64> {
    let mut out = Vec::new();
    for sql in [
        "SELECT count(*) FROM node",
        "SELECT count(*) FROM revision",
        "SELECT count(*) FROM outline_request WHERE decided IS NULL",
        "SELECT coalesce(sum(length(title)), 0) FROM node",
        "SELECT coalesce(sum(numbered), 0) FROM node",
        "SELECT coalesce(sum(seq * 31 + coalesce(parent_seq, -1)), 0) FROM node",
    ] {
        out.push(count(store, sql).await);
    }
    out
}

#[tokio::test]
async fn the_preview_changes_nothing_and_matches_the_outline_after_confirming() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    let origin = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    let key = |work: &Work, title: &str| work.nodes.iter().find(|n| n.title == title).unwrap().key;
    let loaded = store.load_work(work).await.unwrap();
    let (saat, wachstum, unkraut, ernte) =
        (key(&loaded, "Saat"), key(&loaded, "Wachstum"), key(&loaded, "Unkraut"), key(&loaded, "Ernte"));

    // Jede Art von Antrag nacheinander: Vorschau, Bestätigen, Vergleich. Die erwartete Marke des
    // betroffenen Punkts steht dabei.
    let cases = [
        (OutlineOp::Rename { key: saat, title: "Aussaat".into() }, saat, PreviewMark::Renamed),
        (OutlineOp::Move { key: ernte, target_key: saat, place: Place::Before }, ernte, PreviewMark::Moved),
        (OutlineOp::SetLevel { key: unkraut, level: 1 }, unkraut, PreviewMark::Level),
        (OutlineOp::SetNumbered { key: wachstum, numbered: false }, wachstum, PreviewMark::Numbered),
        (OutlineOp::Delete { key: unkraut }, unkraut, PreviewMark::Deleted),
    ];
    for (op, point, mark) in cases {
        let request = store.outline_request(work, op.clone(), &origin, "Probe").await.unwrap();
        let before = store.load_work(work).await.unwrap();
        let counts = table_counts(&store).await;
        let preview = store.outline_request_preview(work, request.id).await.unwrap();
        // Die Vorschau rollt zurück: Zählung und Gliederung wie vorher.
        assert_eq!(table_counts(&store).await, counts, "{op:?}: Vorschau ändert keine Zeile");
        assert_eq!(
            outline_rows(&store.load_work(work).await.unwrap()),
            outline_rows(&before),
            "{op:?}: Gliederung unverändert"
        );
        let marked = preview.iter().find(|p| p.key == point).expect("der Punkt des Antrags steht in der Vorschau");
        assert_eq!(marked.mark, mark, "{op:?}: Marke des betroffenen Punkts");

        store.outline_request_decide(work, request.id, true, HUMAN).await.unwrap();
        let after = store.load_work(work).await.unwrap();
        // Titel, Ebenen, Nummern und Reihenfolge der Vorschau = Gliederung nach dem Bestätigen.
        assert_eq!(preview_rows(&preview), outline_rows(&after), "{op:?}: Vorschau = Gliederung danach");
        if mark == PreviewMark::Deleted {
            // Gelöschte Punkte stehen an ihrer alten Stelle (hinter ihrem alten Vorgänger).
            let at = preview.iter().position(|p| p.key == point).unwrap();
            let old_index = before.nodes.iter().position(|n| n.key == point).unwrap();
            let previous = before.nodes[old_index - 1].key;
            assert_eq!(preview[at - 1].key, previous, "gelöschter Punkt hinter seinem alten Vorgänger");
        }
        // Andere Punkte ohne Änderung an Titel, Ebene und Nummernart tragen „same“ (oder „moved“
        // als Unterpunkt des verschobenen Punkts).
        for p in preview.iter().filter(|p| p.key != point) {
            assert!(matches!(p.mark, PreviewMark::Same | PreviewMark::Moved), "{op:?}: {} trägt {:?}", p.title, p.mark);
        }
    }
}

#[tokio::test]
async fn decided_or_stale_requests_have_no_preview() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    let origin = IdeaOrigin::Persona { handle: "@lektor".into() };
    let saat = store.load_work(work).await.unwrap().nodes[0].key;
    let first =
        store.outline_request(work, OutlineOp::Rename { key: saat, title: "Säen".into() }, &origin, "").await.unwrap();
    store.outline_request_decide(work, first.id, false, HUMAN).await.unwrap();
    let decided = store.outline_request_preview(work, first.id).await;
    assert!(matches!(decided, Err(StoreError::Refused(_))), "entschiedener Antrag → Refused: {decided:?}");

    let second =
        store.outline_request(work, OutlineOp::Rename { key: saat, title: "Säen".into() }, &origin, "").await.unwrap();
    // Der Mensch benennt inzwischen selbst um: Der Antrag ist veraltet, die Vorschau sagt warum.
    store.outline_rename(work, 0, "Pflanzen", HUMAN).await.unwrap();
    match store.outline_request_preview(work, second.id).await {
        // Runde 23: eigene Variante `Stale` statt `Heading` (der Wortlaut „Veraltet: <Grund>“ bleibt).
        Err(StoreError::Stale(message)) => {
            assert!(message.starts_with("Veraltet: ") && message.contains("Pflanzen"), "Grund genannt: {message}")
        }
        other => panic!("veralteter Antrag → Stale mit Grund, war {other:?}"),
    }
    assert!(
        store.outline_requests(work, true).await.unwrap().iter().any(|r| r.id == second.id),
        "der Antrag bleibt offen"
    );
}

// ---- Einfall als Gliederungspunkt in einer Transaktion -------------------------------------------

async fn idea_for(store: &Store, work: WorkId, key: i64) -> i64 {
    let words = vec!["Brunnen".to_owned()];
    let new = NewIdea {
        work: Some(work),
        key: Some(key),
        words: &words,
        note: "Ein Kapitel über das Wasser.",
        quote: "",
        source: "",
        bible: "",
        origin: IdeaOrigin::Human { name: HUMAN.into() },
    };
    store.idea_create(&new).await.unwrap().id
}

#[tokio::test]
async fn idea_to_outline_adds_the_point_and_closes_the_idea_together() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    let saat = store.load_work(work).await.unwrap().nodes[0].key;
    let idea = idea_for(&store, work, saat).await;
    let done = store.idea_to_outline(idea, work, 0, Place::After, "Brunnen", HUMAN).await.unwrap();
    let after = store.load_work(work).await.unwrap();
    let node = by_key(&after, done.key);
    assert_eq!(node.title, "Brunnen", "der neue Punkt steht im Werk");
    assert_eq!(node.id, done.change.node, "Stelle und Schlüssel gehören zusammen");
    assert_eq!(current(&after, node.id).status, RevisionStatus::Current, "mit aktueller Revision 1");
    assert_eq!(done.idea.status, IdeaStatus::Done, "der Einfall ist verarbeitet");
    assert_eq!(done.idea.done_as, Some(IdeaUse::Outline), "als Gliederungspunkt");
    assert_eq!(
        done.idea.done_ref.as_deref(),
        Some(format!("key {}", done.key).as_str()),
        "done_ref nennt den Schlüssel"
    );
    assert_eq!(store.idea_get(idea).await.unwrap(), done.idea, "so steht er auch in der Datenbank");
}

#[tokio::test]
async fn idea_to_outline_changes_nothing_when_closing_the_idea_fails() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    let saat = store.load_work(work).await.unwrap().nodes[0].key;
    let idea = idea_for(&store, work, saat).await;
    let nodes = count(&store, "SELECT count(*) FROM node").await;
    let revisions = count(&store, "SELECT count(*) FROM revision").await;
    // Der zweite Schritt (Einfall erledigen) scheitert erzwungen: ein Trigger auf UPDATE idea.
    sqlx::query("CREATE TRIGGER test_idea_fails BEFORE UPDATE ON idea BEGIN SELECT RAISE(ABORT, 'Probe'); END")
        .execute(store.pool())
        .await
        .unwrap();
    let failed = store.idea_to_outline(idea, work, 0, Place::After, "Brunnen", HUMAN).await;
    assert!(failed.is_err(), "der Trigger lässt das Erledigen scheitern: {failed:?}");
    sqlx::query("DROP TRIGGER test_idea_fails").execute(store.pool()).await.unwrap();
    // Eine Transaktion: Auch der schon angelegte Punkt ist zurückgerollt.
    assert_eq!(count(&store, "SELECT count(*) FROM node").await, nodes, "kein neuer Knoten");
    assert_eq!(count(&store, "SELECT count(*) FROM revision").await, revisions, "keine neue Revision");
    let open = store.idea_get(idea).await.unwrap();
    assert_eq!(open.status, IdeaStatus::Open, "der Einfall bleibt offen");
    assert_eq!(open.done_ref, None, "ohne Verweis");
    assert!(store.load_work(work).await.unwrap().nodes.iter().all(|n| n.title != "Brunnen"), "kein Punkt „Brunnen“");
}

#[tokio::test]
async fn idea_to_outline_refuses_an_idea_of_another_work() {
    let (_dir, store) = fresh().await;
    let work = import(&store, PLAIN, "garten.typ").await;
    let other = import(&store, WITH_TEMPLATE, "wegzeichen.typ").await;
    let key = store.load_work(other).await.unwrap().nodes[0].key;
    let idea = idea_for(&store, other, key).await;
    let nodes = count(&store, "SELECT count(*) FROM node").await;
    let wrong = store.idea_to_outline(idea, work, 0, Place::After, "Fremd", HUMAN).await;
    assert!(matches!(wrong, Err(StoreError::Idea(_))), "Einfall eines anderen Werks abgelehnt: {wrong:?}");
    assert_eq!(count(&store, "SELECT count(*) FROM node").await, nodes, "nichts angelegt");
}
