// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 9, task „menu“: one shared header in `base.html`, page-local links in
//! `{% block page_actions %}`, small theme toggle, „Einstellungen“ (Alt+,), badges
//! (requests on the outline, open proposals) and the shell of the settings dialog at the end of the page.
//!
//! Round 33 (ribbon, contract R33 §4): the header with the menus Werk ▾, Ansicht ▾ and Werkzeuge ▾ has become the
//! ribbon (`header.band`: menu Baruch ▾ and the tabs Start · Einfügen · Prüfen · Buch · Ansicht). The
//! assertions of this file have therefore been rewritten for the new place, none has been dropped: what stood „im
//! Menü Werk“ here is now demanded in the menu Baruch ▾ or in the panel of the tab (reason per place). The full
//! mapping with a reason per entry is in `round33_menueband.rs`.
//! All test texts freely invented (`tests/data/gartenbuch.typ` and the texts here).

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

/// The header of the page (from the opening tag to `</header>`). Round 33: `header.band` instead of `header.topbar`.
fn topbar(html: &str) -> &str {
    let start = html.find("<header class=\"band\"").unwrap_or_else(|| panic!("kein Menüband: {html}"));
    let end = start + html[start..].find("</header>").expect("Kopf ohne Ende");
    &html[start..end]
}

/// The menu „Baruch ▾“ (`<details class="menu" data-menu="baruch">` up to `</details>`).
fn app_menu(head: &str) -> &str {
    let marker = "<details class=\"menu\" data-menu=\"baruch\">";
    let start = head.find(marker).unwrap_or_else(|| panic!("Menü Baruch fehlt: {head}"));
    let end = start + head[start..].find("</details>").expect("Menü ohne Ende");
    &head[start..end]
}

/// Panel of a tab (`data-band-panel="ID"` up to the next panel or the dock).
fn panel<'a>(head: &'a str, id: &str) -> &'a str {
    let marker = format!("data-band-panel=\"{id}\"");
    let start = head.find(&marker).unwrap_or_else(|| panic!("Feld {id} fehlt: {head}"));
    let rest = &head[start + marker.len()..];
    let end =
        rest.find("<div class=\"band-panel\"").or_else(|| rest.find("<div class=\"band-dock\"")).unwrap_or(rest.len());
    &rest[..end]
}

fn hrefs(html: &str) -> Vec<&str> {
    html.split("href=\"").skip(1).filter_map(|rest| rest.split('"').next()).collect()
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

/// Stable keys of the sections in tree order.
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

/// All pages with a header (acceptance Round 9): address → HTML, in the garden book with image and idea.
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
    // Integrator Round 28: an own post for the posts page (chat-auftrag).
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
    // Integrator Round 30: a variant group (two sibling proposals) for the comparison page /varianten (chat-loesungen),
    // created directly through the store because the page shows only an existing group.
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
        // Integrator Round 11: cover page (cover).
        format!("/werke/{work}/umschlag"),
        // Integrator Round 9: pages from settings (edit work, outline scheme, settings).
        format!("/werke/{work}/bearbeiten"),
        format!("/werke/{work}/nummerierung"),
        format!("/einstellungen/darstellung?work={work}"),
        format!("/einstellungen/werk?work={work}"),
        format!("/einstellungen/assistenten?work={work}"),
        format!("/einstellungen/verbindungen?work={work}"),
        format!("/einstellungen/quellen?work={work}"),
        format!("/einstellungen/mcp?work={work}"),
        format!("/einstellungen/daten?work={work}"),
        // Round 13 (abgleich): page of the reconciliation with MultiGPT.
        format!("/abgleich?work={work}"),
        // Integrator Round 13: document view of MultiGPT (quellen) and evidence page of the web search (websuche).
        format!("/quelle/multigpt?work={work}&key={key}&collection=Nachbau&document=doc-1"),
        format!("/websuche/belege?work={work}&key={key}&post=1"),
        // Integrator Round 15 (auftraege): threads without a section and the Bible page of the literature (without a
        // Bible an honest hint, the page loads nevertheless).
        format!("/faeden?work={work}"),
        format!("/literatur/bibel?work={work}&key={key}"),
        // Round 16c (stichwortzettel): page of the cue sheet with format choice and preview.
        format!("/werke/{work}/stichwortzettel"),
        // Round 17c (pruefanzeige): check display of the quotations (without a Bible an honest hint, the page loads).
        format!("/werke/{work}/pruefung?key={key}"),
        // Round 26 (interface): write the exchange (page of the work) and read it (form).
        format!("/werke/{work}/austausch"),
        "/austausch/einlesen".to_owned(),
        // Round 25 (karte-bild): idea map of the ideas.
        format!("/werke/{work}/karte"),
        // Integrator Round 28: findings across the work (befund-uebersicht), whole chat history and posts page without
        // JavaScript (chat-auftrag).
        format!("/werke/{work}/befunde"),
        format!("/beitraege?work={work}&key={key}"),
        format!("/beitrag?work={work}&id={post}"),
        // Integrator Round 30: work search (suchen-verweise), cross-references (suchen-verweise) and the comparison of
        // the variants (chat-loesungen).
        format!("/suche?work={work}&q=Laube"),
        format!("/verweise?work={work}"),
        format!("/varianten?work={work}&key={key}&group={group}"),
        // Round 20b (reader): EPUB preview per reader.
        format!("/werke/{work}/epub/vorschau"),
        // Integrator Round 20: check report of the EPUB (epub-pruefung).
        format!("/werke/{work}/epub/pruefung"),
        // Round 31 (interface R31-S1): glossary, index and form at the selection (kontextmenue).
        format!("/werke/{work}/glossar"),
        format!("/werke/{work}/register"),
        format!("/werke/{work}/markierung?key={key}"),
        // Integrator Round 22: archive of the deleted works (befund-rueckfrage) and history of a revision (fassungen).
        "/werke/archiv".to_owned(),
        format!("/werke/{work}/fassungen/verlauf?key={key}&rev=1"),
        // Round 24 (pakete): Über Baruch.
        "/ueber".to_owned(),
    ] {
        let (status, html) = get_with_cookie(server, &uri, &format!("baruch_work={work}")).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
        pages.push((uri, html));
    }
    // Confirmation page: saving without JavaScript would split the section (new heading).
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
/// Round 33: formerly „genau eine Kopfzeile mit den drei Menüs“ – now exactly one ribbon with the menu Baruch ▾ and the
/// tabs; the remaining assertions (widget menubar, theme, settings, hooks, no node= links) continue to hold at the new
/// place.
async fn every_page_has_exactly_one_band_with_the_app_menu_and_tabs() {
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
            html.matches("<header class=\"band\"").count(),
            1,
            "{uri}: genau ein Menüband (die eigene Kopfzeile der Seite ist entfernt, base.html zeichnet es): {html}"
        );
        assert!(!html.contains("class=\"topbar\""), "{uri}: die alte Kopfzeile steht nicht mehr daneben");
        let head = topbar(html);
        assert!(
            // Round 23a: outside the work page the bar additionally carries the work for the badges.
            head.contains("<nav class=\"menubar\" data-widget=\"menubar\" aria-label=\"Menü\""),
            "{uri}: Menüleiste als Widget menubar mit Namen für Hilfsmittel: {head}"
        );
        // Round 33: instead of the three menus werk/ansicht/werkzeuge exactly one menu „Baruch ▾“ – with the same
        // properties (one <details>, <summary> with aria-haspopup, list role=menu, no buttons).
        assert_eq!(
            head.matches("<details class=\"menu\"").count(),
            1,
            "{uri}: genau ein Menü (ein <details>, klappt ohne JavaScript auf)"
        );
        let m = app_menu(head);
        assert!(
            m.contains("<summary class=\"btn\" aria-haspopup=\"menu\""),
            "{uri}: das Menü öffnet über ein <summary> mit aria-haspopup=menu: {m}"
        );
        assert!(m.contains("<div class=\"menu-list\" role=\"menu\">"), "{uri}: Liste mit role=menu: {m}");
        assert!(
            !m.contains("<button"),
            "{uri}: das Menü enthält keine Knöpfe, die nur mit JavaScript etwas tun – alles Links: {m}"
        );
        // Round 33: the entries of the old menus are in the panels of the tabs; every page has at least Start and
        // Ansicht, with the work from page or cookie also Prüfen and Buch – as links (role=tab).
        for id in ["start", "pruefen", "buch", "ansicht"] {
            assert_eq!(
                head.matches(&format!("<a class=\"band-tab\" role=\"tab\" id=\"band-tab-{id}\"")).count(),
                1,
                "{uri}: Reiter {id} genau einmal (Link, ohne JavaScript bedienbar): {head}"
            );
            assert_eq!(head.matches(&format!("data-band-panel=\"{id}\"")).count(), 1, "{uri}: Feld {id} genau einmal");
        }
        assert_eq!(head.matches("aria-selected=\"true\"").count(), 1, "{uri}: genau ein Reiter ist gewählt: {head}");
        assert!(
            html.contains(" data-band=\"open\">"),
            "{uri}: <html> trägt den Zustand des Bands (Schicht band::apply)"
        );
        // The typesetting page loads no main.js (Round 18a): there theme and settings are plain links (follow-up fix
        // Round 18, checked in round18_menueband::the_typeset_page_header_works_without_main_js).
        if uri.contains("/satz") {
            assert!(head.contains("aria-label=\"Thema einstellen"), "{uri}: Thema als Link ohne Widget: {head}");
        } else {
            assert!(
                panel(head, "ansicht").contains("data-widget=\"theme\""),
                "{uri}: kleiner Thema-Umschalter bleibt im Kopf (Runde 33: Reiter Ansicht, Gruppe Darstellung)"
            );
            // Round 33: „Einstellungen“ is an entry of the menu Baruch ▾ (before: a button on the right in the line);
            // target, Alt+, and the hook data-settings-open are the same.
            assert!(
                app_menu(head).contains(&format!(
                    "<a class=\"menu-item settings-open\" role=\"menuitem\" href=\"/einstellungen/darstellung?work={work}\" data-settings-open \
                     aria-haspopup=\"dialog\" aria-controls=\"settings\" aria-keyshortcuts=\"Alt+Comma\" \
                     title=\"Einstellungen (Alt+,)\" data-shortcut=\"Alt+,\">⚙ Einstellungen</a>"
                )),
                "{uri}: Einstellungen mit Werk (aus Seite oder Cookie), Alt+, und Ziel ohne JavaScript: {head}"
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
    // Round 33: places of the former menu entries – menu Baruch ▾ (program) and the panels of the tabs.
    let baruch = app_menu(head);
    let start = panel(head, "start");
    let pruefen = panel(head, "pruefen");
    let buch = panel(head, "buch");
    // Work choice as before (GET form), only on the work page.
    assert!(
        head.contains("<form class=\"work-select\" action=\"/\" method=\"get\"")
            && head.contains("<select name=\"work\" aria-label=\"Werk\" data-widget=\"work-select\">")
            && head.contains("<noscript><button class=\"btn\" type=\"submit\">Öffnen</button></noscript>"),
        "Werkwahl unverändert in der Kopfzeile: {head}"
    );
    // Round 33: the lettering „Baruch“ (link to the work) has become the menu Baruch ▾; on the work page one is already
    // at the work, on other pages „Zum Text“ (tab Start) leads there – see
    // other_pages_keep_their_local_links_in_page_actions.
    assert!(baruch.contains(">Baruch ▾</summary>"), "Baruch steht als Menü vorn im Kopf: {head}");
    assert!(
        head.contains(&format!(" data-work=\"{work}\"")),
        "der Kopf nennt das Werk (menu.ts liest es für die Abzeichen): {head}"
    );
    assert!(head.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "Werktitel wie bisher: {head}");
    assert!(
        head.contains("<span class=\"muted band-author\">· Grete Gärtner</span>"),
        "Autor daneben, kürzt sich zuerst"
    );
    for (entry, place) in [
        ("href=\"/werke\">Alle Werke</a>", baruch),
        (
            "href=\"/werke/neu\" data-work-import-open aria-haspopup=\"dialog\" aria-controls=\"work-import\">Neues Werk / Importieren…</a>",
            baruch,
        ),
        (&format!("href=\"/werke/{work}/bearbeiten\">Werk bearbeiten</a>"), buch),
        (&format!("href=\"/gliederung?work={work}\">Gliederung</a>"), buch),
        (&format!("href=\"/gliederung?work={work}#antraege\""), buch),
        (&format!("href=\"/werke/{work}/nummerierung\">Gliederungsschema</a>"), buch),
        (&format!("href=\"/werke/{work}/bilder\">Bilder</a>"), buch),
        (&format!("href=\"/werke/{work}/export\""), buch),
        // Round 11: the book as PDF, the cover page and the cover PDF (download as attachment).
        (&format!("href=\"/werke/{work}/pdf\" download"), buch),
        (&format!("href=\"/werke/{work}/umschlag\" title"), buch),
        (&format!("href=\"/werke/{work}/umschlag.pdf\" download"), buch),
        (
            &format!(
                "data-nav data-view-toggle href=\"/?work={work}&key={key}&rev={rev}&view=preview&amp;lq=Beete\" title=\"Zwischen Editor und Vorschau im Browser wechseln\">Vorschau</a>"
            ),
            start,
        ),
        (&format!("href=\"/werke/{work}/satz?key={key}\""), buch),
        (
            &format!(
                "href=\"/literatur?work={work}&key={key}&rev={rev}&amp;lq=Beete\" data-literature-open aria-haspopup=\"dialog\" aria-controls=\"literature\""
            ),
            pruefen,
        ),
        (
            &format!(
                "href=\"/einfaelle?work={work}\" data-idea-open aria-haspopup=\"dialog\" aria-controls=\"idea-capture\" title=\"Einfall festhalten (Alt+E)\""
            ),
            start,
        ),
        (&format!("href=\"/einfaelle?work={work}\" title=\"Liste der Einfälle dieses Werks\">Einfälle</a>"), start),
        // Round 25 (karte-bild): idea map, only with a work; since Round 34 it is called „Mindmap“ (same address).
        (
            &format!(
                "href=\"/werke/{work}/karte\" title=\"Mindmap der Einfälle dieses Werks (mit der Wolke als Ablage)\">Mindmap</a>"
            ),
            start,
        ),
    ] {
        // Round 33: entries in the menu Baruch ▾ are still `a.menu-item`, entries in the panels of the tabs `a.btn`.
        assert!(
            place.contains(&format!("<a class=\"menu-item\" role=\"menuitem\" {entry}"))
                || place.contains(&format!("<a class=\"btn\" {entry}")),
            "Eintrag {entry} steht als gewöhnlicher Link (ohne JavaScript bedienbar) am richtigen Ort des Bands: {place}"
        );
    }
    // Round 33: „Satz“ is called „Satz-Vorschau“ in the tab Buch (next to „EPUB-Vorschau“); „Literatur“ as before.
    assert!(buch.contains(">Satz-Vorschau</a>") && pruefen.contains(">Literatur</a>"), "Beschriftungen: {buch}");
    // Preview: the toggle points back to the editor.
    let (_, preview) = server.get(&format!("/?work={work}&key={key}&view=preview")).await;
    let toggle = panel(topbar(&preview), "start");
    assert!(
        toggle.contains(&format!("data-nav data-view-toggle href=\"/?work={work}&key={key}&rev={rev}\" title=\"Zwischen Editor und Vorschau im Browser wechseln\">Editor</a>")),
        "in der Vorschau führt der Umschalter zum Editor (Key-Form, ohne view): {toggle}"
    );
    // Round 20b (reader): „EPUB-Vorschau“ is no longer a stub but a link to the preview of the work; next to it „EPUB“
    // as an attachment next to the PDF. Round 33: both in the tab Buch (groups Satz and Ausgabe); in the group Ausgabe
    // the entries are called briefly „PDF“, „Entwurf“, „EPUB“ (the „herunterladen“ is in the title).
    assert!(
        buch.contains(&format!(
            "<a class=\"btn\" href=\"/werke/{work}/epub/vorschau\" title=\"Das EPUB des Werks in mehreren Readern nebeneinander\">EPUB-Vorschau</a>"
        )),
        "EPUB-Vorschau verlinkt die Seite /werke/W/epub/vorschau (ohne JavaScript bedienbar): {buch}"
    );
    assert!(
        buch.contains(&format!(
            "<a class=\"btn\" href=\"/werke/{work}/epub\" download title=\"Das ganze Werk als E-Book herunterladen (EPUB 3, aktuelle Fassungen)\">EPUB</a>"
        )),
        "EPUB herunterladen: Anhang /werke/W/epub in der Gruppe Ausgabe: {buch}"
    );
    let pdf = buch.find(">PDF</a>").expect("PDF im Reiter Buch");
    let epub = buch.find(">EPUB</a>").expect("EPUB im Reiter Buch");
    assert!(pdf < epub, "EPUB steht neben dem PDF (dahinter)");
    // Round 13 (abgleich): „Abgleichen“ is no longer a stub but an ordinary link to /abgleich (Round 33: tab Prüfen,
    // group Quellen).
    assert!(
        pruefen.contains(&format!(
            "<a class=\"btn\" href=\"/abgleich?work={work}\" title=\"Abgleich mit MultiGPT\">Abgleichen</a>"
        )),
        "Abgleichen verlinkt die Seite /abgleich des Werks (ohne JavaScript bedienbar): {pruefen}"
    );
    assert!(!head.contains("aria-disabled=\"true\" title=\"folgt"), "im Band ist kein Eintrag mehr ein Stub: {head}");
    assert!(
        !pruefen.contains("aria-disabled") && !buch.contains("aria-disabled") && !baruch.contains("aria-disabled"),
        "in Prüfen, Buch und im Menü Baruch ist nichts gesperrt (früher: Menüs Ansicht und Werkzeuge)"
    );
    // Round 24 (pakete): „Über Baruch“ as an ordinary link (Round 33: in the menu Baruch ▾ instead of at the end of
    // Werkzeuge).
    assert!(
        baruch.contains(
            "<a class=\"menu-item\" role=\"menuitem\" href=\"/ueber\" title=\"Version, Lizenz, Quellcode und Bibliotheken Dritter\">Über Baruch</a>"
        ),
        "Über Baruch verlinkt die Seite /ueber (ohne JavaScript bedienbar): {baruch}"
    );
    // Round 33: formerly „statt elf Knöpfen nur noch der Thema-Umschalter als Knopf“ in the one line. In the band line
    // 1 is still free of loose buttons: only the menu, the tabs, the work choice, the title and the switch open/closed
    // are there; all entries lie in groups of the panels.
    let top = &head[..head.find("<div class=\"band-panels\"").expect("Felder des Bands")];
    assert_eq!(top.matches("<a class=\"btn").count(), 1, "in Zeile 1 nur der Schalter auf/zu als Knopf-Link: {top}");
    assert_eq!(top.matches("<button class=\"btn\" type=\"button\"").count(), 0, "kein loser Knopf in Zeile 1: {top}");
}

#[tokio::test]
async fn other_pages_keep_their_local_links_in_page_actions() {
    let server = common::open().await;
    let Pages { work, key, pages } = all_pages(&server).await;
    let page = |prefix: &str| -> &str {
        &pages.iter().find(|(uri, _)| uri.starts_with(prefix)).unwrap_or_else(|| panic!("Seite {prefix}")).1
    };
    // All former header addresses of the pages (state 596ba72), now after the menu bar (Round 33: in the panel of the
    // tab Start; the task „seiten“ puts them into groups there or distributes them over the tabs – therefore only
    // checked is that the link is in the header after the menu bar).
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
                // back_url is a value (not template text): minijinja escapes „/“ as &#x2f; as before.
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
    // Round 18a: the typesetting page carries „Zurück zum Werk“ and „PDF herunterladen“ in the ribbon of the typeset
    // view (hook data-ribbon-satz) – the same addresses, only one place. Round 33: the bar moves with the task „seiten“
    // as groups into the tab Buch; therefore the place at the hook is demanded (no matter whether it is in the header
    // or below it) and that „Zurück zum Werk“ is not on the page twice (before: „nicht zusätzlich in der Kopfzeile“).
    let satz = page(&format!("/werke/{work}/satz"));
    assert_eq!(
        satz.matches(">Zurück zum Werk</a>").count(),
        1,
        "auf der Satz-Seite steht „Zurück zum Werk“ genau einmal"
    );
    let ribbon = &satz[satz.find("data-ribbon-satz").expect("Menüband der Satz-Seite")..];
    assert!(
        ribbon.contains(&format!("href=\"/?work={work}&amp;key={key}\">Zurück zum Werk</a>")),
        "Zurück zum Werk (am Abschnitt) im Menüband: {ribbon}"
    );
    assert!(ribbon.contains(&format!("href=\"/werke/{work}/pdf\" download")), "PDF im Menüband: {ribbon}");
    // The literature page: „Literatur“ on /quelle leads back to the list (with work and key).
    let source = topbar(page("/quelle"));
    let back = &source[source.find("</nav>").unwrap()..];
    assert!(
        back.contains(&format!("href=\"/literatur?work={work}&amp;key={key}")),
        "Literatur-Link auf /quelle: {back}"
    );
    // Page title as before in .work-title.
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
    // Outside the work page: no work choice, literature and typesetting without a section, work from the page.
    let assets = topbar(page(&format!("/werke/{work}/bilder")));
    assert!(!assets.contains("work-select"), "Werkwahl nur auf der Arbeitsseite");
    assert!(
        panel(assets, "pruefen").contains(&format!("href=\"/literatur?work={work}\" data-literature-open title=")),
        "Literatur ohne Popup: gewöhnlicher Link auf /literatur des Werks (Runde 33: Reiter Prüfen): {assets}"
    );
    assert!(
        panel(assets, "buch").contains(&format!("href=\"/werke/{work}/satz\"")),
        "Satz des Werks (Runde 33: Reiter Buch)"
    );
    // Round 33: „Ansicht ▾ → Editor“ (= work page) is called „Zum Text“ outside the work page and is under Start; there
    // it also replaces the lettering „Baruch“ as the way back to the work.
    assert!(
        panel(assets, "start").contains(&format!(
            "href=\"/?work={work}\" title=\"Zur Arbeitsseite mit Gliederung, Editor und Assistenten\">Zum Text</a>"
        )),
        "Zum Text = Arbeitsseite des Werks: {assets}"
    );
    assert!(!assets.contains("data-work-import-open"), "Import-Dialog gibt es nur auf der Arbeitsseite");
}

#[tokio::test]
async fn without_any_work_the_work_entries_point_to_the_list_of_works() {
    let server = common::open().await;
    // Without a cookie and without `work` on the page: /werke has no work.
    let (status, html) = server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "Werkliste lädt");
    let head = topbar(&html);
    // Round 33 (contract §4: „ohne Werk Start · Ansicht“): formerly the fifteen work entries were in the menu Werk even
    // without a work and all pointed to /werke („Erst ein Werk wählen“). Now the tabs Prüfen and Buch do not exist
    // without a work; in their place there is ONE entry „Werk wählen“ with the same target and the same hint. The
    // assertions „keine Adresse mit erfundenem Werk“ continue to hold for the whole header.
    assert!(
        !head.contains("data-band-tab=\"pruefen\"") && !head.contains("data-band-tab=\"buch\""),
        "ohne Werk keine Reiter Prüfen und Buch: {head}"
    );
    assert!(
        panel(head, "start")
            .contains("<a class=\"btn\" href=\"/werke\" title=\"Erst ein Werk wählen\">Werk wählen</a>"),
        "ohne Werk führt der Weg zu allem, was ein Werk braucht, auf die Werkliste: {head}"
    );
    assert!(
        !head.contains("/bearbeiten") && !head.contains("/bilder") && !head.contains("/export"),
        "keine Adresse mit erfundenem Werk: {head}"
    );
    assert!(!head.contains("/epub"), "auch das EPUB ohne erfundenes Werk: {head}");
    assert!(
        !head.contains("/pdf") && !head.contains("/umschlag") && !head.contains("/satz"),
        "auch PDF, Umschlag und Satz ohne erfundenes Werk: {head}"
    );
    assert!(app_menu(head).contains("href=\"/werke\">Alle Werke</a>"), "Alle Werke bleibt im Menü Baruch ▾: {head}");
    // Round 26: importing needs no work and leads without a work to the form without `work`.
    assert!(
        app_menu(head).contains("<a class=\"menu-item\" role=\"menuitem\" href=\"/austausch/einlesen\" title="),
        "Austausch einlesen ohne Werk: Formular ohne work: {head}"
    );
    assert!(head.contains("href=\"/einstellungen/darstellung\" data-settings-open"), "Einstellungen ohne Werk");
    // With the cookie baruch_work: the same work as the work page.
    let (_, html) = get_with_cookie(&server, "/werke", &format!("baruch_work={}", server.state.work)).await;
    let w = server.state.work;
    assert!(
        panel(topbar(&html), "buch").contains(&format!("href=\"/werke/{w}/bearbeiten\"")),
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
    // Round 33: the badges are at the tabs (Buch: requests, Start: proposals) instead of at the menu buttons.
    assert!(
        head.contains("<span class=\"badge\" data-badge=\"outline_requests\" title=\"offene Anträge zur Gliederung\" hidden>0</span>")
            && head.contains("<span class=\"badge\" data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\" hidden>0</span>"),
        "ohne Anträge und Vorschläge sind die Abzeichen verborgen: {head}"
    );
    assert!(head.contains("data-badge-item=\"outline_requests\" hidden>"), "der Eintrag „Anträge“ ist verborgen");
    let zero = server.rpc("menu.badges", json!({ "work": work })).await;
    assert_eq!(
        result(&zero),
        &json!({ "work": work, "outline_requests": 0, "open_proposals": 0, "open_findings": 0 }),
        "RPC menu.badges liefert dieselben Nullen"
    );

    // One request over MCP (renaming waits for the human) and two proposals in two sections.
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
    // Round 33: formerly at the menu button „Werk ▾“ (requests, „0 offen“ hidden) and „Ansicht ▾“ (proposals) – now at
    // the tabs Buch, Prüfen and Start; numbers and hooks are the same.
    assert!(
        head.contains(">Buch <span class=\"badge\" data-badge=\"outline_requests\" title=\"offene Anträge zur Gliederung\">1</span></a>"),
        "Abzeichen am Reiter Buch: ein offener Antrag, serverseitig gezeichnet: {head}"
    );
    assert!(
        head.contains(">Prüfen <span class=\"badge warning\" data-badge=\"open_findings\" title=\"offene Befunde im Werk\" hidden>0 offen</span></a>"),
        "das Abzeichen offener Befunde (Runde 28) ist ohne Befunde verborgen: {head}"
    );
    assert!(
        head.contains(">Start <span class=\"badge\" data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\">2</span></a>"),
        "Abzeichen am Reiter Start: zwei offene Vorschläge im ganzen Werk, nicht nur im gezeigten Abschnitt: {head}"
    );
    assert!(
        panel(head, "buch").contains(&format!(
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
    // Other pages (Round 23a): the server puts the numbers into the delivered HTML there as well (visible without
    // JavaScript); menu.ts only refreshes them.
    let (_, assets) = server.get(&format!("/werke/{work}/bilder")).await;
    assert!(
        topbar(&assets).contains(
            "<span class=\"badge\" data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\">2</span>"
        ) && topbar(&assets).contains(
            "<span class=\"badge\" data-badge=\"outline_requests\" title=\"offene Anträge zur Gliederung\">1</span>"
        ),
        "außerhalb der Arbeitsseite stehen dieselben Zahlen wie in menu.badges im HTML: {}",
        topbar(&assets)
    );
    // Without work the default work; wrong parameters → -32602.
    let default = server.rpc("menu.badges", json!({})).await;
    assert_eq!(result(&default)["work"], json!(server.state.work), "ohne work das Vorgabewerk");
    for bad in [json!({ "work": 99999 }), json!({ "work": "eins" }), json!([1])] {
        let answer = server.rpc("menu.badges", bad.clone()).await;
        assert_eq!(answer["error"]["code"], -32602, "ungültige Parameter {bad} → -32602: {answer}");
    }
}

/// Round 33: formerly „die Kopfzeile bleibt bei 1024 px einzeilig“ (rules `.topbar` in app.css). The header has become
/// the ribbon, its rules are in band.css; the intent of the old assertions holds for line 1 of the band: controls do
/// not shrink, the title shortens itself with „…“, the menu list lies above the page. Honestly: the test only reads CSS
/// rules and measures no width.
#[tokio::test]
async fn the_first_row_of_the_band_keeps_its_controls_and_shortens_the_title() {
    let server = common::open().await;
    let (status, css) = server.get("/static/app.css").await;
    assert_eq!(status, StatusCode::OK, "Stil wird ausgeliefert");
    let rule = |selector: &str| -> &str {
        let at = css.find(&format!("\n{selector} {{")).unwrap_or_else(|| panic!("Regel {selector} fehlt"));
        let rest = &css[at..];
        &rest[..rest.find('}').unwrap()]
    };
    let controls = rule(".band-top > .menubar, .band-top > .work-select, .band-top > .band-toggle");
    assert!(controls.contains("flex: 0 0 auto"), "Menü, Werkwahl und Schalter schrumpfen nicht: {controls}");
    assert!(rule(".band-top > .band-tabs").contains("flex: 0 0 auto"), "die Reiter schrumpfen nicht");
    let title = rule(".band-top > .work-title, .band-top > .band-author");
    assert!(
        title.contains("min-width: 0")
            && title.contains("overflow: hidden")
            && title.contains("text-overflow: ellipsis"),
        "der Werktitel kürzt sich mit „…“ und gibt den Platz frei: {title}"
    );
    assert!(rule(".band-top > .band-author").contains("flex-shrink: 100"), "der Autor weicht zuerst");
    assert!(rule(".menu").contains("flex-shrink: 0"), "das Menü wird nicht zusammengedrückt");
    assert!(
        rule(".menu-list").contains("position: absolute"),
        "die aufgeklappte Liste liegt über der Seite und braucht keinen Platz in der Zeile"
    );
    assert!(rule(".menubar").contains("flex-wrap: nowrap"), "die Menüleiste bricht nicht um");

    // Number of controls in line 1: formerly eight direct children of the header (two buttons, three menus) – now menu
    // bar with one menu, tab row, work choice, title, author, gap, switch.
    let work = server.import_garden().await.work;
    let (_, html) = server.get(&format!("/?work={work}")).await;
    let head = topbar(&html);
    let top = &head[head.find("<div class=\"band-top\">").expect("Zeile 1")
        ..head.find("<div class=\"band-panels\"").expect("Zeile 2")];
    for piece in [
        "<nav class=\"menubar\"",
        "<nav class=\"band-tabs\"",
        "<form class=\"work-select\"",
        "<span class=\"work-title\">",
        "<span class=\"muted band-author\">",
        "<span class=\"grow\">",
        "<a class=\"btn band-toggle\"",
    ] {
        assert_eq!(top.matches(piece).count(), 1, "Zeile 1 enthält genau einmal {piece}: {top}");
    }
    assert_eq!(
        top.matches("<details class=\"menu\"").count(),
        1,
        "in der Menüleiste genau ein Menü (früher drei): {top}"
    );
    assert_eq!(top.matches("role=\"tab\"").count(), 5, "fünf Reiter auf der Arbeitsseite: {top}");
}

/// Round 18a: in narrow windows the header wraps instead of pushing buttons out of view. Round 33: the media rule
/// `@media (max-width: 999.98px)` for `.topbar` has been dropped (contract §2: the modes depend only on the attributes
/// of <html>, no media queries). The band fulfils the same intent without a media rule: line 1 and the panels may
/// always wrap. Honestly: the test only reads the CSS rules and measures no width; whether everything stays in the
/// window is shown only by the visual check of the integrator.
#[test]
fn in_narrow_windows_the_band_wraps_instead_of_running_out_of_the_window() {
    let css = include_str!("../static/band.css");
    let rule = |selector: &str| -> &str {
        let at = css.find(&format!("\n{selector} {{")).unwrap_or_else(|| panic!("Regel {selector} fehlt"));
        let rest = &css[at..];
        &rest[..rest.find('}').unwrap()]
    };
    assert!(
        rule(".band-top").contains("flex-wrap: wrap"),
        "Zeile 1 darf in eine zweite Zeile umbrechen: {}",
        rule(".band-top")
    );
    assert!(
        rule(".band-panels").contains("flex-wrap: wrap"),
        "die Gruppen eines Felds brechen um, statt aus dem Fenster zu laufen"
    );
    assert!(rule(".band-top > .work-title").contains("min-width: 6ch"), "der Titel behält etwas Platz");
    assert!(!css.contains("@media"), "keine Medienregel im Band (eine Wahrheit: die Attribute an <html>)");
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

/// Integrator Round 9: the pages that settings has moved to `page_title`/`page_actions` keep every address of their
/// former own header (state development f851000) – in the menu or in `page_actions`, i.e. reachable without JavaScript.
#[tokio::test]
async fn settings_pages_keep_the_addresses_of_their_former_topbars() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let cookie = format!("baruch_work={work}");
    let cases: Vec<(String, Vec<String>)> = vec![
        // work_edit.html: „Werke“, „Zurück zum Werk“ (back_url = work page of the work).
        (format!("/werke/{work}/bearbeiten"), vec!["/werke".to_owned(), format!("/?work={work}")]),
        // work_numbering.html: „Werk ändern“, „Zurück zum Werk“.
        (format!("/werke/{work}/nummerierung"), vec![format!("/werke/{work}/bearbeiten"), format!("/?work={work}")]),
        // personas.html (today Einstellungen → Assistenten): „Zurück zum Werk“.
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
                "{uri}: frühere Kopfzeilen-Adresse {href} ist im Kopf (Menüband) erreichbar: {links:?}"
            );
        }
        assert!(
            links.contains(&format!("/werke/{work}/bearbeiten").as_str())
                && links.contains(&format!("/werke/{work}/nummerierung").as_str()),
            "{uri}: Werk bearbeiten und Gliederungsschema stehen zusätzlich im Reiter Buch: {links:?}"
        );
    }
}

/// The badges at the menus (`menu.css`: text `--warning` on `--warning-soft`, 11 px) are normal text: WCAG AA demands
/// at least 4.5 : 1 – as with the tints of the personas. Checked are all pairs in `app.css` (light theme, dark theme
/// through the system default and through `data-theme`).
#[test]
fn menu_badges_keep_aa_contrast_in_both_themes() {
    use baruch_server::personas::{MIN_CONTRAST, contrast_ratio};
    // Round 33: the badges are at the tabs and at entries of the panels (band.css); the rule of the entries in the menu
    // (menu.css) stays.
    let menu = include_str!("../static/menu.css");
    let band = include_str!("../static/band.css");
    assert!(
        menu.matches("background: var(--warning-soft); color: var(--warning)").count() >= 1
            && band.matches("background: var(--warning-soft); color: var(--warning)").count() >= 2,
        "die Abzeichen an Reitern und Einträgen nehmen --warning auf --warning-soft"
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
