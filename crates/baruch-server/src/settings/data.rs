// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings page „Daten“: data folder, size (recursive, symbolic links not followed),
//! per work the Git mirror with its last commit, version of the program (display only, since Round 24 with a link
//! to `/ueber`) and „Sicherung jetzt“ (only the human: form `POST /einstellungen/daten/sicherung` with `confirm=1`
//! or RPC `settings.backup {confirm: true}`, no MCP).
//!
//! Instance (Round 26): identifier (`store_meta.instance_uuid`, display only) and name of this Baruch instance,
//! which travels along in the exchange format as the origin. Form `POST /einstellungen/daten/instanz` with `name`
//! (trimmed, at most 80 characters, no control characters, empty = reset; [`instance_act`]), RPC `instance.get` and
//! `instance.set_name` ([`crate::instance`]), no MCP.
//!
//! EPUBCheck (Round 20c): block with the path to an epubcheck.jar (`store_meta.epubcheck_jar`), the tool found
//! (`epubcheck` in the PATH takes precedence) and whether `java` is in the PATH. Form
//! `POST /einstellungen/daten/epubcheck` (field `jar`, empty = remove) and RPC
//! `epub.checker_get`/`epub.checker_set` in [`crate::epub_check`], no MCP.
//!
//! Backup ([`backup`]): under [`AppState::write_lock`] (no commit in between) the folder
//! `<parent folder>/<name>.sicherung-<JJJJ-MM-TT-HHMMSS>` is created next to the data folder (local time from
//! SQLite `strftime(…, 'localtime')`). If the folder already exists (second backup in the same second), that
//! is an error and nothing is overwritten. `baruch.sqlite` comes through `VACUUM INTO` (a state consistent
//! in itself, also while the server runs), everything else is copied recursively except
//! `baruch.sqlite`, `-wal` and `-shm`; symbolic links are skipped and reported.
//!
//! Limits, so that repeated backups do not fill the file system (it also carries the running
//! database and the mirrors): if [`MAX_BACKUPS`] backups (`<name>.sicherung-*`) already lie next to the data
//! folder, „Sicherung jetzt“ refuses until old ones are removed (Baruch deletes no
//! backup itself). Before that the free space is checked (Unix: `statvfs`): after the backup (as large
//! as the data folder) [`RESERVE_BYTES`] must still stay free, otherwise nothing is created.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Form, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use baruch_store::WorkId;
use minijinja::{Value, context};
use serde::Deserialize;
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::rpc::{INTERNAL, RpcError};

/// Maximum number of backups next to the data folder; after that „Sicherung jetzt“ refuses.
pub const MAX_BACKUPS: usize = 10;

/// This much must still stay free on the file system after a backup (512 MiB).
pub const RESERVE_BYTES: u64 = 512 * 1024 * 1024;

/// Files of the database that are not copied (`baruch.sqlite` comes through `VACUUM INTO`).
const DB_FILES: [&str; 3] = ["baruch.sqlite", "baruch.sqlite-wal", "baruch.sqlite-shm"];

/// One-time note „Sicherung angelegt“ per data folder (Round 36): the form „Sicherung jetzt“ deposits the folder
/// name of a backup that was really created, the page `?ok=sicherung` fetches it exactly once. A
/// mere link with `?ok=sicherung` (or a reload) therefore no longer shows a success message. The note lives only
/// in the process (no field in `AppState`, nothing in the database); after a restart it is gone – the backup
/// itself still stands next to the data folder. Per data folder, because several instances can run in one process
/// (tests).
static BACKUP_NOTES: std::sync::Mutex<BTreeMap<PathBuf, String>> = std::sync::Mutex::new(BTreeMap::new());

fn backup_notes() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, String>> {
    BACKUP_NOTES.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Notes the backup `made` just created for the next display of the page.
fn note_backup(data: &Path, made: &Backup) {
    let name = made.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    backup_notes().insert(data.to_owned(), name);
}

/// Fetches the note (afterwards it is gone): folder name of the backup just created.
fn take_backup_note(data: &Path) -> Option<String> {
    backup_notes().remove(data)
}

/// Too open permissions of a data folder (Round 36): `Some(permissions)` (the lower nine bits) if group or others
/// may do anything; `None` if only the owner has access, the folder is not readable or the system knows no
/// Unix permissions. Read only – Baruch never changes the permissions of an existing folder (the human can have
/// reasons: group access for the backup, a mounted drive); new data folders get 0700.
#[cfg(unix)]
pub fn open_permissions(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path).ok()?.permissions().mode() & 0o777;
    (mode & 0o077 != 0).then_some(mode)
}

/// Without Unix permissions there is nothing to report.
#[cfg(not(unix))]
pub fn open_permissions(_path: &Path) -> Option<u32> {
    None
}

/// Note for the page „Daten“ on [`open_permissions`]; `None` if there is nothing to report.
pub fn permissions_warning(path: &Path) -> Option<String> {
    let mode = open_permissions(path)?;
    Some(format!(
        "Der Datenordner ist auch für andere Benutzer dieses Rechners zugänglich (Rechte {mode:04o}). Baruch ändert \
         die Rechte eines vorhandenen Ordners nicht; „chmod 700“ auf den Ordner lässt nur noch den Besitzer hinein."
    ))
}

/// Size of a folder in bytes, recursive; symbolic links do not count and are not
/// followed. Unreadable entries are skipped.
pub(crate) fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else { return 0 };
    if meta.file_type().is_symlink() {
        return 0;
    }
    if !meta.is_dir() {
        return meta.len();
    }
    std::fs::read_dir(path)
        .map(|entries| entries.filter_map(Result::ok).map(|e| dir_size(&e.path())).sum())
        .unwrap_or(0)
}

/// `1234567` → „1,2 MiB“.
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} Bytes");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit]).replace('.', ",")
}

/// A work with its mirror.
struct MirrorRow {
    work: WorkId,
    title: String,
    slug: String,
    dir: String,
    head: Option<String>,
    error: Option<String>,
}

async fn mirrors(state: &AppState) -> Result<Vec<MirrorRow>, String> {
    let works = state.store.work_list().await.map_err(|e| e.to_string())?;
    let mut rows = Vec::with_capacity(works.len());
    for w in works {
        let dir = crate::mirror_dir(&state.data, &w.slug).map_or_else(|e| e, |d| d.display().to_string());
        let (head, error) = match state.mirror(w.id).await {
            Ok(mirror) => match tokio::task::spawn_blocking(move || mirror.head()).await {
                Ok(Ok(head)) => (head, None),
                Ok(Err(e)) => (None, Some(e.to_string())),
                Err(e) => (None, Some(format!("Git: {e}"))),
            },
            Err(e) => (None, Some(e)),
        };
        rows.push(MirrorRow { work: w.id, title: w.title, slug: w.slug, dir, head, error });
    }
    Ok(rows)
}

async fn size_of(state: &AppState) -> u64 {
    let data = state.data.clone();
    tokio::task::spawn_blocking(move || dir_size(&data)).await.unwrap_or(0)
}

pub(super) async fn render(
    state: &AppState,
    work: Option<WorkId>,
    query: &BTreeMap<String, String>,
) -> Result<String, String> {
    let info = super::request();
    let size = size_of(state).await;
    let rows: Vec<Value> = mirrors(state)
        .await?
        .into_iter()
        .map(|m| {
            context! {
                work => m.work, title => m.title, slug => m.slug, dir => m.dir,
                head => m.head.as_deref().map(|h| h.chars().take(12).collect::<String>()), error => m.error,
            }
        })
        .collect();
    // Round 23a: fixed message code `?ok=sicherung`; a `ziel` sent along in the address is never displayed.
    // Round 36: the message appears only if the form has really just created a backup – exactly
    // once ([`take_backup_note`]). A link with `?ok=sicherung` alone shows nothing.
    let saved =
        if query.get("ok").map(String::as_str) == Some("sicherung") { take_backup_note(&state.data) } else { None };
    // Round 36: only report too open permissions of the data folder, never change them.
    let data = state.data.clone();
    let permissions = tokio::task::spawn_blocking(move || permissions_warning(&data)).await.ok().flatten();
    let instance = state.store.instance().await.map_err(|e| e.to_string())?;
    let instance_saved = query.get("ok").map(String::as_str) == Some("instanz");
    // Round 20c: block „EPUBCheck“ (path to the epubcheck.jar, form in crate::epub_check).
    let epubcheck =
        crate::epub_check::settings_context(state, query.get("ok").map(String::as_str) == Some("epubcheck")).await;
    super::render_template(
        "settings/daten.html",
        context! {
            work => work, error => info.error, saved => saved, permissions => permissions,
            dir => state.data.display().to_string(), size => human_size(size), size_bytes => size,
            mirrors => rows, version => crate::about::VERSION,
            instance_uuid => instance.uuid.clone(), instance_name => instance.name.clone(),
            instance_label => instance.label(), instance_saved => instance_saved,
            max_instance_name => baruch_store::MAX_INSTANCE_NAME_CHARS, epubcheck => epubcheck,
        },
    )
}

/// `settings.get {page: "daten"}`.
pub(super) async fn values(state: &AppState) -> Result<Json, RpcError> {
    let size = size_of(state).await;
    let mirrors: Vec<Json> = mirrors(state)
        .await
        .map_err(|e| RpcError::new(INTERNAL, e))?
        .into_iter()
        .map(|m| json!({ "work": m.work, "slug": m.slug, "dir": m.dir, "head": m.head }))
        .collect();
    let instance = state.store.instance().await?;
    Ok(json!({
        "dir": state.data.display().to_string(), "size_bytes": size, "mirrors": mirrors,
        "version": crate::about::VERSION, "instance": { "uuid": instance.uuid, "name": instance.name },
    }))
}

/// Result of a backup.
#[derive(Debug, Clone)]
pub struct Backup {
    /// Folder of the backup (next to the data folder).
    pub dir: PathBuf,
    pub size_bytes: u64,
    /// Skipped symbolic links (relative to the data folder, with `/`).
    pub skipped: Vec<String>,
}

/// Copies `from` to `to` (which does not exist yet), recursively; on the top level without [`DB_FILES`].
/// Symbolic links are not copied but reported in `skipped`.
fn copy_tree(from: &Path, to: &Path, rel: &str, top: bool, skipped: &mut Vec<String>) -> Result<(), String> {
    let entries = std::fs::read_dir(from).map_err(|e| format!("{} lässt sich nicht lesen: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", from.display()))?;
        let name = entry.file_name();
        let shown = name.to_string_lossy();
        if top && DB_FILES.contains(&shown.as_ref()) {
            continue;
        }
        let path = entry.path();
        let target = to.join(&name);
        let relative = if rel.is_empty() { shown.to_string() } else { format!("{rel}/{shown}") };
        let kind = std::fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?.file_type();
        if kind.is_symlink() {
            skipped.push(relative);
        } else if kind.is_dir() {
            std::fs::create_dir(&target).map_err(|e| format!("{} lässt sich nicht anlegen: {e}", target.display()))?;
            copy_tree(&path, &target, &relative, false, skipped)?;
        } else if kind.is_file() {
            std::fs::copy(&path, &target).map_err(|e| format!("{} lässt sich nicht kopieren: {e}", path.display()))?;
        } else {
            // Device files, FIFOs, sockets: do not copy, but report.
            skipped.push(relative);
        }
    }
    Ok(())
}

/// Free space (usable for non-root) of the file system under `path`; `None` if unknown.
#[cfg(unix)]
fn free_space(path: &Path) -> Option<u64> {
    rustix::fs::statvfs(path).ok().map(|s| s.f_bavail.saturating_mul(s.f_frsize))
}

/// Outside of Unix (Windows) the free space stays unknown: `statvfs` does not exist there, and the
/// Windows query (`GetDiskFreeSpaceExW`) would need `unsafe` or a further crate. [`room_for`] does not stop
/// on `None`; if the space is not enough, copying fails with the message of the system, and the
/// backup that was begun is removed again as on every error.
#[cfg(not(unix))]
fn free_space(_path: &Path) -> Option<u64> {
    None
}

/// Is the space enough for a backup of `needed` bytes with `available` free bytes? Afterwards
/// [`RESERVE_BYTES`] must still stay free. Unknown space (`None`) does not stop.
pub(crate) fn room_for(needed: u64, available: Option<u64>) -> Result<(), String> {
    let Some(available) = available else { return Ok(()) };
    if available >= needed.saturating_add(RESERVE_BYTES) {
        return Ok(());
    }
    Err(format!(
        "Zu wenig Platz für die Sicherung: Sie braucht etwa {}, frei sind {}, und {} sollen danach frei \
         bleiben (für die laufende Datenbank und die Spiegel); nichts angelegt.",
        human_size(needed),
        human_size(available),
        human_size(RESERVE_BYTES)
    ))
}

/// Name of the newest backup `<name>.sicherung-<JJJJ-MM-TT-HHMMSS>` next to the data folder `data` (the time stamp
/// sorts like the time); `None` if there is none or the folder is not readable. Only folders count.
pub fn newest_backup(data: &Path) -> Option<String> {
    let data = std::fs::canonicalize(data).ok()?;
    let (parent, name) = (data.parent()?, data.file_name()?.to_string_lossy().into_owned());
    let prefix = format!("{name}.sicherung-");
    std::fs::read_dir(parent)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix))
        .max()
}

/// Backups `<name>.sicherung-*` in `parent`.
fn existing_backups(parent: &Path, name: &str) -> usize {
    let prefix = format!("{name}.sicherung-");
    std::fs::read_dir(parent)
        .map(|entries| {
            entries.filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().starts_with(&prefix)).count()
        })
        .unwrap_or(0)
}

/// Backup of the data folder now (see the module documentation). `Err` = German message.
pub async fn backup(state: &AppState) -> Result<Backup, String> {
    let _held = state.write_lock.lock().await;
    let data = std::fs::canonicalize(&state.data)
        .map_err(|e| format!("Datenordner {} nicht gefunden: {e}", state.data.display()))?;
    let (Some(parent), Some(name)) = (data.parent(), data.file_name()) else {
        return Err(format!("Der Datenordner {} hat keinen Elternordner für die Sicherung.", data.display()));
    };
    let stamp: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%d-%H%M%S', 'now', 'localtime')")
        .fetch_one(state.store.pool())
        .await
        .map_err(|e| format!("Zeit der Sicherung: {e}"))?;
    let name = name.to_string_lossy().into_owned();
    let (dir, parent_dir, base) = (data.clone(), parent.to_owned(), name.clone());
    let (count, needed, available) = tokio::task::spawn_blocking(move || {
        (existing_backups(&parent_dir, &base), dir_size(&dir), free_space(&parent_dir))
    })
    .await
    .map_err(|e| format!("Platz prüfen abgebrochen: {e}"))?;
    if count >= MAX_BACKUPS {
        return Err(format!(
            "Neben dem Datenordner liegen schon {count} Sicherungen ({name}.sicherung-…, höchstens {MAX_BACKUPS}); \
             bitte alte entfernen oder verschieben. Nichts angelegt."
        ));
    }
    room_for(needed, available)?;
    let target = parent.join(format!("{name}.sicherung-{stamp}"));
    match std::fs::create_dir(&target) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!(
                "Die Sicherung {} gibt es schon; nichts überschrieben. Bitte in einer Sekunde noch einmal.",
                target.display()
            ));
        }
        Err(e) => return Err(format!("{} lässt sich nicht anlegen: {e}", target.display())),
    }
    let result = fill(state, &data, &target).await;
    if result.is_err() {
        // Remove only the folder just created again, so that no half backup stays.
        let _ = std::fs::remove_dir_all(&target);
    }
    result
}

async fn fill(state: &AppState, data: &Path, target: &Path) -> Result<Backup, String> {
    let db = target.join("baruch.sqlite");
    let db_text = db.to_str().ok_or_else(|| format!("{} ist kein UTF-8-Pfad", db.display()))?.to_owned();
    sqlx::query("VACUUM INTO ?")
        .bind(db_text)
        .execute(state.store.pool())
        .await
        .map_err(|e| format!("Datenbank sichern (VACUUM INTO): {e}"))?;
    let (from, to) = (data.to_owned(), target.to_owned());
    let skipped = tokio::task::spawn_blocking(move || {
        let mut skipped = Vec::new();
        copy_tree(&from, &to, "", true, &mut skipped).map(|()| skipped)
    })
    .await
    .map_err(|e| format!("Kopieren abgebrochen: {e}"))??;
    let dir = target.to_owned();
    let size_bytes = tokio::task::spawn_blocking(move || dir_size(&dir)).await.unwrap_or(0);
    Ok(Backup { dir: target.to_owned(), size_bytes, skipped })
}

#[derive(Deserialize, Default)]
pub(super) struct ActForm {
    work: Option<String>,
    confirm: Option<String>,
}

/// `POST /einstellungen/daten/sicherung` (`confirm=1` mandatory, otherwise 422) → 303
/// `/einstellungen/daten?ok=sicherung[&work=W]` (Round 23a: fixed message code without `ziel`; the page shows the
/// folder name from the file system, see [`newest_backup`]); error → 422 with the page.
pub(super) async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work = form.work.as_deref().and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|&w| w > 0);
    if !form.confirm.as_deref().is_some_and(|c| matches!(c, "1" | "on")) {
        let message = "Sicherung bitte bestätigen (Kästchen ankreuzen).";
        return super::error_page(&state, &headers, "daten", work, message).await;
    }
    match backup(&state).await {
        Ok(made) => {
            note_backup(&state.data, &made);
            Redirect::to(&format!("/einstellungen/daten?ok=sicherung{}", super::work_suffix(work))).into_response()
        }
        Err(message) => super::error_page(&state, &headers, "daten", work, &message).await,
    }
}

#[derive(Deserialize, Default)]
pub(super) struct InstanceForm {
    work: Option<String>,
    name: Option<String>,
}

/// `POST /einstellungen/daten/instanz` (`name`; if it is missing, it counts as empty = reset) → 303
/// `/einstellungen/daten?ok=instanz[&work=W]`; invalid name → 422 with the page and the message, nothing
/// stored.
pub(super) async fn instance_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<InstanceForm>,
) -> Response {
    let work = form.work.as_deref().and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|&w| w > 0);
    match state.store.set_instance_name(form.name.as_deref().unwrap_or("")).await {
        Ok(_) => Redirect::to(&format!("/einstellungen/daten?ok=instanz{}", super::work_suffix(work))).into_response(),
        Err(baruch_store::StoreError::Exchange(message)) => {
            super::error_page(&state, &headers, "daten", work, &message).await
        }
        Err(e) => super::error_page(&state, &headers, "daten", work, &e.to_string()).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backup_leaves_the_reserve_free_or_is_refused() {
        let mib = 1024 * 1024;
        assert!(room_for(10 * mib, None).is_ok(), "unbekannter Platz hält nicht auf (andere Systeme)");
        assert!(room_for(10 * mib, Some(10 * mib + RESERVE_BYTES)).is_ok(), "genau Sicherung plus Rücklage reicht");
        let refused = room_for(10 * mib, Some(10 * mib + RESERVE_BYTES - 1)).expect_err("ein Byte zu wenig");
        assert!(refused.contains("Zu wenig Platz") && refused.contains("nichts angelegt"), "Meldung: {refused}");
        assert!(room_for(u64::MAX - 1, Some(u64::MAX - 1)).is_err(), "kein Überlauf bei riesigen Zahlen");
    }

    #[test]
    fn sizes_read_like_german_text() {
        assert_eq!(human_size(12), "12 Bytes", "kleine Zahl in Bytes");
        assert_eq!(human_size(1536), "1,5 KiB", "Komma als Dezimalzeichen");
        assert_eq!(human_size(3 * 1024 * 1024), "3,0 MiB", "Mebibyte");
    }
}
