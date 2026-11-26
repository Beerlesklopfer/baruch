// SPDX-License-Identifier: AGPL-3.0-or-later
//! Formulare der Anmeldung (Runde 35, Vertrag §1.5): `POST /anmelden`, `/abmelden`, `/einrichten`,
//! `/konto/passwort`, `/einstellungen/konten/act` – alle hinter `guard::same_origin`.
//!
//! Anmelden, Abmelden und Einrichten gibt es nur hier (sie setzen bzw. löschen das Cookie); die übrigen Aktionen
//! gibt es auch als RPC (`super::rpc`), beide Wege rufen dieselben Bausteine ([`create_account`],
//! [`change_password`], [`set_active`]). Kein Typ dieses Moduls leitet `Debug` ab: Die Formulare tragen Passwörter.
//! Meldungen gehen als Code (`?ok=…`, `?fehler=…`, Vertrag §3.4) zurück an die Seite; die Sätze dazu stehen in
//! `konten_ui::MESSAGES`.

use std::sync::Arc;

use axum::extract::State;
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::post;
use axum::{Form, Router};
use baruch_store::{Account, LoginAttempt, NewAccount, Store, StoreError};
use serde::Deserialize;

use super::{gate, password, session, throttle};
use crate::AppState;
use crate::konten_ui::{self, LoginView, SetupView};

/// Meldung für einen gescheiterten Anmeldeversuch – dieselbe für ein falsches Passwort, einen unbekannten Namen und
/// ein gesperrtes Konto (Vertrag §1.1): Die Antwort verrät nicht, welche Konten es gibt.
pub const LOGIN_FAILED: &str = "Name oder Passwort stimmt nicht.";
/// Meldung während der Sperre gegen Durchprobieren.
pub const LOGIN_BLOCKED: &str = "Zu viele Fehlversuche. Bitte warten und dann erneut versuchen.";

/// Seite der Konten in den Einstellungen und „Mein Konto“ (Ziele der Umleitungen).
const ACCOUNTS_PAGE: &str = "/einstellungen/konten";
const OWN_PAGE: &str = "/konto";

/// Warum eine Aktion nicht ging: ein Meldungscode der Oberfläche (Vertrag §3.4) oder ein innerer Fehler.
pub(super) enum Problem {
    Code(&'static str),
    Internal(String),
}

impl From<StoreError> for Problem {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Refused(text) if text == Store::ACCOUNT_TAKEN => Problem::Code("name_vergeben"),
            StoreError::Refused(text) if text == Store::ACCOUNT_LAST_AUTHOR => Problem::Code("letzter_autor"),
            StoreError::Refused(text) if text == Store::ACCOUNT_NOT_FIRST => Problem::Code(NOT_FIRST),
            StoreError::Refused(text) if text == Store::ACCOUNT_NEEDS_AUTHOR => Problem::Code(NEEDS_AUTHOR),
            other => Problem::Internal(other.to_string()),
        }
    }
}

/// Innere Codes ohne Satz in der Tabelle der Oberfläche: Es gibt schon ein Konto bzw. noch keines.
pub(super) const NOT_FIRST: &str = "schon_eingerichtet";
pub(super) const NEEDS_AUTHOR: &str = "nicht_eingerichtet";

/// Prüft das neue Passwort (Länge, Wiederholung, wenn eine mitkommt) und liefert seinen Argon2id-PHC-Text.
async fn new_phc(password: String, repeat: Option<&str>) -> Result<String, Problem> {
    password::policy(&password).map_err(Problem::Code)?;
    if repeat.is_some_and(|repeat| repeat != password) {
        return Err(Problem::Code("passwort_ungleich"));
    }
    password::hash_blocking(password).await.map_err(Problem::Internal)
}

/// Legt ein Konto an: das erste (`first`) als Autor und nur, wenn es noch keines gibt; jedes weitere als Lektor.
pub(super) async fn create_account(
    state: &AppState,
    name: &str,
    display: &str,
    password: String,
    repeat: Option<&str>,
    first: bool,
) -> Result<Account, Problem> {
    let slug = super::check_name(name, display).map_err(Problem::Code)?;
    let phc = new_phc(password, repeat).await?;
    let kind = if first { "author" } else { "lektor" };
    let new = NewAccount { name, slug: &slug, display, kind, password_phc: &phc };
    let account =
        if first { state.store.account_create_first(&new).await? } else { state.store.account_create(&new).await? };
    state.auth.mark_accounts();
    Ok(account)
}

/// Setzt das Passwort eines Kontos und löscht dessen Sitzungen – außer `keep` (Token-Hash der Sitzung, die es
/// gerade ändert).
pub(super) async fn change_password(
    state: &AppState,
    id: i64,
    password: String,
    repeat: Option<&str>,
    keep: Option<&str>,
) -> Result<(), Problem> {
    let phc = new_phc(password, repeat).await?;
    match state.store.account_set_password(id, &phc, keep).await? {
        true => Ok(()),
        false => Err(Problem::Code("unbekanntes_konto")),
    }
}

/// Prüft das bisherige Passwort des Kontos, mit der Sperre gegen Durchprobieren des Kontos: Auch wer eine Sitzung
/// in der Hand hat, kann das Passwort nicht beliebig oft raten.
pub(super) async fn check_current(state: &AppState, account: &super::Identity, current: String) -> Result<(), Problem> {
    let scopes = vec![throttle::account_scope(&account.name)];
    // Erst zählen, dann prüfen (eine Transaktion): Gleichzeitige Versuche kommen nicht an der Sperre vorbei.
    if let LoginAttempt::Blocked(_) = throttle::reserve(state, &scopes).await? {
        return Err(Problem::Code("passwort_falsch"));
    }
    let phc = state.store.account_password(account.id).await?;
    if phc.is_some() && password::verify_blocking(current, phc).await {
        throttle::succeeded(state, &scopes).await?;
        Ok(())
    } else {
        Err(Problem::Code("passwort_falsch"))
    }
}

/// Sperrt oder entsperrt ein Konto (Sperren löscht seine Sitzungen; das letzte aktive Autor-Konto bleibt).
pub(super) async fn set_active(state: &AppState, id: i64, active: bool) -> Result<(), Problem> {
    match state.store.account_set_active(id, active).await? {
        true => Ok(()),
        false => Err(Problem::Code("unbekanntes_konto")),
    }
}

fn internal(e: impl std::fmt::Display) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response()
}

fn back(page: &str, kind: &str, code: &str) -> Response {
    Redirect::to(&format!("{page}?{kind}={code}")).into_response()
}

/// Umleitung nach einer Aktion: Erfolg `?ok=<ok>`, sonst `?fehler=<code>`; innere Fehler 500.
fn outcome(page: &str, ok: &str, result: Result<(), Problem>) -> Response {
    match result {
        Ok(()) => back(page, "ok", ok),
        Err(Problem::Code(NEEDS_AUTHOR)) => Redirect::to("/einrichten").into_response(),
        Err(Problem::Code(code)) => back(page, "fehler", code),
        Err(Problem::Internal(e)) => internal(e),
    }
}

fn secure(state: &AppState, parts: &Parts) -> bool {
    crate::guard::via_proxy(&state.allowed_hosts, &parts.headers)
}

#[derive(Deserialize)]
struct LoginForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    next: String,
}

/// `POST /anmelden`: richtiges Passwort → Sitzung, Cookie und 303 auf das sichere Ziel; sonst die Anmeldeseite mit
/// 401, während der Sperre (und mit dem Fehlversuch, der sie auslöst) 429 mit `Retry-After`.
async fn login(State(state): State<Arc<AppState>>, parts: Parts, Form(form): Form<LoginForm>) -> Response {
    if !super::accounts_exist(&state).await {
        return Redirect::to("/").into_response();
    }
    let LoginForm { name, password, next } = form;
    let page = |status: StatusCode, error: &str, wait: Option<u64>| {
        let view =
            LoginView { name: name.clone(), next: next.clone(), error: Some(error.to_owned()), retry_after: wait };
        let mut response = konten_ui::login_page(status, &view);
        if let Some(wait) = wait {
            response.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(wait));
        }
        response
    };
    let scopes = throttle::scopes(&state, &name, gate::peer(&parts), &parts.headers);
    // Der Versuch zählt schon jetzt, in einer Transaktion mit der Prüfung der Sperre; ein Erfolg nimmt ihn zurück.
    // Zählte er erst nach der Prüfung, ginge eine Salve gleichzeitiger Versuche an der Sperre vorbei.
    let wait = match throttle::reserve(&state, &scopes).await {
        Ok(LoginAttempt::Blocked(wait)) => return page(StatusCode::TOO_MANY_REQUESTS, LOGIN_BLOCKED, Some(wait)),
        Ok(LoginAttempt::Reserved(wait)) => wait,
        Err(e) => return internal(e),
    };
    let found = match state.store.account_for_login(&name).await {
        Ok(found) => found,
        Err(e) => return internal(e),
    };
    let (account, phc) = match found {
        Some((account, phc)) => (Some(account), Some(phc)),
        None => (None, None),
    };
    // Ein überlanges Passwort kann zu keinem Konto gehören; geprüft wird trotzdem (gleiche Dauer), nur gegen nichts.
    let fits = password.chars().count() <= password::MAX_PASSWORD_CHARS;
    let matches = password::verify_blocking(if fits { password } else { String::new() }, phc).await && fits;
    let Some(account) = account.filter(|account| matches && account.active) else {
        return match wait {
            0 => page(StatusCode::UNAUTHORIZED, LOGIN_FAILED, None),
            wait => page(StatusCode::TOO_MANY_REQUESTS, LOGIN_FAILED, Some(wait)),
        };
    };
    if let Err(e) = throttle::succeeded(&state, &scopes).await {
        return internal(e);
    }
    let token = match session::start(&state, account.id).await {
        Ok(token) => token,
        Err(e) => return internal(e),
    };
    let mut response = Redirect::to(konten_ui::safe_next(&next)).into_response();
    session::set_cookie(&mut response, &session::cookie(&token, secure(&state, &parts)));
    response
}

/// `POST /abmelden`: löscht die Sitzung und das Cookie.
async fn logout(State(state): State<Arc<AppState>>, parts: Parts) -> Response {
    if !super::accounts_exist(&state).await {
        return Redirect::to("/").into_response();
    }
    if let Some(token) = session::token_from(&parts.headers)
        && let Err(e) = state.store.session_delete(&Store::session_token_hash(token)).await
    {
        return internal(e);
    }
    let mut response = Redirect::to("/anmelden?ok=abgemeldet").into_response();
    session::set_cookie(&mut response, &session::clear_cookie(secure(&state, &parts)));
    response
}

#[derive(Deserialize)]
struct SetupForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    display: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    password2: String,
}

/// `POST /einrichten`: das erste Konto (Autor) – nur wenn es noch keines gibt und die Anfrage vom eigenen Rechner
/// kommt ([`gate::own_machine`]; ohne bekannte Gegenstelle: nein). Danach ist der Autor angemeldet.
async fn setup(State(state): State<Arc<AppState>>, parts: Parts, Form(form): Form<SetupForm>) -> Response {
    let SetupForm { name, display, password, password2 } = form;
    let page = |status: StatusCode, error: String, allowed: bool| {
        let view = SetupView { name: name.clone(), display: display.clone(), error: Some(error), allowed };
        konten_ui::setup_page(status, &view)
    };
    if super::accounts_exist(&state).await {
        return page(StatusCode::FORBIDDEN, Store::ACCOUNT_NOT_FIRST.to_owned(), false);
    }
    if !gate::own_machine(&state, &parts) {
        return page(StatusCode::FORBIDDEN, super::message("nicht_eigener_rechner"), false);
    }
    let account = match create_account(&state, &name, &display, password, Some(&password2), true).await {
        Ok(account) => account,
        Err(Problem::Code(NOT_FIRST)) => {
            return page(StatusCode::FORBIDDEN, Store::ACCOUNT_NOT_FIRST.to_owned(), false);
        }
        Err(Problem::Code(code)) => return page(StatusCode::UNPROCESSABLE_ENTITY, super::message(code), true),
        Err(Problem::Internal(e)) => return internal(e),
    };
    let token = match session::start(&state, account.id).await {
        Ok(token) => token,
        Err(e) => return internal(e),
    };
    let mut response = back(ACCOUNTS_PAGE, "ok", "konto_angelegt");
    session::set_cookie(&mut response, &session::cookie(&token, secure(&state, &parts)));
    response
}

#[derive(Deserialize)]
struct OwnPasswordForm {
    #[serde(default)]
    current: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    password2: String,
}

/// `POST /konto/passwort`: eigenes Passwort ändern (mit dem bisherigen); alle anderen Sitzungen des Kontos enden.
async fn own_password(State(state): State<Arc<AppState>>, Form(form): Form<OwnPasswordForm>) -> Response {
    let who = super::current();
    let Some(identity) = who.identity() else {
        // Ohne Konten und mit der Ausnahme am eigenen Rechner gibt es kein eigenes Konto.
        return back(OWN_PAGE, "fehler", "unbekanntes_konto");
    };
    let result = async {
        check_current(&state, identity, form.current).await?;
        let keep = session::current_hash();
        change_password(&state, identity.id, form.password, Some(&form.password2), keep.as_deref()).await
    }
    .await;
    outcome(OWN_PAGE, "passwort_geaendert", result)
}

#[derive(Deserialize)]
struct AccountsForm {
    #[serde(default)]
    action: String,
    id: Option<i64>,
    #[serde(default)]
    name: String,
    #[serde(default)]
    display: String,
    #[serde(default)]
    password: String,
    #[serde(default)]
    password2: String,
    #[serde(default)]
    open: String,
}

/// `POST /einstellungen/konten/act`: Konten verwalten – nur mit vollen Rechten (für Lektoren sperrt schon
/// `roles` die Route; hier steht die Prüfung ein zweites Mal).
async fn accounts(State(state): State<Arc<AppState>>, Form(form): Form<AccountsForm>) -> Response {
    let who = super::current();
    if !who.full_rights() {
        return (StatusCode::FORBIDDEN, [("x-baruch-abgelehnt", "rolle")], "Abgelehnt: Das darf nur der Autor.")
            .into_response();
    }
    let AccountsForm { action, id, name, display, password, password2, open } = form;
    let unknown = || Err(Problem::Code("unbekanntes_konto"));
    match action.as_str() {
        // Weitere Konten sind immer Lektoren (Vertrag 0.4).
        "create" => {
            let result = create_account(&state, &name, &display, password, Some(&password2), false).await.map(|_| ());
            outcome(ACCOUNTS_PAGE, "konto_angelegt", result)
        }
        "deactivate" | "activate" => {
            let active = action == "activate";
            let result = match id {
                Some(id) => set_active(&state, id, active).await,
                None => unknown(),
            };
            outcome(ACCOUNTS_PAGE, if active { "konto_entsperrt" } else { "konto_gesperrt" }, result)
        }
        "reset_password" => {
            let result = match id {
                Some(id) => {
                    // Setzt der Autor sein eigenes Passwort zurück, bleibt die Sitzung, in der er es tut.
                    let own = who.identity().is_some_and(|identity| identity.id == id);
                    let keep = if own { session::current_hash() } else { None };
                    change_password(&state, id, password, Some(&password2), keep.as_deref()).await
                }
                None => unknown(),
            };
            outcome(ACCOUNTS_PAGE, "passwort_geaendert", result)
        }
        "local_open" => {
            let open = open == "1";
            let result = state.store.set_auth_local_open(open).await.map_err(Problem::from);
            outcome(ACCOUNTS_PAGE, if open { "lokal_offen" } else { "lokal_anmelden" }, result)
        }
        _ => (StatusCode::UNPROCESSABLE_ENTITY, "Unbekannte Aktion.").into_response(),
    }
}

/// Schreibende Routen der Anmeldung.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/anmelden", post(login))
        .route("/abmelden", post(logout))
        .route("/einrichten", post(setup))
        .route("/konto/passwort", post(own_password))
        .route("/einstellungen/konten/act", post(accounts))
}
