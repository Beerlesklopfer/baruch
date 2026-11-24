// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 25 „karte-bild“: Seite `/werke/{id}/karte` ohne JavaScript, Ausgaben (SVG, PDF, Markdown, FreeMind),
//! Formulare und DOM-Vertrag der Gedankenkarte (Vertrag §4.3/§4.4). Tests schreibt karte-bild.
//!
//! Das Kartenbuch und alle Einfälle sind frei erfunden. Kein Modell, kein Netz (keine Einbettungen: Nähe wird hier
//! nur als Anzeige geprüft). Was die Daten von karte-daten braucht (Verbindungen, Lagen, wirksame Filter), prüfen
//! die Tests erst, wenn `idea.links` antwortet; vorher melden sie ehrlich „ÜBERSPRUNGEN (R25a fehlt)“ – nach dem
//! Zusammenführen laufen sie voll.

use crate::common;

use std::collections::BTreeSet;
use std::process::Command;
use std::time::Instant;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use baruch_server::idea_map::MapFilter;
use baruch_server::idea_map::bild::layout;
use baruch_store::{IdeaOrigin, NewIdea};
use serde_json::{Value, json};
use tower::ServiceExt;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Erfundenes Kartenbuch: ein Teil mit drei Kapiteln (Rosen mit Abschnitt und Unterabschnitt, Dornen, Wege).
fn book() -> String {
    format!(
        "#set document(title: \"Kartenbuch\")\n\n{}\n\nVorrede.\n\n= Rosen\n\nText.\n\n== Schnitt\n\nText.\n\n\
         === Tiefe\n\nText.\n\n= Dornen\n\nText.\n\n= Wege\n\nLeer.\n",
        baruch_typst::heading_line(0, "Erster Teil", None)
    )
}

struct Book {
    server: common::Server,
    work: i64,
    slug: String,
    /// Titel → key.
    keys: Vec<(String, i64)>,
}

impl Book {
    fn key(&self, title: &str) -> i64 {
        self.keys.iter().find(|(t, _)| t == title).unwrap_or_else(|| panic!("kein Punkt {title}: {:?}", self.keys)).1
    }

    async fn idea(&self, key: Option<i64>, words: &str, note: &str) -> i64 {
        let mut params = json!({ "work": self.work, "words": words, "note": note });
        if let Some(key) = key {
            params["key"] = json!(key);
        }
        result(&self.server.rpc("idea.create", params).await)["idea"]["id"].as_i64().unwrap()
    }

    async fn page(&self, query: &str) -> (StatusCode, String) {
        let uri = if query.is_empty() {
            format!("/werke/{}/karte", self.work)
        } else {
            format!("/werke/{}/karte?{query}", self.work)
        };
        self.server.get(&uri).await
    }

    async fn act(&self, form: &str) -> (StatusCode, Option<String>) {
        self.server.post_form(&format!("/werke/{}/karte/act", self.work), form).await
    }

    async fn act_page(&self, form: &str) -> (StatusCode, String) {
        let request = Request::post(format!("/werke/{}/karte/act", self.work))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_owned()))
            .unwrap();
        let (status, _, body) = self.server.send(request).await;
        (status, body)
    }

    /// Commits im Spiegel des Werks (`None` ohne git).
    fn commits(&self) -> Option<usize> {
        common::git(&self.server.mirror_dir_of(&self.slug), &["rev-list", "--count", "HEAD"])
            .map(|n| n.trim().parse().unwrap())
    }

    /// Antwortet die RPC der Verbindungen schon (karte-daten zusammengeführt)?
    async fn daten_ready(&self) -> bool {
        let answer = self.server.rpc("idea.links", json!({ "work": self.work })).await;
        answer["error"].is_null()
    }
}

async fn open_book() -> Book {
    let server = common::open().await;
    let report = server.import(&book(), "kartenbuch.typ").await;
    let answer = server.rpc("work.get", json!({ "work": report.work })).await;
    let keys = result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["title"].as_str().unwrap().to_owned(), n["key"].as_i64().unwrap()))
        .collect();
    Book { server, work: report.work, slug: report.slug, keys }
}

/// Wert eines Attributs im ersten Tag, das `marker` enthält.
fn attr(html: &str, marker: &str, name: &str) -> String {
    let at = html.find(marker).unwrap_or_else(|| panic!("{marker} fehlt: {html}"));
    let start = if html[at..].starts_with('<') { at } else { html[..at].rfind('<').unwrap() };
    let end = at + html[at..].find('>').unwrap();
    let tag = &html[start..end];
    let key = format!(" {name}=\"");
    let from = tag.find(&key).unwrap_or_else(|| panic!("{name} fehlt in {tag}")) + key.len();
    tag[from..from + tag[from..].find('"').unwrap()].to_owned()
}

/// Ids der Einfälle (`data-idea`) im HTML bzw. SVG.
fn shown_ideas(html: &str) -> BTreeSet<i64> {
    html.split("data-idea=\"").skip(1).map(|r| r[..r.find('"').unwrap()].parse().unwrap()).collect()
}

/// Ids der Einfälle in der Markdown-Ausgabe (`- Einfall n:`).
fn md_ideas(md: &str) -> BTreeSet<i64> {
    md.lines().filter_map(|l| l.strip_prefix("- Einfall ")?.split(':').next()?.parse().ok()).collect()
}

/// `xmllint --noout` auf eine Datei; `None`, wenn es kein xmllint gibt.
fn xmllint(text: &str, name: &str) -> Option<(bool, String)> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    let output = Command::new("xmllint").arg("--noout").arg("--nonet").arg(&path).output().ok()?;
    Some((output.status.success(), String::from_utf8_lossy(&output.stderr).into_owned()))
}

#[tokio::test]
async fn the_map_page_follows_the_dom_contract() {
    let b = open_book().await;
    let (w, rosen, schnitt, tiefe, dornen) =
        (b.work, b.key("Rosen"), b.key("Schnitt"), b.key("Tiefe"), b.key("Dornen"));
    let part = b.key("Erster Teil");
    let a = b.idea(Some(rosen), "Laube, Rose", "Unter der Laube blüht es.").await;
    let s = b.idea(Some(schnitt), "Schere", "Im März schneiden.").await;
    let t = b.idea(Some(tiefe), "Wurzel", "").await;
    let d = b.idea(Some(dornen), "Hecke", "").await;
    let n = b.idea(None, "Ohne Ort", "").await;
    result(&b.server.rpc("idea.update", json!({ "id": d, "status": "done" })).await);

    let (status, html) = b.page("").await;
    assert_eq!(status, StatusCode::OK, "die Karte lädt: {html}");
    assert_eq!(html.matches("<header class=\"band\"").count(), 1, "genau eine Kopfzeile (base.html)"); // Runde 33: der Kopf ist das Menüband (header.band statt header.topbar)
    assert!(!html.contains("data-widget=\"idea-capture\""), "fragments/idea_capture.html ist nicht eingebunden");
    for piece in [
        "<main class=\"karte\">".to_owned(),
        format!("<form class=\"im-filter\" method=\"get\" action=\"/werke/{w}/karte\""),
        format!("<div class=\"im-stage\" data-widget=\"idea-map\" data-work=\"{w}\" data-near=\"1\">"),
        "<div class=\"im-toolbar\" data-im-toolbar>".to_owned(),
        "<svg class=\"im-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"".to_owned(),
        "role=\"group\" aria-label=\"Gedankenkarte „Kartenbuch“\" data-im-svg>".to_owned(),
        "<g class=\"im-viewport\" data-im-viewport>".to_owned(),
        "<g class=\"im-edges\">".to_owned(),
        "<g class=\"im-nodes\">".to_owned(),
        "<aside class=\"im-side\" data-im-side hidden aria-live=\"polite\"></aside>".to_owned(),
        "<section class=\"im-outline\" aria-label=\"Gliederungsliste\">".to_owned(),
        "<section class=\"im-contradictions\" aria-label=\"Widersprüche\">".to_owned(),
    ] {
        assert!(html.contains(&piece), "DOM-Vertrag §4.4: {piece} fehlt: {html}");
    }
    // Reihenfolge: Filter, Bühne (Leiste, SVG, Seitenleiste), Gliederung, Widersprüche.
    let order =
        ["class=\"im-filter\"", "data-im-toolbar", "data-im-svg", "data-im-side", "im-outline", "im-contradictions"];
    let places: Vec<usize> = order.iter().map(|m| html.find(m).unwrap()).collect();
    assert!(places.windows(2).all(|p| p[0] < p[1]), "Reihenfolge nach dem DOM-Vertrag: {order:?} → {places:?}");

    // Knoten.
    assert!(
        html.contains("<g class=\"im-node im-work\" data-node=\"w\" data-open-ideas=\"4\" data-x=\"0\" data-y=\"0\""),
        "Werk in der Mitte bei (0,0), vier offene Einfälle (einer verarbeitet): {html}"
    );
    assert!(
        html.contains(&format!(
            "<a class=\"im-node im-chapter\" data-node=\"n:{rosen}\" data-key=\"{rosen}\" data-open-ideas=\"3\" href=\"/?work={w}&amp;key={rosen}\""
        )),
        "Kapitel als Link in Key-Form; Abschnitte zusammengefasst: drei offene Einfälle an „Rosen“: {html}"
    );
    assert!(
        html.contains(&format!("data-node=\"n:{rosen}\""))
            && html[html.find(&format!("data-node=\"n:{rosen}\"")).unwrap()..]
                .contains("<tspan class=\"im-count\" x=\"0\" y=\"5\">3</tspan>"),
        "offene Einfälle als Zahl im Kreis"
    );
    let wege = format!("<a class=\"im-node im-chapter im-empty\" data-node=\"n:{}\"", b.key("Wege"));
    assert!(html.contains(&wege), "Kapitel ohne Einfälle als im-empty");
    // Hervorgehoben auch ohne Farbe: das Wort „leer“ steht im Kreis (Plan „Zeigen, was fehlt“).
    assert!(
        html[html.find(&wege).unwrap()..].split("</a>").next().unwrap().contains(">leer</tspan>"),
        "„leer“ im Kreis des leeren Kapitels"
    );
    // Der Teil zählt die Einfälle seiner Kapitel (drei offene an „Rosen“, einer an „Dornen“ …) und ist darum nicht
    // leer, obwohl kein Einfall direkt an ihm hängt (Befund Runde 25: vorher blass und ohne Zahl).
    let part_tag = format!("<a class=\"im-node im-part\" data-node=\"n:{part}\"");
    assert!(html.contains(&part_tag), "Teil als Knoten, nicht im-empty: {html}");
    let part_svg = html[html.find(&part_tag).unwrap()..].split("</a>").next().unwrap().to_owned();
    assert!(!part_svg.contains(">leer</tspan>") && !part_svg.contains("y=\"5\"></tspan>"), "Teil mit Zahl: {part_svg}");
    assert!(
        html.contains("Keine eigenen Einfälle; sie hängen an den Punkten darunter."),
        "Gliederungsliste: der Teil meldet nicht „Keine Einfälle.“"
    );
    assert!(
        html.contains("<g class=\"im-node im-none\" data-node=\"none\""),
        "Wolke „Ohne Kapitel“ für den Einfall ohne Bezug"
    );
    for (id, key, anchor) in [(a, rosen, rosen), (s, schnitt, rosen), (t, tiefe, rosen)] {
        assert!(
            html.contains(&format!(
                "<a class=\"im-node im-idea im-open\" data-node=\"i:{id}\" data-idea=\"{id}\" data-key=\"{key}\" data-anchor=\"n:{anchor}\" href=\"/einfaelle/{id}\" data-x=\""
            )),
            "Einfall {id}: genauer Bezug in data-key, Anker das Kapitel (Abschnitte zusammengefasst): {html}"
        );
    }
    assert!(html.contains(&format!("class=\"im-node im-idea im-done\" data-node=\"i:{d}\"")), "verarbeitet → im-done");
    assert!(
        html.contains(&format!("data-node=\"i:{n}\" data-idea=\"{n}\" data-key=\"\" data-anchor=\"none\"")),
        "ohne Bezug: data-key leer, Anker none"
    );
    assert!(
        html.contains(&format!("aria-label=\"Einfall {a}: Laube, Rose (offen)\"><title>Einfall {a}: Laube, Rose – Unter der Laube blüht es. (offen)</title>")),
        "Einfall mit aria-label und <title> nach Vertrag"
    );

    // Kanten von Mittelpunkt zu Mittelpunkt.
    let tree = format!("<line class=\"im-edge im-tree\" data-from=\"n:{rosen}\" data-to=\"n:{part}\"");
    assert!(html.contains(&tree), "Baumkante Kapitel → Teil: {html}");
    assert!(
        html.contains(&format!("<line class=\"im-edge im-tree\" data-from=\"n:{part}\" data-to=\"w\"")),
        "Teil → Werk"
    );
    let edge = format!("<line class=\"im-edge im-ref\" data-from=\"i:{s}\" data-to=\"n:{rosen}\"");
    assert!(html.contains(&edge), "Bezugskante Einfall → Kapitel: {html}");
    let node = format!("data-node=\"i:{s}\"");
    let chapter = format!("data-node=\"n:{rosen}\"");
    assert_eq!(attr(&html, &edge, "x1"), attr(&html, &node, "data-x"), "Kante beginnt im Mittelpunkt des Einfalls");
    assert_eq!(attr(&html, &edge, "y1"), attr(&html, &node, "data-y"), "… auch in y");
    assert_eq!(attr(&html, &edge, "x2"), attr(&html, &chapter, "data-x"), "und endet im Mittelpunkt des Kapitels");
    assert_eq!(attr(&html, &edge, "y2"), attr(&html, &chapter, "data-y"), "… auch in y");
    assert_eq!(
        attr(&html, &node, "transform"),
        format!("translate({} {})", attr(&html, &node, "data-x"), attr(&html, &node, "data-y")),
        "der Knoten steht per translate an data-x/data-y"
    );
    assert_eq!(attr(&html, &node, "data-r"), "13", "Radius in data-r");
    assert!(!html.contains("im-near\""), "ohne Nähe-Paare keine Nähe-Kanten");

    // Werkzeugleiste: Ausgaben, Zurücksetzen, ehrlicher Cluster-Stub.
    for output in ["svg", "pdf", "md", "mm"] {
        assert!(html.contains(&format!("href=\"/werke/{w}/karte.{output}\"")), "Ausgabe .{output} verlinkt");
    }
    assert!(
        html.contains("<input type=\"hidden\" name=\"method\" value=\"idea.map_pos_reset\">"),
        "Formular „Anordnung zurücksetzen“"
    );
    assert!(
        html.contains("<input type=\"hidden\" name=\"method\" value=\"idea.map_groups_name\">")
            && html.contains(">Gruppen benennen (Assistent)</button>")
            && !html.contains("folgt mit R19"),
        "Runde 19b: der Cluster-Knopf ist der echte Knopf (Formular an act), der Stub ist weg"
    );
    // Gliederungsliste mit den Formularen ohne JavaScript.
    assert!(html.contains(&format!("action=\"/werke/{w}/karte/act\"")), "Formulare gehen an /werke/W/karte/act");
    assert!(
        html.contains("<input type=\"hidden\" name=\"method\" value=\"idea.update\">")
            && html.contains(">Bezug setzen</button>"),
        "Formular „Bezug setzen“"
    );
    assert!(
        html.contains("<input type=\"hidden\" name=\"method\" value=\"idea.link\">")
            && html.contains("Verbinden mit Einfall"),
        "Formular „Verbinden mit …“"
    );
    for kind in ["führt weiter", "gehört zu", "Beispiel für", "widerspricht"] {
        assert!(html.contains(&format!(">{kind}</option>")), "Art {kind} wählbar");
    }
    assert!(
        html.contains(&format!("<option value=\"{a} – Laube, Rose\">")),
        "Auswahlliste der Einfälle (eine für alle Formulare)"
    );
    assert!(
        html.contains("Abschnitte aufklappen") && html.contains(&format!("open={rosen}")),
        "Kapitel mit Abschnitten lässt sich aufklappen (open)"
    );
    assert!(
        html.contains("Abschnitte: 1.1 Schnitt (1), 1.1.1 Tiefe (1)"),
        "zusammengefasste Abschnitte mit Zahl ihrer Einfälle: {html}"
    );
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Reste von Platzhaltern");
}

#[tokio::test]
async fn titles_and_words_are_escaped_everywhere() {
    let b = open_book().await;
    let title = "Garten <b>&</b> \"Laube\"";
    result(&b.server.rpc("work.update", json!({ "work": b.work, "title": title })).await);
    let id = b.idea(Some(b.key("Rosen")), "<script>alert(1)</script>, Rose&Dorn", "<img src=x onerror=1>").await;
    let (status, html) = b.page("").await;
    assert_eq!(status, StatusCode::OK, "Seite lädt: {html}");
    assert!(!html.contains("<script>alert") && !html.contains("<img src=x"), "keine Nutzertexte als Markup: {html}");
    assert!(!html.contains("<b>&</b>"), "Werktitel maskiert");
    assert!(
        html.contains("aria-label=\"Gedankenkarte „Garten &lt;b&gt;&amp;&lt;/b&gt; &quot;Laube&quot;“\""),
        "Titel im aria-label maskiert: {html}"
    );
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;, Rose&amp;Dorn"), "Schlagwörter im <title> maskiert");
    let (_, _, svg) =
        b.server.send(Request::get(format!("/werke/{}/karte.svg", b.work)).body(Body::empty()).unwrap()).await;
    assert!(!svg.contains("<script>"), "auch die SVG-Datei enthält kein eingeschleustes Element");
    if let Some((ok, err)) = xmllint(&svg, "karte.svg") {
        assert!(ok, "SVG mit maskierten Texten bleibt wohlgeformt: {err}");
    }
    let (_, _, mm) =
        b.server.send(Request::get(format!("/werke/{}/karte.mm", b.work)).body(Body::empty()).unwrap()).await;
    assert!(mm.contains(&format!("TEXT=\"Einfall {id}: &lt;script&gt;")), "FreeMind maskiert Attribute: {mm}");
    let (_, _, md) =
        b.server.send(Request::get(format!("/werke/{}/karte.md", b.work)).body(Body::empty()).unwrap()).await;
    assert!(md.contains("\\<script\\>"), "Markdown maskiert Auszeichnung: {md}");
}

#[tokio::test]
async fn filters_come_from_the_address_and_reach_forms_links_and_outputs() {
    let b = open_book().await;
    let rosen = b.key("Rosen");
    let open = b.idea(Some(rosen), "Rose", "Laube im Juni.").await;
    let done = b.idea(Some(rosen), "Rose, Dorn", "Hecke.").await;
    let other = b.idea(Some(b.key("Dornen")), "Weg", "Kies.").await;
    result(&b.server.rpc("idea.update", json!({ "id": done, "status": "done" })).await);

    let query = format!("status=open&word=Rose&origin=human&q=Laube&near=0&open={rosen}");
    let (status, html) = b.page(&query).await;
    assert_eq!(status, StatusCode::OK, "gefilterte Karte lädt: {html}");
    assert!(html.contains("<option value=\"open\" selected>offen</option>"), "Status im Formular gewählt");
    assert!(html.contains("name=\"word\" value=\"Rose\""), "Schlagwort im Formular");
    assert!(html.contains("<option value=\"human\" selected>Mensch</option>"), "Herkunft gewählt");
    assert!(html.contains("name=\"q\" value=\"Laube\""), "Suche im Formular");
    assert!(html.contains("<input type=\"hidden\" name=\"near\" value=\"0\">\n    <label class=\"inline\"><input type=\"checkbox\" name=\"near\" value=\"1\"> Nähe zeigen</label>"),
        "near=0: Kästchen leer, verstecktes near=0 davor (ohne JavaScript abschaltbar): {html}");
    assert!(
        html.contains(&format!("<input type=\"hidden\" name=\"open\" value=\"{rosen}\">")),
        "aufgeklappte Kapitel bleiben im Filter"
    );
    assert!(
        html.contains(&format!(
            "<div class=\"im-stage im-hide-near\" data-widget=\"idea-map\" data-work=\"{}\" data-near=\"0\">",
            b.work
        )),
        "near=0 → data-near=0 und Klasse im-hide-near"
    );
    let canonical = format!("status=open&amp;word=Rose&amp;origin=human&amp;q=Laube&amp;near=0&amp;open={rosen}");
    for output in ["svg", "pdf", "md", "mm"] {
        assert!(
            html.contains(&format!("href=\"/werke/{}/karte.{output}?{canonical}\"", b.work)),
            "Ausgabe .{output} trägt denselben Filter: {html}"
        );
    }
    assert!(
        html.contains(&format!(
            "name=\"back\" value=\"status=open&amp;word=Rose&amp;origin=human&amp;q=Laube&amp;near=0&amp;open={rosen}\""
        )),
        "Formulare schicken den Filter als back mit (Rücksprung auf die gefilterte Karte)"
    );
    // Ausgaben zeigen dieselben Einfälle wie die Seite (dieselbe build-Karte mit demselben Filter).
    let page_ideas = shown_ideas(&html);
    for output in ["svg", "md"] {
        let (s, _, body) = b
            .server
            .send(Request::get(format!("/werke/{}/karte.{output}?{query}", b.work)).body(Body::empty()).unwrap())
            .await;
        assert_eq!(s, StatusCode::OK, ".{output} lädt");
        let ids = if output == "md" { md_ideas(&body) } else { shown_ideas(&body) };
        assert_eq!(ids, page_ideas, ".{output} zeigt genau die Einfälle der gefilterten Seite: {body}");
    }
    if b.daten_ready().await {
        assert_eq!(page_ideas, BTreeSet::from([open]), "status, word, origin, q: nur der offene Rosen-Einfall bleibt");
        assert!(!page_ideas.contains(&done) && !page_ideas.contains(&other), "Rest entfernt");
        let (_, html) = b.page("q=Kies").await;
        assert!(html.contains(&format!("im-hit\" data-node=\"i:{other}\"")), "q markiert Treffer");
        assert!(html.contains(&format!("im-dim\" data-node=\"i:{open}\"")), "… und blendet den Rest ab");
    } else {
        println!(
            "ÜBERSPRUNGEN (R25a fehlt): Wirkung der Filter auf die Auswahl; geprüft wurden Formular, Links und Gleichheit Seite/Ausgaben"
        );
    }
}

#[tokio::test]
async fn setting_the_reference_by_form_makes_one_commit_and_returns_to_the_filtered_map() {
    let b = open_book().await;
    let id = b.idea(Some(b.key("Rosen")), "Kompost", "").await;
    let before = b.commits();
    let wege = b.key("Wege");
    let (status, location) =
        b.act(&format!("method=idea.update&id={id}&key={wege}&back=status%3Dopen%26near%3D0%26ok%3Dalt")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "nach dem Formular 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{}/karte?status=open&near=0&ok=bezug", b.work).as_str()),
        "zurück auf die gefilterte Karte, altes ok verworfen"
    );
    let idea = b.server.rpc("idea.get", json!({ "id": id })).await;
    assert_eq!(result(&idea)["idea"]["key"], json!(wege), "Bezug gesetzt");
    if let (Some(before), Some(after)) = (before, b.commits()) {
        assert_eq!(after, before + 1, "genau ein Commit im Spiegel (über idea.update)");
    }
    let (_, html) = b.page("status=open&near=0&ok=bezug").await;
    assert!(html.contains("Bezug gesetzt."), "Rückmeldung auf der Karte: {html}");
    assert!(html.contains(&format!("data-anchor=\"n:{wege}\"")), "der Einfall hängt jetzt an „Wege“");
    // „ohne Kapitel“: leerer key.
    let (status, location) = b.act(&format!("method=idea.update&id={id}&key=")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{}/karte?ok=bezug", b.work).as_str())),
        "ohne back: ungefilterte Karte"
    );
    let idea = b.server.rpc("idea.get", json!({ "id": id })).await;
    assert!(result(&idea)["idea"]["key"].is_null(), "Bezug entfernt");
    // Nach „Lösen“ bietet die Karte „Wiederherstellen“ an (Seitenlogik, unabhängig von den Daten).
    let (_, html) = b.page("ok=geloest&undo=42").await;
    assert!(
        html.contains("<input type=\"hidden\" name=\"method\" value=\"idea.link_restore\">")
            && html.contains("<input type=\"hidden\" name=\"id\" value=\"42\">"),
        "Formular „Wiederherstellen“ für die eben gelöste Verbindung: {html}"
    );
    let (_, html) = b.page("undo=42").await;
    assert!(!html.contains("idea.link_restore"), "ohne ok=geloest kein Wiederherstellen-Formular");
}

#[tokio::test]
async fn form_errors_come_back_as_the_page_with_the_message() {
    let b = open_book().await;
    let id = b.idea(Some(b.key("Rosen")), "Rose", "").await;
    let foreign = result(&b.server.rpc("idea.create", json!({ "work": 1, "words": "Fremd" })).await)["idea"]["id"]
        .as_i64()
        .unwrap();

    let (status, html) = b.act_page(&format!("method=idea.update&id={foreign}&key={}", b.key("Wege"))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "Einfall eines anderen Werks → 422: {html}");
    assert!(
        html.contains("role=\"alert\">Einfall") && html.contains("gehört nicht zu diesem Werk."),
        "Meldung auf der Seite"
    );
    assert_eq!(html.matches("<header class=\"band\"").count(), 1, "Fehlerseite ist die Karte mit Kopfzeile"); // Runde 33: der Kopf ist das Menüband (header.band statt header.topbar)

    let (status, html) = b.act_page("method=idea.purge&id=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unbekannte Aktion → 400");
    assert!(html.contains("Unbekannte Aktion „idea.purge“"), "mit Meldung: {html}");

    let (status, html) = b.act_page(&format!("method=idea.link&from={id}&to=Rose&kind=leads_to")).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "kein Einfall als Ziel → 422");
    assert!(html.contains("to (Einfall): Zahl erwartet"), "verständliche Meldung: {html}");

    let (status, html) =
        b.act_page(&format!("method=idea.link&from={id}&to={id}&kind=contradicts&back=near%3D0")).await;
    assert!(
        matches!(status, StatusCode::UNPROCESSABLE_ENTITY | StatusCode::CONFLICT),
        "Selbstbezug wird abgelehnt: {status}"
    );
    assert!(html.contains("role=\"alert\""), "als Seite mit Meldung: {html}");
    assert!(html.contains("data-near=\"0\""), "die Fehlerseite zeigt die Karte mit dem Filter aus back");

    let (status, _) = b.server.post_form("/werke/999/karte/act", "method=idea.update&id=1&key=").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");
    let request = Request::post(format!("/werke/{}/karte/act", b.work))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://fremd.example")
        .body(Body::from(format!("method=idea.update&id={id}&key=")))
        .unwrap();
    let (status, _, _) = b.server.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "fremde Herkunft → 403 (guard::same_origin)");
    let (status, _) = b.server.get("/werke/999/karte").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "Karte eines unbekannten Werks → 404");
}

#[tokio::test]
async fn linking_unlinking_restoring_and_resetting_work_without_javascript() {
    let b = open_book().await;
    if !b.daten_ready().await {
        println!("ÜBERSPRUNGEN (R25a fehlt): Verbinden, Lösen, Wiederherstellen und Zurücksetzen per Formular");
        return;
    }
    let w = b.work;
    let x = b.idea(Some(b.key("Rosen")), "Sonne", "").await;
    let y = b.idea(Some(b.key("Dornen")), "Schatten", "").await;
    let before = b.commits();
    let (status, location) =
        b.act(&format!("method=idea.link&from={x}&to={y}&kind=contradicts&note=nicht+beides&back=near%3D0")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Verbinden → 303");
    assert_eq!(
        location.as_deref(),
        Some(format!("/werke/{w}/karte?near=0&ok=verbunden").as_str()),
        "zurück mit ok=verbunden"
    );
    if let (Some(before), Some(after)) = (before, b.commits()) {
        assert_eq!(after, before + 1, "eine Verbindung = ein Commit im Spiegel");
    }
    let links = b.server.rpc("idea.links", json!({ "work": w })).await;
    let link = result(&links)["links"][0]["id"].as_i64().expect("Verbindung angelegt");
    let (_, html) = b.page("").await;
    assert!(
        html.contains(&format!("<line class=\"im-edge im-link im-kind-contradicts\" data-link=\"{link}\" data-kind=\"contradicts\" data-from=\"i:{x}\" data-to=\"i:{y}\"")),
        "Verbindung als Kante mit data-link und data-kind: {html}"
    );
    assert!(
        html.contains(&format!("<title>widerspricht: Einfall {x} ↔ Einfall {y} – nicht beides</title></line>")),
        "<title> nennt die Art"
    );
    let contradictions = &html[html.find("im-contradictions").unwrap()..];
    assert!(contradictions.contains(&format!("Einfall {x}</a> (Sonne) ↔ <a href=\"/einfaelle/{y}\">Einfall {y}</a> (Schatten) <strong>widerspricht</strong> – „nicht beides“")),
        "„widerspricht“ in der Liste Widersprüche: {contradictions}");
    let (_, idea_page) = b.server.get(&format!("/einfaelle/{x}")).await;
    assert!(
        idea_page.contains(&format!(
            "↔ <a href=\"/einfaelle/{y}\">Einfall {y}</a> (Schatten) <strong>widerspricht</strong>"
        )),
        "Verbindungsliste auf /einfaelle/{{id}}: {idea_page}"
    );
    assert!(
        idea_page.contains(&format!("action=\"/werke/{w}/karte/act\"")) && idea_page.contains(">Lösen</button>"),
        "mit Formular „Lösen“"
    );

    let (status, location) = b.act(&format!("method=idea.unlink&id={link}")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{w}/karte?ok=geloest&undo={link}").as_str())),
        "Lösen → 303 mit undo"
    );
    let (_, html) = b.page("").await;
    assert!(!html.contains("im-kind-contradicts\" data-link"), "gelöste Verbindung fehlt in der Karte");
    let (status, location) = b.act(&format!("method=idea.link_restore&id={link}")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{w}/karte?ok=wiederhergestellt").as_str())),
        "Wiederherstellen → 303"
    );
    let (_, html) = b.page("").await;
    assert!(html.contains(&format!("data-link=\"{link}\"")), "Verbindung wieder da");

    result(&b.server.rpc("idea.map_pos_set", json!({ "work": w, "idea": x, "x": 2500.0, "y": -40.5 })).await);
    let (_, html) = b.page("").await;
    assert!(html.contains(&format!("im-pinned\" data-node=\"i:{x}\"")), "gespeicherte Lage → im-pinned");
    assert_eq!(attr(&html, &format!("data-node=\"i:{x}\""), "data-x"), "2500", "und steht an der gespeicherten Stelle");
    let (status, location) = b.act("method=idea.map_pos_reset&back=").await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{w}/karte?ok=zurueckgesetzt").as_str())),
        "Zurücksetzen → 303"
    );
    let (_, html) = b.page("").await;
    assert!(!html.contains("im-pinned"), "nach dem Zurücksetzen keine angeheftete Lage mehr");
}

#[tokio::test]
async fn the_outputs_are_standalone_and_well_formed() {
    let b = open_book().await;
    let a = b.idea(Some(b.key("Rosen")), "Laube", "Erster Gedanke\nzweite Zeile").await;
    let n = b.idea(None, "Kies", "").await;
    let get = |ext: &'static str| {
        let uri = format!("/werke/{}/karte.{ext}", b.work);
        let server = &b.server;
        async move { server.send(Request::get(uri).body(Body::empty()).unwrap()).await }
    };
    let (status, headers, svg) = get("svg").await;
    assert_eq!(status, StatusCode::OK, "SVG lädt");
    assert_eq!(headers[header::CONTENT_TYPE], "image/svg+xml", "eigenständige SVG-Datei");
    assert!(
        svg.starts_with(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg class=\"im-svg\" xmlns=\"http://www.w3.org/2000/svg\""
        ),
        "XML-Kopf und Namensraum: {svg}"
    );
    assert!(svg.contains("<style>") && svg.contains(".im-kind-contradicts{"), "Stil eingebettet (ohne app.css lesbar)");
    assert!(
        svg.contains(" data-im-svg=\"\"") && svg.contains(" data-im-viewport=\"\""),
        "Attribute ohne Wert als =\"\" (wohlgeformtes XML)"
    );
    match xmllint(&svg, "karte.svg") {
        Some((ok, err)) => assert!(ok, "xmllint hält die SVG für wohlgeformt: {err}"),
        None => println!("ÜBERSPRUNGEN: xmllint fehlt (SVG)"),
    }
    let (status, headers, md) = get("md").await;
    assert_eq!(status, StatusCode::OK, "Markdown lädt");
    assert_eq!(headers[header::CONTENT_TYPE], "text/markdown; charset=utf-8", "Art Markdown");
    assert!(md.starts_with("# Gedankenkarte: Kartenbuch\n"), "Titel: {md}");
    assert!(
        md.contains(
            "## I Erster Teil (1 Einfall, 1 offen)\n\nKeine eigenen Einfälle; sie hängen an den Punkten darunter."
        ) && md.contains("### 1 Rosen (1 Einfall, 1 offen)"),
        "Gliederung Teil → Kapitel als Überschriften (Zähler je Knoten samt Unterpunkten): {md}"
    );
    assert!(
        md.contains(&format!("- Einfall {a}: Laube (offen) – Erster Gedanke zweite Zeile")),
        "Einfall mit Gedanke in einer Zeile: {md}"
    );
    assert!(
        md.contains(&format!("## Ohne Kapitel (1 Einfall, 1 offen)\n\n- Einfall {n}: Kies (offen)")),
        "Wolke ohne Kapitel"
    );
    assert!(
        md.contains("Abschnitte: 1.1 Schnitt (0), 1.1.1 Tiefe (0)"),
        "zusammengefasste Abschnitte mit Nummer und Zahl: {md}"
    );
    assert!(md.contains("## Widersprüche\n\nKeine."), "Abschnitt Widersprüche auch ohne Einträge");
    let (status, headers, mm) = get("mm").await;
    assert_eq!(status, StatusCode::OK, "FreeMind lädt");
    assert_eq!(headers[header::CONTENT_TYPE], "application/x-freemind", "Art FreeMind");
    assert!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains(&format!("{}-karte.mm", b.slug)),
        "als Datei mit Namen"
    );
    assert!(mm.starts_with("<map version=\"1.0.1\">\n"), "FreeMind 1.0.1: {mm}");
    assert!(
        mm.contains(&format!("<node ID=\"ID_i{a}\" TEXT=\"Einfall {a}: Laube (offen)\">")),
        "Einfall als Knoten: {mm}"
    );
    assert!(mm.contains("<richcontent TYPE=\"NOTE\"><html><head></head><body><p>Erster Gedanke<br/>zweite Zeile</p></body></html></richcontent>"), "Gedanke als Notiz");
    match xmllint(&mm, "karte.mm") {
        Some((ok, err)) => assert!(ok, "xmllint --noout hält die FreeMind-Datei für wohlgeformt: {err}"),
        None => println!("ÜBERSPRUNGEN: xmllint fehlt (FreeMind)"),
    }
    let (status, _, _) = b.server.send(Request::get("/werke/999/karte.mm").body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404 auch für Ausgaben");
}

#[tokio::test]
async fn the_pdf_comes_from_source_to_pdf_with_the_map_kind() {
    let _serial = common::PDF_SERIAL.lock().await;
    let b = open_book().await;
    b.idea(Some(b.key("Rosen")), "Laube", "Gedanke").await;
    let response = b
        .server
        .app
        .clone()
        .oneshot(Request::get(format!("/werke/{}/karte.pdf?status=open", b.work)).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(status, StatusCode::OK, "PDF entsteht: {}", String::from_utf8_lossy(&bytes[..bytes.len().min(400)]));
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf", "Art PDF");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        format!("attachment; filename=\"{}-karte.pdf\"", b.slug).as_str(),
        "Anhang mit Namen aus dem slug"
    );
    assert!(bytes.starts_with(b"%PDF-"), "echte PDF-Bytes (über oneshot, nicht als Text)");
    // Für dieses Werk entsteht schon ein PDF → 409 mit der Meldung auf der Karte.
    let busy = baruch_server::pdf::claim(b.work).expect("Werk frei");
    let (status, html) = b.server.get(&format!("/werke/{}/karte.pdf", b.work)).await;
    drop(busy);
    assert_eq!(status, StatusCode::CONFLICT, "zweite gleichzeitige Erzeugung → 409");
    assert!(html.contains(baruch_server::pdf::BUSY), "ehrliche Meldung auf der Seite: {html}");
}

#[tokio::test]
async fn three_hundred_ideas_are_laid_out_without_overlap_in_under_200_ms() {
    let b = open_book().await;
    let keys: Vec<Option<i64>> = vec![
        Some(b.key("Erster Teil")),
        Some(b.key("Rosen")),
        Some(b.key("Schnitt")),
        Some(b.key("Tiefe")),
        Some(b.key("Dornen")),
        Some(b.key("Wege")),
        None,
    ];
    for i in 0..300usize {
        let words = vec![format!("Wort{}", i % 23), format!("Einfall{i}")];
        let new = NewIdea {
            work: Some(b.work),
            key: keys[i % keys.len()],
            words: &words,
            note: "",
            quote: "",
            source: "",
            bible: "",
            origin: IdeaOrigin::Human { name: "Autor".into() },
        };
        b.server.state.store.idea_create(&new).await.unwrap();
    }
    let map =
        baruch_server::idea_map::build(&b.server.state, b.work, &MapFilter { near: true, ..MapFilter::default() })
            .await
            .expect("Karte");
    assert_eq!(map.ideas.len(), 300, "alle 300 erfundenen Einfälle auf der Karte");
    let started = Instant::now();
    let first = layout::layout(&map);
    let millis = started.elapsed().as_secs_f64() * 1000.0;
    println!("Anordnung von 300 Einfällen: {millis:.1} ms (Debug-Bau, karte-bild)");
    assert!(millis < 200.0, "Abnahme: Anordnung < 200 ms im Debug-Bau, gemessen {millis:.1} ms");
    assert_eq!(layout::layout(&map), first, "deterministisch: zweimal gleiche Eingabe → gleiche Koordinaten");
    let free: Vec<&layout::Placed> = first.items.iter().filter(|p| !p.pinned).collect();
    let mut bad = Vec::new();
    for (i, p) in free.iter().enumerate() {
        for q in &free[i + 1..] {
            if (p.x - q.x).hypot(p.y - q.y) < p.r + q.r {
                bad.push((p.id.clone(), q.id.clone()));
            }
        }
    }
    assert!(bad.is_empty(), "keine zwei nicht angehefteten Kreise näher als r1 + r2: {:?}", &bad[..bad.len().min(5)]);
    let started = Instant::now();
    let (status, html) = b.page("").await;
    println!(
        "Seite mit 300 Einfällen: {:.1} ms, {} KiB (Debug-Bau)",
        started.elapsed().as_secs_f64() * 1000.0,
        html.len() / 1024
    );
    assert_eq!(status, StatusCode::OK, "die Seite mit 300 Einfällen lädt");
    assert_eq!(shown_ideas(&html).len(), 300, "alle 300 Einfälle im SVG");
}

#[tokio::test]
async fn menu_and_idea_pages_link_the_map() {
    let b = open_book().await;
    let w = b.work;
    let id = b.idea(Some(b.key("Rosen")), "Laube", "").await;
    let request = Request::get(format!("/einfaelle?work={w}"))
        .header(header::COOKIE, format!("baruch_work={w}"))
        .body(Body::empty())
        .unwrap();
    let (_, _, list) = b.server.send(request).await;
    assert!(
        // Runde 33 (Menüband): früher Werkzeuge ▾ → Gedankenkarte, jetzt Reiter Start, Gruppe Einfälle.
        list.contains(&format!("<a class=\"btn\" href=\"/werke/{w}/karte\" title=\"Gedankenkarte der Einfälle dieses Werks\">Gedankenkarte</a>")),
        "Start → Einfälle → Gedankenkarte mit Werk: {list}"
    );
    assert!(
        list.contains(&format!("<a class=\"btn idea-map-link\" href=\"/werke/{w}/karte\"")),
        "Link „Gedankenkarte“ auf /einfaelle?work=W"
    );
    let (_, all) = b.server.get("/einfaelle?work=all").await;
    assert!(!all.contains("idea-map-link"), "ohne gewähltes Werk kein Link auf eine Karte");
    let (_, works) = b.server.get("/werke").await;
    assert!(!works.contains(">Gedankenkarte</a>"), "ohne Werk kein Menüpunkt Gedankenkarte");
    let (_, map) = b.page("").await;
    assert!(
        map.contains(&format!(
            "href=\"/werke/{w}/karte\" title=\"Gedankenkarte der Einfälle dieses Werks\">Gedankenkarte</a>"
        )),
        "auf der Karte selbst (Werk aus der Seite)"
    );
    let (status, one) = b.server.get(&format!("/einfaelle/{id}")).await;
    assert_eq!(status, StatusCode::OK, "Einfall lädt");
    assert!(
        one.contains("<section class=\"work-form idea-links\" aria-label=\"Verbindungen\">"),
        "Abschnitt „Verbindungen“: {one}"
    );
    assert!(
        one.contains(&format!("<a class=\"idea-map-link\" href=\"/werke/{w}/karte\">")),
        "Weg zur Karte zum Verbinden"
    );
    if !b.daten_ready().await {
        assert!(one.contains("Keine Verbindungen."), "ehrlich leer, solange es keine Verbindungen gibt");
    }
}
