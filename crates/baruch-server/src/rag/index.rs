// SPDX-License-Identifier: AGPL-3.0-or-later
//! The local index (LanceDB) in the service: what belongs into it ([`desired_work`], [`desired_shared`]),
//! reconciliation of the changed chunks only ([`sync`]), the background task with a deduplicated
//! queue ([`Runtime`], [`notify`](super::notify)) and the search way [`LanceWay`].
//!
//! Reconciliation of one scope (a work, or "shared" = ideas without a work and the library):
//! read the stored rows; rows with the same `row_hash` stay untouched; rows whose text
//! (`content_hash`) already stands somewhere in the scope take over its vector; only the rest is
//! embedded (batch ≤ 32), removed rows are deleted. Every batch is written at once,
//! so nothing finished is lost after an abort.
//!
//! Model: the index records model and dimension in `baruch-index.json`. If the configured
//! model (after the alias) does not match that of the index or delivers another dimension, nothing is
//! mixed ([`RagError::ModelMismatch`]) – until „Index neu aufbauen“ ([`super::reindex`]).
//!
//! Errors of the embedding: at most [`MAX_ATTEMPTS`] attempts per job with a growing pause, then
//! retreat: the job (and, if the embedding model is missing, all waiting ones) goes into the list of the
//! stale ones ([`Runtime::stale`]) and is tried again only when something new is saved
//! ([`super::notify`]), a later job could actually embed again or the human rebuilds.
//! Searches do **not** fetch stale jobs back (Round 19a): they only enqueue quietly
//! ([`Runtime::refresh_shared`]) the cheap pruning of deleted works and at most every [`SHARED_REFRESH`]
//! the shared scope – they skip a stale job. The problem stays in the status
//! as long as something is stale. No endless loop, no cycle of attempts per search.
//!
//! „Index neu aufbauen“ ([`super::reindex`]) does not wait for the write lock: if the worker holds it right now,
//! a reset wish is recorded ([`Runtime::wish_reset`]); the worker aborts the running job
//! between two batches (a hanging embedding at once), resets and enqueues anew.
//!
//! Task prefixes (Round 19b, setting of the connection LanceDB): documents are embedded with the document
//! prefix, the query with the query prefix; without a prefix byte-identical as before. The document prefix
//! is recorded in `baruch-index.json`; another configured prefix mixes nothing ([`RagError::ModelMismatch`]).

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

/// Maximum number of attempts per job.
pub const MAX_ATTEMPTS: usize = 3;
/// Pause before the second and the third attempt.
pub const BACKOFF: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(1200)];
/// Kinds in the local index (also the values of `collections` for LanceDB). Since Round 13 ("abgleich") also
/// `remote`: local copies of the documents fetched from MultiGPT (`sync_document`, searchable offline).
pub const KINDS: [&str; 5] = ["section", "idea", "highlight", "library", "remote"];
/// Default of the folder (relative to the data folder).
pub const DEFAULT_DIR: &str = "lancedb";
/// Default of the embedding model.
pub const DEFAULT_MODEL: &str = "nomic-embed-text";
/// This often a running embedding or pause looks for a reset wish.
const ABORT_POLL: Duration = Duration::from_millis(50);

/// A job of the background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Job {
    /// Discard rows of deleted works.
    Prune,
    /// Ideas without a work, library, highlights.
    Shared,
    /// Sections (current revisions) and ideas of a work.
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

/// What a successful job has achieved.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// At least one batch was actually embedded (so the embedding model answers again).
    pub embedded: bool,
    /// Jobs that are due right afterwards (the shared scope after a deleted work).
    pub follow: Vec<Job>,
}

/// Reset wish from „Index neu aufbauen“ while the worker holds the write lock.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ResetWish {
    /// Whole index (another model or prefix, damaged file).
    whole: bool,
    /// Otherwise these works.
    works: BTreeSet<WorkId>,
}

#[derive(Default)]
struct Queue {
    pending: VecDeque<Job>,
    running: Option<Job>,
    worker: bool,
    /// Stale after exhausted attempts.
    stale: BTreeSet<Job>,
    /// Last error of the background task (`None` after a successful job, provided nothing
    /// is stale).
    problem: Option<RagError>,
    /// Open reset wish ([`Runtime::wish_reset`]).
    reset: Option<ResetWish>,
}

/// Runtime of the RAG for one data folder (process-wide, pattern `models::LAST_PROBE`).
pub struct Runtime {
    pub(crate) data: PathBuf,
    store: Mutex<Store>,
    embedders: Mutex<Option<Arc<dyn EmbedderFactory>>>,
    /// Why no embedder is connected (own data folder without a stored connection Eliezer).
    unbound: Mutex<Option<String>>,
    queue: Mutex<Queue>,
    index: tokio::sync::Mutex<Option<Index>>,
    /// Keeps the background task and „Neu aufbauen“ apart.
    pub(crate) writing: tokio::sync::Mutex<()>,
    /// Set as long as a reset wish is open: the running job aborts.
    abort: AtomicBool,
    /// Last reports of the remote ways from searches of this process (time, report).
    pub(crate) reports: Mutex<HashMap<Way, (String, WayReport)>>,
    /// Last refresh of the shared scope by a search.
    shared_refreshed: Mutex<Option<Instant>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A search refreshes the shared scope at most this often (library and highlights
/// do not report their changes through the mirror).
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

    /// Take over the store of the most recent `AppState` of this data folder.
    pub(crate) fn refresh(&self, store: &Store) {
        *lock(&self.store) = store.clone();
    }

    pub(crate) fn store(&self) -> Store {
        lock(&self.store).clone()
    }

    /// Embedders of this data folder; `None` = not connected (tests, helper commands).
    pub(crate) fn embedders(&self) -> Option<Arc<dyn EmbedderFactory>> {
        lock(&self.embedders).clone()
    }

    pub(crate) fn set_embedders(&self, factory: Arc<dyn EmbedderFactory>) {
        *lock(&self.embedders) = Some(factory);
        *lock(&self.unbound) = None;
    }

    /// Reason why no embedder was connected at start (`None` = none recorded).
    pub(crate) fn unbound(&self) -> Option<String> {
        lock(&self.unbound).clone()
    }

    pub(crate) fn set_unbound(&self, reason: impl Into<String>) {
        *lock(&self.unbound) = Some(reason.into());
    }

    /// Embedders or the honest substitute without a backend.
    pub(crate) fn factory(&self) -> Arc<dyn EmbedderFactory> {
        self.embedders().unwrap_or_else(|| Arc::new(super::embed::NoEmbedders))
    }

    /// Outstanding jobs (waiting and running).
    pub(crate) fn pending(&self) -> usize {
        let q = lock(&self.queue);
        q.pending.len() + usize::from(q.running.is_some())
    }

    /// Stale jobs (after exhausted attempts).
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

    /// Enqueue a job (deduplicated) and start the background task. Only with an embedder. This is the
    /// path of a real change ([`super::notify`]): stale jobs are tried once more as well.
    pub(crate) fn enqueue(self: &Arc<Self>, jobs: &[Job]) {
        self.push(jobs, true);
    }

    /// Enqueue quietly (from the search): stale jobs stay stale, even if they are in `jobs`,
    /// and nothing stale is fetched back.
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
                // Something new came: try stale jobs once more as well.
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

    /// Record a reset wish (whole index or one work), let the running job abort and start the
    /// background task if it is idle. Does not wait.
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

    /// Is a reset wish open (the running job is to abort)?
    pub(crate) fn reset_wished(&self) -> bool {
        self.abort.load(Ordering::SeqCst)
    }

    /// From the search: discard rows of deleted works (cheap: count first) and refresh the shared
    /// scope at most every [`SHARED_REFRESH`] – quietly, without fetching stale jobs back.
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

    /// Get the opened index (or the one in the configured folder).
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

/// Is a background task to start? (Records it as running right away.)
fn start_locked(q: &mut Queue) -> bool {
    let start = !q.worker && (!q.pending.is_empty() || q.reset.is_some());
    if start {
        q.worker = true;
    }
    start
}

/// Stale jobs back into the queue.
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

/// Discard the index – entirely (wished, another model or prefix, damaged file) or the named
/// works – and deliver the jobs for the rebuild. The caller holds the write lock.
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

/// Enqueue jobs after a rebuild: forget the problem and the stale jobs.
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

/// Carry out an open reset wish (under the write lock), then enqueue anew.
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
        // If another wish arrived during the reset, it stays for the next pass.
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
                // A wish arrived after the check above (set under the same lock): carry it out first.
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
        // Interrupted by the reset wish: the job goes back into the queue. The rebuild of a
        // work delivers only its job (and the shared scope) – a running job of another
        // work or the pruning would otherwise be lost and stay half reconciled, without a record.
        let mut interrupted = false;
        loop {
            let result = {
                let _held = rt.writing.lock().await;
                // Own task: a panic in the job becomes an error and does not stop the queue.
                let owned = rt.clone();
                tokio::spawn(async move { process(&owned, job).await })
                    .await
                    .unwrap_or_else(|e| Err(RagError::Failed(format!("Auftrag abgebrochen: {e}"))))
            };
            if rt.reset_wished() {
                // Rebuilding takes precedence; the job is enqueued again afterwards (a duplicate only once).
                interrupted = true;
                break;
            }
            match result {
                Ok(outcome) => {
                    let mut q = lock(&rt.queue);
                    if outcome.embedded {
                        // The embedding model answers again: try stale jobs once more.
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
                            // Without an embedding model the others fail in the same way: retreat. The pruning of
                            // deleted works needs no model and stays in the queue.
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

/// Settings of the connection LanceDB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanceConfig {
    /// Folder (absolute, lexically cleaned).
    pub dir: PathBuf,
    /// Configured model (as stored).
    pub configured: String,
    /// Name at Eliezer (alias resolved).
    pub model: String,
    /// Task prefixes (Round 19b, `query_prefix`/`document_prefix`; empty = none).
    pub prefixes: EmbedPrefixes,
}

/// Clean a path lexically (`.` removed, `..` cancels).
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

/// Folder from the setting: relative to the data folder; neither the data folder itself nor below
/// `<data>/works/` (Git mirror).
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

/// Read the settings.
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

/// Sentence: the document prefix of the index does not match the configured one.
pub const PREFIX_CHANGED: &str = "Aufgaben-Präfix geändert; Index neu aufbauen.";

/// Do model and document prefix match the index? Otherwise the honest sentence. An empty index matches
/// everything; an index without a recorded prefix (before Round 19) counts as "without prefix".
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

/// One job.
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
                // Deleted work: its rows go, its ideas are now "without a work" – the shared
                // scope pulls them in at once (not only with the next search).
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

/// Embed, but abort as soon as `abort` holds (reset wish); `None` = aborted.
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

/// Sections of a work: current revision in chunks.
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

/// Text of an idea for embedding (empty = nothing).
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

/// Ideas (not deleted) in the scope.
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

/// Shared scope: ideas without a work, titles with a note, quotations and highlights of the library, since
/// Round 13 also the local copies of fetched MultiGPT documents ([`desired_remote`]).
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
        // Highlights rejected by the human do not belong into the search (as in the wording search).
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

/// Local copies from MultiGPT (kind `remote`, without a work): per document its chunks with id
/// `remote:<sync_document.id>:<n>`, document `remote:<collection>/<document id>`, origin in `meta`
/// (`collection`, `document`, `sync` = id of the copy). Texts are data from sources, never instructions.
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

/// Bring a scope to `desired` (see the module header).
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
                // Reset wish: stop between two batches (finished work is already written).
                return Ok(outcome);
            }
            // Embedding uses the document prefix (empty = the text byte-identical); stored is the text itself.
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

/// The local search way for one search.
pub struct LanceWay {
    pub(crate) rt: Arc<Runtime>,
    pub(crate) factory: Arc<dyn EmbedderFactory>,
}

/// Address of a hit in Baruch.
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
        // Query with the query prefix (empty = byte-identical as before).
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

        // Round 19b: another document prefix mixes as little as another model.
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
