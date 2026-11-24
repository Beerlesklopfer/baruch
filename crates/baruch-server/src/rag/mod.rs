// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bedeutungssuche (RAG, Runde 12, Aufgabe „lancedb“): der gemeinsame Dienst über alle drei Wege und das
//! Haupt-RAG LanceDB im Datenordner.
//!
//! - [`search`]: fragt die Wege des Werks gleichzeitig (je Weg eigenes Zeitlimit [`WAY_TIMEOUT`]), ein
//!   Fehler eines Wegs blockiert die anderen nie; die Treffer führt [`merge::merge`] zusammen (RRF,
//!   Doppelte entfernt). Die Antwort nennt immer alle drei Wege mit Zustand („verbunden“,
//!   „ausgefallen“, „aus“) und Grund. Dieselbe Anfrage wird je Einbettungsmodell höchstens einmal
//!   eingebettet ([`embed::Cached`]). Treffer-Texte sind Daten aus Quellen, nie Anweisungen. Höchstens
//!   [`MAX_REMOTE_SEARCHES`] Suchen fragen gleichzeitig die Remote-Wege ([`remote_slot`]).
//! - [`status`]: Zustand ohne Netz (Index, Einstellungen, gewählte Sammlungen, letzte Suche bzw. Prüfung).
//! - Index aktuell halten: [`notify`] aus `mirror.rs` (jede Übernahme, Speicherung, Umschaltung,
//!   Gliederungsänderung, jeder Einfall und Beitrag) bzw. aus dem Abgleich mit MultiGPT (`crate::sync`,
//!   geholte Dokumente der Art `remote`) reiht das Werk bzw. den gemeinsamen Bereich ein; eine
//!   Hintergrund-Aufgabe bettet nur geänderte Stücke ein ([`index`]). [`catch_up`] (Start des Dienstes)
//!   bindet Eliezer als Einbetter an und reiht alle Werke ein; ohne Einbetter (Tests, Hilfsbefehle) bleibt
//!   [`notify`] wirkungslos und die Suche meldet ehrlich [`embed::NO_EMBEDDER`].
//! - [`reindex`]: Index des Werks verwerfen und neu aufbauen (bei anderem Modell den ganzen Index).
//! - [`check_lancedb`]: „Verbindung prüfen“ für LanceDB.
//! - RPC `rag.search`, `rag.status`, `rag.reindex` ([`handles`], [`call`]); `rag.reindex` nur Mensch.
//!
//! Laufzeit ohne Änderung an `AppState`: prozessweit je Datenordner ([`index::Runtime`]), die
//! Hintergrund-Aufgabe läuft auf der eigenen Laufzeit von `baruch-lancedb` mit Klonen von Store und
//! Geheimnissen.

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

/// Schutz je Weg im Dienst.
pub const WAY_TIMEOUT: Duration = Duration::from_secs(20);

/// Höchstens so viele Suchen fragen gleichzeitig die Remote-Wege (pgvector öffnet je Suche eine Verbindung,
/// MultiGPT eine Sitzung); weitere warten bis [`WAY_TIMEOUT`] und melden dann [`BUSY`].
pub const MAX_REMOTE_SEARCHES: usize = 4;
static REMOTE_SEARCHES: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(MAX_REMOTE_SEARCHES);
/// Satz: zu viele gleichzeitige Suchen auf den Remote-Wegen.
pub const BUSY: &str = "Zu viele gleichzeitige Suchen; bitte gleich noch einmal versuchen.";

/// Satz: Weg für das Werk nicht gewählt.
pub const NOT_CHOSEN: &str = "Für dieses Werk nicht gewählt (Einstellungen → Verbindungen).";
/// Satz: Weg in der Anfrage weggelassen.
pub const NOT_ASKED: &str = "In dieser Anfrage nicht gefragt.";
/// Satz: keine Sammlung gewählt.
pub const NO_COLLECTION: &str = "Keine Sammlung gewählt (Einstellungen → Verbindungen).";
/// Satz: Remote-Weg nie geprüft.
pub const NEVER_CHECKED: &str = "noch nicht geprüft (Verbindung prüfen)";

const METHODS: &[&str] = &["rag.search", "rag.status", "rag.reindex"];

/// Laufzeiten je Datenordner.
static RUNTIMES: LazyLock<Mutex<HashMap<PathBuf, Arc<Runtime>>>> = LazyLock::new(Default::default);

/// Laufzeit des Datenordners (beim ersten Gebrauch angelegt).
pub(crate) fn runtime(state: &AppState) -> Arc<Runtime> {
    let mut all = RUNTIMES.lock().unwrap_or_else(|e| e.into_inner());
    let rt = all
        .entry(state.data.clone())
        .or_insert_with(|| Arc::new(Runtime::new(state.data.clone(), state.store.clone())))
        .clone();
    rt.refresh(&state.store);
    rt
}

/// `RagWay` des Stores → `Way` der Schnittstelle (gleiche Namen).
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

/// Werk geändert (bzw. `None`: Einfälle ohne Werk, Bibliothek): Index nachziehen. Blockiert nie; ohne
/// angebundenen Einbetter geschieht nichts.
pub fn notify(state: &AppState, work: Option<WorkId>) {
    notify_data(&state.data, work);
}

/// Wie [`notify`], nur über den Datenordner (für Hintergrund-Aufgaben ohne `AppState`, etwa den Abgleich
/// mit MultiGPT in `crate::sync`).
pub(crate) fn notify_data(data: &std::path::Path, work: Option<WorkId>) {
    // Ohne Laufzeit gibt es auch keinen Einbetter: nichts anlegen (jeder Test öffnet eigene Datenordner).
    let Some(rt) = RUNTIMES.lock().unwrap_or_else(|e| e.into_inner()).get(data).cloned() else { return };
    match work {
        Some(work) => rt.enqueue(&[Job::Work(work)]),
        None => rt.enqueue(&[Job::Shared]),
    }
}

/// Einbetter für diesen Datenordner ersetzen (Tests; gilt auch für pgvector im Dienst).
pub fn set_embedders(state: &AppState, factory: Arc<dyn EmbedderFactory>) {
    runtime(state).set_embedders(factory);
}

/// Eliezer als Einbetter dieses Datenordners (Adresse und Schlüssel aus der Verbindung `eliezer`).
pub fn eliezer_embedders(state: &AppState) -> Arc<dyn EmbedderFactory> {
    Arc::new(EliezerEmbedders::new(state.store.clone(), state.secrets.clone()))
}

/// Satz, wenn beim Start kein Einbetter angebunden wurde, weil der Datenordner ein eigener ist und die Verbindung
/// Eliezer dort nie gespeichert wurde (Runde 19a). Beginnt wie [`NO_EMBEDDER`].
pub const NO_EMBEDDER_OWN_DATA: &str = "Kein Einbettungsmodell angebunden: eigener Datenordner ohne gespeicherte \
Verbindung Eliezer – Baruch bettet beim Start nicht ein (sonst ginge eine Anfrage an die Vorgabe-Adresse). Eliezer \
unter Einstellungen → Verbindungen speichern, dann „Index neu aufbauen“; die Wortlaut-Suche (FTS) geht weiter.";

/// Liegt `data` woanders als der Vorgabe-Datenordner (`--data` bzw. `BARUCH_DATA` mit eigenem Ordner)?
fn own_data(data: &std::path::Path) -> bool {
    let canonical = |p: &std::path::Path| {
        std::fs::canonicalize(p).unwrap_or_else(|_| std::path::absolute(p).unwrap_or_else(|_| p.to_owned()))
    };
    canonical(data) != canonical(&baruch_store::default_data_dir())
}

/// Ist die Verbindung Eliezer gespeichert (nicht nur die Vorgabe)?
async fn eliezer_saved(state: &AppState) -> bool {
    state.store.connection_get(ConnectionKind::Eliezer).await.is_ok_and(|row| row.updated.is_some())
}

/// Start des Dienstes: Eliezer als Einbetter (falls nicht schon einer gesetzt ist) und alle Werke,
/// der gemeinsame Bereich und das Aufräumen gelöschter Werke einreihen. Mit eigenem Datenordner und ohne
/// gespeicherte Verbindung Eliezer wird **kein** Einbetter gesetzt (keine Anfrage an die Vorgabe-Adresse beim
/// Start); der Status nennt dann [`NO_EMBEDDER_OWN_DATA`].
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

/// Für Tests: wartet, bis die Warteschlange leer ist und die Hintergrund-Aufgabe ruht.
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

/// Wege des Werks (Wahl des Menschen).
async fn work_ways(state: &AppState, work: WorkId) -> Result<Vec<Way>, RagError> {
    let (ways, _) = state.store.work_rag(work).await.map_err(store_rag)?;
    Ok(ways.into_iter().map(way_of).collect())
}

/// Anfrage prüfen: beschnitten 1..=1000 Zeichen, limit 1..=50.
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

/// Ist der Remote-Weg eingerichtet (Rechner bzw. Adresse eingetragen)?
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

/// Ein geplanter Weg: offen mit Anfrage oder „aus“ mit Grund.
enum Planned {
    Ask(Box<dyn SearchWay>, Query),
    Off(String),
}

/// Ein Weg mit Anfrage bzw. der Grund, warum er nicht gefragt wird.
pub(crate) type Plan = Result<(Box<dyn SearchWay>, Query), String>;

/// Wege gleichzeitig fragen, jeder mit eigenem Zeitlimit; Ergebnis in der Reihenfolge der Eingabe.
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
                        // Treffer gehören zum Weg, der sie liefert.
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

/// Platz für die Remote-Wege aus `slots` holen (höchstens `wait` lang), falls einer gefragt würde. Ohne Platz
/// werden die Remote-Pläne gestrichen; Rückgabe: Erlaubnis (solange sie lebt, gilt der Platz) und die
/// gestrichenen Wege (die [`search`] als „ausgefallen“ mit [`BUSY`] meldet). LanceDB ist lokal und nie
/// betroffen.
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

/// Alle Futures gleichzeitig, Ergebnisse in Eingabereihenfolge (ohne eigene Abhängigkeit).
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

/// Bedeutungssuche über die Wege des Werks (siehe Modulkopf). `Err` nur bei ungültiger Anfrage bzw.
/// unbekanntem Werk.
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
        // Ein abgewiesener Weg (BUSY) wurde nicht gefragt: sein letzter Zustand bleibt stehen.
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

/// Zustand des lokalen Index für ein Werk (ohne Netz).
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
        // Ehrlich: Liegengebliebenes ist ein Problem, auch wenn der letzte (billige) Auftrag gelang.
        info.problem = Some(stale_text(info.stale));
    }
    info
}

/// Satz zu liegengebliebenen Aufträgen.
pub fn stale_text(stale: usize) -> String {
    let word = if stale == 1 { "Auftrag" } else { "Aufträge" };
    format!(
        "{stale} {word} liegengeblieben – erneuter Versuch nach einer Änderung oder „Index neu aufbauen“; Suchen \
         stoßen keinen neuen Versuch an."
    )
}

/// Zustand aller Wege eines Werks, ohne Netz.
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

/// Zustand eines Remote-Wegs aus Einstellung, Sammlungen und letzter Suche bzw. Prüfung.
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

/// Ergebnis von [`reindex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reindexed {
    /// Ausstehende Aufträge.
    pub pending: usize,
    /// Der Worker hielt gerade die Schreibsperre: vermerkt, er setzt nach dem laufenden Stapel zurück.
    pub deferred: bool,
}

/// Index des Werks verwerfen und neu aufbauen (im Hintergrund); passt das Modell bzw. Präfix nicht oder ist
/// die Indexdatei beschädigt, den ganzen Index. Wartet nie auf die Schreibsperre: Hält der Worker sie, wird
/// ein Rücksetzwunsch vermerkt (`deferred`), und der Worker bricht den laufenden Auftrag ab. Wurde beim
/// Start kein Einbetter angebunden ([`NO_EMBEDDER_OWN_DATA`]) und ist Eliezer inzwischen gespeichert, wird
/// er jetzt angebunden.
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

/// Wie [`reindex_report`], Rückgabe nur die ausstehenden Aufträge (Haken `search_hooks::rag_reindex`, Seite
/// Verbindungen).
pub async fn reindex(state: &AppState, work: WorkId) -> Result<usize, RagError> {
    reindex_report(state, work).await.map(|done| done.pending)
}

/// Antwort einer Suche als JSON, jeder Treffer mit `weak` ([`baruch_rag::is_weak`] seiner Ähnlichkeit,
/// Runde 19a; Treffer bleiben in ihrer Reihenfolge, nichts wird ausgeblendet).
pub fn answer_json(answer: &Answer) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(answer)?;
    if let Some(hits) = value.get_mut("hits").and_then(Value::as_array_mut) {
        for (hit, merged) in hits.iter_mut().zip(&answer.hits) {
            hit["weak"] = json!(baruch_rag::is_weak(merged.hit.score));
        }
    }
    Ok(value)
}

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn rpc_error(e: RagError) -> RpcError {
    match e {
        RagError::Invalid(m) => RpcError::new(INVALID_PARAMS, m),
        other => RpcError::new(REFUSED, other.message().to_owned()),
    }
}

/// Werk aus `work`; fehlt es, das Vorgabewerk. Unbekannt → -32602.
async fn work_param(state: &AppState, params: &Value) -> Result<WorkId, RpcError> {
    crate::rpc::work_param(state, params).await
}

/// Anfrage aus den RPC-Parametern (`rag.search`).
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

/// RPC `rag.search`, `rag.status`, `rag.reindex` (aus `rpc::call` vor der Wahl des Werks).
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
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
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

        // Begründung: Der einzige Platz ist belegt; eine weitere Suche (etwa von einer fremden Seite ausgelöst)
        // öffnet keine weitere pgvector-Verbindung, sondern meldet ehrlich BUSY.
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
