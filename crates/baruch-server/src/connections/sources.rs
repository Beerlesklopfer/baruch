// SPDX-License-Identifier: AGPL-3.0-or-later
//! Preferred sources (bookmarks for `@belege`): RPC `sources.*` and the validation of the inputs.
//!
//! - `sources.list {}` → `{sources: [{id, title, url, note, position, created, updated}]}`
//! - `sources.create {title, url, note?}` → the new source (at the end of the list)
//! - `sources.update {id, title?, url?, note?}` → the changed source
//! - `sources.move {id, position}` → `{sources}` (position 1..n, outside moves to the edge)
//! - `sources.delete {id, confirm: true}` → `{deleted}`
//!
//! Title 1–200 characters in one line, address http/https with host, without user info and
//! whitespace, at most 2000 characters; note at most 4000 characters. Invalid → -32602, nothing changed.
//! The web search of `@belege` (Round 13, [`crate::websearch`]) queries these sources first (usage text: catalog key
//! `connections_msg.sources.usage`, page „Quellen“, `data-usage`).

use baruch_store::{NewSource, PreferredSource, SourceEdit, StoreError};
use serde_json::{Value, json};

use super::Fault;
use crate::AppState;

const MAX_NOTE: usize = 4000;

fn invalid(code: &'static str, message: &str) -> Fault {
    Fault::Invalid(code, message.to_owned())
}

pub fn check_title(title: &str) -> Result<String, Fault> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 200 || title.chars().any(char::is_control) {
        return Err(invalid("titel", &crate::i18n::tc("connections_msg.sources.title", &[])));
    }
    Ok(title.to_owned())
}

pub fn check_address(url: &str) -> Result<String, Fault> {
    let url = url.trim();
    let bad = || invalid("adresse", &crate::i18n::tc("connections_msg.sources.url", &[]));
    if url.is_empty() || url.chars().count() > 2000 || url.chars().any(char::is_whitespace) {
        return Err(bad());
    }
    let parsed = reqwest::Url::parse(url).map_err(|_| bad())?;
    let fine = matches!(parsed.scheme(), "http" | "https")
        && parsed.host_str().is_some_and(|h| !h.is_empty())
        && parsed.username().is_empty()
        && parsed.password().is_none();
    if fine { Ok(url.to_owned()) } else { Err(bad()) }
}

pub fn check_note(note: &str) -> Result<String, Fault> {
    if note.chars().count() > MAX_NOTE {
        return Err(invalid("notiz", &crate::i18n::tc("connections_msg.sources.note", &[])));
    }
    Ok(note.trim().to_owned())
}

/// Error of the store for sources: `Refused` here means “does not exist (any more)” (inputs are checked
/// beforehand).
fn store_fault(e: StoreError) -> Fault {
    match e {
        StoreError::Refused(message) => {
            Fault::Invalid("quelle", crate::store_text::translate(crate::i18n::current(), message))
        }
        other => other.into(),
    }
}

pub(crate) async fn create(state: &AppState, title: &str, url: &str, note: &str) -> Result<PreferredSource, Fault> {
    let new = NewSource { title: check_title(title)?, url: check_address(url)?, note: check_note(note)? };
    state.store.source_create(&new).await.map_err(store_fault)
}

pub(crate) async fn update(
    state: &AppState,
    id: i64,
    title: Option<&str>,
    url: Option<&str>,
    note: Option<&str>,
) -> Result<PreferredSource, Fault> {
    let edit = SourceEdit {
        title: title.map(check_title).transpose()?,
        url: url.map(check_address).transpose()?,
        note: note.map(check_note).transpose()?,
    };
    state.store.source_update(id, &edit).await.map_err(store_fault)
}

pub(crate) async fn move_to(state: &AppState, id: i64, position: i64) -> Result<Vec<PreferredSource>, Fault> {
    state.store.source_move(id, position).await.map_err(store_fault)
}

pub(crate) async fn delete(state: &AppState, id: i64) -> Result<PreferredSource, Fault> {
    state.store.source_delete(id).await.map_err(store_fault)
}

fn id_param(params: &Value) -> Result<i64, Fault> {
    params.get("id").and_then(Value::as_i64).ok_or_else(|| invalid("quelle", "id: Zahl erwartet."))
}

fn text_param<'a>(params: &'a Value, name: &str, code: &'static str) -> Result<Option<&'a str>, Fault> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(Fault::Invalid(code, format!("{name}: Text erwartet."))),
    }
}

/// RPC `sources.*`; `None` if the method does not belong here.
pub(crate) async fn call(state: &AppState, method: &str, params: &Value) -> Option<Result<Value, Fault>> {
    let answer = match method {
        "sources.list" => {
            state.store.sources_list().await.map(|sources| json!({ "sources": sources })).map_err(Fault::from)
        }
        "sources.create" => {
            let fields = (
                text_param(params, "title", "titel"),
                text_param(params, "url", "adresse"),
                text_param(params, "note", "notiz"),
            );
            match fields {
                (Ok(title), Ok(url), Ok(note)) => {
                    create(state, title.unwrap_or(""), url.unwrap_or(""), note.unwrap_or("")).await.map(|s| json!(s))
                }
                (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
            }
        }
        "sources.update" => {
            let fields = (
                id_param(params),
                text_param(params, "title", "titel"),
                text_param(params, "url", "adresse"),
                text_param(params, "note", "notiz"),
            );
            match fields {
                (Ok(id), Ok(title), Ok(url), Ok(note)) => update(state, id, title, url, note).await.map(|s| json!(s)),
                (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => Err(e),
            }
        }
        "sources.move" => match (id_param(params), params.get("position").and_then(Value::as_i64)) {
            (Ok(id), Some(position)) => move_to(state, id, position).await.map(|sources| json!({ "sources": sources })),
            (Err(e), _) => Err(e),
            (_, None) => Err(invalid("position", "position: Zahl ab 1 erwartet.")),
        },
        "sources.delete" => match id_param(params) {
            Err(e) => Err(e),
            Ok(_) if params.get("confirm") != Some(&Value::Bool(true)) => {
                Err(invalid("bestaetigen", &crate::i18n::tc("connections_msg.sources.confirm", &[])))
            }
            Ok(id) => delete(state, id).await.map(|gone| json!({ "deleted": gone })),
        },
        _ => return None,
    };
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_of_sources_are_http_with_a_host() {
        assert!(check_address("https://bibel.example/lutherbibel").is_ok(), "gewöhnliche Adresse");
        for bad in ["", "https://", "mailto:a@b.example", "https://ich:pw@x.example/", "https://x.example/a b"] {
            assert!(check_address(bad).is_err(), "{bad:?} ist keine Quelle");
        }
        assert!(check_title(&"x".repeat(200)).is_ok() && check_title(&"x".repeat(201)).is_err(), "200 Zeichen");
    }
}
