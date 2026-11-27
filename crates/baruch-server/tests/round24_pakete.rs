// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 24, Aufgabe „pakete“ (Schnittstelle R24-S1): Über-Seite (`/ueber`, `/ueber/lizenz`), Menüeintrag
//! „Über Baruch“, Link neben der Version unter Einstellungen → Daten.
//!
//! Die Seite fragt nie im Netz nach: Geprüft wird das am ausgelieferten HTML (nichts, was ein Browser von selbst
//! von einem fremden Rechner lädt), an einem lauschenden Nachbau auf 127.0.0.1:0, auf den die Verbindungen
//! zeigen (keine Verbindung beim Abruf), und am Quelltext des Moduls (kein HTTP-Client). Alle Texte frei erfunden.

use crate::common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::about;
use serde_json::json;

/// minijinja maskiert `/` als `&#x2f;`; der Browser liest beides gleich.
fn unescape(html: &str) -> String {
    html.replace("&#x2f;", "/")
}

/// Ein Menü der Kopfzeile (`<details class="menu" data-menu="…">` bis `</details>`).
fn menu<'a>(html: &'a str, name: &str) -> &'a str {
    let marker = format!("<details class=\"menu\" data-menu=\"{name}\">");
    let start = html.find(&marker).unwrap_or_else(|| panic!("Menü {name} fehlt: {html}"));
    let end = start + html[start..].find("</details>").expect("Menü ohne Ende");
    &html[start..end]
}

/// Der Inhalt der Seite (`<main …>` bis `</main>`), ohne Kopfzeile und Dialoge.
fn main_part(html: &str) -> &str {
    let start = html.find("<main").unwrap_or_else(|| panic!("kein <main>: {html}"));
    let end = start + html[start..].find("</main>").expect("<main> ohne Ende");
    &html[start..end]
}

/// Werte aller Attribute `name="…"` im HTML.
fn attributes<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let marker = format!(" {name}=\"");
    html.split(marker.as_str()).skip(1).filter_map(|rest| rest.split('"').next()).collect()
}

#[tokio::test]
async fn the_about_page_shows_version_license_source_and_never_asks_the_net() {
    let server = common::open().await;
    let (status, raw) = server.get("/ueber").await;
    assert_eq!(status, StatusCode::OK, "/ueber lädt: {raw}");
    let html = unescape(&raw);
    let page = main_part(&html);

    assert_eq!(
        about::VERSION,
        env!("CARGO_PKG_VERSION"),
        "die Version der Über-Seite ist die des Manifests (eine Quelle: [workspace.package] version)"
    );
    assert!(
        page.contains(&format!("<dt>Version</dt>\n    <dd data-about-version>{}</dd>", env!("CARGO_PKG_VERSION"))),
        "Version aus CARGO_PKG_VERSION steht auf der Seite: {page}"
    );
    assert!(page.contains("<dt>Name</dt>\n    <dd>Baruch</dd>"), "der Name des Programms steht da: {page}");
    assert!(
        page.contains("<a href=\"/ueber/lizenz\">AGPL-3.0-or-later</a>"),
        "Lizenz als SPDX-Kennung mit Link auf den Lizenztext im Programm (ohne Netz lesbar): {page}"
    );
    assert!(
        page.contains(
            "<a href=\"https://github.com/Beerlesklopfer/baruch\" rel=\"noopener noreferrer\">https://github.com/Beerlesklopfer/baruch</a>"
        ),
        "Quellcode-Angebot (AGPL §13): Link auf das Repo, Adresse auch als Text lesbar: {page}"
    );
    assert!(
        page.contains(&format!("beim Tag <code>v{}</code>", env!("CARGO_PKG_VERSION"))),
        "das Angebot nennt den Tag genau dieser Version: {page}"
    );
    assert!(
        page.contains(
            "<a href=\"https://baruch.bernau.family/\" rel=\"noopener noreferrer\">https://baruch.bernau.family/</a>"
        ),
        "Webseite verlinkt: {page}"
    );
    assert!(
        page.contains("Baruch fragt nie von sich aus im Netz nach neuen Versionen."),
        "der Satz zur Aktualisierung steht wörtlich da (Vertrag R24 §1): {page}"
    );
    let release = format!("https://github.com/Beerlesklopfer/baruch/releases/tag/v{}", env!("CARGO_PKG_VERSION"));
    assert!(
        page.contains(&format!("<a href=\"{release}\" rel=\"noopener noreferrer\">Release-Seite dieser Version</a>")),
        "die Release-Seite ist ein gewöhnlicher Link (Klick des Menschen = Zustimmung): {page}"
    );
    assert!(
        page.contains("sudo apt upgrade") && page.contains("von Hand"),
        "wie aktualisiert wird, steht dabei (APT bzw. von Hand): {page}"
    );
    let data = server.state.data.display().to_string();
    assert!(
        page.contains(&format!("<code data-about-data-dir>{data}</code>")),
        "der Datenordner dieses Servers steht da ({data}): {page}"
    );
    assert!(
        page.contains("nie im Installationsverzeichnis") && page.contains("löscht dort nichts"),
        "die Zusage zu den Daten (nur im Nutzerordner, Entfernen löscht nichts) steht da: {page}"
    );
    for (package, place) in about::THIRD_PARTY_PLACES {
        assert!(
            page.contains(&format!("<dt>{package}</dt>\n    <dd><code>{place}</code></dd>")),
            "{package}: Ort der Drittlizenzen genannt ({place}): {page}"
        );
    }
    assert!(
        page.contains("data-about-third-party-missing") && !page.contains("Auf diesem Rechner:"),
        "neben der Test-Binärdatei liegt keine THIRD-PARTY-LICENSES.txt – die Seite behauptet keinen Pfad: {page}"
    );
    assert!(
        !page.contains("data-about-appimage"),
        "der Hinweis auf das eingehängte AppImage gehört zum gefundenen Pfad – ohne Pfad erscheint er nie: {page}"
    );
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste von Platzhaltern: {html}");
    assert!(!page.to_lowercase().contains("todo"), "kein Platzhaltertext auf der Seite: {page}");
}

#[tokio::test]
async fn the_about_page_is_reachable_from_the_menu_without_javascript() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let entry = "<a class=\"menu-item\" role=\"menuitem\" href=\"/ueber\" \
                 title=\"Version, Lizenz, Quellcode und Bibliotheken Dritter\">Über Baruch</a>";
    // Arbeitsseite, eine Seite mit Werk, eine ohne Werk, eine Einstellungsseite und die Über-Seite selbst.
    for uri in [
        format!("/?work={work}"),
        format!("/gliederung?work={work}"),
        "/werke".to_owned(),
        "/einstellungen/daten".to_owned(),
        "/ueber".to_owned(),
    ] {
        let (status, raw) = server.get(&uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt: {raw}");
        let html = unescape(&raw);
        // Runde 33 (Menüband): „Über Baruch“ stand am Ende des Menüs Werkzeuge ▾ und steht jetzt im Menü Baruch ▾
        // (Programm) – weiter genau einmal, als gewöhnlicher Link (dahinter folgt seit
        // Runde 32 nur „Baruch beenden…“, siehe unten).
        let tools = menu(&html, "baruch");
        assert_eq!(
            tools.matches(entry).count(),
            1,
            "{uri}: „Über Baruch“ genau einmal im Menü Baruch, als gewöhnlicher Link (ohne JavaScript): {tools}"
        );
        // Runde 32 (Schnittstelle R32-S1): Hinter „Über Baruch“ steht „Baruch beenden…“, wie „Beenden“ in anderen
        // Programmen. Runde 35 (Vertrag §3.2): Danach folgt der Eintrag zum Konto – ohne Konten „Konten einrichten…“.
        let quit = "<a class=\"menu-item\" role=\"menuitem\" href=\"/beenden\" \
                    title=\"Baruch auf diesem Rechner beenden (mit Rückfrage)\">Baruch beenden…</a>";
        let rest = tools.split(entry).nth(1).unwrap_or("").trim();
        let account = rest.strip_prefix(quit).unwrap_or_else(|| {
            panic!("{uri}: „Über Baruch“ steht am Ende des Menüs, dahinter folgt „Baruch beenden…“: {tools}")
        });
        let account = account.trim();
        assert!(
            account.starts_with("<a class=\"menu-item\" role=\"menuitem\" href=\"/einrichten\"")
                && account.ends_with(">Konten einrichten…</a></div>")
                && account.matches("<a ").count() == 1,
            "{uri}: hinter „Baruch beenden…“ folgt allein der Eintrag zum Konto (ohne Konten: „Konten einrichten…“): \
             {tools}"
        );
    }
    // Der Link führt wirklich auf die Seite; ein Formular oder Skript ist dafür nicht nötig.
    let (status, raw) = server.get("/ueber").await;
    assert_eq!(status, StatusCode::OK, "Ziel des Menüeintrags lädt");
    let html = unescape(&raw);
    let page = main_part(&html);
    assert!(page.contains("<h1 class=\"pane-title\">Über Baruch</h1>"), "es ist die Über-Seite: {page}");
    assert!(
        !page.contains("<script") && !page.contains("<form") && !page.contains("data-widget"),
        "der Inhalt der Seite braucht weder Skript noch Formular noch Widget: {page}"
    );
    assert!(html.contains("<title>Über Baruch – Baruch</title>"), "Titel des Fensters: {html}");
}

#[tokio::test]
async fn the_license_text_is_served_as_plain_text() {
    let server = common::open().await;
    let (status, headers, text) = server.send(Request::get("/ueber/lizenz").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "/ueber/lizenz lädt");
    assert_eq!(
        headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
        Some("text/plain; charset=utf-8"),
        "Lizenztext als text/plain in UTF-8 (kein HTML, nichts wird ausgeführt)"
    );
    assert_eq!(text, about::LICENSE_TEXT, "ausgeliefert wird genau der eingebettete Text");
    assert_eq!(
        text,
        include_str!("../../../LICENSE"),
        "der eingebettete Text ist die Datei LICENSE des Repos – dieselbe, die in jedem Paket liegt"
    );
    assert!(
        text.contains("GNU AFFERO GENERAL PUBLIC LICENSE") && text.contains("Version 3, 19 November 2007"),
        "es ist die AGPL Version 3: {}",
        text.chars().take(200).collect::<String>()
    );
    assert!(
        text.contains("13. Remote Network Interaction"),
        "der vollständige Text samt §13 (Quellcode-Angebot bei Nutzung über das Netz), nicht nur der Kopf"
    );
}

#[tokio::test]
async fn the_data_settings_link_the_about_page_next_to_the_version() {
    let server = common::open().await;
    let (status, raw) = server.get("/einstellungen/daten").await;
    assert_eq!(status, StatusCode::OK, "Einstellungen → Daten lädt: {raw}");
    let html = unescape(&raw);
    assert!(
        html.contains(&format!(
            "<dt>Version</dt><dd>Baruch {} · <a href=\"/ueber\" data-about-link>Über Baruch</a>",
            env!("CARGO_PKG_VERSION")
        )),
        "neben der Version steht der Link auf /ueber (gewöhnlicher Link, ohne JavaScript): {html}"
    );
    // Auch das Fragment des Einstellungs-Dialogs (mit JavaScript) trägt den Link.
    let page = server.rpc("settings.get", json!({ "page": "daten" })).await;
    assert!(page["error"].is_null(), "settings.get daten: {page}");
    assert_eq!(page["result"]["version"], about::VERSION, "RPC nennt dieselbe Version wie die Über-Seite: {page}");
}

#[tokio::test]
async fn the_about_page_triggers_no_network_access() {
    let server = common::open().await;

    // 1. Lauschender Nachbau: Beide Verbindungen, über die Baruch sonst ins Netz ginge, zeigen auf ihn.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(AtomicUsize::new(0));
    let counter = seen.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            drop(socket);
        }
    });
    for kind in ["eliezer", "multigpt"] {
        let set = server.rpc("connection.set", json!({ "kind": kind, "config": { "url": url } })).await;
        assert!(set["error"].is_null(), "Verbindung {kind} zeigt auf den Nachbau: {set}");
    }
    // Kurz warten: Was das Speichern der Verbindungen selbst auslösen könnte, zählt nicht zur Über-Seite.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let before = seen.load(Ordering::SeqCst);
    for _ in 0..3 {
        let (status, _) = server.get("/ueber").await;
        assert_eq!(status, StatusCode::OK, "/ueber lädt");
        let (status, _) = server.get("/ueber/lizenz").await;
        assert_eq!(status, StatusCode::OK, "/ueber/lizenz lädt");
    }
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert_eq!(
        seen.load(Ordering::SeqCst),
        before,
        "der Abruf der Über-Seite und des Lizenztexts öffnet keine Verbindung zu den eingestellten Diensten"
    );
    // Gegenprobe: Der Nachbau bemerkt eine Verbindung wirklich (sonst bewiese die Null oben nichts).
    drop(tokio::net::TcpStream::connect(url.trim_start_matches("http://")).await.unwrap());
    for _ in 0..100 {
        if seen.load(Ordering::SeqCst) > before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(seen.load(Ordering::SeqCst), before + 1, "Gegenprobe: eine echte Verbindung zählt der Nachbau");

    // 2. Das HTML: Nichts, was ein Browser von selbst lädt oder abschickt, zeigt auf einen fremden Rechner.
    let (_, raw) = server.get("/ueber").await;
    let html = unescape(&raw);
    for name in ["src", "action", "srcset", "poster", "data", "formaction", "ping"] {
        for value in attributes(&html, name) {
            assert!(
                !value.contains("://") && !value.starts_with("//"),
                "{name}=\"{value}\" lädt bzw. sendet an einen fremden Rechner – die Seite darf nur Eigenes laden"
            );
        }
    }
    for tag in html.split("<link ").skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        assert!(!tag.contains("://"), "<link {tag}> lädt von einem fremden Rechner");
    }
    assert!(
        !html.contains("http-equiv=\"refresh\"") && !html.contains("@import") && !html.contains("url(http"),
        "keine Weiterleitung und kein fremdes Stylesheet im HTML"
    );
    // Fremde Adressen stehen nur in gewöhnlichen Links – und zwar genau die vier bekannten. Runde 37: dazu die Hilfe
    // auf der Webseite (Eintrag „Hilfe“ im Menü „Baruch ▾“ und die Zeile unter „Webseite“).
    let mut external: Vec<&str> = attributes(&html, "href").into_iter().filter(|h| h.contains("://")).collect();
    external.sort_unstable();
    external.dedup();
    let release = about::release_url();
    assert_eq!(
        external,
        vec![about::HOMEPAGE, about::HELP_URL, about::REPOSITORY, release.as_str()],
        "fremde Adressen nur als Links, die erst ein Klick des Menschen öffnet: Webseite, Hilfe, Repo, Release-Seite"
    );
    for part in html.split("<a ").skip(1) {
        let tag = part.split('>').next().unwrap_or_default();
        if tag.contains("://") {
            assert!(
                tag.contains("rel=\"noopener noreferrer\"") && !tag.contains("download") && !tag.contains("ping="),
                "fremder Link ohne Referrer, ohne Download und ohne ping: <a {tag}>"
            );
        }
    }

    // 3. Der Quelltext des Moduls: kein HTTP-Client, keine Steckdose, kein Start fremder Programme. So bleibt die
    //    Zusage „keine Aktualisierungsprüfung“ auch bei späteren Änderungen an dieser Stelle sichtbar.
    let source = include_str!("../src/about.rs");
    let code = source.split("#[cfg(test)]").next().unwrap();
    for forbidden in ["reqwest", "TcpStream", "UdpSocket", "hyper::", "Command::new", "baruch_llm", "connections::"] {
        assert!(!code.contains(forbidden), "src/about.rs benutzt {forbidden} – die Über-Seite darf nicht ins Netz");
    }
}
