// SPDX-License-Identifier: AGPL-3.0-or-later
//! Der lokale Index (LanceDB) im Dienst: was hineingehört ([`desired_work`], [`desired_shared`]),
//! Abgleich nur der geänderten Stücke ([`sync`]), die Hintergrund-Aufgabe mit entdoppelter
//! Warteschlange ([`Runtime`], [`notify`](super::notify)) und der Suchweg [`LanceWay`].
//!
//! Abgleich eines Bereichs (ein Werk bzw. „gemeinsam“ = Einfälle ohne Werk und Bibliothek):
//! gespeicherte Zeilen lesen; Zeilen mit gleichem `row_hash` bleiben unberührt; Zeilen, deren Text
//! (`content_hash`) schon irgendwo im Bereich steht, übernehmen dessen Vektor; nur der Rest wird
//! eingebettet (Stapel ≤ 32), entfernte Zeilen werden gelöscht. Jeder Stapel wird sofort geschrieben,
//! so geht nach einem Abbruch nichts Fertiges verloren.
//!
//! Modell: Der Index vermerkt Modell und Dimension in `baruch-index.json`. Passt das eingestellte
//! Modell (nach Alias) nicht zu dem des Index oder liefert es eine andere Dimension, wird nichts
//! gemischt ([`RagError::ModelMismatch`]) – bis „Index neu aufbauen“ ([`super::reindex`]).
//!
//! Fehler der Einbettung: höchstens [`MAX_ATTEMPTS`] Versuche je Auftrag mit wachsender Pause, dann
//! Rückzug: Der Auftrag (und bei fehlendem Einbettungsmodell alle wartenden) kommt in die Liste der
//! liegengebliebenen ([`Runtime::stale`]) und wird erst wieder versucht, wenn etwas Neues gespeichert wird
//! ([`super::notify`]), ein späterer Auftrag tatsächlich wieder einbetten konnte oder der Mensch neu aufbaut.
//! Suchen holen Liegengebliebenes **nicht** zurück (Runde 19a): Sie reihen nur still
//! ([`Runtime::refresh_shared`]) das billige Aufräumen gelöschter Werke und höchstens alle [`SHARED_REFRESH`]
//! den gemeinsamen Bereich ein – einen liegengebliebenen Auftrag überspringen sie. Das Problem bleibt im Status
//! stehen, solange etwas liegengeblieben ist. Keine Dauerschleife, kein Versuchszyklus je Suche.
//!
//! „Index neu aufbauen“ ([`super::reindex`]) wartet nicht auf die Schreibsperre: Hält der Worker sie gerade,
//! wird ein Rücksetzwunsch vermerkt ([`Runtime::wish_reset`]); der Worker bricht den laufenden Auftrag
//! zwischen zwei Stapeln (eine hängende Einbettung sofort) ab, setzt zurück und reiht neu ein.
//!
//! Aufgaben-Präfixe (Runde 19b, Einstellung der Verbindung LanceDB): Dokumente werden mit dem Dokument-Präfix
//! eingebettet, die Anfrage mit dem Anfrage-Präfix; ohne Präfix byte-gleich wie bisher. Das Dokument-Präfix
//! steht in `baruch-index.json`; ein anderes eingestelltes Präfix mischt nichts ([`RagError::ModelMismatch`]).

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use baruch_core::Work;
use baruch_lancedb::{Entry, Index, IndexMeta, Scope};
use baruch_rag::{
    BoxFuture, EmbedPrefixes, EmbedderFactory, Embeddings, Hit, MAX_EMBED_BATCH, Query, RagError, SearchWay, Way,
    WayReport, clean_snippet,
};
use baruch_store::{ConnectionKind, IdeaFilter, IdeaScope, Store, StoreError, WorkId};
use serde_json::{Value, json};

/// Höchstzahl der Versuche je Auftrag.
pub const MAX_ATTEMPTS: usize = 3;
/// Pause vor dem zweiten bzw. dritten Versuch.
pub const BACKOFF: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(1200)];
/// Arten im lokalen Index (auch die Werte von `collections` für LanceDB). Seit Runde 13 („abgleich“) dazu
/// `remote`: lokale Kopien der aus MultiGPT geholten Dokumente (`sync_document`, offline suchbar).
pub const KINDS: [&str; 5] = ["section", "idea", "highlight", "library", "remote"];
/// Vorgabe des Ordners (relativ zum Datenordner).
pub const DEFAULT_DIR: &str = "lancedb";
/// Vorgabe des Einbettungsmodells.
pub const DEFAULT_MODEL: &str = "nomic-embed-text";
/// So oft schaut eine laufende Einbettung bzw. Pause nach einem Rücksetzwunsch.
const ABORT_POLL: Duration = Duration::from_millis(50);

/// Ein Auftrag der Hintergrund-Aufgabe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Job {
    /// Zeilen gelöschter Werke verwerfen.
    Prune,
    /// Einfälle ohne Werk, Bibliothek, Hervorhebungen.
    Shared,
    /// Abschnitte (aktuelle Fassungen) und Einfälle eines Werks.
    Work(WorkId),
}

impl Job {
    fn built_key(self) -> Option<String> {
        match self {
            Job::Prune => None,
            Job::Shared => Some("shared".to_owned()),
            Job::Work(w) => Some(format!("work:{w}")),
        }
    }
}

/// Was ein gelungener Auftrag bewirkt hat.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Mindestens ein Stapel wurde tatsächlich eingebettet (das Einbettungsmodell antwortet also wieder).
    pub embedded: bool,
    /// Aufträge, die gleich danach dran sind (der gemeinsame Bereich nach einem gelöschten Werk).
    pub follow: Vec<Job>,
}

/// Rücksetzwunsch aus „Index neu aufbauen“, solange der Worker die Schreibsperre hält.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ResetWish {
    /// Ganzer Index (anderes Modell oder Präfix, beschädigte Datei).
    whole: bool,
    /// Sonst diese Werke.
    works: BTreeSet<WorkId>,
}

#[derive(Default)]
struct Queue {
    pending: VecDeque<Job>,
    running: Option<Job>,
    worker: bool,
    /// Liegengeblieben nach erschöpften Versuchen.
    stale: BTreeSet<Job>,
    /// Letzter Fehler der Hintergrund-Aufgabe (`None` nach einem gelungenen Auftrag, sofern nichts
    /// liegengeblieben ist).
    problem: Option<RagError>,
    /// Offener Rücksetzwunsch ([`Runtime::wish_reset`]).
    reset: Option<ResetWish>,
}

/// Laufzeit des RAG für einen Datenordner (prozessweit, Muster `models::LAST_PROBE`).
pub struct Runtime {
    pub(crate) data: PathBuf,
    store: Mutex<Store>,
    embedders: Mutex<Option<Arc<dyn EmbedderFactory>>>,
    /// Warum kein Einbetter angebunden ist (eigener Datenordner ohne gespeicherte Verbindung Eliezer).
    unbound: Mutex<Option<String>>,
    queue: Mutex<Queue>,
    index: tokio::sync::Mutex<Option<Index>>,
    /// Hält die Hintergrund-Aufgabe und „Neu aufbauen“ auseinander.
    pub(crate) writing: tokio::sync::Mutex<()>,
    /// Gesetzt, solange ein Rücksetzwunsch offen ist: Der laufende Auftrag bricht ab.
    abort: AtomicBool,
    /// Letzte Berichte der Remote-Wege aus Suchen dieses Prozesses (Zeit, Bericht).
    pub(crate) reports: Mutex<HashMap<Way, (String, WayReport)>>,
    /// Letzte Auffrischung des gemeinsamen Bereichs durch eine Suche.
    shared_refreshed: Mutex<Option<Instant>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Höchstens so oft frischt eine Suche den gemeinsamen Bereich auf (Bibliothek und Hervorhebungen
/// melden ihre Änderungen nicht über den Spiegel).
pub const SHARED_REFRESH: Duration = Duration::from_secs(30);

impl Runtime {
    pub(crate) fn new(data: PathBuf, store: Store) -> Runtime {
        Runtime {
            data,
            store: Mutex::new(store),
            embedders: Mutex::new(None),
            unbound: Mutex::new(None),
            queue: Mutex::new(Queue::default()),
            index: tokio::sync::Mutex::new(None),
            writing: tokio::sync::Mutex::new(()),
            abort: AtomicBool::new(false),
            reports: Mutex::new(HashMap::new()),
            shared_refreshed: Mutex::new(None),
        }
    }

    /// Store des jüngsten `AppState` dieses Datenordners übernehmen.
    pub(crate) fn refresh(&self, store: &Store) {
        *lock(&self.store) = store.clone();
    }

    pub(crate) fn store(&self) -> Store {
        lock(&self.store).clone()
    }

    /// Einbetter dieses Datenordners; `None` = nicht angebunden (Tests, Hilfsbefehle).
    pub(crate) fn embedders(&self) -> Option<Arc<dyn EmbedderFactory>> {
        lock(&self.embedders).clone()
    }

    pub(crate) fn set_embedders(&self, factory: Arc<dyn EmbedderFactory>) {
        *lock(&self.embedders) = Some(factory);
        *lock(&self.unbound) = None;
    }

    /// Grund, warum beim Start kein Einbetter angebunden wurde (`None` = keiner vermerkt).
    pub(crate) fn unbound(&self) -> Option<String> {
        lock(&self.unbound).clone()
    }

    pub(crate) fn set_unbound(&self, reason: impl Into<String>) {
        *lock(&self.unbound) = Some(reason.into());
    }

    /// Einbetter oder der ehrliche Ersatz ohne Backend.
    pub(crate) fn factory(&self) -> Arc<dyn EmbedderFactory> {
        self.embedders().unwrap_or_else(|| Arc::new(super::embed::NoEmbedders))
    }

    /// Ausstehende Aufträge (wartend und laufend).
    pub(crate) fn pending(&self) -> usize {
        let q = lock(&self.queue);
        q.pending.len() + usize::from(q.running.is_some())
    }

    /// Liegengebliebene Aufträge (nach erschöpften Versuchen).
    pub(crate) fn stale(&self) -> usize {
        lock(&self.queue).stale.len()
    }

    pub(crate) fn idle(&self) -> bool {
        let q = lock(&self.queue);
        q.pending.is_empty() && q.running.is_none() && !q.worker && q.reset.is_none()
    }

    pub(crate) fn problem(&self) -> Option<RagError> {
        lock(&self.queue).problem.clone()
    }

    pub(crate) fn clear_problem(&self) {
        let mut q = lock(&self.queue);
        q.problem = None;
        q.stale.clear();
    }

    /// Auftrag einreihen (entdoppelt) und die Hintergrund-Aufgabe starten. Nur mit Einbetter. Das ist der
    /// Weg einer echten Änderung ([`super::notify`]): Auch Liegengebliebenes wird noch einmal versucht.
    pub(crate) fn enqueue(self: &Arc<Self>, jobs: &[Job]) {
        self.push(jobs, true);
    }

    /// Still einreihen (aus der Suche): Liegengebliebene Aufträge bleiben liegen, auch wenn sie in `jobs`
    /// stehen, und nichts Liegengebliebenes wird zurückgeholt.
    pub(crate) fn enqueue_quiet(self: &Arc<Self>, jobs: &[Job]) {
        self.push(jobs, false);
    }

    fn push(self: &Arc<Self>, jobs: &[Job], reclaim: bool) {
        if self.embedders().is_none() {
            return;
        }
        let start = {
            let mut q = lock(&self.queue);
            for job in jobs {
                if !reclaim && q.stale.contains(job) {
                    continue;
                }
                q.stale.remove(job);
                if !q.pending.contains(job) {
                    q.pending.push_back(*job);
                }
            }
            if reclaim {
                // Neues kam: auch Liegengebliebenes noch einmal versuchen.
                reclaim_stale(&mut q);
            }
            start_locked(&mut q)
        };
        if start {
            self.spawn_worker();
        }
    }

    fn spawn_worker(self: &Arc<Self>) {
        let rt = self.clone();
        baruch_lancedb::runtime().spawn(async move { worker(rt).await });
    }

    /// Rücksetzwunsch vermerken (ganzer Index bzw. ein Werk), den laufenden Auftrag abbrechen lassen und die
    /// Hintergrund-Aufgabe starten, falls sie ruht. Wartet nicht.
    pub(crate) fn wish_reset(self: &Arc<Self>, whole: bool, work: WorkId) {
        let start = {
            let mut q = lock(&self.queue);
            let wish = q.reset.get_or_insert_with(ResetWish::default);
            wish.whole |= whole;
            wish.works.insert(work);
            self.abort.store(true, Ordering::SeqCst);
            start_locked(&mut q)
        };
        if start {
            self.spawn_worker();
        }
    }

    /// Ist ein Rücksetzwunsch offen (der laufende Auftrag soll abbrechen)?
    pub(crate) fn reset_wished(&self) -> bool {
        self.abort.load(Ordering::SeqCst)
    }

    /// Aus der Suche: Zeilen gelöschter Werke verwerfen (billig: erst zählen) und den gemeinsamen
    /// Bereich höchstens alle [`SHARED_REFRESH`] auffrischen – still, ohne Liegengebliebenes zurückzuholen.
    pub(crate) fn refresh_shared(self: &Arc<Self>) {
        let due = {
            let mut last = lock(&self.shared_refreshed);
            let due = last.is_none_or(|t| t.elapsed() >= SHARED_REFRESH);
            if due {
                *last = Some(Instant::now());
            }
            due
        };
        self.enqueue_quiet(if due { &[Job::Prune, Job::Shared] } else { &[Job::Prune] });
    }

    /// Den geöffneten Index (bzw. den im eingestellten Ordner) holen.
    pub(crate) async fn index(&self, dir: &Path) -> Result<Index, RagError> {
        let mut held = self.index.lock().await;
        if let Some(index) = held.as_ref().filter(|i| i.dir() == dir) {
            return Ok(index.clone());
        }
        let index = Index::open(dir).await?;
        *held = Some(index.clone());
        Ok(index)
    }
}

/// Soll eine Hintergrund-Aufgabe starten? (Vermerkt sie gleich als laufend.)
fn start_locked(q: &mut Queue) -> bool {
    let start = !q.worker && (!q.pending.is_empty() || q.reset.is_some());
    if start {
        q.worker = true;
    }
    start
}

/// Liegengebliebenes zurück in die Warteschlange.
fn reclaim_stale(q: &mut Queue) {
    let stale: Vec<Job> = std::mem::take(&mut q.stale).into_iter().collect();
    for job in stale {
        if !q.pending.contains(&job) {
            q.pending.push_back(job);
        }
    }
}

fn retryable(e: &RagError) -> bool {
    matches!(e, RagError::NoEmbedder(_) | RagError::Unreachable(_) | RagError::Timeout(_) | RagError::Failed(_))
}

/// Index verwerfen – ganz (gewünscht, anderes Modell oder Präfix, beschädigte Datei) bzw. die genannten
/// Werke – und die Aufträge für den Neuaufbau liefern. Der Aufrufer hält die Schreibsperre.
pub(crate) async fn reset_index(rt: &Runtime, whole: bool, works: &BTreeSet<WorkId>) -> Result<Vec<Job>, RagError> {
    let store = rt.store();
    let cfg = config(&store, &rt.data).await?;
    let idx = rt.index(&cfg.dir).await?;
    let whole = whole
        || match idx.meta() {
            Ok(meta) => mismatch(&meta, &cfg).is_some(),
            Err(_) => true,
        };
    if whole {
        idx.reset().await?;
        let mut jobs = vec![Job::Prune, Job::Shared];
        let all = store.works().await.map_err(store_failed)?;
        jobs.extend(all.into_iter().map(|(w, _, _)| Job::Work(w)));
        return Ok(jobs);
    }
    let mut jobs = Vec::new();
    for work in works {
        idx.delete_scope(Scope::Work(*work)).await?;
        jobs.push(Job::Work(*work));
    }
    jobs.push(Job::Shared);
    Ok(jobs)
}

/// Aufträge nach einem Neuaufbau einreihen: Problem und Liegengebliebenes vergessen.
pub(crate) fn after_reset(q_owner: &Runtime, jobs: Vec<Job>) {
    let mut q = lock(&q_owner.queue);
    q.problem = None;
    q.stale.clear();
    for job in jobs {
        if !q.pending.contains(&job) {
            q.pending.push_back(job);
        }
    }
}

/// Offenen Rücksetzwunsch ausführen (unter der Schreibsperre), danach neu einreihen.
async fn apply_reset(rt: &Arc<Runtime>) {
    let Some(wish) = lock(&rt.queue).reset.clone() else {
        rt.abort.store(false, Ordering::SeqCst);
        return;
    };
    let result = {
        let _held = rt.writing.lock().await;
        reset_index(rt, wish.whole, &wish.works).await
    };
    {
        let mut q = lock(&rt.queue);
        // Kam während des Zurücksetzens ein weiterer Wunsch dazu, bleibt er für die nächste Runde stehen.
        let newer = q.reset.take().filter(|now| *now != wish);
        q.reset = newer;
        rt.abort.store(q.reset.is_some(), Ordering::SeqCst);
        if let Err(e) = &result {
            eprintln!("RAG (LanceDB): Neuaufbau gescheitert: {e}");
            q.problem = Some(e.clone());
        }
    }
    if let Ok(jobs) = result {
        after_reset(rt, jobs);
    }
}

async fn worker(rt: Arc<Runtime>) {
    loop {
        if rt.reset_wished() {
            apply_reset(&rt).await;
            continue;
        }
        let job = {
            let mut q = lock(&rt.queue);
            if q.reset.is_some() {
                // Ein Wunsch kam nach der Prüfung oben (unter derselben Sperre gesetzt): erst ihn ausführen.
                None
            } else {
                match q.pending.pop_front() {
                    Some(job) => {
                        q.running = Some(job);
                        Some(job)
                    }
                    None => {
                        q.running = None;
                        q.worker = false;
                        return;
                    }
                }
            }
        };
        let Some(job) = job else { continue };
        let mut attempt = 1;
        // Vom Rücksetzwunsch unterbrochen: Der Auftrag kommt zurück in die Warteschlange. Der Neuaufbau eines
        // Werks liefert nur dessen Auftrag (und den gemeinsamen Bereich) – ein laufender Auftrag eines anderen
        // Werks oder das Aufräumen ginge sonst verloren und bliebe halb abgeglichen, ohne Vermerk.
        let mut interrupted = false;
        loop {
            let result = {
                let _held = rt.writing.lock().await;
                // Eigene Aufgabe: Ein Panic im Auftrag wird zum Fehler und hält die Warteschlange nicht an.
                let owned = rt.clone();
                tokio::spawn(async move { process(&owned, job).await })
                    .await
                    .unwrap_or_else(|e| Err(RagError::Failed(format!("Auftrag abgebrochen: {e}"))))
            };
            if rt.reset_wished() {
                // Neu aufbauen geht vor; der Auftrag wird danach wieder eingereiht (doppelt nur einmal).
                interrupted = true;
                break;
            }
            match result {
                Ok(outcome) => {
                    let mut q = lock(&rt.queue);
                    if outcome.embedded {
                        // Das Einbettungsmodell antwortet wieder: Liegengebliebenes noch einmal versuchen.
                        q.problem = None;
                        reclaim_stale(&mut q);
                    } else if q.stale.is_empty() {
                        q.problem = None;
                    }
                    for follow in outcome.follow {
                        q.stale.remove(&follow);
                        if !q.pending.contains(&follow) {
                            q.pending.push_front(follow);
                        }
                    }
                    break;
                }
                Err(e) if retryable(&e) && attempt < MAX_ATTEMPTS => {
                    lock(&rt.queue).problem = Some(e);
                    let pause = BACKOFF[attempt - 1];
                    let started = Instant::now();
                    while started.elapsed() < pause && !rt.reset_wished() {
                        tokio::time::sleep(ABORT_POLL).await;
                    }
                    if rt.reset_wished() {
                        interrupted = true;
                        break;
                    }
                    attempt += 1;
                }
                Err(e) => {
                    eprintln!("RAG (LanceDB): {e}");
                    let mut q = lock(&rt.queue);
                    if retryable(&e) {
                        q.stale.insert(job);
                        if matches!(e, RagError::NoEmbedder(_)) {
                            // Ohne Einbettungsmodell scheitern die übrigen genauso: Rückzug. Das Aufräumen
                            // gelöschter Werke braucht kein Modell und bleibt in der Warteschlange.
                            let waiting: Vec<Job> = q.pending.drain(..).collect();
                            for other in waiting {
                                if other == Job::Prune {
                                    q.pending.push_back(other);
                                } else {
                                    q.stale.insert(other);
                                }
                            }
                        }
                    }
                    q.problem = Some(e);
                    break;
                }
            }
        }
        let mut q = lock(&rt.queue);
        q.running = None;
        if interrupted && !q.pending.contains(&job) {
            q.pending.push_front(job);
        }
    }
}

/// Einstellungen der Verbindung LanceDB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanceConfig {
    /// Ordner (absolut, lexikalisch bereinigt).
    pub dir: PathBuf,
    /// Eingestelltes Modell (wie gespeichert).
    pub configured: String,
    /// Name bei Eliezer (Alias aufgelöst).
    pub model: String,
    /// Aufgaben-Präfixe (Runde 19b, `query_prefix`/`document_prefix`; leer = keins).
    pub prefixes: EmbedPrefixes,
}

/// Pfad lexikalisch bereinigen (`.` weg, `..` hebt auf).
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Ordner aus der Einstellung: relativ zum Datenordner; weder der Datenordner selbst noch unter
/// `<data>/works/` (Git-Spiegel).
pub fn resolve_dir(data: &Path, dir: &str) -> Result<PathBuf, RagError> {
    let dir = dir.trim();
    let dir = if dir.is_empty() { DEFAULT_DIR } else { dir };
    let base = normalize(&std::path::absolute(data).unwrap_or_else(|_| data.to_owned()));
    let path = normalize(&if Path::new(dir).is_absolute() { PathBuf::from(dir) } else { base.join(dir) });
    if path == base {
        return Err(RagError::Failed(
            "Der Ordner des Index darf nicht der Datenordner selbst sein (Einstellungen → Verbindungen → LanceDB)."
                .into(),
        ));
    }
    if path.starts_with(base.join("works")) {
        return Err(RagError::Failed(format!(
            "Der Ordner des Index {} liegt im Git-Spiegel (<Datenordner>/works); bitte einen anderen wählen \
             (Einstellungen → Verbindungen → LanceDB).",
            path.display()
        )));
    }
    Ok(path)
}

/// Einstellungen lesen.
pub async fn config(store: &Store, data: &Path) -> Result<LanceConfig, RagError> {
    let row = store
        .connection_get(ConnectionKind::Lancedb)
        .await
        .map_err(|e| RagError::Failed(format!("Verbindung lancedb nicht lesbar: {e}")))?;
    let field = |name: &str| row.config.get(name).and_then(Value::as_str).unwrap_or("").trim().to_owned();
    let configured = Some(field("embedding_model")).filter(|m| !m.is_empty()).unwrap_or_else(|| DEFAULT_MODEL.into());
    Ok(LanceConfig {
        dir: resolve_dir(data, &field("dir"))?,
        model: baruch_llm::embedding_model_name(&configured),
        configured,
        prefixes: crate::connections::config::embed_prefixes(ConnectionKind::Lancedb, &row.config),
    })
}

/// Satz: das Dokument-Präfix des Index passt nicht zum eingestellten.
pub const PREFIX_CHANGED: &str = "Aufgaben-Präfix geändert; Index neu aufbauen.";

/// Passen Modell und Dokument-Präfix zum Index? Sonst der ehrliche Satz. Ein leerer Index passt zu allem;
/// ein Index ohne vermerktes Präfix (vor Runde 19) gilt als „ohne Präfix“.
pub fn mismatch(meta: &IndexMeta, cfg: &LanceConfig) -> Option<RagError> {
    let built = meta.requested.as_deref()?;
    if built != cfg.model {
        return Some(RagError::ModelMismatch(format!(
            "Der Index ist mit dem Einbettungsmodell {built} aufgebaut, eingestellt ist {}. Baruch mischt keine \
             Vektoren verschiedener Modelle; „Index neu aufbauen“ (Einstellungen → Verbindungen) baut ihn mit dem \
             neuen Modell auf.",
            cfg.configured
        )));
    }
    let prefix = meta.document_prefix.as_deref().unwrap_or("");
    (prefix != cfg.prefixes.document).then(|| {
        RagError::ModelMismatch(format!(
            "{PREFIX_CHANGED} Der Index ist mit dem Dokument-Präfix „{prefix}“ eingebettet, eingestellt ist „{}“; \
             Baruch mischt keine Vektoren (Einstellungen → Verbindungen → LanceDB, „Index neu aufbauen“).",
            cfg.prefixes.document
        ))
    })
}

fn store_failed(e: StoreError) -> RagError {
    RagError::Failed(format!("Speicher: {e}"))
}

/// Ein Auftrag.
async fn process(rt: &Runtime, job: Job) -> Result<Outcome, RagError> {
    let store = rt.store();
    let cfg = config(&store, &rt.data).await?;
    let index = rt.index(&cfg.dir).await?;
    let meta = index.meta()?;
    if let Some(e) = mismatch(&meta, &cfg) {
        return Err(e);
    }
    let factory = rt.factory();
    let abort = || rt.reset_wished();
    match job {
        Job::Prune => {
            let works: Vec<WorkId> =
                store.works().await.map_err(store_failed)?.into_iter().map(|(w, _, _)| w).collect();
            let gone = Scope::OtherWorks(works);
            if index.count(Some(gone.clone())).await? > 0 {
                index.delete_scope(gone).await?;
            }
            Ok(Outcome::default())
        }
        Job::Work(work) => match store.load_work(work).await {
            Err(StoreError::UnknownWork(_)) => {
                // Gelöschtes Werk: Seine Zeilen gehen, seine Einfälle sind jetzt „ohne Werk“ – der gemeinsame
                // Bereich zieht sie sofort nach (nicht erst mit der nächsten Suche).
                index.delete_scope(Scope::Work(work)).await?;
                Ok(Outcome { embedded: false, follow: vec![Job::Shared] })
            }
            Err(e) => Err(store_failed(e)),
            Ok(loaded) => {
                let mut desired = desired_sections(work, &loaded);
                desired.extend(desired_ideas(&store, IdeaScope::Work(work)).await?);
                sync(&index, job, Scope::Work(work), desired, &cfg, factory.as_ref(), &abort).await
            }
        },
        Job::Shared => {
            let desired = desired_shared(&store).await?;
            sync(&index, job, Scope::Shared, desired, &cfg, factory.as_ref(), &abort).await
        }
    }
}

/// Einbetten, aber abbrechen, sobald `abort` gilt (Rücksetzwunsch); `None` = abgebrochen.
async fn embed_or_abort(
    embedder: &dyn baruch_rag::Embedder,
    texts: &[String],
    abort: &(dyn Fn() -> bool + Sync),
) -> Option<Result<Embeddings, RagError>> {
    let watch = async {
        while !abort() {
            tokio::time::sleep(ABORT_POLL).await;
        }
    };
    tokio::select! {
        result = embedder.embed(texts) => Some(result),
        () = watch => None,
    }
}

/// Abschnitte eines Werks: aktuelle Fassung in Stücken.
pub fn desired_sections(work: WorkId, loaded: &Work) -> Vec<Entry> {
    let mut out = Vec::new();
    for node in &loaded.nodes {
        let Some(current) = node.current_index().and_then(|i| node.revisions.get(i)) else { continue };
        let title = if node.number.trim().is_empty() {
            node.title.clone()
        } else {
            format!("{} {}", node.number.trim(), node.title)
        };
        for chunk in baruch_lancedb::chunk::chunks(&current.body) {
            out.push(Entry {
                id: format!("section:{}:{}", node.key, chunk.number),
                work: Some(work),
                kind: "section".into(),
                document: format!("work:{work}"),
                section: Some(format!("key:{}", node.key)),
                title: title.clone(),
                text: chunk.text,
                meta: json!({ "work": work, "key": node.key, "rev": current.number, "chunk": chunk.number }),
            });
        }
    }
    out
}

/// Text eines Einfalls zum Einbetten (leer = nichts).
fn idea_text(idea: &baruch_store::Idea) -> String {
    let mut parts = Vec::new();
    if !idea.words.is_empty() {
        parts.push(format!("Stichworte: {}", idea.words.join(", ")));
    }
    for (label, value) in
        [("", &idea.note), ("Zitat: ", &idea.quote), ("Quelle: ", &idea.source), ("Bibel: ", &idea.bible)]
    {
        if !value.trim().is_empty() {
            parts.push(format!("{label}{}", value.trim()));
        }
    }
    parts.join("\n")
}

/// Einfälle (nicht gelöschte) im Bereich.
pub async fn desired_ideas(store: &Store, scope: IdeaScope) -> Result<Vec<Entry>, RagError> {
    let mut out = Vec::new();
    let mut offset = 0u32;
    loop {
        let filter = IdeaFilter { scope, limit: 500, offset, ..IdeaFilter::default() };
        let page = store.idea_list(&filter).await.map_err(store_failed)?;
        let count = page.len();
        for idea in page {
            let text = idea_text(&idea);
            if text.is_empty() {
                continue;
            }
            let title = if idea.words.is_empty() { format!("Einfall {}", idea.id) } else { idea.words.join(", ") };
            out.push(Entry {
                id: format!("idea:{}", idea.id),
                work: idea.work,
                kind: "idea".into(),
                document: format!("idea:{}", idea.id),
                section: None,
                title,
                text,
                meta: json!({ "idea": idea.id, "work": idea.work, "key": idea.key, "status": idea.status.as_str() }),
            });
        }
        if count < 500 {
            break;
        }
        offset += 500;
    }
    Ok(out)
}

/// Gemeinsamer Bereich: Einfälle ohne Werk, Titel mit Notiz, Zitate und Hervorhebungen der Bibliothek, seit
/// Runde 13 auch die lokalen Kopien geholter MultiGPT-Dokumente ([`desired_remote`]).
pub async fn desired_shared(store: &Store) -> Result<Vec<Entry>, RagError> {
    let mut out = desired_ideas(store, IdeaScope::NoWork).await?;
    let pool = store.pool();
    let titles = baruch_library::list_titles(pool).await.map_err(|e| RagError::Failed(format!("Bibliothek: {e}")))?;
    let mut names = HashMap::new();
    for entry in &titles {
        let t = &entry.title;
        names.insert(t.id, t.title.clone());
        let mut line = t.title.trim().to_owned();
        if !t.authors.trim().is_empty() {
            line.push_str(&format!(" – {}", t.authors.trim()));
        }
        if let Some(year) = t.year {
            line.push_str(&format!(" ({year})"));
        }
        let mut text = line;
        for extra in [&t.publisher, &t.place, &t.note] {
            if !extra.trim().is_empty() {
                text.push('\n');
                text.push_str(extra.trim());
            }
        }
        out.push(Entry {
            id: format!("title:{}", t.id),
            work: None,
            kind: "library".into(),
            document: format!("title:{}", t.id),
            section: None,
            title: t.title.clone(),
            text,
            meta: json!({ "title": t.id, "key": t.key }),
        });
        for quote in &entry.quotes {
            if quote.text.trim().is_empty() {
                continue;
            }
            out.push(Entry {
                id: format!("quote:{}", quote.id),
                work: None,
                kind: "library".into(),
                document: format!("title:{}", t.id),
                section: Some(format!("quote:{}", quote.id)),
                title: t.title.clone(),
                text: quote.text.trim().to_owned(),
                meta: json!({ "title": t.id, "quote": quote.id, "page": quote.page }),
            });
        }
    }
    let highlights = baruch_library::list_highlights(pool, None)
        .await
        .map_err(|e| RagError::Failed(format!("Hervorhebungen: {e}")))?;
    for h in highlights {
        // Vom Menschen abgelehnte Hervorhebungen gehören nicht in die Suche (wie bei der Wortlaut-Suche).
        if h.status == baruch_library::HighlightStatus::Rejected {
            continue;
        }
        let mut text = h.text.trim().to_owned();
        if !h.note.trim().is_empty() {
            text.push_str(&format!("\nNotiz: {}", h.note.trim()));
        }
        if text.is_empty() {
            continue;
        }
        out.push(Entry {
            id: format!("highlight:{}", h.id),
            work: None,
            kind: "highlight".into(),
            document: format!("title:{}", h.title_id),
            section: Some(format!("highlight:{}", h.id)),
            title: names.get(&h.title_id).cloned().unwrap_or_else(|| format!("Quelle {}", h.title_id)),
            text,
            meta: json!({ "title": h.title_id, "highlight": h.id, "quote": h.quote_id, "kind": h.kind.as_str() }),
        });
    }
    out.extend(desired_remote(store).await?);
    Ok(out)
}

/// Lokale Kopien aus MultiGPT (Art `remote`, ohne Werk): je Dokument seine Stücke mit id
/// `remote:<sync_document.id>:<n>`, Dokument `remote:<Sammlung>/<Dokument-ID>`, Herkunft in `meta`
/// (`collection`, `document`, `sync` = id der Kopie). Texte sind Daten aus Quellen, nie Anweisungen.
pub async fn desired_remote(store: &Store) -> Result<Vec<Entry>, RagError> {
    let documents = store.sync_documents().await.map_err(store_failed)?;
    let mut out = Vec::new();
    for doc in documents {
        let title = if doc.title.trim().is_empty() { doc.document_id.clone() } else { doc.title.clone() };
        for chunk in baruch_lancedb::chunk::chunks(&doc.text) {
            out.push(Entry {
                id: format!("remote:{}:{}", doc.id, chunk.number),
                work: None,
                kind: "remote".into(),
                document: format!("remote:{}/{}", doc.collection, doc.document_id),
                section: None,
                title: title.clone(),
                text: chunk.text,
                meta: json!({ "collection": doc.collection, "document": doc.document_id, "sync": doc.id }),
            });
        }
    }
    Ok(out)
}

/// Bereich auf `desired` bringen (siehe Modulkopf).
pub async fn sync(
    index: &Index,
    job: Job,
    scope: Scope,
    desired: Vec<Entry>,
    cfg: &LanceConfig,
    factory: &dyn EmbedderFactory,
    abort: &(dyn Fn() -> bool + Sync),
) -> Result<Outcome, RagError> {
    let mut outcome = Outcome::default();
    let stored = index.stored(scope.clone()).await?;
    let mut meta = index.meta()?;
    let by_id: HashMap<&str, &baruch_lancedb::Stored> = stored.iter().map(|s| (s.id.as_str(), s)).collect();
    let by_content: HashMap<&str, &Vec<f32>> = stored.iter().map(|s| (s.content_hash.as_str(), &s.vector)).collect();
    let wanted: HashSet<&str> = desired.iter().map(|e| e.id.as_str()).collect();
    let mut remove: Vec<String> =
        stored.iter().filter(|s| !wanted.contains(s.id.as_str())).map(|s| s.id.clone()).collect();
    let mut reused: Vec<(Entry, Vec<f32>)> = Vec::new();
    let mut to_embed: Vec<Entry> = Vec::new();
    for entry in desired {
        if by_id.get(entry.id.as_str()).is_some_and(|s| s.row_hash == entry.row_hash()) {
            continue;
        }
        match by_content.get(entry.content_hash().as_str()) {
            Some(vector) => reused.push((entry, (*vector).clone())),
            None => to_embed.push(entry),
        }
    }
    let dimension = meta.dimension.or_else(|| reused.first().map(|(_, v)| v.len()));
    if !remove.is_empty() || !reused.is_empty() {
        match dimension {
            Some(dimension) => {
                remove.extend(reused.iter().map(|(e, _)| e.id.clone()));
                index.replace(scope.clone(), remove, reused, dimension).await?;
            }
            None => index.replace(scope.clone(), remove, Vec::new(), 1).await?,
        }
    }
    if !to_embed.is_empty() {
        let embedder = factory.for_model(&cfg.model);
        for batch in to_embed.chunks(MAX_EMBED_BATCH) {
            if abort() {
                // Rücksetzwunsch: zwischen zwei Stapeln aufhören (Fertiges ist schon geschrieben).
                return Ok(outcome);
            }
            // Eingebettet wird mit dem Dokument-Präfix (leer = der Text byte-gleich); gespeichert der Text selbst.
            let texts: Vec<String> = batch.iter().map(|e| cfg.prefixes.document_text(&e.text)).collect();
            let Some(answer) = embed_or_abort(embedder.as_ref(), &texts, abort).await else { return Ok(outcome) };
            let answer = answer?;
            outcome.embedded = true;
            if answer.vectors.len() != batch.len() {
                return Err(RagError::Failed(format!(
                    "Einbettung lieferte {} statt {} Vektoren.",
                    answer.vectors.len(),
                    batch.len()
                )));
            }
            match meta.dimension {
                None => {
                    meta.requested = Some(embedder.model().to_owned());
                    meta.model = Some(answer.model.clone());
                    meta.dimension = Some(answer.dimension);
                    meta.document_prefix = Some(cfg.prefixes.document.clone());
                    meta.layout = baruch_lancedb::LAYOUT;
                    index.set_meta(&meta)?;
                }
                Some(d) if d != answer.dimension => {
                    return Err(RagError::ModelMismatch(format!(
                        "Das Einbettungsmodell liefert {} Dimensionen, der Index hat {d}; nichts gemischt. „Index neu \
                         aufbauen“ hilft.",
                        answer.dimension
                    )));
                }
                Some(_) => {}
            }
            let rows: Vec<(Entry, Vec<f32>)> = batch.iter().cloned().zip(answer.vectors).collect();
            let ids = rows.iter().map(|(e, _)| e.id.clone()).collect();
            index.replace(scope.clone(), ids, rows, answer.dimension).await?;
        }
    }
    if let Some(key) = job.built_key() {
        let mut meta = index.meta()?;
        meta.built.insert(key, crate::models::iso_now());
        if meta.layout == 0 {
            meta.layout = baruch_lancedb::LAYOUT;
        }
        index.set_meta(&meta)?;
    }
    Ok(outcome)
}

/// Der lokale Suchweg für eine Suche.
pub struct LanceWay {
    pub(crate) rt: Arc<Runtime>,
    pub(crate) factory: Arc<dyn EmbedderFactory>,
}

/// Adresse eines Treffers in Baruch.
fn url_of(entry: &Entry, work: WorkId) -> Option<String> {
    let number = |name: &str| entry.meta.get(name).and_then(Value::as_i64);
    match entry.kind.as_str() {
        "section" => Some(format!("/?work={}&key={}", entry.work.unwrap_or(work), number("key")?)),
        "idea" => Some(format!("/einfaelle/{}", number("idea")?)),
        "library" => Some(match number("quote") {
            Some(quote) => format!("/quelle?work={work}&title={}&quote={quote}", number("title")?),
            None => format!("/quelle?work={work}&title={}", number("title")?),
        }),
        "highlight" => {
            Some(format!("/quelle?work={work}&title={}&highlight={}", number("title")?, number("highlight")?))
        }
        "remote" => {
            let text = |name: &str| entry.meta.get(name).and_then(Value::as_str);
            Some(format!(
                "/quelle/multigpt?collection={}&document={}",
                crate::view::query_escape(text("collection")?),
                crate::view::query_escape(text("document")?)
            ))
        }
        _ => None,
    }
}

impl LanceWay {
    async fn run(&self, query: &Query) -> Result<Vec<Hit>, RagError> {
        let store = self.rt.store();
        let cfg = config(&store, &self.rt.data).await?;
        let index = self.rt.index(&cfg.dir).await?;
        let meta = index.meta()?;
        if let Some(e) = mismatch(&meta, &cfg) {
            return Err(e);
        }
        let embedder = self.factory.for_model(&cfg.model);
        // Anfrage mit dem Anfrage-Präfix (leer = byte-gleich wie bisher).
        let answer = embedder.embed(&[cfg.prefixes.query_text(&query.text)]).await?;
        if let Some(d) = meta.dimension.filter(|d| *d != answer.dimension) {
            return Err(RagError::ModelMismatch(format!(
                "Das Einbettungsmodell {} liefert {} Dimensionen, der Index hat {d}; „Index neu aufbauen“ hilft.",
                cfg.configured, answer.dimension
            )));
        }
        let vector = answer.vectors.into_iter().next().unwrap_or_default();
        let kinds: Vec<String> = query.collections.iter().filter(|c| KINDS.contains(&c.as_str())).cloned().collect();
        let found = index.search(vector, Scope::Visible(query.work), kinds, query.limit.saturating_mul(4)).await?;
        let mut seen = HashSet::new();
        let mut hits = Vec::new();
        for f in found {
            let score = f.score();
            let entry = f.entry;
            let hit = Hit {
                way: Way::Lancedb,
                collection: entry.kind.clone(),
                document: entry.document.clone(),
                section: entry.section.clone(),
                title: entry.title.clone(),
                snippet: clean_snippet(&entry.text),
                score,
                url: url_of(&entry, query.work),
                meta: entry.meta.clone(),
            };
            if seen.insert(hit.identity()) {
                hits.push(hit);
            }
            if hits.len() >= query.limit {
                break;
            }
        }
        Ok(hits)
    }
}

impl SearchWay for LanceWay {
    fn way(&self) -> Way {
        Way::Lancedb
    }

    fn search<'a>(&'a self, query: &'a Query) -> BoxFuture<'a, Result<Vec<Hit>, RagError>> {
        Box::pin(self.run(query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_directory_is_relative_to_the_data_and_never_in_the_mirror() {
        let data = Path::new("/srv/baruch-daten");
        assert_eq!(resolve_dir(data, "lancedb").unwrap(), PathBuf::from("/srv/baruch-daten/lancedb"), "relativ");
        assert_eq!(resolve_dir(data, "").unwrap(), PathBuf::from("/srv/baruch-daten/lancedb"), "leer: Vorgabe");
        assert_eq!(resolve_dir(data, "/var/idx").unwrap(), PathBuf::from("/var/idx"), "absolut bleibt");
        assert!(resolve_dir(data, "works/rag").is_err(), "nicht im Git-Spiegel");
        assert!(resolve_dir(data, "x/../works/rag").is_err(), "auch nicht über ..");
        assert!(resolve_dir(data, ".").is_err(), "nicht der Datenordner selbst");
        assert!(resolve_dir(data, "workshop").is_ok(), "ähnlicher Name ist erlaubt");
    }

    #[test]
    fn a_different_configured_model_is_a_mismatch_but_the_alias_is_the_same_model() {
        let meta = IndexMeta {
            requested: Some("text-embedding-nomic-embed-text-v1.5".into()),
            dimension: Some(768),
            ..IndexMeta::default()
        };
        let cfg = |configured: &str| LanceConfig {
            dir: PathBuf::from("/x"),
            configured: configured.into(),
            model: baruch_llm::embedding_model_name(configured),
            prefixes: EmbedPrefixes::default(),
        };
        assert!(mismatch(&meta, &cfg("nomic-embed-text")).is_none(), "Alias = gleiches Modell");
        let e = mismatch(&meta, &cfg("bge-m3")).expect("anderes Modell");
        assert_eq!(e.code(), "model_mismatch", "Code");
        assert!(e.message().contains("bge-m3") && e.message().contains("neu aufbauen"), "Satz: {e}");
        assert!(mismatch(&IndexMeta::default(), &cfg("bge-m3")).is_none(), "leerer Index passt zu jedem Modell");

        // Runde 19b: ein anderes Dokument-Präfix mischt so wenig wie ein anderes Modell.
        let mut prefixed = cfg("nomic-embed-text");
        prefixed.prefixes.document = "search_document: ".into();
        let e = mismatch(&meta, &prefixed).expect("Index ohne Präfix, eingestellt mit Präfix");
        assert_eq!(e.code(), "model_mismatch", "Präfixwechsel = ModelMismatch");
        assert!(e.message().starts_with(PREFIX_CHANGED), "Satz des Vertrags vorn: {e}");
        let built = IndexMeta { document_prefix: Some("search_document: ".into()), ..meta.clone() };
        assert!(mismatch(&built, &prefixed).is_none(), "gleiches Präfix passt");
        let empty = IndexMeta { document_prefix: Some(String::new()), ..meta.clone() };
        assert!(mismatch(&empty, &cfg("nomic-embed-text")).is_none(), "leeres Präfix = keins (ältere Indexe: None)");
    }

    #[test]
    fn hits_link_into_baruch() {
        let entry = |kind: &str, meta: Value| Entry {
            id: "x".into(),
            work: Some(3),
            kind: kind.into(),
            document: "d".into(),
            section: None,
            title: "t".into(),
            text: "x".into(),
            meta,
        };
        assert_eq!(url_of(&entry("section", json!({ "key": 17 })), 9).unwrap(), "/?work=3&key=17", "Abschnitt");
        assert_eq!(url_of(&entry("idea", json!({ "idea": 5 })), 9).unwrap(), "/einfaelle/5", "Einfall");
        assert_eq!(
            url_of(&entry("highlight", json!({ "title": 2, "highlight": 8 })), 9).unwrap(),
            "/quelle?work=9&title=2&highlight=8",
            "Hervorhebung"
        );
        assert_eq!(url_of(&entry("library", json!({ "title": 2 })), 9).unwrap(), "/quelle?work=9&title=2", "Titel");
        assert_eq!(url_of(&entry("section", json!({})), 9), None, "ohne Schlüssel kein Link");
        assert_eq!(
            url_of(&entry("remote", json!({ "collection": "Bibeln & Kommentare", "document": "d/7" })), 9).unwrap(),
            "/quelle/multigpt?collection=Bibeln%20%26%20Kommentare&document=d%2F7",
            "geholtes Dokument: Dokumentansicht von MultiGPT, Werte url-kodiert"
        );
        assert_eq!(url_of(&entry("remote", json!({ "collection": "c" })), 9), None, "ohne Dokument kein Link");
    }
}
