// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einstellungen (Runde 9, Aufgabe settings): Registrierung der sieben Seiten mit der Vertragssignatur,
//! Seiten `/einstellungen/<seite>` ohne JavaScript (genau ein Rahmen, Navigation, Speicherort,
//! `?fragment=1`), Darstellung über Cookies, `baruch_pv`, Werk mit den wiederverwendeten Formularen und
//! `back`, Assistenten mit `persona.update` und Lektor, MCP-Zugang mit gesehenen Clients, Daten mit
//! Sicherung und RPC `settings.*`.

use crate::common;

use std::collections::BTreeMap;
use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::pin::Pin;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::AppState;
use baruch_server::rpc::INVALID_PARAMS;
use baruch_server::settings::{PageRender, SettingsPage, registry};
use baruch_store::{Store, WorkId};
use serde_json::{Value, json};

const SLUGS: [&str; 7] = ["darstellung", "werk", "assistenten", "verbindungen", "quellen", "mcp", "daten"];

/// Anfrage mit Formular; Antwort Status, Kopfzeilen, Text.
async fn form(server: &common::Server, uri: &str, body: &str) -> (StatusCode, HeaderMap, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    server.send(request).await
}

fn location(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::LOCATION).and_then(|v| v.to_str().ok())
}

async fn get_with(server: &common::Server, uri: &str, extra: &[(&str, &str)]) -> (StatusCode, HeaderMap, String) {
    let mut request = Request::get(uri);
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    server.send(request.body(Body::empty()).unwrap()).await
}

fn error_code(answer: &Value) -> Option<i64> {
    answer["error"]["code"].as_i64()
}

/// Ausschnitt ab `start` bis einschließlich des ersten `end` dahinter.
fn cut<'a>(html: &'a str, start: &str, end: &str) -> &'a str {
    let at = html.find(start).unwrap_or_else(|| panic!("{start} fehlt: {html}"));
    let len = html[at..].find(end).unwrap_or_else(|| panic!("{end} fehlt hinter {start}")) + end.len();
    &html[at..at + len]
}

/// minijinja maskiert `/` in Werten als `&#x2f;`.
fn unescape(html: &str) -> String {
    html.replace("&#x2f;", "/").replace("&amp;", "&")
}

// ---- Registrierung ------------------------------------------------------------------------------

/// Eine Seite mit genau der Vertragssignatur, außerhalb des Moduls geschrieben (wie connections sie
/// liefern wird): Sie lässt sich als `render` eintragen.
fn outside_render<'a>(
    _state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(async move {
        Ok(format!(
            "<section class=\"settings-page\" data-settings-page-body=\"probe\">{work:?} {}</section>",
            query.len()
        ))
    })
}

#[tokio::test]
async fn the_registry_has_seven_pages_in_order_and_takes_a_render_function_from_outside() {
    let titles: Vec<(&str, &str, &str)> = registry().iter().map(|p| (p.slug, p.title, p.storage)).collect();
    assert_eq!(
        titles,
        [
            ("darstellung", "Darstellung", "Cookie in diesem Browser"),
            ("werk", "Werk", "SQLite, Git-Spiegel des Werks"),
            ("assistenten", "Assistenten", "SQLite (alle Werke)"),
            ("verbindungen", "Verbindungen", "SQLite, Geheimnisse im Schlüsselbund"),
            ("quellen", "Quellen", "SQLite (alle Werke)"),
            ("mcp", "MCP-Zugang", "SQLite (nur Hash der Tokens); Clients zur Laufzeit"),
            ("daten", "Daten", "Datenordner"),
        ],
        "Seiten, Titel und Speicherorte nach Vertrag §3, in fester Reihenfolge"
    );
    let render: PageRender = outside_render;
    let page = SettingsPage { slug: "probe", title: "Probe", storage: "nirgends", render };
    let server = common::open().await;
    let mut query = BTreeMap::new();
    query.insert("ok".to_owned(), "1".to_owned());
    let html = (page.render)(&server.state, Some(1), &query).await.unwrap();
    assert_eq!(
        html, "<section class=\"settings-page\" data-settings-page-body=\"probe\">Some(1) 1</section>",
        "eine fremde Funktion mit der Vertragssignatur lässt sich eintragen und aufrufen (so hängt der Integrator connections ein)"
    );
}

// ---- Seiten und Rahmen --------------------------------------------------------------------------

#[tokio::test]
async fn every_settings_page_has_exactly_one_frame_with_navigation_and_storage() {
    let server = common::open().await;
    let (status, headers, _) = get_with(&server, "/einstellungen", &[]).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "/einstellungen leitet weiter");
    assert_eq!(location(&headers), Some("/einstellungen/darstellung"), "auf die erste Seite");
    let (_, headers, _) = get_with(&server, "/einstellungen?work=1", &[]).await;
    assert_eq!(location(&headers), Some("/einstellungen/darstellung?work=1"), "mit dem Werk");

    for (page, slug) in registry().iter().zip(SLUGS) {
        let (status, html) = server.get(&format!("/einstellungen/{slug}?work=1")).await;
        assert_eq!(status, StatusCode::OK, "{slug} lädt: {html}");
        assert_eq!(html.matches("data-settings-frame").count(), 1, "{slug}: genau ein Rahmen: {html}");
        assert!(
            html.contains(&format!("<div class=\"settings-frame\" data-settings-frame data-page=\"{slug}\">")),
            "{slug}: Rahmen nennt die Seite"
        );
        assert!(html.contains("<nav class=\"settings-nav\" aria-label=\"Einstellungen\">"), "{slug}: Navigation");
        for other in SLUGS {
            let link = format!("<a href=\"/einstellungen/{other}?work=1\" data-settings-page=\"{other}\"");
            assert!(html.contains(&link), "{slug}: Link auf {other} mit Werk und data-settings-page");
        }
        assert_eq!(html.matches("aria-current=\"page\"").count(), 1, "{slug}: genau eine Seite ist die gezeigte");
        assert!(
            html.contains(&format!("data-settings-page=\"{slug}\" aria-current=\"page\">{}</a>", page.title)),
            "{slug}: die gezeigte Seite trägt aria-current"
        );
        assert!(
            html.contains(&format!("<p class=\"settings-storage\">Gespeichert: {}</p>", page.storage)),
            "{slug}: sagt, wo der Wert liegt"
        );
        assert!(html.starts_with("<!doctype html>") || html.contains("<!doctype html>"), "{slug}: ganze Seite");

        let (status, headers, fragment) =
            get_with(&server, &format!("/einstellungen/{slug}?work=1&fragment=1"), &[]).await;
        assert_eq!(status, StatusCode::OK, "{slug}: Fragment lädt");
        assert!(
            fragment.starts_with("<div class=\"settings-frame\" data-settings-frame")
                && fragment.trim_end().ends_with("</div>"),
            "{slug}: ?fragment=1 liefert nur den Rahmen: {fragment}"
        );
        assert!(!fragment.contains("<html") && !fragment.contains("<body"), "{slug}: ohne base.html");
        assert!(
            headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.starts_with("text/html")),
            "{slug}: Fragment ist text/html"
        );
    }
    assert_eq!(server.get("/einstellungen/gibtsnicht").await.0, StatusCode::NOT_FOUND, "unbekannte Seite → 404");
    assert_eq!(server.get("/einstellungen/werk?work=999").await.0, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
    let (status, headers, _) = get_with(&server, "/einstellungen/personas?work=1", &[]).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "alte Seite der Farben leitet weiter");
    assert_eq!(location(&headers), Some("/einstellungen/assistenten?work=1"), "auf Assistenten");
    for (slug, body) in [
        ("verbindungen", "data-settings-page-body=\"verbindungen\""),
        ("quellen", "data-settings-page-body=\"quellen\""),
    ] {
        let (_, html) = server.get(&format!("/einstellungen/{slug}")).await;
        assert!(
            html.contains(body) && !html.contains("Diese Seite folgt mit Aufgabe connections"),
            "{slug}: der Integrator hat die Seite aus connections eingehängt, kein Platzhalter mehr: {html}"
        );
    }
}

#[tokio::test]
async fn the_work_of_a_page_comes_from_query_then_cookie_then_default() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let (_, html) = server.get("/einstellungen/werk").await;
    assert!(html.contains("href=\"/einstellungen/werk?work=1\""), "ohne Angabe das Vorgabewerk (kleinste id)");
    let cookie = format!("baruch_work={}", garden.work);
    let (_, _, html) = get_with(&server, "/einstellungen/werk", &[("cookie", &cookie)]).await;
    assert!(
        html.contains(&format!("href=\"/einstellungen/werk?work={}\"", garden.work)),
        "das Werk aus dem Cookie baruch_work"
    );
    let (_, _, html) = get_with(&server, "/einstellungen/werk?work=1", &[("cookie", &cookie)]).await;
    assert!(html.contains("href=\"/einstellungen/werk?work=1\""), "die Abfrage geht vor dem Cookie");
    let (_, _, html) = get_with(&server, "/einstellungen/werk", &[("cookie", "baruch_work=999")]).await;
    assert!(html.contains("href=\"/einstellungen/werk?work=1\""), "ein Cookie mit unbekanntem Werk zählt nicht");
}

// ---- Darstellung ---------------------------------------------------------------------------------

#[tokio::test]
async fn the_display_form_sets_and_deletes_cookies_in_the_widget_format() {
    let server = common::open().await;
    let (status, headers, _) = form(
        &server,
        "/einstellungen/darstellung/act",
        "work=1&theme=dark&editor=quelltext&pv=closed&accordion=personas&reset_widths=1",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert → 303");
    assert_eq!(location(&headers), Some("/einstellungen/darstellung?ok=1&work=1"), "zurück zur Seite");
    let cookies: Vec<&str> = headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap()).collect();
    assert_eq!(
        cookies,
        [
            "baruch_theme=dark; Path=/; Max-Age=31536000; SameSite=Lax",
            "baruch_editor=quelltext; Path=/; Max-Age=31536000; SameSite=Lax",
            "baruch_pv=closed; Path=/; Max-Age=31536000; SameSite=Lax",
            "baruch_accordion=personas; Path=/; Max-Age=31536000; SameSite=Lax",
            "baruch_left=; Path=/; Max-Age=0; SameSite=Lax",
            "baruch_right=; Path=/; Max-Age=0; SameSite=Lax",
        ],
        "Format wie store.set in widgets.ts; Spaltenbreiten werden gelöscht"
    );
    let (_, headers, _) = form(&server, "/einstellungen/darstellung/act", "theme=system&pv=open&editor=satz").await;
    let cookies: Vec<&str> = headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap()).collect();
    assert_eq!(
        cookies,
        [
            "baruch_theme=; Path=/; Max-Age=0; SameSite=Lax",
            "baruch_editor=satz; Path=/; Max-Age=31536000; SameSite=Lax",
            "baruch_pv=; Path=/; Max-Age=0; SameSite=Lax",
        ],
        "System und „offen“ sind Vorgaben: deren Cookie wird gelöscht"
    );
    assert_eq!(location(&headers), Some("/einstellungen/darstellung?ok=1"), "ohne Werk kein &work=");

    let (status, headers, page) = form(&server, "/einstellungen/darstellung/act", "theme=lila&editor=quelltext").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültiges Thema → 422");
    assert!(headers.get(header::SET_COOKIE).is_none(), "bei einem ungültigen Feld wird gar nichts gesetzt");
    assert!(page.contains("Thema: „System“, „Hell“ oder „Dunkel“ wählen."), "Meldung auf der Seite: {page}");
    assert!(
        page.contains("data-settings-frame data-page=\"darstellung\""),
        "422 zeigt die Seite (Popup nimmt den Rahmen)"
    );

    let request = Request::post("/einstellungen/darstellung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::from("theme=dark"))
        .unwrap();
    let (status, headers, _) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft abgelehnt (guard::same_origin)");
    assert!(headers.get(header::SET_COOKIE).is_none(), "und kein Cookie");
}

#[tokio::test]
async fn the_display_page_shows_the_cookies_of_the_request() {
    let server = common::open().await;
    let cookie = "baruch_theme=dark; baruch_editor=quelltext; baruch_pv=closed; baruch_left=22%";
    let (_, _, html) = get_with(&server, "/einstellungen/darstellung", &[("cookie", cookie)]).await;
    for checked in [
        "name=\"theme\" value=\"dark\" checked",
        "name=\"editor\" value=\"quelltext\" checked",
        "name=\"pv\" value=\"closed\" checked",
        "name=\"accordion\" value=\"chat\" checked",
    ] {
        assert!(html.contains(checked), "{checked}: die Seite zeigt den Wert aus dem Cookie: {html}");
    }
    assert!(html.contains("links 22\u{a0}%"), "Breite links aus dem Cookie");
    // Alle drei Abschnitte des Akkordeons sind wählbar; steht das Cookie auf „Fassungen“, ist genau das
    // markiert (vorher war dann gar nichts markiert und das Formular lehnte den Wert ab).
    for value in ["personas", "revisions", "chat"] {
        assert!(html.contains(&format!("name=\"accordion\" value=\"{value}\"")), "Wahl {value} auf der Seite");
    }
    let (_, _, html) =
        get_with(&server, "/einstellungen/darstellung", &[("cookie", "baruch_accordion=revisions")]).await;
    assert!(html.contains("name=\"accordion\" value=\"revisions\" checked"), "Fassungen markiert: {html}");
    for other in ["personas", "chat"] {
        assert!(!html.contains(&format!("name=\"accordion\" value=\"{other}\" checked")), "nur eine Wahl markiert");
    }
    let (status, headers, _) = form(&server, "/einstellungen/darstellung/act", "accordion=revisions").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "„Fassungen“ wird angenommen, nicht mit 422 abgelehnt");
    assert_eq!(
        headers.get(header::SET_COOKIE).and_then(|v| v.to_str().ok()),
        Some("baruch_accordion=revisions; Path=/; Max-Age=31536000; SameSite=Lax"),
        "Cookie im Format von widgets.ts"
    );
    let answer = server.rpc("settings.get", json!({ "page": "darstellung" })).await;
    assert_eq!(
        answer["result"],
        json!({ "theme": "system", "editor": "satz", "pv": "open", "accordion": "chat", "widths": { "left": null, "right": null } }),
        "RPC ohne Cookies: die Vorgaben"
    );
}

// ---- baruch_pv -----------------------------------------------------------------------------------

/// Öffnendes `<details …>` des ersten Stücks von Vorschlag `<key>-<rev>`.
fn pv_tag(html: &str, key: i64, rev: u32) -> String {
    let at = html.find(&format!("data-pv=\"{key}-{rev}\"")).unwrap_or_else(|| panic!("Vorschlag {rev} fehlt: {html}"));
    let start = html[..at].rfind("<details").expect("data-pv steht an einem <details>");
    html[start..at + html[at..].find('>').unwrap() + 1].to_owned()
}

#[tokio::test]
async fn closed_proposals_start_folded_except_the_shown_one() {
    let server = common::open().await;
    // Einleitung (Stelle 2): Fassung 2 aktuell, 3 Vorschlag von @lektor; dazu 4 von @nathan.
    let four = server
        .state
        .store
        .add_proposal(1, 2, "= Einleitung\n\nDer Igel schläft unter dem Laub.\n", "@nathan", "probe", "")
        .await
        .unwrap();
    assert_eq!(four, 4, "neuer Vorschlag Nummer 4");
    let key = server.rpc("node.get", json!({ "node": 2 })).await["result"]["key"].as_i64().unwrap();

    let (_, _, open) = get_with(&server, &format!("/?work=1&key={key}&rev=2"), &[]).await;
    for rev in [3, 4] {
        assert!(pv_tag(&open, key, rev).ends_with(" open>"), "ohne baruch_pv steht {rev} offen (Vorgabe unverändert)");
    }
    let closed_cookie = [("cookie", "baruch_pv=closed")];
    let (_, _, closed) = get_with(&server, &format!("/?work=1&key={key}&rev=2"), &closed_cookie).await;
    for rev in [3, 4] {
        assert!(
            !pv_tag(&closed, key, rev).contains(" open"),
            "baruch_pv=closed: {rev} eingeklappt: {}",
            pv_tag(&closed, key, rev)
        );
    }
    let (_, _, shown) = get_with(&server, &format!("/?work=1&key={key}&rev=3"), &closed_cookie).await;
    assert!(
        pv_tag(&shown, key, 3).ends_with(" open>"),
        "der gezeigte Vorschlag 3 bleibt offen: {}",
        pv_tag(&shown, key, 3)
    );
    assert!(!pv_tag(&shown, key, 4).contains(" open"), "der andere (4) bleibt eingeklappt");
    let (_, _, preview) = get_with(&server, &format!("/?work=1&key={key}&rev=2&view=preview"), &closed_cookie).await;
    assert!(!pv_tag(&preview, key, 3).contains(" open"), "auch in der Vorschau eingeklappt");
}

// ---- Werk ----------------------------------------------------------------------------------------

/// Formular ohne das versteckte Feld `back` (das nur die Einstellungsseite trägt).
fn without_back(form: &str) -> String {
    let Some(at) = form.find("<input type=\"hidden\" name=\"back\"") else { return form.to_owned() };
    let end = at + form[at..].find('>').unwrap() + 1;
    format!("{}{}", &form[..at], &form[end..])
}

#[tokio::test]
async fn the_work_page_reuses_the_forms_of_the_edit_and_numbering_pages() {
    let server = common::open().await;
    let (status, settings) = server.get("/einstellungen/werk?work=1").await;
    assert_eq!(status, StatusCode::OK, "Seite Werk lädt");
    let (_, edit) = server.get("/werke/1/bearbeiten").await;
    let (_, numbering) = server.get("/werke/1/nummerierung").await;

    let ours = cut(&settings, "<form class=\"work-form wide\"", "</form>");
    let theirs = cut(&edit, "<form class=\"work-form wide\"", "</form>");
    assert!(
        unescape(ours).contains("<input type=\"hidden\" name=\"back\" value=\"/einstellungen/werk?work=1\">"),
        "das Formular trägt back: {ours}"
    );
    assert!(!theirs.contains("name=\"back\""), "/werke/1/bearbeiten bleibt ohne back");
    assert_eq!(without_back(ours), theirs, "dasselbe Formular-Markup aus work_edit.html, nur mit back");

    let ours = cut(&settings, "<form class=\"numbering-choice\"", "</form>");
    let theirs = cut(&numbering, "<form class=\"numbering-choice\"", "</form>");
    assert!(
        ours.contains("formaction=\"/einstellungen/werk\">Vorschau</button>"),
        "Vorschau bleibt in den Einstellungen"
    );
    assert_eq!(
        without_back(ours).replace("formaction=\"/einstellungen/werk\"", "formaction=\"/werke/1/nummerierung\""),
        theirs,
        "dasselbe Formular-Markup aus work_numbering.html, nur mit back und eigener Vorschau-Adresse"
    );
    assert_eq!(settings.matches("class=\"template-revisions\"").count(), 1, "Fassungen der Werkvorlage einmal");

    let (_, preview) = server.get("/einstellungen/werk?work=1&scheme=custom&custom=I.%3BA.&back=x").await;
    assert!(preview.contains("value=\"I.;A.\""), "Vorschau-Werte gehen über die Abfrage an das Formular: {preview}");
}

#[tokio::test]
async fn forms_with_back_return_to_the_settings_and_foreign_back_is_ignored() {
    let server = common::open().await;
    let back = "%2Feinstellungen%2Fwerk%3Fwork%3D1";
    let (status, headers, _) =
        form(&server, "/werke/act", &format!("action=update&work=1&subtitle=Mit+Garten&back={back}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert");
    assert_eq!(location(&headers), Some("/einstellungen/werk?work=1&ok=werk"), "zurück in die Einstellungen");
    let (_, page) = server.get("/einstellungen/werk?work=1&ok=werk").await;
    assert!(page.contains("Das Werk ist gespeichert."), "Hinweis auf der Seite");
    assert!(page.contains("value=\"Mit Garten\""), "der neue Untertitel steht im Formular");

    for foreign in
        ["https%3A%2F%2Fevil.example%2F", "%2Fwerke", "%2F%2Fevil.example", "%2Feinstellungen%2Fwerk%3Fa%3D%3Cb%3E"]
    {
        let (_, headers, _) =
            form(&server, "/werke/act", &format!("action=update&work=1&audience=Kinder&back={foreign}")).await;
        assert_eq!(location(&headers), Some("/?work=1"), "fremdes back {foreign} wird übergangen");
    }
    let (_, headers, _) = form(&server, "/werke/act", "action=update&work=1&audience=Eltern").await;
    assert_eq!(location(&headers), Some("/?work=1"), "ohne back unverändert");

    let (status, headers, _) =
        form(&server, "/werke/nummerierung/act", &format!("work=1&scheme=chapter_roman&custom=&back={back}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Schema übernommen");
    assert_eq!(location(&headers), Some("/einstellungen/werk?work=1&ok=schema"), "zurück in die Einstellungen");
    let (_, page) = server.get("/einstellungen/werk?work=1&ok=schema").await;
    assert!(page.contains("Das Gliederungsschema ist übernommen."), "Hinweis des Schemas");
    let (_, headers, _) = form(&server, "/werke/nummerierung/act", "work=1&scheme=decimal&back=%2Fwerke").await;
    assert_eq!(location(&headers), Some("/werke/1/nummerierung?ok=1"), "fremdes back beim Schema übergangen");

    let (status, _, page) = form(&server, "/werke/act", &format!("action=update&work=1&title=&back={back}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "leerer Titel → 422 wie bisher");
    assert!(
        unescape(&page).contains("name=\"back\" value=\"/einstellungen/werk?work=1\""),
        "die Fehlerseite behält back für den zweiten Versuch"
    );

    // Abgelehnt im Popup: Die Antwort muss einen Rahmen tragen, den settings.ts übernehmen kann, mit der
    // Meldung und der Eingabe – sonst ersetzt das Popup alles durch den Fehlertext und die Eingabe ist weg.
    let body =
        format!("action=update&work=1&title=&subtitle=Eingabe+bleibt&template=%2F%2F+eigene+Vorlage&back={back}");
    let (status, _, page) = form(&server, "/werke/act", &body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "leerer Titel bleibt abgelehnt");
    assert_eq!(page.matches("data-settings-frame").count(), 1, "genau ein Rahmen für das Popup");
    assert!(page.contains("data-settings-page-body=\"werk\""), "die Seite „Werk“ der Einstellungen");
    assert!(page.contains("role=\"alert\""), "die Meldung steht im Rahmen");
    assert!(page.contains("value=\"Eingabe bleibt\""), "der eingegebene Untertitel bleibt stehen");
    assert!(unescape(&page).contains("// eigene Vorlage"), "die bearbeitete Werkvorlage bleibt stehen");
    assert!(page.contains("action=\"/werke/nummerierung/act\""), "das Schema-Formular bleibt im Rahmen");

    let (status, _, page) =
        form(&server, "/werke/nummerierung/act", &format!("work=1&scheme=custom&custom=I.%3BNr.&back={back}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültiges Muster → 422");
    assert_eq!(page.matches("data-settings-frame").count(), 1, "auch hier ein Rahmen für das Popup");
    assert!(page.contains("role=\"alert\"") && page.contains("Ebene 2"), "Meldung des Schemas im Rahmen");
    assert!(page.contains("value=\"I.;Nr.\""), "das eingegebene Muster bleibt stehen");
    assert!(
        unescape(&page).contains("name=\"back\" value=\"/einstellungen/werk?work=1\""),
        "back für den zweiten Versuch"
    );
}

// ---- Assistenten ---------------------------------------------------------------------------------

#[tokio::test]
async fn personas_are_edited_by_form_and_rpc_and_the_lector_follows() {
    let server = common::open().await;
    let (_, page) = server.get("/einstellungen/assistenten?work=1").await;
    assert!(page.contains("Lektor: <strong>Lektor</strong>"), "Vorgabe: @lektor hat den größten Kontext: {page}");
    // Runde 10: kein STUB-Hinweis mehr; ohne Modell-Backend sagt die Seite das ehrlich.
    assert!(!page.contains("Noch nicht umgesetzt: Modell und Kontext"), "STUB-Hinweis B2 entfernt");
    // Runde 17a (L4): Die Modellliste fragt die Seite nur auf Wunsch ab („Modellliste abfragen“).
    assert!(page.contains("Modellliste noch nicht abgefragt"), "beim Laden keine Abfrage: {page}");
    let (_, fetched) = server.get("/einstellungen/assistenten?work=1&modelle=abfragen").await;
    assert!(fetched.contains("Modellliste nicht verfügbar: Kein Modell-Backend"), "ehrliche Meldung ohne Backend");
    // Der Lektor folgt dem Kontext nur mit „automatisch“ (Vorgabe des Versuchs: fest @lektor).
    server.rpc("persona.set_lector", json!({ "mode": "auto" })).await;
    assert!(page.contains("id=\"persona-nathan\""), "Anker je Persona");
    // Runde 11: ein Formular je Persona (action=save); das alte method=persona.update geht weiter.
    assert!(page.contains("name=\"action\" value=\"save\""), "Formular „Speichern“ je Persona");

    let (status, headers, _) = form(
        &server,
        "/einstellungen/personas/act",
        "method=persona.update&handle=%40nathan&role=Pr%C3%BCft+gegen&model=probe-2&context_length=200000&active=1&work=1",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert → 303");
    assert_eq!(
        location(&headers),
        Some("/einstellungen/assistenten?ok=persona&work=1#persona-nathan"),
        "zurück zur Persona"
    );
    let (_, page) = server.get("/einstellungen/assistenten?ok=persona").await;
    assert!(page.contains("Die Persona ist gespeichert."), "Hinweis");
    assert!(page.contains("Lektor: <strong>Nathan</strong>"), "größerer Kontext macht Nathan zum Lektor: {page}");
    assert!(
        page.contains(">Prüft gegen</textarea>") && page.contains("value=\"probe-2\""),
        "Rolle und Modell gespeichert"
    );

    // Kästchen „aktiv“ fehlt → inaktiv; der Lektor wechselt zurück.
    let (status, ..) = form(
        &server,
        "/einstellungen/personas/act",
        "method=persona.update&handle=%40nathan&role=Pr%C3%BCft&model=probe-2&context_length=200000",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert");
    let (_, page) = server.get("/einstellungen/assistenten").await;
    assert!(page.contains("Lektor: <strong>Lektor</strong>"), "inaktiv zählt nicht: wieder der Lektor");

    for (body, why) in [
        ("method=persona.update&handle=%40nathan&role=+&model=m&context_length=5", "leere Rolle"),
        ("method=persona.update&handle=%40nathan&role=r&model=m&context_length=0", "Kontext 0"),
        ("method=persona.update&handle=%40nathan&role=r&model=m&context_length=viel", "Kontext keine Zahl"),
        ("method=persona.update&handle=%40niemand&role=r&model=m&context_length=5", "unbekannte Persona"),
    ] {
        let (status, _, page) = form(&server, "/einstellungen/personas/act", body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why} → 422");
        assert!(page.contains("role=\"alert\""), "{why}: Meldung auf der Seite");
        assert!(page.contains("data-settings-frame data-page=\"assistenten\""), "{why}: die Seite Assistenten");
    }

    let updated =
        server.rpc("persona.update", json!({ "handle": "@exeget", "context_length": 300000, "model": " groß " })).await;
    assert_eq!(updated["result"]["context_length"], 300000, "RPC persona.update speichert: {updated}");
    assert_eq!(updated["result"]["model"], "groß", "Modell getrimmt");
    let list = server.rpc("persona.list", json!({})).await;
    let lector = list["result"]["personas"].as_array().unwrap().iter().find(|p| p["lector"] == true).cloned();
    assert_eq!(lector.unwrap()["handle"], "@exeget", "persona.list zeigt den neuen Lektor");
    for (params, why) in [
        (json!({ "handle": "@exeget", "role": "x".repeat(201) }), "Rolle über 200 Zeichen"),
        (json!({ "handle": "@exeget", "model": "" }), "leeres Modell"),
        (json!({ "handle": "@exeget", "context_length": 10_000_001 }), "Kontext über 10 Mio."),
        (json!({ "handle": "@exeget", "context_length": -3 }), "negativer Kontext"),
        (json!({ "handle": "@exeget", "active": "ja" }), "active kein Wahrheitswert"),
        (json!({ "handle": "@niemand", "active": true }), "unbekannte Persona"),
        (json!({ "role": "r" }), "handle fehlt"),
    ] {
        let refused = server.rpc("persona.update", params).await;
        assert_eq!(error_code(&refused), Some(INVALID_PARAMS), "{why} → -32602: {refused}");
    }
}

#[tokio::test]
async fn persona_list_reads_every_field_even_without_a_work() {
    let server = common::open().await;
    let title = server.rpc("work.meta", json!({ "work": 1 })).await["result"]["work"]["title"].clone();
    let deleted = server.rpc("work.delete", json!({ "work": 1, "confirm_title": title })).await;
    assert!(deleted["result"]["deleted"].is_number(), "das einzige Werk ist gelöscht: {deleted}");
    let list = server.rpc("persona.list", json!({})).await;
    let lektor = &list["result"]["personas"][0];
    assert_eq!(lektor["handle"], "@lektor", "{list}");
    assert_eq!(lektor["model"], "google/gemma-4-26b-a4b", "Modell aus der Tabelle, nicht aus einem Werk");
    assert_eq!(lektor["context_length"], 32768, "Kontext aus der Tabelle (wie Eliezer Gemma lädt)");
    assert_eq!(lektor["active"], true, "aktiv aus der Tabelle");
    assert_eq!(lektor["lector"], true, "der Lektor ergibt sich auch ohne Werk");
    let (status, page) = server.get("/einstellungen/assistenten").await;
    assert_eq!(status, StatusCode::OK, "die Seite geht auch ohne Werk");
    assert!(page.contains("value=\"google&#x2f;gemma-4-26b-a4b\""), "Modell im Formular: {page}");
    let (_, werk) = server.get("/einstellungen/werk").await;
    assert!(werk.contains("Es gibt noch kein Werk."), "Seite Werk ohne Werk: ein Hinweis");
}

// ---- MCP-Zugang ----------------------------------------------------------------------------------

#[tokio::test]
async fn the_mcp_page_shows_the_address_and_the_clients_seen_after_initialize() {
    let server = common::open().await;
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Einstellungsprobe", "version": "9.1" } } })
            .to_string(),
        ))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, ..) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "initialize gelingt");

    let (status, _, html) = get_with(&server, "/einstellungen/mcp", &[("host", "127.0.0.1:8765")]).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    assert!(
        unescape(&html).contains("value=\"http://127.0.0.1:8765/mcp\""),
        "Adresse aus der Host-Kopfzeile + /mcp zum Kopieren: {html}"
    );
    assert!(html.contains("Nur über Loopback"), "Hinweis: nur Loopback ohne Token");
    assert!(!html.contains("Noch nicht umgesetzt"), "Tokens sind seit Runde 10 umgesetzt, kein STUB-Hinweis mehr");
    assert!(html.contains("Token erzeugen"), "Formular für Tokens (Runde 10)");
    let row = cut(&html, "<td>Einstellungsprobe</td>", "</tr>");
    assert!(
        row.contains("<td>9.1</td>") && row.contains("@mcp:einstellungsprobe"),
        "Client mit Version und Handle: {row}"
    );
    assert!(row.matches("<td>20").count() == 2, "zuerst und zuletzt gesehen als Datum: {row}");

    let answer = server.rpc("settings.get", json!({ "page": "mcp" })).await;
    assert_eq!(answer["result"]["path"], "/mcp", "{answer}");
    assert_eq!(answer["result"]["loopback_without_token"], true, "ohne Token nur Loopback");
    assert_eq!(answer["result"]["tokens"], json!([]), "noch keine Tokens (Runde 10: Liste ohne Hash)");
    let seen = answer["result"]["clients"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["handle"] == "@mcp:einstellungsprobe")
        .cloned();
    let seen = seen.expect("der Client steht in der Liste");
    assert!(seen["last_seen"].as_u64().unwrap() >= seen["first_seen"].as_u64().unwrap(), "zuletzt ≥ zuerst: {seen}");
    let rendered = server.rpc("settings.render", json!({ "page": "mcp" })).await;
    assert!(
        rendered["result"]["html"].as_str().unwrap().contains("value=\"/mcp\""),
        "über RPC ohne Host-Kopfzeile nur der Pfad: {rendered}"
    );
}

// ---- Daten ---------------------------------------------------------------------------------------

/// Server mit Datenordner `<tempdir>/daten`, damit eine Sicherung im tempdir landet (nicht in /tmp).
async fn open_in_subdir() -> common::Server {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("daten");
    std::fs::create_dir(&data).unwrap();
    let state = std::sync::Arc::new(AppState::open(&data).await.expect("Datenordner lässt sich öffnen"));
    common::Server { app: baruch_server::router(state.clone()), state, data: root }
}

/// Alle Dateien unter `dir` (relativ, sortiert).
fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path.strip_prefix(dir).unwrap().display().to_string());
            }
        }
    }
    out.sort();
    out
}

fn backups(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("daten.sicherung-"))
        .collect();
    found.sort();
    found
}

#[tokio::test]
async fn the_data_page_shows_folder_size_mirrors_and_version() {
    let server = open_in_subdir().await;
    let garden = server.import_garden().await;
    let (status, html) = server.get("/einstellungen/daten").await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    let data = server.state.data.display().to_string();
    assert!(unescape(&html).contains(&data), "Datenordner genannt: {html}");
    assert!(html.contains(&format!("Baruch {}", env!("CARGO_PKG_VERSION"))), "Version angezeigt");
    let mirror = server.state.data.join("works").join(&garden.slug);
    assert!(unescape(&html).contains(&mirror.display().to_string()), "Spiegelordner des Gartenbuchs");
    if let Some(head) = common::git(&mirror, &["rev-parse", "HEAD"]) {
        assert!(html.contains(&format!("<code>{}</code>", &head.trim()[..12])), "letzter Commit kurz: {html}");
    }
    let answer = server.rpc("settings.get", json!({ "page": "daten" })).await;
    let result = &answer["result"];
    assert!(result["size_bytes"].as_u64().unwrap() > 0, "Größe gezählt: {answer}");
    assert_eq!(result["version"], env!("CARGO_PKG_VERSION"), "Version");
    assert_eq!(result["mirrors"].as_array().unwrap().len(), 2, "Beispielwerk und Gartenbuch");
    assert!(result["mirrors"][1]["head"].as_str().is_some_and(|h| h.len() == 40), "voller Commit über RPC");
}

#[tokio::test]
async fn a_backup_needs_confirmation_and_copies_the_data_folder_next_to_it() {
    let server = open_in_subdir().await;
    let garden = server.import_garden().await;
    let root = server.path().to_owned();
    std::os::unix::fs::symlink(root.join("anderswo"), server.state.data.join("verknuepfung")).unwrap();

    let (status, _, page) = form(&server, "/einstellungen/daten/sicherung", "work=1").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Bestätigung → 422");
    assert!(page.contains("Sicherung bitte bestätigen"), "Meldung: {page}");
    let refused = server.rpc("settings.backup", json!({})).await;
    assert_eq!(error_code(&refused), Some(INVALID_PARAMS), "RPC ohne confirm → -32602: {refused}");
    assert!(backups(&root).is_empty(), "ohne Bestätigung keine Sicherung");

    let (status, headers, _) = form(&server, "/einstellungen/daten/sicherung", "work=1&confirm=1").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Sicherung angelegt");
    let made = backups(&root);
    assert_eq!(made.len(), 1, "genau eine Sicherung neben dem Datenordner: {made:?}");
    let name = made[0].file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(
        location(&headers).map(str::to_owned),
        Some("/einstellungen/daten?ok=sicherung&work=1".to_owned()),
        "zurück mit festem Meldecode, ohne Ordnernamen in der Adresse (Runde 23a)"
    );
    let stamp = name.strip_prefix("daten.sicherung-").unwrap();
    assert!(
        stamp.len() == 17 && stamp.as_bytes()[4] == b'-' && stamp.as_bytes()[10] == b'-',
        "Zeitstempel JJJJ-MM-TT-HHMMSS: {stamp}"
    );
    let (_, page) = server.get("/einstellungen/daten?ok=sicherung").await;
    assert!(
        page.contains(&format!("Sicherung angelegt: <code>{name}</code>")),
        "Hinweis mit dem Ordner, den der Server selbst im Dateisystem findet (Runde 23a): {page}"
    );

    assert!(!made[0].join("baruch.sqlite-wal").exists(), "keine WAL-Datei kopiert (VACUUM INTO)");
    let copy = Store::open(&made[0]).await.expect("die gesicherte baruch.sqlite öffnet sich");
    let titles = |list: Vec<baruch_store::WorkSummary>| list.into_iter().map(|w| w.title).collect::<Vec<_>>();
    assert_eq!(
        titles(copy.work_list().await.unwrap()),
        titles(server.state.store.work_list().await.unwrap()),
        "dieselben Werke in der Sicherung"
    );
    assert_eq!(
        files(&made[0].join("works")),
        files(&server.state.data.join("works")),
        "works/ vollständig kopiert (auch die Git-Ordner)"
    );
    assert!(made[0].join("works").join(&garden.slug).join(".git").is_dir(), "Git-Spiegel mitkopiert");
    assert!(std::fs::symlink_metadata(made[0].join("verknuepfung")).is_err(), "symbolische Verknüpfung übersprungen");

    let rpc = server.rpc("settings.backup", json!({ "confirm": true })).await;
    let skipped = &rpc["result"]["skipped"];
    if rpc["result"].is_object() {
        assert_eq!(skipped, &json!(["verknuepfung"]), "übersprungene Verknüpfung gemeldet: {rpc}");
    } else {
        // Zweite Sicherung in derselben Sekunde: sauber abgelehnt (siehe unten), nicht überschrieben.
        assert!(rpc["error"]["message"].as_str().unwrap().contains("gibt es schon"), "{rpc}");
    }

    // Zweite Sicherung in derselben Sekunde: Die Ordner der nächsten Sekunden gibt es schon (wie nach
    // einer Sicherung eben); die Sicherung scheitert, ohne etwas zu überschreiben.
    let pool = server.state.store.pool();
    let mut blocked = Vec::new();
    for offset in 0..4 {
        let stamp: String =
            sqlx::query_scalar(&format!("SELECT strftime('%Y-%m-%d-%H%M%S', 'now', '+{offset} seconds', 'localtime')"))
                .fetch_one(pool)
                .await
                .unwrap();
        let dir = root.join(format!("daten.sicherung-{stamp}"));
        if std::fs::create_dir(&dir).is_ok() {
            std::fs::write(dir.join("merker"), "unberührt").unwrap();
            blocked.push(dir);
        }
    }
    let again = baruch_server::settings::backup(&server.state).await;
    let message = again.expect_err("Ziel gibt es schon → Fehler");
    assert!(message.contains("gibt es schon; nichts überschrieben"), "deutsche Meldung: {message}");
    for dir in &blocked {
        assert_eq!(files(dir), ["merker"], "der vorhandene Ordner bleibt unberührt: {}", dir.display());
    }
    assert!(Store::open(&made[0]).await.is_ok(), "die erste Sicherung bleibt heil");
}

#[tokio::test]
async fn backups_stop_at_the_limit_instead_of_filling_the_disk() {
    // Jede Sicherung ist eine volle Kopie; ohne Obergrenze füllten wiederholte Aufrufe (RPC ohne Browser)
    // das Dateisystem, das auch die laufende Datenbank trägt.
    let server = open_in_subdir().await;
    let root = server.path().to_owned();
    for n in 0..baruch_server::settings::MAX_BACKUPS {
        let dir = root.join(format!("daten.sicherung-alt-{n:02}"));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("merker"), "alt").unwrap();
    }
    let before = backups(&root);
    let refused = baruch_server::settings::backup(&server.state).await.expect_err("Obergrenze erreicht");
    assert!(refused.contains("höchstens 10") && refused.contains("Nichts angelegt"), "deutsche Meldung: {refused}");
    let rpc = server.rpc("settings.backup", json!({ "confirm": true })).await;
    assert!(rpc["error"]["message"].as_str().is_some_and(|m| m.contains("bitte alte entfernen")), "RPC: {rpc}");
    let (status, _, page) = form(&server, "/einstellungen/daten/sicherung", "confirm=1").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Formular → 422 mit Meldung");
    assert!(page.contains("bitte alte entfernen"), "Meldung auf der Seite");
    assert_eq!(backups(&root), before, "keine neue Sicherung, keine alte gelöscht (Baruch löscht keine selbst)");

    std::fs::remove_dir_all(&before[0]).unwrap();
    let made = baruch_server::settings::backup(&server.state).await.expect("unter der Grenze wieder möglich");
    assert!(made.dir.join("baruch.sqlite").is_file(), "die Sicherung ist vollständig");
}

// ---- RPC -----------------------------------------------------------------------------------------

#[tokio::test]
async fn settings_rpc_lists_renders_reads_and_writes_the_pages() {
    let server = common::open().await;
    let pages = server.rpc("settings.pages", json!({})).await;
    let list = pages["result"]["pages"].as_array().unwrap();
    assert_eq!(list.len(), 7, "sieben Seiten: {pages}");
    assert_eq!(
        list[0],
        json!({ "slug": "darstellung", "title": "Darstellung", "storage": "Cookie in diesem Browser", "url": "/einstellungen/darstellung" }),
        "Eintrag mit slug, Titel, Speicherort, Adresse"
    );
    let slugs: Vec<&str> = list.iter().map(|p| p["slug"].as_str().unwrap()).collect();
    assert_eq!(slugs, SLUGS, "Reihenfolge wie die Registrierung");

    let rendered =
        server.rpc("settings.render", json!({ "page": "assistenten", "work": 1, "query": { "ok": "farbe" } })).await;
    let (_, fragment) = server.get("/einstellungen/assistenten?work=1&ok=farbe&fragment=1").await;
    assert_eq!(rendered["result"]["html"], fragment, "settings.render liefert dasselbe Fragment wie ?fragment=1");
    assert_eq!(rendered["result"]["title"], "Assistenten", "Titel");
    assert_eq!(rendered["result"]["storage"], "SQLite (alle Werke)", "Speicherort");
    let unknown = server.rpc("settings.render", json!({ "page": "nichts" })).await;
    assert_eq!(error_code(&unknown), Some(INVALID_PARAMS), "unbekannte Seite → -32602");
    let missing = server.rpc("settings.render", json!({ "page": "werk", "work": 999 })).await;
    assert_eq!(error_code(&missing), Some(INVALID_PARAMS), "unbekanntes Werk → -32602");

    let werk = server.rpc("settings.get", json!({ "page": "werk", "work": 1 })).await;
    assert_eq!(werk["result"]["title"], "Beispielwerk: Leise Abhängigkeit (Auszug)", "{werk}");
    assert_eq!(werk["result"]["kind"], "book", "Art");
    assert!(werk["result"]["numbering"]["scheme"].is_string(), "Gliederungsschema dabei");
    assert_eq!(
        werk["result"]["timing"],
        json!({ "words_per_minute": 110, "target_seconds": null }),
        "Runde 16b: Predigtzeit dabei, ohne Einstellung die Vorgabe 110 Wörter je Minute und keine Zielzeit"
    );
    let assistants = server.rpc("settings.get", json!({ "page": "assistenten" })).await;
    assert_eq!(assistants["result"]["lector"], "@lektor", "Lektor");
    assert_eq!(assistants["result"]["personas"][0]["context_length"], 32768, "PersonaRow mit Kontext");
    for page in ["verbindungen", "quellen"] {
        let answer = server.rpc("settings.get", json!({ "page": page })).await;
        assert_eq!(error_code(&answer), Some(INVALID_PARAMS), "{page} über connection.list bzw. sources.list");
        assert!(answer["error"]["message"].as_str().unwrap().contains("connection.list bzw. sources.list"), "{answer}");
    }

    let set = server.rpc("settings.set", json!({ "page": "werk", "work": 1, "values": { "subtitle": "Neu" } })).await;
    assert_eq!(set["result"]["changed"], json!(["subtitle"]), "dieselbe Funktion wie work.update: {set}");
    // Der Lektor folgt dem Kontext nur mit „automatisch“ (Runde 10, Vorgabe fest @lektor).
    server.rpc("persona.set_lector", json!({ "mode": "auto" })).await;
    let set = server
        .rpc("settings.set", json!({ "page": "assistenten", "values": { "handle": "@hoerer", "context_length": 999999, "color": "#123456" } }))
        .await;
    assert_eq!(set["result"]["persona"]["context_length"], 999999, "Kontext gespeichert: {set}");
    assert_eq!(set["result"]["persona"]["color"], "#123456", "Farbe gespeichert");
    assert_eq!(set["result"]["lector"], "@hoerer", "der Lektor folgt");
    let bad = server
        .rpc(
            "settings.set",
            json!({ "page": "assistenten", "values": { "handle": "@hoerer", "context_length": 5, "color": "grün" } }),
        )
        .await;
    assert_eq!(error_code(&bad), Some(INVALID_PARAMS), "ungültige Farbe → -32602");
    let after = server.rpc("settings.get", json!({ "page": "assistenten" })).await;
    let hoerer =
        after["result"]["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@hoerer").cloned().unwrap();
    assert_eq!(hoerer["context_length"], 999999, "abgelehnter Aufruf ändert auch den Kontext nicht");
    let display = server.rpc("settings.set", json!({ "page": "darstellung", "values": { "theme": "dark" } })).await;
    assert_eq!(error_code(&display), Some(INVALID_PARAMS), "Darstellung nicht über RPC");
    assert!(
        display["error"]["message"].as_str().unwrap().contains("POST /einstellungen/darstellung/act"),
        "mit Hinweis auf das Formular: {display}"
    );
    let other = server.rpc("settings.set", json!({ "page": "mcp", "values": {} })).await;
    assert_eq!(error_code(&other), Some(INVALID_PARAMS), "MCP-Zugang hat nichts zu speichern");
}

#[tokio::test]
async fn settings_are_not_reachable_over_mcp_or_act() {
    let server = common::open().await;
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = server.send(request).await;
    let list: Value = serde_json::from_str(&body).unwrap();
    let tools = list["result"]["tools"].as_array().expect("Werkzeugliste");
    assert!(
        // Ausnahme Runde 29: `typeset_settings` liest die Satz-Einstellungen nur (Vertrag R29 §6.2); ändern kann sie nur
        // der Mensch. Dass es nur liest, prüft round29_daten; hier zählt, dass kein weiteres Einstellungswerkzeug kommt.
        !tools.iter().any(|t| t["name"].as_str().is_some_and(|n| n != "typeset_settings"
            && (n.contains("setting") || n.contains("backup") || n.contains("persona_update")))),
        "kein MCP-Werkzeug für Einstellungen, Sicherung oder persona.update: {list}"
    );
    for method in ["settings.backup", "settings.set", "persona.update"] {
        let (status, ..) = form(&server, "/act", &format!("method={method}&confirm=1")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "/act kennt {method} nicht");
    }
}
