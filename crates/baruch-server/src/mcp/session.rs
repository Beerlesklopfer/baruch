// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP sessions: `initialize` remembers `clientInfo` under a new `Mcp-Session-Id`.
//!
//! Process-wide table with at most [`MAX_SESSIONS`] entries; if it is full, the oldest
//! session drops out. The id is no secret (access over loopback or with a token, Round 10), it
//! only assigns later requests to the client, so that proposals and posts are attributed to it.
//! With a token the client is named like the token, even if the session was created differently ([`crate::mcp`]).
//!
//! Round 23: the handle follows [`handle_name`] (umlauts transliterated); a client without a token whose handle
//! belongs to a token is named `lokal-<name>` ([`local_name`]); for old posts a token also owns its legacy handle
//! ([`Client::owned_handles`]). If two tokens from the time before Round 23 coincided only through the new rule
//! („Büro“ and „buero“), the token with the umlaut keeps its legacy handle ([`keeps_legacy`]).

use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

/// Maximum number of remembered sessions.
pub const MAX_SESSIONS: usize = 256;

/// Name and version from `clientInfo` of `initialize`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Client {
    pub name: String,
    pub version: String,
    /// Signed in with a token (another machine possible): acts only in its own name, never as a persona
    /// ([`super::tools::acting_persona`]).
    pub token: bool,
    /// Only with a token: the token keeps writing under its legacy handle, because its handle under the rule since
    /// Round 23 would coincide with that of another token only through this rule ([`keeps_legacy`]).
    pub keeps_legacy: bool,
}

/// Prefix in the name of a client **without** a token whose handle belongs to a token (Round 23): author
/// `@mcp:lokal-<handle>`. No token may get a handle with this prefix
/// ([`baruch_store::Store::mcp_token_create_unique`]).
pub const LOCAL_PREFIX: &str = "lokal-";

/// Character of a handle after transliteration: `[a-z0-9._-]` stays, everything else becomes '-'.
fn safe(c: char) -> char {
    if c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-') { c } else { '-' }
}

/// Name part of a handle (without `@mcp:`), rule since Round 23: trimmed, NFC, lower case, umlauts transliterated
/// (ä → ae, ö → oe, ü → ue, ß → ss), otherwise after NFKD without accents (é → e, ﬁ → fi), every remaining character
/// outside `[a-z0-9._-]` → '-', at most 32 characters, empty → `unbekannt`. `mcp_token.create` checks the same rule.
pub fn handle_name(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    use unicode_normalization::char::is_combining_mark;
    let mut spelled = String::with_capacity(name.len());
    // NFC first: a „ü“ typed decomposed (u + diaeresis, e.g. from macOS) is the same umlaut as the single character.
    for c in name.trim().nfc().flat_map(char::to_lowercase) {
        match c {
            'ä' => spelled.push_str("ae"),
            'ö' => spelled.push_str("oe"),
            'ü' => spelled.push_str("ue"),
            'ß' => spelled.push_str("ss"),
            c => spelled.push(c),
        }
    }
    // NFKD can expose upper-case letters (e.g. from compatibility characters), so lower-case once more afterwards.
    let out: String =
        spelled.nfkd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_lowercase).map(safe).take(32).collect();
    if out.is_empty() { "unbekannt".to_owned() } else { out }
}

/// Name part of a handle under the rule **up to Round 22** (lower case, everything outside `[a-z0-9._-]` → '-', so
/// umlauts too; not trimmed; at most 32 characters, empty → `unbekannt`). Only used any more to recognise the
/// ownership of a token of its old posts ([`Client::owned_handles`]) and to rule out collisions with
/// such authors when a token is created.
pub fn legacy_handle_name(name: &str) -> String {
    let out: String = name.chars().flat_map(char::to_lowercase).map(safe).take(32).collect();
    if out.is_empty() { "unbekannt".to_owned() } else { out }
}

/// Name of a client **without** a token (Round 23): if its handle belongs to a token (also a revoked one; handle
/// or legacy handle), the name gets the prefix [`LOCAL_PREFIX`] – otherwise it stays. This way a client from the
/// own machine never writes under the author of a token and gains no ownership of its posts. `token_names` are
/// the names of all tokens. The loop only covers tokens from the time before Round 23 whose handle itself begins
/// with `lokal-` (`mcp_token.create` refuses new ones).
pub fn local_name<'a>(name: &str, token_names: impl IntoIterator<Item = &'a str>) -> String {
    let taken: std::collections::HashSet<String> =
        token_names.into_iter().flat_map(|t| [handle_name(t), legacy_handle_name(t)]).collect();
    let mut name = name.to_owned();
    for _ in 0..4 {
        if !taken.contains(&handle_name(&name)) {
            break;
        }
        name = format!("{LOCAL_PREFIX}{}", name.trim());
    }
    name
}

/// Does the token `name` keep its legacy handle ([`legacy_handle_name`])? Yes, if the rule since Round 23 changed
/// its handle (umlauts) **and** the new handle equals the handle or legacy handle of a token that wrote under a
/// **different** author up to Round 22. Example: „Büro“ (up to Round 22 `b-ro`) and „buero“ (`buero`) were separate
/// and would both yield `buero` – so „Büro“ stays with `b-ro`, „buero“ with `buero`; neither writes under the author
/// of the other or owns its posts. Tokens with the same legacy handle („Büro“ and „BÜRO“, also a
/// revoked one of the same name) already shared it before and change together. `token_names` are the names of all
/// tokens (revoked ones too: their posts keep carrying the author); the token itself may be among them. The rule
/// does not depend on the order. It never hits new tokens: `mcp_token.create` refuses such names.
pub fn keeps_legacy<'a>(name: &str, token_names: impl IntoIterator<Item = &'a str>) -> bool {
    let (new, legacy) = (handle_name(name), legacy_handle_name(name));
    new != legacy
        && token_names.into_iter().any(|other| {
            let other_legacy = legacy_handle_name(other);
            other_legacy != legacy && (other_legacy == new || handle_name(other) == new)
        })
}

/// Author (`@mcp:<handle>`) under which the token `name` writes: by [`handle_name`], with [`keeps_legacy`] by
/// [`legacy_handle_name`]. `token_names` as there.
pub fn token_handle<'a>(name: &str, token_names: impl IntoIterator<Item = &'a str>) -> String {
    Client { name: name.to_owned(), token: true, keeps_legacy: keeps_legacy(name, token_names), ..Default::default() }
        .handle()
}

impl Client {
    /// Handle for author attributions: `@mcp:` + [`handle_name`] of the name; a token with [`Client::keeps_legacy`]
    /// keeps `@mcp:` + [`legacy_handle_name`].
    pub fn handle(&self) -> String {
        if self.token && self.keeps_legacy {
            return format!("@mcp:{}", legacy_handle_name(&self.name));
        }
        format!("@mcp:{}", handle_name(&self.name))
    }

    /// Authors whose posts belong to this client (Round 23): its handle; with a token additionally the legacy handle
    /// of the same token ([`legacy_handle_name`]) if the new rule changed it (umlauts). A client without a
    /// token owns only its handle – the legacy handle of a token is denied to it through [`local_name`]. A token
    /// with [`Client::keeps_legacy`] owns only the legacy handle (the new one belongs to another token).
    pub fn owned_handles(&self) -> Vec<String> {
        let mut owned = vec![self.handle()];
        if self.token && !self.keeps_legacy {
            let legacy = format!("@mcp:{}", legacy_handle_name(&self.name));
            if !owned.contains(&legacy) {
                owned.push(legacy);
            }
        }
        owned
    }

    /// Model name if the caller names none: „<Name> <Version>“, otherwise „unbekannt“.
    pub fn model(&self) -> String {
        match (self.name.trim(), self.version.trim()) {
            ("", _) => "unbekannt".to_owned(),
            (name, "") => name.to_owned(),
            (name, version) => format!("{name} {version}"),
        }
    }
}

#[derive(Default)]
struct Table {
    order: VecDeque<String>,
    clients: HashMap<String, Client>,
}

static SESSIONS: LazyLock<Mutex<Table>> = LazyLock::new(Mutex::default);
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn table() -> std::sync::MutexGuard<'static, Table> {
    SESSIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// New id that is hard to guess (32 hex characters) from the random keys of `RandomState`,
/// a counter and the time.
fn new_id() -> String {
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let part = |salt: u64| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u64(count);
        h.write_u128(nanos);
        h.finish()
    };
    format!("{:016x}{:016x}", part(1), part(2))
}

/// A client as the settings page „MCP-Zugang“ shows it (Round 9): one entry per handle,
/// times in Unix seconds. Only in the memory of the server (empty after a restart), at most
/// [`MAX_SESSIONS`] entries (the one not seen for the longest time drops out).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SeenClient {
    pub name: String,
    pub version: String,
    pub handle: String,
    pub first_seen: u64,
    pub last_seen: u64,
}

static SEEN: LazyLock<Mutex<HashMap<String, SeenClient>>> = LazyLock::new(Mutex::default);

fn seen() -> std::sync::MutexGuard<'static, HashMap<String, SeenClient>> {
    SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `initialize`: take over name and version, last seen = now; first seen = now if
/// the handle is new (otherwise the earlier point in time stays).
fn saw_start(client: &Client) {
    let now = now_secs();
    let handle = client.handle();
    let mut seen = seen();
    if !seen.contains_key(&handle) && seen.len() >= MAX_SESSIONS {
        let oldest = seen.values().min_by_key(|c| c.last_seen).map(|c| c.handle.clone());
        if let Some(oldest) = oldest {
            seen.remove(&oldest);
        }
    }
    let entry = seen.entry(handle.clone()).or_insert_with(|| SeenClient {
        name: String::new(),
        version: String::new(),
        handle,
        first_seen: now,
        last_seen: now,
    });
    entry.name.clone_from(&client.name);
    entry.version.clone_from(&client.version);
    entry.last_seen = now;
}

/// Clients seen, last seen first (ties by handle).
pub fn clients() -> Vec<SeenClient> {
    let mut list: Vec<SeenClient> = seen().values().cloned().collect();
    list.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then_with(|| a.handle.cmp(&b.handle)));
    list
}

/// Creates a session for `client` and returns its id.
pub fn start(client: Client) -> String {
    saw_start(&client);
    let id = new_id();
    let mut table = table();
    while table.order.len() >= MAX_SESSIONS {
        if let Some(oldest) = table.order.pop_front() {
            table.clients.remove(&oldest);
        }
    }
    table.order.push_back(id.clone());
    table.clients.insert(id.clone(), client);
    id
}

/// Client of the session `id`; unknown or missing id → `None`. Sets „zuletzt gesehen“ of the client.
pub fn client(id: &str) -> Option<Client> {
    let found = table().clients.get(id).cloned();
    if let Some(client) = &found
        && let Some(entry) = seen().get_mut(&client.handle())
    {
        entry.last_seen = now_secs();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests share the process-wide tables.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn client(name: &str, version: &str) -> Client {
        Client { name: name.into(), version: version.into(), ..Default::default() }
    }

    #[test]
    fn handles_are_lowercase_and_safe() {
        assert_eq!(client("Claude Code", "2.1").handle(), "@mcp:claude-code", "Leerzeichen → '-'");
        assert_eq!(client("", "").handle(), "@mcp:unbekannt", "leerer Name");
        assert_eq!(client("a/b@c", "").handle(), "@mcp:a-b-c", "Sonderzeichen → '-'");
        assert_eq!(client(&"x".repeat(50), "").handle().len(), "@mcp:".len() + 32, "höchstens 32 Zeichen");
    }

    #[test]
    fn umlauts_are_spelled_out_and_everything_else_collapses() {
        assert_eq!(handle_name("Laptop Küche"), "laptop-kueche", "ü → ue, Leerzeichen → '-' (Vertrag R23)");
        assert_eq!(handle_name("ÄÖÜ ß"), "aeoeue-ss", "auch große Umlaute: erst klein, dann umschreiben");
        assert_eq!(handle_name("Café Noël"), "cafe-noel", "Akzente fallen nach NFKD weg");
        assert_eq!(handle_name("ﬁx²"), "fix2", "NFKD löst Ligaturen und hochgestellte Ziffern auf");
        assert_eq!(handle_name("Ku\u{308}che"), "kueche", "ein zerlegtes ü (u + Trema) ist derselbe Umlaut (erst NFC)");
        assert_eq!(handle_name("  Laptop A  "), handle_name("laptop-a"), "Groß/klein und Leerzeichen fallen zusammen");
        assert_eq!(handle_name("Büro/1"), handle_name("buero 1"), "Sonderzeichen und Leerzeichen werden beide '-'");
        let long = format!("{}-eins", "x".repeat(32));
        assert_eq!(
            handle_name(&long),
            handle_name(&format!("{}-zwei", "x".repeat(32))),
            "nach 32 Zeichen zählt nichts"
        );
        assert_eq!(handle_name(&"ü".repeat(40)).chars().count(), 32, "gekürzt wird nach dem Umschreiben");
        assert_eq!(handle_name(""), "unbekannt", "leer → unbekannt");
        assert_eq!(handle_name("   "), "unbekannt", "nur Leerraum ist leer");
        assert_eq!(handle_name("日本"), "--", "Schriften ohne Umschrift werden '-' (nicht leer)");
    }

    #[test]
    fn the_legacy_rule_is_the_one_before_round_23() {
        assert_eq!(legacy_handle_name("Laptop Küche"), "laptop-k-che", "bis Runde 22 wurde ü zu '-'");
        assert_eq!(legacy_handle_name("Claude Code"), handle_name("Claude Code"), "ohne Umlaute sind beide gleich");
        assert_eq!(legacy_handle_name(""), "unbekannt", "leer → unbekannt, wie früher");
        assert_eq!(legacy_handle_name(&"x".repeat(50)).len(), 32, "höchstens 32 Zeichen, wie früher");
    }

    #[test]
    fn a_token_owns_its_legacy_handle_and_a_client_without_token_only_its_own() {
        let with_token = Client { name: "Laptop Küche".into(), token: true, ..Default::default() };
        assert_eq!(
            with_token.owned_handles(),
            ["@mcp:laptop-kueche", "@mcp:laptop-k-che"],
            "mit Token gilt für alte Beiträge zusätzlich der Alt-Handle"
        );
        assert_eq!(
            client("Laptop Küche", "").owned_handles(),
            ["@mcp:laptop-kueche"],
            "ohne Token nur der eigene Handle – sonst erlangte er die alten Beiträge eines Tokens"
        );
        let plain = Client { name: "laptop".into(), token: true, ..Default::default() };
        assert_eq!(plain.owned_handles(), ["@mcp:laptop"], "ohne Umlaute kein zweiter Eintrag");
    }

    #[test]
    fn tokens_that_only_the_new_rule_would_merge_keep_their_old_handles() {
        // Finding on Round 23: „Büro“ (up to R22 b-ro) and „buero“ were separate and would both yield buero.
        let pair = ["Büro", "buero"];
        assert!(keeps_legacy("Büro", pair), "der neue Handle buero gehört schon dem Token „buero“");
        assert!(!keeps_legacy("buero", pair), "ohne Umlaut ändert die Regel nichts – der Token bleibt, wo er war");
        assert_eq!(token_handle("Büro", pair), "@mcp:b-ro", "„Büro“ schreibt weiter unter dem Autor von vor Runde 23");
        assert_eq!(token_handle("buero", pair), "@mcp:buero", "„buero“ behält seinen Autor allein");
        let kept = Client { name: "Büro".into(), token: true, keeps_legacy: true, ..Default::default() };
        assert_eq!(kept.owned_handles(), ["@mcp:b-ro"], "er besitzt nur seinen Alt-Handle, nicht den des anderen");
        let other = Client { name: "buero".into(), token: true, ..Default::default() };
        assert_eq!(other.owned_handles(), ["@mcp:buero"], "und der andere besitzt b-ro nicht");

        // Two umlaut tokens with different legacy handles but the same new handle: both stay with the old one.
        let both = ["Büro", "Bueró"];
        assert_eq!(handle_name("Bueró"), handle_name("Büro"), "Voraussetzung: die neue Regel legt beide zusammen");
        assert_eq!(token_handle("Büro", both), "@mcp:b-ro", "unabhängig von der Reihenfolge");
        assert_eq!(token_handle("Bueró", both), "@mcp:buer-", "auch der zweite behält seinen Alt-Handle");

        // No new coincidence: alone, or already the same author before Round 23.
        assert!(!keeps_legacy("Büro", ["Büro"]), "der Token selbst zählt nicht als anderer");
        assert!(!keeps_legacy("Büro", ["Büro", "BÜRO"]), "teilten b-ro schon vor Runde 23: wechseln gemeinsam");
        assert_eq!(token_handle("Büro", ["Büro", "Laptop Küche"]), "@mcp:buero", "ohne Zusammenfall die neue Regel");
        let without = Client { name: "Büro".into(), keeps_legacy: true, ..Default::default() };
        assert_eq!(without.handle(), "@mcp:buero", "ohne Token gilt der Merker nie (Alt-Handle nur für Tokens)");
    }

    #[test]
    fn a_client_without_token_gets_the_local_prefix_only_when_a_token_owns_its_handle() {
        let tokens = ["Laptop Küche", "Büro"];
        assert_eq!(local_name("claude-code", tokens), "claude-code", "kein Token mit diesem Handle: Name bleibt");
        assert_eq!(local_name("laptop kueche", tokens), "lokal-laptop kueche", "gleicher Handle wie der Token");
        assert_eq!(handle_name(&local_name("laptop kueche", tokens)), "lokal-laptop-kueche", "Autor mit Vorsilbe");
        assert_eq!(
            local_name("laptop-k-che", tokens),
            "lokal-laptop-k-che",
            "auch der Alt-Handle des Tokens ist vergeben"
        );
        assert_eq!(local_name(" BÜRO ", tokens), "lokal-BÜRO", "Groß/klein und Rand zählen nicht");
        assert_eq!(
            local_name("lokal-Büro", tokens),
            "lokal-Büro",
            "schon mit Vorsilbe: bleibt (kein doppeltes lokal-)"
        );
        assert_eq!(
            local_name("büro", ["Büro", "lokal-büro"]),
            "lokal-lokal-büro",
            "ein alter Token mit lokal- im Namen (vor Runde 23) wird nicht getroffen"
        );
    }

    #[test]
    fn model_falls_back_to_name_and_version() {
        assert_eq!(client("claude-code", "2.1.0").model(), "claude-code 2.1.0");
        assert_eq!(client("claude-code", "").model(), "claude-code");
        assert_eq!(client("", "1").model(), "unbekannt");
    }

    #[test]
    fn sessions_remember_their_client_and_the_table_stays_bounded() {
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let first = start(client("erster", "1"));
        assert_eq!(super::client(&first), Some(client("erster", "1")), "Sitzung gleich wieder lesbar");
        assert_ne!(start(client("zweiter", "1")), first, "jede Sitzung bekommt eine eigene Id");
        for i in 0..MAX_SESSIONS {
            start(client(&format!("c{i}"), "1"));
        }
        assert_eq!(super::client(&first), None, "älteste Sitzung fällt bei voller Tabelle heraus");
        let (clients, order) = {
            let t = table();
            (t.clients.len(), t.order.len())
        };
        assert!(clients <= MAX_SESSIONS, "Tabelle bleibt begrenzt");
        assert_eq!(clients, order, "Reihenfolge und Tabelle stimmen überein");
        assert!(seen().len() <= MAX_SESSIONS, "auch die Liste der gesehenen Clients bleibt begrenzt");
    }

    #[test]
    fn seen_clients_have_one_entry_per_handle_and_keep_their_first_time() {
        // The other test fills both tables up to the limit; in parallel it could displace this entry.
        let _serial = SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = start(client("Sitzungsprobe Eins", "1.0"));
        let first = clients().into_iter().find(|c| c.handle == "@mcp:sitzungsprobe-eins").expect("gesehen");
        assert_eq!((first.name.as_str(), first.version.as_str()), ("Sitzungsprobe Eins", "1.0"), "aus clientInfo");
        assert!(first.first_seen <= first.last_seen && first.first_seen > 0, "beide Zeiten gesetzt: {first:?}");
        start(client("Sitzungsprobe Eins", "1.1"));
        let again: Vec<SeenClient> = clients().into_iter().filter(|c| c.handle == "@mcp:sitzungsprobe-eins").collect();
        assert_eq!(again.len(), 1, "je Handle ein Eintrag, auch nach zweitem initialize");
        assert_eq!(again[0].version, "1.1", "die neue Version gilt");
        assert_eq!(again[0].first_seen, first.first_seen, "zuerst gesehen bleibt");
        assert!(super::client(&id).is_some(), "Sitzung bekannt; client() setzt zuletzt gesehen");
        let list = clients();
        assert!(list.windows(2).all(|w| w[0].last_seen >= w[1].last_seen), "zuletzt gesehen zuerst");
    }
}
