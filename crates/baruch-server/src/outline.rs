// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederungsmanager im Server (Vertrag Runde 4, Abschnitte 3e, 4, 5 und 6): RPC `outline.*`, die
//! Seiten `/gliederung` und `/gliederung/bearbeiten` und Formulare an `POST /gliederung/act`.
//!
//! RPC ([`handles`], [`call`]; aus `rpc::call` heraus, alle nehmen `work?`):
//! - `outline.move {node, target, place}` – `place` = `before`, `after`, `into`
//! - `outline.set_level {node, level}` – Ebene 1–3, Unterpunkte wandern mit
//! - `outline.rename {node, title}`
//! - `outline.add {target, place, title, level?}` – `level` absolut 0–3 (fehlt: Ebene des Orts); Ebene 0
//!   legt einen Teil vor/hinter einem Teil oder Kapitel an (auch mit Werkvorlage, Runde 14); Antwort
//!   zusätzlich `adopted: [key]` (vorhandene Kapitel, die jetzt unter dem neuen Teil stehen)
//! - `outline.delete {node, confirm: true}` – ohne `confirm` → -32602
//! - `outline.check {node, body}` → `HeadingCheck` (nur lesen)
//! - `outline.save {node, body, based_on?, based_on_amend?, split?, merge?, mode?}` → `{node, rev, saved, amend_seq?, reason?,
//!   reason_label?, outline?, commit?, mirror?}` – Runde 22: `mode` „auto“ schreibt die eigene aktuelle Fassung fort,
//!   wenn die Regeln es erlauben (`saved: "amended"` mit `amend_seq`), sonst neue Fassung mit `reason`/`reason_label`;
//!   fehlt `mode`, gilt „new“ (wie vor Runde 22). Das Formular schickt `mode=auto` (verstecktes Feld) bzw. mit dem
//!   Knopf „Als neue Fassung speichern“ `as_new=1` (→ `mode` „new“). `based_on_amend` ist der Stand der Basis, den
//!   der Editor geladen hatte: Ist die aktuelle Fassung seither fortgeschrieben, entsteht eine neue Fassung
//!   (`reason` „not_on_current“) statt einer stillen Fortschreibung über den zwischenzeitlichen Text.
//! - `outline.set_numbering {numbering: string|null}`
//! - `outline.set_numbered {node, numbered}` – Punkt mit bzw. ohne Nummer zählen (z. B. Vorwort)
//! - `outline.requests {open?, preview?}` → `{requests}` – Anträge der Assistenten (Migration 0012, nur
//!   lesen); mit `preview: true` je offenem Antrag `preview: [PreviewPoint]` oder `preview_error` (Runde 14)
//! - `outline.request_decide {id, decision: "confirm"|"reject"}` → `{request, outline?, commit?, mirror?}`
//!
//! Statt `node` bzw. `target` (Stelle, jede Änderung vergibt sie neu) nehmen alle `key` bzw.
//! `target_key` (fester Schlüssel `Node::key`); der Schlüssel geht vor ([`rpc::resolve_keys`]).
//!
//! Antwort der Gliederungsmethoden: `{node, action, title, detail, revisions: [{node, rev}],
//! renumbered: [[alt, neu], …], commit?, mirror?}`. Wahrheitswerte nehmen `true` oder `"1"`/`"on"`
//! (Formulare), Zahlen auch als Text. Alles Schreibende läuft unter [`AppState::write_lock`], danach
//! genau ein Commit im Spiegel ([`mirror::record_outline`]). MCP und Assistenten erreichen nichts
//! davon: Sie legen Punkte über `outline_add` an und stellen für alles andere Anträge
//! (`crate::mcp::outline`), über die hier der Mensch entscheidet (Wunsch 2026-11-06). Ein bestätigter
//! Antrag ist genau ein Commit mit dem Menschen als Bestätiger und dem Antragsteller als Herkunft
//! ([`mirror::OutlineBy::Confirmed`]); Ablehnen und ein veralteter Antrag ändern die Gliederung nicht,
//! stehen aber für jede Antragsart als leerer Protokoll-Commit im Spiegel (Nutzerentscheidung
//! 2026-11-08, [`mirror::record_request_decision`]).
//!
//! Runde 16c: Die Vorschau eines Antrags nennt je Punkt Art und Nummer (`label`, „Teil 1“, „Kapitel 2“,
//! bei Predigten „Predigtteil I.“ in der Form des Schemas, seit Runde 17b) und für vorhandene Punkte den Stand davor (`before_label`); die Seite
//! `/gliederung` zeigt bei Predigten oder gesetzter Zielzeit die Spalte „Dauer“ mit Zeitmarke und oben
//! Gesamt- und Zielzeit ([`crate::timing::work_timing`]).

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_core::Author;
use baruch_core::Work;
use baruch_store::{NewPoint, OutlineChange, Place, Restructure, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::rpc::{self, INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{self, Settings, lq_param};
use crate::{AppState, mirror};

const METHODS: &[&str] = &[
    "outline.move",
    "outline.set_level",
    "outline.rename",
    "outline.add",
    "outline.delete",
    "outline.check",
    "outline.save",
    "outline.set_numbering",
    "outline.set_numbered",
    "outline.requests",
    "outline.request_decide",
];

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

// ---- Parameter ---------------------------------------------------------------------------------

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Zahl ab 0 aus `params[key]`, auch als Text (Formulare); fehlt, `null` oder leer → `None`.
fn number(params: &Json, key: &str) -> Result<Option<u64>, RpcError> {
    let wrong = || invalid(format!("{key}: Zahl ab 0 erwartet"));
    match params.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Number(n)) => n.as_u64().map(Some).ok_or_else(wrong),
        Some(Json::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Json::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

fn required(params: &Json, key: &str) -> Result<usize, RpcError> {
    let value = number(params, key)?.ok_or_else(|| invalid(format!("{key} fehlt")))?;
    usize::try_from(value).map_err(|_| invalid(format!("{key}: zu groß")))
}

/// Wahrheitswert: `true`, `1`, `"1"`, `"on"` oder `"true"`; alles andere ist falsch.
fn flag(params: &Json, key: &str) -> bool {
    match params.get(key) {
        Some(Json::Bool(b)) => *b,
        Some(Json::Number(n)) => n.as_u64() == Some(1),
        Some(Json::String(s)) => matches!(s.trim(), "1" | "on" | "true"),
        _ => false,
    }
}

fn text(params: &Json, key: &str) -> Result<String, RpcError> {
    match params.get(key) {
        Some(Json::String(s)) => Ok(s.clone()),
        _ => Err(invalid(format!("{key} fehlt (Text)"))),
    }
}

fn place(params: &Json) -> Result<Place, RpcError> {
    match params.get("place").and_then(Json::as_str).map(str::trim) {
        Some("before") => Ok(Place::Before),
        Some("after") => Ok(Place::After),
        Some("into") => Ok(Place::Into),
        _ => Err(invalid("place: before, after oder into erwartet")),
    }
}

// ---- RPC ---------------------------------------------------------------------------------------

/// Antwort einer Gliederungsänderung (ohne Spiegel); auch `outline` in der Antwort von
/// `revision.set_current`/`revision.accept`.
pub(crate) fn outline_answer(change: &OutlineChange) -> Json {
    json!({
        "node": change.node,
        "action": change.action,
        "title": change.title,
        "detail": change.detail,
        "revisions": change.revisions.iter().map(|(node, rev)| json!({ "node": node, "rev": rev })).collect::<Vec<_>>(),
        "renumbered": change.renumbered,
    })
}

fn add_mirror(answer: &mut Json, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Festen Schlüssel des Abschnitts `node` der Antwort ergänzen (`key`, Runde 8): Danach zu zeigende
/// Adressen tragen ihn (`/?work=W&key=K`). Gibt es den Punkt nicht, `null`.
async fn add_key(state: &AppState, work: WorkId, answer: &mut Json) {
    let key = match answer["node"].as_u64().and_then(|n| usize::try_from(n).ok()) {
        Some(node) => state.store.node_key(work, node).await.ok(),
        None => None,
    };
    answer["key"] = json!(key);
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params: Json) -> Result<Json, RpcError> {
    let store = &state.store;
    let human = mirror::HUMAN_NAME;
    // Schreibende Methoden sperren, bevor `key`/`target_key` in Stellen übersetzt werden: Bis zum
    // Schreiben ändert dann niemand die Gliederung (siehe `rpc::resolve_keys`).
    let held = match method {
        "outline.check" | "outline.requests" => None,
        _ => Some(state.write_lock.lock().await),
    };
    let params = rpc::resolve_keys(state, work, params).await?;
    match method {
        "outline.requests" => {
            // Vorgabe: nur offene; `open: false` alle (auch entschiedene, mit Entscheidung und Meldung).
            let open = match params.get("open") {
                None | Some(Json::Null) => true,
                _ => flag(&params, "open"),
            };
            let requests = store.outline_requests(work, open).await?;
            let mut list: Vec<Json> = requests.iter().map(request_json).collect();
            if flag(&params, "preview") {
                for (value, request) in list.iter_mut().zip(&requests) {
                    add_preview(state, work, request, value).await?;
                }
            }
            Ok(json!({ "work": work, "requests": list }))
        }
        "outline.request_decide" => {
            let id = required(&params, "id")?;
            let id = i64::try_from(id).map_err(|_| invalid("id: zu groß"))?;
            let confirm = match params.get("decision").and_then(Json::as_str).map(str::trim) {
                Some("confirm") => true,
                Some("reject") => false,
                _ => return Err(invalid("decision: confirm oder reject erwartet")),
            };
            let held = held.ok_or_else(|| RpcError::new(INTERNAL, "Schreibsperre fehlt"))?;
            // Runde 36 (store-5): Bestätigt der Mensch einen Antrag „Löschen“, wird vorher gelesen, was außer der
            // aktuellen Fassung mitgelöscht wird (übrige Fassungen, Beiträge) – für den Spiegel-Commit.
            let mut losses = baruch_store::OutlineLosses::default();
            if confirm {
                let open = store.outline_requests(work, true).await?;
                if let Some(baruch_store::OutlineOp::Delete { key }) = open.iter().find(|r| r.id == id).map(|r| &r.op)
                    && let Ok(seq) = store.node_seq(work, *key).await
                {
                    losses = store.outline_delete_losses(work, seq).await?;
                }
            }
            let decided = store.outline_request_decide(work, id, confirm, human).await?;
            let mut answer = json!({ "request": request_json(&decided.request) });
            if let Some(change) = &decided.change {
                let mut outline = outline_answer(change);
                add_key(state, work, &mut outline).await;
                answer["node"] = outline["node"].clone();
                answer["key"] = outline["key"].clone();
                answer["outline"] = outline;
                let by = mirror::OutlineBy::Confirmed(&decided.request);
                // Nur ein wirklich ausgeführtes Löschen nennt Verluste (ein veralteter Antrag ändert nichts).
                let lost =
                    if change.action == baruch_store::OutlineAction::Deleted { losses } else { Default::default() };
                add_mirror(&mut answer, mirror::record_outline_losses(state, &held, change, by, &lost).await);
            } else {
                // Nutzerentscheidung 2026-11-08 (Runde 16c): Jede Entscheidung ohne Gliederungsänderung –
                // „abgelehnt“ oder „veraltet“ – steht für jede Antragsart als leerer Commit mit Herkunft im
                // Spiegel (vorher nur beim Antrag `add`, Nutzerfrage 12).
                add_mirror(&mut answer, mirror::record_request_decision(state, &held, &decided.request).await);
            }
            Ok(answer)
        }
        "outline.check" => {
            let node = required(&params, "node")?;
            // Markdown-Überschriften des Menschen (`# Titel`) zählen wie `=`-Zeilen (Nutzer 2026-11-07).
            let body = baruch_typst::markdown_headings(&text(&params, "body")?).into_owned();
            Ok(json!(store.outline_check(work, node, &body).await?))
        }
        "outline.save" => {
            let node = required(&params, "node")?;
            // Eingabe des Menschen: Markdown-Überschriften werden Typst-Überschriften, auch ohne JavaScript
            // (maßgebliche Regel `baruch_typst::markdown`); danach gleicht `outline_save` die Gliederung ab.
            let body = baruch_typst::markdown_headings(&text(&params, "body")?).into_owned();
            let based_on = number(&params, "based_on")?
                .map(|b| u32::try_from(b).map_err(|_| invalid("based_on: zu groß")))
                .transpose()?;
            // Nachbesserung R22: Stand der Basis, den der Editor geladen hatte (fehlt: nur die Fassungsnummer zählt).
            let based_on_amend = number(&params, "based_on_amend")?
                .map(|b| u32::try_from(b).map_err(|_| invalid("based_on_amend: zu groß")))
                .transpose()?;
            let allow = Restructure { split: flag(&params, "split"), merge: flag(&params, "merge") };
            let mode = rpc::save_mode(&params)?;
            let held = held.ok_or_else(|| RpcError::new(INTERNAL, "Schreibsperre fehlt"))?;
            let saved = store.outline_save_at(work, node, &body, based_on, based_on_amend, allow, human, mode).await?;
            let shown = saved.outline.as_ref().map_or(node, |o| o.node);
            let rev = store.load_work(work).await?.nodes.get(shown).and_then(|n| n.current_number());
            let mut answer = json!({ "node": shown, "rev": rev });
            rpc::add_saved(&mut answer, saved.change.as_ref(), saved.new_reason);
            add_key(state, work, &mut answer).await;
            match (&saved.outline, &saved.change) {
                (Some(outline), _) => {
                    answer["outline"] = outline_answer(outline);
                    add_mirror(&mut answer, mirror::record_outline(state, &held, outline).await);
                }
                (None, Some(change)) => add_mirror(&mut answer, mirror::record(state, &held, change).await),
                (None, None) => {}
            }
            Ok(answer)
        }
        "outline.set_numbering" => {
            let numbering = match params.get("numbering") {
                None | Some(Json::Null) => None,
                Some(Json::String(s)) => Some(s.clone()),
                Some(_) => return Err(invalid("numbering: Text oder null erwartet")),
            };
            let held = held.ok_or_else(|| RpcError::new(INTERNAL, "Schreibsperre fehlt"))?;
            let change = store.set_numbering(work, numbering.as_deref()).await?;
            let mut answer = outline_answer(&change);
            add_key(state, work, &mut answer).await;
            add_mirror(&mut answer, mirror::record_outline(state, &held, &change).await);
            Ok(answer)
        }
        "outline.move"
        | "outline.set_level"
        | "outline.rename"
        | "outline.add"
        | "outline.delete"
        | "outline.set_numbered" => {
            // Erst alle Parameter prüfen (gesperrt ist schon), dann schreiben.
            enum Op {
                Move(usize, usize, Place),
                Level(usize, u8),
                Rename(usize, String),
                Add(usize, Place, String, Option<u8>),
                Delete(usize),
                Numbered(usize, bool),
            }
            let op = match method {
                "outline.move" => Op::Move(required(&params, "node")?, required(&params, "target")?, place(&params)?),
                "outline.set_level" => {
                    let level = required(&params, "level")?;
                    Op::Level(required(&params, "node")?, u8::try_from(level).map_err(|_| invalid("level: 1 bis 3"))?)
                }
                "outline.rename" => Op::Rename(required(&params, "node")?, text(&params, "title")?),
                "outline.add" => {
                    let level = number(&params, "level")?
                        .map(|l| u8::try_from(l).map_err(|_| invalid("level: 0 bis 3")))
                        .transpose()?;
                    Op::Add(required(&params, "target")?, place(&params)?, text(&params, "title")?, level)
                }
                "outline.set_numbered" => {
                    if params.get("numbered").is_none() {
                        return Err(invalid("numbered fehlt (true = mit Nummer, false = ohne)"));
                    }
                    Op::Numbered(required(&params, "node")?, flag(&params, "numbered"))
                }
                _ => {
                    if !flag(&params, "confirm") {
                        return Err(invalid("Löschen bitte bestätigen (confirm)."));
                    }
                    Op::Delete(required(&params, "node")?)
                }
            };
            let held = held.ok_or_else(|| RpcError::new(INTERNAL, "Schreibsperre fehlt"))?;
            let mut adopted = None;
            // Runde 36 (store-5): vor dem Löschen lesen, was außer der aktuellen Fassung mitgelöscht wird.
            let mut losses = baruch_store::OutlineLosses::default();
            let change = match op {
                Op::Move(node, target, place) => store.outline_move(work, node, target, place, human).await?,
                Op::Level(node, level) => store.outline_set_level(work, node, level, human).await?,
                Op::Rename(node, title) => store.outline_rename(work, node, &title, human).await?,
                Op::Add(target, place, title, level) => {
                    // Über den Block-Anleger: kennt `level` (auch 0 = Teil) und meldet `adopted`.
                    let point = NewPoint { title, level };
                    let author = Author::Human { name: human.to_owned() };
                    let added = store.outline_add_many(work, Some(target), place, &[point], &author).await?;
                    adopted = Some(added.adopted);
                    added.change
                }
                Op::Delete(node) => {
                    losses = store.outline_delete_losses(work, node).await?;
                    store.outline_delete(work, node, human).await?
                }
                Op::Numbered(node, numbered) => store.outline_set_numbered(work, node, numbered, human).await?,
            };
            let mut answer = outline_answer(&change);
            if let Some(adopted) = adopted {
                answer["adopted"] = json!(adopted);
            }
            add_key(state, work, &mut answer).await;
            let by = mirror::OutlineBy::Human;
            add_mirror(&mut answer, mirror::record_outline_losses(state, &held, &change, by, &losses).await);
            Ok(answer)
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Vorschau eines offenen Antrags in seine JSON-Form schreiben (Runde 14): `preview` oder, wenn er
/// veraltet bzw. nicht mehr ausführbar ist, `preview_error`. Entschiedene Anträge bekommen
/// `preview_error` ohne Rechnung. Andere Fehler (Datenbank) gehen durch.
pub(crate) async fn add_preview(
    state: &AppState,
    work: WorkId,
    request: &baruch_store::OutlineRequest,
    value: &mut Json,
) -> Result<(), RpcError> {
    match state.store.outline_request_preview(work, request.id).await {
        Ok(points) => {
            let kind = state.store.work_meta(work).await?.kind;
            let shown: Vec<Json> = points
                .iter()
                .map(|p| {
                    let mut point = json!(p);
                    point["label"] = json!(preview_label(kind, p.level, &p.number));
                    point["before_label"] = json!(
                        p.before_level
                            .map(|level| { preview_label(kind, level, p.before_number.as_deref().unwrap_or("")) })
                    );
                    // „vorher → nachher“ nur, wo sich Art oder Nummer ändern oder ein Kapitel übernommen wird.
                    let changed = p.before_level.is_some()
                        && (p.before_level != Some(p.level)
                            || p.before_number.as_deref() != Some(p.number.as_str())
                            || p.mark == baruch_store::PreviewMark::Adopted);
                    point["changed_label"] = json!(changed && p.mark != baruch_store::PreviewMark::Deleted);
                    point
                })
                .collect();
            value["preview"] = json!(shown);
        }
        Err(e) => {
            let e = RpcError::from(e);
            if e.code == INTERNAL {
                return Err(e);
            }
            value["preview_error"] = json!(e.message);
        }
    }
    Ok(())
}

/// Art und Nummer eines Punkts der Vorschau, eindeutig je Ebene: „Teil 1“, „Kapitel 2“; ohne Nummer
/// „Kapitel (ohne Nummer)“. Die Nummer folgt dem Schema des Werks. Bei Predigten steht sie genau in der Form
/// des Schemas („Predigtteil II.“, wie Gliederung und Satz sie zeigen; Runde 17b, L8), beim Buch ohne den
/// Schlusspunkt („Kapitel 2“, unverändert seit Runde 16c).
pub(crate) fn preview_label(kind: baruch_core::WorkKind, level: u8, number: &str) -> String {
    let word = view::level_name(kind, level);
    let number = match kind {
        baruch_core::WorkKind::Sermon => number.trim(),
        baruch_core::WorkKind::Book => number.trim().trim_end_matches('.'),
    };
    if number.is_empty() { format!("{word} (ohne Nummer)") } else { format!("{word} {number}") }
}

/// Ein Antrag als JSON (RPC, MCP): wie [`baruch_store::OutlineRequest`], dazu `origin_label`
/// („MCP @mcp:claude-code“) und `decision_label` („bestätigt“, „abgelehnt“, „veraltet“).
pub(crate) fn request_json(request: &baruch_store::OutlineRequest) -> Json {
    let mut value = json!(request);
    value["origin_label"] = json!(request.origin.label());
    value["decision_label"] = json!(request.decision.map(|d| d.label()));
    value
}

// ---- Seiten ------------------------------------------------------------------------------------

/// Eigene Template-Umgebung der Gliederungsseiten (die der Arbeitsseite bleibt unberührt).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("outline.html", include_str!("../templates/outline.html")),
            ("outline_edit.html", include_str!("../templates/outline_edit.html")),
            ("outline_confirm.html", include_str!("../templates/outline_confirm.html")),
            ("outline_retry.html", include_str!("../templates/outline_retry.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Gliederung fehlerhaft");
        }
        env
    })
}

fn render(name: &str, ctx: Value) -> Result<String, RpcError> {
    templates()
        .get_template(name)
        .and_then(|t| t.render(ctx))
        .map_err(|e| RpcError::new(INTERNAL, format!("Vorlage {name}: {e}")))
}

/// Selbst gebaute Adresse (nur Ziffern, feste Namen, prozentkodierte Werte) als sicherer Wert für
/// `href`; `&` wird maskiert, `/` bleibt lesbar.
fn href(url: String) -> Value {
    debug_assert!(!url.contains(['"', '<', '>', '\'', ' ']), "unsichere Adresse {url:?}");
    Value::from_safe_string(url.replace('&', "&amp;"))
}

fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(e: RpcError) -> Response {
    (status_of(&e), format!("Fehler: {}", e.message)).into_response()
}

fn page_settings(headers: &HeaderMap) -> Value {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    Value::from(Serde(settings))
}

fn parse<T: std::str::FromStr>(value: Option<&str>) -> Option<T> {
    value.map(str::trim).filter(|v| !v.is_empty()).and_then(|v| v.parse().ok())
}

/// Werk aus der Adresse bzw. dem Formular; fehlt es, das Werk mit der kleinsten id.
async fn resolve_work(state: &AppState, work: Option<&str>) -> Result<WorkId, RpcError> {
    match work.map(str::trim).filter(|w| !w.is_empty()) {
        Some(text) => {
            let id: WorkId = text.parse().map_err(|_| invalid("work: Werk-id (Zahl) erwartet"))?;
            state.store.work_slug(id).await?;
            Ok(id)
        }
        None => Ok(state.store.default_work().await?),
    }
}

fn clean_lq(lq: Option<&str>) -> Option<String> {
    lq.map(str::trim).filter(|q| !q.is_empty()).map(str::to_owned)
}

/// Ein Eintrag der Gliederungsseiten.
#[derive(Serialize)]
struct Entry {
    id: usize,
    /// Fester Schlüssel (`Node::key`) für die Formulare.
    key: i64,
    number: String,
    title: String,
    level: u8,
    level_name: &'static str,
    up: Option<usize>,
    up_key: Option<i64>,
    up_title: Option<String>,
    down: Option<usize>,
    down_key: Option<i64>,
    down_title: Option<String>,
    promote: bool,
    demote: bool,
    selected: bool,
    has_children: bool,
    /// Zählt mit Nummer (`node.numbered`); sonst z. B. ein Vorwort ohne Nummer.
    numbered: bool,
    /// Runde 28: offene Befunde samt Unterpunkten (Zählregel §1.5; dieselbe Zahl wie in der Gliederung der
    /// Arbeitsseite) und am Punkt selbst; 0 ohne geladene Zahlen.
    findings: usize,
    findings_own: usize,
}

fn entries(work: &Work, selected: usize, unnumbered: &[i64]) -> Vec<Entry> {
    entries_with(work, selected, unnumbered, &std::collections::HashMap::new())
}

/// Einträge mit offenen Befunden je Punkt (`open_by_key` aus `Store::finding_open_counts`).
fn entries_with(
    work: &Work,
    selected: usize,
    unnumbered: &[i64],
    open_by_key: &std::collections::HashMap<i64, usize>,
) -> Vec<Entry> {
    let sums = crate::findings::view::subtree_open(work, open_by_key);
    let levels = view::levels(work);
    work.nodes
        .iter()
        .map(|n| {
            let (up, down) = view::neighbours(work, &levels, n.id);
            let level = levels[n.id];
            Entry {
                id: n.id,
                key: n.key,
                number: n.number.clone(),
                title: n.title.clone(),
                level,
                level_name: view::level_name(work.kind, level),
                up,
                up_key: up.map(|u| work.nodes[u].key),
                up_title: up.map(|u| work.nodes[u].title.clone()),
                down,
                down_key: down.map(|d| work.nodes[d].key),
                down_title: down.map(|d| work.nodes[d].title.clone()),
                promote: level >= 2,
                demote: (1..=2).contains(&level) && n.id > 0,
                selected: n.id == selected,
                has_children: work.children(Some(n.id)).next().is_some(),
                numbered: !unnumbered.contains(&n.key),
                findings: sums.get(n.id).copied().unwrap_or(0),
                findings_own: open_by_key.get(&n.key).copied().unwrap_or(0),
            }
        })
        .collect()
}

#[derive(Deserialize, Default)]
struct PageQuery {
    work: Option<String>,
    /// Fester Schlüssel des gewählten Punkts; geht vor `node` (Runde 8).
    key: Option<String>,
    node: Option<String>,
    lq: Option<String>,
}

/// Gemeinsamer Teil beider Seiten: Werk, gewählter Punkt, Suche und Adressen.
struct PageBase {
    work_id: WorkId,
    work: Work,
    node: usize,
    lq: Option<String>,
}

impl PageBase {
    async fn load(state: &AppState, query: &PageQuery) -> Result<PageBase, RpcError> {
        let work_id = resolve_work(state, query.work.as_deref()).await?;
        let work = state.store.load_work(work_id).await?;
        // Schlüssel vor Stelle; unbekannt → die Stelle, sonst der erste Punkt.
        let by_key = parse::<i64>(query.key.as_deref()).and_then(|k| work.nodes.iter().find(|n| n.key == k));
        let node = match by_key {
            Some(n) => n.id,
            None => parse::<usize>(query.node.as_deref()).filter(|&n| n < work.nodes.len()).unwrap_or(0),
        };
        Ok(PageBase { work_id, work, node, lq: clean_lq(query.lq.as_deref()) })
    }

    fn lq_q(&self) -> String {
        lq_param(self.lq.as_deref())
    }

    /// Fester Schlüssel des gewählten Punkts (0 in einem Werk ohne Gliederung).
    fn key(&self) -> i64 {
        self.work.nodes.get(self.node).map_or(0, |n| n.key)
    }

    fn work_url(&self) -> String {
        view::node_url(self.work_id, self.key(), None, false, "", &self.lq_q())
    }

    fn manager_url(&self) -> String {
        format!("/gliederung?work={}&key={}{}", self.work_id, self.key(), self.lq_q())
    }

    fn base_context(&self) -> Value {
        context! {
            work => context! { id => self.work_id, title => self.work.title.as_str(), author => self.work.author.as_str() },
            lq => self.lq.as_deref(),
            lq_q => self.lq_q(),
            work_url => href(self.work_url()),
            manager_url => href(self.manager_url()),
        }
    }
}

fn page_error(e: RpcError) -> Response {
    if e.code == INVALID_PARAMS && e.message.starts_with("unbekanntes Werk") {
        return (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response();
    }
    error_response(e)
}

/// Runde 23a: So viele offene Anträge bekommen ihre Vorschau sofort beim Laden von `/gliederung`. Jede Vorschau ist
/// eine Schreib-Transaktion mit Zurückrollen; bei sehr vielen offenen Anträgen (ein Assistent kann bis zur Obergrenze
/// je Herkunft stellen) hielte die Seite sonst lange die Schreibsperre der Datenbank. Die übrigen rechnet die Seite
/// auf Anforderung (`GET /gliederung/antraege/{id}/vorschau`, ein Link – ohne JavaScript bedienbar).
pub const MAX_IMMEDIATE_PREVIEWS: usize = 20;

/// `GET /gliederung?work&node&lq`: alle Einträge mit Ebene, Nummer und Schaltflächen, Nummerierung.
async fn manager(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(query): Query<PageQuery>) -> Response {
    manager_page(&state, &headers, &query, None).await
}

/// `GET /gliederung/antraege/{id}/vorschau?work&key&lq` (Runde 23a, nur lesend): dieselbe Seite wie `/gliederung`,
/// dazu die Vorschau des Antrags `id` gerechnet und aufgeklappt (für Anträge jenseits der ersten
/// [`MAX_IMMEDIATE_PREVIEWS`]). Der Link trägt den Anker `#antrag-<id>`.
async fn request_preview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<i64>,
    Query(query): Query<PageQuery>,
) -> Response {
    manager_page(&state, &headers, &query, Some(id)).await
}

async fn manager_page(state: &Arc<AppState>, headers: &HeaderMap, query: &PageQuery, asked: Option<i64>) -> Response {
    let base = match PageBase::load(state, query).await {
        Ok(base) => base,
        Err(e) => return page_error(e),
    };
    let numbering = match state.store.outline_numbering(base.work_id).await {
        Ok(numbering) => numbering,
        Err(e) => return error_response(e.into()),
    };
    let unnumbered = match state.store.unnumbered_keys(base.work_id).await {
        Ok(keys) => keys,
        Err(e) => return error_response(e.into()),
    };
    // Anträge (Migration 0012): alle offenen, dazu die zehn zuletzt entschiedenen.
    let requests = match state.store.outline_requests(base.work_id, false).await {
        Ok(requests) => requests,
        Err(e) => return error_response(e.into()),
    };
    // Vorschau je offenem Antrag (Runde 14), auf der Seite ohne JavaScript in <details>. Runde 23a: sofort nur für
    // die ersten MAX_IMMEDIATE_PREVIEWS und für den ausdrücklich verlangten; die übrigen bekommen einen Link.
    let mut open: Vec<Json> = Vec::new();
    for (index, request) in requests.iter().filter(|r| r.is_open()).enumerate() {
        let mut value = request_json(request);
        let wanted = asked == Some(request.id);
        if index < MAX_IMMEDIATE_PREVIEWS || wanted {
            if let Err(e) = add_preview(state, base.work_id, request, &mut value).await {
                return error_response(e);
            }
            value["preview_open"] = json!(wanted);
        } else {
            let key_q = query
                .key
                .as_deref()
                .and_then(|k| k.trim().parse::<i64>().ok())
                .map_or(String::new(), |k| format!("&key={k}"));
            value["preview_later"] = json!(true);
            value["preview_url"] = json!(format!(
                "/gliederung/antraege/{}/vorschau?work={}{key_q}{}#antrag-{}",
                request.id,
                base.work_id,
                base.lq_q(),
                request.id
            ));
        }
        open.push(value);
    }
    let decided: Vec<Json> = requests.iter().rev().filter(|r| !r.is_open()).take(10).map(request_json).collect();
    // Runde 16c: Zeiten aus dem Zeitwächter (nur lesend); gezeigt nur bei Predigten oder gesetzter Zielzeit.
    let timing = match crate::timing::work_timing(state, base.work_id).await {
        Ok(timing) => crate::timing::timing_json(&timing),
        Err(e) => return error_response(e),
    };
    // Runde 28: offene Befunde je Punkt für die Abzeichen „k offen“ (wie in der Gliederung der Arbeitsseite).
    let open_findings = match state.store.finding_open_counts(base.work_id).await {
        Ok(counts) => counts,
        Err(e) => return error_response(e.into()),
    };
    let ctx = context! {
        timing => Value::from(Serde(timing)),
        settings => page_settings(headers),
        findings_url => href(format!("/werke/{}/befunde", base.work_id)),
        entries => Value::from(Serde(entries_with(&base.work, base.node, &unnumbered, &open_findings))),
        numbering => Value::from(Serde(numbering)),
        open_requests => Value::from(Serde(open)),
        decided_requests => Value::from(Serde(decided)),
        ..base.base_context()
    };
    match render("outline.html", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_response(e),
    }
}

/// `GET /gliederung/bearbeiten?work&node&lq`: einen Punkt umbenennen, umstufen, verschieben, neu
/// anlegen, löschen.
async fn edit_page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(query): Query<PageQuery>) -> Response {
    let base = match PageBase::load(&state, &query).await {
        Ok(base) => base,
        Err(e) => return page_error(e),
    };
    let unnumbered = match state.store.unnumbered_keys(base.work_id).await {
        Ok(keys) => keys,
        Err(e) => return error_response(e.into()),
    };
    let all = entries(&base.work, base.node, &unnumbered);
    let Some(entry) = all.iter().find(|e| e.selected) else {
        return (StatusCode::NOT_FOUND, "Das Werk hat keine Gliederung.").into_response();
    };
    let names: Vec<&str> = (0..=3).map(|l| view::level_name(base.work.kind, l)).collect();
    let ctx = context! {
        settings => page_settings(&headers),
        entry => Value::from(Serde(entry)),
        entries => Value::from(Serde(&all)),
        level_names => names,
        ..base.base_context()
    };
    match render("outline_edit.html", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_response(e),
    }
}

/// Formular ohne JavaScript; Felder wie die RPC-Parameter, alle als Text.
#[derive(Deserialize, Default)]
struct OutlineForm {
    method: String,
    work: Option<String>,
    node: Option<String>,
    key: Option<String>,
    target: Option<String>,
    target_key: Option<String>,
    place: Option<String>,
    level: Option<String>,
    title: Option<String>,
    body: Option<String>,
    based_on: Option<String>,
    /// Nachbesserung R22: Stand der Basis, den der Editor geladen hatte.
    based_on_amend: Option<String>,
    split: Option<String>,
    merge: Option<String>,
    confirm: Option<String>,
    /// `vorlage` (oder leer) = aus der Werkvorlage, `keine` = keine Nummern, sonst ein Muster.
    numbering: Option<String>,
    /// `outline.set_numbered`: `1` = mit Nummer, `0` = ohne.
    numbered: Option<String>,
    /// Antrag (`outline.request_decide`): id und `confirm` bzw. `reject`.
    id: Option<String>,
    decision: Option<String>,
    /// `preview` bzw. `gliederung` (zurück zum Gliederungsmanager).
    view: Option<String>,
    lq: Option<String>,
    /// Runde 22: Speichermodus von `outline.save` (`auto` | `new`; der Editor schickt `auto`).
    mode: Option<String>,
    /// Runde 22: Knopf „Als neue Fassung speichern“ (`1`) → `mode` „new“.
    as_new: Option<String>,
}

/// `POST /gliederung/act`: dieselben Methoden wie RPC, danach 303 auf die Seite; `outline.save`, das
/// teilen oder zusammenführen würde, fragt erst auf einer eigenen Seite nach.
async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<OutlineForm>) -> Response {
    if !handles(&form.method) || matches!(form.method.as_str(), "outline.check" | "outline.requests") {
        return (StatusCode::BAD_REQUEST, "unbekannte Aktion").into_response();
    }
    let work = match resolve_work(&state, form.work.as_deref()).await {
        Ok(work) => work,
        Err(e) => return error_response(e),
    };
    let mut params = json!({});
    for (key, value) in [
        ("node", &form.node),
        ("key", &form.key),
        ("target", &form.target),
        ("target_key", &form.target_key),
        ("place", &form.place),
        ("level", &form.level),
        ("title", &form.title),
        ("body", &form.body),
        ("based_on", &form.based_on),
        ("based_on_amend", &form.based_on_amend),
        ("split", &form.split),
        ("merge", &form.merge),
        ("confirm", &form.confirm),
        ("id", &form.id),
        ("numbered", &form.numbered),
        ("decision", &form.decision),
        ("mode", &form.mode),
    ] {
        if let Some(value) = value {
            params[key] = json!(value);
        }
    }
    params["numbering"] = match form.numbering.as_deref().map(str::trim) {
        None | Some("") | Some("vorlage") => Json::Null,
        Some("keine") => json!(""),
        Some(pattern) => json!(pattern),
    };
    // Runde 22: „Als neue Fassung speichern“ geht vor dem versteckten `mode=auto`.
    if matches!(form.as_new.as_deref().map(str::trim), Some("1" | "on" | "true")) {
        params["mode"] = json!("new");
    }
    let lq = clean_lq(form.lq.as_deref());
    let view = form.view.as_deref().unwrap_or("");

    if form.method == "outline.save"
        && !flag(&params, "split")
        && !flag(&params, "merge")
        && let Some(response) = confirm_page(&state, &headers, work, &form, &params, lq.as_deref()).await
    {
        return response;
    }

    let result = match call(&state, work, &form.method, params.clone()).await {
        Ok(result) => result,
        // Runde 23a (Codereview web-4): Ein abgelehntes Speichern zeigt Meldung **und** Text, statt den Text zu verlieren.
        Err(e) if form.method == "outline.save" && form.body.is_some() => {
            return retry_page(&state, &headers, work, &form, &params, lq.as_deref(), e).await;
        }
        Err(e) => return error_response(e),
    };
    if form.method == "outline.request_decide" {
        // Zurück zu den Anträgen; die Entscheidung (auch „veraltet“ mit Grund) steht dort in der Liste.
        let key = result["key"].as_i64().or_else(|| parse(form.key.as_deref()));
        let key_q = key.map(|k| format!("&key={k}")).unwrap_or_default();
        let lq_q = lq_param(lq.as_deref());
        return Redirect::to(&format!("/gliederung?work={work}{key_q}{lq_q}#antraege")).into_response();
    }
    let node = result["node"].as_u64().or_else(|| parse(form.node.as_deref())).unwrap_or(0);
    let node = usize::try_from(node).unwrap_or(0);
    // Key-Form (Runde 8): Schlüssel aus der Antwort, sonst nachgeschlagen, sonst der des Formulars.
    let key = match result["key"].as_i64() {
        Some(key) => Some(key),
        None => state.store.node_key(work, node).await.ok().or_else(|| parse(form.key.as_deref())),
    };
    let lq_q = lq_param(lq.as_deref());
    let url = match (key, view) {
        // Der Anker bleibt die Stelle: die Gliederungsseite nummeriert ihre Zeilen nach ihr.
        (Some(key), "gliederung") => format!("/gliederung?work={work}&key={key}{lq_q}#punkt-{node}"),
        (Some(key), _) => {
            let rev = result["rev"].as_u64().and_then(|r| u32::try_from(r).ok());
            view::node_url(work, key, rev, view == "preview", "", &lq_q)
        }
        (None, "gliederung") => format!("/gliederung?work={work}{lq_q}"),
        (None, _) => format!("/?work={work}{lq_q}"),
    };
    // Runde 22: Rückmeldung ohne JavaScript („fortgeschrieben (Stand k)“ bzw. „neue Fassung n (Grund)“) im Editor-Kopf.
    // Nur, wenn das Formular einen Modus schickt (der Editor seit Runde 22); ältere Formulare leiten wie bisher weiter.
    let with_mode = form.mode.as_deref().is_some_and(|m| !m.trim().is_empty()) || form.as_new.is_some();
    let url = match (form.method.as_str(), result["saved"].as_str()) {
        _ if url.contains('#') || !with_mode => url,
        ("outline.save", Some("amended")) => {
            format!("{url}&saved=amended-{}", result["amend_seq"].as_u64().unwrap_or(0))
        }
        ("outline.save", Some("new")) => {
            let rev = result["rev"].as_u64().unwrap_or(0);
            match result["reason"].as_str() {
                Some(reason) => format!("{url}&saved=new-{rev}-{reason}"),
                None => format!("{url}&saved=new-{rev}"),
            }
        }
        _ => url,
    };
    Redirect::to(&url).into_response()
}

/// Runde 23a (Codereview web-4): Seite nach einem abgelehnten `outline.save` ohne JavaScript – Meldung und der
/// eingegebene Text in einem Textfeld zum erneuten Speichern (Status wie [`error_response`]). Lässt sich die Seite
/// nicht bauen, bleibt es bei der Klartext-Meldung.
async fn retry_page(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    form: &OutlineForm,
    params: &Json,
    lq: Option<&str>,
    error: RpcError,
) -> Response {
    let status = status_of(&error);
    let Ok(loaded) = state.store.load_work(work).await else { return error_response(error) };
    let node = match rpc::resolve_keys(state, work, params.clone()).await {
        Ok(resolved) => required(&resolved, "node").ok().and_then(|n| loaded.nodes.get(n).map(|found| (n, found))),
        Err(_) => None,
    };
    let back = match node {
        Some((_, found)) => view::node_url(work, found.key, None, false, "", &lq_param(lq)),
        None => format!("/?work={work}{}", lq_param(lq)),
    };
    let based_on: Option<u32> = parse(form.based_on.as_deref());
    let based_on_amend: Option<u32> = parse(form.based_on_amend.as_deref());
    let ctx = context! {
        settings => page_settings(headers),
        work => context! { id => work, title => loaded.title.as_str() },
        node => node.map(|(id, found)| context! { id => id, key => found.key, title => found.title.as_str() }),
        message => error.message.as_str(),
        body => form.body.as_deref().unwrap_or("").replace("\r\n", "\n"),
        based_on => based_on,
        based_on_amend => based_on_amend,
        mode => save_mode_word(params),
        lq => lq,
        view => form.view.as_deref().filter(|v| *v == "preview"),
        back_url => href(back),
    };
    match render("outline_retry.html", ctx) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(_) => error_response(error),
    }
}

/// Speichermodus, den ein Folgeformular weiterreicht: `auto` oder `new` aus den (schon um `as_new` bereinigten)
/// Parametern; alles andere → keiner.
fn save_mode_word(params: &Json) -> Option<&'static str> {
    match params.get("mode").and_then(Json::as_str).map(str::trim) {
        Some("auto") => Some("auto"),
        Some("new") => Some("new"),
        _ => None,
    }
}

/// Rückfrageseite, wenn Speichern ohne Bestätigung teilen oder zusammenführen würde (und das gültig
/// wäre); sonst `None` (dann meldet `outline.save` selbst, was nicht geht).
async fn confirm_page(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    form: &OutlineForm,
    params: &Json,
    lq: Option<&str>,
) -> Option<Response> {
    let params = rpc::resolve_keys(state, work, params.clone()).await.ok()?;
    let node = required(&params, "node").ok()?;
    // Wie `outline.save`: Markdown-Überschriften zählen schon in der Rückfrage als Überschriften.
    let converted = baruch_typst::markdown_headings(form.body.as_deref()?);
    let body = converted.as_ref();
    let check = state.store.outline_check(work, node, body).await.ok()?;
    if !check.valid || (check.split.is_empty() && !check.merge) {
        return None;
    }
    let loaded = state.store.load_work(work).await.ok()?;
    let current = loaded.nodes.get(node)?;
    let previous = node.checked_sub(1).and_then(|p| loaded.nodes.get(p)).map(|p| p.title.clone());
    let based_on: Option<u32> = parse(form.based_on.as_deref());
    let based_on_amend: Option<u32> = parse(form.based_on_amend.as_deref());
    let back = view::node_url(work, current.key, None, false, "", &lq_param(lq));
    let ctx = context! {
        settings => page_settings(headers),
        work => context! { id => work, title => loaded.title.as_str() },
        node => context! { id => node, key => current.key, title => current.title.as_str() },
        based_on => based_on,
        based_on_amend => based_on_amend,
        // Runde 23a: Der Speichermodus geht durch die Rückfrage mit; sonst fehlte danach die Rückmeldung
        // („neue Fassung n (Überschrift geändert)“) im Editor-Kopf.
        mode => save_mode_word(&params),
        body => body.replace("\r\n", "\n"),
        check => Value::from(Serde(&check)),
        previous => previous,
        lq => lq,
        view => form.view.as_deref().filter(|v| *v == "preview"),
        back_url => href(back),
    };
    Some(match render("outline_confirm.html", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_response(e),
    })
}

/// `GET /gliederung` und `GET /gliederung/bearbeiten`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/gliederung", get(manager))
        .route("/gliederung/bearbeiten", get(edit_page))
        // Runde 23a: Vorschau eines Antrags auf Anforderung (lesend).
        .route("/gliederung/antraege/{id}/vorschau", get(request_preview))
}

/// `POST /gliederung/act` (im Router der schreibenden Routen, also hinter `guard::same_origin`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/gliederung/act", post(act))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_values_count_as_numbers_and_flags() {
        let params = json!({ "node": "3", "target": 4, "split": "1", "merge": "on", "confirm": true, "x": "nein" });
        assert_eq!(required(&params, "node").unwrap(), 3, "Zahl als Text aus dem Formular");
        assert_eq!(required(&params, "target").unwrap(), 4, "Zahl aus JSON");
        assert!(flag(&params, "split") && flag(&params, "merge") && flag(&params, "confirm"), "1, on, true");
        assert!(!flag(&params, "x") && !flag(&params, "fehlt"), "anderes und Fehlendes ist falsch");
        assert!(required(&json!({ "node": "-1" }), "node").is_err(), "negative Zahl abgelehnt");
        assert!(place(&json!({ "place": "neben" })).is_err(), "unbekannter Ort abgelehnt");
    }
}
