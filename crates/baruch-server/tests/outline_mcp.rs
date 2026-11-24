// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gliederung über MCP (Wunsch 2026-11-06): `outline_add` legt Punkte sofort an (einen oder eine ganze
//! Liste, atomar, ein Commit, Herkunft = Persona bzw. Client); `outline_rename`, `outline_move`,
//! `outline_set_level`, `outline_set_numbered`, `outline_delete` legen nur Anträge an, über die der
//! Mensch entscheidet (RPC `outline.request_decide`, Formular auf `/gliederung`). Dazu `outline.set_numbered`.
//! Alle Testtexte frei erfunden (`tests/data/gartenbuch.typ`).

use crate::common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

async fn post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

async fn initialize(server: &common::Server) -> String {
    let (headers, answer) = post(
        server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Claude Code", "version": "2.1" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned()
}

async fn call(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                          "params": { "name": tool, "arguments": arguments } });
    post(server, Some(session), message).await.1
}

async fn ok(server: &common::Server, session: &str, tool: &str, arguments: Value) -> Value {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], false, "{tool} sollte gelingen: {answer}");
    answer["result"]["structuredContent"].clone()
}

async fn refused(server: &common::Server, session: &str, tool: &str, arguments: Value) -> String {
    let answer = call(server, session, tool, arguments).await;
    assert_eq!(answer["result"]["isError"], true, "{tool} sollte fachlich scheitern: {answer}");
    answer["result"]["content"][0]["text"].as_str().unwrap().to_owned()
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Titel der Gliederung in Baumreihenfolge.
async fn titles(server: &common::Server, work: i64) -> Vec<String> {
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    result(&answer)["work"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["title"].as_str().unwrap().to_owned())
        .collect()
}

/// Fester Schlüssel des Punkts mit diesem Titel.
async fn key_of(server: &common::Server, work: i64, title: &str) -> i64 {
    let answer = server.rpc("work.get", json!({ "work": work })).await;
    let nodes = result(&answer)["work"]["nodes"].as_array().unwrap().clone();
    nodes.iter().find(|n| n["title"] == title).unwrap_or_else(|| panic!("kein Punkt {title}: {nodes:?}"))["key"]
        .as_i64()
        .unwrap()
}

fn log(server: &common::Server, slug: &str) -> Option<Vec<String>> {
    common::git(&server.mirror_dir_of(slug), &["log", "--format=%s"]).map(|l| l.lines().map(str::to_owned).collect())
}

fn last_commit(server: &common::Server, slug: &str) -> Option<String> {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an <%ae>%n%cn <%ce>%n%B"])
}

async fn garden() -> (common::Server, i64, String, String) {
    let server = common::open().await;
    let report = server.import_garden().await;
    let session = initialize(&server).await;
    (server, report.work, report.slug, session)
}

#[tokio::test]
async fn outline_add_over_mcp_creates_a_point_whose_first_revision_and_commit_name_the_client() {
    let (server, work, slug, s) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    let added = ok(&server, &s, "outline_add", json!({ "work": work, "key": summer, "title": "Herbst" })).await;
    assert_eq!(
        titles(&server, work).await,
        ["Frühling", "Aussaat", "Tiefe", "Sommer", "Herbst"],
        "hinter Sommer (Vorgabe after)"
    );
    let key = added["key"].as_i64().unwrap();
    assert_eq!(added["url"], format!("/?work={work}&key={key}"), "Antwort mit Adresse des neuen Punkts: {added}");
    assert_eq!(added["points"].as_array().unwrap().len(), 1, "genau ein neuer Punkt: {added}");
    assert_eq!(added["author"], "@mcp:claude-code", "Autor ist der Client: {added}");

    let node = ok(&server, &s, "read_node", json!({ "work": work, "key": key })).await;
    assert_eq!(node["text"], "= Herbst\n\n", "Revision 1 = Überschriftszeile + Leerzeile: {node}");
    assert_eq!(
        node["revisions"][0]["author"],
        json!({ "kind": "assistant", "name": "@mcp:claude-code", "model": "Claude Code 2.1" }),
        "Revision 1 trägt den Client, nicht den Menschen: {node}"
    );
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(
            commit.starts_with("MCP-Client (@mcp:claude-code) <mcp-claude-code@persona.baruch.invalid>\nAutor <"),
            "Autor Client, Committer Mensch: {commit}"
        );
        assert!(commit.contains("Gliederung: „Herbst“ nach „Sommer“ angelegt"), "Betreff: {commit}");
        assert!(
            commit.contains("Baruch-Action: outline-add") && commit.contains("Baruch-Origin: mcp"),
            "Herkunft im Spiegel: {commit}"
        );
        assert!(commit.contains("Persona: @mcp:claude-code"), "Handle als Trailer: {commit}");
    }
}

#[tokio::test]
async fn a_whole_outline_in_one_call_is_one_transaction_and_one_commit() {
    let (server, work, slug, s) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    let before = log(&server, &slug);
    let items = json!([
        { "title": "Herbst" },
        { "title": "Laub", "level": 2 },
        { "title": "Rechen", "level": 3 },
        { "title": "Winter", "level": 1 },
        { "title": "Schlussgebet" }
    ]);
    let added =
        ok(&server, &s, "outline_add", json!({ "work": work, "key": summer, "place": "after", "items": items })).await;
    let points = added["points"].as_array().unwrap();
    assert_eq!(points.len(), 5, "jeder Punkt mit key und url: {added}");
    assert!(points.iter().all(|p| p["key"].is_i64() && p["url"].as_str().unwrap().contains("&key=")), "{added}");
    assert_eq!(
        titles(&server, work).await,
        ["Frühling", "Aussaat", "Tiefe", "Sommer", "Herbst", "Laub", "Rechen", "Winter", "Schlussgebet"],
        "Block in Baumreihenfolge hinter Sommer"
    );
    let levels: Vec<i64> = points.iter().map(|p| p["level"].as_i64().unwrap()).collect();
    assert_eq!(levels, [1, 2, 3, 1, 1], "ohne level die Ebene des Orts (Kapitel)");
    if let (Some(before), Some(after)) = (before, log(&server, &slug)) {
        assert_eq!(after.len(), before.len() + 1, "genau ein Commit für die ganze Liste: {after:?}");
        assert!(
            after[0].starts_with("Gliederung: 5 Punkte („Herbst“, „Laub“, „Rechen“, …) nach „Sommer“"),
            "{after:?}"
        );
    }

    // Atomar: ein ungültiger Titel an dritter Stelle → nichts angelegt, kein Commit.
    let shown = titles(&server, work).await;
    let commits = log(&server, &slug);
    let bad = json!([{ "title": "Eins" }, { "title": "Zwei" }, { "title": "   " }]);
    let text = refused(&server, &s, "outline_add", json!({ "work": work, "key": summer, "items": bad })).await;
    assert_eq!(text, "Punkt 3: Ein Gliederungspunkt braucht einen Titel.", "Meldung nennt den Punkt");
    let jump = json!([{ "title": "Eins" }, { "title": "Drei", "level": 3 }]);
    let text = refused(&server, &s, "outline_add", json!({ "work": work, "key": summer, "items": jump })).await;
    assert!(text.contains("höchstens eine Ebene tiefer"), "Ebenensprung abgelehnt: {text}");
    // Bis Runde 13 stand hier „kein Teil in einem Werk mit Werkvorlage“. Seit Runde 14 dürfen
    // Assistenten Teile auch dort anlegen (round14_outline.rs); abgelehnt bleibt ein Teil *unter*
    // einem Punkt (into), weil ein Teil immer auf oberster Ebene steht.
    let part = json!([{ "title": "Erster Teil", "level": 0 }]);
    let args = json!({ "work": work, "key": summer, "place": "into", "items": part });
    let text = refused(&server, &s, "outline_add", args).await;
    assert!(text.contains("oberster Ebene"), "kein Teil unter einem Kapitel: {text}");
    assert_eq!(titles(&server, work).await, shown, "bei einem Fehler nichts angelegt");
    assert_eq!(log(&server, &slug), commits, "und kein Commit");
}

#[tokio::test]
async fn outline_add_keeps_the_title_rules_and_refuses_node() {
    let (server, work, _, s) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    let long = "W".repeat(201);
    let text = refused(&server, &s, "outline_add", json!({ "work": work, "key": summer, "title": long })).await;
    assert_eq!(text, "Der Titel ist zu lang (höchstens 200 Zeichen, hier 201).", "Titelregel wie überall");
    let text =
        refused(&server, &s, "outline_add", json!({ "work": work, "key": summer, "title": "Zwei\nZeilen" })).await;
    assert_eq!(text, "Der Titel darf keinen Zeilenumbruch enthalten.", "einzeilig");
    let answer = call(&server, &s, "outline_add", json!({ "work": work, "node": 3, "title": "Herbst" })).await;
    assert_eq!(answer["error"]["code"], -32602, "node (Stelle) wird abgelehnt: {answer}");
    let answer = call(&server, &s, "outline_add", json!({ "work": work, "title": "Herbst" })).await;
    assert_eq!(answer["result"]["isError"], true, "ohne key nur in einem leeren Werk: {answer}");
    assert_eq!(titles(&server, work).await.len(), 4, "nichts angelegt");
}

#[tokio::test]
async fn in_a_work_without_points_outline_add_creates_the_first_one_without_key() {
    let (server, work, _, s) = garden().await;
    // Ein Werk ohne Gliederungspunkt entsteht über die Oberfläche nicht (Anlegen bringt ein Kapitel,
    // das letzte lässt sich nicht löschen); nachgestellt direkt in SQLite.
    let pool = server.state.store.pool();
    for sql in [
        "UPDATE revision SET status = 'superseded' WHERE status = 'current' AND node_id IN (SELECT id FROM node WHERE work_id = ?)",
        "DELETE FROM revision WHERE node_id IN (SELECT id FROM node WHERE work_id = ?)",
        "DELETE FROM node WHERE work_id = ?",
    ] {
        sqlx::query(sql).bind(work).execute(pool).await.unwrap();
    }
    assert!(titles(&server, work).await.is_empty(), "Werk ohne Punkt");
    let added = ok(&server, &s, "outline_add", json!({ "work": work, "title": "Anfang" })).await;
    assert_eq!(titles(&server, work).await, ["Anfang"], "erster Punkt ohne key: {added}");
    assert_eq!(added["points"][0]["level"], 1, "ein Kapitel");
}

#[tokio::test]
async fn a_rename_request_changes_nothing_until_the_human_confirms_and_the_commit_names_both() {
    let (server, work, slug, s) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    let commits = log(&server, &slug);
    let arguments = json!({ "work": work, "key": summer, "title": "Hochsommer", "note": "klingt wärmer" });
    let asked = ok(&server, &s, "outline_rename", arguments).await;
    assert_eq!(asked["status"], "open", "ein offener Antrag: {asked}");
    assert_eq!(asked["request"]["summary"], "„Sommer“ in „Hochsommer“ umbenennen", "{asked}");
    assert_eq!(titles(&server, work).await[3], "Sommer", "der Antrag ändert nichts");
    assert_eq!(log(&server, &slug), commits, "und schreibt keinen Commit");
    let mine = ok(&server, &s, "outline_requests", json!({ "work": work, "mine": true })).await;
    assert_eq!(mine["requests"].as_array().unwrap().len(), 1, "eigene offene Anträge lesbar: {mine}");

    let listed = server.rpc("outline.requests", json!({ "work": work })).await;
    let id = result(&listed)["requests"][0]["id"].as_i64().unwrap();
    assert_eq!(result(&listed)["requests"][0]["origin_label"], "MCP @mcp:claude-code", "Antragsteller: {listed}");
    let decided = server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await;
    assert_eq!(result(&decided)["request"]["decision"], "confirmed", "{decided}");
    assert_eq!(titles(&server, work).await[3], "Hochsommer", "Bestätigen führt aus");
    if let Some(commit) = last_commit(&server, &slug) {
        assert!(commit.starts_with("Autor <autor@baruch.invalid>\nAutor <"), "Mensch als Bestätiger: {commit}");
        assert!(commit.contains(&format!("(Antrag {id})")), "Betreff nennt den Antrag: {commit}");
        for trailer in [
            "Baruch-Origin: mcp".to_owned(),
            format!("Baruch-Request: {id}"),
            "Baruch-Requested-By: @mcp:claude-code".to_owned(),
            "Baruch-Confirmed-By: Autor".to_owned(),
            "Baruch-Action: outline-rename".to_owned(),
        ] {
            assert!(commit.contains(&trailer), "Trailer {trailer} fehlt: {commit}");
        }
        assert!(commit.contains("Begründung: klingt wärmer"), "Begründung im Rumpf: {commit}");
    }
    let again = server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "reject" })).await;
    assert_eq!(again["error"]["code"], -32000, "über einen Antrag wird genau einmal entschieden: {again}");
}

#[tokio::test]
async fn rejecting_a_request_changes_nothing() {
    let (server, work, slug, s) = garden().await;
    let spring = key_of(&server, work, "Frühling").await;
    let summer = key_of(&server, work, "Sommer").await;
    let arguments = json!({ "work": work, "key": summer, "target_key": spring, "place": "before" });
    ok(&server, &s, "outline_move", arguments).await;
    let before = titles(&server, work).await;
    let commits = log(&server, &slug);
    let listed = server.rpc("outline.requests", json!({ "work": work })).await;
    let id = result(&listed)["requests"][0]["id"].as_i64().unwrap();
    let decided = server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "reject" })).await;
    assert_eq!(result(&decided)["request"]["decision"], "rejected", "{decided}");
    assert!(result(&decided)["outline"].is_null(), "keine Gliederungsänderung: {decided}");
    assert_eq!(titles(&server, work).await, before, "Ablehnen ändert die Gliederung nicht");
    // Nutzerentscheidung 2026-11-08: Ablehnen steht für jede Antragsart als ein leerer Protokoll-Commit im Spiegel.
    if let (Some(before), Some(after)) = (commits, log(&server, &slug)) {
        let mut expected = vec![format!("Gliederung: Antrag {id} abgelehnt")];
        expected.extend(before);
        assert_eq!(after, expected, "genau ein Protokoll-Commit für die Ablehnung, sonst nichts");
    }
    let open = server.rpc("outline.requests", json!({ "work": work })).await;
    assert!(result(&open)["requests"].as_array().unwrap().is_empty(), "nicht mehr offen: {open}");
}

#[tokio::test]
async fn a_stale_request_is_closed_with_a_reason_and_changes_nothing() {
    let (server, work, slug, s) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    let depth = key_of(&server, work, "Tiefe").await;
    ok(&server, &s, "outline_rename", json!({ "work": work, "key": summer, "title": "Hochsommer" })).await;
    ok(&server, &s, "outline_delete", json!({ "work": work, "key": depth })).await;
    // Inzwischen benennt der Mensch selbst um und löscht „Tiefe“.
    result(&server.rpc("outline.rename", json!({ "work": work, "key": summer, "title": "Spätsommer" })).await);
    result(&server.rpc("outline.delete", json!({ "work": work, "key": depth, "confirm": true })).await);
    let commits = log(&server, &slug);
    let listed = server.rpc("outline.requests", json!({ "work": work })).await;
    let ids: Vec<i64> =
        result(&listed)["requests"].as_array().unwrap().iter().map(|r| r["id"].as_i64().unwrap()).collect();
    let renamed =
        server.rpc("outline.request_decide", json!({ "work": work, "id": ids[0], "decision": "confirm" })).await;
    assert_eq!(result(&renamed)["request"]["decision"], "stale", "veraltet statt ausgeführt: {renamed}");
    assert_eq!(
        result(&renamed)["request"]["message"],
        "Der Punkt heißt inzwischen „Spätsommer“ (beim Antrag „Sommer“).",
        "Meldung nennt den Grund"
    );
    let deleted =
        server.rpc("outline.request_decide", json!({ "work": work, "id": ids[1], "decision": "confirm" })).await;
    assert_eq!(result(&deleted)["request"]["decision"], "stale", "{deleted}");
    assert!(result(&deleted)["request"]["message"].as_str().unwrap().contains("gibt es nicht mehr"), "{deleted}");
    assert_eq!(titles(&server, work).await, ["Frühling", "Aussaat", "Spätsommer"], "Gliederung unverändert");
    // Nutzerentscheidung 2026-11-08: auch „veraltet“ steht je Antrag als leerer Protokoll-Commit im Spiegel.
    if let (Some(before), Some(after)) = (commits, log(&server, &slug)) {
        let mut expected =
            vec![format!("Gliederung: Antrag {} veraltet", ids[1]), format!("Gliederung: Antrag {} veraltet", ids[0])];
        expected.extend(before);
        assert_eq!(after, expected, "je veraltetem Antrag genau ein Protokoll-Commit, keine Gliederungsänderung");
    }
}

#[tokio::test]
async fn requests_that_could_not_run_now_are_refused_at_once_and_not_stored() {
    let (server, work, _, s) = garden().await;
    let spring = key_of(&server, work, "Frühling").await;
    let text = refused(&server, &s, "outline_delete", json!({ "work": work, "key": spring })).await;
    assert!(text.contains("Unterpunkte"), "Löschen nur ohne Unterpunkte wie im Store: {text}");
    let text = refused(&server, &s, "outline_rename", json!({ "work": work, "key": spring, "title": "" })).await;
    assert_eq!(text, "Ein Gliederungspunkt braucht einen Titel.", "Titelregeln schon beim Antrag");
    let text = refused(&server, &s, "outline_set_level", json!({ "work": work, "key": spring, "level": 1 })).await;
    assert!(text.contains("schon diese Ebene"), "ohne Wirkung kein Antrag: {text}");
    let arguments = json!({ "work": work, "key": spring, "target": 3, "place": "after" });
    let answer = call(&server, &s, "outline_move", arguments).await;
    assert_eq!(answer["error"]["code"], -32602, "target (Stelle) wird abgelehnt: {answer}");
    let summer = key_of(&server, work, "Sommer").await;
    ok(&server, &s, "outline_set_level", json!({ "work": work, "key": summer, "level": 2 })).await;
    let text = refused(&server, &s, "outline_set_level", json!({ "work": work, "key": summer, "level": 2 })).await;
    assert!(text.contains("gibt es schon"), "kein doppelter offener Antrag: {text}");
    let all = server.rpc("outline.requests", json!({ "work": work, "open": false })).await;
    assert_eq!(result(&all)["requests"].as_array().unwrap().len(), 1, "nur der eine gültige Antrag: {all}");
}

#[tokio::test]
async fn requests_can_be_decided_without_javascript_and_the_tree_points_to_them() {
    let (server, work, _, s) = garden().await;
    let summer = key_of(&server, work, "Sommer").await;
    ok(&server, &s, "outline_rename", json!({ "work": work, "key": summer, "title": "Hochsommer & Hitze" })).await;
    let (status, page) = server.get(&format!("/?work={work}&key={summer}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("1 Antrag zur Gliederung") && page.contains("#antraege"), "Hinweis im Gliederungsbaum");
    let (status, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Anträge der Assistenten (1 offen)"), "Abschnitt mit offenen Anträgen");
    assert!(page.contains("„Sommer“ in „Hochsommer &amp; Hitze“ umbenennen"), "Antrag maskiert: {page}");
    assert!(
        page.contains(r#"name="method" value="outline.request_decide""#)
            && page.contains(">Bestätigen</button>")
            && page.contains(">Ablehnen</button>"),
        "Formulare ohne JavaScript"
    );
    let listed = server.rpc("outline.requests", json!({ "work": work })).await;
    let id = result(&listed)["requests"][0]["id"].as_i64().unwrap();
    let form = format!("method=outline.request_decide&work={work}&id={id}&decision=confirm&view=gliederung");
    let foreign = Request::post("/gliederung/act")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::ORIGIN, "http://127.0.0.2:9999")
        .header(header::HOST, "127.0.0.1:8765")
        .body(Body::from(form.clone()))
        .unwrap();
    let (status, _, _) = server.send(foreign).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "eine fremde Seite darf nicht bestätigen (CSRF)");
    assert_eq!(titles(&server, work).await[3], "Sommer", "nichts geändert");
    let (status, location) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Formular leitet weiter");
    assert!(location.as_deref().unwrap().ends_with("#antraege"), "zurück zu den Anträgen: {location:?}");
    assert_eq!(titles(&server, work).await[3], "Hochsommer & Hitze", "bestätigt und ausgeführt");
    let (_, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert!(
        page.contains("Keine offenen Anträge.") && page.contains("<strong>bestätigt</strong>"),
        "zuletzt entschieden"
    );
}

#[tokio::test]
async fn a_point_can_be_counted_without_number_by_rpc_form_and_request() {
    let (server, work, _, s) = garden().await;
    let spring = key_of(&server, work, "Frühling").await;
    async fn numbers(server: &common::Server, work: i64) -> Vec<String> {
        let answer = server.rpc("work.get", json!({ "work": work })).await;
        let nodes = result(&answer)["work"]["nodes"].as_array().unwrap().clone();
        nodes.iter().map(|n| n["number"].as_str().unwrap().to_owned()).collect()
    }
    let before = numbers(&server, work).await;
    assert_eq!(before, ["1", "1.1", "1.1.1", "2"], "Ausgangslage");
    let done = server.rpc("outline.set_numbered", json!({ "work": work, "key": spring, "numbered": false })).await;
    assert_eq!(result(&done)["action"], "numbered", "{done}");
    let after = numbers(&server, work).await;
    assert_eq!(after[0], "", "Frühling ohne Nummer: {after:?}");
    assert_eq!(after[3], "1", "Sommer rückt nach: {after:?}");
    let again = server.rpc("outline.set_numbered", json!({ "work": work, "key": spring, "numbered": false })).await;
    assert_eq!(again["error"]["code"], -32000, "schon ohne Nummer: {again}");

    let (_, page) = server.get(&format!("/gliederung?work={work}")).await;
    assert!(
        page.contains(r#"value="outline.set_numbered""#) && page.contains(">mit Nummer</button>"),
        "Umschalter im Gliederungsmanager"
    );
    let form = format!("method=outline.set_numbered&work={work}&key={spring}&numbered=1&view=gliederung");
    let (status, _) = server.post_form("/gliederung/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript per Formular");
    assert_eq!(numbers(&server, work).await, before, "wieder nummeriert");

    let arguments = json!({ "work": work, "key": spring, "numbered": false });
    let asked = ok(&server, &s, "outline_set_numbered", arguments).await;
    assert_eq!(asked["request"]["summary"], "„Frühling“ ohne Nummer zählen", "über MCP nur als Antrag: {asked}");
    assert_eq!(numbers(&server, work).await, before, "der Antrag ändert nichts");
}
