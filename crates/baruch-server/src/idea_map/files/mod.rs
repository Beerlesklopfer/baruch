// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ausgaben und Formulare der Gedankenkarte bzw. Mindmap (Runde 34, Schnittstelle R34-S1; danach Besitz
//! mindmap_austausch). In S1 unverändert aus `idea_map/bild/mod.rs` hierher verschoben (Vertrag Runde 25, §4.3/§4.4).
//!
//! - [`pages`]: `/werke/{id}/karte.svg` (eigenständig, `image/svg+xml`), `/werke/{id}/karte.pdf` (über
//!   [`crate::pdf::source_to_pdf`] mit [`PdfKind::Map`]), `/werke/{id}/karte.md` (Gliederung Kapitel → Einfälle →
//!   Verbindungen) und `/werke/{id}/karte.mm` (FreeMind). Alle Ausgaben nehmen dieselben Filter wie die Seite.
//! - [`writes`]: POST `/werke/{id}/karte/act` (hinter `guard::same_origin`): `method` ∈ `idea.update` (Bezug
//!   setzen, über [`crate::ideas::call`]), `idea.link`, `idea.unlink`, `idea.link_restore`, `idea.map_pos_reset`
//!   (über [`crate::idea_map::call`]), seit Runde 19b `idea.map_groups_name`, `idea.map_group_accept`,
//!   `idea.map_group_dismiss`; danach 303 auf die gefilterte Karte mit `ok=<wort>`, Fehler als Seite mit der Meldung
//!   (422 ungültig, 409 abgelehnt, 400 unbekannte Aktion).
//!
//! Filter, Laden und Fehlerseite kommen aus [`crate::idea_map::bild`] (dort `pub(crate)`); Text-Ausgaben schreibt
//! [`export`]. Im S1-Stand zeichnen `.svg`/`.pdf` noch über das eingefrorene `bild::svg`/`bild::layout`; künftig
//! über `bild::mindmap::render` (Vertrag R34 §5).

pub mod export;

use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_render::Source;
use baruch_store::{IdeaId, WorkId};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::idea_map::bild::{
    Extra, back_filter, filter_query, layout, load, page_response, parse_filter, plain, status_of, svg, work_info,
};
use crate::idea_map::{IdeaMap, MapFilter};
use crate::pdf::{PDF_TIMEOUT, PdfKind};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// Ausgaben (GET), gemischt in `lib.rs` bei den Seiten.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/werke/{id}/karte.svg", get(svg_file))
        .route("/werke/{id}/karte.pdf", get(pdf_file))
        .route("/werke/{id}/karte.md", get(md_file))
        .route("/werke/{id}/karte.mm", get(mm_file))
}

/// Formulare (POST), gemischt in `lib.rs` im writes-Router hinter `guard::same_origin`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/karte/act", post(act))
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

async fn svg_file(
    State(state): State<Arc<AppState>>,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let (filter, _) = parse_filter(&pairs);
    match load(&state, work, &filter).await {
        Ok((slug, map, layout)) => file(
            "image/svg+xml",
            Some(format!("inline; filename=\"{slug}-karte.svg\"")),
            svg::render(&map, &layout, true),
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
    match load(&state, work, &filter).await {
        Ok((slug, map, _)) => file(
            "text/markdown; charset=utf-8",
            Some(format!("inline; filename=\"{slug}-karte.md\"")),
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
    match load(&state, work, &filter).await {
        Ok((slug, map, _)) => file(
            "application/x-freemind",
            Some(format!("attachment; filename=\"{slug}-karte.mm\"")),
            export::freemind(&map),
        ),
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

/// Typst-Quelle der Karte als PDF: A4 quer, Titel und Filter (Nutzertexte nur als Zeichenketten), darunter die
/// eigenständige SVG-Datei auf die Seite eingepasst.
pub fn pdf_source(map: &IdeaMap, layout: &layout::Layout) -> Source {
    let filter = export::filter_words(&map.filter);
    let mut main =
        String::from("#set page(paper: \"a4\", flipped: true, margin: 10mm)\n#set text(lang: \"de\", size: 9pt)\n");
    main.push_str(&format!(
        "#text(size: 14pt, weight: \"bold\", {})\n",
        typst_string(&format!("Gedankenkarte: {}", map.title))
    ));
    if !filter.is_empty() {
        main.push_str(&format!("\n#text(fill: gray, {})\n", typst_string(&format!("Filter: {filter}"))));
    }
    main.push_str("\n#align(center, image(\"karte.svg\", width: 100%, height: 165mm, fit: \"contain\"))\n");
    Source { main, files: vec![("karte.svg".to_owned(), svg::render(map, layout, true).into_bytes())] }
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
    let (slug, map, layout) = match load(&state, work, &filter).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    match crate::pdf::source_to_pdf(work, PdfKind::Map, pdf_source(&map, &layout), PDF_TIMEOUT).await {
        Ok(pdf) => crate::pdf::attachment(&format!("{slug}-karte.pdf"), pdf.bytes),
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

/// Methoden des Formulars der Karte.
pub const ACT_METHODS: &[&str] = &[
    "idea.update",
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

/// Gehört der Einfall zu diesem Werk? Sonst -32602 (die Karte ändert nur Einfälle ihres Werks).
async fn ensure_in_work(state: &AppState, idea: IdeaId, work: WorkId) -> Result<(), RpcError> {
    let found = state.store.idea_get(idea).await?;
    if found.work != Some(work) {
        return Err(invalid(format!("Einfall {idea} gehört nicht zu diesem Werk.")));
    }
    Ok(())
}

/// Letzter Wert eines Formularfelds.
fn form_field(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs.iter().rev().find(|(n, _)| n == name).map(|(_, v)| v.clone())
}

/// Eine Aktion der Karte ausführen; Ergebnis: Wort für `ok` und ggf. die gelöste Verbindung.
async fn run_act(
    state: &AppState,
    work: WorkId,
    method: &str,
    pairs: &[(String, String)],
    filter: &MapFilter,
) -> Result<(&'static str, Option<i64>), RpcError> {
    let field = |name: &str| form_field(pairs, name);
    match method {
        "idea.update" => {
            let id = number(field("id").as_deref(), "id")?;
            ensure_in_work(state, id, work).await?;
            let key = field("key").unwrap_or_default();
            crate::ideas::call(state, "idea.update", json!({ "id": id, "key": key.trim() })).await?;
            Ok(("bezug", None))
        }
        "idea.link" => {
            let from = number(field("from").as_deref(), "from")?;
            let to = number(field("to").as_deref(), "to (Einfall)")?;
            ensure_in_work(state, from, work).await?;
            let mut params = json!({ "from": from, "to": to, "kind": field("kind").unwrap_or_default().trim() });
            if let Some(note) = field("note").filter(|n| !n.trim().is_empty()) {
                params["note"] = Json::String(note);
            }
            let answer = crate::idea_map::call(state, "idea.link", params).await?;
            Ok((if answer["changed"] == Json::Bool(false) { "unveraendert" } else { "verbunden" }, None))
        }
        "idea.unlink" | "idea.link_restore" => {
            let id = number(field("id").as_deref(), "id (Verbindung)")?;
            let link = state.store.idea_link_get(id).await?;
            ensure_in_work(state, link.from, work).await?;
            crate::idea_map::call(state, method, json!({ "id": id })).await?;
            Ok(if method == "idea.unlink" { ("geloest", Some(id)) } else { ("wiederhergestellt", None) })
        }
        "idea.map_pos_reset" => {
            let mut params = json!({ "work": work });
            if let Some(idea) = field("idea").filter(|i| !i.trim().is_empty()) {
                params["idea"] = json!(number(Some(&idea), "idea")?);
            }
            crate::idea_map::call(state, "idea.map_pos_reset", params).await?;
            Ok(("zurueckgesetzt", None))
        }
        // Runde 19b: Gruppen benennen (Vorschlag eines Assistenten), Übernehmen nur hier und über RPC (Mensch).
        "idea.map_groups_name" => {
            let persona = field("persona").filter(|p| !p.trim().is_empty());
            let answer = crate::idea_map::groups::name_groups(state, work, filter, persona.as_deref()).await?;
            let proposed = answer["proposals"].as_array().map_or(0, Vec::len);
            Ok(match (proposed, answer.get("message").is_some()) {
                (0, true) => ("keine-gruppen", None),
                (0, false) => ("unbenannt", None),
                _ => ("benannt", None),
            })
        }
        "idea.map_group_accept" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            let name = field("name").filter(|n| !n.trim().is_empty());
            crate::idea_map::groups::accept(state, work, &group, name.as_deref(), filter).await?;
            Ok(("uebernommen", None))
        }
        "idea.map_group_dismiss" => {
            let group = crate::idea_map::groups::group_param(&json!({ "group": field("group").unwrap_or_default() }))?;
            crate::idea_map::groups::dismiss(state, work, &group);
            Ok(("verworfen", None))
        }
        other => Err(RpcError::new(METHOD_NOT_FOUND, format!("Unbekannte Aktion „{other}“ auf der Gedankenkarte."))),
    }
}

/// `POST /werke/{id}/karte/act` (urlencoded): Felder `method`, `id`, `key`, `from`, `to`, `kind`, `note`, `idea`,
/// seit Runde 19b `group`, `name`, `persona`, dazu `back` (nur der Abfrageteil der Karte; sein Filter gilt auch für
/// die Gruppen). Danach 303 auf `/werke/{id}/karte?<back>&ok=<wort>`.
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
    let method = field("method").unwrap_or_default();
    let outcome = if ACT_METHODS.contains(&method.trim()) {
        run_act(&state, work, method.trim(), &pairs, &filter).await
    } else {
        Err(RpcError::new(METHOD_NOT_FOUND, format!("Unbekannte Aktion „{}“ auf der Gedankenkarte.", method.trim())))
    };
    match outcome {
        Ok((ok, undo)) => {
            let mut query = filter_query(&filter);
            if !query.is_empty() {
                query.push('&');
            }
            query.push_str(&format!("ok={ok}"));
            if let Some(undo) = undo {
                query.push_str(&format!("&undo={undo}"));
            }
            Redirect::to(&format!("/werke/{work}/karte?{query}")).into_response()
        }
        Err(e) if e.code == INTERNAL => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
        Err(e) => {
            let status = status_of(&e);
            page_response(&state, &headers, work, &filter, &Extra::default(), status, Some(e.message)).await
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
    }
}
