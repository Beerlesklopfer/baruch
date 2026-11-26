// SPDX-License-Identifier: AGPL-3.0-or-later
//! Seiten der Konten (Runde 35, Vertrag §3.1): `GET /anmelden`, `GET /einrichten`, `GET /konto`,
//! `GET /werke/{id}/mitwirkende` und der Inhalt der Einstellungsseite „Konten“ (`settings::PAGES`).
//!
//! Jede Seite trennt das Holen der Daten (dünn, hier) von der Darstellung: [`accounts_html`], [`members_html`] und
//! [`account_html`] sind reine Funktionen über Ansichtsdaten und lassen sich mit erfundenen Daten prüfen. Die
//! schreibenden Wege gehören nicht hierher: Formulare gehen an `auth::routes` (`/anmelden`, `/abmelden`,
//! `/einrichten`, `/konto/passwort`, `/einstellungen/konten/act`) und `roles::routes`
//! (`/werke/{id}/mitwirkende/act`).
//!
//! Annahmen über die Nachbarn (Schnittstelle R35-S1): Die Liste der Konten kommt aus `Store::account_list`, der
//! Schalter „Am eigenen Rechner ohne Anmeldung“ aus `account.me` (`local_open`), die Freigaben aus `member.list`.
//! Scheitert eine dieser Quellen, zeigt die Seite die Meldung statt der Liste.

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

/// Kopfzeilen, an denen ein vorgeschalteter Server zu erkennen ist (Vertrag §0.2). `guard::via_proxy` kennt nur den
/// Rechnernamen; ein Proxy, der `Host` umschreibt, verrät sich allein hierüber.
const FORWARD_HEADERS: [&str; 6] =
    ["forwarded", "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip", "via"];

/// Kommt die Anfrage unmittelbar vom eigenen Rechner (Vertrag §0.2)? Loopback ohne Proxy-Namen
/// (`shutdown::own_machine`) und ohne eine der Kopfzeilen eines vorgeschalteten Servers. Ohne bekannte Gegenstelle
/// nein.
pub fn direct_from_own_machine(state: &AppState, peer: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    crate::shutdown::own_machine(&state.allowed_hosts, peer, headers)
        && !FORWARD_HEADERS.iter().any(|name| headers.contains_key(*name))
}

/// Die Seiten der Konten.
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

/// Anzeigename eines Kontos, sonst sein Name.
fn shown_name(identity: &Identity) -> &str {
    if identity.display.trim().is_empty() { &identity.name } else { &identity.display }
}

/// Meldungen aus der Adresse: nur der Satz aus der Tabelle, nie der Text der Adresse.
fn notices<'a>(mut get: impl FnMut(&str) -> Option<&'a str>) -> (Option<&'static str>, Option<&'static str>) {
    (get("ok").and_then(ok_message), get("fehler").and_then(error_message))
}

/// Zeitpunkt (ISO 8601, UTC) kurz: `2026-11-26 09:15 UTC`.
fn short_time(time: &str) -> String {
    match time.get(..16) {
        Some(head) if time.is_char_boundary(16) => format!("{} UTC", head.replace('T', " ")),
        _ => time.to_owned(),
    }
}

// ---- /anmelden ------------------------------------------------------------------------------------------------

/// `GET /anmelden[?next=…][&ok=abgemeldet]`: ohne Konten 303 `/`; sonst das Formular. Trägt die Anfrage schon eine
/// gültige Sitzung, nennt die Seite das Konto und bietet „Weiter“ an.
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

/// `GET /einrichten`: mit Konten 303 `/einstellungen/konten`; sonst das Formular – nicht vom eigenen Rechner 403 mit
/// Erklärung.
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

/// Ein Werk in „Mein Konto“.
#[derive(Debug, Clone, Serialize)]
pub struct AccountWork {
    pub id: WorkId,
    pub title: String,
}

/// Ansicht „Mein Konto“. `state`: `open` (es gibt keine Konten), `local` (Autor am eigenen Rechner ohne Anmeldung)
/// oder `account` (angemeldet).
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountView {
    pub state: &'static str,
    pub name: String,
    pub display: String,
    /// „Autor“ oder „Lektor“.
    pub role: &'static str,
    /// Herkunft eines Lektors (`@lektorat:<slug>`); der Autor schreibt als „Autor“.
    pub handle: Option<String>,
    pub works: Vec<AccountWork>,
    pub notice: Option<String>,
    pub error: Option<String>,
}

impl AccountView {
    /// Ansicht eines angemeldeten Kontos (ohne Werke und Meldungen).
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

/// „Mein Konto“ als HTML (`konto.html`).
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
            // Ohne Anmeldung leitet schon auth::gate um; `Open` mit Konten gibt es nur in der Anfrage, die das
            // erste Konto anlegt.
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

/// Eine Freigabe (Antwort von `member.list`, Feld `members`).
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

/// Ein Lektor-Konto ohne Freigabe (Antwort von `member.list`, Feld `candidates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Candidate {
    pub account: i64,
    pub name: String,
    pub display: String,
    pub handle: String,
}

/// Ansicht „Mitwirkende“ eines Werks.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MembersView {
    pub work: WorkId,
    pub title: String,
    pub accounts_exist: bool,
    pub members: Vec<Member>,
    pub candidates: Vec<Candidate>,
    pub notice: Option<String>,
    pub error: Option<String>,
    /// Meldung, wenn `member.list` scheitert (dann ohne Listen).
    pub load_error: Option<String>,
}

impl MembersView {
    /// Übernimmt `members` und `candidates` aus der Antwort von `member.list`; Unlesbares zählt als Ladefehler,
    /// statt eine leere Liste vorzutäuschen.
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

/// „Mitwirkende“ als HTML (`mitwirkende.html`).
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

/// `GET /werke/{id}/mitwirkende[?ok=…|?fehler=…]`: nur mit vollen Rechten (für Lektoren sperrt schon `roles` die
/// Route; die Prüfung hier hält die Seite auch dann zu, wenn die Tabelle der Routen sich ändert).
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

/// Ein Konto in der Liste der Einstellungsseite.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountRow {
    pub id: i64,
    pub name: String,
    pub display: String,
    /// `author` oder `lektor`.
    pub kind: String,
    /// „Autor“ oder „Lektor“.
    pub kind_label: &'static str,
    /// Herkunft eines Lektors (`@lektorat:<slug>`); der Autor schreibt als „Autor“.
    pub handle: Option<String>,
    pub active: bool,
    pub created: String,
    pub last_login: Option<String>,
    /// Das Konto, mit dem die Anfrage angemeldet ist.
    pub own: bool,
}

impl AccountRow {
    /// Zeile aus einem Konto des Stores; `own_id` ist das angemeldete Konto.
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

/// Ansicht der Einstellungsseite „Konten“.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AccountsView {
    pub work: Option<WorkId>,
    pub accounts_exist: bool,
    /// Volle Rechte (nur dann die Liste und die Formulare).
    pub allowed: bool,
    pub accounts: Vec<AccountRow>,
    /// „Am eigenen Rechner ohne Anmeldung“ ist eingeschaltet.
    pub local_open: bool,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub load_error: Option<String>,
}

/// Inhalt der Einstellungsseite „Konten“ als HTML (`settings/konten.html`, ohne Rahmen).
pub fn accounts_html(view: &AccountsView) -> Result<String, String> {
    crate::settings::render_template(
        "settings/konten.html",
        context! { min => MIN_PASSWORD_CHARS, max => MAX_PASSWORD_CHARS, ..Value::from(Serde(view)) },
    )
}

/// Steht der Schalter „Am eigenen Rechner ohne Anmeldung“ auf an? Aus `account.me` (`local_open`); lässt sich das
/// nicht lesen, gilt die Vorgabe „aus“.
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

/// Zeichnet die Einstellungsseite „Konten“ (Eintrag in `settings::PAGES`).
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
        for name in FORWARD_HEADERS {
            assert_eq!(name, name.to_lowercase(), "{name}: Kopfzeilennamen vergleicht http klein geschrieben");
        }
        assert_eq!(FORWARD_HEADERS.len(), 6, "die sechs Kopfzeilen aus Vertrag §0.2");
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
