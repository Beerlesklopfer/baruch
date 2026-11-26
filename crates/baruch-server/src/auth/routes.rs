// SPDX-License-Identifier: AGPL-3.0-or-later
//! Formulare der Anmeldung (Runde 35, Vertrag §1.5): `POST /anmelden`, `/abmelden`, `/einrichten`,
//! `/konto/passwort`, `/einstellungen/konten/act` – alle hinter `guard::same_origin`.

use std::sync::Arc;

use axum::Router;

use crate::AppState;

/// Schreibende Routen der Anmeldung.
pub fn writes() -> Router<Arc<AppState>> {
    // STUB: Anmelden, Abmelden, Einrichten, Passwort ändern, Konten verwalten (Runde 35)
    Router::new()
}
