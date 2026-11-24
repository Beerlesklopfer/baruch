// SPDX-License-Identifier: AGPL-3.0-or-later
//! Warteschlange je MCP-Client (Runde 15, Befund aus dem Gutachten zu Runde 10): Die Modell-Warteschlange ist
//! gemeinsam (eine laufende Anfrage je Endpunkt, wenige wartende); ein einzelner Token-Inhaber könnte sie mit
//! `chat` füllen. Darum darf jeder Client höchstens [`MAX_PER_CLIENT`] Modellaufträge (`chat`, `ask_persona`)
//! zugleich laufen bzw. warten lassen; der nächste bekommt sofort `isError` mit [`busy_message`], ohne dass eine
//! Anfrage an das Modell geht. Andere Clients sind davon nicht betroffen.
//!
//! Gezählt wird je Server (Adresse seines `AppState`; ein Prozess kann mehrere tragen, etwa die Tests) und
//! Client-Handle (`@mcp:<name>`, bei Token der Tokenname); ein [`Slot`] gibt seinen Platz frei, sobald er fällt –
//! auch wenn der Auftrag abbricht (Fallenlassen des Futures).
//!
//! Nachbesserung Runde 23: Ein Client **ohne** Token wählt seinen Namen bei jedem `initialize` frei und bekäme mit
//! jedem neuen Namen zwei frische Plätze. Darum teilen sich alle Clients ohne Token zusätzlich
//! [`MAX_WITHOUT_TOKEN`] Plätze (Schlüssel [`WITHOUT_TOKEN`], kein gültiger Handle); die Grenze je Name bleibt.
//! Ein Token ist davon nicht betroffen – sein Name steht fest.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Höchstens so viele laufende oder wartende Modellaufträge je MCP-Client.
pub const MAX_PER_CLIENT: usize = 2;

/// Höchstens so viele laufende oder wartende Modellaufträge aller Clients **ohne** Token zusammen. Die gemeinsame
/// Modell-Warteschlange fasst eine laufende und acht wartende Anfragen (`baruch_llm::MAX_WAITING`); so bleibt auch
/// bei wechselnden Namen Platz für den Menschen und für Clients mit Token.
pub const MAX_WITHOUT_TOKEN: usize = 4;

/// Zählschlüssel der Clients ohne Token (kein Handle: Handles beginnen mit `@mcp:`).
pub const WITHOUT_TOKEN: &str = "ohne Token";

/// Schlüssel: Server (Adresse seines Zustands) und Client.
type QuotaKey = (usize, String);

fn counts() -> &'static Mutex<HashMap<QuotaKey, usize>> {
    static COUNTS: OnceLock<Mutex<HashMap<QuotaKey, usize>>> = OnceLock::new();
    COUNTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(state: &crate::AppState, client: &str) -> QuotaKey {
    (std::ptr::from_ref(state) as usize, client.to_owned())
}

/// Ehrliche Meldung, wenn ein Client schon [`MAX_PER_CLIENT`] Aufträge laufen hat.
pub fn busy_message(client: &str) -> String {
    format!(
        "{client} hat schon {MAX_PER_CLIENT} Modellaufträge (chat, ask_persona) laufen oder wartend; dieser wurde \
         nicht angenommen und nichts gespeichert. Bitte warten, bis einer fertig ist, und dann erneut senden."
    )
}

/// Ehrliche Meldung, wenn die Clients ohne Token zusammen schon [`MAX_WITHOUT_TOKEN`] Aufträge laufen haben.
pub fn group_busy_message() -> String {
    format!(
        "Die Clients ohne Token haben zusammen schon {MAX_WITHOUT_TOKEN} Modellaufträge (chat, ask_persona) laufen \
         oder wartend; dieser wurde nicht angenommen und nichts gespeichert. Bitte warten, bis einer fertig ist, und \
         dann erneut senden."
    )
}

/// Platz eines laufenden Auftrags; gibt ihn beim Fallen frei (ohne Token auch den gemeinsamen).
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

/// Einen Platz für `client` an diesem Server nehmen; voll → `Err` mit [`busy_message`] (Grenze je Client) bzw.
/// [`group_busy_message`] (gemeinsame Grenze der Clients ohne Token). Beide Zähler ändern sich unter einer Sperre:
/// ein abgelehnter Auftrag belegt nichts.
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

/// Laufende bzw. wartende Aufträge eines Clients an diesem Server (für Tests und Meldungen).
pub fn in_flight(state: &crate::AppState, client: &str) -> usize {
    counts().lock().expect("Zähler der MCP-Warteschlange").get(&key(state, client)).copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_busy_message_names_the_client_the_limit_and_that_nothing_was_saved() {
        let text = busy_message("@mcp:probe");
        assert!(text.starts_with("@mcp:probe hat schon 2 "), "Client und Grenze genannt: {text}");
        assert!(text.contains("nichts gespeichert"), "die Meldung sagt ehrlich, was geschah: {text}");
        let group = group_busy_message();
        assert!(group.contains("ohne Token") && group.contains(" 4 "), "Gruppe und ihre Grenze genannt: {group}");
        assert!(group.contains("nichts gespeichert"), "auch die Gruppenmeldung sagt, was geschah: {group}");
    }
}
