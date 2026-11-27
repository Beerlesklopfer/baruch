// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC of the roles (Round 35, contract §2.3): releases (`member.*`, full rights only) and the write paths of a
//! Lektor (`revision.propose`, `finding.add`, [`crate::roles::lektor`]).
//!
//! - `member.list {work}` → `{work, members: [{account, name, display, handle, role, granted, active}],
//!   candidates: [{account, name, display, handle}]}`: releases of the work (also those of locked accounts, `active`
//!   says so) and the active Lektor accounts without a release.
//! - `member.grant {work, account, role?: "lektor"}` → `{work, account, role, granted}`: idempotent; only accounts of
//!   the kind `lektor` (-32000), unknown account or work -32602.
//! - `member.revoke {work, account}` → `{work, account, revoked}` (`revoked: false` if there was no release).
//!
//! `member.*` requires `work` (no default work: a release must never hit the wrong work by accident) and
//! full rights – for Lektors [`crate::roles::check_rpc`] already locks them, here the check stands once more
//! so that a direct call of [`call`] releases nothing either. Releases are instance data: no commit in the mirror.

use baruch_store::WorkId;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::Who;
use crate::rpc::{INVALID_PARAMS, REFUSED, RpcError};

pub const METHODS: &[&str] = &["member.list", "member.grant", "member.revoke", "revision.propose", "finding.add"];

/// `crate::rpc` asks before the domain modules whether the method belongs here.
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Integer from `params[name]` (number or digits as text from forms); missing → -32602.
fn required(params: &Value, name: &str, what: &str) -> Result<i64, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        None | Some(Value::Null) => Err(invalid(format!("{name} fehlt ({what})"))),
        Some(Value::Number(n)) => n.as_i64().ok_or_else(wrong),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// An account as far as the releases show it (never the hash).
#[derive(Debug, Clone, sqlx::FromRow)]
struct AccountRow {
    id: i64,
    name: String,
    slug: String,
    display: String,
    kind: String,
    active: bool,
}

impl AccountRow {
    fn handle(&self) -> String {
        format!("@lektorat:{}", self.slug)
    }
}

/// All accounts, by id. The table is read directly: the releases need only name, slug, kind and "active".
async fn accounts(state: &AppState) -> Result<Vec<AccountRow>, RpcError> {
    sqlx::query_as::<_, AccountRow>("SELECT id, name, slug, display, kind, active FROM account ORDER BY id")
        .fetch_all(state.store.pool())
        .await
        .map_err(|e| baruch_store::StoreError::from(e).into())
}

/// Why a work cannot be released for an account (for the codes of the form, contract §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantRefusal {
    UnknownAccount,
    NotLektor,
}

impl GrantRefusal {
    /// Code for `?fehler=` (`konten_ui::MESSAGES`).
    pub fn code(self) -> &'static str {
        match self {
            GrantRefusal::UnknownAccount => "unbekanntes_konto",
            GrantRefusal::NotLektor => "kein_lektor",
        }
    }
}

/// Can a work be released for the account? Unknown → [`GrantRefusal::UnknownAccount`], not a Lektor account →
/// [`GrantRefusal::NotLektor`].
pub async fn grantable(state: &AppState, account: i64) -> Result<Result<(), GrantRefusal>, RpcError> {
    let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM account WHERE id = ?")
        .bind(account)
        .fetch_optional(state.store.pool())
        .await
        .map_err(|e| RpcError::from(baruch_store::StoreError::from(e)))?;
    Ok(match kind.as_deref() {
        None => Err(GrantRefusal::UnknownAccount),
        Some("lektor") => Ok(()),
        Some(_) => Err(GrantRefusal::NotLektor),
    })
}

async fn list(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let members = state.store.member_list(work).await?;
    let accounts = accounts(state).await?;
    let shown: Vec<Value> = members
        .iter()
        .filter_map(|member| {
            let account = accounts.iter().find(|a| a.id == member.account)?;
            Some(json!({
                "account": account.id, "name": account.name, "display": account.display,
                "handle": account.handle(), "role": member.role, "granted": member.granted, "active": account.active,
            }))
        })
        .collect();
    let candidates: Vec<Value> = accounts
        .iter()
        .filter(|a| a.kind == "lektor" && a.active && !members.iter().any(|m| m.account == a.id))
        .map(|a| json!({ "account": a.id, "name": a.name, "display": a.display, "handle": a.handle() }))
        .collect();
    Ok(json!({ "work": work, "members": shown, "candidates": candidates }))
}

async fn grant(state: &AppState, work: WorkId, params: &Value) -> Result<Value, RpcError> {
    let account = required(params, "account", "Konto")?;
    let role = match params.get("role") {
        None | Some(Value::Null) => "lektor",
        Some(Value::String(role)) if role == "lektor" => "lektor",
        Some(_) => return Err(invalid("role: nur „lektor“ lässt sich freigeben")),
    };
    // First the work (unknown → -32602 as everywhere), then the account.
    state.store.work_slug(work).await?;
    match grantable(state, account).await? {
        Ok(()) => {}
        Err(GrantRefusal::UnknownAccount) => return Err(invalid(format!("Unbekanntes Konto {account}."))),
        Err(GrantRefusal::NotLektor) => {
            return Err(RpcError::new(
                REFUSED,
                "Nur Lektor-Konten bekommen eine Freigabe; der Autor ist Autor in jedem Werk.",
            ));
        }
    }
    state.store.member_grant(work, account, role).await?;
    let granted = state.store.member_list(work).await?.into_iter().find(|m| m.account == account).map(|m| m.granted);
    Ok(json!({ "work": work, "account": account, "role": role, "granted": granted }))
}

pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let who = crate::auth::current();
    call_as(state, &who, method, params).await
}

/// Like [`call`], with the actor named explicitly.
pub async fn call_as(state: &AppState, who: &Who, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err(invalid("Parameter als Objekt erwartet"));
    }
    match method {
        "revision.propose" | "finding.add" => {
            // For Lektors `check_rpc` has already inserted and checked the work; all others are rejected right
            // away (Lektors only), the work then only serves the uniform error message.
            let work = crate::rpc::work_param(state, &params).await?;
            if method == "revision.propose" {
                super::lektor::propose(state, who, work, params).await
            } else {
                super::lektor::add_finding(state, who, work, params).await
            }
        }
        "member.list" | "member.grant" | "member.revoke" => {
            if !who.full_rights() {
                return Err(super::author_only(method));
            }
            let work = required(&params, "work", "Werk")?;
            match method {
                "member.list" => list(state, work).await,
                "member.grant" => grant(state, work, &params).await,
                _ => {
                    let account = required(&params, "account", "Konto")?;
                    state.store.work_slug(work).await?;
                    let revoked = state.store.member_revoke(work, account).await?;
                    Ok(json!({ "work": work, "account": account, "revoked": revoked }))
                }
            }
        }
        other => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_five_methods_belong_to_this_module() {
        for method in METHODS {
            assert!(handles(method), "{method} gehört den Rollen");
        }
        assert!(!handles("finding.list"), "Befunde lesen bleibt bei findings::rpc");
        assert!(!handles("revision.save"), "Speichern bleibt dem Autor und bei rpc.rs");
    }

    #[test]
    fn numbers_come_as_numbers_or_digits_and_refusals_carry_the_form_codes() {
        assert_eq!(required(&json!({ "account": 4 }), "account", "Konto").unwrap(), 4, "Zahl aus JSON");
        assert_eq!(required(&json!({ "account": " 4 " }), "account", "Konto").unwrap(), 4, "Ziffern aus Formularen");
        for bad in [json!({}), json!({ "account": "vier" }), json!({ "account": true }), json!({ "account": 1.5 })] {
            let error = required(&bad, "account", "Konto").unwrap_err();
            assert_eq!(error.code, INVALID_PARAMS, "{bad}: fehlend oder keine ganze Zahl ist ein Parameterfehler");
        }
        assert_eq!(GrantRefusal::UnknownAccount.code(), "unbekanntes_konto", "Code aus Vertrag §3.4");
        assert_eq!(GrantRefusal::NotLektor.code(), "kein_lektor", "Code aus Vertrag §3.4");
    }
}
