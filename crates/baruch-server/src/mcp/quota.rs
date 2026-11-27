// SPDX-License-Identifier: AGPL-3.0-or-later
//! Queue per MCP client (Round 15, finding from the review of Round 10): the model queue is
//! shared (one running request per endpoint, a few waiting); a single token holder could fill it with
//! `chat`. Therefore every client may have at most [`MAX_PER_CLIENT`] model jobs (`chat`, `ask_persona`)
//! running or waiting at the same time; the next one immediately gets `isError` with [`busy_message`], without a
//! request going to the model. Other clients are not affected by this.
//!
//! Counting is per server (address of its `AppState`; one process can carry several, for instance the tests) and
//! client handle (`@mcp:<name>`, with a token the token name); a [`Slot`] releases its place as soon as it is
//! dropped – even if the job aborts (dropping the future).
//!
//! Follow-up fix Round 23: a client **without** a token chooses its name freely at every `initialize` and would get
//! two fresh places with every new name. Therefore all clients without a token additionally share
//! [`MAX_WITHOUT_TOKEN`] places (key [`WITHOUT_TOKEN`], not a valid handle); the limit per name stays.
//! A token is not affected by this – its name is fixed.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// At most this many running or waiting model jobs per MCP client.
pub const MAX_PER_CLIENT: usize = 2;

/// At most this many running or waiting model jobs of all clients **without** a token together. The shared
/// model queue holds one running and eight waiting requests (`baruch_llm::MAX_WAITING`); this leaves room
/// for the human and for clients with a token even with changing names.
pub const MAX_WITHOUT_TOKEN: usize = 4;

/// Counting key of the clients without a token (not a handle: handles begin with `@mcp:`).
pub const WITHOUT_TOKEN: &str = "ohne Token";

/// Key: server (address of its state) and client.
type QuotaKey = (usize, String);

fn counts() -> &'static Mutex<HashMap<QuotaKey, usize>> {
    static COUNTS: OnceLock<Mutex<HashMap<QuotaKey, usize>>> = OnceLock::new();
    COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(state: &crate::AppState, client: &str) -> QuotaKey {
    (std::ptr::from_ref(state) as usize, client.to_owned())
}

/// Honest message when a client already has [`MAX_PER_CLIENT`] jobs running.
pub fn busy_message(client: &str) -> String {
    format!(
        "{client} already has {MAX_PER_CLIENT} model jobs (chat, ask_persona) running or waiting; this one was not \
         accepted and nothing was stored. Please wait until one is finished and then send again."
    )
}

/// Honest message when the clients without a token together already have [`MAX_WITHOUT_TOKEN`] jobs running.
pub fn group_busy_message() -> String {
    format!(
        "The clients without a token together already have {MAX_WITHOUT_TOKEN} model jobs (chat, ask_persona) running \
         or waiting; this one was not accepted and nothing was stored. Please wait until one is finished and then \
         send again."
    )
}

/// Place of a running job; releases it when dropped (without a token also the shared one).
#[derive(Debug)]
pub struct Slot {
    keys: Vec<QuotaKey>,
}

impl Drop for Slot {
    fn drop(&mut self) {
        let mut map = counts().lock().expect("Zähler der MCP-Warteschlange");
        for key in &self.keys {
            if let Some(n) = map.get_mut(key) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    map.remove(key);
                }
            }
        }
    }
}

/// Take a place for `client` on this server; full → `Err` with [`busy_message`] (limit per client) or
/// [`group_busy_message`] (shared limit of the clients without a token). Both counters change under one lock:
/// a refused job occupies nothing.
pub fn acquire(state: &crate::AppState, client: &crate::mcp::session::Client) -> Result<Slot, String> {
    let handle = client.handle();
    let own = key(state, &handle);
    let mut map = counts().lock().expect("Zähler der MCP-Warteschlange");
    if map.get(&own).copied().unwrap_or(0) >= MAX_PER_CLIENT {
        return Err(busy_message(&handle));
    }
    let mut keys = vec![own];
    if !client.token {
        let group = key(state, WITHOUT_TOKEN);
        if map.get(&group).copied().unwrap_or(0) >= MAX_WITHOUT_TOKEN {
            return Err(group_busy_message());
        }
        keys.push(group);
    }
    for key in &keys {
        *map.entry(key.clone()).or_insert(0) += 1;
    }
    Ok(Slot { keys })
}

/// Running or waiting jobs of a client on this server (for tests and messages).
pub fn in_flight(state: &crate::AppState, client: &str) -> usize {
    counts().lock().expect("Zähler der MCP-Warteschlange").get(&key(state, client)).copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_busy_message_names_the_client_the_limit_and_that_nothing_was_saved() {
        let text = busy_message("@mcp:probe");
        assert!(text.starts_with("@mcp:probe already has 2 "), "Client und Grenze genannt: {text}");
        assert!(text.contains("nothing was stored"), "die Meldung sagt ehrlich, was geschah: {text}");
        let group = group_busy_message();
        assert!(group.contains("without a token") && group.contains(" 4 "), "Gruppe und ihre Grenze genannt: {group}");
        assert!(group.contains("nothing was stored"), "auch die Gruppenmeldung sagt, was geschah: {group}");
    }
}
