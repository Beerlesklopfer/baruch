// SPDX-License-Identifier: AGPL-3.0-or-later
//! Fehler 0.9.0 (Nutzer 2026-11-25, „Ablauffehler“): Ein Kritiker prüft den Vorschlag, den der Lektor aus seinen
//! Befunden geschrieben hat – iterativ, über alle Wege (Persona-Formular mit Prüfgrundlage, Chat „@nathan …“ beim
//! gezeigten Vorschlag, MCP `ask_persona` mit `rev`), und der Lektor schreibt auf dem Vorschlag weiter.
//!
//! Modelle nur über den Nachbau `MockEliezer` (nie Eliezer, nie Netz). Das „Bootshaus“ ist frei erfunden.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use baruch_llm::mock::{MockEliezer, Reply};
use baruch_server::assist::eliezer::EliezerPort;
use baruch_server::assist::prompts;
use baruch_store::FindingStatus;
use serde_json::{Value, json};

use crate::common;

const BOOK: &str = "#set document(title: \"Das Bootshaus\", author: \"Mira Kiel\")\n#set heading(numbering: \"1.\")\n\n\
= Am Kai\n\nDie Bootsfrau knotet das graue Tau an den Poller. Möwen kreisen über dem Hafen.\n\n\
= Abfahrt\n\nNiemand winkt. Nur die Glocke schlägt.\n";

struct Harbour {
    server: common::Server,
    eliezer: MockEliezer,
    work: i64,
    key: i64,
}

impl Harbour {
    fn last_user(&self) -> String {
        let body = self.eliezer.chat_bodies().pop().expect("ein Aufruf ging an den Nachbau");
        body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "user")
            .map(|m| m["content"].as_str().unwrap_or("").to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    async fn form(&self, form: &str) -> StatusCode {
        let request = Request::post("/assistenten/act")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_owned()))
            .unwrap();
        self.server.send(request).await.0
    }

    async fn based_on(&self, rev: u32) -> Option<u32> {
        sqlx::query_scalar("SELECT based_on FROM revision WHERE node_id = ? AND number = ?")
            .bind(self.key)
            .bind(rev)
            .fetch_one(self.server.state.store.pool())
            .await
            .unwrap()
    }

    async fn last_post_meta(&self, author: &str) -> baruch_store::PostMeta {
        let loaded = self.server.state.store.load_work(self.work).await.unwrap();
        let node = loaded.nodes.iter().find(|n| n.key == self.key).unwrap();
        let post = node.posts.iter().rev().find(|p| p.author == author).expect("Beitrag der Persona");
        self.server.state.store.post_meta(self.work, &[post.id]).await.unwrap().remove(&post.id).unwrap()
    }

    async fn mcp(&self, arguments: Value) -> Value {
        use axum::extract::ConnectInfo;
        use std::net::SocketAddr;
        let mut request = Request::post("/mcp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call",
                        "params": { "name": "ask_persona", "arguments": arguments } })
                .to_string(),
            ))
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50_000))));
        let (status, _, text) = self.server.send(request).await;
        assert_eq!(status, StatusCode::OK, "MCP antwortet: {text}");
        let answer: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(answer["result"]["isError"], false, "ask_persona gelingt: {text}");
        answer
    }
}

async fn harbour() -> Harbour {
    let data = tempfile::tempdir().unwrap();
    let state = baruch_server::AppState::open_with(
        data.path(),
        Arc::new(baruch_server::connections::MemorySecrets::default()),
        Arc::new(EliezerPort),
    )
    .await
    .expect("Datenordner lässt sich nicht öffnen");
    let state = Arc::new(state);
    let server = common::Server { app: baruch_server::router(state.clone()), state, data };
    let eliezer = MockEliezer::start().await;
    let answer = server.rpc("connection.set", json!({ "kind": "eliezer", "config": { "url": eliezer.url() } })).await;
    assert!(answer["error"].is_null(), "Verbindung zum Nachbau: {answer}");
    let report = server.import(BOOK, "bootshaus.typ").await;
    let key = server.state.store.load_work(report.work).await.unwrap().nodes[0].key;
    Harbour { server, eliezer, work: report.work, key }
}

const PROPOSAL_2: &str =
    "= Am Kai\n\nDie Bootsfrau knotet das nasse Tau an den Poller. Möwen kreisen über dem Hafen.\n";
const PROPOSAL_3: &str =
    "= Am Kai\n\nDie Bootsfrau knotet das nasse Tau an den Poller. Möwen schreien über dem Hafen.\n";

#[tokio::test]
async fn critic_and_lector_iterate_on_proposals_over_all_ways() {
    let _turn = common::model_queue(module_path!()).await;
    let h = harbour().await;
    let (work, key) = (h.work, h.key);

    // (1) @nathan prüft die aktuelle Fassung (Persona-Formular „Prüfen“).
    h.eliezer.push(Reply::text(&["Kritisch: „graue Tau“ ist zu blass.\n- statt: „graue Tau“\n- besser: „nasse Tau“"]));
    let status =
        h.form(&format!("method=persona.review&work={work}&key={key}&rev=1&persona=%40nathan&basis=shown")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Prüfen über das Formular");
    let open = h.server.state.store.finding_list(work, &Default::default()).await.unwrap();
    let first = open.iter().find(|f| f.persona == "@nathan").expect("Befund von @nathan").id;

    // (2) Der Lektor schreibt daraus einen Vorschlag (Formular „Vorschlag“).
    h.eliezer.push(Reply::text(&[PROPOSAL_2]));
    let status = h.form(&format!("method=persona.propose&work={work}&key={key}&rev=1&persona=%40lektor")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Vorschlag über das Formular");
    assert_eq!(h.based_on(2).await, Some(1), "Vorschlag 2 beruht auf Fassung 1");

    // Die Prüfgrundlage bietet den offenen Vorschlag an (ohne JavaScript).
    let (_, html) = h.server.get(&format!("/?work={work}&key={key}")).await;
    assert!(html.contains("<option value=\"2\">Vorschlag 2 von @lektor</option>"), "Auswahl nennt Vorschlag 2");

    // (3) @nathan prüft DIESEN Vorschlag gegen – Formular, Prüfgrundlage „2“ bei gezeigter aktueller Fassung.
    let reply = format!(
        "Erledigt: Befund {first} (Stelle „nasse Tau“)\nKritisch: „nasse Tau“ und „Poller“ klingen hart.\n- Schritt: den Satz weicher bauen."
    );
    h.eliezer.push(Reply::text(&[&reply]));
    let status = h.form(&format!("method=persona.review&work={work}&key={key}&rev=1&persona=%40nathan&basis=2")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Gegenprüfen über das Formular");
    let user = h.last_user();
    assert!(user.contains("(Fassung 2)") && user.contains("nasse Tau"), "der Kritiker sieht den Vorschlag: {user}");
    assert!(user.contains(prompts::OWN_FINDINGS_TITLE) && user.contains(&format!("[B{first}]")), "eigene Befunde");
    let meta = h.last_post_meta("@nathan").await;
    assert_eq!(meta.basis_label, "geprüft: Fassung 2 (Vorschlag von @lektor)", "Kopfzeile nennt den Vorschlag");
    let pending = h.server.state.store.finding_get(work, first).await.unwrap().pending.expect("Statusvorschlag");
    assert_eq!(pending.status, FindingStatus::Done, "erledigt am Vorschlag → Statusvorschlag");
    let newest = h.server.state.store.finding_list(work, &Default::default()).await.unwrap();
    let new = newest.iter().filter(|f| f.persona == "@nathan").max_by_key(|f| f.id).unwrap();
    assert!(new.id != first && new.body.contains("klingen hart"), "neuer Befund zum Vorschlag: {}", new.body);
    let post = h.server.state.store.load_work(work).await.unwrap().nodes[0].posts.last().unwrap().body.clone();
    assert!(!post.contains(prompts::UNBACKED_NOTE), "Zitat aus dem Vorschlag gilt: {post}");

    // (4) Der Lektor schreibt auf dem Vorschlag weiter (RPC persona.propose mit rev 2).
    h.eliezer.push(Reply::text(&[PROPOSAL_3]));
    let answer =
        h.server.rpc("persona.propose", json!({ "work": work, "key": key, "rev": 2, "persona": "@lektor" })).await;
    assert!(answer["error"].is_null(), "Vorschlag auf dem Vorschlag: {answer}");
    assert_eq!(answer["result"]["rev"], 3, "{answer}");
    assert_eq!(h.based_on(3).await, Some(2), "Vorschlag 3 beruht auf Vorschlag 2, nicht auf der aktuellen Fassung");

    // Chat „@nathan …“ bei gezeigtem Vorschlag 3: Gegenprüfung mit den eigenen offenen Befunden.
    h.eliezer.push(Reply::text(&["Kritisch: „schreien“ ist laut.\n- Schritt: leiser."]));
    let answer = h
        .server
        .rpc(
            "persona.chat",
            json!({ "work": work, "key": key, "rev": 3, "message": "@nathan prüf bitte den Vorschlag" }),
        )
        .await;
    assert!(answer["error"].is_null(), "Chat gelingt: {answer}");
    let user = h.last_user();
    assert!(user.contains("(Fassung 3)") && user.contains("schreien"), "Chat: Kritiker sieht Vorschlag 3: {user}");
    assert!(user.contains(prompts::OWN_FINDINGS_TITLE), "Chat: eigene offene Befunde zum Nachprüfen: {user}");
    let meta = h.last_post_meta("@nathan").await;
    assert_eq!(meta.basis_label, "geprüft: Fassung 3 (Vorschlag von @lektor)", "Chat: Kopfzeile");

    // MCP ask_persona review mit rev 3 und propose mit rev 3.
    h.eliezer.push(Reply::text(&["Kritisch: „Hafen“ bleibt blass.\n- Schritt: den Hafen benennen."]));
    let answer = h.mcp(json!({ "work": work, "key": key, "persona": "@nathan", "mode": "review", "rev": 3 })).await;
    let user = h.last_user();
    assert!(user.contains("(Fassung 3)") && user.contains(prompts::OWN_FINDINGS_TITLE), "MCP review: {user}");
    assert!(answer.to_string().contains("Vorschlag von @lektor"), "MCP nennt die Prüfgrundlage: {answer}");
    h.eliezer.push(Reply::text(&["= Am Kai\n\nDie Bootsfrau knotet das nasse Tau an den Poller. Möwen rufen.\n"]));
    h.mcp(json!({ "work": work, "key": key, "persona": "@lektor", "mode": "propose", "rev": 3 })).await;
    assert_eq!(h.based_on(4).await, Some(3), "MCP: Vorschlag 4 beruht auf Vorschlag 3");
}

/// Gegenprobe 0.9.0: Das Persona-Formular trägt die Prüfgrundlage genau einmal. Doppelt (Umfang-Fieldset und
/// Formular) galt mit JavaScript still nur der letzte Wert – „Vorschlag n“ im ersten Feld wäre verloren gegangen –,
/// und ohne JavaScript scheitert das Lesen des Formulars an einem doppelten Feld.
#[tokio::test]
async fn the_persona_form_carries_the_basis_exactly_once() {
    let h = harbour().await;
    let (work, key) = (h.work, h.key);
    let (_, html) = h.server.get(&format!("/?work={work}&key={key}")).await;
    let form = html.split("<form class=\"assist-form\"").nth(1).expect("Persona-Formular auf der Arbeitsseite");
    let form = form.split("</form>").next().unwrap();
    assert_eq!(form.matches("name=\"basis\"").count(), 1, "genau eine Auswahl der Prüfgrundlage: {form}");
}
