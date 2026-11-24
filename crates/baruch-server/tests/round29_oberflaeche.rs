// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satz-Einstellungen, Oberfläche (Runde 29c, Vertrag §8.3): Block „Satz“ ohne JavaScript, 422 mit erhaltenen Eingaben,
//! Vorschau (work.typeset_preview, Vorschaubilder), Vorlagen-Angebot, fremde Herkunft, reine Funktionen der Widgets
//! (typeset_settings.ts, xref.ts) unter Node.
//!
//! Besitz: satz-oberflaeche. Kein Netz, kein Eliezer. Den Erfolgsweg von Speichern, Zurücksetzen und Übernehmen prüft
//! der Integrator (§8.4); hier nur, was vor dem Speichern geschieht – ohne Ausweichzweige. Alle Werke frei erfunden;
//! jedes Werk hat einen eigenen Titel, damit seine Vorschau einen eigenen Stand im prozessweiten Zwischenspeicher hat.

use crate::common;

use std::process::Command;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_server::settings::work_typeset::settings_from_form;
use baruch_typst::typeset::{CANONICAL_CHAPTER_BREAK, FIELDS, TypesetSettings};
use serde_json::{Value, json};

/// Erfundenes Buch ohne eigene Kapitelanfangs-Regel.
const PLAIN: &str = "#set document(title: \"Das Muschelbuch\", author: \"Lene Probst\")\n\
#set text(lang: \"de\")\n\n= Ebbe\n\nDas Wasser zieht sich zurück.\n\n= Flut\n\nEs kommt wieder.\n";

/// Erfundenes Buch, dessen Vorlage den Kapitelanfang selbst regelt (kanonische Zeile, Zeile 3).
fn ruled() -> String {
    format!(
        "#set document(title: \"Das Leuchtfeuerbuch\", author: \"Ole Tamm\")\n#set text(lang: \"de\")\n\
         {CANONICAL_CHAPTER_BREAK}\n\n= Turm\n\nDie Lampe brennt.\n\n= Hafen\n\nDie Boote liegen still.\n"
    )
}

fn ok(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Nur der Block „Satz“ aus einer Einstellungsseite (vom Anker bis zum nächsten Block „Abgleich“).
fn satz_block(page: &str) -> &str {
    let start = page.find("id=\"satz\"").unwrap_or_else(|| panic!("Block „Satz“ fehlt: {page}"));
    let end = page[start..].find("id=\"abgleich\"").map_or(page.len(), |e| start + e);
    &page[start..end]
}

async fn post(server: &common::Server, uri: &str, form: &str) -> (StatusCode, Option<String>, String) {
    let request = Request::post(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.to_owned()))
        .unwrap();
    let (status, headers, body) = server.send(request).await;
    (status, headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()), body)
}

async fn typeset_get(server: &common::Server, work: i64) -> Value {
    ok(&server.rpc("work.typeset_get", json!({ "work": work })).await).clone()
}

fn commits(server: &common::Server, slug: &str) -> Option<u32> {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).and_then(|c| c.trim().parse().ok())
}

async fn slug(server: &common::Server, work: i64) -> String {
    server.state.store.work_slug(work).await.unwrap()
}

#[tokio::test]
async fn the_block_shows_all_sixteen_fields_in_groups_without_javascript() {
    let server = common::open().await;
    let work = server.import(PLAIN, "muschel.typ").await.work;
    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Einstellungsseite ohne JavaScript: {page}");
    let block = satz_block(&page);
    assert!(!block.contains("Noch nicht umgesetzt"), "kein Stub mehr: {block}");
    assert!(block.contains("<h3>Satz</h3>"), "Überschrift des Blocks");
    // Seit Runde 31 stehen auch Glossarseite, Registerseiten und erste Erwähnung im Block (S1-Ausnahme entfallen).
    for (id, label) in FIELDS.iter() {
        // S1-Ausnahme Runde 22: „Entwurf“ (draft) bekommt sein Formular erst mit spiegel-entwurf (dann entfernen).
        if *id == "draft" {
            continue;
        }
        // minijinja maskiert „/“ als &#x2f; („Schusterjungen/Hurenkinder“).
        let label = label.replace('/', "&#x2f;");
        assert!(block.contains(&label), "Bezeichnung „{label}“ ({id}) fehlt im Block: {block}");
    }
    for legend in [
        "Kapitel und Teile",
        "Seite und Ränder",
        "Schrift und Absatz",
        "Seitenzahlen, Kolumnentitel, Fußnoten",
        "Querverweise",
    ] {
        assert!(block.contains(&format!("<legend class=\"small\">{legend}</legend>")), "Gruppe „{legend}“ fehlt");
    }
    // Jedes Formularfeld aus Vertrag §6.3 steht im Block (ohne JavaScript bedienbar).
    for name in [
        "chapter_start",
        "part_side",
        "part_blank_back",
        "page_format",
        "page_width_mm",
        "page_height_mm",
        "margin_inside_mm",
        "margin_outside_mm",
        "margin_top_mm",
        "margin_bottom_mm",
        "binding_mm",
        "font",
        "font_size_pt",
        "leading_em",
        "paragraph",
        "paragraph_em",
        "justify",
        "hyphenate",
        "widows_orphans",
        "xref_with_page",
        "lang",
        "page_number_position",
        "page_number_start",
        "running_head",
        "footnote_numbering",
        "footnote_restart",
    ] {
        assert!(block.contains(&format!("name=\"{name}\"")), "Formularfeld {name} fehlt: {block}");
    }
    // Elf Auswahllisten mit erster Wahl „aus der Werkvorlage“: sieben Listen (Kapitelanfang, Teil-Seite, Seitenformat,
    // Absatz, Seitenzahlen, Kolumnentitel, Fußnoten) und vier Schalter (Blocksatz, Silbentrennung,
    // Schusterjungen/Hurenkinder, Verweis mit Seitenzahl); die Zählweisen gelten nur mit gewählter Hauptliste.
    let first_choices = block.matches("<option value=\"\" selected>aus der Werkvorlage</option>").count();
    assert_eq!(first_choices, 11, "ohne Einstellungen ist überall „aus der Werkvorlage“ gewählt: {block}");
    assert!(block.contains("placeholder=\"Vorlage (Typst: 11)\""), "Platzhalter mit dem Vorgabewert der Schriftgröße");
    assert!(block.contains("<option value=\"right_page\">neue rechte Seite</option>"), "deutsche Bezeichnung der Wahl");
    assert!(block.contains("<option value=\"bottom_outside\">unten außen</option>"), "Seitenzahl unten außen");
    assert!(block.contains("action=\"/einstellungen/werk/satz\""), "Formular geht an die Route ohne JavaScript");
    assert!(block.contains("name=\"action\" value=\"preview\""), "Knopf „Vorschau“");
    assert!(block.contains("name=\"action\" value=\"save\""), "Knopf „Speichern“");
    assert!(block.contains("name=\"action\" value=\"reset\""), "Zurücksetzen als eigenes Formular");
    assert!(block.contains("name=\"confirm\" value=\"1\" required"), "Zurücksetzen nur mit Bestätigung");
    assert!(block.contains("<details class=\"typeset-rules\">"), "erzeugte Typst-Regeln in <details>");
    assert!(block.contains("Keine – alles kommt aus der Werkvorlage."), "ohne Einstellungen keine Regel");
    assert!(block.contains(&format!("href=\"/werke/{work}/satz\"")), "Link „Ganzes Werk im Satz ansehen“: {block}");
    assert!(block.contains("data-widget=\"typeset-settings\""), "Widget für die Vorschau ohne Neuladen");
    assert!(!block.contains("/einstellungen/werk/satz/vorlage"), "ohne Vorlagenregel kein Angebot");
    assert!(!block.contains("{{") && !block.contains("{%"), "keine Reste der Vorlage: {block}");
}

/// Befund R29: Die Satzbegriffe Bundsteg, Schusterjunge/Hurenkind und Kolumnentitel sind beim Feld kurz erklärt und
/// über `aria-describedby` mit ihm verbunden (Bildschirmleser lesen die Erklärung zum Feld vor) – ohne JavaScript.
#[tokio::test]
async fn the_typesetting_terms_are_explained_at_their_fields() {
    let server = common::open().await;
    let work = server.import(PLAIN, "muschel-begriffe.typ").await.work;
    let (status, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "Einstellungsseite: {page}");
    let block = satz_block(&page);
    for (field, help, words) in [
        ("binding_mm", "typeset-help-binding", ["Bindung", "Innenrand"]),
        ("widows_orphans", "typeset-help-widows", ["erste Zeile eines Absatzes", "letzte Zeile eines Absatzes"]),
        ("running_head", "typeset-help-head", ["Kopfzeile", "laufenden Kapitels"]),
    ] {
        let at = block.find(&format!("name=\"{field}\"")).unwrap_or_else(|| panic!("Feld {field} fehlt"));
        let tag = &block[at..at + block[at..].find('>').unwrap()];
        assert!(tag.contains(&format!("aria-describedby=\"{help}\"")), "{field} verweist auf seine Erklärung: {tag}");
        let start = block.find(&format!("id=\"{help}\"")).unwrap_or_else(|| panic!("Erklärung {help} fehlt: {block}"));
        let text = &block[start..start + block[start..].find("</span>").unwrap()];
        for word in words {
            assert!(text.contains(word), "{help}: die Erklärung nennt „{word}“: {text}");
        }
    }
}

#[tokio::test]
async fn invalid_input_returns_422_with_kept_values_and_a_message_per_field() {
    let server = common::open().await;
    let work = server.import(PLAIN, "muschel.typ").await.work;
    let before = commits(&server, &slug(&server, work).await);
    let form = format!(
        "work={work}&action=save&chapter_start=right_page&font=Probe+Antiqua&font_size_pt=30&margin_inside_mm=20\
         &margin_top_mm=18&lang=de-CH"
    );
    let (status, location, page) = post(&server, "/einstellungen/werk/satz", &form).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültig → 422: {page}");
    assert!(location.is_none(), "keine Weiterleitung, die Eingabe bliebe sonst nicht stehen");
    let block = satz_block(&page);
    assert!(block.contains("value=\"Probe Antiqua\""), "Schrift bleibt im Feld: {block}");
    assert!(block.contains("value=\"30\""), "die abgelehnte Schriftgröße bleibt im Feld");
    assert!(block.contains("value=\"de-CH\""), "gültige Werte bleiben ebenfalls");
    assert!(block.contains("<option value=\"right_page\" selected>"), "gewählter Kapitelanfang bleibt gewählt");
    assert!(
        block.contains("data-typeset-error=\"font_size_pt\">Schriftgröße: 6 bis 24 pt<"),
        "Meldung beim Feld Schriftgröße, wörtlich wie TypesetSettings::validate: {block}"
    );
    assert!(
        block.contains("data-typeset-error=\"margins\">Ränder: alle vier Ränder angeben oder alle leer<"),
        "Meldung bei den Rändern: {block}"
    );
    let size_field = block.find("name=\"font_size_pt\"").unwrap();
    let size_error = block.find("data-typeset-error=\"font_size_pt\"").unwrap();
    let leading_field = block.find("name=\"leading_em\"").unwrap();
    assert!(size_field < size_error && size_error < leading_field, "die Meldung steht beim Feld, nicht am Ende");
    let read = typeset_get(&server, work).await;
    assert_eq!(read["set"], false, "nichts gespeichert: {read}");
    assert_eq!(commits(&server, &slug(&server, work).await), before, "kein Spiegel-Commit");
}

#[tokio::test]
async fn reset_and_adopting_the_template_rule_need_the_confirmation() {
    let server = common::open().await;
    let source = ruled();
    let work = server.import(&source, "leuchtfeuer.typ").await.work;
    let template = server.state.store.work_template(work).await.unwrap();
    assert!(template.contains(CANONICAL_CHAPTER_BREAK), "Voraussetzung: die Zeile steht in der Werkvorlage");
    let before = commits(&server, &slug(&server, work).await);

    let (status, _, page) = post(&server, "/einstellungen/werk/satz", &format!("work={work}&action=reset")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Zurücksetzen ohne Häkchen → 422");
    assert!(
        satz_block(&page).contains("Zurücksetzen bitte mit dem Häkchen bestätigen; nichts geändert."),
        "Hinweis im Block: {page}"
    );

    let (status, _, page) = post(&server, "/einstellungen/werk/satz/vorlage", &format!("work={work}")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Übernahme ohne Häkchen → 422");
    let block = satz_block(&page);
    assert!(
        block.contains("data-typeset-error=\"template_rule\">Übernehmen bitte mit dem Häkchen bestätigen"),
        "Hinweis beim Angebot: {block}"
    );
    assert_eq!(
        server.state.store.work_template(work).await.unwrap(),
        template,
        "ohne Bestätigung bleibt die Werkvorlage unverändert"
    );
    assert_eq!(typeset_get(&server, work).await["set"], false, "keine Einstellungen gespeichert");
    assert_eq!(commits(&server, &slug(&server, work).await), before, "kein Spiegel-Commit");
}

#[tokio::test]
async fn the_offer_appears_with_line_and_text_of_the_canonical_template_rule() {
    let server = common::open().await;
    let work = server.import(&ruled(), "leuchtfeuer.typ").await.work;
    let read = typeset_get(&server, work).await;
    let line = read["template_rule"]["line"].as_u64().unwrap_or_else(|| panic!("Regel erkannt: {read}"));
    let (_, page) = server.get(&format!("/einstellungen/werk?work={work}")).await;
    let block = satz_block(&page);
    assert!(block.contains("action=\"/einstellungen/werk/satz/vorlage\""), "Angebot als Formular ohne JavaScript");
    assert!(block.contains(&format!("(Zeile {line})")), "Zeile der Regel aus work.typeset_get: {block}");
    // minijinja maskiert u. a. " und > (aus „it => …“).
    let escaped = CANONICAL_CHAPTER_BREAK.replace('"', "&quot;").replace('>', "&gt;");
    assert!(block.contains(&format!("<code>{escaped}</code>")), "Text der Regel, maskiert: {block}");
    assert!(block.contains("„neue rechte Seite“"), "der Regelwert in Worten (to: \"odd\" → neue rechte Seite)");
    let offer = &block[block.find("data-typeset-adopt").unwrap()..];
    assert!(offer.contains("name=\"confirm\" value=\"1\" required"), "Pflicht-Häkchen vor der Übernahme");
}

/// Erstes Vorschaubild in einer Seite: `/einstellungen/werk/satz/vorschau/<stand>/1.svg`.
fn first_image(page: &str) -> String {
    let marker = "<img src=\"/einstellungen/werk/satz/vorschau/";
    let at = page.find(marker).unwrap_or_else(|| panic!("kein Vorschaubild: {page}")) + "<img src=\"".len();
    page[at..at + page[at..].find('"').unwrap()].to_owned()
}

#[tokio::test]
async fn the_preview_works_without_javascript_and_serves_only_cached_pages() {
    let server = common::open().await;
    let work = server.import(PLAIN, "muschel-vorschau.typ").await.work;
    let before = commits(&server, &slug(&server, work).await);
    let form = format!("work={work}&action=preview&chapter_start=new_page&font_size_pt=10%2C5&page_format=a5");
    let (status, location, page) = post(&server, "/einstellungen/werk/satz", &form).await;
    assert_eq!(status, StatusCode::OK, "Vorschau ohne JavaScript: ganze Seite mit 200: {page}");
    assert!(location.is_none(), "keine Weiterleitung");
    let block = satz_block(&page);
    assert!(block.contains("<option value=\"new_page\" selected>"), "ungespeicherte Wahl bleibt stehen: {block}");
    assert!(block.contains("value=\"10,5\""), "Dezimalkomma bleibt, wie eingegeben");
    assert!(block.contains("Ungespeicherte Werte"), "Hinweis, dass nichts gespeichert ist");
    assert!(block.contains("data-typeset-preview-info"), "Angabe zur Vorschau (Seitenzahl)");
    let url = first_image(block);
    let stand = url.trim_start_matches("/einstellungen/werk/satz/vorschau/").trim_end_matches("/1.svg").to_owned();
    assert_eq!(stand.len(), 64, "Stand als Hash in der Adresse: {url}");

    let (status, headers, svg) = server.send(Request::get(&url).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "Vorschaubild aus dem Zwischenspeicher: {svg}");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "SVG-Kopf");
    // Der Router hängt allen Antworten noch „frame-ancestors 'none'“ an; die SVG-Regel steht vorn.
    let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(csp.starts_with(baruch_server::typeset::SVG_CSP), "SVG ohne Skripte (CSP): {csp}");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff", "kein Raten des Typs");
    assert!(svg.contains("<svg"), "eine SVG-Seite");

    let cross = Request::get(&url).header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap();
    assert_eq!(server.send(cross).await.0, StatusCode::FORBIDDEN, "fremde Seite → 403");
    let same_site = Request::get(&url).header("sec-fetch-site", "same-site").body(Body::empty()).unwrap();
    assert_eq!(server.send(same_site).await.0, StatusCode::FORBIDDEN, "Nachbar-Site → 403");
    let unknown = format!("/einstellungen/werk/satz/vorschau/{}/1.svg", "0".repeat(64));
    assert_eq!(server.get(&unknown).await.0, StatusCode::NOT_FOUND, "nie gesetzter Stand → 404 (kein Satzlauf)");
    let beyond = format!("/einstellungen/werk/satz/vorschau/{stand}/999.svg");
    assert_eq!(server.get(&beyond).await.0, StatusCode::NOT_FOUND, "Seite außerhalb → 404");
    let odd = format!("/einstellungen/werk/satz/vorschau/{stand}/eins.svg");
    assert_eq!(server.get(&odd).await.0, StatusCode::NOT_FOUND, "keine Seitenzahl → 404");

    assert_eq!(typeset_get(&server, work).await["set"], false, "die Vorschau speichert nichts");
    assert_eq!(commits(&server, &slug(&server, work).await), before, "kein Spiegel-Commit durch die Vorschau");
}

#[tokio::test]
async fn the_rpc_preview_answers_with_page_urls_and_validates() {
    let server = common::open().await;
    let work = server.import(PLAIN, "muschel-rpc.typ").await.work;
    let before = commits(&server, &slug(&server, work).await);
    let settings = json!({ "chapter_start": "right_page", "page_format": { "kind": "a5" }, "font_size_pt": 10.5,
                           "paragraph": { "kind": "spacing", "em": 0.6 }, "xref_with_page": true });
    let answer = server.rpc("work.typeset_preview", json!({ "work": work, "settings": settings })).await;
    let result = ok(&answer);
    let stand = result["stand"].as_str().unwrap();
    let pages = result["pages"].as_u64().unwrap();
    assert!(pages >= 1, "die Vorschau hat Seiten: {result}");
    let urls: Vec<&str> = result["urls"].as_array().unwrap().iter().map(|u| u.as_str().unwrap()).collect();
    assert_eq!(urls.len() as u64, pages, "eine Adresse je Seite");
    assert_eq!(urls[0], format!("/einstellungen/werk/satz/vorschau/{stand}/1.svg"), "Form der Adresse (§6.1)");
    assert_eq!(result["errors"], 0, "der Mustertext setzt ohne Fehler: {result}");
    assert!(result["diagnostics"].is_array() && result["conflicts"].is_array(), "Felder diagnostics, conflicts");
    assert!(result["warnings"].is_u64(), "Zahl der Warnungen");
    let (status, _) = server.get(urls[urls.len() - 1]).await;
    assert_eq!(status, StatusCode::OK, "die letzte Seite liegt im Zwischenspeicher");

    let refused = |answer: &Value, why: &str| {
        assert_eq!(answer["error"]["code"], -32602, "{why}: {answer}");
        answer["error"]["message"].as_str().unwrap_or_default().to_owned()
    };
    let too_big = server.rpc("work.typeset_preview", json!({ "work": work, "settings": { "font_size_pt": 40 } })).await;
    assert!(refused(&too_big, "außerhalb der Grenzen").contains("Schriftgröße: 6 bis 24 pt"), "Meldung mit Grenze");
    let unknown = server.rpc("work.typeset_preview", json!({ "work": work, "settings": { "farbe": "rot" } })).await;
    refused(&unknown, "unbekanntes Feld");
    let missing = server.rpc("work.typeset_preview", json!({ "work": work })).await;
    assert!(refused(&missing, "settings fehlt").contains("settings"), "Meldung nennt das fehlende Feld");
    assert_eq!(commits(&server, &slug(&server, work).await), before, "work.typeset_preview schreibt nichts");
    assert_eq!(typeset_get(&server, work).await["set"], false, "nichts gespeichert");
}

#[tokio::test]
async fn every_new_post_refuses_a_foreign_origin() {
    let server = common::open().await;
    let work = server.import(&ruled(), "leuchtfeuer.typ").await.work;
    for (uri, form) in [
        ("/einstellungen/werk/satz", format!("work={work}&action=save&chapter_start=new_page")),
        ("/einstellungen/werk/satz", format!("work={work}&action=preview")),
        ("/einstellungen/werk/satz", format!("work={work}&action=reset&confirm=1")),
        ("/einstellungen/werk/satz/vorlage", format!("work={work}&confirm=1")),
    ] {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::HOST, "127.0.0.1:8765")
            .header(header::ORIGIN, "https://evil.example")
            .body(Body::from(form.clone()))
            .unwrap();
        let (status, _, _) = server.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri} ({form}) mit fremder Herkunft → 403 (guard::same_origin)");
    }
    assert!(
        server.state.store.work_template(work).await.unwrap().contains(CANONICAL_CHAPTER_BREAK),
        "die Vorlage ist unverändert"
    );
    assert_eq!(typeset_get(&server, work).await["set"], false, "nichts gespeichert");
}

#[tokio::test]
async fn an_unknown_work_or_action_changes_nothing() {
    let server = common::open().await;
    let (status, _, _) = post(&server, "/einstellungen/werk/satz", "work=987654&action=save").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404, nie still ein anderes Werk");
    let work = server.import(PLAIN, "muschel.typ").await.work;
    let (status, _, _) = post(&server, "/einstellungen/werk/satz", &format!("work={work}&action=loeschen")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Aktion → 400");
}

// ---- Widgets unter Node -----------------------------------------------------------------------

/// Ruft einen Ausdruck mit den ausgelieferten Modulen unter Node auf (alle Skripte im selben Ordner, damit die Importe
/// aufgehen) und liefert das Ergebnis als JSON (Muster aus `round30_live.rs`).
fn node(module: &str, expr: &str) -> Value {
    let dir = tempfile::tempdir().unwrap();
    for (name, source) in baruch_server::SCRIPTS {
        std::fs::write(dir.path().join(name.replace(".js", ".mjs")), source.replace(".js\"", ".mjs\"")).unwrap();
    }
    let script =
        format!("import * as m from './{}';\nconsole.log(JSON.stringify({expr}));\n", module.replace(".js", ".mjs"));
    std::fs::write(dir.path().join("probe.mjs"), script).unwrap();
    let out = Command::new("node").arg("probe.mjs").current_dir(dir.path()).output().expect("node läuft (wie für tsc)");
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Formular als JavaScript-Liste `[[name, wert], …]`.
fn js_pairs(form: &[(&str, &str)]) -> String {
    serde_json::to_string(&form.iter().map(|(k, v)| [*k, *v]).collect::<Vec<_>>()).unwrap()
}

#[test]
fn the_widget_reads_the_form_exactly_like_the_server() {
    let forms: [&[(&str, &str)]; 5] = [
        // Alle Felder gesetzt, mit Dezimalkomma und eigenem Format.
        &[
            ("work", "3"),
            ("chapter_start", "right_page"),
            ("part_side", "right"),
            ("part_blank_back", "1"),
            ("page_format", "custom"),
            ("page_width_mm", "125,5"),
            ("page_height_mm", "200"),
            ("margin_inside_mm", "20"),
            ("margin_outside_mm", "15"),
            ("margin_top_mm", "18"),
            ("margin_bottom_mm", "22,5"),
            ("binding_mm", "5"),
            ("font", " Libertinus Serif "),
            ("font_size_pt", "10,5"),
            ("leading_em", "0.65"),
            ("paragraph", "indent"),
            ("paragraph_em", "1,2"),
            ("justify", "on"),
            ("hyphenate", "off"),
            ("lang", "de-CH"),
            ("page_number_position", "bottom_outside"),
            ("page_number_start", "first_chapter"),
            ("running_head", "chapter"),
            ("footnote_numbering", "symbols"),
            ("footnote_restart", "per_chapter"),
            ("widows_orphans", "on"),
            ("xref_with_page", "off"),
            // Runde 22: Entwurf mit allen Feldern (Häkchen, Text, Zusatz, Datum, Deckkraft, Lage, Umschlag).
            ("draft", "1"),
            ("draft_text", " Vorabzug "),
            ("draft_note", "für die Lektorin"),
            ("draft_date", "1"),
            ("draft_opacity", "25"),
            ("draft_placement", "header"),
            ("draft_cover", "1"),
        ],
        // Runde 22: Entwurf mit leeren Feldern → Vorgaben („ENTWURF“, 12 %, diagonal); ohne Häkchen kein Entwurf.
        &[("draft", "1"), ("draft_text", ""), ("draft_opacity", ""), ("draft_placement", "")],
        &[("draft_text", "Vorabzug"), ("draft_opacity", "25")],
        // Lauter „aus der Werkvorlage“, Unterfelder ohne Hauptwahl zählen nicht.
        &[
            ("chapter_start", ""),
            ("part_side", ""),
            ("part_blank_back", "1"),
            ("page_format", ""),
            ("page_number_position", ""),
            ("page_number_start", "first_chapter"),
            ("footnote_numbering", ""),
            ("footnote_restart", "per_chapter"),
            ("binding_mm", ""),
            ("justify", ""),
        ],
        // Vorgaben der Unterfelder: Abstand ohne Maß, Seitenzahlen ohne Zählung, Bundsteg leer.
        &[
            ("paragraph", "spacing"),
            ("page_number_position", "hidden"),
            ("footnote_numbering", "letters"),
            ("margin_inside_mm", "18"),
            ("margin_outside_mm", "14"),
            ("margin_top_mm", "16"),
            ("margin_bottom_mm", "20"),
            ("page_format", "a4"),
        ],
    ];
    for form in forms {
        let owned: Vec<(String, String)> = form.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        let server = settings_from_form(&owned).expect("gültiges Formular");
        let widget = node("typeset_settings.js", &format!("m.formToSettings({})", js_pairs(form)));
        assert_eq!(widget["errors"], json!([]), "das Widget meldet keinen Fehler: {widget}");
        let read: TypesetSettings =
            serde_json::from_value(widget["settings"].clone()).expect("das Widget schickt gültiges JSON");
        // Gleiches Formular → gleiche Einstellungen: die Vorschau zeigt, was „Speichern“ speichern würde.
        assert_eq!(read, server, "Widget und settings_from_form lesen das Formular gleich: {widget}");
    }
    let partial = node("typeset_settings.js", &format!("m.formToSettings({})", js_pairs(&[("binding_mm", "5")])));
    assert_eq!(
        partial["errors"][0]["message"], "Ränder: alle vier Ränder angeben oder alle leer",
        "unvollständige Ränder meldet das Widget wie der Server: {partial}"
    );
}

#[test]
fn the_preview_is_debounced_and_only_the_newest_request_counts() {
    // Nachgebaute Zeitgeber: set merkt sich die Aufgabe, clear verwirft sie, fire führt die offenen aus.
    let result = node(
        "typeset_settings.js",
        "(() => { const tasks = new Map(); let id = 0; \
           const timers = { set: (run, ms) => { tasks.set(++id, run); return id; }, clear: (h) => tasks.delete(h) }; \
           const fire = () => { const all = [...tasks.values()]; tasks.clear(); all.forEach((r) => r()); }; \
           let runs = 0; const d = m.debounce(() => runs++, m.PREVIEW_DELAY_MS, timers); \
           d.trigger(); d.trigger(); d.trigger(); const pending = tasks.size; fire(); const afterBurst = runs; \
           d.trigger(); d.now(); const afterNow = runs; const leftover = tasks.size; \
           const n = new m.Newest(); const a = n.start(); const b = n.start(); \
           return { pending, afterBurst, afterNow, leftover, aAborted: a.signal.aborted, aCurrent: n.isCurrent(a), \
                    bAborted: b.signal.aborted, bCurrent: n.isCurrent(b), delay: m.PREVIEW_DELAY_MS }; })()",
    );
    assert_eq!(result["pending"], 1, "drei schnelle Eingaben → eine wartende Vorschau: {result}");
    assert_eq!(result["afterBurst"], 1, "nach der Pause genau ein Lauf");
    assert_eq!(result["afterNow"], 2, "„Vorschau“ setzt sofort");
    assert_eq!(result["leftover"], 0, "und verwirft die wartende Pause (kein doppelter Lauf)");
    assert_eq!(result["aAborted"], true, "eine neue Anfrage bricht die alte ab");
    assert_eq!(result["aCurrent"], false, "die Antwort der alten wird verworfen");
    assert_eq!(result["bAborted"], false, "die neueste läuft");
    assert_eq!(result["bCurrent"], true, "und ihre Antwort zählt");
    assert!(result["delay"].as_u64().unwrap() >= 300, "Pause lang genug gegen einen Satzlauf je Tastendruck");
}

#[test]
fn the_reference_dialog_starts_with_the_work_setting() {
    let cases = node(
        "xref.js",
        "[m.initialPage(undefined, true, false), m.initialPage(undefined, false, true), \
          m.initialPage(undefined, null, true), m.initialPage(undefined, undefined, false), \
          m.initialPage(false, true, true)]",
    );
    // Werk-Einstellung vor Browser-Gedächtnis; ein vorhandener Verweis (Bearbeiten) behält seine Form.
    assert_eq!(cases, json!([true, false, true, false, false]), "Anfangswert des Häkchens „mit Seitenzahl“");
    let source = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "xref.js").unwrap().1;
    assert!(source.contains("work.typeset_get"), "der Dialog liest die Werk-Einstellung über work.typeset_get");
    let read = source.find("localStorage.getItem").expect("Browser-Gedächtnis bleibt für ungesetzte Werke");
    let guarded = source[..read].rfind("try {").expect("localStorage nur in try/catch");
    assert!(read - guarded < 80, "der Zugriff steht direkt im try-Block");
}
