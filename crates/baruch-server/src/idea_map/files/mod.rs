// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ein- und Ausgabe der Mindmap und ihre Bedienung ohne JavaScript (Vertrag Runde 34 §5/§8, Besitz
//! mindmap_austausch).
//!
//! - [`pages`] (GET): die Ausgaben `/werke/{id}/karte.svg` und `.pdf` (über
//!   [`crate::idea_map::bild::mindmap::render`], eigenständig), `.md` ([`export::markdown`]), `.mm`
//!   ([`freemind::write`]) und `.xmind` ([`xmind::write`]); die Gliederungsansicht `/werke/{id}/karte/gliederung`
//!   ([`outline`]) und das Formular `/werke/{id}/karte/einlesen` ([`upload`]). Alle nehmen die Filter der Seite.
//! - [`writes`] (POST, hinter `guard::same_origin`): `/werke/{id}/karte/act` (Formulare der Mindmap, [`ACT_METHODS`]),
//!   `/werke/{id}/karte/einlesen` (Vorschau, legt nichts an) und `/werke/{id}/karte/einlesen/anlegen` (Bestätigung,
//!   legt über [`crate::idea_map::tree::import`] an).
//!
//! **Wolke** (Nachtrag des Nutzers 2026-11-25): Einfälle ohne Platz liegen in der Wolke neben der Mindmap
//! ([`export::cloud`]). Das Formular kennt dafür beim Umhängen das Feld `ziel` (`wolke`, `i:<id>`, `n:<key>`): „in
//! die Wolke“ löst Elternknoten und Bezug, „in die Mindmap unter …“ hängt unter einen Einfall bzw. an ein Kapitel.
//! Ein Knoten wandert **mit** seinen Unterknoten in die Wolke (sie bleiben seine Unterknoten).
//!
//! Filter, Fehlerseite der Mindmap und Werkangaben kommen aus [`crate::idea_map::bild`] (dort `pub(crate)`).

pub mod export;
pub mod freemind;
pub mod import;
pub mod outline;
pub mod upload;
pub mod xmind;

use std::sync::Arc;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_render::Source;
use baruch_store::{IdeaId, WorkId};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::idea_map::MapFilter;
use crate::idea_map::bild::mindmap::{self, RenderOptions};
use crate::idea_map::bild::{
    Extra, back_filter, filter_query, page_response, parse_filter, plain, status_of, work_info,
};
use crate::idea_map::tree::{self, MindMap};
use crate::pdf::{PDF_TIMEOUT, PdfKind};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// Ausgaben, Gliederungsansicht und Formular „Einlesen“ (GET), gemischt in `lib.rs` bei den Seiten.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/karte.svg", get(svg_file))
        .route("/werke/{id}/karte.pdf", get(pdf_file))
        .route("/werke/{id}/karte.md", get(md_file))
        .route("/werke/{id}/karte.mm", get(mm_file))
        .route("/werke/{id}/karte.xmind", get(xmind_file))
        .route("/werke/{id}/karte/gliederung", get(outline::page))
        .route("/werke/{id}/karte/einlesen", get(upload::form))
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/karte/act", post(act))
        .route("/werke/{id}/karte/einlesen", post(upload::preview).layer(DefaultBodyLimit::max(upload::UPLOAD_LIMIT)))
        .route(
            "/werke/{id}/karte/einlesen/anlegen",
            post(upload::apply).layer(DefaultBodyLimit::max(upload::APPLY_LIMIT)),
        )
}

// ---- Laden ---------------------------------------------------------------------------------------

/// Ordnername des Werks und seine Mindmap; unbekanntes Werk → 404, sonst Fehler mit Status nach RpcError.
// `Response` als Fehler ist hier gewollt (fertige Antwort für `?` im Handler), wie in `bild::load`.
#[allow(clippy::result_large_err)]
pub(crate) async fn load_tree(
    state: &AppState,
    work: WorkId,
    filter: &MapFilter,
) -> Result<(String, MindMap), Response> {
    let slug = match work_info(state, work).await? {
        Some((slug, _)) => slug,
        None => return Err(plain(StatusCode::NOT_FOUND, "unbekanntes Werk")),
    };
    let map =
        tree::tree(state, work, filter).await.map_err(|e| plain(status_of(&e), format!("Fehler: {}", e.message)))?;
    Ok((slug, map))
}

/// Ist das eine Kennung eines Knotens (`w`, `n:<key>`, `i:<id>`)?
pub(crate) fn is_node_id(id: &str) -> bool {
    id == "w"
        || id
            .strip_prefix("n:")
            .or_else(|| id.strip_prefix("i:"))
            .is_some_and(|n| !n.is_empty() && n.len() <= 18 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Eingeklappte Knoten aus `zu=<Kennungen, Komma>` (nur gültige Kennungen, höchstens 500).
pub(crate) fn closed_of(pairs: &[(String, String)]) -> Vec<String> {
    let mut closed: Vec<String> = pairs
        .iter()
        .filter(|(name, _)| name == "zu")
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .filter(|id| is_node_id(id) && *id != "w")
        .map(str::to_owned)
        .collect();
    closed.sort();
    closed.dedup();
    closed.truncate(500);
    closed
}

// ---- Handler (GET) -------------------------------------------------------------------------------

/// Kopfzeilen einer Ausgabe: Art, ggf. Anhang mit Dateinamen aus dem slug (`[a-z0-9-]`, ohne Maskierung).
fn file(content_type: &'static str, disposition: Option<String>, body: impl Into<axum::body::Body>) -> Response {
    let mut response = (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        body.into(),
    )
        .into_response();
    if let Some(value) = disposition.and_then(|d| HeaderValue::from_str(&d).ok()) {
        response.headers_mut().insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

/// Die eigenständige SVG-Datei der Mindmap (dieselbe Zeichnung wie auf der Seite).
fn standalone_svg(map: &MindMap, closed: Vec<String>) -> String {
    mindmap::render(map, &RenderOptions { standalone: true, closed, back: filter_query(&map.filter) }).svg
}

async fn svg_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => file(
            "image/svg+xml",
            Some(format!("inline; filename=\"{slug}-mindmap.svg\"")),
            standalone_svg(&map, closed_of(&pairs)),
        ),
        Err(response) => response,
    }
}

async fn md_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => file(
            "text/markdown; charset=utf-8",
            Some(format!("inline; filename=\"{slug}-mindmap.md\"")),
            export::markdown(&map),
        ),
        Err(response) => response,
    }
}

async fn mm_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => file(
            "application/x-freemind",
            Some(format!("attachment; filename=\"{slug}-mindmap.mm\"")),
            freemind::write(&export::sheet(&map)),
        ),
        Err(response) => response,
    }
}

async fn xmind_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load_tree(&state, work, &filter).await {
        Ok((slug, map)) => match xmind::write(&export::sheet(&map)) {
            Ok(bytes) => file(
                "application/vnd.xmind.workbook",
                Some(format!("attachment; filename=\"{slug}-mindmap.xmind\"")),
                bytes,
            ),
            Err(message) => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {message}")),
        },
        Err(response) => response,
    }
}

/// Typst-Zeichenkette (`"…"`): `\` und `"` maskiert, Zeilenumbrüche als `\n`, andere Steuerzeichen entfernt.
fn typst_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Typst-Quelle der Mindmap als PDF: A4 quer, Titel und Filter (Nutzertexte nur als Zeichenketten), darunter die
/// eigenständige SVG-Datei (`svg`, aus [`mindmap::render`]) auf die Seite eingepasst.
pub fn pdf_source(map: &MindMap, svg: String) -> Source {
    let filter = export::filter_words(&map.filter);
    let mut main =
        String::from("#set page(paper: \"a4\", flipped: true, margin: 10mm)\n#set text(lang: \"de\", size: 9pt)\n");
    main.push_str(&format!(
        "#text(size: 14pt, weight: \"bold\", {})\n",
        typst_string(&format!("Mindmap: {}", map.title))
    ));
    if !filter.is_empty() {
        main.push_str(&format!("\n#text(fill: gray, {})\n", typst_string(&format!("Filter: {filter}"))));
    }
    main.push_str("\n#align(center, image(\"mindmap.svg\", width: 100%, height: 165mm, fit: \"contain\"))\n");
    Source { main, files: vec![("mindmap.svg".to_owned(), svg.into_bytes())] }
}

async fn pdf_file(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    if crate::guard::cross_site_get(&headers) {
        return crate::typeset::foreign_get();
    }
    let (filter, extra) = parse_filter(&pairs);
    let (slug, map) = match load_tree(&state, work, &filter).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let source = pdf_source(&map, standalone_svg(&map, closed_of(&pairs)));
    match crate::pdf::source_to_pdf(work, PdfKind::Map, source, PDF_TIMEOUT).await {
        Ok(pdf) => crate::pdf::attachment(&format!("{slug}-mindmap.pdf"), pdf.bytes),
        Err(e) => {
            let status = if e.message == crate::pdf::BUSY {
                StatusCode::CONFLICT
            } else if e.message.starts_with("Typst brauchte länger") {
                StatusCode::SERVICE_UNAVAILABLE
            } else if e.code == REFUSED {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            page_response(&state, &headers, work, &filter, &extra, status, Some(e.message)).await
        }
    }
}

// ---- Formulare (POST) ----------------------------------------------------------------------------

/// Methoden des Formulars der Mindmap (Vertrag §5).
pub const ACT_METHODS: &[&str] = &[
    "idea.create",
    "idea.update",
    "idea.move",
    "idea.delete",
    "idea.restore",
    "idea.link",
    "idea.unlink",
    "idea.link_restore",
    "idea.map_pos_reset",
    "idea.map_groups_name",
    "idea.map_group_accept",
    "idea.map_group_dismiss",
];

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Zahl aus einem Formularfeld; „Einfall 12“ oder „12 – Laube“ (Auswahlliste) zählen als 12.
fn number(value: Option<&str>, name: &str) -> Result<i64, RpcError> {
    let value = value.unwrap_or("").trim();
    let value = value.strip_prefix("Einfall").unwrap_or(value).trim_start();
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().map_err(|_| invalid(format!("{name}: Zahl erwartet")))
}

/// Wahlfreier Verweis aus einem vorhandenen Formularfeld: leer, `null`, `none` oder `keiner` → `null`, sonst Zahl.
fn reference(value: &str, name: &str) -> Result<Option<i64>, RpcError> {
    match value.trim() {
        "" | "null" | "none" | "keiner" => Ok(None),
        other => number(Some(other), name).map(Some),
    }
}

/// Gehört der Einfall zu diesem Werk? Sonst „abgelehnt“ (409): Die Mindmap ändert nur Einfälle ihres Werks.
pub(crate) async fn ensure_in_work(state: &AppState, idea: IdeaId, work: WorkId) -> Result<(), RpcError> {
    let found = state.store.idea_get(idea).await?;
    if found.work != Some(work) {
        return Err(RpcError::new(REFUSED, format!("Einfall {idea} gehört nicht zu diesem Werk.")));
    }
    Ok(())
}

/// Letzter Wert eines Formularfelds.
fn form_field(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone())
}

/// Ergebnis einer Aktion: Wort für `ok`, ggf. die gelöste Verbindung (`undo`) bzw. der gelöschte Einfall (`wieder`).
#[derive(Debug, Default, PartialEq)]
struct Done {
    ok: &'static str,
    undo: Option<i64>,
    restore: Option<IdeaId>,
}

impl Done {
    fn word(ok: &'static str) -> Self {
        Done { ok, ..Done::default() }
    }
}

/// Eine Aktion der Mindmap ausführen. Jeder genannte Einfall muss zum Werk der Adresse gehören.
async fn run_act(
    state: &AppState,
    work: WorkId,
    method: &str,
    pairs: &[(String, String)],
    filter: &MapFilter,
) -> Result<Done, RpcError> {
    let field = |name: &str| form_field(pairs, name);
    let filled = |name: &str| form_field(pairs, name).filter(|v| !v.trim().is_empty());
    match method {
        // Neuer Einfall: unter einem Einfall (`parent`), an einem Kapitel (`key`) oder – ohne beides – in der Wolke.
        "idea.create" => {
            let mut params = json!({ "work": work });
            for name in ["title", "words", "note"] {
                if let Some(value) = filled(name) {
                    params[name] = Json::String(value.trim().to_owned());
                }
            }
            for name in ["parent", "before"] {
                if let Some(value) = filled(name) {
                    let id = number(Some(&value), name)?;
                    ensure_in_work(state, id, work).await?;
                    params[name] = json!(id);
                }
            }
            if let Some(key) = filled("key") {
                params["key"] = json!(number(Some(&key), "key")?);
            }
            crate::ideas::call(state, "idea.create", params).await?;
            Ok(Done::word("angelegt"))
        }
        // Umbenennen (`title`) und/oder Bezug setzen (`key`); Felder, die das Formular nicht schickt, bleiben.
        "idea.update" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            let mut params = json!({ "id": id });
            let title = field("title");
            if let Some(title) = &title {
                params["title"] = Json::String(title.trim().to_owned());
            }
            let key = field("key");
            if let Some(key) = &key {
                params["key"] = Json::String(key.trim().to_owned());
            }
            for name in ["words", "note"] {
                if let Some(value) = field(name) {
                    params[name] = Json::String(value);
                }
            }
            crate::ideas::call(state, "idea.update", params).await?;
            Ok(Done::word(match (title.is_some(), key.is_some()) {
                (true, _) => "umbenannt",
                (false, true) => "bezug",
                _ => "gespeichert",
            }))
        }
        // Umhängen und Ordnen. `ziel` (`wolke`, `i:<id>`, `n:<key>`) ist die Kurzform für Formulare; daneben die
        // Felder der RPC (`parent`, `key`, `before`; vorhanden und leer = `null`).
        "idea.move" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            let mut params = json!({ "id": id });
            let mut to_cloud = false;
            if let Some(target) = filled("ziel") {
                let target = target.trim();
                if matches!(target, "wolke" | "w") {
                    params["parent"] = Json::Null;
                    params["key"] = Json::Null;
                    to_cloud = true;
                } else if let Some(parent) = target.strip_prefix("i:") {
                    params["parent"] = json!(number(Some(parent), "ziel")?);
                } else if let Some(key) = target.strip_prefix("n:") {
                    params["parent"] = Json::Null;
                    params["key"] = json!(number(Some(key), "ziel")?);
                } else {
                    return Err(invalid("ziel: „wolke“, „i:<Einfall>“ oder „n:<Kapitel>“ erwartet"));
                }
            }
            for name in ["parent", "key", "before"] {
                if let Some(value) = field(name) {
                    params[name] = json!(reference(&value, name)?);
                }
            }
            for name in ["parent", "before"] {
                if let Some(other) = params.get(name).and_then(Json::as_i64) {
                    ensure_in_work(state, other, work).await?;
                }
            }
            let answer = crate::ideas::call(state, "idea.move", params).await?;
            Ok(Done::word(if answer["changed"] == Json::Bool(false) {
                "ort-unveraendert"
            } else if to_cloud {
                "in-wolke"
            } else {
                "umgehaengt"
            }))
        }
        "idea.delete" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            crate::ideas::call(state, "idea.delete", json!({ "id": id, "confirm": true })).await?;
            Ok(Done { ok: "geloescht", undo: None, restore: Some(id) })
        }
        "idea.restore" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            crate::ideas::call(state, "idea.restore", json!({ "id": id })).await?;
            Ok(Done::word("einfall-wieder"))
        }
        "idea.link" => {
            let from = number(field("from").as_deref(), "from")?;
            let to = number(field("to").as_deref(), "to (Einfall)")?;
            ensure_in_work(state, from, work).await?;
            let mut params = json!({ "from": from, "to": to, "kind": field("kind").unwrap_or_default().trim() });
            if let Some(note) = filled("note") {
                params["note"] = Json::String(note);
            }
            let answer = crate::idea_map::call(state, "idea.link", params).await?;
            Ok(Done::word(if answer["changed"] == Json::Bool(false) { "unveraendert" } else { "verbunden" }))
        }
        "idea.unlink" | "idea.link_restore" => {
            let id = number(field("id").as_deref(), "id (Verbindung)")?;
            let link = state.store.idea_link_get(id).await?;
            ensure_in_work(state, link.from, work).await?;
            crate::idea_map::call(state, method, json!({ "id": id })).await?;
            Ok(if method == "idea.unlink" {
                Done { ok: "geloest", undo: Some(id), restore: None }
            } else {
                Done::word("wiederhergestellt")
            })
        }
        // Lagen bleiben gespeichert, wirken in der Mindmap aber nicht mehr (Vertrag §0).
        "idea.map_pos_reset" => {
            let mut params = json!({ "work": work });
            if let Some(idea) = filled("idea") {
                params["idea"] = json!(number(Some(&idea), "idea")?);
            }
            crate::idea_map::call(state, "idea.map_pos_reset", params).await?;
            Ok(Done::word("zurueckgesetzt"))
        }
        // Runde 19b: Gruppen benennen (Vorschlag eines Assistenten), Übernehmen nur hier und über RPC (Mensch).
        "idea.map_groups_name" => {
            let persona = filled("persona");
            let answer = crate::idea_map::groups::name_groups(state, work, filter, persona.as_deref()).await?;
            let proposed = answer["proposals"].as_array().map_or(0, Vec::len);
            Ok(Done::word(match (proposed, answer.get("message").is_some()) {
                (0, true) => "keine-gruppen",
                (0, false) => "unbenannt",
                _ => "benannt",
            }))
        }
        "idea.map_group_accept" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            let name = filled("name");
            crate::idea_map::groups::accept(state, work, &group, name.as_deref(), filter).await?;
            Ok(Done::word("uebernommen"))
        }
        "idea.map_group_dismiss" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            crate::idea_map::groups::dismiss(state, work, &group);
            Ok(Done::word("verworfen"))
        }
        other => Err(unknown_action(other)),
    }
}

fn unknown_action(method: &str) -> RpcError {
    let shown: String = method.chars().filter(|c| !c.is_control()).take(60).collect();
    RpcError::new(METHOD_NOT_FOUND, format!("Unbekannte Aktion „{shown}“ auf der Mindmap."))
}

/// `POST /werke/{id}/karte/act` (urlencoded): Feld `method` ∈ [`ACT_METHODS`], dazu die Felder der Aktion (`id`,
/// `title`, `words`, `note`, `parent`, `key`, `before`, `ziel`, `from`, `to`, `kind`, `idea`, `group`, `name`,
/// `persona`), `back` (nur der Abfrageteil; sein Filter gilt auch für die Gruppen), `seite` ∈ `karte` | `gliederung`
/// (Ziel der Weiterleitung, Vorgabe `karte`) und `wahl` (gewählter Knoten der Gliederungsansicht). Danach 303 auf
/// `/werke/{id}/karte[/gliederung]?<back>&ok=<wort>`; Fehler als Seite mit der Meldung (422 ungültig, 409 abgelehnt,
/// 400 unbekannte Aktion).
async fn act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let field = |name: &str| form_field(&pairs, name);
    let filter = back_filter(field("back").as_deref());
    match work_info(&state, work).await {
        Ok(Some(_)) => {}
        Ok(None) => return plain(StatusCode::NOT_FOUND, "unbekanntes Werk"),
        Err(response) => return response,
    }
    let to_outline = field("seite").as_deref().map(str::trim) == Some("gliederung");
    let chosen = field("wahl").map(|w| w.trim().to_owned()).filter(|w| is_node_id(w));
    let method = field("method").unwrap_or_default();
    let outcome = if ACT_METHODS.contains(&method.trim()) {
        run_act(&state, work, method.trim(), &pairs, &filter).await
    } else {
        Err(unknown_action(method.trim()))
    };
    match outcome {
        Ok(done) => {
            let mut query = filter_query(&filter);
            if !query.is_empty() {
                query.push('&');
            }
            query.push_str(&format!("ok={}", done.ok));
            if let Some(undo) = done.undo {
                query.push_str(&format!("&undo={undo}"));
            }
            if to_outline {
                if let Some(restore) = done.restore {
                    query.push_str(&format!("&wieder={restore}"));
                }
                // Der gelöschte Knoten kann nicht gewählt bleiben.
                if let Some(chosen) = chosen.filter(|c| done.restore.is_none_or(|gone| *c != format!("i:{gone}"))) {
                    query.push_str(&format!("&wahl={chosen}"));
                }
                Redirect::to(&format!("/werke/{work}/karte/gliederung?{query}")).into_response()
            } else {
                Redirect::to(&format!("/werke/{work}/karte?{query}")).into_response()
            }
        }
        Err(e) if e.code == INTERNAL => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
        Err(e) => {
            let status = status_of(&e);
            if to_outline {
                let view = outline::View { chosen, ..outline::View::default() };
                outline::render(&state, &headers, work, &filter, &view, status, Some(e.message)).await
            } else {
                page_response(&state, &headers, work, &filter, &Extra::default(), status, Some(e.message)).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typst_strings_cannot_break_out() {
        assert_eq!(typst_string("a\"b\\c\nd#e"), "\"a\\\"b\\\\c\\nd#e\"", "nur als Zeichenkette, # bleibt Text");
    }

    #[test]
    fn form_numbers_accept_the_list_labels() {
        assert_eq!(number(Some("12"), "to").unwrap(), 12, "Ziffern");
        assert_eq!(number(Some("Einfall 12"), "to").unwrap(), 12, "mit Wort davor");
        assert_eq!(number(Some("12 – Laube"), "to").unwrap(), 12, "Eintrag der Auswahlliste");
        assert!(number(Some("Laube"), "to").is_err(), "ohne Zahl → Fehler");
        assert_eq!(reference("", "parent").unwrap(), None, "ein vorhandenes, leeres Feld heißt „keiner“");
        assert_eq!(reference(" 7 ", "parent").unwrap(), Some(7), "sonst die Zahl");
    }

    #[test]
    fn closed_nodes_come_only_from_valid_ids() {
        let pairs = vec![("zu".to_owned(), "i:5, n:12,w,i:x,<b>,i:5".to_owned()), ("zu".to_owned(), "n:3".to_owned())];
        assert_eq!(
            closed_of(&pairs),
            vec!["i:5", "n:12", "n:3"],
            "nur Kennungen von Knoten, ohne Doppel; die Wurzel lässt sich nicht einklappen, Unsinn fällt weg"
        );
        assert!(is_node_id("w") && is_node_id("n:1") && !is_node_id("n:") && !is_node_id("i:-1"), "Kennungen");
    }
}
