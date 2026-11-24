// SPDX-License-Identifier: AGPL-3.0-or-later
//! Beiträge im Abschnitts-Chat verwalten (Vertrag Runde 8, 3d): löschen, verschieben, in einen
//! Einfall umwandeln – über RPC, über das Formular `POST /beitraege/act` (ohne JavaScript) und für
//! MCP über [`act`] mit der Regel „nur eigene“.
//!
//! RPC ([`handles`], [`call`]; aus `rpc::call` heraus, alle nehmen `work?`; Handelnder ist der Mensch):
//! - `post.delete {id, confirm: true}` → `{id, node, key, deleted: {author, body}, commit?|mirror?}`;
//!   ohne `confirm: true` → -32602
//! - `post.move {id, target_key | target}` → `{id, node, key, from_node, from_key, commit?|mirror?}`
//!   (ans Ende des Ziels; gleiches Ziel → -32000)
//! - `post.to_idea {id, words}` → `{id, idea, node, key, commit?|mirror?}` (`words` wie bei
//!   `idea.create`: Liste oder Text „a, b; c“)
//!
//! Regeln: Beiträge sind kein Buchtext. Der Mensch darf jeden Beitrag verwalten, MCP-Clients und
//! Personas nur ihre eigenen (Autor = Persona-Handle aus `persona` bzw. sonst `@mcp:<name>`), geprüft
//! hier unter der Schreibsperre, bevor sich etwas ändert. Jede Aktion läuft unter
//! [`AppState::write_lock`] und ergibt genau einen Commit im Spiegel des Werks
//! ([`mirror::record_post`]); die Umwandlung ist **ein** Commit, dessen Schnappschuss schon den
//! neuen Einfall in `einfaelle.md` enthält.
//!
//! Runde 28 (chat-auftrag): Chat bedienbar machen.
//! - `post.edit {id, body}` → `{id, key, edited, body, versions, url, commit?|mirror?}`: nur Beiträge des Menschen
//!   (Autor „Autor“); über MCP `post_edit` nur eigene (`@mcp:<name>`). Beiträge von Personas sind nie bearbeitbar
//!   (nur kopieren, zitieren, in Einfall oder Befund umwandeln). Die alte Fassung bleibt als `post_version`;
//!   genau ein Commit „Beitrag <id>: bearbeitet“ mit Text vorher und nachher ([`mirror::PostAction::Edit`]).
//! - `post.versions {id}` → `{id, versions}` (älteste zuerst).
//! - `post.page {key, before?, limit?}` → `{posts, older}`: die bis zu `limit` (1–200, Vorgabe [`CHAT_PAGE_POSTS`])
//!   Beiträge vor dem Beitrag `before` (ohne: die neuesten); `older` = wie viele davor noch stehen.
//! - `post.to_findings {id}` → `{id, findings, attached}`: Befunde aus dem Beitrag über
//!   [`crate::findings::ingest`] mit `manual = true` (ohne Marken ein Hinweis mit dem ganzen Text); der Beitrag
//!   bleibt unverändert, kein Commit (das Anlegen von Befunden ist keiner, Vertrag R28 §1.2).
//! - Seiten ohne JavaScript ([`pages`]): `GET /beitraege?work&key` (ganze Chat-Historie, Anker `#post-<id>` und
//!   `#chat-neueste`), `GET /beitrag?work&id` (Text in einem markierbaren, schreibgeschützten Feld zum Kopieren,
//!   „Zitieren“ als eigenes Chat-Formular mit „> …“, Bearbeiten eigener Beiträge, Verlauf, „In Befund umwandeln“).
//!   `POST /beitraege/act` nimmt dazu `post.edit` (Feld `body`) und `post.to_findings`; `back=post` führt danach
//!   auf die Beitragsseite zurück.
//! - Der Abschnitts-Chat zeigt nur die neuesten [`CHAT_PAGE_POSTS`] ([`visible_range`]); ältere lädt das Widget
//!   über `post.page` nach, ohne JavaScript führt ein Link auf `/beitraege`.
//!
//! Runde 30c (chat-bedienung):
//! - `post.repeat {id, rev?, basis?}` → `{post, url, repeat_of, basis, proposed_done, notes}` ([`repeat`]): dieselbe
//!   Anfrage (Persona, Modus, Umfang, Text aus `PostMeta::request`) gegen die gezeigte (`rev`) bzw. nach `basis`
//!   gewählte Fassung; der neue Beitrag trägt `repeat_of`, der alte bleibt. Offene Befunde der alten Antwort, die in
//!   der neuen nicht wiederkommen ([`crate::findings::parse::similar`]), schlägt die Persona als „erledigt“ vor
//!   (`finding_propose_status` über [`crate::findings::rpc::propose`], je ein Commit). Rückfall für Beiträge ohne
//!   gespeicherten Auftrag (vor Runde 30): die vorangehende Nachricht des Menschen bzw. eines MCP-Clients als Chat
//!   ([`crate::chat_render::repeat_fallback`]); gibt es keine, kein Knopf und REFUSED. Runde 23: Der gespeicherte
//!   Auftrag trägt auch Herkunft (`asker`), Websuche (`web`) und beim Relay die anredende Antwort (`relay`); die
//!   Wiederholung übernimmt sie (ein Relay-Beitrag bleibt ein Relay-Beitrag mit `relay_from`), ältere Beiträge ohne
//!   diese Felder wiederholen wie bisher (Websuche zugelassen, Fragender der Mensch, ohne Rahmen).
//! - `post.since {key, after, rev?}` → `{posts: [{id, author, html}], typing, failures}` ([`since`]): Beiträge nach
//!   `after` als fertiges HTML (Makro `post_entry` aus `fragments/post_item.html`, wie im Abschnitts-Chat), wer gerade
//!   schreibt ([`crate::chat_live::typing_now`]) und Fehler gescheiterter Aufrufe ([`crate::chat_live::failures_now`]).
//! - Formular `POST /beitraege/act` nimmt `post.repeat` (Felder `id`, `rev`, `basis`); „✕ Löschen“ ohne JavaScript
//!   führt auf `/beitrag?work=W&id=N#loeschen` (Bestätigung mit Kästchen).
//! - Die Seiten setzen Beiträge als Markdown-Teilset mit Links ([`crate::chat_render::render_post`]).

use std::sync::Arc;

use std::sync::OnceLock;

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::{IdeaOrigin, PostInfo, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, context};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{self, lq_param};
use crate::{AppState, mirror};

const METHODS: &[&str] = &[
    "post.delete",
    "post.move",
    "post.to_idea",
    "post.edit",
    "post.versions",
    "post.page",
    "post.to_findings",
    "post.repeat",
    "post.since",
];

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Ganzzahl aus `params[name]` (Zahl oder Ziffern als Text aus Formularen); fehlt oder leer → `None`.
fn int(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || invalid(format!("{name}: Zahl erwartet"));
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

fn id_param(params: &Value) -> Result<i64, RpcError> {
    int(params, "id")?.ok_or_else(|| invalid("id fehlt (Beitrag)"))
}

/// Gezeigte Fassung `rev` (ab 1); fehlt → `None`.
fn rev_param(params: &Value) -> Result<Option<u32>, RpcError> {
    match int(params, "rev")? {
        None => Ok(None),
        Some(rev) => u32::try_from(rev).ok().filter(|r| *r >= 1).map(Some).ok_or_else(|| invalid("rev: Fassung ab 1")),
    }
}

/// Ziel des Verschiebens: fester Schlüssel oder (Rückfall) Stelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Key(i64),
    Seq(usize),
}

/// Was mit dem Beitrag geschieht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Op {
    Delete,
    Move(Target),
    ToIdea(Vec<String>),
}

/// Wer handelt: der Mensch (darf alles) oder ein Assistent bzw. MCP-Client (nur eigene Beiträge). Bei MCP
/// ist `origin` die Angabe des Clients, keine geprüfte Identität (docs/MCP.md).
#[derive(Debug, Clone)]
pub(crate) struct Actor {
    pub origin: IdeaOrigin,
    pub only_own: bool,
    /// Runde 23: weitere Autorennamen, deren Beiträge dem Handelnden gehören – der Alt-Handle eines Tokens
    /// (`@mcp:laptop-k-che` neben `@mcp:laptop-kueche`, [`crate::mcp::session::Client::owned_handles`]). Sonst leer.
    pub also: Vec<String>,
}

impl Actor {
    pub(crate) fn human() -> Actor {
        Actor { origin: IdeaOrigin::Human { name: mirror::HUMAN_NAME.to_owned() }, only_own: false, also: Vec::new() }
    }

    /// Ein Assistent bzw. MCP-Client, der nur eigene Beiträge anfassen darf.
    pub(crate) fn own(origin: IdeaOrigin, also: Vec<String>) -> Actor {
        Actor { origin, only_own: true, also }
    }

    /// Gehört ein Beitrag des Autors `author` dem Handelnden? (Name der Herkunft oder einer aus [`Actor::also`].)
    pub(crate) fn owns(&self, author: &str) -> bool {
        author == self.origin.name() || self.also.iter().any(|a| a == author)
    }
}

/// Meldung, wenn ein MCP-Client oder eine Persona einen fremden Beitrag anfassen will.
pub(crate) fn not_own(post: &PostInfo, actor: &IdeaOrigin) -> String {
    format!(
        "Nur eigene Beiträge lassen sich über MCP verschieben, löschen oder umwandeln: Beitrag {} stammt von {}, \
         nicht von {}. Fremde Beiträge verwaltet der Mensch.",
        post.id,
        post.author,
        actor.name()
    )
}

/// Ergebnis des Spiegels in die Antwort: `commit` bei neuem Commit, `mirror` bei Fehler.
fn add_mirror(answer: &mut Value, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Eine Aktion an Beitrag `id` im Werk `work` ausführen (RPC, Formular und MCP): unter der
/// Schreibsperre lesen, Eigentum prüfen (`actor.only_own`), ändern, genau ein Commit.
pub(crate) async fn act(state: &AppState, work: WorkId, id: i64, op: Op, actor: &Actor) -> Result<Value, RpcError> {
    let store = &state.store;
    let held = state.write_lock.lock().await;
    let before = store.post_get(work, id).await?;
    if actor.only_own && !actor.owns(&before.author) {
        return Err(RpcError::new(REFUSED, not_own(&before, &actor.origin)));
    }
    let action = match &op {
        Op::Delete => mirror::PostAction::Delete,
        Op::Move(_) => mirror::PostAction::Move,
        Op::ToIdea(_) => mirror::PostAction::ToIdea,
    };
    let (mut answer, after, idea) = match op {
        Op::Delete => {
            let deleted = store.post_delete(work, id).await?;
            let answer = json!({
                "id": id, "node": deleted.node, "key": deleted.key,
                "deleted": { "author": deleted.author, "body": deleted.body },
            });
            (answer, deleted, None)
        }
        Op::Move(target) => {
            let seq = match target {
                Target::Key(key) => store.node_seq(work, key).await?,
                Target::Seq(seq) => seq,
            };
            let moved = store.post_move(work, id, seq).await?;
            let answer = json!({
                "id": id, "node": moved.node, "key": moved.key, "from_node": before.node, "from_key": before.key,
            });
            (answer, moved, None)
        }
        Op::ToIdea(words) => {
            let (info, idea) = store.post_to_idea(work, id, &words).await?;
            let answer = json!({ "id": id, "idea": idea, "node": info.node, "key": info.key });
            (answer, info, Some(idea.id))
        }
    };
    let result = match store.load_work(work).await {
        Ok(loaded) => {
            let from = (action == mirror::PostAction::Move).then_some(&before);
            let request = mirror::describe_post(&loaded, &after, action, from, idea, &actor.origin);
            mirror::record_post(state, &held, work, request).await
        }
        Err(e) => Err(e.to_string()),
    };
    add_mirror(&mut answer, result);
    Ok(answer)
}

/// Aktion aus RPC-Parametern (`id`, `confirm`, `target_key`/`target`, `words`).
fn op_of(method: &str, params: &Value) -> Result<Op, RpcError> {
    match method {
        "post.delete" => {
            if params.get("confirm") != Some(&Value::Bool(true)) {
                return Err(invalid("Löschen bitte bestätigen (confirm)."));
            }
            Ok(Op::Delete)
        }
        "post.move" => match (int(params, "target_key")?, int(params, "target")?) {
            (Some(key), _) => Ok(Op::Move(Target::Key(key))),
            (None, Some(seq)) => {
                let seq = usize::try_from(seq).map_err(|_| invalid("target: Stelle ab 0 erwartet"))?;
                Ok(Op::Move(Target::Seq(seq)))
            }
            (None, None) => Err(invalid("Ziel fehlt: target_key (fester Schlüssel des Abschnitts) angeben.")),
        },
        "post.to_idea" => {
            let words = match params.get("words") {
                Some(value) if !value.is_null() => crate::ideas::split_words(value, "words")?,
                _ => return Err(invalid("words fehlt (mindestens ein Schlagwort)")),
            };
            Ok(Op::ToIdea(words))
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {other}"))),
    }
}

/// RPC `post.*` (der Mensch).
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    match method {
        "post.edit" => {
            let id = id_param(&params)?;
            let body = match params.get("body") {
                Some(Value::String(body)) => body.clone(),
                _ => return Err(invalid("body fehlt (neuer Text)")),
            };
            return edit(state, work, id, &body, &Actor::human()).await;
        }
        "post.versions" => {
            let id = id_param(&params)?;
            let versions = state.store.post_versions(work, id).await?;
            return Ok(json!({ "id": id, "work": work, "versions": versions }));
        }
        "post.page" => {
            let key = int(&params, "key")?.ok_or_else(|| invalid("key fehlt (fester Schlüssel des Abschnitts)"))?;
            let limit = match int(&params, "limit")? {
                None => CHAT_PAGE_POSTS,
                Some(n) if (1..=200).contains(&n) => n as usize,
                Some(_) => return Err(invalid("limit: 1 bis 200 erwartet")),
            };
            return page(state, work, key, int(&params, "before")?, limit).await;
        }
        "post.to_findings" => return to_findings(state, work, id_param(&params)?).await,
        "post.repeat" => {
            let id = id_param(&params)?;
            let rev = rev_param(&params)?;
            let basis = crate::assist::chat::basis_param(&params)?;
            return repeat(state, work, id, rev, basis).await;
        }
        "post.since" => {
            let key = int(&params, "key")?.ok_or_else(|| invalid("key fehlt (fester Schlüssel des Abschnitts)"))?;
            let after = int(&params, "after")?.unwrap_or(0);
            return since(state, work, key, after, rev_param(&params)?).await;
        }
        _ => {}
    }
    let op = op_of(method, &params)?;
    let id = id_param(&params)?;
    act(state, work, id, op, &Actor::human()).await
}

// ---- Formular ohne JavaScript ------------------------------------------------------------------

/// Formular an `POST /beitraege/act`: alles als Text.
#[derive(Debug, Default, Deserialize)]
struct ActForm {
    method: String,
    work: Option<String>,
    /// Gezeigter Abschnitt (fester Schlüssel): dorthin geht es danach zurück.
    key: Option<String>,
    /// Stelle des gezeigten Abschnitts (nur Rückfall, wenn `key` fehlt).
    node: Option<String>,
    id: Option<String>,
    rev: Option<String>,
    view: Option<String>,
    lq: Option<String>,
    /// Kästchen „wirklich löschen“ (`1` oder `on`); Pflicht bei `post.delete`.
    confirm: Option<String>,
    target_key: Option<String>,
    words: Option<String>,
    /// Runde 28: neuer Text bei `post.edit`.
    body: Option<String>,
    /// Runde 28: `post` = danach zurück auf die Beitragsseite `/beitrag` (statt zum Abschnitt).
    back: Option<String>,
    /// Runde 30c: Prüfgrundlage bei `post.repeat` (`shown` | `current` | `<n>`).
    basis: Option<String>,
}

fn unprocessable(message: impl Into<String>) -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, format!("Fehler: {}", message.into())).into_response()
}

/// `POST /beitraege/act`: `method` = post.delete|post.move|post.to_idea|post.edit|post.to_findings|post.repeat; danach
/// 303 auf den gezeigten Abschnitt `/?work=W&key=K[&rev][&view=preview][&lq]` (beim Wiederholen mit Anker auf den
/// neuen Beitrag), Fehler 422 mit Text (500 bei internen).
async fn act_form(State(state): State<Arc<AppState>>, Form(form): Form<ActForm>) -> Response {
    if !handles(&form.method) {
        return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response();
    }
    let work = match form.work.as_deref().map(str::trim).filter(|w| !w.is_empty()) {
        Some(text) => match text.parse::<WorkId>() {
            Ok(work) => work,
            Err(_) => return unprocessable("work: Werk-id (Zahl) erwartet"),
        },
        None => match state.default_work().await {
            Ok(work) => work,
            Err(e) => return unprocessable(e.to_string()),
        },
    };
    if let Err(e) = state.store.work_slug(work).await {
        return unprocessable(e.to_string());
    }
    let mut params = json!({});
    for (name, value) in
        [("id", &form.id), ("target_key", &form.target_key), ("words", &form.words), ("body", &form.body)]
    {
        if let Some(value) = value {
            params[name] = json!(value);
        }
    }
    if form.method == "post.repeat" {
        for (name, value) in [("rev", &form.rev), ("basis", &form.basis)] {
            if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                params[name] = json!(value);
            }
        }
    }
    if form.method == "post.delete" {
        // Der Server prüft das Kästchen selbst; `required` im HTML allein hält niemanden auf.
        if !matches!(form.confirm.as_deref(), Some("1" | "on")) {
            return unprocessable("Löschen bitte bestätigen (Kästchen ankreuzen).");
        }
        params["confirm"] = json!(true);
    }
    let shown_key: Option<i64> = form.key.as_deref().and_then(|k| k.trim().parse().ok());
    let shown_key = match (shown_key, form.node.as_deref().and_then(|n| n.trim().parse::<usize>().ok())) {
        (Some(key), _) => Some(key),
        (None, Some(node)) => state.store.node_key(work, node).await.ok(),
        (None, None) => None,
    };
    let answer = match call(&state, work, &form.method, params).await {
        Ok(answer) => answer,
        Err(e) if e.code == INTERNAL => {
            return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)).into_response();
        }
        Err(e) => return unprocessable(e.message),
    };
    // Runde 28: zurück auf die Beitragsseite (Bearbeiten, Umwandeln in Befunde von dort).
    if form.back.as_deref() == Some("post")
        && let Some(id) = form.id.as_deref().and_then(|i| i.trim().parse::<i64>().ok())
    {
        return Redirect::to(&post_url(work, id)).into_response();
    }
    // Zurück zum gezeigten Abschnitt; ohne Angabe dorthin, wo der Beitrag stand.
    let key = shown_key.or_else(|| answer["from_key"].as_i64()).or_else(|| answer["key"].as_i64());
    let lq_q = lq_param(form.lq.as_deref());
    let url = match key {
        Some(key) => {
            let rev = form.rev.as_deref().and_then(|r| r.trim().parse::<u32>().ok());
            view::node_url(work, key, rev, form.view.as_deref() == Some("preview"), "", &lq_q)
        }
        None => format!("/?work={work}{lq_q}"),
    };
    // Runde 30c: nach dem Wiederholen an den neuen Beitrag.
    let url = match answer["post"]["id"].as_i64() {
        Some(id) if form.method == "post.repeat" => format!("{url}#post-{id}"),
        _ => url,
    };
    Redirect::to(&url).into_response()
}

/// `POST /beitraege/act` (im Router der schreibenden Routen, also hinter `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/beitraege/act", post(act_form))
}

// ---- Runde 28: Chat-Historie, Bearbeiten, Beitragsseiten (chat-auftrag) -----------------------------------------

/// So viele Beiträge zeigt der Abschnitts-Chat auf einmal (die neuesten); ältere über `post.page` bzw. `/beitraege`.
pub const CHAT_PAGE_POSTS: usize = 50;

/// Welche der `total` Beiträge eines Abschnitts (älteste zuerst) der Chat zeigt: die neuesten [`CHAT_PAGE_POSTS`].
pub fn visible_range(total: usize) -> std::ops::Range<usize> {
    total.saturating_sub(CHAT_PAGE_POSTS)..total
}

/// Bearbeitete Beiträge des Werks: `post.id` → Zeit der letzten Änderung.
pub async fn edited_map(state: &AppState, work: WorkId) -> Result<std::collections::HashMap<i64, String>, RpcError> {
    Ok(state.store.post_edited_map(work).await?)
}

/// Adresse der Beitragsseite.
pub fn post_url(work: WorkId, id: i64) -> String {
    format!("/beitrag?work={work}&id={id}")
}

/// Adresse der ganzen Chat-Historie eines Abschnitts (Anker `#chat-neueste` = der neueste Beitrag).
pub fn history_url(work: WorkId, key: i64) -> String {
    format!("/beitraege?work={work}&key={key}")
}

/// Meldung, wenn jemand einen Beitrag bearbeiten will, der nicht seiner ist (Runde 28).
pub(crate) fn not_editable(id: i64, author: &str, actor: &str) -> String {
    format!(
        "Nur eigene Beiträge lassen sich bearbeiten: Beitrag {id} stammt von {author}, nicht von {actor}. Beiträge von \
         Personas bleiben unverändert (kopieren, zitieren, in Einfall oder Befund umwandeln geht)."
    )
}

/// Höchstens so viele Zeichen des zitierten Texts (die Antwort selbst darf 4000 haben).
const QUOTE_CHARS: usize = 1500;

/// Zitat eines Beitrags für eine Antwort: je Zeile „> “, die erste mit dem Autor, danach eine Leerzeile; lange Texte
/// gekürzt (mit „…“).
pub fn quote_text(author: &str, body: &str) -> String {
    let body = body.trim();
    let cut: String = body.chars().take(QUOTE_CHARS).collect();
    let cut = if cut.chars().count() < body.chars().count() { format!("{} …", cut.trim_end()) } else { cut };
    let mut out = String::new();
    for (i, line) in cut.lines().enumerate() {
        out.push_str("> ");
        if i == 0 {
            out.push_str(author);
            out.push_str(": ");
        }
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out
}

/// Beitrag bearbeiten (RPC `post.edit`, Formular, MCP `post_edit`): nur, wenn `actor` der Autor ist (der Mensch
/// seine, ein MCP-Client seine – seit Runde 23 über [`Actor::owns`], also mit Token auch die unter dem Alt-Handle);
/// unter der Schreibsperre; genau ein Commit mit Text vorher und nachher.
pub(crate) async fn edit(state: &AppState, work: WorkId, id: i64, body: &str, by: &Actor) -> Result<Value, RpcError> {
    crate::assist::check_text(body, "Der Beitrag")?;
    let store = &state.store;
    let held = state.write_lock.lock().await;
    let before = store.post_get(work, id).await?;
    let actor = &by.origin;
    if !by.owns(&before.author) {
        return Err(RpcError::new(REFUSED, not_editable(id, &before.author, actor.name())));
    }
    let (after, _) = store.post_edit(work, id, body, actor.name()).await?;
    let versions = store.post_versions(work, id).await?;
    let mut answer = json!({
        "id": id, "work": work, "node": after.node, "key": after.key, "body": after.body, "edited": after.edited,
        "versions": versions, "url": post_url(work, id),
    });
    let result = match store.load_work(work).await {
        Ok(loaded) => {
            let request = mirror::describe_post(&loaded, &after, mirror::PostAction::Edit, Some(&before), None, actor);
            mirror::record_post(state, &held, work, request).await
        }
        Err(e) => Err(e.to_string()),
    };
    add_mirror(&mut answer, result);
    Ok(answer)
}

/// Beitrag als JSON für `post.page` und die Seiten.
fn post_json(work: WorkId, post: &baruch_core::Post, edited: Option<&String>, style: Option<String>) -> Value {
    json!({
        "id": post.id, "author": post.author, "body": post.body, "created": post.created,
        "time": crate::localtime::minutes(&post.created), "edited": edited,
        "edited_time": edited.map(|t| crate::localtime::minutes(t)), "own": post.author == mirror::HUMAN_NAME,
        "style": style, "sources": post.sources, "url": post_url(work, post.id),
    })
}

/// Farbe je Persona (geprüfte `#rrggbb` als CSS-Variablen, [`crate::personas::style`]).
async fn styles(state: &AppState) -> std::collections::HashMap<String, String> {
    state
        .store
        .persona_list()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.handle, crate::personas::style(&p.color)))
        .collect()
}

/// `post.page`: Beiträge des Abschnitts `key` vor dem Beitrag `before` (ohne: die neuesten), höchstens `limit`.
async fn page(state: &AppState, work: WorkId, key: i64, before: Option<i64>, limit: usize) -> Result<Value, RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let node = loaded.nodes.get(seq).ok_or_else(|| invalid(format!("Abschnitt {key} gibt es nicht")))?;
    let end = match before {
        Some(id) => node
            .posts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| invalid(format!("before: Beitrag {id} steht nicht in diesem Abschnitt")))?,
        None => node.posts.len(),
    };
    let start = end.saturating_sub(limit);
    let edits = edited_map(state, work).await?;
    let colors = styles(state).await;
    let posts: Vec<Value> = node.posts[start..end]
        .iter()
        .map(|p| post_json(work, p, edits.get(&p.id), colors.get(&p.author).cloned()))
        .collect();
    Ok(json!({ "work": work, "key": key, "posts": posts, "older": start, "history_url": history_url(work, key) }))
}

/// `post.to_findings`: Befunde aus dem Beitrag (manuell, auch ohne Marken), der Beitrag bleibt.
async fn to_findings(state: &AppState, work: WorkId, id: i64) -> Result<Value, RpcError> {
    let _held = state.write_lock.lock().await;
    let info = state.store.post_get(work, id).await?;
    let source = crate::findings::IngestSource {
        work,
        origin: crate::findings::IngestOrigin::Post { id, key: info.key },
        persona: &info.author,
        text: &info.body,
        summary: false,
        manual: true,
    };
    let report = crate::findings::ingest(state, &source).await.map_err(|e| RpcError::new(REFUSED, e))?;
    let findings: Vec<Value> = report.created.iter().map(|f| crate::findings::finding_json(work, f)).collect();
    let attached: Vec<Value> = report.attached.iter().map(|f| crate::findings::finding_json(work, f)).collect();
    Ok(json!({ "id": id, "work": work, "key": info.key, "findings": findings, "attached": attached }))
}

// ---- Runde 30c: Wiederholen und Live-Abfrage (chat-bedienung) ----------------------------------------------------

/// Auftrag für den Rückfall ohne gespeicherten Auftrag: die vorangehende Nachricht des Menschen bzw. eines
/// MCP-Clients, als Chat an dieselbe Persona.
async fn fallback_request(
    state: &AppState,
    work: WorkId,
    info: &PostInfo,
) -> Result<baruch_store::PostRequest, RpcError> {
    let loaded = state.store.load_work(work).await?;
    let previous = loaded.nodes.iter().find(|n| n.key == info.key).and_then(|node| {
        let index = node.posts.iter().position(|p| p.id == info.id)?;
        crate::chat_render::repeat_fallback(&node.posts, index).map(|p| p.body.clone())
    });
    match previous {
        Some(text) => Ok(baruch_store::PostRequest {
            persona: info.author.clone(),
            mode: "chat".into(),
            scope: "section".into(),
            text,
            rev: None,
            ..Default::default()
        }),
        None => Err(RpcError::new(
            REFUSED,
            format!(
                "Beitrag {} hat keinen gespeicherten Auftrag und keine vorangehende Nachricht des Menschen; wiederholen \
                 lässt er sich nicht. Bitte die Frage neu stellen.",
                info.id
            ),
        )),
    }
}

/// Offene Befunde `old` (der alten Antwort), die in `text` (der neuen Antwort) nicht wiederkommen, schlägt die
/// Persona `handle` als „erledigt“ vor (je ein Commit). Lob bleibt außen vor. Ergebnis: ids der Vorschläge und
/// Hinweise zu gescheiterten.
pub(crate) async fn propose_done(
    state: &AppState,
    work: WorkId,
    old: &[baruch_store::Finding],
    handle: &str,
    new_label: &str,
    text: &str,
) -> (Vec<i64>, Vec<String>) {
    let fresh = crate::findings::parse::split_answer(text);
    let by = IdeaOrigin::Persona { handle: handle.to_owned() };
    let mut proposed = Vec::new();
    let mut notes = Vec::new();
    for finding in old {
        if finding.status != baruch_store::FindingStatus::Open || finding.kind == baruch_store::FindingKind::Praise {
            continue;
        }
        // Runde 22: Hat die Persona in derselben Antwort schon „Erledigt: Befund n“ gemeldet (offene Rückfrage
        // „erledigt“), nicht noch einmal vorschlagen – sonst stünde ein Vermerk „schon offen“ in den notes. Ein offener
        // Vorschlag anderer Art (z. B. „übernommen“) hält den Vorschlag „erledigt“ nicht auf (Gutachten R22).
        if finding.pending.as_ref().is_some_and(|p| p.status == baruch_store::FindingStatus::Done) {
            continue;
        }
        if fresh.iter().any(|n| crate::findings::parse::similar(&finding.body, &n.body)) {
            continue;
        }
        let change = baruch_store::StatusChange::Done {
            reason: format!("kommt in der Wiederholung ({new_label}) nicht mehr vor"),
        };
        let note = format!("Wiederholung: {new_label} nennt den Befund nicht mehr – erledigt?");
        match crate::findings::rpc::propose(state, work, finding.id, &change, &by, &note).await {
            Ok(_) => proposed.push(finding.id),
            Err(e) => notes.push(format!("[B{}]: {}", finding.id, e.message)),
        }
    }
    (proposed, notes)
}

/// `post.repeat`: dieselbe Anfrage gegen die gezeigte bzw. gewählte Fassung (siehe Moduldoku).
pub(crate) async fn repeat(
    state: &AppState,
    work: WorkId,
    id: i64,
    rev: Option<u32>,
    basis: crate::assist::basis::BasisChoice,
) -> Result<Value, RpcError> {
    use crate::assist::{Mode, Order, OrderFrame, PostLinks, Scope};
    let info = state.store.post_get(work, id).await?;
    if !crate::chat_render::is_persona(&info.author) {
        return Err(RpcError::new(
            REFUSED,
            format!("Nur Antworten von Personas lassen sich wiederholen; Beitrag {id} stammt von {}.", info.author),
        ));
    }
    let meta = state.store.post_meta(work, &[id]).await?.remove(&id).unwrap_or_default();
    let request = match meta.request.clone() {
        Some(request) => request,
        None => fallback_request(state, work, &info).await?,
    };
    let mode = Mode::parse(&request.mode).filter(|m| *m != Mode::Propose).ok_or_else(|| {
        RpcError::new(REFUSED, format!("Der Auftrag „{}“ lässt sich nicht wiederholen.", request.mode))
    })?;
    let scope = if request.scope == "chapter" { Scope::Chapter } else { Scope::Section };
    let chosen = crate::assist::basis::choose(state, work, info.key, rev, basis).await?;
    // Runde 23: Herkunft, Websuche und Relay-Rahmen aus dem gespeicherten Auftrag. Beiträge aus der Zeit davor tragen
    // die Felder nicht und wiederholen wie bisher (Websuche zugelassen, Fragender der Mensch, kein Rahmen).
    let relay = request.relay.as_deref().and_then(crate::assist::FramedAnswer::from_request);
    let relay_from = relay.as_ref().and(meta.relay_from.clone());
    let order = Order {
        work,
        key: info.key,
        persona: request.persona.clone(),
        mode,
        text: request.text.clone(),
        rev: Some(chosen.rev),
        asker: request.asker.clone(),
        post_question: false,
        sources: vec![],
        web: request.web.unwrap_or(true),
        scope,
        frame: OrderFrame { relay, ..OrderFrame::default() },
        links: PostLinks { reply_to: meta.reply_to, repeat_of: Some(id), relay_from },
        basis: Some(chosen.clone()),
    };
    let outcome = match crate::assist::run(state, &order, None).await {
        Ok(outcome) => outcome,
        Err(e) => {
            crate::chat_live::record_failure(work, info.key, &request.persona, &e.message);
            return Err(e);
        }
    };
    let Some(post) = outcome.post.clone() else {
        return Err(RpcError::new(INTERNAL, "Die Wiederholung hat keinen Beitrag ergeben."));
    };
    let filter = baruch_store::FindingFilter {
        post: Some(id),
        statuses: vec![baruch_store::FindingStatus::Open],
        limit: Some(500),
        ..Default::default()
    };
    let old = state.store.finding_list(work, &filter).await.unwrap_or_default();
    let label = format!("Beitrag {}", post.id);
    let (proposed_done, notes) = propose_done(state, work, &old, &outcome.persona, &label, &outcome.text).await;
    let url = format!("{}#post-{}", view::node_url(work, info.key, rev, false, "", ""), post.id);
    Ok(json!({
        "work": work, "key": info.key, "post": post, "url": url, "repeat_of": id, "basis": chosen,
        "proposed_done": proposed_done, "notes": notes,
    }))
}

/// Ziele für „Verschieben…“ (alle Abschnitte außer `key`, „Nummer Titel“).
fn move_targets(work: &baruch_core::Work, key: i64) -> Vec<Value> {
    work.nodes
        .iter()
        .filter(|n| n.key != key)
        .map(|n| json!({ "key": n.key, "label": format!("{} {}", n.number, n.title).trim() }))
        .collect()
}

/// Beitrag des Abschnitts-Chats für das Makro `post_entry` – dieselben Felder wie `view::posts`.
#[allow(clippy::too_many_arguments)]
fn chat_post_value(
    work_id: WorkId,
    work: &baruch_core::Work,
    node: &baruch_core::Node,
    post: &baruch_core::Post,
    shown_rev: u32,
    extras: &view::ViewExtras,
    colors: &std::collections::HashMap<String, String>,
) -> minijinja::Value {
    let shown_body = node.revision(shown_rev).map_or("", |r| r.body.as_str());
    let refs: Vec<minijinja::Value> = post
        .refs
        .iter()
        .filter_map(|r| work.nodes.get(r.node).map(|target| (r, target)))
        .map(|(r, target)| {
            context! {
                number => target.number.as_str(), title => target.title.as_str(), rev => r.rev, quote => r.quote.as_str(),
                url => view::node_url(work_id, target.key, Some(r.rev), false, "", ""),
            }
        })
        .collect();
    let ctx = crate::chat_render::PostCtx { work_id, work, key: node.key, shown_rev, extras };
    let edited = extras.post_edits.get(&post.id);
    context! {
        id => post.id, created => post.created.as_str(), time => crate::localtime::minutes(&post.created),
        author => post.author.as_str(), body => post.body.as_str(), style => colors.get(&post.author).cloned(),
        sources => minijinja::Value::from(Serde(&post.sources)), refs => refs,
        edited => edited.map(String::as_str), edited_time => edited.map(|t| crate::localtime::minutes(t)),
        own => post.author == mirror::HUMAN_NAME,
        extra => minijinja::Value::from(Serde(crate::chat_render::post_extra(&ctx, post))),
        solutions => minijinja::Value::from(Serde(crate::assist::solutions::post_solutions(&post.body, shown_body))),
    }
}

/// Höchstens so viele Beiträge liefert `post.since` auf einmal (danach lädt der Browser neu).
pub const SINCE_MAX_POSTS: usize = 50;

/// `post.since`: Beiträge des Abschnitts nach `after` als HTML, wer schreibt, Fehler.
pub(crate) async fn since(
    state: &AppState,
    work: WorkId,
    key: i64,
    after: i64,
    rev: Option<u32>,
) -> Result<Value, RpcError> {
    let loaded = state.store.load_work(work).await?;
    let node =
        loaded.nodes.iter().find(|n| n.key == key).ok_or_else(|| invalid(format!("Abschnitt {key} gibt es nicht")))?;
    // Nach dem Beitrag `after`; steht er nicht (mehr) im Abschnitt, alle mit größerer id.
    let start = match node.posts.iter().position(|p| p.id == after) {
        Some(i) => i + 1,
        None => node.posts.iter().position(|p| p.id > after).unwrap_or(node.posts.len()),
    };
    let fresh = &node.posts[start..];
    let truncated = fresh.len() > SINCE_MAX_POSTS;
    let mut posts: Vec<Value> = Vec::new();
    if !fresh.is_empty() {
        let extras = view::ViewExtras {
            findings: crate::findings::view::load(state, work).await.unwrap_or_default(),
            post_edits: edited_map(state, work).await.unwrap_or_default(),
            post_meta: state.store.work_post_meta(work).await.unwrap_or_default(),
            ..Default::default()
        };
        let colors = styles(state).await;
        let shown_rev = rev.or_else(|| node.current_number()).unwrap_or(0);
        let at = context! {
            work => work, key => key, node => node.id, rev => shown_rev, view => "", lq => "",
            targets => minijinja::Value::from(Serde(move_targets(&loaded, key))),
        };
        for post in fresh.iter().take(SINCE_MAX_POSTS) {
            let value = chat_post_value(work, &loaded, node, post, shown_rev, &extras, &colors);
            let html = templates()
                .render_str(POST_ENTRY_SRC, context! { post => value, at => at.clone() })
                .map_err(|e| RpcError::new(INTERNAL, format!("Vorlage post_item.html: {e}")))?;
            posts.push(json!({ "id": post.id, "author": post.author, "html": html }));
        }
    }
    Ok(json!({
        "work": work, "key": key, "posts": posts, "truncated": truncated,
        "typing": crate::chat_live::typing_now(work, key), "failures": crate::chat_live::failures_now(work, key),
    }))
}

/// Ein Beitrag des Abschnitts-Chats über das Makro `post_entry` (für `post.since`).
const POST_ENTRY_SRC: &str = "{% from \"fragments/post_item.html\" import post_entry %}{{ post_entry(post, at) }}";

// ---- Seiten /beitraege und /beitrag (ohne JavaScript) -------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("posts.html", include_str!("../templates/posts.html")),
            ("post.html", include_str!("../templates/post.html")),
            // Runde 30 (Schnittstelle R30-S1): Makros für Beiträge, Befund-Blöcke und Lösungen.
            ("fragments/post_item.html", include_str!("../templates/fragments/post_item.html")),
            ("fragments/finding_block.html", include_str!("../templates/fragments/finding_block.html")),
            ("fragments/post_solutions.html", include_str!("../templates/fragments/post_solutions.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Beitragsseiten fehlerhaft");
        }
        env
    })
}

fn render(name: &str, ctx: minijinja::Value) -> Response {
    match templates().get_template(name).and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage {name}: {e}")).into_response(),
    }
}

fn settings_of(headers: &HeaderMap) -> minijinja::Value {
    minijinja::Value::from(Serde(view::Settings::from_cookie_header(
        headers.get(header::COOKIE).and_then(|v| v.to_str().ok()),
    )))
}

/// Selbst gebaute Adresse (Ziffern und feste Namen) als sicherer Wert für `href`.
fn href(url: String) -> minijinja::Value {
    minijinja::Value::from_safe_string(url.replace('&', "&amp;"))
}

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    work: Option<String>,
    key: Option<String>,
    id: Option<String>,
}

fn number<T: std::str::FromStr>(value: &Option<String>) -> Option<T> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty()).and_then(|v| v.parse().ok())
}

async fn work_of(state: &AppState, work: Option<WorkId>) -> Result<(WorkId, baruch_core::Work), RpcError> {
    let work = match work {
        Some(work) => work,
        None => state.default_work().await?,
    };
    Ok((work, state.store.load_work(work).await?))
}

/// Beitrag für die Vorlagen: wie [`post_json`], Adressen als sichere Werte; seit Runde 30c mit `extra`
/// ([`crate::chat_render::render_post`]: Markdown, Links, Kopf, Befund-Blöcke) und `delete_url`.
fn post_value(
    work: WorkId,
    key: i64,
    post: &baruch_core::Post,
    edited: Option<&String>,
    style: Option<String>,
    extra: crate::chat_render::PostExtra,
) -> minijinja::Value {
    context! {
        id => post.id, author => post.author.as_str(), body => post.body.as_str(),
        created => post.created.as_str(), time => crate::localtime::minutes(&post.created),
        edited => edited.map(|t| crate::localtime::minutes(t)), style => style,
        own => post.author == mirror::HUMAN_NAME,
        persona => post.author.starts_with('@') && !post.author.starts_with("@mcp:"),
        sources => minijinja::Value::from(Serde(&post.sources)),
        url => href(post_url(work, post.id)),
        delete_url => href(format!("{}#loeschen", post_url(work, post.id))),
        anchor_url => href(format!("{}#post-{}", history_url(work, key), post.id)),
        anchor => "post-",
        extra => minijinja::Value::from(Serde(extra)),
    }
}

/// Befunde und Metadaten des Werks für die Seiten (ohne die übrigen Zusätze der Arbeitsseite).
async fn page_extras(state: &AppState, work: WorkId) -> view::ViewExtras {
    view::ViewExtras {
        findings: crate::findings::view::load(state, work).await.unwrap_or_default(),
        post_meta: state.store.work_post_meta(work).await.unwrap_or_default(),
        ..Default::default()
    }
}

/// Zusätze eines Beitrags auf den Seiten (gegen die aktuelle Fassung des Abschnitts).
fn page_extra(
    work_id: WorkId,
    work: &baruch_core::Work,
    node: Option<&baruch_core::Node>,
    post: &baruch_core::Post,
    extras: &view::ViewExtras,
) -> crate::chat_render::PostExtra {
    match node {
        Some(node) => {
            let ctx = crate::chat_render::PostCtx {
                work_id,
                work,
                key: node.key,
                shown_rev: node.current_number().unwrap_or(0),
                extras,
            };
            crate::chat_render::post_extra(&ctx, post)
        }
        None => crate::chat_render::PostExtra {
            html: crate::chat_render::post_html(work_id, work, &post.body, None),
            plain: crate::chat_render::plain_text(Some(work), &post.body),
            ..Default::default()
        },
    }
}

/// `GET /beitraege?work&key`: alle Beiträge des Abschnitts, älteste zuerst, Anker `#post-<id>` und `#chat-neueste`.
async fn history(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<PageQuery>) -> Response {
    let (work, loaded) = match work_of(&state, number(&q.work)).await {
        Ok(found) => found,
        Err(e) => return (StatusCode::NOT_FOUND, e.message).into_response(),
    };
    let Some(key) = number::<i64>(&q.key) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: key fehlt (fester Schlüssel des Abschnitts)")
            .into_response();
    };
    let Some(node) = loaded.nodes.iter().find(|n| n.key == key) else {
        return (StatusCode::NOT_FOUND, format!("Abschnitt {key} gibt es in diesem Werk nicht.")).into_response();
    };
    let edits = edited_map(&state, work).await.unwrap_or_default();
    let colors = styles(&state).await;
    let extras = page_extras(&state, work).await;
    let posts: Vec<minijinja::Value> = node
        .posts
        .iter()
        .map(|p| {
            let extra = page_extra(work, &loaded, Some(node), p, &extras);
            post_value(work, key, p, edits.get(&p.id), colors.get(&p.author).cloned(), extra)
        })
        .collect();
    let personas: Vec<minijinja::Value> = state
        .store
        .persona_list()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.active)
        .map(|p| context! { handle => p.handle, name => p.name })
        .collect();
    let title = if node.number.is_empty() { node.title.clone() } else { format!("{} {}", node.number, node.title) };
    let ctx = context! {
        settings => settings_of(&headers), work => context! { id => work, title => loaded.title.as_str() },
        node => context! { key => key, title => title, rev => node.current_number() }, posts => posts,
        personas => personas, node_url => href(view::node_url(work, key, None, false, "", "")),
    };
    render("posts.html", ctx)
}

/// `GET /beitrag?work&id`: ein Beitrag zum Kopieren (markierbares, schreibgeschütztes Feld), Zitieren (eigenes
/// Chat-Formular mit „> …“), Bearbeiten (nur eigene), Verlauf und „In Befund umwandeln“.
async fn single(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<PageQuery>) -> Response {
    let (work, loaded) = match work_of(&state, number(&q.work)).await {
        Ok(found) => found,
        Err(e) => return (StatusCode::NOT_FOUND, e.message).into_response(),
    };
    let Some(id) = number::<i64>(&q.id) else {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Fehler: id fehlt (Beitrag)").into_response();
    };
    let info = match state.store.post_get(work, id).await {
        Ok(info) => info,
        Err(e) => return (StatusCode::NOT_FOUND, RpcError::from(e).message).into_response(),
    };
    let versions: Vec<minijinja::Value> = state
        .store
        .post_versions(work, id)
        .await
        .unwrap_or_default()
        .iter()
        .map(|v| {
            context! {
                body => v.body.as_str(), valid_from => crate::localtime::minutes(&v.valid_from),
                replaced => crate::localtime::minutes(&v.replaced), edited_by => v.edited_by.as_str(),
            }
        })
        .collect();
    let colors = styles(&state).await;
    let node = loaded.nodes.iter().find(|n| n.key == info.key);
    let title = node.map_or(String::new(), |n| {
        if n.number.is_empty() { n.title.clone() } else { format!("{} {}", n.number, n.title) }
    });
    let core = baruch_core::Post {
        id,
        created: info.created.clone(),
        author: info.author.clone(),
        body: info.body.clone(),
        sources: info.sources.clone(),
        refs: vec![],
    };
    let extras = page_extras(&state, work).await;
    let extra = page_extra(work, &loaded, node, &core, &extras);
    let post = post_value(work, info.key, &core, info.edited.as_ref(), colors.get(&info.author).cloned(), extra);
    let ctx = context! {
        settings => settings_of(&headers), work => context! { id => work, title => loaded.title.as_str() },
        node => context! { key => info.key, id => info.node, title => title },
        post => post, versions => versions, quote => quote_text(&info.author, &info.body),
        node_url => href(view::node_url(work, info.key, None, false, "", "")),
        history_url => href(format!("{}#post-{id}", history_url(work, info.key))),
    };
    render("post.html", ctx)
}

/// Seiten `GET /beitraege?work&key` (ganze Chat-Historie) und `GET /beitrag?work&id` (ein Beitrag).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/beitraege", get(history)).route("/beitrag", get(single))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_post_management_methods_are_handled_here() {
        assert!(handles("post.delete") && handles("post.move") && handles("post.to_idea"), "post.* gehört hierher");
        for method in ["post.edit", "post.versions", "post.page", "post.to_findings"] {
            assert!(handles(method), "{method}: Runde 28 (chat-auftrag) gehört hierher");
        }
        for method in ["post.repeat", "post.since"] {
            assert!(handles(method), "{method}: Runde 30c (chat-bedienung) gehört hierher");
        }
        assert!(!handles("post.add") && !handles("post.purge"), "post.add bleibt in rpc.rs, anderes gibt es nicht");
    }

    #[test]
    fn parameters_come_as_numbers_or_as_form_text() {
        assert_eq!(op_of("post.move", &json!({ "target_key": "12" })).unwrap(), Op::Move(Target::Key(12)), "Text");
        assert_eq!(op_of("post.move", &json!({ "target": 3 })).unwrap(), Op::Move(Target::Seq(3)), "Stelle");
        assert_eq!(
            op_of("post.move", &json!({ "target_key": 7, "target": 3 })).unwrap(),
            Op::Move(Target::Key(7)),
            "der Schlüssel geht vor"
        );
        assert!(op_of("post.move", &json!({})).is_err(), "ohne Ziel abgelehnt");
        assert!(op_of("post.delete", &json!({ "confirm": "1" })).is_err(), "über RPC zählt nur confirm: true");
        assert_eq!(op_of("post.delete", &json!({ "confirm": true })).unwrap(), Op::Delete, "bestätigt");
        assert_eq!(
            op_of("post.to_idea", &json!({ "words": "Rose, Dorn" })).unwrap(),
            Op::ToIdea(vec!["Rose".into(), " Dorn".into()]),
            "Schlagwörter als Text (getrimmt wird im Store)"
        );
        assert!(op_of("post.to_idea", &json!({})).is_err(), "ohne Schlagwörter abgelehnt");
        assert_eq!(id_param(&json!({ "id": " 42 " })).unwrap(), 42, "id als Text aus dem Formular");
    }
}
