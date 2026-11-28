// SPDX-License-Identifier: AGPL-3.0-or-later
//! Web search for `@belege` through MultiGPT (contract Round 13, task "websuche").
//!
//! **Sequence** ([`research`], from [`crate::assist::hooks::web_research`]): only for `@belege`, only if the switch
//! is on (`store_meta` `websearch_belege` = `'1'`, default off; [`enabled`]) and MultiGPT is set up.
//! First the preferred sources (`sources.*`): at most [`MAX_PREFERRED_CALLS`] calls of `web_search` with
//! "site:<host>" in the order of the list, then one general call. At most [`MAX_HITS`]
//! hits, of them at most [`MAX_FETCHES`] retrievals through `fetch_url`; per call [`CALL_TIMEOUT`], the whole
//! web search [`TOTAL_TIMEOUT`]. A page over [`PAGE_MAX_BYTES`] or with a timeout is
//! skipped and appears as a note. Whether a hit comes from a preferred source is decided by the
//! host of its address ([`search::host_matches`]).
//!
//! **Check in the code** ([`verify`]): quotations in the same line as `[Wn]` must be a substring of the retrieved
//! page, otherwise „(nicht belegt)“; unknown `[Wn]`, addresses not retrieved and missing evidence are
//! marked; the line „Quellen: …“ is set by the code. Every named page is stored as a row in `web_evidence`
//! (migration 0034, [`evidence`]) and can be adopted on the evidence page ([`pages`]) without JavaScript into
//! the author's library – only for „belegt“, only by the human.
//!
//! **RPC** ([`handles`], [`call`]; only through `POST /rpc`, not `/act`, no MCP tool):
//! - `websearch.get {}` → `{on, configured, preferred}`
//! - `websearch.set {on: bool}` → `{on}`
//! - `websearch.evidence {post}` → `{post, evidence: [{id, label, url, title, preferred, accessed, quote,
//!   verdict, library_title}]}`
//! - `websearch.adopt {evidence}` → `{title: TitleEntry, quote, highlight}`; only for `verdict = 'belegt'`,
//!   otherwise -32602.
//!
//! **No MCP tool:** the web search has its value in the check within `@belege`; an
//! MCP passage into the Internet would enlarge the attack surface, and MCP clients have web searches of their own.
//! The evidence stands in the posts and is readable through `read_node` (posts).
//!
//! Contents of web pages are data, not instructions: in the prompt only defused
//! ([`crate::assist::rag_facts::defuse`]) between `<<<seite` and `>>>`, escaped in pages. Messages of the
//! remote side run through the removal of the token of the session.

pub mod evidence;
pub mod fetch;
pub mod mock;
pub mod pages;
pub mod search;
pub mod verify;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use baruch_store::ConnectionKind;
use serde::Serialize;
use serde_json::{Value, json};

pub use pages::{pages, writes};
pub use search::research;

use crate::AppState;
use crate::connections::multigpt::Limits;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Key of the switch in `store_meta` (`'1'` = on; missing = off).
pub const SWITCH_KEY: &str = "websearch_belege";
/// Tools of the web search (SearXNG behind it) and of the page retrieval at MultiGPT.
pub use crate::connections::multigpt::{FETCH_URL_TOOL, WEB_SEARCH_TOOL};
/// At most this many calls of `web_search` with "site:<host>" (the first preferred sources).
pub const MAX_PREFERRED_CALLS: usize = 3;
/// At most this many hits over all calls.
pub const MAX_HITS: usize = 8;
/// At most this many retrievals through `fetch_url`.
pub const MAX_FETCHES: usize = 4;
/// Time limit per call (`web_search`, `fetch_url`).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// Time limit of the whole web search (connecting, searching, retrieving).
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
/// Largest page (text or HTML, as `fetch_url` delivers it).
pub const PAGE_MAX_BYTES: usize = 1024 * 1024;
/// This many characters of a page remain for the check of the quotations.
pub const CHECK_CHARS: usize = 200_000;
/// This many characters of a page go into the block of the user message.
pub const BLOCK_CHARS: usize = 1_500;
/// Longest search query (characters, without "site:").
pub const QUERY_CHARS: usize = 200;
/// Limits of a search call.
pub const SEARCH_LIMITS: Limits = Limits { timeout: CALL_TIMEOUT, max_body: 2 * 1024 * 1024 };
/// Limits of a retrieval: the answer of MultiGPT may be larger than the page (JSON, escaping, text
/// twice in `structuredContent` and `content`); the page itself is checked by [`fetch::parse_page`].
pub const FETCH_LIMITS: Limits = Limits { timeout: CALL_TIMEOUT, max_body: 4 * PAGE_MAX_BYTES };

/// Names in the `inputSchema` of `fetch_url` for the address (assumption, see docs/Architektur.md).
pub const URL_KEYS: &[&str] = &["url", "uri", "link", "href"];
/// Names for the content of a retrieved page.
pub const CONTENT_KEYS: &[&str] = &["content", "text", "body", "markdown", "html"];
/// Names for the title (hit and page).
pub const TITLE_KEYS: &[&str] = &["title", "name", "filename"];
/// Names for the snippet of a hit.
pub const SNIPPET_KEYS: &[&str] = &["snippet", "content", "description", "summary", "text"];
/// Names of a hit list in the answer of `web_search`.
pub const RESULT_LIST_KEYS: &[&str] = &["results", "hits", "items", "data", "documents", "organic", "result", "web"];

/// A hit of `web_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WebHit {
    pub url: String,
    pub title: String,
    pub snippet: String,
    /// The host of the address matches a preferred source.
    pub preferred: bool,
}

/// A page retrieved through `fetch_url`; only it may stand in the block and be named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WebPage {
    /// Number n of `[Wn]` (from 1).
    pub label: usize,
    pub url: String,
    pub title: String,
    pub preferred: bool,
    /// Text of the page (at most [`CHECK_CHARS`] characters) for the check of the quotations.
    pub text: String,
    /// Instant of the retrieval (UTC, `2026-11-07T10:00:00Z`; stored).
    pub accessed: String,
    /// Day of the retrieval as `TT.MM.JJJJ` (local time; displayed).
    pub day: String,
}

/// Result of a web search.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Research {
    /// The search query (without "site:").
    pub query: String,
    /// Hits (at most [`MAX_HITS`]).
    pub hits: Vec<WebHit>,
    /// Retrieved pages (at most [`MAX_FETCHES`]).
    pub pages: Vec<WebPage>,
    /// Notes (failed search, skipped page), already without token and on one line.
    pub notes: Vec<String>,
    /// Reason if the web search failed as a whole (not set up, time limit, tool missing …).
    pub failure: Option<String>,
}

impl Research {
    /// Page `[Wn]`.
    pub fn page(&self, label: usize) -> Option<&WebPage> {
        self.pages.iter().find(|p| p.label == label)
    }
}

/// Now as an ISO instant (UTC, stored) and as a day `TT.MM.JJJJ` (local time, displayed).
pub fn now_stamp() -> (String, String) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    stamp(secs)
}

/// Seconds since 1970 → (`JJJJ-MM-TTTHH:MM:SSZ` in UTC, day `TT.MM.JJJJ` in local time).
pub fn stamp(secs: u64) -> (String, String) {
    stamp_in(secs, &chrono::Local)
}

/// Like [`stamp`], the day in `zone` (tests without changing `TZ`).
pub fn stamp_in<Tz: chrono::TimeZone>(secs: u64, zone: &Tz) -> (String, String)
where
    Tz::Offset: std::fmt::Display,
{
    let at = i64::try_from(secs)
        .ok()
        .and_then(|s| chrono::DateTime::<chrono::Utc>::from_timestamp(s, 0))
        .unwrap_or_default();
    let iso = at.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let day = crate::localtime::german_day_in(&iso, zone);
    (iso, day)
}

/// Stored ISO instant (UTC, `2026-11-07T…`) → day `07.11.2026` in local time; otherwise unchanged.
pub fn day_of(iso: &str) -> String {
    crate::localtime::german_day(iso)
}

/// Is the switch on?
pub async fn enabled(state: &AppState) -> Result<bool, sqlx::Error> {
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
        .bind(SWITCH_KEY)
        .fetch_optional(state.store.pool())
        .await?;
    Ok(value.as_deref() == Some("1"))
}

/// Set the switch (off = row removed, like "missing").
pub async fn set_enabled(state: &AppState, on: bool) -> Result<(), sqlx::Error> {
    let pool = state.store.pool();
    if on {
        sqlx::query(
            "INSERT INTO store_meta (key, value) VALUES (?, '1') ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(SWITCH_KEY)
        .execute(pool)
        .await?;
    } else {
        sqlx::query("DELETE FROM store_meta WHERE key = ?").bind(SWITCH_KEY).execute(pool).await?;
    }
    Ok(())
}

/// Is an address entered for MultiGPT?
pub async fn configured(state: &AppState) -> bool {
    match state.store.connection_get(ConnectionKind::Multigpt).await {
        Ok(row) => row.config.get("url").and_then(Value::as_str).is_some_and(|u| !u.trim().is_empty()),
        Err(_) => false,
    }
}

const METHODS: &[&str] = &["websearch.get", "websearch.set", "websearch.evidence", "websearch.adopt"];

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

fn db(e: sqlx::Error) -> RpcError {
    RpcError::new(INTERNAL, format!("Speicher: {e}"))
}

/// Integer (number or digits as text).
pub(crate) fn int_param(params: &Value, name: &str) -> Result<i64, RpcError> {
    let wrong = || invalid(crate::i18n::tc("websearch_msg.rpc.number_expected", &[("name", name.into())]));
    match params.get(name) {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(wrong),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| wrong()),
        None | Some(Value::Null) => {
            Err(invalid(crate::i18n::tc("websearch_msg.rpc.missing", &[("name", name.into())])))
        }
        Some(_) => Err(wrong()),
    }
}

/// `websearch.get` (also for pages).
pub async fn status(state: &AppState) -> Result<Value, RpcError> {
    let on = enabled(state).await.map_err(db)?;
    let preferred = state.store.sources_list().await?.len();
    Ok(json!({ "on": on, "configured": configured(state).await, "preferred": preferred }))
}

/// RPC `websearch.*`.
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "websearch.get" => status(state).await,
        "websearch.set" => {
            let on = params
                .get("on")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid(crate::i18n::tc("websearch_msg.rpc.on", &[])))?;
            set_enabled(state, on).await.map_err(db)?;
            Ok(json!({ "on": on }))
        }
        "websearch.evidence" => {
            let post = int_param(&params, "post")?;
            let rows = evidence::for_post(state.store.pool(), post).await.map_err(db)?;
            Ok(json!({ "post": post, "evidence": rows }))
        }
        "websearch.adopt" => {
            let id = int_param(&params, "evidence")?;
            evidence::adopt(state, id).await
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("Unbekannte Methode {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_store_utc_and_show_the_local_day_in_german_order() {
        let utc = chrono::Utc;
        let berlin = chrono::FixedOffset::east_opt(3600).expect("eine Stunde Versatz ist gültig");
        // 2026-11-07T09:30:05Z = 20_764 days · 86_400 s + 34_205 s since 1970-01-01.
        assert_eq!(
            stamp_in(20_764 * 86_400 + 34_205, &utc),
            ("2026-11-07T09:30:05Z".to_owned(), "07.11.2026".to_owned()),
            "ISO (UTC) für die Datenbank, TT.MM.JJJJ für Block und Quellenzeile"
        );
        assert_eq!(stamp_in(0, &utc).1, "01.01.1970", "Anfang der Zählung");
        assert_eq!(stamp_in(11_016 * 86_400, &utc).1, "29.02.2000", "Schalttag im Jahr 2000 (durch 400 teilbar)");
        // 23:30 UTC on 07.11. is already 08.11. in Berlin: UTC stays stored, the retrieval date is the day
        // of the user (user 2026-11-07: local time instead of UTC).
        assert_eq!(
            stamp_in(20_764 * 86_400 + 84_600, &berlin),
            ("2026-11-07T23:30:00Z".to_owned(), "08.11.2026".to_owned()),
            "gespeichert UTC, angezeigt der Tag in Ortszeit"
        );
        assert_eq!(
            day_of("2026-11-07T09:30:05Z"),
            crate::localtime::german_day("2026-11-07T09:30:05Z"),
            "Tag aus dem gespeicherten Zeitpunkt, in Ortszeit"
        );
        assert_eq!(day_of("kaputt"), "kaputt", "Unlesbares bleibt, wie es ist");
    }
}
