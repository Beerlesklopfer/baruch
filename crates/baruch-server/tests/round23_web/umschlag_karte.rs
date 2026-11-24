// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 23a: Umschlag-Reste aus Runde 27 (eigene Fehlerart für ungültige Blöcke, Zeitgrenze der Mess-Sätze,
//! Pixelgrenze und Zwischenspeicher der Kontrastprüfung) und Gedankenkarte (veralteter Gruppenvorschlag, Obergrenze
//! der Nähe). Werke, Bilder und Einfälle frei erfunden.

use std::io::Cursor;
use std::time::Duration;

use baruch_server::cover_print::{MAX_CONTRAST_SIDE_PX, contrast_decodes, cover_checks, decode_for_contrast};
use baruch_server::idea_map::groups::{Proposal, remember};
use baruch_server::idea_map::near::MAX_NEAR_IDEAS;
use serde_json::{Value, json};

use crate::common;

fn ok(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none_or(Value::is_null), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    let image = image::RgbImage::from_pixel(width, height, image::Rgb(rgb));
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image).write_to(&mut out, image::ImageFormat::Png).expect("PNG schreiben");
    out.into_inner()
}

// ---- Umschlag ------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn an_invalid_cover_block_is_invalid_params_with_the_same_words_as_before() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let view = ok(&server.rpc("cover.layout", json!({ "work": work })).await).clone();
    let mut title = view["blocks"].as_array().unwrap().iter().find(|b| b["kind"] == "front_title").unwrap().clone();
    title["x_mm"] = json!(2000.0);
    let refused = server.rpc("cover.layout_set", json!({ "work": work, "blocks": [title] })).await;
    assert_eq!(refused["error"]["code"], -32602, "StoreError::CoverBlock bleibt -32602 (ungültige Eingabe): {refused}");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("Umschlag-Block „Titel (Vorderseite)“ (front_title), Feld x_mm: "),
        "Art und Feld stehen in der Meldung, Wortlaut wie vor der eigenen Fehlerart: {message}"
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM cover_block WHERE work_id = ?")
        .bind(work)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0, "nichts gespeichert");
}

#[tokio::test]
async fn default_blocks_reach_the_layout_rounded_to_a_hundredth_of_a_millimetre() {
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let view = ok(&server.rpc("cover.layout", json!({ "work": work })).await).clone();
    let blocks = view["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 15, "alle Vorgabeblöcke");
    for block in blocks {
        for field in ["x_mm", "y_mm", "width_mm"] {
            let mm = block[field].as_f64().unwrap();
            assert!(
                ((mm * 100.0).round() - mm * 100.0).abs() < 1e-6,
                "{}.{field} = {mm}: Vorgaben stehen auf 0,01 mm in cover.layout (vorher z. B. 20,999999)",
                block["kind"]
            );
        }
    }
}

#[tokio::test]
async fn the_measuring_compile_has_the_time_limit_of_the_typesetter() {
    assert_eq!(
        baruch_server::cover::MEASURE_TIMEOUT,
        Duration::from_secs(60),
        "book_page_mm und block_heights setzen mit derselben Grenze wie der Satz (60 s)"
    );
    let quick = baruch_render::Source { main: "#set page(width: 100mm, height: 60mm)\nKurz.\n".into(), files: vec![] };
    let rendered = baruch_server::cover::compile_limited(quick, Duration::from_secs(60))
        .await
        .expect("Arbeitsfaden läuft")
        .expect("eine kurze Quelle wird in der Zeit fertig");
    assert!(!rendered.has_errors(), "gesetzt ohne Fehler");

    // Eine Quelle, die rechnet (eine Schleife; Funktionsaufrufe merkt sich Typst und wäre sofort fertig): mit einer
    // Grenze von einer Millisekunde wird sie nicht fertig.
    let slow = baruch_render::Source {
        main: "#{\n  let s = 0\n  for i in range(1500000) { s += i }\n  s\n}\n".into(),
        files: vec![],
    };
    let cut = baruch_server::cover::compile_limited(slow, Duration::from_millis(1)).await.expect("Arbeitsfaden läuft");
    assert!(cut.is_none(), "über der Zeitgrenze kommt kein Ergebnis zurück (der Aufrufer meldet es als Ablehnung)");
}

#[tokio::test]
async fn the_contrast_check_refuses_oversized_images_and_decodes_a_cover_only_once() {
    // Pixelgrenze: ein Bild mit mehr als 20 000 Pixel an einer Seite wird nicht entpackt.
    assert_eq!(MAX_CONTRAST_SIDE_PX, 20_000, "Grenze wie bei den Vorschaubildern");
    let wide = png(MAX_CONTRAST_SIDE_PX + 1, 2, [10, 10, 10]);
    assert!(decode_for_contrast(&wide).is_none(), "20 001 Pixel breit: nicht entpackt, also „nicht gemessen“");
    let fine = png(64, 48, [250, 240, 180]);
    let decoded = decode_for_contrast(&fine).expect("ein gewöhnliches Bild wird entpackt");
    assert_eq!((decoded.width(), decoded.height()), (64, 48), "mit seinen Maßen");
    assert!(decode_for_contrast(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").is_none(), "SVG hat keine Pixel");

    // Zwischenspeicher: dieselbe Druckprüfung zweimal entpackt das Umschlagbild einmal.
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let work = server.import_garden().await.work;
    // Einmalige Farbe, damit kein anderer Test dasselbe Bild (dieselbe Prüfsumme) entpackt.
    let cover = png(300, 420, [37, 91, 143]);
    let sum = baruch_render::sha256_hex(&cover);
    let added = server
        .rpc(
            "asset.add",
            json!({ "work": work, "file_name": "nebelmeer.png", "content": baruch_store::encode_base64(&cover),
                    "encoding": "base64", "caption": "Erfundenes Nebelmeer", "alt": "Nebel über Wasser" }),
        )
        .await;
    let name = ok(&added)["name"].as_str().unwrap().to_owned();
    ok(&server.rpc("work.set_cover", json!({ "work": work, "name": name })).await);
    let view = ok(&server.rpc("cover.layout", json!({ "work": work })).await).clone();
    let title = view["blocks"].as_array().unwrap().iter().find(|b| b["kind"] == "front_title").unwrap().clone();
    ok(&server.rpc("cover.layout_set", json!({ "work": work, "blocks": [title] })).await);

    let first = cover_checks(&server.state, work, None).await.unwrap();
    let second = cover_checks(&server.state, work, None).await.unwrap();
    let third = cover_checks(&server.state, work, None).await.unwrap();
    assert_eq!(first, second, "dieselbe Prüfung, dasselbe Ergebnis (auch aus dem Zwischenspeicher)");
    assert_eq!(second, third, "…und beim dritten Mal");
    assert_eq!(
        contrast_decodes(&sum),
        1,
        "das Umschlagbild wurde in diesem Prozess genau einmal entpackt, obwohl cover.layout_set und drei \
         Druckprüfungen es brauchten (vorher: bei jedem Aufruf)"
    );
}

// ---- Gedankenkarte -------------------------------------------------------------------------------------------------

async fn idea(server: &common::Server, work: i64, words: &[&str], note: &str) -> i64 {
    let created = server.rpc("idea.create", json!({ "work": work, "words": words, "note": note })).await;
    ok(&created)["idea"]["id"].as_i64().unwrap_or_else(|| panic!("id des Einfalls: {created}"))
}

async fn words_of(server: &common::Server, id: i64) -> Vec<String> {
    server.state.store.idea_get(id).await.unwrap().words
}

#[tokio::test]
async fn accepting_a_stale_group_proposal_is_refused_and_changes_no_idea() {
    let server = common::open().await;
    let work = server.import("= Im Moor\n\nDer Steg führt weit hinaus.\n", "moor-23.typ").await.work;
    let a = idea(&server, work, &["Torf"], "Torf brennt langsam.").await;
    let b = idea(&server, work, &["Torf", "Rauch"], "").await;
    let c = idea(&server, work, &["Rauch", "Herd"], "").await;
    let groups = ok(&server.rpc("idea.map_groups", json!({ "work": work })).await).clone();
    let group = groups["groups"][0]["group"].as_str().expect("eine Gruppe").to_owned();
    let members: Vec<i64> =
        groups["groups"][0]["ideas"].as_array().unwrap().iter().map(|i| i.as_i64().unwrap()).collect();
    assert_eq!(members, [a, b, c], "Testvoraussetzung: die drei Einfälle hängen über gleiche Schlagwörter zusammen");

    // Ein Vorschlag aus der Zeit, als die Gruppe nur zwei Einfälle hatte (jetzt hat sie drei).
    let stale = Proposal {
        name: "Feuerstelle".into(),
        persona: "@lektor".into(),
        model: "probe-1".into(),
        at: "2026-11-24T08:00:00Z".into(),
        ideas: vec![a, b],
    };
    remember(&server.state, work, &group, stale);
    let listed = ok(&server.rpc("idea.map_groups", json!({ "work": work })).await).clone();
    assert!(
        listed["groups"][0].get("proposal").is_none_or(Value::is_null),
        "Testvoraussetzung: die Anzeige blendet den veralteten Vorschlag schon aus: {listed}"
    );
    let before = [words_of(&server, a).await, words_of(&server, b).await, words_of(&server, c).await];

    let refused = server.rpc("idea.map_group_accept", json!({ "work": work, "group": group })).await;
    assert_eq!(refused["error"]["code"], -32000, "veralteter Vorschlag → -32000: {refused}");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("Veraltet: ") && message.contains("Feuerstelle"),
        "die Meldung sagt „Veraltet“ und nennt den Vorschlag: {message}"
    );
    let after = [words_of(&server, a).await, words_of(&server, b).await, words_of(&server, c).await];
    assert_eq!(after, before, "kein Einfall hat ein Schlagwort bekommen (vorher: die zwei aus dem alten Vorschlag)");

    // Der veraltete Vorschlag ist weg; ohne Namen gibt es nichts mehr zu übernehmen.
    let again = server.rpc("idea.map_group_accept", json!({ "work": work, "group": group })).await;
    assert_eq!(again["error"]["code"], -32602, "ohne Vorschlag und ohne Namen: -32602 wie bisher: {again}");

    // Mit eigenem Namen gilt die Gruppe, wie die Karte sie jetzt zeigt (alle drei).
    let taken = server.rpc("idea.map_group_accept", json!({ "work": work, "group": group, "name": "Herdfeuer" })).await;
    assert_eq!(ok(&taken)["changed"], json!([a, b, c]), "alle drei Einfälle der jetzigen Gruppe: {taken}");
    for id in [a, b, c] {
        assert!(words_of(&server, id).await.iter().any(|w| w == "Herdfeuer"), "Einfall {id} trägt das Schlagwort");
    }
}

#[tokio::test]
async fn a_full_idea_stops_the_whole_acceptance_before_the_first_change() {
    let server = common::open().await;
    let work = server.import("= Am Wehr\n\nDas Wasser staut sich.\n", "wehr-23.typ").await.work;
    let a = idea(&server, work, &["Schleuse"], "").await;
    // Der zweite Einfall hat schon die Höchstzahl an Schlagwörtern.
    let mut many: Vec<String> = (1..baruch_store::MAX_IDEA_WORDS).map(|n| format!("Wort{n}")).collect();
    many.push("Schleuse".into());
    let many_refs: Vec<&str> = many.iter().map(String::as_str).collect();
    let b = idea(&server, work, &many_refs, "").await;
    let groups = ok(&server.rpc("idea.map_groups", json!({ "work": work })).await).clone();
    let group = groups["groups"][0]["group"].as_str().expect("eine Gruppe").to_owned();
    let before = (words_of(&server, a).await, words_of(&server, b).await);
    let refused = server.rpc("idea.map_group_accept", json!({ "work": work, "group": group, "name": "Stau" })).await;
    assert_eq!(refused["error"]["code"], -32000, "Grenze der Schlagwörter → -32000: {refused}");
    assert_eq!(
        (words_of(&server, a).await, words_of(&server, b).await),
        before,
        "die Grenzen sind vorab geprüft: auch der erste Einfall blieb unverändert"
    );
}

#[tokio::test]
async fn the_near_computation_is_capped_and_says_so() {
    let server = common::open().await;
    let work = server.import("= Im Archiv\n\nRegale bis zur Decke.\n", "archiv-23.typ").await.work;
    let origin = baruch_store::IdeaOrigin::Human { name: "Ida".into() };
    // Über der Obergrenze: direkt im Speicher angelegt (ohne Spiegel), damit der Test schnell bleibt.
    for n in 0..MAX_NEAR_IDEAS + 3 {
        // Jeder Einfall hat ein eigenes Schlagwort; die ersten zwei und die letzten zwei teilen dazu „Kartei“.
        let mut words = vec![format!("Zettel{n}")];
        if !(2..=MAX_NEAR_IDEAS).contains(&n) {
            words.push("Kartei".to_owned());
        }
        let words: &[String] = &words;
        let note = format!("Zettel {n}");
        let new = baruch_store::NewIdea {
            work: Some(work),
            key: None,
            words,
            note: &note,
            quote: "",
            source: "",
            bible: "",
            origin: origin.clone(),
        };
        server.state.store.idea_create(&new).await.unwrap();
    }
    let map = ok(&server.rpc("idea.map", json!({ "work": work })).await).clone();
    let message = map["near"]["message"].as_str().unwrap();
    assert!(
        message.contains(&format!("nur für die ersten {MAX_NEAR_IDEAS}")),
        "die Karte sagt, dass die Nähe gekappt ist: {message}"
    );
    let pairs = map["near"]["pairs"].as_array().unwrap();
    assert_eq!(
        pairs.len(),
        1,
        "nur das Paar unter den ersten {MAX_NEAR_IDEAS} Einfällen ist berechnet; die zwei letzten liegen jenseits \
         der Obergrenze: {pairs:?}"
    );
}
