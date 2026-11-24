// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einfälle über RPC und Oberfläche (Vertrag Runde 7, Abschnitte 6c–6e): echter Datenordner im
//! Temp-Ordner (SQLite, Bibliothek, Git-Spiegel), Router über `oneshot`. Je Aktion genau ein Commit
//! mit Herkunft, `einfaelle.md` im Werkspiegel bzw. in `<data>/einfaelle/`, Verarbeiten zu
//! Gliederungspunkt, Beitrag und Notiz, Persona-Auftrag als Stub, Seiten ohne JavaScript mit
//! Formularen an `/einfaelle/act`, Maskierung und Schutz vor fremden Seiten. Testtexte frei erfunden.

use crate::common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().unwrap_or_else(|| panic!("Fehler erwartet: {answer}"))
}

/// Formularwert prozentkodieren.
fn enc(value: &str) -> String {
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

fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&")
}

async fn key(server: &common::Server, work: i64, seq: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM node WHERE work_id = ? AND seq = ?")
        .bind(work)
        .bind(seq)
        .fetch_one(server.state.store.pool())
        .await
        .unwrap()
}

/// Anzahl der Commits im Spiegelordner; `None` ohne git-Programm.
fn commits(dir: &std::path::Path) -> Option<usize> {
    common::git(dir, &["rev-list", "--count", "HEAD"]).map(|n| n.trim().parse().unwrap())
}

fn last_message(dir: &std::path::Path) -> Option<String> {
    common::git(dir, &["log", "-1", "--format=%B"])
}

#[tokio::test]
async fn each_action_is_one_commit_with_trailers_and_einfaelle_md_follows() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let dir = server.mirror_dir();
    let k = key(&server, work, 1).await;
    let start = commits(&dir);

    let created = server
        .rpc(
            "idea.create",
            json!({ "work": work, "key": k, "words": "Laube, Rosen; Laube", "note": "Ein Dach aus Rosen." }),
        )
        .await;
    let created = result(&created).clone();
    let id = created["idea"]["id"].as_i64().unwrap();
    assert_eq!(created["idea"]["words"], json!(["Laube", "Rosen"]), "Text an Komma und Semikolon geteilt, Doppel weg");
    assert!(created["commit"].is_string(), "Anlegen ergibt einen Commit: {created}");
    let file = std::fs::read_to_string(dir.join("einfaelle.md")).expect("einfaelle.md im Werkspiegel");
    assert!(file.contains(&format!("## Einfall {id} · offen")), "Eintrag im Spiegel: {file}");
    assert!(file.contains("- Gedanke: Ein Dach aus Rosen."), "Gedanke im Spiegel: {file}");

    if let (Some(start), Some(message)) = (start, last_message(&dir)) {
        assert_eq!(commits(&dir), Some(start + 1), "genau ein Commit");
        assert!(message.starts_with(&format!("Einfall {id}: angelegt")), "Betreff: {message}");
        for trailer in
            [format!("Baruch-Idea: {id}"), "Baruch-Action: idea-create".into(), "Baruch-Origin: human".into()]
        {
            assert!(message.contains(&trailer), "{trailer} fehlt: {message}");
        }
        assert!(!message.contains("Persona:"), "kein Persona-Trailer beim Menschen: {message}");
    }

    let unchanged = server.rpc("idea.update", json!({ "id": id, "note": "Ein Dach aus Rosen." })).await;
    assert_eq!(result(&unchanged)["changed"], false, "gleicher Text: unverändert");
    assert!(result(&unchanged)["commit"].is_null(), "unverändert: kein Commit");
    let changed = server.rpc("idea.update", json!({ "id": id, "note": "Ein Dach aus Efeu.", "status": "done" })).await;
    let changed = result(&changed);
    assert_eq!(
        (changed["changed"].as_bool(), changed["idea"]["status"].as_str()),
        (Some(true), Some("done")),
        "{changed}"
    );
    if let Some(message) = last_message(&dir) {
        assert!(message.starts_with(&format!("Einfall {id}: geändert")), "Betreff: {message}");
        assert!(message.contains("Gedanke vorher: Ein Dach aus Rosen."), "vorher/nachher im Rumpf: {message}");
        assert!(message.contains("Baruch-Action: idea-update"), "{message}");
    }

    let refused = server.rpc("idea.delete", json!({ "id": id })).await;
    assert_eq!(error_code(&refused), -32602, "Löschen ohne confirm → -32602");
    let deleted = server.rpc("idea.delete", json!({ "id": id, "confirm": true })).await;
    assert_eq!(result(&deleted)["deleted"], true, "{deleted}");
    assert!(!dir.join("einfaelle.md").exists(), "letzter Einfall gelöscht: Datei fehlt im Spiegel");
    if let Some(message) = last_message(&dir) {
        assert!(message.starts_with(&format!("Einfall {id}: gelöscht")), "Betreff: {message}");
        assert!(
            message.contains("Gelöschter Eintrag:") && message.contains("- Schlagwörter: Laube, Rosen"),
            "der ganze Eintrag steht im Lösch-Commit: {message}"
        );
    }
    let listed = server.rpc("idea.list", json!({ "work": work })).await;
    assert!(result(&listed)["ideas"].as_array().unwrap().is_empty(), "gelöschte fehlen in der Liste");
    let restored = server.rpc("idea.restore", json!({ "id": id })).await;
    assert!(result(&restored)["idea"]["deleted"].is_null(), "wiederhergestellt: {restored}");
    assert!(dir.join("einfaelle.md").exists(), "wieder im Spiegel");
    if let (Some(start), Some(message)) = (start, last_message(&dir)) {
        assert!(message.contains("Baruch-Action: idea-restore"), "{message}");
        assert_eq!(commits(&dir), Some(start + 4), "anlegen, ändern, löschen, wiederherstellen: vier Commits");
    }
    let got = server.rpc("idea.get", json!({ "id": id })).await;
    assert_eq!(result(&got)["idea"]["note"], "Ein Dach aus Efeu.", "idea.get");
    assert_eq!(error_code(&server.rpc("idea.get", json!({ "id": 999 })).await), -32602, "unbekannte id → -32602");
}

#[tokio::test]
async fn ideas_without_a_work_have_their_own_mirror_and_moving_commits_in_both() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let work_dir = server.mirror_dir();
    let loose_dir = server.path().join("einfaelle");
    let created = server.rpc("idea.create", json!({ "words": ["Zaun"], "note": "Ohne Werk." })).await;
    let id = result(&created)["idea"]["id"].as_i64().unwrap();
    assert!(result(&created)["idea"]["work"].is_null(), "ohne Werk");
    let file = std::fs::read_to_string(loose_dir.join("einfaelle.md")).expect("<data>/einfaelle/einfaelle.md");
    assert!(file.starts_with("# Einfälle ohne Werk\n") && file.contains("Zaun"), "{file}");
    assert!(!work_dir.join("einfaelle.md").exists(), "nicht im Werkspiegel");

    let (before_work, before_loose) = (commits(&work_dir), commits(&loose_dir));
    let moved = server.rpc("idea.update", json!({ "id": id, "work": work })).await;
    let moved = result(&moved);
    assert_eq!(moved["idea"]["work"], work, "umgezogen: {moved}");
    assert!(work_dir.join("einfaelle.md").exists(), "jetzt im Werkspiegel");
    assert!(!loose_dir.join("einfaelle.md").exists(), "nicht mehr unter einfaelle/");
    if let (Some(w), Some(l)) = (before_work, before_loose) {
        assert_eq!(
            (commits(&work_dir), commits(&loose_dir)),
            (Some(w + 1), Some(l + 1)),
            "je ein Commit im alten und neuen"
        );
        assert_eq!(moved["commits"].as_array().map(Vec::len), Some(2), "beide Commits in der Antwort: {moved}");
    }
}

#[tokio::test]
async fn list_and_search_filter_and_an_empty_search_is_refused() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    result(&server.rpc("idea.create", json!({ "work": work, "words": ["Laube"], "note": "Gärtnerin pflanzt" })).await);
    result(&server.rpc("idea.create", json!({ "words": ["Laube", "Dorn"] })).await);
    let all = server.rpc("idea.list", json!({})).await;
    assert_eq!(result(&all)["ideas"].as_array().unwrap().len(), 2, "alle");
    assert_eq!(result(&all)["words"][0], json!({ "word": "Laube", "count": 2 }), "Wolke, häufigste zuerst: {all}");
    let none = server.rpc("idea.list", json!({ "work": "none" })).await;
    assert_eq!(result(&none)["ideas"].as_array().unwrap().len(), 1, "ohne Werk");
    let dorn = server.rpc("idea.list", json!({ "word": "dorn", "status": "open" })).await;
    assert_eq!(result(&dorn)["ideas"].as_array().unwrap().len(), 1, "Schlagwort und Status");
    let found = server.rpc("idea.search", json!({ "query": "gartnerin" })).await;
    let found = result(&found);
    assert_eq!(found["hits"].as_array().unwrap().len(), 1, "Volltext ohne Akzente: {found}");
    assert!(found["hits"][0]["snippet"].as_str().unwrap().contains("<mark>"), "Ausschnitt markiert: {found}");
    assert_eq!(
        error_code(&server.rpc("idea.search", json!({ "query": "  ( ) " })).await),
        -32602,
        "leere Suche → -32602"
    );
    assert_eq!(error_code(&server.rpc("idea.list", json!({ "work": 999 })).await), -32602, "unbekanntes Werk → -32602");
    assert_eq!(
        error_code(&server.rpc("idea.create", json!({ "words": [] })).await),
        -32602,
        "ohne Schlagwort → -32602"
    );
}

#[tokio::test]
async fn processing_makes_an_outline_point_a_post_or_a_note_and_the_persona_task_needs_a_model() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let dir = server.mirror_dir();
    let k = key(&server, work, 1).await;
    let nodes_before = server.state.store.load_work(work).await.unwrap().nodes.len();
    let created = server.rpc("idea.create", json!({ "work": work, "key": k, "words": ["Laube", "Abendlicht"] })).await;
    let id = result(&created)["idea"]["id"].as_i64().unwrap();
    let start = commits(&dir);

    let processed = server.rpc("idea.process", json!({ "id": id, "as": "outline" })).await;
    let processed = result(&processed);
    assert_eq!(processed["idea"]["status"], "done", "verarbeitet: {processed}");
    assert_eq!(processed["idea"]["done_as"], "outline", "als Gliederungspunkt");
    let loaded = server.state.store.load_work(work).await.unwrap();
    assert_eq!(loaded.nodes.len(), nodes_before + 1, "ein Punkt mehr");
    let node = processed["node"].as_u64().unwrap() as usize;
    assert_eq!(loaded.nodes[node].title, "Laube, Abendlicht", "Titel aus den Schlagwörtern");
    assert_eq!(processed["key"], loaded.nodes[node].key, "Schlüssel des neuen Punkts");
    assert!(processed["outline"]["commit"].is_string(), "Gliederungs-Commit: {processed}");
    if let (Some(start), Some(message)) = (start, last_message(&dir)) {
        assert_eq!(commits(&dir), Some(start + 2), "Gliederung und Verarbeitung: zwei Commits");
        assert!(message.starts_with(&format!("Einfall {id}: verarbeitet als Gliederungspunkt")), "{message}");
        assert!(message.contains("Baruch-Action: idea-process"), "{message}");
    }

    let quoted = server
        .rpc(
            "idea.create",
            json!({ "work": work, "key": k, "words": ["Rose"], "note": "Rosen am Tor", "quote": "Das Tor knarrt." }),
        )
        .await;
    let quoted = result(&quoted)["idea"]["id"].as_i64().unwrap();
    let posts_before = server.state.store.load_work(work).await.unwrap().nodes[1].posts.len();
    let post = server.rpc("idea.process", json!({ "id": quoted, "as": "post" })).await;
    assert_eq!(result(&post)["idea"]["done_as"], "post", "{post}");
    let posts = server.state.store.load_work(work).await.unwrap().nodes[1].posts.clone();
    assert_eq!(posts.len(), posts_before + 1, "ein Beitrag mehr");
    let last = posts.last().unwrap();
    assert_eq!(last.author, "Autor", "Autor ist der Mensch");
    assert_eq!(last.body, "Rosen am Tor\n\n> Das Tor knarrt.", "Gedanke und Markierung als Zitat");
    assert_eq!(last.sources, [format!("Einfall {quoted}")], "Quelle nennt den Einfall");

    let noted = server.rpc("idea.create", json!({ "work": work, "words": ["Tor"] })).await;
    let noted = result(&noted)["idea"]["id"].as_i64().unwrap();
    let without = server.rpc("idea.process", json!({ "id": noted, "as": "note" })).await;
    assert_eq!(error_code(&without), -32602, "ohne Abschnitt (key) → -32602: {without}");
    let note = server.rpc("idea.process", json!({ "id": noted, "as": "note", "key": k })).await;
    assert_eq!(result(&note)["idea"]["done_as"], "note", "{note}");
    let posts = server.state.store.load_work(work).await.unwrap().nodes[1].posts.clone();
    assert_eq!((posts.last().unwrap().author.as_str(), posts.last().unwrap().body.as_str()), ("Notiz", "Tor"), "Notiz");

    let stub = server.rpc("idea.create", json!({ "work": work, "key": k, "words": ["Auftrag"] })).await;
    let stub = result(&stub)["idea"]["id"].as_i64().unwrap();
    let answer = server.rpc("idea.process", json!({ "id": stub, "as": "persona", "persona": "@lektor" })).await;
    // Runde 10: kein Stub mehr; ohne Modell-Backend (AppState::open = NoModel) ehrlich abgelehnt.
    assert_eq!(error_code(&answer), -32000, "ohne Modell-Backend abgelehnt: {answer}");
    let message = answer["error"]["message"].as_str().unwrap();
    assert!(message.contains("Kein Modell-Backend"), "ehrliche Meldung ohne Backend: {message}");
    let still = server.rpc("idea.get", json!({ "id": stub })).await;
    assert_eq!(result(&still)["idea"]["status"], "open", "der Einfall bleibt offen (nichts gespeichert)");
}

#[tokio::test]
async fn the_pages_work_without_javascript_and_escape_everything() {
    let server = common::open().await;
    let work = server.state.default_work().await.unwrap();
    let evil = "<b>\"Anna's\" & Co</b>";
    let created = server
        .rpc("idea.create", json!({ "work": work, "words": [evil, "Laube"], "note": evil, "source": evil }))
        .await;
    let id = result(&created)["idea"]["id"].as_i64().unwrap();

    let (status, html) = server.get(&format!("/einfaelle?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(!html.contains(evil) && !html.contains("<b>"), "nichts unmaskiert: {html}");
    assert!(html.contains("&lt;b&gt;&quot;Anna&#x27;s&quot; &amp; Co&lt;&#x2f;b&gt;"), "maskiert gezeigt: {html}");
    assert!(
        html.contains("action=\"/einfaelle/act\"") && html.contains("name=\"method\" value=\"idea.create\""),
        "Formular „Neuer Einfall“"
    );
    assert!(
        html.contains(&format!("href=\"/einfaelle?work={work}&amp;word=Laube\"")),
        "Wolke verlinkt das Schlagwort: {html}"
    );
    assert!(html.contains(&format!("href=\"/einfaelle/{id}\"")), "Link auf den Einfall");
    assert!(!html.contains("{{") && !html.contains("{%"), "keine Platzhalter-Reste");

    let (status, html) = server.get(&format!("/einfaelle?work={work}&q=Anna")).await;
    assert_eq!(status, StatusCode::OK, "Suche nach „Anna“ liefert die Liste");
    assert!(html.contains("Treffer der Volltextsuche") && html.contains("<mark>"), "Suche auf der Seite: {html}");
    assert!(!html.contains("<b>"), "auch der Ausschnitt maskiert: {html}");

    let (status, html) = server.get(&format!("/einfaelle/{id}")).await;
    assert_eq!(status, StatusCode::OK, "Seite des Einfalls lädt");
    for part in [
        "value=\"idea.update\"",
        "value=\"idea.process\"",
        "value=\"idea.delete\"",
        "name=\"confirm\"",
        "value=\"persona\"",
    ] {
        assert!(html.contains(part), "{part} fehlt auf der Seite des Einfalls");
    }
    assert!(!html.contains("<b>"), "maskiert: {html}");
    assert_eq!(server.get("/einfaelle/999").await.0, StatusCode::NOT_FOUND, "unbekannter Einfall → 404");

    let (status, location) = server
        .post_form(
            "/einfaelle/act",
            &form(&[("method", "idea.create"), ("work", &work.to_string()), ("words", "Hecke, Zaun"), ("note", "Neu")]),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anlegen leitet weiter");
    let location = location.unwrap();
    assert!(location.starts_with("/einfaelle/") && location.ends_with("?ok=angelegt"), "{location}");
    let new_id: i64 = location.trim_start_matches("/einfaelle/").trim_end_matches("?ok=angelegt").parse().unwrap();

    let (status, _) =
        server.post_form("/einfaelle/act", &form(&[("method", "idea.delete"), ("id", &new_id.to_string())])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Kästchen kein Löschen");
    let (status, _) =
        server.post_form("/einfaelle/act", &form(&[("method", "idea.create"), ("words", ""), ("work", "none")])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Schlagwort → 422");
    let (status, location) = server
        .post_form("/einfaelle/act", &form(&[("method", "idea.delete"), ("id", &new_id.to_string()), ("confirm", "1")]))
        .await;
    assert_eq!((status, location.as_deref()), (StatusCode::SEE_OTHER, Some("/einfaelle?ok=geloescht")), "gelöscht");
    let (_, html) = server.get(&format!("/einfaelle/{new_id}")).await;
    assert!(html.contains("value=\"idea.restore\""), "Wiederherstellen auf der Seite des gelöschten Einfalls");
    let (status, _) =
        server.post_form("/einfaelle/act", &form(&[("method", "idea.restore"), ("id", &new_id.to_string())])).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "wiederhergestellt");

    let (status, location) = server
        .post_form(
            "/einfaelle/act",
            &form(&[("method", "idea.create"), ("words", "Weg"), ("back", "//fremd.example/")]),
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Anlegen über das Formular leitet weiter");
    assert!(location.unwrap().starts_with("/einfaelle/"), "fremder Rücksprung wird übergangen");
    let (status, location) = server
        .post_form("/einfaelle/act", &form(&[("method", "idea.process"), ("id", &id.to_string()), ("as", "persona")]))
        .await;
    // Runde 10: kein Stub mehr; dieser Einfall hat keinen Abschnitt, also fehlt das Ziel des Auftrags.
    assert_eq!(
        (status, location),
        (StatusCode::UNPROCESSABLE_ENTITY, None),
        "Persona-Auftrag über das Formular ohne Abschnitt: 422 mit der Seite"
    );
    let (status, _) = server.post_form("/einfaelle/act", &form(&[("method", "work.delete")])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nur idea.* über dieses Formular");
}

#[tokio::test]
async fn the_header_has_the_idea_button_and_dialog_and_foreign_pages_are_refused() {
    let server = common::open().await;
    let (status, html) = server.get("/").await;
    assert_eq!(status, StatusCode::OK, "Startseite lädt");
    assert!(
        html.contains("data-idea-open") && html.contains("title=\"Einfall festhalten (Alt+E)\""),
        "Schaltfläche „Einfall“"
    );
    assert!(html.contains("<dialog id=\"idea-capture\" class=\"idea-dialog\" data-widget=\"idea-capture\""), "Dialog");
    assert!(html.contains("name=\"method\" value=\"idea.create\""), "Formular der Schnellerfassung");
    assert!(
        html.contains("href=\"&#x2f;einfaelle?work=") || html.contains("href=\"/einfaelle?work="),
        "ohne JavaScript ein Link auf /einfaelle"
    );

    let request = Request::post("/einfaelle/act")
        .header(header::HOST, "127.0.0.1:8765")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form(&[("method", "idea.create"), ("words", "fremd")])))
        .unwrap();
    assert_eq!(server.send(request).await.0, StatusCode::FORBIDDEN, "fremder Origin → 403");
    let listed = server.rpc("idea.list", json!({})).await;
    assert!(result(&listed)["ideas"].as_array().unwrap().is_empty(), "nichts angelegt");
}

#[tokio::test]
async fn the_header_has_a_button_for_the_list_of_ideas_next_to_the_capture() {
    let server = common::open().await;
    let (status, html) = server.get("/?work=1").await;
    assert_eq!(status, StatusCode::OK, "Werkansicht lädt");
    assert!(
        html.contains("href=\"&#x2f;einfaelle?work=1\" title=\"Liste der Einfälle dieses Werks\">Einfälle</a>")
            || html.contains("href=\"/einfaelle?work=1\" title=\"Liste der Einfälle dieses Werks\">Einfälle</a>"),
        "eigener Knopf „Einfälle“ zur Liste, nicht nur die Schnellerfassung: {html}"
    );
    assert!(html.contains("data-idea-open"), "die Schnellerfassung „Einfall“ bleibt daneben");
}
