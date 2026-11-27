// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pages of the accounts (Round 35, contract §3.1): `GET /anmelden`, `GET /einrichten`, `GET /konto`,
//! `GET /werke/{id}/mitwirkende` and the content of the settings page „Konten“ (`settings::PAGES`).
//!
//! Every page separates fetching the data (thin, here) from the presentation: [`accounts_html`], [`members_html`]
//! and [`account_html`] are pure functions over view data and can be checked with invented data. The
//! writing paths do not belong here: forms go to `auth::routes` (`/anmelden`, `/abmelden`,
//! `/einrichten`, `/konto/passwort`, `/einstellungen/konten/act`) and `roles::routes`
//! (`/werke/{id}/mitwirkende/act`).
//!
//! Assumptions about the neighbours (interface R35-S1): the list of accounts comes from `Store::account_list`, the
//! switch „Am eigenen Rechner ohne Anmeldung“ from `account.me` (`local_open`), the shares from `member.list`.
//! If one of these sources fails, the page shows the message instead of the list.

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use baruch_store::WorkId;
use minijinja::value::Serde;
use minijinja::{Value, context};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use super::{LoginView, SetupView, error_message, html_response, login_html, ok_message, render_template, setup_page};
use crate::AppState;
use crate::auth::password::{MAX_PASSWORD_CHARS, MIN_PASSWORD_CHARS};
use crate::auth::{AccountKind, Identity, Who};
use crate::view::Settings;

/// Does the request come directly from the own machine (contract §0.2)? Loopback without a proxy name
/// (`shutdown::own_machine`) and without one of the headers of an upstream server (`guard::forwarded` –
/// the same rule as `auth::gate::own_machine`; `guard::via_proxy` knows only the host name, a proxy that rewrites
/// `Host` gives itself away through these headers alone). Without a known peer: no.
pub fn direct_from_own_machine(state: &AppState, peer: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    crate::shutdown::own_machine(&state.allowed_hosts, peer, headers) && !crate::guard::forwarded(headers)
}

/// The pages of the accounts.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/anmelden", get(login))
        .route("/einrichten", get(setup))
        .route("/konto", get(account))
        .route("/werke/{id}/mitwirkende", get(members))
}

fn cookie_settings(headers: &HeaderMap) -> Settings {
    Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))
}

/// Display name of an account, otherwise its name.
fn shown_name(identity: &Identity) -> &str {
    if identity.display.trim().is_empty() { &identity.name } else { &identity.display }
}

/// Messages from the address: only the sentence from the table, never the text of the address.
fn notices<'a>(mut get: impl FnMut(&str) -> Option<&'a str>) -> (Option<&'static str>, Option<&'static str>) {
    (get("ok").and_then(ok_message), get("fehler").and_then(error_message))
}

/// Point in time (ISO 8601, UTC) in short: `2026-11-26 09:15 UTC`.
fn short_time(time: &str) -> String {
    match time.get(..16) {
        Some(head) if time.is_char_boundary(16) => format!("{} UTC", head.replace('T', " ")),
        _ => time.to_owned(),
    }
}

// ---- /anmelden ------------------------------------------------------------------------------------------------

/// `GET /anmelden[?next=…][&ok=abgemeldet]`: without accounts 303 `/`; otherwise the form. If the request already
/// carries a valid session, the page names the account and offers „Weiter“.
async fn login(State(state): State<Arc<AppState>>, Query(query): Query<HashMap<String, String>>) -> Response {
    if !crate::auth::accounts_exist(&state).await {
        return Redirect::to("/").into_response();
    }
    let who = crate::auth::current();
    let view = LoginView {
        name: String::new(),
        next: query.get("next").cloned().unwrap_or_default(),
        error: None,
        retry_after: None,
    };
    let (notice, _) = notices(|key| query.get(key).map(String::as_str));
    html_response(StatusCode::OK, login_html(&view, notice, who.identity().map(shown_name)))
}

// ---- /einrichten ----------------------------------------------------------------------------------------------

/// `GET /einrichten`: with accounts 303 `/einstellungen/konten`; otherwise the form – not from the own machine 403
/// with an explanation.
async fn setup(State(state): State<Arc<AppState>>, request: Request) -> Response {
    if crate::auth::accounts_exist(&state).await {
        return Redirect::to("/einstellungen/konten").into_response();
    }
    let peer = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|info| info.0);
    let allowed = direct_from_own_machine(&state, peer, request.headers());
    let status = if allowed { StatusCode::OK } else { StatusCode::FORBIDDEN };
    setup_page(status, &SetupView { name: String::new(), display: String::new(), error: None, allowed })
}

// ---- /konto ---------------------------------------------------------------------------------------------------

/// A work in „Mein Konto“.
#[derive(Debug, Clone, Serialize)]
pub struct AccountWork {
    pub id: WorkId,
    pub title: String,
}

/// View „Mein Konto“. `state`: `open` (there are no accounts), `local` (author on the own machine without signing
/// in) or `account` (signed in).
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountView {
    pub state: &'static str,
    pub name: String,
    pub display: String,
    /// „Autor“ or „Lektor“.
    pub role: &'static str,
    /// Origin of a Lektor (`@lektorat:<slug>`); the author writes as „Autor“.
    pub handle: Option<String>,
    pub works: Vec<AccountWork>,
    pub notice: Option<String>,
    pub error: Option<String>,
}

impl AccountView {
    /// View of a signed-in account (without works and messages).
    pub fn of(identity: &Identity) -> Self {
        let lektor = identity.kind == AccountKind::Lektor;
        AccountView {
            state: "account",
            name: identity.name.clone(),
            display: identity.display.clone(),
            role: if lektor { "Lektor" } else { "Autor" },
            handle: lektor.then(|| identity.handle.clone()),
            ..AccountView::default()
        }
    }
}

/// „Mein Konto“ as HTML (`konto.html`).
pub fn account_html(view: &AccountView, settings: &Settings) -> Result<String, String> {
    render_template(
        "konto.html",
        context! {
            settings => Value::from(Serde(settings)),
            min => MIN_PASSWORD_CHARS,
            max => MAX_PASSWORD_CHARS,
            ..Value::from(Serde(view))
        },
    )
}

/// `GET /konto[?ok=…|?fehler=…]`.
async fn account(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let who = crate::auth::current();
    let mut view = if !crate::auth::accounts_exist(&state).await {
        AccountView { state: "open", ..AccountView::default() }
    } else {
        match &who {
            Who::LocalAuthor => AccountView { state: "local", ..AccountView::default() },
            Who::Account(identity) => {
                let mut view = AccountView::of(identity);
                match crate::roles::visible_works(&state, &who).await {
                    Ok(works) => {
                        view.works = works.into_iter().map(|w| AccountWork { id: w.id, title: w.title }).collect();
                    }
                    Err(e) => view.error = Some(format!("Die Werke lassen sich nicht lesen: {e}")),
                }
                view
            }
            // Without signing in auth::gate already redirects; `Open` with accounts exists only in the request that
            // creates the first account.
            Who::Anonymous | Who::Open => return Redirect::to("/anmelden?next=/konto").into_response(),
        }
    };
    let (notice, error) = notices(|key| query.get(key).map(String::as_str));
    view.notice = notice.map(str::to_owned);
    if view.error.is_none() {
        view.error = error.map(str::to_owned);
    }
    html_response(StatusCode::OK, account_html(&view, &cookie_settings(&headers)))
}

// ---- /werke/{id}/mitwirkende ------------------------------------------------------------------------------------

/// A share (answer of `member.list`, field `members`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Member {
    pub account: i64,
    pub name: String,
    pub display: String,
    pub handle: String,
    pub role: String,
    pub granted: String,
    pub active: bool,
}

/// A Lektor account without a share (answer of `member.list`, field `candidates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Candidate {
    pub account: i64,
    pub name: String,
    pub display: String,
    pub handle: String,
}

/// View „Mitwirkende“ of a work.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MembersView {
    pub work: WorkId,
    pub title: String,
    pub accounts_exist: bool,
    pub members: Vec<Member>,
    pub candidates: Vec<Candidate>,
    pub notice: Option<String>,
    pub error: Option<String>,
    /// Message when `member.list` fails (then without lists).
    pub load_error: Option<String>,
}

impl MembersView {
    /// Takes `members` and `candidates` from the answer of `member.list`; anything unreadable counts as a load error
    /// instead of pretending an empty list.
    pub fn fill(&mut self, answer: &Json) {
        let members = serde_json::from_value::<Vec<Member>>(answer["members"].clone());
        let candidates = serde_json::from_value::<Vec<Candidate>>(answer["candidates"].clone());
        match (members, candidates) {
            (Ok(members), Ok(candidates)) => {
                self.members = members.into_iter().map(|m| Member { granted: short_time(&m.granted), ..m }).collect();
                self.candidates = candidates;
            }
            _ => self.load_error = Some("unerwartete Antwort von member.list".to_owned()),
        }
    }
}

/// „Mitwirkende“ as HTML (`mitwirkende.html`).
pub fn members_html(view: &MembersView, settings: &Settings) -> Result<String, String> {
    render_template(
        "mitwirkende.html",
        context! {
            settings => Value::from(Serde(settings)),
            work => context! { id => view.work, title => view.title.as_str() },
            accounts_exist => view.accounts_exist,
            members => Value::from(Serde(&view.members)),
            candidates => Value::from(Serde(&view.candidates)),
            notice => view.notice.as_deref(),
            error => view.error.as_deref(),
            load_error => view.load_error.as_deref(),
        },
    )
}

/// `GET /werke/{id}/mitwirkende[?ok=…|?fehler=…]`: only with full rights (for Lektors `roles` already locks the
/// route; the check here keeps the page closed even if the table of routes changes).
async fn members(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    if !crate::auth::current().full_rights() {
        return (StatusCode::FORBIDDEN, "Abgelehnt: Freigaben verwaltet nur der Autor.").into_response();
    }
    let title = match state.store.works().await {
        Ok(works) => match works.into_iter().find(|(id, _, _)| *id == work) {
            Some((_, _, title)) => title,
            None => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        },
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let settings = cookie_settings(&headers);
    let (notice, error) = notices(|key| query.get(key).map(String::as_str));
    let mut view = MembersView {
        work,
        title,
        accounts_exist: crate::auth::accounts_exist(&state).await,
        notice: notice.map(str::to_owned),
        error: error.map(str::to_owned),
        ..MembersView::default()
    };
    if view.accounts_exist {
        match crate::rpc::call(&state, &settings, "member.list", json!({ "work": work })).await {
            Ok(answer) => view.fill(&answer),
            Err(e) => view.load_error = Some(e.message),
        }
    }
    html_response(StatusCode::OK, members_html(&view, &settings))
}

// ---- Einstellungen → Konten -------------------------------------------------------------------------------------

/// An account in the list of the settings page.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountRow {
    pub id: i64,
    pub name: String,
    pub display: String,
    /// `author` or `lektor`.
    pub kind: String,
    /// „Autor“ or „Lektor“.
    pub kind_label: &'static str,
    /// Origin of a Lektor (`@lektorat:<slug>`); the author writes as „Autor“.
    pub handle: Option<String>,
    pub active: bool,
    pub created: String,
    pub last_login: Option<String>,
    /// The account with which the request is signed in.
    pub own: bool,
}

impl AccountRow {
    /// Row from an account of the store; `own_id` is the signed-in account.
    pub fn of(account: &baruch_store::Account, own_id: Option<i64>) -> Self {
        let lektor = account.kind == "lektor";
        AccountRow {
            id: account.id,
            name: account.name.clone(),
            display: account.display.clone(),
            kind: account.kind.clone(),
            kind_label: if lektor { "Lektor" } else { "Autor" },
            handle: lektor.then(|| format!("@lektorat:{}", account.slug)),
            active: account.active,
            created: short_time(&account.created),
            last_login: account.last_login.as_deref().map(short_time),
            own: own_id == Some(account.id),
        }
    }
}

/// View of the settings page „Konten“.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountsView {
    pub work: Option<WorkId>,
    pub accounts_exist: bool,
    /// Full rights (only then the list and the forms).
    pub allowed: bool,
    pub accounts: Vec<AccountRow>,
    /// „Am eigenen Rechner ohne Anmeldung“ is switched on.
    pub local_open: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub load_error: Option<String>,
}

/// Content of the settings page „Konten“ as HTML (`settings/konten.html`, without frame).
pub fn accounts_html(view: &AccountsView) -> Result<String, String> {
    crate::settings::render_template(
        "settings/konten.html",
        context! { min => MIN_PASSWORD_CHARS, max => MAX_PASSWORD_CHARS, ..Value::from(Serde(view)) },
    )
}

/// Is the switch „Am eigenen Rechner ohne Anmeldung“ on? From `account.me` (`local_open`); if that cannot be
/// read, the default „aus“ applies.
async fn local_open(state: &AppState) -> bool {
    match crate::auth::rpc::call(state, "account.me", json!({})).await {
        Ok(answer) => answer["local_open"].as_bool().unwrap_or(false),
        Err(_) => false,
    }
}

async fn settings_page(
    state: &AppState,
    work: Option<WorkId>,
    query: &BTreeMap<String, String>,
) -> Result<String, String> {
    let who = crate::auth::current();
    let count = state.store.account_count().await.map_err(|e| e.to_string())?;
    let (notice, error) = notices(|key| query.get(key).map(String::as_str));
    let mut view = AccountsView {
        work,
        accounts_exist: count > 0,
        allowed: who.full_rights(),
        notice: notice.map(str::to_owned),
        error: error.map(str::to_owned),
        ..AccountsView::default()
    };
    if view.accounts_exist && view.allowed {
        let own = who.identity().map(|identity| identity.id);
        match state.store.account_list().await {
            Ok(accounts) => view.accounts = accounts.iter().map(|a| AccountRow::of(a, own)).collect(),
            Err(e) => view.load_error = Some(e.to_string()),
        }
        view.local_open = local_open(state).await;
    }
    accounts_html(&view)
}

/// Draws the settings page „Konten“ (entry in `settings::PAGES`).
pub fn render_settings<'a>(
    state: &'a AppState,
    work: Option<WorkId>,
    query: &'a BTreeMap<String, String>,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(settings_page(state, work, query))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_codes_of_the_table_become_sentences() {
        let query: HashMap<&str, &str> =
            [("ok", "freigegeben"), ("fehler", "<script>alert(1)</script>")].into_iter().collect();
        let (notice, error) = notices(|key| query.get(key).copied());
        assert_eq!(notice, Some("Das Werk ist für das Konto freigegeben."), "ein bekannter Code wird sein Satz");
        assert_eq!(error, None, "ein unbekannter Code ergibt nichts – kein Text aus der Adresse");
        let swapped: HashMap<&str, &str> = [("ok", "passwort_falsch"), ("fehler", "abgemeldet")].into_iter().collect();
        assert_eq!(
            notices(|key| swapped.get(key).copied()),
            (None, None),
            "ein Fehlercode gilt nicht als Erfolg und umgekehrt – sonst ließe sich über die Adresse ein falscher \
             Erfolg vortäuschen"
        );
    }

    #[test]
    fn times_are_shortened_to_the_minute() {
        assert_eq!(short_time("2026-11-26T09:15:42.123Z"), "2026-11-26 09:15 UTC", "Datum und Minute genügen");
        assert_eq!(short_time("gestern"), "gestern", "Unerwartetes bleibt, wie es ist");
        assert_eq!(short_time(""), "", "leer bleibt leer");
    }

    #[test]
    fn a_proxy_header_means_not_the_own_machine() {
        for name in crate::guard::FORWARDED_HEADERS {
            assert_eq!(name, name.to_lowercase(), "{name}: Kopfzeilennamen vergleicht http klein geschrieben");
        }
        assert_eq!(crate::guard::FORWARDED_HEADERS.len(), 6, "die sechs Kopfzeilen aus Vertrag §0.2");
    }

    #[test]
    fn rows_name_the_handle_only_for_lektors() {
        let lektor = baruch_store::Account {
            id: 2,
            name: "Erika".into(),
            slug: "erika".into(),
            display: String::new(),
            kind: "lektor".into(),
            active: false,
            created: "2026-11-20T08:00:00.000Z".into(),
            last_login: None,
        };
        let row = AccountRow::of(&lektor, Some(1));
        assert_eq!(row.handle.as_deref(), Some("@lektorat:erika"), "Herkunft aus dem Slug des Kontos");
        assert_eq!((row.kind_label, row.own, row.active), ("Lektor", false, false), "Art, fremdes Konto, gesperrt");
        let author = baruch_store::Account { id: 1, kind: "author".into(), ..lektor };
        let row = AccountRow::of(&author, Some(1));
        assert_eq!((row.handle, row.kind_label, row.own), (None, "Autor", true), "der Autor schreibt als „Autor“");
    }
}
