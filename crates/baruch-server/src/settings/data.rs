// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einstellungsseite „Daten“: Datenordner, Größe (rekursiv, symbolische Verknüpfungen nicht verfolgt),
//! je Werk der Git-Spiegel mit letztem Commit, Version des Programms (nur Anzeige, seit Runde 24 mit Link auf `/ueber`) und „Sicherung
//! jetzt“ (nur der Mensch: Formular `POST /einstellungen/daten/sicherung` mit `confirm=1` bzw. RPC
//! `settings.backup {confirm: true}`, kein MCP).
//!
//! Instanz (Runde 26): Kennung (`store_meta.instance_uuid`, nur Anzeige) und Name dieser Baruch-Instanz, der
//! im Austauschformat als Herkunft mitgeht. Formular `POST /einstellungen/daten/instanz` mit `name` (getrimmt,
//! höchstens 80 Zeichen, keine Steuerzeichen, leer = zurücksetzen; [`instance_act`]), RPC `instance.get` und
//! `instance.set_name` ([`crate::instance`]), kein MCP.
//!
//! EPUBCheck (Runde 20c): Block mit dem Pfad zu einem epubcheck.jar (`store_meta.epubcheck_jar`), dem gefundenen
//! Werkzeug (`epubcheck` im PATH geht vor) und ob `java` im PATH liegt. Formular `POST /einstellungen/daten/epubcheck`
//! (Feld `jar`, leer = entfernen) und RPC `epub.checker_get`/`epub.checker_set` in [`crate::epub_check`], kein MCP.
//!
//! Sicherung ([`backup`]): unter [`AppState::write_lock`] (kein Commit dazwischen) entsteht neben dem
//! Datenordner `<Elternordner>/<Name>.sicherung-<JJJJ-MM-TT-HHMMSS>` (Ortszeit aus SQLite
//! `strftime(…, 'localtime')`). Gibt es den Ordner schon (zweite Sicherung in derselben Sekunde), ist
//! das ein Fehler und nichts wird überschrieben. `baruch.sqlite` kommt per `VACUUM INTO` (ein in sich
//! stimmiger Stand, auch während der Server läuft), alles andere wird rekursiv kopiert außer
//! `baruch.sqlite`, `-wal` und `-shm`; symbolische Verknüpfungen werden übersprungen und gemeldet.
//!
//! Grenzen, damit wiederholte Sicherungen das Dateisystem nicht füllen (es trägt auch die laufende
//! Datenbank und die Spiegel): Liegen neben dem Datenordner schon [`MAX_BACKUPS`] Sicherungen
//! (`<Name>.sicherung-*`), lehnt „Sicherung jetzt“ ab, bis alte entfernt sind (Baruch löscht keine
//! Sicherung selbst). Vorher wird der freie Platz geprüft (Unix: `statvfs`): Nach der Sicherung (so groß
//! wie der Datenordner) müssen noch [`RESERVE_BYTES`] frei bleiben, sonst wird nichts angelegt.

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

/// Höchstzahl der Sicherungen neben dem Datenordner; danach lehnt „Sicherung jetzt“ ab.
pub const MAX_BACKUPS: usize = 10;

/// So viel muss nach einer Sicherung auf dem Dateisystem noch frei bleiben (512 MiB).
pub const RESERVE_BYTES: u64 = 512 * 1024 * 1024;

/// Dateien der Datenbank, die nicht kopiert werden (`baruch.sqlite` kommt per `VACUUM INTO`).
const DB_FILES: [&str; 3] = ["baruch.sqlite", "baruch.sqlite-wal", "baruch.sqlite-shm"];

/// Größe eines Ordners in Bytes, rekursiv; symbolische Verknüpfungen zählen nicht und werden nicht
/// verfolgt. Unlesbares wird übergangen.
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

/// Ein Werk mit seinem Spiegel.
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
    // Runde 23a: fester Meldecode `?ok=sicherung`. Den Ordnernamen bestimmt der Server selbst (neueste Sicherung neben
    // dem Datenordner); ein mitgeschicktes `ziel` aus der Adresse wird nie angezeigt (sonst ließe sich über einen Link
    // eine falsche Erfolgsmeldung mit beliebigem Namen zeigen). Ohne lesbare Sicherung bleibt die Meldung allgemein.
    let saved = if query.get("ok").map(String::as_str) == Some("sicherung") {
        let data = state.data.clone();
        Some(tokio::task::spawn_blocking(move || newest_backup(&data)).await.ok().flatten().unwrap_or_default())
    } else {
        None
    };
    let instance = state.store.instance().await.map_err(|e| e.to_string())?;
    let instance_saved = query.get("ok").map(String::as_str) == Some("instanz");
    // Runde 20c: Block „EPUBCheck“ (Pfad zum epubcheck.jar, Formular in crate::epub_check).
    let epubcheck =
        crate::epub_check::settings_context(state, query.get("ok").map(String::as_str) == Some("epubcheck")).await;
    super::render_template(
        "settings/daten.html",
        context! {
            work => work, error => info.error, saved => saved,
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

/// Ergebnis einer Sicherung.
#[derive(Debug, Clone)]
pub struct Backup {
    /// Ordner der Sicherung (neben dem Datenordner).
    pub dir: PathBuf,
    pub size_bytes: u64,
    /// Übersprungene symbolische Verknüpfungen (relativ zum Datenordner, mit `/`).
    pub skipped: Vec<String>,
}

/// Kopiert `from` nach `to` (das es noch nicht gibt), rekursiv; auf oberster Ebene ohne [`DB_FILES`].
/// Symbolische Verknüpfungen werden nicht kopiert, sondern in `skipped` gemeldet.
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
            // Gerätedateien, FIFOs, Sockets: nicht kopieren, aber melden.
            skipped.push(relative);
        }
    }
    Ok(())
}

/// Freier Platz (für Nicht-Root nutzbar) des Dateisystems unter `path`; `None`, wenn unbekannt.
#[cfg(unix)]
fn free_space(path: &Path) -> Option<u64> {
    rustix::fs::statvfs(path).ok().map(|s| s.f_bavail.saturating_mul(s.f_frsize))
}

/// Außerhalb von Unix (Windows) bleibt der freie Platz unbekannt: `statvfs` gibt es dort nicht, und die
/// Windows-Abfrage (`GetDiskFreeSpaceExW`) bräuchte `unsafe` oder eine weitere Crate. [`room_for`] hält bei
/// `None` nicht auf; reicht der Platz nicht, scheitert das Kopieren mit der Meldung des Systems, und die
/// angefangene Sicherung wird wie bei jedem Fehler wieder entfernt.
#[cfg(not(unix))]
fn free_space(_path: &Path) -> Option<u64> {
    None
}

/// Reicht der Platz für eine Sicherung von `needed` Bytes bei `available` freien Bytes? Danach müssen
/// noch [`RESERVE_BYTES`] frei bleiben. Unbekannter Platz (`None`) hält nicht auf.
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

/// Name der neuesten Sicherung `<name>.sicherung-<JJJJ-MM-TT-HHMMSS>` neben dem Datenordner `data` (der Zeitstempel
/// sortiert wie die Zeit); `None`, wenn es keine gibt oder der Ordner nicht lesbar ist. Nur Ordner zählen.
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

/// Sicherungen `<name>.sicherung-*` in `parent`.
fn existing_backups(parent: &Path, name: &str) -> usize {
    let prefix = format!("{name}.sicherung-");
    std::fs::read_dir(parent)
        .map(|entries| {
            entries.filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().starts_with(&prefix)).count()
        })
        .unwrap_or(0)
}

/// Sicherung des Datenordners jetzt (siehe Moduldoku). `Err` = deutsche Meldung.
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
        // Nur den eben angelegten Ordner wieder entfernen, damit keine halbe Sicherung bleibt.
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

/// `POST /einstellungen/daten/sicherung` (`confirm=1` Pflicht, sonst 422) → 303
/// `/einstellungen/daten?ok=sicherung[&work=W]` (Runde 23a: fester Meldecode ohne `ziel`; den Ordnernamen zeigt die
/// Seite aus dem Dateisystem, siehe [`newest_backup`]); Fehler → 422 mit der Seite.
pub(super) async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work = form.work.as_deref().and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|&w| w > 0);
    if !form.confirm.as_deref().is_some_and(|c| matches!(c, "1" | "on")) {
        let message = "Sicherung bitte bestätigen (Kästchen ankreuzen).";
        return super::error_page(&state, &headers, "daten", work, message).await;
    }
    match backup(&state).await {
        Ok(_) => {
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

/// `POST /einstellungen/daten/instanz` (`name`; fehlt er, gilt er als leer = zurücksetzen) → 303
/// `/einstellungen/daten?ok=instanz[&work=W]`; ungültiger Name → 422 mit der Seite und der Meldung, nichts
/// gespeichert.
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
