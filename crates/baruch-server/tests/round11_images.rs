// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 11 („images“): images in the user interface. Search on the page „Bilder“ (`?q=`, RPC `asset.search`),
//! thumbnails (`…/vorschau?b=`), places for cover and requests (hooks until the integration), button „Als Umschlag“,
//! chapter image from the work template in the editor header, „Bild einfügen“ as a new revision of the human with one
//! commit and missing images in preview and typeset view. Real data folder in the temp folder; all images created in
//! the test (PNG through the image crate, SVG as text).

use crate::common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Invented SVG with one colour (different bytes per colour).
fn svg(fill: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"20\" height=\"10\"><rect fill=\"{fill}\" width=\"20\" height=\"10\"/></svg>\n"
    )
}

/// Valid PNG `width` × `height` with a colour gradient, created in the test.
fn png(width: u32, height: u32) -> Vec<u8> {
    let picture = image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 90]));
    let mut out = std::io::Cursor::new(Vec::new());
    picture.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

async fn add_svg(server: &common::Server, work: i64, file: &str, fill: &str, path: &str, caption: &str, alt: &str) {
    let answer = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": file, "content": svg(fill), "encoding": "svg", "path": path,
                    "caption": caption, "alt": alt }),
        )
        .await;
    result(&answer);
}

async fn send_raw(server: &common::Server, request: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

/// Nodes of a work from `work.get`.
async fn nodes(server: &common::Server, work: i64) -> Vec<Value> {
    result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].as_array().unwrap().clone()
}

fn current_rev(node: &Value) -> u64 {
    node["revisions"].as_array().unwrap().iter().find(|r| r["status"] == "current").unwrap()["number"].as_u64().unwrap()
}

#[tokio::test]
async fn the_images_page_searches_name_caption_and_alt_without_case() {
    let server = common::open().await;
    add_svg(&server, 1, "garten.svg", "green", "bilder/garten.svg", "Der Garten im Mai", "Beete mit Kohl").await;
    add_svg(&server, 1, "hof.svg", "gray", "bilder/hof.svg", "Rabatt 50% im Hofladen", "Ein Tor").await;
    add_svg(&server, 1, "teich.svg", "blue", "bilder/teich.svg", "", "Schilf am Ufer").await;

    let (status, page) = server.get("/werke/1/bilder?q=GARTEN").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("<form class=\"asset-search\" id=\"suche\" method=\"get\""), "Suchformular ohne JavaScript");
    assert!(page.contains("value=\"GARTEN\""), "Suche bleibt im Feld stehen");
    assert!(page.contains("<strong>garten.svg</strong>"), "Treffer nach Name (ohne Groß/Klein): {page}");
    assert!(!page.contains("<strong>hof.svg</strong>") && !page.contains("<strong>teich.svg</strong>"), "nur Treffer");
    assert!(page.contains("1 von 3 Bildern für „GARTEN“"), "wie viele von allen: {page}");
    let (_, page) = server.get("/werke/1/bilder?q=schilf").await;
    assert!(page.contains("<strong>teich.svg</strong>"), "Treffer nach Alternativtext: {page}");
    let (_, page) = server.get("/werke/1/bilder?q=im+mai").await;
    assert!(page.contains("<strong>garten.svg</strong>"), "Treffer nach Bildunterschrift");
    let (_, page) = server.get("/werke/1/bilder?q=%25").await;
    assert!(page.contains("<strong>hof.svg</strong>") && !page.contains("<strong>garten.svg</strong>"), "% wörtlich");
    let (_, page) = server.get("/werke/1/bilder?q=xyz").await;
    assert!(page.contains("Kein Bild passt zu „xyz“."), "ehrlich leer: {page}");

    let found = server.rpc("asset.search", json!({ "work": 1, "query": "tor" })).await;
    let names: Vec<&str> =
        result(&found)["assets"].as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["hof.svg"], "RPC asset.search findet über den Alternativtext: {found}");
    let all = server.rpc("asset.search", json!({ "work": 1, "query": "" })).await;
    assert_eq!(result(&all)["assets"].as_array().unwrap().len(), 3, "leere Suche = alle");
    let refused = server.rpc("asset.search", json!({ "work": 1 })).await;
    assert_eq!(refused["error"]["code"], -32602, "query fehlt → -32602: {refused}");
}

#[tokio::test]
async fn previews_are_small_valid_pngs_and_svgs_stay_sandboxed() {
    let server = common::open().await;
    let big = png(1000, 500);
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": 1, "file_name": "weit.png", "content": baruch_store::encode_base64(&big), "encoding": "base64" }),
        )
        .await;
    result(&added);
    let (status, headers, bytes) = send_raw(&server, get("/werke/1/bilder/weit.png/vorschau?b=64")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau erzeugt");
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "image/png", "PNG bleibt PNG");
    assert_eq!(headers.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff", "kein Raten des Typs");
    let thumb = image::load_from_memory(&bytes).expect("gültiges PNG");
    assert!(
        thumb.width() <= 64 && thumb.height() <= 64,
        "längste Seite höchstens 64: {}×{}",
        thumb.width(),
        thumb.height()
    );
    assert_eq!((thumb.width(), thumb.height()), (64, 32), "Seitenverhältnis 2:1 bleibt");
    assert!(bytes.len() < big.len(), "kleiner als das Original ({} < {})", bytes.len(), big.len());
    let etag = headers.get(header::ETAG).expect("ETag").to_str().unwrap().to_owned();
    assert!(etag.ends_with("-64\""), "ETag aus SHA-256 und b: {etag}");
    let again = Request::get("/werke/1/bilder/weit.png/vorschau?b=64")
        .header(header::IF_NONE_MATCH, &etag)
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = send_raw(&server, again).await;
    assert_eq!((status, body.len()), (StatusCode::NOT_MODIFIED, 0), "gleiches ETag → 304 ohne Inhalt");

    for bad in ["63", "1025", "0", "viel", "-5"] {
        let (status, _, _) = send_raw(&server, get(&format!("/werke/1/bilder/weit.png/vorschau?b={bad}"))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "b={bad} liegt nicht in 64–1024 → 400");
    }
    let (status, _, bytes) = send_raw(&server, get("/werke/1/bilder/weit.png/vorschau")).await;
    assert_eq!(status, StatusCode::OK, "ohne b die Vorgabe 320");
    let thumb = image::load_from_memory(&bytes).unwrap();
    assert_eq!(thumb.width(), 320, "Vorgabe 320 Pixel");
    let (status, _, _) = send_raw(&server, get("/werke/1/bilder/fehlt.png/vorschau?b=64")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Bild → 404");

    add_svg(&server, 1, "haus.svg", "red", "bilder/haus.svg", "", "").await;
    let (status, headers, bytes) = send_raw(&server, get("/werke/1/bilder/haus.svg/vorschau?b=64")).await;
    assert_eq!(status, StatusCode::OK, "SVG-Vorschau wird ausgeliefert");
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "image/svg+xml", "SVG wie /datei");
    assert!(
        headers.get(header::CONTENT_SECURITY_POLICY).unwrap().to_str().unwrap().contains("sandbox"),
        "SVG nur mit CSP sandbox (kein Skript)"
    );
    assert_eq!(bytes, svg("red").as_bytes(), "SVG unverändert");

    let (_, page) = server.get("/werke/1/bilder").await;
    assert!(
        page.contains("<img src=\"/werke/1/bilder/weit.png/vorschau?b=320\" alt=\"\" loading=\"lazy\">"),
        "die Liste lädt Vorschaubilder, verzögert: {page}"
    );
}

#[tokio::test]
async fn the_images_page_has_places_for_cover_and_requests_and_a_cover_button_per_row() {
    let server = common::open().await;
    add_svg(&server, 1, "umschlag.svg", "navy", "bilder/umschlag.svg", "Umschlag", "Ein Garten").await;
    let (_, page) = server.get("/werke/1/bilder").await;
    assert!(
        page.contains(
            "<form method=\"post\" action=\"/werke/umschlag/act\"><input type=\"hidden\" name=\"work\" value=\"1\">\
             <input type=\"hidden\" name=\"action\" value=\"set\"><input type=\"hidden\" name=\"name\" value=\"umschlag.svg\">\
             <input type=\"hidden\" name=\"back\" value=\"/werke/1/bilder\"><button class=\"btn\">Als Umschlag</button></form>"
        ),
        "Knopf genau nach dem Vertrag (bedient von „cover“): {page}"
    );
    assert!(
        !page.contains("<span class=\"badge primary\">Umschlag</span>"),
        "ohne Umschlag (Haken: None) kein Abzeichen"
    );
    let (_, detail) = server.get("/werke/1/bilder/umschlag.svg").await;
    assert!(detail.contains("<button class=\"btn\">Als Umschlag</button>"), "derselbe Knopf auf der Detailseite");
    for (code, text) in [
        ("umschlag_gesetzt", "Das Bild ist jetzt der Umschlag des Werks."),
        ("umschlag_entfernt", "Das Werk hat keinen Umschlag mehr."),
        ("antrag_bestaetigt", "Der Antrag ist bestätigt und ausgeführt."),
        ("antrag_abgelehnt", "Der Antrag ist abgelehnt."),
        ("antrag_veraltet", "Der Antrag ist veraltet"),
    ] {
        let (_, page) = server.get(&format!("/werke/1/bilder?hinweis={code}")).await;
        assert!(page.contains(text), "Hinweis für {code}: {page}");
    }
    let template = include_str!("../templates/assets.html");
    let cover = template.find("{{ cover_panel|safe }}").expect("Platz für den Umschlag-Block");
    let requests = template.find("{{ requests_panel|safe }}").expect("Platz für die Anträge");
    let table = template.find("<table").unwrap();
    assert!(cover < table && requests > table, "Umschlag oben, Anträge unten");
    // Since the integration the hooks are wired: the cover block (id="umschlag") is at the top, the requests
    // (id="antraege") at the bottom (end to end in tests/round11.rs).
    let at_cover = page.find("id=\"umschlag\"").expect("Umschlag-Block auf der Seite");
    let at_requests = page.find("id=\"antraege\"").expect("Anträge-Block auf der Seite");
    assert!(at_cover < at_requests, "Umschlag oben, Anträge unten: {page}");
    let source = include_str!("../src/assets.rs");
    assert!(!source.contains("VERTRAG R11"), "kein offener Haken mehr in assets.rs");
}

/// Invented work template: chapter images from a literal list, index = chapter number − 1. The template numbers the
/// headings; only then does Typst count them in `counter(heading)`.
const CHAPTERS: &str = "#set document(title: \"Tageszeiten\", author: \"Erika Erfunden\")\n\
    #set heading(numbering: \"1.\")\n\
    #let kapitelbilder = (\"bilder/morgen.svg\", \"bilder/mittag.svg\")\n\
    #show heading.where(level: 1): it => context {\n  \
    image(kapitelbilder.at(counter(heading).get().first() - 1))\n  it\n}\n\n\
    = Morgen\n\nDer Tau liegt.\n\n= Mittag\n\nDie Sonne steht hoch.\n\n= Abend\n\nEs wird still.\n";

/// Header of the editor for a node (up to the title).
async fn editor_head(server: &common::Server, work: i64, key: i64) -> String {
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    page[page.find("class=\"editor-head\"").unwrap()..page.find("class=\"node-title\"").unwrap()].to_owned()
}

/// Follow-up fix Round 11: Typst counts only numbered headings in `counter(heading)`
/// (`baruch-render/tests/frame_pages.rs`, `typst_counts_only_numbered_headings`). The editor header must name the same
/// image as the typesetting – also after a preface with „Nummer aus“ and with a template without numbering (there the
/// counter is 0 and `at(0 - 1)` the last image).
#[tokio::test]
async fn the_chapter_image_follows_typsts_heading_counter() {
    let server = common::open().await;
    let source = CHAPTERS.replace("= Morgen", "= Vorwort\n\nVorab.\n\n= Morgen").replace("Tageszeiten", "Mit Vorwort");
    let work = server.import(&source, "vorwort.typ").await.work;
    add_svg(&server, work, "morgen.svg", "white", "bilder/morgen.svg", "Tau", "Helle Fläche").await;
    add_svg(&server, work, "mittag.svg", "gold", "bilder/mittag.svg", "Sonne", "Gelbe Scheibe").await;
    let list = nodes(&server, work).await;
    let key = |title: &str| list.iter().find(|n| n["title"] == title).unwrap()["key"].as_i64().unwrap();
    let morning = "alt=\"Helle Fläche\"";
    let noon = "alt=\"Gelbe Scheibe\"";

    // Scheme „aus der Werkvorlage“: the template numbers all headings. Since Round 14 (contract §4) Baruch projects
    // „Nummer aus“ as the marking `numbering: none` here too; Typst therefore does not count the preface (like Baruch:
    // Morgen 1), in the preface the counter is at 0 → `at(0 - 1)` = last image.
    let off =
        server.rpc("outline.set_numbered", json!({ "work": work, "key": key("Vorwort"), "numbered": false })).await;
    result(&off);
    let export = server.state.store.export_typst(work).await.unwrap();
    assert!(
        export.contains("#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>"),
        "Markierung für das Vorwort auch bei „aus der Werkvorlage“: {export}"
    );
    assert!(editor_head(&server, work, key("Morgen")).await.contains(morning), "Morgen ist Kapitel 1 → Bild 1");
    assert!(editor_head(&server, work, key("Vorwort")).await.contains(noon), "Vorwort: Zähler 0 → letztes Bild");

    // Own scheme: the same marking, Typst does not count the preface.
    result(&server.rpc("numbering.set", json!({ "work": work, "scheme": "decimal" })).await);
    let export = server.state.store.export_typst(work).await.unwrap();
    assert!(
        export.contains("#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered>"),
        "Markierung statt Show-Regel: {export}"
    );
    assert!(!export.contains("body: [Vorwort]): set heading(numbering: none)"), "keine Show-Regel mehr: {export}");
    assert!(editor_head(&server, work, key("Morgen")).await.contains(morning), "Morgen ist Kapitel 1 → Bild 1");
    assert!(editor_head(&server, work, key("Mittag")).await.contains(noon), "Mittag ist Kapitel 2 → Bild 2");
    assert!(editor_head(&server, work, key("Vorwort")).await.contains(noon), "Vorwort: Zähler 0 → letztes Bild");

    // Template without numbering: Typst counts no heading, every chapter shows the last image.
    let plain = CHAPTERS.replace("#set heading(numbering: \"1.\")\n", "").replace("Tageszeiten", "Ohne Nummern");
    let other = server.import(&plain, "ohne.typ").await.work;
    add_svg(&server, other, "mittag.svg", "gold", "bilder/mittag.svg", "Sonne", "Gelbe Scheibe").await;
    let list = nodes(&server, other).await;
    let first = list.iter().find(|n| n["title"] == "Morgen").unwrap()["key"].as_i64().unwrap();
    assert!(editor_head(&server, other, first).await.contains(noon), "ohne Nummerierung: Zähler 0 → letztes Bild");
}

#[tokio::test]
async fn the_chapter_image_of_the_template_appears_in_the_head_of_its_chapter() {
    let server = common::open().await;
    let report = server.import(CHAPTERS, "tageszeiten.typ").await;
    let work = report.work;
    add_svg(&server, work, "mittag.svg", "gold", "bilder/mittag.svg", "Sonne über dem Feld", "Gelbe Scheibe").await;
    let list = nodes(&server, work).await;
    let key = |title: &str| list.iter().find(|n| n["title"] == title).unwrap()["key"].as_i64().unwrap();

    let (_, page) = server.get(&format!("/?work={work}&key={}", key("Mittag"))).await;
    let head = &page[page.find("class=\"editor-head\"").unwrap()..page.find("class=\"node-title\"").unwrap()];
    assert!(
        head.contains(&format!("<img src=\"/werke/{work}/bilder/mittag.svg/vorschau?b=320\" alt=\"Gelbe Scheibe\"")),
        "Kapitel 2 zeigt das zweite Bild der Liste im Kopf: {head}"
    );
    assert!(head.contains("Sonne über dem Feld"), "Bildunterschrift sichtbar: {head}");
    assert!(head.contains("Alternativtext: Gelbe Scheibe"), "Alternativtext sichtbar: {head}");

    let (_, page) = server.get(&format!("/?work={work}&key={}", key("Morgen"))).await;
    assert!(
        page.contains("Kapitelbild fehlt: <code>bilder/morgen.svg</code>"),
        "Kapitel 1: Pfad bekannt, aber kein Bild im Werk – deutlich gemeldet: {page}"
    );
    let (_, page) = server.get(&format!("/?work={work}&key={}", key("Abend"))).await;
    assert!(
        page.contains("Bild aus der Vorlage · <a href=\"/werke/") && page.contains("/bilder\">Bilder</a></p>"),
        "Kapitel 3 liegt hinter der Liste: nicht raten, Hinweis mit Link zur Seite Bilder: {page}"
    );

    let unknown = CHAPTERS
        .replace("kapitelbilder.at(counter(heading).get().first() - 1)", "bild-zu(it)")
        .replace("Tageszeiten", "Berechnet");
    let other = server.import(&unknown, "berechnet.typ").await.work;
    let first = nodes(&server, other).await[0]["key"].as_i64().unwrap();
    let (_, page) = server.get(&format!("/?work={other}&key={first}")).await;
    assert!(page.contains("chapter-image-hint"), "berechnet ohne Liste → Hinweis „Bild aus der Vorlage“: {page}");
    let (_, plain) = server.get("/?work=1").await;
    assert!(!plain.contains("chapter-image"), "ohne berechnete Bilder in der Vorlage kein Kapitelbild-Block");
}

#[tokio::test]
async fn inserting_an_image_makes_one_human_revision_with_a_figure_and_one_commit() {
    let server = common::open().await;
    add_svg(&server, 1, "laube.svg", "olive", "", "Die Laube", "Holz & <Wein>").await;
    let section = nodes(&server, 1).await.into_iter().find(|n| n["kind"] == "section").expect("ein Abschnitt");
    let (key, base) = (section["key"].as_i64().unwrap(), current_rev(&section));
    let mirror = server.mirror_dir();
    let count = |dir: &std::path::Path| {
        common::git(dir, &["rev-list", "--count", "HEAD"]).and_then(|c| c.trim().parse::<u32>().ok()).unwrap_or(0)
    };
    let commits_before = count(&mirror);

    let body = format!("work=1&key={key}&rev={base}&name=laube.svg&caption=Ein+%23Test+%5Bmit%5D+*Zeichen*&place=end");
    let (status, location) = server.post_form("/werke/bilder/einfuegen", &body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "eingefügt → 303");
    assert_eq!(location.as_deref(), Some(format!("/?work=1&key={key}").as_str()), "zurück zum Abschnitt");
    let section = nodes(&server, 1).await.into_iter().find(|n| n["key"] == key).unwrap();
    let revisions = section["revisions"].as_array().unwrap();
    let newest = revisions.iter().find(|r| r["status"] == "current").unwrap();
    assert!(newest["number"].as_u64().unwrap() > base, "eine neue Fassung ist aktuell");
    assert_eq!(newest["author"]["kind"], "human", "angelegt als Mensch: {newest}");
    let text = newest["body"].as_str().unwrap();
    assert!(
        text.ends_with(
            "\n\n#figure(image(\"bilder/laube.svg\", alt: \"Holz & <Wein>\"), caption: [Ein \\#Test \\[mit\\] \\*Zeichen\\*])\n"
        ),
        "Abbildung am Ende, Nutzertext maskiert (kein Markup aus # [ ] *): {text}"
    );
    let asset = result(&server.rpc("asset.list", json!({ "work": 1 })).await)["assets"][0].clone();
    assert_eq!(asset["paths"], json!(["bilder/laube.svg"]), "fehlender Pfad vergeben: {asset}");
    if common::git(&mirror, &["--version"]).is_some() {
        assert!(commits_before > 0, "das Hinzufügen des Bilds hat schon einen Commit gemacht");
        assert_eq!(count(&mirror), commits_before + 1, "genau ein Commit für Fassung und Pfad");
        let subject = common::git(&mirror, &["log", "-1", "--format=%an"]).unwrap_or_default();
        assert!(!subject.trim().starts_with('@'), "Autor des Commits ist der Mensch, keine Persona: {subject}");
    }

    for (body, status, why) in [
        (
            format!("work=1&key={key}&rev={base}&name=gibtsnicht.svg"),
            StatusCode::UNPROCESSABLE_ENTITY,
            "unbekanntes Bild",
        ),
        (format!("work=1&key={key}&rev=999&name=laube.svg"), StatusCode::UNPROCESSABLE_ENTITY, "unbekannte Fassung"),
        (
            format!("work=1&key={key}&rev={base}&name=laube.svg&place=anfang"),
            StatusCode::UNPROCESSABLE_ENTITY,
            "Stelle",
        ),
        (format!("work=1&key={key}&rev={base}"), StatusCode::UNPROCESSABLE_ENTITY, "kein Bild gewählt"),
    ] {
        let (got, _) = server.post_form("/werke/bilder/einfuegen", &body).await;
        assert_eq!(got, status, "{why}");
    }
    let request = Request::post("/werke/bilder/einfuegen")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::from(format!("work=1&key={key}&rev={base}&name=laube.svg")))
        .unwrap();
    let (status, _, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft abgelehnt (guard::same_origin)");

    let (_, page) = server.get(&format!("/?work=1&key={key}")).await;
    assert!(
        page.contains("<form class=\"image-insert-form\" method=\"post\" action=\"/werke/bilder/einfuegen\">"),
        "Formular „Bild einfügen“ im Editor, ohne JavaScript: {page}"
    );
    assert!(page.contains("<option value=\"laube.svg\">laube.svg – Die Laube</option>"), "Auswahl der Werkbilder");
}

#[tokio::test]
async fn missing_images_are_reported_in_preview_and_satzbild() {
    let server = common::open().await;
    let section = nodes(&server, 1).await.into_iter().find(|n| n["kind"] == "section").unwrap();
    let key = section["key"].as_i64().unwrap();
    let saved = server
        .rpc(
            "revision.save",
            json!({ "work": 1, "key": key, "body": "== Probe\n\nText.\n\n#figure(image(\"bilder/fehlt.png\"), caption: [Weg])\n" }),
        )
        .await;
    result(&saved);
    let (_, page) = server.get(&format!("/?work=1&key={key}&view=preview")).await;
    assert!(
        page.contains("class=\"notice warning missing-images\" role=\"alert\"")
            && page.contains("Bild fehlt: <code>bilder/fehlt.png</code>"),
        "Vorschau meldet das fehlende Bild deutlich: {page}"
    );
    let parsed = server
        .rpc("wysiwyg.parse", json!({ "work": 1, "body": "#figure(image(\"bilder/fehlt.png\"), caption: [Weg])\n" }))
        .await;
    let html = result(&parsed)["html"].as_str().unwrap().to_owned();
    assert!(
        html.contains("ws-missing") && html.contains("Bild fehlt: <code>bilder/fehlt.png</code>"),
        "Satzbild: {html}"
    );
    add_svg(&server, 1, "fehlt.svg", "teal", "bilder/fehlt.png", "", "").await;
    let (_, page) = server.get(&format!("/?work=1&key={key}&view=preview")).await;
    assert!(!page.contains("Bild fehlt:"), "mit Bild zum Pfad keine Meldung mehr");
}
