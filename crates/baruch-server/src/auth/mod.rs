// SPDX-License-Identifier: AGPL-3.0-or-later
//! Konten und Anmeldung (Runde 35, Vertrag §S1.4 und §1).
//!
//! Ohne Konten (Tabelle `account` leer) bleibt Baruch offen wie vor Runde 35: [`Who::Open`], keine Cookies, keine
//! Umleitung. Mit Konten verlangen Seiten, Formulare und `/rpc` eine Anmeldung – auch am eigenen Rechner (Vorgabe;
//! die Ausnahme „Am eigenen Rechner ohne Anmeldung“ schaltet der Autor ein, sie ergibt [`Who::LocalAuthor`]).
//! `/mcp` bleibt bei Tokens; die Schicht [`gate`] lässt `/mcp` unberührt.
//!
//! Wer eine Anfrage stellt, steht für ihre Dauer in einer task-lokalen Ablage: [`gate`] bestimmt [`Who`] und führt
//! die Anfrage in [`scope`] aus, jede Stelle darunter fragt [`current`]. Außerhalb einer Anfrage (Start, Hilfsbefehle,
//! Tests ohne Router) ist das [`Who::Open`].
//!
//! Regel: [`current`] nie in abgespaltenen Aufgaben (`tokio::spawn`, `spawn_blocking`) auswerten – dort ist die
//! Ablage leer und die Antwort wäre [`Who::Open`]. Vorher holen und mitgeben.
//!
//! Was eine Rolle darf, entscheidet allein [`crate::roles`].

pub mod cli;
pub mod gate;
pub mod password;
pub mod routes;
pub mod rpc;
pub mod session;
pub mod throttle;

use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

pub use gate::gate;

use crate::AppState;

/// Art eines Kontos (global; die Rolle gilt je Werk, siehe [`crate::roles::role_in`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Author,
    Lektor,
}

/// Ein angemeldetes Konto. `handle` ist die Herkunft, unter der ein Lektor schreibt (`@lektorat:<slug>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Identity {
    pub id: i64,
    pub name: String,
    pub display: String,
    pub kind: AccountKind,
    pub handle: String,
}

/// Wer eine Anfrage stellt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Who {
    /// Es gibt keine Konten: alles offen wie vor Runde 35.
    Open,
    /// Es gibt Konten, der Autor hat „Am eigenen Rechner ohne Anmeldung“ eingeschaltet und die Anfrage kommt vom
    /// eigenen Rechner ohne Proxy-Kopfzeilen: Autorrechte ohne Sitzung.
    LocalAuthor,
    /// Gültige Sitzung eines aktiven Kontos.
    Account(Identity),
    /// Es gibt Konten, aber keine gültige Sitzung.
    Anonymous,
}

impl Who {
    /// Volle Rechte: ohne Konten, Autor am eigenen Rechner oder ein Autor-Konto.
    pub fn full_rights(&self) -> bool {
        match self {
            Who::Open | Who::LocalAuthor => true,
            Who::Account(identity) => identity.kind == AccountKind::Author,
            Who::Anonymous => false,
        }
    }

    /// Das Konto, wenn ein Lektor angemeldet ist.
    pub fn lektor(&self) -> Option<&Identity> {
        match self {
            Who::Account(identity) if identity.kind == AccountKind::Lektor => Some(identity),
            _ => None,
        }
    }

    /// Das angemeldete Konto (Autor oder Lektor).
    pub fn identity(&self) -> Option<&Identity> {
        match self {
            Who::Account(identity) => Some(identity),
            _ => None,
        }
    }
}

tokio::task_local! {
    static WHO: Who;
}

/// Wer die laufende Anfrage stellt; außerhalb einer Anfrage [`Who::Open`]. Nie in abgespaltenen Aufgaben auswerten
/// (siehe Modulkopf).
pub fn current() -> Who {
    WHO.try_with(Who::clone).unwrap_or(Who::Open)
}

/// Führt `f` aus, als stellte `who` die Anfrage ([`current`] liefert darin `who`).
pub async fn scope<F: std::future::Future>(who: Who, f: F) -> F::Output {
    WHO.scope(who, f).await
}

/// Herkunft eines Lektors: `@lektorat:<slug>`, der Slug nach derselben Regel wie
/// [`crate::docx_import::word_author`] (klein, Umlaute umschrieben, höchstens 40 Zeichen; bleibt nichts übrig,
/// `unbekannt` – diesen Kontonamen lehnt „konten“ darum ab).
pub fn lektor_handle(name: &str) -> String {
    let word = crate::docx_import::word_author(name);
    format!("@lektorat:{}", word.strip_prefix("@word:").unwrap_or(&word))
}

/// Cookie der Sitzung.
pub const SESSION_COOKIE: &str = "baruch_session";

/// Pfade, die auch ohne Anmeldung erreichbar bleiben; dazu alles unter [`PUBLIC_PREFIX`].
pub const PUBLIC_PATHS: &[&str] = &["/anmelden", "/static/app.css", "/static/favicon.svg"];

/// Präfix der Widgets, die die Anmeldeseite lädt.
pub const PUBLIC_PREFIX: &str = "/static/js/";

/// Zustand der Anmeldung je [`AppState`] (nie prozessweit): das gemerkte „es gibt Konten“.
///
/// Gemerkt wird nur das Ja: Konten werden nie gelöscht, ein Ja bleibt also für die Lebensdauer des Datenordners
/// wahr. Das Nein wird jedes Mal nachgesehen (ein `SELECT count(*)` auf einer leeren Tabelle) – so kann kein
/// veraltetes „keine Konten“ eine Anfrage ohne Anmeldung durchlassen, auch wenn ein Konto an dieser Ablage vorbei
/// entsteht (anderer Prozess, Test per SQL).
#[derive(Default)]
pub struct AuthState {
    accounts: AtomicBool,
}

impl AuthState {
    /// Nach dem Einrichten: Ab jetzt gibt es Konten.
    pub fn mark_accounts(&self) {
        self.accounts.store(true, Ordering::Relaxed);
    }
}

/// Gibt es Konten? Ohne Konten bleibt jede Seite unverändert. Ein Datenbankfehler zählt als „ja“ (sichere Seite:
/// lieber eine Anmeldung zu viel verlangen als keine); er steht auf stderr.
pub async fn accounts_exist(state: &AppState) -> bool {
    if state.auth.accounts.load(Ordering::Relaxed) {
        return true;
    }
    match state.store.account_count().await {
        Ok(0) => false,
        Ok(_) => {
            state.auth.mark_accounts();
            true
        }
        Err(e) => {
            eprintln!("Konten: Anzahl nicht lesbar: {e}");
            true
        }
    }
}

/// Die Identität zu einem Konto aus dem Store (`kind` aus der Datenbank: `author`, sonst Lektor – die sichere
/// Seite, falls dort je etwas anderes stünde).
pub fn identity_of(account: &baruch_store::Account) -> Identity {
    Identity {
        id: account.id,
        name: account.name.clone(),
        display: account.display.clone(),
        kind: if account.kind == "author" { AccountKind::Author } else { AccountKind::Lektor },
        handle: format!("@lektorat:{}", account.slug),
    }
}

/// Prüft Kontonamen und Anzeigenamen; liefert den Slug des Handles oder den Meldungscode `name_ungueltig`
/// (Vertrag §3.4). Ein Name hat 1 bis 40 Zeichen ohne Steuerzeichen, und sein Slug darf nicht `unbekannt` sein (so
/// heißt, wovon nach der Regel nichts übrig bleibt); der Anzeigename hat höchstens 80 Zeichen.
pub fn check_name(name: &str, display: &str) -> Result<String, &'static str> {
    let name = name.trim();
    let length = name.chars().count();
    if !(1..=40).contains(&length) || name.chars().any(char::is_control) {
        return Err("name_ungueltig");
    }
    if display.trim().chars().count() > 80 || display.chars().any(char::is_control) {
        return Err("name_ungueltig");
    }
    let handle = lektor_handle(name);
    let slug = handle.strip_prefix("@lektorat:").unwrap_or(&handle);
    if slug == "unbekannt" {
        return Err("name_ungueltig");
    }
    Ok(slug.to_owned())
}

/// Deutscher Satz zu einem Meldungscode aus `konten_ui::MESSAGES`; ein unbekannter Code bleibt, wie er ist.
pub fn message(code: &str) -> String {
    crate::konten_ui::MESSAGES
        .iter()
        .find(|(known, _)| *known == code)
        .map_or_else(|| code.to_owned(), |(_, text)| (*text).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(kind: AccountKind) -> Identity {
        Identity { id: 1, name: "Erika".into(), display: "Erika Beispiel".into(), kind, handle: lektor_handle("Erika") }
    }

    #[test]
    fn only_open_local_author_and_author_accounts_have_full_rights() {
        assert!(Who::Open.full_rights(), "ohne Konten ist alles offen wie vor Runde 35");
        assert!(Who::LocalAuthor.full_rights(), "die Ausnahme am eigenen Rechner gibt Autorrechte");
        assert!(Who::Account(identity(AccountKind::Author)).full_rights(), "ein Autor-Konto darf alles");
        assert!(!Who::Account(identity(AccountKind::Lektor)).full_rights(), "ein Lektor hat nie volle Rechte");
        assert!(!Who::Anonymous.full_rights(), "ohne Anmeldung gibt es keine Rechte");
    }

    #[test]
    fn lektor_and_identity_tell_the_kinds_apart() {
        let lektor = Who::Account(identity(AccountKind::Lektor));
        let author = Who::Account(identity(AccountKind::Author));
        assert_eq!(lektor.lektor().map(|i| i.id), Some(1), "ein Lektor-Konto ist Lektor");
        assert!(author.lektor().is_none(), "ein Autor-Konto ist kein Lektor");
        assert!(author.identity().is_some() && lektor.identity().is_some(), "beide sind angemeldete Konten");
        for who in [Who::Open, Who::LocalAuthor, Who::Anonymous] {
            assert!(who.identity().is_none() && who.lektor().is_none(), "{who:?} ist kein Konto");
        }
    }

    #[test]
    fn the_handle_follows_the_rule_for_word_authors() {
        assert_eq!(lektor_handle("Jörg Müßig"), "@lektorat:joerg-muessig", "klein, Umlaute und ß umschrieben");
        assert_eq!(lektor_handle("…"), "@lektorat:unbekannt", "bleibt nichts übrig, heißt der Slug unbekannt");
        let long = lektor_handle(&"a".repeat(60));
        assert_eq!(long, format!("@lektorat:{}", "a".repeat(40)), "höchstens 40 Zeichen im Slug");
    }

    #[test]
    fn names_follow_the_rules_of_the_contract() {
        assert_eq!(check_name(" Jörg Müßig ", ""), Ok("joerg-muessig".to_owned()), "der Slug des Handles");
        assert_eq!(check_name("", ""), Err("name_ungueltig"), "ein Name braucht mindestens ein Zeichen");
        assert_eq!(check_name("   ", ""), Err("name_ungueltig"), "nur Leerraum ist kein Name");
        assert_eq!(check_name(&"a".repeat(41), ""), Err("name_ungueltig"), "höchstens 40 Zeichen");
        assert!(check_name(&"ä".repeat(40), "").is_ok(), "gezählt werden Zeichen, nicht Bytes");
        assert_eq!(check_name("Unbekannt", ""), Err("name_ungueltig"), "unbekannt ist als Kontoname abgelehnt");
        assert_eq!(check_name("…?!", ""), Err("name_ungueltig"), "ohne a–z und 0–9 hieße das Handle unbekannt");
        assert_eq!(check_name("Erika\u{0}x", ""), Err("name_ungueltig"), "keine Steuerzeichen");
        assert_eq!(check_name("Erika", &"x".repeat(81)), Err("name_ungueltig"), "Anzeigename höchstens 80 Zeichen");
        assert!(check_name("Erika", &"x".repeat(80)).is_ok(), "80 Zeichen Anzeigename gehen");
    }

    #[test]
    fn the_identity_takes_the_slug_from_the_database() {
        let account = baruch_store::Account {
            id: 4,
            name: "Jörg".into(),
            slug: "joerg".into(),
            display: "Jörg B.".into(),
            kind: "lektor".into(),
            active: true,
            created: String::new(),
            last_login: None,
        };
        let identity = identity_of(&account);
        assert_eq!(identity.handle, "@lektorat:joerg", "das Handle kommt aus dem gespeicherten Slug");
        assert_eq!(identity.kind, AccountKind::Lektor, "lektor bleibt Lektor");
        let author = identity_of(&baruch_store::Account { kind: "author".into(), ..account.clone() });
        assert_eq!(author.kind, AccountKind::Author, "author ist Autor");
        let odd = identity_of(&baruch_store::Account { kind: "sonstwas".into(), ..account });
        assert_eq!(odd.kind, AccountKind::Lektor, "eine unbekannte Art bekäme nie volle Rechte");
    }

    #[test]
    fn messages_come_from_the_table_and_unknown_codes_stay_codes() {
        let known = crate::konten_ui::MESSAGES[0];
        assert_eq!(message(known.0), known.1, "der Satz aus der Tabelle der Oberfläche");
        assert_eq!(message("gibt_es_nicht"), "gibt_es_nicht", "ein unbekannter Code wird nicht zu fremdem Text");
    }

    #[tokio::test]
    async fn current_is_open_outside_a_request_and_the_scoped_who_inside() {
        assert_eq!(current(), Who::Open, "außerhalb einer Anfrage gilt der offene Zustand");
        let seen = scope(Who::Anonymous, async { current() }).await;
        assert_eq!(seen, Who::Anonymous, "innerhalb von scope gilt, wer gesetzt wurde");
        assert_eq!(current(), Who::Open, "nach scope ist die Ablage wieder leer");
    }
}
