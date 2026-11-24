// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bibeln in der Literatur (Nutzerwunsch 2026-11-07, Runde 15 „auftraege“): die SWORD-Bibeln stehen neben den
//! Titeln der Bibliothek – zum Lesen, Suchen, Zitieren und „an Persona geben“.
//!
//! - Kasten „Bibeln“ (Popup und `/literatur`, Vorlage `fragments/literature_bibles.html`): die Bibelmodule aus
//!   `bible.modules` (Einträge ohne `kind` gelten als Bibel, Kommentare und Lexika nicht), die Bibel des Werks
//!   markiert und zuerst; je Bibel ein Link auf `/literatur/bibel`.
//! - Treffergruppen im gemeinsamen Suchfeld (nur Wortlaut-Modus): „Bibel“ je Modul (Modulname, höchstens
//!   [`HITS_PER_MODULE`] Treffer, höchstens [`SEARCH_MODULES`] Module, Zeitgrenze [`SEARCH_TIME`]) über
//!   [`crate::bibles::search`] und „Kommentar“ als fertiger Baustein [`crate::bibles::commentary_hits_html`].
//! - `GET /literatur/bibel?work&key&rev&lq&lmode&module&ref&book&chapter&q&kom` (ohne JavaScript): Bibel öffnen
//!   (Bücherliste), Buch/Kapitel blättern ([`crate::bibles::lookup`] mit „Buch Kapitel“), Stelle aufschlagen
//!   (`ref`, z. B. „Ps 139,23-24“), Wortlautsuche (`q`). Neben einer aufgeschlagenen Stelle der Baustein
//!   [`crate::bibles::commentary_panel_html`] (`kom=1` klappt ihn auf). Ohne Bibel ein ehrlicher Hinweis mit Link zu
//!   Einstellungen → Quellen.
//! - `POST /literatur/bibel/act` (hinter `guard::same_origin`): `action=cite` legt ein Bibelzitat mit
//!   Stellenangabe als Vorschlag-Revision an (Autor `@literatur`, Modell `baruch-zitat`, wie `highlight.cite`;
//!   übernehmen kann nur der Mensch), `action=persona` gibt die Verse an eine Persona
//!   ([`super::to_persona`], Quelle „Bibel MODUL, Stelle“). Der Text kommt immer aus dem Modul (Feld `ref`), nie
//!   aus dem Formular. `action=highlight` gibt es nicht: Hervorhebungen gehören zu Titeln der Bibliothek (mit
//!   Zitat und Zeichenbereich), eine Bibelstelle ist kein solcher Titel – die Seite sagt das ehrlich.
//!
//! Modulordner: Gelesen wird nur, wenn Modulordner ausdrücklich eingestellt sind oder das Werk eine Bibel hat
//! ([`scan_allowed`]). So liest eine Seite ohne Einstellung nie die Vorgabeordner des Rechners (Tests setzen ihre
//! Ordner selbst); stattdessen der Hinweis auf Einstellungen → Quellen.

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

/// Höchstens so viele Treffer je Bibel in der Gruppe „Bibel“.
pub const HITS_PER_MODULE: usize = 10;
/// Höchstens so viele Bibeln durchsucht das gemeinsame Suchfeld.
pub const SEARCH_MODULES: usize = 3;
/// Zeitgrenze der Gruppe „Bibel“ im gemeinsamen Suchfeld (alle Module zusammen).
pub const SEARCH_TIME: Duration = Duration::from_secs(3);
/// Treffer der Wortlautsuche auf der Bibelseite.
pub const PAGE_HITS: usize = 50;
/// Hinweis ohne Bibel.
pub const NO_BIBLE: &str = "Keine SWORD-Bibel verfügbar: Modulordner und Bibel des Werks stellst du unter \
     Einstellungen → Quellen → Bibeln (SWORD) ein.";
/// Hinweis zu „Hervorheben“ auf der Bibelseite.
pub const NO_HIGHLIGHT: &str = "Hervorheben geht nur bei Titeln der Bibliothek (Zitat mit Zeichenbereich); eine \
     Bibelstelle ist kein solcher Titel. Zitieren und „an Persona geben“ gehen.";
/// Ziel des Links „Einstellungen → Quellen“.
pub const SOURCES_URL: &str = "/einstellungen/quellen";

/// Eine Bibel im Kasten.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BibleEntry {
    pub module: String,
    pub description: String,
    pub notice: String,
    /// Bibel des Werks.
    pub work_bible: bool,
    pub supported: bool,
    pub problem: Option<String>,
    pub url: String,
}

/// Treffer einer Bibel im gemeinsamen Suchfeld.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HitGroup {
    pub module: String,
    pub hits: Vec<BibleHit>,
    pub truncated: bool,
    /// Ehrlicher Hinweis (Zeitgrenze, Modulfehler).
    pub problem: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BibleHit {
    pub reference: String,
    pub text: String,
    pub url: String,
}

/// Kasten und Treffergruppen für `fragments/library.html`.
#[derive(Debug, Clone, Default)]
pub struct Shelf {
    pub bibles: Vec<BibleEntry>,
    /// Hinweis statt der Liste (keine Bibel, Ordner nicht eingestellt, Fehler beim Lesen).
    pub hint: Option<String>,
    pub groups: Vec<HitGroup>,
    /// Fertiges HTML der Gruppe „Kommentar“ ([`crate::bibles::commentary_hits_html`]).
    pub commentary_hits: String,
    /// Bibelseite ohne gewähltes Modul.
    pub page_url: String,
}

impl Shelf {
    /// Kontext `bibles` für die Vorlagen (Adressen als sichere Werte, Texte maskiert die Vorlage).
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
    /// Runde 19c: Kontext für die Arbeitsseite ohne JavaScript ([`workspace_shelf`]): wie [`Shelf::context`], die
    /// Links der Bibeln tragen aber zusätzlich den festen Schlüssel und die Fassung des gezeigten Abschnitts (so
    /// gehen „Zitieren“ und „an Persona geben“ auf der Bibelseite).
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

/// Runde 19c: Kasten „Bibeln“ für die Arbeitsseite (`fragments/library.html` über `view.rs`), serverseitig und ohne
/// JavaScript – dieselben Bibeln wie im Popup (`literature.render`) und auf `/literatur`, ohne Treffergruppen (die
/// Suche der Arbeitsseite führt auf `/literatur`). Ohne erlaubte Modulordner der ehrliche Hinweis.
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

/// Darf Baruch die Modulordner lesen? Nur bei ausdrücklich eingestellten Ordnern oder einer Bibel des Werks.
pub async fn scan_allowed(state: &AppState, work: WorkId) -> bool {
    let explicit = matches!(state.store.bible_dirs().await, Ok(Some(_)));
    explicit || matches!(state.store.work_bible(work).await, Ok(Some(_)))
}

/// Anfragerest der Bibelseite: `&key=…&rev=…&lq=…&lmode=rag` (nur vorhandene Werte, prozentkodiert).
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

/// Adresse der Bibelseite mit Modul und weiteren Feldern (`extra` beginnt mit `&` oder ist leer).
fn page_url(at: &At, module: Option<&str>, extra: &str) -> String {
    let mut url = format!("/literatur/bibel?work={}", at.work);
    if let Some(module) = module {
        url.push_str(&format!("&module={}", query_escape(module)));
    }
    url.push_str(extra);
    url.push_str(&tail(at));
    url
}

/// Bibelmodule (ohne Kommentare und Lexika), die Bibel des Werks zuerst.
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

/// Kasten „Bibeln“ und – mit Suchwörtern im Wortlaut-Modus – die Gruppen „Bibel“ und „Kommentar“. Scheitert
/// etwas, steht der Grund im Hinweis; die übrige Literatur bleibt.
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
        // Zu lange oder leere Suche: keine Gruppe (die Literatur meldet das schon).
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

// ---- Seite ---------------------------------------------------------------------------------------------

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

/// Gewähltes Modul: aus der Adresse, sonst die Bibel des Werks, sonst die erste unterstützte.
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

/// Verse einer Stelle für die Vorlage (aus der Antwort von [`crate::bibles::lookup`]). Runde 19c: `ref` ist die
/// **deutsche** Stelle des Verses (Rückabbildung aus der Zählung des Moduls, `german_ref`), `module_label` die
/// Nummer im Modul, `url` setzt die deutsche Stelle ohne JavaScript ins Feld (`ref_url(Stelle)`); lässt sich ein
/// Vers nicht abbilden, fehlen `ref`/`url` und `unmapped` nennt den Grund.
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

/// Buch und Kapitel einer gelesenen Stelle (für „voriges/nächstes Kapitel“).
fn book_chapter(found: &serde_json::Value) -> Option<(&'static baruch_scripture::Book, u16)> {
    let first = found["refs"].as_array()?.first()?;
    let book = baruch_scripture::find_book(first["book_id"].as_str()?)?;
    let chapter = first["verses"].as_array()?.first()?["chapter"].as_u64()?;
    Some((book, u16::try_from(chapter).ok()?))
}

/// Selbst gebaute Adresse für `passages` (als Text; die Vorlage gibt sie über `|safe` weiter): `&` maskiert.
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
    // Stelle aus `ref` bzw. `book`+`chapter`.
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

/// Aktive Personas für „an Persona geben“ (der Lektor zuerst gewählt).
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

// ---- Formular ------------------------------------------------------------------------------------------

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

/// Verse einer Stelle als Text mit Stellenangabe aus dem Modul (`Joh 3,16–17`); nie aus dem Formular.
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

/// Bibelzitat als Vorschlag-Revision: aktueller Text + Leerzeile + `#quote(block: true, attribution: […])[…]`
/// (Autor `@literatur`, Modell `baruch-zitat`). Unter der Schreibsperre, Abschnitt über `key`.
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

/// Seite (lesend): `GET /literatur/bibel`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/literatur/bibel", get(page))
}

/// Formular (schreibend, im Router hinter `guard::same_origin`): `POST /literatur/bibel/act`.
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
