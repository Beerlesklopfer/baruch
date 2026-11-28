// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 20b (reader, ownership „reader“): EPUB preview per reader, `epub.readers`, `epub.preview`, reader files with a
//! CSP of their own (exception in `guard::no_framing`) and reader CSS. All test texts freely invented
//! („Das Vogelhaus“).

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_epub::readers::{PROFILES, profile};
use baruch_epub::{EpubItem, ItemKind};
use baruch_server::epub_preview::{NIGHT_MARK, reader_css, reader_file};
use serde_json::{Value, json};

/// Freely invented work with preface, two parts, three chapters (one with a subsection) and a footnote.
fn birdhouse() -> String {
    format!(
        "#set text(lang: \"de\")\n\n\
         = Vorwort\nEin Brett, ein Nagel, ein Loch.\n\n\
         {}\n\n\
         = Das Holz\nFichte ist weich, Lärche hält länger.#footnote[Lärche braucht keinen Anstrich.]\n\n\
         == Die Bretter\nSechs Bretter, gleich dick.\n\n\
         = Das Dach\nDas Dach steht über, damit der Regen abläuft.\n\n\
         {}\n\n\
         = Der Platz\nIn zwei Metern Höhe, mit dem Loch nach Osten.\n",
        baruch_typst::part_line("Bauen"),
        baruch_typst::part_line("Aufhängen"),
    )
}

struct Book {
    server: common::Server,
    work: i64,
    files: Vec<Value>,
}

/// Create the work and fetch the files with text (reading order) through `work.epub`.
async fn book() -> Book {
    let server = common::open().await;
    let work = server.import(&birdhouse(), "vogelhaus.typ").await.work;
    let answer = server.rpc("work.epub", json!({ "work": work })).await;
    assert!(answer["error"].is_null(), "das EPUB entsteht: {answer}");
    let files = answer["result"]["files"].as_array().unwrap().clone();
    Book { server, work, files }
}

fn href_of<'a>(files: &'a [Value], title: &str) -> &'a str {
    files
        .iter()
        // Since R20a titles carry the number from the outline scheme („2 Das Holz“, „I Bauen“).
        .find(|f| f["title"] == title || f["title"].as_str().is_some_and(|t| t.ends_with(&format!(" {title}"))))
        .and_then(|f| f["href"].as_str())
        .unwrap_or_else(|| panic!("{title} fehlt in {files:?}"))
}

async fn get_with(
    server: &common::Server,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, header::HeaderMap, String) {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    server.send(request.body(Body::empty()).unwrap()).await
}

fn header_of(headers: &header::HeaderMap, name: header::HeaderName) -> &str {
    headers.get(&name).and_then(|v| v.to_str().ok()).unwrap_or("")
}

#[tokio::test]
async fn the_preview_without_parameters_shows_the_required_readers_the_chapter_boundaries_and_the_kindle_hint() {
    let Book { server, work, files } = book().await;
    let (status, html) = server.get(&format!("/werke/{work}/epub/vorschau")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau lädt: {html}");
    // The preface is a chapter (level 1) before the first part: the first chapter file.
    let first_part = href_of(&files, "Vorwort");
    for p in PROFILES {
        let figure = format!("<figure class=\"epub-frame\" data-reader=\"{}\">", p.slug);
        assert_eq!(html.contains(&figure), p.required, "{}: ohne Wahl genau die Pflicht-Profile", p.slug);
        if p.required {
            let src = format!("<iframe src=\"/werke/{work}/epub/leser/{}/{first_part}\"", p.slug);
            assert!(html.contains(&src), "{}: iframe auf die erste Teil-/Kapiteldatei: {html}", p.slug);
            assert!(
                html.contains(&format!("width=\"{}\" height=\"{}\"", p.width_px, p.height_px)),
                "{}: Rahmen in der Größe des Geräts",
                p.slug
            );
        }
    }
    assert!(html.contains("Send to Kindle nimmt EPUB an"), "Hinweis nach Vertrag §0.4: {html}");
    // List of chapter boundaries: every file with text, title from EpubBook.items, part/chapter marked.
    let list = &html[html.find("data-epub-files").expect("Dateiliste")..];
    let list = &list[..list.find("</ol>").unwrap()];
    for f in &files {
        let title = f["title"].as_str().unwrap();
        assert!(list.contains(title), "{title} steht in der Kapitelgrenzen-Liste: {list}");
    }
    for (title, kind) in [("Bauen", "Teil"), ("Das Holz", "Kapitel"), ("Aufhängen", "Teil"), ("Der Platz", "Kapitel")]
    {
        let href = href_of(&files, title);
        let item = format!(
            "<li class=\"epub-file boundary\" data-href=\"{href}\">\n        <span class=\"epub-kind\">{kind}</span>"
        );
        assert!(list.contains(&item), "{title} ist als Grenze ({kind}) markiert: {list}");
    }
    assert!(!list.contains("Die Bretter"), "Abschnitte der Ebene 2 haben keine eigene Datei (Vertrag §0.2): {list}");
    assert!(
        list.contains("<strong aria-current=\"page\">1 Vorwort</strong>"),
        "die gezeigte Datei ist hervorgehoben, nicht verlinkt: {list}"
    );
    assert!(
        list.contains(&format!(
            "href=\"/werke/{work}/epub/vorschau?leser=apple-books-iphone&amp;leser=apple-books-ipad&amp;leser=google-play-books&amp;leser=kindle&amp;leser=kindle-app&amp;datei={}\"",
            href_of(&files, "Das Dach")
        )),
        "jede andere Datei ist ein gewöhnlicher Link mit der Profilwahl (ohne JavaScript): {list}"
    );
    // Form GET: all profiles as checkboxes, mandatory profiles preselected.
    assert!(
        html.contains(&format!("<form class=\"epub-preview-form\" method=\"get\" action=\"/werke/{work}/epub/vorschau\" data-widget=\"epub-preview\">")),
        "Profilwahl als gewöhnliches GET-Formular: {html}"
    );
    for p in PROFILES {
        let input = format!(
            "<input type=\"checkbox\" name=\"leser\" value=\"{}\"{}>",
            p.slug,
            if p.required { " checked" } else { "" }
        );
        assert!(html.contains(&input), "{}: Häkchen, vorgewählt genau bei Pflicht: {html}", p.slug);
    }
    assert!(
        html.contains("<button class=\"btn primary\" type=\"submit\">Anzeigen</button>"),
        "Absenden ohne JavaScript"
    );
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste von Platzhaltern");
}

#[tokio::test]
async fn leser_and_datei_choose_readers_and_file_by_form_get() {
    let Book { server, work, files } = book().await;
    let roof = href_of(&files, "Das Dach");
    let (status, html) =
        server.get(&format!("/werke/{work}/epub/vorschau?leser=kobo&leser=kindle&datei={roof}&nacht=1")).await;
    assert_eq!(status, StatusCode::OK, "Auswahl lädt: {html}");
    let frames: Vec<&str> = html.split("<figure class=\"epub-frame").skip(1).collect();
    assert_eq!(frames.len(), 2, "genau die zwei gewählten Profile: {html}");
    assert!(
        frames[0].contains("data-reader=\"kindle\"") && frames[1].contains("data-reader=\"kobo\""),
        "Reihenfolge wie PROFILES"
    );
    assert!(
        html.contains(&format!("<iframe src=\"/werke/{work}/epub/leser/kindle/{roof}?nacht=1\"")),
        "die gewählte Datei im Nachtmodus: {html}"
    );
    assert!(html.contains(&format!("<option value=\"{roof}\" selected>")), "die Datei ist im Formular gewählt");
    assert!(html.contains("<input type=\"checkbox\" name=\"nacht\" value=\"1\" checked>"), "Nachtmodus bleibt gewählt");
    assert!(
        html.contains("<input type=\"checkbox\" name=\"leser\" value=\"apple-books-iphone\">"),
        "nicht gewählte Pflicht-Profile sind abgewählt"
    );
    assert!(html.contains("Über Send to Kindle (EPUB wird umgewandelt)."), "Kindle-Rahmen nennt die Umwandlung");
    assert!(html.contains("Fußnoten als Sprung statt Popup"), "Kobo nennt seine Eigenheit: {html}");
    // Footnotes as annotations: the frames and the download carry the choice on.
    let (_, html) = server.get(&format!("/werke/{work}/epub/vorschau?leser=thorium&fussnoten=endnoten")).await;
    assert!(
        html.contains(&format!("/werke/{work}/epub/leser/thorium/")) && html.contains("?fussnoten=endnoten\""),
        "Endnoten im Rahmen: {html}"
    );
    assert!(
        html.contains(&format!("href=\"/werke/{work}/epub?fussnoten=endnoten\" download")),
        "Download mit derselben Wahl"
    );
}

#[tokio::test]
async fn unknown_readers_files_and_works_give_404_with_a_notice_instead_of_a_crash() {
    let Book { server, work, files } = book().await;
    let (status, html) = server.get(&format!("/werke/{work}/epub/vorschau?leser=nook&leser=tolino")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekannter Slug → 404: {html}");
    assert!(html.contains("Unbekanntes Reader-Profil „nook“"), "Hinweis nennt den Slug: {html}");
    assert!(html.contains("data-reader=\"tolino\""), "das bekannte Profil wird trotzdem gezeigt");
    let (status, html) = server.get(&format!("/werke/{work}/epub/vorschau?datei=text/k999999.xhtml")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekannte Datei → 404");
    assert!(html.contains("k999999.xhtml“ gibt es im EPUB nicht"), "Hinweis statt Absturz: {html}");
    assert!(html.contains(href_of(&files, "Vorwort")), "gezeigt wird die erste Kapiteldatei");
    let (status, html) = server.get(&format!("/werke/{work}/epub/vorschau?datei=../../etc/passwd")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "Pfad hinaus → 404: {html}");
    let (status, _) = server.get("/werke/987654/epub/vorschau").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
    let (status, _, _) =
        get_with(&server, &format!("/werke/{work}/epub/vorschau"), &[("sec-fetch-site", "cross-site")]).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Seiten stoßen keinen EPUB-Bau an");
}

#[tokio::test]
async fn a_reader_file_is_xhtml_with_the_reader_css_its_own_csp_and_nosniff() {
    let Book { server, work, files } = book().await;
    let chapter = href_of(&files, "Das Holz");
    let (status, headers, body) = get_with(&server, &format!("/werke/{work}/epub/leser/kindle/{chapter}"), &[]).await;
    assert_eq!(status, StatusCode::OK, "Kapiteldatei lädt: {body}");
    assert_eq!(header_of(&headers, header::CONTENT_TYPE), "application/xhtml+xml; charset=utf-8", "XHTML wie im EPUB");
    assert_eq!(
        header_of(&headers, header::CONTENT_SECURITY_POLICY),
        "sandbox allow-same-origin; frame-ancestors 'self'",
        "eigene CSP nach Vertrag §0.9 (keine Skripte, nur in Baruch einrahmbar); allow-same-origin, damit Bilder \
         und Stile der Lesedatei nicht als fremde Anfrage mit 403 abgewiesen werden (Umschlagbild fehlte)"
    );
    assert_eq!(header_of(&headers, header::X_CONTENT_TYPE_OPTIONS), "nosniff", "kein Raten des Typs");
    assert_eq!(
        header_of(&headers, header::X_FRAME_OPTIONS),
        "SAMEORIGIN",
        "no_framing lässt die eigene Vorschau einrahmen (kein DENY)"
    );
    let style = body
        .find("<style id=\"baruch-reader\" type=\"text/css\" data-reader=\"kindle\">")
        .expect("Reader-CSS eingefügt");
    let link = body.find("href=\"../style/book.css\"").expect("style/book.css bleibt verlinkt");
    assert!(link < style && style < body.find("</head>").unwrap(), "Reader-CSS steht hinter book.css im Kopf");
    assert!(body.contains("Bookerly"), "Kindle-Vorgabeschrift im eingefügten CSS");
    assert!(!body.contains(NIGHT_MARK), "ohne nacht=1 kein Nachtteil");
    assert!(body.contains("Fichte ist weich"), "der Text des Kapitels");
    // Night mode: the part after the mark is added.
    let (_, _, night) = get_with(&server, &format!("/werke/{work}/epub/leser/kindle/{chapter}?nacht=1"), &[]).await;
    assert!(night.contains(NIGHT_MARK), "mit nacht=1 der Nachtteil des Profils");
    // Well-formed (xmllint; if it is missing, skipped – quick-xml is no dependency of the server).
    let lint = std::path::Path::new("/usr/bin/xmllint");
    if lint.exists() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("k.xhtml");
        std::fs::write(&file, &body).unwrap();
        let out = std::process::Command::new(lint).args(["--noout", "--nonet"]).arg(&file).output().unwrap();
        assert!(
            out.status.success(),
            "XHTML mit Reader-CSS bleibt wohlgeformt: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    } else {
        println!("ÜBERSPRUNGEN: xmllint fehlt, Wohlgeformtheit nicht geprüft");
    }
    // style/book.css unchanged with a type of its own.
    let (status, headers, css) =
        get_with(&server, &format!("/werke/{work}/epub/leser/kindle/style/book.css"), &[]).await;
    assert_eq!(status, StatusCode::OK, "Stildatei lädt");
    assert!(header_of(&headers, header::CONTENT_TYPE).starts_with("text/css"), "CSS als text/css");
    assert!(!css.contains("baruch-reader"), "book.css bleibt unverändert (Reader-CSS nur im XHTML)");
}

#[tokio::test]
async fn bad_paths_unknown_files_and_foreign_sites_are_refused() {
    let Book { server, work, files } = book().await;
    let chapter = href_of(&files, "Das Holz");
    for (uri, why) in [
        (format!("/werke/{work}/epub/leser/kindle/../../etc/passwd"), "Pfad mit .."),
        (format!("/werke/{work}/epub/leser/kindle/text/%2e%2e/%2e%2e/x"), "kodiertes .."),
        (format!("/werke/{work}/epub/leser/kindle/text/k999999.xhtml"), "unbekannte Datei"),
        (format!("/werke/{work}/epub/leser/kindle/mimetype"), "nicht im Manifest"),
        (format!("/werke/{work}/epub/leser/nook/{chapter}"), "unbekanntes Profil"),
        (format!("/werke/987654/epub/leser/kindle/{chapter}"), "unbekanntes Werk"),
    ] {
        let (status, _, body) = get_with(&server, &uri, &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{why}: {uri} → 404: {body}");
    }
    let uri = format!("/werke/{work}/epub/leser/kindle/{chapter}");
    let (status, _, _) = get_with(&server, &uri, &[("sec-fetch-site", "cross-site")]).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Seite → 403");
    let (status, _, _) = get_with(
        &server,
        &uri,
        &[
            ("sec-fetch-site", "cross-site"),
            ("host", "127.0.0.1:8765"),
            ("referer", "http://evil.example/werke/1/epub/leser/kindle/x"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ein fremder Referer öffnet nichts");
    // Sub-requests from the own frame, which has no origin because of sandbox (image, book.css), arrive as cross-site.
    let referer = format!("http://127.0.0.1:8765/werke/{work}/epub/leser/kindle/{chapter}");
    let (status, _, _) = get_with(
        &server,
        &format!("/werke/{work}/epub/leser/kindle/style/book.css"),
        &[("sec-fetch-site", "cross-site"), ("host", "127.0.0.1:8765"), ("referer", &referer)],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "Unteranfrage aus dem eigenen Rahmen desselben Werks wird beantwortet");
    // Integrator R20 (visual check with Firefox): sub-requests from the sandbox frame carry only the origin as Referer;
    // images and book.css must load nevertheless, a foreign origin must not.
    for (referer, expected, why) in [
        ("http://127.0.0.1:8765/", StatusCode::OK, "gekürzter Referer desselben Hosts"),
        ("http://evil.example/", StatusCode::FORBIDDEN, "gekürzter Referer einer fremden Seite"),
    ] {
        let (status, _, _) = get_with(
            &server,
            &format!("/werke/{work}/epub/leser/kindle/style/book.css"),
            &[("sec-fetch-site", "cross-site"), ("host", "127.0.0.1:8765"), ("referer", referer)],
        )
        .await;
        assert_eq!(status, expected, "{why}");
    }
}

#[tokio::test]
async fn images_keep_their_bytes_and_media_type() {
    // reader_file works on the manifest; an own image added by hand (own path, because since R20a the test work itself
    // brings images under images/bilder/).
    let mut book = baruch_epub::build(&baruch_epub::sample::input()).unwrap();
    let png = baruch_epub::sample::png(2, 2, [10, 20, 30]);
    book.items.push(EpubItem {
        id: "img-1".into(),
        href: "images/probe/reader.png".into(),
        media_type: "image/png".into(),
        kind: ItemKind::Image,
        properties: None,
        title: None,
        key: None,
        bytes: png.clone(),
    });
    let kindle = profile("kindle").unwrap();
    let (media_type, bytes) = reader_file(&book, kindle, "images/probe/reader.png", false).expect("Bild gefunden");
    assert_eq!(media_type, "image/png", "Bild mit dem Typ aus dem Manifest");
    assert_eq!(bytes, png, "Bild byte-gleich, kein Reader-CSS");
    assert!(reader_file(&book, kindle, "images/../nav.xhtml", false).is_none(), "kein Pfad mit ..");
    let (media_type, nav) = reader_file(&book, kindle, "nav.xhtml", false).expect("nav.xhtml");
    assert!(media_type.starts_with("application/xhtml+xml"), "nav ist XHTML");
    assert!(String::from_utf8(nav).unwrap().contains("data-reader=\"kindle\""), "auch nav.xhtml mit Reader-CSS");
}

#[tokio::test]
async fn no_framing_keeps_deny_everywhere_except_for_responses_with_their_own_frame_ancestors() {
    let Book { server, work, files } = book().await;
    for uri in ["/".to_owned(), "/werke".to_owned(), format!("/werke/{work}/epub/vorschau")] {
        let (status, headers, _) = get_with(&server, &uri, &[]).await;
        assert!(status.is_success(), "{uri} lädt");
        assert_eq!(header_of(&headers, header::X_FRAME_OPTIONS), "DENY", "{uri}: nie einrahmbar (Clickjacking)");
        assert!(
            header_of(&headers, header::CONTENT_SECURITY_POLICY).contains("frame-ancestors 'none'"),
            "{uri}: frame-ancestors 'none' bleibt"
        );
    }
    let (_, headers, _) =
        get_with(&server, &format!("/werke/{work}/epub/leser/tolino/{}", href_of(&files, "Bauen")), &[]).await;
    assert_ne!(header_of(&headers, header::X_FRAME_OPTIONS), "DENY", "die Reader-Datei darf in der Vorschau stehen");
    assert!(
        !header_of(&headers, header::CONTENT_SECURITY_POLICY).contains("'none'"),
        "an die eigene CSP wird kein frame-ancestors 'none' angehängt"
    );
}

#[tokio::test]
async fn rpc_lists_the_readers_and_builds_the_preview_addresses() {
    let Book { server, work, files } = book().await;
    let answer = server.rpc("epub.readers", json!({})).await;
    let readers = answer["result"]["readers"].as_array().expect("readers");
    assert_eq!(readers.len(), PROFILES.len(), "alle Profile: {answer}");
    assert_eq!(readers[3]["name"], "Kindle (über EPUB)", "Werte wie in baruch_epub::readers");
    let answer = server.rpc("epub.preview", json!({ "work": work })).await;
    let result = &answer["result"];
    assert!(answer["error"].is_null(), "epub.preview gelingt: {answer}");
    let first = href_of(&files, "Vorwort");
    assert_eq!(result["file"], first, "Vorgabe: erste Teil- bzw. Kapiteldatei");
    let slugs: Vec<&str> = result["readers"].as_array().unwrap().iter().map(|r| r["slug"].as_str().unwrap()).collect();
    assert_eq!(
        slugs,
        ["apple-books-iphone", "apple-books-ipad", "google-play-books", "kindle", "kindle-app"],
        "Pflicht-Profile"
    );
    assert_eq!(
        result["readers"][0]["url"],
        format!("/werke/{work}/epub/leser/apple-books-iphone/{first}"),
        "Adresse je Profil"
    );
    assert_eq!(result["files"].as_array().unwrap().len(), files.len(), "dieselben Dateien wie work.epub");
    let roof = href_of(&files, "Das Dach");
    let answer = server
        .rpc("epub.preview", json!({ "work": work, "readers": ["calibre"], "file": roof, "footnotes": "endnotes" }))
        .await;
    assert_eq!(
        answer["result"]["readers"][0]["url"],
        format!("/werke/{work}/epub/leser/calibre/{roof}?fussnoten=endnoten"),
        "gewählte Datei, Profil und Endnoten: {answer}"
    );
    let answer = server.rpc("epub.preview", json!({ "work": work, "readers": ["nook"] })).await;
    assert_eq!(answer["error"]["code"], -32602, "unbekannter Slug → -32602: {answer}");
    let answer = server.rpc("epub.preview", json!({ "work": work, "file": "../content.opf" })).await;
    assert_eq!(answer["error"]["code"], -32602, "unbekannte Datei → -32602: {answer}");
}

#[tokio::test]
async fn the_preview_has_a_place_for_the_check_panel_of_epub_pruefung() {
    let Book { server, work, .. } = book().await;
    let (_, html) = server.get(&format!("/werke/{work}/epub/vorschau")).await;
    let slot = &html[html.find("data-epub-check-slot").expect("Platz für den Prüfbericht")..];
    assert!(
        slot.contains("<h2 class=\"pane-title\" id=\"epub-check-title\">Prüfung</h2>"),
        "Überschrift vorhanden: {slot}"
    );
    let panel = baruch_server::i18n::scope(
        baruch_server::i18n::Lang::De,
        baruch_server::epub_check::preview_panel(
            &server.state,
            work,
            &baruch_epub::readers::required().collect::<Vec<_>>(),
        ),
    )
    .await;
    if panel.is_empty() {
        assert!(
            slot.contains(&format!("href=\"/werke/{work}/epub/pruefung\"")),
            "ohne Bericht ein Link auf die Prüfseite"
        );
    } else {
        assert!(slot.contains(&panel), "das Fragment steht unverändert in der Vorschau");
    }
}

#[test]
fn every_reader_css_imitates_font_size_margins_line_height_hyphenation_and_night_mode() {
    for p in PROFILES {
        let css = reader_css(p.slug).unwrap_or_else(|| panic!("{}: CSS fehlt", p.slug));
        let (day, night) = css.split_once(NIGHT_MARK).unwrap_or_else(|| panic!("{}: Nachtteil fehlt", p.slug));
        for property in ["font-family:", "font-size:", "padding:", "line-height:", "hyphens:", "text-align:"] {
            assert!(day.contains(property), "{}: {property} bildet die Vorgabe des Readers nach", p.slug);
        }
        assert!(night.contains("background:") && night.contains("color:"), "{}: Nachtmodus mit Farben", p.slug);
        assert!(!css.contains('<') && !css.contains('&'), "{}: kein < oder & – das CSS steht in XHTML", p.slug);
        assert!(!css.contains("url(") && !css.contains("@import"), "{}: keine Adressen (sandbox, kein Netz)", p.slug);
        assert_eq!(
            css.contains("filter: grayscale(1)"),
            p.slug == "kindle" || p.slug == "tolino" || p.slug == "kobo",
            "{}: Graustufen genau bei E-Ink",
            p.slug
        );
        assert_eq!(
            css.contains("Popup-Fußnote"),
            p.supports.popup_footnotes,
            "{}: Fußnoten-Darstellung folgt supports.popup_footnotes",
            p.slug
        );
        assert_eq!(
            css.contains("font-family: Bookerly, Georgia, serif !important"),
            p.supports.overrides_publisher_font,
            "{}: nur Reader mit eigener Vorgabeschrift ersetzen die Schrift des Buchs",
            p.slug
        );
    }
}
