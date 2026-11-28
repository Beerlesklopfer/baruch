// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 35, oberflaeche_konten (contract §3): sign-in page, setup, Mein Konto, Mitwirkende, settings page „Konten“,
//! the account entry in the menu and what a Lektor sees.
//!
//! Every assertion with its reason; only invented works and names, no network, no Eliezer. Who makes a request is
//! determined by the tests without session and password: through `common::with_who` (through the whole router) or –
//! where the lock of the roles (`roles`, task „rollen“) still has to open the page for Lektors – through `auth::scope`
//! around the pages themselves (small router from `pages()` and the layer `consent::inject`, or `view::page` directly).
//! That way the tests check **what the user interface shows**; whether a role may reach a route is checked by „rollen“.
//! Accounts are created via SQL (hash column: fixed text `$argon2id$test`) – nobody ever signs in here.

use crate::common;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_core::Post;
use baruch_server::auth::{self, AccountKind, Identity, Who};
use baruch_server::konten_ui::pages::{
    AccountRow, AccountView, AccountWork, AccountsView, MembersView, account_html, accounts_html, members_html,
};
use baruch_server::konten_ui::{self, LoginView, SetupView};
use baruch_server::view::{self, Location, Settings, Shelf};
use baruch_store::{FindingKind, NewFinding};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A name full of special characters: every output has to escape it.
const NASTY: &str = "<b>\"O'Neil\" & Söhne</b>";
const NASTY_ESCAPED: &str = "&lt;b&gt;&quot;O&#x27;Neil&quot; &amp; Söhne&lt;&#x2f;b&gt;";

const HANDLE: &str = "@lektorat:erika";

fn author_account() -> Who {
    Who::Account(Identity {
        id: 1,
        name: "joerg".into(),
        display: "Jörg Beispiel".into(),
        kind: AccountKind::Author,
        handle: auth::lektor_handle("joerg"),
    })
}

fn lektor_account() -> Who {
    Who::Account(Identity {
        id: 2,
        name: "Erika".into(),
        display: "Erika Muster".into(),
        kind: AccountKind::Lektor,
        handle: auth::lektor_handle("Erika"),
    })
}

/// Creates an account via SQL (no password involved: the hash is a fixed text that is never checked).
async fn insert_account(server: &common::Server, name: &str, slug: &str, kind: &str) -> i64 {
    sqlx::query("INSERT INTO account (name, name_key, slug, display, kind, password) VALUES (?, ?, ?, '', ?, ?)")
        .bind(name)
        .bind(name.to_lowercase())
        .bind(slug)
        .bind(kind)
        .bind("$argon2id$test")
        .execute(server.state.store.pool())
        .await
        .expect("Konto anlegen")
        .last_insert_rowid()
}

/// Sample work with one section in which the author, the Lektor Erika and a persona have written, plus one proposal and
/// two findings of the Lektor (one at the section, one as a remark on the book).
struct Desk {
    server: common::Server,
    work: i64,
    key: i64,
    /// Number of the Lektor's proposal.
    proposal: u32,
}

async fn new_desk() -> Desk {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let loaded = server.state.store.load_work(work).await.unwrap();
    let node = 1usize;
    let key = loaded.nodes[node].key;
    let mut persona_post = 0;
    for (author, body) in [
        ("Autor", "Bitte auf die Übergänge achten."),
        (HANDLE, "Der zweite Satz holpert."),
        ("@lektor", "Notiert.\n- Hinweis: Das Bild der Rosen kommt zu früh."),
    ] {
        let post = Post {
            id: 0,
            created: String::new(),
            author: author.into(),
            body: body.into(),
            sources: vec![],
            refs: vec![],
        };
        persona_post = server.state.store.add_post(work, node, &post).await.unwrap();
    }
    // The finding of the persona belongs to its post (position 0): the chat shows it there as a finding block.
    let block = NewFinding {
        work,
        key: Some(key),
        post: Some(persona_post),
        thread_post: None,
        position: 0,
        persona: "@lektor",
        kind: FindingKind::Hint,
        body: "Das Bild der Rosen kommt zu früh.",
        quote: "",
        quote_check: "",
        similar_to: None,
        summary_of: &[],
    };
    server.state.store.finding_insert(&block).await.unwrap();
    let proposal = server
        .state
        .store
        .add_proposal(work, node, "Ein anderer Wortlaut mit Rosen am Zaun.", HANDLE, "", "Satzbau geglättet")
        .await
        .unwrap();
    for (at, body) in [(Some(key), "Hier fehlt ein Komma."), (None, "Das Vorwort ist zu lang.")] {
        let new = NewFinding {
            work,
            key: at,
            post: None,
            thread_post: None,
            position: 0,
            persona: HANDLE,
            kind: FindingKind::Hint,
            body,
            quote: "",
            quote_check: "",
            similar_to: None,
            summary_of: &[],
        };
        server.state.store.finding_insert(&new).await.unwrap();
    }
    Desk { server, work, key, proposal }
}

async fn body_of(response: axum::response::Response) -> String {
    String::from_utf8(axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap()
}

/// GET through the whole router as `who` (the layer `auth::gate` takes `who` from the extensions).
async fn get_as(server: &common::Server, uri: &str, who: Who) -> (StatusCode, axum::http::HeaderMap, String) {
    server.send(common::with_who(Request::get(uri).body(Body::empty()).unwrap(), who)).await
}

/// GET to the pages `pages` behind the layer `consent::inject`, executed as `who` (`auth::scope`) – without the layer
/// `auth::gate`: what `who` would see if the roles allow the page.
async fn page_as(
    server: &common::Server,
    pages: Router<Arc<baruch_server::AppState>>,
    uri: &str,
    who: Who,
) -> (StatusCode, String) {
    let app = pages
        .layer(axum::middleware::from_fn_with_state(server.state.clone(), baruch_server::consent::inject))
        .with_state(server.state.clone());
    let response = auth::scope(who, app.oneshot(Request::get(uri).body(Body::empty()).unwrap())).await.unwrap();
    let status = response.status();
    (status, body_of(response).await)
}

/// The work page as `who` would get it: `view::page` in `auth::scope`, then `konten_ui::apply` (with accounts).
async fn workspace_as(desk: &Desk, who: &Who, rev: Option<u32>, preview: bool) -> String {
    let state = desk.server.state.clone();
    let at = Location {
        work: Some(desk.work),
        key: Some(desk.key),
        rev: rev.map(|r| r as usize),
        view: preview.then(|| "preview".to_owned()),
        ..Location::default()
    };
    // Round 39a: drawn outside a request, so German is pinned here (the assertions read the German wording).
    let drawn = auth::scope(who.clone(), async {
        let work = state.store.load_work(desk.work).await.unwrap();
        let shelf = Shelf {
            id: desk.work,
            slug: state.store.work_slug(desk.work).await.unwrap(),
            works: state.store.work_list().await.unwrap(),
        };
        let library = baruch_server::rpc::library_view(&state, desk.work, None).await.expect("Ansichtsdaten");
        let resolved = at.resolve(&work);
        view::page(&state.templates, &work, &shelf, resolved, &Settings::from_cookie_header(None), &library)
            .expect("Arbeitsseite zeichnen")
    });
    let html = baruch_server::i18n::scope(baruch_server::i18n::Lang::De, drawn).await;
    konten_ui::apply(&html, who, true)
}

/// The three panes (like `ui.render`), drawn as `who`.
async fn panes_as(desk: &Desk, who: Who) -> view::Panes {
    let state = desk.server.state.clone();
    let (work_id, key) = (desk.work, desk.key);
    auth::scope(who, async move {
        let work = state.store.load_work(work_id).await.unwrap();
        let shelf = Shelf { id: work_id, slug: String::new(), works: Vec::new() };
        let library = baruch_server::rpc::library_view(&state, work_id, None).await.expect("Ansichtsdaten");
        let at = Location { work: Some(work_id), key: Some(key), ..Location::default() };
        let settings = Settings::from_cookie_header(None);
        view::panes(&state.templates, &work, &shelf, at.resolve(&work), &settings, &library).expect("Bereiche zeichnen")
    })
    .await
}

/// The content of the page (`<main>…</main>`), without ribbon.
fn main_of(page: &str) -> &str {
    &page[page.find("<main").expect("Seite mit <main>")..page.find("</main>").expect("Ende von <main>")]
}

/// All opening tags `<name …>` of a page.
fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name} ");
    html.match_indices(&open).map(|(at, _)| &html[at..at + html[at..].find('>').unwrap() + 1]).collect()
}

/// Value of an attribute in a tag (minijinja escapes `/` as `&#x2f;`).
fn attr(tag: &str, name: &str) -> Option<String> {
    let rest = &tag[tag.find(&format!(" {name}=\""))? + name.len() + 3..];
    Some(rest[..rest.find('"')?].replace("&#x2f;", "/").replace("&amp;", "&"))
}

/// The opening tag of the element that contains `needle` (e.g. the link with a certain address).
fn tag_with<'a>(html: &'a str, needle: &str) -> &'a str {
    let at = html.find(needle).unwrap_or_else(|| panic!("{needle} steht nicht in der Seite"));
    let start = html[..at].rfind('<').unwrap();
    &html[start..start + html[start..].find('>').unwrap() + 1]
}

/// The form (from `<form` to `</form>`) that contains `needle`.
fn form_with<'a>(html: &'a str, needle: &str) -> &'a str {
    let at = html.find(needle).unwrap_or_else(|| panic!("{needle} steht nicht in der Seite"));
    let start = html[..at].rfind("<form").unwrap_or_else(|| panic!("{needle} steht in keinem Formular"));
    &html[start..start + html[start..].find("</form>").unwrap()]
}

/// Password fields of a page: never prefilled, with the expected `autocomplete`.
fn assert_password_fields(html: &str, expected: &[(&str, &str)], at: &str) {
    let fields: Vec<&str> = tags(html, "input").into_iter().filter(|t| t.contains("type=\"password\"")).collect();
    assert_eq!(fields.len(), expected.len(), "{at}: genau die Passwortfelder des Vertrags: {fields:?}");
    for (field, (name, autocomplete)) in fields.iter().zip(expected) {
        assert_eq!(attr(field, "name").as_deref(), Some(*name), "{at}: Feldname aus Vertrag §1.5");
        assert_eq!(
            attr(field, "autocomplete").as_deref(),
            Some(*autocomplete),
            "{at}: {name} sagt dem Passwortmanager, ob das bisherige oder ein neues Passwort gemeint ist"
        );
        assert!(!field.contains(" value="), "{at}: ein Passwortfeld ist nie vorbelegt: {field}");
    }
}

/// The page works without JavaScript: no script, no button without a form, no widget.
fn assert_plain_forms(html: &str, at: &str) {
    assert!(!html.contains("type=\"button\""), "{at}: jeder Knopf schickt ein Formular ab (ohne JavaScript bedienbar)");
    assert!(!html.contains("data-widget"), "{at}: die Seite braucht kein Widget");
    assert!(!html.contains("onclick") && !html.contains("javascript:"), "{at}: kein Skript im Markup");
}

// ---- Without accounts ----------------------------------------------------------------------------------------

#[tokio::test]
async fn without_accounts_every_page_only_gains_the_one_menu_entry() {
    let desk = new_desk().await;
    let uris = [
        format!("/?work={}&key={}", desk.work, desk.key),
        "/werke".to_owned(),
        format!("/werke/{}/befunde", desk.work),
        format!("/beitraege?work={}&key={}", desk.work, desk.key),
        format!("/gliederung?work={}", desk.work),
    ];
    for uri in &uris {
        let (status, page) = desk.server.get(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: ohne Konten lädt die Seite ohne Anmeldung und ohne Umleitung");
        assert_eq!(page.matches("Konten einrichten…").count(), 1, "{uri}: genau ein neuer Eintrag im Menü „Baruch ▾“");
        assert!(
            tag_with(&page, ">Konten einrichten…").contains("href=\"/einrichten\""),
            "{uri}: der Eintrag führt auf /einrichten"
        );
        for absent in [
            "data-rolle",
            "nur-autor",
            "nur-lektor",
            "NUR-",
            "BARUCH-KONTO",
            "/lektorat/act",
            "/abmelden",
            "Angemeldet:",
            "/mitwirkende",
            "Mein Konto",
        ] {
            assert!(
                !page.contains(absent),
                "{uri}: ohne Konten steht nichts von Konten und Rollen in der Seite ({absent}) – sie bleibt bis auf \
                 den Menüeintrag wie vor Runde 35"
            );
        }
    }
    let (_, page) = desk.server.get(&uris[0]).await;
    for present in [
        "name=\"method\" value=\"outline.save\"",
        "data-saved=\"Gespeichert\" data-dirty=\"Speichern\"",
        ">Als neue Fassung speichern</button>",
        "action=\"/assistenten/act\"",
        "name=\"method\" value=\"revision.delete\"",
        "id=\"band-tab-einfuegen\"",
        "class=\"literature-dialog\"",
        "data-widget=\"outline\"",
    ] {
        assert!(page.contains(present), "die Arbeitsseite hat ohne Konten alle bisherigen Knöpfe: {present}");
    }
    let (_, proposal) = desk.server.get(&format!("/?work={}&key={}&rev={}", desk.work, desk.key, desk.proposal)).await;
    assert!(
        proposal.contains(">Übernehmen</button>") && proposal.contains(">Verwerfen</button>"),
        "über einen offenen Vorschlag entscheidet ohne Konten, wer am Rechner sitzt – wie bisher"
    );
}

#[tokio::test]
async fn without_accounts_the_account_pages_explain_and_offer_setup() {
    let server = common::open().await;
    let (status, headers, _) = server.send(Request::get("/anmelden?next=/werke").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne Konten gibt es nichts anzumelden");
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/", "die Anmeldeseite führt dann zur Startseite");

    let (status, page) = server.get("/konto").await;
    assert_eq!(status, StatusCode::OK, "Mein Konto lädt auch ohne Konten");
    assert!(page.contains("Baruch läuft ohne Konten"), "die Seite sagt ehrlich, dass es keine Konten gibt");
    assert!(page.contains("href=\"/einrichten\""), "und bietet das Einrichten an");
    assert!(!page.contains("action=\"/konto/passwort\""), "ohne Konto gibt es kein Passwort zu ändern");

    let (status, page) = server.get("/einstellungen/konten").await;
    assert_eq!(status, StatusCode::OK, "die achte Einstellungsseite lädt");
    assert!(page.contains("data-settings-page-body=\"konten\""), "der Inhalt der Seite Konten");
    assert!(page.contains("<strong>ohne Konten</strong>"), "ohne Konten erklärt die Seite den offenen Zustand");
    assert!(
        page.contains("<a class=\"btn primary\" href=\"/einrichten\">Einrichten</a>"),
        "und hat den Knopf „Einrichten“"
    );
    assert!(!page.contains("/einstellungen/konten/act"), "ohne Konten gibt es nichts zu verwalten");
    assert!(
        page.contains("data-settings-page=\"konten\" aria-current=\"page\">Konten</a>"),
        "die Navigation der Einstellungen nennt die Seite Konten als gezeigte"
    );

    let work = server.state.default_work().await.unwrap();
    let (status, page) = server.get(&format!("/werke/{work}/mitwirkende")).await;
    assert_eq!(status, StatusCode::OK, "Mitwirkende lädt ohne Konten");
    assert!(
        page.contains("Baruch läuft ohne Konten") && !page.contains("/mitwirkende/act"),
        "ohne Konten gibt es niemanden freizugeben – Erklärung statt Formular"
    );
    let (status, _) = server.get("/werke/987654/mitwirkende").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "ein unbekanntes Werk ist 404, keine leere Seite");
}

// ---- Sign-in and setup -----------------------------------------------------------------------------------------

#[tokio::test]
async fn the_login_page_has_the_form_the_notice_and_a_safe_way_on() {
    let server = common::open().await;
    insert_account(&server, "joerg", "joerg", "author").await;

    let (status, headers, page) =
        get_as(&server, "/anmelden?next=%2Fwerke%2F3%2Fbefunde%3Fkey%3D7&ok=abgemeldet", Who::Anonymous).await;
    assert_eq!(status, StatusCode::OK, "die Anmeldeseite ist ohne Anmeldung erreichbar");
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store", "sie gehört in keinen Zwischenspeicher");
    let form = form_with(&page, "name=\"password\"");
    assert!(form.starts_with("<form class=\"konto-form\" method=\"post\" action=\"/anmelden\">"), "POST /anmelden");
    let next = tags(form, "input").into_iter().find(|t| t.contains("name=\"next\"")).expect("Feld next");
    assert_eq!(
        attr(next, "value").as_deref(),
        Some("/werke/3/befunde?key=7"),
        "das Ziel nach der Anmeldung reist im Formular mit"
    );
    assert!(tags(form, "input").iter().any(|t| t.contains("name=\"name\"")), "Feld name");
    assert_password_fields(&page, &[("password", "current-password")], "Anmeldeseite");
    assert!(page.contains("<p class=\"notice\" role=\"status\">Abgemeldet.</p>"), "?ok=abgemeldet zeigt den Satz");
    let onward = tag_with(&page, ">Weiter</a>");
    assert_eq!(
        attr(onward, "href").as_deref(),
        Some("/werke/3/befunde?key=7"),
        "„Schon angemeldet? Weiter“ führt auf das Ziel: Bei einem Link von außen schickt der Browser das Cookie \
         (SameSite=Strict) erst bei diesem Klick mit"
    );
    assert!(page.contains("Schon angemeldet?"), "der Hinweis steht da, solange keine Sitzung erkannt ist");
    assert!(!page.contains("<script") && !page.contains("class=\"band\""), "ohne JavaScript und ohne Menüband");
    assert_plain_forms(&page, "Anmeldeseite");

    let (_, _, evil) = get_as(
        &server,
        "/anmelden?next=%2F%2Fboese.example%2Fx&ok=%3Cscript%3Ealert(1)%3C%2Fscript%3E&fehler=passwort_falsch",
        Who::Anonymous,
    )
    .await;
    assert!(!evil.contains("boese.example"), "ein fremdes Ziel steht nirgends in der Seite");
    assert_eq!(attr(tag_with(&evil, ">Weiter</a>"), "href").as_deref(), Some("/"), "es wird zur Startseite");
    assert!(!evil.contains("alert(1)"), "ein unbekannter Code zeigt nichts – kein Text aus der Adresse im HTML");
    assert!(
        !evil.contains("role=\"alert\"") && !evil.contains("role=\"status\""),
        "die Anmeldeseite nimmt aus der Adresse nur Erfolgsmeldungen der Tabelle; Fehler der Anmeldung setzt der \
         Server selbst"
    );

    let (_, _, signed) = get_as(&server, "/anmelden?next=/konto", author_account()).await;
    assert!(
        signed.contains("Angemeldet als Jörg Beispiel. <a href=\"&#x2f;konto\">Weiter</a>")
            || signed.contains("Angemeldet als Jörg Beispiel. <a href=\"/konto\">Weiter</a>"),
        "mit gültiger Sitzung nennt die Seite das Konto und den Weg weiter: {signed}"
    );
}

#[tokio::test]
async fn login_and_setup_show_error_wait_and_status_as_given() {
    let view = LoginView {
        name: NASTY.into(),
        next: "/?work=2&key=7".into(),
        error: Some("Name oder Passwort stimmt nicht.".into()),
        retry_after: Some(8),
    };
    let response = konten_ui::login_page(StatusCode::TOO_MANY_REQUESTS, &view);
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS, "der Status kommt vom Aufrufer (429 in der Sperre)");
    let page = body_of(response).await;
    assert!(
        page.contains("<p class=\"notice error\" role=\"alert\">Name oder Passwort stimmt nicht.</p>"),
        "die Fehlermeldung steht sichtbar und als Alarm für Vorleseprogramme da"
    );
    assert!(
        page.contains("data-retry-after=\"8\"") && page.contains("Bitte 8 Sekunden warten"),
        "die Wartezeit steht in Sekunden da: {page}"
    );
    assert!(page.contains(&format!("value=\"{NASTY_ESCAPED}\"")), "der Name ist maskiert vorbelegt: {page}");
    assert!(!page.contains("<b>"), "nichts aus dem Namen wird Markup");
    assert_password_fields(&page, &[("password", "current-password")], "Anmeldeseite mit Fehler");
    let one = LoginView { retry_after: Some(1), error: None, ..view };
    let page = body_of(konten_ui::login_page(StatusCode::TOO_MANY_REQUESTS, &one)).await;
    assert!(page.contains("Bitte 1 Sekunde warten"), "eine Sekunde steht in der Einzahl da");

    let setup =
        SetupView { name: NASTY.into(), display: "Jörg & Co".into(), error: Some("Zu kurz.".into()), allowed: true };
    let response = konten_ui::setup_page(StatusCode::UNPROCESSABLE_ENTITY, &setup);
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY, "Status wie übergeben");
    let page = body_of(response).await;
    let form = form_with(&page, "name=\"password2\"");
    assert!(form.starts_with("<form class=\"konto-form\" method=\"post\" action=\"/einrichten\">"), "POST /einrichten");
    for field in ["name=\"name\"", "name=\"display\"", "name=\"password\"", "name=\"password2\""] {
        assert!(form.contains(field), "Einrichten mit dem Feld {field} (Vertrag §1.5)");
    }
    assert!(
        form.contains(&format!("value=\"{NASTY_ESCAPED}\"")) && form.contains("value=\"Jörg &amp; Co\""),
        "Name und Anzeigename bleiben nach einem Fehler erhalten, maskiert"
    );
    assert_password_fields(&page, &[("password", "new-password"), ("password2", "new-password")], "Einrichten");
    assert!(page.contains("role=\"alert\">Zu kurz.</p>"), "die Fehlermeldung steht da");
    assert!(!page.contains("<script"), "ohne JavaScript");
    assert_plain_forms(&page, "Einrichten");
}

#[tokio::test]
async fn setup_is_offered_only_on_the_own_machine_and_only_without_accounts() {
    let server = common::open().await;
    let own = ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41000)));
    let request = |build: axum::http::request::Builder, peer: Option<ConnectInfo<SocketAddr>>| {
        let mut request = build.body(Body::empty()).unwrap();
        if let Some(peer) = peer {
            request.extensions_mut().insert(peer);
        }
        request
    };

    let (status, _, page) =
        server.send(request(Request::get("/einrichten").header(header::HOST, "127.0.0.1:8765"), Some(own))).await;
    assert_eq!(status, StatusCode::OK, "vom eigenen Rechner kommt das Formular");
    assert!(page.contains("action=\"/einrichten\""), "Formular zum Anlegen des Autor-Kontos");

    let (status, _, page) = server.send(request(Request::get("/einrichten"), None)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "ohne bekannte Gegenstelle gilt die Anfrage nicht als eigener Rechner");
    assert!(
        page.contains("nur am eigenen Rechner einrichten") && !page.contains("<form"),
        "403 mit Erklärung, ohne Formular: {page}"
    );

    for name in ["x-forwarded-for", "forwarded", "x-real-ip", "via", "x-forwarded-host", "x-forwarded-proto"] {
        let build = Request::get("/einrichten").header(header::HOST, "127.0.0.1:8765").header(name, "203.0.113.9");
        let (status, _, page) = server.send(request(build, Some(own))).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{name}: Die Kopfzeile verrät einen vorgeschalteten Server – auch von Loopback ist das nicht der eigene \
             Rechner (Vertrag §0.2)"
        );
        assert!(!page.contains("<form"), "{name}: kein Formular");
    }

    insert_account(&server, "joerg", "joerg", "author").await;
    let build = Request::get("/einrichten").header(header::HOST, "127.0.0.1:8765");
    let (status, headers, _) = server.send(common::with_who(request(build, Some(own)), author_account())).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit Konten gibt es nichts mehr einzurichten");
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/einstellungen/konten", "weiter zur Verwaltung der Konten");
}

// ---- Menu „Baruch ▾“ -------------------------------------------------------------------------------------------

#[tokio::test]
async fn an_author_account_sees_who_is_signed_in_and_keeps_every_button() {
    let desk = new_desk().await;
    insert_account(&desk.server, "joerg", "joerg", "author").await;
    let uri = format!("/?work={}&key={}&rev={}", desk.work, desk.key, desk.proposal);
    let (status, headers, page) = get_as(&desk.server, &uri, author_account()).await;
    assert_eq!(status, StatusCode::OK, "der Autor erreicht die Arbeitsseite");
    assert!(headers.get(header::SET_COOKIE).is_some(), "die Seite merkt sich das Werk wie bisher (Cookie baruch_work)");
    let menu = &page[page.find("data-menu=\"baruch\"").unwrap()..];
    let menu = &menu[..menu.find("</details>").unwrap()];
    assert!(menu.contains("Angemeldet: Jörg Beispiel (Autor)"), "Anzeigename und Rolle im Menü „Baruch ▾“: {menu}");
    assert!(menu.contains("href=\"/konto\"") && menu.contains(">Mein Konto</a>"), "Mein Konto");
    assert!(
        menu.contains("<form class=\"menu-form\" method=\"post\" action=\"/abmelden\">"),
        "Abmelden als Formular (POST /abmelden)"
    );
    assert!(
        menu.find("Baruch beenden…").unwrap() < menu.find("Angemeldet:").unwrap(),
        "der Eintrag steht nach „Baruch beenden…“"
    );
    assert!(!menu.contains("Konten einrichten"), "mit Konten gibt es nichts mehr einzurichten");
    assert!(page.contains("<body data-rolle=\"autor\">"), "der body nennt die Rolle");
    for mark in ["BARUCH-KONTO", "NUR-LEKTOR", "NUR-KONTEN", "baruch-consent"] {
        assert!(!page.contains(mark), "die Marke {mark} steht nie in der ausgelieferten Seite");
    }
    assert!(!page.contains("/lektorat/act"), "der Autor bekommt keine Lektor-Formulare");
    for present in
        ["value=\"outline.save\"", ">Übernehmen</button>", ">Verwerfen</button>", "action=\"/assistenten/act\""]
    {
        assert!(page.contains(present), "der Autor behält alle Knöpfe: {present}");
    }
    assert!(
        page.contains("<div class=\"fb fb-kind-hint") && page.contains("Status setzen …"),
        "im Beitrag der Persona sieht der Autor den Befund als Karte mit den Status-Knöpfen"
    );
    assert!(
        tag_with(&page, &format!("/werke/{}/mitwirkende", desk.work)).contains("class=\"btn nur-autor\""),
        "mit Konten führt der Reiter Buch auf die Seite Mitwirkende"
    );
    // Origin of the Lektor in revision, revision tree and post – readable, without an empty model and not as a persona.
    let meta = &page[page.find("<span class=\"muted small rev-meta\">").unwrap()..];
    let meta = &meta[..meta.find("</nav>").unwrap()];
    assert!(
        meta.contains("· Vorschlag ·") && meta.contains(&format!(">{HANDLE}</span>")),
        "der Kopf der Fassung nennt die Herkunft der Lektorin: {meta}"
    );
    assert!(!page.contains(&format!("{HANDLE} ()")), "ein Lektor hat kein Modell – kein leeres „()“");
    assert!(!page.contains(&format!("Persona {HANDLE}")), "ein Lektor ist ein Mensch, keine Persona");
    assert!(page.contains(&format!("data-post-author=\"{HANDLE}\"")), "der Beitrag trägt die Herkunft");
}

#[tokio::test]
async fn the_local_author_gets_the_hint_and_the_way_to_sign_in() {
    let server = common::open().await;
    insert_account(&server, "joerg", "joerg", "author").await;
    let (status, _, page) = get_as(&server, "/werke", Who::LocalAuthor).await;
    assert_eq!(status, StatusCode::OK, "am eigenen Rechner ohne Anmeldung gelten Autorrechte");
    assert!(page.contains("Autor (eigener Rechner, ohne Anmeldung)"), "das Menü sagt, warum keine Anmeldung nötig war");
    assert!(page.contains("href=\"/anmelden\">Anmelden</a>"), "und bietet die Anmeldung an");
    assert!(page.contains("<body data-rolle=\"autor\">") && !page.contains("/abmelden"), "Autorrechte, keine Sitzung");
    let (_, _, konto) = get_as(&server, "/konto", Who::LocalAuthor).await;
    assert!(konto.contains("ohne Anmeldung") && !konto.contains("action=\"/konto/passwort\""), "Mein Konto erklärt es");
}

// ---- Work page of a Lektor ---------------------------------------------------------------------------------------

/// Words and methods that only the author may trigger: for Lektors they are not in the HTML at all.
const AUTHOR_ONLY: &[&str] = &[
    "outline.save",
    "revision.save",
    "revision.update",
    "revision.accept",
    "revision.reject",
    "revision.set_current",
    "revision.delete",
    "revision.fold_set",
    "revision.fold_clear",
    "persona.",
    "post.repeat",
    "post.move",
    "post.to_idea",
    "finding.set_status",
    "finding.decide",
    "solution.",
    ">Speichern</button>",
    ">Als neue Fassung",
    ">Übernehmen</button>",
    ">Verwerfen</button>",
    ">Löschen</button>",
    "aktuell machen",
    "ausblenden</button>",
    "Diese Fassung verwenden",
    "action=\"/act\"",
    "/assistenten/act",
    "/gliederung/act",
    "/beitraege/act",
    "/werke/befunde/act",
    "/loesungen/act",
    "/literatur",
    "class=\"literature-dialog\"",
    "id=\"work-import\"",
    "id=\"idea-capture\"",
    "data-widget=\"outline\"",
    "data-widget=\"assist\"",
    "id=\"band-tab-einfuegen\"",
    "id=\"band-panel-einfuegen\"",
    "data-band-part=\"einfuegen\"",
    "/einstellungen/assistenten",
    "/einstellungen/werk",
];

#[tokio::test]
async fn a_lektor_gets_propose_post_and_finding_but_no_author_button() {
    let desk = new_desk().await;
    let who = lektor_account();
    for (rev, preview) in [(None, false), (Some(desk.proposal), false), (None, true)] {
        let page = workspace_as(&desk, &who, rev, preview).await;
        let at = format!("Fassung {rev:?}, Vorschau {preview}");
        // The ribbon carries the author entries as nur-autor (konten.css hides them); everything else is missing.
        let body = &page[page.find("<main class=\"workspace\"").unwrap()..];
        for absent in AUTHOR_ONLY {
            // In the ribbon links of the author have the marker; in the work area and in the dialogs there is nothing
            // at all.
            let scope = if absent.starts_with('/') { body } else { page.as_str() };
            assert!(!scope.contains(absent), "{at}: für Lektoren fehlt im HTML, was nur der Autor darf: {absent}");
        }
        for form in tags(&page, "form") {
            if attr(form, "method").as_deref() == Some("post") {
                let action = attr(form, "action").unwrap_or_default();
                assert!(
                    action == "/lektorat/act" || action == "/abmelden" || action == "/einstellungen/sprache",
                    "{at}: jedes Formular eines Lektors geht an /lektorat/act (oder meldet ab): {form}"
                );
            }
        }
        assert!(page.contains("<body data-rolle=\"lektor\">"), "{at}: Rolle lektor am body");
        assert!(page.contains("Angemeldet: Erika Muster (Lektor)"), "{at}: Name und Rolle im Menü");
        assert!(page.contains(&format!("Lektor · {HANDLE}")), "{at}: der Kopf des Abschnitts nennt die Herkunft");
    }

    let page = workspace_as(&desk, &who, None, false).await;
    let propose = form_with(&page, "name=\"method\" value=\"revision.propose\"");
    assert!(
        propose.starts_with(
            "<form class=\"editor-form\" id=\"editor-form\" method=\"post\" action=\"/lektorat/act\" data-rpc"
        ),
        "der Text des Abschnitts geht als Vorschlag an /lektorat/act (ohne JavaScript) bzw. über /rpc (data-rpc): \
         {propose:.200}"
    );
    assert!(
        propose.contains(&format!("name=\"work\" value=\"{}\"", desk.work))
            && propose.contains(&format!("name=\"key\" value=\"{}\"", desk.key))
            && propose.contains("<textarea name=\"body\""),
        "Werk, fester Schlüssel und Text reisen mit (Parameter von revision.propose)"
    );
    assert!(
        !propose.contains("name=\"node\"") && !propose.contains("based_on"),
        "nur die Parameter des Vertrags – keine Stelle, kein based_on"
    );
    assert!(
        page.contains("form=\"editor-form\" data-saved=\"Vorschlagen\" data-dirty=\"Vorschlagen\"")
            && page.contains(">Vorschlagen</button>"),
        "der Knopf heißt „Vorschlagen“ und gehört zum Formular"
    );
    assert!(page.contains("name=\"note\" form=\"editor-form\""), "eine Notiz zum Vorschlag geht mit");

    let post = form_with(&page, "name=\"method\" value=\"post.add\"");
    assert!(
        post.starts_with("<form class=\"chat-input\" method=\"post\" action=\"/lektorat/act\" data-rpc>"),
        "Beitrag"
    );
    assert!(
        post.contains("name=\"body\"") && post.contains(&format!("name=\"key\" value=\"{}\"", desk.key)),
        "Text, Abschnitt"
    );

    let finding = form_with(&page, "name=\"method\" value=\"finding.add\"");
    assert!(finding.contains("action=\"/lektorat/act\""), "Befund über /lektorat/act");
    for field in
        ["name=\"kind\"", "value=\"hint\"", "value=\"critical\"", "value=\"praise\"", "name=\"body\"", "name=\"quote\""]
    {
        assert!(finding.contains(field), "Befund schreiben mit {field} (Parameter von finding.add)");
    }
    assert!(page.contains(">Befund schreiben</button>"), "der Knopf „Befund schreiben“");

    assert_eq!(
        page.matches("name=\"method\" value=\"post.edit\"").count(),
        1,
        "nur der eigene Beitrag (Autor = eigene Herkunft) lässt sich bearbeiten – nicht der des Autors, nicht der \
         der Persona"
    );
    assert_eq!(page.matches("name=\"method\" value=\"post.delete\"").count(), 1, "und nur er lässt sich entfernen");
    let edit = form_with(&page, "name=\"method\" value=\"post.edit\"");
    assert!(edit.contains("Der zweite Satz holpert."), "das Formular gehört zum Beitrag der Lektorin");
    for text in ["Bitte auf die Übergänge achten.", "Notiert.", "Hier fehlt ein Komma."] {
        assert!(page.contains(text), "lesen darf der Lektor alles im Abschnitt: {text}");
    }
    let chat = &page[page.find("class=\"chat chat-lektor\"").unwrap()..];
    assert!(
        chat.contains("class=\"post-finding\"") && chat.contains("Das Bild der Rosen kommt zu früh."),
        "den Befund im Beitrag der Persona liest der Lektor als Text des Beitrags"
    );
    assert!(
        !chat.contains("class=\"fb ") && !chat.contains("Status setzen"),
        "ohne die Karte mit den Status-Knöpfen (die Formulare kann nur der Autor nutzen)"
    );
    assert!(page.contains("data-widget=\"wysiwyg\""), "das Satzbild bleibt (nur lesende RPC-Methoden)");

    // Ribbon: what only the author can use carries the marker.
    for href in ["/werke/neu", "/werke/import/quelle", "/werke/import/word", "/austausch/einlesen", "/beenden"] {
        let tag = tag_with(&page, &format!("href=\"{href}"));
        assert!(tag.contains("class=\"menu-item nur-autor\""), "Menü „Baruch ▾“: {href} nur für den Autor: {tag}");
    }
    for label in ["Einfälle", "Quellen", "Satz", "Ausgabe", "Umschlag"] {
        let tag = tag_with(&page, &format!("<summary class=\"band-group-label\">{label}</summary>"));
        assert!(tag.contains(" data-nur-autor"), "Gruppe „{label}“ nur für den Autor: {tag}");
    }
    for href in ["/bearbeiten\"", "/nummerierung\"", "/bilder\"", "/mitwirkende\"", "/pruefung"] {
        let tag = tag_with(&page, href);
        assert!(tag.contains("class=\"btn nur-autor\""), "Reiter: {href} nur für den Autor: {tag}");
    }
    let settings = tag_with(&page, "data-settings-open");
    assert_eq!(
        attr(settings, "href").as_deref(),
        Some(format!("/einstellungen/darstellung?work={}", desk.work).as_str()),
        "von den Einstellungen bleibt dem Lektor die Darstellung (Cookie im eigenen Browser)"
    );
}

#[tokio::test]
async fn the_panes_of_a_lektor_come_from_the_lektor_templates() {
    let desk = new_desk().await;
    let lektor = panes_as(&desk, lektor_account()).await;
    assert!(
        lektor.editor.contains("value=\"revision.propose\""),
        "ui.render zeichnet für Lektoren den Vorschlags-Editor"
    );
    assert!(!lektor.tree.contains("<form") && !lektor.tree.contains("draggable"), "die Gliederung ist nur zum Lesen");
    assert!(lektor.tree.contains("data-nav"), "Springen in der Gliederung bleibt");
    assert!(lektor.assistants.contains("value=\"post.add\""), "rechts der Beitrag über /lektorat/act");
    for pane in [&lektor.tree, &lektor.editor, &lektor.assistants] {
        for absent in AUTHOR_ONLY {
            assert!(!pane.contains(absent), "auch nach dem Neuzeichnen fehlt, was nur der Autor darf: {absent}");
        }
    }
    let author = panes_as(&desk, author_account()).await;
    let open = panes_as(&desk, Who::Open).await;
    assert_eq!(author.editor, open.editor, "der Autor mit Konto bekommt denselben Editor wie ohne Konten");
    assert_eq!(author.assistants, open.assistants, "und dieselbe rechte Spalte");
    assert!(open.editor.contains("value=\"outline.save\"") && !open.editor.contains("/lektorat/act"), "wie bisher");
}

// ---- Remaining pages: marker and CSS -----------------------------------------------------------------------------

#[tokio::test]
async fn the_stylesheet_hides_author_things_for_lektors_without_javascript() {
    let server = common::open().await;
    let (status, css) = server.get("/static/app.css").await;
    assert_eq!(status, StatusCode::OK, "das Stylesheet lädt");
    let flat: String = css.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains(
            "body[data-rolle=\"lektor\"] .nur-autor, body[data-rolle=\"lektor\"] [data-nur-autor] { display: none !important; }"
        ),
        "für Lektoren verschwindet, was nur der Autor nutzen kann – reines CSS, wirkt ohne JavaScript"
    );
    assert!(
        flat.contains("body:not([data-rolle=\"lektor\"]) .nur-lektor { display: none !important; }"),
        "was nur ein Lektor braucht, ist nur für body[data-rolle=\"lektor\"] sichtbar"
    );
    let rule = flat
        .split('}')
        .find(|rule| rule.contains("body[data-rolle=\"lektor\"] form[method=\"post\"]"))
        .expect("Regel für Formulare");
    for allowed in ["/lektorat/act", "/abmelden", "/konto/passwort", "/einstellungen/darstellung/act"] {
        assert!(
            rule.contains(&format!(":not([action=\"{allowed}\"])")),
            "alles verboten, was nicht erlaubt ist: sichtbar bleiben für Lektoren nur Formulare an {allowed} (die \
             POST-Routen aus LEKTOR_ROUTES außer /rpc)"
        );
    }
    assert!(rule.contains("display: none !important"), "alle anderen POST-Formulare sind für Lektoren verborgen");

    let own = include_str!("../static/konten.css");
    assert!(css.contains(own.trim()), "konten.css ist Teil von /static/app.css");
    let code: String = own.split("/*").map(|part| part.split_once("*/").map_or(part, |(_, rest)| rest)).collect();
    assert!(
        !code.contains('#') && !code.contains("rgb") && !code.contains("hsl"),
        "keine festen Farben: nur die Variablen von app.css – so stimmen hell, dunkel und Laptop-Modus von selbst"
    );
    assert!(!code.contains("@media"), "keine eigenen Umbrüche neben dem Laptop-Modus");
    for state in ["data-rail", "data-narrow", "data-compact", "data-laptop", "#pane-"] {
        assert!(
            !code.contains(state),
            "konten.css steht hinter laptop.css und fasst dessen Zustände nicht an ({state}) – sonst überschriebe es              den Laptop-Modus"
        );
    }
}

#[tokio::test]
async fn the_findings_page_marks_status_forms_for_the_author_and_offers_the_lektor_form() {
    let desk = new_desk().await;
    insert_account(&desk.server, "joerg", "joerg", "author").await;
    let uri = format!("/werke/{}/befunde?key={}", desk.work, desk.key);

    let (status, page) = page_as(&desk.server, baruch_server::findings::page::pages(), &uri, lektor_account()).await;
    assert_eq!(status, StatusCode::OK, "die Befundseite zeichnet sich für einen Lektor");
    let form = form_with(&page, "name=\"method\" value=\"finding.add\"");
    assert!(
        form.starts_with("<form class=\"fd-form lektor-finding nur-lektor\" method=\"post\" action=\"/lektorat/act\""),
        "das Formular „Befund schreiben“ ist nur für Lektoren und geht an /lektorat/act: {form:.160}"
    );
    assert!(form.contains("name=\"back\" value=\"befunde\""), "danach geht es zurück auf die Befundseite");
    assert!(!form.contains("name=\"key\""), "ohne Abschnitt: eine Anmerkung zum Buch");
    assert!(
        form.contains("Befund schreiben: Anmerkung zum Buch")
            && form.contains(">Anmerkung zum Buch schreiben</button>"),
        "Überschrift und Knopf sagen, was entsteht"
    );
    assert!(page.contains("<body data-rolle=\"lektor\">"), "Rolle am body – erst sie macht nur-lektor sichtbar");
    assert!(page.contains(&format!("<span class=\"fd-persona\">{HANDLE}</span>")), "der Befund nennt die Herkunft");
    let forms: Vec<&str> = tags(&page, "form").into_iter().filter(|t| t.contains("/werke/befunde/act")).collect();
    assert!(!forms.is_empty(), "die Karte hat Formulare zum Status");
    assert!(
        page.contains("<details class=\"fd-forms nur-autor\""),
        "die Status-Knöpfe („Status setzen …“) tragen nur-autor und sind damit für Lektoren verborgen"
    );
    assert!(!page.contains("NUR-LEKTOR"), "die Klammern um das Lektor-Stück stehen nie in der Seite");

    let (_, _, author) = get_as(&desk.server, &uri, author_account()).await;
    assert!(!author.contains("/lektorat/act"), "der Autor bekommt das Lektor-Formular gar nicht erst");
    assert!(
        author.contains("<details class=\"fd-forms nur-autor\""),
        "seine Status-Knöpfe bleiben (nur gekennzeichnet)"
    );

    // Without accounts (own data directory): the findings page as before, without marker and without Lektor form.
    let plain = new_desk().await;
    let (_, open) = plain.server.get(&format!("/werke/{}/befunde?key={}", plain.work, plain.key)).await;
    assert!(
        open.contains("<details class=\"fd-forms\"") && !open.contains("/lektorat/act") && !open.contains("nur-"),
        "ohne Konten ist die Befundseite wie bisher"
    );
}

#[tokio::test]
async fn the_work_list_of_a_lektor_has_no_author_buttons_and_says_when_nothing_is_shared() {
    let desk = new_desk().await;
    insert_account(&desk.server, "joerg", "joerg", "author").await;
    insert_account(&desk.server, "Erika", "erika", "lektor").await;
    // No grant in work_member: the Lektor sees no work.
    let (status, page) = page_as(&desk.server, baruch_server::works::pages(), "/werke", lektor_account()).await;
    assert_eq!(status, StatusCode::OK, "die Werkliste zeichnet sich für einen Lektor");
    assert!(
        page.contains("<p class=\"muted nur-lektor\">Noch kein Werk freigegeben."),
        "ohne Freigabe sagt die Liste dem Lektor, woran es liegt"
    );
    assert!(
        page.contains("<p class=\"muted nur-autor\">Noch kein Werk. Lege eins an"),
        "der Satz für den Autor trägt nur-autor"
    );
    assert!(
        tag_with(&page, "<summary class=\"band-group-label\">Werke</summary>").contains(" data-nur-autor"),
        "die Gruppe mit „Neues Werk oder Import…“ und „Archiv“ ist nur für den Autor"
    );

    let (_, _, author) = get_as(&desk.server, "/werke", author_account()).await;
    for href in ["/bearbeiten\">Ändern", "/export\" download", "/pdf\" download", "/loeschen\">Löschen"] {
        assert!(
            tag_with(&author, href).contains("nur-autor\""),
            "Werkliste: {href} trägt nur-autor (Ändern, Export, PDF, Löschen sind dem Autor vorbehalten)"
        );
    }
    assert!(!author.contains("Noch kein Werk freigegeben."), "der Satz für Lektoren fehlt beim Autor ganz");
}

#[tokio::test]
async fn a_lektor_only_finds_the_display_page_in_the_settings() {
    let desk = new_desk().await;
    insert_account(&desk.server, "joerg", "joerg", "author").await;
    let uri = format!("/einstellungen/darstellung?work={}", desk.work);
    let (status, page) = page_as(&desk.server, baruch_server::settings::pages(), &uri, lektor_account()).await;
    assert_eq!(status, StatusCode::OK, "Darstellung zeichnet sich für einen Lektor");
    let nav = &page[page.find("<nav class=\"settings-nav\"").unwrap()..];
    let nav = &nav[..nav.find("</nav>").unwrap()];
    assert_eq!(nav.matches("data-settings-page=").count(), 1, "in der Navigation steht nur eine Seite: {nav}");
    assert!(nav.contains("data-settings-page=\"darstellung\""), "nämlich Darstellung (Cookies im eigenen Browser)");
    assert!(page.contains("action=\"/einstellungen/darstellung/act\""), "ihr Formular bleibt");

    let (_, _, author) = get_as(&desk.server, &uri, author_account()).await;
    let nav = &author[author.find("<nav class=\"settings-nav\"").unwrap()..];
    let nav = &nav[..nav.find("</nav>").unwrap()];
    assert_eq!(nav.matches("data-settings-page=").count(), 8, "der Autor sieht alle acht Seiten: {nav}");
    assert!(
        nav.rfind("data-settings-page=\"konten\"").unwrap() > nav.rfind("data-settings-page=\"daten\"").unwrap(),
        "Konten ist der achte und letzte Eintrag: {nav}"
    );
}

// ---- Consent (notice bar and dialog) -------------------------------------------------------------------------------

/// An MCP client requests a part under which existing chapters would move: a request that needs consent.
async fn request_needing_consent(server: &common::Server) -> i64 {
    let report = server
        .import("= Saat\n\nKörner im Boden.\n\n= Wuchs\n\nHalme im Wind.\n\n= Ernte\n\nGarben am Weg.\n", "feld.typ")
        .await;
    let loaded = server.state.store.load_work(report.work).await.unwrap();
    let key = loaded.nodes.iter().find(|n| n.title == "Wuchs").unwrap().key;
    let post = |session: Option<String>, message: Value| {
        let mut build = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
        if let Some(id) = session {
            build = build.header("mcp-session-id", id);
        }
        let mut request = build.body(Body::from(message.to_string())).unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 42000))));
        request
    };
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "Feldbuch", "version": "1.0" } } });
    let (status, headers, body) = server.send(post(None, init)).await;
    assert_eq!(status, StatusCode::OK, "MCP initialize: {body}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "outline_add",
        "arguments": { "work": report.work, "key": key, "place": "before", "title": "Vom Wachsen", "level": 0 } } });
    let (_, _, body) = server.send(post(Some(session), call)).await;
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["result"]["structuredContent"]["action"], "requested", "ein Antrag, kein Anlegen: {answer}");
    report.work
}

#[tokio::test]
async fn the_consent_bar_and_dialog_are_only_for_full_rights() {
    let server = common::open().await;
    request_needing_consent(&server).await;
    let (_, open) = server.get("/werke").await;
    assert!(
        open.contains("class=\"consent-bar\"") && open.contains("data-widget=\"consent\""),
        "ohne Konten zeigt jede Seite Hinweisleiste und Dialog wie bisher"
    );
    insert_account(&server, "joerg", "joerg", "author").await;
    let (_, _, author) = get_as(&server, "/werke", author_account()).await;
    assert!(author.contains("class=\"consent-bar\""), "der Autor sieht den offenen Antrag weiter");
    let (status, lektor) = page_as(&server, baruch_server::works::pages(), "/werke", lektor_account()).await;
    assert_eq!(status, StatusCode::OK, "die Seite zeichnet sich für einen Lektor");
    assert!(
        !lektor.contains("consent-bar")
            && !lektor.contains("consent-dialog")
            && !lektor.contains("data-widget=\"consent\""),
        "zustimmen kann nur der Autor – ein Lektor bekommt weder Hinweisleiste noch Dialog"
    );
    assert!(!lektor.contains("<!--baruch-consent-->"), "die Marke verschwindet trotzdem");
}

// ---- Einstellungen → Konten (pure presentation, invented data) --------------------------------------------------------

fn sample_accounts() -> AccountsView {
    AccountsView {
        work: Some(4),
        accounts_exist: true,
        allowed: true,
        accounts: vec![
            AccountRow {
                id: 1,
                name: "joerg".into(),
                display: "Jörg Beispiel".into(),
                kind: "author".into(),
                kind_label: "Autor",
                handle: None,
                active: true,
                created: "2026-11-20 08:00 UTC".into(),
                last_login: Some("2026-11-26 07:30 UTC".into()),
                own: true,
            },
            AccountRow {
                id: 2,
                name: NASTY.into(),
                display: NASTY.into(),
                kind: "lektor".into(),
                kind_label: "Lektor",
                handle: Some("@lektorat:o-neil-soehne".into()),
                active: true,
                created: "2026-11-21 09:00 UTC".into(),
                last_login: None,
                own: false,
            },
            AccountRow {
                id: 3,
                name: "Paul".into(),
                display: String::new(),
                kind: "lektor".into(),
                kind_label: "Lektor",
                handle: Some("@lektorat:paul".into()),
                active: false,
                created: "2026-11-22 10:00 UTC".into(),
                last_login: Some("2026-11-23 11:11 UTC".into()),
                own: false,
            },
        ],
        local_open: false,
        notice: Some("Das Konto ist angelegt.".into()),
        error: None,
        load_error: None,
    }
}

#[test]
fn the_accounts_page_lists_accounts_and_offers_every_action_of_the_contract() {
    let page = accounts_html(&sample_accounts()).expect("Seite Konten");
    for head in ["Name", "Anzeigename", "Art", "Herkunft", "Aktiv", "Letzte Anmeldung"] {
        assert!(page.contains(&format!("<th scope=\"col\">{head}</th>")), "Spalte {head}");
    }
    assert_eq!(page.matches("<tr data-account=").count(), 3, "drei Konten, drei Zeilen");
    assert!(page.contains(NASTY_ESCAPED) && !page.contains("<b>"), "Namen sind maskiert, nichts wird Markup");
    assert!(page.contains("<code>@lektorat:o-neil-soehne</code>"), "die Herkunft eines Lektors steht da");
    assert!(page.contains("2026-11-26 07:30 UTC") && page.contains("noch nie"), "letzte Anmeldung bzw. „noch nie“");
    assert!(page.contains("<tr data-account=\"3\" class=\"konto-gesperrt\">"), "ein gesperrtes Konto ist erkennbar");
    assert!(page.contains("<span class=\"badge warning\">gesperrt</span>"), "als Text, nicht nur als Farbe");
    assert!(page.contains("role=\"status\">Das Konto ist angelegt.</p>"), "die Meldung nach einer Aktion");

    let posts = tags(&page, "form");
    assert!(!posts.is_empty(), "die Seite besteht aus Formularen");
    for form in &posts {
        assert_eq!(attr(form, "method").as_deref(), Some("post"), "jedes Formular schreibt: {form}");
        assert_eq!(
            attr(form, "action").as_deref(),
            Some("/einstellungen/konten/act"),
            "alle Formulare gehen an POST /einstellungen/konten/act (Vertrag §1.5): {form}"
        );
    }
    let actions = |value: &str| page.matches(&format!("name=\"action\" value=\"{value}\"")).count();
    assert_eq!(actions("create"), 1, "ein Formular legt einen Lektor an");
    assert_eq!(actions("deactivate"), 2, "die zwei aktiven Konten lassen sich sperren");
    assert_eq!(actions("activate"), 1, "das gesperrte lässt sich entsperren");
    assert_eq!(actions("reset_password"), 3, "für jedes Konto lässt sich das Passwort zurücksetzen");
    assert_eq!(actions("local_open"), 1, "der Schalter „Am eigenen Rechner ohne Anmeldung“");
    let create = form_with(&page, "name=\"action\" value=\"create\"");
    for field in ["name=\"name\"", "name=\"display\"", "name=\"password\"", "name=\"password2\""] {
        assert!(create.contains(field), "Lektor anlegen mit {field}");
    }
    assert!(!create.contains("name=\"kind\""), "die Art steht nicht zur Wahl: ein neues Konto ist immer ein Lektor");
    let reset = form_with(&page, "name=\"action\" value=\"reset_password\"");
    assert!(reset.contains("name=\"id\" value=\"1\"") && reset.contains("name=\"password2\""), "Zurücksetzen mit id");
    for field in tags(&page, "input").into_iter().filter(|t| t.contains("type=\"password\"")) {
        assert_eq!(attr(field, "autocomplete").as_deref(), Some("new-password"), "neues Passwort: {field}");
        assert!(!field.contains(" value="), "Passwortfelder sind nie vorbelegt: {field}");
    }
    let switch = form_with(&page, "name=\"action\" value=\"local_open\"");
    assert!(switch.contains("name=\"open\" value=\"1\""), "ausgeschaltet bietet die Seite das Einschalten an (open=1)");
    assert!(page.contains("data-local-open=\"0\""), "der Stand steht als Text da");
    assert!(
        page.contains("role=\"note\"><strong>Vorsicht mit einem Proxy auf demselben Rechner:</strong>"),
        "die Warnung zum Proxy steht beim Schalter"
    );
    assert!(page.contains("href=\"/werke/4/mitwirkende\""), "Verweis auf die Mitwirkenden des Werks");
    assert_plain_forms(&page, "Einstellungen → Konten");

    let on = accounts_html(&AccountsView { local_open: true, ..sample_accounts() }).unwrap();
    let switch = form_with(&on, "name=\"action\" value=\"local_open\"");
    assert!(switch.contains("name=\"open\" value=\"0\""), "eingeschaltet bietet die Seite das Ausschalten an (open=0)");
    assert!(on.contains("data-local-open=\"1\""), "und nennt den Stand");

    let failed = accounts_html(&AccountsView {
        accounts: Vec::new(),
        load_error: Some("Datenbank gesperrt".into()),
        ..sample_accounts()
    })
    .unwrap();
    assert!(
        failed.contains("Die Konten lassen sich nicht lesen: Datenbank gesperrt"),
        "scheitert das Lesen, sagt die Seite es, statt eine leere Liste vorzutäuschen"
    );

    let denied = accounts_html(&AccountsView { allowed: false, ..sample_accounts() }).unwrap();
    assert!(
        denied.contains("Konten verwaltet nur der Autor.") && !denied.contains("<form") && !denied.contains("joerg"),
        "ohne volle Rechte zeigt die Seite weder Liste noch Formulare"
    );
}

// ---- Mitwirkende (pure presentation, invented data) -------------------------------------------------------------------

#[tokio::test]
async fn the_members_page_lists_shares_and_offers_grant_and_revoke() {
    let mut view =
        MembersView { work: 4, title: "Das Feldbuch".into(), accounts_exist: true, ..MembersView::default() };
    view.fill(&json!({
        "work": 4,
        "members": [
            { "account": 2, "name": NASTY, "display": NASTY, "handle": "@lektorat:o-neil-soehne", "role": "lektor",
              "granted": "2026-11-25T14:03:09.120Z", "active": true },
            { "account": 3, "name": "Paul", "display": "", "handle": "@lektorat:paul", "role": "lektor",
              "granted": "2026-11-24T09:00:00.000Z", "active": false }
        ],
        "candidates": [ { "account": 5, "name": "Mara", "display": "Mara Feld", "handle": "@lektorat:mara" } ]
    }));
    assert!(view.load_error.is_none(), "die Antwort von member.list in der Form des Vertrags ist lesbar");
    view.notice = Some("Das Werk ist für das Konto freigegeben.".into());
    let page = members_html(&view, &Settings::from_cookie_header(None)).expect("Seite Mitwirkende");
    assert!(page.contains("Mitwirkende an „Das Feldbuch“"), "die Seite nennt das Werk");
    assert_eq!(page.matches("<tr data-member=").count(), 2, "zwei Freigaben, zwei Zeilen");
    assert!(page.contains(NASTY_ESCAPED) && !page.contains("<b>"), "Namen sind maskiert");
    assert!(page.contains("2026-11-25 14:03 UTC"), "der Zeitpunkt der Freigabe steht kurz und lesbar da");
    assert!(page.contains("gesperrt</span>"), "ein gesperrtes Konto ist als solches erkennbar");
    // Round 39a: the language switch of the menu is not a form of this page.
    let acts: Vec<&str> = tags(&page, "form")
        .into_iter()
        .filter(|t| t.contains("method=\"post\"") && !t.contains("lang-switch"))
        .collect();
    assert_eq!(
        acts.len(),
        3,
        "zweimal Entziehen, einmal Freigeben (das Abmelden im Menü setzt erst konten_ui::apply ein)"
    );
    for form in &acts {
        assert_eq!(
            attr(form, "action").as_deref(),
            Some("/werke/4/mitwirkende/act"),
            "Freigeben und Entziehen gehen an POST /werke/{{id}}/mitwirkende/act: {form}"
        );
    }
    let revoke = form_with(&page, "name=\"action\" value=\"revoke\"");
    assert!(revoke.contains("name=\"account\" value=\"2\"") && revoke.contains(">Entziehen</button>"), "Entziehen");
    let grant = form_with(&page, "name=\"action\" value=\"grant\"");
    assert!(
        grant.contains("<select name=\"account\" required>")
            && grant.contains("<option value=\"5\">Mara – Mara Feld (@lektorat:mara)</option>"),
        "Freigeben wählt unter den Lektoren ohne Freigabe: {grant}"
    );
    assert!(page.contains("role=\"status\">Das Werk ist für das Konto freigegeben.</p>"), "Meldung nach der Aktion");
    assert!(page.contains("data-band-default=\"buch\""), "die Seite gehört zum Reiter Buch");

    let mut broken =
        MembersView { work: 4, title: "Das Feldbuch".into(), accounts_exist: true, ..MembersView::default() };
    broken.fill(&json!({ "members": "kaputt" }));
    let page = members_html(&broken, &Settings::from_cookie_header(None)).unwrap();
    assert!(
        page.contains("Die Freigaben lassen sich nicht lesen") && !page.contains("/mitwirkende/act"),
        "eine unlesbare Antwort wird als Fehler gezeigt, nicht als leere Liste"
    );

    let server = common::open().await;
    insert_account(&server, "joerg", "joerg", "author").await;
    let work = server.state.default_work().await.unwrap();
    let uri = format!("/werke/{work}/mitwirkende?ok=entzogen&fehler=%3Cscript%3Ealert(1)%3C%2Fscript%3E");
    let (status, _, page) = get_as(&server, &uri, author_account()).await;
    assert_eq!(status, StatusCode::OK, "der Autor erreicht die Seite");
    assert!(page.contains("role=\"status\">Die Freigabe ist entzogen.</p>"), "?ok=entzogen zeigt den Satz der Tabelle");
    // The ribbon carries the query of the address on, percent-encoded, in the addresses of its tabs (band::apply);
    // nothing of it is in the content of the page.
    assert!(
        !main_of(&page).contains("alert(1)"),
        "ein unbekannter Code zeigt nichts – kein Text aus der Adresse im Inhalt"
    );
    assert!(!page.contains("<script>alert"), "und nirgends in der Seite wird die Adresse zu Markup");
    let (status, page) =
        page_as(&server, konten_ui::pages(), &format!("/werke/{work}/mitwirkende"), lektor_account()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "Freigaben verwaltet nur der Autor – auch wenn die Route offen wäre");
    assert!(page.starts_with("Abgelehnt:"), "mit einer klaren Meldung: {page}");
}

// ---- Mein Konto ---------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn my_account_shows_role_handle_works_password_form_and_logout() {
    let Who::Account(identity) = lektor_account() else { unreachable!() };
    let mut view = AccountView::of(&identity);
    view.works = vec![AccountWork { id: 4, title: NASTY.into() }];
    let page = account_html(&view, &Settings::from_cookie_header(None)).expect("Mein Konto");
    assert!(page.contains("<dd data-konto-name>Erika</dd>"), "Name des Kontos");
    assert!(page.contains("<dd data-konto-rolle>Lektor</dd>"), "Rolle");
    assert!(page.contains(&format!("<code>{HANDLE}</code>")), "die Herkunft, unter der ein Lektor schreibt");
    assert!(page.contains(&format!("\">{NASTY_ESCAPED}</a>")), "freigegebene Werke, maskiert");
    assert!(page.contains("Freigegebene Werke"), "Überschrift für Lektoren");
    let form = form_with(&page, "name=\"current\"");
    assert!(
        form.starts_with("<form class=\"konto-form\" method=\"post\" action=\"/konto/passwort\">"),
        "POST /konto/passwort"
    );
    assert_password_fields(
        &page,
        &[("current", "current-password"), ("password", "new-password"), ("password2", "new-password")],
        "Mein Konto",
    );
    assert!(page.contains("<form class=\"konto-form\" method=\"post\" action=\"/abmelden\">"), "Abmelden als Formular");
    assert_plain_forms(main_of(&page), "Mein Konto");

    let empty = account_html(&AccountView::of(&identity), &Settings::from_cookie_header(None)).unwrap();
    assert!(empty.contains("Noch kein Werk freigegeben."), "ohne Freigabe sagt die Seite, woran es liegt");

    let Who::Account(author) = author_account() else { unreachable!() };
    let page = account_html(&AccountView::of(&author), &Settings::from_cookie_header(None)).unwrap();
    assert!(
        page.contains("<dd data-konto-rolle>Autor</dd>") && !page.contains("@lektorat:"),
        "der Autor hat keine Herkunft"
    );

    let server = common::open().await;
    insert_account(&server, "joerg", "joerg", "author").await;
    let (status, headers, page) =
        get_as(&server, "/konto?ok=passwort_geaendert&fehler=%3Cb%3Efrech%3C%2Fb%3E", author_account()).await;
    assert_eq!(status, StatusCode::OK, "Mein Konto lädt für ein angemeldetes Konto");
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store", "die Seite gehört in keinen Zwischenspeicher");
    assert!(page.contains("role=\"status\">Das Passwort ist geändert."), "?ok=passwort_geaendert zeigt den Satz");
    assert!(
        !main_of(&page).contains("frech"),
        "ein unbekannter Code zeigt nichts – kein Text aus der Adresse im Inhalt"
    );
    assert!(!page.contains("<b>frech"), "und nirgends in der Seite wird die Adresse zu Markup");
    assert!(page.contains("<dd data-konto-name>joerg</dd>"), "die Seite zeigt das Konto der Anfrage");
    let (_, _, page) = get_as(&server, "/konto?fehler=passwort_ungleich", author_account()).await;
    assert!(page.contains("role=\"alert\">Die beiden Passwörter stimmen nicht überein.</p>"), "?fehler= als Alarm");
}

// ---- Templates --------------------------------------------------------------------------------------------------------

#[test]
fn the_new_templates_are_plain_forms_and_links() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    for file in ["anmelden.html", "einrichten.html", "konto.html", "mitwirkende.html", "settings/konten.html"] {
        let source = std::fs::read_to_string(dir.join(file)).unwrap();
        assert!(!source.contains("<script"), "{file}: die Seite lädt kein eigenes Skript");
        assert!(
            !source.contains("data-widget") && !source.contains("type=\"button\""),
            "{file}: nur Formulare und Links"
        );
        assert!(!source.contains("style=\""), "{file}: keine Farben oder Maße im Markup – alles aus konten.css");
        for field in
            source.match_indices("type=\"password\"").map(|(at, _)| &source[source[..at].rfind('<').unwrap()..])
        {
            let field = &field[..field.find('>').unwrap()];
            assert!(field.contains("autocomplete=\"") && !field.contains("value="), "{file}: Passwortfeld {field}");
        }
    }
    for file in ["fragments/lektor_tree.html", "fragments/lektor_editor.html", "fragments/lektor_assistants.html"] {
        let source = std::fs::read_to_string(dir.join(file)).unwrap();
        for action in source.match_indices("action=\"").map(|(at, _)| &source[at + 8..]) {
            assert!(
                action.starts_with("/lektorat/act\""),
                "{file}: jedes Formular eines Lektors geht an /lektorat/act: {:.40}",
                action
            );
        }
    }
}
