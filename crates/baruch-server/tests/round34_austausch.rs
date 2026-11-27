// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 34 „Mindmap“ – Ein- und Ausgabe und Gliederungsansicht (Besitz mindmap_austausch).
//!
//! Geprüft werden die Ausgaben (.mm, .xmind, .md, .svg, .pdf) aus einem von Hand gebauten `MindMap` (drei Ebenen,
//! Wolke, Beziehungen) und über die Routen, das Einlesen (Vorschau legt nichts an; fremde Eingabe wird abgelehnt)
//! und die Gliederungsansicht mit ihren Formularen. Werke, Einfälle und Dateien sind frei erfunden; kein Modell,
//! kein Netz.
//!
//! Stand der Schnittstelle: `tree::import`, `idea.move` und Titel/Elternknoten der Einfälle kommen von
//! mindmap_daten. Wo das Verhalten davon abhängt, prüft der Test beide ehrlichen Ausgänge (S1: Ablehnung mit
//! Meldung, nie 500; danach: es wirkt) und sagt das an der Stelle.

use crate::common;

use std::io::{Cursor, Read, Write};

use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::idea_map::files::import::{
    self, MAX_NOTE_CHARS, MAX_UNPACKED_BYTES, MAX_UPLOAD_BYTES, MAX_ZIP_ENTRIES,
};
use baruch_server::idea_map::files::{self, export, freemind, xmind};
use baruch_server::idea_map::tree::{
    self, ImportLink, ImportNode, ImportTree, MAX_IMPORT_NODES, MAX_TITLE_CHARS, MAX_TREE_DEPTH, MindMap, Scene,
    TreeKind, TreeNode,
};
use baruch_server::idea_map::{MapFilter, Near, NearState};
use baruch_store::{IdeaLink, IdeaOrigin, IdeaStatus, LinkKind};
use serde_json::{Value, json};
use tower::ServiceExt;

// ---- Bausteine -----------------------------------------------------------------------------------

fn human() -> IdeaOrigin {
    IdeaOrigin::Human { name: "Autor".to_owned() }
}

/// Knoten von Hand (alle Felder sind `pub`); Art und Nummern aus der Kennung.
fn node(id: &str, title: &str, children: Vec<TreeNode>) -> TreeNode {
    let idea = id.strip_prefix("i:").and_then(|n| n.parse().ok());
    let key = id.strip_prefix("n:").and_then(|n| n.parse().ok());
    let kind = match (id, idea) {
        ("w", _) => TreeKind::Work,
        (_, Some(_)) => TreeKind::Idea,
        _ => TreeKind::Chapter,
    };
    let below = |n: &TreeNode| n.descendants + u32::from(n.kind == TreeKind::Idea);
    TreeNode {
        id: id.to_owned(),
        kind,
        idea,
        key,
        number: String::new(),
        title: title.to_owned(),
        own_title: if idea.is_some() { title.to_owned() } else { String::new() },
        note: String::new(),
        has_note: false,
        words: Vec::new(),
        status: idea.map(|_| IdeaStatus::Open),
        origin: idea.map(|_| human()),
        section_title: None,
        editable: idea.is_some(),
        hit: false,
        ghost: false,
        open_findings: 0,
        descendants: children.iter().map(below).sum(),
        children,
    }
}

fn link(id: i64, from: i64, to: i64, kind: LinkKind, note: &str) -> IdeaLink {
    IdeaLink {
        id,
        uuid: format!("00000000-0000-4000-8000-{id:012}"),
        from,
        to,
        kind,
        note: note.to_owned(),
        origin: human(),
        created: "2026-11-25T10:00:00Z".to_owned(),
        updated: "2026-11-25T10:00:00Z".to_owned(),
        deleted: None,
    }
}

/// Erfundenes „Werftbuch“: zwei Kapitel, Einfälle über drei Ebenen, zwei Einfälle in der Wolke, zwei Beziehungen.
fn shipyard() -> MindMap {
    let mut keel = node(
        "i:1",
        "Kiel <legen> & \"richten\"",
        vec![node("i:2", "Spanten", vec![node("i:3", "Dampfbiegen", vec![])])],
    );
    keel.note = "Erste Zeile\nZweite & letzte".to_owned();
    keel.has_note = true;
    keel.words = vec!["Holz".to_owned(), "Eiche".to_owned()];
    let mut slip = node("n:10", "Helling", vec![keel, node("i:4", "Planken", vec![])]);
    slip.number = "1".to_owned();
    let mut launch = node("n:20", "Stapellauf", vec![node("i:5", "Sekt", vec![])]);
    launch.number = "2".to_owned();
    let naming = node("i:6", "Namensfindung", vec![node("i:7", "Taufpatin", vec![])]);
    MindMap {
        work: 1,
        title: "Werftbuch".to_owned(),
        root: node("w", "Werftbuch", vec![slip, launch, naming]),
        links: vec![link(31, 1, 5, LinkKind::LeadsTo, "erst dann"), link(32, 4, 5, LinkKind::Contradicts, "")],
        near: Near { state: NearState::Off, message: String::new(), pairs: Vec::new() },
        filter: MapFilter::default(),
        total_ideas: 7,
    }
}

fn leaf(title: &str) -> ImportNode {
    ImportNode { title: title.to_owned(), ..ImportNode::default() }
}

fn branch(title: &str, children: Vec<ImportNode>) -> ImportNode {
    ImportNode { title: title.to_owned(), children, ..ImportNode::default() }
}

/// Was der eigene Leser aus einer Ausgabe von [`shipyard`] holen muss.
/// Die Sicht zum Baum von Hand: Die Einfälle direkt an der Wurzel („Namensfindung“) liegen in der Wolke.
fn scene_of(map: MindMap) -> Scene {
    let cloud: Vec<String> =
        map.root.children.iter().filter(|c| c.kind == TreeKind::Idea).map(|c| c.id.clone()).collect();
    Scene { cloud_count: cloud.len() as u32, cloud, map, hints: Vec::new() }
}

fn shipyard_as_import() -> ImportTree {
    let keel = ImportNode {
        title: "Kiel <legen> & \"richten\"".to_owned(),
        note: "Erste Zeile\nZweite & letzte".to_owned(),
        words: vec!["Holz".to_owned(), "Eiche".to_owned()],
        children: vec![branch("Spanten", vec![leaf("Dampfbiegen")])],
        cloud: false,
    };
    ImportTree {
        title: "Werftbuch".to_owned(),
        nodes: vec![
            branch("1 Helling", vec![keel, leaf("Planken")]),
            branch("2 Stapellauf", vec![leaf("Sekt")]),
            // Runde 36: Der Einfall aus dem Ast „Wolke“ merkt sich die Wolke (`cloud: true`).
            ImportNode { cloud: true, ..branch("Namensfindung", vec![leaf("Taufpatin")]) },
        ],
        links: vec![
            ImportLink { from: vec![0, 0], to: vec![1, 0], note: "führt weiter: erst dann".to_owned() },
            ImportLink { from: vec![0, 1], to: vec![1, 0], note: "widerspricht".to_owned() },
        ],
        warnings: Vec::new(),
    }
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).expect("die Ausgabe ist ein ZIP-Archiv");
    (0..zip.len())
        .map(|index| {
            let mut entry = zip.by_index(index).unwrap();
            let mut content = Vec::new();
            entry.read_to_end(&mut content).unwrap();
            (String::from_utf8_lossy(entry.name_raw()).into_owned(), content)
        })
        .collect()
}

async fn send_bytes(server: &common::Server, request: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = server.app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, headers, bytes.to_vec())
}

/// Hochladen wie das Formular (multipart: wahlweise `parent`, dann `file`).
fn upload(work: i64, bytes: &[u8], parent: Option<i64>) -> Request<Body> {
    let boundary = "----baruch-r34-grenze";
    let mut body = Vec::new();
    if let Some(parent) = parent {
        body.extend(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"parent\"\r\n\r\n{parent}\r\n").into_bytes(),
        );
    }
    body.extend(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"karte\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .into_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
    Request::post(format!("/werke/{work}/karte/einlesen"))
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap()
}

async fn preview(server: &common::Server, work: i64, bytes: &[u8]) -> (StatusCode, String) {
    let (status, _, body) = send_bytes(server, upload(work, bytes, None)).await;
    (status, String::from_utf8(body).unwrap())
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

async fn idea_count(server: &common::Server, work: i64) -> usize {
    result(&server.rpc("idea.list", json!({ "work": work })).await)["ideas"].as_array().map_or(0, Vec::len)
}

struct Book {
    server: common::Server,
    work: i64,
    slip: i64,
    launch: i64,
}

impl Book {
    async fn idea(&self, key: Option<i64>, words: &str, note: &str) -> i64 {
        let mut params = json!({ "work": self.work, "words": words, "note": note });
        if let Some(key) = key {
            params["key"] = json!(key);
        }
        result(&self.server.rpc("idea.create", params).await)["idea"]["id"].as_i64().unwrap()
    }

    fn act(&self) -> String {
        format!("/werke/{}/karte/act", self.work)
    }

    fn outline(&self) -> String {
        format!("/werke/{}/karte/gliederung", self.work)
    }
}

/// Erfundenes Werk „Werftbuch“ mit den Kapiteln „Helling“ und „Stapellauf“.
async fn open_book() -> Book {
    let server = common::open().await;
    let source = "#set document(title: \"Werftbuch\")\n\n= Helling\n\nText.\n\n= Stapellauf\n\nText.\n";
    let work = server.import(source, "werftbuch.typ").await.work;
    let nodes = result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].clone();
    let key =
        |title: &str| nodes.as_array().unwrap().iter().find(|n| n["title"] == title).unwrap()["key"].as_i64().unwrap();
    Book { work, slip: key("Helling"), launch: key("Stapellauf"), server }
}

/// HTML-Maskierung der Vorlage zurücknehmen (für das verborgene Feld mit dem Baum).
fn unescape(html: &str) -> String {
    html.replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&#x2f;", "/")
        .replace("&#x2F;", "/")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn hidden_tree(html: &str) -> ImportTree {
    let start = html.find("name=\"tree\" value=\"").expect("die Vorschau trägt das verborgene Feld tree") + 19;
    let end = start + html[start..].find('"').unwrap();
    serde_json::from_str(&unescape(&html[start..end])).expect("das verborgene Feld ist das JSON des Baums")
}

fn form_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

// ---- Ausgaben aus dem von Hand gebauten Baum -----------------------------------------------------

#[test]
fn the_mm_output_is_well_formed_nested_xml_with_notes_and_arrowlinks() {
    let mm = freemind::write(&export::sheet(&scene_of(shipyard())));
    assert!(mm.starts_with("<map version=\"1.0.1\">"), "FreeMind 1.0.1: {mm}");
    let raw =
        freemind::read(mm.as_bytes()).expect("die Ausgabe ist wohlgeformtes XML (der eigene Leser prüft End-Tags)");
    assert_eq!(raw.root.title, "Werftbuch", "die Wurzel ist das Werk");
    let keel = &raw.root.children[0].children[0];
    assert_eq!(
        (keel.title.as_str(), keel.children[0].title.as_str(), keel.children[0].children[0].title.as_str()),
        ("Kiel <legen> & \"richten\"", "Spanten", "Dampfbiegen"),
        "drei Ebenen Einfälle als verschachtelte node-Elemente, Sonderzeichen kommen unversehrt zurück"
    );
    assert!(
        mm.contains("TEXT=\"Kiel &lt;legen&gt; &amp; &quot;richten&quot;\""),
        "Sonderzeichen sind in Attributen maskiert: {mm}"
    );
    assert!(
        mm.contains("<richcontent TYPE=\"NOTE\"><html><head></head><body><p>Erste Zeile</p><p>Zweite &amp; letzte</p></body></html></richcontent>"),
        "die Notiz steht als richcontent NOTE, je Zeile ein Absatz, maskiert: {mm}"
    );
    assert!(
        mm.contains("<arrowlink DESTINATION=\"ID_i5\" ENDARROW=\"Default\" STARTARROW=\"None\" MIDDLE_LABEL=\"führt weiter: erst dann\""),
        "die Beziehung steht als arrowlink mit Beschriftung (Art und Notiz) am Ausgangsknoten: {mm}"
    );
    assert!(
        mm.contains("STARTARROW=\"Default\" MIDDLE_LABEL=\"widerspricht\""),
        "„widerspricht“ ist ungerichtet: Pfeil an beiden Enden: {mm}"
    );
    assert!(mm.contains("<attribute NAME=\"Schlagwort\" VALUE=\"Eiche\"/>"), "Schlagwörter als Attribute: {mm}");
    assert!(
        mm.contains("TEXT=\"1 Helling\" POSITION=\"right\"") && mm.contains("TEXT=\"Wolke\" POSITION=\"left\""),
        "Hauptäste mit Seite: erste Hälfte rechts, zweite links: {mm}"
    );
    assert!(!mm.contains("FOLDED=\"true\""), "eingeklappt wird in der Ausgabe nie");
}

#[test]
fn the_xmind_output_is_a_zip_with_content_metadata_and_manifest() {
    let bytes = xmind::write(&export::sheet(&scene_of(shipyard()))).expect("die XMind-Datei lässt sich schreiben");
    let files = unzip(&bytes);
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec!["content.json", "metadata.json", "manifest.json"], "genau die drei Dateien des Formats");
    let content: Value = serde_json::from_slice(&files[0].1).expect("content.json ist JSON");
    let sheet = &content[0];
    assert_eq!(sheet["rootTopic"]["title"], "Werftbuch", "rootTopic ist das Werk: {sheet}");
    let attached = sheet["rootTopic"]["children"]["attached"].as_array().expect("children.attached der Wurzel");
    let titles: Vec<&str> = attached.iter().map(|t| t["title"].as_str().unwrap()).collect();
    assert_eq!(
        titles,
        vec!["1 Helling", "2 Stapellauf", "Wolke"],
        "Hauptäste in Reihenfolge, die Wolke als letzter Ast"
    );
    let keel = &attached[0]["children"]["attached"][0];
    assert_eq!(keel["title"], "Kiel <legen> & \"richten\"", "Titel unverändert (JSON maskiert selbst)");
    assert_eq!(keel["notes"]["plain"]["content"], "Erste Zeile\nZweite & letzte", "Notiz als notes.plain.content");
    assert_eq!(keel["labels"], json!(["Holz", "Eiche"]), "Schlagwörter als labels");
    assert_eq!(
        keel["children"]["attached"][0]["children"]["attached"][0]["title"], "Dampfbiegen",
        "Unterknoten verschachtelt über children.attached"
    );
    assert_eq!(
        sheet["relationships"][0],
        json!({ "id": "v:31", "end1Id": "i:1", "end2Id": "i:5", "title": "führt weiter: erst dann" }),
        "Beziehung mit end1Id, end2Id und title"
    );
    assert_eq!(keel["id"], "i:1", "die Enden der Beziehung sind die ids der Themen");
    let manifest: Value = serde_json::from_slice(&files[2].1).unwrap();
    assert!(manifest["file-entries"]["content.json"].is_object(), "manifest.json nennt content.json: {manifest}");
    let metadata: Value = serde_json::from_slice(&files[1].1).unwrap();
    assert_eq!(metadata["creator"]["name"], "Baruch", "metadata.json nennt das erzeugende Programm");
}

#[test]
fn mm_and_xmind_round_trip_to_the_same_import_tree() {
    let sheet = export::sheet(&scene_of(shipyard()));
    let expected = shipyard_as_import();
    let from_mm = import::read(freemind::write(&sheet).as_bytes()).expect(".mm lässt sich wieder lesen");
    assert_eq!(
        from_mm, expected,
        ".mm → eigener Leser: Titel, Notizen, Schlagwörter, Reihenfolge und Beziehungen bleiben; der Ast „Wolke“ ist \
         aufgelöst, sein Einfall trägt cloud = true (Runde 36)"
    );
    let from_xmind = import::read(&xmind::write(&sheet).unwrap()).expect(".xmind lässt sich wieder lesen");
    assert_eq!(from_xmind, expected, ".xmind → eigener Leser ergibt denselben Baum wie .mm");
}

#[test]
fn the_cloud_is_its_own_branch_and_markdown_is_a_nested_list() {
    let map = scene_of(shipyard());
    let loose: Vec<&str> = export::cloud(&map).iter().map(|n| n.title.as_str()).collect();
    assert_eq!(loose, vec!["Namensfindung"], "in der Wolke liegt, was weder Elternknoten noch Bezug hat");
    let sheet = export::sheet(&map);
    let last = sheet.root.children.last().unwrap();
    assert_eq!(
        (last.id.as_str(), last.title.as_str()),
        (export::CLOUD_ID, "Wolke"),
        "Wolken-Einfälle als eigener Ast „Wolke“"
    );
    assert_eq!(
        last.children[0].children[0].title, "Taufpatin",
        "ein Einfall liegt mit seinen Unterknoten in der Wolke"
    );

    let mut without = scene_of(shipyard());
    without.map.root.children.pop();
    assert!(
        export::sheet(&without).root.children.iter().all(|c| c.id != export::CLOUD_ID),
        "ohne lose Einfälle gibt es keinen Ast „Wolke“"
    );

    let md = export::markdown(&map);
    assert!(md.starts_with("# Mindmap: Werftbuch\n"), "Titel: {md}");
    assert!(
        md.contains("- **1 Helling**\n  - Kiel \\<legen\\> & \"richten\" (offen; Schlagwörter: Holz, Eiche; Mensch Autor)\n    Notiz: Erste Zeile Zweite & letzte\n    - Spanten (offen; Mensch Autor)\n      - Dampfbiegen"),
        "verschachtelte Liste: je Ebene zwei Leerzeichen, die Notiz eingerückt unter ihrem Knoten, Markdown maskiert: {md}"
    );
    assert!(md.contains("\n## Wolke\n\n1 in der Wolke"), "eigener Abschnitt für die Wolke mit Zähler: {md}");
    assert!(
        md.contains("- Namensfindung (offen; Mensch Autor)\n  - Taufpatin"),
        "Wolken-Einfälle mit Unterknoten: {md}"
    );
    let relations = md.find("\n## Beziehungen\n").expect("Abschnitt Beziehungen");
    let contradictions = md.find("\n## Widersprüche\n").expect("Abschnitt Widersprüche");
    assert!(relations < contradictions, "erst Beziehungen, dann Widersprüche: {md}");
    assert!(
        md.contains("- Kiel \\<legen\\> & \"richten\" (Einfall 1) → Sekt (Einfall 5): führt weiter: erst dann"),
        "Beziehung mit Art und Notiz: {md}"
    );
    assert!(
        md[contradictions..].contains("- Planken (Einfall 4) ↔ Sekt (Einfall 5)"),
        "„widerspricht“ steht auch unter Widersprüche: {md}"
    );
}

#[test]
fn the_pdf_source_embeds_the_standalone_drawing() {
    let map = shipyard();
    let source = files::pdf_source(&map, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_owned());
    assert!(source.main.contains("\"Mindmap: Werftbuch\""), "Titel nur als Typst-Zeichenkette: {}", source.main);
    assert!(source.main.contains("image(\"mindmap.svg\""), "das Bild der Mindmap füllt die Seite: {}", source.main);
    assert_eq!(source.files.len(), 1, "genau eine Datei neben der Quelle");
    assert_eq!(source.files[0].0, "mindmap.svg", "die übergebene SVG-Datei liegt unter ihrem Namen bei");
}

// ---- Ausgaben über die Routen --------------------------------------------------------------------

#[tokio::test]
async fn the_routes_deliver_every_output_and_take_the_filters() {
    let b = open_book().await;
    let keel = b.idea(Some(b.slip), "Kiel", "Eiche vom Deich").await;
    let champagne = b.idea(Some(b.launch), "Sekt", "").await;
    b.idea(None, "Namensfindung", "").await;
    let answer = b.server.rpc("idea.link", json!({ "from": keel, "to": champagne, "kind": "contradicts" })).await;
    result(&answer);

    let base = format!("/werke/{}/karte", b.work);
    let (status, headers, body) = b.server.send(Request::get(format!("{base}.svg")).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "karte.svg antwortet");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "als SVG");
    assert!(
        body.contains("class=\"mm-svg\"") && body.contains("xmlns=\"http://www.w3.org/2000/svg\""),
        "die Datei kommt aus bild::mindmap::render, eigenständig (mit xmlns): {body}"
    );
    assert!(body.contains("Werftbuch") && body.contains("Kiel"), "sie zeigt Werk und Einfall: {body}");

    let (status, headers, md) = b.server.send(Request::get(format!("{base}.md")).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "karte.md antwortet");
    assert_eq!(headers[header::CONTENT_TYPE], "text/markdown; charset=utf-8", "als Markdown");
    assert!(md.contains("- **1 Helling**\n  - Kiel"), "verschachtelte Liste Kapitel → Einfall: {md}");
    assert!(md.contains("## Wolke\n\n1 in der Wolke"), "der Einfall ohne Platz liegt in der Wolke: {md}");
    assert!(md.contains("## Beziehungen") && md.contains("## Widersprüche"), "beide Abschnitte: {md}");
    assert!(
        md.contains(&format!("(Einfall {keel}) ↔ Sekt (Einfall {champagne})")),
        "der Widerspruch steht darin: {md}"
    );

    let (_, _, filtered) =
        b.server.send(Request::get(format!("{base}.md?word=Sekt")).body(Body::empty()).unwrap()).await;
    assert!(
        filtered.contains("Sekt") && !filtered.contains("- Kiel"),
        "der Filter der Seite gilt auch für die Ausgabe: {filtered}"
    );
    assert!(filtered.contains("Filter: Schlagwort „Sekt“"), "und wird genannt: {filtered}");

    let (status, headers, mm) =
        b.server.send(Request::get(format!("{base}.mm?word=Sekt")).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "karte.mm antwortet");
    assert_eq!(headers[header::CONTENT_TYPE], "application/x-freemind", "als FreeMind-Datei");
    assert!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap().ends_with("-mindmap.mm\""),
        "als Anhang mit dem Namen des Werks: {:?}",
        headers[header::CONTENT_DISPOSITION]
    );
    assert!(mm.contains("TEXT=\"Sekt\"") && !mm.contains("TEXT=\"Kiel\""), "gefiltert wie die Seite: {mm}");

    let request = Request::get(format!("{base}.xmind")).body(Body::empty()).unwrap();
    let (status, headers, bytes) = send_bytes(&b.server, request).await;
    assert_eq!(status, StatusCode::OK, "karte.xmind antwortet");
    assert_eq!(headers[header::CONTENT_TYPE], "application/vnd.xmind.workbook", "als XMind-Datei");
    let tree = import::read(&bytes).expect("die ausgelieferte XMind-Datei liest der eigene Leser");
    assert_eq!(tree.title, "Werftbuch", "mit dem Werk als Hauptthema");
    assert_eq!(
        tree.nodes.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
        vec!["1 Helling", "2 Stapellauf", "Namensfindung"],
        "Kapitel als Äste, der Wolken-Einfall (Ast „Wolke“ aufgelöst) dahinter"
    );
    assert_eq!(tree.links.len(), 1, "samt der Beziehung");

    let (status, _) = b.server.get("/werke/999999/karte.xmind").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn the_pdf_is_built_from_the_mindmap_drawing() {
    let _serial = common::PDF_SERIAL.lock().await;
    let b = open_book().await;
    b.idea(Some(b.slip), "Kiel", "").await;
    let request = Request::get(format!("/werke/{}/karte.pdf", b.work)).body(Body::empty()).unwrap();
    let (status, headers, bytes) = send_bytes(&b.server, request).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "karte.pdf antwortet: {}",
        String::from_utf8_lossy(&bytes[..bytes.len().min(300)])
    );
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf", "als PDF");
    assert!(bytes.starts_with(b"%PDF"), "Typst hat aus Titel und eigenständiger SVG-Datei ein PDF gesetzt");
    assert!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains("-mindmap.pdf"),
        "Dateiname nennt die Mindmap: {:?}",
        headers[header::CONTENT_DISPOSITION]
    );
}

// ---- Einlesen ------------------------------------------------------------------------------------

#[tokio::test]
async fn the_preview_creates_nothing_and_only_the_confirmation_imports() {
    let b = open_book().await;
    let (status, html) = b.server.get(&format!("/werke/{}/karte/einlesen", b.work)).await;
    assert_eq!(status, StatusCode::OK, "das Formular zum Einlesen antwortet");
    assert!(
        html.contains("enctype=\"multipart/form-data\"") && html.contains("name=\"file\""),
        "Formular mit Datei-Upload, ohne JavaScript: {html}"
    );
    assert!(html.contains("als Hauptthemen ans Werk"), "Vorgabe: Die eingelesenen Knoten werden Hauptthemen am Werk");

    let file = include_bytes!("data/r34/hafenfest.mm");
    let (status, html) = preview(&b.server, b.work, file).await;
    assert_eq!(status, StatusCode::OK, "die Vorschau antwortet: {html}");
    assert!(
        html.contains("data-import-preview") && html.contains("data-count=\"6\""),
        "Vorschau mit der Zahl der Knoten: {html}"
    );
    for title in ["Buden", "Räucherfisch", "Teig am Vorabend", "Shanty-Chor"] {
        assert!(html.contains(title), "die Vorschau zeigt den Knoten „{title}“");
    }
    assert!(html.contains("Noch ist nichts angelegt"), "die Vorschau sagt, dass noch nichts angelegt ist");
    assert_eq!(idea_count(&b.server, b.work).await, 0, "die Vorschau legt keinen Einfall an");

    let tree = hidden_tree(&html);
    assert_eq!(tree, import::read(file).unwrap(), "das verborgene Feld trägt genau den gelesenen Baum");
    assert_eq!(tree.title, "Hafenfest", "die Wurzel der Datei wird Titel, nicht Knoten");
    assert_eq!(tree.nodes[0].note, "Fisch & Waffeln\nStrom vom Kran", "HTML in der Notiz ist reiner Text geworden");
    assert_eq!(tree.nodes[0].words, vec!["Markt"], "nur das Attribut „Schlagwort“ wird Schlagwort");
    assert_eq!(
        tree.links,
        vec![ImportLink { from: vec![0], to: vec![1], note: "braucht".to_owned() }],
        "Beziehung mit Beschriftung"
    );

    let apply = format!("/werke/{}/karte/einlesen/anlegen", b.work);
    let body = format!("tree={}&parent=", form_encode(&serde_json::to_string(&tree).unwrap()));
    let request = Request::post(&apply)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();
    let (status, headers, page) = b.server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "die Bestätigung legt an: {page}");
    assert_eq!(
        headers[header::LOCATION],
        format!("/werke/{}/karte?ok=eingelesen", b.work).as_str(),
        "nach dem Anlegen zurück zur Mindmap"
    );
    assert_eq!(idea_count(&b.server, b.work).await, 6, "alle Knoten der Vorschau sind Einfälle geworden");

    let request = Request::post(&apply)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("tree=%7Bkaputt&parent="))
        .unwrap();
    let (status, _, page) = b.server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ein beschädigtes Feld tree wird abgelehnt");
    assert!(page.contains("Die Vorschau ist beschädigt"), "mit verständlicher Meldung: {page}");

    let deep = (0..=MAX_TREE_DEPTH).fold(leaf("unten"), |below, level| branch(&format!("e{level}"), vec![below]));
    let forged = serde_json::to_string(&ImportTree { nodes: vec![deep], ..ImportTree::default() }).unwrap();
    let request = Request::post(&apply)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("tree={}&parent=", form_encode(&forged))))
        .unwrap();
    let (status, _, page) = b.server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "der Baum aus dem Formular wird serverseitig erneut geprüft");
    assert!(page.contains(&import::too_deep()), "ein gefälschter, zu tiefer Baum wird abgelehnt: {page}");

    let other = b.server.import("#set document(title: \"Fremdes Buch\")\n\n= Eins\n\nText.\n", "fremd.typ").await.work;
    let foreign = result(&b.server.rpc("idea.create", json!({ "work": other, "words": "Fremd" })).await)["idea"]["id"]
        .as_i64()
        .unwrap();
    let (status, _, page) = send_bytes(&b.server, upload(b.work, file, Some(foreign))).await;
    assert_eq!(status, StatusCode::CONFLICT, "ein Zielknoten aus einem anderen Werk wird abgelehnt");
    assert!(String::from_utf8_lossy(&page).contains("gehört nicht zu diesem Werk"), "mit Meldung");

    let request = upload(b.work, file, None);
    let (parts, body) = request.into_parts();
    let mut request = Request::from_parts(parts, body);
    request.headers_mut().insert(header::ORIGIN, "http://fremd.example".parse().unwrap());
    let (status, _, _) = send_bytes(&b.server, request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "das Einlesen bleibt hinter guard::same_origin");
}

#[tokio::test]
async fn foreign_input_is_refused_with_a_clear_message_and_no_crash() {
    let b = open_book().await;
    let work = b.work;

    let huge = vec![b'a'; MAX_UPLOAD_BYTES + 1024 * 1024];
    let (status, html) = preview(&b.server, work, &huge).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "Upload über 10 MiB → 413");
    assert!(
        html.contains("data-import-error") && html.contains("größer als 10 MiB"),
        "mit verständlicher Meldung: {html}"
    );

    let many: Vec<(String, Vec<u8>)> = (0..=MAX_ZIP_ENTRIES).map(|i| (format!("bild{i}.txt"), vec![b'x'])).collect();
    let mut entries: Vec<(&str, &[u8])> = many.iter().map(|(n, c)| (n.as_str(), c.as_slice())).collect();
    entries.push(("content.json", b"[]"));
    let (status, html) = preview(&b.server, work, &zip_of(&entries)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ZIP mit mehr als 50 Einträgen → abgelehnt");
    assert!(html.contains("erlaubt sind höchstens 50"), "Meldung nennt die Grenze: {html}");

    // Zip-Bombe: 21 MiB Leerzeichen schrumpfen auf wenige Kilobyte; gezählt wird beim Entpacken.
    let blown = vec![b' '; MAX_UNPACKED_BYTES as usize + 1024 * 1024];
    let bomb = zip_of(&[("content.json", &blown)]);
    assert!(bomb.len() < 200 * 1024, "die Testdatei ist hoch komprimiert ({} Bytes)", bomb.len());
    let (status, html) = preview(&b.server, work, &bomb).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "entpackt über 20 MiB → abgelehnt");
    assert!(html.contains("entpackt größer als 20 MiB"), "Meldung nennt die Grenze: {html}");

    let good =
        json!([{ "rootTopic": { "title": "Karte", "children": { "attached": [{ "title": "Ast" }] } } }]).to_string();
    for name in ["../../boese.txt", "/etc/boese.txt"] {
        let archive = zip_of(&[("content.json", good.as_bytes()), (name, b"x")]);
        let (status, html) = preview(&b.server, work, &archive).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Archiv mit dem Eintrag {name} → abgelehnt");
        assert!(html.contains("aus dem Archiv herausführte"), "Meldung zu {name}: {html}");
    }
    assert!(
        !b.server.path().join("boese.txt").exists() && !b.server.path().parent().unwrap().join("boese.txt").exists(),
        "kein Name aus dem Archiv wurde je als Pfad benutzt"
    );

    let doctype = "<?xml version=\"1.0\"?><!DOCTYPE map [<!ENTITY x SYSTEM \"file:///etc/hostname\">]>\
        <map version=\"1.0.1\"><node TEXT=\"&x;\"><node TEXT=\"a\"/></node></map>";
    let (status, html) = preview(&b.server, work, doctype.as_bytes()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "XML mit DOCTYPE → abgelehnt");
    assert!(html.contains("DOCTYPE"), "Meldung nennt den Grund: {html}");
    let entity = "<map version=\"1.0.1\"><node TEXT=\"w\"><node TEXT=\"&eigene;\"/></node></map>";
    let (status, html) = preview(&b.server, work, entity.as_bytes()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "eigene Entitäten → abgelehnt");
    assert!(html.contains("Entitäten"), "Meldung nennt den Grund: {html}");

    let mut wide = String::from("<map version=\"1.0.1\"><node TEXT=\"w\">");
    for i in 0..=MAX_IMPORT_NODES {
        wide.push_str(&format!("<node TEXT=\"k{i}\"/>"));
    }
    wide.push_str("</node></map>");
    let (status, html) = preview(&b.server, work, wide.as_bytes()).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "mehr als MAX_IMPORT_NODES Knoten → abgelehnt");
    assert!(html.contains(&import::too_many_nodes()), "Meldung nennt die Grenze: {html}");

    let mut topic = json!({ "title": "unten" });
    for level in 0..=MAX_TREE_DEPTH {
        topic = json!({ "title": format!("e{level}"), "children": { "attached": [topic] } });
    }
    let deep = json!([{ "rootTopic": { "title": "w", "children": { "attached": [topic] } } }]).to_string();
    let (status, html) = preview(&b.server, work, &zip_of(&[("content.json", deep.as_bytes())])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Tiefe über MAX_TREE_DEPTH → abgelehnt");
    assert!(html.contains(&import::too_deep()), "Meldung nennt die Grenze: {html}");

    let (status, html) = preview(&b.server, work, &zip_of(&[("content.json", b"{ \"rootTopic\": ")])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "kaputtes JSON → abgelehnt");
    assert!(html.contains("kein gültiges JSON"), "mit Meldung: {html}");
    let (status, html) = preview(&b.server, work, b"<map version=\"1.0.1\"><node TEXT=\"a\"><node></map>").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "kaputtes XML → abgelehnt");
    assert!(unescape(&html).contains("kein lesbares FreeMind-/Freeplane-XML"), "mit Meldung: {html}");
    let (status, html) = preview(&b.server, work, b"PK\x03\x04 kein Archiv").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "kaputtes ZIP → abgelehnt");
    assert!(html.contains("kein lesbares XMind-Archiv"), "mit Meldung: {html}");
    let (status, html) = preview(&b.server, work, b"Nur ein Text.").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "weder XML noch ZIP → abgelehnt");
    assert!(unescape(&html).contains("weder eine FreeMind-/Freeplane-Datei"), "mit Meldung: {html}");
    let old = zip_of(&[("content.xml", b"<xmap-content/>"), ("META-INF/manifest.xml", b"<manifest/>")]);
    let (status, html) = preview(&b.server, work, &old).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "altes XMind-8-Format → abgelehnt (Entscheidung, dokumentiert)"
    );
    assert!(html.contains("XMind-8-Format"), "mit Hinweis, was zu tun ist: {html}");

    let long = json!([{ "rootTopic": { "title": "Karte", "children": { "attached": [
        { "title": "t".repeat(MAX_TITLE_CHARS + 20), "notes": { "plain": { "content": "n".repeat(MAX_NOTE_CHARS + 20) } } }
    ] } } }])
    .to_string();
    let (status, html) = preview(&b.server, work, &zip_of(&[("content.json", long.as_bytes())])).await;
    assert_eq!(status, StatusCode::OK, "überlange Texte sind kein Grund zur Ablehnung: {html}");
    assert!(
        html.contains("1 Titel waren länger als 200 Zeichen und wurden gekürzt")
            && html.contains("1 Notiz(en) waren länger als 4000 Zeichen und wurden gekürzt"),
        "die Vorschau nennt die Kürzungen (warnings): {html}"
    );
    let tree = hidden_tree(&html);
    assert_eq!(
        (tree.nodes[0].title.chars().count(), tree.nodes[0].note.chars().count()),
        (MAX_TITLE_CHARS, MAX_NOTE_CHARS),
        "Titel und Notiz sind auf ihre Höchstlänge gekürzt"
    );

    assert_eq!(idea_count(&b.server, work).await, 0, "keine der abgelehnten Dateien hat etwas angelegt");
    let (status, _) = b.server.get(&format!("/werke/{work}/karte/gliederung")).await;
    assert_eq!(status, StatusCode::OK, "der Server läuft nach all dem weiter");
}

// ---- Gliederungsansicht und Formulare ------------------------------------------------------------

#[tokio::test]
async fn the_outline_view_shows_tree_cloud_and_forms_without_javascript() {
    let b = open_book().await;
    let keel = b.idea(Some(b.slip), "Kiel, Eiche", "Holz vom Deich").await;
    let champagne = b.idea(Some(b.launch), "Sekt", "").await;
    let naming = b.idea(None, "Namensfindung, Eiche", "").await;
    result(
        &b.server
            .rpc("idea.link", json!({ "from": keel, "to": champagne, "kind": "contradicts", "note": "nass" }))
            .await,
    );

    let (status, html) = b.server.get(&b.outline()).await;
    assert_eq!(status, StatusCode::OK, "die Gliederungsansicht antwortet");
    let slip = html.find(&format!("data-node=\"n:{}\"", b.slip)).expect("Kapitel Helling als Knoten");
    let idea = html.find(&format!("data-node=\"i:{keel}\"")).expect("der Einfall als Knoten");
    let launch = html.find(&format!("data-node=\"n:{}\"", b.launch)).expect("Kapitel Stapellauf als Knoten");
    assert!(slip < idea && idea < launch, "der Einfall steht in der Liste unter seinem Kapitel, vor dem nächsten");
    assert!(
        html[slip..idea].contains("<details open><summary>") && html[slip..idea].contains("<ul class=\"mmg-list\">"),
        "verschachtelte Liste; Unterknoten in <details>, aufgeklappt, ohne JavaScript zuklappbar"
    );
    assert!(html.contains("data-cloud-count=\"1\"") && html.contains("1 in der Wolke"), "Zähler der Wolke: {html}");
    let cloud = html.find("id=\"wolke\"").expect("Bereich Wolke");
    assert!(
        html[cloud..].contains(&format!("data-node=\"i:{naming}\"")),
        "der Einfall ohne Platz liegt in der Wolke, nicht im Baum"
    );
    assert!(
        html[cloud..].contains("passt zu Ast „1 Helling“")
            && html[cloud..].contains(&format!("name=\"ziel\" value=\"i:{keel}\"")),
        "die Nähe (geteiltes Schlagwort „Eiche“) schlägt einen Ast vor, mit Formular zum Einhängen: {}",
        &html[cloud..]
    );
    assert!(html.contains("data-list=\"contradictions\""), "Liste der Widersprüche");
    assert!(html.contains("class=\"mmg-contradiction\""), "der Widerspruch ist hervorgehoben");
    assert!(!html.contains("id=\"wahl\""), "ohne gewählten Knoten keine Formulare zum Knoten");
    for (label, address) in
        [("Ausgabe", ".xmind"), ("Ausgabe", ".mm"), ("Einlesen", "/karte/einlesen"), ("Zurück", "/einfaelle?work=")]
    {
        assert!(
            html.contains(&format!("<summary class=\"band-group-label\">{label}</summary>")),
            "Menüband-Gruppe „{label}“"
        );
        assert!(html.contains(address), "Menüband verweist auf {address}");
    }
    assert!(
        html.contains("<details class=\"band-group\" data-band-group open>"),
        "Gruppen wie in Runde 33 (Laptop-Modus)"
    );

    let act = b.act();
    let (status, html) = b.server.get(&format!("{}?status=open&wahl=i:{keel}", b.outline())).await;
    assert_eq!(status, StatusCode::OK, "mit gewähltem Knoten");
    let panel = &html[html.find("id=\"wahl\"").expect("Formulare des gewählten Knotens")..];
    let panel = &panel[..panel.find("</section>").unwrap()];
    for (form, method) in [
        ("rename", "idea.update"),
        ("child", "idea.create"),
        ("sibling", "idea.create"),
        ("move", "idea.move"),
        ("cloud", "idea.move"),
        ("section", "idea.update"),
        ("link", "idea.link"),
        ("links", "idea.unlink"),
        ("delete", "idea.delete"),
    ] {
        let at =
            panel.find(&format!("data-form=\"{form}\"")).unwrap_or_else(|| panic!("Formular „{form}“ fehlt: {panel}"));
        assert!(
            panel[at..].contains(&format!("name=\"method\" value=\"{method}\"")),
            "Formular „{form}“ ruft {method}"
        );
    }
    assert_eq!(
        panel.matches(&format!("action=\"{act}\"")).count(),
        panel.matches("<form").count(),
        "jedes Formular postet an /werke/{{id}}/karte/act"
    );
    assert_eq!(
        panel.matches("name=\"seite\" value=\"gliederung\"").count(),
        panel.matches("<form").count(),
        "jedes Formular kehrt in die Gliederungsansicht zurück"
    );
    assert!(panel.contains("name=\"back\" value=\"status=open\""), "und behält den Filter (back)");
    assert!(panel.contains(&format!("<option value=\"n:{}\"", b.launch)), "Umhängen: Kapitel als Ziel wählbar");
    assert!(!panel.contains(&format!("<option value=\"i:{keel}\"")), "der Knoten selbst ist kein Ziel (kein Kreis)");

    let (_, html) = b.server.get(&format!("{}?wahl=i:{naming}", b.outline())).await;
    let panel = &html[html.find("id=\"wahl\"").unwrap()..];
    let panel = &panel[..panel.find("</section>").unwrap()];
    assert!(
        panel.contains("In die Mindmap unter") && panel.contains("In die Mindmap hängen"),
        "Wolken-Einfall: „in die Mindmap unter …“"
    );
    assert!(!panel.contains("data-form=\"cloud\""), "er liegt schon in der Wolke: kein „In die Wolke“");

    let (_, html) = b.server.get(&format!("{}?wahl=n:{}", b.outline(), b.slip)).await;
    let panel = &html[html.find("id=\"wahl\"").unwrap()..];
    let panel = &panel[..panel.find("</section>").unwrap()];
    assert!(panel.contains("data-form=\"child\""), "an ein Kapitel lässt sich ein Unterknoten hängen");
    assert_eq!(panel.matches("<form").count(), 1, "sonst kein Formular: Kapitel sind Gliederung");
    assert!(
        !panel.contains("idea.update") && !panel.contains("idea.delete") && !panel.contains("idea.move"),
        "Teile/Kapitel haben keine Formulare zum Umbenennen, Umhängen oder Löschen: {panel}"
    );

    let (status, html) = b.server.get(&format!("{}?zu=n:{}", b.outline(), b.slip)).await;
    assert_eq!(status, StatusCode::OK, "mit zu=");
    let at = html.find(&format!("data-node=\"n:{}\"", b.slip)).unwrap();
    let start: String = html[at..].chars().take(200).collect();
    assert!(start.contains("<details><summary>"), "zu= klappt den Knoten vorab zu: {start}");

    let css = include_str!("../static/karte_gliederung.css");
    assert!(
        !css.lines().any(|line| !line.trim_start().starts_with("/*") && line.contains('#')),
        "das Stylesheet nennt keine festen Farben, nur die Variablen aus app.css (hell und dunkel lesbar)"
    );
    assert!(css.contains("@media (max-width: 900px)"), "schmale Fenster (Laptop-Modus): Wolke unter dem Baum");
    let (status, _) = b.server.get("/werke/999999/karte/gliederung").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

#[tokio::test]
async fn the_forms_post_to_act_and_return_to_the_outline() {
    let b = open_book().await;
    let act = b.act();
    let outline = b.outline();
    let keel = b.idea(Some(b.slip), "Kiel", "").await;
    let champagne = b.idea(Some(b.launch), "Sekt", "").await;

    // Anlegen: mit Bezug (Kapitel) und – ohne Platz – in die Wolke.
    let form =
        format!("method=idea.create&words=Planken&key={}&back=near%3D0&seite=gliederung&wahl=n:{}", b.slip, b.slip);
    let (status, location) = b.server.post_form(&act, &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anlegen leitet weiter");
    assert_eq!(
        location.as_deref(),
        Some(format!("{outline}?near=0&ok=angelegt&wahl=n:{}", b.slip).as_str()),
        "zurück in die Gliederungsansicht, mit Filter, Rückmeldung und gewähltem Knoten"
    );
    let (status, location) = b.server.post_form(&act, "method=idea.create&words=Taufe&seite=gliederung").await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=angelegt").as_str())),
        "neuer Einfall ohne Platz"
    );
    assert_eq!(idea_count(&b.server, b.work).await, 4, "beide Einfälle sind angelegt");
    let (_, html) = b.server.get(&format!("{outline}?ok=angelegt")).await;
    assert!(html.contains("role=\"status\">Einfall angelegt."), "die Seite meldet es: {html}");
    assert!(html.contains("data-cloud-count=\"1\""), "der Einfall ohne Platz liegt in der Wolke");

    // Bezug setzen (idea.update mit key) – Umbenennen (title) wirkt, sobald mindmap_daten den Titel speichert.
    let form = format!("method=idea.update&id={champagne}&key={}&seite=gliederung&wahl=i:{champagne}", b.slip);
    let (status, location) = b.server.post_form(&act, &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Bezug setzen leitet weiter");
    assert_eq!(
        location.as_deref(),
        Some(format!("{outline}?ok=bezug&wahl=i:{champagne}").as_str()),
        "mit Rückmeldung „bezug“"
    );
    let stored = result(&b.server.rpc("idea.get", json!({ "id": champagne })).await)["idea"].clone();
    assert_eq!(stored["key"].as_i64(), Some(b.slip), "der Bezug ist gespeichert");
    let form = format!("method=idea.update&id={champagne}&title=Schaumwein&seite=gliederung");
    let (status, location) = b.server.post_form(&act, &form).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=umbenannt").as_str())),
        "Umbenennen"
    );
    let stored = result(&b.server.rpc("idea.get", json!({ "id": champagne })).await)["idea"].clone();
    assert_eq!(
        stored["key"].as_i64(),
        Some(b.slip),
        "Umbenennen lässt den Bezug stehen (das Feld key fehlte im Formular)"
    );
    assert_eq!(stored["title"], "Schaumwein", "der Titel ist gesetzt: {stored}");

    // Umhängen unter einen anderen Einfall.
    let form = format!("method=idea.move&id={champagne}&ziel=i:{keel}&seite=gliederung");
    let (status, location) = b.server.post_form(&act, &form).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=umgehaengt").as_str())),
        "umgehängt, zurück in die Ansicht"
    );
    let stored = result(&b.server.rpc("idea.get", json!({ "id": champagne })).await)["idea"].clone();
    assert_eq!(stored["parent"].as_i64(), Some(keel), "der Einfall hängt unter dem gewählten Knoten: {stored}");
    let (status, _) = b.server.post_form(&act, &format!("method=idea.move&id={champagne}&ziel=unsinn")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ein unbekanntes Ziel ist eine ungültige Eingabe");

    // Verbinden, Lösen, Wiederherstellen.
    let form = format!("method=idea.link&from={keel}&to={champagne}&kind=leads_to&note=erst+dann&seite=gliederung");
    let (status, location) = b.server.post_form(&act, &form).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=verbunden").as_str())),
        "Verbinden"
    );
    let links = result(&b.server.rpc("idea.links", json!({ "idea": keel })).await)["links"].clone();
    let link = links[0]["id"].as_i64().expect("die Verbindung gibt es");
    let (_, html) = b.server.get(&outline).await;
    assert!(
        html.contains(&format!("data-link=\"{link}\"")) && html.contains("führt weiter: erst dann"),
        "sie steht unter „Beziehungen“"
    );
    let (status, location) = b.server.post_form(&act, &format!("method=idea.unlink&id={link}&seite=gliederung")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=geloest&undo={link}").as_str())),
        "Lösen nennt die gelöste Verbindung"
    );
    let (_, html) = b.server.get(&format!("{outline}?ok=geloest&undo={link}")).await;
    assert!(html.contains("name=\"method\" value=\"idea.link_restore\""), "die Meldung bietet „Wiederherstellen“ an");
    let (status, location) =
        b.server.post_form(&act, &format!("method=idea.link_restore&id={link}&seite=gliederung")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=wiederhergestellt").as_str())),
        "Wiederherstellen"
    );

    // Löschen und Wiederherstellen eines Einfalls.
    let before = idea_count(&b.server, b.work).await;
    let form = format!("method=idea.delete&id={keel}&seite=gliederung&wahl=i:{keel}");
    let (status, location) = b.server.post_form(&act, &form).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=geloescht&wieder={keel}").as_str())),
        "Löschen nennt den Einfall zum Rückgängigmachen; der gelöschte Knoten bleibt nicht gewählt"
    );
    assert!(idea_count(&b.server, b.work).await < before, "der Einfall ist gelöscht");
    let (_, html) = b.server.get(&format!("{outline}?ok=geloescht&wieder={keel}")).await;
    assert!(html.contains(">Rückgängig</button>"), "die Meldung bietet „Rückgängig“ an");
    let deleted = &html[html.find("Gelöschte Einfälle</h2>").expect("Abschnitt der gelöschten Einfälle")..];
    assert!(
        deleted.contains(&format!("(Einfall {keel}")) && deleted.contains("idea.restore"),
        "er steht bei den gelöschten, mit Formular"
    );
    let (status, location) = b.server.post_form(&act, &format!("method=idea.restore&id={keel}&seite=gliederung")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("{outline}?ok=einfall-wieder").as_str())),
        "Wiederherstellen"
    );
    assert_eq!(idea_count(&b.server, b.work).await, before, "der Einfall ist wieder da");

    // Ohne seite: zurück zur Mindmap (Vorgabe karte).
    let (status, location) = b.server.post_form(&act, &format!("method=idea.update&id={keel}&key={}", b.launch)).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{}/karte?ok=bezug", b.work).as_str())),
        "Vorgabe der Weiterleitung ist die Mindmap"
    );
}

#[tokio::test]
async fn act_refuses_unknown_methods_foreign_ideas_and_foreign_origins() {
    let b = open_book().await;
    let act = b.act();
    let own = b.idea(Some(b.slip), "Kiel", "").await;
    let other = b.server.import("#set document(title: \"Fremdes Buch\")\n\n= Eins\n\nText.\n", "fremd.typ").await.work;
    let foreign = result(&b.server.rpc("idea.create", json!({ "work": other, "words": "Fremd" })).await)["idea"]["id"]
        .as_i64()
        .unwrap();

    let request = |form: String| {
        Request::post(&act)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form))
            .unwrap()
    };
    let (status, _, page) = b.server.send(request("method=idea.purge&id=1&seite=gliederung".to_owned())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Methode → 400");
    assert!(page.contains("Unbekannte Aktion „idea.purge“"), "mit Meldung: {page}");
    let (status, _, _) = b.server.send(request("method=work.delete&id=1".to_owned())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "das Formular führt nur die Methoden der Mindmap aus");

    for form in [
        format!("method=idea.delete&id={foreign}&seite=gliederung"),
        format!("method=idea.update&id={foreign}&title=Gekapert&seite=gliederung"),
        format!("method=idea.move&id={foreign}&ziel=wolke&seite=gliederung"),
        format!("method=idea.move&id={own}&ziel=i:{foreign}&seite=gliederung"),
        format!("method=idea.create&words=Kuckuck&parent={foreign}&seite=gliederung"),
        format!("method=idea.link&from={foreign}&to={own}&kind=leads_to&seite=gliederung"),
        format!("method=idea.restore&id={foreign}&seite=gliederung"),
    ] {
        let (status, _, page) = b.server.send(request(form.clone())).await;
        assert_eq!(status, StatusCode::CONFLICT, "Einfall eines anderen Werks → 409 ({form})");
        assert!(page.contains("gehört nicht zu diesem Werk."), "mit Meldung ({form}): {page}");
    }
    let still = result(&b.server.rpc("idea.get", json!({ "id": foreign })).await)["idea"].clone();
    assert!(still["deleted"].is_null(), "der fremde Einfall ist unberührt: {still}");
    assert_eq!(idea_count(&b.server, b.work).await, 1, "und im eigenen Werk ist nichts dazugekommen");

    let request = Request::post(&act)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://fremd.example")
        .body(Body::from(format!("method=idea.delete&id={own}")))
        .unwrap();
    let (status, _, _) = b.server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft → 403 (guard::same_origin)");
    assert_eq!(idea_count(&b.server, b.work).await, 1, "nichts gelöscht");

    // Die Gruppen-Formulare aus Runde 19b laufen weiter über dasselbe Formular.
    let (status, location) = b.server.post_form(&act, "method=idea.map_group_dismiss&group=g1&back=near%3D0").await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{}/karte?near=0&ok=verworfen", b.work).as_str())),
        "idea.map_group_dismiss wird weiter bedient"
    );
    let (status, _) = b.server.post_form(&act, "method=idea.map_group_accept&group=unsinn").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "idea.map_group_accept prüft seine Gruppe wie zuvor");
    let (status, location) = b.server.post_form(&act, "method=idea.map_groups_name&back=near%3D0").await;
    assert_eq!(
        status,
        StatusCode::SEE_OTHER,
        "idea.map_groups_name wird weiter bedient (ohne Gruppen kein Modellauftrag)"
    );
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{}/karte?near=0&ok=keine-gruppen", b.work).as_str()),
        "ohne nahe Einfälle gibt es keine Gruppe"
    );
    let (status, _) = b.server.post_form("/werke/999999/karte/act", "method=idea.delete&id=1").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
}

/// `tree::import` legt genau die Knoten des Baums an (mehr dazu in `round34_daten`).
#[tokio::test]
async fn import_creates_exactly_the_tree() {
    let b = open_book().await;
    let wanted = ImportTree {
        title: "Fremde Karte".to_owned(),
        nodes: vec![branch("Tau", vec![leaf("Knoten")])],
        ..ImportTree::default()
    };
    let report = tree::import(&b.server.state, b.work, None, &wanted, &human()).await.expect("der Import gelingt");
    assert_eq!(report.ideas.len(), 2, "jeder Knoten des Baums wird ein Einfall");
    assert_eq!(idea_count(&b.server, b.work).await, 2, "und steht in der Liste des Werks");
    let json = serde_json::to_string(&wanted).unwrap();
    let back: ImportTree = serde_json::from_str(&json).unwrap();
    assert_eq!(back, wanted, "ImportTree übersteht JSON (verborgenes Formularfeld zwischen Vorschau und Bestätigung)");
}
