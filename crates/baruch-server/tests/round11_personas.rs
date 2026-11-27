// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 11 („images“): Einstellungen → Assistenten with one tab per persona (without JavaScript links
//! `?persona=<handle>`), one form with exactly one „Speichern“ per persona (`action=save`), „Vorgabe wiederherstellen“
//! with a confirmation prompt, `persona.update` with name; plus the completion of „@…“ in the section chat
//! (`<datalist>` without JavaScript, widget `mention`). Real data folder in the temp folder, no model backend
//! (`AppState::open` has `NoModel`), no network.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Form like a browser (urlencoded); response with status, redirect and page.
async fn form(server: &common::Server, body: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::post("/einstellungen/personas/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let (status, headers, page) = server.send(request).await;
    let location = headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned());
    (status, location, page)
}

/// The panel (`role="tabpanel"`) of a persona as an excerpt up to the next panel.
fn panel<'a>(page: &'a str, anchor: &str) -> &'a str {
    let start = page.find(&format!("role=\"tabpanel\" id=\"persona-{anchor}\"")).unwrap_or_else(|| {
        panic!("Tafel persona-{anchor} fehlt: {page}");
    });
    let rest = &page[start..];
    let end = rest[1..].find("role=\"tabpanel\"").map_or(rest.len(), |e| e + 1);
    &rest[..end]
}

/// Whole opening tag of the element in which `marker` is.
fn tag_with<'a>(html: &'a str, marker: &str) -> &'a str {
    let at = html.find(marker).unwrap_or_else(|| panic!("{marker} fehlt: {html}"));
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    &html[start..=end]
}

#[tokio::test]
async fn every_persona_has_a_tab_link_and_only_the_chosen_panel_is_shown_without_javascript() {
    let server = common::open().await;
    let (status, page) = server.get("/einstellungen/assistenten?work=1").await;
    assert_eq!(status, StatusCode::OK, "Seite lädt: {page}");
    assert!(page.contains("role=\"tablist\" aria-label=\"Personas\""), "Tab-Liste mit ARIA-Rolle: {page}");
    assert!(page.contains("data-widget=\"persona-tabs\""), "Widget für das Umschalten ohne Neuladen");
    let personas = result(&server.rpc("persona.list", json!({})).await)["personas"].as_array().unwrap().clone();
    assert!(personas.len() >= 6, "Beispiel-Personas vorhanden: {personas:?}");
    for p in &personas {
        let anchor = p["handle"].as_str().unwrap().trim_start_matches('@');
        assert!(
            page.contains(&format!("href=\"/einstellungen/assistenten?persona={anchor}&amp;work=1\"")),
            "{anchor}: Tab ist ein Link mit ?persona= und behält das Werk (ohne JavaScript): {page}"
        );
        assert!(page.contains(&format!("aria-controls=\"persona-{anchor}\"")), "{anchor}: Tab nennt seine Tafel");
    }
    let first = personas[0]["handle"].as_str().unwrap().trim_start_matches('@').to_owned();
    let tab = tag_with(&page, &format!("id=\"tab-{first}\""));
    assert!(tab.contains("aria-selected=\"true\"") && tab.contains("tabindex=\"0\""), "ohne Wahl der erste: {tab}");
    assert!(!tag_with(&page, &format!("id=\"persona-{first}\"")).contains(" hidden"), "seine Tafel ist sichtbar");
    assert!(
        tag_with(&page, "id=\"persona-nathan\"").contains(" hidden"),
        "die übrigen Tafeln stehen im HTML, aber verborgen (für das Umschalten ohne Neuladen)"
    );

    let (_, page) = server.get("/einstellungen/assistenten?persona=nathan").await;
    let nathan_tab = tag_with(&page, "id=\"tab-nathan\"");
    assert!(nathan_tab.contains("aria-selected=\"true\""), "?persona=nathan wählt Nathan: {nathan_tab}");
    assert!(nathan_tab.contains("style=\"--pc-light:#"), "Tab trägt die Farbe der Persona: {nathan_tab}");
    let open = tag_with(&page, "id=\"persona-nathan\"");
    assert!(!open.contains(" hidden"), "Nathans Tafel ist offen: {open}");
    assert!(tag_with(&page, &format!("id=\"persona-{first}\"")).contains(" hidden"), "die erste ist jetzt verborgen");
    let (_, fragment) = server.get("/einstellungen/assistenten?persona=nathan&fragment=1").await;
    assert!(
        fragment.starts_with("<div class=\"settings-frame\"")
            && tag_with(&fragment, "id=\"tab-nathan\"").contains("aria-selected=\"true\""),
        "auch als Fragment fürs Popup mit dem gewählten Tab: {fragment}"
    );
}

#[tokio::test]
async fn the_tab_holds_every_field_with_big_text_areas_and_exactly_one_save_button() {
    let server = common::open().await;
    let (_, page) = server.get("/einstellungen/assistenten?persona=nathan").await;
    let nathan = panel(&page, "nathan");
    for (marker, why) in [
        ("type=\"color\" name=\"color\"", "Farbe"),
        ("name=\"name\" value=\"Nathan\"", "Name"),
        ("value=\"@nathan\" readonly", "Kürzel sichtbar, aber fest"),
        ("<textarea name=\"role\"", "Rolle als mehrzeiliges Textfeld"),
        ("name=\"model\" value=\"gpt-4.1-mini\"", "Modell"),
        ("name=\"context_length\" value=\"16384\"", "Kontext"),
        ("name=\"active\" value=\"1\" checked", "aktiv"),
        ("Vorgabe wiederherstellen", "Zurücksetzen des Prompts"),
    ] {
        assert!(nathan.contains(marker), "{why} fehlt in Nathans Tafel: {nathan}");
    }
    let prompt = tag_with(nathan, "class=\"persona-prompt-text\"");
    let rows: u32 = prompt
        .split("rows=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .and_then(|r| r.parse().ok())
        .expect("rows am Prompt-Feld");
    assert!(rows >= 20, "Rollen-Prompt als großes Textfeld (mindestens 20 Zeilen): {prompt}");
    assert!(prompt.starts_with("<textarea") && prompt.contains("name=\"prompt\""), "Prompt ist ein textarea: {prompt}");
    assert_eq!(
        nathan.matches("name=\"action\" value=\"save\"").count(),
        1,
        "genau ein Speichern-Knopf je Persona: {nathan}"
    );
    assert!(!nathan.contains("persona.set_color") && !nathan.contains("persona.update"), "keine Einzelformulare mehr");
    let (_, css) = server.get("/static/app.css").await;
    assert!(
        css.contains(".persona-prompt-text {\n  font-family: ui-monospace"),
        "Prompt in Monospace (settings.css im Stylesheet): {css}"
    );
    assert!(css.contains(".persona-wide textarea { width: 100%;"), "Textfelder in voller Breite");
    assert!(css.contains("resize: vertical"), "vergrößerbar");
    assert!(css.contains("width: min(1480px, calc(100vw - 32px))"), "das Popup nutzt die Breite des Fensters");
}

#[tokio::test]
async fn one_save_changes_name_role_model_context_active_color_and_prompt_together() {
    let server = common::open().await;
    let body = "action=save&handle=%40nathan&work=1&name=Nathan+der+Weise&role=Pr%C3%BCft+jede+Zeile%0AZweite+Zeile\
                &model=probe-9&context_length=65536&color=%23123456&color_text=\
                &prompt=Du+bist+Nathan.%0APr%C3%BCfe+streng.";
    let (status, location, page) = form(&server, body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert → 303: {page}");
    assert_eq!(
        location.as_deref(),
        Some("/einstellungen/assistenten?persona=nathan&ok=persona&work=1#persona-nathan"),
        "zurück in Nathans Tab mit Hinweis"
    );
    let list = result(&server.rpc("persona.list", json!({})).await).clone();
    let nathan = list["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@nathan").unwrap().clone();
    assert_eq!(nathan["name"], "Nathan der Weise", "Name gespeichert: {nathan}");
    assert_eq!(nathan["role"], "Prüft jede Zeile\nZweite Zeile", "Rolle mehrzeilig gespeichert");
    assert_eq!(nathan["model"], "probe-9", "Modell gespeichert");
    assert_eq!(nathan["context_length"], 65536, "Kontext gespeichert");
    assert_eq!(nathan["active"], false, "Kästchen „aktiv“ fehlte → inaktiv");
    assert_eq!((nathan["color"].as_str(), nathan["chosen"].as_bool()), (Some("#123456"), Some(true)), "Farbe");
    let prompt = result(&server.rpc("persona.prompt_get", json!({ "handle": "@nathan" })).await).clone();
    assert_eq!(prompt["prompt"], "Du bist Nathan.\nPrüfe streng.", "eigener Rollen-Prompt gespeichert");
    assert_eq!(prompt["custom"], true, "als eigener markiert");
    let (_, page) = server.get("/einstellungen/assistenten?persona=nathan&ok=persona").await;
    assert!(page.contains("Die Persona ist gespeichert."), "Hinweis nach dem Speichern");
    assert!(page.contains("Nathan der Weise"), "neuer Name im Tab: {page}");
}

#[tokio::test]
async fn saving_the_shown_default_prompt_keeps_the_default_and_an_unchanged_color_stays_default() {
    let server = common::open().await;
    let before = result(&server.rpc("persona.prompt_get", json!({ "handle": "@exeget" })).await).clone();
    let list = result(&server.rpc("persona.list", json!({})).await).clone();
    let exeget = list["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@exeget").unwrap().clone();
    let encode = |s: &str| {
        s.bytes()
            .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
            .collect::<String>()
    };
    let body = format!(
        "action=save&handle=%40exeget&name=Exeget&role={}&model={}&context_length=8192&active=1&color={}&prompt={}",
        encode(exeget["role"].as_str().unwrap()),
        encode(exeget["model"].as_str().unwrap()),
        encode(exeget["color"].as_str().unwrap()),
        encode(before["prompt"].as_str().unwrap()),
    );
    let (status, _, page) = form(&server, &body).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "unverändert gespeichert: {page}");
    let after = result(&server.rpc("persona.prompt_get", json!({ "handle": "@exeget" })).await).clone();
    assert_eq!(after["custom"], false, "die gezeigte Vorgabe wird durch Speichern kein eigener Prompt");
    let list = result(&server.rpc("persona.list", json!({})).await).clone();
    let exeget = list["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@exeget").unwrap().clone();
    assert_eq!(exeget["chosen"], false, "die Farbwahl schickt immer einen Wert; unverändert bleibt die Vorgabe");
}

#[tokio::test]
async fn a_rejected_save_shows_the_tab_with_the_input_and_changes_nothing() {
    let server = common::open().await;
    let body = "action=save&handle=%40nathan&name=Nathan&role=Neue+Rolle&model=m&context_length=0&active=1\
                &color=%23111111&prompt=Behalte+mich";
    let (status, _, page) = form(&server, body).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Kontext 0 → 422");
    assert!(page.contains("Die Kontextlänge muss zwischen 1 und 10000000 Tokens liegen."), "Meldung: {page}");
    assert!(tag_with(&page, "id=\"tab-nathan\"").contains("aria-selected=\"true\""), "Nathans Tab bleibt gewählt");
    let nathan = panel(&page, "nathan");
    assert!(nathan.contains("Neue Rolle") && nathan.contains("Behalte mich"), "Eingabe bleibt erhalten: {nathan}");
    let list = result(&server.rpc("persona.list", json!({})).await).clone();
    let stored = list["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@nathan").unwrap().clone();
    assert_eq!(
        stored["role"], "Auditor: nur Befunde, nie Freigabe",
        "nichts gespeichert (erst prüfen, dann schreiben)"
    );
    assert_eq!(stored["chosen"], false, "auch die Farbe nicht");

    let (status, _, page) = form(&server, "action=save&handle=%40nathan&name=+&role=r&model=m&context_length=5").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "leerer Name → 422");
    assert!(page.contains("Der Name darf nicht leer sein."), "Meldung: {page}");
    let (status, _, _) = form(&server, "action=loeschen&handle=%40nathan").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Aktion");
}

#[tokio::test]
async fn restoring_the_default_prompt_asks_first_and_works_only_with_confirm() {
    let server = common::open().await;
    let set = server.rpc("persona.set_prompt", json!({ "handle": "@hoerer", "prompt": "Eigener Text" })).await;
    assert_eq!(result(&set)["custom"], true, "eigener Prompt gesetzt");
    let (status, location, _) = form(&server, "action=reset_prompt&handle=%40hoerer&work=1").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne confirm nur Weiterleitung zur Rückfrage");
    assert_eq!(
        location.as_deref(),
        Some("/einstellungen/assistenten?persona=hoerer&rueckfrage=vorgabe&work=1#persona-hoerer"),
        "Rückfrage im Tab des Hörers"
    );
    let still = result(&server.rpc("persona.prompt_get", json!({ "handle": "@hoerer" })).await).clone();
    assert_eq!(still["custom"], true, "ohne confirm nichts zurückgesetzt");
    let (_, page) = server.get("/einstellungen/assistenten?persona=hoerer&rueckfrage=vorgabe").await;
    let question = tag_with(&page, "class=\"persona-confirm");
    assert!(question.contains("role=\"alertdialog\""), "Rückfrage als Dialog: {question}");
    assert!(panel(&page, "hoerer").contains("name=\"confirm\" value=\"1\""), "die Rückfrage schickt confirm=1: {page}");
    let (status, location, _) = form(&server, "action=reset_prompt&handle=%40hoerer&confirm=1").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "mit confirm wirksam");
    assert_eq!(location.as_deref(), Some("/einstellungen/assistenten?persona=hoerer&ok=vorgabe#persona-hoerer"));
    let reset = result(&server.rpc("persona.prompt_get", json!({ "handle": "@hoerer" })).await).clone();
    assert_eq!(reset["custom"], false, "wieder die Vorgabe");
}

/// Follow-up fix Round 11 (review „plan“): since the tabs there was no way back to the colour of the palette.
#[tokio::test]
async fn the_tab_resets_a_chosen_color_to_the_palette() {
    let server = common::open().await;
    let (_, page) = server.get("/einstellungen/assistenten?persona=hoerer").await;
    let button = tag_with(panel(&page, "hoerer"), "value=\"reset_color\"");
    assert!(button.contains("disabled"), "ohne eigene Farbe nichts zurückzusetzen: {button}");
    result(&server.rpc("persona.set_color", json!({ "handle": "@hoerer", "color": "#123456" })).await);
    let (_, page) = server.get("/einstellungen/assistenten?persona=hoerer").await;
    let button = tag_with(panel(&page, "hoerer"), "value=\"reset_color\"");
    assert!(!button.contains("disabled"), "mit eigener Farbe bedienbar: {button}");
    let (status, location, _) = form(&server, "action=reset_color&handle=%40hoerer&work=1").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "zurückgesetzt → 303");
    assert_eq!(
        location.as_deref(),
        Some("/einstellungen/assistenten?persona=hoerer&ok=farbe&work=1#persona-hoerer"),
        "zurück in den Tab des Hörers"
    );
    let list = result(&server.rpc("persona.list", json!({})).await).clone();
    let hoerer = list["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@hoerer").unwrap().clone();
    assert_eq!(hoerer["chosen"], false, "wieder die Farbe der Palette: {hoerer}");
}

#[tokio::test]
async fn an_unknown_persona_shows_the_first_tab_with_a_hint() {
    let server = common::open().await;
    let (status, page) = server.get("/einstellungen/assistenten?persona=niemand").await;
    assert_eq!(status, StatusCode::OK, "kein Fehler, nur ein Hinweis");
    assert!(page.contains("Persona „niemand“ gibt es nicht; gezeigt wird die erste."), "Hinweis: {page}");
    assert!(tag_with(&page, "id=\"tab-lektor\"").contains("aria-selected=\"true\""), "der erste Tab (Lektor)");
}

#[tokio::test]
async fn persona_update_over_rpc_takes_a_trimmed_name_of_one_to_eighty_chars() {
    let server = common::open().await;
    let renamed = server.rpc("persona.update", json!({ "handle": "@belege", "name": "  Quellenprüfer " })).await;
    assert_eq!(result(&renamed)["name"], "Quellenprüfer", "Name getrimmt gespeichert: {renamed}");
    for (name, why) in [(json!("   "), "leer"), (json!("x".repeat(81)), "81 Zeichen"), (json!(7), "keine Zeichenkette")]
    {
        let refused = server.rpc("persona.update", json!({ "handle": "@belege", "name": name })).await;
        assert_eq!(refused["error"]["code"], -32602, "{why} → -32602: {refused}");
    }
    let list = result(&server.rpc("persona.list", json!({})).await).clone();
    let belege = list["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@belege").unwrap().clone();
    assert_eq!(belege["name"], "Quellenprüfer", "abgelehnte Namen ändern nichts");
}

#[tokio::test]
async fn the_chat_offers_the_active_personas_as_datalist_and_the_mention_widget() {
    let server = common::open().await;
    server.rpc("persona.update", json!({ "handle": "@hoerer", "active": false })).await;
    let (_, page) = server.get("/?work=1").await;
    let input = tag_with(&page, "name=\"message\"");
    assert!(input.contains("data-widget=\"mention\""), "Widget für die Vervollständigung: {input}");
    let list_id = input.split("list=\"").nth(1).and_then(|r| r.split('"').next()).expect("list-Attribut");
    let start = page.find(&format!("<datalist id=\"{list_id}\">")).expect("datalist im Server-HTML (ohne JavaScript)");
    let datalist = &page[start..start + page[start..].find("</datalist>").unwrap()];
    assert!(
        datalist.contains(
            "<option value=\"@nathan\" data-name=\"Nathan\" data-role=\"Auditor: nur Befunde, nie Freigabe\""
        ),
        "aktive Persona mit Name und Rolle: {datalist}"
    );
    assert!(datalist.contains("data-style=\"--pc-light:#"), "mit ihrer Farbe für die Liste");
    assert!(!datalist.contains("@hoerer"), "inaktive Personas fehlen: {datalist}");
    let (status, js) = server.get("/static/js/mention.js").await;
    assert_eq!(status, StatusCode::OK, "Widget ausgeliefert");
    for marker in [
        "\"combobox\"",
        "\"listbox\"",
        "aria-activedescendant",
        "case \"ArrowDown\"",
        "case \"Tab\"",
        "case \"Escape\"",
        "input.removeAttribute(\"list\")",
    ] {
        assert!(js.contains(marker), "mention.js: {marker} fehlt");
    }
    let (_, tabs) = server.get("/static/js/persona_tabs.js").await;
    assert!(
        tabs.contains("history.replaceState") && tabs.contains("aria-selected") && tabs.contains("ArrowRight"),
        "persona_tabs.js schaltet um, setzt die Adresse und kennt die Pfeiltasten"
    );
}
