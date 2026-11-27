// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC `account.*` (Round 35, contract §1.6). Login, logout and setup exist only as a form (cookie);
//! passwords never stand in an answer.
//!
//! What a role may call is decided by `roles::check_rpc` before the dispatch. The methods only the author
//! may use additionally check [`Who::full_rights`] here – two independent places would have to fail before a Lektor
//! manages accounts. Answers carry only what [`baruch_store::Account`] holds (no hash) and the handle.

use baruch_store::Account;
use serde::Deserialize;
use serde_json::{Value, json};

use super::routes::{self, Problem};
use super::{Who, session};
use crate::AppState;
use crate::rpc::{FORBIDDEN, INTERNAL, INVALID_PARAMS, REFUSED, RpcError};

pub const METHODS: &[&str] = &[
    "account.me",
    "account.list",
    "account.create",
    "account.set_active",
    "account.set_password",
    "account.set_local_open",
    "account.logout_all",
];

/// `crate::rpc` asks before every other module whether the method belongs here.
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// German sentence for the message codes of the building blocks (the UI has its own table).
fn sentence(code: &str) -> &'static str {
    match code {
        "passwort_falsch" => "Das bisherige Passwort stimmt nicht.",
        "passwort_kurz" => "Das Passwort braucht mindestens 10 Zeichen.",
        "passwort_lang" => "Das Passwort darf höchstens 200 Zeichen haben.",
        "passwort_ungleich" => "Die beiden Passwörter sind verschieden.",
        "name_vergeben" => "Diesen Namen gibt es schon.",
        "name_ungueltig" => {
            "Ungültiger Name: 1 bis 40 Zeichen mit mindestens einem Buchstaben a–z oder einer Ziffer, nicht \
             „unbekannt“; Anzeigename höchstens 80 Zeichen."
        }
        "letzter_autor" => "Das letzte aktive Autor-Konto lässt sich nicht sperren.",
        "unbekanntes_konto" => "Dieses Konto gibt es nicht.",
        routes::NEEDS_AUTHOR => {
            "Es gibt noch kein Konto. Das erste Konto (der Autor) wird am eigenen Rechner unter /einrichten angelegt."
        }
        _ => "Das ging nicht.",
    }
}

fn refused(problem: Problem) -> RpcError {
    match problem {
        Problem::Code(code) => RpcError::new(REFUSED, sentence(code)),
        Problem::Internal(e) => RpcError::new(INTERNAL, e),
    }
}

fn internal(e: impl std::fmt::Display) -> RpcError {
    RpcError::new(INTERNAL, e.to_string())
}

/// Read parameters. The error message deliberately does not name serde's text: it could quote a value, and
/// among the values are passwords.
fn parse<T: for<'de> Deserialize<'de>>(method: &str, params: Value, expected: &str) -> Result<T, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|_| RpcError::new(INVALID_PARAMS, format!("{method}: erwartet {expected}")))
}

fn author_only(who: &Who, method: &str) -> Result<(), RpcError> {
    if who.full_rights() { Ok(()) } else { Err(RpcError::new(FORBIDDEN, format!("Das darf nur der Autor: {method}"))) }
}

/// An account as it stands in answers: the fields of [`Account`] and the handle. No hash, no password.
fn account_json(account: &Account) -> Value {
    json!({
        "id": account.id,
        "name": account.name,
        "display": account.display,
        "kind": account.kind,
        "handle": format!("@lektorat:{}", account.slug),
        "active": account.active,
        "created": account.created,
        "last_login": account.last_login,
    })
}

#[derive(Deserialize)]
struct CreateParams {
    name: String,
    #[serde(default)]
    display: String,
    password: String,
}

#[derive(Deserialize)]
struct ActiveParams {
    id: i64,
    active: bool,
}

#[derive(Deserialize)]
struct OpenParams {
    open: bool,
}

#[derive(Deserialize)]
struct PasswordParams {
    current: Option<String>,
    password: String,
    id: Option<i64>,
}

pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let who = super::current();
    match method {
        "account.me" => {
            let local_open = state.store.auth_local_open().await.map_err(internal)?;
            let mut me = json!({
                "accounts_exist": super::accounts_exist(state).await,
                "signed_in": who.identity().is_some(),
                "local_open": local_open,
            });
            if let Some(identity) = who.identity() {
                me["id"] = json!(identity.id);
                me["name"] = json!(identity.name);
                me["display"] = json!(identity.display);
                me["kind"] = json!(identity.kind);
                me["handle"] = json!(identity.handle);
            }
            Ok(me)
        }
        "account.list" => {
            author_only(&who, method)?;
            let accounts = state.store.account_list().await.map_err(internal)?;
            Ok(json!({ "accounts": accounts.iter().map(account_json).collect::<Vec<_>>() }))
        }
        "account.create" => {
            author_only(&who, method)?;
            let p: CreateParams = parse(method, params, "{name, display?, password}")?;
            // Further accounts are always Lektors (contract 0.4); the first is created only through /einrichten.
            let account =
                routes::create_account(state, &p.name, &p.display, p.password, None, false).await.map_err(refused)?;
            Ok(account_json(&account))
        }
        "account.set_active" => {
            author_only(&who, method)?;
            let p: ActiveParams = parse(method, params, "{id, active}")?;
            routes::set_active(state, p.id, p.active).await.map_err(refused)?;
            let account = state.store.account_get(p.id).await.map_err(internal)?;
            account.as_ref().map(account_json).ok_or_else(|| RpcError::new(REFUSED, sentence("unbekanntes_konto")))
        }
        "account.set_local_open" => {
            author_only(&who, method)?;
            let p: OpenParams = parse(method, params, "{open}")?;
            state.store.set_auth_local_open(p.open).await.map_err(internal)?;
            Ok(json!({ "local_open": p.open }))
        }
        "account.set_password" => {
            let p: PasswordParams = parse(method, params, "{current?, password, id?}")?;
            let own = who.identity().filter(|identity| p.id.is_none_or(|id| id == identity.id));
            match (own, p.id) {
                // Own password: only with the current one; the session of this request stays, all others end.
                (Some(identity), _) => {
                    let Some(current) = p.current else {
                        return Err(RpcError::new(INVALID_PARAMS, "account.set_password: current fehlt"));
                    };
                    routes::check_current(state, identity, current).await.map_err(refused)?;
                    let keep = session::current_hash();
                    routes::change_password(state, identity.id, p.password, None, keep.as_deref())
                        .await
                        .map_err(refused)?;
                    Ok(json!({ "id": identity.id, "changed": true }))
                }
                // Someone else's password: the author only; all sessions of the account end.
                (None, Some(id)) => {
                    author_only(&who, method)?;
                    routes::change_password(state, id, p.password, None, None).await.map_err(refused)?;
                    Ok(json!({ "id": id, "changed": true }))
                }
                (None, None) => Err(RpcError::new(
                    REFUSED,
                    "Ohne Anmeldung gibt es kein eigenes Konto; mit id das Passwort eines Kontos setzen.",
                )),
            }
        }
        "account.logout_all" => {
            let Some(identity) = who.identity() else {
                return Err(RpcError::new(REFUSED, "Ohne Anmeldung gibt es keine Sitzungen zu beenden."));
            };
            let sessions = state.store.session_delete_all(identity.id, None).await.map_err(internal)?;
            Ok(json!({ "sessions": sessions }))
        }
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unbekannte Methode: {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_account_methods_belong_to_this_module() {
        for method in METHODS {
            assert!(handles(method), "{method} gehört der Anmeldung");
            assert!(method.starts_with("account."), "{method}: alle Methoden der Anmeldung heißen account.*");
        }
        assert!(!handles("account.delete"), "Konten werden nie gelöscht – die Methode gibt es nicht");
        assert!(!handles("work.list"), "fremde Methoden gehören nicht hierher");
    }

    #[test]
    fn a_wrong_parameter_is_reported_without_quoting_any_value() {
        let params = json!({ "name": "Erika", "password": ["streng-geheimes-wort"] });
        let error = parse::<CreateParams>("account.create", params, "{name, display?, password}").err().unwrap();
        assert_eq!(error.code, INVALID_PARAMS, "falsche Parameter sind -32602");
        assert!(!error.message.contains("streng-geheimes-wort"), "die Meldung zitiert keinen Wert: {}", error.message);
        assert!(error.message.contains("{name, display?, password}"), "sie nennt, was erwartet wird");
    }

    #[test]
    fn an_account_in_an_answer_has_a_handle_and_no_secret() {
        let account = Account {
            id: 2,
            name: "Erika".into(),
            slug: "erika".into(),
            display: "Erika Beispiel".into(),
            kind: "lektor".into(),
            active: true,
            created: "2026-11-26T10:00:00.000Z".into(),
            last_login: None,
        };
        let value = account_json(&account);
        assert_eq!(value["handle"], "@lektorat:erika", "die Herkunft, unter der das Konto schreibt");
        let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["active", "created", "display", "handle", "id", "kind", "last_login", "name"],
            "genau diese Felder – kein Passwort, kein Hash, kein Token"
        );
    }
}
