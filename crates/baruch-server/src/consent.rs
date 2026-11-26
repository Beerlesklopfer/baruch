// SPDX-License-Identifier: AGPL-3.0-or-later
//! Aktive Zustimmung zu Anträgen (Nutzerfrage 12, Entscheidung des Nutzers 2026-11-07: „es braucht eine
//! aktive Zustimmung (Popup)“).
//!
//! Legt ein Assistent einen Teil so an, dass vorhandene Kapitel unter ihn wandern, entsteht statt des
//! Teils ein Antrag ([`baruch_store::Store::outline_add_by_assistant`]). Solange ein solcher Antrag offen
//! ist, zeigt jede Seite, die `base.html` erweitert, eine Hinweisleiste mit Link zum Antrag (ohne
//! JavaScript bedienbar: dort Formular mit „Zustimmen“/„Ablehnen“) und einen Dialog mit Vorschau und
//! denselben Knöpfen, den das Widget `consent` (`web/src/consent.ts`) modal öffnet. Nichts geschieht ohne
//! Klick: Kein Zeitablauf und kein Schließen des Dialogs gilt als Zustimmung.
//!
//! Umsetzung: `base.html` trägt die Marke [`MARKER`]; die Schicht [`inject`] ersetzt sie in jeder
//! HTML-Antwort durch [`banner`] (leer, wenn nichts offen ist). So braucht keine der vielen
//! Template-Umgebungen eigene Daten dafür. Die Abfrage läuft nur, wenn die Marke in der Antwort steht.

use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::Response;
use minijinja::value::Serde;
use minijinja::{Environment, context};
use serde_json::{Value as Json, json};

use crate::AppState;

/// Platzhalter in `base.html` (direkt unter der Kopfzeile).
pub const MARKER: &str = "<!--baruch-consent-->";

/// Größte HTML-Antwort, in die die Hinweisleiste eingesetzt wird (größere bleiben unverändert).
const MAX_PAGE: usize = 32 * 1024 * 1024;

fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        env.add_template("consent.html", include_str!("../templates/consent.html"))
            .expect("Vorlage der Zustimmung fehlerhaft");
        env
    })
}

/// Hinweisleiste und Dialog für alle offenen Anträge, die Zustimmung brauchen; leer, wenn keiner offen
/// ist. Fehler (Datenbank, Vorlage) stehen auf stderr; die Seite kommt dann ohne Leiste – die Anträge
/// bleiben auf `/gliederung` sichtbar.
pub async fn banner(state: &AppState) -> String {
    match build(state).await {
        Ok(html) => html,
        Err(e) => {
            eprintln!("Zustimmung: {e}");
            String::new()
        }
    }
}

async fn build(state: &AppState) -> Result<String, String> {
    let requests = state.store.outline_requests_needing_consent().await.map_err(|e| e.to_string())?;
    if requests.is_empty() {
        return Ok(String::new());
    }
    let titles = state.store.works().await.map_err(|e| e.to_string())?;
    let mut shown: Vec<Json> = Vec::new();
    for request in &requests {
        let mut value = crate::outline::request_json(request);
        crate::outline::add_preview(state, request.work, request, &mut value).await.map_err(|e| e.message)?;
        let title = titles.iter().find(|(id, _, _)| *id == request.work).map_or("", |(_, _, t)| t.as_str());
        value["work_title"] = json!(title);
        shown.push(value);
    }
    let first = &requests[0];
    let ctx = context! {
        requests => minijinja::Value::from(Serde(&shown)),
        // Nur Ziffern und feste Zeichen: als sicherer Wert, damit `/` nicht maskiert wird.
        first_url => minijinja::Value::from_safe_string(format!("/gliederung?work={}#antrag-{}", first.work, first.id)),
    };
    templates()
        .get_template("consent.html")
        .and_then(|t| t.render(ctx))
        .map_err(|e| format!("Vorlage consent.html: {e}"))
}

/// Schicht über allen Routen: setzt in HTML-Antworten mit [`MARKER`] die Hinweisleiste ein (bzw. entfernt
/// die Marke) und füllt die Abzeichen der Kopfzeile (Runde 23a, `menu::fill_page`); seit Runde 33 setzt sie
/// dort auch den Zustand von Menüband und Laptop-Modus ein (`band::apply`) und merkt ihn als Cookie. Andere Antworten (JSON, PDF, Bilder, Fragmente ohne Marke) laufen unverändert durch.
pub async fn inject(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    // Runde 33 (Schnittstelle R33-S1): Zustand von Menüband und Laptop-Modus aus Cookie und Adresse, festgehalten,
    // bevor die Anfrage weitergereicht wird.
    let cookies = request.headers().get(header::COOKIE).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let query = request.uri().query().map(str::to_owned);
    let band = crate::band::BandState::from_request(cookies.as_deref(), query.as_deref());
    let laptop = crate::laptop::LaptopState::from_request(cookies.as_deref(), query.as_deref());
    let response = next.run(request).await;
    let html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if !html {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, MAX_PAGE).await {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Zustimmung: Seite nicht lesbar: {e}");
            parts.headers.remove(header::CONTENT_LENGTH);
            return Response::from_parts(parts, Body::from("Fehler: Seite zu groß"));
        }
    };
    let Some(text) = std::str::from_utf8(&bytes).ok().filter(|t| t.contains(MARKER)) else {
        return Response::from_parts(parts, Body::from(bytes));
    };
    let page = text.replacen(MARKER, &banner(&state).await, 1);
    // Runde 23a: Abzeichen der Kopfzeile außerhalb der Arbeitsseite serverseitig füllen (auch ohne JavaScript).
    let page = crate::menu::fill_page(&state, page).await;
    // Runde 33: aktiver Reiter, Adressen der Reiter und Laptop-Zustand im fertigen HTML; gültige Schlüssel der
    // Adresse als Cookie merken – anhängen, nicht ersetzen (`/` setzt schon `baruch_work`).
    let page = crate::band::apply(&page, &band, &laptop);
    // Runde 35 (Schnittstelle R35-S1): Konto-Eintrag im Menü „Baruch ▾“ und Rolle an <body>; die Abfrage läuft nur,
    // wenn die Marke in der Seite steht.
    let page = if page.contains(crate::konten_ui::KONTO_MARK) {
        let accounts_exist = crate::auth::accounts_exist(&state).await;
        crate::konten_ui::apply(&page, &crate::auth::current(), accounts_exist)
    } else {
        page
    };
    for cookie in
        crate::band::set_cookies(query.as_deref()).into_iter().chain(crate::laptop::set_cookies(query.as_deref()))
    {
        match HeaderValue::from_str(&cookie) {
            Ok(value) => {
                parts.headers.append(header::SET_COOKIE, value);
            }
            Err(e) => eprintln!("Menüband: Cookie nicht setzbar: {e}"),
        }
    }
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(page))
}
