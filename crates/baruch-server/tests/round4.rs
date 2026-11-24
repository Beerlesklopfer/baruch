// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ende-zu-Ende-Abnahme der Runde 4 (works · outline · fixes) nach der Zusammenführung: Werk anlegen,
//! umbenennen (doppelter Titel abgelehnt), Gliederung umstellen, Überschrift im Text ändern, Export
//! gültig (lässt sich wieder importieren), Werk löschen (Spiegel archiviert); Hochladen ohne JavaScript
//! und Gliederung über Formulare; Werkwahl per Cookie; Teilen im Editor mit Rückfrage; übernommener
//! Vorschlag mit neuer Überschrift benennt den Gliederungspunkt um; MCP ohne Werk- und
//! Gliederungswerkzeuge; Migrationen 0005/0006/0105 auf einer Datenbank im Stand davor mit zwei gleichen
//! Titeln. Echter Datenordner (SQLite, Bibliothek, Git-Spiegel) im Temp-Ordner.

use crate::common;

use std::net::SocketAddr;
use std::path::Path;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::rpc;
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "Fehler statt Ergebnis: {answer}");
    &answer["result"]
}

/// Fester Schlüssel (`key`) des Abschnitts an Stelle `seq`: Schreiben über MCP und Zitieren nehmen
/// nur ihn (Vertrag Runde 5).
async fn key_of(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

/// (Nummer, Titel) aller Gliederungspunkte eines Werks laut `work.get`, in seq-Reihenfolge.
async fn outline_of(server: &common::Server, work: i64) -> Vec<(String, String)> {
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["number"].as_str().unwrap().to_owned(), n["title"].as_str().unwrap().to_owned()))
        .collect()
}

fn titles(outline: &[(String, String)]) -> Vec<&str> {
    outline.iter().map(|(_, t)| t.as_str()).collect()
}

fn subjects(dir: &Path) -> Option<Vec<String>> {
    common::git(dir, &["log", "--format=%s"]).map(|log| log.lines().map(str::to_owned).collect())
}

/// MCP-Anfrage vom eigenen Rechner (ohne Token nur Loopback).
async fn mcp(server: &common::Server, method: &str, params: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let mut request = Request::post("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(message.to_string()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, _, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP {method} antwortet mit 200: {body}");
    serde_json::from_str(&body).unwrap()
}

#[tokio::test]
async fn a_new_work_is_renamed_restructured_exported_and_deleted_with_its_mirror_archived() {
    let server = common::open().await;
    let sample = server.state.default_work().await.unwrap();
    let sample_title = server.state.store.works().await.unwrap()[0].2.clone();

    let created = server.rpc("work.create", json!({ "title": "Das Wanderbuch", "author": "Wanda Weg" })).await;
    let work = result(&created)["work"].as_i64().unwrap();
    let dir = server.mirror_dir_of("das-wanderbuch");
    assert_eq!(
        outline_of(&server, work).await,
        [("1".to_owned(), "Erstes Kapitel".to_owned())],
        "ein neues Buch beginnt mit einem nummerierten Kapitel (Muster 1.1 aus der Vorlage)"
    );

    // Umbenennen: der Titel des Beispielwerks ist vergeben (auch in anderer Schreibung), ein neuer nicht.
    let taken = server.rpc("work.update", json!({ "work": work, "title": sample_title.to_uppercase() })).await;
    assert_eq!(
        taken["error"]["code"],
        rpc::REFUSED,
        "doppelter Titel → -32000, ohne Rücksicht auf Groß/klein: {taken}"
    );
    assert!(
        taken["error"]["message"].as_str().unwrap().starts_with("Es gibt schon ein Werk"),
        "Grund genannt: {taken}"
    );
    let renamed = server.rpc("work.update", json!({ "work": work, "title": "Wege und Pfade" })).await;
    assert_eq!(result(&renamed)["changed"], json!(["title"]), "nur der Titel ändert sich: {renamed}");
    assert!(dir.is_dir(), "der Spiegelordner bleibt beim Umbenennen (slug bleibt)");

    // Gliederung: zweites Kapitel dahinter, Abschnitt unter das erste, dann umstellen und umstufen.
    let added = server
        .rpc("outline.add", json!({ "work": work, "target": 0, "place": "after", "title": "Zweites Kapitel" }))
        .await;
    assert_eq!(result(&added)["node"], 1, "neues Kapitel hinter dem ersten: {added}");
    let into =
        server.rpc("outline.add", json!({ "work": work, "target": 0, "place": "into", "title": "Am Morgen" })).await;
    assert_eq!(result(&into)["node"], 1, "der Abschnitt steht als erster Unterpunkt direkt hinter dem Kapitel: {into}");
    let moved = server.rpc("outline.move", json!({ "work": work, "node": 2, "target": 0, "place": "before" })).await;
    assert_eq!(result(&moved)["node"], 0, "das zweite Kapitel steht jetzt vorn: {moved}");
    let level = server.rpc("outline.set_level", json!({ "work": work, "node": 2, "level": 1 })).await;
    assert_eq!(result(&level)["action"], "level", "Abschnitt wird Kapitel: {level}");
    assert_eq!(
        outline_of(&server, work).await,
        [
            ("1".to_owned(), "Zweites Kapitel".to_owned()),
            ("2".to_owned(), "Erstes Kapitel".to_owned()),
            ("3".to_owned(), "Am Morgen".to_owned()),
        ],
        "Reihenfolge und Nummern folgen der neuen Gliederung"
    );

    // Überschrift im Text ändern: der Editor speichert über outline.save, der Titel folgt.
    let saved = server
        .rpc("outline.save", json!({ "work": work, "node": 0, "body": "= Der zweite Weg\n\nÜber den Berg.\n" }))
        .await;
    assert_eq!(result(&saved)["outline"]["action"], "renamed", "geänderte Überschriftszeile benennt um: {saved}");
    // Auch der rohe Weg (revision.save) übernimmt eine Überschrift derselben Ebene als Titel (sync_heading).
    let raw =
        server.rpc("revision.save", json!({ "work": work, "node": 1, "body": "= Das erste Kapitel\n\nNeu.\n" })).await;
    result(&raw);
    let outline = outline_of(&server, work).await;
    assert_eq!(titles(&outline), ["Der zweite Weg", "Das erste Kapitel", "Am Morgen"], "die Gliederung passt zum Text");

    // Export ist gültiges Typst: wieder importiert ergibt er dieselbe Gliederung.
    let export = server.rpc("work.export", json!({ "work": work })).await;
    let typst = result(&export)["typst"].as_str().unwrap().to_owned();
    assert!(typst.starts_with("#set document(title: \"Das Wanderbuch\""), "Werkvorlage vorn: {typst}");
    assert!(
        typst.find("= Der zweite Weg").unwrap() < typst.find("= Das erste Kapitel").unwrap()
            && typst.find("= Das erste Kapitel").unwrap() < typst.find("= Am Morgen").unwrap(),
        "Export in neuer Reihenfolge: {typst}"
    );
    let again =
        server.rpc("work.import", json!({ "file_name": "wege.typ", "source": typst, "title": "Wege (Rückweg)" })).await;
    let copy = result(&again)["work"].as_i64().unwrap();
    assert_eq!(outline_of(&server, copy).await, outline, "der Export lässt sich verlustfrei wieder einlesen");
    let copy_export = server.rpc("work.export", json!({ "work": copy })).await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(result(&copy_export)["typst"].as_str().unwrap()),
        baruch_typst::xref::strip_key_labels(&typst),
        "Rundreise byte-gleich (bis auf die Sprungmarken, die neuen Schlüssel folgen dem neuen Werk)"
    );

    if let Some(log) = subjects(&dir) {
        let outline_commits = log.iter().filter(|s| s.starts_with("Gliederung: ")).count();
        assert_eq!(outline_commits, 5, "add, add, move, set_level, umbenennen im Editor → je ein Commit: {log:?}");
        assert_eq!(log.last().map(String::as_str), Some("Werk angelegt: Das Wanderbuch"), "erster Commit: {log:?}");
        assert!(dir.join("abschnitte/000-der-zweite-weg.typ").is_file(), "Dateiname folgt seq und Titel");
    }

    // Löschen: falscher Titel abgelehnt, richtiger löscht und archiviert den Spiegel.
    let wrong = server.rpc("work.delete", json!({ "work": work, "confirm_title": "Das Wanderbuch" })).await;
    assert_eq!(wrong["error"]["code"], rpc::REFUSED, "der alte Titel bestätigt nicht mehr: {wrong}");
    let deleted = server.rpc("work.delete", json!({ "work": work, "confirm_title": " wege UND pfade " })).await;
    let deleted = result(&deleted);
    assert_eq!(deleted["next"].as_i64(), Some(sample), "danach ist wieder das Beispielwerk die Vorgabe: {deleted}");
    assert!(!dir.exists(), "der Spiegelordner ist weg");
    let archive = server.path().join(deleted["archive"].as_str().unwrap());
    assert!(archive.join("main.typ").is_file(), "der Spiegel liegt im Archiv: {}", archive.display());
    if let Some(log) = subjects(&archive) {
        assert_eq!(log[0], "Werk gelöscht: Wege und Pfade", "letzter Commit protokolliert das Löschen: {log:?}");
    }
    let gone = server.rpc("work.get", json!({ "work": work })).await;
    assert_eq!(gone["error"]["code"], rpc::INVALID_PARAMS, "das Werk gibt es nicht mehr: {gone}");
    assert_eq!(titles(&outline_of(&server, copy).await).len(), 3, "die Kopie bleibt unberührt");
}

const BOUNDARY: &str = "baruch-runde4-grenze";

fn upload(file_name: &str, content: &str, title: &str) -> Request<Body> {
    let body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"title\"\r\n\r\n{title}\r\n\
         --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n{content}\r\n--{BOUNDARY}--\r\n"
    );
    Request::post("/werke/import")
        .header(header::CONTENT_TYPE, format!("multipart/form-data; boundary={BOUNDARY}"))
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn an_uploaded_book_is_restructured_with_forms_and_exports_in_the_new_order() {
    let server = common::open().await;
    let (status, headers, _) = server.send(upload("gartenbuch.typ", common::GARDEN, "")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Hochladen ohne JavaScript leitet auf das Werk");
    let work: i64 = headers[header::LOCATION].to_str().unwrap().strip_prefix("/?work=").unwrap().parse().unwrap();
    let export = server.rpc("work.export", json!({ "work": work })).await;
    assert_eq!(
        baruch_typst::xref::strip_key_labels(result(&export)["typst"].as_str().unwrap()),
        common::GARDEN,
        "ohne Änderung ist der Export byte-gleich zur Datei (bis auf die Sprungmarken)"
    );
    let (status, _, html) =
        server.send(upload("garten-kopie.typ", &common::GARDEN.replace("Gießen", "Wässern"), "")).await;
    assert_eq!(status, StatusCode::CONFLICT, "andere Datei, gleicher Titel → 409");
    assert!(html.contains("Es gibt schon ein Werk"), "Hinweis auf den vergebenen Titel: {html}");

    // ↑ am Sommer (Formular aus der linken Spalte) stellt ihn vor den Frühling.
    let (_, page) = server.get(&format!("/?work={work}&node=3")).await;
    assert!(page.contains("id=\"o-up-3\""), "linke Spalte hat das Formular ↑ für Sommer");
    let (status, location) = server
        .post_form("/gliederung/act", &format!("method=outline.move&work={work}&node=3&target=0&place=before"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet zurück");
    let summer = key_at(&server, work, 0).await;
    assert_eq!(
        location.as_deref(),
        Some(format!("/?work={work}&key={summer}").as_str()),
        "der Sommer ist jetzt Punkt 0; die Adresse nennt seinen Schlüssel"
    );
    // → an der Aussaat (Ebene 2 → 3 geht nicht unter dem Frühling ohne Abschnitt davor): abgelehnt, nichts geändert.
    let (status, _) =
        server.post_form("/gliederung/act", &format!("method=outline.set_level&work={work}&node=2&level=3")).await;
    assert_eq!(status, StatusCode::CONFLICT, "ungültige Ebene → 409");
    let outline = outline_of(&server, work).await;
    assert_eq!(titles(&outline), ["Sommer", "Frühling", "Aussaat", "Tiefe"], "nach der Ablehnung unverändert");
    assert_eq!(
        outline.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        ["1", "2", "2.1", "2.1.1"],
        "Nummern ohne Muster nach Vorgabe 1.1, Kapitel neu gezählt"
    );
    let export = server.rpc("work.export", json!({ "work": work })).await;
    // Runde 30a: ohne die Sprungmarken `<kN>` (sie stehen nur im Export).
    let typst = baruch_typst::xref::strip_key_labels(result(&export)["typst"].as_str().unwrap()).into_owned();
    assert!(typst.starts_with("// Gartenbuch"), "Vorlage bleibt vorn");
    assert!(typst.find("= Sommer").unwrap() < typst.find("= Frühling").unwrap(), "Export in neuer Reihenfolge");
    let mut lines: Vec<&str> = typst.lines().collect();
    let mut original: Vec<&str> = common::GARDEN.lines().collect();
    lines.sort_unstable();
    original.sort_unstable();
    assert_eq!(lines, original, "nur umgestellt: dieselben Zeilen, kein Text verloren oder dazu: {typst}");
}

#[tokio::test]
async fn the_cookie_picks_the_work_and_its_outline_is_editable_without_javascript() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let request = Request::get("/").header(header::COOKIE, format!("baruch_work={}", garden.work)).body(Body::empty());
    let (status, _, html) = server.send(request.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite lädt mit dem Werk aus dem Cookie: {html}");
    assert!(html.contains("<span class=\"work-title\">Das Gartenbuch</span>"), "Werk aus dem Cookie");
    assert!(html.contains("data-widget=\"work-select\""), "Werkwahl in der Kopfzeile");
    assert!(html.contains(&format!("data-widget=\"outline\" data-work=\"{}\"", garden.work)), "Gliederung des Werks");
    let k = key_at(&server, garden.work, 3).await;
    assert!(
        html.contains(&format!("href=\"/gliederung/bearbeiten?work={}&key={k}\"", garden.work)),
        "Link „Bearbeiten“ je Eintrag, mit Schlüssel"
    );
    assert!(
        html.contains("action=\"/gliederung/act\" data-rpc data-widget=\"editor\""),
        "Editor speichert über die Gliederung"
    );
    let (status, manager) = server.get(&format!("/gliederung?work={}&node=0", garden.work)).await;
    assert_eq!(status, StatusCode::OK, "Gliederungsmanager lädt");
    for title in ["Frühling", "Aussaat", "Tiefe", "Sommer"] {
        assert!(manager.contains(title), "Manager nennt {title}");
    }
}

#[tokio::test]
async fn a_new_heading_in_the_editor_splits_only_after_the_question() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let work = garden.work;
    let body = "%3D+Sommer%0A%0AGie%C3%9Fen.%0A%0A%3D%3D+Mulchen%0A%0AStroh.%0A";
    let form = format!("method=outline.save&work={work}&node=3&based_on=1&body={body}");
    let request = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.clone()))
        .unwrap();
    let (status, _, html) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "erst die Rückfrageseite, nicht speichern");
    assert!(
        html.contains("Teilen und speichern") && html.contains("<li>Mulchen</li>"),
        "Rückfrage nennt die neue Überschrift"
    );
    assert_eq!(outline_of(&server, work).await.len(), 4, "die Rückfrage speichert nichts");
    let (status, _) = server.post_form("/gliederung/act", &format!("{form}&split=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "bestätigt: gespeichert");
    let outline = outline_of(&server, work).await;
    assert_eq!(titles(&outline), ["Frühling", "Aussaat", "Tiefe", "Sommer", "Mulchen"], "geteilt");
    assert_eq!(outline[4].0, "2.1", "der neue Abschnitt zählt unter dem Sommer");
}

#[tokio::test]
async fn an_accepted_proposal_with_a_new_heading_renames_the_point_and_its_mirror_file() {
    let server = common::open().await;
    let garden = server.import_garden().await;
    let work = garden.work;
    let dir = server.mirror_dir_of(&garden.slug);
    let call = json!({
        "name": "propose_revision",
        "arguments": { "work": work, "key": key_of(&server, work, 3).await, "body": "= Hochsommer\n\nGießen am Abend.\n" }
    });
    let proposed = mcp(&server, "tools/call", call).await;
    assert_eq!(proposed["result"]["isError"], false, "Vorschlag über MCP: {proposed}");
    let rev = proposed["result"]["structuredContent"]["rev"].as_u64().unwrap();
    assert_eq!(titles(&outline_of(&server, work).await)[3], "Sommer", "ein Vorschlag ändert den Titel nicht");
    let accepted = server.rpc("revision.accept", json!({ "work": work, "node": 3, "rev": rev })).await;
    result(&accepted);
    assert_eq!(
        titles(&outline_of(&server, work).await)[3],
        "Hochsommer",
        "übernommen: der Titel folgt der Überschrift"
    );
    if common::git(&dir, &["status"]).is_some() {
        assert!(dir.join("abschnitte/003-hochsommer.typ").is_file(), "Spiegeldatei trägt den neuen Titel");
        assert!(!dir.join("abschnitte/003-sommer.typ").exists(), "die alte Datei ist umbenannt");
    }
    let back = server.rpc("revision.set_current", json!({ "work": work, "node": 3, "rev": 1 })).await;
    result(&back);
    assert_eq!(titles(&outline_of(&server, work).await)[3], "Sommer", "Zurückschalten stellt den Titel wieder her");
}

#[tokio::test]
async fn mcp_lists_no_tool_for_works_or_the_outline() {
    let server = common::open().await;
    let answer = mcp(&server, "tools/list", json!({})).await;
    let mut names: Vec<&str> =
        answer["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "add_idea",
            "add_post",
            "ask_persona",
            "asset_add",
            "asset_delete",
            "asset_replace",
            "asset_requests",
            "asset_set_cover",
            "bible_lookup",
            "bible_modules",
            "bible_search",
            "chat",
            "commentary_lookup",
            "cover_layout",
            "cover_layout_set",
            "epub_check",
            "epub_export",
            "exchange_export",
            "finding_propose_status",
            "findings",
            "glossary_list",
            "glossary_propose",
            "idea_create",
            "idea_delete",
            "idea_link",
            "idea_links",
            "idea_list",
            "idea_map",
            "idea_read",
            "idea_search",
            "idea_unlink",
            "idea_update",
            "index_propose",
            "lexicon_lookup",
            "lexicon_search",
            "list_assets",
            "list_personas",
            "list_works",
            "outline",
            "outline_add",
            "outline_delete",
            "outline_move",
            "outline_rename",
            "outline_requests",
            "outline_set_level",
            "outline_set_numbered",
            "post_delete",
            "post_edit",
            "post_move",
            "post_to_idea",
            "propose_revision",
            "rag_search",
            "rag_status",
            "read_asset",
            "read_node",
            "revision_amends",
            "scripture_check",
            "scripture_map",
            "search",
            "solution_variants",
            "source_read",
            "sources_list",
            "text_search",
            "typeset_settings",
            "typst_check",
            "work_timing"
        ],
        "Runde 4 bringt keine MCP-Werkzeuge für Werke und Gliederung (die ändert nur der Mensch); \
         Runde 6 bringt nur die lesenden Bild-Werkzeuge, Runde 7 die Einfälle (kein Buchtext), Runde 8 das \
         Verwalten eigener Beiträge (kein Buchtext) und die lesende Typst-Prüfung; Wunsch 2026-11-06: \
         outline_add legt an, die übrigen outline_* stellen nur Anträge (der Mensch bestätigt); Runde 9: \
         sources_list liest nur die bevorzugten Quellen; Runde 10: scripture_check prüft nur lesend Bibelstellen; \
         Runde 11: asset_add legt Bilder an, die übrigen asset_* stellen nur Anträge ; 2026-11-06: bible_* lesen nur SWORD-Bibeln (keine Werke, keine Gliederung); Runde 12: rag_search und rag_status lesen nur (kein rag_reindex über MCP); Runde 13: source_read liest nur den Volltext einer Quelle; Runde 16b: work_timing liest nur die Sprechdauer; Runde 17a: scripture_map rechnet nur Stellen zwischen Verszählungen um; Runde 26: exchange_export gibt das Werk nur lesend aus (kein Einlesen über MCP); Runde 25: idea_map und idea_links lesen die Gedankenkarte, idea_link und idea_unlink verbinden Einfälle (kein Buchtext); Runde 27: cover_layout liest die Umschlag-Anordnung, cover_layout_set stellt nur einen Antrag (der Mensch bestätigt); Runde 28: findings liest Befunde, finding_propose_status stellt nur einen Vorschlag, post_edit bearbeitet nur eigene Beiträge (kein Buchtext); Runde 30: text_search sucht nur lesend, solution_variants legt nur Vorschläge an; Runde 29: typeset_settings liest nur die Satz-Einstellungen (ändern nur der Mensch); Runde 20: epub_export gibt das Werk nur lesend als EPUB aus, epub_check prüft nur lesend (startet nie EPUBCheck); Runde 31: glossary_list liest nur, glossary_propose und index_propose legen nur Vorschläge an (übernehmen nur der Mensch); Runde 22: revision_amends liest nur den Verlauf der Fortschreibungen (Rückgängig nur der Mensch)"
    );
    for method in ["work.delete", "outline.move", "work_create"] {
        let call = mcp(&server, "tools/call", json!({ "name": method, "arguments": {} })).await;
        assert!(
            call["error"].is_object() || call["result"]["isError"] == true,
            "{method} ist über MCP nicht aufrufbar: {call}"
        );
    }
}

// ---- Migrationen 0005, 0006, 0105 auf einer Datenbank im Stand davor -----------------------------

fn partial(migrator: &sqlx::migrate::Migrator, last: i64) -> sqlx::migrate::Migrator {
    sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrator.migrations.iter().filter(|m| m.version <= last).cloned().collect(),
        ),
        ignore_missing: true,
        locking: true,
        no_tx: false,
    }
}

const TEMPLATE: &str = "#set document(title: \"Kostenbuch\")\n\n";
/// (Titel wie ihn der alte Import speicherte – wörtlich mit Maskierung –, Nummer, Elternteil, Stelle, Art, Text)
type Old = (&'static str, &'static str, Option<i64>, i64, &'static str, &'static str);
const HEADINGS: [Old; 3] = [
    ("Kosten \\#1", "1", None, 0, "chapter", "= Kosten \\#1\n\nRechnung.\n\n"),
    ("Posten", "1.1", Some(0), 0, "section", "== Posten\n\nSteinpilze im Korb.\n\n"),
    ("Summe", "2", None, 1, "chapter", "= Summe\n\nAlles.\n"),
];

/// Ein Buch, wie der Import vor Runde 4 es schrieb (ohne title_key, Titel mit Maskierung), mit Beitrag.
async fn insert_book(pool: &sqlx::SqlitePool, slug: &str, sha: &str) -> i64 {
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256) \
         VALUES (?, 'book', 'Kostenbuch', 'Jemand', ?, 'kosten.typ', ?) RETURNING id",
    )
    .bind(slug)
    .bind(TEMPLATE)
    .bind(sha)
    .fetch_one(pool)
    .await
    .unwrap();
    for (seq, (title, number, parent, position, kind, body)) in HEADINGS.iter().enumerate() {
        let node: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(parent)
        .bind(position)
        .bind(number)
        .bind(title)
        .bind(kind)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO revision (node_id, number, body, author_kind, author_name, status, note) \
             VALUES (?, 1, ?, 'human', 'Autor', 'current', 'Import aus kosten.typ')",
        )
        .bind(node)
        .bind(body)
        .execute(pool)
        .await
        .unwrap();
        if seq == 1 {
            sqlx::query("INSERT INTO post (node_id, position, author, body, sources) VALUES (?, 0, 'Leser', ?, '[]')")
                .bind(node)
                .bind(format!("Pfifferlinge statt Steinpilze? ({slug})"))
                .execute(pool)
                .await
                .unwrap();
        }
    }
    work
}

async fn counts(pool: &sqlx::SqlitePool) -> Vec<i64> {
    let mut out = Vec::new();
    for table in ["work", "node", "revision", "post", "post_ref"] {
        out.push(sqlx::query_scalar(&format!("SELECT count(*) FROM {table}")).fetch_one(pool).await.unwrap());
    }
    out
}

async fn post_hits(state: &baruch_server::AppState) -> usize {
    let options = baruch_library::SearchOptions { all_revisions: true, ..Default::default() };
    baruch_library::search(state.store.pool(), "Pfifferlinge", &options).await.unwrap().len()
}

#[tokio::test]
async fn the_migrations_of_round_4_keep_every_row_and_make_titles_unique_and_unescaped() {
    let data = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(data.path().join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap();
    partial(&baruch_store::MIGRATOR, 4).run(&pool).await.unwrap();
    partial(&baruch_library::MIGRATOR, 104).run(&pool).await.unwrap();
    let first = insert_book(&pool, "kostenbuch", "aa").await;
    let second = insert_book(&pool, "kostenbuch-2", "bb").await;
    let before = counts(&pool).await;
    pool.close().await;

    let state = baruch_server::AppState::open(data.path()).await.expect("Start mit 0005, 0006 und 0105");
    let versions: Vec<i64> = sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(state.store.pool())
        .await
        .unwrap();
    for version in [5, 6, 105] {
        assert!(versions.contains(&version), "Migration {version} ist gelaufen: {versions:?}");
    }
    assert_eq!(counts(state.store.pool()).await, before, "gleich viele Werke, Knoten, Revisionen, Beiträge, Verweise");
    let works = state.store.works().await.unwrap();
    assert_eq!(
        works.iter().map(|(_, s, t)| (s.as_str(), t.as_str())).collect::<Vec<_>>(),
        [("kostenbuch", "Kostenbuch"), ("kostenbuch-2", "Kostenbuch (2)")],
        "zwei gleiche Titel: das ältere behält seinen, das jüngere bekommt „(2)“; slugs bleiben"
    );
    for work in [first, second] {
        let book: String = HEADINGS.iter().map(|h| h.5).collect();
        assert_eq!(
            baruch_typst::xref::strip_key_labels(&state.store.export_typst(work).await.unwrap()).into_owned(),
            format!("{TEMPLATE}{book}"),
            "Export byte-gleich"
        );
        let loaded = state.store.load_work(work).await.unwrap();
        let outline: Vec<(&str, &str)> = loaded.nodes.iter().map(|n| (n.number.as_str(), n.title.as_str())).collect();
        assert_eq!(
            outline,
            [("1", "Kosten #1"), ("1.1", "Posten"), ("2", "Summe")],
            "0006 löst die Maskierung im Titel auf; Nummern wie vorher"
        );
    }
    let wrong: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM node n \
          WHERE (SELECT count(*) FROM revision r WHERE r.node_id = n.id AND r.status = 'current') <> 1",
    )
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(wrong, 0, "genau eine aktuelle Fassung je Abschnitt");
    assert_eq!(post_hits(&state).await, 2, "die Beiträge beider Bücher sind weiter im Volltext auffindbar");

    // 0105: Löschen eines ganzen Werks nimmt seine Beiträge aus dem Index (Trigger post_fts_delete).
    let deleted = state.store.delete_work(second, "Kostenbuch (2)").await.unwrap();
    assert_eq!(deleted.nodes, 3, "drei Gliederungspunkte gelöscht");
    assert_eq!(post_hits(&state).await, 1, "nur der Beitrag des verbliebenen Buchs wird noch gefunden");
}

/// Fester Schlüssel (`Node::key`) des Abschnitts an Stelle `seq`: Erzeugte Adressen tragen ihn statt
/// der Stelle (`/?work=W&key=K`, Vertrag Runde 8).
async fn key_at(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}
