// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23a: lesende Anfragen fremder Seiten (`guard::own_reads`), feste Meldecodes statt Text aus der Adresse,
//! Abzeichen und „Anträge zur Gliederung“ ohne JavaScript auf allen Seiten, Kasten „Bilder des Werks“ auf der
//! Arbeitsseite mit `?lq=`. Werke frei erfunden; das Modell ist ein Nachbau im Prozess (kein Netz).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::mock::ScriptedModel;
use baruch_server::assist::{BoxFuture, ModelAnswer, ModelCall, ModelFailure, ModelList, ModelPort};
use baruch_store::{FindingKind, IdeaOrigin, NewFinding, OutlineOp};
use serde_json::json;

use crate::common;

/// Nachbau des Modell-Backends, der zählt, wie oft die Modellliste abgefragt wird.
struct CountingModel {
    inner: ScriptedModel,
    lists: Arc<AtomicUsize>,
}

impl ModelPort for CountingModel {
    fn chat<'a>(
        &'a self,
        state: &'a baruch_server::AppState,
        call: ModelCall,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> BoxFuture<'a, Result<ModelAnswer, ModelFailure>> {
        self.inner.chat(state, call, deltas)
    }

    fn models<'a>(&'a self, state: &'a baruch_server::AppState) -> BoxFuture<'a, Result<ModelList, ModelFailure>> {
        self.lists.fetch_add(1, Ordering::SeqCst);
        self.inner.models(state)
    }

    fn alias(&self, name: &str) -> Option<String> {
        self.inner.alias(name)
    }
}

async fn with_counting_model() -> (common::Server, Arc<AtomicUsize>) {
    let lists = Arc::new(AtomicUsize::new(0));
    let model = CountingModel {
        inner: ScriptedModel::new().with_models(&["probe-klein", "probe-gross"]),
        lists: lists.clone(),
    };
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model),
    )
    .await
    .expect("Datenordner lässt sich öffnen");
    let state = Arc::new(state);
    (common::Server { app: baruch_server::router(state.clone()), state, data }, lists)
}

fn get(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut request = Request::get(uri);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    request.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn a_foreign_page_cannot_trigger_the_model_list_as_an_image() {
    let (server, lists) = with_counting_model().await;
    let uri = "/einstellungen/assistenten?modelle=abfragen";
    // <img src="http://127.0.0.1:8765/einstellungen/assistenten?modelle=abfragen"> auf einer fremden Seite.
    for (site, mode, dest) in [
        ("cross-site", "no-cors", "image"),
        ("same-site", "no-cors", "image"),
        ("cross-site", "cors", "empty"),
        ("cross-site", "navigate", "iframe"),
    ] {
        let request = get(uri, &[("sec-fetch-site", site), ("sec-fetch-mode", mode), ("sec-fetch-dest", dest)]);
        let (status, _, body) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{site}/{mode}/{dest}: als Unterressource abgelehnt: {body}");
        assert_eq!(
            body, "Abgelehnt: Anfrage von einer fremden Seite (lesend)",
            "mit der Meldung der Schicht own_reads"
        );
    }
    assert_eq!(lists.load(Ordering::SeqCst), 0, "die Gegenstelle (Nachbau) wurde dabei nie nach Modellen gefragt");

    // Der Mensch selbst: gleiche Herkunft, Adresse getippt, Link von außen (Navigation), kein Browser.
    for (headers, why) in [
        (
            vec![("sec-fetch-site", "same-origin"), ("sec-fetch-mode", "cors"), ("sec-fetch-dest", "empty")],
            "eigene Seite",
        ),
        (vec![("sec-fetch-site", "none"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "document")], "getippt"),
        (
            vec![("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "document")],
            "Link von einer fremden Seite",
        ),
        (vec![], "ohne Sec-Fetch-Site (curl, Tests, MCP-Clients)"),
    ] {
        let before = lists.load(Ordering::SeqCst);
        let (status, _, page) = server.send(get(uri, &headers)).await;
        assert_eq!(status, StatusCode::OK, "{why}: geht durch");
        assert!(page.contains("<option value=\"probe-klein\">"), "{why}: die Seite zeigt die abgefragte Liste");
        assert_eq!(lists.load(Ordering::SeqCst), before + 1, "{why}: genau eine Abfrage am Nachbau");
    }
}

#[tokio::test]
async fn other_reading_routes_refuse_foreign_subresources_but_the_epub_preview_frame_keeps_its_own_rule() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let image = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "no-cors"), ("sec-fetch-dest", "image")];
    for uri in [
        "/".to_owned(),
        format!("/werke/{work}/stichwortzettel"),
        format!("/gliederung?work={work}"),
        "/static/app.css".to_owned(),
        format!("/werke/{work}/export"),
    ] {
        let (status, _, _) = server.send(get(&uri, &image)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}: lesend von einer fremden Seite als Unterressource → 403");
        let (status, _, _) = server.send(get(&uri, &[])).await;
        assert_ne!(status, StatusCode::FORBIDDEN, "{uri}: ohne Browser-Kopfzeilen wie bisher erreichbar");
    }
    // Schreibende Routen prüft weiter same_origin (POST zählt nicht als „lesend“).
    let post = Request::post("/rpc")
        .header(header::CONTENT_TYPE, "application/json")
        .header("sec-fetch-site", "cross-site")
        .body(Body::from("{}"))
        .unwrap();
    let (status, _, body) = server.send(post).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "POST von einer fremden Seite bleibt abgelehnt");
    assert!(body.contains("Sec-Fetch-Site"), "…mit der Meldung von same_origin, nicht von own_reads: {body}");

    // Dateien der EPUB-Vorschau: Der Rahmen mit `sandbox` meldet seine Bilder als cross-site; dort entscheidet die
    // Route selbst über den Referer (Runde 20b) – own_reads lässt sie durch.
    let reader = format!("/werke/{work}/epub/leser/kindle/OEBPS/nichtda.xhtml");
    let referer = format!("http://127.0.0.1:8765/werke/{work}/epub/leser/kindle/OEBPS/text/k1.xhtml");
    let from_frame = get(
        &reader,
        &[
            ("sec-fetch-site", "cross-site"),
            ("sec-fetch-mode", "no-cors"),
            ("sec-fetch-dest", "image"),
            ("host", "127.0.0.1:8765"),
            ("referer", &referer),
        ],
    );
    let (status, _, body) = server.send(from_frame).await;
    assert_ne!(
        (status, body.as_str()),
        (StatusCode::FORBIDDEN, "Abgelehnt: Anfrage von einer fremden Seite (lesend)"),
        "eine Datei der Vorschau aus dem eigenen Rahmen scheitert nicht an own_reads"
    );
    let (status, _, body) = server.send(get(&reader, &image)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ohne eigenen Referer lehnt die Route der Vorschau selbst ab");
    assert!(body.contains("EPUB-Vorschau"), "…mit ihrer eigenen Meldung: {body}");
}

// ---- Meldecodes ----------------------------------------------------------------------------------------------------

async fn open_in_subdir() -> common::Server {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("daten");
    std::fs::create_dir(&data).unwrap();
    let state = Arc::new(baruch_server::AppState::open(&data).await.expect("Datenordner lässt sich öffnen"));
    common::Server { app: baruch_server::router(state.clone()), state, data: root }
}

#[tokio::test]
async fn the_backup_notice_never_shows_a_name_from_the_address() {
    let server = open_in_subdir().await;
    // Ein Link mit erfundenem Ziel, ohne dass es eine Sicherung gibt.
    let (status, page) = server.get("/einstellungen/daten?ok=sicherung&ziel=geheimer-ordner-aus-der-adresse").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    // Runde 33 (Menüband): Die Reiter und Schalter des Kopfs wiederholen die Abfrage der Seite in ihren Adressen
    // (href, maskiert) – das ist kein angezeigter Text. Geprüft wird darum alles außer den href-Werten.
    let shown: String = page.split("href=\"").enumerate().map(|(i, part)| if i == 0 { part } else { &part[part.find('"').unwrap_or(0)..] }).collect();
    assert!(page.contains("ziel=geheimer-ordner-aus-der-adresse&amp;tab="), "der Kopf behält die Abfrage in den Adressen der Reiter: {page}");
    assert!(
        !shown.contains("geheimer-ordner-aus-der-adresse"),
        "ein mitgeschicktes `ziel` wird nie angezeigt (sonst falsche Erfolgsmeldung über einen Link): {page}"
    );
    assert!(
        page.contains("Sicherung angelegt (neben dem Datenordner)."),
        "ohne Sicherung im Dateisystem bleibt die Meldung allgemein: {page}"
    );

    // Die echte Sicherung leitet mit festem Meldecode weiter, ohne `ziel`.
    let (status, location) = server.post_form("/einstellungen/daten/sicherung", "confirm=1").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Sicherung angelegt");
    assert_eq!(
        location.as_deref(),
        Some("/einstellungen/daten?ok=sicherung"),
        "Weiterleitung mit festem Code, kein Ordnername in der Adresse"
    );
    let made: Vec<String> = std::fs::read_dir(server.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("daten.sicherung-"))
        .collect();
    assert_eq!(made.len(), 1, "genau eine Sicherung neben dem Datenordner: {made:?}");
    let (_, page) = server.get("/einstellungen/daten?ok=sicherung&ziel=geheimer-ordner-aus-der-adresse").await;
    assert!(
        page.contains(&format!("Sicherung angelegt: <code>{}</code>", made[0])),
        "die Seite nennt den Ordner, den der Server selbst im Dateisystem findet: {page}"
    );
    // Runde 33: wie oben – geprüft wird alles außer den href-Werten (die Reiter wiederholen die Abfrage als Adresse).
    let shown: String = page.split("href=\"").enumerate().map(|(i, part)| if i == 0 { part } else { &part[part.find('"').unwrap_or(0)..] }).collect();
    assert!(!shown.contains("geheimer-ordner-aus-der-adresse"), "und weiter nie den aus der Adresse");
    assert_eq!(
        baruch_server::settings::newest_backup(&server.state.data).as_deref(),
        Some(made[0].as_str()),
        "newest_backup liefert genau diesen Ordner"
    );
}

// ---- Abzeichen ohne JavaScript -------------------------------------------------------------------------------------

/// Kopfzeile einer Seite (bis `</header>`).
fn topbar(page: &str) -> &str {
    let start = page.find("<header class=\"band\"").expect("Kopfzeile"); // Runde 33: der Kopf ist das Menüband (header.band statt header.topbar)
    &page[start..start + page[start..].find("</header>").expect("Ende der Kopfzeile")]
}

#[tokio::test]
async fn pages_outside_the_workspace_show_badges_and_the_requests_entry_without_javascript() {
    let server = common::open().await;
    let work = server
        .import("= Am Brunnen\n\nDas Seil ist nass.\n\n= Im Hof\n\nDie Katze schläft.\n", "brunnen-23.typ")
        .await
        .work;
    let keys: Vec<i64> = server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| n.key).collect();

    // Ohne offene Dinge: alles verborgen, kein Kennzeichen bleibt im HTML.
    let (_, empty) = server.get(&format!("/gliederung?work={work}")).await;
    let head = topbar(&empty);
    assert!(!head.contains("data-badge-fill"), "das Kennzeichen zum Füllen ist immer entfernt: {head}");
    assert!(
        head.contains("data-badge-item=\"outline_requests\" hidden>"),
        "ohne offene Anträge ist der Eintrag verborgen: {head}"
    );

    // Zwei offene Anträge zur Gliederung, ein offener Vorschlag, drei offene Befunde.
    let mcp = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
    for title in ["Am alten Brunnen", "Am tiefen Brunnen"] {
        let op = OutlineOp::Rename { key: keys[0], title: title.into() };
        server.state.store.outline_request(work, op, &mcp, "").await.unwrap();
    }
    server
        .state
        .store
        .add_proposal(work, 1, "= Im Hof\n\nDie Katze schläft in der Sonne.\n", "@lektor", "probe-1", "")
        .await
        .unwrap();
    for n in 0..3 {
        let body = format!("Hinweis {n}: Das Seil genauer beschreiben.");
        let post = baruch_core::Post {
            id: 0,
            created: String::new(),
            author: "@lektor".into(),
            body: body.clone(),
            sources: vec![],
            refs: vec![],
        };
        let post = server.state.store.add_post(work, 0, &post).await.unwrap();
        let new = NewFinding {
            work,
            key: Some(keys[0]),
            post: Some(post),
            thread_post: None,
            position: 0,
            persona: "@lektor",
            kind: FindingKind::Hint,
            body: &body,
            quote: "",
            quote_check: "",
            similar_to: None,
            summary_of: &[],
        };
        server.state.store.finding_insert(&new).await.unwrap();
    }

    // Seiten verschiedener Module, alle ohne JavaScript ausgeliefert (nur das HTML zählt).
    for uri in [
        format!("/gliederung?work={work}"),
        format!("/werke/{work}/bilder"),
        format!("/werke/{work}/befunde"),
        format!("/einstellungen/daten?work={work}"),
        format!("/werke/{work}/glossar"),
    ] {
        let (status, page) = server.get(&uri).await;
        assert_eq!(status, StatusCode::OK, "{uri} lädt");
        let head = topbar(&page);
        // Runde 33 (Menüband): Die Abzeichen standen an den Menüknöpfen Werk ▾ (Anträge, Befunde) und Ansicht ▾
        // (Vorschläge) und stehen jetzt an den Reitern Buch, Prüfen und Start – weiter vom Server gefüllt.
        assert!(
            head.contains(">Buch <span class=\"badge\" data-badge=\"outline_requests\" title=\"offene Anträge zur Gliederung\">2</span></a>"),
            "{uri}: Abzeichen der offenen Anträge am Reiter Buch im ausgelieferten HTML: {head}"
        );
        assert!(
            head.contains("<span class=\"badge\" data-badge=\"outline_requests\">2</span></a>"),
            "{uri}: dieselbe Zahl am Eintrag „Anträge zur Gliederung“: {head}"
        );
        assert!(
            head.contains(&format!(
                "href=\"/gliederung?work={work}#antraege\" data-badge-item=\"outline_requests\">Anträge zur Gliederung"
            )),
            "{uri}: der Eintrag „Anträge zur Gliederung“ steht sichtbar im Reiter Buch: {head}"
        );
        assert!(
            head.contains(">Start <span class=\"badge\" data-badge=\"open_proposals\" title=\"offene Vorschläge im Werk\">1</span></a>"),
            "{uri}: Abzeichen der offenen Vorschläge am Reiter Start: {head}"
        );
        assert!(
            head.contains(">Prüfen <span class=\"badge warning\" data-badge=\"open_findings\" title=\"offene Befunde im Werk\">3 offen</span></a>"),
            "{uri}: Abzeichen der offenen Befunde am Reiter Prüfen mit dem Wort „offen“: {head}"
        );
        assert!(!page.contains("data-badge-fill"), "{uri}: kein Kennzeichen bleibt im HTML");
    }
    // Die Zahlen sind dieselben wie über die RPC-Methode für das Widget.
    let badges = server.rpc("menu.badges", json!({ "work": work })).await;
    assert_eq!(
        (
            &badges["result"]["outline_requests"],
            &badges["result"]["open_proposals"],
            &badges["result"]["open_findings"]
        ),
        (&json!(2), &json!(1), &json!(3)),
        "menu.badges und das ausgelieferte HTML stimmen überein: {badges}"
    );
}

// ---- Kasten „Bilder des Werks“ -------------------------------------------------------------------------------------

#[tokio::test]
async fn the_workspace_with_a_search_shows_the_images_of_the_work_without_javascript() {
    let server = common::open().await;
    let work = server.import("= Am Teich\n\nDer Reiher steht still.\n", "teich-23.typ").await.work;
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"30\" height=\"20\"><rect width=\"30\" height=\"20\" fill=\"#557\"/></svg>\n";
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "graureiher.svg", "content": svg, "encoding": "svg",
                    "caption": "Der Graureiher am Schilf", "alt": "Vogel im Wasser" }),
        )
        .await;
    assert!(added["result"]["name"].is_string(), "Bild angelegt: {added}");

    /// Der Kasten „Bilder des Werks“ der Seite (ohne die Bildauswahl des Editors, die alle Bilder nennt).
    fn images_box(page: &str) -> Option<&str> {
        let start = page.find("data-library-images")?;
        Some(&page[start..start + page[start..].find("</section>")?])
    }
    let (status, page) = server.get(&format!("/?work={work}&lq=Graureiher")).await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite mit Suche lädt");
    let hit = images_box(&page).expect("der Kasten steht im ausgelieferten HTML (vorher erst im Popup)");
    assert!(
        hit.contains("Bilder des Werks") && hit.contains(&format!("/werke/{work}/bilder/graureiher.svg")),
        "der Kasten „Bilder des Werks“ nennt das gefundene Bild mit Link: {hit}"
    );
    assert!(hit.contains("Der Graureiher am Schilf"), "samt Bildunterschrift: {hit}");
    let (_, none) = server.get(&format!("/?work={work}&lq=Kranich")).await;
    assert!(
        images_box(&none).is_none_or(|b| !b.contains("graureiher.svg")),
        "eine Suche ohne passendes Bild nennt es im Kasten nicht"
    );
    let (_, plain) = server.get(&format!("/?work={work}")).await;
    assert!(images_box(&plain).is_none(), "ohne Suche gibt es den Kasten nicht");
}
