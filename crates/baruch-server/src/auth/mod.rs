// SPDX-License-Identifier: AGPL-3.0-or-later
//! Accounts and login (Round 35, contract §S1.4 and §1).
//!
//! Without accounts (table `account` empty) Baruch stays open as before Round 35: [`Who::Open`], no cookies, no
//! redirect. With accounts pages, forms and `/rpc` demand a login – also on the own machine (default;
//! the exception „Am eigenen Rechner ohne Anmeldung“ is switched on by the author, it yields [`Who::LocalAuthor`]).
//! `/mcp` stays with tokens; the layer [`gate`] leaves `/mcp` untouched.
//!
//! Who makes a request is kept for its duration in a task-local slot: [`gate`] determines [`Who`] and runs
//! the request in [`scope`], every place below asks [`current`]. Outside of a request (start, helper commands,
//! tests without router) that is [`Who::Open`].
//!
//! Rule: never evaluate [`current`] in spawned tasks (`tokio::spawn`, `spawn_blocking`) – there the
//! slot is empty and the answer would be [`Who::Open`]. Fetch it before and pass it along.
//!
//! What a role may do is decided solely by [`crate::roles`].

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

/// Kind of an account (global; the role applies per work, see [`crate::roles::role_in`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Author,
    Lektor,
}

/// A logged-in account. `handle` is the origin under which a Lektor writes (`@lektorat:<slug>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Identity {
    pub id: i64,
    pub name: String,
    pub display: String,
    pub kind: AccountKind,
    pub handle: String,
}

/// Who makes a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Who {
    /// There are no accounts: everything open as before Round 35.
    Open,
    /// There are accounts, the author has switched on „Am eigenen Rechner ohne Anmeldung“ and the request comes from
    /// the own machine without proxy headers: author rights without a session.
    LocalAuthor,
    /// Valid session of an active account.
    Account(Identity),
    /// There are accounts, but no valid session.
    Anonymous,
}

impl Who {
    /// Full rights: without accounts, author on the own machine or an author account.
    pub fn full_rights(&self) -> bool {
        match self {
            Who::Open | Who::LocalAuthor => true,
            Who::Account(identity) => identity.kind == AccountKind::Author,
            Who::Anonymous => false,
        }
    }

    /// The account if a Lektor is logged in.
    pub fn lektor(&self) -> Option<&Identity> {
        match self {
            Who::Account(identity) if identity.kind == AccountKind::Lektor => Some(identity),
            _ => None,
        }
    }

    /// The logged-in account (author or Lektor).
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

/// Who makes the running request; outside of a request [`Who::Open`]. Never evaluate in spawned tasks
/// (see module head).
pub fn current() -> Who {
    WHO.try_with(Who::clone).unwrap_or(Who::Open)
}

/// Runs `f` as if `who` made the request ([`current`] returns `who` inside).
pub async fn scope<F: std::future::Future>(who: Who, f: F) -> F::Output {
    WHO.scope(who, f).await
}

/// Origin of a Lektor: `@lektorat:<slug>`, the slug by the same rule as
/// [`crate::docx_import::word_author`] (lower case, umlauts transliterated, at most 40 characters; if nothing
/// remains, `unbekannt` – which is why “konten” rejects this account name).
pub fn lektor_handle(name: &str) -> String {
    let word = crate::docx_import::word_author(name);
    format!("@lektorat:{}", word.strip_prefix("@word:").unwrap_or(&word))
}

/// Session cookie.
pub const SESSION_COOKIE: &str = "baruch_session";

/// Paths that stay reachable without login; plus everything under [`PUBLIC_PREFIX`].
/// Round 39a: `/einstellungen/sprache` too (an anonymous visitor only gets the cookie, see `crate::i18n_switch`).
pub const PUBLIC_PATHS: &[&str] = &["/anmelden", "/static/app.css", "/static/favicon.svg", "/einstellungen/sprache"];

/// Prefix of the widgets the login page loads.
pub const PUBLIC_PREFIX: &str = "/static/js/";

/// Login state per [`AppState`] (never process-wide): the remembered “there are accounts”.
///
/// Only the yes is remembered: accounts are never deleted, so a yes stays true for the lifetime of the data
/// folder. The no is looked up every time (a `SELECT count(*)` on an empty table) – so no
/// stale “no accounts” can let a request through without login, even if an account is created past this
/// slot (other process, test through SQL).
#[derive(Default)]
pub struct AuthState {
    accounts: AtomicBool,
}

impl AuthState {
    /// After the setup: from now on there are accounts.
    pub fn mark_accounts(&self) {
        self.accounts.store(true, Ordering::Relaxed);
    }
}

/// Are there accounts? Without accounts every page stays unchanged. A database error counts as “yes” (safe side:
/// rather demand one login too many than none); it goes to stderr.
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

/// The identity for an account from the store (`kind` from the database: `author`, otherwise Lektor – the safe
/// side, should anything else ever stand there).
pub fn identity_of(account: &baruch_store::Account) -> Identity {
    Identity {
        id: account.id,
        name: account.name.clone(),
        display: account.display.clone(),
        kind: if account.kind == "author" { AccountKind::Author } else { AccountKind::Lektor },
        handle: format!("@lektorat:{}", account.slug),
    }
}

/// Checks account name and display name; returns the slug of the handle or the message code `name_ungueltig`
/// (contract §3.4). A name has 1 to 40 characters without control characters, and its slug must not be `unbekannt`
/// (the name of what the rule leaves nothing of); the display name has at most 80 characters.
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

/// Sentence for a message code from `konten_ui::MESSAGES` in the language of the request; an unknown code stays as
/// it is.
pub fn message(code: &str) -> String {
    crate::konten_ui::MESSAGES
        .iter()
        .find(|(known, _)| *known == code)
        .map_or_else(|| code.to_owned(), |(_, key)| crate::i18n::tc(key, &[]))
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
        let german = crate::i18n::t(crate::i18n::Lang::De, known.1, &[]);
        assert_eq!(
            crate::i18n::sync_scope(crate::i18n::Lang::De, || message(known.0)),
            german,
            "der Satz aus der Tabelle der Oberfläche"
        );
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
