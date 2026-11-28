// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bug 0.9.0 (user 2026-11-25, „Ablauffehler“): a critic checks the proposal that the lector wrote from his findings –
//! iteratively, through all ways (persona form with basis of the check, chat „@nathan …“ at the proposal shown, MCP
//! `ask_persona` with `rev`), and the lector keeps writing on the proposal.
//!
//! Models only through the mock `MockEliezer` (never Eliezer, never the network). The „Bootshaus“ is freely invented.

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

    // (1) @nathan checks the current revision (persona form „Prüfen“).
    h.eliezer.push(Reply::text(&["Kritisch: „graue Tau“ ist zu blass.\n- statt: „graue Tau“\n- besser: „nasse Tau“"]));
    let status =
        h.form(&format!("method=persona.review&work={work}&key={key}&rev=1&persona=%40nathan&basis=shown")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Prüfen über das Formular");
    let open = h.server.state.store.finding_list(work, &Default::default()).await.unwrap();
    let first = open.iter().find(|f| f.persona == "@nathan").expect("Befund von @nathan").id;

    // (2) The lector writes a proposal from it (form „Vorschlag“).
    h.eliezer.push(Reply::text(&[PROPOSAL_2]));
    let status = h.form(&format!("method=persona.propose&work={work}&key={key}&rev=1&persona=%40lektor")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Vorschlag über das Formular");
    assert_eq!(h.based_on(2).await, Some(1), "Vorschlag 2 beruht auf Fassung 1");

    // The basis of the check offers the open proposal (without JavaScript).
    let (_, html) = h.server.get(&format!("/?work={work}&key={key}")).await;
    assert!(html.contains("<option value=\"2\">Vorschlag 2 von @lektor</option>"), "Auswahl nennt Vorschlag 2");

    // (3) @nathan cross-checks THIS proposal – form, basis of the check „2“ while the current revision is shown.
    let reply = format!(
        "Erledigt: Befund {first} (Stelle „nasse Tau“)\nKritisch: „nasse Tau“ und „Poller“ klingen hart.\n- Schritt: den Satz weicher bauen."
    );
    h.eliezer.push(Reply::text(&[&reply]));
    let status = h.form(&format!("method=persona.review&work={work}&key={key}&rev=1&persona=%40nathan&basis=2")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Gegenprüfen über das Formular");
    let user = h.last_user();
    assert!(user.contains("(revision 2)") && user.contains("nasse Tau"), "der Kritiker sieht den Vorschlag: {user}");
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

    // (4) The lector keeps writing on the proposal (RPC persona.propose with rev 2).
    h.eliezer.push(Reply::text(&[PROPOSAL_3]));
    let answer =
        h.server.rpc("persona.propose", json!({ "work": work, "key": key, "rev": 2, "persona": "@lektor" })).await;
    assert!(answer["error"].is_null(), "Vorschlag auf dem Vorschlag: {answer}");
    assert_eq!(answer["result"]["rev"], 3, "{answer}");
    assert_eq!(h.based_on(3).await, Some(2), "Vorschlag 3 beruht auf Vorschlag 2, nicht auf der aktuellen Fassung");

    // Chat „@nathan …“ while proposal 3 is shown: cross-check with the own open findings.
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
    assert!(user.contains("(revision 3)") && user.contains("schreien"), "Chat: Kritiker sieht Vorschlag 3: {user}");
    assert!(user.contains(prompts::OWN_FINDINGS_TITLE), "Chat: eigene offene Befunde zum Nachprüfen: {user}");
    let meta = h.last_post_meta("@nathan").await;
    assert_eq!(meta.basis_label, "geprüft: Fassung 3 (Vorschlag von @lektor)", "Chat: Kopfzeile");

    // MCP ask_persona review with rev 3 and propose with rev 3.
    h.eliezer.push(Reply::text(&["Kritisch: „Hafen“ bleibt blass.\n- Schritt: den Hafen benennen."]));
    let answer = h.mcp(json!({ "work": work, "key": key, "persona": "@nathan", "mode": "review", "rev": 3 })).await;
    let user = h.last_user();
    assert!(user.contains("(revision 3)") && user.contains(prompts::OWN_FINDINGS_TITLE), "MCP review: {user}");
    assert!(answer.to_string().contains("Vorschlag von @lektor"), "MCP nennt die Prüfgrundlage: {answer}");
    h.eliezer.push(Reply::text(&["= Am Kai\n\nDie Bootsfrau knotet das nasse Tau an den Poller. Möwen rufen.\n"]));
    h.mcp(json!({ "work": work, "key": key, "persona": "@lektor", "mode": "propose", "rev": 3 })).await;
    assert_eq!(h.based_on(4).await, Some(3), "MCP: Vorschlag 4 beruht auf Vorschlag 3");
}

/// Counter-check 0.9.0: the persona form carries the basis of the check exactly once. Doubled (scope fieldset and
/// form), with JavaScript silently only the last value held – „Vorschlag n“ in the first field would have been lost –,
/// and without JavaScript reading the form fails on a duplicate field.
#[tokio::test]
async fn the_persona_form_carries_the_basis_exactly_once() {
    let h = harbour().await;
    let (work, key) = (h.work, h.key);
    let (_, html) = h.server.get(&format!("/?work={work}&key={key}")).await;
    let form = html.split("<form class=\"assist-form\"").nth(1).expect("Persona-Formular auf der Arbeitsseite");
    let form = form.split("</form>").next().unwrap();
    assert_eq!(form.matches("name=\"basis\"").count(), 1, "genau eine Auswahl der Prüfgrundlage: {form}");
}

// ---- Round 36: proposal for the whole chapter on the right basis -----------------------------------------------

const CHAPTER_BOOK: &str = "#set document(title: \"Die Schleuse\", author: \"Mira Kiel\")\n#set heading(numbering: \"1.\")\n\n\
= Oberwasser\n\nDas Tor steht offen. Ein Kahn wartet vor der Kammer.\n\n\
== Die Kammer\n\nDas Wasser steigt langsam. Der Schleusenwärter zählt die Stufen.\n";
const CHAMBER_2: &str = "== Die Kammer\n\nDas Wasser steigt gurgelnd. Der Schleusenwärter zählt die Stufen.\n";
const CHAMBER_3: &str = "== Die Kammer\n\nDas Wasser steigt gurgelnd. Die Schleusenwärterin zählt die Stufen.\n";
const UPPER_2: &str = "= Oberwasser\n\nDas Tor steht weit offen. Ein Kahn wartet vor der Kammer.\n";

/// `based_on` of revision `rev` of section `key`.
async fn based_on_of(h: &Harbour, key: i64, rev: u64) -> Option<u32> {
    sqlx::query_scalar("SELECT based_on FROM revision WHERE node_id = ? AND number = ?")
        .bind(key)
        .bind(rev as i64)
        .fetch_one(h.server.state.store.pool())
        .await
        .unwrap()
}

/// Note from 0.9.0: the proposal for the whole chapter (`scope: chapter`) was, for its own part, always based on the
/// current revision, even if a proposal was shown or chosen and the persona revised exactly that one; the response
/// named no basis. Now it is based on the chosen or shown revision and names it.
#[tokio::test]
async fn a_proposal_for_the_whole_chapter_is_based_on_the_chosen_or_shown_version() {
    let _turn = common::model_queue(module_path!()).await;
    let h = harbour().await;
    let work = h.server.import(CHAPTER_BOOK, "schleuse.typ").await.work;
    let nodes = h.server.state.store.load_work(work).await.unwrap().nodes;
    let (upper, chamber) = (nodes[0].key, nodes[1].key);
    assert_eq!(
        (nodes[0].title.as_str(), nodes[1].title.as_str()),
        ("Oberwasser", "Die Kammer"),
        "Vorbedingung: ein Kapitel mit einem Unterabschnitt"
    );

    // The lector proposes revision 2 for the subsection (one section): the response names the basis.
    h.eliezer.push(Reply::text(&[CHAMBER_2]));
    let answer = h.server.rpc("persona.propose", json!({ "work": work, "key": chamber, "persona": "@lektor" })).await;
    assert!(answer["error"].is_null(), "Vorschlag für den Abschnitt: {answer}");
    assert_eq!(answer["result"]["rev"], 2, "{answer}");
    assert_eq!(
        answer["result"]["basis"],
        json!({ "rev": 1, "label": "Grundlage: aktuelle Fassung 1", "proposal": false }),
        "die Antwort nennt die Fassung, auf der der Vorschlag beruht"
    );

    // Whole chapter, shown is proposal 2 of the subsection: one call per part, in the order of the chapter.
    let before = h.eliezer.chat_bodies().len();
    h.eliezer.push(Reply::text(&[UPPER_2]));
    h.eliezer.push(Reply::text(&[CHAMBER_3]));
    let params = json!({ "work": work, "key": chamber, "rev": 2, "persona": "@lektor", "scope": "chapter" });
    let answer = h.server.rpc("persona.propose", params).await;
    assert!(answer["error"].is_null(), "Vorschlag für das ganze Kapitel: {answer}");
    let result = &answer["result"];
    let calls = h.eliezer.chat_bodies()[before..].to_vec();
    assert_eq!(calls.len(), 2, "je Teil mit Text ein Aufruf");
    let own_call = calls[1]["messages"].to_string();
    assert!(
        own_call.contains("steigt gurgelnd") && own_call.contains("(revision 2)"),
        "der eigene Teil steht in der gezeigten Fassung 2 im Auftrag (nicht in der aktuellen): {own_call}"
    );
    let proposals = result["proposals"].as_array().expect("die Antwort nennt die Vorschläge je Teil");
    let rev_of = |key: i64| proposals.iter().find(|p| p["key"] == key).map(|p| p["rev"].as_u64().unwrap());
    let (upper_rev, chamber_rev) = (rev_of(upper).expect("Teil Oberwasser"), rev_of(chamber).expect("Teil Kammer"));
    assert_eq!((upper_rev, chamber_rev), (2, 3), "je Teil genau ein neuer Vorschlag: {result}");
    assert_eq!(
        based_on_of(&h, chamber, chamber_rev).await,
        Some(2),
        "der Vorschlag für den eigenen Teil beruht auf der gezeigten Fassung 2 (vorher: stets auf der aktuellen)"
    );
    assert_eq!(
        based_on_of(&h, upper, upper_rev).await,
        Some(1),
        "die übrigen Teile standen in ihrer aktuellen Fassung im Auftrag und beruhen auf ihr"
    );
    assert_eq!(
        result["basis"],
        json!({ "rev": 2, "label": "Grundlage: Fassung 2 (Vorschlag von @lektor)", "proposal": true }),
        "Outcome::basis und ihre Kopfzeile nennen die gezeigte Fassung"
    );
    // The header line of the new proposal on the work page names the same basis (without JavaScript).
    let (_, page) = h.server.get(&format!("/?work={work}&key={chamber}&rev={chamber_rev}")).await;
    assert!(page.contains("beruht auf Fassung 2"), "der Kopf des Vorschlags nennt Fassung 2 als Grundlage");

    // Explicitly chosen basis „current“ while a proposal is shown: the own part is based on the current one.
    h.eliezer
        .push(Reply::text(&["= Oberwasser\n\nDas Tor steht sperrangelweit offen. Ein Kahn wartet vor der Kammer.\n"]));
    h.eliezer.push(Reply::text(&["== Die Kammer\n\nDas Wasser steigt träge. Der Schleusenwärter zählt die Stufen.\n"]));
    let params =
        json!({ "work": work, "key": chamber, "rev": 3, "basis": "current", "persona": "@lektor", "scope": "chapter" });
    let answer = h.server.rpc("persona.propose", params).await;
    assert!(answer["error"].is_null(), "Vorschlag fürs Kapitel auf der aktuellen Fassung: {answer}");
    let result = &answer["result"];
    assert_eq!(
        result["basis"],
        json!({ "rev": 1, "label": "Grundlage: aktuelle Fassung 1", "proposal": false }),
        "die gewählte Grundlage (aktuelle Fassung) steht in der Antwort"
    );
    let own = result["proposals"].as_array().unwrap().iter().find(|p| p["key"] == chamber).unwrap()["rev"].clone();
    assert_eq!(based_on_of(&h, chamber, own.as_u64().unwrap()).await, Some(1), "und ist die Basis des Vorschlags");

    // If the response for the own part equals the revision shown, no proposal is created for it.
    let count = async || -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM revision WHERE node_id IN (?, ?)")
            .bind(upper)
            .bind(chamber)
            .fetch_one(h.server.state.store.pool())
            .await
            .unwrap()
    };
    let revisions = count().await;
    let current_upper = nodes[0].revisions[0].body.clone();
    h.eliezer.push(Reply::text(&[&current_upper]));
    h.eliezer.push(Reply::text(&[CHAMBER_2]));
    let params = json!({ "work": work, "key": chamber, "rev": 2, "persona": "@lektor", "scope": "chapter" });
    let answer = h.server.rpc("persona.propose", params).await;
    assert_eq!(answer["error"]["code"], -32000, "nichts geändert → abgelehnt: {answer}");
    assert!(
        answer["error"]["message"].as_str().unwrap().contains("Die Kammer (unverändert)"),
        "ein Vorschlag, der der gezeigten Fassung gleicht, zählt als unverändert: {answer}"
    );
    assert_eq!(count().await, revisions, "nichts gespeichert");
}
