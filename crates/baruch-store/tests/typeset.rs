// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satz-Einstellungen im Store (Runde 29a, Vertrag §4, Migration 0048; Aufgabe satz-daten): Lesen, Setzen mit
//! Normalisieren und Prüfen (alle Meldungen, nichts geändert), gleiche Werte ohne Schreiben, stabile uuid,
//! Zurücksetzen und Löschen des Werks mit Grabstein samt Werk-uuid, Übernahme der Vorlagenregel in einer Transaktion,
//! Export ohne Einstellungen byte-gleich zum Golden aus dem Stand R29-S1 und mit Einstellungen = alter Export plus
//! `baruch_typst::typeset::rules(&s)` genau am Haken in `snapshot::head`. Echte SQLite-Datei im Temp-Ordner; alle Texte
//! frei erfunden; jede Zusicherung mit Begründung.

use std::path::Path;

use baruch_store::{Store, StoreError, TypesetSettings, WorkId};
use baruch_typst::typeset::{
    CANONICAL_CHAPTER_BREAK, ChapterStart, Margins, PageFormat, PageNumberPosition, PageNumberStart, PageNumbers,
    Paragraph, RunningHead, rules,
};

/// Quelle des Golden-Tests (frei erfunden).
const SOURCE: &str = include_str!("data/satz_quelle.typ");
/// Export und `main.typ` dieser Quelle mit Schema „decimal“, erzeugt im Stand R29-S1 (vor Migration 0048 und ohne
/// Satz-Einstellungen; Schreiber `write_goldens_from_the_current_state`, ignoriert).
const GOLDEN_EXPORT: &str = include_str!("data/satz_golden_export.typ");
const GOLDEN_MAIN: &str = include_str!("data/satz_golden_main.typ");

async fn fresh() -> (tempfile::TempDir, Store, WorkId) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst(SOURCE, "faehre.typ", "Lene").await.unwrap().work;
    // Ein Gliederungsschema setzt eine eigene Regel hinter die Werkvorlage; der Satz-Block muss dahinter stehen.
    store.set_numbering_scheme(work, "decimal", None).await.unwrap();
    (dir, store, work)
}

fn sample() -> TypesetSettings {
    TypesetSettings {
        chapter_start: Some(ChapterStart::RightPage),
        page_format: Some(PageFormat::A5),
        margins: Some(Margins { inside_mm: 20.0, outside_mm: 15.0, top_mm: 18.0, bottom_mm: 22.0, binding_mm: 5.0 }),
        font_size_pt: Some(10.5),
        paragraph: Some(Paragraph::Indent { em: 1.0 }),
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomOutside,
            start: PageNumberStart::FirstChapter,
        }),
        running_head: Some(RunningHead::Chapter),
        xref_with_page: Some(true),
        ..Default::default()
    }
}

async fn row(store: &Store, work: WorkId) -> Option<(String, String, String, String)> {
    sqlx::query_as("SELECT uuid, settings, updated, updated_by FROM work_typeset WHERE work_id = ?")
        .bind(work)
        .fetch_optional(store.pool())
        .await
        .unwrap()
}

async fn tombstones(store: &Store) -> Vec<(String, Option<String>)> {
    sqlx::query_as("SELECT uuid, work_uuid FROM uuid_tombstone WHERE kind = 'work_typeset' ORDER BY uuid")
        .fetch_all(store.pool())
        .await
        .unwrap()
}

async fn work_uuid(store: &Store, work: WorkId) -> String {
    sqlx::query_scalar("SELECT uuid FROM work WHERE id = ?").bind(work).fetch_one(store.pool()).await.unwrap()
}

async fn template_revisions(store: &Store, work: WorkId) -> Vec<(i64, String, String, String)> {
    sqlx::query_as(
        "SELECT number, body, author_name, note FROM work_template_revision WHERE work_id = ? ORDER BY number",
    )
    .bind(work)
    .fetch_all(store.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn a_work_without_settings_reads_none_and_unknown_works_are_refused() {
    let (_dir, store, work) = fresh().await;
    assert_eq!(store.work_typeset(work).await.unwrap(), None, "ohne Zeile keine Satz-Einstellungen (Vertrag §3)");
    assert!(matches!(store.work_typeset(9999).await, Err(StoreError::UnknownWork(9999))), "Lesen: unbekanntes Werk");
    assert!(
        matches!(store.set_work_typeset(9999, &sample(), "Lene").await, Err(StoreError::UnknownWork(9999))),
        "Setzen: unbekanntes Werk"
    );
    assert!(
        matches!(store.reset_work_typeset(9999, "Lene").await, Err(StoreError::UnknownWork(9999))),
        "Zurücksetzen: unbekanntes Werk"
    );
    let unknown = store.adopt_typeset_rule(9999, &sample(), "#set text(lang: \"de\")\n", "x", "Lene").await;
    assert!(matches!(unknown, Err(StoreError::UnknownWork(9999))), "Übernehmen: unbekanntes Werk: {unknown:?}");
}

#[tokio::test]
async fn setting_stores_normalized_json_keeps_the_uuid_and_skips_equal_values() {
    let (_dir, store, work) = fresh().await;
    let mut wanted = sample();
    wanted.font = Some("  Libertinus Serif  ".into());
    wanted.font_size_pt = Some(10.54);
    wanted.leading_em = Some(0.654);
    let change = store.set_work_typeset(work, &wanted, "Lene").await.unwrap();
    let normalized = wanted.normalized();
    assert_eq!(change.before, TypesetSettings::default(), "vorher keine Einstellungen");
    assert_eq!(change.after, normalized, "gespeichert wird die normalisierte Form (Vertrag §4)");
    assert_eq!(change.after.font.as_deref(), Some("Libertinus Serif"), "Schrift getrimmt");
    assert_eq!(change.after.font_size_pt, Some(10.5), "pt auf 0,1 gerundet");
    assert_eq!(change.after.leading_em, Some(0.65), "em auf 0,01 gerundet");
    assert_eq!(change.changed, TypesetSettings::default().changed_fields(&normalized), "alle gesetzten Felder");
    let (uuid, json, updated, by) = row(&store, work).await.expect("eine Zeile in work_typeset");
    assert_eq!(json, normalized.to_json(), "Spalte settings = kanonisches JSON (Vertrag §2)");
    assert_eq!(by, "Lene", "updated_by = der Mensch");
    assert_eq!(uuid.len(), 36, "uuid über den Fülltrigger");
    let read = store.work_typeset(work).await.unwrap().expect("gelesen");
    assert_eq!((read.settings, read.uuid.clone(), read.updated.clone()), (normalized.clone(), uuid.clone(), updated));

    // Gleiche Werte (auch in anderer, gleich normalisierter Schreibweise): nichts geändert, nichts geschrieben.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let same = store.set_work_typeset(work, &wanted, "Jemand anders").await.unwrap();
    assert!(same.changed.is_empty(), "gleiche Werte → changed leer");
    assert_eq!(row(&store, work).await.unwrap().3, "Lene", "nicht geschrieben: updated_by unverändert");
    assert_eq!(row(&store, work).await.unwrap().2, read.updated, "nicht geschrieben: updated unverändert");

    // Ändern: dieselbe Zeile, dieselbe uuid; changed nennt nur das geänderte Feld.
    let mut other = normalized.clone();
    other.running_head = Some(RunningHead::WorkTitle);
    let change = store.set_work_typeset(work, &other, "Lene").await.unwrap();
    assert_eq!(change.changed, vec!["running_head"], "nur der Kolumnentitel änderte sich");
    assert_eq!(row(&store, work).await.unwrap().0, uuid, "uuid bleibt bei Änderungen gleich (Vertrag §4)");
    assert!(tombstones(&store).await.is_empty(), "Ändern hinterlässt keinen Grabstein");
}

#[tokio::test]
async fn invalid_settings_are_refused_with_every_message_and_change_nothing() {
    let (_dir, store, work) = fresh().await;
    store.set_work_typeset(work, &sample(), "Lene").await.unwrap();
    let before = row(&store, work).await;
    let bad = TypesetSettings {
        margins: Some(Margins { inside_mm: 1.0, outside_mm: 15.0, top_mm: 18.0, bottom_mm: 22.0, binding_mm: 50.0 }),
        font_size_pt: Some(40.0),
        lang: Some("Deutsch".into()),
        ..sample()
    };
    let Err(StoreError::Typeset(message)) = store.set_work_typeset(work, &bad, "Lene").await else {
        panic!("ungültige Einstellungen müssen abgelehnt werden")
    };
    for part in ["Rand innen", "Bundsteg", "Schriftgröße", "Sprache"] {
        assert!(message.contains(part), "alle Meldungen, auch „{part}“: {message}");
    }
    assert!(
        StoreError::Typeset(message.clone()).to_string().starts_with("Satz-Einstellungen: "),
        "Meldung für RPC mit Vorsatz"
    );
    assert_eq!(row(&store, work).await, before, "nichts geändert");
}

#[tokio::test]
async fn resetting_and_empty_settings_leave_a_tombstone_with_the_work_uuid() {
    let (_dir, store, work) = fresh().await;
    let nothing = store.reset_work_typeset(work, "Lene").await.unwrap();
    assert!(nothing.changed.is_empty(), "ohne Zeile ändert Zurücksetzen nichts");
    assert!(tombstones(&store).await.is_empty(), "und hinterlässt keinen Grabstein");

    store.set_work_typeset(work, &sample(), "Lene").await.unwrap();
    let (uuid, ..) = row(&store, work).await.unwrap();
    let change = store.reset_work_typeset(work, "Lene").await.unwrap();
    assert_eq!(change.before, sample().normalized(), "vorher die gespeicherten Einstellungen");
    assert_eq!(change.after, TypesetSettings::default(), "nachher leer");
    assert_eq!(change.changed.len(), 8, "alle acht gesetzten Felder geändert: {:?}", change.changed);
    assert_eq!(row(&store, work).await, None, "Zeile gelöscht");
    assert_eq!(
        tombstones(&store).await,
        vec![(uuid, Some(work_uuid(&store, work).await))],
        "Grabstein der Art work_typeset mit der Werk-uuid (Vertrag §3)"
    );

    // Leere Einstellungen über set = Zurücksetzen; eine neue Zeile bekommt eine neue uuid.
    store.set_work_typeset(work, &sample(), "Lene").await.unwrap();
    let (second, ..) = row(&store, work).await.unwrap();
    let change = store.set_work_typeset(work, &TypesetSettings::default(), "Lene").await.unwrap();
    assert!(!change.changed.is_empty(), "leere Einstellungen setzen zurück");
    assert_eq!(row(&store, work).await, None, "Zeile gelöscht");
    assert_eq!(tombstones(&store).await.len(), 2, "zweiter Grabstein");
    assert!(tombstones(&store).await.iter().any(|(u, _)| *u == second), "mit der uuid der zweiten Zeile");
}

#[tokio::test]
async fn deleting_the_work_leaves_a_tombstone_with_the_work_uuid() {
    let (_dir, store, work) = fresh().await;
    store.set_work_typeset(work, &sample(), "Lene").await.unwrap();
    let (uuid, ..) = row(&store, work).await.unwrap();
    let owner = work_uuid(&store, work).await;
    store.delete_work(work, "Die Fähre im Nebel").await.unwrap();
    // Der Kaskaden-Trigger löscht die Zeile, solange das Werk noch steht; sonst wüsste der Grabstein sein Werk nicht.
    assert_eq!(tombstones(&store).await, vec![(uuid, Some(owner))], "Grabstein mit Werk-uuid nach dem Löschen");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM work_typeset").fetch_one(store.pool()).await.unwrap();
    assert_eq!(left, 0, "keine Zeile bleibt übrig");
}

#[tokio::test]
async fn adopting_sets_settings_and_a_new_template_revision_in_one_transaction() {
    let (_dir, store, work) = fresh().await;
    let template = store.work_template(work).await.unwrap();
    let with_rule = format!("{template}{CANONICAL_CHAPTER_BREAK}\n");
    store
        .update_work(work, &baruch_store::WorkEdit { template: Some(&with_rule), ..Default::default() }, "Lene")
        .await
        .unwrap();
    let revisions_before = template_revisions(&store, work).await;
    let partial = TypesetSettings { font_size_pt: Some(11.0), ..Default::default() };
    store.set_work_typeset(work, &partial, "Lene").await.unwrap();

    // Fehler in einem Teil → nichts geändert: ungültige Einstellungen, Vorlage mit Überschrift, unveränderte Vorlage.
    let invalid = TypesetSettings { font_size_pt: Some(99.0), ..Default::default() };
    let err = store.adopt_typeset_rule(work, &invalid, &template, "Notiz", "Lene").await;
    assert!(matches!(err, Err(StoreError::Typeset(_))), "ungültige Einstellungen: {err:?}");
    let with_heading = format!("{template}= Fremdes Kapitel\n");
    let err = store.adopt_typeset_rule(work, &partial, &with_heading, "Notiz", "Lene").await;
    assert!(matches!(err, Err(StoreError::Refused(_))), "Vorlage mit Überschrift: {err:?}");
    let err = store.adopt_typeset_rule(work, &partial, &with_rule, "Notiz", "Lene").await;
    assert!(matches!(err, Err(StoreError::Refused(_))), "unveränderte Vorlage ist keine neue Fassung: {err:?}");
    assert_eq!(template_revisions(&store, work).await, revisions_before, "keine neue Vorlagenfassung");
    assert_eq!(store.work_template(work).await.unwrap(), with_rule, "Vorlage unverändert");
    assert_eq!(store.work_typeset(work).await.unwrap().unwrap().settings, partial, "Einstellungen unverändert");

    // Erfolg: beides zusammen.
    let wanted = TypesetSettings { chapter_start: Some(ChapterStart::RightPage), ..partial.clone() };
    let adopted = store
        .adopt_typeset_rule(work, &wanted, &template, "Kapitelanfang in die Satz-Einstellungen übernommen", "Lene")
        .await
        .unwrap();
    let last = revisions_before.last().unwrap().0;
    assert_eq!(i64::from(adopted.template_rev), last + 1, "neue Vorlagenfassung mit der nächsten Nummer");
    assert_eq!(adopted.change.changed, vec!["chapter_start"], "nur der Kapitelanfang änderte sich");
    assert_eq!(adopted.change.before, partial, "übrige Felder bleiben");
    let revisions = template_revisions(&store, work).await;
    assert_eq!(
        revisions.last().unwrap(),
        &(last + 1, template.clone(), "Lene".to_owned(), "Kapitelanfang in die Satz-Einstellungen übernommen".into()),
        "Fassung mit Text, Autor und Notiz"
    );
    assert_eq!(store.work_template(work).await.unwrap(), template, "work.template ist die neue Fassung");
    assert_eq!(store.work_typeset(work).await.unwrap().unwrap().settings, wanted, "Einstellungen gesetzt");
}

/// Bytebereich-Anfang des ersten Abschnitts im Export: Davor steht der Kopf aus `snapshot::head` (Werkvorlage,
/// Schema-Regel, Rückfall-Regeln und – mit Einstellungen – der Satz-Block).
async fn head_end(store: &Store, work: WorkId) -> usize {
    store.export_map(work).await.unwrap().sections.first().expect("mindestens ein Abschnitt").start
}

#[tokio::test]
async fn export_and_mirror_without_settings_equal_the_goldens_from_s1() {
    // Vertrag §4/§8.1: Ein Werk ohne Satz-Einstellungen exportiert byte-gleich wie vor Runde 29.
    let (_dir, store, work) = fresh().await;
    assert_eq!(store.export_typst(work).await.unwrap(), GOLDEN_EXPORT, "Export byte-gleich zum Golden aus R29-S1");
    let files = store.mirror_snapshot(work).await.unwrap();
    let main = files.iter().find(|f| f.path == "main.typ").expect("main.typ");
    assert_eq!(main.contents, GOLDEN_MAIN, "main.typ byte-gleich zum Golden aus R29-S1");
    // Nach Zurücksetzen wieder genau so: keine Spur der Einstellungen bleibt im Export.
    store.set_work_typeset(work, &sample(), "Lene").await.unwrap();
    store.reset_work_typeset(work, "Lene").await.unwrap();
    assert_eq!(store.export_typst(work).await.unwrap(), GOLDEN_EXPORT, "nach dem Zurücksetzen wieder byte-gleich");
}

#[tokio::test]
async fn export_with_settings_is_the_old_export_plus_the_rules_at_the_hook() {
    // Vertrag §8.1: alter Export + rules(&s) an der Stelle von S1.2 – unabhängig vom Inhalt der Regeln (satz-typst).
    let (_dir, store, work) = fresh().await;
    let old = store.export_typst(work).await.unwrap();
    let at = head_end(&store, work).await;
    assert!(old[..at].ends_with('\n'), "der Kopf endet mit einem Zeilenende (sonst käme ein Trenner dazu)");
    let old_main = store.mirror_snapshot(work).await.unwrap().into_iter().find(|f| f.path == "main.typ").unwrap();
    let main_at = old_main.contents.find("#include").expect("main.typ bindet Abschnitte ein");

    store.set_work_typeset(work, &sample(), "Lene").await.unwrap();
    let block = rules(&sample().normalized());
    let expected = format!("{}{block}{}", &old[..at], &old[at..]);
    assert_eq!(store.export_typst(work).await.unwrap(), expected, "Export: Block hinter Vorlage und Schema-Regel");
    let map = store.export_map(work).await.unwrap();
    assert_eq!(map.typst, expected, "Export-Karte byte-gleich zum Export");
    assert_eq!(map.sections.first().unwrap().start, at + block.len(), "Abschnitte rücken um den Block");
    let main = store.mirror_snapshot(work).await.unwrap().into_iter().find(|f| f.path == "main.typ").unwrap();
    let expected_main = format!("{}{block}{}", &old_main.contents[..main_at], &old_main.contents[main_at..]);
    assert_eq!(main.contents, expected_main, "main.typ im Spiegel trägt denselben Block an derselben Stelle");
}

#[tokio::test]
#[ignore = "schreibt die Goldens neu; nur bewusst im Stand R29-S1 ausführen"]
async fn write_goldens_from_the_current_state() {
    let (_dir, store, work) = fresh().await;
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    std::fs::write(data.join("satz_golden_export.typ"), store.export_typst(work).await.unwrap()).unwrap();
    let files = store.mirror_snapshot(work).await.unwrap();
    let main = files.into_iter().find(|f| f.path == "main.typ").unwrap();
    std::fs::write(data.join("satz_golden_main.typ"), main.contents).unwrap();
}

// ---- Austausch (Unterversion 6): Ausgabe, neu einlesen, abgleichen ------------------------------------------

const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000029";

async fn store_only() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

#[tokio::test]
async fn exchange_carries_the_settings_and_merging_never_overwrites_silently() {
    use baruch_exchange::sample::{sample_document, sample_uuid};
    use baruch_store::exchange_apply::{ApplyMode, DeviationAction};

    // Neu einlesen: Kennung, Zeitpunkt, Name und Einstellungen wie in der Datei; die Ausgabe gibt sie gleich zurück.
    let doc = sample_document();
    let there = doc.work.typeset.clone().expect("das Beispiel hat Satz-Einstellungen");
    let (_a_dir, a) = store_only().await;
    let applied = a.exchange_apply(&doc, SHA, ApplyMode::New, None).await.unwrap();
    assert_eq!(applied.report.add.typeset, 1, "Satz-Einstellungen zählen im Bericht");
    assert!(applied.report.items.iter().any(|i| i.kind == "satz" && i.uuid == there.uuid), "als Stück „satz“");
    let stored = a.work_typeset(applied.work).await.unwrap().expect("angelegt");
    assert_eq!(
        (stored.uuid.as_str(), stored.updated.as_str(), stored.updated_by.as_str()),
        (there.uuid.as_str(), there.updated.as_str(), there.updated_by.as_str()),
        "Kennung und letzte Änderung aus der Datei"
    );
    assert_eq!(stored.settings, there.settings, "alle sechzehn Felder");
    let dumped = a.exchange_dump(applied.work).await.unwrap();
    assert_eq!(dumped.typeset, Some(there.clone()), "Ausgabe = Eingabe (Rundreise über die Datenbank)");
    assert!(
        dumped.deleted.iter().any(|t| t.uuid == sample_uuid(401) && t.kind.as_db() == "work_typeset"),
        "der Grabstein satz der Datei steht hier und geht wieder hinaus"
    );

    // Abgleichen mit abweichenden Werten derselben Kennung: nichts überschrieben, ein Hinweis nennt Feld und Stände.
    let mut changed = sample_document();
    changed.work.typeset.as_mut().unwrap().settings.running_head = Some(RunningHead::WorkTitle);
    let report = a.exchange_apply(&changed, SHA, ApplyMode::Merge, None).await.unwrap().report;
    let hint = report.deviations.iter().find(|d| d.kind == "satz").expect("Hinweis zur Abweichung");
    assert_eq!(hint.action, DeviationAction::Hinweis, "nur ein Hinweis");
    assert!(hint.label.contains("Kolumnentitel"), "das abweichende Feld steht im Hinweis: {}", hint.label);
    assert!(hint.there.contains("work_title") && hint.here.contains("chapter"), "beide Stände ({hint:?})");
    assert_eq!(a.work_typeset(applied.work).await.unwrap().unwrap().settings, there.settings, "hier unverändert");

    // Ungültige Werte der Datei: nicht übernommen (unter „Nicht übernommen“), das übrige Werk schon.
    let (_b_dir, b) = store_only().await;
    let mut bad = sample_document();
    bad.work.typeset.as_mut().unwrap().settings.font_size_pt = Some(99.0);
    let applied_b = b.exchange_apply(&bad, SHA, ApplyMode::New, None).await.unwrap();
    assert!(applied_b.report.refusals.iter().any(|r| r.contains("Schriftgröße")), "{:?}", applied_b.report.refusals);
    assert_eq!(b.work_typeset(applied_b.work).await.unwrap(), None, "keine ungültigen Einstellungen gespeichert");

    // Fehlen sie hier, kommen sie beim Abgleichen aus der Datei.
    let report = b.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap().report;
    assert_eq!(report.add.typeset, 1, "beim Abgleichen angelegt");
    assert_eq!(b.work_typeset(applied_b.work).await.unwrap().unwrap().uuid, there.uuid, "mit der Kennung der Datei");

    // Hier zurückgesetzt (Grabstein): Die Datei bringt sie nicht wieder.
    b.reset_work_typeset(applied_b.work, "Lene").await.unwrap();
    let report = b.exchange_apply(&doc, SHA, ApplyMode::Merge, None).await.unwrap().report;
    assert!(report.deleted_here.iter().any(|d| d.kind == "satz" && d.uuid == there.uuid), "unter „hier gelöscht“");
    assert_eq!(b.work_typeset(applied_b.work).await.unwrap(), None, "kommt nicht wieder");

    // Dort zurückgesetzt (Grabstein in der Datei): Hier bleiben sie stehen, mit Hinweis.
    let mut reset_there = sample_document();
    let stone = reset_there.work.typeset.take().unwrap();
    reset_there.work.deleted.push(baruch_exchange::ExchangeTombstone {
        uuid: stone.uuid.clone(),
        kind: baruch_exchange::ExchangeTombstoneKind::Typeset,
        deleted: "2026-01-09T10:00:00.000Z".into(),
    });
    let report = a.exchange_apply(&reset_there, SHA, ApplyMode::Merge, None).await.unwrap().report;
    assert!(
        report.deviations.iter().any(|d| d.kind == "satz" && d.uuid == stone.uuid && d.label.contains("bleibt")),
        "Hinweis „dort gelöscht, hier vorhanden – bleibt“: {:?}",
        report.deviations
    );
    assert!(a.work_typeset(applied.work).await.unwrap().is_some(), "hier nicht gelöscht");
}
