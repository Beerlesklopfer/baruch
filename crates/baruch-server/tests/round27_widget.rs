// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 27, Aufgabe umschlag-widget (R27c): Editor-Seite, Formulare ohne JavaScript, Widget cover-editor, Impressum-Vorlage übernehmen.
//!
//! Echter Datenordner (SQLite, Git-Spiegel) im Temp-Ordner mit dem erfundenen Gartenbuch der Server-Tests. Geprüft
//! werden die Umschlagseite als Editor (Rahmen mit aria-label, Hilfslinien, Druckprüfung, Formular je Block), die
//! Vorschau per GET ohne Speichern (auch als SVG), das Formular `POST /werke/umschlag/anordnung` (save, quick, reset,
//! 422 mit Eingabe, 403 von fremder Herkunft), GET mit Satz von fremden Seiten ohne Typst-Lauf, die Anschriften in den
//! Buchangaben, der Knopf „Impressum-Vorlage in die Werkvorlage übernehmen“, `checks` in `work.cover_pdf` und das
//! Widget im Node-Nachbau des DOM (fehlt `node`: ÜBERSPRUNGEN).
//!
//! Seit dem Zusammenführen von umschlag-daten (R27a) und umschlag-satz (R27b) durch den Integrator prüfen die Tests
//! nur noch die Wirkung (Store, Commit, Weiterleitung); antwortet ein Platzhalter „noch nicht umgesetzt“, schlagen
//! sie fehl.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_store::{CoverBlockKind, CoverFace, TitleArea};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Platzhalter der Aufgabe umschlag-daten bzw. umschlag-satz noch nicht ersetzt?
fn stub(message: &str, round: &str) -> bool {
    message.contains("noch nicht umgesetzt") && message.contains(round)
}

/// Gartenbuch importieren; Werk-id und slug.
async fn garden(server: &common::Server) -> (i64, String) {
    let report = server.import_garden().await;
    (report.work, report.slug)
}

fn commit_count(server: &common::Server, slug: &str) -> u32 {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |c| c.trim().parse().unwrap())
}

fn last_commit(server: &common::Server, slug: &str) -> String {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%B"]).unwrap_or_default()
}

/// HTML-Entitäten eines Attributwerts auflösen (minijinja maskiert `"`, `'`, `/`, `&`, `<`, `>`).
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(end) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let entity = &tail[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") || e.starts_with("#X") => {
                u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
            }
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Wert eines Attributs im ersten Tag, das `marker` enthält.
fn attribute(html: &str, marker: &str, name: &str) -> Option<String> {
    let at = html.find(marker)?;
    let start = html[..=at].rfind('<')?;
    let end = at + html[at..].find('>')?;
    let tag = &html[start..end];
    let key = format!(" {name}=\"");
    let from = tag.find(&key)? + key.len();
    let to = from + tag[from..].find('"')?;
    Some(unescape(&tag[from..to]))
}

/// Der Editor-Abschnitt der Seite (`<section class="cover-editor" …>…</section>`).
fn editor_section(page: &str) -> String {
    let start = page.find("<section class=\"cover-editor\"").expect("Editor-Abschnitt fehlt");
    let end = start + page[start..].find("</section>").expect("Ende des Editors") + "</section>".len();
    page[start..end].to_owned()
}

/// Alle Felder `b.<kind>.<feld>` des Formulars mit ihren Werten, wie ein Browser sie abschickt (Eingabefeld:
/// `value`, Auswahl: die gewählte Option, Textfeld: Inhalt).
fn block_fields(page: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = page;
    while let Some(at) = rest.find("name=\"b.") {
        let start = rest[..at].rfind('<').unwrap();
        let tag_end = at + rest[at..].find('>').unwrap();
        let tag = &rest[start..=tag_end];
        let name = attribute(tag, "name=\"b.", "name").unwrap();
        let after = &rest[tag_end + 1..];
        let value = if tag.starts_with("<input") {
            attribute(tag, "name=\"b.", "value").unwrap_or_default()
        } else if tag.starts_with("<select") {
            let body = &after[..after.find("</select>").unwrap()];
            let selected =
                body.split("<option").find(|o| o.contains(" selected")).or_else(|| body.split("<option").nth(1));
            let option = format!("<option{}", selected.unwrap());
            attribute(&option, "<option", "value").unwrap()
        } else {
            unescape(&after[..after.find("</textarea>").unwrap()])
        };
        out.push((name, value));
        rest = &rest[tag_end..];
    }
    out
}

/// Formular-Kodierung (application/x-www-form-urlencoded).
fn encode(pairs: &[(String, String)]) -> String {
    let escape = |text: &str| {
        let mut out = String::new();
        for b in text.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
                b' ' => out.push('+'),
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    };
    pairs.iter().map(|(k, v)| format!("{}={}", escape(k), escape(v))).collect::<Vec<_>>().join("&")
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// POST eines Formulars; Status, Weiterleitung und Text.
async fn post(server: &common::Server, uri: &str, form: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    let (status, headers, text) = server.send(request).await;
    let location = headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned());
    (status, location, text)
}

/// Antwort von `cover.layout` (Maße und wirksame Blöcke).
async fn layout(server: &common::Server, work: i64) -> Value {
    result(&server.rpc("cover.layout", json!({ "work": work })).await).clone()
}

/// Ist das Speichern der Anordnung (R27a) schon da? Probe mit einem gültigen Block über RPC.
async fn layout_set_is_stub(server: &common::Server, work: i64) -> bool {
    let view = layout(server, work).await;
    let block = view["blocks"][0].clone();
    let mut block = block.as_object().unwrap().clone();
    block.remove("stored");
    block.remove("height_mm");
    let answer = server.rpc("cover.layout_set", json!({ "work": work, "blocks": [block] })).await;
    match answer["error"]["message"].as_str() {
        Some(message) if stub(message, "R27a") => true,
        Some(message) => panic!("Probe cover.layout_set scheitert unerwartet: {message}"),
        None => {
            // Probe hat gespeichert: wieder zurücksetzen, damit der Test vom leeren Stand ausgeht.
            result(&server.rpc("cover.layout_reset", json!({ "work": work })).await);
            false
        }
    }
}

/// Die Umschlagseite ist ein Editor: Rahmen je Block mit aria-label (Art und Fläche), Hilfslinien für Beschnitt,
/// Schnittkante, Sicherheitsabstand, Rückenkanten und Mitte, Daten aus `cover.layout` im Attribut `data-layout`, je
/// Block ein Formularteil; die Elemente aus Runde 11 (Vorschau-SVG, Druckprüfung, PDF, Formulare) bleiben.
#[tokio::test]
async fn the_cover_page_is_an_editor_with_frames_guides_and_fields() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let (status, page) = server.get(&format!("/werke/{work}/umschlag")).await;
    assert_eq!(status, StatusCode::OK, "Umschlagseite: {page}");
    assert!(page.contains("data-widget=\"cover-editor\""), "Container des Widgets fehlt");
    let data = attribute(&page, "data-widget=\"cover-editor\"", "data-layout").expect("data-layout fehlt");
    let data: Value = serde_json::from_str(&data).expect("data-layout ist JSON");
    let view = layout(&server, work).await;
    assert_eq!(data, view, "data-layout ist die Antwort von cover.layout (Vertrag Abschnitt 4)");
    assert_eq!(data["blocks"].as_array().unwrap().len(), 15, "alle 15 Arten (Vertrag 1.1)");
    for kind in CoverBlockKind::ALL {
        let marker = format!("data-cover-block=\"{}\"", kind.as_str());
        let aria = attribute(&page, &marker, "aria-label").unwrap_or_else(|| panic!("Rahmen {marker} fehlt"));
        let label = format!("{} ({})", kind.label(), kind.face().label());
        assert!(aria.starts_with(&label), "aria-label nennt Art und Fläche „{label}“: {aria}");
        assert!(page.contains(&format!("name=\"b.{}.x_mm\"", kind.as_str())), "Formularfeld x für {kind:?}");
        assert_eq!(
            page.contains(&format!("name=\"b.{}.direction\"", kind.as_str())),
            kind.face() == CoverFace::Spine,
            "Laufrichtung nur beim Rücken ({kind:?})"
        );
        assert_eq!(
            page.contains(&format!("name=\"b.{}.text\"", kind.as_str())),
            kind.own_text(),
            "eigener Text nur bei Freitext und Motto ({kind:?})"
        );
    }
    for guide in ["bleed", "trim", "safety", "spine", "center"] {
        assert!(page.contains(&format!("data-guide=\"{guide}\"")), "Hilfslinie {guide} fehlt");
    }
    assert!(page.contains("title=\"Schnittkante\"") && page.contains("Rückenkante links"), "Hilfslinien beschriftet");
    // Ohne JavaScript: Formular mit Zahlenfeldern, Vorschau per GET, Übernehmen per POST, Schnellwahl, Zurücksetzen.
    assert!(page.contains("action=\"/werke/umschlag/anordnung\""), "Formular an POST /werke/umschlag/anordnung");
    assert!(page.contains("type=\"number\" name=\"b.front_title.y_mm\""), "Zahlenfelder");
    assert!(page.contains("formmethod=\"get\""), "Vorschau ohne Speichern per GET");
    assert!(page.contains("name=\"action\" value=\"save\""), "Übernehmen");
    assert!(page.contains("value=\"quick\"") && page.contains("data-cover-quick=\"bottom\""), "Schnellwahl");
    assert!(
        page.contains("value=\"reset\"") && page.contains("name=\"faces\" value=\"spine\""),
        "Zurücksetzen je Fläche"
    );
    // Die Tastaturhilfe steht da, Knöpfe nur mit JavaScript sichtbar.
    assert!(page.contains("Pfeiltasten 0,5 mm (mit Umschalt 5 mm)"), "Tastaturhilfe");
    assert!(page.contains("class=\"cover-editor-toolbar cover-js-only\""), "Werkzeugleiste nur mit JavaScript");
    // Runde 11 bleibt (round11_cover.rs prüft dasselbe).
    assert!(page.contains("id=\"druckpruefung\""), "Druckprüfung");
    assert!(page.contains(&format!("/werke/{work}/umschlag.svg?stand=")), "Vorschau als SVG");
    assert!(page.contains(&format!("/werke/{work}/umschlag.pdf")), "Link zum Umschlag-PDF");
    assert!(
        page.contains("action=\"/werke/umschlag/act\"") && page.contains("id=\"buchangaben\""),
        "Formulare bleiben"
    );
    // Warnungen der Druckprüfung: jede aus cover.layout steht als Listenpunkt da.
    for warning in view["warnings"].as_array().unwrap() {
        let code = warning["code"].as_str().unwrap();
        assert!(page.contains(&format!("data-cover-warning=\"{code}\"")), "Warnung {code} als Listenpunkt");
    }
    // Hell und Dunkel: der Editor nutzt die Farb-Tokens von app.css.
    let (_, css) = server.get("/static/app.css").await;
    let editor_css = &css[css.find("/* Umschlag-Editor (Runde 27c)").expect("Stil des Editors in app.css")..];
    let editor_css = &editor_css[..editor_css.find(".cover-imprint-form").unwrap()];
    for token in ["var(--panel)", "var(--line)", "var(--primary)", "var(--danger)", "var(--text)"] {
        assert!(editor_css.contains(token), "Farb-Token {token} im Stil des Editors");
    }
    let raw_colors = editor_css.matches('#').count();
    assert_eq!(raw_colors, 1, "außer dem weißen Grund der Vorschau keine festen Farben: {editor_css}");
}

/// Ohne JavaScript: `GET /werke/{id}/umschlag?b.front_title.y_mm=…` zeigt die Werte als Vorschau (Rahmen, Felder,
/// SVG-Adresse), speichert aber nichts; ungültige Werte → 422 mit Meldung; die SVG-Vorschau nimmt dieselben Felder.
#[tokio::test]
async fn a_preview_by_get_shows_the_values_without_saving() {
    let server = common::open().await;
    let (work, slug) = garden(&server).await;
    let commits = commit_count(&server, &slug);
    let query = "b.front_title.y_mm=40&b.front_title.x_mm=12,5&b.front_title.width_mm=80";
    let (status, page) = server.get(&format!("/werke/{work}/umschlag?{query}")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau: {page}");
    assert!(page.contains("data-cover-preview"), "Hinweis „Vorschau – noch nicht gespeichert“");
    let aria = attribute(&page, "data-cover-block=\"front_title\"", "aria-label").unwrap();
    assert!(aria.contains("x 12,5 mm, y 40,0 mm, Breite 80,0 mm"), "Rahmen zeigt die Vorschauwerte: {aria}");
    assert!(page.contains("name=\"b.front_title.y_mm\" value=\"40\""), "Feld zeigt den Vorschauwert");
    let data: Value =
        serde_json::from_str(&attribute(&page, "data-widget=\"cover-editor\"", "data-layout").unwrap()).unwrap();
    let title = data["blocks"].as_array().unwrap().iter().find(|b| b["kind"] == "front_title").unwrap();
    assert_eq!((title["x_mm"].as_f64(), title["y_mm"].as_f64()), (Some(12.5), Some(40.0)), "Widget-Daten mit Vorschau");
    let src = attribute(&page, "data-cover-image", "src").expect("Vorschaubild");
    assert!(src.contains("b.front_title.y_mm=40"), "das SVG zeigt dieselbe Vorschau: {src}");
    assert!(server.state.store.cover_layout(work).await.unwrap().is_empty(), "Vorschau speichert nichts");
    assert_eq!(commit_count(&server, &slug), commits, "Vorschau macht keinen Commit");

    let (status, svg) = server.get(&src.replace("&amp;", "&")).await;
    assert_eq!(status, StatusCode::OK, "SVG-Vorschau mit Feldern: {svg}");
    assert!(svg.starts_with("<svg"), "SVG-Antwort");

    let (status, page) = server.get(&format!("/werke/{work}/umschlag?b.front_title.y_mm=abc")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültige Vorschau → 422");
    assert!(
        page.contains("Titel (Vorderseite), y: Zahl zwischen -50 und 1000 mm erwartet"),
        "Meldung nennt Art und Feld"
    );
    assert!(page.contains("name=\"b.front_title.y_mm\" value=\"abc\""), "die Eingabe bleibt stehen");
    let (status, _) = server.get(&format!("/werke/{work}/umschlag.svg?b.front_title.size_pt=900")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültige SVG-Vorschau → 422");
    assert!(server.state.store.cover_layout(work).await.unwrap().is_empty(), "auch danach nichts gespeichert");
}

/// GET mit Satz von fremden Seiten (Seite und SVG, auch mit Vorschau) → 403 ohne Typst-Lauf; Formulare von fremder
/// Herkunft → 403.
#[tokio::test]
async fn foreign_sites_get_403_without_typesetting() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let (status, _) = server.get(&format!("/werke/{work}/umschlag")).await;
    assert_eq!(status, StatusCode::OK, "eigene Seite");
    let wrap = baruch_server::cover::build_wrap_for(&server.state, work, None).await.unwrap();
    let stand = baruch_render::stand(&wrap.source);
    let runs = baruch_server::typeset::runs_of(&stand);
    for uri in [
        format!("/werke/{work}/umschlag"),
        format!("/werke/{work}/umschlag?b.front_title.y_mm=33"),
        format!("/werke/{work}/umschlag.svg"),
        format!("/werke/{work}/umschlag.svg?b.front_title.y_mm=33"),
    ] {
        let request = Request::get(&uri).header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap();
        let (status, _, text) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} von fremder Seite → 403: {text}");
        assert!(text.contains("Der Satz startet nicht"), "Meldung ohne Satz: {text}");
    }
    assert_eq!(baruch_server::typeset::runs_of(&stand), runs, "kein Typst-Lauf für die fremden Anfragen");
    for (uri, form) in [
        ("/werke/umschlag/anordnung", format!("work={work}&action=reset")),
        ("/werke/umschlag/impressum", format!("work={work}")),
    ] {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::ORIGIN, "https://boese.example")
            .body(Body::from(form))
            .unwrap();
        assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "{uri} von fremder Herkunft → 403");
    }
}

/// `POST /werke/umschlag/anordnung`: `save` übergibt alle Blöcke des Formulars an `cover_layout::layout_set` und
/// leitet mit 303 und `ok=anordnung` auf die sichere `back`-Adresse; `quick` und `reset` rufen `layout_reset` mit
/// `area` bzw. `faces`; ungültige Eingabe → 422 mit Eingabe und Meldung, nichts gespeichert.
#[tokio::test]
async fn the_layout_form_saves_quick_selects_and_resets_without_javascript() {
    let server = common::open().await;
    let (work, slug) = garden(&server).await;
    // Seit dem Zusammenführen von R27a (Integrator) muss der echte Fall laufen, nicht mehr der Platzhalter.
    assert!(!layout_set_is_stub(&server, work).await, "cover.layout_set ist umgesetzt (R27a)");
    let (_, page) = server.get(&format!("/werke/{work}/umschlag")).await;
    let mut fields = block_fields(&editor_section(&page));
    assert_eq!(fields.iter().filter(|(k, _)| k.ends_with(".x_mm")).count(), 15, "alle 15 Blöcke im Formular");
    let y = fields.iter_mut().find(|(k, _)| k == "b.front_title.y_mm").unwrap();
    y.1 = "44,5".to_owned();
    let mut form = pairs(&[
        ("work", &work.to_string()),
        ("action", "save"),
        ("back", &format!("/werke/{work}/umschlag#anordnung")),
    ]);
    form.extend(fields.clone());

    // Ungültig: 422, Eingabe und Meldung bleiben, nichts gespeichert.
    let mut wrong = form.clone();
    wrong.iter_mut().find(|(k, _)| k == "b.back_blurb.width_mm").unwrap().1 = "0".to_owned();
    let (status, _, text) = post(&server, "/werke/umschlag/anordnung", &encode(&wrong)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültige Breite → 422");
    assert!(text.contains("Klappentext (Rückseite), Breite: Zahl zwischen 1 und 1000 mm erwartet"), "Meldung: {text}");
    assert!(text.contains("name=\"b.back_blurb.width_mm\" value=\"0\""), "Eingabe bleibt stehen");
    assert!(text.contains("name=\"b.front_title.y_mm\" value=\"44.5\""), "auch die übrigen Eingaben (Zahl mit Punkt)");
    assert!(server.state.store.cover_layout(work).await.unwrap().is_empty(), "nichts gespeichert");
    let (status, _, text) =
        post(&server, "/werke/umschlag/anordnung", &format!("work={work}&action=quick&area=links")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Schnellwahl mit falschem Bereich → 422");
    assert!(text.contains("Schnellwahl: „top“, „middle“ oder „bottom“ erwartet"), "Meldung: {text}");
    let (status, _, _) =
        post(&server, "/werke/umschlag/anordnung", &format!("work={work}&action=reset&faces=innen")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Zurücksetzen mit falscher Fläche → 422");

    let commits = commit_count(&server, &slug);
    let (status, location, text) = post(&server, "/werke/umschlag/anordnung", &encode(&form)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "gespeichert → 303: {text}");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/umschlag?ok=anordnung#anordnung").as_str()),
        "Weiterleitung auf back mit ok=anordnung vor dem Anker"
    );
    let stored = server.state.store.cover_layout(work).await.unwrap();
    assert_eq!(stored.blocks.len(), 15, "alle Blöcke des Formulars gespeichert");
    assert_eq!(stored.get(CoverBlockKind::FrontTitle).unwrap().y_mm, 44.5, "Wert aus dem Formular (Komma)");
    assert_eq!(commit_count(&server, &slug), commits + 1, "ein Commit");
    let (_, page) = server.get(location.as_deref().unwrap()).await;
    assert!(page.contains("Die Umschlag-Anordnung ist gespeichert."), "Hinweis nach ok=anordnung");

    let (status, location, _) = post(
        &server,
        "/werke/umschlag/anordnung",
        &format!("work={work}&action=quick&area=bottom&back=%2F%2Fboese.example"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Schnellwahl → 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{work}/umschlag?ok=anordnung").as_str()),
        "unsicheres back → Umschlagseite"
    );
    assert_eq!(server.state.store.book_meta(work).await.unwrap().title_area, TitleArea::Bottom, "Titelbereich unten");
    let stored = server.state.store.cover_layout(work).await.unwrap();
    assert!(stored.blocks.iter().all(|b| b.kind.face() != CoverFace::Front), "Vorderseite zurückgesetzt");
    assert!(stored.get(CoverBlockKind::SpineTitle).is_some(), "Rücken bleibt");

    let (status, _, _) =
        post(&server, "/werke/umschlag/anordnung", &format!("work={work}&action=reset&faces=spine")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Zurücksetzen des Rückens → 303");
    let stored = server.state.store.cover_layout(work).await.unwrap();
    assert!(stored.blocks.iter().all(|b| b.kind.face() == CoverFace::Back), "nur die Rückseite bleibt: {stored:?}");
    let (status, _, _) = post(&server, "/werke/umschlag/anordnung", &format!("work={work}&action=reset")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Zurücksetzen ohne Fläche → 303");
    assert!(server.state.store.cover_layout(work).await.unwrap().is_empty(), "ohne Fläche: alle zurückgesetzt");
}

/// Buchangaben: Formular mit beiden Anschriften (mehrzeilig); `work.meta.set` nimmt `publisher_address` und
/// `printer_address` an und nennt sie im Commit-Rumpf; `work.meta.get` liefert sie.
#[tokio::test]
async fn book_meta_takes_both_addresses_and_names_them_in_the_commit() {
    let server = common::open().await;
    let (work, slug) = garden(&server).await;
    let (_, page) = server.get(&format!("/werke/{work}/umschlag")).await;
    assert!(page.contains("Anschrift des Verlags bzw. Herausgebers"), "Feld Verlagsanschrift");
    assert!(page.contains("Anschrift der Druckerei"), "Feld Druckereianschrift");
    assert!(
        page.contains("<textarea name=\"publisher_address\" rows=\"3\" maxlength=\"500\">"),
        "mehrzeilig, höchstens 500"
    );
    assert!(
        page.contains("<textarea name=\"printer_address\" rows=\"3\" maxlength=\"500\">"),
        "mehrzeilig, höchstens 500"
    );
    let (_, settings) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert!(settings.contains("name=\"publisher_address\""), "auch in Einstellungen → Werk");
    assert_eq!(baruch_server::cover::field_label("publisher_address"), "Anschrift des Verlags bzw. Herausgebers");
    assert_eq!(baruch_server::cover::field_label("printer_address"), "Anschrift der Druckerei");

    // Leere Werte gehen immer durch.
    result(&server.rpc("work.meta.set", json!({ "work": work, "publisher_address": "", "printer_address": "" })).await);
    let commits = commit_count(&server, &slug);
    let address = "Gartenweg 7\n12345 Erfundstadt";
    let answer = server
        .rpc(
            "work.meta.set",
            json!({ "work": work, "publisher_address": address, "printer_address": "Druckhaus Beet\nAm Hang 2" }),
        )
        .await;
    assert!(answer["error"].is_null(), "Anschriften werden gespeichert (R27a zusammengeführt): {answer}");
    let changed: Vec<&str> =
        result(&answer)["changed"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    assert!(
        changed.contains(&"publisher_address") && changed.contains(&"printer_address"),
        "beide geändert: {changed:?}"
    );
    assert_eq!(commit_count(&server, &slug), commits + 1, "beide Anschriften in einem Commit");
    let body = last_commit(&server, &slug);
    assert!(
        body.contains("Anschrift des Verlags bzw. Herausgebers: – → Gartenweg 7 12345 Erfundstadt"),
        "Rumpf: {body}"
    );
    assert!(body.contains("Anschrift der Druckerei: – → Druckhaus Beet Am Hang 2"), "Rumpf: {body}");
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await)["meta"].clone();
    assert_eq!(meta["publisher_address"], address, "work.meta.get liefert die Anschrift mehrzeilig");
    let form =
        format!("work={work}&flags=1&printer_address=Neue+Druckerei%0D%0AWeg+3&back=%2Fwerke%2F{work}%2Fumschlag");
    let (status, location, _) = post(&server, "/werke/buchangaben/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular gespeichert");
    assert_eq!(location.as_deref(), Some(format!("/werke/{work}/umschlag?hinweis=buchangaben").as_str()));
    let meta = result(&server.rpc("work.meta.get", json!({ "work": work })).await)["meta"].clone();
    assert!(meta["printer_address"].as_str().unwrap().starts_with("Neue Druckerei"), "aus dem Formular: {meta}");
}

/// Knopf „Impressum-Vorlage in die Werkvorlage übernehmen“ (Einstellungen → Werk und Umschlagseite): neue
/// Vorlagenfassung über `work.update` mit Commit; ist alles schon da, ehrliche
/// Meldung ohne Änderung; `ok=impressum` zeigt den Hinweis. Dasselbe per RPC `cover.imprint_adopt`.
#[tokio::test]
async fn the_imprint_button_adopts_the_template_or_refuses_honestly() {
    let server = common::open().await;
    let (work, slug) = garden(&server).await;
    let button = "Impressum-Vorlage in die Werkvorlage übernehmen";
    let (_, page) = server.get(&format!("/werke/{work}/umschlag")).await;
    assert!(
        page.contains(button) && page.contains("action=\"/werke/umschlag/impressum\""),
        "Knopf auf der Umschlagseite"
    );
    let (_, settings) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert!(settings.contains(button), "Knopf in Einstellungen → Werk");
    assert!(
        settings.contains(&format!("name=\"back\" value=\"/einstellungen/werk?work={work}\"")),
        "Rücksprung in die Einstellungen"
    );

    let template = server.state.store.work_template(work).await.unwrap();
    let rev = server.state.store.work_meta(work).await.unwrap().template_rev;
    let commits = commit_count(&server, &slug);
    let form = format!("work={work}&back=%2Feinstellungen%2Fwerk%3Fwork%3D{work}");
    match baruch_typst::cover::imprint_template_missing(&template) {
        Err(why) => panic!("die Impressum-Vorlage ist umgesetzt (R27b), der Garten hat keine Funktion: {why}"),
        Ok(adopted) => {
            let (status, location, text) = post(&server, "/werke/umschlag/impressum", &form).await;
            assert_eq!(status, StatusCode::SEE_OTHER, "übernommen → 303: {text}");
            assert_eq!(location.as_deref(), Some(format!("/einstellungen/werk?work={work}&ok=impressum").as_str()));
            assert_eq!(server.state.store.work_template(work).await.unwrap(), adopted, "Vorlage mit den Funktionen");
            assert_eq!(server.state.store.work_meta(work).await.unwrap().template_rev, rev + 1, "neue Vorlagenfassung");
            assert_eq!(commit_count(&server, &slug), commits + 1, "ein Commit");
            let (_, page) = server.get(location.as_deref().unwrap()).await;
            assert!(
                page.contains("Die Impressum-Vorlage ist in die Werkvorlage übernommen"),
                "Hinweis nach ok=impressum"
            );
            // Jetzt ist alles da: ehrliche Ablehnung ohne Änderung.
            let (status, _, text) =
                post(&server, "/werke/umschlag/impressum", &format!("work={work}&back=%2Fwerke%2F{work}%2Fumschlag"))
                    .await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "alles schon da → 422");
            assert!(text.contains("Impressum-Vorlage nicht übernommen"), "Meldung auf der Umschlagseite: {text}");
            let answer = server.rpc("cover.imprint_adopt", json!({ "work": work })).await;
            assert_eq!(answer["error"]["code"], -32000, "RPC: REFUSED: {answer}");
        }
    }
    assert!(
        server.state.store.work_meta(work).await.unwrap().template_rev <= rev + 1,
        "höchstens eine neue Vorlagenfassung"
    );
}

/// `work.cover_pdf` liefert zusätzlich `checks` aus `cover_print::cover_checks` (gespeicherte Anordnung).
#[tokio::test]
async fn work_cover_pdf_carries_the_cover_checks() {
    // Erzeugt ein Umschlag-PDF: die Markierung „PDF entsteht“ ist prozessweit je Werk-id.
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let answer = server.rpc("work.cover_pdf", json!({ "work": work })).await;
    let checks = result(&answer)["checks"].clone();
    assert!(checks.is_array(), "checks ist eine Liste: {answer}");
    let expected = baruch_server::cover_print::cover_checks(&server.state, work, None).await.unwrap();
    assert_eq!(checks, json!(expected), "dieselben Warnungen wie die Druckprüfung");
    assert!(result(&answer)["sha256"].is_string(), "die übrigen Felder bleiben");
}

/// Szene im Nachbau ausführen; `None`, wenn Node fehlt (dann wird ehrlich übersprungen).
fn run_widget(scenario: &Value) -> Option<Value> {
    let node = std::process::Command::new("node").arg("--version").output().ok()?;
    if !node.status.success() {
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let js = dir.path().join("js");
    std::fs::create_dir(&js).unwrap();
    for (name, text) in baruch_server::SCRIPTS {
        std::fs::write(js.join(name), text).unwrap();
    }
    let harness = dir.path().join("harness.mjs");
    std::fs::write(&harness, HARNESS).unwrap();
    let file = dir.path().join("scenario.json");
    std::fs::write(&file, scenario.to_string()).unwrap();
    let output = std::process::Command::new("node")
        .arg(&harness)
        .arg(&file)
        .arg(format!("file://{}", js.display()))
        .output()
        .expect("node startet");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "Nachbau scheitert: {}\n{stdout}", String::from_utf8_lossy(&output.stderr));
    Some(serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("Ausgabe des Nachbaus ist kein JSON ({e}): {stdout}")))
}

fn close(a: &Value, b: f64) -> bool {
    a.as_f64().is_some_and(|a| (a - b).abs() < 0.011)
}

/// Verhaltenstest des Widgets im Node-Nachbau (echtes `cover_editor.js` auf dem HTML der Umschlagseite):
/// Ziehen ändert x/y, der Griff ändert die Breite, Pfeiltasten 0,5 mm bzw. 5 mm mit Umschalt, Einrasten an der
/// Hilfslinie „Sicherheitsabstand“, Rückgängig stellt den vorigen Wert her, „Hell/Dunkel“ und ein geändertes Feld
/// wirken auf den Block, Strg+↓ ändert die Breite des Rückentitels, Speichern schickt `cover.layout_set` mit allen
/// geänderten Blöcken (und nur ihnen).
#[tokio::test]
async fn the_widget_drags_resizes_steps_snaps_undoes_and_saves_without_a_browser() {
    let server = common::open().await;
    let (work, _) = garden(&server).await;
    let geometry = layout(&server, work).await["geometry"].clone();
    let w = geometry["page_width_mm"].as_f64().unwrap();
    let h = geometry["page_height_mm"].as_f64().unwrap();
    let safety = geometry["safety_mm"].as_f64().unwrap();
    let bleed = geometry["bleed_mm"].as_f64().unwrap();
    let spine = geometry["spine_mm"].as_f64().unwrap();
    // Gartenbuch ohne Seitenmaß: A4. Die Wege unten liegen dann fern jeder Hilfslinie (außer dem Einrast-Schritt).
    assert!(w > 130.0 && h > 150.0, "Seitenmaß reicht für die Szene: {geometry}");
    let x0 = w / 2.0 + 20.0;
    let query =
        format!("b.front_title.x_mm={x0}&b.front_title.y_mm=40&b.front_title.width_mm=20&b.front_title.color=light");
    let (status, page) = server.get(&format!("/werke/{work}/umschlag?{query}")).await;
    assert_eq!(status, StatusCode::OK, "Seite mit Vorschau");
    let view: Value =
        serde_json::from_str(&attribute(&page, "data-widget=\"cover-editor\"", "data-layout").unwrap()).unwrap();
    let wrap_w = 2.0 * w + spine + 2.0 * bleed;
    // 2 px je mm: 20 px Ziehen = 10 mm.
    let snap_dx = ((safety + 0.8) - (x0 + 10.5)) * 2.0;
    let steps = json!([
        { "name": "move", "type": "drag", "dx": 20, "dy": 30 },
        { "name": "resize", "type": "drag", "handle": "end", "dx": 10, "dy": 0 },
        { "name": "right", "type": "key", "key": "ArrowRight" },
        { "name": "down5", "type": "key", "key": "ArrowDown", "shift": true },
        { "name": "undo", "type": "click", "selector": "[data-cover-undo]" },
        { "name": "snap", "type": "drag", "dx": snap_dx, "dy": 0 },
        { "name": "tone", "type": "click", "selector": "[data-cover-tone]", "select": true },
        { "name": "size", "type": "input", "field": "size_pt", "value": "18" },
        { "name": "spine", "type": "key", "kind": "spine_title", "key": "ArrowDown", "ctrl": true },
        { "name": "save", "type": "click", "selector": "[data-cover-save]" },
    ]);
    let scenario = json!({
        "html": editor_section(&page), "stageWidth": wrap_w * 2.0, "steps": steps,
        "answers": { "cover.layout_set": { "layout": view } },
    });
    let Some(seen) = run_widget(&scenario) else {
        eprintln!("ÜBERSPRUNGEN: node fehlt – Verhaltenstest des Umschlag-Editors nicht gelaufen.");
        return;
    };
    assert_eq!(seen["errors"], json!([]), "keine Fehler beim Einhängen und Bedienen: {seen}");
    assert_eq!(seen["ready"], true, "das Widget hat sich eingehängt");
    let step = |name: &str| seen["steps"].as_array().unwrap().iter().find(|s| s["name"] == name).unwrap().clone();
    let start = step("start");
    assert!(close(&start["x"], x0) && close(&start["y"], 40.0) && close(&start["w"], 20.0), "Startwerte: {start}");
    assert_eq!(start["undoDisabled"], true, "nichts rückgängig zu machen");
    assert_eq!(start["saveDisabled"], true, "nichts zu speichern");
    let left: f64 = start["left"].as_str().unwrap().trim_end_matches('%').parse().unwrap();
    let expected_left = (bleed + w + spine + x0) / wrap_w * 100.0;
    assert!((left - expected_left).abs() < 0.01, "Rahmen links nach Vertrag 1.2 ({expected_left} %): {start}");

    let moved = step("move");
    assert!(
        close(&moved["x"], x0 + 10.0) && close(&moved["y"], 55.0),
        "Ziehen ändert x und y (20/30 px = 10/15 mm): {moved}"
    );
    assert_eq!(moved["snapped"], "", "fern jeder Hilfslinie kein Einrasten: {moved}");
    assert!(
        close(&Value::from(moved["inputY"].as_str().unwrap().parse::<f64>().unwrap()), 55.0),
        "Feld y folgt: {moved}"
    );
    assert!(moved["aria"].as_str().unwrap().contains("y 55,0 mm"), "aria-label folgt: {moved}");
    assert_eq!(moved["undoDisabled"], false, "jetzt rückgängig möglich");
    assert_eq!(moved["dirty"], "1", "geändert");
    let resized = step("resize");
    assert!(close(&resized["w"], 25.0) && close(&resized["x"], x0 + 10.0), "Griff ändert nur die Breite: {resized}");
    let right = step("right");
    assert!(close(&right["x"], x0 + 10.5), "Pfeil rechts: 0,5 mm: {right}");
    let down = step("down5");
    assert!(close(&down["y"], 60.0), "Umschalt+Pfeil runter: 5 mm: {down}");
    let undone = step("undo");
    assert!(close(&undone["y"], 55.0) && close(&undone["x"], x0 + 10.5), "Rückgängig stellt y wieder her: {undone}");
    let snapped = step("snap");
    assert!(close(&snapped["x"], safety), "rastet am Sicherheitsabstand ein ({safety} mm statt +0,8): {snapped}");
    assert_eq!(snapped["snapped"], "x", "eingerastet in x: {snapped}");
    let toned = step("tone");
    assert_eq!(toned["color"], "dark", "Hell/Dunkel: hell → dunkel: {toned}");
    assert_eq!(toned["inputColor"], "dark", "Feld Farbe folgt: {toned}");
    let spined = step("spine");
    let spine_start =
        view["blocks"].as_array().unwrap().iter().find(|b| b["kind"] == "spine_title").unwrap()["width_mm"]
            .as_f64()
            .unwrap();
    assert!(close(&spined["w"], spine_start + 0.5), "Strg+↓ verlängert den Rückentitel um 0,5 mm: {spined}");

    let calls = seen["calls"].as_array().unwrap();
    let saves: Vec<&Value> = calls.iter().filter(|c| c["method"] == "cover.layout_set").collect();
    assert_eq!(saves.len(), 1, "genau ein Speichern: {calls:?}");
    let params = &saves[0]["params"];
    assert_eq!(params["work"], work, "für das Werk der Seite");
    let blocks = params["blocks"].as_array().unwrap();
    let kinds: Vec<&str> = blocks.iter().map(|b| b["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["front_title", "spine_title"], "alle geänderten Blöcke, sonst keine: {kinds:?}");
    let title = &blocks[0];
    assert!(
        close(&title["x_mm"], safety) && close(&title["y_mm"], 55.0) && close(&title["width_mm"], 25.0),
        "Werte: {title}"
    );
    assert_eq!(title["color"], "dark", "Farbe");
    assert!(close(&title["size_pt"], 18.0), "Schriftgröße aus dem Feld");
    assert!(title.get("stored").is_none() && title.get("height_mm").is_none(), "nur Blockfelder: {title}");
    let saved = step("save");
    assert_eq!(saved["saveDisabled"], true, "nach dem Speichern nichts mehr offen: {saved}");
    assert_eq!(saved["dirty"], "", "nicht mehr geändert");
    assert!(saved["status"].as_str().unwrap().starts_with("Gespeichert (2 Blöcke)"), "Meldung: {saved}");
}

const HARNESS: &str = r#####"
import { readFileSync } from "node:fs";
const scenario = JSON.parse(readFileSync(process.argv[2], "utf8"));
const errors = [];
const calls = [];
const submitted = [];
console.error = (...args) => errors.push(args.map(String).join(" "));

class Node_ {
  constructor() { this.parentNode = null; this.childNodes = []; this.listeners = []; }
  get parentElement() { return this.parentNode instanceof HTMLElement ? this.parentNode : null; }
  get isConnected() { let n = this; while (n.parentNode) n = n.parentNode; return n === document; }
  get nextSibling() { const p = this.parentNode; return p ? p.childNodes[p.childNodes.indexOf(this) + 1] ?? null : null; }
  remove() { if (this.parentNode) { const s = this.parentNode.childNodes; s.splice(s.indexOf(this), 1); this.parentNode = null; } }
  before(...nodes) { const p = this.parentNode; for (const n of nodes) { n.remove(); n.parentNode = p; p.childNodes.splice(p.childNodes.indexOf(this), 0, n); } }
  after(...nodes) { const next = this.nextSibling; if (next) next.before(...nodes); else this.parentNode.append(...nodes); }
  replaceWith(node) { this.before(node); this.remove(); }
  addEventListener(type, fn, options) { this.listeners.push({ type, fn, capture: options === true || !!options?.capture, once: !!options?.once }); }
  removeEventListener(type, fn) { this.listeners = this.listeners.filter((l) => l.type !== type || l.fn !== fn); }
  dispatchEvent(event) {
    event.target = this;
    const path = [];
    for (let n = this.parentNode; n; n = n.parentNode) path.push(n);
    if (this !== window) path.push(window);
    const run = (node, capture) => {
      for (const l of [...node.listeners]) {
        if (l.type !== event.type || (capture !== null && l.capture !== capture)) continue;
        if (l.once) node.removeEventListener(l.type, l.fn);
        event.currentTarget = node;
        l.fn.call(node, event);
        if (event.stopped) return false;
      }
      return true;
    };
    for (const node of [...path].reverse()) if (!run(node, true)) return !event.defaultPrevented;
    if (!run(this, null)) return !event.defaultPrevented;
    if (event.bubbles) for (const node of path) if (!run(node, false)) break;
    return !event.defaultPrevented;
  }
  get textContent() { return this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(value) { for (const c of [...this.childNodes]) c.remove(); if (value !== "") this.append(new Text_(String(value))); }
}
class Text_ extends Node_ { constructor(data) { super(); this.data = data; this.nodeType = 3; } get textContent() { return this.data; } set textContent(v) { this.data = String(v); } }
class Comment_ extends Node_ { constructor(data) { super(); this.data = data; this.nodeType = 8; } get textContent() { return ""; } }

class HTMLElement extends Node_ {
  constructor(tag) { super(); this.tagName = tag.toUpperCase(); this.attrs = new Map(); this.nodeType = 1; this.style = {}; }
  get localName() { return this.tagName.toLowerCase(); }
  getAttribute(name) { return this.attrs.has(name) ? this.attrs.get(name) : null; }
  setAttribute(name, value) { this.attrs.set(name, String(value)); }
  hasAttribute(name) { return this.attrs.has(name); }
  removeAttribute(name) { this.attrs.delete(name); }
  toggleAttribute(name, force) { const on = force === undefined ? !this.hasAttribute(name) : !!force; if (on) this.setAttribute(name, ""); else this.removeAttribute(name); return on; }
  get id() { return this.getAttribute("id") ?? ""; }
  get className() { return this.getAttribute("class") ?? ""; }
  set className(v) { this.setAttribute("class", v); }
  get classList() {
    const el = this;
    const list = () => el.className.split(/\s+/).filter(Boolean);
    return {
      contains: (c) => list().includes(c),
      add: (...cs) => { el.className = [...new Set([...list(), ...cs])].join(" "); },
      remove: (...cs) => { el.className = list().filter((x) => !cs.includes(x)).join(" "); },
      toggle: (c, force) => { const on = force ?? !list().includes(c); if (on) el.classList.add(c); else el.classList.remove(c); return on; },
    };
  }
  get dataset() {
    const el = this;
    const attr = (key) => "data-" + String(key).replace(/[A-Z]/g, (m) => "-" + m.toLowerCase());
    return new Proxy({}, {
      get: (_, key) => (typeof key === "string" ? el.getAttribute(attr(key)) ?? undefined : undefined),
      set: (_, key, value) => { el.setAttribute(attr(key), value); return true; },
      deleteProperty: (_, key) => { el.removeAttribute(attr(key)); return true; },
      has: (_, key) => el.hasAttribute(attr(key)),
    });
  }
  get hidden() { return this.hasAttribute("hidden"); }
  set hidden(v) { this.toggleAttribute("hidden", !!v); }
  get disabled() { return this.hasAttribute("disabled"); }
  set disabled(v) { this.toggleAttribute("disabled", !!v); }
  get children() { return this.childNodes.filter((c) => c instanceof HTMLElement); }
  get firstElementChild() { return this.children[0] ?? null; }
  append(...nodes) { for (let n of nodes) { if (typeof n === "string") n = new Text_(n); n.remove(); n.parentNode = this; this.childNodes.push(n); } }
  prepend(...nodes) { const first = this.childNodes[0]; if (first) first.before(...nodes); else this.append(...nodes); }
  appendChild(n) { this.append(n); return n; }
  contains(n) { for (; n; n = n.parentNode) if (n === this) return true; return false; }
  set innerHTML(html) { for (const c of [...this.childNodes]) c.remove(); parseInto(this, html); }
  get innerHTML() { return this.childNodes.map(serialize).join(""); }
  focus() { document.activeElement = this; }
  setPointerCapture() {}
  releasePointerCapture() {}
  blur() {}
  scrollIntoView() {}
  getBoundingClientRect() {
    if (this.hasAttribute("data-cover-stage")) return { top: 0, left: 0, right: scenario.stageWidth, bottom: 0, width: scenario.stageWidth, height: 0 };
    return { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 };
  }
  descendants() { const out = []; const walk = (n) => { for (const c of n.children) { out.push(c); walk(c); } }; walk(this); return out; }
  querySelectorAll(selector) { return this.descendants().filter((e) => matches(e, selector, this)); }
  querySelector(selector) { return this.querySelectorAll(selector)[0] ?? null; }
  matches(selector) { return matches(this, selector, null); }
  closest(selector) { for (let n = this; n instanceof HTMLElement; n = n.parentNode) if (matches(n, selector, null)) return n; return null; }
  get elements() { return this.querySelectorAll("input, select, textarea, button"); }
}
class HTMLFormElement extends HTMLElement {
  submit() {
    const fields = {};
    for (const e of this.elements) {
      const name = e.getAttribute("name");
      if (name && !e.disabled && !(e instanceof HTMLButtonElement)) fields[name] = e.value;
    }
    submitted.push({ action: this.getAttribute("action"), fields });
  }
  requestSubmit() { this.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true })); }
}
class HTMLInputElement extends HTMLElement {
  get value() { return this._value ?? this.getAttribute("value") ?? ""; }
  set value(v) { this._value = String(v); }
  get type() { return this.getAttribute("type") ?? "text"; }
  get checked() { return this._checked ?? this.hasAttribute("checked"); }
  set checked(v) { this._checked = !!v; }
}
class HTMLButtonElement extends HTMLElement { get type() { return this.getAttribute("type") ?? "submit"; } }
class HTMLTextAreaElement extends HTMLElement {
  get value() { return this._value ?? this.textContent; }
  set value(v) { this._value = String(v); }
}
class HTMLOptionElement extends HTMLElement { get value() { return this.getAttribute("value") ?? this.textContent; } set value(v) { this.setAttribute("value", v); } }
class HTMLSelectElement extends HTMLElement {
  get options() { return this.querySelectorAll("option"); }
  get value() { const o = this.options; const s = o.find((x) => x._selected) ?? o.find((x) => x.hasAttribute("selected")) ?? o[0]; return s ? s.value : ""; }
  set value(v) { for (const o of this.options) o._selected = o.value === v; }
}
class HTMLDetailsElement extends HTMLElement { get open() { return this.hasAttribute("open"); } set open(v) { this.toggleAttribute("open", !!v); } }
class HTMLStyleElement extends HTMLElement {}
const KINDS = { form: HTMLFormElement, input: HTMLInputElement, button: HTMLButtonElement, textarea: HTMLTextAreaElement,
  option: HTMLOptionElement, select: HTMLSelectElement, details: HTMLDetailsElement, style: HTMLStyleElement };
const make = (tag) => new (KINDS[tag.toLowerCase()] ?? HTMLElement)(tag);

// ---- Selektoren: Listen mit Komma, Nachfahre (Leerzeichen) und Kind (>), Tag, #id, .klasse, [a], [a=b], :scope.
function splitTop(text, sep) {
  const out = []; let depth = 0, quote = null, cur = "";
  for (const ch of text) {
    if (quote) { if (ch === quote) quote = null; cur += ch; continue; }
    if (ch === '"' || ch === "'") { quote = ch; cur += ch; continue; }
    if (ch === "[" || ch === "(") depth++;
    if (ch === "]" || ch === ")") depth--;
    if (ch === sep && depth === 0) { out.push(cur); cur = ""; continue; }
    cur += ch;
  }
  out.push(cur);
  return out;
}
function compound(el, part, scope) {
  const re = /(:scope)|#([\w-]+)|\.([\w-]+)|\[([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\]]*)))?\]|^([\w*-]+)/g;
  let m, used = 0;
  while ((m = re.exec(part))) {
    used += m[0].length;
    if (m[1] && el !== scope) return false;
    if (m[2] && el.id !== m[2]) return false;
    if (m[3] && !el.classList.contains(m[3])) return false;
    if (m[4]) { const v = m[5] ?? m[6] ?? m[7]; if (!el.hasAttribute(m[4]) || (v !== undefined && el.getAttribute(m[4]) !== v)) return false; }
    if (m[8] && m[8] !== "*" && el.localName !== m[8].toLowerCase()) return false;
  }
  if (used !== part.length) throw new Error("Selektor nicht nachgebaut: " + part);
  return true;
}
function matchesComplex(el, tokens, scope) {
  if (!compound(el, tokens[tokens.length - 1], scope)) return false;
  if (tokens.length === 1) return true;
  const comb = tokens[tokens.length - 2], rest = tokens.slice(0, -2);
  if (comb === ">") return el.parentNode instanceof HTMLElement && matchesComplex(el.parentNode, rest, scope);
  for (let p = el.parentNode; p instanceof HTMLElement; p = p.parentNode) if (matchesComplex(p, rest, scope)) return true;
  return false;
}
function matches(el, selector, scope) {
  return splitTop(selector, ",").some((one) => {
    const tokens = one.trim().replace(/\s*>\s*/g, " > ").split(/\s+/).filter(Boolean);
    return matchesComplex(el, tokens, scope);
  });
}

// ---- HTML-Leser für das wohlgeformte Server-HTML.
const VOID = new Set(["input", "br", "img", "meta", "link", "hr", "source", "wbr"]);
const RAW = new Set(["style", "script", "textarea"]);
function decode(text) {
  return text.replace(/&(#x[0-9a-f]+|#\d+|amp|lt|gt|quot|apos|nbsp);/gi, (_, e) => {
    if (e[0] === "#") return String.fromCodePoint(e[1] === "x" || e[1] === "X" ? parseInt(e.slice(2), 16) : parseInt(e.slice(1), 10));
    return { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " }[e.toLowerCase()];
  });
}
function parseInto(root, html) {
  let at = 0, cur = root;
  while (at < html.length) {
    if (html.startsWith("<!--", at)) { const end = html.indexOf("-->", at); cur.append(new Comment_(html.slice(at + 4, end))); at = end + 3; continue; }
    if (html.startsWith("</", at)) {
      const end = html.indexOf(">", at); const tag = html.slice(at + 2, end).trim().toLowerCase();
      for (let n = cur; n !== root; n = n.parentNode) if (n.localName === tag) { cur = n.parentNode; break; }
      at = end + 1; continue;
    }
    if (html[at] === "<" && /[a-zA-Z]/.test(html[at + 1] ?? "")) {
      const m = /^<([a-zA-Z][\w-]*)/.exec(html.slice(at)); const tag = m[1].toLowerCase(); at += m[0].length;
      const el = make(tag);
      const attr = /^\s*([^\s=>\/]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/;
      for (;;) {
        const rest = html.slice(at); const close = /^\s*\/?>/.exec(rest);
        if (close) { at += close[0].length; break; }
        const a = attr.exec(rest); if (!a) throw new Error("HTML nicht lesbar bei " + rest.slice(0, 40));
        el.setAttribute(a[1].toLowerCase(), decode(a[2] ?? a[3] ?? a[4] ?? "")); at += a[0].length;
      }
      cur.append(el);
      if (RAW.has(tag)) {
        const end = html.toLowerCase().indexOf("</" + tag, at); let text = html.slice(at, end);
        if (tag === "textarea") { if (text.startsWith("\n")) text = text.slice(1); text = decode(text); }
        if (text) el.append(new Text_(text));
        at = html.indexOf(">", end) + 1; continue;
      }
      if (!VOID.has(tag)) cur = el;
      continue;
    }
    const next = html.indexOf("<", at + 1); const end = next < 0 ? html.length : next;
    cur.append(new Text_(decode(html.slice(at, end)))); at = end;
  }
}
function serialize(n) {
  if (n instanceof Text_) return n.data.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  if (n instanceof Comment_) return "<!--" + n.data + "-->";
  const attrs = [...n.attrs].map(([k, v]) => (v === "" ? " " + k : ` ${k}="${v.replace(/"/g, "&quot;")}"`)).join("");
  return `<${n.localName}${attrs}>` + (VOID.has(n.localName) ? "" : n.childNodes.map(serialize).join("") + `</${n.localName}>`);
}

class Event { constructor(type, init = {}) { this.type = type; this.bubbles = !!init.bubbles; this.cancelable = !!init.cancelable; this.defaultPrevented = false; this.stopped = false; }
  preventDefault() { if (this.cancelable) this.defaultPrevented = true; } stopPropagation() { this.stopped = true; } stopImmediatePropagation() { this.stopped = true; } }
class CustomEvent extends Event { constructor(type, init = {}) { super(type, init); this.detail = init.detail ?? null; } }

const document = new HTMLElement("#document");
Object.defineProperty(document, "parentNode", { value: null, writable: true });
const html = make("html"), head = make("head"), body = make("body");
html.append(head, body); document.append(html);
document.documentElement = html; document.head = head; document.body = body;
let cookie = scenario.cookie ?? "";
Object.defineProperty(document, "cookie", { get: () => cookie, set: (v) => { const pair = String(v).split(";")[0]; const name = pair.split("=")[0]; cookie = [...cookie.split(";").map((c) => c.trim()).filter((c) => c && !c.startsWith(name + "=")), pair].join("; "); } });
document.createElement = make;
document.createComment = (d) => new Comment_(d);
document.createTextNode = (d) => new Text_(d);
document.getElementById = (id) => document.querySelector("#" + id);
document.getSelection = () => ({ rangeCount: 0, anchorNode: null, removeAllRanges() {}, addRange() {} });
document.execCommand = () => false;
const window = new Node_();
const assigned = [];
window.location = { hash: "", href: "http://127.0.0.1:8765/", pathname: "/", search: "", assign(url) { assigned.push(url); }, reload() {} };
Object.assign(globalThis, { window, document, HTMLElement, HTMLFormElement, HTMLInputElement, HTMLButtonElement,
  HTMLTextAreaElement, HTMLSelectElement, HTMLOptionElement, HTMLDetailsElement, HTMLStyleElement, Event, CustomEvent,
  Node: Node_, Text: Text_, Comment: Comment_, Element: HTMLElement,
  MutationObserver: class { observe() {} disconnect() {} takeRecords() { return []; } },
  ResizeObserver: class { observe() {} disconnect() {} unobserve() {} },
  requestAnimationFrame: (fn) => setTimeout(fn, 0), getComputedStyle: () => ({ getPropertyValue: () => "" }),
  matchMedia: () => ({ matches: false, addEventListener() {} }), confirm: () => true, alert: () => {},
  localStorage: { getItem: () => null, setItem() {}, removeItem() {} } });
globalThis.fetch = async (url, init) => {
  const request = JSON.parse(init?.body ?? "{}");
  calls.push({ method: request.method ?? String(url), params: request.params ?? null });
  const result = (scenario.answers ?? {})[request.method] ?? {};
  return { ok: true, status: 200, json: async () => ({ jsonrpc: "2.0", id: request.id, result }) };
};

const host = make("div"); body.append(host);
host.innerHTML = scenario.html;
const dir = process.argv[3];
const widgets = await import(dir + "/widgets.js");
await import(dir + "/cover_editor.js");
widgets.mount(host);
const settle = async () => { for (let i = 0; i < 20; i++) await new Promise((r) => setTimeout(r, 0)); };
const root = host.querySelector("[data-widget=cover-editor]");
const frame = (kind) => root.querySelector(`[data-cover-block="${kind}"]`);
const input = (kind, field) => root.querySelector(`[name="b.${kind}.${field}"]`);
const look = (kind) => {
  const f = frame(kind);
  return { x: Number(f.dataset.x), y: Number(f.dataset.y), w: Number(f.dataset.w), snapped: f.dataset.snapped ?? "",
    color: f.dataset.color, aria: f.getAttribute("aria-label"), left: f.style.left, top: f.style.top, width: f.style.width,
    inputX: input(kind, "x_mm")?.value ?? null, inputY: input(kind, "y_mm")?.value ?? null,
    inputW: input(kind, "width_mm")?.value ?? null, inputColor: input(kind, "color")?.value ?? null };
};
const pointer = (target, type, x, y) => {
  const e = new Event(type, { bubbles: true, cancelable: true });
  Object.assign(e, { clientX: x, clientY: y, button: 0, pointerId: 1 });
  target.dispatchEvent(e);
};
const key = (target, k, mods = {}) => {
  const e = new Event("keydown", { bubbles: true, cancelable: true });
  Object.assign(e, { key: k, shiftKey: !!mods.shift, ctrlKey: !!mods.ctrl, metaKey: false, altKey: false });
  target.dispatchEvent(e);
  return e.defaultPrevented;
};
const click = (target) => target.dispatchEvent(new Event("click", { bubbles: true, cancelable: true }));
const out = { ready: root.classList.contains("cover-editor-ready"), steps: [] };
const record = (name, kind = "front_title") => out.steps.push({ name, ...look(kind),
  undoDisabled: root.querySelector("[data-cover-undo]").disabled, saveDisabled: root.querySelector("[data-cover-save]").disabled,
  dirty: root.dataset.dirty ?? "", status: root.querySelector("[data-cover-status]").textContent });
record("start");
for (const step of scenario.steps) {
  const kind = step.kind ?? "front_title";
  const f = frame(kind);
  if (step.type === "drag") {
    const target = step.handle ? f.querySelector(`[data-cover-handle="${step.handle}"]`) : f;
    pointer(target, "pointerdown", 100, 100);
    pointer(f, "pointermove", 100 + step.dx / 2, 100 + step.dy / 2);
    pointer(f, "pointermove", 100 + step.dx, 100 + step.dy);
    pointer(f, "pointerup", 100 + step.dx, 100 + step.dy);
  } else if (step.type === "key") {
    step.prevented = key(f, step.key, step);
  } else if (step.type === "click") {
    if (step.select) f.dispatchEvent(new Event("focus"));
    click(root.querySelector(step.selector));
  } else if (step.type === "input") {
    const field = input(kind, step.field);
    field.value = step.value;
    field.dispatchEvent(new Event("change", { bubbles: true }));
  }
  await settle();
  record(step.name, kind);
}
out.calls = calls;
out.assigned = assigned;
out.errors = errors;
process.stdout.write(JSON.stringify(out));
"#####;
