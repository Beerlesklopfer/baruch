// SPDX-License-Identifier: AGPL-3.0-or-later
//! Kennung und Name dieser Instanz (Runde 26): RPC `instance.get {}` → `{uuid, name}` und
//! `instance.set_name {name}` → `{uuid, name}` (getrimmt, höchstens 80 Zeichen, keine Steuerzeichen, leer =
//! zurücksetzen; fehlt `name` oder ist er `null`, gilt das wie leer). Ungültige Namen ergeben `INVALID_PARAMS`
//! mit deutscher Meldung. Das Formular liegt unter Einstellungen → Daten (`POST /einstellungen/daten/instanz`,
//! [`crate::settings`]). Über MCP gibt es dafür kein Werkzeug: Der Name ist eine Einstellung des Menschen.

use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Methoden dieses Moduls.
pub const METHODS: &[&str] = &["instance.get", "instance.set_name"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// RPC der Instanz (ohne Werk).
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
                Some(_) => return Err(RpcError::new(INVALID_PARAMS, "name: Text erwartet")),
            };
            // Prüfen (trim, Länge, Steuerzeichen) im Store; ungültig → INVALID_PARAMS, nichts gespeichert.
            let instance = state.store.set_instance_name(&name).await?;
            Ok(json!({ "uuid": instance.uuid, "name": instance.name }))
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method: {other}"))),
    }
}
