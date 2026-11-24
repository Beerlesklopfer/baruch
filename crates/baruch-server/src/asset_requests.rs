// SPDX-License-Identifier: AGPL-3.0-or-later
//! Anträge zu Bildern (Vertrag Runde 11, Migration 0018): Assistenten beantragen über MCP
//! (`asset_replace`, `asset_set_cover`, `asset_delete`, `asset_add` mit `role: "cover"`), der Mensch
//! entscheidet hier – auf der Seite „Bilder“ (Abschnitt `#antraege`, [`panel`]), über das Formular
//! `POST /werke/bilder/antraege/act` oder über RPC.
//!
//! - **RPC** ([`handles`], [`call`]): `asset.request_list {work, open?: bool = true}` → `{requests}`;
//!   `asset.request_decide {work, id, decision: "confirm"|"reject"}` → `{request, commit?, mirror?}`.
//! - **Bestätigen** prüft zuerst, ob der Antrag veraltet ist (`expected_sha256` passt nicht mehr zur
//!   aktuellen bzw. genannten Fassung, das Bild ist weg, der Umschlag ist inzwischen ein anderer); dann
//!   `stale` mit Grund, nichts geändert. Sonst führt Baruch ihn unter dem Namen des Menschen aus:
//!   Ersetzen und Löschen über den Store und **einen** Spiegel-Commit mit Antragsteller und Bestätiger
//!   ([`mirror::AssetBy::Confirmed`]). Den Umschlag setzt Baruch seit Runde 20c unter derselben Schreibsperre
//!   über `Store::set_work_cover` und schreibt **genau einen** Commit „Bildantrag n bestätigt: …“
//!   ([`mirror::describe_asset_request`], im Rumpf Umschlag vorher/nachher). Ist ein Bild, das gelöscht werden
//!   soll, der Umschlag, kommt vor dem Löschen der Commit „Umschlag entfernt“
//!   ([`crate::cover::release_cover_before_delete`]). Lehnt die Ausführung fachlich ab, ebenfalls `stale` mit ihrer
//!   Meldung.
//! - **Ablehnen** ändert nur den Antrag (kein Commit).
//! - Anträge zur Umschlag-Anordnung (Runde 27, op `cover_layout`) prüft und führt
//!   [`crate::cover_layout::decide_request`] aus; die Liste zeigt je Block eine Zeile (`block_lines`).
//! - Der vorgeschlagene Inhalt eines Antrags zum Ersetzen lässt sich ansehen:
//!   `GET /werke/{id}/bilder/antraege/{nr}/datei` (MIME, `nosniff`, CSP mit `sandbox`).
//! - **Runde 21c:** Ein Antrag mit `rename` (Typwechsel) wird mit [`baruch_store::Store::replace_asset_renaming`]
//!   ausgeführt: das Bild bekommt die neue Endung, die Pfade bleiben. Vor jedem Listen und Entscheiden (und vor dem
//!   Ausliefern der Antragsdatei) entfernt [`purge`] den Inhalt entschiedener Anträge zum Ersetzen, die länger als
//!   [`baruch_store::ASSET_REQUEST_PURGE_DAYS`] Tage entschieden sind; die Antragsdatei antwortet dann 410 Gone.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_store::{AssetEncoding, AssetOp, AssetRequest, Decision, StoreError, WorkId};
use minijinja::Environment;
use minijinja::value::Serde;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::mirror::{self, HUMAN_NAME};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// Sicherheitskopf für den vorgeschlagenen Inhalt (wie bei den Bildern selbst).
const IMAGE_CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; sandbox";

pub fn handles(method: &str) -> bool {
    matches!(method, "asset.request_list" | "asset.request_decide")
}

/// Umschlag des Werks (Bildname, [`crate::cover::cover_name`]).
async fn hook_cover_name(state: &AppState, work: WorkId) -> Result<Option<String>, StoreError> {
    crate::cover::cover_name(state, work).await
}

/// Umschlag des Werks für Antrag und Prüfung auf Veralten (MCP `asset_set_cover`, `asset_add` mit Rolle).
pub(crate) async fn cover_name(state: &AppState, work: WorkId) -> Result<Option<String>, StoreError> {
    hook_cover_name(state, work).await
}

/// Ein Antrag als JSON: Felder des Stores (ohne Inhalt), dazu `origin_label`, `decision_label` und
/// `url` des Bilds bzw. `content_url` des vorgeschlagenen Inhalts.
pub fn request_json(request: &AssetRequest) -> Value {
    let mut value = json!(request);
    value["origin_label"] = json!(request.origin.label());
    value["decision_label"] = json!(request.decision.map(|d| d.label()));
    let name = request.op.asset_name();
    if !name.is_empty() {
        value["url"] = json!(format!("/werke/{}/bilder/{name}", request.work));
    }
    if matches!(request.op, AssetOp::Replace { .. }) {
        value["content_url"] = json!(format!("/werke/{}/bilder/antraege/{}/datei", request.work, request.id));
    }
    if let AssetOp::CoverLayout { blocks, .. } = &request.op {
        // Umschlag-Anordnung (Runde 27): Verweis auf die Umschlagseite und je Block eine Zeile für die Liste.
        value["url"] = json!(format!("/werke/{}/umschlag", request.work));
        value["block_lines"] = json!(blocks.iter().map(crate::cover_layout::block_line).collect::<Vec<_>>());
    }
    value
}

/// Ausgang einer Entscheidung (für Weiterleitung und Antwort).
pub struct Decided {
    pub request: AssetRequest,
    pub commit: Option<String>,
    pub mirror: Option<String>,
    /// Bei bestätigtem Umschlag: Antwort von `work.set_cover`.
    pub cover: Option<Value>,
}

/// Fachliche Ablehnung bei der Ausführung (dann ist der Antrag nicht mehr ausführbar)?
fn refusal(e: &StoreError) -> bool {
    matches!(
        e,
        StoreError::Refused(_)
            | StoreError::Asset(_)
            // Runde 23a: ungültige Umschlag-Blöcke haben eine eigene Fehlerart.
            | StoreError::CoverBlock(_)
            | StoreError::UnknownAsset { .. }
            | StoreError::UnknownRevision { .. }
    )
}

async fn stale(state: &AppState, work: WorkId, id: i64, message: &str) -> Result<Decided, RpcError> {
    let _held = state.write_lock.lock().await;
    let request = state.store.asset_request_decide(work, id, Decision::Stale, HUMAN_NAME, Some(message)).await?;
    Ok(Decided { request, commit: None, mirror: None, cover: None })
}

fn split(result: Result<Option<String>, String>) -> (Option<String>, Option<String>) {
    match result {
        Ok(commit) => (commit, None),
        Err(e) => (None, Some(e)),
    }
}

/// Inhalt eines Antrags zum Ersetzen als Bytes.
fn replace_bytes(content: &str, encoding: AssetEncoding) -> Result<Vec<u8>, StoreError> {
    match encoding {
        AssetEncoding::Base64 => baruch_store::decode_base64(content),
        AssetEncoding::Svg => Ok(content.as_bytes().to_vec()),
    }
}

/// Inhalt entschiedener Anträge zum Ersetzen entfernen, die älter als
/// [`baruch_store::ASSET_REQUEST_PURGE_DAYS`] Tage entschieden sind (Runde 21c). Ergebnis: Zahl der bereinigten.
pub async fn purge(state: &AppState, work: WorkId) -> Result<usize, StoreError> {
    state.store.purge_decided_asset_requests(work, baruch_store::ASSET_REQUEST_PURGE_DAYS).await
}

/// Über einen offenen Antrag entscheiden (nur der Mensch). Siehe Moduldoku.
pub async fn decide(state: &AppState, work: WorkId, id: i64, confirm: bool) -> Result<Decided, RpcError> {
    let store = &state.store;
    purge(state, work).await?;
    let request = store.asset_request_get(work, id).await?;
    if !request.is_open() {
        return Err(RpcError::new(REFUSED, "Über diesen Antrag ist schon entschieden."));
    }
    if !confirm {
        let _held = state.write_lock.lock().await;
        let request = store.asset_request_decide(work, id, Decision::Rejected, HUMAN_NAME, None).await?;
        return Ok(Decided { request, commit: None, mirror: None, cover: None });
    }
    match &request.op {
        AssetOp::SetCover { name, expected_cover } => {
            let now = cover_name(state, work).await?;
            if now != *expected_cover {
                let shown = |c: &Option<String>| c.as_deref().map_or_else(|| "keiner".to_owned(), |n| format!("„{n}“"));
                let message =
                    format!("Der Umschlag ist inzwischen {} (beim Antrag {}).", shown(&now), shown(expected_cover));
                return stale(state, work, id, &message).await;
            }
            if let Some(name) = name
                && let Err(StoreError::UnknownAsset { .. }) = store.asset_revisions(work, name).await
            {
                return stale(state, work, id, &format!("Das Bild „{name}“ gibt es nicht mehr.")).await;
            }
            // Runde 20c: Umschlag setzen und Antrag entscheiden unter einer Schreibsperre, dann **ein** Commit
            // „Bildantrag n bestätigt: …“ mit dem Umschlag vorher/nachher im Rumpf (früher zwei Commits).
            let held = state.write_lock.lock().await;
            let change = match store.set_work_cover(work, name.as_deref()).await {
                Ok(change) => change,
                Err(e) if refusal(&e) => {
                    drop(held);
                    return stale(state, work, id, &format!("Nicht mehr ausführbar: {e}")).await;
                }
                Err(e) => return Err(e.into()),
            };
            let request = store.asset_request_decide(work, id, Decision::Confirmed, HUMAN_NAME, None).await?;
            let mut commit_request = mirror::describe_asset_request(&request);
            commit_request.body.push_str(&crate::cover::cover_change_body(&change));
            let result = crate::cover::commit_request(state, &held, work, commit_request).await;
            if let Err(e) = &result {
                eprintln!("Git-Spiegel: {e}");
            }
            let (commit, mirror) = split(result);
            let mut answer = json!({ "work": work, "cover": crate::cover::cover_info(state, work).await? });
            if let Some(commit) = &commit {
                answer["commit"] = json!(commit);
            }
            Ok(Decided { request, commit, mirror, cover: Some(answer) })
        }
        AssetOp::Replace { name, encoding, content, caption, alt, expected_sha256, .. } => {
            let held = state.write_lock.lock().await;
            let revisions = match store.asset_revisions(work, name).await {
                Ok(list) => list,
                Err(StoreError::UnknownAsset { .. }) => {
                    drop(held);
                    return stale(state, work, id, &format!("Das Bild „{name}“ gibt es nicht mehr.")).await;
                }
                Err(e) => return Err(e.into()),
            };
            let current = revisions.iter().find(|r| r.current);
            if current.is_none_or(|r| &r.sha256 != expected_sha256) {
                let number = current.map_or(0, |r| r.number);
                drop(held);
                let message = format!(
                    "„{name}“ hat inzwischen eine andere aktuelle Fassung ({number}); der Antrag bezog sich auf den Stand davor."
                );
                return stale(state, work, id, &message).await;
            }
            let bytes = replace_bytes(content, *encoding)?;
            let note = if request.note.is_empty() {
                format!("Antrag {id} von {}", request.origin.name())
            } else {
                format!("Antrag {id} von {}: {}", request.origin.name(), request.note)
            };
            let as_base64 = *encoding == AssetEncoding::Base64;
            let replaced =
                store.replace_asset_renaming(work, name, name, &bytes, as_base64, &note, HUMAN_NAME, request.rename);
            let mut change = match replaced.await {
                Ok(change) => change,
                Err(e) if refusal(&e) => {
                    drop(held);
                    return stale(state, work, id, &format!("Nicht mehr ausführbar: {e}")).await;
                }
                Err(e) => return Err(e.into()),
            };
            // Bildunterschrift und Alternativtext des Antrags gehören zur neuen Fassung (ein Commit).
            let before = change.before.clone();
            let caption = caption.as_deref().filter(|c| before.as_ref().is_none_or(|b| b.caption != *c));
            let alt = alt.as_deref().filter(|a| before.as_ref().is_none_or(|b| b.alt != *a));
            if caption.is_some() || alt.is_some() {
                // Nach einem Typwechsel heißt das Bild schon neu (change.name).
                store.update_asset_revision(work, &change.name, change.rev, caption, alt, None, HUMAN_NAME).await?;
                for (label, value) in [("Bildunterschrift", caption), ("Alternativtext", alt)] {
                    if let Some(value) = value {
                        change.detail.push_str(&format!("{label} neu: {}\n", value.replace('\n', " ")));
                    }
                }
            }
            let request = store.asset_request_decide(work, id, Decision::Confirmed, HUMAN_NAME, None).await?;
            let by = mirror::AssetBy::Confirmed(&request);
            let (commit, mirror) = split(mirror::record_asset_by(state, &held, &change, by).await);
            Ok(Decided { request, commit, mirror, cover: None })
        }
        AssetOp::Delete { name, rev, expected_sha256 } => {
            let held = state.write_lock.lock().await;
            let revisions = match store.asset_revisions(work, name).await {
                Ok(list) => list,
                Err(StoreError::UnknownAsset { .. }) => {
                    drop(held);
                    return stale(state, work, id, &format!("Das Bild „{name}“ gibt es nicht mehr.")).await;
                }
                Err(e) => return Err(e.into()),
            };
            let target = match rev {
                None => revisions.iter().find(|r| r.current),
                Some(rev) => revisions.iter().find(|r| r.number == *rev),
            };
            let message = match target {
                None => Some(format!("Fassung {} von „{name}“ gibt es nicht mehr.", rev.unwrap_or(0))),
                Some(r) if &r.sha256 != expected_sha256 => Some(match rev {
                    None => format!(
                        "„{name}“ hat inzwischen eine andere aktuelle Fassung ({}); der Antrag bezog sich auf den Stand davor.",
                        r.number
                    ),
                    Some(rev) => format!("Fassung {rev} von „{name}“ ist inzwischen eine andere."),
                }),
                Some(r) if rev.is_some() && r.current => Some(format!(
                    "Fassung {} von „{name}“ ist inzwischen die aktuelle und lässt sich nicht löschen.",
                    r.number
                )),
                Some(_) => None,
            };
            if let Some(message) = message {
                drop(held);
                return stale(state, work, id, &message).await;
            }
            // Runde 20c: Ist das Bild der Umschlag, zuerst „Umschlag entfernt“ (eigener Commit), dann löschen.
            if rev.is_none() {
                crate::cover::release_cover_before_delete(state, &held, work, name).await?;
            }
            let done = match rev {
                None => store.delete_asset(work, name, HUMAN_NAME).await,
                Some(rev) => store.delete_asset_revision(work, name, *rev, HUMAN_NAME).await,
            };
            let change = match done {
                Ok(change) => change,
                Err(e) if refusal(&e) => {
                    drop(held);
                    return stale(state, work, id, &format!("Nicht mehr ausführbar: {e}")).await;
                }
                Err(e) => return Err(e.into()),
            };
            let request = store.asset_request_decide(work, id, Decision::Confirmed, HUMAN_NAME, None).await?;
            let by = mirror::AssetBy::Confirmed(&request);
            let (commit, mirror) = split(mirror::record_asset_by(state, &held, &change, by).await);
            Ok(Decided { request, commit, mirror, cover: None })
        }
        // Umschlag-Anordnung (Runde 27): Prüfung auf Veralten und Ausführung in `cover_layout`.
        AssetOp::CoverLayout { .. } => crate::cover_layout::decide_request(state, work, &request).await,
    }
}

// ---- RPC ---------------------------------------------------------------------------------------

fn id_param(params: &Value) -> Result<i64, RpcError> {
    match params.get("id") {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(|| RpcError::new(INVALID_PARAMS, "id: Zahl erwartet")),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| RpcError::new(INVALID_PARAMS, "id: Zahl erwartet")),
        _ => Err(RpcError::new(INVALID_PARAMS, "id fehlt (Nummer des Antrags)")),
    }
}

fn confirm_param(params: &Value) -> Result<bool, RpcError> {
    match params.get("decision").and_then(Value::as_str).map(str::trim) {
        Some("confirm") => Ok(true),
        Some("reject") => Ok(false),
        _ => Err(RpcError::new(INVALID_PARAMS, "decision: confirm oder reject erwartet")),
    }
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "asset.request_list" => {
            let open = match params.get("open") {
                None | Some(Value::Null) => true,
                Some(Value::Bool(b)) => *b,
                Some(_) => return Err(RpcError::new(INVALID_PARAMS, "open: true oder false erwartet")),
            };
            purge(state, work).await?;
            let list = state.store.asset_requests(work, open).await?;
            Ok(json!({ "work": work, "requests": list.iter().map(request_json).collect::<Vec<_>>() }))
        }
        "asset.request_decide" => {
            let id = id_param(&params)?;
            let confirm = confirm_param(&params)?;
            let decided = decide(state, work, id, confirm).await?;
            let mut answer = json!({ "request": request_json(&decided.request) });
            if let Some(commit) = decided.commit {
                answer["commit"] = json!(commit);
            }
            if let Some(mirror) = decided.mirror {
                answer["mirror"] = json!(mirror);
            }
            if let Some(cover) = decided.cover {
                answer["cover"] = cover;
            }
            Ok(answer)
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

// ---- Abschnitt der Seite „Bilder“ --------------------------------------------------------------

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        env.add_template("asset_requests.html", include_str!("../templates/fragments/asset_requests.html"))
            .expect("Vorlage der Bildanträge fehlerhaft");
        env
    })
}

/// HTML des Abschnitts `#antraege` für die Seite „Bilder“: offene Anträge mit „Bestätigen“ und
/// „Ablehnen“ (Formulare, ohne JavaScript), die zuletzt entschiedenen mit Ergebnis und Grund.
pub async fn panel(state: &AppState, work: WorkId) -> Result<String, RpcError> {
    purge(state, work).await?;
    let requests = state.store.asset_requests(work, false).await?;
    let open: Vec<Value> = requests.iter().filter(|r| r.is_open()).map(request_json).collect();
    let decided: Vec<Value> = requests.iter().rev().filter(|r| !r.is_open()).take(10).map(request_json).collect();
    let ctx = minijinja::context! {
        work => work,
        open => minijinja::Value::from(Serde(open)),
        decided => minijinja::Value::from(Serde(decided)),
    };
    templates()
        .get_template("asset_requests.html")
        .and_then(|t| t.render(ctx))
        .map_err(|e| RpcError::new(INTERNAL, format!("Vorlage asset_requests.html: {e}")))
}

// ---- Formular und vorgeschlagener Inhalt -------------------------------------------------------

/// `POST /werke/bilder/antraege/act` (hinter `guard::same_origin`) und
/// `GET /werke/{id}/bilder/antraege/{nr}/datei` (nur lesen; liegt hier, weil der Router dieses Moduls
/// nur über `writes` eingebunden ist – `same_origin` lässt GET aus der eigenen Seite und direkte Aufrufe durch).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/bilder/antraege/act", post(act))
        .route("/werke/{id}/bilder/antraege/{nr}/datei", get(content))
}

#[derive(Deserialize)]
struct ActForm {
    work: Option<String>,
    id: Option<String>,
    decision: Option<String>,
}

async fn act(State(state): State<Arc<AppState>>, Form(form): Form<ActForm>) -> Response {
    let work = match form.work.as_deref().map(str::trim).and_then(|w| w.parse::<WorkId>().ok()) {
        Some(work) => work,
        None => return (StatusCode::UNPROCESSABLE_ENTITY, "work fehlt (Werk-id)").into_response(),
    };
    let params = json!({ "id": form.id, "decision": form.decision });
    let (id, confirm) = match (id_param(&params), confirm_param(&params)) {
        (Ok(id), Ok(confirm)) => (id, confirm),
        (Err(e), _) | (_, Err(e)) => return (StatusCode::UNPROCESSABLE_ENTITY, e.message).into_response(),
    };
    match decide(&state, work, id, confirm).await {
        Ok(decided) => {
            let hint = match decided.request.decision {
                Some(Decision::Confirmed) => "antrag_bestaetigt",
                Some(Decision::Stale) => "antrag_veraltet",
                _ => "antrag_abgelehnt",
            };
            Redirect::to(&format!("/werke/{work}/bilder?hinweis={hint}#antraege")).into_response()
        }
        Err(e) => {
            let status =
                if e.code == INTERNAL { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNPROCESSABLE_ENTITY };
            (status, e.message).into_response()
        }
    }
}

async fn content(State(state): State<Arc<AppState>>, Path((work, nr)): Path<(WorkId, i64)>) -> Response {
    let not_found = || (StatusCode::NOT_FOUND, "Antrag mit Inhalt gibt es nicht").into_response();
    if let Err(e) = purge(&state, work).await
        && !matches!(e, StoreError::UnknownWork(_))
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response();
    }
    let request = match state.store.asset_request_get(work, nr).await {
        Ok(request) => request,
        Err(StoreError::UnknownRequest { .. } | StoreError::UnknownWork(_)) => return not_found(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let AssetOp::Replace { mime, encoding, content, .. } = &request.op else { return not_found() };
    if request.purged {
        let decided = request.decided.as_deref().map(crate::localtime::minutes).unwrap_or_default();
        return (
            StatusCode::GONE,
            [(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"))],
            format!(
                "Der vorgeschlagene Inhalt von Antrag {nr} ist entfernt: über den Antrag wurde am {decided} entschieden, \
                 und Baruch hebt den Inhalt entschiedener Anträge {} Tage lang auf.",
                baruch_store::ASSET_REQUEST_PURGE_DAYS
            ),
        )
            .into_response();
    }
    let bytes = match replace_bytes(content, *encoding) {
        Ok(bytes) => bytes,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {e}")).into_response(),
    };
    let mime = HeaderValue::from_str(mime).unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(IMAGE_CSP)),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        bytes,
    )
        .into_response()
}
