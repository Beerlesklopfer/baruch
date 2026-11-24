// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Vorschau mit Werkvorlage (Vertrag Runde 8, Abschnitt 4c): das ganze Werk mit dem
//! eingebetteten Typst-Compiler ([`baruch_render`]) zu SVG-Seiten gesetzt.
//!
//! - **Quelle:** [`baruch_store::Store::export_map`] (byte-gleich mit `work.export`, also mit
//!   Werkvorlage und Gliederungsschema) und die Bilder aus `Store::export_assets` als virtuelle
//!   Dateien. Die Karte führt jede Meldung über ihre Zeile auf den Abschnitt zurück (fester
//!   Schlüssel, „2.1 Titel“, Zeile im Abschnitt) bzw. auf „Werkvorlage“.
//! - **Lauf:** nur in `spawn_blocking`, höchstens einer gleichzeitig ([`RUNS`]), mit Zeitlimit
//!   [`COMPILE_TIMEOUT`]; danach antwortet der Server mit `REFUSED` „Typst brauchte länger als 60 s“.
//!   Nach dem Zeitlimit rechnet der Lauf im Hintergrund zu Ende, hält so lange die Sperre und legt sein
//!   Ergebnis noch in den Zwischenspeicher.
//! - **Abbruch (Runde 18a):** Jeder Lauf für ein Werk (Satz-Seite, SVG-Seite, `preview.typst`, `typst_check`)
//!   steht mit seinem [`baruch_render::Cancel`] in einer prozessweiten Liste ([`cancel`], [`cancel_in`]); RPC
//!   `preview.cancel` und das Formular `POST /werke/{id}/satz/abbrechen` brechen alle Läufe des Werks ab, auch
//!   solche, die noch auf die Sperre warten oder nach dem Zeitlimit im Hintergrund weiterrechnen. Die wartende
//!   Anfrage kehrt sofort mit `REFUSED` [`CANCELLED`] zurück; Typst selbst wird nur vor dem Lauf, zwischen Satz
//!   und SVG und vor jeder SVG-Seite unterbrochen (nicht über Welt-Zugriffe – das bricht comemo, siehe
//!   `baruch_render`); Auswertung und Umbruch laufen also zu Ende und halten bis dahin die Sperre.
//!   Ein abgebrochener Lauf legt nichts in den Zwischenspeicher.
//! - **Fremde Seiten (Runde 18a):** `GET /werke/{id}/satz`, `…/{stand}/{n}.svg` und `/werke/{id}/pdf` antworten
//!   bei `Sec-Fetch-Site: cross-site|same-site` mit 403, ohne Typst zu bemühen ([`crate::guard::cross_site_get`]).
//! - **Zwischenspeicher:** statisch, nach Fassungsstand ([`baruch_render::stand`] über Text und
//!   Bilder), höchstens [`CACHE_SIZE`] Einträge und zusammen höchstens [`CACHE_BYTES`] SVG-Bytes (Runde 18a; der
//!   neueste Stand bleibt immer), der älteste Gebrauch fliegt zuerst. Kein Feld in
//!   [`AppState`]: Der Stand ist ein Inhaltsschlüssel, gleiche Quellen ergeben gleiche Seiten.
//! - **Kapitel einer Seite (Runde 18a):** Eine Seite, die zwei Kapitel teilen, zählt zu beiden – zum Kapitel, das
//!   auf ihr beginnt (seine Überschrift steht dort), und zum Kapitel, das auf ihr endet (Text aus seinen Zeilen
//!   steht dort, [`baruch_render::Rendered::page_lines`]). Zeilen von Überschriften zählen dabei nicht, weil die
//!   Vorlage sie als lebenden Kolumnentitel oder im Inhaltsverzeichnis auf fremde Seiten zitieren kann.
//! - **Schriften (Runde 18a):** [`warm_fonts`] sucht die Systemschriften vorab (für den Start des Dienstes).
//! - **RPC** `preview.typst {work?, key?, page?}` ([`call`]), **Seiten** `GET /werke/{id}/satz?key=&page=`
//!   (ohne JavaScript: eine Seite groß, ‹ ›, Seitenliste des Kapitels, Meldungen mit Link zum
//!   Abschnitt) und `GET /werke/{id}/satz/{stand}/{n}.svg` ([`pages`]); MCP `typst_check` in
//!   [`crate::mcp::typst`]. Nur lesend: kein Schreibweg, kein Commit.
//! - **Rahmen (Runde 11):** [`prepare`] reicht Quelle und Karte nach `export_map`/`export_assets` an
//!   den Rahmen des Werks weiter (Umschlag, Titelei; `crate::cover::frame` nach der Integration) und
//!   berechnet den Stand erst danach. Satz-Ansicht und PDF ([`crate::pdf`]) nutzen dasselbe
//!   [`prepare`] und haben darum dieselbe Seitenzahl.
//! - **Teil-Seiten (Runde 14):** Stehen Teil-Markierungen (`<baruch-part>`, [`baruch_typst::marker`])
//!   im Werk, setzt [`prepare`] hinter Werkvorlage und Rahmen die Regel
//!   [`baruch_typst::part_page_rule`] mit den Nummern der Teile ([`part_labels`]): jeder Teil allein auf
//!   einer eigenen rechten Seite. Ohne Teil-Markierung ist die Quelle byte-gleich zum Stand vorher.
//! - **Pflichtangaben (Runde 27):** Bei eingeschalteter Titelei nennt `#druckpruefung` auch fehlende
//!   Impressumsangaben (`crate::cover_print::imprint_checks`: Verlag/Anschrift, Druckerei/Anschrift, Jahr,
//!   Rechtehinweis der Bibelübersetzung).
//! - **Druckprüfung (Runde 11):** `preview.typst` und `typst_check` tragen `print` (Rasterbilder unter
//!   300 dpi, [`baruch_render::PrintWarning`]); die Satz-Seite zeigt sie im Abschnitt `#druckpruefung`
//!   zusammen mit fehlenden Bildern („Bild fehlt: <Pfad>“) und dem Knopf „PDF herunterladen“.
//! - **Satz-Einstellungen (Runde 29):** Die Regeln „// Baruch: Satz“ stehen schon in der Quelle (`export_map`, Haken in
//!   `baruch_store::snapshot::head`); [`prepare`] reicht nur die Teil-Seite (`part_page`) an die Regel der Teil-Seiten
//!   weiter. `#druckpruefung` nennt Stellen der Werkvorlage, die eine gesetzte Einstellung selbst setzen, und eine
//!   erkannte Kapitelanfangs-Regel der Vorlage mit Link zu Einstellungen → Werk → Satz ([`template_hints`]).
//! - **Entwurf (Runde 22):** [`prepare_with`] setzt das Wasserzeichen ([`draft_rule_for`]) hinter Werkvorlage und
//!   Regelblock vor den Rahmen, dreiwertig nach [`effective_draft`] (`preview.typst {draft?}`, Seite und SVG mit
//!   `?entwurf=1|0`); ohne Wasserzeichen ist die Quelle byte-gleich. Die Satz-Seite trägt die Zeile `#entwurf` mit
//!   „Als Entwurf herunterladen“ und „Vorschau als Entwurf/ohne Wasserzeichen“ (ändern die Einstellung nicht).

use std::collections::{HashMap, VecDeque};
use std::path::{Path as FsPath, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::{NodeKind, Work};
use baruch_render::{Cancel, Rendered, Severity, Source};
use baruch_store::{ExportMap, StoreError, WorkId};
use baruch_typst::typeset::{Draft, TypesetSettings};
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, REFUSED, RpcError};
use crate::view::Settings;

/// Zeitlimit eines Typst-Laufs.
pub const COMPILE_TIMEOUT: Duration = Duration::from_secs(60);

/// Höchstzahl gesetzter Stände im Zwischenspeicher.
pub const CACHE_SIZE: usize = 8;

/// Höchstsumme der SVG-Bytes aller Stände im Zwischenspeicher (Runde 18a). Ein Buch von einigen hundert Seiten
/// braucht als SVG (Glyphen als Pfade) leicht 30–60 MiB; ohne Grenze hielten acht große Stände mehrere hundert MiB.
/// Der neueste Stand bleibt auch dann, wenn er allein größer ist.
pub const CACHE_BYTES: usize = 192 * 1024 * 1024;

/// Meldung eines abgebrochenen Laufs (`REFUSED`).
pub const CANCELLED: &str = "Satz abgebrochen.";

/// Höchstens ein Typst-Lauf gleichzeitig (Rechnerlast; Typst nutzt selbst mehrere Kerne nicht). Die
/// PDF-Erzeugung ([`crate::pdf`]) teilt sich diese Sperre.
pub(crate) static RUNS: Semaphore = Semaphore::const_new(1);

/// CSP der SVG-Seiten: nichts ausführen, nichts nachladen; eingebettete Bilder (`data:`) und
/// Inline-Stile, wie sie typst-svg schreibt, sind erlaubt.
pub const SVG_CSP: &str = "sandbox; default-src 'none'; img-src data:; style-src 'unsafe-inline'";

pub fn handles(method: &str) -> bool {
    matches!(method, "preview.typst" | "preview.cancel")
}

/// Ein gesetzter Stand.
pub struct Compiled {
    pub rendered: Rendered,
    /// Dauer des Typst-Laufs in Millisekunden.
    pub millis: u64,
}

fn cache() -> std::sync::MutexGuard<'static, VecDeque<Arc<Compiled>>> {
    static CACHE: OnceLock<Mutex<VecDeque<Arc<Compiled>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Eintrag zum Stand; ein Treffer rückt nach hinten (zuletzt gebraucht).
fn cache_get(stand: &str) -> Option<Arc<Compiled>> {
    let mut cache = cache();
    let at = cache.iter().position(|c| c.rendered.stand == stand)?;
    let hit = cache.remove(at)?;
    cache.push_back(hit.clone());
    Some(hit)
}

/// Seite `n` (1-basiert) des Stands `stand` aus dem Zwischenspeicher als SVG (Runde 29, Vorschau der
/// Satz-Einstellungen); zählt als Gebrauch. Unbekannter Stand oder Seite → `None`.
pub fn cached_page(stand: &str, n: usize) -> Option<String> {
    let compiled = cache_get(stand)?;
    compiled.rendered.pages.get(n.checked_sub(1)?).cloned()
}

fn cache_put(compiled: Arc<Compiled>) {
    cache_put_within(compiled, CACHE_BYTES);
}

/// SVG-Bytes eines Stands.
fn compiled_bytes(compiled: &Compiled) -> usize {
    compiled.rendered.pages.iter().map(String::len).sum()
}

/// Ablegen mit Bytegrenze `max_bytes`: höchstens [`CACHE_SIZE`] Stände und zusammen höchstens `max_bytes`, der am
/// längsten nicht gebrauchte fliegt zuerst; der neue bleibt immer.
fn cache_put_within(compiled: Arc<Compiled>, max_bytes: usize) {
    let mut cache = cache();
    cache.retain(|c| c.rendered.stand != compiled.rendered.stand);
    cache.push_back(compiled);
    let mut total: usize = cache.iter().map(|c| compiled_bytes(c)).sum();
    while cache.len() > CACHE_SIZE || (total > max_bytes && cache.len() > 1) {
        if let Some(old) = cache.pop_front() {
            total -= compiled_bytes(&old);
        }
    }
}

/// Liegt der Stand im Zwischenspeicher (ohne ihn als gebraucht zu zählen)? Für Tests.
#[doc(hidden)]
pub fn cache_contains(stand: &str) -> bool {
    cache().iter().any(|c| c.rendered.stand == stand)
}

// ---- Laufende Läufe und Abbruch (Runde 18a) ----------------------------------------------------

/// Ein angemeldeter Lauf: Datenordner des Servers (in Tests laufen viele Server in einem Prozess), Werk, Abbruch.
struct Running {
    id: u64,
    owner: PathBuf,
    work: WorkId,
    cancel: Cancel,
}

fn running() -> std::sync::MutexGuard<'static, Vec<Running>> {
    static RUNNING: Mutex<Vec<Running>> = Mutex::new(Vec::new());
    RUNNING.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Anmeldung eines Laufs; fällt mit dem letzten Halter (Anfrage und Hintergrundlauf) aus der Liste.
struct Registration(u64);

impl Drop for Registration {
    fn drop(&mut self) {
        running().retain(|r| r.id != self.0);
    }
}

fn register(owner: &FsPath, work: WorkId, cancel: &Cancel) -> Arc<Registration> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    running().push(Running { id, owner: owner.to_owned(), work, cancel: cancel.clone() });
    Arc::new(Registration(id))
}

/// Laufenden Satz des Werks abbrechen; `true`, wenn einer lief. Bricht die Läufe dieser Werk-id aller Server im
/// Prozess ab (im Dienst gibt es nur einen); Server-Code nimmt [`cancel_in`] mit dem eigenen Datenordner.
pub fn cancel(work: WorkId) -> bool {
    cancel_matching(|r| r.work == work)
}

/// Wie [`cancel`], nur für die Läufe des Servers mit Datenordner `owner`.
pub fn cancel_in(owner: &FsPath, work: WorkId) -> bool {
    cancel_matching(|r| r.work == work && r.owner == owner)
}

fn cancel_matching(matches: impl Fn(&Running) -> bool) -> bool {
    let running = running();
    let mut any = false;
    for run in running.iter().filter(|r| matches(r) && !r.cancel.is_cancelled()) {
        run.cancel.cancel();
        any = true;
    }
    any
}

/// Zahl der angemeldeten, nicht abgebrochenen Läufe des Werks (für Tests).
#[doc(hidden)]
pub fn running_in(owner: &FsPath, work: WorkId) -> usize {
    running().iter().filter(|r| r.work == work && r.owner == owner && !r.cancel.is_cancelled()).count()
}

/// Begonnene Typst-Läufe je Stand (für Tests: „kein Lauf von einer fremden Seite“). Höchstens 256 Stände; danach
/// fängt die Zählung von vorn an.
fn run_counts() -> std::sync::MutexGuard<'static, HashMap<String, u64>> {
    static COUNTS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    COUNTS.get_or_init(|| Mutex::new(HashMap::new())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn note_run(stand: &str) {
    let mut counts = run_counts();
    if counts.len() >= 256 && !counts.contains_key(stand) {
        counts.clear();
    }
    *counts.entry(stand.to_owned()).or_default() += 1;
}

/// Zahl der begonnenen Typst-Läufe für diesen Stand (für Tests).
#[doc(hidden)]
pub fn runs_of(stand: &str) -> u64 {
    run_counts().get(stand).copied().unwrap_or(0)
}

/// Kehrt zurück, sobald `cancel` abgebrochen ist (prüft alle 20 ms).
async fn until_cancelled(cancel: &Cancel) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn cancelled() -> RpcError {
    RpcError::new(REFUSED, CANCELLED)
}

/// Schriftsuche vorab (Runde 18a, Vorschlag für den Start in `main.rs`): lädt eingebettete und Systemschriften
/// im Hintergrund, damit der erste Satz nicht darauf wartet. Kehrt sofort zurück; braucht eine Tokio-Laufzeit.
pub fn warm_fonts() {
    tokio::task::spawn_blocking(|| {
        let _ = baruch_render::warm_fonts();
    });
}

/// Zahl der Einträge im Zwischenspeicher (für Tests).
#[doc(hidden)]
pub fn cache_len() -> usize {
    cache().len()
}

/// Das Werk für Typst vorbereitet: Karte, geladenes Werk (Nummern, Titel, Eltern), Quelle, Stand.
pub struct Prepared {
    pub map: ExportMap,
    pub work: Work,
    pub source: Source,
    pub stand: String,
}

pub async fn prepare(state: &AppState, work: WorkId) -> Result<Prepared, StoreError> {
    prepare_with(state, work, None).await
}

/// Wie [`prepare`] mit dem dreiwertigen Entwurf-Schalter (Runde 22, [`effective_draft`]): `None` = nach der
/// Satz-Einstellung, `Some(true)` = mit Wasserzeichen, `Some(false)` = ohne. Die Regel ([`draft_rule_for`]) steht
/// hinter Werkvorlage und Regelblock vor dem Rahmen (Umschlagseiten und Titelei im Buch tragen sie also auch) und
/// verschiebt die Karte wie der Rahmen; ohne Wasserzeichen bleibt alles byte-gleich.
pub async fn prepare_with(state: &AppState, work: WorkId, draft: Option<bool>) -> Result<Prepared, StoreError> {
    let map = state.store.export_map(work).await?;
    let files = state.store.export_assets(work).await?.into_iter().map(|f| (f.path, f.bytes)).collect();
    let source = Source { main: map.typst.clone(), files };
    let settings = state.store.work_typeset(work).await?.map(|t| t.settings).unwrap_or_default();
    let (source, map) = match effective_draft(settings.draft.as_ref(), draft) {
        Some(d) => crate::cover::insert_parts(source, map, &draft_rule_for(&d), "", Vec::new()),
        None => (source, map),
    };
    // Rahmen (Umschlag, Titelei) vor dem Stand: Er ist Teil dessen, was gesetzt wird.
    let (source, map) = frame_hook(state, work, source, map).await?;
    let loaded = state.store.load_work(work).await?;
    // Runde 29: Seite und leere Rückseite der Teil-Seite aus den Satz-Einstellungen (ohne Einstellungen byte-gleich).
    // Befund R29: Mit Regelblock, aber ohne `part_page` trägt die Teil-Regel trotzdem die Marke vor dem Umbruch
    // (`part_page_rule_for`), sonst trüge die Leerseite vor einem Teil Seitenzahl und Kolumnentitel.
    let (source, map) = part_pages(source, map, &loaded, &settings);
    let stand = baruch_render::stand(&source);
    Ok(Prepared { map, work: loaded, source, stand })
}

// ---- Entwurf (Runde 22, Vertrag §2.9) -------------------------------------------------------------

/// Wirksames Wasserzeichen für Satz-Vorschau, Buch-PDF und Stichwortzettel: `None` (Schalter fehlt) = die gespeicherte
/// Einstellung, `Some(true)` = die gespeicherten Werte bzw. [`Draft::default`], `Some(false)` = keins. Die Einstellung
/// selbst ändert der Schalter nie.
pub fn effective_draft(stored: Option<&Draft>, choice: Option<bool>) -> Option<Draft> {
    match choice {
        None => stored.cloned(),
        Some(true) => Some(stored.cloned().unwrap_or_default()),
        Some(false) => None,
    }
}

/// Wie [`effective_draft`] für das Umschlag-PDF: nach Einstellung nur mit `draft.cover`, sonst nur ausdrücklich.
pub fn cover_draft(stored: Option<&Draft>, choice: Option<bool>) -> Option<Draft> {
    match choice {
        None => stored.filter(|d| d.cover).cloned(),
        other => effective_draft(stored, other),
    }
}

/// Heutiges Datum in Ortszeit („10.11.2026“) – der einzige Ort, an dem das Wasserzeichen ein Datum bekommt.
pub fn draft_today() -> String {
    chrono::Local::now().format("%-d.%-m.%Y").to_string()
}

/// Regel des Wasserzeichens mit dem heutigen Datum, wenn `draft.date` gesetzt ist.
pub fn draft_rule_for(draft: &Draft) -> String {
    let date = draft.date.then(draft_today);
    baruch_typst::typeset::draft_rule(draft, date.as_deref())
}

/// Gespeicherter Entwurf des Werks (Satz-Einstellung `draft`).
pub async fn stored_draft(state: &AppState, work: WorkId) -> Result<Option<Draft>, StoreError> {
    Ok(state.store.work_typeset(work).await?.and_then(|t| t.settings.draft))
}

/// RPC-Parameter `draft`: fehlt/`null` → `None` (nach Einstellung), sonst `true`/`false`.
pub fn draft_param(params: &Value) -> Result<Option<bool>, RpcError> {
    match params.get("draft") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(RpcError::new(
            INVALID_PARAMS,
            "draft: true (mit Wasserzeichen), false (ohne) oder weglassen (nach Einstellung)",
        )),
    }
}

/// Abfrage-Parameter `?entwurf=`: fehlt/leer → `None`, `1`/`ja`/`true` → mit, `0`/`nein`/`false` → ohne.
pub fn draft_query(value: Option<&str>) -> Result<Option<bool>, String> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some("1" | "ja" | "true") => Ok(Some(true)),
        Some("0" | "nein" | "false") => Ok(Some(false)),
        Some(other) => Err(format!("entwurf „{other}“: 1 (mit Wasserzeichen) oder 0 (ohne) erwartet")),
    }
}

/// Anhang an eine Adresse, der den Schalter weiterreicht: „“, „entwurf=1“, „entwurf=0“.
pub fn draft_pair(choice: Option<bool>) -> &'static str {
    match choice {
        None => "",
        Some(true) => "entwurf=1",
        Some(false) => "entwurf=0",
    }
}

/// Rahmen des Werks um die Quelle legen (Umschlag-Vorder- und Rückseite, Titelei) und die Karte
/// verschieben ([`crate::cover::frame`]; ohne Umschlag und Titelei byte-gleich).
async fn frame_hook(
    state: &AppState,
    work: WorkId,
    source: Source,
    map: ExportMap,
) -> Result<(Source, ExportMap), StoreError> {
    crate::cover::frame(state, work, source, map).await
}

/// Nummern der Teil-Markierungen in der Reihenfolge, in der sie in der Quelle stehen (Runde 14): die
/// Nummer des Teils (`Node.number`, z. B. „II“), wenn die Markierung die erste Überschrift eines
/// Teil-Knotens ist; leer für einen Teil ohne Nummer und für jede andere Teil-Markierung (etwa eine im
/// Text eines Kapitels). Keine Markierung → leer.
pub fn part_labels(map: &ExportMap, work: &Work) -> Vec<String> {
    let mut labels = Vec::new();
    for section in &map.sections {
        let Some(body) = map.typst.get(section.start..section.end) else { continue };
        let Ok(doc) = baruch_typst::parse(body) else { continue };
        let node = work.nodes.iter().find(|n| n.key == section.key);
        for (index, heading) in doc.headings.iter().enumerate() {
            if heading.level != 0 {
                continue;
            }
            let own = index == 0 && doc.template.trim().is_empty() && node.is_some_and(|n| n.kind == NodeKind::Part);
            labels.push(if own { node.map(|n| n.number.clone()).unwrap_or_default() } else { String::new() });
        }
    }
    labels
}

/// Teil-Seiten (Runde 14): Stehen Teil-Markierungen in der Quelle, kommt hinter Werkvorlage und
/// Rahmen (vor den ersten Abschnitt) die Regel [`baruch_typst::part_page_rule`] mit den Nummern aus
/// [`part_labels`]: jeder Teil allein auf einer eigenen rechten Seite. Bytebereiche und erste Zeilen
/// der Abschnitte rücken nach (wie beim Rahmen). Ohne Teil-Markierung bleibt alles byte-gleich.
/// Runde 29: Die Regel kommt aus [`baruch_typst::typeset::part_page_rule_for`] – mit `part_page` Seite und leere
/// Rückseite, mit Regelblock die Marke vor dem Umbruch, ohne Satz-Einstellungen byte-gleich zu `part_page_rule`.
fn part_pages(source: Source, map: ExportMap, work: &Work, settings: &TypesetSettings) -> (Source, ExportMap) {
    let labels = part_labels(&map, work);
    if labels.is_empty() {
        return xref_safety(source, map, work);
    }
    let rule = baruch_typst::typeset::part_page_rule_for(&labels, settings);
    let (source, map) = crate::cover::insert_parts(source, map, &rule, "", Vec::new());
    // Runde 30a: Die Ankerzeile eines Teils (`#metadata("baruch-key") <kN>`) läge hinter dem Umbruch der Teil-Regel auf
    // der Folgeseite; im Satz steht sie darum vor der Teil-Zeile (`part_anchors_for_typesetting`).
    let (source, map) = rewrite_sections(source, map, |_, text| baruch_typst::xref::part_anchors_for_typesetting(text));
    xref_safety(source, map, work)
}

/// Runde 30a: Kaputte Querverweise (Ziel ohne aktuelle Fassung, `crate::xref::broken_in`) brächen den ganzen Satz
/// („label does not exist“). Im Satz werden sie zum bloßen Linktext; die Druckprüfung nennt sie.
fn xref_safety(source: Source, map: ExportMap, work: &Work) -> (Source, ExportMap) {
    let present: std::collections::HashSet<i64> =
        work.nodes.iter().filter(|n| n.current_number().is_some()).map(|n| n.key).collect();
    rewrite_sections(source, map, |_, text| {
        let broken: Vec<baruch_typst::xref::XrefUse> =
            baruch_typst::xref::find_xrefs(text).into_iter().filter(|x| !present.contains(&x.key)).collect();
        if broken.is_empty() {
            return std::borrow::Cow::Borrowed(text);
        }
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for x in broken {
            out.push_str(&text[at..x.start]);
            out.push_str(&x.text);
            at = x.end;
        }
        out.push_str(&text[at..]);
        std::borrow::Cow::Owned(out)
    })
}

/// Text je Abschnitt umschreiben (nur im Satz) und Bytebereiche und erste Zeilen der Karte nachziehen. Die Quelle muss
/// der Text der Karte sein (sonst bleibt alles, wie es ist).
fn rewrite_sections(
    source: Source,
    map: ExportMap,
    f: impl for<'a> Fn(i64, &'a str) -> std::borrow::Cow<'a, str>,
) -> (Source, ExportMap) {
    if source.main != map.typst {
        return (source, map);
    }
    let mut typst = String::with_capacity(map.typst.len());
    let mut sections = Vec::with_capacity(map.sections.len());
    let mut at = 0;
    let mut changed = false;
    let (mut counted_to, mut line) = (0usize, 1usize);
    for section in &map.sections {
        if section.start < at || section.end < section.start || section.end > map.typst.len() {
            return (source, map);
        }
        typst.push_str(&map.typst[at..section.start]);
        let text = &map.typst[section.start..section.end];
        let new = f(section.key, text);
        changed |= matches!(new, std::borrow::Cow::Owned(_));
        let start = typst.len();
        line += baruch_typst::line_at(&typst[counted_to..], start - counted_to) - 1;
        counted_to = start;
        typst.push_str(&new);
        sections.push(baruch_store::ExportSection { start, end: typst.len(), first_line: line, ..section.clone() });
        at = section.end;
    }
    if !changed {
        return (source, map);
    }
    typst.push_str(&map.typst[at..]);
    let source = Source { main: typst.clone(), ..source };
    (source, ExportMap { typst, sections })
}

/// „60 s“, „0,05 s“.
pub(crate) fn seconds(limit: Duration) -> String {
    if limit.subsec_millis() == 0 {
        format!("{} s", limit.as_secs())
    } else {
        format!("{:.3} s", limit.as_secs_f64()).replace('.', ",")
    }
}

/// Setzt die Quelle oder nimmt den Stand aus dem Zwischenspeicher (`true` = Treffer). Höchstens ein
/// Lauf gleichzeitig; das Zeitlimit zählt ab dem Warten auf die Sperre. Nicht angemeldet (kein Werk, z. B. der
/// Umschlag): über `preview.cancel` nicht abbrechbar.
pub async fn compile(source: Source, stand: &str, limit: Duration) -> Result<(Arc<Compiled>, bool), RpcError> {
    compile_run(source, stand, limit, Cancel::new(), None).await
}

/// Wie [`compile`], angemeldet als Lauf des Werks `work` dieses Servers (abbrechbar über [`cancel_in`]).
pub async fn compile_for(
    state: &AppState,
    work: WorkId,
    source: Source,
    stand: &str,
    limit: Duration,
) -> Result<(Arc<Compiled>, bool), RpcError> {
    if let Some(hit) = cache_get(stand) {
        return Ok((hit, true));
    }
    let cancel = Cancel::new();
    let registration = register(&state.data, work, &cancel);
    compile_run(source, stand, limit, cancel, Some(registration)).await
}

async fn compile_run(
    source: Source,
    stand: &str,
    limit: Duration,
    cancel: Cancel,
    registration: Option<Arc<Registration>>,
) -> Result<(Arc<Compiled>, bool), RpcError> {
    if let Some(hit) = cache_get(stand) {
        return Ok((hit, true));
    }
    let stand = stand.to_owned();
    let run = async move {
        let permit = tokio::select! {
            permit = RUNS.acquire() => permit.map_err(|e| RpcError::new(INTERNAL, format!("Typst-Sperre: {e}")))?,
            () = until_cancelled(&cancel) => return Err(cancelled()),
        };
        // Während des Wartens kann ein anderer Lauf genau diesen Stand gesetzt haben.
        if let Some(hit) = cache_get(&stand) {
            return Ok((hit, true));
        }
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        note_run(&stand);
        let run_cancel = cancel.clone();
        let run_registration = registration.clone();
        let handle = tokio::task::spawn_blocking(move || {
            // Sperre und Anmeldung gehören dem Lauf, nicht der Anfrage: Läuft die Zeit ab, rechnet Typst zu
            // Ende (bleibt dabei abbrechbar) und gibt die Sperre erst dann frei.
            let _permit = permit;
            let _registration = run_registration;
            let started = Instant::now();
            let rendered = baruch_render::compile_cancellable(&source, &run_cancel)?;
            let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let compiled = Arc::new(Compiled { rendered, millis });
            cache_put(compiled.clone());
            Some(compiled)
        });
        // Abgebrochen: die Anfrage kehrt sofort zurück, auch wenn Typst noch bis zur nächsten Prüfstelle rechnet.
        tokio::select! {
            joined = handle => match joined.map_err(|e| RpcError::new(INTERNAL, format!("Typst ist abgestürzt: {e}")))? {
                Some(compiled) => Ok((compiled, false)),
                None => Err(cancelled()),
            },
            () = until_cancelled(&cancel) => Err(cancelled()),
        }
    };
    let begun = Instant::now();
    // `tokio::time::timeout` pollt zuerst den Lauf: Ist er schneller fertig, als der Zeitgeber feuert, kommt
    // `Ok` auch nach Ablauf des Limits (bei 0 immer möglich). Darum zählt zusätzlich die gemessene Zeit; der
    // fertige Satz liegt dann schon im Zwischenspeicher.
    match tokio::time::timeout(limit, run).await {
        Ok(_) if begun.elapsed() >= limit => {
            Err(RpcError::new(REFUSED, format!("Typst brauchte länger als {}", seconds(limit))))
        }
        Ok(result) => result,
        Err(_) => Err(RpcError::new(REFUSED, format!("Typst brauchte länger als {}", seconds(limit)))),
    }
}

/// Ergebnis für RPC, Seite und MCP.
pub struct Typeset {
    pub prepared: Prepared,
    pub compiled: Arc<Compiled>,
    pub cached: bool,
}

/// Werk vorbereiten und setzen (mit Zeitlimit `limit`). Tests rufen das mit kurzem Limit auf.
#[doc(hidden)]
pub async fn typeset_with_limit(state: &AppState, work: WorkId, limit: Duration) -> Result<Typeset, RpcError> {
    typeset_draft(state, work, limit, None).await
}

/// Wie [`typeset_with_limit`] mit dem Entwurf-Schalter (Runde 22, [`prepare_with`]).
pub async fn typeset_draft(
    state: &AppState,
    work: WorkId,
    limit: Duration,
    draft: Option<bool>,
) -> Result<Typeset, RpcError> {
    let prepared = prepare_with(state, work, draft).await?;
    let (compiled, cached) = compile_for(state, work, prepared.source.clone(), &prepared.stand, limit).await?;
    Ok(Typeset { prepared, compiled, cached })
}

pub async fn typeset(state: &AppState, work: WorkId) -> Result<Typeset, RpcError> {
    typeset_with_limit(state, work, COMPILE_TIMEOUT).await
}

/// Nur aus dem Zwischenspeicher (Runde 18a, Seite nach „Satz abbrechen“): der aktuelle Stand, wenn er schon
/// gesetzt ist, sonst `None` – ohne Typst-Lauf.
pub async fn cached_typeset(state: &AppState, work: WorkId) -> Result<Option<Typeset>, RpcError> {
    cached_typeset_draft(state, work, None).await
}

/// Wie [`cached_typeset`] mit dem Entwurf-Schalter.
pub async fn cached_typeset_draft(
    state: &AppState,
    work: WorkId,
    draft: Option<bool>,
) -> Result<Option<Typeset>, RpcError> {
    let prepared = prepare_with(state, work, draft).await?;
    Ok(cache_get(&prepared.stand).map(|compiled| Typeset { prepared, compiled, cached: true }))
}

/// Wie [`typeset`], prüft aber vor dem Lauf, ob es den Abschnitt `key` gibt (sonst
/// `INVALID_PARAMS`, ohne Typst zu bemühen).
pub async fn typeset_at(state: &AppState, work: WorkId, key: Option<i64>) -> Result<Typeset, RpcError> {
    typeset_at_draft(state, work, key, None).await
}

/// Wie [`typeset_at`] mit dem Entwurf-Schalter (`preview.typst {draft?}`).
pub async fn typeset_at_draft(
    state: &AppState,
    work: WorkId,
    key: Option<i64>,
    draft: Option<bool>,
) -> Result<Typeset, RpcError> {
    let prepared = prepare_with(state, work, draft).await?;
    check_key(&prepared.work, work, key)?;
    let (compiled, cached) =
        compile_for(state, work, prepared.source.clone(), &prepared.stand, COMPILE_TIMEOUT).await?;
    Ok(Typeset { prepared, compiled, cached })
}

// ---- Abschnitte, Kapitel, Meldungen ------------------------------------------------------------

/// Anzeigename eines Abschnitts: „2.1 Titel“ (ohne Nummer nur der Titel).
fn section_label(work: &Work, key: i64) -> Option<String> {
    let node = work.nodes.iter().find(|n| n.key == key)?;
    Some(format!("{} {}", node.number, node.title).trim().to_owned())
}

impl Typeset {
    pub fn rendered(&self) -> &Rendered {
        &self.compiled.rendered
    }

    pub fn page_count(&self) -> usize {
        self.rendered().pages.len()
    }

    /// Seite (1-basiert), auf der der Abschnitt `index` der Karte beginnt: die seiner ersten
    /// Überschrift; ohne eigene Überschrift die letzte davor; sonst 1.
    fn section_page(&self, index: usize) -> usize {
        let sections = &self.prepared.map.sections;
        let Some(section) = sections.get(index) else { return 1 };
        let next = sections.get(index + 1).map_or(usize::MAX, |s| s.first_line);
        let headings = &self.rendered().headings;
        headings
            .iter()
            .find(|h| h.line >= section.first_line && h.line < next.max(section.first_line + 1))
            .or_else(|| headings.iter().rev().find(|h| h.line < section.first_line))
            .map_or(1, |h| h.page)
    }

    /// Stelle des Abschnitts mit Schlüssel `key` in der Karte.
    fn index_of(&self, key: i64) -> Option<usize> {
        self.prepared.map.sections.iter().position(|s| s.key == key)
    }

    /// Kapitel des Abschnitts `key`: der oberste Gliederungspunkt über ihm (unter einem Teil das
    /// Kapitel darin). Seiten `first..=last`: von der Seite seiner Überschrift bis vor die Seite des
    /// nächsten Punkts außerhalb (mindestens `first`), am Ende bis zur letzten Seite. Ohne `key`:
    /// das ganze Werk.
    pub fn chapter(&self, key: Option<i64>) -> (usize, usize) {
        let total = self.page_count().max(1);
        let Some(index) = key.and_then(|k| self.index_of(k)) else { return (1, total) };
        let work = &self.prepared.work;
        let seq_of = |key: i64| work.nodes.iter().find(|n| n.key == key).map(|n| n.id);
        let parent_of = |seq: usize| work.nodes.iter().find(|n| n.id == seq).and_then(|n| n.parent);
        let kind_of = |seq: usize| work.nodes.iter().find(|n| n.id == seq).map(|n| n.kind);
        let Some(seq) = seq_of(self.prepared.map.sections[index].key) else { return (1, total) };
        let mut chain = vec![seq];
        while let Some(parent) = parent_of(*chain.last().unwrap()) {
            if chain.contains(&parent) {
                break; // Schleife: nicht endlos laufen
            }
            chain.push(parent);
        }
        chain.reverse();
        let chapter = match chain.as_slice() {
            [part, inner, ..] if kind_of(*part) == Some(NodeKind::Part) => *inner,
            [top, ..] => *top,
            [] => seq,
        };
        let in_chapter = |seq: usize| {
            let mut at = Some(seq);
            let mut steps = 0;
            while let Some(s) = at {
                if s == chapter {
                    return true;
                }
                steps += 1;
                if steps > work.nodes.len() {
                    return false;
                }
                at = parent_of(s);
            }
            false
        };
        let sections = &self.prepared.map.sections;
        let Some(start) = sections.iter().position(|s| seq_of(s.key) == Some(chapter)) else {
            return (self.section_page(index), self.section_page(index));
        };
        let first = self.section_page(start);
        let after = sections.iter().enumerate().skip(start + 1).find(|(_, s)| !seq_of(s.key).is_some_and(in_chapter));
        let last = match after {
            Some((next, _)) => {
                // Die Seite, auf der das nächste Kapitel beginnt, gehört auch zu diesem, wenn Text aus seinen
                // Zeilen auf ihr steht (geteilte Seite, siehe Modulkopf).
                let page = self.section_page(next);
                let lines = sections[start].first_line..sections[next].first_line;
                if self.has_text_from(page, lines) { page } else { page.saturating_sub(1) }.max(first)
            }
            None => total,
        };
        (first, last.min(total).max(first))
    }

    /// Steht auf Seite `page` (1-basiert) Text aus den Zeilen `lines` von `main`, außer Überschriftszeilen?
    fn has_text_from(&self, page: usize, lines: std::ops::Range<usize>) -> bool {
        let rendered = self.rendered();
        let Some(on_page) = page.checked_sub(1).and_then(|i| rendered.page_lines.get(i)) else { return false };
        on_page.iter().any(|line| lines.contains(line) && !rendered.headings.iter().any(|h| h.line == *line))
    }

    /// Seite des Abschnitts `key` (ohne bzw. unbekannt: 1).
    pub fn page_of(&self, key: Option<i64>) -> usize {
        key.and_then(|k| self.index_of(k)).map_or(1, |i| self.section_page(i))
    }

    /// Meldungen mit Abschnitt: `key`, `section` („2.1 Titel“, „Werkvorlage“ bzw. – Runde 23a – [`TAIL_LABEL`] für
    /// Zeilen des Buchendes) und `section_line` (Zeile im Abschnitt, in der Vorlage bzw. im Buchende, 1-basiert).
    pub fn diagnostics(&self) -> Vec<DiagnosticView> {
        let map = &self.prepared.map;
        self.rendered()
            .diagnostics
            .iter()
            .map(|d| {
                let (key, section, section_line) = match (d.line, &d.file) {
                    (Some(line), None) => match map.section_at_line(line) {
                        Some(s) => {
                            (Some(s.key), section_label(&self.prepared.work, s.key), Some(line - s.first_line + 1))
                        }
                        // Runde 23a: Zeilen hinter dem letzten Abschnitt gehören dem Buchende, nicht ihm.
                        None => match map.tail_first_line().filter(|first| line >= *first) {
                            Some(first) => (None, Some(TAIL_LABEL.to_owned()), Some(line - first + 1)),
                            None => (None, Some("Werkvorlage".to_owned()), Some(line)),
                        },
                    },
                    _ => (None, None, None),
                };
                DiagnosticView {
                    severity: d.severity.as_str(),
                    message: d.message.clone(),
                    hints: d.hints.clone(),
                    line: d.line,
                    column: d.column,
                    file: d.file.clone(),
                    key,
                    section,
                    section_line,
                }
            })
            .collect()
    }

    pub fn counts(&self) -> (usize, usize) {
        let errors = self.rendered().diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
        (errors, self.rendered().diagnostics.len() - errors)
    }
}

/// Runde 23a: Name des Buchendes in Satzmeldungen (Register, Glossar und Verzeichnisse hinter dem letzten Abschnitt).
pub const TAIL_LABEL: &str = "Buchende (Register, Glossar)";

/// Eine Meldung für RPC, MCP und Seite.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticView {
    pub severity: &'static str,
    pub message: String,
    pub hints: Vec<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub key: Option<i64>,
    pub section: Option<String>,
    pub section_line: Option<usize>,
}

// ---- RPC ---------------------------------------------------------------------------------------

/// Fester Schlüssel als Zahl oder Ziffern (Formulare); leer → `None`.
fn key_value(value: Option<&Value>, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, format!("{name}: Zahl erwartet"));
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Gibt es den Abschnitt `key` im Werk? Sonst `INVALID_PARAMS`.
fn check_key(work: &Work, work_id: WorkId, key: Option<i64>) -> Result<(), RpcError> {
    match key {
        Some(key) if !work.nodes.iter().any(|n| n.key == key) => Err(RpcError::new(
            INVALID_PARAMS,
            format!("Abschnitt mit Schlüssel {key} gibt es in Werk {work_id} nicht."),
        )),
        _ => Ok(()),
    }
}

/// `preview.typst {work?, key?, page?, draft?}` → `{stand, pages, page, chapter: {first, last},
/// diagnostics, cached, millis, draft, svg?}` (Runde 22: `draft` dreiwertig; Antwort `draft` = Wasserzeichen im Satz). `page` ist die verlangte Seite, sonst die Seite des
/// Abschnitts `key` (ohne `key`: 1); `svg` nur mit `page`.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    if !handles(method) {
        return Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unbekannte Methode {method}")));
    }
    if method == "preview.cancel" {
        // Unbekanntes Werk: dieselbe Meldung wie überall (`UnknownWork` → -32602), nicht still `false`.
        state.store.work_template(work).await?;
        return Ok(json!({ "cancelled": cancel_in(&state.data, work) }));
    }
    let key = key_value(params.get("key"), "key")?;
    let page = key_value(params.get("page"), "page")?;
    let draft = draft_param(&params)?;
    let typeset = typeset_at_draft(state, work, key, draft).await?;
    let total = typeset.page_count();
    let (first, last) = typeset.chapter(key);
    let mut answer = json!({
        "stand": typeset.prepared.stand,
        "pages": total,
        "page": typeset.page_of(key),
        "chapter": { "first": first, "last": last },
        "diagnostics": typeset.diagnostics(),
        "print": typeset.rendered().print,
        "cached": typeset.cached,
        "millis": typeset.compiled.millis,
        "draft": typeset.prepared.source.main.contains(baruch_typst::typeset::DRAFT_MARK),
    });
    if let Some(page) = page {
        let svg = usize::try_from(page)
            .ok()
            .filter(|&p| p >= 1)
            .and_then(|p| typeset.rendered().pages.get(p - 1))
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("Seite {page} gibt es nicht (1–{total}).")))?;
        answer["page"] = json!(page);
        answer["svg"] = json!(svg);
    }
    Ok(answer)
}

// ---- Seiten ------------------------------------------------------------------------------------

/// `GET /werke/{id}/satz?key=&page=` und `GET /werke/{id}/satz/{stand}/{n}.svg` (nur lesen).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/satz", get(page)).route("/werke/{id}/satz/{stand}/{file}", get(svg))
}

/// `POST /werke/{id}/satz/abbrechen` (hinter guard::same_origin).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/satz/abbrechen", post(cancel_act))
}

/// Formular „Satz abbrechen“ des Menübands der Satz-Seite: bricht die Läufe des Werks ab und leitet mit 303 auf
/// `/werke/{id}/satz?abgebrochen=1` (lief keiner: `abgebrochen=0`; mit Feld `key` hängt `&key=K` an). Die Seite
/// setzt dann nicht neu, sondern zeigt nur einen schon gesetzten Stand (siehe [`page`]).
async fn cancel_act(
    State(state): State<Arc<AppState>>,
    Path(id): Path<WorkId>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    match state.store.works().await {
        Ok(works) if works.iter().any(|(w, _, _)| *w == id) => {}
        Ok(_) => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    }
    let flag = if cancel_in(&state.data, id) { 1 } else { 0 };
    let key = pairs
        .iter()
        .find(|(name, _)| name == "key")
        .and_then(|(_, value)| value.trim().parse::<i64>().ok())
        .map(|k| format!("&key={k}"))
        .unwrap_or_default();
    Redirect::to(&format!("/werke/{id}/satz?abgebrochen={flag}{key}")).into_response()
}

/// 403 für einen GET, den eine fremde Seite ausgelöst hat (Runde 18a): Bilder, Links und Formulare anderer
/// Seiten sollen keinen Typst-Lauf anstoßen.
pub(crate) fn foreign_get() -> Response {
    (StatusCode::FORBIDDEN, "Abgelehnt: Der Satz startet nicht auf Anfrage einer fremden Seite.").into_response()
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            // Runde 33: Die Bedienung der Satz-Seite steht im Menüband (Block band_buch in typeset.html);
            // fragments/edit_toolbar.html wird hier nicht mehr eingebunden.
            ("typeset.html", include_str!("../templates/typeset.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Satz-Seite fehlerhaft");
        }
        env
    })
}

/// Selbst gebaute Adresse (Ziffern, Hex-Stand): nur `&` maskieren, damit `/` lesbar bleibt.
pub(crate) fn href(url: String) -> JinjaValue {
    if url.chars().any(|c| matches!(c, '"' | '<' | '>' | '\'' | ' ') || c.is_control()) {
        return JinjaValue::from_safe_string("/werke".to_owned());
    }
    JinjaValue::from_safe_string(url.replace('&', "&amp;"))
}

#[derive(Deserialize)]
struct PageQuery {
    key: Option<String>,
    page: Option<String>,
    /// Nach dem Formular „Satz abbrechen“: `1` (abgebrochen) bzw. `0` (lief keiner) – dann kein neuer Lauf.
    abgebrochen: Option<String>,
    /// Runde 22: Wasserzeichen dreiwertig (fehlt = nach Einstellung, `1` = mit, `0` = ohne).
    entwurf: Option<String>,
}

#[derive(Serialize)]
struct PageLink {
    number: usize,
    href: JinjaValue,
    current: bool,
}

#[derive(Serialize)]
pub(crate) struct DiagnosticLink {
    #[serde(flatten)]
    diagnostic: DiagnosticView,
    label: &'static str,
    href: Option<JinjaValue>,
}

/// Serde-Wert für die Vorlage (minijinja 3 nimmt eigene Typen nur so).
pub(crate) fn sv<T: Serialize>(value: T) -> JinjaValue {
    JinjaValue::from(Serde(value))
}

fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::NOT_FOUND,
        REFUSED => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(crate) fn render(status: StatusCode, ctx: JinjaValue) -> Response {
    // Die Satz-Seite lädt kein main.js: Die Kopfzeile in base.html zeigt dann nur, was ohne Widget geht.
    let ctx = minijinja::context! { scriptless => true, ..ctx };
    match templates().get_template("typeset.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage typeset.html: {e}")).into_response(),
    }
}

async fn page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(q): Query<PageQuery>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return foreign_get();
    }
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let settings = JinjaValue::from(Serde(settings));
    let key: Option<i64> = q.key.as_deref().and_then(|k| k.trim().parse().ok());
    // Runde 22: Unlesbares `entwurf` zählt wie „fehlt“ (die Seite bleibt bedienbar).
    let draft = draft_query(q.entwurf.as_deref()).unwrap_or(None);
    let pair = draft_pair(draft);
    let title = match state.store.works().await {
        Ok(works) => match works.into_iter().find(|(w, _, _)| *w == id) {
            Some((_, _, title)) => title,
            None => return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
        },
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let back = href(match key {
        Some(key) => format!("/?work={id}&key={key}"),
        None => format!("/?work={id}"),
    });
    let work_ctx = json!({ "id": id, "title": title });
    // Nach „Satz abbrechen“: nicht neu setzen, nur einen schon gesetzten Stand zeigen.
    let cancelled_notice = q.abgebrochen.as_deref().map(|flag| {
        if flag == "0" { "Es lief kein Satz, der abzubrechen war." } else { "Der Satz wurde abgebrochen." }
    });
    let restart = href(match (key, pair) {
        (Some(key), "") => format!("/werke/{id}/satz?key={key}"),
        (Some(key), pair) => format!("/werke/{id}/satz?key={key}&{pair}"),
        (None, "") => format!("/werke/{id}/satz"),
        (None, pair) => format!("/werke/{id}/satz?{pair}"),
    });
    let stored = stored_draft(&state, id).await.ok().flatten();
    let draft_links = draft_links(id, key, draft, stored.is_some());
    let result = match cancelled_notice {
        Some(_) => cached_typeset_draft(&state, id, draft).await,
        None => typeset_draft(&state, id, COMPILE_TIMEOUT, draft).await.map(Some),
    };
    let typeset = match result {
        Ok(Some(typeset)) => typeset,
        Ok(None) => {
            let ctx = minijinja::context! {
                settings, work => sv(work_ctx), back, key, restart, cancelled_notice, idle => true,
                draft_links => sv(&draft_links),
            };
            return render(StatusCode::OK, ctx);
        }
        Err(e) => {
            let cancelled = e.message == CANCELLED;
            let ctx = minijinja::context! {
                settings, work => sv(work_ctx), back, key, restart,
                error => (!cancelled).then(|| e.message.clone()),
                cancelled_notice => cancelled.then_some("Der Satz wurde abgebrochen."),
                idle => cancelled,
                draft_links => sv(&draft_links),
            };
            return render(status_of(&e), ctx);
        }
    };
    let key = key.filter(|k| typeset.prepared.work.nodes.iter().any(|n| n.key == *k));
    let total = typeset.page_count();
    let (first, last) = typeset.chapter(key);
    let shown = q
        .page
        .as_deref()
        .and_then(|p| p.trim().parse::<usize>().ok())
        .filter(|p| (1..=total).contains(p))
        .unwrap_or_else(|| typeset.page_of(key).clamp(1, total.max(1)));
    let key_param = key.map(|k| format!("key={k}&")).unwrap_or_default();
    let key_param = if pair.is_empty() { key_param } else { format!("{key_param}{pair}&") };
    let page_href = |n: usize| href(format!("/werke/{id}/satz?{key_param}page={n}"));
    let chapter_pages: Vec<PageLink> = if total == 0 {
        Vec::new()
    } else {
        (first..=last).map(|n| PageLink { number: n, href: page_href(n), current: n == shown }).collect()
    };
    let diagnostics = diagnostic_links(id, typeset.diagnostics());
    let missing = missing_images(&typeset.prepared.source);
    let broken = broken_xrefs(id, &typeset.prepared.work);
    let (errors, warnings) = typeset.counts();
    let section = key.and_then(|k| section_label(&typeset.prepared.work, k));
    let imprint = imprint_warnings(&state, id).await;
    let (conflicts, template_rule) = template_hints(&state, id).await;
    let ctx = minijinja::context! {
        settings,
        work => sv(work_ctx),
        back,
        key,
        restart,
        cancelled_notice,
        section,
        total,
        shown,
        image => (total > 0).then(|| {
            let query = if pair.is_empty() { String::new() } else { format!("?{pair}") };
            href(format!("/werke/{id}/satz/{}/{shown}.svg{query}", typeset.prepared.stand))
        }),
        previous => (shown > 1).then(|| page_href(shown - 1)),
        next => (shown < total).then(|| page_href(shown + 1)),
        chapter => sv(json!({ "first": first, "last": last })),
        chapter_pages => sv(chapter_pages),
        diagnostics => sv(diagnostics),
        errors,
        warnings,
        cached => typeset.cached,
        millis => typeset.compiled.millis,
        print => sv(&typeset.rendered().print),
        missing => sv(missing),
        broken => sv(broken),
        images_href => href(format!("/werke/{id}/bilder")),
        min_dpi => baruch_render::MIN_DPI as u64,
        imprint => sv(imprint),
        conflicts => sv(conflicts),
        template_rule => sv(template_rule),
        typeset_settings_href => href(format!("/einstellungen/werk?work={id}#satz")),
        draft_links => sv(&draft_links),
        watermarked => typeset.prepared.source.main.contains(baruch_typst::typeset::DRAFT_MARK),
    };
    render(StatusCode::OK, ctx)
}

/// Runde 22: Links der Satz-Seite rund um das Wasserzeichen (ohne JavaScript): „Als Entwurf herunterladen“ (Buch-PDF
/// mit Wasserzeichen), dazu die Vorschau mit bzw. ohne Wasserzeichen (je nachdem, was gerade gezeigt wird) und das PDF
/// ohne Wasserzeichen, wenn die Einstellung eins vorsieht. Keiner ändert die Einstellung.
#[derive(Serialize)]
struct DraftLinks {
    /// `/werke/{id}/pdf?entwurf=1`.
    pdf_draft: JinjaValue,
    /// `/werke/{id}/pdf?entwurf=0` (nur, wenn die Einstellung ein Wasserzeichen vorsieht).
    pdf_plain: Option<JinjaValue>,
    /// Vorschau umschalten: Adresse und Beschriftung.
    toggle: JinjaValue,
    toggle_label: &'static str,
}

fn draft_links(id: WorkId, key: Option<i64>, draft: Option<bool>, stored: bool) -> DraftLinks {
    let shown = draft.unwrap_or(stored);
    let key_part = key.map(|k| format!("key={k}&")).unwrap_or_default();
    let (flag, label) = if shown { ("0", "Vorschau ohne Wasserzeichen") } else { ("1", "Vorschau als Entwurf") };
    DraftLinks {
        pdf_draft: href(format!("/werke/{id}/pdf?entwurf=1")),
        pdf_plain: stored.then(|| href(format!("/werke/{id}/pdf?entwurf=0"))),
        toggle: href(format!("/werke/{id}/satz?{key_part}entwurf={flag}")),
        toggle_label: label,
    }
}

/// Runde 29 (Abschnitt `#druckpruefung`): Stellen der Werkvorlage, die gesetzte Satz-Einstellungen selbst setzen
/// ([`baruch_typst::typeset::template_conflicts`], je `{field, label, line, text, effect, message}`), und eine
/// erkannte Kapitelanfangs-Regel der Vorlage (`{line, text, chapter_start, label}`), die sich in den Einstellungen
/// übernehmen lässt. Ein Lesefehler zeigt nichts (die Seite bleibt bedienbar).
pub async fn template_hints(state: &AppState, work: WorkId) -> (Vec<Value>, Option<Value>) {
    let Ok(template) = state.store.work_template(work).await else { return (Vec::new(), None) };
    let settings: TypesetSettings =
        state.store.work_typeset(work).await.ok().flatten().map(|t| t.settings).unwrap_or_default();
    let conflicts = baruch_typst::typeset::template_conflicts(&template, &settings)
        .into_iter()
        .map(|c| {
            json!({
                "field": c.field,
                "label": baruch_typst::typeset::field_label(c.field),
                "line": c.line,
                "text": c.text,
                "effect": c.effect,
                "message": c.message,
            })
        })
        .collect();
    let rule = baruch_typst::typeset::detect_chapter_break(&template).map(|r| {
        json!({
            "line": r.line,
            "text": r.text,
            "chapter_start": r.chapter_start.id(),
            "label": r.chapter_start.label(),
            "applied": settings.chapter_start == Some(r.chapter_start),
        })
    });
    (conflicts, rule)
}

/// Kaputte Querverweise für die Druckprüfung (Runde 30a, [`crate::xref::broken_in`]): Abschnitt, Sprungmarke,
/// Linktext und Link zur Stelle. Im Satz stehen sie nur als Text (siehe `xref_safety`).
fn broken_xrefs(id: WorkId, work: &Work) -> Vec<Value> {
    crate::xref::broken_in(work)
        .into_iter()
        .map(|b| {
            let section = work
                .nodes
                .iter()
                .find(|n| n.key == b.key)
                .map_or_else(String::new, |n| format!("{} {}", n.number, n.title).trim().to_owned());
            let target = crate::xref::JumpTarget { work: id, key: b.key, rev: None, find: Some(b.text.clone()), at: Some(b.start) };
            json!({ "section": section, "label": b.label, "text": b.text, "href": href(crate::xref::jump_url(&target)).to_string() })
        })
        .collect()
}

/// Pflichtangaben im Impressum (Runde 27, [`crate::cover_print::imprint_checks`]) für den Abschnitt
/// `#druckpruefung`: nur bei eingeschalteter Titelei; ein Lesefehler zeigt nichts (die Seite bleibt bedienbar).
async fn imprint_warnings(state: &AppState, work: WorkId) -> Vec<crate::cover_print::CoverWarning> {
    let (Ok(meta), Ok(w)) = (state.store.book_meta(work).await, state.store.work_meta(work).await) else {
        return Vec::new();
    };
    crate::cover_print::imprint_checks(&meta, &w.bible)
}

/// Meldungen mit Bezeichnung („Fehler“/„Warnung“) und Link zum Abschnitt (Satz-Seite, PDF-Fehlerseite).
pub(crate) fn diagnostic_links(id: WorkId, diagnostics: Vec<DiagnosticView>) -> Vec<DiagnosticLink> {
    diagnostics
        .into_iter()
        .map(|d| DiagnosticLink {
            label: if d.severity == "error" { "Fehler" } else { "Warnung" },
            href: d.key.map(|k| href(format!("/?work={id}&key={k}"))),
            diagnostic: d,
        })
        .collect()
}

/// Bildpfade, die Vorlage oder Text über `image("…")` nennen, ohne Datei im Werk (Normalform,
/// sortiert, ohne Doppel). Pfade ohne Normalform (`..`) zählen als fehlend, wie sie im Text stehen.
pub fn missing_images(source: &Source) -> Vec<String> {
    let present: std::collections::HashSet<String> =
        source.files.iter().filter_map(|(path, _)| baruch_render::normalize_path(path)).collect();
    let mut missing: Vec<String> = baruch_typst::images::image_paths(&source.main)
        .into_iter()
        .map(|path| baruch_render::normalize_path(&path).unwrap_or(path))
        .filter(|path| !present.contains(path))
        .collect();
    missing.sort();
    missing.dedup();
    missing
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Seite gibt es nicht (anderer oder veralteter Stand)").into_response()
}

#[derive(Deserialize)]
struct SvgQuery {
    entwurf: Option<String>,
}

/// Eine Seite als SVG, nur für den aktuellen Stand des Werks (fremder oder alter Stand → 404).
async fn svg(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((id, stand, file)): Path<(WorkId, String, String)>,
    Query(q): Query<SvgQuery>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return foreign_get();
    }
    let Some(n) = file.strip_suffix(".svg").and_then(|n| n.parse::<usize>().ok()).filter(|&n| n >= 1) else {
        return not_found();
    };
    if stand.len() != 64 || !stand.bytes().all(|b| b.is_ascii_hexdigit()) {
        return not_found();
    }
    // Runde 22: derselbe Schalter wie die Satz-Seite, sonst passt der Stand nicht.
    let draft = draft_query(q.entwurf.as_deref()).unwrap_or(None);
    let prepared = match prepare_with(&state, id, draft).await {
        Ok(prepared) => prepared,
        Err(StoreError::UnknownWork(_)) => return not_found(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    if prepared.stand != stand {
        return not_found();
    }
    let compiled = match compile_for(&state, id, prepared.source, &prepared.stand, COMPILE_TIMEOUT).await {
        Ok((compiled, _)) => compiled,
        Err(e) => return (status_of(&e), e.message).into_response(),
    };
    let Some(page) = compiled.rendered.pages.get(n - 1) else { return not_found() };
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml")),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(SVG_CSP)),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        page.clone(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nur dieser Test nutzt im Unit-Test-Lauf Sperre [`RUNS`] und Zwischenspeicher – in einem Test
    /// nacheinander, damit das Füllen des Speichers den abgelaufenen Lauf nicht verdrängt.
    #[tokio::test]
    async fn the_cache_holds_eight_stands_and_a_timed_out_run_still_fills_it() {
        let entry = |stand: &str| {
            Arc::new(Compiled {
                rendered: Rendered {
                    stand: stand.into(),
                    pages: vec![],
                    headings: vec![],
                    diagnostics: vec![],
                    print: vec![],
                    page_lines: vec![],
                },
                millis: 0,
            })
        };
        let sized = |stand: &str, bytes: usize| {
            let compiled = entry(stand);
            let mut rendered = compiled.rendered.clone();
            rendered.pages = vec!["x".repeat(bytes)];
            Arc::new(Compiled { rendered, millis: 0 })
        };
        for i in 0..CACHE_SIZE {
            cache_put(entry(&format!("unit-{i}")));
        }
        assert!(cache_get("unit-0").is_some(), "unit-0 ist noch da und rückt nach hinten");
        cache_put(entry("unit-neu"));
        assert_eq!(cache_len(), CACHE_SIZE, "nie mehr als {CACHE_SIZE} Einträge");
        assert!(cache_get("unit-0").is_some(), "zuletzt gebraucht: bleibt");
        assert!(cache_get("unit-1").is_none(), "am längsten nicht gebraucht: fliegt zuerst");

        // Bytegrenze (Runde 18a): drei Stände zu je 1000 Bytes bei 2500 erlaubten – der älteste fliegt, obwohl
        // weniger als CACHE_SIZE Stände da sind; ein einzelner zu großer Stand bleibt trotzdem.
        cache_put_within(sized("groß-1", 1000), 2500);
        cache_put_within(sized("groß-2", 1000), 2500);
        cache_put_within(sized("groß-3", 1000), 2500);
        assert!(cache_contains("groß-3") && cache_contains("groß-2"), "die beiden neuesten passen in die Grenze");
        assert!(!cache_contains("groß-1"), "der älteste große Stand fliegt wegen der Bytegrenze");
        assert!(!cache_contains("unit-0"), "auch kleine ältere Stände weichen, bis die Summe passt");
        cache_put_within(sized("riesig", 5000), 2500);
        assert!(cache_contains("riesig"), "der neueste Stand bleibt, auch wenn er allein über der Grenze liegt");
        assert_eq!(cache_len(), 1, "alles Ältere ist gewichen: {}", cache_len());

        // Der Lauf mit Limit 0 beginnt sicher (niemand sonst hält die Sperre), läuft ab und rechnet im
        // Hintergrund zu Ende; der nächste Aufruf wartet auf die Sperre und findet den Stand.
        let source = Source { main: "= Zeitlimit\nNur für den Unit-Test.\n".into(), files: Vec::new() };
        let stand = baruch_render::stand(&source);
        let refused = compile(source.clone(), &stand, Duration::ZERO).await.err().expect("Limit 0 läuft ab");
        assert_eq!(refused.code, REFUSED, "Zeitüberschreitung → REFUSED");
        assert_eq!(refused.message, "Typst brauchte länger als 0 s", "Meldung nennt das Limit");
        let (compiled, cached) = compile(source, &stand, COMPILE_TIMEOUT).await.expect("mit 60 s gelingt es");
        assert!(cached, "der abgelaufene Lauf hat zu Ende gerechnet und den Stand abgelegt");
        assert_eq!(compiled.rendered.pages.len(), 1, "eine Seite");
    }

    /// Runde 29: Konflikte und erkannte Vorlagenregel stehen im Abschnitt `#druckpruefung` (ohne JavaScript), mit
    /// Wirkung, Zeile, maskiertem Regeltext und Link zu den Satz-Einstellungen. Gespeicherte Einstellungen (und damit
    /// echte Konflikte auf der Seite) gibt es erst mit satz-daten; die Abnahme prüft den ganzen Weg.
    #[test]
    fn the_print_check_lists_template_conflicts_and_the_detected_rule() {
        let template = "#set page(paper: \"a4\")\n#show heading.where(level: 1): it => { pagebreak(weak: true); it }\n";
        let settings = TypesetSettings {
            page_format: Some(baruch_typst::typeset::PageFormat::A5),
            chapter_start: Some(baruch_typst::typeset::ChapterStart::Continuous),
            ..Default::default()
        };
        let conflicts: Vec<Value> = baruch_typst::typeset::template_conflicts(template, &settings)
            .into_iter()
            .map(|c| json!({ "field": c.field, "line": c.line, "text": c.text, "effect": c.effect, "message": c.message }))
            .collect();
        let rule = baruch_typst::typeset::detect_chapter_break(template).expect("Regel erkannt");
        let ctx = minijinja::context! {
            settings => sv(Settings::from_cookie_header(None)),
            work => sv(json!({ "id": 7, "title": "Erfunden" })),
            back => href("/?work=7".into()),
            restart => href("/werke/7/satz".into()),
            total => 1usize, shown => 1usize, errors => 0usize, warnings => 0usize, cached => true, millis => 0u64,
            chapter => sv(json!({ "first": 1, "last": 1 })),
            chapter_pages => sv(Vec::<Value>::new()), diagnostics => sv(Vec::<Value>::new()),
            print => sv(Vec::<Value>::new()), missing => sv(Vec::<String>::new()), broken => sv(Vec::<Value>::new()),
            imprint => sv(Vec::<Value>::new()), images_href => href("/werke/7/bilder".into()), min_dpi => 300u64,
            conflicts => sv(conflicts),
            template_rule => sv(json!({ "line": rule.line, "text": rule.text, "chapter_start": rule.chapter_start.id(),
                                        "label": rule.chapter_start.label(), "applied": false })),
            typeset_settings_href => href("/einstellungen/werk?work=7#satz".into()),
        };
        let html = templates().get_template("typeset.html").unwrap().render(ctx).expect("Vorlage rendert");
        let section = html.split("id=\"druckpruefung\"").nth(1).expect("Druckprüfung");
        assert!(
            section.contains("data-field=\"page_format\" data-effect=\"overrides\""),
            "Seitenformat: die Einstellung gilt hinter der Vorlage: {section}"
        );
        assert!(
            section.contains("data-field=\"chapter_start\" data-effect=\"ineffective\""),
            "fortlaufend gegen den Umbruch der Vorlage: wirkt nicht: {section}"
        );
        assert!(
            section.contains("die Einstellung wirkt nicht, solange die Zeile in der Vorlage steht"),
            "Meldung: {section}"
        );
        assert!(section.contains("<code>#set page(paper: &quot;a4&quot;)</code>"), "Zeilentext maskiert: {section}");
        assert!(section.contains("data-chapter-start=\"new_page\""), "erkannte Regel ohne to: neue Seite: {section}");
        assert!(
            section.contains("href=\"/einstellungen/werk?work=7#satz\""),
            "Link zu den Satz-Einstellungen: {section}"
        );
        assert!(
            !section.contains("Alle Bilder sind da"),
            "mit Hinweisen nicht die Meldung „alles in Ordnung“: {section}"
        );
    }

    #[test]
    fn limits_are_named_in_seconds() {
        assert_eq!(seconds(COMPILE_TIMEOUT), "60 s", "die Meldung nennt das Zeitlimit wie im Vertrag");
        assert_eq!(seconds(Duration::from_millis(50)), "0,050 s", "kurze Limits mit Dezimalkomma");
    }

    #[test]
    fn unsafe_addresses_fall_back_to_the_work_list() {
        assert_eq!(
            href("/werke/1/satz?page=2&key=3".into()).to_string(),
            "/werke/1/satz?page=2&amp;key=3",
            "& maskiert"
        );
        assert_eq!(href("/x\"onload".into()).to_string(), "/werke", "ein Anführungszeichen ergibt die sichere Adresse");
    }
}
