// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederungsschemata über RPC und die Seite `/werke/{id}/nummerierung` (Vertrag Runde 6,
//! Abschnitt 2e): echter Datenordner im Temp-Ordner (SQLite, Bibliothek, Git-Spiegel), Gartenbuch
//! aus `tests/data` (Frühling, Aussaat, Tiefe, Sommer).

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

async fn garden() -> (common::Server, i64, std::path::PathBuf) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let dir = server.mirror_dir_of(&report.slug);
    (server, report.work, dir)
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn commits(dir: &std::path::Path) -> Option<u32> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|c| c.trim().parse().unwrap())
}

async fn numbers(server: &common::Server, work: i64) -> Vec<String> {
    let loaded = server.rpc("work.get", json!({ "work": work })).await;
    result(&loaded)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["number"].as_str().unwrap().to_owned())
        .collect()
}

fn editor_head(html: &str) -> &str {
    let start = html.find("<h2 class=\"node-title\"").expect("Kopf des Editors");
    let rest = &html[start..];
    &rest[..rest.find("</h2>").unwrap()]
}

#[tokio::test]
async fn numbering_schemes_lists_all_thirteen_with_a_preview_of_the_own_outline() {
    let (server, work, _dir) = garden().await;
    let answer = server.rpc("numbering.schemes", json!({ "work": work })).await;
    let schemes = result(&answer)["schemes"].as_array().unwrap();
    let ids: Vec<&str> = schemes.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 13, "dreizehn Schemata: {ids:?}");
    assert_eq!((ids[0], ids[12]), ("template", "custom"), "Vorlage zuerst, eigenes Muster zuletzt");
    let sermon = ids.iter().position(|id| *id == "sermon").unwrap();
    assert_eq!(ids[sermon + 1], "book", "Buch direkt hinter Predigt");
    let book = schemes.iter().find(|s| s["id"] == "book").unwrap();
    assert_eq!(book["label"], "Buch", "deutscher Name");
    let preview: Vec<&str> =
        book["preview"].as_array().unwrap().iter().map(|p| p["number"].as_str().unwrap()).collect();
    assert_eq!(preview, ["Einleitung", "", "", "Schluss"], "Buch: Kapitel wie Predigt, Abschnitte ohne Nummer");
    let legal = schemes.iter().find(|s| s["id"] == "legal").unwrap();
    assert_eq!(legal["label"], "Juristisch", "deutscher Name");
    let preview: Vec<&str> =
        legal["preview"].as_array().unwrap().iter().map(|p| p["number"].as_str().unwrap()).collect();
    assert_eq!(preview, ["§ 1", "Abs. 1", "Nr. 1", "§ 2"], "Vorschau an der eigenen Gliederung");
    assert_eq!(legal["preview"][2]["title"], "Tiefe", "mit Titel");
    assert!(legal.get("error").is_none(), "keine Fehlermeldung: {legal}");
    let custom = schemes.iter().find(|s| s["id"] == "custom").unwrap();
    assert!(custom["error"].as_str().unwrap().contains("fehlt"), "ohne gespeichertes Muster keine Vorschau: {custom}");
    let current = &result(&answer)["current"];
    assert_eq!(
        (current["scheme"].as_str(), current["effective"].as_str()),
        (Some("template"), Some("1.1")),
        "{current}"
    );
}

#[tokio::test]
async fn numbering_preview_gives_numbers_and_the_export_rule_without_writing() {
    let (server, work, dir) = garden().await;
    let before = commits(&dir);
    let answer = server.rpc("numbering.preview", json!({ "work": work, "scheme": "custom", "custom": "I.;;a)" })).await;
    let numbers_shown: Vec<&str> =
        result(&answer)["numbers"].as_array().unwrap().iter().map(|p| p["number"].as_str().unwrap()).collect();
    assert_eq!(numbers_shown, ["I.", "", "a)", "II."], "Muster je Ebene, Ebene 2 leer");
    let rule = result(&answer)["rule"].as_str().unwrap();
    assert!(rule.starts_with("#set heading(numbering: (..n) =>"), "die Regel des Exports: {rule}");
    let template = server.rpc("numbering.preview", json!({ "work": work, "scheme": "template" })).await;
    assert!(result(&template)["rule"].is_null(), "aus der Werkvorlage: keine Regel");
    for (params, what) in [
        (json!({ "work": work, "scheme": "roemisch" }), "unbekanntes Schema"),
        (json!({ "work": work, "scheme": "custom", "custom": "Ohne" }), "Muster ohne Zählzeichen"),
        (json!({ "work": work }), "scheme fehlt"),
    ] {
        let bad = server.rpc("numbering.preview", params).await;
        assert_eq!(bad["error"]["code"], -32602, "{what}: ungültige Parameter: {bad}");
    }
    assert_eq!(numbers(&server, work).await, ["1", "1.1", "1.1.1", "2"], "die Vorschau ändert nichts");
    assert_eq!(commits(&dir), before, "und schreibt keinen Commit");
}

#[tokio::test]
async fn numbering_set_is_one_commit_and_tree_editor_and_export_follow() {
    let (server, work, dir) = garden().await;
    let before = commits(&dir);
    let set = server.rpc("numbering.set", json!({ "work": work, "scheme": "legal" })).await;
    assert_eq!(result(&set)["action"], "numbering", "Gliederungsantwort wie outline.set_numbering: {set}");
    assert_eq!(result(&set)["detail"], "Gliederungsschema „Juristisch“", "Betreff");
    assert!(result(&set)["commit"].is_string(), "Commit gemeldet: {set}");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "genau ein Commit");
        let log = common::git(&dir, &["log", "-1", "--format=%s%n%(trailers:only,unfold)"]).unwrap();
        assert!(log.starts_with("Gliederung: Gliederungsschema „Juristisch“"), "Betreff im Spiegel: {log}");
        assert!(log.contains("Baruch-Action: outline-numbering"), "Trailer: {log}");
    }
    assert_eq!(numbers(&server, work).await, ["§ 1", "Abs. 1", "Nr. 1", "§ 2"], "Gliederung nach Schema");

    let (_, html) = server.get(&format!("/?work={work}&node=2")).await;
    assert!(
        editor_head(&html).contains("<span class=\"num\">Nr. 1</span>Tiefe"),
        "Editor-Kopf mit Nummer nach Schema: {}",
        editor_head(&html)
    );
    assert!(html.contains("Nummern: Juristisch"), "Gliederungskopf nennt das Schema");

    let export = server.rpc("work.export", json!({ "work": work })).await;
    let typst = result(&export)["typst"].as_str().unwrap();
    let template_end = common::GARDEN.find("= Frühling").unwrap();
    let template = &common::GARDEN[..template_end];
    assert!(
        typst.starts_with(&format!(
            "{template}// Baruch: Gliederungsschema „Juristisch“\n#set heading(numbering: (..n) =>"
        )),
        "Export: Vorlage, Kommentar, Regel: {typst}"
    );
    let main = std::fs::read_to_string(dir.join("main.typ")).unwrap();
    assert!(main.contains("// Baruch: Gliederungsschema „Juristisch“\n#set heading("), "main.typ mit Regel: {main}");
    assert_eq!(std::fs::read_to_string(dir.join("vorlage.typ")).unwrap(), template, "vorlage.typ unverändert");

    let again = server.rpc("numbering.set", json!({ "work": work, "scheme": "legal" })).await;
    assert_eq!(again["error"]["code"], -32000, "unverändert abgelehnt: {again}");
    let back = server.rpc("numbering.set", json!({ "work": work, "scheme": "template" })).await;
    assert!(result(&back)["detail"].as_str().unwrap().starts_with("Nummerierung aus der Vorlage"), "{back}");
    let export = server.rpc("work.export", json!({ "work": work })).await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(result(&export)["typst"].as_str().unwrap()),
        common::GARDEN,
        "aus der Werkvorlage: Export wieder byte-gleich (bis auf die Sprungmarken)"
    );
    assert!(
        std::fs::read_to_string(dir.join("main.typ")).unwrap().starts_with(&format!("{template}#include")),
        "main.typ"
    );
}

#[tokio::test]
async fn outline_set_numbering_stays_compatible_and_sets_the_custom_scheme() {
    let (server, work, _dir) = garden().await;
    let set = server.rpc("outline.set_numbering", json!({ "work": work, "numbering": "I|1" })).await;
    assert_eq!(result(&set)["detail"], "Nummerierung „I|1“", "Betreff wie bisher");
    assert_eq!(numbers(&server, work).await, ["I", "", "", "II"], "Mustersprache: nur Kapitel römisch");
    let schemes = server.rpc("numbering.schemes", json!({ "work": work })).await;
    let current = &result(&schemes)["current"];
    assert_eq!((current["scheme"].as_str(), current["custom"].as_str()), (Some("custom"), Some("I|1")), "{current}");
    let bad = server.rpc("outline.set_numbering", json!({ "work": work, "numbering": "I.;A.|2" })).await;
    assert_eq!(bad["error"]["code"], -32602, "ungültiges Muster → -32602: {bad}");
    assert!(bad["error"]["message"].as_str().unwrap().starts_with("Nummerierung: "), "deutsche Meldung: {bad}");
    let (_, html) = server.get(&format!("/?work={work}&node=0")).await;
    // Runde 7: Das eigene Muster „I|1“ entspricht „Nur Kapitel römisch“; roh steht es nur im title.
    assert!(html.contains("Nummern: Nur Kapitel römisch</summary>"), "Kopf nennt das passende Schema: {html}");
    assert!(!html.contains("Nummern: I|1"), "das rohe Muster steht nicht mehr im Kopf");
    assert!(html.contains("Gliederungsschema: Eigenes Muster, Muster I|1)"), "das Muster steht im title");
    assert!(html.contains(&format!("href=\"/werke/{work}/nummerierung\"")), "Link „Schema wählen…“ im Kopf");
    assert!(html.contains(">Schema wählen…</a>"), "Linktext");
    assert!(html.contains("data-outline-numbering"), "das Musterformular bleibt für outline.ts");
}

#[tokio::test]
async fn the_numbering_page_shows_every_scheme_with_a_preview_without_javascript() {
    let server = common::open().await;
    let source = common::GARDEN.replace("Das Gartenbuch", "Rosen & <Dornen>");
    let report = server.import(&source, "rosen.typ").await;
    let work = report.work;
    let (status, html) = server.get(&format!("/werke/{work}/nummerierung")).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("<html lang=\"de\""), "deutsche Seite");
    assert!(html.contains("Rosen &amp; &lt;Dornen&gt;"), "Titel maskiert: {html}");
    assert!(!html.contains("<Dornen>"), "kein unmaskierter Titel");
    assert_eq!(html.matches("type=\"radio\" name=\"scheme\"").count(), 13, "dreizehn Radioknöpfe");
    assert!(html.contains("value=\"book\""), "Radioknopf für Buch");
    assert!(html.contains("Einleitung · I. · II. · Schluss, Teile „Erster Teil“"), "Beispiel für Buch");
    assert!(html.contains("value=\"template\" checked"), "das geltende Schema ist gewählt");
    assert!(html.contains("<span class=\"num\">Erstes Kapitel</span>Frühling"), "Vorschau ausgeschrieben");
    assert!(html.contains("<span class=\"num\">Einleitung</span>Frühling"), "Vorschau Predigt");
    assert!(html.contains("<span class=\"num\">Schluss</span>Sommer"), "letztes Kapitel Schluss");
    assert!(
        html.contains("<form class=\"numbering-choice\" method=\"post\" action=\"/werke/nummerierung/act\">"),
        "POST"
    );
    assert!(
        html.contains(&format!("formmethod=\"get\" formaction=\"/werke/{work}/nummerierung\">Vorschau</button>")),
        "Vorschau per GET ohne JavaScript"
    );
    assert!(html.contains(">Übernehmen</button>"), "Knopf Übernehmen");
    assert!(html.contains("name=\"custom\""), "Feld für das eigene Muster");
    assert!(html.contains("Mustersprache"), "Hinweis zur Mustersprache");
    assert!(!html.contains("{{") && !html.contains("{%") && !html.contains("}}"), "keine Platzhalterreste");

    // Vorschau eines eigenen Musters per GET (so schickt es der Knopf „Vorschau“).
    let (status, html) =
        server.get(&format!("/werke/{work}/nummerierung?scheme=custom&custom=A.%3B1.%3Ba)&work={work}")).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("value=\"custom\" checked"), "eigenes Muster gewählt");
    assert!(html.contains("value=\"A.;1.;a)\""), "das Feld behält das Muster");
    assert!(html.contains("<span class=\"num\">a)</span>Tiefe"), "Vorschau des eigenen Musters: {html}");
    assert!(html.contains("Im Export steht dann"), "die Regel des Exports wird gezeigt");
    let (_, html) = server.get(&format!("/werke/{work}/nummerierung?schema=custom&muster=Ohne")).await;
    assert!(html.contains("„Ohne“"), "ungültiges Muster: Meldung statt Vorschau: {html}");
    let (status, _) = server.get("/werke/999/nummerierung").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unbekanntes Werk → 404");

    let (_, edit) = server.get(&format!("/werke/{work}/bearbeiten")).await;
    assert!(
        edit.contains(&format!("<a class=\"btn\" href=\"/werke/{work}/nummerierung\">Gliederungsschema…</a>")),
        "Werk ändern verlinkt die Seite"
    );
    assert!(edit.contains(&format!("<a class=\"btn\" href=\"/werke/{work}/bilder\">Bilder…</a>")), "und die Bilder");
}

#[tokio::test]
async fn the_numbering_form_posts_redirects_and_reports_errors() {
    let (server, work, dir) = garden().await;
    let before = commits(&dir);
    let (status, location) =
        server.post_form("/werke/nummerierung/act", &format!("work={work}&scheme=chapter_words&custom=")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "303 nach dem Übernehmen");
    assert_eq!(location.as_deref(), Some(format!("/werke/{work}/nummerierung?ok=1").as_str()), "zurück zur Seite");
    if let (Some(before), Some(after)) = (before, commits(&dir)) {
        assert_eq!(after, before + 1, "ein Commit");
    }
    let (_, html) = server.get(location.as_deref().unwrap()).await;
    assert!(html.contains("Das Gliederungsschema ist übernommen."), "Hinweis nach dem Übernehmen");
    assert!(html.contains("value=\"chapter_words\" checked"), "jetzt gewählt");
    assert_eq!(numbers(&server, work).await, ["Erstes Kapitel", "", "", "Zweites Kapitel"], "Nummern folgen");

    let form = format!("work={work}&scheme=custom&custom=I.%3BNr.");
    let request = Request::post("/werke/nummerierung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ungültiges Muster → 422");
    assert!(html.contains("role=\"alert\"") && html.contains("Ebene 2"), "Seite mit Fehlermeldung: {html}");
    assert!(html.contains("value=\"I.;Nr.\""), "die Eingabe bleibt stehen");

    let request = Request::post("/werke/nummerierung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("work={work}&scheme=chapter_words")))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::CONFLICT, "unverändert → 409");
    assert!(html.contains("schon so eingestellt"), "{html}");

    let request = Request::post("/werke/nummerierung/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("work={work}&scheme=none")))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "fremder Origin → 403");
    assert_eq!(numbers(&server, work).await[0], "Erstes Kapitel", "nichts geändert");
}

/// Schlüssel des Punkts mit dem Titel `title`.
async fn key_of(server: &common::Server, work: i64, title: &str) -> i64 {
    let loaded = server.state.store.load_work(work).await.unwrap();
    loaded.nodes.iter().find(|n| n.title == title).unwrap_or_else(|| panic!("kein Punkt „{title}“")).key
}

#[tokio::test]
async fn the_book_scheme_switches_to_sermon_and_back_keeping_chapter_numbers() {
    // Schema „Buch“: Kapitel wie „Predigt“ (Einleitung · I. · Schluss), Teile „Erster Teil“ … . Umschalten
    // Buch → Predigt → Buch: Kapitelnummern bleiben, Teile verlieren bzw. bekommen ihre Nummer, je ein
    // Commit im Spiegel.
    let (server, work, dir) = garden().await;
    let spring = key_of(&server, work, "Frühling").await;
    let summer = key_of(&server, work, "Sommer").await;
    for (target, place, title, level) in
        [(summer, "after", "Herbst", 1), (spring, "before", "Vom Säen", 0), (summer, "before", "Vom Ernten", 0)]
    {
        let added = server
            .rpc(
                "outline.add",
                json!({ "work": work, "target_key": target, "place": place, "title": title, "level": level }),
            )
            .await;
        result(&added);
    }
    let book = ["Erster Teil", "Einleitung", "", "", "Zweiter Teil", "I.", "Schluss"];
    let sermon = ["", "Einleitung", "", "", "", "I.", "Schluss"];
    for (scheme, label, expected) in [("book", "Buch", book), ("sermon", "Predigt", sermon), ("book", "Buch", book)] {
        let before = commits(&dir);
        let set = server.rpc("numbering.set", json!({ "work": work, "scheme": scheme })).await;
        assert_eq!(result(&set)["detail"], format!("Gliederungsschema „{label}“"), "Betreff: {set}");
        assert!(result(&set)["commit"].is_string(), "{scheme}: Commit gemeldet: {set}");
        if let (Some(before), Some(after)) = (before, commits(&dir)) {
            assert_eq!(after, before + 1, "{scheme}: genau ein Commit");
            let log = common::git(&dir, &["log", "-1", "--format=%s%n%(trailers:only,unfold)"]).unwrap();
            assert!(log.starts_with(&format!("Gliederung: Gliederungsschema „{label}“")), "Spiegel: {log}");
            assert!(log.contains("Baruch-Action: outline-numbering"), "Trailer: {log}");
        }
        assert_eq!(numbers(&server, work).await, expected, "{scheme}: Nummern der Gliederung");
        let (_, html) = server.get(&format!("/?work={work}")).await;
        assert!(html.contains(&format!("Nummern: {label}")), "{scheme}: Gliederungskopf nennt das Schema");
        let schemes = server.rpc("numbering.schemes", json!({ "work": work })).await;
        assert_eq!(result(&schemes)["current"]["scheme"], scheme, "{scheme}: aktuelles Schema");
    }

    // Export trägt Kommentar und eigene Regel; der Satz zeigt „Erster Teil“ auf der Teil-Seite.
    let export = server.rpc("work.export", json!({ "work": work })).await;
    let typst = result(&export)["typst"].as_str().unwrap().to_owned();
    assert!(typst.contains("// Baruch: Gliederungsschema „Buch“\n#set heading(numbering: (..n) =>"), "{typst}");
    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    assert!(
        prepared.source.main.contains("(\"Erster Teil\", \"Zweiter Teil\")"),
        "Teil-Seiten mit den Nummern des Schemas: {}",
        prepared.source.main
    );
    let typeset = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    let errors: Vec<_> = typeset.diagnostics().into_iter().filter(|d| d.severity == "error").collect();
    assert!(errors.is_empty(), "Buch setzt fehlerfrei: {errors:?}");

    // Re-Import des Buch-Stands: wieder die Nummerierung „Buch“ (samt Teilen), Export byte-gleich.
    let copy = typst.replace("Das Gartenbuch", "Das Gartenbuch, zweiter Druck");
    let again = server.import(&copy, "gartenbuch-buch.typ").await.work;
    assert_eq!(numbers(&server, again).await, book, "Re-Import: dieselben Nummern wie Buch, Teile als Wörter");
    let schemes = server.rpc("numbering.schemes", json!({ "work": again })).await;
    assert_eq!(
        result(&schemes)["current"]["scheme"],
        "template",
        "re-importiert gilt die Werkvorlage (Regel von Buch)"
    );
    let export = server.rpc("work.export", json!({ "work": again })).await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(result(&export)["typst"].as_str().unwrap()),
        baruch_typst::xref::strip_key_labels(&copy),
        "Export → Import → Export byte-gleich"
    );
}
