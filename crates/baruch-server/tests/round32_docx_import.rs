// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 32, task docx_import: Word import with preview (pages, RPC docx_import.*, CLI).
//!
//! The Word file „Das Haus am Deich“ is freely invented and is created in the test as an archive
//! (`crates/baruch-docx/tests/data/import/mod.rs`, the same as in the tests of the reader): three sections (heading
//! 1–3), one deletion and one insertion by „Erika Beispiel“, two insertions by „Jörg Müller“ (one of them in a
//! heading), three comments (one is a reply), one footnote, one image. No real Word, no network, no model.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::{Author, RevisionStatus};
use baruch_server::docx_import::{self, CliDocxImport, WORD_MODEL};
use serde_json::{Value, json};

use crate::common;

#[path = "../../baruch-docx/tests/data/import/mod.rs"]
pub(crate) mod docx;

const ERIKA: &str = "@word:erika-beispiel";
const JOERG: &str = "@word:joerg-mueller";

async fn ok(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_null(), "{method} sollte gelingen: {answer}");
    answer["result"].clone()
}

async fn fails(server: &common::Server, method: &str, params: Value) -> Value {
    let answer = server.rpc(method, params).await;
    assert!(answer["error"].is_object(), "{method} sollte scheitern: {answer}");
    answer["error"].clone()
}

async fn works(server: &common::Server) -> usize {
    server.state.store.work_list().await.unwrap().len()
}

fn base64(bytes: &[u8]) -> String {
    baruch_store::encode_base64(bytes)
}

async fn preview(server: &common::Server) -> Value {
    ok(server, "docx_import.preview", json!({ "file_name": "deich.docx", "data_base64": base64(&docx::sample()) }))
        .await
}

fn codes(value: &Value) -> Vec<String> {
    value["warnings"].as_array().unwrap().iter().map(|w| w["code"].as_str().unwrap().to_owned()).collect()
}

/// multipart/form-data with text fields and at most one file.
fn multipart(fields: &[(&str, &str)], file: Option<(&str, &[u8])>) -> (String, Vec<u8>) {
    let boundary = "----baruch-r32-grenze".to_owned();
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
        );
    }
    if let Some((name, bytes)) = file {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\
                 Content-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn post_multipart(
    server: &common::Server,
    uri: &str,
    fields: &[(&str, &str)],
    file: Option<(&str, &[u8])>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let (content_type, body) = multipart(fields, file);
    let request = Request::post(uri).header(header::CONTENT_TYPE, content_type).body(Body::from(body)).unwrap();
    server.send(request).await
}

fn location(headers: &axum::http::HeaderMap) -> String {
    headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()).unwrap_or_default()
}

#[tokio::test]
async fn the_preview_only_stores_a_draft_and_discard_removes_it() {
    let server = common::open().await;
    let before = works(&server).await;
    let shown = preview(&server).await;
    assert_eq!(works(&server).await, before, "die Vorschau legt kein Werk an");
    assert_eq!(docx_import::draft_count(&server.state), 1, "nur einen Entwurf unter docx-drafts");
    let id = shown["draft"].as_str().expect("Kennung des Entwurfs").to_owned();
    assert!(
        server.path().join(docx_import::DRAFT_DIR).join(&id).join("source.docx").is_file(),
        "der Entwurf liegt mit der Datei unter <data>/docx-drafts/<draft>/"
    );
    assert_eq!(shown["file_name"], "deich.docx", "Dateiname der Vorschau");
    assert_eq!(
        shown["title"], "Das Haus am Deich",
        "ohne title-Parameter gilt der Titel aus der Datei (Vorlage Title)"
    );
    let sections: Vec<(u64, &str)> = shown["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["level"].as_u64().unwrap(), s["title"].as_str().unwrap()))
        .collect();
    assert_eq!(
        sections,
        vec![(1, "Ankunft"), (2, "Der Deich"), (3, "Die Schleuse")],
        "die Gliederung ist die des Originals – Jörgs Einfügung in der Überschrift ändert sie nicht"
    );
    assert!(
        shown["sections"][0]["sample"].as_str().unwrap().contains("leise")
            && shown["sections"][0]["chars"].as_u64().unwrap() > 50,
        "Textprobe und Länge je Abschnitt, aus dem Stand vor den Änderungen: {}",
        shown["sections"][0]
    );
    assert_eq!(
        shown["editors"],
        json!([
            { "name": "Erika Beispiel", "handle": ERIKA, "insertions": 1, "deletions": 1, "sections": 1 },
            { "name": "Jörg Müller", "handle": JOERG, "insertions": 2, "deletions": 0, "sections": 1 },
        ]),
        "je Bearbeiter Name, Handle nach word_author, Zahl der Einfügungen und Löschungen und der Abschnitte mit \
         Vorschlag (Jörgs zweite Einfügung steht in einer Überschrift und ergibt keinen Vorschlag)"
    );
    assert_eq!(
        (shown["comments"].as_u64(), shown["footnotes"].as_u64(), shown["images"].as_u64()),
        (Some(3), Some(1), Some(1)),
        "Zahl der Kommentare, Fußnoten und Bilder"
    );
    assert_eq!(codes(&shown), vec!["docx_ueberschrift_tief"], "die Warnung zur Überschrift 4 steht in der Vorschau");

    let again = ok(&server, "docx_import.get", json!({ "draft": id })).await;
    assert_eq!(again, shown, "docx_import.get liefert dasselbe Objekt wie die Vorschau");

    let gone = ok(&server, "docx_import.discard", json!({ "draft": id })).await;
    assert_eq!(gone, json!({ "discarded": true }), "verwerfen meldet discarded");
    assert_eq!(docx_import::draft_count(&server.state), 0, "der Entwurf ist gelöscht");
    let twice = ok(&server, "docx_import.discard", json!({ "draft": id })).await;
    assert_eq!(twice, json!({ "discarded": true }), "auch wenn er schon weg ist");
    let error = fails(&server, "docx_import.get", json!({ "draft": id })).await;
    assert_eq!(error["code"], -32602, "ein unbekannter Entwurf ist ein ungültiger Parameter: {error}");
    let error = fails(&server, "docx_import.apply", json!({ "draft": id })).await;
    assert_eq!(error["code"], -32602, "ein verworfener Entwurf lässt sich nicht übernehmen: {error}");
    assert_eq!(works(&server).await, before, "nach alledem gibt es kein neues Werk");
}

#[tokio::test]
async fn broken_files_are_refused_with_invalid_params() {
    let server = common::open().await;
    let before = works(&server).await;
    let cases: [(&str, Vec<u8>, &str); 4] = [
        ("brief.docx", b"Nur Text, kein Archiv.".to_vec(), "kein ZIP"),
        ("alt.docx", [&[0xd0u8, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1][..], &[0u8; 64][..]].concat(), ".doc"),
        ("brief.doc", docx::sample(), "keine Word-Datei (.docx)"),
        ("leer.docx", docx::with_body(""), "keinen Text"),
    ];
    for (name, bytes, expected) in cases {
        let error =
            fails(&server, "docx_import.preview", json!({ "file_name": name, "data_base64": base64(&bytes) })).await;
        assert_eq!(error["code"], -32602, "{name}: Fehler der Datei sind ungültige Parameter: {error}");
        assert!(
            error["message"].as_str().unwrap().contains(expected),
            "{name}: die Meldung nennt den Grund im Klartext („{expected}“): {error}"
        );
    }
    let error =
        fails(&server, "docx_import.preview", json!({ "file_name": "x.docx", "data_base64": "kein base64 !!" })).await;
    assert_eq!(error["code"], -32602, "kaputtes Base64 ist ein ungültiger Parameter: {error}");
    assert_eq!(docx_import::draft_count(&server.state), 0, "nach Fehlern bleibt kein Entwurf liegen");
    assert_eq!(works(&server).await, before, "und kein Werk");
}

#[tokio::test]
async fn apply_creates_the_work_with_proposals_and_posts_and_nothing_becomes_current() {
    let server = common::open().await;
    let shown = preview(&server).await;
    let id = shown["draft"].as_str().unwrap();
    let applied = ok(&server, "docx_import.apply", json!({ "draft": id })).await;
    assert_eq!(
        (applied["sections"].as_u64(), applied["proposals"].as_u64(), applied["posts"].as_u64()),
        (Some(3), Some(2), Some(4)),
        "drei Abschnitte, zwei Vorschläge (Erika und Jörg am ersten Abschnitt), vier Beiträge (drei Kommentare und \
         die geänderte Überschrift): {applied}"
    );
    assert_eq!(codes(&applied), vec!["docx_ueberschrift_tief"], "die Hinweise der Vorschau stehen auch im Ergebnis");
    assert_eq!(docx_import::draft_count(&server.state), 0, "nach dem Übernehmen ist der Entwurf gelöscht");
    let work_id = applied["work"].as_i64().expect("Nummer des Werks");
    let slug = applied["slug"].as_str().expect("Kurzname des Werks").to_owned();
    let work = server.state.store.load_work(work_id).await.unwrap();
    assert_eq!(work.title, "Das Haus am Deich", "Titel des Werks aus der Datei");
    assert_eq!(
        work.nodes.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
        vec!["Ankunft", "Der Deich", "Die Schleuse"],
        "die Gliederung des Werks ist die des Originals"
    );

    for node in &work.nodes {
        let first = &node.revisions[0];
        assert_eq!(
            (first.number, first.status),
            (1, RevisionStatus::Current),
            "Fassung 1 von „{}“ ist die aktuelle",
            node.title
        );
        assert!(matches!(first.author, Author::Human { .. }), "und stammt vom Menschen (Import), nicht von Word");
        assert_eq!(
            node.revisions.iter().filter(|r| r.status == RevisionStatus::Current).count(),
            1,
            "je Abschnitt genau eine aktuelle Fassung – kein Vorschlag aus Word ist aktuell"
        );
    }
    let first = &work.nodes[0];
    let original = &first.revisions[0].body;
    assert!(
        original.starts_with("= Ankunft\n") && original.contains("vom Meer, leise#footnote["),
        "Fassung 1 ist der Stand vor den Änderungen (mit dem gelöschten Wort): {original}"
    );
    assert!(!original.contains("laut") && !original.contains("Abends"), "und ohne jeden eingefügten Text: {original}");
    assert_eq!(first.revisions.len(), 3, "am ersten Abschnitt: Fassung 1 und je ein Vorschlag von Erika und Jörg");
    let by = |handle: &str| {
        first
            .revisions
            .iter()
            .find(|r| matches!(&r.author, Author::Assistant { persona, .. } if persona == handle))
            .unwrap_or_else(|| panic!("Vorschlag von {handle} fehlt: {:?}", first.revisions))
    };
    let erika = by(ERIKA);
    assert_eq!(erika.status, RevisionStatus::Proposed, "Erikas Änderung ist ein offener Vorschlag");
    assert!(
        matches!(&erika.author, Author::Assistant { model, .. } if model == WORD_MODEL),
        "mit dem Modell-Feld „word“: {:?}",
        erika.author
    );
    assert_eq!(
        erika.note, "Änderungen aus Word von Erika Beispiel, 2026-11-20",
        "die Notiz nennt den wörtlichen Namen und das Datum"
    );
    assert!(
        erika.body.starts_with("= Ankunft\n") && erika.body.contains("]laut") && !erika.body.contains("leise"),
        "Erikas Vorschlag: ihre Löschung und Einfügung, dieselbe Überschriftszeile: {}",
        erika.body
    );
    assert!(!erika.body.contains("Abends"), "Jörgs Änderung steht nicht in Erikas Vorschlag (jeder gegen Fassung 1)");
    let joerg = by(JOERG);
    assert_eq!(joerg.status, RevisionStatus::Proposed, "Jörgs Änderung ist ein offener Vorschlag");
    assert!(
        joerg.body.contains("Abends wurde es still.") && joerg.body.contains("leise") && !joerg.body.contains("]laut"),
        "Jörgs Vorschlag: nur seine Einfügung, gegen Fassung 1: {}",
        joerg.body
    );
    assert_eq!(work.nodes[1].revisions.len(), 1, "am unveränderten Abschnitt gibt es keinen Vorschlag");
    assert_eq!(
        work.nodes[2].revisions.len(),
        1,
        "eine geänderte Überschrift ergibt keinen Vorschlag (sie änderte sonst die Gliederung)"
    );

    let posts = &first.posts;
    assert_eq!(posts.len(), 2, "am ersten Abschnitt: der Kommentar und seine Antwort");
    assert_eq!(posts[0].author, ERIKA, "der Kommentar steht unter dem Handle der Bearbeiterin");
    assert_eq!(
        posts[0].body,
        "Kommentar aus Word von Erika Beispiel, 2026-11-20\n\n> Der Wind kam\n\nWoher kam der Wind genau?",
        "erste Zeile Name und Datum, dann die kommentierte Stelle als Zitat, dann der Kommentar"
    );
    assert_eq!(
        posts[0].refs.iter().map(|r| (r.node, r.rev, r.quote.as_str())).collect::<Vec<_>>(),
        vec![(0, 1, "Der Wind kam")],
        "der Beitrag verweist auf die Stelle in Fassung 1"
    );
    assert_eq!(posts[1].author, JOERG, "die Antwort steht direkt danach unter dem Handle des Antwortenden");
    assert!(
        posts[1].body.starts_with("Antwort auf Erika Beispiel – aus Word von Jörg Müller, 2026-11-21\n")
            && posts[1].body.ends_with("Aus Nordwest & zwar stetig."),
        "sie nennt, wem sie antwortet: {}",
        posts[1].body
    );
    let third = &work.nodes[2].posts;
    assert_eq!(third.len(), 2, "am dritten Abschnitt: die geänderte Überschrift und der Kommentar zur Schleuse");
    assert_eq!(third[0].author, JOERG, "die Überschriftsänderung steht unter Jörgs Handle");
    assert!(
        third[0].body.starts_with("Überschrift in Word geändert: „Die Schleuse“ → „Die Schleuse am Hafen“"),
        "geänderte Überschriften werden ein Beitrag, nie eine neue Gliederung: {}",
        third[0].body
    );
    assert!(
        third[1].body.contains("> Die Schleuse\n") && third[1].body.ends_with("Schleuse oder Siel?"),
        "der Kommentar steht am Abschnitt der kommentierten Stelle, mit Zitat: {}",
        third[1].body
    );
    assert!(work.nodes[1].posts.is_empty(), "am zweiten Abschnitt hängt kein Kommentar");

    let image = server.state.store.asset_by_path(work_id, "bilder/leuchtturm.png").await.unwrap();
    assert!(image.is_some(), "das eingebettete Bild liegt im Werk unter dem Pfad, den der Text nennt");
    assert!(
        work.nodes[1].revisions[0].body.contains("image(\"bilder/leuchtturm.png\", alt: \"Ein Leuchtturm im Nebel\")"),
        "und der Text bindet es mit Alt-Text ein: {}",
        work.nodes[1].revisions[0].body
    );

    // Origin in the mirror: the import as a commit of the human, every proposal as a side branch with persona.
    let dir = server.mirror_dir_of(&slug);
    if common::git(&dir, &["--version"]).is_some() {
        let log = common::git(&dir, &["log", "--format=%s"]).unwrap();
        assert!(log.contains("Import aus deich.docx"), "der Import steht als Commit im Spiegel: {log}");
        let refs = common::git(&dir, &["for-each-ref", "--format=%(refname)", "refs/revisions/"]).unwrap();
        for number in [erika.number, joerg.number] {
            let name = format!("refs/revisions/{}/{number}", first.key);
            assert!(refs.lines().any(|r| r == name), "Seitenzweig des Vorschlags {number} im Spiegel: {refs}");
        }
        let side = format!("refs/revisions/{}/{}", first.key, erika.number);
        let message = common::git(&dir, &["log", "-1", "--format=%B", &side]).unwrap();
        assert!(
            message.contains(&format!("Persona: {ERIKA}")) && message.contains("Model: word"),
            "der Seitenzweig nennt die Herkunft (Persona und Modell): {message}"
        );
    }
}

#[tokio::test]
async fn changes_and_comments_can_be_left_out_and_a_taken_title_is_refused() {
    let server = common::open().await;
    let shown = preview(&server).await;
    let applied = ok(
        &server,
        "docx_import.apply",
        json!({ "draft": shown["draft"], "title": "Deich ohne Anhang", "changes": false, "comments": false }),
    )
    .await;
    assert_eq!(
        (applied["proposals"].as_u64(), applied["posts"].as_u64()),
        (Some(0), Some(0)),
        "ohne changes und comments entsteht nur das Werk: {applied}"
    );
    let work = server.state.store.load_work(applied["work"].as_i64().unwrap()).await.unwrap();
    assert_eq!(work.title, "Deich ohne Anhang", "der title-Parameter geht dem Titel der Datei vor");
    assert!(
        work.nodes.iter().all(|n| n.revisions.len() == 1 && n.posts.is_empty()),
        "jeder Abschnitt hat nur Fassung 1 und keinen Beitrag"
    );

    let again = preview(&server).await;
    let before = works(&server).await;
    let error =
        fails(&server, "docx_import.apply", json!({ "draft": again["draft"], "title": "Deich ohne Anhang" })).await;
    assert_eq!(error["code"], -32000, "ein vergebener Titel wird abgelehnt (REFUSED) wie bei jedem Import: {error}");
    assert_eq!(works(&server).await, before, "und es entsteht kein zweites Werk");
    assert_eq!(docx_import::draft_count(&server.state), 1, "der Entwurf bleibt für einen neuen Versuch liegen");
}

#[tokio::test]
async fn the_pages_work_without_javascript() {
    let server = common::open().await;
    let before = works(&server).await;
    let (status, page) = server.get("/werke/import/word").await;
    assert_eq!(status, StatusCode::OK, "die Formularseite antwortet");
    assert!(
        page.contains("action=\"/werke/import/word/vorschau\"")
            && page.contains("enctype=\"multipart/form-data\"")
            && page.contains("name=\"file\""),
        "ein gewöhnliches Formular mit Datei-Upload, ohne JavaScript bedienbar"
    );

    let sample = docx::sample();
    let (status, headers, _) =
        post_multipart(&server, "/werke/import/word/vorschau", &[("title", "")], Some(("deich.docx", &sample))).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "nach dem Lesen leitet die Vorschau um (303)");
    let target = location(&headers);
    assert!(target.starts_with("/werke/import/word?draft="), "auf die Seite mit dem Entwurf: {target}");
    assert_eq!(works(&server).await, before, "die Vorschau legt kein Werk an");
    let id = target.rsplit('=').next().unwrap().to_owned();

    let (status, page) = server.get(&target).await;
    assert_eq!(status, StatusCode::OK, "die Vorschauseite antwortet");
    for expected in [
        "Änderungen und Kommentare werden nie zur aktuellen Fassung.",
        "Ankunft",
        "Der Deich",
        "Die Schleuse",
        "Erika Beispiel",
        "<code>@word:erika-beispiel</code>",
        "Jörg Müller",
        "<code>@word:joerg-mueller</code>",
        "data-comments=\"3\"",
        "3 Kommentare",
        "data-code=\"docx_ueberschrift_tief\"",
        "action=\"/werke/import/word/uebernehmen\"",
        "action=\"/werke/import/word/verwerfen\"",
        "name=\"changes\" value=\"1\" checked",
        "name=\"comments\" value=\"1\" checked",
    ] {
        assert!(
            page.contains(expected),
            "die Vorschau zeigt „{expected}“ (Gliederung, Bearbeiter mit Handle, Kommentare, Warnungen, Formulare)"
        );
    }
    assert!(
        page.contains("<td>1</td><td>1</td><td>1</td>") && page.contains("<td>2</td><td>0</td><td>1</td>"),
        "je Bearbeiter die Zahl der Einfügungen, Löschungen und Abschnitte"
    );

    // Accept: 303 to the new work.
    let form = format!("draft={id}&title=Deich+vom+Formular&changes=1&comments=1");
    let (status, target) = server.post_form("/werke/import/word/uebernehmen", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "nach dem Übernehmen leitet die Seite um (303)");
    let target = target.unwrap_or_default();
    assert!(target.starts_with("/?work="), "auf das neue Werk: {target}");
    let work_id: i64 = target.trim_start_matches("/?work=").parse().expect("Nummer des Werks");
    let work = server.state.store.load_work(work_id).await.unwrap();
    assert_eq!(work.title, "Deich vom Formular", "der Titel aus dem Formular gilt");
    assert_eq!(work.nodes[0].revisions.len(), 3, "mit Häkchen „Änderungen“ entstehen die Vorschläge");
    assert_eq!(work.nodes[0].posts.len(), 2, "mit Häkchen „Kommentare“ die Beiträge");

    // Without the check boxes (a form without JavaScript then does not send them at all): only the work.
    let (_, headers, _) = post_multipart(
        &server,
        "/werke/import/word/vorschau",
        &[("title", "Deich pur")],
        Some(("deich.docx", &sample)),
    )
    .await;
    let second = location(&headers).rsplit('=').next().unwrap().to_owned();
    let (status, target) =
        server.post_form("/werke/import/word/uebernehmen", &format!("draft={second}&title=Deich+pur")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "auch ohne Häkchen wird übernommen");
    let pure: i64 = target.unwrap_or_default().trim_start_matches("/?work=").parse().expect("Nummer des Werks");
    let work = server.state.store.load_work(pure).await.unwrap();
    assert!(
        work.nodes.iter().all(|n| n.revisions.len() == 1 && n.posts.is_empty()),
        "ohne Häkchen keine Vorschläge und keine Beiträge"
    );

    // Title already taken: 422 with the preview and the message, the draft stays.
    let (_, headers, _) =
        post_multipart(&server, "/werke/import/word/vorschau", &[], Some(("deich.docx", &sample))).await;
    let third = location(&headers).rsplit('=').next().unwrap().to_owned();
    let request = Request::post("/werke/import/word/uebernehmen")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("draft={third}&title=Deich+pur&changes=1")))
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ein vergebener Titel ist ein Fehler der Eingabe (422)");
    assert!(
        page.contains("data-import-error") && page.contains("action=\"/werke/import/word/uebernehmen\""),
        "mit Meldung und erneut dem Formular der Vorschau"
    );
    assert!(
        page.contains("value=\"Deich pur\"") && !page.contains("name=\"comments\" value=\"1\" checked"),
        "die Eingaben bleiben stehen (Titel, Häkchen)"
    );

    // Discard: 303 back to the form, the draft is gone.
    let (status, target) = server.post_form("/werke/import/word/verwerfen", &format!("draft={third}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "verwerfen leitet um (303)");
    assert_eq!(target.as_deref(), Some("/werke/import/word"), "zurück auf die Formularseite");
    assert_eq!(docx_import::draft_count(&server.state), 0, "kein Entwurf bleibt liegen");
    let (status, page) = server.get(&format!("/werke/import/word?draft={third}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ein verworfener Entwurf hat keine Vorschau mehr (422)");
    assert!(page.contains("data-import-error") && page.contains("name=\"file\""), "Meldung und das Formular");
}

#[tokio::test]
async fn form_errors_are_422_with_the_form_and_foreign_origins_are_403() {
    let server = common::open().await;
    let before = works(&server).await;
    let (status, _, page) =
        post_multipart(&server, "/werke/import/word/vorschau", &[("title", "Bleibt")], Some(("notiz.txt", b"Text")))
            .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "eine Datei ohne .docx ist ein Fehler der Eingabe (422)");
    assert!(
        page.contains("data-import-error") && page.contains("name=\"file\"") && page.contains("value=\"Bleibt\""),
        "mit Meldung, dem Formular und dem eingegebenen Titel"
    );
    let (status, _, page) =
        post_multipart(&server, "/werke/import/word/vorschau", &[], Some(("kaputt.docx", b"PK\x03\x04kein Archiv")))
            .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "eine unlesbare Datei auch (422)");
    assert!(page.contains("Archiv der Word-Datei"), "die Meldung des Lesers steht im Klartext auf der Seite");
    let (status, _, page) = post_multipart(&server, "/werke/import/word/vorschau", &[("title", "x")], None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Datei: 422");
    assert!(page.contains("Bitte eine Word-Datei"), "mit der Bitte, eine Datei zu wählen");

    let sample = docx::sample();
    for (uri, content_type, body) in [
        (
            "/werke/import/word/vorschau",
            multipart(&[], Some(("deich.docx", &sample))).0,
            multipart(&[], Some(("deich.docx", &sample))).1,
        ),
        (
            "/werke/import/word/uebernehmen",
            "application/x-www-form-urlencoded".to_owned(),
            format!("draft={}", "a".repeat(32)).into_bytes(),
        ),
        (
            "/werke/import/word/verwerfen",
            "application/x-www-form-urlencoded".to_owned(),
            format!("draft={}", "a".repeat(32)).into_bytes(),
        ),
    ] {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, content_type)
            .header(header::ORIGIN, "http://fremd.example")
            .body(Body::from(body))
            .unwrap();
        let (status, _, _) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}: fremde Herkunft wird abgelehnt (guard::same_origin)");
    }
    assert_eq!(docx_import::draft_count(&server.state), 0, "nach alledem liegt kein Entwurf vor");
    assert_eq!(works(&server).await, before, "und kein Werk ist entstanden");
}

fn snapshot(dir: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).unwrap().flatten() {
            let path = entry.path();
            // The directories are compared (works, drafts); the database writes its companion files when reading.
            if path.is_dir() {
                found.push(format!("{}/", path.strip_prefix(dir).unwrap().display()));
                if !path.ends_with(".git") {
                    stack.push(path);
                }
            }
        }
    }
    found.sort();
    found
}

#[tokio::test]
async fn the_command_line_previews_without_writing_and_applies_on_request() {
    let server = common::open().await;
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("deich.docx");
    std::fs::write(&file, docx::sample()).unwrap();
    let before = works(&server).await;
    let folders = snapshot(server.path());

    let shown =
        docx_import::cli_import(&server.state, &file, &CliDocxImport::default()).await.expect("Vorschau gelingt");
    for expected in [
        "Vorschau von deich.docx",
        "Titel „Das Haus am Deich“",
        "Gliederung (3 Abschnitte)",
        "= Ankunft",
        "== Der Deich",
        "=== Die Schleuse",
        "Erika Beispiel → @word:erika-beispiel: 1 Einfügung(en), 1 Löschung(en), 1 Abschnitt(e)",
        "Jörg Müller → @word:joerg-mueller: 2 Einfügung(en), 0 Löschung(en), 1 Abschnitt(e)",
        "Kommentare: 3, Fußnoten: 1, Bilder: 1",
        "Hinweis [docx_ueberschrift_tief]",
        "nie zur aktuellen Fassung",
    ] {
        assert!(shown.contains(expected), "die Vorschau nennt „{expected}“: {shown}");
    }
    assert_eq!(works(&server).await, before, "ohne --uebernehmen entsteht kein Werk");
    assert_eq!(snapshot(server.path()), folders, "der Datenordner bleibt unverändert – auch kein Entwurf");
    assert!(!server.path().join(docx_import::DRAFT_DIR).exists(), "kein Ordner docx-drafts");

    let options = CliDocxImport { title: Some("Deich von der Kommandozeile".into()), apply: true };
    let done = docx_import::cli_import(&server.state, &file, &options).await.expect("Übernehmen gelingt");
    assert!(
        done.contains("„Deich von der Kommandozeile“") && done.contains("3 Abschnitte, 2 Vorschläge, 4 Beiträge"),
        "die Ausgabe nennt Titel und Zahlen: {done}"
    );
    assert_eq!(works(&server).await, before + 1, "mit --uebernehmen entsteht genau ein Werk");
    let list = server.state.store.work_list().await.unwrap();
    let created = list.iter().find(|w| w.title == "Deich von der Kommandozeile").expect("das neue Werk");
    let work = server.state.store.load_work(created.id).await.unwrap();
    assert_eq!(
        work.nodes[0].revisions.iter().filter(|r| r.status == RevisionStatus::Proposed).count(),
        2,
        "samt den Vorschlägen beider Bearbeiter"
    );
    assert_eq!(work.nodes.iter().map(|n| n.posts.len()).sum::<usize>(), 4, "und den vier Beiträgen");

    let broken = folder.path().join("kaputt.docx");
    std::fs::write(&broken, b"Das ist kein Archiv.").unwrap();
    let error = docx_import::cli_import(&server.state, &broken, &CliDocxImport { title: None, apply: true })
        .await
        .expect_err("eine kaputte Datei ist ein Fehler (Exit 1 in main.rs)");
    assert!(
        error.contains("kaputt.docx") && error.contains("Keine Word-Datei (.docx)") && error.contains("kein ZIP"),
        "die Fehlermeldung nennt Datei und Grund im Klartext: {error}"
    );
    let missing = docx_import::cli_import(&server.state, &folder.path().join("fehlt.docx"), &CliDocxImport::default())
        .await
        .expect_err("eine fehlende Datei ist ein Fehler");
    assert!(missing.contains("lässt sich nicht lesen"), "mit dem Grund des Betriebssystems: {missing}");
    assert_eq!(works(&server).await, before + 1, "Fehler legen nichts an");
}

#[tokio::test]
async fn there_is_no_word_import_over_mcp() {
    let server = common::open().await;
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/list antwortet: {body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = answer["result"]["tools"]
        .as_array()
        .expect("Werkzeugliste")
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.len() > 20, "die Liste ist die echte Werkzeugliste ({} Werkzeuge)", names.len());
    for name in &names {
        assert!(
            !name.contains("docx") && !name.contains("word") && !name.contains("import"),
            "über MCP gibt es keinen Import: das Werkzeug „{name}“ dürfte es nicht geben"
        );
    }
}
