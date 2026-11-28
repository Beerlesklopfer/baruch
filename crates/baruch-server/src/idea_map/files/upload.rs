// SPDX-License-Identifier: AGPL-3.0-or-later
//! Importing a foreign mind map through the page (contract Round 34 §5/§8, owner mindmap_austausch), entirely without
//! JavaScript:
//!
//! 1. `GET /werke/{id}/karte/einlesen` – form (file `.mm` or `.xmind`, optionally „unter Knoten …“).
//! 2. `POST /werke/{id}/karte/einlesen` (multipart) – reads the file ([`super::import::read`]) and shows the
//!    **preview**; nothing is created. The tree read stands as JSON in a hidden form field – no
//!    file in the data folder.
//! 3. `POST /werke/{id}/karte/einlesen/anlegen` – confirmation: the tree from the form field is foreign
//!    input again and is checked anew ([`super::import::check`]); then [`crate::idea_map::tree::import`] creates
//!    (origin human) and a 303 leads to the mind map.
//!
//! Without a target node the imported nodes become **free main topics on the work** (`tree::import`); with a target
//! node they become its sub-nodes. The root of the foreign file never becomes an idea itself.

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

/// Largest request of the preview: the file and some air for the form framing.
pub const UPLOAD_LIMIT: usize = MAX_UPLOAD_BYTES + 64 * 1024;
/// Largest request of the confirmation: the tree as JSON, form-encoded (at most [`import::MAX_TEXT_BYTES`] text,
/// encoded a multiple of that).
pub const APPLY_LIMIT: usize = 32 * 1024 * 1024;

/// What the page shows.
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

/// The page „Mindmap einlesen“ (form or preview; also as error page).
async fn show(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    status: StatusCode,
    mode: Mode<'_>,
    parent: Option<IdeaId>,
    error: Option<String>,
) -> Response {
    // Without nearness: for the selection „unter Knoten“ only the tree counts.
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

/// „unter Knoten“ from a form field: empty → none (main topics on the work), `12` or `i:12` → idea 12.
fn parent_of(value: &str) -> Result<Option<IdeaId>, String> {
    let value = value.trim();
    let value = value.strip_prefix("i:").unwrap_or(value);
    if value.is_empty() {
        return Ok(None);
    }
    value.parse().map(Some).map_err(|_| "Der gewählte Knoten ist kein Einfall.".to_owned())
}

/// `GET /werke/{id}/karte/einlesen` (optionally `?unter=i:<id>` as preselection).
pub async fn form(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(work): Path<WorkId>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let parent = pairs.iter().rev().find(|(n, _)| n == "unter").and_then(|(_, v)| parent_of(v).ok().flatten());
    show(&state, &headers, work, StatusCode::OK, Mode::Form, parent, None).await
}

/// `POST /werke/{id}/karte/einlesen` (multipart: `file`, optionally `parent`): preview, creates nothing.
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
    // Reading and unpacking compute; not on the runtime thread.
    let read = tokio::task::spawn_blocking(crate::i18n::carry(move || import::read(&bytes))).await;
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

/// `POST /werke/{id}/karte/einlesen/anlegen` (urlencoded: `tree` = JSON from the preview, `parent`): creates.
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
        // Refused (in the S1 state: “not implemented yet”): the preview stays, with the message.
        Err(e) => show(&state, &headers, work, status_of(&e), Mode::Preview(&tree), parent, Some(e.message)).await,
    }
}
