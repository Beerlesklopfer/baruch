// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 33 – acceptance by the integrator (contract §11): cut through the pages with band, active tab and data-laptop,
//! after the three tasks (menueband, laptop, seiten) have been merged.
//!
//! The details are checked by the tasks themselves (round33_menueband, round33_laptop, round33_seiten); here is what
//! only holds together: every page carries the band including its state, the tabs are links without JavaScript, the
//! entries of the old menus are also on a subpage, cookies and `?laptop=on` take effect everywhere, the badges are
//! there. Every assertion with its reason, only one invented work (garden book of the tests), no network, no Eliezer.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};

use crate::common;

/// The band of the page (from the opening tag to `</header>`).
fn band(html: &str) -> &str {
    let start = html.find("<header class=\"band\"").unwrap_or_else(|| panic!("kein Menüband: {html}"));
    let end = start + html[start..].find("</header>").expect("Band ohne Ende");
    &html[start..end]
}

/// The opening tag `<html …>` (data-laptop, data-compact, data-narrow, data-rail-*, data-band are there).
fn html_tag(html: &str) -> &str {
    let start = html.find("<html").expect("<html>");
    &html[start..start + html[start..].find('>').unwrap()]
}

/// Identifiers of the tabs in the order of the page.
fn tabs(head: &str) -> Vec<&str> {
    head.split("data-band-tab=\"").skip(1).filter_map(|rest| rest.split('"').next()).collect()
}

/// The opening tag of the tab `id` (from `<a class="band-tab"` to `>`).
fn tab_tag<'a>(head: &'a str, id: &str) -> &'a str {
    let at = head.find(&format!("data-band-tab=\"{id}\"")).unwrap_or_else(|| panic!("Reiter {id} fehlt: {head}"));
    let start = head[..at].rfind("<a ").unwrap();
    &head[start..at + head[at..].find('>').unwrap()]
}

async fn get(server: &common::Server, uri: &str, cookie: Option<&str>) -> (HeaderMap, String) {
    let mut request = Request::get(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let (status, headers, html) = server.send(request.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{uri} lädt: {html}");
    (headers, html)
}

fn set_cookies(headers: &HeaderMap) -> Vec<String> {
    headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_owned()).collect()
}

/// Checks that exactly the tab `active` is selected: data-tab at the header, aria-selected, exactly one open panel.
fn assert_active(html: &str, uri: &str, active: &str) {
    let head = band(html);
    assert!(
        head.starts_with(&format!("<header class=\"band\" data-widget=\"band\" data-tab=\"{active}\"")),
        "{uri}: der Kopf nennt den aktiven Reiter {active} (daran hängen CSS und band.ts): {}",
        &head[..head.find('>').unwrap()]
    );
    for id in tabs(head) {
        let selected = tab_tag(head, id).contains("aria-selected=\"true\"");
        assert_eq!(selected, id == active, "{uri}: aria-selected nur am aktiven Reiter (Reiter {id})");
        let open = head.contains(&format!("data-band-panel=\"{id}\">"));
        let hidden = head.contains(&format!("data-band-panel=\"{id}\" hidden>"));
        assert!(open != hidden, "{uri}: das Feld {id} ist entweder offen oder verborgen");
        assert_eq!(open, id == active, "{uri}: ohne JavaScript ist genau das Feld des aktiven Reiters sichtbar ({id})");
    }
}

struct Garden {
    server: common::Server,
    work: i64,
    key: i64,
}

async fn garden() -> Garden {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let key = server.state.store.load_work(work).await.unwrap().nodes[1].key;
    Garden { server, work, key }
}

/// (address, default tab, tabs of the page). Pages with four or five tabs are fetched as in the browser with the cookie
/// `baruch_work` (the work page sets it; some pages – e.g. `/einfaelle` – know their work in the band only through it,
/// as already before Round 33); pages with two tabs without the cookie, so that they really have no work.
fn pages(g: &Garden) -> Vec<(String, &'static str, Vec<&'static str>)> {
    let (w, k) = (g.work, g.key);
    let all = vec!["start", "einfuegen", "pruefen", "buch", "ansicht"];
    let with_work = vec!["start", "pruefen", "buch", "ansicht"];
    let without = vec!["start", "ansicht"];
    vec![
        (format!("/?work={w}&key={k}"), "start", all),
        ("/werke".into(), "start", without.clone()),
        ("/werke/neu".into(), "start", without.clone()),
        ("/ueber".into(), "start", without),
        (format!("/gliederung?work={w}"), "start", with_work.clone()),
        (format!("/einfaelle?work={w}"), "start", with_work.clone()),
        (format!("/werke/{w}/karte"), "start", with_work.clone()),
        (format!("/werke/{w}/satz?key={k}"), "buch", with_work.clone()),
        (format!("/werke/{w}/bearbeiten"), "buch", with_work.clone()),
        (format!("/werke/{w}/glossar"), "buch", with_work.clone()),
        (format!("/werke/{w}/bilder"), "buch", with_work.clone()),
        (format!("/werke/{w}/umschlag"), "buch", with_work.clone()),
        (format!("/werke/{w}/befunde"), "pruefen", with_work.clone()),
        (format!("/suche?work={w}"), "pruefen", with_work.clone()),
        (format!("/verweise?work={w}"), "pruefen", with_work.clone()),
        (format!("/literatur?work={w}"), "pruefen", with_work.clone()),
        (format!("/einstellungen/darstellung?work={w}"), "ansicht", with_work.clone()),
        (format!("/einstellungen/assistenten?work={w}"), "ansicht", with_work),
    ]
}

#[tokio::test]
async fn every_page_carries_the_band_its_tab_and_the_laptop_state_and_every_tab_is_a_link() {
    let g = garden().await;
    for (uri, default, expected_tabs) in pages(&g) {
        let work_cookie = format!("baruch_work={}", g.work);
        let cookie = (expected_tabs.len() > 2).then_some(work_cookie.as_str());
        let (_, html) = get(&g.server, &uri, cookie).await;
        assert_eq!(html.matches("<header class=\"band\"").count(), 1, "{uri}: genau ein Menüband");
        assert!(!html.contains("class=\"topbar\""), "{uri}: die alte Kopfzeile steht nicht daneben");
        assert!(!html.contains("data-band-fill"), "{uri}: die Schicht hat die Marke an <html> ersetzt");
        let tag = html_tag(&html);
        assert!(
            tag.contains(" data-laptop=\"auto\"") && tag.ends_with(" data-band=\"open\""),
            "{uri}: <html> trägt den Laptop-Zustand (ohne Cookie automatisch, breit) und data-band – einzige Quelle für \
             CSS und Widgets: {tag}"
        );
        assert!(
            !tag.contains("data-compact") && !tag.contains("data-narrow") && !tag.contains("data-rail-"),
            "{uri}: ohne Cookie kennt der Server keine Breite und zeichnet breit (Vertrag §2): {tag}"
        );
        let head = band(&html);
        assert_eq!(tabs(head), expected_tabs, "{uri}: Reiter der Seite (Arbeitsseite fünf, mit Werk vier, ohne zwei)");
        assert_active(&html, &uri, default);
        assert!(
            head.contains("<details class=\"menu\" data-menu=\"baruch\">"),
            "{uri}: Menü „Baruch ▾“ auf jeder Seite"
        );
        assert!(
            html.contains("</header>\n<!--baruch-consent-->") || !html.contains("<!--baruch-consent-->"),
            "{uri}: die Marke für die Zustimmung bleibt direkt unter dem Kopf (oder ist schon ersetzt)"
        );
        // Every tab is a link to the same page with ?tab=… without JavaScript; following it selects it.
        let join = if uri.contains('?') { '&' } else { '?' };
        for id in &expected_tabs {
            let link = tab_tag(head, id);
            assert!(
                link.starts_with("<a class=\"band-tab\" role=\"tab\"") && link.contains(&format!("tab={id}")),
                "{uri}: Reiter {id} ist ein gewöhnlicher Link mit seiner Kennung in der Adresse: {link}"
            );
            let followed = format!("{uri}{join}tab={id}");
            let (_, html) = get(&g.server, &followed, cookie).await;
            assert_active(&html, &followed, id);
        }
        // Laptop mode „An“ from the cookie: compact and narrow on every page, groups as closed details.
        let laptop_on = cookie.map_or_else(|| "baruch_laptop=on".to_owned(), |c| format!("{c}; baruch_laptop=on"));
        let (_, html) = get(&g.server, &uri, Some(&laptop_on)).await;
        let tag = html_tag(&html);
        assert!(
            tag.contains(" data-laptop=\"on\" data-compact data-narrow data-rail-left data-rail-right"),
            "{uri}: „An“ zeichnet der Server ohne JavaScript kompakt mit eingeklappten Leisten: {tag}"
        );
        let head = band(&html);
        assert!(
            !head.contains(" data-band-group open>") && head.contains(" data-band-group name=\"band-group\">"),
            "{uri}: im kompakten Band sind die Gruppen zugeklappte details (Popup ohne JavaScript, nur eines offen)"
        );
    }
}

#[tokio::test]
async fn every_entry_of_the_former_menus_is_a_link_on_a_sub_page_too() {
    // The work page is checked by round33_menueband::every_entry_of_the_former_menus_is_a_link_on_the_workspace; here
    // the same list old → new on a subpage (outline), where there are neither dialogs nor a dock.
    let g = garden().await;
    let w = g.work;
    let (_, html) = get(&g.server, &format!("/gliederung?work={w}"), None).await;
    let head = band(&html);
    let entries: Vec<(String, &str)> = vec![
        ("href=\"/werke\">Alle Werke</a>".into(), "Werk ▾ → Alle Werke"),
        (
            "href=\"/werke/neu\">Neues Werk / Importieren…</a>".into(),
            "Werk ▾ → Neues Werk / Importieren… (hier die Seite, kein Dialog)",
        ),
        ("href=\"/werke/import/quelle\"".into(), "Werk ▾ → PDF oder EPUB importieren…"),
        (format!("href=\"/austausch/einlesen?work={w}\""), "Werk ▾ → Austausch: Einlesen…"),
        (format!("href=\"/einstellungen/darstellung?work={w}\" data-settings-open"), "⚙ Einstellungen (Alt+,)"),
        ("href=\"/ueber\"".into(), "Werkzeuge ▾ → Über Baruch"),
        (
            format!(
                "href=\"/?work={w}\" title=\"Zur Arbeitsseite mit Gliederung, Editor und Assistenten\">Zum Text</a>"
            ),
            "Ansicht ▾ → Editor heißt außerhalb der Arbeitsseite „Zum Text“",
        ),
        (format!("href=\"/einfaelle?work={w}\" data-idea-open"), "Werkzeuge ▾ → Einfall festhalten (Alt+E)"),
        (
            format!("href=\"/einfaelle?work={w}\" title=\"Liste der Einfälle dieses Werks\">Einfälle</a>"),
            "Werkzeuge ▾ → Einfälle",
        ),
        (format!("href=\"/werke/{w}/karte\""), "Werkzeuge ▾ → Gedankenkarte"),
        (format!("href=\"/werke/{w}/befunde\""), "Werk ▾ → Befunde"),
        (format!("href=\"/werke/{w}/pruefung\""), "Werkzeuge ▾ → Zitate prüfen"),
        (format!("href=\"/suche?work={w}\""), "Werkzeuge ▾ → Im Werk suchen und ersetzen"),
        (format!("href=\"/verweise?work={w}\""), "Werkzeuge ▾ → Querverweise prüfen"),
        (format!("href=\"/literatur?work={w}\" data-literature-open"), "Werkzeuge ▾ → Literatur (Alt+L)"),
        (format!("href=\"/abgleich?work={w}\""), "Werkzeuge ▾ → Abgleichen"),
        (format!("href=\"/werke/{w}/bearbeiten\""), "Werk ▾ → Werk bearbeiten"),
        (format!("href=\"/gliederung?work={w}\""), "Werk ▾ → Gliederung"),
        (format!("href=\"/gliederung?work={w}#antraege\""), "Werk ▾ → Anträge zur Gliederung"),
        (format!("href=\"/werke/{w}/nummerierung\""), "Werk ▾ → Gliederungsschema"),
        (format!("href=\"/werke/{w}/bilder\""), "Werk ▾ → Bilder"),
        (format!("href=\"/werke/{w}/satz\""), "Ansicht ▾ → Satz (jetzt „Satz-Vorschau“)"),
        (format!("href=\"/werke/{w}/epub/vorschau\""), "Ansicht ▾ → EPUB-Vorschau"),
        (format!("href=\"/werke/{w}/pdf\" download"), "Werk ▾ → Als PDF herunterladen"),
        (format!("href=\"/werke/{w}/pdf?entwurf=1\" download"), "Werk ▾ → Als Entwurf herunterladen"),
        (format!("href=\"/werke/{w}/epub\" download"), "Werk ▾ → Als EPUB herunterladen"),
        (format!("href=\"/werke/{w}/stichwortzettel\""), "Werk ▾ → Stichwortzettel …"),
        (format!("href=\"/werke/{w}/export\""), "Werk ▾ → Exportieren (Typst)"),
        (format!("href=\"/werke/{w}/austausch\""), "Werk ▾ → Austausch: Ausgeben"),
        (format!("href=\"/werke/{w}/umschlag\""), "Werk ▾ → Umschlag"),
        (format!("href=\"/werke/{w}/umschlag.pdf\" download"), "Werk ▾ → Umschlag-PDF"),
        (format!("href=\"/werke/{w}/glossar\""), "Werk ▾ → Glossar"),
        (format!("href=\"/werke/{w}/register\""), "Werk ▾ → Register"),
    ];
    for (piece, why) in &entries {
        assert!(head.contains(piece.as_str()), "Unterseite Gliederung: {why} fehlt im Band ({piece}): {head}");
    }
    for hook in ["data-literature-open", "data-idea-open", "data-settings-open"] {
        assert_eq!(
            html.matches(hook).count(),
            1,
            "{hook} steht je Seite genau einmal (die Widgets nehmen querySelector)"
        );
    }
    assert!(
        !head.contains("data-rail-toggle")
            && !head.contains("data-band-dock")
            && !head.contains("data-band-tab=\"einfuegen\""),
        "Seitenleisten, Andockplatz und der Reiter Einfügen gehören nur zur Arbeitsseite: {head}"
    );
    for mode in ["auto", "on", "off"] {
        assert!(
            head.contains(&format!("data-laptop-set=\"{mode}\"")),
            "der Laptop-Schalter „{mode}“ steht im Reiter Ansicht jeder Seite"
        );
    }
}

#[tokio::test]
async fn laptop_keys_and_cookies_work_together_with_the_band_and_the_work_cookie() {
    let g = garden().await;
    let w = g.work;
    // Keys in the address: take effect in the same answer and are remembered – next to baruch_work (append!).
    let (headers, html) = get(&g.server, &format!("/?work={w}&laptop=on&band=closed&tab=buch"), None).await;
    let cookies = set_cookies(&headers);
    for wanted in ["baruch_laptop=on;", "baruch_band=closed;", "baruch_work="] {
        assert!(
            cookies.iter().any(|c| c.starts_with(wanted)),
            "Set-Cookie {wanted} steht in der Antwort (die Schicht hängt an, sie ersetzt das Werk-Cookie nicht): {cookies:?}"
        );
    }
    let tag = html_tag(&html);
    assert!(
        tag.contains("data-laptop=\"on\" data-compact data-narrow") && tag.ends_with(" data-band=\"closed\""),
        "beide Schlüssel wirken schon in dieser Antwort: {tag}"
    );
    assert_active(&html, "/?work=…&laptop=on&band=closed&tab=buch", "buch");
    let head = band(&html);
    assert!(
        tab_tag(head, "pruefen").contains(&format!("href=\"?work={w}&amp;tab=pruefen&amp;band=open\"")),
        "die Reiter behalten das Werk, lassen laptop/band/tab aus dem Rest und öffnen das zugeklappte Band: {}",
        tab_tag(head, "pruefen")
    );
    // Width from the cookie of the widget: medium = only compact, narrow = rails as well; „Aus“ beats both.
    let cases = [
        ("baruch_vw=m", true, false),
        ("baruch_vw=s", true, true),
        ("baruch_vw=w", false, false),
        ("baruch_vw=s; baruch_laptop=off", false, false),
        ("baruch_vw=w; baruch_laptop=on", true, true),
    ];
    for (cookie, compact, narrow) in cases {
        let (_, html) = get(&g.server, &format!("/?work={w}"), Some(cookie)).await;
        let tag = html_tag(&html);
        assert_eq!(tag.contains(" data-compact"), compact, "{cookie}: kompakt unter 1600 px bzw. „An“: {tag}");
        assert_eq!(tag.contains(" data-narrow"), narrow, "{cookie}: Leisten unter 1280 px bzw. „An“: {tag}");
        assert_eq!(
            tag.contains(" data-rail-left") && tag.contains(" data-rail-right"),
            narrow,
            "{cookie}: schmal klappt beide Seitenleisten ein: {tag}"
        );
        assert!(
            html.contains("id=\"pane-tree\"") && html.contains("id=\"pane-assistants\""),
            "die Spalten bleiben im HTML (Schublade)"
        );
    }
    // One rail alone, permanently: ?lrail=1 takes effect immediately and is remembered; the switch in the tab Ansicht
    // flips.
    let (headers, html) = get(&g.server, &format!("/?work={w}&lrail=1"), None).await;
    assert!(set_cookies(&headers).iter().any(|c| c.starts_with("baruch_lrail=1;")), "lrail wird gemerkt");
    let tag = html_tag(&html);
    assert!(
        tag.contains(" data-rail-left") && !tag.contains(" data-rail-right"),
        "nur die Gliederung ist eingeklappt: {tag}"
    );
    let head = band(&html);
    assert!(
        head.contains("data-rail-toggle=\"left\" aria-pressed=\"false\"") && head.contains("lrail=0"),
        "der Schalter „Gliederung“ ist nicht gedrückt und führt zum Ausklappen (lrail=0): {head}"
    );
    assert!(
        head.contains("data-rail-toggle=\"right\" aria-pressed=\"true\""),
        "der Schalter „Assistenten“ ist gedrückt (Spalte sichtbar): {head}"
    );
    // Invalid input is ignored: no cookie, no state, nothing of it in the HTML tag.
    let (headers, html) = get(&g.server, &format!("/?work={w}&laptop=%22%3E%3Cscript%3E&tab=quatsch"), None).await;
    assert!(
        !set_cookies(&headers).iter().any(|c| c.starts_with("baruch_laptop")),
        "ein ungültiger Wert setzt kein Cookie: {:?}",
        set_cookies(&headers)
    );
    assert!(html_tag(&html).contains(" data-laptop=\"auto\""), "und ändert den Zustand nicht: {}", html_tag(&html));
    assert_active(&html, "/?work=…&tab=quatsch", "start");
    assert!(
        !band(&html).contains("<script>") && !band(&html).contains("%3Cscript"),
        "der ungültige Wert steht nirgends im Band: laptop gehört nicht zum Rest der Abfrage, auch nicht kodiert"
    );
}

#[tokio::test]
async fn badges_stand_at_the_tabs_of_a_sub_page_without_javascript() {
    let g = garden().await;
    let w = g.work;
    let seq = g.server.state.store.node_seq(w, g.key).await.unwrap();
    let loaded = g.server.state.store.load_work(w).await.unwrap();
    let base = loaded.nodes[seq].current_number().unwrap();
    // A proposal keeps the heading of the section (otherwise the store rejects it): current text plus one sentence.
    let body =
        format!("{}\nDie Beete liegen jetzt am Zaun.\n", loaded.nodes[seq].revision(base).unwrap().body.trim_end());
    g.server.state.store.add_proposal_on(w, seq, &body, "@lektor", "m", "", base, None).await.unwrap();
    for uri in [format!("/werke/{w}/bilder"), format!("/?work={w}")] {
        let (_, html) = get(&g.server, &uri, Some(&format!("baruch_work={w}"))).await;
        let head = band(&html);
        let at = head.find("data-band-tab=\"start\"").unwrap();
        let tab = &head[at..at + head[at..].find("</a>").unwrap()];
        assert!(
            tab.contains(
                "<span class=\"badge\" data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\">1</span>"
            ),
            "{uri}: ein offener Vorschlag steht als Abzeichen am Reiter Start – im ausgelieferten HTML, ohne JavaScript: {tab}"
        );
        let at = head.find("data-band-tab=\"pruefen\"").unwrap();
        let tab = &head[at..at + head[at..].find("</a>").unwrap()];
        assert!(
            tab.contains("data-badge=\"open_findings\"") && tab.contains(" hidden>"),
            "{uri}: ohne offene Befunde bleibt das Abzeichen am Reiter Prüfen verborgen (Null → kein Abzeichen): {tab}"
        );
    }
}

#[test]
fn the_integration_left_no_dead_branch_and_no_placeholder_behind() {
    let toolbar = include_str!("../templates/fragments/edit_toolbar.html");
    assert!(
        !toolbar.contains("{%- if satz_page") && !toolbar.contains("ribbon-satz"),
        "der tote Zweig satz_page (Satz-Seite) ist nach dem Zusammenführen entfernt (Vertrag §5/§11)"
    );
    assert!(
        toolbar.contains("data-band-part=\"start\"") && toolbar.contains("data-band-part=\"einfuegen\""),
        "geblieben sind die zwei Teile für die Reiter Start und Einfügen"
    );
    assert!(
        !include_str!("../static/ribbon.css").contains(".ribbon-satz"),
        "auch die Regeln der alten Satz-Leiste sind weg"
    );
    let guide = include_str!("../../../docs/Bedienung.md");
    assert!(
        !guide.contains("<!-- R33 menueband -->") && !guide.contains("<!-- R33 laptop -->"),
        "die Abschnitte aus docs/r33 sind an den Platzhaltern eingesetzt (Vertrag §8, Doku-Regel)"
    );
    // Round 37: the user guide for users is in the help of the website; the sections of the ribbon are therefore
    // searched for in the responsible help page (docs/Bedienung.md is only a pointer and a technical note now).
    let help = include_str!("../../../site/content/de/hilfe/menueband.md");
    assert!(
        !help.contains("<!-- R33 menueband -->") && !help.contains("<!-- R33 laptop -->"),
        "auch die Hilfeseite trägt keinen Platzhalter aus docs/r33 (ein HTML-Kommentar bräche dort zudem den Bau)"
    );
    for heading in ["## Menüband", "### Band ein- und ausklappen", "### Laptop-Modus"] {
        assert!(help.contains(heading), "site/content/de/hilfe/menueband.md beschreibt „{heading}“");
    }
    assert!(
        guide.contains("site/content/de/hilfe/menueband.md"),
        "docs/Bedienung.md führt in „Wo steht was“ vom früheren Abschnitt „Menüband“ zur Hilfeseite"
    );
    let base = include_str!("../templates/base.html");
    for mark in ["{#- R32-PLATZ import-word -#}", "{#- R32-PLATZ beenden -#}", "{#- R32-PLATZ export-word -#}"] {
        assert_eq!(
            base.matches(mark).count(),
            1,
            "die Marke {mark} für das Zusammenführen mit Runde 32 steht genau einmal"
        );
    }
}
