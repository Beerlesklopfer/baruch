// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 36 „Reste“, Besitz reste: Reste der Mindmap aus Runde 34 (Wolke in der Rundreise, Beschriftungen, dunkler Modus).
//!
//! Werke, Einfälle und Dateien sind frei erfunden (ein „Deichbuch“); kein Modell, kein Netz.

use crate::common;

use std::collections::BTreeMap;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use baruch_server::idea_map::files::export;
use baruch_server::idea_map::tree::ImportTree;
use serde_json::{Value, json};
use tower::ServiceExt;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

async fn bytes_of(server: &common::Server, uri: &str) -> Vec<u8> {
    let response = server.app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{uri} antwortet");
    to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()
}

/// Vorschau wie das Formular (multipart, Feld `file`); liefert das HTML der Vorschau.
async fn preview(server: &common::Server, work: i64, bytes: &[u8]) -> String {
    let boundary = "----baruch-r36-grenze";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"karte\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend(format!("\r\n--{boundary}--\r\n").into_bytes());
    let request = Request::post(format!("/werke/{work}/karte/einlesen"))
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "die Vorschau antwortet: {html}");
    html
}

fn hidden_field(html: &str) -> String {
    let start = html.find("name=\"tree\" value=\"").expect("die Vorschau trägt das verborgene Feld tree") + 19;
    let end = start + html[start..].find('"').unwrap();
    html[start..end]
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&#x2f;", "/")
        .replace("&#x2F;", "/")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn form_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// Bestätigt die Vorschau (Formular „Anlegen“ ohne Zielknoten).
async fn apply(server: &common::Server, work: i64, tree_json: &str) {
    let request = Request::post(format!("/werke/{work}/karte/einlesen/anlegen"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("tree={}&parent=", form_encode(tree_json))))
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "die Bestätigung legt an: {page}");
}

/// Ort eines Einfalls, wie `idea.list` ihn meldet.
#[derive(Debug, PartialEq, Eq)]
struct Place {
    cloud: bool,
    main_topic: bool,
    /// Titel des Elternknotens.
    parent: Option<String>,
}

/// Titel → Ort aller Einfälle des Werks.
async fn places(server: &common::Server, work: i64) -> BTreeMap<String, Place> {
    let answer = server.rpc("idea.list", json!({ "work": work })).await;
    let ideas = result(&answer)["ideas"].as_array().unwrap().clone();
    let title_of = |id: i64| {
        ideas.iter().find(|i| i["id"].as_i64() == Some(id)).map(|i| i["title"].as_str().unwrap().to_owned())
    };
    ideas
        .iter()
        .map(|idea| {
            let place = Place {
                cloud: idea["cloud"].as_bool().unwrap(),
                main_topic: idea["main_topic"].as_bool().unwrap(),
                parent: idea["parent"].as_i64().and_then(title_of),
            };
            (idea["title"].as_str().unwrap().to_owned(), place)
        })
        .collect()
}

async fn empty_work(server: &common::Server, title: &str, file: &str) -> i64 {
    let source = format!("#set document(title: \"{title}\")\n\n= Anfang\n\nNoch leer.\n");
    server.import(&source, file).await.work
}

/// Die Rundreise der eigenen `.mm` und `.xmind` erhält die Wolke: Was in der Wolke lag, liegt nach dem
/// Wiedereinlesen in ein leeres Werk wieder in der Wolke (samt Unterknoten), alles andere an seinem Platz im Baum.
#[tokio::test]
async fn the_round_trip_of_the_own_files_keeps_the_cloud() {
    let server = common::open().await;
    let source = "#set document(title: \"Deichbuch\")\n\n= Deichfuß\n\nText.\n\n= Siel\n\nText.\n";
    let work = server.import(source, "deichbuch.typ").await.work;
    let nodes = result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].clone();
    let foot = nodes.as_array().unwrap().iter().find(|n| n["title"] == "Deichfuß").unwrap()["key"].as_i64().unwrap();
    let create = async |params: Value| -> i64 {
        result(&server.rpc("idea.create", params).await)["idea"]["id"].as_i64().unwrap()
    };
    // Im Baum: ein Einfall am Kapitel, ein freies Hauptthema mit Unterknoten.
    let sod = create(json!({ "work": work, "title": "Grassoden", "key": foot })).await;
    let sheep = create(json!({ "work": work, "title": "Schafe", "cloud": false })).await;
    create(json!({ "work": work, "title": "Trittfest", "parent": sheep })).await;
    // In der Wolke: zwei Einfälle, einer mit Unterknoten.
    let gate = create(json!({ "work": work, "title": "Sturmflut 1962" })).await;
    create(json!({ "work": work, "title": "Pegelstand", "parent": gate })).await;
    create(json!({ "work": work, "title": "Reetdach" })).await;
    result(&server.rpc("idea.link", json!({ "from": gate, "to": sod, "kind": "leads_to", "note": "prüft" })).await);

    let before = places(&server, work).await;
    assert!(
        before["Sturmflut 1962"].cloud && before["Reetdach"].cloud && !before["Schafe"].cloud && !before["Grassoden"].cloud,
        "Vorbedingung: zwei Einfälle liegen in der Wolke, zwei im Baum: {before:?}"
    );

    for (ending, name) in [("mm", "FreeMind"), ("xmind", "XMind")] {
        let file = bytes_of(&server, &format!("/werke/{work}/karte.{ending}")).await;
        let target = empty_work(&server, &format!("Leeres Werk {name}"), &format!("leer-{ending}.typ")).await;
        let html = preview(&server, target, &file).await;
        let field = hidden_field(&html);
        let tree: ImportTree = serde_json::from_str(&field).expect("das verborgene Feld ist das JSON des Baums");
        let flagged: Vec<&str> = tree.nodes.iter().filter(|n| n.cloud).map(|n| n.title.as_str()).collect();
        assert_eq!(
            flagged,
            ["Sturmflut 1962", "Reetdach"],
            "{name}: Die Vorschau merkt sich genau die Einfälle der Wolke (der Ast „Wolke“ selbst ist aufgelöst)"
        );
        assert!(
            !tree.nodes.iter().any(|n| n.title == export::CLOUD_TITLE),
            "{name}: Der Ast „Wolke“ wird nie selbst ein Einfall"
        );
        assert_eq!(
            html.matches("mmg-cloud-mark").count(),
            2,
            "{name}: Die Vorschau zeigt ohne JavaScript, welche Knoten wieder in die Wolke kommen"
        );
        apply(&server, target, &field).await;

        let after = places(&server, target).await;
        let in_cloud = || Place { cloud: true, main_topic: false, parent: None };
        assert_eq!(after["Sturmflut 1962"], in_cloud(), "{name}: wieder in der Wolke: {after:?}");
        assert_eq!(after["Reetdach"], in_cloud(), "{name}: wieder in der Wolke: {after:?}");
        assert_eq!(
            after["Pegelstand"],
            Place { cloud: false, main_topic: false, parent: Some("Sturmflut 1962".to_owned()) },
            "{name}: Der Unterknoten bleibt an seinem Einfall in der Wolke"
        );
        assert_eq!(
            after["Schafe"],
            Place { cloud: false, main_topic: true, parent: None },
            "{name}: Das freie Hauptthema bleibt Hauptthema am Werk"
        );
        assert_eq!(
            after["Trittfest"].parent.as_deref(),
            Some("Schafe"),
            "{name}: Sein Unterknoten bleibt an seinem Platz"
        );
        let chapter = after["Grassoden"].parent.clone().unwrap_or_default();
        assert!(
            chapter.ends_with("Deichfuß"),
            "{name}: Der Einfall am Kapitel hängt wieder unter dem Kapitel-Ast (im fremden Werk ein Einfall mit der \
             Nummer im Titel): {chapter}"
        );
        assert_eq!(
            after[&chapter],
            Place { cloud: false, main_topic: true, parent: None },
            "{name}: Der Kapitel-Ast selbst ist ein Hauptthema am Werk"
        );
        assert_eq!(
            after.values().filter(|p| p.cloud).count(),
            2,
            "{name}: Sonst liegt nichts in der Wolke – auch nicht die Kapitel-Äste: {after:?}"
        );
        assert!(
            !after.contains_key(export::CLOUD_TITLE) && !after.contains_key("Deichbuch"),
            "{name}: Werk-Wurzel und Ast „Wolke“ sind keine Einfälle geworden: {after:?}"
        );
        // Die Beziehung über die Wolke hinweg kommt mit.
        let map = server.rpc("idea.tree", json!({ "work": target })).await;
        let links = result(&map).to_string();
        assert!(links.contains("prüft"), "{name}: Die Verbindung zwischen Wolke und Baum bleibt erhalten: {links}");
    }
}

/// Ein fremder Ast, der nur „Wolke“ heißt, aber die eigene Kennung nicht trägt, ist ein gewöhnliches Hauptthema.
#[tokio::test]
async fn a_foreign_branch_that_is_only_called_wolke_becomes_a_main_topic() {
    let server = common::open().await;
    let work = empty_work(&server, "Wetterbuch", "wetterbuch.typ").await;
    let file = "<map version=\"1.0.1\"><node ID=\"ID_1\" TEXT=\"Wetterkarte\">\
                <node ID=\"ID_2\" TEXT=\"Wolke\"><node ID=\"ID_3\" TEXT=\"Nieselregen\"/></node>\
                <node ID=\"ID_4\" TEXT=\"Wind\"/></node></map>";
    let html = preview(&server, work, file.as_bytes()).await;
    assert_eq!(html.matches("mmg-cloud-mark").count(), 0, "die Vorschau kennzeichnet nichts als Wolke");
    let field = hidden_field(&html);
    let tree: ImportTree = serde_json::from_str(&field).unwrap();
    assert!(tree.nodes.iter().all(|n| !n.cloud), "ohne die Kennung der eigenen Ausgabe ist kein Knoten Wolke");
    apply(&server, work, &field).await;
    let after = places(&server, work).await;
    assert_eq!(
        after["Wolke"],
        Place { cloud: false, main_topic: true, parent: None },
        "der fremde Ast „Wolke“ wird Hauptthema am Werk"
    );
    assert_eq!(after["Nieselregen"].parent.as_deref(), Some("Wolke"), "sein Kind bleibt sein Unterknoten");
    assert!(after.values().all(|p| !p.cloud), "nichts landet in der Wolke: {after:?}");

    // Gegenprobe: Mit der eigenen Kennung ist derselbe Ast die Wolke.
    let own = file.replace("ID=\"ID_2\"", &format!("ID=\"ID_{}\"", export::CLOUD_ID.replace('-', "")));
    let other = empty_work(&server, "Zweites Wetterbuch", "wetterbuch2.typ").await;
    let field = hidden_field(&preview(&server, other, own.as_bytes()).await);
    apply(&server, other, &field).await;
    let after = places(&server, other).await;
    assert!(
        after["Nieselregen"].cloud && !after.contains_key("Wolke"),
        "Gegenprobe: Mit der Kennung der eigenen Ausgabe liegt das Kind in der Wolke: {after:?}"
    );
}

/// Ein gefälschtes Kennzeichen `cloud` an einem Unterknoten oder beim Einlesen unter einen Zielknoten wirkt nicht:
/// In der Wolke liegen nur oberste Knoten ohne Zielknoten.
#[tokio::test]
async fn the_cloud_flag_counts_only_at_the_top_and_without_a_target() {
    let server = common::open().await;
    let work = empty_work(&server, "Marschbuch", "marschbuch.typ").await;
    let host = result(&server.rpc("idea.create", json!({ "work": work, "title": "Warft", "cloud": false })).await)["idea"]
        ["id"]
        .as_i64()
        .unwrap();
    let forged = json!({
        "title": "Gefälscht", "links": [], "warnings": [],
        "nodes": [{ "title": "Priel", "note": "", "words": [], "cloud": true,
                    "children": [{ "title": "Schlick", "note": "", "words": [], "cloud": true, "children": [] }] }],
    })
    .to_string();
    let request = Request::post(format!("/werke/{work}/karte/einlesen/anlegen"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("tree={}&parent={host}", form_encode(&forged))))
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Einlesen unter den Zielknoten gelingt: {page}");
    let after = places(&server, work).await;
    assert_eq!(after["Priel"].parent.as_deref(), Some("Warft"), "unter einem Zielknoten gibt es keine Wolke");
    assert!(!after["Priel"].cloud && !after["Schlick"].cloud, "das Kennzeichen wirkt dort nicht: {after:?}");

    apply(&server, work, &forged.replace("Priel", "Hallig").replace("Schlick", "Queller")).await;
    let after = places(&server, work).await;
    assert!(after["Hallig"].cloud, "ohne Zielknoten kommt der oberste Knoten in die Wolke");
    assert_eq!(
        (after["Queller"].cloud, after["Queller"].parent.as_deref()),
        (false, Some("Hallig")),
        "ein Unterknoten bleibt Unterknoten, auch mit Kennzeichen"
    );
}

// ---- Beschriftungen der Verbindungen ------------------------------------------------------------------------

/// Wert des Attributs `name` im Tag (Zahl).
fn number(tag: &str, name: &str) -> f64 {
    let start = tag.find(&format!(" {name}=\"")).unwrap_or_else(|| panic!("Attribut {name} in {tag}")) + name.len() + 3;
    let end = start + tag[start..].find('"').unwrap();
    tag[start..end].parse().unwrap_or_else(|_| panic!("{name} ist eine Zahl: {tag}"))
}

/// Alle Rechtecke `<rect class="{class}" …>` des SVG als (x, y, Breite, Höhe).
fn rects(svg: &str, class: &str) -> Vec<(f64, f64, f64, f64)> {
    let open = format!("<rect class=\"{class}\"");
    svg.match_indices(&open)
        .map(|(at, _)| {
            let tag = &svg[at..at + svg[at..].find('>').unwrap()];
            (number(tag, "x"), number(tag, "y"), number(tag, "width"), number(tag, "height"))
        })
        .collect()
}

fn overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

/// Dichte, frei erfundene Karte: vier Kapitel mit je vier Einfällen und Untereinfällen, eine volle Wolke und
/// vierzehn Beziehungen kreuz und quer, die Hälfte mit langer Notiz.
async fn dense_map(server: &common::Server) -> (i64, usize) {
    let source = "#set document(title: \"Tidenbuch\")\n\n= Ebbe\n\nText.\n\n= Flut\n\nText.\n\n= Priel\n\nText.\n\n\
                  = Watt\n\nText.\n";
    let work = server.import(source, "tidenbuch.typ").await.work;
    let nodes = result(&server.rpc("work.get", json!({ "work": work })).await)["work"]["nodes"].clone();
    let mut ideas: Vec<i64> = Vec::new();
    for chapter in ["Ebbe", "Flut", "Priel", "Watt"] {
        let key = nodes.as_array().unwrap().iter().find(|n| n["title"] == chapter).unwrap()["key"].as_i64().unwrap();
        for n in 1..=4 {
            let title = format!("{chapter}-Einfall {n} mit etwas längerem Titel");
            let answer = server.rpc("idea.create", json!({ "work": work, "title": title, "key": key })).await;
            let id = result(&answer)["idea"]["id"].as_i64().unwrap();
            ideas.push(id);
            for m in 1..=2 {
                let title = format!("{chapter} {n}.{m} Unterpunkt");
                let answer = server.rpc("idea.create", json!({ "work": work, "title": title, "parent": id })).await;
                ideas.push(result(&answer)["idea"]["id"].as_i64().unwrap());
            }
        }
    }
    for n in 1..=5 {
        let answer = server.rpc("idea.create", json!({ "work": work, "title": format!("Strandgut {n}") })).await;
        ideas.push(result(&answer)["idea"]["id"].as_i64().unwrap());
    }
    let kinds = ["contradicts", "belongs_to", "leads_to", "example_of"];
    let mut links = 0usize;
    for n in 0..14usize {
        let (from, to) = (ideas[(n * 7) % ideas.len()], ideas[(n * 11 + 5) % ideas.len()]);
        if from == to {
            continue;
        }
        let note = if n % 2 == 0 { "eine recht lange Anmerkung zur Verbindung dieser beiden" } else { "" };
        let params = json!({ "from": from, "to": to, "kind": kinds[n % 4], "note": note });
        let answer = server.rpc("idea.link", params).await;
        if answer["error"].is_null() {
            links += 1;
        }
    }
    (work, links)
}

/// Prüfung R34: Beschriftungen lagen bei dichten Ästen auf den Knoten der Nachbaräste. Jetzt überschneidet das
/// Rechteck keiner Beschriftung einen Knoten oder eine andere Beschriftung, und jede trägt einen deckenden Grund.
#[tokio::test]
async fn link_labels_never_lie_on_a_node_or_on_each_other_in_a_dense_map() {
    let server = common::open().await;
    let (work, links) = dense_map(&server).await;
    assert!(links >= 10, "Vorbedingung: die Karte hat viele Beziehungen ({links})");
    let svg = String::from_utf8(bytes_of(&server, &format!("/werke/{work}/karte.svg")).await).unwrap();
    let boxes = rects(&svg, "mm-box");
    let labels = rects(&svg, "mm-link-label-bg");
    assert!(boxes.len() >= 50, "Vorbedingung: die Karte ist dicht ({} Knoten)", boxes.len());
    assert_eq!(labels.len(), links, "jede gezeichnete Beziehung hat genau ein Rechteck für ihre Beschriftung");
    assert_eq!(svg.matches("<text class=\"mm-link-label").count(), links, "und genau einen Text");
    assert!(!svg.contains("data-crowded"), "für jede Beschriftung fand sich ein freier Platz");
    for (i, label) in labels.iter().enumerate() {
        for node in &boxes {
            assert!(!overlap(*label, *node), "Beschriftung {i} {label:?} liegt auf dem Knoten {node:?}");
        }
        for other in &labels[..i] {
            assert!(!overlap(*label, *other), "Beschriftung {i} {label:?} liegt auf der Beschriftung {other:?}");
        }
        assert!(label.2 > 20.0 && label.3 >= 14.0, "das Rechteck hat die Größe des Texts: {label:?}");
    }
    let head = &svg[..svg.find('>').unwrap()];
    let (width, height) = (number(head, "width"), number(head, "height"));
    for label in &labels {
        assert!(
            label.0 >= 0.0 && label.1 >= 0.0 && label.0 + label.2 <= width && label.1 + label.3 <= height,
            "die Beschriftung bleibt im Bild ({width} × {height}): {label:?}"
        );
    }
    // Der Text steht in seinem Rechteck (Mitte waagerecht, Grundlinie innerhalb), direkt hinter dem Rechteck.
    for (at, _) in svg.match_indices("<rect class=\"mm-link-label-bg\"") {
        let end = at + svg[at..].find("/>").unwrap() + 2;
        let rect = &svg[at..end];
        assert!(rect.contains(" fill=\"#ffffff\""), "eigenständige Datei: der Grund deckt weiß (ohne Stil lesbar)");
        let rest = &svg[end..];
        assert!(rest.starts_with("<text class=\"mm-link-label"), "auf das Rechteck folgt sein Text");
        let text = &rest[..rest.find('>').unwrap()];
        let (x, y, w, h) = (number(rect, "x"), number(rect, "y"), number(rect, "width"), number(rect, "height"));
        let (tx, ty) = (number(text, "x"), number(text, "y"));
        assert!((tx - (x + w / 2.0)).abs() < 0.6, "der Text steht mittig im Rechteck");
        assert!(ty > y + h / 2.0 && ty < y + h, "die Grundlinie liegt in der unteren Hälfte des Rechtecks");
    }
    let last_node = svg.rfind("class=\"mm-box\"").unwrap();
    let first_label = svg.find("mm-link-label-bg\"").unwrap();
    assert!(last_node < first_label, "die Beschriftungen werden zuletzt gezeichnet (über Linien und Knoten)");

    // Auf der Seite färbt karte.css den Grund (hell wie dunkel): kein festes Weiß im Fragment.
    let (status, page) = server.get(&format!("/werke/{work}/karte")).await;
    assert_eq!(status, StatusCode::OK, "die Seite der Mindmap antwortet");
    let at = page.find("<rect class=\"mm-link-label-bg\"").expect("auch die Seite zeichnet den Grund");
    let rect = &page[at..at + page[at..].find("/>").unwrap()];
    assert!(!rect.contains("fill="), "im Fragment kommt die Farbe aus karte.css (var(--panel)): {rect}");
}

/// Stil der Seite: Grund der Beschriftungen in der Flächenfarbe; der Scrollbehälter der Seite reicht über die volle
/// Breite. Vorher war `.karte` auf 1400 px begrenzt und mittig – ihre Bildlaufleiste stand in breiten Fenstern
/// neben der Bühne (im dunklen Modus ein heller Streifen). Belegt ist die CSS-Ursache; ein Browser misst hier nicht.
#[test]
fn the_page_style_paints_the_label_ground_and_keeps_the_scrollbar_at_the_window_edge() {
    let css = include_str!("../static/karte.css");
    assert!(
        css.contains(".mm-svg .mm-link-label-bg { fill: var(--panel);"),
        "der Grund der Beschriftung nimmt die Flächenfarbe (hell und dunkel über dieselbe Variable)"
    );
    let start = css.find("\n.karte {").expect("Regel .karte") + 1;
    let rule = &css[start..start + css[start..].find('}').unwrap()];
    assert!(rule.contains("overflow: auto"), "Vorbedingung: .karte ist der Scrollbehälter der Seite: {rule}");
    assert!(
        rule.contains("max-width: none") && !rule.contains("max-width: 1400px"),
        "der Scrollbehälter ist nicht mehr auf 1400 px begrenzt – seine Leiste steht am Fensterrand: {rule}"
    );
    assert!(
        rule.contains("padding-inline: max(18px, calc((100% - 1400px) / 2 + 18px))"),
        "der Inhalt bleibt höchstens 1400 px breit und mittig (über den Innenabstand): {rule}"
    );
    assert!(rule.contains("scrollbar-color: var(--line) var(--bg)"), "die Leiste nimmt die Farben der Seite: {rule}");
    let canvas = &css[css.find("\n.mm-canvas {").expect("Regel .mm-canvas")..];
    let canvas = &canvas[..canvas.find('}').unwrap()];
    assert!(
        canvas.contains("scrollbar-color: var(--line) var(--panel)"),
        "auch die Leiste der Bildfläche (ohne JavaScript) nimmt die Farben der Fläche: {canvas}"
    );
}
