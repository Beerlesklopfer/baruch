// SPDX-License-Identifier: AGPL-3.0-or-later
//! Prüfanzeige der Zitate (Runde 17c): `GET /werke/{id}/pruefung?key=K[&rev=N]` zeigt das Ergebnis der
//! Wortlautprüfung eines Abschnitts **ohne JavaScript** als Liste – je Stelle Stellenangabe, Zitat, Ergebnis
//! („stimmt“, „weicht ab“, „ohne Zitat“, „ungeprüft“, „nicht prüfbar“), Unterschiede, die gelesene Stelle in
//! der Zählung des Moduls, Versifikation und Lizenzvermerk der conf; seit Runde 19c „Wortlaut stimmt (Leerraum im
//! Modul)“ bei Trennresten und der Hinweis auf ein Modul mit auffällig vielen Wortfragmenten. Dieselben Daten wie RPC `bible.marks`
//! ([`crate::bibles::marks_with`]). Mit JavaScript markiert das Widget `scripture-check`
//! (`web/src/scripture_check.ts`) dieselben Ergebnisse im Satzbild über die CSS Custom Highlight API; der Link
//! „Zitate prüfen“ in der Statuszeile des Editors führt ohne JavaScript hierher. Nur lesend.

use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use baruch_store::WorkId;
use minijinja::Environment;
use minijinja::value::{Serde, Value as JinjaValue};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::bibles::{marks_with, state_label};
use crate::view::Settings;

/// Routen der Seite (gemischt in `lib.rs`).
pub fn pages() -> Router<Arc<AppState>> {
    Router::new().route("/werke/{id}/pruefung", get(check_page))
}

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("scripture_check.html", include_str!("../templates/scripture_check.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Prüfanzeige fehlerhaft");
        }
        env
    })
}

/// Abfrage der Seite: `key` (fester Schlüssel des Abschnitts), `rev` (Nummer der Fassung, Vorgabe aktuelle).
#[derive(Debug, Default, Deserialize)]
pub struct CheckQuery {
    pub key: Option<String>,
    pub rev: Option<String>,
}

fn error_page(status: StatusCode, message: &str) -> Response {
    (
        status,
        Html(format!(
            "<!doctype html><meta charset=\"utf-8\"><title>Zitate prüfen</title><p>{}</p>",
            crate::bibles::html_escape(message)
        )),
    )
        .into_response()
}

/// Markierungen für die Vorlage: je Eintrag dazu `state_label`.
fn rows(result: &Value) -> Vec<Value> {
    result["marks"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let mut row = m.clone();
            row["state_label"] = json!(state_label(m["state"].as_str().unwrap_or("")));
            row
        })
        .collect()
}

/// `GET /werke/{id}/pruefung?key=K[&rev=N]`.
pub async fn check_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<WorkId>,
    Query(query): Query<CheckQuery>,
) -> Response {
    let work_title = match state.store.works().await {
        Ok(list) => match list.into_iter().find(|(w, _, _)| *w == id) {
            Some((_, _, title)) => title,
            None => return error_page(StatusCode::NOT_FOUND, "Unbekanntes Werk."),
        },
        Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Fehler: {e}")),
    };
    let Some(key) = query.key.as_deref().and_then(|k| k.trim().parse::<i64>().ok()) else {
        return error_page(StatusCode::BAD_REQUEST, "Bitte einen Abschnitt angeben (key = fester Schlüssel).");
    };
    let rev = match query.rev.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        None => None,
        Some(r) => match r.parse::<u32>() {
            Ok(n) => Some(n),
            Err(_) => return error_page(StatusCode::BAD_REQUEST, "rev: Nummer der Fassung erwartet."),
        },
    };
    let (title, number, body) = match crate::bibles::shown_text(&state, id, key, rev).await {
        Ok(found) => found,
        Err(e) => return error_page(StatusCode::NOT_FOUND, &e.message),
    };
    let choice = match crate::bibles::of_work(&state, id).await {
        Ok(choice) => choice,
        Err(e) => return error_page(StatusCode::INTERNAL_SERVER_ERROR, &e.message),
    };
    let result = match marks_with(&choice, id, key, number, &title, &body) {
        Ok(result) => result,
        Err(message) => return error_page(StatusCode::UNPROCESSABLE_ENTITY, &message),
    };
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    let bible = &result["bible"];
    let back = format!("/?work={id}&key={key}&rev={number}");
    let ctx = minijinja::context! {
        settings => JinjaValue::from(Serde(settings)),
        work => minijinja::context! { id => id, title => work_title },
        key => key,
        rev => number,
        title => title,
        back => crate::typeset::href(back),
        settings_url => JinjaValue::from_safe_string("/einstellungen/quellen".to_owned()),
        bible => JinjaValue::from(Serde(bible.clone())),
        has_bible => choice.bible().is_some(),
        no_choice => matches!(choice, crate::bibles::Choice::None),
        notice => result["notice"].as_str().map(str::to_owned),
        marks => JinjaValue::from(Serde(rows(&result))),
        counts => JinjaValue::from(Serde(result["counts"].clone())),
        // Runde 19c: Hinweis auf auffällig viele Wortfragmente im Modul (Trennreste), sonst leer.
        module_note => result["module_note"].as_str().map(str::to_owned),
    };
    match templates().get_template("scripture_check.html").and_then(|t| t.render(ctx)) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_page(StatusCode::INTERNAL_SERVER_ERROR, &format!("Vorlage scripture_check.html: {e}")),
    }
}
