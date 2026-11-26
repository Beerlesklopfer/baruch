// SPDX-License-Identifier: AGPL-3.0-or-later
//! Formulare der Rollen (Runde 35, Vertrag §2.4): `POST /lektorat/act` (Formularweg eines Lektors über
//! `rpc::call`) und `POST /werke/{id}/mitwirkende/act` (Freigeben, Entziehen) – hinter `guard::same_origin`.

use std::sync::Arc;

use axum::Router;

use crate::AppState;

/// Schreibende Routen der Rollen.
pub fn writes() -> Router<Arc<AppState>> {
    // STUB: /lektorat/act und /werke/{id}/mitwirkende/act (Runde 35)
    Router::new()
}
