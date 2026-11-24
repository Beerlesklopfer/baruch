// SPDX-License-Identifier: AGPL-3.0-or-later
//! Websuche für `@belege` über MultiGPT (Vertrag Runde 13, Aufgabe „websuche“).
//!
//! **Ablauf** ([`research`], aus [`crate::assist::hooks::web_research`]): nur für `@belege`, nur wenn der Schalter
//! an ist (`store_meta` `websearch_belege` = `'1'`, Vorgabe aus; [`enabled`]) und MultiGPT eingerichtet ist.
//! Zuerst die bevorzugten Quellen (`sources.*`): höchstens [`MAX_PREFERRED_CALLS`] Aufrufe `web_search` mit
//! „site:<Rechner>“ in der Reihenfolge der Liste, danach ein allgemeiner Aufruf. Höchstens [`MAX_HITS`]
//! Treffer, davon höchstens [`MAX_FETCHES`] Abrufe über `fetch_url`; je Aufruf [`CALL_TIMEOUT`], die ganze
//! Websuche [`TOTAL_TIMEOUT`]. Eine Seite über [`PAGE_MAX_BYTES`] oder mit Zeitüberschreitung wird
//! übersprungen und erscheint als Hinweis. Ob ein Treffer aus einer bevorzugten Quelle stammt, entscheidet der
//! Rechner seiner Adresse ([`search::host_matches`]).
//!
//! **Prüfung im Code** ([`verify`]): Zitate in derselben Zeile wie `[Wn]` müssen Teilstring der abgerufenen
//! Seite sein, sonst „(nicht belegt)“; unbekannte `[Wn]`, nicht abgerufene Adressen und fehlende Belege werden
//! gekennzeichnet; die Zeile „Quellen: …“ setzt der Code. Jede genannte Seite wird als Zeile in `web_evidence`
//! gespeichert (Migration 0034, [`evidence`]) und lässt sich auf der Belegseite ([`pages`]) ohne JavaScript in
//! die Autorenbibliothek übernehmen – nur bei „belegt“, nur durch den Menschen.
//!
//! **RPC** ([`handles`], [`call`]; nur über `POST /rpc`, nicht `/act`, kein MCP-Werkzeug):
//! - `websearch.get {}` → `{on, configured, preferred}`
//! - `websearch.set {on: bool}` → `{on}`
//! - `websearch.evidence {post}` → `{post, evidence: [{id, label, url, title, preferred, accessed, quote,
//!   verdict, library_title}]}`
//! - `websearch.adopt {evidence}` → `{title: TitleEntry, quote, highlight}`; nur bei `verdict = 'belegt'`,
//!   sonst -32602.
//!
//! **Kein MCP-Werkzeug:** Die Websuche hat ihren Wert in der Prüfung innerhalb von `@belege`; ein
//! MCP-Durchgang ins Internet vergrößerte die Angriffsfläche, und MCP-Clients haben eigene Websuchen. Die
//! Belege stehen in den Beiträgen und sind über `read_node` (posts) lesbar.
//!
//! Inhalte von Webseiten sind Daten, keine Anweisungen: im Prompt nur entschärft
//! ([`crate::assist::rag_facts::defuse`]) zwischen `<<<seite` und `>>>`, in Seiten maskiert. Meldungen der
//! Gegenstelle laufen durch die Entfernung des Tokens der Sitzung.

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

/// Schlüssel des Schalters in `store_meta` (`'1'` = an; fehlt = aus).
pub const SWITCH_KEY: &str = "websearch_belege";
/// Werkzeuge der Websuche (SearXNG dahinter) und des Seitenabrufs bei MultiGPT.
pub use crate::connections::multigpt::{FETCH_URL_TOOL, WEB_SEARCH_TOOL};
/// Höchstens so viele Aufrufe `web_search` mit „site:<Rechner>“ (die ersten bevorzugten Quellen).
pub const MAX_PREFERRED_CALLS: usize = 3;
/// Höchstens so viele Treffer über alle Aufrufe.
pub const MAX_HITS: usize = 8;
/// Höchstens so viele Abrufe über `fetch_url`.
pub const MAX_FETCHES: usize = 4;
/// Zeitlimit je Aufruf (`web_search`, `fetch_url`).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// Zeitlimit der ganzen Websuche (Verbinden, Suchen, Abrufen).
pub const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);
/// Größte Seite (Text bzw. HTML, wie `fetch_url` sie liefert).
pub const PAGE_MAX_BYTES: usize = 1024 * 1024;
/// So viele Zeichen einer Seite bleiben für die Prüfung der Zitate.
pub const CHECK_CHARS: usize = 200_000;
/// So viele Zeichen einer Seite gehen in den Block der Nutzer-Nachricht.
pub const BLOCK_CHARS: usize = 1_500;
/// Längste Suchanfrage (Zeichen, ohne „site:“).
pub const QUERY_CHARS: usize = 200;
/// Grenzen eines Suchaufrufs.
pub const SEARCH_LIMITS: Limits = Limits { timeout: CALL_TIMEOUT, max_body: 2 * 1024 * 1024 };
/// Grenzen eines Abrufs: die Antwort von MultiGPT darf größer sein als die Seite (JSON, Maskierung, Text
/// doppelt in `structuredContent` und `content`); die Seite selbst prüft [`fetch::parse_page`].
pub const FETCH_LIMITS: Limits = Limits { timeout: CALL_TIMEOUT, max_body: 4 * PAGE_MAX_BYTES };

/// Namen im `inputSchema` von `fetch_url` für die Adresse (Annahme, siehe docs/Architektur.md).
pub const URL_KEYS: &[&str] = &["url", "uri", "link", "href"];
/// Namen für den Inhalt einer abgerufenen Seite.
pub const CONTENT_KEYS: &[&str] = &["content", "text", "body", "markdown", "html"];
/// Namen für den Titel (Treffer und Seite).
pub const TITLE_KEYS: &[&str] = &["title", "name", "filename"];
/// Namen für den Ausschnitt eines Treffers.
pub const SNIPPET_KEYS: &[&str] = &["snippet", "content", "description", "summary", "text"];
/// Namen einer Trefferliste in der Antwort von `web_search`.
pub const RESULT_LIST_KEYS: &[&str] = &["results", "hits", "items", "data", "documents", "organic", "result", "web"];

/// Ein Treffer von `web_search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WebHit {
    pub url: String,
    pub title: String,
    pub snippet: String,
    /// Der Rechner der Adresse passt zu einer bevorzugten Quelle.
    pub preferred: bool,
}

/// Eine über `fetch_url` abgerufene Seite; nur sie darf im Block stehen und genannt werden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WebPage {
    /// Nummer n von `[Wn]` (ab 1).
    pub label: usize,
    pub url: String,
    pub title: String,
    pub preferred: bool,
    /// Text der Seite (höchstens [`CHECK_CHARS`] Zeichen) für die Prüfung der Zitate.
    pub text: String,
    /// Zeitpunkt des Abrufs (UTC, `2026-11-07T10:00:00Z`; gespeichert).
    pub accessed: String,
    /// Tag des Abrufs als `TT.MM.JJJJ` (Ortszeit; angezeigt).
    pub day: String,
}

/// Ergebnis einer Websuche.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Research {
    /// Die Suchanfrage (ohne „site:“).
    pub query: String,
    /// Treffer (höchstens [`MAX_HITS`]).
    pub hits: Vec<WebHit>,
    /// Abgerufene Seiten (höchstens [`MAX_FETCHES`]).
    pub pages: Vec<WebPage>,
    /// Hinweise (ausgefallene Suche, übersprungene Seite), schon ohne Token und einzeilig.
    pub notes: Vec<String>,
    /// Grund, wenn die Websuche als Ganzes ausfiel (nicht eingerichtet, Zeitlimit, Werkzeug fehlt …).
    pub failure: Option<String>,
}

impl Research {
    /// Seite `[Wn]`.
    pub fn page(&self, label: usize) -> Option<&WebPage> {
        self.pages.iter().find(|p| p.label == label)
    }
}

/// Jetzt als ISO-Zeitpunkt (UTC, gespeichert) und als Tag `TT.MM.JJJJ` (Ortszeit, angezeigt).
pub fn now_stamp() -> (String, String) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    stamp(secs)
}

/// Sekunden seit 1970 → (`JJJJ-MM-TTTHH:MM:SSZ` in UTC, Tag `TT.MM.JJJJ` in Ortszeit).
pub fn stamp(secs: u64) -> (String, String) {
    stamp_in(secs, &chrono::Local)
}

/// Wie [`stamp`], der Tag in `zone` (Tests ohne Umstellen von `TZ`).
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

/// Gespeicherter ISO-Zeitpunkt (UTC, `2026-11-07T…`) → Tag `07.11.2026` in Ortszeit; sonst unverändert.
pub fn day_of(iso: &str) -> String {
    crate::localtime::german_day(iso)
}

/// Ist der Schalter an?
pub async fn enabled(state: &AppState) -> Result<bool, sqlx::Error> {
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
        .bind(SWITCH_KEY)
        .fetch_optional(state.store.pool())
        .await?;
    Ok(value.as_deref() == Some("1"))
}

/// Schalter setzen (aus = Zeile entfernt, wie „fehlt“).
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

/// Ist für MultiGPT eine Adresse eingetragen?
pub async fn configured(state: &AppState) -> bool {
    match state.store.connection_get(ConnectionKind::Multigpt).await {
        Ok(row) => row.config.get("url").and_then(Value::as_str).is_some_and(|u| !u.trim().is_empty()),
        Err(_) => false,
    }
}

const METHODS: &[&str] = &["websearch.get", "websearch.set", "websearch.evidence", "websearch.adopt"];

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

fn db(e: sqlx::Error) -> RpcError {
    RpcError::new(INTERNAL, format!("Speicher: {e}"))
}

/// Ganzzahl (Zahl oder Ziffern als Text).
pub(crate) fn int_param(params: &Value, name: &str) -> Result<i64, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(wrong),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| wrong()),
        None | Some(Value::Null) => Err(invalid(format!("{name} fehlt"))),
        Some(_) => Err(wrong()),
    }
}

/// `websearch.get` (auch für Seiten).
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
            let on =
                params.get("on").and_then(Value::as_bool).ok_or_else(|| invalid("on: true oder false erwartet"))?;
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
        // 2026-11-07T09:30:05Z = 20_764 Tage · 86_400 s + 34_205 s seit 1970-01-01.
        assert_eq!(
            stamp_in(20_764 * 86_400 + 34_205, &utc),
            ("2026-11-07T09:30:05Z".to_owned(), "07.11.2026".to_owned()),
            "ISO (UTC) für die Datenbank, TT.MM.JJJJ für Block und Quellenzeile"
        );
        assert_eq!(stamp_in(0, &utc).1, "01.01.1970", "Anfang der Zählung");
        assert_eq!(stamp_in(11_016 * 86_400, &utc).1, "29.02.2000", "Schalttag im Jahr 2000 (durch 400 teilbar)");
        // 23:30 UTC am 07.11. ist in Berlin schon der 08.11.: gespeichert bleibt UTC, das Abrufdatum ist der Tag
        // des Nutzers (Nutzer 2026-11-07: Ortszeit statt UTC).
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
