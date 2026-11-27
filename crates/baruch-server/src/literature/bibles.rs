// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bibles in the literature (user request 2026-11-07, Round 15 „auftraege“): the SWORD Bibles stand next to the
//! titles of the library – for reading, searching, quoting and „an Persona geben“.
//!
//! - Box „Bibeln“ (popup and `/literatur`, template `fragments/literature_bibles.html`): the Bible modules from
//!   `bible.modules` (entries without `kind` count as a Bible, commentaries and lexicons do not), the Bible of the
//!   work marked and first; per Bible a link to `/literatur/bibel`.
//! - Hit groups in the common search field (wording mode only): „Bibel“ per module (module name, at most
//!   [`HITS_PER_MODULE`] hits, at most [`SEARCH_MODULES`] modules, time limit [`SEARCH_TIME`]) through
//!   [`crate::bibles::search`] and „Kommentar“ as the ready-made building block
//!   [`crate::bibles::commentary_hits_html`].
//! - `GET /literatur/bibel?work&key&rev&lq&lmode&module&ref&book&chapter&q&kom` (without JavaScript): open a Bible
//!   (list of books), browse book/chapter ([`crate::bibles::lookup`] with „Buch Kapitel“), look up a reference
//!   (`ref`, e.g. „Ps 139,23-24“), wording search (`q`). Next to a reference that was looked up the building block
//!   [`crate::bibles::commentary_panel_html`] (`kom=1` opens it). Without a Bible an honest notice with a link to
//!   Einstellungen → Quellen.
//! - `POST /literatur/bibel/act` (behind `guard::same_origin`): `action=cite` creates a Bible quote with its
//!   reference as a proposal revision (author `@literatur`, model `baruch-zitat`, like `highlight.cite`;
//!   only the human can accept it), `action=persona` gives the verses to a persona
//!   ([`super::to_persona`], source „Bibel MODUL, Stelle“). The text always comes from the module (field `ref`),
//!   never from the form. `action=highlight` does not exist: highlights belong to titles of the library (with
//!   quote and character range), a Bible reference is no such title – the page says so honestly.
//!
//! Module folders: reading happens only if module folders are set explicitly or the work has a Bible
//! ([`scan_allowed`]). This way a page without the setting never reads the default folders of the machine (tests
//! set their folders themselves); instead the notice pointing to Einstellungen → Quellen.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::{StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Value, context};
use serde::Deserialize;
use serde_json::json;

use super::{At, ToPersona, error_response, href, number, page_settings, render, text};
use crate::AppState;
use crate::bibles::Choice;
use crate::rpc::{INVALID_PARAMS, REFUSED, RpcError};
use crate::view::query_escape;

/// At most this many hits per Bible in the group „Bibel“.
pub const HITS_PER_MODULE: usize = 10;
/// The common search field searches at most this many Bibles.
pub const SEARCH_MODULES: usize = 3;
/// Time limit of the group „Bibel“ in the common search field (all modules together).
pub const SEARCH_TIME: Duration = Duration::from_secs(3);
/// Hits of the wording search on the Bible page.
pub const PAGE_HITS: usize = 50;
/// Notice without a Bible.
pub const NO_BIBLE: &str = "Keine SWORD-Bibel verfügbar: Modulordner und Bibel des Werks stellst du unter \
     Einstellungen → Quellen → Bibeln (SWORD) ein.";
/// Notice about „Hervorheben“ on the Bible page.
pub const NO_HIGHLIGHT: &str = "Hervorheben geht nur bei Titeln der Bibliothek (Zitat mit Zeichenbereich); eine \
     Bibelstelle ist kein solcher Titel. Zitieren und „an Persona geben“ gehen.";
/// Target of the link „Einstellungen → Quellen“.
pub const SOURCES_URL: &str = "/einstellungen/quellen";

/// A Bible in the box.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BibleEntry {
    pub module: String,
    pub description: String,
    pub notice: String,
    /// Bible of the work.
    pub work_bible: bool,
    pub supported: bool,
    pub problem: Option<String>,
    pub url: String,
}

/// Hits of one Bible in the common search field.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HitGroup {
    pub module: String,
    pub hits: Vec<BibleHit>,
    pub truncated: bool,
    /// Honest notice (time limit, module error).
    pub problem: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BibleHit {
    pub reference: String,
    pub text: String,
    pub url: String,
}

/// Box and hit groups for `fragments/library.html`.
#[derive(Debug, Clone, Default)]
pub struct Shelf {
    pub bibles: Vec<BibleEntry>,
    /// Notice instead of the list (no Bible, folders not set, error while reading).
    pub hint: Option<String>,
    pub groups: Vec<HitGroup>,
    /// Finished HTML of the group „Kommentar“ ([`crate::bibles::commentary_hits_html`]).
    pub commentary_hits: String,
    /// Bible page without a chosen module.
    pub page_url: String,
}

impl Shelf {
    /// Context `bibles` for the templates (addresses as safe values, the template escapes texts).
    pub fn context(&self) -> Value {
        let list: Vec<Value> = self.bibles.iter().map(entry_value).collect();
        let groups: Vec<Value> = self
            .groups
            .iter()
            .map(|g| {
                let hits: Vec<Value> = g.hits.iter().map(hit_value).collect();
                context! { module => g.module.as_str(), hits => hits, truncated => g.truncated, problem => g.problem.as_deref() }
            })
            .collect();
        context! {
            list => list,
            hint => self.hint.as_deref(),
            groups => groups,
            commentary_hits => Value::from_safe_string(self.commentary_hits.clone()),
            page_url => href(self.page_url.clone()),
            settings_url => Value::from_safe_string(SOURCES_URL.to_owned()),
        }
    }
}

impl Shelf {
    /// Round 19c: context for the work page without JavaScript ([`workspace_shelf`]): like [`Shelf::context`], but
    /// the links of the Bibles additionally carry the fixed key and the revision of the section shown (this is
    /// how „Zitieren“ and „an Persona geben“ work on the Bible page).
    pub fn context_at(&self, key: Option<i64>, rev: Option<u32>) -> Value {
        let mut place = String::new();
        if let Some(key) = key {
            place.push_str(&format!("&key={key}"));
            if let Some(rev) = rev {
                place.push_str(&format!("&rev={rev}"));
            }
        }
        let mut shelf = self.clone();
        for b in &mut shelf.bibles {
            b.url.push_str(&place);
        }
        shelf.page_url.push_str(&place);
        shelf.context()
    }
}

/// Round 19c: box „Bibeln“ for the work page (`fragments/library.html` through `view.rs`), on the server side and
/// without JavaScript – the same Bibles as in the popup (`literature.render`) and on `/literatur`, without hit
/// groups (the search of the work page leads to `/literatur`). Without allowed module folders the honest notice.
pub async fn workspace_shelf(state: &AppState, work: WorkId) -> Option<Shelf> {
    let at = At::resolve(state, Some(work), (None, None), None, None, (None, None)).await.ok()?;
    Some(shelf(state, &at, None).await)
}

fn entry_value(b: &BibleEntry) -> Value {
    context! {
        module => b.module.as_str(), description => b.description.as_str(), notice => b.notice.as_str(),
        work_bible => b.work_bible, supported => b.supported, problem => b.problem.as_deref(), url => href(b.url.clone()),
    }
}

fn hit_value(h: &BibleHit) -> Value {
    context! { reference => h.reference.as_str(), text => h.text.as_str(), url => href(h.url.clone()) }
}

/// May Baruch read the module folders? Only with explicitly set folders or a Bible of the work.
pub async fn scan_allowed(state: &AppState, work: WorkId) -> bool {
    let explicit = matches!(state.store.bible_dirs().await, Ok(Some(_)));
    explicit || matches!(state.store.work_bible(work).await, Ok(Some(_)))
}

/// Query tail of the Bible page: `&key=…&rev=…&lq=…&lmode=rag` (only values present, percent-encoded).
fn tail(at: &At) -> String {
    let mut out = String::new();
    if let Some(key) = at.key {
        out.push_str(&format!("&key={key}"));
    }
    if let Some(rev) = at.rev {
        out.push_str(&format!("&rev={rev}"));
    }
    out.push_str(&at.lq_tail());
    out
}

/// Address of the Bible page with module and further fields (`extra` begins with `&` or is empty).
fn page_url(at: &At, module: Option<&str>, extra: &str) -> String {
    let mut url = format!("/literatur/bibel?work={}", at.work);
    if let Some(module) = module {
        url.push_str(&format!("&module={}", query_escape(module)));
    }
    url.push_str(extra);
    url.push_str(&tail(at));
    url
}

/// Bible modules (without commentaries and lexicons), the Bible of the work first.
async fn bible_list(state: &AppState, at: &At) -> Result<Vec<BibleEntry>, RpcError> {
    let modules = crate::bibles::modules_value(state, at.work).await?;
    let chosen = modules["work_bible"].as_str().map(str::to_owned);
    let mut list: Vec<BibleEntry> = modules["modules"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter(|m| m.get("kind").and_then(serde_json::Value::as_str).is_none_or(|k| k == "bible"))
        .map(|m| {
            let module = m["module"].as_str().unwrap_or_default().to_owned();
            BibleEntry {
                work_bible: chosen.as_deref() == Some(module.as_str()),
                description: m["description"].as_str().unwrap_or_default().to_owned(),
                notice: m["license"].as_str().unwrap_or_default().to_owned(),
                supported: m["supported"].as_bool().unwrap_or(false),
                problem: m["problem"].as_str().map(str::to_owned),
                url: page_url(at, Some(&module), ""),
                module,
            }
        })
        .collect();
    list.sort_by_key(|b| !b.work_bible);
    Ok(list)
}

/// Box „Bibeln“ and – with search words in wording mode – the groups „Bibel“ and „Kommentar“. If something
/// fails, the reason is in the notice; the rest of the literature stays.
pub(super) async fn shelf(state: &AppState, at: &At, query: Option<&str>) -> Shelf {
    let mut shelf = Shelf { page_url: page_url(at, None, ""), ..Shelf::default() };
    if !scan_allowed(state, at.work).await {
        shelf.hint = Some(NO_BIBLE.to_owned());
        return shelf;
    }
    match bible_list(state, at).await {
        Ok(list) if list.is_empty() => shelf.hint = Some(NO_BIBLE.to_owned()),
        Ok(list) => shelf.bibles = list,
        Err(e) => shelf.hint = Some(format!("Bibeln nicht lesbar: {}", e.message)),
    }
    let Some(query) = query.map(str::trim).filter(|q| !q.is_empty()) else { return shelf };
    let usable: Vec<String> =
        shelf.bibles.iter().filter(|b| b.supported).take(SEARCH_MODULES).map(|b| b.module.clone()).collect();
    let searches = async {
        let mut groups = Vec::new();
        for module in usable {
            groups.push(hit_group(state, at, &module, query).await);
        }
        groups
    };
    match tokio::time::timeout(SEARCH_TIME, searches).await {
        Ok(groups) => shelf.groups = groups.into_iter().filter(|g| !g.hits.is_empty() || g.problem.is_some()).collect(),
        Err(_) => shelf.groups.push(HitGroup {
            module: String::new(),
            hits: Vec::new(),
            truncated: true,
            problem: Some("Bibelsuche abgebrochen: Zeitgrenze von 3 s erreicht.".to_owned()),
        }),
    }
    shelf.commentary_hits = crate::bibles::commentary_hits_html(state, at.work, query, &tail(at)).await;
    shelf
}

async fn hit_group(state: &AppState, at: &At, module: &str, query: &str) -> HitGroup {
    let mut group = HitGroup { module: module.to_owned(), hits: Vec::new(), truncated: false, problem: None };
    let choice = match crate::bibles::by_name(state, module).await {
        Ok(choice) => choice,
        Err(e) => {
            group.problem = Some(e.message);
            return group;
        }
    };
    match crate::bibles::search(&choice, query, HITS_PER_MODULE).await {
        Ok(found) => {
            group.truncated = found["truncated"].as_bool().unwrap_or(false);
            group.hits = hits_of(&found, |reference| page_url(at, Some(module), &ref_param(reference)));
        }
        // Search too long or empty: no group (the literature already reports that).
        Err(e) if e.code == INVALID_PARAMS => {}
        Err(e) => group.problem = Some(e.message),
    }
    group
}

fn ref_param(reference: &str) -> String {
    format!("&ref={}", query_escape(reference))
}

fn hits_of(found: &serde_json::Value, url: impl Fn(&str) -> String) -> Vec<BibleHit> {
    found["hits"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|h| {
            let reference = h["ref"].as_str().unwrap_or_default().to_owned();
            BibleHit { url: url(&reference), text: h["text"].as_str().unwrap_or_default().to_owned(), reference }
        })
        .collect()
}

// ---- Page ----------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct BibleQuery {
    work: Option<String>,
    key: Option<String>,
    node: Option<String>,
    rev: Option<String>,
    view: Option<String>,
    lq: Option<String>,
    lmode: Option<String>,
    module: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    book: Option<String>,
    chapter: Option<String>,
    q: Option<String>,
    kom: Option<String>,
    notice: Option<String>,
}

/// Chosen module: from the address, otherwise the Bible of the work, otherwise the first supported one.
fn pick_module(list: &[BibleEntry], wanted: Option<&str>) -> Option<String> {
    match wanted {
        Some(w) => list.iter().find(|b| b.module.eq_ignore_ascii_case(w)).map(|b| b.module.clone()),
        None => list
            .iter()
            .find(|b| b.work_bible && b.supported)
            .or_else(|| list.iter().find(|b| b.supported))
            .map(|b| b.module.clone()),
    }
}

/// Verses of a reference for the template (from the answer of [`crate::bibles::lookup`]). Round 19c: `ref` is the
/// **German** reference of the verse (mapped back from the numbering of the module, `german_ref`), `module_label`
/// the number in the module, `url` puts the German reference into the field without JavaScript
/// (`ref_url(Stelle)`); if a verse cannot be mapped, `ref`/`url` are missing and `unmapped` names the reason.
fn passages(found: &serde_json::Value, ref_url: impl Fn(&str) -> String) -> Vec<serde_json::Value> {
    found["refs"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let abbr = r["book_id"].as_str().and_then(baruch_scripture::find_book).map_or("", |b| b.abbr);
            let verses: Vec<serde_json::Value> = r["verses"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|v| {
                    let (c, n) = (v["chapter"].as_u64().unwrap_or(0), v["verse"].as_u64().unwrap_or(0));
                    let module_label = format!("{abbr} {c},{n}");
                    let german = v["german_ref"].as_str().map(str::to_owned);
                    json!({
                        "chapter": c, "verse": n, "text": v["text"], "module_label": module_label,
                        "ref": german,
                        "moved": german.as_deref().is_some_and(|g| g != module_label),
                        "url": german.as_deref().map(|g| href_string(ref_url(g))),
                        "unmapped": v["german_problem"],
                    })
                })
                .collect();
            json!({
                "input": r["input"], "normalized": r["normalized"], "book": r["book"], "status": r["status"],
                "message": r["message"], "verses": verses, "missing": r["missing"], "truncated": r["truncated"],
                "module_ref": r["module_ref"],
            })
        })
        .collect()
}

/// Book and chapter of a reference that was read (for „voriges/nächstes Kapitel“).
fn book_chapter(found: &serde_json::Value) -> Option<(&'static baruch_scripture::Book, u16)> {
    let first = found["refs"].as_array()?.first()?;
    let book = baruch_scripture::find_book(first["book_id"].as_str()?)?;
    let chapter = first["verses"].as_array()?.first()?["chapter"].as_u64()?;
    Some((book, u16::try_from(chapter).ok()?))
}

/// Self-built address for `passages` (as text; the template passes it on through `|safe`): `&` escaped.
fn href_string(url: String) -> String {
    url.replace('&', "&amp;")
}

fn notice_text(code: Option<&str>) -> Option<&'static str> {
    match code? {
        "cited" => Some("Zitat als Vorschlag im Abschnitt angelegt; übernehmen kannst nur du."),
        "highlight" => Some(NO_HIGHLIGHT),
        _ => None,
    }
}

async fn page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<BibleQuery>) -> Response {
    let at = match At::resolve(
        &state,
        number(&q.work),
        (number(&q.node), number(&q.key)),
        number(&q.rev),
        q.view.as_deref(),
        (q.lq.as_deref(), q.lmode.as_deref()),
    )
    .await
    {
        Ok(at) => at,
        Err(StoreError::UnknownWork(_)) => {
            return (axum::http::StatusCode::NOT_FOUND, "unbekanntes Werk").into_response();
        }
        Err(e) => return error_response(e.into()),
    };
    let allowed = scan_allowed(&state, at.work).await;
    let list = if allowed { bible_list(&state, &at).await.unwrap_or_default() } else { Vec::new() };
    let module = pick_module(&list, text(&q.module));
    let (work, node, rev) = at.context();
    let mut ctx = context! {
        settings => page_settings(&headers), work => work, node => node, rev => rev,
        bibles => list.iter().map(entry_value).collect::<Vec<_>>(), place_fields => at.fields(),
        literature_url => href(at.literature_url()), work_url => href(at.work_url()),
        settings_url => Value::from_safe_string(SOURCES_URL.to_owned()), no_bible => NO_BIBLE, no_highlight => NO_HIGHLIGHT,
        notice => notice_text(q.notice.as_deref()), has_key => at.key.is_some(),
        library_query => at.lq.as_deref(),
    };
    let Some(module) = module else {
        return match render("literature_bible.html", context! { missing => true, ..ctx }) {
            Ok(html) => Html(html).into_response(),
            Err(e) => error_response(e),
        };
    };
    let choice = match crate::bibles::by_name(&state, &module).await {
        Ok(choice) => choice,
        Err(e) => return error_response(e),
    };
    let books: Vec<Value> = match &choice {
        Choice::Ready(bible) => bible
            .v11n
            .books()
            .filter_map(|(id, chapters)| {
                let book = baruch_scripture::find_book(id)?;
                let url = href(page_url(&at, Some(&module), &format!("&book={}&chapter=1", book.id)));
                Some(context! { id => book.id, name => book.name, abbr => book.abbr, chapters => chapters.len(), url => url })
            })
            .collect(),
        _ => Vec::new(),
    };
    let entry = list.iter().find(|b| b.module == module).cloned();
    ctx = context! { module => module.as_str(), entry => entry.as_ref().map(entry_value), books => books,
    problem => choice.bible().is_none().then(|| choice.json()["problem"].as_str().map(str::to_owned)).flatten(),
    ..ctx };
    // Reference from `ref` or `book`+`chapter`.
    let reference = match (text(&q.reference), text(&q.book), number::<u16>(&q.chapter)) {
        (Some(r), _, _) => Some(r.chars().take(500).collect::<String>()),
        (None, Some(book), chapter) => {
            baruch_scripture::find_book(book).map(|b| format!("{} {}", b.abbr, chapter.unwrap_or(1).max(1)))
        }
        _ => None,
    };
    if let Some(reference) = reference {
        let found = crate::bibles::lookup(&state, &choice, &reference).await;
        ctx = match found {
            Ok(found) => {
                let normalized = found["refs"][0]["normalized"].as_str().unwrap_or(&reference).to_owned();
                let (prev, next) = match book_chapter(&found) {
                    Some((book, chapter)) => {
                        let link = |c: u16| page_url(&at, Some(&module), &format!("&book={}&chapter={c}", book.id));
                        (
                            (chapter > 1).then(|| href(link(chapter - 1))),
                            (chapter < book.chapters()).then(|| href(link(chapter + 1))),
                        )
                    }
                    None => (None, None),
                };
                let mut panel = crate::bibles::commentary_panel_html(&state, at.work, &normalized).await;
                if q.kom.as_deref() == Some("1") {
                    panel =
                        panel.replacen("<details class=\"commentaries\"", "<details open class=\"commentaries\"", 1);
                }
                context! {
                    reference => reference.as_str(), normalized => normalized.as_str(),
                    passages => Value::from(Serde(passages(&found, |r| page_url(&at, Some(&module), &ref_param(r))))),
                    prev_url => prev, next_url => next,
                    commentaries => Value::from_safe_string(panel),
                    personas => persona_options(&state, at.work).await,
                    ..ctx
                }
            }
            Err(e) => context! { reference => reference.as_str(), lookup_error => e.message, ..ctx },
        };
    } else if let Some(query) = text(&q.q) {
        ctx = match crate::bibles::search(&choice, query, PAGE_HITS).await {
            Ok(found) => {
                let hits = hits_of(&found, |r| page_url(&at, Some(&module), &ref_param(r)));
                let hits: Vec<Value> = hits
                    .into_iter()
                    .map(|h| context! { reference => h.reference, text => h.text, url => href(h.url) })
                    .collect();
                context! { q => query, hits => hits, truncated => found["truncated"].as_bool(), ..ctx }
            }
            Err(e) => context! { q => query, search_error => e.message, ..ctx },
        };
    }
    match render("literature_bible.html", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_response(e),
    }
}

/// Active personas for „an Persona geben“ (the lector selected first).
async fn persona_options(state: &AppState, work: WorkId) -> Vec<Value> {
    let lector = state.store.load_work(work).await.ok().and_then(|w| w.lector().map(|p| p.handle.clone()));
    state
        .store
        .persona_list()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.active)
        .map(|p| {
            let selected = lector.as_deref() == Some(p.handle.as_str());
            context! { handle => p.handle, name => p.name, selected => selected }
        })
        .collect()
}

// ---- Form ----------------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct BibleAct {
    action: String,
    work: Option<String>,
    key: Option<String>,
    node: Option<String>,
    rev: Option<String>,
    view: Option<String>,
    lq: Option<String>,
    lmode: Option<String>,
    module: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    persona: Option<String>,
    question: Option<String>,
}

/// Verses of a reference as text with the reference from the module (`Joh 3,16–17`); never from the form.
async fn verses(state: &AppState, module: &str, reference: &str) -> Result<(String, String, String), RpcError> {
    let choice = crate::bibles::by_name(state, module).await?;
    let Some(bible) = choice.bible() else {
        return Err(RpcError::new(REFUSED, choice.json()["problem"].as_str().unwrap_or(NO_BIBLE).to_owned()));
    };
    let module = bible.info.name.clone();
    let found = crate::bibles::lookup(state, &choice, reference).await?;
    let refs = found["refs"].as_array().cloned().unwrap_or_default();
    let mut labels = Vec::new();
    let mut parts = Vec::new();
    for r in &refs {
        if r["status"].as_str() != Some("valid") {
            return Err(RpcError::new(INVALID_PARAMS, r["message"].as_str().unwrap_or("Stelle ungültig").to_owned()));
        }
        labels.push(r["normalized"].as_str().unwrap_or_default().to_owned());
        for v in r["verses"].as_array().map(Vec::as_slice).unwrap_or_default() {
            if let Some(t) = v["text"].as_str() {
                parts.push(t.to_owned());
            }
        }
    }
    if parts.is_empty() {
        return Err(RpcError::new(INVALID_PARAMS, format!("Zu „{reference}“ hat das Modul {module} keinen Text.")));
    }
    let mut body = parts.join(" ");
    if body.chars().count() > crate::bibles::MAX_TEXT_CHARS {
        body = body.chars().take(crate::bibles::MAX_TEXT_CHARS).collect::<String>() + " …";
    }
    Ok((body, labels.join("; "), module))
}

/// Bible quote as a proposal revision: current text + empty line + `#quote(block: true, attribution: […])[…]`
/// (author `@literatur`, model `baruch-zitat`). Under the write lock, section through `key`.
pub async fn cite_verses(
    state: &AppState,
    work: WorkId,
    key: i64,
    module: &str,
    reference: &str,
) -> Result<u32, RpcError> {
    let (body_text, label, module) = verses(state, module, reference).await?;
    let quote = format!(
        "#quote(block: true, attribution: [{} ({})])[{}]",
        baruch_library::escape_markup(&label),
        baruch_library::escape_markup(&module),
        baruch_library::escape_markup(&body_text)
    );
    let _held = state.write_lock.lock().await;
    let node = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let n = loaded.nodes.get(node).ok_or(StoreError::UnknownNode { work, node })?;
    let current = n
        .revisions
        .iter()
        .find(|r| r.status == baruch_core::RevisionStatus::Current)
        .ok_or(StoreError::Refused("Vorschläge brauchen eine aktuelle Revision."))?;
    let mut body = current.body.clone();
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    if !body.is_empty() {
        body.push('\n');
    }
    body.push_str(&quote);
    body.push('\n');
    let note = format!("Bibelzitat {label} ({module})");
    let rev = state.store.add_proposal(work, node, &body, super::CITE_PERSONA, super::CITE_MODEL, &note).await?;
    crate::mirror::side_refs_after_proposal(state, &_held, work).await;
    Ok(rev)
}

async fn act(State(state): State<Arc<AppState>>, Form(form): Form<BibleAct>) -> Response {
    let at = match At::resolve(
        &state,
        number(&form.work),
        (number(&form.node), number(&form.key)),
        number(&form.rev),
        form.view.as_deref(),
        (form.lq.as_deref(), form.lmode.as_deref()),
    )
    .await
    {
        Ok(at) => at,
        Err(e) => return error_response(e.into()),
    };
    let module = text(&form.module).unwrap_or_default().to_owned();
    let reference = text(&form.reference).unwrap_or_default().to_owned();
    let back = page_url(&at, Some(&module), &ref_param(&reference));
    let notice = |message: &str| crate::assist::routes::notice_page(message, &back, None);
    if form.action == "highlight" {
        return notice(NO_HIGHLIGHT);
    }
    if reference.is_empty() || module.is_empty() {
        return notice("Stelle und Bibel angeben (z. B. „Ps 139,23-24“).");
    }
    let Some(key) = number::<i64>(&form.key) else { return notice(super::PICK_SECTION) };
    match form.action.as_str() {
        "cite" => match cite_verses(&state, at.work, key, &module, &reference).await {
            Ok(rev) => {
                Redirect::to(&crate::view::node_url(at.work, key, Some(rev), false, "", &at.lq_tail())).into_response()
            }
            Err(e) => notice(&e.message),
        },
        "persona" => {
            let (body, label, module) = match verses(&state, &module, &reference).await {
                Ok(found) => found,
                Err(e) => return notice(&e.message),
            };
            let order = ToPersona {
                work: at.work,
                key,
                persona: text(&form.persona).map(str::to_owned),
                text: body,
                source: format!("Bibel {module}, {label}"),
                title: None,
                highlight: None,
                question: text(&form.question).map(str::to_owned),
            };
            match super::to_persona(&state, &order).await {
                Ok(answer) => {
                    let url = crate::view::node_url(at.work, key, at.rev, at.preview, "", &at.lq_tail());
                    match answer.get("compressed").filter(|c| c.is_object()) {
                        Some(c) => crate::assist::routes::notice_page(
                            "Die Antwort der Persona ist gespeichert.",
                            &url,
                            Some(c),
                        ),
                        None => Redirect::to(&url).into_response(),
                    }
                }
                Err(e) => notice(&e.message),
            }
        }
        other => notice(&format!("Unbekannte Aktion „{other}“.")),
    }
}

/// Page (reading): `GET /literatur/bibel`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/literatur/bibel", get(page))
}

/// Form (writing, in the router behind `guard::same_origin`): `POST /literatur/bibel/act`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/literatur/bibel/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(module: &str, work_bible: bool, supported: bool) -> BibleEntry {
        BibleEntry {
            module: module.into(),
            description: String::new(),
            notice: String::new(),
            work_bible,
            supported,
            problem: None,
            url: String::new(),
        }
    }

    #[test]
    fn the_work_bible_is_picked_first_and_unsupported_ones_are_skipped() {
        let list = [entry("Kaputt", false, false), entry("Fremd", false, true), entry("Werk", true, true)];
        assert_eq!(pick_module(&list, None).as_deref(), Some("Werk"), "ohne Wahl die Bibel des Werks");
        assert_eq!(
            pick_module(&list, Some("fremd")).as_deref(),
            Some("Fremd"),
            "Modulname ohne Rücksicht auf Groß/klein"
        );
        assert_eq!(pick_module(&list, Some("Unbekannt")), None, "unbekanntes Modul: keine stille Ersatzwahl");
        let without = [entry("Kaputt", false, false), entry("Zweite", false, true)];
        assert_eq!(pick_module(&without, None).as_deref(), Some("Zweite"), "sonst die erste unterstützte");
    }
}
