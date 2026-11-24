// SPDX-License-Identifier: AGPL-3.0-or-later
//! Farben der Personas im Server (Vertrag Runde 7, „proposals“): RPC `persona.list`/`persona.set_color`,
//! Seite `/einstellungen/assistenten` (seit Runde 9; `/einstellungen/personas` leitet dorthin) mit
//! Formular (ohne JavaScript), Fehler -32602 bzw. 422, kein Weg über
//! `/act` und MCP, Kontrast der Tönungen (WCAG AA), der Linien (WCAG 1.4.11, Runde 8) und die Farbe
//! überall, wo die Persona auftritt (Assistentenliste, Abschnitts-Chat, Fassungsliste, Fassungskopf).

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::personas::{
    DARK_FOREGROUNDS, DARK_PANEL, DARK_SHEET, DARK_TEXT, LIGHT_FOREGROUNDS, LIGHT_PANEL, LIGHT_SURFACES, LIGHT_TEXT,
    MIN_CONTRAST, MIN_LINE_CONTRAST, contrast_ratio, style, tints,
};
use baruch_server::rpc::INVALID_PARAMS;
use baruch_store::{FALLBACK_COLOR, PERSONA_PALETTE};
use serde_json::{Value, json};

/// Formular an eine Adresse; Antwort Status, Weiterleitung, Text.
async fn form(server: &common::Server, uri: &str, body: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let (status, headers, text) = server.send(request).await;
    (status, headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()), text)
}

fn error_code(answer: &Value) -> Option<i64> {
    answer["error"]["code"].as_i64()
}

#[test]
fn every_tint_keeps_the_text_readable_in_both_themes() {
    let extremes = ["#000000", "#ffffff", "#ffff00", "#0000ff", FALLBACK_COLOR];
    for color in PERSONA_PALETTE.iter().chain(extremes.iter()) {
        let t = tints(color);
        let light = contrast_ratio(LIGHT_TEXT, &t.light);
        let dark = contrast_ratio(DARK_TEXT, &t.dark);
        assert!(light >= MIN_CONTRAST, "{color}: Text {LIGHT_TEXT} auf {} nur {light:.2}:1", t.light);
        assert!(dark >= MIN_CONTRAST, "{color}: Text {DARK_TEXT} auf {} nur {dark:.2}:1", t.dark);
        for value in [&t.light, &t.dark, &t.line, &t.line_dark] {
            assert!(
                value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit()),
                "{color}: Tönung {value} ist #rrggbb"
            );
        }
    }
    // Die Tönung bleibt erkennbar: nicht einfach die Fläche selbst, wenn die Farbe es zulässt.
    assert_ne!(tints(PERSONA_PALETTE[0]).light, "#ffffff", "Blau hell getönt, nicht weiß");
    assert_ne!(tints(PERSONA_PALETTE[0]).dark, "#1f2328", "Blau dunkel getönt, nicht die Fläche");
}

/// Runde 8 (WCAG 1.4.11, Nicht-Text-Kontrast): Die eingeklappte Linie eines Vorschlags ist oft das
/// Einzige, was von ihm zu sehen ist. Vorher war `--pc-line` die Farbe selbst – Gelb erreicht auf Weiß
/// nur 1,07:1, ein dunkles Braun auf der dunklen Fläche kaum mehr. Jetzt: `--pc-line` mindestens 3:1
/// gegen die helle Fläche, `--pc-line-dark` mindestens 3:1 gegen die dunkle, für die Palette und frei
/// erfundene Farben (Ocker, Braun, Magenta, Violett, dazu die Extreme).
#[test]
fn persona_lines_reach_three_to_one_against_the_panel_in_both_themes() {
    let invented = [
        ("Ocker", "#cc9a2e"),
        ("Braun", "#5b3a1e"),
        ("Magenta", "#e0109b"),
        ("Violett", "#7a3fd1"),
        ("Gelb", "#ffff00"),
        ("Weiß", "#ffffff"),
        ("Schwarz", "#000000"),
        ("Grau", FALLBACK_COLOR),
    ];
    let palette = PERSONA_PALETTE.iter().map(|c| ("Palette", *c));
    for (name, color) in palette.chain(invented) {
        let t = tints(color);
        // --pc-line steht im hellen Thema auf Weiß und auf dem Blatt in beiden Themen, im dunklen also
        // auf #f4f1ea (dunkler als Weiß): gegen jede dieser Flächen 3:1.
        for surface in LIGHT_SURFACES {
            let light = contrast_ratio(&t.line, surface);
            assert!(light >= MIN_LINE_CONTRAST, "{name} {color}: --pc-line {} auf {surface} nur {light:.2}:1", t.line);
        }
        let dark = contrast_ratio(&t.line_dark, DARK_PANEL);
        assert!(
            dark >= MIN_LINE_CONTRAST,
            "{name} {color}: --pc-line-dark {} auf {DARK_PANEL} nur {dark:.2}:1",
            t.line_dark
        );
        let s = style(color);
        assert!(
            s.contains(&format!(";--pc-line:{};", t.line)) && s.ends_with(&format!(";--pc-line-dark:{}", t.line_dark)),
            "{name}: beide Linien stehen im style: {s}"
        );
    }
    // Eine Farbe, die schon reicht, bleibt unverändert (die Persona bleibt an ihrer Farbe erkennbar).
    for color in
        PERSONA_PALETTE.iter().filter(|c| LIGHT_SURFACES.iter().all(|s| contrast_ratio(c, s) >= MIN_LINE_CONTRAST))
    {
        assert_eq!(tints(color).line, *color, "{color} reicht auf beiden hellen Flächen schon und bleibt die Linie");
    }
    // Der Befund aus Runde 8: Palettenfarbe #e0a800 wurde nur gegen Weiß gerechnet (#b38600, 3,32:1)
    // und hielt auf dem Blatt des dunklen Themas nur 2,94:1.
    assert!(contrast_ratio("#b38600", DARK_SHEET) < MIN_LINE_CONTRAST, "Ausgangslage: Weiß allein reicht nicht");
    assert!(contrast_ratio(&tints("#e0a800").line, DARK_SHEET) >= MIN_LINE_CONTRAST, "jetzt auch auf dem Blatt");
    assert_ne!(tints("#ffff00").line, "#ffff00", "Gelb wird für die helle Fläche abgedunkelt");
    assert_ne!(tints("#5b3a1e").line_dark, "#5b3a1e", "dunkles Braun wird für die dunkle Fläche aufgehellt");
    assert_eq!(MIN_LINE_CONTRAST, 3.0, "WCAG 1.4.11 verlangt 3:1");
    // proposals.css nimmt je Thema die passende Linie (und auf dem stets hellen Blatt die helle).
    let css = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/static/proposals.css")).unwrap();
    assert!(css.contains(".pc { --pc-bg: var(--pc-light, var(--bg)); --pc-edge: var(--pc-line, "), "hell: --pc-line");
    assert_eq!(css.matches("--pc-edge: var(--pc-line-dark, ").count(), 2, "dunkel (System und Wahl): --pc-line-dark");
    assert!(
        css.contains(".ws-sheet .pv.pc, .sheet .pv.pc {")
            && css.contains(
                "--pc-edge: var(--pc-line, var(--muted));
  color-scheme: light;"
            ),
        "auf dem Blatt die helle Linie: {css}"
    );
}

/// Nachbesserung Runde 7: Auf den Tönungen steht nicht nur `--text`, sondern auch gedämpfter Text
/// (`.muted`: Handle und „Vorgabe #…“ auf /einstellungen/personas, „Quelle: …“ im Chat) und Linktext
/// (`.ref` in `--primary`). Vorher erreichten etwa `--muted` auf Magenta hell nur 4,09:1 und
/// `--primary` auf Ocker dunkel nur 3,16:1.
#[test]
fn muted_and_link_text_stay_readable_on_every_tint() {
    let extremes = ["#000000", "#ffffff", "#ffff00", "#0000ff", "#ff00ff", FALLBACK_COLOR];
    for color in PERSONA_PALETTE.iter().chain(extremes.iter()) {
        let t = tints(color);
        for (theme, background, foregrounds) in
            [("hell", &t.light, LIGHT_FOREGROUNDS), ("dunkel", &t.dark, DARK_FOREGROUNDS)]
        {
            for fg in foregrounds {
                let ratio = contrast_ratio(fg, background);
                assert!(ratio >= MIN_CONTRAST, "{color} {theme}: Schrift {fg} auf {background} nur {ratio:.2}:1");
            }
        }
    }
    // Die geprüften Schriftfarben sind die von app.css (sonst prüfte der Test die falschen Farben):
    // jede Variable steht im hellen bzw. in beiden dunklen Blöcken mit genau diesem Wert.
    let css = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/static/app.css")).unwrap();
    let names = ["--text", "--muted", "--primary"];
    for (name, (light, dark)) in names.iter().zip(LIGHT_FOREGROUNDS.iter().zip(DARK_FOREGROUNDS.iter())) {
        assert!(css.contains(&format!("{name}: {light};")), "app.css hell: {name}: {light}");
        assert_eq!(
            css.matches(&format!("{name}: {dark};")).count(),
            2,
            "app.css dunkel (beide Blöcke): {name}: {dark}"
        );
    }
    assert!(css.contains(&format!("--panel: {LIGHT_PANEL};")), "Fläche hell wie app.css");
    assert!(css.contains(&format!("--sheet: {LIGHT_PANEL};")), "Blatt im hellen Thema = Weiß wie app.css");
    assert_eq!(css.matches(&format!("--sheet: {DARK_SHEET};")).count(), 2, "Blatt dunkel wie app.css (beide Blöcke)");
    assert_eq!(css.matches(&format!("--panel: {DARK_PANEL};")).count(), 2, "Fläche dunkel wie app.css");
    // Die Tönung bleibt erkennbar, wo die Farbe es zulässt (sonst wäre die Prüfung trivial erfüllt).
    let tinted =
        PERSONA_PALETTE.iter().filter(|c| tints(c).light != LIGHT_PANEL && tints(c).dark != DARK_PANEL).count();
    assert_eq!(tinted, PERSONA_PALETTE.len(), "jede Palettenfarbe bleibt in beiden Themen getönt");
}

#[tokio::test]
async fn persona_list_names_every_persona_with_color_tints_and_lector() {
    let server = common::open().await;
    let answer = server.rpc("persona.list", json!({})).await;
    let personas = answer["result"]["personas"].as_array().expect("personas als Liste");
    assert_eq!(personas.len(), 6, "alle Personas des Beispiels: {answer}");
    let lektor = &personas[0];
    assert_eq!(lektor["handle"], "@lektor", "Reihenfolge nach position: {answer}");
    assert_eq!(lektor["color"], PERSONA_PALETTE[0], "Vorgabefarbe der ersten Persona");
    assert_eq!(lektor["chosen"], false, "nicht gewählt");
    assert_eq!(lektor["lector"], true, "der Lektor ist markiert (aktiv, größter Kontext)");
    // Die Vorgabe 32768 ist die Kontextlänge, mit der Eliezer Gemma lädt; mehr ergäbe ein zu großes Budget.
    assert_eq!(lektor["context_length"], 32_768, "Kontextlänge mitgeliefert");
    assert_eq!(lektor["model"], "google/gemma-4-26b-a4b", "Modell mitgeliefert");
    assert!(lektor["role"].as_str().is_some_and(|r| !r.is_empty()), "Rolle mitgeliefert");
    assert_eq!(lektor["active"], true, "aktiv");
    let t = tints(PERSONA_PALETTE[0]);
    assert_eq!(
        lektor["tints"],
        json!({ "light": t.light, "dark": t.dark, "line": t.line, "line_dark": t.line_dark }),
        "Tönungen wie tints(), mit der Linie je Thema"
    );
    assert_eq!(personas.iter().filter(|p| p["lector"] == true).count(), 1, "genau ein Lektor");
    assert_eq!(answer["result"]["palette"], json!(PERSONA_PALETTE), "die Palette");
}

#[tokio::test]
async fn persona_set_color_sets_resets_and_refuses_with_invalid_params() {
    let server = common::open().await;
    let set = server.rpc("persona.set_color", json!({ "handle": "@nathan", "color": "#FF8800" })).await;
    assert_eq!(set["result"], json!({ "handle": "@nathan", "color": "#ff8800", "chosen": true }), "{set}");
    let list = server.rpc("persona.list", json!({})).await;
    let nathan = list["result"]["personas"].as_array().unwrap().iter().find(|p| p["handle"] == "@nathan").cloned();
    assert_eq!(nathan.unwrap()["color"], "#ff8800", "persona.list zeigt die neue Farbe");

    let reset = server.rpc("persona.set_color", json!({ "handle": "@nathan", "color": null })).await;
    assert_eq!(reset["result"]["chosen"], false, "null = zurück zur Vorgabe: {reset}");
    assert_eq!(reset["result"]["color"], PERSONA_PALETTE[4], "Nathan steht an fünfter Stelle");
    let list = server.rpc("persona.list", json!({})).await;

    for (params, why) in [
        (json!({ "handle": "@nathan", "color": "orange" }), "ungültige Farbe"),
        (json!({ "handle": "@gibtsnicht", "color": "#123456" }), "unbekannte Persona"),
        (json!({ "handle": "@nathan" }), "color fehlt (kein stilles Zurücksetzen)"),
        (json!({ "color": "#123456" }), "handle fehlt"),
        (json!({ "handle": "@nathan", "color": 7 }), "color ist keine Zeichenkette"),
    ] {
        let refused = server.rpc("persona.set_color", params).await;
        assert_eq!(error_code(&refused), Some(INVALID_PARAMS), "{why} → -32602: {refused}");
    }
    let unknown = server.rpc("persona.set_color", json!({ "handle": "@gibtsnicht", "color": "#123456" })).await;
    assert_eq!(unknown["error"]["message"], "Persona „@gibtsnicht“ gibt es nicht.", "deutsche Meldung");
    let after = server.rpc("persona.list", json!({})).await;
    assert_eq!(after["result"], list["result"], "abgelehnte Aufrufe ändern nichts");
}

#[tokio::test]
async fn the_settings_page_changes_colors_with_forms_and_answers_422_on_errors() {
    let server = common::open().await;
    // Runde 9: Die Farben stehen jetzt auf der Einstellungsseite „Assistenten“; die alte Adresse leitet
    // dorthin weiter (Lesezeichen bleiben gültig), das Formular und seine Adresse bleiben gleich.
    let request = Request::get("/einstellungen/personas?work=1").body(Body::empty()).unwrap();
    let (status, headers, _) = server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "alte Adresse → Weiterleitung");
    assert_eq!(
        headers.get(header::LOCATION).and_then(|v| v.to_str().ok()),
        Some("/einstellungen/assistenten?work=1"),
        "weiter zur Seite „Assistenten“ mit dem Werk"
    );
    let (status, page) = server.get("/einstellungen/assistenten?work=1").await;
    assert_eq!(status, StatusCode::OK, "Seite lädt: {page}");
    assert!(
        page.contains("<form class=\"persona-save\" method=\"post\" action=\"/einstellungen/personas/act\">"),
        "Formular ohne JavaScript (seit Runde 11 eins je Persona mit allen Feldern): {page}"
    );
    assert!(page.contains("name=\"action\" value=\"save\""), "Aktion im Knopf „Speichern“");
    assert!(
        page.contains(&format!("<input type=\"color\" name=\"color\" value=\"{}\">", PERSONA_PALETTE[0])),
        "Farbwahl mit Vorgabe: {page}"
    );
    assert!(page.contains("name=\"color_text\""), "Textfeld für #rrggbb");
    assert!(page.contains(&format!("style=\"{}\"", style(PERSONA_PALETTE[0]))), "Vorschau in der Farbe: {page}");
    // Die Kopfzeile mit „Zurück zum Werk“ zeichnet seit Runde 9 base.html (Aufgabe menu); hier zählt,
    // dass die Navigation der Einstellungen das Werk behält.
    assert!(page.contains("href=\"/einstellungen/darstellung?work=1\""), "Navigation behält das Werk: {page}");

    let (status, location, _) = form(
        &server,
        "/einstellungen/personas/act",
        "method=persona.set_color&handle=%40lektor&color=%23336699&work=1",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert → Weiterleitung");
    assert_eq!(
        location.as_deref(),
        Some("/einstellungen/assistenten?ok=farbe&work=1#persona-lektor"),
        "zurück zur Seite „Assistenten“, Hinweis „farbe“, Anker der Persona"
    );
    let (_, page) = server.get("/einstellungen/assistenten?ok=farbe").await;
    assert!(page.contains("Die Farbe ist gespeichert."), "Hinweis nach dem Speichern");
    assert!(page.contains("value=\"#336699\""), "die Farbwahl zeigt die neue Farbe: {page}");

    let (status, ..) =
        form(&server, "/einstellungen/personas/act", "handle=%40lektor&color=%23000000&color_text=%23ABC").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Textfeld gespeichert");
    let lektor = server.state.store.persona_colors().await.unwrap().remove(0);
    assert_eq!(lektor.color, "#aabbcc", "das Textfeld geht vor der Farbwahl");

    let (status, ..) = form(&server, "/einstellungen/personas/act", "handle=%40lektor&color=%23000000&reset=1").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Vorgabe gespeichert");
    let lektor = server.state.store.persona_colors().await.unwrap().remove(0);
    assert_eq!((lektor.color.as_str(), lektor.chosen), (PERSONA_PALETTE[0], false), "reset=1 → Vorgabe");

    let (status, _, page) = form(&server, "/einstellungen/personas/act", "handle=%40lektor&color_text=gr%C3%BCn").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültige Farbe → 422");
    assert!(page.contains("Farbe als #rrggbb angeben."), "Meldung auf der Seite: {page}");
    let (status, _, page) = form(&server, "/einstellungen/personas/act", "handle=%40niemand&color=%23123456").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "unbekannte Persona → 422");
    assert!(page.contains("Persona „@niemand“ gibt es nicht."), "Meldung: {page}");
    let (status, ..) =
        form(&server, "/einstellungen/personas/act", "method=revision.delete&handle=%40lektor&color=%23123456").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nur persona.set_color");
    assert_eq!(
        server.state.store.persona_colors().await.unwrap().remove(0).color,
        PERSONA_PALETTE[0],
        "nichts geändert"
    );

    let request = Request::post("/einstellungen/personas/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::from("handle=%40lektor&color=%23123456"))
        .unwrap();
    let (status, ..) = server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft abgelehnt (guard::same_origin)");
}

#[tokio::test]
async fn the_settings_page_escapes_names_and_handles() {
    let server = common::open().await;
    sqlx::query(
        "INSERT INTO persona (handle, name, role, model, context_length, position) VALUES ('@x\"<>&''', 'Rosa \"<>&''', 'Probe', 'm', 1, 50)",
    )
    .execute(server.state.store.pool())
    .await
    .unwrap();
    // Runde 9: Die Seite heißt jetzt /einstellungen/assistenten (und zeigt dazu Rolle und Modell).
    let (_, page) = server.get("/einstellungen/assistenten").await;
    assert!(page.contains("Rosa &quot;&lt;&gt;&amp;&#x27;"), "Name maskiert: {page}");
    assert!(page.contains("value=\"@x&quot;&lt;&gt;&amp;&#x27;\""), "Handle im Formular maskiert: {page}");
    assert!(!page.contains("Rosa \"<>&'"), "kein roher Name");
}

#[tokio::test]
async fn persona_colors_are_not_reachable_over_act_or_mcp() {
    let server = common::open().await;
    let (status, ..) = form(&server, "/act", "method=persona.set_color&node=0&handle=%40lektor&color=%23123456").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "/act kennt persona.set_color nicht");
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (_, _, body) = server.send(request).await;
    let list: Value = serde_json::from_str(&body).unwrap();
    let tools = list["result"]["tools"].as_array().expect("Werkzeugliste");
    assert!(!tools.is_empty(), "MCP antwortet mit Werkzeugen: {list}");
    assert!(
        !tools.iter().any(|t| t["name"].as_str().is_some_and(|n| n.contains("color") || n.contains("farbe"))),
        "kein MCP-Werkzeug für Farben: {list}"
    );
    assert_eq!(server.state.store.persona_colors().await.unwrap()[0].color, PERSONA_PALETTE[0], "unverändert");
}

#[tokio::test]
async fn the_persona_color_appears_wherever_the_persona_does() {
    let server = common::open().await;
    server.rpc("persona.set_color", json!({ "handle": "@lektor", "color": "#c0ffee" })).await;
    let lektor = format!("style=\"{}\"", style("#c0ffee"));
    let nathan = format!("style=\"{}\"", style(PERSONA_PALETTE[4]));
    // Einleitung (Stelle 2): Beiträge von @nathan, @hoerer, @lektor; Fassung 3 ist ein Vorschlag von @lektor.
    let (status, page) = server.get("/?work=1&node=2&rev=3").await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    assert!(
        page.contains(&format!("<li class=\"persona pc\" {lektor}>")),
        "Assistentenliste in der Farbe des Lektors: {page}"
    );
    assert!(page.contains(&format!("<div class=\"post pc\" {nathan}>")), "Chat-Beitrag von @nathan in seiner Farbe");
    assert!(page.contains(&format!("<div class=\"post pc\" {lektor}>")), "Chat-Beitrag von @lektor in seiner Farbe");
    assert!(
        page.contains(&format!("<span class=\"revtree-author small pc pc-chip\" {lektor}>Persona @lektor")),
        "Fassungsliste: Persona-Fassung in ihrer Farbe: {page}"
    );
    assert!(
        page.contains(&format!("<span class=\"pc pc-chip\" {lektor}>@lektor (google&#x2f;gemma-4-26b-a4b)</span>")),
        "Fassungskopf im Editor in der Farbe: {page}"
    );
    // Runde 9: „Farben …“ führt auf die Einstellungsseite „Assistenten“ (ohne JavaScript als Seite, mit
    // JavaScript im Einstellungs-Popup über data-settings-open/data-settings-page).
    assert!(
        page.contains(
            "href=\"/einstellungen/assistenten?work=1\" data-settings-open data-settings-page=\"assistenten\""
        ),
        "Link „Farben …“ im Akkordeon Assistenten öffnet die Seite Assistenten: {page}"
    );
    let assistants = &page[page.find("data-section=\"personas\"").unwrap()..];
    let personas_section = &assistants[..assistants.find("data-section=\"chat\"").unwrap()];
    assert!(personas_section.contains(">Farben …</a>"), "der Link steht im Akkordeon „Assistenten“");
    let header = page.find("<header class=\"topbar\"").expect("Kopfzeile");
    let topbar = &page[header..header + page[header..].find("</header>").unwrap()];
    assert!(!topbar.contains("/einstellungen/assistenten"), "nicht in der Kopfzeile: {topbar}");
    // Menschliche Beiträge und Fassungen bleiben ungefärbt.
    let (_, current) = server.get("/?work=1&node=2&rev=2").await;
    assert!(
        current.contains("<span class=\"revtree-author small\">Autor</span>"),
        "Fassungen des Menschen ohne Farbe: {current}"
    );
    for value in current.split("--pc-line:").skip(1) {
        let color = &value[..7];
        assert!(
            color.starts_with('#') && color[1..].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "nur geprüfte #rrggbb im style: {color}"
        );
    }
}
