// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 25 „karte-bild“: page `/werke/{id}/karte` without JavaScript, outputs (SVG, PDF, Markdown, FreeMind), forms
//! and DOM contract of the idea map (contract §4.3/§4.4). Tests are written by karte-bild.
//!
//! The map book and all ideas are freely invented. No model, no network (no embeddings: proximity is checked here only
//! as display). What needs the data of karte-daten (links, positions, effective filters) is checked by the tests only
//! once `idea.links` answers; before that they honestly report „ÜBERSPRUNGEN (R25a fehlt)“ – after the merge they run
//! in full.

use crate::common;

use std::collections::BTreeSet;
use std::process::Command;
use std::time::Instant;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use baruch_server::idea_map::MapFilter;
use baruch_server::idea_map::bild::tree_layout;
use baruch_store::{IdeaOrigin, NewIdea};
use serde_json::{Value, json};
use tower::ServiceExt;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Invented map book: one part with three chapters (Rosen with section and subsection, Dornen, Wege).
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
    /// Title → key.
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

    /// Commits in the mirror of the work (`None` without git).
    fn commits(&self) -> Option<usize> {
        common::git(&self.server.mirror_dir_of(&self.slug), &["rev-list", "--count", "HEAD"])
            .map(|n| n.trim().parse().unwrap())
    }

    /// Does the RPC of the links answer already (karte-daten merged)?
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

/// Ids of the ideas (`data-idea`) in the HTML or SVG.
fn shown_ideas(html: &str) -> BTreeSet<i64> {
    html.split("data-idea=\"").skip(1).map(|r| r[..r.find('"').unwrap()].parse().unwrap()).collect()
}

/// `xmllint --noout` on a file; `None` if there is no xmllint.
fn xmllint(text: &str, name: &str) -> Option<(bool, String)> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    let output = Command::new("xmllint").arg("--noout").arg("--nonet").arg(&path).output().ok()?;
    Some((output.status.success(), String::from_utf8_lossy(&output.stderr).into_owned()))
}

// Round 34: the test of the old DOM contract (.im-stage, circles, outline list with forms) has been dropped; the page
// is called „Mindmap“ and its DOM contract is in round34_widget.rs (page, image, ribbon).

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
        html.contains("aria-label=\"Mindmap: Garten &lt;b&gt;&amp;&lt;/b&gt; &quot;Laube&quot;\""),
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
    assert!(
        mm.contains(&format!("<node ID=\"ID_i{id}\" TEXT=\"&lt;script&gt;alert(1)&lt;/script&gt;, Rose&amp;Dorn\"")),
        "FreeMind maskiert Attribute: {mm}"
    );
    assert!(!mm.contains("<script>") && !mm.contains("<img src=x"), "kein Nutzertext als Element in der .mm-Datei");
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
    // Round 34: `open` (unfold the sections of a chapter) no longer applies in the tree of the mind map and drops out
    // of the canonical filter; folding is done through `zu=` (round34_widget).
    assert!(!html.contains("name=\"open\""), "open steht nicht mehr im Filter der Mindmap");
    assert!(
        html.contains(&format!(
            "<div class=\"mm-stage mm-hide-near\" data-widget=\"mindmap\" data-work=\"{}\"",
            b.work
        )),
        "near=0 → Klasse mm-hide-near an der Bühne"
    );
    let canonical = "status=open&amp;word=Rose&amp;origin=human&amp;q=Laube&amp;near=0";
    for output in ["svg", "pdf", "md", "mm"] {
        assert!(
            html.contains(&format!("href=\"/werke/{}/karte.{output}?{canonical}\"", b.work)),
            "Ausgabe .{output} trägt denselben Filter: {html}"
        );
    }
    assert!(
        html.contains("name=\"back\" value=\"status=open&amp;word=Rose&amp;origin=human&amp;q=Laube&amp;near=0\""),
        "Formulare schicken den Filter als back mit (Rücksprung auf die gefilterte Karte)"
    );
    // Outputs show the same ideas as the page (the same build map with the same filter).
    let page_ideas = shown_ideas(&html);
    for output in ["svg", "md"] {
        let (s, _, body) = b
            .server
            .send(Request::get(format!("/werke/{}/karte.{output}?{query}", b.work)).body(Body::empty()).unwrap())
            .await;
        assert_eq!(s, StatusCode::OK, ".{output} lädt");
        if output == "md" {
            // Round 34: Markdown is the nested list of the tree (without „Einfall n:“); checked on the thought.
            assert!(
                body.contains("Laube im Juni.") && !body.contains("Hecke.") && !body.contains("Kies."),
                ".md zeigt genau die Einfälle der gefilterten Seite: {body}"
            );
        } else {
            assert_eq!(
                shown_ideas(&body),
                page_ideas,
                ".{output} zeigt genau die Einfälle der gefilterten Seite: {body}"
            );
        }
    }
    if b.daten_ready().await {
        assert_eq!(page_ideas, BTreeSet::from([open]), "status, word, origin, q: nur der offene Rosen-Einfall bleibt");
        assert!(!page_ideas.contains(&done) && !page_ideas.contains(&other), "Rest entfernt");
        let (_, html) = b.page("q=Kies").await;
        assert!(html.contains(&format!("mm-hit\" data-node=\"i:{other}\"")), "q markiert Treffer");
        assert!(html.contains(&format!("mm-dim\" data-node=\"i:{open}\"")), "… und blendet den Rest ab");
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
    assert!(
        html.contains(&format!("data-node=\"i:{id}\" data-parent=\"n:{wege}\"")),
        "der Einfall hängt jetzt an „Wege“ (Runde 34: data-parent im Bild der Mindmap)"
    );
    // „ohne Kapitel“: empty key.
    let (status, location) = b.act(&format!("method=idea.update&id={id}&key=")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{}/karte?ok=bezug", b.work).as_str())),
        "ohne back: ungefilterte Karte"
    );
    let idea = b.server.rpc("idea.get", json!({ "id": id })).await;
    assert!(result(&idea)["idea"]["key"].is_null(), "Bezug entfernt");
    // After „Lösen“ the map offers „Wiederherstellen“ (page logic, independent of the data).
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
    assert_eq!(status, StatusCode::CONFLICT, "Einfall eines anderen Werks → 409 abgelehnt (seit Runde 34): {html}");
    assert!(
        html.contains("role=\"alert\">Einfall") && html.contains("gehört nicht zu diesem Werk."),
        "Meldung auf der Seite"
    );
    assert_eq!(html.matches("<header class=\"band\"").count(), 1, "Fehlerseite ist die Karte mit Kopfzeile"); // Round 33: the header is the ribbon (header.band, not header.topbar)

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
    assert!(
        html.contains("class=\"mm-stage mm-hide-near\""),
        "die Fehlerseite zeigt die Mindmap mit dem Filter aus back"
    );

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
        html.contains(&format!("<path class=\"mm-link mm-link-contradicts mm-contradiction\" data-link=\"{link}\" data-from=\"i:{x}\" data-to=\"i:{y}\"")),
        "Verbindung als Beziehungslinie mit data-link (Runde 34, DOM-Vertrag §6): {html}"
    );
    assert!(
        html.contains("<text class=\"mm-link-label mm-contradiction\"")
            && html.contains(">widerspricht: nicht beides</text>"),
        "die Beschriftung nennt Art und Notiz"
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
    assert!(!html.contains("mm-contradiction\" data-link"), "gelöste Verbindung fehlt in der Karte");
    let (status, location) = b.act(&format!("method=idea.link_restore&id={link}")).await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{w}/karte?ok=wiederhergestellt").as_str())),
        "Wiederherstellen → 303"
    );
    let (_, html) = b.page("").await;
    assert!(html.contains(&format!("data-link=\"{link}\"")), "Verbindung wieder da");

    result(&b.server.rpc("idea.map_pos_set", json!({ "work": w, "idea": x, "x": 2500.0, "y": -40.5 })).await);
    // Round 34: positions stay stored, the tree layout of the mind map does not use them (contract §0).
    let (_, html) = b.page("").await;
    assert!(!html.contains("im-pinned") && !html.contains("data-x="), "die Mindmap kennt keine angehefteten Lagen");
    let stored = b.server.rpc("idea.map", json!({ "work": w })).await;
    assert_eq!(result(&stored)["positions"].as_array().map(Vec::len), Some(1), "die Lage bleibt gespeichert");
    let (status, location) = b.act("method=idea.map_pos_reset&back=").await;
    assert_eq!(
        (status, location.as_deref()),
        (StatusCode::SEE_OTHER, Some(format!("/werke/{w}/karte?ok=zurueckgesetzt").as_str())),
        "Zurücksetzen → 303"
    );
    let stored = b.server.rpc("idea.map", json!({ "work": w })).await;
    assert_eq!(result(&stored)["positions"].as_array().map(Vec::len), Some(0), "nach dem Zurücksetzen keine Lage mehr");
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
    // Round 34: the same drawing as on the page (`mindmap::render`), standalone with namespace and style.
    assert!(
        svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"mm-svg\""),
        "Namensraum an der Wurzel: {svg}"
    );
    assert!(svg.contains("<style>") && svg.contains(".mm-link-label"), "Stil eingebettet (ohne app.css lesbar)");
    assert!(!svg.contains("<a ") && !svg.contains("var(--"), "keine Links, keine CSS-Variablen der Seite in der Datei");
    assert!(svg.contains("1 in der Wolke"), "die Wolke steht auch in der Datei");
    match xmllint(&svg, "karte.svg") {
        Some((ok, err)) => assert!(ok, "xmllint hält die SVG für wohlgeformt: {err}"),
        None => println!("ÜBERSPRUNGEN: xmllint fehlt (SVG)"),
    }
    let (status, headers, md) = get("md").await;
    assert_eq!(status, StatusCode::OK, "Markdown lädt");
    assert_eq!(headers[header::CONTENT_TYPE], "text/markdown; charset=utf-8", "Art Markdown");
    assert!(md.starts_with("# Mindmap: Kartenbuch\n"), "Titel: {md}");
    assert!(
        md.contains("- **I Erster Teil**\n  - **1 Rosen**\n    - Laube (offen;"),
        "Runde 34: Teil → Kapitel → Einfall als verschachtelte Liste: {md}"
    );
    assert!(md.contains("      Notiz: Erster Gedanke zweite Zeile"), "Gedanke eingerückt unter seinem Einfall: {md}");
    assert!(
        md.contains("## Wolke\n\n1 in der Wolke") && md.contains("- Kies (offen;"),
        "Wolke als eigener Abschnitt: {md}"
    );
    assert!(md.contains("## Widersprüche\n\nKeine."), "Abschnitt Widersprüche auch ohne Einträge");
    let (status, headers, mm) = get("mm").await;
    assert_eq!(status, StatusCode::OK, "FreeMind lädt");
    assert_eq!(headers[header::CONTENT_TYPE], "application/x-freemind", "Art FreeMind");
    assert!(
        headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains(&format!("{}-mindmap.mm", b.slug)),
        "als Datei mit Namen"
    );
    assert!(mm.starts_with("<map version=\"1.0.1\">\n"), "FreeMind 1.0.1: {mm}");
    assert!(mm.contains(&format!("<node ID=\"ID_i{a}\" TEXT=\"Laube\">")), "Einfall als Knoten mit seinem Titel: {mm}");
    assert!(mm.contains("<richcontent TYPE=\"NOTE\"><html><head></head><body><p>Erster Gedanke</p><p>zweite Zeile</p></body></html></richcontent>"), "Gedanke als Notiz, je Zeile ein Absatz");
    assert!(
        mm.contains("TEXT=\"Wolke\"") && mm.contains(&format!("<node ID=\"ID_i{n}\" TEXT=\"Kies\">")),
        "der Einfall ohne Platz steht im Ast „Wolke“: {mm}"
    );
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
        format!("attachment; filename=\"{}-mindmap.pdf\"", b.slug).as_str(),
        "Anhang mit Namen aus dem slug"
    );
    assert!(bytes.starts_with(b"%PDF-"), "echte PDF-Bytes (über oneshot, nicht als Text)");
    // A PDF is already being produced for this work → 409 with the message on the map.
    let busy = baruch_server::pdf::claim(b.work).expect("Werk frei");
    let (status, html) = b.server.get(&format!("/werke/{}/karte.pdf", b.work)).await;
    drop(busy);
    assert_eq!(status, StatusCode::CONFLICT, "zweite gleichzeitige Erzeugung → 409");
    assert!(
        html.contains(baruch_server::i18n::t(baruch_server::i18n::Lang::De, "export_msg.pdf.busy", &[]).as_str()),
        "ehrliche Meldung auf der Seite: {html}"
    );
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
    // Round 34: the layout is computed by the tree of the mind map (`tree_layout`), no longer by the circles of the
    // idea map.
    let scene = baruch_server::idea_map::tree::scene(
        &b.server.state,
        b.work,
        &MapFilter { near: true, ..MapFilter::default() },
    )
    .await
    .expect("Mindmap");
    let ideas = scene.map.walk().into_iter().filter(|(n, _)| n.idea.is_some()).count();
    assert_eq!(ideas, 300, "alle 300 erfundenen Einfälle in Baum und Wolke");
    let lay = || tree_layout::layout_with(&scene.map, &[], &scene.cloud, &scene.hints, Some(scene.cloud_count));
    let started = Instant::now();
    let first = lay();
    let millis = started.elapsed().as_secs_f64() * 1000.0;
    println!("Anordnung von 300 Einfällen: {millis:.1} ms (Debug-Bau)");
    assert!(millis < 200.0, "Abnahme: Anordnung < 200 ms im Debug-Bau, gemessen {millis:.1} ms");
    let boxes = |l: &tree_layout::Layout| -> Vec<(String, f64, f64, f64, f64)> {
        l.items.iter().map(|p| (p.node.id.clone(), p.x, p.y, p.w, p.h)).collect()
    };
    let placed = boxes(&first);
    assert_eq!(boxes(&lay()), placed, "deterministisch: zweimal gleiche Eingabe → gleiche Koordinaten");
    let mut bad = Vec::new();
    for (i, p) in placed.iter().enumerate() {
        for q in &placed[i + 1..] {
            if p.1 < q.1 + q.3 && q.1 < p.1 + p.3 && p.2 < q.2 + q.4 && q.2 < p.2 + p.4 {
                bad.push((p.0.clone(), q.0.clone()));
            }
        }
    }
    assert!(bad.is_empty(), "keine zwei Kästen überlappen: {:?}", &bad[..bad.len().min(5)]);
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
        // Round 33 (ribbon): tab Start, group Einfälle; Round 34: the entry is called „Mindmap“.
        list.contains(&format!("<a class=\"btn\" href=\"/werke/{w}/karte\" title=\"Mindmap der Einfälle dieses Werks (mit der Wolke als Ablage)\">Mindmap</a>")),
        "Start → Einfälle → Mindmap mit Werk: {list}"
    );
    assert!(
        list.contains(&format!("<a class=\"btn idea-map-link\" href=\"/werke/{w}/karte\"")),
        "Link „Gedankenkarte“ auf /einfaelle?work=W"
    );
    let (_, all) = b.server.get("/einfaelle?work=all").await;
    assert!(!all.contains("idea-map-link"), "ohne gewähltes Werk kein Link auf eine Karte");
    let (_, works) = b.server.get("/werke").await;
    assert!(
        !works.contains(">Mindmap</a>") && !works.contains(">Gedankenkarte</a>"),
        "ohne Werk kein Menüpunkt Mindmap"
    );
    let (_, map) = b.page("").await;
    assert!(
        map.contains(&format!("href=\"/werke/{w}/karte\" title=\"Mindmap der Einfälle dieses Werks (mit der Wolke als Ablage)\">Mindmap</a>")),
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
