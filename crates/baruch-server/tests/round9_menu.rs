// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 9, Aufgabe „menu“: eine gemeinsame Kopfzeile in `base.html` mit den Menüs Werk ▾, Ansicht ▾
//! und Werkzeuge ▾ (`<details>`/`<summary>`, ohne JavaScript bedienbar), seitenlokalen Links in
//! `{% block page_actions %}`, kleinem Thema-Umschalter, Knopf „Einstellungen“ (Alt+,), Abzeichen
//! (Anträge zur Gliederung, offene Vorschläge) und der Hülle des Einstellungs-Dialogs am Seitenende.
//! Alle Testtexte frei erfunden (`tests/data/gartenbuch.typ` und die Texte hier).

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

const HOUSE: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><path d=\"M0 8 L4 0 L8 8 Z\"/></svg>\n";

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Die Kopfzeile der Seite (vom öffnenden Tag bis `</header>`).
fn topbar(html: &str) -> &str {
    let start = html.find("<header class=\"topbar\">").unwrap_or_else(|| panic!("keine Kopfzeile: {html}"));
    let end = start + html[start..].find("</header>").expect("Kopfzeile ohne Ende");
    &html[start..end]
}

/// Ein Menü der Kopfzeile (`<details class="menu" data-menu="…">` bis `</details>`).
fn menu<'a>(head: &'a str, name: &str) -> &'a str {
    let marker = format!("<details class=\"menu\" data-menu=\"{name}\">");
    let start = head.find(&marker).unwrap_or_else(|| panic!("Menü {name} fehlt: {head}"));
    let end = start + head[start..].find("</details>").expect("Menü ohne Ende");
    &head[start..end]
}

fn hrefs(html: &str) -> Vec<&str> {
    html.split("href=\"").skip(1).filter_map(|rest| rest.split('"').next()).collect()
}

/// Öffnende Tags der direkten Kinder eines Elementinhalts (einfacher Tiefenzähler; in der
/// Kopfzeile stehen keine leeren Elemente wie `<input>` direkt).
fn direct_children(inner: &str) -> Vec<String> {
    const VOID: &[&str] = &["input", "br", "img", "meta", "link", "hr"];
    let mut depth = 0usize;
    let mut out = Vec::new();
    let mut rest = inner;
    while let Some(at) = rest.find('<') {
        rest = &rest[at..];
        let end = rest.find('>').expect("Tag ohne Ende");
        let tag = &rest[..=end];
        if tag.starts_with("</") {
            depth = depth.saturating_sub(1);
        } else if !tag.starts_with("<!") {
            let name: String = tag[1..].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if depth == 0 {
                out.push(tag.to_owned());
            }
            if !VOID.contains(&name.as_str()) && !tag.ends_with("/>") {
                depth += 1;
            }
        }
        rest = &rest[end + 1..];
    }
    out
}

async fn mcp_call(server: &common::Server, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/call {tool}: {body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer
}

/// Feste Schlüssel der Abschnitte in Baumreihenfolge.
async fn keys(server: &common::Server, work: i64) -> Vec<(i64, String)> {
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["key"].as_i64().unwrap(), n["title"].as_str().unwrap().to_owned()))
        .collect()
}

async fn get_with_cookie(server: &common::Server, uri: &str, cookie: &str) -> (StatusCode, String) {
    let request = Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap();
    let (status, _, html) = server.send(request).await;
    (status, html)
}

/// Alle Seiten mit Kopfzeile (Abnahme Runde 9): Adresse → HTML, im Gartenbuch mit Bild und Einfall.
struct Pages {
    work: i64,
    key: i64,
    pages: Vec<(String, String)>,
}

async fn all_pages(server: &common::Server) -> Pages {
    let work = server.import_garden().await.work;
    let nodes = keys(server, work).await;
    let key = nodes[1].0;
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "Haus.svg", "content": HOUSE, "encoding": "svg",
                    "path": "/bilder/haus.svg", "caption": "Das Haus", "alt": "Ein Haus mit Dach" }),
        )
        .await;
    let asset = result(&added)["name"].as_str().unwrap().to_owned();
    let idea = server.rpc("idea.create", json!({ "work": work, "key": key, "words": "Laube", "note": "Rosen." })).await;
    let idea = result(&idea)["idea"]["id"].as_i64().unwrap();
    let list = server.rpc("library.list", json!(null)).await;
    let title = result(&list)["titles"][0]["id"].as_i64().unwrap();
    // Integrator Runde 28: ein eigener Beitrag für die Beitragsseite (chat-auftrag).
    let seq = server.state.store.node_seq(work, key).await.unwrap();
    let note = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "Autor".into(),
        body: "Die Laube braucht Schatten.".into(),
        sources: vec![],
        refs: vec![],
    };
    let post = server.state.store.add_post(work, seq, &note).await.unwrap();
    // Integrator Runde 30: eine Variantengruppe (zwei Geschwister-Vorschläge) für die Vergleichsseite /varianten
    // (chat-loesungen), direkt über den Store angelegt, weil die Seite nur eine vorhandene Gruppe zeigt.
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = &loaded.nodes[seq];
    let base = node.current_number().unwrap();
    let current = node.revision(base).unwrap().body.clone();
    let group = "0b6f4c1e-7a1d-4c55-9a51-2f3c8d9e0a11";
    for (label, extra) in [("A", "Kurz gefasst."), ("B", "Anders gefasst.")] {
        let info = baruch_store::VariantInfo {
            group: group.into(),
            label: label.into(),
            summary: format!("Variante {label}"),
            findings: vec![],
        };
        let body = format!("{}\n{extra}\n", current.trim_end());
        server.state.store.add_proposal_on(work, seq, &body, "@lektor", "m", "", base, Some(&info)).await.unwrap();
    }

    let mut pages = Vec::new();
    for uri in [
        format!("/?work={work}&key={key}"),
        "/werke".to_owned(),
        "/werke/neu".to_owned(),
        format!("/werke/{work}/loeschen"),
        format!("/gliederung?work={work}"),
        format!("/gliederung/bearbeiten?work={work}&key={key}"),
        format!("/werke/{work}/bilder"),
        format!("/werke/{work}/bilder/{asset}"),
        format!("/einfaelle?work={work}"),
        format!("/einfaelle/{idea}"),
        format!("/literatur?work={work}&key={key}"),
        format!("/quelle?title={title}&work={work}&key={key}"),
        format!("/werke/{work}/satz?key={key}"),
        // Integrator Runde 11: Umschlagseite (cover).
        format!("/werke/{work}/umschlag"),
        // Integrator Runde 9: Seiten aus settings (Werk bearbeiten, Gliederungsschema, Einstellungen).
        format!("/werke/{work}/bearbeiten"),
        format!("/werke/{work}/nummerierung"),
        format!("/einstellungen/darstellung?work={work}"),
        format!("/einstellungen/werk?work={work}"),
        format!("/einstellungen/assistenten?work={work}"),
        format!("/einstellungen/verbindungen?work={work}"),
        format!("/einstellungen/quellen?work={work}"),
        format!("/einstellungen/mcp?work={work}"),
        format!("/einstellungen/daten?work={work}"),
        // Runde 13 (abgleich): Seite des Abgleichs mit MultiGPT.
        format!("/abgleich?work={work}"),
        // Integrator Runde 13: Dokumentansicht von MultiGPT (quellen) und Belegseite der Websuche (websuche).
        format!("/quelle/multigpt?work={work}&key={key}&collection=Nachbau&document=doc-1"),
        format!("/websuche/belege?work={work}&key={key}&post=1"),
        // Integrator Runde 15 (auftraege): Fäden ohne Abschnitt und die Bibelseite der Literatur (ohne Bibel
        // ein ehrlicher Hinweis, die Seite lädt trotzdem).
        format!("/faeden?work={work}"),
        format!("/literatur/bibel?work={work}&key={key}"),
        // Runde 16c (stichwortzettel): Seite des Stichwortzettels mit Formatwahl und Vorschau.
        format!("/werke/{work}/stichwortzettel"),
        // Runde 17c (pruefanzeige): Prüfanzeige der Zitate (ohne Bibel ein ehrlicher Hinweis, die Seite lädt).
        format!("/werke/{work}/pruefung?key={key}"),
        // Runde 26 (Schnittstelle): Austausch ausgeben (Seite des Werks) und einlesen (Formular).
        format!("/werke/{work}/austausch"),
        "/austausch/einlesen".to_owned(),
        // Runde 25 (karte-bild): Gedankenkarte der Einfälle.
        format!("/werke/{work}/karte"),
        // Integrator Runde 28: Befunde übers Werk (befund-uebersicht), ganze Chat-Historie und Beitragsseite
        // ohne JavaScript (chat-auftrag).
        format!("/werke/{work}/befunde"),
        format!("/beitraege?work={work}&key={key}"),
        format!("/beitrag?work={work}&id={post}"),
        // Integrator Runde 30: Werk-Suche (suchen-verweise), Querverweise (suchen-verweise) und der Vergleich der
        // Varianten (chat-loesungen).
        format!("/suche?work={work}&q=Laube"),
        format!("/verweise?work={work}"),
        format!("/varianten?work={work}&key={key}&group={group}"),
        // Runde 20b (reader): EPUB-Vorschau je Reader.
        format!("/werke/{work}/epub/vorschau"),
        // Integrator Runde 20: Prüfbericht des EPUB (epub-pruefung).
        format!("/werke/{work}/epub/pruefung"),
        // Runde 31 (Schnittstelle R31-S1): Glossar, Register und Formular an der Markierung (kontextmenue).
        format!("/werke/{work}/glossar"),
        format!("/werke/{work}/register"),
        format!("/werke/{work}/markierung?key={key}"),
        // Integrator Runde 22: Archiv der gelöschten Werke (befund-rueckfrage) und Verlauf einer Fassung (fassungen).
        "/werke/archiv".to_owned(),
        format!("/werke/{work}/fassungen/verlauf?key={key}&rev=1"),
        // Runde 24 (pakete): Über Baruch.
        "/ueber".to_owned(),
    ] {
        let (status, html) = get_with_cookie(server, &uri, &format!("baruch_work={work}")).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        pages.push((uri, html));
    }
    // Bestätigungsseite: Speichern ohne JavaScript würde den Abschnitt teilen (neue Überschrift).
    let body = format!(
        "method=outline.save&work={work}&key={}&body={}",
        nodes[3].0, "%3D+Sommer%0A%0AGie%C3%9Fen.%0A%0A%3D+Herbst%0A%0ALaub.%0A"
    );
    let request = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Bestätigungsseite statt Speichern: {html}");
    assert!(html.contains("Speichern bestätigen"), "es ist die Bestätigungsseite: {html}");
    pages.push(("POST /gliederung/act (Rückfrage)".to_owned(), html));
    Pages { work, key, pages }
}

#[tokio::test]
async fn every_page_has_exactly_one_topbar_with_the_three_menus() {
    let server = common::open().await;
    let Pages { work, pages, .. } = all_pages(&server).await;
    assert_eq!(
        pages.len(),
        48,
        "alle vierzehn Seiten der Abnahme, dazu Werk bearbeiten, Gliederungsschema, die sieben Einstellungsseiten, \
         die Umschlagseite (Runde 11), die Seite des Abgleichs, die Dokumentansicht von MultiGPT, die \
         Belegseite der Websuche (Runde 13), die Fadenliste und die Bibelseite der Literatur (Runde 15) und der \
         Stichwortzettel (Runde 16), die Prüfanzeige der Zitate (Runde 17c), die beiden Austauschseiten \
         (Runde 26), die Gedankenkarte (Runde 25), die Befundseite, die Chat-Historie und die Beitragsseite \
         (Runde 28), Werk-Suche, Querverweise und Vergleich der Varianten (Runde 30), die EPUB-Vorschau (Runde 20b) \
         und der Prüfbericht des EPUB (Runde 20c), Glossar, Register und das Formular an der Markierung (Runde 31), \
         Archiv und Verlauf einer Fassung (Runde 22), die Über-Seite (Runde 24)"
    );
    for (uri, html) in &pages {
        assert_eq!(
            html.matches("<header class=\"topbar\"").count(),
            1,
            "{uri}: genau eine Kopfzeile (die eigene der Seite ist entfernt, base.html zeichnet sie): {html}"
        );
        let head = topbar(html);
        assert!(
            // Runde 23a: außerhalb der Arbeitsseite trägt die Leiste zusätzlich das Werk für die Abzeichen.
            head.contains("<nav class=\"menubar\" data-widget=\"menubar\" aria-label=\"Menü\""),
            "{uri}: Menüleiste als Widget menubar mit Namen für Hilfsmittel: {head}"
        );
        for name in ["werk", "ansicht", "werkzeuge"] {
            assert_eq!(
                head.matches(&format!("<details class=\"menu\" data-menu=\"{name}\">")).count(),
                1,
                "{uri}: Menü {name} genau einmal (ein <details>, klappt ohne JavaScript auf)"
            );
            let m = menu(head, name);
            assert!(
                m.contains("<summary class=\"btn\" aria-haspopup=\"menu\">"),
                "{uri}: Menü {name} öffnet über ein <summary> mit aria-haspopup=menu: {m}"
            );
            assert!(m.contains("<div class=\"menu-list\" role=\"menu\">"), "{uri}: Liste mit role=menu in {name}: {m}");
            assert!(
                !m.contains("<button"),
                "{uri}: {name} enthält keine Knöpfe, die nur mit JavaScript etwas tun – alles Links oder Stubs: {m}"
            );
        }
        // Die Satz-Seite lädt kein main.js (Runde 18a): Dort sind Thema und Einstellungen schlichte Links
        // (Nachbesserung Runde 18, geprüft in round18_menueband::the_typeset_page_header_works_without_main_js).
        if uri.contains("/satz") {
            assert!(head.contains("aria-label=\"Thema einstellen"), "{uri}: Thema als Link ohne Widget: {head}");
        } else {
            assert!(head.contains("data-widget=\"theme\""), "{uri}: kleiner Thema-Umschalter bleibt in der Kopfzeile");
            assert!(
                head.contains(&format!(
                    "<a class=\"btn settings-open\" href=\"/einstellungen/darstellung?work={work}\" data-settings-open \
                     aria-haspopup=\"dialog\" aria-controls=\"settings\" aria-keyshortcuts=\"Alt+Comma\" \
                     title=\"Einstellungen (Alt+,)\">⚙ Einstellungen</a>"
                )),
                "{uri}: Knopf Einstellungen mit Werk (aus Seite oder Cookie), Alt+, und Ziel ohne JavaScript: {head}"
            );
        }
        for marker in ["data-literature-open", "data-idea-open", "data-work-import-open", "data-view-toggle"] {
            assert!(
                html.matches(marker).count() <= 1,
                "{uri}: höchstens ein {marker} (literature.ts/ideas.ts/workspace.ts nehmen querySelector)"
            );
        }
        let bad: Vec<&str> = hrefs(html).into_iter().filter(|h| h.contains("node=")).collect();
        assert!(bad.is_empty(), "{uri}: kein erzeugter Link mit node= (Vertrag Runde 8 §1): {bad:?}");
        assert!(!html.contains("{{") && !html.contains("{%"), "{uri}: keine Reste von Platzhaltern");
    }
}

#[tokio::test]
async fn the_settings_dialog_shell_follows_the_page_content() {
    let server = common::open().await;
    let Pages { pages, .. } = all_pages(&server).await;
    let shell = "<dialog class=\"settings-dialog\" id=\"settings\" data-widget=\"settings\" aria-labelledby=\"settings-title\">\n  \
        <header class=\"literature-head\"><h2 class=\"pane-title\" id=\"settings-title\">Einstellungen</h2><span class=\"grow\"></span>\
        <form method=\"dialog\"><button class=\"btn\" type=\"submit\">Schließen</button></form></header>\n  \
        <div class=\"settings-content\" data-settings-content></div>\n</dialog>";
    for (uri, html) in &pages {
        assert_eq!(html.matches(shell).count(), 1, "{uri}: Hülle des Einstellungs-Dialogs genau nach Vertrag: {html}");
        let at = html.find(shell).unwrap();
        let last_dialog = html.rfind("<dialog").unwrap();
        assert_eq!(
            at, last_dialog,
            "{uri}: die Hülle steht hinter allen Dialogen der Seite (literature.rs nimmt den ersten)"
        );
        if let Some(main) = html.rfind("</main>") {
            assert!(main < at, "{uri}: die Hülle folgt dem Seiteninhalt");
        }
        assert!(at < html.find("</body>").unwrap(), "{uri}: und steht noch im <body>");
    }
    let (_, workspace) = &pages[0];
    assert!(
        workspace.find("<dialog class=\"literature-dialog\" id=\"literature\"").unwrap()
            < workspace.find(shell).unwrap(),
        "Arbeitsseite: der Literatur-Dialog bleibt der erste <dialog>"
    );
}

#[tokio::test]
async fn the_workspace_topbar_keeps_every_former_address() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let nodes = keys(&server, work).await;
    let key = nodes[3].0;
    let (status, html) = server.get(&format!("/?work={work}&key={key}&lq=Beete")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite lädt: {html}");
    let head = topbar(&html);
    let rev = {
        let tail = &head[head.find("&rev=").expect("rev in der Kopfzeile") + 5..];
        tail.chars().take_while(char::is_ascii_digit).collect::<String>()
    };
    let werk = menu(head, "werk");
    let ansicht = menu(head, "ansicht");
    let werkzeuge = menu(head, "werkzeuge");
    // Werkwahl wie bisher (GET-Formular), nur auf der Arbeitsseite.
    assert!(
        head.contains("<form class=\"work-select\" action=\"/\" method=\"get\"")
            && head.contains("<select name=\"work\" aria-label=\"Werk\" data-widget=\"work-select\">")
            && head.contains("<noscript><button class=\"btn\" type=\"submit\">Öffnen</button></noscript>"),
        "Werkwahl unverändert in der Kopfzeile: {head}"
    );
    assert!(head.contains(&format!("<a class=\"brand\" href=\"/?work={work}\">Baruch</a>")), "Baruch führt zum Werk");
    assert!(head.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "Werktitel wie bisher: {head}");
    assert!(head.contains("<span class=\"muted\">· Grete Gärtner</span>"), "Autor daneben, kürzt sich zuerst");
    for (entry, place) in [
        ("href=\"/werke\">Alle Werke</a>", werk),
        (
            "href=\"/werke/neu\" data-work-import-open aria-haspopup=\"dialog\" aria-controls=\"work-import\">Neues Werk / Importieren…</a>",
            werk,
        ),
        (&format!("href=\"/werke/{work}/bearbeiten\">Werk bearbeiten</a>"), werk),
        (&format!("href=\"/gliederung?work={work}\">Gliederung</a>"), werk),
        (&format!("href=\"/gliederung?work={work}#antraege\""), werk),
        (&format!("href=\"/werke/{work}/nummerierung\">Gliederungsschema</a>"), werk),
        (&format!("href=\"/werke/{work}/bilder\">Bilder</a>"), werk),
        (&format!("href=\"/werke/{work}/export\""), werk),
        // Runde 11: das Buch als PDF, die Umschlagseite und das Umschlag-PDF (Herunterladen als Anhang).
        (&format!("href=\"/werke/{work}/pdf\" download"), werk),
        (&format!("href=\"/werke/{work}/umschlag\" title"), werk),
        (&format!("href=\"/werke/{work}/umschlag.pdf\" download"), werk),
        (
            &format!(
                "data-nav data-view-toggle href=\"/?work={work}&key={key}&rev={rev}&view=preview&amp;lq=Beete\">Vorschau</a>"
            ),
            ansicht,
        ),
        (&format!("href=\"/werke/{work}/satz?key={key}\""), ansicht),
        (
            &format!(
                "href=\"/literatur?work={work}&key={key}&rev={rev}&amp;lq=Beete\" data-literature-open aria-haspopup=\"dialog\" aria-controls=\"literature\""
            ),
            werkzeuge,
        ),
        (
            &format!(
                "href=\"/einfaelle?work={work}\" data-idea-open aria-haspopup=\"dialog\" aria-controls=\"idea-capture\" title=\"Einfall festhalten (Alt+E)\""
            ),
            werkzeuge,
        ),
        (&format!("href=\"/einfaelle?work={work}\" title=\"Liste der Einfälle dieses Werks\">Einfälle</a>"), werkzeuge),
        // Runde 25 (karte-bild): Gedankenkarte, nur mit Werk.
        (
            &format!(
                "href=\"/werke/{work}/karte\" title=\"Gedankenkarte der Einfälle dieses Werks\">Gedankenkarte</a>"
            ),
            werkzeuge,
        ),
    ] {
        assert!(
            place.contains(&format!("<a class=\"menu-item\" role=\"menuitem\" {entry}"))
                || place.contains(&format!("role=\"menuitem\" {entry}")),
            "Eintrag {entry} steht als gewöhnlicher Link (ohne JavaScript bedienbar) im richtigen Menü: {place}"
        );
    }
    assert!(ansicht.contains(">Satz</a>") && werkzeuge.contains(">Literatur</a>"), "Beschriftungen wie bisher");
    // Vorschau: der Umschalter zeigt zurück auf den Editor.
    let (_, preview) = server.get(&format!("/?work={work}&key={key}&view=preview")).await;
    let toggle = menu(topbar(&preview), "ansicht");
    assert!(
        toggle.contains(&format!("data-nav data-view-toggle href=\"/?work={work}&key={key}&rev={rev}\">Editor</a>")),
        "in der Vorschau führt der Umschalter zum Editor (Key-Form, ohne view): {toggle}"
    );
    // Runde 20b (reader): „EPUB-Vorschau“ ist kein Stub mehr, sondern ein Link auf die Vorschau des Werks; im Menü
    // Werk steht „Als EPUB herunterladen“ als Anhang neben dem PDF.
    assert!(
        ansicht.contains(&format!(
            "<a class=\"menu-item\" role=\"menuitem\" href=\"/werke/{work}/epub/vorschau\" title=\"Das EPUB des Werks in mehreren Readern nebeneinander\">EPUB-Vorschau</a>"
        )),
        "EPUB-Vorschau verlinkt die Seite /werke/W/epub/vorschau (ohne JavaScript bedienbar): {ansicht}"
    );
    assert!(!ansicht.contains("aria-disabled"), "im Menü Ansicht ist nichts mehr gesperrt: {ansicht}");
    assert!(
        werk.contains(&format!(
            "<a class=\"menu-item\" role=\"menuitem\" href=\"/werke/{work}/epub\" download title=\"Das ganze Werk als E-Book (EPUB 3, aktuelle Fassungen)\">Als EPUB herunterladen</a>"
        )),
        "Als EPUB herunterladen: Anhang /werke/W/epub im Menü Werk: {werk}"
    );
    let pdf = werk.find(">Als PDF herunterladen</a>").expect("PDF im Menü Werk");
    let epub = werk.find(">Als EPUB herunterladen</a>").expect("EPUB im Menü Werk");
    assert!(pdf < epub, "EPUB steht direkt neben dem PDF (dahinter)");
    // Runde 13 (abgleich): „Abgleichen“ ist kein Stub mehr, sondern ein gewöhnlicher Link auf /abgleich.
    assert!(
        werkzeuge.contains(&format!(
            "<a class=\"menu-item\" role=\"menuitem\" href=\"/abgleich?work={work}\" title=\"Abgleich mit MultiGPT\">Abgleichen</a>"
        )),
        "Abgleichen verlinkt die Seite /abgleich des Werks (ohne JavaScript bedienbar): {werkzeuge}"
    );
    assert!(!werkzeuge.contains("aria-disabled"), "im Menü Werkzeuge ist nichts mehr gesperrt: {werkzeuge}");
    // Runde 24 (pakete): „Über Baruch“ als gewöhnlicher Link am Ende des Menüs Werkzeuge.
    assert!(
        werkzeuge.contains(
            "<a class=\"menu-item\" role=\"menuitem\" href=\"/ueber\" title=\"Version, Lizenz, Quellcode und Bibliotheken Dritter\">Über Baruch</a>"
        ),
        "Über Baruch verlinkt die Seite /ueber (ohne JavaScript bedienbar): {werkzeuge}"
    );
    let old_buttons =
        head.matches("<a class=\"btn\"").count() + head.matches("<button class=\"btn\" type=\"button\"").count();
    assert!(old_buttons <= 1, "statt elf Knöpfen nur noch der Thema-Umschalter als Knopf neben Einstellungen: {head}");
}

#[tokio::test]
async fn other_pages_keep_their_local_links_in_page_actions() {
    let server = common::open().await;
    let Pages { work, key, pages } = all_pages(&server).await;
    let page = |prefix: &str| -> &str {
        &pages.iter().find(|(uri, _)| uri.starts_with(prefix)).unwrap_or_else(|| panic!("Seite {prefix}")).1
    };
    // Alle früheren Kopfzeilen-Adressen der Seiten (Stand 596ba72), jetzt hinter der Menüleiste.
    let expected: Vec<(String, Vec<String>)> = vec![
        (
            "/werke".into(),
            vec![
                "<a class=\"btn primary\" href=\"/werke/neu\">Neues Werk oder Import…</a>".into(),
                "\">Zurück zum Werk</a>".into(),
            ],
        ),
        ("/werke/neu".into(), vec!["<a class=\"btn\" href=\"/werke\">Werke</a>".into()]),
        (
            format!("/werke/{work}/loeschen"),
            vec![
                "<a class=\"btn\" href=\"/werke\">Werke</a>".into(),
                // back_url ist ein Wert (kein Vorlagentext): minijinja maskiert „/“ als &#x2f; wie bisher.
                format!("<a class=\"btn\" href=\"&#x2f;?work={work}\">Zurück zum Werk</a>"),
            ],
        ),
        ("/gliederung?".into(), vec![format!("<a class=\"btn\" href=\"/?work={work}&amp;key=")]),
        (
            "/gliederung/bearbeiten".into(),
            vec![
                format!("<a class=\"btn\" href=\"/gliederung?work={work}&amp;key={key}\">Gliederung</a>"),
                format!("<a class=\"btn\" href=\"/?work={work}&amp;key={key}\">Zurück zum Werk</a>"),
            ],
        ),
        (
            format!("/werke/{work}/bilder/"),
            vec![
                format!("<a class=\"btn\" href=\"/werke/{work}/bilder\">Alle Bilder</a>"),
                format!("<a class=\"btn\" href=\"/?work={work}\">Zurück zum Werk</a>"),
            ],
        ),
        ("/einfaelle?".into(), vec!["\">Zurück zum Werk</a>".into()]),
        (
            "/einfaelle/".into(),
            vec![
                "\">Alle Einfälle</a>".into(),
                format!("<a class=\"btn\" href=\"/?work={work}&amp;key={key}\">Zum Werk</a>"),
            ],
        ),
        (
            "/literatur".into(),
            vec![format!("<a class=\"btn\" href=\"/?work={work}&amp;key={key}\" data-back>Zurück zum Werk</a>")],
        ),
        (
            "/quelle".into(),
            vec![
                "\">Literatur</a>".into(),
                format!("<a class=\"btn\" href=\"/?work={work}&amp;key={key}\" data-back>Zurück zum Werk</a>"),
            ],
        ),
    ];
    for (prefix, links) in &expected {
        let head = topbar(page(prefix));
        let actions = &head[head.find("</nav>").expect("Menüleiste")..];
        for link in links {
            assert!(
                actions.contains(link.as_str()),
                "{prefix}: seitenlokaler Link {link} steht in page_actions hinter den Menüs: {head}"
            );
        }
    }
    // Runde 18a: Die Satz-Seite trägt „Zurück zum Werk“ und „PDF herunterladen“ im Menüband unter der Kopfzeile
    // (fragments/edit_toolbar.html), nicht mehr in page_actions – dieselben Adressen, nur ein Ort.
    let satz = page(&format!("/werke/{work}/satz"));
    let head = topbar(satz);
    assert!(!head.contains(">Zurück zum Werk</a>"), "auf der Satz-Seite nicht doppelt in der Kopfzeile: {head}");
    let ribbon = &satz[satz.find("data-ribbon-satz").expect("Menüband der Satz-Seite")..];
    assert!(
        ribbon.contains(&format!("<a class=\"btn\" href=\"/?work={work}&amp;key={key}\">Zurück zum Werk</a>")),
        "Zurück zum Werk (am Abschnitt) im Menüband: {ribbon}"
    );
    assert!(ribbon.contains(&format!("href=\"/werke/{work}/pdf\" download")), "PDF im Menüband: {ribbon}");
    // Die Literaturseite: „Literatur“ auf /quelle führt zurück zur Liste (mit Werk und Schlüssel).
    let source = topbar(page("/quelle"));
    let back = &source[source.find("</nav>").unwrap()..];
    assert!(
        back.contains(&format!("href=\"/literatur?work={work}&amp;key={key}")),
        "Literatur-Link auf /quelle: {back}"
    );
    // Seitentitel wie bisher in .work-title.
    for (prefix, title) in [
        ("/werke", "Werke".to_owned()),
        ("/gliederung?", "Gliederung · Das Gartenbuch".to_owned()),
        ("/literatur", "Literatur · Das Gartenbuch".to_owned()),
        ("/einfaelle?", "Einfälle".to_owned()),
        ("POST", "Speichern bestätigen · Das Gartenbuch".to_owned()),
    ] {
        assert!(
            topbar(page(prefix)).contains(&format!("<span class=\"work-title\">{title}</span>")),
            "{prefix}: Seitentitel „{title}“ in .work-title"
        );
    }
    // Außerhalb der Arbeitsseite: keine Werkwahl, Literatur und Satz ohne Abschnitt, Werk aus der Seite.
    let assets = topbar(page(&format!("/werke/{work}/bilder")));
    assert!(!assets.contains("work-select"), "Werkwahl nur auf der Arbeitsseite");
    assert!(
        menu(assets, "werkzeuge").contains(&format!("href=\"/literatur?work={work}\" data-literature-open title=")),
        "Literatur ohne Popup: gewöhnlicher Link auf /literatur des Werks: {assets}"
    );
    assert!(menu(assets, "ansicht").contains(&format!("href=\"/werke/{work}/satz\"")), "Satz des Werks");
    assert!(menu(assets, "ansicht").contains(&format!("href=\"/?work={work}\">Editor</a>")), "Editor = Arbeitsseite");
    assert!(!assets.contains("data-work-import-open"), "Import-Dialog gibt es nur auf der Arbeitsseite");
}

#[tokio::test]
async fn without_any_work_the_work_entries_point_to_the_list_of_works() {
    let server = common::open().await;
    // Ohne Cookie und ohne `work` auf der Seite: /werke hat kein Werk.
    let (status, html) = server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "Werkliste lädt");
    let head = topbar(&html);
    assert!(
        head.contains("<a class=\"brand\" href=\"/\">Baruch</a>"),
        "Baruch führt auf / (Werk aus Cookie oder Vorgabe)"
    );
    let werk = menu(head, "werk");
    assert!(
        !werk.contains("/bearbeiten") && !werk.contains("/bilder") && !werk.contains("/export"),
        "keine Adresse mit erfundenem Werk: {werk}"
    );
    assert_eq!(
        werk.matches("href=\"/werke\"").count(),
        16,
        "Alle Werke und die fünfzehn Werk-Einträge (Runde 11: PDF, Umschlag, Umschlag-PDF; Runde 16: Stichwortzettel; \
         Runde 26: Austausch ausgeben; Runde 28: Befunde; Runde 20: EPUB; Runde 31: Glossar, Register; Runde 22: Als \
         Entwurf herunterladen) zeigen auf /werke: {werk}"
    );
    assert!(!werk.contains("/epub"), "auch das EPUB ohne erfundenes Werk: {werk}");
    assert!(
        menu(head, "ansicht").contains(
            "href=\"/werke\" title=\"Das EPUB des Werks in mehreren Readern nebeneinander\">EPUB-Vorschau</a>"
        ),
        "EPUB-Vorschau ohne Werk führt auf die Werkliste"
    );
    assert!(
        !werk.contains("/pdf") && !werk.contains("/umschlag"),
        "auch PDF und Umschlag ohne erfundenes Werk: {werk}"
    );
    // Runde 26: Einlesen braucht kein Werk und führt ohne Werk auf das Formular ohne `work`.
    assert!(
        werk.contains("<a class=\"menu-item\" role=\"menuitem\" href=\"/austausch/einlesen\">Austausch: Einlesen…</a>"),
        "Austausch einlesen ohne Werk: Formular ohne work: {werk}"
    );
    assert!(head.contains("href=\"/einstellungen/darstellung\" data-settings-open"), "Einstellungen ohne Werk");
    // Mit Cookie baruch_work: dasselbe Werk wie die Arbeitsseite.
    let (_, html) = get_with_cookie(&server, "/werke", &format!("baruch_work={}", server.state.work)).await;
    let w = server.state.work;
    assert!(
        menu(topbar(&html), "werk").contains(&format!("href=\"/werke/{w}/bearbeiten\"")),
        "das Werk aus dem Cookie baruch_work füllt die Werk-Einträge"
    );
}

#[tokio::test]
async fn badges_count_open_outline_requests_and_proposals() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let nodes = keys(&server, work).await;
    let (_, html) = server.get(&format!("/?work={work}")).await;
    let head = topbar(&html);
    assert!(
        head.contains("<span class=\"badge\" data-badge=\"outline_requests\" hidden>0</span>")
            && head.contains("<span class=\"badge\" data-badge=\"open_proposals\" hidden>0</span>"),
        "ohne Anträge und Vorschläge sind die Abzeichen verborgen: {head}"
    );
    assert!(head.contains("data-badge-item=\"outline_requests\" hidden>"), "der Eintrag „Anträge“ ist verborgen");
    let zero = server.rpc("menu.badges", json!({ "work": work })).await;
    assert_eq!(
        result(&zero),
        &json!({ "work": work, "outline_requests": 0, "open_proposals": 0, "open_findings": 0 }),
        "RPC menu.badges liefert dieselben Nullen"
    );

    // Ein Antrag über MCP (umbenennen wartet auf den Menschen) und zwei Vorschläge in zwei Abschnitten.
    let (summer, _) = nodes.iter().find(|(_, t)| t == "Sommer").unwrap().clone();
    let (seed, _) = nodes.iter().find(|(_, t)| t == "Aussaat").unwrap().clone();
    mcp_call(&server, "outline_rename", json!({ "work": work, "key": summer, "title": "Hochsommer" })).await;
    mcp_call(
        &server,
        "propose_revision",
        json!({ "work": work, "key": summer, "body": "= Sommer\n\nRegen, Regen.\n" }),
    )
    .await;
    mcp_call(&server, "propose_revision", json!({ "work": work, "key": seed, "body": "== Aussaat\n\nBohnen.\n" }))
        .await;

    let (_, html) = server.get(&format!("/?work={work}&key={}", nodes[0].0)).await;
    let head = topbar(&html);
    assert!(
        menu(head, "werk").contains("<summary class=\"btn\" aria-haspopup=\"menu\">Werk ▾ <span class=\"badge\" data-badge=\"outline_requests\">1</span> <span class=\"badge warning\" data-badge=\"open_findings\" title=\"offene Befunde im Werk\" hidden>0 offen</span></summary>"),
        "Abzeichen am Menü Werk: ein offener Antrag, serverseitig gezeichnet; das Abzeichen offener Befunde \
         (Runde 28) ist ohne Befunde verborgen: {head}"
    );
    assert!(
        menu(head, "ansicht").contains("Ansicht ▾ <span class=\"badge\" data-badge=\"open_proposals\">2</span>"),
        "Abzeichen am Menü Ansicht: zwei offene Vorschläge im ganzen Werk, nicht nur im gezeigten Abschnitt: {head}"
    );
    assert!(
        menu(head, "werk").contains(&format!(
            "href=\"/gliederung?work={work}#antraege\" data-badge-item=\"outline_requests\">Anträge zur Gliederung"
        )),
        "der Eintrag „Anträge zur Gliederung“ ist sichtbar und führt zu den Anträgen"
    );
    let counted = server.rpc("menu.badges", json!({ "work": work })).await;
    assert_eq!(
        result(&counted),
        &json!({ "work": work, "outline_requests": 1, "open_proposals": 2, "open_findings": 0 }),
        "RPC menu.badges liefert dieselben Zahlen wie die Arbeitsseite"
    );
    // Andere Seiten (Runde 23a): Der Server setzt die Zahlen auch dort ins ausgelieferte HTML (ohne JavaScript
    // sichtbar); menu.ts frischt sie nur noch auf.
    let (_, assets) = server.get(&format!("/werke/{work}/bilder")).await;
    assert!(
        topbar(&assets).contains("<span class=\"badge\" data-badge=\"open_proposals\">2</span>")
            && topbar(&assets).contains("<span class=\"badge\" data-badge=\"outline_requests\">1</span>"),
        "außerhalb der Arbeitsseite stehen dieselben Zahlen wie in menu.badges im HTML: {}",
        topbar(&assets)
    );
    // Ohne work das Vorgabewerk; falsche Parameter → -32602.
    let default = server.rpc("menu.badges", json!({})).await;
    assert_eq!(result(&default)["work"], json!(server.state.work), "ohne work das Vorgabewerk");
    for bad in [json!({ "work": 99999 }), json!({ "work": "eins" }), json!([1])] {
        let answer = server.rpc("menu.badges", bad.clone()).await;
        assert_eq!(answer["error"]["code"], -32602, "ungültige Parameter {bad} → -32602: {answer}");
    }
}

#[tokio::test]
async fn the_topbar_stays_on_one_line_at_1024_pixels() {
    let server = common::open().await;
    let (status, css) = server.get("/static/app.css").await;
    assert_eq!(status, StatusCode::OK, "Stil wird ausgeliefert");
    let rule = |selector: &str| -> &str {
        let at = css.find(&format!("\n{selector} {{")).unwrap_or_else(|| panic!("Regel {selector} fehlt"));
        let rest = &css[at..];
        &rest[..rest.find('}').unwrap()]
    };
    assert!(rule(".topbar").contains("flex-wrap: nowrap"), "die Kopfzeile bricht nie in eine zweite Zeile um");
    let controls = rule(".topbar > .btn, .topbar > .brand, .topbar > .work-select, .topbar > .menubar");
    assert!(controls.contains("flex-shrink: 0"), "Knöpfe, Werkwahl und Menüs schrumpfen nicht: {controls}");
    let title = rule(".topbar > .work-title, .topbar > .muted");
    assert!(
        title.contains("min-width: 0")
            && title.contains("overflow: hidden")
            && title.contains("text-overflow: ellipsis"),
        "der Werktitel kürzt sich mit „…“ und gibt den Platz frei: {title}"
    );
    assert!(rule(".menu").contains("flex-shrink: 0"), "kein Menü wird zusammengedrückt");
    assert!(
        rule(".menu-list").contains("position: absolute"),
        "die aufgeklappte Liste liegt über der Seite und braucht keinen Platz in der Zeile"
    );
    assert!(rule(".menubar").contains("flex-wrap: nowrap"), "die Menüs stehen nebeneinander");

    // Zahl der direkten Bedienelemente: früher elf Knöpfe, jetzt Marke, Werkwahl, Titel, Autor,
    // Menüleiste, Lücke, Thema, Einstellungen.
    let work = server.import_garden().await.work;
    let (_, html) = server.get(&format!("/?work={work}")).await;
    let head = topbar(&html);
    let inner = &head[head.find('>').unwrap() + 1..];
    let children = direct_children(inner);
    assert_eq!(children.len(), 8, "acht direkte Kinder der Kopfzeile auf der Arbeitsseite: {children:#?}");
    let controls = children.iter().filter(|t| t.contains("class=\"btn")).count();
    assert_eq!(controls, 2, "nur Thema und Einstellungen stehen als Knopf direkt in der Zeile: {children:#?}");
    let nav = &head[head.find("<nav").unwrap()..head.find("</nav>").unwrap()];
    let menus = direct_children(&nav[nav.find('>').unwrap() + 1..]);
    assert_eq!(menus.len(), 3, "in der Menüleiste genau drei Menüs: {menus:#?}");
}

/// Runde 18a: In schmalen Fenstern bricht die Kopfzeile um, statt Knöpfe aus dem Bild zu schieben. Ehrlich: Der
/// Test liest nur die CSS-Regeln (wie der Test für 1024 px darüber) und misst keine Breite; ob alles im Fenster
/// bleibt, zeigt erst die Sichtprüfung des Integrators bei 800, 1024 und 1400 px.
#[tokio::test]
async fn in_narrow_windows_the_topbar_wraps_instead_of_running_out_of_the_window() {
    let server = common::open().await;
    let (_, css) = server.get("/static/app.css").await;
    let media = css.find("@media (max-width: 999.98px) {").expect("eigene Medienregel für schmale Fenster");
    let block = &css[media..media + css[media..].find("\n}").unwrap()];
    assert!(
        block.contains(".topbar { flex-wrap: wrap; row-gap: 6px; }"),
        "unter 1000 px darf die Kopfzeile in eine zweite Zeile umbrechen: {block}"
    );
    assert!(block.contains(".topbar > .work-title { flex-basis: 12ch;"), "der Titel behält etwas Platz: {block}");
    // Ab 1000 px gilt weiter die einzeilige Grundregel (Test oben); die Medienregel liegt dahinter.
    let base = css.find("\n.topbar {").expect("Grundregel der Kopfzeile");
    assert!(base < media, "die Medienregel überschreibt die Grundregel, nicht umgekehrt");
    assert!(css[base..].starts_with("\n.topbar {\n  display: flex; flex-wrap: nowrap;"), "Grundregel einzeilig");
}

#[tokio::test]
async fn the_scripts_bring_the_menu_keyboard_and_the_theme_listens_to_settings() {
    let script = |name: &str| -> &str {
        baruch_server::SCRIPTS
            .iter()
            .find(|(file, _)| *file == name)
            .map(|(_, body)| *body)
            .unwrap_or_else(|| panic!("{name} fehlt"))
    };
    let menu = script("menu.js");
    for needle in [
        "register(\"menubar\"",
        "\"ArrowDown\"",
        "\"ArrowUp\"",
        "\"Home\"",
        "\"End\"",
        "\"Escape\"",
        "aria-expanded",
        "menu.badges",
    ] {
        assert!(menu.contains(needle), "menu.js enthält {needle} (ARIA-Menü-Tastatur, Abzeichen über RPC)");
    }
    assert!(script("main.js").contains("./menu.js"), "main.js lädt das Menü");
    let panes = script("panes.js");
    assert!(panes.contains("baruch:settings-changed"), "der Thema-Umschalter hört auf baruch:settings-changed");
    assert!(
        !baruch_server::SCRIPTS.iter().any(|(file, _)| *file == "theme.js"),
        "das Thema bleibt in panes.ts (keine eigene theme.ts)"
    );
}

/// Integrator Runde 9: Die Seiten, die settings auf `page_title`/`page_actions` umgestellt hat, behalten
/// jede Adresse ihrer früheren eigenen Kopfzeile (Stand development f851000) – im Menü oder in
/// `page_actions`, also ohne JavaScript erreichbar.
#[tokio::test]
async fn settings_pages_keep_the_addresses_of_their_former_topbars() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let cookie = format!("baruch_work={work}");
    let cases: Vec<(String, Vec<String>)> = vec![
        // work_edit.html: „Werke“, „Zurück zum Werk“ (back_url = Arbeitsseite des Werks).
        (format!("/werke/{work}/bearbeiten"), vec!["/werke".to_owned(), format!("/?work={work}")]),
        // work_numbering.html: „Werk ändern“, „Zurück zum Werk“.
        (format!("/werke/{work}/nummerierung"), vec![format!("/werke/{work}/bearbeiten"), format!("/?work={work}")]),
        // personas.html (heute Einstellungen → Assistenten): „Zurück zum Werk“.
        (format!("/einstellungen/assistenten?work={work}"), vec![format!("/?work={work}")]),
    ];
    for (uri, former) in cases {
        let (status, html) = get_with_cookie(&server, &uri, &cookie).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        let head = topbar(&html);
        let links = hrefs(head);
        for href in &former {
            assert!(
                links.contains(&href.as_str()),
                "{uri}: frühere Kopfzeilen-Adresse {href} ist in der neuen Kopfzeile erreichbar: {links:?}"
            );
        }
        assert!(
            links.contains(&format!("/werke/{work}/bearbeiten").as_str())
                && links.contains(&format!("/werke/{work}/nummerierung").as_str()),
            "{uri}: Werk bearbeiten und Gliederungsschema stehen zusätzlich im Menü Werk: {links:?}"
        );
    }
}

/// Die Abzeichen an den Menüs (`menu.css`: Text `--warning` auf `--warning-soft`, 11 px) sind normaler
/// Text: WCAG AA verlangt mindestens 4,5 : 1 – wie bei den Tönungen der Personas. Geprüft werden alle
/// Paare in `app.css` (helles Thema, dunkles Thema über die Systemvorgabe und über `data-theme`).
#[test]
fn menu_badges_keep_aa_contrast_in_both_themes() {
    use baruch_server::personas::{MIN_CONTRAST, contrast_ratio};
    let menu = include_str!("../static/menu.css");
    assert!(
        menu.matches("background: var(--warning-soft); color: var(--warning)").count() >= 2,
        "beide Abzeichen nehmen --warning auf --warning-soft"
    );
    let css = include_str!("../static/app.css");
    let value = |line: &str, name: &str| -> String {
        let at = line.find(&format!("{name}: ")).unwrap_or_else(|| panic!("{name} fehlt in {line}"));
        line[at + name.len() + 2..].split(';').next().unwrap().trim().to_owned()
    };
    let pairs: Vec<(String, String)> = css
        .lines()
        .filter(|l| l.contains("--warning:") && l.contains("--warning-soft:"))
        .map(|l| (value(l, "--warning"), value(l, "--warning-soft")))
        .collect();
    assert_eq!(pairs.len(), 3, "hell, dunkel (System), dunkel (data-theme): {pairs:?}");
    for (text, background) in &pairs {
        let ratio = contrast_ratio(text, background);
        assert!(ratio >= MIN_CONTRAST, "{text} auf {background}: {ratio:.2} : 1 < 4,5 : 1");
    }
}
