// SPDX-License-Identifier: AGPL-3.0-or-later
//! Work language over RPC (Round 38, interface R38-S1): `work.language` and `work.language_set`.
//!
//! The language itself lives in the store ([`baruch_store::Store::work_language`], migration 0055). Setting it is
//! reserved for the human with full rights: not through `/act`, not through MCP (pattern `work.timing_set`).

use baruch_store::{WorkId, WorkLanguage};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::RpcError;

/// Message for a value that is no work language (`-32602`).
pub const INVALID_LANGUAGE: &str = "language: allowed values are \"de\" and \"en\"";

/// Does this module serve the RPC method? `work.language`, `work.language_set`.
pub fn handles(method: &str) -> bool {
    matches!(method, "work.language" | "work.language_set")
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(crate::rpc::INVALID_PARAMS, message)
}

/// Language of a work, read by prompts and recognitions. A work that cannot be read falls back to German, the
/// language of every work from before Round 38, and says so on stderr ([`fallback`]; finding of Round 38).
pub async fn of(state: &AppState, work: WorkId) -> WorkLanguage {
    let (language, message) = fallback(work, state.store.work_language(work).await);
    if let Some(message) = message {
        eprintln!("{message}");
    }
    language
}

/// The language read from the store, or German with a message for stderr if it could not be read (Round 38c,
/// finding 1: an English work answered in German must not happen silently).
pub(crate) fn fallback(
    work: WorkId,
    read: Result<WorkLanguage, baruch_store::StoreError>,
) -> (WorkLanguage, Option<String>) {
    match read {
        Ok(language) => (language, None),
        Err(e) => {
            (WorkLanguage::De, Some(format!("work language of work {work} unreadable, answering in German: {e}")))
        }
    }
}

/// Sets the work language (only the human: RPC and form, not MCP, not `/act`). The value must be exactly
/// `"de"` or `"en"`.
pub async fn set(state: &AppState, work: WorkId, value: Option<&Value>) -> Result<Value, RpcError> {
    let language =
        value.and_then(Value::as_str).and_then(WorkLanguage::parse).ok_or_else(|| invalid(INVALID_LANGUAGE))?;
    let changed = state.store.set_work_language(work, language).await?;
    Ok(json!({ "work": work, "language": language, "changed": changed }))
}

/// Serves `work.language {work?}` and `work.language_set {work, language}`. `work` is already resolved (missing →
/// default work); `work.language_set` demands it explicitly.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "work.language" => Ok(json!({ "work": work, "language": state.store.work_language(work).await? })),
        "work.language_set" => {
            if matches!(params.get("work"), None | Some(Value::Null)) {
                return Err(invalid("work is missing: the language is set per work (work: work id)."));
            }
            set(state, work, params.get("language")).await
        }
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkLanguage, fallback, handles};

    #[test]
    fn a_readable_language_is_kept_without_a_message() {
        for language in WorkLanguage::ALL {
            assert_eq!(
                fallback(7, Ok(language)),
                (language, None),
                "a language read from the store is used as it is and nothing is logged"
            );
        }
    }

    #[test]
    fn an_unreadable_language_falls_back_to_german_and_says_so() {
        let (language, message) = fallback(7, Err(baruch_store::StoreError::UnknownWork(7)));
        assert_eq!(language, WorkLanguage::De, "German is the language of every work from before Round 38");
        let message = message.expect("the fallback must not be silent (finding 1 of Round 38)");
        assert!(
            message.starts_with("work language of work 7 unreadable, answering in German: "),
            "the message names the work and the consequence: {message}"
        );
        assert!(
            message.len() > "work language of work 7 unreadable, answering in German: ".len(),
            "the message carries the store error after the colon: {message}"
        );
    }

    #[test]
    fn only_the_two_language_methods_are_handled() {
        for method in ["work.language", "work.language_set"] {
            assert!(handles(method), "the dispatcher in rpc.rs sends {method} to this module");
        }
        for method in ["work.languages", "work.timing", "work.get", "language", ""] {
            assert!(!handles(method), "{method:?} belongs to another module and must not be swallowed here");
        }
    }
}
