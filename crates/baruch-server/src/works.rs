// SPDX-License-Identifier: AGPL-3.0-or-later
//! Werke verwalten (Vertrag Runde 4, Abschnitte 4–6): Übersicht, Anlegen, Hochladen, Ändern,
//! Löschen und Export, alles durch den Menschen und ohne JavaScript bedienbar. Über MCP gibt es
//! davon nichts (ein Werk anzulegen, zu ändern oder zu löschen ist dem Menschen vorbehalten).
//!
//! RPC ([`handles`], [`call`]; aus `rpc::call` heraus, vor der Wahl des Vorgabewerks, geht also auch
//! ohne Werke):
//! - `work.list` → `{works: [WorkSummary]}`
//! - `work.meta {work?}` → `{work: WorkMeta, template_revisions}` (seit Runde 26 mit `work.uuid`, nur lesend)
//! - `work.create {title, author?, kind?, preset?, subtitle?, audience?, bible?, text_ref?, module?, from_work?}` →
//!   `{work, slug, title, nodes, sermon_text?, commit?, mirror?}` (Runde 16a: Predigt mit Predigttext aus SWORD)
//! - `work.import {file_name, source, title?}` → `ImportReport` (`.typ` oder seit Runde 21b `.md`/`.markdown`, nur
//!   Text, höchstens 16 MiB, sonst -32602; die ganze Anfrage an `/rpc` höchstens [`RPC_BODY_LIMIT`], sonst 413)
//! - `work.import_archive {file_name, content, title?}` → `ImportReport` (Runde 21b: ZIP als Base64 mit Haupt-Datei
//!   `.typ`/`.md` und Bildern, [`crate::markdown_import`]; andere Endung oder kaputtes Archiv → -32602)
//! - `work.update {work, title?, subtitle?, author?, kind?, audience?, bible?, template?, note?}` →
//!   `{work, changed, template_rev?, commit?, mirror?}`
//! - `work.delete {work, confirm_title}` → `{deleted, title, slug, archive, next, commit?, mirror?, ideas_commit?}`
//! - `work.archives {}` → `{archives: [{slug, title, deleted, folder?, restorable, reason?}]}` (Runde 22, nur lesend)
//! - `work.restore {slug}` → `{work, slug, title, nodes, folder, skipped_images, commit?|mirror?}` (Runde 22, nur der
//!   Mensch; nicht über `/act` und MCP)
//!
//! Seiten ([`pages`]): `GET /werke` (Runde 11: je Werk Umschlag-Vorschaubild und Link „PDF“), `/werke/neu`, `/werke/{id}/bearbeiten[?vorlage=n]`,
//! `/werke/{id}/loeschen`, `/werke/{id}/export`. Formulare ([`writes`], hinter `guard::same_origin`):
//! `POST /werke/import` (multipart, Felder `file` (`.typ`, `.md`, `.markdown`, `.zip`), `files` (mehrere Dateien mit
//! relativen Pfaden = Ordner, Runde 21b) und `title`; höchstens [`IMPORT_UPLOAD_LIMIT`]) und `POST /werke/act` (`action` =
//! `create`, `update`, `delete`). Ein Import mit Hinweisen oder übersprungenen Bildern zeigt den Bericht
//! (`import_report.html`, 200) statt weiterzuleiten.
//!
//! Spiegel: Anlegen → Commit „Werk angelegt: <Titel>“, Ändern → „Werk geändert: <Felder>“ (nur bei
//! geänderten Dateien), Löschen → leerer Protokoll-Commit „Werk gelöscht: <Titel>“ (Trailer
//! `Baruch-Action: work-deleted`), danach wandert der Ordner nach `<data>/works/.geloescht/<slug>-<JJJJ-MM-TT>/`.
//! Ein Fehler im Spiegel hält die Änderung in der Datenbank nicht auf; die Antwort trägt dann `mirror`.
//! Seit Runde 22 führt das Löschen danach den Spiegel der Einfälle ohne Werk nach ([`crate::mirror_ideas`]): Die
//! Einfälle des Werks sind dort sofort sichtbar.
//!
//! Archiv (Runde 22, Vertrag §2.16): `GET /werke/archiv` listet die gelöschten Werke (`work_retired`) mit ihrem
//! Archivordner `works/.geloescht/<slug>-<JJJJ-MM-TT>[-n]`; `POST /werke/archiv/wiederherstellen` (Felder `slug`,
//! `confirm`) stellt eins wieder her ([`restore`]): Archivordner lesen ([`read_archive`]: keine Symlinks, nur
//! `main.typ`, `vorlage.typ` und die eingebundenen `abschnitte/*.typ`, zusammen höchstens [`MAX_SOURCE_BYTES`]), daraus
//! den Typst-Text wie einen Export zusammensetzen (Buchende und Projektion wie beim Import entfernt), im Store mit dem
//! reservierten slug anlegen (`Store::restore_retired`), Ordner zurück nach `works/<slug>`, ein Commit „Werk
//! wiederhergestellt: <Titel>“ (`Baruch-Action: work-restored`). Bilder des Archivs kommen nicht mit (Liste
//! `skipped_images`); Einfälle ohne Werk bleiben ohne Werk.

use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::multipart::MultipartRejection;
use axum::extract::{DefaultBodyLimit, Form, Multipart, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::WorkKind;
use baruch_gitmirror::{CommitRequest, MirrorFile, Signature};
use baruch_store::{DeletedWork, NewWork, Preset, StoreError, WorkEdit, WorkId, title_key};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::Deserialize;
use serde_json::json;

use crate::import::{self, ImportReport, MAX_SOURCE_BYTES};
use crate::markdown_import::{self, Bundle};
use crate::mirror::{self, HUMAN_EMAIL, HUMAN_NAME, WriteGuard};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::Settings;
use crate::{AppState, mirror_dir};

/// Ordner unter `<data>/works/`, in den gelöschte Werke wandern.
pub const TRASH_DIR: &str = ".geloescht";

/// Größte Anfrage an `POST /werke/import`: 16 MiB Datei und 64 KiB für Titel und Rahmen.
pub const UPLOAD_LIMIT: usize = MAX_SOURCE_BYTES as usize + 64 * 1024;

/// Größte Anfrage an `POST /werke/import` seit Runde 21b: ZIP oder Ordner bis 80 MiB ([`crate::archive::IMPORT_LIMITS`])
/// und 64 KiB für Titel und Rahmen. Gilt nur an dieser Route; [`UPLOAD_LIMIT`] (eine Typst-/Markdown-Datei) und
/// [`RPC_BODY_LIMIT`] bleiben, wie sie sind.
pub const IMPORT_UPLOAD_LIMIT: usize = 80 * 1024 * 1024 + 64 * 1024;

/// Größte Anfrage an `POST /rpc` (verdrahtet in `lib.rs` über `DefaultBodyLimit`): `work.import`
/// trägt den Typst-Text als JSON-String, und die Maskierung (`\n`, `\"`, `\\`) kann ihn bis aufs
/// Doppelte wachsen lassen. Darum 2 × 16 MiB und 64 KiB für den Rahmen. Bis 16 MiB Text nimmt
/// `work.import` an, darüber → -32602 „größer als 16 MiB“; eine Anfrage über dieser Grenze lehnt
/// axum schon vorher mit 413 ab.
pub const RPC_BODY_LIMIT: usize = 2 * MAX_SOURCE_BYTES as usize + 64 * 1024;

/// Runde 32: Antwort von `POST /werke/import` (422) und `work.import` (-32602) auf eine Word-Datei – sie verweist auf
/// den eigenen Import mit Vorschau ([`crate::docx_import`]).
pub const USE_WORD_IMPORT: &str = "Word-Dateien liest der eigene Import mit Vorschau: Baruch ▾ → Word (.docx) \
     importieren… (/werke/import/word) bzw. RPC docx_import.preview.";

// ---- RPC ---------------------------------------------------------------------------------------

/// Methoden dieses Moduls.
pub fn handles(method: &str) -> bool {
    matches!(
        method,
        "work.list"
            | "work.meta"
            | "work.create"
            | "work.import"
            | "work.import_archive"
            | "work.update"
            | "work.delete"
            | "work.archives"
            | "work.restore"
    )
}

#[derive(Debug, Default, Deserialize)]
struct CreateParams {
    #[serde(default)]
    title: String,
    author: Option<String>,
    kind: Option<String>,
    preset: Option<String>,
    subtitle: Option<String>,
    audience: Option<String>,
    bible: Option<String>,
    /// Predigttext als Stellenangabe (nur bei einer Predigt, Runde 16a).
    text_ref: Option<String>,
    /// SWORD-Bibel für den Predigttext (leer: Bibel von `from_work`, sonst des Vorgabewerks).
    module: Option<String>,
    /// Werk, aus dem heraus angelegt wird (Dialog „Importieren…“): dessen Bibel gilt bei leerem `module`.
    from_work: Option<WorkId>,
}

#[derive(Deserialize)]
struct ImportParams {
    file_name: String,
    source: String,
    title: Option<String>,
}

#[derive(Deserialize)]
struct ArchiveParams {
    file_name: String,
    /// ZIP-Archiv als Base64.
    content: String,
    title: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct UpdateParams {
    work: Option<WorkId>,
    title: Option<String>,
    subtitle: Option<String>,
    author: Option<String>,
    kind: Option<String>,
    audience: Option<String>,
    bible: Option<String>,
    template: Option<String>,
    note: Option<String>,
}

#[derive(Deserialize)]
struct DeleteParams {
    work: Option<WorkId>,
    confirm_title: Option<String>,
}

#[derive(Deserialize)]
struct RestoreParams {
    slug: Option<String>,
}

#[derive(Deserialize)]
struct MetaParams {
    work: Option<WorkId>,
}

fn params<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Werkart aus `book`/`sermon`; leer → `None`.
fn kind_param(value: Option<&str>) -> Result<Option<WorkKind>, RpcError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some("book") => Ok(Some(WorkKind::Book)),
        Some("sermon") => Ok(Some(WorkKind::Sermon)),
        Some(other) => Err(invalid(format!("kind: „book“ oder „sermon“ erwartet, nicht „{other}“"))),
    }
}

fn preset_param(value: Option<&str>) -> Result<Option<Preset>, RpcError> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(None),
        Some("book") => Ok(Some(Preset::Book)),
        Some("sermon") => Ok(Some(Preset::Sermon)),
        Some(other) => Err(invalid(format!("preset: „book“ oder „sermon“ erwartet, nicht „{other}“"))),
    }
}

fn required_work(work: Option<WorkId>) -> Result<WorkId, RpcError> {
    work.ok_or_else(|| invalid("work fehlt: Welches Werk? (Werk-id, Zahl)"))
}

/// Ergebnis des Spiegels in die Antwort: `commit` bei neuem Commit, `mirror` bei Fehler.
fn add_mirror(answer: &mut serde_json::Value, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

pub async fn call(
    state: &AppState,
    method: &str,
    params_value: serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    match method {
        // Runde 35: gefiltert nach Rolle (ein Lektor sieht nur freigegebene Werke).
        "work.list" => Ok(json!({ "works": crate::roles::visible_works(state, &crate::auth::current()).await? })),
        "work.meta" => {
            let p: MetaParams = params(params_value)?;
            let work = match p.work {
                Some(work) => work,
                None => state.default_work().await?,
            };
            let meta = state.store.work_meta(work).await?;
            let revisions = state.store.template_revisions(work).await?;
            Ok(json!({ "work": meta, "template_revisions": revisions }))
        }
        "work.create" => create(state, &params(params_value)?).await,
        "work.import" => {
            let p: ImportParams = params(params_value)?;
            let base = p.file_name.rsplit(['/', '\\']).next().unwrap_or("").trim();
            // Runde 32: Word-Dateien liest nur der eigene Import mit Vorschau.
            if crate::docx_import::is_docx_name(base) {
                return Err(invalid(USE_WORD_IMPORT));
            }
            let report = if markdown_import::is_markdown_name(base) {
                markdown_import::import_text(state, &p.source, &p.file_name, p.title.as_deref()).await?
            } else if markdown_import::is_archive_name(base) {
                return Err(invalid("Ein ZIP-Archiv geht über work.import_archive (Inhalt als Base64)."));
            } else {
                import::import_upload(state, &p.source, &p.file_name, p.title.as_deref()).await?
            };
            Ok(json!(report))
        }
        "work.import_archive" => {
            let p: ArchiveParams = params(params_value)?;
            let base = p.file_name.rsplit(['/', '\\']).next().unwrap_or("").trim().to_owned();
            if !markdown_import::is_archive_name(&base) {
                return Err(invalid(format!("„{base}“ ist kein ZIP-Archiv; work.import_archive nimmt nur .zip.")));
            }
            let bytes = baruch_store::decode_base64(p.content.trim()).map_err(|e| invalid(e.to_string()))?;
            let report = markdown_import::import_archive(state, &bytes, &base, p.title.as_deref()).await?;
            Ok(json!(report))
        }
        "work.update" => update(state, &params(params_value)?).await,
        "work.delete" => {
            let p: DeleteParams = params(params_value)?;
            let work = required_work(p.work)?;
            let confirm =
                p.confirm_title.ok_or_else(|| invalid("confirm_title fehlt: Titel des Werks zur Bestätigung"))?;
            delete(state, work, &confirm).await
        }
        "work.archives" => Ok(json!({ "archives": archives(state).await? })),
        "work.restore" => {
            let p: RestoreParams = params(params_value)?;
            let slug = p
                .slug
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| invalid("slug fehlt: Ordnername des archivierten Werks (aus work.archives)"))?;
            restore(state, slug.trim()).await
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Längste Stellenangabe für den Predigttext (wie `bible.lookup`).
pub const MAX_TEXT_REF_CHARS: usize = 500;
/// Längster Name eines Bibelmoduls beim Anlegen einer Predigt (Regel der Tabelle `work_bible`).
pub const MAX_MODULE_CHARS: usize = 100;

/// Der Predigttext eines neuen Werks (Runde 16a): der Typst-Text unter der Überschrift „Predigttext“, die
/// Angaben für die Antwort (`sermon_text`) und die SWORD-Bibel, die das neue Werk bekommt.
struct SermonText {
    body: String,
    info: serde_json::Value,
    bible: Option<String>,
}

/// Wortlaut eines Verses als Typst-Text: Markup-Zeichen maskiert (wie ein Titel), Zeilenenden als
/// Leerzeichen. Die Wortlautprüfung entfernt die Maskierung wieder (`wording::strip_markup`).
fn typst_text(text: &str) -> String {
    baruch_typst::escape_title(text)
}

/// Predigttext aus der Stellenangabe `text_ref` lesen: genau eine gültige Stelle (sonst -32602 mit der
/// Meldung der Stellenprüfung, nichts angelegt). Bibel: `module`, sonst die von `from_work`, sonst die des
/// Vorgabewerks. Mit lesbarer Bibel steht der Wortlaut als Blockzitat mit Quelle (Stelle und Modul) da,
/// fehlende Verse zählt `missing`; ohne Bibel nur die Stellenangabe und ein deutscher Hinweis (`notice`).
async fn sermon_text(
    state: &AppState,
    text_ref: &str,
    module: Option<&str>,
    from_work: Option<WorkId>,
) -> Result<SermonText, RpcError> {
    let text_ref = text_ref.trim();
    if text_ref.chars().count() > MAX_TEXT_REF_CHARS {
        return Err(invalid(format!("text_ref: höchstens {MAX_TEXT_REF_CHARS} Zeichen")));
    }
    // Runde 23: Der Modulname wird die Bibel des Werks (`work_bible`: 1–100 Zeichen, eine Zeile). Unzulässiges
    // lehnt schon die Eingabe ab, statt erst das Schreiben scheitern zu lassen.
    if module.is_some_and(|m| m.chars().count() > MAX_MODULE_CHARS || m.chars().any(char::is_control)) {
        return Err(invalid(format!(
            "module: Modulname mit höchstens {MAX_MODULE_CHARS} Zeichen in einer Zeile erwartet"
        )));
    }
    let found = baruch_scripture::parse(text_ref);
    let one = match found.as_slice() {
        [one] => one,
        [] => return Err(invalid(format!("Predigttext: „{text_ref}“ ist keine Bibelstelle (z. B. „Joh 3,16“)."))),
        _ => return Err(invalid(format!("Predigttext: bitte genau eine Bibelstelle angeben, nicht „{text_ref}“."))),
    };
    let (Some(passage), baruch_scripture::RefStatus::Valid) = (one.passage.as_ref(), &one.status) else {
        return Err(invalid(format!("Predigttext „{}“: {}", one.input, one.message())));
    };
    let normalized = one.normalized.clone().unwrap_or_else(|| one.input.clone());
    let choice = match module {
        Some(name) => crate::bibles::by_name(state, name).await?,
        None => {
            let work = match from_work {
                Some(work) => Some(work),
                None => match state.default_work().await {
                    Ok(work) => Some(work),
                    Err(StoreError::UnknownWork(_)) => None,
                    Err(e) => return Err(e.into()),
                },
            };
            match work {
                Some(work) => crate::bibles::of_work(state, work).await?,
                None => crate::bibles::Choice::None,
            }
        }
    };
    let reference = typst_text(&normalized);
    let plain = |notice: String, module: Option<String>| SermonText {
        body: reference.clone(),
        info: json!({
            "ref": text_ref, "normalized": normalized, "module": module, "quoted": false, "verses": 0,
            "missing": 0, "notice": notice,
        }),
        bible: None,
    };
    let bible = match &choice {
        crate::bibles::Choice::Ready(bible) => Arc::clone(bible),
        crate::bibles::Choice::None => {
            return Ok(plain(
                "Keine SWORD-Bibel eingerichtet (Einstellungen → Quellen → Bibeln); nur die Stellenangabe steht da."
                    .to_owned(),
                None,
            ));
        }
        crate::bibles::Choice::Missing(name) => {
            return Ok(plain(
                format!("Bibelmodul {name} in keinem Modulordner gefunden; nur die Stellenangabe steht da."),
                Some(name.clone()),
            ));
        }
        crate::bibles::Choice::Unusable(info, why) => {
            return Ok(plain(
                format!("Bibelmodul {}: {why}; nur die Stellenangabe steht da.", info.name),
                Some(info.name.clone()),
            ));
        }
    };
    let name = bible.info.name.clone();
    let (book, start, end) = (passage.book.id, passage.start, passage.end);
    // Runde 17b: über `bibles::read_passage` (deutsche Zählung → Zählung des Moduls); „gekürzt“ sagt die Stelle
    // selbst (`truncated`: mehr als MAX_PASSAGE_VERSES Verse), nicht die Zahl der gelesenen Verse – genau 400 Verse
    // sind vollständig.
    let read = {
        let bible = Arc::clone(&bible);
        tokio::task::spawn_blocking(move || crate::bibles::read_passage(&bible, book, start, end))
            .await
            .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    };
    let passage = match read {
        Ok(passage) => passage,
        Err(e) => return Ok(plain(format!("Bibelmodul {name}: {e}; nur die Stellenangabe steht da."), Some(name))),
    };
    let verses = passage.verses;
    let texts: Vec<&str> = verses.iter().filter_map(|v| v.text.as_deref()).map(str::trim).collect();
    let missing = verses.len() - texts.len();
    let truncated = passage.truncated;
    if texts.is_empty() {
        let mut out = plain(
            format!("Die Stelle fehlt im Bibelmodul {name} ({missing} Verse); nur die Stellenangabe steht da."),
            Some(name.clone()),
        );
        out.info["missing"] = json!(missing);
        return Ok(out);
    }
    let quote = texts.iter().map(|t| typst_text(t)).collect::<Vec<_>>().join(" ");
    let body = format!("#quote(block: true, attribution: [{reference} ({})])[{quote}]", typst_text(&name));
    let mut notes: Vec<String> = Vec::new();
    if missing > 0 {
        notes.push(format!("{missing} Verse fehlen im Bibelmodul {name}; der Wortlaut ist unvollständig."));
    }
    if truncated {
        notes.push(format!("Nur die ersten {} Verse eingefügt.", baruch_sword::MAX_PASSAGE_VERSES));
    }
    let notice = (!notes.is_empty()).then(|| notes.join(" "));
    Ok(SermonText {
        body,
        info: json!({
            "ref": text_ref, "normalized": normalized, "module": name, "quoted": true, "verses": texts.len(),
            "missing": missing, "notice": notice,
        }),
        bible: Some(name),
    })
}

/// Leere Angaben (nur Leerraum) zählen als nicht angegeben.
fn given(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

/// Neues Werk anlegen und spiegeln. Art und Grundgerüst ergänzen sich: fehlt eins, gilt das andere
/// (Predigt ↔ Predigtteile), sonst Buch. Runde 16a: Bei einer Predigt mit `text_ref` steht der Predigttext
/// (Zitat mit Quelle bzw. die Stellenangabe) im ersten Punkt, und die gewählte Bibel wird die SWORD-Bibel des
/// Werks; die Antwort trägt dann `sermon_text`. Runde 23: Werk und Bibel entstehen in **einer** Transaktion
/// (`Store::create_work_with_bible`) – scheitert die Bibel, gibt es kein halbes Werk, und ein zweiter Versuch
/// endet nicht mit `DuplicateTitle`.
async fn create(state: &AppState, p: &CreateParams) -> Result<serde_json::Value, RpcError> {
    let kind = kind_param(p.kind.as_deref())?;
    let preset = preset_param(p.preset.as_deref())?;
    let (kind, preset) = match (kind, preset) {
        (Some(kind), Some(preset)) => (kind, preset),
        (Some(WorkKind::Sermon), None) => (WorkKind::Sermon, Preset::Sermon),
        (Some(kind), None) => (kind, Preset::Book),
        (None, Some(Preset::Sermon)) => (WorkKind::Sermon, Preset::Sermon),
        (None, _) => (WorkKind::Book, Preset::Book),
    };
    let sermon = match given(p.text_ref.as_deref()) {
        Some(_) if preset != Preset::Sermon => {
            return Err(invalid("Einen Predigttext gibt es nur beim Grundgerüst „Predigt“."));
        }
        Some(text_ref) => Some(sermon_text(state, text_ref, given(p.module.as_deref()), p.from_work).await?),
        None => None,
    };
    let new = NewWork {
        title: &p.title,
        author: p.author.as_deref().unwrap_or(""),
        kind,
        preset,
        subtitle: p.subtitle.as_deref().unwrap_or(""),
        audience: p.audience.as_deref().unwrap_or(""),
        bible: p.bible.as_deref().unwrap_or(""),
    };
    let held = state.write_lock.lock().await;
    let created = state
        .store
        .create_work_with_bible(
            &new,
            HUMAN_NAME,
            sermon.as_ref().map(|s| s.body.as_str()),
            sermon.as_ref().and_then(|s| s.bible.as_deref()),
        )
        .await?;
    let mut answer =
        json!({ "work": created.work, "slug": created.slug, "title": created.title, "nodes": created.nodes });
    if let Some(sermon) = &sermon {
        answer["sermon_text"] = sermon.info.clone();
    }
    let summary = format!("Werk angelegt: {}", created.title);
    add_mirror(&mut answer, mirror::sync_work(state, &held, created.work, &summary).await);
    Ok(answer)
}

/// Deutsche Namen der Felder für den Commit-Betreff.
fn field_label(field: &str) -> &'static str {
    match field {
        "title" => "Titel",
        "subtitle" => "Untertitel",
        "author" => "Autor",
        "kind" => "Art",
        "audience" => "Zielgruppe",
        "bible" => "Bibelübersetzung",
        _ => "Werkvorlage",
    }
}

/// Betreff „Werk geändert: Titel, Autor, Werkvorlage Fassung 3“.
fn update_summary(changed: &[&str], template_rev: Option<u32>) -> String {
    let labels: Vec<String> = changed
        .iter()
        .map(|f| match (*f, template_rev) {
            ("template", Some(n)) => format!("Werkvorlage Fassung {n}"),
            (f, _) => field_label(f).to_owned(),
        })
        .collect();
    format!("Werk geändert: {}", labels.join(", "))
}

async fn update(state: &AppState, p: &UpdateParams) -> Result<serde_json::Value, RpcError> {
    let work = required_work(p.work)?;
    let edit = WorkEdit {
        title: p.title.as_deref(),
        subtitle: p.subtitle.as_deref(),
        author: p.author.as_deref(),
        kind: kind_param(p.kind.as_deref())?,
        audience: p.audience.as_deref(),
        bible: p.bible.as_deref(),
        template: p.template.as_deref(),
        note: p.note.as_deref(),
    };
    let held = state.write_lock.lock().await;
    let update = state.store.update_work(work, &edit, HUMAN_NAME).await?;
    let mut answer = json!({ "work": work, "changed": update.changed });
    if let Some(rev) = update.template_rev {
        answer["template_rev"] = json!(rev);
    }
    if !update.changed.is_empty() {
        let summary = update_summary(&update.changed, update.template_rev);
        add_mirror(&mut answer, mirror::sync_work(state, &held, work, &summary).await);
    }
    Ok(answer)
}

/// Werk löschen: Titel prüfen, Spiegel und Schnappschuss vorher holen (danach gibt es das Werk in
/// der Datenbank nicht mehr), in der Datenbank löschen, Protokoll-Commit, Ordner ins Archiv.
async fn delete(state: &AppState, work: WorkId, confirm_title: &str) -> Result<serde_json::Value, RpcError> {
    let held = state.write_lock.lock().await;
    let meta = state.store.work_meta(work).await?;
    if title_key(confirm_title) != title_key(&meta.title) {
        return Err(StoreError::Refused("Der eingegebene Titel passt nicht; nichts gelöscht.").into());
    }
    let prepared = prepare_mirror(state, work).await;
    let deleted = state.store.delete_work(work, confirm_title).await?;
    // Bedeutungssuche: die Zeilen des Werks verlassen den Index sofort (nie blockierend).
    crate::rag::notify(state, Some(work));
    let (commit, archive, error) = archive_mirror(state, &held, &deleted, prepared).await;
    // Runde 22: Die Einfälle des Werks sind jetzt Einfälle ohne Werk; ihr Spiegel holt sofort auf.
    let reason = format!("Einfälle ohne Werk nach dem Löschen von „{}“ gespiegelt", deleted.title);
    let ideas = crate::mirror_ideas::sync_ideas_without_work(state, &held, &reason).await;
    let next = match state.default_work().await {
        Ok(next) => Some(next),
        Err(StoreError::UnknownWork(_)) => None,
        Err(e) => return Err(e.into()),
    };
    let mut answer = json!({
        "deleted": deleted.work, "title": deleted.title, "slug": deleted.slug, "archive": archive, "next": next,
    });
    if let Some(commit) = commit {
        answer["commit"] = json!(commit);
    }
    if let Some(error) = error {
        eprintln!("Git-Spiegel (Werk {work} gelöscht): {error}");
        answer["mirror"] = json!(error);
    }
    match ideas {
        Ok(Some(commit)) => answer["ideas_commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["ideas_mirror"] = json!(e),
    }
    Ok(answer)
}

type Prepared = Result<(Arc<baruch_gitmirror::Mirror>, Vec<MirrorFile>), String>;

/// Spiegel öffnen und den letzten Stand des Werks holen (für den Protokoll-Commit).
async fn prepare_mirror(state: &AppState, work: WorkId) -> Prepared {
    let mirror = state.mirror(work).await?;
    let files = state.store.mirror_snapshot(work).await.map_err(|e| e.to_string())?;
    Ok((mirror, files.into_iter().map(|f| MirrorFile { path: f.path, contents: f.contents }).collect()))
}

fn human() -> Signature {
    Signature { name: HUMAN_NAME.to_owned(), email: HUMAN_EMAIL.to_owned() }
}

/// Protokoll-Commit „Werk gelöscht: …“ und Ordner nach `works/.geloescht/<slug>-<datum>/`.
/// Ergebnis: Commit-Id, Archivordner (relativ zum Datenordner), Fehlertext.
async fn archive_mirror(
    state: &AppState,
    _held: &WriteGuard<'_>,
    deleted: &DeletedWork,
    prepared: Prepared,
) -> (Option<String>, Option<String>, Option<String>) {
    let mut errors: Vec<String> = Vec::new();
    let mut commit = None;
    match prepared {
        Ok((mirror, files)) => {
            let request = CommitRequest {
                files,
                author: human(),
                committer: human(),
                summary: format!("Werk gelöscht: {}", deleted.title),
                body: format!(
                    "Gelöscht: {} Gliederungspunkte, {} Revisionen.\nDer Spiegel liegt danach unter works/{TRASH_DIR}/.\n",
                    deleted.nodes, deleted.revisions
                ),
                trailers: vec![("Baruch-Action".to_owned(), "work-deleted".to_owned())],
                allow_empty: true,
            };
            match tokio::task::spawn_blocking(move || mirror.commit(&request)).await {
                Ok(Ok(id)) => commit = id,
                Ok(Err(e)) => errors.push(e.to_string()),
                Err(e) => errors.push(format!("Git: {e}")),
            }
        }
        Err(e) => errors.push(e),
    }
    // Der Spiegel wird nicht mehr gebraucht; nicht offen halten, während der Ordner wandert.
    state.forget_mirror(deleted.work);
    let today = today();
    let data = state.data.clone();
    let slug = deleted.slug.clone();
    let moved = tokio::task::spawn_blocking(move || move_to_trash(&data, &slug, &today)).await;
    let archive = match moved {
        Ok(Ok(archive)) => archive,
        Ok(Err(e)) => {
            errors.push(e);
            None
        }
        Err(e) => {
            errors.push(format!("Verschieben abgebrochen: {e}"));
            None
        }
    };
    let error = if errors.is_empty() { None } else { Some(errors.join("; ")) };
    (commit, archive, error)
}

/// Spiegelordner `<data>/works/<slug>` nach `<data>/works/.geloescht/<slug>-<date>` (Kollision `-2`,
/// `-3` …) verschieben. Kein Ordner → `Ok(None)`. Symbolische Verknüpfungen (Spiegel, `works/`,
/// `.geloescht`) → Fehler, nichts verschoben. Ergebnis relativ zum Datenordner, mit `/`.
pub fn move_to_trash(data: &FsPath, slug: &str, date: &str) -> Result<Option<String>, String> {
    let dir = mirror_dir(data, slug)?;
    if std::fs::symlink_metadata(&dir).is_err() {
        return Ok(None);
    }
    let trash = data.join("works").join(TRASH_DIR);
    if std::fs::symlink_metadata(&trash).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("{} ist eine symbolische Verknüpfung; der Spiegel bleibt, wo er ist", trash.display()));
    }
    std::fs::create_dir_all(&trash).map_err(|e| format!("{} lässt sich nicht anlegen: {e}", trash.display()))?;
    let base = format!("{slug}-{date}");
    let mut name = base.clone();
    let mut n = 2;
    let target: PathBuf = loop {
        let candidate = trash.join(&name);
        if std::fs::symlink_metadata(&candidate).is_err() {
            break candidate;
        }
        name = format!("{base}-{n}");
        n += 1;
    };
    std::fs::rename(&dir, &target)
        .map_err(|e| format!("{} lässt sich nicht nach {} verschieben: {e}", dir.display(), target.display()))?;
    Ok(Some(format!("works/{TRASH_DIR}/{name}")))
}

/// Heutiges Datum in Ortszeit als `JJJJ-MM-TT` (Name des Ordners im Papierkorb, den der Nutzer sieht).
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

// ---- Archiv (Runde 22) -------------------------------------------------------------------------

/// Erste Zeile jeder Abschnittsdatei eines Spiegels mit Werkvorlage (`snapshot::TEMPLATE_IMPORT` im Store); beim
/// Zusammensetzen aus dem Archiv fällt sie weg.
const ARCHIVE_TEMPLATE_IMPORT: &str = "#import \"../vorlage.typ\": *\n";

/// Höchstens so viele eingebundene Abschnittsdateien liest [`read_archive`].
pub const MAX_ARCHIVE_SECTIONS: usize = 10_000;

/// Ein gelöschtes Werk in der Archivliste (`work.archives`, Seite `/werke/archiv`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ArchiveEntry {
    pub slug: String,
    pub title: String,
    /// Zeitpunkt des Löschens (UTC, aus `work_retired`).
    pub deleted: String,
    /// Archivordner relativ zum Datenordner (`works/.geloescht/<slug>-<JJJJ-MM-TT>`); `None` = nicht gefunden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub restorable: bool,
    /// Warum nicht wiederherstellbar (deutsch).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Aus dem Archivordner zusammengesetzter Typst-Text ([`read_archive`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archived {
    /// Werkvorlage und Abschnitte wie ein Export (ohne Buchende und Projektion).
    pub source: String,
    /// Bilder im Ordner `bilder/` des Archivs (werden nicht wiederhergestellt, nur genannt).
    pub images: Vec<String>,
}

fn is_date(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// Archivordner des Werks `slug`: `works/.geloescht/<slug>-<JJJJ-MM-TT>` bzw. bei einer Kollision `…-<n>`
/// ([`move_to_trash`]); bei mehreren der jüngste. Ein anderes Werk, dessen slug mit `slug-` beginnt, passt nicht (nach
/// `slug-` muss gleich das Datum folgen). `works/` bzw. `.geloescht` als symbolische Verknüpfung → Fehler; kein
/// Papierkorb → `Ok(None)`. Ergebnis relativ zum Datenordner, mit `/`.
pub fn archive_folder(data: &FsPath, slug: &str) -> Result<Option<String>, String> {
    let works = data.join("works");
    let trash = works.join(TRASH_DIR);
    for path in [&works, &trash] {
        match std::fs::symlink_metadata(path) {
            Err(_) => return Ok(None),
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "{} ist eine symbolische Verknüpfung; das Archiv wird nicht gelesen",
                    path.display()
                ));
            }
            Ok(m) if !m.is_dir() => return Ok(None),
            Ok(_) => {}
        }
    }
    let entries = std::fs::read_dir(&trash).map_err(|e| format!("{} lässt sich nicht lesen: {e}", trash.display()))?;
    let mut best: Option<(String, u32, String)> = None;
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        let Some(rest) = name.strip_prefix(slug).and_then(|r| r.strip_prefix('-')) else { continue };
        if !rest.is_ascii() {
            continue;
        }
        let (date, n) = if rest.len() == 10 {
            (rest, 1)
        } else if rest.len() > 11 && rest.as_bytes()[10] == b'-' {
            match rest[11..].parse::<u32>() {
                Ok(n) => (&rest[..10], n),
                Err(_) => continue,
            }
        } else {
            continue;
        };
        if !is_date(date) {
            continue;
        }
        let candidate = (date.to_owned(), n, name.clone());
        if best.as_ref().is_none_or(|b| (&candidate.0, candidate.1) > (&b.0, b.1)) {
            best = Some(candidate);
        }
    }
    Ok(best.map(|(_, _, name)| format!("works/{TRASH_DIR}/{name}")))
}

/// `#include "abschnitte/…"`-Zeile → Pfad.
fn include_path(line: &str) -> Option<&str> {
    line.trim_end().strip_prefix("#include \"")?.strip_suffix('"')
}

/// Nur `abschnitte/<name>.typ` ohne weitere Ordner, ohne `..`, ohne Steuerzeichen.
fn section_path_ok(path: &str) -> bool {
    let Some(name) = path.strip_prefix("abschnitte/") else { return false };
    !name.is_empty()
        && name.ends_with(".typ")
        && name.len() > 4
        && !name.contains(['/', '\\'])
        && !name.starts_with('.')
        && !name.chars().any(char::is_control)
}

/// Reguläre Datei (keine Verknüpfung) `rel` unter `dir` als UTF-8 lesen; `budget` = noch erlaubte Bytes.
fn read_plain(dir: &FsPath, rel: &str, budget: &mut u64) -> Result<String, String> {
    let path = dir.join(rel);
    if let Some(parent) = path.parent().filter(|p| *p != dir)
        && std::fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(format!("{rel}: Ordner ist eine symbolische Verknüpfung; nichts gelesen"));
    }
    let meta = std::fs::symlink_metadata(&path).map_err(|_| format!("{rel} fehlt im Archivordner"))?;
    if meta.file_type().is_symlink() {
        return Err(format!("{rel} ist eine symbolische Verknüpfung; nichts gelesen"));
    }
    if !meta.is_file() {
        return Err(format!("{rel} ist keine Datei"));
    }
    if meta.len() > *budget {
        return Err("Der Archivordner ist größer als 16 MiB (Grenze wie beim Import); nichts wiederhergestellt.".into());
    }
    *budget -= meta.len();
    let bytes = std::fs::read(&path).map_err(|e| format!("{rel} lässt sich nicht lesen: {e}"))?;
    String::from_utf8(bytes).map_err(|_| format!("{rel} ist kein UTF-8-Text"))
}

/// Typst-Text eines Archivordners (`folder` relativ zum Datenordner, aus [`archive_folder`]) wie ein Export
/// zusammensetzen: `main.typ` Zeile für Zeile, jede Zeile `#include "abschnitte/…"` durch den Text der Datei ersetzt
/// (ohne die Import-Zeile der Werkvorlage), ohne Werkvorlage die Kopfzeile `// Baruch: <Titel>` weggelassen; danach
/// Buchende und Projektion wie beim Import entfernt. Pfadschutz: nur Ordner direkt unter `works/.geloescht/`, keine
/// symbolischen Verknüpfungen (Ordner, `abschnitte/`, Dateien), nur `abschnitte/<name>.typ`, zusammen höchstens
/// [`MAX_SOURCE_BYTES`], höchstens [`MAX_ARCHIVE_SECTIONS`] Abschnitte. Blockiert (nur in `spawn_blocking`).
pub fn read_archive(data: &FsPath, folder: &str) -> Result<Archived, String> {
    let prefix = format!("works/{TRASH_DIR}/");
    let name = folder.strip_prefix(&prefix).unwrap_or("");
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(format!("ungültiger Archivordner {folder:?}"));
    }
    let works = data.join("works");
    let trash = works.join(TRASH_DIR);
    let dir = trash.join(name);
    for path in [&works, &trash, &dir] {
        let meta = std::fs::symlink_metadata(path).map_err(|_| format!("{} fehlt", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("{} ist eine symbolische Verknüpfung; nichts gelesen", path.display()));
        }
        if !meta.is_dir() {
            return Err(format!("{} ist kein Ordner", path.display()));
        }
    }
    let mut budget = MAX_SOURCE_BYTES;
    let main = read_plain(&dir, "main.typ", &mut budget)?;
    let has_template = match std::fs::symlink_metadata(dir.join("vorlage.typ")) {
        Ok(m) if m.file_type().is_symlink() => return Err("vorlage.typ ist eine symbolische Verknüpfung".into()),
        Ok(m) => m.is_file(),
        Err(_) => false,
    };
    let mut out = String::with_capacity(main.len());
    let mut sections = 0usize;
    let mut dropped_head = false;
    for (index, line) in main.split_inclusive('\n').enumerate() {
        let bare = line.trim_end_matches(['\n', '\r']);
        if index == 0 && !has_template && bare.starts_with("// Baruch: ") {
            dropped_head = true;
            continue;
        }
        // Ohne Werkvorlage folgt der Kopfzeile eine Leerzeile (leere Vorlage); sie gehört zu keinem Abschnitt.
        if index == 1 && dropped_head && bare.is_empty() {
            continue;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        match include_path(bare) {
            Some(path) => {
                if !section_path_ok(path) {
                    return Err(format!("main.typ bindet {path:?} ein; erlaubt sind nur abschnitte/<name>.typ"));
                }
                sections += 1;
                if sections > MAX_ARCHIVE_SECTIONS {
                    return Err(format!("mehr als {MAX_ARCHIVE_SECTIONS} Abschnittsdateien; nichts wiederhergestellt"));
                }
                let text = read_plain(&dir, path, &mut budget)?;
                out.push_str(text.strip_prefix(ARCHIVE_TEMPLATE_IMPORT).unwrap_or(&text));
            }
            None => out.push_str(line),
        }
    }
    let (cut, exported) = baruch_typst::index::strip_back_matter(&out);
    let source = if exported {
        baruch_typst::index::strip_projection(cut).into_owned()
    } else {
        baruch_typst::index::strip_first_mentions(cut).into_owned()
    };
    let mut images: Vec<String> = Vec::new();
    let pictures = dir.join("bilder");
    if std::fs::symlink_metadata(&pictures).is_ok_and(|m| m.is_dir())
        && let Ok(entries) = std::fs::read_dir(&pictures)
    {
        images = entries.flatten().filter_map(|e| e.file_name().to_str().map(str::to_owned)).collect();
        images.sort();
    }
    Ok(Archived { source, images })
}

/// Archivliste: alle gelöschten Werke (neueste zuerst) mit Archivordner und ob sie sich wiederherstellen lassen.
pub async fn archives(state: &AppState) -> Result<Vec<ArchiveEntry>, RpcError> {
    let retired = state.store.retired_works().await?;
    let live: Vec<String> = state.store.work_list().await?.iter().map(|w| title_key(&w.title)).collect();
    let data = state.data.clone();
    let slugs: Vec<String> = retired.iter().map(|r| r.slug.clone()).collect();
    let folders = tokio::task::spawn_blocking(move || {
        slugs
            .iter()
            .map(|slug| {
                let folder = archive_folder(&data, slug)?;
                let main = folder.as_ref().is_some_and(|f| {
                    std::fs::symlink_metadata(data.join(f).join("main.typ")).is_ok_and(|m| m.is_file())
                });
                Ok::<_, String>((folder, main))
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| RpcError::new(INTERNAL, format!("Archiv lesen abgebrochen: {e}")))?;
    Ok(retired
        .into_iter()
        .zip(folders)
        .map(|(r, found)| {
            let (folder, reason) = match found {
                Err(e) => (None, Some(e)),
                Ok((None, _)) => (None, Some(format!("Kein Archivordner works/{TRASH_DIR}/{}-… gefunden.", r.slug))),
                Ok((Some(f), false)) => (Some(f), Some("Im Archivordner fehlt main.typ.".to_owned())),
                Ok((Some(f), true)) if live.contains(&title_key(&r.title)) => {
                    (Some(f), Some("Ein lebendes Werk trägt denselben Titel; erst jenes umbenennen.".to_owned()))
                }
                Ok((Some(f), true)) => (Some(f), None),
            };
            ArchiveEntry {
                restorable: reason.is_none(),
                slug: r.slug,
                title: r.title,
                deleted: r.deleted,
                folder,
                reason,
            }
        })
        .collect())
}

fn refused(message: impl Into<String>) -> RpcError {
    RpcError::new(REFUSED, message)
}

/// Archiviertes Werk wiederherstellen (nur der Mensch, siehe Moduldoku): unter der Schreibsperre Archivordner lesen,
/// im Store anlegen, Ordner zurück nach `works/<slug>`, ein Commit „Werk wiederhergestellt: <Titel>“. Gibt es
/// `works/<slug>` schon, bleibt alles, wie es ist (`REFUSED`). Ein Fehler beim Verschieben oder im Spiegel hält das
/// Werk in der Datenbank nicht auf (`mirror`).
pub async fn restore(state: &AppState, slug: &str) -> Result<serde_json::Value, RpcError> {
    let held = state.write_lock.lock().await;
    if !state.store.retired_works().await?.iter().any(|r| r.slug == slug) {
        return Err(refused(format!("„{slug}“ steht nicht im Archiv der gelöschten Werke; nichts wiederhergestellt.")));
    }
    let data = state.data.clone();
    let wanted = slug.to_owned();
    let (folder, archived) = tokio::task::spawn_blocking(move || {
        let folder = archive_folder(&data, &wanted)?.ok_or_else(|| {
            format!("Kein Archivordner works/{TRASH_DIR}/{wanted}-… gefunden; nichts wiederhergestellt.")
        })?;
        let archived = read_archive(&data, &folder)?;
        let target = mirror_dir(&data, &wanted)?;
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(format!("works/{wanted} gibt es schon; nichts wiederhergestellt."));
        }
        Ok::<_, String>((folder, archived))
    })
    .await
    .map_err(|e| RpcError::new(INTERNAL, format!("Archiv lesen abgebrochen: {e}")))?
    .map_err(refused)?;
    let restored = state.store.restore_retired(slug, &archived.source, HUMAN_NAME).await?;
    crate::rag::notify(state, Some(restored.work));
    let mut errors: Vec<String> = Vec::new();
    let data = state.data.clone();
    let (from, to_slug) = (folder.clone(), slug.to_owned());
    let moved = tokio::task::spawn_blocking(move || {
        let target = mirror_dir(&data, &to_slug)?;
        std::fs::rename(data.join(&from), &target)
            .map_err(|e| format!("{from} lässt sich nicht nach works/{to_slug} verschieben: {e}"))
    })
    .await;
    match moved {
        Ok(Ok(())) => {}
        Ok(Err(e)) => errors.push(e),
        Err(e) => errors.push(format!("Verschieben abgebrochen: {e}")),
    }
    state.forget_mirror(restored.work);
    let mut commit = None;
    match prepare_mirror(state, restored.work).await {
        Ok((mirror, files)) => {
            let mut body = format!(
                "Aus works/{TRASH_DIR}/ zurückgeholt: {} Gliederungspunkte, aktuelle Texte als Fassung 1 \
                 („aus dem Archiv wiederhergestellt“).\n",
                restored.nodes
            );
            if !archived.images.is_empty() {
                body.push_str(&format!("Nicht wiederhergestellt: {} Bild(er) aus bilder/.\n", archived.images.len()));
            }
            let request = CommitRequest {
                files,
                author: human(),
                committer: human(),
                summary: format!("Werk wiederhergestellt: {}", restored.title),
                body,
                trailers: vec![("Baruch-Action".to_owned(), "work-restored".to_owned())],
                allow_empty: true,
            };
            match tokio::task::spawn_blocking(move || mirror.commit(&request)).await {
                Ok(Ok(id)) => commit = id,
                Ok(Err(e)) => errors.push(e.to_string()),
                Err(e) => errors.push(format!("Git: {e}")),
            }
        }
        Err(e) => errors.push(e),
    }
    let _ = &held;
    let mut answer = json!({
        "work": restored.work, "slug": restored.slug, "title": restored.title, "nodes": restored.nodes,
        "folder": folder, "skipped_images": archived.images,
    });
    if let Some(commit) = commit {
        answer["commit"] = json!(commit);
    }
    if !errors.is_empty() {
        let error = errors.join("; ");
        eprintln!("Git-Spiegel (Werk {} wiederhergestellt): {error}", restored.work);
        answer["mirror"] = json!(error);
    }
    Ok(answer)
}

// ---- Vorlagen ----------------------------------------------------------------------------------

/// Eigene Template-Umgebung der Werkverwaltung (die der Arbeitsseite bleibt unberührt).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("works.html", include_str!("../templates/works.html")),
            ("work_new.html", include_str!("../templates/work_new.html")),
            ("work_edit.html", include_str!("../templates/work_edit.html")),
            ("work_delete.html", include_str!("../templates/work_delete.html")),
            ("works_archive.html", include_str!("../templates/works_archive.html")),
            ("fragments/work_new.html", include_str!("../templates/fragments/work_new.html")),
            ("import_report.html", include_str!("../templates/import_report.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Werkverwaltung fehlerhaft");
        }
        env
    })
}

fn page_settings(headers: &HeaderMap) -> Value {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    Value::from(Serde(settings))
}

fn render(status: StatusCode, name: &str, ctx: Value) -> Response {
    match templates().get_template(name).and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage {name}: {e}")).into_response(),
    }
}

/// HTTP-Status zu einem RPC-Fehler: ungültig 422, abgelehnt 409, sonst 500.
fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn kind_label(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Book => "Buch",
        WorkKind::Sermon => "Predigt",
    }
}

pub(crate) fn kind_word(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Book => "book",
        WorkKind::Sermon => "sermon",
    }
}

/// Zeitstempel `2026-11-05T10:11:12.123Z` (UTC) → `2026-11-05 11:11` in Ortszeit.
fn short_time(time: &str) -> String {
    crate::localtime::minutes(time)
}

/// Gibt es ein Werk, zu dem „Zurück“ führen kann?
async fn back_url(state: &AppState) -> Option<String> {
    state.default_work().await.ok().map(|work| format!("/?work={work}"))
}

// ---- Seiten ------------------------------------------------------------------------------------

/// Seiten der Werkverwaltung (nur lesen).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke", get(list_page))
        .route("/werke/neu", get(new_page))
        .route("/werke/{id}/bearbeiten", get(edit_page))
        .route("/werke/{id}/loeschen", get(delete_page))
        .route("/werke/{id}/export", get(export))
        .route("/werke/archiv", get(archive_page))
}

/// Formulare (schreibend, im Router hinter `guard::same_origin`). Nur `POST /werke/import` nimmt
/// große Anfragen an (seit Runde 21b [`IMPORT_UPLOAD_LIMIT`] = 80 MiB + 64 KiB für ZIP und Ordner).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/import", post(upload).layer(DefaultBodyLimit::max(IMPORT_UPLOAD_LIMIT)))
        .route("/werke/act", post(act))
        .route("/werke/archiv/wiederherstellen", post(restore_form))
}

#[derive(Deserialize)]
struct ListQuery {
    geloescht: Option<String>,
}

async fn list_page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<ListQuery>) -> Response {
    // Runde 35: gefiltert nach Rolle (ein Lektor sieht nur freigegebene Werke).
    let works = match crate::roles::visible_works(&state, &crate::auth::current()).await {
        Ok(works) => works,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let mut rows: Vec<Value> = Vec::with_capacity(works.len());
    for w in &works {
        // Runde 11: Umschlag je Werk (Bildname; das Vorschaubild kommt über /vorschau?b=320).
        let cover = match crate::cover::cover_info(&state, w.id).await {
            Ok(cover) => cover.map(|c| Value::from(Serde(c))),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
        };
        rows.push(context! {
            id => w.id, slug => w.slug.as_str(), title => w.title.as_str(), subtitle => w.subtitle.as_str(),
            author => w.author.as_str(), kind => kind_label(w.kind), sections => w.sections,
            proposals => w.proposals, updated => w.updated.as_deref().map(short_time),
            created => short_time(&w.created), cover => cover,
        });
    }
    let notice = q.geloescht.as_deref().filter(|t| !t.trim().is_empty()).map(|t| {
        format!("Das Werk „{}“ ist gelöscht; sein Git-Spiegel liegt jetzt unter works/{TRASH_DIR}/.", t.trim())
    });
    let ctx = context! {
        settings => page_settings(&headers), works => rows, notice => notice, back_url => back_url(&state).await,
        // Runde 36: Für Lektoren ist die Seite des Umschlags gesperrt – das kleine Bild bleibt, der Link entfällt
        // (nur Bequemlichkeit; die Sperre selbst liegt in `roles`).
        lektor => crate::auth::current().lektor().is_some(),
    };
    render(StatusCode::OK, "works.html", ctx)
}

/// Werte eines Anlege- oder Upload-Formulars (zum erneuten Zeigen nach einem Fehler).
#[derive(Debug, Default, Clone, serde::Serialize)]
struct NewForm {
    title: String,
    author: String,
    kind: String,
    preset: String,
    subtitle: String,
    audience: String,
    bible: String,
    upload_title: String,
    text_ref: String,
    module: String,
    from_work: Option<WorkId>,
}

/// SWORD-Bibeln für die Auswahl auf `/werke/neu` (nur lesend, `bibles::scan`): Name, Beschreibung, nutzbar.
/// Ein Fehler beim Lesen der Modulordner lässt die Liste leer (die Seite bleibt bedienbar, das Textfeld
/// fällt dann auf die Bibel des Vorgabewerks zurück). Gelesen wird wie in der Literatur nur bei eingestellten
/// Modulordnern oder gewählter Bibel des Vorgabewerks (`literature::bibles::scan_allowed`), damit die Seite ohne
/// Einstellung – und kein Test – die Vorgabeordner des Rechners liest.
async fn sword_bibles(state: &AppState) -> Vec<Value> {
    let allowed = match state.default_work().await {
        Ok(work) => crate::literature::bibles::scan_allowed(state, work).await,
        Err(_) => matches!(state.store.bible_dirs().await, Ok(Some(_))),
    };
    if !allowed {
        return Vec::new();
    }
    match crate::bibles::scan(state).await {
        Ok((found, ..)) => found
            .modules
            .iter()
            .map(|m| context! { name => m.name.as_str(), description => m.description.as_str(), usable => m.problem.is_none() })
            .collect(),
        Err(_) => Vec::new(),
    }
}

async fn new_page_response(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    form: NewForm,
    error: Option<String>,
) -> Response {
    let ctx = context! {
        settings => page_settings(headers), form => Value::from(Serde(form)), error => error,
        back_url => back_url(state).await, max_mib => MAX_SOURCE_BYTES / (1024 * 1024),
        bible_modules => sword_bibles(state).await,
    };
    render(status, "work_new.html", ctx)
}

async fn new_page(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    new_page_response(&state, &headers, StatusCode::OK, NewForm::default(), None).await
}

#[derive(Deserialize)]
struct EditQuery {
    /// Fassung der Werkvorlage, die gezeigt werden soll.
    vorlage: Option<String>,
}

/// Rücksprung eines Formulars in die Einstellungen (Runde 9, Feld `back`): nur
/// `/einstellungen/werk`, höchstens mit einer Abfrage aus `[A-Za-z0-9=&_%.-]`; alles andere → `None`
/// (dann gilt die bisherige Weiterleitung). So führt `back` nie auf eine fremde Seite.
pub(crate) fn settings_back(back: Option<&str>) -> Option<String> {
    let back = back?.trim();
    let rest = back.strip_prefix("/einstellungen/werk")?;
    let query_ok = |q: &str| q.bytes().all(|b| b.is_ascii_alphanumeric() || b"=&_%.-".contains(&b));
    match rest.strip_prefix('?') {
        _ if rest.is_empty() => Some(back.to_owned()),
        Some(query) if query_ok(query) => Some(back.to_owned()),
        _ => None,
    }
}

/// `back` mit angehängtem `ok=<code>` (Hinweis auf der Einstellungsseite).
pub(crate) fn with_ok(back: &str, code: &str) -> String {
    format!("{back}{}ok={code}", if back.contains('?') { '&' } else { '?' })
}

/// Kontext des Formularteils von `work_edit.html` (`{% block content %}`): Werk, Formularwerte (nach
/// einem Fehler die eingegebenen), Fassungen der Werkvorlage, ältere Fassung zum Lesen, Fehler und
/// `back` (Rücksprung in die Einstellungen, Runde 9). Dieselbe Funktion baut die Seite
/// `/werke/{id}/bearbeiten` und die Einstellungsseite „Werk“ (`crate::settings`).
pub(crate) async fn edit_context(
    state: &AppState,
    work: WorkId,
    shown: Option<u32>,
    form: Option<UpdateParams>,
    error: Option<String>,
    back: Option<&str>,
) -> Result<Value, StoreError> {
    let meta = state.store.work_meta(work).await?;
    let revisions = state.store.template_revisions(work).await?;
    let old = match shown.filter(|&n| n != meta.template_rev) {
        Some(number) => {
            state.store.template_revision(work, number).await?.map(|body| context! { number => number, body => body })
        }
        None => None,
    };
    // Nach einem Fehler die eingegebenen Werte zeigen, sonst die gespeicherten.
    let form = form.unwrap_or_default();
    let pick = |typed: &Option<String>, saved: &str| typed.clone().unwrap_or_else(|| saved.to_owned());
    let values = context! {
        title => pick(&form.title, &meta.title), subtitle => pick(&form.subtitle, &meta.subtitle),
        author => pick(&form.author, &meta.author), audience => pick(&form.audience, &meta.audience),
        bible => pick(&form.bible, &meta.bible), template => pick(&form.template, &meta.template),
        kind => pick(&form.kind, kind_word(meta.kind)), note => form.note.clone().unwrap_or_default(),
    };
    let list: Vec<Value> = revisions
        .iter()
        .rev()
        .map(|r| {
            context! {
                number => r.number, author => r.author.as_str(), note => r.note.as_str(), created => short_time(&r.created),
                current => r.number == meta.template_rev, shown => old.is_some() && shown == Some(r.number),
            }
        })
        .collect();
    Ok(context! {
        work => Value::from(Serde(&meta)), values => values,
        kind_label => kind_label(meta.kind), revisions => list, old => old, error => error,
        back_url => format!("/?work={work}"), back => back,
    })
}

#[allow(clippy::too_many_arguments)]
async fn edit_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    status: StatusCode,
    shown: Option<u32>,
    form: Option<UpdateParams>,
    error: Option<String>,
    back: Option<&str>,
) -> Response {
    let ctx = match edit_context(state, work, shown, form, error, back).await {
        Ok(ctx) => ctx,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    render(status, "work_edit.html", context! { settings => page_settings(headers), ..ctx })
}

async fn edit_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(q): Query<EditQuery>,
) -> Response {
    let shown = q.vorlage.as_deref().and_then(|v| v.trim().parse().ok());
    edit_response(&state, &headers, id, StatusCode::OK, shown, None, None, None).await
}

async fn delete_response(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    status: StatusCode,
    error: Option<String>,
) -> Response {
    let meta = match state.store.work_meta(work).await {
        Ok(meta) => meta,
        Err(StoreError::UnknownWork(_)) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let summary = state.store.work_list().await.ok().and_then(|list| list.into_iter().find(|w| w.id == work));
    let ctx = context! {
        settings => page_settings(headers), work => Value::from(Serde(&meta)), error => error,
        sections => summary.as_ref().map(|s| s.sections), proposals => summary.as_ref().map(|s| s.proposals),
        trash => TRASH_DIR, back_url => format!("/?work={work}"),
    };
    render(status, "work_delete.html", ctx)
}

async fn delete_page(State(state): State<Arc<AppState>>, headers: HeaderMap, Path(id): Path<WorkId>) -> Response {
    delete_response(&state, &headers, id, StatusCode::OK, None).await
}

/// `GET /werke/{id}/export`: das Werk als eine Typst-Datei zum Herunterladen (`<slug>.typ`).
async fn export(State(state): State<Arc<AppState>>, Path(id): Path<WorkId>) -> Response {
    let (slug, typst) = match (state.store.work_slug(id).await, state.store.export_typst(id).await) {
        (Ok(slug), Ok(typst)) => (slug, typst),
        (Err(StoreError::UnknownWork(_)), _) | (_, Err(StoreError::UnknownWork(_))) => {
            return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response();
        }
        (Err(e), _) | (_, Err(e)) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response();
        }
    };
    // Der slug besteht nur aus [a-z0-9-] und darf darum ohne Maskierung in die Kopfzeile.
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{slug}.typ\""))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment"));
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8")),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        typst,
    )
        .into_response()
}

// ---- Archiv: Seite und Formular (Runde 22) -----------------------------------------------------

async fn archive_response(
    state: &AppState,
    headers: &HeaderMap,
    status: StatusCode,
    error: Option<String>,
) -> Response {
    let list = match archives(state).await {
        Ok(list) => list,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response(),
    };
    let rows: Vec<Value> = list
        .iter()
        .map(|a| {
            context! {
                slug => a.slug.as_str(), title => a.title.as_str(), deleted => short_time(&a.deleted),
                folder => a.folder.as_deref(), restorable => a.restorable, reason => a.reason.as_deref(),
            }
        })
        .collect();
    let ctx = context! {
        settings => page_settings(headers), archives => rows, error => error, trash => TRASH_DIR,
        back_url => back_url(state).await,
    };
    render(status, "works_archive.html", ctx)
}

/// `GET /werke/archiv`: gelöschte Werke mit Archivordner und Formular „Wiederherstellen“ (ohne JavaScript).
async fn archive_page(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    archive_response(&state, &headers, StatusCode::OK, None).await
}

#[derive(Debug, Default, Deserialize)]
struct RestoreForm {
    slug: Option<String>,
    confirm: Option<String>,
}

/// `POST /werke/archiv/wiederherstellen` (Felder `slug`, `confirm`): ohne Bestätigung 422, Fehler 409/422 mit der
/// Archivseite und Meldung; Erfolg → 303 auf das wiederhergestellte Werk.
async fn restore_form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(form): Form<RestoreForm>,
) -> Response {
    let slug = form.slug.as_deref().map(str::trim).unwrap_or("");
    if slug.is_empty() {
        let message = "Welches Werk? Der Ordnername fehlt.".to_owned();
        return archive_response(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, Some(message)).await;
    }
    if !matches!(form.confirm.as_deref().map(str::trim), Some("1" | "on" | "true" | "ja")) {
        let message = "Bitte das Wiederherstellen mit dem Kästchen bestätigen; nichts wiederhergestellt.".to_owned();
        return archive_response(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, Some(message)).await;
    }
    match restore(&state, slug).await {
        Ok(answer) => Redirect::to(&format!("/?work={}", answer["work"])).into_response(),
        Err(e) => archive_response(&state, &headers, status_of(&e), Some(e.message)).await,
    }
}

// ---- Formulare ---------------------------------------------------------------------------------

/// `POST /werke/import` (multipart): Felder `file` (eine Datei: `.typ`, `.md`, `.markdown` oder `.zip`), `files`
/// (mehrere Dateien mit relativen Pfaden, Ordner-Upload, Runde 21b) und `title` (leer = aus der Datei).
/// Erfolg ohne Hinweise → 303 auf `/?work=<id>`; mit Hinweisen oder übersprungenen Bildern → 200 mit Bericht
/// (`import_report.html`). Falsche Endung, kein UTF-8, unsicheres oder kaputtes Archiv → 422, zu groß → 413, schon
/// importiert oder Titel vergeben → 409; die Fehlerseite zeigt das Formular wieder, angelegt ist dann nichts.
async fn upload(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let mut form = NewForm::default();
    let failed = |status: StatusCode, form: NewForm, message: String| {
        let state = state.clone();
        let headers = headers.clone();
        async move { new_page_response(&state, &headers, status, form, Some(message)).await }
    };
    let too_large = || "Die Anfrage ist größer als 80 MiB (Grenze für ZIP und Ordner).".to_owned();
    let mut multipart = match multipart {
        Ok(multipart) => multipart,
        Err(e) => {
            let message = format!("Das Formular kam nicht als Datei-Upload an ({}).", e.body_text());
            return failed(StatusCode::UNPROCESSABLE_ENTITY, form, message).await;
        }
    };
    let mut file: Option<(String, Vec<u8>)> = None;
    let mut folder: Vec<(String, Vec<u8>)> = Vec::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                let status = e.status();
                let (status, message) = if status == StatusCode::PAYLOAD_TOO_LARGE {
                    (status, too_large())
                } else {
                    (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        format!("Das Formular ließ sich nicht lesen ({}).", e.body_text()),
                    )
                };
                return failed(status, form, message).await;
            }
        };
        let name = field.name().map(str::to_owned);
        match name.as_deref() {
            Some(field_name @ ("file" | "files")) => {
                let file_name = field.file_name().unwrap_or("").to_owned();
                let bytes = match field.bytes().await {
                    Ok(bytes) => bytes.to_vec(),
                    Err(e) => {
                        let status = e.status();
                        let (status, message) = if status == StatusCode::PAYLOAD_TOO_LARGE {
                            (status, too_large())
                        } else {
                            (
                                StatusCode::UNPROCESSABLE_ENTITY,
                                format!("Die Datei ließ sich nicht lesen ({}).", e.body_text()),
                            )
                        };
                        return failed(status, form, message).await;
                    }
                };
                // Ein Browser schickt für ein leeres Dateifeld einen Teil ohne Namen: kein Upload.
                if file_name.trim().is_empty() {
                    continue;
                }
                if field_name == "file" {
                    file = Some((file_name, bytes));
                } else {
                    folder.push((file_name, bytes));
                }
            }
            Some("title") => form.upload_title = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }
    let title = form.upload_title.trim().to_owned();
    let title = Some(title.as_str()).filter(|t| !t.is_empty());
    let result = match (file, folder.is_empty()) {
        (Some(_), false) => {
            let message = "Bitte entweder eine Datei oder einen Ordner wählen, nicht beides.".to_owned();
            return failed(StatusCode::UNPROCESSABLE_ENTITY, form, message).await;
        }
        (None, true) => {
            let message = "Bitte eine Datei (.typ, .md, .markdown, .zip) oder einen Ordner wählen.".to_owned();
            return failed(StatusCode::UNPROCESSABLE_ENTITY, form, message).await;
        }
        (None, false) => {
            // Ordner: Browser schicken den relativen Pfad als Dateinamen („buch/kapitel.md“); der Ordnername ist der
            // erste Teil, sonst gibt es keinen.
            let origin = folder[0].0.split('/').next().filter(|_| folder[0].0.contains('/')).unwrap_or("").to_owned();
            match Bundle::from_files(folder) {
                Ok(bundle) => markdown_import::import_bundle(&state, bundle, &origin, title).await,
                Err(e) => {
                    let status = archive_status(&e);
                    return failed(status, form, format!("Der Ordner lässt sich nicht importieren: {e}.")).await;
                }
            }
        }
        (Some((file_name, bytes)), true) => {
            let base = file_name.rsplit(['/', '\\']).next().unwrap_or("").to_owned();
            if markdown_import::is_archive_name(&base) {
                match Bundle::from_archive(&bytes) {
                    Ok(bundle) => markdown_import::import_bundle(&state, bundle, &base, title).await,
                    Err(e) => {
                        let status = archive_status(&e);
                        return failed(status, form, format!("„{base}“ lässt sich nicht importieren: {e}.")).await;
                    }
                }
            } else {
                // Runde 32: Word-Dateien liest nur der eigene Import mit Vorschau.
                if crate::docx_import::is_docx_name(&base) {
                    return failed(StatusCode::UNPROCESSABLE_ENTITY, form, USE_WORD_IMPORT.into()).await;
                }
                let markdown = markdown_import::is_markdown_name(&base);
                if !markdown && !import::is_typst_name(&base) {
                    let message = format!(
                        "„{base}“ lässt sich nicht importieren; nur .typ lässt sich importieren, dazu Markdown (.md, \
                         .markdown) und ZIP-Archive (.zip)."
                    );
                    return failed(StatusCode::UNPROCESSABLE_ENTITY, form, message).await;
                }
                if bytes.len() as u64 > MAX_SOURCE_BYTES {
                    return failed(StatusCode::PAYLOAD_TOO_LARGE, form, "Die Datei ist größer als 16 MiB.".into())
                        .await;
                }
                let Ok(source) = String::from_utf8(bytes) else {
                    return failed(StatusCode::UNPROCESSABLE_ENTITY, form, format!("„{base}“ ist kein UTF-8-Text."))
                        .await;
                };
                if markdown {
                    markdown_import::import_text(&state, &source, &base, title).await
                } else {
                    import::import_upload(&state, &source, &base, title).await
                }
            }
        }
    };
    match result {
        Ok(report) if report.warnings.is_empty() && report.skipped.is_empty() => {
            Redirect::to(&format!("/?work={}", report.work)).into_response()
        }
        Ok(report) => report_page(&headers, &report),
        Err(e) => {
            let mut message = e.message.clone();
            if e.code == REFUSED && message.starts_with("Es gibt schon ein Werk") {
                message.push_str(" Bitte unten einen anderen Titel eintragen und die Datei noch einmal wählen.");
            }
            failed(status_of(&e), form, message).await
        }
    }
}

/// HTTP-Status zu einem Fehler beim Lesen eines Archivs bzw. Ordners: Grenze überschritten → 413, sonst 422.
fn archive_status(e: &crate::archive::ArchiveError) -> StatusCode {
    match e {
        crate::archive::ArchiveError::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

/// Bericht eines Imports mit Hinweisen (200): Werk, angelegte Bilder, übersprungene Bilder mit Grund, Hinweise mit Zeile.
fn report_page(headers: &HeaderMap, report: &ImportReport) -> Response {
    let ctx = context! {
        settings => page_settings(headers), report => Value::from(Serde(report)),
        work_url => format!("/?work={}", report.work),
    };
    render(StatusCode::OK, "import_report.html", ctx)
}

/// Formular ohne JavaScript: `action` = `create`, `update` oder `delete`, Felder wie RPC.
#[derive(Debug, Default, Deserialize)]
struct ActForm {
    action: String,
    work: Option<String>,
    title: Option<String>,
    author: Option<String>,
    kind: Option<String>,
    preset: Option<String>,
    subtitle: Option<String>,
    audience: Option<String>,
    bible: Option<String>,
    template: Option<String>,
    note: Option<String>,
    confirm_title: Option<String>,
    /// Rücksprung nach Erfolg (nur `/einstellungen/werk…`, sonst übergangen; Runde 9).
    back: Option<String>,
    /// Predigttext, SWORD-Bibel und Ausgangswerk (Runde 16a, nur `create`).
    text_ref: Option<String>,
    module: Option<String>,
    from_work: Option<String>,
}

async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work: Option<WorkId> =
        form.work.as_deref().map(str::trim).filter(|w| !w.is_empty()).and_then(|w| w.parse().ok());
    match form.action.as_str() {
        "create" => {
            let p = CreateParams {
                title: form.title.clone().unwrap_or_default(),
                author: form.author.clone(),
                kind: form.kind.clone(),
                preset: form.preset.clone(),
                subtitle: form.subtitle.clone(),
                audience: form.audience.clone(),
                bible: form.bible.clone(),
                text_ref: form.text_ref.clone(),
                module: form.module.clone(),
                from_work: form.from_work.as_deref().map(str::trim).and_then(|w| w.parse().ok()),
            };
            match create(&state, &p).await {
                Ok(answer) => Redirect::to(&format!("/?work={}", answer["work"])).into_response(),
                Err(e) => {
                    let values = NewForm {
                        title: p.title,
                        author: p.author.unwrap_or_default(),
                        kind: p.kind.unwrap_or_default(),
                        preset: p.preset.unwrap_or_default(),
                        subtitle: p.subtitle.unwrap_or_default(),
                        audience: p.audience.unwrap_or_default(),
                        bible: p.bible.unwrap_or_default(),
                        upload_title: String::new(),
                        text_ref: p.text_ref.unwrap_or_default(),
                        module: p.module.unwrap_or_default(),
                        from_work: p.from_work,
                    };
                    new_page_response(&state, &headers, status_of(&e), values, Some(e.message)).await
                }
            }
        }
        "update" => {
            let Some(work) = work else {
                return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Werk fehlt.").into_response();
            };
            let p = UpdateParams {
                work: Some(work),
                title: form.title,
                subtitle: form.subtitle,
                author: form.author,
                kind: form.kind,
                audience: form.audience,
                bible: form.bible,
                template: form.template,
                note: form.note,
            };
            let back = settings_back(form.back.as_deref());
            match update(&state, &p).await {
                Ok(_) => match &back {
                    Some(back) => Redirect::to(&with_ok(back, "werk")).into_response(),
                    None => Redirect::to(&format!("/?work={work}")).into_response(),
                },
                Err(e) => {
                    let status = status_of(&e);
                    if back.is_some() {
                        // Aus den Einstellungen: dieselbe Einstellungsseite mit Fehler und Eingabe (das
                        // Popup übernimmt daraus den Rahmen, ohne JavaScript steht die ganze Seite da).
                        let retry = crate::settings::WorkRetry::Edit { form: p, error: e.message };
                        return crate::settings::work_retry_page(&state, &headers, status, work, retry).await;
                    }
                    edit_response(&state, &headers, work, status, None, Some(p), Some(e.message), None).await
                }
            }
        }
        "delete" => {
            let Some(work) = work else {
                return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: Werk fehlt.").into_response();
            };
            let confirm = form.confirm_title.unwrap_or_default();
            match delete(&state, work, &confirm).await {
                Ok(answer) => {
                    let title = answer["title"].as_str().unwrap_or("");
                    Redirect::to(&format!("/werke?geloescht={}", crate::view::query_escape(title))).into_response()
                }
                Err(e) if e.code == INTERNAL => {
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response()
                }
                Err(e) => delete_response(&state, &headers, work, status_of(&e), Some(e.message)).await,
            }
        }
        _ => (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_is_a_calendar_date() {
        let day = today();
        assert!(
            day.len() == 10 && day.as_bytes()[4] == b'-' && day.as_bytes()[7] == b'-',
            "JJJJ-MM-TT für den Ordnernamen im Papierkorb: {day}"
        );
    }

    #[test]
    fn only_includes_of_section_files_are_read_from_an_archive() {
        assert!(section_path_ok("abschnitte/001-anfang.typ"), "Abschnittsdatei des Spiegels");
        for bad in
            ["../main.typ", "abschnitte/../x.typ", "abschnitte/a/b.typ", "abschnitte/.typ", "vorlage.typ", "/etc/x.typ"]
        {
            assert!(!section_path_ok(bad), "{bad}: außerhalb von abschnitte/ bzw. ohne Namen – Pfadschutz");
        }
        assert_eq!(include_path(r#"#include "abschnitte/001-a.typ""#), Some("abschnitte/001-a.typ"));
        assert!(is_date("2026-11-10") && !is_date("2-2026-11-"), "nur JJJJ-MM-TT ist ein Datum");
    }

    #[test]
    fn the_commit_subject_names_the_changed_fields_in_german() {
        assert_eq!(
            update_summary(&["title", "author", "template"], Some(3)),
            "Werk geändert: Titel, Autor, Werkvorlage Fassung 3",
            "Felder auf Deutsch, Vorlage mit Fassung"
        );
    }

    #[test]
    fn only_methods_of_this_module_are_handled_here() {
        for method in [
            "work.list",
            "work.meta",
            "work.create",
            "work.import",
            "work.import_archive",
            "work.update",
            "work.delete",
            "work.archives",
            "work.restore",
        ] {
            assert!(handles(method), "{method} gehört zur Werkverwaltung");
        }
        for method in ["work.get", "work.export", "node.get"] {
            assert!(!handles(method), "{method} bleibt in rpc.rs");
        }
    }
}
