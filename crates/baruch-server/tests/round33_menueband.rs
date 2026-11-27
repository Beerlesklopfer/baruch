// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 33 – ribbon (task „menueband“, contract §4/§5): band in base.html, band::apply, docking of the editor buttons.
//!
//! Checked is the delivered HTML (without JavaScript) and – in the Node mock, without a browser – the behaviour of the
//! compiled widgets band/band-dock on the real work page of the server. Every assertion with its reason, only invented
//! works (text below), no network, no Eliezer.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_store::{FindingKind, NewFinding};
use serde_json::{Value, json};

use crate::common;
use crate::round18_satzbild::run_harness;

/// Freely invented work: a boatyard with two chapters.
const WERFT: &str = "#set document(title: \"Die Bootswerft am Schilfsee\", author: \"Henrike Planke\")\n\n\
= Kiel legen\n\nDer Kiel aus Eiche liegt seit dem Morgen auf den Böcken.\n\n\
== Spanten\n\nZwölf Spanten warten gedämpft im Schuppen.\n\n\
= Stapellauf\n\nAm Sonntag rutscht das Boot ins Wasser.\n";

const SCENES: &str = include_str!("round23_web/widgets.mjs");

fn ok(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none_or(Value::is_null), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// The band of the page (from the opening tag to `</header>`).
fn band(html: &str) -> &str {
    let start = html.find("<header class=\"band\"").unwrap_or_else(|| panic!("kein Menüband: {html}"));
    let end = start + html[start..].find("</header>").expect("Band ohne Ende");
    &html[start..end]
}

/// One panel of the band (`data-band-panel="ID"` up to the next panel or the dock).
fn panel<'a>(head: &'a str, id: &str) -> &'a str {
    let marker = format!("data-band-panel=\"{id}\"");
    let start = head.find(&marker).unwrap_or_else(|| panic!("Feld {id} fehlt: {head}"));
    let rest = &head[start + marker.len()..];
    let end =
        rest.find("<div class=\"band-panel\"").or_else(|| rest.find("<div class=\"band-dock\"")).unwrap_or(rest.len());
    &rest[..end]
}

/// The menu „Baruch ▾“.
fn app_menu(head: &str) -> &str {
    let start = head
        .find("<details class=\"menu\" data-menu=\"baruch\">")
        .unwrap_or_else(|| panic!("Menü Baruch fehlt: {head}"));
    let end = start + head[start..].find("</details>").expect("Menü ohne Ende");
    &head[start..end]
}

/// Identifiers of the tabs in the order of the page.
fn tabs(head: &str) -> Vec<&str> {
    head.split("data-band-tab=\"").skip(1).filter_map(|rest| rest.split('"').next()).collect()
}

struct Yard {
    server: common::Server,
    work: i64,
    /// Keys of the sections: Kiel legen, Spanten, Stapellauf.
    keys: Vec<i64>,
}

async fn yard() -> Yard {
    let server = common::open().await;
    let work = server.import(WERFT, "bootswerft.typ").await.work;
    let keys = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Yard { server, work, keys }
}

async fn get_with_cookie(
    server: &common::Server,
    uri: &str,
    cookie: &str,
) -> (StatusCode, axum::http::HeaderMap, String) {
    server.send(Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap()).await
}

async fn mcp_call(server: &common::Server, tool: &str, arguments: Value) {
    let message = json!({ "jsonrpc": "2.0", "id": 33, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40033))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "tools/call {tool}: {body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
}

// ---- Structure ------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_workspace_shows_the_band_with_menu_work_choice_title_five_tabs_and_five_panels() {
    let y = yard().await;
    let (status, html) = y.server.get(&format!("/?work={}", y.work)).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite lädt: {html}");
    assert_eq!(html.matches("<header class=\"band\"").count(), 1, "genau ein Menüband: {html}");
    assert!(
        !html.contains("class=\"topbar\""),
        "die alte Kopfzeile (Klasse topbar) ist ersetzt, nicht ergänzt: {html}"
    );
    assert!(!html.contains("data-band-fill"), "die Marke an <html> hat die Schicht ersetzt (kein Rest im HTML)");
    assert!(
        html.contains(" data-laptop=\"") && html.contains(" data-band=\"open\">"),
        "<html> trägt Laptop-Zustand und data-band (einzige Quelle für CSS und Widgets): {}",
        &html[..html.find("<head>").unwrap_or(200)]
    );
    let head = band(&html);
    assert!(
        head.starts_with(&format!(
            "<header class=\"band\" data-widget=\"band\" data-tab=\"start\" data-work=\"{}\">",
            y.work
        )),
        "Kopf als Widget band; aktiver Reiter ist ohne ?tab= die Vorgabe der Arbeitsseite (Start); das Werk für die Abzeichen: {head}"
    );
    assert_eq!(
        head.matches("<details class=\"menu\"").count(),
        1,
        "genau ein Menü in der Menüleiste (Werk ▾/Ansicht ▾/Werkzeuge ▾ sind in die Reiter gewandert): {head}"
    );
    assert!(
        app_menu(head).contains(">Baruch ▾</summary>")
            && app_menu(head).contains("<div class=\"menu-list\" role=\"menu\">"),
        "das Menü heißt „Baruch ▾“ und ist ein <details> mit role=menu (klappt ohne JavaScript auf): {head}"
    );
    assert!(
        head.contains(
            "<form class=\"work-select\" action=\"/\" method=\"get\" role=\"search\" aria-label=\"Werk wählen\">"
        ) && head.contains("<select name=\"work\" aria-label=\"Werk\" data-widget=\"work-select\">")
            && head.contains("<noscript><button class=\"btn\" type=\"submit\">Öffnen</button></noscript>"),
        "Werkwahl unverändert (GET-Formular, ohne JavaScript mit „Öffnen“): {head}"
    );
    assert!(
        head.contains("<span class=\"work-title\">Die Bootswerft am Schilfsee</span>"),
        "Titel des Werks im Band: {head}"
    );
    assert!(head.contains("· Henrike Planke</span>"), "der Autor steht daneben: {head}");
    assert_eq!(
        tabs(head),
        ["start", "einfuegen", "pruefen", "buch", "ansicht"],
        "Arbeitsseite: alle fünf Reiter in fester Reihenfolge"
    );
    assert!(head.contains("<nav class=\"band-tabs\" role=\"tablist\""), "Reiterzeile als tablist: {head}");
    for (id, label) in baruch_server::band::TABS {
        assert!(
            head.contains(&format!("<a class=\"band-tab\" role=\"tab\" id=\"band-tab-{id}\" aria-controls=\"band-panel-{id}\" data-band-tab=\"{id}\" href=\"?work={}&amp;tab={id}", y.work)),
            "Reiter {id} ist ein Link mit role=tab (ohne JavaScript ein Seitenaufruf mit ?tab=), der das Werk behält: {head}"
        );
        assert!(head.contains(&format!(">{label}")), "Beschriftung „{label}“: {head}");
        assert_eq!(
            head.matches(&format!("<div class=\"band-panel\" role=\"tabpanel\" id=\"band-panel-{id}\" aria-labelledby=\"band-tab-{id}\" data-band-panel=\"{id}\"")).count(),
            1,
            "Feld {id} genau einmal, mit seinem Reiter verknüpft: {head}"
        );
    }
    assert_eq!(head.matches("aria-selected=\"true\"").count(), 1, "genau ein Reiter gewählt: {head}");
    assert!(head.contains("data-band-panel=\"start\">"), "das Feld des aktiven Reiters ist nicht verborgen: {head}");
    for id in ["einfuegen", "pruefen", "buch", "ansicht"] {
        assert!(
            head.contains(&format!("data-band-panel=\"{id}\" hidden>")),
            "Feld {id} ist verborgen, solange sein Reiter nicht gewählt ist"
        );
    }
    assert!(
        head.contains("<div class=\"band-dock\" data-band-dock></div>"),
        "leerer Andockplatz für die Knöpfe des Editors (füllt band.ts): {head}"
    );
    let after = &html[html.find("</header>").unwrap() + "</header>".len()..];
    assert!(
        after.trim_start().starts_with("<main") || after.trim_start().starts_with("<div class=\"consent"),
        "direkt unter dem Kopf folgt der Platz der Zustimmung bzw. der Inhalt (die Marke ist ersetzt): {}",
        &after[..after.len().min(120)]
    );
}

#[tokio::test]
async fn without_a_work_there_are_only_start_and_ansicht() {
    // Fresh server without a work and without a cookie: /werke knows no work.
    let server = common::open().await;
    let (status, html) = server.get("/werke").await;
    assert_eq!(status, StatusCode::OK, "Werkliste lädt: {html}");
    let head = band(&html);
    assert_eq!(
        tabs(head),
        ["start", "ansicht"],
        "ohne Werk gibt es Einfügen, Prüfen und Buch nicht (sie brauchen ein Werk): {head}"
    );
    assert!(!head.contains("data-work="), "ohne Werk trägt der Kopf keines: {head}");
    assert!(
        panel(head, "start")
            .contains("<a class=\"btn\" href=\"/werke\" title=\"Erst ein Werk wählen\">Werk wählen</a>"),
        "wie bisher führt der Weg ohne Werk auf /werke („Erst ein Werk wählen“): {head}"
    );
    assert!(
        panel(head, "start").contains("href=\"/literatur\" data-literature-open")
            && panel(head, "start").contains("href=\"/abgleich\""),
        "Literatur und Abgleichen brauchen kein Werk und bleiben darum unter Start erreichbar: {head}"
    );
    assert!(
        panel(head, "start").contains("href=\"/einfaelle\" data-idea-open"),
        "Einfall festhalten ohne Werk wie bisher: {head}"
    );
    assert!(!head.contains("data-band-dock"), "kein Andockplatz außerhalb der Arbeitsseite: {head}");
    assert!(!head.contains("data-rail-toggle"), "Seitenleisten gibt es nur auf der Arbeitsseite: {head}");
}

#[tokio::test]
async fn other_pages_of_a_work_have_four_tabs_and_lead_back_to_the_text() {
    let y = yard().await;
    let (status, html) = y.server.get(&format!("/gliederung?work={}", y.work)).await;
    assert_eq!(status, StatusCode::OK, "Gliederung lädt");
    let head = band(&html);
    assert_eq!(
        tabs(head),
        ["start", "pruefen", "buch", "ansicht"],
        "Seite mit Werk: ohne „Einfügen“ (kein Editor): {head}"
    );
    assert!(
        panel(head, "start").contains(&format!("<a class=\"btn\" href=\"/?work={}\" title=\"Zur Arbeitsseite mit Gliederung, Editor und Assistenten\">Zum Text</a>", y.work)),
        "Start führt zurück zur Arbeitsseite (früher Ansicht ▾ → Editor und der Schriftzug Baruch): {head}"
    );
    assert!(
        !head.contains("data-work-import-open"),
        "außerhalb der Arbeitsseite ist „Neues Werk“ ein schlichter Link (kein Dialog auf der Seite)"
    );
    assert!(
        app_menu(head).contains("href=\"/werke/neu\">Neues Werk / Importieren…</a>"),
        "…auf die Seite /werke/neu: {head}"
    );
}

// ---- State from address and cookie ----------------------------------------------------------------------------------

#[tokio::test]
async fn tab_from_the_address_selects_the_tab_and_keeps_the_rest_of_the_query() {
    let y = yard().await;
    let key = y.keys[1];
    let (_, html) = y.server.get(&format!("/?work={}&key={key}&tab=buch", y.work)).await;
    let head = band(&html);
    assert!(head.contains("data-tab=\"buch\""), "?tab=buch wählt den Reiter Buch: {head}");
    assert!(
        head.contains(&format!("data-band-tab=\"buch\" href=\"?work={}&amp;key={key}&amp;tab=buch&amp;band=closed\" aria-selected=\"true\"", y.work)),
        "Buch ist gewählt; sein Link klappt das Band zu und behält Werk und Abschnitt: {head}"
    );
    assert!(
        head.contains("data-band-panel=\"buch\">") && head.contains("data-band-panel=\"start\" hidden>"),
        "sichtbar ist das Feld Buch, Start ist verborgen: {head}"
    );
    assert!(
        head.contains(&format!(
            "data-band-tab=\"start\" href=\"?work={}&amp;key={key}&amp;tab=start\" aria-selected=\"false\"",
            y.work
        )),
        "die anderen Reiter führen auf denselben Abschnitt: {head}"
    );
    // Invalid or not present on this page: default of the page.
    let (_, html) = y.server.get(&format!("/?work={}&tab=unsinn", y.work)).await;
    assert!(band(&html).contains("data-tab=\"start\""), "ein unbekannter Reiter wird übergangen");
    let (_, html) = y.server.get(&format!("/gliederung?work={}&tab=einfuegen", y.work)).await;
    assert!(
        !band(&html).contains("data-tab=\"einfuegen\"") && band(&html).contains("aria-selected=\"true\""),
        "„Einfügen“ gibt es auf der Gliederungsseite nicht: Es gilt deren Vorgabe, ein Reiter ist immer gewählt"
    );
}

#[tokio::test]
async fn band_closed_is_remembered_as_a_cookie_next_to_the_work_cookie() {
    let y = yard().await;
    let (status, headers, html) =
        y.server.send(Request::get(format!("/?work={}&band=closed", y.work)).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let cookies: Vec<&str> = headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap()).collect();
    assert!(
        cookies.iter().any(|c| c.starts_with(&format!("baruch_work={}", y.work))),
        "die Arbeitsseite merkt weiter das Werk (das Cookie des Bands wird angehängt, nicht ersetzt): {cookies:?}"
    );
    assert!(
        cookies.contains(&"baruch_band=closed; Path=/; Max-Age=31536000; SameSite=Lax"),
        "?band=closed wird ein Jahr gemerkt: {cookies:?}"
    );
    assert!(
        html.contains(" data-band=\"closed\">"),
        "die Antwort selbst ist schon zugeklappt (Adresse gilt vor dem Cookie)"
    );
    assert!(
        band(&html).contains(&format!(
            "data-band-tab=\"start\" href=\"?work={}&amp;tab=start&amp;band=open\" aria-selected=\"true\"",
            y.work
        )),
        "zugeklappt öffnet ein Reiter das Band wieder (ohne JavaScript über ?band=open): {}",
        band(&html)
    );
    // On the next call the cookie applies; ?band=open takes precedence and deletes it.
    let (_, _, html) = get_with_cookie(&y.server, &format!("/?work={}", y.work), "baruch_band=closed").await;
    assert!(html.contains(" data-band=\"closed\">"), "Cookie baruch_band=closed hält das Band zu");
    let (_, headers, html) =
        get_with_cookie(&y.server, &format!("/?work={}&band=open", y.work), "baruch_band=closed").await;
    assert!(html.contains(" data-band=\"open\">"), "?band=open geht vor dem Cookie");
    assert!(
        headers.get_all(header::SET_COOKIE).iter().any(|v| v == "baruch_band=; Path=/; Max-Age=0; SameSite=Lax"),
        "und löscht es (offen ist die Vorgabe)"
    );
    // Without a key in the address nothing is set.
    let (_, headers, _) =
        y.server.send(Request::get(format!("/gliederung?work={}", y.work)).body(Body::empty()).unwrap()).await;
    assert!(
        !headers.get_all(header::SET_COOKIE).iter().any(|v| v.to_str().unwrap().starts_with("baruch_band")),
        "ohne ?band= kein Cookie des Bands"
    );
}

#[tokio::test]
async fn laptop_mode_on_turns_the_groups_into_details_popups() {
    let y = yard().await;
    let uri = format!("/?work={}", y.work);
    let (_, html) = y.server.get(&uri).await;
    let head = band(&html);
    let groups = head.matches("<details class=\"band-group\"").count();
    assert!(groups >= 10, "die Arbeitsseite hat Gruppen in allen Feldern: {groups}");
    assert_eq!(
        head.matches(" data-band-group open>").count(),
        groups,
        "volles Band: jede Gruppe ist offen (Inhalt sichtbar, Beschriftung darunter): {head}"
    );
    assert!(
        head.contains("<details class=\"band-group\" data-band-group open><summary class=\"band-group-label\">Ansicht des Textes</summary><div class=\"band-items\" role=\"group\" aria-label=\"Ansicht des Textes\">"),
        "Gruppe in der wörtlichen Form des Vertrags (details/summary, role=group mit Namen): {head}"
    );
    // Whether the cookie is read already is decided by src/laptop.rs (task „laptop“; in the state S1 the default always
    // applies).
    let read = baruch_server::laptop::LaptopState::from_request(Some("baruch_laptop=on"), None).compact();
    if !read {
        eprintln!(
            "ÜBERSPRUNGEN: laptop.rs steht noch im Stand S1 (Cookie baruch_laptop wird nicht gelesen) – die Probe mit Cookie läuft erst nach dem Zusammenführen; band::apply mit kompaktem Zustand prüft der Unit-Test compact_turns_every_group_into_a_popup."
        );
        return;
    }
    let (_, _, html) = get_with_cookie(&y.server, &uri, "baruch_laptop=on").await;
    let head = band(&html);
    assert_eq!(
        head.matches(" data-band-group open>").count(),
        0,
        "Laptop-Modus an: keine Gruppe ist aufgeklappt: {head}"
    );
    assert_eq!(
        head.matches(" data-band-group name=\"band-group\"><summary class=\"band-group-label\">").count(),
        groups,
        "jede Gruppe ist ein Popup aus details/summary (ohne JavaScript bedienbar, gemeinsamer Name = nur eines offen): {head}"
    );
    assert!(html.contains("data-laptop=\"on\""), "<html> nennt den Modus");
}

#[tokio::test]
async fn a_quote_in_the_query_cannot_break_out_of_the_tab_addresses() {
    let y = yard().await;
    // Browsers send ' in the query raw; < > and " arrive encoded (%3C …) and are harmless then.
    let uri = format!("/?work={}&x='onmouseover='alert(1)&y=%22%3E%3Cscript%3E", y.work);
    let (status, html) = y.server.get(&uri).await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt trotz der fremden Schlüssel");
    let head = band(&html);
    assert!(!head.contains("'onmouseover="), "das Hochkomma aus der Adresse steht nirgends roh im Band: {head}");
    assert!(
        head.contains(&format!(
            "href=\"?work={}&amp;x=&#39;onmouseover=&#39;alert(1)&amp;y=%22%3E%3Cscript%3E&amp;tab=buch\"",
            y.work
        )),
        "der Rest der Abfrage steht maskiert im href (& und ' maskiert, Kodiertes unverändert): {head}"
    );
    assert!(!html.contains("<script>alert") && !head.contains("\"><script"), "kein Skript aus der Adresse");
}

// ---- Mapping of the old menus (contract §4) -------------------------------------------------------------------------

#[tokio::test]
async fn every_entry_of_the_former_menus_is_a_link_on_the_workspace() {
    let y = yard().await;
    let (w, key) = (y.work, y.keys[1]);
    let (_, html) = y.server.get(&format!("/?work={w}&key={key}&lq=Eiche")).await;
    let head = band(&html);
    let rev = {
        let tail = &head[head.find("&rev=").expect("rev im Band") + 5..];
        tail.chars().take_while(char::is_ascii_digit).collect::<String>()
    };
    let menu = app_menu(head);
    // (panel or menu, expected piece, reason: where the entry comes from and why it is there)
    let entries: Vec<(&str, String, &str)> = vec![
        ("baruch", "href=\"/werke\">Alle Werke</a>".into(), "Werk ▾ → Alle Werke: gilt für das Programm, nicht für ein Werk"),
        ("baruch", "href=\"/werke/neu\" data-work-import-open aria-haspopup=\"dialog\" aria-controls=\"work-import\">Neues Werk / Importieren…</a>".into(), "Werk ▾ → Neues Werk / Importieren…: mit JavaScript der Dialog, sonst die Seite"),
        ("baruch", "href=\"/werke/import/quelle\"".into(), "Werk ▾ → PDF oder EPUB importieren…"),
        ("baruch", format!("href=\"/austausch/einlesen?work={w}\""), "Werk ▾ → Austausch: Einlesen… (legt ein Werk an bzw. führt zusammen: Programm)"),
        ("baruch", format!("href=\"/einstellungen/darstellung?work={w}\" data-settings-open aria-haspopup=\"dialog\" aria-controls=\"settings\" aria-keyshortcuts=\"Alt+Comma\" title=\"Einstellungen (Alt+,)\""), "Knopf ⚙ Einstellungen der Kopfzeile (Alt+,)"),
        ("baruch", "href=\"/ueber\"".into(), "Werkzeuge ▾ → Über Baruch"),
        ("start", format!("data-nav data-view-toggle href=\"/?work={w}&key={key}&rev={rev}&view=preview&amp;lq=Eiche\""), "Ansicht ▾ → Vorschau/Editor (Umschalter; behält Abschnitt, Fassung und Suche)"),
        ("start", format!("href=\"/einfaelle?work={w}\" data-idea-open aria-haspopup=\"dialog\" aria-controls=\"idea-capture\" title=\"Einfall festhalten (Alt+E)\""), "Werkzeuge ▾ → Einfall festhalten (Alt+E)"),
        ("start", format!("href=\"/einfaelle?work={w}\" title=\"Liste der Einfälle dieses Werks\">Einfälle</a>"), "Werkzeuge ▾ → Einfälle"),
        ("start", format!("href=\"/werke/{w}/karte\""), "Werkzeuge ▾ → Gedankenkarte"),
        ("pruefen", format!("href=\"/werke/{w}/befunde\""), "Werk ▾ → Befunde"),
        ("pruefen", format!("href=\"/werke/{w}/pruefung?key={key}&amp;rev={rev}\""), "Zitate prüfen (bisher nur in der Statuszeile des Editors; hier für die gezeigte Fassung)"),
        ("pruefen", format!("href=\"/suche?work={w}\""), "Werkzeuge ▾ → Im Werk suchen und ersetzen"),
        ("pruefen", format!("href=\"/verweise?work={w}\""), "Werkzeuge ▾ → Querverweise prüfen"),
        ("pruefen", format!("href=\"/literatur?work={w}&key={key}&rev={rev}&amp;lq=Eiche\" data-literature-open aria-haspopup=\"dialog\" aria-controls=\"literature\""), "Werkzeuge ▾ → Literatur (Alt+L; behält Abschnitt und Suche)"),
        ("pruefen", format!("href=\"/abgleich?work={w}\""), "Werkzeuge ▾ → Abgleichen"),
        ("buch", format!("href=\"/werke/{w}/bearbeiten\">Werk bearbeiten</a>"), "Werk ▾ → Werk bearbeiten"),
        ("buch", format!("href=\"/gliederung?work={w}\">Gliederung</a>"), "Werk ▾ → Gliederung"),
        ("buch", format!("href=\"/gliederung?work={w}#antraege\" data-badge-item=\"outline_requests\""), "Werk ▾ → Anträge zur Gliederung (sichtbar, sobald einer offen ist)"),
        ("buch", format!("href=\"/werke/{w}/nummerierung\">Gliederungsschema</a>"), "Werk ▾ → Gliederungsschema"),
        ("buch", format!("href=\"/werke/{w}/bilder\">Bilder</a>"), "Werk ▾ → Bilder"),
        ("buch", format!("href=\"/werke/{w}/satz?key={key}\""), "Ansicht ▾ → Satz (jetzt „Satz-Vorschau“; beginnt beim gezeigten Abschnitt)"),
        ("buch", format!("href=\"/werke/{w}/epub/vorschau\""), "Ansicht ▾ → EPUB-Vorschau"),
        ("buch", format!("href=\"/werke/{w}/pdf\" download"), "Werk ▾ → Als PDF herunterladen (jetzt „PDF“ in der Gruppe Ausgabe)"),
        ("buch", format!("href=\"/werke/{w}/pdf?entwurf=1\" download"), "Werk ▾ → Als Entwurf herunterladen"),
        ("buch", format!("href=\"/werke/{w}/epub\" download"), "Werk ▾ → Als EPUB herunterladen"),
        ("buch", format!("href=\"/werke/{w}/stichwortzettel\""), "Werk ▾ → Stichwortzettel …"),
        ("buch", format!("href=\"/werke/{w}/export\""), "Werk ▾ → Exportieren (Typst)"),
        ("buch", format!("href=\"/werke/{w}/austausch\""), "Werk ▾ → Austausch: Ausgeben"),
        ("buch", format!("href=\"/werke/{w}/umschlag\" title"), "Werk ▾ → Umschlag"),
        ("buch", format!("href=\"/werke/{w}/umschlag.pdf\" download"), "Werk ▾ → Umschlag-PDF"),
        ("buch", format!("href=\"/werke/{w}/glossar\""), "Werk ▾ → Glossar"),
        ("buch", format!("href=\"/werke/{w}/register\""), "Werk ▾ → Register"),
        ("ansicht", "data-widget=\"theme\" data-auto=\"◐\" data-light=\"☀\" data-dark=\"☾\" aria-label=\"Thema wechseln\"".into(), "Thema-Umschalter der Kopfzeile (Widget theme unverändert)"),
    ];
    for (place, piece, why) in &entries {
        let area = if *place == "baruch" { menu } else { panel(head, place) };
        assert!(
            area.contains(piece.as_str()),
            "{why} – erwartet in „{place}“ als Link ohne JavaScript: {piece}\n{area}"
        );
        assert_eq!(
            head.matches(piece.as_str()).count(),
            1,
            "{why} – genau einmal im Band (kein Eintrag doppelt): {piece}"
        );
    }
    assert_eq!(
        entries.len(),
        34,
        "alle Einträge der drei alten Menüs samt Thema und Einstellungen sind einzeln belegt"
    );
    assert!(panel(head, "start").contains(">Vorschau</a>"), "im Editor heißt der Umschalter „Vorschau“: {head}");
    assert!(
        !menu.contains("<button"),
        "das Menü Baruch enthält nur Links (nichts, was nur mit JavaScript etwas täte): {menu}"
    );
    // New switches of the tab Ansicht (contract §4, verbatim forms; the state is set by the layer).
    let view = panel(head, "ansicht");
    for piece in [
        "data-rail-toggle=\"left\"",
        "data-rail-toggle=\"right\"",
        "data-laptop-set=\"auto\"",
        "data-laptop-set=\"on\"",
        "data-laptop-set=\"off\"",
        "<button class=\"btn js-only\" type=\"button\" data-zoom-reset>Zoom zurücksetzen</button>",
    ] {
        assert!(view.contains(piece), "Reiter Ansicht: {piece} fehlt: {view}");
    }
    assert!(
        view.contains(&format!("data-laptop-set=\"on\" aria-pressed=\"false\" href=\"?work={w}&amp;key={key}&amp;lq=Eiche&amp;tab=start&amp;laptop=on\">An</a>")),
        "die Laptop-Schalter sind Links, die Werk, Abschnitt und Reiter behalten (ohne JavaScript bedienbar): {view}"
    );
    assert!(
        view.matches("aria-pressed=\"true\"").count() >= 1,
        "der geltende Laptop-Modus ist als gedrückt gezeichnet: {view}"
    );
    // Exactly one element in the band per hook (the widgets take querySelector or, for Alt+, the first in the
    // document).
    for hook in
        ["data-work-import-open", "data-literature-open", "data-idea-open", "data-view-toggle", "data-settings-open"]
    {
        assert_eq!(head.matches(hook).count(), 1, "{hook} kommt im Band genau einmal vor: {head}");
    }
    for hook in ["data-work-import-open", "data-literature-open", "data-idea-open", "data-view-toggle"] {
        assert_eq!(html.matches(&format!(" {hook}")).count(), 1, "{hook} gibt es auf der ganzen Seite nur einmal");
    }
    assert!(
        html.find("data-settings-open").unwrap() < html.find("<main").unwrap(),
        "der Eintrag „Einstellungen“ des Bands ist der erste data-settings-open der Seite (Alt+, nimmt den ersten)"
    );
    let bad: Vec<&str> =
        head.split("href=\"").skip(1).filter_map(|r| r.split('"').next()).filter(|h| h.contains("node=")).collect();
    assert!(bad.is_empty(), "kein Link des Bands mit node= (Vertrag Runde 8 §1): {bad:?}");
}

#[test]
fn the_template_keeps_the_three_places_for_round_32() {
    let base = include_str!("../templates/base.html");
    let at = |mark: &str| base.find(mark).unwrap_or_else(|| panic!("Marke fehlt in base.html: {mark}"));
    let menu_start = at("<details class=\"menu\" data-menu=\"baruch\">");
    let menu_end = menu_start + base[menu_start..].find("</details>").unwrap();
    let import = at("{#- R32-PLATZ import-word -#}");
    let quit = at("{#- R32-PLATZ beenden -#}");
    let export = at("{#- R32-PLATZ export-word -#}");
    assert!(menu_start < import && import < menu_end, "Platz „Word importieren“ im Menü Baruch ▾ (bei den Importen)");
    assert!(
        at("PDF oder EPUB importieren…") < import && import < at("Austausch: Einlesen…"),
        "…zwischen „PDF oder EPUB importieren…“ und „Austausch: Einlesen…“"
    );
    assert!(at("Über Baruch</a>") < quit && quit < menu_end, "Platz „Baruch beenden“ ist der letzte Eintrag des Menüs");
    assert!(base[quit..menu_end].matches("<a ").count() == 0, "hinter dem Platz „beenden“ folgt kein weiterer Eintrag");
    assert!(
        at(">EPUB</a>") < export && export < at("Stichwortzettel …</a>"),
        "Platz „Word exportieren“ in der Gruppe Ausgabe, hinter EPUB"
    );
    assert!(!base.contains("class=\"topbar\""), "die alte Kopfzeile steht nicht mehr in der Vorlage");
}

// ---- Badges ---------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn badges_stand_at_the_tabs_on_the_workspace_and_are_filled_on_other_pages_without_javascript() {
    let y = yard().await;
    let w = y.work;
    // One request on the outline (renaming waits for the human), two proposals, three open findings.
    mcp_call(&y.server, "outline_rename", json!({ "work": w, "key": y.keys[2], "title": "Stapellauf am Sonntag" }))
        .await;
    mcp_call(
        &y.server,
        "propose_revision",
        json!({ "work": w, "key": y.keys[0], "body": "= Kiel legen\n\nDer Kiel liegt.\n" }),
    )
    .await;
    mcp_call(
        &y.server,
        "propose_revision",
        json!({ "work": w, "key": y.keys[2], "body": "= Stapellauf\n\nEs regnet.\n" }),
    )
    .await;
    for body in ["Eiche oder Lärche?", "„seit dem Morgen“ doppelt.", "Wie viele Spanten?"] {
        let new = NewFinding {
            work: w,
            key: Some(y.keys[0]),
            post: None,
            thread_post: None,
            position: 0,
            persona: "@lektor",
            kind: FindingKind::Hint,
            body,
            quote: "",
            quote_check: "",
            similar_to: None,
            summary_of: &[],
        };
        y.server.state.store.finding_insert(&new).await.expect("Befund lässt sich anlegen");
    }
    let counted = y.server.rpc("menu.badges", json!({ "work": w })).await;
    assert_eq!(
        ok(&counted),
        &json!({ "work": w, "outline_requests": 1, "open_proposals": 2, "open_findings": 3 }),
        "Ausgangslage: menu.badges zählt Antrag, Vorschläge und Befunde (Name und Ergebnis der Methode unverändert)"
    );

    // Work page: drawn from the template.
    let (_, html) = y.server.get(&format!("/?work={w}")).await;
    let head = band(&html);
    assert!(
        !head.contains("data-badge-fill"),
        "auf der Arbeitsseite zeichnet die Vorlage selbst (kein Kennzeichen zum Füllen): {head}"
    );
    assert!(
        head.contains(">Start <span class=\"badge\" data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\">2</span></a>"),
        "offene Vorschläge am Reiter Start (dort stehen Text und Fassungen): {head}"
    );
    assert!(
        head.contains(">Prüfen <span class=\"badge warning\" data-badge=\"open_findings\" title=\"offene Befunde im Werk\">3 offen</span></a>"),
        "„3 offen“ am Reiter Prüfen, sichtbar ohne das Feld zu öffnen: {head}"
    );
    assert!(
        head.contains(">Buch <span class=\"badge\" data-badge=\"outline_requests\" title=\"offene Anträge zur Gliederung\">1</span></a>"),
        "offene Anträge am Reiter Buch (dort steht die Gliederung): {head}"
    );
    assert!(
        panel(head, "buch").contains(&format!("href=\"/gliederung?work={w}#antraege\" data-badge-item=\"outline_requests\">Anträge zur Gliederung <span class=\"badge\" data-badge=\"outline_requests\">1</span></a>")),
        "der Eintrag „Anträge zur Gliederung“ ist sichtbar und trägt die Zahl: {head}"
    );
    assert!(
        panel(head, "pruefen")
            .contains("Befunde <span class=\"badge warning\" data-badge=\"open_findings\">3</span></a>"),
        "am Eintrag „Befunde“ steht nur die Zahl: {head}"
    );

    // Another page: the layer (menu::fill_badges) puts the same numbers into the finished HTML – without JavaScript.
    let (_, html) = y.server.get(&format!("/werke/{w}/bilder")).await;
    let head = band(&html);
    assert!(!head.contains("data-badge-fill"), "das Kennzeichen ist nach dem Füllen entfernt: {head}");
    assert!(head.contains(&format!("data-badges-work=\"{w}\"")), "die Menüleiste nennt das Werk der Abzeichen: {head}");
    for piece in [
        "data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\">2</span>",
        "data-badge=\"open_findings\" title=\"offene Befunde im Werk\">3 offen</span>",
        "data-badge=\"outline_requests\" title=\"offene Anträge zur Gliederung\">1</span>",
        "data-badge-item=\"outline_requests\">Anträge zur Gliederung <span class=\"badge\" data-badge=\"outline_requests\">1</span>",
        "data-badge=\"open_findings\">3</span>",
    ] {
        assert!(
            head.contains(piece),
            "außerhalb der Arbeitsseite steht dieselbe Zahl im ausgelieferten HTML: {piece}\n{head}"
        );
    }
    // Without open items the badges stay hidden (another, empty work).
    let other = y.server.import("= Leer\n\nNichts offen.\n", "leer.typ").await.work;
    let (_, html) = y.server.get(&format!("/gliederung?work={other}")).await;
    let head = band(&html);
    assert!(
        head.contains("data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\" hidden>0</span>")
            && head.contains("data-badge=\"open_findings\" title=\"offene Befunde im Werk\" hidden>0 offen</span>")
            && head.contains("data-badge-item=\"outline_requests\" hidden>"),
        "null bleibt verborgen (kein leeres Abzeichen am Reiter): {head}"
    );
}

// ---- Editor and docking ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn the_editor_fragment_keeps_the_toolbar_in_two_parts_with_every_hook_and_ends_with_the_dock_mark() {
    let y = yard().await;
    // Second revision so that ‹ › are links.
    let saved = y.server.rpc("outline.save", json!({ "work": y.work, "key": y.keys[0], "body": "= Kiel legen\n\nDer Kiel aus Eiche liegt auf den Böcken, frisch geölt.\n", "mode": "new" })).await;
    ok(&saved);
    let rendered = y.server.rpc("ui.render", json!({ "work": y.work, "key": y.keys[0], "rev": 2 })).await;
    let result = ok(&rendered);
    let mut names: Vec<&str> = result.as_object().unwrap().keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["assistants", "editor", "title", "tree", "url"],
        "ui.render liefert unverändert genau diese fünf Felder"
    );
    let editor = result["editor"].as_str().unwrap();
    assert!(
        editor.trim_end().ends_with("<span data-widget=\"band-dock\" hidden></span>"),
        "das Fragment endet mit der Marke, an der band.ts nach jedem Neuzeichnen andockt: …{}",
        &editor[editor.len().saturating_sub(200)..]
    );
    let start = editor.find("<div class=\"ws-bar edit-toolbar ribbon\" role=\"toolbar\"").expect("Leiste im Fragment");
    let wysiwyg = editor.find("<div class=\"wysiwyg\" data-widget=\"wysiwyg\"").expect("Editor-Bereich");
    let body = editor.find("<div class=\"editor-body\"").expect("Blatt");
    assert!(
        wysiwyg < start && start < body,
        "die Leiste steht weiter in div.wysiwyg vor dem Blatt (ui.render zeichnet sie mit)"
    );
    let bar = &editor[start..editor.find("<div class=\"search-bar\"").expect("Suchleiste hinter der Leiste")];
    assert!(
        bar.starts_with("<div class=\"ws-bar edit-toolbar ribbon\" role=\"toolbar\" aria-label=\"Menüband für das Bearbeiten\" data-zoom-zone=\"toolbar\" data-edit-toolbar hidden>"),
        "ohne JavaScript verborgen ([data-edit-toolbar][hidden]) – dann gibt es wie bisher nur Textfeld und Statuszeile: {bar}"
    );
    let einfuegen = bar.find("data-band-part=\"einfuegen\"").expect("Teil einfuegen");
    let (first, second) = bar.split_at(einfuegen);
    assert!(first.contains("data-band-part=\"start\""), "Teil für den Reiter Start: {bar}");
    for (hook, part, why) in [
        ("data-ws-mode=\"satz\"", first, "Umschalter Satzbild (Gruppe Text)"),
        ("data-ws-mode=\"quelltext\"", first, "Umschalter Quelltext"),
        ("class=\"ws-tools\" data-ws-tools", first, "Werkzeuge des Satzbilds"),
        ("data-ws-source-note", first, "Hinweis „nur im Satzbild“ für den Quelltext"),
        ("data-ws-format", first, "Absatzformat"),
        ("data-ws-cmd=\"italic\"", first, "K"),
        ("data-ws-cmd=\"bold\"", first, "F"),
        ("form=\"editor-form\" data-band-save", first, "Speichern gehört zum Formular im Blatt"),
        ("form=\"editor-form\" name=\"as_new\" value=\"1\"", first, "Als neue Fassung"),
        ("data-band-assist=\"persona.review\"", first, "Prüfen (löst das Formular rechts aus)"),
        ("data-band-assist=\"persona.propose\"", first, "Vorschlag"),
        ("data-widget=\"ribbon\"", second, "Widget ribbon um die Formate"),
        (
            "data-ribbon-cmd=\"footnote\" data-ribbon-snippet=\"#footnote[]\"",
            second,
            "Fußnote mit Typst-Text für den Quelltext",
        ),
        ("data-ribbon-cmd=\"quote\"", second, "Zitat"),
        ("data-ribbon-cmd=\"list\"", second, "Liste"),
        ("data-ribbon-cmd=\"enum\"", second, "Aufzählung"),
        ("data-ribbon-cmd=\"terms\"", second, "Begriff"),
        ("data-ribbon-cmd=\"break\"", second, "Umbruch"),
        ("data-ribbon-cmd=\"link\"", second, "Link"),
        ("data-ribbon-cmd=\"xref\"", second, "Verweis"),
        ("data-ribbon-image", second, "Bild"),
        ("data-widget=\"context-menu\"", second, "Widget context-menu um Register und Glossar"),
        ("data-ctx-action=\"glossary_add\"", second, "In Glossar"),
        ("data-ctx-action=\"index_add\"", second, "In Register"),
        ("data-ctx-action=\"glossary_lookup\"", second, "Nachschlagen"),
        ("data-index-list", second, "Liste der Registereinträge"),
        ("data-index-toggle", second, "Knopf Register"),
        ("data-index-entries", second, "Einträge"),
    ] {
        assert_eq!(
            part.matches(hook).count(),
            1,
            "{why}: Haken {hook} genau einmal im richtigen Teil der Leiste: {bar}"
        );
    }
    assert_eq!(
        bar.matches("<details class=\"band-group\" data-band-group open><summary class=\"band-group-label\">").count(),
        7,
        "sieben Gruppen in der Form des Bands (Text, Absatz, Fassung, Assistenten; Einfügen, Bild, Register und Glossar): {bar}"
    );
    // Revision in the band: ‹ › without data-key – Alt+←/→ still takes the links in the header of the editor.
    assert!(
        first.contains(&format!(
            "<a class=\"btn\" data-nav href=\"/?work={}&key={}&rev=1\" aria-label=\"Vorige Fassung\"",
            y.work, y.keys[0]
        )),
        "‹ im Band führt zur vorigen Fassung, als data-nav ohne Neuladen: {first}"
    );
    assert!(
        !bar.contains("data-key="),
        "die Links im Band tragen kein data-key (sonst träfe a[data-key=prev] zwei Elemente): {bar}"
    );
    assert_eq!(
        editor.matches("data-key=\"prev\"").count(),
        1,
        "data-key=prev bleibt genau einmal im Kopf des Editors: {editor}"
    );
    assert!(first.contains(">2/2</span>"), "Zähler der Fassungen wie im Kopf: {first}");
    // The authoritative button stays in the status line.
    assert!(
        editor.contains("<button class=\"btn primary\" type=\"submit\" form=\"editor-form\" data-saved=\"Gespeichert\" data-dirty=\"Speichern\""),
        "„Speichern“ der Statuszeile unverändert (geht ohne JavaScript; trägt die Beschriftung Gespeichert/Speichern): {editor}"
    );
    assert!(
        editor.contains("id=\"bild-einfuegen\" data-image-insert"),
        "Anker für den Hinweis ohne JavaScript im Reiter Einfügen"
    );

    // Preview: no bar, but the marker (band.ts then empties the dock).
    let preview = y.server.rpc("ui.render", json!({ "work": y.work, "key": y.keys[0], "view": "preview" })).await;
    let editor = ok(&preview)["editor"].as_str().unwrap();
    assert!(!editor.contains("data-edit-toolbar"), "in der Vorschau gibt es keine Leiste");
    assert!(
        editor.trim_end().ends_with("<span data-widget=\"band-dock\" hidden></span>"),
        "die Marke steht auch in der Vorschau am Ende"
    );
}

#[tokio::test]
async fn without_javascript_the_insert_tab_points_to_the_sheet() {
    let y = yard().await;
    let (_, html) = y.server.get(&format!("/?work={}&key={}&tab=einfuegen", y.work, y.keys[0])).await;
    let head = band(&html);
    let insert = panel(head, "einfuegen");
    assert!(
        head.contains("data-band-panel=\"einfuegen\">"),
        "das Feld Einfügen ist mit ?tab=einfuegen sichtbar: {head}"
    );
    assert!(
        insert.contains("<p class=\"band-note nojs-only\">")
            && insert.contains("<a href=\"#bild-einfuegen\">Bild einfügen …</a>"),
        "ohne JavaScript erklärt ein Hinweis den leeren Reiter und führt zu „Bild einfügen …“ am Blatt: {insert}"
    );
    assert!(
        insert.contains(&format!("<a href=\"/werke/{}/markierung?key={}&amp;rev=1\"", y.work, y.keys[0])),
        "…und zu „Markierung …“ (Glossar und Register als Formulare) für die gezeigte Fassung: {insert}"
    );
    assert!(
        html.contains("<details class=\"image-insert\" id=\"bild-einfuegen\""),
        "das Sprungziel steht auf der Seite"
    );
    let (_, css) = y.server.get("/static/app.css").await;
    assert!(
        css.contains("html.js .nojs-only { display: none !important; }"),
        "mit JavaScript ist der Hinweis verborgen (band.css)"
    );
    assert!(
        css.contains("html:not(.js) .js-only { display: none !important; }"),
        "und js-only ohne JavaScript (z. B. „Zoom zurücksetzen“)"
    );
}

#[test]
fn the_band_style_uses_only_state_attributes_and_known_colours() {
    let css = include_str!("../static/band.css");
    assert!(
        !css.contains("@media"),
        "die Modi hängen nur an den Attributen von <html>, keine Media-Queries (sonst zwei Wahrheiten)"
    );
    assert!(
        css.contains("html[data-compact] .band-group")
            && css.contains("html[data-band=\"closed\"] .band-panels { display: none; }"),
        "kompakt und zugeklappt über die Attribute"
    );
    let app = include_str!("../static/app.css");
    let mut rest = css;
    while let Some(at) = rest.find("var(--") {
        rest = &rest[at + 4..];
        let name = &rest[..rest.find([')', ',']).unwrap()];
        assert!(
            name.starts_with("--band-") || app.contains(&format!("{name}:")),
            "band.css nutzt nur Farbvariablen aus app.css (hell und dunkel definiert) bzw. eigene Maße: {name}"
        );
    }
    assert!(
        css.contains(".band-tab[aria-selected=\"true\"]") && css.contains("box-shadow: inset 0 2px 0 var(--primary)"),
        "der gewählte Reiter ist an Form und Strich erkennbar, nicht nur an der Farbe"
    );
    assert!(css.contains(":focus-visible"), "sichtbarer Fokus");
}

// ---- Behaviour of the widgets in the Node mock ----------------------------------------------------------------------

/// Content of `<body>` of the real work page.
fn body_of(html: &str) -> &str {
    let start = html.find("<body>").expect("body") + "<body>".len();
    &html[start..html.rfind("</body>").expect("body-Ende")]
}

#[tokio::test]
async fn the_band_widgets_dock_the_toolbar_switch_tabs_fold_and_keep_the_shortcuts() {
    let y = yard().await;
    let (w, key) = (y.work, y.keys[0]);
    let (_, html) = y.server.get(&format!("/?work={w}&key={key}")).await;
    let rendered = y.server.rpc("ui.render", json!({ "work": w, "key": key })).await;
    let scenario = json!({
        "scene": "band", "url": format!("http://127.0.0.1:8765/?work={w}&key={key}"), "body": body_of(&html), "cookie": "",
        "rpc": {
            "outline.check": { "result": { "valid": true, "split": [], "merge": false } },
            "outline.save": { "result": { "node": 0, "key": key, "rev": 1, "saved": "amended", "amend_seq": 1 } },
            "ui.render": { "result": ok(&rendered) },
            "wysiwyg.serialize": { "result": { "body": "= Kiel legen\n\nDer Kiel aus Eiche liegt seit dem Morgen auf den Böcken.\n" } },
        },
    });
    let Some(seen) = run_harness(SCENES, &scenario) else {
        eprintln!("ÜBERSPRUNGEN: node fehlt – Verhaltenstest „Menüband“ nicht gelaufen.");
        return;
    };
    assert_eq!(seen["errors"], json!([]), "keine Fehler im Nachbau: {seen}");

    // Docking.
    assert_eq!(
        seen["before"],
        json!({ "barInPane": true, "barHidden": true, "dockChildren": 0 }),
        "vor dem Einhängen: Leiste verborgen am Blatt, Andockplatz leer"
    );
    let docked = &seen["docked"];
    assert_eq!(docked["barInDock"], true, "band-dock hängt die Leiste in den Andockplatz des Bands: {docked}");
    assert_eq!(docked["barInPane"], false, "…sie wird verschoben, nicht kopiert: {docked}");
    assert_eq!(docked["barHidden"], false, "wysiwyg.ts hat sie vorher sichtbar gemacht: {docked}");
    assert_eq!(
        docked["parts"],
        json!(["start", "einfuegen"]),
        "beide Teile sind angedockt (band.css zeigt den des Reiters): {docked}"
    );
    assert_eq!(
        docked["headDocked"], true,
        "der Kopf weiß, dass eine Leiste angedockt ist (Hinweis „Einfügen geht im Editor“ bleibt verborgen)"
    );
    assert_eq!(
        (&docked["satz"], &docked["quelltext"]),
        (&json!("true"), &json!("false")),
        "Vorgabe Satzbild: Umschalter nach dem Andocken unverändert gedrückt: {docked}"
    );
    assert_eq!(docked["sheetOpen"], true, "das Satzbild ist offen");
    // Toggle after docking: wysiwyg.ts finds the bar outside of div.wysiwyg as well.
    let q = &seen["afterQuelltext"];
    assert_eq!(
        (&q["satz"], &q["quelltext"]),
        (&json!("false"), &json!("true")),
        "Klick auf „Quelltext“ im Band: aria-pressed wandert mit (die Leiste liegt nicht mehr unter dem Editor): {q}"
    );
    assert_eq!(q["areaHidden"], false, "das Textfeld ist sichtbar: {q}");
    assert_eq!(
        q["toolsHidden"], true,
        "Absatzformat, K, F gelten nur im Satzbild und sind im Quelltext verborgen: {q}"
    );
    assert_eq!(q["sheetOpen"], false, "das Satzbild ist zu: {q}");

    // Saving: Ctrl+S and the button in the band.
    assert_eq!(
        seen["saveDisabledClean"], true,
        "unverändert ist „Speichern“ der Statuszeile gesperrt (maßgeblicher Knopf, nicht der im Band)"
    );
    assert_eq!(seen["bandSaveIdlePrevented"], true, "dann speichert auch „Speichern“ im Band nicht");
    assert_eq!(seen["saveLabelDirty"], "Speichern", "nach einer Änderung: Knopf der Statuszeile bereit");
    assert_eq!(seen["bandSaveLabel"], "Speichern", "der Knopf im Band behält seine Beschriftung");
    assert_eq!(
        seen["ctrlSPrevented"], true,
        "Strg+S ist verdrahtet, auch mit dem Fokus außerhalb des Textfelds (kein Browser-Dialog „Seite speichern“)"
    );
    assert_eq!(
        seen["dockInertWhileSaving"], true,
        "während des Speicherns ist auch der Andockplatz gesperrt (kein zweites Speichern, kein Umschalten)"
    );
    let calls: Vec<&str> = seen["saveCalls"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    assert!(
        calls.contains(&"outline.save") && calls.contains(&"ui.render"),
        "Strg+S speichert über outline.save und zeichnet neu: {calls:?}"
    );
    assert_eq!(seen["dockInertAfter"], false, "danach ist der Andockplatz wieder frei");
    assert_eq!(
        seen["placeAfterSave"],
        serde_json::json!({ "newArea": true, "focused": true, "start": 4, "end": 9 }),
        "normales Speichern (2026-11-27): im neu gezeichneten Textfeld stehen Fokus und Auswahl wie vorher"
    );
    assert_eq!(
        seen["redocked"],
        json!({ "bars": 1, "barInDock": true, "barInPane": false }),
        "nach ui.render dockt die neue Leiste an, die alte ist weg (genau eine)"
    );

    // Tabs with the keyboard.
    let t = &seen["tabs"];
    assert_eq!(t["start"]["tab"], "start", "Ausgangszustand vom Server: {t}");
    assert_eq!(t["rightPrevented"], true, "→ ist verbraucht");
    assert_eq!(t["afterRight"]["tab"], "einfuegen", "→ wählt den nächsten Reiter ohne Neuladen: {t}");
    assert_eq!(t["afterRight"]["selected"], json!(["einfuegen"]), "genau er ist aria-selected: {t}");
    assert_eq!(t["afterRight"]["shown"], json!(["einfuegen"]), "nur sein Feld ist sichtbar: {t}");
    assert_eq!(t["afterRight"]["focus"], "einfuegen", "der Fokus wandert mit (ARIA-Tabs): {t}");
    assert_eq!(t["afterEnd"]["tab"], "ansicht", "Ende springt zum letzten Reiter");
    assert_eq!(t["afterWrap"]["tab"], "start", "→ am Ende läuft um");
    assert_eq!(t["afterLeft"]["tab"], "ansicht", "← am Anfang läuft um");
    assert_eq!(t["afterHome"]["tab"], "start", "Pos1 springt zum ersten");
    assert_eq!(t["tabIndex"], json!([0, -1, -1, -1, -1]), "nur der gewählte Reiter steht in der Tab-Folge");

    // Clicks.
    let c = &seen["clicks"];
    assert_eq!(c["buchPrevented"], true, "ein Klick auf einen Reiter lädt die Seite nicht neu");
    assert_eq!(c["afterBuch"]["tab"], "buch", "Klick wählt den Reiter");
    assert_eq!(c["afterBuch"]["band"], "open", "das Band bleibt offen");
    assert_eq!(
        c["afterActive"]["band"], "closed",
        "Klick auf den aktiven Reiter klappt das Band zu (data-band an <html>): {c}"
    );
    assert!(
        c["afterActive"]["cookie"].as_str().unwrap().contains("baruch_band=closed"),
        "…und merkt es im Cookie baruch_band: {c}"
    );
    assert_eq!(c["afterActive"]["expanded"], "false", "der Schalter sagt es Hilfsmitteln (aria-expanded)");
    assert_eq!(
        (&c["peek"]["tab"], &c["peek"]["peek"], &c["peek"]["band"]),
        (&json!("pruefen"), &json!(true), &json!("closed")),
        "zugeklappt zeigt ein Reiter sein Feld vorübergehend, das Band bleibt zu: {c}"
    );
    assert_eq!(c["afterEscape"]["peek"], false, "Esc blendet das Feld wieder aus");
    assert_eq!(c["afterOutside"]["peek"], false, "ein Klick daneben ebenso");
    assert_eq!(c["afterToggle"]["band"], "open", "der Schalter öffnet das Band dauerhaft");
    assert!(
        !c["afterToggle"]["cookie"].as_str().unwrap().contains("baruch_band=closed"),
        "und nimmt das Cookie zurück: {c}"
    );

    // Groups.
    let g = &seen["groups"];
    let count = g["full"]["count"].as_u64().unwrap();
    assert!(
        g["full"]["dockGroups"].as_u64().unwrap() == 7,
        "die sieben Gruppen der angedockten Leiste gehören zum Band: {g}"
    );
    assert_eq!(
        (g["full"]["open"].as_u64().unwrap(), g["full"]["named"].as_u64().unwrap()),
        (count, 0),
        "voll: alle Gruppen offen, keine als Popup: {g}"
    );
    assert_eq!(
        (g["compact"]["open"].as_u64().unwrap(), g["compact"]["named"].as_u64().unwrap()),
        (0, count),
        "nach baruch:laptop (kompakt): alle zu, alle als Popup mit gemeinsamem Namen – auch die angedockten: {g}"
    );
    assert_eq!(g["afterEscape"]["open"], 0, "Esc schließt ein offenes Popup: {g}");
    assert_eq!(
        (g["fullAgain"]["open"].as_u64().unwrap(), g["fullAgain"]["named"].as_u64().unwrap()),
        (count, 0),
        "zurück im vollen Band sind alle wieder offen: {g}"
    );

    // Keyboard shortcuts.
    for (name, what) in [
        ("altL", "Alt+L öffnet die Literatur"),
        ("altE", "Alt+E öffnet „Einfall festhalten“"),
        ("altComma", "Alt+, öffnet die Einstellungen"),
    ] {
        assert_eq!(
            seen["keys"][name],
            json!({ "dialog": true, "prevented": true, "open": true }),
            "{what}: {}",
            seen["keys"]
        );
    }
    assert_eq!(
        seen["openers"],
        json!({ "data-settings-open": 1, "data-literature-open": 1, "data-idea-open": 1, "data-work-import-open": 1, "data-view-toggle": 1 }),
        "je Haken genau ein Element im Band"
    );
}
