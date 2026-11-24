// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 31b (register-satz): Registermarken (`index.entries/mark/unmark/overview`), Projektion (Bibelstellen) und
//! Registerseiten in Export, Spiegel, Satz und PDF. Werke frei erfunden; jede Zusicherung mit Begründung.

use crate::common;

use baruch_store::WorkId;
use baruch_typst::index::BACK_MATTER_MARK;
use baruch_typst::typeset::{
    ChapterStart, IndexPages, PageNumberPosition, PageNumberStart, PageNumbers, TypesetSettings,
};
use serde_json::{Value, json};

const BEES: &str = "// Bienenjahr für die Registertests (frei erfunden).\n\
#set document(title: \"Das Bienenjahr\", author: \"Mara Feld\")\n\
#set text(lang: \"de\")\n\n\
= Frühjahr\n\n\
Die Imkerin öffnet den Stock und zählt die Waben. Wie Joh 3,16 sagt, ist Liebe groß.\n\n\
// Röm 8,28 nur im Kommentar\n\n\
== Schwarm\n\n\
Ein Schwarm hängt am Apfelbaum. Der Schwarm summt.\n\n\
= Herbst\n\n\
Honig wird geschleudert, und Ps 23 klingt im Radio.\n";

struct Bees {
    server: common::Server,
    work: WorkId,
    slug: String,
}

async fn bees() -> Bees {
    let server = common::open().await;
    let report = server.import(BEES, "bienenjahr.typ").await;
    Bees { server, work: report.work, slug: report.slug }
}

impl Bees {
    /// Fester Schlüssel und aktuelle Fassung des Abschnitts mit diesem Titel.
    async fn section(&self, title: &str) -> (i64, u32, String) {
        let work = self.server.state.store.load_work(self.work).await.unwrap();
        let node = work.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("Abschnitt {title}"));
        let rev = node.current_number().expect("aktuelle Fassung");
        (node.key, rev, node.revision(rev).unwrap().body.clone())
    }

    async fn rpc(&self, method: &str, mut params: Value) -> Value {
        params["work"] = json!(self.work);
        self.server.rpc(method, params).await
    }

    fn commits(&self) -> usize {
        common::git(&self.server.mirror_dir_of(&self.slug), &["rev-list", "--count", "HEAD"])
            .map_or(0, |c| c.trim().parse().unwrap())
    }

    fn last_commit(&self) -> String {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "-1", "--format=%s%n%b"]).unwrap_or_default()
    }

    async fn settings(&self, settings: &TypesetSettings) {
        self.server.state.store.set_work_typeset(self.work, settings, "Mara").await.expect("Satz-Einstellungen");
    }
}

fn error(answer: &Value) -> (i64, String) {
    (
        answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}")),
        answer["error"]["message"].as_str().unwrap_or("").to_owned(),
    )
}

#[tokio::test]
async fn marking_a_word_saves_a_new_human_revision_with_one_mirror_commit() {
    let b = bees().await;
    let (key, rev, before) = b.section("Frühjahr").await;
    let commits = b.commits();
    let answer = b
        .rpc("index.mark", json!({ "key": key, "rev": rev, "word": "Stock", "term": "Bienenstock", "sub": "Waben" }))
        .await;
    let result = &answer["result"];
    assert_eq!(result["marks"], json!(1), "eine Marke gesetzt: {answer}");
    assert_eq!(result["key"], json!(key), "fester Schlüssel in der Antwort");
    assert_eq!(result["work"], json!(b.work), "Werk in der Antwort");
    let new_rev = result["rev"].as_u64().expect("neue Fassung") as u32;
    assert!(new_rev > rev, "neue Fassung auf der genannten: {rev} → {new_rev}");
    assert!(result["commit"].is_string(), "Spiegel-Commit gemeldet: {answer}");
    assert_eq!(b.commits(), commits + 1, "genau ein Commit im Spiegel");
    assert!(b.last_commit().contains(&format!("Baruch-Revision: {new_rev}")), "Commit nennt die neue Fassung");

    let (_, current, body) = b.section("Frühjahr").await;
    assert_eq!(current, new_rev, "die neue Fassung ist aktuell (Mensch)");
    let mark = r#"#metadata((register: "stichwort", term: "Bienenstock", sub: "Waben"))<baruch-idx>"#;
    assert_eq!(body, before.replacen("den Stock", &format!("den Stock{mark}"), 1), "Marke direkt hinter dem Wort");
    let work = b.server.state.store.load_work(b.work).await.unwrap();
    let node = work.nodes.iter().find(|n| n.key == key).unwrap();
    assert!(
        matches!(&node.revision(new_rev).unwrap().author, baruch_core::Author::Human { .. }),
        "die neue Fassung stammt vom Menschen"
    );
    let currents = node.revisions.iter().filter(|r| r.status == baruch_core::RevisionStatus::Current).count();
    assert_eq!(currents, 1, "genau eine aktuelle Fassung je Abschnitt");
}

#[tokio::test]
async fn entries_list_marks_with_their_span_and_unmark_removes_exactly_that_mark() {
    let b = bees().await;
    let (key, rev, before) = b.section("Schwarm").await;
    let answer = b.rpc("index.mark", json!({ "key": key, "rev": rev, "word": "Schwarm", "occurrence": "all" })).await;
    assert_eq!(answer["result"]["marks"], json!(2), "beide Fundstellen (Überschrift zählt nicht): {answer}");
    let rev = answer["result"]["rev"].as_u64().unwrap() as u32;
    let entries = b.rpc("index.entries", json!({ "key": key })).await;
    let list = entries["result"]["entries"].as_array().expect("Liste").clone();
    assert_eq!(list.len(), 2, "zwei Marken im Abschnitt: {entries}");
    let (_, _, body) = b.section("Schwarm").await;
    for e in &list {
        assert_eq!(e["term"], json!("Schwarm"), "Vorgabe: Stichwort = Wort");
        assert_eq!(e["register"], json!("stichwort"), "Vorgabe-Register");
        assert_eq!(e["auto"], json!(false), "Marke aus dem Text");
        let (s, t) = (e["start"].as_u64().unwrap() as usize, e["end"].as_u64().unwrap() as usize);
        assert!(body[s..t].starts_with("#metadata((") && body[s..t].ends_with("<baruch-idx>"), "Bereich = Marke");
    }
    let first = &list[0];
    let commits = b.commits();
    let answer =
        b.rpc("index.unmark", json!({ "key": key, "rev": rev, "start": first["start"], "end": first["end"] })).await;
    assert!(answer["result"]["commit"].is_string(), "Entfernen mit Spiegel-Commit: {answer}");
    assert_eq!(b.commits(), commits + 1, "genau ein Commit");
    let (_, _, after) = b.section("Schwarm").await;
    let left = b.rpc("index.entries", json!({ "key": key })).await;
    assert_eq!(left["result"]["entries"].as_array().unwrap().len(), 1, "eine Marke bleibt: {left}");
    let mark = r#"#metadata((register: "stichwort", term: "Schwarm"))<baruch-idx>"#;
    assert_eq!(after.matches(mark).count(), 1, "nur die gewählte Marke entfernt");
    assert_eq!(after.replace(mark, ""), before, "ohne Marken wieder der alte Text");

    let stale = b.rpc("index.unmark", json!({ "key": key, "rev": rev, "start": 0, "end": 5 })).await;
    let (code, message) = error(&stale);
    assert_eq!(code, baruch_server::rpc::INVALID_PARAMS, "kein Markenbereich: INVALID_PARAMS");
    assert!(message.contains("keine Registermarke"), "deutsche Meldung: {message}");
}

#[tokio::test]
async fn wrong_requests_are_refused_with_german_messages_and_change_nothing() {
    let b = bees().await;
    let (key, rev, _) = b.section("Frühjahr").await;
    let commits = b.commits();
    let cases = [
        (json!({ "key": key, "rev": rev }), "Wort fehlt"),
        (json!({ "key": key, "rev": rev, "word": "Frühjahr" }), "steht nicht im Abschnitt"),
        (json!({ "key": key, "rev": rev, "word": "Waben", "register": "orte" }), "Register „orte“ gibt es"),
        (json!({ "key": key, "rev": rev, "word": "Waben", "register": "Groß" }), "Register „Groß“"),
        (json!({ "key": key, "rev": rev, "word": "Waben", "occurrence": "zweite" }), "occurrence"),
        (json!({ "key": key, "rev": 99, "word": "Waben" }), "Fassung 99"),
        (json!({ "key": 999_999, "rev": rev, "word": "Waben" }), "Abschnitt 999999"),
        (json!({ "key": key, "rev": rev, "word": "Waben", "term": "x".repeat(201) }), "höchstens 200 Zeichen"),
        (json!({ "key": key, "rev": rev, "word": "Röm" }), "steht nicht im Abschnitt"),
    ];
    for (params, expected) in cases {
        let answer = b.rpc("index.mark", params.clone()).await;
        let (code, message) = error(&answer);
        assert_eq!(code, baruch_server::rpc::INVALID_PARAMS, "{params}: INVALID_PARAMS");
        assert!(message.contains(expected), "{params}: Meldung enthält „{expected}“: {message}");
    }
    assert_eq!(b.commits(), commits, "kein Commit bei abgelehnten Anfragen");
    let (_, current, _) = b.section("Frühjahr").await;
    assert_eq!(current, rev, "keine neue Fassung");
}

#[tokio::test]
async fn overview_and_entries_show_automatic_scripture_passages_only_with_the_scripture_register() {
    let b = bees().await;
    let (key, rev, _) = b.section("Herbst").await;
    b.rpc("index.mark", json!({ "key": key, "rev": rev, "word": "Honig" })).await;
    let entries = b.rpc("index.entries", json!({})).await;
    let list = entries["result"]["entries"].as_array().unwrap();
    assert!(list.iter().all(|e| e["auto"] == json!(false)), "ohne Bibelstellenregister keine automatischen: {entries}");

    b.settings(&TypesetSettings {
        index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "bibel".into()], columns: 1 }),
        ..TypesetSettings::default()
    })
    .await;
    let entries = b.rpc("index.entries", json!({})).await;
    let autos: Vec<(String, String)> = entries["result"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["auto"] == json!(true))
        .map(|e| {
            assert!(e["start"].is_null() && e["end"].is_null(), "automatische Einträge ohne Bereich: {e}");
            (e["term"].as_str().unwrap().to_owned(), e["sub"].as_str().unwrap().to_owned())
        })
        .collect();
    assert_eq!(
        autos,
        vec![("Johannes".to_owned(), "3,16".to_owned()), ("Psalmen".to_owned(), "23".to_owned())],
        "Stellen im Fließtext (Röm 8,28 im Kommentar nicht): {entries}"
    );
    let overview = b.rpc("index.overview", json!({})).await;
    let registers = overview["result"]["registers"].as_array().expect("Register");
    let slugs: Vec<&str> = registers.iter().map(|r| r["slug"].as_str().unwrap()).collect();
    assert_eq!(slugs, vec!["stichwort", "personen", "bibel"], "alle Register des Werks");
    assert_eq!(registers[0]["terms"][0]["term"], json!("Honig"), "Stichwort Honig: {overview}");
    assert_eq!(registers[0]["terms"][0]["keys"], json!([key]), "im Abschnitt Herbst");
    let bible: Vec<&str> =
        registers[2]["terms"].as_array().unwrap().iter().map(|t| t["term"].as_str().unwrap()).collect();
    assert_eq!(
        bible,
        vec!["Psalmen", "Johannes"],
        "biblische Reihenfolge (Psalmen vor Johannes, nicht alphabetisch): {overview}"
    );
    assert_eq!(registers[2]["kind"], json!("scripture"), "Art des Registers");
}

/// Text des PDFs je Seite (pdftotext, Seiten an `\x0c` getrennt); `None`, wenn pdftotext fehlt.
fn pdf_pages(pdf: &[u8]) -> Option<Vec<String>> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("buch.pdf");
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext").arg(&path).arg("-").output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout).replace('\u{a0}', " ").split('\u{c}').map(str::to_owned).collect()
    })
}

#[tokio::test]
async fn the_book_pdf_and_the_standalone_export_print_the_registers_with_measured_pages() {
    let b = bees().await;
    let (spring, rev, _) = b.section("Frühjahr").await;
    b.rpc("index.mark", json!({ "key": spring, "rev": rev, "word": "Stock", "term": "Bienenstock" })).await;
    let (autumn, rev, _) = b.section("Herbst").await;
    b.rpc("index.mark", json!({ "key": autumn, "rev": rev, "word": "Honig" })).await;
    b.settings(&TypesetSettings {
        chapter_start: Some(ChapterStart::NewPage),
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomCenter,
            start: PageNumberStart::FirstPage,
        }),
        index_pages: Some(IndexPages { registers: vec!["stichwort".into(), "bibel".into()], columns: 1 }),
        ..TypesetSettings::default()
    })
    .await;

    let export = b.rpc("work.export", json!({})).await;
    let typst = export["result"]["typst"].as_str().expect("Export").to_owned();
    assert_eq!(typst.matches(BACK_MATTER_MARK).count(), 1, "Buchende im Export");
    let map = b.server.state.store.export_map(b.work).await.unwrap();
    assert_eq!(map.typst, typst, "Export-Karte byte-gleich zum Export, auch mit Buchende");

    let _serial = common::PDF_SERIAL.lock().await;
    let pdf = baruch_server::pdf::book_pdf(&b.server.state, b.work).await.expect("Buch-PDF entsteht");
    match pdf_pages(&pdf.bytes) {
        Some(pages) => {
            let page_of = |needle: &str| {
                pages.iter().position(|p| p.contains(needle)).map(|i| i + 1).unwrap_or_else(|| panic!("{needle}"))
            };
            let (stock, honey) = (page_of("zählt die Waben"), page_of("Honig wird"));
            assert!(honey > stock, "Herbst beginnt auf einer neuen Seite: {stock} / {honey}");
            let index = pages.iter().position(|p| p.contains("Stichwortregister")).expect("Registerseite");
            let text = pages[index..].join("\n");
            let lines: Vec<&str> = text.lines().map(str::trim).collect();
            assert!(lines.contains(&format!("Bienenstock {stock}").as_str()), "Stichwort mit gemessener Seite: {text}");
            assert!(lines.contains(&format!("Honig {honey}").as_str()), "Honig mit Seite {honey}: {text}");
            let at = |l: &str| lines.iter().position(|x| *x == l).unwrap_or_else(|| panic!("{l}: {text}"));
            assert!(at("Psalmen") < at("Johannes"), "Bibelstellenregister in biblischer Reihenfolge: {text}");
            assert!(lines.contains(&format!("3,16 {stock}").as_str()), "Joh 3,16 auf Seite {stock}: {text}");
            assert!(lines.contains(&format!("23 {honey}").as_str()), "Ps 23 auf Seite {honey}: {text}");
            assert!(!text.contains("8,28"), "Röm 8,28 steht nur im Kommentar");
        }
        None => eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Seitenzahlen im PDF nicht gemessen)"),
    }

    // Der Typst-Export als eigenständige Datei ohne Paket und ohne Netz: mit `typst compile`, falls vorhanden; sonst
    // mit dem eingebetteten Compiler (dieselbe Typst-Version, keine Dateien außer main).
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bienenjahr.typ");
    std::fs::write(&file, &typst).unwrap();
    match std::process::Command::new("typst").arg("compile").arg(&file).arg(dir.path().join("b.pdf")).output() {
        Ok(out) => {
            assert!(out.status.success(), "typst compile setzt den Export: {}", String::from_utf8_lossy(&out.stderr));
            if let Some(pages) = pdf_pages(&std::fs::read(dir.path().join("b.pdf")).unwrap()) {
                let text = pages.join("\n");
                assert!(text.contains("Stichwortregister") && text.contains("Bienenstock"), "Register im Export");
            }
        }
        Err(_) => {
            eprintln!("ÜBERSPRUNGEN: typst nicht im PATH – der Export wird mit dem eingebetteten Compiler gesetzt")
        }
    }
    let source = baruch_render::Source { main: typst, files: Vec::new() };
    let texts = tokio::task::spawn_blocking(move || baruch_render::page_texts(&source)).await.unwrap();
    let texts = texts.expect("der Export setzt eigenständig ohne Fehler");
    let words: Vec<String> = texts.iter().flatten().map(|t| t.text.clone()).collect();
    assert!(words.iter().any(|w| w.contains("Stichwortregister")), "Registerseite im eigenständigen Export");
    assert!(words.iter().any(|w| w.contains("Bienenstock")), "Eintrag im eigenständigen Export");
}

#[tokio::test]
async fn a_work_without_r31_settings_exports_unchanged_even_with_marks() {
    let b = bees().await;
    let before = b.rpc("work.export", json!({})).await["result"]["typst"].as_str().unwrap().to_owned();
    assert!(!before.contains(BACK_MATTER_MARK), "ohne R31-Einstellungen kein Buchende");
    assert!(!before.contains("register: \"bibel\""), "ohne R31-Einstellungen keine automatischen Marken");
    let map = b.server.state.store.export_map(b.work).await.unwrap();
    assert_eq!(map.typst, before, "Export-Karte byte-gleich zum Export (Vertrag §2.6)");
    let (key, rev, _) = b.section("Herbst").await;
    b.rpc("index.mark", json!({ "key": key, "rev": rev, "word": "Honig" })).await;
    let after = b.rpc("work.export", json!({})).await["result"]["typst"].as_str().unwrap().to_owned();
    let mark = r#"#metadata((register: "stichwort", term: "Honig"))<baruch-idx>"#;
    assert_eq!(after, before.replacen("Honig wird", &format!("Honig{mark} wird"), 1), "nur die Marke kommt dazu");
}
