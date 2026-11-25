// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 18a (menueband): Menüband für das Bearbeiten, Formate der Werkvorlage als Knöpfe, Satz-Seite
//! (Abbruch, keine Läufe fremder Seiten, ohne main.js), Kapitel einer geteilten Seite, Kopfzeile, „neu“ als Wort
//! und Gedankenkarte (Beschriftung, Pfeil, Strichart).
//!
//! Alle Werke und Texte sind frei erfunden. Der Zwischenspeicher des Satzes ist prozessweit: Jeder Test setzt eine
//! eigene Schlusszeile, damit sein Stand nur ihm gehört. Läufe, die abgebrochen werden, sind absichtlich lang
//! (viele Seiten), damit der Abbruch sicher während des Laufs ankommt.

use crate::common;

use std::time::{Duration, Instant};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Werk aus `source` mit eigener Schlusszeile `tag` (eigener Stand) anlegen; (Werk, Titel → key).
async fn import(server: &common::Server, source: &str, tag: &str) -> (i64, Vec<(String, i64)>) {
    let report = server.import(&format!("{source}\n// Test: {tag}\n"), &format!("{tag}.typ")).await;
    let work = server.state.store.load_work(report.work).await.unwrap();
    (report.work, work.nodes.iter().map(|n| (n.title.clone(), n.key)).collect())
}

fn key_of(keys: &[(String, i64)], title: &str) -> i64 {
    keys.iter().find(|(t, _)| t == title).unwrap_or_else(|| panic!("kein Abschnitt {title}: {keys:?}")).1
}

/// Langes Werk: viele Seiten aus Typsts eingebautem Blindtext, damit ein Lauf lange genug dauert, um ihn abzubrechen.
fn slow_book() -> String {
    "#set page(width: 10cm, height: 14cm)\n\n= Sehr lang\n\n#for i in range(1500) [#lorem(90)\n\n]\n".to_owned()
}

async fn stand_of(server: &common::Server, work: i64) -> String {
    baruch_server::typeset::prepare(&server.state, work).await.unwrap().stand
}

/// Wartet, bis für `stand` ein Typst-Lauf begonnen hat (höchstens 60 s, andere Tests können die Sperre halten).
async fn wait_for_run(stand: &str) {
    let started = Instant::now();
    while baruch_server::typeset::runs_of(stand) == 0 {
        assert!(started.elapsed() < Duration::from_secs(60), "der Lauf beginnt nicht");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Wartet, bis die Typst-Sperre wieder frei ist: ein winziger eigener Satz braucht sie.
async fn wait_for_lock(tag: &str) {
    let source = baruch_render::Source { main: format!("= Sperre\n{tag}\n"), files: Vec::new() };
    let stand = baruch_render::stand(&source);
    baruch_server::typeset::compile(source, &stand, Duration::from_secs(120)).await.expect("Sperre wird frei");
}

fn get_from(uri: &str, site: &str) -> Request<Body> {
    Request::get(uri).header("sec-fetch-site", site).body(Body::empty()).unwrap()
}

// ---- Formate der Vorlage -----------------------------------------------------------------------------------

#[tokio::test]
async fn ribbon_formats_offer_scene_and_work_only_when_the_template_defines_them() {
    let server = common::open().await;
    let (with, _) = import(
        &server,
        "#let szene(body) = block(inset: 4pt, body)\n#let arbeit(body) = rect(body)\n\n= Kapitel\n\nText.\n",
        "formate-mit",
    )
    .await;
    let (without, _) = import(&server, "= Kapitel\n\nText.\n", "formate-ohne").await;

    let cmds = |answer: &Value| -> Vec<String> {
        result(answer)["formats"].as_array().unwrap().iter().map(|f| f["cmd"].as_str().unwrap().to_owned()).collect()
    };
    let answer = server.rpc("ribbon.formats", json!({ "work": with })).await;
    let list = cmds(&answer);
    assert_eq!(&list[..2], ["scene", "work"], "Szene und Arbeit aus der Vorlage zuerst: {list:?}");
    let scene = &result(&answer)["formats"][0];
    assert_eq!(scene["label"], "Szene", "deutsche Beschriftung: {scene}");
    assert_eq!(scene["snippet"], "#szene[\n\n]\n", "Typst-Text für den Quelltext: {scene}");
    assert!(scene["title"].as_str().unwrap().contains("#szene"), "der Titel nennt die Funktion: {scene}");

    let list = cmds(&server.rpc("ribbon.formats", json!({ "work": without })).await);
    assert!(!list.contains(&"scene".to_owned()) && !list.contains(&"work".to_owned()), "ohne #let keine: {list:?}");
    for always in ["footnote", "quote", "list"] {
        assert!(list.contains(&always.to_owned()), "{always} gibt es immer: {list:?}");
    }
    let unknown = server.rpc("ribbon.formats", json!({ "work": 999_999 })).await;
    assert_eq!(unknown["error"]["code"], -32602, "unbekanntes Werk → -32602: {unknown}");
}

#[tokio::test]
async fn the_editor_ribbon_keeps_every_hook_and_shows_the_formats_of_the_template() {
    let server = common::open().await;
    let (work, keys) =
        import(&server, "#let szene(body) = block(inset: 4pt, body)\n\n= Kapitel\n\nText.\n", "editor-band").await;
    let key = key_of(&keys, "Kapitel");
    let (status, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite lädt");
    // Runde 33 (Menüband): Die Leiste hat jetzt zwei Teile mit Gruppen (geschachtelte <div>), darum reicht der
    // Ausschnitt bis zur Suchleiste dahinter statt bis zum ersten </div>. Alle Haken sind dieselben.
    let start = page.find("<div class=\"ws-bar edit-toolbar ribbon\"").expect("Leiste des Editors");
    let ribbon =
        &page[start..start + page[start..].find("<div class=\"search-bar\"").expect("Suchleiste hinter der Leiste")];
    // DOM-Vertrag §2: die Haken des Satzbilds bleiben unverändert (wysiwyg.ts liest sie).
    for hook in [
        "data-edit-toolbar hidden>",
        "data-ws-mode=\"satz\"",
        "data-ws-mode=\"quelltext\"",
        "class=\"ws-tools\" data-ws-tools",
        "data-ws-format",
        "data-ws-cmd=\"italic\"",
        "data-ws-cmd=\"bold\"",
    ] {
        assert!(ribbon.contains(hook), "{hook} steht im Menüband: {ribbon}");
    }
    assert!(page.contains("data-ws-hint"), "der Hinweis des Satzbilds bleibt in der Statuszeile");
    assert!(ribbon.contains("aria-label=\"Menüband für das Bearbeiten\""), "das Menüband heißt so: {ribbon}");
    assert!(ribbon.contains("data-widget=\"ribbon\""), "das Widget „ribbon“ sitzt im Menüband: {ribbon}");
    assert!(
        ribbon.contains("data-ribbon-cmd=\"scene\" data-ribbon-snippet=\"#szene[\n\n]\n\" title=\"Szene nach der Werkvorlage (#szene)\">Szene</button>"),
        "Knopf „Szene“ mit Befehl und Typst-Text: {ribbon}"
    );
    assert!(!ribbon.contains("data-ribbon-cmd=\"work\""), "die Vorlage definiert kein #arbeit: {ribbon}");
    for cmd in ["footnote", "quote", "list"] {
        assert!(ribbon.contains(&format!("data-ribbon-cmd=\"{cmd}\"")), "Knopf {cmd} immer da: {ribbon}");
    }
    assert!(ribbon.contains("data-ribbon-image"), "Knopf „Bild“ öffnet das Einfügen: {ribbon}");
    assert!(
        // Runde 33: das Ziel trägt zusätzlich den Anker id="bild-einfuegen" (Hinweis ohne JavaScript im Reiter Einfügen).
        page.contains("<details class=\"image-insert\" id=\"bild-einfuegen\" data-image-insert"),
        "das Ziel des Knopfs „Bild“ steht im Editor: {page}"
    );
    assert!(!ribbon.contains("data-ribbon-satz"), "im Editor nicht die Fassung der Satz-Seite");
}

#[tokio::test]
async fn the_ribbon_widget_sends_commands_inserts_snippets_and_says_honestly_what_does_not_work() {
    let server = common::open().await;
    let (status, js) = server.get("/static/js/ribbon.js").await;
    assert_eq!(status, StatusCode::OK, "ribbon.js wird ausgeliefert (aus ribbon.ts übersetzt)");
    assert!(js.contains("register(\"ribbon\""), "das Widget ist registriert");
    assert!(js.contains("sendWsCommand(root, cmd, arg)"), "im Satzbild nur über baruch:ws-command (ws_command.ts)");
    assert!(js.contains("data-ws-open"), "die Ansicht entscheidet: offenes Satzbild oder Quelltext");
    assert!(js.contains("geht im Satzbild noch nicht"), "handled=false → ehrlicher Hinweis statt Stille");
    assert!(js.contains("\"insertText\"") && js.contains("setRangeText"), "Einfügen im Quelltext an der Cursorstelle");
    assert!(js.contains("details[data-image-insert]"), "„Bild“ öffnet nur das Formular von satzbild-werk");
    assert!(!js.contains("wysiwyg.js"), "kein Import von wysiwyg.ts (Vertrag §4)");
    assert!(!js.contains("STUB"), "der S1-Stub ist weg");
    let (_, main) = server.get("/static/js/main.js").await;
    assert!(main.contains("import \"./ribbon.js\""), "main.js lädt das Widget");
}

// ---- Satz-Seite --------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_typeset_page_has_an_accessible_ribbon_without_main_js() {
    let server = common::open().await;
    let (work, keys) =
        import(&server, "= Eins\n\nErster Text.\n\n#pagebreak()\n\n= Zwei\n\nZweiter.\n", "satz-band").await;
    let key = key_of(&keys, "Eins");
    let (status, html) = server.get(&format!("/werke/{work}/satz?key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite lädt: {html}");
    assert!(!html.contains("main.js"), "die Satz-Seite lädt kein main.js (sie braucht kein Widget): {html}");
    assert!(!html.contains("<script"), "überhaupt kein Skript: Menüband aus Links und einem Formular");
    // Runde 33: Das eigene Band unter der Kopfzeile (edit_toolbar.html, Zweig satz_page, aria-label „Menüband der
    // Satz-Ansicht“) ist entfernt; die Bedienung steht als Gruppen im Reiter „Buch“ des Menübands (typeset.html, Block
    // band_buch, Haken data-ribbon-satz an der ersten Gruppe). Die Zusicherungen gelten darum für das Feld „Buch“;
    // „nie verborgen“ heißt jetzt: das Feld Buch ist auf der Satz-Seite das offene (ohne JavaScript, vom Server).
    assert!(!html.contains("aria-label=\"Menüband der Satz-Ansicht\""), "kein zweites Band unter dem Kopf: {html}");
    let ribbon = &html[html.find("data-band-panel=\"buch\"").expect("Feld Buch")..];
    let ribbon = &ribbon[..ribbon.find("</header>").expect("Ende des Kopfs")];
    let open_tag = &ribbon[..ribbon.find('>').unwrap()];
    assert!(
        !open_tag.contains("hidden"),
        "das Feld Buch ist auf der Satz-Seite offen (ohne JavaScript bedienbar): {open_tag}"
    );
    assert!(ribbon.contains("data-ribbon-satz"), "der Haken der Satz-Bedienung steht im Feld Buch: {ribbon}");
    assert!(ribbon.contains(&format!("href=\"/?work={work}&amp;key={key}\">Zurück zum Werk</a>")), "Zurück: {ribbon}");
    assert!(ribbon.contains(&format!("href=\"/werke/{work}/pdf\" download")), "PDF herunterladen: {ribbon}");
    assert!(
        ribbon.contains("aria-disabled=\"true\" aria-label=\"Vorige Seite (dies ist die erste)\">‹</span>"),
        "‹ auf Seite 1 gesperrt, mit Beschriftung für Bildschirmleser: {ribbon}"
    );
    assert!(
        ribbon.contains(&format!(
            "<a class=\"btn\" href=\"/werke/{work}/satz?key={key}&amp;page=2\" rel=\"next\" aria-label=\"Nächste Seite\""
        )),
        "› mit aria-label: {ribbon}"
    );
    assert!(ribbon.contains("Seite 1 von 2"), "Seitenangabe im Menüband: {ribbon}");
    assert!(
        ribbon.contains(&format!(
            "<form class=\"typeset-cancel\" method=\"post\" action=\"/werke/{work}/satz/abbrechen\">"
        )),
        "„Satz abbrechen“ ist ein Formular (ohne JavaScript): {ribbon}"
    );
    assert!(ribbon.contains(&format!("name=\"key\" value=\"{key}\"")), "der Abschnitt reist mit: {ribbon}");
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(page.contains("/static/js/main.js"), "die Arbeitsseite lädt main.js weiter (Block scripts in base.html)");
}

#[tokio::test]
async fn the_typeset_page_header_works_without_main_js() {
    // Befund nach Runde 18: Ohne main.js täte der Thema-Knopf (Widget „theme“) nichts, und Dialog/Alt+, der
    // Einstellungen gäbe es nicht. Die Kopfzeile der Satz-Seite bietet darum nur, was ohne JavaScript geht.
    let server = common::open().await;
    let (work, keys) = import(&server, "= Eins\n\nText für die Kopfzeile.\n", "satz-kopfzeile").await;
    let key = key_of(&keys, "Eins");
    let (status, html) = server.get(&format!("/werke/{work}/satz?key={key}")).await;
    assert_eq!(status, StatusCode::OK, "Satz-Seite lädt: {html}");
    let header = &html[html.find("<header class=\"band\"").expect("Kopfzeile")..]; // Runde 33: der Kopf ist das Menüband (header.band statt header.topbar)
    let header = &header[..header.find("</header>").unwrap()];
    assert!(
        // Runde 33: vorher „kein <button im Kopf“. Im Kopf steht jetzt der Absende-Knopf von „Satz abbrechen“
        // (type="submit", geht ohne JavaScript); verboten bleiben Knöpfe, die nur ein Skript bedient (type="button").
        !header.contains("data-widget=\"theme\"") && !header.contains("type=\"button\""),
        "kein Knopf in der Kopfzeile, der nur mit main.js etwas täte (die Menüs sind <details>/Links): {header}"
    );
    assert!(!header.contains("<noscript>"), "nichts nur für „ohne JavaScript“: die Seite ist immer ohne: {header}");
    assert!(
        header.contains(&format!(
            "<a class=\"btn\" href=\"/einstellungen/darstellung?work={work}\" aria-label=\"Thema einstellen (Einstellungen → Darstellung)\""
        )),
        "das Thema ist ein Link auf die Darstellung (dort ein Formular ohne JavaScript): {header}"
    );
    assert!(header.contains(">◐</a>"), "ohne Cookie zeigt der Link das Zeichen für „System“: {header}");
    assert!(
        !header.contains("aria-keyshortcuts") && !header.contains("aria-haspopup=\"dialog\""),
        "kein versprochenes Alt+, und kein Dialog, die bräuchten main.js: {header}"
    );
    let (_, page) = server.get(&format!("/?work={work}&key={key}")).await;
    assert!(page.contains("data-widget=\"theme\""), "auf der Arbeitsseite bleibt der Umschalter ein Widget");
    assert!(page.contains("aria-keyshortcuts=\"Alt+Comma\""), "und die Einstellungen mit Alt+,");
}

#[tokio::test]
async fn the_cancel_form_sits_behind_the_origin_guard_and_redirects_without_a_new_run() {
    let server = common::open().await;
    let (work, keys) = import(&server, "= Eins\n\nText.\n", "abbrechen-formular").await;
    let key = key_of(&keys, "Eins");
    let foreign = Request::post(format!("/werke/{work}/satz/abbrechen"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://fremd.example")
        .header(header::HOST, "127.0.0.1:8765")
        .body(Body::from(format!("key={key}")))
        .unwrap();
    let (status, _, _) = server.send(foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremder Origin → 403 (guard::same_origin)");

    let (status, location) = server.post_form(&format!("/werke/{work}/satz/abbrechen"), &format!("key={key}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Abbrechen → 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/satz?abgebrochen=0&key={key}").as_str()),
        "lief kein Satz: abgebrochen=0, der Abschnitt bleibt"
    );
    let stand = stand_of(&server, work).await;
    let (status, html) = server.get(location.as_deref().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "die Seite danach lädt: {html}");
    assert!(html.contains("Es lief kein Satz, der abzubrechen war."), "ehrliche Meldung: {html}");
    assert!(html.contains("Neu setzen</a>"), "Link zum neuen Satz: {html}");
    assert!(!html.contains("<img"), "ohne gesetzten Stand keine Seite");
    assert_eq!(baruch_server::typeset::runs_of(&stand), 0, "die Seite nach dem Abbrechen setzt nicht neu");
    let (status, _) = server.post_form("/werke/999999/satz/abbrechen", "").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn cancelling_a_running_typeset_returns_at_once_and_caches_nothing() {
    let server = common::open().await;
    let (work, _) = import(&server, &slow_book(), "abbruch-seite").await;
    let stand = stand_of(&server, work).await;
    let app = server.app.clone();
    let uri = format!("/werke/{work}/satz");
    let page = tokio::spawn(async move {
        let response = app.oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap(), Instant::now())
    });
    wait_for_run(&stand).await;
    assert_eq!(
        baruch_server::typeset::running_in(server.path(), work),
        1,
        "genau ein angemeldeter Lauf für dieses Werk dieses Servers"
    );
    let (status, location) = server.post_form(&format!("/werke/{work}/satz/abbrechen"), "").await;
    let cancelled_at = Instant::now();
    assert_eq!(status, StatusCode::SEE_OTHER, "Abbrechen → 303");
    assert_eq!(location.as_deref(), Some(format!("/werke/{work}/satz?abgebrochen=1").as_str()), "abgebrochen=1");

    let (status, html, answered_at) = page.await.unwrap();
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "die wartende Seite bekommt keinen Satz: {html}");
    assert!(html.contains("Der Satz wurde abgebrochen."), "sie sagt warum: {html}");
    assert!(
        answered_at.saturating_duration_since(cancelled_at) < Duration::from_secs(2),
        "die Anfrage kehrt gleich nach dem Abbruch zurück, ohne auf Typst zu warten: {:?}",
        answered_at.saturating_duration_since(cancelled_at)
    );
    wait_for_lock("abbruch-seite").await;
    assert!(!baruch_server::typeset::cache_contains(&stand), "ein abgebrochener Lauf legt nichts ab");
    assert_eq!(baruch_server::typeset::running_in(server.path(), work), 0, "kein Lauf mehr angemeldet");

    let (status, html) = server.get(location.as_deref().unwrap()).await;
    assert_eq!(status, StatusCode::OK, "Seite nach dem Abbruch: {html}");
    assert!(html.contains("Der Satz wurde abgebrochen. Es wurde nicht neu gesetzt."), "Meldung: {html}");
    assert!(html.contains(&format!("<a href=\"/werke/{work}/satz\">Neu setzen</a>")), "Neu setzen: {html}");
    assert_eq!(baruch_server::typeset::runs_of(&stand), 1, "die Seite nach dem Abbruch startet keinen zweiten Lauf");
}

#[tokio::test]
async fn preview_cancel_stops_a_running_preview_over_rpc() {
    let server = common::open().await;
    let (work, _) = import(&server, &slow_book(), "abbruch-rpc").await;
    let stand = stand_of(&server, work).await;
    let idle = server.rpc("preview.cancel", json!({ "work": work })).await;
    assert_eq!(result(&idle)["cancelled"], false, "ohne laufenden Satz: cancelled false");

    let app = server.app.clone();
    let running = tokio::spawn(async move { common::rpc(app, "preview.typst", json!({ "work": work })).await });
    wait_for_run(&stand).await;
    let answer = server.rpc("preview.cancel", json!({ "work": work })).await;
    assert_eq!(result(&answer)["cancelled"], true, "der laufende Satz wird abgebrochen: {answer}");
    let preview = running.await.unwrap();
    assert_eq!(preview["error"]["code"], -32000, "abgebrochen → REFUSED: {preview}");
    assert_eq!(preview["error"]["message"], "Satz abgebrochen.", "Meldung: {preview}");
    let again = server.rpc("preview.cancel", json!({ "work": work })).await;
    assert_eq!(result(&again)["cancelled"], false, "danach läuft nichts mehr");
    let unknown = server.rpc("preview.cancel", json!({ "work": 999_999 })).await;
    assert_eq!(unknown["error"]["code"], -32602, "unbekanntes Werk → -32602: {unknown}");
    wait_for_lock("abbruch-rpc").await;
    assert!(!baruch_server::typeset::cache_contains(&stand), "nichts abgelegt");
}

#[tokio::test]
async fn foreign_pages_cannot_start_a_typeset_or_a_pdf() {
    let server = common::open().await;
    let (work, _) = import(&server, "= Eins\n\nText für fremde Seiten.\n", "fremde-seite").await;
    let stand = stand_of(&server, work).await;
    for site in ["cross-site", "same-site"] {
        for uri in
            [format!("/werke/{work}/satz"), format!("/werke/{work}/satz/{stand}/1.svg"), format!("/werke/{work}/pdf")]
        {
            let (status, _, body) = server.send(get_from(&uri, site)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{uri} mit Sec-Fetch-Site {site} → 403: {body}");
            assert!(body.contains("fremden Seite"), "Meldung nennt den Grund: {body}");
        }
    }
    assert_eq!(baruch_server::typeset::runs_of(&stand), 0, "keine der abgelehnten Anfragen hat Typst gestartet");
    assert!(!baruch_server::typeset::cache_contains(&stand), "und nichts liegt im Zwischenspeicher");
    {
        let _serial = common::PDF_SERIAL.lock().await;
        let before = baruch_server::pdf::pdf_runs();
        let (status, _, _) = server.send(get_from(&format!("/werke/{work}/pdf"), "cross-site")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "PDF von fremder Seite → 403");
        assert_eq!(baruch_server::pdf::pdf_runs(), before, "keine PDF-Erzeugung begonnen");
    }
    // Eigene Seite (same-origin), Aufruf aus der Adresszeile (none) und ohne Kopf (curl, Tests): unverändert.
    let (status, _, html) = server.send(get_from(&format!("/werke/{work}/satz"), "same-origin")).await;
    assert_eq!(status, StatusCode::OK, "eigene Seite setzt: {html}");
    assert_eq!(baruch_server::typeset::runs_of(&stand), 1, "genau ein Lauf – die Zählung funktioniert");
    let (status, _, _) = server.send(get_from(&format!("/werke/{work}/satz/{stand}/1.svg"), "none")).await;
    assert_eq!(status, StatusCode::OK, "Aufruf aus der Adresszeile (none) bekommt die Seite");
    let (status, _) = server.get(&format!("/werke/{work}/satz/{stand}/1.svg")).await;
    assert_eq!(status, StatusCode::OK, "ohne Sec-Fetch-Site wie bisher");
}

#[tokio::test]
async fn foreign_pages_cannot_start_cover_notes_or_map_runs() {
    // Befund nach Runde 18: Umschlag (Seite, SVG, PDF), Stichwortzettel und Gedankenkarte als PDF halten dieselbe
    // Typst-Sperre wie der Satz, ohne dass `preview.cancel` sie erreicht – eine fremde Seite darf sie nicht anstoßen.
    let server = common::open().await;
    let (work, _) = import(&server, "= Eins\n\nText für Umschlag, Zettel und Karte.\n", "fremde-seite-pdf").await;
    let uris = [
        format!("/werke/{work}/umschlag"),
        format!("/werke/{work}/umschlag.svg"),
        format!("/werke/{work}/umschlag.pdf"),
        format!("/werke/{work}/stichwortzettel.pdf"),
        format!("/werke/{work}/karte.pdf"),
    ];
    let _serial = common::PDF_SERIAL.lock().await;
    let before = baruch_server::pdf::pdf_runs();
    for site in ["cross-site", "same-site"] {
        for uri in &uris {
            let (status, _, body) = server.send(get_from(uri, site)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{uri} mit Sec-Fetch-Site {site} → 403: {body}");
            assert!(body.contains("fremden Seite"), "Meldung nennt den Grund: {body}");
        }
    }
    assert_eq!(baruch_server::pdf::pdf_runs(), before, "keine der abgelehnten Anfragen hat ein PDF begonnen");
    // Gegenprobe: Von der eigenen Seite (same-origin) entstehen PDFs und Umschlag wie bisher – der Schutz sperrt
    // nicht pauschal, und die Zählung erfasst diese Wege.
    for uri in [format!("/werke/{work}/stichwortzettel.pdf"), format!("/werke/{work}/karte.pdf")] {
        // Ein PDF ist kein UTF-8: Antwort direkt lesen statt über `send`.
        let response = server.app.clone().oneshot(get_from(&uri, "same-origin")).await.unwrap();
        let (status, content_type) = (response.status(), response.headers().get(header::CONTENT_TYPE).cloned());
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{uri} von der eigenen Seite: {}", String::from_utf8_lossy(&bytes));
        assert_eq!(content_type.unwrap(), "application/pdf", "{uri} liefert ein PDF");
        assert!(bytes.starts_with(b"%PDF"), "{uri}: der Inhalt ist ein PDF");
    }
    assert_eq!(baruch_server::pdf::pdf_runs(), before + 2, "genau die zwei eigenen PDF-Erzeugungen sind gezählt");
    let (status, headers, body) = server.send(get_from(&format!("/werke/{work}/umschlag.svg"), "same-origin")).await;
    assert_eq!(status, StatusCode::OK, "Umschlag-Vorschau von der eigenen Seite: {body}");
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "image/svg+xml", "als SVG");
}

// ---- Kapitel einer geteilten Seite -------------------------------------------------------------------------

/// Drei Kapitel auf kleinen Seiten mit lebendem Kolumnentitel: „Anfang“ füllt mehr als eine Seite, „Mitte“ beginnt
/// mitten auf der Seite, auf der „Anfang“ endet, und endet mit einem Seitenumbruch vor „Ende“.
fn shared_book() -> String {
    let mut text = String::from(
        "#set page(width: 8cm, height: 9cm, header: context { let h = query(heading.where(level: 1).before(here())); \
         if h.len() > 0 [#h.last().body] })\n#set text(size: 9pt)\n\n= Anfang\n\n",
    );
    for i in 0..7 {
        text.push_str(&format!(
            "Absatz {i}: Die Gärtnerin zählt Beete, misst Wege und notiert, wo im Frühjahr Bohnen stehen sollen.\n\n"
        ));
    }
    text.push_str("= Mitte\n\nEin kurzer Absatz in der Mitte.\n\n#pagebreak()\n\n= Ende\n\nSchluss.\n");
    text
}

#[tokio::test]
async fn a_page_two_chapters_share_counts_for_both_and_a_running_head_does_not() {
    let server = common::open().await;
    let (work, keys) = import(&server, &shared_book(), "geteilte-seite").await;
    let typeset = baruch_server::typeset::typeset(&server.state, work).await.expect("das Werk setzt");
    assert!(!typeset.rendered().has_errors(), "fehlerfrei: {:?}", typeset.rendered().diagnostics);
    let (anfang, mitte, ende) = (key_of(&keys, "Anfang"), key_of(&keys, "Mitte"), key_of(&keys, "Ende"));
    let shared = typeset.page_of(Some(mitte));
    assert!(shared >= 2, "Voraussetzung: „Anfang“ füllt mehr als eine Seite, „Mitte“ beginnt auf Seite {shared}");
    assert_eq!(typeset.page_of(Some(ende)), shared + 1, "Voraussetzung: „Ende“ beginnt nach dem Umbruch neu");

    let lines = |page: usize| typeset.rendered().page_lines.get(page - 1).cloned().unwrap_or_default();
    let starts: Vec<usize> = typeset.prepared.map.sections.iter().map(|s| s.first_line).collect();
    let heads: Vec<usize> = typeset.rendered().headings.iter().map(|h| h.line).collect();
    let why = format!(
        "Zeilen der geteilten Seite {:?}, davor {:?}, Abschnitte ab {starts:?}, Überschriften {heads:?}",
        lines(shared),
        lines(shared - 1)
    );
    assert_eq!(
        typeset.chapter(Some(anfang)),
        (1, shared),
        "die Seite, auf der „Anfang“ endet und „Mitte“ beginnt, zählt auch zu „Anfang“ (vorher nur zu „Mitte“); {why}"
    );
    assert_eq!(
        typeset.chapter(Some(mitte)),
        (shared, shared),
        "„Mitte“ beginnt und endet auf der geteilten Seite; {why}"
    );
    assert_eq!(
        typeset.chapter(Some(ende)),
        (shared + 1, typeset.page_count()),
        "„Ende“ beginnt nach dem Umbruch; der Kolumnentitel „Mitte“ oben auf seiner Seite macht sie nicht zu „Mitte“"
    );
    // Dasselbe über RPC und auf der Seite (Seitenliste des Kapitels).
    let answer = server.rpc("preview.typst", json!({ "work": work, "key": anfang })).await;
    assert_eq!(result(&answer)["chapter"], json!({ "first": 1, "last": shared }), "RPC wie die Seite: {answer}");
    let (_, html) = server.get(&format!("/werke/{work}/satz?key={anfang}")).await;
    let list = &html[html.find("typeset-chapter").expect("Seitenliste")..];
    assert!(
        list.contains(&format!("href=\"/werke/{work}/satz?key={anfang}&amp;page={shared}\">{shared}</a>")),
        "die geteilte Seite steht in der Seitenliste von „Anfang“: {list}"
    );
}

// ---- Kopfzeile, „neu“, Schriften ---------------------------------------------------------------------------

#[tokio::test]
async fn new_posts_are_marked_with_the_word_neu_not_only_by_colour() {
    let server = common::open().await;
    let (_, js) = server.get("/static/js/chat.js").await;
    assert!(js.contains("post-new-label"), "das Widget hängt ein eigenes Element an neue Beiträge");
    assert!(js.contains("label.textContent = \"neu\""), "mit dem Wort „neu“ (Farbe allein reicht nicht)");
    let (_, css) = server.get("/static/app.css").await;
    assert!(css.contains(".post-new-label {"), "das Wort ist gestaltet");
}

#[tokio::test]
async fn warming_the_fonts_returns_at_once() {
    let started = Instant::now();
    baruch_server::typeset::warm_fonts();
    assert!(started.elapsed() < Duration::from_millis(500), "die Schriftsuche läuft im Hintergrund");
}

// ---- Gedankenkarte ----------------------------------------------------------------------------------------

#[tokio::test]
async fn map_edges_carry_a_visible_label_an_arrow_for_directed_kinds_and_their_own_stroke() {
    let server = common::open().await;
    let (work, keys) = import(&server, "= Rosen\n\nText.\n\n= Dornen\n\nText.\n", "karte-kanten").await;
    let idea = |key: i64, words: &str| {
        let server = &server;
        let words = words.to_owned();
        async move {
            let answer = server.rpc("idea.create", json!({ "work": work, "key": key, "words": words })).await;
            result(&answer)["idea"]["id"].as_i64().unwrap()
        }
    };
    let a = idea(key_of(&keys, "Rosen"), "Sonne").await;
    let b = idea(key_of(&keys, "Dornen"), "Schatten").await;
    let c = idea(key_of(&keys, "Dornen"), "Regen").await;
    for (from, to, kind) in [(a, b, "leads_to"), (b, c, "contradicts")] {
        let answer = server.rpc("idea.link", json!({ "work": work, "from": from, "to": to, "kind": kind })).await;
        assert!(answer["error"].is_null(), "Verbindung {kind}: {answer}");
    }
    // Runde 34: Die Gedankenkarte ist die Mindmap; aus Kanten (`im-edge`) wurden Beziehungslinien (`mm-link`).
    let (status, html) = server.get(&format!("/werke/{work}/karte")).await;
    assert_eq!(status, StatusCode::OK, "Mindmap lädt: {html}");
    assert!(html.contains("<marker id=\"mm-arrow\""), "Pfeilspitze für gerichtete Beziehungen: {html}");
    let line = |kind: &str| -> String {
        let at = html.find(&format!("class=\"mm-link mm-link-{kind}")).unwrap_or_else(|| panic!("{kind}"));
        html[at..at + html[at..].find('>').unwrap()].to_owned()
    };
    let leads = line("leads_to");
    assert!(leads.contains("marker-end=\"url(#mm-arrow)\""), "gerichtet mit Spitze: {leads}");
    assert!(
        leads.contains(&format!("data-from=\"i:{a}\" data-to=\"i:{b}\"")),
        "die Linie nennt beide Einfälle: {leads}"
    );
    let contra = line("contradicts");
    assert!(!contra.contains("marker-end"), "„widerspricht“ ist ungerichtet, ohne Spitze: {contra}");
    assert!(contra.contains("mm-contradiction"), "und hervorgehoben: {contra}");
    assert!(contra.contains("stroke-dasharray=\"8 3 2 3\""), "Strich-Punkt als Attribut an der Linie: {contra}");
    assert!(
        html.contains("class=\"mm-link-label\"") && html.contains(">führt weiter</text>"),
        "sichtbare Beschriftung von „führt weiter“ (nicht nur <title>): {html}"
    );
    assert!(html.contains(">widerspricht</text>"), "auch „widerspricht“ ist beschriftet");
    let last_link = html.rfind("class=\"mm-rel").expect("Gruppe einer Beziehung");
    let first_node = html.find("<g class=\"mm-node").expect("Knoten");
    assert!(last_link < first_node, "Knoten liegen über den Linien (zuletzt gezeichnet)");
    assert!(html.contains("im-key-leads_to"), "die Legende nennt die Arten der Beziehungen");

    // Eigenständige Datei (auch Quelle des PDF): Farben als Attribute, eingebetteter Stil für die Beschriftung.
    let (status, svg) = server.get(&format!("/werke/{work}/karte.svg")).await;
    assert_eq!(status, StatusCode::OK, "karte.svg");
    assert!(svg.contains(".mm-link-label"), "Stil der Beschriftung eingebettet");
    assert!(svg.contains("stroke=\"#b3261e\""), "Widerspruch mit fester Farbe (ohne CSS-Variablen)");
    let (status, mm) = server.get(&format!("/werke/{work}/karte.mm")).await;
    assert_eq!(status, StatusCode::OK, "karte.mm");
    assert!(
        mm.contains("<arrowlink ") && mm.contains("COLOR=\"#"),
        "FreeMind: Beziehung als Pfeil mit Farbe je Art: {mm}"
    );
}

// Runde 34: Die Seitenleiste der Gedankenkarte (`.im-side`, Test the_map_side_panel_starts_below_the_toolbar) gibt es
// nicht mehr; die Mindmap hat keine Leiste über dem Bild (Ersatz: round34_widget).

#[test]
fn typeset_and_numbering_pages_scroll_at_the_window_edge() {
    // Ursache des hellen Streifens: .works-page (works.css) ist Scrollbehälter und auf 1100px begrenzt – die
    // Bildlaufleiste stand mitten im Fenster. Geprüft wird die Regel, nicht das Bild (Sichtprüfung).
    for (file, css) in [
        ("typeset.css", include_str!("../static/typeset.css")),
        ("numbering.css", include_str!("../static/numbering.css")),
    ] {
        assert!(
            css.contains("max-width: none; padding-inline: max(18px, calc((100% - 1100px) / 2 + 18px));"),
            "{file}"
        );
    }
}
