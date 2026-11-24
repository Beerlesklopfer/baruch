// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 14 „Teile und Gliederung“, Ende-zu-Ende (Integrator, Vertrag Abschnitt 9): Teil-Markierung
//! (R14a), Teile in Gliederung, MCP und Anträgen (R14b) und Nummerierung je Ebene samt „ohne Nummer“
//! im Typst-Text (R14c) zusammen – über RPC, MCP und Formulare ohne JavaScript, mit Export, Re-Import,
//! Spiegel und Satz. Alle Werke und Texte frei erfunden.

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::{Author, NodeKind};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Werk mit Werkvorlage (Nummerierung „1.1“, kleine Seiten). Jedes Kapitel prüft im Satz selbst, wie
/// Typst zählt (`#context assert.eq`); die erwarteten Werte sind die Nummern, die Baruch zeigen soll,
/// sobald das Vorwort ohne Nummer zählt und ein Teil davor steht.
fn lighthouse(title: &str) -> String {
    format!(
        "#set document(title: \"{title}\", author: \"Hanne Erdacht\")\n#set page(width: 10cm, height: 14cm)\n\
         #set heading(numbering: \"1.1\")\n\n\
         = Vorwort\n#context assert.eq(counter(heading).get(), (0,), message: \"Vorwort zählt nicht\")\n\
         Ein Turm am Rand der Karte.\n\n\
         = Die Ankunft\n#context assert.eq(counter(heading).get(), (1,), message: \"Ankunft ist Kapitel 1\")\n\
         Der Wärter steigt aus dem Boot.\n\n\
         == Das Logbuch\n#context assert.eq(counter(heading).get(), (1, 1), message: \"Logbuch ist 1.1\")\n\
         Erste Eintragung: Wind aus Nordwest.\n\n\
         = Der Sturm\n#context assert.eq(counter(heading).get(), (2,), message: \"Sturm ist Kapitel 2\")\n\
         Die Lampe flackert, das Glas hält.\n"
    )
}

/// Baumzeilen: (Art, Ebene, Titel, Nummer, numbered) in Baumreihenfolge, direkt aus Store und SQLite.
async fn outline(server: &common::Server, work: i64) -> Vec<(NodeKind, u8, String, String, bool)> {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let flags: Vec<(i64, i64)> = sqlx::query_as("SELECT id, numbered FROM node WHERE work_id = ? ORDER BY seq")
        .bind(work)
        .fetch_all(server.state.store.pool())
        .await
        .unwrap();
    loaded
        .nodes
        .iter()
        .map(|n| {
            let mut level = u8::from(n.kind != NodeKind::Part);
            let mut parent = n.parent;
            while let Some(p) = parent {
                if loaded.nodes[p].kind != NodeKind::Part {
                    level += 1;
                }
                parent = loaded.nodes[p].parent;
            }
            let numbered = flags.iter().find(|(id, _)| *id == n.key).unwrap().1 != 0;
            (n.kind, level, n.title.clone(), n.number.clone(), numbered)
        })
        .collect()
}

async fn key_of(server: &common::Server, work: i64, title: &str) -> i64 {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("kein Punkt „{title}“")).key
}

/// Aktuelle Fassung (Text, Autor) des Punkts mit dem Schlüssel `key`.
async fn current_of(server: &common::Server, work: i64, key: i64) -> (String, Author) {
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = loaded.nodes.iter().find(|n| n.key == key).expect("Punkt mit diesem Schlüssel");
    let rev = &node.revisions[node.current_index().unwrap()];
    (rev.body.clone(), rev.author.clone())
}

fn last_commit(server: &common::Server, slug: &str) -> Option<String> {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an%n%B"])
}

// ---- MCP (wie outline_mcp.rs) -----------------------------------------------------------------------

async fn mcp_post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

async fn mcp_session(server: &common::Server) -> String {
    let (headers, answer) = mcp_post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Turmwart", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn mcp_ok(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    let answer = mcp_post(server, Some(session), message).await.1;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

// ---- Vertrag Abschnitt 9: Teil + „Vorwort“ ohne Nummer → Export → Re-Import → Satz/PDF ----------------

#[tokio::test]
async fn a_part_and_an_unnumbered_preface_survive_export_reimport_and_the_book_pdf() {
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let report = server.import(&lighthouse("Turmjahr"), "turmjahr.typ").await;
    let work = report.work;
    let preface = key_of(&server, work, "Vorwort").await;
    let arrival = key_of(&server, work, "Die Ankunft").await;
    let storm = key_of(&server, work, "Der Sturm").await;
    let preface_text = current_of(&server, work, preface).await.0;

    // Der Mensch: Teil vor das erste Kapitel (auch mit Werkvorlage), Vorwort ohne Nummer.
    let added = server
        .rpc(
            "outline.add",
            json!({ "work": work, "target_key": preface, "place": "before", "title": "Vom Ankommen", "level": 0 }),
        )
        .await;
    let added = result(&added);
    assert_eq!(added["adopted"], json!([preface, arrival, storm]), "alle drei Kapitel stehen jetzt unter dem Teil");
    let part = added["key"].as_i64().unwrap();
    // Gegenprobe: Solange das Vorwort mitzählt, scheitern die Zählprüfungen im Text – sie sind wirksam.
    let counted_preface = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    assert!(
        counted_preface.diagnostics().iter().any(|d| d.severity == "error"),
        "mit nummeriertem Vorwort meldet Typst die falsche Zählung"
    );
    result(&server.rpc("outline.set_numbered", json!({ "work": work, "key": preface, "numbered": false })).await);

    let expected = vec![
        (NodeKind::Part, 0, "Vom Ankommen".to_owned(), "I".to_owned(), true),
        (NodeKind::Chapter, 1, "Vorwort".to_owned(), String::new(), false),
        (NodeKind::Chapter, 1, "Die Ankunft".to_owned(), "1".to_owned(), true),
        (NodeKind::Section, 2, "Das Logbuch".to_owned(), "1.1".to_owned(), true),
        (NodeKind::Chapter, 1, "Der Sturm".to_owned(), "2".to_owned(), true),
    ];
    // Diese Nummern stehen auch in den `assert.eq` im Text: gleicht der Satz unten fehlerfrei, zählt Typst
    // genau wie Baruch.
    assert_eq!(outline(&server, work).await, expected, "Baum: Teil I, Vorwort ohne Nummer, Kapitel 1 und 2");

    // Export: beide Markierungen; der gespeicherte Text des Vorworts bleibt (nur projiziert).
    let exported =
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(work).await.unwrap()).into_owned();
    assert!(
        exported.contains(&baruch_typst::part_line("Vom Ankommen")),
        "Export trägt die Teil-Markierung: {exported}"
    );
    assert!(
        exported.contains(&baruch_typst::unnumbered_line(1, "Vorwort")),
        "Export trägt die Markierung „ohne Nummer“ für das Vorwort: {exported}"
    );
    assert!(!exported.contains("Überschriften ohne Nummer"), "keine Show-Regel mehr nötig");
    assert_eq!(current_of(&server, work, preface).await.0, preface_text, "die DB behält „= Vorwort“ (Vertrag §4)");

    // Re-Import: dieselbe Gliederung (Arten, Ebenen, Titel, Nummern, numbered), Export byte-gleich.
    let again_source = exported.replace("Turmjahr", "Turmjahr wieder");
    let again = server.import(&again_source, "wieder.typ").await.work;
    assert_eq!(outline(&server, again).await, expected, "Re-Import ergibt dieselbe Gliederung");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(again).await.unwrap()).into_owned(),
        again_source,
        "Export → Import → Export byte-gleich"
    );

    // Satz: die Zähler-Prüfungen im Text gelten (Typst-Nummern = Baruch-Nummern), Teil auf rechter Seite.
    let typeset = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    let errors: Vec<_> = typeset.diagnostics().into_iter().filter(|d| d.severity == "error").collect();
    assert!(errors.is_empty(), "Typst zählt wie Baruch (assert.eq im Text): {errors:?}");
    let part_page = typeset.page_of(Some(part));
    assert_eq!(part_page % 2, 1, "die Teil-Seite ist eine rechte Seite: {part_page}");
    assert!(typeset.page_of(Some(preface)) > part_page, "das Vorwort beginnt nach der Teil-Seite");
    let counted = server.state.store.counted_headings(work).await.unwrap();
    assert_eq!(
        counted,
        Some(vec![arrival, storm]),
        "counted_headings = Kapitel, die Typst zählt (ohne Teil und Vorwort): Kapitelbild und Seitenzahl folgen"
    );

    // Buch als PDF: dieselbe Quelle wie der Satz, also dieselben Seiten samt Teil-Seite.
    let pdf = server.rpc("work.pdf", json!({ "work": work })).await;
    assert_eq!(
        result(&pdf)["pages"].as_u64().unwrap() as usize,
        typeset.page_count(),
        "PDF hat die Seiten des Satzes (mit Teil-Seite)"
    );
    // PDF-Antworten sind Bytes (SKILLS.md): `oneshot` + `to_bytes` statt `Server::send`.
    let response = tower::ServiceExt::oneshot(
        server.app.clone(),
        Request::get(format!("/werke/{work}/pdf")).body(Body::empty()).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Buch-PDF über die Seite");
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/pdf", "als PDF");
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "ein PDF");
}

// ---- Assistent legt einen Teil an, der Mensch benennt ohne JavaScript um und schaltet zurück -----------

#[tokio::test]
async fn an_assistant_creates_a_part_and_renaming_or_switching_keeps_text_and_outline_together() {
    let server = common::open().await;
    // Ohne Werkvorlage (keine Präambel).
    let report = server.import("= Muschelsand\n\nAm Strand.\n\n= Treibholz\n\nAm Ufer.\n", "kueste.typ").await;
    let (work, slug) = (report.work, report.slug);
    let first = key_of(&server, work, "Muschelsand").await;
    let session = mcp_session(&server).await;
    let answer = mcp_ok(
        &server,
        &session,
        "outline_add",
        json!({ "work": work, "key": first, "place": "before", "title": "Vom Meer", "level": 0 }),
    )
    .await;
    // Nutzerfrage 12 (2026-11-07): Der Teil übernähme beide Kapitel → Antrag, dem der Mensch zustimmt.
    assert_eq!(answer["action"], "requested", "Teil mit Folgekapiteln wird ein Antrag: {answer}");
    assert_eq!(answer["adopted"].as_array().unwrap().len(), 2, "beide Kapitel würden wandern: {answer}");
    let id = answer["request"]["id"].as_i64().unwrap();
    result(&server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await);
    let part = key_of(&server, work, "Vom Meer").await;
    let (body, author) = current_of(&server, work, part).await;
    assert_eq!(body, format!("{}\n", baruch_typst::part_line("Vom Meer")), "Text = Teil-Markierung + Leerzeile");
    assert!(
        matches!(&author, Author::Assistant { persona, .. } if persona == "@mcp:turmwart"),
        "Revision 1 trägt den Client, der den Teil verfasst hat (der Mensch hat zugestimmt): {author:?}"
    );
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(commit.contains("Baruch-Origin: mcp"), "Spiegel protokolliert die Herkunft: {commit}");
        assert!(commit.contains("Baruch-Confirmed-By: Autor"), "und die Zustimmung: {commit}");
    }

    // Umbenennen nur der Mensch – hier über das Formular ohne JavaScript.
    let form = format!("method=outline.rename&work={work}&key={part}&title=Von+der+See&view=gliederung");
    let (status, _) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript: 303");
    let (body, author) = current_of(&server, work, part).await;
    assert_eq!(body, format!("{}\n", baruch_typst::part_line("Von der See")), "Markierungszeile neu");
    assert_eq!(author, Author::Human { name: "Autor".into() }, "neue Fassung vom Menschen");
    assert_eq!(outline(&server, work).await[0].2, "Von der See", "Baum folgt");
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(commit.contains("Baruch-Action: outline-rename"), "Spiegel: Umbenennen protokolliert: {commit}");
    }
    let mirrored = std::fs::read_dir(server.mirror_dir_of(&slug).join("abschnitte"))
        .unwrap()
        .map(|e| {
            baruch_typst::xref::strip_key_labels(&std::fs::read_to_string(e.unwrap().path()).unwrap()).into_owned()
        })
        .find(|text| text.contains("<baruch-part>"))
        .expect("Teil-Datei im Spiegel");
    assert_eq!(mirrored, body, "Spiegel-Datei = aktuelle Fassung des Teils");

    // Zurückschalten auf Fassung 1: der Titel folgt der Markierung (Text der Fassung unverändert).
    result(&server.rpc("revision.set_current", json!({ "work": work, "key": part, "rev": 1 })).await);
    assert_eq!(outline(&server, work).await[0].2, "Vom Meer", "Umschalten benennt über die Markierung zurück");
    assert_eq!(
        current_of(&server, work, part).await.0,
        format!("{}\n", baruch_typst::part_line("Vom Meer")),
        "Fassung 1 des Teils ist wieder aktuell: ihr Text ist die Markierungszeile mit dem alten Titel"
    );
    // Export und Re-Import auch ohne Werkvorlage.
    let exported =
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(work).await.unwrap()).into_owned();
    let again = server.import(&exported, "kueste-wieder.typ").await.work;
    assert_eq!(outline(&server, again).await, outline(&server, work).await, "Re-Import ohne Vorlage gleich");
    assert_eq!(
        baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(again).await.unwrap()).into_owned(),
        exported,
        "Export byte-gleich"
    );
}

// ---- Gliederungs-Vorschlag eines Assistenten: Vorschau → Antrag → Bestätigung ------------------------

#[tokio::test]
async fn an_assistant_request_on_a_part_shows_a_preview_and_the_human_confirms_without_javascript() {
    let server = common::open().await;
    let report = server.import(&lighthouse("Turm mit Antrag"), "antrag.typ").await;
    let (work, slug) = (report.work, report.slug);
    let preface = key_of(&server, work, "Vorwort").await;
    let added = server
        .rpc(
            "outline.add",
            json!({ "work": work, "target_key": preface, "place": "before", "title": "Erster Teil", "level": 0 }),
        )
        .await;
    let part = result(&added)["key"].as_i64().unwrap();
    let session = mcp_session(&server).await;
    // Umbenennen eines Teils über MCP ist ein Antrag (AGENTS.md: nur nach Bestätigung).
    let request =
        mcp_ok(&server, &session, "outline_rename", json!({ "work": work, "key": part, "title": "Ankunft am Turm" }))
            .await;
    let id = request["request"]["id"].as_i64().expect("Antrag statt Änderung");
    assert_eq!(outline(&server, work).await[0].2, "Erster Teil", "noch nichts geändert");

    let listed = mcp_ok(&server, &session, "outline_requests", json!({ "work": work, "preview": true })).await;
    let preview = listed["requests"][0]["preview"].as_array().expect("Vorschau").clone();
    assert_eq!(preview[0]["title"], "Ankunft am Turm", "Vorschau zeigt den neuen Titel");
    assert_eq!(preview[0]["mark"], "renamed", "markiert als umbenannt");
    assert_eq!(preview[0]["level"], 0, "als Teil");
    let (status, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Gliederungsseite");
    let row = page.split(&format!(r#"id="antrag-{id}""#)).nth(1).expect("Antrag auf der Seite");
    let row = row.split("</tr>").next().unwrap();
    assert!(row.contains("<details class=\"request-preview\">"), "Vorschau ohne JavaScript");
    assert!(row.contains("Ankunft am Turm"), "neuer Titel in der Vorschau");

    let form = format!("method=outline.request_decide&work={work}&id={id}&decision=confirm");
    let (status, location) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "bestätigt ohne JavaScript");
    assert!(location.unwrap().ends_with("#antraege"), "zurück zu den Anträgen");
    let after = outline(&server, work).await;
    let shown: Vec<(String, String)> = after.iter().map(|n| (n.2.clone(), n.3.clone())).collect();
    let previewed: Vec<(String, String)> = preview
        .iter()
        .map(|p| (p["title"].as_str().unwrap().to_owned(), p["number"].as_str().unwrap().to_owned()))
        .collect();
    assert_eq!(shown, previewed, "Gliederung nach der Bestätigung = Vorschau");
    let (body, _) = current_of(&server, work, part).await;
    assert_eq!(body, format!("{}\n", baruch_typst::part_line("Ankunft am Turm")), "Teiltext folgt");
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(commit.contains(&format!("Baruch-Request: {id}")), "Spiegel nennt den Antrag: {commit}");
    }
}

// ---- idea.process atomar (Teil eines Werks mit Teilen) ---------------------------------------------------

#[tokio::test]
async fn idea_process_as_outline_adds_the_point_and_closes_the_idea_together() {
    let server = common::open().await;
    let report = server.import(&lighthouse("Turm mit Einfall"), "einfall.typ").await;
    let work = report.work;
    let storm = key_of(&server, work, "Der Sturm").await;
    let created =
        server.rpc("idea.create", json!({ "work": work, "key": storm, "words": ["Nebelhorn"], "note": "" })).await;
    let id = result(&created)["idea"]["id"].as_i64().unwrap();
    let before = outline(&server, work).await;
    // Scheitert der zweite Schritt (Einfall schließen), darf auch der Punkt nicht bleiben.
    sqlx::query("CREATE TRIGGER r14_idea_fails BEFORE UPDATE ON idea BEGIN SELECT RAISE(ABORT, 'Probe'); END")
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let failed = server.rpc("idea.process", json!({ "id": id, "as": "outline", "key": storm, "title": "Nebel" })).await;
    assert!(failed["error"].is_object(), "scheitert erzwungen: {failed}");
    sqlx::query("DROP TRIGGER r14_idea_fails").execute(server.state.store.pool()).await.unwrap();
    assert_eq!(outline(&server, work).await, before, "kein halber Zustand: Gliederung unverändert");
    let done = server.rpc("idea.process", json!({ "id": id, "as": "outline", "key": storm, "title": "Nebel" })).await;
    let done = result(&done);
    assert_eq!(done["idea"]["status"], "done", "jetzt verarbeitet");
    assert_eq!(
        done["idea"]["done_ref"],
        format!("key {}", key_of(&server, work, "Nebel").await),
        "Verweis auf den Punkt"
    );
}

// ---- Nummerierung je Ebene: Baum = Vorschau = Re-Import (Juristisch, Predigt) -------------------------

#[tokio::test]
async fn numbering_per_level_is_the_same_in_tree_preview_and_reimport() {
    let server = common::open().await;
    let report = server.import(&lighthouse("Turm gezählt"), "gezaehlt.typ").await;
    let work = report.work;
    let preface = key_of(&server, work, "Vorwort").await;
    result(
        &server
            .rpc(
                "outline.add",
                json!({ "work": work, "target_key": preface, "place": "before", "title": "Teil eins", "level": 0 }),
            )
            .await,
    );
    result(&server.rpc("outline.set_numbered", json!({ "work": work, "key": preface, "numbered": false })).await);
    for scheme in ["legal", "sermon", "book"] {
        result(&server.rpc("numbering.set", json!({ "work": work, "scheme": scheme })).await);
        let tree = outline(&server, work).await;
        let preview = server.rpc("numbering.preview", json!({ "work": work, "scheme": scheme })).await;
        let numbers: Vec<String> = result(&preview)["numbers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["number"].as_str().unwrap().to_owned())
            .collect();
        let shown: Vec<String> = tree.iter().map(|n| n.3.clone()).collect();
        assert_eq!(shown, numbers, "{scheme}: Baum und Vorschau haben dieselbe Quelle (Numbering)");
        let source = baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(work).await.unwrap())
            .into_owned()
            .replace("Turm gezählt", &format!("Turm {scheme}"));
        let again = server.import(&source, &format!("{scheme}.typ")).await.work;
        assert_eq!(
            outline(&server, again).await,
            tree,
            "{scheme}: Re-Import mit gleichen Nummern und gleichem numbered (auch Ebenen mit leerer Nummer)"
        );
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&server.state.store.export_typst(again).await.unwrap()).into_owned(),
            source,
            "{scheme}: Export byte-gleich"
        );
        let typeset = baruch_server::typeset::typeset(&server.state, again).await.unwrap();
        let errors: Vec<_> = typeset.diagnostics().into_iter().filter(|d| d.severity == "error").collect();
        // Die Prüfungen im Text lesen `counter(heading).get()` – die Zählung, nicht die Darstellung: Auch
        // unter diesem Schema zählen Teil und Vorwort (Markierungen) in Typst nicht mit.
        assert!(errors.is_empty(), "{scheme}: Re-Import setzt fehlerfrei, Zählung wie Baruch: {errors:?}");
    }
}
