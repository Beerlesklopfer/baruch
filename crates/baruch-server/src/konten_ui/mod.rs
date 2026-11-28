// SPDX-License-Identifier: AGPL-3.0-or-later
//! User interface of the accounts (Round 35, contract §S1.4 and §3): sign-in page, setting up the first account,
//! „Mein Konto“, contributors per work, settings page „Konten“ and the account entry in the menu „Baruch ▾“.
//!
//! [`apply`] runs in `consent::inject` after `band::apply` over every page that extends `base.html`: at the
//! marker [`KONTO_MARK`] it inserts the entry for the account, labels `<body>` with the role (`data-rolle`) and
//! tidies up the role markers (see there). Messages from the address (`?ok=`, `?fehler=`) are shown by the pages
//! only through [`MESSAGES`] – never text from the address.
//!
//! **What a Lektor sees** (contract §0.8): security lies in the lock alone (`crate::roles`). The
//! work page leaves out author buttons on the server side (`crate::view`, templates `fragments/lektor_*.html`); all
//! other pages hide them through `static/konten.css` – without JavaScript: `.nur-autor` and `[data-nur-autor]`
//! (groups of the ribbon whose tag must stay verbatim) disappear for `body[data-rolle="lektor"]`, and so does every
//! POST form that does not go to an address allowed for Lektors; `.nur-lektor` is visible only there.
//!
//! The sign-in page and „Einrichten“ are standalone pages without a ribbon (whoever is not signed in sees no
//! menu); „Mein Konto“ and „Mitwirkende“ extend `base.html`. Pages and pure presentation functions are in
//! [`pages`].

pub mod names;
pub mod pages;

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use minijinja::{Environment, Value, context};

use crate::AppState;
use crate::auth::{AccountKind, Who};

/// Sign-in page: prefilled name (never the password), target after signing in, error message and – during the
/// lock against brute forcing – the waiting time in seconds.
pub struct LoginView {
    pub name: String,
    pub next: String,
    pub error: Option<String>,
    pub retry_after: Option<u64>,
}

/// Page „Einrichten“ (first account, the author). `allowed` = the request comes from the own machine; otherwise the
/// page shows only the explanation.
pub struct SetupView {
    pub name: String,
    pub display: String,
    pub error: Option<String>,
    pub allowed: bool,
}

/// Marker in `base.html` (menu „Baruch ▾“ after „Baruch beenden…“) at which [`apply`] inserts the account entry.
pub const KONTO_MARK: &str = "<!--BARUCH-KONTO-->";

/// Brackets around pieces of a page that only a Lektor needs (forms to `/lektorat/act`): [`apply`] cuts
/// them out for everyone else and removes only the brackets for Lektors. The content additionally carries the class
/// `nur-lektor`.
pub const LEKTOR_OPEN: &str = "<!--NUR-LEKTOR-->";
/// End of a Lektor piece, see [`LEKTOR_OPEN`].
pub const LEKTOR_CLOSE: &str = "<!--/NUR-LEKTOR-->";

/// Brackets around pieces that exist only with accounts (the button „Mitwirkende“ in the tab Buch): without accounts
/// [`apply`] cuts them out, with accounts it removes only the brackets.
pub const KONTEN_OPEN: &str = "<!--NUR-KONTEN-->";
/// End of a piece only with accounts, see [`KONTEN_OPEN`].
pub const KONTEN_CLOSE: &str = "<!--/NUR-KONTEN-->";

/// Message codes of the addresses `?ok=<code>` and `?fehler=<code>` with their German sentence (contract §3.4): first
/// the nine success messages, then the ten errors.
pub const MESSAGES: &[(&str, &str)] = &[
    ("konto_angelegt", "Das Konto ist angelegt."),
    ("passwort_geaendert", "Das Passwort ist geändert. Andere Sitzungen dieses Kontos sind abgemeldet."),
    ("konto_gesperrt", "Das Konto ist gesperrt; seine Sitzungen sind beendet."),
    ("konto_entsperrt", "Das Konto ist wieder aktiv."),
    ("lokal_offen", "Am eigenen Rechner gilt jetzt keine Anmeldung mehr (Autorrechte ohne Passwort)."),
    ("lokal_anmelden", "Auch am eigenen Rechner ist jetzt eine Anmeldung nötig."),
    ("freigegeben", "Das Werk ist für das Konto freigegeben."),
    ("entzogen", "Die Freigabe ist entzogen."),
    ("abgemeldet", "Abgemeldet."),
    ("passwort_falsch", "Das bisherige Passwort stimmt nicht."),
    ("passwort_kurz", "Das Passwort ist zu kurz (mindestens 10 Zeichen)."),
    ("passwort_lang", "Das Passwort ist zu lang (höchstens 200 Zeichen)."),
    ("passwort_ungleich", "Die beiden Passwörter stimmen nicht überein."),
    ("name_vergeben", "Diesen Namen gibt es schon (oder einen, der dasselbe Kürzel ergäbe)."),
    ("name_ungueltig", "Der Name ist ungültig (1 bis 40 Zeichen, nicht „unbekannt“)."),
    ("letzter_autor", "Das letzte aktive Autor-Konto lässt sich nicht sperren."),
    ("unbekanntes_konto", "Dieses Konto gibt es nicht."),
    ("kein_lektor", "Freigeben lässt sich ein Werk nur für ein Lektor-Konto."),
    (
        "nicht_eigener_rechner",
        "Das geht nur am eigenen Rechner (Baruch direkt im Browser dieses Rechners, nicht über einen Proxy).",
    ),
];

/// Number of success messages at the beginning of [`MESSAGES`]; the rest are errors.
const OK_MESSAGES: usize = 9;

/// Sentence for a code from `?ok=`; an unknown code (or an error code) yields nothing.
pub fn ok_message(code: &str) -> Option<&'static str> {
    MESSAGES[..OK_MESSAGES].iter().find(|(known, _)| *known == code).map(|(_, text)| *text)
}

/// Sentence for a code from `?fehler=`; an unknown code (or a success code) yields nothing.
pub fn error_message(code: &str) -> Option<&'static str> {
    MESSAGES[OK_MESSAGES..].iter().find(|(known, _)| *known == code).map(|(_, text)| *text)
}

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

/// Own template environment (autoescape by `.html`): the standalone pages `anmelden.html` and
/// `einrichten.html`, plus `konto.html` and `mitwirkende.html`, which extend `base.html`.
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../../templates/base.html")),
            ("anmelden.html", include_str!("../../templates/anmelden.html")),
            ("einrichten.html", include_str!("../../templates/einrichten.html")),
            ("konto.html", include_str!("../../templates/konto.html")),
            ("mitwirkende.html", include_str!("../../templates/mitwirkende.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Konten fehlerhaft");
        }
        env
    })
}

pub(crate) fn render_template(name: &str, ctx: Value) -> Result<String, String> {
    templates().get_template(name).and_then(|t| t.render(ctx)).map_err(|e| format!("Vorlage {name}: {e}"))
}

/// HTML response of an account page; never in the cache of a browser or proxy (`no-store`).
pub(crate) fn html_response(status: StatusCode, html: Result<String, String>) -> Response {
    match html {
        Ok(html) => {
            (status, [(header::CONTENT_TYPE, "text/html; charset=utf-8"), (header::CACHE_CONTROL, "no-store")], html)
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    }
}

/// Sign-in page with the form `POST /anmelden` (fields `name`, `password`, `next`). The password field is never
/// prefilled; `next` enters the page only as [`safe_next`].
pub fn login_page(status: StatusCode, view: &LoginView) -> Response {
    html_response(status, login_html(view, None, None))
}

/// The sign-in page as HTML. `notice` is a finished sentence from [`MESSAGES`] (e.g. after signing out),
/// `signed_in` the display name if the request already carries a valid session.
pub fn login_html(view: &LoginView, notice: Option<&str>, signed_in: Option<&str>) -> Result<String, String> {
    render_template(
        "anmelden.html",
        context! {
            name => view.name.as_str(),
            next => safe_next(&view.next),
            error => view.error.as_deref(),
            retry_after => view.retry_after,
            notice => notice,
            signed_in => signed_in,
        },
    )
}

/// Page „Einrichten“ with the form `POST /einrichten` (fields `name`, `display`, `password`, `password2`); without
/// [`SetupView::allowed`] only the explanation. Password fields are never prefilled.
pub fn setup_page(status: StatusCode, view: &SetupView) -> Response {
    html_response(status, setup_html(view))
}

/// The page „Einrichten“ as HTML.
pub fn setup_html(view: &SetupView) -> Result<String, String> {
    render_template(
        "einrichten.html",
        context! {
            name => view.name.as_str(),
            display => view.display.as_str(),
            error => view.error.as_deref(),
            allowed => view.allowed,
            min => crate::auth::password::MIN_PASSWORD_CHARS,
            max => crate::auth::password::MAX_PASSWORD_CHARS,
        },
    )
}

/// Safe target after signing in: only a path on this server (`/…`), never `//…` or `/\…` (a browser would read
/// both as a foreign host) and nothing with control characters (browsers strip tab and line break from addresses,
/// `/\t/x` would become `//x`); otherwise `/`.
pub fn safe_next(next: &str) -> &str {
    let mut chars = next.chars();
    let local = chars.next() == Some('/') && !matches!(chars.next(), Some('/' | '\\'));
    if local && !next.chars().any(char::is_control) { next } else { "/" }
}

/// The entry for the account in the menu „Baruch ▾“ (finished HTML, everything foreign escaped).
fn menu_entry(who: &Who, accounts_exist: bool) -> String {
    const ITEM: &str = "<a class=\"menu-item\" role=\"menuitem\"";
    const NOTE: &str = "<span class=\"menu-konto\" role=\"presentation\">";
    if !accounts_exist {
        return format!(
            "{ITEM} href=\"/einrichten\" title=\"Ein Konto für den Autor anlegen; danach verlangt Baruch eine \
             Anmeldung, und Lektoren können mitarbeiten\">Konten einrichten…</a>"
        );
    }
    match who {
        Who::Account(identity) => {
            let shown = if identity.display.trim().is_empty() { &identity.name } else { &identity.display };
            let role = match identity.kind {
                AccountKind::Author => "Autor",
                AccountKind::Lektor => "Lektor",
            };
            format!(
                "{NOTE}Angemeldet: {} ({role})</span>{ITEM} href=\"/konto\" title=\"Name, Rolle, freigegebene Werke, \
                 Passwort ändern\">Mein Konto</a><form class=\"menu-form\" method=\"post\" \
                 action=\"/abmelden\"><button class=\"menu-item\" role=\"menuitem\" \
                 type=\"submit\">Abmelden</button></form>",
                escape(shown)
            )
        }
        Who::LocalAuthor => {
            format!("{NOTE}Autor (eigener Rechner, ohne Anmeldung)</span>{ITEM} href=\"/anmelden\">Anmelden</a>")
        }
        // Without signing in no page with a menu arrives (auth::gate redirects); to be safe only the way to the
        // sign-in. `Open` with accounts exists only in the request that creates the first account.
        Who::Anonymous | Who::Open => format!("{ITEM} href=\"/anmelden\">Anmelden</a>"),
    }
}

/// Cuts out every piece between `open` and `close` including the brackets; an open bracket without an end cuts
/// up to the end (better to show too little than a Lektor form to the author).
fn cut_blocks(page: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut rest = page;
    while let Some(start) = rest.find(open) {
        out.push_str(&rest[..start]);
        match rest[start..].find(close) {
            Some(end) => rest = &rest[start + end + close.len()..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn cut_lektor_blocks(page: &str) -> String {
    cut_blocks(page, LEKTOR_OPEN, LEKTOR_CLOSE)
}

/// Takes the markers ` nur-autor` (last class) and ` data-nur-autor` out of the **tags** of the page. Text between
/// the tags stays untouched – even if the book text (text field, typeset view) happens to contain the same
/// characters: there `<` is always escaped, so a tag never begins in the text.
fn strip_author_marks(page: &str) -> String {
    let mut out = String::with_capacity(page.len());
    let mut rest = page;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let end = tail.find('>').map_or(tail.len(), |end| end + 1);
        let tag = &tail[..end];
        if tag.contains("nur-autor") {
            out.push_str(&tag.replace(" nur-autor\"", "\"").replace(" data-nur-autor", ""));
        } else {
            out.push_str(tag);
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// Inserts the account entry of the menu at [`KONTO_MARK`] and arranges the page by role:
///
/// - **Without accounts** ([`Who::Open`]): only the entry „Konten einrichten…“; the markers ` nur-autor` (always the
///   last class of an element) and ` data-nur-autor`, the Lektor pieces and the pieces only with accounts
///   ([`KONTEN_OPEN`]) disappear – the page is, apart from the one menu entry, the same as before Round 35.
/// - **With accounts:** „Angemeldet: <Anzeigename> (Autor|Lektor)“, „Mein Konto“ and the form „Abmelden“ or, on
///   the own machine without signing in, the notice with „Anmelden“; `<body>` carries `data-rolle="autor|lektor"`.
///   For Lektors the Lektor pieces stay (without brackets), for everyone else they are cut out; what only the
///   author can use, `static/konten.css` hides for Lektors.
///
/// A page without the marker (it does not extend `base.html`) stays the same byte for byte.
pub fn apply(page: &str, who: &Who, accounts_exist: bool) -> String {
    if !page.contains(KONTO_MARK) {
        return page.to_owned();
    }
    let page = page.replacen(KONTO_MARK, &menu_entry(who, accounts_exist), 1);
    if !accounts_exist {
        return strip_author_marks(&cut_blocks(&cut_lektor_blocks(&page), KONTEN_OPEN, KONTEN_CLOSE));
    }
    let page = page.replace(KONTEN_OPEN, "").replace(KONTEN_CLOSE, "");
    let (role, page) = match who {
        Who::Account(identity) if identity.kind == AccountKind::Lektor => {
            (Some("lektor"), page.replace(LEKTOR_OPEN, "").replace(LEKTOR_CLOSE, ""))
        }
        Who::Account(_) | Who::LocalAuthor => (Some("autor"), cut_lektor_blocks(&page)),
        Who::Anonymous | Who::Open => (None, cut_lektor_blocks(&page)),
    };
    match role {
        Some(role) => page.replacen("<body>", &format!("<body data-rolle=\"{role}\">"), 1),
        None => page,
    }
}

/// Head of the standalone pages of the accounts (`anmelden.html`, `einrichten.html`), verbatim as in the templates
/// up to the language: `<!doctype html>\n<html lang="xx">` with any two-letter code (Round 39a, `ui_lang()`).
const STANDALONE_HEAD: &str = "<!doctype html>\n<html lang=\"";

/// Language code of a standalone head (`de` in `<html lang="de">`) and the length of the head; `None` if the page
/// does not start with it.
fn standalone_lang(page: &str) -> Option<(&str, usize)> {
    let rest = page.strip_prefix(STANDALONE_HEAD)?;
    let code = rest.get(..2).filter(|code| code.bytes().all(|b| b.is_ascii_lowercase()))?;
    rest[2..].starts_with("\">").then_some((code, STANDALONE_HEAD.len() + 4))
}

/// Round 36: the standalone pages `/anmelden` and `/einrichten` (without a ribbon) follow the cookie
/// `baruch_theme`: `theme` (`light` | `dark`, already checked by [`crate::view::Settings`]) goes as `data-theme`
/// onto `<html>`, as with `base.html`. `None` = the page stays as it is (not a standalone page of the accounts
/// or no cookie: then the colour scheme of the system keeps applying).
pub fn themed(page: &str, theme: Option<&str>) -> Option<String> {
    let theme = theme.filter(|theme| matches!(*theme, "light" | "dark"))?;
    let (lang, head) = standalone_lang(page)?;
    if !page.contains("<body class=\"konto-seite\">") {
        return None;
    }
    Some(format!("<!doctype html>\n<html lang=\"{lang}\" data-theme=\"{theme}\">{}", &page[head..]))
}

/// Pages of the accounts: `/anmelden`, `/einrichten`, `/konto`, `/werke/{id}/mitwirkende`.
pub fn pages() -> Router<Arc<AppState>> {
    pages::router()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::Identity;

    fn account(kind: AccountKind, name: &str, display: &str) -> Who {
        Who::Account(Identity {
            id: 3,
            name: name.into(),
            display: display.into(),
            kind,
            handle: crate::auth::lektor_handle(name),
        })
    }

    /// A piece of a page in the verbatim forms of `base.html`: marker in the menu, an author entry, an author group
    /// and a Lektor piece.
    fn sample() -> String {
        format!(
            "<html><body>\n<nav><a class=\"menu-item nur-autor\" href=\"/beenden\">Baruch beenden…</a>\n{KONTO_MARK}\n\
             </nav><details class=\"band-group\" data-nur-autor data-band-group open>G</details>{KONTEN_OPEN}\
             <a class=\"btn nur-autor\" href=\"/werke/1/mitwirkende\">Mitwirkende</a>{KONTEN_CLOSE}\
             <main>Text{LEKTOR_OPEN}<form class=\"nur-lektor\" method=\"post\" action=\"/lektorat/act\"></form>\
             {LEKTOR_CLOSE}Ende</main></body></html>"
        )
    }

    #[test]
    fn next_stays_on_this_server() {
        assert_eq!(safe_next("/"), "/", "die Startseite");
        assert_eq!(safe_next("/?work=2&key=7"), "/?work=2&key=7", "Pfad mit Adresszusatz bleibt");
        assert_eq!(safe_next("/werke/3/befunde"), "/werke/3/befunde", "ein Pfad auf diesem Server bleibt");
        assert_eq!(safe_next("//boese.example/x"), "/", "// liest der Browser als fremden Rechner");
        assert_eq!(safe_next("/\\boese.example"), "/", "/\\ liest der Browser ebenfalls als fremden Rechner");
        assert_eq!(safe_next("https://boese.example/"), "/", "eine volle Adresse ist kein Pfad");
        assert_eq!(safe_next("javascript:alert(1)"), "/", "ein Schema ist kein Pfad");
        assert_eq!(safe_next("werke"), "/", "ohne / am Anfang ist es kein Pfad dieses Servers");
        assert_eq!(safe_next(""), "/", "leer heißt Startseite");
        assert_eq!(safe_next("/\t/boese.example"), "/", "der Browser streicht den Tab – übrig bliebe //");
    }

    #[test]
    fn without_accounts_only_the_one_menu_entry_is_new() {
        let out = apply(&sample(), &Who::Open, false);
        assert_eq!(
            out,
            "<html><body>\n<nav><a class=\"menu-item\" href=\"/beenden\">Baruch beenden…</a>\n<a class=\"menu-item\" \
             role=\"menuitem\" href=\"/einrichten\" title=\"Ein Konto für den Autor anlegen; danach verlangt Baruch \
             eine Anmeldung, und Lektoren können mitarbeiten\">Konten einrichten…</a>\n</nav><details \
             class=\"band-group\" data-band-group open>G</details><main>TextEnde</main></body></html>",
            "ohne Konten: der Eintrag „Konten einrichten…“ an der Marke, sonst die Seite wie vor Runde 35 – keine \
             Rolle am body, keine Kennzeichen nur-autor, kein Lektor-Stück"
        );
        let plain = "<body><p class=\"x nur-autor\">ohne Marke</p></body>";
        assert_eq!(apply(plain, &Who::Open, false), plain, "eine Seite ohne Marke bleibt Byte für Byte gleich");
    }

    #[test]
    fn an_author_account_sees_name_role_account_link_and_the_logout_form() {
        let out = apply(&sample(), &account(AccountKind::Author, "joerg", "Jörg <B> & Co"), true);
        assert!(!out.contains(KONTO_MARK), "die Marke steht nie in der ausgelieferten Seite");
        assert!(
            out.contains("Angemeldet: Jörg &lt;B&gt; &amp; Co (Autor)"),
            "Anzeigename (maskiert) und Rolle stehen im Menü: {out}"
        );
        assert!(out.contains("href=\"/konto\""), "„Mein Konto“ führt auf /konto");
        assert!(
            out.contains("<form class=\"menu-form\" method=\"post\" action=\"/abmelden\"><button"),
            "Abmelden ist ein Formular (POST), kein Link – ein Link ließe sich von außen auslösen"
        );
        assert!(out.contains("<body data-rolle=\"autor\">"), "der body nennt die Rolle für konten.css");
        assert!(!out.contains("/lektorat/act"), "ein Autor bekommt kein Lektor-Formular");
        assert!(
            out.contains("</details><a class=\"btn nur-autor\" href=\"/werke/1/mitwirkende\">Mitwirkende</a><main>"),
            "mit Konten steht der Knopf „Mitwirkende“ da, ohne Klammern: {out}"
        );
        assert!(
            out.contains("class=\"menu-item nur-autor\"") && out.contains(" data-nur-autor"),
            "mit Konten bleiben die Kennzeichen stehen (sie wirken nur für Lektoren)"
        );
        assert!(
            out.find("Baruch beenden…").unwrap() < out.find("Angemeldet:").unwrap(),
            "der Eintrag steht nach „Baruch beenden…“"
        );
    }

    #[test]
    fn a_lektor_gets_the_role_lektor_and_keeps_the_lektor_pieces() {
        let out = apply(&sample(), &account(AccountKind::Lektor, "Erika", ""), true);
        assert!(out.contains("Angemeldet: Erika (Lektor)"), "ohne Anzeigenamen steht der Kontoname: {out}");
        assert!(out.contains("<body data-rolle=\"lektor\">"), "Rolle lektor am body");
        assert!(
            out.contains("<main>Text<form class=\"nur-lektor\" method=\"post\" action=\"/lektorat/act\"></form>Ende"),
            "das Lektor-Stück bleibt, nur die Klammern verschwinden: {out}"
        );
        for mark in [LEKTOR_OPEN, LEKTOR_CLOSE, KONTEN_OPEN, KONTEN_CLOSE, KONTO_MARK] {
            assert!(!out.contains(mark), "die Marke {mark} steht nie in der ausgelieferten Seite");
        }
    }

    #[test]
    fn the_local_author_sees_the_hint_and_a_link_to_sign_in() {
        let out = apply(&sample(), &Who::LocalAuthor, true);
        assert!(out.contains("Autor (eigener Rechner, ohne Anmeldung)"), "Hinweis auf die Ausnahme: {out}");
        assert!(out.contains("href=\"/anmelden\">Anmelden</a>"), "Link zur Anmeldung");
        assert!(out.contains("<body data-rolle=\"autor\">"), "die Ausnahme gibt Autorrechte");
        assert!(!out.contains("/abmelden"), "ohne Sitzung gibt es nichts abzumelden");
        let anonymous = apply(&sample(), &Who::Anonymous, true);
        assert!(
            !anonymous.contains("data-rolle") && anonymous.contains("href=\"/anmelden\">Anmelden</a>"),
            "ohne Anmeldung keine Rolle, nur der Weg zur Anmeldung"
        );
    }

    #[test]
    fn book_text_that_looks_like_a_mark_is_left_alone() {
        let text = "Im Tag steht class=\"btn nur-autor\" oder data-nur-autor – so der Text.";
        let page = format!(
            "<body>{KONTO_MARK}<a class=\"btn nur-autor\" href=\"/x\">x</a><textarea name=\"body\">{text}</textarea>\
             <p>{text}</p></body>"
        );
        let out = apply(&page, &Who::Open, false);
        assert!(out.contains("<a class=\"btn\" href=\"/x\">x</a>"), "das Kennzeichen im Tag verschwindet: {out}");
        assert_eq!(
            out.matches(text).count(),
            2,
            "Text zwischen den Tags (Textfeld, Absatz) bleibt Zeichen für Zeichen – sonst änderte die Schicht den \
             Buchtext, sobald er zufällig wie ein Kennzeichen aussieht: {out}"
        );
    }

    #[test]
    fn an_unclosed_lektor_piece_is_cut_to_the_end() {
        let page = format!("<body>{KONTO_MARK}vor{LEKTOR_OPEN}<form action=\"/lektorat/act\">");
        let out = apply(&page, &Who::Open, false);
        assert!(out.ends_with("vor"), "fehlt das Ende, wird bis zum Schluss geschnitten: {out}");
    }

    #[test]
    fn every_message_code_of_the_contract_has_a_german_sentence() {
        let codes: Vec<&str> = MESSAGES.iter().map(|(code, _)| *code).collect();
        assert_eq!(codes.len(), 19, "neun ok-Codes und zehn Fehlercodes (Vertrag §3.4)");
        let unique: std::collections::BTreeSet<&str> = codes.iter().copied().collect();
        assert_eq!(unique.len(), codes.len(), "kein Code doppelt – sonst wäre die Meldung mehrdeutig");
        for (code, text) in MESSAGES {
            assert!(
                text.ends_with('.') && text.starts_with(char::is_uppercase) && !text.contains('_'),
                "{code}: ein deutscher Satz (groß begonnen, mit Punkt), nicht der Code: {text}"
            );
        }
        for code in [
            "konto_angelegt",
            "passwort_geaendert",
            "konto_gesperrt",
            "konto_entsperrt",
            "lokal_offen",
            "lokal_anmelden",
            "freigegeben",
            "entzogen",
            "abgemeldet",
        ] {
            assert!(ok_message(code).is_some(), "{code} ist eine Erfolgsmeldung");
            assert!(error_message(code).is_none(), "{code} erscheint nie als Fehler");
        }
        for code in [
            "passwort_falsch",
            "passwort_kurz",
            "passwort_lang",
            "passwort_ungleich",
            "name_vergeben",
            "name_ungueltig",
            "letzter_autor",
            "unbekanntes_konto",
            "kein_lektor",
            "nicht_eigener_rechner",
        ] {
            assert!(error_message(code).is_some(), "{code} ist ein Fehler");
            assert!(ok_message(code).is_none(), "{code} erscheint nie als Erfolg");
        }
        assert!(ok_message("<script>").is_none() && error_message("").is_none(), "Unbekanntes ergibt nichts");
        let min = crate::auth::password::MIN_PASSWORD_CHARS.to_string();
        let max = crate::auth::password::MAX_PASSWORD_CHARS.to_string();
        assert!(error_message("passwort_kurz").unwrap().contains(&min), "die Meldung nennt die echte Mindestlänge");
        assert!(error_message("passwort_lang").unwrap().contains(&max), "die Meldung nennt die echte Höchstlänge");
    }

    async fn body(response: Response) -> String {
        String::from_utf8(axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap()
    }

    #[tokio::test]
    async fn the_forms_carry_the_field_names_of_the_contract_and_never_a_password() {
        let view =
            LoginView { name: "Erika \"E\"".into(), next: "//boese.example".into(), error: None, retry_after: None };
        let response = login_page(StatusCode::UNAUTHORIZED, &view);
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "der Status kommt vom Aufrufer");
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store",
            "die Anmeldeseite gehört in keinen Zwischenspeicher"
        );
        let html = body(response).await;
        for field in ["name=\"name\"", "name=\"password\"", "name=\"next\"", "action=\"/anmelden\""] {
            assert!(html.contains(field), "Anmeldeformular mit {field}");
        }
        assert!(html.contains("value=\"Erika &quot;E&quot;\""), "der Name ist maskiert vorbelegt: {html}");
        assert!(
            html.contains("name=\"next\" value=\"&#x2f;\""),
            "ein unsicheres Ziel wird zur Startseite (minijinja maskiert / im Attribut als &#x2f;): {html}"
        );
        assert!(!html.contains("boese.example"), "das fremde Ziel steht nirgends in der Seite");
        let view = SetupView { name: String::new(), display: String::new(), error: None, allowed: true };
        let html = body(setup_page(StatusCode::OK, &view)).await;
        for field in
            ["name=\"name\"", "name=\"display\"", "name=\"password\"", "name=\"password2\"", "action=\"/einrichten\""]
        {
            assert!(html.contains(field), "Einrichten mit {field}");
        }
        let view = SetupView { name: String::new(), display: String::new(), error: None, allowed: false };
        let html = body(setup_page(StatusCode::FORBIDDEN, &view)).await;
        assert!(!html.contains("<form"), "nicht vom eigenen Rechner: nur die Erklärung, kein Formular");
    }

    #[test]
    fn standalone_pages_get_the_theme_in_every_interface_language() {
        for lang in ["de", "en"] {
            let page =
                format!("<!doctype html>\n<html lang=\"{lang}\">\n<head></head>\n<body class=\"konto-seite\">x</body>");
            let themed = themed(&page, Some("dark")).expect("a standalone account page is themed");
            assert!(
                themed.starts_with(&format!("<!doctype html>\n<html lang=\"{lang}\" data-theme=\"dark\">\n<head>")),
                "the language of the page stays and the theme is added: {themed}"
            );
        }
        let other = "<!doctype html>\n<html lang=\"de\" data-band-fill>\n<body class=\"konto-seite\">";
        assert_eq!(themed(other, Some("dark")), None, "another head (base.html) is not a standalone page");
    }
}
