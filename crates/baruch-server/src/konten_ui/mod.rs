// SPDX-License-Identifier: AGPL-3.0-or-later
//! Oberfläche der Konten (Runde 35, Vertrag §S1.4 und §3): Anmeldeseite, Einrichten des ersten Kontos, „Mein
//! Konto“, Mitwirkende je Werk und der Konto-Eintrag im Menü „Baruch ▾“.
//!
//! [`apply`] läuft in `consent::inject` nach `band::apply` über jede Seite, die `base.html` erweitert, und setzt
//! an der Marke [`KONTO_MARK`] den Eintrag zum Konto ein. Meldungen aus der Adresse (`?ok=`, `?fehler=`) zeigen die
//! Seiten nur über [`MESSAGES`] – nie Text aus der Adresse.
//!
//! Schnittstelle R35-S1: schlichte, funktionierende Formulare und [`safe_next`]; Gestaltung, Seiten und Menü setzt
//! „oberflaeche_konten“ um.

use std::sync::Arc;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::auth::Who;

/// Anmeldeseite: vorbelegter Name (nie das Passwort), Ziel nach der Anmeldung, Fehlermeldung und – während der
/// Sperre gegen Durchprobieren – die Wartezeit in Sekunden.
pub struct LoginView {
    pub name: String,
    pub next: String,
    pub error: Option<String>,
    pub retry_after: Option<u64>,
}

/// Seite „Einrichten“ (erstes Konto, der Autor). `allowed` = die Anfrage kommt vom eigenen Rechner; sonst zeigt die
/// Seite nur die Erklärung.
pub struct SetupView {
    pub name: String,
    pub display: String,
    pub error: Option<String>,
    pub allowed: bool,
}

/// Marke in `base.html` (Menü „Baruch ▾“ nach „Baruch beenden…“), an der [`apply`] den Konto-Eintrag einsetzt.
pub const KONTO_MARK: &str = "<!--BARUCH-KONTO-->";

/// Meldungscodes der Adressen `?ok=<code>` und `?fehler=<code>` mit ihrem deutschen Satz (Vertrag §3.4).
// STUB: deutsche Sätze statt der Codes (Runde 35)
pub const MESSAGES: &[(&str, &str)] = &[
    ("konto_angelegt", "konto_angelegt"),
    ("passwort_geaendert", "passwort_geaendert"),
    ("konto_gesperrt", "konto_gesperrt"),
    ("konto_entsperrt", "konto_entsperrt"),
    ("lokal_offen", "lokal_offen"),
    ("lokal_anmelden", "lokal_anmelden"),
    ("freigegeben", "freigegeben"),
    ("entzogen", "entzogen"),
    ("abgemeldet", "abgemeldet"),
    ("passwort_falsch", "passwort_falsch"),
    ("passwort_kurz", "passwort_kurz"),
    ("passwort_lang", "passwort_lang"),
    ("passwort_ungleich", "passwort_ungleich"),
    ("name_vergeben", "name_vergeben"),
    ("name_ungueltig", "name_ungueltig"),
    ("letzter_autor", "letzter_autor"),
    ("unbekanntes_konto", "unbekanntes_konto"),
    ("kein_lektor", "kein_lektor"),
    ("nicht_eigener_rechner", "nicht_eigener_rechner"),
];

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn plain_page(status: StatusCode, title: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html>\n<html lang=\"de\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title} – Baruch</title>\n\
         <link rel=\"icon\" href=\"/static/favicon.svg\">\n<link rel=\"stylesheet\" href=\"/static/app.css\">\n\
         </head>\n<body class=\"konto-seite\">\n<main>\n<h1>{title}</h1>\n{body}</main>\n</body>\n</html>\n"
    );
    (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], html)
        .into_response()
}

fn error_line(error: Option<&str>) -> String {
    error.map_or_else(String::new, |e| format!("<p class=\"fehler\" role=\"alert\">{}</p>\n", escape(e)))
}

/// Anmeldeseite mit dem Formular `POST /anmelden` (Felder `name`, `password`, `next`). Das Passwortfeld ist nie
/// vorbelegt.
pub fn login_page(status: StatusCode, view: &LoginView) -> Response {
    // STUB: Gestaltung über Vorlage `templates/…`, Hinweis „Schon angemeldet? Weiter“ (Runde 35)
    let mut body = error_line(view.error.as_deref());
    if let Some(seconds) = view.retry_after {
        body.push_str(&format!(
            "<p class=\"hinweis\">Bitte {seconds} Sekunden warten und dann erneut versuchen.</p>\n"
        ));
    }
    body.push_str(&format!(
        "<form method=\"post\" action=\"/anmelden\">\n<input type=\"hidden\" name=\"next\" value=\"{next}\">\n\
         <p><label>Name <input type=\"text\" name=\"name\" value=\"{name}\" autocomplete=\"username\" required \
         autofocus></label></p>\n\
         <p><label>Passwort <input type=\"password\" name=\"password\" autocomplete=\"current-password\" \
         required></label></p>\n<p><button type=\"submit\">Anmelden</button></p>\n</form>\n",
        next = escape(safe_next(&view.next)),
        name = escape(&view.name),
    ));
    plain_page(status, "Anmelden", &body)
}

/// Seite „Einrichten“ mit dem Formular `POST /einrichten` (Felder `name`, `display`, `password`, `password2`); ohne
/// [`SetupView::allowed`] nur die Erklärung. Passwortfelder sind nie vorbelegt.
pub fn setup_page(status: StatusCode, view: &SetupView) -> Response {
    // STUB: Gestaltung über Vorlage `templates/…` (Runde 35)
    let mut body = error_line(view.error.as_deref());
    if view.allowed {
        body.push_str(&format!(
            "<p>Das erste Konto gehört dem Autor. Danach verlangt Baruch eine Anmeldung.</p>\n\
             <form method=\"post\" action=\"/einrichten\">\n\
             <p><label>Name <input type=\"text\" name=\"name\" value=\"{name}\" maxlength=\"40\" \
             autocomplete=\"username\" required autofocus></label></p>\n\
             <p><label>Anzeigename <input type=\"text\" name=\"display\" value=\"{display}\" maxlength=\"80\" \
             autocomplete=\"name\"></label></p>\n\
             <p><label>Passwort (mindestens {min} Zeichen) <input type=\"password\" name=\"password\" \
             autocomplete=\"new-password\" minlength=\"{min}\" maxlength=\"{max}\" required></label></p>\n\
             <p><label>Passwort wiederholen <input type=\"password\" name=\"password2\" \
             autocomplete=\"new-password\" minlength=\"{min}\" maxlength=\"{max}\" required></label></p>\n\
             <p><button type=\"submit\">Konto anlegen</button></p>\n</form>\n",
            name = escape(&view.name),
            display = escape(&view.display),
            min = crate::auth::password::MIN_PASSWORD_CHARS,
            max = crate::auth::password::MAX_PASSWORD_CHARS,
        ));
    } else {
        body.push_str(
            "<p>Das erste Konto lässt sich nur am eigenen Rechner einrichten (Baruch direkt im Browser dieses \
             Rechners öffnen, nicht über einen Proxy).</p>\n",
        );
    }
    plain_page(status, "Konten einrichten", &body)
}

/// Sicheres Ziel nach der Anmeldung: nur ein Pfad auf diesem Server (`/…`), nie `//…` oder `/\…` (beides läse ein
/// Browser als fremden Rechner) und nichts mit Steuerzeichen (Browser streichen Tab und Zeilenumbruch aus Adressen,
/// aus `/\t/x` würde `//x`); sonst `/`.
pub fn safe_next(next: &str) -> &str {
    let mut chars = next.chars();
    let local = chars.next() == Some('/') && !matches!(chars.next(), Some('/' | '\\'));
    if local && !next.chars().any(char::is_control) { next } else { "/" }
}

/// Setzt an [`KONTO_MARK`] den Konto-Eintrag des Menüs ein und kennzeichnet `<body>` mit der Rolle.
pub fn apply(page: &str, who: &Who, accounts_exist: bool) -> String {
    // STUB: Menüeintrag („Konten einrichten…“, „Angemeldet: …“, Abmelden) und data-rolle an <body> (Runde 35) –
    // bis dahin wird nur die Marke entfernt.
    let _ = (who, accounts_exist);
    if page.contains(KONTO_MARK) { page.replacen(KONTO_MARK, "", 1) } else { page.to_owned() }
}

/// Seiten der Konten: `/anmelden`, `/einrichten`, `/konto`, `/werke/{id}/mitwirkende`.
pub fn pages() -> Router<Arc<AppState>> {
    // STUB: Seiten der Konten (Runde 35)
    Router::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_stays_on_this_server() {
        assert_eq!(safe_next("/"), "/", "die Startseite");
        assert_eq!(safe_next("/?work=2&key=7"), "/?work=2&key=7", "Pfad mit Adresszusatz bleibt");
        assert_eq!(safe_next("/werke/3/befunde"), "/werke/3/befunde", "ein Pfad auf diesem Server bleibt");
        assert_eq!(safe_next("//boese.example/x"), "/", "// liest der Browser als fremden Rechner");
        assert_eq!(safe_next("/\\boese.example"), "/", "/\\ liest der Browser ebenfalls als fremden Rechner");
        assert_eq!(safe_next("https://boese.example/"), "/", "eine volle Adresse ist kein Pfad");
        assert_eq!(safe_next("werke"), "/", "ohne / am Anfang ist es kein Pfad dieses Servers");
        assert_eq!(safe_next(""), "/", "leer heißt Startseite");
        assert_eq!(safe_next("/\t/boese.example"), "/", "der Browser streicht den Tab – übrig bliebe //");
    }

    #[test]
    fn apply_only_removes_the_mark() {
        let page = format!("<body><nav>{KONTO_MARK}</nav></body>");
        assert_eq!(apply(&page, &Who::Open, false), "<body><nav></nav></body>", "die Marke verschwindet");
        let plain = "<body><p>ohne Marke</p></body>";
        assert_eq!(apply(plain, &Who::Open, false), plain, "eine Seite ohne Marke bleibt Byte für Byte gleich");
    }

    #[test]
    fn every_message_code_of_the_contract_is_listed_once() {
        let codes: Vec<&str> = MESSAGES.iter().map(|(code, _)| *code).collect();
        assert_eq!(codes.len(), 19, "neun ok-Codes und zehn Fehlercodes (Vertrag §3.4)");
        let unique: std::collections::BTreeSet<&str> = codes.iter().copied().collect();
        assert_eq!(unique.len(), codes.len(), "kein Code doppelt – sonst wäre die Meldung mehrdeutig");
        for code in ["abgemeldet", "konto_angelegt", "letzter_autor", "nicht_eigener_rechner"] {
            assert!(unique.contains(code), "{code} steht in der Tabelle");
        }
    }

    #[tokio::test]
    async fn the_forms_carry_the_field_names_of_the_contract_and_never_a_password() {
        let view =
            LoginView { name: "Erika \"E\"".into(), next: "//boese.example".into(), error: None, retry_after: None };
        let response = login_page(StatusCode::UNAUTHORIZED, &view);
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "der Status kommt vom Aufrufer");
        let html =
            String::from_utf8(axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
        for field in ["name=\"name\"", "name=\"password\"", "name=\"next\"", "action=\"/anmelden\""] {
            assert!(html.contains(field), "Anmeldeformular mit {field}");
        }
        assert!(html.contains("value=\"Erika &quot;E&quot;\""), "der Name ist maskiert vorbelegt");
        assert!(html.contains("name=\"next\" value=\"/\""), "ein unsicheres Ziel wird zur Startseite");
        let view = SetupView { name: String::new(), display: String::new(), error: None, allowed: true };
        let html = setup_page(StatusCode::OK, &view);
        let html = String::from_utf8(axum::body::to_bytes(html.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
        for field in
            ["name=\"name\"", "name=\"display\"", "name=\"password\"", "name=\"password2\"", "action=\"/einrichten\""]
        {
            assert!(html.contains(field), "Einrichten mit {field}");
        }
        let view = SetupView { name: String::new(), display: String::new(), error: None, allowed: false };
        let html = setup_page(StatusCode::FORBIDDEN, &view);
        let html = String::from_utf8(axum::body::to_bytes(html.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
        assert!(!html.contains("<form"), "nicht vom eigenen Rechner: nur die Erklärung, kein Formular");
    }
}
