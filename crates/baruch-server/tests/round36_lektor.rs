// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 „Reste“, Besitz reste: Reste aus Runde 35 (Konten, Rollen, Lektor-Oberfläche).
//!
//! Bühne und Helfer aus `round35_rollen` (zwei frei erfundene Werke: das Beispielwerk der Instanz mit der kleinsten
//! id – für den Lektor fremd – und ein freigegebenes; Konten per SQL, Identität über `common::with_who`). Kein
//! Modell, kein Netz.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::auth;
use baruch_server::roles::{LEKTOR_ROUTES, LEKTOR_RPC, WorkFrom};
use baruch_store::StoreError;
use serde_json::{Value, json};

use crate::common;
use crate::round35_rollen::{FORBIDDEN, Stage, code, denied_by_role, get_as, rpc_as, send_as, stage};

/// Die fünf Methoden, deren Werk-Bindung Runde 36 lockert.
const LOOSENED: [&str; 5] = ["wysiwyg.parse", "wysiwyg.serialize", "wysiwyg.diff", "wysiwyg.style", "ribbon.formats"];

fn get_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::get(uri).header(header::COOKIE, cookie).body(Body::empty()).unwrap()
}

/// Nennt der Text das fremde Werk – über seinen Titel oder über eine Adresse mit seiner id?
fn names_foreign(s: &Stage, text: &str) -> bool {
    text.contains(&s.foreign_title)
        || text.contains(&format!("work={}\"", s.foreign))
        || text.contains(&format!("work={}&", s.foreign))
        || text.contains(&format!("/werke/{}/", s.foreign))
}

/// Grundlage der Lockerung (Vertrag §5a): Über `AppState::default_work` erreicht ein Lektor nie ein nicht
/// freigegebenes Werk – auch nicht, wenn ihm gar keines freigegeben ist.
#[tokio::test]
async fn the_default_work_of_a_lektor_is_never_a_work_that_is_not_granted() {
    let s = stage().await;
    let state = &s.server.state;
    assert_eq!(
        auth::scope(s.author.clone(), state.default_work()).await.unwrap(),
        s.foreign,
        "Vorbedingung: Das Vorgabewerk der Instanz (kleinste id) ist das für den Lektor fremde Werk"
    );
    assert_eq!(
        auth::scope(s.erika.clone(), state.default_work()).await.unwrap(),
        s.granted,
        "für die Lektorin ist das Vorgabewerk das kleinste freigegebene"
    );
    assert!(
        matches!(auth::scope(s.gustav.clone(), state.default_work()).await, Err(StoreError::UnknownWork(0))),
        "ohne Freigabe gibt es kein Vorgabewerk (UnknownWork(0)), nie das der Instanz"
    );
    // Entzug wirkt bei der nächsten Anfrage: nichts wird gemerkt.
    state.store.member_revoke(s.granted, s.erika_id).await.expect("Freigabe entziehen");
    assert!(
        matches!(auth::scope(s.erika.clone(), state.default_work()).await, Err(StoreError::UnknownWork(0))),
        "nach dem Entzug hat auch Erika kein Vorgabewerk mehr"
    );
}

#[tokio::test]
async fn the_tables_carry_the_loosened_entries_without_a_work() {
    let route = LEKTOR_ROUTES.iter().find(|(verb, path, _)| *verb == "GET" && *path == "/einstellungen/darstellung");
    assert_eq!(
        route.map(|(_, _, from)| *from),
        Some(WorkFrom::None),
        "die Seite Darstellung braucht kein Werk mehr (Cookies im eigenen Browser)"
    );
    for method in LOOSENED {
        let entry = LEKTOR_RPC.iter().find(|(name, _)| *name == method);
        assert_eq!(
            entry.map(|(_, from)| *from),
            Some(WorkFrom::None),
            "{method} steht ohne Werk-Bindung in LEKTOR_RPC"
        );
    }
    let unbound: Vec<&str> =
        LEKTOR_RPC.iter().filter(|(_, from)| *from == WorkFrom::None).map(|(name, _)| *name).collect();
    assert_eq!(
        unbound,
        [
            "work.list",
            "account.me",
            "account.set_password",
            "account.logout_all",
            "wysiwyg.parse",
            "wysiwyg.serialize",
            "wysiwyg.diff",
            "wysiwyg.style",
            "ribbon.formats"
        ],
        "sonst ist nichts gelockert: Jede andere Methode bleibt an ein freigegebenes Werk gebunden"
    );
}

/// Die Seite Darstellung ohne `work`: erreichbar für jeden Lektor, nie mit fremdem Inhalt – auch nicht über ein
/// altes Cookie `baruch_work`, das auf ein fremdes Werk zeigt (Falle der Sichtprüfung R35).
#[tokio::test]
async fn the_display_settings_open_without_a_work_and_never_show_a_foreign_one() {
    let s = stage().await;
    let server = &s.server;
    let stale = format!("baruch_work={}", s.foreign);
    for (who, name) in [(&s.erika, "Erika (mit Freigabe)"), (&s.gustav, "Gustav (ohne Freigabe)")] {
        for cookie in [None, Some(stale.as_str())] {
            let request = match cookie {
                Some(cookie) => get_with_cookie("/einstellungen/darstellung", cookie),
                None => Request::get("/einstellungen/darstellung").body(Body::empty()).unwrap(),
            };
            let (status, headers, page) = send_as(server, who, request).await;
            assert_eq!(status, StatusCode::OK, "{name}, Cookie {cookie:?}: die Seite ist ohne work erreichbar");
            assert!(!denied_by_role(&headers), "{name}: keine Ablehnung der Rolle");
            assert!(
                page.contains("action=\"/einstellungen/darstellung/act\""),
                "{name}: das Formular der Darstellung ist da"
            );
            assert!(
                !names_foreign(&s, &page),
                "{name}, Cookie {cookie:?}: weder Titel noch Adresse des fremden Werks stehen auf der Seite"
            );
        }
    }
    let (_, _, page) = get_as(server, &s.erika, "/einstellungen/darstellung").await;
    assert!(
        page.contains(&format!("work={}", s.granted)),
        "für Erika hängt die Seite am freigegebenen Werk (Rückweg zum Werk)"
    );
    let (_, _, page) = get_as(server, &s.gustav, "/einstellungen/darstellung").await;
    assert!(!page.contains("work="), "ohne Freigabe nennt die Seite gar kein Werk");

    // Ein genanntes Werk wird weiter geprüft – die Lockerung gilt nur für das fehlende.
    for who in [&s.erika, &s.gustav] {
        let (status, headers, body) =
            get_as(server, who, &format!("/einstellungen/darstellung?work={}", s.foreign)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "ein fremdes work in der Adresse bleibt abgelehnt");
        assert!(denied_by_role(&headers), "und zwar von der Rollenprüfung");
        assert!(!body.contains(&s.foreign_title), "die Ablehnung nennt das fremde Werk nicht");
    }
    let (status, _, _) = get_as(server, &s.erika, &format!("/einstellungen/darstellung?work={}", s.granted)).await;
    assert_eq!(status, StatusCode::OK, "mit dem freigegebenen Werk wie bisher");

    // Gegenprobe: Für den Autor zählt das Cookie weiter.
    let (status, _, page) = send_as(server, &s.author, get_with_cookie("/einstellungen/darstellung", &stale)).await;
    assert_eq!(status, StatusCode::OK, "der Autor erreicht die Seite");
    assert!(page.contains(&format!("work={}", s.foreign)), "beim Autor gilt das zuletzt geöffnete Werk des Cookies");
}

/// `wysiwyg.*` und `ribbon.formats` ohne `work`: für die Lektorin das freigegebene Werk, ohne Freigabe ein Fehler –
/// nie das Vorgabewerk der Instanz.
#[tokio::test]
async fn the_loosened_methods_without_a_work_use_the_granted_work_only() {
    let s = stage().await;
    let server = &s.server;
    // Ein Bild nur im freigegebenen Werk: Daran zeigt das Satzbild, in welchem Werk es nachsieht.
    let picture = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"30\" height=\"20\"><rect width=\"30\" height=\"20\"/></svg>\n";
    let added = rpc_as(
        server,
        &s.author,
        "asset.add",
        json!({ "work": s.granted, "file_name": "hafen.svg", "content": picture, "encoding": "svg",
                "path": "/bilder/hafen.svg", "caption": "Hafen", "alt": "Ein Hafen" }),
    )
    .await;
    assert!(added.get("error").is_none(), "Bild angelegt: {added}");
    // Das Satzbild findet ein Bild über seinen Pfad im Werk (nicht über den Namen).
    let image = "#image(\"/bilder/hafen.svg\")";
    let body = format!("Ein Absatz.\n\n{image}\n");
    let body = body.as_str();
    let params = |method: &str, work: Option<i64>, key: i64| -> Value {
        let mut p = match method {
            "wysiwyg.parse" => json!({ "body": body }),
            "wysiwyg.serialize" => json!({ "body": body, "edits": [] }),
            "wysiwyg.diff" => json!({ "key": key, "rev": 1 }),
            _ => json!({}),
        };
        if let Some(work) = work {
            p["work"] = json!(work);
        }
        p
    };

    // Vorbedingung, damit die Probe etwas unterscheidet: Der Autor bekommt ohne work das fremde Werk.
    let author = rpc_as(server, &s.author, "wysiwyg.parse", params("wysiwyg.parse", None, 0)).await;
    let author_html = author["result"]["html"].as_str().unwrap();
    assert!(
        author_html.contains("Bild fehlt") && !author_html.contains(&format!("/werke/{}/bilder/", s.granted)),
        "Vorbedingung: Ohne work ist das Werk des Autors das Vorgabewerk der Instanz – dort fehlt das Bild: {author}"
    );

    for method in LOOSENED {
        let without = rpc_as(server, &s.erika, method, params(method, None, s.granted_key)).await;
        let with = rpc_as(server, &s.erika, method, params(method, Some(s.granted), s.granted_key)).await;
        assert!(without.get("error").is_none(), "{method} ohne work gelingt der Lektorin: {without}");
        assert_eq!(
            without["result"], with["result"],
            "{method} ohne work antwortet genau wie mit dem freigegebenen Werk"
        );
        assert!(!names_foreign(&s, &without.to_string()), "{method}: nichts aus dem fremden Werk in der Antwort");

        let foreign = rpc_as(server, &s.erika, method, params(method, Some(s.foreign), s.foreign_key)).await;
        assert_eq!(code(&foreign), Some(FORBIDDEN), "{method} mit dem fremden Werk bleibt verboten: {foreign}");

        let nobody = rpc_as(server, &s.gustav, method, params(method, None, s.granted_key)).await;
        assert!(nobody.get("result").is_none(), "{method} ohne Freigabe liefert kein Ergebnis: {nobody}");
        assert!(code(&nobody).is_some(), "{method} ohne Freigabe ist ein Fehler");
        assert!(
            !names_foreign(&s, &nobody.to_string()),
            "{method}: Die Fehlermeldung nennt das fremde Werk nicht: {nobody}"
        );
        let nobody = rpc_as(server, &s.gustav, method, params(method, Some(s.foreign), s.foreign_key)).await;
        assert_eq!(code(&nobody), Some(FORBIDDEN), "{method}: ohne Freigabe ist auch ein genanntes Werk verboten");
    }

    let parsed = rpc_as(server, &s.erika, "wysiwyg.parse", params("wysiwyg.parse", None, 0)).await;
    assert!(
        parsed["result"]["html"].as_str().unwrap().contains(&format!("/werke/{}/bilder/", s.granted)),
        "das Bild im Satzbild zeigt auf das freigegebene Werk: {parsed}"
    );
    // Ein fester Schlüssel des fremden Werks führt ohne work nicht hinüber: Gesucht wird nur im freigegebenen.
    let diff = rpc_as(server, &s.erika, "wysiwyg.diff", json!({ "key": s.foreign_key, "rev": 1 })).await;
    let reached_foreign = diff.get("result").is_some() && s.foreign_key != s.granted_key;
    assert!(!reached_foreign, "ein Schlüssel des fremden Werks wird ohne work nicht im fremden Werk gesucht: {diff}");
}

// ---- Anzeigename zur Herkunft @lektorat:<slug> --------------------------------------------------------------

/// Text der Elemente mit der Klasse `class` (roh, bis zum nächsten schließenden Tag desselben Namens).
fn texts_of(page: &str, class: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = page;
    while let Some(at) = rest.find(&format!("{class}\"")).or_else(|| rest.find(&format!("{class} "))) {
        let Some(open_end) = rest[at..].find('>') else { break };
        let after = &rest[at + open_end + 1..];
        let end = after.find("</div>").or_else(|| after.find("</span>")).unwrap_or(after.len());
        found.push(after[..end].to_owned());
        rest = &after[end..];
    }
    found
}

/// Fassungen, Beiträge und Befunde eines Lektors zeigten auf der Arbeitsseite nur den Handle. Jetzt steht der
/// Anzeigename des Kontos davor – nur als Anzeige: Attribute, Buchtext und Beitragstext bleiben wörtlich.
#[tokio::test]
async fn the_workspace_shows_the_display_name_next_to_the_lektor_handle() {
    let s = stage().await;
    let server = &s.server;
    let (g, key) = (s.granted, s.granted_key);
    let shown = "Erika Beispiel (@lektorat:erika)";
    let text = "= Nebelhorn\n\nDas Horn rief viermal über den Sund, und die Möwen stoben vom Geländer.\n";
    let proposed = rpc_as(server, &s.erika, "revision.propose", json!({ "work": g, "key": key, "body": text })).await;
    let rev = proposed["result"]["rev"].as_u64().unwrap_or_else(|| panic!("Vorschlag der Lektorin: {proposed}"));
    let mention = "Bitte @lektorat:erika fragen, sagt @lektorat:erika.";
    let post = rpc_as(server, &s.erika, "post.add", json!({ "work": g, "key": key, "body": mention })).await;
    assert!(post.get("error").is_none(), "Beitrag der Lektorin: {post}");
    let finding = json!({ "work": g, "key": key, "kind": "hint", "body": "Dreimal oder viermal?" });
    let finding = rpc_as(server, &s.erika, "finding.add", finding).await;
    assert!(finding.get("error").is_none(), "Befund der Lektorin: {finding}");

    for (who, name) in [(&s.author, "Autor"), (&s.erika, "Lektorin")] {
        let (status, _, page) = get_as(server, who, &format!("/?work={g}&key={key}&rev={rev}")).await;
        assert_eq!(status, StatusCode::OK, "{name}: die Arbeitsseite mit dem Vorschlag lädt");
        let meta = texts_of(&page, "rev-meta").join(" | ");
        assert!(meta.contains(shown), "{name}: der Kopf der Fassung nennt Anzeigename und Handle: {meta}");
        let authors = texts_of(&page, "post-author").join(" | ");
        assert!(authors.contains(shown), "{name}: der Beitrag nennt Anzeigename und Handle: {authors}");
        assert!(
            page.contains("data-post-author=\"@lektorat:erika\""),
            "{name}: das Attribut des Beitrags bleibt der reine Handle (Widgets und Rechte vergleichen ihn)"
        );
        assert!(
            page.contains("Bitte @lektorat:erika fragen, sagt @lektorat:erika."),
            "{name}: ein Handle im Beitragstext bleibt wörtlich – ersetzt wird nur die Herkunft"
        );
        assert!(!page.contains("Erika Beispiel (Erika Beispiel"), "{name}: der Name steht nicht doppelt da");
    }
    // Befunde: Karte auf der Befundseite des Werks.
    let (status, _, findings) = get_as(server, &s.author, &format!("/werke/{g}/befunde?key={key}")).await;
    assert_eq!(status, StatusCode::OK, "die Befundseite lädt");
    if findings.contains("fd-persona") {
        let personas = texts_of(&findings, "fd-persona").join(" | ");
        assert!(personas.contains(shown), "der Befund nennt Anzeigename und Handle: {personas}");
    }
    // Der Rahmen nach einem Wechsel ohne Neuladen (ui.render) trägt denselben Namen.
    let rendered = rpc_as(server, &s.author, "ui.render", json!({ "work": g, "key": key, "rev": rev })).await;
    let editor = rendered["result"]["editor"].as_str().expect("ui.render liefert den Editor");
    assert!(editor.contains(shown), "auch das über ui.render nachgeladene Stück nennt den Anzeigenamen");

    // Ein Konto ohne Anzeigenamen und ein unbekannter Handle bleiben, wie sie sind.
    sqlx::query("UPDATE account SET display = '' WHERE id = ?")
        .bind(s.erika_id)
        .execute(server.state.store.pool())
        .await
        .unwrap();
    let (_, _, page) = get_as(server, &s.author, &format!("/?work={g}&key={key}&rev={rev}")).await;
    assert!(
        !page.contains("(@lektorat:erika)") && texts_of(&page, "rev-meta").join(" ").contains("@lektorat:erika"),
        "ohne Anzeigenamen steht nur der Handle da, ohne leere Klammern"
    );
}

/// Ohne Konten bleibt die Seite, wie sie war: Ein Handle `@lektorat:…` (etwa aus einem Austausch mit einer anderen
/// Instanz) wird nicht umgeschrieben.
#[tokio::test]
async fn without_accounts_a_lektor_handle_is_shown_unchanged() {
    let server = common::open().await;
    let work = server
        .import("#set document(title: \"Sund\")\n\n= Nebelhorn\n\nDas Horn rief dreimal.\n", "sund.typ")
        .await
        .work;
    let store = &server.state.store;
    let rev = store
        .add_proposal(work, 0, "= Nebelhorn\n\nDas Horn rief viermal.\n", "@lektorat:erika", "extern", "von anderswo")
        .await
        .unwrap();
    let key = store.load_work(work).await.unwrap().nodes[0].key;
    let (status, page) = server.get(&format!("/?work={work}&key={key}&rev={rev}")).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite lädt");
    assert!(page.contains("@lektorat:erika"), "Vorbedingung: der Handle steht auf der Seite");
    assert!(!page.contains("(@lektorat:erika)"), "ohne Konten kein Anzeigename und keine Klammer");
    assert_eq!(
        baruch_server::konten_ui::names::show(&page, &std::collections::HashMap::new()),
        page,
        "ohne Namen gibt die Ersetzung die Seite Byte für Byte zurück"
    );
    assert!(
        baruch_server::konten_ui::names::display_names(&server.state).await.is_empty(),
        "ohne Konten gibt es keine Namen (die Konten werden gar nicht erst gelesen)"
    );
}

// ---- „Im Werk suchen“, gesperrte Links ----------------------------------------------------------------------

#[tokio::test]
async fn the_search_button_is_named_for_what_a_lektor_can_do() {
    let s = stage().await;
    let server = &s.server;
    let uri = format!("/?work={}", s.granted);
    let (_, _, lektor) = get_as(server, &s.erika, &uri).await;
    assert!(
        lektor.contains(">Im Werk suchen (Strg+Umschalt+F)</a>"),
        "für die Lektorin heißt der Knopf „Im Werk suchen“ – ersetzen kann sie nicht"
    );
    let own = part_between(&lektor, "<a class=\"btn nur-lektor\" href=\"/suche?work=", "</a>");
    assert!(own.is_some(), "ihr Knopf ist ein eigener Link auf die Suche (Klasse nur-lektor)");
    assert!(
        lektor.contains("class=\"btn nur-autor\" href=\"/suche?work="),
        "der Knopf des Autors trägt nur-autor (konten.css blendet ihn für Lektoren aus, ohne JavaScript)"
    );
    let (_, _, author) = get_as(server, &s.author, &uri).await;
    assert!(
        author.contains(">Im Werk suchen und ersetzen (Strg+Umschalt+F)</a>"),
        "für den Autor heißt der Knopf weiter „Im Werk suchen und ersetzen“"
    );
    assert!(!author.contains(">Im Werk suchen (Strg+Umschalt+F)</a>"), "der Knopf der Lektoren fehlt beim Autor ganz");

    // Ohne Konten: nur der eine Knopf, ohne Kennzeichen (wie vor Runde 35).
    let open = common::open().await;
    let (_, page) = open.get("/").await;
    assert!(
        page.contains("<a class=\"btn\" href=\"/suche?work=") && !page.contains(">Im Werk suchen (Strg"),
        "ohne Konten steht der Knopf da wie bisher: ein Link, Klasse btn, „suchen und ersetzen“"
    );
}

fn part_between<'a>(page: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = page.find(start)?;
    let to = page[from..].find(end)?;
    Some(&page[from..from + to])
}

const COVER: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"60\" height=\"90\">\
<rect width=\"60\" height=\"90\" fill=\"#35506b\"/></svg>\n";

/// Links auf gesperrte Seiten: Das Umschlagbild in der Werkliste führte Lektoren auf `/werke/{id}/umschlag` (403).
#[tokio::test]
async fn the_cover_in_the_work_list_is_no_link_to_a_locked_page_for_a_lektor() {
    let s = stage().await;
    let server = &s.server;
    let added = rpc_as(
        server,
        &s.author,
        "asset.add",
        json!({ "work": s.granted, "file_name": "umschlag.svg", "content": COVER, "encoding": "svg",
                "caption": "Leuchtturm im Nebel", "alt": "Ein Leuchtturm" }),
    )
    .await;
    let name = added["result"]["name"].as_str().unwrap_or_else(|| panic!("Bild angelegt: {added}")).to_owned();
    let set = rpc_as(server, &s.author, "work.set_cover", json!({ "work": s.granted, "name": name })).await;
    assert!(set.get("error").is_none(), "Umschlag gesetzt: {set}");
    let link = format!("<a href=\"/werke/{}/umschlag\">", s.granted);
    let image = format!("src=\"/werke/{}/bilder/{name}/vorschau?b=320\"", s.granted);
    // Die Zelle des Umschlags in der Zeile des freigegebenen Werks.
    let cell = |page: &str| -> String {
        let at = page.find(&image).expect("das Umschlagbild steht in der Werkliste");
        let from = page[..at].rfind("<td class=\"works-cover\">").expect("Zelle des Umschlags");
        let to = at + page[at..].find("</td>").unwrap();
        page[from..to].to_owned()
    };

    let (_, _, author) = get_as(server, &s.author, "/werke").await;
    assert!(cell(&author).contains(&link), "Vorbedingung: beim Autor ist das Bild ein Link auf den Umschlag");
    let (status, _, lektor) = get_as(server, &s.erika, "/werke").await;
    assert_eq!(status, StatusCode::OK, "die Werkliste der Lektorin lädt");
    let shown = cell(&lektor);
    assert!(shown.contains("<img "), "die Lektorin sieht das kleine Umschlagbild weiter (die Vorschau ist erlaubt)");
    assert!(
        !shown.contains("<a ") && !shown.contains(&format!("/werke/{}/umschlag", s.granted)),
        "aber ohne Link auf die Seite des Umschlags, die für sie gesperrt ist: {shown}"
    );
    // Die Sperre selbst bleibt in roles – das Ausblenden ist nur Bequemlichkeit.
    let (status, headers, _) = get_as(server, &s.erika, &format!("/werke/{}/umschlag", s.granted)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "die Seite des Umschlags bleibt für Lektoren gesperrt");
    assert!(denied_by_role(&headers), "abgelehnt von der Rollenprüfung");
    let (status, _, _) = get_as(server, &s.erika, &format!("/werke/{}/bilder/{name}/vorschau?b=320", s.granted)).await;
    assert_eq!(status, StatusCode::OK, "die Vorschau des Bilds ist erreichbar – kein toter Verweis in der Liste");
}

// ---- Anmelden und Einrichten nach dem Cookie baruch_theme ---------------------------------------------------

/// Das öffnende `<html …>` der Seite.
fn html_tag(page: &str) -> &str {
    let start = page.find("<html").expect("Seite mit <html>");
    &page[start..start + page[start..].find('>').unwrap() + 1]
}

#[tokio::test]
async fn the_login_and_setup_pages_follow_the_theme_cookie_without_javascript() {
    // Einrichten: noch ohne Konten.
    let open = common::open().await;
    let (status, plain) = open.get("/einrichten").await;
    assert!(plain.contains("konto-seite"), "Vorbedingung: die Seite Einrichten wird gezeigt (Status {status})");
    assert_eq!(html_tag(&plain), "<html lang=\"de\">", "ohne Cookie wie bisher: das Farbschema des Systems");
    for (cookie, tag) in [
        ("baruch_theme=dark", "<html lang=\"de\" data-theme=\"dark\">"),
        ("baruch_work=1; baruch_theme=light", "<html lang=\"de\" data-theme=\"light\">"),
        ("baruch_theme=\"><script>", "<html lang=\"de\">"),
        ("baruch_theme=blau", "<html lang=\"de\">"),
    ] {
        let (_, _, page) = open.send(get_with_cookie("/einrichten", cookie)).await;
        assert_eq!(html_tag(&page), tag, "Einrichten mit Cookie „{cookie}“");
        assert!(!page.contains("<script"), "die Seite bleibt ohne JavaScript");
        if tag == "<html lang=\"de\">" {
            assert_eq!(page, plain, "ein ungültiger Wert ändert die Seite nicht (Byte für Byte wie ohne Cookie)");
        } else {
            assert_eq!(page.replacen(tag, "<html lang=\"de\">", 1), plain, "nur das Attribut kommt hinzu");
        }
    }

    // Anmelden: mit Konten, ohne Sitzung.
    let s = stage().await;
    let server = &s.server;
    let (status, plain) = server.get("/anmelden").await;
    assert_eq!(status, StatusCode::OK, "die Anmeldeseite lädt");
    assert_eq!(html_tag(&plain), "<html lang=\"de\">", "ohne Cookie wie bisher");
    let (_, _, dark) = server.send(get_with_cookie("/anmelden", "baruch_theme=dark")).await;
    assert_eq!(html_tag(&dark), "<html lang=\"de\" data-theme=\"dark\">", "dunkel nach dem Cookie");
    let (_, _, light) = server.send(get_with_cookie("/anmelden", "baruch_theme=light")).await;
    assert_eq!(html_tag(&light), "<html lang=\"de\" data-theme=\"light\">", "hell nach dem Cookie");
    assert!(dark.contains("name=\"password\"") && !dark.contains("<script"), "das Formular bleibt, ohne JavaScript");
    // Auch die Antwort auf eine gescheiterte Anmeldung (POST) folgt dem Cookie.
    let request = Request::post("/anmelden")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, "baruch_theme=dark")
        .body(Body::from("name=Erika&password=falsch-geraten-1&next=/"))
        .unwrap();
    let (status, _, failed) = server.send(request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "falsches Passwort");
    assert_eq!(html_tag(&failed), "<html lang=\"de\" data-theme=\"dark\">", "auch die Fehlerseite ist dunkel");
    // Der Stil hängt an genau diesem Attribut (hell und dunkel in app.css).
    let css = include_str!("../static/app.css");
    assert!(
        css.contains(":root[data-theme=\"dark\"]") && css.contains(":root:not([data-theme=\"light\"])"),
        "app.css schaltet über data-theme an <html> – dasselbe Attribut wie auf den Seiten mit Menüband"
    );
}
