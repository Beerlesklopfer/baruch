// SPDX-License-Identifier: AGPL-3.0-or-later
//! Formulare der Rollen (Runde 35, Vertrag §2.4): `POST /lektorat/act` (Formularweg eines Lektors über
//! `rpc::call`) und `POST /werke/{id}/mitwirkende/act` (Freigeben, Entziehen) – hinter `guard::same_origin`.
//!
//! `POST /lektorat/act` nimmt die Felder `method`, `work`, `key`, `node`, `id`, `body`, `note`, `kind`, `quote`,
//! `confirm`, `back` (alles Text) und ruft damit [`crate::rpc::call`] – dort entscheidet
//! [`crate::roles::check_rpc`], dieselbe Prüfung wie für `/rpc`. Angenommen werden nur Methoden aus
//! [`crate::roles::LEKTOR_RPC`]: Das Formular ist kein zweiter Weg zu Methoden, die es über Formulare bewusst nicht
//! gibt (auch nicht für den Autor). Danach 303 zurück: auf den Abschnitt (`view::node_url`, nach einem Vorschlag auf
//! die neue Fassung), mit `back=befunde` auf `/werke/{work}/befunde`, mit `back=post` auf die Beitragsseite. Fehler:
//! 403 „Abgelehnt: …“ mit `x-baruch-abgelehnt: rolle` bei [`FORBIDDEN`], 500 bei internen, sonst 422 „Fehler: …“.
//!
//! `POST /werke/{id}/mitwirkende/act` (`action` = `grant` | `revoke`, `account`) ruft `member.grant` bzw.
//! `member.revoke` über [`crate::rpc::call`] und leitet auf `/werke/{id}/mitwirkende?ok=freigegeben|entzogen` bzw.
//! `?fehler=unbekanntes_konto|kein_lektor` um (Codes aus `konten_ui::MESSAGES`).

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::post;
use baruch_store::WorkId;
use serde::Deserialize;
use serde_json::{Value, json};

use super::rpc::GrantRefusal;
use crate::rpc::{FORBIDDEN, INTERNAL, RpcError, UNAUTHENTICATED};
use crate::view::Settings;
use crate::{AppState, auth, rpc};

/// Schreibende Routen der Rollen.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/lektorat/act", post(lektorat_act)).route("/werke/{id}/mitwirkende/act", post(members_act))
}

/// Antwort auf einen RPC-Fehler eines Formulars.
fn failure(error: &RpcError) -> Response {
    match error.code {
        FORBIDDEN => {
            (StatusCode::FORBIDDEN, [("x-baruch-abgelehnt", "rolle")], format!("Abgelehnt: {}", error.message))
                .into_response()
        }
        UNAUTHENTICATED => (StatusCode::UNAUTHORIZED, error.message.clone()).into_response(),
        INTERNAL => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", error.message)).into_response(),
        _ => (StatusCode::UNPROCESSABLE_ENTITY, format!("Fehler: {}", error.message)).into_response(),
    }
}

/// Zahl aus einem Formularfeld; leer oder fehlend → `Ok(None)`, anderes als Ziffern → `Err` mit dem Feldnamen.
fn number(form: &HashMap<String, String>, name: &str) -> Result<Option<i64>, String> {
    match form.get(name).map(|v| v.trim()).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(value) => value.parse().map(Some).map_err(|_| format!("{name}: Zahl erwartet")),
    }
}

/// RPC-Parameter aus den Formularfeldern: Zahlen als Zahlen, das Kästchen `confirm` (`1` | `on` | `true`) als
/// `true`, Texte unverändert. Unbekannte Felder werden übergangen.
fn params_of(form: &HashMap<String, String>) -> Result<Value, String> {
    let mut params = json!({});
    for name in ["work", "key", "node", "id"] {
        if let Some(value) = number(form, name)? {
            params[name] = json!(value);
        }
    }
    for name in ["body", "note", "kind", "quote"] {
        if let Some(value) = form.get(name) {
            params[name] = json!(value);
        }
    }
    if matches!(form.get("confirm").map(String::as_str), Some("1" | "on" | "true")) {
        params["confirm"] = json!(true);
    }
    Ok(params)
}

/// Wohin es nach der Aktion geht.
fn back_url(form: &HashMap<String, String>, method: &str, work: WorkId, answer: &Value) -> String {
    let key = answer["key"].as_i64().or_else(|| number(form, "key").ok().flatten());
    match form.get("back").map(String::as_str) {
        Some("befunde") => return format!("/werke/{work}/befunde"),
        // Nach dem Löschen gibt es die Beitragsseite nicht mehr: dann zurück zum Abschnitt.
        Some("post") if method != "post.delete" => {
            if let Some(id) = answer["id"].as_i64().or_else(|| number(form, "id").ok().flatten()) {
                return crate::posts::post_url(work, id);
            }
        }
        _ => {}
    }
    let rev = match method {
        "revision.propose" => answer["rev"].as_u64().and_then(|r| u32::try_from(r).ok()),
        _ => None,
    };
    match key {
        Some(key) => crate::view::node_url(work, key, rev, false, "", ""),
        None if method == "finding.add" => format!("/werke/{work}/befunde"),
        None => format!("/?work={work}"),
    }
}

/// `POST /lektorat/act`: siehe Modulkopf.
async fn lektorat_act(State(state): State<Arc<AppState>>, Form(form): Form<HashMap<String, String>>) -> Response {
    let who = auth::current();
    let method = form.get("method").map(String::as_str).unwrap_or_default();
    if !super::LEKTOR_RPC.iter().any(|(name, _)| *name == method) {
        // Dieselbe Antwort wie `check_rpc` sie einem Lektor gäbe; der Autor hat für alles andere `/act` und `/rpc`.
        return failure(&super::author_only(method));
    }
    let mut params = match params_of(&form) {
        Ok(params) => params,
        Err(message) => return (StatusCode::UNPROCESSABLE_ENTITY, format!("Fehler: {message}")).into_response(),
    };
    // Das Werk steht danach fest (für die Umleitung): das genannte, sonst das kleinste freigegebene – nie das
    // Vorgabewerk der Instanz. Ob es freigegeben ist, prüft `check_rpc` in `rpc::call`.
    let work = match params["work"].as_i64() {
        Some(work) => work,
        None if who.full_rights() => match state.default_work().await {
            Ok(work) => work,
            Err(e) => return failure(&e.into()),
        },
        None => match super::granted_works(&state, &who).await {
            Ok(granted) => match granted.first() {
                Some(work) => *work,
                None => return failure(&RpcError::new(FORBIDDEN, super::NO_WORK)),
            },
            Err(e) => return failure(&e.into()),
        },
    };
    params["work"] = json!(work);
    match rpc::call(&state, &Settings::default(), method, params).await {
        Ok(answer) => Redirect::to(&back_url(&form, method, work, &answer)).into_response(),
        Err(error) => failure(&error),
    }
}

#[derive(Debug, Deserialize)]
struct MembersForm {
    action: String,
    account: Option<String>,
}

/// `POST /werke/{id}/mitwirkende/act`: siehe Modulkopf.
async fn members_act(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Form(form): Form<MembersForm>,
) -> Response {
    let (method, done) = match form.action.as_str() {
        "grant" => ("member.grant", "freigegeben"),
        "revoke" => ("member.revoke", "entzogen"),
        _ => return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response(),
    };
    // Die Rechte prüfen die Schicht (Route nicht in LEKTOR_ROUTES) und `rpc::call` (member.* nicht in LEKTOR_RPC);
    // hier noch einmal zuerst, damit vor der Prüfung nichts über Werke und Konten verraten wird.
    if !auth::current().full_rights() {
        return failure(&super::author_only(method));
    }
    let page = format!("/werke/{work}/mitwirkende");
    let refused = |refusal: GrantRefusal| Redirect::to(&format!("{page}?fehler={}", refusal.code())).into_response();
    let Some(account) = form.account.as_deref().and_then(|a| a.trim().parse::<i64>().ok()) else {
        return refused(GrantRefusal::UnknownAccount);
    };
    if let Err(e) = state.store.work_slug(work).await {
        return failure(&e.into());
    }
    match super::rpc::grantable(&state, account).await {
        Ok(Ok(())) => {}
        // Entziehen geht auch bei einem Konto, das (inzwischen) kein Lektor ist – nur ein unbekanntes ist ein Fehler.
        Ok(Err(GrantRefusal::NotLektor)) if method == "member.revoke" => {}
        Ok(Err(refusal)) => return refused(refusal),
        Err(error) => return failure(&error),
    }
    match rpc::call(&state, &Settings::default(), method, json!({ "work": work, "account": account })).await {
        Ok(_) => Redirect::to(&format!("{page}?ok={done}")).into_response(),
        Err(error) => failure(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn form_fields_become_typed_parameters() {
        let params = params_of(&form(&[
            ("method", "post.delete"),
            ("work", "3"),
            ("key", " 12 "),
            ("id", "7"),
            ("body", " Text mit Rand "),
            ("confirm", "on"),
            ("back", "post"),
            ("fremd", "x"),
        ]))
        .unwrap();
        assert_eq!(
            params,
            json!({ "work": 3, "key": 12, "id": 7, "body": " Text mit Rand ", "confirm": true }),
            "Zahlen als Zahlen, das Kästchen als true, Text unverändert; method, back und Unbekanntes sind keine Parameter"
        );
        assert_eq!(params_of(&form(&[("work", ""), ("confirm", "0")])).unwrap(), json!({}), "leer und 0 zählen nicht");
        assert_eq!(
            params_of(&form(&[("work", "drei")])).unwrap_err(),
            "work: Zahl erwartet",
            "ein Werk, das keine Zahl ist, wird nicht stillschweigend zum Vorgabewerk"
        );
    }

    #[test]
    fn the_way_back_follows_back_and_the_answer() {
        let empty = form(&[]);
        assert_eq!(
            back_url(&empty, "revision.propose", 3, &json!({ "key": 12, "rev": 4 })),
            "/?work=3&key=12&rev=4",
            "nach einem Vorschlag zeigt die Seite die neue Fassung"
        );
        assert_eq!(
            back_url(&form(&[("key", "12")]), "post.add", 3, &json!({ "id": 9, "key": 12 })),
            "/?work=3&key=12",
            "nach einem Beitrag zurück zum Abschnitt"
        );
        assert_eq!(
            back_url(&form(&[("back", "befunde"), ("key", "12")]), "finding.add", 3, &json!({})),
            "/werke/3/befunde",
            "back=befunde führt auf die Befundseite des Werks"
        );
        assert_eq!(
            back_url(&empty, "finding.add", 3, &json!({})),
            "/werke/3/befunde",
            "Anmerkung zum Buch: Befundseite"
        );
        assert_eq!(
            back_url(&form(&[("back", "post"), ("id", "9")]), "post.edit", 3, &json!({ "id": 9, "key": 12 })),
            "/beitrag?work=3&id=9",
            "back=post führt auf die Beitragsseite"
        );
        assert_eq!(
            back_url(&form(&[("back", "post"), ("id", "9")]), "post.delete", 3, &json!({ "id": 9, "key": 12 })),
            "/?work=3&key=12",
            "ein gelöschter Beitrag hat keine Seite mehr: zurück zum Abschnitt"
        );
    }
}
