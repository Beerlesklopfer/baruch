// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 26 „export“: Austauschdatei ausgeben (Seite, Downloads, RPC `work.exchange_export`, CLI `--baruch`, MCP
//! `exchange_export`), Ausgabe gegen das XSD.
//!
//! Echter Datenordner im Temp-Ordner, Router über `oneshot`, Geheimnisse nur in `MemorySecrets`; keine Anfrage
//! an Eliezer oder ins Netz. Testtexte frei erfunden.

use crate::common;

use std::io::Read as _;
use std::net::SocketAddr;
use std::path::Path;
use std::process::Command;

use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_core::Post;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Frei erfundenes Buch mit Teil, Kapiteln und Abschnitt.
const BOOK: &str = "#set text(lang: \"de\")\n\n\
#heading(level: 1, numbering: none)[Erster Teil] <baruch-part>\n\n\
= Die Mühle\nDas Rad dreht sich am Bach.\n\n\
== Der Stein\nZwei Steine mahlen.\n\n\
= Der Müller\nEr singt beim Mahlen.\n";

const SECRET: &str = "geheim-r26-verbindung-7f3a9c";
const MODEL: &str = "modell-r26-einmalig-c4d2";

async fn get_bytes(server: &common::Server, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    (status, headers, to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec())
}

/// Werk „Die Mühle“ mit einem Vorschlag, einem Beitrag, einem Einfall und einem Bild; gibt (id, slug) zurück.
async fn mill(server: &common::Server) -> (i64, String) {
    let report = server.import(BOOK, "muehle.typ").await;
    let store = &server.state.store;
    store
        .add_proposal(report.work, 1, "= Die Mühle\nDas Rad steht still.\n", "@lektor", "probe", "Ruhe")
        .await
        .unwrap();
    let post = Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: "Schöner Anfang.".into(),
        sources: vec!["Probequelle".into()],
        refs: vec![],
    };
    store.add_post(report.work, 1, &post).await.unwrap();
    let words = vec!["Mühle".to_owned()];
    store
        .idea_create(&baruch_store::NewIdea {
            work: Some(report.work),
            key: None,
            words: &words,
            note: "Wasser treibt das Rad.",
            quote: "",
            source: "",
            bible: "",
            origin: baruch_store::IdeaOrigin::Human { name: "Autor".into() },
        })
        .await
        .unwrap();
    let new = baruch_store::NewAsset {
        file_name: "rad.png",
        name: None,
        bytes: baruch_exchange::sample::SAMPLE_PNG_GREEN,
        as_base64: true,
        path: Some("bilder/rad.png"),
        caption: "Das Rad",
        alt: "Ein grüner Punkt",
        note: "",
    };
    store.add_asset(report.work, &new, "Autor").await.unwrap();
    (report.work, report.slug)
}

fn xmllint_available() -> bool {
    Command::new("xmllint").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Prüft XML gegen `docs/schema/baruch-werk-1-u10.xsd`; fehlt xmllint, mit Meldung übersprungen.
fn assert_schema(name: &str, xml: &[u8]) {
    if !xmllint_available() {
        eprintln!("xmllint fehlt: XSD-Prüfung von {name} übersprungen (die Rust-Prüfung steht in baruch-exchange).");
        return;
    }
    let schema = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/baruch-werk-1-u10.xsd");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(name);
    std::fs::write(&file, xml).unwrap();
    let output =
        Command::new("xmllint").args(["--noout", "--nonet", "--schema"]).arg(&schema).arg(&file).output().unwrap();
    assert!(output.status.success(), "{name} besteht das XSD nicht: {}", String::from_utf8_lossy(&output.stderr));
}

/// Einträge eines ZIP in Reihenfolge: (Name, Inhalt).
fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).unwrap();
    (0..archive.len())
        .map(|i| {
            let mut entry = archive.by_index(i).unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            (entry.name().unwrap().into_owned(), data)
        })
        .collect()
}

fn entry<'a>(entries: &'a [(String, Vec<u8>)], name: &str) -> &'a [u8] {
    &entries.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("{name} fehlt im ZIP")).1
}

// ---- Seite und Downloads ---------------------------------------------------------------------------

#[tokio::test]
async fn the_page_works_without_javascript_and_links_both_downloads() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    let (status, html) = server.get(&format!("/werke/{work}/austausch")).await;
    assert_eq!(status, StatusCode::OK, "Seite: {html}");
    // Genau eine Kopfzeile (aus base.html), keine eigene.
    assert_eq!(html.matches("<header class=\"band\"").count(), 1, "genau eine Kopfzeile"); // Runde 33: der Kopf ist das Menüband (header.band statt header.topbar)
    // Ohne JavaScript: gewöhnliche Links auf beide Downloads und auf das Einlesen.
    assert!(html.contains(&format!("href=\"/werke/{work}/austausch.baruch\"")), "Download .baruch: {html}");
    assert!(html.contains(&format!("href=\"/werke/{work}/austausch.xml\"")), "Download .baruch.xml: {html}");
    assert!(html.contains(&format!("href=\"/austausch/einlesen?work={work}\"")), "Link zum Einlesen: {html}");
    assert!(html.contains(&format!("{slug}.baruch")), "Dateiname mit slug");
    // Erklärung: was drin ist und was nicht; Kennung des Werks und Umfang.
    assert!(html.contains("allen Fassungen") && html.contains("Nicht in der Datei"), "Erklärung");
    let uuid = server.state.store.work_uuid(work).await.unwrap();
    assert!(html.contains(&uuid), "Werk-Kennung auf der Seite");
    assert!(html.contains("4 Gliederungspunkte mit 5 Fassungen und 1 Beitrag"), "Umfang (Einzahl bei 1): {html}");
    assert!(html.contains("ohne Namen ("), "Instanz ohne Namen mit UUID-Präfix");
    assert!(!html.contains("noch nicht umgesetzt") && !html.contains("Noch nicht umgesetzt"), "kein Stub mehr");

    let (status, _) = server.get("/werke/9999/austausch").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn downloads_carry_type_attachment_name_and_pass_the_schema() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;

    let (status, headers, zip) = get_bytes(&server, &format!("/werke/{work}/austausch.baruch")).await;
    assert_eq!(status, StatusCode::OK, "ZIP-Download");
    assert_eq!(headers[header::CONTENT_TYPE], "application/vnd.baruch.werk+zip", "Medientyp ZIP");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap(),
        format!("attachment; filename=\"{slug}.baruch\""),
        "Anhang mit <slug>.baruch"
    );
    let entries = unzip(&zip);
    let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names.first(), Some(&"mimetype"), "mimetype zuerst");
    assert_eq!(names.last(), Some(&"MANIFEST.xml"), "Manifest zuletzt");
    assert_eq!(names.len(), 4, "mimetype, werk.xml, ein Bild, Manifest: {names:?}");
    assert_schema("werk.xml", entry(&entries, "werk.xml"));
    assert_schema("MANIFEST.xml", entry(&entries, "MANIFEST.xml"));
    let image = entries.iter().find(|(n, _)| n.starts_with("bilder/")).unwrap();
    assert_eq!(image.1, baruch_exchange::sample::SAMPLE_PNG_GREEN, "Bild byte-gleich im ZIP");

    let (status, headers, xml) = get_bytes(&server, &format!("/werke/{work}/austausch.xml")).await;
    assert_eq!(status, StatusCode::OK, "XML-Download");
    assert_eq!(headers[header::CONTENT_TYPE], "application/vnd.baruch.werk+xml", "Medientyp XML");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap(),
        format!("attachment; filename=\"{slug}.baruch.xml\""),
        "Anhang mit <slug>.baruch.xml"
    );
    assert_schema("muehle.baruch.xml", &xml);
    let text = String::from_utf8(xml).unwrap();
    assert!(text.contains("<titel>Die Mühle</titel>") && text.contains("status=\"vorschlag\""), "Inhalt");
    assert!(text.contains("<daten>"), "Nur-XML trägt das Bild als Base64");

    for uri in ["/werke/9999/austausch.baruch", "/werke/9999/austausch.xml"] {
        let (status, _, _) = get_bytes(&server, uri).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: unbekanntes Werk → 404");
    }
}

// ---- RPC -------------------------------------------------------------------------------------------

#[tokio::test]
async fn rpc_returns_fields_and_bytes_that_match_their_checksum() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    let uuid = server.state.store.work_uuid(work).await.unwrap();

    let zip = server.rpc("work.exchange_export", json!({ "work": work, "format": "baruch" })).await;
    let zip = &zip["result"];
    assert_eq!(zip["work"], work, "work");
    assert_eq!(zip["uuid"], uuid, "uuid des Werks");
    assert_eq!(zip["file_name"], format!("{slug}.baruch"), "Dateiname");
    assert_eq!(zip["media_type"], "application/vnd.baruch.werk+zip", "Medientyp");
    assert!(zip.get("xml").is_none(), "bei baruch kein xml");
    let bytes = baruch_store::decode_base64(zip["data"].as_str().unwrap()).unwrap();
    assert_eq!(zip["bytes"], bytes.len(), "bytes = Länge der gelieferten Datei");
    assert_eq!(zip["sha256"], baruch_exchange::sha256_hex(&bytes), "sha256 passt zu den gelieferten Bytes");
    assert!(bytes.starts_with(b"PK\x03\x04"), "ZIP");

    let xml = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    let xml = &xml["result"];
    assert_eq!(xml["file_name"], format!("{slug}.baruch.xml"), "Dateiname Nur-XML");
    assert_eq!(xml["media_type"], "application/vnd.baruch.werk+xml", "Medientyp Nur-XML");
    assert!(xml.get("data").is_none(), "bei xml kein data");
    let text = xml["xml"].as_str().unwrap();
    assert_eq!(xml["bytes"], text.len(), "bytes = Länge des XML");
    assert_eq!(xml["sha256"], baruch_exchange::sha256_hex(text.as_bytes()), "sha256 des XML");
    assert_schema("rpc.baruch.xml", text.as_bytes());

    // Vorgabe: format baruch, ohne work das Vorgabewerk (das gesäte Beispielwerk hat die kleinste id).
    let default = server.rpc("work.exchange_export", json!({})).await;
    assert!(default["result"]["data"].is_string(), "Vorgabeform baruch: {default}");
    assert_eq!(default["result"]["work"], server.state.default_work().await.unwrap(), "Vorgabewerk");

    for params in
        [json!({ "work": work, "format": "zip" }), json!({ "work": 9999 }), json!({ "work": work, "format": 1 })]
    {
        let answer = server.rpc("work.exchange_export", params.clone()).await;
        assert_eq!(answer["error"]["code"], -32602, "{params}: Parameterfehler: {answer}");
    }
}

// ---- Kein Geheimnis im Export -------------------------------------------------------------------------

#[tokio::test]
async fn no_secret_token_or_persona_model_ends_up_in_any_export() {
    let server = common::open().await;
    let (work, _) = mill(&server).await;
    // Verbindungsgeheimnis (nur im Speicher), MCP-Token und ein eindeutiges Persona-Modell, das in keiner Fassung
    // steht: keine dieser Zeichenketten darf in XML, ZIP, RPC- oder MCP-Antwort vorkommen.
    let set = server.rpc("connection.set_secret", json!({ "kind": "multigpt", "secret": SECRET })).await;
    assert!(set["error"].is_null(), "Geheimnis gesetzt: {set}");
    let token = server.rpc("mcp_token.create", json!({ "name": "Prüfrechner" })).await;
    let token = token["result"]["secret"].as_str().expect("Token-Klartext einmal").to_owned();
    let persona = server.rpc("persona.update", json!({ "handle": "@lektor", "model": MODEL })).await;
    assert!(persona["error"].is_null(), "Persona-Modell gesetzt: {persona}");
    let forbidden = [SECRET, token.as_str(), MODEL];

    let mut texts: Vec<(String, String)> = Vec::new();
    let (_, _, zip) = get_bytes(&server, &format!("/werke/{work}/austausch.baruch")).await;
    texts.push(("ZIP roh".into(), String::from_utf8_lossy(&zip).into_owned()));
    for (name, data) in unzip(&zip) {
        texts.push((format!("ZIP {name}"), String::from_utf8_lossy(&data).into_owned()));
    }
    let (_, _, xml) = get_bytes(&server, &format!("/werke/{work}/austausch.xml")).await;
    texts.push(("XML".into(), String::from_utf8(xml).unwrap()));
    for format in ["baruch", "xml"] {
        let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": format })).await;
        texts.push((format!("RPC {format}"), answer.to_string()));
    }
    let answer = mcp_call(&server, json!({ "name": "exchange_export", "arguments": { "work": work } })).await;
    assert_eq!(answer["result"]["isError"], false, "MCP-Ausgabe gelingt: {answer}");
    texts.push(("MCP".into(), answer.to_string()));
    let (_, page) = server.get(&format!("/werke/{work}/austausch")).await;
    texts.push(("Seite".into(), page));
    for (what, text) in &texts {
        for secret in forbidden {
            assert!(!text.contains(secret), "{what} enthält „{secret}“");
        }
    }
    // Gegenprobe: Das Modell eines Vorschlags (Herkunft) steht sehr wohl in der Datei.
    assert!(texts.iter().any(|(w, t)| w == "XML" && t.contains("modell=\"probe\"")), "Modell der Fassung als Herkunft");
}

// ---- MCP -------------------------------------------------------------------------------------------

async fn mcp_call(server: &common::Server, params: Value) -> Value {
    mcp(server, json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": params })).await
}

async fn mcp(server: &common::Server, message: Value) -> Value {
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn mcp_lists_exchange_export_read_only_and_returns_xml() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    let list = mcp(&server, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })).await;
    let tools = list["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let at = names.iter().position(|n| *n == "exchange_export").expect("exchange_export in tools/list");
    assert_eq!(names[at - 1], "typst_check", "exchange_export folgt auf das lesende typst_check: {names:?}");
    let description = tools[at]["description"].as_str().unwrap();
    assert!(description.contains("read-only"), "deutsche Beschreibung, nur lesend: {description}");
    assert!(description.contains("Reading in does not exist over MCP"), "kein Einlesen über MCP: {description}");
    assert!(!description.starts_with("STUB"), "kein Stub");
    // Kein Werkzeug zum Einlesen.
    assert!(!names.iter().any(|n| n.contains("import") || n.contains("einlesen")), "kein Einlesen: {names:?}");
    // Der Hinweis beim Verbinden nennt das Werkzeug, sonst findet ein Client es nur über tools/list.
    let init = mcp(
        &server,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "clientInfo": { "name": "x" } } }),
    )
    .await;
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("exchange_export"), "instructions nennen exchange_export: {instructions}");
    assert!(
        instructions.contains("reading an exchange file in does not exist over MCP"),
        "instructions sagen, dass das Einlesen dem Menschen bleibt: {instructions}"
    );

    let answer = mcp_call(&server, json!({ "name": "exchange_export", "arguments": { "work": work } })).await;
    let result = &answer["result"];
    assert_eq!(result["isError"], false, "gelingt: {answer}");
    let content = &result["structuredContent"];
    assert_eq!(content["file_name"], format!("{slug}.baruch.xml"), "Vorgabe xml");
    let xml = content["xml"].as_str().expect("xml als Text");
    assert_eq!(content["sha256"], baruch_exchange::sha256_hex(xml.as_bytes()), "sha256 des XML");
    assert_schema("mcp.baruch.xml", xml.as_bytes());

    // Auch per slug und als ZIP.
    let answer =
        mcp_call(&server, json!({ "name": "exchange_export", "arguments": { "work": slug, "format": "baruch" } }))
            .await;
    let data = answer["result"]["structuredContent"]["data"].as_str().expect("ZIP als Base64");
    assert!(baruch_store::decode_base64(data).unwrap().starts_with(b"PK\x03\x04"), "ZIP");
    // Unbekanntes Format ist ein Argumentfehler, unbekanntes Werk ein ehrlicher Werkzeugfehler.
    let answer =
        mcp_call(&server, json!({ "name": "exchange_export", "arguments": { "work": work, "format": "zip" } })).await;
    assert_eq!(answer["error"]["code"], -32602, "format zip: {answer}");
    let answer = mcp_call(&server, json!({ "name": "exchange_export", "arguments": { "work": 9999 } })).await;
    assert_eq!(answer["result"]["isError"], true, "unbekanntes Werk: {answer}");
}

#[tokio::test]
async fn mcp_refuses_files_above_the_documented_limit_honestly() {
    let server = common::open().await;
    let (work, slug) = mill(&server).await;
    // Ein Vorschlag mit gut 4 MiB Text macht die Datei größer als die dokumentierte Grenze.
    let filler = "Das Wasser fließt und fließt. ".repeat(160_000);
    let body = format!("= Die Mühle\n{filler}\n");
    assert!(body.len() > baruch_server::mcp::exchange::MCP_EXPORT_LIMIT, "Text über der Grenze");
    server.state.store.add_proposal(work, 1, &body, "@lektor", "probe", "lang").await.unwrap();
    let answer = mcp_call(&server, json!({ "name": "exchange_export", "arguments": { "work": work } })).await;
    let result = &answer["result"];
    assert_eq!(result["isError"], true, "über der Grenze ehrlich als Fehler: {}", answer.to_string().len());
    let message = result["content"][0]["text"].as_str().unwrap();
    assert!(message.contains("4 MiB"), "Grenze genannt: {message}");
    assert!(message.contains(&format!("/werke/{work}/austausch")), "Hinweis auf das Formular: {message}");
    assert!(message.contains(&format!("baruch-server export {slug} --baruch")), "Hinweis auf die CLI: {message}");
    // Über RPC und die Seite geht dieselbe Datei weiter (die Grenze gilt nur für MCP).
    let rpc = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    assert!(
        rpc["result"]["bytes"].as_u64().unwrap() > baruch_server::mcp::exchange::MCP_EXPORT_LIMIT as u64,
        "RPC liefert"
    );
}

// ---- Kommandozeile -------------------------------------------------------------------------------

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_baruch-server")).args(args).output().expect("baruch-server startet")
}

#[tokio::test]
async fn the_cli_writes_zip_or_xml_never_overwrites_and_refuses_mixed_exports() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("daten");
    let slug = {
        let state = baruch_server::AppState::open(&data).await.unwrap();
        baruch_server::import::import_typst(&state, BOOK, "muehle.typ").await.unwrap().slug
    };
    let data_arg = data.to_str().unwrap();

    let zip_path = dir.path().join("muehle.baruch");
    let out = cli(&["export", &slug, "--baruch", zip_path.to_str().unwrap(), "--data", data_arg]);
    assert!(out.status.success(), "export --baruch: {}", String::from_utf8_lossy(&out.stderr));
    let zip = std::fs::read(&zip_path).unwrap();
    assert!(zip.starts_with(b"PK\x03\x04"), "ohne .xml ein ZIP");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(&baruch_exchange::sha256_hex(&zip)), "Ausgabe nennt die Prüfsumme: {stdout}");
    assert_schema("cli-werk.xml", entry(&unzip(&zip), "werk.xml"));

    let xml_path = dir.path().join("muehle.baruch.xml");
    let out = cli(&["export", &slug, "--baruch", xml_path.to_str().unwrap(), "--data", data_arg]);
    assert!(out.status.success(), "export --baruch .xml: {}", String::from_utf8_lossy(&out.stderr));
    let xml = std::fs::read(&xml_path).unwrap();
    assert!(xml.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<werk "), "bei .xml nur XML");
    assert_schema("cli.baruch.xml", &xml);

    // Vorhandene Datei: Exit 1, nichts überschrieben.
    std::fs::write(&xml_path, "alt").unwrap();
    let out = cli(&["export", &slug, "--baruch", xml_path.to_str().unwrap(), "--data", data_arg]);
    assert_eq!(out.status.code(), Some(1), "vorhandene Datei → Exit 1");
    assert_eq!(std::fs::read_to_string(&xml_path).unwrap(), "alt", "nicht überschrieben");
    assert!(String::from_utf8_lossy(&out.stderr).contains("nichts überschrieben"), "ehrliche Meldung");

    // --baruch mit --out, --pdf oder --stichwort: Gebrauchsfehler, Exit 2, keine Datei.
    let other = dir.path().join("neu.baruch");
    for extra in [["--out", "ordner"], ["--pdf", "buch.pdf"], ["--stichwort", "zettel.pdf"]] {
        let out = cli(&["export", &slug, "--baruch", other.to_str().unwrap(), extra[0], extra[1], "--data", data_arg]);
        assert_eq!(out.status.code(), Some(2), "--baruch mit {}: Exit 2", extra[0]);
        assert!(!other.exists(), "keine Datei bei {}", extra[0]);
    }
    // Unbekanntes Werk → Exit 1.
    let out = cli(&["export", "gibt-es-nicht", "--baruch", other.to_str().unwrap(), "--data", data_arg]);
    assert_eq!(out.status.code(), Some(1), "unbekanntes Werk → Exit 1");
    assert!(!other.exists(), "keine Datei für ein unbekanntes Werk");
}
