// SPDX-License-Identifier: AGPL-3.0-or-later
//! Meaning search (RAG, Round 12, task "lancedb"): the shared service over all three ways and the
//! main RAG LanceDB in the data folder.
//!
//! - [`search`]: asks the ways of the work at the same time (own time limit [`WAY_TIMEOUT`] per way), an
//!   error of one way never blocks the others; the hits are merged by [`merge::merge`] (RRF,
//!   duplicates removed). The answer always names all three ways with their state („verbunden“,
//!   „ausgefallen“, „aus“) and the reason. The same query is embedded at most once per embedding model
//!   ([`embed::Cached`]). Hit texts are data from sources, never instructions. At most
//!   [`MAX_REMOTE_SEARCHES`] searches ask the remote ways at the same time ([`remote_slot`]).
//! - [`status`]: state without network (index, settings, chosen collections, last search or check).
//! - Keeping the index current: [`notify`] from `mirror.rs` (every acceptance, save, switch,
//!   outline change, every idea and post) or from the reconciliation with MultiGPT (`crate::sync`,
//!   fetched documents of the kind `remote`) enqueues the work or the shared scope; a
//!   background task embeds only changed chunks ([`index`]). [`catch_up`] (start of the service)
//!   connects Eliezer as the embedder and enqueues all works; without an embedder (tests, helper commands)
//!   [`notify`] has no effect and the search reports honestly [`embed::NO_EMBEDDER`].
//! - [`reindex`]: discard the index of the work and rebuild it (with another model the whole index).
//! - [`check_lancedb`]: „Verbindung prüfen“ for LanceDB.
//! - RPC `rag.search`, `rag.status`, `rag.reindex` ([`handles`], [`call`]); `rag.reindex` human only.
//!
//! Runtime without a change to `AppState`: process-wide per data folder ([`index::Runtime`]), the
//! background task runs on the own runtime of `baruch-lancedb` with clones of store and
//! secrets.

mod check;
pub mod embed;
pub mod hooks;
pub mod index;
pub mod merge;
pub mod neighbors;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use baruch_rag::{
    Answer, DEFAULT_LIMIT, EmbedderFactory, Hit, IndexInfo, MAX_LIMIT, MAX_QUERY_CHARS, Query, RagError, SearchRequest,
    SearchWay, Status, Way, WayReport, WayState,
};
use baruch_store::{ConnectionKind, RagWay, StoreError, WorkId};
use serde_json::{Value, json};

pub use check::{PROBE_TEXT, check_lancedb};
pub use embed::{EliezerEmbedders, NO_EMBEDDER, NoEmbedders};
pub use index::{KINDS, MAX_ATTEMPTS};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use index::{Job, LanceWay, Runtime};

/// Guard per way in the service.
pub const WAY_TIMEOUT: Duration = Duration::from_secs(20);

/// At most this many searches ask the remote ways at the same time (pgvector opens one connection per search,
/// MultiGPT one session); further ones wait up to [`WAY_TIMEOUT`] and then report [`BUSY`].
pub const MAX_REMOTE_SEARCHES: usize = 4;
static REMOTE_SEARCHES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(MAX_REMOTE_SEARCHES);
/// Sentence: too many simultaneous searches on the remote ways.
pub const BUSY: &str = "Zu viele gleichzeitige Suchen; bitte gleich noch einmal versuchen.";

/// Sentence: way not chosen for the work.
pub const NOT_CHOSEN: &str = "Für dieses Werk nicht gewählt (Einstellungen → Verbindungen).";
/// Sentence: way left out in the request.
pub const NOT_ASKED: &str = "In dieser Anfrage nicht gefragt.";
/// Sentence: no collection chosen.
pub const NO_COLLECTION: &str = "Keine Sammlung gewählt (Einstellungen → Verbindungen).";
/// Sentence: remote way never checked.
pub const NEVER_CHECKED: &str = "noch nicht geprüft (Verbindung prüfen)";

const METHODS: &[&str] = &["rag.search", "rag.status", "rag.reindex"];

/// Runtimes per data folder.
static RUNTIMES: LazyLock<Mutex<HashMap<PathBuf, Arc<Runtime>>>> = LazyLock::new(Default::default);

/// Runtime of the data folder (created on first use).
pub(crate) fn runtime(state: &AppState) -> Arc<Runtime> {
    let mut all = RUNTIMES.lock().unwrap_or_else(|e| e.into_inner());
    let rt = all
        .entry(state.data.clone())
        .or_insert_with(|| Arc::new(Runtime::new(state.data.clone(), state.store.clone())))
        .clone();
    rt.refresh(&state.store);
    rt
}

/// `RagWay` of the store → `Way` of the interface (same names).
pub fn way_of(w: RagWay) -> Way {
    match w {
        RagWay::Lancedb => Way::Lancedb,
        RagWay::Pgvector => Way::Pgvector,
        RagWay::Multigpt => Way::Multigpt,
    }
}

/// `Way` → `RagWay`.
pub fn rag_way(w: Way) -> RagWay {
    match w {
        Way::Lancedb => RagWay::Lancedb,
        Way::Pgvector => RagWay::Pgvector,
        Way::Multigpt => RagWay::Multigpt,
    }
}

/// Work changed (or `None`: ideas without a work, library): bring the index up to date. Never blocks; without
/// a connected embedder nothing happens.
pub fn notify(state: &AppState, work: Option<WorkId>) {
    notify_data(&state.data, work);
}

/// Like [`notify`], only through the data folder (for background tasks without `AppState`, such as the
/// reconciliation with MultiGPT in `crate::sync`).
pub(crate) fn notify_data(data: &std::path::Path, work: Option<WorkId>) {
    // Without a runtime there is no embedder either: create nothing (every test opens its own data folders).
    let Some(rt) = RUNTIMES.lock().unwrap_or_else(|e| e.into_inner()).get(data).cloned() else { return };
    match work {
        Some(work) => rt.enqueue(&[Job::Work(work)]),
        None => rt.enqueue(&[Job::Shared]),
    }
}

/// Replace the embedders for this data folder (tests; also applies to pgvector in the service).
pub fn set_embedders(state: &AppState, factory: Arc<dyn EmbedderFactory>) {
    runtime(state).set_embedders(factory);
}

/// Eliezer as the embedder of this data folder (address and key from the connection `eliezer`).
pub fn eliezer_embedders(state: &AppState) -> Arc<dyn EmbedderFactory> {
    Arc::new(EliezerEmbedders::new(state.store.clone(), state.secrets.clone()))
}

/// Sentence shown when no embedder was connected at start because the data folder is an own one and the
/// connection Eliezer was never stored there (Round 19a). Begins like [`NO_EMBEDDER`].
pub const NO_EMBEDDER_OWN_DATA: &str = "Kein Einbettungsmodell angebunden: eigener Datenordner ohne gespeicherte \
Verbindung Eliezer – Baruch bettet beim Start nicht ein (sonst ginge eine Anfrage an die Vorgabe-Adresse). Eliezer \
unter Einstellungen → Verbindungen speichern, dann „Index neu aufbauen“; die Wortlaut-Suche (FTS) geht weiter.";

/// Does `data` lie elsewhere than the default data folder (`--data` or `BARUCH_DATA` with an own folder)?
fn own_data(data: &std::path::Path) -> bool {
    let canonical = |p: &std::path::Path| {
        std::fs::canonicalize(p).unwrap_or_else(|_| std::path::absolute(p).unwrap_or_else(|_| p.to_owned()))
    };
    canonical(data) != canonical(&baruch_store::default_data_dir())
}

/// Is the connection Eliezer stored (not only the default)?
async fn eliezer_saved(state: &AppState) -> bool {
    state.store.connection_get(ConnectionKind::Eliezer).await.is_ok_and(|row| row.updated.is_some())
}

/// Start of the service: Eliezer as the embedder (if none is set already) and enqueue all works,
/// the shared scope and the pruning of deleted works. With an own data folder and without a
/// stored connection Eliezer **no** embedder is set (no request to the default address at
/// start); the status then names [`NO_EMBEDDER_OWN_DATA`].
pub async fn catch_up(state: &AppState) {
    let rt = runtime(state);
    if rt.embedders().is_none() {
        if own_data(&state.data) && !eliezer_saved(state).await {
            rt.set_unbound(NO_EMBEDDER_OWN_DATA);
            return;
        }
        rt.set_embedders(eliezer_embedders(state));
    }
    let mut jobs = vec![Job::Prune, Job::Shared];
    match state.store.works().await {
        Ok(works) => jobs.extend(works.into_iter().map(|(w, _, _)| Job::Work(w))),
        Err(e) => eprintln!("RAG: Werke nicht lesbar, Index nicht nachgeführt: {e}"),
    }
    rt.enqueue(&jobs);
}

/// For tests: waits until the queue is empty and the background task is idle.
pub async fn wait_idle(state: &AppState) {
    let rt = runtime(state);
    while !rt.idle() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn store_rag(e: StoreError) -> RagError {
    match e {
        StoreError::UnknownWork(w) => RagError::Invalid(format!("Unbekanntes Werk {w}.")),
        other => RagError::Failed(format!("Speicher: {other}")),
    }
}

/// Ways of the work (choice of the human).
async fn work_ways(state: &AppState, work: WorkId) -> Result<Vec<Way>, RagError> {
    let (ways, _) = state.store.work_rag(work).await.map_err(store_rag)?;
    Ok(ways.into_iter().map(way_of).collect())
}

/// Check the query: trimmed 1..=1000 characters, limit 1..=50.
fn checked_query(request: &SearchRequest) -> Result<String, RagError> {
    let query = request.query.trim();
    let count = query.chars().count();
    if count == 0 || count > MAX_QUERY_CHARS {
        return Err(RagError::Invalid(format!("query: 1 bis {MAX_QUERY_CHARS} Zeichen, nicht {count}.")));
    }
    if !(1..=MAX_LIMIT).contains(&request.limit) {
        return Err(RagError::Invalid(format!("limit: 1 bis {MAX_LIMIT}, nicht {}.", request.limit)));
    }
    Ok(query.to_owned())
}

/// Is the remote way set up (host or address entered)?
async fn remote_configured(state: &AppState, way: Way) -> bool {
    let kind = match way {
        Way::Pgvector => ConnectionKind::Pgvector,
        Way::Multigpt => ConnectionKind::Multigpt,
        Way::Lancedb => return true,
    };
    let field = if way == Way::Pgvector { "host" } else { "url" };
    match state.store.connection_get(kind).await {
        Ok(row) => row.config.get(field).and_then(Value::as_str).is_some_and(|v| !v.trim().is_empty()),
        Err(_) => false,
    }
}

/// A planned way: open with a query, or "off" with a reason.
enum Planned {
    Ask(Box<dyn SearchWay>, Query),
    Off(String),
}

/// A way with its query, or the reason why it is not asked.
pub(crate) type Plan = Result<(Box<dyn SearchWay>, Query), String>;

/// Ask the ways at the same time, each with its own time limit; result in the order of the input.
pub(crate) async fn run_ways(planned: Vec<(Way, Plan)>, timeout: Duration) -> Vec<(WayReport, Vec<Hit>)> {
    let runs = planned.into_iter().map(|(way, plan)| async move {
        match plan {
            Err(reason) => (WayReport::new(way, WayState::Off, reason, 0, 0), Vec::new()),
            Ok((searcher, query)) => {
                let started = Instant::now();
                let result = tokio::time::timeout(timeout, searcher.search(&query)).await;
                let millis = started.elapsed().as_millis() as u64;
                match result {
                    Err(_) => {
                        let message = format!("Keine Antwort innerhalb von {} s; abgebrochen.", timeout.as_secs_f32());
                        (WayReport::new(way, WayState::Failed, message, 0, millis), Vec::new())
                    }
                    Ok(Err(e)) if e.is_off() => {
                        (WayReport::new(way, WayState::Off, e.message(), 0, millis), Vec::new())
                    }
                    Ok(Err(e)) => (WayReport::new(way, WayState::Failed, e.message(), 0, millis), Vec::new()),
                    Ok(Ok(mut hits)) => {
                        hits.truncate(query.limit);
                        // Hits belong to the way that delivers them.
                        hits.iter_mut().for_each(|h| h.way = way);
                        let message = format!("{} Treffer.", hits.len());
                        (WayReport::new(way, WayState::Connected, message, hits.len(), millis), hits)
                    }
                }
            }
        }
    });
    futures_join_all(runs).await
}

/// Get a slot for the remote ways from `slots` (for at most `wait`) if one of them would be asked. Without a slot
/// the remote plans are dropped; returned: the permit (the slot holds as long as it lives) and the
/// dropped ways (which [`search`] reports as „ausgefallen“ with [`BUSY`]). LanceDB is local and never
/// affected.
pub(crate) async fn remote_slot<'a>(
    planned: &mut [(Way, Plan)],
    slots: &'a tokio::sync::Semaphore,
    wait: Duration,
) -> (Option<tokio::sync::SemaphorePermit<'a>>, Vec<Way>) {
    let remote = |way: Way| way != Way::Lancedb;
    if !planned.iter().any(|(way, plan)| remote(*way) && plan.is_ok()) {
        return (None, Vec::new());
    }
    if let Ok(Ok(permit)) = tokio::time::timeout(wait, slots.acquire()).await {
        return (Some(permit), Vec::new());
    }
    let mut busy = Vec::new();
    for (way, plan) in planned.iter_mut() {
        if remote(*way) && plan.is_ok() {
            *plan = Err(BUSY.into());
            busy.push(*way);
        }
    }
    (None, busy)
}

/// All futures at the same time, results in input order (without a dependency of its own).
async fn futures_join_all<F: std::future::Future>(futures: impl IntoIterator<Item = F>) -> Vec<F::Output> {
    let mut set = Vec::new();
    for f in futures {
        set.push(Box::pin(f));
    }
    let mut out: Vec<Option<F::Output>> = (0..set.len()).map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut pending = false;
        for (i, f) in set.iter_mut().enumerate() {
            if out[i].is_none() {
                match f.as_mut().poll(cx) {
                    std::task::Poll::Ready(v) => out[i] = Some(v),
                    std::task::Poll::Pending => pending = true,
                }
            }
        }
        if pending { std::task::Poll::Pending } else { std::task::Poll::Ready(()) }
    })
    .await;
    out.into_iter().map(|v| v.expect("alle fertig")).collect()
}

/// Meaning search over the ways of the work (see the module header). `Err` only for an invalid request or
/// an unknown work.
pub async fn search(state: &AppState, request: &SearchRequest) -> Result<Answer, RagError> {
    let query = checked_query(request)?;
    let work = request.work;
    let chosen = work_ways(state, work).await?;
    let rt = runtime(state);
    let factory: Arc<dyn EmbedderFactory> = Arc::new(embed::Cached::new(rt.factory()));
    let mut planned = Vec::new();
    for way in Way::ALL {
        let plan = plan_way(state, &rt, &factory, request, &query, &chosen, way).await;
        planned.push((
            way,
            match plan {
                Planned::Ask(searcher, q) => Ok((searcher, q)),
                Planned::Off(reason) => Err(reason),
            },
        ));
    }
    let (permit, busy) = remote_slot(&mut planned, &REMOTE_SEARCHES, WAY_TIMEOUT).await;
    let mut results = run_ways(planned, WAY_TIMEOUT).await;
    drop(permit);
    for (report, _) in results.iter_mut().filter(|(r, _)| busy.contains(&r.way)) {
        *report = WayReport::new(report.way, WayState::Failed, BUSY, 0, 0);
    }
    let now = crate::models::iso_now();
    let mut per_way = Vec::new();
    let mut reports = Vec::new();
    for (report, hits) in results {
        // A rejected way (BUSY) was not asked: its last state stays.
        if report.way != Way::Lancedb && report.state != WayState::Off && !busy.contains(&report.way) {
            rt.reports.lock().unwrap_or_else(|e| e.into_inner()).insert(report.way, (now.clone(), report.clone()));
        }
        if report.state == WayState::Connected {
            per_way.push((report.way, hits));
        }
        reports.push(report);
    }
    if chosen.contains(&Way::Lancedb) {
        rt.refresh_shared();
    }
    Ok(Answer { work, query, hits: merge::merge(per_way, request.limit), ways: reports })
}

async fn plan_way(
    state: &AppState,
    rt: &Arc<Runtime>,
    factory: &Arc<dyn EmbedderFactory>,
    request: &SearchRequest,
    query: &str,
    chosen: &[Way],
    way: Way,
) -> Planned {
    if !chosen.contains(&way) {
        return Planned::Off(NOT_CHOSEN.into());
    }
    if request.ways.as_ref().is_some_and(|asked| !asked.contains(&way)) {
        return Planned::Off(NOT_ASKED.into());
    }
    let mut q = Query { text: query.to_owned(), limit: request.limit, collections: Vec::new(), work: request.work };
    if way == Way::Lancedb {
        if let Some(asked) = &request.collections {
            q.collections = asked.iter().filter(|c| KINDS.contains(&c.trim())).map(|c| c.trim().to_owned()).collect();
            if q.collections.is_empty() {
                return Planned::Off(
                    "In dieser Anfrage nicht gefragt (keine der Arten section, idea, highlight, library, remote)."
                        .into(),
                );
            }
        }
        return Planned::Ask(Box::new(LanceWay { rt: rt.clone(), factory: factory.clone() }), q);
    }
    let selected = match hooks::collections(state, request.work, way).await {
        Ok(selected) => selected,
        Err(e) => return Planned::Off(e.message().to_owned()),
    };
    q.collections = match &request.collections {
        Some(asked) => selected.into_iter().filter(|c| asked.iter().any(|a| a.trim() == c)).collect(),
        None => selected,
    };
    if q.collections.is_empty() {
        return Planned::Off(NO_COLLECTION.into());
    }
    let opened = match way {
        Way::Pgvector => hooks::open_pgvector(state, factory.clone()).await,
        _ => hooks::open_multigpt(state).await,
    };
    match opened {
        Ok(searcher) => Planned::Ask(searcher, q),
        Err(e) => Planned::Off(e.message().to_owned()),
    }
}

/// State of the local index for a work (without network).
async fn index_info(state: &AppState, rt: &Runtime, work: WorkId) -> IndexInfo {
    let mut info = IndexInfo {
        dir: String::new(),
        entries: 0,
        model: None,
        dimension: None,
        configured_model: String::new(),
        built: None,
        pending: rt.pending(),
        stale: rt.stale(),
        problem: None,
    };
    let cfg = match index::config(&state.store, &state.data).await {
        Ok(cfg) => cfg,
        Err(e) => {
            info.problem = Some(e.message().to_owned());
            return info;
        }
    };
    info.dir = cfg.dir.display().to_string();
    info.configured_model = cfg.configured.clone();
    let mut problem = match rt.index(&cfg.dir).await {
        Err(e) => Some(e.message().to_owned()),
        Ok(idx) => {
            info.entries = idx.count(Some(baruch_lancedb::Scope::Visible(work))).await.unwrap_or(0);
            match idx.meta() {
                Ok(meta) => {
                    info.model = meta.requested.clone();
                    info.dimension = meta.dimension;
                    info.built = meta.built.get(&format!("work:{work}")).cloned();
                    index::mismatch(&meta, &cfg).map(|e| e.message().to_owned())
                }
                Err(e) => Some(e.message().to_owned()),
            }
        }
    };
    if problem.is_none() && rt.embedders().is_none() {
        problem = Some(rt.unbound().unwrap_or_else(|| NO_EMBEDDER.to_owned()));
    }
    info.problem = problem.or_else(|| rt.problem().map(|e| e.message().to_owned()));
    if info.problem.is_none() && info.stale > 0 {
        // Honest: stale jobs are a problem, even if the last (cheap) job succeeded.
        info.problem = Some(stale_text(info.stale));
    }
    info
}

/// Sentence about stale jobs.
pub fn stale_text(stale: usize) -> String {
    let word = if stale == 1 { "Auftrag" } else { "Aufträge" };
    format!(
        "{stale} {word} liegengeblieben – erneuter Versuch nach einer Änderung oder „Index neu aufbauen“; Suchen \
         stoßen keinen neuen Versuch an."
    )
}

/// State of all ways of a work, without network.
pub async fn status(state: &AppState, work: WorkId) -> Result<Status, RagError> {
    let chosen = work_ways(state, work).await?;
    let rt = runtime(state);
    let index = index_info(state, &rt, work).await;
    let mut ways = Vec::new();
    for way in Way::ALL {
        let report = if !chosen.contains(&way) {
            WayReport::new(way, WayState::Off, NOT_CHOSEN, 0, 0)
        } else if way == Way::Lancedb {
            match &index.problem {
                Some(problem) if index.stale > 0 && !problem.contains("liegengeblieben") => {
                    WayReport::new(way, WayState::Failed, format!("{problem} {}", stale_text(index.stale)), 0, 0)
                }
                Some(problem) => WayReport::new(way, WayState::Failed, problem.clone(), 0, 0),
                None if index.pending > 0 => WayReport::new(
                    way,
                    WayState::Connected,
                    format!("Index mit {} Einträgen; {} Aufträge ausstehend.", index.entries, index.pending),
                    0,
                    0,
                ),
                None => {
                    WayReport::new(way, WayState::Connected, format!("Index mit {} Einträgen.", index.entries), 0, 0)
                }
            }
        } else {
            remote_status(state, &rt, work, way).await
        };
        ways.push(report);
    }
    Ok(Status { work, ways, index, fts: true })
}

/// State of a remote way from setting, collections and last search or check.
async fn remote_status(state: &AppState, rt: &Runtime, work: WorkId, way: Way) -> WayReport {
    if !remote_configured(state, way).await {
        return WayReport::new(way, WayState::Off, "Nicht eingerichtet (Einstellungen → Verbindungen).", 0, 0);
    }
    match hooks::collections(state, work, way).await {
        Ok(list) if list.is_empty() => return WayReport::new(way, WayState::Off, NO_COLLECTION, 0, 0),
        Ok(_) => {}
        Err(e) => return WayReport::new(way, WayState::Off, e.message(), 0, 0),
    }
    let kind = if way == Way::Pgvector { ConnectionKind::Pgvector } else { ConnectionKind::Multigpt };
    let check = state.store.connection_get(kind).await.ok().and_then(|row| row.check);
    let searched = rt.reports.lock().unwrap_or_else(|e| e.into_inner()).get(&way).cloned();
    match (searched, check) {
        (Some((at, report)), check) if check.as_ref().is_none_or(|c| c.at <= at) => {
            WayReport::new(way, report.state, format!("Letzte Suche {at}: {}", report.message), 0, 0)
        }
        (_, Some(check)) => {
            let state = if check.ok { WayState::Connected } else { WayState::Failed };
            WayReport::new(way, state, format!("Letzte Prüfung {}: {}", check.at, check.message), 0, 0)
        }
        (_, None) => WayReport::new(way, WayState::Off, NEVER_CHECKED, 0, 0),
    }
}

/// Result of [`reindex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reindexed {
    /// Outstanding jobs.
    pub pending: usize,
    /// The worker held the write lock right then: recorded, it resets after the running batch.
    pub deferred: bool,
}

/// Discard the index of the work and rebuild it (in the background); if model or prefix do not match or
/// the index file is damaged, the whole index. Never waits for the write lock: if the worker holds it,
/// a reset wish is recorded (`deferred`), and the worker aborts the running job. If no embedder was
/// connected at start ([`NO_EMBEDDER_OWN_DATA`]) and Eliezer has been stored in the meantime, it is
/// connected now.
pub async fn reindex_report(state: &AppState, work: WorkId) -> Result<Reindexed, RagError> {
    state.store.work_slug(work).await.map_err(store_rag)?;
    let rt = runtime(state);
    if rt.embedders().is_none() && rt.unbound().is_some() && eliezer_saved(state).await {
        rt.set_embedders(eliezer_embedders(state));
    }
    if rt.embedders().is_none() {
        return Err(RagError::NoEmbedder(rt.unbound().unwrap_or_else(|| NO_EMBEDDER.to_owned())));
    }
    index::config(&state.store, &state.data).await?;
    let Ok(held) = rt.writing.try_lock() else {
        rt.wish_reset(false, work);
        return Ok(Reindexed { pending: rt.pending(), deferred: true });
    };
    let works = std::collections::BTreeSet::from([work]);
    let jobs = index::reset_index(&rt, false, &works).await;
    drop(held);
    let jobs = jobs?;
    rt.clear_problem();
    rt.enqueue(&jobs);
    Ok(Reindexed { pending: rt.pending(), deferred: false })
}

/// Like [`reindex_report`], returning only the outstanding jobs (hook `search_hooks::rag_reindex`, page
/// Verbindungen).
pub async fn reindex(state: &AppState, work: WorkId) -> Result<usize, RagError> {
    reindex_report(state, work).await.map(|done| done.pending)
}

/// Answer of a search as JSON, every hit with `weak` ([`baruch_rag::is_weak`] of its similarity,
/// Round 19a; hits stay in their order, nothing is hidden).
pub fn answer_json(answer: &Answer) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(answer)?;
    if let Some(hits) = value.get_mut("hits").and_then(Value::as_array_mut) {
        for (hit, merged) in hits.iter_mut().zip(&answer.hits) {
            hit["weak"] = json!(baruch_rag::is_weak(merged.hit.score));
        }
    }
    Ok(value)
}

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn rpc_error(e: RagError) -> RpcError {
    match e {
        RagError::Invalid(m) => RpcError::new(INVALID_PARAMS, m),
        other => RpcError::new(REFUSED, other.message().to_owned()),
    }
}

/// Work from `work`; if it is missing, the default work. Unknown → -32602.
async fn work_param(state: &AppState, params: &Value) -> Result<WorkId, RpcError> {
    crate::rpc::work_param(state, params).await
}

/// Request from the RPC parameters (`rag.search`).
pub(crate) fn request_from(params: &Value, work: WorkId, origin: &str) -> Result<SearchRequest, RpcError> {
    let invalid = |m: &str| RpcError::new(INVALID_PARAMS, m.to_owned());
    let query = params.get("query").and_then(Value::as_str).ok_or_else(|| invalid("query: Text erwartet."))?;
    let limit = match params.get("limit") {
        None | Some(Value::Null) => DEFAULT_LIMIT,
        Some(v) => match v.as_u64() {
            Some(n @ 1..=50) => n as usize,
            _ => return Err(invalid("limit: ganze Zahl von 1 bis 50.")),
        },
    };
    let list = |name: &str| -> Result<Option<Vec<String>>, RpcError> {
        match params.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| v.as_str().map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()))
                .collect::<Option<Vec<_>>>()
                .map(Some)
                .ok_or_else(|| invalid(&format!("{name}: Liste von Texten erwartet."))),
            Some(_) => Err(invalid(&format!("{name}: Liste von Texten erwartet."))),
        }
    };
    let ways = match list("ways")? {
        None => None,
        Some(names) => Some(
            names
                .iter()
                .map(|n| Way::parse(n))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| invalid("ways: nur lancedb, pgvector, multigpt."))?,
        ),
    };
    let request = SearchRequest {
        work,
        query: query.to_owned(),
        ways,
        collections: list("collections")?,
        limit,
        origin: origin.to_owned(),
    };
    checked_query(&request).map_err(rpc_error)?;
    Ok(request)
}

/// RPC `rag.search`, `rag.status`, `rag.reindex` (from `rpc::call` before the work is chosen).
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    if !params.is_object() {
        return Err(RpcError::new(INVALID_PARAMS, "Parameter als Objekt erwartet"));
    }
    let to_json =
        |v: Result<Value, serde_json::Error>| v.map_err(|e| RpcError::new(crate::rpc::INTERNAL, e.to_string()));
    match method {
        "rag.search" => {
            let work = work_param(state, &params).await?;
            let request = request_from(&params, work, "Autor")?;
            let answer = search(state, &request).await.map_err(rpc_error)?;
            to_json(answer_json(&answer))
        }
        "rag.status" => {
            let work = work_param(state, &params).await?;
            to_json(serde_json::to_value(status(state, work).await.map_err(rpc_error)?))
        }
        "rag.reindex" => {
            if params.get("work").is_none_or(Value::is_null) {
                return Err(RpcError::new(INVALID_PARAMS, "work: Werk-id fehlt."));
            }
            let work = work_param(state, &params).await?;
            if params.get("confirm") != Some(&Value::Bool(true)) {
                return Err(RpcError::new(INVALID_PARAMS, "confirm: true fehlt; nichts geändert."));
            }
            let done = reindex_report(state, work).await.map_err(rpc_error)?;
            Ok(json!({ "work": work, "queued": true, "pending": done.pending, "deferred": done.deferred }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use baruch_rag::BoxFuture;

    use super::*;

    struct Fixed(Way, Result<Vec<Hit>, RagError>, Duration);

    impl SearchWay for Fixed {
        fn way(&self) -> Way {
            self.0
        }
        fn search<'a>(&'a self, _query: &'a Query) -> BoxFuture<'a, Result<Vec<Hit>, RagError>> {
            Box::pin(async move {
                tokio::time::sleep(self.2).await;
                self.1.clone()
            })
        }
    }

    fn query() -> Query {
        Query { text: "Leuchtturm".into(), limit: 5, collections: vec![], work: 1 }
    }

    fn hit(document: &str) -> Hit {
        Hit {
            way: Way::Multigpt,
            collection: "c".into(),
            document: document.into(),
            section: None,
            title: "t".into(),
            snippet: "s".into(),
            score: 0.5,
            url: None,
            meta: json!({}),
        }
    }

    fn plans() -> Vec<(Way, Plan)> {
        let ask = |way: Way| -> Plan { Ok((Box::new(Fixed(way, Ok(vec![]), Duration::ZERO)), query())) };
        vec![(Way::Lancedb, ask(Way::Lancedb)), (Way::Pgvector, ask(Way::Pgvector)), (Way::Multigpt, Err("aus".into()))]
    }

    #[tokio::test]
    async fn remote_ways_get_a_limited_number_of_slots_and_lancedb_never_waits() {
        let slots = tokio::sync::Semaphore::new(1);
        let mut first = plans();
        let (permit, busy) = remote_slot(&mut first, &slots, Duration::from_millis(10)).await;
        assert!(permit.is_some() && busy.is_empty(), "freier Platz: die Suche darf die Remote-Wege fragen");

        // Reason: the only slot is taken; a further search (triggered for instance by a foreign page)
        // opens no further pgvector connection, but reports BUSY honestly.
        let mut second = plans();
        let (none, busy) = remote_slot(&mut second, &slots, Duration::from_millis(10)).await;
        assert!(none.is_none(), "kein zweiter Platz");
        assert_eq!(busy, vec![Way::Pgvector], "nur der gefragte Remote-Weg wird gestrichen");
        assert!(second[0].1.is_ok(), "LanceDB (lokal) läuft trotzdem");
        assert_eq!(second[1].1.as_ref().err().map(String::as_str), Some(BUSY), "pgvector mit Grund gestrichen");
        assert_eq!(second[2].1.as_ref().err().map(String::as_str), Some("aus"), "„aus“ bleibt „aus“");

        let mut local = vec![(Way::Lancedb, plans().remove(0).1)];
        let (none, busy) = remote_slot(&mut local, &slots, Duration::from_secs(5)).await;
        assert!(none.is_none() && busy.is_empty() && local[0].1.is_ok(), "nur LanceDB: kein Platz nötig, kein Warten");

        drop(permit);
        let mut third = plans();
        let (again, busy) = remote_slot(&mut third, &slots, Duration::from_millis(10)).await;
        assert!(again.is_some() && busy.is_empty(), "nach dem Ende der ersten Suche ist der Platz wieder frei");
        assert_eq!(MAX_REMOTE_SEARCHES, 4, "im Dienst vier Plätze");
    }

    #[tokio::test]
    async fn a_hanging_or_failing_way_never_blocks_the_others() {
        let slow: Box<dyn SearchWay> = Box::new(Fixed(Way::Pgvector, Ok(vec![]), Duration::from_secs(30)));
        let broken: Box<dyn SearchWay> =
            Box::new(Fixed(Way::Multigpt, Err(RagError::Unreachable("weg".into())), Duration::ZERO));
        let good: Box<dyn SearchWay> = Box::new(Fixed(Way::Lancedb, Ok(vec![hit("d")]), Duration::from_millis(10)));
        let started = Instant::now();
        let out = run_ways(
            vec![
                (Way::Lancedb, Ok((good, query()))),
                (Way::Pgvector, Ok((slow, query()))),
                (Way::Multigpt, Ok((broken, query()))),
            ],
            Duration::from_millis(200),
        )
        .await;
        assert!(started.elapsed() < Duration::from_secs(5), "gleichzeitig, mit Zeitlimit: {:?}", started.elapsed());
        assert_eq!(out[0].0.state, WayState::Connected, "LanceDB antwortet: {:?}", out[0].0);
        assert_eq!(out[0].1[0].way, Way::Lancedb, "Treffer tragen den Weg, der sie liefert");
        assert_eq!(out[1].0.state, WayState::Failed, "pgvector hängt → ausgefallen: {:?}", out[1].0);
        assert!(out[1].0.message.contains("Keine Antwort"), "ehrlicher Grund: {:?}", out[1].0);
        assert_eq!((out[2].0.state, out[2].0.message.as_str()), (WayState::Failed, "weg"), "Fehler → ausgefallen");
    }

    #[tokio::test]
    async fn not_configured_means_off_and_planned_off_ways_are_not_asked() {
        let off: Box<dyn SearchWay> =
            Box::new(Fixed(Way::Pgvector, Err(RagError::NotConfigured("kein Rechner".into())), Duration::ZERO));
        let out = run_ways(
            vec![(Way::Pgvector, Ok((off, query()))), (Way::Multigpt, Err(NO_COLLECTION.to_owned()))],
            Duration::from_secs(1),
        )
        .await;
        assert_eq!((out[0].0.state, out[0].0.label), (WayState::Off, "aus"), "NotConfigured = aus");
        assert_eq!(out[1].0.message, NO_COLLECTION, "nicht gefragt, mit Grund");
    }

    #[test]
    fn way_names_map_both_ways() {
        for way in RagWay::ALL {
            assert_eq!(rag_way(way_of(way)), way, "{way}: hin und zurück");
            assert_eq!(way_of(way).as_str(), way.as_str(), "gleiche Namen");
        }
    }

    #[test]
    fn rpc_parameters_are_checked_before_searching() {
        let code = |params: Value| request_from(&params, 1, "Autor").err().map(|e| e.code);
        assert_eq!(code(json!({ "query": "Leuchtturm" })), None, "gültig");
        assert_eq!(code(json!({})), Some(INVALID_PARAMS), "query fehlt");
        assert_eq!(code(json!({ "query": "   " })), Some(INVALID_PARAMS), "leer nach dem Beschneiden");
        assert_eq!(code(json!({ "query": "x".repeat(1001) })), Some(INVALID_PARAMS), "über 1000 Zeichen");
        assert_eq!(code(json!({ "query": "x".repeat(1000) })), None, "genau 1000 gehen");
        assert_eq!(code(json!({ "query": "x", "limit": 0 })), Some(INVALID_PARAMS), "limit 0");
        assert_eq!(code(json!({ "query": "x", "limit": 51 })), Some(INVALID_PARAMS), "limit 51");
        assert_eq!(code(json!({ "query": "x", "limit": "5" })), Some(INVALID_PARAMS), "limit als Text");
        assert_eq!(code(json!({ "query": "x", "ways": ["fts"] })), Some(INVALID_PARAMS), "unbekannter Weg");
        assert_eq!(code(json!({ "query": "x", "ways": "lancedb" })), Some(INVALID_PARAMS), "ways keine Liste");
        let request =
            request_from(&json!({ "query": " Senfkorn ", "ways": ["multigpt", "lancedb"] }), 4, "Autor").unwrap();
        assert_eq!(request.limit, DEFAULT_LIMIT, "Vorgabe 10");
        assert_eq!(request.ways, Some(vec![Way::Multigpt, Way::Lancedb]), "Wege übernommen");
        assert_eq!((request.work, request.origin.as_str()), (4, "Autor"), "Werk und Herkunft");
    }
}
