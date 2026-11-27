// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reconciliation Baruch ↔ MultiGPT (Round 13, task "abgleich"; plan „RAG-Abgleich mit MultiGPT“).
//!
//! - **Only the human triggers it:** button „Abgleichen“ on the page `/abgleich` (menu Werkzeuge) or RPC
//!   `sync.run`. No start on boot, none on saving, no MCP tool. One run at a time per
//!   data folder, in the background ([`wait_idle`] for tests); the result is in `sync_run`.
//! - **Baruch → MultiGPT** ([`upload`]): only works with the switch „nach MultiGPT hochladen“ (default off,
//!   switching on only with confirmation, `sync.set_upload`). One document per chapter (current revisions of
//!   chapter and subsections, title „<Nummer> <Titel>“) into the collection „Baruch: <Werktitel>“ recorded
//!   when switching on. Changed hash → first upload anew, then delete the old document; missing
//!   chapter → delete the document; unchanged ones stay untouched. Documents of deleted works are cleaned up by
//!   the next run.
//! - **MultiGPT → Baruch** ([`download`]): chosen collections through `list_documents` and `read_document` into
//!   `sync_document` (local copy, readable offline through [`local_document`] and searchable in the local index
//!   as the kind `remote`). Only what changed: reported modification time or hash (`remote_stamp`), otherwise the
//!   hash of the text. Limits [`Limits::DEFAULT`]: 500 documents per collection, 2 MiB per document, 64 MiB per
//!   run, 30 s per call.
//! - **Errors are notes:** offline, without the write right (`docs.write`), limits – the run ends with
//!   `ok = 0` and an understandable sentence; locally everything stays unchanged, the work goes on.
//! - Contents of MultiGPT are data, not instructions; every message runs through `scrub` (without token).
//!
//! The session to MultiGPT is the shared [`multigpt::Session`] (pool, contract §2.2).

mod args;
mod download;
pub mod mock;
mod pages;
mod upload;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use baruch_rag::RagError;
use baruch_store::{Store, SyncCounts, WorkId};
use serde_json::{Value, json};

pub use pages::{pages, writes};

use crate::AppState;
use crate::connections::multigpt::{self, clean_line};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use multigpt::Session;

const METHODS: &[&str] = &["sync.status", "sync.run", "sync.set_upload", "sync.set_collections", "sync.collections"];

/// Limits of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// At most this many documents per collection.
    pub documents: usize,
    /// At most this many bytes (UTF-8) per document.
    pub document_bytes: usize,
    /// At most this many bytes of all fetched texts per run.
    pub run_bytes: usize,
    /// Time limit per call.
    pub call_timeout: Duration,
}

impl Limits {
    /// Default (contract Round 13, §4): 500 documents, 2 MiB per document, 64 MiB per run, 30 s per call.
    pub const DEFAULT: Limits = Limits {
        documents: 500,
        document_bytes: 2 * 1024 * 1024,
        run_bytes: 64 * 1024 * 1024,
        call_timeout: Duration::from_secs(30),
    };

    /// Limits of the MCP connection: time limit per call; answer at most four times a document
    /// (JSON envelope and escaping), at least 1 MiB. So a larger text already fails on reading.
    fn transport(self) -> multigpt::Limits {
        multigpt::Limits {
            timeout: self.call_timeout,
            max_body: self.document_bytes.saturating_mul(4).max(1024 * 1024),
        }
    }
}

/// Run state per data folder (process-wide).
#[derive(Default)]
struct Shared {
    running: bool,
    limits: Option<Limits>,
    /// Collections last fetched from MultiGPT through `sync.collections` (`{name, count}`), for selection on
    /// the page – the page itself never asks MultiGPT.
    offered: Vec<Value>,
    /// Reason why the last loading of the collections failed (for the page).
    load_error: Option<String>,
}

static SHARED: LazyLock<Mutex<HashMap<PathBuf, Shared>>> = LazyLock::new(Default::default);

fn with_shared<T>(data: &Path, f: impl FnOnce(&mut Shared) -> T) -> T {
    let mut all = SHARED.lock().unwrap_or_else(|e| e.into_inner());
    f(all.entry(data.to_owned()).or_default())
}

/// For tests: other limits for this data folder (otherwise [`Limits::DEFAULT`]).
pub fn set_limits(state: &AppState, limits: Limits) {
    with_shared(&state.data, |s| s.limits = Some(limits));
}

fn limits_of(data: &Path) -> Limits {
    with_shared(data, |s| s.limits).unwrap_or(Limits::DEFAULT)
}

/// Is a reconciliation running for this data folder right now?
pub fn running(state: &AppState) -> bool {
    with_shared(&state.data, |s| s.running)
}

/// For tests: waits until no reconciliation is running any more.
pub async fn wait_idle(state: &AppState) {
    while running(state) {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Address and token of the connection MultiGPT according to the rules of `open_way`: without an address
/// `NotConfigured`; the token only if it is stored for the present address, otherwise `NotConfigured`
/// with a note (it then goes nowhere). Reads only settings and keyring, no network.
pub(crate) async fn credentials(
    store: &Store,
    secrets: &Arc<dyn crate::connections::SecretStore>,
) -> Result<(String, Option<String>), RagError> {
    let row = store
        .connection_get(baruch_store::ConnectionKind::Multigpt)
        .await
        .map_err(|e| RagError::Failed(format!("Einstellungen der Verbindung MultiGPT: {e}")))?;
    let url = row.config.get("url").and_then(Value::as_str).unwrap_or("").trim().to_owned();
    if url.is_empty() {
        return Err(RagError::NotConfigured(
            "Keine Adresse für MultiGPT eingetragen (Einstellungen → Verbindungen).".into(),
        ));
    }
    let secret = crate::connections::secrets::get(secrets, "multigpt")
        .await
        .map_err(|e| RagError::Failed(format!("Schlüsselbund: {e}")))?;
    if secret.is_some() && !crate::connections::secret_bound(&row) {
        return Err(RagError::NotConfigured(
            "Das Token baruch/multigpt ist für eine andere Adresse hinterlegt; Baruch schickt es nicht an die \
             jetzige. Für die neue Adresse das Token neu eintragen (Einstellungen → Verbindungen)."
                .into(),
        ));
    }
    Ok((url, secret))
}

/// Is an address for MultiGPT entered (without network)?
pub(crate) async fn configured(state: &AppState) -> bool {
    state
        .store
        .connection_get(baruch_store::ConnectionKind::Multigpt)
        .await
        .is_ok_and(|row| row.config.get("url").and_then(Value::as_str).is_some_and(|u| !u.trim().is_empty()))
}

/// Instant for the display: `2026-11-07T17:53:42.451Z` (UTC) → `07.11.2026 18:53` in local time (other forms
/// unchanged).
pub fn display_time(stamp: &str) -> String {
    crate::localtime::german_minutes(stamp)
}

/// Collection of a work: „Baruch: <Werktitel>“ in one line, at most 200 characters.
pub fn collection_for(title: &str) -> String {
    clean_line(&format!("Baruch: {}", title.trim()), 200)
}

/// Connection of a run: opened only on first need; if that fails, the reason stays.
pub(crate) struct Link {
    credentials: Option<Result<(String, Option<String>), RagError>>,
    store: Store,
    secrets: Arc<dyn crate::connections::SecretStore>,
    limits: Limits,
    session: Option<Session>,
    failed: Option<RagError>,
}

impl Link {
    fn new(store: Store, secrets: Arc<dyn crate::connections::SecretStore>, limits: Limits) -> Link {
        Link { credentials: None, store, secrets, limits, session: None, failed: None }
    }

    /// The session (connected on first need); `Err` with the reason, also on every further attempt.
    pub(crate) async fn session(&mut self) -> Result<&mut Session, RagError> {
        if let Some(e) = &self.failed {
            return Err(e.clone());
        }
        if self.session.is_none() {
            if self.credentials.is_none() {
                self.credentials = Some(credentials(&self.store, &self.secrets).await);
            }
            let opened = match self.credentials.as_ref().expect("eben gesetzt") {
                Ok((url, secret)) => Session::connect(url, secret.as_deref(), self.limits.transport()).await,
                Err(e) => Err(e.clone()),
            };
            match opened {
                Ok(session) => self.session = Some(session),
                Err(e) => {
                    self.failed = Some(e.clone());
                    return Err(e);
                }
            }
        }
        Ok(self.session.as_mut().expect("eben verbunden"))
    }

    /// Does the session stand and did the last answer arrive completely (also an `isError`)? `false` after a
    /// transport or protocol error or without a connection.
    pub(crate) fn answered(&self) -> bool {
        self.failed.is_none() && self.session.as_ref().is_some_and(|s| !s.broken())
    }

    /// Text without the token (also before connecting).
    pub(crate) fn scrub(&self, text: &str) -> String {
        match (&self.session, &self.credentials) {
            (Some(session), _) => session.scrub(text),
            (None, Some(Ok((_, secret)))) => multigpt::scrub_text(text, secret.as_deref()),
            _ => text.to_owned(),
        }
    }

    /// Give the session back; after errors end it instead of putting it into the pool (do not pass on a disturbed
    /// session).
    async fn finish(self, clean: bool) {
        match self.session {
            Some(session) if clean => session.release().await,
            Some(session) => session.close().await,
            None => {}
        }
    }
}

/// Result of a run while it is running.
pub(crate) struct Report {
    pub counts: SyncCounts,
    pub notes: Vec<String>,
    pub ok: bool,
}

impl Report {
    /// Note (without token) and `ok = 0`.
    pub(crate) fn problem(&mut self, link: &Link, note: impl AsRef<str>) {
        self.ok = false;
        self.note(link, note);
    }

    /// Note (without token), `ok` stays.
    pub(crate) fn note(&mut self, link: &Link, note: impl AsRef<str>) {
        let line = clean_line(&link.scrub(note.as_ref()), 500);
        if !self.notes.contains(&line) {
            self.notes.push(line);
        }
    }
}

/// Sentence about an error of the remote side.
pub(crate) fn why(e: &RagError) -> String {
    match e {
        RagError::Denied(m) => format!(
            "MultiGPT verweigert den Zugriff ({m}). Zum Hochladen braucht der Schlüssel das Recht docs.write, zum \
             Holen docs.read."
        ),
        RagError::Unreachable(m) | RagError::Timeout(m) => {
            format!("MultiGPT ist nicht erreichbar ({m}); die Arbeit geht lokal weiter.")
        }
        RagError::NotConfigured(m) => m.clone(),
        other => other.message().to_owned(),
    }
}

/// A whole run (in the background).
async fn work(store: Store, secrets: Arc<dyn crate::connections::SecretStore>, data: PathBuf) -> Report {
    let limits = limits_of(&data);
    let mut link = Link::new(store.clone(), secrets, limits);
    let mut report = Report { counts: SyncCounts::default(), notes: Vec::new(), ok: true };
    upload::run(&store, &mut link, &mut report).await;
    let changed = download::run(&store, &mut link, &mut report, limits).await;
    if changed {
        crate::rag::notify_data(&data, None);
    }
    link.finish(report.ok).await;
    report
}

/// Short sentence about the result (above the notes).
fn summary(report: &Report) -> String {
    let c = &report.counts;
    let total = c.uploaded + c.replaced + c.removed + c.downloaded + c.unchanged + c.failed;
    if report.ok && total == 0 && report.notes.is_empty() {
        return "Nichts abzugleichen: kein Werk mit „nach MultiGPT hochladen“, keine Sammlung gewählt.".into();
    }
    if report.ok { "Abgleich fertig.".into() } else { "Abgleich mit Hinweisen beendet.".into() }
}

/// Clears the mark "running" also if the run aborts.
struct RunningGuard(PathBuf);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        with_shared(&self.0, |s| s.running = false);
    }
}

/// `sync.run`: start a run in the background; if one is already running, `started: false`.
pub async fn start(state: &AppState) -> Result<Value, RpcError> {
    let claimed = with_shared(&state.data, |s| {
        if s.running {
            false
        } else {
            s.running = true;
            true
        }
    });
    if !claimed {
        return Ok(json!({ "started": false, "running": true }));
    }
    let guard = RunningGuard(state.data.clone());
    let id = match state.store.sync_run_start().await {
        Ok(id) => id,
        Err(e) => return Err(RpcError::from(e)),
    };
    let (store, secrets, data) = (state.store.clone(), state.secrets.clone(), state.data.clone());
    tokio::spawn(async move {
        let _guard = guard;
        // Own task: a panic in the run becomes a note and does not leave the mark hanging.
        let report = tokio::spawn(work(store.clone(), secrets, data)).await.unwrap_or_else(|e| Report {
            counts: SyncCounts::default(),
            notes: vec![format!("Abgleich abgebrochen: {e}")],
            ok: false,
        });
        let mut message = summary(&report);
        for note in &report.notes {
            message.push('\n');
            message.push_str(note);
        }
        if let Err(e) = store.sync_run_finish(id, report.ok, &message, &report.counts).await {
            eprintln!("Abgleich: Ergebnis nicht gespeichert: {e}");
        }
    });
    Ok(json!({ "started": true, "running": true, "run": id }))
}

/// Local copy of a fetched document (for the integrator: document view offline).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LocalDocument {
    pub title: String,
    pub url: Option<String>,
    pub text: String,
    pub fetched: String,
}

/// Local copy of a document of a chosen collection; `None` if there is none.
pub async fn local_document(
    state: &AppState,
    collection: &str,
    document: &str,
) -> Result<Option<LocalDocument>, RagError> {
    let found = state
        .store
        .sync_document(collection.trim(), document.trim())
        .await
        .map_err(|e| RagError::Failed(format!("Speicher: {e}")))?;
    Ok(found.map(|d| LocalDocument {
        title: if d.title.is_empty() { d.document_id.clone() } else { d.title },
        url: Some(d.url).filter(|u| !u.is_empty()),
        text: d.text,
        fetched: d.fetched,
    }))
}

/// `sync.status`.
pub async fn status(state: &AppState, work: Option<WorkId>) -> Result<Value, RpcError> {
    let last = state.store.sync_run_last().await?;
    let mut works = Vec::new();
    for sw in state.store.sync_works().await? {
        if work.is_some_and(|w| w != sw.work) {
            continue;
        }
        let tally = state.store.sync_work_tally(sw.work).await?;
        works.push(json!({
            "work": sw.work, "title": sw.title, "upload": sw.upload, "collection": sw.collection,
            "confirmed": sw.confirmed, "documents": tally.documents, "synced": tally.synced,
        }));
    }
    if let Some(w) = work
        && works.is_empty()
    {
        return Err(RpcError::new(INVALID_PARAMS, format!("Unbekanntes Werk {w}.")));
    }
    let mut collections = Vec::new();
    for name in state.store.sync_collections().await? {
        let tally = state.store.sync_collection_tally(&name).await?;
        collections.push(json!({ "collection": name, "documents": tally.documents, "synced": tally.synced }));
    }
    let last = last.map(|run| {
        json!({
            "id": run.id, "started": run.started, "finished": run.finished, "ok": run.ok, "message": run.message,
            "uploaded": run.counts.uploaded, "replaced": run.counts.replaced, "removed": run.counts.removed,
            "downloaded": run.counts.downloaded, "unchanged": run.counts.unchanged, "failed": run.counts.failed,
        })
    });
    Ok(json!({
        "last": last, "running": running(state), "works": works, "collections": collections,
        "configured": configured(state).await,
    }))
}

/// `sync.set_upload {work, on, confirm?}`: switching on only with `confirm: true` (confirmation before the first
/// upload); records the collection „Baruch: <Werktitel>“. No network.
pub async fn set_upload(state: &AppState, work: WorkId, on: bool, confirm: bool) -> Result<Value, RpcError> {
    if on && !confirm {
        return Err(RpcError::new(
            INVALID_PARAMS,
            "confirm: true fehlt. Hochladen bitte bestätigen: Die aktuellen Fassungen dieses Werks gehen beim \
             nächsten Abgleich als Dokumente nach MultiGPT; nichts geändert.",
        ));
    }
    let meta = state.store.sync_work(work).await?;
    let saved = state.store.sync_set_upload(work, on, &collection_for(&meta.title)).await?;
    Ok(json!({ "work": work, "upload": saved.upload, "collection": saved.collection }))
}

/// `sync.set_collections {collections}`. No network.
pub async fn set_collections(state: &AppState, collections: &[String]) -> Result<Value, RpcError> {
    let saved = state.store.sync_set_collections(collections).await.map_err(|e| match e {
        baruch_store::StoreError::Refused(m) => RpcError::new(INVALID_PARAMS, m),
        other => RpcError::from(other),
    })?;
    Ok(json!({ "collections": saved }))
}

/// `sync.collections`: collections of MultiGPT (`list_collections`, online, only at the press of a button), marked
/// with the chosen ones; the page afterwards shows this list for selection.
pub async fn offered_collections(state: &AppState) -> Result<Value, RpcError> {
    let limits = limits_of(&state.data);
    let refused = |e: RagError| RpcError::new(REFUSED, why(&e));
    let mut session = Session::open(state, limits.transport()).await.map_err(refused)?;
    if session.tools().find(multigpt::COLLECTIONS_TOOL).is_none() {
        session.close().await;
        return Err(RpcError::new(REFUSED, "MultiGPT bietet das Werkzeug list_collections nicht an (tools/list)."));
    }
    let outcome = session.call(multigpt::COLLECTIONS_TOOL, json!({}), None).await;
    session.release().await;
    let result = outcome.map_err(refused)?;
    let payload = multigpt::tool_payload(&result).ok_or_else(|| {
        RpcError::new(REFUSED, "list_collections: Antwort nicht lesbar (weder structuredContent noch JSON in content).")
    })?;
    let found = multigpt::parse_collections(&payload);
    with_shared(&state.data, |s| s.offered = found.clone());
    Ok(json!({ "collections": decorate(state, found).await? }))
}

/// Collections with `selected` (chosen) for page and RPC.
async fn decorate(state: &AppState, found: Vec<Value>) -> Result<Vec<Value>, RpcError> {
    let chosen = state.store.sync_collections().await?;
    Ok(found
        .into_iter()
        .map(|mut c| {
            let selected = c["name"].as_str().is_some_and(|n| chosen.iter().any(|x| x == n));
            c["selected"] = json!(selected);
            c
        })
        .collect())
}

/// Collections fetched last (without network).
pub(crate) fn offered(state: &AppState) -> Vec<Value> {
    with_shared(&state.data, |s| s.offered.clone())
}

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn work_of(params: &Value) -> Result<Option<WorkId>, RpcError> {
    match params.get("work") {
        None | Some(Value::Null) => Some(None),
        Some(Value::Number(n)) => n.as_i64().filter(|w| *w > 0).map(Some),
        Some(Value::String(s)) => s.trim().parse::<WorkId>().ok().filter(|w| *w > 0).map(Some),
        _ => None,
    }
    .ok_or_else(|| RpcError::new(INVALID_PARAMS, "work: Werk-id (Zahl) erwartet."))
}

/// RPC `sync.*` (from `rpc::call`, before the work is chosen).
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err(RpcError::new(INVALID_PARAMS, "Parameter als Objekt erwartet"));
    }
    match method {
        "sync.status" => status(state, work_of(&params)?).await,
        "sync.run" => start(state).await,
        "sync.set_upload" => {
            let work = work_of(&params)?.ok_or_else(|| RpcError::new(INVALID_PARAMS, "work: Werk-id fehlt."))?;
            let on = params
                .get("on")
                .and_then(Value::as_bool)
                .ok_or_else(|| RpcError::new(INVALID_PARAMS, "on: true oder false erwartet."))?;
            let confirm = params.get("confirm") == Some(&Value::Bool(true));
            set_upload(state, work, on, confirm).await
        }
        "sync.set_collections" => {
            let list = params
                .get("collections")
                .and_then(Value::as_array)
                .and_then(|items| items.iter().map(|v| v.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
                .ok_or_else(|| RpcError::new(INVALID_PARAMS, "collections: Liste von Texten erwartet."))?;
            set_collections(state, &list).await
        }
        "sync.collections" => offered_collections(state).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_limits_are_those_of_the_contract() {
        assert_eq!(Limits::DEFAULT.documents, 500, "500 Dokumente je Sammlung");
        assert_eq!(Limits::DEFAULT.document_bytes, 2 * 1024 * 1024, "2 MiB je Dokument");
        assert_eq!(Limits::DEFAULT.run_bytes, 64 * 1024 * 1024, "64 MiB je Lauf");
        assert_eq!(Limits::DEFAULT.call_timeout, Duration::from_secs(30), "30 s je Aufruf");
        let transport = Limits::DEFAULT.transport();
        assert!(transport.max_body > Limits::DEFAULT.document_bytes, "die Hülle eines 2-MiB-Texts passt durch");
        assert_eq!(transport.timeout, Duration::from_secs(30), "Zeitlimit der Verbindung = je Aufruf");
    }

    #[test]
    fn the_collection_of_a_work_is_one_line_with_at_most_200_characters() {
        assert_eq!(collection_for(" Das Gartenbuch "), "Baruch: Das Gartenbuch", "Vorsatz und Titel");
        assert_eq!(collection_for("Zwei\nZeilen"), "Baruch: Zwei Zeilen", "eine Zeile");
        assert_eq!(collection_for(&"x".repeat(300)).chars().count(), 200, "höchstens 200 Zeichen");
    }
}

#[cfg(test)]
mod display_tests {
    use super::display_time;

    #[test]
    fn stamps_are_shown_as_day_month_year_and_minutes_in_local_time() {
        let shown = display_time("2026-11-07T17:53:42.451Z");
        assert_eq!(
            shown,
            crate::localtime::german_minutes("2026-11-07T17:53:42.451Z"),
            "ISO-Stempel des Stores → lesbares Datum mit Uhrzeit in Ortszeit (Umrechnung: localtime::tests)"
        );
        assert!(!shown.contains("UTC"), "Ortszeit statt UTC (Nutzer 2026-11-07): {shown}");
        assert_eq!(display_time("gestern"), "gestern", "anderes bleibt unverändert (nichts erfunden)");
        assert_eq!(display_time(""), "", "leer bleibt leer");
    }
}
