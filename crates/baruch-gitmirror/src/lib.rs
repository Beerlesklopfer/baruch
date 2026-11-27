// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git mirror of a work.
//!
//! The database is the source. After every acceptance this crate mirrors the complete Typst state of a work as a
//! commit into a folder with its own Git repo. It knows nothing about database, store or core: it receives a
//! snapshot of files, author, committer and message and turns it into exactly one commit, or none if nothing has
//! changed. With [`CommitRequest::allow_empty`] a commit is created even for an unchanged tree: this is how the
//! server logs changes and deletions of revisions that are not in the mirror.
//!
//! # Why gix
//!
//! - **Pure Rust:** gix (gitoxide) needs neither libgit2 nor a C toolchain, nor OpenSSL. That keeps the build lean
//!   and reproducible; `git2` would bring `libgit2-sys` (C) and, depending on features, `openssl-sys`.
//! - **Exactly the building blocks needed:** `Repository::write_blob`, `edit_tree` (tree from the snapshot),
//!   `commit_as` (separate author and committer signature, without depending on a Git configuration of the user)
//!   and `index_from_tree` (index matching the tree, so that `git status` in the mirror is clean).
//! - **Lean features:** only `tree-editor` and `index`, without default features. So network clients
//!   (`blocking-network-client`, `async-network-client`, HTTP), credentials, status/dirwalk and diff are missing.
//!   `gix-transport` does come along as a fixed dependency of `gix-protocol`, but without any client feature
//!   (`default = []`). The mirror creates no remotes and never pushes.
//!
//! The API is synchronous; the server calls it in `tokio::task::spawn_blocking`.
//!
//! # Side refs (Round 22)
//!
//! [`Mirror::sync_side_refs`] keeps references under a prefix (the server takes `refs/revisions/<key>/<n>`) on
//! commits with the snapshot in which one file carries a different content (a revision of a section that is not the
//! current one); the parent is HEAD. Working folder, index and HEAD stay untouched, paths are checked by the same
//! rule as for the commit. Unchanged side refs are not written again. [`Mirror::side_refs`], [`Mirror::read_file`]
//! and [`Mirror::parents`] only read.

mod message;

pub use message::format_message;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gix::bstr::BStr;
use gix::objs::tree::EntryKind;

/// A file of the snapshot. `path` is relative and `/`-separated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorFile {
    pub path: String,
    pub contents: String,
}

/// Name and e-mail for author or committer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub name: String,
    pub email: String,
}

/// Order for a mirror commit.
#[derive(Debug, Clone)]
pub struct CommitRequest {
    /// Complete state of the folder (snapshot).
    pub files: Vec<MirrorFile>,
    pub author: Signature,
    pub committer: Signature,
    /// First line of the message.
    pub summary: String,
    /// Body, may be empty.
    pub body: String,
    /// Trailers, e.g. `("Persona", "@lektor")`, `("Model", "qwen3:4b")`.
    pub trailers: Vec<(String, String)>,
    /// `true`: a commit even for an unchanged tree (log, e.g. „Fassung 2 gelöscht“).
    pub allow_empty: bool,
}

/// A side ref (Round 22): reference `name` to a commit with the snapshot in which `path` carries the content
/// `contents` (e.g. a revision of a section that is not the current one), parent HEAD.
#[derive(Debug, Clone)]
pub struct SideRef {
    /// Full reference name, e.g. `refs/revisions/12/3`.
    pub name: String,
    pub path: String,
    pub contents: String,
    pub author: Signature,
    pub committer: Signature,
    pub summary: String,
    pub body: String,
    pub trailers: Vec<(String, String)>,
}

/// Result of [`Mirror::sync_side_refs`]: newly written, unchanged and removed references.
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

/// A mirror folder with its own Git repo.
pub struct Mirror {
    dir: PathBuf,
    repo: gix::ThreadSafeRepository,
    /// Serialises commits of the same mirror (working folder, index and HEAD).
    lock: Mutex<()>,
}

impl std::fmt::Debug for Mirror {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mirror").field("dir", &self.dir).finish_non_exhaustive()
    }
}

impl Mirror {
    /// Creates the folder; without `.git` a repo is created (HEAD → `refs/heads/main`). An existing repo is opened.
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

    /// Working folder of the mirror.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Hex id of the commit HEAD points to; `None` as long as there is no commit.
    pub fn head(&self) -> Result<Option<String>, MirrorError> {
        let repo = self.repo.to_thread_local();
        let head = repo.head().map_err(git)?;
        Ok(head.id().map(|id| id.to_string()))
    }

    /// Writes the files into the working folder, removes formerly managed files that are missing in the snapshot,
    /// builds the tree, writes the index (so that `git status` is clean) and commits on HEAD. Tree unchanged →
    /// `Ok(None)`, no empty commit, except with `allow_empty` (then a commit with the same tree as its
    /// predecessor). Otherwise `Ok(Some(hex_commit_id))`. Invalid paths → [`MirrorError::InvalidPath`], before
    /// anything is written.
    pub fn commit(&self, request: &CommitRequest) -> Result<Option<String>, MirrorError> {
        validate(&request.files)?;
        let _guard = self.lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let repo = self.repo.to_thread_local();

        // Previous state: HEAD commit and the paths managed at that time.
        let parent = repo.head().map_err(git)?.id().map(|id| id.detach());
        let old_tree = match parent {
            Some(id) => Some(repo.find_commit(id).map_err(git)?.tree_id().map_err(git)?.detach()),
            None => None,
        };
        let old_paths = managed_paths(&repo, old_tree)?;

        // New tree from the snapshot.
        let empty = gix::ObjectId::empty_tree(repo.object_hash());
        let mut editor = repo.edit_tree(empty).map_err(git)?;
        for file in &request.files {
            let blob = repo.write_blob(file.contents.as_bytes()).map_err(git)?;
            editor.upsert(file.path.as_str(), EntryKind::Blob, blob.detach()).map_err(git)?;
        }
        let tree = editor.write().map_err(git)?.detach();

        // Working folder: first check all intermediate folders (no symlink), then remove what is outdated, then
        // write. This way a symlink aborts before anything is changed.
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

        // Index matching the tree, with file status from the working folder.
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

    /// Update side refs (Round 22): for each entry of `refs` the reference `name` (under `prefix`, e.g.
    /// `refs/revisions/`) shall point to a commit whose tree is the snapshot `base`, only with `path` = `contents`;
    /// the parent is HEAD. An existing reference whose tree already carries exactly this content at `path` and
    /// whose message is the same stays (no new commit, no growth of objects on every save; the remaining files then
    /// show the state at their creation); otherwise a new commit is created. References under `prefix` that are
    /// missing in `refs` are removed. Working folder, index and HEAD stay untouched. Paths are checked by the same
    /// rule as [`Mirror::commit`], names must lie under `prefix` and be valid Git reference names – otherwise
    /// [`MirrorError::InvalidPath`], before anything is written.
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
                // Same content at the same place and same message (status, title): the reference stays.
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

    /// References under `prefix` (e.g. `refs/revisions/`): full name → hex id of the commit.
    pub fn side_refs(&self, prefix: &str) -> Result<std::collections::BTreeMap<String, String>, MirrorError> {
        let repo = self.repo.to_thread_local();
        side_refs_in(&repo, prefix)
    }

    /// Content of the file `path` in the commit `reference` points to (`HEAD` or full name); `None` if the
    /// reference or the file does not exist there. Read-only (for tests and displays).
    pub fn read_file(&self, reference: &str, path: &str) -> Result<Option<String>, MirrorError> {
        let repo = self.repo.to_thread_local();
        let Some(id) = resolve(&repo, reference)? else { return Ok(None) };
        let tree = repo.find_commit(id).map_err(git)?.tree().map_err(git)?;
        let Some(entry) = tree.lookup_entry_by_path(path).map_err(git)? else { return Ok(None) };
        let blob = repo.find_blob(entry.object_id()).map_err(git)?;
        Ok(Some(String::from_utf8_lossy(&blob.data).into_owned()))
    }

    /// Parents (hex ids) of the commit `reference` points to; `None` without this reference.
    pub fn parents(&self, reference: &str) -> Result<Option<Vec<String>>, MirrorError> {
        let repo = self.repo.to_thread_local();
        let Some(id) = resolve(&repo, reference)? else { return Ok(None) };
        let commit = repo.find_commit(id).map_err(git)?;
        Ok(Some(commit.parent_ids().map(|p| p.to_string()).collect()))
    }

    /// Checks every intermediate folder between mirror folder and file `rel` with `symlink_metadata`: a symlink
    /// (also to a folder) → [`MirrorError::InvalidPath`], because through it something would be written or deleted
    /// outside the mirror. With `create` missing folders are created one by one (never across a symlink). Result:
    /// whether all intermediate folders exist now.
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
                // A file where a folder would have to be: no way leads outside through it; writing then fails with
                // an input/output error, deleting finds nothing.
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

    /// Check without creating (see [`Mirror::walk_parents`]).
    fn check_parents(&self, rel: &str) -> Result<(), MirrorError> {
        self.walk_parents(rel, false).map(|_| ())
    }

    fn remove_file(&self, rel: &str) -> Result<(), MirrorError> {
        if !self.walk_parents(rel, false)? {
            return Ok(()); // folder is missing, so the file is too
        }
        let full = self.dir.join(rel);
        match std::fs::symlink_metadata(&full) {
            Ok(meta) if !meta.is_dir() => std::fs::remove_file(&full).map_err(io(&full))?,
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(io(&full)(err)),
        }
        // Clean up empty parent folders up to the mirror folder.
        let mut parent = full.parent();
        while let Some(dir) = parent {
            if dir == self.dir || !dir.starts_with(&self.dir) {
                break;
            }
            if std::fs::remove_dir(dir).is_err() {
                break; // not empty or not present
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
            // Do not touch unchanged files (timestamps stay).
            Ok(meta) if meta.is_file() && std::fs::read(&full).map_err(io(&full))? == file.contents.as_bytes() => {
                return Ok(());
            }
            _ => {}
        }
        std::fs::write(&full, file.contents.as_bytes()).map_err(io(&full))
    }
}

/// References under `prefix`: full name → hex id (symbolic and unreadable ones are skipped).
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

/// Commit of a reference (`HEAD` or full name), `None` without it.
fn resolve(repo: &gix::Repository, reference: &str) -> Result<Option<gix::ObjectId>, MirrorError> {
    if reference == "HEAD" {
        return Ok(repo.head().map_err(git)?.id().map(|id| id.detach()));
    }
    match repo.try_find_reference(reference).map_err(git)? {
        Some(mut found) => Ok(Some(found.peel_to_id().map_err(git)?.detach())),
        None => Ok(None),
    }
}

/// All blob paths of the tree `tree` (empty without a tree).
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

/// Makes sure that HEAD points to `refs/heads/main`, whatever `init.defaultBranch` says.
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

/// Checks all paths before anything is written.
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
            return Err(invalid(path)); // duplicate
        }
    }
    // A file and a folder with the same name ("a" and "a/b") do not go together.
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
