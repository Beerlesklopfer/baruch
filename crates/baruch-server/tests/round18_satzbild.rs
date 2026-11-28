// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 18c (satzbild-werk): typeset-view settings per work (RPC `work.sheet`/`work.sheet_set`, form
//! `POST /einstellungen/werk/satzbild`, style already in the HTML), long sections without a typeset view set in
//! advance, image at the cursor position (`POST /werke/bilder/einfuegen` with `place` and `body`), proposal leftovers
//! (empty „−“ line, „Alle einklappen“ before the reading flow) and a behaviour test "typeset view as default": the
//! compiled widgets (`wysiwyg.js`, `proposals.js`, `sheet.js`) run in Node 20 on a small mocked DOM against the editor
//! HTML of the server – without browser, without network (fetch is mocked). All texts freely invented.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

/// Freely invented work with template (font, size, measure) and two chapters.
const BOOK: &str = "#set document(title: \"Das Zimmerbuch\", author: \"Ida Probe\")\n\
#set text(font: \"Erfundene Antiqua\", size: 10pt)\n#set page(width: 13cm)\n\n\
= Fenster\n\nDas Licht fällt schräg.\n\nDer Vorhang weht.\n\nAm Rahmen blättert Farbe.\n\n\
= Tür\n\nSie knarrt.\n";

fn ok(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Import the work; work id and key/revision of the first chapter.
async fn book(server: &common::Server) -> (i64, i64, u64) {
    let work = server.import(BOOK, "zimmer.typ").await.work;
    let (key, rev) = first_chapter(server, work).await;
    (work, key, rev)
}

async fn first_chapter(server: &common::Server, work: i64) -> (i64, u64) {
    let got = server.rpc("work.get", json!({ "work": work })).await;
    let node = ok(&got)["work"]["nodes"][0].clone();
    let rev = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap()["number"]
        .as_u64()
        .unwrap();
    (node["key"].as_i64().unwrap(), rev)
}

async fn current_body(server: &common::Server, work: i64, key: i64) -> (u64, String) {
    let got = server.rpc("work.get", json!({ "work": work })).await;
    let nodes = ok(&got)["work"]["nodes"].as_array().unwrap().clone();
    let node = nodes.iter().find(|n| n["key"] == key).unwrap();
    let current = node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap();
    (current["number"].as_u64().unwrap(), current["body"].as_str().unwrap().to_owned())
}

/// Content of `<style id="ws-style" data-work="W">…</style>` of a page.
fn served_style(page: &str, work: i64) -> Option<&str> {
    let open = format!("<style id=\"ws-style\" data-work=\"{work}\">");
    let at = page.find(&open)? + open.len();
    Some(&page[at..at + page[at..].find("</style>")?])
}

fn commits(server: &common::Server, slug: &str) -> Option<u32> {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).and_then(|c| c.trim().parse().ok())
}

// ---- Typeset-view settings: RPC, style the same in all places -----------------------------------

#[tokio::test]
async fn work_sheet_settings_override_the_template_the_same_way_in_rpc_style_and_page() {
    let server = common::open().await;
    let (work, key, _) = book(&server).await;
    let start = server.rpc("work.sheet", json!({ "work": work })).await;
    let start = ok(&start);
    assert!(
        start["settings"].as_object().unwrap().values().all(Value::is_null),
        "ohne Einstellungen ist kein Feld gesetzt: {start}"
    );
    let css = start["css"].as_str().unwrap();
    assert!(css.contains("--ws-font: \"Erfundene Antiqua\";"), "ohne Einstellungen die Schrift der Vorlage: {css}");
    assert!(css.contains("--ws-width: 13cm;"), "und ihre Satzbreite: {css}");

    let set = server
        .rpc(
            "work.sheet_set",
            json!({ "work": work, "width": "16cm", "background": "#FBF8F1", "font": "Georgia, serif" }),
        )
        .await;
    let set = ok(&set);
    assert_eq!(set["settings"]["width"], "16cm", "Satzbreite gespeichert: {set}");
    assert_eq!(set["settings"]["background"], "#fbf8f1", "Blattfarbe klein geschrieben: {set}");
    let css = set["css"].as_str().unwrap().to_owned();
    for part in ["--ws-width: 16cm;", "--ws-background: #fbf8f1;", "--ws-font: Georgia, serif;", "--ws-size: 10pt;"] {
        assert!(css.contains(part), "{part}: Einstellungen gewinnen, der Rest (Größe) bleibt aus der Vorlage: {css}");
    }
    assert!(!css.contains("13cm"), "die Satzbreite der Vorlage ist überschrieben: {css}");

    let style = server.rpc("wysiwyg.style", json!({ "work": work })).await;
    assert_eq!(ok(&style)["css"], css.as_str(), "wysiwyg.style liefert dasselbe CSS wie work.sheet");
    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite lädt");
    assert_eq!(
        served_style(&page, work),
        Some(css.as_str()),
        "der Stil steht schon im HTML, gleich work.sheet (kein Wechsel von Schrift und Größe nach dem Laden)"
    );

    // A missing field stays, null takes back, reset takes everything back.
    let more = server.rpc("work.sheet_set", json!({ "work": work, "size": "12pt" })).await;
    assert_eq!(ok(&more)["settings"]["width"], "16cm", "ein fehlendes Feld bleibt, wie es war: {more}");
    assert_eq!(ok(&more)["settings"]["size"], "12pt", "das genannte Feld ist gesetzt: {more}");
    let less = server.rpc("work.sheet_set", json!({ "work": work, "width": null, "font": "" })).await;
    assert!(ok(&less)["settings"]["width"].is_null(), "null nimmt die Satzbreite zurück: {less}");
    assert!(ok(&less)["settings"]["font"].is_null(), "leerer Text nimmt die Schrift zurück: {less}");
    assert!(ok(&less)["css"].as_str().unwrap().contains("--ws-width: 13cm;"), "wieder die Breite der Vorlage");
    let reset = server.rpc("work.sheet_set", json!({ "work": work, "reset": true })).await;
    assert!(
        ok(&reset)["settings"].as_object().unwrap().values().all(Value::is_null),
        "reset nimmt alle Felder zurück: {reset}"
    );
    let rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM work_sheet").fetch_one(server.state.store.pool()).await.unwrap();
    assert_eq!(rows, 0, "nach reset steht keine Zeile mehr in work_sheet");
}

#[tokio::test]
async fn invalid_sheet_values_are_refused_and_change_nothing() {
    let server = common::open().await;
    let (work, _, _) = book(&server).await;
    ok(&server.rpc("work.sheet_set", json!({ "work": work, "fill": "#123456" })).await);
    for (params, code, why) in [
        (json!({ "work": work, "fill": "red" }), -32000, "Farbname statt #rrggbb"),
        (json!({ "work": work, "size": "200pt" }), -32000, "Schriftgröße über 48 pt"),
        (json!({ "work": work, "font": "Serif; } body { x" }), -32000, "Schrift mit ; und Klammern"),
        (json!({ "work": work, "font": 5 }), -32602, "Schrift ist kein Text"),
        (
            json!({ "work": work, "reset": "vielleicht" }),
            -32000,
            "unbekannter Text bei reset gilt als nein, fill bleibt",
        ),
    ] {
        let answer = server.rpc("work.sheet_set", params.clone()).await;
        if why.starts_with("unbekannter") {
            assert_eq!(ok(&answer)["settings"]["fill"], "#123456", "{why}: {answer}");
            continue;
        }
        assert_eq!(answer["error"]["code"], code, "{why}: {answer}");
        let read = server.rpc("work.sheet", json!({ "work": work })).await;
        assert_eq!(ok(&read)["settings"]["fill"], "#123456", "{why}: nichts geändert");
    }
    let answer = server.rpc("work.sheet_set", json!({ "work": work, "width": "99cm" })).await;
    let message = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("Satzbreite") && message.contains("60 cm"), "deutsche Meldung mit Grenze: {answer}");
    let unknown = server.rpc("work.sheet", json!({ "work": 99_999 })).await;
    assert!(unknown["error"].is_object(), "unbekanntes Werk: Fehler statt Vorgabe: {unknown}");
}

#[tokio::test]
async fn the_settings_form_works_without_javascript_and_refuses_a_foreign_origin() {
    let server = common::open().await;
    let (work, _, _) = book(&server).await;
    let form = format!(
        "method=work.sheet_set&work={work}&font=%22Probe+Serif%22%2C+serif&heading_font=&size=11pt&width=&fill=&heading_fill=%237a1f1f&background="
    );
    let (status, location) = server.post_form("/einstellungen/werk/satzbild", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular ohne JavaScript: 303 zurück");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&ok=satzbild#satzbild").as_str()),
        "Rücksprung zum Block „Satzbild“"
    );
    let read = server.rpc("work.sheet", json!({ "work": work })).await;
    assert_eq!(ok(&read)["settings"]["font"], "\"Probe Serif\", serif", "Schrift gespeichert: {read}");
    assert!(ok(&read)["settings"]["width"].is_null(), "leere Felder = aus der Vorlage: {read}");

    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}&ok=satzbild")).await;
    assert!(page.contains("id=\"satzbild\"") && page.contains("<h3>Satzbild</h3>"), "Block „Satzbild“ mit Anker");
    assert!(page.contains("action=\"/einstellungen/werk/satzbild\""), "das Formular geht an die Route");
    assert!(page.contains("Die Satzbild-Einstellungen sind gespeichert."), "Hinweis nach ok=satzbild");
    assert!(page.contains("value=\"&quot;Probe Serif&quot;, serif\""), "gespeicherte Schrift maskiert im Feld");
    assert!(page.contains("placeholder=\"13cm\""), "leeres Feld zeigt den Wert der Vorlage als Platzhalter");
    assert!(page.contains("name=\"reset\" value=\"1\""), "mit Einstellungen gibt es „Alles aus der Vorlage“");

    let (status, location) =
        server.post_form("/einstellungen/werk/satzbild", &format!("method=work.sheet_set&work={work}&size=3pt")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "auch ein Fehler führt ohne JavaScript zurück");
    assert_eq!(
        location.as_deref(),
        Some(format!("/einstellungen/werk?work={work}&satzbildfehler=1#satzbild").as_str()),
        "Rücksprung mit Fehlermarke"
    );
    let read = server.rpc("work.sheet", json!({ "work": work })).await;
    assert_eq!(ok(&read)["settings"]["size"], "11pt", "nach dem Fehler unverändert: {read}");
    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}&satzbildfehler=1")).await;
    assert!(page.contains("ließen sich nicht speichern"), "nach satzbildfehler=1 steht die Meldung da");

    let (status, _) = server.post_form("/einstellungen/werk/satzbild", &format!("work={work}&reset=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "zurücksetzen geht ohne JavaScript");
    let read = server.rpc("work.sheet", json!({ "work": work })).await;
    assert!(ok(&read)["settings"].as_object().unwrap().values().all(Value::is_null), "alles zurück: {read}");

    let request = Request::post("/einstellungen/werk/satzbild")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::from(format!("method=work.sheet_set&work={work}&size=20pt")))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft abgelehnt (guard::same_origin)");
    let read = server.rpc("work.sheet", json!({ "work": work })).await;
    assert!(ok(&read)["settings"]["size"].is_null(), "die fremde Anfrage hat nichts gesetzt: {read}");
}

/// A value that would get into the table past the check of the store (here directly by SQL, only the length CHECKs take
/// effect) never reaches the `<style>` element: `sheet_css` drops it.
#[tokio::test]
async fn a_malicious_value_never_leaves_the_style_element() {
    let server = common::open().await;
    let (work, key, _) = book(&server).await;
    let evil = "X</style><script>alert(1)</script>";
    let refused = server.rpc("work.sheet_set", json!({ "work": work, "font": evil })).await;
    assert_eq!(refused["error"]["code"], -32000, "der Store lehnt den Wert ab: {refused}");
    sqlx::query("INSERT INTO work_sheet (work_id, font, heading_font, size) VALUES (?, ?, ?, '11pt')")
        .bind(work)
        .bind(evil)
        .bind("\"Offen, serif")
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    let style = served_style(&page, work).expect("Stil im HTML");
    assert!(!style.contains('<'), "kein < im Stil, also kein </style: {style}");
    assert!(!page.contains("<script>alert(1)"), "das Skript steht nirgends auf der Seite");
    assert!(style.contains("--ws-size: 11pt;"), "die gültigen Werte bleiben: {style}");
    assert!(!style.contains("Offen"), "ein unpaariges Anführungszeichen fällt weg: {style}");
    let rpc = server.rpc("work.sheet", json!({ "work": work })).await;
    assert!(!ok(&rpc)["css"].as_str().unwrap().contains('<'), "auch work.sheet liefert kein <");
}

#[tokio::test]
async fn sheet_settings_leave_typst_export_and_pdf_byte_equal() {
    let _pdf = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let (work, _, _) = book(&server).await;
    let export = server.state.store.export_typst(work).await.unwrap();
    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    let first = baruch_server::pdf::book_pdf(&server.state, work).await.expect("PDF vorher").bytes;
    let second = baruch_server::pdf::book_pdf(&server.state, work).await.expect("PDF vorher, zweiter Lauf").bytes;
    ok(&server
        .rpc(
            "work.sheet_set",
            json!({ "work": work, "font": "Georgia", "size": "14pt", "width": "20cm", "fill": "#ff0000", "background": "#000000" }),
        )
        .await);
    assert_eq!(server.state.store.export_typst(work).await.unwrap(), export, "Typst-Export byte-gleich");
    let after = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert_eq!(after.source.main, prepared.source.main, "die Quelle für Satz und PDF ist byte-gleich");
    assert_eq!(after.stand, prepared.stand, "derselbe Stand (Zwischenspeicher der Satz-Ansicht bleibt gültig)");
    let pdf = baruch_server::pdf::book_pdf(&server.state, work).await.expect("PDF nachher").bytes;
    if first == second {
        assert_eq!(pdf, first, "das PDF ist byte-gleich: Satzbild-Einstellungen gelten nur im Browser");
    } else {
        // Two runs without a change already differ (e.g. timestamps): then only size and pages.
        eprintln!("Hinweis: zwei PDF-Läufe ohne Änderung sind nicht byte-gleich; verglichen wird die Länge.");
        assert_eq!(pdf.len(), first.len(), "das PDF hat dieselbe Länge wie vorher");
    }
}

// ---- Long sections --------------------------------------------------------------------------------

#[tokio::test]
async fn sections_over_64_kib_get_no_ready_satzbild_but_the_widget_can_fetch_it() {
    let server = common::open().await;
    let (work, key, rev) = book(&server).await;
    let (_, short) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(short.contains(" data-ws-ready>"), "kurzer Abschnitt: Satzbild fertig vom Server (Gegenprobe)");

    let paragraph = "Der Staub tanzt im Licht, das schräg durch das Fenster fällt, und niemand wischt ihn fort.\n\n";
    let mut long = String::from("= Fenster\n\n");
    while long.len() <= baruch_server::view::READY_SHEET_MAX {
        long.push_str(paragraph);
    }
    let saved = server.rpc("revision.save", json!({ "work": work, "key": key, "based_on": rev, "body": long })).await;
    ok(&saved);
    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Seite mit langem Abschnitt lädt");
    assert!(
        !page.contains("data-ws-ready"),
        "über 64 KiB kein vorab gesetztes Satzbild (sonst doppelter Text im HTML)"
    );
    assert!(page.contains("data-ws-sheet contenteditable=\"true\""), "das leere Blatt ist da, das Widget füllt es");
    // Round 39a: the widget catalog in the page head (every widget text) does not count as the section's text.
    let catalog = baruch_server::i18n::widget_catalog_json(baruch_server::i18n::Lang::De).len();
    assert!(
        page.len() - catalog < 2 * long.len() + 64 * 1024,
        "der Text steht nur einmal im HTML (Textfeld): {} Bytes",
        page.len()
    );
    let parsed = server.rpc("wysiwyg.parse", json!({ "work": work, "body": long })).await;
    assert!(ok(&parsed)["html"].as_str().unwrap().contains("Der Staub tanzt"), "das Widget holt es per RPC");
}

// ---- Proposals: empty „−“ line, bar before the reading flow ----------------------------------------

#[tokio::test]
async fn a_removed_block_shows_no_empty_minus_line_and_the_bar_stands_before_the_flow() {
    let server = common::open().await;
    let (work, key, _) = book(&server).await;
    let node = server.state.store.node_seq(work, key).await.unwrap();
    let proposal = "= Fenster\n\nDas Licht fällt schräg.\n\nAm Rahmen blättert Farbe.\n";
    server.state.store.add_proposal(work, node, proposal, "@lektor", "probe-1", "").await.unwrap();
    let (_, page) = server.get(&format!("/?work={work}&key={key}&view=preview")).await;
    assert!(
        page.contains("<span class=\"removed\">− Der Vorhang weht.</span>"),
        "das Gestrichene steht als Zeile da: {page}"
    );
    assert!(!page.contains("<span class=\"removed\">− </span>"), "keine leere „−“-Zeile (Leerzeile hinter dem Block)");

    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    let bar =
        page.find(&format!("<div class=\"pv-bar pv-bar-editor\" data-pv-bar=\"{key}\">")).expect("Leiste im Editor");
    let flow = page.find("<section class=\"pv-flow\"").expect("Lesefluss");
    assert!(bar < flow, "die Leiste steht vor dem Lesefluss (eigenes Kind des Blatts), nicht in ihm");
    assert!(
        page[bar..flow]
            .contains("<button class=\"btn tiny\" type=\"button\" data-pv-all hidden>Alle einklappen</button>"),
        "„Alle einklappen“ in der Leiste, ohne JavaScript verborgen"
    );
    assert!(
        page[flow..].contains("<div class=\"pv-forms\">"),
        "die Formulare Übernehmen/Verwerfen bleiben im Lesefluss"
    );
}

// ---- Image at the cursor position -------------------------------------------------------------------

async fn add_image(server: &common::Server, work: i64, file: &str, path: &str) {
    let svg =
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><rect width=\"8\" height=\"8\"/></svg>\n";
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": file, "content": svg, "encoding": "svg", "path": path,
                    "caption": "Ein Fenster", "alt": "Fensterkreuz" }),
        )
        .await;
    ok(&answer);
}

async fn asset_paths(server: &common::Server, work: i64, name: &str) -> Value {
    let list = server.rpc("asset.list", json!({ "work": work })).await;
    ok(&list)["assets"].as_array().unwrap().iter().find(|a| a["name"] == name).unwrap()["paths"].clone()
}

const FIGURE: &str = "#figure(image(\"bilder/fenster.svg\", alt: \"Fensterkreuz\"), caption: [Ein Fenster])";

#[tokio::test]
async fn an_image_lands_at_end_start_before_a_block_or_at_an_offset_as_its_own_block() {
    let server = common::open().await;
    let (work, key, _) = book(&server).await;
    add_image(&server, work, "fenster.svg", "bilder/fenster.svg").await;
    let slug = server.state.store.work_slug(work).await.unwrap();
    let original = "= Fenster\n\nDas Licht fällt schräg.\n\nDer Vorhang weht.\n\nAm Rahmen blättert Farbe.\n\n";
    let cases: Vec<(String, Option<&str>, String, &str)> = vec![
        ("end".into(), None, format!("{}\n\n{FIGURE}\n", original.trim_end()), "am Ende wie seit Runde 11"),
        (
            "start".into(),
            None,
            format!(
                "= Fenster\n\n{FIGURE}\n\nDas Licht fällt schräg.\n\nDer Vorhang weht.\n\nAm Rahmen blättert Farbe.\n\n"
            ),
            "am Anfang: direkt unter der Überschrift, nie davor",
        ),
        (
            "block:2".into(),
            None,
            format!(
                "= Fenster\n\nDas Licht fällt schräg.\n\n{FIGURE}\n\nDer Vorhang weht.\n\nAm Rahmen blättert Farbe.\n\n"
            ),
            "vor Block 2 (dem zweiten Absatz)",
        ),
        (
            "block:0".into(),
            None,
            format!(
                "= Fenster\n\n{FIGURE}\n\nDas Licht fällt schräg.\n\nDer Vorhang weht.\n\nAm Rahmen blättert Farbe.\n\n"
            ),
            "vor Block 0 wäre vor der Überschrift: rückt dahinter",
        ),
        ("block:99".into(), None, format!("{}\n\n{FIGURE}\n", original.trim_end()), "Blockzahl überschritten: am Ende"),
        (
            // „ä“ counts one unit in UTF-16; the position in the middle of „Licht“ moves to the next line boundary.
            format!("offset:{}", "= Fenster\n\nDas Li".encode_utf16().count()),
            None,
            format!(
                "= Fenster\n\nDas Licht fällt schräg.\n\n{FIGURE}\n\nDer Vorhang weht.\n\nAm Rahmen blättert Farbe.\n\n"
            ),
            "Stelle mitten im Absatz: an der nächsten Zeilengrenze, mit Leerzeile davor",
        ),
    ];
    for (place, body, expected, why) in cases {
        // Every position on the original revision: first back to the initial text.
        let (rev, text) = current_body(&server, work, key).await;
        if text != original {
            ok(&server
                .rpc("revision.save", json!({ "work": work, "key": key, "based_on": rev, "body": original }))
                .await);
        }
        let (rev, _) = current_body(&server, work, key).await;
        let before = commits(&server, &slug);
        let mut form = format!("work={work}&key={key}&rev={rev}&name=fenster.svg&place={place}");
        if let Some(body) = body {
            form.push_str(&format!("&body={}", urlencoding(body)));
        }
        let (status, location) = server.post_form("/werke/bilder/einfuegen", &form).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{why}: eingefügt → 303");
        assert_eq!(
            location.as_deref(),
            Some(format!("/?work={work}&key={key}").as_str()),
            "{why}: zurück zum Abschnitt"
        );
        let (_, text) = current_body(&server, work, key).await;
        assert_eq!(text, expected, "{why}");
        if let (Some(before), Some(after)) = (before, commits(&server, &slug)) {
            assert_eq!(after, before + 1, "{why}: genau ein Spiegel-Commit");
        }
    }
}

/// With JavaScript the widget sends the unsaved text (`body`) and the UTF-16 position; the form encodes line breaks as
/// CRLF. The heading line must stay the same, otherwise „erst speichern“.
#[tokio::test]
async fn with_an_unsaved_body_the_offset_counts_in_it_and_a_changed_heading_is_refused() {
    let server = common::open().await;
    let (work, key, rev) = book(&server).await;
    add_image(&server, work, "fenster.svg", "bilder/fenster.svg").await;
    let unsaved = "= Fenster\n\nNeu getippt, noch nicht gespeichert.\n\nDas Licht fällt schräg.\n";
    let offset = "= Fenster\n\nNeu getippt, noch nicht gespeichert.\n\n".encode_utf16().count();
    let form = format!(
        "work={work}&key={key}&rev={rev}&name=fenster.svg&place=offset:{offset}&body={}",
        urlencoding(&unsaved.replace('\n', "\r\n"))
    );
    let (status, _) = server.post_form("/werke/bilder/einfuegen", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "eingefügt in den ungespeicherten Text");
    let (_, text) = current_body(&server, work, key).await;
    assert_eq!(
        text,
        format!("= Fenster\n\nNeu getippt, noch nicht gespeichert.\n\n{FIGURE}\n\nDas Licht fällt schräg.\n"),
        "der ungespeicherte Text samt Bild ist die neue Fassung; die Stelle zählt im Text mit LF"
    );

    let (rev, _) = current_body(&server, work, key).await;
    let renamed = "= Ein anderer Titel\n\nText.\n";
    let form = format!("work={work}&key={key}&rev={rev}&name=fenster.svg&place=end&body={}", urlencoding(renamed));
    let request = Request::post("/werke/bilder/einfuegen")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let (status, _, message) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "geänderte Überschriftszeile → 422: {message}");
    assert!(message.contains("erst speichern"), "Meldung „erst speichern“: {message}");
    let (after, _) = current_body(&server, work, key).await;
    assert_eq!(after, rev, "keine neue Fassung");
    let (status, _) = server
        .post_form("/werke/bilder/einfuegen", &format!("work={work}&key={key}&rev={rev}&name=fenster.svg&place=mitte"))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unbekannte Stelle → 422");
}

/// Backlog Round 11: the path was assigned before saving the revision; if saving failed, it was left behind without a
/// mirror commit. Now path and revision are in one transaction.
#[tokio::test]
async fn when_saving_fails_the_image_gets_no_path_and_nothing_is_committed() {
    let server = common::open().await;
    let (work, key, rev) = book(&server).await;
    add_image(&server, work, "ohnepfad.svg", "").await;
    assert_eq!(asset_paths(&server, work, "ohnepfad.svg").await, json!([]), "das Bild hat noch keinen Pfad");
    let slug = server.state.store.work_slug(work).await.unwrap();
    let before = commits(&server, &slug);
    // Same heading line, but a second heading of the same level: saving fails in the store (exactly one heading per
    // section) – after the path assignment in the same transaction.
    let broken = "= Fenster\n\nText.\n\n= Noch ein Kapitel\n\nMehr.\n";
    let form = format!("work={work}&key={key}&rev={rev}&name=ohnepfad.svg&place=end&body={}", urlencoding(broken));
    let (status, _) = server.post_form("/werke/bilder/einfuegen", &form).await;
    assert!(status.is_client_error(), "Speichern scheitert → Fehler (4xx): {status}");
    assert_eq!(asset_paths(&server, work, "ohnepfad.svg").await, json!([]), "kein Pfad zurückgeblieben");
    let (after, _) = current_body(&server, work, key).await;
    assert_eq!(after, rev, "keine neue Fassung");
    assert_eq!(commits(&server, &slug), before, "kein Spiegel-Commit");

    let (status, _) = server
        .post_form("/werke/bilder/einfuegen", &format!("work={work}&key={key}&rev={rev}&name=ohnepfad.svg"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne Fehler: eingefügt");
    assert_eq!(
        asset_paths(&server, work, "ohnepfad.svg").await,
        json!(["bilder/ohnepfad.svg"]),
        "Pfad mit der Fassung"
    );
    let (_, text) = current_body(&server, work, key).await;
    assert!(text.contains("#figure(image(\"bilder/ohnepfad.svg\""), "die Fassung nennt den vergebenen Pfad: {text}");
    if let (Some(before), Some(after)) = (before, commits(&server, &slug)) {
        assert_eq!(after, before + 1, "ein Commit für Fassung und Pfad zusammen");
    }
}

#[tokio::test]
async fn the_insert_form_offers_the_places_without_javascript() {
    let server = common::open().await;
    let (work, key, _) = book(&server).await;
    add_image(&server, work, "fenster.svg", "bilder/fenster.svg").await;
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    let form = &page[page.find("<form class=\"image-insert-form\"").expect("Formular „Bild einfügen“")..];
    let form = &form[..form.find("</form>").unwrap()];
    assert!(form.contains("<select name=\"place\" data-image-place>"), "Auswahl der Stelle ohne JavaScript: {form}");
    for option in [
        "<option value=\"end\">am Ende</option>",
        "<option value=\"start\">am Anfang (unter der Überschrift)</option>",
        "<option value=\"block:1\">vor „Das Licht fällt schräg.“</option>",
        "<option value=\"block:3\">vor „Am Rahmen blättert Farbe.“</option>",
    ] {
        assert!(form.contains(option), "Stelle {option}: {form}");
    }
    assert!(!form.contains("value=\"block:0\""), "nie vor der Überschrift: {form}");
    assert!(
        form.contains("<input type=\"hidden\" name=\"body\" value=\"\" disabled data-image-body>"),
        "body ohne JavaScript gesperrt (wird nie geschickt): {form}"
    );
    assert!(page.contains("data-image-insert data-widget=\"sheet-tools\""), "Widget sheet-tools am details");
}

/// Form encoding for test texts (letters, digits and `-_.~` stay, everything else `%XX`).
fn urlencoding(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// ---- Behaviour test "typeset view as default" (Node, without browser, without network) --------------

/// Small DOM mock and flow of the scenes. Reads `scenario.json` (HTML of the editor, cookie, responses for the mocked
/// `fetch`), mounts the real, compiled widgets and writes what was observed as JSON. Only what the widgets need on
/// mounting is mocked; if something is missing, the error is in `errors`. Round 23a: the DOM mock ([`DOM`]) stands
/// alone, so that other behaviour tests (`round23_web/`) can use it with a flow of their own too ([`run_harness`]);
/// [`SCENE`] is the flow of this test.
pub(crate) const DOM: &str = r#####"
import { readFileSync } from "node:fs";
const scenario = JSON.parse(readFileSync(process.argv[2], "utf8"));
const errors = [];
const calls = [];
const submitted = [];
console.error = (...args) => errors.push(args.map(String).join(" "));

class Node_ {
  constructor() { this.parentNode = null; this.childNodes = []; this.listeners = []; }
  get parentElement() { return this.parentNode instanceof HTMLElement ? this.parentNode : null; }
  get isConnected() { let n = this; while (n.parentNode) n = n.parentNode; return n === document; }
  get nextSibling() { const p = this.parentNode; return p ? p.childNodes[p.childNodes.indexOf(this) + 1] ?? null : null; }
  remove() { if (this.parentNode) { const s = this.parentNode.childNodes; s.splice(s.indexOf(this), 1); this.parentNode = null; } }
  before(...nodes) { const p = this.parentNode; for (const n of nodes) { n.remove(); n.parentNode = p; p.childNodes.splice(p.childNodes.indexOf(this), 0, n); } }
  after(...nodes) { const next = this.nextSibling; if (next) next.before(...nodes); else this.parentNode.append(...nodes); }
  replaceWith(node) { this.before(node); this.remove(); }
  addEventListener(type, fn, options) { this.listeners.push({ type, fn, capture: options === true || !!options?.capture, once: !!options?.once }); }
  removeEventListener(type, fn) { this.listeners = this.listeners.filter((l) => l.type !== type || l.fn !== fn); }
  dispatchEvent(event) {
    event.target = this;
    const path = [];
    for (let n = this.parentNode; n; n = n.parentNode) path.push(n);
    if (this !== window) path.push(window);
    const run = (node, capture) => {
      for (const l of [...node.listeners]) {
        if (l.type !== event.type || (capture !== null && l.capture !== capture)) continue;
        if (l.once) node.removeEventListener(l.type, l.fn);
        event.currentTarget = node;
        l.fn.call(node, event);
        if (event.stopped) return false;
      }
      return true;
    };
    for (const node of [...path].reverse()) if (!run(node, true)) return !event.defaultPrevented;
    if (!run(this, null)) return !event.defaultPrevented;
    if (event.bubbles) for (const node of path) if (!run(node, false)) break;
    return !event.defaultPrevented;
  }
  get textContent() { return this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(value) { for (const c of [...this.childNodes]) c.remove(); if (value !== "") this.append(new Text_(String(value))); }
}
class Text_ extends Node_ { constructor(data) { super(); this.data = data; this.nodeType = 3; } get textContent() { return this.data; } set textContent(v) { this.data = String(v); } }
class Comment_ extends Node_ { constructor(data) { super(); this.data = data; this.nodeType = 8; } get textContent() { return ""; } }

class HTMLElement extends Node_ {
  constructor(tag) { super(); this.tagName = tag.toUpperCase(); this.attrs = new Map(); this.nodeType = 1; this.style = {}; }
  get localName() { return this.tagName.toLowerCase(); }
  getAttribute(name) { return this.attrs.has(name) ? this.attrs.get(name) : null; }
  setAttribute(name, value) { this.attrs.set(name, String(value)); }
  hasAttribute(name) { return this.attrs.has(name); }
  removeAttribute(name) { this.attrs.delete(name); }
  toggleAttribute(name, force) { const on = force === undefined ? !this.hasAttribute(name) : !!force; if (on) this.setAttribute(name, ""); else this.removeAttribute(name); return on; }
  get id() { return this.getAttribute("id") ?? ""; }
  get className() { return this.getAttribute("class") ?? ""; }
  set className(v) { this.setAttribute("class", v); }
  get classList() {
    const el = this;
    const list = () => el.className.split(/\s+/).filter(Boolean);
    return {
      contains: (c) => list().includes(c),
      add: (...cs) => { el.className = [...new Set([...list(), ...cs])].join(" "); },
      remove: (...cs) => { el.className = list().filter((x) => !cs.includes(x)).join(" "); },
      toggle: (c, force) => { const on = force ?? !list().includes(c); if (on) el.classList.add(c); else el.classList.remove(c); return on; },
    };
  }
  get dataset() {
    const el = this;
    const attr = (key) => "data-" + String(key).replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());
    return new Proxy({}, {
      get: (_, key) => (typeof key === "string" ? el.getAttribute(attr(key)) ?? undefined : undefined),
      set: (_, key, value) => { el.setAttribute(attr(key), value); return true; },
      deleteProperty: (_, key) => { el.removeAttribute(attr(key)); return true; },
      has: (_, key) => el.hasAttribute(attr(key)),
    });
  }
  get hidden() { return this.hasAttribute("hidden"); }
  set hidden(v) { this.toggleAttribute("hidden", !!v); }
  get disabled() { return this.hasAttribute("disabled"); }
  set disabled(v) { this.toggleAttribute("disabled", !!v); }
  get children() { return this.childNodes.filter((c) => c instanceof HTMLElement); }
  get firstElementChild() { return this.children[0] ?? null; }
  append(...nodes) { for (let n of nodes) { if (typeof n === "string") n = new Text_(n); n.remove(); n.parentNode = this; this.childNodes.push(n); } }
  prepend(...nodes) { const first = this.childNodes[0]; if (first) first.before(...nodes); else this.append(...nodes); }
  appendChild(n) { this.append(n); return n; }
  contains(n) { for (; n; n = n.parentNode) if (n === this) return true; return false; }
  set innerHTML(html) { for (const c of [...this.childNodes]) c.remove(); parseInto(this, html); }
  get innerHTML() { return this.childNodes.map(serialize).join(""); }
  focus() {}
  blur() {}
  scrollIntoView() {}
  getBoundingClientRect() { return { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 }; }
  descendants() { const out = []; const walk = (n) => { for (const c of n.children) { out.push(c); walk(c); } }; walk(this); return out; }
  querySelectorAll(selector) { return this.descendants().filter((e) => matches(e, selector, this)); }
  querySelector(selector) { return this.querySelectorAll(selector)[0] ?? null; }
  matches(selector) { return matches(this, selector, null); }
  closest(selector) { for (let n = this; n instanceof HTMLElement; n = n.parentNode) if (matches(n, selector, null)) return n; return null; }
  get elements() { return this.querySelectorAll("input, select, textarea, button"); }
}
class HTMLFormElement extends HTMLElement {
  submit() {
    const fields = {};
    for (const e of this.elements) {
      const name = e.getAttribute("name");
      if (name && !e.disabled && !(e instanceof HTMLButtonElement)) fields[name] = e.value;
    }
    submitted.push({ action: this.getAttribute("action"), fields });
  }
  requestSubmit() { this.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })); }
}
class HTMLInputElement extends HTMLElement {
  get value() { return this._value ?? this.getAttribute("value") ?? ""; }
  set value(v) { this._value = String(v); }
  get type() { return this.getAttribute("type") ?? "text"; }
}
class HTMLButtonElement extends HTMLElement { get type() { return this.getAttribute("type") ?? "submit"; } }
class HTMLTextAreaElement extends HTMLElement {
  get value() { return this._value ?? this.textContent; }
  set value(v) { this._value = String(v); }
}
class HTMLOptionElement extends HTMLElement { get value() { return this.getAttribute("value") ?? this.textContent; } set value(v) { this.setAttribute("value", v); } }
class HTMLSelectElement extends HTMLElement {
  get options() { return this.querySelectorAll("option"); }
  get value() { const o = this.options; const s = o.find((x) => x._selected) ?? o.find((x) => x.hasAttribute("selected")) ?? o[0]; return s ? s.value : ""; }
  set value(v) { for (const o of this.options) o._selected = o.value === v; }
}
class HTMLDetailsElement extends HTMLElement { get open() { return this.hasAttribute("open"); } set open(v) { this.toggleAttribute("open", !!v); } }
class HTMLStyleElement extends HTMLElement {}
const KINDS = { form: HTMLFormElement, input: HTMLInputElement, button: HTMLButtonElement, textarea: HTMLTextAreaElement,
  option: HTMLOptionElement, select: HTMLSelectElement, details: HTMLDetailsElement, style: HTMLStyleElement };
const make = (tag) => new (KINDS[tag.toLowerCase()] ?? HTMLElement)(tag);

// ---- Selektoren: Listen mit Komma, Nachfahre (Leerzeichen) und Kind (>), Tag, #id, .klasse, [a], [a=b], :scope, :checked.
function splitTop(text, sep) {
  const out = []; let depth = 0, quote = null, cur = "";
  for (const ch of text) {
    if (quote) { if (ch === quote) quote = null; cur += ch; continue; }
    if (ch === '"' || ch === "'") { quote = ch; cur += ch; continue; }
    if (ch === "[" || ch === "(") depth++;
    if (ch === "]" || ch === ")") depth--;
    if (ch === sep && depth === 0) { out.push(cur); cur = ""; continue; }
    cur += ch;
  }
  out.push(cur);
  return out;
}
function compound(el, part, scope) {
  const re = /(:scope)|#([\w-]+)|\.([\w-]+)|\[([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?\]|^([\w*-]+)|(:checked)/g;
  let m, used = 0;
  while ((m = re.exec(part))) {
    used += m[0].length;
    if (m[1] && el !== scope) return false;
    if (m[2] && el.id !== m[2]) return false;
    if (m[3] && !el.classList.contains(m[3])) return false;
    if (m[4]) { const v = m[5] ?? m[6] ?? m[7]; if (!el.hasAttribute(m[4]) || (v !== undefined && el.getAttribute(m[4]) !== v)) return false; }
    if (m[8] && m[8] !== "*" && el.localName !== m[8].toLowerCase()) return false;
    // Runde 23a: `:checked` (gesetzte Eigenschaft geht vor dem Attribut).
    if (m[9] && !(el.checked ?? el.hasAttribute("checked"))) return false;
  }
  if (used !== part.length) throw new Error("Selektor nicht nachgebaut: " + part);
  return true;
}
function matchesComplex(el, tokens, scope) {
  if (!compound(el, tokens[tokens.length - 1], scope)) return false;
  if (tokens.length === 1) return true;
  const comb = tokens[tokens.length - 2];
  if (comb === ">") return el.parentNode instanceof HTMLElement && matchesComplex(el.parentNode, tokens.slice(0, -2), scope);
  // Runde 23a: Nachfahre (Leerzeichen) – der Rest ist alles vor dem letzten Teil (vorher fiel der Vorfahre mit weg,
  // und kein Selektor „a b“ traf je).
  const rest = tokens.slice(0, -1);
  for (let p = el.parentNode; p instanceof HTMLElement; p = p.parentNode) if (matchesComplex(p, rest, scope)) return true;
  return false;
}
function matches(el, selector, scope) {
  return splitTop(selector, ",").some((one) => {
    const tokens = one.trim().replace(/\s*>\s*/g, " > ").split(/\s+/).filter(Boolean);
    return matchesComplex(el, tokens, scope);
  });
}

// ---- HTML-Leser für das wohlgeformte Server-HTML.
const VOID = new Set(["input", "br", "img", "meta", "link", "hr", "source", "wbr"]);
const RAW = new Set(["style", "script", "textarea"]);
function decode(text) {
  return text.replace(/&(#x[0-9a-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/gi, (_, e) => {
    if (e[0] === "#") return String.fromCodePoint(e[1] === "x" || e[1] === "X" ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
    return { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " }[e.toLowerCase()];
  });
}
function parseInto(root, html) {
  let at = 0, cur = root;
  while (at < html.length) {
    if (html.startsWith("<!--", at)) { const end = html.indexOf("-->", at); cur.append(new Comment_(html.slice(at + 4, end))); at = end + 3; continue; }
    if (html.startsWith("</", at)) {
      const end = html.indexOf(">", at); const tag = html.slice(at + 2, end).trim().toLowerCase();
      for (let n = cur; n !== root; n = n.parentNode) if (n.localName === tag) { cur = n.parentNode; break; }
      at = end + 1; continue;
    }
    if (html[at] === "<" && /[a-zA-Z]/.test(html[at + 1] ?? "")) {
      const m = /^<([a-zA-Z][\w-]*)/.exec(html.slice(at)); const tag = m[1].toLowerCase(); at += m[0].length;
      const el = make(tag);
      const attr = /^\s*([^\s=>\/]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/;
      for (;;) {
        const rest = html.slice(at); const close = /^\s*\/?>/.exec(rest);
        if (close) { at += close[0].length; break; }
        const a = attr.exec(rest); if (!a) throw new Error("HTML nicht lesbar bei " + rest.slice(0, 40));
        el.setAttribute(a[1].toLowerCase(), decode(a[2] ?? a[3] ?? a[4] ?? "")); at += a[0].length;
      }
      cur.append(el);
      if (RAW.has(tag)) {
        const end = html.toLowerCase().indexOf("</" + tag, at); let text = html.slice(at, end);
        if (tag === "textarea") { if (text.startsWith("\n")) text = text.slice(1); text = decode(text); }
        if (text) el.append(new Text_(text));
        at = html.indexOf(">", end) + 1; continue;
      }
      if (!VOID.has(tag)) cur = el;
      continue;
    }
    const next = html.indexOf("<", at + 1); const end = next < 0 ? html.length : next;
    cur.append(new Text_(decode(html.slice(at, end)))); at = end;
  }
}
function serialize(n) {
  if (n instanceof Text_) return n.data.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  if (n instanceof Comment_) return "<!--" + n.data + "-->";
  const attrs = [...n.attrs].map(([k, v]) => (v === "" ? " " + k : ` ${k}="${v.replace(/"/g, "&quot;")}"`)).join("");
  return `<${n.localName}${attrs}>` + (VOID.has(n.localName) ? "" : n.childNodes.map(serialize).join("") + `</${n.localName}>`);
}

class Event { constructor(type, init = {}) { this.type = type; this.bubbles = !!init.bubbles; this.cancelable = !!init.cancelable; this.defaultPrevented = false; this.stopped = false; }
  preventDefault() { if (this.cancelable) this.defaultPrevented = true; } stopPropagation() { this.stopped = true; } stopImmediatePropagation() { this.stopped = true; } }
class CustomEvent extends Event { constructor(type, init = {}) { super(type, init); this.detail = init.detail ?? null; } }

const document = new HTMLElement("#document");
Object.defineProperty(document, "parentNode", { value: null, writable: true });
const html = make("html"), head = make("head"), body = make("body");
html.append(head, body); document.append(html);
document.documentElement = html; document.head = head; document.body = body;
let cookie = scenario.cookie ?? "";
Object.defineProperty(document, "cookie", { get: () => cookie, set: (v) => { const pair = String(v).split(";")[0]; const name = pair.split("=")[0]; cookie = [...cookie.split(";").map((c) => c.trim()).filter((c) => c && !c.startsWith(name + "=")), pair].join("; "); } });
document.createElement = make;
document.createComment = (d) => new Comment_(d);
document.createTextNode = (d) => new Text_(d);
document.getElementById = (id) => document.querySelector("#" + id);
document.getSelection = () => ({ rangeCount: 0, anchorNode: null, removeAllRanges() {}, addRange() {} });
document.execCommand = () => false;
const window = new Node_();
window.location = { hash: "", href: "http://127.0.0.1:8765/", pathname: "/", search: "", assign() {} };
Object.assign(globalThis, { window, document, HTMLElement, HTMLFormElement, HTMLInputElement, HTMLButtonElement,
  HTMLTextAreaElement, HTMLSelectElement, HTMLOptionElement, HTMLDetailsElement, HTMLStyleElement, Event, CustomEvent,
  Node: Node_, Text: Text_, Comment: Comment_, Element: HTMLElement,
  MutationObserver: class { observe() {} disconnect() {} takeRecords() { return []; } },
  ResizeObserver: class { observe() {} disconnect() {} unobserve() {} },
  requestAnimationFrame: (fn) => setTimeout(fn, 0), getComputedStyle: () => ({ getPropertyValue: () => "" }),
  matchMedia: () => ({ matches: false, addEventListener() {} }), confirm: () => true, alert: () => {},
  localStorage: { getItem: () => null, setItem() {}, removeItem() {} } });
globalThis.fetch = async (url, init) => {
  const request = JSON.parse(init?.body ?? "{}");
  calls.push(request.method ?? String(url));
  const answers = scenario.answers ?? {};
  const result = answers[request.method] ?? {};
  return { ok: true, status: 200, json: async () => ({ jsonrpc: "2.0", id: request.id, result }) };
};
"#####;

/// Flow "typeset view as default" behind [`DOM`].
const SCENE: &str = r#####"
const pane = make("div"); pane.setAttribute("id", "pane-editor"); body.append(pane);
pane.innerHTML = scenario.html;
const dir = process.argv[3];
const widgets = await import(dir + "/widgets.js");
await import(dir + "/proposals.js");
await import(dir + "/sheet.js");
await import(dir + "/wysiwyg.js");
widgets.mount(pane);
const settle = async () => { for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 0)); };
const visible = (el) => { for (let n = el; n instanceof HTMLElement; n = n.parentNode) if (n.hidden) return false; return !!el; };
const root = pane.querySelector("div.wysiwyg");
const look = () => ({
  open: root.hasAttribute("data-ws-open"),
  areaHidden: pane.querySelector("textarea[name=body]").hidden,
  wrapHidden: pane.querySelector("[data-ws-wrap]").hidden,
  satzPressed: pane.querySelector("[data-ws-mode=satz]")?.getAttribute("aria-pressed") ?? null,
  flowHidden: pane.querySelector("section.pv-flow")?.hidden ?? null,
  placedInSheet: pane.querySelectorAll("[data-ws-sheet] > details.pv").length,
  allVisible: visible(pane.querySelector("[data-pv-all]")),
  allLabel: pane.querySelector("[data-pv-all]")?.textContent ?? null,
  firstPlace: pane.querySelector("select[data-image-place]")?.value ?? null,
  calls: [...calls],
});
const out = { mounted: look() };
await settle();
out.settled = look();
if (scenario.caret !== undefined) {
  const area = pane.querySelector("textarea[name=body]");
  area.selectionStart = scenario.caret;
  const form = pane.querySelector("form.image-insert-form");
  form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  await settle();
  out.submitted = submitted;
}
if (scenario.settingsCss !== undefined) {
  document.dispatchEvent(new CustomEvent("baruch:settings-changed", { detail: { page: "werk" } }));
  await settle();
  out.styleAfter = document.querySelector("style#ws-style")?.textContent ?? null;
  out.callsAfter = [...calls];
}
out.errors = errors;
process.stdout.write(JSON.stringify(out));
"#####;

/// Run the scene in the mock; `None` if Node is missing (then it is honestly skipped).
fn run_widgets(scenario: &Value) -> Option<Value> {
    run_harness(SCENE, scenario)
}

/// Round 23a: run [`DOM`] and behind it `scene` (JavaScript module; reads `scenario`, mounts widgets from
/// `process.argv[3]` and writes exactly one JSON to stdout) under Node. `None` if Node is missing – the caller then
/// reports „ÜBERSPRUNGEN“.
pub(crate) fn run_harness(scene: &str, scenario: &Value) -> Option<Value> {
    let node = std::process::Command::new("node").arg("--version").output().ok()?;
    if !node.status.success() {
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let js = dir.path().join("js");
    std::fs::create_dir(&js).unwrap();
    for (name, text) in baruch_server::SCRIPTS {
        std::fs::write(js.join(name), text).unwrap();
    }
    let harness = dir.path().join("harness.mjs");
    // Round 39a: the widgets read their texts from #baruch-i18n; the scenes assert the German wording.
    let catalog =
        serde_json::to_string(&baruch_server::i18n::widget_catalog_json(baruch_server::i18n::Lang::De)).unwrap();
    let catalog = format!(
        "{{ const byId = document.getElementById; document.getElementById = (id) => (id === \"baruch-i18n\" ? {{ textContent: {catalog} }} : byId(id)); }}"
    );
    std::fs::write(&harness, format!("{DOM}\n{catalog}\n{scene}")).unwrap();
    let file = dir.path().join("scenario.json");
    std::fs::write(&file, scenario.to_string()).unwrap();
    let output = std::process::Command::new("node")
        .arg(&harness)
        .arg(&file)
        .arg(format!("file://{}", js.display()))
        .output()
        .expect("node startet");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Nachbau scheitert: {}\n{stdout}", String::from_utf8_lossy(&output.stderr));
    Some(serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("Ausgabe des Nachbaus ist kein JSON ({e}): {stdout}")))
}

/// Behaviour test "typeset view as default" (backlog Round 8: so far only text places in `wysiwyg.js`): the real
/// widgets mount themselves into the editor HTML of the server. The behaviour is evidenced, not the source text:
/// - Default (without cookie): the typeset view is open at once on mounting (without the intermediate state source
///   text), without `wysiwyg.parse`; the pieces of the proposals are in the typeset view, the reading flow is hidden,
///   „Alle einklappen“ stays visible; „Bild einfügen“ offers „an der Cursorstelle“; after `baruch:settings-changed`
///   sheet.js fetches `work.sheet` and replaces the style.
/// - Cookie `quelltext`: text field visible, typeset view closed, no `wysiwyg.parse`; „Bild einfügen“ at the cursor
///   position sends `place=offset:<selectionStart>` and the text as `body`.
/// - Long section (no typeset view set in advance): the widget fetches the typeset view through `wysiwyg.parse` and
///   opens it afterwards, without showing the source text first.
#[tokio::test]
async fn satzbild_as_default_behaves_right_in_the_real_widgets_without_a_browser() {
    let server = common::open().await;
    let (work, key, rev) = book(&server).await;
    add_image(&server, work, "fenster.svg", "bilder/fenster.svg").await;
    let node = server.state.store.node_seq(work, key).await.unwrap();
    let proposal = "= Fenster\n\nDas Licht fällt schräg.\n\nDer Vorhang weht nicht.\n\nAm Rahmen blättert Farbe.\n";
    server.state.store.add_proposal(work, node, proposal, "@lektor", "probe-1", "").await.unwrap();
    let editor = |answer: &Value| ok(answer)["editor"].as_str().unwrap().to_owned();
    let satz = editor(&server.rpc("ui.render", json!({ "work": work, "key": key })).await);
    assert!(satz.contains(" data-ws-ready>"), "Szene Vorgabe: der Server liefert das Satzbild fertig");
    let new_css = ".ws-sheet { --ws-width: 18cm; }";
    let scenario = json!({ "html": satz, "cookie": "", "answers": { "work.sheet": { "css": new_css, "settings": {} },
        "wysiwyg.style": { "css": ".ws-sheet { }" } }, "settingsCss": new_css });
    let Some(seen) = run_widgets(&scenario) else {
        eprintln!("ÜBERSPRUNGEN: node fehlt – Verhaltenstest „Satzbild als Vorgabe“ nicht gelaufen.");
        return;
    };
    assert_eq!(seen["errors"], json!([]), "keine Fehler beim Einhängen der Widgets: {seen}");
    let mounted = &seen["mounted"];
    assert_eq!(mounted["open"], true, "Satzbild sofort beim Einhängen offen (kein Zwischenzustand Quelltext): {seen}");
    assert_eq!(mounted["areaHidden"], true, "das Textfeld ist von Anfang an verborgen: {seen}");
    assert_eq!(mounted["wrapHidden"], false, "das Blatt des Satzbilds ist sichtbar: {seen}");
    assert_eq!(mounted["satzPressed"], "true", "„Satzbild“ gedrückt: {seen}");
    assert_eq!(mounted["placedInSheet"], 1, "das Stück des Vorschlags steht im Satzbild: {seen}");
    assert_eq!(mounted["flowHidden"], true, "der Lesefluss ist verborgen: {seen}");
    assert_eq!(mounted["allVisible"], true, "„Alle einklappen“ bleibt bei offenem Satzbild sichtbar: {seen}");
    assert_eq!(mounted["allLabel"], "Alle einklappen", "Beschriftung nach dem Zustand (Stück offen): {seen}");
    assert_eq!(mounted["firstPlace"], "cursor", "„Bild einfügen“ bietet zuerst die Cursorstelle: {seen}");
    let calls: Vec<String> = serde_json::from_value(seen["settled"]["calls"].clone()).unwrap();
    assert!(!calls.iter().any(|c| c == "wysiwyg.parse"), "kein wysiwyg.parse, das Satzbild kam vom Server: {calls:?}");
    assert_eq!(seen["settled"]["open"], true, "das Satzbild bleibt offen: {seen}");
    assert_eq!(seen["styleAfter"], new_css, "nach baruch:settings-changed ersetzt sheet.js den Stil: {seen}");
    let after: Vec<String> = serde_json::from_value(seen["callsAfter"].clone()).unwrap();
    assert!(after.iter().any(|c| c == "work.sheet"), "dafür fragt es work.sheet: {after:?}");

    // Source text on request (cookie).
    let request = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, "baruch_editor=quelltext")
        .body(Body::from(
            json!({ "jsonrpc": "2.0", "id": 1, "method": "ui.render", "params": { "work": work, "key": key } })
                .to_string(),
        ))
        .unwrap();
    let (_, _, text) = server.send(request).await;
    let source = editor(&serde_json::from_str(&text).unwrap());
    let caret = "= Fenster\n\nDas Licht fällt schräg.\n\n".encode_utf16().count();
    let scenario = json!({ "html": source, "cookie": "baruch_editor=quelltext", "caret": caret });
    let seen = run_widgets(&scenario).expect("node war eben noch da");
    assert_eq!(seen["errors"], json!([]), "keine Fehler: {seen}");
    assert_eq!(seen["mounted"]["open"], false, "Quelltext gewünscht: Satzbild zu: {seen}");
    assert_eq!(seen["mounted"]["areaHidden"], false, "Textfeld sichtbar: {seen}");
    assert_eq!(seen["mounted"]["satzPressed"], "false", "„Satzbild“ nicht gedrückt: {seen}");
    let calls: Vec<String> = serde_json::from_value(seen["settled"]["calls"].clone()).unwrap();
    assert!(!calls.iter().any(|c| c == "wysiwyg.parse"), "im Quelltext kein wysiwyg.parse: {calls:?}");
    let sent = &seen["submitted"][0];
    assert_eq!(sent["action"], "/werke/bilder/einfuegen", "das Formular wird abgeschickt: {seen}");
    assert_eq!(sent["fields"]["place"], format!("offset:{caret}"), "Stelle aus selectionStart: {seen}");
    assert_eq!(sent["fields"]["body"], current_body(&server, work, key).await.1, "der Text geht als body mit: {seen}");
    assert_eq!(sent["fields"]["rev"], rev.to_string(), "auf der gezeigten Fassung: {seen}");

    // Long section: without a typeset view set in advance the widget fetches it through RPC.
    let parsed =
        server.rpc("wysiwyg.parse", json!({ "work": work, "body": current_body(&server, work, key).await.1 })).await;
    let html = ok(&parsed)["html"].as_str().unwrap().to_owned();
    let unready = satz.replacen(" data-ws-ready>", ">", 1);
    let unready = unready.replacen(&html, "", 1);
    assert!(!unready.contains("data-ws-ready"), "Szene „lang“: kein fertiges Satzbild im HTML");
    let scenario = json!({ "html": unready, "cookie": "", "answers": { "wysiwyg.parse": { "html": html },
        "wysiwyg.style": { "css": ".ws-sheet { }" } } });
    let seen = run_widgets(&scenario).expect("node war eben noch da");
    assert_eq!(seen["errors"], json!([]), "keine Fehler: {seen}");
    assert_eq!(seen["mounted"]["open"], false, "vor der Antwort ist das Satzbild noch zu: {seen}");
    assert_eq!(seen["mounted"]["areaHidden"], true, "…aber der Quelltext wird nicht erst gezeigt: {seen}");
    assert_eq!(seen["mounted"]["satzPressed"], "true", "„Satzbild“ bleibt gedrückt (kein Umschalten): {seen}");
    let calls: Vec<String> = serde_json::from_value(seen["settled"]["calls"].clone()).unwrap();
    assert!(calls.iter().any(|c| c == "wysiwyg.parse"), "das Widget holt das Satzbild per wysiwyg.parse: {calls:?}");
    assert_eq!(seen["settled"]["open"], true, "nach der Antwort ist das Satzbild offen: {seen}");
    assert_eq!(seen["settled"]["placedInSheet"], 1, "und das Stück steht darin: {seen}");
}
