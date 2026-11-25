// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einlesen einer fremden Mindmap über die Seite (Vertrag Runde 34 §5/§8, Besitz mindmap_austausch), ganz ohne
//! JavaScript:
//!
//! 1. `GET /werke/{id}/karte/einlesen` – Formular (Datei `.mm` oder `.xmind`, wahlweise „unter Knoten …“).
//! 2. `POST /werke/{id}/karte/einlesen` (multipart) – liest die Datei ([`super::import::read`]) und zeigt die
//!    **Vorschau**; angelegt wird nichts. Der gelesene Baum steht als JSON in einem verborgenen Formularfeld – keine
//!    Datei im Datenordner.
//! 3. `POST /werke/{id}/karte/einlesen/anlegen` – Bestätigung: Der Baum aus dem Formularfeld ist wieder fremde
//!    Eingabe und wird erneut geprüft ([`super::import::check`]); dann legt [`crate::idea_map::tree::import`] an
//!    (Herkunft Mensch) und es geht mit 303 zur Mindmap.
//!
//! Ohne Zielknoten werden die eingelesenen Knoten **freie Hauptthemen am Werk** (`tree::import`); mit Zielknoten werden sie
//! dessen Unterknoten. Die Wurzel der fremden Datei wird nie selbst Einfall.

use std::sync::Arc;

use axum::extract::multipart::MultipartRejection;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Multipart, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use baruch_store::{IdeaId, IdeaOrigin, WorkId};
use minijinja::value::Serde;
use minijinja::{Value, context};
use serde::Deserialize;

use super::import::{self, MAX_UPLOAD_BYTES};
use super::outline::{href, templates};
use super::{ensure_in_work, load_tree};
use crate::AppState;
use crate::idea_map::MapFilter;
use crate::idea_map::bild::{plain, status_of};
use crate::idea_map::tree::{self, ImportTree, MAX_IMPORT_NODES, MAX_TREE_DEPTH, TreeKind};
use crate::rpc::INTERNAL;
use crate::view::Settings;

/// Größte Anfrage der Vorschau: die Datei und etwas Luft für den Formularrahmen.
pub const UPLOAD_LIMIT: usize = MAX_UPLOAD_BYTES + 64 * 1024;
/// Größte Anfrage der Bestätigung: der Baum als JSON, formularkodiert (höchstens [`import::MAX_TEXT_BYTES`] Text,
/// kodiert ein Mehrfaches).
pub const APPLY_LIMIT: usize = 32 * 1024 * 1024;

/// Was die Seite zeigt.
enum Mode<'a> {
    Form,
    Preview(&'a ImportTree),
}

fn cut(text: &str, max: usize) -> String {
    let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= max {
        return one;
    }
    let mut short: String = one.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

/// Die Seite „Mindmap einlesen“ (Formular oder Vorschau; auch als Fehlerseite).
async fn show(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    status: StatusCode,
    mode: Mode<'_>,
    parent: Option<IdeaId>,
    error: Option<String>,
) -> Response {
    // Ohne Nähe: Für die Auswahl „unter Knoten“ zählt nur der Baum.
    let (_, scene) = match load_tree(state, work, &MapFilter::default()).await {
        Ok(loaded) => loaded,
        Err(response) => return response,
    };
    let map = &scene.map;
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let karte = format!("/werke/{work}/karte");
    let parents: Vec<Value> = map
        .walk()
        .into_iter()
        .filter(|(n, _)| n.kind == TreeKind::Idea)
        .filter_map(|(n, depth)| {
            let id = n.idea?;
            let label = format!("{}{} – {}", "– ".repeat(depth.saturating_sub(1).min(12)), id, cut(&n.title, 70));
            Some(context! { id => id, label => label, selected => parent == Some(id) })
        })
        .collect();
    let parent_label = parent.and_then(|id| map.find(&format!("i:{id}"))).map(|n| cut(&n.title, 80));
    let preview = match mode {
        Mode::Form => None,
        Mode::Preview(tree) => Some(context! {
            title => tree.title.as_str(),
            count => import::count(&tree.nodes),
            links => tree.links.len(),
            warnings => tree.warnings.clone(),
            nodes => Value::from(Serde(&tree.nodes)),
            json => serde_json::to_string(tree).unwrap_or_default(),
        }),
    };
    let ctx = context! {
        settings => Value::from(Serde(settings)),
        work => context! { id => work, title => map.title.as_str() },
        title => map.title.as_str(),
        mode => if preview.is_some() { "preview" } else { "form" },
        preview => preview,
        parents => parents,
        parent => parent,
        parent_label => parent_label,
        error => error,
        max_mib => MAX_UPLOAD_BYTES / (1024 * 1024),
        max_nodes => MAX_IMPORT_NODES,
        max_depth => MAX_TREE_DEPTH,
        form_href => href(format!("{karte}/einlesen")),
        apply_href => href(format!("{karte}/einlesen/anlegen")),
        map_href => href(karte.clone()),
        outline_href => href(format!("{karte}/gliederung")),
        workspace => href(format!("/?work={work}")),
    };
    match templates().get_template("karte_einlesen.html").and_then(|t| t.render(ctx)) {
        Ok(html) => (status, Html(html)).into_response(),
        Err(e) => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: Vorlage karte_einlesen.html: {e}")),
    }
}

/// „unter Knoten“ aus einem Formularfeld: leer → keiner (Hauptthemen am Werk), `12` oder `i:12` → Einfall 12.
fn parent_of(value: &str) -> Result<Option<IdeaId>, String> {
    let value = value.trim();
    let value = value.strip_prefix("i:").unwrap_or(value);
    if value.is_empty() {
        return Ok(None);
    }
    value.parse().map(Some).map_err(|_| "Der gewählte Knoten ist kein Einfall.".to_owned())
}

/// `GET /werke/{id}/karte/einlesen` (wahlweise `?unter=i:<id>` als Vorauswahl).
pub async fn form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let parent = pairs.iter().rev().find(|(n, _)| n == "unter").and_then(|(_, v)| parent_of(v).ok().flatten());
    show(&state, &headers, work, StatusCode::OK, Mode::Form, parent, None).await
}

/// `POST /werke/{id}/karte/einlesen` (multipart: `file`, wahlweise `parent`): Vorschau, legt nichts an.
pub async fn preview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let fail =
        |status: StatusCode, message: String| show(&state, &headers, work, status, Mode::Form, None, Some(message));
    let mut multipart = match multipart {
        Ok(m) => m,
        Err(e) => {
            return fail(unprocessable, format!("Das Formular kam nicht als Datei-Upload an ({}).", e.body_text()))
                .await;
        }
    };
    let mut file: Option<Vec<u8>> = None;
    let mut parent_field = String::new();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) if e.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                return fail(StatusCode::PAYLOAD_TOO_LARGE, import::too_large()).await;
            }
            Err(e) => {
                return fail(unprocessable, format!("Das Formular ließ sich nicht lesen ({}).", e.body_text())).await;
            }
        };
        match field.name().unwrap_or("") {
            "file" => match field.bytes().await {
                Ok(bytes) => file = Some(bytes.to_vec()),
                Err(e) if e.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                    return fail(StatusCode::PAYLOAD_TOO_LARGE, import::too_large()).await;
                }
                Err(e) => {
                    return fail(unprocessable, format!("Die Datei ließ sich nicht lesen ({}).", e.body_text())).await;
                }
            },
            "parent" => parent_field = field.text().await.unwrap_or_default(),
            _ => {}
        }
    }
    let Some(bytes) = file.filter(|b| !b.is_empty()) else {
        return fail(unprocessable, "Bitte eine Datei wählen (.mm oder .xmind).".to_owned()).await;
    };
    if bytes.len() > MAX_UPLOAD_BYTES {
        return fail(StatusCode::PAYLOAD_TOO_LARGE, import::too_large()).await;
    }
    let parent = match parent_of(&parent_field) {
        Ok(parent) => parent,
        Err(message) => return fail(unprocessable, message).await,
    };
    if let Some(parent) = parent
        && let Err(e) = ensure_in_work(&state, parent, work).await
    {
        return fail(status_of(&e), e.message).await;
    }
    // Lesen und Entpacken rechnen; nicht auf dem Laufzeit-Thread.
    let read = tokio::task::spawn_blocking(move || import::read(&bytes)).await;
    match read {
        Ok(Ok(tree)) => show(&state, &headers, work, StatusCode::OK, Mode::Preview(&tree), parent, None).await,
        Ok(Err(message)) => fail(unprocessable, message).await,
        Err(_) => fail(unprocessable, "Die Datei ließ sich nicht lesen.".to_owned()).await,
    }
}

#[derive(Deserialize)]
pub struct ApplyForm {
    #[serde(default)]
    tree: String,
    #[serde(default)]
    parent: String,
}

/// `POST /werke/{id}/karte/einlesen/anlegen` (urlencoded: `tree` = JSON aus der Vorschau, `parent`): legt an.
pub async fn apply(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    form: Result<Form<ApplyForm>, FormRejection>,
) -> Response {
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let again = "Bitte die Datei noch einmal einlesen.";
    let Form(form) = match form {
        Ok(form) => form,
        Err(e) => {
            let (status, message) = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                (StatusCode::PAYLOAD_TOO_LARGE, format!("Die Vorschau ist zu groß geworden. {again}"))
            } else {
                (unprocessable, format!("Das Formular ließ sich nicht lesen. {again}"))
            };
            return show(&state, &headers, work, status, Mode::Form, None, Some(message)).await;
        }
    };
    let tree = serde_json::from_str::<ImportTree>(&form.tree)
        .map_err(|_| format!("Die Vorschau ist beschädigt oder fehlt. {again}"))
        .and_then(import::check);
    let tree = match tree {
        Ok(tree) => tree,
        Err(message) => return show(&state, &headers, work, unprocessable, Mode::Form, None, Some(message)).await,
    };
    let parent = match parent_of(&form.parent) {
        Ok(parent) => parent,
        Err(message) => {
            return show(&state, &headers, work, unprocessable, Mode::Preview(&tree), None, Some(message)).await;
        }
    };
    if let Some(parent) = parent
        && let Err(e) = ensure_in_work(&state, parent, work).await
    {
        return show(&state, &headers, work, status_of(&e), Mode::Preview(&tree), None, Some(e.message)).await;
    }
    let origin = IdeaOrigin::Human { name: crate::mirror::HUMAN_NAME.to_owned() };
    match tree::import(&state, work, parent, &tree, &origin).await {
        Ok(_) => Redirect::to(&format!("/werke/{work}/karte?ok=eingelesen")).into_response(),
        Err(e) if e.code == INTERNAL => plain(StatusCode::INTERNAL_SERVER_ERROR, format!("Fehler: {}", e.message)),
        // Abgelehnt (im S1-Stand: „noch nicht umgesetzt“): die Vorschau bleibt stehen, mit der Meldung.
        Err(e) => show(&state, &headers, work, status_of(&e), Mode::Preview(&tree), parent, Some(e.message)).await,
    }
}
