// SPDX-License-Identifier: AGPL-3.0-or-later
//! Variants (Round 30b): solution.variants (siblings A/B/C on the same base, line, implemented findings), partial
//! proposal for long sections, comparison page /varianten, „übrige verwerfen“, chat order, MCP solution_variants, form
//! without JavaScript and the round trip in the exchange format minor version 5.
//!
//! Owner: chat-loesungen. No network, no Eliezer: models only through `ScriptedModel`. Texts freely invented.

use crate::common;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::assist::chat::ChatOrder;
use baruch_server::assist::mock::{ScriptedModel, Step};
use baruch_server::assist::port::{ModelCall, Role};
use baruch_server::assist::variants;
use serde_json::{Value, json};

const TOWER: &str = "#set document(title: \"Das Brückenbuch\", author: \"Ida Kai\")\n\n\
= Ankunft\n\nDas Boot legt am Kai an.\n\n\
== Der Leuchtturm\n\nDer Leuchtturm steht am Kai. Immer leuchtet der Turm über der Mole.\n\n\
== Der Wachturm\n\nDer Wächter zählt die Schiffe.\n";

struct Tower {
    server: common::Server,
    model: ScriptedModel,
    work: i64,
    keys: Vec<i64>,
}

async fn tower_with(source: &str, steps: impl IntoIterator<Item = Step>) -> Tower {
    let model = ScriptedModel::with(steps);
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(model.clone()),
    )
    .await
    .expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let report = server.import(source, "bruecke.typ").await;
    let keys = server.state.store.load_work(report.work).await.unwrap().nodes.iter().map(|n| n.key).collect();
    Tower { server, model, work: report.work, keys }
}

async fn tower(steps: impl IntoIterator<Item = Step>) -> Tower {
    tower_with(TOWER, steps).await
}

fn result(answer: &Value) -> &Value {
    assert!(answer.get("error").is_none(), "unerwarteter Fehler: {answer}");
    &answer["result"]
}

fn user(call: &ModelCall) -> &str {
    call.messages.iter().find(|m| m.role == Role::User).map(|m| m.content.as_str()).unwrap()
}

/// Post of @lektor on the Leuchtturm with one critical point including a solution, split as after an order; returns
/// (post, finding).
async fn finding_with_solution(t: &Tower) -> (i64, i64) {
    let body = "Kritisch: „Immer leuchtet der Turm“ behauptet zu viel.\n- statt: „Immer leuchtet der Turm“\n\
                - besser: „Meist leuchtet der Turm“";
    let seq = t.server.state.store.node_seq(t.work, t.keys[1]).await.unwrap();
    let post = baruch_core::Post {
        id: 0,
        created: String::new(),
        author: "@lektor".into(),
        body: body.into(),
        sources: vec![],
        refs: vec![],
    };
    let id = t.server.state.store.add_post(t.work, seq, &post).await.unwrap();
    let source = baruch_server::findings::IngestSource {
        work: t.work,
        origin: baruch_server::findings::IngestOrigin::Post { id, key: t.keys[1] },
        persona: "@lektor",
        text: body,
        summary: false,
        manual: false,
    };
    let report = baruch_server::findings::ingest(&t.server.state, &source).await.unwrap();
    (id, report.created[0].id)
}

fn variant_answer(summary: &str, finding: i64, sentence: &str) -> Step {
    Step::words(&format!(
        "Zeile: {summary}\nBefunde: [B{finding}]\nText:\n== Der Leuchtturm\n\nDer Leuchtturm steht am Kai. {sentence}\n"
    ))
}

#[tokio::test]
async fn three_variants_are_siblings_on_the_same_base_with_labels_lines_and_findings() {
    let _turn = common::model_queue(module_path!()).await;
    let t = tower([]).await;
    let (_, finding) = finding_with_solution(&t).await;
    t.model.push(variant_answer("knapp, nur das Wort", finding, "Meist leuchtet der Turm über der Mole."));
    t.model.push(variant_answer("gründlich, Satz umgestellt", finding, "Über der Mole leuchtet meist der Turm."));
    t.model.push(variant_answer("frei, neues Bild", finding, "Der Turm hält Wache über der Mole."));
    let answer = t
        .server
        .rpc("solution.variants", json!({ "work": t.work, "key": t.keys[1], "findings": [finding], "count": 3 }))
        .await;
    let r = result(&answer).clone();
    assert_eq!(r["persona"], "@lektor", "der Lektor nach Einstellung schreibt: {r}");
    assert_eq!(r["base"], 1, "Basis ist die aktuelle Fassung");
    let labels: Vec<&str> = r["variants"].as_array().unwrap().iter().map(|v| v["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["A", "B", "C"], "drei Varianten mit Etikett: {r}");

    let seq = t.server.state.store.node_seq(t.work, t.keys[1]).await.unwrap();
    let tree = t.server.state.store.revision_tree(t.work).await.unwrap();
    let infos = &tree.iter().find(|n| n.node == seq).unwrap().revisions;
    let group = r["group"].as_str().unwrap();
    let siblings: Vec<_> = infos.iter().filter(|i| i.variant.as_ref().is_some_and(|v| v.group == group)).collect();
    assert_eq!(siblings.len(), 3, "drei Geschwister in einer Gruppe");
    for s in &siblings {
        assert_eq!(s.based_on, Some(1), "Fassung {}: gleiche Basis", s.number);
        assert_eq!(s.status, baruch_core::RevisionStatus::Proposed, "Fassung {}: nur Vorschlag", s.number);
        assert_eq!(s.variant.as_ref().unwrap().findings, vec![finding], "Fassung {}: setzt den Befund um", s.number);
    }
    assert_eq!(siblings[0].variant.as_ref().unwrap().summary, "knapp, nur das Wort", "Zeile aus der Antwort");
    let work = t.server.state.store.load_work(t.work).await.unwrap();
    assert_eq!(work.nodes[seq].current_number(), Some(1), "übernommen wird nichts – das bleibt beim Menschen");

    let calls = t.model.calls();
    assert_eq!(calls.len(), 3, "je Variante ein Aufruf, nacheinander");
    assert!(user(&calls[0]).contains(variants::VARIANT_STYLES[0]), "Richtung A: knapp");
    assert!(
        user(&calls[0]).contains(&format!("[B{finding}]"))
            && user(&calls[0]).contains("besser „Meist leuchtet der Turm“"),
        "Befund mit Lösung gerahmt im Aufruf: {}",
        user(&calls[0])
    );
    assert!(
        user(&calls[2]).contains("Schon geschrieben") && user(&calls[2]).contains("A: knapp"),
        "C weiß, was A und B tun"
    );

    // Revision tree and comparison page.
    let (_, page) = t.server.get(&format!("/?work={}&key={}", t.work, t.keys[1])).await;
    assert!(page.contains("Variante A") && page.contains("Variante C"), "der Fassungsbaum zeigt die Etiketten");
    assert!(page.contains(&format!("/varianten?work={}&amp;group={group}", t.work)), "Link „vergleichen“: {page}");
    let (status, cmp) = t.server.get(&format!("/varianten?work={}&key={}&group={group}", t.work, t.keys[1])).await;
    assert_eq!(status, StatusCode::OK, "{cmp}");
    assert_eq!(cmp.matches("class=\"variant-col").count(), 3, "drei Spalten nebeneinander");
    assert!(cmp.contains("<ins class=\"d-add\">") && cmp.contains("<del class=\"d-del\">"), "Unterschied zur Basis");
    assert!(cmp.contains("gegenüber Variante B"), "… und untereinander");
    assert!(cmp.contains("value=\"solution.reject_siblings\""), "„übrige verwerfen“ ohne JavaScript");
    let (status, by_group) = t.server.get(&format!("/varianten?work={}&group={group}", t.work)).await;
    assert_eq!(status, StatusCode::OK, "ohne key bestimmt die Gruppe den Abschnitt: {by_group}");
    let compare =
        result(&t.server.rpc("solution.compare", json!({ "work": t.work, "key": t.keys[1], "group": group })).await)
            .clone();
    assert_eq!(compare["base"], 1, "Vergleich nennt die Basis: {compare}");
    assert!(compare["variants"][0]["diff_html"].as_str().unwrap().contains("d-add"), "Diff als HTML");
}

#[tokio::test]
async fn rejecting_the_siblings_keeps_the_chosen_variant_and_needs_a_confirmation() {
    let _turn = common::model_queue(module_path!()).await;
    let t = tower([]).await;
    let (_, finding) = finding_with_solution(&t).await;
    t.model.push(variant_answer("knapp", finding, "Meist leuchtet der Turm über der Mole."));
    t.model.push(variant_answer("gründlich", finding, "Über der Mole leuchtet meist der Turm."));
    let r = result(
        &t.server.rpc("solution.variants", json!({ "work": t.work, "key": t.keys[1], "findings": [finding] })).await,
    )
    .clone();
    assert_eq!(r["variants"].as_array().unwrap().len(), 2, "Vorgabe: zwei Varianten");
    let (a, b) = (r["variants"][0]["rev"].as_u64().unwrap(), r["variants"][1]["rev"].as_u64().unwrap());
    let refused = t.server.rpc("solution.reject_siblings", json!({ "work": t.work, "key": t.keys[1], "rev": a })).await;
    assert_eq!(refused["error"]["code"], -32602, "ohne confirm nichts verworfen: {refused}");
    let form = format!("method=solution.reject_siblings&work={}&key={}&rev={a}&confirm=1", t.work, t.keys[1]);
    let (status, location) = t.server.post_form("/loesungen/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript: 303");
    assert!(location.unwrap().starts_with(&format!("/?work={}&key={}", t.work, t.keys[1])), "zurück zum Abschnitt");
    let work = t.server.state.store.load_work(t.work).await.unwrap();
    let node = work.nodes.iter().find(|n| n.key == t.keys[1]).unwrap();
    assert_eq!(node.revision(a as u32).unwrap().status, baruch_core::RevisionStatus::Proposed, "A bleibt Vorschlag");
    assert_eq!(node.revision(b as u32).unwrap().status, baruch_core::RevisionStatus::Rejected, "B verworfen");
    let plain = t
        .server
        .rpc("solution.reject_siblings", json!({ "work": t.work, "key": t.keys[1], "rev": 1, "confirm": true }))
        .await;
    assert_eq!(plain["error"]["code"], -32602, "die aktuelle Fassung ist keine Variante: {plain}");
}

#[tokio::test]
async fn a_wrong_count_is_refused_and_without_findings_no_model_is_asked() {
    let _turn = common::model_queue(module_path!()).await;
    let t = tower([]).await;
    let wrong = t.server.rpc("solution.variants", json!({ "work": t.work, "key": t.keys[1], "count": 4 })).await;
    assert_eq!(wrong["error"]["code"], -32602, "höchstens 3 Varianten: {wrong}");
    let none = t.server.rpc("solution.variants", json!({ "work": t.work, "key": t.keys[1] })).await;
    assert_eq!(none["error"]["code"], -32000, "ohne Befunde nichts umzusetzen: {none}");
    assert!(none["error"]["message"].as_str().unwrap().contains("Keine Befunde"), "ehrlich: {none}");
    assert!(t.model.calls().is_empty(), "kein Modellaufruf ohne Auftrag");
}

#[tokio::test]
async fn a_long_section_gets_partial_variants_for_the_affected_paragraph_only() {
    let _turn = common::model_queue(module_path!()).await;
    let filler = |n: usize| format!("Absatz {n}: {}", "Die Brücke trägt den Wagen über den Fluss. ".repeat(20));
    let long = format!(
        "#set document(title: \"Das Brückenbuch\")\n\n= Ankunft\n\nDas Boot legt an.\n\n== Die Brücke\n\n{}\n\n{}\n\n\
         Immer leuchtet der Turm über der Mole.\n\n{}\n\n{}\n",
        filler(1),
        filler(2),
        filler(4),
        filler(5)
    );
    let t = tower_with(&long, []).await;
    let (_, finding) = {
        let body = "Kritisch: „Immer leuchtet der Turm“ behauptet zu viel.\n- Schritt: abschwächen.";
        let seq = t.server.state.store.node_seq(t.work, t.keys[1]).await.unwrap();
        let post = baruch_core::Post {
            id: 0,
            created: String::new(),
            author: "@lektor".into(),
            body: body.into(),
            sources: vec![],
            refs: vec![],
        };
        let id = t.server.state.store.add_post(t.work, seq, &post).await.unwrap();
        let source = baruch_server::findings::IngestSource {
            work: t.work,
            origin: baruch_server::findings::IngestOrigin::Post { id, key: t.keys[1] },
            persona: "@lektor",
            text: body,
            summary: false,
            manual: false,
        };
        (id, baruch_server::findings::ingest(&t.server.state, &source).await.unwrap().created[0].id)
    };
    // Context length so small that the whole section plus answer does not fit, but one paragraph does.
    result(&t.server.rpc("persona.update", json!({ "handle": "@lektor", "context_length": 2600 })).await);
    for summary in ["knapp", "gründlich"] {
        t.model.push(Step::words(&format!(
            "Zeile: {summary}\nBefunde: [B{finding}]\nText:\n[[Absatz 4]]\nMeist leuchtet der Turm über der Mole ({summary}).\n"
        )));
    }
    let r = result(
        &t.server.rpc("solution.variants", json!({ "work": t.work, "key": t.keys[1], "findings": [finding] })).await,
    )
    .clone();
    assert!(
        r["notes"].as_array().unwrap().iter().any(|n| n.as_str().unwrap().contains("Teilvorschlag")),
        "der Hinweis nennt den Teilvorschlag: {r}"
    );
    let call = &t.model.calls()[0];
    assert!(user(call).contains("[[Absatz 4]]\nImmer leuchtet der Turm"), "nur der betroffene Absatz geht hinaus");
    assert!(!user(call).contains("Absatz 2: Die Brücke"), "die übrigen Absätze nicht (der Kontext reicht sonst nicht)");
    let rev = r["variants"][0]["rev"].as_u64().unwrap() as u32;
    let work = t.server.state.store.load_work(t.work).await.unwrap();
    let node = work.nodes.iter().find(|n| n.key == t.keys[1]).unwrap();
    let body = &node.revision(rev).unwrap().body;
    assert!(body.contains("Meist leuchtet der Turm über der Mole (knapp)."), "der neue Absatz steht an seiner Stelle");
    assert!(body.contains(&filler(1)) && body.contains(&filler(5)), "der Rest bleibt wörtlich");
    assert!(!body.contains("Immer leuchtet"), "der alte Absatz ist ersetzt");
}

#[tokio::test]
async fn a_chat_message_asking_for_variants_lets_the_lector_write_them() {
    let _turn = common::model_queue(module_path!()).await;
    let t = tower([]).await;
    let (_, finding) = finding_with_solution(&t).await;
    t.model.push(variant_answer("knapp", finding, "Meist leuchtet der Turm über der Mole."));
    t.model.push(variant_answer("gründlich", finding, "Über der Mole leuchtet meist der Turm."));
    let message = format!("@lektor setze [B{finding}] in Varianten um");
    let chat = ChatOrder {
        work: t.work,
        key: t.keys[1],
        message: &message,
        rev: None,
        author: "Autor",
        extra: None,
        web: false,
        as_order: false,
        basis: Default::default(),
        relay: false,
    };
    let outcome = baruch_server::assist::chat(&t.server.state, &chat, None).await.unwrap();
    assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
    assert_eq!(outcome.proposals.len(), 2, "zwei Varianten als Vorschläge: {:?}", outcome.proposals);
    assert_eq!(outcome.posts.len(), 1, "nur die Nachricht des Autors, kein Persona-Beitrag statt der Varianten");
    assert!(variants::is_variants_order(&message), "das Wort „Varianten“ macht den Auftrag");
}

#[tokio::test]
async fn the_form_writes_variants_without_javascript_and_leads_to_the_comparison() {
    let _turn = common::model_queue(module_path!()).await;
    let t = tower([]).await;
    let (post, finding) = finding_with_solution(&t).await;
    t.model.push(variant_answer("knapp", finding, "Meist leuchtet der Turm über der Mole."));
    t.model.push(variant_answer("gründlich", finding, "Über der Mole leuchtet meist der Turm."));
    let form = format!("method=solution.variants&work={}&key={}&post={post}&count=2", t.work, t.keys[1]);
    let (status, location) = t.server.post_form("/loesungen/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "ohne JavaScript: 303");
    let location = location.unwrap();
    assert!(
        location.starts_with(&format!("/varianten?work={}&key={}&group=", t.work, t.keys[1])),
        "zum Vergleich: {location}"
    );
    let (status, _) = t.server.get(&location).await;
    assert_eq!(status, StatusCode::OK, "die Vergleichsseite öffnet");
    assert!(user(&t.model.calls()[0]).contains(&format!("[B{finding}]")), "aus dem Beitrag: dessen Befunde");
}

// ---- MCP -------------------------------------------------------------------------------------------------

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40001))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

#[tokio::test]
async fn mcp_solution_variants_creates_only_proposals_and_is_no_stub() {
    let _turn = common::model_queue(module_path!()).await;
    let t = tower([]).await;
    let (_, finding) = finding_with_solution(&t).await;
    t.model.push(variant_answer("knapp", finding, "Meist leuchtet der Turm über der Mole."));
    t.model.push(variant_answer("gründlich", finding, "Über der Mole leuchtet meist der Turm."));
    let (headers, init) = mcp(
        &t.server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "varianten-probe", "version": "1" } } }),
    )
    .await;
    assert!(init["result"].is_object(), "{init}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let (_, listed) =
        mcp(&t.server, Some(&session), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let tool = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "solution_variants")
        .unwrap()
        .clone();
    let text = tool["description"].as_str().unwrap();
    assert!(!text.contains("Stub") && !text.contains("noch nicht umgesetzt"), "kein Stub mehr: {text}");
    let (_, answer) = mcp(
        &t.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "solution_variants", "arguments": { "work": t.work, "key": t.keys[1], "findings": [finding] } } }),
    )
    .await;
    assert_eq!(answer["result"]["isError"], false, "gelingt: {answer}");
    let content = &answer["result"]["structuredContent"];
    assert_eq!(content["status"], "proposed", "nur Vorschläge: {content}");
    assert_eq!(content["variants"].as_array().unwrap().len(), 2, "zwei Varianten");
    let (_, wrong) = mcp(
        &t.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": { "name": "solution_variants", "arguments": { "work": t.work, "key": t.keys[1], "count": 5 } } }),
    )
    .await;
    assert_eq!(wrong["result"]["isError"], true, "count 5 → isError: {wrong}");
    // MCP `findings` returns the solutions of each finding.
    let (_, listed) = mcp(
        &t.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                "params": { "name": "findings", "arguments": { "work": t.work, "key": t.keys[1] } } }),
    )
    .await;
    let found = &listed["result"]["structuredContent"]["findings"][0];
    assert_eq!(found["solutions"][0]["replacement"], "Meist leuchtet der Turm", "Lösung im MCP-Ergebnis: {listed}");
}

// ---- Exchange minor version 5 ------------------------------------------------------------------------------

#[tokio::test]
async fn solutions_variants_and_post_links_travel_to_another_instance_in_version_five() {
    let _turn = common::model_queue(module_path!()).await;
    let t = tower([]).await;
    let (_, finding) = finding_with_solution(&t).await;
    t.model.push(variant_answer("knapp", finding, "Meist leuchtet der Turm über der Mole."));
    t.model.push(variant_answer("gründlich", finding, "Über der Mole leuchtet meist der Turm."));
    result(
        &t.server.rpc("solution.variants", json!({ "work": t.work, "key": t.keys[1], "findings": [finding] })).await,
    );
    t.model.push(Step::words("Hinweis: Der Wächter trägt."));
    result(
        &t.server
            .rpc("persona.ask", json!({ "work": t.work, "key": t.keys[1], "persona": "@lektor", "question": "Und?" }))
            .await,
    );
    let xml = result(&t.server.rpc("work.exchange_export", json!({ "work": t.work, "format": "xml" })).await)["xml"]
        .as_str()
        .unwrap()
        .to_owned();
    for needle in [
        // Round 34 (mindmap_daten): the writer now writes minor version 9 (tree of the ideas).
        "version=\"10\"",
        "variante=\"A\"",
        "variante-zeile=\"knapp\"",
        "<setzt-um befund=",
        "<loesung zitat=",
        "grundlage=\"geprüft: aktuelle Fassung 1\"",
        "schluessel=",
    ] {
        assert!(xml.contains(needle), "{needle} in der Ausgabe: {xml}");
    }
    // XSD of minor version 5 (if xmllint is missing, honestly skipped).
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("u5.baruch.xml");
    std::fs::write(&file, &xml).unwrap();
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
    // Second instance: import as a new work, then write out again – identical except for <ausgabe> and the keys.
    let b = common::open().await;
    let imported = b
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()), "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": "new" }),
        )
        .await;
    let work_b = result(&imported)["work"].as_i64().unwrap();
    let xml_b = result(&b.rpc("work.exchange_export", json!({ "work": work_b, "format": "xml" })).await)["xml"]
        .as_str()
        .unwrap()
        .to_owned();
    let normal = |text: &str| -> String {
        let start = text.find("<ausgabe").unwrap();
        let end = start + text[start..].find("/>").unwrap() + 2;
        let rest = format!("{}{}", &text[..start], &text[end..]);
        // Keys differ per instance (schluessel="N").
        let mut out = String::new();
        let mut tail = rest.as_str();
        while let Some(at) = tail.find(" schluessel=\"") {
            out.push_str(&tail[..at]);
            let after = &tail[at + " schluessel=\"".len()..];
            tail = &after[after.find('"').unwrap() + 1..];
        }
        out.push_str(tail);
        out
    };
    assert_eq!(normal(&xml_b), normal(&xml), "Rundreise der Unterversion 5 ohne Verlust");
    let tree = b.state.store.revision_tree(work_b).await.unwrap();
    let count = tree.iter().flat_map(|n| &n.revisions).filter(|r| r.variant.is_some()).count();
    assert_eq!(count, 2, "beide Varianten sind in der zweiten Instanz Varianten");
}
