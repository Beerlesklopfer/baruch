// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 31 – Abnahme des Integrators (Vertrag §6) über alle drei Aufgaben (glossar-daten, register-satz,
//! kontextmenue) an einem frei erfundenen Werk „Die Wärterin vom Kap“ mit zwei Teilen, drei Kapiteln, Bibelstellen
//! und Personen:
//! - Markierung → „In Glossar aufnehmen“ ohne JavaScript (Seite „Markierung“, Formular `/glossar/act`) und mit
//!   JavaScript (RPC `glossary.add`, wie der Dialog des Kontextmenüs) → Begriff auf der Glossar-Seite, Glossarseite im
//!   PDF (pdftotext) und Glossar im EPUB (`glossterm`/`glossdef`);
//! - „In Register aufnehmen“ ohne JavaScript (Formular `/register/act`) und über RPC `index.mark` mit Unterstichwort und
//!   „siehe auch“ → Registerseiten im PDF mit gemessenen Seitenzahlen und zusammengefassten Bereichen,
//!   Personenregister, Bibelstellenregister automatisch in biblischer Reihenfolge, erste Erwähnung je Kapitel;
//! - der Typst-Export setzt eigenständig dieselben Register (`typst compile`, sonst eingebetteter Compiler);
//! - Persona schlägt Begriffe vor, MCP schlägt Begriffe und Registereinträge vor, nur der Mensch übernimmt (Formulare);
//! - Rundreise in Unterversion 7 zwischen zwei Wegwerf-Instanzen (gleicher Export samt Buchende);
//! - ohne R31-Einstellungen byte-gleicher Export und gleiches EPUB; Import eines Exports mit Buchende;
//! - Tastatur (Menütaste, Umschalt+F10) und ARIA im ausgelieferten Kontextmenü, Link „Markierung …“ ohne JavaScript.
//!
//! Kein Netz, kein Eliezer (Modell nur `ScriptedModel`). PDFs unter `common::PDF_SERIAL`, Modell unter
//! `common::model_queue`. Jede Zusicherung mit Begründung.

use crate::common;

use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_typst::index::BACK_MATTER_MARK;
use baruch_typst::typeset::{
    ChapterStart, GlossaryPosition, IndexPages, PageNumberPosition, PageNumberStart, PageNumbers, TypesetSettings,
};
use serde_json::{Value, json};

/// Ein langer Absatz für das zweite Kapitel; „Nebel“ steht nur hier (für den Seitenbereich im Register).
const FOG: &str = "Der Nebel liegt schwer über der Bucht, und die Wärterin zählt die Stunden bis zum Morgen, während \
                   unten die Brandung gegen die Klippen schlägt und die Möwen schweigen.";

/// Das erfundene Werk: zwei Teile, drei Kapitel, Bibelstellen im Fließtext (eine nur im Kommentar), zwei Personen.
fn book() -> String {
    let fog = vec![FOG; 70].join("\n\n");
    format!(
        "// Die Wärterin vom Kap – frei erfunden für die Abnahme der Runde 31.\n\
         #set document(title: \"Die Wärterin vom Kap\", author: \"Lene Sand\")\n\
         #set text(lang: \"de\")\n\n\
         #heading(level: 1, numbering: none)[Am Wasser] <baruch-part>\n\n\
         = Die Ankunft\n\n\
         Hanna Berg kommt im Herbst zum Leuchtturm. Das Leuchtfeuer brennt jede Nacht. \
         Wie Ps 107,23 sagt, fahren manche mit Schiffen auf dem Meer.\n\n\
         = Der Sturm\n\n\
         {fog}\n\n\
         Das Leuchtfeuer flackert im Wind, und im Logbuch steht Joh 1,5.\n\n\
         // Röm 3,23 nur im Kommentar\n\n\
         #heading(level: 1, numbering: none)[Im Hafen] <baruch-part>\n\n\
         = Die Rückkehr\n\n\
         Matthias Kehr bringt Brot. Das Leuchtfeuer ruht am Tag. Nach 1. Mose 1,3 wurde es Licht, und Röm 8,28 \
         tröstet.\n"
    )
}

struct Cape {
    server: common::Server,
    work: i64,
    slug: String,
}

async fn cape(server: common::Server) -> Cape {
    let report = server.import(&book(), "waerterin.typ").await;
    Cape { server, work: report.work, slug: report.slug }
}

impl Cape {
    async fn section(&self, title: &str) -> (i64, u32, String) {
        let work = self.server.state.store.load_work(self.work).await.unwrap();
        let node = work.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("Abschnitt {title}"));
        let rev = node.current_number().expect("aktuelle Fassung");
        (node.key, rev, node.revision(rev).unwrap().body.clone())
    }

    async fn rpc(&self, method: &str, mut params: Value) -> Value {
        params["work"] = json!(self.work);
        let answer = self.server.rpc(method, params).await;
        assert!(answer.get("error").is_none(), "{method}: unerwarteter Fehler: {answer}");
        answer["result"].clone()
    }

    async fn post(&self, uri: &str, form: &str) -> (StatusCode, Option<String>, String) {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_owned()))
            .unwrap();
        let (status, headers, body) = self.server.send(request).await;
        (status, headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()), body)
    }

    fn last_commit(&self) -> String {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "-1", "--format=%an%n%s%n%b"]).unwrap_or_default()
    }

    async fn export(&self) -> String {
        self.rpc("work.export", json!({})).await.get("typst").and_then(Value::as_str).expect("Typst-Export").to_owned()
    }

    async fn epub(&self) -> Vec<u8> {
        let answer = self.rpc("work.epub", json!({ "inline": true })).await;
        baruch_store::decode_base64(answer["epub_base64"].as_str().expect("EPUB inline")).unwrap()
    }
}

/// Alle R31-Einstellungen eingeschaltet, Kapitel auf neuer Seite, Seitenzahlen ab der ersten Seite.
fn r31_settings() -> TypesetSettings {
    TypesetSettings {
        chapter_start: Some(ChapterStart::NewPage),
        page_numbers: Some(PageNumbers {
            position: PageNumberPosition::BottomCenter,
            start: PageNumberStart::FirstPage,
        }),
        index_pages: Some(IndexPages {
            registers: vec!["stichwort".into(), "personen".into(), "bibel".into()],
            columns: 1,
        }),
        glossary_page: Some(GlossaryPosition::AfterIndexes),
        glossary_first_mention: Some(true),
        ..TypesetSettings::default()
    }
}

/// Wert für `application/x-www-form-urlencoded` (alles außer Buchstaben/Ziffern als %XX).
fn form_escape(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Text des PDFs je Seite (pdftotext); `None`, wenn pdftotext fehlt.
fn pdf_pages(pdf: &[u8]) -> Option<Vec<String>> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("buch.pdf");
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext").arg(&path).arg("-").output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout).replace('\u{a0}', " ").split('\u{c}').map(str::to_owned).collect()
    })
}

fn zip_entries(bytes: &[u8]) -> Vec<(String, String)> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).expect("gültiges ZIP");
    (0..archive.len())
        .map(|i| {
            let mut file = archive.by_index(i).unwrap();
            let mut data = Vec::new();
            file.read_to_end(&mut data).unwrap();
            (file.name().unwrap().into_owned(), String::from_utf8_lossy(&data).into_owned())
        })
        .collect()
}

fn zip_entry<'a>(files: &'a [(String, String)], name: &str) -> &'a str {
    &files
        .iter()
        .find(|f| f.0 == name)
        .unwrap_or_else(|| panic!("{name} fehlt im EPUB: {:?}", files.iter().map(|f| &f.0).collect::<Vec<_>>()))
        .1
}

/// Glossar- und Registerdaten so anlegen, wie die Oberfläche sie schickt: ohne JavaScript über die Formulare, mit
/// JavaScript über dieselben RPC-Methoden wie Dialoge des Kontextmenüs.
async fn fill(c: &Cape) {
    let w = c.work;
    let (arrival, rev, _) = c.section("Die Ankunft").await;

    // Ohne JavaScript: Link „Markierung …“ → Seite mit den drei Aktionen → Formular „In Glossar aufnehmen“.
    let (status, page) = c.server.get(&format!("/werke/{w}/markierung?key={arrival}&rev={rev}&wort=Leuchtfeuer")).await;
    assert_eq!(status, StatusCode::OK, "Seite „Markierung“ ohne JavaScript: {page}");
    assert!(page.contains(&format!("action=\"/werke/{w}/glossar/act\"")), "Formular „In Glossar aufnehmen“: {page}");
    assert!(page.contains(&format!("action=\"/werke/{w}/register/act\"")), "Formular „In Register aufnehmen“");
    let form = format!(
        "action=add&term=Leuchtfeuer&short={}&long={}&variants=Leuchtfeuers&bible_refs={}&back={}",
        form_escape("Licht, das Schiffen nachts den Weg weist."),
        form_escape("Ein Feuer auf einem Turm an der Küste."),
        form_escape("Psalm 107,23"),
        form_escape(&format!("/werke/{w}/markierung?key={arrival}")),
    );
    let (status, location, page) = c.post(&format!("/werke/{w}/glossar/act"), &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Begriff angelegt → 303: {page}");
    let location = location.expect("Weiterleitung");
    assert!(location.contains("ok=add#begriff-"), "zurück an die Markierung mit Hinweis und Anker: {location}");
    assert!(c.last_commit().contains("Baruch-Action: glossary-add"), "ein Spiegel-Commit je Änderung");

    // Mit JavaScript: der Dialog „In Glossar aufnehmen“ ruft glossary.add.
    let logbook = c.rpc("glossary.add", json!({ "term": "Logbuch", "short": "Tagebuch der Wache." })).await;
    assert_eq!(logbook["term"]["status"], "accepted", "vom Menschen angelegt = angenommen: {logbook}");

    // Glossar-Seite des Werks zeigt beide Begriffe mit Anker; die Bibelstelle ist normalisiert.
    let (status, page) = c.server.get(&format!("/werke/{w}/glossar")).await;
    assert_eq!(status, StatusCode::OK, "Glossar-Seite");
    for needle in ["Leuchtfeuer", "Logbuch", "Ps 107,23", "id=\"begriff-"] {
        assert!(page.contains(needle), "Glossar-Seite enthält {needle}: {page}");
    }
    let lookup = c.rpc("glossary.lookup", json!({ "word": "Leuchtfeuers" })).await;
    assert_eq!(lookup["terms"][0]["term"], "Leuchtfeuer", "Nachschlagen über die Variante: {lookup}");

    // Register ohne JavaScript: alle Fundstellen von „Nebel“ im Sturm-Kapitel.
    let (storm, rev, _) = c.section("Der Sturm").await;
    let (status, location, page) = c
        .post(
            &format!("/werke/{w}/register/act"),
            &format!("action=mark&key={storm}&rev={rev}&word=Nebel&occurrence=all"),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Marken gesetzt → 303: {page}");
    assert!(location.unwrap().contains("ok=mark"), "Hinweis nach Erfolg");
    let (_, _, body) = c.section("Der Sturm").await;
    assert_eq!(body.matches("<baruch-idx>").count(), 70, "jede Fundstelle von „Nebel“ markiert (neue Fassung)");

    // Mit Unterstichwort und „siehe auch“ (RPC index.mark wie der Dialog „In Register aufnehmen“ beim Speichern).
    let (arrival, rev, _) = c.section("Die Ankunft").await;
    let marked = c
        .rpc(
            "index.mark",
            json!({ "key": arrival, "rev": rev, "word": "Leuchtturm", "term": "Küste", "sub": "Leuchtturm",
                    "also": "Nebel" }),
        )
        .await;
    assert_eq!(marked["marks"], 1, "eine Marke: {marked}");
    let rev = marked["rev"].as_u64().unwrap() as u32;
    // Personenregister.
    c.rpc(
        "index.mark",
        json!({ "key": arrival, "rev": rev, "word": "Berg", "register": "personen", "term": "Berg, Hanna" }),
    )
    .await;
    let (back, rev, _) = c.section("Die Rückkehr").await;
    c.rpc(
        "index.mark",
        json!({ "key": back, "rev": rev, "word": "Kehr", "register": "personen", "term": "Kehr, Matthias" }),
    )
    .await;
    let work = c.server.state.store.load_work(w).await.unwrap();
    for node in &work.nodes {
        let currents = node.revisions.iter().filter(|r| r.status == baruch_core::RevisionStatus::Current).count();
        assert!(currents <= 1, "je Abschnitt höchstens eine aktuelle Fassung: {}", node.title);
        if let Some(rev) = node.current_number() {
            assert!(
                matches!(node.revision(rev).unwrap().author, baruch_core::Author::Human { .. }),
                "Marken setzt nur der Mensch: {}",
                node.title
            );
        }
    }
}

#[tokio::test]
async fn glossary_and_registers_reach_the_pdf_the_epub_and_the_standalone_export() {
    let c = cape(common::open().await).await;
    fill(&c).await;
    c.server.state.store.set_work_typeset(c.work, &r31_settings(), "Lene").await.expect("Satz-Einstellungen");

    // Export: ein Buchende, Karte byte-gleich, erste Erwähnung je Kapitel als Link auf die Glossarseite.
    let typst = c.export().await;
    assert_eq!(typst.matches(BACK_MATTER_MARK).count(), 1, "genau ein Buchende im Export");
    let map = c.server.state.store.export_map(c.work).await.unwrap();
    assert_eq!(map.typst, typst, "Export-Karte = Export, auch mit Buchende");
    let term = c.server.state.store.glossary(c.work, None).await.unwrap();
    let fire = term.iter().find(|t| t.term == "Leuchtfeuer").expect("Leuchtfeuer im Glossar");
    let link = format!("#link(<baruch-gl-{}>)[#emph[Leuchtfeuer]]", fire.uuid);
    assert_eq!(typst.matches(&link).count(), 3, "erste Erwähnung je Kapitel (drei Kapitel) verlinkt: {typst}");
    assert!(typst.contains(&format!("<baruch-gl-{}>", fire.uuid)), "Label auf der Glossarseite");
    let stored = c.section("Die Ankunft").await.2;
    assert!(!stored.contains("#emph[Leuchtfeuer]"), "SQLite-Text bleibt unverändert (nur Projektion)");

    let _serial = common::PDF_SERIAL.lock().await;
    let pdf = baruch_server::pdf::book_pdf(&c.server.state, c.work).await.expect("Buch-PDF entsteht");
    match pdf_pages(&pdf.bytes) {
        Some(pages) => {
            let index = pages.iter().position(|p| p.contains("Stichwortregister")).expect("Registerseite im PDF");
            let body = &pages[..index];
            let page_of = |needle: &str| {
                body.iter().position(|p| p.contains(needle)).map(|i| i + 1).unwrap_or_else(|| panic!("{needle}"))
            };
            let fog: Vec<usize> =
                body.iter().enumerate().filter(|(_, p)| p.contains("Nebel")).map(|(i, _)| i + 1).collect();
            assert!(fog.len() >= 2, "das Sturm-Kapitel reicht über mehrere Seiten: {fog:?}");
            assert!(fog.windows(2).all(|w| w[1] == w[0] + 1), "Folgeseiten: {fog:?}");
            let (first, last) = (fog[0], *fog.last().unwrap());
            let (arrival, back) = (page_of("Hanna Berg"), page_of("Matthias Kehr"));
            assert!(arrival < first && last < back, "Kapitel auf eigenen Seiten: {arrival} {first}–{last} {back}");

            let text = pages[index..].join("\n");
            let lines: Vec<&str> = text.lines().map(str::trim).collect();
            let at = |l: &str| lines.iter().position(|x| *x == l).unwrap_or_else(|| panic!("Zeile „{l}“: {text}"));
            assert!(lines.contains(&format!("Nebel {first}–{last}").as_str()), "Bereich zusammengefasst: {text}");
            // „siehe auch“ gehört zur Marke mit dem Unterstichwort und steht hinter dessen Seitenzahl.
            assert!(
                at("Küste") < at(&format!("Leuchtturm {arrival}; siehe auch Nebel")),
                "Unterstichwort unter dem Stichwort, mit Seite und „siehe auch“: {text}"
            );
            assert!(at("Personenregister") > at("Stichwortregister"), "Register in der Reihenfolge der Einstellung");
            assert!(lines.contains(&format!("Berg, Hanna {arrival}").as_str()), "Personenregister: {text}");
            assert!(lines.contains(&format!("Kehr, Matthias {back}").as_str()), "Personenregister: {text}");
            let bible = at("Bibelstellenregister");
            assert!(
                bible < at("1. Mose")
                    && at("1. Mose") < at("Psalmen")
                    && at("Psalmen") < at("Johannes")
                    && at("Johannes") < at("Römer"),
                "Bibelstellenregister in biblischer Reihenfolge, nicht alphabetisch: {text}"
            );
            assert!(lines.contains(&format!("107,23 {arrival}").as_str()), "Ps 107,23 mit Seite: {text}");
            assert!(lines.contains(&format!("1,5 {last}").as_str()), "Joh 1,5 auf der letzten Sturm-Seite: {text}");
            assert!(!text.contains("3,23"), "Röm 3,23 steht nur im Kommentar");
            let glossary = at("Glossar");
            assert!(glossary > bible, "Glossarseite nach den Registern (Einstellung): {text}");
            assert!(
                text.contains("Leuchtfeuer (Leuchtfeuers) – Licht, das Schiffen nachts den Weg weist."),
                "Glossar: Begriff, Variante, kurze Erklärung: {text}"
            );
            assert!(text.contains("Ein Feuer auf einem Turm an der Küste."), "ausführliche Erklärung: {text}");
            assert!(text.contains("Bibelstellen: Ps 107,23"), "Bibelstellen im Glossar: {text}");
        }
        None => eprintln!("ÜBERSPRUNGEN: pdftotext fehlt (Seitenzahlen im PDF nicht gemessen)"),
    }
    drop(_serial);

    // EPUB: Glossar als glossterm/glossdef, Registerdateien, erste Erwähnung als glossref.
    let files = zip_entries(&c.epub().await);
    let glossary = zip_entry(&files, "OEBPS/text/glossary.xhtml");
    assert!(glossary.contains("epub:type=\"glossary\""), "Glossar im EPUB: {glossary}");
    assert!(glossary.contains("glossterm") && glossary.contains("Leuchtfeuer"), "glossterm Leuchtfeuer: {glossary}");
    assert!(glossary.contains("<dd epub:type=\"glossdef\">"), "glossdef: {glossary}");
    let persons = zip_entry(&files, "OEBPS/text/index-personen.xhtml");
    assert!(persons.contains("Berg, Hanna") && persons.contains("Kehr, Matthias"), "Personenregister: {persons}");
    let bible = zip_entry(&files, "OEBPS/text/index-bibel.xhtml");
    let pos = |s: &str| bible.find(s).unwrap_or_else(|| panic!("{s}: {bible}"));
    assert!(pos("1. Mose") < pos("Psalmen") && pos("Psalmen") < pos("Johannes"), "biblische Reihenfolge im EPUB");
    assert!(
        files.iter().any(|(name, text)| name.starts_with("OEBPS/text/") && text.contains("glossref")),
        "erste Erwähnung als glossref-Link im Kapitel"
    );

    // Der Typst-Export setzt eigenständig (ohne Paket, ohne Netz) dieselben Register.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("waerterin.typ");
    std::fs::write(&file, &typst).unwrap();
    match std::process::Command::new("typst").arg("compile").arg(&file).arg(dir.path().join("w.pdf")).output() {
        Ok(out) => {
            assert!(out.status.success(), "typst compile setzt den Export: {}", String::from_utf8_lossy(&out.stderr));
            if let Some(pages) = pdf_pages(&std::fs::read(dir.path().join("w.pdf")).unwrap()) {
                let text = pages.join("\n");
                for needle in ["Stichwortregister", "Personenregister", "Bibelstellenregister", "Glossar"] {
                    assert!(text.contains(needle), "{needle} im eigenständigen Export");
                }
            }
        }
        Err(_) => {
            eprintln!("ÜBERSPRUNGEN: typst nicht im PATH – der Export wird mit dem eingebetteten Compiler gesetzt")
        }
    }
    let source = baruch_render::Source { main: typst, files: Vec::new() };
    let texts = tokio::task::spawn_blocking(move || baruch_render::page_texts(&source)).await.unwrap();
    let words: Vec<String> =
        texts.expect("Export setzt eigenständig").iter().flatten().map(|t| t.text.clone()).collect();
    for needle in ["Stichwortregister", "Personenregister", "Bibelstellenregister", "Glossar", "Kehr, Matthias"] {
        assert!(words.iter().any(|w| w.contains(needle)), "{needle} im eigenständig gesetzten Export");
    }
}

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40031))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

#[tokio::test]
async fn personas_and_mcp_only_propose_and_the_human_accepts_without_javascript() {
    let _turn = common::model_queue(module_path!()).await;
    let model = ScriptedModel::with([Step::words(
        "- Brandung | Wellen, die sich an der Küste brechen.\n- Leuchtfeuer | schon da\n- Klippe | steiler Fels",
    )]);
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .expect("Datenordner");
    let state = Arc::new(state);
    let c = cape(common::Server { app: baruch_server::router(state.clone()), state, data }).await;
    let w = c.work;
    c.rpc("glossary.add", json!({ "term": "Leuchtfeuer" })).await;

    // Die Persona (Lektor nach Einstellung) schlägt vor; nichts ist angenommen.
    let (storm, _, _) = c.section("Der Sturm").await;
    let suggested = c.rpc("glossary.suggest", json!({ "key": storm })).await;
    let proposed: Vec<&str> =
        suggested["proposed"].as_array().unwrap().iter().map(|t| t["term"].as_str().unwrap()).collect();
    assert_eq!(proposed, ["Brandung", "Klippe"], "nur Vorschläge, der vorhandene Begriff übersprungen: {suggested}");
    assert_eq!(model.calls().len(), 1, "genau ein Modellaufruf über den Nachbau");
    let accepted = || async {
        c.server.state.store.glossary(w, Some(baruch_store::GlossaryStatus::Accepted)).await.unwrap().len()
    };
    assert_eq!(accepted().await, 1, "die Persona nimmt nichts an");

    // Der Mensch übernimmt auf der Glossar-Seite ohne JavaScript.
    let brandung = suggested["proposed"][0]["id"].as_i64().unwrap();
    let (_, page) = c.server.get(&format!("/werke/{w}/glossar")).await;
    assert!(page.contains(&format!("id=\"begriff-{brandung}\"")), "Vorschlag mit Anker auf der Glossar-Seite: {page}");
    let (status, location, page) =
        c.post(&format!("/werke/{w}/glossar/act"), &format!("action=accept&id={brandung}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "übernommen → 303: {page}");
    assert!(location.unwrap().contains("ok=accept"), "Hinweis nach Übernahme");
    assert_eq!(accepted().await, 2, "Brandung jetzt angenommen");
    let log = c.last_commit();
    assert!(log.contains("Baruch-Action: glossary-accept") && log.contains("Baruch-Proposer: @lektor"), "{log}");

    // MCP schlägt einen Registereintrag vor; der Text ändert sich erst, wenn der Mensch übernimmt.
    let (headers, init) = mcp(
        &c.server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Abnahme31", "version": "1.0" } } }),
    )
    .await;
    assert!(init["result"].is_object(), "initialize: {init}");
    let session = headers.get("mcp-session-id").expect("Sitzung").to_str().unwrap().to_owned();
    let (arrival, rev, before) = c.section("Die Ankunft").await;
    let (_, answer) = mcp(
        &c.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "index_propose",
                "arguments": { "work": w, "key": arrival, "word": "Herbst", "term": "Jahreszeiten", "sub": "Herbst",
                               "reason": "Zeitangabe" } } }),
    )
    .await;
    assert_eq!(answer["result"]["isError"], false, "index_propose gelingt: {answer}");
    let proposal = answer["result"]["structuredContent"]["id"].as_i64().unwrap();
    assert_eq!(c.section("Die Ankunft").await.1, rev, "MCP ändert nie den Text");
    let (_, page) = c.server.get(&format!("/werke/{w}/register")).await;
    assert!(page.contains(&format!("id=\"vorschlag-{proposal}\"")), "Vorschlag auf der Registerseite: {page}");
    let (status, _, page) = c.post(&format!("/werke/{w}/register/act"), &format!("action=accept&id={proposal}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Registervorschlag übernommen → 303: {page}");
    let (_, new_rev, after) = c.section("Die Ankunft").await;
    assert!(new_rev > rev, "neue Fassung durch die Übernahme");
    let mark = r#"#metadata((register: "stichwort", term: "Jahreszeiten", sub: "Herbst"))<baruch-idx>"#;
    assert_eq!(after, before.replacen("Herbst", &format!("Herbst{mark}"), 1), "Marke direkt hinter dem Wort");
    let log = c.last_commit();
    assert!(log.contains("Baruch-Action: index-accept") && log.contains("Baruch-Proposer: @mcp:abnahme31"), "{log}");
}

#[tokio::test]
async fn two_instances_exchange_the_work_and_export_the_same_registers() {
    let a = cape(common::open().await).await;
    fill(&a).await;
    a.rpc("index.register_add", json!({ "slug": "orte", "title": "Ortsregister" })).await;
    let gone = a.rpc("glossary.add", json!({ "term": "Möwe" })).await["term"]["id"].clone();
    a.rpc("glossary.delete", json!({ "id": gone, "confirm": true })).await;
    let mut settings = r31_settings();
    settings.index_pages.as_mut().unwrap().registers.push("orte".into());
    a.server.state.store.set_work_typeset(a.work, &settings, "Lene").await.unwrap();
    let xml = a.rpc("work.exchange_export", json!({ "format": "xml" })).await["xml"].as_str().unwrap().to_owned();
    // Runde 22 (Vertrag §6.5, spiegel-entwurf): Der Schreiber schreibt jetzt Unterversion 8.
    assert!(xml.contains("version=\"8\"") && xml.contains("<glossar>"), "Unterversion 8 (seit Runde 22) mit Glossar");

    let b = common::open().await;
    let imported = b
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": "new" }),
        )
        .await;
    let work_b = imported["result"]["work"].as_i64().unwrap_or_else(|| panic!("Import in B: {imported}"));
    let export_a = a.export().await;
    let export_b =
        b.rpc("work.export", json!({ "work": work_b })).await["result"]["typst"].as_str().unwrap().to_owned();
    assert!(export_a.contains("Ortsregister"), "eigenes Register im Buchende");
    assert_eq!(export_b, export_a, "B setzt dasselbe Buch samt Registern und Glossar (gleiche Kennungen)");
    let graves: i64 = sqlx::query_scalar("SELECT count(*) FROM uuid_tombstone WHERE kind = 'glossary_term'")
        .fetch_one(b.state.store.pool())
        .await
        .unwrap();
    assert_eq!(graves, 1, "Grabstein des gelöschten Begriffs reist mit");

    // Ein zweiter Abgleich bringt nichts doppelt.
    let merged = b
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": "merge" }),
        )
        .await;
    assert_eq!(merged["result"]["added"]["glossary_terms"], 0, "nichts doppelt: {merged}");
    assert_eq!(b.state.store.glossary(work_b, None).await.unwrap().len(), 2, "Leuchtfeuer und Logbuch");
}

#[tokio::test]
async fn without_r31_settings_export_and_epub_stay_byte_identical_and_an_exported_back_matter_is_not_imported() {
    let c = cape(common::open().await).await;
    let before = c.export().await;
    assert!(
        !before.contains(BACK_MATTER_MARK) && !before.contains("<baruch-idx>"),
        "ohne Einstellungen kein R31-Zusatz"
    );
    let epub_before = c.epub().await;
    c.rpc("glossary.add", json!({ "term": "Leuchtfeuer", "short": "Licht" })).await;
    c.rpc("index.register_add", json!({ "slug": "orte", "title": "Orte" })).await;
    assert_eq!(c.export().await, before, "Glossar und Register allein ändern den Export nicht");
    assert_eq!(c.epub().await, epub_before, "… und das EPUB auch nicht");

    // Mit Einstellungen exportiert und in eine zweite Instanz importiert: das Buchende landet in keinem Abschnitt.
    c.server.state.store.set_work_typeset(c.work, &r31_settings(), "Lene").await.unwrap();
    let with_tail = c.export().await;
    assert!(with_tail.contains(BACK_MATTER_MARK), "Buchende im Export");
    assert!(
        with_tail.contains("#link(<baruch-gl-") && with_tail.contains("register: \"bibel\""),
        "der Export trägt Zusätze der Projektion (verlinkte erste Erwähnung, Bibelstellen-Marken)"
    );
    let other = common::open().await;
    let report = other.import(&with_tail, "waerterin-export.typ").await;
    let work = other.state.store.load_work(report.work).await.unwrap();
    for node in &work.nodes {
        let body = node.current_number().map(|r| node.revision(r).unwrap().body.clone()).unwrap_or_default();
        assert!(!body.contains(BACK_MATTER_MARK), "Buchende nicht im Abschnitt „{}“: {body}", node.title);
    }
    assert_eq!(work.nodes.last().unwrap().title, "Die Rückkehr", "kein zusätzlicher Abschnitt aus dem Buchende");
    // Auch die Zusätze der Projektion kommen nicht mit: der Text gleicht dem Original (ohne Kennungs-Labels).
    for (node, original) in work.nodes.iter().zip(c.server.state.store.load_work(c.work).await.unwrap().nodes.iter()) {
        let body = node.current_number().map(|r| node.revision(r).unwrap().body.clone()).unwrap_or_default();
        let original =
            original.current_number().map(|r| original.revision(r).unwrap().body.clone()).unwrap_or_default();
        assert!(!body.contains("baruch-gl-"), "kein Verweis auf ein fremdes Glossar-Label: {body}");
        assert!(!body.contains("register: \"bibel\""), "keine automatischen Bibelstellen-Marken: {body}");
        assert_eq!(body, original, "Abschnitt „{}“ wie im Ausgangswerk", node.title);
    }
}

#[tokio::test]
async fn the_editor_offers_keyboard_menu_aria_and_the_marking_link() {
    let c = cape(common::open().await).await;
    let (key, rev, _) = c.section("Die Ankunft").await;
    let (status, page) = c.server.get(&format!("/?work={}&key={key}", c.work)).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite");
    assert!(
        page.contains(&format!("href=\"/werke/{}/markierung?key={key}&amp;rev={rev}\"", c.work)),
        "ohne JavaScript: Link „Markierung …“ mit Abschnitt und Fassung"
    );
    assert!(page.contains("data-widget=\"context-menu\""), "Kontextmenü-Widget im Menüband");
    let (_, js) = c.server.get("/static/js/context_menu.js").await;
    for needle in ["\"ContextMenu\"", "\"F10\"", "\"role\", \"menu\"", "\"role\", \"menuitem\"", "\"Escape\""] {
        assert!(js.contains(needle), "Kontextmenü mit Tastatur und ARIA: {needle}");
    }
    let (_, base) = c.server.get(&format!("/werke/{}/glossar", c.work)).await;
    assert!(
        base.contains(&format!("href=\"/werke/{}/glossar\"", c.work))
            && base.contains(&format!("href=\"/werke/{}/register\"", c.work)),
        // Runde 33 (Menüband): früher Menü Werk ▾, jetzt Reiter Buch, Gruppe „Register und Glossar“ – dieselben Links.
        "Reiter Buch führt zu Glossar und Register"
    );
}
