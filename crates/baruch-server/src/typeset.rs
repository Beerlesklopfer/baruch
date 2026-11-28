// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst preview with the work template (contract Round 8, section 4c): the whole work typeset with the
//! embedded Typst compiler ([`baruch_render`]) into SVG pages.
//!
//! - **Source:** [`baruch_store::Store::export_map`] (byte-identical with `work.export`, that is with
//!   work template and outline scheme) and the images from `Store::export_assets` as virtual
//!   files. The map leads every message through its line back to the section (fixed
//!   key, „2.1 Titel“, line in the section) or to „Werkvorlage“.
//! - **Run:** only in `spawn_blocking`, at most one at a time ([`RUNS`]), with the time limit
//!   [`COMPILE_TIMEOUT`]; after that the server answers with `REFUSED` „Typst brauchte länger als 60 s“.
//!   After the time limit the run computes to the end in the background, holds the lock that long and still puts
//!   its result into the cache.
//! - **Cancelling (Round 18a):** every run for a work (typesetting page, SVG page, `preview.typst`, `typst_check`)
//!   stands with its [`baruch_render::Cancel`] in a process-wide list ([`cancel`], [`cancel_in`]); RPC
//!   `preview.cancel` and the form `POST /werke/{id}/satz/abbrechen` cancel all runs of the work, also
//!   those that still wait for the lock or go on computing in the background after the time limit. The waiting
//!   request returns at once with `REFUSED` [`CANCELLED`]; Typst itself is interrupted only before the run, between
//!   typesetting and SVG and before every SVG page (not through world accesses – that breaks comemo, see
//!   `baruch_render`); evaluation and layout therefore run to the end and hold the lock until then.
//!   A cancelled run puts nothing into the cache.
//! - **Foreign pages (Round 18a):** `GET /werke/{id}/satz`, `…/{stand}/{n}.svg` and `/werke/{id}/pdf` answer
//!   `Sec-Fetch-Site: cross-site|same-site` with 403, without troubling Typst ([`crate::guard::cross_site_get`]).
//! - **Cache:** static, by revision state ([`baruch_render::stand`] over text and
//!   images), at most [`CACHE_SIZE`] entries and together at most [`CACHE_BYTES`] SVG bytes (Round 18a; the
//!   newest state always stays), the least recently used goes first. No field in
//!   [`AppState`]: the state is a content key, equal sources result in equal pages.
//! - **Chapter of a page (Round 18a):** a page that two chapters share counts for both – for the chapter that
//!   begins on it (its heading stands there) and for the chapter that ends on it (text from its lines
//!   stands there, [`baruch_render::Rendered::page_lines`]). Lines of headings do not count here, because the
//!   template can quote them as a running head or in the table of contents on foreign pages.
//! - **Fonts (Round 18a):** [`warm_fonts`] searches the system fonts in advance (for the start of the service).
//! - **RPC** `preview.typst {work?, key?, page?}` ([`call`]), **pages** `GET /werke/{id}/satz?key=&page=`
//!   (without JavaScript: one page large, ‹ ›, page list of the chapter, messages with a link to the
//!   section) and `GET /werke/{id}/satz/{stand}/{n}.svg` ([`pages`]); MCP `typst_check` in
//!   [`crate::mcp::typst`]. Read only: no write path, no commit.
//! - **Frame (Round 11):** [`prepare`] hands source and map after `export_map`/`export_assets` on to
//!   the frame of the work (cover, front matter; `crate::cover::frame` after the integration) and
//!   computes the state only afterwards. Typesetting view and PDF ([`crate::pdf`]) use the same
//!   [`prepare`] and therefore have the same number of pages.
//! - **Part pages (Round 14):** if part markers (`<baruch-part>`, [`baruch_typst::marker`]) stand
//!   in the work, [`prepare`] puts the rule
//!   [`baruch_typst::part_page_rule`] with the numbers of the parts ([`part_labels`]) behind work template and
//!   frame: every part alone on a right-hand page of its own. Without a part marker the source is byte-identical
//!   to the state before.
//! - **Mandatory details (Round 27):** with the front matter switched on `#druckpruefung` also names missing
//!   imprint details (`crate::cover_print::imprint_checks`: publisher/address, printer/address, year,
//!   rights notice of the Bible translation).
//! - **Print check (Round 11):** `preview.typst` and `typst_check` carry `print` (raster images below
//!   300 dpi, [`baruch_render::PrintWarning`]); the typesetting page shows them in the section `#druckpruefung`
//!   together with missing images („Bild fehlt: <Pfad>“) and the button „PDF herunterladen“.
//! - **Typeset settings (Round 29):** the rules "// Baruch: Satz" already stand in the source (`export_map`, hook in
//!   `baruch_store::snapshot::head`); [`prepare`] only hands the part page (`part_page`) on to the rule of the part
//!   pages. `#druckpruefung` names places of the work template that set a set setting themselves, and a
//!   detected chapter start rule of the template with a link to Einstellungen → Werk → Satz ([`template_hints`]).
//! - **Draft (Round 22):** [`prepare_with`] puts the watermark ([`draft_rule_for`]) behind work template and
//!   rule block before the frame, three-valued according to [`effective_draft`] (`preview.typst {draft?}`, page and
//!   SVG with `?entwurf=1|0`); without a watermark the source is byte-identical. The typesetting page carries the
//!   line `#entwurf` with „Als Entwurf herunterladen“ and „Vorschau als Entwurf/ohne Wasserzeichen“ (they do not
//!   change the setting).

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
use crate::i18n::tc;
use crate::rpc::{INTERNAL, INVALID_PARAMS, REFUSED, RpcError};
use crate::view::Settings;

/// Time limit of a Typst run.
pub const COMPILE_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum number of typeset states in the cache.
pub const CACHE_SIZE: usize = 8;

/// Maximum sum of the SVG bytes of all states in the cache (Round 18a). A book of a few hundred pages
/// easily needs 30–60 MiB as SVG (glyphs as paths); without a limit eight large states would hold several hundred
/// MiB. The newest state stays even if it alone is larger.
pub const CACHE_BYTES: usize = 192 * 1024 * 1024;

/// Message of a cancelled run (`REFUSED`), in the language of the request (Round 39a).
pub fn cancelled_text() -> String {
    tc("export_msg.typeset.cancelled", &[])
}

/// Time-out refusal (`REFUSED`, „Typst took longer than …“) in the language of the request.
pub fn timeout_text(limit: Duration) -> String {
    tc("export_msg.typeset.timeout", &[("limit", seconds(limit).into())])
}

/// Is `message` the time-out refusal of [`timeout_text`] (in German or in the language of the request)?
pub fn is_timeout(message: &str) -> bool {
    let shown = tc("export_msg.typeset.timeout", &[("limit", "\u{1}".into())]);
    let prefix = shown.split('\u{1}').next().unwrap_or("").trim_end();
    message.starts_with("Typst brauchte länger") || (!prefix.is_empty() && message.starts_with(prefix))
}

/// At most one Typst run at a time (machine load; Typst itself does not use several cores). The
/// PDF generation ([`crate::pdf`]) shares this lock.
pub(crate) static RUNS: Semaphore = Semaphore::const_new(1);

/// CSP of the SVG pages: execute nothing, load nothing; embedded images (`data:`) and
/// inline styles as typst-svg writes them are allowed.
pub const SVG_CSP: &str = "sandbox; default-src 'none'; img-src data:; style-src 'unsafe-inline'";

pub fn handles(method: &str) -> bool {
    matches!(method, "preview.typst" | "preview.cancel")
}

/// A typeset state.
pub struct Compiled {
    pub rendered: Rendered,
    /// Duration of the Typst run in milliseconds.
    pub millis: u64,
}

fn cache() -> std::sync::MutexGuard<'static, VecDeque<Arc<Compiled>>> {
    static CACHE: OnceLock<Mutex<VecDeque<Arc<Compiled>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new())).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Entry for the state; a hit moves to the back (most recently used).
fn cache_get(stand: &str) -> Option<Arc<Compiled>> {
    let mut cache = cache();
    let at = cache.iter().position(|c| c.rendered.stand == stand)?;
    let hit = cache.remove(at)?;
    cache.push_back(hit.clone());
    Some(hit)
}

/// Page `n` (1-based) of the state `stand` from the cache as SVG (Round 29, preview of the
/// typeset settings); counts as a use. Unknown state or page → `None`.
pub fn cached_page(stand: &str, n: usize) -> Option<String> {
    let compiled = cache_get(stand)?;
    compiled.rendered.pages.get(n.checked_sub(1)?).cloned()
}

fn cache_put(compiled: Arc<Compiled>) {
    cache_put_within(compiled, CACHE_BYTES);
}

/// SVG bytes of a state.
fn compiled_bytes(compiled: &Compiled) -> usize {
    compiled.rendered.pages.iter().map(String::len).sum()
}

/// Store with the byte limit `max_bytes`: at most [`CACHE_SIZE`] states and together at most `max_bytes`, the
/// least recently used goes first; the new one always stays.
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

/// Does the state lie in the cache (without counting it as used)? For tests.
#[doc(hidden)]
pub fn cache_contains(stand: &str) -> bool {
    cache().iter().any(|c| c.rendered.stand == stand)
}

// ---- Running runs and cancelling (Round 18a) ---------------------------------------------------

/// A registered run: data folder of the server (in tests many servers run in one process), work, cancel handle.
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

/// Registration of a run; drops out of the list with the last holder (request and background run).
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

/// Cancel the running typesetting of the work; `true` if one was running. Cancels the runs of this work id of all
/// servers in the process (in the service there is only one); server code takes [`cancel_in`] with its own data
/// folder.
pub fn cancel(work: WorkId) -> bool {
    cancel_matching(|r| r.work == work)
}

/// Like [`cancel`], only for the runs of the server with the data folder `owner`.
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

/// Number of the registered, not cancelled runs of the work (for tests).
#[doc(hidden)]
pub fn running_in(owner: &FsPath, work: WorkId) -> usize {
    running().iter().filter(|r| r.work == work && r.owner == owner && !r.cancel.is_cancelled()).count()
}

/// Typst runs begun per state (for tests: "no run from a foreign page"). At most 256 states; after that
/// the counting starts over.
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

/// Number of the Typst runs begun for this state (for tests).
#[doc(hidden)]
pub fn runs_of(stand: &str) -> u64 {
    run_counts().get(stand).copied().unwrap_or(0)
}

/// Returns as soon as `cancel` is cancelled (checks every 20 ms).
async fn until_cancelled(cancel: &Cancel) {
    while !cancel.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn cancelled() -> RpcError {
    RpcError::new(REFUSED, cancelled_text())
}

/// Font search in advance (Round 18a, proposal for the start in `main.rs`): loads embedded and system fonts
/// in the background, so that the first typesetting does not wait for it. Returns at once; needs a Tokio runtime.
pub fn warm_fonts() {
    tokio::task::spawn_blocking(crate::i18n::carry(|| {
        let _ = baruch_render::warm_fonts();
    }));
}

/// Number of the entries in the cache (for tests).
#[doc(hidden)]
pub fn cache_len() -> usize {
    cache().len()
}

/// The work prepared for Typst: map, loaded work (numbers, titles, parents), source, state.
pub struct Prepared {
    pub map: ExportMap,
    pub work: Work,
    pub source: Source,
    pub stand: String,
}

pub async fn prepare(state: &AppState, work: WorkId) -> Result<Prepared, StoreError> {
    prepare_with(state, work, None).await
}

/// Like [`prepare`] with the three-valued draft switch (Round 22, [`effective_draft`]): `None` = according to the
/// typeset setting, `Some(true)` = with watermark, `Some(false)` = without. The rule ([`draft_rule_for`]) stands
/// behind work template and rule block before the frame (so cover pages and front matter in the book carry it
/// too) and shifts the map like the frame; without a watermark everything stays byte-identical.
pub async fn prepare_with(state: &AppState, work: WorkId, draft: Option<bool>) -> Result<Prepared, StoreError> {
    let map = state.store.export_map(work).await?;
    let files = state.store.export_assets(work).await?.into_iter().map(|f| (f.path, f.bytes)).collect();
    let source = Source { main: map.typst.clone(), files };
    let settings = state.store.work_typeset(work).await?.map(|t| t.settings).unwrap_or_default();
    let (source, map) = match effective_draft(settings.draft.as_ref(), draft) {
        Some(d) => {
            let lang = crate::work_language::of(state, work).await;
            crate::cover::insert_parts(source, map, &draft_rule_for_in(&d, lang), "", Vec::new())
        }
        None => (source, map),
    };
    // Frame (cover, front matter) before the state: it is part of what is typeset.
    let (source, map) = frame_hook(state, work, source, map).await?;
    let loaded = state.store.load_work(work).await?;
    // Round 29: page and empty back side of the part page from the typeset settings (without settings byte-identical).
    // Finding R29: with a rule block but without `part_page` the part rule still carries the marker before the break
    // (`part_page_rule_for`), otherwise the blank page before a part would carry page number and running head.
    let (source, map) = part_pages(source, map, &loaded, &settings);
    let stand = baruch_render::stand(&source);
    Ok(Prepared { map, work: loaded, source, stand })
}

// ---- Draft (Round 22, contract §2.9) --------------------------------------------------------------

/// Effective watermark for typesetting preview, book PDF and cue sheet: `None` (switch missing) = the stored
/// setting, `Some(true)` = the stored values or [`Draft::default`], `Some(false)` = none. The switch never changes
/// the setting itself.
pub fn effective_draft(stored: Option<&Draft>, choice: Option<bool>) -> Option<Draft> {
    match choice {
        None => stored.cloned(),
        Some(true) => Some(stored.cloned().unwrap_or_default()),
        Some(false) => None,
    }
}

/// Like [`effective_draft`] for the cover PDF: by setting only with `draft.cover`, otherwise only explicitly.
pub fn cover_draft(stored: Option<&Draft>, choice: Option<bool>) -> Option<Draft> {
    match choice {
        None => stored.filter(|d| d.cover).cloned(),
        other => effective_draft(stored, other),
    }
}

/// Today's date in local time in the German form („10.11.2026“, as [`draft_today_in`] with German).
pub fn draft_today() -> String {
    draft_today_in(baruch_core::WorkLanguage::De)
}

/// Today's date in local time in the form of the work language (Round 39c) – the only place where the watermark
/// gets a date: German „10.11.2026“ (`%-d.%-m.%Y`, byte-equal to the form before), English ISO 8601 „2026-11-10“
/// (`%Y-%m-%d`, no words, so no book catalog key).
pub fn draft_today_in(lang: baruch_core::WorkLanguage) -> String {
    let format = match lang {
        baruch_core::WorkLanguage::De => "%-d.%-m.%Y",
        baruch_core::WorkLanguage::En => "%Y-%m-%d",
    };
    chrono::Local::now().format(format).to_string()
}

/// Rule of the watermark with today's date if `draft.date` is set.
pub fn draft_rule_for(draft: &Draft) -> String {
    draft_rule_for_in(draft, baruch_core::WorkLanguage::De)
}

/// Like [`draft_rule_for`] in the work language `lang` (Round 39b integration): the stored default text and the
/// date line come from the book catalog (`DRAFT` in an English work); German stays byte-equal.
pub fn draft_rule_for_in(draft: &Draft, lang: baruch_core::WorkLanguage) -> String {
    let date = draft.date.then(|| draft_today_in(lang));
    baruch_typst::typeset::draft_rule_in(draft, date.as_deref(), lang)
}

/// Stored draft of the work (typeset setting `draft`).
pub async fn stored_draft(state: &AppState, work: WorkId) -> Result<Option<Draft>, StoreError> {
    Ok(state.store.work_typeset(work).await?.and_then(|t| t.settings.draft))
}

/// RPC parameter `draft`: missing/`null` → `None` (according to the setting), otherwise `true`/`false`.
pub fn draft_param(params: &Value) -> Result<Option<bool>, RpcError> {
    match params.get("draft") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(RpcError::new(INVALID_PARAMS, tc("export_msg.typeset.draft_param", &[]))),
    }
}

/// Query parameter `?entwurf=`: missing/empty → `None`, `1`/`ja`/`true` → with, `0`/`nein`/`false` → without.
pub fn draft_query(value: Option<&str>) -> Result<Option<bool>, String> {
    match value.map(str::trim) {
        None | Some("") => Ok(None),
        Some("1" | "ja" | "true") => Ok(Some(true)),
        Some("0" | "nein" | "false") => Ok(Some(false)),
        Some(other) => Err(tc("export_msg.typeset.draft_form", &[("other", other.into())])),
    }
}

/// Suffix to an address that passes the switch on: "", "entwurf=1", "entwurf=0".
pub fn draft_pair(choice: Option<bool>) -> &'static str {
    match choice {
        None => "",
        Some(true) => "entwurf=1",
        Some(false) => "entwurf=0",
    }
}

/// Lay the frame of the work around the source (cover front and back, front matter) and shift the
/// map ([`crate::cover::frame`]; without cover and front matter byte-identical).
async fn frame_hook(
    state: &AppState,
    work: WorkId,
    source: Source,
    map: ExportMap,
) -> Result<(Source, ExportMap), StoreError> {
    crate::cover::frame(state, work, source, map).await
}

/// Numbers of the part markers in the order in which they stand in the source (Round 14): the
/// number of the part (`Node.number`, e.g. "II") if the marker is the first heading of a
/// part node; empty for a part without a number and for every other part marker (such as one in the
/// text of a chapter). No marker → empty.
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

/// Part pages (Round 14): if part markers stand in the source, the rule [`baruch_typst::part_page_rule`]
/// with the numbers from [`part_labels`] comes behind work template and
/// frame (before the first section): every part alone on a right-hand page of its own. Byte ranges and first
/// lines of the sections move along (as for the frame). Without a part marker everything stays byte-identical.
/// Round 29: the rule comes from [`baruch_typst::typeset::part_page_rule_for`] – with `part_page` page and empty
/// back side, with a rule block the marker before the break, without typeset settings byte-identical to
/// `part_page_rule`.
fn part_pages(source: Source, map: ExportMap, work: &Work, settings: &TypesetSettings) -> (Source, ExportMap) {
    let labels = part_labels(&map, work);
    if labels.is_empty() {
        return xref_safety(source, map, work);
    }
    let rule = baruch_typst::typeset::part_page_rule_for(&labels, settings);
    let (source, map) = crate::cover::insert_parts(source, map, &rule, "", Vec::new());
    // Round 30a: the anchor line of a part (`#metadata("baruch-key") <kN>`) would lie behind the break of the part
    // rule on the following page; in typesetting it therefore stands before the part line
    // (`part_anchors_for_typesetting`).
    let (source, map) = rewrite_sections(source, map, |_, text| baruch_typst::xref::part_anchors_for_typesetting(text));
    xref_safety(source, map, work)
}

/// Round 30a: broken cross-references (target without a current revision, `crate::xref::broken_in`) would break
/// the whole typesetting („label does not exist“). In typesetting they become the mere link text; the print check
/// names them.
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

/// Rewrite the text per section (only in typesetting) and update byte ranges and first lines of the map. The
/// source must be the text of the map (otherwise everything stays as it is).
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
    } else if crate::i18n::current() == crate::i18n::Lang::De {
        format!("{:.3} s", limit.as_secs_f64()).replace('.', ",")
    } else {
        format!("{:.3} s", limit.as_secs_f64())
    }
}

/// Typesets the source or takes the state from the cache (`true` = hit). At most one
/// run at a time; the time limit counts from the waiting for the lock. Not registered (no work, e.g. the
/// cover): cannot be cancelled through `preview.cancel`.
pub async fn compile(source: Source, stand: &str, limit: Duration) -> Result<(Arc<Compiled>, bool), RpcError> {
    compile_run(source, stand, limit, Cancel::new(), None).await
}

/// Like [`compile`], registered as a run of the work `work` of this server (can be cancelled through [`cancel_in`]).
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
            permit = RUNS.acquire() => permit.map_err(|e| RpcError::new(INTERNAL, tc("export_msg.typeset.lock", &[("error", e.to_string().into())])))?,
            () = until_cancelled(&cancel) => return Err(cancelled()),
        };
        // While waiting, another run can have typeset exactly this state.
        if let Some(hit) = cache_get(&stand) {
            return Ok((hit, true));
        }
        if cancel.is_cancelled() {
            return Err(cancelled());
        }
        note_run(&stand);
        let run_cancel = cancel.clone();
        let run_registration = registration.clone();
        let handle = tokio::task::spawn_blocking(crate::i18n::carry(move || {
            // Lock and registration belong to the run, not to the request: if the time runs out, Typst computes
            // to the end (staying cancellable) and releases the lock only then.
            let _permit = permit;
            let _registration = run_registration;
            let started = Instant::now();
            let rendered = baruch_render::compile_cancellable(&source, &run_cancel)?;
            let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let compiled = Arc::new(Compiled { rendered, millis });
            cache_put(compiled.clone());
            Some(compiled)
        }));
        // Cancelled: the request returns at once, even if Typst still computes up to the next check point.
        tokio::select! {
            joined = handle => match joined.map_err(|e| RpcError::new(INTERNAL, tc("export_msg.typeset.crashed", &[("error", e.to_string().into())])))? {
                Some(compiled) => Ok((compiled, false)),
                None => Err(cancelled()),
            },
            () = until_cancelled(&cancel) => Err(cancelled()),
        }
    };
    let begun = Instant::now();
    // `tokio::time::timeout` polls the run first: if it is finished faster than the timer fires, `Ok`
    // also comes after the limit has expired (always possible with 0). Therefore the measured time counts in
    // addition; the finished typesetting then already lies in the cache.
    match tokio::time::timeout(limit, run).await {
        Ok(_) if begun.elapsed() >= limit => Err(RpcError::new(REFUSED, timeout_text(limit))),
        Ok(result) => result,
        Err(_) => Err(RpcError::new(REFUSED, timeout_text(limit))),
    }
}

/// Result for RPC, page and MCP.
pub struct Typeset {
    pub prepared: Prepared,
    pub compiled: Arc<Compiled>,
    pub cached: bool,
}

/// Prepare the work and typeset it (with the time limit `limit`). Tests call this with a short limit.
#[doc(hidden)]
pub async fn typeset_with_limit(state: &AppState, work: WorkId, limit: Duration) -> Result<Typeset, RpcError> {
    typeset_draft(state, work, limit, None).await
}

/// Like [`typeset_with_limit`] with the draft switch (Round 22, [`prepare_with`]).
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

/// Only from the cache (Round 18a, page after „Satz abbrechen“): the current state if it is already
/// typeset, otherwise `None` – without a Typst run.
pub async fn cached_typeset(state: &AppState, work: WorkId) -> Result<Option<Typeset>, RpcError> {
    cached_typeset_draft(state, work, None).await
}

/// Like [`cached_typeset`] with the draft switch.
pub async fn cached_typeset_draft(
    state: &AppState,
    work: WorkId,
    draft: Option<bool>,
) -> Result<Option<Typeset>, RpcError> {
    let prepared = prepare_with(state, work, draft).await?;
    Ok(cache_get(&prepared.stand).map(|compiled| Typeset { prepared, compiled, cached: true }))
}

/// Like [`typeset`], but checks before the run whether the section `key` exists (otherwise
/// `INVALID_PARAMS`, without troubling Typst).
pub async fn typeset_at(state: &AppState, work: WorkId, key: Option<i64>) -> Result<Typeset, RpcError> {
    typeset_at_draft(state, work, key, None).await
}

/// Like [`typeset_at`] with the draft switch (`preview.typst {draft?}`).
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

// ---- Sections, chapters, messages --------------------------------------------------------------

/// Display name of a section: „2.1 Titel“ (without a number only the title).
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

    /// Page (1-based) on which the section `index` of the map begins: that of its first
    /// heading; without a heading of its own the last one before; otherwise 1.
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

    /// Position of the section with the key `key` in the map.
    fn index_of(&self, key: i64) -> Option<usize> {
        self.prepared.map.sections.iter().position(|s| s.key == key)
    }

    /// Chapter of the section `key`: the topmost outline item above it (under a part the
    /// chapter in it). Pages `first..=last`: from the page of its heading up to before the page of the
    /// next item outside (at least `first`), at the end up to the last page. Without `key`:
    /// the whole work.
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
                break; // loop: do not run endlessly
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
                // The page on which the next chapter begins also belongs to this one if text from its
                // lines stands on it (shared page, see the module header).
                let page = self.section_page(next);
                let lines = sections[start].first_line..sections[next].first_line;
                if self.has_text_from(page, lines) { page } else { page.saturating_sub(1) }.max(first)
            }
            None => total,
        };
        (first, last.min(total).max(first))
    }

    /// Does text from the lines `lines` of `main` stand on page `page` (1-based), except heading lines?
    fn has_text_from(&self, page: usize, lines: std::ops::Range<usize>) -> bool {
        let rendered = self.rendered();
        let Some(on_page) = page.checked_sub(1).and_then(|i| rendered.page_lines.get(i)) else { return false };
        on_page.iter().any(|line| lines.contains(line) && !rendered.headings.iter().any(|h| h.line == *line))
    }

    /// Page of the section `key` (without or unknown: 1).
    pub fn page_of(&self, key: Option<i64>) -> usize {
        key.and_then(|k| self.index_of(k)).map_or(1, |i| self.section_page(i))
    }

    /// Messages with section: `key`, `section` („2.1 Titel“, „Werkvorlage“ or – Round 23a – [`TAIL_LABEL`] for
    /// lines of the book end) and `section_line` (line in the section, in the template or in the book end, 1-based).
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
                        // Round 23a: lines behind the last section belong to the book end, not to it.
                        None => match map.tail_first_line().filter(|first| line >= *first) {
                            Some(first) => (None, Some(tail_label()), Some(line - first + 1)),
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

/// Round 23a: name of the book end in typesetting messages (index, glossary and lists behind the last section).
pub fn tail_label() -> String {
    tc("export_msg.typeset.tail", &[])
}

/// A message for RPC, MCP and page.
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

/// Fixed key as a number or digits (forms); empty → `None`.
fn key_value(value: Option<&Value>, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, tc("export_msg.typeset.number", &[("name", name.into())]));
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Does the section `key` exist in the work? Otherwise `INVALID_PARAMS`.
fn check_key(work: &Work, work_id: WorkId, key: Option<i64>) -> Result<(), RpcError> {
    match key {
        Some(key) if !work.nodes.iter().any(|n| n.key == key) => Err(RpcError::new(
            INVALID_PARAMS,
            tc("export_msg.typeset.no_section", &[("key", key.into()), ("work", work_id.into())]),
        )),
        _ => Ok(()),
    }
}

/// `preview.typst {work?, key?, page?, draft?}` → `{stand, pages, page, chapter: {first, last},
/// diagnostics, cached, millis, draft, svg?}` (Round 22: `draft` three-valued; answer `draft` = watermark in the
/// typesetting). `page` is the requested page, otherwise the page of the
/// section `key` (without `key`: 1); `svg` only with `page`.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    if !handles(method) {
        return Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {method}")));
    }
    if method == "preview.cancel" {
        // Unknown work: the same message as everywhere (`UnknownWork` → -32602), not silently `false`.
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
            .ok_or_else(|| {
                RpcError::new(
                    INVALID_PARAMS,
                    tc("export_msg.typeset.no_page", &[("page", page.into()), ("total", total.into())]),
                )
            })?;
        answer["page"] = json!(page);
        answer["svg"] = json!(svg);
    }
    Ok(answer)
}

// ---- Pages -------------------------------------------------------------------------------------

/// `GET /werke/{id}/satz?key=&page=` and `GET /werke/{id}/satz/{stand}/{n}.svg` (read only).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/satz", get(page)).route("/werke/{id}/satz/{stand}/{file}", get(svg))
}

/// `POST /werke/{id}/satz/abbrechen` (behind guard::same_origin).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/satz/abbrechen", post(cancel_act))
}

/// Form „Satz abbrechen“ of the ribbon of the typesetting page: cancels the runs of the work and redirects with
/// 303 to `/werke/{id}/satz?abgebrochen=1` (none was running: `abgebrochen=0`; with the field `key` `&key=K` is
/// appended). The page then does not typeset anew, but only shows a state already typeset (see [`page`]).
async fn cancel_act(
    State(state): State<Arc<AppState>>,
    Path(id): Path<WorkId>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    match state.store.works().await {
        Ok(works) if works.iter().any(|(w, _, _)| *w == id) => {}
        Ok(_) => return (StatusCode::NOT_FOUND, tc("export_msg.page.unknown_work", &[])).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, typeset_error(e)).into_response(),
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

/// 403 for a GET that a foreign page has triggered (Round 18a): images, links and forms of other
/// pages are not to start a Typst run.
pub(crate) fn foreign_get() -> Response {
    (StatusCode::FORBIDDEN, tc("export_msg.typeset.foreign", &[])).into_response()
}

/// „Fehler: …“ in the language of the request.
fn typeset_error(error: impl std::fmt::Display) -> String {
    tc("export_msg.page.error", &[("error", error.to_string().into())])
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            // Round 33: the controls of the typesetting page stand in the ribbon (block band_buch in typeset.html);
            // fragments/edit_toolbar.html is no longer included here.
            ("typeset.html", include_str!("../templates/typeset.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Satz-Seite fehlerhaft");
        }
        env
    })
}

/// Self-built address (digits, hex state): escape only `&`, so that `/` stays readable.
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
    /// After the form „Satz abbrechen“: `1` (cancelled) or `0` (none was running) – then no new run.
    abgebrochen: Option<String>,
    /// Round 22: watermark three-valued (missing = according to the setting, `1` = with, `0` = without).
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

/// Serde value for the template (minijinja 3 takes own types only this way).
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
    // The typesetting page loads no main.js: the header in base.html then shows only what works without a widget.
    let ctx = minijinja::context! { scriptless => true, ..ctx };
    match templates().get_template("typeset.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            tc(
                "export_msg.typeset.template_error",
                &[("name", "typeset.html".into()), ("error", e.to_string().into())],
            ),
        )
            .into_response(),
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
    // Round 22: an unreadable `entwurf` counts like "missing" (the page stays usable).
    let draft = draft_query(q.entwurf.as_deref()).unwrap_or(None);
    let pair = draft_pair(draft);
    let title = match state.store.works().await {
        Ok(works) => match works.into_iter().find(|(w, _, _)| *w == id) {
            Some((_, _, title)) => title,
            None => return (StatusCode::NOT_FOUND, tc("export_msg.page.unknown_work", &[])).into_response(),
        },
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, typeset_error(e)).into_response(),
    };
    let back = href(match key {
        Some(key) => format!("/?work={id}&key={key}"),
        None => format!("/?work={id}"),
    });
    let work_ctx = json!({ "id": id, "title": title });
    // After „Satz abbrechen“: do not typeset anew, only show a state already typeset.
    let cancelled_notice = q.abgebrochen.as_deref().map(|flag| {
        if flag == "0" {
            tc("export_msg.typeset.nothing_to_cancel", &[])
        } else {
            tc("export_msg.typeset.was_cancelled", &[])
        }
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
            let cancelled = e.message == cancelled_text();
            let ctx = minijinja::context! {
                settings, work => sv(work_ctx), back, key, restart,
                error => (!cancelled).then(|| e.message.clone()),
                cancelled_notice => cancelled.then(|| tc("export_msg.typeset.was_cancelled", &[])),
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

/// Round 22: links of the typesetting page around the watermark (without JavaScript): „Als Entwurf herunterladen“
/// (book PDF with watermark), plus the preview with or without watermark (depending on what is shown right now)
/// and the PDF without watermark if the setting provides for one. None changes the setting.
#[derive(Serialize)]
struct DraftLinks {
    /// `/werke/{id}/pdf?entwurf=1`.
    pdf_draft: JinjaValue,
    /// `/werke/{id}/pdf?entwurf=0` (only if the setting provides for a watermark).
    pdf_plain: Option<JinjaValue>,
    /// Toggle the preview: address and label.
    toggle: JinjaValue,
    toggle_label: String,
}

fn draft_links(id: WorkId, key: Option<i64>, draft: Option<bool>, stored: bool) -> DraftLinks {
    let shown = draft.unwrap_or(stored);
    let key_part = key.map(|k| format!("key={k}&")).unwrap_or_default();
    let (flag, label) = if shown {
        ("0", tc("export_msg.typeset.preview_plain", &[]))
    } else {
        ("1", tc("export_msg.typeset.preview_draft", &[]))
    };
    DraftLinks {
        pdf_draft: href(format!("/werke/{id}/pdf?entwurf=1")),
        pdf_plain: stored.then(|| href(format!("/werke/{id}/pdf?entwurf=0"))),
        toggle: href(format!("/werke/{id}/satz?{key_part}entwurf={flag}")),
        toggle_label: label,
    }
}

/// Round 29 (section `#druckpruefung`): places of the work template that set typeset settings that are set
/// themselves ([`baruch_typst::typeset::template_conflicts`], each `{field, label, line, text, effect, message}`),
/// and a detected chapter start rule of the template (`{line, text, chapter_start, label}`) that can be adopted in
/// the settings. A read error shows nothing (the page stays usable).
pub async fn template_hints(state: &AppState, work: WorkId) -> (Vec<Value>, Option<Value>) {
    let Ok(template) = state.store.work_template(work).await else { return (Vec::new(), None) };
    let settings: TypesetSettings =
        state.store.work_typeset(work).await.ok().flatten().map(|t| t.settings).unwrap_or_default();
    let conflicts = baruch_typst::typeset::template_conflicts(&template, &settings)
        .into_iter()
        .map(|c| {
            json!({
                "field": c.field,
                "label": ui_labels::field_label(crate::i18n::current(), c.field),
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
            "label": ui_labels::choice_label(crate::i18n::current(), "chapter_start", r.chapter_start.id()),
            "applied": settings.chapter_start == Some(r.chapter_start),
        })
    });
    (conflicts, rule)
}

/// Broken cross-references for the print check (Round 30a, [`crate::xref::broken_in`]): section, jump mark,
/// link text and link to the place. In typesetting they stand only as text (see `xref_safety`).
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

/// Mandatory details in the imprint (Round 27, [`crate::cover_print::imprint_checks`]) for the section
/// `#druckpruefung`: only with the front matter switched on; a read error shows nothing (the page stays usable).
async fn imprint_warnings(state: &AppState, work: WorkId) -> Vec<crate::cover_print::CoverWarning> {
    let (Ok(meta), Ok(w)) = (state.store.book_meta(work).await, state.store.work_meta(work).await) else {
        return Vec::new();
    };
    crate::cover_print::imprint_checks(&meta, &w.bible)
}

/// Messages with label („Fehler“/„Warnung“) and link to the section (typesetting page, PDF error page).
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

/// Image paths that template or text name through `image("…")` without a file in the work (normal form,
/// sorted, without duplicates). Paths without a normal form (`..`) count as missing, as they stand in the text.
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
    (StatusCode::NOT_FOUND, tc("export_msg.typeset.page_gone", &[])).into_response()
}

#[derive(Deserialize)]
struct SvgQuery {
    entwurf: Option<String>,
}

/// One page as SVG, only for the current state of the work (foreign or old state → 404).
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
    // Round 22: the same switch as the typesetting page, otherwise the state does not match.
    let draft = draft_query(q.entwurf.as_deref()).unwrap_or(None);
    let prepared = match prepare_with(&state, id, draft).await {
        Ok(prepared) => prepared,
        Err(StoreError::UnknownWork(_)) => return not_found(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, typeset_error(e)).into_response(),
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

// ---- Labels in the interface language (Round 39b) --------------------------------------------------------------

/// Labels of the typesetting settings in the interface language. `baruch-typst` keeps its German labels
/// (`FIELDS`, the `label()` of every choice; the mirror quotes them), the pages translate them here by identifier.
pub mod ui_labels {
    use crate::i18n::{self, Lang};

    /// Catalog key of a field (for subfields such as `margins.inside_mm` that of the field).
    fn field_key(field: &str) -> Option<&'static str> {
        Some(match field.split('.').next().unwrap_or(field) {
            "chapter_start" => "settings.typeset.field.chapter_start",
            "part_page" => "settings.typeset.field.part_page",
            "page_format" => "settings.typeset.field.page_format",
            "margins" => "settings.typeset.field.margins",
            "font" => "settings.typeset.field.font",
            "font_size_pt" => "settings.typeset.field.font_size_pt",
            "leading_em" => "settings.typeset.field.leading_em",
            "paragraph" => "settings.typeset.field.paragraph",
            "justify" => "settings.typeset.field.justify",
            "hyphenate" => "settings.typeset.field.hyphenate",
            "lang" => "settings.typeset.field.lang",
            "page_numbers" => "settings.typeset.field.page_numbers",
            "running_head" => "settings.typeset.field.running_head",
            "footnotes" => "settings.typeset.field.footnotes",
            "widows_orphans" => "settings.typeset.field.widows_orphans",
            "xref_with_page" => "settings.typeset.field.xref_with_page",
            "glossary_page" => "settings.typeset.field.glossary_page",
            "index_pages" => "settings.typeset.field.index_pages",
            "glossary_first_mention" => "settings.typeset.field.glossary_first_mention",
            "draft" => "settings.typeset.field.draft",
            _ => return None,
        })
    }

    /// Label of a field in `lang` (German: verbatim `baruch_typst::typeset::field_label`); unknown → "".
    pub fn field_label(lang: Lang, field: &str) -> String {
        field_key(field).map_or_else(String::new, |key| i18n::t(lang, key, &[]))
    }

    /// Catalog key of a choice: `group` names the selection list (as in the form), `id` the serde value.
    fn choice_key(group: &str, id: &str) -> Option<&'static str> {
        Some(match (group, id) {
            ("chapter_start", "continuous") => "settings.typeset.choice.chapter_start.continuous",
            ("chapter_start", "new_page") => "settings.typeset.choice.chapter_start.new_page",
            ("chapter_start", "right_page") => "settings.typeset.choice.chapter_start.right_page",
            ("part_side", "right") => "settings.typeset.choice.part_side.right",
            ("part_side", "any") => "settings.typeset.choice.part_side.any",
            ("page_number_position", "bottom_center") => "settings.typeset.choice.page_number_position.bottom_center",
            ("page_number_position", "bottom_outside") => "settings.typeset.choice.page_number_position.bottom_outside",
            ("page_number_position", "top_outside") => "settings.typeset.choice.page_number_position.top_outside",
            ("page_number_position", "hidden") => "settings.typeset.choice.page_number_position.hidden",
            ("page_number_start", "first_page") => "settings.typeset.choice.page_number_start.first_page",
            ("page_number_start", "first_chapter") => "settings.typeset.choice.page_number_start.first_chapter",
            ("running_head", "off") => "settings.typeset.choice.running_head.off",
            ("running_head", "work_title") => "settings.typeset.choice.running_head.work_title",
            ("running_head", "chapter") => "settings.typeset.choice.running_head.chapter",
            ("footnote_numbering", "arabic") => "settings.typeset.choice.footnote_numbering.arabic",
            ("footnote_numbering", "symbols") => "settings.typeset.choice.footnote_numbering.symbols",
            ("footnote_numbering", "letters") => "settings.typeset.choice.footnote_numbering.letters",
            ("footnote_restart", "continuous") => "settings.typeset.choice.footnote_restart.continuous",
            ("footnote_restart", "per_chapter") => "settings.typeset.choice.footnote_restart.per_chapter",
            ("glossary_page", "before_indexes") => "settings.typeset.choice.glossary_page.before_indexes",
            ("glossary_page", "after_indexes") => "settings.typeset.choice.glossary_page.after_indexes",
            ("draft_placement", "diagonal") => "settings.typeset.choice.draft_placement.diagonal",
            ("draft_placement", "header") => "settings.typeset.choice.draft_placement.header",
            ("draft_placement", "footer") => "settings.typeset.choice.draft_placement.footer",
            ("paragraph", "indent") => "settings.typeset.choice.paragraph.indent",
            ("paragraph", "spacing") => "settings.typeset.choice.paragraph.spacing",
            ("page_format", "a5") => "settings.typeset.choice.page_format.a5",
            ("page_format", "120x190") => "settings.typeset.choice.page_format.120x190",
            ("page_format", "135x215") => "settings.typeset.choice.page_format.135x215",
            ("page_format", "a4") => "settings.typeset.choice.page_format.a4",
            ("page_format", "custom") => "settings.typeset.choice.page_format.custom",
            _ => return None,
        })
    }

    /// Label of a choice in `lang` (German: verbatim the `label()` of the crate); unknown → the identifier.
    pub fn choice_label(lang: Lang, group: &str, id: &str) -> String {
        choice_key(group, id).map_or_else(|| id.to_owned(), |key| i18n::t(lang, key, &[]))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use baruch_typst::typeset::{
            ChapterStart, DraftPlacement, FIELDS, FootnoteNumbering, FootnoteRestart, GlossaryPosition, PageFormat,
            PageNumberPosition, PageNumberStart, Paragraph, PartSide, RunningHead,
        };

        #[test]
        fn german_labels_are_verbatim_the_labels_of_baruch_typst() {
            for (id, label) in FIELDS {
                assert_eq!(field_label(Lang::De, id), label, "German pages keep the field label of the crate");
                let english = field_label(Lang::En, id);
                assert!(
                    !english.is_empty() && !english.starts_with("settings."),
                    "{id}: an English label exists (not the raw key)"
                );
            }
            assert_eq!(field_label(Lang::De, "margins.inside_mm"), "Ränder", "a subfield takes the label of its field");
            for v in ChapterStart::ALL.iter().copied() {
                assert_eq!(choice_label(Lang::De, "chapter_start", v.id()), v.label(), "ChapterStart: German verbatim");
            }
            for v in PartSide::ALL.iter().copied() {
                assert_eq!(choice_label(Lang::De, "part_side", v.id()), v.label(), "PartSide: German verbatim");
            }
            for v in PageNumberPosition::ALL.iter().copied() {
                assert_eq!(
                    choice_label(Lang::De, "page_number_position", v.id()),
                    v.label(),
                    "PageNumberPosition: German verbatim"
                );
            }
            for v in PageNumberStart::ALL.iter().copied() {
                assert_eq!(
                    choice_label(Lang::De, "page_number_start", v.id()),
                    v.label(),
                    "PageNumberStart: German verbatim"
                );
            }
            for v in RunningHead::ALL.iter().copied() {
                assert_eq!(choice_label(Lang::De, "running_head", v.id()), v.label(), "RunningHead: German verbatim");
            }
            for v in FootnoteNumbering::ALL.iter().copied() {
                assert_eq!(
                    choice_label(Lang::De, "footnote_numbering", v.id()),
                    v.label(),
                    "FootnoteNumbering: German verbatim"
                );
            }
            for v in FootnoteRestart::ALL.iter().copied() {
                assert_eq!(
                    choice_label(Lang::De, "footnote_restart", v.id()),
                    v.label(),
                    "FootnoteRestart: German verbatim"
                );
            }
            for v in GlossaryPosition::ALL.iter().copied() {
                assert_eq!(
                    choice_label(Lang::De, "glossary_page", v.id()),
                    v.label(),
                    "GlossaryPosition: German verbatim"
                );
            }
            for v in DraftPlacement::ALL.iter().copied() {
                assert_eq!(
                    choice_label(Lang::De, "draft_placement", v.id()),
                    v.label(),
                    "DraftPlacement: German verbatim"
                );
            }
            for v in Paragraph::ALL.iter().copied() {
                assert_eq!(choice_label(Lang::De, "paragraph", v.id()), v.label(), "Paragraph: German verbatim");
            }
            for v in PageFormat::ALL.iter().copied() {
                assert_eq!(choice_label(Lang::De, "page_format", v.id()), v.label(), "PageFormat: German verbatim");
            }
            assert_eq!(
                choice_label(Lang::De, "page_format", "custom"),
                PageFormat::Custom { width_mm: 1.0, height_mm: 1.0 }.label(),
                "the own format is verbatim too"
            );
        }

        #[test]
        fn english_labels_carry_no_german_letter() {
            for (id, _) in FIELDS {
                let text = field_label(Lang::En, id);
                assert!(!text.contains(['ä', 'ö', 'ü', 'ß']), "{id}: the English label is English: {text}");
            }
            assert_eq!(
                choice_label(Lang::En, "draft_placement", "diagonal"),
                "diagonally across the page",
                "a choice in English"
            );
            assert_eq!(choice_label(Lang::En, "nope", "x"), "x", "an unknown choice shows its identifier");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only this test uses the lock [`RUNS`] and the cache in the unit test run – in one test
    /// one after another, so that filling the cache does not push out the expired run.
    #[tokio::test]
    async fn the_cache_holds_eight_stands_and_a_timed_out_run_still_fills_it() {
        crate::i18n::scope(crate::i18n::Lang::De, async {
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

            // Byte limit (Round 18a): three states of 1000 bytes each with 2500 allowed – the oldest goes although
            // fewer than CACHE_SIZE states are there; a single state that is too large stays nevertheless.
            cache_put_within(sized("groß-1", 1000), 2500);
            cache_put_within(sized("groß-2", 1000), 2500);
            cache_put_within(sized("groß-3", 1000), 2500);
            assert!(cache_contains("groß-3") && cache_contains("groß-2"), "die beiden neuesten passen in die Grenze");
            assert!(!cache_contains("groß-1"), "der älteste große Stand fliegt wegen der Bytegrenze");
            assert!(!cache_contains("unit-0"), "auch kleine ältere Stände weichen, bis die Summe passt");
            cache_put_within(sized("riesig", 5000), 2500);
            assert!(cache_contains("riesig"), "der neueste Stand bleibt, auch wenn er allein über der Grenze liegt");
            assert_eq!(cache_len(), 1, "alles Ältere ist gewichen: {}", cache_len());

            // The run with limit 0 certainly begins (nobody else holds the lock), expires and computes to the end
            // in the background; the next call waits for the lock and finds the state.
            let source = Source { main: "= Zeitlimit\nNur für den Unit-Test.\n".into(), files: Vec::new() };
            let stand = baruch_render::stand(&source);
            let refused = compile(source.clone(), &stand, Duration::ZERO).await.err().expect("Limit 0 läuft ab");
            assert_eq!(refused.code, REFUSED, "Zeitüberschreitung → REFUSED");
            assert_eq!(refused.message, "Typst brauchte länger als 0 s", "Meldung nennt das Limit");
            let (compiled, cached) = compile(source, &stand, COMPILE_TIMEOUT).await.expect("mit 60 s gelingt es");
            assert!(cached, "der abgelaufene Lauf hat zu Ende gerechnet und den Stand abgelegt");
            assert_eq!(compiled.rendered.pages.len(), 1, "eine Seite");
        })
        .await
    }

    /// Round 29: conflicts and the detected template rule stand in the section `#druckpruefung` (without JavaScript),
    /// with effect, line, escaped rule text and link to the typeset settings. Stored settings (and with them
    /// real conflicts on the page) exist only with satz-daten; the acceptance checks the whole path.
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
        crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            assert_eq!(seconds(COMPILE_TIMEOUT), "60 s", "die Meldung nennt das Zeitlimit wie im Vertrag");
            assert_eq!(seconds(Duration::from_millis(50)), "0,050 s", "kurze Limits mit Dezimalkomma");
        })
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
