// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 22 (sync, ownership spiegel-entwurf, contract §2.10/§2.11/§2.14/§2.15): exchange format sub-version 8 across
//! two instances (amendments, states, draft), new part as a request with identifier and revisions (consent, mirror
//! commit, renewed sync), `<kN>` show rule in the export and the mirror (`main.typ` with `#include`) typeset with
//! Typst.
//!
//! Works freely invented (`baruch_exchange::sample`, own texts). PDFs/typesetting under `common::PDF_SERIAL`; every
//! assertion with a reason.

use baruch_exchange::sample::{sample_document, sample_document_u8, sample_uuid};
use baruch_exchange::write::{ImageMode, write_xml};
use baruch_exchange::{ExchangeAuthor, ExchangeNode, ExchangeNodeKind, ExchangeRevision, ExchangeRevisionStatus};
use serde_json::{Value, json};

use crate::common;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

async fn import(server: &common::Server, xml: &str, mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": mode }),
        )
        .await;
    result(&answer).clone()
}

async fn export_xml(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    result(&answer)["xml"].as_str().unwrap().to_owned()
}

fn xmllint_u8(xml: &str) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("werk.baruch.xml");
    std::fs::write(&file, xml).unwrap();
    let schema = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u10.xsd");
    match std::process::Command::new("xmllint")
        .args(["--noout", "--nonet", "--schema"])
        .arg(&schema)
        .arg(&file)
        .output()
    {
        Ok(output) => assert!(output.status.success(), "XSD u8: {}", String::from_utf8_lossy(&output.stderr)),
        Err(e) => eprintln!("ÜBERSPRUNGEN: xmllint nicht gefunden ({e})"),
    }
}

/// Output without `<ausgabe …/>` and without the keys of the instance (`schluessel`, hold only there).
fn comparable(xml: &str) -> String {
    let re = regex::Regex::new(r#" schluessel="\d+""#).unwrap();
    let no_keys = re.replace_all(xml, "");
    no_keys.lines().filter(|l| !l.trim_start().starts_with("<ausgabe ")).collect::<Vec<_>>().join("\n")
}

#[tokio::test]
async fn two_instances_carry_amends_stands_draft_and_adopted_amend_byte_for_byte() {
    let a = common::open().await;
    let xml = write_xml(&sample_document_u8(), ImageMode::Inline).unwrap();
    let work_a = import(&a, &xml, "new").await["work"].as_i64().unwrap();
    let out_a = export_xml(&a, work_a).await;
    xmllint_u8(&out_a);
    for needle in [
        "version=\"10\"",
        "<fortschreibung ",
        "stand=\"3\"",
        "beruht-stand=\"2\"",
        "<entwurf ",
        "art=\"fortschreibung\"",
    ] {
        assert!(out_a.contains(needle), "{needle} in der Ausgabe von A");
    }
    let b = common::open().await;
    let work_b = import(&b, &out_a, "new").await["work"].as_i64().unwrap();
    let out_b = export_xml(&b, work_b).await;
    assert_eq!(comparable(&out_b), comparable(&out_a), "A → B → Ausgabe byte-gleich (bis auf <ausgabe> und Schlüssel)");
    let typeset = result(&b.rpc("work.typeset_get", json!({ "work": work_b })).await)["settings"]["draft"].clone();
    assert_eq!(typeset["placement"], "footer", "der Entwurf kommt in den Satz-Einstellungen an: {typeset}");
}

/// Work with two chapters (file of the one side) and the same file with a part around it (other side).
fn part_files() -> (String, String) {
    let mut doc = sample_document();
    let water = doc.work.outline.remove(0);
    let mut chapters: Vec<ExchangeNode> = water.children;
    chapters[0].posts[1].refs.clear();
    doc.work.outline = chapters.clone();
    let local = write_xml(&doc, ImageMode::Inline).unwrap();
    let heading = "#heading(level: 1, numbering: none)[Ganz neuer Teil] <baruch-part>\n";
    doc.work.outline = vec![ExchangeNode {
        uuid: sample_uuid(700),
        kind: ExchangeNodeKind::Part,
        numbered: true,
        title: "Ganz neuer Teil".into(),
        revisions: vec![
            ExchangeRevision {
                uuid: sample_uuid(701),
                number: 1,
                status: ExchangeRevisionStatus::Superseded,
                author: ExchangeAuthor::Human { name: "Hanna Probe".into() },
                body: heading.to_owned(),
                created: "2026-02-04T09:00:00.000Z".into(),
                ..ExchangeRevision::default()
            },
            ExchangeRevision {
                uuid: sample_uuid(702),
                number: 2,
                status: ExchangeRevisionStatus::Current,
                author: ExchangeAuthor::Human { name: "Hanna Probe".into() },
                based_on: Some(sample_uuid(701)),
                body: format!("{heading}\nVorwort zum Teil.\n"),
                created: "2026-02-05T09:00:00.000Z".into(),
                ..ExchangeRevision::default()
            },
        ],
        children: chapters,
        ..ExchangeNode::default()
    }];
    (local, write_xml(&doc, ImageMode::Inline).unwrap())
}

#[tokio::test]
async fn agreeing_to_a_new_part_from_the_exchange_keeps_its_uuid_and_revisions_and_a_second_merge_reports_nothing() {
    let server = common::open().await;
    let (local, there) = part_files();
    let imported = import(&server, &local, "new").await;
    let (work, slug) = (imported["work"].as_i64().unwrap(), imported["slug"].as_str().unwrap().to_owned());
    let merged = import(&server, &there, "merge").await;
    assert_eq!(merged["requests"].as_array().map(Vec::len), Some(1), "ein Antrag: {merged}");
    let deviation = merged["deviations"].as_array().unwrap().iter().find(|d| d["uuid"] == sample_uuid(700)).unwrap();
    assert!(
        deviation["there"].as_str().unwrap().contains("Antrag mit Kennung und 2 Fassung(en)"),
        "Vorschau nennt Kennung und Fassungen: {deviation}"
    );
    let id = merged["requests"][0].as_i64().unwrap();
    // Consent without JavaScript: notice bar (consent) and /gliederung name the request with identifier.
    let (_, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert!(page.contains("mit Kennung und 2 Fassungen"), "die Gliederung nennt „mit Kennung und 2 Fassungen“");
    assert!(page.contains("name=\"method\" value=\"outline.request_decide\""), "Formular zum Zustimmen");

    let decided = server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await;
    let decided = result(&decided);
    assert_eq!(decided["request"]["decision"], "confirmed", "zugestimmt: {decided}");
    assert!(decided["commit"].is_string(), "ein Spiegel-Commit für das Anlegen: {decided}");
    let node = server.state.store.load_work(work).await.unwrap();
    let part = node.nodes.iter().find(|n| n.kind == baruch_core::NodeKind::Part).expect("der Teil steht da");
    let uuid: String = sqlx::query_scalar("SELECT uuid FROM node WHERE id = ?")
        .bind(part.key)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap();
    assert_eq!(uuid, sample_uuid(700), "der Teil hat die Kennung aus der Datei");
    let numbers: Vec<u32> = part.revisions.iter().map(|r| r.number).collect();
    assert_eq!(numbers, vec![1, 2], "genau seine zwei Fassungen mit ihren Nummern");
    assert_eq!(part.current_number(), Some(2), "aktuell ist Fassung 2 wie in der Datei");
    let main = std::fs::read_to_string(server.mirror_dir_of(&slug).join("main.typ")).unwrap();
    assert!(main.contains("ganz-neuer-teil"), "der Teil steht im Spiegel: {main}");

    let again = import(&server, &there, "merge").await;
    let added = &again["added"];
    let total: u64 = added.as_object().unwrap().values().filter_map(Value::as_u64).sum();
    assert_eq!(total, 0, "erneuter Abgleich fügt nichts hinzu: {added}");
    assert_eq!(again["requests"], json!([]), "und stellt keinen Antrag");
    assert!(again.get("commit").is_none(), "kein Commit ohne Änderung: {again}");
}

#[tokio::test]
async fn a_title_with_code_shared_with_a_numbered_node_gets_its_show_rule_over_the_jump_label() {
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let source =
        "#set heading(numbering: \"1.\")\n\n= Anfang\n\nText.\n\n= #emph[Rast]\n\nPause.\n\n= #emph[Rast]\n\nWeiter.\n";
    let report = server.import(source, "rast.typ").await;
    let work = report.work;
    let loaded = server.state.store.load_work(work).await.unwrap();
    let pause = loaded.nodes.iter().find(|n| n.revisions.iter().any(|r| r.body.contains("Pause"))).unwrap();
    let (key, seq) = (pause.key, pause.id);
    result(
        &server.rpc("outline.set_numbered", json!({ "work": work, "node": seq, "key": key, "numbered": false })).await,
    );
    let export =
        result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].as_str().unwrap().to_owned();
    let rule = format!("#show <k{key}>: set heading(numbering: none)");
    assert!(export.contains(&rule), "Show-Regel über die Sprungmarke im Export: {export}");
    assert!(!export.contains("body: [#emph[Rast]]"), "keine Regel über den Titel (träfe beide): {export}");
    // Typst does not count the section: the second „Rast“ is 2., not 3.
    let source = baruch_render::Source { main: export, files: Vec::new() };
    let text: String = baruch_render::page_texts(&source).unwrap().into_iter().flatten().map(|t| t.text).collect();
    assert!(text.contains("2.") && !text.contains("3."), "Zählung 1., (ohne), 2.: {text}");
}

const HOUSE: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"30\" viewBox=\"0 0 40 30\">\
<rect x=\"5\" y=\"12\" width=\"30\" height=\"18\" fill=\"#a0522d\"/><path d=\"M2 12 L20 2 L38 12 Z\" fill=\"#8b0000\"/></svg>\n";

#[tokio::test]
async fn the_mirror_main_typ_with_its_includes_sets_like_the_export() {
    let _serial = common::PDF_SERIAL.lock().await;
    let server = common::open().await;
    let source = "#set page(paper: \"a6\")\n#set text(lang: \"de\")\n#set heading(numbering: \"1.\")\n\
                  #let hinweis(body) = block(inset: 4pt, stroke: 0.5pt, body)\n\n\
                  = Erstes Kapitel\n\nAm Hang steht ein Haus.\n\n#figure(image(\"bilder/haus.svg\", width: 3cm), caption: [Das Haus])\n\n\
                  == Der Garten\n\n#hinweis[Im Garten blühen Bohnen.]\n\n\
                  = Zweites Kapitel\n\nDer Weg führt hinunter zum Bach, wo die Kinder Steine sammeln.\n";
    let report = server.import(source, "hang.typ").await;
    let work = report.work;
    result(
        &server
            .rpc(
                "asset.add",
                json!({ "work": work, "file_name": "haus.svg", "content": HOUSE, "encoding": "svg",
                        "path": "bilder/haus.svg" }),
            )
            .await,
    );
    // A part before the first chapter (as in the user interface through „Neuer Teil davor“).
    let loaded = server.state.store.load_work(work).await.unwrap();
    let first = &loaded.nodes[0];
    result(
        &server
            .rpc(
                "outline.add",
                json!({ "work": work, "target": first.id, "place": "before",
                                         "title": "Erster Teil", "level": 0 }),
            )
            .await,
    );
    let export =
        result(&server.rpc("work.export", json!({ "work": work })).await)["typst"].as_str().unwrap().to_owned();
    let assets = server.state.store.export_assets(work).await.unwrap();
    let export_source = baruch_render::Source {
        main: export,
        files: assets.iter().map(|f| (f.path.clone(), f.bytes.clone())).collect(),
    };
    let export_pages = baruch_render::compile(&export_source);
    assert!(!export_pages.has_errors(), "Vorbedingung: der Export setzt fehlerfrei: {:?}", export_pages.diagnostics);

    let files = server.state.store.mirror_snapshot(work).await.unwrap();
    let main = files.iter().find(|f| f.path == "main.typ").unwrap().contents.clone();
    assert!(main.contains("#include \"abschnitte/"), "main.typ bindet die Abschnitte ein: {main}");
    let section = files.iter().find(|f| f.contents.contains("Am Hang")).unwrap();
    assert!(section.contents.contains("image(\"/bilder/haus.svg\""), "Bildpfad ab der Wurzel: {}", section.contents);
    // Raster images lie in the mirror as Base64 text (`.b64`, documented deviation); here only an SVG, raw.
    let mirror_files: Vec<(String, Vec<u8>)> = files
        .iter()
        .filter(|f| f.path != "main.typ" && !f.path.ends_with(".b64"))
        .map(|f| (f.path.clone(), f.contents.clone().into_bytes()))
        .collect();
    let mirror_source = baruch_render::Source { main, files: mirror_files };
    let mirror_pages = baruch_render::compile(&mirror_source);
    let errors: Vec<_> =
        mirror_pages.diagnostics.iter().filter(|d| d.severity == baruch_render::Severity::Error).collect();
    assert!(errors.is_empty(), "der Spiegel setzt fehlerfrei: {errors:?}");
    assert_eq!(mirror_pages.pages.len(), export_pages.pages.len(), "gleiche Seitenzahl wie der Export");
}
