// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 25 „karte-widget“: Widget der Gedankenkarte (DOM-Vertrag, ausgeliefertes Skript) und Verbindungen und
//! Lagen im Austauschformat (Unterversion 2, Vertrag §4.5/§4.6). Tests schreibt karte-widget.
//!
//! - Das Skript `/static/js/idea_map.js` wird ausgeliefert, registriert `idea-map`, liest nur Klassen und Attribute
//!   aus dem DOM-Vertrag §4.4 (dazu eigene, die es selbst anlegt) und ruft nur die erlaubten RPC-Methoden.
//! - Austausch: Rundreise A → B → A mit Verbindungen und Lagen über RPC; Verbindungen und Lagen „hier“ entstehen über
//!   SQL, weil die Schreibwege der Karte (karte-daten) in diesem Zweig noch Stubs sind. Die Fülltrigger aus 0043
//!   vergeben ihre Kennungen wie auf jedem anderen Weg.
//!
//! Keine Anfrage an Eliezer oder andere Dienste (kein Modell, keine Einbettung).

use std::collections::BTreeSet;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_exchange::sample::{sample_document, sample_uuid};
use baruch_exchange::write::{ImageMode, write_xml};
use serde_json::{Value, json};

use crate::common;

const SOURCE: &str = include_str!("../web/src/idea_map.ts");
const BOUNDARY: &str = "baruch-karte-grenze";

/// Erlaubte RPC-Methoden des Widgets (Vertrag §4.5: nur 4.1 und die vorhandenen idea.get/create/update).
const ALLOWED_METHODS: &[&str] =
    &["idea.get", "idea.create", "idea.update", "idea.link", "idea.map_pos_set", "idea.map_pos_reset"];

/// Klassen des DOM-Vertrags §4.4 (erzeugt karte-bild) …
const CONTRACT_CLASSES: &[&str] = &[
    "im-filter",
    "im-stage",
    "im-toolbar",
    "im-svg",
    "im-viewport",
    "im-edges",
    "im-edge",
    "im-tree",
    "im-ref",
    "im-link",
    "im-near",
    "im-nodes",
    "im-node",
    "im-work",
    "im-part",
    "im-chapter",
    "im-section",
    "im-empty",
    "im-none",
    "im-idea",
    "im-open",
    "im-done",
    "im-pinned",
    "im-hit",
    "im-dim",
    "im-side",
    "im-outline",
    "im-contradictions",
    "im-hide-near",
    // Runde 18a: sichtbare Beschriftung jeder Verbindung und Nähe-Kante (svg.rs), vom Widget beim Ziehen mitbewegt.
    "im-edge-label",
];
/// … und die, die das Widget selbst setzt oder anlegt (Stil in static/karte_widget.css).
const WIDGET_CLASSES: &[&str] = &[
    "im-ready",
    "im-status",
    "im-help",
    "im-zoom",
    "im-panning",
    "im-drag-active",
    "im-dragging",
    "im-drop-target",
    "im-source",
    "im-target",
    "im-connecting",
    "im-dialog",
    "im-kind",
    "im-note",
    "im-dialog-status",
    "im-dialog-actions",
    "im-side-form",
    "im-side-quote",
    "im-side-status",
    "im-side-actions",
];
/// data-Attribute des DOM-Vertrags und das eigene `data-im-zoom` der Zoom-Knöpfe.
const DATA_ATTRIBUTES: &[&str] = &[
    "data-widget",
    "data-work",
    "data-near",
    "data-im-toolbar",
    "data-im-svg",
    "data-im-viewport",
    "data-im-side",
    "data-from",
    "data-to",
    "data-link",
    "data-kind",
    "data-score",
    "data-node",
    "data-key",
    "data-open-ideas",
    "data-x",
    "data-y",
    "data-r",
    "data-idea",
    "data-anchor",
    "data-im-zoom",
    // Runde 18a: Kante mit Pfeilspitze (endet am Rand des Zielkreises).
    "data-arrow",
];

/// Alle Wörter `im-…` in Zeichenketten des Quelltexts (Klassen in Selektoren, classList, class-Attributen).
fn classes_in(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while let Some(at) = source[i..].find("im-") {
        let start = i + at;
        let before = if start == 0 { b' ' } else { bytes[start - 1] };
        let mut end = start + 3;
        while end < bytes.len() && (bytes[end].is_ascii_lowercase() || bytes[end] == b'-') {
            end += 1;
        }
        // Nur echte Bezeichner: davor kein Buchstabe/Bindestrich (nicht „data-im-…“, nicht „baruch_im_…“).
        if !(before.is_ascii_alphanumeric() || before == b'-' || before == b'_') {
            out.insert(source[start..end].trim_end_matches('-').to_owned());
        }
        i = end;
    }
    out
}

/// Alle `data-…`-Namen und `dataset.<name>` des Quelltexts, als `data-…`.
fn data_attributes_in(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let bytes = source.as_bytes();
    for (at, _) in source.match_indices("data-") {
        let mut end = at + 5;
        while end < bytes.len() && (bytes[end].is_ascii_lowercase() || bytes[end] == b'-') {
            end += 1;
        }
        out.insert(source[at..end].to_owned());
    }
    for (at, m) in source.match_indices("dataset.") {
        let mut end = at + m.len();
        while end < bytes.len() && bytes[end].is_ascii_alphabetic() {
            end += 1;
        }
        let camel = &source[at + m.len()..end];
        let mut name = String::from("data-");
        for c in camel.chars() {
            if c.is_ascii_uppercase() {
                name.push('-');
                name.push(c.to_ascii_lowercase());
            } else {
                name.push(c);
            }
        }
        out.insert(name);
    }
    out
}

/// Alle RPC-Methoden in `call("…")` bzw. `call<…>("…")`.
fn methods_in(source: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (at, _) in source.match_indices("call") {
        let rest = &source[at + 4..];
        let rest = if rest.starts_with('<') { &rest[rest.find('>').map_or(0, |i| i + 1)..] } else { rest };
        if let Some(arg) = rest.strip_prefix("(\"") {
            out.insert(arg[..arg.find('"').unwrap()].to_owned());
        }
    }
    out
}

#[tokio::test]
async fn the_widget_script_is_served_compiled_and_registers_idea_map() {
    let server = common::open().await;
    let (status, js) = server.get("/static/js/idea_map.js").await;
    assert_eq!(status, StatusCode::OK, "das übersetzte Widget wird ausgeliefert");
    assert!(js.contains("register(\"idea-map\""), "es registriert das Widget idea-map: {}", &js[..js.len().min(400)]);
    assert!(!js.contains(": SVGElement") && !js.contains("interface Drag"), "tsc hat die Typen entfernt (JavaScript)");
    let (_, main) = server.get("/static/js/main.js").await;
    assert!(main.contains("import \"./idea_map.js\""), "main.js lädt das Widget (S1)");
    // Der S1-Stub (R25c) ist weg; kein neuer Stub im Widget.
    assert!(!SOURCE.contains("STUB"), "kein Stub mehr im Widget");
    assert!(!js.contains("() => {})"), "kein leeres Widget mehr wie in S1");
}

#[tokio::test]
async fn the_widget_reads_only_the_dom_contract_and_calls_only_the_allowed_methods() {
    let server = common::open().await;
    let (_, js) = server.get("/static/js/idea_map.js").await;
    let methods = methods_in(&js);
    assert!(!methods.is_empty(), "Methoden gefunden: {methods:?}");
    for method in &methods {
        assert!(ALLOWED_METHODS.contains(&method.as_str()), "nur erlaubte RPC (Vertrag §4.5), nicht {method}");
    }
    for needed in ["idea.link", "idea.map_pos_set", "idea.map_pos_reset", "idea.update", "idea.get", "idea.create"] {
        assert!(methods.contains(needed), "das Widget nutzt {needed} (Verhalten nach §4.5): {methods:?}");
    }
    let classes = classes_in(&js);
    for class in &classes {
        assert!(
            CONTRACT_CLASSES.contains(&class.as_str()) || WIDGET_CLASSES.contains(&class.as_str()),
            "Klasse {class} gehört weder zum DOM-Vertrag §4.4 noch zu den eigenen des Widgets"
        );
    }
    let css = include_str!("../static/karte_widget.css");
    for own in WIDGET_CLASSES.iter().filter(|c| classes.contains(**c)) {
        let styled = css.contains(&format!(".{own}"));
        // Bloße Markierungen ohne eigenes Aussehen: im-kind/im-note sind Teil des Dialogs und dort gestaltet.
        assert!(styled, "eigene Klasse {own} ist in karte_widget.css gestaltet");
    }
    for attribute in data_attributes_in(&js) {
        assert!(DATA_ATTRIBUTES.contains(&attribute.as_str()), "Attribut {attribute} gehört nicht zum DOM-Vertrag");
    }
    // Strg+Mausrad bleibt zoom.ts, nur der Viewport bekommt ein transform.
    assert!(js.contains("event.ctrlKey"), "das Mausrad mit Strg wird nicht abgefangen");
    assert!(js.contains("g[data-im-viewport]"), "transform nur am Viewport des Vertrags");
}

#[test]
fn the_widget_implements_every_behaviour_of_the_contract() {
    // Belege im Quelltext je Punkt aus §4.5 (die Abläufe selbst prüft die Sichtprüfung des Integrators im Browser).
    for (what, needle) in [
        ("Zoom mit dem Mausrad ohne Strg", "\"wheel\""),
        ("Knöpfe + − Einpassen", "\"Einpassen\""),
        ("Tasten + − 0", "case \"0\":"),
        ("Verschieben auf leerer Fläche", "pan = { pointer"),
        ("Koordinaten über getScreenCTM des Viewports", "layer.getScreenCTM()"),
        ("Kanten live nachziehen", "`.im-edge[data-from=\"${id}\"], .im-edge[data-to=\"${id}\"]`"),
        // Runde 18a: Beschriftungen wandern mit, Pfeilkanten enden am Rand des Zielkreises (wie svg.rs).
        ("Beschriftungen live nachziehen", ".im-edge-label[data-from="),
        ("Pfeilkante endet am Kreisrand", "line.hasAttribute(\"data-arrow\")"),
        ("Ablage auf Kapitel → idea.update", "call(\"idea.update\", { id, key })"),
        ("Ablage auf Einfall → Dialog", "linkDialog(el, target)"),
        ("Lage merken → im-pinned", "el.classList.add(\"im-pinned\")"),
        ("Pfeile wandern", "ArrowLeft"),
        ("Enter öffnet die Seitenleiste", "openSide(node)"),
        ("V startet Verbinden", "case \"V\":"),
        ("Escape bricht ab", "case \"Escape\":"),
        ("N schaltet Nähe mit replaceState", "history.replaceState"),
        ("Klasse im-hide-near", "\"im-hide-near\""),
        ("Alt+E legt an", "event.code !== \"KeyE\""),
        ("Live-Region", "\"aria-live\": \"polite\""),
        ("aria-Beschriftung der Knoten", "setAttribute(\"aria-label\""),
        ("Anordnung zurücksetzen", "call(\"idea.map_pos_reset\", { work })"),
    ] {
        assert!(SOURCE.contains(needle), "{what}: Beleg „{needle}“ fehlt im Widget");
    }
    // Texte nie als HTML (keine Einschleusung über Schlagwörter oder Notizen).
    assert!(!SOURCE.contains("innerHTML") && !SOURCE.contains("insertAdjacentHTML"), "nur textContent");
}

// ---- Austausch mit Karte ------------------------------------------------------------------------------------

fn sha(bytes: &[u8]) -> String {
    baruch_exchange::sha256_hex(bytes)
}

async fn import(server: &common::Server, bytes: &[u8], mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(bytes), "sha256": sha(bytes), "mode": mode }),
        )
        .await;
    assert!(answer["error"].is_null(), "Einlesen ({mode}) scheitert: {answer}");
    answer["result"].clone()
}

async fn export(server: &common::Server, work: i64, format: &str) -> Vec<u8> {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": format })).await;
    assert!(answer["error"].is_null(), "Ausgabe scheitert: {answer}");
    let result = &answer["result"];
    match format {
        "xml" => result["xml"].as_str().expect("xml").as_bytes().to_vec(),
        _ => baruch_store::decode_base64(result["data"].as_str().expect("data")).unwrap(),
    }
}

/// XML ohne `<ausgabe …/>` (Zeit und Instanz der Ausgabe unterscheiden sich naturgemäß).
fn without_output(xml: &[u8]) -> String {
    let text = String::from_utf8(xml.to_vec()).expect("UTF-8");
    let Some(start) = text.find("<ausgabe") else { return text };
    let end = start + text[start..].find("/>").expect("<ausgabe/> geschlossen") + 2;
    format!("{}{}", &text[..start], &text[end..])
}

fn assert_u2(name: &str, xml: &[u8]) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(name);
    std::fs::write(&file, xml).unwrap();
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u7.xsd");
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => {
            assert!(output.status.success(), "{name} gegen u2-XSD: {}", String::from_utf8_lossy(&output.stderr))
        }
        Err(e) => eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e})"),
    }
}

async fn idea_id(server: &common::Server, work: i64, n: u32) -> i64 {
    sqlx::query_scalar("SELECT id FROM idea WHERE work_id = ? AND uuid = ?")
        .bind(work)
        .bind(sample_uuid(n))
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

/// Multipart-Anfrage an `/austausch/vorschau` wie ein Browserformular (ohne JavaScript).
fn upload(file_name: &str, content: &[u8]) -> Request<Body> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"datei\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    Request::post("/austausch/vorschau")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn links_and_positions_travel_a_to_b_and_back_without_loss_or_doubles() {
    // A liest das Beispielwerk (Unterversion 3, mit Karte) als neues Werk.
    let a = common::open().await;
    let sample = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    let work_a = import(&a, sample.as_bytes(), "new").await["work"].as_i64().unwrap();
    let xml_a = export(&a, work_a, "xml").await;
    let text_a = String::from_utf8(xml_a.clone()).unwrap();
    // Runde 22 (Vertrag §6.5, spiegel-entwurf): Der Schreiber schreibt jetzt Unterversion 8.
    assert!(text_a.contains("version=\"8\""), "Ausgabe in Unterversion 8 (seit Runde 22)");
    assert!(
        text_a.contains("<karte>") && text_a.contains("art=\"widerspricht\"") && text_a.contains("<lage "),
        "die Ausgabe trägt die Karte"
    );
    assert!(text_a.contains("art=\"verbindung\"") && text_a.contains("art=\"lage\""), "Grabsteine der Karte");
    assert_u2("a.baruch.xml", &xml_a);

    // Rundreise A → B (neu) → Ausgabe: byte-gleich bis auf <ausgabe>, auch über den Container.
    let b = common::open().await;
    let zip_a = export(&a, work_a, "baruch").await;
    let work_b = import(&b, &zip_a, "new").await["work"].as_i64().unwrap();
    let xml_b = export(&b, work_b, "xml").await;
    assert_eq!(without_output(&xml_b), without_output(&xml_a), "Rundreise mit Karte ohne Verlust");

    // Auf B: neue Verbindung 40 „gehört zu“ 42, neue Lage für 42 und die Lage von 40 verschoben.
    let pool_b = b.state.store.pool();
    sqlx::query("INSERT INTO idea_link (from_id, to_id, kind, note, origin_kind, origin_name) VALUES (?, ?, 'belongs_to', 'auf B', 'human', 'Autor')")
        .bind(idea_id(&b, work_b, 40).await)
        .bind(idea_id(&b, work_b, 42).await)
        .execute(pool_b)
        .await
        .unwrap();
    sqlx::query("INSERT INTO idea_map_pos (work_id, idea_id, x, y) VALUES (?, ?, 33.0, 44.0)")
        .bind(work_b)
        .bind(idea_id(&b, work_b, 42).await)
        .execute(pool_b)
        .await
        .unwrap();
    sqlx::query("UPDATE idea_map_pos SET x = 500.0 WHERE uuid = ?")
        .bind(sample_uuid(152))
        .execute(pool_b)
        .await
        .unwrap();
    let new_link: String =
        sqlx::query_scalar("SELECT uuid FROM idea_link WHERE note = 'auf B'").fetch_one(pool_b).await.unwrap();
    // Auf A: dieselbe Lage anders verschoben (lokal gewinnt).
    let pool_a = a.state.store.pool();
    sqlx::query("UPDATE idea_map_pos SET x = 11.0 WHERE uuid = ?")
        .bind(sample_uuid(152))
        .execute(pool_a)
        .await
        .unwrap();

    let zip_b = export(&b, work_b, "baruch").await;
    // Vorschau ohne JavaScript: die Seite nennt Verbindungen und Lagen.
    let (status, _, page) = a.send(upload("b.baruch", &zip_b)).await;
    assert_eq!(status, StatusCode::OK, "Vorschauseite: {page}");
    assert!(page.contains("1 Verbindung der Gedankenkarte"), "Vorschau zählt die Verbindung: {page}");
    assert!(page.contains("1 Lage auf der Gedankenkarte"), "Vorschau zählt die Lage: {page}");

    let merged = import(&a, &zip_b, "merge").await;
    assert_eq!(
        (merged["added"]["links"].as_u64(), merged["added"]["positions"].as_u64()),
        (Some(1), Some(1)),
        "{merged}"
    );
    let arrived: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM idea_link l JOIN idea i ON i.id = l.from_id WHERE l.uuid = ? AND i.work_id = ?",
    )
    .bind(&new_link)
    .bind(work_a)
    .fetch_one(pool_a)
    .await
    .unwrap();
    assert_eq!(arrived, 1, "die Verbindung von B kommt mit ihrer Kennung auf A an");
    let x: f64 = sqlx::query_scalar("SELECT x FROM idea_map_pos WHERE uuid = ?")
        .bind(sample_uuid(152))
        .fetch_one(pool_a)
        .await
        .unwrap();
    assert_eq!(x, 11.0, "die Lage auf A bleibt (bekannte Kennung, lokal gewinnt)");

    // Zweiter Abgleich: nichts doppelt.
    let again = import(&a, &zip_b, "merge").await;
    let total: u64 = again["added"].as_object().unwrap().values().map(|v| v.as_u64().unwrap()).sum();
    assert_eq!(total, 0, "ein zweiter Abgleich legt nichts an: {again}");

    // Die Seite „Austausch: Ausgeben“ nennt den Umfang der Karte.
    let (status, html) = a.get(&format!("/werke/{work_a}/austausch")).await;
    assert_eq!(status, StatusCode::OK, "Seite Austausch");
    assert!(
        html.contains("Gedankenkarte: 3 Verbindungen (davon 1 gelöscht) und 3 gemerkte Lagen"),
        "Umfang der Karte auf der Seite: {html}"
    );
}
