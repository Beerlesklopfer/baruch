// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rollen (Runde 35, Vertrag §S1.4 und §2): die EINZIGE Stelle der Rechteprüfung.
//!
//! Die Kontoart ist global, die Rolle gilt je Werk ([`role_in`]): Ein Autor-Konto ist Autor in jedem Werk, ein
//! Lektor-Konto ist Lektor nur in freigegebenen Werken (`work_member`), sonst hat es keine Rolle (Werk unsichtbar).
//!
//! Für Lektoren ist alles verboten, was nicht ausdrücklich erlaubt ist: [`check_request`] prüft jede Route gegen
//! [`LEKTOR_ROUTES`] (aufgerufen aus [`crate::auth::gate`]), [`check_rpc`] jede RPC-Methode gegen [`LEKTOR_RPC`]
//! (aufgerufen aus [`crate::rpc::call`], also für `/rpc`, `/act` und `/lektorat/act`). Neue Routen und Methoden
//! künftiger Runden sind damit ohne Zutun für Lektoren gesperrt. Formular-Handler der Module (`/beitraege/act`
//! usw.) rufen ihr Modul direkt und gehen nicht durch `rpc::call` – sie stehen darum nicht in [`LEKTOR_ROUTES`].
//!
//! Schnittstelle R35-S1 (sichere Vorgabe): volle Rechte dürfen alles, alle anderen nichts; „rollen“ füllt die
//! Tabellen und die Prüfung der Freigaben.

pub mod lektor;
pub mod routes;
pub mod rpc;

use axum::http::Method;
use baruch_store::{StoreError, WorkId, WorkSummary};
use serde::Serialize;
use serde_json::Value;

use crate::AppState;
use crate::auth::Who;
use crate::rpc::{FORBIDDEN, RpcError, UNAUTHENTICATED};

/// Rolle in einem Werk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Author,
    Lektor,
}

/// Urteil über eine Anfrage: erlaubt, Umleitung (303) oder abgelehnt (403 mit Grund).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Redirect(String),
    Deny(String),
}

/// Woher eine erlaubte Route bzw. Methode ihr Werk nimmt: gar nicht, aus dem Pfad (`{id}`), aus der Adresse
/// (`?work=`) oder aus dem RPC-Parameter `work`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkFrom {
    None,
    Path,
    Query,
    Param,
}

/// Routen, die ein Lektor aufrufen darf: Methode (`GET` | `POST`), Muster wie im Router, Herkunft des Werks.
/// „rollen“ füllt die Liste (Vertrag §2.1).
// STUB: erlaubte Routen für Lektoren eintragen (Runde 35)
pub const LEKTOR_ROUTES: &[(&str, &str, WorkFrom)] = &[];

/// RPC-Methoden, die ein Lektor aufrufen darf, mit der Herkunft des Werks. „rollen“ füllt die Liste (Vertrag §2.2).
// STUB: erlaubte RPC-Methoden für Lektoren eintragen (Runde 35)
pub const LEKTOR_RPC: &[(&str, WorkFrom)] = &[];

const LEKTOR_NOT_YET: &str = "Lektor-Rechte: noch nicht umgesetzt (Runde 35)";

/// Rolle von `who` im Werk; `None` = keine (Werk unsichtbar, 403).
pub async fn role_in(state: &AppState, who: &Who, work: WorkId) -> Result<Option<Role>, StoreError> {
    if who.full_rights() {
        return Ok(Some(Role::Author));
    }
    // STUB: Lektor-Konto mit Freigabe in `work_member` → Role::Lektor (Runde 35); bis dahin keine Rolle.
    let _ = (state, work);
    Ok(None)
}

/// Die Werke, die `who` sehen darf (für Übersicht und Werkwahl). Systemzwecke (Sicherung, Spiegel) nehmen weiter
/// `Store::work_list`.
pub async fn visible_works(state: &AppState, who: &Who) -> Result<Vec<WorkSummary>, StoreError> {
    if who.full_rights() {
        return state.store.work_list().await;
    }
    // STUB: für Lektoren die freigegebenen Werke (Runde 35); bis dahin sieht, wer keine vollen Rechte hat, keines.
    Ok(Vec::new())
}

/// Die einzige Prüfung einer Route. Volle Rechte → erlaubt; Lektor → nur Treffer in [`LEKTOR_ROUTES`] mit
/// freigegebenem Werk.
pub async fn check_request(state: &AppState, who: &Who, method: &Method, path: &str, query: Option<&str>) -> Verdict {
    if who.full_rights() {
        return Verdict::Allow;
    }
    let _ = (state, method, path, query);
    match who {
        Who::Anonymous => Verdict::Deny("Anmeldung erforderlich.".to_owned()),
        // STUB: Lektor gegen LEKTOR_ROUTES und Freigaben prüfen, Umleitung auf das freigegebene Werk (Runde 35)
        _ => Verdict::Deny(LEKTOR_NOT_YET.to_owned()),
    }
}

/// Die einzige Prüfung einer RPC-Methode, vor jedem Dispatch (auch für unbekannte Namen). Volle Rechte → Ok;
/// ohne Anmeldung [`UNAUTHENTICATED`]; Lektor → nur Methoden aus [`LEKTOR_RPC`], sonst [`FORBIDDEN`]. Darf `params`
/// ergänzen (fehlendes `work` → das kleinste freigegebene Werk).
pub async fn check_rpc(state: &AppState, who: &Who, method: &str, params: &mut Value) -> Result<(), RpcError> {
    if who.full_rights() {
        return Ok(());
    }
    let _ = (state, method, params);
    match who {
        Who::Anonymous => Err(RpcError::new(UNAUTHENTICATED, "Anmeldung erforderlich.")),
        // STUB: Lektor gegen LEKTOR_RPC und Freigaben prüfen, `work` einsetzen (Runde 35)
        _ => Err(RpcError::new(FORBIDDEN, LEKTOR_NOT_YET)),
    }
}

/// Name, unter dem `who` schreibt: ein Lektor unter seinem Handle (`@lektorat:<slug>`), alle anderen als
/// [`crate::mirror::HUMAN_NAME`].
pub fn author_name(who: &Who) -> String {
    match who.lektor() {
        Some(identity) => identity.handle.clone(),
        None => crate::mirror::HUMAN_NAME.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AccountKind, Identity};

    fn account(kind: AccountKind) -> Who {
        Who::Account(Identity {
            id: 7,
            name: "Erika".into(),
            display: "Erika Beispiel".into(),
            kind,
            handle: crate::auth::lektor_handle("Erika"),
        })
    }

    #[test]
    fn a_lektor_writes_under_the_handle_everybody_else_as_the_author() {
        assert_eq!(author_name(&account(AccountKind::Lektor)), "@lektorat:erika", "Herkunft eines Lektors");
        for who in [Who::Open, Who::LocalAuthor, account(AccountKind::Author), Who::Anonymous] {
            assert_eq!(author_name(&who), crate::mirror::HUMAN_NAME, "{who:?} schreibt als Autor wie bisher");
        }
    }
}
