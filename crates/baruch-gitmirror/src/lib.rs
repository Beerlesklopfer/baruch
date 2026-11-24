// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git-Spiegel eines Werks.
//!
//! Die Datenbank ist die Quelle. Dieser Crate spiegelt nach jeder Übernahme den vollständigen
//! Typst-Stand eines Werks als Commit in einen Ordner mit eigenem Git-Repo. Er weiß nichts über
//! Datenbank, Store oder Kern: Er bekommt einen Schnappschuss aus Dateien, Autor, Committer und
//! Nachricht und macht daraus genau einen Commit, oder keinen, wenn sich nichts geändert hat.
//! Mit [`CommitRequest::allow_empty`] entsteht auch bei unverändertem Baum ein Commit: So
//! protokolliert der Server Änderungen und Löschungen von Fassungen, die nicht im Spiegel stehen.
//!
//! # Warum gix
//!
//! - **Reines Rust:** gix (gitoxide) braucht weder libgit2 noch eine C-Werkzeugkette, noch
//!   OpenSSL. Das hält den Bau schlank und reproduzierbar; `git2` würde `libgit2-sys` (C) und
//!   je nach Features `openssl-sys` mitbringen.
//! - **Genau die nötigen Bausteine:** `Repository::write_blob`, `edit_tree` (Baum aus dem
//!   Schnappschuss), `commit_as` (getrennte Autor- und Committer-Signatur, ohne Abhängigkeit
//!   von einer Git-Konfiguration des Nutzers) und `index_from_tree` (Index passend zum Baum,
//!   damit `git status` im Spiegel sauber ist).
//! - **Schlanke Features:** nur `tree-editor` und `index`, ohne Standard-Features. Damit
//!   fehlen Netzwerk-Clients (`blocking-network-client`, `async-network-client`, HTTP),
//!   Zugangsdaten, Status/Dirwalk und Diff. `gix-transport` kommt zwar als feste Abhängigkeit
//!   von `gix-protocol` mit, aber ohne jedes Client-Feature (`default = []`). Der Spiegel legt
//!   keine Remotes an und pusht nie.
//!
//! Die API ist synchron; der Server ruft sie in `tokio::task::spawn_blocking` auf.
//!
//! # Seitenzweige (Runde 22)
//!
//! [`Mirror::sync_side_refs`] hält Referenzen unter einem Präfix (der Server nimmt `refs/revisions/<key>/<n>`) auf
//! Commits mit dem Schnappschuss, in dem eine Datei einen anderen Inhalt trägt (eine nicht aktuelle Fassung eines
//! Abschnitts); Elternteil ist HEAD. Arbeitsordner, Index und HEAD bleiben unberührt, Pfade prüft dieselbe Regel wie
//! beim Commit. Unveränderte Seitenzweige werden nicht neu geschrieben. [`Mirror::side_refs`], [`Mirror::read_file`]
//! und [`Mirror::parents`] lesen nur.

mod message;

pub use message::format_message;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gix::bstr::BStr;
use gix::objs::tree::EntryKind;

/// Eine Datei des Schnappschusses. `path` ist relativ und `/`-getrennt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorFile {
    pub path: String,
    pub contents: String,
}

/// Name und E-Mail für Autor oder Committer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub email: String,
}

/// Auftrag für einen Spiegel-Commit.
#[derive(Debug, Clone)]
pub struct CommitRequest {
    /// Vollständiger Stand des Ordners (Schnappschuss).
    pub files: Vec<MirrorFile>,
    pub author: Signature,
    pub committer: Signature,
    /// Erste Zeile der Nachricht.
    pub summary: String,
    /// Rumpf, darf leer sein.
    pub body: String,
    /// Trailer, z. B. `("Persona", "@lektor")`, `("Model", "qwen3:4b")`.
    pub trailers: Vec<(String, String)>,
    /// `true`: auch bei unverändertem Baum ein Commit (Protokoll, z. B. „Fassung 2 gelöscht“).
    pub allow_empty: bool,
}

/// Ein Seitenzweig (Runde 22): Referenz `name` auf einen Commit mit dem Schnappschuss, in dem `path` den Inhalt
/// `contents` trägt (z. B. eine nicht aktuelle Fassung eines Abschnitts), Elternteil HEAD.
#[derive(Debug, Clone)]
pub struct SideRef {
    /// Voller Referenzname, z. B. `refs/revisions/12/3`.
    pub name: String,
    pub path: String,
    pub contents: String,
    pub author: Signature,
    pub committer: Signature,
    pub summary: String,
    pub body: String,
    pub trailers: Vec<(String, String)>,
}

/// Ergebnis von [`Mirror::sync_side_refs`]: neu geschriebene, unveränderte und entfernte Referenzen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SideSync {
    pub written: usize,
    pub kept: usize,
    pub removed: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    #[error("ungültiger Pfad im Spiegel: {0}")]
    InvalidPath(String),
    #[error("Ordner {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("Git: {0}")]
    Git(String),
}

fn git(err: impl std::fmt::Display) -> MirrorError {
    MirrorError::Git(err.to_string())
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> MirrorError + '_ {
    move |source| MirrorError::Io { path: path.to_path_buf(), source }
}

const BRANCH: &str = "refs/heads/main";

/// Ein Spiegelordner mit eigenem Git-Repo.
pub struct Mirror {
    dir: PathBuf,
    repo: gix::ThreadSafeRepository,
    /// Serialisiert Commits desselben Spiegels (Arbeitsordner, Index und HEAD).
    lock: Mutex<()>,
}

impl std::fmt::Debug for Mirror {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mirror").field("dir", &self.dir).finish_non_exhaustive()
    }
}

impl Mirror {
    /// Legt den Ordner an; ohne `.git` wird ein Repo angelegt (HEAD → `refs/heads/main`).
    /// Ein vorhandenes Repo wird geöffnet.
    pub fn open_or_init(dir: &Path) -> Result<Mirror, MirrorError> {
        std::fs::create_dir_all(dir).map_err(io(dir))?;
        let dir = std::fs::canonicalize(dir).map_err(io(dir))?;
        let repo = if dir.join(".git").exists() {
            gix::open(&dir).map_err(git)?
        } else {
            let repo = gix::init(&dir).map_err(git)?;
            ensure_main(&repo)?;
            repo
        };
        Ok(Mirror { dir, repo: repo.into_sync(), lock: Mutex::new(()) })
    }

    /// Arbeitsordner des Spiegels.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Hex-Id des Commits, auf den HEAD zeigt; `None`, solange es keinen Commit gibt.
    pub fn head(&self) -> Result<Option<String>, MirrorError> {
        let repo = self.repo.to_thread_local();
        let head = repo.head().map_err(git)?;
        Ok(head.id().map(|id| id.to_string()))
    }

    /// Schreibt die Dateien in den Arbeitsordner, entfernt früher verwaltete Dateien, die im
    /// Schnappschuss fehlen, baut den Baum, schreibt den Index (damit `git status` sauber ist)
    /// und committet auf HEAD. Baum unverändert → `Ok(None)`, kein leerer Commit, außer mit
    /// `allow_empty` (dann ein Commit mit demselben Baum wie sein Vorgänger). Sonst
    /// `Ok(Some(hex_commit_id))`. Ungültige Pfade → [`MirrorError::InvalidPath`], bevor
    /// irgendetwas geschrieben wird.
    pub fn commit(&self, request: &CommitRequest) -> Result<Option<String>, MirrorError> {
        validate(&request.files)?;
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo = self.repo.to_thread_local();

        // Vorheriger Stand: HEAD-Commit und die damals verwalteten Pfade.
        let parent = repo.head().map_err(git)?.id().map(|id| id.detach());
        let old_tree = match parent {
            Some(id) => Some(repo.find_commit(id).map_err(git)?.tree_id().map_err(git)?.detach()),
            None => None,
        };
        let old_paths = managed_paths(&repo, old_tree)?;

        // Neuer Baum aus dem Schnappschuss.
        let empty = gix::ObjectId::empty_tree(repo.object_hash());
        let mut editor = repo.edit_tree(empty).map_err(git)?;
        for file in &request.files {
            let blob = repo.write_blob(file.contents.as_bytes()).map_err(git)?;
            editor.upsert(file.path.as_str(), EntryKind::Blob, blob.detach()).map_err(git)?;
        }
        let tree = editor.write().map_err(git)?.detach();

        // Arbeitsordner: erst alle Zwischenordner prüfen (kein Symlink), dann Veraltetes
        // entfernen, dann schreiben. So bricht ein Symlink ab, bevor irgendetwas geändert ist.
        let new_paths: BTreeSet<&str> = request.files.iter().map(|f| f.path.as_str()).collect();
        for path in old_paths.iter().map(String::as_str).chain(new_paths.iter().copied()) {
            self.check_parents(path)?;
        }
        for stale in old_paths.iter().filter(|p| !new_paths.contains(p.as_str())) {
            self.remove_file(stale)?;
        }
        for file in &request.files {
            self.write_file(file)?;
        }

        // Index passend zum Baum, mit Dateistatus aus dem Arbeitsordner.
        let mut index = repo.index_from_tree(&tree).map_err(git)?;
        let paths: Vec<(usize, String)> =
            index.entries().iter().enumerate().map(|(i, entry)| (i, entry.path(&index).to_string())).collect();
        for (i, path) in paths {
            let full = self.dir.join(&path);
            let meta = gix::index::fs::Metadata::from_path_no_follow(&full).map_err(io(&full))?;
            let stat = gix::index::entry::Stat::from_fs(&meta).map_err(git)?;
            index.entries_mut()[i].stat = stat;
        }
        index.write(gix::index::write::Options::default()).map_err(git)?;

        if old_tree == Some(tree) && !request.allow_empty {
            return Ok(None);
        }

        let message = format!("{}\n", format_message(&request.summary, &request.body, &request.trailers));
        let now = gix::date::Time::now_local_or_utc();
        let mut author_time = gix::date::parse::TimeBuf::default();
        let mut committer_time = gix::date::parse::TimeBuf::default();
        let author = signature(&request.author, now);
        let committer = signature(&request.committer, now);
        let id = repo
            .commit_as(
                committer.to_ref(&mut committer_time),
                author.to_ref(&mut author_time),
                "HEAD",
                message,
                tree,
                parent,
            )
            .map_err(git)?;
        Ok(Some(id.to_string()))
    }

    /// Seitenzweige nachführen (Runde 22): Je Eintrag aus `refs` soll die Referenz `name` (unter `prefix`, z. B.
    /// `refs/revisions/`) auf einen Commit zeigen, dessen Baum der Schnappschuss `base` ist, nur mit `path` =
    /// `contents`; Elternteil ist HEAD. Eine vorhandene Referenz, deren Baum an `path` schon genau diesen Inhalt
    /// trägt und deren Nachricht gleich ist, bleibt (kein neuer Commit, kein Objekt-Zuwachs bei jedem Speichern; die
    /// übrigen Dateien zeigen dann den Stand bei ihrer Anlage); sonst entsteht ein neuer Commit.
    /// Referenzen unter `prefix`, die in `refs` fehlen, werden entfernt. Arbeitsordner, Index und HEAD bleiben
    /// unberührt. Pfade prüft dieselbe Regel wie [`Mirror::commit`], Namen müssen unter `prefix` liegen und gültige
    /// Git-Referenznamen sein – sonst [`MirrorError::InvalidPath`], bevor irgendetwas geschrieben wird.
    pub fn sync_side_refs(&self, prefix: &str, base: &[MirrorFile], refs: &[SideRef]) -> Result<SideSync, MirrorError> {
        use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
        if !prefix.starts_with("refs/") || !prefix.ends_with('/') || prefix.contains("..") {
            return Err(MirrorError::InvalidPath(prefix.to_string()));
        }
        validate(base)?;
        let mut names = BTreeSet::new();
        for side in refs {
            let suffix = side.name.strip_prefix(prefix).unwrap_or("");
            if suffix.is_empty()
                || !suffix
                    .split('/')
                    .all(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
                || !names.insert(side.name.as_str())
            {
                return Err(MirrorError::InvalidPath(side.name.clone()));
            }
            validate(&[MirrorFile { path: side.path.clone(), contents: String::new() }])?;
            gix::refs::FullName::try_from(side.name.as_str())
                .map_err(|_| MirrorError::InvalidPath(side.name.clone()))?;
        }
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo = self.repo.to_thread_local();
        let existing = side_refs_in(&repo, prefix)?;
        let parent = repo.head().map_err(git)?.id().map(|id| id.detach());

        let mut sync = SideSync::default();
        let mut base_tree: Option<gix::ObjectId> = None;
        let mut edits = Vec::new();
        for side in refs {
            let blob = repo.write_blob(side.contents.as_bytes()).map_err(git)?.detach();
            let message = format!("{}\n", format_message(&side.summary, &side.body, &side.trailers));
            if let Some(commit) = existing.get(&side.name) {
                let id = gix::ObjectId::from_hex(commit.as_bytes()).map_err(git)?;
                let found = repo.find_commit(id).map_err(git)?;
                let same_message = found.message_raw().map_err(git)? == message.as_str();
                let tree = found.tree().map_err(git)?;
                let entry = tree.lookup_entry_by_path(&side.path).map_err(git)?;
                // Gleicher Inhalt an derselben Stelle und gleiche Nachricht (Status, Titel): Referenz bleibt.
                if same_message && entry.is_some_and(|e| e.object_id() == blob) {
                    sync.kept += 1;
                    continue;
                }
            }
            let base_id = match base_tree {
                Some(id) => id,
                None => {
                    let empty = gix::ObjectId::empty_tree(repo.object_hash());
                    let mut editor = repo.edit_tree(empty).map_err(git)?;
                    for file in base {
                        let blob = repo.write_blob(file.contents.as_bytes()).map_err(git)?;
                        editor.upsert(file.path.as_str(), EntryKind::Blob, blob.detach()).map_err(git)?;
                    }
                    let id = editor.write().map_err(git)?.detach();
                    base_tree = Some(id);
                    id
                }
            };
            let mut editor = repo.edit_tree(base_id).map_err(git)?;
            editor.upsert(side.path.as_str(), EntryKind::Blob, blob).map_err(git)?;
            let tree = editor.write().map_err(git)?.detach();
            let now = gix::date::Time::now_local_or_utc();
            let mut author_time = gix::date::parse::TimeBuf::default();
            let mut committer_time = gix::date::parse::TimeBuf::default();
            let author = signature(&side.author, now);
            let committer = signature(&side.committer, now);
            let commit = repo
                .new_commit_as(
                    committer.to_ref(&mut committer_time),
                    author.to_ref(&mut author_time),
                    message,
                    tree,
                    parent,
                )
                .map_err(git)?;
            edits.push(RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: "Seitenzweig".into(),
                    },
                    expected: PreviousValue::Any,
                    new: gix::refs::Target::Object(commit.id),
                },
                name: side.name.as_str().try_into().map_err(git)?,
                deref: false,
            });
            sync.written += 1;
        }
        for name in existing.keys().filter(|name| !names.contains(name.as_str())) {
            edits.push(RefEdit {
                change: Change::Delete { expected: PreviousValue::Any, log: RefLog::AndReference },
                name: name.as_str().try_into().map_err(git)?,
                deref: false,
            });
            sync.removed += 1;
        }
        if !edits.is_empty() {
            repo.edit_references(edits).map_err(git)?;
        }
        Ok(sync)
    }

    /// Referenzen unter `prefix` (z. B. `refs/revisions/`): voller Name → Hex-Id des Commits.
    pub fn side_refs(&self, prefix: &str) -> Result<std::collections::BTreeMap<String, String>, MirrorError> {
        let repo = self.repo.to_thread_local();
        side_refs_in(&repo, prefix)
    }

    /// Inhalt der Datei `path` im Commit, auf den `reference` zeigt (`HEAD` oder voller Name); `None`, wenn es die
    /// Referenz oder die Datei dort nicht gibt. Nur lesend (für Tests und Anzeigen).
    pub fn read_file(&self, reference: &str, path: &str) -> Result<Option<String>, MirrorError> {
        let repo = self.repo.to_thread_local();
        let Some(id) = resolve(&repo, reference)? else { return Ok(None) };
        let tree = repo.find_commit(id).map_err(git)?.tree().map_err(git)?;
        let Some(entry) = tree.lookup_entry_by_path(path).map_err(git)? else { return Ok(None) };
        let blob = repo.find_blob(entry.object_id()).map_err(git)?;
        Ok(Some(String::from_utf8_lossy(&blob.data).into_owned()))
    }

    /// Elternteile (Hex-Ids) des Commits, auf den `reference` zeigt; `None` ohne diese Referenz.
    pub fn parents(&self, reference: &str) -> Result<Option<Vec<String>>, MirrorError> {
        let repo = self.repo.to_thread_local();
        let Some(id) = resolve(&repo, reference)? else { return Ok(None) };
        let commit = repo.find_commit(id).map_err(git)?;
        Ok(Some(commit.parent_ids().map(|p| p.to_string()).collect()))
    }

    /// Prüft jeden Zwischenordner zwischen Spiegelordner und Datei `rel` mit
    /// `symlink_metadata`: Ein Symlink (auch auf einen Ordner) → [`MirrorError::InvalidPath`],
    /// denn über ihn würde außerhalb des Spiegels geschrieben oder gelöscht. Mit `create` werden
    /// fehlende Ordner einzeln angelegt (nie über einen Symlink hinweg). Ergebnis: ob alle
    /// Zwischenordner jetzt bestehen.
    fn walk_parents(&self, rel: &str, create: bool) -> Result<bool, MirrorError> {
        let invalid = || MirrorError::InvalidPath(rel.to_string());
        let components: Vec<&str> = rel.split('/').collect();
        let Some((_, parents)) = components.split_last() else {
            return Err(invalid());
        };
        let mut dir = self.dir.clone();
        for component in parents {
            if component.is_empty() || *component == "." || *component == ".." || component.contains('\\') {
                return Err(invalid());
            }
            dir.push(component);
            match std::fs::symlink_metadata(&dir) {
                Ok(meta) if meta.file_type().is_symlink() => return Err(invalid()),
                Ok(meta) if meta.is_dir() => {}
                // Eine Datei, wo ein Ordner sein müsste: Darüber führt kein Weg nach außen; das
                // Schreiben scheitert dann mit einem Ein-/Ausgabefehler, Löschen findet nichts.
                Ok(_) if create => return Err(io(&dir)(std::io::Error::from(std::io::ErrorKind::NotADirectory))),
                Ok(_) => return Ok(false),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    if !create {
                        return Ok(false);
                    }
                    std::fs::create_dir(&dir).map_err(io(&dir))?;
                }
                Err(err) => return Err(io(&dir)(err)),
            }
        }
        Ok(true)
    }

    /// Prüfen ohne anzulegen (siehe [`Mirror::walk_parents`]).
    fn check_parents(&self, rel: &str) -> Result<(), MirrorError> {
        self.walk_parents(rel, false).map(|_| ())
    }

    fn remove_file(&self, rel: &str) -> Result<(), MirrorError> {
        if !self.walk_parents(rel, false)? {
            return Ok(()); // Ordner fehlt, also auch die Datei
        }
        let full = self.dir.join(rel);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) if !meta.is_dir() => std::fs::remove_file(&full).map_err(io(&full))?,
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(io(&full)(err)),
        }
        // Leere Elternordner bis zum Spiegelordner aufräumen.
        let mut parent = full.parent();
        while let Some(dir) = parent {
            if dir == self.dir || !dir.starts_with(&self.dir) {
                break;
            }
            if std::fs::remove_dir(dir).is_err() {
                break; // nicht leer oder nicht vorhanden
            }
            parent = dir.parent();
        }
        Ok(())
    }

    fn write_file(&self, file: &MirrorFile) -> Result<(), MirrorError> {
        self.walk_parents(&file.path, true)?;
        let full = self.dir.join(&file.path);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) if meta.file_type().is_symlink() => std::fs::remove_file(&full).map_err(io(&full))?,
            // Unveränderte Dateien nicht anfassen (Zeitstempel bleiben).
            Ok(meta) if meta.is_file() && std::fs::read(&full).map_err(io(&full))? == file.contents.as_bytes() => {
                return Ok(());
            }
            _ => {}
        }
        std::fs::write(&full, file.contents.as_bytes()).map_err(io(&full))
    }
}

/// Referenzen unter `prefix`: voller Name → Hex-Id (symbolische und unlesbare werden übergangen).
fn side_refs_in(
    repo: &gix::Repository,
    prefix: &str,
) -> Result<std::collections::BTreeMap<String, String>, MirrorError> {
    let mut out = std::collections::BTreeMap::new();
    let platform = repo.references().map_err(git)?;
    for reference in platform.prefixed(prefix).map_err(git)? {
        let Ok(reference) = reference else { continue };
        if let Some(id) = reference.try_id() {
            out.insert(reference.name().as_bstr().to_string(), id.to_string());
        }
    }
    Ok(out)
}

/// Commit einer Referenz (`HEAD` oder voller Name), `None` ohne sie.
fn resolve(repo: &gix::Repository, reference: &str) -> Result<Option<gix::ObjectId>, MirrorError> {
    if reference == "HEAD" {
        return Ok(repo.head().map_err(git)?.id().map(|id| id.detach()));
    }
    match repo.try_find_reference(reference).map_err(git)? {
        Some(mut found) => Ok(Some(found.peel_to_id().map_err(git)?.detach())),
        None => Ok(None),
    }
}

/// Alle Blob-Pfade des Baums `tree` (leer ohne Baum).
fn managed_paths(repo: &gix::Repository, tree: Option<gix::ObjectId>) -> Result<BTreeSet<String>, MirrorError> {
    let Some(tree) = tree else {
        return Ok(BTreeSet::new());
    };
    let index = repo.index_from_tree(&tree).map_err(git)?;
    Ok(index.entries().iter().map(|entry| entry.path(&index).to_string()).collect())
}

fn signature(sig: &Signature, time: gix::date::Time) -> gix::actor::Signature {
    gix::actor::Signature { name: sig.name.as_str().into(), email: sig.email.as_str().into(), time }
}

/// Sorgt dafür, dass HEAD auf `refs/heads/main` zeigt, egal was `init.defaultBranch` sagt.
fn ensure_main(repo: &gix::Repository) -> Result<(), MirrorError> {
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit};
    let current = repo.head_name().map_err(git)?;
    if current.as_ref().map(|name| name.as_bstr()) == Some(BStr::new(BRANCH)) {
        return Ok(());
    }
    repo.edit_reference(RefEdit {
        change: Change::Update {
            log: LogChange::default(),
            expected: PreviousValue::Any,
            new: gix::refs::Target::Symbolic(BRANCH.try_into().map_err(git)?),
        },
        name: "HEAD".try_into().map_err(git)?,
        deref: false,
    })
    .map_err(git)?;
    Ok(())
}

/// Prüft alle Pfade, bevor etwas geschrieben wird.
fn validate(files: &[MirrorFile]) -> Result<(), MirrorError> {
    let invalid = |path: &str| MirrorError::InvalidPath(path.to_string());
    let mut seen = BTreeSet::new();
    for file in files {
        let path = file.path.as_str();
        if path.is_empty()
            || path.starts_with('/')
            || path.ends_with('/')
            || path.contains('\\')
            || path.contains('\0')
            || Path::new(path).is_absolute()
        {
            return Err(invalid(path));
        }
        for component in path.split('/') {
            if component.is_empty()
                || component == "."
                || component == ".."
                || component.eq_ignore_ascii_case(".git")
                || (cfg!(windows) && component.contains(':'))
            {
                return Err(invalid(path));
            }
        }
        if !seen.insert(path) {
            return Err(invalid(path)); // doppelt
        }
    }
    // Datei und Ordner mit demselben Namen („a“ und „a/b“) gehen nicht zusammen.
    for path in &seen {
        let mut prefix = String::new();
        for component in path.split('/').collect::<Vec<_>>().split_last().unwrap().1 {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(component);
            if seen.contains(prefix.as_str()) {
                return Err(invalid(path));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> MirrorFile {
        MirrorFile { path: path.to_string(), contents: String::new() }
    }

    #[test]
    fn validation() {
        for bad in ["", "/abs", "a/../b", "..", ".git/config", "a/.git/x", ".GIT", "a\\b", "a//b", "./a", "a/"] {
            assert!(matches!(validate(&[file(bad)]), Err(MirrorError::InvalidPath(_))), "{bad:?}");
        }
        assert!(validate(&[file("main.typ"), file("abschnitte/000-a.typ")]).is_ok());
        assert!(validate(&[file("a"), file("a")]).is_err());
        assert!(validate(&[file("a"), file("a/b")]).is_err());
        assert!(validate(&[file(".gitignore")]).is_ok());
    }
}
