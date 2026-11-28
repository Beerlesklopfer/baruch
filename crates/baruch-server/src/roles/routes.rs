// SPDX-License-Identifier: AGPL-3.0-or-later
//! Forms of the roles (Round 35, contract §2.4): `POST /lektorat/act` (form path of a Lektor through
//! `rpc::call`) and `POST /werke/{id}/mitwirkende/act` (release, withdraw) – behind `guard::same_origin`.
//!
//! `POST /lektorat/act` takes the fields `method`, `work`, `key`, `node`, `id`, `body`, `note`, `kind`, `quote`,
//! `confirm`, `back` (all text) and calls [`crate::rpc::call`] with them – there
//! [`crate::roles::check_rpc`] decides, the same check as for `/rpc`. Only methods from
//! [`crate::roles::LEKTOR_RPC`] are accepted: the form is no second path to methods that deliberately do not exist
//! through forms (not for the author either). Afterwards 303 back: to the section (`view::node_url`, after a
//! proposal to the new revision), with `back=befunde` to `/werke/{work}/befunde`, with `back=post` to the post page.
//! Errors: 403 „Abgelehnt: …“ with `x-baruch-abgelehnt: rolle` for [`FORBIDDEN`], 500 for internal ones, otherwise
//! 422 „Fehler: …“.
//!
//! `POST /werke/{id}/mitwirkende/act` (`action` = `grant` | `revoke`, `account`) calls `member.grant` or
//! `member.revoke` through [`crate::rpc::call`] and redirects to `/werke/{id}/mitwirkende?ok=freigegeben|entzogen` or
//! `?fehler=unbekanntes_konto|kein_lektor` (codes from `konten_ui::MESSAGES`).

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

/// Writing routes of the roles.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/lektorat/act", post(lektorat_act)).route("/werke/{id}/mitwirkende/act", post(members_act))
}

/// "Fehler: …" of a form in the language of the request.
fn form_error(detail: &str) -> String {
    crate::i18n::tc("accounts_msg.roles.form_error", &[("detail", detail.into())])
}

/// Answer to an RPC error of a form.
fn failure(error: &RpcError) -> Response {
    match error.code {
        FORBIDDEN => {
            let text = crate::i18n::tc("accounts_msg.gate.refused", &[("detail", (&error.message).into())]);
            (StatusCode::FORBIDDEN, [("x-baruch-abgelehnt", "rolle")], text).into_response()
        }
        UNAUTHENTICATED => (StatusCode::UNAUTHORIZED, error.message.clone()).into_response(),
        INTERNAL => (StatusCode::INTERNAL_SERVER_ERROR, form_error(&error.message)).into_response(),
        _ => (StatusCode::UNPROCESSABLE_ENTITY, form_error(&error.message)).into_response(),
    }
}

/// Number from a form field; empty or missing → `Ok(None)`, anything other than digits → `Err` with the field name.
fn number(form: &HashMap<String, String>, name: &str) -> Result<Option<i64>, String> {
    match form.get(name).map(|v| v.trim()).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some(value) => value.parse().map(Some).map_err(|_| format!("{name}: Zahl erwartet")),
    }
}

/// RPC parameters from the form fields: numbers as numbers, the checkbox `confirm` (`1` | `on` | `true`) as
/// `true`, texts unchanged. Unknown fields are skipped.
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

/// Where to go after the action.
fn back_url(form: &HashMap<String, String>, method: &str, work: WorkId, answer: &Value) -> String {
    let key = answer["key"].as_i64().or_else(|| number(form, "key").ok().flatten());
    match form.get("back").map(String::as_str) {
        Some("befunde") => return format!("/werke/{work}/befunde"),
        // After deleting, the post page no longer exists: then back to the section.
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

/// `POST /lektorat/act`: see the module header.
async fn lektorat_act(State(state): State<Arc<AppState>>, Form(form): Form<HashMap<String, String>>) -> Response {
    let who = auth::current();
    let method = form.get("method").map(String::as_str).unwrap_or_default();
    if !super::LEKTOR_RPC.iter().any(|(name, _)| *name == method) {
        // The same answer `check_rpc` would give a Lektor; the author has `/act` and `/rpc` for everything else.
        return failure(&super::author_only(method));
    }
    let mut params = match params_of(&form) {
        Ok(params) => params,
        Err(message) => return (StatusCode::UNPROCESSABLE_ENTITY, form_error(&message)).into_response(),
    };
    // The work is fixed afterwards (for the redirect): the named one, otherwise the smallest released one – never
    // the default work of the instance. Whether it is released is checked by `check_rpc` in `rpc::call`.
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

/// `POST /werke/{id}/mitwirkende/act`: see the module header.
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
    // The rights are checked by the layer (route not in LEKTOR_ROUTES) and `rpc::call` (member.* not in LEKTOR_RPC);
    // here once more first, so that nothing about works and accounts is revealed before the check.
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
        // Withdrawing also works for an account that is not a Lektor (any more) – only an unknown one is an error.
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
