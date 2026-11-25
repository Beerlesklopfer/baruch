// SPDX-License-Identifier: AGPL-3.0-or-later
//! Baruch beenden (Runde 32; Schnittstelle R32-S1, Besitz beenden).
//!
//! [`serve`] bedient den Server und fährt ihn sauber herunter – auf ein Signal des Systems oder wenn der Mensch auf
//! der Seite `/beenden` den Knopf drückt ([`Shutdown::request`]). Beenden ist bewusst **keine** RPC-Methode und kein
//! MCP-Werkzeug: `rpc::call` kennt die Gegenstelle nicht, und nur der eigene Rechner darf beenden ([`own_machine`]).
//!
//! - Seiten: `GET /beenden` (Bestätigung), `POST /beenden` (hinter `guard::same_origin`) – [`pages`], [`writes`].
//! - Auslöser: [`Shutdown`] liegt in [`AppState::shutdown`].
//! - Signale: `shutdown/signals.rs` (Strg+C, Unix `SIGTERM`, Windows Strg+Pause, Fenster schließen, Herunterfahren).
//! - Ablauf nach dem Auslöser: [`serve_until`]. Vorlagen `templates/shutdown.html` (Rückfrage, 403-Erklärung) und
//!   `templates/shutdown_done.html` (Abschlussseite ohne Verweis auf Dateien des Servers).
//! - Tests: unten (Auslöser, Tabelle zu [`own_machine`]) und `tests/round32_beenden.rs`.

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

/// Auslöser für das Herunterfahren: einmal verlangt, bleibt es verlangt. Kopien teilen denselben Zustand.
#[derive(Debug, Clone)]
pub struct Shutdown {
    requested: Arc<watch::Sender<bool>>,
}

impl Shutdown {
    pub fn new() -> Self {
        Self { requested: Arc::new(watch::channel(false).0) }
    }

    /// Verlangt das Herunterfahren; mehrfaches Verlangen schadet nicht.
    pub fn request(&self) {
        self.requested.send_replace(true);
    }

    pub fn is_requested(&self) -> bool {
        *self.requested.borrow()
    }

    /// Kehrt zurück, sobald das Herunterfahren verlangt ist (sofort, wenn es das schon war).
    pub async fn requested(&self) {
        let mut receiver = self.requested.subscribe();
        // Der Sender lebt in `self`, solange dieser Aufruf wartet; ein Fehler (Sender weg) kann darum nicht auftreten.
        let _ = receiver.wait_for(|requested| *requested).await;
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

/// Längste Wartezeit auf laufende Antworten nach dem Auslöser; offene Ströme (Live-Chat) halten das Beenden nicht
/// länger auf.
pub const GRACE: Duration = Duration::from_secs(10);

/// Verzögerung zwischen der Antwort auf `POST /beenden` und dem Auslöser, damit die Antwort den Browser erreicht.
pub const REPLY_DELAY: Duration = Duration::from_millis(500);

/// Längste Wartezeit auf das Schließen der Datenbank nach der Frist: Hält eine abgeschnittene Anfrage noch eine
/// Verbindung zur Datenbank, soll das Beenden daran nicht hängen (SQLite übersteht auch das harte Ende).
pub const CLOSE_WAIT: Duration = Duration::from_secs(5);

/// Satz der Abschlussseite (auch ohne JavaScript sichtbar); Tests und Doku nennen ihn wörtlich.
pub const DONE_TEXT: &str = "Baruch ist beendet – dieses Fenster kann geschlossen werden";

/// Letzte Zeile auf der Standardausgabe nach dem sauberen Herunterfahren.
pub const GOODBYE: &str = "Baruch beendet.";

/// Kommt die Anfrage vom eigenen Rechner? `true` nur, wenn die Gegenstelle bekannt und Loopback ist, die Anfrage
/// nicht über einen Proxy kam ([`guard::via_proxy`]) und `Host` (falls vorhanden) [`guard::allowed_host`] erfüllt.
/// Ohne bekannte Gegenstelle `false`.
///
/// Ein Reverse-Proxy auf demselben Rechner verbindet sich ebenfalls über Loopback; ihn verrät nur der `Host` –
/// darum zählt ein zusätzlich erlaubter Rechnername (`--allow-host`) nie als eigener Rechner, und sind solche
/// Namen gesetzt, auch eine Anfrage ohne `Host` nicht.
pub fn own_machine(hosts: &guard::AllowedHosts, peer: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    let Some(peer) = peer else { return false };
    // `to_canonical`: Ein Socket für beide Familien meldet IPv4-Gegenstellen als `::ffff:127.0.0.1`.
    if !peer.ip().to_canonical().is_loopback() || guard::via_proxy(hosts, headers) {
        return false;
    }
    match headers.get(header::HOST).map(|host| host.to_str()) {
        None => true,
        Some(Ok(host)) => guard::allowed_host(host),
        Some(Err(_)) => false,
    }
}

/// Bedient den Server auf `listener`, bis ein Signal des Systems kommt ([`system_signal`]) oder
/// [`Shutdown::request`] ausgelöst wird. Danach gehen laufende Antworten zu Ende (höchstens [`GRACE`]), die
/// Datenbank wird geschlossen, und auf der Standardausgabe steht [`GOODBYE`]. `main` endet danach mit Exit 0.
pub async fn serve(listener: tokio::net::TcpListener, state: Arc<AppState>) -> std::io::Result<()> {
    serve_with(listener, state, system_signal()).await
}

/// Wie [`serve`], mit einer schon eingerichteten Signalquelle: `main` ruft [`system_signal`] auf, bevor es „Baruch
/// läuft“ ausgibt, damit ein Signal gleich nach dieser Zeile schon sauber herunterfährt (Integration Runde 32).
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

/// Wie [`serve`], mit eigener Frist und eigener Signalquelle (Tests: kurze Frist, `std::future::pending()`), und
/// ohne die Schlusszeile.
///
/// Ablauf: Der Auslöser (Signal oder [`Shutdown::request`]) beendet das Annehmen neuer Verbindungen; ruhende
/// Verbindungen schließen sofort, laufende Anfragen werden zu Ende beantwortet. Ist nach `grace` noch eine
/// Verbindung offen (ein Strom wie der Live-Chat endet nie von selbst), wird nicht länger gewartet. Zuletzt wird
/// die Datenbank geschlossen: SQLite schreibt dabei das Protokoll (`-wal`) in die Datei zurück.
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
    // ConnectInfo: MCP erlaubt ohne Token nur Anfragen vom eigenen Rechner; `/beenden` prüft die Gegenstelle.
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
            // Der Server endete ohne Auslöser (Fehler beim Bedienen): Dann gibt es keine Frist.
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

/// Schließt die Datenbank (höchstens [`CLOSE_WAIT`]); ein Fehlschlag steht auf stderr und hält das Ende nicht auf.
async fn close_database(state: &AppState) {
    if tokio::time::timeout(CLOSE_WAIT, state.store.pool().close()).await.is_err() {
        eprintln!(
            "Beenden: Die Datenbank ließ sich in {} s nicht schließen (eine Anfrage hält sie noch); SQLite holt \
             das beim nächsten Start nach.",
            CLOSE_WAIT.as_secs()
        );
    }
}

/// Eigene Template-Umgebung der beiden Seiten (die der Arbeitsseite bleibt unberührt).
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
        // Nie aus dem Zwischenspeicher: Die Rückfrage hängt von der Gegenstelle ab, die Abschlussseite vom Auslöser.
        Ok(html) => {
            (status, [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))], Html(html)).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage {name}: {e}")).into_response(),
    }
}

/// Die Rückfrage (`allowed`) bzw. die Erklärung, warum hier nicht beendet werden kann (403, ohne Formular).
fn confirm_page(request: &Request, allowed: bool) -> Response {
    let settings = Settings::from_cookie_header(request.headers().get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let status = if allowed { StatusCode::OK } else { StatusCode::FORBIDDEN };
    // „Abbrechen“ führt zum zuletzt geöffneten Werk (`settings.work`, Cookie) zurück.
    let ctx = context! { settings => Jinja::from(Serde(settings)), allowed => allowed };
    render(status, "shutdown.html", ctx)
}

fn from_own_machine(state: &AppState, request: &Request) -> bool {
    let peer = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|info| info.0);
    own_machine(&state.allowed_hosts, peer, request.headers())
}

/// `GET /beenden`: Rückfrage mit Formular; nicht vom eigenen Rechner 403 mit Erklärung.
async fn confirm(State(state): State<Arc<AppState>>, request: Request) -> Response {
    confirm_page(&request, from_own_machine(&state, &request))
}

/// `POST /beenden` (hinter `guard::same_origin`): nur vom eigenen Rechner. Antwortet mit der in sich
/// geschlossenen Abschlussseite und verlangt [`REPLY_DELAY`] später das Herunterfahren.
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

/// Seiten (GET), gemischt in `lib.rs` bei den Seiten: `GET /beenden`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/beenden", get(confirm))
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`: `POST /beenden`.
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
        /// (Namen des Proxys, Gegenstelle, Host, erwartet, Begründung)
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
