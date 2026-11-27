// SPDX-License-Identifier: AGPL-3.0-or-later
//! Quitting Baruch (Round 32; interface R32-S1, ownership beenden).
//!
//! [`serve`] serves the server and shuts it down cleanly – on a signal of the system or when the human presses
//! the button on the page `/beenden` ([`Shutdown::request`]). Quitting is deliberately **not** an RPC method and no
//! MCP tool: `rpc::call` does not know the peer, and only the own machine may quit ([`own_machine`]).
//!
//! - Pages: `GET /beenden` (confirmation), `POST /beenden` (behind `guard::same_origin`) – [`pages`], [`writes`].
//! - Trigger: [`Shutdown`] lies in [`AppState::shutdown`].
//! - Signals: `shutdown/signals.rs` (Ctrl+C, Unix `SIGTERM`, Windows Ctrl+Break, closing the window, shutting down).
//! - Sequence after the trigger: [`serve_until`]. Templates `templates/shutdown.html` (confirmation question, 403
//!   explanation) and `templates/shutdown_done.html` (final page without a reference to files of the server).
//! - Tests: below (trigger, table for [`own_machine`]) and `tests/round32_beenden.rs`.

use std::future::{Future, IntoFuture};
use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use minijinja::value::Serde;
use minijinja::{Environment, Value as Jinja, context};
use tokio::sync::watch;

use crate::view::Settings;
use crate::{AppState, guard};

mod signals;

pub use signals::{SystemSignal, WINDOWS_CLOSE_GRACE, system_signal};

/// Trigger for the shutdown: once requested, it stays requested. Copies share the same state.
#[derive(Debug, Clone)]
pub struct Shutdown {
    requested: Arc<watch::Sender<bool>>,
}

impl Shutdown {
    pub fn new() -> Self {
        Self { requested: Arc::new(watch::channel(false).0) }
    }

    /// Requests the shutdown; requesting several times does no harm.
    pub fn request(&self) {
        self.requested.send_replace(true);
    }

    pub fn is_requested(&self) -> bool {
        *self.requested.borrow()
    }

    /// Returns as soon as the shutdown is requested (at once if it already was).
    pub async fn requested(&self) {
        let mut receiver = self.requested.subscribe();
        // The sender lives in `self` as long as this call waits; so an error (sender gone) cannot occur.
        let _ = receiver.wait_for(|requested| *requested).await;
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

/// Longest waiting time for running answers after the trigger; open streams (live chat) do not hold up quitting
/// any longer.
pub const GRACE: Duration = Duration::from_secs(10);

/// Delay between the answer to `POST /beenden` and the trigger, so that the answer reaches the browser.
pub const REPLY_DELAY: Duration = Duration::from_millis(500);

/// Longest waiting time for the closing of the database after the grace period: if a cut-off request still holds
/// a connection to the database, quitting is not to hang on it (SQLite also survives the hard end).
pub const CLOSE_WAIT: Duration = Duration::from_secs(5);

/// Sentence of the final page (visible without JavaScript too); tests and documentation name it literally.
pub const DONE_TEXT: &str = "Baruch ist beendet – dieses Fenster kann geschlossen werden";

/// Last line on the standard output after the clean shutdown.
pub const GOODBYE: &str = "Baruch beendet.";

/// Does the request come from the own machine? `true` only if the peer is known and loopback, the request
/// did not come through a proxy ([`guard::via_proxy`]) and `Host` (if present) fulfils [`guard::allowed_host`].
/// Without a known peer `false`.
///
/// A reverse proxy on the same machine also connects through loopback; only the `Host` gives it away –
/// therefore an additionally allowed host name (`--allow-host`) never counts as the own machine, and if such
/// names are set, neither does a request without `Host`.
pub fn own_machine(hosts: &guard::AllowedHosts, peer: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    let Some(peer) = peer else { return false };
    // `to_canonical`: a socket for both families reports IPv4 peers as `::ffff:127.0.0.1`.
    if !peer.ip().to_canonical().is_loopback() || guard::via_proxy(hosts, headers) {
        return false;
    }
    match headers.get(header::HOST).map(|host| host.to_str()) {
        None => true,
        Some(Ok(host)) => guard::allowed_host(host),
        Some(Err(_)) => false,
    }
}

/// Serves the server on `listener` until a signal of the system comes ([`system_signal`]) or
/// [`Shutdown::request`] is triggered. Afterwards running answers come to an end (at most [`GRACE`]), the
/// database is closed, and [`GOODBYE`] stands on the standard output. `main` then ends with exit 0.
pub async fn serve(listener: tokio::net::TcpListener, state: Arc<AppState>) -> std::io::Result<()> {
    serve_with(listener, state, system_signal()).await
}

/// Like [`serve`], with a signal source that is already set up: `main` calls [`system_signal`] before it prints
/// „Baruch läuft“, so that a signal right after this line already shuts down cleanly (integration Round 32).
pub async fn serve_with<S>(listener: tokio::net::TcpListener, state: Arc<AppState>, signal: S) -> std::io::Result<()>
where
    S: Future<Output = SystemSignal> + Send + 'static,
{
    let result = serve_until(listener, state, GRACE, signal).await;
    if result.is_ok() {
        println!("{GOODBYE}");
    }
    result
}

/// Like [`serve`], with its own grace period and its own signal source (tests: short grace period,
/// `std::future::pending()`), and without the closing line.
///
/// Sequence: the trigger (signal or [`Shutdown::request`]) ends the accepting of new connections; idle
/// connections close at once, running requests are answered to the end. If a connection is still open after
/// `grace` (a stream such as the live chat never ends by itself), there is no further waiting. Finally
/// the database is closed: SQLite writes the log (`-wal`) back into the file in doing so.
pub async fn serve_until<S>(
    listener: tokio::net::TcpListener,
    state: Arc<AppState>,
    grace: Duration,
    signal: S,
) -> std::io::Result<()>
where
    S: Future<Output = SystemSignal> + Send + 'static,
{
    let shutdown = state.shutdown.clone();
    let (fired, started) = tokio::sync::oneshot::channel::<Duration>();
    let trigger = async move {
        let limit = tokio::select! {
            signal = signal => {
                println!("Baruch wird beendet ({}) …", signal.name);
                signal.grace_limit
            }
            () = shutdown.requested() => {
                println!("Baruch wird beendet (über die Oberfläche) …");
                None
            }
        };
        let _ = fired.send(limit.map_or(grace, |limit| limit.min(grace)));
    };
    // ConnectInfo: without a token MCP allows only requests from the own machine; `/beenden` checks the peer.
    let server =
        axum::serve(listener, crate::router(state.clone()).into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(trigger)
            .into_future();
    tokio::pin!(server);
    let deadline = async move {
        match started.await {
            Ok(grace) => {
                tokio::time::sleep(grace).await;
                grace
            }
            // The server ended without a trigger (error while serving): then there is no grace period.
            Err(_) => std::future::pending().await,
        }
    };
    let result = tokio::select! {
        result = &mut server => result,
        grace = deadline => {
            eprintln!(
                "Beenden: Nach {} s sind noch Verbindungen offen (etwa ein Live-Chat); sie werden getrennt.",
                grace.as_secs_f32()
            );
            Ok(())
        }
    };
    close_database(&state).await;
    result
}

/// Closes the database (at most [`CLOSE_WAIT`]); a failure stands on stderr and does not hold up the end.
async fn close_database(state: &AppState) {
    if tokio::time::timeout(CLOSE_WAIT, state.store.pool().close()).await.is_err() {
        eprintln!(
            "Beenden: Die Datenbank ließ sich in {} s nicht schließen (eine Anfrage hält sie noch); SQLite holt \
             das beim nächsten Start nach.",
            CLOSE_WAIT.as_secs()
        );
    }
}

/// Own template environment of the two pages (that of the workspace page stays untouched).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("shutdown.html", include_str!("../templates/shutdown.html")),
            ("shutdown_done.html", include_str!("../templates/shutdown_done.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Seite Beenden fehlerhaft");
        }
        env
    })
}

fn render(status: StatusCode, name: &str, ctx: Jinja) -> Response {
    match templates().get_template(name).and_then(|t| t.render(ctx)) {
        // Never from the cache: the confirmation question depends on the peer, the final page on the trigger.
        Ok(html) => {
            (status, [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))], Html(html)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage {name}: {e}")).into_response(),
    }
}

/// The confirmation question (`allowed`) or the explanation why quitting is not possible here (403, without a form).
fn confirm_page(request: &Request, allowed: bool) -> Response {
    let settings = Settings::from_cookie_header(request.headers().get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let status = if allowed { StatusCode::OK } else { StatusCode::FORBIDDEN };
    // „Abbrechen“ leads back to the work opened last (`settings.work`, cookie).
    let ctx = context! { settings => Jinja::from(Serde(settings)), allowed => allowed };
    render(status, "shutdown.html", ctx)
}

fn from_own_machine(state: &AppState, request: &Request) -> bool {
    let peer = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|info| info.0);
    own_machine(&state.allowed_hosts, peer, request.headers())
}

/// `GET /beenden`: confirmation question with a form; not from the own machine 403 with an explanation.
async fn confirm(State(state): State<Arc<AppState>>, request: Request) -> Response {
    confirm_page(&request, from_own_machine(&state, &request))
}

/// `POST /beenden` (behind `guard::same_origin`): only from the own machine. Answers with the
/// self-contained final page and requests the shutdown [`REPLY_DELAY`] later.
async fn quit(State(state): State<Arc<AppState>>, request: Request) -> Response {
    if !from_own_machine(&state, &request) {
        return confirm_page(&request, false);
    }
    let shutdown = state.shutdown.clone();
    tokio::spawn(async move {
        tokio::time::sleep(REPLY_DELAY).await;
        shutdown.request();
    });
    render(StatusCode::OK, "shutdown_done.html", context! { text => DONE_TEXT })
}

/// Pages (GET), merged in `lib.rs` with the pages: `GET /beenden`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/beenden", get(confirm))
}

/// Forms (POST), merged in `lib.rs` in the writes router behind `guard::same_origin`: `POST /beenden`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/beenden", post(quit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_stays_requested_and_copies_share_it() {
        let shutdown = Shutdown::new();
        let copy = shutdown.clone();
        assert!(!shutdown.is_requested(), "frisch angelegt ist nichts verlangt");
        copy.request();
        assert!(shutdown.is_requested(), "eine Kopie löst denselben Auslöser aus (AppState und serve teilen ihn)");
        copy.request();
        assert!(shutdown.is_requested(), "mehrfaches Verlangen schadet nicht");
        assert!(!Shutdown::default().is_requested(), "ein neuer Auslöser hat mit dem alten nichts zu tun");
    }

    #[tokio::test]
    async fn waiting_returns_once_requested_and_at_once_afterwards() {
        let shutdown = Shutdown::new();
        let waiting = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { shutdown.requested().await }
        });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished(), "ohne Auslöser wartet requested()");
        shutdown.request();
        tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("nach dem Auslöser kehrt requested() zurück")
            .expect("der wartende Task endet ohne Panik");
        tokio::time::timeout(Duration::from_secs(5), shutdown.requested())
            .await
            .expect("war es schon verlangt, kehrt requested() sofort zurück");
    }

    fn with_host(host: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(host) = host {
            headers.insert(header::HOST, HeaderValue::from_str(host).unwrap());
        }
        headers
    }

    #[test]
    fn only_a_loopback_peer_without_a_proxy_name_is_the_own_machine() {
        let none = guard::AllowedHosts::default();
        let proxy = guard::AllowedHosts::parse("baruch.example.org").unwrap();
        let v4: SocketAddr = "127.0.0.1:40000".parse().unwrap();
        let v6: SocketAddr = "[::1]:40000".parse().unwrap();
        let mapped: SocketAddr = "[::ffff:127.0.0.1]:40000".parse().unwrap();
        let lan: SocketAddr = "192.168.7.20:40000".parse().unwrap();
        let lan6: SocketAddr = "[2001:db8::7]:40000".parse().unwrap();
        /// (names of the proxy, peer, host, expected, reason)
        type Case<'a> = (&'a guard::AllowedHosts, Option<SocketAddr>, Option<&'a str>, bool, &'a str);
        let table: [Case; 16] = [
            (
                &none,
                Some(v4),
                Some("127.0.0.1:8765"),
                true,
                "Loopback v4, Host ist die Adresse: der Browser am Rechner",
            ),
            (&none, Some(v4), Some("localhost:8765"), true, "localhost zeigt nur auf den eigenen Rechner"),
            (&none, Some(v6), Some("[::1]:8765"), true, "Loopback v6"),
            (&none, Some(mapped), Some("127.0.0.1"), true, "v4 über einen Socket für beide Familien"),
            (&none, Some(v4), None, true, "ohne Host und ohne Proxy-Namen (HTTP/1.0, Tests) zählt die Gegenstelle"),
            (&proxy, Some(v4), Some("127.0.0.1:8765"), true, "Proxy-Namen sind gesetzt, die Anfrage kam aber direkt"),
            (&none, Some(lan), Some("192.168.7.20:8765"), false, "fremde Adresse (Betrieb mit --listen im Heimnetz)"),
            (&none, Some(lan6), Some("[2001:db8::7]:8765"), false, "fremde Adresse v6"),
            (&none, Some(lan), None, false, "fremde Adresse ohne Host"),
            (&proxy, Some(v4), Some("baruch.example.org"), false, "Loopback, aber über den Namen des Proxys"),
            (&proxy, Some(v4), Some("BARUCH.example.org:443"), false, "Proxy-Name, Schreibweise und Port beliebig"),
            (&proxy, Some(v6), Some("baruch.example.org"), false, "Proxy-Name über v6"),
            (&proxy, Some(v4), None, false, "Proxy-Namen gesetzt und Host fehlt: nicht von direkt unterscheidbar"),
            (&none, None, Some("127.0.0.1:8765"), false, "unbekannte Gegenstelle: im Zweifel nicht beenden"),
            (&none, Some(v4), Some("fremd.example.net"), false, "fremder Host-Name (DNS-Rebinding)"),
            (&proxy, Some(v4), Some("fremd.example.net"), false, "fremder Name auch neben erlaubten Proxy-Namen"),
        ];
        for (hosts, peer, host, expected, why) in table {
            assert_eq!(
                own_machine(hosts, peer, &with_host(host)),
                expected,
                "{why} (Gegenstelle {peer:?}, Host {host:?})"
            );
        }
        let mut unreadable = HeaderMap::new();
        unreadable.insert(header::HOST, HeaderValue::from_bytes(b"127.0.0.1\xff").unwrap());
        assert!(
            !own_machine(&none, Some(v4), &unreadable),
            "ein nicht lesbarer Host ist keine Adresse dieses Rechners"
        );
    }
}
