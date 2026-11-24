// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 26 „export“: `Store::exchange_dump` bildet ein Werk vollständig auf das Austauschmodell ab.
//!
//! Das Werk entsteht über die echten Wege des Stores (Typst-Import, Vorschläge, Übernehmen, Verwerfen, Löschen,
//! Einfälle, Beiträge, Faden, Bilder, Buchangaben, Sprechdauer, Bibelmodul, SWORD-Quellen); alle Texte sind frei
//! erfunden. Daneben liegt das gesäte Beispielwerk, aus dem nichts in die Ausgabe gelangen darf.

use std::collections::{HashMap, HashSet};

use baruch_core::{Post, TextRef};
use baruch_exchange::*;
use baruch_store::{BookEdit, IdeaOrigin, IdeaUse, NewAsset, NewIdea, Store, StoreError, TimingSettings, WorkEdit};

const SOURCE: &str = "#set text(lang: \"de\")\n\n\
#heading(level: 1, numbering: none)[Erster Teil] <baruch-part>\n\n\
= Der Krug\nEin Krug steht am Tor.\n\n\
#heading(level: 1, numbering: none)[Zwischenstück] <baruch-unnumbered>\nOhne Nummer.\n\n\
== Henkel\nDer Henkel bricht nicht.\n\n\
#heading(level: 1, numbering: none)[Zweiter Teil] <baruch-part>\n\n\
= Die Schale\nEine Schale aus Ton.\n";

const PNG_GREEN: &[u8] = sample::SAMPLE_PNG_GREEN;
const PNG_RED: &[u8] = sample::SAMPLE_PNG_RED;
const SVG_ONE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"4\" height=\"4\"/></svg>\n";
const SVG_TWO: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><circle r=\"2\" cx=\"2\" cy=\"2\"/></svg>\n";

/// Alle UUIDs der Ausgabe je Art (Art wie in `uuid_tombstone.kind`).
fn collect(work: &ExchangeWork) -> HashMap<&'static str, Vec<String>> {
    fn nodes(list: &[ExchangeNode], out: &mut HashMap<&'static str, Vec<String>>) {
        for node in list {
            out.entry("node").or_default().push(node.uuid.clone());
            out.entry("revision").or_default().extend(node.revisions.iter().map(|r| r.uuid.clone()));
            out.entry("post").or_default().extend(node.posts.iter().map(|p| p.uuid.clone()));
            nodes(&node.children, out);
        }
    }
    let mut out: HashMap<&'static str, Vec<String>> = HashMap::new();
    out.entry("work").or_default().push(work.uuid.clone());
    nodes(&work.outline, &mut out);
    out.entry("template_revision").or_default().extend(work.template.revisions.iter().map(|r| r.uuid.clone()));
    out.entry("idea").or_default().extend(work.ideas.iter().map(|i| i.uuid.clone()));
    out.entry("thread").or_default().extend(work.threads.iter().map(|t| t.uuid.clone()));
    out.entry("thread_post")
        .or_default()
        .extend(work.threads.iter().flat_map(|t| t.posts.iter().map(|p| p.uuid.clone())));
    out.entry("asset").or_default().extend(work.assets.iter().map(|a| a.uuid.clone()));
    out.entry("asset_revision")
        .or_default()
        .extend(work.assets.iter().flat_map(|a| a.revisions.iter().map(|r| r.uuid.clone())));
    out
}

/// UUIDs einer Tabelle für ein Werk direkt aus SQLite (die Quelle).
async fn table_uuids(store: &Store, kind: &str, work: i64) -> Vec<String> {
    let sql = match kind {
        "work" => "SELECT uuid FROM work WHERE id = ?",
        "node" => "SELECT uuid FROM node WHERE work_id = ?",
        "revision" => "SELECT r.uuid FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ?",
        "template_revision" => "SELECT uuid FROM work_template_revision WHERE work_id = ?",
        "post" => "SELECT p.uuid FROM post p JOIN node n ON n.id = p.node_id WHERE n.work_id = ?",
        "idea" => "SELECT uuid FROM idea WHERE work_id = ?",
        "thread" => "SELECT uuid FROM chat_thread WHERE work_id = ?",
        "thread_post" => {
            "SELECT p.uuid FROM chat_thread_post p JOIN chat_thread t ON t.id = p.thread_id WHERE t.work_id = ?"
        }
        "asset" => "SELECT uuid FROM asset WHERE work_id = ?",
        "asset_revision" => "SELECT r.uuid FROM asset_revision r JOIN asset a ON a.id = r.asset_id WHERE a.work_id = ?",
        other => panic!("unbekannte Art {other}"),
    };
    let mut list: Vec<String> = sqlx::query_scalar(sql).bind(work).fetch_all(store.pool()).await.unwrap();
    list.sort();
    list
}

async fn revision_uuid(store: &Store, work: i64, seq: i64, number: i64) -> String {
    sqlx::query_scalar(
        "SELECT r.uuid FROM revision r JOIN node n ON n.id = r.node_id WHERE n.work_id = ? AND n.seq = ? AND r.number = ?",
    )
    .bind(work)
    .bind(seq)
    .bind(number)
    .fetch_one(store.pool())
    .await
    .unwrap()
}

async fn node_uuid_at(store: &Store, work: i64, seq: i64) -> (i64, String) {
    sqlx::query_as("SELECT id, uuid FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn idea_uuid(store: &Store, id: i64) -> String {
    sqlx::query_scalar("SELECT uuid FROM idea WHERE id = ?").bind(id).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn dump_covers_every_row_once_with_uuids_and_nothing_else() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let other = store.works().await.unwrap()[0].0;
    let imported = store.import_typst(SOURCE, "krug.typ", "Hanna Probe").await.unwrap();
    let work = imported.work;

    // Gliederung: 0 Erster Teil, 1 Der Krug, 2 Zwischenstück (ohne Nummer), 3 Henkel, 4 Zweiter Teil, 5 Die Schale.
    // „Der Krug“ bekommt vier Fassungen je Status und eine fünfte, die gelöscht wird (Grabstein).
    let jug = 1usize;
    let p2 = store
        .add_proposal(work, jug, "= Der Krug\nEin Krug aus Ton.\n", "@lektor", "probe/modell-x", "Ton")
        .await
        .unwrap();
    store.accept(work, jug, p2).await.unwrap();
    let p3 = store.add_proposal(work, jug, "= Der Krug\nVerworfen.\n", "@lektor", "probe/modell-x", "").await.unwrap();
    store.reject(work, jug, p3).await.unwrap();
    let p4 = store.add_proposal(work, jug, "= Der Krug\nOffen.\n", "@lektor", "probe/modell-x", "").await.unwrap();
    let p5 = store.add_proposal(work, jug, "= Der Krug\nWeg damit.\n", "@lektor", "probe/modell-x", "").await.unwrap();
    let gone = revision_uuid(&store, work, 1, p5 as i64).await;
    store.delete_revision(work, jug, p5, "Hanna Probe").await.unwrap();
    assert_eq!((p2, p3, p4), (2, 3, 4), "Nummern der Fassungen");

    // Werkvorlage mit zwei Fassungen.
    let edit = WorkEdit {
        title: None,
        subtitle: Some("Ein erfundenes Werk"),
        author: None,
        kind: None,
        audience: None,
        bible: None,
        template: Some("#set page(paper: \"a5\")\n#set text(lang: \"de\")\n\n"),
        note: Some("A5"),
    };
    store.update_work(work, &edit, "Hanna Probe").await.unwrap();

    // Einfälle: einer mit Bezug, einer erledigt (als Gliederungspunkt „key N“), einer gelöscht; dazu einer im
    // anderen Werk, der nicht erscheinen darf.
    let (jug_key, jug_uuid) = node_uuid_at(&store, work, 1).await;
    let words = vec!["Krug".to_owned(), "Ton".to_owned()];
    let new_idea = |key: Option<i64>, note: &'static str, w: Option<i64>| NewIdea {
        work: w,
        key,
        words: &words,
        note,
        quote: "",
        source: "",
        bible: "",
        origin: IdeaOrigin::Human { name: "Hanna Probe".into() },
    };
    let with_ref = store.idea_create(&new_idea(Some(jug_key), "Der Krug als Bild.", Some(work))).await.unwrap();
    let done = store.idea_create(&new_idea(None, "Ein neues Kapitel.", Some(work))).await.unwrap();
    store.idea_mark_done(done.id, IdeaUse::Outline, &format!("key {jug_key}")).await.unwrap();
    let deleted = store.idea_create(&new_idea(None, "Zu viel Krug.", Some(work))).await.unwrap();
    store.idea_delete(deleted.id).await.unwrap();
    store.idea_create(&new_idea(None, "Fremder Einfall.", Some(other))).await.unwrap();

    // Beiträge im Abschnitts-Chat von „Der Krug“ mit Quellen und Bezügen (auf „Die Schale“ Fassung 1 und auf
    // „Der Krug“ Fassung 2).
    let post = |body: &str, refs: Vec<TextRef>| Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: body.into(),
        sources: vec!["Probequelle, S. 7".into()],
        refs,
    };
    store
        .add_post(work, jug, &post("Siehe die Schale.", vec![TextRef { node: 5, rev: 1, quote: "Schale".into() }]))
        .await
        .unwrap();
    store
        .add_post(work, jug, &post("Der Ton passt.", vec![TextRef { node: 1, rev: 2, quote: "Ton".into() }]))
        .await
        .unwrap();

    // Faden mit zwei Beiträgen.
    let (thread, _) = store.thread_create(work, "@lektor", "Wie alt ist der Krug?").await.unwrap();
    store
        .thread_add_post(work, thread.id, "Hanna Probe", "Hundert Jahre, erfunden.", &["Notizbuch".into()])
        .await
        .unwrap();

    // Zwei Bilder mit je zwei Fassungen und Pfaden; Umschlag und Autorenfoto.
    let asset = |file: &'static str, bytes: &'static [u8], path: &'static str| NewAsset {
        file_name: file,
        name: None,
        bytes,
        as_base64: false,
        path: Some(path),
        caption: "Unterschrift",
        alt: "Alternativtext",
        note: "",
    };
    store.add_asset(work, &asset("krug.png", PNG_GREEN, "bilder/krug.png"), "Hanna Probe").await.unwrap();
    store.replace_asset(work, "krug.png", "krug.png", PNG_RED, false, "rot", "Hanna Probe").await.unwrap();
    store.add_asset(work, &asset("henkel.svg", SVG_ONE.as_bytes(), "bilder/henkel.svg"), "Hanna Probe").await.unwrap();
    store.replace_asset(work, "henkel.svg", "henkel.svg", SVG_TWO.as_bytes(), false, "", "Hanna Probe").await.unwrap();
    store.set_asset_paths(work, "henkel.svg", &["bilder/henkel.svg".into(), "abb/henkel.svg".into()]).await.unwrap();
    store.set_work_cover(work, Some("krug.png")).await.unwrap();
    let book = BookEdit {
        publisher: Some("Probeverlag".into()),
        dedication: Some("Für die Töpfer.".into()),
        author_photo: Some(Some("henkel.svg".into())),
        ..BookEdit::default()
    };
    store.set_book_meta(work, &book).await.unwrap();
    store.set_timing_settings(work, TimingSettings { words_per_minute: 140, target_seconds: Some(900) }).await.unwrap();
    store.set_work_bible(work, Some("ProbeBibel")).await.unwrap();
    store.set_work_sword_sources(work, &["ProbeKommentar".into()], &["ProbeLexikon".into()]).await.unwrap();

    let dump = store.exchange_dump(work).await.unwrap();

    // ---- Jede Zeile genau einmal mit ihrer uuid, nichts aus dem anderen Werk ----
    let seen = collect(&dump);
    for kind in [
        "work",
        "node",
        "revision",
        "template_revision",
        "post",
        "idea",
        "thread",
        "thread_post",
        "asset",
        "asset_revision",
    ] {
        let mut listed = seen.get(kind).cloned().unwrap_or_default();
        let before = listed.len();
        listed.sort();
        listed.dedup();
        assert_eq!(listed.len(), before, "{kind}: keine UUID doppelt");
        assert_eq!(listed, table_uuids(&store, kind, work).await, "{kind}: genau die Zeilen des Werks");
        assert!(listed.iter().all(|u| is_uuid(u)), "{kind}: kanonische UUIDs");
        let foreign = table_uuids(&store, kind, other).await;
        assert!(listed.iter().all(|u| !foreign.contains(u)), "{kind}: nichts aus dem anderen Werk");
    }
    assert_eq!(seen["revision"].len(), 9, "6 Punkte mit je einer Fassung, „Der Krug“ mit vier");
    assert_eq!(seen["asset_revision"].len(), 4, "zwei Bilder mit je zwei Fassungen");
    assert_eq!(seen["idea"].len(), 3, "drei Einfälle, auch der gelöschte, nicht der fremde");

    // ---- Werk und Angaben ----
    assert_eq!(dump.uuid, store.work_uuid(work).await.unwrap(), "Werk-UUID");
    assert_eq!(dump.kind, ExchangeWorkKind::Book, "Buch");
    assert_eq!(dump.subtitle, "Ein erfundenes Werk", "Untertitel");
    assert_eq!(dump.bible_module.as_deref(), Some("ProbeBibel"), "Bibelmodul");
    assert_eq!(
        dump.sword_sources,
        vec![
            ExchangeSwordSource { kind: ExchangeSourceKind::Commentary, module: "ProbeKommentar".into() },
            ExchangeSwordSource { kind: ExchangeSourceKind::Lexicon, module: "ProbeLexikon".into() },
        ],
        "SWORD-Quellen"
    );
    assert_eq!(dump.timing, Some(ExchangeTiming { words_per_minute: 140, target_seconds: Some(900) }), "Sprechdauer");

    // ---- Vorlage: aktueller Text und alle Fassungen nach Nummer ----
    let meta = store.work_meta(work).await.unwrap();
    assert_eq!(dump.template.current, meta.template, "aktueller Vorlagentext = work.template");
    let numbers: Vec<u32> = dump.template.revisions.iter().map(|r| r.number).collect();
    let expected: Vec<u32> = store.template_revisions(work).await.unwrap().iter().map(|r| r.number).collect();
    assert_eq!(numbers, expected, "alle Vorlagenfassungen nach Nummer");
    assert!(numbers.len() >= 2, "mindestens zwei Vorlagenfassungen: {numbers:?}");
    assert_eq!(dump.template.revisions.last().unwrap().body, meta.template, "letzte Fassung = aktueller Text");

    // ---- Gliederung als Baum in der Reihenfolge der Oberfläche ----
    let tree: Vec<(&str, ExchangeNodeKind, bool, Vec<&str>)> = dump
        .outline
        .iter()
        .map(|n| {
            (n.title.as_str(), n.kind, n.numbered, n.children.iter().map(|c| c.title.as_str()).collect::<Vec<_>>())
        })
        .collect();
    assert_eq!(
        tree,
        vec![
            ("Erster Teil", ExchangeNodeKind::Part, true, vec!["Der Krug", "Zwischenstück"]),
            ("Zweiter Teil", ExchangeNodeKind::Part, true, vec!["Die Schale"]),
        ],
        "Teile mit ihren Kapiteln in Reihenfolge"
    );
    let between = &dump.outline[0].children[1];
    assert!(!between.numbered, "„Zwischenstück“ ohne Nummer");
    assert_eq!(between.children.len(), 1, "Henkel unter Zwischenstück");
    assert_eq!(between.children[0].kind, ExchangeNodeKind::Section, "Henkel ist ein Abschnitt");

    // ---- Fassungen von „Der Krug“: Nummer, Status, Autor, basis über UUIDs ----
    let jug_node = &dump.outline[0].children[0];
    assert_eq!(jug_node.uuid, jug_uuid, "Punkt-UUID");
    let r: Vec<(u32, ExchangeRevisionStatus)> = jug_node.revisions.iter().map(|r| (r.number, r.status)).collect();
    assert_eq!(
        r,
        vec![
            (1, ExchangeRevisionStatus::Superseded),
            (2, ExchangeRevisionStatus::Current),
            (3, ExchangeRevisionStatus::Rejected),
            (4, ExchangeRevisionStatus::Proposed),
        ],
        "vier Fassungen je Status nach Nummer; die gelöschte fünfte fehlt"
    );
    let uuid_of = |n: usize| jug_node.revisions[n - 1].uuid.clone();
    assert_eq!(jug_node.revisions[1].based_on, Some(uuid_of(1)), "Fassung 2 beruht auf Fassung 1");
    assert_eq!(jug_node.revisions[2].based_on, Some(uuid_of(2)), "Vorschlag 3 beruht auf der aktuellen 2");
    assert_eq!(jug_node.revisions[0].based_on, None, "Fassung 1 ist Wurzel");
    assert_eq!(
        jug_node.revisions[1].author,
        ExchangeAuthor::Assistant { persona: "@lektor".into(), model: "probe/modell-x".into() },
        "Modell der Fassung als Herkunft"
    );
    assert_eq!(jug_node.revisions[1].note, "Ton", "Notiz");
    assert!(!seen["revision"].contains(&gone), "gelöschte Fassung nicht unter den Fassungen");
    assert!(
        dump.deleted.iter().any(|t| t.uuid == gone && t.kind == ExchangeTombstoneKind::Revision),
        "gelöschte Fassung als Grabstein: {:?}",
        dump.deleted
    );

    // ---- Beiträge: Reihenfolge ab 1, Quellen, Bezüge über UUIDs ----
    let positions: Vec<u32> = jug_node.posts.iter().map(|p| p.position).collect();
    assert_eq!(positions, vec![1, 2], "Beiträge in Reihenfolge ab 1");
    assert_eq!(jug_node.posts[0].sources, vec!["Probequelle, S. 7".to_owned()], "Quellen");
    let (_, bowl_uuid) = node_uuid_at(&store, work, 5).await;
    assert_eq!(
        jug_node.posts[0].refs,
        vec![ExchangePostRef {
            node: bowl_uuid.clone(),
            revision: Some(revision_uuid(&store, work, 5, 1).await),
            quote: "Schale".into()
        }],
        "Bezug auf „Die Schale“ Fassung 1 über UUIDs"
    );
    assert_eq!(jug_node.posts[1].refs[0].revision, Some(uuid_of(2)), "Bezug auf „Der Krug“ Fassung 2");

    // ---- Einfälle ----
    let idea = |uuid: &str| dump.ideas.iter().find(|i| i.uuid == uuid).unwrap();
    let referenced = idea(&idea_uuid(&store, with_ref.id).await);
    assert_eq!(referenced.node.as_deref(), Some(jug_uuid.as_str()), "bezug über die Punkt-UUID");
    assert_eq!(referenced.words, words, "Schlagwörter in Reihenfolge");
    let finished = idea(&idea_uuid(&store, done.id).await);
    assert_eq!(finished.status, ExchangeIdeaStatus::Done, "erledigt");
    assert_eq!(finished.done_as, Some(ExchangeIdeaUse::Outline), "als Gliederungspunkt");
    assert_eq!(finished.done_ref.as_deref(), Some(jug_uuid.as_str()), "„key N“ → UUID des Punkts");
    let removed = idea(&idea_uuid(&store, deleted.id).await);
    assert!(removed.deleted.is_some(), "gelöschter Einfall mit Zeitpunkt");

    // ---- Faden ----
    assert_eq!(dump.threads.len(), 1, "ein Faden");
    let posts: Vec<(u32, &str)> = dump.threads[0].posts.iter().map(|p| (p.position, p.author.as_str())).collect();
    assert_eq!(posts, vec![(1, "@lektor"), (2, "Hanna Probe")], "Fadenbeiträge in Reihenfolge ab 1");
    assert_eq!(dump.threads[0].posts[1].sources, vec!["Notizbuch".to_owned()], "Quellen im Faden");

    // ---- Bilder: Fassungen, Bytes, Pfade, basis, Umschlag und Autorenfoto ----
    let names: Vec<&str> = dump.assets.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["krug.png", "henkel.svg"], "Bilder nach Anlage");
    let png = &dump.assets[0];
    assert_eq!(png.revisions[0].data, PNG_GREEN, "Fassung 1: Rohbytes aus Base64");
    assert_eq!(png.revisions[1].data, PNG_RED, "Fassung 2: Rohbytes");
    assert_eq!(png.revisions[1].based_on, Some(png.revisions[0].uuid.clone()), "basis der Bildfassung über die UUID");
    assert_eq!(png.revisions[1].status, ExchangeAssetStatus::Current, "die ersetzende Fassung ist aktuell");
    assert_eq!(png.revisions[1].sha256, sha256_hex(PNG_RED), "sha256 über die Bytes");
    let svg = &dump.assets[1];
    assert_eq!(svg.revisions[1].data, SVG_TWO.as_bytes(), "SVG als UTF-8-Bytes");
    assert_eq!(svg.paths, vec!["abb/henkel.svg".to_owned(), "bilder/henkel.svg".to_owned()], "Pfade, sortiert");
    let book = dump.book.as_ref().expect("Buchangaben");
    assert_eq!(book.cover.as_deref(), Some(png.uuid.as_str()), "Umschlag = UUID des Bilds");
    assert_eq!(book.author_photo.as_deref(), Some(svg.uuid.as_str()), "Autorenfoto = UUID des Bilds");
    assert_eq!((book.publisher.as_str(), book.dedication.as_str()), ("Probeverlag", "Für die Töpfer."), "Buchangaben");

    // ---- Die Ausgabe lässt sich schreiben (Inline und ZIP) ----
    let document = ExchangeDocument {
        version: FORMAT_VERSION,
        exported: "2026-11-08T12:00:00.000Z".into(),
        instance: ExchangeInstance { uuid: store.instance().await.unwrap().uuid, name: String::new() },
        generator: "test".into(),
        work: dump.clone(),
        unknown: Vec::new(),
    };
    baruch_exchange::write::write_xml(&document, baruch_exchange::write::ImageMode::Inline).unwrap();
    baruch_exchange::write::write_baruch(&document).unwrap();

    // ---- Lesend: Der Dump ändert nichts und ist wiederholbar ----
    assert_eq!(store.exchange_dump(work).await.unwrap(), dump, "zweimal derselbe Stand");
    let uniq: HashSet<&String> = seen.values().flatten().collect();
    assert_eq!(uniq.len(), seen.values().map(Vec::len).sum::<usize>(), "keine UUID über Arten hinweg doppelt");
}

#[tokio::test]
async fn unknown_work_is_refused_honestly() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    // Unbekanntes Werk → UnknownWork (RPC macht daraus einen Parameterfehler), kein leerer Dump.
    assert!(matches!(store.exchange_dump(9999).await, Err(StoreError::UnknownWork(9999))), "UnknownWork");
}

#[tokio::test]
async fn plain_work_dumps_without_optional_parts() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let imported = store.import_typst("= Nur ein Kapitel\nText.\n", "kurz.typ", "Hanna Probe").await.unwrap();
    let dump = store.exchange_dump(imported.work).await.unwrap();
    // Ohne Zeilen in work_book, work_timing, work_bible, work_sword_source bleiben die Teile leer – so weiß das
    // Einlesen, dass hier „nichts“ steht (F6 g: fehlende Zeile wird angelegt).
    assert_eq!(dump.book, None, "keine Buchangaben");
    assert_eq!(dump.timing, None, "keine Sprechdauer");
    assert_eq!(dump.bible_module, None, "kein Bibelmodul");
    assert!(dump.sword_sources.is_empty(), "keine SWORD-Quellen");
    assert!(dump.ideas.is_empty() && dump.threads.is_empty() && dump.assets.is_empty(), "nichts weiter");
    assert_eq!(dump.outline.len(), 1, "ein Kapitel");
    assert_eq!(dump.outline[0].kind, ExchangeNodeKind::Chapter, "Kapitel auf oberster Ebene");
    assert_eq!(dump.sheet, None, "ohne work_sheet kein Satzbild (Runde 27)");
}

/// Runde 27 (Unterversion 3): Blöcke, Anschriften und Satzbild in der Ausgabe; Blöcke ohne Zeile in `work_book`
/// bekommen die Vorgaben der Buchangaben, damit `<umschlag>` ein `<buch>` hat.
#[tokio::test]
async fn cover_blocks_addresses_and_sheet_are_dumped_with_their_uuids() {
    use baruch_typst::cover_layout::{Backdrop, CoverAlign, FontStyle, SpineDirection};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    let work = store.import_typst("= Nur ein Kapitel\nText.\n", "kurz.typ", "Hanna Probe").await.unwrap().work;
    let spine = baruch_store::CoverBlock {
        kind: baruch_store::CoverBlockKind::SpineAuthor,
        x_mm: 1.5,
        y_mm: 120.0,
        width_mm: 60.0,
        align: CoverAlign::Right,
        size_pt: 9.0,
        leading_em: 0.5,
        color: "#336699".into(),
        font: Some("Libertinus Sans".into()),
        style: FontStyle::Italic,
        backdrop: Backdrop::Gradient,
        visible: false,
        direction: Some(SpineDirection::Up),
        text: None,
    };
    store.set_cover_blocks(work, &[spine]).await.unwrap();
    let dump = store.exchange_dump(work).await.unwrap();
    let book = dump.book.expect("Blöcke ohne work_book: Buchangaben mit Vorgaben");
    assert_eq!(
        (book.title_area, book.cover_pages, book.paper_mm, book.bleed_mm),
        (ExchangeTitleArea::Top, true, 0.1, 3.0),
        "Vorgaben von BookMeta::default()"
    );
    let uuid: String = sqlx::query_scalar("SELECT uuid FROM cover_block WHERE work_id = ?")
        .bind(work)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let block = &book.cover_blocks[0];
    assert_eq!(block.uuid, uuid, "Block mit seiner uuid");
    assert_eq!(block.kind, ExchangeCoverBlockKind::SpineAuthor, "Art");
    assert_eq!(
        (block.color.as_str(), block.direction, block.style, block.backdrop, block.visible),
        ("#336699", Some(ExchangeSpineDirection::Up), ExchangeFontStyle::Italic, ExchangeBackdrop::Gradient, false),
        "alle Werte"
    );
    assert_eq!(
        (block.x_mm, block.align, block.font.as_deref()),
        (1.5, ExchangeCoverAlign::Right, Some("Libertinus Sans"))
    );
    // Anschriften und Satzbild.
    let edit = BookEdit { publisher_address: Some("Verlag Probe\nWeg 1".into()), ..BookEdit::default() };
    store.set_book_meta(work, &edit).await.unwrap();
    let sheet = baruch_store::SheetSettings { width: Some("16cm".into()), ..Default::default() };
    store.set_sheet_settings(work, &sheet).await.unwrap();
    let dump = store.exchange_dump(work).await.unwrap();
    assert_eq!(dump.book.unwrap().publisher_address, "Verlag Probe\nWeg 1", "Anschrift in der Ausgabe");
    assert_eq!(dump.sheet.and_then(|s| s.width).as_deref(), Some("16cm"), "Satzbild in der Ausgabe");
    // Grabstein nach dem Zurücksetzen.
    store.reset_cover_blocks(work, &[], None).await.unwrap();
    let dump = store.exchange_dump(work).await.unwrap();
    assert!(
        dump.deleted.iter().any(|t| t.uuid == uuid && t.kind == ExchangeTombstoneKind::CoverBlock),
        "Grabstein umschlag-block in der Ausgabe"
    );
}
