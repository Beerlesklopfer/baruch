// SPDX-License-Identifier: AGPL-3.0-or-later
//! Baruch beenden (Runde 32; Schnittstelle R32-S1, Besitz beenden).
//!
//! [`serve`] bedient den Server und fährt ihn sauber herunter – auf ein Signal des Systems oder wenn der Mensch auf
//! der Seite `/beenden` den Knopf drückt ([`Shutdown::request`]). Beenden ist bewusst **keine** RPC-Methode und kein
//! MCP-Werkzeug: `rpc::call` kennt die Gegenstelle nicht, und nur der eigene Rechner darf beenden ([`own_machine`]).
//!
//! - Seiten: `GET /beenden` (Bestätigung), `POST /beenden` (hinter `guard::same_origin`) – [`pages`], [`writes`].
//! - Auslöser: [`Shutdown`] liegt in [`AppState::shutdown`].

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tokio::sync::watch;

use crate::{AppState, guard};

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

/// Meldung der Routen, solange sie Stub sind.
const NOT_YET: &str = "Beenden über die Oberfläche ist noch nicht umgesetzt (Runde 32).";

/// Kommt die Anfrage vom eigenen Rechner? `true` nur, wenn die Gegenstelle bekannt und Loopback ist, die Anfrage
/// nicht über einen Proxy kam ([`guard::via_proxy`]) und `Host` (falls vorhanden) [`guard::allowed_host`] erfüllt.
/// Ohne bekannte Gegenstelle `false`.
pub fn own_machine(hosts: &guard::AllowedHosts, peer: Option<SocketAddr>, headers: &HeaderMap) -> bool {
    // STUB: Regel fehlt – bis dahin gilt keine Anfrage als vom eigenen Rechner, niemand kann beenden (R32 beenden)
    let _ = (hosts, peer, headers);
    false
}

/// Bedient den Server auf `listener`, bis Strg+C kommt oder [`Shutdown::request`] ausgelöst wird; laufende
/// Antworten gehen zu Ende.
pub async fn serve(listener: tokio::net::TcpListener, state: Arc<AppState>) -> std::io::Result<()> {
    // STUB: SIGTERM und die Windows-Signale, Frist GRACE, Schließen der Datenbank und die Zeile „Baruch beendet.“
    // fehlen – bisher nur Strg+C (wie vor Runde 32) und der Auslöser (R32 beenden)
    let shutdown = state.shutdown.clone();
    // ConnectInfo: MCP erlaubt ohne Token nur Anfragen vom eigenen Rechner; `/beenden` prüft die Gegenstelle.
    axum::serve(listener, crate::router(state).into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = shutdown.requested() => {}
            }
        })
        .await
}

async fn not_yet() -> Response {
    // STUB: Bestätigungsseite, Prüfung own_machine und Auslöser fehlen (R32 beenden)
    (StatusCode::NOT_IMPLEMENTED, NOT_YET).into_response()
}

/// Seiten (GET), gemischt in `lib.rs` bei den Seiten: `GET /beenden`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/beenden", get(not_yet))
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`: `POST /beenden`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/beenden", post(not_yet))
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
}
