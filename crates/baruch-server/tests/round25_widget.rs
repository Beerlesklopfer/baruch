// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 25 „karte-widget“: links and positions in the exchange format (minor version 2, contract §4.5/§4.6).
//!
//! - The widget of the idea map (`idea_map.ts`) has been replaced by `mindmap.ts` since Round 34; its tests (script,
//!   DOM contract, allowed RPC) are now in `round34_widget.rs`.
//! - Exchange: round trip A → B → A with links and positions over RPC; links and positions „hier“ are created through
//!   SQL, because the write paths of the map (karte-daten) are still stubs in this branch. The fill triggers from 0043
//!   assign their identifiers as on every other path.
//!
//! No request to Eliezer or other services (no model, no embedding).

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_exchange::sample::{sample_document, sample_uuid};
use baruch_exchange::write::{ImageMode, write_xml};
use serde_json::{Value, json};

use crate::common;

const BOUNDARY: &str = "baruch-karte-grenze";

// ---- Exchange with map --------------------------------------------------------------------------------------

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

/// XML without `<ausgabe …/>` (time and instance of the output naturally differ).
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
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u10.xsd");
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

/// Multipart request to `/austausch/vorschau` like a browser form (without JavaScript).
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
    // A reads the sample work (minor version 3, with map) as a new work.
    let a = common::open().await;
    let sample = write_xml(&sample_document(), ImageMode::Inline).unwrap();
    let work_a = import(&a, sample.as_bytes(), "new").await["work"].as_i64().unwrap();
    let xml_a = export(&a, work_a, "xml").await;
    let text_a = String::from_utf8(xml_a.clone()).unwrap();
    // Round 34 (mindmap_daten): the writer now writes minor version 9 (tree of the ideas).
    assert!(text_a.contains("version=\"10\""), "Ausgabe in Unterversion 9 (seit Runde 34)");
    assert!(
        text_a.contains("<karte>") && text_a.contains("art=\"widerspricht\"") && text_a.contains("<lage "),
        "die Ausgabe trägt die Karte"
    );
    assert!(text_a.contains("art=\"verbindung\"") && text_a.contains("art=\"lage\""), "Grabsteine der Karte");
    assert_u2("a.baruch.xml", &xml_a);

    // Round trip A → B (new) → output: byte-identical except for <ausgabe>, also through the container.
    let b = common::open().await;
    let zip_a = export(&a, work_a, "baruch").await;
    let work_b = import(&b, &zip_a, "new").await["work"].as_i64().unwrap();
    let xml_b = export(&b, work_b, "xml").await;
    assert_eq!(without_output(&xml_b), without_output(&xml_a), "Rundreise mit Karte ohne Verlust");

    // On B: new link 40 „gehört zu“ 42, new position for 42 and the position of 40 moved.
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
    // On A: the same position moved differently (local wins).
    let pool_a = a.state.store.pool();
    sqlx::query("UPDATE idea_map_pos SET x = 11.0 WHERE uuid = ?")
        .bind(sample_uuid(152))
        .execute(pool_a)
        .await
        .unwrap();

    let zip_b = export(&b, work_b, "baruch").await;
    // Preview without JavaScript: the page names links and positions.
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

    // Second merge: nothing twice.
    let again = import(&a, &zip_b, "merge").await;
    let total: u64 = again["added"].as_object().unwrap().values().map(|v| v.as_u64().unwrap()).sum();
    assert_eq!(total, 0, "ein zweiter Abgleich legt nichts an: {again}");

    // The page „Austausch: Ausgeben“ names the extent of the map.
    let (status, html) = a.get(&format!("/werke/{work_a}/austausch")).await;
    assert_eq!(status, StatusCode::OK, "Seite Austausch");
    assert!(
        html.contains("Gedankenkarte: 3 Verbindungen (davon 1 gelöscht) und 3 gemerkte Lagen"),
        "Umfang der Karte auf der Seite: {html}"
    );
}
