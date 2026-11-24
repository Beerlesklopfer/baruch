// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einfälle (Vertrag Runde 7, Abschnitt 6): RPC `idea.*`, die Seiten `/einfaelle` und
//! `/einfaelle/{id}` und ihr Formular an `POST /einfaelle/act`, alles ohne JavaScript bedienbar. Die
//! Schnellerfassung der Kopfzeile (`fragments/idea_capture.html`, Widget `idea-capture` in
//! `web/src/ideas.ts`) schickt dasselbe Formular, mit JavaScript über `idea.create`.
//!
//! RPC ([`handles`], [`call`]; vor der Wahl des Vorgabewerks, weil Einfälle auch ohne Werk gehen).
//! Herkunft ist hier immer der Mensch ([`mirror::HUMAN_NAME`]); `work` fehlt oder `null` = ohne Werk
//! (bei Listen: alle):
//! - `idea.create {work?, key?, words: [..] | "a, b; c", note?, quote?, source?, bible?}` → `{idea, commit?|mirror?}`
//! - `idea.get {id}` → `{idea}`
//! - `idea.list {work?: id|"none"|"all", word?, status?: "open"|"done"|"all", deleted?, limit?, offset?}`
//!   → `{ideas, words}` (`words` = Schlagwort-Wolke über denselben Werk-/Statusfilter)
//! - `idea.search {query, work?, status?, limit?, mode?}` → `{ideas, hits: [{id, snippet}]}` (FTS5 über
//!   [`baruch_library::fts_query`]; leere Suche → -32602). `mode: "rag"` (Runde 12) sucht nach Bedeutung
//!   über [`crate::search_hooks::rag_search`] (Weg LanceDB, Art `idea`) und braucht ein Werk (sonst -32602
//!   „Bedeutungssuche nur innerhalb eines Werks“) → `{ideas, hits: [{id, snippet, score, weak}], ways}` in der
//!   Reihenfolge des Rangs, schwache Treffer (`weak`, [`baruch_rag::is_weak`], Runde 19c) zuletzt; gelöschte und
//!   (nach `status`) nicht passende Einfälle fallen heraus. Fällt die Bedeutungssuche aus (LanceDB nicht verbunden,
//!   etwa ohne Einbettungsmodell), antwortet sie seit Runde 19c mit -32001 ([`crate::rpc::RAG_UNAVAILABLE`]) und dem
//!   Grund. Die Seite `/einfaelle?work=W&q=…&mode=rag` zeigt dasselbe ohne JavaScript (schwache Treffer zugeklappt).
//! - `idea.update {id, words?, note?, quote?, source?, bible?, work?, key?, status?}` → `{idea, changed, commit?}`
//! - `idea.delete {id, confirm: true}` → `{id, deleted: true, commit?}`; ohne `confirm` → -32602
//! - `idea.restore {id}` → `{idea, commit?}`
//! - `idea.process {id, as: "outline"|"post"|"note"|"persona", key?, place?, title?, persona?}` →
//!   `{idea, node?, key?, outline?, commit?}`: `outline` legt über [`Store::idea_to_outline`] (Runde 14,
//!   eine Transaktion mit dem Erledigen des Einfalls) einen
//!   Gliederungspunkt an (Ziel `key` bzw. der Bezug des Einfalls, Vorgabe `after`; Titel = `title`
//!   oder die Schlagwörter, auf 200 Zeichen gekürzt) und spiegelt ihn wie `outline.add`; `post` und
//!   `note` legen einen Beitrag im Abschnitts-Chat an (Autor der Mensch bzw. „Notiz“); danach ist der
//!   Einfall verarbeitet (ein Commit `idea-process`). `persona` (Runde 10) ist ein Auftrag an die
//!   Persona `persona` (Vorgabe: der Lektor des Werks) mit dem Einfall als Frage zum Bezugsabschnitt: Ihre
//!   Antwort wird ein Beitrag im Abschnitts-Chat (Quelle „Einfall <id>“), der Einfall ist danach
//!   verarbeitet (`done_as` persona, `done_ref` der Beitrag). Der Modellaufruf läuft **vor** der
//!   Schreibsperre ([`crate::assist`]); scheitert er, bleibt der Einfall offen und nichts ist gespeichert.
//!   Der Lektor kennt außerdem die Einfälle als Gedächtnis ([`crate::assist::context`]).
//!
//! Jede Aktion läuft unter [`AppState::write_lock`] und ergibt genau einen Commit je betroffenem
//! Spiegel ([`mirror::record_idea`]): Werk → Werkspiegel mit `einfaelle.md`, ohne Werk →
//! `<data>/einfaelle/`; beim Umziehen zwischen Werken je einer im alten und im neuen.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::Post;
use baruch_rag::{Answer, DEFAULT_LIMIT, MAX_LIMIT, RagError, SearchRequest, Way, WayState};
use baruch_store::{
    Idea, IdeaEdit, IdeaFilter, IdeaId, IdeaLinkId, IdeaOrigin, IdeaScope, IdeaStatus, IdeaUse, NewIdea, Place, Store,
    StoreError, WordCount, WorkId,
};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::Deserialize;
use serde_json::{Value as Json, json};

use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{Settings, query_escape};
use crate::{AppState, mirror};

const METHODS: &[&str] = &[
    "idea.create",
    "idea.get",
    "idea.list",
    "idea.search",
    "idea.update",
    "idea.delete",
    "idea.restore",
    "idea.process",
];

/// Höchstlänge eines Titels aus den Schlagwörtern (Titelregel der Gliederung).
const MAX_TITLE: usize = 200;

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

// ---- Parameter ---------------------------------------------------------------------------------

/// Zahl aus einem JSON-Wert (Zahl oder Ziffern als Text, Formulare).
fn int(value: &Json, name: &str) -> Result<i64, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match value {
        Json::Number(n) => n.as_i64().ok_or_else(wrong),
        Json::String(s) => s.trim().parse().map_err(|_| wrong()),
        _ => Err(wrong()),
    }
}

/// `id` (Pflicht).
fn id_param(params: &Json) -> Result<IdeaId, RpcError> {
    match params.get("id") {
        None | Some(Json::Null) => Err(invalid("id fehlt (Einfall)")),
        Some(value) => int(value, "id"),
    }
}

/// Werk oder Abschnitt, der sich ändern lässt: fehlt → `None` (bleibt), `null`, `""` oder `"none"` →
/// `Some(None)` (ohne), Zahl → `Some(Some(n))`.
fn optional_ref(params: &Json, name: &str) -> Result<Option<Option<i64>>, RpcError> {
    match params.get(name) {
        None => Ok(None),
        Some(Json::Null) => Ok(Some(None)),
        Some(Json::String(s)) if s.trim().is_empty() || s.trim() == "none" => Ok(Some(None)),
        Some(value) => int(value, name).map(|n| Some(Some(n))),
    }
}

/// Text oder fehlend; `null` zählt als fehlend.
fn text(params: &Json, name: &str) -> Result<Option<String>, RpcError> {
    match params.get(name) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(format!("{name}: Text erwartet"))),
    }
}

/// Schlagwörter: Liste von Texten oder ein Text, getrennt durch Komma oder Semikolon.
pub(crate) fn split_words(value: &Json, name: &str) -> Result<Vec<String>, RpcError> {
    match value {
        Json::String(s) => Ok(s.split([',', ';']).map(str::to_owned).collect()),
        Json::Array(list) => list
            .iter()
            .map(|w| w.as_str().map(str::to_owned).ok_or_else(|| invalid(format!("{name}: Liste von Texten erwartet"))))
            .collect(),
        _ => Err(invalid(format!("{name}: Liste oder Text „a, b; c“ erwartet"))),
    }
}

fn words_param(params: &Json) -> Result<Option<Vec<String>>, RpcError> {
    match params.get("words") {
        None | Some(Json::Null) => Ok(None),
        Some(value) => split_words(value, "words").map(Some),
    }
}

/// Status für Listen: fehlt, `null` oder `"all"` → alle.
fn list_status(params: &Json) -> Result<Option<IdeaStatus>, RpcError> {
    match params.get("status") {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) if s.is_empty() || s == "all" => Ok(None),
        Some(Json::String(s)) => {
            IdeaStatus::parse(s).map(Some).ok_or_else(|| invalid("status: open, done oder all erwartet"))
        }
        Some(_) => Err(invalid("status: open, done oder all erwartet")),
    }
}

/// Werkfilter für Listen: fehlt, `null` oder `"all"` → alle, `"none"` → ohne Werk, sonst die id
/// (unbekannt → `UnknownWork`).
async fn list_scope(state: &AppState, params: &Json) -> Result<IdeaScope, RpcError> {
    match params.get("work") {
        None | Some(Json::Null) => Ok(IdeaScope::All),
        Some(Json::String(s)) if s.is_empty() || s == "all" => Ok(IdeaScope::All),
        Some(Json::String(s)) if s == "none" => Ok(IdeaScope::NoWork),
        Some(value) => {
            let work = int(value, "work")?;
            state.store.work_slug(work).await?;
            Ok(IdeaScope::Work(work))
        }
    }
}

fn limit_param(params: &Json, name: &str) -> Result<u32, RpcError> {
    match params.get(name) {
        None | Some(Json::Null) => Ok(0),
        Some(value) => u32::try_from(int(value, name)?).map_err(|_| invalid(format!("{name}: nicht negativ"))),
    }
}

fn place_param(params: &Json) -> Result<Place, RpcError> {
    match params.get("place").and_then(Json::as_str).map(str::trim) {
        None | Some("") | Some("after") => Ok(Place::After),
        Some("before") => Ok(Place::Before),
        Some("into") => Ok(Place::Into),
        Some(_) => Err(invalid("place: before, after oder into erwartet")),
    }
}

// ---- Abläufe (auch für MCP) ---------------------------------------------------------------------

/// Ergebnis des Spiegels in die Antwort: `commit` bei neuem Commit, `mirror` bei Fehler.
fn add_mirror(answer: &mut Json, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Commit-Auftrag für eine Aktion von `actor` an `idea`: Autor und `Baruch-Origin` nennen den, der
/// handelt (bei einer Änderung durch einen Assistenten also ihn), der Eintrag im Lösch-Commit die
/// Herkunft des Einfalls.
fn request_for(idea: &Idea, actor: &IdeaOrigin, action: &str, detail: &str) -> baruch_gitmirror::CommitRequest {
    let acting = Idea { origin: actor.clone(), ..idea.clone() };
    let mut request = mirror::describe_idea(&acting, action, detail);
    if action == "idea-delete" {
        let mut body = detail.trim_end().to_owned();
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str("Gelöschter Eintrag:\n");
        body.push_str(&idea.entry_markdown());
        request.body = body;
    }
    request
}

/// Einfall anlegen, unter der Schreibsperre, mit Commit. Antwort `{idea, commit?|mirror?}`.
pub(crate) async fn create(state: &AppState, new: &NewIdea<'_>) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    let idea = state.store.idea_create(new).await?;
    let request = request_for(&idea, &new.origin, "idea-create", "");
    let mut answer = json!({ "idea": idea });
    add_mirror(&mut answer, mirror::record_idea(state, &held, idea.work, request).await);
    Ok(answer)
}

/// Was sich geändert hat, für den Rumpf des Commits.
fn changes(before: &Idea, after: &Idea) -> String {
    let mut lines = Vec::new();
    let mut field = |label: &str, old: &str, new: &str| {
        if old != new {
            lines.push(format!("{label} vorher: {}\n{label} nachher: {}", one_line(old), one_line(new)));
        }
    };
    field("Schlagwörter", &before.words.join(", "), &after.words.join(", "));
    field("Gedanke", &before.note, &after.note);
    field("Markiert", &before.quote, &after.quote);
    field("Quelle", &before.source, &after.source);
    field("Bibelstelle", &before.bible, &after.bible);
    let work = |w: Option<WorkId>| w.map_or_else(|| "ohne Werk".to_owned(), |w| format!("Werk {w}"));
    field("Werk", &work(before.work), &work(after.work));
    let key = |k: Option<i64>| k.map_or_else(|| "ohne".to_owned(), |k| format!("key {k}"));
    field("Bezug", &key(before.key), &key(after.key));
    field("Status", before.status.as_str(), after.status.as_str());
    lines.join("\n")
}

fn one_line(s: &str) -> String {
    let joined = s.split(['\r', '\n']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
    if joined.is_empty() { "–".to_owned() } else { joined }
}

/// Einfall ändern: unverändert → `{idea, changed: false}` ohne Commit; sonst ein Commit, beim
/// Umziehen zwischen Werken (bzw. von/zu „ohne Werk“) je einer im alten und im neuen Spiegel.
pub(crate) async fn update(
    state: &AppState,
    id: IdeaId,
    edit: &IdeaEdit<'_>,
    actor: &IdeaOrigin,
) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    let before = state.store.idea_get(id).await?;
    // Verbindungen, die ein Umzug löst (der Store löst alle nicht gelöschten Verbindungen des Einfalls).
    let linked: Vec<IdeaLinkId> = state
        .store
        .idea_links_of_idea(id, true)
        .await?
        .into_iter()
        .filter(|l| l.deleted.is_none())
        .map(|l| l.id)
        .collect();
    let Some(after) = state.store.idea_update(id, edit).await? else {
        return Ok(json!({ "idea": before, "changed": false }));
    };
    let detail = changes(&before, &after);
    let mut answer = json!({ "idea": after, "changed": true });
    if before.work != after.work {
        let mut moved = format!("Umgezogen: {}", moved_text(before.work, after.work));
        if !linked.is_empty() {
            let list: Vec<String> = linked.iter().map(ToString::to_string).collect();
            moved.push_str(&format!("\nVerbindungen gelöst (nur innerhalb eines Werks möglich): {}", list.join(", ")));
        }
        moved.push_str(&format!("\n\n{detail}"));
        answer["unlinked"] = json!(linked);
        let old =
            mirror::record_idea(state, &held, before.work, request_for(&after, actor, "idea-update", &moved)).await;
        let mut commits = Vec::new();
        if let Ok(Some(commit)) = &old {
            commits.push(json!(commit));
        }
        if let Err(e) = &old {
            answer["mirror"] = json!(e);
        }
        let new =
            mirror::record_idea(state, &held, after.work, request_for(&after, actor, "idea-update", &moved)).await;
        if let Ok(Some(commit)) = &new {
            commits.push(json!(commit));
        }
        add_mirror(&mut answer, new);
        answer["commits"] = Json::Array(commits);
    } else {
        add_mirror(
            &mut answer,
            mirror::record_idea(state, &held, after.work, request_for(&after, actor, "idea-update", &detail)).await,
        );
    }
    Ok(answer)
}

fn moved_text(from: Option<WorkId>, to: Option<WorkId>) -> String {
    let name = |w: Option<WorkId>| w.map_or_else(|| "ohne Werk".to_owned(), |w| format!("Werk {w}"));
    format!("{} → {}", name(from), name(to))
}

/// Einfall löschen (wiederherstellbar), Commit mit dem ganzen Eintrag. Antwort `{id, deleted, commit?}`.
pub(crate) async fn delete(state: &AppState, id: IdeaId, actor: &IdeaOrigin) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    let idea = state.store.idea_delete(id).await?;
    let mut answer = json!({ "id": id, "deleted": true, "idea": idea });
    let request = request_for(&idea, actor, "idea-delete", "");
    add_mirror(&mut answer, mirror::record_idea(state, &held, idea.work, request).await);
    Ok(answer)
}

/// Gelöschten Einfall wiederherstellen, mit Commit. Antwort `{idea, commit?}`.
pub(crate) async fn restore(state: &AppState, id: IdeaId, actor: &IdeaOrigin) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    let idea = state.store.idea_restore(id).await?;
    let mut answer = json!({ "idea": idea });
    let request = request_for(&idea, actor, "idea-restore", "");
    add_mirror(&mut answer, mirror::record_idea(state, &held, idea.work, request).await);
    Ok(answer)
}

/// Titel eines neuen Gliederungspunkts aus den Schlagwörtern: mit „, “ verbunden, auf 200 Zeichen gekürzt.
pub fn title_from_words(words: &[String]) -> String {
    let joined = words.join(", ");
    joined.chars().take(MAX_TITLE).collect::<String>().trim().to_owned()
}

/// Text des Beitrags aus einem Einfall: Gedanke (sonst die Schlagwörter), die Markierung als Zitat.
fn post_body(idea: &Idea) -> String {
    let mut body = if idea.note.is_empty() { idea.words.join(", ") } else { idea.note.clone() };
    if !idea.quote.is_empty() {
        body.push_str("\n\n");
        let quoted: Vec<String> = idea.quote.lines().map(|line| format!("> {line}")).collect();
        body.push_str(&quoted.join("\n"));
    }
    body
}

/// Werk und Stelle für die Verarbeitung: Werk des Einfalls (Pflicht), Abschnitt aus `key` bzw. dem Bezug.
async fn target(state: &AppState, idea: &Idea, key: Option<i64>) -> Result<(WorkId, i64, usize), RpcError> {
    let Some(work) = idea.work else {
        return Err(invalid("Ein Einfall ohne Werk lässt sich so nicht verarbeiten; erst einem Werk zuordnen."));
    };
    let Some(key) = key.or(idea.key) else {
        return Err(invalid("Welcher Abschnitt? key angeben oder den Einfall einem Abschnitt zuordnen."));
    };
    let seq = state.store.node_seq(work, key).await?;
    Ok((work, key, seq))
}

/// `idea.process` (nur der Mensch).
async fn process(state: &AppState, params: &Json) -> Result<Json, RpcError> {
    let id = id_param(params)?;
    let used = match params.get("as").and_then(Json::as_str).map(str::trim) {
        Some(s) => IdeaUse::parse(s).ok_or_else(|| invalid("as: outline, post, note oder persona erwartet"))?,
        None => return Err(invalid("as fehlt (outline, post, note oder persona)")),
    };
    let key = optional_ref(params, "key")?.flatten();
    if used == IdeaUse::Persona {
        return process_persona(state, params, id, key).await;
    }
    let held = state.write_lock.lock().await;
    let idea = state.store.idea_get(id).await?;
    if idea.deleted.is_some() {
        return Err(StoreError::Idea(format!("Einfall {id} ist gelöscht; erst wiederherstellen.")).into());
    }
    let human = IdeaOrigin::Human { name: mirror::HUMAN_NAME.to_owned() };
    let mut answer = json!({});
    let (done, detail) = match used {
        IdeaUse::Persona => unreachable!("Persona-Auftrag läuft über process_persona"),
        IdeaUse::Outline => {
            let (work, _, seq) = target(state, &idea, key).await?;
            let title = match text(params, "title")?.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()) {
                Some(title) => title,
                None => title_from_words(&idea.words),
            };
            // Runde 14: Punkt anlegen und Einfall erledigen in einer Transaktion (scheitert etwas,
            // bleibt beides, wie es war, und es gibt keinen Commit).
            let place = place_param(params)?;
            let done = state.store.idea_to_outline(id, work, seq, place, &title, mirror::HUMAN_NAME).await?;
            let mut outline = crate::outline::outline_answer(&done.change);
            add_mirror(&mut outline, mirror::record_outline(state, &held, &done.change).await);
            answer["node"] = json!(done.change.node);
            answer["key"] = json!(done.key);
            answer["outline"] = outline;
            let detail = format!("Neuer Gliederungspunkt „{}“ ({})", done.change.title, done.key);
            (Ok(done.idea), detail)
        }
        IdeaUse::Post | IdeaUse::Note => {
            let (work, key, seq) = target(state, &idea, key).await?;
            let author = if used == IdeaUse::Note { "Notiz" } else { mirror::HUMAN_NAME };
            // Runde 15: Herkunft sichtbar – stammt der Einfall von einer Persona oder einem MCP-Client, nennt die
            // Quelle des Beitrags sie („Einfall 3 von @belege“); der Autor bleibt der Mensch, der verarbeitet.
            let post = Post {
                id: 0,
                created: String::new(),
                author: author.to_owned(),
                body: post_body(&idea),
                sources: vec![idea_source(&idea)],
                refs: vec![],
            };
            let post_id = state.store.add_post(work, seq, &post).await?;
            let loaded = state.store.load_work(work).await?;
            let position = loaded.nodes.get(seq).map_or(0, |n| n.posts.len().saturating_sub(1));
            answer["node"] = json!(seq);
            answer["key"] = json!(key);
            answer["post"] = json!(position);
            answer["post_id"] = json!(post_id);
            let what = if used == IdeaUse::Note { "Notiz" } else { "Beitrag" };
            (
                Err(format!("key {key}, Beitrag {position}")),
                format!("{what} im Abschnitts-Chat (key {key}, Beitrag {position})"),
            )
        }
    };
    // Gliederungspunkt: schon erledigt (`Ok`); Beitrag/Notiz: jetzt mit dem Verweis (`Err`) markieren.
    let idea = match done {
        Ok(idea) => idea,
        Err(done_ref) => state.store.idea_mark_done(id, used, &done_ref).await?,
    };
    let request = request_for(&idea, &human, "idea-process", &detail);
    answer["idea"] = json!(idea);
    add_mirror(&mut answer, mirror::record_idea(state, &held, idea.work, request).await);
    Ok(answer)
}

/// Quelle eines Beitrags aus einem Einfall: „Einfall n“, bei einer Persona bzw. einem MCP-Client als Urheber
/// „Einfall n von @x“ bzw. „Einfall n von @mcp:…“ (Runde 15: Herkunft im Beitrag sichtbar).
pub fn idea_source(idea: &Idea) -> String {
    match &idea.origin {
        IdeaOrigin::Human { .. } => format!("Einfall {}", idea.id),
        other => format!("Einfall {} von {}", idea.id, other.name()),
    }
}

/// `idea.process {as: "persona"}`: Auftrag an die Persona (Vorgabe Lektor), Modellaufruf ohne Sperre;
/// danach unter der Schreibsperre den Einfall als verarbeitet markieren und spiegeln.
async fn process_persona(state: &AppState, params: &Json, id: IdeaId, key: Option<i64>) -> Result<Json, RpcError> {
    let idea = state.store.idea_get(id).await?;
    if idea.deleted.is_some() {
        return Err(StoreError::Idea(format!("Einfall {id} ist gelöscht; erst wiederherstellen.")).into());
    }
    let (work, key, _) = target(state, &idea, key).await?;
    let persona = match text(params, "persona")?.filter(|p| !p.trim().is_empty()) {
        Some(p) => p,
        None => state
            .store
            .load_work(work)
            .await?
            .lector()
            .map(|p| p.handle.clone())
            .ok_or_else(|| RpcError::new(REFUSED, "Keine Persona ist aktiv (kein Lektor)."))?,
    };
    let whose = match &idea.origin {
        IdeaOrigin::Human { .. } => "des Autors".to_owned(),
        other => format!("von {}", other.name()),
    };
    let order = crate::assist::Order {
        work,
        key,
        persona,
        mode: crate::assist::Mode::Ask,
        text: format!("Einfall {whose} zu diesem Abschnitt: {}", post_body(&idea)),
        rev: None,
        asker: None,
        post_question: false,
        sources: vec![idea_source(&idea)],
        web: true,
        scope: crate::assist::Scope::Section,
        frame: Default::default(),
        links: Default::default(),
        basis: None,
    };
    let outcome = crate::assist::run(state, &order, None).await?;
    let post_id = outcome.post.as_ref().map_or(0, |p| p.id);
    let held = state.write_lock.lock().await;
    let done_ref = format!("key {key}, Beitrag {post_id}");
    let detail = format!("Auftrag an {} (key {key}, Beitrag {post_id})", outcome.persona);
    let idea = state.store.idea_mark_done(id, IdeaUse::Persona, &done_ref).await?;
    let human = IdeaOrigin::Human { name: mirror::HUMAN_NAME.to_owned() };
    let request = request_for(&idea, &human, "idea-process", &detail);
    let seq = state.store.node_seq(work, key).await?;
    let mut answer = json!({
        "node": seq, "key": key, "post_id": post_id, "persona": outcome.persona, "model": outcome.model,
        "truncated": outcome.truncated, "url": outcome.url,
    });
    answer["idea"] = json!(idea);
    add_mirror(&mut answer, mirror::record_idea(state, &held, idea.work, request).await);
    Ok(answer)
}

pub async fn call(state: &AppState, method: &str, params: Json) -> Result<Json, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    let human = IdeaOrigin::Human { name: mirror::HUMAN_NAME.to_owned() };
    match method {
        "idea.create" => {
            let words = words_param(&params)?.ok_or_else(|| invalid("words fehlt (mindestens ein Schlagwort)"))?;
            let work = optional_ref(&params, "work")?.flatten();
            let key = optional_ref(&params, "key")?.flatten();
            let (note, quote, source, bible) =
                (text(&params, "note")?, text(&params, "quote")?, text(&params, "source")?, text(&params, "bible")?);
            let new = NewIdea {
                work,
                key,
                words: &words,
                note: note.as_deref().unwrap_or(""),
                quote: quote.as_deref().unwrap_or(""),
                source: source.as_deref().unwrap_or(""),
                bible: bible.as_deref().unwrap_or(""),
                origin: human,
            };
            create(state, &new).await
        }
        "idea.get" => Ok(json!({ "idea": state.store.idea_get(id_param(&params)?).await? })),
        "idea.list" => {
            let filter = IdeaFilter {
                scope: list_scope(state, &params).await?,
                word: text(&params, "word")?.filter(|w| !w.trim().is_empty()),
                status: list_status(&params)?,
                deleted: params.get("deleted").and_then(Json::as_bool).unwrap_or(false),
                limit: limit_param(&params, "limit")?,
                offset: limit_param(&params, "offset")?,
            };
            let ideas = state.store.idea_list(&filter).await?;
            let words = state.store.idea_words(&filter).await?;
            Ok(json!({ "ideas": ideas, "words": words }))
        }
        "idea.search" if search_mode(&params)? == "rag" => {
            let query = text(&params, "query")?.unwrap_or_default();
            if query.trim().is_empty() {
                return Err(invalid("Suchbegriff fehlt."));
            }
            let work = match list_scope(state, &params).await? {
                IdeaScope::Work(work) => work,
                _ => return Err(invalid("Bedeutungssuche nur innerhalb eines Werks (work: Werk-id).")),
            };
            let limit = match limit_param(&params, "limit")? {
                0 => DEFAULT_LIMIT,
                n => (n as usize).min(MAX_LIMIT),
            };
            let (found, answer) =
                rag_ideas(state, work, &query, list_status(&params)?, limit).await.map_err(|e| match e {
                    RagError::Invalid(message) => invalid(message),
                    e => rag_unavailable(e.message()),
                })?;
            // Runde 19c: Ist LanceDB (der einzige Weg der Einfälle) nicht verbunden, ist die Suche ausgefallen –
            // ein Fehler mit Grund statt einer leeren Liste, die wie „keine Treffer“ aussähe.
            if !answer.ways.iter().any(|w| w.way == Way::Lancedb && w.state == WayState::Connected) {
                let reason = answer
                    .ways
                    .iter()
                    .find(|w| w.way == Way::Lancedb)
                    .map(|w| format!("{}: {}", w.way.title(), w.message.trim().trim_end_matches('.')))
                    .unwrap_or_else(|| "LanceDB wurde nicht gefragt".to_owned());
                return Err(rag_unavailable(&reason));
            }
            let hits: Vec<Json> = found
                .iter()
                .map(|(idea, snippet, score)| {
                    json!({ "id": idea.id, "snippet": snippet, "score": score, "weak": baruch_rag::is_weak(*score) })
                })
                .collect();
            let ideas: Vec<&Idea> = found.iter().map(|(idea, ..)| idea).collect();
            Ok(json!({ "ideas": ideas, "hits": hits, "ways": answer.ways }))
        }
        "idea.search" => {
            let query = text(&params, "query")?.unwrap_or_default();
            let fts = baruch_library::fts_query(&query).ok_or_else(|| invalid("Suchbegriff fehlt."))?;
            let filter = IdeaFilter {
                scope: list_scope(state, &params).await?,
                status: list_status(&params)?,
                limit: limit_param(&params, "limit")?,
                ..IdeaFilter::default()
            };
            let found = state.store.idea_search(&fts, &filter).await?;
            let hits: Vec<Json> =
                found.iter().map(|(idea, snippet)| json!({ "id": idea.id, "snippet": snippet })).collect();
            let ideas: Vec<&Idea> = found.iter().map(|(idea, _)| idea).collect();
            Ok(json!({ "ideas": ideas, "hits": hits }))
        }
        "idea.update" => {
            let id = id_param(&params)?;
            let words = words_param(&params)?;
            let (note, quote, source, bible) =
                (text(&params, "note")?, text(&params, "quote")?, text(&params, "source")?, text(&params, "bible")?);
            let status = match text(&params, "status")? {
                None => None,
                Some(s) => Some(IdeaStatus::parse(s.trim()).ok_or_else(|| invalid("status: open oder done erwartet"))?),
            };
            let edit = IdeaEdit {
                words: words.as_deref(),
                note: note.as_deref(),
                quote: quote.as_deref(),
                source: source.as_deref(),
                bible: bible.as_deref(),
                work: optional_ref(&params, "work")?,
                key: optional_ref(&params, "key")?,
                status,
            };
            update(state, id, &edit, &human).await
        }
        "idea.delete" => {
            if params.get("confirm") != Some(&Json::Bool(true)) {
                return Err(invalid("Löschen bitte bestätigen (confirm)."));
            }
            delete(state, id_param(&params)?, &human).await
        }
        "idea.restore" => restore(state, id_param(&params)?, &human).await,
        "idea.process" => process(state, &params).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

/// Runde 19c: Ausfall der Bedeutungssuche als -32001 ([`crate::rpc::RAG_UNAVAILABLE`]) mit Grund.
fn rag_unavailable(reason: &str) -> RpcError {
    RpcError::new(
        crate::rpc::RAG_UNAVAILABLE,
        format!("Bedeutungssuche nicht verfügbar: {}.", reason.trim().trim_end_matches('.')),
    )
}

/// `mode` von `idea.search`: fehlt/leer/`fts` → `"fts"`, `rag` → `"rag"`, sonst -32602.
fn search_mode(params: &Json) -> Result<&'static str, RpcError> {
    match params.get("mode") {
        None | Some(Json::Null) => Ok("fts"),
        Some(Json::String(s)) if s.trim().is_empty() || s.trim() == "fts" => Ok("fts"),
        Some(Json::String(s)) if s.trim() == "rag" => Ok("rag"),
        Some(_) => Err(invalid("mode: fts oder rag erwartet")),
    }
}

/// Einfall eines Treffers: `meta.idea` (Zahl) bzw. Dokument `idea:<id>`.
fn idea_of_hit(hit: &baruch_rag::Hit) -> Option<IdeaId> {
    if let Some(id) = hit.meta.get("idea").and_then(Json::as_i64) {
        return Some(id);
    }
    hit.document.trim().strip_prefix("idea:").and_then(|id| id.trim().parse().ok())
}

/// Bedeutungssuche über die Einfälle eines Werks (Weg LanceDB, Art `idea`): Einfälle in der Reihenfolge des
/// Rangs mit Ausschnitt und Ähnlichkeit, schwache zuletzt (Runde 19c); gelöschte, unbekannte und nach `status`
/// nicht passende fallen heraus.
pub(crate) async fn rag_ideas(
    state: &AppState,
    work: WorkId,
    query: &str,
    status: Option<IdeaStatus>,
    limit: usize,
) -> Result<(Vec<(Idea, String, f32)>, Answer), RagError> {
    let request = SearchRequest {
        work,
        query: query.to_owned(),
        ways: Some(vec![Way::Lancedb]),
        collections: Some(vec!["idea".to_owned()]),
        limit,
        origin: mirror::HUMAN_NAME.to_owned(),
    };
    let answer = crate::search_hooks::rag_search(state, &request).await?;
    let mut found: Vec<(Idea, String, f32)> = Vec::new();
    for merged in &answer.hits {
        let Some(id) = idea_of_hit(&merged.hit) else { continue };
        if found.iter().any(|(idea, ..)| idea.id == id) {
            continue;
        }
        let Ok(idea) = state.store.idea_get(id).await else { continue };
        if idea.deleted.is_some() || status.is_some_and(|s| s != idea.status) {
            continue;
        }
        found.push((idea, merged.hit.snippet.clone(), merged.hit.score));
    }
    // Runde 19c: schwache Treffer hinter die übrigen (sonst Reihenfolge des Rangs).
    found.sort_by_key(|(_, _, score)| baruch_rag::is_weak(*score));
    Ok((found, answer))
}

// ---- Seiten ------------------------------------------------------------------------------------

/// Eigene Template-Umgebung der Seiten (die der Arbeitsseite bleibt unberührt).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("ideas.html", include_str!("../templates/ideas.html")),
            ("idea.html", include_str!("../templates/idea.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Einfälle fehlerhaft");
        }
        env
    })
}

fn render(name: &str, status: StatusCode, ctx: Value) -> Response {
    match templates().get_template(name).and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage {name}: {e}")).into_response(),
    }
}

/// Rückfall, wenn eine selbst gebaute Adresse doch ein unsicheres Zeichen enthielte.
const FALLBACK_HREF: &str = "/einfaelle";

/// Darf die Adresse ungeprüft (als sicherer Wert) in ein `href`? Nein bei `"` `<` `>` `'`, Leerzeichen
/// und Steuerzeichen, die das Attribut beenden oder etwas einschleusen könnten.
fn is_safe_href(url: &str) -> bool {
    !url.chars().any(|c| matches!(c, '"' | '<' | '>' | '\'' | ' ') || c.is_control())
}

/// Selbst gebaute Adresse (nur Ziffern, feste Namen, prozentkodierte Werte) als sicherer Wert für
/// `href`; `&` wird maskiert, `/` bleibt lesbar. Die Prüfung gilt auch im Release (Runde 8, früher nur
/// `debug_assert!`): Enthielte die Adresse doch ein unsicheres Zeichen, steht statt ihrer die Liste der
/// Einfälle da, und stderr nennt die Adresse.
fn href(url: String) -> Value {
    if !is_safe_href(&url) {
        eprintln!("Einfälle: unsichere Adresse {url:?} durch {FALLBACK_HREF} ersetzt");
        return Value::from_safe_string(FALLBACK_HREF.to_owned());
    }
    Value::from_safe_string(url.replace('&', "&amp;"))
}

fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn settings_of(headers: &HeaderMap) -> Value {
    Value::from(Serde(Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()))))
}

/// Filter der Liste, wie er in der Adresse steht (`?work=<id>|none|all&word=&status=&q=&deleted=1`).
#[derive(Debug, Clone, Default, Deserialize)]
struct ListQuery {
    work: Option<String>,
    word: Option<String>,
    status: Option<String>,
    q: Option<String>,
    deleted: Option<String>,
    ok: Option<String>,
    /// `fts` (Vorgabe) oder `rag` (Runde 12).
    mode: Option<String>,
}

impl ListQuery {
    fn work(&self) -> String {
        self.work.as_deref().map(str::trim).filter(|w| !w.is_empty()).unwrap_or("all").to_owned()
    }

    fn status(&self) -> String {
        match self.status.as_deref().map(str::trim) {
            Some(s @ ("open" | "done")) => s.to_owned(),
            _ => "all".to_owned(),
        }
    }

    fn deleted(&self) -> bool {
        matches!(self.deleted.as_deref(), Some("1" | "on" | "true"))
    }

    /// Bedeutungssuche gewählt (`mode=rag`)?
    fn rag(&self) -> bool {
        self.mode.as_deref().map(str::trim) == Some("rag")
    }

    /// Adresse der Liste mit diesem Filter, `word` ersetzt.
    fn url(&self, word: Option<&str>) -> String {
        let mut url = format!("/einfaelle?work={}", query_escape(&self.work()));
        if let Some(word) = word.filter(|w| !w.trim().is_empty()) {
            url.push_str(&format!("&word={}", query_escape(word)));
        }
        if self.status() != "all" {
            url.push_str(&format!("&status={}", self.status()));
        }
        if self.deleted() {
            url.push_str("&deleted=1");
        }
        if self.rag() {
            url.push_str("&mode=rag");
        }
        url
    }

    /// Dieselbe Suche im Wortlaut (FTS): Liste mit `q`, ohne `mode`.
    fn fts_url(&self) -> String {
        let mut url = ListQuery { mode: None, ..self.clone() }.url(self.word.as_deref());
        if let Some(q) = self.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            url.push_str(&format!("&q={}", query_escape(q)));
        }
        url
    }
}

/// Werkwahl für Filter und Formulare: `[{id, title, selected}]`.
async fn work_choices(store: &Store, selected: &str) -> Result<Vec<Value>, StoreError> {
    Ok(store
        .works()
        .await?
        .into_iter()
        .map(|(id, _, title)| context! { id => id, title => title, selected => id.to_string() == selected })
        .collect())
}

fn short_time(time: &str) -> String {
    crate::localtime::minutes(time)
}

/// Ein Einfall für die Vorlagen.
fn idea_view(idea: &Idea, titles: &[(WorkId, String, String)], snippet: Option<&str>) -> Value {
    let work_title = idea.work.and_then(|w| titles.iter().find(|(id, ..)| *id == w).map(|(_, _, t)| t.clone()));
    // Zum Werk bzw. zum Abschnitt des Bezugs, in Key-Form (Vertrag Runde 8, §1).
    let link = idea.work.map(|work| match idea.key {
        Some(key) => href(format!("/?work={work}&key={key}")),
        None => href(format!("/?work={work}")),
    });
    let node_link = idea.work.zip(idea.key).map(|(work, key)| href(format!("/?work={work}&key={key}")));
    context! {
        id => idea.id,
        href => href(format!("/einfaelle/{}", idea.id)),
        words => idea.words.clone(),
        words_text => idea.words.join(", "),
        note => idea.note.as_str(),
        quote => idea.quote.as_str(),
        source => idea.source.as_str(),
        bible => idea.bible.as_str(),
        work => idea.work,
        work_title => work_title,
        work_href => link,
        node_href => node_link,
        key => idea.key,
        node_title => idea.node_title.as_deref(),
        origin => idea.origin.label(),
        origin_kind => idea.origin.kind(),
        status => idea.status.as_str(),
        open => idea.status == IdeaStatus::Open,
        done_label => idea.done_as.map(IdeaUse::label),
        done_ref => idea.done_ref.as_deref(),
        created => short_time(&idea.created),
        updated => short_time(&idea.updated),
        deleted => idea.deleted.as_deref().map(short_time),
        // Ausschnitt der Suche: vom Store maskiert, nur <mark> bleibt.
        snippet => snippet.map(|s| Value::from_safe_string(s.to_owned())),
    }
}

fn cloud(words: &[WordCount], query: &ListQuery) -> Vec<Value> {
    let max = words.iter().map(|w| w.count).max().unwrap_or(1).max(1);
    let active = query.word.as_deref().map(str::to_lowercase);
    words
        .iter()
        .map(|w| {
            // Größe 1–4 nach Häufigkeit.
            let size = 1 + (w.count.saturating_sub(1) * 3) / max.max(1);
            context! {
                word => w.word.as_str(), count => w.count, size => size.min(4),
                href => href(query.url(Some(&w.word))),
                active => active.as_deref() == Some(w.word.to_lowercase().as_str()),
            }
        })
        .collect()
}

/// `GET /einfaelle`: Liste, Schlagwort-Wolke, Filter, Suche, Formular „Neuer Einfall“.
async fn list_page(
    state: &AppState,
    headers: &HeaderMap,
    query: &ListQuery,
    status: StatusCode,
    error: Option<String>,
) -> Response {
    let params = json!({ "work": query.work(), "status": query.status() });
    let failed = |e: RpcError| {
        if e.code == INVALID_PARAMS && e.message.starts_with("unbekanntes Werk") {
            return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response();
        }
        (status_of(&e), format!("Fehler: {}", e.message)).into_response()
    };
    let scope = match list_scope(state, &params).await {
        Ok(scope) => scope,
        Err(e) => return failed(e),
    };
    let filter = IdeaFilter {
        scope,
        word: query.word.clone().filter(|w| !w.trim().is_empty()),
        status: IdeaStatus::parse(&query.status()),
        deleted: query.deleted(),
        ..IdeaFilter::default()
    };
    let titles = match state.store.works().await {
        Ok(titles) => titles,
        Err(e) => return failed(e.into()),
    };
    let search = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    // Bedeutungssuche (Runde 12): nur mit gewähltem Werk; sonst Hinweis und Wortlaut-Suche.
    let mut rag_note: Option<&str> = None;
    let mut rag: Option<Json> = None;
    let rag_work = match (search, query.rag(), &filter.scope) {
        (Some(_), true, IdeaScope::Work(work)) => Some(*work),
        (Some(_), true, _) => {
            rag_note = Some("Bedeutungssuche nur innerhalb eines Werks; gezeigt wird die Wortlaut-Suche (FTS).");
            None
        }
        _ => None,
    };
    // Eine fremde Seite darf die Bedeutungssuche nicht per GET auslösen (wie bei der Literatur).
    let own_page = crate::guard::check(headers).is_ok();
    let rag_found = match (rag_work, search) {
        (Some(_), Some(_)) if !own_page => {
            rag = Some(crate::literature::rag_unavailable(crate::literature::FOREIGN_PAGE, None));
            Some(Vec::new())
        }
        (Some(work), Some(q)) => match rag_ideas(state, work, q, filter.status, DEFAULT_LIMIT).await {
            Ok((found, answer)) if answer.ways.iter().any(|w| w.state == WayState::Connected) => {
                rag = Some(json!({ "available": true, "ways_line": crate::literature::ways_line(&answer) }));
                Some(found)
            }
            Ok((_, answer)) => {
                rag = Some(crate::literature::rag_view(&answer));
                Some(Vec::new())
            }
            Err(e) => {
                rag = Some(crate::literature::rag_unavailable(e.message(), None));
                Some(Vec::new())
            }
        },
        _ => None,
    };
    // Runde 19c: Zahl der schwachen Treffer (zugeklappt in „n schwache Treffer“).
    let weak_count =
        rag_found.as_ref().map_or(0, |found| found.iter().filter(|(_, _, score)| baruch_rag::is_weak(*score)).count());
    let (ideas, searched): (Vec<Value>, bool) = match (rag_found, search) {
        (Some(found), _) => (
            found
                .iter()
                .map(|(idea, snippet, score)| {
                    let percent = (f64::from(score.clamp(0.0, 1.0)) * 100.0).round() as i64;
                    let weak = baruch_rag::is_weak(*score);
                    context! { rag_snippet => snippet.as_str(), percent => percent, weak => weak,
                    ..idea_view(idea, &titles, None) }
                })
                .collect(),
            true,
        ),
        (None, None) => match state.store.idea_list(&filter).await {
            Ok(list) => (list.iter().map(|i| idea_view(i, &titles, None)).collect(), false),
            Err(e) => return failed(e.into()),
        },
        (None, Some(q)) => match baruch_library::fts_query(q) {
            Some(fts) => match state.store.idea_search(&fts, &filter).await {
                Ok(found) => (found.iter().map(|(i, s)| idea_view(i, &titles, Some(s))).collect(), true),
                Err(e) => return failed(e.into()),
            },
            None => (Vec::new(), true),
        },
    };
    let words = match state.store.idea_words(&filter).await {
        Ok(words) => words,
        Err(e) => return failed(e.into()),
    };
    let work = query.work();
    let works = match work_choices(&state.store, &work).await {
        Ok(works) => works,
        Err(e) => return failed(e.into()),
    };
    let workspace = work.parse::<WorkId>().ok().map(|w| href(format!("/?work={w}")));
    let notice = query.ok.as_deref().map(|ok| match ok {
        "geloescht" => "Der Einfall ist gelöscht; unter „Gelöschte“ lässt er sich wiederherstellen.",
        _ => "Gespeichert.",
    });
    let ctx = context! {
        settings => settings_of(headers),
        filter => context! {
            work => work.as_str(), word => query.word.as_deref().unwrap_or(""), status => query.status(),
            q => search.unwrap_or(""), deleted => query.deleted(),
            mode => if query.rag() { "rag" } else { "fts" },
        },
        rag => rag.as_ref().map_or(Value::UNDEFINED, |r| Value::from(Serde(r))),
        rag_note => rag_note,
        fts_href => href(query.fts_url()),
        works => works,
        ideas => ideas,
        weak_count => weak_count,
        weak_label => baruch_rag::WEAK_LABEL,
        searched => searched,
        cloud => cloud(&words, query),
        all_words => href(query.url(None)),
        deleted_href => href(format!("/einfaelle?work={}&deleted=1", query_escape(&work))),
        present_href => href(format!("/einfaelle?work={}", query_escape(&work))),
        workspace => workspace,
        // Runde 25: Link „Gedankenkarte“ (nur mit gewähltem Werk).
        map_href => work.parse::<WorkId>().ok().map(|w| href(format!("/werke/{w}/karte"))),
        back => query.url(query.word.as_deref()),
        notice => notice,
        error => error,
    };
    render("ideas.html", status, ctx)
}

async fn ideas_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    list_page(&state, &headers, &query, StatusCode::OK, None).await
}

#[derive(Debug, Clone, Default, Deserialize)]
struct IdeaQuery {
    ok: Option<String>,
}

/// `GET /einfaelle/{id}`: Ansicht, Bearbeiten, Löschen, Wiederherstellen, Verarbeiten.
async fn one_page(
    state: &AppState,
    headers: &HeaderMap,
    id: IdeaId,
    status: StatusCode,
    notice: Option<&str>,
    error: Option<String>,
) -> Response {
    let idea = match state.store.idea_get(id).await {
        Ok(idea) => idea,
        Err(StoreError::UnknownIdea(_)) => {
            return (StatusCode::NOT_FOUND, format!("Einfall {id} gibt es nicht.")).into_response();
        }
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let titles = match state.store.works().await {
        Ok(titles) => titles,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let selected = idea.work.map_or_else(|| "none".to_owned(), |w| w.to_string());
    let works = match work_choices(&state.store, &selected).await {
        Ok(works) => works,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    // Runde 15: Persona-Auswahl für „Auftrag an eine Persona“ (aktive Personas, Vorgabe der Lektor des Werks).
    let lector = match idea.work {
        Some(work) => state.store.load_work(work).await.ok().and_then(|w| w.lector().map(|p| p.handle.clone())),
        None => None,
    };
    let personas: Vec<Value> = state
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
        .collect();
    // Abschnitte des Werks für Bezug und Verarbeitung (Titel mit Nummer).
    let nodes: Vec<Value> = match idea.work {
        Some(work) => match state.store.load_work(work).await {
            Ok(loaded) => loaded
                .nodes
                .iter()
                .map(|n| {
                    let label = if n.number.is_empty() { n.title.clone() } else { format!("{} {}", n.number, n.title) };
                    context! { key => n.key, label => label, selected => Some(n.key) == idea.key }
                })
                .collect(),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
        },
        None => Vec::new(),
    };
    let list = idea.work.map_or_else(|| "none".to_owned(), |w| w.to_string());
    // Runde 25: Verbindungen des Einfalls (nur aktive, zu nicht gelöschten Einfällen), je mit Formular „Lösen“ an
    // die Karte des Werks.
    let links = idea_links_view(state, &idea).await;
    let ctx = context! {
        settings => settings_of(headers),
        idea => idea_view(&idea, &titles, None),
        works => works,
        nodes => nodes,
        personas => personas,
        title_default => title_from_words(&idea.words),
        links => links,
        map_href => idea.work.map(|w| href(format!("/werke/{w}/karte"))),
        map_act => idea.work.map(|w| href(format!("/werke/{w}/karte/act"))),
        list_href => href(format!("/einfaelle?work={}", query_escape(&list))),
        back => format!("/einfaelle/{id}"),
        notice => notice,
        error => error,
    };
    render("idea.html", status, ctx)
}

/// Verbindungen eines Einfalls für die Seite `/einfaelle/{id}` (Runde 25): Pfeil wie im Spiegel (`→` ausgehend,
/// `←` eingehend, `↔` ungerichtet), anderer Einfall mit Schlagwörtern, Art, Notiz; nach Verbindungs-id.
async fn idea_links_view(state: &AppState, idea: &Idea) -> Vec<Value> {
    let links = state.store.idea_links_of_idea(idea.id, false).await.unwrap_or_default();
    let mut out = Vec::with_capacity(links.len());
    for link in links {
        let other = if link.from == idea.id { link.to } else { link.from };
        let arrow = if link.kind.symmetric() {
            "↔"
        } else if link.from == idea.id {
            "→"
        } else {
            "←"
        };
        let other_words = state.store.idea_get(other).await.map(|o| o.words.join(", ")).unwrap_or_default();
        out.push(context! {
            id => link.id, arrow => arrow, other => other, other_href => href(format!("/einfaelle/{other}")),
            other_words => other_words, kind => link.kind.as_str(), kind_label => link.kind.label(),
            note => link.note,
        });
    }
    out
}

async fn idea_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<IdeaId>,
    Query(query): Query<IdeaQuery>,
) -> Response {
    let notice = query.ok.as_deref().map(|ok| match ok {
        "angelegt" => "Der Einfall ist festgehalten.",
        "wiederhergestellt" => "Der Einfall ist wiederhergestellt.",
        "verarbeitet" => "Der Einfall ist verarbeitet.",
        _ => "Gespeichert.",
    });
    one_page(&state, &headers, id, StatusCode::OK, notice, None).await
}

/// Formular an `POST /einfaelle/act`: dieselben Felder wie die RPC-Methoden, alles als Text.
#[derive(Debug, Default, Deserialize)]
struct ActForm {
    method: String,
    id: Option<String>,
    work: Option<String>,
    key: Option<String>,
    words: Option<String>,
    note: Option<String>,
    quote: Option<String>,
    source: Option<String>,
    bible: Option<String>,
    status: Option<String>,
    confirm: Option<String>,
    #[serde(rename = "as")]
    used: Option<String>,
    place: Option<String>,
    title: Option<String>,
    /// Persona für `as=persona` (Runde 15; leer = der Lektor des Werks).
    persona: Option<String>,
    /// Wohin danach (nur Adressen dieses Servers, beginnend mit genau einem `/`).
    back: Option<String>,
}

/// Rücksprung nur auf eigene Adressen: beginnt mit `/`, nicht mit `//` oder `/\`, ohne Steuerzeichen.
fn safe_back(back: Option<&str>) -> Option<String> {
    let back = back?.trim();
    let ok = back.starts_with('/')
        && !back.starts_with("//")
        && !back.starts_with("/\\")
        && !back.chars().any(|c| c.is_control() || c == ' ' || c == '"' || c == '<' || c == '>');
    ok.then(|| back.to_owned())
}

/// Formularwert → JSON: fehlt → nicht gesetzt.
fn put(params: &mut Json, name: &str, value: &Option<String>) {
    if let Some(value) = value {
        params[name] = json!(value);
    }
}

/// `POST /einfaelle/act`: `method` = idea.create|idea.update|idea.delete|idea.restore|idea.process;
/// danach 303 zurück (`back` bzw. die Seite des Einfalls), Fehler 422 (409 bei fachlicher
/// Ablehnung) mit der Seite und der Meldung. Löschen braucht das Kästchen `confirm`.
async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let allowed = ["idea.create", "idea.update", "idea.delete", "idea.restore", "idea.process"];
    if !allowed.contains(&form.method.as_str()) {
        return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response();
    }
    let id: Option<IdeaId> = form.id.as_deref().and_then(|id| id.trim().parse().ok());
    let mut params = json!({});
    if let Some(id) = id {
        params["id"] = json!(id);
    }
    for (name, value) in [
        ("work", &form.work),
        ("key", &form.key),
        ("words", &form.words),
        ("note", &form.note),
        ("quote", &form.quote),
        ("source", &form.source),
        ("bible", &form.bible),
        ("status", &form.status),
        ("as", &form.used),
        ("place", &form.place),
        ("title", &form.title),
        ("persona", &form.persona),
    ] {
        put(&mut params, name, value);
    }
    if form.method == "idea.delete" {
        // Der Server prüft das Kästchen selbst; `required` im HTML allein hält niemanden auf.
        if !matches!(form.confirm.as_deref(), Some("1" | "on")) {
            return match id {
                Some(id) => {
                    one_page(
                        &state,
                        &headers,
                        id,
                        StatusCode::UNPROCESSABLE_ENTITY,
                        None,
                        Some("Löschen bitte bestätigen.".into()),
                    )
                    .await
                }
                None => (StatusCode::UNPROCESSABLE_ENTITY, "Löschen bitte bestätigen.").into_response(),
            };
        }
        params["confirm"] = json!(true);
    }
    if form.method == "idea.update"
        && let Some(id) = id
        && let Ok(before) = state.store.idea_get(id).await
    {
        // Wechselt das Werk im Formular, gehört der alte Bezug nicht mehr dazu.
        let work = form.work.as_deref().map(str::trim).and_then(|w| w.parse::<WorkId>().ok());
        let key = form.key.as_deref().map(str::trim).and_then(|k| k.parse::<i64>().ok());
        if form.work.is_some() && work != before.work && key == before.key {
            params["key"] = Json::Null;
        }
    }
    match call(&state, &form.method, params).await {
        Ok(answer) => {
            let target = answer["idea"]["id"].as_i64().or(id);
            let url = match (safe_back(form.back.as_deref()), form.method.as_str(), target) {
                (Some(back), ..) => back,
                (None, "idea.delete", _) => "/einfaelle?ok=geloescht".to_owned(),
                (None, "idea.create", Some(t)) => format!("/einfaelle/{t}?ok=angelegt"),
                (None, "idea.restore", Some(t)) => format!("/einfaelle/{t}?ok=wiederhergestellt"),
                (None, "idea.process", Some(t)) => format!("/einfaelle/{t}?ok=verarbeitet"),
                (None, _, Some(t)) => format!("/einfaelle/{t}?ok=1"),
                (None, _, None) => "/einfaelle".to_owned(),
            };
            Redirect::to(&url).into_response()
        }
        Err(e) if e.code == INTERNAL => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response()
        }
        Err(e) => {
            let status = status_of(&e);
            match (form.method.as_str(), id) {
                ("idea.create", _) | (_, None) => {
                    let query = ListQuery { work: form.work.clone(), ..ListQuery::default() };
                    list_page(&state, &headers, &query, status, Some(e.message)).await
                }
                (_, Some(id)) => one_page(&state, &headers, id, status, None, Some(e.message)).await,
            }
        }
    }
}

/// `GET /einfaelle`, `GET /einfaelle/{id}`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/einfaelle", get(ideas_page)).route("/einfaelle/{id}", get(idea_page))
}

/// `POST /einfaelle/act` (im Router der schreibenden Routen, also hinter `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/einfaelle/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_come_as_a_list_or_as_text_with_commas_and_semicolons() {
        assert_eq!(
            split_words(&json!("Laube, Rose; Dorn"), "words").unwrap(),
            vec!["Laube", " Rose", " Dorn"],
            "Text wird an Komma und Semikolon geteilt (getrimmt wird im Store)"
        );
        assert_eq!(split_words(&json!(["a", "b"]), "words").unwrap(), vec!["a", "b"], "Liste bleibt");
        assert!(split_words(&json!([1]), "words").is_err(), "Liste nur aus Texten");
    }

    #[test]
    fn titles_from_words_are_cut_at_200_characters() {
        let words: Vec<String> = (0..30).map(|i| format!("Gartenwort{i:02}")).collect();
        let title = title_from_words(&words);
        assert!(title.chars().count() <= MAX_TITLE, "höchstens 200 Zeichen: {}", title.chars().count());
        assert!(title.starts_with("Gartenwort00, Gartenwort01"), "mit „, “ verbunden: {title}");
    }

    #[test]
    fn going_back_only_goes_to_own_addresses() {
        assert_eq!(safe_back(Some("/?work=1&key=2")), Some("/?work=1&key=2".into()), "eigene Adresse");
        for bad in ["//fremd.example/", "https://fremd.example/", "/\\fremd", "/a b", "/\"x"] {
            assert_eq!(safe_back(Some(bad)), None, "{bad} ist keine eigene Adresse");
        }
    }

    #[test]
    fn unsafe_addresses_fall_back_to_the_list_even_in_release() {
        // Kein debug_assert mehr: Der Test prüft das Ergebnis, nicht eine Panik im Debug-Bau.
        for bad in ["/einfaelle?q=\"><script>", "/a b", "/x'y", "/x<y", "/zeile\nzwei", "/tab\tx", "/nul\0x"] {
            assert_eq!(href(bad.to_owned()).to_string(), FALLBACK_HREF, "{bad:?} wird nicht ausgegeben");
        }
        assert_eq!(
            href("/?work=1&key=12".to_owned()).to_string(),
            "/?work=1&amp;key=12",
            "eine sichere Adresse bleibt, & maskiert, / lesbar"
        );
        assert!(href("/?work=1".to_owned()).is_safe(), "als sicherer Wert (wird nicht doppelt maskiert)");
    }

    #[test]
    fn only_idea_methods_are_handled_here() {
        assert!(handles("idea.create") && handles("idea.process"), "idea.* gehört hierher");
        assert!(!handles("idea.purge") && !handles("post.add"), "anderes nicht");
    }
}
