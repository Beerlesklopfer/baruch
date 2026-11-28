// SPDX-License-Identifier: AGPL-3.0-or-later
//! Identifier and name of this instance (Round 26): RPC `instance.get {}` → `{uuid, name}` and
//! `instance.set_name {name}` → `{uuid, name}` (trimmed, at most 80 characters, no control characters, empty =
//! reset; if `name` is missing or `null`, that counts as empty). Invalid names yield `INVALID_PARAMS`
//! with a German message. The form is under Einstellungen → Daten (`POST /einstellungen/daten/instanz`,
//! [`crate::settings`]). There is no MCP tool for it: the name is a setting of the human.

use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Methods of this module.
pub const METHODS: &[&str] = &["instance.get", "instance.set_name"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC of the instance (without a work).
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "instance.get" => {
            let instance = state.store.instance().await?;
            Ok(json!({ "uuid": instance.uuid, "name": instance.name }))
        }
        "instance.set_name" => {
            let name = match params.get("name") {
                Some(Value::String(name)) => name.clone(),
                None | Some(Value::Null) => String::new(),
                Some(_) => {
                    return Err(RpcError::new(INVALID_PARAMS, crate::i18n::tc("system_msg.rpc.instance_name", &[])));
                }
            };
            // Check (trim, length, control characters) in the store; invalid → INVALID_PARAMS, nothing stored.
            let instance = state.store.set_instance_name(&name).await?;
            Ok(json!({ "uuid": instance.uuid, "name": instance.name }))
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method: {other}"))),
    }
}
